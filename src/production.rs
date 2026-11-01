//! V0.1 local capture, history, full-read verification and fresh-target restore.
//!
//! The adapter is deliberately narrow: restic owns repository format, encryption,
//! deduplication and chunk integrity. PeerVault owns source policy, the encrypted
//! tree oracle, contained restore and the diagnostic reports.

use crate::pins;
use anyhow::{Context, Result, bail, ensure};
use bailiff::fixture::{hash_file, validate_path};
use bailiff::process::bounded;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tar::EntryType;

pub const SCHEMA_VERSION: u32 = 2;
pub const EXIT_FAILURE: i32 = 3;
pub const EXIT_INCOMPLETE: i32 = 4;

const MANIFEST_SCHEMA_VERSION: u32 = 1;
const CAPTURE_TAG: &str = "peervault-capture";
const CAPTURE_ID_TAG_PREFIX: &str = "peervault-capture-id-";
const MANIFEST_TAG_PREFIX: &str = "peervault-manifest-";
const HOST_TAG: &str = "peervault";
const MANIFEST_LIMIT: u64 = 64 * 1024 * 1024;
pub(crate) const COMMAND_TIMEOUT: Duration = Duration::from_secs(3600);
pub(crate) const OUTPUT_LIMIT: u64 = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 1_000_000;
const POLICY: &str = r#"{"schema_version":1,"include":["regular-files","directories"],"exclude":["symlinks","reparse-points","special-files"],"case_collisions":"reject","path_encoding":"utf-8","path_syntax":"portable-relative","max_depth":16,"max_path_bytes":1024}"#;

#[derive(Debug)]
pub struct ExitFailure {
    pub code: i32,
    pub message: String,
}

impl fmt::Display for ExitFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ExitFailure {}

fn failure(code: i32, message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(ExitFailure {
        code,
        message: message.into(),
    })
}

pub fn operation_failure(error: anyhow::Error) -> anyhow::Error {
    if error.downcast_ref::<ExitFailure>().is_some() {
        error
    } else {
        failure(EXIT_FAILURE, format!("{error:#}"))
    }
}

pub fn exit_code(error: &anyhow::Error) -> i32 {
    error
        .downcast_ref::<ExitFailure>()
        .map(|failure| failure.code)
        .unwrap_or(1)
}

#[derive(Clone, Serialize)]
pub struct BackendInfo {
    pub path: String,
    #[serde(skip)]
    pub executable: PathBuf,
    pub version: String,
    pub executable_sha256: String,
}

