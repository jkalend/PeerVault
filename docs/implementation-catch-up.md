# PeerVault implementation catch-up plan

Implementation follow-up: [CP0/CP1 record](cp0-cp1-progress.md) documents the applied
harness fixes and actual local checks. The assessment below retains its original
baseline; later source/evidence does not turn the remaining CP2+ gates into passes.

Status: proposed, 2026-10-29. This addresses the
[DeepSeek/GLM/Kimi review](implementation-plan-review.md) against implementation
that began after the original planning review. It is a static gap assessment and
ordered implementation handoff. This pass changes documentation only; it does
not establish that backend, CI or production acceptance tests pass.

The [release plan](../../../project-planning/projects/peervault-release-plan.md)
owns release acceptance; linked [execution contracts](../../../project-planning/projects/peervault-contracts.md)
and [qualification protocol](../../../project-planning/projects/peervault-validation.md)
define the revised detail. All review findings below have a planning disposition;
implementation and experimental evidence remain open where identified. These
working-tree documents and source additions are not claimed committed or pushed.

## Inspected implementation baseline

The checkout HEAD is `46925db815e43e00d554cad9dd0c41fa8e8cc90c`. Its working tree
now contains a Rust `0.0.1` R0 tool with `fixture`, `compare`, and `bakeoff`
commands, a pinned toolchain/lockfile, exact backend archive pins, fixture goldens
and CI/package definitions. Preserve this work. Nothing inspected implements a
production profile/snapshot/restore CLI, peer replication, retention, sync,
credentials/recovery kits, desktop or scheduled proof jobs.

| Existing asset | Reuse / observed coverage | Qualification gap |
|---|---|---|
| [CLI](../src/main.rs) | Explicit synthetic-only fixture/compare/bakeoff surface | No public adapter/report contract or production commands; do not turn bakeoff into a user backup path |
| [Fixture generator/comparator (Components crate)](../../Components/crates/bailiff/src/fixture.rs), [golden A](../tests/fixtures/v1-a.json), [golden B](../tests/fixtures/v1-b.json) | Exact xorshift/seed/names/size mutation, sorted SHA-256 manifests, path/type checks, malformed-input unit tests | Independently hash the full generated corpus; golden fixture does not cover live sources, hostile extraction, >4 GiB or many-file workloads |
| [Backend runner (Components crate)](../../Components/crates/bailiff/src/backend.rs) | Version checks, isolated backend environment, actual repository-vs-snapshot verification label, capture/list/restore, wrong-secret, reconnect and saved-object corruption exercises | No scored copy/containment/profile ADR; fresh local config is not independent-device recovery; native restore accepts self-generated fixtures only |
| Same runner's process helper | 300 s timeout, 4 MiB output threshold, temp output files, child cleanup and output-withholding tests | Direct child cleanup is not process-tree/cancellation reconciliation; polling spool is not a hard disk bound; no combined RSS/handle budget |
| [Installer](../scripts/install-backends.py), [backend pins](../backend-lock.json) | Archive SHA-256 verification and named-member extraction | No pin-file-to-runner version coupling or pinned extracted-binary verification; cached/decompressed size caps and atomic failure behavior need explicit checks |
| [CI](../.github/workflows/ci.yml) | Windows/Linux build/lint/test/bakeoff definitions and experimental packaging | No run evidence inspected; Windows Server runner is not Windows 11 qualification; docs/schema/profile/evidence checks missing; 30-day CI artifacts are not durable gate evidence |

At this inspection, CLI and CI referred to `docs/r0.md`, which was not present.
CP0 must recheck and repair that documentation/package dependency; another active
implementation change may already have supplied it. No source edit is requested
by this plan. The current runner already labels reports “not full R0
qualification”; preserve that distinction.

Retained SHA-256 baseline (uppercase hex, exact working-tree bytes at review time).
2026-11-01: the harness moved to `Components/crates/bailiff`; rows below keep the
pre-extraction paths and describe the reviewed snapshot, not the current tree.

