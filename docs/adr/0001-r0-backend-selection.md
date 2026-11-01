# ADR 0001 — R0 backend and profile selection

- Status: accepted for V0.1 adapter implementation; R0 qualification residuals
  remain open and must not be read as a production release gate pass.
- Date: 2026-11-01
- Decision owner: PeerVault implementation maintainer

## Decision

Use **restic 0.19.1** with repository format v2 as the selected backup backend for
the first production adapter. Consume an explicitly configured, checksum-verified
external binary; never fall back to `PATH`. Keep Kopia 0.23.1 as the experimental
comparison candidate until the open clock/storage and containment cases are
qualified.

Scored profiles:

- `docs/profiles/windows11-26200-9168-r0.json` — Windows 11 26200, NTFS, direct
  NVMe source, SATA HDD copy, SATA SSD receiver.
- `docs/profiles/ubuntu2404-24.04.5-wsl-r0.json` — Ubuntu 24.04.5 LTS, ext4,
  hash-pinned WSL2 image, virtual same-host storage.

## Evidence

| Run | Backend | Profile | Result | Verification scope | Manifest SHA-256 |
|---|---|---|---|---|---|
| `r0-1793503318320016900-45644` | restic | Windows scored | pass / partial | repository | `00850aebe9e0f192822a0a4c359640c6641b014f9c727cb8f8baf778aca5a223` |
| `r0-1793503353800889600-42804` | Kopia | Windows scored | pass / partial | snapshot | `7f000610f0063a44ab990fdb91f23b6a64363831ebc527efc926f5584e4ca36c` |
| `r0-1793503456273812386-1480` | restic | Ubuntu scored | pass / partial | repository | `516c3597139cb6626fb0c7a1e2f694fc1a901d7564aba680c2b764b6ad29052e` |
| `r0-1793503516597910572-1935` | Kopia | Ubuntu scored | pass / partial | snapshot | `9a4a61c76bfb249ea2ac815455d80abca84b303e041af5fc48c77380e38e145b` |

All Windows runs used `copy-drill` with source, copy and receiver roots on three
observed distinct physical devices. All four runs restored both fixture revisions
exactly, left the outside canary unchanged, and the receiver inventory showed zero
missing/changed/added non-ephemeral objects. Linux runs used `bakeoff` on the
pinned ext4 image; their storage is virtual and same-host.

## Scoring

| Capability | restic 0.19.1 | Kopia 0.23.1 |
|---|---|---|
| License | BSD-2-Clause | Apache-2.0 |
| Full-read verification | repository (`check --read-data`) | snapshot (`snapshot verify --verify-files-percent=100`) |
| Copy path | `copy --from-repo` into an initialized destination | `repository sync-to filesystem` with default no-delete |
| Copy identity | new snapshot IDs with source ID in `original` | IDs preserved |
| Key path | password env for destination; source password file for copy | shared password env; repository key |
| Clock prerequisite | none observed | isolated clock/storage stress still open |
| Scored Windows copy drill | pass | pass |
| Scored Linux baseline | pass | pass |
| Windows/Linux restoration oracle | exact on both | exact on both |

## Rationale

restic wins on the two claims the first slice needs most:

1. **Actual verification scope is the whole repository.** `check --read-data`
   exercises the repository, while Kopia's supported command verifies an
   attempted snapshot scope. A stronger, simpler full-read claim reduces the
   chance of advertising backend output as more than it is.
2. **No clock dependency was observed.** The host clock is not synchronized
   (HTTPS probe about −6.7 s), and Kopia's isolated clock/storage protocol is not
   yet qualified. Selecting restic avoids making a clock prerequisite part of the
   first recovery path.

The additive copy path, explicit `original` snapshot mapping, BSD-2-Clause
license, and smaller adapter surface are secondary advantages. Repository growth
and elapsed time were recorded but no dedup-ratio threshold is claimed.

## Consequences

- V0.1 implements the selected adapter against restic's repository format v2.
- Production code must keep the backend pin and executable hash check; a version
  or hash mismatch refuses capture/read/restore mutation.
- The copy path re-encrypts into the destination repository. It is not a raw
  repository file copy and must retain the source ID mapping.
- Recovery material is a password/key custody problem; it is not stored in the
  repository or logs.
- R0 reports and evidence remain `gate_status=partial`; this ADR records a
  reasoned selection, not a completed production qualification.

## Open R0 residuals

- Hostile restore containment, link/reparse and target-parent races.
- Observable phase-targeted crash and power-loss drills; startup kill alone is
  not sufficient.
- Index loss/rebuild, denied access and disposable disk-full cases.
- Linux copy/recovery drill on independent physical media (the scored Ubuntu
  profile uses virtual same-host storage).
- Scored-profile corruption/fault runs; current corruption evidence is smoke.
- Kopia clock/storage stress, retained for comparison and possible reconsideration.
- Clean-device/key recovery belongs to V0.2 and is not established here.
