# Compatibility and contract matrix

Status: draft for backup-only 0.9. No frozen supported release exists yet.

## Versioned local contracts

| Contract | Version | Producer | Compatibility rule |
|---|---|---|---|
| Capture/history/verify/restore JSON | 2 | `peervault` | additive fields only within a supported release |
| Tree manifest | 1 | capture | reject unknown/mismatched schema and policy digest |
| RecoveryKit | 1 | recovery-kit | reject unknown members/schema; no secrets |
| `PeerCopyRecord` | 1 | V0.3 drill | append-only step trail; no secret fields |
| Retention policy | 1 | owner | unknown fields refused; missing floor fields default to 3/30 |
| Retention plan | 2 | `retention preview` | exact schema/expiry/state/inventory binding |
| Retention state | 1 | `retention state-*` | monotonic generation; unknown fields refused |
| Schedule config/state | 1 | owner/core | unknown fields refused; state atomically replaced |
| IPC request/response | 1 | desktop/client | unknown fields/schema refused; newline JSON, 64 KiB cap |

## Backend and repository

- restic 0.19.1, repository format v2, pinned executable SHA-256.
- Backend owns encryption, keys, chunking, dedup, repository format and native
  locking/maintenance.
- Kopia 0.23.1 remains an R0 comparison candidate, not a supported runtime
  backend.
- Schema or policy-digest changes require a new artifact and affected reruns.

## OS, filesystem and metadata

| Area | Supported candidate | Rejected/lossy |
|---|---|---|
| Windows | 11 x64, NTFS | reparse points, streams/ACLs, placeholders, VSS, shares |
| Linux | Ubuntu 24.04 LTS x64, ext4 | special files, xattrs/owners beyond read-only, network/FUSE roots |
| Paths | UTF-8 portable relative, <=1024 bytes, <=16 levels | case collisions, reserved names, unsafe characters |
| Content | regular files/dirs, empty files, read-only bit | hard-link identity, timestamps, sparse allocation, symlinks |
| Restore | whole snapshot to new empty destination | selective/merge/overwrite/resume |

## Upgrade and rollback

- No prior released PeerVault artifact exists, so upgrade/rollback is not yet
  qualified.
- A future update must read snapshots created by the previous supported version
  and be tested side by side with the old executable.
- Backend format migration requires a verified recovery copy and an old-snapshot
  restore before irreversible maintenance.
- Removing a supported version requires a retained recovery path; an old
  executable is not guaranteed to understand a migrated repository.

## Breaking changes from experimental builds

The pre-release `retention-preview` command was replaced by
`retention preview` with required owner state, and `retention apply` now performs
backend maintenance. Experimental 0.0.x builds make no compatibility promise;
the frozen 0.9 artifact must record an explicit contract version.
