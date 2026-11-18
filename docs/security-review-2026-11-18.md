# Internal security review — 2026-11-18

Status: internal read-only review of `26164ce`, with fixes applied afterward.
This is **not** the independent external security gate; it is review input and a
record of code dispositions.

## Findings and dispositions

| ID | Severity | Finding | Disposition |
|---|---|---|---|
| F1 | High | Windows desktop trusted a predictable pipe name; a local squatter could impersonate the agent and fabricate `pass` reports | **fixed**: unguessable GUID endpoint plus `GetNamedPipeServerProcessId` must equal the spawned agent PID before the client uses the pipe |
| F2 | Medium | Scheduled restore job id `..`/reserved names escaped `destination_root` | **fixed**: portable single-component job IDs (`validate_path` + no `/`) and canonical `job_dir.starts_with(root)` |
| F3 | Medium | Future snapshot timestamps could occupy keep/floor slots and push older recovery points to delete candidates | **fixed**: timestamps more than five minutes in the future are protected, listed as `future-timestamp`, do not consume keep slots, and appear in plan limitations; backward-skew detection remains an explicit limit |
| F4 | Medium | `peer-record` accepted any 1–16 MiB file as proof of a peer root | **fixed**: evidence must be JSON binding peer repository, snapshot, oracle, completed clean restore/full-read and a verification time within 31 days |
| F5 | Medium | Successful `forget --prune` followed by an unreadable newest oracle still reported `pass` | **fixed**: baseline failure now records `unknown`, advances the generation and returns nonzero |
| F6 | Medium | DPAPI wrapper wrote plaintext to a fixed path before ACL restriction | **fixed**: random per-run path, explicit owner-only `FileSecurity` at creation, dedicated `%LOCALAPPDATA%\PeerVault\run` ACL, cleanup in `finally` |
| F7 | Medium | Package verifier trusted a self-declared SBOM property and did not bind bytes to the release manifest | **fixed**: every SBOM component must declare a license; packaged binary/SBOM hashes must match the packaged manifest; dirty-source manifests are refused |
| F8 | Low | Streaming restore `restic dump` had no timeout/cancellation/process-tree kill | **fixed**: process-group supervision with timeout, cancellation, kill and reaping |
| F9 | Low | Unix password files did not enforce owner-only mode; state temp files were chmodded after creation | **fixed**: password files with group/other bits are refused; state temp files are created `0600` on Unix |
| F10 | Medium (second review) | Future timestamps were excluded from selection but still used as the baseline/newest for mass-change, so a future-dated baseline could hide a destructive change | **fixed**: baseline and mass-change use the same future filter; a future or missing validated baseline reports `unknown` |
| F11 | Medium (second review) | Peer evidence accepted a generic `result:pass` and optional oracle, so a transfer-success report could qualify as recovery evidence | **fixed**: oracle binding is mandatory and completion requires `clean-restored`, `manifest-hash` + pass, or a passing clean-restore step |
| F12 | Medium (second review) | Desktop `.exe`/`.dll` payloads were not listed in the release manifest, so a substituted desktop binary could pass package verification | **fixed**: desktop payloads are hashed into the manifest and unmanifested executable members are rejected |

## Verifications run

- `cargo test --all-targets`: 24 tests, including future-timestamp protection/baseline filtering and peer-evidence binding.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- `scripts/v05-retention.py`, `scripts/v08-schedule.py`, `scripts/v08-secret.py`, `scripts/v01-smoke.py` after the fixes.
- `scripts/verify-package.py` on a manifest-bound package; a one-byte binary tamper is detected.
- Windows desktop C# build after the server-PID check.

## Residual evidence gaps

- Cross-user IPC denial still needs a second OS account; the code compares SIDs/UIDs and fails closed, but no cross-account drill exists.
- Elevated same-user tokens are accepted by the SID comparison; the agent should not be run elevated.
- Power-loss, physical disk-full, index rebuild and concurrent capture/restore/transfer during retention remain open.
- External independent review and second-operator/clean-hardware beta evidence remain required.
