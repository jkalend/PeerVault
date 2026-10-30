"""Collect license/notice files for the resolved host dependency graph into a package."""

import json
from pathlib import Path
import subprocess
import sys

host = next(line.removeprefix("host: ") for line in subprocess.check_output(["rustc", "-vV"], text=True).splitlines() if line.startswith("host: "))
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version=1", "--filter-platform", host], text=True, encoding="utf-8"))
resolved = {node["id"] for node in metadata["resolve"]["nodes"]}
notices = ["PeerVault: all rights reserved until a distribution license is chosen.\nThird-party components retain the following terms.\n"]
for package in sorted(metadata["packages"], key=lambda package: (package["name"], package["version"])):
    if package["id"] not in resolved or package["name"] == "peervault":
        continue
    directory = Path(package["manifest_path"]).parent
    licenses = sorted(path for path in directory.iterdir() if path.is_file() and path.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT")))
    if not licenses:
        raise SystemExit(f"No license/notice files found for {package['name']}; package review required")
    notices.append(f"\n{package['name']} {package['version']} — {package.get('license')}\n{package.get('repository') or ''}\n")
    for path in licenses:
        notices.append(f"\n{path.name}\n{path.read_text(encoding='utf-8')}\n")
Path(sys.argv[1]).write_text("".join(notices), encoding="utf-8", newline="\n")
