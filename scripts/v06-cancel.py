"""V0.6 cancellation and operation-serialization drill.

Starts a large capture, proves a second mutating/reading PeerVault operation gets
a busy refusal, interrupts the capture, then verifies the prior snapshot still
restores exactly. Partial backend snapshots without an oracle are allowed and
must be identifiable as incomplete. Process-kill behavior is not power loss.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import signal
import subprocess
import sys
import time


def environment():
    return {
        name: os.environ[name]
        for name in ("PATH", "SystemRoot", "WINDIR", "TMP", "TEMP", "TMPDIR")
        if name in os.environ
    }


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
    secret = root / "repository-secret.txt"
    secret.write_text(secrets.token_hex(24), encoding="utf-8")
    if os.name != "nt":
        os.chmod(secret, 0o600)

    def call(args):
        return run(binary, args)

    (source / "baseline.txt").write_text("baseline\n", encoding="utf-8")
    baseline_tree = tree(source)
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
                    "--tag",
                    "baseline",
                    "--password-file",
                    str(secret),
                ]
            ),
            "baseline capture",
        ).stdout
    )

    large = source / "large-sparse.bin"
    with large.open("wb") as stream:
        stream.truncate(2 * 1024 * 1024 * 1024)
    arguments = [
        binary,
        "capture",
        "--repository",
        str(repository),
        "--backend",
        backend,
        "--source",
        str(source),
        "--tag",
        "interrupted",
        "--password-file",
        str(secret),
    ]
    flags = 0
    if os.name == "nt":
        flags = subprocess.CREATE_NEW_PROCESS_GROUP
    child = subprocess.Popen(
        arguments,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=environment(),
        creationflags=flags,
    )
    time.sleep(1.0)
    if child.poll() is not None:
        raise SystemExit("capture finished before cancellation; enlarge the probe")
    second = call(
        [
            "capture",
            "--repository",
            str(repository),
            "--backend",
            backend,
            "--source",
            str(source),
            "--tag",
            "second",
            "--password-file",
            str(secret),
        ]
    )
    assert second.returncode == 5, (
        "second capture was not refused as busy",
        second.returncode,
        second.stderr,
    )
    check = call(
        [
            "verify",
            "--repository",
            str(repository),
            "--backend",
            backend,
            "--password-file",
            str(secret),
        ]
    )
    assert check.returncode == 5, (
        "verification was not refused as busy during capture",
        check.returncode,
        check.stderr,
    )
    if os.name == "nt":
        child.send_signal(signal.CTRL_BREAK_EVENT)
    else:
        child.send_signal(signal.SIGINT)
    try:
        stdout, stderr = child.communicate(timeout=120)
    except subprocess.TimeoutExpired:
        child.kill()
        stdout, stderr = child.communicate()
        raise SystemExit("interrupted capture did not exit after cancellation")
    assert child.returncode != 0, "interrupted capture reported success"
    if child.returncode not in (130, -signal.SIGINT):
        # Windows can report the native status; keep the check informative.
        print(f"capture exit status: {child.returncode}", file=sys.stderr)

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
            "post-cancel history",
        ).stdout
    )
    assert any(snapshot["id"] == baseline["snapshot_id"] for snapshot in history["snapshots"])
    for snapshot in history["snapshots"]:
        if snapshot["id"] == baseline["snapshot_id"]:
            assert snapshot["oracle_manifest"]
        else:
            assert snapshot["oracle_manifest"] is None, (
                "interrupted capture published a complete oracle",
                snapshot,
            )
    require(
        call(
            [
                "verify",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--password-file",
                str(secret),
            ]
        ),
        "post-cancel verify",
    )
    destination = root / "restored-baseline"
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
        "post-cancel restore",
    )
    assert tree(destination) == baseline_tree, "baseline did not restore after cancellation"
    print(
        json.dumps(
            {
                "result": "pass",
                "cancel_exit": child.returncode,
                "snapshots": len(history["snapshots"]),
                "incomplete_snapshots": sum(
                    1
                    for snapshot in history["snapshots"]
                    if snapshot["oracle_manifest"] is None
                ),
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
