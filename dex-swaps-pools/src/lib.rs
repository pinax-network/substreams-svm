//! Shared pool projection with independent Hosted Store and legacy store adapters.
//! Native consumers use `dex-pools` directly.
use buffa::{Message, MessageField};
use buffa_types::google::protobuf::Any;
use dex_pools::{Block, EvidencePosition, Pool, Projection, Protocol};
pub use proto::pb::dex::pools::v1 as pb;
use substreams::pb::sf::substreams::foundational_store::model::v2::{Entry, Key, SinkEntries};
use substreams::pb::substreams::store_delta::Operation;
use substreams::store::{DeltaProto, Deltas, StoreSetIfNotExists};

pub const POOL_TYPE_URL: &str = "type.googleapis.com/dex.pools.v1.Pool";

/// Shared body for native/wrapper parity tests. Failures propagate; malformed
/// supported input must not silently advance the store with partial identities.
pub fn map_pool_creations_impl(block: &Block) -> Result<pb::Pools, dex_pools::ProjectError> {
    dex_pools::project(block).map(|projection| to_pool_creations(&projection))
}

/// Convenience composition for native/wrapper parity tests. The manifest uses
/// separate projection and encoding modules so both store branches decode once.
pub fn map_pool_entries_impl(block: &Block) -> Result<SinkEntries, dex_pools::ProjectError> {
    map_pool_creations_impl(block).map(|pools| pool_entries(&pools))
}

pub fn to_pool_creations(projection: &Projection) -> pb::Pools {
    pb::Pools {
        pools: projection.pools.iter().map(to_proto).collect(),
    }
}

/// Encode already decoded records without walking or decoding the block again.
pub fn to_sink_entries(projection: &Projection) -> SinkEntries {
    pool_entries(&to_pool_creations(projection))
}

/// Hosted Store branch: raw address keys and the same Pool bytes as the legacy store.
pub fn pool_entries(pools: &pb::Pools) -> SinkEntries {
    SinkEntries {
        entries: pools
            .pools
            .iter()
            .map(|pool| Entry {
                key: MessageField::some(Key {
                    bytes: pool.pool_address.clone(),
                }),
                value: MessageField::some(Any {
                    type_url: POOL_TYPE_URL.to_owned(),
                    value: pool.encode_to_vec().into(),
                    ..Default::default()
                }),
            })
            .collect(),
        if_not_exist: true,
    }
}

/// Legacy store branch. Ordinal zero is deliberate: this immutable registry
/// does not model execution order. Use Pool.creation for event provenance.
pub fn store_pools_impl<S: StoreSetIfNotExists<pb::Pool>>(pools: &pb::Pools, store: &S) {
    for pool in &pools.pools {
        store.set_if_not_exists(0, dex_pools::solana::base58::encode(&pool.pool_address), pool);
    }
}

/// Emit only identities newly inserted into the legacy store, preserving the
/// stored protobuf payload including optional-field presence and provenance.
pub fn map_pools_impl(deltas: Deltas<DeltaProto<pb::Pool>>) -> pb::Pools {
    pb::Pools {
        pools: deltas
            .deltas
            .into_iter()
            .filter(|delta| delta.operation == Operation::Create)
            .map(|delta| delta.new_value)
            .collect(),
    }
}

pub fn to_proto(pool: &Pool) -> pb::Pool {
    let creation = &pool.creation;
    pb::Pool {
        protocol: match pool.protocol {
            Protocol::PumpfunAmm => pb::Protocol::PROTOCOL_PUMPFUN_AMM,
            Protocol::OrcaWhirlpool => pb::Protocol::PROTOCOL_ORCA_WHIRLPOOL,
        }
        .into(),
        pool_address: pool.address.to_vec(),
        program_id: pool.program_id.to_vec(),
        tokens: pool
            .tokens
            .iter()
            .map(|token| pb::Token {
                mint: token.mint.to_vec(),
                vault: token.vault.map(|value| value.to_vec()),
                decimals: token.decimals.map(u32::from),
                token_program: token.token_program.map(|value| value.to_vec()),
            })
            .collect(),
        creation: MessageField::some(pb::Creation {
            slot: creation.slot,
            blockhash: creation.blockhash.clone(),
            signature: creation.signature.to_vec(),
            input_transaction_index: creation.transaction_index,
            instruction: MessageField::some(position(creation.instruction)),
            evidence: Some(match creation.evidence {
                EvidencePosition::Instruction(value) => pb::creation::Evidence::EventInstruction(Box::new(position(value))),
                EvidencePosition::Log { log_index } => pb::creation::Evidence::LogIndex(log_index),
            }),
        }),
    }
}

fn position(value: dex_pools::InstructionPosition) -> pb::InstructionPosition {
    pb::InstructionPosition {
        instruction_index: value.instruction_index,
        inner_instruction_index: value.inner_instruction_index,
    }
}

#[cfg(target_arch = "wasm32")]
mod handler {
    use super::*;
    use substreams::store::{StoreNew, StoreSetIfNotExistsProto};

    #[substreams::handlers::map]
    fn map_pool_creations(block: Block) -> Result<pb::Pools, substreams::errors::Error> {
        Ok(map_pool_creations_impl(&block)?)
    }

    #[substreams::handlers::map]
    fn map_pool_entries(pools: pb::Pools) -> Result<SinkEntries, substreams::errors::Error> {
        Ok(pool_entries(&pools))
    }

    #[substreams::handlers::store]
    fn store_pools(pools: pb::Pools, store: StoreSetIfNotExistsProto<pb::Pool>) {
        store_pools_impl(&pools, &store);
    }

    #[substreams::handlers::map]
    fn map_pools(deltas: Deltas<DeltaProto<pb::Pool>>) -> Result<pb::Pools, substreams::errors::Error> {
        Ok(map_pools_impl(deltas))
    }
}
