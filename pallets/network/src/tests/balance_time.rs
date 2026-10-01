use super::mock::*;
use super::test_utils::{account, insert_elected_subnet_node};
use crate::{
    AccountSubnetDelegateStakeShares, DelegateStakeCooldownEpochs, Error, NextSwapQueueId,
    QueuedSwapRefundBalance, SubnetBalanceTime, SubnetBalanceTimes, SubnetData,
    SubnetDelegatePoolGeneration, SubnetSlot, SubnetState, SubnetsData, SwapCallQueue,
    SwapQueueCount, SwapQueueOrder, TotalDelegateStake, TotalQueuedSwapPrincipal,
    TotalSubnetDelegateStakeBalance, TotalSubnetDelegateStakeCirculatingShares,
    TotalSubnetDelegateStakeShares, TxRateLimit, NETWORK_SUBNET_EMISSION_SLOT,
};
use frame_support::{assert_err, assert_ok, traits::Currency, weights::WeightMeter};
use sp_core::U256;

fn empty_record(block: u32) -> SubnetBalanceTime {
    SubnetBalanceTime {
        last_updated_block: block,
        current_epoch_area: U256::zero(),
        previous_epoch_area: U256::zero(),
    }
}

// Keep consensus setup minimal: the exact elected round supplies allocation eligibility.
fn seed_subnet(id: u32, block: u32) {
    SubnetsData::<Test>::insert(
        id,
        SubnetData {
            id,
            state: SubnetState::Active,
            consensus_eligible_from_subnet_epoch: Some(0),
            ..Default::default()
        },
    );
    SubnetSlot::<Test>::insert(id, 3 + id);
    SubnetBalanceTimes::<Test>::insert(id, empty_record(block));
    TxRateLimit::<Test>::put(0);
}

fn eligible_for(id: u32, epoch: u32) {
    let allocation_block = epoch * EpochLength::get() + NETWORK_SUBNET_EMISSION_SLOT;
    let local_epoch = Network::get_subnet_epoch_with_block_as_u32(id, allocation_block);
    insert_elected_subnet_node(id, local_epoch, 1);
}

fn checkpoint(id: u32, block: u32) -> SubnetBalanceTime {
    let record = Network::checkpoint_subnet_balance_time(
        SubnetBalanceTimes::<Test>::get(id).unwrap(),
        TotalSubnetDelegateStakeBalance::<Test>::get(id),
        block,
    )
    .unwrap();
    SubnetBalanceTimes::<Test>::insert(id, record.clone());
    record
}

#[test]
fn balance_time_checkpoint_matches_per_block_reference_across_changes_and_gaps() {
    new_test_ext().execute_with(|| {
        let length = EpochLength::get();
        // Independent reference counts each block rather than duplicating checkpoint branches.
        for start in [0, 1, length - 1, length + 7] {
            let mut record = empty_record(start);
            let mut reference = vec![U256::zero(); 20];
            let mut last = start;
            for (i, gap) in [0, 1, length / 2, length, 1, 3 * length + 9, 0, length - 1]
                .into_iter()
                .enumerate()
            {
                let balance = [0, 7, 13, 1000, 1, u128::MAX, 2, 99][i];
                let block = last + gap;
                for b in last..block {
                    reference[(b / length) as usize] += U256::from(balance);
                }
                record = Network::checkpoint_subnet_balance_time(record, balance, block).unwrap();
                let epoch = block / length;
                assert_eq!(record.last_updated_block, block);
                assert_eq!(record.current_epoch_area, reference[epoch as usize]);
                assert_eq!(
                    record.previous_epoch_area,
                    if epoch == 0 {
                        U256::zero()
                    } else {
                        reference[(epoch - 1) as usize]
                    }
                );
                last = block;
            }
        }
    });
}

