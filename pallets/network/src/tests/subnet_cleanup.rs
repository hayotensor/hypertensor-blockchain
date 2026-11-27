use super::{mock::*, test_utils::*};
use crate::{weights::WeightInfo, *};
use frame_support::{
    assert_noop, assert_ok,
    traits::{Currency, Hooks},
    weights::{Weight, WeightMeter},
};

const DEPOSIT: u128 = 10_000_000_000_000_000_000_000;

fn fixture(class: SubnetNodeClass, pending: bool) -> (u32, u32, AccountId) {
    build_activated_subnet(
        b"deferred-removal".to_vec(),
        0,
        4,
        DEPOSIT,
        MinSubnetMinStake::<Test>::get(),
    );
    let subnet = SubnetName::<Test>::get(b"deferred-removal".to_vec()).unwrap();
    let node = if class == SubnetNodeClass::Queued {
        build_queued_subnet_nodes(subnet, 4, 5, DEPOSIT, MinSubnetMinStake::<Test>::get());
        5
    } else {
        if class == SubnetNodeClass::Probation {
            assert_ok!(Network::remove_node_from_election_slot(subnet, 4));
            SubnetNodesData::<Test>::mutate(subnet, 4, |n| n.classification.node_class = class);
        }
        4
    };
    if pending {
        if class == SubnetNodeClass::Queued {
            PendingQueuedNodeRemovals::<Test>::mutate(subnet, |ids| {
                ids.try_insert(node).unwrap();
            });
        } else {
            PendingAdmittedNodeRemovals::<Test>::mutate(subnet, |ids| {
                ids.try_insert(node).unwrap();
            });
        }
    }
    TxRateLimit::<Test>::put(0);
    (
        subnet,
        node,
        Network::get_subnet_node_associated_coldkey(subnet, node).unwrap(),
    )
}

fn one_cleanup_step() {
    let stage = RemovedSubnetCleanupQueue::<Test>::iter_values()
        .next()
        .unwrap();
    let limit = <() as WeightInfo>::removed_subnet_cleanup_selector()
        .saturating_add(Network::removed_subnet_cleanup_step_weight(stage))
        .saturating_add(<() as WeightInfo>::removed_subnet_cleanup_progress());
    let mut meter = WeightMeter::with_limit(limit);
    Network::cleanup_removed_subnets(&mut meter);
    assert!(meter.consumed().all_lte(limit));
}

#[test]
fn every_node_class_can_unstake_on_the_first_call_without_idle() {
    for class in [
        SubnetNodeClass::Queued,
        SubnetNodeClass::Probation,
        SubnetNodeClass::Active,
    ] {
        for pending in [false, true] {
            new_test_ext().execute_with(|| {
                let (subnet, node, coldkey) = fixture(class, pending);
                let stake = NodeSubnetStake::<Test>::get(node, subnet);
                let total = TotalStake::<Test>::get();
                let subnet_total = TotalSubnetStake::<Test>::get(subnet);
                let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
                Network::do_remove_subnet(subnet, SubnetRemovalReason::Owner);
                assert!(SubnetNodeReputation::<Test>::contains_key(subnet, node));
                assert!(SubnetMinStakeBalance::<Test>::contains_key(subnet));
                assert!(
                    SubnetNodesData::<Test>::contains_key(subnet, node)
                        || QueuedSubnetNodesData::<Test>::contains_key(subnet, node)
                );
                assert_eq!(TotalNodes::<Test>::get(), 0);
                assert_eq!(TotalAdmittedNodes::<Test>::get(), 0);
                assert_eq!(TotalElectableNodes::<Test>::get(), 0);
                assert_noop!(
                    Network::remove_node_stake(
                        RuntimeOrigin::signed(account(987_654)),
                        subnet,
                        node,
                        stake
                    ),
                    Error::<Test>::NotKeyOwner
                );
                assert_ok!(Network::remove_node_stake(
                    RuntimeOrigin::signed(coldkey.clone()),
                    subnet,
                    node,
                    stake
                ));
                assert_eq!(NodeSubnetStake::<Test>::get(node, subnet), 0);
                assert_eq!(TotalStake::<Test>::get(), total - stake);
                assert_eq!(TotalSubnetStake::<Test>::get(subnet), subnet_total - stake);
                let ledger = StakeUnbondingLedger::<Test>::get(&coldkey);
                assert_eq!(ledger.values().map(|e| e.network).sum::<u128>(), stake);
                let claim_block = *ledger.keys().max().unwrap();
                let balance = Balances::free_balance(&coldkey);
                System::set_block_number(claim_block);
                assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
                    coldkey.clone()
                )));
                assert_eq!(Balances::free_balance(&coldkey), balance + stake);
                // Claiming does not run cleanup and never requires live membership.
                assert!(RemovedSubnetCleanupQueue::<Test>::contains_key(subnet));
                drain_removed_subnets();
                assert_eq!(
                    SubnetNodeValidatorId::<Test>::get(subnet, node),
                    Some(validator)
                );
                assert_eq!(TotalSubnetStake::<Test>::get(subnet), subnet_total - stake);
                assert_eq!(TotalNodes::<Test>::get(), 0);
            });
        }
    }
}

