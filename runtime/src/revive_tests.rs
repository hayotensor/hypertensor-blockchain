use super::*;
use frame_support::{assert_ok, traits::fungible::InspectHold};
use pallet_revive::{
    evm::{runtime::EthExtra, Account, Transaction1559Unsigned},
    AddressMapper,
};
use sp_core::{sr25519, Pair, H160, U256};
use sp_runtime::traits::Header as _;

type Mapper = pallet_revive::AccountId32Mapper<Runtime>;
const CONTRACT_WEIGHT: Weight = Weight::from_parts(400_000_000_000, 1024 * 1024);

fn alice() -> sr25519::Pair {
    sr25519::Pair::from_string("//Alice", None).unwrap()
}

fn initialize_block() {
    let digest = sp_consensus_babe::digests::PreDigest::SecondaryPlain(
        sp_consensus_babe::digests::SecondaryPlainPreDigest {
            authority_index: 0,
            slot: 1.into(),
        },
    );
    let header = Header::new(
        1,
        H256::zero(),
        H256::zero(),
        H256::repeat_byte(1),
        generic::Digest {
            logs: vec![DigestItem::PreRuntime(
                sp_consensus_babe::BABE_ENGINE_ID,
                digest.encode(),
            )],
        },
    );
    Executive::initialize_block(&header);
    assert_ok!(Executive::apply_extrinsic(
        generic::UncheckedExtrinsic::new_bare(RuntimeCall::Timestamp(
            pallet_timestamp::Call::set { now: SLOT_DURATION }
        ),)
        .into()
    )
    .unwrap());
}

fn native_call(call: RuntimeCall) -> sp_runtime::DispatchOutcome {
    let pair = alice();
    let account = AccountId::from(pair.public());
    let mut extra = EthExtraImpl::get_eth_extension(System::account_nonce(&account), 0);
    // Native sr25519 transactions keep their signed origin.
    extra.9 = Default::default();
    let payload = SignedPayload::new(call.clone(), extra.clone()).unwrap();
    let signature = payload.using_encoded(|bytes| pair.sign(bytes));
    let extrinsic = generic::UncheckedExtrinsic::new_signed(call, account, signature.into(), extra);
    Executive::apply_extrinsic(extrinsic.into()).expect("valid native signed transaction")
}

fn map_alice() {
    assert_ok!(native_call(pallet_revive::Call::map_account {}.into()));
}

/// EVM constructor returning the supplied runtime bytecode, with no compiler dependency.
fn evm_init(code: &[u8]) -> Vec<u8> {
    assert!(code.len() < 256);
    let len = code.len() as u8;
    let mut init = vec![0x60, len, 0x60, 12, 0x60, 0, 0x39, 0x60, len, 0x60, 0, 0xf3];
    init.extend_from_slice(code);
    init
}

/// Writes calldata, caller and value into slots 0, 1 and 2; emits a log and returns calldata.
fn storage_contract() -> Vec<u8> {
    evm_init(&[
        0x60, 0, 0x35, 0x60, 0, 0x55, 0x33, 0x60, 1, 0x55, 0x34, 0x60, 2, 0x55, 0x60, 0, 0x35,
        0x60, 0, 0x52, 0x60, 1, 0x60, 32, 0x60, 0, 0xa1, 0x60, 32, 0x60, 0, 0xf3,
    ])
}

fn pvm_contract(trap_on_call: bool) -> Vec<u8> {
    use polkavm_common::{
        program::{asm, InstructionSetKind},
        writer::ProgramBlobBuilder,
    };
    let mut blob = ProgramBlobBuilder::new(InstructionSetKind::ReviveV1);
    blob.add_export_by_basic_block(0, b"deploy");
    blob.add_export_by_basic_block(1, b"call");
    blob.set_code(
        &[
            asm::ret(),
            if trap_on_call {
                asm::trap()
            } else {
                asm::ret()
            },
        ],
        &[],
    );
    blob.into_vec().unwrap()
}

fn deploy(code: Vec<u8>) -> H160 {
    assert_ok!(native_call(
        pallet_revive::Call::instantiate_with_code {
            value: 0,
            weight_limit: CONTRACT_WEIGHT,
            storage_deposit_limit: TENSOR,
            code,
            data: vec![],
            salt: None,
        }
        .into()
    ));
    System::events()
        .iter()
        .rev()
        .find_map(|record| match record.event {
            RuntimeEvent::Revive(pallet_revive::Event::Instantiated { contract, .. }) => {
                Some(contract)
            }
            _ => None,
        })
        .expect("contract instantiated")
}