#[test]
fn balance_time_full_epoch_half_epoch_and_same_block_deposits() {
    new_test_ext().execute_with(|| {
        let length = EpochLength::get();
        seed_subnet(1, 0);
        seed_subnet(2, 0);
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(1),
            1,
            1000
        ));
        System::set_block_number(length / 2);
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(2),
            2,
            1000
        ));
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(2),
            2,
            1000
        ));
        assert_eq!(
            SubnetBalanceTimes::<Test>::get(2)
                .unwrap()
                .current_epoch_area,
            U256::zero()
        );
        eligible_for(1, 1);
        eligible_for(2, 1);
        System::set_block_number(length + NETWORK_SUBNET_EMISSION_SLOT);
        let (weights, _) = Network::get_time_weighted_stake_weights(vec![1, 2], 1);
        assert_eq!(weights[&1], Network::percentage_factor_as_u128() / 2);
        assert_eq!(weights[&2], weights[&1]);
        let expected = U256::from(1000u128 * length as u128);
        assert_eq!(
            SubnetBalanceTimes::<Test>::get(1)
                .unwrap()
                .previous_epoch_area,
            expected
        );
        assert_eq!(
            SubnetBalanceTimes::<Test>::get(2)
                .unwrap()
                .previous_epoch_area,
            expected
        );
    });
}

#[test]
fn balance_time_opening_slot_deposits_do_not_rewrite_completed_epoch() {
    new_test_ext().execute_with(|| {
        let length = EpochLength::get();
        for id in [1, 2] {
            seed_subnet(id, 0);
            assert_ok!(Network::handle_increase_account_delegate_stake(
                &account(id),
                id,
                1000
            ));
            eligible_for(id, 1);
        }
        for slot in [0, 1] {
            System::set_block_number(length + slot);
            assert_ok!(Network::handle_increase_account_delegate_stake(
                &account(1),
                1,
                9000
            ));
        }
        System::set_block_number(length + NETWORK_SUBNET_EMISSION_SLOT);
        let (first, _) = Network::get_time_weighted_stake_weights(vec![1, 2], 1);
        let before = SubnetBalanceTimes::<Test>::get(1).unwrap();
        let (repeated, _) = Network::get_time_weighted_stake_weights(vec![1, 2], 1);
        assert_eq!(first, repeated);
        assert_eq!(first[&1], first[&2]);
        assert_eq!(SubnetBalanceTimes::<Test>::get(1), Some(before));
    });
}

#[test]
fn balance_time_rewards_count_from_credit_and_share_transfer_preserves_area() {
    new_test_ext().execute_with(|| {
        let length = EpochLength::get();
        seed_subnet(1, 0);
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(1),
            1,
            1000
        ));
        System::set_block_number(length / 2);
        assert_ok!(Network::do_increase_delegate_stake(1, 1000));
        let before_transfer = SubnetBalanceTimes::<Test>::get(1).unwrap();
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        System::set_block_number(length * 3 / 4);
        assert_ok!(Network::transfer_delegate_stake(
            RuntimeOrigin::signed(account(1)),
            1,
            account(2),
            shares,
        ));
        assert_eq!(SubnetBalanceTimes::<Test>::get(1), Some(before_transfer));
        assert_eq!(
            checkpoint(1, length).previous_epoch_area,
            U256::from(1500u128 * length as u128)
        );
    });
}

#[test]
fn balance_time_withdrawal_stops_credit_and_deleted_pool_does_not_recreate_record() {
    new_test_ext().execute_with(|| {
        let length = EpochLength::get();
        seed_subnet(1, 0);
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(1),
            1,
            1000
        ));
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        System::set_block_number(length / 2);
        assert_ok!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(account(1)),
            1,
            shares / 2,
            1
        ));
        let remaining = TotalSubnetDelegateStakeBalance::<Test>::get(1);
        assert!(remaining < 1000);
        let area = checkpoint(1, length).previous_epoch_area;
        assert_eq!(area, U256::from((1000 + remaining) * (length / 2) as u128));
        SubnetsData::<Test>::remove(1);
        SubnetBalanceTimes::<Test>::remove(1);
        let remaining_shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        System::set_block_number(length + 1);
        assert_ok!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(account(1)),
            1,
            remaining_shares,
            1
        ));
        assert!(!SubnetBalanceTimes::<Test>::contains_key(1));
    });
}