#[test]
fn partial_cleanup_and_coldkey_rotation_do_not_change_withdrawal_rights() {
    new_test_ext().execute_with(|| {
        let (subnet, node, old_key) = fixture(SubnetNodeClass::Active, true);
        let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
        let stake = NodeSubnetStake::<Test>::get(node, subnet);
        Network::do_remove_subnet(subnet, SubnetRemovalReason::Owner);
        for _ in 0..15 {
            one_cleanup_step();
        }
        let new_key = account(987_653);
        assert_ok!(Network::update_validator_coldkey(
            RuntimeOrigin::signed(old_key.clone()),
            validator,
            new_key.clone()
        ));
        assert_noop!(
            Network::remove_node_stake(RuntimeOrigin::signed(old_key), subnet, node, stake),
            Error::<Test>::NotKeyOwner
        );
        assert_ok!(Network::remove_node_stake(
            RuntimeOrigin::signed(new_key.clone()),
            subnet,
            node,
            stake / 2
        ));
        drain_removed_subnets();
        assert_ok!(Network::remove_node_stake(
            RuntimeOrigin::signed(new_key.clone()),
            subnet,
            node,
            stake - stake / 2
        ));
        assert_eq!(
            StakeUnbondingLedger::<Test>::get(&new_key)
                .values()
                .map(|e| e.network)
                .sum::<u128>(),
            stake
        );
    });
}

#[test]
fn ledger_failure_rolls_back_withdrawal_with_residual_nodes() {
    new_test_ext().execute_with(|| {
        let (subnet, node, coldkey) = fixture(SubnetNodeClass::Queued, true);
        let stake = NodeSubnetStake::<Test>::get(node, subnet);
        Network::do_remove_subnet(subnet, SubnetRemovalReason::Owner);
        MaxUnbondings::<Test>::put(0);
        assert_noop!(
            Network::remove_node_stake(RuntimeOrigin::signed(coldkey), subnet, node, stake),
            Error::<Test>::MaxUnlockingsReached
        );
    });
}

#[test]
fn financial_retries_remain_separate_from_subnet_garbage_collection() {
    new_test_ext().execute_with(|| {
        let (subnet, node, coldkey) = fixture(SubnetNodeClass::Active, true);
        let stake = NodeSubnetStake::<Test>::get(node, subnet);
        assert_ok!(Network::enqueue_consensus_operation(
            subnet,
            ConsensusOperation::NodeSlash {
                subnet_id: subnet,
                subnet_node_id: node,
                amount: 200,
                attestation_percentage: 0,
            }
        ));
        Network::do_remove_subnet(subnet, SubnetRemovalReason::Owner);
        assert_noop!(
            Network::remove_node_stake(RuntimeOrigin::signed(coldkey.clone()), subnet, node, stake),
            Error::<Test>::NodeStakeReservedForPendingSlash
        );
        assert_ok!(Network::remove_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet,
            node,
            stake - 200
        ));
        drain_removed_subnets();
        assert_eq!(Network::pending_node_slash_locked_amount(subnet, node), 200);
        Network::on_idle(System::block_number(), Weight::MAX);
        assert_eq!(NodeSubnetStake::<Test>::get(node, subnet), 0);
        assert_eq!(Network::pending_node_slash_locked_amount(subnet, node), 0);
        assert_ok!(Network::enqueue_consensus_operation(
            subnet,
            ConsensusOperation::ValidatorReward {
                subnet_id: subnet,
                subnet_node_id: node,
                amount: 300,
            }
        ));
        Network::on_idle(System::block_number(), Weight::MAX);
        assert_ok!(Network::remove_node_stake(
            RuntimeOrigin::signed(coldkey),
            subnet,
            node,
            300
        ));
        assert!(!SubnetNodeReputation::<Test>::contains_key(subnet, node));
        assert!(!SubnetDelegateStakeEmas::<Test>::contains_key(subnet));
        assert!(!SubnetNetFlows::<Test>::contains_key(subnet));
    });
}

