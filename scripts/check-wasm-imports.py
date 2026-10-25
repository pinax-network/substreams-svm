#!/usr/bin/env python3
"""Check that plain `wasm/rust-v1` Substreams binaries import only host calls.

Substreams provides wasm-bindgen shims only to binaries declared as
`wasm/rust-v1+wasm-bindgen-shims`. A binary declared as exactly `wasm/rust-v1`
that imports anything outside the `env` and `logger` host modules cannot be
instantiated. Run after the WASM build, from any directory:

    cargo build --locked --workspace --release --target wasm32-unknown-unknown
    python3 scripts/check-wasm-imports.py

Every `substreams*.yaml` under the repository is checked. A declared plain
binary that was not built is an error. No dependencies beyond Python 3.8.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

PLAIN = "wasm/rust-v1"
HOST_MODULES = {"env", "logger"}
SKIP_DIRS = {".git", "target", "node_modules"}


def binaries(manifest: Path) -> list[tuple[str, str, str]]:
    """Return (name, type, file) for each entry of the top-level `binaries` map.

    The manifests use one plain block-style map, so no YAML library is needed.
    """
    entries: dict[str, dict[str, str]] = {}
    inside = False
    entry = None
    for raw in manifest.read_text().splitlines():
        line = raw.rstrip()
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        indent = len(line) - len(line.lstrip())
        key, _, value = line.strip().partition(":")
        value = value.split(" #", 1)[0].strip().strip("'\"")
        if indent == 0:
            inside = key == "binaries"
            entry = None
        elif inside and not value:
            entry = entries.setdefault(key, {})
        elif inside and entry is not None:
            entry[key] = value
    return [(name, fields.get("type", ""), fields.get("file", "")) for name, fields in entries.items()]


def leb128(data: bytes, position: int) -> tuple[int, int]:
    value = 0
    for shift in range(0, 35, 7):
        if position >= len(data):
            raise ValueError("truncated LEB128 integer")
        byte = data[position]
        position += 1
        value |= (byte & 0x7F) << shift
        if byte < 0x80:
            return value, position
    raise ValueError("oversized LEB128 integer")


def name(data: bytes, position: int) -> tuple[str, int]:
    length, position = leb128(data, position)
    if position + length > len(data):
        raise ValueError("truncated import name")
    return data[position : position + length].decode("utf-8"), position + length


def limits(data: bytes, position: int) -> int:
    flags, position = leb128(data, position)
    _, position = leb128(data, position)
    if flags & 1:
        _, position = leb128(data, position)
    return position


def imports(module: bytes) -> list[tuple[str, str]]:
    """Return (module, name) for every entry of the WASM import section."""
    if module[:8] != b"\x00asm\x01\x00\x00\x00":
        raise ValueError("not a version 1 WASM module")
    position = 8
    while position < len(module):
        section = module[position]
        size, position = leb128(module, position + 1)
        end = position + size
        if end > len(module):
            raise ValueError("truncated WASM section")
        if section != 2:
            position = end
            continue
        count, position = leb128(module, position)
        output = []
        for _ in range(count):
            module_name, position = name(module, position)
            field, position = name(module, position)
            kind = module[position]
            position += 1
            if kind == 0:  # function: type index
                _, position = leb128(module, position)
            elif kind == 1:  # table: reference type, limits
                position = limits(module, position + 1)
            elif kind == 2:  # memory: limits
                position = limits(module, position)
            elif kind == 3:  # global: value type, mutability
                position += 2
            elif kind == 4:  # tag: attribute, type index
                _, position = leb128(module, position + 1)
            else:
                raise ValueError(f"unknown import kind {kind}")
            output.append((module_name, field))
        if position != end:
            raise ValueError("malformed WASM import section")
        return output
    return []


def manifests(root: Path):
    for directory, subdirectories, files in os.walk(root):
        subdirectories[:] = sorted(d for d in subdirectories if d not in SKIP_DIRS)
        for file in sorted(files):
            if file.startswith("substreams") and file.endswith((".yaml", ".yml")):
                yield Path(directory) / file


def main() -> int:
    root = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[1]
    failures = []
    checked = 0
    for manifest in manifests(root):
        for binary, kind, file in binaries(manifest):
            if kind != PLAIN:
                continue
            label = f"{manifest.relative_to(root)} ({binary})"
            path = (manifest.parent / file).resolve()
            if not path.is_file():
                failures.append(f"{label}: {file} was not built")
                continue
            try:
                found = imports(path.read_bytes())
            except (OSError, UnicodeDecodeError, ValueError) as error:
                failures.append(f"{label}: cannot read imports of {file}: {error}")
                continue
            foreign = [f"{module}.{field}" for module, field in found if module not in HOST_MODULES]
            checked += 1
            if foreign:
                shown = ", ".join(foreign[:5]) + (f", ... ({len(foreign)} total)" if len(foreign) > 5 else "")
                failures.append(f"{label}: {path.name} imports outside env/logger: {shown}")
            else:
                print(f"ok {label}: {path.name} imports {', '.join(f'{m}.{f}' for m, f in found)}")
    for failure in failures:
        print(f"error {failure}", file=sys.stderr)
    if failures:
        print(
            f"{len(failures)} plain {PLAIN} binaries need only env/logger imports; "
            "keep native-only code out of wasm32 or declare +wasm-bindgen-shims.",
            file=sys.stderr,
        )
        return 1
    if checked == 0:
        print(f"error no {PLAIN} binaries found under {root}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
