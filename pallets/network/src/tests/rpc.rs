use super::mock::*;
use crate::tests::test_utils::*;
use crate::{
    ConsensusMechanism, ConsensusValidatorNodeCountDecay, ConsensusValidatorStakeWeightPower,
    EmergencySubnetNodeElectionData, EmergencySubnetValidatorData, IdleClassificationEpochs,
    IncludedClassificationEpochs, LastConsensusValidatorNodeCountDecayUpdate,
    LastConsensusValidatorStakeWeightPowerUpdate, LastSubnetDelegateStakeRewardsUpdate,
    MaxSubnetNodes, MaxSubnets, MinSubnetMinStake, MinSubnetNodeReputation,
    OverwatchNodeValidatorId, PeerInfo, PendingConsensusValidatorNodeCountDecay,
    PendingConsensusValidatorStakeWeightPower, PendingIdleClassificationEpochs,
    PendingIncludedClassificationEpochs, PendingMinSubnetNodeReputation, PendingOwnerU128Update,
    PendingOwnerU32Update, PendingQueueImmunityEpochs, PendingSubnetDelegateStakeRewardsPercentage,
    PendingSubnetDelegateStakeRewardsPercentageUpdate,
    PendingSubnetNodeMinWeightDecreaseReputationThreshold, PendingSubnetNodeQueueEpochs,
    QueueImmunityEpochs, RegisteredSubnetNodesData, SlotAssignment,
    SubnetDelegateStakeRewardsPercentage, SubnetElectedValidator, SubnetName,
    SubnetNodeElectionSlots, SubnetNodeMinWeightDecreaseReputationThreshold, SubnetNodeQueueEpochs,
    SubnetNodeValidatorId, SubnetNodesData, SubnetOwner, SubnetPauseData, SubnetSlot, SubnetState,
    SubnetsData, TotalActiveSubnets, ValidatorNodeDelegateStakeWeights,
};
use frame_support::assert_ok;
use network_rpc_types::{NetworkQueryError, PageRequest};
use sp_std::collections::btree_map::BTreeMap;

//
// RPC Getter Function Tests
//

#[test]
fn test_get_validator_subnet_nodes_info() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let rpc_results = Network::get_validator_subnet_nodes_info(1);

        assert!(rpc_results.len() > 0);
    })
}

