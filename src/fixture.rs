use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path},
};

pub const MIB: u64 = 1024 * 1024;
const MAX_MANIFEST: u64 = MIB;
const MAX_ENTRIES: usize = 1024;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub fixture_version: u32,
    pub directories: Vec<String>,
    pub files: Vec<Entry>,
}

fn name(index: usize) -> String {
    match index {
        58 => "unicode/žluťoučký-雪.txt".into(),
        59 => "readonly.bin".into(),
        60 => "unicode/composed-é.bin".into(),
        61 => "unicode/decomposed-e\u{301}.bin".into(),
        62 => "empty.txt".into(),
        63 => "nested/empty.bin".into(),
        _ => format!("nested/file-{index:02}.bin"),
    }
}

/// Fixture v1: xorshift64*, seed 0x504545525641554c XOR stream index.
/// All integer output is little endian. This is synthetic data, not cryptography.
fn block(state: &mut u64, output: &mut [u8]) {
    for chunk in output.as_chunks_mut::<8>().0 {
        *state ^= *state >> 12;
        *state ^= *state << 25;
        *state ^= *state >> 27;
        chunk.copy_from_slice(&state.wrapping_mul(0x2545f4914f6cdd1d).to_le_bytes());
    }
}

pub fn generate(root: &Path, small: bool) -> Result<Manifest> {
    fs::create_dir(root)?;
    for dir in ["nested", "unicode", "empty-directory"] {
        fs::create_dir(root.join(dir))?;
    }
    for index in 0..64 {
        let size = match index {
            62 | 63 => 0,
            60 | 61 => 8 * MIB,
            _ => 4 * MIB,
        } / if small { 1024 } else { 1 };
        let mut file = File::create(root.join(name(index)))?;
        let mut state = 0x504545525641554c ^ if index <= 2 { 0 } else { index as u64 };
        let mut buffer = [0; 4096];
        for _ in 0..size / 4096 {
            block(&mut state, &mut buffer);
            file.write_all(&buffer)?;
        }
    }
    let path = root.join("readonly.bin");
    let mut permissions = fs::metadata(&path)?.permissions();
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions)?;
    scan(root)
}

pub fn mutate(root: &Path) -> Result<Manifest> {
    // Insert 4 KiB at 4 KiB and drop the final 4 KiB, preserving the file length.
    let path = root.join(name(3));
    let bytes = fs::read(&path)?;
    ensure!(bytes.len() >= 8192, "mutation needs at least 8 KiB");
    let mut file = File::create(path)?;
    file.write_all(&bytes[..4096])?;
    file.write_all(&[0xa5; 4096])?;
    file.write_all(&bytes[4096..bytes.len() - 4096])?;
    fs::rename(root.join(name(4)), root.join("nested/renamed.bin"))?;
    fs::remove_file(root.join(name(5)))?;
    scan(root)
}

pub fn hash_file(path: &Path) -> Result<(u64, String)> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        size += count as u64;
    }
    Ok((size, format!("{:x}", hash.finalize())))
}

pub fn validate_path(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty() && path.len() <= 1024,
        "invalid path length"
    );
    // A portable relative path subset, independent of the host OS parser.
    for part in path.split('/') {
        ensure!(
            !part.is_empty() && part != "." && part != "..",
            "unsafe path component"
        );
        ensure!(!part.ends_with([' ', '.']), "ambiguous path component");
        ensure!(
            !part
                .chars()
                .any(|c| c.is_control() || "\\:<>\"|?*".contains(c)),
            "unsafe path characters"
        );
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        ensure!(
            !["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
                && !(stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && matches!(stem.as_bytes()[3], b'1'..=b'9')),
            "reserved path component"
        );
    }
    ensure!(
        Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_))),
        "path must be relative"
    );
    ensure!(path.split('/').count() <= 16, "path depth exceeded");
    Ok(())
}

pub fn scan(root: &Path) -> Result<Manifest> {
    let mut manifest = Manifest {
        schema_version: 1,
        fixture_version: 1,
        directories: vec![],
        files: vec![],
    };
    visit(root, root, &mut manifest)?;
    manifest.directories.sort();
    manifest.files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(manifest)
}