#[test]
fn balance_time_drain_and_redeposit_keeps_earned_area_across_pool_generation() {
    new_test_ext().execute_with(|| {
        let length = EpochLength::get();
        seed_subnet(1, 0);
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(1),
            1,
            1000
        ));
        // A pre-existing one-unit pool with redeemable circulating shares can be fully drained.
        TotalSubnetDelegateStakeBalance::<Test>::insert(1, 1);
        TotalDelegateStake::<Test>::put(1);
        System::set_block_number(length / 4);
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        let (result, amount, _) =
            Network::perform_do_remove_subnet_delegate_stake(&account(1), 1, shares, 1, false);
        assert_ok!(result);
        assert_eq!(amount, 1);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(1), 0);
        assert_eq!(SubnetDelegatePoolGeneration::<Test>::get(1), 1);
        System::set_block_number(length / 2);
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(2),
            1,
            1000
        ));
        assert_eq!(
            checkpoint(1, length).previous_epoch_area,
            U256::from((length / 4) as u128 + 1000 * (length / 2) as u128)
        );
    });
}

#[test]
fn balance_time_missing_record_fails_deposit_reward_and_withdrawal_without_writes() {
    new_test_ext().execute_with(|| {
        seed_subnet(1, 0);
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(1),
            1,
            1000
        ));
        SubnetBalanceTimes::<Test>::remove(1);
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        let _ = Balances::deposit_creating(&account(1), 2000);
        let wallet = Balances::free_balance(account(1));
        System::set_block_number(10);
        assert_err!(
            Network::add_subnet_delegate_stake(RuntimeOrigin::signed(account(1)), 1, 1000, 1),
            Error::<Test>::MissingSubnetBalanceTime
        );
        assert_eq!(Balances::free_balance(account(1)), wallet);
        assert_err!(
            Network::do_increase_delegate_stake(1, 1000),
            Error::<Test>::MissingSubnetBalanceTime
        );
        assert_err!(
            Network::remove_delegate_stake(RuntimeOrigin::signed(account(1)), 1, shares, 1),
            Error::<Test>::MissingSubnetBalanceTime
        );
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(1), 1000);
        assert_eq!(TotalDelegateStake::<Test>::get(), 1000);
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1),
            shares
        );
        assert!(!SubnetBalanceTimes::<Test>::contains_key(1));
    });
}

#[test]
fn balance_time_failed_slippage_and_overflow_keep_checkpoint_unchanged() {
    new_test_ext().execute_with(|| {
        seed_subnet(1, 0);
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(1),
            1,
            1000
        ));
        System::set_block_number(10);
        let before = SubnetBalanceTimes::<Test>::get(1);
        assert!(Network::handle_increase_account_delegate_stake_with_limit(
            &account(2),
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
        TotalDelegateStake::<Test>::put(u128::MAX);
        assert!(Network::do_increase_delegate_stake(1, 1).is_err());
        assert_eq!(SubnetBalanceTimes::<Test>::get(1), before);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(1), 1000);
    });
}

#[test]
fn balance_time_empty_epoch_zero_and_missing_record_allocation_are_atomic() {
    new_test_ext().execute_with(|| {
        assert!(Network::get_time_weighted_stake_weights(vec![], 0)
            .0
            .is_empty());
        seed_subnet(1, 0);
        seed_subnet(2, 0);
        assert!(Network::get_time_weighted_stake_weights(vec![1, 2], 0)
            .0
            .values()
            .all(|v| *v == 0));
        for id in [1, 2] {
            eligible_for(id, 1);
        }
        System::set_block_number(EpochLength::get() + NETWORK_SUBNET_EMISSION_SLOT);
        assert!(Network::get_time_weighted_stake_weights(vec![1, 2], 1)
            .0
            .values()
            .all(|v| *v == 0));
        SubnetBalanceTimes::<Test>::insert(1, empty_record(0));
        SubnetBalanceTimes::<Test>::remove(2);
        TotalSubnetDelegateStakeBalance::<Test>::insert(1, 1000);
        let before = SubnetBalanceTimes::<Test>::get(1);
        assert!(Network::get_time_weighted_stake_weights(vec![1, 2], 1)
            .0
            .is_empty());
        assert_eq!(SubnetBalanceTimes::<Test>::get(1), before);
    });
}

