# Windows clean-client RecoveryKit drill

Status: experimental harness, 2026-11-02. This is not a CP2, R0 or V0.2 gate
pass. It exercises RecoveryKit verification and exact restore from read-only
copied media in a fresh Windows VM. The source workspace, original device and
writer state are absent from the VM; independent hardware, key custody and
network topology are not tested.

## Scope

The drill boots a new Windows VM with:

- one blank SATA disk and the Windows installation ISO;
- a read-only `PEERVAULT_TEST` ISO containing the compiled CLI, pinned restic,
  the copied repository, RecoveryKit, independent `expected.json`, the
  `autounattend.xml` and `run-test.ps1`;
- a separate read-only `PEERVAULT_SECRET` ISO containing only `secret.txt`;
- a serial file as the result channel.

No manual OOBE input is accepted. The answer file configures `oobeSystem`
locale settings, hides the documented account pages, creates the disposable
local administrator and uses the deprecated `SkipMachineOOBE`/`SkipUserOOBE`
settings needed to suppress the remaining region/network pages on this lab
image. If OOBE still appears, the run is a provisioning failure; do not click
through it and call the drill automated.

## Build gate

Windows recovery media must not require a separately installed VC++
redistributable. The workspace pins:

```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
```

After a release build, reject forbidden CRT imports:

```text
python scripts/check-windows-imports.py target/release/peervault.exe
```

The expected imports are Windows inbox DLLs only. A `VCRUNTIME140.dll`,
`MSVCP*.dll`, `UCRTBASE.dll` or `api-ms-win-crt-*` import fails the gate.
The historical failure was `0xC0000135` (`STATUS_DLL_NOT_FOUND`) from a fresh
Windows VM.

## Media build

The source directory must contain:

```text
autounattend.xml
run-test.ps1
peervault.exe
restic.exe
expected.json
repository/
kit/
```

Build the test ISO on Windows PowerShell 5.1+:

```powershell
.\scripts\windows\build-iso.ps1 `
  -SourceDirectory <media-source> `
  -OutputIso <output>\peervault-test.iso `
  -VolumeLabel PEERVAULT_TEST `
  -Force
```

The separate `PEERVAULT_SECRET` ISO is not built by this script and must contain
`secret.txt`. Keep the secret out of the test ISO and source tree. The committed
`autounattend.xml` contains a disposable lab credential only; it is not a
product secret.

## Harness contract

`run-test.ps1` runs from the `PEERVAULT_TEST` volume. It:

1. refuses missing `PEERVAULT_TEST`/`PEERVAULT_SECRET` volumes;
2. requires exactly four expected file entries;
3. copies the CLI and restic to `C:\peervault`;
4. removes any stale `C:\restored` and passes that **nonexistent** destination
   to `restore` (the CLI intentionally rejects an existing destination);
5. records verify and restore exit codes without letting redirected stderr
   terminate PowerShell before the exit code is captured;
6. compares every expected path, size and SHA-256 and requires the restored file
   count to equal the expected count;
7. writes the JSON result to COM1 between `PEERVAULT_RESULT_BEGIN` and
   `PEERVAULT_RESULT_END`, then shuts down.

A valid result has `verify_exit: 0`, `restore_exit: 0`,
`expected_file_count: 4`, `file_count: 4`, no hash mismatches and `ok: true`.

## Validation status

The first clean-VM attempt failed before the binary started because the Windows
build imported `VCRUNTIME140.dll`; the static build removes that dependency. Second run with the static binary passed RecoveryKit verification but the
harness had pre-created `C:\restored`, so restore was refused and the run was
correctly `ok: false`. This document records the repaired harness.

A host-side execution of the same test media and static binary on 2026-11-02
passed RecoveryKit verify, restore, the four-file count, all expected sizes and
SHA-256 hashes (`ok: true`). This validates the product/harness data path, not
the clean VM, boot selection or OOBE path.

A fresh run with the repaired media and unattended answer file did not reach
Windows Setup: VMware's UEFI firmware stopped at the Boot Manager screen.
`efi.bootOrder` changed the displayed device order but did not select the
installer, and no guest input was used. Therefore zero-click OOBE and a passing
restore remain unvalidated. VM provisioning must force the installer boot without
firmware interaction before this drill contributes passing evidence. See
[CP2 copy drill progress](cp2-copy-drill.md) for the gate's remaining
independent-media and fault work.

## Limits

This harness does not prove:

- a clean device with removed source hardware;
- independent storage media or physical-disk absence;
- separate key custody, rotation or wrong-secret recovery;
- arbitrary archive extraction, selective restore, resume or overwrite;
- network/keyless peer behavior, ransomware resistance or backend maintenance;
- CP2 receiver inventory, media-identity, source-removal or scored R0 conditions.
