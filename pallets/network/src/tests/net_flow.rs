use super::mock::*;
use super::test_utils::{account, insert_elected_subnet_node};
use crate::*;
use frame_support::{assert_noop, assert_ok, weights::WeightMeter};
use sp_core::U256;

const Q: u128 = PERCENTAGE_FACTOR_U128;
const UNIT: u128 = NET_FLOW_ATOMIC_UNITS_PER_UNIT;

fn amount(value: i128) -> SignedNetFlow {
    SignedNetFlow::from_amount(value < 0, value.unsigned_abs())
}

fn units(value: i128) -> SignedNetFlow {
    SignedNetFlow::from_amount(value < 0, value.unsigned_abs() * UNIT)
}

fn seed(id: u32) {
    if System::block_number() == 0 {
        System::set_block_number(1);
    }
    SubnetsData::<Test>::insert(
        id,
        SubnetData {
            id,
            state: SubnetState::Active,
            ..Default::default()
        },
    );
    SubnetSlot::<Test>::insert(id, 3 + id);
    SubnetNetFlows::<Test>::insert(id, SubnetNetFlow::default());
    TxRateLimit::<Test>::put(0);
}

fn deposit(id: u32, value: u128) {
    assert_ok!(Network::handle_increase_account_delegate_stake(
        &account(1),
        id,
        value
    ));
}

fn eligible(id: u32, epoch: u32) {
    let block = epoch * EpochLength::get() + NETWORK_SUBNET_EMISSION_SLOT;
    insert_elected_subnet_node(
        id,
        Network::get_subnet_epoch_with_block_as_u32(id, block),
        1,
    );
}

fn collective() -> RuntimeOrigin {
    RuntimeOrigin::from(pallet_collective::RawOrigin::Members(2, 3))
}

#[test]
fn signed_flow_supports_full_width_amounts_and_symmetric_cancellation() {
    let full = SignedNetFlow::from_amount(false, u128::MAX);
    let negative = SignedNetFlow::from_amount(true, u128::MAX);
    assert_eq!(full.checked_add(negative), Some(amount(0)));
    assert_eq!(negative.checked_add(full), Some(amount(0)));
    assert!(negative < amount(-1) && amount(-1) < amount(0) && amount(0) < full);
    assert_eq!(full.blend(negative, Q / 2), amount(0));
    assert_eq!(full.blend(full, Q), full);
    // Even the largest representable checkpoint is safe in weighted intermediate arithmetic.
    let max = SignedNetFlow::new(false, U256::MAX);
    assert_eq!(max.blend(max, Q / 2), max);
    assert_eq!(
        max.blend(SignedNetFlow::new(true, U256::MAX), Q / 2),
        amount(0)
    );
    assert_eq!(SignedNetFlow::new(true, U256::zero()), amount(0));
}

#[test]
fn ema_updates_once_per_completed_epoch_and_retains_negative_values() {
    new_test_ext().execute_with(|| {
        let initial = SubnetNetFlow {
            epoch: 0,
            flow: amount(1000),
            ema: amount(0),
        };
        assert_eq!(
            Network::checkpoint_subnet_net_flow(initial, 0, Q / 2),
            initial
        );
        let mut record = Network::checkpoint_subnet_net_flow(initial, 1, Q / 2);
        assert_eq!(record.ema, amount(500));
        assert_eq!(record.flow, amount(0));
        assert_eq!(
            Network::checkpoint_subnet_net_flow(record, 1, Q / 2),
            record
        );
        record.flow = amount(-2000);
        record = Network::checkpoint_subnet_net_flow(record, 2, Q / 2);
        assert_eq!(record.ema, amount(-750));
        assert_eq!(
            Network::checkpoint_subnet_net_flow(record, 3, Q / 2).ema,
            amount(-375)
        );
        assert_eq!(
            Network::checkpoint_subnet_net_flow(record, 4, Q / 2).ema,
            amount(-187)
                .checked_add(SignedNetFlow::new(true, U256::from(Q / 2)))
                .unwrap()
        );
    });
}