#[test]
fn replacement_subnet_and_counters_survive_old_cleanup_and_repeated_removal() {
    new_test_ext().execute_with(|| {
        let (old, node, coldkey) = fixture(SubnetNodeClass::Active, true);
        let slot = SubnetSlot::<Test>::get(old).unwrap();
        let friendly = SubnetIdFriendlyUid::<Test>::get(old).unwrap();
        Network::do_remove_subnet(old, SubnetRemovalReason::Owner);
        one_cleanup_step();
        let stage = RemovedSubnetCleanupQueue::<Test>::get(old);
        Network::do_remove_subnet(old, SubnetRemovalReason::Council);
        assert_eq!(RemovedSubnetCleanupQueue::<Test>::get(old), stage);
        build_activated_subnet(
            b"deferred-removal".to_vec(),
            0,
            4,
            DEPOSIT,
            MinSubnetMinStake::<Test>::get(),
        );
        let new = SubnetName::<Test>::get(b"deferred-removal".to_vec()).unwrap();
        assert_ne!(old, new);
        assert_eq!(SubnetSlot::<Test>::get(new), Some(slot));
        assert_eq!(SubnetIdFriendlyUid::<Test>::get(new), Some(friendly));
        let counts = (
            TotalNodes::<Test>::get(),
            TotalAdmittedNodes::<Test>::get(),
            TotalElectableNodes::<Test>::get(),
        );
        assert_ok!(Network::remove_subnet_node(
            RuntimeOrigin::signed(coldkey),
            old,
            node
        ));
        assert_ok!(Network::do_collective_remove_subnet_node(old, node));
        drain_removed_subnets();
        assert_eq!(
            counts,
            (
                TotalNodes::<Test>::get(),
                TotalAdmittedNodes::<Test>::get(),
                TotalElectableNodes::<Test>::get()
            )
        );
        assert_eq!(FriendlyUidSubnetId::<Test>::get(friendly), Some(new));
        assert_eq!(SlotAssignment::<Test>::get(slot), Some(new));
        assert!(SubnetNodesData::<Test>::contains_key(new, node));
        assert!(SubnetsData::<Test>::contains_key(new));
    });
}

#[test]
fn removed_records_cannot_authorize_live_node_operations() {
    new_test_ext().execute_with(|| {
        let (subnet, node, coldkey) = fixture(SubnetNodeClass::Active, true);
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
        Network::do_remove_subnet(subnet, SubnetRemovalReason::Owner);
        assert!(SubnetNodesData::<Test>::contains_key(subnet, node));
        assert_noop!(
            Network::update_node_peer_info(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet,
                node,
                None
            ),
            Error::<Test>::InvalidSubnetId
        );
        assert_noop!(
            Network::update_node_hotkey(RuntimeOrigin::signed(coldkey.clone()), subnet, node, None),
            Error::<Test>::InvalidSubnetId
        );
        assert_noop!(
            Network::add_node_stake(RuntimeOrigin::signed(coldkey), subnet, node, 1),
            Error::<Test>::InvalidSubnetId
        );
        assert_noop!(
            Network::attest(RuntimeOrigin::signed(hotkey.clone()), subnet, node, None),
            Error::<Test>::InvalidSubnetId
        );
        assert!(Network::propose_attestation(
            RuntimeOrigin::signed(hotkey),
            subnet,
            vec![],
            None,
            None,
            None,
            None
        )
        .is_err());
        frame_support::assert_storage_noop!(Network::elect_validator(subnet, 1_000_000));
        frame_support::assert_storage_noop!(Network::emission_settlement_step(
            &mut WeightMeter::new(),
            1_000_000,
            subnet
        ));
        assert!(Network::get_subnet_node_info(subnet, node).is_none());
        assert!(Network::effective_consensus_validator_ids(subnet).is_empty());
    });
}

#[test]
fn fixed_removal_reservation_is_atomic_in_both_weight_dimensions() {
    for missing in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
        new_test_ext().execute_with(|| {
            let (subnet, _, _) = fixture(SubnetNodeClass::Active, false);
            let required = <() as WeightInfo>::do_remove_subnet();
            let limit = required.saturating_sub(missing);
            let mut meter = WeightMeter::with_limit(limit);
            frame_support::assert_storage_noop!(assert_eq!(
                Network::try_do_remove_subnet(&mut meter, subnet, SubnetRemovalReason::Owner),
                SubnetRemovalOutcome::Deferred
            ));
            assert!(meter.consumed().all_lte(limit));
            let mut exact = WeightMeter::with_limit(required);
            assert_eq!(
                Network::try_do_remove_subnet(&mut exact, subnet, SubnetRemovalReason::Owner),
                SubnetRemovalOutcome::Removed
            );
            assert_eq!(exact.consumed(), required);
        });
    }
}

