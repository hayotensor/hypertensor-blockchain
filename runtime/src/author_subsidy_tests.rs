//! Integration tests for verified payouts using the real Babe resolver and signed extrinsics.
use super::*;
use fp_account::EthereumSigner;
use frame_support::{
    __private::{sp_io, TestExternalities},
    assert_ok,
    traits::{Currency, Hooks},
};
use sp_core::{ecdsa, sr25519, Pair};
use sp_runtime::{transaction_validity::InvalidTransaction, BuildStorage, MultiSignature};

fn wallet() -> ecdsa::Pair {
    // Public Alith development key from the repository's test account documentation.
    ecdsa::Pair::from_seed(&hex_literal::hex!(
        "5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133"
    ))
}
fn account(key: &ecdsa::Pair) -> AccountId {
    EthereumSigner::from(key.public()).into_account()
}
fn babe() -> sr25519::Pair {
    sr25519::Pair::from_string("//Alice", None).unwrap()
}
fn new_ext() -> TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Runtime>::default()
        .build_storage()
        .unwrap();
    pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![(account(&wallet()), 100 * TENSOR)],
        ..Default::default()
    }
    .assimilate_storage(&mut storage)
    .unwrap();
    pallet_babe::GenesisConfig::<Runtime> {
        authorities: vec![(babe().public().into(), 1)],
        epoch_config: BABE_GENESIS_EPOCH_CONFIG,
        ..Default::default()
    }
    .assimilate_storage(&mut storage)
    .unwrap();
    storage.into()
}
fn start_block(number: u32, slot: Option<u64>) {
    System::initialize(
        &number,
        &H256::repeat_byte(42),
        &generic::Digest {
            logs: slot
                .into_iter()
                .map(|s| {
                    DigestItem::PreRuntime(
                        sp_consensus_babe::BABE_ENGINE_ID,
                        sp_consensus_babe::digests::PreDigest::SecondaryPlain(
                            sp_consensus_babe::digests::SecondaryPlainPreDigest {
                                authority_index: 0,
                                slot: s.into(),
                            },
                        )
                        .encode(),
                    )
                })
                .collect(),
        },
    );
    System::note_finished_initialize();
}
fn configure_call(destination: H160, nonce: u64) -> RuntimeCall {
    let proof = AuthorSubsidy::reward_address_payload(&babe().public(), destination, nonce, 100);
    RuntimeCall::AuthorSubsidy(pallet_author_subsidy::Call::set_reward_address {
        babe_key: babe().public(),
        reward_address: destination,
        nonce,
        valid_until: 100,
        babe_signature: babe().sign(&proof),
    })
}
fn signed(
    call: RuntimeCall,
    signer: &ecdsa::Pair,
    claimed_account: AccountId,
    nonce: u32,
) -> UncheckedExtrinsic {
    let extra: SignedExtra = (
        frame_system::CheckNonZeroSender::<Runtime>::new(),
        frame_system::CheckSpecVersion::<Runtime>::new(),
        frame_system::CheckTxVersion::<Runtime>::new(),
        frame_system::CheckGenesis::<Runtime>::new(),
        frame_system::CheckEra::<Runtime>::from(generic::Era::Immortal),
        frame_system::CheckNonce::<Runtime>::from(nonce),
        frame_system::CheckWeight::<Runtime>::new(),
        pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0),
    );
    let payload = SignedPayload::new(call.clone(), extra.clone()).unwrap();
    let sig =
        payload.using_encoded(|bytes| signer.sign_prehashed(&sp_io::hashing::keccak_256(bytes)));
    UncheckedExtrinsic::new_signed(
        call,
        claimed_account,
        EthereumSignature::from(MultiSignature::from(sig)),
        extra,
    )
}
fn resolved() -> Option<H160> {
    let digest = System::digest();
    FindAuthorRewardAddress::<Babe>::find_author(
        digest.logs.iter().filter_map(|d| d.as_pre_runtime()),
    )
}

