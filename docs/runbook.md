# Owner runbook

Status: draft for the experimental/backup-only 0.9 workflow. Commands use a
Windows binary and pinned restic; Linux uses the same arguments with `/` paths.
Never put a password on a command line.

## 1. Install and verify the artifact

1. Retain the package, `SHA256SUMS` and the separately published publisher
   fingerprint/verification instructions.
2. Verify the checksum and publisher signature before extracting.
3. Extract to an owned directory; do not run from a world-writable location.
4. Install the pinned backend with `python scripts/install-backends.py`.
5. Run `peervault --version` and the packaged V0.1/V0.5 drills on disposable
   data.

## 2. Create the repository and secret

```text
peervault init --repository <new-empty-dir> --backend <restic> --password-file <secret>
```

Keep the repository on a separately named device from the source when possible.
Store the repository secret and a recovery copy independently of the source and
repository. Test a readback of the secret before trusting it.

## 3. Capture

```text
peervault capture --repository <repo> --backend <restic> --source <dir> --tag <label> --password-file <secret>
```

A nonzero exit means no complete oracle was published. Check `history`; a
snapshot without `oracle_manifest` is incomplete and is not a recoverable point.

## 4. Verify and restore

```text
peervault verify --repository <repo> --backend <restic> --password-file <secret>
peervault restore --repository <repo> --backend <restic> --snapshot <id> --destination <new-empty-dir> --password-file <secret>
```

Restore to a new empty destination, then compare the restored tree with an
independent oracle. Do not restore over live data.

## 5. Recovery kit and independent copy

```text
peervault recovery-kit generate --repository <repo> --backend <restic> --snapshot <id> --kit <new-dir> --password-file <secret>
peervault recovery-kit verify --kit <dir>
peervault recovery-kit verify --kit <dir> --repository <surviving-copy> --backend <restic> --password-file <separate-secret>
```

Use `scripts/v03-peer-copy.py` for the local-filesystem additive copy drill. A
network transport, pairing and keyless receiver are not supported. Record the
peer repository ID, snapshot mapping and full-read/clean-restore evidence.

## 6. Retention

Initialize state after the first complete capture:

```text
peervault retention state-init --repository <repo> --backend <restic> --state <state> --password-file <secret>
peervault retention peer-record --state <state> --peer-id <peer> --peer-repository-id <id> --snapshot <id> --capture-id <id> --oracle-manifest <id> --key-version v1 --evidence <copy-drill-report>
peervault retention preview --repository <repo> --backend <restic> --policy <policy> --state <state> --password-file <secret> > plan.json
peervault retention apply --plan plan.json --backend <restic> --state <state> --password-file <secret>
```

Review every `delete_candidates` entry and every `mass_change` result. A
`triggered`/`unknown` guard requires an explicit `--ack-mass-change`; do not
acknowledge without checking that the changed set is expected. A missing required
peer root refuses the preview. `retention rebaseline` is an owner-reviewed
baseline replacement, not a deletion bypass.

## 7. Damaged repository

1. Stop scheduled jobs.
2. Preserve the damaged repository and its logs; do not run destructive
   maintenance.
3. Try a full read (`verify`). A failed read is not a restore failure until a
   second independent copy is tested.
4. Recover from the peer/independent copy with separately held keys.
5. Rebuild history from backend snapshots; mark missing verification evidence as
   unknown. Do not invent a pass.

## 8. Lost or wrong secret

Wrong secret: retry with the separately held recovery material; do not overwrite
the repository. Lost secret: recover from a copy whose key material is separately
available. If no copy has usable keys, the data is unrecoverable; record the
incident. Key rotation/rewrap is not part of this build.

## 9. Low space or denied access

- Do not let a low-space condition broaden deletion.
- Free space on a disposable target or add a qualified destination.
- A partial restore target is labelled and must not be treated as complete.
- Retention apply refuses drift and records `partial`/`unknown`; retry requires a
  fresh preview after reconciliation.

## 10. Backend or PeerVault upgrade

1. Capture and verify a complete snapshot.
2. Create a second recovery copy and verify it.
3. Install the new backend/PeerVault side by side; keep the old executable and
   repository secret until the new version restores old snapshots.
4. Run a new-target restore from the old snapshot.
5. Only then run any irreversible backend maintenance.

## 11. Scheduled operation

Run `schedule-tick` under the owner's OS scheduler with a dedicated account.
`schedule-status` reports `unknown`, `fresh`, `stale`, `needs-attention`,
`clock-anomaly` or `disabled`; a green state never overrides an offline peer,
missing key or stale required copy.

Windows DPAPI:

```text
powershell -File scripts/secret-store.ps1 -Action Store -SecretFile <plaintext> -ProtectedFile <secret.dpapi>
powershell -File scripts/install-scheduler.ps1 -Core <peervault.exe> -Config <jobs.json> -State <state.json> -ProtectedSecret <secret.dpapi>
```

Delete the plaintext after storing. The wrapper decrypts to a random owner-only
runtime file for one tick and removes it. Test the exact task account; S4U cannot
read another user's DPAPI blob.

Linux systemd credential:

```text
bash scripts/install-scheduler.sh --core <peervault> --config <jobs.json> --state <state.json> --credential <owner-only-secret> --every 15min
```

The job config may omit `password_file`; `PEERVAULT_PASSWORD_FILE` points at the
systemd credential. Do not put the secret in the unit or command line.

For a restore job, set `minimum_free_bytes` to the declared scratch capacity. A
low-space refusal creates no destination and leaves the job `needs-attention`.
Physical disk-full behavior still requires the qualified capacity drill; do not
treat the preflight as proof against every storage failure.

## 12. Uninstall

1. Stop and remove the scheduled task/timer.
2. Remove the desktop/core binaries and the agent endpoint.
3. **Do not delete the repository or recovery material** unless the owner
   explicitly intends to destroy the backup.
4. Remove credentials from the OS secret store and document which copies remain.
5. Retain the final evidence manifest and recovery instructions.

## Incident record

Record: time, host/profile, repository binding, snapshot/capture refs, operation,
observed phase, exit/report, whether any mutation was attempted, surviving
copies, key availability and the next manual action. Never include secrets or
plaintext content.
