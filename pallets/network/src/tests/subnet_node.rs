use super::mock::*;
use crate::tests::test_utils::*;
use crate::Event;
use crate::{
    BootnodePeerIdSubnetNodeId, ClientPeerIdSubnetNodeId, CurrentNodeBurnRate,
    DefaultMaxVectorLength, Error, MaxDelegateStakePercentage, MaxRegisteredNodes,
    MaxRewardRateDecrease, MaxSubnetNodes, MaxSubnets, MinSubnetMinStake, MinSubnetNodes,
    MultiaddrSubnetNodeId, NodeRewardRateUpdatePeriod, NodeSlotIndex, NodeSubnetStake,
    PeerIdSubnetNodeId, PeerInfo, RegisteredSubnetNodesData, SubnetElectedValidator,
    SubnetMinStakeBalance, SubnetName, SubnetNode, SubnetNodeClass, SubnetNodeClassification,
    SubnetNodeElectionSlots, SubnetNodeQueueEpochs, SubnetNodeReputation, SubnetNodeV2,
    SubnetNodeValidatorId, SubnetNodesData, SubnetOwner, SubnetPauseCooldownEpochs,
    SubnetRegistrationEpochs, SubnetState, TotalActiveNodes, TotalActiveSubnetNodes,
    TotalActiveSubnets, TotalElectableNodes, TotalNodes, TotalStake, TotalSubnetElectableNodes,
    TotalSubnetNodeUids, TotalSubnetNodes, TotalSubnetStake, TotalSubnetUids, TotalValidatorIds,
    UniqueParamSubnetNodeId, ValidatorColdkey, ValidatorIdHotkey, ValidatorReputation,
    ValidatorSubnetNodes,
};
use frame_support::traits::Currency;
use frame_support::traits::ExistenceRequirement;
use frame_support::weights::WeightMeter;
use frame_support::BoundedVec;
use frame_support::{assert_err, assert_ok};
use sp_core::OpaquePeerId as PeerId;
use sp_std::collections::{btree_map::BTreeMap, btree_set::BTreeSet};

///
///
///
///
///
///
///
/// Subnet Nodes Add/Remove
///
///
///
///
///
///
///

#[test]
fn test_activate_subnet_then_register_subnet_node_then_activate_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount);

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            subnet_id,
            None,
            PeerInfo {
                peer_id: peer_id.clone(),
                multiaddr: None,
            },
            None,
            None,
            stake_amount,
            None,
            None,
            burn_amount + 100000000,
        ),);

        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let subnet_node = RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id);

        let start_epoch = subnet_node.classification.start_epoch;

        let queue_epochs = SubnetNodeQueueEpochs::<Test>::get(subnet_id);

        let epoch = Network::get_current_epoch_as_u32();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // increase to the nodes start epoch
        set_block_to_subnet_slot_epoch(subnet_epoch + queue_epochs + 2, subnet_id);

        let epoch = Network::get_current_epoch_as_u32();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // Get subnet weights (nodes only activate from queue if there are weights)
        // Note: This means a subnet is active if it gets weights
        let _ = Network::handle_subnet_emission_weights(epoch);

        // Trigger the node activation
        Network::emission_step(
            &mut WeightMeter::new(),
            System::block_number(),
            Network::get_current_epoch_as_u32(),
            Network::get_current_subnet_epoch_as_u32(subnet_id),
            subnet_id,
        );

        // Ensure node was activated from queue
        assert_eq!(
            RegisteredSubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
            Err(())
        );

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.classification.node_class, SubnetNodeClass::Idle);
        // assert_eq!(subnet_node.classification.start_epoch, subnet_epoch + 1);
        assert_eq!(subnet_node.classification.start_epoch, subnet_epoch);

        let new_total_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        assert_eq!(total_subnet_nodes + 1, new_total_nodes);
    })
}

#[test]
fn test_register_subnet_subnet_is_paused_error() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let max_subnets = MaxSubnets::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 4;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let pause_cooldown_epochs = SubnetPauseCooldownEpochs::<Test>::get();
        increase_epochs(pause_cooldown_epochs + 1);

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let epoch = Network::get_current_epoch_as_u32();

        // Transfer to new owner
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);

        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount);
        let starting_balance = Balances::free_balance(&coldkey.clone());

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: peer_id.clone(),
                    multiaddr: None,
                },
                None,
                None,
                stake_amount,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::SubnetIsPaused
        );
    });
}

#[test]
fn test_register_subnet_subnet_must_be_registering_or_active_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let max_subnets = MaxSubnets::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 4;

        build_registered_subnet_v2(
            subnet_name.clone(),
            0,
            4,
            deposit_amount,
            stake_amount,
            true,
            None,
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        // --- increase to enactment period
        let epochs = SubnetRegistrationEpochs::<Test>::get();
        increase_epochs(epochs + 1);

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);

        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount);
        let starting_balance = Balances::free_balance(&coldkey.clone());

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: peer_id.clone(),
                    multiaddr: None,
                },
                None,
                None,
                stake_amount,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::SubnetMustBeRegisteringOrActivated
        );
    });
}

#[test]
fn test_register_subnet_coldkey_registration_whitelist_error_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let max_subnets = MaxSubnets::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 4;

        build_registered_subnet_v2(
            subnet_name.clone(),
            0,
            4,
            deposit_amount,
            stake_amount,
            true,
            None,
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);

        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount);
        let starting_balance = Balances::free_balance(&coldkey.clone());
        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: peer_id.clone(),
                    multiaddr: None,
                },
                None,
                None,
                stake_amount,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::ValidatorIdNotInWhitelist
        );
    });
}

#[test]
fn test_register_subnet_max_registered_nodes_error_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let max_subnets = MaxSubnets::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 4;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let max_registered_nodes = 4;
        MaxRegisteredNodes::<Test>::insert(subnet_id, max_registered_nodes);

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);

        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount);
        let starting_balance = Balances::free_balance(&coldkey.clone());

        let mut touched = false;
        for n in end..max_registered_nodes + end + 2 {
            let _n = n + 1;

            let expected_validator_id = _n;
            let coldkey =
                if let Some(v_coldkey) = ValidatorColdkey::<Test>::get(expected_validator_id) {
                    v_coldkey
                } else {
                    let coldkey = get_coldkey(subnet_id, max_subnet_nodes, expected_validator_id);
                    let hotkey = get_hotkey(
                        subnet_id,
                        max_subnet_nodes,
                        max_subnets,
                        expected_validator_id,
                    );
                    let v_reward_rate = 50000000000000000; // 5%

                    assert_ok!(Network::register_validator(
                        RuntimeOrigin::signed(coldkey.clone()),
                        hotkey.clone(),
                        v_reward_rate,
                        None,
                        None
                    ));

                    coldkey
                };

            let validator_id = TotalValidatorIds::<Test>::get();
            let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, _n);
            let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, _n);
            let bootnode_peer_id = get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, _n);
            let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, _n);
            let burn_amount = Network::calculate_burn_amount(subnet_id);

            assert_ok!(Balances::transfer(
                &account(0), // alice
                &coldkey.clone(),
                amount + burn_amount + 500,
                ExistenceRequirement::KeepAlive,
            ));
            if _n - end > max_registered_nodes + 1 {
                touched = true;
                assert_err!(
                    Network::register_subnet_node(
                        RuntimeOrigin::signed(coldkey.clone()),
                        validator_id,
                        subnet_id,
                        None,
                        PeerInfo {
                            peer_id: peer_id.clone(),
                            multiaddr: None,
                        },
                        None,
                        None,
                        stake_amount,
                        None,
                        None,
                        u128::MAX,
                    ),
                    Error::<Test>::MaxQueuedNodes
                );
            } else {
                assert_ok!(Network::register_subnet_node(
                    RuntimeOrigin::signed(coldkey.clone()),
                    validator_id,
                    subnet_id,
                    None,
                    PeerInfo {
                        peer_id: peer_id.clone(),
                        multiaddr: None,
                    },
                    None,
                    None,
                    stake_amount,
                    None,
                    None,
                    u128::MAX,
                ));
            }
        }
        assert!(touched);
    });
}

#[test]
fn test_register_subnet_node_and_then_update_a_param_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let max_subnets = MaxSubnets::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 4;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let burn_amount = Network::calculate_burn_amount(subnet_id);

        assert_ok!(Balances::transfer(
            &account(0), // alice
            &coldkey.clone(),
            amount + burn_amount + 500,
            ExistenceRequirement::KeepAlive,
        ));

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        let unique: Vec<u8> = "a".into();
        let bounded_unique: BoundedVec<u8, DefaultMaxVectorLength> =
            unique.try_into().expect("String too long");

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            subnet_id,
            None,
            PeerInfo {
                peer_id: peer_id.clone(),
                multiaddr: None,
            },
            None,
            None,
            stake_amount,
            Some(bounded_unique.clone()),
            None,
            u128::MAX,
        ));

        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let subnet_node = RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.unique, Some(bounded_unique.clone()));

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 2);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 2);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 2);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 2);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 2);
        let burn_amount = Network::calculate_burn_amount(subnet_id);

        assert_ok!(Balances::transfer(
            &account(0), // alice
            &coldkey.clone(),
            amount + burn_amount + 500,
            ExistenceRequirement::KeepAlive,
        ));

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: peer_id.clone(),
                    multiaddr: None,
                },
                None,
                None,
                stake_amount,
                Some(bounded_unique.clone()),
                None,
                u128::MAX,
            ),
            Error::<Test>::SubnetNodeUniqueParamTaken
        );
    })
}

#[test]
fn test_register_subnet_node_post_subnet_activation_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 4;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);

        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount);
        let starting_balance = Balances::free_balance(&coldkey.clone());

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            subnet_id,
            None,
            PeerInfo {
                peer_id: peer_id.clone(),
                multiaddr: None,
            },
            Some(PeerInfo {
                peer_id: bootnode_peer_id.clone(),
                multiaddr: None,
            }),
            None,
            amount,
            None,
            None,
            u128::MAX,
        ));

        let post_balance = Balances::free_balance(&coldkey.clone());
        assert_eq!(post_balance, starting_balance - amount - burn_amount);

        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let subnet_node = RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.validator_id, validator_id);
        assert_eq!(subnet_node.peer_info.peer_id, peer_id.clone());
        assert_eq!(
            subnet_node.classification.node_class,
            SubnetNodeClass::Registered
        );

        let peer_account = PeerIdSubnetNodeId::<Test>::get(subnet_id, peer_id.clone());
        assert_eq!(peer_account, subnet_node_id);

        let bootnode_peer_account =
            BootnodePeerIdSubnetNodeId::<Test>::get(subnet_id, bootnode_peer_id.clone());
        assert_eq!(bootnode_peer_account, subnet_node_id);

        let account_subnet_stake = NodeSubnetStake::<Test>::get(validator_id, subnet_id);
        assert_eq!(account_subnet_stake, amount);

        // System::assert_last_event(RuntimeEvent::Network(crate::Event::SubnetNodeRegistered {
        //     subnet_id,
        //     subnet_node_id: subnet_node_id,
        //     coldkey: coldkey.clone(),
        //     hotkey: hotkey.clone(),
        //     data: subnet_node.clone(),
        // }));
    })
}

