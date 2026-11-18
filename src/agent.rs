//! Versioned owner-only IPC command server.
//!
//! The transport is a Windows named pipe or a Linux Unix-domain socket. The
//! caller must be the same OS user. Requests contain an argv vector for an
//! allowlisted PeerVault subcommand; the server executes the same core binary
//! in a bounded child process, so the CLI remains the mutation authority.

use crate::ipc;
use anyhow::{Context, Result, bail, ensure};
use bailiff::process::bounded;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    process::{Command, Stdio},
    time::Duration,
};

pub const SCHEMA_VERSION: u32 = 1;
const MAX_ARGS: usize = 64;
const MAX_ARG_BYTES: usize = 4096;
const CHILD_OUTPUT_LIMIT: u64 = 16 * 1024 * 1024;
const CHILD_TIMEOUT: Duration = Duration::from_secs(3600);
const ALLOWED: &[&str] = &[
    "init",
    "capture",
    "history",
    "verify",
    "restore",
    "recovery-kit",
    "retention",
    "schedule-status",
    "schedule-tick",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema_version: u32,
    request_id: String,
    argv: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Response {
    schema_version: u32,
    request_id: String,
    result: &'static str,
    exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn failed(request_id: String, code: i32, message: impl Into<String>) -> Response {
    Response {
        schema_version: SCHEMA_VERSION,
        request_id,
        result: "fail",
        exit_code: code,
        report: None,
        error: Some(message.into()),
    }
}

fn valid_request_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn execute(request: &Request) -> Result<Response> {
    ensure!(
        request.schema_version == SCHEMA_VERSION,
        "unsupported IPC schema version"
    );
    ensure!(valid_request_id(&request.request_id), "invalid request ID");
    ensure!(
        !request.argv.is_empty() && request.argv.len() <= MAX_ARGS,
        "invalid argument count"
    );
    for argument in &request.argv {
        ensure!(
            !argument.is_empty() && argument.len() <= MAX_ARG_BYTES,
            "invalid argument"
        );
        ensure!(
            argument != "--error-json" && !argument.starts_with("--error-json="),
            "the agent owns the error-json flag"
        );
    }
    ensure!(
        ALLOWED.contains(&request.argv[0].as_str()),
        "operation is not exposed over IPC"
    );
    let executable = std::env::current_exe().context("cannot resolve the core executable")?;
    let mut command = Command::new(executable);
    command.env_clear();
    for name in ["PATH", "SystemRoot", "WINDIR", "TMP", "TEMP", "TMPDIR"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
        .arg("--error-json")
        .args(&request.argv)
        .stdin(Stdio::null());
    let (code, stdout) = match bounded(&mut command, CHILD_TIMEOUT, CHILD_OUTPUT_LIMIT) {
        Ok(result) => result,
        Err(error) => {
            return Ok(failed(
                request.request_id.clone(),
                1,
                format!("core operation did not complete: {error:#}"),
            ));
        }
    };
    let code = code.unwrap_or(1);
    let report = if stdout.trim().is_empty() {
        None
    } else {
        serde_json::from_str::<Value>(&stdout).ok()
    };
    if code == 0 {
        return Ok(Response {
            schema_version: SCHEMA_VERSION,
            request_id: request.request_id.clone(),
            result: "pass",
            exit_code: 0,
            report,
            error: None,
        });
    }
    let message = report
        .as_ref()
        .and_then(|value| value["message"].as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("core operation exited with code {code}"));
    Ok(Response {
        schema_version: SCHEMA_VERSION,
        request_id: request.request_id.clone(),
        result: "fail",
        exit_code: code,
        report,
        error: Some(message),
    })
}

fn handle(bytes: &[u8]) -> Vec<u8> {
    let response = match serde_json::from_slice::<Request>(bytes) {
        Ok(request) => execute(&request)
            .unwrap_or_else(|error| failed(request.request_id.clone(), 2, format!("{error:#}"))),
        Err(error) => failed(String::new(), 2, format!("invalid IPC request: {error}")),
    };
    serde_json::to_vec(&response).unwrap_or_else(|error| {
        format!(r#"{{"schema_version":1,"result":"fail","error":"{error}"}}"#).into_bytes()
    })
}

pub fn serve(endpoint: &str) -> Result<()> {
    if endpoint.is_empty() {
        bail!("IPC endpoint must not be empty");
    }
    ipc::serve(endpoint, handle)
}
