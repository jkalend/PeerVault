# Local single-host smoke — 2026-11-18

Status: experimental same-host evidence for the V0.5/V0.6/V0.8 additions. It does
not satisfy independent-media, clean-hardware, second-operator, power-loss or
supported-release gates.

## Environment

- Windows 11 `10.0.26200.9974` x64, NTFS
- Ubuntu 22.04.4 LTS WSL2, kernel `6.18.58.1-microsoft-standard-WSL2`, ext4
- pinned restic 0.19.1
- release `peervault` for the Windows drills unless noted; debug Linux build for
  the WSL2 matrix/IPC drills

## Results

| Check | Result | Evidence / limit |
|---|---|---|
| V0.1 capture/history/verify/restore | pass | `.peervault-work-v01-rel2`; two exact restores, rejection/corruption paths |
| V0.2 RecoveryKit | pass | `.peervault-work-v02-rel2`; inventory, full read, wrong secret and tamper refusal |
| V0.3 one-peer copy | pass | clean-restored record; same-host local filesystem, author-only |
| V0.5 retention preview/apply | pass | `.peervault-work-v05-rel2`; 2 kept, 2 deleted, peer root pinned, stale replay refused, mass-change pause honored |
| V0.6 metadata/path matrix | pass | Windows NTFS and WSL2 ext4; long path/unicode/readonly supported, links/special/case collisions rejected, metadata loss recorded |
| V0.6 cancellation/serialization | pass | exit 130, prior snapshot intact, concurrent capture/verify refused as busy |
| V0.6 cross-OS restore | pass | Windows->Linux and Linux->Windows through WSL2, exact 6-file/2-directory oracle |
| V0.6 previous-candidate upgrade | pass | old commit -> new reads/restores old; old reads/restores new |
| V0.6 quota-style capacity | pass | limited capture/restore fail without a complete target; baseline restores |
| V0.8 owner-only IPC | pass | named pipe and Unix socket; allowlist/version refusals, history/status, second-owner refusal, restart |
| V0.8 scheduler/DPAPI | pass | status/fresh/missed/clock/quota; DPAPI runtime secret removed after a passing tick |
| V0.9 repeated measurement | pass | 5 runs, 64 MiB + 100 files; see `v0.9-measure.md` |

`cargo test --all-targets` passed 21 tests. `cargo clippy --all-targets -- -D
warnings` and `cargo fmt --check` passed. Python unit tests and the document-link
check passed. The local package pipeline generated notices, a 54-component SBOM,
a release manifest and a checksummed archive; package verification passed with
the license override.

## Open limits

- Power loss, physical disk-full, index rebuild, denied-access injection and a
  released-artifact upgrade are not covered. Quota-style capacity and a
  previous-commit upgrade rehearsal pass.
- The Linux profile is WSL2 Ubuntu 22.04, not clean Ubuntu 24.04.
- IPC cross-user denial needs a second OS account.
- Retention apply is not exercised concurrently with a real capture/restore
  transfer; the lock behavior is covered by the cancellation drill.
- The 14-day beta, three clean-device drills, second operator and independent
  hardware remain open.
- `bailiff` is all-rights-reserved in the Components workspace; binary
  redistribution is blocked until the owner resolves it.