fn contract_call(dest: H160, value: Balance, data: Vec<u8>) -> RuntimeCall {
    pallet_revive::Call::call {
        dest,
        value,
        data,
        weight_limit: CONTRACT_WEIGHT,
        storage_deposit_limit: TENSOR,
    }
    .into()
}

fn slot(address: H160, key: u8) -> U256 {
    let mut bytes = [0; 32];
    bytes[31] = key;
    Revive::get_storage(address, bytes)
        .unwrap()
        .map(|value| U256::from_big_endian(&value))
        .unwrap_or_default()
}

#[test]
fn native_sr25519_can_deploy_and_call_evm_with_the_same_balance_and_identity() {
    npos_tests::ext().execute_with(|| {
        initialize_block();
        let who = AccountId::from(alice().public());
        map_alice();
        assert_eq!(Mapper::to_account_id(&Mapper::to_address(&who)), who);
        let address = deploy(storage_contract());
        let contract_account = Mapper::to_account_id(&address);
        let before = Balances::free_balance(&contract_account);
        let nonce_before = System::account_nonce(&who);
        let mut input = vec![0; 32];
        input[31] = 42;
        assert_ok!(native_call(contract_call(address, TENSOR, input.clone())));
        assert_eq!(slot(address, 0), 42.into());
        assert_eq!(
            slot(address, 1),
            U256::from_big_endian(Mapper::to_address(&who).as_bytes())
        );
        assert_eq!(slot(address, 2), TENSOR.into());
        assert_eq!(Balances::free_balance(&contract_account) - before, TENSOR);
        assert_eq!(System::account_nonce(&who), nonce_before + 1);
        assert!(System::events().iter().any(|record| matches!(&record.event,
            RuntimeEvent::Revive(pallet_revive::Event::ContractEmitted { contract, data, .. })
                if *contract == address && *data == input
        )));
        Executive::finalize_block();
    });
}

#[test]
fn native_sr25519_can_execute_pvm_and_traps_revert_value() {
    for trap in [false, true] {
        npos_tests::ext().execute_with(|| {
            initialize_block();
            map_alice();
            let address = deploy(pvm_contract(trap));
            let account = Mapper::to_account_id(&address);
            let before = Balances::free_balance(&account);
            let result = native_call(contract_call(address, TENSOR, vec![]));
            if trap {
                assert!(result.is_err());
                assert_eq!(Balances::free_balance(&account), before);
            } else {
                assert_ok!(result);
                assert_eq!(Balances::free_balance(&account) - before, TENSOR);
            }
        });
    }
}

#[test]
fn reverted_evm_call_rolls_back_storage_and_value_but_charges_a_transaction_fee() {
    npos_tests::ext().execute_with(|| {
        initialize_block();
        map_alice();
        // SSTORE(0, 1), then REVERT(0, 0).
        let address = deploy(evm_init(&[0x60, 1, 0x60, 0, 0x55, 0x60, 0, 0x60, 0, 0xfd]));
        let account = Mapper::to_account_id(&address);
        let who = AccountId::from(alice().public());
        let contract_before = Balances::free_balance(&account);
        let caller_before = Balances::free_balance(&who);
        let nonce_before = System::account_nonce(&who);
        assert_eq!(
            native_call(contract_call(address, TENSOR, vec![])),
            Err(pallet_revive::Error::<Runtime>::ContractReverted.into())
        );
        assert_eq!(slot(address, 0), U256::zero());
        assert_eq!(Balances::free_balance(&account), contract_before);
        assert!(Balances::free_balance(&who) < caller_before);
        assert!(Balances::free_balance(&who) > caller_before - TENSOR);
        assert_eq!(System::account_nonce(&who), nonce_before + 1);
    });
}

