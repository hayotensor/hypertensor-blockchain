use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{
    assert_ok,
    storage::{with_transaction, TransactionOutcome},
    traits::{Currency, Hooks},
    weights::Weight,
};
use sp_std::collections::btree_map::BTreeMap;

const TOKEN: u128 = 1_000_000_000_000_000_000;
const NODES: u32 = 4;

fn run_slot(subnet: u32, epoch: u32) {
    set_block_to_subnet_slot_epoch(epoch, subnet);
    let consumed = Network::on_initialize(System::block_number());
    assert!(consumed.all_lte(MaximumHooksWeight::get()));
}

fn elected_round() -> (u32, u32, u32) {
    elected_round_with_nodes(NODES)
}

fn elected_round_with_nodes(nodes: u32) -> (u32, u32, u32) {
    increase_epochs(50);
    let name = b"reward-failure-liveness".to_vec();
    build_activated_subnet(name.clone(), 0, nodes, 10_000 * TOKEN, 100 * TOKEN);
    let subnet = SubnetName::<Test>::get(name).unwrap();
    for node in 1..=nodes {
        let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
        assert_ok!(Network::handle_increase_account_validator_delegate_stake(
            &account(99),
            validator,
            100 * TOKEN,
        ));
    }
    seed_equal_validator_delegate_stake_for_subnet(subnet);
    BaseSlashPercentage::<Test>::put(TOKEN / 10);
    MaxSlashAmount::<Test>::put(TOKEN);
    let epoch = SubnetsData::<Test>::get(subnet)
        .unwrap()
        .consensus_eligible_from_subnet_epoch
        .unwrap();
    run_slot(subnet, epoch);
    let proposer = SubnetElectedValidator::<Test>::get(subnet, epoch)
        .unwrap()
        .validator_subnet_node_id;
    let emission_epoch = epoch.saturating_add(1);
    FinalSubnetEmissionWeights::<Test>::insert(
        emission_epoch,
        DistributionData {
            subnets_emissions: 10 * TOKEN,
            subnet_weights: BTreeMap::from([(subnet, TOKEN)]),
        },
    );
    (subnet, epoch, proposer)
}

fn assert_next_round(subnet: u32, old_epoch: u32) {
    assert_eq!(
        ConsensusRoundSettlementStatus::<Test>::get(subnet, old_epoch),
        Some(true)
    );

    assert_eq!(
        ConsensusRoundSettlementStatus::<Test>::get(subnet, old_epoch + 1),
        Some(false)
    );
    assert!(SubnetElectedValidator::<Test>::contains_key(
        subnet,
        old_epoch + 1
    ));
}

#[derive(Clone, Copy, Debug)]
enum CreditFault {
    SubnetDelegateAggregateOverflow,
    SubnetDelegatePoolInvariant,
    NodeAggregateOverflow,
}

#[test]
fn failed_owner_payment_is_queued_and_the_hook_elects_again() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch, proposer) = elected_round();
        run_subnet_consensus_step(subnet, None, None);
        let round = SubnetElectedValidator::<Test>::get(subnet, epoch).unwrap();
        let (rewards, _) = Network::calculate_rewards_with_policy(10 * TOKEN, TOKEN, &round.policy);
        assert!(rewards.subnet_owner_reward > 0);
        let owner = SubnetOwner::<Test>::get(subnet).unwrap();
        let owner_before = Balances::free_balance(&owner);
        let proposer_before = NodeSubnetStake::<Test>::get(proposer, subnet);
        let pool_before = TotalSubnetDelegateStakeBalance::<Test>::get(subnet);
        let capacitor_before = SubnetRewardCapacitor::<Test>::get(subnet);
        let headroom = u128::MAX - Balances::total_issuance();
        drop(Balances::deposit_creating(&account(987_654), headroom));
        assert_eq!(Balances::total_issuance(), u128::MAX);

        run_slot(subnet, epoch + 1);
        assert_next_round(subnet, epoch);
        assert_eq!(Balances::free_balance(&owner), owner_before);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), capacitor_before);
        let pending = PendingConsensusOperations::<Test>::get(0).unwrap();
        assert_eq!(
            pending.operation,
            ConsensusOperation::OwnerReward {
                recipient: owner,
                amount: rewards.subnet_owner_reward,
            }
        );
        assert_eq!(pending.expires_at, System::block_number() + 100_800);
        assert!(NodeSubnetStake::<Test>::get(proposer, subnet) > proposer_before);
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet),
            pool_before + rewards.delegate_stake_rewards
        );
    });
}