#[test]
fn balance_time_normalization_handles_large_balances_and_rounds_down() {
    new_test_ext().execute_with(|| {
        for id in 1..=3 {
            seed_subnet(id, 0);
            TotalSubnetDelegateStakeBalance::<Test>::insert(id, u128::MAX);
            eligible_for(id, 1);
        }
        System::set_block_number(EpochLength::get() + NETWORK_SUBNET_EMISSION_SLOT);
        let (weights, _) = Network::get_time_weighted_stake_weights(vec![1, 2, 3], 1);
        assert_eq!(weights.len(), 3);
        assert!(weights
            .values()
            .all(|v| *v == Network::percentage_factor_as_u128() / 3));
        assert_eq!(
            weights.values().sum::<u128>(),
            Network::percentage_factor_as_u128() - 1
        );
        assert!(
            SubnetBalanceTimes::<Test>::get(1)
                .unwrap()
                .previous_epoch_area
                > U256::from(u128::MAX)
        );
    });
}

#[test]
fn balance_time_skipped_allocations_and_paused_subnets_keep_exact_previous_window() {
    new_test_ext().execute_with(|| {
        let length = EpochLength::get();
        seed_subnet(1, 0);
        seed_subnet(2, 0);
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(1),
            1,
            1000
        ));
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(2),
            2,
            1000
        ));
        System::set_block_number(length / 2);
        assert_ok!(Network::do_increase_delegate_stake(1, 1000));
        SubnetsData::<Test>::mutate(1, |data| data.as_mut().unwrap().state = SubnetState::Paused);
        // No allocation hooks run during this gap. Eligibility still comes from the exact round.
        for id in [1, 2] {
            eligible_for(id, 9);
        }
        System::set_block_number(9 * length + NETWORK_SUBNET_EMISSION_SLOT);
        let (weights, _) = Network::get_time_weighted_stake_weights(vec![1, 2], 9);
        assert_eq!(weights[&1], Network::percentage_factor_as_u128() * 2 / 3);
        assert_eq!(
            SubnetBalanceTimes::<Test>::get(1)
                .unwrap()
                .previous_epoch_area,
            U256::from(2000u128 * length as u128)
        );
        assert_eq!(
            SubnetBalanceTimes::<Test>::get(2)
                .unwrap()
                .previous_epoch_area,
            U256::from(1000u128 * length as u128)
        );
    });
}

#[test]
fn balance_time_registration_does_not_backdate_and_ineligible_pool_does_not_dilute() {
    new_test_ext().execute_with(|| {
        let length = EpochLength::get();
        seed_subnet(1, 0);
        TotalSubnetDelegateStakeBalance::<Test>::insert(1, 1000);
        System::set_block_number(length / 2);
        seed_subnet(2, length / 2);
        TotalSubnetDelegateStakeBalance::<Test>::insert(2, 1000);
        seed_subnet(3, length / 2);
        TotalSubnetDelegateStakeBalance::<Test>::insert(3, u128::MAX);
        for id in [1, 2] {
            eligible_for(id, 1);
        }
        System::set_block_number(length + NETWORK_SUBNET_EMISSION_SLOT);
        let (weights, _) = Network::get_time_weighted_stake_weights(vec![1, 2, 3], 1);
        assert_eq!(weights[&1], Network::percentage_factor_as_u128() * 2 / 3);
        assert_eq!(weights[&2], Network::percentage_factor_as_u128() / 3);
        assert!(!weights.contains_key(&3));
        assert_eq!(
            SubnetBalanceTimes::<Test>::get(2)
                .unwrap()
                .previous_epoch_area,
            U256::from(1000u128 * (length / 2) as u128)
        );
    });
}

