//! Owner-only local stdio IPC for the experimental V0.8 desktop.
//!
//! The desktop starts this process with inherited redirected stdin/stdout.
//! No TCP port or shared socket is opened. Requests and responses are one JSON
//! line each; only the parent that owns the anonymous pipes can reach the agent.
//! A future versioned named-pipe/Unix-socket transport must preserve the same
//! single-core authority and single-use destructive-token rules.

use crate::schedule;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
};

pub const SCHEMA_VERSION: u32 = 1;
const MAX_REQUEST_BYTES: usize = 64 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema_version: u32,
    id: u64,
    op: String,
}

#[derive(Debug, Serialize)]
struct Response {
    schema_version: u32,
    id: u64,
    result: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<schedule::StatusReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn response(id: u64, result: Result<schedule::StatusReport>) -> Response {
    match result {
        Ok(report) => Response {
            schema_version: SCHEMA_VERSION,
            id,
            result: "pass",
            report: Some(report),
            error: None,
        },
        Err(error) => Response {
            schema_version: SCHEMA_VERSION,
            id,
            result: "fail",
            report: None,
            error: Some(format!("{error:#}")),
        },
    }
}

fn execute(request: &Request, config: &Path, state: &Path) -> Result<schedule::StatusReport> {
    ensure!(
        request.schema_version == SCHEMA_VERSION,
        "unsupported agent request version"
    );
    match request.op.as_str() {
        "status" => schedule::status(config, state),
        "tick" => schedule::tick_once(config, state),
        other => anyhow::bail!("unsupported agent operation {other:?}"),
    }
}

/// Serve requests until the owning desktop closes stdin.
pub fn serve(config: &Path, state: &Path) -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .context("cannot read agent request")?;
        if read == 0 {
            return Ok(());
        }
        ensure!(
            line.len() <= MAX_REQUEST_BYTES,
            "agent request exceeds bounded size"
        );
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(request) => response(request.id, execute(&request, config, state)),
            Err(error) => Response {
                schema_version: SCHEMA_VERSION,
                id: 0,
                result: "fail",
                report: None,
                error: Some(format!("invalid request: {error}")),
            },
        };
        serde_json::to_writer(&mut writer, &response).context("cannot encode agent response")?;
        writer
            .write_all(b"\n")
            .context("cannot terminate agent response")?;
        writer.flush().context("cannot flush agent response")?;
    }
}
