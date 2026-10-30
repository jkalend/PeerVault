---
title: PeerVault implementation plan — independent review
date: 2026-10-29
status: review
disposition: planning-addressed; implementation-and-qualification-open
scope: >-
  docs/implementation-plan.md and project-planning/projects/peervault-release-plan.md,
  cross-checked against the canonical design, README, AGENTS.md and evidence dossier.
reviewers:
  - omp (litellm/deepseek-v4.1-flash)
  - GLM-5.3-Flash (independent)
  - Kimi K3 (independent; see appendix)
verdict: >-
  R0 can start now, but its exit is not judgeable until the fixture spec and
  threat-matrix template are defined as R0 entry tasks. V0.1 is executable after
  the contract and containment fixes below. V0.2–V0.7 have unresolved
  prerequisites. 1.0 as written is not executable because it is gated on an
  unproven sync contract and several unproven engineering bets. The plan is
  technically serious and unusually honest; this review is about closing gaps
  before they become rework, not about replacing the direction.
---

# PeerVault implementation plan — independent review

> This document is a review artifact, not a plan, a commitment or a release
> approval. It records observations against the documents hashed below. The
> original plan owner decides which findings to accept; each finding proposes a
> concrete resolution so that decision can be made without re-research.

## Inputs and method

**Historical reviewed snapshot:** the digests and observations below identify the
documents reviewed on 2026-10-29 before the plan repairs and subsequent source
additions. They are intentionally preserved, not hashes of the revised plan.
See [current dispositions and implementation catch-up](implementation-catch-up.md#review-dispositions)
for all PVR-01–PVR-33, the new source baseline and remaining code/evidence gates.
This review artifact remains worktree-only until committed; linking it is not
proof that any runtime finding passed. Its original verdict/severity are retained
as historical review output; the disposition ledger governs current follow-up.

| Input | Revision / digest |
|---|---|
| Product roadmap | `docs/implementation-plan.md`, SHA-256 `a7d18f52474e4abeb926e6be1ec76979fa00bc7bf43cd7a773458bcd0d2398e7` |
| Product README | `README.md`, SHA-256 `3d556870573ff0e39079c418bf52b12493e9f490717a42903c8dfb842b043d32` |
| Product agent guide | `AGENTS.md`, SHA-256 `82fb3af183bd2dd23544c5a2412856324d4e4b087ba85b661718606b95dfd07d` |
| Detailed release plan | `project-planning/projects/peervault-release-plan.md`, SHA-256 `380ab06cb6cc68c3039dc2ecc4f499c113361c73b874c4cc6287a823574d06ff` |
| Canonical design | `project-planning/projects/peervault.md`, SHA-256 `ef5b3bb31a992ee28542520d9d816e818e355919463ff0b8f27c190d5892acba` |
| Evidence dossier | `project-planning/research/ecosystems/peervault.md`, SHA-256 `4fbd3b8d4992a9c46b6b6977da2b01f41d151a38438bd9b2349e9d3102c18b64` |
| Product git state | `46925db815e43e00d554cad9dd0c41fa8e8cc90c`; README modified; `AGENTS.md` and `docs/` untracked |

Method:

1. Read both review targets in full, including raw reads of lines truncated by the
   reader, then read the design, README, AGENTS and dossier sections cited by the
   plan.
2. Cross-check every cross-document claim that affects a gate: stage ordering,
   acceptance strength, contract operations, evidence states, recovery/key rules,
   peer retention, sync behavior and version claims.
3. Run a relative-link and heading-anchor check across all five documents: 0
   missing files and 0 missing anchors (55 headings across the product and
   planning documents). Link integrity is currently clean; this does not test
   external URLs.
4. Verify specific absences and contradictions by direct text search: there is no
   severity rubric, no evidence-directory convention, no `repository_id`
   derivation, no adapter cancellation/retention/tag operation, no local-state
   engine choice, and the dossier still carries the pre-alignment product claim.
5. No executable test was possible: the product has no source, manifests, CLI,
   backend binary, repository or test tooling. Everything below is a document and
   feasibility review, not a runtime result.

## Verdict

**R0 can start now, but its exit is not judgeable yet.** The candidate comparison
and decision output are sufficiently specified to begin, and the plan correctly
refuses to scaffold the desktop or a custom store first. However, R0's exit
requires an “exact fixture oracle” and a “threat/support matrix” whose contents
are not defined. Those must be R0 **entry** tasks (PVR-07, PVR-12), not findings
discovered when the exit is evaluated. The document set is stronger than typical
planning: evidence states are separated, peer retention is conservative, crash
and power-loss claims are separated, and every executable gate is marked unrun.

**The full 1.0 claim is not executable as written.** Three dependencies are
treated as if they exist but have no defined mechanism:

1. A ciphertext-only peer copy that a separate authorized device can verify and
   restore (PVR-01).
2. A reused sync engine that can satisfy V0.7's causal-ID, tombstone,
   acknowledgement, quarantine and deterministic-conflict contract (PVR-02).
3. An adapter/report/state contract complete enough for cancellation, partial
   outcomes, idempotent retries, retention and history rebuild (PVR-03).

Two further high findings from the independent Kimi K3 review sharpen the risk:
`verify_snapshot(snapshot, full_read)` may not be implementable at snapshot scope
for restic (PVR-29), and nothing in 1.0 produces recurring restore evidence
despite the “regularly prove” destination (PVR-30).

Release-structure issues make the risk worse: 1.0 is gated on sync (PVR-08), and
the recovery/credential/threat-matrix artifacts that 1.0 claims are not product
deliverables with reproducible contents (PVR-05, PVR-06, PVR-07).

The prior planning review's conclusion — “no remaining planning blocker” — is
defensible for the documentation review and for starting R0. It is not a
conclusion about technical feasibility through 1.0. This review does not overturn
the direction; it adds the missing gates and contracts. When findings are
dispositioned, the plan's §Planning review and the roadmap header must be updated
so an implementer does not read only the old “no blocker” claim (PVR-33).

## Severity summary

| Severity | Count | IDs |
|---|---:|---|
| Blocker | 3 | PVR-01, PVR-02, PVR-03 |
| High | 14 | PVR-04 … PVR-14, PVR-19, PVR-29, PVR-30 |
| Medium | 15 | PVR-15 … PVR-18, PVR-20 … PVR-27, PVR-31 … PVR-33 |
| Low | 1 | PVR-28 |

Definition used here:

- **Blocker** — the named stage cannot be executed or judged as written, and the
  plan provides no workaround. Resolve before entering that stage.
- **High** — likely rework, failed acceptance or an unsafe claim if unresolved;
  resolve before the named stage.
- **Medium** — ambiguity or omission that will produce inconsistent
  implementations, evidence or support statements.
- **Low** — consistency, terminology or drift; fix in the next documentation pass.

## Findings by primary stage

| Resolve at | Findings |
|---|---|
| Now (review/scope) | PVR-08 (release split), PVR-33 (review disposition) |
| R0 entry/exit | PVR-04 (restore model/containment), PVR-07 (threat matrix), PVR-12 (fixture/fault harness), PVR-15 (copy path criterion), PVR-19 (evidence registry), PVR-23 (packaging/version), PVR-29 (verification scope), PVR-31 (profile pin) |
| V0.1 | PVR-03 (adapter/report/state contracts), PVR-09 (restore oracle), PVR-10 (same-media guard), PVR-11 (state/locks), PVR-17 (policy/selector/identity), PVR-21 (clock quality), PVR-22 (resource caps), PVR-32 (consistency class) |
| V0.2 | PVR-05 (recovery kit), PVR-06 (credential lifecycle), PVR-14 (recovery material/usability) |
| V0.3 | PVR-01 (ciphertext-only peer verification) |
| V0.4 | PVR-02 (sync decision/engine matrix), PVR-25 (conflict bridge), PVR-27 (licensing) |
| V0.5 | PVR-18 (peer/retention records), PVR-20 (security checkpoints/severity), PVR-24 (retention floor/mass change) |
| V0.6 | PVR-13 (Windows source/restore matrix), PVR-16 (restore scope/overwrite) |
| V0.8 | PVR-30 (recurring proof cadence) |
| Documentation pass | PVR-26 (cross-doc drift), PVR-28 (terminology/duplication) |

## Findings

### PVR-01 — Ciphertext-only peer verification has no executable predicate

**Severity:** blocker · **Origin:** omp (repository-identity overlap: GLM F7) · **Confidence:** high

**Location:** release plan §V0.3 (L211–222), version map (L79); design §Snapshot
publication, recovery and garbage collection (L134); README §What it is.

**Problem.** V0.3 requires: “Verify the selected snapshot on the receiver,
decrypting on a separate authorized recovery device if the peer stores ciphertext
only, and perform a clean-target restore from that copy.” The plan conflates three
different claims:

- transfer completeness on the peer (objects/bytes present);
- full-read verification of the snapshot (which requires repository keys and a
  backend that can decrypt and read every object);
- a clean-target restore with independent byte comparison.

For a ciphertext-only peer, only the first is possible on the peer itself. The
authorized device's access path to the peer's copy is not defined (network mount,
SSH, copy-back, removable media). No backend command is identified that verifies
ciphertext-only content, and the `PeerCopy` contract only says “receiver
verification” without a predicate. The dossier documents Kopia `repository sync-to`
as a repository copy and restic `check --read-data` as a full read; it does not
document a supported ciphertext-only verification path for either. Restic's
repository copy and verification commands require opening the repository, which
requires the password. [INFERENCE: restic `copy` opens both repositories; Kopia
`sync-to` behavior with a keyless receiver should be treated as unproven until R0.]

**Why it matters.** The 1.0 claim “maintain a verified copy on one authorized
peer” and V0.5's per-peer protection root both key on this state. Without a
predicate, “copy exists” will drift into “peer copy verified”, which the plan
elsewhere explicitly forbids. An unimplementable V0.3 also invalidates the V0.5
peer-root design that depends on it.

**Resolution.**

1. Freeze a three-party topology in R0/V0.3: source, peer (ciphertext-only or
   key-holding), authorized recovery device. Name the transport and the access
   path by which the recovery device reads the peer copy.
2. Define `PeerCopyRecord` states with distinct evidence: `requested` →
   `transferring` → `transfer-complete` (peer-local object/byte accounting) →
   `full-read-verified` (authorized device) → `clean-restored` (authorized device,
   independent hashes) → `stale`/`abandoned`.
3. Prove the chosen backend's copy path in R0 before selection, including whether
   the receiver can be keyless, whether snapshot IDs survive the copy, and what
   deletion semantics the copy path exposes.
4. If no supported ciphertext-only path exists, make the peer key-holding in the
   supported topology and document the confidentiality consequence; do not claim
   ciphertext-only verification.

---

### PVR-02 — V0.7 sync semantics exceed a reused engine; no build-vs-reuse gate

**Severity:** blocker · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** release plan §V0.4 (L231–248), §V0.7 (L317–340); short roadmap
V0.4/V0.7 rows (L34, L48); dossier current recheck Syncthing row (L162).

**Problem.** V0.7 requires PeerVault-frozen causal version/operation IDs,
deterministic conflict paths, explicit tombstones with acknowledgements and
expiry, clock-independent reconnect, quarantine/rejoin with a fresh baseline, and
“all distinct versions remain recoverable under declared retention.” The named
comparator, Syncthing, exposes conflict copies and optional per-folder/per-device
versioning that is off by default and only archives changes received from another
device. It does not expose wrapper-visible causal IDs or a tombstone
acknowledgement contract, and deletion propagation and path-bearing logs are
engine-internal. V0.4's exit is “reproducible evidence identifies the viable
engine/profile and its gaps” — an evaluation, not a decision with kill criteria.
The plan's fallback is “hold the gate”, which arrives after V0.5–V0.6 investment.

**Why it matters.** Sync is the hardest and least reusable part of 1.0. A wrapper
can observe an engine's conflict copies but cannot enforce a causal protocol the
engine does not implement. “Hold the gate” at V0.7 leaves no pre-approved product
scope and no decision record.

**Kimi K3 extension (K3-N1).** The reviewer reports three concrete Syncthing
mismatches that must appear in the V0.4 capability matrix, not just the general
“engine may lack controls” concern. These are reported from current Syncthing
documentation and must be verified against the pinned v2.1.7 build before being
treated as facts:

- **(a) Clock-dependent conflict names.** Syncthing names conflict copies
  `<filename>.sync-conflict-<date>-<time>-<modifiedBy>.<ext>`, using wall-clock
  time plus device ID. That cannot “converge under retries” and can produce
  divergent names for the same logical conflict, directly contradicting V0.4's
  invariant that “conflict artifact names and operation identity must converge
  under retries; timestamps alone cannot decide a winner.”
- **(b) No tombstone-expiry signal.** Deletion tombstones live in the engine's
  index without an exposed retention horizon. V0.7's “device offline past
  tombstone retention enters quarantine/rejoin” therefore has no engine signal to
  trigger the wrapper's quarantine logic.
- **(c) No fault-injection seam.** V0.7's exit requires duplicate/reordered
  events, network partition, crash mid-apply and interrupted/repeated rejoin, but
  the engine owns transport. The plan identifies no proxy or test double for those
  events, so the pass criteria are unexecutable.

**Resolution.**

1. Add an explicit sync decision gate at V0.4. Score every V0.7 contract item as
   *native*, *wrapper-enforceable*, or *impossible* for the candidate engine;
   record the matrix and the chosen fallback in an ADR.
2. Pre-approve the fallback now: backup + peer copy without supported live-folder
   sync (PVR-08), or a first-party sync protocol with its own design, security and
   fault-injection gate.
3. If reusing Syncthing, add explicit profile rules for discovery/relay/telemetry
   (no public relays or global discovery in the supported topology), log
   redaction (the engine writes its own logs), and version pinning.
4. Add the conflict-artifact bridge in PVR-25: conflict bytes must reach retained
   backup before a deletion-propagating sync can remove them.
5. Score conflict-name determinism, tombstone-horizon source and fault-injection
   seam explicitly in the V0.4 ADR. If none is native/enforceable, record the
   wrapper rule (rename/reconcile, wrapper-invented horizon, proxy harness) or
   hold the gate.

---

### PVR-03 — Adapter, report and state contracts cannot implement V0.1/V0.5

**Severity:** blocker · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** design §Proposed boundary and identity (L78, L80); release plan
§Implementation shape and contracts (L104–121); V0.1 exit (L181–186); V0.5
(L252–259).

**Problem.** `BackendAdapter v1` provides only `create_snapshot`,
`list_snapshots`, `verify_snapshot`, `restore_snapshot` and `check_repository`.
The plan's own stages require operations it cannot express:

| Required behavior | Missing from the contract |
|---|---|
| V0.1 cancellation | No cancel token, progress event, or defined child-process termination |
| V0.1 retry after timeout/crash | No run/operation ID, idempotency key, or “find snapshot by run” lookup |
| V0.1 policy/evidence binding | No snapshot tags/description parameter; no way to bind a policy revision |
| V0.1 policy freeze | No consistency-class enum or semantics; policy revisions cannot be compared (PVR-32) |
| V0.2 key export/import/rotation | No key/recovery-material operation |
| V0.3 repository identity/copy | No repository-identity/capability call and no copy/transfer operation |
| V0.5 retention/maintenance | No delete/forget, retention preview, or maintenance run operation |
| V0.6 upgrade/recovery | No backend-format/version/capability negotiation |

`RestoreReport v1` records `result = pass | fail`, while the adapter's typed
outcomes are success/failure/partial/unknown and V0.1 explicitly allows an
explicitly partial target. A pass/fail flag cannot represent partial or unknown.
The report also lacks a per-file outcome artifact, byte counters, an enumerated
failure-code taxonomy, exit-code mapping, reader limits and redaction rules. The
local-state row names durable records but chooses no store, schema, migration or
corruption behavior.

**Why it matters.** An implementer will either improvise a second convention or
bypass the frozen contract at V0.5. The plan forbids both. V0.1 acceptance
(cancellation, partial targets) and V0.5 acceptance (retention) are not judgeable
against the named contract.

**Resolution.**

1. Freeze `BackendAdapter v2` before V0.1 with explicit operations and typed
   parameters: cancellation, progress, run identity, tags/description, repository
   identity/capabilities/version range, copy/transfer, retention
   preview/apply, maintenance, key export/import/rotation. Mark optional
   capabilities so a backend can fail closed.
2. Freeze `RestoreReport v1` as a field table plus JSON examples: outcome enum
   (`pass`, `fail`, `partial`, `unknown`), per-file outcome artifact reference,
   byte counts, reason codes, verification mode, policy/run/snapshot refs,
   destination profile, clock quality, limits and schema-version rejection.
3. Define a stable CLI JSON contract and exit-code map at V0.1; version it and
   test golden reports.
4. Decide retention execution: adapter operation, or a separate
   `BackendMaintenance` contract with its own evidence record and lock semantics.
   Do not leave V0.5 to invent it.
5. Choose the local state store and schema in the V0.1 ADR (see PVR-11).

---

### PVR-04 — Restore containment is test-only; no enforcement layer

**Severity:** high · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** release plan §V0.1 (L174–179); design §Data and restore profile
(L108); dossier Borg advisory R24 (L106).

**Problem.** The plan says PeerVault must “check path and link containment for
backend-produced writes” and that “a post-write scan cannot undo an escaped
write.” It then offers only detect-and-narrow: “If the backend cannot safely
restore an accepted profile, narrow the profile or block that adapter.” No
enforcement layer is specified. `restore_snapshot(snapshot, new_destination)`
delegates writes to the backend; there is no pre-flight manifest/path validation
(reject absolute names, `..`, unsafe link targets), no PeerVault-owned staging
directory, no least-privilege child process, and no canary outside the root. The
dossier records a 2026 Borg extraction path/hard-link advisory (fixed in 1.4.5),
which is direct evidence that mature backends have had unsafe extraction paths.

**Why it matters.** Restore containment is the safety-critical path of the
product. If the selected backend fails the adversarial matrix, the only recorded
options are narrowing (which may leave nothing supported) or blocking the adapter
(which may leave no backend) — discovered after R0's ADR and V0.1 work.

**Resolution.**

1. Make containment a scored R0 selection criterion, not only a V0.1 test.
2. Decide the restore execution model in the R0 ADR: backend-performed restore
   (with pre-flight validation and sandboxing) versus adapter-controlled streaming
   writes (which allows `openat2`/`FILE_FLAG_OPEN_REPARSE_POINT`-style enforcement).
3. Require pre-flight validation of every entry name and link target, staging under
   a PeerVault-owned directory on the destination volume, a least-privilege child,
   and a canary assertion that no write occurred outside the staging root; promote
   only after validation.
4. Treat the post-write scan as a backstop, never the only control. Document that a
   backend cannot be trusted merely because it is mature.

---

### PVR-05 — Recovery kit is a runbook, not a reproducible artifact

**Severity:** high · **Origin:** omp · **Confidence:** high

**Location:** release plan §V0.2 (L190–205); 1.0 meaning (L45–47); design §Key
recovery.

**Problem.** V0.2 says the recovery kit “identifies backend/format/repository,
needed binary acquisition, non-secret policy/support information and recovery
steps.” There is no kit format, required contents, generator command, verifier, or
clean-device acquisition/verification procedure. A clean device with no internet
cannot obtain the backend; downloading an unpinned binary on the recovery path
undermines the recovery. The kit does not currently have to contain the exact
snapshot IDs, repository identity, backend version/checksum/signature, policy
revision, supported profile, or fixture oracle. V0.2's operator test is authored
by the same person who wrote the runbook; independent usability is only tested at
V0.9.

**Why it matters.** Clean-device recovery is the product's defining claim. A
runbook that works only on the author's machine, or a kit that depends on
unverified downloads, is not a recovery capability.

**Resolution.**

1. Define `RecoveryKit v1` as a bounded, non-secret artifact: manifest with
   backend/version/checksum/signature, repository identity, snapshot references,
   policy revision, profile/support matrix, recovery instructions, expected file
   counts/hashes for the drill, and a verifier command. Secrets stay separate.
2. Decide binary acquisition: bundle the pinned backend in the kit/installer, or
   require an offline verified copy; do not rely on “download latest.”
3. Add a `recovery-kit` generate/verify command to V0.2 and test it on a clean OS
   profile with network disabled.
4. Add an independent operator run at V0.2 (or mark the exit explicitly
   “author-verified only” rather than claiming independent usability).

---

### PVR-06 — Credential lifecycle and per-OS secret storage are unassigned

**Severity:** high · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** `AGENTS.md` §Design invariants (“backend owns … keys”); release plan
§V0.2; §V0.8 (L351–356); design §Key recovery; dossier key-recovery rows.

**Problem.** No document assigns who generates, stores, rotates, recovers or
revokes the repository password; who may read it; how a scheduled run on headless
Linux obtains it without a Secret Service; how a Windows scheduled task stores it
(DPAPI per user versus service account); how secrets are passed to backend CLI
processes without appearing in command lines or logs; or how password change,
key rotation and transport revocation differ operationally. The plan tests the
effects (wrong secret, rotation, removed device) but never names the product
responsibility or UX.

**Why it matters.** Unattended backup and clean-device recovery both depend on the
credential path. A bad choice either loses the only key or exposes it to every
local process. The backend owning crypto does not remove PeerVault's responsibility
for the owner-facing lifecycle.

**Resolution.**

1. Add a credential contract: PeerVault owns non-secret references, instructions
   and workflow; the backend owns cryptographic key material; the owner supplies or
   generates the repository secret.
2. Publish a per-OS secret-store matrix: Windows DPAPI/Credential Manager and
   scheduled-task identity; Linux 0600 file or systemd credentials with an explicit
   headless threat note; no keyring dependency for unattended CLI.
3. Specify per-backend secret passing (password file or protected environment, not
   `--password` argv) and redaction tests for child stderr.
4. Define rotation/revocation runbooks and tests for Kopia and restic separately;
   state master-key revocation limits (restic requires re-encryption).

---

### PVR-07 — No threat/survival matrix content; ransomware and media separation missing

**Severity:** high · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** R0 exit (L156–158); 1.0 meaning (L59–64); §V0.5; design §Security
and data-lifecycle boundaries; dossier PV-34 (L227).

**Problem.** R0 must produce a “threat/support matrix”, but no required rows,
columns, adversaries or dispositions are defined. 1.0 says the published topology
must identify which device/disk/credential failures it survives, but it does not
require the peer or recovery copy to be a distinct physical failure domain; a
second folder on the same disk satisfies every written exit. Ransomware is absent:
there is no mass-change pause, no retention floor, and no statement that a
reachable peer copy can be deleted by the same malware. Malicious-backend rollback,
repository deletion, and supply-chain compromise are also not in the matrix.

**Why it matters.** “Which failures does this survive?” is the product's core
answer. Without a matrix, the answer is unfalsifiable, and the default topology can
look safer than it is.

**Resolution.**

1. Define the matrix template in the release plan: adversary/failure rows
   (stolen source device, stolen peer, lost disk, deleted/corrupted repository,
   ransomware/mass change, malicious archive, malicious/rolled-back backend,
   compromised endpoint, supply-chain binary, lost credential) and columns
   (survived / degraded / not survived / not claimed), with the evidence reference.
2. Require peer and independent recovery copies to be on separately named physical
   media for the 1.0 topology; make unplug/eject the primary disk the canonical
   drill.
3. Add a default retention floor and a default-on mass-change pause (PVR-24);
   state that these limit blast radius, not ransomware resistance.
4. Publish the residual non-claims in README and recovery docs.

---

### PVR-08 — 1.0 is gated on sync; no supported backup-only release

**Severity:** high · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** release plan §What 1.0 means (L43–50); L92–94; version map;
§V0.6; short roadmap L53–56.

**Problem.** 1.0 includes opt-in live-folder sync, and “a failed sync gate holds
the advertised 1.0 scope.” The plan says to “keep the last qualified backup
milestone usable,” but V0.6 is a beta, not a supported release. Thus the riskiest,
least reusable capability blocks shipping the backup/recovery product that is
already valuable and that V0.1–V0.6 qualify.

**Why it matters.** Sync feasibility (PVR-02) is the largest schedule/technical
bet. Coupling it to 1.0 means the owner may wait years for a usable backup product,
or ship a “1.0” with an unqualified feature.

**Resolution.** Split the release: define 1.0 as backup, recovery, peer copy,
retention and desktop/scheduling, and move supported live-folder sync to 1.1/2.0
with its own RC gates. If a single release is required, add a supported 0.9
backup-only release with the same frozen-scope, clean-host, security and evidence
requirements as 1.0. Record this as a scope revision now, not at V0.7.

---

### PVR-09 — “Clean-target byte-restored” has no defined oracle for arbitrary data

**Severity:** high · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** design §Recovery set and evidence lifecycle; V0.1 exit (L181–184);
V0.3 (L220–222); README tagline; `RestoreReport v1` (L80).

**Problem.** The acceptance drill compares restored files to fixture hashes
generated before capture. For real user data after source loss, there is no source
oracle. `RestoreReport v1` records counts, not per-file expected/actual hashes, and
does not say whether restore re-hashes restored bytes. `verify_snapshot` verifies
the repository, not the restored tree. A backend restore that exits 0 may still
have skipped or truncated files; the plan's “clean-target byte-restored” state
therefore has no defined verification method for arbitrary data.

**Why it matters.** The product claim is “proves it can restore.” If restore
evidence is “the backend exited 0 and the counts look right,” the proof is weaker
than the claim. Independent comparison is only possible in drills or when a
capture-time hash manifest exists.

**Resolution.**

1. Define the restore evidence chain: full-read verification before restore;
   backend restore status; per-file outcome artifact; and a declared oracle.
2. Decide whether the product re-hashes restored files and against what:
   backend-provided per-file hashes (if exposed), a capture-time source hash
   manifest stored as evidence, or the fixture oracle for drills. Record the
   changed-during-read caveat.
3. If no oracle exists for arbitrary data, narrow the claim to “backend-reported
   restore plus full-read verification” and reserve “byte-restored” for drills
   with an independent manifest.
4. Update README and the design's evidence states to match.

---

### PVR-10 — Same-volume/same-media repository default is unguarded

**Severity:** high · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** README §First demo; 1.0 meaning (L59–64); design §Security
boundaries; dossier recovery-domain text (L176); GLM F5.

**Problem.** Nothing in profile review or `init` detects that the repository is on
the same volume, disk, network share or cloud-sync root as the source, or that the
peer/recovery copy is on the same physical device. The README demo uses
`peervault init ~/Documents --vault docs`, putting the vault beside the source.
The plan states “a local snapshot alone does not survive loss of its disk” but does
not enforce or warn about the default that creates that exposure.

**Why it matters.** The first-run experience can create a backup that survives
only the failures the source already survives. That is a false sense of safety in
a data-durability product.

**Resolution.**

1. Add source/repository/copy failure-domain detection to `init`/profile review:
   volume/device identity, network share, removable media, cloud-sync root. Warn
   or block with an explicit override and record the policy revision.
2. Make the 1.0 topology require distinct physical media for the peer and
   independent recovery copies; publish it in the support matrix.
3. Change the README demo to use a separate volume and show the verification
   step before restore.

---

### PVR-11 — Local state, mutation authority and stale-lock recovery are undefined

**Severity:** high · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** release plan contracts (L110–111); §V0.5 (L275–276); §V0.8 (L348–355).

**Problem.** The plan names durable policy/job/evidence records, a rebuildable
history cache, one mutation authority per repository and “lock ownership and
startup reconciliation.” It chooses no local state engine, schema, migration or
corruption behavior, and does not distinguish authoritative records from cache.
“No age-only stale-lock expiry establishes safe ownership” is correct, but without
an OS-owned lock that is released on process death, a crashed process leaves a
lock that can never be safely cleared. V0.8 requires restart-safe jobs and “no
second writer,” which depends on this mechanism.

**Why it matters.** V0.1 must persist policy and evidence; V0.5 retention and V0.8
scheduling both depend on durable state and a single-writer guarantee. Late choices
cause schema and lock rework, and a bad lock rule can either deadlock or allow two
writers.

**Resolution.**

1. Choose the state store in the V0.1 ADR: SQLite WAL with a single-writer
   connection (plus schema version/migrations) or append-only JSON with atomic
   replace. State which records are authoritative and which are rebuildable.
2. Use OS-owned advisory locks (Windows named mutex/file lock released by the OS;
   Linux `flock`) plus the backend's documented lock. Define lock states, owner
   identity, orphan handling and startup reconciliation.
3. Treat unknown local root/evidence state as a deletion-deferral condition (see
   PVR-18), not only unknown backend/peer state.
4. Add corruption and interrupted-migration tests, and a documented cache-rebuild
   path.

---

### PVR-12 — Fixture and fault-injection harness are unspecified

**Severity:** high · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** R0 protocols 1–2; V0.1 exit (L181–186); V0.5 exit (L278–288); V0.6.

**Problem.** The deterministic fixture is described as “256 MiB/64 files with a
fixed PRNG seed, repeated files, a 4 KiB insertion, rename, deletion, Unicode
names, empty files and a read-only file.” It does not define the PRNG algorithm,
file-size distribution, Unicode normalization forms, locale/permissions,
manifest format, duplicate-file count, or how a same-length insertion affects
total size. “Independent SHA-256 comparison” is only meaningful if the expected
manifest is generated independently of the restore tool; the plan does not define
that separation. The exit criteria also
require corruption, truncation, wrong credentials, disk full, denied access,
phase-targeted process termination and clock jumps, but no harness or pass/fail
assertion is specified. “Duplicate data is observed” has no acceptance threshold.

**Why it matters.** The plan's core discipline is reproducible evidence. Without a
committed fixture generator, golden manifest and fault harness, every stage's exit
is an opinion and V0.6's “accumulated matrix” cannot be compared across runs.

**Resolution.**

1. Commit one pinned deterministic generator and a golden path/size/SHA-256
   manifest; record the generator/seed/hash. Verify the manifest with an
   independent hash check (a second implementation is optional). The required
   independence is fixture build versus restore tool, not two generators.
2. Define the fixture precisely: PRNG algorithm/seed, size distribution, names,
   Unicode forms, permissions, duplicate count, insertion/deletion sequence and
   expected total bytes.
3. Commit a fault harness: copy/flip/truncate repository objects, remove index
   entries, simulate wrong credentials, disk full (VHD/loop), ACL deny, kill at
   named phases (using backend logs/progress or wrapper markers), and clock
   jump/backdate.
4. Give every protocol an explicit pass/fail assertion and record commands,
   timings and checksums. Replace “duplicate data is observed” with a recorded
   numeric observation and a stated non-promise.
5. Make the fixture spec and threat-matrix template **R0 entry tasks**. R0 can
   start without them, but its exit cannot be judged until they exist (Kimi K3).

---

### PVR-13 — Windows source/restore reality is incompletely scoped

**Severity:** high · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** V0.6 matrix (L292–304); design §Data and restore profile (L108);
dossier VSS rows.

**Problem.** The matrix covers case sensitivity, Unicode, long names,
symlinks/reparse, hard links, sparse files, ADS/ACLs, ownership/xattrs and special
files. It does not cover:

- cloud-placeholder files (OneDrive/Dropbox `RECALL_ON_DATA_ACCESS`/offline
  attributes): hydrate vs skip vs report, scan-completeness accounting, cost, and
  whether a hydrated restore matches the live tree;
- VSS/open files (whether 1.0 captures crash-consistent open files or reports them
  unreadable);
- files larger than 4 GiB and many-small-file workloads as first-class fixture
  rows;
- network shares, removable media and cloud-synced source roots;
- a deterministic policy for restoring case-distinct names from a case-sensitive
  source onto NTFS.

**Why it matters.** These are common on a Windows 11 owner machine. Silently
hydrating a OneDrive library is a cost/surprise failure; silently skipping
placeholders makes a snapshot look healthy while selected content is unprotected.
A case collision on restore is a data-selection decision that must be explicit.

**Resolution.**

1. Add source-policy rows: default skip-and-report placeholders; hydration only
   with explicit per-root consent; VSS decision (supported crash-consistent capture
   or explicit non-goal); network/removable roots excluded in 1.0 or tested.
2. Add a case-collision restore policy: fail the restore with a per-file report, or
   disambiguate deterministically and report; never silently pick one.
3. Add a >4 GiB file and a bounded many-small-file corpus to the V0.6 fixture set,
   with disk/time/RSS observations.
4. Add tests for an open/locked file and a placeholder file; report them in the
   enumeration counts.

---

### PVR-14 — Recovery-material choice blocks V0.2; independent usability is too late

**Severity:** high · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** design §Open questions (“Which independent recovery material is
acceptable, and who can read it?”); V0.2 exit (L205–207); V0.9 independent operator
(L376–378).

**Problem.** V0.2 requires “separately held recovery material” but the design's
open question about what counts as acceptable material and who may read it is not
assigned to any stage. The V0.2 operator test is authored by the plan author;
independent usability is deferred to V0.9. A kit can pass author testing and fail a
real clean device, especially when the author unconsciously supplies missing
context.

**Why it matters.** Recovery is the defining test. “The author could follow their
own runbook” is weak evidence for a product whose value is recovery by the owner
under stress.

**Resolution.**

1. Decide acceptable recovery-material types before V0.2 (password-manager entry,
   paper key, second device, envelope, escrow) with threat notes and who-can-read.
2. Add an independent operator (not the author) to the V0.2 clean-device drill; if
   unavailable, mark the exit “author-verified only; independent usability open”
   and do not claim independent recovery.
3. Include the cleanup/rotation procedure for recovery material in the kit.

---

### PVR-15 — Backend copy path is evaluated after backend selection

**Severity:** medium · **Origin:** omp · **Confidence:** high

**Location:** R0 step 4 (L144–147), step 6 (L152–154); V0.2 (L194–195); V0.3
(L211–215).

**Problem.** V0.2/V0.3 depend on a “backend-supported independent copy/export
procedure.” R0 step 4 says evaluate “the chosen backend's supported repository-copy
path”; selection is step 6. The plan forbids copying a live repository's private
files by convention. If restic has no supported ciphertext-only/repository-copy
path, or if Kopia's copy path requires keys on the receiver, a candidate can pass
the local bakeoff but fail V0.2/V0.3 after V0.1 adapter work. [INFERENCE: restic
`copy` requires opening both repositories with keys; a raw filesystem copy is what
the plan forbids unless the backend documents it.]

**Why it matters.** Backend selection can be invalidated by a capability that is
not in the selection criteria, causing a late re-selection and rework of the
adapter, recovery kit and peer design.

**Resolution.** Make the copy/transfer path an explicit scored R0 criterion for
both candidates: exact command, key requirements, destination identity guard,
delete semantics, snapshot-ID behavior and receiver verification. Disqualify a
candidate if V0.2/V0.3 cannot be satisfied. If raw copy is acceptable, require the
backend's documented lock/consistency rules and a tested quiesce.

---

### PVR-16 — Restore scope beyond new-empty-directory is undefined

**Severity:** medium · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** design §Data and restore profile (L108); `AGENTS.md` (“replacing
existing files is an explicit operation”); V0.1 (L174); V0.6.

**Problem.** V0.1 restores a whole snapshot to a new empty destination and rejects
existing/unsafe targets. The 1.0 destination says “restore a selected set”, but no
stage defines selective path restore, overwrite/merge, resume, or per-file
verification. `AGENTS.md` says replacing existing files is an explicit operation,
yet no stage implements that operation. Real recovery onto a reinstalled device
usually needs merge/overwrite; users will do it manually outside the tool, losing
containment and evidence.

**Why it matters.** The product's recovery workflow is incomplete for the most
common disaster (reinstall/repair with existing data), and the AGENTS invariant is
not backed by a gate.

**Resolution.** Either declare replace/merge and selective restore explicit 1.0
non-goals with a documented manual workaround, or add a V0.6 “restore into existing
tree” workflow with preview, backup of overwritten files, per-file outcomes and
containment tests. Define whether “selected set” means a snapshot or a path subset.

---

### PVR-17 — Policy revision, snapshot tags and selector semantics are undefined

**Severity:** medium · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** design `SnapshotRef` (L78); V0.1 source policy (L167–172); README
demo (`restore latest`); design “never select by newer timestamp alone” (L132).

**Problem.** “Policy revision” has no derivation (monotonic integer, canonical JSON
hash, or both). The adapter has no tag/description parameter, so a snapshot cannot
carry its policy revision/run ID; a rebuilt history cache cannot recover the
linkage. `restore latest` in the README conflicts with the design's rule that a
candidate is never selected by newer timestamp alone; there is no tie-break or
partial/unknown handling. `SnapshotRef` includes `backend_version`, so a backend
upgrade changes the reference for the same repository snapshot; the plan does not
say whether evidence is tied to capture-time or verification-time version. A
copied repository may assign different snapshot IDs; the plan does not define a
logical identity across copies.

**Why it matters.** Evidence must tie to the exact policy and snapshot. A rebuilt
cache or a peer copy without stable linkage can attribute evidence to the wrong
snapshot or release a retention root incorrectly.

**Resolution.**

1. Define policy revision as a monotonic integer plus a canonical JSON SHA-256;
   store both in snapshot tags/metadata and in local evidence.
2. Add `tags`/`description` (or a backend-native metadata equivalent) to the
   adapter; require the run ID/policy revision to be discoverable after a cache
   rebuild.
3. Define selectors: `latest-committed`, `latest-validated`, explicit snapshot ID,
   and time range, with ordering per repository, tie-breaks and refusal on
   ambiguity. Make `latest` mean “latest validated for this policy” or remove it.
4. Define copy identity: whether the backend preserves snapshot IDs; otherwise
   store a logical content/run identity and a source→destination mapping.
5. State capture-time versus verification-time backend version in evidence.

---

### PVR-18 — Peer/retention/sync records and atomic root advancement are under-specified

**Severity:** medium · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** release plan contracts (L112–113); V0.5 (L261–269); V0.7.

**Problem.** The plan names peer copy, retention and sync records but does not
define their fields, enums or transitions. “Atomically advance the protected root
with its evidence reference” across local files/state needs a transaction; no
mechanism is named. The record does not say whether retention roots include the
key/recovery-material versions needed to read retained snapshots, or where the
root evidence lives if the local device is lost. A lost local root/evidence record
is not in the “unknown state defers collection” list, even though it is exactly the
state that should block deletion.

**Why it matters.** Retention safety depends on exact transitions. An ambiguous
record can release a peer root or retain an unreadable snapshot.

**Resolution.**

1. Define versioned schemas/enums for `PeerCopyRecord`, `RetentionPlan`,
   `PeerRoot` and `SyncOperation`, with allowed transitions and evidence links.
2. Specify atomic root advancement (single DB transaction or atomic file replace +
   flush) and test crash/restart at each transition.
3. Include key/recovery-material availability and retained-snapshot readability in
   the retention plan; do not retain a snapshot whose required material is unknown
   without marking it needs-attention.
4. Add “local root/evidence state unknown or lost” to deletion deferral, and back up
   non-secret evidence/root records independently of the source device.
5. Define what a peer acknowledgement proves and what it does not.

---

### PVR-19 — Evidence registry, CI and R0 artifact placement are absent

**Severity:** high · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** R0 exit (L156–158); V0.9 evidence sentence (L395–396); RC evidence
manifest (L410); repo layouts.

**Problem.** Every gate depends on “exact revisions, fixtures, profiles, commands,
results and failures,” and RC requires an immutable evidence manifest. There is no
evidence directory, naming convention, manifest schema, checksum/immutability rule
or CI matrix. R0 — the next executable gate — produces the first evidence with
nowhere specified to put it. The planning repo has unrelated uncommitted work and
the product repo has no CI, so “accumulated matrix” claims cannot be audited.

**Why it matters.** Reproducibility is the plan's central discipline. Evidence
created ad hoc at R0 sets a precedent that is hard to retrofit at V0.6/RC.

**Resolution.**

1. Define an evidence registry in project-planning (or a dedicated evidence repo):
   one immutable directory per gate/run, with a manifest of inputs, hashes,
   commands, environment, results, failures and reviewer. Reference it from the
   ADR.
2. Add CI jobs for Windows and Linux from V0.1: build, fixture generation/golden
   manifest, link/anchor check, JSON schema checks and pinned backend checksum
   verification.
3. Record the planning commit/content digest at R0; keep evidence append-only and
   exclude secrets/paths as required.
4. Make RC's evidence manifest an aggregation of the same format, not a new one.

---

### PVR-20 — Security review and severity rubric arrive only at RC

**Severity:** medium · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** V0.9 (L390–394); RC security row (L406); R0 threat matrix.

**Problem.** Independent security review is a RC gate, although V0.3 adds peer
authorization/transfer, V0.5 adds destructive retention and V0.7 adds sync. The
release criteria use “critical/high” and “zero unresolved” without a severity
rubric or triage process, so the gate is unfalsifiable. The R0 “threat/support
matrix” has no required contents (PVR-07).

**Why it matters.** High-risk surfaces are designed and operated for months before
review; a late finding can invalidate a stage or the release. Undefined severity
lets a data-loss finding be classified as medium.

**Resolution.** Add security checkpoints at V0.3 (peer auth/copy), V0.5
(deletion/retention) and V0.7 (sync protocol). Define a severity rubric with
concrete data-loss/security examples and required dispositions. Maintain a findings
register with owner, severity, disposition and retest evidence.

---

### PVR-21 — Clock-quality method and future-dated evidence rule are unenforceable

**Severity:** medium · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** contracts (L109); evidence paragraph (L127–128); R0 Kopia protocol 7.

**Problem.** Evidence records “UTC time/clock quality,” and “clock jumps and
future-dated records cannot establish fresh recovery evidence.” No probe (NTP
offset, last synchronization, boot ID, monotonic clock), threshold or enum is
defined. The Kopia stress protocol samples offsets but does not define product
behavior when quality is unknown. Restic/Kopia listing/retention behavior can also
depend on timestamps.

**Why it matters.** Freshness and retention decisions depend on time. An
unenforceable clock rule can accept stale evidence or, for Kopia, contribute to
data deletion under skew.

**Resolution.** Define a `clock_quality` enum and probe (offset versus configured
NTP, last sync age, boot ID/monotonic clock), store it in evidence, refuse
freshness/retention decisions on unknown or skewed clocks, and add jump/backdate
tests. Document the selected backend's clock prerequisites as profile gates.

---

### PVR-22 — No provisional resource/performance guardrails before V0.9

**Severity:** medium · **Origin:** omp · **Confidence:** high

**Location:** V0.1; V0.6 (L299–304); V0.9 (L380–384).

**Problem.** The plan requires “bounded child processes/output,” bounded memory,
staging and concurrency, but proposes no provisional numeric limits and measures
performance only at V0.9. Many-small-file and large-file behavior can invalidate
the architecture at V0.6; a backup tool that exhausts memory or deadlocks on child
output is not fixable by a late benchmark.

**Why it matters.** Resource bounds are implementation invariants, not release
metrics. Late discovery causes redesign and invalidates V0.6 acceptance.

**Resolution.** Set provisional caps in V0.1 (peak RSS, child stdout/stderr bytes,
open handles, staging bytes, file/entry counts, timeouts) with explicit overflow
behavior. Add a bounded many-small-file and >4 GiB smoke at V0.6 with recorded
observations. Keep V0.9 for the measured envelope, not first discovery.

---

### PVR-23 — Backend packaging and binary-version policy are undefined

**Severity:** medium · **Origin:** omp (licensing overlap: GLM F13) · **Confidence:** high

**Location:** R0 ADR; adapter capabilities (L106); RC distribution (L407–408).

**Problem.** The adapter shells out to backend binaries. The plan does not decide
whether the backend is bundled with PeerVault or required as an external install,
how the installed version is detected, how an unsupported version fails closed,
whether the binary is checksum/signature-verified at runtime, how security updates
are delivered, or how the recovery kit obtains the exact binary. A user-installed
newer/older backend can change output formats and behavior.

**Why it matters.** Support, reproducibility and clean-device recovery depend on
the exact binary. A silently different backend can produce different snapshot IDs,
errors or restore behavior.

**Resolution.** Decide at R0/V0.1. Prefer bundling a pinned backend for the desktop
and a verified external dependency for CLI, with `--version` detection and a
supported range that fails closed. Store version/checksum in evidence and the
recovery kit. Add mismatch tests and a documented update policy.

---

### PVR-24 — No retention floor or mass-change pause

**Severity:** medium · **Origin:** GLM-5.3-Flash (F14) + omp · **Confidence:** medium-high

**Location:** V0.5 (L252–259); V0.7; dossier PV-34 (L227).

**Problem.** Retention is owner-reviewed with no minimum snapshot count/age, and
there is no mass-deletion/encryption circuit breaker. A ransomware-style mass
change on one device can be captured, propagated to the peer, and pruned under
whatever thin retention the owner accepted once. “Protect active work” is
unquantified. Ransomware guarantees are correctly out of scope, but the retention
floor and pause are factual safety features.

**Why it matters.** The last line of defense against destructive mass change is
currently an owner choice made before the incident.

**Resolution.** Define a default retention floor (e.g., never fewer than N
snapshots or M days without explicit override) and a default-on mass-change pause
with owner-configurable thresholds, preview and override. State that these limit
blast radius, not ransomware resistance. Test both with a synthetic mass-change
corpus.

---

### PVR-25 — Conflict artifacts can be lost before they reach retained backup

**Severity:** medium · **Origin:** GLM-5.3-Flash (F12) · **Confidence:** medium-high

**Location:** V0.7 exit (L335–340); dossier Syncthing versioning row (L162).

**Problem.** “All distinct versions remain recoverable under declared retention”
does not say which retention domain preserves them. Engine versioning is optional
and off by default; conflict copies live in the working tree until the next backup
capture. An edit/delete conflict followed by a deletion-propagating sync before any
capture can lose the only divergent bytes.

**Why it matters.** This is the silent-loss class the product exists to prevent,
hidden in an ambiguous exit criterion.

**Resolution.** Add a bridge rule: conflict artifacts and pre-deletion divergent
versions must be captured into the backup repository within a bounded interval, or
engine versioning must be enabled for the profile. Verify with a fault test that
deletes after conflict before capture and recovers every distinct byte.

---

### PVR-26 — Cross-document consistency drift

**Severity:** medium · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** short roadmap V0.4 (L34) vs release plan V0.4 (L245–248); README
tagline/demo; design V1 row; dossier L19.

**Problem.** Specific divergences:

| # | Document A | Document B | Issue |
|---|---|---|---|
| 1 | Short roadmap V0.4: experiment “preserves conflicting edit/delete/rename bytes” | Detailed V0.4: “identifies the viable engine/profile and its gaps”; full reconnect may remain an experiment | Acceptance is stronger in the summary than in the authoritative plan |
| 2 | README tagline “proves it can restore” | Plan: report describes an observed drill, not a permanent guarantee | Marketing claim outruns evidence semantics |
| 3 | README demo restores `latest` before `verify latest` | Plan: full-read verify precedes clean restore; never select by timestamp alone | Wrong order and undefined selector |
| 4 | `AGENTS.md`: replacing existing files is an explicit operation | Plan V0.1 rejects existing destinations; no replace stage | Invariant without a gate |
| 5 | Design milestone table has a “V1” stage | Release plan splits it into V0.6/V0.8 | Version-label drift |
| 6 | Dossier L19 says AGENTS/README direct a chunk-store/Merkle first slice and C-CAS/Bailiff/Anchor dependencies; pins commit `46925db` with only README/AGENTS | Design alignment gate and current files say backend-first, later candidates, and `docs/` now exists | Dossier carries the pre-alignment claim as current |
| 7 | Release plan R0 step 1 asks to reconcile README/AGENTS | Design says aligned locally on 2026-10-29 | Completed R0 task still listed |
| 8 | Dossier §Reproducible tests duplicates the design's seven protocols and has already drifted (P7: “two test clients” vs “two disposable client VMs and a disposable storage emulator”) | Design is named canonical; dossier copy reads as current | Protocol drift makes the canonical fixture ambiguous at R0 |

**Resolution.** Do one consistency pass: make the detailed release plan
authoritative for acceptance; soften the short-roadmap V0.4 row to match; change
the README tagline to “verifies it can restore” with per-drill scope; mark the demo
illustrative and reorder verify before restore; add a dated correction to the
dossier's “Plan changes and current profile” and claim ledger pointing to the
2026-10-29 alignment; mark R0 step 1 done; annotate the design's V1 row with its
expansion; replace the dossier's duplicated protocol bodies with dated pointers to
the canonical design anchors (keep the prose as historical record).

---

### PVR-27 — Sync/backend licensing is decided too late

**Severity:** medium · **Origin:** GLM-5.3-Flash (F13) + omp · **Confidence:** high

**Location:** V0.4; RC licensing row (L408); R0 step 2 (L135–139).

**Problem.** The V0.4 engine selection has no licensing/redistribution criterion,
although RC requires reviewed dependency/backend/sync redistribution terms and
V0.8 packages the desktop around the result. Embedding versus separate-process
invocation of an MPL-2.0 engine (Syncthing) changes architecture, notice
obligations and the artifact set. Backend Apache-2.0/BSD terms and bundling also
matter for the installer/recovery kit.

**Why it matters.** A licensing-incompatible engine or bundling choice surfaces at
RC, after sync, scheduling and desktop work are built on it.

**Resolution.** Add “redistribution posture recorded (embedding vs separate
process, notice obligations)” to the V0.4 and R0 ADRs. Review backend and sync
licenses when selected, not only at RC; include notices/SBOM generation in the
build from V0.1.

---

### PVR-28 — Terminology, duplicated sources of truth and prior-review scope

**Severity:** low · **Origin:** omp + GLM-5.3-Flash · **Confidence:** high

**Location:** design V1 row; README capability list; planning review section;
short roadmap vs detailed plan.

**Problem.** The design's V1 label and the short/detailed plan duplication create
two sources of truth. The README's capability names are explicitly framed as
“product concerns, not separate services to scaffold”, so they are a terminology
risk, not a divergence (Kimi K3 pushback). The prior planning review's “no
remaining planning blocker” is scoped to a documentation review but reads as a
technical clearance. The current link/anchor check is clean, but there is no CI
check to keep it that way.

**Resolution.** Annotate the design V1 row; add a terminology note that the README
capability names are concerns, not modules; add a one-line scope statement to the
planning review (“documentation review, not feasibility/security clearance”); make
the detailed plan the single acceptance source and generate or check the short
summary; add the link/anchor checker to CI.

---

### PVR-29 — Snapshot-scoped full-read verification is not guaranteed by the adapter/backends

**Severity:** high · **Origin:** Kimi K3 (K3-N2) · **Confidence:** high (restic scope);
medium (Kopia scope, unproven)

**Location:** design §Proposed boundary and identity (`verify_snapshot(snapshot,
full_read)`, L80); dossier R55, R28; release plan contracts (L104–121).

**Problem.** The adapter promises snapshot-scoped full-read verification. For
restic, the only documented full-content check, `check --read-data`, “reads/downloads
the whole repository” (dossier R55): it is repository-scoped, not snapshot-scoped.
Per-snapshot content proof then either reduces to a restore (collapsing the plan's
deliberately separate `full-read verified` and `clean-target byte-restored` states)
or does not exist. Kopia's `--verify-files-percent=100` is a CLI flag (dossier R28);
whether it can scope to one snapshot is unproven until R0. In addition,
`RestoreReport v1`'s `verification_mode ∈ {metadata, full_read}` has no value for
`clean-target-restored` or `application-validated`, although the lifecycle requires
those as distinct states.

**Why it matters.** The plan's central evidence discipline breaks at the first
contract: the advertised state may be unproducible, or silently produced at
repository scope at restore-level cost. A user may believe one snapshot is fully
verified when the whole repository was read, or that a snapshot was verified when
only metadata was checked.

**Resolution.**

1. R0 must score “snapshot-scoped full-read verification” per candidate:
   *native snapshot scope*, *restore-equivalent*, or *repo-wide only*. Record the
   chosen granularity in the ADR.
2. Add `verification_scope` and a stage/outcome field to the report contract;
   record what was actually checked, not just the mode flag.
3. If only repo-wide verification is available, accept it as a recorded profile
   limit (cost/time), not as an implied per-snapshot proof.
4. Keep `full-read verified` and `clean-target byte-restored` distinct in both the
   adapter and the evidence record; do not let a restore substitute for a
   verification state without saying so.

---

### PVR-30 — No recurring restore/verification cadence in 1.0

**Severity:** high · **Origin:** Kimi K3 (K3-N4) · **Confidence:** high

**Location:** design §Product claim (“regularly prove”), §Destination; release plan
§V0.8 (L344–361), §V0.9 (L372–396), §After 1.0; README tagline.

**Problem.** Nothing in the 1.0 scope produces recurring restore evidence.
V0.9's three drills are one-off beta evidence; recurring drills appear only under
“After 1.0.” V0.8 schedules captures and displays last-verified/last-restored
timestamps with owner-configured freshness, but no supported scheduled full-read
verification or clean-target restore job exists. After release, freshness evidence
can only age into permanently stale. The design's “regularly prove that saved
files can be restored” and the README's “proves it can restore” claim a property
the 1.0 workflow does not generate.

**Why it matters.** The defining claim is a cadence. Shipping the status surface
without the jobs that feed it makes “needs attention” the steady state and the
tagline an overreach.

**Resolution.**

1. Add an opt-in scheduled verification job (full-read at the selected scope) to
   V0.8's supported scheduling, with its own evidence entries and failure/backoff
   behavior.
2. Add a periodic bounded clean-target restore drill job (scratch directory, size
   cap, retention of report) or state explicitly that drills are owner-run manual
   operations and narrow the wording.
3. Define default freshness thresholds and staleness behavior, and test a missed
   run, a stale evidence state and a clock jump.
4. Update the design/README claim to match the implemented cadence.

---

### PVR-31 — Linux distribution and Windows build are not selected before profile-specific evidence

**Severity:** medium · **Origin:** Kimi K3 (K3-N5) · **Confidence:** high

**Location:** 1.0 meaning (L52–57); R0 step 3 (L140–143); design §Reproducible
validation protocols preamble; V0.1 exit (“both same-OS profiles”, L181).

**Problem.** “One pinned Linux x86-64 distribution/ext4” is never selected by any
stage; it is only “frozen and published before release.” R0's evidence is
explicitly profile-specific (“reproducible candidate evidence on both OS
profiles”), and V0.1's exit requires both same-OS profiles. Choosing the
distribution or Windows build later invalidates R0/V0.1 evidence or forces reruns.
The choice is a de facto R0 prerequisite that the plan never assigns.

**Why it matters.** Profile-specific evidence against an unnamed profile is not
reproducible evidence. Backend selection (R0's purpose) could rest on a
distribution that is later changed.

**Resolution.** Name the Linux distribution/release (including kernel/filesystem
settings) and the exact Windows 11 build in R0 step 1; record both in the ADR
alongside backend pinning. Treat any later change as a profile revision requiring
affected reruns and a new evidence entry.

---

### PVR-32 — Consistency-class enum is undefined

**Severity:** medium · **Origin:** Kimi K3 (K3-N6) · **Confidence:** high

**Location:** design §Scope (“capture consistency mode”); release plan contracts
(Source/capture policy row, L108); V0.1 policy freeze.

**Problem.** The policy contract requires a “consistency class,” and acceptance
hinges on “the declared file-level consistency limit” and “explicitly
crash-consistent” marking. The enumeration is defined nowhere: stable/offline?
live-file? crash-consistent? app-consistent only via a named profile? V0.1 freezes
source policy, but validators cannot compare policy revisions or judge a run
against an undefined enum.

**Why it matters.** Two implementers will encode different classes. A
“crash-consistent” versus “live-file” mislabel is exactly the silent upgrade the
design forbids elsewhere.

**Resolution.** Define the enum semantics with the V0.1 policy freeze (suggested:
`stable-declared`, `live-file`, `crash-consistent-marked`, `app-profile:<name>`),
its serialization, and which classes may claim which evidence states. Add a test
that a policy revision with a changed class cannot explain previous evidence.

---

### PVR-33 — Review integration and planning-review state

**Severity:** medium · **Origin:** Kimi K3 (K3-N3) · **Confidence:** high

**Location:** `docs/implementation-plan.md` header (L4–7); release plan
§Planning review; dossier §Plan changes and current profile (L17–19); this review;
git state (`docs/` untracked).

**Problem.** This review is a 33-finding artifact in the same `docs/` folder as
the roadmap, committed nowhere, while the roadmap header and the release plan's
§Planning review still assert a single-review state: “GPT-6.1 Terra … three
issues; all repaired … no remaining planning blocker.” An implementer reading only
the roadmap starts V0.1 unaware of PVR-01/02/03. The review's input-digest table
goes stale on the first consolidating edit, and the dossier's L17–19 still carries
the pre-alignment claim.

**Why it matters.** The repository's visible claim (“no remaining planning
blocker”) would contradict its own unreferenced blocker review. Stale digests can
later masquerade as coverage of changed text. Kimi K3 rated this high; this review
keeps it medium because it is a process/visibility issue rather than a stage gate,
but it must be dispositioned before implementation continues.

**Resolution.**

1. After disposition, record per-finding accepted/rebutted/deferred outcomes in
   the release plan's §Planning review and the roadmap header, with a link to this
   file. Do not leave “no blocker” while blockers are open.
2. Re-hash the inputs after edits or label the digest table “reviewed content as of
   2026-10-29” and record the new revision separately.
3. Commit `docs/` and the review file, or mark the review as worktree-only and link
   it from the roadmap; do not rely on an untracked file for a release gate.
4. Close Appendix B/C in this file when the independent reviews are integrated (done
   in this revision).

---

## Cross-document consistency matrix

| # | Claim / gate | Documents | Status |
|---|---|---|---|
| 1 | Backend-first, no custom crypto first | README, AGENTS, design, release plan | Consistent |
| 2 | No PeerVault GC command in first slice | AGENTS, design, release plan V0.1 | Consistent |
| 3 | Keep-both sync separate from backup history | README, AGENTS, design, release plan | Consistent |
| 4 | Ciphertext-only peer needs authorized recovery device | README, V0.3 | Consistent in intent; mechanism missing (PVR-01) |
| 5 | Retention pins per peer/repository | Design, V0.5 | Consistent; record schema missing (PVR-18) |
| 6 | V0.4 experiment strength | Short roadmap vs release plan | Divergent (PVR-26) |
| 7 | Restore to new empty destination vs replace | AGENTS vs V0.1/V0.6 | Divergent (PVR-16) |
| 8 | `latest` selector vs no timestamp selection | README vs design | Divergent (PVR-17) |
| 9 | Sync is 1.0 scope | 1.0 meaning, V0.7, RC | Consistent but risky (PVR-08) |
| 10 | Adapter operations vs V0.5 retention | Contracts vs V0.5 | Missing operation (PVR-03) |
| 11 | Report result pass/fail vs partial/unknown | Design vs contracts/V0.1 | Internal mismatch (PVR-03) |
| 12 | Product doc alignment | Design says done; dossier says pre-alignment | Dossier stale (PVR-26) |
| 13 | R0 documentation reconciliation | R0 step 1 vs design alignment note | R0 task stale (PVR-26) |
| 14 | Version labels V1 vs V0.6/V0.8 | Design vs release plan | Drift (PVR-26) |
| 15 | Snapshot-scoped full-read verification | Adapter contract vs restic repo-wide check | Missing capability/scope (PVR-29) |
| 16 | Recurring restore/verification evidence | Design/README “regularly prove” vs 1.0 scheduling scope | Not produced by 1.0 (PVR-30) |
| 17 | Linux distro/Windows build selection | 1.0/R0/V0.1 evidence | Unassigned R0 prerequisite (PVR-31) |
| 18 | Consistency-class semantics | Design scope vs contracts | Undefined enum (PVR-32) |
| 19 | Planning-review state | Roadmap/release plan vs this review | “No blocker” claim stale while blockers open (PVR-33) |
| 20 | Protocol body duplication | Design vs dossier | Drifted copies (PVR-26) |

## Contract gaps at a glance

| Contract | Missing or ambiguous | Needed before |
|---|---|---|
| `BackendAdapter v1` | Cancel/progress, run ID/idempotency, tags, repository identity/capabilities, copy, retention/maintenance, key export/rotation, verification scope | V0.1 |
| `RestoreReport v1` | `partial`/`unknown` outcome, per-file artifact, byte counts, reason codes, exit codes, limits, clock quality, redaction, `verification_scope` + stage/outcome | V0.1 |
| Source/capture policy | Policy revision derivation, consistency-class enum, placeholder/open-file rules, case-collision policy, unsupported-item disposition | V0.1/V0.6 |
| Local state | Engine, schema, migrations, corruption, authoritative vs cache, evidence/root durability | V0.1 |
| Jobs/mutation | OS lock mechanism, owner identity, orphan/stale handling, startup reconciliation | V0.1/V0.8 |
| Peer copy | States/predicates, transport, key role, copy identity, receiver verification, acknowledgement meaning | V0.3 |
| Sync | Causal IDs, tombstones/acks/expiry, quarantine/rejoin, conflict artifact bridge, log/discovery policy | V0.4/V0.7 |
| Scheduling | Recurring full-read verification and drill jobs, freshness/staleness defaults | V0.8 |
| CLI/JSON | Versioned schema, errors, cancellation, exit codes, compatibility | V0.1/RC |

## Decisions the owner must make before the named stage

| Decision | Needed by | Current state |
|---|---|---|
| Restore execution model (backend-performed vs adapter-streamed) and containment enforcement | R0 | Open; determines whether PVR-04 is solvable |
| Backend copy/transfer path and receiver key model | R0 | Open; V0.2/V0.3 depend on it |
| `repository_id` derivation and copy semantics | R0 | Undefined |
| Threat/survival matrix contents and distinct physical media requirement | R0 | Undefined |
| Local state store, lock mechanism, schema/migration | V0.1 | Undefined |
| Credential storage per OS and secret-passing method | V0.1/V0.2 | Undefined |
| Recovery-material types/custody | V0.2 | Design open question |
| Sync build-vs-reuse, engine capability matrix, fallback scope | V0.4 | Undefined |
| 1.0 scope split (backup-only vs sync-blocked) | Now | Not decided |
| Linux distribution/Windows build pin | R0 | Unassigned |
| Post-release proof cadence (scheduled verify/drill) | V0.8 | Not in 1.0 scope |
| Consistency-class enum and evidence mapping | V0.1 | Undefined |
| Review disposition and planning-review update | Now | Pending |
| Retention floor, mass-change defaults, severity rubric | V0.5 | Undefined |
| Backend bundling/version policy and licensing posture | R0/V0.1 | Undefined |

## What is genuinely strong — do not “fix” these

- **Evidence-state separation** is consistent across all documents: committed,
  full-read verified, clean-target byte-restored, peer copy verified and
  application validated are distinct; a later partial run cannot replace last-good
  evidence; unavailable evidence becomes unknown after rebuild.
- **Per-peer retention pinning** is unusually rigorous: a durable last-good root
  per peer and repository, advanced only after same-path verification and clean
  restore, with explicit owner abandonment and an offline peer unable to release
  it. The prior review's high finding is genuinely repaired.
- **Crash epistemics** are disciplined: process kill versus VM power-off is
  separated; Kopia's “few minutes” is not converted into a threshold; maintenance
  only through the documented safety mode; no age-only stale-lock expiry.
- **Restore-safety test matrix** is concrete and adversarial: `openat2`
  `RESOLVE_BENEATH`/`NO_SYMLINKS`, `FILE_FLAG_OPEN_REPARSE_POINT`, canary outside
  the root, case-only names, normalization variants, race attempts, and the
  admission that post-write scans cannot undo escapes.
- **Scope honesty** is strong: non-goals match; no custom crypto; no PeerVault GC;
  no multi-writer; no app consistency without an oracle; all gates marked unrun;
  versions treated as dated evidence.
- **Key-recovery realism**: restic's master-key revocation limit, Borg's
  export/passphrase separation, and passphrase-change versus rotation are carried
  into V0.2's distinctions and tests.
- **Documentation discipline**: relative links and heading anchors are currently
  clean (0 failures), and the plan records the prior review's findings and repairs.

---

## Appendix A — GLM-5.3-Flash independent review

> Condensed from the reviewer's returned output; severity labels are the
> reviewer's. Its verdict: R0 is executable today; the plan is not executable
> through 1.0 as written because of the sync bet, missing contract operations,
> containment fallback absence, and undefined artifacts (severity taxonomy,
> evidence layout, repository identity).

| ID | Severity | Finding | Resolution (reviewer) |
|---|---|---|---|
| F1 | high | V0.4 exit is an evaluation, not a decision; V0.7 requires causal IDs, tombstones/acks, quarantine and deterministic paths the cited engine class does not provide. | Scored capability matrix + ADR; pre-approve fallback (backup + peer copy without supported sync). |
| F2 | high | Contract table omits V0.2 key export/rotation and V0.5 retention/delete; no extension/versioning rule. | Add capability blocks or define a separate backend-maintenance contract and evidence schema. |
| F3 | high | Containment is detect-and-narrow; no enforcement layer; `restore_snapshot` delegates wholesale. | Decide backend-performed vs adapter-streamed restore at R0; make containment a scored criterion. |
| F4 | medium-high | Cloud-placeholder sources (OneDrive) are absent; hydration vs skip, scan completeness and cost undefined. | Default skip+report; hydration only with explicit consent; V0.6 matrix row. |
| F5 | medium | 1.0 topology does not require peer/recovery copies on distinct physical media; same-disk second folder satisfies written exits. | Name distinct failure domains; eject-primary-disk drill. |
| F6 | medium | README “proves it can restore” is a permanence claim; plan says observed drill, not permanent guarantee. | Reword to “verifies it can restore” with per-drill scope. |
| F7 | medium | `repository_id` derivation and copy inheritance undefined; wrong-peer/repository detection is decorative. | Define derivation/copy semantics in R0 ADR before V0.1 freeze. |
| F8 | medium | Report lacks per-file outcome artifact, byte counts, enumerated failure codes. | Add bounded per-file artifact/reason codes. |
| F9 | medium | “Critical/high” severity and threat/support matrix contents undefined; gates unfalsifiable. | Define severity rubric and matrix rows before V0.5. |
| F10 | medium | Dossier still asserts pre-alignment AGENTS/README claims and pins a checkout that no longer matches. | Dated correction entry in dossier and claim ledger. |
| F11 | medium | No evidence location/naming/manifest convention; R0 produces first evidence with nowhere to put it. | Define append-only evidence registry before R0. |
| F12 | medium | V0.7 “all distinct versions recoverable” does not name the retention domain; conflict bytes can be deleted before capture. | Bridge rule: capture conflict artifacts/pre-deletion versions within a bounded interval. |
| F13 | medium | V0.4 engine selection has no licensing/redistribution criterion; embedding MPL affects architecture/notices. | Add redistribution posture to V0.4/R0 ADR. |
| F14 | medium | No retention floor or mass-change pause; thin owner retention plus ransomware can prune the last good state. | Define minimum retention floor and default-on mass-change pause. |
| F15 | low | Desktop/core IPC says “version/permission boundaries” without naming authentication; any local process could reach destructive operations. | Named pipe/socket ACL + per-call operation tokens; core is sole mutation authority. |
| F16 | low | README demo invents unspecified CLI shape and restores before verify; design still has a merged “V1” stage. | Mark demo non-normative/align; annotate V1 expansion into V0.6–V0.8. |

Reviewer strengths: per-peer retention pinning, evidence-state separation,
ciphertext-only honesty, crash epistemics, adversarial restore matrix, scope
honesty, cross-OS restore epistemics, key-recovery realism.

Reviewer open questions: peer key model (ciphertext-only vs key-holding); recovery
material custody; restore execution model; pre-registered 1.0 fallback; retention
floor/mass-change defaults; repository identity derivation.

## Appendix B — Kimi K3 independent review

> Condensed from the reviewer's returned output. Kimi K3 read all six inputs plus
> the first revision of this review, and independently reproduced the three top
> risks. Its verdict: the plan is executable through R0 and, with the prior
> repairs, mostly through V0.1, but not through 1.0 as written. It adds three
> unique high/medium findings (per-snapshot verification scope, recurring proof
> cadence, profile selection) and three concrete Syncthing mismatches. It also
> correctly pushes back on “R0 is executable now”: R0 can **start**, but its exit
> is unjudgeable until the fixture spec and threat-matrix template exist. Those are
> now R0 entry tasks in PVR-12/PVR-07.

| ID | Severity | Finding | Resolution (reviewer) |
|---|---|---|---|
| K3-N1 | high | Three Syncthing mismatches: conflict names use wall-clock + device ID and cannot converge under retries; deletion tombstones have no expiry horizon to trigger quarantine; no fault-injection seam for duplicate/reordered/partition/rejoin tests. | Add conflict-name determinism, tombstone-horizon source and fault-injection seam to the V0.4 scored matrix; expect “impossible”/“wrapper-invented” and record the wrapper rule or hold the gate. Verify exact-build behavior, not rolling docs. |
| K3-N2 | high | `verify_snapshot(snapshot, full_read)` is snapshot-scoped, but restic's `check --read-data` is repository-scoped; per-snapshot proof reduces to restore or is absent. Kopia scope unproven. `RestoreReport` has no stage/outcome value for clean-restored/app-validated. | R0 scores verification scope per candidate; add `verification_scope` and stage/outcome to the report; accept repo-wide as a profile limit, not per-snapshot proof. |
| K3-N3 | high (process) | This review is untracked in `docs/`, while the roadmap/release plan still claim a single review with “no remaining planning blocker”; input digests go stale; Appendix B/crosswalk were pending. | Record dispositions in §Planning review and the roadmap header; re-hash or date the digest table; commit the review or mark it worktree-only and link it; close appendices. |
| K3-N4 | medium | Nothing in 1.0 produces recurring restore evidence; V0.9 drills are one-off; V0.8 displays freshness but has no scheduled verify/drill job; recurring drills are “After 1.0.” | Add opt-in scheduled full-read verification and bounded clean-target drill jobs to V0.8, or narrow “regularly prove”/“proves” wording. |
| K3-N5 | medium | Linux distribution and Windows build are never selected; R0/V0.1 evidence is profile-specific, so a later choice invalidates it. | Name the distro/release and Windows build in R0 step 1; record in the ADR; treat changes as profile revisions requiring reruns. |
| K3-N6 | medium | Consistency-class enum is undefined (stable/live-file/crash-consistent/app-profile); policy revisions cannot be compared and “crash-consistent” can be mislabeled. | Define enum semantics, serialization and which classes may claim which evidence states with the V0.1 policy freeze. |
| K3-N7 | low | Design and dossier both carry the seven protocols and have drifted (P7 wording differs); the dossier copy reads as current. | Replace dossier protocol bodies with dated pointers to the canonical design anchors. |
| K3-N8 | low | Three pushbacks on the consolidated review: (a) README capability names are explicitly “concerns, not services”, so PVR-26 row 8 overstates; (b) PVR-12's two independent fixture implementations are gold-plating; (c) “R0 executable now” ignores that R0's exit needs the fixture spec and matrix. | (a) demote to terminology; (b) one generator + recorded seed/hash + independent hash check; (c) “R0 can start; entry tasks required.” |

**Independent corroborations (mapped to existing IDs, no extension):**

| # | Confirms | Basis |
|---|---|---|
| 9 | PVR-03/PVR-12 fixture | Protocol 1 specifies no PRNG, seed, file-size table, Unicode strings or insertion offset |
| 10 | PVR-03 operations | Contract lacks copy/transfer, retention/forget, key export/rotation, cancel/run-ID |
| 11 | PVR-03 report enum | `pass|fail` vs partial/unknown and partial targets |
| 12 | PVR-01/PVR-15 | Copy path evaluated post-selection; ciphertext-only receiver has no access path/predicate |
| 13 | PVR-16 | `restore_snapshot` has no path-subset parameter; no stage implements the AGENTS replace invariant |
| 14 | PVR-10 | `--vault docs` teaches a same-volume vault; no failure-domain check |
| 15 | PVR-26 | Roadmap V0.4 stronger than detailed; demo order; dossier pre-alignment claim; R0 step 1 stale |
| 16 | PVR-19/PVR-20 | No evidence registry/schema/CI; “critical/high” undefined |
| 17 | PVR-11 (IPC) | Desktop↔core IPC is absent from the contracts table, not only under-authenticated; add a row |
| 18 | PVR-06/PVR-07/PVR-24 | Credential custody; threat-matrix rows; retention floor/mass-change |

**Reviewer strengths:** evidence-state separation; per-peer retention pinning;
crash epistemics; adversarial restore posture; key/recovery realism; dossier claim
ledger and dated-source discipline; scope honesty; backup/sync separation.

**Reviewer open questions:** 1.0 scope split now; peer key model; restore
execution model; recovery-material types/custody; backend/sync distribution
posture; post-release proof cadence.

**Pushbacks accepted in this revision.** (a) PVR-26 row 8 was removed; README
capability names are now a terminology note in PVR-28. (b) PVR-12 now requires one
pinned generator plus an independent hash check, not two implementations. (c) The
verdict and PVR-12 now say R0 can start but cannot exit until the fixture spec and
threat-matrix template exist.

**Verification note.** Kimi K3 reports the Syncthing conflict-name pattern from
current Syncthing documentation. The dossier does not pin that rolling page to
v2.1.7; PVR-02 marks the claim as requiring exact-build verification before it is
treated as a fact.

## Appendix C — Consolidation crosswalk

| Consolidated | GLM-5.3-Flash | Kimi K3 | Note |
|---|---|---|---|
| PVR-01 | F7 (repository identity overlap) | #12 | Ciphertext-only verification + repository identity |
| PVR-02 | F1, F12 | K3-N1 | Sync feasibility + concrete Syncthing mismatches + conflict bridge |
| PVR-03 | F2, F8 | #10, #11 | Adapter/report/state contracts |
| PVR-04 | F3 | — | Restore containment |
| PVR-05 | F11 (evidence overlap) | — | Recovery kit/evidence |
| PVR-06 | — | #18 | Credential lifecycle (omp unique) |
| PVR-07 | F5, F9, F14 | #18 | Threat/survival matrix |
| PVR-08 | F1 (fallback) | Open Q1 | Release split |
| PVR-09 | F8 | — | Restore oracle |
| PVR-10 | F5 | #14 | Same-media default |
| PVR-11 | F15 (IPC) | #17 | State/locks/authority |
| PVR-12 | F11 (part) | #9 + K3-N8b | Fixture/fault harness |
| PVR-13 | F4 | — | Windows source/restore reality |
| PVR-14 | — | Open Q4 | Recovery material/usability |
| PVR-15 | F2 (copy operation) | #12 | Backend copy path selection |
| PVR-16 | F16 | #13 | Restore scope/demo |
| PVR-17 | F7, F8 | #11 | Policy revision/selector/identity |
| PVR-18 | F7, F12, F14 | — | Peer/retention records |
| PVR-19 | F11 | #16 | Evidence registry/CI |
| PVR-20 | F9 | #16 | Security checkpoints/severity |
| PVR-21 | — | — | Clock quality (omp unique) |
| PVR-22 | — | — | Resource guardrails (omp unique) |
| PVR-23 | F13 (part) | — | Backend packaging/version |
| PVR-24 | F14 | #18 | Retention floor/mass-change |
| PVR-25 | F12 | — | Conflict artifact bridge |
| PVR-26 | F6, F10, F16 | #15, K3-N7 | Consistency drift |
| PVR-27 | F13 | — | Licensing |
| PVR-28 | F15/F16 | K3-N8a | Terminology/duplication |
| PVR-29 | — | K3-N2 | Snapshot-scoped verification |
| PVR-30 | — | K3-N4 | Recurring proof cadence |
| PVR-31 | — | K3-N5 | Profile selection |
| PVR-32 | — | K3-N6 | Consistency-class enum |
| PVR-33 | — | K3-N3 | Review integration/planning-review state |

## Verification performed and limits

- Read all six inputs in full; used raw reads for lines truncated at 768
  characters. Read both independent reviewer outputs in full, including raw lines
  where the delivery was truncated.
- Relative-link/anchor check: 0 missing files, 0 missing anchors across README,
  AGENTS, both planning documents and the short roadmap.
- Confirmed by direct text search: no severity rubric; no evidence-directory
  convention; no `repository_id` derivation; no adapter cancel/retention/tag
  operations; no local-state engine choice; dossier L19 still carries the
  pre-alignment product claim; failure-domain principle exists in the dossier but
  is not a release requirement; no consistency-class enum; no scheduled
  verification/drill job in 1.0.
- Kimi K3 performed one external search to ground the Syncthing conflict-name
  pattern; that claim is marked for exact-build verification, not accepted as a
  pinned fact.
- Not performed: any backend installation, snapshot, restore, crash test, clock
  test, path-traversal test, license audit or cryptographic review. The product
  has no executable artifacts.
- The review did not modify the plan or any planning file; it is a separate
  artifact for the plan owner. `docs/` and this file are untracked until the owner
  commits them (PVR-33).
