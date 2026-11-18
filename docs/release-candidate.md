# Release candidate 0.9.0-rc.1 — 2026-11-18

Status: **release candidate for superproject integration, not a supported
release.** Owner go/no-go for supported 0.9 and the external/hardware gates
remain open. Draft tag: `v0.9.0-rc.1`.

## Identity

| Field | Value |
|---|---|
| Commit | `687cb60eacf5fd2518719b5e3cc4c278a5ea87fd` |
| Components pin | `3f674a2cbe67bcc25f30145cbcfb58d4dfb846bd` (bailiff Apache-2.0) |
| Crate/artifact version | `0.9.0-rc.1` |
| Windows package | `peervault-windows-2025.zip` (85 members; framework-dependent desktop payload included) |
| Windows SHA-256 | `d0eb1898a4ac9293b7a398e91f7a12e2c33f0dd44708434c6ad82525d92f1f2a` |
| Linux package | `peervault-ubuntu-24.04.tar.gz` (89 members) |
| Linux SHA-256 | `3dc9669f7d7323479913b4a072c81b88f429db977553b0ece20222523ebe10b5` |
| SBOM SHA-256 (Windows) | `adb1299539c2bf1030635340aabad0a6bb656efa43235700bcdc74557ab0d4d3` |
| Manifest SHA-256 (Windows) | `d47fbc4de6418eacd24e423a4eb8dfe95558f2d1bd47795c2c772013ef869da2` |
| Notices SHA-256 (Windows) | `a0dbecd9beb001efbf7c19a7655ac1720e2218fadd5549b44d77b7660eebcab9` |

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