#[test]
fn ema_default_half_life_and_skipped_epochs_match_epoch_reference() {
    new_test_ext().execute_with(|| {
        let alpha = SubnetNetFlowSmoothingAlpha::<Test>::get();
        assert_eq!(SubnetNetFlowK::<Test>::get(), 500);
        let initial = SubnetNetFlow {
            ema: amount(1000),
            ..Default::default()
        };
        let ten = Network::checkpoint_subnet_net_flow(initial, 10, alpha);
        let difference = ten.ema.magnitude.abs_diff(amount(500).magnitude);
        assert!(difference < U256::from(Q / 1_000_000));
        for alpha in [1, Q / 10, Q / 2, Q - 1, Q] {
            for negative in [false, true] {
                let initial = SubnetNetFlow {
                    epoch: 0,
                    flow: SignedNetFlow::from_amount(negative, 5000),
                    ema: SignedNetFlow::from_amount(!negative, 900),
                };
                let fast = Network::checkpoint_subnet_net_flow(initial, 123, alpha);
                let mut reference = initial;
                for epoch in 1..=123 {
                    reference = Network::checkpoint_subnet_net_flow(reference, epoch, alpha);
                }
                assert_eq!(fast.ema.negative, reference.ema.negative);
                let difference = fast.ema.magnitude.abs_diff(reference.ema.magnitude);
                assert!(difference < U256::from(1_000_000u128));
                let far = Network::checkpoint_subnet_net_flow(initial, u32::MAX, alpha);
                assert!(far.ema.magnitude <= amount(5000).magnitude);
            }
        }
        let bad = SubnetNetFlow {
            epoch: 10,
            ..initial
        };
        assert_eq!(
            Network::checkpoint_subnet_net_flow(bad, 9, alpha),
            SubnetNetFlow {
                epoch: 9,
                ..Default::default()
            }
        );
    });
}

#[test]
fn softmax_orders_outflows_and_handles_neutral_mixed_and_extreme_scores() {
    new_test_ext().execute_with(|| {
        let weights = Network::normalize_subnet_net_flows(
            BTreeMap::from([(1, units(-100)), (2, units(-200)), (3, units(-300))]),
            500,
        );
        for (id, expected) in [
            (1, 0.4017595785333554),
            (2, 0.3289329222889067),
            (3, 0.2693074991777379),
        ] {
            assert!(weights[&id].abs_diff((expected * Q as f64) as u128) < 1024);
        }
        assert!(weights[&1] > weights[&2] && weights[&2] > weights[&3] && weights[&3] > 0);
        assert!(weights.values().sum::<u128>() <= Q);
        for value in [-1000, 0, 1000] {
            assert_eq!(
                Network::normalize_subnet_net_flows(
                    BTreeMap::from([(1, units(value)), (2, units(value))]),
                    500
                ),
                BTreeMap::from([(1, Q / 2), (2, Q / 2)])
            );
        }
        let mixed = BTreeMap::from([(1, units(100)), (2, units(0)), (3, units(-100))]);
        let strong = Network::normalize_subnet_net_flows(mixed.clone(), 100);
        let weak = Network::normalize_subnet_net_flows(mixed, 1000);
        assert!(strong[&1] > weak[&1] && strong[&3] < weak[&3]);
        assert!(Network::normalize_subnet_net_flows(BTreeMap::new(), 500).is_empty());
        let extremes = BTreeMap::from([
            (1, SignedNetFlow::new(false, U256::MAX)),
            (2, SignedNetFlow::new(true, U256::MAX)),
        ]);
        assert_eq!(
            Network::normalize_subnet_net_flows(extremes, u128::MAX),
            BTreeMap::from([(1, Q), (2, 0)])
        );
        // Subtract before float conversion: the signal survives a large common offset.
        let high = SignedNetFlow::from_amount(false, u128::MAX);
        let low = high.checked_add(units(-500)).unwrap();
        let large = Network::normalize_subnet_net_flows(BTreeMap::from([(1, high), (2, low)]), 500);
        let small = Network::normalize_subnet_net_flows(
            BTreeMap::from([(1, units(500)), (2, units(0))]),
            500,
        );
        assert_eq!(large, small);
    });
}