#[test]
fn test_get_subnet_info() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "test-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let owner = SubnetOwner::<Test>::get(subnet_id).unwrap();
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let effective_subnet_epoch = current_subnet_epoch.saturating_add(1);

        let queue_immunity_epochs = 11;
        let pending_queue_immunity_epochs = 12;
        let subnet_node_queue_epochs = 13;
        let pending_subnet_node_queue_epochs = 14;
        let idle_classification_epochs = 21;
        let pending_idle_classification_epochs = 22;
        let included_classification_epochs = 31;
        let pending_included_classification_epochs = 32;
        let delegate_stake_percentage = 41;
        let pending_delegate_stake_percentage = 42;
        let consensus_validator_node_count_decay = 51;
        let pending_consensus_validator_node_count_decay = 52;
        let consensus_validator_stake_weight_power = 61;
        let pending_consensus_validator_stake_weight_power = 62;
        let min_weight_decrease_reputation_threshold = 71;
        let pending_min_weight_decrease_reputation_threshold = 72;
        let min_subnet_node_reputation = 81;
        let pending_min_subnet_node_reputation = 82;
        let last_delegate_stake_rewards_update = 101;
        let last_consensus_validator_node_count_decay_update = 102;
        let last_consensus_validator_stake_weight_power_update = 103;

        QueueImmunityEpochs::<Test>::insert(subnet_id, queue_immunity_epochs);
        SubnetNodeQueueEpochs::<Test>::insert(subnet_id, subnet_node_queue_epochs);
        IdleClassificationEpochs::<Test>::insert(subnet_id, idle_classification_epochs);
        IncludedClassificationEpochs::<Test>::insert(subnet_id, included_classification_epochs);
        SubnetDelegateStakeRewardsPercentage::<Test>::insert(subnet_id, delegate_stake_percentage);
        ConsensusValidatorNodeCountDecay::<Test>::insert(
            subnet_id,
            consensus_validator_node_count_decay,
        );
        ConsensusValidatorStakeWeightPower::<Test>::insert(
            subnet_id,
            consensus_validator_stake_weight_power,
        );
        SubnetNodeMinWeightDecreaseReputationThreshold::<Test>::insert(
            subnet_id,
            min_weight_decrease_reputation_threshold,
        );
        MinSubnetNodeReputation::<Test>::insert(subnet_id, min_subnet_node_reputation);
        LastSubnetDelegateStakeRewardsUpdate::<Test>::insert(
            subnet_id,
            last_delegate_stake_rewards_update,
        );
        LastConsensusValidatorNodeCountDecayUpdate::<Test>::insert(
            subnet_id,
            last_consensus_validator_node_count_decay_update,
        );
        LastConsensusValidatorStakeWeightPowerUpdate::<Test>::insert(
            subnet_id,
            last_consensus_validator_stake_weight_power_update,
        );

        let pending_queue = PendingOwnerU32Update::<Test> {
            value: pending_queue_immunity_epochs,
            effective_subnet_epoch,
            owner: owner.clone(),
        };
        let pending_idle = PendingOwnerU32Update::<Test> {
            value: pending_idle_classification_epochs,
            effective_subnet_epoch,
            owner: owner.clone(),
        };
        let pending_subnet_node_queue = PendingOwnerU32Update::<Test> {
            value: pending_subnet_node_queue_epochs,
            effective_subnet_epoch,
            owner: owner.clone(),
        };
        let pending_included = PendingOwnerU32Update::<Test> {
            value: pending_included_classification_epochs,
            effective_subnet_epoch,
            owner: owner.clone(),
        };
        let pending_delegate = PendingSubnetDelegateStakeRewardsPercentageUpdate::<Test> {
            value: pending_delegate_stake_percentage,
            effective_subnet_epoch,
            owner: owner.clone(),
        };
        let pending_node_count_decay = PendingOwnerU128Update::<Test> {
            value: pending_consensus_validator_node_count_decay,
            effective_subnet_epoch,
            owner: owner.clone(),
        };
        let pending_stake_weight_power = PendingOwnerU128Update::<Test> {
            value: pending_consensus_validator_stake_weight_power,
            effective_subnet_epoch,
            owner: owner.clone(),
        };
        let pending_min_weight_threshold = PendingOwnerU128Update::<Test> {
            value: pending_min_weight_decrease_reputation_threshold,
            effective_subnet_epoch,
            owner: owner.clone(),
        };
        let pending_min_reputation = PendingOwnerU128Update::<Test> {
            value: pending_min_subnet_node_reputation,
            effective_subnet_epoch,
            owner: owner.clone(),
        };

        PendingQueueImmunityEpochs::<Test>::insert(subnet_id, pending_queue.clone());
        PendingSubnetNodeQueueEpochs::<Test>::insert(subnet_id, pending_subnet_node_queue.clone());
        PendingIdleClassificationEpochs::<Test>::insert(subnet_id, pending_idle.clone());
        PendingIncludedClassificationEpochs::<Test>::insert(subnet_id, pending_included.clone());
        PendingSubnetDelegateStakeRewardsPercentage::<Test>::insert(
            subnet_id,
            pending_delegate.clone(),
        );
        PendingConsensusValidatorNodeCountDecay::<Test>::insert(
            subnet_id,
            pending_node_count_decay.clone(),
        );
        PendingConsensusValidatorStakeWeightPower::<Test>::insert(
            subnet_id,
            pending_stake_weight_power.clone(),
        );
        PendingSubnetNodeMinWeightDecreaseReputationThreshold::<Test>::insert(
            subnet_id,
            pending_min_weight_threshold.clone(),
        );
        PendingMinSubnetNodeReputation::<Test>::insert(subnet_id, pending_min_reputation.clone());

        let subnet_info = Network::get_subnet_info(subnet_id);

        assert!(subnet_info.is_some(), "Subnet info should exist");
        let info = subnet_info.unwrap();
        assert_eq!(info.id, subnet_id);
        assert_eq!(info.name, subnet_name);
        assert_eq!(info.consensus_mechanism, ConsensusMechanism::Attestation);
        assert_eq!(
            info.consensus_eligible_from_subnet_epoch,
            SubnetsData::<Test>::get(subnet_id)
                .unwrap()
                .consensus_eligible_from_subnet_epoch
        );
        assert!(info.pause_started_global_epoch.is_none());
        assert!(info.pause_started_subnet_epoch.is_none());
        assert_eq!(info.queue_immunity_epochs, queue_immunity_epochs);
        assert_eq!(info.pending_queue_immunity_epochs, Some(pending_queue));
        assert_eq!(info.subnet_node_queue_epochs, subnet_node_queue_epochs);
        assert_eq!(
            info.pending_subnet_node_queue_epochs,
            Some(pending_subnet_node_queue)
        );
        assert_eq!(info.idle_classification_epochs, idle_classification_epochs);
        assert_eq!(info.pending_idle_classification_epochs, Some(pending_idle));
        assert_eq!(
            info.included_classification_epochs,
            included_classification_epochs
        );
        assert_eq!(
            info.pending_included_classification_epochs,
            Some(pending_included)
        );
        assert_eq!(info.delegate_stake_percentage, delegate_stake_percentage);
        assert_eq!(
            info.pending_delegate_stake_percentage,
            Some(pending_delegate)
        );
        assert_eq!(
            info.last_delegate_stake_rewards_update,
            last_delegate_stake_rewards_update
        );
        assert_eq!(
            info.consensus_validator_node_count_decay,
            consensus_validator_node_count_decay
        );
        assert_eq!(
            info.pending_consensus_validator_node_count_decay,
            Some(pending_node_count_decay)
        );
        assert_eq!(
            info.last_consensus_validator_node_count_decay_update,
            Some(last_consensus_validator_node_count_decay_update)
        );
        assert_eq!(
            info.consensus_validator_stake_weight_power,
            consensus_validator_stake_weight_power
        );
        assert_eq!(
            info.pending_consensus_validator_stake_weight_power,
            Some(pending_stake_weight_power)
        );
        assert_eq!(
            info.last_consensus_validator_stake_weight_power_update,
            Some(last_consensus_validator_stake_weight_power_update)
        );
        assert_eq!(
            info.subnet_node_min_weight_decrease_reputation_threshold,
            min_weight_decrease_reputation_threshold
        );
        assert_eq!(
            info.pending_subnet_node_min_weight_decrease_reputation_threshold,
            Some(pending_min_weight_threshold)
        );
        assert_eq!(info.min_subnet_node_reputation, min_subnet_node_reputation);
        assert_eq!(
            info.pending_min_subnet_node_reputation,
            Some(pending_min_reputation)
        );

        PendingQueueImmunityEpochs::<Test>::mutate(subnet_id, |pending| {
            pending.as_mut().unwrap().effective_subnet_epoch = current_subnet_epoch;
        });
        PendingSubnetNodeQueueEpochs::<Test>::mutate(subnet_id, |pending| {
            pending.as_mut().unwrap().effective_subnet_epoch = current_subnet_epoch;
        });
        PendingIdleClassificationEpochs::<Test>::mutate(subnet_id, |pending| {
            pending.as_mut().unwrap().effective_subnet_epoch = current_subnet_epoch;
        });
        PendingIncludedClassificationEpochs::<Test>::mutate(subnet_id, |pending| {
            pending.as_mut().unwrap().effective_subnet_epoch = current_subnet_epoch;
        });
        PendingSubnetDelegateStakeRewardsPercentage::<Test>::mutate(subnet_id, |pending| {
            pending.as_mut().unwrap().effective_subnet_epoch = current_subnet_epoch;
        });
        PendingConsensusValidatorNodeCountDecay::<Test>::mutate(subnet_id, |pending| {
            pending.as_mut().unwrap().effective_subnet_epoch = current_subnet_epoch;
        });
        PendingConsensusValidatorStakeWeightPower::<Test>::mutate(subnet_id, |pending| {
            pending.as_mut().unwrap().effective_subnet_epoch = current_subnet_epoch;
        });
        PendingSubnetNodeMinWeightDecreaseReputationThreshold::<Test>::mutate(
            subnet_id,
            |pending| {
                pending.as_mut().unwrap().effective_subnet_epoch = current_subnet_epoch;
            },
        );
        PendingMinSubnetNodeReputation::<Test>::mutate(subnet_id, |pending| {
            pending.as_mut().unwrap().effective_subnet_epoch = current_subnet_epoch;
        });

        let info = Network::get_subnet_info(subnet_id).unwrap();
        assert_eq!(info.queue_immunity_epochs, pending_queue_immunity_epochs);
        assert!(info.pending_queue_immunity_epochs.is_none());
        assert_eq!(
            info.subnet_node_queue_epochs,
            pending_subnet_node_queue_epochs
        );
        assert!(info.pending_subnet_node_queue_epochs.is_none());
        assert_eq!(
            info.idle_classification_epochs,
            pending_idle_classification_epochs
        );
        assert!(info.pending_idle_classification_epochs.is_none());
        assert_eq!(
            info.included_classification_epochs,
            pending_included_classification_epochs
        );
        assert!(info.pending_included_classification_epochs.is_none());
        assert_eq!(
            info.delegate_stake_percentage,
            pending_delegate_stake_percentage
        );
        assert!(info.pending_delegate_stake_percentage.is_none());
        assert_eq!(
            info.consensus_validator_node_count_decay,
            pending_consensus_validator_node_count_decay
        );
        assert!(info.pending_consensus_validator_node_count_decay.is_none());
        assert_eq!(
            info.consensus_validator_stake_weight_power,
            pending_consensus_validator_stake_weight_power
        );
        assert!(info
            .pending_consensus_validator_stake_weight_power
            .is_none());
        assert_eq!(
            info.subnet_node_min_weight_decrease_reputation_threshold,
            pending_min_weight_decrease_reputation_threshold
        );
        assert!(info
            .pending_subnet_node_min_weight_decrease_reputation_threshold
            .is_none());
        assert_eq!(
            info.min_subnet_node_reputation,
            pending_min_subnet_node_reputation
        );
        assert!(info.pending_min_subnet_node_reputation.is_none());
        SubnetsData::<Test>::mutate(subnet_id, |subnet| {
            let subnet = subnet.as_mut().unwrap();
            subnet.state = SubnetState::Paused;
            subnet.consensus_eligible_from_subnet_epoch = None;
            subnet.pause = Some(SubnetPauseData {
                started_global_epoch: 111,
                started_subnet_epoch: 109,
            });
        });
        let paused_info = Network::get_subnet_info(subnet_id).unwrap();
        assert!(paused_info.consensus_eligible_from_subnet_epoch.is_none());
        assert_eq!(paused_info.pause_started_global_epoch, Some(111));
        assert_eq!(paused_info.pause_started_subnet_epoch, Some(109));

        SubnetsData::<Test>::mutate(subnet_id, |subnet| {
            let subnet = subnet.as_mut().unwrap();
            subnet.state = SubnetState::Registered;
            subnet.consensus_eligible_from_subnet_epoch = None;
            subnet.pause = None;
        });
        let registered_info = Network::get_subnet_info(subnet_id).unwrap();
        assert!(registered_info
            .consensus_eligible_from_subnet_epoch
            .is_none());
        assert!(registered_info.pause_started_global_epoch.is_none());
        assert!(registered_info.pause_started_subnet_epoch.is_none());

        assert!(Network::get_subnet_info(u32::MAX).is_none());
    })
}