#[test]
fn weight_and_storage_deposit_limits_prevent_state_changes() {
    for (weight_limit, storage_deposit_limit, expected) in [
        (
            Weight::zero(),
            TENSOR,
            pallet_revive::Error::<Runtime>::OutOfGas,
        ),
        (
            CONTRACT_WEIGHT,
            0,
            pallet_revive::Error::<Runtime>::StorageDepositLimitExhausted,
        ),
    ] {
        npos_tests::ext().execute_with(|| {
            initialize_block();
            map_alice();
            let address = deploy(storage_contract());
            let account = Mapper::to_account_id(&address);
            let before = Balances::free_balance(&account);
            let mut data = vec![0; 32];
            data[31] = 42;
            assert_eq!(
                native_call(
                    pallet_revive::Call::call {
                        dest: address,
                        value: TENSOR,
                        weight_limit,
                        storage_deposit_limit,
                        data,
                    }
                    .into()
                ),
                Err(expected.into())
            );
            assert_eq!(slot(address, 0), U256::zero());
            assert_eq!(Balances::free_balance(&account), before);
        });
    }
}

#[test]
fn native_mapping_deposit_is_refundable_and_unmapped_calls_are_rejected() {
    npos_tests::ext().execute_with(|| {
        initialize_block();
        let who = AccountId::from(alice().public());
        assert_eq!(
            native_call(contract_call(H160::repeat_byte(4), TENSOR, vec![])),
            Err(pallet_revive::Error::<Runtime>::AccountUnmapped.into())
        );
        map_alice();
        let reason = pallet_revive::HoldReason::AddressMapping.into();
        assert!(Balances::balance_on_hold(&reason, &who) > 0);
        assert_ok!(native_call(pallet_revive::Call::unmap_account {}.into()));
        assert_eq!(Balances::balance_on_hold(&reason, &who), 0);
        assert!(!Mapper::is_mapped(&who));
    });
}

fn eth_extrinsic(signer: &Account, to: H160, nonce: u32, chain_id: u64) -> UncheckedExtrinsic {
    let payload = signer
        .sign_transaction(
            Transaction1559Unsigned {
                chain_id: chain_id.into(),
                nonce: nonce.into(),
                to: Some(to),
                // Include the native byte fee and a refundable storage-deposit budget.
                gas: (U256::from(TENSOR) / Revive::evm_base_fee()),
                max_fee_per_gas: Revive::evm_base_fee(),
                value: TENSOR.into(),
                ..Default::default()
            }
            .into(),
        )
        .signed_payload();
    generic::UncheckedExtrinsic::new_bare(RuntimeCall::Revive(pallet_revive::Call::eth_transact {
        payload,
    }))
    .into()
}

fn validate_eth(
    extrinsic: UncheckedExtrinsic,
) -> sp_runtime::transaction_validity::TransactionValidity {
    // The node runs validation in a disposable overlay, including extension side effects.
    frame_support::storage::with_transaction_unchecked(|| {
        frame_support::storage::TransactionOutcome::Rollback(Executive::validate_transaction(
            sp_runtime::transaction_validity::TransactionSource::External,
            extrinsic,
            System::parent_hash(),
        ))
    })
}

#[test]
fn ethereum_signatures_use_native_balances_and_enforce_chain_id_and_nonce() {
    npos_tests::ext().execute_with(|| {
        initialize_block();
        let signer = Account::from_secret_key([7; 32]);
        assert_ok!(native_call(
            pallet_balances::Call::transfer_allow_death {
                dest: signer.substrate_account(),
                value: 10 * TENSOR,
            }
            .into()
        ));
        let dest = H160::repeat_byte(0x42);
        let chain_id = revive::ChainId::get();
        assert!(validate_eth(eth_extrinsic(&signer, dest, 0, chain_id + 1)).is_err());
        assert_ok!(validate_eth(eth_extrinsic(&signer, dest, 0, chain_id)));
        assert_ok!(Executive::apply_extrinsic(eth_extrinsic(&signer, dest, 0, chain_id)).unwrap());
        assert_eq!(Revive::evm_balance(&dest), TENSOR.into());
        assert_eq!(System::account_nonce(signer.substrate_account()), 1);
        assert!(validate_eth(eth_extrinsic(&signer, dest, 0, chain_id)).is_err());
        Executive::finalize_block();
        assert_eq!(Revive::eth_receipt_data().len(), 1);
    });
}

