use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{
    assert_noop, assert_ok, assert_storage_noop,
    traits::{Currency, Hooks},
    weights::{RuntimeDbWeight, Weight},
};

fn settle_owner_reward(weight_meter: &mut WeightMeter, subnet: u32, amount: u128) {
    let mut operations = Vec::new();
    Network::settle_subnet_owner_reward(weight_meter, &mut operations, subnet, amount);
    weight_meter.consume(Network::consensus_operations_enqueue_weight(
        operations.len() as u32,
    ));
    Network::enqueue_consensus_rewards(subnet, &operations);
}

fn queue_reward(subnet: u32, recipient_id: u32, amount: u128) -> AccountIdOf<Test> {
    if System::block_number() == 0 {
        System::set_block_number(1);
    }
    let recipient = account(recipient_id);
    SubnetOwner::<Test>::insert(subnet, &recipient);
    settle_owner_reward(&mut WeightMeter::new(), subnet, amount);
    recipient
}

fn fund(recipient: &AccountIdOf<Test>) {
    drop(Balances::deposit_creating(recipient, EXISTENTIAL_DEPOSIT));
}

#[test]
fn empty_enqueue_batch_touches_no_storage_and_costs_no_weight() {
    new_test_ext().execute_with(|| {
        assert_storage_noop!(Network::enqueue_consensus_rewards(7, &[]));
        assert_eq!(
            Network::consensus_operations_enqueue_weight(0),
            Weight::zero()
        );
    });
}

#[test]
fn maximum_batch_preserves_each_record_across_fifo_wrap_and_idle_retries() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        let recipient = account(980_050);
        let count = <Test as Config>::MaxSubnetNodesUpperBound::get() + 3;
        let first = u64::MAX - u64::from(count) / 2;
        // One existing failed payment must stay ahead of the appended batch.
        PendingConsensusOperationsQueue::<Test>::put(ConsensusOperationQueueState {
            head: first - 1,
            tail: first,
        });
        PendingConsensusOperations::<Test>::insert(
            first - 1,
            PendingConsensusOperation {
                operation: ConsensusOperation::OwnerReward {
                    recipient: recipient.clone(),
                    amount: 7,
                },
                expires_at: 100_000,
            },
        );
        let operations: Vec<_> = (1..=count)
            .map(|amount| ConsensusOperation::OwnerReward {
                recipient: recipient.clone(),
                amount: u128::from(amount),
            })
            .collect();
        let events_before = System::events().len();
        Network::enqueue_consensus_rewards(7, &operations);
        assert_eq!(System::events().len(), events_before + 1);
        System::assert_last_event(
            Event::ConsensusOperationsQueued {
                subnet_id: 7,
                first_operation_id: first,
                operation_count: count,
                expires_at: 100_810,
            }
            .into(),
        );
        let queue = PendingConsensusOperationsQueue::<Test>::get();
        assert_eq!(queue.head, first - 1);
        assert_eq!(queue.tail, first.wrapping_add(u64::from(count)));
        for (offset, operation) in operations.iter().enumerate() {
            let pending =
                PendingConsensusOperations::<Test>::get(first.wrapping_add(offset as u64))
                    .expect("every failed operation is durable");
            assert_eq!(&pending.operation, operation);
            assert_eq!(pending.expires_at, 100_810);
        }
        // The unchanged idle path pays the existing record and every new record exactly once.
        fund(&recipient);
        idle(Weight::MAX);
        let total = u128::from(count) * u128::from(count + 1) / 2 + 7;
        assert_eq!(
            Balances::free_balance(&recipient),
            EXISTENTIAL_DEPOSIT + total
        );
        assert!(!PendingConsensusOperationsQueue::<Test>::exists());
        assert_eq!(PendingConsensusOperations::<Test>::iter().count(), 0);
        assert_storage_noop!(idle(Weight::MAX));
    });
}

#[test]
fn failed_slash_reservation_does_not_change_records_or_existing_reservations() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        node_stake(100);
        assert_ok!(Network::enqueue_consensus_operation(7, slash(60)));
        assert_noop!(
            Network::enqueue_consensus_operation(7, slash(60)),
            Error::<Test>::NotEnoughStakeToWithdraw
        );
        assert_eq!(Network::pending_node_slash_locked_amount(7, 9), 60);
        assert_eq!(PendingConsensusOperations::<Test>::iter().count(), 1);
    });
}

