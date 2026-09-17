use super::*;
use frame_support::{__private::TestExternalities, assert_ok, traits::PalletInfoAccess};
use pallet_evm::AddressMapping;
use sp_core::{ecdsa, Pair};
use sp_runtime::{traits::Header as HeaderT, BuildStorage};

fn authority(seed: &str) -> (AccountId, BabeId, GrandpaId) {
    genesis_config_presets::authority_keys_from_seed(seed)
}

fn merge(base: &mut serde_json::Value, patch: serde_json::Value) {
    match (base, patch) {
        (serde_json::Value::Object(base), serde_json::Value::Object(patch)) => {
            for (key, value) in patch {
                merge(base.entry(key).or_insert(serde_json::Value::Null), value);
            }
        }
        (base, patch) => *base = patch,
    }
}

fn from_preset(preset: serde_json::Value) -> TestExternalities {
    let mut value = serde_json::to_value(RuntimeGenesisConfig::default()).unwrap();
    merge(&mut value, preset);
    serde_json::from_value::<RuntimeGenesisConfig>(value)
        .unwrap()
        .build_storage()
        .unwrap()
        .into()
}

fn ext() -> TestExternalities {
    from_preset(genesis_config_presets::local_config_genesis())
}

fn validate_report(
    source: sp_runtime::transaction_validity::TransactionSource,
    extrinsic: UncheckedExtrinsic,
) -> sp_runtime::transaction_validity::TransactionValidity {
    // Runtime API calls run in a disposable overlay. Preserve that behavior
    // when calling Executive directly from native tests.
    frame_support::storage::with_transaction_unchecked(|| {
        frame_support::storage::TransactionOutcome::Rollback(Executive::validate_transaction(
            source,
            extrinsic,
            System::parent_hash(),
        ))
    })
}

fn block(number: u32) {
    let rotating =
        number > 1 && u64::from(number) >= *Babe::current_epoch_start() + EpochDuration::get();
    let author_count = if rotating {
        Session::queued_keys().len()
    } else {
        Session::validators().len()
    } as u32;
    let disabled = if rotating && pallet_session::QueuedChanged::<Runtime>::get() {
        vec![]
    } else {
        Session::disabled_validators()
    };
    let author_index = (0..author_count)
        .map(|offset| (number + offset) % author_count)
        .find(|index| !disabled.contains(index))
        .unwrap();
    block_with_author(number, author_index);
}

fn block_with_author(number: u32, author_index: u32) -> (Option<AccountId>, u32) {
    let digest = sp_consensus_babe::digests::PreDigest::SecondaryPlain(
        sp_consensus_babe::digests::SecondaryPlainPreDigest {
            authority_index: author_index,
            slot: (number as u64).into(),
        },
    );
    let header = Header::new(
        number,
        H256::zero(),
        H256::zero(),
        System::parent_hash(),
        generic::Digest {
            logs: vec![DigestItem::PreRuntime(
                sp_consensus_babe::BABE_ENGINE_ID,
                digest.encode(),
            )],
        },
    );
    Executive::initialize_block(&header);
    let author = Authorship::author();
    let era = Staking::active_era().unwrap().index;
    assert_ok!(
        Executive::apply_extrinsic(UncheckedExtrinsic::new_bare(RuntimeCall::Timestamp(
            pallet_timestamp::Call::set {
                now: number as u64 * SLOT_DURATION
            }
        ),))
        .unwrap()
    );
    Executive::finalize_block();
    (author, era)
}

fn advance_to_era(era: u32) -> bool {
    let limit =
        System::block_number() + (era + 2) * SessionsPerEra::get() * EpochDuration::get() as u32;
    let mut elected = false;
    while Staking::active_era().map_or(0, |e| e.index) < era {
        let next = System::block_number() + 1;
        assert!(next <= limit, "era transition stalled");
        block(next);
        elected |= System::events().iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::Staking(pallet_staking::Event::StakersElected)
            )
        });
    }
    elected
}

