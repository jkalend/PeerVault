# PeerVault

> Encrypted, deduplicated, versioned storage with repeatable restore verification.

**Status: experimental R0/V0.1/V0.2 implementation.** The Rust fixture/compare/
bakeoff harness, backend pins, CI definitions, a V0.1 local capture/history/verify/
restore adapter and a V0.2 secret-free RecoveryKit/copy drill exist. Production
backup/restore, clean-hardware recovery, desktop/sync and complete qualification
remain open. Each restore report
covers an exact tested snapshot/profile/drill, not a permanent recoverability
guarantee.

Canonical design: [peervault](https://github.com/jkalend/project-planning/blob/main/projects/peervault.md)

Delivery map: [V0–1.0 roadmap](docs/implementation-plan.md). Detailed stages,
acceptance and release criteria: [local project-planning plan](../../project-planning/projects/peervault-release-plan.md)
([GitHub fallback](https://github.com/jkalend/project-planning/blob/main/projects/peervault-release-plan.md)).
The local plan may precede publication. See the [implementation catch-up plan](docs/implementation-catch-up.md)
for the 33 independent-review dispositions and source-specific follow-up tasks;
no release gate is inferred from implementation presence.

## What it is

PeerVault is a standalone backup and recovery product and the planned
data-durability layer of PersonalMesh. The selected backend owns snapshot metadata
and integrity checks. An authorized recovery device verifies full content and
performs restore drills; a ciphertext-only peer needs a separate authorized
key-holding recovery path to establish recoverability.

## Planned capability areas

These are product concerns, not separate services to scaffold. The release plan
maps their standalone 1.0 scope and later boundaries.

- PeerVault Store
- PeerVault History
- PeerVault Sync
- PeerVault Crypto
- PeerVault Database
- PeerVault Network
- PeerVault Desktop
- PeerVault Recovery
- PeerVault Collaboration

## First useful slice

Compare pinned mature backup backends on disposable Windows/Linux profiles, then
build a thin Rust adapter for local snapshot/history, full-content verification,
restore to a new empty destination and independent fixture-hash comparison.
The selected backend owns encrypted deduplication, repository format, keys and
maintenance. Complete clean-device/key recovery before one-peer replication;
qualify keep-both live-folder sync separately from backup repository copies.

## Ecosystem relationships

| Relationship | Projects |
|---|---|
| Consumers | PersonalMesh |
| First-slice dependency | One mature backup backend selected and pinned by R0; no unbuilt ecosystem prerequisite |
| Dependencies | `bailiff` from the Components workspace (path dependency); C-CAS and Anchor remain later candidates |
| Product concerns / reuse candidates | Restore, Conflict, KeyRec, GC, Sync |

Ecosystems are separate repositories. This repository owns its implementation, tests, releases, and versioning; superplatforms consume it as a pinned dependency.
Core use remains local and self-hostable. R0 commands below exercise synthetic
fixtures only; they are not a production backup interface.

## Experimental V0.1 local capture

The selected restic adapter has separate production commands: `init`, `capture`,
`history`, `verify` and `restore`. They print schema-v2 JSON reports, refuse
unapproved binaries, require `PEERVAULT_PASSWORD` or `--password-file`, and only
restore a whole snapshot into a new empty destination. Restore streams a tar into
owned staging, validates paths/types/hashes against an encrypted tree oracle, and
refuses a missing/ambiguous oracle.

```powershell
$env:PEERVAULT_PASSWORD = '<disposable secret>'
.\target\release\peervault.exe init --repository .peervault-work-v01-repo --backend .tools\restic.exe
.\target\release\peervault.exe capture --repository .peervault-work-v01-repo --backend .tools\restic.exe --source .\source --tag demo
.\target\release\peervault.exe history --repository .peervault-work-v01-repo --backend .tools\restic.exe
.\target\release\peervault.exe verify --repository .peervault-work-v01-repo --backend .tools\restic.exe
.\target\release\peervault.exe restore --repository .peervault-work-v01-repo --backend .tools\restic.exe --snapshot <id> --destination .peervault-work-v01-restored
```

See [V0.1 limits and report fields](docs/v0.1.md). This is not clean-device
recovery, sync, a supported backup product or arbitrary-archive containment.

An experimental V0.2 `recovery-kit` generator/verifier records the snapshot,
stable capture ID, encrypted oracle, backend acquisition hashes, repository
binding and custody instructions without embedding secrets:

```powershell
.\target\release\peervault.exe recovery-kit generate --repository .peervault-work-v01-repo --backend .tools\restic.exe --snapshot <id> --kit .peervault-work-v02-kit
.\target\release\peervault.exe recovery-kit verify --kit .peervault-work-v02-kit
.\target\release\peervault.exe recovery-kit verify --kit .peervault-work-v02-kit --repository <surviving-copy> --backend .tools\restic.exe --password-file <separate-secret>
```

See [V0.2 limits and drill evidence](docs/v0.2.md). The clean-copy drill is
author-only and same-host; independent-operator and clean-hardware recovery remain
open.

## Components

Component libraries live in [`Components/`](../../Components/README.md), one workspace package per component ([D-0012](../../project-planning/decisions/0012-component-repositories.md)). This repository consumes them as pinned dependencies and contains no component source.

| Package | Component | Spec |
|---|---|---|
| `bailiff` | Bailiff | [spec](../../project-planning/components/bailiff.md) |
| `ferry` | Ferry | [spec](../../project-planning/components/ferry.md) |
| `parapet` | Parapet | [spec](../../project-planning/components/parapet.md) |

### Component boundary

The R0 harness — fixture generation, backend pinning, bounded process execution, media identity and full-read comparison — lives in [`Components/crates/bailiff`](../../Components/crates/bailiff/src/lib.rs) and is consumed as a path dependency. This repository keeps the CLI (`src/main.rs`), the pin document (`backend-lock.json`) and the evidence inputs (`src/pins.rs`) that identify exactly what was tested.

The exact Components commit is recorded in [`components-lock.txt`](components-lock.txt); CI checks out that commit and the evidence inputs hash the lock file. Evidence also records the bailiff crate version plus hashes of this repository's own build inputs. The harness no longer reads this repository's files at compile time, so moving it did not silently change what a report attests to.

## Planned repository layout

```text
PeerVault/
├─ crates/
├─ cli/
├─ desktop/
├─ tests/
├─ docs/
└─ README.md
```

## Future production workflow (illustrative, unimplemented)

```text
1. Review source roots and select a repository on separately named media.
2. Capture the selected source set and record its committed snapshot reference.
3. Full-read verify that exact reference at the backend's actual supported scope.
4. Restore the same reference into a new empty scratch destination.
5. Compare the complete restored tree against the declared oracle and retain the report.
```

Whole-snapshot/new-target restore is the initial 1.0 scope. Selective, merge,
overwrite and resume need separate qualification. Known same-media or unknown
failure domains require explicit acknowledgement and cannot display independent
backup coverage. An accessible peer under shared credentials can be deleted by
malware with those credentials; no endpoint-compromise/ransomware guarantee is
made. Supported backup-only 0.9 precedes full 1.0 if sync remains unqualified.

## Non-goals

- A general distributed filesystem or multi-user collaboration in 1.0
- Block-device replication
- Public file sharing
- Custom cryptography

## Planning and research

- [Ecosystem design](https://github.com/jkalend/project-planning/blob/main/projects/peervault.md)
- [PersonalMesh](https://github.com/jkalend/project-planning/blob/main/platforms/personalmesh.md)
- [Additions catalog](https://github.com/jkalend/project-planning/blob/main/components/additions-catalog.md)
- [PersonalMesh research](https://github.com/jkalend/project-planning/blob/main/research/personalmesh-research.md)
- [Confidence assessment](https://github.com/jkalend/project-planning/blob/main/research/research-quality-2026-10-18.md)

## License

All rights reserved until a license is chosen. Public visibility does not grant
an open-source license. Third-party dependencies retain their own licenses.
