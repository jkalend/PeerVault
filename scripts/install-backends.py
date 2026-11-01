"""Install approved R0 executables with bounded reads and atomic replacement (Python 3.10+)."""

import argparse
import bz2
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import platform
import stat
import tarfile
import tempfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent.parent
ARCHIVE_LIMIT = 128 * 1024 * 1024
BINARY_LIMIT = 128 * 1024 * 1024
TAR_LIMIT = BINARY_LIMIT + 8 * 1024 * 1024


def bounded_read(stream, limit):
    data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError("size limit exceeded")
    return data


class LimitedStream:
    def __init__(self, stream, limit):
        self.stream, self.remaining = stream, limit

    def read(self, size=-1):
        data = self.stream.read(min(size if size >= 0 else self.remaining + 1, self.remaining + 1))
        self.remaining -= len(data)
        if self.remaining < 0:
            raise ValueError("decompressed archive size limit exceeded")
        return data


def check_digest(data, expected):
    if hashlib.sha256(data).hexdigest() != expected:
        raise ValueError("SHA-256 mismatch; refusing executable installation")


def read_cache(path):
    if not path.is_file() or path.is_symlink() or path.stat().st_size > ARCHIVE_LIMIT:
        raise ValueError("invalid or oversized archive cache")
    with path.open("rb") as stream:
        return bounded_read(stream, ARCHIVE_LIMIT)


def atomic_write(target, data, executable=False):
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=target.parent, delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        if executable:
            temporary.chmod(0o755)
        os.replace(temporary, target)
        temporary = None
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def extract(data, asset):
    member_name = asset.get("member")
    if asset["url"].endswith(".bz2"):
        with bz2.BZ2File(io.BytesIO(data)) as package:
            return bounded_read(package, BINARY_LIMIT)
    if not member_name or PurePosixPath(member_name).is_absolute() or ".." in PurePosixPath(member_name).parts:
        raise ValueError("invalid named archive member")
    if asset["url"].endswith(".zip"):
        with zipfile.ZipFile(io.BytesIO(data)) as package:
            matches = [entry for entry in package.infolist() if entry.filename == member_name]
            if len(matches) != 1:
                raise ValueError("missing/duplicate archive member")
            entry = matches[0]
            mode = entry.external_attr >> 16
            if entry.is_dir() or stat.S_ISLNK(mode) or (stat.S_IFMT(mode) not in (0, stat.S_IFREG)) or entry.file_size > BINARY_LIMIT:
                raise ValueError("invalid member type/size")
            with package.open(entry) as stream:
                return bounded_read(stream, BINARY_LIMIT)
    if not asset["url"].endswith(".tar.gz"):
        raise ValueError("unsupported archive format")
    binary = None
    with gzip.GzipFile(fileobj=io.BytesIO(data)) as decompressed:
        stream = LimitedStream(decompressed, TAR_LIMIT)
        with tarfile.open(fileobj=stream, mode="r|") as package:
            for entry in package:
                if entry.name != member_name:
                    continue
                if binary is not None or not entry.isfile() or entry.size > BINARY_LIMIT:
                    raise ValueError("duplicate or invalid member type/size")
                with package.extractfile(entry) as selected:
                    binary = bounded_read(selected, BINARY_LIMIT)
        # Require complete gzip integrity, bounded even past the TAR end marker.
        while stream.read(64 * 1024):
            pass
    if binary is None:
        raise ValueError("pinned archive member missing")
    return binary


def select_asset(pin, system):
    if system not in ("windows", "linux"):
        raise ValueError("unsupported platform")
    asset = pin[system]
    if not asset["url"].startswith("https://github.com/") or f"/v{pin['version']}/" not in asset["url"]:
        raise ValueError("backend version/URL drift")
    for field in ("sha256", "executable_sha256"):
        digest = asset[field]
        if len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
            raise ValueError("invalid digest pin")
    return asset


def install(name, pin, system, destination, opener=urllib.request.urlopen):
    asset = select_asset(pin, system)
    archive = destination / asset["url"].rsplit("/", 1)[1]
    if archive.exists():
        data = read_cache(archive)
    else:
        request = urllib.request.Request(asset["url"], headers={"User-Agent": "PeerVault-R0"})
        with opener(request, timeout=60) as response:
            data = bounded_read(response, ARCHIVE_LIMIT)
    check_digest(data, asset["sha256"])
    binary = extract(data, asset)
    check_digest(binary, asset["executable_sha256"])
    # Verify everything before publishing cache or replacing a previous executable.
    if not archive.exists():
        atomic_write(archive, data)
    target = destination / (name + (".exe" if system == "windows" else ""))
    atomic_write(target, binary, executable=True)
    return target


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--destination",
        type=Path,
        default=ROOT / ".tools",
        help="install directory (default: <repo>/.tools)",
    )
    arguments = parser.parse_args()
    system = platform.system().lower()
    if platform.machine().lower() not in ("amd64", "x86_64"):
        raise ValueError("only x86-64 backends are pinned")
    with (ROOT / "backend-lock.json").open("rb") as stream:
        pins = json.loads(bounded_read(stream, 65536))
    if pins["schema_version"] != 2:
        raise ValueError("unsupported backend pin schema")
    destination = arguments.destination
    destination.mkdir(parents=True, exist_ok=True)
    for name in ("restic", "kopia"):
        target = install(name, pins[name], system, destination)
        print(f"{name} {pins[name]['version']}: archive and executable SHA-256 verified; {target}")


if __name__ == "__main__":
    main()
