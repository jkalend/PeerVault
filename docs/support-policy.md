# Support policy and scope (draft for backup-only 0.9)

Status: draft. **No supported release exists.** These are the frozen-scope
candidates for a supported backup-only 0.9; they are not a claim that the current
experimental build meets them.

## Supported profile (candidate)

| Dimension | Candidate supported value |
|---|---|
| PeerVault | 0.0.x frozen artifact set selected at RC |
| OS | Windows 11 x64 NTFS; Ubuntu 24.04 LTS x64 ext4 |
| Backend | restic 0.19.1, repository format v2, pinned executable SHA-256 |
| Topology | one owner; local repository; additive copy to a separately named local filesystem or removable medium |
| Source content | regular files and directories, portable relative UTF-8 paths, read-only bit |
| Restore | whole snapshot into a new empty destination, exact path/type/size/SHA-256 |
| Scheduling | one-shot `schedule-tick` under the owner's OS scheduler |
| Desktop | Windows C# shell over owner-only IPC; Linux remains CLI |
| Sync | **excluded** from 0.9; no live-folder conflict/tombstone behavior |

Windows Server CI labels, WSL2 and VMs are smoke environments, not supported
clean-host profiles. The Linux 0.9 profile requires a clean Ubuntu 24.04 install;
WSL2 Ubuntu 22.04 evidence is diagnostic only.

## Explicitly unsupported

- V0.7 live-folder sync of any kind, tombstones, public discovery, relays and
  multi-user collaboration.
- VSS, network shares, cloud-synced roots, offline placeholders and automatic
  hydration.
- Symlinks, Windows reparse points, special files, NTFS streams/ACLs, POSIX
  owners/xattrs/modes beyond the read-only bit, timestamps, hard-link identity
  and sparse allocation.
- Selective restore, merge, overwrite, resume, block-device replication and
  arbitrary backend/archive extraction.
- Backends other than pinned restic 0.19.1; multiple concurrent repository
  writers; repositories inside a captured source.
- Claims against a compromised endpoint, administrator, publisher or ransomware
  using the owner's deletion credentials.

## Recovery guarantee

A restore report covers the exact tested snapshot, backend binary, policy,
profile and drill. It is not a permanent recoverability guarantee. A local
snapshot is not an independent backup; supported independent-copy claims require
a distinct-media source-disk-absent drill plus separately held keys.

## Security and support reporting

Use the repository's private security advisory channel. The owner must publish a
monitored contact and response target before RC; an unpublished channel is a
release blocker. Do not include repository secrets, keys or plaintext file
content in reports.

## Updates

- One backend version and one PeerVault format generation are supported at a
  time.
- A PeerVault update must read snapshots created by the previous supported
  version; rollback may require the pre-upgrade recovery copy.
- A backend migration is gated by a verified recovery copy and an old-snapshot
  restore drill.
- Withdrawing a supported version requires a security notice, migration advice
  and retained recovery instructions.

## Data handling

No telemetry, no listening TCP control port and no required SaaS. Logs and
reports are local diagnostic artifacts; secrets stay in the owner's chosen secret
store/file and are never copied into retention/schedule state.

## Known release blockers

See [release-checklist.md](release-checklist.md) and
[license-review.md](license-review.md). The current critical blockers include the
unresolved bailiff redistribution license, independent security review, clean
Ubuntu 24.04 evidence, power-loss/upgrade/disk-full qualification and the 14-day
beta with a second operator.
