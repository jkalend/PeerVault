# PeerVault v0.0.1-rc.1 — release candidate notes

**Status: release candidate for superproject integration, not a supported
backup release.** Do not describe this as backup-only 0.9 or 1.0.

## Artifact identity

- Commit: `a473985e7a9e4a47187225c2425f55a600ea41dc`
- Components pin: `3f674a2cbe67bcc25f30145cbcfb58d4dfb846bd`
- Windows published package SHA-256: `4cb7e9c21d8ec4130b4843631010e7a3d9d7c0a66a6722ffb190506224c6f1ff` (85 members, 54 SBOM components)
- Linux published package SHA-256: `67abca447a9f05fa3529a92ba836bf4a9e762089fbef99f81da0e5e1a3b9e65f` (89 members, 59 SBOM components)
- CI: run `44409613946` (windows-2025 + ubuntu-24.04) plus tag run `44411439476`
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
exercise the candidate. Report failures against the exact artifact hash above.