#[test]
fn test_activate_subnet_node_post_subnet_activation_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount);

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            subnet_id,
            None,
            PeerInfo {
                peer_id: peer_id.clone(),
                multiaddr: None,
            },
            Some(PeerInfo {
                peer_id: bootnode_peer_id.clone(),
                multiaddr: None,
            }),
            None,
            amount,
            None,
            None,
            u128::MAX,
        ));

        let new_total_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        assert_eq!(total_subnet_nodes + 1, new_total_nodes);

        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let subnet_node = RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        let start_epoch = subnet_node.classification.start_epoch;

        set_block_to_subnet_slot_epoch(start_epoch, subnet_id);

        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        let prev_total_active_subnet_nodes = TotalActiveSubnetNodes::<Test>::get(subnet_id);
        let prev_total_active_nodes = TotalActiveNodes::<Test>::get();
        let prev_validator_reputation = ValidatorReputation::<Test>::get(validator_id);

        let queue_epochs = SubnetNodeQueueEpochs::<Test>::get(subnet_id);

        let epoch = Network::get_current_epoch_as_u32();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // increase to the nodes start epoch
        set_block_to_subnet_slot_epoch(subnet_epoch + queue_epochs + 2, subnet_id);

        let epoch = Network::get_current_epoch_as_u32();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // Get subnet weights (nodes only activate from queue if there are weights)
        // Note: This means a subnet is active if it gets weights
        let _ = Network::handle_subnet_emission_weights(epoch);

        // Trigger the node activation
        Network::emission_step(
            &mut WeightMeter::new(),
            System::block_number(),
            Network::get_current_epoch_as_u32(),
            Network::get_current_subnet_epoch_as_u32(subnet_id),
            subnet_id,
        );

        assert_eq!(
            RegisteredSubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
            Err(())
        );

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.classification.node_class, SubnetNodeClass::Idle);
        // assert_eq!(subnet_node.classification.start_epoch, subnet_epoch + 1);
        // assert_eq!(subnet_node.classification.start_epoch, subnet_epoch);

        assert_eq!(
            prev_total_active_subnet_nodes + 1,
            TotalActiveSubnetNodes::<Test>::get(subnet_id)
        );
        assert_eq!(prev_total_active_nodes + 1, TotalActiveNodes::<Test>::get());
        assert_eq!(
            prev_validator_reputation.lifetime_node_count + 1,
            ValidatorReputation::<Test>::get(validator_id).lifetime_node_count
        );
        assert_eq!(
            prev_validator_reputation.total_active_nodes + 1,
            ValidatorReputation::<Test>::get(validator_id).total_active_nodes
        );
    })
}

#[test]
fn test_remove_subnet_node_registered_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 12;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount);

        let unique: Vec<u8> = "a".into();
        let bounded_unique: BoundedVec<u8, DefaultMaxVectorLength> =
            unique.try_into().expect("String too long");

        let non_unique: Vec<u8> = "a".into();
        let bounded_non_unique: BoundedVec<u8, DefaultMaxVectorLength> =
            non_unique.try_into().expect("String too long");

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            subnet_id,
            None,
            PeerInfo {
                peer_id: peer_id.clone(),
                multiaddr: None,
            },
            Some(PeerInfo {
                peer_id: bootnode_peer_id.clone(),
                multiaddr: None,
            }),
            None,
            amount,
            None,
            None,
            u128::MAX,
        ));

        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let validator_subnet_nodes = ValidatorSubnetNodes::<Test>::get(validator_id);
        assert!(validator_subnet_nodes
            .get(&subnet_id)
            .unwrap()
            .contains(&subnet_node_id));

        let prev_total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        let prev_total_nodes = TotalNodes::<Test>::get();

        let prev_total_active_subnet_nodes = TotalActiveSubnetNodes::<Test>::get(subnet_id);
        let prev_total_active_nodes = TotalActiveNodes::<Test>::get();

        let prev_slot_list_len = SubnetNodeElectionSlots::<Test>::get(subnet_id).len();

        assert_ok!(Network::remove_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetNodeRemoved {
                subnet_id: subnet_id,
                subnet_node_id: subnet_node_id,
            }
        );

        assert_eq!(
            RegisteredSubnetNodesData::<Test>::iter_prefix(subnet_id).count(),
            0
        );

        let peer_account = PeerIdSubnetNodeId::<Test>::try_get(subnet_id, peer_id.clone());
        assert_eq!(peer_account, Err(()));

        let bootnode_peer_account =
            BootnodePeerIdSubnetNodeId::<Test>::try_get(subnet_id, bootnode_peer_id.clone());
        assert_eq!(bootnode_peer_account, Err(()));

        let validator_subnet_nodes = ValidatorSubnetNodes::<Test>::get(validator_id);
        assert_eq!(validator_subnet_nodes.get(&subnet_id), None);

        assert_eq!(
            prev_total_subnet_nodes - 1,
            TotalSubnetNodes::<Test>::get(subnet_id)
        );
        assert_eq!(prev_total_nodes - 1, TotalNodes::<Test>::get());

        // Not active node, this shouldn't change
        assert_eq!(
            prev_total_active_subnet_nodes,
            TotalActiveSubnetNodes::<Test>::get(subnet_id)
        );
        assert_eq!(prev_total_active_nodes, TotalActiveNodes::<Test>::get());
        assert_eq!(
            prev_slot_list_len,
            SubnetNodeElectionSlots::<Test>::get(subnet_id).len()
        );

        //
        //
        //
        // Test another node and force it into Idle
        //
        //
        //

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 2);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 2);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 2);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 2);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 2);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount);

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            subnet_id,
            None,
            PeerInfo {
                peer_id: peer_id.clone(),
                multiaddr: None,
            },
            Some(PeerInfo {
                peer_id: bootnode_peer_id.clone(),
                multiaddr: None,
            }),
            None,
            amount,
            None,
            None,
            u128::MAX,
        ));

        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let subnet_node = RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        let initial_start_epoch = subnet_node.classification.start_epoch;

        let queue_epochs = SubnetNodeQueueEpochs::<Test>::get(subnet_id);

        let epoch = Network::get_current_epoch_as_u32();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // increase to the nodes start epoch
        set_block_to_subnet_slot_epoch(subnet_epoch + queue_epochs + 2, subnet_id);

        let epoch = Network::get_current_epoch_as_u32();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // Get subnet weights (nodes only activate from queue if there are weights)
        // Note: This means a subnet is active if it gets weights
        let _ = Network::handle_subnet_emission_weights(epoch);

        // Trigger the node activation
        Network::emission_step(
            &mut WeightMeter::new(),
            System::block_number(),
            Network::get_current_epoch_as_u32(),
            Network::get_current_subnet_epoch_as_u32(subnet_id),
            subnet_id,
        );

        assert_eq!(
            RegisteredSubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
            Err(())
        );

        assert!(SubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id).is_ok());

        assert!(SubnetNodesData::<Test>::contains_key(
            subnet_id,
            subnet_node_id
        ));

        let validator_subnet_nodes = ValidatorSubnetNodes::<Test>::get(validator_id);
        assert!(validator_subnet_nodes
            .get(&subnet_id)
            .unwrap()
            .contains(&subnet_node_id));

        let prev_total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        let prev_total_nodes = TotalNodes::<Test>::get();

        let prev_total_active_subnet_nodes = TotalActiveSubnetNodes::<Test>::get(subnet_id);
        let prev_total_active_nodes = TotalActiveNodes::<Test>::get();

        let prev_slot_list_len = SubnetNodeElectionSlots::<Test>::get(subnet_id).len();

        assert_ok!(Network::remove_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
        ));

        assert_eq!(
            SubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
            Err(())
        );

        let peer_account = PeerIdSubnetNodeId::<Test>::try_get(subnet_id, peer_id.clone());
        assert_eq!(peer_account, Err(()));

        let bootnode_peer_account =
            BootnodePeerIdSubnetNodeId::<Test>::try_get(subnet_id, bootnode_peer_id.clone());
        assert_eq!(bootnode_peer_account, Err(()));

        let validator_subnet_nodes = ValidatorSubnetNodes::<Test>::get(validator_id);
        assert_eq!(validator_subnet_nodes.get(&subnet_id), None);

        assert_eq!(
            prev_total_subnet_nodes - 1,
            TotalSubnetNodes::<Test>::get(subnet_id)
        );
        assert_eq!(prev_total_nodes - 1, TotalNodes::<Test>::get());

        assert_eq!(
            prev_total_active_subnet_nodes - 1,
            TotalActiveSubnetNodes::<Test>::get(subnet_id)
        );
        assert_eq!(prev_total_active_nodes - 1, TotalActiveNodes::<Test>::get());
        // Node not Validator yet, this should not change
        assert_eq!(
            prev_slot_list_len,
            SubnetNodeElectionSlots::<Test>::get(subnet_id).len()
        );

        //
        //
        //
        // Test another node and force it into Included
        //
        //
        //

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 3);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 3);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 3);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 3);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 3);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount);

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            subnet_id,
            None,
            PeerInfo {
                peer_id: peer_id.clone(),
                multiaddr: None,
            },
            Some(PeerInfo {
                peer_id: bootnode_peer_id.clone(),
                multiaddr: None,
            }),
            None,
            amount,
            None,
            None,
            u128::MAX,
        ));

        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let subnet_node = RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        let initial_start_epoch = subnet_node.classification.start_epoch;

        let queue_epochs = SubnetNodeQueueEpochs::<Test>::get(subnet_id);

        let epoch = Network::get_current_epoch_as_u32();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // increase to the nodes start epoch
        set_block_to_subnet_slot_epoch(subnet_epoch + queue_epochs + 2, subnet_id);

        let epoch = Network::get_current_epoch_as_u32();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // Get subnet weights (nodes only activate from queue if there are weights)
        // Note: This means a subnet is active if it gets weights
        let _ = Network::handle_subnet_emission_weights(epoch);

        // Trigger the node activation
        Network::emission_step(
            &mut WeightMeter::new(),
            System::block_number(),
            Network::get_current_epoch_as_u32(),
            Network::get_current_subnet_epoch_as_u32(subnet_id),
            subnet_id,
        );

        assert_eq!(
            RegisteredSubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
            Err(())
        );

        // Force to included
        let mut subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        subnet_node.classification.start_epoch = 0;
        subnet_node.classification.node_class = SubnetNodeClass::Included;
        SubnetNodesData::<Test>::insert(subnet_id, subnet_node_id, subnet_node);

        assert!(SubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id).is_ok());

        let validator_subnet_nodes = ValidatorSubnetNodes::<Test>::get(validator_id);
        assert!(validator_subnet_nodes
            .get(&subnet_id)
            .unwrap()
            .contains(&subnet_node_id));

        let prev_total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        let prev_total_nodes = TotalNodes::<Test>::get();

        let prev_total_active_subnet_nodes = TotalActiveSubnetNodes::<Test>::get(subnet_id);
        let prev_total_active_nodes = TotalActiveNodes::<Test>::get();

        let prev_slot_list_len = SubnetNodeElectionSlots::<Test>::get(subnet_id).len();

        assert_ok!(Network::remove_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
        ));

        assert_eq!(
            SubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
            Err(())
        );

        let peer_account = PeerIdSubnetNodeId::<Test>::try_get(subnet_id, peer_id.clone());
        assert_eq!(peer_account, Err(()));

        let bootnode_peer_account =
            BootnodePeerIdSubnetNodeId::<Test>::try_get(subnet_id, bootnode_peer_id.clone());
        assert_eq!(bootnode_peer_account, Err(()));

        let validator_subnet_nodes = ValidatorSubnetNodes::<Test>::get(validator_id); // This is tested, see `test_clean_validator_subnet_nodes`
        assert_eq!(validator_subnet_nodes.get(&subnet_id), None);

        assert_eq!(
            prev_total_subnet_nodes - 1,
            TotalSubnetNodes::<Test>::get(subnet_id)
        );
        assert_eq!(prev_total_nodes - 1, TotalNodes::<Test>::get());

        assert_eq!(
            prev_total_active_subnet_nodes - 1,
            TotalActiveSubnetNodes::<Test>::get(subnet_id)
        );
        assert_eq!(prev_total_active_nodes - 1, TotalActiveNodes::<Test>::get());

        assert_eq!(
            prev_slot_list_len,
            SubnetNodeElectionSlots::<Test>::get(subnet_id).len()
        );

        //
        //
        //
        // Test another node and force it into Validator
        //
        //
        //

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 4);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 4);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 4);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 4);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 4);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount);

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            subnet_id,
            None,
            PeerInfo {
                peer_id: peer_id.clone(),
                multiaddr: None,
            },
            Some(PeerInfo {
                peer_id: bootnode_peer_id.clone(),
                multiaddr: None,
            }),
            None,
            amount,
            None,
            None,
            u128::MAX,
        ));

        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let subnet_node = RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        let initial_start_epoch = subnet_node.classification.start_epoch;

        let queue_epochs = SubnetNodeQueueEpochs::<Test>::get(subnet_id);

        let epoch = Network::get_current_epoch_as_u32();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // increase to the nodes start epoch
        set_block_to_subnet_slot_epoch(subnet_epoch + queue_epochs + 2, subnet_id);

        let epoch = Network::get_current_epoch_as_u32();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // Get subnet weights (nodes only activate from queue if there are weights)
        // Note: This means a subnet is active if it gets weights
        let _ = Network::handle_subnet_emission_weights(epoch);

        // Trigger the node activation
        Network::emission_step(
            &mut WeightMeter::new(),
            System::block_number(),
            Network::get_current_epoch_as_u32(),
            Network::get_current_subnet_epoch_as_u32(subnet_id),
            subnet_id,
        );

        assert_eq!(
            RegisteredSubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
            Err(())
        );

        let mut subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        subnet_node.classification.start_epoch = 0;
        subnet_node.classification.node_class = SubnetNodeClass::Validator;

        SubnetNodesData::<Test>::insert(subnet_id, subnet_node_id, subnet_node);

        Network::insert_node_into_election_slot(subnet_id, subnet_node_id);

        let validator_subnet_nodes = ValidatorSubnetNodes::<Test>::get(validator_id);
        assert!(validator_subnet_nodes
            .get(&subnet_id)
            .unwrap()
            .contains(&subnet_node_id));

        let prev_total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        let prev_total_nodes = TotalNodes::<Test>::get();

        let prev_total_active_subnet_nodes = TotalActiveSubnetNodes::<Test>::get(subnet_id);
        let prev_total_active_nodes = TotalActiveNodes::<Test>::get();

        let prev_slot_list_len = SubnetNodeElectionSlots::<Test>::get(subnet_id).len();

        let prev_total_subnet_electable_nodes = TotalSubnetElectableNodes::<Test>::get(subnet_id);
        let prev_total_electable_nodes = TotalElectableNodes::<Test>::get();

        let rep = ValidatorReputation::<Test>::get(validator_id);
        let rep_total_active_nodes = rep.total_active_nodes;

        assert_ok!(Network::remove_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
        ));

        assert_eq!(
            SubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
            Err(())
        );

        let peer_account = PeerIdSubnetNodeId::<Test>::try_get(subnet_id, peer_id.clone());
        assert_eq!(peer_account, Err(()));

        let bootnode_peer_account =
            BootnodePeerIdSubnetNodeId::<Test>::try_get(subnet_id, bootnode_peer_id.clone());
        assert_eq!(bootnode_peer_account, Err(()));

        let validator_subnet_nodes = ValidatorSubnetNodes::<Test>::get(validator_id); // This is tested, see `test_clean_validator_subnet_nodes`
        assert_eq!(validator_subnet_nodes.get(&subnet_id), None);

        assert_eq!(
            prev_total_subnet_nodes - 1,
            TotalSubnetNodes::<Test>::get(subnet_id)
        );
        assert_eq!(prev_total_nodes - 1, TotalNodes::<Test>::get());

        assert_eq!(
            prev_total_active_subnet_nodes - 1,
            TotalActiveSubnetNodes::<Test>::get(subnet_id)
        );
        assert_eq!(prev_total_active_nodes - 1, TotalActiveNodes::<Test>::get());

        assert_eq!(
            prev_total_subnet_electable_nodes - 1,
            TotalSubnetElectableNodes::<Test>::get(subnet_id)
        );
        assert_eq!(
            prev_total_electable_nodes - 1,
            TotalElectableNodes::<Test>::get()
        );

        assert_eq!(
            prev_slot_list_len - 1,
            SubnetNodeElectionSlots::<Test>::get(subnet_id).len()
        );

        assert_eq!(
            rep_total_active_nodes - 1,
            ValidatorReputation::<Test>::get(validator_id).total_active_nodes
        );
    })
}