#[test]
fn subnet_info_reports_subnet_slot_across_collision_removal_and_reuse() {
    new_test_ext().execute_with(|| {
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        for name in ["slot-rpc-1", "slot-rpc-2", "slot-rpc-3"] {
            build_activated_subnet(name.as_bytes().to_vec(), 0, 0, deposit_amount, stake_amount);
        }

        let subnet_ids = ["slot-rpc-1", "slot-rpc-2", "slot-rpc-3"]
            .map(|name| SubnetName::<Test>::get(name.as_bytes().to_vec()).unwrap());

        for subnet_id in subnet_ids {
            let slot = SubnetSlot::<Test>::get(subnet_id).unwrap();
            assert_eq!(
                Network::get_subnet_info(subnet_id).unwrap().slot_index,
                Some(slot)
            );
            assert_eq!(SlotAssignment::<Test>::get(slot), Some(subnet_id));
        }

        // Seed the exact key collision that made the old RPC field silently report an unrelated
        // subnet: a subnet ID is present as a key in the reverse slot map. The subnet-oriented
        // response must remain sourced exclusively from `SubnetSlot`.
        let collision_subnet_id = subnet_ids[2];
        let unrelated_subnet_id = subnet_ids[0];
        SlotAssignment::<Test>::insert(collision_subnet_id, unrelated_subnet_id);
        assert_eq!(
            Network::get_subnet_info(collision_subnet_id)
                .unwrap()
                .slot_index,
            SubnetSlot::<Test>::get(collision_subnet_id),
        );
        SlotAssignment::<Test>::remove(collision_subnet_id);

        let removed_subnet_id = subnet_ids[0];
        let freed_slot = SubnetSlot::<Test>::get(removed_subnet_id).unwrap();
        let owner = SubnetOwner::<Test>::get(removed_subnet_id).unwrap();
        assert_ok!(Network::owner_deactivate_subnet(
            RuntimeOrigin::signed(owner),
            removed_subnet_id,
        ));

        assert!(Network::get_subnet_info(removed_subnet_id).is_none());
        assert_eq!(SubnetSlot::<Test>::get(removed_subnet_id), None);
        assert_eq!(SlotAssignment::<Test>::get(freed_slot), None);

        build_activated_subnet(
            b"slot-rpc-replacement".to_vec(),
            0,
            0,
            deposit_amount,
            stake_amount,
        );
        let replacement_subnet_id =
            SubnetName::<Test>::get(b"slot-rpc-replacement".to_vec()).unwrap();

        assert_eq!(
            SubnetSlot::<Test>::get(replacement_subnet_id),
            Some(freed_slot)
        );
        assert_eq!(
            SlotAssignment::<Test>::get(freed_slot),
            Some(replacement_subnet_id)
        );
        assert_eq!(
            Network::get_subnet_info(replacement_subnet_id)
                .unwrap()
                .slot_index,
            Some(freed_slot),
        );
    })
}

