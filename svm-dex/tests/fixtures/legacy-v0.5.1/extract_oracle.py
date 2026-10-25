#!/usr/bin/env python3
"""Extract three immutable WASM test binaries from the exact published svm-dex-v0.5.1 package.
No download or network access occurs. Different existing files are never replaced.
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path


PACKAGE_SIZE = 2_097_201
PACKAGE_SHA256 = "0774a2330e9d070d55f1e39d0b0c057415aaafec7df94602ac21671020c5bf46"
ORACLES = (
    (b'db_out', 0, b'db_out', 'db_out.wasm', 291933,
     'c7e8e781484b615115c55761db49a165c25575a78a31b1bc85a3b978c674a5b4'),
    (b'dex_swaps:map_events', 1, b'map_events', 'dex_swaps.wasm', 1229232,
     '5f00c0e2dc5161fede0d8b25226a8c0ab8405823b4796e2b29692209aca4e087'),
    (b'dex_swaps:solana_common:blocks_without_votes', 2, b'blocks_without_votes',
     'blocks_without_votes.wasm', 316182,
     '3507399c9f8091276b72d750632c6979e78e42a0137de8cee1be5e61b8cdb392'),
)


def varint(data: bytes, position: int) -> tuple[int, int]:
    value = 0
    for shift in range(0, 70, 7):
        if position >= len(data):
            raise ValueError("truncated protobuf varint")
        byte = data[position]
        position += 1
        if shift == 63 and byte > 1:
            raise ValueError("oversized protobuf varint")
        value |= (byte & 0x7F) << shift
        if byte < 0x80:
            return value, position
    raise ValueError("unterminated protobuf varint")


def fields(data: bytes) -> list[tuple[int, int, int | bytes]]:
    """Read only the protobuf wire framing; do not interpret decoder code."""
    output = []
    position = 0
    while position < len(data):
        tag, position = varint(data, position)
        number, wire = tag >> 3, tag & 7
        if number == 0:
            raise ValueError("invalid protobuf field zero")
        if wire == 0:
            value, position = varint(data, position)
        elif wire in (1, 2, 5):
            if wire == 2:
                size, position = varint(data, position)
            else:
                size = 8 if wire == 1 else 4
            end = position + size
            if end > len(data):
                raise ValueError("truncated protobuf field")
            value = data[position:end]
            position = end
        else:
            raise ValueError(f"unsupported protobuf wire type {wire}")
        output.append((number, wire, value))
    return output


def values(message: list[tuple[int, int, int | bytes]], number: int, wire: int):
    matching = [(kind, value) for field, kind, value in message if field == number]
    if any(kind != wire for kind, _ in matching):
        raise ValueError(f"wrong wire type for protobuf field {number}")
    return [value for _, value in matching]


def one(message: list[tuple[int, int, int | bytes]], number: int, wire: int):
    matching = values(message, number, wire)
    if len(matching) != 1:
        raise ValueError(f"expected one protobuf field {number}, got {len(matching)}")
    return matching[0]


def extract(package: bytes) -> list[tuple[str, bytes]]:
    if len(package) != PACKAGE_SIZE or hashlib.sha256(package).hexdigest() != PACKAGE_SHA256:
        raise ValueError("input is not the pinned historical v0.5.1 SPKG")
    # sf.substreams.v1.Package.modules = 6; Modules.modules = 1, binaries = 2.
    modules = fields(one(fields(package), 6, 2))
    declarations = [fields(value) for value in values(modules, 1, 2)]
    binaries = [fields(value) for value in values(modules, 2, 2)]
    output = []
    for module_name, index, entrypoint, filename, size, digest in ORACLES:
        matching = [module for module in declarations if one(module, 1, 2) == module_name]
        if len(matching) != 1:
            raise ValueError(f"missing or duplicate module {module_name!r}")
        module = matching[0]
        encoded_index = values(module, 4, 0)
        if encoded_index != [index] and not (index == 0 and encoded_index == []):
            raise ValueError(f"unexpected binary index for {module_name!r}")
        if one(module, 5, 2) != entrypoint:
            raise ValueError(f"unexpected entrypoint for {module_name!r}")
        if index >= len(binaries):
            raise ValueError(f"missing binary {index}")
        binary = one(binaries[index], 2, 2)
        if (
            len(binary) != size
            or not binary.startswith(b"\x00asm\x01\x00\x00\x00")
            or hashlib.sha256(binary).hexdigest() != digest
        ):
            raise ValueError(f"unexpected WASM bytes for {module_name!r}")
        output.append((filename, binary))
    return output


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path, help="local, unmodified svm-dex-v0.5.1.spkg")
    args = parser.parse_args()
    try:
        # Bound the read even if the supplied path is not a normal file.
        with args.package.open("rb") as source:
            package = source.read(PACKAGE_SIZE + 1)
        extracted = extract(package)
        directory = Path(__file__).resolve().parent
        # Validate every existing destination before creating any missing file.
        for filename, binary in extracted:
            target = directory / filename
            if target.exists() and target.read_bytes() != binary:
                raise ValueError(f"refusing to replace differing fixture {filename}")
        for filename, binary in extracted:
            target = directory / filename
            if not target.exists():
                with target.open("xb") as destination:
                    destination.write(binary)
            print(f"{filename}: {len(binary)} bytes, sha256={hashlib.sha256(binary).hexdigest()}")
    except (OSError, ValueError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
