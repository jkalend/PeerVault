"""Create a bounded release/evidence manifest for exact retained artifacts."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time
import tomllib

ROOT = Path(__file__).resolve().parent.parent
LIMIT = 2 * 1024 * 1024 * 1024


def digest(path):
    hasher = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            hasher.update(chunk)
    return hasher.hexdigest()


def artifact(path):
    if path.is_symlink() or not path.is_file():
        raise SystemExit(f"artifact is not a regular file: {path}")
    if path.stat().st_size > LIMIT:
        raise SystemExit(f"artifact exceeds the size limit: {path}")
    return {
        "name": path.name,
        "size_bytes": path.stat().st_size,
        "sha256": digest(path),
    }


def git(*arguments):
    return subprocess.check_output(["git", *arguments], cwd=ROOT, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, action="append", required=True)
    parser.add_argument("--sbom", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--gate", action="append", default=[], help="name=pass|fail|partial|unknown")
    parser.add_argument("--allow-dirty", action="store_true")
    arguments = parser.parse_args()
    dirty = bool(git("status", "--porcelain"))
    if dirty and not arguments.allow_dirty:
        raise SystemExit("source tree is dirty; retained release evidence requires a clean tree")
    gates = []
    for item in arguments.gate:
        name, _, status = item.partition("=")
        if not name or status not in ("pass", "fail", "partial", "unknown"):
            raise SystemExit(f"invalid gate result: {item}")
        gates.append({"gate": name, "status": status})
    with (ROOT / "Cargo.toml").open("rb") as stream:
        version = tomllib.load(stream)["package"]["version"]
    pins = json.loads((ROOT / "backend-lock.json").read_text(encoding="utf-8"))
    manifest = {
        "schema_version": 1,
        "created_unix_seconds": int(time.time()),
        "product": "peervault",
        "version": version,
        "source": {
            "commit": git("rev-parse", "HEAD"),
            "dirty": dirty,
            "components_lock": (ROOT / "components-lock.txt").read_text(encoding="utf-8").strip(),
            "backend_lock_sha256": hashlib.sha256((ROOT / "backend-lock.json").read_bytes()).hexdigest(),
            "cargo_lock_sha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
        },
        "toolchain": {
            "rustc": subprocess.check_output(["rustc", "-vV"], text=True).strip(),
            "python": platform.python_version(),
            "platform": platform.platform(),
        },
        "backends": {
            name: {"version": pin["version"], "license": pin["license"], "source": pin["source"]}
            for name, pin in pins.items()
            if name != "schema_version"
        },
        "artifacts": [artifact(path) for path in arguments.package],
        "sbom": artifact(arguments.sbom),
        "gates": gates,
        "limitations": [
            "a manifest identifies retained bytes; it does not prove recoverability or witness truth",
            "CI artifacts are not a durable evidence archive",
            "publishing requires a separate owner go/no-go and publisher verification",
        ],
    }
    text = json.dumps(manifest, ensure_ascii=False, indent=2) + "\n"
    if str(ROOT) in text or str(Path.home()) in text:
        raise SystemExit("manifest contains a workstation path")
    temporary = arguments.output.with_suffix(arguments.output.suffix + ".tmp")
    temporary.write_text(text, encoding="utf-8", newline="\n")
    os.replace(temporary, arguments.output)
    print(f"manifest: {arguments.output}")


if __name__ == "__main__":
    main()