#[test]
fn deposits_net_withdrawals_without_transaction_frequency_or_balance_weighting() {
    new_test_ext().execute_with(|| {
        seed(1);
        seed(2);
        deposit(1, 10_000);
        deposit(2, 10_000);
        System::set_block_number(EpochLength::get());
        SubnetNetFlowSmoothingAlpha::<Test>::put(Q);
        deposit(1, 2000);
        deposit(2, 1000);
        System::set_block_number(EpochLength::get() + 10);
        deposit(2, 1000);
        let before_reward = SubnetNetFlows::<Test>::get(1);
        assert_ok!(Network::do_increase_delegate_stake(1, 1_000_000));
        assert_eq!(SubnetNetFlows::<Test>::get(1), before_reward);
        assert_ne!(
            TotalSubnetDelegateStakeBalance::<Test>::get(1),
            TotalSubnetDelegateStakeBalance::<Test>::get(2)
        );
        for id in [1, 2] {
            eligible(id, 2);
        }
        System::set_block_number(2 * EpochLength::get() + NETWORK_SUBNET_EMISSION_SLOT);
        assert_eq!(
            Network::get_net_flow_weights(vec![1, 2], 2).0,
            BTreeMap::from([(1, Q / 2), (2, Q / 2)])
        );
        for id in [1, 2] {
            assert_eq!(SubnetNetFlows::<Test>::get(id).unwrap().ema, amount(2000));
        }
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1) / 2;
        let before = TotalSubnetDelegateStakeBalance::<Test>::get(1);
        assert_ok!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(account(1)),
            1,
            shares,
            1
        ));
        let removed = before - TotalSubnetDelegateStakeBalance::<Test>::get(1);
        assert_eq!(
            SubnetNetFlows::<Test>::get(1).unwrap().flow,
            SignedNetFlow::from_amount(true, removed)
        );
        // In-epoch activity cannot change the completed epoch's flow EMA.
        assert_eq!(
            Network::get_net_flow_weights(vec![1, 2], 2).0,
            BTreeMap::from([(1, Q / 2), (2, Q / 2)])
        );
    });
}

#[test]
fn rewards_and_share_transfers_never_count_as_flow_or_recreate_deleted_accounting() {
    new_test_ext().execute_with(|| {
        seed(1);
        deposit(1, 10_000);
        let before = SubnetNetFlows::<Test>::get(1);
        System::set_block_number(3 * EpochLength::get());
        assert_ok!(Network::do_increase_delegate_stake(1, 1000));
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        assert_ok!(Network::transfer_delegate_stake(
            RuntimeOrigin::signed(account(1)),
            1,
            account(2),
            shares
        ));
        assert_eq!(SubnetNetFlows::<Test>::get(1), before);
        SubnetNetFlows::<Test>::remove(1);
        assert_ok!(Network::do_increase_delegate_stake(1, 1000));
        assert!(!SubnetNetFlows::<Test>::contains_key(1));
        SubnetsData::<Test>::remove(1);
        assert_ok!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(account(2)),
            1,
            shares,
            1
        ));
        assert!(!SubnetNetFlows::<Test>::contains_key(1));
    });
}

#[test]
fn failed_financial_calls_do_not_record_flow_and_missing_history_is_not_invented() {
    new_test_ext().execute_with(|| {
        seed(1);
        deposit(1, 10_000);
        let before = SubnetNetFlows::<Test>::get(1);
        assert!(Network::handle_increase_account_delegate_stake_with_limit(
            &account(1),
            1,
            1000,
            u128::MAX
        )
        .is_err());
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        assert!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(account(1)),
            1,
            shares,
            u128::MAX
        )
        .is_err());
        assert_eq!(SubnetNetFlows::<Test>::get(1), before);
        SubnetNetFlows::<Test>::remove(1);
        System::set_block_number(10 * EpochLength::get());
        deposit(1, 1000);
        assert_eq!(
            SubnetNetFlows::<Test>::get(1),
            Some(SubnetNetFlow {
                epoch: 10,
                flow: amount(1000),
                ema: amount(0)
            })
        );
    });
}

#[test]
fn queued_swaps_count_source_exit_and_successful_destination_credit_only() {
    new_test_ext().execute_with(|| {
        DelegateStakeCooldownEpochs::<Test>::put(1);
        for id in 1..=3 {
            seed(id);
        }
        deposit(1, 10_000);
        System::set_block_number(EpochLength::get());
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        let first_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::swap_from_subnet_to_subnet(
            RuntimeOrigin::signed(account(1)),
            1,
            2,
            shares / 2,
            1,
            1,
            u32::MAX
        ));
        let second_id = NextSwapQueueId::<Test>::get();
        let remaining = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        assert_ok!(Network::swap_from_subnet_to_subnet(
            RuntimeOrigin::signed(account(1)),
            1,
            3,
            remaining,
            1,
            u128::MAX,
            u32::MAX
        ));
        let source_exit = 10_000 - TotalSubnetDelegateStakeBalance::<Test>::get(1);
        assert_eq!(
            SubnetNetFlows::<Test>::get(1).unwrap().flow,
            SignedNetFlow::from_amount(true, source_exit)
        );
        for id in [2, 3] {
            assert_eq!(SubnetNetFlows::<Test>::get(id).unwrap().flow, amount(0));
        }
        let first = SwapCallQueue::<Test>::get(first_id).unwrap();
        let second = SwapCallQueue::<Test>::get(second_id).unwrap();
        let ready = first.queued_at_running_block + first.execute_after_blocks;
        System::set_block_number(ready);
        Network::execute_ready_swap_calls_with_limit(ready, 2, &mut WeightMeter::new());
        assert_eq!(
            SubnetNetFlows::<Test>::get(2).unwrap().flow,
            SignedNetFlow::from_amount(false, first.call.get_queue_balance())
        );
        assert_eq!(SubnetNetFlows::<Test>::get(3).unwrap().flow, amount(0));
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(account(1)),
            second.call.get_queue_balance()
        );
    });
}

