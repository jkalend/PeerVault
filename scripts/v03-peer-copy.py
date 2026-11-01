"""V0.3 experimental one-peer copy record and receiver verification.

This exercises the backend-native additive copy to a second repository with a
PeerCopyRecord v1 state trail. The transport is a local filesystem; authenticated
network transport/pairing and independent hardware remain open gates.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import sys
import time


def remove_tree(path):
    def retry(function, target, _error):
        os.chmod(target, 0o666)
        function(target)

    if sys.version_info >= (3, 12):
        shutil.rmtree(path, onexc=retry)
    else:
        shutil.rmtree(path, onerror=lambda function, target, error: retry(function, target, error))


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


def write_record(path, record):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    os.replace(temporary, path)


def step(record, state, result, detail=None):
    record["state"] = state
    record["updated_unix_seconds"] = int(time.time())
    record["steps"].append({"state": state, "result": result, "detail": detail or ""})
    write_record(record["_path"], {key: value for key, value in record.items() if key != "_path"})


def require(record, state, result):
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()[:300]
        step(record, f"{state}-failed", "fail", detail)
        raise SystemExit(f"{state} failed: {detail}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, required=True, help="new source-host root")
    parser.add_argument("--peer-root", type=Path, required=True, help="new receiver repository root")
    parser.add_argument("--recovery-root", type=Path, required=True, help="new receiver/recovery root")
    arguments = parser.parse_args()
    roots = [
        arguments.source_root.resolve(),
        arguments.peer_root.resolve(),
        arguments.recovery_root.resolve(),
    ]
    for left, right in ((0, 1), (0, 2), (1, 2)):
        if roots[left] == roots[right] or roots[left] in roots[right].parents or roots[right] in roots[left].parents:
            raise SystemExit("source, peer and recovery roots must be disjoint")
    arguments.source_root, arguments.peer_root, arguments.recovery_root = roots
    for path in roots:
        if path.exists():
            raise SystemExit(f"root must be new: {path}")
        path.mkdir(parents=True)
    binary, backend = str(arguments.binary), str(arguments.backend)
    source = arguments.source_root / "source"
    (source / "nested").mkdir(parents=True)
    (source / "empty.txt").write_bytes(b"")
    (source / "unicode-žluťoučký-雪.txt").write_text("unicode\n", encoding="utf-8")
    (source / "nested" / "data.bin").write_bytes(os.urandom(1024 * 1024))
    expected = tree(source)
    source_secret = arguments.recovery_root / "source-secret.txt"
    peer_secret = arguments.recovery_root / "peer-secret.txt"
    source_secret.write_text(secrets.token_hex(24), encoding="utf-8")
    peer_secret.write_text(secrets.token_hex(24), encoding="utf-8")
    if os.name != "nt":
        os.chmod(source_secret, 0o600)
        os.chmod(peer_secret, 0o600)
    environment = dict(clean_environment(), PEERVAULT_PASSWORD="")
    restic_environment = dict(clean_environment(), RESTIC_PASSWORD_FILE=str(peer_secret))

    source_repository = arguments.source_root / "repository"
    result = run(binary, ["init", "--repository", str(source_repository), "--backend", backend, "--password-file", str(source_secret)], environment)
    assert result.returncode == 0, result.stderr
    result = run(binary, ["capture", "--repository", str(source_repository), "--backend", backend, "--source", str(source), "--tag", "peer-drill", "--password-file", str(source_secret)], environment)
    assert result.returncode == 0, result.stderr
    capture = json.loads(result.stdout)
    result = run(binary, ["history", "--repository", str(source_repository), "--backend", backend, "--password-file", str(source_secret)], environment)
    assert result.returncode == 0, result.stderr
    source_history = json.loads(result.stdout)
    assert len(source_history["snapshots"]) == 1
    capture_id = source_history["snapshots"][0]["capture_id"]
    assert capture_id

    record_path = arguments.recovery_root / "peer-copy-record.json"
    record = {
        "schema_version": 1,
        "record_id": secrets.token_hex(16),
        "capture_id": capture_id,
        "source_snapshot": capture["snapshot_id"],
        "peer_snapshot": None,
        "oracle_manifest": capture["manifest_snapshot_id"],
        "state": "requested",
        "transport": "local-filesystem restic copy; authenticated network transport not qualified",
        "key_role": "peer repository has its own password; source password was required only for the copy",
        "delete_behavior": "additive; no delete command issued",
        "source_absent": False,
        "steps": [],
        "limitations": [
            "same host; peer and recovery roots are not independent hardware",
            "no authenticated transport, pairing or keyless receiver",
            "no interrupted-transfer or replaced-mount fault injection",
            "author-only drill; no second operator",
        ],
        "_path": record_path,
    }
    step(record, "requested", "pass")

    peer_repository = arguments.peer_root / "repository"
    result = subprocess.run([backend, "--repo", str(peer_repository), "--no-cache", "init", "--repository-version=2"], capture_output=True, text=True, env=restic_environment)
    require(record, "peer-init", result)
    step(record, "transferring", "pass")
    result = subprocess.run(
        [backend, "--repo", str(peer_repository), "--no-cache", "copy", "--from-repo", str(source_repository), "--from-password-file", str(source_secret)],
        capture_output=True, text=True, env=restic_environment,
    )
    require(record, "transfer", result)
    config = subprocess.run([backend, "--repo", str(peer_repository), "--no-cache", "cat", "config"], capture_output=True, text=True, env=restic_environment)
    require(record, "peer-config", config)
    record["peer_repository_id"] = json.loads(config.stdout)["id"]
    step(record, "transfer-complete", "pass")

    remove_tree(source)
    remove_tree(source_repository)
    record["source_absent"] = True
    step(record, "source-absent", "pass")

    result = run(binary, ["history", "--repository", str(peer_repository), "--backend", backend, "--password-file", str(peer_secret)], environment)
    require(record, "peer-history", result)
    peer_history = json.loads(result.stdout)
    copied = [snapshot for snapshot in peer_history["snapshots"] if snapshot["capture_id"] == capture_id]
    assert len(copied) == 1
    record["peer_snapshot"] = copied[0]["id"]
    record["oracle_manifest"] = copied[0]["oracle_manifest"]
    result = run(binary, ["verify", "--repository", str(peer_repository), "--backend", backend, "--snapshot", copied[0]["id"], "--password-file", str(peer_secret)], environment)
    require(record, "full-read", result)
    step(record, "full-read-verified", "pass")

    destination = arguments.recovery_root / "restored"
    result = run(binary, ["restore", "--repository", str(peer_repository), "--backend", backend, "--snapshot", copied[0]["id"], "--destination", str(destination), "--password-file", str(peer_secret)], environment)
    require(record, "clean-restore", result)
    if tree(destination) != expected:
        step(record, "clean-restored-failed", "fail", "restored tree mismatch")
        raise SystemExit("restored tree mismatch")
    step(record, "clean-restored", "pass")
    print(json.dumps({key: value for key, value in record.items() if key != "_path"}, indent=2))


if __name__ == "__main__":
    try:
        main()
    except AssertionError as error:
        print(f"V0.3 peer copy failed: {error}", file=sys.stderr)
        raise SystemExit(1)