#[test]
fn ethereum_transactions_work_with_fractional_and_higher_fee_multipliers() {
    for multiplier in [Multiplier::from_rational(3, 2), Multiplier::from_u32(2)] {
        npos_tests::ext().execute_with(|| {
            initialize_block();
            pallet_transaction_payment::NextFeeMultiplier::<Runtime>::put(multiplier);
            let signer = Account::from_secret_key([9; 32]);
            let who = signer.substrate_account();
            assert_ok!(native_call(
                pallet_balances::Call::transfer_allow_death {
                    dest: who.clone(),
                    value: 10 * TENSOR,
                }
                .into()
            ));
            let before = Balances::free_balance(&who);
            let destination = H160::repeat_byte(0x43);
            let extrinsic = eth_extrinsic(&signer, destination, 0, revive::ChainId::get());
            assert_ok!(validate_eth(extrinsic.clone()));
            assert_ok!(Executive::apply_extrinsic(extrinsic).unwrap());
            assert_eq!(Revive::evm_balance(&destination), TENSOR.into());
            let spent = before - Balances::free_balance(&who);
            assert!(spent > TENSOR, "included transactions must pay fees");
            assert!(spent <= 2 * TENSOR, "value plus signed fee/deposit budget");
            Executive::finalize_block();
            assert_eq!(Revive::eth_receipt_data().len(), 1);
        });
    }
}

#[test]
fn ethereum_transaction_fits_after_maximum_network_initialization() {
    npos_tests::ext().execute_with(|| {
        initialize_block();
        let signer = Account::from_secret_key([10; 32]);
        assert_ok!(native_call(
            pallet_balances::Call::transfer_allow_death {
                dest: signer.substrate_account(),
                value: 10 * TENSOR,
            }
            .into()
        ));
        System::register_extra_weight_unchecked(
            MaximumHooksWeight::get(),
            frame_support::dispatch::DispatchClass::Mandatory,
        );
        let destination = H160::repeat_byte(0x44);
        assert_ok!(Executive::apply_extrinsic(eth_extrinsic(
            &signer,
            destination,
            0,
            revive::ChainId::get(),
        ))
        .unwrap());
        assert_eq!(Revive::evm_balance(&destination), TENSOR.into());
    });
}

#[test]
fn non_transfer_proxy_cannot_execute_contract_calls() {
    npos_tests::ext().execute_with(|| {
        initialize_block();
        let owner = AccountId::from(alice().public());
        let delegate = AccountId::from(sr25519::Pair::from_string("//Bob", None).unwrap().public());
        assert_ok!(native_call(
            pallet_proxy::Call::add_proxy {
                delegate: delegate.clone(),
                proxy_type: ProxyType::NonTransfer,
                delay: 0,
            }
            .into()
        ));
        let call = contract_call(H160::repeat_byte(0x42), TENSOR, vec![]);
        assert!(!ProxyType::NonTransfer.filter(&call));
        assert_ok!(Proxy::proxy(
            RuntimeOrigin::signed(delegate),
            owner,
            None,
            Box::new(call)
        ));
        assert!(System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::Proxy(pallet_proxy::Event::ProxyExecuted {
                result: Err(error)
            }) if error == frame_system::Error::<Runtime>::CallFiltered.into()
        )));
    });
}

#[test]
fn paused_contract_calls_are_filtered_for_native_and_ethereum_transactions() {
    npos_tests::ext().execute_with(|| {
        initialize_block();
        map_alice();
        let name = (
            b"Revive".to_vec().try_into().unwrap(),
            b"call".to_vec().try_into().unwrap(),
        );
        assert_ok!(TxPause::pause(RuntimeOrigin::root(), name));
        assert_eq!(
            native_call(contract_call(H160::repeat_byte(1), TENSOR, vec![])),
            Err(frame_system::Error::<Runtime>::CallFiltered.into())
        );

        let signer = Account::from_secret_key([8; 32]);
        assert_ok!(native_call(
            pallet_balances::Call::transfer_allow_death {
                dest: signer.substrate_account(),
                value: 10 * TENSOR,
            }
            .into()
        ));
        let name = (
            b"Revive".to_vec().try_into().unwrap(),
            b"eth_call".to_vec().try_into().unwrap(),
        );
        let dest = H160::repeat_byte(2);
        // Prove this transaction has sufficient fees and valid authorization
        // before pausing, so an unrelated rejection cannot satisfy the test.
        assert_ok!(validate_eth(eth_extrinsic(
            &signer,
            dest,
            0,
            revive::ChainId::get()
        )));
        assert_ok!(TxPause::pause(RuntimeOrigin::root(), name));
        assert!(Executive::apply_extrinsic(eth_extrinsic(
            &signer,
            dest,
            0,
            revive::ChainId::get()
        ))
        .map_or(true, |result| result.is_err()));
        assert_eq!(Revive::evm_balance(&dest), U256::zero());
    });
}
