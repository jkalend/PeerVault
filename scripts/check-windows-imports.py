#!/usr/bin/env python3
"""Reject Windows PE recovery binaries that require a redistributable CRT.

The clean-client drill must run on fresh Windows without a VC++ redistributable.
This intentionally small PE reader checks the import directory for the CRT DLLs
that Rust's MSVC target links when ``+crt-static`` is missing.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import struct
import sys

FORBIDDEN_PREFIXES = (
    "vcruntime",
    "msvcp",
    "ucrtbase",
    "api-ms-win-crt-",
)
IMPORT_DIRECTORY_INDEX = 1


def _u16(data: bytes, offset: int) -> int:
    return struct.unpack_from("<H", data, offset)[0]


def _u32(data: bytes, offset: int) -> int:
    return struct.unpack_from("<I", data, offset)[0]


def _c_string(data: bytes, offset: int, limit: int = 4096) -> str:
    end = data.find(b"\0", offset, min(len(data), offset + limit))
    if end < 0:
        raise ValueError(f"unterminated import name at file offset {offset:#x}")
    return data[offset:end].decode("ascii", "replace")


def imports(path: Path) -> list[str]:
    data = path.read_bytes()
    if len(data) < 0x40 or data[:2] != b"MZ":
        raise ValueError("not a DOS/PE image")
    pe = _u32(data, 0x3C)
    if pe + 24 > len(data) or data[pe : pe + 4] != b"PE\0\0":
        raise ValueError("missing PE signature")
    coff = pe + 4
    section_count = _u16(data, coff + 2)
    optional_size = _u16(data, coff + 16)
    optional = coff + 20
    if optional + optional_size > len(data):
        raise ValueError("truncated optional header")
    magic = _u16(data, optional)
    if magic == 0x20B:
        directory_offset = optional + 112
        directory_count = _u32(data, optional + 108)
    elif magic == 0x10B:
        directory_offset = optional + 96
        directory_count = _u32(data, optional + 92)
    else:
        raise ValueError(f"unsupported optional-header magic {magic:#x}")
    if directory_count <= IMPORT_DIRECTORY_INDEX:
        return []
    import_rva = _u32(data, directory_offset + IMPORT_DIRECTORY_INDEX * 8)
    import_size = _u32(data, directory_offset + IMPORT_DIRECTORY_INDEX * 8 + 4)
    if import_rva == 0 or import_size == 0:
        return []

    sections = []
    sections_offset = optional + optional_size
    for index in range(section_count):
        header = sections_offset + index * 40
        if header + 40 > len(data):
            raise ValueError("truncated section table")
        virtual_size = _u32(data, header + 8)
        virtual_address = _u32(data, header + 12)
        raw_size = _u32(data, header + 16)
        raw_offset = _u32(data, header + 20)
        sections.append((virtual_address, max(virtual_size, raw_size), raw_offset))

    def file_offset(rva: int, length: int = 1) -> int:
        for virtual_address, size, raw_offset in sections:
            if virtual_address <= rva < virtual_address + size:
                offset = raw_offset + (rva - virtual_address)
                if offset + length <= len(data):
                    return offset
        raise ValueError(f"RVA {rva:#x} is outside every file-backed section")

    names: list[str] = []
    descriptor = file_offset(import_rva, 20)
    for index in range(4096):
        fields = struct.unpack_from("<IIIII", data, descriptor + index * 20)
        if not any(fields):
            break
        name_rva = fields[3]
        if name_rva == 0:
            break
        names.append(_c_string(data, file_offset(name_rva)))
    return names


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    args = parser.parse_args()
    try:
        names = imports(args.binary)
    except (OSError, ValueError, struct.error) as error:
        print(json.dumps({"result": "fail", "binary": str(args.binary), "error": str(error)}))
        return 1
    forbidden = sorted(
        name for name in names if name.lower().startswith(FORBIDDEN_PREFIXES)
    )
    report = {
        "result": "fail" if forbidden else "pass",
        "binary": str(args.binary),
        "imports": sorted(names),
        "forbidden_crt_imports": forbidden,
    }
    print(json.dumps(report, indent=2))
    return 1 if forbidden else 0


if __name__ == "__main__":
    sys.exit(main())
