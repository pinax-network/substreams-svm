#!/usr/bin/env python3
"""Re-encode a public Solana JSON-RPC getBlock response as sf.solana.type.v1.Block bytes.

The field mapping mirrors the Firehose RPC poller in
streamingfast/firehose-solana cbf1571, block/fetcher/rpc.go (blockFromBlockResult,
toPbTransactionMeta, toPbMessage and their helpers). The input is a getBlock
response requested with encoding "json", transactionDetails "full", rewards and
maxSupportedTransactionVersion 1.

Known approximation: the poller stores meta.err as the bincode-encoded
TransactionError, which the JSON response does not carry. A present error is
written as 12 zero bytes; only its presence is read by this repository. The
poller's hash and parent-slot patch tables cover historical slots only and are
not applied.

The output is Firehose-format, not bytes served by a Firehose provider.
Standard library only; no network access. Fields are written in field-number
order, as the Go encoder does.

Usage: python3 -I rpc_to_pb.py <getBlock.json> <slot> <out.pb>
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import struct
from pathlib import Path

ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
DIGITS = {character: value for value, character in enumerate(ALPHABET)}
# solana-go's zero PublicKey, which the poller maps to an absent program id.
ZERO_KEY = "11111111111111111111111111111111"
REWARD_TYPES = {"": 0, "Fee": 1, "Rent": 2, "Staking": 3, "Voting": 4, "DeactivatedStake": 5, "VATDebit": 6}


def base58(text: str, size: int | None = None) -> bytes:
    """Decode base58; keys (32) and signatures (64) are left-padded to their fixed size."""
    value = 0
    for character in text:
        value = value * 58 + DIGITS[character]
    body = value.to_bytes((value.bit_length() + 7) // 8, "big") if value else b""
    decoded = b"\x00" * (len(text) - len(text.lstrip("1"))) + body
    if size is None:
        return decoded
    if len(decoded) > size:
        raise ValueError(f"base58 value longer than {size} bytes")
    return decoded.rjust(size, b"\x00")


def varint(value: int) -> bytes:
    if value < 0:
        value += 1 << 64
    output = bytearray()
    while True:
        low = value & 0x7F
        value >>= 7
        if value:
            output.append(low | 0x80)
        else:
            output.append(low)
            return bytes(output)


def key(number: int, wire: int) -> bytes:
    return varint(number << 3 | wire)


def length_delimited(number: int, payload: bytes) -> bytes:
    return key(number, 2) + varint(len(payload)) + payload


def scalar(number: int, value: int | None) -> bytes:
    """A proto3 scalar: zero is the default and is not written."""
    return key(number, 0) + varint(value) if value else b""


def optional(number: int, value: int | None) -> bytes:
    """An explicit-presence scalar: written whenever it is set, zero included."""
    return b"" if value is None else key(number, 0) + varint(value)


def string(number: int, value: str | None) -> bytes:
    return length_delimited(number, value.encode()) if value else b""


def raw(number: int, value: bytes) -> bytes:
    return length_delimited(number, value) if value else b""


def packed(number: int, values: list[int] | None) -> bytes:
    return length_delimited(number, b"".join(varint(value) for value in values)) if values else b""


def reward(item: dict) -> bytes:
    commission = item.get("commission")
    return (
        string(1, item.get("pubkey"))
        + scalar(2, item.get("lamports"))
        + scalar(3, item.get("postBalance"))
        + scalar(4, REWARD_TYPES[item.get("rewardType") or ""])
        + string(5, None if commission is None else str(commission))
    )


def rewards(number: int, items: list[dict] | None) -> bytes:
    # toPBReward sorts by lamports. Go's sort is not stable; ties keep input order here.
    ordered = sorted(items or [], key=lambda item: item.get("lamports") or 0)
    return b"".join(length_delimited(number, reward(item)) for item in ordered)


def token_balance(balance: dict) -> bytes:
    output = scalar(1, balance.get("accountIndex")) + string(2, balance.get("mint"))
    amount = balance.get("uiTokenAmount")
    if amount is not None:
        ui_amount = amount.get("uiAmount") or 0.0
        output += length_delimited(
            3,
            (key(1, 1) + struct.pack("<d", ui_amount) if ui_amount else b"")
            + scalar(2, amount.get("decimals"))
            + string(3, amount.get("amount"))
            + string(4, amount.get("uiAmountString")),
        )
    program_id = balance.get("programId")
    return output + string(4, balance.get("owner")) + string(5, None if program_id == ZERO_KEY else program_id)


def instruction(item: dict, inner: bool) -> bytes:
    output = scalar(1, item["programIdIndex"]) + raw(2, bytes(item["accounts"])) + raw(3, base58(item["data"]))
    if inner:
        # toStackHeight maps 0 (unrecorded) to an absent height.
        output += optional(4, item.get("stackHeight") or None)
    return output


def transaction_config(config: dict | None) -> bytes | None:
    # A v1 message's inline compute budget; empty settings map to nil.
    if config is None or all(value is None for value in config.values()):
        return None
    return (
        optional(1, config.get("priorityFee"))
        + optional(2, config.get("computeUnitLimit"))
        + optional(3, config.get("loadedAccountsDataSizeLimit"))
        + optional(4, config.get("heapSize"))
    )


def message(item: dict, version: str) -> bytes:
    header = item["header"]
    output = length_delimited(
        1,
        scalar(1, header["numRequiredSignatures"])
        + scalar(2, header["numReadonlySignedAccounts"])
        + scalar(3, header["numReadonlyUnsignedAccounts"]),
    )
    output += b"".join(length_delimited(2, base58(account, 32)) for account in item["accountKeys"])
    output += raw(3, base58(item["recentBlockhash"], 32))
    output += b"".join(length_delimited(4, instruction(compiled, False)) for compiled in item["instructions"])
    if version != "legacy":
        output += scalar(5, 1)
    for lookup in item.get("addressTableLookups") or []:
        output += length_delimited(
            6,
            raw(1, base58(lookup["accountKey"], 32)) + raw(2, bytes(lookup["writableIndexes"])) + raw(3, bytes(lookup["readonlyIndexes"])),
        )
    if version != "legacy":
        # The wire version: 0 for a v0 message, 1 for a v1 message.
        output += optional(7, int(version))
    config = transaction_config(item.get("transactionConfig"))
    if config is not None:
        output += length_delimited(8, config)
    return output


def meta(item: dict | None) -> bytes:
    if item is None:
        return b""
    output = length_delimited(1, raw(1, b"\x00" * 12)) if item.get("err") is not None else b""
    output += scalar(2, item.get("fee")) + packed(3, item.get("preBalances")) + packed(4, item.get("postBalances"))
    for group in item.get("innerInstructions") or []:
        output += length_delimited(
            5,
            scalar(1, group["index"]) + b"".join(length_delimited(2, instruction(inner, True)) for inner in group["instructions"]),
        )
    output += b"".join(length_delimited(6, (line or "").encode()) for line in item.get("logMessages") or [])
    output += b"".join(length_delimited(7, token_balance(balance)) for balance in item.get("preTokenBalances") or [])
    output += b"".join(length_delimited(8, token_balance(balance)) for balance in item.get("postTokenBalances") or [])
    output += rewards(9, item.get("rewards"))
    loaded = item.get("loadedAddresses") or {}
    output += b"".join(length_delimited(12, base58(address, 32)) for address in loaded.get("writable") or [])
    output += b"".join(length_delimited(13, base58(address, 32)) for address in loaded.get("readonly") or [])
    return_data = item.get("returnData")
    data = base64.b64decode(return_data["data"][0]) if return_data and return_data.get("data") else b""
    if data:
        output += length_delimited(14, raw(1, base58(return_data["programId"], 32)) + raw(2, data))
    return output + optional(16, item.get("computeUnitsConsumed")) + optional(17, item.get("costUnits"))


def block(result: dict, slot: int) -> bytes:
    output = string(1, result["previousBlockhash"]) + string(2, result["blockhash"]) + scalar(3, result["parentSlot"])
    for confirmed in result["transactions"]:
        transaction = confirmed["transaction"]
        version = str(confirmed.get("version", "legacy"))
        encoded = b"".join(length_delimited(1, base58(signature, 64)) for signature in transaction["signatures"])
        encoded += length_delimited(2, message(transaction["message"], version))
        output += length_delimited(4, length_delimited(1, encoded) + length_delimited(2, meta(confirmed.get("meta"))))
    output += rewards(5, result.get("rewards"))
    if result.get("blockTime") is not None:
        output += length_delimited(6, scalar(1, result["blockTime"]))
    if result.get("blockHeight") is not None:
        output += length_delimited(7, scalar(1, result["blockHeight"]))
    return output + scalar(20, slot)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("response", type=Path, help="getBlock JSON-RPC response")
    parser.add_argument("slot", type=int, help="the requested slot, which the response does not repeat")
    parser.add_argument("output", type=Path, help="where to write the encoded block; must not exist")
    args = parser.parse_args()
    source = args.response.read_bytes()
    encoded = block(json.loads(source)["result"], args.slot)
    with args.output.open("xb") as destination:
        destination.write(encoded)
    print(f"response sha256={hashlib.sha256(source).hexdigest()}")
    print(f"block {len(encoded)} bytes, sha256={hashlib.sha256(encoded).hexdigest()}")


if __name__ == "__main__":
    main()
