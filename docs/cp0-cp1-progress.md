# CP0/CP1 implementation record — 2026-10-29

The [catch-up plan](implementation-catch-up.md) remains the ordered handoff.
This records the harness delta; it does not close CP2/R0 or any production gate.

## Changes applied

- Added the missing [R0 runbook](r0.md), named smoke profiles and retained
  [planning-input digests](planning-inputs.json). Entry documentation keeps real
  experimental commands separate from the planned whole-snapshot/fresh-target
  workflow. The backup-only supported 0.9 / full 1.0 sync distinction is preserved.
- Created unscored threat/profile/evidence templates in the planning registry.
  Unknown profile/media/clock fields stay explicit. No scored Windows/Ubuntu
  profile was manufactured from hosted CI or WSL labels.
- `backend-lock.json` v2 now binds versions, archives and extracted executables.
  Rust and installer consume that single source. Hash mismatch refuses execution,
  including version probing. Bounded staging prevents oversized supplied binaries.
- Bounded cache reads, downloads and ZIP/bzip2/TAR decompression; reject missing,
  duplicate or non-regular named members. Verify archive and executable before
  atomic install; injected failures preserve the previous valid binary.
- Replaced polling disk spools with bounded pipes. Windows jobs/Unix groups own
  child trees; cancellation, limits and early parent exit clean them up. A possible
  interrupted mutation remains unknown, including the deliberate startup drill.
- Added experimental Evidence v2 setup/phase/limit/fault outcomes and source,
  binary, pin and profile hashes. Fault diagnostics identify the opaque object,
  original digest and exact byte mutation. The exporter explicitly maps this to
  inventoried EvidenceManifest v1, preserving exact named-profile input bytes.
  Existing runs cannot be overwritten; missing/changed/extra members invalidate
  the retained inventory. Legacy Evidence v1 is not silently reinterpreted.
- Fixed the earlier fault harness: Kopia data packs live under sharded `p/`
  directories; restic's restored pack permissions must be made owner-writable
  again before each independent mutation. Kopia's secret-store disable flag is
  platform-specific (Windows credential manager / Linux keyring).
- CI now labels runs as smoke, observes image/kernel inputs, independently checks
  both full-size source revisions, runs installer/evidence/process negatives and
  documentation checks, and retains inventoried sanitized evidence. Packages
  include third-party license notices. Tag delivery stays a draft experimental
  prerelease, not a supported release.

## Verification observed

Windows checks: `cargo fmt`, Clippy with warnings denied, release build and seven
unit/process tests passed. Linux/ext4 under WSL: Clippy, release build and eight
tests passed (the extra test rejects Unix links). Each platform has two ignored
subprocess fixtures, invoked by the passing process-boundary tests. Six Python
installer/evidence negative tests passed on both platforms.

The independent Python source checker passed both full-size revisions: A has
64 files / 268435456 bytes; B has 63 / 264241152. Golden manifest bytes remain
unchanged: A SHA-256 `9731533c6970876599588e1439886226dcf9c1da4155558145987759a6162242`,
B `8ff269081ae3875bcca564143fd143fefc8102214df96ce68590d81ef4e6f5eb`.

Both candidates passed local Windows/NTFS and WSL Linux/ext4 smoke baselines:
capture/history, full reads, both exact restores, wrong secret, startup interruption,
fresh-client access, truncation and bit-flip detection, and exact restore after
repair. These retained runs identify their exact compiled source/binary inputs;
later diagnostic additions do not rewrite earlier evidence. The earlier failed
Linux Kopia version-probe run is retained alongside the corrected passing run.

The planning registry contains five inventoried runs (four passing baselines,
one retained failure), each verified by `scripts/evidence.py verify`:

| Profile | Backend / result | Run ID |
|---|---|---|
| windows11-26200-9168-smoke | Kopia / pass, R0 partial | `r0-1793305752773803200-39920` |
| windows11-26200-9168-smoke | restic / pass, R0 partial | `r0-1793305752781746100-7488` |
| wsl-ubuntu22-ext4-smoke | restic / pass, R0 partial | `r0-1793305780991225354-1333` |
| wsl-ubuntu22-ext4-smoke | Kopia / fail | `r0-1793305805749877993-1872` |
| wsl-ubuntu22-ext4-smoke | Kopia / pass, R0 partial | `r0-1793305910951227793-914` |

Local Markdown links/anchors passed. A packaged executable ran `--version` and
compared the full fixture correctly; package members and third-party notices were
present, and archive creation passed. Hosted Actions results must be read from
the actual run; local source/workflow presence is not CI success.

## Still open

CP0's **scored** profile qualification remains open: NTFS geometry/options query
was denied without elevation, further Windows settings/ACL/storage/clock details
are unqualified, and WSL Ubuntu 22.04 is not the target Ubuntu 24.04 image.
Historical startup kills do not establish every commit phase or power-loss safety.

CP2 still needs both candidates' independent-media copy/key/recovery topology,
production export/containment, observed-phase/index/access/disk-full/metadata
faults, storage/clock stress, license/acquisition matrix and a backend decision.
Production schemas/state/reconciliation, peer/retention/sync/desktop and supported
releases remain in their later capability blocks. No new production scaffold or
backend selection was added by these repairs.
