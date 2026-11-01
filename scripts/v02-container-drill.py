"""V0.2 clean Linux recovery in a fresh, network-isolated container."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import sys

IMAGE = "ubuntu@sha256:33ceb71981b602c1a7443a53469e4dba065f7503eab3078a2d7a57a2ab987517"


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


def docker(args):
    return subprocess.run(["docker", *args], capture_output=True, text=True, timeout=1800)


def mount(source, target, readonly=False):
    return ["--mount", f"type=bind,source={Path(source).as_posix()},target={target}" + (",readonly" if readonly else "")]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True, help="host PeerVault CLI for setup")
    parser.add_argument("--backend", type=Path, required=True, help="host restic for setup/copy")
    parser.add_argument("--container-binary", type=Path, required=True, help="Linux PeerVault binary")
    parser.add_argument("--container-backend", type=Path, required=True, help="Linux restic binary")
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--copy-root", type=Path, required=True)
    parser.add_argument("--recovery-root", type=Path, required=True)
    parser.add_argument("--image", default=IMAGE)
    arguments = parser.parse_args()
    roots = [arguments.source_root.resolve(), arguments.copy_root.resolve(), arguments.recovery_root.resolve()]
    for left, right in ((0, 1), (0, 2), (1, 2)):
        if roots[left] == roots[right] or roots[left] in roots[right].parents or roots[right] in roots[left].parents:
            raise SystemExit("source, copy and recovery roots must be disjoint")
    for path in roots:
        if path.exists():
            raise SystemExit(f"root must be new: {path}")
        path.mkdir(parents=True)
    arguments.source_root, arguments.copy_root, arguments.recovery_root = roots
    if docker(["version"]).returncode != 0:
        raise SystemExit("docker is not available")
    source = arguments.source_root / "source"
    (source / "nested").mkdir(parents=True)
    (source / "empty.txt").write_bytes(b"")
    (source / "unicode-žluťoučký-雪.txt").write_text("unicode\n", encoding="utf-8")
    (source / "nested" / "data.bin").write_bytes(os.urandom(1024 * 1024))
    expected = tree(source)
    secret = arguments.recovery_root / "secret.txt"
    secret.write_text(secrets.token_hex(24), encoding="utf-8")
    if os.name != "nt":
        os.chmod(secret, 0o600)
    environment = dict(clean_environment(), PEERVAULT_PASSWORD="")
    binary, backend = os.path.abspath(arguments.binary), os.path.abspath(arguments.backend)
    source_repository = arguments.source_root / "repository"
    result = run(binary, ["init", "--repository", str(source_repository), "--backend", backend, "--password-file", str(secret)], environment)
    assert result.returncode == 0, result.stderr
    result = run(binary, ["capture", "--repository", str(source_repository), "--backend", backend, "--source", str(source), "--password-file", str(secret)], environment)
    assert result.returncode == 0, result.stderr
    copy_repository = arguments.copy_root / "repository"
    restic_environment = dict(clean_environment(), RESTIC_PASSWORD_FILE=str(secret))
    result = subprocess.run([backend, "--repo", str(copy_repository), "--no-cache", "init", "--repository-version=2"], capture_output=True, text=True, env=restic_environment)
    assert result.returncode == 0, result.stderr
    result = subprocess.run([backend, "--repo", str(copy_repository), "--no-cache", "copy", "--from-repo", str(source_repository), "--from-password-file", str(secret)], capture_output=True, text=True, env=restic_environment)
    assert result.returncode == 0, result.stderr
    result = run(binary, ["history", "--repository", str(copy_repository), "--backend", backend, "--password-file", str(secret)], environment)
    assert result.returncode == 0, result.stderr
    copied = json.loads(result.stdout)["snapshots"]
    assert len(copied) == 1 and copied[0]["capture_id"] and copied[0]["oracle_manifest"]
    remove_tree(source)
    remove_tree(source_repository)
    assert not source.exists() and not source_repository.exists()

    kit = arguments.recovery_root / "kit"
    restore_root = arguments.recovery_root / "restore"
    restore_root.mkdir()
    container_binary = os.path.abspath(arguments.container_binary)
    container_backend = os.path.abspath(arguments.container_backend)
    common = [
        "--rm", "--network", "none", "--read-only", "--tmpfs", "/tmp",
        *mount(arguments.copy_root / "repository", "/copy", True),
        *mount(container_binary, "/opt/peervault", True),
        *mount(container_backend, "/opt/restic", True),
    ]
    generate = docker([
        "run", *common, *mount(arguments.recovery_root, "/recovery"),
        arguments.image, "/opt/peervault", "recovery-kit", "generate",
        "--repository", "/copy", "--backend", "/opt/restic",
        "--snapshot", copied[0]["id"], "--kit", "/recovery/kit",
        "--password-file", "/recovery/secret.txt",
    ])
    assert generate.returncode == 0, generate.stderr
    assert kit.is_dir()

    verify_restore = docker([
        "run", *common, *mount(kit, "/kit", True), *mount(secret, "/secret.txt", True),
        *mount(restore_root, "/restore"),
        arguments.image, "/bin/sh", "-c",
        " && ".join([
            "/opt/peervault recovery-kit verify --kit /kit --repository /copy --backend /opt/restic --password-file /secret.txt",
            f"/opt/peervault restore --repository /copy --backend /opt/restic --snapshot {copied[0]['id']} --destination /restore/restored --password-file /secret.txt",
        ]),
    ])
    assert verify_restore.returncode == 0, verify_restore.stderr
    assert tree(restore_root / "restored") == expected
    print(json.dumps({
        "result": "pass",
        "scope": "fresh Ubuntu 24.04 container, network none, read-only root, source removed",
        "image": arguments.image,
        "capture_id": copied[0]["capture_id"],
        "copy_snapshot": copied[0]["id"],
        "restored_files": len(expected[0]),
        "limitations": [
            "same host kernel and Docker Desktop VM; not independent hardware",
            "copy and secret are host bind mounts, not separately observed media",
            "Windows clean-profile recovery remains open",
        ],
    }, indent=2))


if __name__ == "__main__":
    try:
        main()
    except AssertionError as error:
        print(f"V0.2 container drill failed: {error}", file=sys.stderr)
        raise SystemExit(1)
