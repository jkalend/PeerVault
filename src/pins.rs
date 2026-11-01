//! PeerVault-owned evidence inputs for the bailiff harness.
//!
//! The pin document and this product's build inputs stay in this repository; the
//! harness only defines the schema and records the hashes.

use bailiff::backend::EvidenceInputs;
use std::collections::BTreeMap;

/// Sole version/archive/executable pin source shipped with this product.
pub const BYTES: &[u8] = include_bytes!("../backend-lock.json");

pub fn load() -> anyhow::Result<bailiff::pins::Pins> {
    bailiff::pins::load_from(BYTES)
}

pub fn digest() -> String {
    bailiff::pins::digest(BYTES)
}

pub fn source_inputs() -> BTreeMap<&'static str, String> {
    [
        ("Cargo.toml", include_bytes!("../Cargo.toml").as_slice()),
        ("Cargo.lock", include_bytes!("../Cargo.lock").as_slice()),
        (
            "rust-toolchain.toml",
            include_bytes!("../rust-toolchain.toml").as_slice(),
        ),
        ("backend-lock.json", BYTES),
        ("src/main.rs", include_bytes!("main.rs").as_slice()),
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
    .map(|(path, bytes)| (path, bailiff::pins::hash_bytes(bytes)))
    .collect()
}

pub fn inputs() -> anyhow::Result<EvidenceInputs> {
    Ok(EvidenceInputs {
        pins: load()?,
        backend_lock_sha256: digest(),
        source_inputs: source_inputs(),
    })
}