#[test]
fn test_register_subnet_node_subnet_err_v2() {
    new_test_ext().execute_with(|| {
        let subnet_id = 0;

        let max_subnets = MaxSubnets::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 0;

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        let amount: u128 = 1000;
        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: peer_id.clone(),
                    multiaddr: None,
                },
                Some(PeerInfo {
                    peer_id: bootnode_peer_id.clone(),
                    multiaddr: None,
                }),
                None,
                amount,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::InvalidSubnetId
        );

        let subnet_id = 1;

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: peer_id.clone(),
                    multiaddr: None,
                },
                Some(PeerInfo {
                    peer_id: bootnode_peer_id.clone(),
                    multiaddr: None,
                }),
                None,
                amount,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::InvalidSubnetId
        );
    })
}

// #[test]
// fn test_get_classification_subnet_nodes() {
//     new_test_ext().execute_with(|| {
//         let subnet_name: Vec<u8> = "subnet-name".into();

//         let deposit_amount: u128 = 10000000000000000000000;
//         let amount: u128 = 1000000000000000000000;

//         let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
//         let end = 4;

//         build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

//         let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
//         let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
//         let epoch_length = EpochLength::get();
//         let subnet_epoch: u32 = Network::get_current_subnet_epoch_as_u32(subnet_id);

//         let submittable = Network::get_active_classified_subnet_nodes(
//             subnet_id,
//             &SubnetNodeClass::Validator,
//             subnet_epoch,
//         );

//         assert_eq!(submittable.len() as u32, total_subnet_nodes);
//     })
// }

#[test]
fn test_register_subnet_node_not_exists_err() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 16;

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let used_hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount);

        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        assert_eq!(Network::total_subnet_nodes(subnet_id), total_subnet_nodes);

        let bad_peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end);

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: bad_peer_id.clone(),
                    multiaddr: None,
                },
                Some(PeerInfo {
                    peer_id: bootnode_peer_id.clone(),
                    multiaddr: None,
                }),
                None,
                amount,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::PeerIdExist
        );

        let bad_bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end);

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: bad_peer_id.clone(),
                    multiaddr: None,
                },
                Some(PeerInfo {
                    peer_id: bad_bootnode_peer_id.clone(),
                    multiaddr: None,
                }),
                Some(PeerInfo {
                    peer_id: client_peer_id.clone(),
                    multiaddr: None,
                }),
                amount,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::PeerIdExist
        );

        let bad_client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end);

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: bad_peer_id.clone(),
                    multiaddr: None,
                },
                Some(PeerInfo {
                    peer_id: bootnode_peer_id.clone(),
                    multiaddr: None,
                }),
                Some(PeerInfo {
                    peer_id: bad_client_peer_id.clone(),
                    multiaddr: None,
                }),
                amount,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::PeerIdExist
        );
    })
}

#[test]
fn test_add_subnet_node_stake_err_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 12;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let deposit_amount: u128 = 100000;
        let amount: u128 = 1;

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount);

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: peer_id.clone(),
                    multiaddr: None,
                },
                None,
                None,
                1,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::MinStakeNotReached
        );
    })
}

#[test]
fn test_add_subnet_node_stake_not_enough_balance_err_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 4;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let deposit_amount: u128 = 999999999999999999999;

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), burn_amount + 500);

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: peer_id.clone(),
                    multiaddr: None,
                },
                None,
                None,
                amount,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::NotEnoughBalanceToStake
        );
    })
}

