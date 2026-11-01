//! RecoveryKit v1: bounded, secret-free recovery instructions and inventory.

use crate::{
    pins,
    production::{self, Credential, SourceManifest},
};
use anyhow::{Context, Result, bail, ensure};
use bailiff::fixture::hash_file;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub const SCHEMA_VERSION: u32 = 1;
const MAX_MEMBERS: usize = 16;
const MAX_MEMBER_BYTES: u64 = 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const KIT_FILE: &str = "kit.json";
const MANIFEST_FILE: &str = "manifest.json";
const RUNBOOK_FILE: &str = "RUNBOOK.md";
const ACQUISITION_FILE: &str = "ACQUISITION.md";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitBackend {
    pub name: String,
    pub version: String,
    pub license: String,
    pub executable_sha256: String,
    pub pin_source: String,
    pub acquisition_url: String,
    pub acquisition_sha256: String,
    pub acquisition_member: Option<String>,
    pub publisher_verification: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryBinding {
    pub format: String,
    pub repository_id: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitProfile {
    pub os: String,
    pub architecture: String,
    pub tool_version: String,
    pub harness_sha256: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Custody {
    pub secret_route: String,
    pub named_custodian: String,
    pub rotation: String,
    pub cleanup: String,
    pub limits: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRefs {
    pub snapshot_id: String,
    pub capture_id: String,
    pub manifest_snapshot_id: String,
    pub verification: String,
    pub policy_sha256: String,
    pub tree_sha256: String,
    pub files: usize,
    pub bytes: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryKit {
    pub schema_version: u32,
    pub kit_id: String,
    pub created_unix_seconds: u64,
    pub backend: KitBackend,
    pub repository: RepositoryBinding,
    pub profile: KitProfile,
    pub evidence: EvidenceRefs,
    pub custody: Custody,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitMember {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitManifest {
    pub schema_version: u32,
    pub kit_id: String,
    pub members: Vec<KitMember>,
}

#[derive(Serialize)]
pub struct RecoveryKitReport {
    pub schema_version: u32,
    pub result: &'static str,
    pub kit_id: String,
    pub snapshot_id: String,
    pub members: usize,
    pub repository_bound: bool,
    pub full_read: bool,
    pub elapsed_ms: u128,
}

fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path).context("cannot read recovery kit member")?;
    ensure!(metadata.is_file(), "recovery kit member must be a file");
    ensure!(metadata.len() <= limit, "recovery kit member is oversized");
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "recovery kit member grew");
    Ok(bytes)
}

fn simple_name(path: &str) -> bool {
    let mut components = Path::new(path).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

fn validate_kit(kit: &RecoveryKit) -> Result<()> {
    ensure!(
        kit.schema_version == SCHEMA_VERSION,
        "unsupported kit version"
    );
    ensure!(
        kit.kit_id.len() == 32 && kit.kit_id.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "invalid kit ID"
    );
    production::validate_snapshot_id(&kit.evidence.snapshot_id)?;
    production::validate_snapshot_id(&kit.evidence.manifest_snapshot_id)?;
    ensure!(
        matches!(kit.evidence.capture_id.len(), 32 | 64)
            && kit
                .evidence
                .capture_id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "invalid kit capture ID"
    );
    ensure!(
        kit.evidence.verification == "manifest-hash",
        "unsupported kit verification mode"
    );
    ensure!(
        kit.evidence.policy_sha256.len() == 64 && kit.evidence.tree_sha256.len() == 64,
        "invalid kit evidence digest"
    );
    ensure!(
        kit.repository.format == "restic-v2" && !kit.repository.repository_id.is_empty(),
        "invalid repository binding"
    );
    ensure!(
        !kit.backend.name.is_empty()
            && !kit.backend.version.is_empty()
            && kit.backend.executable_sha256.len() == 64
            && kit
                .backend
                .acquisition_url
                .starts_with("https://github.com/")
            && kit.backend.acquisition_sha256.len() == 64,
        "invalid backend acquisition binding"
    );
    ensure!(
        !kit.profile.os.is_empty()
            && !kit.profile.architecture.is_empty()
            && kit.profile.harness_sha256.len() == 64,
        "invalid kit profile"
    );
    ensure!(
        !kit.custody.secret_route.is_empty()
            && !kit.custody.named_custodian.is_empty()
            && !kit.custody.rotation.is_empty()
            && !kit.custody.cleanup.is_empty()
            && !kit.custody.limits.is_empty(),
        "kit custody/rotation guidance is incomplete"
    );
    Ok(())
}

fn validate_manifest_inventory(manifest: &KitManifest) -> Result<()> {
    ensure!(
        manifest.schema_version == SCHEMA_VERSION,
        "unsupported kit manifest version"
    );
    ensure!(
        manifest.kit_id.len() == 32 && manifest.kit_id.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "invalid kit manifest ID"
    );
    ensure!(
        (1..=MAX_MEMBERS).contains(&manifest.members.len()),
        "invalid kit member count"
    );
    let mut names = BTreeSet::new();
    for member in &manifest.members {
        ensure!(simple_name(&member.path), "unsafe kit member path");
        ensure!(
            member.path != MANIFEST_FILE,
            "kit manifest cannot inventory itself"
        );
        ensure!(
            names.insert(member.path.clone()),
            "duplicate kit member path"
        );
        ensure!(
            member.size_bytes <= MAX_MEMBER_BYTES && member.sha256.len() == 64,
            "invalid kit member inventory"
        );
    }
    ensure!(names.contains(KIT_FILE), "kit manifest lacks kit.json");
    Ok(())
}

fn verify_inventory(directory: &Path) -> Result<KitManifest> {
    let bytes = read_bounded(&directory.join(MANIFEST_FILE), MAX_MANIFEST_BYTES)?;
    let manifest: KitManifest = serde_json::from_slice(&bytes).context("invalid kit manifest")?;
    validate_manifest_inventory(&manifest)?;
    let expected: BTreeSet<&str> = manifest
        .members
        .iter()
        .map(|member| member.path.as_str())
        .collect();
    let mut observed = BTreeSet::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        ensure!(
            entry.file_type()?.is_file(),
            "kit contains a non-file entry"
        );
        if name != MANIFEST_FILE {
            observed.insert(name);
        }
    }
    ensure!(
        observed.iter().map(String::as_str).collect::<BTreeSet<_>>() == expected,
        "kit members do not match the inventory"
    );
    for member in &manifest.members {
        let data = read_bounded(&directory.join(&member.path), MAX_MEMBER_BYTES)?;
        ensure!(
            data.len() as u64 == member.size_bytes && digest(&data) == member.sha256,
            "kit member missing or changed"
        );
    }
    Ok(manifest)
}

fn write_member(directory: &Path, name: &str, data: &[u8]) -> Result<KitMember> {
    ensure!(simple_name(name), "unsafe generated kit member");
    let path = directory.join(name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(data)?;
    file.sync_all()?;
    Ok(KitMember {
        path: name.into(),
        size_bytes: data.len() as u64,
        sha256: digest(data),
    })
}

fn runbook(kit: &RecoveryKit) -> String {
    format!(
        "# PeerVault RecoveryKit {kit_id}\n\n\
         Snapshot: `{snapshot}`\n\
         Stable capture ID: `{capture}`\n\
         Encrypted oracle snapshot: `{manifest}`\n\
         Repository binding: `{repository}` (restic-v2)\n\
         Backend: restic {version}, executable SHA-256 `{sha}`\n\n\
         ## Custody\n\n\
         - Secret route: {route}\n\
         - Named custodian: {custodian}\n\
         - Rotation: {rotation}\n\
         - Cleanup: {cleanup}\n\n\
         ## Offline recovery\n\n\
         1. Obtain the exact backend from `ACQUISITION.md`, verify its published\n\
            checksum/signature and the executable SHA-256 above. Do not use latest.\n\
         2. Mount the surviving repository copy read-only. Keep the recovery secret\n\
            on its separate offline medium; never store it in this kit.\n\
         3. Inventory check only:\n\n\
            `peervault recovery-kit verify --kit <this-kit>`\n\n\
         4. Bound repository check with a full read:\n\n\
            `peervault recovery-kit verify --kit <this-kit> --repository <copy> --backend <restic> --password-file <separate-secret>`\n\n\
         5. Restore into a new empty destination:\n\n\
            `peervault restore --repository <copy> --backend <restic> --snapshot {snapshot} --destination <new-empty> --password-file <separate-secret>`\n\n\
         6. Compare the restore report's tree digest with `{tree}` and retain the\n\
            report with the kit.\n\n\
         A kit self-checksum is not publisher authenticity. The copy, secret and\n\
         backend are separate failure domains; losing any one may prevent recovery.\n",
        kit_id = kit.kit_id,
        snapshot = kit.evidence.snapshot_id,
        capture = kit.evidence.capture_id,
        manifest = kit.evidence.manifest_snapshot_id,
        repository = kit.repository.repository_id,
        version = kit.backend.version,
        sha = kit.backend.executable_sha256,
        route = kit.custody.secret_route,
        custodian = kit.custody.named_custodian,
        rotation = kit.custody.rotation,
        cleanup = kit.custody.cleanup,
        tree = kit.evidence.tree_sha256,
    )
}

fn acquisition(kit: &RecoveryKit) -> String {
    format!(
        "# Backend acquisition\n\n\
         Backend: restic {version}\n\
         License: {license}\n\
         Pin source: {source}\n\
         Immutable URL: {url}\n\
         Archive SHA-256: {archive}\n\
         Archive member: {member}\n\
         Executable SHA-256: {executable}\n\n\
         Verify the release checksum/signature from the publisher's separately\n\
         obtained trust anchor, then verify the archive and executable hashes above.\n\
         Never use a download-latest URL or an unverified PATH binary for recovery.\n\
         If redistribution is unavailable, obtain the backend separately and carry\n\
         the verified offline dependency with the kit.\n",
        version = kit.backend.version,
        license = kit.backend.license,
        source = kit.backend.pin_source,
        url = kit.backend.acquisition_url,
        archive = kit.backend.acquisition_sha256,
        member = kit
            .backend
            .acquisition_member
            .as_deref()
            .unwrap_or("(single file)"),
        executable = kit.backend.executable_sha256,
    )
}

pub fn generate(
    repository: &Path,
    backend_path: &Path,
    snapshot: &str,
    kit_path: &Path,
    password_file: Option<&Path>,
) -> Result<RecoveryKitReport> {
    let started = Instant::now();
    ensure!(!kit_path.try_exists()?, "kit directory must be new");
    ensure!(repository.is_dir(), "repository does not exist");
    production::validate_snapshot_id(snapshot)?;
    let backend = production::backend(backend_path)?;
    let credential = Credential::new(password_file)?;
    let (capture_id, manifest_snapshot) =
        production::find_manifest(&backend, repository, &credential, snapshot)?;
    let manifest =
        production::read_manifest(&backend, repository, &credential, &manifest_snapshot)?;
    let config = production::repository_config(&backend, repository, &credential)?;
    let repository_id = config["id"]
        .as_str()
        .context("backend repository config lacks an ID")?
        .to_owned();
    ensure!(
        repository_id.len() >= 32 && repository_id.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "invalid backend repository ID"
    );
    let pins = pins::load()?;
    let pin = pins.restic;
    let asset = match std::env::consts::OS {
        "windows" => pin.windows,
        "linux" => pin.linux,
        _ => bail!("unsupported backend platform"),
    };
    let (_, harness_sha256) = hash_file(&std::env::current_exe()?)?;
    let mut kit_bytes = [0u8; 16];
    getrandom::fill(&mut kit_bytes).context("OS random source unavailable")?;
    let kit_id: String = kit_bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    let kit = RecoveryKit {
        schema_version: SCHEMA_VERSION,
        kit_id: kit_id.clone(),
        created_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        backend: KitBackend {
            name: "restic".into(),
            version: backend.version.clone(),
            license: pin.license,
            executable_sha256: backend.executable_sha256,
            pin_source: "backend-lock.json".into(),
            acquisition_url: asset.url,
            acquisition_sha256: asset.sha256,
            acquisition_member: asset.member,
            publisher_verification: "verify the published release checksum/signature from a separately held trust anchor; the kit checksum is not publisher authenticity".into(),
        },
        repository: RepositoryBinding {
            format: "restic-v2".into(),
            repository_id,
        },
        profile: KitProfile {
            os: std::env::consts::OS.into(),
            architecture: std::env::consts::ARCH.into(),
            tool_version: env!("CARGO_PKG_VERSION").into(),
            harness_sha256,
        },
        evidence: EvidenceRefs {
            snapshot_id: snapshot.into(),
            capture_id,
            manifest_snapshot_id: manifest_snapshot,
            verification: "manifest-hash".into(),
            policy_sha256: manifest.policy_sha256.clone(),
            tree_sha256: production::tree_sha256(&manifest),
            files: manifest.files.len(),
            bytes: manifest.files.iter().map(|entry| entry.size).sum(),
        },
        custody: Custody {
            secret_route: "separate offline medium or independently recoverable password-manager entry".into(),
            named_custodian: "named owner; not the source host, peer or the kit medium".into(),
            rotation: "rewrap/replace the repository password through the backend procedure; old kit material is not retroactively secret-free".into(),
            cleanup: "destroy replaced kit copies and secrets explicitly; deleting a copy cannot recall already-read data".into(),
            limits: vec![
                "no secrets are embedded in the kit".into(),
                "a former key-holding device can still read what it copied".into(),
                "the kit does not prove publisher authenticity by itself".into(),
            ],
        },
    };
    validate_kit(&kit)?;
    fs::create_dir(kit_path).context("cannot create kit directory")?;
    let members = vec![
        write_member(kit_path, KIT_FILE, &serde_json::to_vec_pretty(&kit)?)?,
        write_member(kit_path, RUNBOOK_FILE, runbook(&kit).as_bytes())?,
        write_member(kit_path, ACQUISITION_FILE, acquisition(&kit).as_bytes())?,
    ];
    let manifest = KitManifest {
        schema_version: SCHEMA_VERSION,
        kit_id,
        members,
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(kit_path.join(MANIFEST_FILE))?;
    serde_json::to_writer_pretty(&mut file, &manifest)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    verify_inventory(kit_path)?;
    let report = RecoveryKitReport {
        schema_version: SCHEMA_VERSION,
        result: "pass",
        kit_id: kit.kit_id.clone(),
        snapshot_id: kit.evidence.snapshot_id.clone(),
        members: manifest.members.len(),
        repository_bound: true,
        full_read: false,
        elapsed_ms: started.elapsed().as_millis(),
    };
    production::print_json(&report)?;
    Ok(report)
}

pub fn verify(
    kit_path: &Path,
    repository: Option<&Path>,
    backend_path: Option<&Path>,
    password_file: Option<&Path>,
) -> Result<RecoveryKitReport> {
    let started = Instant::now();
    ensure!(kit_path.is_dir(), "kit directory does not exist");
    let inventory = verify_inventory(kit_path)?;
    let bytes = read_bounded(&kit_path.join(KIT_FILE), MAX_MEMBER_BYTES)?;
    let kit: RecoveryKit = serde_json::from_slice(&bytes).context("invalid kit.json")?;
    validate_kit(&kit)?;
    ensure!(
        inventory.kit_id == kit.kit_id,
        "kit.json and inventory IDs differ"
    );
    let (repository_bound, full_read) = match (repository, backend_path) {
        (None, None) => (false, false),
        (Some(repository), Some(backend_path)) => {
            let backend = production::backend(backend_path)?;
            ensure!(
                backend.executable_sha256 == kit.backend.executable_sha256
                    && backend.version == kit.backend.version,
                "configured backend does not match the kit"
            );
            let credential = Credential::new(password_file)?;
            let config = production::repository_config(&backend, repository, &credential)?;
            ensure!(
                config["id"].as_str() == Some(kit.repository.repository_id.as_str()),
                "repository binding does not match the kit"
            );
            let (capture_id, manifest_snapshot) = production::find_manifest(
                &backend,
                repository,
                &credential,
                &kit.evidence.snapshot_id,
            )?;
            ensure!(
                capture_id == kit.evidence.capture_id
                    && manifest_snapshot == kit.evidence.manifest_snapshot_id,
                "oracle snapshot does not match the kit"
            );
            let manifest: SourceManifest =
                production::read_manifest(&backend, repository, &credential, &manifest_snapshot)?;
            ensure!(
                manifest.policy_sha256 == kit.evidence.policy_sha256
                    && production::tree_sha256(&manifest) == kit.evidence.tree_sha256
                    && manifest.files.len() == kit.evidence.files
                    && manifest.files.iter().map(|entry| entry.size).sum::<u64>()
                        == kit.evidence.bytes,
                "encrypted oracle does not match the kit evidence"
            );
            production::full_read(&backend, repository, &credential)
                .context("repository full-read failed")?;
            (true, true)
        }
        _ => bail!("--repository and --backend must be supplied together"),
    };
    let report = RecoveryKitReport {
        schema_version: SCHEMA_VERSION,
        result: "pass",
        kit_id: kit.kit_id,
        snapshot_id: kit.evidence.snapshot_id,
        members: inventory.members.len(),
        repository_bound,
        full_read,
        elapsed_ms: started.elapsed().as_millis(),
    };
    production::print_json(&report)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(path: &str) -> KitMember {
        KitMember {
            path: path.into(),
            size_bytes: 1,
            sha256: "a".repeat(64),
        }
    }

    #[test]
    fn kit_inventory_rejects_traversal_duplicates_and_missing_kit() {
        let mut value = KitManifest {
            schema_version: 1,
            kit_id: "a".repeat(32),
            members: vec![member(KIT_FILE)],
        };
        assert!(validate_manifest_inventory(&value).is_ok());
        value.members.push(member("../escape"));
        assert!(validate_manifest_inventory(&value).is_err());
        value.members = vec![member(KIT_FILE), member(KIT_FILE)];
        assert!(validate_manifest_inventory(&value).is_err());
        value.members = vec![member(RUNBOOK_FILE)];
        assert!(validate_manifest_inventory(&value).is_err());
    }

    #[test]
    fn kit_schema_requires_custody_and_bindings() {
        let kit = RecoveryKit {
            schema_version: 1,
            kit_id: "a".repeat(32),
            created_unix_seconds: 1,
            backend: KitBackend {
                name: "restic".into(),
                version: "0.19.1".into(),
                license: "BSD-2-Clause".into(),
                executable_sha256: "a".repeat(64),
                pin_source: "backend-lock.json".into(),
                acquisition_url: "https://github.com/x/y".into(),
                acquisition_sha256: "b".repeat(64),
                acquisition_member: None,
                publisher_verification: "verify".into(),
            },
            repository: RepositoryBinding {
                format: "restic-v2".into(),
                repository_id: "c".repeat(32),
            },
            profile: KitProfile {
                os: "windows".into(),
                architecture: "x86_64".into(),
                tool_version: "0.0.1".into(),
                harness_sha256: "d".repeat(64),
            },
            evidence: EvidenceRefs {
                snapshot_id: "e".repeat(64),
                capture_id: "c".repeat(32),
                manifest_snapshot_id: "f".repeat(64),
                verification: "manifest-hash".into(),
                policy_sha256: "a".repeat(64),
                tree_sha256: "b".repeat(64),
                files: 1,
                bytes: 1,
            },
            custody: Custody {
                secret_route: "offline".into(),
                named_custodian: "owner".into(),
                rotation: "rewrap".into(),
                cleanup: "destroy".into(),
                limits: vec!["limit".into()],
            },
        };
        assert!(validate_kit(&kit).is_ok());
        let mut broken = kit.clone();
        broken.custody.limits.clear();
        assert!(validate_kit(&broken).is_err());
        let mut broken = kit;
        broken.repository.repository_id.clear();
        assert!(validate_kit(&broken).is_err());
    }
}
