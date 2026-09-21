use super::*;
use frame_support::{__private::TestExternalities, assert_ok};
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{proof_of_possession::ProofOfPossessionGenerator, Pair};
use sp_runtime::{traits::Header as HeaderT, BuildStorage};

fn authority(seed: &str) -> (AccountId, BabeId, GrandpaId) {
    genesis_config_presets::authority_keys_from_seed(seed)
}

fn key_proof(
    owner: &AccountId,
    mut babe: sp_core::sr25519::Pair,
    mut grandpa: sp_core::ed25519::Pair,
) -> Vec<u8> {
    let owner = owner.encode();
    (
        babe.generate_proof_of_possession(&owner),
        grandpa.generate_proof_of_possession(&owner),
    )
        .encode()
}

fn seed_key_proof(owner: &AccountId, seed: &str) -> Vec<u8> {
    key_proof(
        owner,
        sp_core::sr25519::Pair::from_string(seed, None).unwrap(),
        sp_core::ed25519::Pair::from_string(seed, None).unwrap(),
    )
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

pub(super) fn ext() -> TestExternalities {
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
    assert_ok!(Executive::apply_extrinsic(
        generic::UncheckedExtrinsic::new_bare(RuntimeCall::Timestamp(
            pallet_timestamp::Call::set {
                now: number as u64 * SLOT_DURATION
            }
        ),)
        .into()
    )
    .unwrap());
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
fn presets_have_funded_native_stakers_and_matching_consensus_keys() {
    for (preset, expected) in [
        (genesis_config_presets::development_config_genesis(), 1),
        (genesis_config_presets::local_config_genesis(), 2),
        (genesis_config_presets::four_validator_test_genesis(), 4),
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
                assert_eq!(account.encode().len(), 32);
                assert!(Balances::free_balance(account) > VALIDATOR_BOND);
                let ledger = pallet_staking::Ledger::<Runtime>::get(&account).unwrap();
                assert_eq!(ledger.stash, *account);
                assert_eq!(ledger.active, VALIDATOR_BOND);
                assert_eq!(
                    pallet_staking::Bonded::<Runtime>::get(account),
                    Some(account.clone())
                );
            }
        });
    }
}

#[test]
fn native_account_identity_and_consensus_keys_are_valid() {
    use sp_staking::currency_to_vote::CurrencyToVote;
    assert!(
        <Runtime as pallet_staking::Config>::CurrencyToVote::to_vote(
            MIN_NOMINATOR_BOND,
            Balance::MAX,
        ) > 0
    );
    let (account, babe, grandpa) = authority("Alice");
    assert_eq!(
        account,
        sp_runtime::AccountId32::from(
            sp_core::sr25519::Pair::from_string("//Alice", None)
                .unwrap()
                .public()
        )
    );
    assert_eq!(core::mem::size_of::<AccountId>(), 32);
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
}

#[test]
fn session_keys_require_owner_bound_proofs_and_refund_the_key_deposit() {
    use frame_support::{assert_noop, traits::fungible::InspectHold};
    ext().execute_with(|| {
        block(1);
        let (charlie, babe, grandpa) = authority("Charlie");
        let keys = opaque::SessionKeys { babe, grandpa };
        let reason = pallet_session::HoldReason::Keys.into();
        assert_noop!(
            Session::set_keys(RuntimeOrigin::signed(charlie.clone()), keys.clone(), vec![]),
            pallet_session::Error::<Runtime>::InvalidProof
        );
        assert_noop!(
            Session::set_keys(
                RuntimeOrigin::signed(charlie.clone()),
                keys.clone(),
                seed_key_proof(&authority("Dave").0, "//Charlie"),
            ),
            pallet_session::Error::<Runtime>::InvalidProof
        );
        assert_ok!(Session::set_keys(
            RuntimeOrigin::signed(charlie.clone()),
            keys.clone(),
            seed_key_proof(&charlie, "//Charlie"),
        ));
        assert_eq!(
            Balances::balance_on_hold(&reason, &charlie),
            SessionKeyDeposit::get()
        );
        assert_ok!(Session::set_keys(
            RuntimeOrigin::signed(charlie.clone()),
            keys,
            seed_key_proof(&charlie, "//Charlie"),
        ));
        assert_eq!(
            Balances::balance_on_hold(&reason, &charlie),
            SessionKeyDeposit::get()
        );
        assert_ok!(Session::purge_keys(RuntimeOrigin::signed(charlie.clone())));
        assert_eq!(Balances::balance_on_hold(&reason, &charlie), 0);
        assert!(!pallet_session::NextKeys::<Runtime>::contains_key(charlie));
    });
}