#[test]
fn test_register_subnet_node_invalid_peer_id_err_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let max_subnets = MaxSubnets::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 4;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let peer_id = format!("2");
        let bootnode_peer_id = format!("3");
        let client_peer_id = format!("4");

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bad_peer: PeerId = PeerId(peer_id.clone().into());
        let bootnode_peer: PeerId = PeerId(bootnode_peer_id.clone().into());
        let client_peer: PeerId = PeerId(client_peer_id.clone().into());

        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount);

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: bad_peer.clone(),
                    multiaddr: None,
                },
                None,
                None,
                amount,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::InvalidPeerId
        );

        let valid_peer_id = peer(subnets * max_subnet_nodes + end + 1);
        let valid_bootnode_peer_id = peer(subnets * max_subnet_nodes + end + 2);
        let valid_client_peer_id = peer(subnets * max_subnet_nodes + end + 3);

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: valid_peer_id.clone(),
                    multiaddr: None,
                },
                Some(PeerInfo {
                    peer_id: bad_peer.clone(),
                    multiaddr: None,
                }),
                Some(PeerInfo {
                    peer_id: valid_client_peer_id.clone(),
                    multiaddr: None,
                }),
                amount,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::InvalidPeerId
        );

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: valid_peer_id.clone(),
                    multiaddr: None,
                },
                Some(PeerInfo {
                    peer_id: valid_bootnode_peer_id.clone(),
                    multiaddr: None,
                }),
                Some(PeerInfo {
                    peer_id: bad_peer.clone(),
                    multiaddr: None,
                }),
                amount,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::InvalidPeerId
        );
    })
}

#[test]
fn test_remove_subnet_node_not_key_owner_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 4;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let deposit_amount: u128 = 1000000000000000000000000;

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount);

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            subnet_id,
            None,
            PeerInfo {
                peer_id: peer_id.clone(),
                multiaddr: None,
            },
            None,
            None,
            amount,
            None,
            None,
            u128::MAX,
        ));

        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        assert_err!(
            Network::remove_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id + 1
            ),
            Error::<Test>::NotKeyOwner
        );
    });
}

#[test]
fn test_remove_subnet_nodes_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 1000000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 4;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        let amount_staked = TotalSubnetStake::<Test>::get(subnet_id);
        let remove_n_peers = total_subnet_nodes / 2;

        let block_number = System::block_number();
        let epoch_length = EpochLength::get();
        let subnet_epoch: u32 = Network::get_current_subnet_epoch_as_u32(subnet_id);

        for n in 0..remove_n_peers {
            let _n = n + 1;
            let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, _n).unwrap();
            assert_ok!(Network::remove_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                _n,
            ));
            let subnet_node_data = SubnetNodesData::<Test>::try_get(subnet_id, _n);
            assert_eq!(subnet_node_data, Err(()));
        }

        let node_set = Network::get_active_classified_subnet_nodes_v2(
            subnet_id,
            &SubnetNodeClass::Idle,
            subnet_epoch,
        );

        assert_eq!(
            node_set.len(),
            (total_subnet_nodes - remove_n_peers) as usize
        );
        assert_eq!(Network::total_stake(), amount_staked);
        assert_eq!(Network::total_subnet_stake(subnet_id), amount_staked);
        assert_eq!(
            TotalSubnetNodes::<Test>::get(subnet_id),
            total_subnet_nodes - remove_n_peers
        );

        for n in 0..remove_n_peers {
            let _n = n + 1;
            let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);

            let subnet_node_account =
                PeerIdSubnetNodeId::<Test>::try_get(subnet_id, peer_id.clone());
            assert_eq!(subnet_node_account, Err(()));

            // Stake should still be present since not removed yet
            let node_subnet_stake = NodeSubnetStake::<Test>::get(_n, subnet_id);
            assert_eq!(node_subnet_stake, stake_amount);
        }

        let total_subnet_stake = TotalSubnetStake::<Test>::get(subnet_id);
        assert_eq!(total_subnet_stake, amount_staked);

        let total_stake = TotalStake::<Test>::get();
        assert_eq!(total_subnet_stake, amount_staked);
    });
}

#[test]
fn test_update_peer_id_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end);
        let bootnode_peer_id = get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end);

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let subnet_node_id = end;

        let coldkey =
            Network::get_subnet_node_associated_coldkey(subnet_id, subnet_node_id).unwrap();

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);

        let current_peer_id = subnet_node.peer_info.peer_id;
        let new_peer_info = PeerInfo {
            peer_id: peer(500),
            multiaddr: None,
        };

        assert_ok!(Network::update_node_peer_info(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            new_peer_info.clone()
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetNodeUpdatePeerInfo {
                subnet_id,
                subnet_node_id,
                peer_info: new_peer_info.clone()
            }
        );

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.peer_info.peer_id, peer(500));
        assert_ne!(subnet_node.peer_info.peer_id, current_peer_id);
        assert_eq!(subnet_node.peer_info.multiaddr, None);

        let peer_subnet_node_id = PeerIdSubnetNodeId::<Test>::get(subnet_id, peer(500));
        assert_eq!(peer_subnet_node_id, subnet_node_id);

        assert_eq!(
            PeerIdSubnetNodeId::<Test>::try_get(subnet_id, &current_peer_id),
            Err(())
        );

        let multiaddr_subnet_node_id = MultiaddrSubnetNodeId::<Test>::try_get(
            subnet_id,
            get_multiaddr(Some(subnet_id), Some(subnet_node_id), None).unwrap(),
        );
        assert_eq!(multiaddr_subnet_node_id, Err(()));

        let prev_peer_subnet_node_id = PeerIdSubnetNodeId::<Test>::get(subnet_id, &current_peer_id);
        assert_ne!(prev_peer_subnet_node_id, subnet_node_id);

        // test using previous peer id under a diff subnet node
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end - 1).unwrap();
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end - 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end - 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end - 1);

        let subnet_node_id = end - 1;

        let new_peer_info = PeerInfo {
            peer_id: current_peer_id.clone(),
            multiaddr: None,
        };

        assert_ok!(Network::update_node_peer_info(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            new_peer_info.clone()
        ));

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.peer_info.peer_id, new_peer_info.clone().peer_id);

        let peer_subnet_node_id =
            PeerIdSubnetNodeId::<Test>::get(subnet_id, current_peer_id.clone());
        assert_eq!(peer_subnet_node_id, subnet_node_id);
    })
}

#[test]
fn test_update_peer_id_exists_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end).unwrap();

        let subnet_node_id = end;

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);

        let current_peer_info = subnet_node.peer_info.clone();
        let current_peer_id = subnet_node.peer_info.clone().peer_id;

        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end - 1);
        let new_peer_info = PeerInfo {
            peer_id: peer_id.clone(),
            multiaddr: None,
        };

        // Ensure peer id always present
        assert_err!(
            Network::update_node_peer_info(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                new_peer_info
            ),
            Error::<Test>::PeerIdExist
        );

        // --- fail if same peer id
        assert_err!(
            Network::update_node_peer_info(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                current_peer_info
            ),
            Error::<Test>::PeerIdExist
        );
    })
}

#[test]
fn test_update_peer_id_not_key_owner_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end).unwrap();

        let subnet_node_id = end;

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);

        let current_peer_id = subnet_node.peer_info.clone().peer_id;
        let current_peer_info = subnet_node.peer_info.clone();

        assert_err!(
            Network::update_node_peer_info(
                RuntimeOrigin::signed(account(2)),
                subnet_id,
                subnet_node_id,
                current_peer_info
            ),
            Error::<Test>::NotKeyOwner
        );
    })
}

#[test]
fn test_update_peer_id_invalid_peer_id_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end).unwrap();

        let subnet_node_id = end;

        let peer_id = format!("2");

        let bad_peer: PeerId = PeerId(peer_id.clone().into());
        let new_peer_info = PeerInfo {
            peer_id: bad_peer,
            multiaddr: None,
        };

        assert_err!(
            Network::update_node_peer_info(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                new_peer_info.clone()
            ),
            Error::<Test>::InvalidPeerId
        );
    })
}

#[test]
fn test_update_bootnode_peer_id_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end);

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end).unwrap();

        let subnet_node_id = end;

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);

        let current_bootnode_peer_info = subnet_node.bootnode_peer_info.clone();
        let current_bootnode_peer_id = subnet_node.bootnode_peer_info.clone().unwrap().peer_id;
        let current_bootnode_multiaddr = subnet_node.bootnode_peer_info.clone().unwrap().multiaddr;

        let curr_bootnode_multiaddr_subnet_node_id = MultiaddrSubnetNodeId::<Test>::get(
            subnet_id,
            get_multiaddr(Some(subnet_id), Some(subnet_node_id), Some(1)).unwrap(),
        );
        assert_eq!(curr_bootnode_multiaddr_subnet_node_id, subnet_node_id);

        // Updated peer info
        let new_peer_info = Some(PeerInfo {
            peer_id: peer(500),
            multiaddr: None,
        });

        assert_ok!(Network::update_node_bootnode_peer_info(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            new_peer_info.clone()
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetNodeUpdateBootnodePeerInfo {
                subnet_id,
                subnet_node_id,
                bootnode_peer_info: new_peer_info.clone()
            }
        );

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        // Check new peer Id
        assert_eq!(
            subnet_node.bootnode_peer_info.clone().unwrap().peer_id,
            new_peer_info.clone().unwrap().peer_id
        );
        assert_eq!(
            subnet_node.bootnode_peer_info.clone().unwrap().multiaddr,
            new_peer_info.clone().unwrap().multiaddr
        );
        assert_ne!(
            subnet_node.bootnode_peer_info.clone().unwrap().peer_id,
            current_bootnode_peer_id
        );
        assert_ne!(
            subnet_node.bootnode_peer_info.clone().unwrap().multiaddr,
            current_bootnode_multiaddr
        );

        let bootnode_peer_subnet_node_id =
            BootnodePeerIdSubnetNodeId::<Test>::get(subnet_id, peer(500));
        assert_eq!(bootnode_peer_subnet_node_id, subnet_node_id);

        assert_eq!(
            BootnodePeerIdSubnetNodeId::<Test>::try_get(subnet_id, &current_bootnode_peer_id),
            Err(())
        );

        // Check multiaddr is None
        assert_eq!(
            subnet_node.bootnode_peer_info.clone().unwrap().multiaddr,
            None
        );

        // Ensure old multaddr was removed
        let bootnode_multiaddr_subnet_node_id = MultiaddrSubnetNodeId::<Test>::try_get(
            subnet_id,
            get_multiaddr(Some(subnet_id), Some(subnet_node_id), Some(1)).unwrap(),
        );
        assert_eq!(bootnode_multiaddr_subnet_node_id, Err(()));

        let prev_bootnode_peer_subnet_node_id =
            BootnodePeerIdSubnetNodeId::<Test>::get(subnet_id, &current_bootnode_peer_id);
        assert_ne!(prev_bootnode_peer_subnet_node_id, subnet_node_id);

        // update back to original with peer info
        assert_ok!(Network::update_node_bootnode_peer_info(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            current_bootnode_peer_info.clone()
        ));

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(
            subnet_node.bootnode_peer_info,
            current_bootnode_peer_info.clone()
        );
        // assert_eq!(subnet_node.bootnode_peer_info.clone().unwrap().multiaddr, get_multiaddr(Some(subnet_id), Some(subnet_node_id), Some(1)));
        assert_eq!(
            subnet_node.bootnode_peer_info.clone().unwrap().multiaddr,
            current_bootnode_multiaddr
        );

        let bootnode_peer_subnet_node_id =
            BootnodePeerIdSubnetNodeId::<Test>::get(subnet_id, current_bootnode_peer_id.clone());
        assert_eq!(bootnode_peer_subnet_node_id, subnet_node_id);

        let bootnode_multiaddr_subnet_node_id = MultiaddrSubnetNodeId::<Test>::get(
            subnet_id,
            get_multiaddr(Some(subnet_id), Some(subnet_node_id), Some(1)).unwrap(),
        );
        assert_eq!(bootnode_multiaddr_subnet_node_id, subnet_node_id);
    })
}

