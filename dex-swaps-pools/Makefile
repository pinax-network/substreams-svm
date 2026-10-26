.DEFAULT_GOAL := pack

.PHONY: build pack test
build:
	cargo build --locked -p dex-swaps-pools --target wasm32-unknown-unknown --release

pack: build
	substreams pack substreams.yaml -o ../spkg/{spkgDefaultName}

test:
	cargo test --locked -p dex-pools -p dex-swaps-pools