| Input | SHA-256 |
|---|---|
| Cargo.toml | `F0C93708153BF86EDA44EF05E69FDB4C25413082A3BB41F2B55809EFFBA30A8E` |
| Cargo.lock | `288C34DE8F9BF1E578E9D4237967354C5120A7A4D34CD51639C75D73675B0D49` |
| rust-toolchain.toml | `C910997CB152C6DC8ED13B1FDF9FA28A4ED30D6776123E27DD6170E5C66067A0` |
| backend-lock.json | `3CEAF7AA38105ACC0EE067B0A23B88FBBD413490A975F6C771B7D2AAD1A4DCAF` |
| src/main.rs | `907BD1A73F80B672A72772939921F22F38CEB022E66440064E8A7F1CF7E86B64` |
| src/fixture.rs | `DF761EE210D96D20C8D7DC61A06640D4CF85F51D4E74867D033414D4677F4861` |
| src/backend.rs | `D731C71C1B962CCB9921F48CF872AEE7A9FFB3DFEF94E62A17DE449E1D900E44` |
| scripts/install-backends.py | `FB2719883CCACF9342F6D1588B40D4B5AC70C1D0AE04BB8C07BD087F6ACA0C9A` |
| tests/fixtures/v1-a.json | `9731533C6970876599588E1439886226DCF9C1DA4155558145987759A6162242` |
| tests/fixtures/v1-b.json | `8FF269081AE3875BCCA564143FD143FEFC8102214DF96CE68590D81EF4E6F5EB` |
| .github/workflows/ci.yml | `A94CD3A619AB644A569653E6C129C30B9C799AD2F0F4B48A821FB33A59813B0C` |

If these inputs change, retain this as an earlier observation, inspect the new
diff and supersede only affected tasks/evidence. Do not reset the working tree
to reproduce this baseline. The review's older input hashes identify its original
document snapshot; they are not hashes of this revised plan.

## Release decision and ordering

Keep full 1.0's opt-in sync requirement. Add a supported **backup-only 0.9** release
with the same security, compatibility, clean-host recovery, distribution and
support gates for its narrower advertised scope. This accepts PVR-08's alternative
without reducing the user's full 1.0 destination. V0.4 decides sync feasibility
early; an unqualified V0.7 cannot block backup-only desktop/scheduling or 0.9.
Experimental R0 packages are not the supported 0.9 release.

Immediate sequence: **CP0 → CP1 → CP2 → R0 decision → CP3 → CP4 → CP5 → CP6**.
CP7's sync feasibility matrix can begin after CP2 and stays a separate branch;
it must pass before supporting sync. CP8 ships backup UI/scheduling/proof cadence
and supported 0.9, then combined sync qualification/RC is required for full 1.0.
One implementation slice at a time; no desktop, daemon, database framework or sync
protocol scaffolding is required to finish CP0–CP2.

## CP0 — Reconcile the moving baseline and entry documentation

Owner: PeerVault implementation maintainer. Required before more scored R0 runs.

- Recheck current status/digests, newly added docs and any existing evidence;
  preserve concurrent implementation. This plan is a delta over the inspected
  harness, not a request to restart it.
- Align README/agent status with experimental R0, link this catch-up and review
  dispositions, and keep real R0 commands separate from illustrative future CLI.
  Supply or correct the `docs/r0.md` reference used by CLI/packaging; execute a
  package smoke only after all referenced members exist.
- Record exact Windows build/update/NTFS settings and the Ubuntu 24.04 LTS
  image/kernel/ext4 profile before scoring. Hosted CI labels count as smoke only.