#[test]
fn test_update_bootnode_peer_id_exists_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end).unwrap();

        let subnet_node_id = end;

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);

        let current_bootnode_peer_id = subnet_node.bootnode_peer_info.clone().unwrap().peer_id;

        let someone_elses_bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end - 1);

        let someone_elses_peer_info = Some(PeerInfo {
            peer_id: someone_elses_bootnode_peer_id,
            multiaddr: None,
        });

        assert_err!(
            Network::update_node_bootnode_peer_info(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                someone_elses_peer_info
            ),
            Error::<Test>::PeerIdExist
        );

        let current_peer_info = Some(PeerInfo {
            peer_id: current_bootnode_peer_id,
            multiaddr: None,
        });

        // --- fail if same peer id
        assert_err!(
            Network::update_node_bootnode_peer_info(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                current_peer_info
            ),
            Error::<Test>::PeerIdExist
        );
    })
}

#[test]
fn test_update_bootnode_peer_id_not_key_owner_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end).unwrap();

        let subnet_node_id = end;

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);

        let new_peer_info = Some(PeerInfo {
            peer_id: peer(1),
            multiaddr: None,
        });

        assert_err!(
            Network::update_node_bootnode_peer_info(
                RuntimeOrigin::signed(account(2)),
                subnet_id,
                subnet_node_id,
                new_peer_info
            ),
            Error::<Test>::NotKeyOwner
        );
    })
}

#[test]
fn test_update_bootnode_peer_id_invalid_peer_id_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end).unwrap();

        let subnet_node_id = end;

        let bootnode_peer_id = format!("2");

        // let bad_bootnode_peer: PeerId = PeerId(bootnode_peer_id.clone().into());

        let new_peer_info = Some(PeerInfo {
            peer_id: PeerId(bootnode_peer_id.clone().into()),
            multiaddr: None,
        });

        assert_err!(
            Network::update_node_bootnode_peer_info(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                new_peer_info
            ),
            Error::<Test>::InvalidPeerId
        );
    })
}

#[test]
fn test_update_client_peer_id_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let subnet_node_id = end;
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end).unwrap();

        // current peer id
        let current_peer_info = subnet_node.client_peer_info.clone();
        let current_client_peer_id = subnet_node.client_peer_info.unwrap().peer_id;

        // new and unused peer id
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);

        let new_peer_info = Some(PeerInfo {
            peer_id: client_peer_id.clone(),
            multiaddr: None,
        });

        assert_ok!(Network::update_node_client_peer_info(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            new_peer_info.clone()
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetNodeUpdateClientPeerInfo {
                subnet_id,
                subnet_node_id,
                client_peer_info: new_peer_info.clone()
            }
        );

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(
            subnet_node.client_peer_info.clone().unwrap().peer_id,
            new_peer_info.clone().unwrap().peer_id
        );
        assert_ne!(
            subnet_node.client_peer_info.clone().unwrap().peer_id,
            current_client_peer_id
        );

        let client_peer_subnet_node_id =
            ClientPeerIdSubnetNodeId::<Test>::get(subnet_id, client_peer_id.clone());
        assert_eq!(client_peer_subnet_node_id, subnet_node_id);

        assert_eq!(
            ClientPeerIdSubnetNodeId::<Test>::try_get(subnet_id, &current_client_peer_id),
            Err(())
        );

        let prev_client_peer_subnet_node_id =
            ClientPeerIdSubnetNodeId::<Test>::get(subnet_id, &current_client_peer_id);
        assert_ne!(prev_client_peer_subnet_node_id, subnet_node_id);

        // test using previous peer id under a diff subnet node
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end - 1).unwrap();
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end - 1);

        let subnet_node_id = end - 1;

        let new_peer_info = Some(PeerInfo {
            peer_id: client_peer_id.clone(),
            multiaddr: None,
        });

        assert_ok!(Network::update_node_client_peer_info(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            current_peer_info.clone()
        ));

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(
            subnet_node.client_peer_info.unwrap().peer_id,
            current_client_peer_id.clone()
        );

        let client_peer_subnet_node_id =
            ClientPeerIdSubnetNodeId::<Test>::get(subnet_id, current_client_peer_id.clone());
        assert_eq!(client_peer_subnet_node_id, subnet_node_id);
    })
}

#[test]
fn test_update_client_peer_id_exists_v2() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end).unwrap();

        let subnet_node_id = end;

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);

        let current_peer_info = subnet_node.client_peer_info.clone();
        let current_client_peer_id = subnet_node.client_peer_info.unwrap().peer_id;

        // let peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end - 1);

        let new_peer_info = Some(PeerInfo {
            peer_id: get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end - 1),
            multiaddr: None,
        });

        assert_err!(
            Network::update_node_client_peer_info(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                new_peer_info
            ),
            Error::<Test>::PeerIdExist
        );

        // --- fail if same peer id
        assert_err!(
            Network::update_node_client_peer_info(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                current_peer_info.clone()
            ),
            Error::<Test>::PeerIdExist
        );
    })
}

#[test]
fn test_update_client_peer_id_not_key_owner() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end).unwrap();

        let subnet_node_id = end;

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);

        let current_client_peer_id = subnet_node.client_peer_info.unwrap().peer_id;
        let new_peer_info = Some(PeerInfo {
            peer_id: current_client_peer_id,
            multiaddr: None,
        });

        assert_err!(
            Network::update_node_client_peer_info(
                RuntimeOrigin::signed(account(2)),
                subnet_id,
                subnet_node_id,
                new_peer_info
            ),
            Error::<Test>::NotKeyOwner
        );
    })
}

#[test]
fn test_update_client_peer_id_invalid_peer_id() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end).unwrap();

        let subnet_node_id = end;

        let client_peer_id = format!("2");

        let bad_client_peer: PeerId = PeerId(client_peer_id.clone().into());
        let new_peer_info = Some(PeerInfo {
            peer_id: bad_client_peer,
            multiaddr: None,
        });

        assert_err!(
            Network::update_node_client_peer_info(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                new_peer_info
            ),
            Error::<Test>::InvalidPeerId
        );
    })
}

#[test]
fn subnet_stake_multiplier_works() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;

        // Set test constants
        MinSubnetNodes::<Test>::put(10);
        MaxSubnetNodes::<Test>::put(100);
        TotalActiveSubnetNodes::<Test>::insert(subnet_id, 10);

        // Multiplier should be 100% at min
        let mult = Network::get_subnet_min_delegate_staking_multiplier(10);
        assert_eq!(mult, Network::percentage_factor_as_u128()); // 100%

        // Multiplier should be 400% at max
        TotalActiveSubnetNodes::<Test>::insert(subnet_id, 100);
        let mult = Network::get_subnet_min_delegate_staking_multiplier(100);
        assert_eq!(mult, 4000000000000000000); // 400%

        // Multiplier should be ~250% halfway
        TotalActiveSubnetNodes::<Test>::insert(subnet_id, 55); // halfway between 10 and 100
        let mult = Network::get_subnet_min_delegate_staking_multiplier(55);
        let expected = Network::percentage_factor_as_u128() + (3000000000000000000 / 2);
        assert_eq!(mult, expected);
    });
}

#[test]
fn test_clean_validator_subnet_nodes() {
    new_test_ext().execute_with(|| {
        insert_subnet(1, SubnetState::Active, 0);
        insert_subnet(2, SubnetState::Active, 0);

        let validator_id = 1;
        // Seed data
        let mut subnet_nodes: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();

        // Subnet 1: Valid subnet with valid and invalid nodes
        manual_insert_subnet_node_v2(
            validator_id,
            1,
            100, // node id
            1,   // coldkey
            2,   // hotkey
            2,   // peer
            SubnetNodeClass::Validator,
            0,
        );
        let mut node_ids1 = BTreeSet::new();
        node_ids1.insert(100); // Valid node
                               // insert nodes for subnet 1
        subnet_nodes.insert(1, node_ids1);

        // Subnet 2: Valid subnet with only valid nodes
        manual_insert_subnet_node_v2(
            validator_id,
            2,
            200, // node id
            1,   // coldkey
            3,   // hotkey
            3,   // peer
            SubnetNodeClass::Validator,
            0,
        );
        let mut node_ids2 = BTreeSet::new();
        node_ids2.insert(200); // Valid node
                               // insert nodes for subnet 2
        subnet_nodes.insert(2, node_ids2);

        // Subnet 3: Invalid subnet
        let mut node_ids3 = BTreeSet::new();
        node_ids3.insert(300); // Valid node
        node_ids3.insert(301); // Invalid node
                               // insert nodes for subnet 3
        subnet_nodes.insert(3, node_ids3);

        // Insert seed data into storage
        ValidatorSubnetNodes::<Test>::insert(validator_id, subnet_nodes);

        // Verify initial state
        let initial = ValidatorSubnetNodes::<Test>::get(validator_id);
        assert_eq!(initial.len(), 3);
        assert_eq!(initial.get(&1).unwrap().len(), 1);
        assert_eq!(initial.get(&2).unwrap().len(), 1);

        // Subnet doesn't exist, both will be removed later
        assert_eq!(initial.get(&3).unwrap().len(), 2);

        // Call the function to clean invalid subnets and nodes
        Network::clean_validator_subnet_nodes(validator_id);

        // Verify final state
        let final_state = ValidatorSubnetNodes::<Test>::get(validator_id);
        log::error!("final_state {:?}", final_state);

        assert_eq!(final_state.len(), 2, "Invalid subnet 3 should be removed");

        assert_eq!(
            final_state.get(&1).unwrap().len(),
            1,
            "Invalid node 101 should be removed from subnet 1"
        );

        assert!(final_state.get(&1).unwrap().contains(&100));
        assert!(final_state.get(&1).unwrap().contains(&101) == false);

        assert_eq!(
            final_state.get(&2).unwrap().len(),
            1,
            "Subnet 2 should remain unchanged"
        );
        assert!(final_state.get(&2).unwrap().contains(&200));
        assert!(final_state.get(&3).is_none(), "Subnet 3 should be gone");
    })
}

