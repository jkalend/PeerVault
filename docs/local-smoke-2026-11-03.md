# Local single-host smoke — 2026-11-03

Status: experimental same-host evidence. This does not satisfy independent-media,
clean-hardware, second-operator or supported-release gates.

Environment:

- Windows 11 host (`10.0.26200`), Rust release binary SHA-256
  `d71330cf6c2d5348367f926c6562415ef416ce2952590fed733a3ef462d37a1e`
- pinned restic `0.19.1` from `.tools/restic.exe`
- source/repository roots on `G:`; restore/copy roots also on `G:` except where
  noted; one host/controller/power failure domain
- source tree: `.tools` excluded; all roots disposable
- VM note: the QEMU/BIOS Windows VM is a clean-OS simulation on the same host. It
  is useful for Windows-only UI tests but does not create independent hardware.

## Results

| Check | Result | Evidence / root |
|---|---|---|
| V0.1 capture/history/verify/restore | **pass** | `G:\PV-tests\v01-20261103`; two exact restores, independent oracle, wrong-secret/corruption refusals |
| V0.2 copy/source-removal/kit/recovery | **pass** | `G:\PV-tests\v02-20261103`; kit full read, four exact restored files, source absent |
| V0.3 one-peer local copy/recovery | **pass** | `G:\PV-tests\v03-20261103`; `PeerCopyRecord` state `clean-restored`, source absent, four exact restored files |
| V0.5 retention preview | **pass (preview only)** | exercised inside V0.1; no deletion/maintenance |
| V0.6 5 GiB/50k corpus | **current rerun failed** | source `G:` + restore `F:` exceeded the 900 s harness limit; failed roots cleaned |
| V0.8 scheduler/status/agent/desktop | **partial pass** | unit tests, CLI/agent smokes and C# build; no named-pipe IPC or full desktop/stress matrix |
| V0.9 measured beta | **not attempted** | needs elapsed cadence, independent recovery, second operator and RC gates |

V0.3 record SHA-256:
`8bdae4eb6a77304bd02459b61df43710db58cff0f29409aa153d40057f672e2d`.

V0.3 record summary:

- capture `dc6646ab40559e182b79fd7923993444`
- source snapshot `a2ca0cdb1813c244c7ab5ac43b43ed76724e5221a92182d9ce73978a4d69bea8`
- peer snapshot `9d9c9ef8aa0b52daee77df0ac2162b9c4426acb691553f95f3c579a74b1ea94e`
- state `clean-restored`; source absent; local-filesystem transport only
- limitations: same host, no authenticated transport/pairing/keyless receiver,
  no interrupted/replaced-mount fault injection, author-only

The 2026-11-01 V0.6 pass (664.5 s) remains the only documented corpus pass. The
2026-11-03 rerun failing the declared 900 s limit is retained as a current
performance/qualification failure, not replaced by the earlier run.

## Remaining single-host work

- V0.6 metadata/path matrix, upgrade and power-loss tests; corpus rerun after
  resolving the time limit.
- V0.8 full owner-only named-pipe/Unix-socket IPC, OS schedulers, secret delivery,
  quota/restart/suspend tests and full desktop operations.
- V0.5 remains preview-only; destructive maintenance is blocked by its independent
  security gate.

## Deferred until secondary hardware

V0.2 clean hardware, V0.3 authenticated network/keyless recovery, V0.6 independent
beta, V0.7 two-device sync and V0.9 measured/independent recovery. A second VM or
second local root does not remove the shared host/controller/power failure domain.
