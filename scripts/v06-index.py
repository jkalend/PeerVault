"""V0.6 backend index-loss/rebuild drill.

Removes one index file from an owned repository copy, runs the backend-native
rebuild-index, then verifies and restores the prior snapshot exactly.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess


def run(binary, args, timeout=900, env=None):
    environment = {
        name: os.environ[name]
        for name in ("PATH", "SystemRoot", "WINDIR", "TMP", "TEMP", "TMPDIR")
        if name in os.environ
    }
    if env:
        environment.update(env)
    return subprocess.run(
        [binary, *args], capture_output=True, text=True, env=environment, timeout=timeout
    )


def require(result, label):
    if result.returncode != 0:
        raise SystemExit(f"{label} failed ({result.returncode}): {(result.stderr or result.stdout)[:800]}")
    return result


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
    parser.add_argument("--root", type=Path, required=True, help="new drill root")
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    if root.exists():
        raise SystemExit(f"root must be new: {root}")
    root.mkdir(parents=True)
    binary, backend = str(arguments.binary.resolve()), str(arguments.backend.resolve())
    repository = root / "repository"
    damaged = root / "repository-damaged"
    source = root / "source"
    source.mkdir()
    (source / "hello.txt").write_text("index\n", encoding="utf-8")
    (source / "nested").mkdir()
    (source / "nested" / "data.bin").write_bytes(bytes(range(256)) * 256)
    secret = root / "secret.txt"
    secret.write_text(secrets.token_hex(24), encoding="utf-8")
    if os.name != "nt":
        os.chmod(secret, 0o600)

    def call(args):
        return run(binary, args)

    require(call(["init", "--repository", str(repository), "--backend", backend, "--password-file", str(secret)]), "init")
    baseline = json.loads(
        require(
            call(
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
                ]
            ),
            "baseline capture",
        ).stdout
    )
    baseline_tree = tree(source)
    shutil.copytree(repository, damaged)
    indexes = sorted((damaged / "index").glob("*"))
    assert indexes, "repository has no index files"
    indexes[0].unlink()
    rebuild = run(
        backend,
        ["--repo", str(damaged), "--no-cache", "rebuild-index"],
        env={"RESTIC_PASSWORD_FILE": str(secret)},
    )
    require(rebuild, "backend rebuild-index")
    require(
        call(
            [
                "verify",
                "--repository",
                str(damaged),
                "--backend",
                backend,
                "--password-file",
                str(secret),
            ]
        ),
        "verify after rebuild",
    )
    destination = root / "restored"
    require(
        call(
            [
                "restore",
                "--repository",
                str(damaged),
                "--backend",
                backend,
                "--snapshot",
                baseline["snapshot_id"],
                "--destination",
                str(destination),
                "--password-file",
                str(secret),
            ]
        ),
        "restore after rebuild",
    )
    assert tree(destination) == baseline_tree, "baseline did not restore exactly after index rebuild"
    report = {
        "result": "pass",
        "removed_index": indexes[0].name,
        "baseline_restored": True,
        "limitation": "backend-native rebuild-index on an owned repository copy; not physical index media failure",
    }
    (root / "v06-index.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
