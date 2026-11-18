//! Versioned owner-only local IPC.
//!
//! Windows uses a named pipe; Linux uses a Unix-domain socket. The server
//! accepts one request line at a time, checks that the caller is the same OS
//! user as the server, and passes the JSON request to the handler. No TCP port
//! is opened and the server never reads a password value.

use anyhow::{Result, ensure};
use std::io::{Read, Write};

pub const MAX_REQUEST_BYTES: usize = 64 * 1024;

pub fn serve(endpoint: &str, handler: impl Fn(&[u8]) -> Vec<u8>) -> Result<()> {
    ensure!(!endpoint.is_empty(), "IPC endpoint must not be empty");
    platform::serve(endpoint, handler)
}

fn read_request(reader: &mut impl Read) -> Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        if reader.read(&mut byte)? == 0 {
            return Ok(None);
        }
        if byte[0] == b'\n' {
            return Ok(Some(bytes));
        }
        bytes.push(byte[0]);
        ensure!(
            bytes.len() <= MAX_REQUEST_BYTES,
            "IPC request exceeds the bounded size"
        );
    }
}

fn respond(
    request: &[u8],
    writer: &mut impl Write,
    handler: &impl Fn(&[u8]) -> Vec<u8>,
) -> Result<()> {
    let response = handler(request);
    ensure!(
        response.len() <= MAX_REQUEST_BYTES,
        "IPC response exceeds the bounded size"
    );
    writer.write_all(&response)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

#[cfg(windows)]
mod platform {
    use super::*;
    use anyhow::bail;
    use std::{
        ffi::{OsStr, c_void},
        fs::File,
        os::windows::ffi::OsStrExt,
        os::windows::io::FromRawHandle,
        ptr::null_mut,
    };
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, ERROR_PIPE_CONNECTED, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
        },
        Security::{
            CopySid, EqualSid, GetLengthSid, GetTokenInformation, TOKEN_QUERY, TOKEN_USER,
            TokenUser,
        },
        Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX},
        System::{
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId,
                PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
            },
            Threading::{
                GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };

    fn wide(value: &str) -> Vec<u16> {
        OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    fn token_user_sid(token: HANDLE) -> Result<Vec<u8>> {
        let mut length = 0u32;
        unsafe {
            GetTokenInformation(token, TokenUser, null_mut(), 0, &mut length);
        }
        ensure!(length > 0, "cannot size the caller token user");
        let mut buffer = vec![0u8; length as usize];
        let ok = unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                length,
                &mut length,
            )
        };
        ensure!(ok != 0, "cannot read the caller token user");
        let user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
        let source = user.User.Sid;
        let sid_length = unsafe { GetLengthSid(source) };
        ensure!(sid_length > 0, "caller SID is invalid");
        let mut sid = vec![0u8; sid_length as usize];
        let copied = unsafe { CopySid(sid_length, sid.as_mut_ptr().cast(), source) };
        ensure!(copied != 0, "cannot copy the caller SID");
        Ok(sid)
    }

    fn current_user_sid() -> Result<Vec<u8>> {
        let mut token: HANDLE = null_mut();
        let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) };
        ensure!(opened != 0, "cannot open the server process token");
        let sid = token_user_sid(token);
        unsafe {
            CloseHandle(token);
        }
        sid
    }

    fn client_is_owner(pipe: HANDLE, server_sid: &[u8]) -> bool {
        let mut pid = 0u32;
        if unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) } == 0 {
            return false;
        }
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return false;
        }
        let mut token: HANDLE = null_mut();
        let opened = unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) };
        if opened == 0 {
            unsafe {
                CloseHandle(process);
            }
            return false;
        }
        let client_sid = token_user_sid(token);
        unsafe {
            CloseHandle(token);
            CloseHandle(process);
        }
        client_sid.is_ok_and(|sid| unsafe {
            EqualSid(
                server_sid.as_ptr() as *mut c_void,
                sid.as_ptr() as *mut c_void,
            ) != 0
        })
    }

    pub fn serve(endpoint: &str, handler: impl Fn(&[u8]) -> Vec<u8>) -> Result<()> {
        ensure!(
            endpoint.len() <= 128
                && endpoint
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')),
            "named-pipe endpoint must be 1..=128 safe ASCII characters"
        );
        let name = wide(&format!(r"\\.\pipe\{endpoint}"));
        let server_sid = current_user_sid()?;
        loop {
            let pipe = unsafe {
                CreateNamedPipeW(
                    name.as_ptr(),
                    PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                    1,
                    1024 * 1024,
                    1024 * 1024,
                    0,
                    null_mut(),
                )
            };
            if pipe == INVALID_HANDLE_VALUE {
                bail!("cannot create the owner-only named pipe");
            }
            let connected = unsafe { ConnectNamedPipe(pipe, null_mut()) };
            if connected == 0 {
                let error = unsafe { GetLastError() };
                if error != ERROR_PIPE_CONNECTED {
                    unsafe {
                        CloseHandle(pipe);
                    }
                    bail!("named-pipe connect failed");
                }
            }
            let mut file = unsafe { File::from_raw_handle(pipe) };
            if !client_is_owner(pipe, &server_sid) {
                continue;
            }
            let _ = (|| -> Result<()> {
                if let Some(request) = read_request(&mut file)? {
                    respond(&request, &mut file, &handler)?;
                }
                Ok(())
            })();
            if bailiff::process::cancelled() {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
mod platform {
    use super::*;
    use anyhow::Context;
    use fs2::FileExt;
    use std::{
        fs::{self, OpenOptions},
        os::unix::{
            fs::{FileTypeExt, MetadataExt, PermissionsExt},
            io::AsRawFd,
            net::{UnixListener, UnixStream},
        },
        path::{Path, PathBuf},
        thread,
        time::Duration,
    };

    struct SocketGuard(PathBuf);

    impl Drop for SocketGuard {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[cfg(target_os = "linux")]
    fn peer_uid(stream: &UnixStream) -> Option<u32> {
        let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
        let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credentials as *mut libc::ucred).cast(),
                &mut length,
            )
        };
        (result == 0).then_some(credentials.uid)
    }

    #[cfg(not(target_os = "linux"))]
    fn peer_uid(_stream: &UnixStream) -> Option<u32> {
        None
    }

    pub fn serve(endpoint: &str, handler: impl Fn(&[u8]) -> Vec<u8>) -> Result<()> {
        let path = Path::new(endpoint);
        ensure!(path.is_absolute(), "Unix IPC endpoint must be absolute");
        let parent = path.parent().context("IPC endpoint has no parent")?;
        ensure!(parent.is_dir(), "IPC endpoint parent does not exist");
        #[cfg(target_os = "linux")]
        {
            let mode = fs::metadata(parent)?.permissions().mode();
            ensure!(
                mode & 0o022 == 0,
                "IPC endpoint directory must not be group/other writable"
            );
        }
        let lock_path = path.with_extension("lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .with_context(|| format!("cannot open IPC lock {}", lock_path.display()))?;
        lock.try_lock_exclusive()
            .with_context(|| format!("another agent owns {}", lock_path.display()))?;
        if path.exists() {
            ensure!(
                fs::symlink_metadata(path)?.file_type().is_socket(),
                "IPC endpoint exists and is not a socket"
            );
            fs::remove_file(path).context("cannot remove a stale IPC socket")?;
        }
        let listener = UnixListener::bind(path).context("cannot bind the owner-only IPC socket")?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        listener
            .set_nonblocking(true)
            .context("cannot poll the IPC socket")?;
        let _guard = SocketGuard(path.to_owned());
        let server_uid = fs::metadata(path)?.uid();
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    if peer_uid(&stream) != Some(server_uid) {
                        continue;
                    }
                    let _ = (|| -> Result<()> {
                        let mut reader = &stream;
                        if let Some(request) = read_request(&mut reader)? {
                            respond(&request, &mut &stream, &handler)?;
                        }
                        Ok(())
                    })();
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if bailiff::process::cancelled() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(error) => return Err(error).context("IPC accept failed"),
            }
        }
        Ok(())
    }
}