#[test]
fn presets_have_funded_h160_stakers_and_matching_consensus_keys() {
    for (preset, expected) in [
        (genesis_config_presets::development_config_genesis(), 1),
        (
            genesis_config_presets::ethereum_development_config_genesis(),
            1,
        ),
        (genesis_config_presets::local_config_genesis(), 2),
        (genesis_config_presets::hoskinson_config_genesis(), 4),
    ] {
        from_preset(preset).execute_with(|| {
            use sp_staking::currency_to_vote::CurrencyToVote;
            assert!(
                <Runtime as pallet_staking::Config>::CurrencyToVote::to_vote(
                    MIN_NOMINATOR_BOND,
                    Balances::total_issuance(),
                ) > 0,
                "minimum nominator bond must not round down to zero votes"
            );
            assert_eq!(Staking::validator_count(), expected);
            assert_eq!(Session::validators().len(), expected as usize);
            assert_eq!(Babe::authorities().len(), expected as usize);
            assert_eq!(Grandpa::grandpa_authorities().len(), expected as usize);
            assert_eq!(<EVMChainId as Get<u64>>::get(), 42);
            assert!(pallet_staking::Invulnerables::<Runtime>::get().is_empty());
            assert_eq!(
                pallet_staking::MaxValidatorsCount::<Runtime>::get(),
                Some(MAX_VALIDATORS)
            );
            assert_eq!(
                pallet_staking::MaxNominatorsCount::<Runtime>::get(),
                Some(MAX_NOMINATORS)
            );
            for (index, account) in Session::validators().iter().enumerate() {
                let keys = pallet_session::NextKeys::<Runtime>::get(account).unwrap();
                assert_eq!(keys.babe, Babe::authorities()[index].0);
                assert_eq!(keys.grandpa, Grandpa::grandpa_authorities()[index].0);
                assert_eq!(account.encode().len(), 20);
                assert!(Balances::free_balance(account) > VALIDATOR_BOND);
                let ledger = pallet_staking::Ledger::<Runtime>::get(account).unwrap();
                assert_eq!(ledger.stash, *account);
                assert_eq!(ledger.active, VALIDATOR_BOND);
                assert_eq!(
                    pallet_staking::Bonded::<Runtime>::get(account),
                    Some(*account)
                );
            }
        });
    }
}

#[test]
fn account_identity_keys_and_existing_pallet_indices_are_preserved() {
    use sp_staking::currency_to_vote::CurrencyToVote;
    assert!(
        <Runtime as pallet_staking::Config>::CurrencyToVote::to_vote(
            MIN_NOMINATOR_BOND,
            Balance::MAX,
        ) > 0
    );
    let (account, babe, grandpa) = authority("Alice");
    let h160: H160 = account.into();
    assert_eq!(core::mem::size_of::<AccountId>(), 20);
    assert_eq!(
        <IdentityAddressMapping as AddressMapping<AccountId>>::into_account_id(h160),
        account
    );
    let keys = opaque::SessionKeys { babe, grandpa };
    let encoded = keys.encode();
    assert_eq!(encoded.len(), 64);
    assert_eq!(
        opaque::SessionKeys::decode(&mut &encoded[..]).unwrap(),
        keys
    );
    let raw = opaque::SessionKeys::decode_into_raw_public_keys(&encoded).unwrap();
    assert_eq!(
        raw.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
        vec![sp_consensus_babe::KEY_TYPE, sp_consensus_grandpa::KEY_TYPE]
    );
    assert_eq!(
        FrontierPrecompiles::<Runtime>::used_addresses().to_vec(),
        [1, 2, 3, 4, 5, 1024, 1025, 2048, 2049, 2050, 2051]
            .map(H160::from_low_u64_be)
            .to_vec()
    );
    let indices = [
        System::index(),
        Timestamp::index(),
        Grandpa::index(),
        Balances::index(),
        TransactionPayment::index(),
        Sudo::index(),
        Ethereum::index(),
        EVM::index(),
        EVMChainId::index(),
        BaseFee::index(),
        AtomicSwap::index(),
        InsecureRandomnessCollectiveFlip::index(),
        Utility::index(),
        Proxy::index(),
        Preimage::index(),
        Scheduler::index(),
        Treasury::index(),
        Multisig::index(),
        TxPause::index(),
        Collective::index(),
        Network::index(),
        AuthorSubsidy::index(),
    ];
    assert_eq!(
        indices,
        [0, 1, 3, 4, 5, 6, 7, 8, 9, 10, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24]
    );
}

