use crate::*;
use frame_support::{
    assert_noop, assert_ok,
    dispatch::DispatchClass,
    traits::{Hooks, PalletInfoAccess},
    weights::WeightToFee,
};
use sp_runtime::traits::{Convert, Dispatchable};

fn account(seed: &str) -> AccountId {
    genesis::presets::get_account_id_from_seed::<sp_core::sr25519::Public>(seed)
}

#[test]
fn overwatch_genesis_has_an_explicit_empty_opening_snapshot() {
    crate::tests::npos::ext().execute_with(|| {
        let snapshot = pallet_network::OverwatchEpochSnapshots::<Runtime>::get(0).unwrap();
        assert!(snapshot.nodes.is_empty());
        assert_eq!(
            snapshot.stake_weight_factor,
            pallet_network::OverwatchStakeWeightFactor::<Runtime>::get()
        );
        assert_eq!(
            snapshot.reward_budget,
            OverwatchEpochEmissions::get().saturating_mul(
                pallet_network::ActiveOverwatchEpochLengthMultiplier::<Runtime>::get() as u128,
            )
        );
    });
}

#[test]
fn overwatch_opening_and_settlement_weights_fit_the_hook_budget() {
    use pallet_network::weights::WeightInfo as _;
    type W = <Runtime as pallet_network::Config>::WeightInfo;
    let subnets = NetworkMaxPhysicalSubnetsUpperBound::get();
    let nodes = NetworkMaxOverwatchNodesUpperBound::get();
    let settlement = W::calculate_overwatch_rewards_empty()
        .max(W::calculate_overwatch_rewards_small(subnets))
        .max(W::calculate_overwatch_rewards_medium(nodes))
        .max(W::calculate_overwatch_rewards(nodes * subnets));
    assert!(W::on_initialize_base()
        .saturating_add(W::advance_overwatch_epoch_noop())
        .saturating_add(settlement)
        .all_lte(MaximumHooksWeight::get()));
    assert!(W::on_initialize_base()
        .saturating_add(W::advance_overwatch_epoch())
        .saturating_add(W::total_subnets_selector())
        .saturating_add(W::do_epoch_preliminaries(subnets))
        .all_lte(MaximumHooksWeight::get()));
}

#[test]
fn time_weighted_subnet_allocation_fits_the_hook_budget() {
    use pallet_network::weights::WeightInfo as _;
    type W = <Runtime as pallet_network::Config>::WeightInfo;
    let subnets = NetworkMaxPhysicalSubnetsUpperBound::get();
    // Slot 2 always selects the Overwatch no-op path. Its mandatory work ends after
    // allocation; any following swap queue processing is admitted by the remaining meter.
    let allocation = W::handle_subnet_emission_weights(subnets)
        .max(W::handle_subnet_emission_weights_empty());
    let envelope = W::on_initialize_base()
        .saturating_add(W::advance_overwatch_epoch_noop())
        .saturating_add(W::total_subnets_selector())
        .saturating_add(allocation);
    let limit = MaximumHooksWeight::get();
    eprintln!(
        "slot2 allocation: subnets={subnets}, ref_time={}, proof_size={}, limit_ref_time={}, limit_proof_size={}",
        envelope.ref_time(),
        envelope.proof_size(),
        limit.ref_time(),
        limit.proof_size(),
    );
    assert!(
        envelope.all_lte(limit),
        "maximum subnet allocation must fit both hook weight dimensions"
    );
}

#[test]
fn network_reads_babe_randomness_after_consensus_rotation() {
    use frame_support::traits::Randomness as RandomnessT;
    type NetworkRandomness = <Runtime as pallet_network::Config>::Randomness;

    crate::tests::npos::ext().execute_with(|| {
        assert!(Network::index() > Babe::index());
        assert!(Network::index() > Session::index());
        assert!(Network::index() < Scheduler::index());

        let subject = b"network/provider-regression";
        pallet_babe::Randomness::<Runtime>::put([7; 32]);
        pallet_babe::EpochStart::<Runtime>::put((20, 40));
        let first = <NetworkRandomness as RandomnessT<Hash, BlockNumber>>::random(subject);
        assert_eq!(first, Randomness::random(subject));
        assert_eq!(first.1, 20);

        // Query the provider through Network's actual runtime configuration.
        pallet_babe::Randomness::<Runtime>::put([8; 32]);
        let next = <NetworkRandomness as RandomnessT<Hash, BlockNumber>>::random(subject);
        assert_ne!(next.0, first.0);
        assert_eq!(next.1, first.1);
    });
}