#[test]
fn bonded_stake_is_held_and_cannot_be_transferred() {
    use frame_support::{assert_noop, traits::fungible::InspectHold};
    ext().execute_with(|| {
        block(1);
        let charlie = authority("Charlie").0;
        let before = Balances::free_balance(&charlie);
        assert_ok!(Staking::bond(
            RuntimeOrigin::signed(charlie.clone()),
            VALIDATOR_BOND,
            pallet_staking::RewardDestination::Staked,
        ));
        assert_eq!(
            Balances::balance_on_hold(&pallet_staking::HoldReason::Staking.into(), &charlie),
            VALIDATOR_BOND,
        );
        assert_eq!(Balances::free_balance(&charlie), before - VALIDATOR_BOND);
        assert_noop!(
            Balances::transfer_allow_death(
                RuntimeOrigin::signed(charlie),
                authority("Dave").0,
                before,
            ),
            sp_runtime::TokenError::FundsUnavailable,
        );
    });
}

#[test]
fn staking_offences_disable_the_more_severe_offender_within_the_session_limit() {
    use sp_staking::offence::{OffenceDetails, OnOffenceHandler};
    from_preset(genesis_config_presets::four_validator_test_genesis()).execute_with(|| {
        block(1);
        let validators = Session::validators();
        assert_eq!(validators.len(), 4);
        let report = |index: usize, percent| {
            <Staking as OnOffenceHandler<AccountId, (AccountId, ()), Weight>>::on_offence(
                &[OffenceDetails {
                    offender: (validators[index].clone(), ()),
                    reporters: vec![],
                }],
                &[Perbill::from_percent(percent)],
                Session::current_index(),
            );
        };
        report(0, 10);
        assert_eq!(Session::disabled_validators(), vec![0]);
        report(1, 20);
        assert_eq!(Session::disabled_validators(), vec![1]);
        report(2, 5);
        assert_eq!(Session::disabled_validators(), vec![1]);
        // BABE consults this same disabled set when validating block authors.
        assert!(<Session as frame_support::traits::DisabledValidators>::is_disabled(1));
        assert!(!<Session as frame_support::traits::DisabledValidators>::is_disabled(0));
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
            RuntimeOrigin::signed(nominator.clone()),
            stake,
            pallet_staking::RewardDestination::Stash
        ));
        assert_ok!(Staking::bond_extra(
            RuntimeOrigin::signed(nominator.clone()),
            10 * TENSOR
        ));
        assert_ok!(Staking::nominate(
            RuntimeOrigin::signed(nominator.clone()),
            vec![alice.clone()]
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
        let balance = Balances::free_balance(&nominator);
        assert!(pallet_staking::ErasValidatorReward::<Runtime>::get(1).unwrap() > 0);
        assert_ok!(Staking::payout_stakers(
            RuntimeOrigin::signed(nominator.clone()),
            alice.clone(),
            1
        ));
        assert!(Balances::free_balance(&nominator) > balance);
        assert!(Balances::total_issuance() > issuance);
        assert_ok!(Staking::chill(RuntimeOrigin::signed(nominator.clone())));
        assert_ok!(Staking::unbond(
            RuntimeOrigin::signed(nominator.clone()),
            110 * TENSOR
        ));
        advance_to_era(2 + BondingDuration::get());
        assert_ok!(Staking::withdraw_unbonded(
            RuntimeOrigin::signed(nominator.clone()),
            0
        ));
        assert!(!pallet_staking::Bonded::<Runtime>::contains_key(&nominator));
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
            RuntimeOrigin::signed(charlie.clone()),
            VALIDATOR_BOND,
            pallet_staking::RewardDestination::Staked
        ));
        assert_ok!(Session::set_keys(
            RuntimeOrigin::signed(charlie.clone()),
            opaque::SessionKeys {
                babe: babe.clone(),
                grandpa: grandpa.clone()
            },
            seed_key_proof(&charlie, "//Charlie")
        ));
        assert_ok!(Staking::validate(
            RuntimeOrigin::signed(charlie.clone()),
            Default::default()
        ));
        assert_ok!(Staking::chill(RuntimeOrigin::signed(alice.clone())));
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
            RuntimeOrigin::signed(charlie.clone()),
            VALIDATOR_BOND,
            pallet_staking::RewardDestination::Staked
        ));
        assert_ok!(Session::set_keys(
            RuntimeOrigin::signed(charlie.clone()),
            opaque::SessionKeys { babe, grandpa },
            seed_key_proof(&charlie, "//Charlie")
        ));
        assert_ok!(Staking::validate(
            RuntimeOrigin::signed(charlie.clone()),
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
        assert_eq!(author, Some(charlie.clone()));
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
            RuntimeOrigin::signed(alice.clone()),
            opaque::SessionKeys {
                babe: new_babe.clone(),
                grandpa: new_grandpa.clone(),
            },
            seed_key_proof(&alice, "//Alice//rotation")
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
        let nominator = authority("Charlie").0;
        assert_ok!(Staking::bond(
            RuntimeOrigin::signed(nominator.clone()),
            100 * TENSOR,
            pallet_staking::RewardDestination::Stash,
        ));
        assert_ok!(Staking::nominate(
            RuntimeOrigin::signed(nominator.clone()),
            vec![alice.clone()],
        ));
        advance_to_era(1);
        let offence_era = Staking::active_era().unwrap().index;
        let offence_block = System::block_number();
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
                    slot: u64::from(offence_block).into(),
                },
            );
            let mut header = Header::new(
                offence_block,
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
            slot: u64::from(offence_block).into(),
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
            pallet_staking::Ledger::<Runtime>::get(&alice)
                .unwrap()
                .active,
            VALIDATOR_BOND
        );
        advance_to_era(offence_era + SlashDeferDuration::get() + 1);
        assert!(
            pallet_staking::Ledger::<Runtime>::get(&alice)
                .unwrap()
                .active
                < VALIDATOR_BOND
        );
        // Historical proofs carry only account identity. Staking must still load
        // the offence-era exposure and slash its nominators.
        assert!(
            pallet_staking::Ledger::<Runtime>::get(&nominator)
                .unwrap()
                .active
                < 100 * TENSOR
        );
    });
}

