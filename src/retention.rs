//! V0.5 retention preview: select keep/delete candidates without mutating anything.

use crate::production::{self, Credential, SnapshotRef};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub const SCHEMA_VERSION: u32 = 1;
const MAX_POLICY_BYTES: u64 = 64 * 1024;

#[derive(Clone, Serialize, Deserialize)]
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
}

fn default_minimum_keep() -> usize {
    1
}

#[derive(Serialize)]
pub struct RetentionDecision {
    pub id: String,
    pub capture_id: Option<String>,
    pub time: String,
    pub reason: String,
}

#[derive(Serialize)]
pub struct RetentionPlan {
    pub schema_version: u32,
    pub preview_id: String,
    pub created_unix_seconds: u64,
    pub policy_sha256: String,
    pub apply: bool,
    pub keep: Vec<RetentionDecision>,
    pub protected: Vec<RetentionDecision>,
    pub delete_candidates: Vec<RetentionDecision>,
    pub limitations: Vec<&'static str>,
}

fn decision(snapshot: &SnapshotRef, reason: impl Into<String>) -> RetentionDecision {
    RetentionDecision {
        id: snapshot.id.clone(),
        capture_id: snapshot.capture_id.clone(),
        time: snapshot.time.clone(),
        reason: reason.into(),
    }
}