#[test]
fn slash_reservation_and_enqueue_do_not_need_a_transaction_layer() {
    use frame_support::storage::{
        transactional::{TRANSACTIONAL_LIMIT, TRANSACTION_LEVEL_KEY},
        unhashed,
    };
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        node_stake(100);
        // Reproduce the SDK boundary without consuming the test thread's native call stack.
        unhashed::put(TRANSACTION_LEVEL_KEY, &TRANSACTIONAL_LIMIT);
        assert_ok!(Network::enqueue_consensus_operation(7, slash(60)));
        assert_eq!(Network::pending_node_slash_locked_amount(7, 9), 60);
        assert_eq!(
            PendingConsensusOperations::<Test>::get(0)
                .unwrap()
                .operation,
            slash(60)
        );
        assert_noop!(
            Network::enqueue_consensus_operation(7, slash(60)),
            Error::<Test>::NotEnoughStakeToWithdraw
        );
        unhashed::kill(TRANSACTION_LEVEL_KEY);
        idle(Weight::MAX);
        assert_eq!(NodeSubnetStake::<Test>::get(9, 7), 40);
        assert_eq!(Network::pending_node_slash_locked_amount(7, 9), 0);
        assert_eq!(PendingConsensusOperations::<Test>::iter().count(), 0);
    });
}

fn idle(limit: Weight) -> Weight {
    let used = Network::on_idle(System::block_number(), limit);
    assert!(used.all_lte(limit));
    used
}

fn one_retry_weight() -> Weight {
    <() as WeightInfo>::pending_consensus_operations_empty()
        .saturating_add(<() as WeightInfo>::pending_consensus_operation_read())
        .saturating_add(Network::pending_consensus_operation_step_weight(
            &ConsensusOperation::OwnerReward {
                recipient: account(0),
                amount: 1,
            },
        ))
}

#[test]
fn owner_rewards_preserve_success_zero_and_missing_owner_behavior() {
    new_test_ext().execute_with(|| {
        let recipient = account(980_001);
        fund(&recipient);
        SubnetOwner::<Test>::insert(1, &recipient);
        let before = Balances::free_balance(&recipient);
        settle_owner_reward(&mut WeightMeter::new(), 1, 10);
        assert_eq!(Balances::free_balance(&recipient), before + 10);
        assert_storage_noop!(settle_owner_reward(
            &mut WeightMeter::new(),
            1,
            0
        ));
        assert_storage_noop!(settle_owner_reward(
            &mut WeightMeter::new(),
            2,
            10
        ));
        assert!(!PendingConsensusOperationsQueue::<Test>::exists());
        assert_eq!(PendingConsensusOperations::<Test>::iter().count(), 0);
    });
}

#[test]
fn failed_rewards_remain_separate_and_recover_exactly_once() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        let issuance = Balances::total_issuance();
        let recipient = queue_reward(1, 980_002, 100);
        queue_reward(2, 980_002, 200);
        System::assert_last_event(
            Event::ConsensusOperationsQueued {
                subnet_id: 2,
                expires_at: 100_810,
                first_operation_id: 1,
                operation_count: 1,
            }
            .into(),
        );
        assert_eq!(Balances::total_issuance(), issuance);
        assert_eq!(PendingConsensusOperations::<Test>::iter().count(), 2);
        assert_eq!(
            PendingConsensusOperations::<Test>::get(0)
                .unwrap()
                .expires_at,
            100_810
        );
        assert_eq!(SubnetRewardCapacitor::<Test>::get(1), 0);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(2), 0);

        fund(&recipient);
        let issuance = Balances::total_issuance();
        idle(Weight::MAX);
        assert_eq!(
            Balances::free_balance(&recipient),
            EXISTENTIAL_DEPOSIT + 300
        );
        assert_eq!(Balances::total_issuance(), issuance + 300);
        assert_eq!(PendingConsensusOperations::<Test>::iter().count(), 0);
        assert!(!PendingConsensusOperationsQueue::<Test>::exists());
        assert_storage_noop!(idle(Weight::MAX));
    });
}

