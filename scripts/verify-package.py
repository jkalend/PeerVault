"""Verify a retained package archive, checksum sidecar, manifest and SBOM posture."""

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
    "docs/security-review-2026-11-18.md",
    "docs/license-review.md",
    "docs/local-smoke-2026-11-18.md",
    "scripts/v05-retention.py",
    "scripts/v06-matrix.py",
    "scripts/v08-ipc.py",
    "scripts/v08-schedule.py",
    "scripts/v08-secret.py",
    "scripts/v06-capacity.py",
    "scripts/v06-diskfull.py",
    "scripts/v06-upgrade.py",
    "scripts/secret-store.ps1",
    "scripts/task-run.ps1",
    "scripts/install-scheduler.ps1",
    "scripts/install-scheduler.sh",
    "scripts/uninstall-scheduler.ps1",
    "scripts/uninstall-scheduler.sh",
}
FORBIDDEN = (".peervault-work", ".git/", "target/", ".env", "secret.txt", "password.txt")


def normalize(name):
    return name.removeprefix("./").removeprefix("dist/")


def sha256(path):
    hasher = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            hasher.update(chunk)
    return hasher.hexdigest()


def member_bytes(archive, wanted):
    wanted = normalize(wanted)
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as package:
            for name in package.namelist():
                if normalize(name) == wanted:
                    return package.read(name)
    elif archive.name.endswith(".tar.gz"):
        with tarfile.open(archive, "r:gz") as package:
            for entry in package:
                if normalize(entry.name) == wanted:
                    stream = package.extractfile(entry)
                    if stream is None:
                        raise SystemExit(f"package member is not a file: {wanted}")
                    return stream.read()
    return None


def members(archive):
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as package:
            return {normalize(name) for name in package.namelist()}
    if archive.name.endswith(".tar.gz"):
        with tarfile.open(archive, "r:gz") as package:
            return {normalize(name) for name in package.getnames()}
    raise SystemExit("unsupported package archive")


def require_licenses(sbom):
    for component in sbom.get("components", []):
        name = component.get("name", "<unnamed>")
        licenses = component.get("licenses")
        if not licenses:
            raise SystemExit(f"SBOM component lacks a license: {name}")
        for entry in licenses:
            if entry.get("expression"):
                continue
            license_id = entry.get("license", {}).get("id")
            license_name = entry.get("license", {}).get("name")
            if not license_id and not license_name:
                raise SystemExit(f"SBOM component has an empty license entry: {name}")
            if license_name == "LicenseRef-See-Component-LICENSE":
                raise SystemExit(f"SBOM component has an unresolved license file: {name}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--require-desktop", action="store_true")
    parser.add_argument("--allow-dirty", action="store_true")
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

    sbom_bytes = member_bytes(archive, "sbom.cdx.json")
    manifest_bytes = member_bytes(archive, "release-manifest.json")
    if sbom_bytes is None or manifest_bytes is None:
        raise SystemExit("package has no SBOM or release manifest")
    sbom = json.loads(sbom_bytes)
    manifest = json.loads(manifest_bytes)
    if manifest.get("schema_version") != 1 or manifest.get("product") != "peervault":
        raise SystemExit("unsupported release manifest")
    if manifest.get("source", {}).get("dirty") and not arguments.allow_dirty:
        raise SystemExit("release manifest was produced from a dirty tree")
    for artifact in manifest.get("artifacts", []):
        data = member_bytes(archive, artifact["name"])
        if data is None:
            raise SystemExit(f"manifest artifact is missing from package: {artifact['name']}")
        if len(data) != artifact["size_bytes"] or hashlib.sha256(data).hexdigest() != artifact["sha256"]:
            raise SystemExit(f"manifest artifact does not match packaged bytes: {artifact['name']}")
    sbom_entry = manifest.get("sbom", {})
    if (
        sbom_entry.get("name") != "sbom.cdx.json"
        or len(sbom_bytes) != sbom_entry.get("size_bytes")
        or hashlib.sha256(sbom_bytes).hexdigest() != sbom_entry.get("sha256")
    ):
        raise SystemExit("SBOM does not match the release manifest")

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
    require_licenses(sbom)
    print(
        json.dumps(
            {
                "result": "pass",
                "sha256": actual,
                "members": len(names),
                "manifest_artifacts": len(manifest.get("artifacts", [])),
                "sbom_components": len(sbom.get("components", [])),
                "license_review": review,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
