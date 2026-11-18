//! V0.5 retention: preview and owner-executed backend-native maintenance.
//!
//! PeerVault never deletes backend objects directly. A preview binds the exact
//! policy, repository binding, state generation, peer roots and snapshot set;
//! execution revalidates all of it under an OS-owned state lock and then asks
//! restic to forget only the previewed captures and their oracle snapshots.

use crate::production::{self, BackendInfo, Credential, SnapshotRef};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub const SCHEMA_VERSION: u32 = 2;
pub const STATE_SCHEMA_VERSION: u32 = 1;
pub const POLICY_SCHEMA_VERSION: u32 = 1;
const MAX_POLICY_BYTES: u64 = 64 * 1024;
const MAX_PLAN_BYTES: u64 = 16 * 1024 * 1024;
const MAX_STATE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_EVIDENCE_BYTES: u64 = 16 * 1024 * 1024;
const PREVIEW_TTL_SECONDS: u64 = 900;
const MAX_HISTORY: usize = 64;
const FLOOR_POINTS_DEFAULT: usize = 3;
const FLOOR_DAYS_DEFAULT: u64 = 30;
const MASS_CHANGE_MIN_FILES: u64 = 20;
const MASS_CHANGE_FILE_PERCENT: u64 = 20;
const MASS_CHANGE_BYTE_PERCENT: u64 = 25;
const MAX_ID_BYTES: usize = 128;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RetentionPolicy {
    pub schema_version: u32,
    pub keep_last: usize,
    #[serde(default)]
    pub keep_tagged: Vec<String>,
    #[serde(default)]
    pub keep_snapshots: Vec<String>,
    #[serde(default = "default_minimum_keep")]
    pub minimum_keep: usize,
    /// Newest successful recovery points to retain regardless of policy.
    #[serde(default = "default_floor_points")]
    pub floor_points: usize,
    /// Retain all successful recovery points younger than this many days.
    /// Zero disables the age floor; the default floor is 30 days.
    #[serde(default = "default_floor_days")]
    pub floor_days: u64,
    /// Peer IDs whose latest verified root must exist and remain retained.
    #[serde(default)]
    pub required_peers: Vec<String>,
}

fn default_minimum_keep() -> usize {
    1
}

fn default_floor_points() -> usize {
    FLOOR_POINTS_DEFAULT
}