#[test]
fn test_update_node_unique() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let subnet_node_id = end;

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);

        let unique: Vec<u8> = "a".into();
        let bounded_unique: BoundedVec<u8, DefaultMaxVectorLength> =
            unique.try_into().expect("String too long");

        // sanity check
        assert_eq!(
            UniqueParamSubnetNodeId::<Test>::try_get(subnet_id, &bounded_unique),
            Err(())
        );

        // update unique parameter
        assert_ok!(Network::update_node_unique(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            Some(bounded_unique.clone())
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetNodeUpdateUnique {
                subnet_id,
                subnet_node_id,
                unique: Some(bounded_unique.clone())
            }
        );

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.unique, Some(bounded_unique.clone()));
        let unique_owner_id = UniqueParamSubnetNodeId::<Test>::get(subnet_id, &bounded_unique);
        assert_eq!(subnet_node_id, unique_owner_id);

        // Allow same parameter if owner
        assert_ok!(Network::update_node_unique(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            Some(bounded_unique.clone())
        ));

        // Shouldn't allow same parameter unless owner
        let coldkey = get_coldkey(subnets, max_subnet_nodes, end - 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end - 1);

        assert_err!(
            Network::update_node_unique(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                Some(bounded_unique.clone())
            ),
            Error::<Test>::NotKeyOwner
        );

        let subnet_node_id = end - 1;

        assert_err!(
            Network::update_node_unique(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                Some(bounded_unique.clone())
            ),
            Error::<Test>::UniqueParameterTaken
        );

        // back to original node and update to a new value
        let coldkey = get_coldkey(subnets, max_subnet_nodes, end);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);
        let subnet_node_id = end;

        let new_unique: Vec<u8> = "new".into();
        let new_bounded_unique: BoundedVec<u8, DefaultMaxVectorLength> =
            new_unique.try_into().expect("String too long");

        assert_ok!(Network::update_node_unique(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            Some(new_bounded_unique.clone())
        ));

        // ensure old deletes
        assert_eq!(
            UniqueParamSubnetNodeId::<Test>::try_get(subnet_id, &bounded_unique),
            Err(())
        );

        // new
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.unique, Some(new_bounded_unique.clone()));
        let unique_owner_id = UniqueParamSubnetNodeId::<Test>::get(subnet_id, &new_bounded_unique);
        assert_eq!(subnet_node_id, unique_owner_id);
    })
}

#[test]
fn test_update_unique_to_none() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end).unwrap();

        let subnet_node_id = end;

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);

        let unique: Vec<u8> = "a".into();
        let bounded_unique: BoundedVec<u8, DefaultMaxVectorLength> =
            unique.try_into().expect("String too long");

        // sanity check
        assert_eq!(
            UniqueParamSubnetNodeId::<Test>::try_get(subnet_id, &bounded_unique),
            Err(())
        );

        // update unique parameter
        assert_ok!(Network::update_node_unique(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            Some(bounded_unique.clone())
        ));

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.unique, Some(bounded_unique.clone()));
        let unique_owner_id = UniqueParamSubnetNodeId::<Test>::get(subnet_id, &bounded_unique);
        assert_eq!(subnet_node_id, unique_owner_id);

        // Allow same parameter if owner
        assert_ok!(Network::update_node_unique(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            Some(bounded_unique.clone())
        ));

        assert_ok!(Network::update_node_unique(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            None
        ));
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.unique, None);
        // Old unique parameter should be removed from storage
        assert_eq!(
            UniqueParamSubnetNodeId::<Test>::try_get(subnet_id, &bounded_unique),
            Err(())
        );
    })
}

#[test]
fn test_update_node_non_unique() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end).unwrap();

        let subnet_node_id = end;

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);

        let non_unique: Vec<u8> = "a".into();
        let bounded_non_unique: BoundedVec<u8, DefaultMaxVectorLength> =
            non_unique.try_into().expect("String too long");

        assert_ok!(Network::update_node_non_unique(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            Some(bounded_non_unique.clone())
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetNodeUpdateNonUnique {
                subnet_id,
                subnet_node_id,
                non_unique: Some(bounded_non_unique.clone())
            }
        );

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.non_unique, Some(bounded_non_unique.clone()));

        assert_err!(
            Network::update_node_non_unique(
                RuntimeOrigin::signed(account(999)),
                subnet_id,
                subnet_node_id,
                Some(bounded_non_unique.clone())
            ),
            Error::<Test>::NotKeyOwner
        );
    })
}

#[test]
fn test_update_node_non_unique_to_none() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = 3;

        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, end).unwrap();

        let subnet_node_id = end;

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);

        let non_unique: Vec<u8> = "a".into();
        let bounded_non_unique: BoundedVec<u8, DefaultMaxVectorLength> =
            non_unique.try_into().expect("String too long");

        assert_ok!(Network::update_node_non_unique(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            Some(bounded_non_unique.clone())
        ));

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.non_unique, Some(bounded_non_unique.clone()));

        assert_ok!(Network::update_node_non_unique(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            None
        ));

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.non_unique, None);
    })
}

#[test]
fn test_insert_node_into_election_slot() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();

        let end = MinSubnetNodes::<Test>::get();

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let coldkey = get_coldkey(subnet_id, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnet_id, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnet_id, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnet_id, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnet_id, max_subnet_nodes, max_subnets, end + 1);

        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), amount + burn_amount + 500);

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            subnet_id,
            None,
            PeerInfo {
                peer_id: peer_id.clone(),
                multiaddr: None,
            },
            None,
            None,
            stake_amount,
            None,
            None,
            u128::MAX,
        ));

        let subnet_node_id = end + 1;

        // Get initial counts for comparison
        let initial_subnet_count = TotalSubnetElectableNodes::<Test>::get(subnet_id);
        let initial_total_count = TotalElectableNodes::<Test>::get();
        let initial_slot_list = SubnetNodeElectionSlots::<Test>::get(subnet_id);

        let result = Network::insert_node_into_election_slot(subnet_id, subnet_node_id);
        assert!(result);

        // Check all storage elements were updated correctly:

        // 1. Node should be in the slot list
        let updated_slot_list = SubnetNodeElectionSlots::<Test>::get(subnet_id);
        assert!(
            updated_slot_list.contains(&subnet_node_id),
            "Node {} should be in subnet {} slot list",
            subnet_node_id,
            subnet_id
        );

        // 2. NodeSlotIndex should have the correct index
        let expected_index = initial_slot_list.len() as u32;
        let stored_index = NodeSlotIndex::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(
            stored_index,
            Some(expected_index),
            "NodeSlotIndex should store correct index {} for node {}",
            expected_index,
            subnet_node_id
        );

        // 3. TotalSubnetElectableNodes should increment by 1
        let updated_subnet_count = TotalSubnetElectableNodes::<Test>::get(subnet_id);
        assert_eq!(
            updated_subnet_count,
            initial_subnet_count.saturating_add(1),
            "TotalSubnetElectableNodes should increment by 1"
        );

        // 4. TotalElectableNodes should increment by 1
        let updated_total_count = TotalElectableNodes::<Test>::get();
        assert_eq!(
            updated_total_count,
            initial_total_count.saturating_add(1),
            "TotalElectableNodes should increment by 1"
        );

        // 5. Test duplicate insertion returns false
        let duplicate_result = Network::insert_node_into_election_slot(subnet_id, subnet_node_id);
        assert_eq!(
            duplicate_result, false,
            "Inserting duplicate node should return false"
        );

        // 6. Verify no additional changes on duplicate
        assert_eq!(
            SubnetNodeElectionSlots::<Test>::get(subnet_id).len(),
            updated_slot_list.len(),
            "Slot list length should not change on duplicate insertion"
        );
    })
}

#[test]
fn test_remove_node_from_election_slot() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1u32;
        let node_to_remove = 42u32;
        let other_node_1 = 10u32;
        let other_node_2 = 20u32;

        // Setup: Insert multiple nodes first
        Network::insert_node_into_election_slot(subnet_id, other_node_1); // index 0
        Network::insert_node_into_election_slot(subnet_id, node_to_remove); // index 1
        Network::insert_node_into_election_slot(subnet_id, other_node_2); // index 2

        // Get initial state
        let initial_subnet_count = TotalSubnetElectableNodes::<Test>::get(subnet_id);
        let initial_total_count = TotalElectableNodes::<Test>::get();
        let initial_slot_list = SubnetNodeElectionSlots::<Test>::get(subnet_id);
        let initial_list_len = initial_slot_list.len();

        // The node that will be moved (last node in list)
        let node_that_will_move = initial_slot_list[initial_list_len - 1];
        let remove_position = initial_slot_list
            .iter()
            .position(|&id| id == node_to_remove)
            .unwrap();

        // Call the removal function
        let result = Network::remove_node_from_election_slot(subnet_id, node_to_remove);

        // Verify the function returned true
        assert_eq!(
            result, true,
            "Function should return true for successful removal"
        );

        // Check all storage elements were updated correctly:

        // 1. Node should NOT be in the slot list anymore
        let updated_slot_list = SubnetNodeElectionSlots::<Test>::get(subnet_id);
        assert!(
            !updated_slot_list.contains(&node_to_remove),
            "Node {} should NOT be in subnet {} slot list after removal",
            node_to_remove,
            subnet_id
        );

        // 2. Slot list should be 1 shorter
        assert_eq!(
            updated_slot_list.len(),
            initial_list_len - 1,
            "Slot list should be 1 element shorter"
        );

        // 3. NodeSlotIndex for removed node should be gone
        let removed_node_index = NodeSlotIndex::<Test>::get(subnet_id, node_to_remove);
        assert_eq!(
            removed_node_index, None,
            "NodeSlotIndex should be None for removed node {}",
            node_to_remove
        );

        // 4. If a node was moved (swap_remove), verify its index was updated
        if remove_position != initial_list_len - 1 {
            // A node was moved to fill the gap
            let moved_node_new_index = NodeSlotIndex::<Test>::get(subnet_id, node_that_will_move);
            assert_eq!(
                moved_node_new_index,
                Some(remove_position as u32),
                "Moved node {} should have updated index {}",
                node_that_will_move,
                remove_position
            );

            // Verify the moved node is at the correct position in the list
            assert_eq!(
                updated_slot_list[remove_position], node_that_will_move,
                "Moved node should be at position {} in slot list",
                remove_position
            );
        }

        // 5. TotalSubnetElectableNodes should decrement by 1
        let updated_subnet_count = TotalSubnetElectableNodes::<Test>::get(subnet_id);
        assert_eq!(
            updated_subnet_count,
            initial_subnet_count.saturating_sub(1),
            "TotalSubnetElectableNodes should decrement by 1"
        );

        // 6. TotalElectableNodes should decrement by 1
        let updated_total_count = TotalElectableNodes::<Test>::get();
        assert_eq!(
            updated_total_count,
            initial_total_count.saturating_sub(1),
            "TotalElectableNodes should decrement by 1"
        );

        // 7. Test removing non-existent node returns false
        let non_existent_result = Network::remove_node_from_election_slot(subnet_id, 999u32);
        assert_eq!(
            non_existent_result, false,
            "Removing non-existent node should return false"
        );

        // 8. Verify no changes when removing non-existent node
        assert_eq!(
            SubnetNodeElectionSlots::<Test>::get(subnet_id).len(),
            updated_slot_list.len(),
            "Slot list length should not change when removing non-existent node"
        );
    })
}

