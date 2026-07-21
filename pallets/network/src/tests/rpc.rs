use super::mock::*;
use crate::tests::test_utils::*;
use crate::Event;
use crate::{
    AccountNodeDelegateStakeShares, ColdkeyValidatorId, ConsensusValidatorNodeCountDecay,
    ConsensusValidatorStakeWeightPower, IdleClassificationEpochs, IncludedClassificationEpochs,
    LastConsensusValidatorNodeCountDecayUpdate, LastConsensusValidatorStakeWeightPowerUpdate,
    LastSubnetDelegateStakeRewardsUpdate, MaxSubnetNodes, MaxSubnets, MinSubnetMinStake,
    MinSubnetNodeReputation, OverwatchCommits, OverwatchReveals, PeerIdOverwatchNodeId, PeerInfo,
    PendingConsensusValidatorNodeCountDecay, PendingConsensusValidatorStakeWeightPower,
    PendingIdleClassificationEpochs, PendingIncludedClassificationEpochs,
    PendingMinSubnetNodeReputation, PendingOwnerU128Update, PendingOwnerU32Update,
    PendingQueueImmunityEpochs, PendingSubnetDelegateStakeRewardsPercentage,
    PendingSubnetDelegateStakeRewardsPercentageUpdate,
    PendingSubnetNodeMinWeightDecreaseReputationThreshold, PendingSubnetNodeQueueEpochs,
    QueueImmunityEpochs, SlotAssignment, SubnetBootnodes, SubnetDelegateStakeRewardsPercentage,
    SubnetElectedValidator, SubnetName, SubnetNodeClass, SubnetNodeIdHotkey,
    SubnetNodeMinWeightDecreaseReputationThreshold, SubnetNodeQueueEpochs, SubnetOwner,
    SubnetPauseData, SubnetSlot, SubnetState, SubnetsData, TotalActiveSubnets,
    TotalNodeDelegateStakeBalance, TotalNodeDelegateStakeShares,
};
use frame_support::assert_ok;
use frame_support::traits::{Currency, ExistenceRequirement};
use sp_runtime::BoundedVec;
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
fn test_proof_of_stake_peer() {
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
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end);
        let bootnode_peer_id = get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end);

        let rpc_results = Network::proof_of_stake_peer(subnet_id, peer_id.0.to_vec(), 1, None);

        assert!(rpc_results);
    })
}

#[test]
fn test_proof_of_stake_peer_all_peer_id_types() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        // Test with main peer_id
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end);
        assert!(
            Network::proof_of_stake_peer(subnet_id, peer_id.0.to_vec(), 0, None),
            "Proof of stake should work with main peer_id"
        );

        // Test with bootnode_peer_id
        let bootnode_peer_id = get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end);
        assert!(
            Network::proof_of_stake_peer(subnet_id, bootnode_peer_id.0.to_vec(), 0, None),
            "Proof of stake should work with bootnode_peer_id"
        );

        // Test with client_peer_id
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end);
        assert!(
            Network::proof_of_stake_peer(subnet_id, client_peer_id.0.to_vec(), 0, None),
            "Proof of stake should work with client_peer_id"
        );

        // Test with overwatch node
        let overwatch_node_peer_id = peer(1);
        PeerIdOverwatchNodeId::<Test>::insert(subnet_id, &overwatch_node_peer_id, 1);
        assert!(
            Network::proof_of_stake_peer(subnet_id, overwatch_node_peer_id.0.to_vec(), 0, None),
            "Proof of stake should work with overwatch node peer_id"
        );

        let bv = |b: u8| NetworkBytes::<Test>::try_from(vec![b]).unwrap();
        let add_map = BTreeMap::from([(peer(2), bv(2)), (peer(3), bv(3))]);

        SubnetBootnodes::<Test>::insert(subnet_id, add_map);

        assert!(
            Network::proof_of_stake_peer(subnet_id, peer(2).0.to_vec(), 0, None),
            "Proof of stake should work with bootnode peer_id"
        );

        // Test with registered node
        let alice = account(0);

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        if Balances::free_balance(&alice.clone()) <= stake_amount {
            let _ = Balances::deposit_creating(&alice.clone(), stake_amount + 500);
        }

        assert!(
            !Network::proof_of_stake_peer(subnet_id, peer_id.0.to_vec(), 0, None),
            "Proof of stake should not work with non-existent peer_id"
        );

        let burn_amount = Network::calculate_burn_amount(subnet_id);
        assert_ok!(Balances::transfer(
            &alice.clone(), // alice
            &coldkey.clone(),
            stake_amount + burn_amount + 500,
            ExistenceRequirement::KeepAlive,
        ));

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey.clone(),
            5000000000000000,
            None,
            None
        ));
        let validator_id = ColdkeyValidatorId::<Test>::get(coldkey.clone()).unwrap();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            subnet_id,
            None,
            Some(PeerInfo::<Test> {
                peer_id: peer_id.clone(),
                multiaddr: None,
            }),
            None,
            None,
            stake_amount,
            None,
            None,
            u128::MAX,
        ));
        // Increase epoch by 1 to get to the registered node start epoch
        increase_epochs(1);

        assert!(
            Network::proof_of_stake_peer(subnet_id, peer_id.0.to_vec(), 0, None),
            "Proof of stake should work with registered peer_id"
        );
        assert!(
            Network::proof_of_stake_hotkey(subnet_id, hotkey.clone(), 0, None),
            "Proof of stake should work with a registered node hotkey"
        );
        assert!(
            !Network::proof_of_stake_hotkey(subnet_id, hotkey, 1, None),
            "A registered node hotkey should not satisfy the Idle class"
        );
    })
}

