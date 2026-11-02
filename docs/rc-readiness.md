# RC readiness — honest gate status

Status: **RC not reached**. The implementation now covers R0 through the V0.5
retention preview plus a V0.6 corpus prerequisite. V0.6 beta qualification,
V0.8 desktop, V0.9 measured-recovery beta and the RC gates remain open. No gate is
marked passed from source presence, CI definitions or a partial drill.

## Stage matrix

| Stage | Implemented | Evidence / limits | Gate |
|---|---|---|---|
| R0 | restic 0.19.1 selection, scored Windows/Ubuntu profiles, copy drill, threat/evidence entry artifacts | ADR 0001; four scored runs retained | selection recorded; fault/containment residuals open |
| V0.1 | init/capture/history/verify/restore, encrypted oracle, contained tar restore | Windows + Ubuntu smoke; two exact restores; CI smoke | local synthetic scope; arbitrary-archive, index/access/disk-full and app consistency open |
| V0.2 | RecoveryKit generate/verify, author-only drill and fresh network-isolated Linux container recovery | Windows + Ubuntu drills; container full-read/restore; kit smoke | Windows clean profile, independent operator/hardware and rotation remain open |
| V0.3 | one-peer additive copy record, source removal, peer full read, clean restore | Windows + Ubuntu local-filesystem drills | no authenticated transport/pairing/keyless receiver or independent hardware |
| V0.4 | backup-only 0.9 decision | ADR 0002 | supported sync intentionally not claimed |
| V0.5 | read-only retention preview | unit tests + preview smoke; no mutation | no deletion, maintenance, locks, peer roots or GC command |
| V0.6 | 5 GiB/50k corpus prerequisite | see `docs/v0.6-corpus.md` | metadata/path matrix, power loss, independent beta and same-OS both-platform qualification open |
| V0.7 | deferred | ADR 0002 | not qualified |
| V0.8 | scheduler/status core, owner-only stdio agent and thin WinForms status/tick shell | unit tests, CLI/agent smokes, desktop build | versioned named-pipe/socket IPC, full desktop operations, OS scheduling installers, secret delivery and restart/clock/quota evidence open |
| V0.9 | not implemented | none | 14-day/three-drill beta evidence missing |
| RC → 1.0 | not reached | none | frozen compatibility/support, independent security review, packaged clean recovery, license/SBOM, publisher verification and owner go/no-go missing |

## What can be demonstrated today

- Pinned restic repository-v2 capture, history, repository full read and exact
  fresh-target restore on Windows/NTFS and Ubuntu 24.04/ext4.
- An encrypted tree oracle with stable capture-ID mapping across backend copy.
- A secret-free RecoveryKit and an author-only copy/source-removal/recovery drill.
- A `PeerCopyRecord v1` additive local peer-copy state trail.
- A read-only retention preview that protects missing-oracle history.
- CI on windows-2025 and ubuntu-24.04 for the R0 smoke, V0.1 restore, V0.2 kit and
  V0.3 peer record.

## Blocking RC

1. **Independent evidence**: second operator, clean hardware, authenticated
   network transport/pairing, physical source-disk removal and 14-day beta.
2. **Fault qualification**: observable-phase crashes, power loss, index rebuild,
   denied access, disk full, clock/storage stress and hostile archive containment.
3. **Product surface**: scheduling/status and the Windows desktop with owner-only
   IPC; no supported GUI exists.
4. **Release engineering**: independent security review, frozen compatibility and
   support matrix, packaged clean recovery, license/notices/SBOM, publisher
   verification, signed artifacts and owner go/no-go.
5. **Retention**: no deletion/maintenance implementation; the preview is read-only
   and does not evaluate locks, peer roots or recovery pins.

## Honest stop point

The repository is an experimental implementation, not an RC. Calling the current
state RC would violate the plan's evidence rules and the product's data-durability
invariants. The next actionable engineering work is listed in the stage matrix;
the human-only and independent gates cannot be manufactured by code in this
repository.
