# PeerVault

> Encrypted, deduplicated, versioned storage that proves it can restore.

**Status: planning. No implementation yet.**

Canonical design: [peervault](https://github.com/jkalend/project-planning/blob/main/projects/peervault.md)

## What it is

PeerVault is the data-durability layer of PersonalMesh. Every snapshot has a manifest, every device can verify it, and restore drills are part of the product.

## Subprojects

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

Local snapshot, deduplicated chunk store, exact restore, and hash verification; then encrypted peer replication with keep-both conflict policy.

## Ecosystem relationships

| Relationship | Projects |
|---|---|
| Consumers | PersonalMesh |
| Depends on | C-CAS, P-BAILIFF, P-ANCHOR |
| Shared additions | Bailiff, Anchor, Restore, Conflict, KeyRec, GC, Sync |

Ecosystems are separate repositories. This repository owns its implementation, tests, releases, and versioning; superplatforms consume it as a pinned dependency.

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

## First demo (planned)

```bash
peervault init ~/Documents --vault docs
peervault snapshot --wait
peervault restore latest --to /tmp/restore
peervault verify latest
```

## Non-goals

- A general distributed filesystem
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

Private. All rights reserved until a license is chosen.