// Helper function to simulate multiple registrations
fn simulate_registrations(subnet_id: u32, count: u32) -> Vec<u128> {
    let mut burn_amounts = Vec::new();
    for _ in 0..count {
        let burn = Network::calculate_burn_amount(subnet_id);
        burn_amounts.push(burn);
        assert_ok!(Network::record_registration(subnet_id));
    }
    burn_amounts
}

#[test]
fn test_low_registration_volume_decreases_burn_rate() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let initial_burn = Network::calculate_burn_amount(subnet_id);
        let initial_rate = CurrentNodeBurnRate::<Test>::get(subnet_id);

        // Epoch 1: Low volume (10 registrations)
        simulate_registrations(subnet_id, 10);
        Network::update_burn_rate_for_epoch(&mut WeightMeter::new(), subnet_id);
        let epoch1_burn = Network::calculate_burn_amount(subnet_id);
        let epoch1_rate = CurrentNodeBurnRate::<Test>::get(subnet_id);

        // Epoch 2: Very low volume (5 registrations)
        simulate_registrations(subnet_id, 5);
        Network::update_burn_rate_for_epoch(&mut WeightMeter::new(), subnet_id);
        let epoch2_burn = Network::calculate_burn_amount(subnet_id);
        let epoch2_rate = CurrentNodeBurnRate::<Test>::get(subnet_id);

        // Epoch 3: No registrations
        // Rate is still based on Epoch 2's activity (5 registrations)
        Network::update_burn_rate_for_epoch(&mut WeightMeter::new(), subnet_id);
        let epoch3_burn = Network::calculate_burn_amount(subnet_id);
        let epoch3_rate = CurrentNodeBurnRate::<Test>::get(subnet_id);

        // Epoch 4: This is where you see the effect of Epoch 3's zero registrations
        Network::update_burn_rate_for_epoch(&mut WeightMeter::new(), subnet_id);
        let epoch4_burn = Network::calculate_burn_amount(subnet_id);
        let epoch4_rate = CurrentNodeBurnRate::<Test>::get(subnet_id);

        // Epoch 5: Very low volume (5 registrations)
        simulate_registrations(subnet_id, 5);
        Network::update_burn_rate_for_epoch(&mut WeightMeter::new(), subnet_id);
        let epoch5_burn = Network::calculate_burn_amount(subnet_id);
        let epoch5_rate = CurrentNodeBurnRate::<Test>::get(subnet_id);

        // Epoch 6: Very low volume (5 registrations)
        simulate_registrations(subnet_id, 10);
        Network::update_burn_rate_for_epoch(&mut WeightMeter::new(), subnet_id);
        let epoch6_burn = Network::calculate_burn_amount(subnet_id);
        let epoch6_rate = CurrentNodeBurnRate::<Test>::get(subnet_id);

        // Epoch 7: Very low volume (1 registrations)
        simulate_registrations(subnet_id, 1);
        Network::update_burn_rate_for_epoch(&mut WeightMeter::new(), subnet_id);
        let epoch7_burn = Network::calculate_burn_amount(subnet_id);
        let epoch7_rate = CurrentNodeBurnRate::<Test>::get(subnet_id);

        log::error!("Initial rate: {:?}, burn: {:?}", initial_rate, initial_burn);
        log::error!(
            "Epoch1 rate:  {:?}, burn: {:?} (based on initial)",
            epoch1_rate,
            epoch1_burn
        );
        log::error!(
            "Epoch2 rate:  {:?}, burn: {:?} (based on 10 reg)",
            epoch2_rate,
            epoch2_burn
        );
        log::error!(
            "Epoch3 rate:  {:?}, burn: {:?} (based on 5 reg)",
            epoch3_rate,
            epoch3_burn
        );
        log::error!(
            "Epoch4 rate:  {:?}, burn: {:?} (based on 0 reg)",
            epoch4_rate,
            epoch4_burn
        );
        log::error!(
            "Epoch5 rate:  {:?}, burn: {:?} (based on 5 reg)",
            epoch5_rate,
            epoch5_burn
        );
        log::error!(
            "Epoch6 rate:  {:?}, burn: {:?} (based on 10 reg)",
            epoch6_rate,
            epoch6_burn
        );
        log::error!(
            "Epoch7 rate:  {:?}, burn: {:?} (based on 1 reg)",
            epoch7_rate,
            epoch7_burn
        );

        // CORRECT EXPECTATIONS for reactive design:
        // Epoch 1 rate is based on initial state
        // Epoch 2 rate should decrease (based on 10 registrations vs initial high rate)
        // Epoch 3 rate should decrease more (based on 5 registrations)
        // Epoch 4 rate should decrease most (based on 0 registrations)

        assert!(
            epoch2_rate <= epoch1_rate,
            "Epoch 2 should be <= than Epoch 1"
        );
        assert!(
            epoch3_rate < epoch2_rate,
            "Epoch 3 should be greater than Epoch 2"
        );
        assert!(
            epoch4_rate < epoch3_rate,
            "Epoch 4 should be lowest (zero registrations effect)"
        );
        assert!(
            epoch5_rate > epoch4_rate,
            "Epoch 5 should be greater than Epoch 4"
        );
        assert!(
            epoch6_rate > epoch5_rate,
            "Epoch 6 should be greater than Epoch 5"
        );
        assert!(
            epoch7_rate < epoch6_rate,
            "Epoch 7 should be less than Epoch 6"
        );

        assert!(epoch2_burn <= epoch1_burn);
        assert!(epoch3_burn < epoch2_burn);
        assert!(epoch4_burn < epoch3_burn);
    });
}

#[test]
fn test_register_subnet_node_initial_coldkeys_max_registered() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let max_subnets = MaxSubnets::<Test>::get();
        // let subnets = TotalActiveSubnets::<Test>::get() + 1;

        let subnets = TotalSubnetUids::<Test>::get() + 1;
        let subnet_id_key_offset = get_subnet_id_key_offset(subnets);

        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 4;

        let add_subnet_data = default_registration_subnet_data_v2(
            subnet_id_key_offset,
            max_subnet_nodes,
            subnet_name.clone().into(),
            0,
            end + 1,
        );

        build_registered_subnet_v2(
            subnet_name.clone(),
            0,
            4,
            deposit_amount,
            stake_amount,
            true,
            Some(add_subnet_data),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let coldkey = get_coldkey(subnet_id_key_offset, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnet_id, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnet_id_key_offset, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnet_id_key_offset, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id =
            get_client_peer_id(subnet_id_key_offset, max_subnet_nodes, max_subnets, end + 1);

        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount);
        let starting_balance = Balances::free_balance(&coldkey.clone());

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            50000000000,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            subnet_id,
            None,
            PeerInfo {
                peer_id: peer_id.clone(),
                multiaddr: None,
            },
            None,
            None,
            amount,
            None,
            None,
            u128::MAX,
        ));

        let peer_id = get_peer_id(subnet_id_key_offset, max_subnet_nodes, max_subnets, end + 2);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnet_id_key_offset, max_subnet_nodes, max_subnets, end + 2);
        let client_peer_id =
            get_client_peer_id(subnet_id_key_offset, max_subnet_nodes, max_subnets, end + 2);

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: peer_id.clone(),
                    multiaddr: None,
                },
                None,
                None,
                amount,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::MaxRegisteredNodes
        );
    });
}

#[test]
fn test_do_activate_subnet_node_subnet_active_node_queued() {
    new_test_ext().execute_with(|| {
        // Subnet is active
        // Node is queued
        let validator_id = 1;
        let subnet_id = 1;
        let subnet_node_id = 1;
        let hotkey = account(1);
        let peer_id = peer(1);
        let bootnode_peer_id = peer(2);
        let client_peer_id = peer(3);

        manual_insert_validator(validator_id, 1, 1);

        let classification = SubnetNodeClassification {
            node_class: SubnetNodeClass::Registered,
            start_epoch: 0,
        };

        let subnet_node = SubnetNodeV2 {
            id: subnet_node_id,
            validator_id: validator_id,
            peer_info: PeerInfo {
                peer_id: peer_id,
                multiaddr: None,
            },
            bootnode_peer_info: None,
            client_peer_info: None,
            classification: classification,
            unique: None,
            non_unique: None,
        };

        RegisteredSubnetNodesData::<Test>::insert(subnet_id, subnet_node_id, &subnet_node);

        // Starting values
        let initial_active_subnet_nodes = TotalActiveSubnetNodes::<Test>::get(subnet_id);
        let initial_active_nodes = TotalActiveNodes::<Test>::get();
        let coldkey_rep = ValidatorReputation::<Test>::get(validator_id);
        let lifetime_node_count = coldkey_rep.lifetime_node_count;
        let total_active_nodes = coldkey_rep.total_active_nodes;

        // Queue is true
        assert!(Network::do_activate_subnet_node(
            &mut WeightMeter::new(),
            validator_id,
            subnet_id,
            SubnetState::Active,
            subnet_node,
            Network::get_current_subnet_epoch_as_u32(subnet_id),
            true,
        ));

        assert_eq!(
            RegisteredSubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
            Err(())
        );
        assert!(SubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id).is_ok());
        assert_eq!(
            SubnetNodesData::<Test>::get(subnet_id, subnet_node_id)
                .classification
                .node_class,
            SubnetNodeClass::Idle
        );

        assert_eq!(
            initial_active_subnet_nodes + 1,
            TotalActiveSubnetNodes::<Test>::get(subnet_id)
        );
        assert_eq!(initial_active_nodes + 1, TotalActiveNodes::<Test>::get());

        assert_eq!(
            lifetime_node_count + 1,
            ValidatorReputation::<Test>::get(validator_id).lifetime_node_count
        );
        assert_eq!(
            total_active_nodes + 1,
            ValidatorReputation::<Test>::get(validator_id).total_active_nodes
        );
    });
}

