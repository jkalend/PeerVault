# PeerVault v0.9.0-rc.1 — release candidate notes

**Status: release candidate for superproject integration, not a supported
release.** Do not describe this as supported backup-only 0.9 or 1.0.

## Artifact identity

- Tag: `v0.9.0-rc.1`
- Commit: `687cb60eacf5fd2518719b5e3cc4c278a5ea87fd`
- Crate/artifact version: `0.9.0-rc.1`
- Components pin: `3f674a2cbe67bcc25f30145cbcfb58d4dfb846bd`
- Windows package SHA-256: `d0eb1898a4ac9293b7a398e91f7a12e2c33f0dd44708434c6ad82525d92f1f2a` (85 members, 54 SBOM components)
- Linux package SHA-256: `3dc9669f7d7323479913b4a072c81b88f429db977553b0ece20222523ebe10b5` (89 members, 59 SBOM components)
- SBOM SHA-256 (Windows): `adb1299539c2bf1030635340aabad0a6bb656efa43235700bcdc74557ab0d4d3`
- Manifest SHA-256 (Windows): `d47fbc4de6418eacd24e423a4eb8dfe95558f2d1bd47795c2c772013ef869da2`
- Notices SHA-256 (Windows): `a0dbecd9beb001efbf7c19a7655ac1720e2218fadd5549b44d77b7660eebcab9`
- SBOM: CycloneDX 1.5, all licenses declared
- Package verifier: checksum, manifest artifact/SBOM binding, desktop executable
  manifest binding and license review pass without overrides on both downloaded
  assets

## Included capabilities

- restic 0.19.1 repository-v2 init/capture/history/full-read/verify/exact restore.
- Encrypted tree oracle; whole-snapshot restore into a new empty destination only.
- Secret-free RecoveryKit and author-only copy drill; one-peer local-filesystem
  copy record.
- Owner-executed retention: state generation, preview token, peer-root pins,
  floors, mass-change pause, backend-native `forget --prune`, partial/unknown
  reconciliation.
- Owner-only named-pipe/Unix-socket IPC, Windows desktop capture/history/verify/
  restore/preview, OS scheduler installers, DPAPI/systemd credential paths.
- Metadata/path matrix, cancellation, cross-OS, previous-commit upgrade,
  quota/tmpfs/loopback ENOSPC, Unix/Windows denial, index rebuild and QEMU guest
  hard-power-off evidence.

## Deferred / not claimed

- External independent security review and second-operator/clean-hardware beta.
- Physical media failure and independent-hardware source-disk-absent recovery.
- 14-day operational beta and three clean-device restores.
- Publisher signing/fingerprint and owner go/no-go for a supported release.
- Live-folder sync (V0.7), VSS/network/cloud roots, selective/merge/overwrite
  restore, key rotation and remote pruning.

Supported backup-only 0.9 requires the remaining P0 gates in
`docs/release-checklist.md`; this draft exists so a superproject can pin and
exercise the candidate. Report failures against the exact published asset hashes.