#[derive(Serialize)]
pub struct RepositoryProfile {
    pub schema_version: u32,
    pub repository: String,
    pub repository_format: &'static str,
    pub backend: BackendInfo,
    pub policy_sha256: String,
    pub initialized_unix_seconds: u64,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceManifest {
    pub schema_version: u32,
    pub policy_sha256: String,
    pub directories: Vec<String>,
    pub files: Vec<FileEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SnapshotRef {
    pub id: String,
    pub capture_id: Option<String>,
    pub time: String,
    pub hostname: Option<String>,
    pub paths: Vec<String>,
    pub tags: Vec<String>,
    pub oracle_manifest: Option<String>,
}

#[derive(Serialize)]
pub struct CaptureReport {
    pub schema_version: u32,
    pub result: &'static str,
    pub scan: &'static str,
    pub snapshot_id: String,
    pub capture_id: String,
    pub manifest_snapshot_id: Option<String>,
    pub source: String,
    pub files: usize,
    pub directories: usize,
    pub bytes: u64,
    pub policy_sha256: String,
    pub backend: BackendInfo,
    pub elapsed_ms: u128,
    pub limitations: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct HistoryReport {
    pub schema_version: u32,
    pub result: &'static str,
    pub snapshots: Vec<SnapshotRef>,
}

#[derive(Serialize)]
pub struct VerifyReport {
    pub schema_version: u32,
    pub result: &'static str,
    pub scope: &'static str,
    pub snapshot_id: Option<String>,
    pub backend: BackendInfo,
    pub elapsed_ms: u128,
}

#[derive(Serialize)]
pub struct RestoreReport {
    pub schema_version: u32,
    pub result: &'static str,
    pub snapshot_id: String,
    pub manifest_snapshot_id: String,
    pub destination: String,
    pub files: usize,
    pub directories: usize,
    pub bytes: u64,
    pub verification: &'static str,
    pub metadata_scope: &'static str,
    pub policy_sha256: String,
    pub tree_sha256: String,
    pub backend: BackendInfo,
    pub elapsed_ms: u128,
    pub limitations: Vec<&'static str>,
}

#[derive(Deserialize)]
struct ResticSnapshot {
    id: String,
    time: String,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

pub(crate) struct Credential {
    file: Option<PathBuf>,
}

impl Credential {
    pub(crate) fn new(file: Option<&Path>) -> Result<Self> {
        if let Some(file) = file {
            let metadata = fs::metadata(file).context("cannot read password file")?;
            ensure!(metadata.is_file(), "password file must be a regular file");
            ensure!(
                (1..=8192).contains(&metadata.len()),
                "password file must contain 1..=8192 bytes"
            );
            return Ok(Self {
                file: Some(fs::canonicalize(file)?),
            });
        }
        let password = std::env::var("PEERVAULT_PASSWORD").unwrap_or_default();
        ensure!(
            !password.is_empty(),
            "set PEERVAULT_PASSWORD or pass --password-file"
        );
        Ok(Self { file: None })
    }

    fn apply(&self, command: &mut Command) {
        if let Some(file) = &self.file {
            command.env("RESTIC_PASSWORD_FILE", file);
        } else if let Ok(password) = std::env::var("PEERVAULT_PASSWORD") {
            command.env("RESTIC_PASSWORD", password);
        }
    }
}

pub(crate) fn policy_sha256() -> String {
    format!("{:x}", Sha256::digest(POLICY.as_bytes()))
}

pub(crate) fn backend(path: &Path) -> Result<BackendInfo> {
    let pins = pins::load()?;
    let pin = pins.restic;
    let asset = match std::env::consts::OS {
        "windows" => pin.windows,
        "linux" => pin.linux,
        _ => bail!("unsupported backend platform"),
    };
    ensure!(
        std::env::consts::ARCH == "x86_64",
        "unsupported backend architecture"
    );
    let executable = std::path::absolute(path).context("cannot resolve backend path")?;
    let (size, sha256) = hash_file(&executable).context("cannot hash configured backend")?;
    ensure!(size <= 128 * 1024 * 1024, "backend executable is oversized");
    ensure!(
        sha256 == asset.executable_sha256,
        "unapproved backend executable hash; no operation permitted"
    );
    let mut command = Command::new(&executable);
    command.env_clear();
    for name in ["PATH", "SystemRoot", "WINDIR", "TMP", "TEMP", "TMPDIR"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command.arg("version").stdin(Stdio::null());
    let (code, stdout) = bounded(&mut command, Duration::from_secs(10), 64 * 1024)
        .context("backend version check failed")?;
    ensure!(code == Some(0), "backend version check exited nonzero");
    ensure!(
        stdout.split_whitespace().nth(1) == Some(pin.version.as_str()),
        "unsupported backend version"
    );
    Ok(BackendInfo {
        path: path.display().to_string(),
        executable,
        version: pin.version,
        executable_sha256: sha256,
    })
}

pub(crate) fn command(
    backend: &BackendInfo,
    repository: &Path,
    credential: &Credential,
) -> Result<Command> {
    let repository = std::path::absolute(repository).context("cannot resolve repository path")?;
    let mut command = Command::new(&backend.executable);
    command.env_clear();
    for name in ["PATH", "SystemRoot", "WINDIR", "TMP", "TEMP", "TMPDIR"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
        .arg("--repo")
        .arg(repository)
        .arg("--no-cache")
        .stdin(Stdio::null());
    credential.apply(&mut command);
    Ok(command)
}

pub(crate) fn run(command: &mut Command, timeout: Duration, limit: u64) -> Result<String> {
    let (code, stdout) = bounded(command, timeout, limit)?;
    ensure!(code == Some(0), "backend command exited nonzero");
    Ok(stdout)
}

pub(crate) fn full_read(
    backend: &BackendInfo,
    repository: &Path,
    credential: &Credential,
) -> Result<()> {
    let mut command = command(backend, repository, credential)?;
    command.args(["check", "--read-data", "--no-lock"]);
    run(&mut command, COMMAND_TIMEOUT, OUTPUT_LIMIT).map(|_| ())
}

pub(crate) fn repository_config(
    backend: &BackendInfo,
    repository: &Path,
    credential: &Credential,
) -> Result<serde_json::Value> {
    let mut command = command(backend, repository, credential)?;
    command.args(["cat", "config", "--no-lock"]);
    let output = run(&mut command, COMMAND_TIMEOUT, OUTPUT_LIMIT)?;
    serde_json::from_str(&output).context("invalid repository config")
}

fn backup_summary(output: &str) -> Result<(String, u64, u64)> {
    let mut summary = None;
    for line in output.lines() {
        let value: serde_json::Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if value["message_type"] == "summary" {
            let id = value["snapshot_id"]
                .as_str()
                .context("summary lacks a snapshot ID")?
                .to_owned();
            let files = value["total_files_processed"]
                .as_u64()
                .context("summary lacks a processed-file count")?;
            let bytes = value["total_bytes_processed"]
                .as_u64()
                .context("summary lacks a processed-byte count")?;
            summary = Some((id, files, bytes));
        }
    }
    let summary = summary.context("backend did not return a committed snapshot ID")?;
    validate_snapshot_id(&summary.0)?;
    Ok(summary)
}

pub(crate) fn validate_snapshot_id(id: &str) -> Result<()> {
    ensure!(
        id.len() == 64 && id.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "invalid snapshot ID"
    );
    Ok(())
}

fn snapshots(
    backend: &BackendInfo,
    repository: &Path,
    credential: &Credential,
) -> Result<Vec<ResticSnapshot>> {
    let mut command = command(backend, repository, credential)?;
    command.args(["snapshots", "--json", "--no-lock"]);
    let output = run(&mut command, COMMAND_TIMEOUT, OUTPUT_LIMIT)
        .context("cannot list snapshots; check repository credentials and state")?;
    let snapshots: Vec<ResticSnapshot> =
        serde_json::from_str(&output).context("invalid backend snapshot JSON")?;
    for snapshot in &snapshots {
        validate_snapshot_id(&snapshot.id)?;
    }
    Ok(snapshots)
}

fn manifest_tag(capture_id: &str) -> String {
    format!("{MANIFEST_TAG_PREFIX}{capture_id}")
}

fn capture_id(tags: &[String]) -> Option<String> {
    tags.iter().find_map(|tag| {
        tag.strip_prefix(CAPTURE_ID_TAG_PREFIX)
            .filter(|value| validate_capture_id(value).is_ok())
            .map(str::to_owned)
    })
}

pub(crate) fn validate_capture_id(id: &str) -> Result<()> {
    ensure!(
        matches!(id.len(), 32 | 64) && id.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "invalid capture ID"
    );
    Ok(())
}

pub(crate) fn find_manifest(
    backend: &BackendInfo,
    repository: &Path,
    credential: &Credential,
    snapshot_id: &str,
) -> Result<(String, String)> {
    let snapshots = snapshots(backend, repository, credential)?;
    let data = snapshots
        .iter()
        .find(|snapshot| snapshot.id == snapshot_id)
        .with_context(|| format!("snapshot {snapshot_id} not found in repository"))?;
    let capture_id = capture_id(&data.tags).unwrap_or_else(|| snapshot_id.to_owned());
    let tag = manifest_tag(&capture_id);
    let mut found = Vec::new();
    for snapshot in snapshots {
        if snapshot.tags.iter().any(|value| value == &tag) {
            found.push(snapshot.id);
        }
    }
    ensure!(
        found.len() == 1,
        "expected exactly one PeerVault manifest snapshot for {capture_id}"
    );
    Ok((capture_id, found.remove(0)))
}

pub fn init(
    repository: &Path,
    backend_path: &Path,
    password_file: Option<&Path>,
) -> Result<RepositoryProfile> {
    let backend = backend(backend_path)?;
    let credential = Credential::new(password_file)?;
    if repository.try_exists()? {
        ensure!(repository.is_dir(), "repository path must be a directory");
        ensure!(
            fs::read_dir(repository)?.next().is_none(),
            "repository directory must be empty"
        );
    } else {
        fs::create_dir_all(repository).context("cannot create repository directory")?;
    }
    let mut command = command(&backend, repository, &credential)?;
    command.args(["init", "--repository-version=2"]);
    run(&mut command, COMMAND_TIMEOUT, OUTPUT_LIMIT).context("repository init failed")?;
    let repository = fs::canonicalize(repository)?;
    let profile = RepositoryProfile {
        schema_version: SCHEMA_VERSION,
        repository: repository.display().to_string(),
        repository_format: "restic-v2",
        backend,
        policy_sha256: policy_sha256(),
        initialized_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    print_json(&profile)?;
    Ok(profile)
}

fn validate_tag(tag: &str) -> Result<()> {
    ensure!(
        !tag.is_empty()
            && tag.len() <= 64
            && tag
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')),
        "tag must be 1..=64 ASCII alphanumeric, '-', '_' or '.'"
    );
    Ok(())
}

pub fn capture(
    repository: &Path,
    backend_path: &Path,
    source: &Path,
    tag: Option<&str>,
    password_file: Option<&Path>,
) -> Result<CaptureReport> {
    let backend = backend(backend_path)?;
    let credential = Credential::new(password_file)?;
    ensure!(repository.is_dir(), "repository does not exist");
    ensure!(source.is_dir(), "source must be a directory");
    let started = Instant::now();
    let before = scan_source(source)?;
    if before.files.is_empty() {
        let report = CaptureReport {
            schema_version: SCHEMA_VERSION,
            result: "empty",
            scan: "complete",
            snapshot_id: String::new(),
            capture_id: String::new(),
            manifest_snapshot_id: None,
            source: source.display().to_string(),
            files: 0,
            directories: before.directories.len(),
            bytes: 0,
            policy_sha256: before.policy_sha256,
            backend,
            elapsed_ms: started.elapsed().as_millis(),
            limitations: vec![
                "empty selection is refused: the backend cannot commit an empty snapshot",
                "no repository mutation was performed",
            ],
        };
        print_json(&report)?;
        return Err(failure(
            EXIT_INCOMPLETE,
            "empty source selection; no snapshot created",
        ));
    }
    let capture_id = random_hex()?;
    let mut command = command(&backend, repository, &credential)?;
    command.current_dir(source).args([
        "backup",
        "--json",
        "--host",
        HOST_TAG,
        "--tag",
        CAPTURE_TAG,
        "--tag",
        &format!("{CAPTURE_ID_TAG_PREFIX}{capture_id}"),
    ]);
    if let Some(tag) = tag {
        validate_tag(tag)?;
        command.args(["--tag", &format!("peervault-name-{tag}")]);
    }
    command.arg(".");
    let output = run(&mut command, COMMAND_TIMEOUT, OUTPUT_LIMIT).context("capture failed")?;
    let (snapshot_id, backend_files, backend_bytes) = backup_summary(&output)?;
    let after = scan_source(source)?;
    let expected_bytes: u64 = before.files.iter().map(|entry| entry.size).sum();
    let scan = if before != after {
        "changed-during-capture"
    } else if backend_files != before.files.len() as u64 || backend_bytes != expected_bytes {
        "backend-count-mismatch"
    } else {
        "complete"
    };
    let incomplete = scan != "complete";
    let report = CaptureReport {
        schema_version: SCHEMA_VERSION,
        result: if incomplete { "incomplete" } else { "pass" },
        scan,
        snapshot_id: snapshot_id.clone(),
        capture_id: capture_id.clone(),
        manifest_snapshot_id: None,
        source: source.display().to_string(),
        files: before.files.len(),
        directories: before.directories.len(),
        bytes: expected_bytes,
        policy_sha256: before.policy_sha256.clone(),
        backend: backend.clone(),
        elapsed_ms: started.elapsed().as_millis(),
        limitations: vec![
            "file-level consistency only; no application quiesce",
            "portable regular-file/directory subset; no links, reparse points or special files",
            "tree oracle is stored as a separate encrypted restic snapshot",
        ],
    };
    if incomplete {
        print_json(&report)?;
        return Err(failure(
            EXIT_INCOMPLETE,
            format!("capture scan {scan}; no PeerVault oracle published"),
        ));
    }
    let manifest_snapshot_id =
        publish_manifest(&backend, repository, &credential, &before, &capture_id)?;
    let report = CaptureReport {
        manifest_snapshot_id: Some(manifest_snapshot_id),
        ..report
    };
    print_json(&report)?;
    Ok(report)
}

fn publish_manifest(
    backend: &BackendInfo,
    repository: &Path,
    credential: &Credential,
    manifest: &SourceManifest,
    capture_id: &str,
) -> Result<String> {
    let temporary = tempfile::tempdir().context("cannot create manifest staging")?;
    let path = temporary.path().join("manifest.json");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    serde_json::to_writer_pretty(&mut file, manifest)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    let mut command = command(backend, repository, credential)?;
    command.current_dir(temporary.path()).args([
        "backup",
        "--json",
        "--host",
        HOST_TAG,
        "--tag",
        &manifest_tag(capture_id),
        "manifest.json",
    ]);
    let output = run(&mut command, COMMAND_TIMEOUT, OUTPUT_LIMIT)
        .context("manifest snapshot failed; data snapshot has no PeerVault oracle")?;
    let (id, files, bytes) = backup_summary(&output)?;
    ensure!(files == 1 && bytes > 0, "manifest snapshot was incomplete");
    Ok(id)
}

pub(crate) fn snapshot_refs(
    backend: &BackendInfo,
    repository: &Path,
    credential: &Credential,
) -> Result<Vec<SnapshotRef>> {
    let snapshots = snapshots(backend, repository, credential)?;
    let mut manifests = BTreeMap::new();
    for snapshot in &snapshots {
        for tag in &snapshot.tags {
            if let Some(capture_id) = tag.strip_prefix(MANIFEST_TAG_PREFIX)
                && validate_capture_id(capture_id).is_ok()
            {
                manifests.insert(capture_id.to_owned(), snapshot.id.clone());
            }
        }
    }
    let mut refs = Vec::new();
    for snapshot in snapshots {
        if snapshot
            .tags
            .iter()
            .any(|tag| tag.starts_with(MANIFEST_TAG_PREFIX))
        {
            continue;
        }
        if !snapshot.tags.iter().any(|tag| tag == CAPTURE_TAG) {
            continue;
        }
        let cid = capture_id(&snapshot.tags);
        let oracle_key = cid.clone().unwrap_or_else(|| snapshot.id.clone());
        refs.push(SnapshotRef {
            id: snapshot.id.clone(),
            capture_id: cid,
            time: snapshot.time,
            hostname: snapshot.hostname,
            paths: snapshot.paths,
            tags: snapshot.tags,
            oracle_manifest: manifests.get(&oracle_key).cloned(),
        });
    }
    Ok(refs)
}

pub fn history(
    repository: &Path,
    backend_path: &Path,
    password_file: Option<&Path>,
) -> Result<HistoryReport> {
    let backend = backend(backend_path)?;
    let credential = Credential::new(password_file)?;
    ensure!(repository.is_dir(), "repository does not exist");
    let snapshots = snapshot_refs(&backend, repository, &credential)?;
    let report = HistoryReport {
        schema_version: SCHEMA_VERSION,
        result: "pass",
        snapshots,
    };
    print_json(&report)?;
    Ok(report)
}

pub fn verify(
    repository: &Path,
    backend_path: &Path,
    snapshot: Option<&str>,
    password_file: Option<&Path>,
) -> Result<VerifyReport> {
    let backend = backend(backend_path)?;
    let credential = Credential::new(password_file)?;
    ensure!(repository.is_dir(), "repository does not exist");
    if let Some(snapshot) = snapshot {
        validate_snapshot_id(snapshot)?;
        let known = snapshots(&backend, repository, &credential)?;
        ensure!(
            known.iter().any(|item| item.id == snapshot),
            "snapshot not found in repository"
        );
    }
    let started = Instant::now();
    full_read(&backend, repository, &credential).context("full-read verification failed")?;
    let report = VerifyReport {
        schema_version: SCHEMA_VERSION,
        result: "pass",
        scope: "repository",
        snapshot_id: snapshot.map(str::to_owned),
        backend,
        elapsed_ms: started.elapsed().as_millis(),
    };
    print_json(&report)?;
    Ok(report)
}

fn validate_destination(destination: &Path) -> Result<()> {
    ensure!(
        !destination.try_exists()?,
        "restore destination must not exist"
    );
    ensure!(
        !destination
            .components()
            .any(|component| matches!(component, Component::ParentDir)),
        "restore destination must not contain .. components"
    );
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    ensure!(parent.is_dir(), "restore destination parent does not exist");
    let absolute = std::path::absolute(destination).context("cannot resolve destination")?;
    let mut current = PathBuf::new();
    for component in absolute.components() {
        if matches!(component, Component::CurDir) {
            continue;
        }
        current.push(component.as_os_str());
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            ensure!(
                !metadata.file_type().is_symlink(),
                "restore destination contains a symbolic link"
            );
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                ensure!(
                    metadata.file_attributes() & 0x400 == 0,
                    "restore destination contains a reparse point"
                );
            }
        }
    }
    Ok(())
}

pub fn restore(
    repository: &Path,
    backend_path: &Path,
    snapshot: &str,
    destination: &Path,
    password_file: Option<&Path>,
) -> Result<RestoreReport> {
    let backend = backend(backend_path)?;
    let credential = Credential::new(password_file)?;
    ensure!(repository.is_dir(), "repository does not exist");
    validate_snapshot_id(snapshot)?;
    validate_destination(destination)?;
    let parent = destination
        .parent()
        .context("restore destination needs a parent directory")?;
    let started = Instant::now();
    let (_, manifest_snapshot) = find_manifest(&backend, repository, &credential, snapshot)?;
    let manifest = read_manifest(&backend, repository, &credential, &manifest_snapshot)?;
    let staging = parent.join(format!(
        ".peervault-staging-{}",
        random_hex().context("OS random source unavailable")?
    ));
    ensure!(!staging.try_exists()?, "staging path collision");
    fs::create_dir(&staging).context("cannot create owned staging directory")?;
    let result = extract_snapshot(
        &backend,
        repository,
        &credential,
        snapshot,
        &staging,
        &manifest,
    );
    match result {
        Ok(()) => {
            if let Err(error) = fs::rename(&staging, destination) {
                let _ = fs::remove_dir_all(&staging);
                return Err(error).context("cannot publish restored tree");
            }
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    }
    let report = RestoreReport {
        schema_version: SCHEMA_VERSION,
        result: "pass",
        snapshot_id: snapshot.to_owned(),
        manifest_snapshot_id: manifest_snapshot,
        destination: destination.display().to_string(),
        files: manifest.files.len(),
        directories: manifest.directories.len(),
        bytes: manifest.files.iter().map(|entry| entry.size).sum(),
        verification: "manifest-hash",
        metadata_scope: "portable paths, sizes, SHA-256 and readonly bit; ACLs, owners and timestamps are not preserved",
        policy_sha256: manifest.policy_sha256.clone(),
        tree_sha256: tree_sha256(&manifest),
        backend,
        elapsed_ms: started.elapsed().as_millis(),
        limitations: vec![
            "whole-snapshot restore into a new empty destination only",
            "no selective, merge, overwrite or resume restore",
            "no proof against a compromised backend that rewrites both data and oracle snapshots",
        ],
    };
    print_json(&report)?;
    Ok(report)
}

pub(crate) fn read_manifest(
    backend: &BackendInfo,
    repository: &Path,
    credential: &Credential,
    manifest_snapshot: &str,
) -> Result<SourceManifest> {
    let mut list = command(backend, repository, credential)?;
    list.args(["ls", "--json", "--no-lock", manifest_snapshot]);
    let output = run(&mut list, COMMAND_TIMEOUT, OUTPUT_LIMIT)?;
    let mut files = Vec::new();
    for line in output.lines() {
        let value: serde_json::Value =
            serde_json::from_str(line).context("invalid manifest snapshot listing")?;
        if value["struct_type"] == "node"
            && value["type"] == "file"
            && let Some(path) = value["path"].as_str()
        {
            files.push(path.to_owned());
        }
    }
    ensure!(
        files.len() == 1 && files[0].ends_with("manifest.json"),
        "manifest snapshot must contain exactly one manifest.json file"
    );
    let mut dump = command(backend, repository, credential)?;
    dump.args(["dump", "--no-lock", manifest_snapshot, &files[0]]);
    let output = run(&mut dump, COMMAND_TIMEOUT, MANIFEST_LIMIT)?;
    let manifest: SourceManifest =
        serde_json::from_str(&output).context("invalid PeerVault tree manifest")?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn extract_snapshot(
    backend: &BackendInfo,
    repository: &Path,
    credential: &Credential,
    snapshot: &str,
    staging: &Path,
    manifest: &SourceManifest,
) -> Result<()> {
    let mut command = command(backend, repository, credential)?;
    command
        .args(["dump", "--no-lock", "--archive", "tar", snapshot, "/"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .context("cannot launch contained extraction")?;
    let stdout = child.stdout.take().context("missing extraction stdout")?;
    let stderr = child.stderr.take().context("missing extraction stderr")?;
    let stderr_reader = std::thread::spawn(move || {
        let mut stderr = stderr;
        let mut buffer = Vec::new();
        let _ = stderr.by_ref().take(256 * 1024).read_to_end(&mut buffer);
        let _ = std::io::copy(&mut stderr, &mut std::io::sink());
        buffer
    });
    let result = extract_tar(stdout, staging, manifest);
    if result.is_err() {
        let _ = child.kill();
    }
    let status = child.wait().context("cannot wait for extraction process")?;
    let _stderr = stderr_reader.join().unwrap_or_default();
    result?;
    ensure!(status.success(), "backend extraction exited nonzero");
    Ok(())
}

fn extract_tar(reader: impl Read, staging: &Path, manifest: &SourceManifest) -> Result<()> {
    let expected_directories: BTreeSet<&str> =
        manifest.directories.iter().map(String::as_str).collect();
    let expected_files: BTreeMap<&str, &FileEntry> = manifest
        .files
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect();
    let mut seen_directories = BTreeSet::new();
    let mut seen_files = BTreeSet::new();
    let mut directory_modes = Vec::new();
    let mut archive = tar::Archive::new(reader);
    for entry in archive.entries().context("invalid extraction tar")? {
        let mut entry = entry.context("invalid extraction tar entry")?;
        let path = std::str::from_utf8(&entry.path_bytes())
            .context("tar entry path is not UTF-8")?
            .trim_start_matches("./")
            .trim_end_matches('/')
            .to_owned();
        if path.is_empty() || path == "." {
            continue;
        }
        validate_path(&path)?;
        let kind = entry.header().entry_type();
        ensure!(
            matches!(kind, EntryType::Regular | EntryType::Directory),
            "unsupported tar entry type for {path}"
        );
        if kind == EntryType::Directory {
            ensure!(
                expected_directories.contains(path.as_str()),
                "tar contains an unexpected directory"
            );
            ensure!(
                seen_directories.insert(path.clone()),
                "duplicate tar directory"
            );
            let target = staging.join(&path);
            fs::create_dir_all(&target).context("cannot create restored directory")?;
            directory_modes.push((target, entry.header().mode()?));
            continue;
        }
        let expected = expected_files
            .get(path.as_str())
            .with_context(|| format!("tar contains an unexpected file {path}"))?;
        ensure!(seen_files.insert(path.clone()), "duplicate tar file {path}");
        ensure!(
            entry.header().size()? == expected.size,
            "restored size metadata mismatch for {path}"
        );
        let target = staging.join(&path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .with_context(|| format!("cannot create restored file {path}"))?;
        let mut hash = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = entry.read(&mut buffer).context("cannot read tar entry")?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
            file.write_all(&buffer[..count])?;
            size += count as u64;
        }
        ensure!(size == expected.size, "restored size mismatch for {path}");
        ensure!(
            format!("{:x}", hash.finalize()) == expected.sha256,
            "restored hash mismatch for {path}"
        );
        set_mode(&target, entry.header().mode()?)?;
        file.sync_all()?;
    }
    ensure!(
        seen_files.len() == expected_files.len()
            && expected_files.keys().all(|path| seen_files.contains(*path)),
        "restore is missing expected files"
    );
    ensure!(
        seen_directories.len() == expected_directories.len()
            && expected_directories
                .iter()
                .all(|path| seen_directories.contains(*path)),
        "restore is missing expected directories"
    );
    directory_modes.sort_by(|left, right| {
        right
            .0
            .components()
            .count()
            .cmp(&left.0.components().count())
    });
    for (path, mode) in directory_modes {
        set_mode(&path, mode)?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777))?;
    Ok(())
}

#[cfg(windows)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    #[allow(
        clippy::permissions_set_readonly_false,
        reason = "restore reproduces the backed-up readonly bit on owned files"
    )]
    permissions.set_readonly(mode & 0o200 == 0);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

pub(crate) fn random_hex() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub(crate) fn tree_sha256(manifest: &SourceManifest) -> String {
    let mut hash = Sha256::new();
    let mut directories: Vec<&String> = manifest.directories.iter().collect();
    directories.sort();
    for directory in directories {
        hash.update(b"d");
        hash.update(directory.as_bytes());
        hash.update([0]);
    }
    let mut files: Vec<&FileEntry> = manifest.files.iter().collect();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    for entry in files {
        hash.update(b"f");
        hash.update(entry.path.as_bytes());
        hash.update([0]);
        hash.update(entry.size.to_le_bytes());
        hash.update(entry.sha256.as_bytes());
        hash.update([0]);
    }
    format!("{:x}", hash.finalize())
}

fn scan_source(root: &Path) -> Result<SourceManifest> {
    let metadata = fs::symlink_metadata(root).context("cannot inspect source root")?;
    ensure!(!metadata.file_type().is_symlink(), "source root is a link");
    ensure!(metadata.is_dir(), "source root must be a directory");
    let mut manifest = SourceManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        policy_sha256: policy_sha256(),
        directories: Vec::new(),
        files: Vec::new(),
    };
    let mut seen = BTreeSet::new();
    visit(root, root, &mut manifest, &mut seen)?;
    manifest.directories.sort();
    manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn visit(
    root: &Path,
    path: &Path,
    manifest: &mut SourceManifest,
    seen: &mut BTreeSet<String>,
) -> Result<()> {
    let metadata = fs::symlink_metadata(path).context("cannot enumerate source")?;
    ensure!(!metadata.file_type().is_symlink(), "links are unsupported");
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "reparse points are unsupported"
        );
    }
    if path != root {
        let relative = path
            .strip_prefix(root)?
            .components()
            .map(|component| component.as_os_str().to_str().context("non-UTF-8 path"))
            .collect::<Result<Vec<_>>>()?
            .join("/");
        validate_path(&relative)?;
        ensure!(
            seen.insert(relative.to_lowercase()),
            "duplicate/colliding path"
        );
        ensure!(
            manifest.files.len() + manifest.directories.len() < MAX_ENTRIES,
            "source entry limit exceeded"
        );
        if metadata.is_dir() {
            manifest.directories.push(relative);
        } else if metadata.is_file() {
            let (size, sha256) = hash_file(path)?;
            manifest.files.push(FileEntry {
                path: relative,
                size,
                sha256,
            });
        } else {
            bail!("special files are unsupported");
        }
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            visit(root, &entry?.path(), manifest, seen)?;
        }
    }
    Ok(())
}

fn validate_manifest(manifest: &SourceManifest) -> Result<()> {
    ensure!(
        manifest.schema_version == MANIFEST_SCHEMA_VERSION,
        "unsupported tree manifest version"
    );
    ensure!(
        manifest.policy_sha256 == policy_sha256(),
        "tree manifest policy does not match this build"
    );
    ensure!(
        manifest.files.len() + manifest.directories.len() <= MAX_ENTRIES,
        "tree manifest entry limit exceeded"
    );
    let mut seen = BTreeSet::new();
    for path in manifest
        .directories
        .iter()
        .chain(manifest.files.iter().map(|entry| &entry.path))
    {
        validate_path(path)?;
        ensure!(
            seen.insert(path.to_lowercase()),
            "duplicate/colliding tree manifest path"
        );
    }
    for entry in &manifest.files {
        ensure!(
            entry.sha256.len() == 64
                && entry
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "invalid tree manifest file hash"
        );
    }
    Ok(())
}

pub(crate) fn print_json(value: &impl Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(files: &[(&str, u64, char)], directories: &[&str]) -> SourceManifest {
        SourceManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            policy_sha256: policy_sha256(),
            directories: directories.iter().map(|path| (*path).to_owned()).collect(),
            files: files
                .iter()
                .map(|(path, size, digest)| FileEntry {
                    path: (*path).to_owned(),
                    size: *size,
                    sha256: digest.to_string().repeat(64),
                })
                .collect(),
        }
    }

    #[test]
    fn tree_manifest_rejects_collisions_and_unsafe_paths() {
        assert!(validate_manifest(&manifest(&[("a", 1, 'a')], &[])).is_ok());
        assert!(validate_manifest(&manifest(&[("../a", 1, 'a')], &[])).is_err());
        assert!(validate_manifest(&manifest(&[("A", 1, 'a'), ("a", 1, 'b')], &[])).is_err());
        assert!(validate_manifest(&manifest(&[("a", 1, 'A')], &[])).is_err());
    }

    #[test]
    fn tree_digest_is_order_independent() {
        let left = manifest(&[("a", 1, 'a'), ("b", 2, 'b')], &["d"]);
        let mut right = left.clone();
        right.directories.reverse();
        right.files.reverse();
        assert_eq!(tree_sha256(&left), tree_sha256(&right));
    }

    #[test]
    fn destination_rejects_links_existing_paths_and_traversal() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let existing = temporary.path().join("existing");
        fs::create_dir(&existing)?;
        assert!(validate_destination(&existing).is_err());
        assert!(validate_destination(&temporary.path().join("sub/../new")).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(temporary.path(), temporary.path().join("link"))?;
            assert!(validate_destination(&temporary.path().join("link/new")).is_err());
        }
        Ok(())
    }

    #[test]
    fn source_scan_rejects_links() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        fs::write(temporary.path().join("file"), b"data")?;
        let manifest = scan_source(temporary.path())?;
        assert_eq!(manifest.files.len(), 1);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("file", temporary.path().join("link"))?;
            assert!(scan_source(temporary.path()).is_err());
        }
        Ok(())
    }

    fn tar_entries(entries: &[(&str, EntryType, &[u8], Option<&str>)]) -> Result<Vec<u8>> {
        let mut builder = tar::Builder::new(Vec::new());
        for (path, kind, data, link) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_path(path)?;
            header.set_entry_type(*kind);
            header.set_size(data.len() as u64);
            header.set_mode(if *kind == EntryType::Directory {
                0o755
            } else {
                0o644
            });
            if let Some(link) = link {
                header.set_link_name(link)?;
            }
            header.set_cksum();
            builder.append(&header, *data)?;
        }
        Ok(builder.into_inner()?)
    }

    fn tar_entry(path: &str, kind: EntryType, data: &[u8], link: Option<&str>) -> Result<Vec<u8>> {
        tar_entries(&[(path, kind, data, link)])
    }

    fn digest(data: &[u8]) -> String {
        format!("{:x}", Sha256::digest(data))
    }

    #[cfg(unix)]
    #[test]
    fn extraction_defers_directory_modes_until_after_contents() -> Result<()> {
        let data = b"payload";
        let mut expected = manifest(&[("locked/file", data.len() as u64, 'x')], &["locked"]);
        expected.files[0].sha256 = digest(data);
        let mut builder = tar::Builder::new(Vec::new());
        let mut directory = tar::Header::new_gnu();
        directory.set_path("locked")?;
        directory.set_entry_type(EntryType::Directory);
        directory.set_size(0);
        directory.set_mode(0o555);
        directory.set_cksum();
        builder.append(&directory, &b""[..])?;
        let mut file = tar::Header::new_gnu();
        file.set_path("locked/file")?;
        file.set_entry_type(EntryType::Regular);
        file.set_size(data.len() as u64);
        file.set_mode(0o644);
        file.set_cksum();
        builder.append(&file, &data[..])?;
        let staging = tempfile::tempdir()?;
        extract_tar(
            std::io::Cursor::new(builder.into_inner()?),
            staging.path(),
            &expected,
        )?;
        assert_eq!(fs::read(staging.path().join("locked/file"))?, data);
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            staging.path().join("locked"),
            fs::Permissions::from_mode(0o755),
        )?;
        Ok(())
    }

    #[test]
    fn extraction_validates_expected_tree_and_hashes() -> Result<()> {
        let data = b"payload";
        let manifest = manifest(&[("dir/file", data.len() as u64, 'x')], &["dir"]);
        let mut manifest = manifest;
        manifest.files[0].sha256 = digest(data);
        let tar = tar_entries(&[
            ("dir", EntryType::Directory, b"", None),
            ("dir/file", EntryType::Regular, data, None),
        ])?;
        let staging = tempfile::tempdir()?;
        extract_tar(std::io::Cursor::new(tar), staging.path(), &manifest)?;
        assert_eq!(fs::read(staging.path().join("dir/file"))?, data);
        Ok(())
    }

    #[test]
    fn extraction_rejects_unexpected_links_and_duplicates() -> Result<()> {
        let staging = tempfile::tempdir()?;
        let empty = manifest(&[], &[]);
        let link = tar_entry("link", EntryType::Symlink, b"", Some("target"))?;
        assert!(extract_tar(std::io::Cursor::new(link), staging.path(), &empty).is_err());
        let data = b"payload";
        let mut expected = manifest(&[("file", data.len() as u64, 'x')], &[]);
        expected.files[0].sha256 = digest(data);
        let duplicate = tar_entries(&[
            ("file", EntryType::Regular, data, None),
            ("file", EntryType::Regular, data, None),
        ])?;
        assert!(extract_tar(std::io::Cursor::new(duplicate), staging.path(), &expected).is_err());
        let mut wrong_hash = expected.clone();
        wrong_hash.files[0].sha256 = "0".repeat(64);
        let single = tar_entry("file", EntryType::Regular, data, None)?;
        assert!(extract_tar(std::io::Cursor::new(single), staging.path(), &wrong_hash).is_err());
        Ok(())
    }

    #[test]
    fn extraction_rejects_missing_files_and_traversal() -> Result<()> {
        let staging = tempfile::tempdir()?;
        let expected = manifest(&[("file", 1, 'x')], &[]);
        assert!(extract_tar(std::io::Cursor::new(Vec::new()), staging.path(), &expected).is_err());
        let mut header = tar::Header::new_gnu();
        header.set_size(1);
        header.set_mode(0o644);
        header.set_entry_type(EntryType::Regular);
        header.as_old_mut().name[..9].copy_from_slice(b"../escape");
        header.set_cksum();
        let mut builder = tar::Builder::new(Vec::new());
        builder.append(&header, &b"x"[..])?;
        let traversal = builder.into_inner()?;
        assert!(extract_tar(std::io::Cursor::new(traversal), staging.path(), &expected).is_err());
        Ok(())
    }
}
