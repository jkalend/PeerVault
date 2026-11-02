//! Experimental V0.8 scheduler/status slice.
//!
//! One local job specification and one owner state file. `tick` runs due
//! verification or scratch-restore commands through the normal CLI, whose
//! bounded process runner owns child trees. A lock prevents two ticks from
//! running concurrently; a stale lock is never cleared by age. State is
//! atomically replaced and becomes owner-only on Unix.

use anyhow::{Context, Result, bail, ensure};
use bailiff::process::bounded;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const SCHEMA_VERSION: u32 = 1;
const MAX_CONFIG_BYTES: u64 = 256 * 1024;
const MAX_STATE_BYTES: u64 = 1024 * 1024;
const MAX_JOB_OUTPUT: u64 = 4 * 1024 * 1024;
const JOB_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_ID_LENGTH: usize = 64;
const MAX_INTERVAL_SECONDS: u64 = 31_536_000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Verify,
    Restore,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobSpec {
    pub id: String,
    pub kind: JobKind,
    pub repository: PathBuf,
    pub backend: PathBuf,
    pub password_file: PathBuf,
    #[serde(default)]
    pub snapshot: Option<String>,
    #[serde(default)]
    pub destination_root: Option<PathBuf>,
    pub interval_seconds: u64,
    pub freshness_seconds: u64,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleConfig {
    pub schema_version: u32,
    pub jobs: Vec<JobSpec>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JobState {
    last_attempt_unix: Option<u64>,
    last_success_unix: Option<u64>,
    last_result: Option<String>,
    last_error: Option<String>,
    last_destination: Option<String>,
    next_due_unix: u64,
    missed_runs: u64,
    consecutive_failures: u64,
    last_duration_ms: u128,
    clock_anomaly: bool,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScheduleState {
    schema_version: u32,
    jobs: BTreeMap<String, JobState>,
}

#[derive(Debug, Serialize)]
pub struct JobStatus {
    pub id: String,
    pub kind: JobKind,
    pub enabled: bool,
    pub status: &'static str,
    pub last_attempt_unix: Option<u64>,
    pub last_success_unix: Option<u64>,
    pub last_result: Option<String>,
    pub last_error: Option<String>,
    pub last_destination: Option<String>,
    pub next_due_unix: u64,
    pub missed_runs: u64,
    pub consecutive_failures: u64,
    pub last_duration_ms: u128,
    pub clock_anomaly: bool,
}

#[derive(Debug, Serialize)]
pub struct StatusReport {
    pub schema_version: u32,
    pub now_unix: u64,
    pub jobs: Vec<JobStatus>,
}

struct Lock {
    path: PathBuf,
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn unix_now() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_secs())
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)
        .with_context(|| format!("cannot open {}", path.display()))?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "file exceeds bounded size limit"
    );
    Ok(bytes)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LENGTH
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn validate_config(config: &ScheduleConfig) -> Result<()> {
    ensure!(
        config.schema_version == SCHEMA_VERSION,
        "unsupported schedule schema version"
    );
    ensure!(!config.jobs.is_empty(), "schedule has no jobs");
    let mut ids = BTreeSet::new();
    for job in &config.jobs {
        ensure!(valid_id(&job.id), "invalid schedule job id");
        ensure!(ids.insert(job.id.as_str()), "duplicate schedule job id");
        ensure!(
            job.interval_seconds > 0 && job.interval_seconds <= MAX_INTERVAL_SECONDS,
            "invalid schedule interval"
        );
        ensure!(
            job.freshness_seconds > 0 && job.freshness_seconds <= 10 * MAX_INTERVAL_SECONDS,
            "invalid freshness interval"
        );
        ensure!(
            !job.repository.as_os_str().is_empty() && !job.backend.as_os_str().is_empty(),
            "schedule job paths must not be empty"
        );
        ensure!(
            !job.password_file.as_os_str().is_empty(),
            "schedule job password file must not be empty"
        );
        match job.kind {
            JobKind::Verify => {
                ensure!(
                    job.snapshot.is_none()
                        || job.snapshot.as_deref().is_some_and(|id| !id.is_empty()),
                    "invalid verify snapshot"
                );
            }
            JobKind::Restore => {
                ensure!(
                    job.snapshot.as_deref().is_some_and(|id| !id.is_empty()),
                    "restore job requires a snapshot"
                );
                ensure!(
                    job.destination_root.is_some(),
                    "restore job requires destination_root"
                );
            }
        }
    }
    Ok(())
}

fn load_config(path: &Path) -> Result<ScheduleConfig> {
    let bytes = read_bounded(path, MAX_CONFIG_BYTES)?;
    let config: ScheduleConfig =
        serde_json::from_slice(&bytes).context("invalid schedule configuration")?;
    validate_config(&config)?;
    Ok(config)
}

fn load_state(path: &Path) -> Result<ScheduleState> {
    if !path.try_exists()? {
        return Ok(ScheduleState {
            schema_version: SCHEMA_VERSION,
            jobs: BTreeMap::new(),
        });
    }
    let bytes = read_bounded(path, MAX_STATE_BYTES)?;
    let state: ScheduleState = serde_json::from_slice(&bytes).context("invalid schedule state")?;
    ensure!(
        state.schema_version == SCHEMA_VERSION,
        "unsupported schedule state version"
    );
    Ok(state)
}

fn state_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .context("cannot restrict schedule state permissions")?;
    }
    #[cfg(not(unix))]
    {
        // Windows owner ACLs are applied by the installation/profile boundary;
        // the state contains no password material.
        let _ = path;
    }
    Ok(())
}