#[test]
fn fifo_rotates_failures_without_blocking_other_subnets() {
    new_test_ext().execute_with(|| {
        let first = queue_reward(3, 980_003, 101);
        let second = queue_reward(1, 980_004, 102);
        let third = queue_reward(2, 980_005, 103);
        let original = PendingConsensusOperations::<Test>::get(0).unwrap();
        fund(&second);
        fund(&third);
        idle(one_retry_weight());
        assert_eq!(
            PendingConsensusOperationsQueue::<Test>::get(),
            ConsensusOperationQueueState { head: 1, tail: 4 }
        );
        assert_eq!(
            PendingConsensusOperations::<Test>::get(3).unwrap(),
            original
        );
        assert_eq!(Balances::free_balance(&second), EXISTENTIAL_DEPOSIT);
        idle(one_retry_weight());
        assert_eq!(Balances::free_balance(&second), EXISTENTIAL_DEPOSIT + 102);

        fund(&first);
        System::reset_events();
        idle(Weight::MAX);
        let paid: Vec<_> = System::events()
            .into_iter()
            .filter_map(|record| match record.event {
                RuntimeEvent::Network(Event::ConsensusOperationApplied {
                    operation: ConsensusOperation::OwnerReward { recipient, amount },
                }) => Some((recipient, amount)),
                _ => None,
            })
            .collect();
        assert_eq!(paid, vec![(third, 103), (first, 101)]);
        assert!(!PendingConsensusOperationsQueue::<Test>::exists());
    });
}

#[test]
fn each_failed_payment_is_retried_once_per_hook_without_extending_expiry() {
    new_test_ext().execute_with(|| {
        System::set_block_number(20);
        let recipient = queue_reward(1, 980_006, 123);
        let original = PendingConsensusOperations::<Test>::get(0).unwrap();
        let issuance = Balances::total_issuance();
        for block in 21..25 {
            System::set_block_number(block);
            idle(Weight::MAX);
            let queue = PendingConsensusOperationsQueue::<Test>::get();
            assert_eq!(queue.head, (block - 20) as u64);
            assert_eq!(queue.tail, queue.head + 1);
            assert_eq!(
                PendingConsensusOperations::<Test>::get(queue.head).unwrap(),
                original
            );
            assert_eq!(PendingConsensusOperations::<Test>::iter().count(), 1);
            assert_eq!(Balances::free_balance(&recipient), 0);
            assert_eq!(Balances::total_issuance(), issuance);
        }
    });
}

#[test]
fn expiry_forfeits_at_the_deadline_even_if_no_retry_previously_fitted() {
    for offset in [-1i32, 0, 1] {
        new_test_ext().execute_with(|| {
            System::set_block_number(10);
            let recipient = queue_reward(1, 980_007, 123);
            let expires_at = PendingConsensusOperations::<Test>::get(0)
                .unwrap()
                .expires_at;
            assert_storage_noop!(idle(Weight::zero()));
            fund(&recipient);
            let issuance = Balances::total_issuance();
            System::set_block_number((expires_at as i64 + offset as i64) as u32);
            System::reset_events();
            idle(Weight::MAX);
            assert!(!PendingConsensusOperationsQueue::<Test>::exists());
            assert_eq!(PendingConsensusOperations::<Test>::iter().count(), 0);
            assert_eq!(SubnetRewardCapacitor::<Test>::get(1), 0);
            if offset < 0 {
                assert_eq!(
                    Balances::free_balance(&recipient),
                    EXISTENTIAL_DEPOSIT + 123
                );
                assert_eq!(Balances::total_issuance(), issuance + 123);
                System::assert_last_event(
                    Event::ConsensusOperationApplied {
                        operation: ConsensusOperation::OwnerReward {
                            recipient,
                            amount: 123,
                        },
                    }
                    .into(),
                );
            } else {
                assert_eq!(Balances::free_balance(&recipient), EXISTENTIAL_DEPOSIT);
                assert_eq!(Balances::total_issuance(), issuance);
                System::assert_last_event(
                    Event::ConsensusOperationExpired {
                        operation: ConsensusOperation::OwnerReward {
                            recipient,
                            amount: 123,
                        },
                    }
                    .into(),
                );
            }
        });
    }
}