#[test]
fn ethereum_transaction_recovery_keeps_the_alith_address() {
    use fp_self_contained::SelfContainedCall;
    let key = ecdsa::Pair::from_seed(&hex_literal::hex!(
        "5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133"
    ));
    let message = ethereum::LegacyTransactionMessage {
        nonce: U256::zero(),
        gas_price: U256::from(1_000_000_000u64),
        gas_limit: U256::from(21_000),
        action: ethereum::TransactionAction::Call(H160::repeat_byte(7)),
        value: U256::from(1),
        input: vec![],
        chain_id: Some(42),
    };
    let signature = key.sign_prehashed(message.hash().as_fixed_bytes());
    let transaction = ethereum::LegacyTransaction {
        nonce: message.nonce,
        gas_price: message.gas_price,
        gas_limit: message.gas_limit,
        action: message.action,
        value: message.value,
        input: message.input,
        signature: ethereum::TransactionSignature::new(
            42 * 2 + 35 + signature.0[64] as u64,
            H256::from_slice(&signature.0[..32]),
            H256::from_slice(&signature.0[32..64]),
        )
        .unwrap(),
    };
    let call = RuntimeCall::Ethereum(pallet_ethereum::Call::transact {
        transaction: transaction.into(),
    });
    ext().execute_with(|| {
        let recovered = call.check_self_contained().unwrap().unwrap();
        assert_eq!(recovered, H160::from(authority("Alice").0));
        assert_eq!(
            <IdentityAddressMapping as AddressMapping<AccountId>>::into_account_id(recovered),
            authority("Alice").0
        );
    });
}

#[test]
fn nominations_elections_rewards_and_unbonding_work_across_eras() {
    ext().execute_with(|| {
        block(1);
        let alice = authority("Alice").0;
        let nominator = authority("Charlie").0;
        let stake = 100 * TENSOR;
        assert_ok!(Staking::bond(
            RuntimeOrigin::signed(nominator),
            stake,
            pallet_staking::RewardDestination::Stash
        ));
        assert_ok!(Staking::bond_extra(
            RuntimeOrigin::signed(nominator),
            10 * TENSOR
        ));
        assert_ok!(Staking::nominate(
            RuntimeOrigin::signed(nominator),
            vec![alice]
        ));
        advance_to_era(1);
        let exposure = Staking::eras_stakers(1, &alice);
        let entry = exposure
            .others
            .iter()
            .find(|entry| entry.who == nominator)
            .expect("the nominator must be in the elected exposure");
        // U128CurrencyToVote scales native balances into u64 votes; converting
        // back may lose at most one unit of that scale.
        assert!(entry.value <= 110 * TENSOR);
        assert!(110 * TENSOR - entry.value <= Balances::total_issuance() / u64::MAX as u128 + 1);
        assert!(Session::current_index() >= SessionsPerEra::get());
        assert!(advance_to_era(2));
        let issuance = Balances::total_issuance();
        let balance = Balances::free_balance(nominator);
        assert!(pallet_staking::ErasValidatorReward::<Runtime>::get(1).unwrap() > 0);
        assert_ok!(Staking::payout_stakers(
            RuntimeOrigin::signed(nominator),
            alice,
            1
        ));
        assert!(Balances::free_balance(nominator) > balance);
        assert!(Balances::total_issuance() > issuance);
        assert_ok!(Staking::chill(RuntimeOrigin::signed(nominator)));
        assert_ok!(Staking::unbond(
            RuntimeOrigin::signed(nominator),
            110 * TENSOR
        ));
        advance_to_era(2 + BondingDuration::get());
        assert_ok!(Staking::withdraw_unbonded(
            RuntimeOrigin::signed(nominator),
            0
        ));
        assert!(!pallet_staking::Bonded::<Runtime>::contains_key(nominator));
        assert!(System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::Staking(pallet_staking::Event::Withdrawn { .. })
        )));
    });
}

