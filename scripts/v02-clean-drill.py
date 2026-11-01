"""Author-only V0.2 clean-recovery drill: copy, source removal, kit, full read, restore."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import sys


def remove_tree(path):
    def retry(function, target, _error):
        os.chmod(target, 0o666)
        function(target)

    shutil.rmtree(path, onexc=retry)


def tree(root):
    files, directories = {}, set()
    for path in root.rglob("*"):
        relative = path.relative_to(root).as_posix()
        if path.is_dir():
            directories.add(relative)
        elif path.is_file():
            files[relative] = (path.stat().st_size, hashlib.sha256(path.read_bytes()).hexdigest())
    return files, directories


def clean_environment():
    allowed = ("PATH", "SystemRoot", "WINDIR", "TMP", "TEMP", "TMPDIR")
    return {name: os.environ[name] for name in allowed if name in os.environ}


def run(binary, args, environment):
    return subprocess.run([binary, *args], capture_output=True, text=True, env=environment, timeout=1800)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--work-root", type=Path, required=True, help="new source-host root")
    parser.add_argument("--copy-root", type=Path, required=True, help="new independent copy root")
    parser.add_argument("--recovery-root", type=Path, required=True, help="new recovery-device root")
    arguments = parser.parse_args()
    for path in (arguments.work_root, arguments.copy_root, arguments.recovery_root):
        if path.exists():
            raise SystemExit(f"drill root must be new: {path}")
        path.mkdir(parents=True)
    binary, backend = str(arguments.binary), str(arguments.backend)
    source = arguments.work_root / "source"
    (source / "nested").mkdir(parents=True)
    (source / "empty-directory").mkdir()
    (source / "empty.txt").write_bytes(b"")
    (source / "unicode-žluťoučký-雪.txt").write_text("unicode\n", encoding="utf-8")
    (source / "readonly.bin").write_bytes(b"readonly")
    os.chmod(source / "readonly.bin", 0o444)
    (source / "nested" / "data.bin").write_bytes(os.urandom(2 * 1024 * 1024))
    expected = tree(source)
    secret = arguments.recovery_root / "secret.txt"
    secret.write_text(secrets.token_hex(24), encoding="utf-8")
    if os.name != "nt":
        os.chmod(secret, 0o600)
    environment = clean_environment()
    run_environment = dict(environment, PEERVAULT_PASSWORD="")

    source_repository = arguments.work_root / "repository-source"
    result = run(binary, ["init", "--repository", str(source_repository), "--backend", backend, "--password-file", str(secret)], run_environment)
    assert result.returncode == 0, result.stderr
    result = run(binary, ["capture", "--repository", str(source_repository), "--backend", backend, "--source", str(source), "--tag", "clean-drill", "--password-file", str(secret)], run_environment)
    assert result.returncode == 0, result.stderr
    source_capture = json.loads(result.stdout)
    result = run(binary, ["history", "--repository", str(source_repository), "--backend", backend, "--password-file", str(secret)], run_environment)
    assert result.returncode == 0, result.stderr
    source_history = json.loads(result.stdout)
    assert len(source_history["snapshots"]) == 1
    assert source_history["snapshots"][0]["capture_id"]

    copy_repository = arguments.copy_root / "repository"
    restic_environment = dict(environment, RESTIC_PASSWORD_FILE=str(secret))
    result = subprocess.run([backend, "--repo", str(copy_repository), "--no-cache", "init", "--repository-version=2"], capture_output=True, text=True, env=restic_environment)
    assert result.returncode == 0, result.stderr
    result = subprocess.run(
        [backend, "--repo", str(copy_repository), "--no-cache", "copy", "--from-repo", str(source_repository), "--from-password-file", str(secret)],
        capture_output=True, text=True, env=restic_environment,
    )
    assert result.returncode == 0, result.stderr
    result = run(binary, ["history", "--repository", str(copy_repository), "--backend", backend, "--password-file", str(secret)], run_environment)
    assert result.returncode == 0, result.stderr
    copy_history = json.loads(result.stdout)
    assert len(copy_history["snapshots"]) == 1
    copied = copy_history["snapshots"][0]
    assert copied["capture_id"] == source_history["snapshots"][0]["capture_id"]
    assert copied["oracle_manifest"]

    remove_tree(source)
    remove_tree(source_repository)
    assert not source.exists() and not source_repository.exists()

    kit = arguments.recovery_root / "kit"
    result = run(binary, ["recovery-kit", "generate", "--repository", str(copy_repository), "--backend", backend, "--snapshot", copied["id"], "--kit", str(kit), "--password-file", str(secret)], run_environment)
    assert result.returncode == 0, result.stderr
    result = run(binary, ["recovery-kit", "verify", "--kit", str(kit), "--repository", str(copy_repository), "--backend", backend, "--password-file", str(secret)], run_environment)
    assert result.returncode == 0, result.stderr
    kit_report = json.loads(result.stdout)
    assert kit_report["full_read"] and kit_report["repository_bound"]

    destination = arguments.recovery_root / "restored"
    result = run(binary, ["restore", "--repository", str(copy_repository), "--backend", backend, "--snapshot", copied["id"], "--destination", str(destination), "--password-file", str(secret)], run_environment)
    assert result.returncode == 0, result.stderr
    assert tree(destination) == expected
    if os.name == "nt":
        (destination / "readonly.bin").chmod(0o666)
    print(json.dumps({
        "result": "pass",
        "scope": "author-only same-host copy/recovery drill; source tree/repository removed",
        "capture_id": copied["capture_id"],
        "source_snapshot": source_capture["snapshot_id"],
        "copy_snapshot": copied["id"],
        "manifest_snapshot": copied["oracle_manifest"],
        "kit_id": kit_report["kit_id"],
        "repository_id_bound": kit_report["repository_bound"],
        "full_read": kit_report["full_read"],
        "restored_files": len(expected[0]),
        "limitations": [
            "not a second-operator usability test",
            "same host; copy and recovery media are independently named but not clean hardware",
            "network-disabled claim is documented, not enforced by this script",
        ],
    }, indent=2))


if __name__ == "__main__":
    try:
        main()
    except AssertionError as error:
        print(f"V0.2 clean drill failed: {error}", file=sys.stderr)
        raise SystemExit(1)
