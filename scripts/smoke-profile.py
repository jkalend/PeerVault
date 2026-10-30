"""Capture CI/host identity without claiming a qualified filesystem profile."""

import json
import os
from pathlib import Path
import platform
import sys

system = platform.system().lower()
profile = {
    "schema_version": 1, "profile_id": f"observed-{system}-smoke", "revision": 1,
    "classification": "smoke", "os": system,
    "architecture": "x86_64" if platform.machine().lower() in ("amd64", "x86_64") else platform.machine(),
    "environment": {"platform": platform.platform(), "kernel": platform.release(),
        "image_os": os.environ.get("ImageOS", "not recorded"),
        "image_version": os.environ.get("ImageVersion", "not recorded"),
        "storage_coverage": "unqualified; no independent media claim", "clock_probe": "unmeasured"},
    "missing_fields": ["filesystem/mount/case/long-path settings", "physical/virtual storage topology",
        "independent source/copy/secret media", "clock offset and quality", "image SHA-256 and qualification review"],
}
with Path(sys.argv[1]).open("x", encoding="utf-8", newline="\n") as stream:
    json.dump(profile, stream, indent=2)
    stream.write("\n")
