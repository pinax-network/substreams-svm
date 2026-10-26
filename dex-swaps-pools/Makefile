ENDPOINT ?= solana.substreams.pinax.network:443
START_BLOCK ?= 400000000
STOP_BLOCK ?= 400000050
PARALLEL_JOBS ?= 100
.DEFAULT_GOAL := pack

.PHONY: build pack test
build:
	cargo build --locked -p dex-swaps-pools --target wasm32-unknown-unknown --release

pack: build
	substreams pack substreams.yaml -o ../spkg/{spkgDefaultName}

test:
	cargo test --locked -p dex-pools -p dex-swaps-pools

.PHONY: gui
gui: build
	substreams gui -e $(ENDPOINT) substreams.yaml map_pool_creations -s $(START_BLOCK) -t $(STOP_BLOCK)

.PHONY: prod
prod: build
	substreams gui -e $(ENDPOINT) substreams.yaml map_pool_creations -s $(START_BLOCK) -t $(STOP_BLOCK) --limit-processed-blocks 0 --production-mode  -H "X-Sf-Substreams-Parallel-Jobs: $(PARALLEL_JOBS)"
