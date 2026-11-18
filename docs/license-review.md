# License and redistribution review — 2026-11-18

Status: **blocked for binary distribution.** This is a release-gate finding, not
legal advice.

## Product

- PeerVault is licensed under Apache-2.0 (`LICENSE`).
- PeerVault 0.0.1 links `bailiff` as a path dependency and ships it inside the
  binary. `Components/crates/bailiff/Cargo.toml` declares `license-file = "LICENSE"`.
  That file says:

  > All rights reserved until a distribution license is chosen. No permission is
  > granted to use, copy, modify, or distribute this software except as required
  > to evaluate this repository and its declared consumers.

  Apache-2.0 on PeerVault cannot grant rights to bailiff. A public binary that
  contains bailiff is not redistributable on the current terms.

## External backends

| Component | Version | License | Bundled |
|---|---|---|---|
| restic | 0.19.1 | BSD-2-Clause | no; installed separately by pinned checksum |
| kopia | 0.23.1 | Apache-2.0 | no; R0 comparison only |
| .NET runtime (desktop) | 10 | MIT | framework-dependent build; confirm the deployment model before publishing |

`THIRD-PARTY-NOTICES.txt` includes the resolved crate license files, including
bailiff's current all-rights-reserved text. `scripts/sbom.py` marks the review as
`blocked: bailiff` instead of inventing an SPDX license.

## Required owner decision (critical)

Choose one before any supported binary or container is published:

1. Select and commit a distribution license for `Components` (for example
   Apache-2.0 with the owner's authority), then regenerate notices/SBOM and
   re-run package verification.
2. Replace the bailiff dependency with first-party code or a permissively
   licensed crate, then regenerate the SBOM and dependency notices.
3. Keep releases source-only. That contradicts the planned supported 0.9 binary
   and clean-install recovery gates, so it is not a release path without an
   explicit scope change.

Until one path is completed, `scripts/verify-package.py` fails on the SBOM
license review unless the experimental `--allow-license-block` flag is passed.
That flag is for disposable CI packages only, never for an RC.

## Other checks

- No cryptography is custom; repository encryption/format is restic's.
- PeerVault stores no telemetry and opens no listening network port.
- Publisher signing keys must remain outside the repository; a checksum alone is
  not publisher authentication.
- The final RC must retain the exact dependency/backend notices for the published
  artifact set, not for a neighboring commit.
