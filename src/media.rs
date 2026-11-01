//! Observed storage identity for the R0 copy/recovery topology drill.
//!
//! The drill refuses to claim independent media when the source, repository copy
//! and recovery roots resolve to the same known physical device. Unknown identity
//! requires an explicit acknowledgement and is retained as unknown coverage.

use anyhow::{Context, Result, ensure};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{path::Path, process::Command};

#[cfg(windows)]
use crate::process::bounded;
#[cfg(windows)]
use std::path::PathBuf;
#[cfg(windows)]
use std::time::Duration;

#[cfg(windows)]
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);
#[cfg(windows)]
const PROBE_LIMIT: u64 = 256 * 1024;

#[derive(Serialize)]
pub struct ObservedMedia {
    pub role: &'static str,
    pub known: bool,
    pub volume_id_sha256: Option<String>,
    pub device_id_sha256: Option<String>,
    pub filesystem: Option<String>,
    pub bus: Option<String>,
    pub probe: &'static str,
}

#[derive(Serialize)]
pub struct MediaSet {
    pub source: ObservedMedia,
    pub repository: ObservedMedia,
    pub recovery: ObservedMedia,
    pub distinct_devices: Option<bool>,
    pub distinct_volumes: Option<bool>,
    pub coverage: &'static str,
}

#[derive(Default)]
struct RawObservation {
    volume_id: Option<String>,
    device_id: Option<String>,
    filesystem: Option<String>,
    bus: Option<String>,
    probe: &'static str,
}

/// Observe the three drill roots and enforce distinct known physical devices.
pub fn guard(
    source: &Path,
    repository: &Path,
    recovery: &Path,
    acknowledge_unknown: bool,
) -> Result<MediaSet> {
    let mut observations = observe(&[
        ("source", source),
        ("repository", repository),
        ("recovery", recovery),
    ])?;
    let recovery_media = observations
        .pop()
        .context("media observation count mismatch")?;
    let repository_media = observations
        .pop()
        .context("media observation count mismatch")?;
    let source_media = observations
        .pop()
        .context("media observation count mismatch")?;

    for (left, right) in [
        (&source_media, &repository_media),
        (&source_media, &recovery_media),
        (&repository_media, &recovery_media),
    ] {
        ensure!(
            left.device_id_sha256.is_none()
                || right.device_id_sha256.is_none()
                || left.device_id_sha256 != right.device_id_sha256,
            "{} and {} roots resolve to the same known physical device; distinct media are required",
            left.role,
            right.role
        );
        ensure!(
            left.volume_id_sha256.is_none()
                || right.volume_id_sha256.is_none()
                || left.volume_id_sha256 != right.volume_id_sha256,
            "{} and {} roots resolve to the same known volume; distinct media are required",
            left.role,
            right.role
        );
    }

    let all_known = observations
        .iter()
        .all(|media| media.volume_id_sha256.is_some() && media.device_id_sha256.is_some());
    if !all_known {
        ensure!(
            acknowledge_unknown,
            "physical media identity is unavailable for at least one drill root; use distinct local drives or pass --acknowledge-unknown-media to retain unknown coverage"
        );
    }
    let (distinct_devices, distinct_volumes, coverage) = if all_known {
        (Some(true), Some(true), "distinct-observed-devices")
    } else {
        (None, None, "unknown-media-acknowledged")
    };

    Ok(MediaSet {
        source: source_media,
        repository: repository_media,
        recovery: recovery_media,
        distinct_devices,
        distinct_volumes,
        coverage,
    })
}