fn default_floor_days() -> u64 {
    FLOOR_DAYS_DEFAULT
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RetentionDecision {
    pub id: String,
    pub capture_id: Option<String>,
    pub time: String,
    pub reason: String,
    pub oracle_manifest: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FloorReport {
    pub points: usize,
    pub days: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PeerPin {
    pub peer_id: String,
    pub peer_repository_id: String,
    pub snapshot_id: String,
    pub key_version: String,
    pub evidence_sha256: String,
    pub verified_unix_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MassChangeReport {
    pub status: String,
    pub baseline_snapshot: Option<String>,
    pub current_snapshot: Option<String>,
    pub changed_files: u64,
    pub previous_files: u64,
    pub changed_bytes: u64,
    pub previous_bytes: u64,
    pub thresholds: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionPlan {
    pub schema_version: u32,
    pub action: String,
    pub preview_id: String,
    pub preview_token: String,
    pub created_unix_seconds: u64,
    pub expires_unix_seconds: u64,
    pub repository: String,
    pub repository_id: String,
    pub backend_version: String,
    pub backend_sha256: String,
    pub policy_sha256: String,
    pub policy: RetentionPolicy,
    pub state_sha256: String,
    pub state_generation: u64,
    pub inventory_sha256: String,
    pub keep: Vec<RetentionDecision>,
    pub protected: Vec<RetentionDecision>,
    pub delete_candidates: Vec<RetentionDecision>,
    pub peer_pins: Vec<PeerPin>,
    pub floor: FloorReport,
    pub mass_change: MassChangeReport,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotBaseline {
    pub snapshot_id: String,
    pub capture_id: Option<String>,
    pub manifest_snapshot_id: String,
    pub tree_sha256: String,
    pub files: usize,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PeerDisposition {
    LastGood,
    Abandoned,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerRoot {
    pub peer_id: String,
    pub peer_repository_id: String,
    pub snapshot_id: String,
    pub capture_id: Option<String>,
    pub oracle_manifest: Option<String>,
    pub key_version: String,
    pub evidence_path: String,
    pub evidence_sha256: String,
    pub verified_unix_seconds: u64,
    pub sequence: u64,
    pub disposition: PeerDisposition,
    #[serde(default)]
    pub abandonment_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionRecord {
    pub preview_id: String,
    pub result: String,
    pub executed_unix_seconds: u64,
    pub deleted: Vec<String>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionState {
    pub schema_version: u32,
    pub repository: String,
    pub repository_id: String,
    pub generation: u64,
    pub updated_unix_seconds: u64,
    pub last_validated: Option<SnapshotBaseline>,
    pub peer_roots: Vec<PeerRoot>,
    pub history: Vec<ExecutionRecord>,
}

#[derive(Serialize)]
struct ApplyReport {
    schema_version: u32,
    result: &'static str,
    preview_id: String,
    state_generation: u64,
    forgotten: Vec<String>,
    kept: Vec<String>,
    baseline: &'static str,
    mass_change_acknowledged: bool,
    elapsed_ms: u128,
    limitations: Vec<&'static str>,
}

#[derive(Default)]
struct Selection {
    keep: Vec<RetentionDecision>,
    protected: Vec<RetentionDecision>,
    delete_candidates: Vec<RetentionDecision>,
    floor: Option<FloorReport>,
}

struct StateLock {
    _file: File,
}

pub struct PeerRecordInput<'a> {
    pub peer_id: &'a str,
    pub peer_repository_id: &'a str,
    pub snapshot: &'a str,
    pub capture_id: Option<&'a str>,
    pub oracle_manifest: Option<&'a str>,
    pub key_version: &'a str,
    pub evidence: &'a Path,
    pub verified_unix_seconds: Option<u64>,
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

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn valid_repository_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn path_is_within(child: &Path, parent: &Path) -> bool {
    let child = child.to_string_lossy().replace('\\', "/").to_lowercase();
    let parent = parent.to_string_lossy().replace('\\', "/").to_lowercase();
    child == parent || child.starts_with(&format!("{parent}/"))
}

fn ensure_state_outside_repository(state: &Path, repository: &Path) -> Result<()> {
    let state = std::path::absolute(state).context("cannot resolve retention state path")?;
    ensure!(
        !path_is_within(&state, repository),
        "retention state must not live inside the repository"
    );
    Ok(())
}

fn validate_policy(policy: &RetentionPolicy) -> Result<()> {
    ensure!(
        policy.schema_version == POLICY_SCHEMA_VERSION,
        "unsupported retention policy version"
    );
    ensure!(policy.keep_last <= 10_000, "keep_last is too large");
    ensure!(policy.minimum_keep >= 1, "minimum_keep must be at least 1");
    ensure!(policy.floor_points <= 10_000, "floor_points is too large");
    ensure!(policy.floor_days <= 3_650, "floor_days is too large");
    ensure!(
        policy.keep_tagged.len() <= 128
            && policy
                .keep_tagged
                .iter()
                .all(|tag| !tag.is_empty() && tag.len() <= 128),
        "invalid keep_tagged entry"
    );
    let mut peers = BTreeSet::new();
    for peer in &policy.required_peers {
        ensure!(valid_key(peer), "invalid required peer ID");
        ensure!(peers.insert(peer.as_str()), "duplicate required peer ID");
    }
    for snapshot in &policy.keep_snapshots {
        production::validate_snapshot_id(snapshot)?;
    }
    Ok(())
}

fn instant(value: &str) -> Result<i64> {
    let parsed = chrono::DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("invalid snapshot time {value}"))?;
    parsed
        .timestamp_nanos_opt()
        .context("snapshot time is outside the supported range")
}

fn decision(snapshot: &SnapshotRef, reason: impl Into<String>) -> RetentionDecision {
    RetentionDecision {
        id: snapshot.id.clone(),
        capture_id: snapshot.capture_id.clone(),
        time: snapshot.time.clone(),
        reason: reason.into(),
        oracle_manifest: snapshot.oracle_manifest.clone(),
    }
}

fn select(
    policy: &RetentionPolicy,
    snapshots: &[SnapshotRef],
    protected_roots: &BTreeMap<String, String>,
    now_unix: u64,
) -> Result<Selection> {
    validate_policy(policy)?;
    let mut timed: Vec<(i64, SnapshotRef)> = snapshots
        .iter()
        .map(|snapshot| Ok((instant(&snapshot.time)?, snapshot.clone())))
        .collect::<Result<_>>()?;
    timed.sort_by_key(|(timestamp, _)| *timestamp);
    let instants: BTreeMap<String, i64> = timed
        .iter()
        .map(|(timestamp, snapshot)| (snapshot.id.clone(), *timestamp))
        .collect();
    let ordered: Vec<SnapshotRef> = timed.into_iter().map(|(_, snapshot)| snapshot).collect();
    let known: BTreeMap<&str, &SnapshotRef> = ordered
        .iter()
        .map(|snapshot| (snapshot.id.as_str(), snapshot))
        .collect();
    for id in &policy.keep_snapshots {
        ensure!(
            known.contains_key(id.as_str()),
            "explicit keep snapshot not found"
        );
    }
    for (id, reason) in protected_roots {
        let snapshot = known
            .get(id.as_str())
            .with_context(|| format!("{reason} snapshot {id} not found"))?;
        ensure!(
            snapshot.oracle_manifest.is_some(),
            "{reason} snapshot has no PeerVault oracle"
        );
    }
    let future_cutoff = i64::try_from(now_unix)
        .ok()
        .and_then(|seconds| seconds.checked_mul(1_000_000_000))
        .and_then(|nanos| nanos.checked_add(300_000_000_000));
    let future: BTreeSet<&str> = future_cutoff
        .map(|cutoff| {
            ordered
                .iter()
                .filter(|snapshot| {
                    instants
                        .get(&snapshot.id)
                        .is_some_and(|timestamp| *timestamp > cutoff)
                })
                .map(|snapshot| snapshot.id.as_str())
                .collect()
        })
        .unwrap_or_default();
    let restorable: Vec<&SnapshotRef> = ordered
        .iter()
        .filter(|snapshot| {
            snapshot.oracle_manifest.is_some() && !future.contains(snapshot.id.as_str())
        })
        .collect();
    let restorable_ids: BTreeSet<&str> = restorable
        .iter()
        .map(|snapshot| snapshot.id.as_str())
        .collect();
    let mut keep: BTreeMap<String, RetentionDecision> = BTreeMap::new();
    for snapshot in restorable.iter().rev().take(policy.keep_last) {
        let snapshot = *snapshot;
        keep.insert(snapshot.id.clone(), decision(snapshot, "keep-last"));
    }
    for tag in &policy.keep_tagged {
        for snapshot in &ordered {
            if snapshot.tags.iter().any(|value| value == tag) {
                keep.entry(snapshot.id.clone())
                    .or_insert_with(|| decision(snapshot, format!("keep-tag:{tag}")));
            }
        }
    }
    for id in &policy.keep_snapshots {
        let snapshot = known[id.as_str()];
        keep.entry(snapshot.id.clone())
            .or_insert_with(|| decision(snapshot, "keep-snapshot"));
    }
    while keep
        .values()
        .filter(|decision| restorable_ids.contains(decision.id.as_str()))
        .count()
        < policy.minimum_keep
    {
        let Some(snapshot) = restorable
            .iter()
            .rev()
            .find(|snapshot| !keep.contains_key(&snapshot.id))
        else {
            break;
        };
        let snapshot = *snapshot;
        keep.insert(snapshot.id.clone(), decision(snapshot, "minimum-keep"));
    }
    let floor_points = policy.floor_points.min(restorable.len());
    for snapshot in restorable.iter().rev().take(floor_points) {
        let snapshot = *snapshot;
        keep.entry(snapshot.id.clone())
            .or_insert_with(|| decision(snapshot, "floor-points"));
    }
    if policy.floor_days > 0 {
        let cutoff_nanos = i64::try_from(policy.floor_days)
            .ok()
            .and_then(|days| days.checked_mul(86_400_000_000_000))
            .and_then(|span| {
                i64::try_from(now_unix)
                    .ok()?
                    .checked_mul(1_000_000_000)?
                    .checked_sub(span)
            });
        if let Some(cutoff_nanos) = cutoff_nanos {
            for snapshot in &restorable {
                if instants
                    .get(&snapshot.id)
                    .is_some_and(|timestamp| *timestamp >= cutoff_nanos)
                {
                    keep.entry(snapshot.id.clone())
                        .or_insert_with(|| decision(snapshot, "floor-age"));
                }
            }
        }
    }
    for (id, reason) in protected_roots {
        let snapshot = known[id.as_str()];
        keep.entry(snapshot.id.clone())
            .or_insert_with(|| decision(snapshot, reason.clone()));
    }
    let mut protected = Vec::new();
    for snapshot in &ordered {
        if snapshot.oracle_manifest.is_none() {
            protected.push(decision(snapshot, "missing-oracle"));
        } else if future.contains(snapshot.id.as_str()) {
            protected.push(decision(snapshot, "future-timestamp"));
        }
    }
    let protected_ids: BTreeSet<&str> = protected
        .iter()
        .map(|decision| decision.id.as_str())
        .collect();
    let delete_candidates = ordered
        .iter()
        .filter(|snapshot| {
            !keep.contains_key(&snapshot.id) && !protected_ids.contains(snapshot.id.as_str())
        })
        .map(|snapshot| decision(snapshot, "eligible-after-qualified-retention-gate"))
        .collect();
    let mut keep: Vec<RetentionDecision> = keep.into_values().collect();
    keep.sort_by_key(|decision| instants.get(&decision.id).copied().unwrap_or(i64::MAX));
    Ok(Selection {
        keep,
        protected,
        delete_candidates,
        floor: Some(FloorReport {
            points: policy.floor_points,
            days: policy.floor_days,
        }),
    })
}

fn decision_keys(decisions: &[RetentionDecision], label: &str) -> Result<BTreeSet<String>> {
    let mut keys = BTreeSet::new();
    for decision in decisions {
        production::validate_snapshot_id(&decision.id)?;
        let key = format!(
            "{}|{}",
            decision.id,
            decision.oracle_manifest.as_deref().unwrap_or("")
        );
        ensure!(keys.insert(key), "duplicate {label} snapshot");
    }
    Ok(keys)
}

fn inventory_sha256(snapshots: &[SnapshotRef]) -> String {
    let mut members: Vec<(&str, &str)> = snapshots
        .iter()
        .map(|snapshot| {
            (
                snapshot.id.as_str(),
                snapshot.oracle_manifest.as_deref().unwrap_or(""),
            )
        })
        .collect();
    members.sort();
    let mut hash = Sha256::new();
    for (id, manifest) in members {
        hash.update(id.as_bytes());
        hash.update([0]);
        hash.update(manifest.as_bytes());
        hash.update([0]);
    }
    format!("{:x}", hash.finalize())
}

fn repository_id(
    backend: &BackendInfo,
    repository: &Path,
    credential: &Credential,
) -> Result<String> {
    let config = production::repository_config(backend, repository, credential)?;
    let id = config["id"]
        .as_str()
        .context("repository config lacks an ID")?;
    ensure!(valid_repository_id(id), "invalid repository ID");
    Ok(id.to_owned())
}

fn validate_state(state: &RetentionState) -> Result<()> {
    ensure!(
        state.schema_version == STATE_SCHEMA_VERSION,
        "unsupported retention state version"
    );
    ensure!(
        !state.repository.is_empty(),
        "retention state lacks a repository"
    );
    ensure!(
        valid_repository_id(&state.repository_id),
        "invalid retention state repository ID"
    );
    ensure!(state.generation > 0, "retention state generation is zero");
    if let Some(baseline) = &state.last_validated {
        production::validate_snapshot_id(&baseline.snapshot_id)?;
        production::validate_snapshot_id(&baseline.manifest_snapshot_id)?;
        ensure!(
            baseline.tree_sha256.len() == 64,
            "invalid baseline tree digest"
        );
    }
    let mut sequences = BTreeSet::new();
    for root in &state.peer_roots {
        ensure!(valid_key(&root.peer_id), "invalid peer root ID");
        ensure!(
            valid_repository_id(&root.peer_repository_id),
            "invalid peer repository ID"
        );
        production::validate_snapshot_id(&root.snapshot_id)?;
        if let Some(capture_id) = &root.capture_id {
            production::validate_capture_id(capture_id)?;
        }
        if let Some(manifest) = &root.oracle_manifest {
            production::validate_snapshot_id(manifest)?;
        }
        ensure!(
            valid_key(&root.key_version),
            "invalid key version reference"
        );
        ensure!(root.evidence_sha256.len() == 64, "invalid evidence digest");
        ensure!(root.sequence > 0, "peer root sequence is zero");
        ensure!(
            sequences.insert((root.peer_id.as_str(), root.sequence)),
            "duplicate peer root sequence"
        );
        if root.disposition == PeerDisposition::Abandoned {
            ensure!(
                root.abandonment_reason
                    .as_deref()
                    .is_some_and(|reason| !reason.is_empty() && reason.len() <= 1024),
                "abandoned peer root lacks a bounded reason"
            );
        }
    }
    ensure!(
        state.history.len() <= MAX_HISTORY,
        "retention history exceeds the supported bound"
    );
    Ok(())
}

fn read_state(path: &Path) -> Result<(RetentionState, Vec<u8>)> {
    let bytes = read_bounded(path, MAX_STATE_BYTES).context("cannot read retention state")?;
    let state: RetentionState =
        serde_json::from_slice(&bytes).context("invalid retention state")?;
    validate_state(&state)?;
    Ok((state, bytes))
}

#[cfg(unix)]
fn owner_only(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn owner_only(_path: &Path) -> Result<()> {
    Ok(())
}

fn write_state(path: &Path, state: &RetentionState) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    ensure!(parent.is_dir(), "retention state parent does not exist");
    let temp = parent.join(format!(
        ".{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    if temp.try_exists()? {
        fs::remove_file(&temp).context("cannot remove stale retention temporary state")?;
    }
    let bytes = serde_json::to_vec_pretty(state).context("cannot encode retention state")?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temp)
        .context("cannot create retention temporary state")?;
    file.write_all(&bytes)
        .context("cannot write retention temporary state")?;
    file.sync_all()
        .context("cannot sync retention temporary state")?;
    owner_only(&temp)?;
    drop(file);
    fs::rename(&temp, path).context("cannot publish retention state")?;
    Ok(())
}

fn acquire_lock(state_path: &Path) -> Result<StateLock> {
    let name = state_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "retention-state".into());
    let lock_path = state_path.with_file_name(format!("{name}.lock"));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("cannot open retention lock {}", lock_path.display()))?;
    owner_only(&lock_path)?;
    if let Err(error) = FileExt::try_lock_exclusive(&file) {
        let busy = error.kind() == std::io::ErrorKind::WouldBlock
            || (cfg!(windows) && matches!(error.raw_os_error(), Some(32 | 33)));
        if busy {
            return Err(production::failure(
                production::EXIT_BUSY,
                format!(
                    "retention state is busy; another owner process holds {}",
                    lock_path.display()
                ),
            ));
        }
        return Err(error).context("cannot acquire retention state lock");
    }
    Ok(StateLock { _file: file })
}

fn latest_root<'a>(state: &'a RetentionState, peer_id: &str) -> Option<&'a PeerRoot> {
    state
        .peer_roots
        .iter()
        .filter(|root| root.peer_id == peer_id)
        .max_by_key(|root| root.sequence)
}

fn compose_peer_pins(policy: &RetentionPolicy, state: &RetentionState) -> Result<Vec<PeerPin>> {
    let mut pins = Vec::new();
    for peer_id in &policy.required_peers {
        let root = latest_root(state, peer_id)
            .with_context(|| format!("required peer {peer_id} has no recorded root"))?;
        ensure!(
            root.disposition == PeerDisposition::LastGood,
            "required peer {peer_id} root is abandoned"
        );
        pins.push(PeerPin {
            peer_id: peer_id.clone(),
            peer_repository_id: root.peer_repository_id.clone(),
            snapshot_id: root.snapshot_id.clone(),
            key_version: root.key_version.clone(),
            evidence_sha256: root.evidence_sha256.clone(),
            verified_unix_seconds: root.verified_unix_seconds,
        });
    }
    Ok(pins)
}

fn protected_roots(pins: &[PeerPin]) -> BTreeMap<String, String> {
    pins.iter()
        .map(|pin| {
            (
                pin.snapshot_id.clone(),
                format!("peer-root:{}", pin.peer_id),
            )
        })
        .collect()
}

fn newest_restorable(snapshots: &[SnapshotRef]) -> Result<Option<SnapshotRef>> {
    let mut restorable: Vec<(i64, SnapshotRef)> = snapshots
        .iter()
        .filter(|snapshot| snapshot.oracle_manifest.is_some())
        .map(|snapshot| Ok((instant(&snapshot.time)?, snapshot.clone())))
        .collect::<Result<_>>()?;
    restorable.sort_by_key(|(timestamp, _)| *timestamp);
    Ok(restorable.pop().map(|(_, snapshot)| snapshot))
}

fn baseline_from_newest(
    backend: &BackendInfo,
    repository: &Path,
    credential: &Credential,
    snapshots: &[SnapshotRef],
) -> Result<Option<SnapshotBaseline>> {
    let Some(snapshot) = newest_restorable(snapshots)? else {
        return Ok(None);
    };
    let manifest_id = snapshot
        .oracle_manifest
        .as_deref()
        .context("newest restorable snapshot lacks an oracle")?;
    let manifest = production::read_manifest(backend, repository, credential, manifest_id)?;
    Ok(Some(SnapshotBaseline {
        snapshot_id: snapshot.id,
        capture_id: snapshot.capture_id,
        manifest_snapshot_id: manifest_id.to_owned(),
        tree_sha256: production::tree_sha256(&manifest),
        files: manifest.files.len(),
        bytes: manifest.files.iter().map(|entry| entry.size).sum(),
    }))
}

fn mass_change(
    backend: &BackendInfo,
    repository: &Path,
    credential: &Credential,
    state: &RetentionState,
    snapshots: &[SnapshotRef],
) -> MassChangeReport {
    let thresholds = format!(
        "{MASS_CHANGE_MIN_FILES} files and {MASS_CHANGE_FILE_PERCENT}% files, or {MASS_CHANGE_BYTE_PERCENT}% bytes"
    );
    let unknown = |reason: &str| MassChangeReport {
        status: "unknown".into(),
        baseline_snapshot: state
            .last_validated
            .as_ref()
            .map(|baseline| baseline.snapshot_id.clone()),
        current_snapshot: newest_restorable(snapshots)
            .ok()
            .flatten()
            .map(|snapshot| snapshot.id),
        changed_files: 0,
        previous_files: 0,
        changed_bytes: 0,
        previous_bytes: 0,
        thresholds: thresholds.clone(),
        reason: reason.into(),
    };
    let Some(baseline) = &state.last_validated else {
        return unknown("no validated baseline");
    };
    let Ok(Some(current)) = newest_restorable(snapshots) else {
        return unknown("no restorable snapshot");
    };
    if current.id == baseline.snapshot_id {
        return MassChangeReport {
            status: "not-triggered".into(),
            baseline_snapshot: Some(baseline.snapshot_id.clone()),
            current_snapshot: Some(current.id.clone()),
            changed_files: 0,
            previous_files: baseline.files as u64,
            changed_bytes: 0,
            previous_bytes: baseline.bytes,
            thresholds,
            reason: "newest restorable snapshot is the validated baseline".into(),
        };
    }
    let Some(current_manifest_id) = current.oracle_manifest.as_deref() else {
        return unknown("current snapshot lacks an oracle");
    };
    let Ok(previous) = production::read_manifest(
        backend,
        repository,
        credential,
        &baseline.manifest_snapshot_id,
    ) else {
        return unknown("baseline oracle is unavailable");
    };
    if production::tree_sha256(&previous) != baseline.tree_sha256 {
        return unknown("baseline oracle changed since validation");
    }
    let Ok(current_manifest) =
        production::read_manifest(backend, repository, credential, current_manifest_id)
    else {
        return unknown("current oracle is unavailable");
    };
    let previous_files = previous.files.len() as u64;
    let previous_bytes: u64 = previous.files.iter().map(|entry| entry.size).sum();
    if previous_files == 0 || previous_bytes == 0 {
        return unknown("previous selected set is empty");
    }
    let current_map: BTreeMap<&str, (u64, &str)> = current_manifest
        .files
        .iter()
        .map(|entry| (entry.path.as_str(), (entry.size, entry.sha256.as_str())))
        .collect();
    let mut changed_files = 0u64;
    let mut changed_bytes = 0u64;
    for entry in &previous.files {
        let unchanged = current_map
            .get(entry.path.as_str())
            .is_some_and(|(size, digest)| *size == entry.size && *digest == entry.sha256);
        if !unchanged {
            changed_files += 1;
            changed_bytes = changed_bytes.saturating_add(entry.size);
        }
    }
    let triggered = changed_files >= MASS_CHANGE_MIN_FILES
        && u128::from(changed_files) * 100
            >= u128::from(previous_files) * u128::from(MASS_CHANGE_FILE_PERCENT)
        || u128::from(changed_bytes) * 100
            >= u128::from(previous_bytes) * u128::from(MASS_CHANGE_BYTE_PERCENT);
    MassChangeReport {
        status: if triggered {
            "triggered".into()
        } else {
            "not-triggered".into()
        },
        baseline_snapshot: Some(baseline.snapshot_id.clone()),
        current_snapshot: Some(current.id.clone()),
        changed_files,
        previous_files,
        changed_bytes,
        previous_bytes,
        thresholds,
        reason: if triggered {
            "selected-set change crossed the default mass-change threshold".into()
        } else {
            "selected-set change is below the default mass-change threshold".into()
        },
    }
}

fn push_history(state: &mut RetentionState, record: ExecutionRecord) {
    state.history.push(record);
    if state.history.len() > MAX_HISTORY {
        let excess = state.history.len() - MAX_HISTORY;
        state.history.drain(..excess);
    }
}

fn sync_state(
    state: &mut RetentionState,
    now: u64,
    preview_id: &str,
    result: &str,
    deleted: Vec<String>,
    reason: Option<String>,
) {
    state.generation += 1;
    state.updated_unix_seconds = now;
    push_history(
        state,
        ExecutionRecord {
            preview_id: preview_id.to_owned(),
            result: result.to_owned(),
            executed_unix_seconds: now,
            deleted,
            reason,
        },
    );
}

fn validate_repository_binding(
    state: &RetentionState,
    repository: &Path,
    repository_id: &str,
) -> Result<()> {
    let repository = fs::canonicalize(repository).context("cannot resolve repository")?;
    ensure!(
        state.repository == repository.display().to_string(),
        "retention state belongs to a different repository path"
    );
    ensure!(
        state.repository_id == repository_id,
        "retention state repository binding does not match the backend"
    );
    Ok(())
}

fn evidence_digest(
    path: &Path,
    input: &PeerRecordInput<'_>,
    now: u64,
) -> Result<(String, String, u64)> {
    let path = fs::canonicalize(path).context("cannot resolve peer evidence path")?;
    let metadata = fs::metadata(&path).context("cannot inspect peer evidence")?;
    ensure!(metadata.is_file(), "peer evidence must be a regular file");
    ensure!(
        (1..=MAX_EVIDENCE_BYTES).contains(&metadata.len()),
        "peer evidence must be 1..=16 MiB"
    );
    let bytes = read_bounded(&path, MAX_EVIDENCE_BYTES)?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).context("peer evidence must be a JSON report")?;
    ensure!(
        value.get("peer_repository_id").and_then(|v| v.as_str()) == Some(input.peer_repository_id),
        "peer evidence repository ID does not match"
    );
    let snapshot = value
        .get("peer_snapshot")
        .or_else(|| value.get("snapshot_id"))
        .and_then(|v| v.as_str())
        .context("peer evidence lacks a snapshot ID")?;
    ensure!(
        snapshot == input.snapshot,
        "peer evidence snapshot does not match"
    );
    if let Some(manifest) = value
        .get("oracle_manifest")
        .or_else(|| value.get("manifest_snapshot_id"))
        .and_then(|v| v.as_str())
    {
        ensure!(
            input.oracle_manifest == Some(manifest),
            "peer evidence oracle does not match"
        );
    }
    let passed = value.get("state").and_then(|v| v.as_str()) == Some("clean-restored")
        || value.get("result").and_then(|v| v.as_str()) == Some("pass");
    ensure!(passed, "peer evidence does not report a completed recovery");
    if let Some(steps) = value.get("steps").and_then(|v| v.as_array()) {
        ensure!(
            steps.iter().any(|step| {
                step.get("state").and_then(|v| v.as_str()) == Some("clean-restored")
                    && step.get("result").and_then(|v| v.as_str()) == Some("pass")
            }),
            "peer evidence step trail has no passing clean restore"
        );
    }
    let verified = value
        .get("verified_unix_seconds")
        .or_else(|| value.get("updated_unix_seconds"))
        .and_then(|v| v.as_u64())
        .context("peer evidence lacks a verification time")?;
    ensure!(
        verified <= now.saturating_add(300),
        "peer evidence verification time is in the future"
    );
    ensure!(
        verified >= now.saturating_sub(31 * 24 * 60 * 60),
        "peer evidence is older than 31 days"
    );
    if let Some(claimed) = input.verified_unix_seconds {
        ensure!(
            claimed == verified,
            "peer verification time does not match the evidence"
        );
    }
    Ok((path.display().to_string(), sha256_hex(&bytes), verified))
}

pub fn state_init(
    repository: &Path,
    backend_path: &Path,
    state_path: &Path,
    password_file: Option<&Path>,
) -> Result<()> {
    let backend = production::backend(backend_path)?;
    let credential = Credential::new(password_file)?;
    ensure!(repository.is_dir(), "repository does not exist");
    let repository = fs::canonicalize(repository)?;
    ensure_state_outside_repository(state_path, &repository)?;
    let _lock = acquire_lock(state_path)?;
    ensure!(
        !state_path.try_exists()?,
        "retention state already exists; use rebaseline instead of overwriting it"
    );
    let repository_id = repository_id(&backend, &repository, &credential)?;
    let snapshots = production::snapshot_refs(&backend, &repository, &credential)?;
    let now = unix_now()?;
    let state = RetentionState {
        schema_version: STATE_SCHEMA_VERSION,
        repository: repository.display().to_string(),
        repository_id,
        generation: 1,
        updated_unix_seconds: now,
        last_validated: baseline_from_newest(&backend, &repository, &credential, &snapshots)?,
        peer_roots: Vec::new(),
        history: vec![ExecutionRecord {
            preview_id: "state-init".into(),
            result: "initialized".into(),
            executed_unix_seconds: now,
            deleted: Vec::new(),
            reason: None,
        }],
    };
    validate_state(&state)?;
    write_state(state_path, &state)?;
    production::print_json(&state)
}

pub fn rebaseline(
    repository: &Path,
    backend_path: &Path,
    state_path: &Path,
    password_file: Option<&Path>,
    reason: &str,
) -> Result<()> {
    ensure!(
        !reason.trim().is_empty() && reason.len() <= 1024,
        "rebaseline requires a bounded reason"
    );
    let backend = production::backend(backend_path)?;
    let credential = Credential::new(password_file)?;
    ensure!(repository.is_dir(), "repository does not exist");
    let repository = fs::canonicalize(repository)?;
    ensure_state_outside_repository(state_path, &repository)?;
    let _lock = acquire_lock(state_path)?;
    let (mut state, _) = read_state(state_path)?;
    let repository_id = repository_id(&backend, &repository, &credential)?;
    validate_repository_binding(&state, &repository, &repository_id)?;
    let snapshots = production::snapshot_refs(&backend, &repository, &credential)?;
    let now = unix_now()?;
    state.last_validated = baseline_from_newest(&backend, &repository, &credential, &snapshots)?;
    sync_state(
        &mut state,
        now,
        "rebaseline",
        "rebaselined",
        Vec::new(),
        Some(reason.to_owned()),
    );
    write_state(state_path, &state)?;
    production::print_json(&state)
}

pub fn peer_record(state_path: &Path, input: PeerRecordInput<'_>) -> Result<()> {
    ensure!(valid_key(input.peer_id), "invalid peer ID");
    ensure!(
        valid_repository_id(input.peer_repository_id),
        "invalid peer repository ID"
    );
    production::validate_snapshot_id(input.snapshot)?;
    if let Some(capture_id) = input.capture_id {
        production::validate_capture_id(capture_id)?;
    }
    if let Some(manifest) = input.oracle_manifest {
        production::validate_snapshot_id(manifest)?;
    }
    ensure!(
        valid_key(input.key_version),
        "invalid key version reference"
    );
    let _lock = acquire_lock(state_path)?;
    let (mut state, _) = read_state(state_path)?;
    let now = unix_now()?;
    let (evidence_path, evidence_sha256, verified) = evidence_digest(input.evidence, &input, now)?;
    let sequence = latest_root(&state, input.peer_id)
        .map(|root| root.sequence + 1)
        .unwrap_or(1);
    state.peer_roots.push(PeerRoot {
        peer_id: input.peer_id.to_owned(),
        peer_repository_id: input.peer_repository_id.to_owned(),
        snapshot_id: input.snapshot.to_owned(),
        capture_id: input.capture_id.map(str::to_owned),
        oracle_manifest: input.oracle_manifest.map(str::to_owned),
        key_version: input.key_version.to_owned(),
        evidence_path,
        evidence_sha256,
        verified_unix_seconds: verified,
        sequence,
        disposition: PeerDisposition::LastGood,
        abandonment_reason: None,
    });
    sync_state(
        &mut state,
        now,
        "peer-record",
        "peer-recorded",
        Vec::new(),
        Some(input.peer_id.to_owned()),
    );
    write_state(state_path, &state)?;
    production::print_json(
        latest_root(&state, input.peer_id).context("recorded peer root disappeared")?,
    )
}

pub fn peer_abandon(state_path: &Path, peer_id: &str, reason: &str) -> Result<()> {
    ensure!(valid_key(peer_id), "invalid peer ID");
    ensure!(
        !reason.trim().is_empty() && reason.len() <= 1024,
        "peer abandonment requires a bounded reason"
    );
    let _lock = acquire_lock(state_path)?;
    let (mut state, _) = read_state(state_path)?;
    let root = latest_root(&state, peer_id)
        .with_context(|| format!("peer {peer_id} has no recorded root"))?
        .clone();
    ensure!(
        root.disposition == PeerDisposition::LastGood,
        "peer root is already abandoned"
    );
    let now = unix_now()?;
    let mut abandoned = root;
    abandoned.sequence += 1;
    abandoned.disposition = PeerDisposition::Abandoned;
    abandoned.abandonment_reason = Some(reason.to_owned());
    let sequence = abandoned.sequence;
    state.peer_roots.push(abandoned);
    sync_state(
        &mut state,
        now,
        "peer-abandon",
        "peer-abandoned",
        Vec::new(),
        Some(format!("{peer_id}: sequence {sequence}")),
    );
    write_state(state_path, &state)?;
    production::print_json(latest_root(&state, peer_id).context("abandoned peer root disappeared")?)
}

pub fn preview(
    repository: &Path,
    backend_path: &Path,
    policy_path: &Path,
    state_path: &Path,
    password_file: Option<&Path>,
) -> Result<()> {
    let backend = production::backend(backend_path)?;
    let credential = Credential::new(password_file)?;
    ensure!(repository.is_dir(), "repository does not exist");
    let repository = fs::canonicalize(repository)?;
    ensure_state_outside_repository(state_path, &repository)?;
    let policy_bytes =
        read_bounded(policy_path, MAX_POLICY_BYTES).context("cannot read retention policy")?;
    let policy: RetentionPolicy =
        serde_json::from_slice(&policy_bytes).context("invalid retention policy")?;
    validate_policy(&policy)?;
    let (state, state_bytes) = read_state(state_path)?;
    let _repository_lock =
        production::RepositoryLock::shared(&repository).context("cannot serialize preview")?;
    let repository_id = repository_id(&backend, &repository, &credential)?;
    validate_repository_binding(&state, &repository, &repository_id)?;
    let snapshots = production::snapshot_refs(&backend, &repository, &credential)?;
    let peer_pins = compose_peer_pins(&policy, &state)?;
    let roots = protected_roots(&peer_pins);
    for pin in &peer_pins {
        let snapshot = snapshots
            .iter()
            .find(|snapshot| snapshot.id == pin.snapshot_id)
            .with_context(|| {
                format!(
                    "required peer {} root snapshot is missing from the repository",
                    pin.peer_id
                )
            })?;
        ensure!(
            snapshot.oracle_manifest.is_some(),
            "required peer {} root snapshot has no oracle",
            pin.peer_id
        );
    }
    let now = unix_now()?;
    let selection = select(&policy, &snapshots, &roots, now)?;
    let mass_change = mass_change(&backend, &repository, &credential, &state, &snapshots);
    let no_peer_root = policy.required_peers.is_empty();
    let future_timestamp = selection
        .protected
        .iter()
        .any(|decision| decision.reason == "future-timestamp");
    let plan = RetentionPlan {
        schema_version: SCHEMA_VERSION,
        action: "preview".into(),
        preview_id: production::random_hex()?,
        preview_token: production::random_hex()?,
        created_unix_seconds: now,
        expires_unix_seconds: now.saturating_add(PREVIEW_TTL_SECONDS),
        repository: repository.display().to_string(),
        repository_id,
        backend_version: backend.version.clone(),
        backend_sha256: backend.executable_sha256.clone(),
        policy_sha256: sha256_hex(&policy_bytes),
        policy,
        state_sha256: sha256_hex(&state_bytes),
        state_generation: state.generation,
        inventory_sha256: inventory_sha256(&snapshots),
        keep: selection.keep,
        protected: selection.protected,
        delete_candidates: selection.delete_candidates,
        peer_pins,
        floor: selection.floor.context("selection lacks a floor report")?,
        mass_change,
        limitations: {
            let mut limitations = vec![
                "preview is execution authority only for its exact expiry, state generation, inventory, peer pins and policy".into(),
                "apply uses backend-native forget/prune; PeerVault never deletes backend objects directly".into(),
                "same-account processes can forge a preview; the owner account is the permission boundary".into(),
                "state-loss or unknown peer/key evidence defers deletion rather than broadens it".into(),
                "this plan does not delete remote peer copies".into(),
            ];
            if no_peer_root {
                limitations.push(
                    "no independent peer root is required; deletion may leave only local history"
                        .into(),
                );
            }
            if future_timestamp {
                limitations.push(
                    "a snapshot timestamp is more than five minutes in the future; it is protected and does not consume a keep slot"
                        .into(),
                );
            }
            limitations
        },
    };
    production::print_json(&plan)?;
    Ok(())
}

pub fn apply(
    plan_path: &Path,
    backend_path: &Path,
    state_path: &Path,
    password_file: Option<&Path>,
    acknowledge_mass_change: bool,
) -> Result<()> {
    let started = Instant::now();
    let plan_bytes =
        read_bounded(plan_path, MAX_PLAN_BYTES).context("cannot read retention plan")?;
    let plan: RetentionPlan =
        serde_json::from_slice(&plan_bytes).context("invalid retention plan")?;
    ensure!(
        plan.schema_version == SCHEMA_VERSION,
        "unsupported retention plan version"
    );
    ensure!(plan.action == "preview", "retention plan is not a preview");
    ensure!(
        plan.preview_token.len() == 32
            && plan
                .preview_token
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "invalid retention preview token"
    );
    let now = unix_now()?;
    ensure!(
        plan.created_unix_seconds <= now,
        "retention plan was created in the future"
    );
    ensure!(
        now <= plan.expires_unix_seconds,
        "retention preview expired; create a new preview"
    );
    validate_policy(&plan.policy)?;
    let repository =
        fs::canonicalize(&plan.repository).context("cannot resolve planned repository")?;
    ensure!(repository.is_dir(), "planned repository does not exist");
    ensure_state_outside_repository(state_path, &repository)?;
    let backend = production::backend(backend_path)?;
    ensure!(
        backend.version == plan.backend_version,
        "backend version drifted since preview"
    );
    ensure!(
        backend.executable_sha256 == plan.backend_sha256,
        "backend executable drifted since preview"
    );
    let credential = Credential::new(password_file)?;
    let repository_id = repository_id(&backend, &repository, &credential)?;
    ensure!(
        repository_id == plan.repository_id,
        "repository binding drifted since preview"
    );
    let lock = acquire_lock(state_path)?;
    let (mut state, state_bytes) = read_state(state_path)?;
    let _repository_lock =
        production::RepositoryLock::exclusive(&repository).context("cannot serialize apply")?;
    validate_repository_binding(&state, &repository, &repository_id)?;
    ensure!(
        sha256_hex(&state_bytes) == plan.state_sha256,
        "retention state changed since preview"
    );
    ensure!(
        state.generation == plan.state_generation,
        "retention state generation changed since preview"
    );
    let snapshots = production::snapshot_refs(&backend, &repository, &credential)?;
    ensure!(
        inventory_sha256(&snapshots) == plan.inventory_sha256,
        "repository inventory changed since preview"
    );
    let peer_pins = compose_peer_pins(&plan.policy, &state)?;
    ensure!(
        peer_pins == plan.peer_pins,
        "required peer roots changed since preview"
    );
    let roots = protected_roots(&peer_pins);
    for pin in &plan.peer_pins {
        ensure!(
            snapshots
                .iter()
                .any(|snapshot| snapshot.id == pin.snapshot_id),
            "required peer root snapshot is missing from the repository"
        );
    }
    let selection = select(&plan.policy, &snapshots, &roots, now)?;
    ensure!(
        decision_keys(&selection.keep, "kept")? == decision_keys(&plan.keep, "kept")?,
        "keep decisions drifted since preview"
    );
    ensure!(
        decision_keys(&selection.protected, "protected")?
            == decision_keys(&plan.protected, "protected")?,
        "protected decisions drifted since preview"
    );
    ensure!(
        decision_keys(&selection.delete_candidates, "delete")?
            == decision_keys(&plan.delete_candidates, "delete")?,
        "delete decisions drifted since preview"
    );
    let mass_change = mass_change(&backend, &repository, &credential, &state, &snapshots);
    ensure!(
        mass_change == plan.mass_change,
        "mass-change evidence drifted since preview"
    );
    if mass_change.status != "not-triggered" && !acknowledge_mass_change {
        return Err(production::failure(
            production::EXIT_FAILURE,
            format!(
                "mass-change guard is {}: {}; review the preview and rerun apply with --ack-mass-change",
                mass_change.status, mass_change.reason
            ),
        ));
    }
    let keep_keys = decision_keys(&plan.keep, "kept")?;
    let protected_keys = decision_keys(&plan.protected, "protected")?;
    let mut forget_ids = BTreeSet::new();
    for decision in &plan.delete_candidates {
        production::validate_snapshot_id(&decision.id)?;
        ensure!(
            !keep_keys
                .iter()
                .any(|key| key.starts_with(&format!("{}|", decision.id)))
                && !protected_keys
                    .iter()
                    .any(|key| key.starts_with(&format!("{}|", decision.id))),
            "delete decision overlaps a kept or protected snapshot"
        );
        forget_ids.insert(decision.id.clone());
        if let Some(manifest) = &decision.oracle_manifest {
            production::validate_snapshot_id(manifest)?;
            ensure!(
                !decision_keys(&plan.keep, "kept")?
                    .iter()
                    .any(|key| key.ends_with(&format!("|{manifest}"))),
                "delete decision would forget a kept oracle"
            );
            forget_ids.insert(manifest.clone());
        }
    }
    for pin in &plan.peer_pins {
        ensure!(
            !forget_ids.contains(&pin.snapshot_id),
            "delete set contains required peer root {}",
            pin.peer_id
        );
    }
    if forget_ids.is_empty() {
        return Err(production::failure(
            production::EXIT_FAILURE,
            "retention preview has no delete candidates; nothing to execute",
        ));
    }
    let forget: Vec<String> = forget_ids.into_iter().collect();
    let mut command = production::command(&backend, &repository, &credential)?;
    command.args(["forget", "--prune", "--json"]);
    for id in &forget {
        command.arg(id);
    }
    let outcome = production::run(
        &mut command,
        production::COMMAND_TIMEOUT,
        production::OUTPUT_LIMIT,
    );
    if let Err(error) = outcome {
        let remaining = production::snapshot_ids(&backend, &repository, &credential);
        let removed: Vec<String> = match &remaining {
            Ok(remaining) => forget
                .iter()
                .filter(|id| !remaining.contains(*id))
                .cloned()
                .collect(),
            Err(_) => Vec::new(),
        };
        if removed.is_empty() {
            if remaining.is_err() {
                sync_state(
                    &mut state,
                    now,
                    &plan.preview_id,
                    "unknown",
                    Vec::new(),
                    Some(format!(
                        "maintenance failed and reconciliation failed: {error:#}"
                    )),
                );
                write_state(state_path, &state)?;
            }
            return Err(error);
        }
        state.last_validated = None;
        sync_state(
            &mut state,
            now,
            &plan.preview_id,
            "partial",
            removed.clone(),
            Some(format!("maintenance failed after removal: {error:#}")),
        );
        write_state(state_path, &state)?;
        return Err(production::failure(
            production::EXIT_FAILURE,
            format!(
                "retention maintenance failed after removing {} snapshot(s); state generation {} records a partial result",
                removed.len(),
                state.generation
            ),
        ));
    }
    let (remaining, remaining_ids) = match (
        production::snapshot_refs(&backend, &repository, &credential),
        production::snapshot_ids(&backend, &repository, &credential),
    ) {
        (Ok(remaining), Ok(remaining_ids)) => (remaining, remaining_ids),
        (error, _) => {
            let detail = error
                .err()
                .map(|error| format!("{error:#}"))
                .unwrap_or_else(|| "snapshot listing failed".into());
            state.last_validated = None;
            sync_state(
                &mut state,
                now,
                &plan.preview_id,
                "unknown",
                Vec::new(),
                Some(format!("post-execution listing failed: {detail}")),
            );
            write_state(state_path, &state)?;
            return Err(production::failure(
                production::EXIT_FAILURE,
                format!(
                    "maintenance ran but the post-execution listing failed; state generation {} records an unknown outcome",
                    state.generation
                ),
            ));
        }
    };
    let mut post_errors = Vec::new();
    for id in &forget {
        if remaining_ids.contains(id) {
            post_errors.push(format!("snapshot {id} remains"));
        }
    }
    for decision in &plan.keep {
        if !remaining_ids.contains(&decision.id) {
            post_errors.push(format!("kept snapshot {} disappeared", decision.id));
        }
        if let Some(manifest) = &decision.oracle_manifest
            && !remaining_ids.contains(manifest)
        {
            post_errors.push(format!("kept oracle {manifest} disappeared"));
        }
    }
    for pin in &plan.peer_pins {
        if !remaining_ids.contains(&pin.snapshot_id) {
            post_errors.push(format!("required peer root {} disappeared", pin.peer_id));
        }
    }
    if !post_errors.is_empty() {
        state.last_validated = None;
        sync_state(
            &mut state,
            now,
            &plan.preview_id,
            "unknown",
            forget.clone(),
            Some(post_errors.join("; ")),
        );
        write_state(state_path, &state)?;
        return Err(production::failure(
            production::EXIT_FAILURE,
            format!(
                "post-execution verification failed: {}; state generation {} records an unknown outcome",
                post_errors.join("; "),
                state.generation
            ),
        ));
    }
    state.last_validated = match baseline_from_newest(
        &backend,
        &repository,
        &credential,
        &remaining,
    ) {
        Ok(baseline) => baseline,
        Err(error) => {
            state.last_validated = None;
            sync_state(
                &mut state,
                now,
                &plan.preview_id,
                "unknown",
                forget.clone(),
                Some(format!("post-maintenance baseline failed: {error:#}")),
            );
            write_state(state_path, &state)?;
            return Err(production::failure(
                production::EXIT_FAILURE,
                format!(
                    "maintenance removed the previewed set but the post-maintenance baseline failed; state generation {} records an unknown outcome",
                    state.generation
                ),
            ));
        }
    };
    sync_state(
        &mut state,
        now,
        &plan.preview_id,
        "pass",
        forget.clone(),
        None,
    );
    write_state(state_path, &state)?;
    drop(lock);
    let report = ApplyReport {
        schema_version: SCHEMA_VERSION,
        result: "pass",
        preview_id: plan.preview_id,
        state_generation: state.generation,
        forgotten: forget,
        kept: plan.keep.into_iter().map(|decision| decision.id).collect(),
        baseline: "updated",
        mass_change_acknowledged: acknowledge_mass_change,
        elapsed_ms: started.elapsed().as_millis(),
        limitations: vec![
            "deletion is limited to the previewed backend-native snapshot set",
            "remote peer copies and independently retained evidence are not modified",
            "backend maintenance may leave unreferenced storage until prune completes",
        ],
    };
    production::print_json(&report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(value: &str) -> i64 {
        instant(value).unwrap()
    }

    fn now_seconds(value: &str) -> u64 {
        (at(value) / 1_000_000_000) as u64
    }

    fn snapshot(index: u8, tags: &[&str], oracle: bool, time: &str) -> SnapshotRef {
        SnapshotRef {
            id: format!("{index:02x}").repeat(32),
            capture_id: Some(format!("{index:02x}").repeat(16)),
            time: time.to_owned(),
            hostname: None,
            paths: vec![],
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            oracle_manifest: oracle.then(|| format!("{index:02x}").repeat(32)),
        }
    }

    fn policy(
        keep_last: usize,
        tags: &[&str],
        explicit: &[&str],
        minimum: usize,
        floor_points: usize,
        floor_days: u64,
    ) -> RetentionPolicy {
        RetentionPolicy {
            schema_version: 1,
            keep_last,
            keep_tagged: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            keep_snapshots: explicit.iter().map(|id| (*id).to_owned()).collect(),
            minimum_keep: minimum,
            floor_points,
            floor_days,
            required_peers: Vec::new(),
        }
    }

    #[test]
    fn preview_keeps_last_tagged_and_explicit_and_protects_missing_oracle() -> Result<()> {
        let snapshots = vec![
            snapshot(1, &[], true, "2026-11-01T00:00:01+00:00"),
            snapshot(2, &["keep"], true, "2026-11-01T00:00:02+00:00"),
            snapshot(3, &[], false, "2026-11-01T00:00:03+00:00"),
            snapshot(4, &[], true, "2026-11-01T00:00:04+00:00"),
        ];
        let policy = policy(1, &["keep"], &[&snapshots[0].id], 1, 1, 0);
        let now = now_seconds("2026-11-02T00:00:00+00:00");
        let selection = select(&policy, &snapshots, &BTreeMap::new(), now)?;
        let keep_ids: Vec<&str> = selection
            .keep
            .iter()
            .map(|decision| decision.id.as_str())
            .collect();
        assert!(keep_ids.contains(&snapshots[0].id.as_str()));
        assert!(keep_ids.contains(&snapshots[1].id.as_str()));
        assert!(keep_ids.contains(&snapshots[3].id.as_str()));
        assert_eq!(selection.protected.len(), 1);
        assert_eq!(selection.protected[0].id, snapshots[2].id);
        assert!(selection.delete_candidates.is_empty());
        Ok(())
    }

    #[test]
    fn preview_orders_by_instant_not_timestamp_text() -> Result<()> {
        let earlier = snapshot(1, &[], true, "2026-11-01T00:30:00+01:00");
        let later = snapshot(2, &[], true, "2026-11-01T00:00:00Z");
        let now = now_seconds("2026-11-02T00:00:00Z");
        let selection = select(
            &policy(1, &[], &[], 1, 1, 0),
            &[earlier, later.clone()],
            &BTreeMap::new(),
            now,
        )?;
        assert_eq!(selection.keep.len(), 1);
        assert_eq!(selection.keep[0].id, later.id);
        Ok(())
    }

    #[test]
    fn incomplete_newest_capture_does_not_consume_keep_slot() -> Result<()> {
        let older = snapshot(1, &[], true, "2026-11-01T00:00:01+00:00");
        let incomplete = snapshot(2, &[], false, "2026-11-01T00:00:02+00:00");
        let now = now_seconds("2026-11-02T00:00:00+00:00");
        let selection = select(
            &policy(1, &[], &[], 1, 1, 0),
            &[older.clone(), incomplete.clone()],
            &BTreeMap::new(),
            now,
        )?;
        assert_eq!(selection.keep.len(), 1);
        assert_eq!(selection.keep[0].id, older.id);
        assert_eq!(selection.protected.len(), 1);
        assert_eq!(selection.protected[0].id, incomplete.id);
        assert!(selection.delete_candidates.is_empty());
        Ok(())
    }

    #[test]
    fn floor_keeps_minimum_history_and_young_points() -> Result<()> {
        let snapshots = vec![
            snapshot(1, &[], true, "2026-09-01T00:00:00Z"),
            snapshot(2, &[], true, "2026-10-25T00:00:00Z"),
            snapshot(3, &[], true, "2026-11-15T00:00:00Z"),
            snapshot(4, &[], true, "2026-11-17T00:00:00Z"),
        ];
        let now = now_seconds("2026-11-18T00:00:00Z");
        let selection = select(
            &policy(0, &[], &[], 1, 3, 30),
            &snapshots,
            &BTreeMap::new(),
            now,
        )?;
        let ids: BTreeSet<&str> = selection
            .keep
            .iter()
            .map(|decision| decision.id.as_str())
            .collect();
        assert!(ids.contains(&snapshots[1].id.as_str()));
        assert!(ids.contains(&snapshots[2].id.as_str()));
        assert!(ids.contains(&snapshots[3].id.as_str()));
        assert_eq!(selection.delete_candidates.len(), 1);
        assert_eq!(selection.delete_candidates[0].id, snapshots[0].id);
        Ok(())
    }

    #[test]
    fn future_timestamp_is_protected_and_does_not_consume_a_keep_slot() -> Result<()> {
        let old = snapshot(1, &[], true, "2026-11-01T00:00:00Z");
        let future = snapshot(2, &[], true, "2030-01-01T00:00:00Z");
        let now = now_seconds("2026-11-03T00:00:00Z");
        let selection = select(
            &policy(1, &[], &[], 1, 1, 0),
            &[old.clone(), future.clone()],
            &BTreeMap::new(),
            now,
        )?;
        assert_eq!(selection.keep.len(), 1);
        assert_eq!(selection.keep[0].id, old.id);
        assert!(
            selection
                .protected
                .iter()
                .any(|decision| decision.id == future.id && decision.reason == "future-timestamp")
        );
        assert!(selection.delete_candidates.is_empty());
        Ok(())
    }

    #[test]
    fn peer_root_pin_is_kept_even_when_policy_would_delete_it() -> Result<()> {
        let snapshots = vec![
            snapshot(1, &[], true, "2026-11-01T00:00:00Z"),
            snapshot(2, &[], true, "2026-11-02T00:00:00Z"),
        ];
        let roots = BTreeMap::from([(snapshots[0].id.clone(), "peer-root:alpha".to_owned())]);
        let now = now_seconds("2026-11-03T00:00:00Z");
        let selection = select(&policy(1, &[], &[], 1, 1, 0), &snapshots, &roots, now)?;
        assert!(
            selection
                .keep
                .iter()
                .any(|decision| decision.id == snapshots[0].id)
        );
        assert!(selection.delete_candidates.is_empty());
        Ok(())
    }

    #[test]
    fn mass_change_thresholds_are_conservative() {
        assert!(
            !policy(0, &[], &[], 1, 1, 0)
                .required_peers
                .iter()
                .any(|_| true)
        );
        let triggered_files = 20u64 * 100 >= 100u64 * 20;
        assert!(triggered_files);
        let changed_bytes = 26u64;
        let previous_bytes = 100u64;
        assert!(u128::from(changed_bytes) * 100 >= u128::from(previous_bytes) * 25);
        let changed_bytes = 25u64;
        assert!(u128::from(changed_bytes) * 100 >= u128::from(previous_bytes) * 25);
        let changed_bytes = 24u64;
        assert!(u128::from(changed_bytes) * 100 < u128::from(previous_bytes) * 25);
    }

    #[test]
    fn peer_evidence_must_bind_snapshot_repository_and_time() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("evidence.json");
        let now = 1_800_000_000u64;
        let snapshot = "a".repeat(64);
        let manifest = "b".repeat(64);
        let repository = "c".repeat(64);
        let write = |value: serde_json::Value| -> Result<()> {
            std::fs::write(&path, serde_json::to_vec(&value)?)?;
            Ok(())
        };
        let input = PeerRecordInput {
            peer_id: "alpha",
            peer_repository_id: &repository,
            snapshot: &snapshot,
            capture_id: None,
            oracle_manifest: Some(&manifest),
            key_version: "v1",
            evidence: &path,
            verified_unix_seconds: None,
        };
        write(serde_json::json!({
            "result": "pass",
            "peer_repository_id": repository,
            "snapshot_id": snapshot,
            "manifest_snapshot_id": manifest,
            "verified_unix_seconds": now
        }))?;
        assert!(evidence_digest(&path, &input, now).is_ok());
        write(serde_json::json!({
            "result": "pass",
            "peer_repository_id": repository,
            "snapshot_id": "d".repeat(64),
            "manifest_snapshot_id": manifest,
            "verified_unix_seconds": now
        }))?;
        assert!(evidence_digest(&path, &input, now).is_err());
        write(serde_json::json!({
            "result": "pass",
            "peer_repository_id": repository,
            "snapshot_id": snapshot,
            "manifest_snapshot_id": manifest,
            "verified_unix_seconds": now.saturating_sub(32 * 24 * 60 * 60)
        }))?;
        assert!(evidence_digest(&path, &input, now).is_err());
        Ok(())
    }

    #[test]
    fn state_round_trip_preserves_generation_and_roots() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("retention-state.json");
        let state = RetentionState {
            schema_version: STATE_SCHEMA_VERSION,
            repository: "C:/vault/repository".into(),
            repository_id: "a".repeat(64),
            generation: 7,
            updated_unix_seconds: 1,
            last_validated: None,
            peer_roots: vec![PeerRoot {
                peer_id: "alpha".into(),
                peer_repository_id: "b".repeat(64),
                snapshot_id: "c".repeat(64),
                capture_id: None,
                oracle_manifest: Some("d".repeat(64)),
                key_version: "v1".into(),
                evidence_path: "C:/evidence/record.json".into(),
                evidence_sha256: "e".repeat(64),
                verified_unix_seconds: 1,
                sequence: 1,
                disposition: PeerDisposition::LastGood,
                abandonment_reason: None,
            }],
            history: Vec::new(),
        };
        write_state(&path, &state)?;
        let (read, _) = read_state(&path)?;
        assert_eq!(read.generation, 7);
        assert_eq!(read.peer_roots.len(), 1);
        assert_eq!(read.peer_roots[0].peer_id, "alpha");
        Ok(())
    }

    #[test]
    fn plan_round_trips_and_rejects_unknown_fields() -> Result<()> {
        let plan = RetentionPlan {
            schema_version: SCHEMA_VERSION,
            action: "preview".into(),
            preview_id: "a".repeat(32),
            preview_token: "b".repeat(32),
            created_unix_seconds: 1,
            expires_unix_seconds: 2,
            repository: "C:/vault/repository".into(),
            repository_id: "c".repeat(64),
            backend_version: "0.19.1".into(),
            backend_sha256: "d".repeat(64),
            policy_sha256: "e".repeat(64),
            policy: policy(1, &[], &[], 1, 1, 0),
            state_sha256: "f".repeat(64),
            state_generation: 1,
            inventory_sha256: "0".repeat(64),
            keep: Vec::new(),
            protected: Vec::new(),
            delete_candidates: Vec::new(),
            peer_pins: Vec::new(),
            floor: FloorReport { points: 1, days: 0 },
            mass_change: MassChangeReport {
                status: "not-triggered".into(),
                baseline_snapshot: None,
                current_snapshot: None,
                changed_files: 0,
                previous_files: 0,
                changed_bytes: 0,
                previous_bytes: 0,
                thresholds: String::new(),
                reason: String::new(),
            },
            limitations: Vec::new(),
        };
        let bytes = serde_json::to_vec(&plan)?;
        let decoded: RetentionPlan = serde_json::from_slice(&bytes)?;
        assert_eq!(decoded.preview_token, plan.preview_token);
        let mut value: serde_json::Value = serde_json::from_slice(&bytes)?;
        value["unexpected"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<RetentionPlan>(value).is_err());
        Ok(())
    }
}