#[test]
fn test_get_subnet_node_info() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "test-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let node_info = Network::get_subnet_node_info(subnet_id, 1);

        assert!(node_info.is_some(), "Node info should exist");
        let info = node_info.unwrap();
        assert_eq!(info.subnet_id, subnet_id);
        assert_eq!(info.subnet_node_id, 1);
    })
}

#[test]
fn test_get_elected_validator_info_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "test-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 12, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let epoch_length = EpochLength::get();
        let block_number = System::block_number();
        let epoch = block_number / epoch_length;

        set_block_to_subnet_slot_epoch(epoch, subnet_id);
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // Elect a validator
        Network::elect_validator(subnet_id, subnet_epoch, block_number);

        let validator_info = Network::get_elected_validator_info(subnet_id, subnet_epoch);

        assert!(
            validator_info.is_some(),
            "Elected validator info should exist"
        );
    })
}

#[test]
fn test_get_validators_and_attestors() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "test-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 12, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let validators = Network::get_validators_and_attestors(subnet_id);

        assert!(validators.len() == 12, "Should have validators/attestors");
    })
}

#[test]
fn test_get_overwatch_nodes_page() {
    new_test_ext().execute_with(|| {
        let page = Network::rpc_get_overwatch_nodes(PageRequest::default()).unwrap();
        assert!(page.items.is_empty());
        assert!(page.next_cursor.is_none());
    })
}