#[test]
fn non_transfer_proxy_cannot_escalate_or_redirect_staking_rewards() {
    crate::tests::npos::ext().execute_with(|| {
        System::set_block_number(1);
        let owner = account("Alice");
        let delegate = account("Bob");
        assert_ok!(Proxy::add_proxy(
            RuntimeOrigin::signed(owner.clone()),
            delegate.clone(),
            ProxyType::NonTransfer,
            0,
        ));
        let transfer: RuntimeCall = pallet_balances::Call::transfer_allow_death {
            dest: delegate.clone(),
            value: TENSOR,
        }
        .into();
        let calls: Vec<RuntimeCall> = vec![
            pallet_sudo::Call::sudo_as {
                who: owner.clone(),
                call: Box::new(transfer.clone()),
            }
            .into(),
            pallet_sudo::Call::sudo {
                call: Box::new(
                    pallet_balances::Call::force_transfer {
                        source: owner.clone(),
                        dest: delegate.clone(),
                        value: TENSOR,
                    }
                    .into(),
                ),
            }
            .into(),
            pallet_atomic_swap::Call::create_swap {
                target: delegate.clone(),
                hashed_proof: [1; 32],
                hash_type: pallet_atomic_swap::HashType::Blake2256,
                action: pallet_atomic_swap::BalanceSwapAction::new(TENSOR),
                duration: 10,
            }
            .into(),
            pallet_multisig::Call::as_multi_threshold_1 {
                other_signatories: vec![delegate.clone()],
                call: Box::new(transfer),
            }
            .into(),
            pallet_staking::Call::set_payee {
                payee: pallet_staking::RewardDestination::Account(delegate.clone()),
            }
            .into(),
            pallet_staking::Call::bond {
                value: TENSOR,
                payee: pallet_staking::RewardDestination::Account(delegate.clone()),
            }
            .into(),
        ];
        let before = (
            Balances::free_balance(&owner),
            Balances::free_balance(&delegate),
        );
        for call in calls {
            assert_ok!(Proxy::proxy(
                RuntimeOrigin::signed(delegate.clone()),
                owner.clone(),
                None,
                Box::new(call)
            ));
            System::assert_last_event(
                pallet_proxy::Event::ProxyExecuted {
                    result: Err(frame_system::Error::<Runtime>::CallFiltered.into()),
                }
                .into(),
            );
            assert_eq!(
                (
                    Balances::free_balance(&owner),
                    Balances::free_balance(&delegate)
                ),
                before
            );
        }
        // The restricted proxy remains useful for ordinary NPoS management.
        assert_ok!(Proxy::proxy(
            RuntimeOrigin::signed(delegate),
            owner,
            None,
            Box::new(pallet_staking::Call::chill {}.into())
        ));
        System::assert_last_event(pallet_proxy::Event::ProxyExecuted { result: Ok(()) }.into());
    });
}

#[test]
fn pausing_preserves_the_timestamp_and_root_recovery_path() {
    crate::tests::npos::ext().execute_with(|| {
        System::set_block_number(1);
        let name = |pallet: &[u8], call: &[u8]| -> RuntimeCallNameOf<Runtime> {
            (
                pallet.to_vec().try_into().unwrap(),
                call.to_vec().try_into().unwrap(),
            )
        };
        for protected in [
            name(b"Timestamp", b"set"),
            name(b"Sudo", b"sudo"),
            name(b"Sudo", b"sudo_as"),
        ] {
            assert_noop!(
                TxPause::pause(RuntimeOrigin::root(), protected),
                pallet_tx_pause::Error::<Runtime>::Unpausable
            );
        }
        let paused = name(b"Balances", b"transfer_allow_death");
        assert_ok!(TxPause::pause(RuntimeOrigin::root(), paused.clone()));
        let transfer: RuntimeCall = pallet_balances::Call::transfer_allow_death {
            dest: account("Bob"),
            value: TENSOR,
        }
        .into();
        assert!(transfer
            .clone()
            .dispatch(RuntimeOrigin::signed(account("Alice")))
            .is_err());
        let recover: RuntimeCall = pallet_sudo::Call::sudo {
            call: Box::new(pallet_tx_pause::Call::unpause { ident: paused }.into()),
        }
        .into();
        assert_ok!(recover.dispatch(RuntimeOrigin::signed(account("Alice"))));
        assert_ok!(transfer.dispatch(RuntimeOrigin::signed(account("Alice"))));
    });
}