#[test]
fn allocation_uses_only_eligible_completed_flows_and_changes_final_rewards() {
    new_test_ext().execute_with(|| {
        SubnetWeightFactors::<Test>::put(SubnetWeightFactorsData {
            delegate_stake: 0,
            node_count: 0,
            net_flow: Q,
        });
        SubnetDistributionPower::<Test>::put(Q);
        DefaultOverwatchSubnetWeight::<Test>::put(Q);
        LatestEffectiveOverwatchSignal::<Test>::kill();
        SubnetNetFlowSmoothingAlpha::<Test>::put(Q);
        for (id, flow) in [(1, -100), (2, -200), (3, 100_000)] {
            seed(id);
            SubnetNetFlows::<Test>::insert(
                id,
                SubnetNetFlow {
                    flow: units(flow),
                    ..Default::default()
                },
            );
        }
        eligible(1, 1);
        eligible(2, 1);
        System::set_block_number(EpochLength::get() + NETWORK_SUBNET_EMISSION_SLOT);
        let (weights, _) = Network::calculate_subnet_weights(1);
        assert_eq!(weights.len(), 2);
        assert!(weights[&1] > weights[&2]);
        assert!(weights.values().sum::<u128>() <= Q);
        let before = SubnetNetFlows::<Test>::get(1);
        assert_eq!(Network::calculate_subnet_weights(1).0, weights);
        assert_eq!(SubnetNetFlows::<Test>::get(1), before);
        // A huge deposit in the new epoch affects the next completed observation only.
        deposit(2, 1_000_000 * UNIT);
        assert_eq!(Network::calculate_subnet_weights(1).0, weights);
        eligible(1, 2);
        eligible(2, 2);
        System::set_block_number(2 * EpochLength::get() + NETWORK_SUBNET_EMISSION_SLOT);
        let next = Network::calculate_subnet_weights(2).0;
        assert!(next[&2] > next.get(&1).copied().unwrap_or(0));
    });
}

#[test]
fn collective_alpha_updates_complete_history_under_old_policy_and_preserve_open_flow() {
    new_test_ext().execute_with(|| {
        SubnetNetFlowSmoothingAlpha::<Test>::put(Q / 2);
        for id in 1..=2 {
            seed(id);
            deposit(id, 1000);
        }
        SubnetsData::<Test>::mutate(2, |s| s.as_mut().unwrap().state = SubnetState::Registered);
        System::set_block_number(EpochLength::get());
        deposit(1, 2000); // Completes epoch zero before governance does.
        for origin in [
            RuntimeOrigin::signed(account(1)),
            RuntimeOrigin::root(),
            RuntimeOrigin::from(pallet_collective::RawOrigin::Members(1, 3)),
        ] {
            assert_noop!(
                Network::set_subnet_net_flow_smoothing_alpha(origin, Q / 4),
                sp_runtime::DispatchError::BadOrigin
            );
        }
        for invalid in [0, Q + 1, u128::MAX] {
            assert_noop!(
                Network::set_subnet_net_flow_smoothing_alpha(collective(), invalid),
                Error::<Test>::InvalidPercent
            );
        }
        assert_ok!(Network::set_subnet_net_flow_smoothing_alpha(
            collective(),
            Q / 4
        ));
        System::assert_last_event(Event::<Test>::SetSubnetNetFlowSmoothingAlpha(Q / 4).into());
        assert_eq!(SubnetNetFlowSmoothingAlpha::<Test>::get(), Q / 4);
        for id in 1..=2 {
            assert_eq!(SubnetNetFlows::<Test>::get(id).unwrap().ema, amount(500));
        }
        assert_eq!(SubnetNetFlows::<Test>::get(1).unwrap().flow, amount(2000));
        assert_eq!(Network::read_subnet_net_flow(1, 2, Q / 4).ema, amount(875));
        assert_eq!(Network::read_subnet_net_flow(2, 2, Q / 4).ema, amount(375));
        assert_ok!(Network::set_subnet_net_flow_smoothing_alpha(
            collective(),
            Q
        ));
    });
}