fn write_state(path: &Path, state: &ScheduleState) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    ensure!(parent.is_dir(), "schedule state parent does not exist");
    let temp = parent.join(format!(
        ".{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    if temp.try_exists()? {
        fs::remove_file(&temp).context("cannot remove stale schedule temporary state")?;
    }
    let bytes = serde_json::to_vec_pretty(state).context("cannot encode schedule state")?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .context("cannot create schedule temporary state")?;
    file.write_all(&bytes)
        .context("cannot write schedule temporary state")?;
    file.sync_all()
        .context("cannot sync schedule temporary state")?;
    state_permissions(&temp)?;
    drop(file);
    fs::rename(&temp, path).context("cannot publish schedule state")?;
    Ok(())
}

fn acquire_lock(state_path: &Path) -> Result<Lock> {
    let lock_path = state_path.with_extension("lock");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .with_context(|| {
            format!(
                "schedule state is locked; another tick may be running ({})",
                lock_path.display()
            )
        })?;
    writeln!(file, "pid={} time={}", std::process::id(), unix_now()?)
        .context("cannot write schedule lock")?;
    file.sync_all().context("cannot sync schedule lock")?;
    Ok(Lock { path: lock_path })
}

fn classify(job: &JobSpec, state: &JobState, now: u64) -> &'static str {
    if !job.enabled {
        return "disabled";
    }
    if state.clock_anomaly {
        return "clock-anomaly";
    }
    match state.last_success_unix {
        None => "unknown",
        Some(success) if now < success => "clock-anomaly",
        Some(success) => {
            let age = now.saturating_sub(success);
            if age > job.freshness_seconds {
                "stale"
            } else if state.last_attempt_unix.is_some_and(|attempt| {
                state.last_result.as_deref() != Some("pass") && attempt >= success
            }) {
                "needs-attention"
            } else {
                "fresh"
            }
        }
    }
}

fn status_report(config: &ScheduleConfig, state: &ScheduleState, now: u64) -> StatusReport {
    let jobs = config
        .jobs
        .iter()
        .map(|job| {
            let job_state = state.jobs.get(&job.id).cloned().unwrap_or_default();
            JobStatus {
                id: job.id.clone(),
                kind: job.kind,
                enabled: job.enabled,
                status: classify(job, &job_state, now),
                last_attempt_unix: job_state.last_attempt_unix,
                last_success_unix: job_state.last_success_unix,
                last_result: job_state.last_result,
                last_error: job_state.last_error,
                last_destination: job_state.last_destination,
                next_due_unix: job_state.next_due_unix,
                missed_runs: job_state.missed_runs,
                consecutive_failures: job_state.consecutive_failures,
                last_duration_ms: job_state.last_duration_ms,
                clock_anomaly: job_state.clock_anomaly,
            }
        })
        .collect();
    StatusReport {
        schema_version: SCHEMA_VERSION,
        now_unix: now,
        jobs,
    }
}

