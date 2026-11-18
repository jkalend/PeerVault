"""V0.6 portable-subset cross-OS restore drill through WSL.

Windows captures, Linux restores; Linux captures, Windows restores. Both trees are
compared to the same independent SHA-256 oracle. Same host, so this is not
independent hardware or a clean-host profile.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shlex
import subprocess


def run(command, timeout=1800):
    return subprocess.run(command, capture_output=True, text=True, timeout=timeout)


def require(result, label):
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()[:1000]
        raise SystemExit(f"{label} failed ({result.returncode}): {detail}")
    return result


def wsl_path(path):
    resolved = str(Path(path).resolve())
    if len(resolved) < 3 or resolved[1] != ":":
        raise SystemExit(f"not an absolute Windows path: {resolved}")
    return f"/mnt/{resolved[0].lower()}{resolved[2:].replace(chr(92), '/')}"


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
    parser.add_argument("--windows-binary", type=Path, required=True)
    parser.add_argument("--windows-backend", type=Path, required=True)
    parser.add_argument("--linux-binary", type=Path, required=True)
    parser.add_argument("--linux-backend", type=Path, required=True)
    parser.add_argument("--root", type=Path, required=True, help="new drill root")
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    if root.exists():
        raise SystemExit(f"root must be new: {root}")
    root.mkdir(parents=True)
    windows_binary = str(arguments.windows_binary.resolve())
    windows_backend = str(arguments.windows_backend.resolve())
    linux_binary = wsl_path(arguments.linux_binary.resolve())
    linux_backend = wsl_path(arguments.linux_backend.resolve())
    source = root / "source"
    source.mkdir()
    (source / "ascii.txt").write_text("cross-os\n", encoding="utf-8")
    (source / "unicode-žluťoučký-雪.txt").write_text("unicode\n", encoding="utf-8")
    (source / "composed-é.bin").write_bytes(b"composed\n")
    (source / "decomposed-e\u0301.bin").write_bytes(b"decomposed\n")
    (source / "empty.txt").write_bytes(b"")
    nested = source / "nested" / "inner"
    nested.mkdir(parents=True)
    (nested / "file.bin").write_bytes(bytes(range(256)) * 64)
    expected = tree(source)
    secret = root / "secret.txt"
    secret.write_text(secrets.token_hex(24), encoding="utf-8")
    if os.name != "nt":
        os.chmod(secret, 0o600)

    def windows(args):
        return run([windows_binary, *args])

    def linux(args):
        command = " ".join(shlex.quote(part) for part in [linux_binary, *args])
        return run(["wsl", "-e", "bash", "-lc", command])

    # Windows capture -> Linux restore.
    windows_repository = root / "windows-repository"
    require(
        windows(
            [
                "init",
                "--repository",
                str(windows_repository),
                "--backend",
                windows_backend,
                "--password-file",
                str(secret),
            ]
        ),
        "Windows init",
    )
    captured = require(
        windows(
            [
                "capture",
                "--repository",
                str(windows_repository),
                "--backend",
                windows_backend,
                "--source",
                str(source),
                "--password-file",
                str(secret),
            ]
        ),
        "Windows capture",
    )
    windows_snapshot = json.loads(captured.stdout)["snapshot_id"]
    linux_restore = root / "restored-on-linux"
    require(
        linux(
            [
                "restore",
                "--repository",
                wsl_path(windows_repository),
                "--backend",
                linux_backend,
                "--snapshot",
                windows_snapshot,
                "--destination",
                wsl_path(linux_restore),
                "--password-file",
                wsl_path(secret),
            ]
        ),
        "Linux restore",
    )
    assert tree(linux_restore) == expected, "Linux restore did not match the Windows oracle"

    # Linux capture -> Windows restore.
    linux_repository = root / "linux-repository"
    require(
        linux(
            [
                "init",
                "--repository",
                wsl_path(linux_repository),
                "--backend",
                linux_backend,
                "--password-file",
                wsl_path(secret),
            ]
        ),
        "Linux init",
    )
    linux_captured = require(
        linux(
            [
                "capture",
                "--repository",
                wsl_path(linux_repository),
                "--backend",
                linux_backend,
                "--source",
                wsl_path(source),
                "--password-file",
                wsl_path(secret),
            ]
        ),
        "Linux capture",
    )
    linux_snapshot = json.loads(linux_captured.stdout)["snapshot_id"]
    windows_restore = root / "restored-on-windows"
    require(
        windows(
            [
                "restore",
                "--repository",
                str(linux_repository),
                "--backend",
                windows_backend,
                "--snapshot",
                linux_snapshot,
                "--destination",
                str(windows_restore),
                "--password-file",
                str(secret),
            ]
        ),
        "Windows restore",
    )
    assert tree(windows_restore) == expected, "Windows restore did not match the Linux oracle"
    print(
        json.dumps(
            {
                "result": "pass",
                "windows_capture_to_linux_restore": True,
                "linux_capture_to_windows_restore": True,
                "files": len(expected[0]),
                "directories": len(expected[1]),
                "limitation": "same host through WSL2; not independent hardware",
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
