mod backend;
mod fixture;
mod pins;
mod process;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(
    version,
    about = "Experimental PeerVault R0 qualification tools. Synthetic data only."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create the deterministic R0 corpus and independent expected manifest.
    Fixture {
        /// New directory; refuses existing paths.
        directory: PathBuf,
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=2))]
        revision: u8,
    },
    /// Compare exact paths, sizes and SHA-256 against a fixture manifest.
    Compare {
        directory: PathBuf,
        manifest: PathBuf,
    },
    /// Run one pinned backend against disposable synthetic data (not a backup command).
    Bakeoff {
        #[arg(value_enum)]
        backend: backend::Kind,
        #[arg(long)]
        executable: PathBuf,
        /// New directory on the filesystem being qualified. Never reuse a real repository.
        #[arg(long)]
        work_dir: PathBuf,
        /// Named smoke profile. Scored R0 qualification is a separate gate.
        #[arg(long)]
        profile: Option<PathBuf>,
    },
}

fn main() {
    if ctrlc::set_handler(process::cancel).is_err() {
        eprintln!("peervault: cannot install cancellation handler");
        std::process::exit(if process::cancelled() { 130 } else { 1 });
    }
    if let Err(error) = run() {
        eprintln!("peervault: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    match Cli::parse().command {
        Commands::Fixture {
            directory,
            revision,
        } => {
            fs::create_dir(&directory).context("fixture directory must be new")?;
            let mut manifest = fixture::generate(&directory.join("source"), false)?;
            if revision == 2 {
                manifest = fixture::mutate(&directory.join("source"))?;
            }
            fixture::write_manifest(&directory.join("expected.json"), &manifest)?;
            println!(
                "Created fixture v1 revision {revision}: {} files, {} bytes; expected.json is the oracle.",
                manifest.files.len(),
                manifest.files.iter().map(|entry| entry.size).sum::<u64>()
            );
        }
        Commands::Compare {
            directory,
            manifest,
        } => {
            fixture::compare(&directory, &fixture::read_manifest(&manifest)?)?;
            println!("Exact file/directory set, lengths and SHA-256 verified.");
        }
        Commands::Bakeoff {
            backend,
            executable,
            work_dir,
            profile,
        } => {
            backend::bakeoff(backend, &executable, &work_dir, profile.as_deref())?;
            println!(
                "R0 baseline passed. See evidence.json; remaining qualification gates are listed in docs/r0.md."
            );
        }
    }
    Ok(())
}
