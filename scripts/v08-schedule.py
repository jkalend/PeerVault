"""V0.8 scheduler status, missed-run, clock, credential-env and quota drill."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess
import time

if os.name == "nt":
    import msvcrt
else:
    import fcntl


def hold_lock(path):
    stream = path.open("a+b")
    stream.write(b"x")
    stream.flush()
    if os.name == "nt":
        msvcrt.locking(stream.fileno(), msvcrt.LK_NBLCK, 1)
    else:
        fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
    return stream


def environment(extra=None):
    env = {
        name: os.environ[name]
        for name in ("PATH", "SystemRoot", "WINDIR", "TMP", "TEMP", "TMPDIR")
        if name in os.environ
    }
    if extra:
        env.update(extra)
    return env


def run(binary, args, timeout=900, extra_env=None):
    return subprocess.run(
        [binary, *args],
        capture_output=True,
        text=True,
        env=environment(extra_env),
        timeout=timeout,
    )


def require(result, label):
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()[:1000]
        raise SystemExit(f"{label} failed ({result.returncode}): {detail}")
    return result


def tree(root):
    files, directories = {}, set()
    for path in root.rglob("*"):
        relative = path.relative_to(root).as_posix()
        if path.is_dir():
            directories.add(relative)
        elif path.is_file():
            files[relative] = (
                path.stat().st_size,
                hashlib.sha256(path.read_bytes()).hexdigest(),
            )
    return files, directories


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
    (source / "hello.txt").write_text("schedule\n", encoding="utf-8")
    secret = root / "secret.txt"
    secret.write_text(secrets.token_hex(24), encoding="utf-8")
    if os.name != "nt":
        os.chmod(secret, 0o600)
    require(run(binary, ["init", "--repository", str(repository), "--backend", backend, "--password-file", str(secret)]), "init")
    captured = json.loads(
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
        ).stdout
    )
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
    status = json.loads(require(run(binary, ["schedule-status", "--config", str(config), "--state", str(state)]), "initial status").stdout)
    assert status["jobs"][0]["status"] == "unknown", status
    first = json.loads(require(run(binary, ["schedule-tick", "--config", str(config), "--state", str(state)]), "first tick").stdout)
    assert first["jobs"][0]["last_result"] == "pass", first
    first_attempt = first["jobs"][0]["last_attempt_unix"]
    assert first["jobs"][0]["status"] == "fresh", first
    second = json.loads(require(run(binary, ["schedule-tick", "--config", str(config), "--state", str(state)]), "second tick").stdout)
    assert second["jobs"][0]["last_attempt_unix"] == first_attempt, "not-due tick re-ran the job"

    state_data = json.loads(state.read_text(encoding="utf-8"))
    job_state = state_data["jobs"]["daily-verify"]
    job_state["last_success_unix"] = job_state["last_attempt_unix"] - 3 * 86400
    job_state["next_due_unix"] = job_state["last_attempt_unix"] - 3 * 86400
    job_state["last_result"] = "pass"
    state.write_text(json.dumps(state_data, indent=2) + "\n", encoding="utf-8")
    missed = json.loads(require(run(binary, ["schedule-tick", "--config", str(config), "--state", str(state)]), "missed tick").stdout)
    assert missed["jobs"][0]["missed_runs"] >= 3, missed

    state_data = json.loads(state.read_text(encoding="utf-8"))
    job_state = state_data["jobs"]["daily-verify"]
    now = int(time.time())
    job_state["last_attempt_unix"] = now + 86400
    job_state["next_due_unix"] = now - 1
    job_state["last_result"] = "pass"
    state.write_text(json.dumps(state_data, indent=2) + "\n", encoding="utf-8")
    clock = json.loads(require(run(binary, ["schedule-tick", "--config", str(config), "--state", str(state)]), "clock tick").stdout)
    assert clock["jobs"][0]["clock_anomaly"], clock
    assert clock["jobs"][0]["status"] == "clock-anomaly", clock

    env_config = root / "jobs-env.json"
    env_config.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "jobs": [
                    {
                        "id": "env-verify",
                        "kind": "verify",
                        "repository": str(repository),
                        "backend": backend,
                        "interval_seconds": 86400,
                        "freshness_seconds": 172800,
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    env_state = root / "env-state.json"
    env = json.loads(
        require(
            run(
                binary,
                ["schedule-tick", "--config", str(env_config), "--state", str(env_state)],
                extra_env={"PEERVAULT_PASSWORD_FILE": str(secret)},
            ),
            "credential-env tick",
        ).stdout
    )
    assert env["jobs"][0]["last_result"] == "pass", env

    lock_state = root / "lock-state.json"
    lock = hold_lock(lock_state.with_suffix(".lock"))
    try:
        busy = run(
            binary,
            ["schedule-tick", "--config", str(env_config), "--state", str(lock_state)],
            extra_env={"PEERVAULT_PASSWORD_FILE": str(secret)},
        )
        assert busy.returncode != 0, "tick ignored an OS-held schedule lock"
        assert "busy" in (busy.stderr + busy.stdout), busy.stderr
    finally:
        lock.close()
    resumed = json.loads(
        require(
            run(
                binary,
                ["schedule-tick", "--config", str(env_config), "--state", str(lock_state)],
                extra_env={"PEERVAULT_PASSWORD_FILE": str(secret)},
            ),
            "tick after lock release",
        ).stdout
    )
    assert resumed["jobs"][0]["last_result"] == "pass", resumed

    restore_root = root / "scratch"
    restore_root.mkdir()
    restore_config = root / "jobs-restore.json"
    restore_state = root / "restore-state.json"
    restore_config.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "jobs": [
                    {
                        "id": "scratch-restore",
                        "kind": "restore",
                        "repository": str(repository),
                        "backend": backend,
                        "password_file": str(secret),
                        "snapshot": captured["snapshot_id"],
                        "destination_root": str(restore_root),
                        "minimum_free_bytes": 2**63,
                        "interval_seconds": 86400,
                        "freshness_seconds": 172800,
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    quota = json.loads(require(run(binary, ["schedule-tick", "--config", str(restore_config), "--state", str(restore_state)]), "quota tick").stdout)
    assert quota["jobs"][0]["last_result"] == "fail", quota
    assert "free" in quota["jobs"][0]["last_error"], quota
    assert not (restore_root / "scratch-restore").exists(), "quota failure created a destination"
    restore_config.write_text(restore_config.read_text(encoding="utf-8").replace(str(2**63), "0"), encoding="utf-8")
    os.remove(restore_state)
    passed = json.loads(require(run(binary, ["schedule-tick", "--config", str(restore_config), "--state", str(restore_state)]), "restore tick").stdout)
    assert passed["jobs"][0]["last_result"] == "pass", passed
    destinations = list((restore_root / "scratch-restore").iterdir())
    assert len(destinations) == 1 and tree(destinations[0]) == tree(source)
    print(
        json.dumps(
            {
                "result": "pass",
                "credential_env": True,
                "lock_released_after_holder_exit": True,
                "missed_runs": missed["jobs"][0]["missed_runs"],
                "clock_anomaly": True,
                "quota_refusal": True,
                "restore_after_quota": True,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