#[test]
fn insufficient_weight_preserves_the_queue_and_bounds_each_step() {
    new_test_ext().execute_with(|| {
        let recipient = queue_reward(1, 980_008, 100);
        fund(&recipient);
        let complete = one_retry_weight();
        for limit in [
            Weight::zero(),
            Weight::from_parts(complete.ref_time() - 1, complete.proof_size()),
            Weight::from_parts(complete.ref_time(), complete.proof_size() - 1),
        ] {
            assert_storage_noop!(idle(limit));
        }
        idle(complete);
        assert_eq!(
            Balances::free_balance(&recipient),
            EXISTENTIAL_DEPOSIT + 100
        );
        assert!(!PendingConsensusOperationsQueue::<Test>::exists());
    });
}

#[test]
fn pending_owner_rewards_survive_owner_changes_subnet_removal_and_history_cleanup() {
    new_test_ext().execute_with(|| {
        let name = b"pending-owner-removal".to_vec();
        build_activated_subnet(
            name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet = SubnetName::<Test>::get(name).unwrap();
        let original_owner = queue_reward(subnet, 980_009, 100);
        let new_owner = account(980_010);
        SubnetOwner::<Test>::insert(subnet, &new_owner);
        Network::do_remove_subnet(subnet, SubnetRemovalReason::Council);
        assert!(!SubnetsData::<Test>::contains_key(subnet));
        assert!(PendingConsensusOperations::<Test>::contains_key(0));
        fund(&original_owner);
        for _ in 0..5 {
            idle(Weight::MAX);
        }
        assert!(RemovedSubnetCleanupQueue::<Test>::iter_keys()
            .next()
            .is_none());
        assert_eq!(
            Balances::free_balance(&original_owner),
            EXISTENTIAL_DEPOSIT + 100
        );
        assert_eq!(Balances::free_balance(&new_owner), 0);
        assert!(!PendingConsensusOperationsQueue::<Test>::exists());
    });
}

#[test]
fn queue_positions_wrap_without_overwriting_pending_payments() {
    new_test_ext().execute_with(|| {
        PendingConsensusOperationsQueue::<Test>::put(ConsensusOperationQueueState {
            head: u64::MAX - 1,
            tail: u64::MAX - 1,
        });
        let first = queue_reward(1, 980_011, 101);
        let second = queue_reward(2, 980_012, 102);
        let third = queue_reward(3, 980_013, 103);
        fund(&second);
        fund(&third);
        idle(Weight::MAX);
        let queue = PendingConsensusOperationsQueue::<Test>::get();
        assert_eq!(queue, ConsensusOperationQueueState { head: 1, tail: 2 });
        assert!(
            matches!(PendingConsensusOperations::<Test>::get(1).unwrap().operation,
            ConsensusOperation::OwnerReward { recipient, .. } if recipient == first)
        );
        assert_eq!(Balances::free_balance(&second), EXISTENTIAL_DEPOSIT + 102);
        assert_eq!(Balances::free_balance(&third), EXISTENTIAL_DEPOSIT + 103);
        assert_eq!(PendingConsensusOperations::<Test>::iter().count(), 1);
    });
}

fn node_operation() -> ConsensusOperation<AccountIdOf<Test>> {
    let validator = 81;
    ValidatorDelegateStakeBalance::<Test>::insert(validator, 1_000);
    ValidatorDelegateStakeShares::<Test>::insert(
        validator,
        2 * Network::DELEGATE_POOL_MIN_LIQUIDITY,
    );
    ValidatorDelegateStakeCirculatingShares::<Test>::insert(
        validator,
        Network::DELEGATE_POOL_MIN_LIQUIDITY,
    );
    TotalValidatorDelegateStakeBalance::<Test>::put(1_000);
    ValidatorsData::<Test>::insert(
        validator,
        ValidatorData {
            id: validator,
            hotkey: account(81),
            delegate_reward_rate: Network::percentage_factor_as_u128() / 2,
            last_delegate_reward_rate_update: 0,
            delegate_account: Some(DelegateAccount {
                account_id: account(82),
                rate: Network::percentage_factor_as_u128() / 2,
            }),
        },
    );
    Network::prepare_consensus_node_reward(7, 9, validator, 400)
}

fn subnet_pool(subnet: u32) {
    TotalSubnetDelegateStakeBalance::<Test>::insert(subnet, 1_000);
    TotalSubnetDelegateStakeShares::<Test>::insert(
        subnet,
        2 * Network::DELEGATE_POOL_MIN_LIQUIDITY,
    );
    TotalSubnetDelegateStakeCirculatingShares::<Test>::insert(
        subnet,
        Network::DELEGATE_POOL_MIN_LIQUIDITY,
    );
    TotalDelegateStake::<Test>::put(1_000);
}

fn node_stake(amount: u128) {
    NodeSubnetStake::<Test>::insert(9, 7, amount);
    TotalSubnetStake::<Test>::insert(7, amount);
    TotalStake::<Test>::put(amount);
    SubnetNodeValidatorId::<Test>::insert(7, 9, 81);
    ValidatorColdkey::<Test>::insert(81, account(81));
}

fn slash(amount: u128) -> ConsensusOperation<AccountIdOf<Test>> {
    ConsensusOperation::NodeSlash {
        subnet_id: 7,
        subnet_node_id: 9,
        amount,
        attestation_percentage: 0,
    }
}

#[test]
fn node_reward_retries_every_destination_atomically_with_frozen_routes_and_rates() {
    for fault in 0..3 {
        new_test_ext().execute_with(|| {
            System::set_block_number(10);
            let operation = node_operation();
            match fault {
                0 => TotalValidatorDelegateStakeBalance::<Test>::put(u128::MAX),
                1 => TotalAccountDelegateStake::<Test>::put(u128::MAX),
                _ => TotalStake::<Test>::put(u128::MAX),
            }
            assert_noop!(
                Network::try_apply_consensus_operation(&operation),
                sp_runtime::ArithmeticError::Overflow
            );
            assert_ok!(Network::enqueue_consensus_operation(7, operation.clone()));
            idle(Weight::MAX);
            assert_eq!(ValidatorDelegateStakeBalance::<Test>::get(81), 1_000);
            assert_eq!(DelegateAccountStake::<Test>::get(account(82)), 0);
            assert_eq!(NodeSubnetStake::<Test>::get(9, 7), 0);
            assert_eq!(
                PendingConsensusOperations::<Test>::get(1)
                    .unwrap()
                    .operation,
                operation
            );

            // Removing the validator settings cannot redirect or re-split the frozen reward.
            ValidatorsData::<Test>::remove(81);
            TotalValidatorDelegateStakeBalance::<Test>::put(1_000);
            TotalAccountDelegateStake::<Test>::put(0);
            TotalStake::<Test>::put(0);
            idle(Weight::MAX);
            assert_eq!(ValidatorDelegateStakeBalance::<Test>::get(81), 1_200);
            assert_eq!(DelegateAccountStake::<Test>::get(account(82)), 100);
            assert_eq!(NodeSubnetStake::<Test>::get(9, 7), 100);
            assert_eq!(TotalStake::<Test>::get(), 100);
            assert_eq!(TotalSubnetStake::<Test>::get(7), 100);
            assert!(!PendingConsensusOperationsQueue::<Test>::exists());
            assert_storage_noop!(idle(Weight::MAX));
        });
    }
}

#[test]
fn reset_pools_cannot_capture_pending_rewards_and_other_categories_progress() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        let node = node_operation();
        subnet_pool(8);
        let pool = ConsensusOperation::SubnetDelegateReward {
            subnet_id: 8,
            generation: 0,
            amount: 300,
        };
        assert_ok!(Network::enqueue_consensus_operation(7, node.clone()));
        assert_ok!(Network::enqueue_consensus_operation(8, pool.clone()));
        ValidatorDelegatePoolGeneration::<Test>::insert(81, 1);
        SubnetDelegatePoolGeneration::<Test>::insert(8, 1);
        let recipient = queue_reward(9, 980_020, 100);
        fund(&recipient);
        idle(Weight::MAX);
        assert_eq!(
            Balances::free_balance(&recipient),
            EXISTENTIAL_DEPOSIT + 100
        );
        assert_eq!(
            PendingConsensusOperationsQueue::<Test>::get(),
            ConsensusOperationQueueState { head: 3, tail: 5 }
        );
        assert_eq!(ValidatorDelegateStakeBalance::<Test>::get(81), 1_000);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(8), 1_000);
        assert_eq!(NodeSubnetStake::<Test>::get(9, 7), 0);
        System::set_block_number(100_810);
        idle(Weight::MAX);
        assert!(!PendingConsensusOperationsQueue::<Test>::exists());
        assert_eq!(ValidatorDelegateStakeBalance::<Test>::get(81), 1_000);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(8), 1_000);
    });
}

