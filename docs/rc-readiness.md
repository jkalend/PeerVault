# RC readiness — honest gate status

Status: **RC not reached**. The implementation now covers R0 through an
owner-executed V0.5 retention plan, plus a V0.6 corpus prerequisite and a partial
V0.8 scheduler/desktop. V0.6 beta qualification, complete V0.8 IPC/product
surface, V0.9 measured-recovery beta and the RC gates remain open. No gate is
marked passed from source presence, CI definitions or a partial drill.

## Stage matrix

| Stage | Implemented | Evidence / limits | Gate |
|---|---|---|---|
| R0 | restic 0.19.1 selection, scored Windows/Ubuntu profiles, copy drill, threat/evidence entry artifacts | ADR 0001; four scored runs retained | selection recorded; fault/containment residuals open |
| V0.1 | init/capture/history/verify/restore, encrypted oracle, contained tar restore | Windows + Ubuntu smoke; two exact restores; CI smoke | local synthetic scope; arbitrary-archive, index/access/disk-full and app consistency open |
| V0.2 | RecoveryKit generate/verify, author-only drill and fresh network-isolated Linux container recovery | Windows + Ubuntu drills; container full-read/restore; kit smoke | Windows clean profile, independent operator/hardware and rotation remain open |
| V0.3 | one-peer additive copy record, source removal, peer full read, clean restore | Windows + Ubuntu local-filesystem drills | no authenticated transport/pairing/keyless receiver or independent hardware |
| V0.4 | backup-only 0.9 decision | ADR 0002 | supported sync intentionally not claimed |
| V0.5 | preview/apply with state generation, owner lock, peer-root pins, floors, mass-change pause and backend-native forget/prune | unit tests + same-host drill; stale-plan replay, mass-change refusal and exact post-apply restore | independent security review, concurrent capture/restore, disk-full/kill-at-phase, clock/listing and remote-peer cases open |
| V0.6 | 5 GiB/50k corpus prerequisite; metadata/path matrix; cancellation; bidirectional cross-OS; previous-commit upgrade/rollback; quota-style capacity drill | `docs/v0.6-corpus.md`, `docs/v0.6-matrix.md`; Windows NTFS + Ubuntu 22.04 WSL2 ext4 | released-artifact upgrade, power loss, physical disk-full/index rebuild, target Ubuntu 24.04, independent beta open |
| V0.7 | deferred | ADR 0002 | not qualified |
| V0.8 | scheduler/status core, owner-only IPC, desktop operations, OS scheduler installers, DPAPI/systemd credentials, OS lock release, free-space preflight | unit tests; IPC, schedule/clock/quota/lock, DPAPI drills; Task Scheduler/systemd dry runs; C# build | retention apply, backend-journal restart reconciliation, physical storage stress, cross-user/task identity, accessibility and second-operator evidence open |
| V0.9 | five-run same-host capture/full-read/restore measurement | `docs/v0.9-measure.md` (64 MiB, debug and release builds) | 14-day/three-clean-drill beta, RSS/CPU and independent evidence missing |
| RC → 1.0 | not reached | none | frozen compatibility/support, independent security review, packaged clean recovery, license/SBOM, publisher verification and owner go/no-go missing |

## What can be demonstrated today

- Pinned restic repository-v2 capture, history, repository full read and exact
  fresh-target restore on Windows/NTFS and Ubuntu 24.04/ext4.
- An encrypted tree oracle with stable capture-ID mapping across backend copy.
- A secret-free RecoveryKit and an author-only copy/source-removal/recovery drill.
- A `PeerCopyRecord v1` additive local peer-copy state trail.
- An owner-executed retention plan that protects floors, missing-oracle history
  and required peer roots; stale or drifted plans are refused.
- A declared metadata/path subset with supported, rejected and lossy cases on
  Windows/NTFS and Ubuntu 22.04/ext4; prior snapshots survive refused captures.
- Cancellation of a large capture exits `130`, leaks no complete oracle, and
  refuses concurrent capture/verify through the repository lock.
- A versioned owner-only named pipe/Unix socket, same-user SID/UID checks,
  an operation allowlist and desktop capture/history/verify/restore/preview.
- A five-run repeated capture/full-read/restore measurement with raw
  observations and descriptive median/p95.
- CI on windows-2025 and ubuntu-24.04 for the R0 smoke, V0.1 restore, V0.2 kit,
  V0.3 peer record, V0.5 retention, V0.6 matrix/cancellation and V0.8 IPC drills.

## Blocking RC

1. **Independent evidence**: second operator, clean hardware, authenticated
   network transport/pairing, physical source-disk removal and 14-day beta.
2. **Fault qualification**: observable-phase crashes, power loss, index rebuild,
   denied access, disk full, clock/storage stress and hostile archive containment.
3. **Product surface**: scheduling/status and the Windows desktop with owner-only
   IPC; no supported GUI exists.
4. **Release engineering**: independent security review, frozen compatibility and
   support matrix, packaged clean recovery, publisher verification, signed
   artifacts and owner go/no-go. The bailiff/Components Apache-2.0 relicensing is
   committed locally; its commit and the updated PeerVault pin must be published
   before CI/release can fetch it.
5. **Retention**: deletion/maintenance exists, but the independent security
   checkpoint, concurrent capture/restore/transfer, disk-full, kill-at-phase,
   clock/listing and remote-peer-root cases are not qualified.

## Honest stop point

The repository is an experimental implementation, not an RC. Calling the current
state RC would violate the plan's evidence rules and the product's data-durability
invariants. The next actionable engineering work is listed in the stage matrix;
the human-only and independent gates cannot be manufactured by code in this
repository.
