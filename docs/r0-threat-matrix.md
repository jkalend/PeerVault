# R0 threat and survival matrix

Status vocabulary: `survived`, `degraded`, `not-survived`, `not-claimed`.
R0 records the tested topology and leaves later-gate rows explicitly open; a
missing row is not evidence of survival. Evidence refs point at retained R0 runs
or state `none`.

Selected topology for R0 scoring: source on C: NVMe, copied repository on G: SATA
HDD, fresh receiver on F: SATA SSD, one Windows 11 host. This is a same-host
media-separation drill, not a clean-device or offsite topology.

| # | Actor/failure | Affected material | Required surviving material | Result | Evidence | Residual limit | Owner |
|---|---|---|---|---|---|---|---|
| 1 | Stolen source device | Source tree, local repository, writer config/cache | Independent repository copy plus separately held recovery secret | not-claimed | none | No stolen-device or separate-secret drill | CP4 |
| 2 | Stolen keyless peer | Peer copy without key material | Source copy remains readable; ciphertext-only peer cannot restore | not-claimed | none | No peer/keyless topology | CP5 |
| 3 | Stolen key-holding peer | Peer copy and keys | Independent source copy plus separate recovery secret | not-claimed | none | No peer/key-holding drill | CP5 |
| 4 | Source disk loss | Source tree and source repository | Copied repository on independent media | degraded | `docs/cp2-copy-drill.md` runs | Same-host removal simulation, not physical disk removal | CP2 |
| 5 | Peer disk loss | Peer repository copy | Source repository or another verified copy | not-claimed | none | No peer topology | CP5 |
| 6 | Deleted/missing/corrupted repository | Referenced data/manifest objects | Prior committed snapshot or independent copy | degraded | R0 truncation/bit-flip drill | Owned disposable copy only; no arbitrary archive | CP2 |
| 7 | Lost/wrong recovery secret | Repository key or recovery password | Separately held correct secret | degraded | R0 wrong-password refusal | No separate custody or rotation | CP4 |
| 8 | Malicious archive | Crafted paths/links in a supplied repository | Contained restore, outside canaries unchanged | not-claimed | none | Native restore accepts self-created fixtures only | CP2/CP3 |
| 9 | Malicious storage withholding/replay/rollback | Backend objects or listing | Detected inconsistency and retained prior snapshot | not-claimed | none | No hostile storage emulator | CP2/CP6 |
| 10 | Ransomware/mass rewrite/delete | Reachable repositories and credentials | Offline/independent copy beyond attacker reach | not-claimed | none | No endpoint-compromise guarantee | CP4/CP6 |
| 11 | Compromised authorized endpoint | Keys, config, reachable repositories | Separately held recovery material and independent copy | not-claimed | none | No endpoint-compromise guarantee | CP4 |
| 12 | Stolen transport credentials | Peer/network credentials | Local repository and independent copy | not-claimed | none | No network transport | CP5 |
| 13 | Compromised publisher/backend binary | Backend executable or acquisition path | Pin/checksum refusal before mutation | degraded | R0 binary-pin rejection | Pins and hashes identify bytes, not publisher authority | CP1/CP2 |
| 14 | Root/administrator compromise | Host, keys, repositories | Material outside the compromised host | not-claimed | none | No administrative-compromise guarantee | CP4 |
| 15 | Both copy and recovery-secret loss | Repository copy and secret | None | not-claimed | none | Explicitly unrecoverable; no claim | CP4 |
| 16 | Clock/list-consistency failures | Snapshot ordering, freshness, deletion state | Conservative unknown state; no deletion/freshness claim | not-claimed | Clock probe only | No isolated clock/storage stress | CP2/CP5 |

## Fault coverage planned by R0

| Case | Harness status | Notes |
|---|---|---|
| Baseline A/unchanged/B, full read, exact restore | implemented | `bakeoff` and `copy-drill` |
| Corruption (truncate/bit-flip) with repair | implemented | owned disposable data object |
| Wrong and missing credentials | implemented | fresh config/cache |
| Observable-phase crash | partial | startup interruption only; phase-targeted faults open |
| Index loss/rebuild | open | no rebuild drill |
| Denied access and disk full | open | no disposable disk/VHD fault |
| Hostile restore containment | open | self-created fixtures only |
| Power loss | open | process kill is not power-loss evidence |
| Clock/storage stress | open | Kopia-specific protocol not run |
