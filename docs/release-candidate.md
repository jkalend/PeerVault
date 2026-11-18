# Release candidate 1 — 2026-11-18

Status: **verified build candidate, not a supported release.** Owner go/no-go and
the external/hardware gates remain open. This document records the exact bytes
that were built and verified; it is not a publication.

## Identity

| Field | Value |
|---|---|
| Commit | `a473985e7a9e4a47187225c2425f55a600ea41dc` |
| Components pin | `3f674a2cbe67bcc25f30145cbcfb58d4dfb846bd` (bailiff Apache-2.0) |
| CI | run `44409613946`, windows-2025 + ubuntu-24.04, pass |
| Package | `peervault-rc1-windows-x64.zip` (framework-dependent desktop payload included) |
| Package SHA-256 | `4286efc2fdc5ea318408f37b6403393d2c097d724453624d0c8be8f55854f8ed` |
| SBOM SHA-256 | `2cd76cd059e0ddb2e61ebfa2010861d626a618bd16cf3d49a48c34d9743ab188` |
| Manifest SHA-256 | `d5228c3e9ae18e20f845a14d777879ff408159da4ba92df7fc6ab2d5c7f5005c` |
| Notices SHA-256 | `a0dbecd9beb001efbf7c19a7655ac1720e2218fadd5549b44d77b7660eebcab9` |

The archive contains 93 members: CLI, desktop payload, docs, scripts, fixtures,
LICENSE, notices, SBOM and release manifest. The release manifest hashes the CLI
and all five desktop payload files; package verification rejects unmanifested
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

Do not publish this artifact as a supported release until
[release-checklist.md](release-checklist.md) is fully green. A draft/publication
action must be explicitly authorized by the owner.
