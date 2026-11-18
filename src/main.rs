use anyhow::{Context, Result};
use bailiff::{backend, fixture, process};
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf};

mod agent;
mod ipc;
mod pins;
mod production;
mod recovery_kit;
mod retention;
mod schedule;

#[derive(Parser)]
#[command(
    version,
    about = "Experimental PeerVault recovery tools. R0 qualification and V0.1 local capture; no supported release claim."
)]
struct Cli {
    /// Emit a bounded JSON error document on stdout instead of human stderr.
    #[arg(long, global = true)]
    error_json: bool,
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
    /// Generate or verify a secret-free RecoveryKit v1.
    RecoveryKit {
        #[command(subcommand)]
        action: RecoveryKitAction,
    },
    /// Preview or execute the owner-reviewed retention plan.
    Retention {
        #[command(subcommand)]
        action: RetentionAction,
    },
    /// Report owner-scheduled verification/scratch-restore freshness.
    ScheduleStatus {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        state: PathBuf,
    },
    /// Run at most one due scheduled job per configured entry.
    ScheduleTick {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        state: PathBuf,
    },
    /// Serve owner-only IPC requests on a named pipe (Windows) or Unix socket.
    Agent {
        /// Exact pipe name or absolute Unix socket path. Never a TCP endpoint.
        #[arg(long)]
        endpoint: String,
    },
}

#[derive(Subcommand)]
enum RetentionAction {
    /// Create the owner state file and record the current recovery baseline.
    StateInit {
        #[arg(long)]
        repository: PathBuf,
        #[arg(long)]
        backend: PathBuf,
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        password_file: Option<PathBuf>,
    },
    /// Replace the mass-change baseline after explicit owner review.
    Rebaseline {
        #[arg(long)]
        repository: PathBuf,
        #[arg(long)]
        backend: PathBuf,
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        password_file: Option<PathBuf>,
        #[arg(long)]
        reason: String,
    },
    /// Record one verified peer recovery root in owner state.
    PeerRecord {
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        peer_id: String,
        #[arg(long)]
        peer_repository_id: String,
        #[arg(long)]
        snapshot: String,
        #[arg(long)]
        capture_id: Option<String>,
        #[arg(long)]
        oracle_manifest: Option<String>,
        #[arg(long)]
        key_version: String,
        /// Existing report proving the exact copy's full read and clean restore.
        #[arg(long)]
        evidence: PathBuf,
        /// Observed verification time; defaults to now.
        #[arg(long)]
        verified_unix_seconds: Option<u64>,
    },
    /// Explicitly abandon a peer root after reviewing lost copy coverage.
    PeerAbandon {
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        peer_id: String,
        #[arg(long)]
        reason: String,
    },
    /// Preview exact keep/delete decisions and bind an execution token.
    Preview {
        #[arg(long)]
        repository: PathBuf,
        #[arg(long)]
        backend: PathBuf,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        password_file: Option<PathBuf>,
    },
    /// Revalidate a preview under lock and run backend-native forget/prune.
    Apply {
        /// Preview JSON written by `retention preview`; never hand-edit it.
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        backend: PathBuf,
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        password_file: Option<PathBuf>,
        /// Required when the preview reports a mass-change pause or unknown baseline.
        #[arg(long)]
        ack_mass_change: bool,
    },
}

#[derive(Subcommand)]
enum RecoveryKitAction {
    /// Generate a kit for one committed snapshot and its encrypted oracle.
    Generate {
        #[arg(long)]
        repository: PathBuf,
        #[arg(long)]
        backend: PathBuf,
        #[arg(long)]
        snapshot: String,
        /// New kit directory.
        #[arg(long)]
        kit: PathBuf,
        #[arg(long)]
        password_file: Option<PathBuf>,
    },
    /// Verify the kit inventory, and optionally the repository binding and full read.
    Verify {
        #[arg(long)]
        kit: PathBuf,
        #[arg(long)]
        repository: Option<PathBuf>,
        #[arg(long)]
        backend: Option<PathBuf>,
        #[arg(long)]
        password_file: Option<PathBuf>,
    },
}

