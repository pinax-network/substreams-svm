ENDPOINT ?= solana.substreams.pinax.network:443
START_BLOCK ?= 350000000
STOP_BLOCK ?= 350002000
PARALLEL_JOBS ?= 500

.PHONY: schema
schema:
	@echo "-- This file is generated. Do not edit." > schema.sql
	@echo "" >> schema.sql
	@for f in $$(ls schema.*.sql 2>/dev/null | sort); do \
		cat "$$f" >> schema.sql; \
		echo "" >> schema.sql; \
	done

.PHONY: build
build: schema

.PHONY: pack
pack: build
	substreams pack -o ../../spkg/svm-clickhouse-transfers-v0.3.1.spkg

.PHONY: dev
dev:
	substreams-sink-sql run clickhouse://default:@localhost:9000/default substreams.yaml -e $(ENDPOINT) $(START_BLOCK):$(STOP_BLOCK) --undo-buffer-size 100 --on-module-hash-mistmatch=warn --batch-block-flush-interval 100 --infinite-retry

.PHONY: setup
setup: pack
	substreams-sink-sql setup clickhouse://default:@localhost:9000/default substreams.yaml
