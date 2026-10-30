# PeerVault

> Encrypted, deduplicated, versioned storage with repeatable restore verification.

**Status: experimental R0 implementation.** The Rust fixture/compare/bakeoff
harness, backend pins and CI definitions exist. Production backup/restore,
desktop/sync and complete qualification remain open. Each restore report covers
an exact tested snapshot/profile/drill, not a permanent recoverability guarantee.

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
| Candidate later integrations | C-CAS, Bailiff, Anchor; versioned contracts and separate qualification required |
| Product concerns / reuse candidates | Restore, Conflict, KeyRec, GC, Sync |

Ecosystems are separate repositories. This repository owns its implementation, tests, releases, and versioning; superplatforms consume it as a pinned dependency.
Core use remains local and self-hostable. R0 commands below exercise synthetic
fixtures only; they are not a production backup interface.

## Experimental R0 build and checks

The repository pins Rust in `rust-toolchain.toml`, dependencies in `Cargo.lock`,
and comparison-backend archives in `backend-lock.json`. From this repository:

```powershell
cargo build --locked
cargo test --locked
cargo run --locked -- fixture .peervault-work-fixture
cargo run --locked -- compare .peervault-work-fixture/source .peervault-work-fixture/expected.json
```

Use a new disposable directory for each run. Follow the [R0 runbook](docs/r0.md)
for backend installation, independent full-size A/B source checks, smoke profiles
and append-only evidence export. CP0/CP1 hardening adds approved executable hashes,
bounded/atomic downloads, owned process trees and complete failure diagnostics.
Smoke success remains a partial R0 gate; backend selection still requires CP2.

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