#[test]
fn grandpa_equivocation_queues_a_slash_for_the_native_validator() {
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
            pallet_staking::Ledger::<Runtime>::get(&alice)
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
            let account: AccountId = H256::from_low_u64_be(index).to_fixed_bytes().into();
            drop(Balances::deposit_creating(&account, 2 * VALIDATOR_BOND));
            assert_ok!(Staking::bond(
                RuntimeOrigin::signed(account.clone()),
                value,
                pallet_staking::RewardDestination::Stash
            ));
            account
        };
        for index in 2..MAX_VALIDATORS {
            let account = fund_and_bond(10_000 + index as u64, VALIDATOR_BOND);
            assert_ok!(Session::set_keys(
                RuntimeOrigin::signed(account.clone()),
                opaque::SessionKeys {
                    babe: sp_consensus_babe::AuthorityPair::from_seed(&[index as u8; 32]).public(),
                    grandpa: sp_consensus_grandpa::AuthorityPair::from_seed(&[index as u8; 32])
                        .public(),
                },
                key_proof(
                    &account,
                    sp_core::sr25519::Pair::from_seed(&[index as u8; 32]),
                    sp_core::ed25519::Pair::from_seed(&[index as u8; 32]),
                )
            ));
            assert_ok!(Staking::validate(
                RuntimeOrigin::signed(account.clone()),
                Default::default()
            ));
            candidates.push(account);
        }
        let extra = fund_and_bond(99_999, VALIDATOR_BOND);
        assert_noop!(
            Staking::validate(RuntimeOrigin::signed(extra.clone()), Default::default()),
            pallet_staking::Error::<Runtime>::TooManyValidators
        );
        let targets = candidates[..MAX_NOMINATIONS as usize].to_vec();
        for index in 0..MAX_NOMINATORS {
            let account = fund_and_bond(20_000 + index as u64, 100 * TENSOR);
            assert_ok!(Staking::nominate(
                RuntimeOrigin::signed(account.clone()),
                targets.clone()
            ));
        }
        assert_noop!(
            Staking::nominate(RuntimeOrigin::signed(extra.clone()), targets),
            pallet_staking::Error::<Runtime>::TooManyNominators
        );
        assert_ok!(Staking::set_validator_count(
            RuntimeOrigin::root(),
            MAX_VALIDATORS
        ));
        let supports = <Runtime as pallet_staking::Config>::ElectionProvider::elect(0).unwrap();
        assert_eq!(supports.len(), MAX_VALIDATORS as usize);
        let voters: BTreeSet<_> = supports
            .iter()
            .flat_map(|(_, support)| support.voters.iter().map(|(account, _)| account.clone()))
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
        let (owner, ()) =
            Historical::check_proof((sp_consensus_babe::KEY_TYPE, babe), proof).unwrap();
        assert_eq!(owner, authority("Alice").0);
        assert_eq!(
            Staking::eras_stakers(1, &owner).others.len(),
            MAX_NOMINATORS as usize
        );
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
            let account: AccountId = H256::from_low_u64_be(30_000 + index as u64)
                .to_fixed_bytes()
                .into();
            drop(Balances::deposit_creating(&account, 100 * TENSOR));
            assert_ok!(Staking::bond(
                RuntimeOrigin::signed(account.clone()),
                MIN_NOMINATOR_BOND,
                pallet_staking::RewardDestination::Stash
            ));
            assert_ok!(Staking::nominate(
                RuntimeOrigin::signed(account.clone()),
                vec![authority("Alice").0]
            ));
        }
        assert!(<Runtime as pallet_staking::Config>::ElectionProvider::elect(0).is_err());
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
            let account: AccountId = H256::from_low_u64_be(30_000 + index as u64)
                .to_fixed_bytes()
                .into();
            assert_ok!(Staking::chill(RuntimeOrigin::signed(account.clone())));
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