#[test]
fn mixed_categories_follow_one_fifo_and_only_failed_entries_rotate() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        let recipient = queue_reward(1, 980_021, 100);
        let node = node_operation();
        subnet_pool(8);
        node_stake(1_000);
        let operations = vec![
            node,
            ConsensusOperation::ValidatorReward {
                subnet_id: 7,
                subnet_node_id: 9,
                amount: 50,
            },
            ConsensusOperation::SubnetDelegateReward {
                subnet_id: 8,
                generation: 0,
                amount: 300,
            },
            slash(200),
        ];
        for operation in &operations {
            assert_ok!(Network::enqueue_consensus_operation(7, operation.clone()));
        }
        System::reset_events();
        idle(Weight::MAX);
        let applied: Vec<_> = network_events()
            .into_iter()
            .filter_map(|event| match event {
                Event::ConsensusOperationApplied { operation } => Some(operation),
                _ => None,
            })
            .collect();
        assert_eq!(applied, operations);
        assert_eq!(
            PendingConsensusOperationsQueue::<Test>::get(),
            ConsensusOperationQueueState { head: 5, tail: 6 }
        );
        assert_eq!(NodeSubnetStake::<Test>::get(9, 7), 950);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(8), 1_300);
        assert_eq!(Network::pending_node_slash_locked_amount(7, 9), 0);
        fund(&recipient);
        idle(Weight::MAX);
        assert!(!PendingConsensusOperationsQueue::<Test>::exists());
    });
}

