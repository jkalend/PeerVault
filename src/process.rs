use anyhow::{Context, Result};
use command_group::{CommandGroup, GroupChild};
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

static CANCELLED: AtomicBool = AtomicBool::new(false);
pub fn cancel() {
    CANCELLED.store(true, Ordering::SeqCst);
}
pub fn cancelled() -> bool {
    CANCELLED.load(Ordering::SeqCst)
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProcessLimit {
    Timeout,
    Output,
    Cancelled,
}

impl std::fmt::Display for ProcessLimit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "backend {:?} limit; output withheld", self)
    }
}
impl std::error::Error for ProcessLimit {}

struct OwnedTree(GroupChild);
impl Drop for OwnedTree {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn bounded(
    command: &mut Command,
    timeout: Duration,
    limit: u64,
) -> Result<(Option<i32>, String)> {
    bounded_with_cancel(command, timeout, limit, &CANCELLED)
}

fn bounded_with_cancel(
    command: &mut Command,
    timeout: Duration,
    limit: u64,
    cancellation: &AtomicBool,
) -> Result<(Option<i32>, String)> {
    if cancellation.load(Ordering::SeqCst) {
        return Err(ProcessLimit::Cancelled.into());
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut group = command.group();
    #[cfg(windows)]
    group.kill_on_drop(true).creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut child = OwnedTree(
        group
            .spawn()
            .context("cannot launch owned backend process tree")?,
    );
    let stdout = child
        .0
        .inner()
        .stdout
        .take()
        .context("missing stdout pipe")?;
    let stderr = child
        .0
        .inner()
        .stderr
        .take()
        .context("missing stderr pipe")?;
    let exceeded = AtomicBool::new(false);
    let start = Instant::now();
    // Pipes replace disk spooling. Read at most limit + one sentinel byte per
    // stream; bounded kernel pipes apply backpressure to the whole process tree.
    thread::scope(|scope| {
        let read = |stream: &mut dyn Read| -> std::io::Result<Vec<u8>> {
            let mut bytes = Vec::new();
            stream.take(limit + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > limit {
                exceeded.store(true, Ordering::SeqCst);
            }
            Ok(bytes)
        };
        let out = scope.spawn(move || read(&mut { stdout }));
        let err = scope.spawn(move || read(&mut { stderr }));
        let status: Result<_> = loop {
            if cancellation.load(Ordering::SeqCst) {
                break Err(ProcessLimit::Cancelled.into());
            }
            if exceeded.load(Ordering::SeqCst) {
                break Err(ProcessLimit::Output.into());
            }
            if start.elapsed() >= timeout {
                break Err(ProcessLimit::Timeout.into());
            }
            match child.0.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => thread::sleep(Duration::from_millis(5)),
                Err(_) => break Err(anyhow::anyhow!("backend process wait failed")),
            }
        };
        // Also kill descendants when their leader exits early. Do this before
        // joining readers, so inherited pipe handles cannot keep us blocked.
        drop(child);
        let stdout = out
            .join()
            .map_err(|_| anyhow::anyhow!("stdout reader failed"))??;
        let _stderr = err
            .join()
            .map_err(|_| anyhow::anyhow!("stderr reader failed"))??;
        if exceeded.load(Ordering::SeqCst) {
            return Err(ProcessLimit::Output.into());
        }
        Ok((
            status?.code(),
            String::from_utf8(stdout).context("backend stdout was not UTF-8")?,
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Write, path::PathBuf};

    fn child(mode: &str, marker: &std::path::Path) -> Result<Command> {
        let mut command = Command::new(std::env::current_exe()?);
        command
            .args([
                "--ignored",
                "--exact",
                "process::tests::child_fixture",
                "--nocapture",
            ])
            .env("PEERVAULT_CHILD_TEST", mode)
            .env("PEERVAULT_TEST_MARKER", marker);
        Ok(command)
    }

    #[test]
    fn overflow_timeout_and_cancellation_reap_descendants() -> Result<()> {
        let temp = tempfile::tempdir()?;
        for mode in [
            "stdout-flood",
            "stderr-flood",
            "tree",
            "cancel-tree",
            "exit-tree",
        ] {
            let marker = temp.path().join(mode);
            let cancellation = AtomicBool::new(false);
            let mut command = child(mode, &marker)?;
            let result = thread::scope(|scope| {
                if mode == "cancel-tree" {
                    let cancellation = &cancellation;
                    scope.spawn(move || {
                        thread::sleep(Duration::from_millis(200));
                        cancellation.store(true, Ordering::SeqCst);
                    });
                }
                bounded_with_cancel(
                    &mut command,
                    Duration::from_millis(500),
                    1024,
                    &cancellation,
                )
            });
            if mode == "exit-tree" {
                assert!(result.is_ok());
            } else {
                let error = result.unwrap_err();
                assert!(error.downcast_ref::<ProcessLimit>().is_some());
                assert!(!format!("{error:#}").contains("secret-from-child"));
            }
            thread::sleep(Duration::from_millis(800));
            assert!(!marker.exists(), "descendant survived {mode}");
        }
        Ok(())
    }

    #[test]
    #[ignore = "subprocess fixture, invoked by process-boundary tests"]
    #[allow(
        clippy::zombie_processes,
        reason = "exit-tree deliberately orphans a child to test process-tree ownership"
    )]
    fn child_fixture() {
        let mode = std::env::var("PEERVAULT_CHILD_TEST").unwrap();
        let marker = PathBuf::from(std::env::var_os("PEERVAULT_TEST_MARKER").unwrap());
        match mode.as_str() {
            "stdout-flood" => {
                std::io::stdout().write_all(&[b'x'; 8192]).unwrap();
            }
            "stderr-flood" => {
                eprintln!("secret-from-child{}", "x".repeat(8192));
            }
            "descendant" => {
                thread::sleep(Duration::from_millis(700));
                fs::write(marker, b"escaped").unwrap();
            }
            "tree" | "cancel-tree" | "exit-tree" => {
                let mut descendant = child("descendant", &marker).unwrap().spawn().unwrap();
                if mode != "exit-tree" {
                    descendant.wait().unwrap();
                }
            }
            _ => panic!("unknown fixture mode"),
        }
    }
}