#[test]
fn collective_k_is_positive_and_does_not_rewrite_measurements() {
    new_test_ext().execute_with(|| {
        seed(1);
        deposit(1, 1000);
        let before = SubnetNetFlows::<Test>::get(1);
        assert_noop!(
            Network::set_subnet_net_flow_k(RuntimeOrigin::signed(account(1)), 100),
            sp_runtime::DispatchError::BadOrigin
        );
        assert_noop!(
            Network::set_subnet_net_flow_k(collective(), 0),
            Error::<Test>::InvalidSubnetNetFlowK
        );
        for k in [1, 500, u128::MAX] {
            assert_ok!(Network::set_subnet_net_flow_k(collective(), k));
            assert_eq!(SubnetNetFlowK::<Test>::get(), k);
            assert_eq!(SubnetNetFlows::<Test>::get(1), before);
            System::assert_last_event(Event::<Test>::SetSubnetNetFlowK(k).into());
        }
    });
}

#[test]
fn withdrawals_above_signed_u128_half_range_remain_exact_negative_flows() {
    new_test_ext().execute_with(|| {
        seed(1);
        AccountSubnetDelegateStakeShares::<Test>::insert(account(1), 1, 2_000_000_000u128);
        TotalSubnetDelegateStakeShares::<Test>::insert(1, 3_000_000_000u128);
        TotalSubnetDelegateStakeCirculatingShares::<Test>::insert(1, 2_000_000_000u128);
        TotalSubnetDelegateStakeBalance::<Test>::insert(1, u128::MAX);
        TotalDelegateStake::<Test>::put(u128::MAX);
        let (result, removed, _) = Network::perform_do_remove_subnet_delegate_stake(
            &account(1),
            1,
            2_000_000_000,
            1,
            false,
        );
        assert_ok!(result);
        assert!(removed > i128::MAX as u128);
        assert_eq!(
            SubnetNetFlows::<Test>::get(1).unwrap().flow,
            SignedNetFlow::from_amount(true, removed)
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(1),
            u128::MAX - removed
        );
    });
}

#[test]
fn failed_queue_insertion_rolls_back_flow_alongside_source_funds() {
    new_test_ext().execute_with(|| {
        seed(1);
        seed(2);
        deposit(1, 10_000);
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        let before = SubnetNetFlows::<Test>::get(1);
        let cap = <Test as Config>::MaxSwapQueueLength::get();
        SwapQueueOrder::<Test>::mutate(|queue| {
            for id in 0..cap {
                queue.try_push(id).unwrap();
            }
        });
        SwapQueueCount::<Test>::put(cap);
        System::set_block_number(20);
        assert!(Network::swap_from_subnet_to_subnet(
            RuntimeOrigin::signed(account(1)),
            1,
            2,
            shares,
            1,
            1,
            u32::MAX
        )
        .is_err());
        assert_eq!(SubnetNetFlows::<Test>::get(1), before);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(1), 10_000);
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1),
            shares
        );
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
    });
}

#[test]
fn same_epoch_round_trip_and_pool_generation_reset_preserve_net_amounts() {
    new_test_ext().execute_with(|| {
        seed(1);
        deposit(1, 10_000);
        let before = SubnetNetFlows::<Test>::get(1).unwrap();
        System::set_block_number(EpochLength::get() / 4);
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        let (result, removed, _) =
            Network::perform_do_remove_subnet_delegate_stake(&account(1), 1, shares / 2, 1, false);
        assert_ok!(result);
        System::set_block_number(EpochLength::get() * 3 / 4);
        deposit(1, removed);
        assert_eq!(SubnetNetFlows::<Test>::get(1), Some(before));
        // A fully redeemable one-unit residual pool can reset its share generation.
        TotalSubnetDelegateStakeBalance::<Test>::insert(1, 1);
        TotalDelegateStake::<Test>::put(1);
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        let (result, removed, _) =
            Network::perform_do_remove_subnet_delegate_stake(&account(1), 1, shares, 1, false);
        assert_ok!(result);
        assert_eq!(removed, 1);
        assert_eq!(SubnetDelegatePoolGeneration::<Test>::get(1), 1);
        assert_eq!(SubnetNetFlows::<Test>::get(1).unwrap().flow, amount(9999));
        deposit(1, 1000);
        assert_eq!(SubnetNetFlows::<Test>::get(1).unwrap().flow, amount(10999));
    });
}