- Create the R0 threat-matrix and append-only evidence-manifest templates in the
  [specified registry](../../../project-planning/projects/peervault-validation.md#evidence-registry-ci-and-review-checkpoints),
  pin planning/source inputs and document missing historical evidence explicitly.

Exit: entry artifacts complete, current docs/package references resolve, source
baseline and untested cases recorded. PVR-07/19/26/28/31/33 cannot be considered
runtime-complete merely because their templates now exist.

## CP1 — Harden and reuse the existing fixture/tool harness

Owner: PeerVault harness maintainer. Required before using results to select a backend.

- Retain fixture v1 and its golden bytes. Independently enumerate/hash the full
  generated A/B corpus outside backend/restore code, including exact counts and
  B's length-preserving insertion/deletion. Version intentional fixture changes.
- Make backend pins one source for runner version and installer checks. Bind the
  accepted executable version **and extracted executable hash** to approved pins,
  not just record whichever binary `--executable` supplied. Mismatch must refuse
  execution. Recheck selected exact release/license/security inputs before adoption.
- Bound cached archive reads and named-member decompression as well as download;
  verify member type, size, archive digest and extracted digest before atomic install.
  Test oversized/corrupt/truncated cache/member, wrong platform/version and failed
  install preserving the prior valid executable. No unbounded decompression.
- Keep current output-withholding and timeout behavior. Add process-tree ownership,
  cancellation/reaping and disk-spool overshoot limits; record partial/unknown
  outcome when a child dies after a possible mutation. Preserve the 4 MiB/300 s
  R0 profile rather than silently raising it to hide a backend failure.
- Emit complete diagnostic evidence for setup/limit/fault failures, with exact
  phase/case, command reference, environment, profile/hash and assertion outcomes;
  map the existing experimental JSON explicitly into the registry format.

Exit: existing unit/process tests still pass plus focused negative checks above;
goldens independent of restore pass; version/pin drift and overflow cannot produce
a passing R0 record. Tests here are planned, not reported run by this document.

## CP2 — Complete scored R0; do not mistake the baseline for the gate

Owner: PeerVault backend/profile maintainer. Blocks production V0.1.

- For **both** candidates retain exact baseline evidence and score real verification
  scope. Keep restic's existing repository label and Kopia's attempted snapshot
  command explicit; validate coverage against exact builds, not a hardcoded label.
- Prove source A / peer B / recovery C copy topology, IDs/mapping, key role,
  supported transport, destination guard, no-delete behavior, C's exact access
  to B and separate physical media. A fresh client config against the same local
  repository proves neither peer replication nor clean-device recovery.
- Select the production restore model: adapter-streamed validated entries by
  default, or separately qualified sandboxed backend extraction. Current native
  restores are limited to self-created fixtures and are reusable as baseline tests;
  they do not prove containment for a supplied repository. Run hostile-entry,
  link/reparse race, target-parent race and outside-write assertions before selection.
- Extend timed startup interruption to observable phase-targeted faults; add
  index loss/rebuild, denied access and disposable disk-full cases. Record phases
  that cannot be triggered as untested. Keep VM power-loss/clock/storage evidence
  separate; add required candidate/topology stress cases without host-clock changes.
- Complete license/redistribution/acquisition matrix and backend ADR. A passing
  local baseline cannot overrule a failed required copy/export/recovery capability.

Exit: scored candidate matrix, complete R0 entry/evidence bundle and one selected
backend/profile ADR, or explicit no-selection outcome. No generic backend plugin
system or custom encrypted store is needed to reach this decision.

## CP3 — Build the production V0.1 contract around the proven path

Owner: PeerVault core maintainer. Enter only after CP2/R0 selection.

- Add the needed BackendAdapter v2 blocks as concrete functions over the selected
  backend; keep R0 fixture/bakeoff commands as separate experimental operations.
  Freeze JSON Schema, golden reports, CLI exit codes and cancellation/reconciliation.
- Validate typed backend output. Current history substring matching and loosely
  defaulted/missing JSON fields cannot prove complete enumeration or exact snapshot
  presence. Add fixtures for missing/wrong-type/error fields and malformed output.
- Implement repository binding/identity, run UUID/tags, policy revision/hash and
  capture classes; retain producer versus executing backend version. Use explicit
  references or defined validated selectors; reconcile unknown captures before retry.
- Choose/record the proposed local SQLite authority/cache split, versioned migrations,
  OS-owned repository lock and backend lock; no age-only unlock. R0's disposable
  `restic unlock` path must not become a general production recovery operation.
- Implement source overlap/media review, no-auto-hydration/open-file accounting,
  encrypted tree oracle/per-file evidence, secure staging/writes and exact post-restore
  hashing. Restrict initial restore to whole snapshot/new target; unsupported
  selected entries leave needs-attention.
- Freeze provisional production caps/clock probe and per-OS secret delivery.
  Add Windows/Linux schema/report/fixture/link/pin checks to CI. Preserve the R0
  comparator's small profile; large-file production support needs its own profile.

Exit: V0.1 boundary/failure/restore tests pass on named Windows and Linux profiles,
including incomplete scan, unsafe paths, unknown outcome, state corruption/migration
and lock contention; no passing byte-restore without a usable oracle.

## CP4 — Deliver independent recovery and its artifacts

Owner: PeerVault recovery maintainer. Required for V0.2.

Implement RecoveryKit v1 generate/verify, exact offline binaries/provenance, encrypted
private evidence, selected independent secret custody and cleanup/rotation runbooks.
Use a clean device/profile with no network, source disk, source config or cached
keys; restore from distinct surviving copy media. Exercise lost/wrong data/secret,
altered kit and historical-key scenarios. Include another operator; otherwise mark
author-only evidence and keep supported-release usability acceptance open.

Exit: both OS recovery drills and material-loss cases have retained evidence;
password change, transport revocation and actual data-key migration are distinct.
The existing `reconnect()` is a component test, not completion of this gate.

## CP5 — Qualify one peer before adding deletion

Owner: PeerVault replication maintainer. Required for V0.3.

Implement the CP2-approved transport/topology, PeerCopyRecord states and copy-ID
mapping; verify actual transfer accounting, authorized full read and clean restore
as separate results. Keyless peers require the qualified recovery-device access
path; otherwise advertise only the explicitly qualified key-holding topology.
Review peer authentication/removal/credential exposure independently before enabling.

Exit: restart/ack-loss/offline/wrong-mount/corruption tests pass; source-disk absence
still recovers with separate material. All transfer behavior remains additive.

## CP6 — Enable safe retention and qualify backup support

Owner: PeerVault storage maintainer. Required for V0.5/V0.6.

Implement versioned RetentionPlan/PeerRoot records and transactional root/evidence
advance, independent record backup, key-version availability, default history floor
and mass-change pause. Preserve each required peer's last verified/restored copy
until its replacement passes or an explicit coverage-abandonment action is reviewed.
Lost authoritative state defers deletion. Use backend safe maintenance only, with
security review, inspectable preview and concurrent work/root revalidation.

Then qualify the published metadata/placeholder/open-file/case matrix, 5 GiB and
50,000-file profiles, upgrades/migrations, old snapshot readability and scoped VM
power loss. Whole-tree overwrite/merge and arbitrary application consistency remain
excluded. Independent backup support requires distinct-media recovery evidence.

Exit: all V0.5/V0.6 cases and critical/high safety findings are resolved with
retests. Local-newer/peer-stale plus failed transfer/restore/maintenance cannot
destroy the last usable peer copy.

## CP7 — Decide and qualify sync on its own branch

Owner: PeerVault sync maintainer. V0.4 decision early; V0.7 required only for full 1.0.

Score every required protocol behavior native/wrapper-enforceable/impossible,
including clock-dependent conflict names, causal identity, tombstone-horizon source,
offline quarantine/rejoin, pre-delete/version-store bridge and deterministic fault
injection. Confirm licensing, separate-process/embedding posture, manual LAN pairing,
disabled public discovery/relay/telemetry and bounded/redacted engine logs. Record
reuse versus separately reviewed custom-protocol versus supported-0.9 fallback ADR.

Before any destructive sync can remove divergent bytes, persist/pin them in backup
or a separately qualified engine-version store. A next-backup timer is insufficient.
If the engine cannot enforce this, do not enable that profile. Implement/review only
the chosen viable path; test conflict→immediate delete→crash, partitions, duplicate/
reordered operations, clock skew and stale/re-added peer with repeatable test seams.

Exit: either an evidence-backed no-supported-sync decision (0.9 still ships), or
the independently reviewed V0.7 preservation/convergence/rejoin matrix passes.
No fictitious wrapper-level causal guarantee over an opaque engine.

## CP8 — Ship recurring recovery workflow, then supported releases

Owner: PeerVault product/release maintainer. Backup work can proceed while CP7 is open.

Implement Windows desktop/Linux CLI scheduling over the same core authority,
owner-only IPC/caller checks and current destructive-preview tokens. Include
opt-in full-content verification and bounded whole-set scratch restore jobs with
explicit cadence/freshness defaults, manual-drill fallback and stale/unknown status.
Test restart, task identity/secrets, missed schedules, quota, output bounds and
clock jumps. Apply the severity rubric and early review register throughout.

Run beta and independent recovery usability checks, then supported backup-only
0.9 RC gates: exact artifact/source/profile evidence, clean installs, upgrades,
old snapshots, security review, license/notices/SBOM, publisher verification and
support policy. Retain evidence beyond CI expiry. Advertise sync only after CP7;
full 1.0 repeats affected combined beta/RC tests and tests the published bytes.
Publishing remains a separately authorized release action.

## Review dispositions

All dispositions below address **planning**. `Accepted` means a concrete requirement/
decision and task now exist, not that the implementation or experiment passed.
Original findings and severity remain preserved in the review artifact.

| ID | Disposition and plan repair | Implementation / exit owner |
|---|---|---|
| PVR-01 | Accepted: A/B/C topology, actual transfer/full-read/restore predicates and keyless fallback | CP2 scores, CP5 implements |
| PVR-02 | Accepted: V0.4 scored engine/protocol/kill/fallback ADR, exact-build fault seam and log/network/license policy | CP7; cannot block supported 0.9 |
| PVR-03 | Accepted: BackendAdapter v2 capability blocks, report/outcome/exit schemas, operation lifecycle and state choice | CP3, later blocks CP4–CP6 |
| PVR-04 | Accepted: production streaming/contained extraction choice scored before selection; preflight, staging and handle safety | CP2 / CP3; current native fixture restore remains diagnostic |
| PVR-05 | Accepted: bounded RecoveryKit generator/verifier and verified offline binaries, refs and evidence | CP4 |
| PVR-06 | Accepted: owner/product/backend responsibility split; per-OS secret store/delivery and separate rotation/revocation routes | CP3 / CP4 / CP8 |
| PVR-07 | Accepted: required threat/survival rows, physical media and residual compromise/ransomware limits | CP0 template, CP2/CP4 evidence |
| PVR-08 | Accepted alternative: retain full 1.0 sync scope, support backup-only 0.9 with full subset release gates | CP7 branch decision, CP8 support/release |
| PVR-09 | Accepted: encrypted CapturedTree oracle/provenance, exact re-hash, backend-reported downgrade without oracle | CP3 |
| PVR-10 | Accepted: init/source overlap/media guard and explicit same/unknown-domain acknowledgement; source-disk-absent drill | CP3 / CP4 |
| PVR-11 | Accepted: local SQLite authority/cache split, OS-owned + backend locks, child-tree reconciliation and owner-only IPC | CP3 / CP8 |
| PVR-12 | Accepted, partly present: retain exact generator/goldens; independent source hashing and observed-phase/fault harness still required | CP1 / CP2 |
| PVR-13 | Accepted: placeholder/open-file, no-auto-hydrate/VSS/share scope, collision rejection, 5 GiB/many-file profiles | CP3 / CP6 |
| PVR-14 | Accepted: selected separate offline material/custodian, cleanup and early independent-operator gate | CP4; author-only remains labelled |
| PVR-15 | Accepted: both candidates' copy/key/ID/delete/verification topology scored before selection | CP2 |
| PVR-16 | Accepted scope option: 1.0 whole-snapshot/fresh-target only; selective/merge/overwrite/resume excluded, manual scratch-copy boundary documented | CP3 / CP8 docs |
| PVR-17 | Accepted: canonical policy rev/hash, backend metadata linkage, selectors, copy mapping, producer/observer versions | CP2 / CP3 / CP5 |
| PVR-18 | Accepted: peer/root/retention/sync field tables, transactional root advance, key refs and lost-local-state deletion deferral | CP5 / CP6 / CP7 |
| PVR-19 | Accepted, CI partly present: append-only run manifest/registry and required CI/profile/schema/link checks | CP0 / CP1 / CP3 / CP8 |
| PVR-20 | Accepted: early V0.3/V0.5/V0.7 review checkpoints and concrete severity/register/closure rules | CP5 / CP6 / CP7 / CP8 |
| PVR-21 | Accepted: clock enum/probe/offset/age/jump rules, no backend threshold inferred from UI defaults | CP2 / CP3 / CP8 |
| PVR-22 | Accepted, R0 partly bounded: provisional numeric production caps with explicit overflow/reap/staging behavior | CP1 / CP3 / CP6 |
| PVR-23 | Accepted: exact verified binary/format/acquisition policy, CLI external pack/desktop-kit redistribution gate and update requalification | CP1 / CP2 / CP4 / CP8 |
| PVR-24 | Accepted: default 3-point/30-day floor plus root pins, default mass-change pause and revisioned preview/override | CP6 |
| PVR-25 | Accepted, strengthened: durable pre-destructive version bridge; timer alone cannot guarantee preservation | CP7 |
| PVR-26 | Accepted: status/demo/selector/scope/version/dossier reconciliation and canonical protocol pointers | Documentation repaired; CP0 rechecks moving source/docs |
| PVR-27 | Accepted: license/redistribution at backend/sync choice and notices/SBOM before packaging | CP2 / CP3 / CP7 / CP8 |
| PVR-28 | Accepted: capability names remain concerns, release acceptance lives in one detailed plan and CI checks summary/links | Documentation repaired; CP3 CI |
| PVR-29 | Accepted, source partly aware: actual verification_scope/stage/outcome/cost; repo-wide inclusion check, no silent restore substitution | CP2 / CP3 |
| PVR-30 | Accepted: opt-in scheduled full read and bounded scratch restore, freshness defaults/manual fallback | CP8 |
| PVR-31 | Accepted: exact installed Windows/Ubuntu profile fields required before first scored run; revisions invalidate affected results | CP0 / CP2 |
| PVR-32 | Accepted: serialized stable/live/crash/app capture classes and revision/evidence semantics | CP3 |
| PVR-33 | Accepted: roadmap/release/review links, historical digest labels, this complete ledger and moving implementation baseline | Documentation repaired; no runtime clearance implied |

Appendix-only GLM F15 / Kimi #17 is also addressed: IPC has an explicit owner-only
contract, caller ACL checks and revision-bound single-use destructive tokens in
the execution contracts and CP8. Crosswalk findings are not lost merely because
they lack another consolidated PVR ID.

GPT-6.1 Terra/xhigh statically rechecked this catch-up, contract changes and source
mapping. Its additional canonical dependency contradiction was repaired and
rechecked: additive replication, backup pruning and sync no longer share one
blanket prerequisite. The final recheck found no remaining planning blocker in
that review scope; executable gates and the tasks above remain open.

## What can start next

Start CP0/CP1 against the current worktree. Keep every useful existing fixture,
negative process test and backend baseline; close missing qualification evidence
before implementing production V0.1. No current R0 test has to become a generic
framework. The next code assignment is the bounded harness/profile delta above,
not the entire V0–1.0 destination.