#[test]
fn staking_intent_controls_future_session_and_consensus_authorities() {
    ext().execute_with(|| {
        block(1);
        let (charlie, babe, grandpa) = authority("Charlie");
        let alice = authority("Alice").0;
        assert_ok!(Staking::bond(
            RuntimeOrigin::signed(charlie),
            VALIDATOR_BOND,
            pallet_staking::RewardDestination::Staked
        ));
        assert_ok!(Session::set_keys(
            RuntimeOrigin::signed(charlie),
            opaque::SessionKeys {
                babe: babe.clone(),
                grandpa: grandpa.clone()
            },
            vec![]
        ));
        assert_ok!(Staking::validate(
            RuntimeOrigin::signed(charlie),
            Default::default()
        ));
        assert_ok!(Staking::chill(RuntimeOrigin::signed(alice)));
        advance_to_era(2);
        assert!(!Session::validators().contains(&alice));
        assert!(Session::validators().contains(&charlie));
        assert!(Babe::authorities().iter().any(|(key, _)| key == &babe));
        assert!(Grandpa::grandpa_authorities()
            .iter()
            .any(|(key, _)| key == &grandpa));
        assert!(Staking::current_era().unwrap() >= 2);
    });
}

#[test]
fn first_block_of_new_validator_set_rewards_its_actual_author_in_the_new_era() {
    ext().execute_with(|| {
        block(1);
        let (charlie, babe, grandpa) = authority("Charlie");
        assert_ok!(Staking::bond(
            RuntimeOrigin::signed(charlie),
            VALIDATOR_BOND,
            pallet_staking::RewardDestination::Staked
        ));
        assert_ok!(Session::set_keys(
            RuntimeOrigin::signed(charlie),
            opaque::SessionKeys { babe, grandpa },
            vec![]
        ));
        assert_ok!(Staking::validate(
            RuntimeOrigin::signed(charlie),
            Default::default()
        ));
        assert_ok!(Staking::chill(RuntimeOrigin::signed(authority("Alice").0)));

        // Wait until the election has queued Charlie, but his set is not active.
        while !Session::queued_keys()
            .iter()
            .any(|(who, _)| *who == charlie)
        {
            assert!(System::block_number() < 100);
            block(System::block_number() + 1);
        }
        assert!(!Session::validators().contains(&charlie));
        let boundary = (*Babe::current_epoch_start() + EpochDuration::get()) as u32;
        while System::block_number() + 1 < boundary {
            block(System::block_number() + 1);
        }
        let old_era = Staking::active_era().unwrap().index;
        let old_points = Staking::eras_reward_points(old_era);
        let index = Session::queued_keys()
            .iter()
            .position(|(who, _)| *who == charlie)
            .unwrap();
        let (author, era) = block_with_author(boundary, index as u32);
        assert_eq!(author, Some(charlie));
        assert_eq!(era, old_era + 1);
        assert_eq!(Staking::eras_reward_points(old_era), old_points);
        assert_eq!(
            Staking::eras_reward_points(era).individual.get(&charlie),
            Some(&20)
        );
    });
}