#[test]
fn typed_subnet_nodes_and_bootnodes_exclude_registered_queue_entries() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "test-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let active_node_count = 4;

        build_activated_subnet(
            subnet_name.clone(),
            0,
            active_node_count,
            deposit_amount,
            stake_amount,
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let queued_node_id = 999;
        let queued_bootnode_peer_id = peer(99_999);
        let mut queued_node = SubnetNodesData::<Test>::get(subnet_id, 1);
        queued_node.id = queued_node_id;
        queued_node.bootnode_peer_info = Some(PeerInfo::<Test> {
            peer_id: queued_bootnode_peer_id.clone(),
            multiaddr: get_multiaddr(Some(subnet_id), Some(queued_node_id), Some(1)),
        });
        RegisteredSubnetNodesData::<Test>::insert(subnet_id, queued_node_id, queued_node);

        let nodes = Network::rpc_get_subnet_nodes(subnet_id, PageRequest::default()).unwrap();
        assert_eq!(nodes.items.len(), active_node_count as usize);
        assert!(nodes.next_cursor.is_none());
        assert!(nodes
            .items
            .iter()
            .all(|node| node.subnet_node_id != queued_node_id));

        let bootnodes = Network::rpc_get_bootnodes(subnet_id).unwrap();
        let active_bootnode_peer_id = SubnetNodesData::<Test>::get(subnet_id, 1)
            .bootnode_peer_info
            .unwrap()
            .peer_id;

        assert_eq!(bootnodes.official.len(), 1);
        assert_eq!(bootnodes.official[0].peer_id.0, peer(0).0);
        assert_eq!(bootnodes.active_nodes.len(), active_node_count as usize);
        assert!(bootnodes
            .active_nodes
            .iter()
            .any(|node| node.peer_id.0 == active_bootnode_peer_id.0));
        assert!(bootnodes
            .active_nodes
            .iter()
            .all(|node| node.peer_id.0 != queued_bootnode_peer_id.0));
        assert!(Network::rpc_get_bootnodes(u32::MAX).is_none());
    })
}

