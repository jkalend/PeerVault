"""V0.6 filesystem-capacity drill on a bounded repository filesystem.

Run the repository on a size-limited tmpfs/loop volume, fill it with an
incompressible source addition, and verify the failed capture leaves the prior
snapshot restorable. This is a real ENOSPC test, but tmpfs/loop storage is not a
physical-disk or power-loss qualification.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess


def run(binary, args, timeout=1800):
    environment = {
        name: os.environ[name]
        for name in ("PATH", "SystemRoot", "WINDIR", "TMP", "TEMP", "TMPDIR")
        if name in os.environ
    }
    return subprocess.run(
        [binary, *args],
        capture_output=True,
        text=True,
        env=environment,
        timeout=timeout,
    )


def require(result, label):
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()[:800]
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
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--restore-root", type=Path, required=True)
    parser.add_argument("--fill-mib", type=int, default=128)
    arguments = parser.parse_args()
    for path in (arguments.repository_root, arguments.source_root, arguments.restore_root):
        if path.exists():
            raise SystemExit(f"path must be new: {path}")
        path.mkdir(parents=True)
    binary, backend = str(arguments.binary.resolve()), str(arguments.backend.resolve())
    repository = arguments.repository_root / "repository"
    source = arguments.source_root / "source"
    source.mkdir()
    (source / "baseline.txt").write_text("baseline\n", encoding="utf-8")
    baseline_tree = tree(source)
    secret = arguments.source_root / "secret.txt"
    secret.write_text(secrets.token_hex(24), encoding="utf-8")
    os.chmod(secret, 0o600)

    def call(args):
        return run(binary, args)

    require(
        call(
            [
                "init",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--password-file",
                str(secret),
            ]
        ),
        "init",
    )
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
    (source / "fill.bin").write_bytes(os.urandom(arguments.fill_mib * 1024 * 1024))
    failed = call(
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
    )
    assert failed.returncode != 0, "capture on the bounded filesystem unexpectedly succeeded"
    history = json.loads(
        require(
            call(
                [
                    "history",
                    "--repository",
                    str(repository),
                    "--backend",
                    backend,
                    "--password-file",
                    str(secret),
                ]
            ),
            "history",
        ).stdout
    )
    assert any(snapshot["id"] == baseline["snapshot_id"] for snapshot in history["snapshots"])
    destination = arguments.restore_root / "restored"
    require(
        call(
            [
                "restore",
                "--repository",
                str(repository),
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
        "baseline restore after ENOSPC",
    )
    assert tree(destination) == baseline_tree, "baseline did not restore exactly after ENOSPC"
    report = {
        "result": "pass",
        "fill_mib": arguments.fill_mib,
        "failed_capture_exit": failed.returncode,
        "baseline_restored": True,
        "limitation": "bounded tmpfs/loop filesystem; not physical media or power loss",
    }
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