fn validate_policy(policy: &RetentionPolicy) -> Result<()> {
    ensure!(
        policy.schema_version == SCHEMA_VERSION,
        "unsupported retention policy version"
    );
    ensure!(policy.keep_last <= 10_000, "keep_last is too large");
    ensure!(policy.minimum_keep >= 1, "minimum_keep must be at least 1");
    ensure!(
        policy.keep_tagged.len() <= 128
            && policy
                .keep_tagged
                .iter()
                .all(|tag| !tag.is_empty() && tag.len() <= 128),
        "invalid keep_tagged entry"
    );
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

fn select(
    policy: &RetentionPolicy,
    snapshots: &[SnapshotRef],
) -> Result<(
    Vec<RetentionDecision>,
    Vec<RetentionDecision>,
    Vec<RetentionDecision>,
)> {
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
    let restorable: Vec<&SnapshotRef> = ordered
        .iter()
        .filter(|snapshot| snapshot.oracle_manifest.is_some())
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
    let mut protected = Vec::new();
    for snapshot in &ordered {
        if snapshot.oracle_manifest.is_none() {
            protected.push(decision(snapshot, "missing-oracle"));
        }
    }
    let protected_ids: BTreeMap<&str, ()> = protected
        .iter()
        .map(|decision| (decision.id.as_str(), ()))
        .collect();
    let delete_candidates = ordered
        .iter()
        .filter(|snapshot| {
            !keep.contains_key(&snapshot.id) && !protected_ids.contains_key(snapshot.id.as_str())
        })
        .map(|snapshot| decision(snapshot, "eligible-after-qualified-retention-gate"))
        .collect();
    let mut keep: Vec<RetentionDecision> = keep.into_values().collect();
    keep.sort_by_key(|decision| instants.get(&decision.id).copied().unwrap_or(i64::MAX));
    Ok((keep, protected, delete_candidates))
}

pub fn preview(
    repository: &Path,
    backend_path: &Path,
    policy_path: &Path,
    password_file: Option<&Path>,
) -> Result<RetentionPlan> {
    let backend = production::backend(backend_path)?;
    let credential = Credential::new(password_file)?;
    ensure!(repository.is_dir(), "repository does not exist");
    let mut bytes = Vec::new();
    File::open(policy_path)
        .context("cannot read retention policy")?
        .take(MAX_POLICY_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_POLICY_BYTES,
        "retention policy is too large"
    );
    let policy: RetentionPolicy =
        serde_json::from_slice(&bytes).context("invalid retention policy")?;
    let snapshots = production::snapshot_refs(&backend, repository, &credential)?;
    let (keep, protected, delete_candidates) = select(&policy, &snapshots)?;
    let plan = RetentionPlan {
        schema_version: SCHEMA_VERSION,
        preview_id: production::random_hex()?,
        created_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        policy_sha256: format!("{:x}", Sha256::digest(&bytes)),
        apply: false,
        keep,
        protected,
        delete_candidates,
        limitations: vec![
            "preview only; no snapshot is deleted and no backend maintenance runs",
            "active locks, peer roots and recovery pins are not evaluated",
            "delete candidates require a separately qualified retention/maintenance gate",
            "missing-oracle snapshots are protected and never listed for deletion",
        ],
    };
    production::print_json(&plan)?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(index: u8, tags: &[&str], oracle: bool) -> SnapshotRef {
        SnapshotRef {
            id: format!("{index:02x}").repeat(32),
            capture_id: Some(format!("{index:02x}").repeat(16)),
            time: format!("2026-11-01T00:00:{index:02}+00:00"),
            hostname: None,
            paths: vec![],
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            oracle_manifest: oracle.then(|| "a".repeat(64)),
        }
    }

    fn policy(
        keep_last: usize,
        tags: &[&str],
        explicit: &[&str],
        minimum: usize,
    ) -> RetentionPolicy {
        RetentionPolicy {
            schema_version: 1,
            keep_last,
            keep_tagged: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            keep_snapshots: explicit.iter().map(|id| (*id).to_owned()).collect(),
            minimum_keep: minimum,
        }
    }

    #[test]
    fn preview_keeps_last_tagged_and_explicit_and_protects_missing_oracle() -> Result<()> {
        let snapshots = vec![
            snapshot(1, &[], true),
            snapshot(2, &["keep"], true),
            snapshot(3, &[], false),
            snapshot(4, &[], true),
        ];
        let policy = policy(1, &["keep"], &[&snapshots[0].id], 1);
        let (keep, protected, candidates) = select(&policy, &snapshots)?;
        let keep_ids: Vec<&str> = keep.iter().map(|decision| decision.id.as_str()).collect();
        assert!(keep_ids.contains(&snapshots[0].id.as_str()));
        assert!(keep_ids.contains(&snapshots[1].id.as_str()));
        assert!(keep_ids.contains(&snapshots[3].id.as_str()));
        assert_eq!(protected.len(), 1);
        assert_eq!(protected[0].id, snapshots[2].id);
        assert!(candidates.is_empty());
        Ok(())
    }

    #[test]
    fn preview_orders_by_instant_not_timestamp_text() -> Result<()> {
        let mut earlier = snapshot(1, &[], true);
        earlier.time = "2026-11-01T00:30:00+01:00".into();
        let mut later = snapshot(2, &[], true);
        later.time = "2026-11-01T00:00:00Z".into();
        let (keep, _, _) = select(&policy(1, &[], &[], 1), &[earlier, later.clone()])?;
        assert_eq!(keep.len(), 1);
        assert_eq!(keep[0].id, later.id);
        Ok(())
    }

    #[test]
    fn incomplete_newest_capture_does_not_consume_keep_slot() -> Result<()> {
        let mut older = snapshot(1, &[], true);
        older.time = "2026-11-01T00:00:01+00:00".into();
        let mut incomplete = snapshot(2, &[], false);
        incomplete.time = "2026-11-01T00:00:02+00:00".into();
        let (keep, protected, candidates) = select(
            &policy(1, &[], &[], 1),
            &[older.clone(), incomplete.clone()],
        )?;
        assert_eq!(keep.len(), 1);
        assert_eq!(keep[0].id, older.id);
        assert_eq!(protected.len(), 1);
        assert_eq!(protected[0].id, incomplete.id);
        assert!(candidates.is_empty());
        Ok(())
    }

    #[test]
    fn preview_minimum_keep_never_deletes_all_history() -> Result<()> {
        let snapshots = vec![snapshot(1, &[], true), snapshot(2, &[], true)];
        let (keep, protected, candidates) = select(&policy(0, &[], &[], 1), &snapshots)?;
        assert_eq!(keep.len(), 1);
        assert!(keep[0].reason == "minimum-keep");
        assert!(protected.is_empty());
        assert_eq!(candidates.len(), 1);
        Ok(())
    }

    #[test]
    fn preview_rejects_unknown_explicit_ids_and_bad_policy() {
        let snapshots = vec![snapshot(1, &[], true)];
        assert!(select(&policy(1, &[], &[&"b".repeat(64)], 1), &snapshots).is_err());
        assert!(select(&policy(1, &[], &[], 0), &snapshots).is_err());
    }
}
