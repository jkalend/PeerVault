"""V0.6 prerequisite smoke: 5 GiB streamed file plus 50,000 small files."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import secrets
import subprocess
import sys
import time

MIB = 1024 * 1024
LARGE_BYTES = 5 * 1024 * MIB
SMALL_FILES = 50_000
SMALL_DIRECTORIES = 50
SMALL_BYTES = 128
TIME_LIMIT_SECONDS = 900
REPOSITORY_LIMIT_BYTES = 8 * 1024 * MIB


def stream_hash(path):
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        while chunk := stream.read(4 * MIB):
            digest.update(chunk)
            size += len(chunk)
    return size, digest.hexdigest()


def tree(root):
    files = {}
    directories = set()
    for path in root.rglob("*"):
        relative = path.relative_to(root).as_posix()
        if path.is_dir():
            directories.add(relative)
        elif path.is_file():
            files[relative] = stream_hash(path)
    return files, directories


def directory_bytes(root):
    return sum(path.stat().st_size for path in root.rglob("*") if path.is_file())


def run(binary, args, environment):
    return subprocess.run([binary, *args], capture_output=True, text=True, env=environment, timeout=1800)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, required=True, help="new source/repository root")
    parser.add_argument("--restore-root", type=Path, required=True, help="new restore root")
    arguments = parser.parse_args()
    if arguments.source_root.exists() or arguments.restore_root.exists():
        raise SystemExit("both roots must be new")
    arguments.source_root.mkdir(parents=True)
    arguments.restore_root.mkdir(parents=True)
    source = arguments.source_root / "source"
    source.mkdir()
    started = time.time()
    large = source / "large-5g.bin"
    block = b"\0" * MIB
    with large.open("wb") as stream:
        for _ in range(5 * 1024):
            stream.write(block)
    generator = random.Random(0x50454552)
    bulk = source / "bulk"
    for directory in range(SMALL_DIRECTORIES):
        target = bulk / f"d{directory:02}"
        target.mkdir(parents=True)
        for index in range(SMALL_FILES // SMALL_DIRECTORIES):
            (target / f"f{index:04}.bin").write_bytes(generator.randbytes(SMALL_BYTES))
    expected = tree(source)
    assert len(expected[0]) == SMALL_FILES + 1
    secret = arguments.source_root / "secret.txt"
    secret.write_text(secrets.token_hex(24), encoding="utf-8")
    environment = dict(os.environ, PEERVAULT_PASSWORD="")
    binary, backend = os.path.abspath(arguments.binary), os.path.abspath(arguments.backend)
    repository = arguments.source_root / "repository"
    result = run(binary, ["init", "--repository", str(repository), "--backend", backend, "--password-file", str(secret)], environment)
    assert result.returncode == 0, result.stderr
    result = run(binary, ["capture", "--repository", str(repository), "--backend", backend, "--source", str(source), "--tag", "v06-corpus", "--password-file", str(secret)], environment)
    assert result.returncode == 0, result.stderr
    capture = json.loads(result.stdout)
    result = run(binary, ["verify", "--repository", str(repository), "--backend", backend, "--password-file", str(secret)], environment)
    assert result.returncode == 0, result.stderr
    destination = arguments.restore_root / "restored"
    result = run(binary, ["restore", "--repository", str(repository), "--backend", backend, "--snapshot", capture["snapshot_id"], "--destination", str(destination), "--password-file", str(secret)], environment)
    assert result.returncode == 0, result.stderr
    observed = tree(destination)
    assert observed == expected
    elapsed = time.time() - started
    repository_bytes = directory_bytes(repository)
    assert elapsed <= TIME_LIMIT_SECONDS, f"corpus smoke exceeded {TIME_LIMIT_SECONDS} s"
    assert repository_bytes <= REPOSITORY_LIMIT_BYTES, "repository exceeded declared limit"
    print(json.dumps({
        "result": "pass",
        "files": len(expected[0]),
        "directories": len(expected[1]),
        "bytes": sum(size for size, _ in expected[0].values()),
        "large_file_bytes": LARGE_BYTES,
        "small_files": SMALL_FILES,
        "elapsed_seconds": round(elapsed, 1),
        "repository_bytes": repository_bytes,
        "limits": {"time_seconds": TIME_LIMIT_SECONDS, "repository_bytes": REPOSITORY_LIMIT_BYTES, "rss": "not measured"},
        "limitations": ["synthetic zero-filled 5 GiB file", "single host", "no power-loss or metadata matrix"],
    }, indent=2))


if __name__ == "__main__":
    try:
        main()
    except AssertionError as error:
        print(f"V0.6 corpus smoke failed: {error}", file=sys.stderr)
        raise SystemExit(1)