#[test]
fn reward_credit_failures_leave_balances_atomic_and_the_hook_elects_again() {
    for fault in [
        CreditFault::SubnetDelegateAggregateOverflow,
        CreditFault::SubnetDelegatePoolInvariant,
        CreditFault::NodeAggregateOverflow,
    ] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch, proposer) = elected_round();
            run_subnet_consensus_step(subnet, None, None);
            let round = SubnetElectedValidator::<Test>::get(subnet, epoch).unwrap();
            let submission = SubnetConsensusSubmission::<Test>::get(subnet, epoch).unwrap();
            assert!(round.policy.base_validator_reward > 0);
            assert!(submission.validator_reward_factor > 0);
            let (rewards, _) =
                Network::calculate_rewards_with_policy(10 * TOKEN, TOKEN, &round.policy);
            assert!(rewards.delegate_stake_rewards > 0);
            assert!(rewards.subnet_node_rewards > 0);

            match fault {
                CreditFault::SubnetDelegateAggregateOverflow => {
                    TotalDelegateStake::<Test>::put(u128::MAX);
                }
                CreditFault::SubnetDelegatePoolInvariant => {
                    // A funded, circulating pool cannot have zero total shares.
                    assert!(TotalSubnetDelegateStakeCirculatingShares::<Test>::get(subnet) > 0);
                    TotalSubnetDelegateStakeShares::<Test>::insert(subnet, 0);
                }
                CreditFault::NodeAggregateOverflow => TotalStake::<Test>::put(u128::MAX),
            }
            let nodes_before: Vec<_> = NodeSubnetStake::<Test>::iter().collect();
            let proposer_before = NodeSubnetStake::<Test>::get(proposer, subnet);
            let validators_before: Vec<_> = ValidatorDelegateStakeBalance::<Test>::iter().collect();
            let validator_total_before = TotalValidatorDelegateStakeBalance::<Test>::get();
            let pool_before = TotalSubnetDelegateStakeBalance::<Test>::get(subnet);
            let delegation_before = TotalDelegateStake::<Test>::get();
            let owner = SubnetOwner::<Test>::get(subnet).unwrap();
            let owner_before = Balances::free_balance(&owner);

            run_slot(subnet, epoch + 1);
            assert_next_round(subnet, epoch);
            assert_eq!(
                Balances::free_balance(&owner),
                owner_before + rewards.subnet_owner_reward
            );

            match fault {
                CreditFault::NodeAggregateOverflow => {
                    // Score rewards and the proposer base reward both fail their checked credit.
                    // Validator-pool allocations inside a failed node credit also roll back.
                    assert_eq!(
                        NodeSubnetStake::<Test>::iter().collect::<Vec<_>>(),
                        nodes_before
                    );
                    assert_eq!(TotalStake::<Test>::get(), u128::MAX);
                    assert_eq!(
                        ValidatorDelegateStakeBalance::<Test>::iter().collect::<Vec<_>>(),
                        validators_before
                    );
                    assert_eq!(
                        TotalValidatorDelegateStakeBalance::<Test>::get(),
                        validator_total_before
                    );
                    assert_eq!(
                        TotalSubnetDelegateStakeBalance::<Test>::get(subnet),
                        pool_before + rewards.delegate_stake_rewards
                    );
                    assert_eq!(
                        TotalDelegateStake::<Test>::get(),
                        delegation_before + rewards.delegate_stake_rewards
                    );
                    let operations: Vec<_> = PendingConsensusOperations::<Test>::iter_values()
                        .map(|pending| pending.operation)
                        .collect();
                    assert_eq!(
                        operations
                            .iter()
                            .filter(|operation| matches!(
                                operation,
                                ConsensusOperation::NodeReward { .. }
                            ))
                            .count(),
                        NODES as usize
                    );
                    assert_eq!(
                        operations
                            .iter()
                            .filter(|operation| matches!(
                                operation,
                                ConsensusOperation::ValidatorReward { .. }
                            ))
                            .count(),
                        1
                    );
                    // Repair only the failing aggregate. Idle retries complete the financial
                    // work without touching the already-finalized round or next election.
                    TotalStake::<Test>::put(
                        nodes_before
                            .iter()
                            .map(|(_, _, amount)| *amount)
                            .sum::<u128>(),
                    );
                    Network::retry_pending_consensus_operations(
                        System::block_number(),
                        &mut WeightMeter::new(),
                    );
                    assert!(NodeSubnetStake::<Test>::get(proposer, subnet) > proposer_before);
                    assert_eq!(PendingConsensusOperations::<Test>::iter().count(), 0);
                    assert_next_round(subnet, epoch);
                }
                CreditFault::SubnetDelegateAggregateOverflow
                | CreditFault::SubnetDelegatePoolInvariant => {
                    assert_eq!(
                        TotalSubnetDelegateStakeBalance::<Test>::get(subnet),
                        pool_before
                    );
                    assert_eq!(TotalDelegateStake::<Test>::get(), delegation_before);
                    assert!(NodeSubnetStake::<Test>::get(proposer, subnet) > proposer_before);
                    assert!(
                        matches!(PendingConsensusOperations::<Test>::get(0).unwrap().operation,
                        ConsensusOperation::SubnetDelegateReward { subnet_id, amount, .. }
                            if subnet_id == subnet && amount == rewards.delegate_stake_rewards)
                    );
                }
            }
        });
    }
}

