use super::mock::*;
use super::test_utils::{account, insert_elected_subnet_node};
use crate::*;
use frame_support::{assert_noop, assert_ok, traits::Hooks, weights::WeightMeter};
use sp_core::U256;

const Q: u128 = PERCENTAGE_FACTOR_U128;

fn record(balance: u128, block: u32) -> SubnetDelegateStakeEma {
    SubnetDelegateStakeEma {
        balance: U256::from(balance) * U256::from(Q),
        last_updated_block: block,
    }
}

fn seed(id: u32) {
    SubnetsData::<Test>::insert(
        id,
        SubnetData {
            id,
            state: SubnetState::Active,
            ..Default::default()
        },
    );
    SubnetSlot::<Test>::insert(id, 3 + id);
    SubnetDelegateStakeEmas::<Test>::insert(id, record(0, 0));
    SubnetNetFlows::<Test>::insert(id, SubnetNetFlow::default());
    TxRateLimit::<Test>::put(0);
}

fn deposit(id: u32, amount: u128) {
    assert_ok!(Network::handle_increase_account_delegate_stake(
        &account(1),
        id,
        amount
    ));
}

fn advance(id: u32, block: u32) -> SubnetDelegateStakeEma {
    Network::read_subnet_delegate_stake_ema(
        id,
        TotalSubnetDelegateStakeBalance::<Test>::get(id),
        block,
        SubnetDelegateStakeSmoothingAlpha::<Test>::get(),
    )
}

fn isolate_delegate_factor(epoch: u32, ids: &[u32]) -> u32 {
    SubnetWeightFactors::<Test>::put(SubnetWeightFactorsData {
        delegate_stake: Q,
        node_count: 0,
        net_flow: 0,
    });
    SubnetDistributionPower::<Test>::put(Q);
    DefaultOverwatchSubnetWeight::<Test>::put(Q);
    LatestEffectiveOverwatchSignal::<Test>::kill();
    let block = epoch * EpochLength::get() + NETWORK_SUBNET_EMISSION_SLOT;
    for id in ids {
        let local_epoch = Network::get_subnet_epoch_with_block_as_u32(*id, block);
        insert_elected_subnet_node(*id, local_epoch, 1);
    }
    block
}

#[test]
fn ema_matches_independent_per_block_reference_and_bounds() {
    new_test_ext().execute_with(|| {
        let scale = U256::from(Q);
        for alpha in [1, Q / 200, Q / 2, Q - 1] {
            for initial in [0, 37, u128::MAX] {
                let mut fast = record(initial, 0);
                let mut reference = fast.balance;
                let mut block = 0;
                for (balance, gap) in [(1000, 0), (u128::MAX, 1), (7, 19), (0, 43), (999, 200)] {
                    let start = fast.balance;
                    for _ in 0..gap {
                        reference = (reference * U256::from(Q - alpha)
                            + U256::from(balance) * scale * U256::from(alpha))
                            / scale;
                    }
                    block += gap;
                    fast =
                        Network::checkpoint_subnet_delegate_stake_ema(fast, balance, block, alpha);
                    let target = U256::from(balance) * scale;
                    assert!(fast.balance >= start.min(target) && fast.balance <= start.max(target));
                    let difference = fast.balance.max(reference) - fast.balance.min(reference);
                    // Q18 exponent rounding is bounded by the elapsed steps times the largest
                    // possible unscaled balance. Test every block independently, not pow again.
                    assert!(difference <= U256::from(u128::MAX) * U256::from(block + 1));
                }
            }
        }
    });
}

#[test]
fn ema_default_closes_about_63_percent_in_ten_epochs() {
    new_test_ext().execute_with(|| {
        let alpha = SubnetDelegateStakeSmoothingAlpha::<Test>::get();
        let blocks = 10 * EpochLength::get();
        assert_eq!(alpha, Q / u128::from(blocks));
        let result =
            Network::checkpoint_subnet_delegate_stake_ema(record(0, 0), 1000, blocks, alpha);
        assert!(result.balance > U256::from(632 * Q));
        assert!(result.balance < U256::from(640 * Q));
        let tail = Network::checkpoint_subnet_delegate_stake_ema(result, 0, blocks * 2, alpha);
        assert!(tail.balance < result.balance && tail.balance > U256::zero());
    });
}

