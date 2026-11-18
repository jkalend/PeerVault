# Independent security review scope — candidate package

Status: review input, not a review result. This lists the exact surfaces an
independent reviewer should examine and the evidence already available. No
critical/high finding may remain open for the supported subset.

## In scope

| Surface | Code/docs | Questions |
|---|---|---|
| Backend process boundary | `src/production.rs`, `src/agent.rs`, `src/schedule.rs` | Are child env/stdout/stderr, timeout, process-tree kill and argument construction bounded? Can a caller inject backend flags or inherit a secret? |
| Restore containment | `src/production.rs` (`validate_destination`, `extract_snapshot`, `read_manifest`) | Can a crafted tar/manifest escape staging, follow links/reparse points, collide case/normalization, overwrite an existing target or leave an unlabelled partial tree? |
| Tree oracle | capture/manifest/restore paths | Can a missing/ambiguous oracle pass? Can backend output or a changed manifest produce a false verified restore? |
| Credentials | `Credential`, scheduler, RecoveryKit, IPC | Are secrets absent from argv, state, plans, logs and reports? Is the DPAPI/systemd-credential runtime file owner-only and removed? What happens on wrong/missing/rotated secrets? |
| Peer/copy records | V0.3 drill, retention peer roots | Does an acknowledgement overstate copy health? Can a source-absent drill pass without the exact peer binding/key role? |
| Retention | `src/retention.rs` | Can preview/apply drift, replay, peer-root loss, mass-change, an abandoned lock, or a partial backend failure broaden deletion? Is only backend-native maintenance used? |
| IPC | `src/ipc.rs`, `src/agent.rs`, desktop | Are the named-pipe/Unix-socket caller SID/UID checks effective? Can a different account connect, replay, or reach a non-allowlisted operation? Is the response bounded? |
| Distribution | `scripts/sbom.py`, `verify-package.py`, package | Does the artifact/notice/SBOM set match the published bytes? Is the published Components pin the relicensed commit? Are publisher/signature instructions non-circular? |

## Out of scope / explicitly not claimed

- Endpoint or administrator compromise, ransomware using the owner's deletion
  credentials, or a malicious backend that rewrites both data and oracle.
- Cryptographic design: restic owns encryption, keys, repository format and
  chunk integrity. PeerVault does not implement custom cryptography.
- Live-folder sync (V0.7) — not part of backup-only 0.9.
- Physical media failure domains and offsite copy beyond the declared topology.

## Required reviewer outputs

1. Finding register: ID, scope/revision, scenario, severity (critical/high/
   medium/low), owner, disposition, required gate, repair and retest evidence.
2. Explicit tested artifact/commit, package SHA-256, SBOM digest and commands.
3. Critical/high findings fixed and retested, or the affected profile removed
   from the supported claim. A waiver cannot preserve a broken safety guarantee.
4. Confirmation that `scripts/verify-package.py` passes without
   `--allow-license-block` on the reviewed artifact and that the published
   Components pin is the relicensed commit.

## Evidence to supply

- `docs/local-smoke-2026-11-18.md`, `docs/v0.5.md`, `docs/v0.6-matrix.md`,
  `docs/v0.8.md`, `docs/v0.9-measure.md`.
- The retained package, checksum sidecar, `release-manifest.json`,
  `sbom.cdx.json`, `THIRD-PARTY-NOTICES.txt` and `docs/license-review.md`.
- CI run IDs and the exact commands from `docs/release-checklist.md`.
- Open limits from `docs/rc-readiness.md`; do not present source presence or a
  CI definition as a passing review.

## Known questions for the reviewer

- Is the Windows named pipe's default DACL plus explicit client-SID comparison
  sufficient for the stated same-user boundary, including admin/SYSTEM?
- Does `restic forget --prune` have any partial state that the reconciliation
  path can misclassify as `pass`?
- Can a crafted repository/dump stream cause unbounded memory/disk use before
  the manifest/extraction caps apply?
- Is the mass-change manifest diff conservative enough for zero/tiny/unknown
  sets and encrypted-looking rewrites?
- Are DPAPI current-user and systemd-credential paths actually owner-only under
  the documented task/service identities?
