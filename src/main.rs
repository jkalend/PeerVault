use anyhow::{Context, Result};
use bailiff::{backend, fixture, process};
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf};

mod pins;
mod production;

#[derive(Parser)]
#[command(
    version,
    about = "Experimental PeerVault recovery tools. R0 qualification and V0.1 local capture; no supported release claim."
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
    /// Copy snapshots to an independently identified medium, remove the source, and recover through a fresh client.
    CopyDrill {
        #[arg(value_enum)]
        backend: backend::Kind,
        #[arg(long)]
        executable: PathBuf,
        /// New writer root: synthetic source, source repository and writer config.
        #[arg(long)]
        work_dir: PathBuf,
        /// New root for the independently stored repository copy.
        #[arg(long)]
        copy_dir: PathBuf,
        /// New root for the fresh receiver profile and restore targets.
        #[arg(long)]
        recovery_dir: PathBuf,
        /// Named smoke profile. Scored R0 qualification is a separate gate.
        #[arg(long)]
        profile: Option<PathBuf>,
        /// Retain unknown media coverage instead of refusing when identity cannot be observed.
        #[arg(long)]
        acknowledge_unknown_media: bool,
    },
    /// Initialize a new restic repository and print the PeerVault profile.
    Init {
        #[arg(long)]
        repository: PathBuf,
        /// Configured, checksum-verified restic executable.
        #[arg(long)]
        backend: PathBuf,
        /// File containing the repository password; otherwise PEERVAULT_PASSWORD.
        #[arg(long)]
        password_file: Option<PathBuf>,
    },
    /// Capture one local source and its encrypted tree oracle.
    Capture {
        #[arg(long)]
        repository: PathBuf,
        #[arg(long)]
        backend: PathBuf,
        #[arg(long)]
        source: PathBuf,
        /// Optional stable capture label; never a secret.
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        password_file: Option<PathBuf>,
    },
    /// List PeerVault captures and whether each has a tree oracle.
    History {
        #[arg(long)]
        repository: PathBuf,
        #[arg(long)]
        backend: PathBuf,
        #[arg(long)]
        password_file: Option<PathBuf>,
    },
    /// Full-read verify the repository and optionally assert a snapshot exists.
    Verify {
        #[arg(long)]
        repository: PathBuf,
        #[arg(long)]
        backend: PathBuf,
        #[arg(long)]
        snapshot: Option<String>,
        #[arg(long)]
        password_file: Option<PathBuf>,
    },
    /// Restore one whole snapshot into a new empty destination.
    Restore {
        #[arg(long)]
        repository: PathBuf,
        #[arg(long)]
        backend: PathBuf,
        #[arg(long)]
        snapshot: String,
        #[arg(long)]
        destination: PathBuf,
        #[arg(long)]
        password_file: Option<PathBuf>,
    },
}

fn main() {
    if ctrlc::set_handler(process::cancel).is_err() {
        eprintln!("peervault: cannot install cancellation handler");
        std::process::exit(if process::cancelled() { 130 } else { 1 });
    }
    if let Err(error) = run() {
        let code = production::exit_code(&error);
        eprintln!("peervault: {error:#}");
        std::process::exit(if process::cancelled() { 130 } else { code });
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
            backend::bakeoff(
                backend,
                &executable,
                &work_dir,
                profile.as_deref(),
                &pins::inputs()?,
            )?;
            println!(
                "R0 baseline passed. See evidence.json; remaining qualification gates are listed in docs/r0.md."
            );
        }
        Commands::CopyDrill {
            backend,
            executable,
            work_dir,
            copy_dir,
            recovery_dir,
            profile,
            acknowledge_unknown_media,
        } => {
            backend::copy_drill(
                backend,
                &executable,
                &work_dir,
                &copy_dir,
                &recovery_dir,
                profile.as_deref(),
                acknowledge_unknown_media,
                &pins::inputs()?,
            )?;
            println!(
                "R0 copy/recovery topology drill passed. See evidence.json; this remains a partial gate."
            );
        }
        Commands::Init {
            repository,
            backend,
            password_file,
        } => {
            production::init(&repository, &backend, password_file.as_deref())
                .map_err(production::operation_failure)?;
        }
        Commands::Capture {
            repository,
            backend,
            source,
            tag,
            password_file,
        } => {
            production::capture(
                &repository,
                &backend,
                &source,
                tag.as_deref(),
                password_file.as_deref(),
            )
            .map_err(production::operation_failure)?;
        }
        Commands::History {
            repository,
            backend,
            password_file,
        } => {
            production::history(&repository, &backend, password_file.as_deref())
                .map_err(production::operation_failure)?;
        }
        Commands::Verify {
            repository,
            backend,
            snapshot,
            password_file,
        } => {
            production::verify(
                &repository,
                &backend,
                snapshot.as_deref(),
                password_file.as_deref(),
            )
            .map_err(production::operation_failure)?;
        }
        Commands::Restore {
            repository,
            backend,
            snapshot,
            destination,
            password_file,
        } => {
            production::restore(
                &repository,
                &backend,
                &snapshot,
                &destination,
                password_file.as_deref(),
            )
            .map_err(production::operation_failure)?;
        }
    }
    Ok(())
}