#[test]
fn slash_accounting_failure_preserves_stake_and_the_hook_elects_again() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch, proposer) = elected_round();
        let nodes_before: Vec<_> = NodeSubnetStake::<Test>::iter().collect();
        let subnet_stake_before = TotalSubnetStake::<Test>::get(subnet);
        let node_reputation_before = SubnetNodeReputation::<Test>::get(subnet, proposer).unwrap();
        TotalStake::<Test>::put(0);
        // No proposal: the hook tries a positive slash, whose aggregate deduction underflows.
        run_slot(subnet, epoch + 1);

        assert_next_round(subnet, epoch);
        assert_eq!(
            NodeSubnetStake::<Test>::iter().collect::<Vec<_>>(),
            nodes_before
        );
        assert_eq!(TotalSubnetStake::<Test>::get(subnet), subnet_stake_before);
        assert_eq!(TotalStake::<Test>::get(), 0);
        assert!(
            SubnetNodeReputation::<Test>::get(subnet, proposer).unwrap() < node_reputation_before
        );
        assert!(network_events().iter().any(|event| matches!(
            event,
            Event::SubnetNodeSlashFailed { subnet_id, subnet_node_id, error }
                if *subnet_id == subnet && *subnet_node_id == proposer
                    && *error == sp_runtime::ArithmeticError::Underflow.into()
        )));
    });
}

