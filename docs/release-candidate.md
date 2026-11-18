# Release candidate 0.9.0-rc.1 — 2026-11-18

Status: **release candidate for superproject integration, not a supported
release.** Owner go/no-go for supported 0.9 and the external/hardware gates
remain open. Draft tag: `v0.9.0-rc.1`.

## Identity

| Field | Value |
|---|---|
| Commit | the commit tagged `v0.9.0-rc.1` |
| Components pin | `3f674a2cbe67bcc25f30145cbcfb58d4dfb846bd` (bailiff Apache-2.0) |
| Crate/artifact version | `0.9.0-rc.1` |
| Windows package | `peervault-windows-2025.zip` (framework-dependent desktop payload included) |
| Linux package | `peervault-ubuntu-24.04.tar.gz` |
| Hashes | recorded in the draft release body and this file after the tag build |

The Windows archive includes the CLI, desktop payload, docs, scripts, fixtures,
LICENSE, notices, SBOM and release manifest. The release manifest hashes the CLI
and all desktop payload files; package verification rejects unmanifested
executables.

## Verification performed

```text
python scripts/verify-package.py <archive> --require-desktop
# result: pass
# license_review: all resolved components declare a license expression
# manifest_artifacts: 6, sbom_components: 54
```

The package checksum sidecar matches; the packaged binary/SBOM match the packaged
manifest; notices are generated from the resolved dependency graph. Earlier in
the same commit family: 24 Rust tests, clippy `-D warnings`, Python tests, docs
checks, V0.1/V0.2/V0.3/V0.5/V0.6/V0.8 drills, QEMU hard-power-off, ext4 loopback
ENOSPC, index rebuild, Windows/Unix denial, and clean Ubuntu 24.04 container
drills.

## Open before calling this supported

- External independent security review (two internal reviews and F1–F12 fixes
  are complete).
- 14-day beta, three clean-device restores, second operator.
- Physical media failure and independent hardware/source-disk-absent recovery.
- Publisher signing/fingerprint and owner go/no-go against this exact manifest.

This draft exists for superproject pinning and review. Do not relabel it as a
supported release until [release-checklist.md](release-checklist.md) is fully
green and the owner records go/no-go against the published hashes.