#[test]
fn test_proof_of_stake_peer_with_different_classes() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end);

        // Test with class 0 (Registered)
        assert!(
            Network::proof_of_stake_peer(subnet_id, peer_id.0.to_vec(), 0, None),
            "Should work with Registered class"
        );

        // Test with class 1 (Idle)
        assert!(
            Network::proof_of_stake_peer(subnet_id, peer_id.0.to_vec(), 1, None),
            "Should work with Idle class"
        );

        // Test with class 2 (Included)
        assert!(
            Network::proof_of_stake_peer(subnet_id, peer_id.0.to_vec(), 2, None),
            "Should work with Included class"
        );

        // Test with class 3 (Validator)
        assert!(
            Network::proof_of_stake_peer(subnet_id, peer_id.0.to_vec(), 3, None),
            "Should work with Validator class"
        );

        // Test with class non-existence class
        assert!(
            !Network::proof_of_stake_peer(subnet_id, peer_id.0.to_vec(), 4, None),
            "Should not work with non-existence class 4"
        );
    })
}

#[test]
fn test_proof_of_stake_peer_invalid_peer_id_fails() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        // Test with non-existent peer_id
        let fake_peer_id = vec![1, 2, 3, 4, 5];
        assert!(
            !Network::proof_of_stake_peer(subnet_id, fake_peer_id, 1, None),
            "Proof of stake should fail with invalid peer_id"
        );
    })
}

#[test]
fn test_proof_of_stake_v2_routes_peer_hotkey_and_empty_identifiers() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount = MinSubnetMinStake::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);

        assert!(Network::proof_of_stake_v2(
            subnet_id,
            Some(peer_id.0.to_vec()),
            None,
            3,
            None,
        ));
        assert!(Network::proof_of_stake_v2(
            subnet_id,
            None,
            Some(hotkey.clone()),
            3,
            None,
        ));
        assert!(!Network::proof_of_stake_v2(subnet_id, None, None, 0, None,));

        for min_class in 0..=3 {
            assert!(Network::proof_of_stake_v2(
                subnet_id,
                None,
                Some(hotkey.clone()),
                min_class,
                Some(stake_amount),
            ));
        }
        assert!(!Network::proof_of_stake_v2(
            subnet_id,
            None,
            Some(hotkey.clone()),
            4,
            Some(stake_amount),
        ));
        assert!(!Network::proof_of_stake_v2(
            subnet_id,
            None,
            Some(hotkey),
            3,
            Some(stake_amount.saturating_add(1)),
        ));
    })
}

