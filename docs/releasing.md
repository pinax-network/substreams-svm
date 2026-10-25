# Releasing packages

A release ships `.spkg` files that are committed under `spkg/` and attached to
a GitHub release. The committed file is the release asset, so it must be built
from the reviewed tree, in dependency order, the same way CI builds the
workspace.

## Build once, the CI way

Do not use `make pack` for a release. The package Makefiles run a per-crate
`cargo build`, whose feature unification differs from the workspace build that
CI tests, and `substreams pack` never rebuilds the WASM of an imported package.

```sh
R=$(pwd -P)   # the physical path; on macOS /tmp is a link to /private/tmp
RUSTFLAGS="--remap-path-prefix=$R=/src --remap-path-prefix=$HOME/.cargo=/cargo --remap-path-prefix=$HOME/.rustup=/rustup" \
  cargo +1.85.0 build --locked --workspace --release --target wasm32-unknown-unknown
python3 scripts/check-wasm-imports.py
```

Rust 1.85.0 is the CI toolchain. `rust-toolchain.toml` asks for channel `1.85`,
which can resolve to 1.85.1. WASM bytes, and therefore module hashes, depend on
the toolchain patch level, the build mode (workspace or per crate), and on the
checkout path and `CARGO_HOME`, which some crates embed (`spl_token` embeds
`spl/transfers/src/decode.rs`, and `svm_transfers` and `svm_dex` embed
`proto/src/pb/*.rs` through `include!`). The remap flags make the bytes
independent of the checkout path. Byte-identity across checkouts has been
verified on the same host OS and architecture only.

## Pack in dependency order

Edit every README first. `substreams pack` embeds the package's README as its
documentation, and an importing package carries the imported package's
metadata, so a README change after packing requires a repack (the `.spkg`
SHA-256 changes, the module hashes do not).

Pack imports before the packages that import them, from the same build. For
the svm-transfers and svm-dex graphs:

```sh
for d in spl/token native/system spl/transfers-substreams svm-transfers dex-swaps svm-dex; do
  (cd "$d" && substreams pack substreams.yaml -o "$R/spkg/{spkgDefaultName}")
done
for d in svm-transfers/clickhouse svm-dex/clickhouse; do
  (cd "$d" && make schema && substreams pack substreams.yaml -o "$R/spkg/{spkgDefaultName}")
done
```

Packing is offline and deterministic. `spkg/.gitignore` re-includes `*.spkg`,
so `git add spkg/<file>` works; the generated `schema.sql` stays ignored.

## Record and check

- `shasum -a 256` of each `.spkg` and of each
  `target/wasm32-unknown-unknown/release/*.wasm` it embeds; the embedded binary
  must equal the target file.
- Module hashes, offline: `substreams info --json <spkg>`.
- `rustc -Vv`, `cargo -V` and `substreams --version`.

The committed package tests (`cargo test -p svm-transfers committed_` and
`cargo test -p svm-dex committed_`) run the WASM embedded in the committed
`spkg/` files with a bounded test-only interpreter and require the native
rows on the pinned blocks. They also check that every package the release
manifests build or import exists with a matching name and version, and that a
released file keeps the SHA-256 recorded in the test. Record a new release's
SHA-256 values there in the release PR.

A decoder or formatter change makes those tests fail until the manifest
version is bumped and the package is repacked under its new name. Never
repack a released version in place.

Validation here is offline. Before a production re-sink, run the released
package on a current block from a provider, and qualify representative
historical ranges.
