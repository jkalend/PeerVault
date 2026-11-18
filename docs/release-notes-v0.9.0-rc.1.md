# PeerVault v0.9.0-rc.1 — release candidate notes

**Status: release candidate for superproject integration, not a supported
release.** Do not describe this as supported backup-only 0.9 or 1.0.

## Artifact identity

- Tag: `v0.9.0-rc.1`
- Crate/artifact version: `0.9.0-rc.1`
- Components pin: `3f674a2cbe67bcc25f30145cbcfb58d4dfb846bd`
- Published asset hashes: recorded in this release body after the tag build
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
