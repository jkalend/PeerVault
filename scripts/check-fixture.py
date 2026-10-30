"""Independent source enumeration/SHA-256 oracle; does not import the Rust/backend code."""

import argparse
import hashlib
import json
from pathlib import Path
import stat

ROOT = Path(__file__).resolve().parent.parent


def check(source, revision):
    expected = json.loads((ROOT / "tests/fixtures" / f"v1-{'a' if revision == 1 else 'b'}.json").read_text(encoding="utf-8"))
    files, directories = [], []
    pending = [source]
    while pending:
        path = pending.pop()
        info = path.lstat()
        if stat.S_ISLNK(info.st_mode) or getattr(info, "st_file_attributes", 0) & 0x400:
            raise ValueError("fixture contains link/reparse point")
        if path != source:
            relative = path.relative_to(source).as_posix()
            if stat.S_ISDIR(info.st_mode):
                directories.append(relative)
            elif stat.S_ISREG(info.st_mode):
                if info.st_size > 16 * 1024 * 1024:
                    raise ValueError("fixture file exceeds small-corpus limit")
                digest = hashlib.sha256()
                size = 0
                with path.open("rb") as stream:
                    while chunk := stream.read(64 * 1024):
                        size += len(chunk)
                        if size > 16 * 1024 * 1024:
                            raise ValueError("fixture grew during independent read")
                        digest.update(chunk)
                files.append({"path": relative, "size": size, "sha256": digest.hexdigest()})
            else:
                raise ValueError("fixture contains unsupported entry")
        if len(files) + len(directories) > 1024:
            raise ValueError("fixture entry limit exceeded")
        if stat.S_ISDIR(info.st_mode):
            pending.extend(path.iterdir())
    observed = {"schema_version": 1, "fixture_version": 1,
                "directories": sorted(directories), "files": sorted(files, key=lambda entry: entry["path"])}
    if observed != expected:
        raise ValueError("independent source path/type/length/SHA-256 comparison failed")
    result = {"revision": revision, "files": len(files), "bytes": sum(entry["size"] for entry in files),
              "directories": len(directories), "oracle": "independent Python hashlib source read", "result": "pass"}
    print(json.dumps(result))
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--revision", type=int, choices=(1, 2), required=True)
    args = parser.parse_args()
    check(args.source, args.revision)