fn truncate_error(output: &str) -> String {
    const LIMIT: usize = 512;
    let mut text = output.replace(['\r', '\n'], " ");
    if text.len() > LIMIT {
        text.truncate(LIMIT);
        text.push_str("...");
    }
    text
}

fn run_job(job: &JobSpec, now: u64) -> Result<Option<String>> {
    ensure!(
        job.repository.is_dir(),
        "scheduled repository does not exist"
    );
    ensure!(job.backend.is_file(), "scheduled backend does not exist");
    ensure!(
        job.password_file.is_file(),
        "scheduled password file does not exist"
    );
    let current = std::env::current_exe().context("cannot locate PeerVault executable")?;
    let mut command = std::process::Command::new(&current);
    let mut destination = None;
    match job.kind {
        JobKind::Verify => {
            command.args([
                "verify",
                "--repository",
                &job.repository.to_string_lossy(),
                "--backend",
                &job.backend.to_string_lossy(),
                "--password-file",
                &job.password_file.to_string_lossy(),
            ]);
            if let Some(snapshot) = &job.snapshot {
                command.args(["--snapshot", snapshot]);
            }
        }
        JobKind::Restore => {
            let root = job
                .destination_root
                .as_ref()
                .context("restore job requires destination_root")?;
            ensure!(root.is_dir(), "restore destination_root does not exist");
            let job_dir = root.join(&job.id);
            if !job_dir.try_exists()? {
                fs::create_dir(&job_dir).context("cannot create restore job directory")?;
            }
            let mut candidate = job_dir.join(now.to_string());
            let mut suffix = 0_u32;
            while candidate.try_exists()? {
                suffix = suffix
                    .checked_add(1)
                    .context("restore destination suffix exhausted")?;
                candidate = job_dir.join(format!("{now}-{suffix}"));
            }
            command.args([
                "restore",
                "--repository",
                &job.repository.to_string_lossy(),
                "--backend",
                &job.backend.to_string_lossy(),
                "--snapshot",
                job.snapshot
                    .as_deref()
                    .context("restore job requires snapshot")?,
                "--destination",
                &candidate.to_string_lossy(),
                "--password-file",
                &job.password_file.to_string_lossy(),
            ]);
            destination = Some(candidate.display().to_string());
        }
    }
    let (code, output) = bounded(&mut command, JOB_TIMEOUT, MAX_JOB_OUTPUT)
        .map_err(|error| anyhow::anyhow!("scheduled job failed to start: {error}"))?;
    if code != Some(0) {
        bail!(
            "scheduled job exited {:?}: {}",
            code,
            truncate_error(&output)
        );
    }
    let value: serde_json::Value =
        serde_json::from_str(&output).context("scheduled job returned invalid JSON")?;
    ensure!(
        value.get("result").and_then(|value| value.as_str()) == Some("pass"),
        "scheduled job did not report pass"
    );
    Ok(destination)
}

fn tick(config: &ScheduleConfig, state: &mut ScheduleState, now: u64) -> Result<()> {
    for job in &config.jobs {
        let job_state = state.jobs.entry(job.id.clone()).or_default();
        if job_state.next_due_unix == 0 {
            job_state.next_due_unix = now;
        }
        if !job.enabled {
            continue;
        }
        if now.saturating_add(2) < job_state.last_attempt_unix.unwrap_or(0) {
            job_state.clock_anomaly = true;
            continue;
        }
        if now < job_state.next_due_unix {
            continue;
        }
        if job_state.next_due_unix.saturating_add(job.interval_seconds) < now {
            let missed = (now - job_state.next_due_unix) / job.interval_seconds;
            job_state.missed_runs = job_state.missed_runs.saturating_add(missed);
        }
        job_state.clock_anomaly = false;
        job_state.last_attempt_unix = Some(now);
        let started = SystemTime::now();
        match run_job(job, now) {
            Ok(destination) => {
                job_state.last_success_unix = Some(now);
                job_state.last_result = Some("pass".to_owned());
                job_state.last_error = None;
                job_state.last_destination = destination;
                job_state.consecutive_failures = 0;
            }
            Err(error) => {
                job_state.last_result = Some("fail".to_owned());
                job_state.last_error = Some(truncate_error(&format!("{error:#}")));
                job_state.consecutive_failures = job_state.consecutive_failures.saturating_add(1);
            }
        }
        job_state.last_duration_ms = started.elapsed().unwrap_or_default().as_millis();
        job_state.next_due_unix = now.saturating_add(job.interval_seconds);
    }
    Ok(())
}

