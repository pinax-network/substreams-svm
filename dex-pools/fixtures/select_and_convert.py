#!/usr/bin/env python3
"""Select one complete transaction from a saved getBlock response, without network access.

The selected fixture retains the source block metadata, but is not a complete
block: its sole transaction has local index zero. The source index and all
capture hashes are printed for the provenance README.
"""

import argparse
import gzip
import hashlib
import importlib.util
import io
import json
import sys
from pathlib import Path

sys.dont_write_bytecode = True


def compressed(data):
    output = io.BytesIO()
    with gzip.GzipFile(filename="", mode="wb", fileobj=output, compresslevel=9, mtime=0) as stream:
        stream.write(data)
    return output.getvalue()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("response", type=Path)
    parser.add_argument("slot", type=int)
    parser.add_argument("transaction_index", type=int)
    parser.add_argument("output_prefix", type=Path)
    args = parser.parse_args()
    source = args.response.read_bytes()
    response = json.loads(source)
    result = response["result"]
    selected = result["transactions"][args.transaction_index]
    source_count = len(result["transactions"])
    result["transactions"] = [selected]
    # Preserve the selected transaction's fields and original block metadata.
    selected_json = (json.dumps(response, sort_keys=True, separators=(",", ":")) + "\n").encode()
    converter_path = Path(__file__).resolve().parents[2] / "spl/transfers/tests/fixtures/rpc/rpc_to_pb.py"
    spec = importlib.util.spec_from_file_location("rpc_to_pb", converter_path)
    converter = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(converter)
    encoded = converter.block(result, args.slot)
    outputs = {
        Path(str(args.output_prefix) + ".json.gz"): compressed(selected_json),
        Path(str(args.output_prefix) + ".pb.gz"): compressed(encoded),
    }
    if any(path.exists() for path in outputs):
        raise FileExistsError("refusing to overwrite an existing fixture")
    for path, contents in outputs.items():
        with path.open("xb") as destination:
            destination.write(contents)
        print(f"{path.name}: bytes={len(contents)} sha256={hashlib.sha256(contents).hexdigest()}")
    print(f"source: bytes={len(source)} sha256={hashlib.sha256(source).hexdigest()}")
    print(f"selected JSON: bytes={len(selected_json)} sha256={hashlib.sha256(selected_json).hexdigest()}")
    print(f"encoded: bytes={len(encoded)} sha256={hashlib.sha256(encoded).hexdigest()}")
    print(f"slot={args.slot} source_transactions={source_count} source_index={args.transaction_index}")
    print(f"signature={selected['transaction']['signatures'][0]}")
    print(f"blockhash={result['blockhash']} blockTime={result['blockTime']} parentSlot={result['parentSlot']}")


if __name__ == "__main__":
    main()