#[test]
fn balance_time_queued_swap_excludes_transit_and_refunds_never_credit_destination() {
    new_test_ext().execute_with(|| {
        let length = EpochLength::get();
        DelegateStakeCooldownEpochs::<Test>::put(1);
        for id in 1..=3 {
            seed_subnet(id, 0);
        }
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(1),
            1,
            10000
        ));
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        System::set_block_number(length / 2);
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
        let remaining_shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        assert_ok!(Network::swap_from_subnet_to_subnet(
            RuntimeOrigin::signed(account(1)),
            1,
            3,
            remaining_shares,
            1,
            u128::MAX,
            u32::MAX
        ));
        let first = SwapCallQueue::<Test>::get(first_id).unwrap();
        let second = SwapCallQueue::<Test>::get(second_id).unwrap();
        let moved = first.call.get_queue_balance();
        let refunded = second.call.get_queue_balance();
        let source_balance = TotalSubnetDelegateStakeBalance::<Test>::get(1);
        let ready = first.queued_at_block + first.execute_after_blocks;
        System::set_block_number(ready);
        Network::execute_ready_swap_calls_with_limit(ready, 2, &mut WeightMeter::new());
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(2), moved);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(3), 0);
        assert_eq!(QueuedSwapRefundBalance::<Test>::get(account(1)), refunded);
        let end = 2 * length;
        let first_area = checkpoint(1, end).previous_epoch_area;
        let second_area = checkpoint(2, end).previous_epoch_area;
        let third_area = checkpoint(3, end).previous_epoch_area;
        assert_eq!(first_area, U256::from(source_balance * length as u128));
        assert_eq!(second_area, U256::from(moved * (end - ready) as u128));
        assert_eq!(third_area, U256::zero());
        assert!(first_area + second_area + third_area < U256::from(10000u128 * length as u128));
    });
}

#[test]
fn balance_time_failed_queue_insertion_rolls_back_source_checkpoint() {
    new_test_ext().execute_with(|| {
        seed_subnet(1, 0);
        seed_subnet(2, 0);
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(1),
            1,
            10000
        ));
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1);
        let before = SubnetBalanceTimes::<Test>::get(1);
        let cap = <Test as crate::Config>::MaxSwapQueueLength::get();
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
        assert_eq!(SubnetBalanceTimes::<Test>::get(1), before);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(1), 10000);
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(account(1), 1),
            shares
        );
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
    });
}

#[test]
fn balance_time_repeated_capital_moves_cannot_increase_total_area() {
    new_test_ext().execute_with(|| {
        let length = EpochLength::get();
        for id in [1, 2] {
            seed_subnet(id, 0);
            assert_ok!(Network::handle_increase_account_delegate_stake(
                &account(1),
                id,
                10000
            ));
        }
        for step in 1..10 {
            System::set_block_number(step * length / 10);
            let source = 1 + step % 2;
            let destination = 3 - source;
            let shares = AccountSubnetDelegateStakeShares::<Test>::get(account(1), source) / 2;
            let (result, amount, _) = Network::perform_do_remove_subnet_delegate_stake(
                &account(1),
                source,
                shares,
                1,
                false,
            );
            assert_ok!(result);
            assert_ok!(Network::handle_increase_account_delegate_stake(
                &account(1),
                destination,
                amount
            ));
            assert_eq!(
                TotalSubnetDelegateStakeBalance::<Test>::get(1)
                    + TotalSubnetDelegateStakeBalance::<Test>::get(2),
                20000
            );
        }
        assert_eq!(
            checkpoint(1, length).previous_epoch_area + checkpoint(2, length).previous_epoch_area,
            U256::from(20000u128 * length as u128)
        );
    });
}

#[test]
fn balance_time_large_withdrawals_no_longer_depend_on_signed_flow_range() {
    new_test_ext().execute_with(|| {
        seed_subnet(1, 0);
        const ACCOUNT_SHARES: u128 = 2_000_000_000;
        const TOTAL_SHARES: u128 = 3_000_000_000;
        AccountSubnetDelegateStakeShares::<Test>::insert(account(1), 1, ACCOUNT_SHARES);
        TotalSubnetDelegateStakeShares::<Test>::insert(1, TOTAL_SHARES);
        TotalSubnetDelegateStakeCirculatingShares::<Test>::insert(1, ACCOUNT_SHARES);
        TotalSubnetDelegateStakeBalance::<Test>::insert(1, u128::MAX);
        TotalDelegateStake::<Test>::put(u128::MAX);
        System::set_block_number(EpochLength::get() / 2);
        let (result, removed, _) = Network::perform_do_remove_subnet_delegate_stake(
            &account(1),
            1,
            ACCOUNT_SHARES,
            1,
            false,
        );
        assert_ok!(result);
        assert!(removed > i128::MAX as u128);
        assert_eq!(
            SubnetBalanceTimes::<Test>::get(1)
                .unwrap()
                .current_epoch_area,
            U256::from(u128::MAX) * U256::from(EpochLength::get() / 2)
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(1),
            u128::MAX - removed
        );
    });
}