pub fn status(config_path: &Path, state_path: &Path) -> Result<StatusReport> {
    let config = load_config(config_path)?;
    let state = load_state(state_path)?;
    Ok(status_report(&config, &state, unix_now()?))
}

pub fn tick_once(config_path: &Path, state_path: &Path) -> Result<StatusReport> {
    let config = load_config(config_path)?;
    let now = unix_now()?;
    let _lock = acquire_lock(state_path)?;
    let mut state = load_state(state_path)?;
    tick(&config, &mut state, now)?;
    write_state(state_path, &state)?;
    Ok(status_report(&config, &state, now))
}

pub fn print_report(report: &StatusReport) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(report).context("cannot encode schedule status")?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(kind: JobKind) -> JobSpec {
        JobSpec {
            id: "test".to_owned(),
            kind,
            repository: PathBuf::from("repo"),
            backend: PathBuf::from("backend"),
            password_file: PathBuf::from("secret"),
            snapshot: Some("a".repeat(64)),
            destination_root: Some(PathBuf::from("restore")),
            interval_seconds: 60,
            freshness_seconds: 120,
            enabled: true,
        }
    }

    #[test]
    fn config_rejects_duplicate_ids_and_missing_restore_root() {
        let mut config = ScheduleConfig {
            schema_version: SCHEMA_VERSION,
            jobs: vec![job(JobKind::Verify), job(JobKind::Restore)],
        };
        config.jobs[1].id = "restore".to_owned();
        assert!(validate_config(&config).is_ok());
        config.jobs[1].id = "test".to_owned();
        assert!(validate_config(&config).is_err());
        config.jobs[1].id = "restore".to_owned();
        config.jobs[1].destination_root = None;
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn classify_distinguishes_unknown_fresh_stale_and_failure() {
        let job = job(JobKind::Verify);
        let mut state = JobState::default();
        assert_eq!(classify(&job, &state, 100), "unknown");
        state.last_success_unix = Some(100);
        state.last_attempt_unix = Some(100);
        state.last_result = Some("pass".to_owned());
        assert_eq!(classify(&job, &state, 150), "fresh");
        assert_eq!(classify(&job, &state, 300), "stale");
        state.last_result = Some("fail".to_owned());
        assert_eq!(classify(&job, &state, 150), "needs-attention");
        state.clock_anomaly = true;
        assert_eq!(classify(&job, &state, 150), "clock-anomaly");
    }

    #[test]
    fn tick_counts_missed_runs_and_schedules_one_execution() {
        let mut job = job(JobKind::Verify);
        job.repository = PathBuf::from("does-not-exist");
        job.enabled = true;
        let config = ScheduleConfig {
            schema_version: SCHEMA_VERSION,
            jobs: vec![job],
        };
        let mut state = ScheduleState {
            schema_version: SCHEMA_VERSION,
            jobs: BTreeMap::new(),
        };
        state.jobs.insert(
            "test".to_owned(),
            JobState {
                next_due_unix: 100,
                ..JobState::default()
            },
        );
        tick(&config, &mut state, 400).unwrap();
        assert_eq!(state.jobs["test"].missed_runs, 5);
        assert_eq!(state.jobs["test"].next_due_unix, 460);
        assert_eq!(state.jobs["test"].last_result.as_deref(), Some("fail"));
    }

    #[test]
    fn clock_rollback_is_recorded_without_execution() {
        let mut job = job(JobKind::Verify);
        job.enabled = true;
        let config = ScheduleConfig {
            schema_version: SCHEMA_VERSION,
            jobs: vec![job],
        };
        let mut state = ScheduleState {
            schema_version: SCHEMA_VERSION,
            jobs: BTreeMap::new(),
        };
        state.jobs.insert(
            "test".to_owned(),
            JobState {
                next_due_unix: 1_000,
                last_attempt_unix: Some(2_000),
                ..JobState::default()
            },
        );
        tick(&config, &mut state, 100).unwrap();
        assert!(state.jobs["test"].clock_anomaly);
    }
}
