# License and redistribution review — 2026-11-18

Status: **resolved and published.** The owner selected Apache-2.0 for the bailiff
component. Components `3f674a2cbe67bcc25f30145cbcfb58d4dfb846bd` is on
`origin/main`, and PeerVault pins it in `components-lock.txt`. This is a
release-gate record, not legal advice.

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
| PeerVault | 0.9.0-rc.1 | Apache-2.0 | yes |
| bailiff | 0.0.1 | Apache-2.0 | yes (linked) |
| restic | 0.19.1 | BSD-2-Clause | no; installed separately by pinned checksum |
| kopia | 0.23.1 | Apache-2.0 | no; R0 comparison only |
| .NET runtime (desktop) | 10 | MIT | framework-dependent build; confirm the deployment model before publishing |

`THIRD-PARTY-NOTICES.txt` includes the resolved crate license files, and the
SBOM records the exact component set for the artifact.

## Publication verification

The Components commit and the PeerVault pin are published. CI run `44386964143`
on `26164ce` cloned the pinned commit, built both platforms, and passed package
verification without `--allow-license-block`.

## Other checks

- No cryptography is custom; repository encryption/format is restic's.
- PeerVault stores no telemetry and opens no listening network port.
- Publisher signing keys must remain outside the repository; a checksum alone is
  not publisher authentication.
- The final RC must retain the exact dependency/backend notices for the published
  artifact set, not for a neighboring commit.
