import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import evidence


class EvidenceTests(unittest.TestCase):
    def test_missing_changed_extra_and_traversal_members_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = b"sanitized diagnostic"
            member = root / "report.json"
            member.write_bytes(data)
            manifest = {"schema_version": 1, "gate": "R0", "gate_status": "partial", "artifacts": [
                {"path": "report.json", "size_bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}]}
            path = root / "manifest.json"
            path.write_text(json.dumps(manifest), encoding="utf-8")
            evidence.verify(root)
            member.write_bytes(b"modified")
            with self.assertRaises(ValueError): evidence.verify(root)
            member.unlink()
            with self.assertRaises(ValueError): evidence.verify(root)
            member.write_bytes(data)
            extra = root / "private-state"
            extra.write_bytes(b"must not be exported")
            with self.assertRaises(ValueError): evidence.verify(root)
            extra.unlink()
            manifest["artifacts"][0]["path"] = "../outside"
            path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaises(ValueError): evidence.verify(root)

    def test_existing_runs_and_legacy_reports_are_not_reinterpreted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            work = root / "work"
            work.mkdir()
            report = {"schema_version": 1}
            (work / "evidence.json").write_text(json.dumps(report), encoding="utf-8")
            with self.assertRaises(ValueError): evidence.export(work, root / "registry")
            inputs = {name: evidence.digest((evidence.ROOT / name).read_bytes())
                for name in ("tests/fixtures/v1-a.json", "tests/fixtures/v1-b.json")}
            report.update({"schema_version": 2, "result": "pass", "gate_status": "partial",
                "profile": None, "os": "windows", "run_id": "existing-run", "source_inputs": inputs})
            (work / "evidence.json").write_text(json.dumps(report), encoding="utf-8")
            (root / "registry/R0/unqualified-windows/existing-run").mkdir(parents=True)
            with self.assertRaises(FileExistsError): evidence.export(work, root / "registry")

    def test_changed_fixture_golden_refuses_export(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            work = root / "work"
            work.mkdir()
            report = {"schema_version": 2, "result": "pass", "gate_status": "partial",
                "profile": None, "os": "windows", "run_id": "fixture-drift",
                "source_inputs": {name: "0" * 64 for name in
                    ("tests/fixtures/v1-a.json", "tests/fixtures/v1-b.json")}}
            (work / "evidence.json").write_text(json.dumps(report), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "changed since the run"):
                evidence.export(work, root / "registry")


if __name__ == "__main__": unittest.main()
