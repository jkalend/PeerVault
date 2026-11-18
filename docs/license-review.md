# License and redistribution review — 2026-11-18

Status: **resolved locally.** The owner selected Apache-2.0 for the bailiff
component. The change is committed in Components as
`3f674a2cbe67bcc25f30145cbcfb58d4dfb846bd`; this is a release-gate record, not
legal advice.

## Resolution

- `Components/crates/bailiff/Cargo.toml` now declares `license = "Apache-2.0"`.
- `Components/crates/bailiff/LICENSE` now contains the workspace Apache-2.0 text.
- The previous all-rights-reserved text is superseded; it is retained in
  Components Git history.
- PeerVault pins the relicensed Components commit in `components-lock.txt`.
- `scripts/sbom.py` now reports
  `all resolved components declare a license expression`, and
  `scripts/verify-package.py` passes without `--allow-license-block`.

## Product and dependencies

| Component | Version | License | Bundled |
|---|---|---|---|
| PeerVault | 0.0.1 | Apache-2.0 | yes |
| bailiff | 0.0.1 | Apache-2.0 | yes (linked) |
| restic | 0.19.1 | BSD-2-Clause | no; installed separately by pinned checksum |
| kopia | 0.23.1 | Apache-2.0 | no; R0 comparison only |
| .NET runtime (desktop) | 10 | MIT | framework-dependent build; confirm the deployment model before publishing |

`THIRD-PARTY-NOTICES.txt` includes the resolved crate license files, and the
SBOM records the exact component set for the artifact.

## Publication requirement

The Components commit and the PeerVault pin update are local until pushed. CI
clones `components-lock.txt` from the remote, so both repositories must be
published for reproducible CI/release evidence. A release candidate built before
that push does not satisfy the distribution gate.

## Other checks

- No cryptography is custom; repository encryption/format is restic's.
- PeerVault stores no telemetry and opens no listening network port.
- Publisher signing keys must remain outside the repository; a checksum alone is
  not publisher authentication.
- The final RC must retain the exact dependency/backend notices for the published
  artifact set, not for a neighboring commit.
