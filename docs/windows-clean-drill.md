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

For the tested SeaBIOS/BIOS lab path, copy `autounattend-bios.xml` over
`autounattend.xml` in the staging directory before running the builder. It uses
an active primary system partition instead of EFI/MSR partitions. The UEFI
answer file remains separate because its VMware/OVMF boot path is not yet
validated.

## Tested QEMU/BIOS runner

The passing lab run used QEMU 6.2 with KVM and SeaBIOS. A representative command
is:

```text
qemu-system-x86_64 -machine pc,accel=kvm -cpu host -smp 4 -m 6144 \
  -drive file=<blank-80g.qcow2>,format=qcow2,if=ide,index=0 \
  -drive file=<secret.iso>,media=cdrom,readonly=on,if=ide,index=1 \
  -drive file=<windows-install.iso>,media=cdrom,readonly=on,if=ide,index=2 \
  -drive file=<peervault-test-bios.iso>,media=cdrom,readonly=on,if=ide,index=3 \
  -boot order=d,menu=off -display none -vnc 127.0.0.1:1 \
  -serial file:serial.log -monitor tcp:127.0.0.1:4445,server,nowait
```

The guest reaches the desktop with no input, runs the first-logon test and shuts
down. Use the monitor only for diagnostics; a successful run needs no manual
OOBE or boot selection. OVMF/UEFI and VMware boot paths remain separate open
issues.

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
build imported `VCRUNTIME140.dll`; the static build removes that dependency. A
second run with the static binary passed RecoveryKit verification but the
harness had pre-created `C:\restored`, so restore was refused and the run was
correctly `ok: false`. This document records the repaired harness.

A host-side execution of the same test media and static binary on 2026-11-02
passed RecoveryKit verify, restore, the four-file count, all expected sizes and
SHA-256 hashes (`ok: true`). This validates the product/harness data path, not
a clean VM boot or OOBE path.

A fresh unattended lab run then passed end-to-end on 2026-11-02 using:
QEMU 6.2 with KVM, SeaBIOS (BIOS), the Windows 11 Enterprise Eval 26H2 x64
image, a blank 80 GiB disk, `peervault-test-bios.iso` built from
`scripts/windows/autounattend-bios.xml`, and the separate secret ISO. The run
reached the desktop with no OOBE input, executed the first-logon test, and
reported:

| Field | Result |
|---|---|
| verify | `pass`; `repository_bound: true`; `full_read: true` |
| restore | `pass`; `files: 4`; `directories: 2`; `bytes: 1048593` |
| expected/restored count | `4 / 4` |
| hash mismatches | none |
| serial `ok` | `true` |

The retained serial SHA-256 is
`40c78009c355a79fff5ef9544189f806c1cdebd41e17fd0e559988040b6ea0d8`;
the BIOS test-media ISO SHA-256 is
`507957945123fed1a72af9f73b7b43137b49ec8098d0b6a7e9a144928f87998d`;
the BIOS answer-file SHA-256 is
`dfd899907147b033c2b8cf03e75f86951fdcf7434771e3787708d26037faf63f`;
the static binary SHA-256 is
`9eedd329588f7259aa6694e46e53bcf825a4b1582fe9c7d159cc1d9e3a66e3f1`.
The Windows image SHA-256 is
`bc3f24086ebadc94489066b5ad78089e2cf5c3491e90e790bb81a2b199c10e38`.

This pass is a **BIOS lab profile**, not a UEFI VMware/OVMF or independent-media
qualification. VMware UEFI stopped at the Boot Manager and QEMU OVMF timed out
starting this installer ISO; those paths remain unvalidated. Windows 11 on BIOS
is suitable only for this disposable recovery-harness experiment. See
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
