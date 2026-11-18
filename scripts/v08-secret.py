"""V0.8 Windows DPAPI scheduled-secret drill.

Stores a repository secret with current-user DPAPI, removes the plaintext, runs
the scheduled tick through the decrypting wrapper with a job config that has no
password_file, and verifies the run and cleanup. Linux systemd credentials are
covered by the installer dry run, not this script.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess


def run(command, timeout=900):
    return subprocess.run(command, capture_output=True, text=True, timeout=timeout)


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
    if os.name != "nt":
        print(json.dumps({"result": "skipped", "reason": "DPAPI drill requires Windows"}))
        return
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
    (source / "secret-drill.txt").write_text("dpapi\n", encoding="utf-8")
    plaintext = root / "plaintext-secret.txt"
    plaintext.write_text(secrets.token_hex(24), encoding="utf-8")
    protected = root / "repository-secret.dpapi"
    run_directory = Path(os.environ["LOCALAPPDATA"]) / "PeerVault" / "run"
    before = set(run_directory.glob("secret-*.txt")) if run_directory.is_dir() else set()
    require(
        run(
            [
                binary,
                "init",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--password-file",
                str(plaintext),
            ]
        ),
        "init",
    )
    require(
        run(
            [
                binary,
                "capture",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--source",
                str(source),
                "--password-file",
                str(plaintext),
            ]
        ),
        "capture",
    )
    require(
        run(
            [
                "powershell",
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                str(Path(__file__).with_name("secret-store.ps1")),
                "-Action",
                "Store",
                "-SecretFile",
                str(plaintext),
                "-ProtectedFile",
                str(protected),
            ]
        ),
        "secret store",
    )
    plaintext.unlink()
    config = root / "jobs.json"
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
                        "interval_seconds": 86400,
                        "freshness_seconds": 172800,
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    state = root / "schedule-state.json"
    result = require(
        run(
            [
                "powershell",
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                str(Path(__file__).with_name("task-run.ps1")),
                "-Core",
                binary,
                "-Config",
                str(config),
                "-State",
                str(state),
                "-ProtectedSecret",
                str(protected),
            ]
        ),
        "task-run",
    )
    after = set(run_directory.glob("secret-*.txt")) if run_directory.is_dir() else set()
    assert after <= before, "runtime secret was not removed"
    state_data = json.loads(state.read_text(encoding="utf-8"))
    job = state_data["jobs"]["daily-verify"]
    assert job["last_result"] == "pass", state_data
    status = json.loads(
        require(
            run(
                [
                    binary,
                    "schedule-status",
                    "--config",
                    str(config),
                    "--state",
                    str(state),
                ]
            ),
            "status",
        ).stdout
    )
    assert status["jobs"][0]["status"] in ("fresh", "stale"), status
    print(
        json.dumps(
            {
                "result": "pass",
                "protected_secret": protected.name,
                "runtime_secret_removed": True,
                "job_result": job["last_result"],
                "limitation": "same-user DPAPI; task identity and cross-user denial remain environment-specific",
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