fn observe(roots: &[(&'static str, &Path)]) -> Result<Vec<ObservedMedia>> {
    #[cfg(windows)]
    let raw = observe_windows(roots)?;
    #[cfg(unix)]
    let raw = roots
        .iter()
        .map(|(_, path)| observe_unix(path))
        .collect::<Vec<_>>();
    #[cfg(not(any(windows, unix)))]
    let raw: Vec<RawObservation> = roots.iter().map(|_| RawObservation::default()).collect();

    Ok(roots
        .iter()
        .zip(raw)
        .map(|((role, _), media)| ObservedMedia {
            role,
            known: media.volume_id.is_some() && media.device_id.is_some(),
            volume_id_sha256: media.volume_id.as_deref().map(digest),
            device_id_sha256: media.device_id.as_deref().map(digest),
            filesystem: media.filesystem,
            bus: media.bus,
            probe: media.probe,
        })
        .collect())
}

fn digest(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(windows)]
fn observe_windows(roots: &[(&'static str, &Path)]) -> Result<Vec<RawObservation>> {
    let paths: Vec<PathBuf> = roots
        .iter()
        .map(|(_, path)| {
            let text = path.to_string_lossy();
            PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text))
        })
        .collect();
    let mut script = String::from("$ErrorActionPreference='Stop'\n$paths=@(");
    for (index, path) in paths.iter().enumerate() {
        if index > 0 {
            script.push(',');
        }
        script.push('\'');
        script.push_str(&path.to_string_lossy().replace('\'', "''"));
        script.push('\'');
    }
    script.push_str(
        ")\n$items = foreach ($p in $paths) {\n\
         try {\n\
         $full=[System.IO.Path]::GetFullPath($p)\n\
         $root=[System.IO.Path]::GetPathRoot($full)\n\
         if ($root.Length -lt 2) { throw 'no-drive' }\n\
         $letter=$root.Substring(0,1)\n\
         $v=Get-Volume -DriveLetter $letter -ErrorAction Stop\n\
         $d=Get-Partition -DriveLetter $letter -ErrorAction Stop | Get-Disk -ErrorAction Stop\n\
         [pscustomobject]@{Volume=[string]$v.UniqueId; Fs=[string]$v.FileSystemType; Disk=[string]$d.UniqueId; Bus=[string]$d.BusType; Error=$null}\n\
         } catch { [pscustomobject]@{Volume=$null; Fs=$null; Disk=$null; Bus=$null; Error='probe-failed'} }\n\
         }\nConvertTo-Json -InputObject @($items) -Compress\n",
    );
    let mut command = Command::new("powershell.exe");
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        &script,
    ]);
    let (code, stdout) = bounded(&mut command, PROBE_TIMEOUT, PROBE_LIMIT)
        .context("media identity probe failed to run")?;
    ensure!(
        code == Some(0),
        "media identity probe exited without a usable result"
    );
    let value: serde_json::Value =
        serde_json::from_str(&stdout).context("invalid media identity JSON")?;
    let items = value
        .as_array()
        .cloned()
        .unwrap_or_else(|| vec![value.clone()]);
    ensure!(
        items.len() == roots.len(),
        "media identity probe returned an unexpected item count"
    );
    Ok(items
        .into_iter()
        .map(|item| {
            if !item["Error"].is_null() {
                return RawObservation {
                    probe: "windows-powershell",
                    ..RawObservation::default()
                };
            }
            let text = |name: &str| {
                item[name]
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
            };
            RawObservation {
                volume_id: text("Volume"),
                device_id: text("Disk"),
                filesystem: text("Fs"),
                bus: text("Bus"),
                probe: "windows-powershell",
            }
        })
        .collect())
}

#[cfg(unix)]
fn observe_unix(path: &Path) -> RawObservation {
    use std::fs;
    let output = Command::new("findmnt")
        .args(["-n", "-o", "SOURCE,FSTYPE", "-T"])
        .arg(path)
        .output();
    let Ok(output) = output else {
        return RawObservation {
            probe: "unix-sysfs",
            ..RawObservation::default()
        };
    };
    if !output.status.success() {
        return RawObservation {
            probe: "unix-sysfs",
            ..RawObservation::default()
        };
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut fields = text.split_whitespace();
    let Some(source) = fields.next() else {
        return RawObservation {
            probe: "unix-sysfs",
            ..RawObservation::default()
        };
    };
    let filesystem = fields.next().map(str::to_owned);
    let Some(name) = Path::new(source).file_name() else {
        return RawObservation {
            filesystem,
            probe: "unix-sysfs",
            ..RawObservation::default()
        };
    };
    let sys_path = Path::new("/sys/class/block").join(name);
    let resolved = fs::canonicalize(&sys_path).unwrap_or(sys_path);
    let serial = ["device/serial", "device/wwid"].iter().find_map(|suffix| {
        fs::read_to_string(resolved.join(suffix))
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    });
    RawObservation {
        volume_id: Some(source.to_owned()),
        device_id: Some(serial.unwrap_or_else(|| resolved.to_string_lossy().into_owned())),
        filesystem,
        bus: None,
        probe: "unix-sysfs",
    }
}
