"""Verify a retained package archive, checksum sidecar and SBOM license posture."""

import argparse
import hashlib
import json
from pathlib import Path
import tarfile
import zipfile

REQUIRED = {
    "README.md",
    "QUALIFICATION.md",
    "LICENSE",
    "backend-lock.json",
    "components-lock.txt",
    "THIRD-PARTY-NOTICES.txt",
    "sbom.cdx.json",
    "release-manifest.json",
    "docs/v0.5.md",
    "docs/v0.6-matrix.md",
    "docs/v0.8.md",
    "docs/v0.9-measure.md",
    "docs/support-policy.md",
    "docs/compatibility.md",
    "docs/runbook.md",
    "docs/release-checklist.md",
    "docs/security-review-scope.md",
    "docs/license-review.md",
    "docs/local-smoke-2026-11-18.md",
    "scripts/v05-retention.py",
    "scripts/v06-matrix.py",
    "scripts/v08-ipc.py",
    "scripts/v08-schedule.py",
    "scripts/v08-secret.py",
    "scripts/v06-capacity.py",
    "scripts/v06-upgrade.py",
    "scripts/secret-store.ps1",
    "scripts/task-run.ps1",
    "scripts/install-scheduler.ps1",
    "scripts/install-scheduler.sh",
    "scripts/uninstall-scheduler.ps1",
    "scripts/uninstall-scheduler.sh",
}
FORBIDDEN = (".peervault-work", ".git/", "target/", ".env", "secret.txt", "password.txt")


def sha256(path):
    hasher = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            hasher.update(chunk)
    return hasher.hexdigest()


def members(archive):
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as package:
            names = package.namelist()
            return {name.removeprefix("./").removeprefix("dist/") for name in names}
    if archive.name.endswith(".tar.gz"):
        with tarfile.open(archive, "r:gz") as package:
            names = package.getnames()
            return {name.removeprefix("./").removeprefix("dist/") for name in names}
    raise SystemExit("unsupported package archive")


def sbom_license_review(archive):
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as package:
            for name in package.namelist():
                if name.removeprefix("./").removeprefix("dist/") == "sbom.cdx.json":
                    return json.loads(package.read(name))
    else:
        with tarfile.open(archive, "r:gz") as package:
            for entry in package:
                if entry.name.removeprefix("./").removeprefix("dist/") == "sbom.cdx.json":
                    return json.loads(package.extractfile(entry).read())
    raise SystemExit("package has no SBOM")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--require-desktop", action="store_true")
    parser.add_argument("--allow-license-block", action="store_true")
    arguments = parser.parse_args()
    archive = arguments.archive
    if not archive.is_file() or archive.is_symlink():
        raise SystemExit("archive is not a regular file")
    sidecar = archive.with_name(archive.name + ".sha256")
    if not sidecar.is_file():
        raise SystemExit("checksum sidecar is missing")
    expected = sidecar.read_text(encoding="utf-8").split()[0].lower()
    actual = sha256(archive)
    if actual != expected:
        raise SystemExit("archive SHA-256 does not match its sidecar")
    names = members(archive)
    missing = sorted(REQUIRED - names)
    if missing:
        raise SystemExit(f"package is missing required members: {missing}")
    if arguments.require_desktop and not any(name.startswith("desktop/") for name in names):
        raise SystemExit("package is missing the desktop payload")
    for name in names:
        lowered = name.lower()
        if any(fragment in lowered for fragment in FORBIDDEN):
            raise SystemExit(f"package contains a forbidden member: {name}")
    sbom = sbom_license_review(archive)
    review = next(
        (
            prop["value"]
            for prop in sbom.get("metadata", {}).get("properties", [])
            if prop.get("name") == "peervault:license_review"
        ),
        "",
    )
    if review.startswith("blocked:"):
        message = f"license review blocked: {review}"
        if arguments.allow_license_block:
            print(f"WARNING: {message}")
        else:
            raise SystemExit(message)
    print(json.dumps({"result": "pass", "sha256": actual, "members": len(names), "license_review": review}, indent=2))


if __name__ == "__main__":
    main()
