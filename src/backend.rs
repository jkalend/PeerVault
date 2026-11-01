use crate::{
    fixture::{self, Manifest},
    media, pins,
    process::{ProcessLimit, bounded},
};
use anyhow::{Context, Result, ensure};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const OUTPUT_LIMIT: u64 = 4 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Copy, Debug, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Restic,
    Kopia,
}

#[derive(Serialize)]
struct Step {
    name: String,
    command_ref: String,
    elapsed_ms: u128,
    exit_code: Option<i32>,
    expectation_met: bool,
    outcome: &'static str,
    reason_code: Option<&'static str>,
}

#[derive(Serialize)]
struct Fault {
    case_id: &'static str,
    object_ref: String,
    original_sha256: String,
    operation: &'static str,
    offset_or_length: u64,
}

#[derive(Clone, Serialize)]
struct SnapshotMapping {
    source_id: String,
    copy_id: String,
}

#[derive(Serialize)]
struct InventorySummary {
    files: usize,
    bytes: u64,
    data_files: usize,
    data_bytes: u64,
    digest: String,
    missing: usize,
    changed: usize,
    added: usize,
    ephemeral_changes: usize,
}

#[derive(Serialize)]
struct TopologyEvidence {
    media: media::MediaSet,
    transport: &'static str,
    concurrent_writers: bool,
    copy_command_ref: &'static str,
    copy_consistency: &'static str,
    key_role: &'static str,
    delete_behavior: &'static str,
    source_absence: &'static str,
    snapshot_mapping: Vec<SnapshotMapping>,
    inventory_before: Option<InventorySummary>,
    inventory_after: Option<InventorySummary>,
}

#[derive(Clone)]
struct Session {
    root: PathBuf,
    config: PathBuf,
    cache: PathBuf,
}

impl Session {
    fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            config: root.join("client.config"),
            cache: root.join("cache"),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    schema_version: u32,
    profile_id: String,
    revision: u32,
    classification: String,
    os: String,
    architecture: String,
    environment: std::collections::BTreeMap<String, String>,
    missing_fields: Vec<String>,
}

#[derive(Serialize)]
struct Evidence {
    schema_version: u32,
    harness_version: &'static str,
    harness_sha256: String,
    scope: &'static str,
    result: &'static str,
    gate_status: &'static str,
    run_id: String,
    active_phase: String,
    failure_reason: Option<&'static str>,
    profile: Option<Profile>,
    profile_sha256: Option<String>,
    backend_lock_sha256: String,
    source_inputs: std::collections::BTreeMap<&'static str, String>,
    untested_cases: Vec<&'static str>,
    backend: Kind,
    backend_version: String,
    executable_sha256: String,
    os: &'static str,
    arch: &'static str,
    started_unix_seconds: u64,
    clock_quality: &'static str,
    verification_scope: &'static str,
    command_timeout_seconds: u64,
    output_limit_bytes_per_stream: u64,
    fixture_manifest_sha256: Vec<String>,
    snapshots: Vec<String>,
    repository_bytes: Vec<u64>,
    restored_files: Vec<usize>,
    steps: Vec<Step>,
    faults: Vec<Fault>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topology: Option<TopologyEvidence>,
}

struct Runner {
    kind: Kind,
    executable: PathBuf,
    root: PathBuf,
    session: Session,
    source: PathBuf,
    repository: PathBuf,
    password: String,
    evidence: Evidence,
    pending_mutation: bool,
}

fn base_evidence(kind: Kind, scope: &'static str) -> Evidence {
    Evidence {
        schema_version: 3,
        harness_version: env!("CARGO_PKG_VERSION"),
        harness_sha256: String::new(),
        scope,
        result: "fail",
        gate_status: "unknown",
        run_id: format!(
            "r0-{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            std::process::id()
        ),
        active_phase: "setup".into(),
        failure_reason: None,
        profile: None,
        profile_sha256: None,
        backend_lock_sha256: pins::digest(),
        source_inputs: pins::source_inputs(),
        untested_cases: vec![
            "scored Windows11/Ubuntu24 profiles",
            "independent copy/key recovery",
            "hostile restore containment",
            "phase-targeted crash and power loss",
            "index recovery, disk full and denied access",
            "Kopia clock/storage stress",
        ],
        backend: kind,
        backend_version: String::new(),
        executable_sha256: String::new(),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        started_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        clock_quality: "unmeasured; wall clock is informational only",
        verification_scope: match kind {
            Kind::Restic => "repository",
            Kind::Kopia => "snapshot",
        },
        command_timeout_seconds: TIMEOUT.as_secs(),
        output_limit_bytes_per_stream: OUTPUT_LIMIT,
        fixture_manifest_sha256: vec![],
        snapshots: vec![],
        repository_bytes: vec![],
        restored_files: vec![],
        steps: vec![],
        faults: vec![],
        topology: None,
    }
}

fn setup(runner: &mut Runner, profile: Option<&Path>) -> Result<()> {
    runner.phase("profile");
    if let Some(profile) = profile {
        let mut bytes = Vec::new();
        File::open(profile)?.take(65537).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 65536, "profile too large");
        let selected: Profile = serde_json::from_slice(&bytes)?;
        ensure!(
            selected.schema_version == 1
                && selected.classification == "smoke"
                && selected.os == std::env::consts::OS
                && selected.architecture == std::env::consts::ARCH
                && !selected.profile_id.is_empty()
                && selected.revision > 0,
            "profile version/platform/classification mismatch"
        );
        runner.evidence.profile_sha256 = Some(pins::hash_bytes(&bytes));
        runner.evidence.profile = Some(selected);
    }
    runner.phase("binary-pin");
    let pins = pins::load()?;
    let pin = match runner.kind {
        Kind::Restic => pins.restic,
        Kind::Kopia => pins.kopia,
    };
    runner.evidence.backend_version = pin.version;
    let asset = match std::env::consts::OS {
        "windows" => pin.windows,
        "linux" => pin.linux,
        _ => anyhow::bail!("unsupported backend platform"),
    };
    ensure!(
        std::env::consts::ARCH == "x86_64",
        "unsupported backend architecture"
    );
    let staged = runner.root.join(if cfg!(windows) {
        "approved-backend.exe"
    } else {
        "approved-backend"
    });
    let source = File::open(&runner.executable).context("cannot open configured backend")?;
    ensure!(
        source.metadata()?.is_file() && source.metadata()?.len() <= 128 * fixture::MIB,
        "invalid/oversized backend executable"
    );
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)?;
    let copied = std::io::copy(&mut source.take(128 * fixture::MIB + 1), &mut output)?;
    ensure!(
        copied <= 128 * fixture::MIB,
        "backend executable grew beyond size limit"
    );
    output.sync_all()?;
    drop(output);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o700))?;
    }
    runner.evidence.executable_sha256 = fixture::hash_file(&staged)?.1;
    ensure!(
        runner.evidence.executable_sha256 == asset.executable_sha256,
        "unapproved backend executable hash; no execution permitted"
    );
    runner.executable = staged;
    Ok(())
}