#[test]
fn balance_time_repeated_allocation_after_later_epoch_deposit_keeps_completed_share() {
    new_test_ext().execute_with(|| {
        let length = EpochLength::get();
        for id in [1, 2] {
            seed_subnet(id, 0);
            assert_ok!(Network::handle_increase_account_delegate_stake(
                &account(id),
                id,
                1000
            ));
            eligible_for(id, 1);
        }
        System::set_block_number(length + NETWORK_SUBNET_EMISSION_SLOT);
        let (original, _) = Network::get_time_weighted_stake_weights(vec![1, 2], 1);
        System::set_block_number(length + length / 2);
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(1),
            1,
            9000
        ));
        assert_ok!(Network::do_increase_delegate_stake(2, 1000));
        let first_record = SubnetBalanceTimes::<Test>::get(1);
        let second_record = SubnetBalanceTimes::<Test>::get(2);
        let (repeated, _) = Network::get_time_weighted_stake_weights(vec![1, 2], 1);
        assert_eq!(repeated, original);
        assert_eq!(SubnetBalanceTimes::<Test>::get(1), first_record);
        assert_eq!(SubnetBalanceTimes::<Test>::get(2), second_record);
    });
}

#[test]
fn balance_time_factor_changes_final_emissions_without_changing_other_inputs() {
    new_test_ext().execute_with(|| {
        let length = EpochLength::get();
        let percentage_factor = Network::percentage_factor_as_u128();
        for id in [1, 2] {
            seed_subnet(id, 0);
            eligible_for(id, 1);
        }
        // The same current balance and elected-node count have different retention durations.
        System::set_block_number(length / 4);
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(1),
            1,
            1000
        ));
        System::set_block_number(3 * length / 4);
        assert_ok!(Network::handle_increase_account_delegate_stake(
            &account(2),
            2,
            1000
        ));
        crate::SubnetDistributionPower::<Test>::put(percentage_factor);
        crate::DefaultOverwatchSubnetWeight::<Test>::put(percentage_factor);
        crate::LatestEffectiveOverwatchSignal::<Test>::kill();
        let factors = crate::SubnetWeightFactors::<Test>::get();
        assert_eq!(factors.delegate_stake, percentage_factor * 2 / 5);
        assert_eq!(factors.node_count, percentage_factor * 2 / 5);
        assert_eq!(factors.time_weighted_stake, percentage_factor / 5);
        System::set_block_number(length + NETWORK_SUBNET_EMISSION_SLOT);

        let (weighted, _) = Network::calculate_subnet_weights(1);
        assert_eq!(weighted.len(), 2);
        assert_eq!(
            SubnetBalanceTimes::<Test>::get(1)
                .unwrap()
                .previous_epoch_area,
            SubnetBalanceTimes::<Test>::get(2)
                .unwrap()
                .previous_epoch_area
                * U256::from(3)
        );
        // 0.4 * 0.5 + 0.4 * 0.5 + 0.2 * (0.75 or 0.25) = 0.55 or 0.45.
        // Existing final normalization uses f64; tolerate one Q18-scale float rounding step.
        assert!(weighted[&1].abs_diff(percentage_factor * 55 / 100) <= 128);
        assert!(weighted[&2].abs_diff(percentage_factor * 45 / 100) <= 128);

        assert_ok!(Network::do_set_subnet_weight_factors(crate::SubnetWeightFactorsData {
            delegate_stake: percentage_factor / 2,
            node_count: percentage_factor / 2,
            time_weighted_stake: 0,
        }));
        let (without_retention, _) = Network::calculate_subnet_weights(1);
        assert_eq!(without_retention[&1], percentage_factor / 2);
        assert_eq!(without_retention[&2], percentage_factor / 2);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(1), 1000);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(2), 1000);
    });
}
