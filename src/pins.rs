use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub url: String,
    pub sha256: String,
    pub executable_sha256: String,
    pub member: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendPin {
    pub version: String,
    pub license: String,
    pub source: String,
    pub windows: Asset,
    pub linux: Asset,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pins {
    pub schema_version: u32,
    pub restic: BackendPin,
    pub kopia: BackendPin,
}

pub const BYTES: &[u8] = include_bytes!("../backend-lock.json");

pub fn load() -> Result<Pins> {
    let pins: Pins = serde_json::from_slice(BYTES).context("invalid compiled backend pins")?;
    ensure!(pins.schema_version == 2, "unsupported backend pin schema");
    for backend in [&pins.restic, &pins.kopia] {
        ensure!(
            !backend.version.is_empty()
                && !backend.license.is_empty()
                && backend.source.starts_with("https://github.com/"),
            "invalid backend metadata"
        );
        for asset in [&backend.windows, &backend.linux] {
            ensure!(
                asset.url.starts_with("https://github.com/")
                    && asset.url.contains(&format!("/v{}/", backend.version)),
                "backend pin version/URL drift"
            );
            for hash in [&asset.sha256, &asset.executable_sha256] {
                ensure!(
                    hash.len() == 64
                        && hash
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                    "invalid backend digest pin"
                );
            }
            if let Some(member) = &asset.member {
                ensure!(
                    !member.is_empty() && !member.starts_with('/') && !member.contains(".."),
                    "unsafe archive member pin"
                );
            }
        }
    }
    Ok(pins)
}

pub fn digest() -> String {
    format!("{:x}", Sha256::digest(BYTES))
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn source_inputs() -> std::collections::BTreeMap<&'static str, String> {
    [
        ("Cargo.toml", include_bytes!("../Cargo.toml").as_slice()),
        ("Cargo.lock", include_bytes!("../Cargo.lock").as_slice()),
        (
            "rust-toolchain.toml",
            include_bytes!("../rust-toolchain.toml").as_slice(),
        ),
        ("backend-lock.json", BYTES),
        ("src/main.rs", include_bytes!("main.rs").as_slice()),
        ("src/fixture.rs", include_bytes!("fixture.rs").as_slice()),
        ("src/media.rs", include_bytes!("media.rs").as_slice()),
        ("src/backend.rs", include_bytes!("backend.rs").as_slice()),
        ("src/process.rs", include_bytes!("process.rs").as_slice()),
        ("src/pins.rs", include_bytes!("pins.rs").as_slice()),
        (
            "tests/fixtures/v1-a.json",
            include_bytes!("../tests/fixtures/v1-a.json").as_slice(),
        ),
        (
            "tests/fixtures/v1-b.json",
            include_bytes!("../tests/fixtures/v1-b.json").as_slice(),
        ),
    ]
    .into_iter()
    .map(|(path, bytes)| (path, hash_bytes(bytes)))
    .collect()
}