#[test]
fn both_consensus_keys_have_historical_ownership_proofs() {
    ext().execute_with(|| {
        block(1);
        let (alice, babe, grandpa) = authority("Alice");
        let babe_proof = Historical::prove((sp_consensus_babe::KEY_TYPE, babe.clone())).unwrap();
        let grandpa_proof =
            Historical::prove((sp_consensus_grandpa::KEY_TYPE, grandpa.clone())).unwrap();
        assert_eq!(
            Historical::check_proof((sp_consensus_babe::KEY_TYPE, babe), babe_proof)
                .unwrap()
                .0,
            alice
        );
        assert_eq!(
            Historical::check_proof((sp_consensus_grandpa::KEY_TYPE, grandpa), grandpa_proof)
                .unwrap()
                .0,
            alice
        );
    });
}

#[test]
fn rotating_keys_preserves_historical_proofs_for_both_consensus_engines() {
    ext().execute_with(|| {
        block(1);
        let (alice, old_babe, old_grandpa) = authority("Alice");
        let babe_proof =
            Historical::prove((sp_consensus_babe::KEY_TYPE, old_babe.clone())).unwrap();
        let grandpa_proof =
            Historical::prove((sp_consensus_grandpa::KEY_TYPE, old_grandpa.clone())).unwrap();
        let new_babe = sp_consensus_babe::AuthorityPair::from_string("//Alice//rotation", None)
            .unwrap()
            .public();
        let new_grandpa =
            sp_consensus_grandpa::AuthorityPair::from_string("//Alice//rotation", None)
                .unwrap()
                .public();
        assert_ok!(Session::set_keys(
            RuntimeOrigin::signed(alice),
            opaque::SessionKeys {
                babe: new_babe.clone(),
                grandpa: new_grandpa.clone(),
            },
            vec![]
        ));
        // Publishing replacement keys must not change the active epoch early.
        assert!(Babe::authorities().iter().any(|(key, _)| key == &old_babe));
        assert!(Grandpa::grandpa_authorities()
            .iter()
            .any(|(key, _)| key == &old_grandpa));
        advance_to_era(1);
        assert!(Babe::authorities().iter().any(|(key, _)| key == &new_babe));
        assert!(Grandpa::grandpa_authorities()
            .iter()
            .any(|(key, _)| key == &new_grandpa));
        assert!(Session::validators().contains(&alice));
        assert_eq!(
            Historical::check_proof((sp_consensus_babe::KEY_TYPE, old_babe), babe_proof)
                .unwrap()
                .0,
            alice
        );
        assert_eq!(
            Historical::check_proof((sp_consensus_grandpa::KEY_TYPE, old_grandpa), grandpa_proof)
                .unwrap()
                .0,
            alice
        );
        assert!(Historical::prove((sp_consensus_babe::KEY_TYPE, new_babe)).is_some());
        assert!(Historical::prove((sp_consensus_grandpa::KEY_TYPE, new_grandpa)).is_some());
    });
}