#[test]
fn initialize_refunds_only_unused_enqueue_weight_and_persists_every_failure() {
    // No failures, one owner failure, node/proposer failures, and every reward destination.
    // Exercise both branch-dominated small cohorts and the full supported batch/refund.
    let max_nodes = <Test as Config>::MaxSubnetNodesUpperBound::get();
    for (nodes, fault) in (0..4)
        .map(|fault| (NODES, fault))
        .chain([(max_nodes, 0), (max_nodes, 3)])
    {
        new_test_ext().execute_with(|| {
            let (subnet, epoch, _) = elected_round_with_nodes(nodes);
            run_subnet_consensus_step(subnet, None, None);
            if fault == 1 || fault == 3 {
                let headroom = u128::MAX - Balances::total_issuance();
                drop(Balances::deposit_creating(&account(987_654), headroom));
            }
            if fault >= 2 {
                TotalStake::<Test>::put(u128::MAX);
            }
            if fault == 3 {
                TotalDelegateStake::<Test>::put(u128::MAX);
            }
            set_block_to_subnet_slot_epoch(epoch + 1, subnet);
            type W = <Test as Config>::WeightInfo;
            let base = W::on_initialize_base()
                .saturating_add(W::advance_overwatch_epoch_noop())
                .saturating_add(W::emission_slot_selector());
            // Execute the real slot transactionally, then rewind. This captures its full
            // reservation and optional work without duplicating the settlement formula.
            let (reserved, refund): (Weight, Weight) = with_transaction(|| {
                let mut meter = WeightMeter::with_limit(MaximumHooksWeight::get());
                meter.consume(base);
                let refund = Network::run_assigned_subnet_slot(&mut meter, epoch + 1, subnet)
                    .expect("complete slot fits");
                meter.consume(W::execute_ready_swap_selectors());
                TransactionOutcome::Rollback(Ok::<_, sp_runtime::DispatchError>((
                    meter.consumed(),
                    refund,
                )))
            })
            .unwrap();
            let charged = Network::on_initialize(System::block_number());
            assert_eq!(charged, reserved.saturating_sub(refund));
            assert!(charged.all_lte(MaximumHooksWeight::get()));
            assert_next_round(subnet, epoch);
            let expected = match fault {
                0 => 0,
                1 => 1,
                2 => nodes + 1,
                _ => nodes + 3,
            };
            assert_eq!(
                PendingConsensusOperations::<Test>::iter().count(),
                expected as usize
            );
            let batch_events = network_events()
                .into_iter()
                .filter(|event| {
                    matches!(event,
                        Event::ConsensusOperationsQueued { subnet_id, operation_count, .. }
                            if *subnet_id == subnet && *operation_count == expected
                    )
                })
                .count();
            assert_eq!(batch_events, usize::from(expected > 0));
            if fault == 0 {
                assert!(refund.ref_time() > 0);
                // At small cohorts the rejected branch can set the proof floor, so an
                // unused enqueue allowance need not produce a proof refund as well.
                let maximum_enqueue = Network::consensus_operations_enqueue_weight(nodes + 3);
                assert!(refund.all_lte(maximum_enqueue));
                if nodes == max_nodes {
                    assert!(refund.proof_size() > 0);
                    assert_eq!(refund, maximum_enqueue);
                }
            }
            if fault == 3 {
                assert_eq!(refund, Weight::zero());
                // FIFO order remains owner, scored nodes, proposer bonus, subnet delegate pool.
                assert!(matches!(
                    PendingConsensusOperations::<Test>::get(0)
                        .unwrap()
                        .operation,
                    ConsensusOperation::OwnerReward { .. }
                ));
                assert!(matches!(
                    PendingConsensusOperations::<Test>::get(u64::from(nodes + 1))
                        .unwrap()
                        .operation,
                    ConsensusOperation::ValidatorReward { .. }
                ));
                assert!(matches!(
                    PendingConsensusOperations::<Test>::get(u64::from(nodes + 2))
                        .unwrap()
                        .operation,
                    ConsensusOperation::SubnetDelegateReward { .. }
                ));
            }
        });
    }
}

#[test]
fn zero_score_early_return_flushes_all_independent_reward_failures() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch, proposer) = elected_round();
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, proposer).unwrap();
        assert_ok!(Network::propose_attestation(
            RuntimeOrigin::signed(hotkey),
            subnet,
            vec![],
            None,
            None,
            None,
            None,
        ));
        for node in (1..=NODES).filter(|node| *node != proposer) {
            let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
            assert_ok!(Network::attest(
                RuntimeOrigin::signed(hotkey),
                subnet,
                node,
                None
            ));
        }
        let headroom = u128::MAX - Balances::total_issuance();
        drop(Balances::deposit_creating(&account(987_654), headroom));
        TotalStake::<Test>::put(u128::MAX);
        TotalDelegateStake::<Test>::put(u128::MAX);
        run_slot(subnet, epoch + 1);
        assert_next_round(subnet, epoch);
        let operations: Vec<_> = (0..3)
            .map(|id| {
                PendingConsensusOperations::<Test>::get(id)
                    .unwrap()
                    .operation
            })
            .collect();
        assert!(matches!(
            operations[0],
            ConsensusOperation::ValidatorReward { .. }
        ));
        assert!(matches!(
            operations[1],
            ConsensusOperation::OwnerReward { .. }
        ));
        assert!(matches!(
            operations[2],
            ConsensusOperation::SubnetDelegateReward { .. }
        ));
        assert_eq!(PendingConsensusOperations::<Test>::iter().count(), 3);
    });
}

