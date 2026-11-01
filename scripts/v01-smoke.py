"""End-to-end V0.1 local capture/history/verify/restore smoke on the selected backend."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import stat
import subprocess
import sys

PASSWORD = secrets.token_hex(16)


def run(binary, args, password=PASSWORD):
    environment = dict(os.environ, PEERVAULT_PASSWORD=password)
    return subprocess.run([binary, *args], capture_output=True, text=True, env=environment, timeout=900)


def tree(root):
    files, directories = {}, set()
    for path in root.rglob("*"):
        relative = path.relative_to(root).as_posix()
        if path.is_dir():
            directories.add(relative)
        elif path.is_file():
            files[relative] = (path.stat().st_size, hashlib.sha256(path.read_bytes()).hexdigest())
    return files, directories


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--root", type=Path, required=True, help="new disposable work root")
    arguments = parser.parse_args()
    binary, backend, root = (
        os.path.abspath(arguments.binary),
        os.path.abspath(arguments.backend),
        Path(os.path.abspath(arguments.root)),
    )
    if root.exists():
        raise SystemExit(f"work root must be new: {root}")
    root.mkdir(parents=True)
    source = root / "source"
    (source / "nested").mkdir(parents=True)
    (source / "empty-directory").mkdir()
    (source / "empty.txt").write_bytes(b"")
    (source / "unicode-žluťoučký-雪.txt").write_text("unicode\n", encoding="utf-8")
    (source / "readonly.bin").write_bytes(b"readonly")
    os.chmod(source / "readonly.bin", 0o444)
    (source / "nested" / "data.bin").write_bytes(os.urandom(1024 * 1024))
    first_tree = tree(source)
    repository = root / "repository"

    result = run(binary, ["init", "--repository", str(repository), "--backend", backend])
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout)["schema_version"] == 2
    result = run(binary, ["capture", "--repository", str(repository), "--backend", backend, "--source", str(source), "--tag", "smoke-a"])
    assert result.returncode == 0, result.stderr
    first = json.loads(result.stdout)

    (source / "added.txt").write_text("added\n", encoding="utf-8")
    (source / "empty.txt").write_bytes(b"changed\n")
    second_tree = tree(source)
    result = run(binary, ["capture", "--repository", str(repository), "--backend", backend, "--source", str(source), "--tag", "smoke-b"])
    assert result.returncode == 0, result.stderr
    second = json.loads(result.stdout)
    assert first["snapshot_id"] != second["snapshot_id"]

    result = run(binary, ["history", "--repository", str(repository), "--backend", backend])
    assert result.returncode == 0, result.stderr
    history = json.loads(result.stdout)
    assert len(history["snapshots"]) == 2
    assert all(snapshot["oracle_manifest"] for snapshot in history["snapshots"])

    result = run(binary, ["verify", "--repository", str(repository), "--backend", backend])
    assert result.returncode == 0, result.stderr

    known = {snapshot["id"]: snapshot for snapshot in history["snapshots"]}
    assert first["snapshot_id"] in known and second["snapshot_id"] in known
    assert known[first["snapshot_id"]]["oracle_manifest"]
    assert known[second["snapshot_id"]]["oracle_manifest"]
    for index, (expected, snapshot_id) in enumerate(((first_tree, first["snapshot_id"]), (second_tree, second["snapshot_id"]))):
        destination = root / f"restored-{index}"
        result = run(binary, ["restore", "--repository", str(repository), "--backend", backend, "--snapshot", snapshot_id, "--destination", str(destination)])
        assert result.returncode == 0, result.stderr
        report = json.loads(result.stdout)
        assert report["verification"] == "manifest-hash"
        assert (tree(destination)) == expected
        if os.name == "nt":
            attributes = (destination / "readonly.bin").stat().st_file_attributes
            assert attributes & stat.FILE_ATTRIBUTE_READONLY
        else:
            assert (destination / "readonly.bin").stat().st_mode & 0o200 == 0
    if os.name == "nt":
        (root / "restored-1" / "readonly.bin").chmod(0o666)

    original = os.getcwd()
    os.chdir(root)
    try:
        result = run(binary, ["restore", "--repository", str(repository), "--backend", backend, "--snapshot", second["snapshot_id"], "--destination", "relative-restored"])
    finally:
        os.chdir(original)
    assert result.returncode == 0, result.stderr
    assert tree(root / "relative-restored") == second_tree
    if os.name == "nt":
        (root / "relative-restored" / "readonly.bin").chmod(0o666)

    result = run(binary, ["restore", "--repository", str(repository), "--backend", backend, "--snapshot", second["snapshot_id"], "--destination", str(root / "restored-1")])
    assert result.returncode == 3

    empty = root / "empty-source"
    empty.mkdir()
    result = run(binary, ["capture", "--repository", str(repository), "--backend", backend, "--source", str(empty)])
    assert result.returncode == 4
    assert json.loads(result.stdout)["result"] == "empty"

    result = run(binary, ["history", "--repository", str(repository), "--backend", backend], password="wrong-password")
    assert result.returncode == 3

    corrupt = root / "repository-corrupt"
    shutil.copytree(repository, corrupt)
    objects = [path for path in (corrupt / "data").rglob("*") if path.is_file()]
    assert objects
    target = max(objects, key=lambda path: path.stat().st_size)
    target.chmod(0o644)
    with target.open("r+b") as stream:
        stream.truncate(max(1, target.stat().st_size // 2))
    result = run(binary, ["verify", "--repository", str(corrupt), "--backend", backend])
    assert result.returncode == 3
    print("V0.1 smoke passed: two exact restores, oracle, rejection and corruption paths")


if __name__ == "__main__":
    try:
        main()
    except AssertionError as error:
        print(f"V0.1 smoke failed: {error}", file=sys.stderr)
        raise SystemExit(1)
