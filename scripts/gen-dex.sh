#!/bin/bash
# Generate DEX crate boilerplate
# Usage: ./scripts/gen-dex.sh <name> <desc> <program_id> <type>
# type: swap-only | swap-xy | buy-sell-events

NAME=$1
DESC=$2
PROGRAM_ID=$3
TYPE=$4
BASE=/data/workspace/substreams-svm

mkdir -p "$BASE/dex/$NAME/src"

# Cargo.toml
cat > "$BASE/dex/$NAME/Cargo.toml" << EOF
[package]
name = "$NAME"
description = "$DESC"
edition = { workspace = true }
version = { workspace = true }

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
substreams = { workspace = true }
substreams-solana = { workspace = true }
substreams-solana-idls = { workspace = true }
proto = { path = "../../proto" }
common = { path = "../../common" }
EOF

# substreams.yaml
cat > "$BASE/dex/$NAME/substreams.yaml" << EOF
specVersion: v0.1.0
package:
  name: $NAME
  version: v0.1.0
  url: https://github.com/pinax-network/substreams-svm
  description: $DESC events for Solana.
  image: ../../image.png

imports:
  solana_common: ../../../spkg/solana-common-v0.4.0.spkg

binaries:
  default:
    type: wasm/rust-v1+wasm-bindgen-shims
    file: ../../target/wasm32-unknown-unknown/release/$NAME.wasm

protobuf:
  files:
    - v1/$NAME.proto
  importPaths:
    - ../../proto

modules:
  - name: map_events
    kind: map
    inputs:
      - map: solana_common:blocks_without_votes
    blockFilter:
      module: solana_common:program_ids_without_votes
      query:
        string: "program:$PROGRAM_ID"
    output:
      type: proto:$NAME.v1.Events

network: solana
EOF

echo "Generated dex/$NAME"