#[test]
fn babe_equivocation_reaches_offences_and_deferred_staking_slashes() {
    use sp_consensus_babe::digests::CompatibleDigestItem;
    use sp_core::offchain::{testing::TestTransactionPoolExt, TransactionPoolExt};
    use sp_runtime::transaction_validity::TransactionSource;
    let mut externalities = ext();
    let (pool, state) = TestTransactionPoolExt::new();
    externalities.register_extension(TransactionPoolExt::new(pool));
    externalities.execute_with(|| {
        block(1);
        let (alice, babe, _) = authority("Alice");
        let pair = sp_consensus_babe::AuthorityPair::from_string("//Alice", None).unwrap();
        let proof = Historical::prove((sp_consensus_babe::KEY_TYPE, babe.clone())).unwrap();
        let author_index = Babe::authorities()
            .iter()
            .position(|(key, _)| key == &babe)
            .unwrap() as u32;
        let header = |root| {
            let pre = sp_consensus_babe::digests::PreDigest::SecondaryPlain(
                sp_consensus_babe::digests::SecondaryPlainPreDigest {
                    authority_index: author_index,
                    slot: 1u64.into(),
                },
            );
            let mut header = Header::new(
                1,
                H256::repeat_byte(root),
                H256::zero(),
                H256::zero(),
                generic::Digest {
                    logs: vec![DigestItem::babe_pre_digest(pre)],
                },
            );
            let signature = pair.sign(header.hash().as_ref());
            header.digest_mut().push(DigestItem::babe_seal(signature));
            header
        };
        let equivocation = sp_consensus_babe::EquivocationProof {
            offender: babe,
            slot: 1u64.into(),
            first_header: header(1),
            second_header: header(2),
        };
        assert!(sp_consensus_babe::check_equivocation_proof(
            equivocation.clone()
        ));
        assert_eq!(
            Babe::submit_unsigned_equivocation_report(equivocation.clone(), proof.clone()),
            Some(())
        );
        let submitted = state.write().transactions.pop().unwrap();
        let unsigned = UncheckedExtrinsic::decode(&mut &submitted[..]).unwrap();
        let validity = validate_report(TransactionSource::Local, unsigned.clone()).unwrap();
        assert!(!validity.propagate);
        assert_eq!(validity.longevity, ReportLongevity::get());
        assert!(validate_report(TransactionSource::External, unsigned.clone()).is_err());

        let mut invalid = equivocation.clone();
        invalid.second_header = invalid.first_header.clone();
        frame_support::assert_noop!(
            Babe::report_equivocation(
                RuntimeOrigin::signed(authority("Charlie").0),
                Box::new(invalid),
                proof.clone()
            ),
            pallet_babe::Error::<Runtime>::InvalidEquivocationProof
        );
        assert_ok!(Babe::report_equivocation(
            RuntimeOrigin::signed(authority("Charlie").0),
            Box::new(equivocation),
            proof
        ));
        assert!(validate_report(TransactionSource::Local, unsigned).is_err());
        assert!(System::events()
            .iter()
            .any(|record| matches!(record.event, RuntimeEvent::Offences(_))));
        assert_eq!(
            pallet_staking::Ledger::<Runtime>::get(alice)
                .unwrap()
                .active,
            VALIDATOR_BOND
        );
        advance_to_era(SlashDeferDuration::get() + 1);
        assert!(
            pallet_staking::Ledger::<Runtime>::get(alice)
                .unwrap()
                .active
                < VALIDATOR_BOND
        );
    });
}

