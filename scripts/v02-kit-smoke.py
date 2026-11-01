"""V0.2 RecoveryKit smoke: generate, inventory verify, bound full-read verify, tamper refusal."""

import argparse
import json
import os
from pathlib import Path
import secrets
import subprocess
import sys


def clean_environment():
    allowed = ("PATH", "SystemRoot", "WINDIR", "TMP", "TEMP", "TMPDIR")
    return {name: os.environ[name] for name in allowed if name in os.environ}


def run(binary, args, environment):
    return subprocess.run([binary, *args], capture_output=True, text=True, env=environment, timeout=1800)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--root", type=Path, required=True, help="new disposable work root")
    arguments = parser.parse_args()
    if arguments.root.exists():
        raise SystemExit(f"work root must be new: {arguments.root}")
    arguments.root.mkdir(parents=True)
    binary, backend = str(arguments.binary), str(arguments.backend)
    source = arguments.root / "source"
    source.mkdir()
    (source / "file.txt").write_text("kit smoke\n", encoding="utf-8")
    (source / "empty.txt").write_bytes(b"")
    repository = arguments.root / "repository"
    secret = arguments.root / "secret.txt"
    secret.write_text(secrets.token_hex(24), encoding="utf-8")
    if os.name != "nt":
        os.chmod(secret, 0o600)
    environment = dict(clean_environment(), PEERVAULT_PASSWORD="")

    result = run(binary, ["init", "--repository", str(repository), "--backend", backend, "--password-file", str(secret)], environment)
    assert result.returncode == 0, result.stderr
    result = run(binary, ["capture", "--repository", str(repository), "--backend", backend, "--source", str(source), "--password-file", str(secret)], environment)
    assert result.returncode == 0, result.stderr
    snapshot = json.loads(result.stdout)
    kit = arguments.root / "kit"
    result = run(binary, ["recovery-kit", "generate", "--repository", str(repository), "--backend", backend, "--snapshot", snapshot["snapshot_id"], "--kit", str(kit), "--password-file", str(secret)], environment)
    assert result.returncode == 0, result.stderr
    generated = json.loads(result.stdout)
    assert generated["repository_bound"] and not generated["full_read"]

    inventory = run(binary, ["recovery-kit", "verify", "--kit", str(kit)], clean_environment())
    assert inventory.returncode == 0, inventory.stderr
    assert json.loads(inventory.stdout)["members"] >= 3

    bound = run(binary, ["recovery-kit", "verify", "--kit", str(kit), "--repository", str(repository), "--backend", backend, "--password-file", str(secret)], environment)
    assert bound.returncode == 0, bound.stderr
    assert json.loads(bound.stdout)["full_read"]

    wrong = arguments.root / "wrong.txt"
    wrong.write_text("wrong-password", encoding="utf-8")
    result = run(binary, ["recovery-kit", "verify", "--kit", str(kit), "--repository", str(repository), "--backend", backend, "--password-file", str(wrong)], environment)
    assert result.returncode != 0

    runbook = kit / "RUNBOOK.md"
    runbook.write_text(runbook.read_text(encoding="utf-8") + "tamper\n", encoding="utf-8")
    result = run(binary, ["recovery-kit", "verify", "--kit", str(kit)], clean_environment())
    assert result.returncode != 0
    print("V0.2 kit smoke passed: inventory, full-read binding, wrong secret and tamper refusal")


if __name__ == "__main__":
    try:
        main()
    except AssertionError as error:
        print(f"V0.2 kit smoke failed: {error}", file=sys.stderr)
        raise SystemExit(1)
