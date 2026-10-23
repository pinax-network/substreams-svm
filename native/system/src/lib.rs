//! Shared system instruction extraction; see [`project`] for the native API.
mod native;
mod system;
pub use native::{project, project_with_limits, Limits, Operation, ProjectError, Projection};

#[cfg(any(feature = "substreams", test))]
use common::solana::{get_fee_payer, get_signers};
use proto::pb::solana::native::token::v1 as pb;
#[cfg(feature = "substreams")]
use substreams::errors::Error;
#[cfg(feature = "substreams")]
use substreams::log;
#[cfg(feature = "substreams")]
use substreams_solana::pb::sf::solana::r#type::v1::Block;
#[cfg(any(feature = "substreams", test))]
use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;

pub const SYSTEM_PROGRAM: [u8; 32] = [0; 32];

pub fn is_system_program(program_id: &[u8]) -> bool {
    program_id == SYSTEM_PROGRAM
}

#[cfg(feature = "substreams")]
#[substreams::handlers::map]
fn map_events(block: Block) -> Result<pb::Events, Error> {
    Ok(pb::Events {
        transactions: block.transactions_owned().filter_map(process_transaction).collect(),
    })
}

#[cfg(any(feature = "substreams", test))]
fn process_transaction(tx: ConfirmedTransaction) -> Option<pb::Transaction> {
    let tx_meta = tx.meta.as_option()?;
    let resolved_accounts = tx.resolved_accounts();

    let create_balances = |balances: &[u64], tag: &str| -> Vec<pb::Balance> {
        if balances.len() != resolved_accounts.len() {
            log::info!(
                "Skipping {tag} balances update: {tag}_balances invalid length {} != {}",
                balances.len(),
                resolved_accounts.len()
            );
            return Vec::new();
        }
        balances
            .iter()
            .zip(resolved_accounts.iter())
            .map(|(&amount, account)| pb::Balance {
                account: account.to_vec(),
                amount,
            })
            .collect()
    };

    let instructions: Vec<_> = tx
        .walk_instructions()
        .filter_map(|iview| {
            let program_id = iview.program_id().0;
            if !is_system_program(program_id) {
                return None;
            }

            system::unpack_transfers(&iview).map(|instruction| pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: iview.stack_height(),
                is_root: iview.is_root(),
                instruction: Some(instruction),
            })
        })
        .collect();

    let post_balances = create_balances(&tx_meta.post_balances, "post");
    let pre_balances = create_balances(&tx_meta.pre_balances, "pre");

    if instructions.is_empty() && post_balances.is_empty() && pre_balances.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed.unwrap_or_default(),
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(&tx).unwrap_or_default(),
        signers: get_signers(&tx).unwrap_or_default(),
        instructions,
        post_balances,
        pre_balances,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use buffa::{Message as _, MessageField};
    use substreams_solana::pb::sf::solana::r#type::v1 as sol;

    fn transfer_transaction() -> ConfirmedTransaction {
        ConfirmedTransaction {
            transaction: sol::Transaction {
                signatures: vec![vec![0x33; 64]],
                message: sol::Message {
                    header: sol::MessageHeader {
                        num_required_signatures: 1,
                        ..Default::default()
                    }
                    .into(),
                    account_keys: vec![vec![0x11; 32], vec![0x22; 32], SYSTEM_PROGRAM.to_vec()],
                    instructions: vec![sol::CompiledInstruction {
                        program_id_index: 2,
                        accounts: vec![0, 1],
                        // SystemInstruction::Transfer, fixed-width bincode, 42 lamports.
                        data: vec![2, 0, 0, 0, 42, 0, 0, 0, 0, 0, 0, 0],
                    }],
                    ..Default::default()
                }
                .into(),
            }
            .into(),
            meta: sol::TransactionStatusMeta {
                pre_balances: vec![100, 0, 0],
                post_balances: vec![58, 42, 0],
                ..Default::default()
            }
            .into(),
        }
    }

    #[test]
    fn native_transfer_preserves_legacy_oneof_wire_and_optional_scalar_default() {
        let decoded = process_transaction(transfer_transaction()).expect("decoded transfer");
        assert_eq!(decoded.compute_units_consumed, 0);
        assert_eq!(decoded.instructions.len(), 1);

        // Existing native-token.proto wire: program_id=1, is_root=3,
        // transfer=10 with source=1, destination=2, lamports=3.
        let mut expected = vec![0x0a, 32];
        expected.extend_from_slice(&SYSTEM_PROGRAM);
        expected.extend_from_slice(&[0x18, 1, 0x52, 70, 0x0a, 32]);
        expected.extend_from_slice(&[0x11; 32]);
        expected.extend_from_slice(&[0x12, 32]);
        expected.extend_from_slice(&[0x22; 32]);
        expected.extend_from_slice(&[0x18, 42]);
        assert_eq!(decoded.instructions[0].encode_to_vec(), expected);
        assert_eq!(pb::Instruction::decode_from_slice(&expected).unwrap(), decoded.instructions[0]);
    }

    #[test]
    fn missing_metadata_still_rejects_a_transaction_with_valid_transfer_bytes() {
        let mut tx = transfer_transaction();
        tx.meta = MessageField::none();
        assert!(process_transaction(tx).is_none());
    }

    fn block(tx: ConfirmedTransaction) -> sol::Block {
        sol::Block {
            slot: 2,
            parent_slot: 1,
            blockhash: substreams_solana::base58::encode([1; 32]),
            previous_blockhash: substreams_solana::base58::encode([2; 32]),
            transactions: vec![tx],
            ..Default::default()
        }
    }

    #[test]
    fn native_projection_preserves_original_positions_cpis_and_loaded_accounts() {
        let mut tx = transfer_transaction();
        let instruction = tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0].clone();
        tx.meta.as_option_mut().unwrap().inner_instructions.push(sol::InnerInstructions {
            index: 0,
            instructions: vec![sol::InnerInstruction {
                program_id_index: 2,
                accounts: vec![0, 1],
                data: instruction.data,
                stack_height: Some(2),
            }],
        });
        tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().version = Some(0);
        tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().versioned = true;
        let destination = tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().account_keys.remove(1);
        tx.transaction
            .as_option_mut()
            .unwrap()
            .message
            .as_option_mut()
            .unwrap()
            .address_table_lookups
            .push(sol::MessageAddressTableLookup {
                account_key: vec![4; 32],
                writable_indexes: vec![7],
                readonly_indexes: vec![],
            });
        tx.meta.as_option_mut().unwrap().loaded_writable_addresses.push(destination);
        tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0].program_id_index = 1;
        tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0].accounts = vec![0, 2];
        tx.meta.as_option_mut().unwrap().inner_instructions[0].instructions[0].program_id_index = 1;
        tx.meta.as_option_mut().unwrap().inner_instructions[0].instructions[0].accounts = vec![0, 2];
        tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions.insert(
            0,
            sol::CompiledInstruction {
                program_id_index: 0,
                ..Default::default()
            },
        );
        tx.meta.as_option_mut().unwrap().inner_instructions[0].index = 1;
        let mut block = block(tx.clone());
        let mut failed = tx;
        failed.meta.as_option_mut().unwrap().err = sol::TransactionError::default().into();
        block.transactions.insert(0, failed);
        let result = project(&block).unwrap();
        assert_eq!(result.operations.len(), 2);
        assert!(matches!(
            project_with_limits(
                &block,
                Limits {
                    max_operations: 1,
                    ..Limits::default()
                }
            ),
            Err(ProjectError::Capacity)
        ));
        for operation in &result.operations {
            assert_eq!(operation.transaction_index, 1);
            assert_eq!(operation.instruction_index, 1);
            assert!(std::ptr::eq(operation.transaction, &block.transactions[1]));
            let Some(pb::instruction::Instruction::Transfer(transfer)) = &operation.instruction.instruction else {
                panic!("transfer")
            };
            assert_eq!(transfer.destination, vec![0x22; 32]);
        }
        assert_eq!(result.operations[0].inner_instruction_index, None);
        assert_eq!(result.operations[1].inner_instruction_index, Some(0));
        assert_eq!(result.operations[1].instruction.stack_height, 2);
    }

    #[test]
    fn malformed_input_and_capacity_return_no_partial_projection() {
        let valid = block(transfer_transaction());
        for limits in [
            Limits {
                max_transactions: 0,
                ..Limits::default()
            },
            Limits {
                max_instructions: 0,
                ..Limits::default()
            },
            Limits {
                max_operations: 0,
                ..Limits::default()
            },
        ] {
            assert!(matches!(project_with_limits(&valid, limits), Err(ProjectError::Capacity)));
        }
        let mut malformed = valid.clone();
        malformed.transactions[0]
            .transaction
            .as_option_mut()
            .unwrap()
            .message
            .as_option_mut()
            .unwrap()
            .instructions[0]
            .accounts
            .clear();
        assert!(matches!(project(&malformed), Err(ProjectError::InvalidSystemInstruction { .. })));
        // The legacy wrapper now skips this malformed instruction instead of panicking.
        assert!(process_transaction(malformed.transactions.remove(0)).unwrap().instructions.is_empty());
        let mut invalid_index = valid.clone();
        invalid_index.transactions[0]
            .transaction
            .as_option_mut()
            .unwrap()
            .message
            .as_option_mut()
            .unwrap()
            .instructions[0]
            .accounts[0] = 255;
        assert!(matches!(project(&invalid_index), Err(ProjectError::InvalidBlock(_))));
        let mut missing = valid.clone();
        missing.transactions[0].meta = MessageField::none();
        assert!(matches!(project(&missing), Err(ProjectError::InvalidBlock(_))));
        let mut version = valid;
        version.transactions[0]
            .transaction
            .as_option_mut()
            .unwrap()
            .message
            .as_option_mut()
            .unwrap()
            .version = Some(1);
        assert!(matches!(project(&version), Err(ProjectError::UnsupportedMessage(_))));
    }

    #[test]
    fn every_supported_system_variant_shares_the_legacy_decoder() {
        use solana_program::{pubkey::Pubkey, system_instruction::SystemInstruction as S};
        for instruction in [
            S::Transfer { lamports: u64::MAX },
            S::TransferWithSeed {
                lamports: 43,
                from_owner: Pubkey::new_from_array([9; 32]),
                from_seed: "seed".into(),
            },
            S::CreateAccount {
                lamports: 44,
                space: 123,
                owner: Pubkey::new_from_array([8; 32]),
            },
            S::CreateAccountWithSeed {
                lamports: 45,
                space: 123,
                owner: Pubkey::new_from_array([8; 32]),
                base: Pubkey::new_from_array([9; 32]),
                seed: "seed".into(),
            },
            S::WithdrawNonceAccount(46),
        ] {
            let mut tx = transfer_transaction();
            tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0].accounts = vec![0, 1, 0, 1, 0];
            tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0].data =
                bincode::serde::encode_to_vec(&instruction, bincode::config::standard().with_fixed_int_encoding().with_little_endian()).unwrap();
            let legacy = process_transaction(tx.clone()).unwrap();
            let block = block(tx);
            let native = project(&block).unwrap();
            assert_eq!(native.operations.len(), 1);
            assert_eq!(native.operations[0].instruction, legacy.instructions[0]);
        }
    }

    #[test]
    fn zero_and_self_instruction_policy_is_explicit() {
        let mut tx = transfer_transaction();
        tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0].accounts = vec![0, 0];
        assert_eq!(project(&block(tx.clone())).unwrap().operations.len(), 1);
        tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0].data[4..].fill(0);
        assert!(project(&block(tx)).unwrap().operations.is_empty());
    }

    #[test]
    fn pinned_mainnet_block_uses_the_same_decoder_and_keeps_spl_available() {
        use std::io::Read;
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(include_bytes!("../../../spl/transfers/tests/fixtures/solana-mainnet-350000026.pb.gz").as_slice())
            .read_to_end(&mut bytes)
            .unwrap();
        let block = sol::Block::decode_from_slice(&bytes).unwrap();
        let result = project(&block).unwrap();
        assert!(!result.operations.is_empty());
        let expected: Vec<_> = block
            .transactions()
            .filter_map(|tx| process_transaction(tx.clone()))
            .flat_map(|tx| tx.instructions)
            .collect();
        assert_eq!(
            result.operations.iter().map(|op| &op.instruction).collect::<Vec<_>>(),
            expected.iter().collect::<Vec<_>>()
        );
        spl_transfers::block_view_buffa!(fn view, substreams_solana::pb::sf::solana::r#type::v1);
        let spl = spl_transfers::project(&view(&block).unwrap(), spl_transfers::Coverage::ALL).unwrap();
        assert!(!spl.operations.is_empty());
    }
}
