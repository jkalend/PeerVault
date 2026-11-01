# PeerVault implementation and release roadmap

Status: experimental R0 implementation, 2026-11-01. Fixture/compare/bakeoff and
same-host copy/recovery topology source, backend pins and CI definitions exist.
Scored runs and the restic selection are in
[ADR 0001](adr/0001-r0-backend-selection.md); R0 fault/containment residuals and
production gates remain unqualified. Source presence and CI definitions are not
passing run evidence. The first CP2 slice is recorded in
[CP2 copy drill progress](cp2-copy-drill.md); the backend/profile selection is
recorded in [ADR 0001](adr/0001-r0-backend-selection.md). A V0.1 local
capture/history/verify/restore adapter exists as experimental source with smoke
evidence; a V0.2 RecoveryKit and author-only copy drill and a V0.3 one-peer
copy-record drill and a read-only retention preview exist. V0.4 selected the
backup-only 0.9 path; supported sync remains a separate open branch. Their
production, authenticated-transport, independent-operator and clean-hardware
gates remain open.

[Review history](../../../project-planning/projects/peervault-release-plan.md#planning-review):
Terra's original three findings were repaired; the subsequent
[DeepSeek/GLM/Kimi review](implementation-plan-review.md) raised 33 more.
[Catch-up plan and dispositions](implementation-catch-up.md) map every finding to
the implementation now present. Planning repairs are recorded; code/evidence
gates remain open. CP0/CP1 harness repairs are recorded in
[the progress record](cp0-cp1-progress.md); complete CP2/scored R0 before
production V0.1.

This is the short delivery map. Contracts, fixtures, failure protocols and release
criteria live in **project-planning**:

- [Canonical design](../../../project-planning/projects/peervault.md) — scope,
  backend adapter, evidence states and safety invariants.
- [Detailed V0–1.0 plan](../../../project-planning/projects/peervault-release-plan.md)
  — ordered stages, acceptance, supported release and integration tracks.
- [Execution contracts](../../../project-planning/projects/peervault-contracts.md)
  and [qualification protocol](../../../project-planning/projects/peervault-validation.md)
  — adapter/state/credential/peer/job contracts, precise fixture, profiles and evidence.
- [Evidence dossier](../../../project-planning/research/ecosystems/peervault.md)
  and [validation protocols](../../../project-planning/projects/peervault.md#reproducible-validation-protocols)
  — dated backend research and reproducible experiments.

These links open the local planning checkout. Remote fallbacks are the
[design](https://github.com/jkalend/project-planning/blob/main/projects/peervault.md)
and [release plan](https://github.com/jkalend/project-planning/blob/main/projects/peervault-release-plan.md).
GitHub may lag local uncommitted changes; nothing here claims publication.

## V0 — Prove recovery first

| Stage | Result and acceptance |
|---|---|
| [R0](../../../project-planning/projects/peervault-release-plan.md#r0--backend-decision-and-executable-fixture) | Compare pinned Kopia/restic on Windows and Linux; full-read check and independent restore hashes; record one backend/profile decision. |
| [V0.1](../../../project-planning/projects/peervault-release-plan.md#v01--local-capture-history-and-exact-restore) | Thin Rust CLI: capture, history, verify and safe fresh-target restore; partial scans, corrupt input and process interruption fail visibly. |
| [V0.2](../../../project-planning/projects/peervault-release-plan.md#v02--recovery-without-the-original-device) | Recover without the original device/index/cached keys, using an independent repository copy and separately held recovery material. |
| [V0.3](../../../project-planning/projects/peervault-release-plan.md#v03--one-authorized-peer-copy) | One authorized peer copy through a supported backend path; interrupt/resume, receiver verification and restore; no remote pruning. |
| [V0.4](../../../project-planning/projects/peervault-release-plan.md#v04--keep-both-experiment) | Separate sync feasibility branch: score native/wrapper/impossible controls, fault seam and license posture; record reuse/custom-protocol/backup-only fallback decision. Experiment is not supported sync. |

Backup proof order: R0 → V0.1 → V0.2 → V0.3. V0.4 is an early separate sync
decision branch. The backend owns encryption, dedup, repository format
and maintenance. A custom store/crypto format and unbuilt shared components are
not prerequisites. Use exact versions chosen and rechecked during R0, rather than
treating historical research versions as current dependencies.

## Finish the standalone product

| Stage | Result and acceptance |
|---|---|
| [V0.5](../../../project-planning/projects/peervault-release-plan.md#v05--retention-without-destroying-recovery) | Retention preview and safe backend maintenance; pin each required peer's last verified/restored copy until its replacement passes; protect active work and pass crash/concurrency tests. |
| [V0.6](../../../project-planning/projects/peervault-release-plan.md#v06--supported-backup-and-peer-beta-on-both-platforms) | Qualified Windows/NTFS and Linux/ext4 backup/replication, metadata/path matrix, portable-subset restore, upgrade and scoped power-loss drills. |
| [V0.7](../../../project-planning/projects/peervault-release-plan.md#v07--supported-opt-in-live-folder-sync) | Separate supported-sync branch for full 1.0: reviewed offline/rejoin/deletion rules and durable pre-delete conflict bridge; can remain open while backup-only 0.9 ships. |
| [V0.8](../../../project-planning/projects/peervault-release-plan.md#v08--scheduling-status-and-windows-desktop) | Windows C# desktop, Linux CLI scheduling and opt-in full-read/scratch-restore jobs over one core, with owner-only IPC, freshness and restart safety. |
| [V0.9](../../../project-planning/projects/peervault-release-plan.md#v09--operational-beta-and-measured-recovery) | At least 14 days and three clean drills, then supported backup-only 0.9 under full subset RC/security/packaging/support gates; sync excluded. |
| [RC → 1.0](../../../project-planning/projects/peervault-release-plan.md#10-rc-and-supported-release) | Frozen compatibility/support, security review, clean packaged recovery, license/notices/SBOM, publisher verification, owner go/no-go and verified publication. |

**1.0:** one owner can back up folders, inspect history, restore exact supported
bytes on a clean device, maintain a verified peer copy, resolve sync conflicts,
schedule runs and use a Windows desktop or Linux CLI. Support is limited to the
published backend/OS/filesystem/metadata/topology matrix. Supported backup-only
0.9 ships independently of sync qualification; full 1.0 still requires V0.7 and
combined beta/RC tests. No unqualified feature is silently relabelled supported.

Full-read verification, a clean restore and application usability are separate
evidence. Local data loss, lost keys, stale peers, incomplete scans and unsupported
selected content must be visible. Device removal cannot recall already copied
keys/plaintext. A local snapshot alone is not an independent backup.

## Integration and later scope

[Integration tracks](../../../project-planning/projects/peervault-release-plan.md#capability-accounting-and-integration-tracks)
qualify ModGrid/CloudMesh/PersonalMesh and named application recovery separately,
using pinned released contracts and application-owned restore oracles. PersonalMesh
version numbers and D-0006/D-0007 ordering remain independent of this roadmap.
Standalone 1.0 works without those ecosystems or mandatory SaaS.

[After 1.0](../../../project-planning/projects/peervault-release-plan.md#after-10):
support/security updates, recurring restore drills, broader recovery inventories,
offsite copies, new profiles and a separately reviewed custom-store track.
Multi-user collaboration, public sharing and arbitrary live-app consistency need
new scope decisions. The nine named capability areas are concerns in this product,
not services to scaffold.

Next action: implement V0.1 against the selected restic adapter. The remaining
R0 residuals — observed-phase faults, index/access/disk-full cases, production
containment, Linux independent-media copy and the full redistribution matrix —
stay open and tracked in ADR 0001. The local copy drill covers native copy, ID
mapping, source absence and receiver restore for both candidates on one host; it
is partial evidence only.