#[test]
fn verified_alice_rewards_reach_alith_and_are_spendable_with_its_ethereum_key() {
    new_ext().execute_with(|| {
        start_block(1, Some(1));
        let key = wallet();
        let who = account(&key);
        let address: H160 = who.into();
        assert_eq!(
            address,
            H160::from(hex_literal::hex!(
                "f24ff3a9cf04c71dbc94d0b566f7a27b94566cac"
            ))
        );
        let call = configure_call(address, 0);
        assert_ok!(Executive::apply_extrinsic(signed(call, &key, who, 0)).unwrap());
        assert_eq!(resolved(), None);
        let issuance = Balances::total_issuance();
        AuthorSubsidy::on_initialize(1);
        assert_eq!(Balances::total_issuance(), issuance);
        start_block(2, Some(2));
        assert_eq!(resolved(), Some(address));
        assert_eq!(EVM::find_author(), address);
        let before = Balances::free_balance(who);
        let evm_before = EVM::account_basic(&address).0.balance;
        AuthorSubsidy::on_initialize(2);
        assert_eq!(Balances::free_balance(who), before + AUTHOR_BLOCK_EMISSIONS);
        assert_eq!(
            EVM::account_basic(&address).0.balance,
            evm_before + U256::from(AUTHOR_BLOCK_EMISSIONS)
        );
        assert_eq!(
            Balances::total_issuance(),
            issuance + AUTHOR_BLOCK_EMISSIONS
        );
        let destination: AccountId = H160::repeat_byte(55).into();
        let transfer = RuntimeCall::Balances(pallet_balances::Call::transfer_keep_alive {
            dest: destination,
            value: AUTHOR_BLOCK_EMISSIONS,
        });
        assert_ok!(Executive::apply_extrinsic(signed(transfer, &key, who, 1)).unwrap());
        assert_eq!(Balances::free_balance(destination), AUTHOR_BLOCK_EMISSIONS);
        let truncated = H160::from_slice(&babe().public().0[4..24]);
        assert_eq!(Balances::free_balance(AccountId::from(truncated)), 0);
        assert_eq!(Balances::free_balance(AccountId::from(H160::zero())), 0);
    });
}

#[test]
fn forged_ethereum_signature_is_rejected_before_payout_configuration() {
    new_ext().execute_with(|| {
        start_block(1, Some(1));
        let who = account(&wallet());
        let attacker = ecdsa::Pair::from_seed(&[7u8; 32]);
        let tx = signed(configure_call(who.into(), 0), &attacker, who, 0);
        assert_eq!(
            Executive::apply_extrinsic(tx),
            Err(InvalidTransaction::BadProof.into())
        );
        assert!(AuthorSubsidy::reward_addresses(babe().public()).is_none());
        assert_eq!(System::account_nonce(who), 0);
    });
}

#[test]
fn runtime_resolver_handles_missing_malformed_and_out_of_range_authors() {
    struct OutOfRange;
    impl FindAuthor<u32> for OutOfRange {
        fn find_author<'a, I>(_: I) -> Option<u32>
        where
            I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
        {
            Some(u32::MAX)
        }
    }
    new_ext().execute_with(|| {
        start_block(1, None);
        assert_eq!(resolved(), None);
        System::deposit_log(DigestItem::PreRuntime(
            sp_consensus_babe::BABE_ENGINE_ID,
            vec![1],
        ));
        assert_eq!(resolved(), None);
        assert_eq!(
            FindAuthorRewardAddress::<OutOfRange>::find_author(core::iter::empty()),
            None
        );
        start_block(2, Some(2));
        assert_eq!(resolved(), None); // Author exists, payout is not configured.
        pallet_babe::Authorities::<Runtime>::kill();
        assert_eq!(resolved(), None); // Empty authority sets cannot resolve an author.
    });
}

#[test]
fn evm_author_and_subsidy_use_the_same_destination_throughout_an_update_block() {
    new_ext().execute_with(|| {
        start_block(1, Some(1));
        let key = wallet();
        let old = account(&key);
        assert_ok!(
            Executive::apply_extrinsic(signed(configure_call(old.into(), 0), &key, old, 0))
                .unwrap()
        );
        start_block(2, Some(2));
        let new_key = ecdsa::Pair::from_seed(&[7u8; 32]);
        let new = account(&new_key);
        drop(Balances::deposit_creating(&new, TENSOR));
        assert_ok!(Executive::apply_extrinsic(signed(
            configure_call(new.into(), 1),
            &new_key,
            new,
            0
        ))
        .unwrap());
        assert_eq!(EVM::find_author(), H160::from(old));
        let old_before = Balances::free_balance(old);
        AuthorSubsidy::on_initialize(2);
        assert_eq!(
            Balances::free_balance(old),
            old_before + AUTHOR_BLOCK_EMISSIONS
        );
        start_block(3, Some(3));
        assert_eq!(EVM::find_author(), H160::from(new));
        let new_before = Balances::free_balance(new);
        AuthorSubsidy::on_initialize(3);
        assert_eq!(
            Balances::free_balance(new),
            new_before + AUTHOR_BLOCK_EMISSIONS
        );
    });
}