#[test]
fn grandpa_equivocation_queues_a_slash_for_the_h160_validator() {
    use sp_core::offchain::{testing::TestTransactionPoolExt, TransactionPoolExt};
    use sp_runtime::transaction_validity::TransactionSource;
    let mut externalities = ext();
    let (pool, state) = TestTransactionPoolExt::new();
    externalities.register_extension(TransactionPoolExt::new(pool));
    externalities.execute_with(|| {
        block(1);
        let (alice, _, grandpa) = authority("Alice");
        let pair = sp_consensus_grandpa::AuthorityPair::from_string("//Alice", None).unwrap();
        let ownership =
            Historical::prove((sp_consensus_grandpa::KEY_TYPE, grandpa.clone())).unwrap();
        let set_id = Grandpa::current_set_id();
        let round = 1u64;
        let vote = |root| {
            let prevote = sp_consensus_grandpa::Prevote::<Header> {
                target_hash: H256::repeat_byte(root),
                target_number: 1,
            };
            let payload = sp_consensus_grandpa::localized_payload(
                round,
                set_id,
                &sp_consensus_grandpa::Message::<Header>::Prevote(prevote.clone()),
            );
            (prevote, pair.sign(&payload))
        };
        // SCALE prevote equivocation: variant, round, identity, two signed votes.
        let encoded = (0u8, round, grandpa, vote(1), vote(2)).encode();
        let equivocation = sp_consensus_grandpa::Equivocation::decode(&mut &encoded[..]).unwrap();
        let proof = sp_consensus_grandpa::EquivocationProof::new(set_id, equivocation);
        assert!(sp_consensus_grandpa::check_equivocation_proof(
            proof.clone()
        ));
        assert_eq!(
            Grandpa::submit_unsigned_equivocation_report(proof.clone(), ownership.clone()),
            Some(())
        );
        let submitted = state.write().transactions.pop().unwrap();
        let unsigned = UncheckedExtrinsic::decode(&mut &submitted[..]).unwrap();
        let validity = validate_report(TransactionSource::Local, unsigned.clone()).unwrap();
        assert!(!validity.propagate);
        assert_eq!(validity.longevity, ReportLongevity::get());
        assert!(validate_report(TransactionSource::External, unsigned.clone()).is_err());
        assert_ok!(Grandpa::report_equivocation(
            RuntimeOrigin::signed(authority("Charlie").0),
            Box::new(proof),
            ownership
        ));
        assert!(validate_report(TransactionSource::Local, unsigned).is_err());
        assert!(System::events()
            .iter()
            .any(|record| matches!(record.event, RuntimeEvent::Offences(_))));
        assert!(pallet_staking::UnappliedSlashes::<Runtime>::iter_values()
            .any(|slashes| !slashes.is_empty()));
        assert!(System::events().iter().any(|record| matches!(
            &record.event,
            RuntimeEvent::Staking(pallet_staking::Event::SlashReported { validator, .. })
                if *validator == alice
        )));
        assert_eq!(
            pallet_staking::Ledger::<Runtime>::get(alice)
                .unwrap()
                .active,
            VALIDATOR_BOND
        );
    });
}

#[test]
fn election_at_registration_caps_includes_every_voter_within_weight_budget() {
    use frame_election_provider_support::ElectionProvider;
    use frame_support::{assert_noop, traits::Currency};
    use std::collections::BTreeSet;
    ext().execute_with(|| {
        block(1);
        let mut candidates = vec![authority("Alice").0, authority("Bob").0];
        let fund_and_bond = |index, value| {
            let account: AccountId = H160::from_low_u64_be(index).into();
            drop(Balances::deposit_creating(&account, 2 * VALIDATOR_BOND));
            assert_ok!(Staking::bond(
                RuntimeOrigin::signed(account),
                value,
                pallet_staking::RewardDestination::Stash
            ));
            account
        };
        for index in 2..MAX_VALIDATORS {
            let account = fund_and_bond(10_000 + index as u64, VALIDATOR_BOND);
            assert_ok!(Session::set_keys(
                RuntimeOrigin::signed(account),
                opaque::SessionKeys {
                    babe: sp_consensus_babe::AuthorityPair::from_seed(&[index as u8; 32]).public(),
                    grandpa: sp_consensus_grandpa::AuthorityPair::from_seed(&[index as u8; 32])
                        .public(),
                },
                vec![]
            ));
            assert_ok!(Staking::validate(
                RuntimeOrigin::signed(account),
                Default::default()
            ));
            candidates.push(account);
        }
        let extra = fund_and_bond(99_999, VALIDATOR_BOND);
        assert_noop!(
            Staking::validate(RuntimeOrigin::signed(extra), Default::default()),
            pallet_staking::Error::<Runtime>::TooManyValidators
        );
        let targets = candidates[..MAX_NOMINATIONS as usize].to_vec();
        for index in 0..MAX_NOMINATORS {
            let account = fund_and_bond(20_000 + index as u64, 100 * TENSOR);
            assert_ok!(Staking::nominate(
                RuntimeOrigin::signed(account),
                targets.clone()
            ));
        }
        assert_noop!(
            Staking::nominate(RuntimeOrigin::signed(extra), targets),
            pallet_staking::Error::<Runtime>::TooManyNominators
        );
        assert_ok!(Staking::set_validator_count(
            RuntimeOrigin::root(),
            MAX_VALIDATORS
        ));
        let supports = <Runtime as pallet_staking::Config>::ElectionProvider::elect().unwrap();
        assert_eq!(supports.len(), MAX_VALIDATORS as usize);
        let voters: BTreeSet<_> = supports
            .iter()
            .flat_map(|(_, support)| support.voters.iter().map(|(account, _)| *account))
            .collect();
        assert_eq!(voters.len(), (MAX_VALIDATORS + MAX_NOMINATORS) as usize);
        assert!(System::block_weight().total().ref_time() < MAXIMUM_BLOCK_WEIGHT.ref_time() / 2);

        // Exercise the actual hooks, queued set, and full paged historical
        // exposure, rather than stopping at an isolated solver result.
        advance_to_era(1);
        assert_eq!(Session::validators().len(), MAX_VALIDATORS as usize);
        assert_eq!(Babe::authorities().len(), MAX_VALIDATORS as usize);
        assert_eq!(
            Grandpa::grandpa_authorities().len(),
            MAX_VALIDATORS as usize
        );
        let (_, babe, _) = authority("Alice");
        let proof = Historical::prove((sp_consensus_babe::KEY_TYPE, babe.clone())).unwrap();
        let (owner, exposure) =
            Historical::check_proof((sp_consensus_babe::KEY_TYPE, babe), proof).unwrap();
        assert_eq!(owner, authority("Alice").0);
        assert_eq!(exposure.others.len(), MAX_NOMINATORS as usize);
    });
}