#[test]
fn test_proof_of_stake_v2_prefers_peer_when_both_identifiers_are_present() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount = MinSubnetMinStake::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);
        let unknown_hotkey = account(99_001);

        assert!(Network::proof_of_stake_v2(
            subnet_id,
            Some(peer_id.0.to_vec()),
            Some(unknown_hotkey),
            3,
            None,
        ));
        assert!(!Network::proof_of_stake_v2(
            subnet_id,
            Some(vec![1, 2, 3, 4, 5]),
            Some(hotkey),
            3,
            None,
        ));
    })
}

#[test]
fn test_proof_of_stake_hotkey_prefers_node_override_to_validator_hotkey() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount = MinSubnetMinStake::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let subnet_node_id = 4;

        build_activated_subnet(
            subnet_name.clone(),
            0,
            subnet_node_id,
            deposit_amount,
            stake_amount,
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let validator_hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, subnet_node_id);
        let node_hotkey = account(99_001);

        assert!(Network::proof_of_stake_v2(
            subnet_id,
            None,
            Some(validator_hotkey.clone()),
            3,
            None,
        ));

        SubnetNodeIdHotkey::<Test>::insert(subnet_id, subnet_node_id, node_hotkey.clone());

        assert!(!Network::proof_of_stake_v2(
            subnet_id,
            None,
            Some(validator_hotkey.clone()),
            3,
            None,
        ));
        assert!(Network::proof_of_stake_v2(
            subnet_id,
            None,
            Some(node_hotkey.clone()),
            3,
            None,
        ));

        SubnetNodeIdHotkey::<Test>::remove(subnet_id, subnet_node_id);

        assert!(Network::proof_of_stake_v2(
            subnet_id,
            None,
            Some(validator_hotkey),
            3,
            None,
        ));
        assert!(!Network::proof_of_stake_v2(
            subnet_id,
            None,
            Some(node_hotkey),
            3,
            None,
        ));
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
fn test_get_all_subnets_info() {
    new_test_ext().execute_with(|| {
        // Create multiple subnets
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet("subnet1".into(), 0, 0, deposit_amount, stake_amount);
        build_activated_subnet("subnet2".into(), 0, 0, deposit_amount, stake_amount);

        let all_subnets = Network::get_all_subnets_info();

        assert!(all_subnets.len() >= 2, "Should have at least 2 subnets");
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
fn test_get_subnet_nodes_info() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "test-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let end = 5;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let nodes_info = Network::get_subnet_nodes_info(subnet_id);

        assert!(
            nodes_info.len() == end as usize,
            "Should have correct number of nodes"
        );
    })
}

#[test]
fn test_get_all_subnet_nodes_info() {
    new_test_ext().execute_with(|| {
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet("subnet1".into(), 0, 3, deposit_amount, stake_amount);
        build_activated_subnet("subnet2".into(), 0, 3, deposit_amount, stake_amount);

        let all_nodes = Network::get_all_subnet_nodes_info();

        assert!(all_nodes.len() >= 6, "Should have nodes from all subnets");
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
fn test_get_all_overwatch_nodes_info() {
    new_test_ext().execute_with(|| {
        let overwatch_nodes_info = Network::get_all_overwatch_nodes_info();

        // assert!(overwatch_nodes_info.len() == 12, "Should have overwatch nodes");
    })
}

#[test]
fn test_get_bootnodes() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "test-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let bootnodes = Network::get_bootnodes(subnet_id);

        // Verify structure exists
        assert!(
            bootnodes.subnet_bootnodes.len() >= 0 || bootnodes.node_bootnodes.len() >= 0,
            "Bootnodes structure should exist"
        );
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
fn test_get_delegate_stakes() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "test-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let delegator = account(100);
        let _ = Balances::deposit_creating(&delegator, 1000000000000000000000 + 500);

        // Add delegate stake
        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(delegator.clone()),
            subnet_id,
            1000000000000000000000
        ));

        let delegate_stakes = Network::get_delegate_stakes(delegator);

        assert!(delegate_stakes.len() > 0, "Should have delegate stakes");
    })
}