#[test]
fn ema_large_gaps_extreme_balances_and_invalid_history() {
    new_test_ext().execute_with(|| {
        for alpha in [1, Q / 200, Q - 1] {
            let grown = Network::checkpoint_subnet_delegate_stake_ema(
                record(0, 0),
                u128::MAX,
                u32::MAX,
                alpha,
            );
            assert!(grown.balance > U256::zero());
            assert!(grown.balance <= record(u128::MAX, 0).balance);
            let decayed = Network::checkpoint_subnet_delegate_stake_ema(
                record(u128::MAX, 0),
                0,
                u32::MAX,
                alpha,
            );
            assert!(decayed.balance < record(u128::MAX, 0).balance);
        }
        for invalid in [
            record(10, 11),
            SubnetDelegateStakeEma {
                balance: U256::MAX,
                last_updated_block: 0,
            },
        ] {
            assert_eq!(
                Network::checkpoint_subnet_delegate_stake_ema(invalid, u128::MAX, 10, Q / 2),
                record(0, 10)
            );
        }
        for invalid_alpha in [0, Q, u128::MAX] {
            assert_eq!(
                Network::checkpoint_subnet_delegate_stake_ema(
                    record(10, 0),
                    100,
                    10,
                    invalid_alpha
                ),
                record(0, 10)
            );
        }
    });
}

#[test]
fn ema_deposits_rewards_and_same_block_changes_use_old_balance() {
    new_test_ext().execute_with(|| {
        seed(1);
        SubnetDelegateStakeSmoothingAlpha::<Test>::put(Q / 2);
        System::set_block_number(1);
        deposit(1, 1000);
        deposit(1, 1000);
        assert_eq!(SubnetDelegateStakeEmas::<Test>::get(1), Some(record(0, 1)));
        System::set_block_number(2);
        assert_ok!(Network::do_increase_delegate_stake(1, 1000));
        assert_eq!(
            SubnetDelegateStakeEmas::<Test>::get(1),
            Some(record(1000, 2))
        );
        assert_eq!(advance(1, 3), record(2000, 3));
        let before = SubnetDelegateStakeEmas::<Test>::get(1);
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        assert_ok!(Network::transfer_delegate_stake(
            RuntimeOrigin::signed(account(1)),
            1,
            account(2),
            shares
        ));
        assert_eq!(SubnetDelegateStakeEmas::<Test>::get(1), before);
    });
}

#[test]
fn ema_allocation_attenuates_late_deposit_and_excludes_ineligible_pool() {
    new_test_ext().execute_with(|| {
        for id in [1, 2, 3] {
            seed(id);
        }
        let block = isolate_delegate_factor(1, &[1, 2]);
        deposit(1, 1000);
        deposit(3, 1_000_000);
        System::set_block_number(block - 1);
        deposit(2, 1000);
        System::set_block_number(block);
        let expected_first = advance(1, block).balance;
        let expected_second = advance(2, block).balance;
        let expected_share =
            (expected_second * U256::from(Q) / (expected_first + expected_second)).as_u128();
        let (weights, _) = Network::calculate_subnet_weights(1);
        assert_eq!(weights.len(), 2);
        assert!(weights[&2].abs_diff(expected_share) <= 128);
        assert!(weights[&1] > weights[&2] * 5);
        assert_eq!(
            SubnetDelegateStakeEmas::<Test>::get(1)
                .unwrap()
                .last_updated_block,
            block
        );
        assert_eq!(weights, Network::calculate_subnet_weights(1).0);
        assert_ok!(Network::do_increase_delegate_stake(2, 1_000_000));
        assert_eq!(weights, Network::calculate_subnet_weights(1).0);
    });
}

#[test]
fn ema_allocation_zero_missing_and_maximum_balances() {
    new_test_ext().execute_with(|| {
        for id in [1, 2] {
            seed(id);
        }
        let block = isolate_delegate_factor(1, &[1, 2]);
        System::set_block_number(block);
        assert!(Network::calculate_subnet_weights(1).0.is_empty());
        for id in [1, 2] {
            TotalSubnetDelegateStakeBalance::<Test>::insert(id, u128::MAX);
            SubnetDelegateStakeEmas::<Test>::insert(id, record(u128::MAX, block));
        }
        let (weights, _) = Network::calculate_subnet_weights(1);
        assert_eq!(weights[&1], Q / 2);
        assert_eq!(weights[&2], Q / 2);
        SubnetDelegateStakeEmas::<Test>::remove(2);
        let (weights, _) = Network::calculate_subnet_weights(1);
        assert_eq!(weights.len(), 1);
        assert_eq!(weights[&1], Q);
        assert_eq!(
            SubnetDelegateStakeEmas::<Test>::get(2),
            Some(record(0, block))
        );
    });
}