fn visit(root: &Path, path: &Path, manifest: &mut Manifest) -> Result<()> {
    let metadata = fs::symlink_metadata(path).context("cannot enumerate selected tree")?;
    ensure!(!metadata.file_type().is_symlink(), "links are unsupported");
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "reparse points are unsupported"
        );
    }
    if path != root {
        let relative = path
            .strip_prefix(root)?
            .components()
            .map(|component| component.as_os_str().to_str().context("non-UTF-8 path"))
            .collect::<Result<Vec<_>>>()?
            .join("/");
        validate_path(&relative)?;
        ensure!(
            manifest.files.len() + manifest.directories.len() < MAX_ENTRIES,
            "entry limit exceeded"
        );
        if metadata.is_dir() {
            manifest.directories.push(relative);
        } else if metadata.is_file() {
            ensure!(metadata.len() <= 16 * MIB, "R0 file size limit exceeded");
            let (size, sha256) = hash_file(path)?;
            manifest.files.push(Entry {
                path: relative,
                size,
                sha256,
            });
        } else {
            bail!("special files are unsupported");
        }
    } else {
        ensure!(metadata.is_dir(), "tree root must be a directory");
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            visit(root, &entry?.path(), manifest)?;
        }
    }
    Ok(())
}

pub fn write_manifest(path: &Path, manifest: &Manifest) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer_pretty(&mut file, manifest)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

pub fn read_manifest(path: &Path) -> Result<Manifest> {
    let mut bytes = Vec::new();
    File::open(path)
        .context("cannot read expected manifest")?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= MAX_MANIFEST, "manifest too large");
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &Manifest) -> Result<()> {
    ensure!(
        manifest.schema_version == 1 && manifest.fixture_version == 1,
        "unsupported manifest version"
    );
    ensure!(
        manifest.files.len() + manifest.directories.len() <= MAX_ENTRIES,
        "entry limit exceeded"
    );
    let mut paths = BTreeSet::new();
    for path in manifest
        .directories
        .iter()
        .chain(manifest.files.iter().map(|e| &e.path))
    {
        validate_path(path)?;
        ensure!(
            paths.insert(path.to_lowercase()),
            "duplicate/colliding path"
        );
    }
    for entry in &manifest.files {
        ensure!(
            entry.size <= 16 * MIB
                && entry.sha256.len() == 64
                && entry
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid file entry"
        );
    }
    Ok(())
}

pub fn compare(root: &Path, expected: &Manifest) -> Result<()> {
    validate_manifest(expected)?;
    ensure!(
        &scan(root)? == expected,
        "restore mismatch: file/directory set, size or SHA-256 differs"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_oracle_detects_damage_and_extra_entries() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("source");
        let first = generate(&root, true)?;
        assert_eq!(first.files.len(), 64);
        assert_eq!(first.files.iter().map(|e| e.size).sum::<u64>(), 256 * 1024);
        compare(&root, &first)?;
        assert_eq!(
            first
                .files
                .iter()
                .find(|e| e.path == name(0))
                .unwrap()
                .sha256,
            first
                .files
                .iter()
                .find(|e| e.path == name(2))
                .unwrap()
                .sha256
        );
        fs::write(root.join("extra"), b"unexpected")?;
        assert!(compare(&root, &first).is_err());
        fs::remove_file(root.join("extra"))?;
        let path = root.join(name(3));
        let mut bytes = fs::read(&path)?;
        bytes[0] ^= 1;
        fs::write(&path, bytes)?;
        assert!(compare(&root, &first).is_err());
        Ok(())
    }

    #[test]
    fn manifests_reject_unsafe_or_ambiguous_paths() {
        for path in [
            "../escape",
            "/absolute",
            "C:/drive",
            "a\\b",
            "a//b",
            "a/./b",
            "NUL.txt",
            "COM1",
            "bad.",
            "x:stream",
            "a\u{0}b",
        ] {
            assert!(validate_path(path).is_err(), "accepted {path:?}");
        }
        assert!(validate_path("unicode/žluťoučký-雪.txt").is_ok());
    }

    #[test]
    fn malformed_versioned_and_colliding_manifests_are_rejected() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("manifest.json");
        for bytes in [
            b"not-json".as_slice(),
            b"{}",
            br#"{"schema_version":2,"fixture_version":1,"directories":[],"files":[]}"#,
        ] {
            fs::write(&path, bytes)?;
            assert!(read_manifest(&path).is_err());
        }
        fs::write(&path, vec![b' '; MAX_MANIFEST as usize + 1])?;
        assert!(read_manifest(&path).is_err());
        let mut manifest: Manifest =
            serde_json::from_str(include_str!("../tests/fixtures/v1-a.json"))?;
        validate_manifest(&manifest)?;
        manifest.directories.push("NESTED".into());
        assert!(validate_manifest(&manifest).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn links_and_special_files_fail_closed() -> Result<()> {
        let temp = tempfile::tempdir()?;
        std::os::unix::fs::symlink("/tmp", temp.path().join("escape"))?;
        assert!(scan(temp.path()).is_err());
        Ok(())
    }
}
