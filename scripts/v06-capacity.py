"""V0.6 capacity/file-size exhaustion drill (Unix only).

Runs capture and restore under a bounded RLIMIT_FSIZE so a child cannot write an
unbounded file. This is a deterministic quota-style exhaustion test, not a
physical disk-full or power-loss test.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess
import sys

try:
    import resource
except ImportError:  # Windows: the drill is skipped before using it.
    resource = None


def environment():
    return {
        name: os.environ[name]
        for name in ("PATH", "SystemRoot", "WINDIR", "TMP", "TEMP", "TMPDIR")
        if name in os.environ
    }


def limited_run(binary, args, limit_bytes, timeout=600):
    def limit():
        resource.setrlimit(resource.RLIMIT_FSIZE, (limit_bytes, limit_bytes))

    return subprocess.run(
        [binary, *args],
        capture_output=True,
        text=True,
        env=environment(),
        preexec_fn=limit,
        timeout=timeout,
    )


def run(binary, args, timeout=600):
    return subprocess.run(
        [binary, *args],
        capture_output=True,
        text=True,
        env=environment(),
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
    if os.name != "posix" or resource is None:
        print(json.dumps({"result": "skipped", "reason": "RLIMIT_FSIZE drill requires Unix"}))
        return
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--root", type=Path, required=True, help="new drill root")
    parser.add_argument("--limit-bytes", type=int, default=256 * 1024)
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    if root.exists():
        raise SystemExit(f"root must be new: {root}")
    root.mkdir(parents=True)
    binary, backend = str(arguments.binary.resolve()), str(arguments.backend.resolve())
    repository = root / "repository"
    source = root / "source"
    source.mkdir()
    (source / "small.txt").write_text("baseline\n", encoding="utf-8")
    (source / "large.bin").write_bytes(os.urandom(8 * 1024 * 1024))
    secret = root / "secret.txt"
    secret.write_text(secrets.token_hex(24), encoding="utf-8")
    os.chmod(secret, 0o600)

    def call(args):
        return run(binary, args)

    require(
        call(["init", "--repository", str(repository), "--backend", backend, "--password-file", str(secret)]),
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
    baseline_tree = tree(source)
    (source / "new-large.bin").write_bytes(os.urandom(8 * 1024 * 1024))
    failed = limited_run(
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
        arguments.limit_bytes,
    )
    assert failed.returncode != 0, "limited capture unexpectedly succeeded"
    history = json.loads(
        require(
            call(["history", "--repository", str(repository), "--backend", backend, "--password-file", str(secret)]),
            "history",
        ).stdout
    )
    assert any(snapshot["id"] == baseline["snapshot_id"] for snapshot in history["snapshots"]), history
    restore = root / "restored"
    limited_restore = limited_run(
        binary,
        [
            "restore",
            "--repository",
            str(repository),
            "--backend",
            backend,
            "--snapshot",
            baseline["snapshot_id"],
            "--destination",
            str(restore),
            "--password-file",
            str(secret),
        ],
        arguments.limit_bytes,
    )
    if limited_restore.returncode == 0:
        assert tree(restore) == baseline_tree, "limited restore produced a different tree"
        restore_result = "content-below-limit"
    else:
        assert not restore.exists() or restore.is_dir(), "failed restore left a non-directory"
        restore_result = "failed-and-no-complete-target"
    full_restore = root / "full-restored"
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
                str(full_restore),
                "--password-file",
                str(secret),
            ]
        ),
        "full restore after exhaustion",
    )
    assert tree(full_restore) == baseline_tree, "baseline did not restore after capacity exhaustion"
    report = {
        "result": "pass",
        "limit_bytes": arguments.limit_bytes,
        "limited_capture_exit": failed.returncode,
        "limited_restore": restore_result,
        "baseline_restored": True,
        "limitation": "RLIMIT_FSIZE quota simulation, not physical disk-full or power loss",
    }
    (root / "v06-capacity.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