#[test]
fn congestion_fees_recover_from_the_floor_in_both_weight_dimensions() {
    type Update = <Runtime as pallet_transaction_payment::Config>::FeeMultiplierUpdate;
    type Fees = <Runtime as pallet_transaction_payment::Config>::WeightToFee;
    crate::tests::npos::ext().execute_with(|| {
        let floor = MinimumMultiplier::get();
        let capacity = BlockWeights::get()
            .get(DispatchClass::Normal)
            .max_total
            .unwrap();
        for weight in [
            Weight::from_parts(capacity.ref_time(), 0),
            Weight::from_parts(0, capacity.proof_size()),
        ] {
            System::set_block_consumed_resources(weight, 0);
            // set_block_consumed_resources records Normal weight in the SDK.
            let increased = Update::convert(floor);
            assert!(increased > floor);
            assert!(Fees::weight_to_fee(&weight) > 0);
            System::set_block_consumed_resources(Weight::zero(), 0);
            assert!(Update::convert(increased) < increased);
            assert_eq!(Update::convert(floor), floor);
        }
        assert_eq!(
            Fees::weight_to_fee(&MAXIMUM_BLOCK_WEIGHT),
            tokenomics::STARTING_SUPPLY / 25_000_000_000
        );
        assert_eq!(
            deposit(1, 1024),
            tokenomics::POLICY.storage_item_deposit
                + 1024 * tokenomics::POLICY.storage_byte_deposit
        );
        assert_eq!(DepositBase::get(), deposit(1, 88));
    });
}

#[test]
fn scheduler_respects_prior_hooks_and_leaves_room_for_inherents() {
    crate::tests::npos::ext().execute_with(|| {
        System::set_block_number(1);
        assert!(Scheduler::index() > Revive::index());
        assert!(Scheduler::index() > Session::index());
        assert!(Scheduler::index() > Network::index());
        // Model a busy settlement / election block before Scheduler runs.
        let used = Perbill::from_percent(60) * MAXIMUM_BLOCK_WEIGHT;
        System::register_extra_weight_unchecked(used, DispatchClass::Mandatory);
        let budget = MaximumSchedulerWeight::get();
        let headroom = Perbill::from_percent(10) * MAXIMUM_BLOCK_WEIGHT;
        assert!(budget
            .saturating_add(used)
            .saturating_add(headroom)
            .all_lte(MAXIMUM_BLOCK_WEIGHT));
        let heavy: RuntimeCall = pallet_sudo::Call::sudo_unchecked_weight {
            call: Box::new(frame_system::Call::remark { remark: vec![] }.into()),
            weight: Perbill::from_percent(50) * MAXIMUM_BLOCK_WEIGHT,
        }
        .into();
        assert_ok!(Scheduler::schedule(
            RuntimeOrigin::root(),
            2,
            None,
            0,
            Box::new(heavy)
        ));
        System::set_block_number(2);
        let consumed = Scheduler::on_initialize(2);
        assert!(consumed.all_lte(budget));
        assert!(!System::events().iter().any(|event| matches!(
            event.event,
            RuntimeEvent::Scheduler(pallet_scheduler::Event::Dispatched { .. })
        )));
    });
}

#[test]
fn ethereum_extrinsics_fit_alongside_the_network_hook_budget() {
    crate::tests::npos::ext().execute_with(|| {
        let combined = Revive::evm_max_extrinsic_weight().saturating_add(MaximumHooksWeight::get());
        assert!(combined.all_lte(Perbill::from_percent(90) * MAXIMUM_BLOCK_WEIGHT));
    });
}

#[test]
fn production_staking_timings_and_genesis_are_not_development_defaults() {
    let era_millis = PRODUCTION_EPOCH_SLOTS * SLOT_DURATION * PRODUCTION_SESSIONS_PER_ERA as u64;
    assert_eq!(era_millis, 24 * 60 * 60 * 1000);
    assert_eq!(
        era_millis * BondingDuration::get() as u64,
        28 * 24 * 60 * 60 * 1000
    );
    assert!(SlashDeferDuration::get() < BondingDuration::get());
    assert!(genesis::presets::get_preset(&"HOSKINSON_RUNTIME_PRESET".into()).is_none());
}

#[test]
fn treasury_retains_funds_and_allows_payouts_beyond_the_development_window() {
    crate::tests::npos::ext().execute_with(|| {
        System::set_block_number(1);
        let treasury = Treasury::account_id();
        let beneficiary = account("Bob");
        assert_ok!(Balances::transfer_allow_death(
            RuntimeOrigin::signed(account("Alice")),
            treasury.clone(),
            100 * TENSOR,
        ));
        let funded = Balances::free_balance(&treasury);
        for block in [2, 10, TreasurySpendPeriod::get()] {
            System::set_block_number(block);
            Treasury::on_initialize(block);
            assert_eq!(Balances::free_balance(&treasury), funded);
        }
        assert_ok!(Treasury::spend(
            RuntimeOrigin::root(),
            Box::new(()),
            10 * TENSOR,
            Box::new(beneficiary.clone()),
            None,
        ));
        let before = Balances::free_balance(&beneficiary);
        System::set_block_number(System::block_number() + DAYS);
        assert_ok!(Treasury::payout(
            RuntimeOrigin::signed(beneficiary.clone()),
            0
        ));
        assert_eq!(Balances::free_balance(&beneficiary), before + 10 * TENSOR);
        assert_eq!(Balances::free_balance(&treasury), funded - 10 * TENSOR);
    });
}
