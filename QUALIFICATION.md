# PeerVault experimental qualification package

This package contains the experimental R0 fixture/compare/bakeoff/copy-drill CLI
for synthetic data. It is not a production backup tool, does not select a
backend and makes no supported-recovery claim.

The full runbook, command references and limits are in [docs/r0.md](docs/r0.md).
The open findings and dispositions are in
[docs/implementation-catch-up.md](docs/implementation-catch-up.md). This package
ships those documents, the pinned backend installer, the independent fixture
checker, the evidence exporter and the fixture goldens it needs.

## Contents

```text
peervault(.exe)                 experimental CLI
backend-lock.json               sole version/archive/executable pin source
components-lock.txt             Components commit used for this build
scripts/install-backends.py     bounded, checksum-verified backend installer
scripts/check-fixture.py        independent Python fixture oracle
scripts/check-windows-imports.py  Windows static-runtime import gate
scripts/evidence.py             append-only evidence exporter/verifier
scripts/v01-smoke.py            repeatable V0.1 capture/restore smoke
scripts/v02-kit-smoke.py        RecoveryKit inventory/full-read smoke
scripts/v02-clean-drill.py      author-only copy/source-removal/recovery drill
scripts/v03-peer-copy.py        one-peer additive copy record drill
scripts/windows/               clean-Windows VM harness and test-media builder
tests/fixtures/v1-a.json        revision 1 golden
tests/fixtures/v1-b.json        revision 2 golden
docs/                           runbook, roadmap, catch-up plan, profiles and planning inputs
QUALIFICATION.md                this guide
```

The backup backends are external and are not bundled. Python 3.10+ is required
for the scripts; Rust is not needed to use this package. Windows binaries are
built with the static C runtime and must not require a separately installed
Visual C++ redistributable; `scripts/check-windows-imports.py` is the release
gate for that property.

The included planning documents are contextual. Links in them to `src/`,
`.github/`, `Components/` or `project-planning/` refer to the source workspace
and are not resolved inside this package.

## Install the pinned backends

```text
python scripts/install-backends.py
```

The installer verifies the version/URL agreement in `backend-lock.json`, the
downloaded archive SHA-256 and the extracted executable SHA-256 before replacing
anything. Installations are written to `.tools/`.

## Verify the independent fixture

Windows PowerShell:

```powershell
.\peervault.exe fixture .peervault-work-fixture-a
python scripts/check-fixture.py .peervault-work-fixture-a/source --revision 1
.\peervault.exe fixture .peervault-work-fixture-b --revision 2
python scripts/check-fixture.py .peervault-work-fixture-b/source --revision 2
```

Linux:

```text
./peervault fixture .peervault-work-fixture-a
python scripts/check-fixture.py .peervault-work-fixture-a/source --revision 1
./peervault fixture .peervault-work-fixture-b --revision 2
python scripts/check-fixture.py .peervault-work-fixture-b/source --revision 2
```

Each fixture and work directory must be new. The Python checker reads and hashes
the source itself; backend output never supplies expected hashes.

## Verify V0.1 local capture and restore

After installing the backends:

```text
python scripts/v01-smoke.py --binary ./peervault --backend .tools/restic --root .peervault-work-v01-smoke
```

On Windows use `.\peervault.exe` and `.tools\restic.exe`. The script creates a
new work root, captures two snapshots, restores both exactly, checks the oracle,
empty selection, wrong password and a truncated repository. It exits nonzero on
any mismatch.

## Generate and verify a recovery kit

```text
python scripts/v02-kit-smoke.py --binary ./peervault --backend .tools/restic --root .peervault-work-v02-kit-smoke
python scripts/v02-clean-drill.py --binary ./peervault --backend .tools/restic --work-root <new-source-root> --copy-root <new-copy-root> --recovery-root <new-recovery-root>
```

The first checks kit inventory, repository binding, full read, wrong secret and
tamper refusal. The second is an author-only copy/source-removal/recovery drill
that needs three new disjoint roots. Neither replaces an independent-operator or
clean-hardware drill.

## Exercise one-peer copy

```text
python scripts/v03-peer-copy.py --binary ./peervault --backend .tools/restic --source-root <new-source-root> --peer-root <new-peer-root> --recovery-root <new-recovery-root>
```

This records `requested` through `clean-restored`, removes the source, full-reads
the peer and restores exactly. The transport is local filesystem only;
authenticated network transport and independent hardware remain open.

## Run the Windows clean-client drill

See [docs/windows-clean-drill.md](docs/windows-clean-drill.md) for the media
layout, unattended answer files, static-runtime import gate and serial result
contract. The VM must reach the test with no manual OOBE interaction; a visible
OOBE page is a provisioning failure, not a prompt to click through. The validated
lab path is QEMU/SeaBIOS with `autounattend-bios.xml`; the VMware/OVMF UEFI path
remains open. This drill is experimental clean-VM evidence, not a CP2 or V0.2
gate pass.

## Preview retention

```text
peervault retention-preview --repository <repo> --backend .tools/restic --policy docs/policies/retention-preview-v1.json --password-file <secret>
```

This is read-only. It never deletes snapshots or runs backend maintenance; see
[V0.5 limits](docs/v0.5.md).

## Run the V0.6 corpus prerequisite

```text
python scripts/v06-corpus-smoke.py --binary ./peervault --backend .tools/restic --source-root <new-source-root> --restore-root <new-restore-root>
```

This creates 5 GiB + 50,000 small files, captures, full-reads and restores them.
It needs several GiB of free space and takes minutes; it is a prerequisite, not a
beta qualification. See [corpus evidence](docs/v0.6-corpus.md).

## Run a smoke baseline

Use a profile that matches the host OS and architecture. The checked-in profiles
are smoke profiles, not scored qualification profiles.

Windows PowerShell:

```powershell
.\peervault.exe bakeoff restic --executable .tools\restic.exe --work-dir .peervault-work-restic --profile docs\profiles\windows11-workstation-smoke.json
.\peervault.exe bakeoff kopia --executable .tools\kopia.exe --work-dir .peervault-work-kopia --profile docs\profiles\windows11-workstation-smoke.json
```

Linux:

```text
./peervault bakeoff restic --executable .tools/restic --work-dir .peervault-work-restic --profile docs/profiles/wsl-ubuntu22-smoke.json
./peervault bakeoff kopia --executable .tools/kopia --work-dir .peervault-work-kopia --profile docs/profiles/wsl-ubuntu22-smoke.json
```

The checked-in Linux profile is Ubuntu 22.04, not the target Ubuntu 24.04
qualification profile. Omit `--profile` for an explicitly unnamed smoke run.

The `copy-drill` command additionally needs three new, disjoint roots on three
observed physical devices; see [docs/r0.md](docs/r0.md).

## Retain evidence

```text
python scripts/evidence.py export .peervault-work-restic .peervault-work-evidence --profile docs/profiles/windows11-workstation-smoke.json
python scripts/evidence.py verify .peervault-work-evidence/R0/<profile-id>/<run-id>
```

Export refuses fixture goldens or a profile that changed since the run, and never
overwrites an existing run directory. `verify` checks the stored inventory; a
passing inventory identifies bytes, not truth, authority or recoverability.
