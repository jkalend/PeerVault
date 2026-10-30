import bz2
import hashlib
import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

spec = importlib.util.spec_from_file_location("installer", Path(__file__).with_name("install-backends.py"))
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.binary = b"approved executable bytes"
        self.target = self.root / "sample.exe"
        self.target.write_bytes(b"prior valid executable")

    def zip_asset(self, binary=None):
        stream = io.BytesIO()
        with zipfile.ZipFile(stream, "w", zipfile.ZIP_DEFLATED) as package:
            package.writestr("sample.exe", self.binary if binary is None else binary)
        return stream.getvalue()

    def pin(self, data, suffix=".zip", binary=None):
        return {"version": "1.0", "windows": {
            "url": "https://github.com/example/project/releases/download/v1.0/sample" + suffix,
            "sha256": hashlib.sha256(data).hexdigest(),
            "executable_sha256": hashlib.sha256(self.binary if binary is None else binary).hexdigest(),
            "member": "sample.exe"}}

    def attempt(self, data, pin=None):
        selected = pin or self.pin(data)
        archive = self.root / selected["windows"]["url"].rsplit("/", 1)[1]
        archive.write_bytes(data)
        return installer.install("sample", selected, "windows", self.root)

    def test_cached_and_decompressed_limits_preserve_previous_binary(self):
        data = self.zip_asset()
        with patch.object(installer, "ARCHIVE_LIMIT", 8):
            with self.assertRaises(ValueError):
                self.attempt(data)
        with patch.object(installer, "BINARY_LIMIT", 8):
            with self.assertRaises(ValueError):
                self.attempt(data)
        self.assertEqual(self.target.read_bytes(), b"prior valid executable")

    def test_digest_truncation_type_and_atomic_failures_preserve_previous_binary(self):
        data = self.zip_asset()
        bad_pin = self.pin(data)
        bad_pin["windows"]["executable_sha256"] = "0" * 64
        with self.assertRaises(ValueError): self.attempt(data, bad_pin)
        for broken in (data[:-12], b"invalid archive"):
            with self.assertRaises((ValueError, zipfile.BadZipFile)):
                self.attempt(broken)
        with patch.object(installer.os, "replace", side_effect=OSError("injected rename failure")):
            with self.assertRaises(OSError): self.attempt(data)
        self.assertEqual(self.target.read_bytes(), b"prior valid executable")
        self.assertEqual(sorted(p.name for p in self.root.iterdir()), ["sample.exe", "sample.zip"])
        self.assertEqual(self.attempt(data).read_bytes(), self.binary)

    def test_link_member_and_version_platform_drift_are_rejected(self):
        stream = io.BytesIO()
        entry = zipfile.ZipInfo("sample.exe")
        entry.create_system = 3
        entry.external_attr = 0o120777 << 16
        with zipfile.ZipFile(stream, "w") as package: package.writestr(entry, "outside")
        with self.assertRaises(ValueError): self.attempt(stream.getvalue())
        pin = self.pin(self.zip_asset())
        pin["version"] = "2.0"
        with self.assertRaises(ValueError): self.attempt(self.zip_asset(), pin)
        with self.assertRaises(ValueError): installer.select_asset(pin, "darwin")

    def test_bzip_tar_and_download_limits(self):
        data = bz2.compress(self.binary)
        pin = self.pin(data, ".bz2")
        self.assertEqual(self.attempt(data, pin).read_bytes(), self.binary)
        with patch.object(installer, "BINARY_LIMIT", 8):
            with self.assertRaises(ValueError): self.attempt(data, pin)
        stream = io.BytesIO()
        with tarfile.open(fileobj=stream, mode="w:gz") as package:
            entry = tarfile.TarInfo("sample.exe")
            entry.size = len(self.binary)
            package.addfile(entry, io.BytesIO(self.binary))
        pin = self.pin(stream.getvalue(), ".tar.gz")
        self.assertEqual(self.attempt(stream.getvalue(), pin).read_bytes(), self.binary)
        with patch.object(installer, "TAR_LIMIT", 8):
            with self.assertRaises(ValueError): self.attempt(stream.getvalue(), pin)
        with self.assertRaises(ValueError): installer.bounded_read(io.BytesIO(b"123456789"), 8)


if __name__ == "__main__": unittest.main()