#[test]
fn failed_slash_reserves_only_its_amount_and_releases_it_after_payment() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        node_stake(1_000);
        TotalStake::<Test>::put(0);
        let q18 = Network::percentage_factor_as_u128();
        assert_eq!(
            Network::apply_subnet_node_slash(7, 9, 0, q18, q18 / 5, 1_000, 1_000).0,
            0
        );
        assert_eq!(Network::pending_node_slash_locked_amount(7, 9), 200);
        assert_eq!(
            PendingConsensusOperations::<Test>::get(0)
                .unwrap()
                .operation,
            slash(200)
        );
        assert_noop!(
            Network::remove_node_stake(RuntimeOrigin::signed(account(81)), 7, 9, 801),
            Error::<Test>::NodeStakeReservedForPendingSlash
        );
        idle(Weight::MAX);
        assert_eq!(Network::pending_node_slash_locked_amount(7, 9), 200);
        TotalStake::<Test>::put(1_000);
        assert_ok!(Network::remove_node_stake(
            RuntimeOrigin::signed(account(81)),
            7,
            9,
            800
        ));
        idle(Weight::MAX);
        assert_eq!(NodeSubnetStake::<Test>::get(9, 7), 0);
        assert_eq!(TotalStake::<Test>::get(), 0);
        assert!(!PendingNodeSlashReservations::<Test>::contains_key(7, 9));
        assert_storage_noop!(idle(Weight::MAX));
    });
}

#[test]
fn slash_expiry_unlocks_stake_without_waiting_for_idle_and_never_slashes_afterwards() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        node_stake(1_000);
        assert_ok!(Network::enqueue_consensus_operation(7, slash(200)));
        assert_ok!(Network::enqueue_consensus_operation(7, slash(300)));
        assert_eq!(PendingNodeSlashReservations::<Test>::get(7, 9).len(), 1);
        assert_eq!(Network::pending_node_slash_locked_amount(7, 9), 500);
        System::set_block_number(100_900);
        assert_storage_noop!(idle(Weight::zero()));
        assert_eq!(Network::pending_node_slash_locked_amount(7, 9), 0);
        assert_ok!(Network::remove_node_stake(
            RuntimeOrigin::signed(account(81)),
            7,
            9,
            1_000
        ));
        idle(Weight::MAX);
        assert_eq!(TotalStake::<Test>::get(), 0);
        assert!(!PendingNodeSlashReservations::<Test>::contains_key(7, 9));
        assert!(!PendingConsensusOperationsQueue::<Test>::exists());
    });
}