#[test]
fn ema_allocation_hook_advances_idle_pools_and_finalizes_their_share() {
    new_test_ext().execute_with(|| {
        seed(1);
        seed(2);
        TotalSubnets::<Test>::put(2);
        let block = isolate_delegate_factor(1, &[1, 2]);
        deposit(1, 1000);
        deposit(2, 3000);
        System::set_block_number(block);
        let expected = advance(1, block);
        assert!(!expected.balance.is_zero());
        Network::on_initialize(block);
        assert_eq!(SubnetDelegateStakeEmas::<Test>::get(1), Some(expected));
        let allocation = FinalSubnetEmissionWeights::<Test>::get(1);
        assert_eq!(allocation.subnet_weights.len(), 2);
        assert_eq!(allocation.subnet_weights[&1], Q / 4);
        assert_eq!(allocation.subnet_weights[&2], Q * 3 / 4);
    });
}

#[test]
fn ema_does_not_change_other_factor_inputs_or_coefficients() {
    new_test_ext().execute_with(|| {
        seed(1);
        seed(2);
        let block = isolate_delegate_factor(1, &[1, 2]);
        let factors = SubnetWeightFactorsData {
            delegate_stake: 0,
            node_count: Q / 2,
            net_flow: Q / 2,
        };
        SubnetWeightFactors::<Test>::put(factors.clone());
        deposit(1, 1000);
        deposit(2, 3000);
        System::set_block_number(block);
        let baseline = Network::calculate_subnet_weights(1).0;
        for (id, balance) in [(1, u128::MAX), (2, 0)] {
            SubnetDelegateStakeEmas::<Test>::insert(id, record(balance, block));
        }
        assert_eq!(Network::calculate_subnet_weights(1).0, baseline);
        assert_eq!(SubnetWeightFactors::<Test>::get(), factors);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(1), 1000);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(2), 3000);
    });
}

#[test]
fn ema_withdrawals_decay_and_pool_generation_reset_preserves_history() {
    new_test_ext().execute_with(|| {
        seed(1);
        SubnetDelegateStakeSmoothingAlpha::<Test>::put(Q / 2);
        deposit(1, 1000);
        // Model a fully redeemable one-unit residual pool, as in the balance-time reset fixture.
        TotalSubnetDelegateStakeBalance::<Test>::insert(1, 1);
        TotalDelegateStake::<Test>::put(1);
        SubnetDelegateStakeEmas::<Test>::insert(1, record(1000, 0));
        System::set_block_number(1);
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        let (result, amount, _) =
            Network::perform_do_remove_subnet_delegate_stake(&account(1), 1, shares, 1, false);
        assert_ok!(result);
        assert_eq!(amount, 1);
        assert_eq!(SubnetDelegatePoolGeneration::<Test>::get(1), 1);
        let withdrawn = SubnetDelegateStakeEmas::<Test>::get(1).unwrap();
        assert_eq!(withdrawn.balance, U256::from(1001 * Q / 2));
        System::set_block_number(2);
        deposit(1, 1000);
        assert_eq!(
            SubnetDelegateStakeEmas::<Test>::get(1).unwrap().balance,
            withdrawn.balance / U256::from(2)
        );
        assert!(advance(1, 3).balance > advance(1, 2).balance);
    });
}

#[test]
fn ema_missing_history_and_deleted_subnet_do_not_backdate_or_recreate() {
    new_test_ext().execute_with(|| {
        seed(1);
        deposit(1, 1000);
        SubnetDelegateStakeEmas::<Test>::remove(1);
        System::set_block_number(100);
        assert_ok!(Network::do_increase_delegate_stake(1, 1000));
        assert_eq!(
            SubnetDelegateStakeEmas::<Test>::get(1),
            Some(record(0, 100))
        );
        SubnetsData::<Test>::remove(1);
        SubnetDelegateStakeEmas::<Test>::remove(1);
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        System::set_block_number(101);
        assert_ok!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(account(1)),
            1,
            shares,
            1
        ));
        assert!(!SubnetDelegateStakeEmas::<Test>::contains_key(1));
    });
}

#[test]
fn ema_failed_operations_and_queue_insertion_roll_back() {
    new_test_ext().execute_with(|| {
        seed(1);
        seed(2);
        deposit(1, 10000);
        let before = SubnetDelegateStakeEmas::<Test>::get(1);
        System::set_block_number(20);
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
        let cap = <Test as Config>::MaxSwapQueueLength::get();
        SwapQueueOrder::<Test>::mutate(|queue| {
            for id in 0..cap {
                queue.try_push(id).unwrap();
            }
        });
        SwapQueueCount::<Test>::put(cap);
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
        assert_eq!(SubnetDelegateStakeEmas::<Test>::get(1), before);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(1), 10000);
        TotalDelegateStake::<Test>::put(u128::MAX);
        assert!(Network::do_increase_delegate_stake(1, 1).is_err());
        assert_eq!(SubnetDelegateStakeEmas::<Test>::get(1), before);
    });
}

