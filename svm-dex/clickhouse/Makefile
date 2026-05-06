ENDPOINT ?= solana.substreams.pinax.network:443
START_BLOCK ?= 400000000
STOP_BLOCK ?= 400000050
PARALLEL_JOBS ?= 500

.DEFAULT_GOAL := pack

.PHONY: schema
schema:
	@echo "-- This file is generated. Do not edit." > schema.sql
	@echo "" >> schema.sql
	@for f in $$(ls schema.*.sql 2>/dev/null | sort); do \
		cat "$$f" >> schema.sql; \
		echo "" >> schema.sql; \
	done

.PHONY: pack
pack: schema
	substreams pack -o ../../spkg/{spkgDefaultName}

.PHONY: dev
dev:
	substreams-sink-sql run clickhouse://default:@localhost:9000/default substreams.yaml -e $(ENDPOINT) $(START_BLOCK):$(STOP_BLOCK) --undo-buffer-size 1 --on-module-hash-mistmatch=warn --batch-block-flush-interval 1 --infinite-retry

.PHONY: setup
setup: pack
	substreams-sink-sql setup clickhouse://default:@localhost:9000/default substreams.yaml