#[test]
fn oversized_registration_fails_election_instead_of_silently_dropping_votes() {
    use frame_election_provider_support::ElectionProvider;
    use frame_support::traits::Currency;
    use pallet_staking::ConfigOp;
    ext().execute_with(|| {
        block(1);
        // Root can remove the SDK's mutable registration cap. The election
        // must still enforce the runtime's tested bounds on the unsorted map.
        assert_ok!(Staking::set_staking_configs(
            RuntimeOrigin::root(),
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Remove,
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Noop,
        ));
        for index in 0..MAX_NOMINATORS + MAX_VALIDATORS - 1 {
            let account: AccountId = H160::from_low_u64_be(30_000 + index as u64).into();
            drop(Balances::deposit_creating(&account, 100 * TENSOR));
            assert_ok!(Staking::bond(
                RuntimeOrigin::signed(account),
                MIN_NOMINATOR_BOND,
                pallet_staking::RewardDestination::Stash
            ));
            assert_ok!(Staking::nominate(
                RuntimeOrigin::signed(account),
                vec![authority("Alice").0]
            ));
        }
        assert!(<Runtime as pallet_staking::Config>::ElectionProvider::elect().is_err());
        let validators = Session::validators();
        for number in 2..=1 + EpochDuration::get() as u32 * SessionsPerEra::get() {
            block(number);
        }
        assert_eq!(Session::validators(), validators);
        assert_eq!(Staking::active_era().unwrap().index, 0);
        assert!(System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::Staking(pallet_staking::Event::StakingElectionFailed)
        )));
        for index in MAX_NOMINATORS..MAX_NOMINATORS + MAX_VALIDATORS - 1 {
            let account: AccountId = H160::from_low_u64_be(30_000 + index as u64).into();
            assert_ok!(Staking::chill(RuntimeOrigin::signed(account)));
        }
        assert_ok!(Staking::set_staking_configs(
            RuntimeOrigin::root(),
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Set(MAX_NOMINATORS),
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Noop,
        ));
        assert!(advance_to_era(1));
        assert_eq!(
            Staking::eras_stakers(1, &authority("Alice").0).others.len(),
            MAX_NOMINATORS as usize
        );
    });
}