#[test]
fn later_slashes_cannot_spend_reserved_stake_and_deadlines_remain_independent() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        node_stake(1_000);
        assert_ok!(Network::enqueue_consensus_operation(7, slash(600)));
        System::set_block_number(120);
        assert_ok!(Network::enqueue_consensus_operation(7, slash(100)));
        let q18 = Network::percentage_factor_as_u128();
        assert_eq!(
            Network::apply_subnet_node_slash(7, 9, 0, q18, q18, 1_000, 1_000).0,
            300
        );
        assert_eq!(NodeSubnetStake::<Test>::get(9, 7), 700);
        assert_eq!(Network::pending_node_slash_locked_amount(7, 9), 700);
        System::set_block_number(100_900);
        assert_eq!(Network::pending_node_slash_locked_amount(7, 9), 100);
        idle(Weight::MAX);
        assert_eq!(NodeSubnetStake::<Test>::get(9, 7), 600);
        assert!(!PendingNodeSlashReservations::<Test>::contains_key(7, 9));
    });
}

#[test]
fn category_weight_is_reserved_before_any_financial_or_queue_mutation() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        let node = node_operation();
        assert_ok!(Network::enqueue_consensus_operation(7, node.clone()));
        let db: RuntimeDbWeight = <Test as frame_system::Config>::DbWeight::get();
        let complete = db
            .reads(1)
            .saturating_add(<() as WeightInfo>::pending_consensus_operations_empty())
            .saturating_add(<() as WeightInfo>::pending_consensus_operation_read())
            .saturating_add(Network::pending_consensus_operation_step_weight(&node));
        assert_storage_noop!(idle(Weight::from_parts(
            complete.ref_time() - 1,
            complete.proof_size()
        )));
        assert_storage_noop!(idle(Weight::from_parts(
            complete.ref_time(),
            complete.proof_size() - 1
        )));
        idle(complete);
        assert_eq!(NodeSubnetStake::<Test>::get(9, 7), 100);
        assert!(!PendingConsensusOperationsQueue::<Test>::exists());
    });
}

#[test]
fn missing_slash_reservation_rolls_back_the_deduction_and_rotates_past_other_work() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        node_stake(1_000);
        assert_ok!(Network::enqueue_consensus_operation(7, slash(200)));
        PendingNodeSlashReservations::<Test>::remove(7, 9);
        let recipient = queue_reward(1, 980_022, 100);
        fund(&recipient);
        idle(Weight::MAX);
        assert_eq!(NodeSubnetStake::<Test>::get(9, 7), 1_000);
        assert_eq!(TotalStake::<Test>::get(), 1_000);
        assert_eq!(
            Balances::free_balance(&recipient),
            EXISTENTIAL_DEPOSIT + 100
        );
        assert_eq!(
            PendingConsensusOperations::<Test>::get(2)
                .unwrap()
                .operation,
            slash(200)
        );
        System::set_block_number(100_810);
        idle(Weight::MAX);
        assert!(!PendingConsensusOperationsQueue::<Test>::exists());
    });
}

#[test]
fn node_credits_and_slash_reservations_survive_subnet_removal() {
    new_test_ext().execute_with(|| {
        let name = b"pending-node-removal".to_vec();
        build_activated_subnet(
            name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet = SubnetName::<Test>::get(name).unwrap();
        let node = 1;
        let before = NodeSubnetStake::<Test>::get(node, subnet);
        let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
        let coldkey = ValidatorColdkey::<Test>::get(validator).unwrap();
        assert_ok!(Network::enqueue_consensus_operation(
            subnet,
            ConsensusOperation::NodeSlash {
                subnet_id: subnet,
                subnet_node_id: node,
                amount: 200,
                attestation_percentage: 0,
            }
        ));
        assert_ok!(Network::enqueue_consensus_operation(
            subnet,
            ConsensusOperation::ValidatorReward {
                subnet_id: subnet,
                subnet_node_id: node,
                amount: 100,
            }
        ));
        Network::do_remove_subnet(subnet, SubnetRemovalReason::Council);
        assert_eq!(Network::pending_node_slash_locked_amount(subnet, node), 200);
        assert_eq!(
            SubnetNodeValidatorId::<Test>::get(subnet, node),
            Some(validator)
        );
        assert_noop!(
            Network::remove_node_stake(RuntimeOrigin::signed(coldkey), subnet, node, before),
            Error::<Test>::NodeStakeReservedForPendingSlash
        );
        idle(Weight::MAX);
        assert_eq!(NodeSubnetStake::<Test>::get(node, subnet), before - 100);
        assert_eq!(Network::pending_node_slash_locked_amount(subnet, node), 0);
        assert!(!PendingConsensusOperationsQueue::<Test>::exists());
    });
}
