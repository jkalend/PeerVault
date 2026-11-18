"""V0.6 metadata/path capability matrix drill.

Runs the supported regular-file/directory subset, then hostile/unsupported source
entries and metadata. Records supported, rejected, lossy and skipped cases in
v06-matrix.json. Same-host synthetic evidence; not a power-loss or independent-
hardware qualification.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import stat
import subprocess
import sys
import time


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


def tree(root, skip=()):
    files, directories = {}, set()
    for path in root.rglob("*"):
        relative = path.relative_to(root).as_posix()
        if relative in skip or any(relative.startswith(prefix + "/") for prefix in skip):
            continue
        if path.is_symlink() or not path.is_file() and not path.is_dir():
            continue
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
    report = {
        "schema_version": 1,
        "platform": sys.platform,
        "rows": [],
        "limitations": [
            "same-host synthetic drill",
            "no application consistency, VSS, cloud/network roots or removable media",
            "no power-loss, disk-full or clock injection",
        ],
    }

    def row(case, result, detail):
        report["rows"].append({"case": case, "result": result, "detail": detail})

    def call(args):
        return run(binary, args)

    def capture(tag):
        return call(
            [
                "capture",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--source",
                str(source),
                "--tag",
                tag,
                "--password-file",
                str(secret),
            ]
        )

    (source / "ascii.txt").write_text("ascii\n", encoding="utf-8")
    (source / "unicode-žluťoučký-雪.txt").write_text("unicode\n", encoding="utf-8")
    (source / "composed-é.bin").write_bytes(b"composed\n")
    (source / "decomposed-e\u0301.bin").write_bytes(b"decomposed\n")
    (source / "empty.txt").write_bytes(b"")
    (source / "large.bin").write_bytes(bytes(range(256)) * 8192)
    (source / "readonly.bin").write_bytes(b"readonly\n")
    os.chmod(source / "readonly.bin", 0o444)
    (source / "nested" / "inner").mkdir(parents=True)
    (source / "nested" / "inner" / "file.txt").write_text("nested\n", encoding="utf-8")
    try:
        os.link(source / "ascii.txt", source / "hardlink.txt")
        row("hard-link", "lossy", "content captured twice; link identity not preserved")
    except OSError as error:
        row("hard-link", "skipped", str(error))
    long_root = source / "long" / ("a" * 80) / ("b" * 80)
    try:
        long_root.mkdir(parents=True)
        (long_root / ("c" * 80 + ".txt")).write_text("long\n", encoding="utf-8")
        row("long-path", "supported", "created and captured below the 1024-byte policy limit")
    except OSError as error:
        row("long-path", "skipped", f"host cannot create the probe: {error}")

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
    captured = capture("supported")
    if captured.returncode != 0:
        row(
            "long-path",
            "unsupported",
            "host created the path but capture refused it: "
            + (captured.stderr or captured.stdout).strip()[:300],
        )
        # Remove the probe and retry the supported subset.
        import shutil

        shutil.rmtree(source / "long", ignore_errors=True)
        captured = require(capture("supported"), "supported capture")
    first = json.loads(captured.stdout)
    report["capture"] = first
    row(
        "regular-files-directories-unicode",
        "supported",
        f"{first['files']} files, {first['directories']} directories",
    )

    destination = root / "restored-supported"
    restored = require(
        call(
            [
                "restore",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--snapshot",
                first["snapshot_id"],
                "--destination",
                str(destination),
                "--password-file",
                str(secret),
            ]
        ),
        "supported restore",
    )
    report["restore"] = json.loads(restored.stdout)
    expected = tree(source)
    assert tree(destination) == expected, "supported subset did not restore exactly"
    if os.name == "nt":
        attributes = (destination / "readonly.bin").stat().st_file_attributes
        assert attributes & stat.FILE_ATTRIBUTE_READONLY
    else:
        assert (destination / "readonly.bin").stat().st_mode & 0o200 == 0
    row("readonly-bit", "supported", "restored on the owned file")

    # Metadata that is outside the declared oracle scope.
    timestamp = time.mktime((2000, 1, 1, 0, 0, 0, 0, 0, -1))
    os.utime(source / "ascii.txt", (timestamp, timestamp))
    ads = source / "ascii.txt:peervault"
    if os.name == "nt":
        try:
            ads.write_text("alternate-stream\n", encoding="utf-8")
            row("ntfs-ads", "lossy", "stream not captured or restored")
        except OSError as error:
            row("ntfs-ads", "skipped", str(error))
    else:
        row("ntfs-ads", "skipped", "not applicable")
    if hasattr(os, "setxattr") and sys.platform.startswith("linux"):
        try:
            os.setxattr(source / "ascii.txt", "user.peervault", b"x")
            row("posix-xattr", "lossy", "xattr not captured or restored")
        except OSError as error:
            row("posix-xattr", "skipped", str(error))
    else:
        row("posix-xattr", "skipped", "not applicable")
    metadata_capture = require(capture("metadata"), "metadata capture")
    metadata = json.loads(metadata_capture.stdout)
    metadata_restore = root / "restored-metadata"
    require(
        call(
            [
                "restore",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--snapshot",
                metadata["snapshot_id"],
                "--destination",
                str(metadata_restore),
                "--password-file",
                str(secret),
            ]
        ),
        "metadata restore",
    )
    assert tree(metadata_restore) == tree(source), "content changed under metadata-only edits"
    if os.name == "nt" and ads.exists():
        assert not (metadata_restore / "ascii.txt:peervault").exists()
    if hasattr(os, "getxattr") and sys.platform.startswith("linux"):
        try:
            os.getxattr(metadata_restore / "ascii.txt", "user.peervault")
            raise AssertionError("xattr survived restore")
        except OSError:
            pass
    assert (metadata_restore / "ascii.txt").stat().st_mtime != timestamp
    row("timestamps", "lossy", "mtime is not part of the restore oracle")

    history_count = len(
        json.loads(
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
        )["snapshots"]
    )

    link = source / "symlink-to-ascii"
    try:
        os.symlink("ascii.txt", link)
        refused = capture("symlink")
        assert refused.returncode != 0, "symlink source was not refused"
        assert "links are unsupported" in (refused.stderr + refused.stdout)
        row("symlink-reparse", "rejected", "capture refuses links before publication")
        link.unlink()
    except OSError as error:
        row("symlink-reparse", "skipped", f"host cannot create the probe: {error}")

    if os.name != "nt":
        fifo = source / "fifo"
        os.mkfifo(fifo)
        refused = capture("special")
        assert refused.returncode != 0, "special file source was not refused"
        assert "special files are unsupported" in (refused.stderr + refused.stdout)
        row("special-file", "rejected", "capture refuses FIFOs before publication")
        fifo.unlink()
        (source / "Case.txt").write_text("upper\n", encoding="utf-8")
        (source / "case.txt").write_text("lower\n", encoding="utf-8")
        refused = capture("case-collision")
        assert refused.returncode != 0, "case collision was not refused"
        assert "colliding path" in (refused.stderr + refused.stdout)
        row("case-collision", "rejected", "case-insensitive collision refused")
        (source / "Case.txt").unlink()
        (source / "case.txt").unlink()
    else:
        row("special-file", "skipped", "FIFO not applicable")
        row("case-collision", "skipped", "host filesystem is case-insensitive")

    after_count = len(
        json.loads(
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
                "post-failure history",
            ).stdout
        )["snapshots"]
    )
    assert after_count == history_count, "failed capture published a snapshot"
    final_restore = root / "restored-final"
    require(
        call(
            [
                "restore",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--snapshot",
                metadata["snapshot_id"],
                "--destination",
                str(final_restore),
                "--password-file",
                str(secret),
            ]
        ),
        "final restore",
    )
    assert tree(final_restore) == tree(source), "prior snapshot did not survive refusals"
    report["result"] = "pass"
    (root / "v06-matrix.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps({"result": "pass", "rows": report["rows"]}, indent=2))


if __name__ == "__main__":
    main()
