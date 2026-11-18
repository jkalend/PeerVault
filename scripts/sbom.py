"""Generate a bounded CycloneDX 1.5 SBOM for the resolved host build."""

import argparse
import json
from pathlib import Path
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def host():
    output = subprocess.check_output(["rustc", "-vV"], text=True)
    return next(line.removeprefix("host: ") for line in output.splitlines() if line.startswith("host: "))


def cargo_metadata():
    return json.loads(
        subprocess.check_output(
            [
                "cargo",
                "metadata",
                "--locked",
                "--format-version=1",
                "--filter-platform",
                host(),
            ],
            cwd=ROOT,
            text=True,
            encoding="utf-8",
        )
    )


def lock_checksums():
    with (ROOT / "Cargo.lock").open("rb") as stream:
        lock = tomllib.load(stream)
    return {
        (package["name"], package["version"]): package.get("checksum")
        for package in lock.get("package", [])
    }


def license_entry(package):
    if package.get("license"):
        return [{"expression": package["license"]}]
    if package.get("license_file"):
        return [{"license": {"name": "LicenseRef-See-Component-LICENSE"}}]
    raise ValueError(f"dependency {package['name']} has no declared license")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    metadata = cargo_metadata()
    with (ROOT / "Cargo.toml").open("rb") as stream:
        version = tomllib.load(stream)["package"]["version"]
    resolved = {node["id"] for node in metadata["resolve"]["nodes"]}
    checksums = lock_checksums()
    components = []
    license_review = []
    for package in sorted(metadata["packages"], key=lambda item: (item["name"], item["version"])):
        if package["id"] not in resolved:
            continue
        if package["name"] == "peervault":
            continue
        checksum = checksums.get((package["name"], package["version"]))
        properties = [
            {"name": "peervault:source", "value": package.get("source") or "path"},
        ]
        if not package.get("license"):
            properties.append(
                {
                    "name": "peervault:license_file",
                    "value": package.get("license_file") or "missing",
                }
            )
            license_review.append(package["name"])
        component = {
            "type": "library",
            "bom-ref": f"pkg:cargo/{package['name']}@{package['version']}",
            "name": package["name"],
            "version": package["version"],
            "purl": f"pkg:cargo/{package['name']}@{package['version']}",
            "licenses": license_entry(package),
            "scope": "required",
            "properties": properties,
        }
        if package.get("repository"):
            component["externalReferences"] = [{"type": "website", "url": package["repository"]}]
        if checksum:
            component["hashes"] = [{"alg": "SHA-256", "content": checksum}]
        components.append(component)

    pins = json.loads((ROOT / "backend-lock.json").read_text(encoding="utf-8"))
    for name, pin in pins.items():
        if name == "schema_version":
            continue
        components.append(
            {
                "type": "application",
                "bom-ref": f"pkg:github/{pin['source'].split('/')[3]}/{pin['source'].split('/')[4]}@{pin['version']}",
                "name": name,
                "version": pin["version"],
                "licenses": [{"expression": pin["license"]}],
                "scope": "required" if name == "restic" else "optional",
                "externalReferences": [{"type": "website", "url": pin["source"]}],
                "properties": [
                    {"name": "peervault:bundled", "value": "false"},
                    {"name": "peervault:role", "value": "selected-backend" if name == "restic" else "r0-comparison"},
                ],
            }
        )

    sbom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "bom-ref": f"pkg:cargo/peervault@{version}",
                "name": "peervault",
                "version": version,
                "licenses": [{"license": {"id": "Apache-2.0"}}],
            },
            "tools": {"components": [{"type": "application", "name": "peervault-sbom"}]},
            "properties": [
                {"name": "peervault:target", "value": host()},
                {"name": "peervault:components_lock", "value": (ROOT / "components-lock.txt").read_text(encoding="utf-8").strip()},
                {
                    "name": "peervault:license_review",
                    "value": "blocked: " + ", ".join(sorted(license_review))
                    if license_review
                    else "all resolved components declare a license expression",
                },
            ],
        },
        "components": components,
    }
    text = json.dumps(sbom, ensure_ascii=False, indent=2) + "\n"
    if str(ROOT) in text or str(Path.home()) in text:
        raise SystemExit("SBOM contains a workstation path")
    arguments.output.write_text(text, encoding="utf-8", newline="\n")
    print(f"SBOM: {len(components)} components; {arguments.output}")


if __name__ == "__main__":
    main()
