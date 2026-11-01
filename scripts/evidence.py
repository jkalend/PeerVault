"""Map experimental Evidence v2/v3 to append-only EvidenceManifest v1; never import private work state."""

import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent.parent
LIMIT = 2 * 1024 * 1024


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read(path):
    if path.is_symlink() or not path.is_file() or path.stat().st_size > LIMIT:
        raise ValueError("invalid evidence member or size")
    with path.open("rb") as stream:
        data = stream.read(LIMIT + 1)
    if len(data) > LIMIT:
        raise ValueError("evidence member grew beyond limit")
    return data


def identifier(value):
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,127}", value):
        raise ValueError("unsafe registry identifier")
    return value


def encode(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def export(work, registry, profile_path=None):
    report_bytes = read(work / "evidence.json")
    report = json.loads(report_bytes)
    if report["schema_version"] not in (2, 3) or report["result"] not in ("pass", "fail") or report["gate_status"] not in ("partial", "fail", "unknown"):
        raise ValueError("unsupported experimental evidence or falsely completed R0")
    profile = report["profile"] or {"schema_version": 1, "profile_id": f"unqualified-{report['os']}", "classification": "smoke", "missing_fields": ["named qualification profile"]}
    run = registry / "R0" / identifier(profile["profile_id"]) / identifier(report["run_id"])
    artifacts = {
        "report.json": report_bytes,
        "profile.json": encode(profile),
        "source-inputs.json": encode(report["source_inputs"]),
        "planning-inputs.json": read(ROOT / "docs/planning-inputs.json"),
        "fixture-a.json": read(ROOT / "tests/fixtures/v1-a.json"),
        "fixture-b.json": read(ROOT / "tests/fixtures/v1-b.json"),
        "commands.md": read(ROOT / "docs/r0.md"),
    }
    if report["profile"] is not None:
        if profile_path is None:
            raise ValueError("exact profile input required for retained named-profile evidence")
        profile_bytes = read(profile_path)
        if digest(profile_bytes) != report["profile_sha256"] or json.loads(profile_bytes) != profile:
            raise ValueError("selected profile input changed since run")
        artifacts["profile-input.json"] = profile_bytes
    # Read/validate everything before reserving a never-overwritten run directory.
    run.mkdir(parents=True, exist_ok=False)
    inventory = []
    for name, data in artifacts.items():
        (run / name).write_bytes(data)
        inventory.append({"path": name, "size_bytes": len(data), "sha256": digest(data)})
    manifest = {
        "schema_version": 1, "gate": "R0", "run_id": report["run_id"], "profile_id": profile["profile_id"],
        "gate_status": report["gate_status"], "baseline_result": report["result"],
        "inputs": {"source": "source-inputs.json", "planning": "planning-inputs.json"},
        "backend": {key: report[key] for key in ("backend", "backend_version", "executable_sha256", "backend_lock_sha256")},
        "tool": {key: report[key] for key in ("harness_version", "harness_sha256")},
        "environment": {key: report[key] for key in ("os", "arch", "clock_quality", "profile_sha256")},
        "budgets": {key: report[key] for key in ("command_timeout_seconds", "output_limit_bytes_per_stream")},
        "assertions": report["steps"], "restored_files": report["restored_files"], "faults": report.get("faults", []),
        "topology": report.get("topology"),
        "failures": [report["failure_reason"]] if report["failure_reason"] else [],
        "untested_cases": report["untested_cases"], "artifacts": inventory,
        "reviewer": {"status": "unreviewed", "identity": None}, "supersedes": [],
    }
    (run / "manifest.json").write_bytes(encode(manifest))
    verify(run)
    print(f"Retained {run}; manifest SHA-256 {digest(read(run / 'manifest.json'))}")
    return run


def verify(run):
    manifest = json.loads(read(run / "manifest.json"))
    if manifest["schema_version"] != 1 or manifest["gate"] != "R0" or manifest["gate_status"] == "pass":
        raise ValueError("invalid/unsupported R0 manifest")
    members = manifest["artifacts"]
    if not 1 <= len(members) <= 32:
        raise ValueError("invalid evidence inventory size")
    names = {"manifest.json"}
    for member in members:
        name = identifier(member["path"])
        if name in names:
            raise ValueError("duplicate evidence inventory member")
        names.add(name)
        data = read(run / name)
        if len(data) != member["size_bytes"] or digest(data) != member["sha256"]:
            raise ValueError("evidence member missing/changed")
    if {path.name for path in run.iterdir()} != names:
        raise ValueError("unlisted evidence member")
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="command", required=True)
    retain = subcommands.add_parser("export")
    retain.add_argument("work", type=Path)
    retain.add_argument("registry", type=Path)
    retain.add_argument("--profile", type=Path)
    check = subcommands.add_parser("verify")
    check.add_argument("run", type=Path)
    args = parser.parse_args()
    if args.command == "export": export(args.work, args.registry, args.profile)
    else:
        verify(args.run)
        print("Evidence inventory verified; this does not establish qualification.")