#[test]
fn test_do_activate_subnet_node_failures() {
    new_test_ext().execute_with(|| {
        // Subnet is registered
        // Node is queued
        let subnet_id = 1;
        let subnet_node_id = 1;
        let hotkey = account(1);
        let peer_id = peer(1);
        let bootnode_peer_id = peer(2);
        let client_peer_id = peer(3);
        let validator_id = 1;

        manual_insert_validator(validator_id, 1, 1);

        let classification = SubnetNodeClassification {
            node_class: SubnetNodeClass::Registered,
            start_epoch: 0,
        };

        let subnet_node = SubnetNodeV2 {
            id: subnet_node_id,
            validator_id: validator_id,
            peer_info: PeerInfo {
                peer_id: peer_id,
                multiaddr: None,
            },
            bootnode_peer_info: None,
            client_peer_info: None,
            classification: classification,
            unique: None,
            non_unique: None,
        };

        RegisteredSubnetNodesData::<Test>::insert(subnet_id, subnet_node_id, &subnet_node);

        // Starting values
        let initial_active_subnet_nodes = TotalActiveSubnetNodes::<Test>::get(subnet_id);
        let initial_active_nodes = TotalActiveNodes::<Test>::get();
        let coldkey_rep = ValidatorReputation::<Test>::get(validator_id);
        let lifetime_node_count = coldkey_rep.lifetime_node_count;
        let total_active_nodes = coldkey_rep.total_active_nodes;

        assert!(!Network::do_activate_subnet_node(
            &mut WeightMeter::new(),
            validator_id,
            subnet_id,
            SubnetState::Registered,
            subnet_node.clone(),
            Network::get_current_subnet_epoch_as_u32(subnet_id),
            true,
        ));

        assert!(!Network::do_activate_subnet_node(
            &mut WeightMeter::new(),
            validator_id,
            subnet_id,
            SubnetState::Active,
            subnet_node.clone(),
            Network::get_current_subnet_epoch_as_u32(subnet_id),
            false,
        ));

        assert!(!Network::do_activate_subnet_node(
            &mut WeightMeter::new(),
            validator_id,
            subnet_id,
            SubnetState::Paused,
            subnet_node.clone(),
            Network::get_current_subnet_epoch_as_u32(subnet_id),
            true,
        ));

        assert!(!Network::do_activate_subnet_node(
            &mut WeightMeter::new(),
            validator_id,
            subnet_id,
            SubnetState::Paused,
            subnet_node.clone(),
            Network::get_current_subnet_epoch_as_u32(subnet_id),
            false,
        ));

        // Nothing should change
        assert!(RegisteredSubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id).is_ok(),);
        assert_eq!(
            SubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
            Err(())
        );

        assert_eq!(
            initial_active_subnet_nodes,
            TotalActiveSubnetNodes::<Test>::get(subnet_id)
        );
        assert_eq!(initial_active_nodes, TotalActiveNodes::<Test>::get());

        assert_eq!(
            lifetime_node_count,
            ValidatorReputation::<Test>::get(validator_id).lifetime_node_count
        );
        assert_eq!(
            total_active_nodes,
            ValidatorReputation::<Test>::get(validator_id).total_active_nodes
        );
    });
}

#[test]
fn test_do_activate_subnet_node_registered_subnet() {
    new_test_ext().execute_with(|| {
        // Subnet is registered
        // Node is queued

        let subnet_id = 1;
        let subnet_node_id = 1;
        let hotkey = account(1);
        let peer_id = peer(1);
        let bootnode_peer_id = peer(2);
        let validator_id = 1;

        manual_insert_validator(validator_id, 1, 1);

        let client_peer_id = peer(3);
        let classification = SubnetNodeClassification {
            node_class: SubnetNodeClass::Registered,
            start_epoch: 0,
        };

        let subnet_node = SubnetNodeV2 {
            id: subnet_node_id,
            validator_id: validator_id,
            peer_info: PeerInfo {
                peer_id: peer_id,
                multiaddr: None,
            },
            bootnode_peer_info: None,
            client_peer_info: None,
            classification: classification,
            unique: None,
            non_unique: None,
        };

        // Starting values
        let initial_active_subnet_nodes = TotalActiveSubnetNodes::<Test>::get(subnet_id);
        let initial_active_nodes = TotalActiveNodes::<Test>::get();
        let coldkey_rep = ValidatorReputation::<Test>::get(validator_id);
        let lifetime_node_count = coldkey_rep.lifetime_node_count;
        let total_active_nodes = coldkey_rep.total_active_nodes;

        assert!(Network::do_activate_subnet_node(
            &mut WeightMeter::new(),
            validator_id,
            subnet_id,
            SubnetState::Registered,
            subnet_node,
            Network::get_current_subnet_epoch_as_u32(subnet_id),
            false,
        ));

        assert_eq!(
            RegisteredSubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
            Err(())
        );
        assert!(SubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id).is_ok());
        assert_eq!(
            SubnetNodesData::<Test>::get(subnet_id, subnet_node_id)
                .classification
                .node_class,
            SubnetNodeClass::Validator
        );

        assert_eq!(
            initial_active_subnet_nodes + 1,
            TotalActiveSubnetNodes::<Test>::get(subnet_id)
        );
        assert_eq!(initial_active_nodes + 1, TotalActiveNodes::<Test>::get());

        assert_eq!(
            lifetime_node_count + 1,
            ValidatorReputation::<Test>::get(validator_id).lifetime_node_count
        );
        assert_eq!(
            total_active_nodes + 1,
            ValidatorReputation::<Test>::get(validator_id).total_active_nodes
        );
    });
}

#[test]
fn test_slash_validator() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);

        let subnet_node_id = end;
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(
            subnet_node.classification.node_class,
            SubnetNodeClass::Validator
        );

        let validator_id = SubnetNodeValidatorId::<Test>::get(subnet_id, subnet_node_id).unwrap();

        let starting_node_rep = SubnetNodeReputation::<Test>::get(subnet_id, subnet_node_id);
        let starting_ck_rep = ValidatorReputation::<Test>::get(validator_id).score;

        let starting_account_stake = NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id);
        let starting_total_subnet_stake = TotalSubnetStake::<Test>::get(subnet_id);
        let starting_total_stake = TotalStake::<Test>::get();

        Network::slash_validator(
            subnet_id,
            subnet_node_id,
            500000000000000000, // 50%
            660000000000000000, // 66%
            100000000000000000, // 10%
            100000000000000000, // 10%
            1,
            1,
        );

        assert!(starting_node_rep > SubnetNodeReputation::<Test>::get(subnet_id, subnet_node_id));
        assert!(starting_ck_rep > ValidatorReputation::<Test>::get(validator_id).score);

        assert!(starting_account_stake > NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id));
        assert!(starting_total_subnet_stake > TotalSubnetStake::<Test>::get(subnet_id));
        assert!(starting_total_stake > TotalStake::<Test>::get());
    });
}

// // #[test]
// // fn test_slash_validator_removes_subnet_node() {
// //     new_test_ext().execute_with(|| {
// //         let subnet_name: Vec<u8> = "subnet-name".into();

// //         let deposit_amount: u128 = 10000000000000000000000;
// //         let amount: u128 = 1000000000000000000000;

// //         let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

// //         let subnets = TotalActiveSubnets::<Test>::get() + 1;
// //         let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
// //         let max_subnets = MaxSubnets::<Test>::get();
// //         let end = 4;

// //         build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
// //         let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

// //         let coldkey = get_coldkey(subnets, max_subnet_nodes, end);
// //         let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end);

// //         let subnet_node_id = end;
// //         let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
// //         assert_eq!(subnet_node.classification.node_class, SubnetNodeClass::Validator);

// //         let starting_node_rep = SubnetNodeReputation::<Test>::get(subnet_id, subnet_node_id);
// //         let starting_ck_rep = ValidatorReputation::<Test>::get(validator_id).score;

// //         let starting_account_stake = NodeSubnetStake::<Test>::get(hotkey, subnet_id);
// //         let starting_total_subnet_stake = TotalSubnetStake::<Test>::get(subnet_id);
// //         let starting_total_stake = TotalStake::<Test>::get();

// //         assert_ok!(Network::remove_subnet_node(
// //             RuntimeOrigin::signed(coldkey.clone()),
// //             subnet_id,
// //             subnet_node_id,
// //         ));

// //         Network::slash_validator(
// //             subnet_id,
// //             subnet_node_id,
// //             500000000000000000, // 50%
// //             660000000000000000, // 66%
// //             100000000000000000, // 10%
// //             100000000000000000, // 10%
// //             1,
// //             1,
// //         );

// //         assert!(starting_node_rep > SubnetNodeReputation::<Test>::get(subnet_id, subnet_node_id));
// //         assert!(starting_ck_rep > ValidatorReputation::<Test>::get(validator_id).score);

// //         assert!(starting_account_stake > NodeSubnetStake::<Test>::get(hotkey, subnet_id));
// //         assert!(starting_total_subnet_stake > TotalSubnetStake::<Test>::get(subnet_id));
// //         assert!(starting_total_stake > TotalStake::<Test>::get());
// //     });
// // }

// #[test]
// fn test_build_activated_subnet_v2() {
//     new_test_ext().execute_with(|| {
//         let subnet_name: Vec<u8> = "subnet-name".into();

//         let deposit_amount: u128 = 10000000000000000000000;
//         let amount: u128 = 1000000000000000000000;

//         let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

//         let subnets = TotalActiveSubnets::<Test>::get() + 1;
//         let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
//         let max_subnets = MaxSubnets::<Test>::get();
//         let end = 4;

//         build_activated_subnet_v2(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
//     })
// }