#[test]
fn node_prefix_cleanup_is_bounded_and_rotates_without_decoding_metadata() {
    use crate::utilities::subnet_cleanup::MAX_REMOVED_SUBNET_STEPS_PER_IDLE;
    for committed in [false, true] {
        let mut ext = new_test_ext();
        let count = MAX_REMOVED_SUBNET_STEPS_PER_IDLE + 10;
        ext.execute_with(|| {
            for subnet in [41, 42] {
                RemovedSubnetCleanupQueue::<Test>::insert(
                    subnet,
                    RemovedSubnetCleanupStage::NodeMetadata,
                );
            }
            for node in 0..count {
                // Deliberately undecodable values prove prefix cleanup only traverses keys.
                sp_io::storage::set(
                    &SubnetNodeMetadata::<Test>::hashed_key_for(41, node),
                    &[0xff; 1000],
                );
            }
            for subnet in [42, 43] {
                sp_io::storage::set(
                    &SubnetNodeMetadata::<Test>::hashed_key_for(subnet, 1),
                    &[0xff; 1000],
                );
            }
        });
        if committed {
            ext.commit_all().unwrap();
        }
        ext.execute_with(|| {
            Network::cleanup_removed_subnets(&mut WeightMeter::new());
            assert_eq!(SubnetNodeMetadata::<Test>::iter_key_prefix(41).count(), 10);
            assert_eq!(
                RemovedSubnetCleanupQueue::<Test>::get(41),
                Some(RemovedSubnetCleanupStage::NodeMetadata)
            );
            Network::cleanup_removed_subnets(&mut WeightMeter::new());
            assert!(!SubnetNodeMetadata::<Test>::contains_key(42, 1));
            assert!(!RemovedSubnetCleanupQueue::<Test>::contains_key(42));
            // Reenter in the same storage overlay; earlier deletions must not stall traversal.
            Network::cleanup_removed_subnets(&mut WeightMeter::new());
            assert_eq!(SubnetNodeMetadata::<Test>::iter_key_prefix(41).count(), 0);
            assert!(!RemovedSubnetCleanupQueue::<Test>::contains_key(41));
            assert!(SubnetNodeMetadata::<Test>::contains_key(43, 1));
        });
    }
}

#[test]
fn cleanup_reserves_singleton_and_node_deletion_plus_progress_in_both_dimensions() {
    for (stage, key) in [
        (
            RemovedSubnetCleanupStage::SubnetBootnodes,
            SubnetBootnodes::<Test>::hashed_key_for(41),
        ),
        (
            RemovedSubnetCleanupStage::NodeMetadata,
            SubnetNodeMetadata::<Test>::hashed_key_for(41, 1),
        ),
    ] {
        new_test_ext().execute_with(|| {
            RemovedSubnetCleanupQueue::<Test>::insert(41, stage);
            sp_io::storage::set(&key, &[0xff; 1000]);
            let exact = <() as WeightInfo>::removed_subnet_cleanup_selector()
                .saturating_add(Network::removed_subnet_cleanup_step_weight(stage))
                .saturating_add(<() as WeightInfo>::removed_subnet_cleanup_progress());
            for missing in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
                let limit = exact.saturating_sub(missing);
                let mut meter = WeightMeter::with_limit(limit);
                Network::cleanup_removed_subnets(&mut meter);
                assert!(meter.consumed().all_lte(limit));
                assert!(sp_io::storage::exists(&key));
                assert_eq!(RemovedSubnetCleanupQueue::<Test>::get(41), Some(stage));
            }
            let mut meter = WeightMeter::with_limit(exact);
            Network::cleanup_removed_subnets(&mut meter);
            assert!(meter.consumed().all_lte(exact));
            assert!(!sp_io::storage::exists(&key));
        });
    }
}

#[test]
fn expired_slash_reservation_does_not_require_idle_before_full_withdrawal() {
    new_test_ext().execute_with(|| {
        let (subnet, node, coldkey) = fixture(SubnetNodeClass::Queued, true);
        let stake = NodeSubnetStake::<Test>::get(node, subnet);
        assert_ok!(Network::enqueue_consensus_operation(
            subnet,
            ConsensusOperation::NodeSlash {
                subnet_id: subnet,
                subnet_node_id: node,
                amount: 200,
                attestation_percentage: 0,
            }
        ));
        let expiry = PendingConsensusOperations::<Test>::iter_values()
            .next()
            .unwrap()
            .expires_at;
        Network::do_remove_subnet(subnet, SubnetRemovalReason::Owner);
        System::set_block_number(expiry);
        assert_ok!(Network::remove_node_stake(
            RuntimeOrigin::signed(coldkey),
            subnet,
            node,
            stake
        ));
        assert_eq!(NodeSubnetStake::<Test>::get(node, subnet), 0);
        assert!(PendingConsensusOperations::<Test>::iter_keys()
            .next()
            .is_some());
        assert!(RemovedSubnetCleanupQueue::<Test>::contains_key(subnet));
    });
}