fn finish(
    mut runner: Runner,
    result: Result<()>,
    root: &Path,
    owned_root: bool,
    success_phase: &str,
) -> Result<()> {
    if result.is_ok() {
        runner.evidence.result = "pass";
        runner.evidence.gate_status = "partial";
        runner.evidence.active_phase = success_phase.into();
    } else {
        runner.evidence.gate_status = if runner.pending_mutation {
            "unknown"
        } else {
            "fail"
        };
        runner.evidence.failure_reason = Some(if crate::process::cancelled() {
            "cancelled"
        } else {
            "baseline-failure"
        });
        runner.evidence.steps.push(Step {
            name: runner.evidence.active_phase.clone(),
            command_ref: format!("r0/{}", runner.evidence.active_phase),
            elapsed_ms: 0,
            exit_code: None,
            expectation_met: false,
            outcome: if runner.pending_mutation {
                "unknown"
            } else {
                "fail"
            },
            reason_code: runner.evidence.failure_reason,
        });
    }
    let saved = if owned_root {
        File::create(root.join("evidence.json")).and_then(|mut report| {
            serde_json::to_writer_pretty(&mut report, &runner.evidence)?;
            report.write_all(b"\n")?;
            report.sync_all()
        })
    } else {
        Err(std::io::Error::other("no owned work directory"))
    };
    if saved.is_err() {
        println!("{}", serde_json::to_string(&runner.evidence)?);
    }
    if result.is_err() {
        anyhow::bail!(
            "R0 {} failed; see diagnostic evidence (backend output withheld)",
            runner.evidence.active_phase
        );
    }
    saved.context("cannot retain evidence")
}

pub fn bakeoff(kind: Kind, executable: &Path, root: &Path, profile: Option<&Path>) -> Result<()> {
    let mut random = [0u8; 32];
    let random_result = getrandom::fill(&mut random);
    let password: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let mut runner = Runner {
        kind,
        executable: executable.to_path_buf(),
        root: root.to_path_buf(),
        session: Session::new(root),
        source: root.join("source"),
        repository: root.join("repository"),
        password,
        pending_mutation: false,
        evidence: base_evidence(
            kind,
            "R0 synthetic local baseline; not full R0 qualification",
        ),
    };
    let mut owned_root = false;
    let result = (|| {
        fs::create_dir(root)
            .context("work directory must be new; existing data is never reused")?;
        owned_root = true;
        let canonical = backend_path(fs::canonicalize(root)?)?;
        runner.source = canonical.join("source");
        runner.repository = canonical.join("repository");
        runner.session = Session::new(&canonical);
        runner.root = canonical;
        ensure!(random_result.is_ok(), "OS random source unavailable");
        runner.evidence.harness_sha256 = fixture::hash_file(&std::env::current_exe()?)?.1;
        setup(&mut runner, profile)?;
        runner.run()
    })();
    finish(runner, result, root, owned_root, "baseline-complete")
}

pub fn copy_drill(
    kind: Kind,
    executable: &Path,
    work: &Path,
    copy: &Path,
    recovery: &Path,
    profile: Option<&Path>,
    acknowledge_unknown_media: bool,
) -> Result<()> {
    let mut random = [0u8; 32];
    let random_result = getrandom::fill(&mut random);
    let password: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let mut runner = Runner {
        kind,
        executable: executable.to_path_buf(),
        root: work.to_path_buf(),
        session: Session::new(work),
        source: work.join("source"),
        repository: work.join("repository-source"),
        password,
        pending_mutation: false,
        evidence: base_evidence(
            kind,
            "R0 synthetic copy/recovery topology baseline; not full R0 qualification",
        ),
    };
    runner.evidence.untested_cases = vec![
        "scored Windows11/Ubuntu24 profiles",
        "keyless peer topology",
        "network transport and authenticated pairing",
        "clean-device/offline recovery and separate secret custody",
        "hostile restore containment",
        "phase-targeted crash and power loss",
        "index recovery, disk full and denied access",
        "Kopia clock/storage stress",
        "Ubuntu24/ext4 copy/recovery drill",
    ];
    let mut owned_root = false;
    let result = (|| {
        fs::create_dir(work)
            .context("work directory must be new; existing data is never reused")?;
        owned_root = true;
        fs::create_dir(copy).context("copy directory must be new")?;
        fs::create_dir(recovery).context("recovery directory must be new")?;
        let work = backend_path(fs::canonicalize(work)?)?;
        let copy = backend_path(fs::canonicalize(copy)?)?;
        let recovery = backend_path(fs::canonicalize(recovery)?)?;
        ensure!(
            copy != work
                && !copy.starts_with(&work)
                && !work.starts_with(&copy)
                && recovery != work
                && !recovery.starts_with(&work)
                && !work.starts_with(&recovery)
                && recovery != copy
                && !recovery.starts_with(&copy)
                && !copy.starts_with(&recovery),
            "source, repository-copy and recovery roots must be disjoint"
        );
        runner.root = work.clone();
        runner.source = work.join("source");
        runner.repository = work.join("repository-source");
        runner.session = Session::new(&work);
        ensure!(random_result.is_ok(), "OS random source unavailable");
        runner.evidence.harness_sha256 = fixture::hash_file(&std::env::current_exe()?)?.1;
        setup(&mut runner, profile)?;
        runner.drill(&copy, &recovery, acknowledge_unknown_media)
    })();
    finish(runner, result, work, owned_root, "copy-drill-complete")
}

