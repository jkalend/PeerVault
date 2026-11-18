"""V0.8 owner-only IPC drill.

Starts the named-pipe/Unix-socket agent, exercises version and allowlist
refusals, runs history and schedule-status through the agent, proves a second
agent cannot own the endpoint, and reconnects after restart. Cross-user denial
requires a second OS account and remains part of independent evidence.
"""

import argparse
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import sys
import time


def environment():
    return {
        name: os.environ[name]
        for name in ("PATH", "SystemRoot", "WINDIR", "TMP", "TEMP", "TMPDIR")
        if name in os.environ
    }


def run(binary, args, timeout=600):
    return subprocess.run(
        [binary, *args],
        capture_output=True,
        text=True,
        env=environment(),
        timeout=timeout,
    )


def require(result, label):
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()[:1000]
        raise SystemExit(f"{label} failed ({result.returncode}): {detail}")
    return result


def connect(endpoint, timeout=10):
    if os.name == "nt":
        pipe = open(rf"\\.\pipe\{endpoint}", "r+b", buffering=0)
        return pipe
    stream = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    stream.settimeout(timeout)
    stream.connect(endpoint)
    return stream


def connect_retry(endpoint, attempts=100):
    last = None
    for _ in range(attempts):
        try:
            return connect(endpoint)
        except OSError as error:
            last = error
            time.sleep(0.05)
    raise last


def request(endpoint, payload, raw=False):
    stream = connect_retry(endpoint)
    try:
        data = payload if raw else (json.dumps(payload) + "\n").encode()
        if hasattr(stream, "write"):
            stream.write(data)
        else:
            stream.sendall(data)
        if hasattr(stream, "flush"):
            stream.flush()
        if os.name == "nt":
            line = stream.readline()
        else:
            buffer = b""
            while not buffer.endswith(b"\n"):
                chunk = stream.recv(65536)
                if not chunk:
                    break
                buffer += chunk
            line = buffer
        return json.loads(line) if line.strip() else None
    finally:
        stream.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--root", type=Path, required=True, help="new drill root")
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    if root.exists():
        raise SystemExit(f"root must be new: {root}")
    root.mkdir(parents=True)
    binary, backend = str(arguments.binary.resolve()), str(arguments.backend.resolve())
    repository = root / "repository"
    source = root / "source"
    source.mkdir()
    (source / "hello.txt").write_text("ipc\n", encoding="utf-8")
    secret = root / "secret.txt"
    secret.write_text(secrets.token_hex(24), encoding="utf-8")
    if os.name != "nt":
        os.chmod(secret, 0o600)
    require(
        run(binary, ["init", "--repository", str(repository), "--backend", backend, "--password-file", str(secret)]),
        "init",
    )
    require(
        run(
            binary,
            [
                "capture",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--source",
                str(source),
                "--password-file",
                str(secret),
            ],
        ),
        "capture",
    )

    endpoint = (
        f"peervault-test-{secrets.token_hex(6)}"
        if os.name == "nt"
        else str(root / "agent.sock")
    )
    if os.name != "nt":
        os.chmod(root, 0o700)
    agent = subprocess.Popen(
        [binary, "agent", "--endpoint", endpoint],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=environment(),
    )
    history = request(
        endpoint,
        {
            "schema_version": 1,
            "request_id": "history-1",
            "argv": [
                "history",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--password-file",
                str(secret),
            ],
        },
    )
    assert history["result"] == "pass", history
    assert len(history["report"]["snapshots"]) == 1, history

    unsupported = request(
        endpoint,
        {"schema_version": 1, "request_id": "bad-op", "argv": ["bakeoff", "restic"]},
    )
    assert unsupported["result"] == "fail" and "not exposed" in unsupported["error"]

    version = request(
        endpoint,
        {"schema_version": 99, "request_id": "bad-version", "argv": ["history"]},
    )
    assert version["result"] == "fail" and "schema" in version["error"]

    unknown = request(
        endpoint,
        {"schema_version": 1, "request_id": "unknown", "argv": ["history"], "extra": 1},
    )
    assert unknown["result"] == "fail"

    missing_secret = request(
        endpoint,
        {
            "schema_version": 1,
            "request_id": "missing-secret",
            "argv": ["history", "--repository", str(repository), "--backend", backend],
        },
    )
    assert missing_secret["result"] == "fail" and missing_secret["exit_code"] == 3
    assert missing_secret["error"]

    config = root / "jobs.json"
    state = root / "schedule-state.json"
    config.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "jobs": [
                    {
                        "id": "daily-verify",
                        "kind": "verify",
                        "repository": str(repository),
                        "backend": backend,
                        "password_file": str(secret),
                        "interval_seconds": 86400,
                        "freshness_seconds": 172800,
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    status = request(
        endpoint,
        {
            "schema_version": 1,
            "request_id": "status-1",
            "argv": [
                "schedule-status",
                "--config",
                str(config),
                "--state",
                str(state),
            ],
        },
    )
    assert status["result"] == "pass", status
    assert status["report"]["jobs"][0]["status"] == "unknown", status

    second = subprocess.Popen(
        [binary, "agent", "--endpoint", endpoint],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=environment(),
    )
    second.wait(timeout=10)
    assert second.returncode != 0, "second agent unexpectedly owned the endpoint"

    agent.terminate()
    try:
        agent.wait(timeout=20)
    except subprocess.TimeoutExpired:
        agent.kill()
        agent.wait(timeout=10)
    if os.name != "nt":
        for _ in range(100):
            if not Path(endpoint).exists():
                break
            time.sleep(0.05)

    restarted = subprocess.Popen(
        [binary, "agent", "--endpoint", endpoint],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=environment(),
    )
    try:
        after = request(
            endpoint,
            {
                "schema_version": 1,
                "request_id": "history-2",
                "argv": [
                    "history",
                    "--repository",
                    str(repository),
                    "--backend",
                    backend,
                    "--password-file",
                    str(secret),
                ],
            },
        )
        assert after["result"] == "pass", after
    finally:
        restarted.terminate()
        try:
            restarted.wait(timeout=20)
        except subprocess.TimeoutExpired:
            restarted.kill()
            restarted.wait(timeout=10)

    print(
        json.dumps(
            {
                "result": "pass",
                "transport": "named-pipe" if os.name == "nt" else "unix-socket",
                "allowlist_refusal": True,
                "version_refusal": True,
                "restart": True,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
