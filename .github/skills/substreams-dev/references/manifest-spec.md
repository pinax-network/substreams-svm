# Substreams Manifest Specification (SVM)

Complete reference for `substreams.yaml` manifest files in Solana/SVM Substreams.

## Basic Structure

```yaml
specVersion: v0.1.0
package:
  name: my_solana_substreams
  version: v1.0.0
  url: https://github.com/pinax-network/substreams-svm
  description: My Solana Substreams module
  image: ../../image.png

imports:
  solana_common: ../../../spkg/solana-common-v0.4.0.spkg

binaries:
  default:
    type: wasm/rust-v1+wasm-bindgen-shims
    file: ../../target/wasm32-unknown-unknown/release/my_module.wasm

protobuf:
  files:
    - v1/my-events.proto
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
        string: "program:<PROGRAM_ID>"
    output:
      type: proto:my.package.v1.Events

network: solana
```

## SVM-Specific Fields

### Binary Type

SVM Substreams use wasm-bindgen shims:

```yaml
binaries:
  default:
    type: wasm/rust-v1+wasm-bindgen-shims
    file: ../../target/wasm32-unknown-unknown/release/my_module.wasm
```

### Block Filter

Filter blocks by Solana program ID to skip irrelevant transactions:

```yaml
blockFilter:
  module: solana_common:program_ids_without_votes
  query:
    string: "program:675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"
```

Multiple programs:

```yaml
blockFilter:
  module: solana_common:program_ids_without_votes
  query:
    string: "program:675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8 || program:CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK"
```

### Network

Always `solana` for SVM:

```yaml
network: solana
```

### Common Import

Most SVM modules import `solana-common` for vote-filtered blocks:

```yaml
imports:
  solana_common: ../../../spkg/solana-common-v0.4.0.spkg
```

### Input Source

SVM modules typically use the filtered block from `solana_common`:

```yaml
inputs:
  - map: solana_common:blocks_without_votes
```

Or the raw Solana block:

```yaml
inputs:
  - source: sf.solana.type.v1.Block
```

## Sink Module Manifests

### ClickHouse Sink

```yaml
specVersion: v0.1.0
package:
  name: svm_dex_clickhouse
  version: v0.3.1

imports:
  db: ../db-svm-dex/substreams.yaml

modules:
  - name: db_out
    use: db:db_out

sink:
  module: db_out
  type: sf.substreams.sink.sql.v1.Service
  config:
    schema: "./schema.sql"
    engine: clickhouse
    postgraphile_frontend:
      enabled: false

network: solana
```

### PostgreSQL Sink

```yaml
specVersion: v0.1.0
package:
  name: svm_dex_postgres
  version: v0.3.1

imports:
  db: ../db-svm-dex/substreams.yaml

modules:
  - name: db_out
    use: db:db_out

sink:
  module: db_out
  type: sf.substreams.sink.sql.v1.Service
  config:
    schema: "./schema.sql"
    engine: postgres
    postgraphile_frontend:
      enabled: true

network: solana
```

## Aggregate DB Module Manifests

Aggregate modules combine multiple DEX module outputs into a single `db_out`:

```yaml
imports:
  database_changes: ../spkg/substreams-sink-database-changes-v4.0.0.spkg
  sql: ../spkg/substreams-sink-sql-protodefs-v1.0.7.spkg
  pumpfun: https://github.com/pinax-network/substreams-solana/releases/download/svm-dex-v0.3.0/pumpfun-bonding-curve-v0.2.2.spkg
  raydium_amm_v4: https://github.com/pinax-network/substreams-solana/releases/download/svm-dex-v0.2.0/raydium-amm-v4-v0.2.0.spkg
  # ... more DEX imports

modules:
  - name: db_out
    kind: map
    inputs:
      - source: sf.substreams.v1.Clock
      - map: pumpfun:map_events
      - map: raydium_amm_v4:map_events
      # ... more DEX inputs
    output:
      type: proto:sf.substreams.sink.database.v1.DatabaseChanges
```
