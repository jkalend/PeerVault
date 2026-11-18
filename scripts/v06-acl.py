"""V0.6 Windows ACL denial drill.

Applies a deny read/execute ACE to a source subdirectory, proves capture fails,
restores the prior snapshot, then removes the deny ACE. Linux non-root denial is
covered separately by v06-denied.py.
"""

import argparse
import getpass
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess


def run(command, timeout=600):
    return subprocess.run(command, capture_output=True, text=True, timeout=timeout)


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
    if os.name != "nt":
        print(json.dumps({"result": "skipped", "reason": "Windows ACL drill requires Windows"}))
        return
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
    source = root / "source"
    source.mkdir()
    closed = source / "closed"
    closed.mkdir()
    (closed / "inside.txt").write_text("closed\n", encoding="utf-8")
    (source / "open.txt").write_text("open\n", encoding="utf-8")
    secret = root / "secret.txt"
    secret.write_text(secrets.token_hex(24), encoding="utf-8")
    identity = f"{os.environ.get('USERDOMAIN', '')}\\{getpass.getuser()}"

    def call(args):
        return run([binary, *args])

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
    try:
        require(run(["icacls", str(closed), "/deny", f"{identity}:(OI)(CI)(RX)"]), "deny ACL")
        denied = call(
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
        assert denied.returncode != 0, "capture succeeded despite a deny ACL"
    finally:
        run(["icacls", str(closed), "/remove:d", identity])
    history = json.loads(
        require(
            call(["history", "--repository", str(repository), "--backend", backend, "--password-file", str(secret)]),
            "history",
        ).stdout
    )
    assert any(snapshot["id"] == baseline["snapshot_id"] for snapshot in history["snapshots"])
    destination = root / "restored"
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
        "baseline restore after denial",
    )
    assert tree(destination) == baseline_tree, "baseline did not restore exactly after denial"
    report = {
        "result": "pass",
        "denied_capture_exit": denied.returncode,
        "baseline_restored": True,
        "limitation": "Windows deny ACE for the invoking user; physical media failure remains separate",
    }
    (root / "v06-acl.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