fn main() {
    if ctrlc::set_handler(process::cancel).is_err() {
        eprintln!("peervault: cannot install cancellation handler");
        std::process::exit(if process::cancelled() { 130 } else { 1 });
    }
    let cli = Cli::parse();
    let error_json = cli.error_json;
    if let Err(error) = run(cli) {
        let code = if process::cancelled() {
            130
        } else {
            production::exit_code(&error)
        };
        if error_json {
            let _ = production::print_json(&serde_json::json!({
                "schema_version": 1,
                "result": "fail",
                "code": code,
                "message": format!("{error:#}"),
            }));
        } else {
            eprintln!("peervault: {error:#}");
        }
        std::process::exit(code);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
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
        Commands::Retention { action } => match action {
            RetentionAction::StateInit {
                repository,
                backend,
                state,
                password_file,
            } => retention::state_init(&repository, &backend, &state, password_file.as_deref())
                .map_err(production::operation_failure)?,
            RetentionAction::Rebaseline {
                repository,
                backend,
                state,
                password_file,
                reason,
            } => retention::rebaseline(
                &repository,
                &backend,
                &state,
                password_file.as_deref(),
                &reason,
            )
            .map_err(production::operation_failure)?,
            RetentionAction::PeerRecord {
                state,
                peer_id,
                peer_repository_id,
                snapshot,
                capture_id,
                oracle_manifest,
                key_version,
                evidence,
                verified_unix_seconds,
            } => retention::peer_record(
                &state,
                retention::PeerRecordInput {
                    peer_id: &peer_id,
                    peer_repository_id: &peer_repository_id,
                    snapshot: &snapshot,
                    capture_id: capture_id.as_deref(),
                    oracle_manifest: oracle_manifest.as_deref(),
                    key_version: &key_version,
                    evidence: &evidence,
                    verified_unix_seconds,
                },
            )
            .map_err(production::operation_failure)?,
            RetentionAction::PeerAbandon {
                state,
                peer_id,
                reason,
            } => retention::peer_abandon(&state, &peer_id, &reason)
                .map_err(production::operation_failure)?,
            RetentionAction::Preview {
                repository,
                backend,
                policy,
                state,
                password_file,
            } => retention::preview(
                &repository,
                &backend,
                &policy,
                &state,
                password_file.as_deref(),
            )
            .map_err(production::operation_failure)?,
            RetentionAction::Apply {
                plan,
                backend,
                state,
                password_file,
                ack_mass_change,
            } => retention::apply(
                &plan,
                &backend,
                &state,
                password_file.as_deref(),
                ack_mass_change,
            )
            .map_err(production::operation_failure)?,
        },
        Commands::ScheduleStatus { config, state } => {
            let report = schedule::status(&config, &state)?;
            schedule::print_report(&report)?;
        }
        Commands::ScheduleTick { config, state } => {
            let report = schedule::tick_once(&config, &state)?;
            schedule::print_report(&report)?;
        }
        Commands::Agent { endpoint } => {
            agent::serve(&endpoint)?;
        }
        Commands::RecoveryKit { action } => match action {
            RecoveryKitAction::Generate {
                repository,
                backend,
                snapshot,
                kit,
                password_file,
            } => {
                recovery_kit::generate(
                    &repository,
                    &backend,
                    &snapshot,
                    &kit,
                    password_file.as_deref(),
                )
                .map_err(production::operation_failure)?;
            }
            RecoveryKitAction::Verify {
                kit,
                repository,
                backend,
                password_file,
            } => {
                recovery_kit::verify(
                    &kit,
                    repository.as_deref(),
                    backend.as_deref(),
                    password_file.as_deref(),
                )
                .map_err(production::operation_failure)?;
            }
        },
    }
    Ok(())
}
