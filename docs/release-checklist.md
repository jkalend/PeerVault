# Backup-only 0.9 release checklist

Status: **not releasable.** This checklist makes the remaining gates explicit and
separates code work from human/elapsed evidence. Do not publish an artifact while
any P0 row is open.

## Release identity

Freeze and record before testing:

- PeerVault version and commit;
- `components-lock.txt` commit;
- `Cargo.lock` and backend pins;
- OS/profile labels and artifact SHA-256;
- SBOM and `THIRD-PARTY-NOTICES.txt` for the exact artifact set;
- evidence manifest and the retained package archive.

Any code/dependency/packaging change invalidates the affected evidence and
requires a new candidate.

## Gate status

| P | Gate | Status | Required evidence |
|---|---|---|---|
| P0 | Redistribution license | **resolved locally** | bailiff relicensed Apache-2.0 in Components `3f674a2`; publish that commit and update/fetch the pin; `verify-package.py` passes without override |
| P0 | Independent security review | not started | adapter/process, restore containment, credential custody, peer authorization, dependency posture, destructive retention; critical/high fixed and retested |
| P0 | Power-loss / kill-at-phase | partial | scoped VM hard-power-off at observed capture/restore/maintenance phases; prior snapshots and pins survive |
| P0 | Upgrade/rollback | partial | previous-commit read/restore both directions passes; prior supported artifact, interrupted migration and verified pre-migration copy open |
| P0 | Disk-full / denied access | partial | RLIMIT_FSIZE quota drill preserves the baseline and leaves no complete target; physical disk-full, denied access and index rebuild open |
| P0 | 14-day operational beta | not started | 14 consecutive days, >=3 clean-device restores, injected outage/reconnect/low-space, every manual repair recorded |
| P0 | Second operator | not started | another operator follows only the runbook for install/capture/restore/recovery |
| P0 | Clean Ubuntu 24.04 | not started | clean install/ext4 capture/verify/restore and copy drill; WSL2 22.04 is diagnostic only |
| P0 | Independent hardware/media | not started | source disk absent, distinct named media, recovery from peer plus separately held keys |
| P0 | Publisher verification | partial | checksum sidecars exist; signing key/fingerprint and published verification instructions outside the repo |
| P0 | Owner go/no-go | not started | signed decision against the exact artifact/evidence manifest |
| P1 | V0.5 retention | partial | independent security checkpoint, concurrent capture/restore/transfer, disk-full, clock/listing, remote peer roots |
| P1 | V0.6 matrix | partial | Windows/NTFS + Ubuntu 22.04 matrix, cross-OS, previous-commit upgrade and quota drill pass; released-artifact upgrade, target 24.04, power loss and corpus rerun open |
| P1 | V0.8 product surface | partial | IPC, desktop operations, scheduler installers, DPAPI/systemd credentials and quota preflight implemented; retention apply, restart reconciliation, physical stress, accessibility and cross-user/task identity open |
| P1 | Support/security channel | draft | monitored contact and response target published |
| P1 | Compatibility freeze | draft | contract versions and one backend generation frozen |
| P1 | Measured recovery | partial | 5-run 64 MiB harness exists; frozen release artifact, RSS/CPU/transfer bytes, controlled cold cache and preselected windows open |
| P2 | Documentation | draft | runbook, support policy, compatibility, limits and install/uninstall reviewed against the artifact |

## Automated candidate check

Run from a clean tree:

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
python -m unittest discover -s scripts -p 'test_*.py'
python scripts/check-docs.py
```

Then, with the pinned backend and disposable roots:

```text
python scripts/v01-smoke.py --binary <peervault> --backend <restic> --root <new-root>
python scripts/v02-kit-smoke.py --binary <peervault> --backend <restic> --root <new-root>
python scripts/v03-peer-copy.py --binary <peervault> --backend <restic> --source-root <new> --peer-root <new> --recovery-root <new>
python scripts/v05-retention.py --binary <peervault> --backend <restic> --root <new-root>
python scripts/v06-matrix.py --binary <peervault> --backend <restic> --root <new-root>
python scripts/v06-cancel.py --binary <peervault> --backend <restic> --root <new-root>
python scripts/v08-ipc.py --binary <peervault> --backend <restic> --root <new-root>
```

Package and verify:

```text
cargo build --release --locked
python scripts/package-notices.py dist/THIRD-PARTY-NOTICES.txt
python scripts/sbom.py --output dist/sbom.cdx.json
python scripts/release-manifest.py --package dist/peervault --sbom dist/sbom.cdx.json --output dist/release-manifest.json --gate fmt=pass --gate clippy=pass --gate tests=pass
python scripts/verify-package.py <archive>
```

`--allow-license-block` is forbidden for an RC. `verify-package.py` must pass
without it.

## Human-only decisions

1. Resolve the bailiff/Components license and record the decision.
2. Name the independent security reviewer and close critical/high findings.
3. Provide clean Ubuntu 24.04 and independent Windows hardware/media.
4. Run the 14-day beta and three clean-device drills.
5. Have a second operator follow the runbook unaided.
6. Publish the security-report channel and publisher fingerprint.
7. Sign the go/no-go against the exact manifest.

## Stop conditions

Do not publish if any of these is true:

- `git status` is dirty or the artifact was rebuilt after testing;
- the package checksum/SBOM/notices do not match the manifest;
- the license review is blocked;
- a P0 test is failed/partial/unknown or its evidence was not retained;
- the only copy of the repository/key exists on the source device;
- a restore into a new target was not independently compared to an oracle;
- the owner has not signed the exact artifact decision.