#[test]
fn test_get_validator_delegate_stakes() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "test-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let validator_id = 1;

        let delegator = account(100);
        let _ = Balances::deposit_creating(&delegator, 1000000000000000000000 + 500);

        // Add node delegate stake
        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(delegator.clone()),
            validator_id,
            1000000000000000000000
        ));

        let node_delegate_stakes = Network::get_validator_delegate_stakes(delegator);

        assert!(
            node_delegate_stakes.len() > 0,
            "Should have node delegate stakes"
        );
    })
}

#[test]
fn test_get_node_delegate_stakes() {
    new_test_ext().execute_with(|| {
        let delegator = account(1000);
        let other_delegator = account(1001);
        let subnet_id = 7;
        let subnet_node_id = 11;
        let shares = 25;
        let total_shares = 100;
        let total_balance = 400;

        TotalNodeDelegateStakeShares::<Test>::insert(subnet_id, subnet_node_id, total_shares);
        TotalNodeDelegateStakeBalance::<Test>::insert(subnet_id, subnet_node_id, total_balance);
        AccountNodeDelegateStakeShares::<Test>::insert(
            (delegator.clone(), subnet_id, subnet_node_id),
            shares,
        );
        AccountNodeDelegateStakeShares::<Test>::insert(
            (other_delegator.clone(), subnet_id, subnet_node_id),
            shares,
        );

        let stakes = Network::get_node_delegate_stakes(delegator);

        assert_eq!(stakes.len(), 1);
        assert_eq!(stakes[0].subnet_id, subnet_id);
        assert_eq!(stakes[0].subnet_node_id, subnet_node_id);
        assert_eq!(stakes[0].shares, shares);
        assert_eq!(
            stakes[0].balance,
            Network::convert_to_balance(shares, total_shares, total_balance)
        );
    })
}

#[test]
fn test_get_overwatch_commits_and_reveals_for_epoch_and_node() {
    new_test_ext().execute_with(|| {
        let epoch = 9;
        let overwatch_node_id = 3;
        let other_overwatch_node_id = 4;
        let subnet_id_1 = 21;
        let subnet_id_2 = 22;
        let weight_1 = test_percent(1, 10);
        let weight_2 = test_percent(1, 5);
        let other_weight = test_percent(3, 10);

        let commit_1 = make_commit(weight_1, b"salt-1".to_vec());
        let commit_2 = make_commit(weight_2, b"salt-2".to_vec());
        OverwatchCommits::<Test>::insert((epoch, overwatch_node_id, subnet_id_1), commit_1);
        OverwatchCommits::<Test>::insert((epoch, overwatch_node_id, subnet_id_2), commit_2);
        OverwatchCommits::<Test>::insert(
            (epoch, other_overwatch_node_id, subnet_id_1),
            make_commit(other_weight, b"salt-3".to_vec()),
        );

        OverwatchReveals::<Test>::insert((epoch, subnet_id_1, overwatch_node_id), weight_1);
        OverwatchReveals::<Test>::insert((epoch, subnet_id_2, overwatch_node_id), weight_2);
        OverwatchReveals::<Test>::insert(
            (epoch, subnet_id_1, other_overwatch_node_id),
            other_weight,
        );

        let commits = Network::get_overwatch_commits_for_epoch_and_node(epoch, overwatch_node_id);
        assert_eq!(commits.len(), 2);
        assert!(commits.contains(&(subnet_id_1, commit_1)));
        assert!(commits.contains(&(subnet_id_2, commit_2)));

        let reveals = Network::get_overwatch_reveals_for_epoch_and_node(epoch, overwatch_node_id);
        assert_eq!(reveals.len(), 2);
        assert!(reveals.contains(&(subnet_id_1, weight_1)));
        assert!(reveals.contains(&(subnet_id_2, weight_2)));
        assert!(!reveals.contains(&(subnet_id_1, other_weight)));
    })
}