impl Runner {
    fn phase(&mut self, name: &str) {
        self.evidence.active_phase = name.into();
    }
    fn command(
        &self,
        session: &Session,
        repository: &Path,
        args: &[&str],
        wrong_password: bool,
    ) -> Command {
        let mut command = Command::new(&self.executable);
        // No inherited backend credentials, provider settings, hooks or user config.
        command.env_clear();
        for name in ["PATH", "SystemRoot", "WINDIR", "TMP", "TEMP", "TMPDIR"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .current_dir(&session.root)
            .stdin(Stdio::null())
            .env("HOME", &session.root)
            .env("USERPROFILE", &session.root)
            .env("XDG_CACHE_HOME", session.root.join("xdg-cache"))
            .env("XDG_CONFIG_HOME", session.root.join("xdg-config"))
            .env("APPDATA", session.root.join("appdata"))
            .env("LOCALAPPDATA", session.root.join("localappdata"))
            .env("USER", "peervault-fixture")
            .env("USERNAME", "peervault-fixture")
            .env("LANG", "C.UTF-8");
        let password = if wrong_password {
            "deliberately-wrong-r0-password"
        } else {
            &self.password
        };
        match self.kind {
            Kind::Restic => {
                command
                    .arg("--repo")
                    .arg(repository)
                    .arg("--no-cache")
                    .env("RESTIC_PASSWORD", password);
            }
            Kind::Kopia => {
                command
                    .arg("--config-file")
                    .arg(&session.config)
                    .args([
                        "--no-persist-credentials",
                        "--disable-file-logging",
                        "--disable-content-log",
                        "--no-progress",
                    ])
                    .env("KOPIA_PASSWORD", password)
                    .env("KOPIA_CHECK_FOR_UPDATES", "false")
                    .env("KOPIA_CACHE_DIRECTORY", &session.cache);
                #[cfg(windows)]
                command.arg("--no-use-credential-manager");
                #[cfg(unix)]
                command.arg("--no-use-keyring");
            }
        }
        command.args(args);
        command
    }

    fn execute_in(
        &mut self,
        session: &Session,
        repository: &Path,
        name: &str,
        args: &[&str],
        success: bool,
        wrong_password: bool,
    ) -> Result<String> {
        let mutation = args.first() == Some(&"backup")
            || args.starts_with(&["snapshot", "create"])
            || args.first() == Some(&"init")
            || args.starts_with(&["repository", "create"])
            || args.starts_with(&["repository", "sync-to"]);
        self.execute_flagged(
            session,
            repository,
            name,
            args,
            success,
            wrong_password,
            mutation,
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "explicit session, repository, command, outcome and mutation flags"
    )]
    fn execute_flagged(
        &mut self,
        session: &Session,
        repository: &Path,
        name: &str,
        args: &[&str],
        success: bool,
        wrong_password: bool,
        mutation: bool,
    ) -> Result<String> {
        self.phase(name);
        eprintln!("R0: {name}");
        let mut command = self.command(session, repository, args, wrong_password);
        if args.first() == Some(&"backup") {
            command.current_dir(&self.source);
        }
        if mutation {
            self.pending_mutation = true;
        }
        let start = Instant::now();
        let result = bounded(&mut command, TIMEOUT, OUTPUT_LIMIT);
        if let Err(error) = &result {
            let reason = match error.downcast_ref::<ProcessLimit>() {
                Some(ProcessLimit::Timeout) => "timeout",
                Some(ProcessLimit::Output) => "output-limit",
                Some(ProcessLimit::Cancelled) => "cancelled",
                None => "process-failure",
            };
            self.evidence.steps.push(Step {
                name: name.into(),
                command_ref: format!("r0/{name}"),
                elapsed_ms: start.elapsed().as_millis(),
                exit_code: None,
                expectation_met: false,
                outcome: if mutation { "unknown" } else { "fail" },
                reason_code: Some(reason),
            });
        }
        let (code, stdout) = result?;
        let met = code.is_some() && ((code == Some(0)) == success);
        self.evidence.steps.push(Step {
            name: name.into(),
            command_ref: format!("r0/{name}"),
            elapsed_ms: start.elapsed().as_millis(),
            exit_code: code,
            expectation_met: met,
            outcome: if met {
                "pass"
            } else if mutation {
                "unknown"
            } else {
                "fail"
            },
            reason_code: if met { None } else { Some("backend-exit") },
        });
        ensure!(
            met,
            "{name}: unexpected backend exit {code:?}; raw output withheld"
        );
        Ok(stdout)
    }

    fn execute(
        &mut self,
        name: &str,
        args: &[&str],
        success: bool,
        wrong_password: bool,
    ) -> Result<String> {
        let session = self.session.clone();
        let repository = self.repository.clone();
        self.execute_in(&session, &repository, name, args, success, wrong_password)
    }

    fn run(&mut self) -> Result<()> {
        let version_args = match self.kind {
            Kind::Restic => vec!["version"],
            Kind::Kopia => vec!["--version"],
        };
        let version = self.execute("version-pin", &version_args, true, false)?;
        let actual = match self.kind {
            Kind::Restic => version.split_whitespace().nth(1),
            Kind::Kopia => version.split_whitespace().next(),
        };
        ensure!(
            actual == Some(self.evidence.backend_version.as_str()),
            "unsupported backend version"
        );
        self.phase("fixture-a");
        let source = self.source.clone();
        let first = fixture::generate(&source, false)?;
        self.oracle(1, &first)?;
        match self.kind {
            Kind::Restic => {
                self.execute("init", &["init", "--repository-version=2"], true, false)?;
            }
            Kind::Kopia => {
                let repo = self.repository.clone();
                self.execute(
                    "init",
                    &[
                        "repository",
                        "create",
                        "filesystem",
                        "--path",
                        text_path(&repo)?,
                        "--no-check-for-updates",
                        "--no-enable-actions",
                    ],
                    true,
                    false,
                )?;
            }
        }
        let snapshot_a = self.snapshot("snapshot-a")?;
        self.evidence.snapshots.push(snapshot_a.clone());
        self.size()?;
        self.snapshot("snapshot-unchanged")?;
        self.size()?;
        self.phase("fixture-b");
        let second = fixture::mutate(&source)?;
        self.oracle(2, &second)?;
        let snapshot_b = self.snapshot("snapshot-b")?;
        ensure!(
            snapshot_a != snapshot_b,
            "changed corpus must produce a distinct snapshot"
        );
        self.evidence.snapshots.push(snapshot_b.clone());
        self.size()?;
        let history = self.list()?;
        for id in [&snapshot_a, &snapshot_b] {
            ensure!(
                history.contains(id),
                "committed snapshot missing from history"
            );
            self.verify(id, true)?;
        }
        self.restore(&snapshot_a, "restore-a", &first)?;
        self.restore(&snapshot_b, "restore-b", &second)?;
        self.wrong_password()?;
        self.interrupt_snapshot()?;
        let history = self.list()?;
        ensure!(
            history.contains(&snapshot_a) && history.contains(&snapshot_b),
            "interruption lost prior snapshot"
        );
        self.verify(&snapshot_a, true)?;
        self.restore(&snapshot_a, "restore-after-interrupt", &first)?;
        self.reconnect()?;
        self.verify(&snapshot_b, true)?;
        self.restore(&snapshot_b, "restore-fresh-client", &second)?;
        self.damage(&snapshot_a)?;
        self.restore(&snapshot_a, "restore-after-repair", &first)?;
        Ok(())
    }

    fn oracle(&mut self, revision: usize, manifest: &Manifest) -> Result<()> {
        let path = self.root.join(format!("expected-{revision}.json"));
        fixture::write_manifest(&path, manifest)?;
        let golden = match revision {
            1 => include_str!("../tests/fixtures/v1-a.json"),
            _ => include_str!("../tests/fixtures/v1-b.json"),
        };
        let expected: Manifest =
            serde_json::from_str(golden).context("invalid checked-in golden manifest")?;
        ensure!(
            manifest == &expected,
            "generated fixture differs from checked-in golden oracle"
        );
        self.evidence
            .fixture_manifest_sha256
            .push(fixture::hash_file(&path)?.1);
        Ok(())
    }

    fn snapshot_args(&self) -> Vec<String> {
        match self.kind {
            Kind::Restic => vec![
                "backup".into(),
                "--json".into(),
                "--host=peervault-fixture".into(),
                ".".into(),
            ],
            Kind::Kopia => vec![
                "snapshot".into(),
                "create".into(),
                "--json".into(),
                "--fail-fast".into(),
                "--force-hash=100".into(),
                self.source.to_string_lossy().into_owned(),
            ],
        }
    }

    fn snapshot(&mut self, name: &str) -> Result<String> {
        let args = self.snapshot_args();
        let output = self.execute(
            name,
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
            true,
            false,
        )?;
        let id = parse_snapshot(self.kind, &output)?;
        self.pending_mutation = false;
        Ok(id)
    }

    fn list(&mut self) -> Result<String> {
        let session = self.session.clone();
        let repository = self.repository.clone();
        self.list_in(&session, &repository, "history")
    }

    fn list_in(&mut self, session: &Session, repository: &Path, name: &str) -> Result<String> {
        let args = match self.kind {
            Kind::Restic => vec!["snapshots", "--json"],
            Kind::Kopia => vec!["snapshot", "list", "--json", "--all"],
        };
        self.execute_in(session, repository, name, &args, true, false)
    }

    fn verify(&mut self, id: &str, success: bool) -> Result<()> {
        let session = self.session.clone();
        let repository = self.repository.clone();
        self.verify_in(&session, &repository, id, success)
    }

    fn verify_in(
        &mut self,
        session: &Session,
        repository: &Path,
        id: &str,
        success: bool,
    ) -> Result<()> {
        match self.kind {
            Kind::Restic => {
                self.execute_in(
                    session,
                    repository,
                    "repository-full-read",
                    &["check", "--read-data"],
                    success,
                    false,
                )?;
            }
            Kind::Kopia => {
                self.execute_in(
                    session,
                    repository,
                    "snapshot-full-read",
                    &["snapshot", "verify", "--verify-files-percent=100", id],
                    success,
                    false,
                )?;
            }
        }
        Ok(())
    }

    fn restore(&mut self, id: &str, name: &str, manifest: &Manifest) -> Result<()> {
        let session = self.session.clone();
        let repository = self.repository.clone();
        let root = self.root.clone();
        self.restore_in(&session, &repository, id, name, manifest, &root)
    }

    fn restore_in(
        &mut self,
        session: &Session,
        repository: &Path,
        id: &str,
        name: &str,
        manifest: &Manifest,
        target_root: &Path,
    ) -> Result<()> {
        let destination = target_root.join(name);
        ensure!(!destination.try_exists()?, "restore target must be new");
        let canary = target_root.join("outside-canary");
        fs::write(&canary, b"outside restore root")?;
        // Only self-generated fixtures enter backend restore. Arbitrary repositories are not accepted.
        match self.kind {
            Kind::Restic => {
                self.execute_in(
                    session,
                    repository,
                    name,
                    &[
                        "restore",
                        id,
                        "--target",
                        text_path(&destination)?,
                        "--verify",
                    ],
                    true,
                    false,
                )?;
            }
            Kind::Kopia => {
                self.execute_in(
                    session,
                    repository,
                    name,
                    &[
                        "snapshot",
                        "restore",
                        id,
                        text_path(&destination)?,
                        "--no-overwrite-files",
                        "--no-overwrite-directories",
                        "--no-overwrite-symlinks",
                    ],
                    true,
                    false,
                )?;
            }
        }
        self.phase(&format!("{name}-independent-hashes"));
        fixture::compare(&destination, manifest)?;
        ensure!(
            fs::read(canary)? == b"outside restore root",
            "outside canary changed"
        );
        self.evidence.restored_files.push(manifest.files.len());
        Ok(())
    }

    fn size(&mut self) -> Result<()> {
        fn total(path: &Path) -> Result<u64> {
            let mut size = 0;
            for item in fs::read_dir(path)? {
                let item = item?;
                let meta = item.metadata()?;
                size += if meta.is_dir() {
                    total(&item.path())?
                } else {
                    meta.len()
                };
            }
            Ok(size)
        }
        self.evidence
            .repository_bytes
            .push(total(&self.repository)?);
        Ok(())
    }

    fn wrong_password(&mut self) -> Result<()> {
        let mut session = self.session.clone();
        let repository = self.repository.clone();
        match self.kind {
            Kind::Restic => {
                self.execute_in(
                    &session,
                    &repository,
                    "wrong-password",
                    &["snapshots", "--json"],
                    false,
                    true,
                )?;
            }
            Kind::Kopia => {
                session.config = self.root.join("wrong-password.config");
                self.execute_in(
                    &session,
                    &repository,
                    "wrong-password",
                    &[
                        "repository",
                        "connect",
                        "filesystem",
                        "--path",
                        text_path(&repository)?,
                    ],
                    false,
                    true,
                )?;
            }
        }
        Ok(())
    }

    fn reconnect(&mut self) -> Result<()> {
        if matches!(self.kind, Kind::Kopia) {
            let suffix = self.evidence.steps.len();
            self.session.config = self.root.join(format!("fresh-{suffix}.config"));
            self.session.cache = self.root.join(format!("fresh-cache-{suffix}"));
            let repo = self.repository.clone();
            self.execute(
                "fresh-client-connect",
                &[
                    "repository",
                    "connect",
                    "filesystem",
                    "--path",
                    text_path(&repo)?,
                    "--no-check-for-updates",
                    "--no-enable-actions",
                ],
                true,
                false,
            )?;
        }
        Ok(())
    }

    fn interrupt_snapshot(&mut self) -> Result<()> {
        self.phase("timed-process-interruption");
        self.pending_mutation = true;
        eprintln!("R0: timed-process-interruption (not phase-targeted crash qualification)");
        let args = self.snapshot_args();
        let session = self.session.clone();
        let repository = self.repository.clone();
        let mut command = self.command(
            &session,
            &repository,
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
            false,
        );
        command.current_dir(&self.source);
        // This is deliberately a startup interruption, not evidence of every commit phase.
        let result = bounded(&mut command, Duration::from_millis(100), OUTPUT_LIMIT);
        ensure!(
            result
                .as_ref()
                .err()
                .and_then(|error| error.downcast_ref::<ProcessLimit>())
                == Some(&ProcessLimit::Timeout),
            "backend exited before interruption; drill inconclusive"
        );
        if matches!(self.kind, Kind::Restic) {
            self.execute(
                "unlock-owned-disposable-repository",
                &["unlock"],
                true,
                false,
            )?;
        }
        self.evidence.steps.push(Step {
            name: "timed-process-interruption".into(),
            command_ref: "r0/timed-process-interruption".into(),
            elapsed_ms: 100,
            exit_code: None,
            expectation_met: true,
            outcome: "unknown",
            reason_code: Some("outcome_unknown"),
        });
        Ok(())
    }

    fn damage(&mut self, id: &str) -> Result<()> {
        self.phase("select-corruption-target");
        fn find(path: &Path, prefix: &str, candidates: &mut Vec<(u64, PathBuf)>) -> Result<()> {
            for item in fs::read_dir(path)? {
                let item = item?;
                let metadata = item.metadata()?;
                if metadata.is_dir() {
                    find(&item.path(), prefix, candidates)?;
                } else if prefix.is_empty()
                    || item.file_name().to_string_lossy().starts_with(prefix)
                {
                    candidates.push((metadata.len(), item.path()));
                }
            }
            Ok(())
        }
        let mut candidates = vec![];
        let repository = self.repository.clone();
        let (path, prefix) = match self.kind {
            Kind::Restic => (repository.join("data"), ""),
            Kind::Kopia => (repository.join("p"), ""),
        };
        find(&path, prefix, &mut candidates)?;
        candidates.sort();
        let (length, object) = candidates
            .pop()
            .context("no backend data object available for fault drill")?;
        ensure!(length > 1024, "data object too small for fault drill");
        let saved = self.root.join("fault-object-original");
        fs::copy(&object, &saved)?;
        let original_sha256 = fixture::hash_file(&saved)?.1;
        let object_ref = object
            .strip_prefix(&self.repository)?
            .to_string_lossy()
            .replace('\\', "/");
        // restic deliberately writes immutable (0444) pack files. Fault injection
        // changes only this runner-owned disposable copy, never a supplied repo.
        for fault in ["truncation", "bit-flip"] {
            self.phase(fault);
            self.evidence.faults.push(Fault {
                case_id: fault,
                object_ref: object_ref.clone(),
                original_sha256: original_sha256.clone(),
                operation: if fault == "truncation" {
                    "truncate-to"
                } else {
                    "xor-0x80-at"
                },
                offset_or_length: length / 2,
            });
            make_writable(&object)?;
            eprintln!("R0: {fault}");
            let mut file = OpenOptions::new().read(true).write(true).open(&object)?;
            if fault == "truncation" {
                file.set_len(length / 2)?;
            } else {
                file.seek(SeekFrom::Start(length / 2))?;
                let mut byte = [0];
                file.read_exact(&mut byte)?;
                byte[0] ^= 0x80;
                file.seek(SeekFrom::Start(length / 2))?;
                file.write_all(&byte)?;
            }
            file.sync_all()?;
            drop(file);
            self.reconnect()?;
            self.verify(id, false)?;
            fs::copy(&saved, &object)?;
            self.reconnect()?;
            self.verify(id, true)?;
        }
        Ok(())
    }

    fn drill(
        &mut self,
        copy_root: &Path,
        recovery_root: &Path,
        acknowledge_unknown_media: bool,
    ) -> Result<()> {
        self.phase("media-identity");
        let media = media::guard(
            &self.root,
            copy_root,
            recovery_root,
            acknowledge_unknown_media,
        )?;
        self.evidence.topology = Some(TopologyEvidence {
            media,
            transport: "local-filesystem",
            concurrent_writers: false,
            copy_command_ref: "r0/repository-copy",
            copy_consistency: match self.kind {
                Kind::Restic => {
                    "quiescent source repository; backend-native copy; receiver repository full read and exact restores"
                }
                Kind::Kopia => {
                    "quiescent source repository; backend-native copy; receiver latest-snapshot full read and exact restores of both revisions"
                }
            },
            key_role: "key-holding receiver; shared disposable password; backend repository keys not separately rotated",
            delete_behavior: "no delete command; restic copy additive; Kopia sync-to without --delete",
            source_absence: "not-reached",
            snapshot_mapping: vec![],
            inventory_before: None,
            inventory_after: None,
        });
        let version_args = match self.kind {
            Kind::Restic => vec!["version"],
            Kind::Kopia => vec!["--version"],
        };
        let version = self.execute("version-pin", &version_args, true, false)?;
        let actual = match self.kind {
            Kind::Restic => version.split_whitespace().nth(1),
            Kind::Kopia => version.split_whitespace().next(),
        };
        ensure!(
            actual == Some(self.evidence.backend_version.as_str()),
            "unsupported backend version"
        );

        self.phase("fixture-a");
        let first = fixture::generate(&self.source, false)?;
        self.oracle(1, &first)?;
        let session = self.session.clone();
        let repository = self.repository.clone();
        self.init_repository(&session, &repository, "source-init")?;
        let snapshot_a = self.snapshot("snapshot-a")?;
        self.evidence.snapshots.push(snapshot_a.clone());
        self.size()?;
        let snapshot_unchanged = self.snapshot("snapshot-unchanged")?;
        self.evidence.snapshots.push(snapshot_unchanged);
        self.size()?;

        self.phase("fixture-b");
        let second = fixture::mutate(&self.source)?;
        self.oracle(2, &second)?;
        let snapshot_b = self.snapshot("snapshot-b")?;
        ensure!(
            snapshot_a != snapshot_b,
            "changed corpus must produce a distinct snapshot"
        );
        self.evidence.snapshots.push(snapshot_b.clone());
        self.size()?;
        let history = self.list()?;
        for id in self.evidence.snapshots.clone() {
            ensure!(
                history.contains(&id),
                "committed source snapshot missing from history"
            );
        }
        self.verify(&snapshot_a, true)?;

        self.phase("repository-copy");
        let copy_repository = copy_root.join("repository");
        ensure!(
            !copy_repository.try_exists()?,
            "copy repository target must not exist"
        );
        self.copy_repository(&copy_repository)?;
        self.pending_mutation = false;
        let before = repository_inventory(&copy_repository)?;
        let summary_before = summarize_inventory(self.kind, &before, None)?;
        if let Some(topology) = self.evidence.topology.as_mut() {
            topology.inventory_before = Some(summary_before);
        }

        self.phase("source-absence");
        fs::remove_dir_all(&self.source)?;
        fs::remove_dir_all(&self.repository)?;
        self.remove_writer_state()?;
        ensure!(
            !self.source.try_exists()? && !self.repository.try_exists()?,
            "source material remained after simulated source loss"
        );
        if let Some(topology) = self.evidence.topology.as_mut() {
            topology.source_absence = "source tree, source repository, writer config and cache removed after copy; same-host simulation, not physical disk removal";
        }
        self.repository = copy_repository.clone();

        let recovery_session = Session::new(recovery_root);
        self.phase("receiver-wrong-password");
        self.wrong_password_in(&recovery_session, &copy_repository)?;
        self.phase("receiver-fresh-client");
        self.connect_repository(&recovery_session, &copy_repository, "fresh-client-connect")?;
        let history = self.list_in(&recovery_session, &copy_repository, "receiver-history")?;
        let mapping = map_snapshots(self.kind, &self.evidence.snapshots, &history)?;
        if let Some(topology) = self.evidence.topology.as_mut() {
            topology.snapshot_mapping = mapping.clone();
        }
        let restore_b = mapping
            .last()
            .context("no copied snapshot available for receiver restore")?
            .copy_id
            .clone();
        let restore_a = mapping
            .first()
            .context("no copied snapshot available for receiver restore")?
            .copy_id
            .clone();
        match self.kind {
            Kind::Restic => {
                self.execute_in(
                    &recovery_session,
                    &copy_repository,
                    "receiver-repository-full-read",
                    &["check", "--read-data"],
                    true,
                    false,
                )?;
            }
            Kind::Kopia => {
                self.execute_in(
                    &recovery_session,
                    &copy_repository,
                    "receiver-snapshot-full-read",
                    &[
                        "snapshot",
                        "verify",
                        "--verify-files-percent=100",
                        &restore_b,
                    ],
                    true,
                    false,
                )?;
            }
        }
        self.restore_in(
            &recovery_session,
            &copy_repository,
            &restore_a,
            "receiver-restore-a",
            &first,
            recovery_root,
        )?;
        self.restore_in(
            &recovery_session,
            &copy_repository,
            &restore_b,
            "receiver-restore-b",
            &second,
            recovery_root,
        )?;

        let after = repository_inventory(&copy_repository)?;
        let summary_after = summarize_inventory(self.kind, &after, Some(&before))?;
        let unchanged =
            summary_after.missing == 0 && summary_after.changed == 0 && summary_after.added == 0;
        if let Some(topology) = self.evidence.topology.as_mut() {
            topology.inventory_after = Some(summary_after);
        }
        ensure!(
            unchanged,
            "receiver access missing/changed/added non-ephemeral repository objects"
        );
        Ok(())
    }

    fn init_repository(&mut self, session: &Session, repository: &Path, name: &str) -> Result<()> {
        match self.kind {
            Kind::Restic => {
                self.execute_in(
                    session,
                    repository,
                    name,
                    &["init", "--repository-version=2"],
                    true,
                    false,
                )?;
            }
            Kind::Kopia => {
                self.execute_in(
                    session,
                    repository,
                    name,
                    &[
                        "repository",
                        "create",
                        "filesystem",
                        "--path",
                        text_path(repository)?,
                        "--no-check-for-updates",
                        "--no-enable-actions",
                    ],
                    true,
                    false,
                )?;
            }
        }
        Ok(())
    }

    fn copy_repository(&mut self, destination: &Path) -> Result<()> {
        let session = self.session.clone();
        match self.kind {
            Kind::Restic => {
                self.execute_in(
                    &session,
                    destination,
                    "copy-destination-init",
                    &["init", "--repository-version=2"],
                    true,
                    false,
                )?;
                let password_file = self.root.join("copy-source-password");
                let mut options = OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let mut file = options.open(&password_file)?;
                file.write_all(self.password.as_bytes())?;
                file.sync_all()?;
                drop(file);
                let source = text_path(&self.repository)?.to_owned();
                let password_path = text_path(&password_file)?.to_owned();
                let result = self.execute_flagged(
                    &session,
                    destination,
                    "repository-copy",
                    &[
                        "copy",
                        "--from-repo",
                        &source,
                        "--from-password-file",
                        &password_path,
                    ],
                    true,
                    false,
                    true,
                );
                let _ = fs::remove_file(&password_file);
                result?;
            }
            Kind::Kopia => {
                fs::create_dir(destination)?;
                ensure!(
                    fs::read_dir(destination)?.next().is_none(),
                    "copy repository target must be empty"
                );
                let destination_text = text_path(destination)?.to_owned();
                self.execute_flagged(
                    &session,
                    &self.repository.clone(),
                    "repository-copy",
                    &[
                        "repository",
                        "sync-to",
                        "filesystem",
                        "--path",
                        &destination_text,
                    ],
                    true,
                    false,
                    true,
                )?;
            }
        }
        Ok(())
    }

    fn wrong_password_in(&mut self, session: &Session, repository: &Path) -> Result<()> {
        let mut wrong = session.clone();
        match self.kind {
            Kind::Restic => {
                self.execute_in(
                    session,
                    repository,
                    "wrong-password",
                    &["snapshots", "--json"],
                    false,
                    true,
                )?;
            }
            Kind::Kopia => {
                wrong.config = session.root.join("wrong-password.config");
                self.execute_in(
                    &wrong,
                    repository,
                    "wrong-password",
                    &[
                        "repository",
                        "connect",
                        "filesystem",
                        "--path",
                        text_path(repository)?,
                    ],
                    false,
                    true,
                )?;
            }
        }
        Ok(())
    }

    fn connect_repository(
        &mut self,
        session: &Session,
        repository: &Path,
        name: &str,
    ) -> Result<()> {
        if matches!(self.kind, Kind::Kopia) {
            self.execute_in(
                session,
                repository,
                name,
                &[
                    "repository",
                    "connect",
                    "filesystem",
                    "--path",
                    text_path(repository)?,
                    "--no-check-for-updates",
                    "--no-enable-actions",
                ],
                true,
                false,
            )?;
        }
        Ok(())
    }

    fn remove_writer_state(&self) -> Result<()> {
        for path in [
            self.session.config.clone(),
            self.session.cache.clone(),
            self.root.join("xdg-cache"),
            self.root.join("xdg-config"),
            self.root.join("appdata"),
            self.root.join("localappdata"),
            self.root.join("wrong-password.config"),
        ] {
            if path.is_dir() {
                fs::remove_dir_all(&path)?;
            } else if path.try_exists()? {
                fs::remove_file(&path)?;
            }
        }
        Ok(())
    }
}

fn repository_inventory(root: &Path) -> Result<BTreeMap<String, (u64, String)>> {
    fn walk(
        root: &Path,
        directory: &Path,
        entries: &mut BTreeMap<String, (u64, String)>,
    ) -> Result<()> {
        for item in fs::read_dir(directory)? {
            let item = item?;
            let file_type = item.file_type()?;
            ensure!(!file_type.is_symlink(), "repository inventory found a link");
            if file_type.is_dir() {
                walk(root, &item.path(), entries)?;
            } else if file_type.is_file() {
                let relative = item
                    .path()
                    .strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/");
                let (size, digest) = fixture::hash_file(&item.path())?;
                ensure!(
                    entries.insert(relative, (size, digest)).is_none(),
                    "repository inventory found a duplicate path"
                );
            }
        }
        Ok(())
    }
    let mut entries = BTreeMap::new();
    walk(root, root, &mut entries)?;
    Ok(entries)
}

fn is_ephemeral(kind: Kind, path: &str) -> bool {
    match kind {
        Kind::Restic => path.starts_with("locks/"),
        Kind::Kopia => matches!(path, "kopia.maintenance.f" | "kopia.blobcfg.f"),
    }
}

fn is_data(kind: Kind, path: &str) -> bool {
    match kind {
        Kind::Restic => path.starts_with("data/"),
        Kind::Kopia => matches!(path.as_bytes().first(), Some(b'p' | b'q')),
    }
}

fn summarize_inventory(
    kind: Kind,
    current: &BTreeMap<String, (u64, String)>,
    previous: Option<&BTreeMap<String, (u64, String)>>,
) -> Result<InventorySummary> {
    let mut summary = InventorySummary {
        files: current.len(),
        bytes: 0,
        data_files: 0,
        data_bytes: 0,
        digest: String::new(),
        missing: 0,
        changed: 0,
        added: 0,
        ephemeral_changes: 0,
    };
    let mut hasher = Sha256::new();
    for (path, (size, digest)) in current {
        summary.bytes += size;
        if is_data(kind, path) {
            summary.data_files += 1;
            summary.data_bytes += size;
        }
        let ephemeral = is_ephemeral(kind, path);
        if !ephemeral {
            hasher.update(path.as_bytes());
            hasher.update([0]);
            hasher.update(size.to_le_bytes());
            hasher.update(digest.as_bytes());
            hasher.update([0]);
        }
        if let Some(previous) = previous {
            match previous.get(path) {
                None if ephemeral => summary.ephemeral_changes += 1,
                None => summary.added += 1,
                Some((old_size, old_digest)) if old_size != size || old_digest != digest => {
                    if ephemeral {
                        summary.ephemeral_changes += 1;
                    } else {
                        summary.changed += 1;
                    }
                }
                Some(_) => {}
            }
        }
    }
    if let Some(previous) = previous {
        for path in previous.keys() {
            if !current.contains_key(path) {
                if is_ephemeral(kind, path) {
                    summary.ephemeral_changes += 1;
                } else {
                    summary.missing += 1;
                }
            }
        }
    }
    summary.digest = format!("{:x}", hasher.finalize());
    Ok(summary)
}

fn map_snapshots(kind: Kind, source_ids: &[String], output: &str) -> Result<Vec<SnapshotMapping>> {
    let value: Value = serde_json::from_str(output).context("invalid receiver snapshot JSON")?;
    let items = value
        .as_array()
        .context("receiver snapshot output is not an array")?;
    ensure!(
        items.len() == source_ids.len(),
        "receiver repository has an unexpected snapshot count"
    );
    let mut available = BTreeMap::new();
    for item in items {
        let id = item["id"]
            .as_str()
            .context("receiver snapshot without an ID")?;
        ensure!(
            id.len() == 64 || id.len() == 32,
            "unexpected receiver snapshot ID length"
        );
        let source = match kind {
            Kind::Restic => item["original"]
                .as_str()
                .context("restic copy without an original snapshot ID")?,
            Kind::Kopia => id,
        };
        ensure!(
            available.insert(source.to_owned(), id.to_owned()).is_none(),
            "receiver snapshot mapping is ambiguous"
        );
    }
    let mut mapping = Vec::new();
    for source in source_ids {
        let copy = available
            .remove(source)
            .with_context(|| format!("receiver snapshot missing for source {source}"))?;
        mapping.push(SnapshotMapping {
            source_id: source.clone(),
            copy_id: copy,
        });
    }
    ensure!(available.is_empty(), "receiver has unmapped snapshots");
    Ok(mapping)
}

fn text_path(path: &Path) -> Result<&str> {
    path.to_str().context("backend path must be UTF-8")
}

fn backend_path(path: PathBuf) -> Result<PathBuf> {
    #[cfg(windows)]
    {
        // Kopia's sharded filesystem provider mixes separators and rejects Rust's
        // verbatim drive prefix. R0 only accepts ordinary local drive paths.
        let text = path.to_str().context("work directory must be UTF-8")?;
        let text = text.strip_prefix(r"\\?\").unwrap_or(text);
        ensure!(
            text.as_bytes().get(1) == Some(&b':') && text.len() <= 180,
            "R0 requires a local drive path of at most 180 UTF-8 bytes"
        );
        Ok(PathBuf::from(text))
    }
    #[cfg(not(windows))]
    Ok(path)
}

fn parse_snapshot(kind: Kind, output: &str) -> Result<String> {
    let mut id = None;
    match kind {
        Kind::Restic => {
            for line in output.lines() {
                let value: Value =
                    serde_json::from_str(line).context("invalid restic JSON output")?;
                if value["message_type"] == "summary" {
                    id = value["snapshot_id"].as_str().map(str::to_owned);
                }
            }
        }
        Kind::Kopia => {
            let value: Value = serde_json::from_str(output).context("invalid Kopia JSON output")?;
            ensure!(
                value["incomplete"].is_null() || value["incomplete"] == "",
                "incomplete Kopia snapshot"
            );
            ensure!(
                value["stats"]["errorCount"].as_u64().unwrap_or(0) == 0,
                "Kopia scan has errors"
            );
            id = value["id"].as_str().map(str::to_owned);
        }
    }
    let id = id.context("backend did not return a committed snapshot ID")?;
    ensure!(
        id.len() == 64 || id.len() == 32,
        "unexpected snapshot ID length"
    );
    ensure!(
        id.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid snapshot ID"
    );
    Ok(id)
}

fn make_writable(path: &Path) -> Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(permissions.mode() | 0o200);
    }
    #[cfg(windows)]
    #[allow(
        clippy::permissions_set_readonly_false,
        reason = "Windows DOS readonly flag on owned fault data"
    )]
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unapproved_binary_is_rejected_before_execution_and_retains_setup_failure() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let binary = temp.path().join("unapproved.exe");
        fs::write(&binary, b"not an approved executable")?;
        let work = temp.path().join("run");
        assert!(bakeoff(Kind::Restic, &binary, &work, None).is_err());
        let report: Value = serde_json::from_slice(&fs::read(work.join("evidence.json"))?)?;
        assert_eq!(report["schema_version"], 3);
        assert!(report.get("topology").is_none());
        assert_eq!(report["result"], "fail");
        assert_eq!(report["active_phase"], "binary-pin");
        assert!(
            report["steps"]
                .as_array()
                .unwrap()
                .iter()
                .all(|step| step["name"] != "version-pin")
        );
        fs::write(work.join("keep"), b"existing data")?;
        assert!(bakeoff(Kind::Restic, &binary, &work, None).is_err());
        assert_eq!(fs::read(work.join("keep"))?, b"existing data");
        Ok(())
    }

    #[test]
    fn snapshot_results_are_fail_closed() {
        assert!(parse_snapshot(Kind::Restic, "{\"message_type\":\"status\"}").is_err());
        assert!(parse_snapshot(Kind::Kopia, "{\"id\":\"../escape\"}").is_err());
        assert!(
            parse_snapshot(
                Kind::Kopia,
                &format!(
                    "{{\"id\":\"{}\",\"incomplete\":\"interrupted\"}}",
                    "a".repeat(32)
                )
            )
            .is_err()
        );
        let id = "a".repeat(64);
        assert_eq!(
            parse_snapshot(
                Kind::Restic,
                &format!("{{\"message_type\":\"summary\",\"snapshot_id\":\"{id}\"}}\n")
            )
            .unwrap(),
            id
        );
    }

    #[test]
    fn receiver_mapping_is_fail_closed() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let c = "c".repeat(64);
        let restic = format!(
            "[{{\"id\":\"{b}\",\"original\":\"{a}\"}},{{\"id\":\"{c}\",\"original\":\"{b}\"}}]"
        );
        let mapped = map_snapshots(Kind::Restic, &[a.clone(), b.clone()], &restic).unwrap();
        assert_eq!(mapped[0].copy_id, b);
        assert_eq!(mapped[1].copy_id, c);
        assert!(map_snapshots(Kind::Restic, std::slice::from_ref(&a), &restic).is_err());
        let missing = format!("[{{\"id\":\"{b}\",\"original\":\"{a}\"}}]");
        assert!(map_snapshots(Kind::Restic, &[a.clone(), b.clone()], &missing).is_err());
        let kopia = format!("[{{\"id\":\"{a}\"}},{{\"id\":\"{b}\"}}]");
        let mapped = map_snapshots(Kind::Kopia, &[b.clone(), a.clone()], &kopia).unwrap();
        assert_eq!(mapped[0].copy_id, b);
        assert_eq!(mapped[1].copy_id, a);
        assert!(map_snapshots(Kind::Kopia, &[c], &kopia).is_err());
    }

    #[test]
    fn inventory_diff_detects_data_changes_and_allows_ephemerals() -> Result<()> {
        let mut before = BTreeMap::new();
        before.insert("data/pack".into(), (3, "a".to_owned()));
        before.insert("locks/one".into(), (1, "b".to_owned()));
        let mut after = before.clone();
        after.insert("data/pack".into(), (3, "c".to_owned()));
        after.remove("locks/one");
        after.insert("locks/two".into(), (1, "b".to_owned()));
        after.insert("index/new".into(), (1, "d".to_owned()));
        let summary = summarize_inventory(Kind::Restic, &after, Some(&before))?;
        assert_eq!(summary.changed, 1);
        assert_eq!(summary.added, 1);
        assert_eq!(summary.missing, 0);
        assert_eq!(summary.ephemeral_changes, 2);
        after.remove("data/pack");
        let summary = summarize_inventory(Kind::Restic, &after, Some(&before))?;
        assert_eq!(summary.missing, 1);
        drop(before);
        Ok(())
    }

    #[test]
    fn subprocess_limits_do_not_expose_child_output() -> Result<()> {
        for mode in ["flood", "sleep"] {
            let mut command = Command::new(std::env::current_exe()?);
            command
                .args([
                    "--ignored",
                    "--exact",
                    "backend::tests::child_fixture",
                    "--nocapture",
                ])
                .env("PEERVAULT_CHILD_TEST", mode);
            let error = bounded(&mut command, Duration::from_millis(150), 1024).unwrap_err();
            assert!(!format!("{error:#}").contains("secret-from-child"));
        }
        Ok(())
    }

    #[test]
    #[ignore = "only run as a subprocess from the limits test"]
    fn child_fixture() {
        match std::env::var("PEERVAULT_CHILD_TEST").as_deref() {
            Ok("flood") => {
                eprintln!("secret-from-child{}", "x".repeat(4096));
            }
            Ok("sleep") => std::thread::sleep(Duration::from_secs(30)),
            _ => panic!("subprocess mode required"),
        }
    }
}
