"""V0.5 retention execution drill: preview, pin, floor, mass-change pause and apply.

This exercises the owner state, peer-root protection, preview drift refusal,
backend-native forget/prune and post-execution verification against the pinned
restic backend. It is a same-host synthetic drill, not an independent retention
or security review.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess


def run(binary, args, environment=None, timeout=1800):
    return subprocess.run(
        [binary, *args],
        capture_output=True,
        text=True,
        env=environment,
        timeout=timeout,
    )


def require(result, label):
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()[:800]
        raise SystemExit(f"{label} failed ({result.returncode}): {detail}")
    return result


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


def state_file(root):
    return root / "retention-state.json"


def policy_file(root, required_peers):
    path = root / "retention-policy.json"
    path.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "keep_last": 1,
                "keep_tagged": [],
                "keep_snapshots": [],
                "minimum_keep": 1,
                "floor_points": 1,
                "floor_days": 0,
                "required_peers": required_peers,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    return path


def backend_snapshots(backend, repository, secret):
    environment = {
        name: os.environ[name]
        for name in ("PATH", "SystemRoot", "WINDIR", "TMP", "TEMP", "TMPDIR")
        if name in os.environ
    }
    environment["RESTIC_PASSWORD_FILE"] = str(secret)
    result = subprocess.run(
        [
            backend,
            "--repo",
            str(repository),
            "--no-cache",
            "snapshots",
            "--json",
            "--no-lock",
        ],
        capture_output=True,
        text=True,
        env=environment,
        timeout=120,
    )
    require(result, "backend snapshot listing")
    return json.loads(result.stdout)


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
    environment = {
        name: os.environ[name]
        for name in ("PATH", "SystemRoot", "WINDIR", "TMP", "TEMP", "TMPDIR")
        if name in os.environ
    }

    def call(args):
        return run(binary, args, environment)

    (source / "nested").mkdir()
    (source / "nested" / "small.txt").write_text("revision-0\n", encoding="utf-8")
    (source / "blob.bin").write_bytes(bytes(range(256)) * 4096)
    (source / "empty.txt").write_bytes(b"")

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

    def capture(tag):
        result = require(
            call(
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
            ),
            f"capture {tag}",
        )
        return json.loads(result.stdout)

    capture("rev0")
    require(
        call(
            [
                "retention",
                "state-init",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--state",
                str(state_file(root)),
                "--password-file",
                str(secret),
            ]
        ),
        "state-init",
    )
    for revision in range(1, 4):
        (source / "nested" / "small.txt").write_text(
            f"revision-{revision}\n", encoding="utf-8"
        )
        capture(f"rev{revision}")

    history = json.loads(
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
    )
    assert len(history["snapshots"]) == 4, history
    pinned = history["snapshots"][2]
    assert pinned["oracle_manifest"]

    pinned_restore = root / "pinned-restore"
    restored = require(
        call(
            [
                "restore",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--snapshot",
                pinned["id"],
                "--destination",
                str(pinned_restore),
                "--password-file",
                str(secret),
            ]
        ),
        "pinned restore",
    )
    evidence = root / "peer-evidence.json"
    evidence.write_text(restored.stdout, encoding="utf-8")

    peer_repository_id = secrets.token_hex(32)
    require(
        call(
            [
                "retention",
                "peer-record",
                "--state",
                str(state_file(root)),
                "--peer-id",
                "alpha",
                "--peer-repository-id",
                peer_repository_id,
                "--snapshot",
                pinned["id"],
                "--capture-id",
                pinned["capture_id"],
                "--oracle-manifest",
                pinned["oracle_manifest"],
                "--key-version",
                "v1",
                "--evidence",
                str(evidence),
            ]
        ),
        "peer-record",
    )
    policy = policy_file(root, ["alpha"])
    preview = require(
        call(
            [
                "retention",
                "preview",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--policy",
                str(policy),
                "--state",
                str(state_file(root)),
                "--password-file",
                str(secret),
            ]
        ),
        "retention preview",
    )
    plan = json.loads(preview.stdout)
    plan_path = root / "retention-plan.json"
    plan_path.write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")
    keep_ids = {decision["id"] for decision in plan["keep"]}
    delete_ids = {decision["id"] for decision in plan["delete_candidates"]}
    assert pinned["id"] in keep_ids, "peer root was not protected"
    assert delete_ids, "preview produced no delete candidates"
    assert plan["mass_change"]["status"] == "not-triggered", plan["mass_change"]
    assert secret.read_text() not in json.dumps(plan)
    assert secret.read_text() not in state_file(root).read_text()

    applied = require(
        call(
            [
                "retention",
                "apply",
                "--plan",
                str(plan_path),
                "--backend",
                backend,
                "--state",
                str(state_file(root)),
                "--password-file",
                str(secret),
            ]
        ),
        "retention apply",
    )
    apply_report = json.loads(applied.stdout)
    assert apply_report["result"] == "pass", apply_report
    assert apply_report["state_generation"] == plan["state_generation"] + 1
    after = json.loads(
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
            "post-apply history",
        ).stdout
    )
    after_ids = {snapshot["id"] for snapshot in after["snapshots"]}
    assert keep_ids <= after_ids, (keep_ids, after_ids)
    assert not (delete_ids & after_ids), (delete_ids, after_ids)
    raw = backend_snapshots(backend, repository, secret)
    assert len(raw) == 2 * len(after_ids), (
        "expected one data and one oracle snapshot per kept capture",
        len(raw),
        len(after_ids),
    )

    replay = call(
        [
            "retention",
            "apply",
            "--plan",
            str(plan_path),
            "--backend",
            backend,
            "--state",
            str(state_file(root)),
            "--password-file",
            str(secret),
        ]
    )
    assert replay.returncode != 0, "stale preview must not execute twice"
    assert "changed since preview" in (replay.stderr + replay.stdout), replay.stderr

    newest_kept = max(after["snapshots"], key=lambda snapshot: snapshot["time"])
    restored_after = root / "post-apply-restore"
    require(
        call(
            [
                "restore",
                "--repository",
                str(repository),
                "--backend",
                backend,
                "--snapshot",
                newest_kept["id"],
                "--destination",
                str(restored_after),
                "--password-file",
                str(secret),
            ]
        ),
        "post-apply restore",
    )
    assert tree(restored_after) == tree(source), "kept snapshot did not restore exactly"

    (source / "blob.bin").write_bytes(bytes(reversed(range(256))) * 4096)
    capture("mass-change")
    preview_mass = json.loads(
        require(
            call(
                [
                    "retention",
                    "preview",
                    "--repository",
                    str(repository),
                    "--backend",
                    backend,
                    "--policy",
                    str(policy),
                    "--state",
                    str(state_file(root)),
                    "--password-file",
                    str(secret),
                ]
            ),
            "mass-change preview",
        ).stdout
    )
    assert preview_mass["mass_change"]["status"] == "triggered", preview_mass["mass_change"]
    mass_plan = root / "mass-change-plan.json"
    mass_plan.write_text(json.dumps(preview_mass, indent=2) + "\n", encoding="utf-8")
    refused = call(
        [
            "retention",
            "apply",
            "--plan",
            str(mass_plan),
            "--backend",
            backend,
            "--state",
            str(state_file(root)),
            "--password-file",
            str(secret),
        ]
    )
    assert refused.returncode != 0, "mass-change pause did not refuse execution"
    assert "--ack-mass-change" in (refused.stderr + refused.stdout), refused.stderr
    accepted = require(
        call(
            [
                "retention",
                "apply",
                "--plan",
                str(mass_plan),
                "--backend",
                backend,
                "--state",
                str(state_file(root)),
                "--password-file",
                str(secret),
                "--ack-mass-change",
            ]
        ),
        "acknowledged mass-change apply",
    )
    assert json.loads(accepted.stdout)["result"] == "pass"

    missing_peer_policy = policy_file(root, ["beta"])
    missing = call(
        [
            "retention",
            "preview",
            "--repository",
            str(repository),
            "--backend",
            backend,
            "--policy",
            str(missing_peer_policy),
            "--state",
            str(state_file(root)),
            "--password-file",
            str(secret),
        ]
    )
    assert missing.returncode != 0, "missing required peer must refuse a deletion plan"
    assert "no recorded root" in (missing.stderr + missing.stdout), missing.stderr

    print(
        json.dumps(
            {
                "result": "pass",
                "first_plan_kept": len(plan["keep"]),
                "first_plan_deleted": len(plan["delete_candidates"]),
                "remaining_captures": len(after_ids),
                "mass_change": preview_mass["mass_change"]["status"],
                "state_generation": json.loads(state_file(root).read_text())["generation"],
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