#[test]
fn ema_swap_transit_and_refunds_do_not_credit_destination() {
    new_test_ext().execute_with(|| {
        for id in [1, 2, 3] {
            seed(id);
        }
        DelegateStakeCooldownEpochs::<Test>::put(1);
        deposit(1, 10000);
        System::set_block_number(EpochLength::get() / 2);
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
        let source = SubnetDelegateStakeEmas::<Test>::get(1).unwrap();
        assert!(source.balance > U256::zero());
        assert_ok!(Network::swap_from_subnet_to_subnet(
            RuntimeOrigin::signed(account(1)),
            1,
            3,
            shares / 4,
            1,
            u128::MAX,
            u32::MAX
        ));
        let queued = SwapCallQueue::<Test>::get(first_id).unwrap();
        let ready = queued.queued_at_running_block + queued.execute_after_blocks;
        System::set_block_number(ready);
        Network::execute_ready_swap_calls_with_limit(ready, 2, &mut WeightMeter::new());
        assert_eq!(
            SubnetDelegateStakeEmas::<Test>::get(2),
            Some(record(0, ready))
        );
        assert!(advance(2, ready + 1).balance > U256::zero());
        assert_eq!(SubnetDelegateStakeEmas::<Test>::get(3), Some(record(0, 0)));
        assert_eq!(advance(3, ready + 1).balance, U256::zero());
        assert!(QueuedSwapRefundBalance::<Test>::get(account(1)) > 0);
    });
}

#[test]
fn ema_policy_change_is_authorized_validated_and_prospective_for_all_subnets() {
    new_test_ext().execute_with(|| {
        let origin = || RuntimeOrigin::from(pallet_collective::RawOrigin::Members(4, 5));
        for id in [1, 2] {
            seed(id);
            deposit(id, 1000);
        }
        SubnetsData::<Test>::mutate(2, |subnet| {
            subnet.as_mut().unwrap().state = SubnetState::Registered
        });
        SubnetDelegateStakeSmoothingAlpha::<Test>::put(Q / 2);
        System::set_block_number(2);
        assert_noop!(
            Network::set_subnet_delegate_stake_smoothing_alpha(
                RuntimeOrigin::signed(account(1)),
                Q / 4
            ),
            sp_runtime::DispatchError::BadOrigin
        );
        for alpha in [0, Q, u128::MAX] {
            assert_noop!(
                Network::set_subnet_delegate_stake_smoothing_alpha(origin(), alpha),
                Error::<Test>::InvalidPercent
            );
        }
        assert_ok!(Network::set_subnet_delegate_stake_smoothing_alpha(
            origin(),
            Q / 4
        ));
        for id in [1, 2] {
            assert_eq!(
                SubnetDelegateStakeEmas::<Test>::get(id),
                Some(record(750, 2))
            );
            assert_eq!(advance(id, 3).balance, U256::from(8125 * Q / 10));
        }
        System::assert_last_event(
            Event::<Test>::SetSubnetDelegateStakeSmoothingAlpha(Q / 4).into(),
        );
    });
}

#[test]
fn ema_checkpoint_splitting_and_capital_moves_do_not_create_material_weight() {
    new_test_ext().execute_with(|| {
        let alpha = SubnetDelegateStakeSmoothingAlpha::<Test>::get();
        let mut split = record(0, 0);
        for block in 1..=1000 {
            split = Network::checkpoint_subnet_delegate_stake_ema(split, 1000, block, alpha);
        }
        let whole = Network::checkpoint_subnet_delegate_stake_ema(record(0, 0), 1000, 1000, alpha);
        let error = split.balance.max(whole.balance) - split.balance.min(whole.balance);
        assert!(error < U256::from(Q)); // Less than one atomic unit, even after 1,000 checkpoints.

        let mut left = record(0, 0);
        let mut right = record(0, 0);
        for block in 1..=1000 {
            let left_balance = if block % 2 == 0 { 1000 } else { 0 };
            left = Network::checkpoint_subnet_delegate_stake_ema(left, left_balance, block, alpha);
            right = Network::checkpoint_subnet_delegate_stake_ema(
                right,
                1000 - left_balance,
                block,
                alpha,
            );
        }
        let moved = left.balance + right.balance;
        assert!(moved.max(whole.balance) - moved.min(whole.balance) < U256::from(Q));
    });
}