#[test]
fn test_get_validator_stakes() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "test-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let stakes = Network::get_validator_stakes(1);

        assert!(stakes.len() > 0, "Coldkey should have stakes");
    })
}

#[test]
fn subnet_node_rpc_lookup_is_panic_free_for_missing_validator_links() {
    new_test_ext().execute_with(|| {
        let subnet_name = b"rpc-safe-node".to_vec();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();

        assert!(Network::get_subnet_node_info(subnet_id, 1).is_some());
        SubnetNodeValidatorId::<Test>::remove(subnet_id, 1);
        assert!(Network::get_subnet_node_info(subnet_id, 1).is_none());
        assert!(Network::rpc_get_subnet_node_info(subnet_id, 1).is_none());
    });
}

#[test]
fn effective_validator_rpc_ignores_pending_emergency_set() {
    new_test_ext().execute_with(|| {
        let subnet_name = b"rpc-validator-set".to_vec();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            12,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        assert_eq!(SubnetNodeElectionSlots::<Test>::get(subnet_id).len(), 12);

        EmergencySubnetNodeElectionData::<Test>::insert(
            subnet_id,
            EmergencySubnetValidatorData {
                subnet_node_ids: vec![1, 2, 3],
                target_emergency_validators_epochs: 10,
                max_emergency_validators_epoch: u32::MAX,
                activated: false,
                ..Default::default()
            },
        );
        assert_eq!(Network::get_validators_and_attestors(subnet_id).len(), 12);
        let pending_page = Network::rpc_get_subnet_validator_nodes(
            subnet_id,
            PageRequest {
                cursor: None,
                limit: 100,
            },
        )
        .unwrap();
        assert_eq!(pending_page.items.len(), 12);
        let pending_status = Network::rpc_get_subnet_epoch_status(subnet_id).unwrap();
        assert_eq!(
            pending_status.validator_set_source,
            network_rpc_types::ConsensusElectionSource::Regular
        );
        assert!(pending_status.pending_emergency_set);

        EmergencySubnetNodeElectionData::<Test>::mutate(subnet_id, |data| {
            data.as_mut().unwrap().activated = true;
        });
        let active = Network::get_validators_and_attestors(subnet_id);
        assert_eq!(active.len(), 3);
        assert_eq!(
            active
                .into_iter()
                .map(|node| node.subnet_node_id)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        let active_page = Network::rpc_get_subnet_validator_nodes(
            subnet_id,
            PageRequest {
                cursor: None,
                limit: 100,
            },
        )
        .unwrap();
        assert_eq!(active_page.items.len(), 3);
        let active_status = Network::rpc_get_subnet_epoch_status(subnet_id).unwrap();
        assert_eq!(
            active_status.validator_set_source,
            network_rpc_types::ConsensusElectionSource::Emergency
        );
        assert!(!active_status.pending_emergency_set);

        EmergencySubnetNodeElectionData::<Test>::mutate(subnet_id, |data| {
            let data = data.as_mut().unwrap();
            data.total_epochs = data.target_emergency_validators_epochs;
        });
        let expired_page = Network::rpc_get_subnet_validator_nodes(
            subnet_id,
            PageRequest {
                cursor: None,
                limit: 100,
            },
        )
        .unwrap();
        assert_eq!(expired_page.items.len(), 12);
        assert!(EmergencySubnetNodeElectionData::<Test>::contains_key(
            subnet_id
        ));

        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        SubnetElectedValidator::<Test>::remove(subnet_id, subnet_epoch);
        Network::elect_validator(subnet_id, subnet_epoch, System::block_number());
        let round = SubnetElectedValidator::<Test>::get(subnet_id, subnet_epoch).unwrap();
        assert!(round.emergency.is_none());
        assert!(!EmergencySubnetNodeElectionData::<Test>::contains_key(
            subnet_id
        ));
    });
}

#[test]
fn consensus_round_rpc_uses_immutable_election_identity() {
    new_test_ext().execute_with(|| {
        let subnet_name = b"rpc-consensus-round".to_vec();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            12,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let block = System::block_number();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        Network::elect_validator(subnet_id, subnet_epoch, block);
        let stored = SubnetElectedValidator::<Test>::get(subnet_id, subnet_epoch).unwrap();

        SubnetNodeValidatorId::<Test>::remove(subnet_id, stored.validator_subnet_node_id);
        let rpc = Network::rpc_get_consensus_round(subnet_id, subnet_epoch)
            .unwrap()
            .unwrap();
        assert_eq!(rpc.elected_subnet_node_id, stored.validator_subnet_node_id);
        assert_eq!(rpc.elected_validator_id, stored.validator_id);
        assert_eq!(
            rpc.election_candidates.len(),
            stored.eligible_subnet_node_ids.len()
        );
        assert!(rpc.election_candidates.iter().any(|candidate| {
            candidate.subnet_node_id == stored.validator_subnet_node_id
                && candidate.validator_id == stored.validator_id
        }));
        assert_eq!(
            rpc.validator_delegate_balance_at_election.0,
            stored.validator_delegate_stake_balance
        );
    });
}

#[test]
fn validator_info_and_node_allocations_are_exposed() {
    new_test_ext().execute_with(|| {
        let subnet_name = b"rpc-validator-info".to_vec();
        build_activated_subnet(
            subnet_name,
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let validator_id = 1;
        let validator = Network::rpc_get_validator_info(validator_id).unwrap();
        assert_eq!(validator.id, validator_id);
        assert_eq!(
            Network::rpc_get_validator_by_coldkey(&validator.coldkey)
                .unwrap()
                .id,
            validator_id
        );
        assert_eq!(
            Network::rpc_get_validator_by_hotkey(&validator.hotkey)
                .unwrap()
                .id,
            validator_id
        );

        ValidatorNodeDelegateStakeWeights::<Test>::insert(
            validator_id,
            BTreeMap::from([((1, 1), 40), ((1, 2), 30), ((2, 1), 30)]),
        );
        let first_allocations = Network::rpc_get_validator_node_allocations(
            validator_id,
            PageRequest {
                cursor: None,
                limit: 2,
            },
        )
        .unwrap();
        assert_eq!(first_allocations.items.len(), 2);
        let second_allocations = Network::rpc_get_validator_node_allocations(
            validator_id,
            PageRequest {
                cursor: first_allocations.next_cursor,
                limit: 2,
            },
        )
        .unwrap();
        assert_eq!(second_allocations.items.len(), 1);
        assert!(second_allocations.next_cursor.is_none());
    });
}

#[test]
fn subnet_pages_are_bounded_and_resume_after_cursor() {
    new_test_ext().execute_with(|| {
        for index in 0..3 {
            build_activated_subnet(
                format!("rpc-page-{index}").into_bytes(),
                0,
                4,
                10_000_000_000_000_000_000_000,
                MinSubnetMinStake::<Test>::get(),
            );
        }

        let first = Network::rpc_get_subnets(PageRequest {
            cursor: None,
            limit: 2,
        })
        .unwrap();
        assert_eq!(first.items.len(), 2);
        let cursor = first.next_cursor.expect("another subnet remains");
        let second = Network::rpc_get_subnets(PageRequest {
            cursor: Some(cursor),
            limit: 2,
        })
        .unwrap();
        assert_eq!(second.items.len(), 1);
        assert!(second.next_cursor.is_none());
        assert!(first.items[1].id < second.items[0].id);

        assert!(matches!(
            Network::rpc_get_subnets(PageRequest {
                cursor: None,
                limit: 0,
            }),
            Err(NetworkQueryError::InvalidPageLimit { .. })
        ));
    });
}

#[test]
fn stale_overwatch_validator_mapping_is_not_rpc_membership() {
    new_test_ext().execute_with(|| {
        OverwatchNodeValidatorId::<Test>::insert(77, 1);
        assert!(Network::rpc_get_overwatch_node_info(77).is_none());
    });
}

#[test]
fn effective_overwatch_rpc_views_preserve_zero_and_default_semantics() {
    new_test_ext().execute_with(|| {
        let subnet_id = 7;
        let default_weight = test_percent(1, 10);
        crate::DefaultOverwatchSubnetWeight::<Test>::set(default_weight);
        crate::OverwatchWeightFactor::<Test>::set(test_percent(1, 2));
        let mut raw_weights = frame_support::BoundedBTreeMap::new();
        raw_weights.try_insert(subnet_id, 0).unwrap();
        crate::LatestEffectiveOverwatchSignal::<Test>::put(
            crate::EffectiveOverwatchSignal::<Test> {
                source_epoch: 12,
                valid: true,
                subnet_weights: raw_weights,
            },
        );
        crate::LatestOverwatchSignalRevision::<Test>::put(4);

        let meta = Network::rpc_get_effective_overwatch_signal_meta();
        assert!(meta.exists);
        assert!(meta.valid);
        assert_eq!(meta.source_epoch, 12);
        assert_eq!(meta.revision, 4);

        let explicit_zero = Network::rpc_get_effective_overwatch_subnet_weight(subnet_id);
        assert!(explicit_zero.raw_weight_exists);
        assert_eq!(explicit_zero.raw_weight.0, 0);
        assert_eq!(explicit_zero.resolved_weight.0, 0);

        let missing = Network::rpc_get_effective_overwatch_subnet_weight(subnet_id + 1);
        assert!(!missing.raw_weight_exists);
        assert_eq!(missing.raw_weight.0, 0);
        assert_eq!(missing.resolved_weight.0, default_weight);

        crate::LatestEffectiveOverwatchSignal::<Test>::mutate(|signal| {
            signal.as_mut().unwrap().valid = false;
        });
        let invalid = Network::rpc_get_effective_overwatch_subnet_weight(subnet_id);
        assert!(invalid.raw_weight_exists);
        assert_eq!(invalid.raw_weight.0, 0);
        assert_eq!(invalid.resolved_weight.0, default_weight);
    });
}
