"""V0.9 measurement harness: repeated capture/verify/restore on a fixed corpus.

Reports raw wall-clock observations plus median/sample p95. This is not a
14-day beta and does not control OS page cache; first/subsequent runs are
labelled rather than claimed cold/warm.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import statistics
import subprocess
import time


def run(binary, args, timeout=3600):
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
        raise SystemExit(f"{label} failed ({result.returncode}): {(result.stderr or result.stdout)[:800]}")
    return result


def tree(root):
    files, directories = {}, set()
    for path in root.rglob("*"):
        relative = path.relative_to(root).as_posix()
        if path.is_dir():
            directories.add(relative)
        elif path.is_file():
            with path.open("rb") as stream:
                digest = hashlib.sha256()
                while chunk := stream.read(1024 * 1024):
                    digest.update(chunk)
            files[relative] = (path.stat().st_size, digest.hexdigest())
    return files, directories


def directory_bytes(root):
    return sum(path.stat().st_size for path in root.rglob("*") if path.is_file())


def timing(call):
    started = time.perf_counter()
    result = call()
    return (time.perf_counter() - started) * 1000.0, result


def summarize(values):
    ordered = sorted(values)
    median = statistics.median(ordered)
    if len(ordered) == 1:
        p95 = ordered[0]
    else:
        index = min(len(ordered) - 1, max(0, round(0.95 * (len(ordered) - 1))))
        p95 = ordered[index]
    return {"median_ms": round(median, 1), "sample_p95_ms": round(p95, 1), "raw_ms": [round(value, 1) for value in values]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--root", type=Path, required=True, help="new measurement root")
    parser.add_argument("--size-mib", type=int, default=128)
    parser.add_argument("--runs", type=int, default=5)
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
    chunk = hashlib.sha256(b"peervault-measure").digest() * 32768
    remaining = arguments.size_mib * 1024 * 1024
    with (source / "large.bin").open("wb") as stream:
        while remaining:
            piece = chunk[: min(len(chunk), remaining)]
            stream.write(piece)
            remaining -= len(piece)
    for index in range(100):
        (source / f"small-{index:03d}.txt").write_text(f"{index}\n", encoding="utf-8")
    expected = tree(source)

    def call(args):
        return run(binary, args)

    require(call(["init", "--repository", str(repository), "--backend", backend, "--password-file", str(secret)]), "init")
    capture_times, verify_times, restore_times = [], [], []
    for run_index in range(arguments.runs):
        if run_index:
            (source / "large.bin").write_bytes(
                hashlib.sha256(f"rev-{run_index}".encode()).digest() * (arguments.size_mib * 1024 * 1024 // 32)
            )
        elapsed, result = timing(
            lambda: require(
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
                f"capture {run_index}",
            )
        )
        capture_times.append(elapsed)
        snapshot = json.loads(result.stdout)["snapshot_id"]
        elapsed, _ = timing(
            lambda: require(
                call(["verify", "--repository", str(repository), "--backend", backend, "--password-file", str(secret)]),
                f"verify {run_index}",
            )
        )
        verify_times.append(elapsed)
        destination = root / f"restore-{run_index}"
        elapsed, _ = timing(
            lambda: require(
                call(
                    [
                        "restore",
                        "--repository",
                        str(repository),
                        "--backend",
                        backend,
                        "--snapshot",
                        snapshot,
                        "--destination",
                        str(destination),
                        "--password-file",
                        str(secret),
                    ]
                ),
                f"restore {run_index}",
            )
        )
        restore_times.append(elapsed)
        assert tree(destination) == tree(source), f"restore {run_index} did not match"
    report = {
        "schema_version": 1,
        "result": "pass",
        "corpus": {
            "files": len(expected[0]),
            "directories": len(expected[1]),
            "source_bytes": sum(size for size, _ in expected[0].values()),
            "repository_bytes": directory_bytes(repository),
        },
        "runs": arguments.runs,
        "cache_label": "first run versus subsequent; OS page cache not controlled",
        "capture": summarize(capture_times),
        "verify": summarize(verify_times),
        "restore": summarize(restore_times),
        "limitations": [
            "descriptive small-sample measurement, not an RPO/RTO commitment",
            "no controlled cold-cache, RSS, CPU or transfer-byte instrumentation",
            "single host and synthetic corpus; not the 14-day beta",
        ],
    }
    (root / "v09-measure.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