#[test]
fn transaction_layer_exhaustion_cannot_abort_settlement_or_next_election() {
    use frame_support::storage::{
        transactional::{TRANSACTIONAL_LIMIT, TRANSACTION_LEVEL_KEY},
        unhashed,
    };
    let max_nodes = <Test as Config>::MaxSubnetNodesUpperBound::get();
    for (nodes, outcome) in [
        (NODES, "accepted"),
        (max_nodes, "accepted"),
        (NODES, "rejected"),
        (NODES, "missing"),
    ] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch, proposer) = elected_round_with_nodes(nodes);
            if outcome == "accepted" {
                run_subnet_consensus_step(subnet, None, None);
            } else if outcome == "rejected" {
                let hotkey = Network::get_subnet_node_associated_hotkey(subnet, proposer).unwrap();
                assert_ok!(Network::propose_attestation(
                    RuntimeOrigin::signed(hotkey),
                    subnet,
                    (1..=nodes)
                        .map(|subnet_node_id| SubnetNodeConsensusData { subnet_node_id, score: 100 })
                        .collect(),
                    None, None, None, None,
                ));
            }
            let total_stake = TotalStake::<Test>::get();
            let total_delegate_stake = TotalDelegateStake::<Test>::get();
            let issuance = Balances::total_issuance();
            let owner = SubnetOwner::<Test>::get(subnet).unwrap();
            let owner_balance = Balances::free_balance(&owner);
            let pool_balance = TotalSubnetDelegateStakeBalance::<Test>::get(subnet);
            let proposer_stake = NodeSubnetStake::<Test>::get(proposer, subnet);
            if outcome == "accepted" {
                // Every reward destination fails, including credits that do not open a
                // transaction. The entire batch must survive even when a transaction cannot.
                TotalStake::<Test>::put(u128::MAX);
                TotalDelegateStake::<Test>::put(u128::MAX);
                pallet_balances::TotalIssuance::<Test>::put(u128::MAX);
            }
            unhashed::put(TRANSACTION_LEVEL_KEY, &TRANSACTIONAL_LIMIT);
            let blocked: Result<(), sp_runtime::DispatchError> =
                with_transaction(|| TransactionOutcome::Commit(Ok(())));
            assert_eq!(blocked, Err(sp_runtime::TransactionalError::LimitReached.into()));

            run_slot(subnet, epoch + 1);
            assert_next_round(subnet, epoch);
            let count = if outcome == "accepted" { nodes + 3 } else { 1 };
            let operations: Vec<_> = (0..u64::from(count))
                .map(|id| PendingConsensusOperations::<Test>::get(id).unwrap().operation)
                .collect();
            assert_eq!(PendingConsensusOperations::<Test>::iter().count(), count as usize);
            let pending_slash = Network::pending_node_slash_locked_amount(subnet, proposer);
            if outcome == "accepted" {
                assert_eq!(Balances::free_balance(&owner), owner_balance);
                assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(subnet), pool_balance);
                assert_eq!(NodeSubnetStake::<Test>::get(proposer, subnet), proposer_stake);
                assert_eq!(pending_slash, 0);
            } else {
                assert!(pending_slash > 0);
                assert!(matches!(operations[0], ConsensusOperation::NodeSlash { amount, .. } if amount == pending_slash));
                assert_eq!(NodeSubnetStake::<Test>::get(proposer, subnet), proposer_stake);
            }

            // Restore only the injected failure conditions. Idle consumes the original records
            // without replaying consensus; a second idle invocation cannot pay them again.
            unhashed::kill(TRANSACTION_LEVEL_KEY);
            if outcome == "accepted" {
                TotalStake::<Test>::put(total_stake);
                TotalDelegateStake::<Test>::put(total_delegate_stake);
                pallet_balances::TotalIssuance::<Test>::put(issuance);
            }
            Network::on_idle(System::block_number(), Weight::MAX);
            assert_eq!(PendingConsensusOperations::<Test>::iter().count(), 0);
            assert_eq!(Network::pending_node_slash_locked_amount(subnet, proposer), 0);
            if outcome == "accepted" {
                assert!(Balances::free_balance(&owner) > owner_balance);
                assert!(TotalSubnetDelegateStakeBalance::<Test>::get(subnet) > pool_balance);
                assert!(NodeSubnetStake::<Test>::get(proposer, subnet) > proposer_stake);
            } else {
                assert_eq!(NodeSubnetStake::<Test>::get(proposer, subnet), proposer_stake - pending_slash);
            }
            let paid_totals = (Balances::total_issuance(), TotalStake::<Test>::get(), TotalDelegateStake::<Test>::get());
            Network::on_idle(System::block_number(), Weight::MAX);
            assert_eq!(paid_totals, (Balances::total_issuance(), TotalStake::<Test>::get(), TotalDelegateStake::<Test>::get()));
            assert_next_round(subnet, epoch);
        });
    }
}
