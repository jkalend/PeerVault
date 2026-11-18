"""V0.6 previous-candidate upgrade/rollback rehearsal.

Creates a repository with the previous candidate binary, reads/restores it and
captures with the new binary, then reads/restores the new snapshot with the old
binary. This is a real repository-format/CLI compatibility check between two
commits; it is not a released-version upgrade or an irreversible backend
migration.
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old-binary", type=Path, required=True)
    parser.add_argument("--new-binary", type=Path, required=True)
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--root", type=Path, required=True, help="new drill root")
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    if root.exists():
        raise SystemExit(f"root must be new: {root}")
    root.mkdir(parents=True)
    old_binary = str(arguments.old_binary.resolve())
    new_binary = str(arguments.new_binary.resolve())
    backend = str(arguments.backend.resolve())
    repository = root / "repository"
    source = root / "source"
    source.mkdir()
    (source / "hello.txt").write_text("old candidate\n", encoding="utf-8")
    (source / "nested").mkdir()
    (source / "nested" / "data.bin").write_bytes(bytes(range(256)) * 128)
    secret = root / "secret.txt"
    secret.write_text(secrets.token_hex(24), encoding="utf-8")
    if os.name != "nt":
        os.chmod(secret, 0o600)

    def call(binary, args):
        return run(binary, args)

    require(
        call(
            old_binary,
            [
                "init",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--password-file",
                str(secret),
            ],
        ),
        "old init",
    )
    old_capture = json.loads(
        require(
            call(
                old_binary,
                [
                    "capture",
                    "--repository",
                    str(repository),
                    "--backend",
                    backend,
                    "--source",
                    str(source),
                    "--tag",
                    "old",
                    "--password-file",
                    str(secret),
                ],
            ),
            "old capture",
        ).stdout
    )
    new_history = json.loads(
        require(
            call(
                new_binary,
                [
                    "history",
                    "--repository",
                    str(repository),
                    "--backend",
                    backend,
                    "--password-file",
                    str(secret),
                ],
            ),
            "new history",
        ).stdout
    )
    assert any(snapshot["id"] == old_capture["snapshot_id"] for snapshot in new_history["snapshots"])
    require(
        call(
            new_binary,
            [
                "verify",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--password-file",
                str(secret),
            ],
        ),
        "new verify",
    )
    old_restore_new = root / "old-restored-by-new"
    require(
        call(
            new_binary,
            [
                "restore",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--snapshot",
                old_capture["snapshot_id"],
                "--destination",
                str(old_restore_new),
                "--password-file",
                str(secret),
            ],
        ),
        "new restore old snapshot",
    )
    assert tree(old_restore_new) == tree(source), "new binary did not restore the old snapshot"

    (source / "hello.txt").write_text("new candidate\n", encoding="utf-8")
    new_capture = json.loads(
        require(
            call(
                new_binary,
                [
                    "capture",
                    "--repository",
                    str(repository),
                    "--backend",
                    backend,
                    "--source",
                    str(source),
                    "--tag",
                    "new",
                    "--password-file",
                    str(secret),
                ],
            ),
            "new capture",
        ).stdout
    )
    rollback_restore = root / "new-restored-by-old"
    require(
        call(
            old_binary,
            [
                "restore",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--snapshot",
                new_capture["snapshot_id"],
                "--destination",
                str(rollback_restore),
                "--password-file",
                str(secret),
            ],
        ),
        "old restore new snapshot",
    )
    assert tree(rollback_restore) == tree(source), "old binary did not restore the new snapshot"
    old_history = json.loads(
        require(
            call(
                old_binary,
                [
                    "history",
                    "--repository",
                    str(repository),
                    "--backend",
                    backend,
                    "--password-file",
                    str(secret),
                ],
            ),
            "old history",
        ).stdout
    )
    assert len(old_history["snapshots"]) == 2
    print(
        json.dumps(
            {
                "result": "pass",
                "old_candidate_snapshot": old_capture["snapshot_id"],
                "new_candidate_snapshot": new_capture["snapshot_id"],
                "new_reads_old": True,
                "old_reads_new": True,
                "limitation": "previous commit, not a released version; no backend format migration",
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
