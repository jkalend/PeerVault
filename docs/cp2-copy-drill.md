# CP2 copy/recovery topology drill — 2026-10-31

This records the CP2 copy/recovery slice against the R0 harness. The backend
selection is recorded separately in [ADR 0001](adr/0001-r0-backend-selection.md);
the open R0 residuals are listed there and in the
[catch-up plan](implementation-catch-up.md). [R0 runbook](r0.md) holds commands
and limits.

## Implemented

- `copy-drill <restic|kopia>` on new, disjoint work/copy/recovery roots.
- Pre-launch media observation through Windows PowerShell `Get-Volume`/`Get-Disk`
  (Linux: `findmnt` + sysfs). Known same volume/device refuses the run; unknown
  identity needs `--acknowledge-unknown-media` and is retained as unknown.
- Native copy path: restic `copy --from-repo` after destination `init`; Kopia
  `repository sync-to filesystem --path` with default no-delete.
- Snapshot mapping: restic copy IDs are new, with the source ID in `original`;
  Kopia preserves IDs. The harness requires a one-to-one mapping and rejects
  missing, extra or ambiguous copies.
- Simulated source absence: source tree, source repository, writer config/cache
  and writer environment directories are removed after the copy returns.
- Fresh receiver session: wrong-secret refusal, fresh-config connect, history,
  backend-native full read at its actual scope, and exact restore of both fixture
  revisions against the independent oracle.
- Byte-for-byte repository inventory before/after receiver access. Missing,
  changed or added non-ephemeral objects fail the run; only restic `locks/` and
  Kopia maintenance/format files are classified ephemeral.
- Evidence v3 topology block: hashed volume/device identities, filesystem/bus,
  mapping, copy/no-delete semantics, source-absence description and inventory
  summaries. `scripts/evidence.py` accepts v2 and v3 and inventories the block.

## Observed on this workstation

Profile `windows11-26200-9168-copy-smoke` revision 1; three new roots on three
observed physical devices (source NVMe, repository SATA HDD, recovery SATA SSD).
Windows 11 `10.0.26200.9168`. The same host/controller failure domain remains.

| Backend | Result | Run ID | Steps | Repository bytes | Snapshot mapping | Restored files |
|---|---|---|---|---|---|---|
| restic 0.19.1 | pass / partial | `r0-1793466636066477400-42872` | 14, 22.2 s | 261,890,649 | 3 new IDs with `original` | 64 + 63 |
| Kopia 0.23.1 | pass / partial | `r0-1793466672418718700-9616` | 14, 16.6 s | 264,322,688 | 3 identical IDs | 64 + 63 |

Both runs: receiver full read passed, both exact restores matched every path,
type, size and SHA-256, outside canary unchanged, and the post-receiver inventory
digest equalled the pre-receiver digest with zero missing/changed/added objects.

Retained under `project-planning/evidence/peervault/R0/windows11-26200-9168-copy-smoke/`:

| Run | Manifest SHA-256 |
|---|---|
| restic pass `r0-1793466636066477400-42872` | `10f07cfd2d053b13014026f19f15eaaa00bba680e598b97a69a8395559be0969` |
| Kopia pass `r0-1793466672418718700-9616` | `1d9a7c7053d58ddc8fa8a7465d8cf823ebcce573e25385396388b8ea58f1c808` |
| same-device refusal `r0-1793466698686352600-54264` | `b8315aa1c49bab5a3a11ef2b6242dcbc334c1f09102dc633be09a8c46b00160e` |

The negative run placed work and copy roots on one known device and failed at
`media-identity` before any backend command; no topology block was produced.
Its retained profile describes the intended distinct-media setup, so the failed
evidence must be read as a guard result, not as a completed profile run.

## Findings

- The two candidates have different copy semantics. Restic re-encrypts into the
  destination and changes snapshot IDs, retaining `original`; Kopia copies blobs
  and preserves IDs. Both mappings are exact and machine-checkable.
- Restic needed an explicit destination repository `init` with the same
  disposable password; Kopia needed the destination directory to exist before
  `sync-to`. Both copy paths were additive: no delete command was issued.
- Receiver list/verify/restore left every non-ephemeral repository byte unchanged.
  No ephemeral lock or maintenance change was observed in these runs.
- Source removal is local simulation. It does not prove physical disk removal,
  clean-device key custody, network transport, keyless peer behavior or
  ransomware resistance. The disposable password was held in harness memory and
  used for both repositories; separate secret custody and rotation remain CP4.

## Scored R0 runs — 2026-11-01

The same harness was rerun with the scored profiles and retained in
`project-planning/evidence/peervault/R0/`. All four runs restored both fixture
revisions exactly and left the outside canary unchanged; the two Windows
`copy-drill` runs also kept the receiver inventory unchanged.

| Run | Backend | Profile | Command | Result | Manifest SHA-256 |
|---|---|---|---|---|---|
| `r0-1793503813846092900-11980` | restic | Windows scored | copy-drill | pass / partial | `c42ed38783c57d02f87f9aa9295a37927e1c76b9d406e98bd070b27349b7c1e3` |
| `r0-1793503839028387000-61928` | Kopia | Windows scored | copy-drill | pass / partial | `ea81832477e819a43a846d9163b0e1371013794a1c72b1209e717a3c26f82e9d` |
| `r0-1793503456273812386-1480` | restic | Ubuntu scored | bakeoff | pass / partial | `516c3597139cb6626fb0c7a1e2f694fc1a901d7564aba680c2b764b6ad29052e` |
| `r0-1793503516597910572-1935` | Kopia | Ubuntu scored | bakeoff | pass / partial | `9a4a61c76bfb249ea2ac815455d80abca84b303e041af5fc48c77380e38e145b` |

Windows runs used three observed distinct physical devices. Ubuntu runs used the
hash-pinned WSL2 ext4 image and virtual same-host storage, so they do not qualify
independent media. Both candidates remain partial: containment, phase faults,
index/access/disk-full and clean-device recovery are open.

## Still open in CP2

- Linux copy/recovery drill on independent physical media; the scored Ubuntu
  profile uses a pinned but virtual same-host ext4 disk.
- Supported network/keyless/clean-device topology, independent recovery secret
  and source-disk-absent drill on separate hardware.
- Production export/streaming and contained restore, including hostile entries,
  traversal, link/reparse and target-parent races.
- Observed-phase crash faults, index loss/rebuild, denied access, disk full,
  metadata/path limits, clock/storage stress and VM power loss.
- Full license/redistribution/acquisition matrix for packaging; the
  backend/profile selection is recorded in the ADR.
