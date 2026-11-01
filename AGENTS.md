# PeerVault agent guide

## Start here

Read [README.md](README.md) and, when present, `../AGENTS.md` for workspace conventions. Before implementation or release work, read the [short roadmap](docs/implementation-plan.md) and its linked [detailed release plan](../../project-planning/projects/peervault-release-plan.md). For feature or architecture work, read the [canonical design](https://github.com/jkalend/project-planning/blob/main/projects/peervault.md), available locally at `../../project-planning/projects/peervault.md`.

The repository now has an experimental Rust R0 fixture/compare/bakeoff harness, backend pins and CI definitions. Inspect manifests/source and actual evidence before selecting commands or claiming a gate passed. Before extending that implementation, read the [catch-up plan and review dispositions](docs/implementation-catch-up.md). Production backup, sync and desktop remain separate gates; Rust core and C# desktop remain the intended direction.

## Scope and first slice

PeerVault owns folder snapshots, deduplicated encrypted storage, history, peer sync, and verifiable recovery. It is a data-durability product; a successful upload alone does not prove a backup works.

Start with R0's pinned mature-backend comparison, then a thin adapter for local snapshot/history, full-content verification and exact restore to a new empty directory with independent fixture hashes. The backend owns chunking, encrypted deduplication, repository format, keys and maintenance. Complete clean-device/key recovery before peer replication; qualify live-folder keep-both sync separately. C-CAS, Bailiff and Anchor are later candidates rather than first-slice dependencies.

## Design invariants

- Use the selected backend's documented encryption, identity and repository contracts. A future custom format requires a separate reviewed design for chunk addressing, key scope, nonce handling, canonical manifests, durable publication, dedup leakage and migration; use reviewed libraries.
- A supported file version reconstructs exact bytes and declared metadata. SnapshotRef and RestoreReport are versioned diagnostic contracts; they imply no PeerVault Merkle/signature scheme, remote attestation or proof against endpoint compromise.
- Publish a snapshot only after its referenced data and manifest are durably committed. Interrupted writes must leave prior snapshots readable and partial work identifiable.
- Treat the history cache as rebuildable from backend-owned authoritative snapshots. Preserve policy/evidence separately and mark unavailable evidence unknown after rebuild. Define and test metadata limits across Windows/Linux, including case collisions, symlinks, permissions, and unsupported file types.
- Production restore preflights paths/types/collisions, uses qualified contained writes into owned staging and verifies the full restored tree against a declared oracle. The initial 1.0 scope is a whole snapshot into a new empty target; selective/merge/overwrite/resume are out of scope. R0's backend-native restore of self-created fixtures does not qualify arbitrary archive extraction.
- Use backend-native maintenance only after the release-plan retention gate. Inspect deletion plans and preserve retained snapshots, recovery pins and active capture/restore/transfer roots; uncertain peer/lock/storage state defers deletion. The first slice has no PeerVault GC command.
- Sync conflicts keep both versions by default. Propagate deletion explicitly with tombstones; an offline or missing peer is not evidence that data can be deleted.
- Keys, recovery material, and plaintext file content stay out of logs. Recovery and rotation must retain access to older snapshots or state their limits explicitly.
- Component code lives in `../../Components/`, not in this repository ([D-0012](../../project-planning/decisions/0012-component-repositories.md)); consume packages as pinned dependencies. An adapter that depends on this ecosystem belongs in the consuming superplatform shell.


## Validation priorities

- Round-trip empty files, duplicate/large files, Unicode paths, and supported metadata; compare every restored hash with the manifest.
- Corrupt/truncate chunks and manifests, interrupt snapshot/restore/GC, and verify loud failures plus intact prior snapshots.
- Rebuild history cache from backend-owned snapshot metadata without inventing verification evidence; test concurrent edits, offline peer reconnects, and deterministic keep-both artifacts when sync exists. Lost authoritative root/evidence state defers deletion.
- Test key loss/recovery and rotation on disposable vaults. A restore report must distinguish verified files, failures, and unsupported metadata.

## Working and completion

Run Git and tooling from this repository and preserve unrelated changes. Discover commands from actual manifests/scripts; document real build/test/demo and recovery commands with the first implementation. Use disposable directories and synthetic data. Report checks actually run and recovery guarantees actually demonstrated.
