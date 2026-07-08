use super::mock::*;
use crate::tests::test_utils::*;
use crate::Event;
use crate::{
    BootnodePeerIdSubnetNodeId, ChurnLimit, ClientPeerIdSubnetNodeId, ColdkeyValidatorId, Error,
    MaxSubnetNodes, MaxSubnets, MinSubnetMinStake, MultiaddrSubnetNodeId,
    NodeRegistrationsThisEpoch, NodeSubnetStake, PeerIdSubnetNodeId, PeerInfo,
    RegisteredSubnetNodesData, SubnetName, SubnetNodeClass, SubnetNodeQueue, SubnetNodeQueueEpochs,
    SubnetNodeReputation, SubnetNodeValidatorId, SubnetNodesData, TotalActiveSubnetNodes,
    TotalActiveSubnets, TotalNodes, TotalStake, TotalSubnetNodeUids, TotalSubnetNodes,
    TotalSubnetStake, ValidatorColdkey, ValidatorSubnetNodes,
};
use frame_support::traits::Currency;
use frame_support::traits::ExistenceRequirement;
use frame_support::weights::WeightMeter;
use frame_support::{assert_err, assert_ok};
use sp_std::collections::btree_map::BTreeMap;

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
fn test_register_subnet_node_v2() {
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

        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let queue_epochs = SubnetNodeQueueEpochs::<Test>::get(subnet_id);

        let v_reward_rate = test_percent(1, 20); // 5%

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey.clone(),
            v_reward_rate,
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
            amount,
            None,
            None,
            u128::MAX,
        ));
        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let subnet_node = RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(
            subnet_node.classification.node_class,
            SubnetNodeClass::Registered
        );
        assert_eq!(
            subnet_node.classification.start_epoch,
            subnet_epoch + 1 // subnet_epoch + queue_epochs
        );

        let new_total_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        assert_eq!(total_subnet_nodes + 1, new_total_nodes);

        let reg_queue = SubnetNodeQueue::<Test>::get(subnet_id);
        let found = reg_queue.iter().find(|node| node.id == subnet_node_id);
        assert_eq!(found.unwrap().id, subnet_node_id);

        // assert_eq!(
        //     *network_events().last().unwrap(),
        //     Event::SubnetNodeRegistered {
        //         subnet_id: subnet_id,
        //         subnet_node_id: subnet_node_id,
        //         coldkey: coldkey,
        //         hotkey: hotkey,
        //         data: subnet_node.clone(),
        //     }
        // );
    })
}

#[test]
fn test_register_subnet_node_duplicate_request_peer_does_not_commit_partial_state() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-duplicate-peer".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 10);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 10);
        let duplicate_peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 10);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey, amount + burn_amount + EXISTENTIAL_DEPOSIT);

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            DEFAULT_DELEGATE_REWARD_RATE,
            None,
            None,
        ));
        let validator_id = ColdkeyValidatorId::<Test>::get(&coldkey).unwrap();

        let total_subnet_node_uids = TotalSubnetNodeUids::<Test>::get(subnet_id);
        let next_subnet_node_id = total_subnet_node_uids.saturating_add(1);
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        let total_nodes = TotalNodes::<Test>::get();
        let queue = SubnetNodeQueue::<Test>::get(subnet_id);
        let peer_mapping_exists =
            PeerIdSubnetNodeId::<Test>::contains_key(subnet_id, &duplicate_peer_id);
        let bootnode_mapping_exists =
            BootnodePeerIdSubnetNodeId::<Test>::contains_key(subnet_id, &duplicate_peer_id);
        let client_mapping_exists =
            ClientPeerIdSubnetNodeId::<Test>::contains_key(subnet_id, &duplicate_peer_id);
        let multiaddr_mappings =
            MultiaddrSubnetNodeId::<Test>::iter_prefix(subnet_id).collect::<BTreeMap<_, _>>();
        let registered_node_exists =
            RegisteredSubnetNodesData::<Test>::contains_key(subnet_id, next_subnet_node_id);
        let active_node_exists =
            SubnetNodesData::<Test>::contains_key(subnet_id, next_subnet_node_id);

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                Some(PeerInfo::<Test> {
                    peer_id: duplicate_peer_id.clone(),
                    multiaddr: None,
                }),
                Some(PeerInfo::<Test> {
                    peer_id: duplicate_peer_id.clone(),
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

        assert_eq!(
            TotalSubnetNodeUids::<Test>::get(subnet_id),
            total_subnet_node_uids
        );
        assert_eq!(TotalSubnetNodes::<Test>::get(subnet_id), total_subnet_nodes);
        assert_eq!(TotalNodes::<Test>::get(), total_nodes);
        assert_eq!(SubnetNodeQueue::<Test>::get(subnet_id), queue);
        assert_eq!(
            PeerIdSubnetNodeId::<Test>::contains_key(subnet_id, &duplicate_peer_id),
            peer_mapping_exists
        );
        assert_eq!(
            BootnodePeerIdSubnetNodeId::<Test>::contains_key(subnet_id, &duplicate_peer_id),
            bootnode_mapping_exists
        );
        assert_eq!(
            ClientPeerIdSubnetNodeId::<Test>::contains_key(subnet_id, &duplicate_peer_id),
            client_mapping_exists
        );
        assert_eq!(
            MultiaddrSubnetNodeId::<Test>::iter_prefix(subnet_id).collect::<BTreeMap<_, _>>(),
            multiaddr_mappings
        );
        assert_eq!(
            RegisteredSubnetNodesData::<Test>::contains_key(subnet_id, next_subnet_node_id),
            registered_node_exists
        );
        assert_eq!(
            SubnetNodesData::<Test>::contains_key(subnet_id, next_subnet_node_id),
            active_node_exists
        );
    })
}

#[test]
fn test_register_subnet_node_stake_failure_does_not_commit_partial_state() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-stake-failure".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 20);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 20);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 20);

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            DEFAULT_DELEGATE_REWARD_RATE,
            None,
            None,
        ));
        let validator_id = ColdkeyValidatorId::<Test>::get(&coldkey).unwrap();

        let total_subnet_node_uids = TotalSubnetNodeUids::<Test>::get(subnet_id);
        let next_subnet_node_id = total_subnet_node_uids.saturating_add(1);
        let coldkey_balance = Balances::free_balance(&coldkey);
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        let total_nodes = TotalNodes::<Test>::get();
        let node_registrations = NodeRegistrationsThisEpoch::<Test>::get(subnet_id);
        let queue = SubnetNodeQueue::<Test>::get(subnet_id);
        let total_subnet_stake = TotalSubnetStake::<Test>::get(subnet_id);
        let total_stake = TotalStake::<Test>::get();
        let validator_subnet_nodes = ValidatorSubnetNodes::<Test>::get(validator_id);
        let peer_mapping_exists = PeerIdSubnetNodeId::<Test>::contains_key(subnet_id, &peer_id);
        let registered_node_exists =
            RegisteredSubnetNodesData::<Test>::contains_key(subnet_id, next_subnet_node_id);
        let active_node_exists =
            SubnetNodesData::<Test>::contains_key(subnet_id, next_subnet_node_id);
        let node_validator_exists =
            SubnetNodeValidatorId::<Test>::contains_key(subnet_id, next_subnet_node_id);
        let node_reputation_exists =
            SubnetNodeReputation::<Test>::contains_key(subnet_id, next_subnet_node_id);
        let node_stake = NodeSubnetStake::<Test>::get(next_subnet_node_id, subnet_id);

        assert_err!(
            Network::register_subnet_node(
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
                amount,
                None,
                None,
                u128::MAX,
            ),
            Error::<Test>::NotEnoughBalanceToStake
        );

        assert_eq!(Balances::free_balance(&coldkey), coldkey_balance);
        assert_eq!(
            TotalSubnetNodeUids::<Test>::get(subnet_id),
            total_subnet_node_uids
        );
        assert_eq!(TotalSubnetNodes::<Test>::get(subnet_id), total_subnet_nodes);
        assert_eq!(TotalNodes::<Test>::get(), total_nodes);
        assert_eq!(
            NodeRegistrationsThisEpoch::<Test>::get(subnet_id),
            node_registrations
        );
        assert_eq!(SubnetNodeQueue::<Test>::get(subnet_id), queue);
        assert_eq!(
            PeerIdSubnetNodeId::<Test>::contains_key(subnet_id, &peer_id),
            peer_mapping_exists
        );
        assert_eq!(
            RegisteredSubnetNodesData::<Test>::contains_key(subnet_id, next_subnet_node_id),
            registered_node_exists
        );
        assert_eq!(
            SubnetNodesData::<Test>::contains_key(subnet_id, next_subnet_node_id),
            active_node_exists
        );
        assert_eq!(
            SubnetNodeValidatorId::<Test>::contains_key(subnet_id, next_subnet_node_id),
            node_validator_exists
        );
        assert_eq!(
            SubnetNodeReputation::<Test>::contains_key(subnet_id, next_subnet_node_id),
            node_reputation_exists
        );
        assert_eq!(
            NodeSubnetStake::<Test>::get(next_subnet_node_id, subnet_id),
            node_stake
        );
        assert_eq!(TotalSubnetStake::<Test>::get(subnet_id), total_subnet_stake);
        assert_eq!(TotalStake::<Test>::get(), total_stake);
        assert_eq!(
            ValidatorSubnetNodes::<Test>::get(validator_id),
            validator_subnet_nodes
        );
    })
}

#[test]
fn test_register_subnet_node_v2_and_activate() {
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

        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let queue_epochs = SubnetNodeQueueEpochs::<Test>::get(subnet_id);

        let v_reward_rate = test_percent(1, 20); // 5%

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey.clone(),
            v_reward_rate,
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
            amount,
            None,
            None,
            u128::MAX,
        ));
        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let subnet_node = RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(
            subnet_node.classification.node_class,
            SubnetNodeClass::Registered
        );
        assert_eq!(subnet_node.classification.start_epoch, subnet_epoch + 1);

        let new_total_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        assert_eq!(total_subnet_nodes + 1, new_total_nodes);

        let reg_queue = SubnetNodeQueue::<Test>::get(subnet_id);
        let found = reg_queue.iter().find(|node| node.id == subnet_node_id);
        assert_eq!(found.unwrap().id, subnet_node_id);

        // assert_eq!(
        //     *network_events().last().unwrap(),
        //     Event::SubnetNodeRegistered {
        //         subnet_id: subnet_id,
        //         subnet_node_id: subnet_node_id,
        //         coldkey: coldkey,
        //         hotkey: hotkey,
        //         data: subnet_node.clone(),
        //     }
        // );

        let start_epoch = subnet_node.classification.start_epoch;

        // increase to the nodes start epoch
        set_block_to_subnet_slot_epoch(start_epoch + queue_epochs + 1, subnet_id);

        let epoch = Network::get_current_epoch_as_u32();

        // Get subnet weights (nodes only activate from queue if there are weights)
        // Note: This means a subnet is active if it gets weights
        let _ = Network::handle_subnet_emission_weights(epoch);

        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // Trigger the node activation
        Network::emission_step(
            &mut WeightMeter::new(),
            System::block_number(),
            Network::get_current_epoch_as_u32(),
            Network::get_current_subnet_epoch_as_u32(subnet_id),
            subnet_id,
        );

        // Check out of queue
        assert_eq!(
            RegisteredSubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
            Err(())
        );
        let reg_queue = SubnetNodeQueue::<Test>::get(subnet_id);
        let found = reg_queue.iter().find(|node| node.id == subnet_node_id);
        assert_eq!(found, None);

        // Check in activation
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.classification.node_class, SubnetNodeClass::Idle);
    })
}

#[test]
fn test_register_subnet_node_v2_and_activate_max_churn_limit() {
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

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let queue_epochs = SubnetNodeQueueEpochs::<Test>::get(subnet_id);
        let churn_limit = ChurnLimit::<Test>::get(subnet_id);
        let prev_active_total_nodes = TotalActiveSubnetNodes::<Test>::get(subnet_id);

        let reg_start = end;
        let reg_end = reg_start + churn_limit * 2;
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        // Put a bunch of nodes into the queue
        for n in reg_start..reg_end {
            let _n = n + 1;
            let coldkey = get_coldkey(subnets, max_subnet_nodes, _n);
            let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, _n);

            let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, _n);
            let bootnode_peer_id = get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, _n);
            let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, _n);
            assert_ok!(Balances::transfer(
                &account(0), // alice
                &coldkey.clone(),
                amount + burn_amount + 500,
                ExistenceRequirement::KeepAlive,
            ));

            let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
            let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

            let v_reward_rate = test_percent(1, 20); // 5%

            assert_ok!(Network::register_validator(
                RuntimeOrigin::signed(coldkey.clone()),
                hotkey.clone(),
                v_reward_rate,
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
                amount,
                None,
                None,
                u128::MAX,
            ));
            let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

            let subnet_node = RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
            assert_eq!(
                subnet_node.classification.node_class,
                SubnetNodeClass::Registered
            );
            assert_eq!(subnet_node.classification.start_epoch, subnet_epoch + 1);

            assert_eq!(
                total_subnet_nodes + 1,
                TotalSubnetNodes::<Test>::get(subnet_id)
            );

            let reg_queue = SubnetNodeQueue::<Test>::get(subnet_id);
            let found = reg_queue.iter().find(|node| node.id == subnet_node_id);
            assert_eq!(found.unwrap().id, subnet_node_id);
            System::set_block_number(System::block_number() + 1);
        }

        assert_eq!(
            SubnetNodeQueue::<Test>::get(subnet_id).len() as u32,
            reg_end - reg_start
        );

        let total_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
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

        // Only activate up to the churn limit
        assert_eq!(
            prev_active_total_nodes + churn_limit,
            TotalActiveSubnetNodes::<Test>::get(subnet_id)
        );

        assert_eq!(
            SubnetNodeQueue::<Test>::get(subnet_id).len() as u32,
            reg_end - reg_start - churn_limit
        );

        for n in reg_start..reg_end {
            let _n = n + 1;
            let subnet_node_id = _n;

            // Ensure all nodes up to the churn limit were activated
            if _n <= reg_start + churn_limit {
                // Check in activation
                let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
                assert_eq!(subnet_node.classification.node_class, SubnetNodeClass::Idle);
                assert_eq!(subnet_node.classification.start_epoch, subnet_epoch);

                // Check out of queue
                assert_eq!(
                    RegisteredSubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
                    Err(())
                );
                let reg_queue = SubnetNodeQueue::<Test>::get(subnet_id);
                let found = reg_queue.iter().find(|node| node.id == subnet_node_id);
                assert_eq!(found, None);
            } else {
                // Other nodes still in queue
                assert_eq!(
                    SubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
                    Err(())
                );
                let reg_queue = SubnetNodeQueue::<Test>::get(subnet_id);
                let found = reg_queue.iter().find(|node| node.id == subnet_node_id);
                assert_eq!(found.unwrap().id, subnet_node_id);
            }
        }
    })
}

#[test]
fn test_register_subnet_node_v2_with_max_nodes() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = max_subnet_nodes;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let queue_epochs = SubnetNodeQueueEpochs::<Test>::get(subnet_id);
        let churn_limit = ChurnLimit::<Test>::get(subnet_id);
        let prev_active_total_nodes = TotalActiveSubnetNodes::<Test>::get(subnet_id);

        let reg_start = end;
        let reg_end = reg_start + churn_limit * 2;
        let burn_amount = Network::calculate_burn_amount(subnet_id);

        // Put a bunch of nodes into the queue
        for n in reg_start..reg_end {
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
                    let v_reward_rate = test_percent(1, 20); // 5%

                    assert_ok!(Network::register_validator(
                        RuntimeOrigin::signed(coldkey.clone()),
                        hotkey.clone(),
                        v_reward_rate,
                        None,
                        None
                    ));

                    coldkey
                };

            let validator_id = ColdkeyValidatorId::<Test>::get(coldkey.clone()).unwrap();
            let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, _n);

            let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, _n);
            let bootnode_peer_id = get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, _n);
            let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, _n);
            assert_ok!(Balances::transfer(
                &account(0), // alice
                &coldkey.clone(),
                amount + burn_amount + 500,
                ExistenceRequirement::KeepAlive,
            ));

            let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
            let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

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
                amount,
                None,
                None,
                u128::MAX,
            ));
            let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

            let subnet_node = RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
            assert_eq!(
                subnet_node.classification.node_class,
                SubnetNodeClass::Registered
            );
            assert_eq!(subnet_node.classification.start_epoch, subnet_epoch + 1);

            assert_eq!(
                total_subnet_nodes + 1,
                TotalSubnetNodes::<Test>::get(subnet_id)
            );

            let reg_queue = SubnetNodeQueue::<Test>::get(subnet_id);
            let found = reg_queue.iter().find(|node| node.id == subnet_node_id);
            assert_eq!(found.unwrap().id, subnet_node_id);
            System::set_block_number(System::block_number() + 1);
        }

        assert_eq!(
            SubnetNodeQueue::<Test>::get(subnet_id).len() as u32,
            reg_end - reg_start
        );

        let total_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
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

        // Only activate up to the churn limit
        assert_eq!(
            max_subnet_nodes,
            TotalActiveSubnetNodes::<Test>::get(subnet_id)
        );
        assert_eq!(
            max_subnet_nodes + reg_end - reg_start,
            TotalSubnetNodes::<Test>::get(subnet_id)
        );

        assert_eq!(
            SubnetNodeQueue::<Test>::get(subnet_id).len() as u32,
            reg_end - reg_start
        );

        // no nodes should be activated
        for n in reg_start..reg_end {
            let _n = n + 1;
            let hotkey_subnet_node_id = _n;

            // Other nodes still in queue
            assert_eq!(
                SubnetNodesData::<Test>::try_get(subnet_id, hotkey_subnet_node_id),
                Err(())
            );
            let reg_queue = SubnetNodeQueue::<Test>::get(subnet_id);
            let found = reg_queue
                .iter()
                .find(|node| node.id == hotkey_subnet_node_id);
            assert_eq!(found.unwrap().id, hotkey_subnet_node_id);
        }
    })
}

#[test]
fn test_register_subnet_node_v2_activate_up_to_max_nodes() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let expected_activated = 2;
        let end = max_subnet_nodes - expected_activated;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let queue_epochs = SubnetNodeQueueEpochs::<Test>::get(subnet_id);
        let churn_limit = ChurnLimit::<Test>::get(subnet_id);
        let prev_active_total_nodes = TotalActiveSubnetNodes::<Test>::get(subnet_id);

        let reg_start = end;
        let reg_end = reg_start + churn_limit * 2;
        let burn_amount = Network::calculate_burn_amount(subnet_id);

        // Put a bunch of nodes into the queue
        for n in reg_start..reg_end {
            let _n = n + 1;
            let coldkey = get_coldkey(subnets, max_subnet_nodes, _n);
            let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, _n);

            let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, _n);
            let bootnode_peer_id = get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, _n);
            let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, _n);
            assert_ok!(Balances::transfer(
                &account(0), // alice
                &coldkey.clone(),
                amount + burn_amount + 500,
                ExistenceRequirement::KeepAlive,
            ));

            let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
            let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

            let v_reward_rate = test_percent(1, 20); // 5%

            assert_ok!(Network::register_validator(
                RuntimeOrigin::signed(coldkey.clone()),
                hotkey.clone(),
                v_reward_rate,
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
                amount,
                None,
                None,
                u128::MAX,
            ));
            let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

            let subnet_node = RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
            assert_eq!(
                subnet_node.classification.node_class,
                SubnetNodeClass::Registered
            );
            assert_eq!(subnet_node.classification.start_epoch, subnet_epoch + 1);

            assert_eq!(
                total_subnet_nodes + 1,
                TotalSubnetNodes::<Test>::get(subnet_id)
            );

            let reg_queue = SubnetNodeQueue::<Test>::get(subnet_id);
            let found = reg_queue.iter().find(|node| node.id == subnet_node_id);
            assert_eq!(found.unwrap().id, subnet_node_id);
        }

        assert_eq!(
            SubnetNodeQueue::<Test>::get(subnet_id).len() as u32,
            reg_end - reg_start
        );
        assert_eq!(end, TotalActiveSubnetNodes::<Test>::get(subnet_id));
        assert_eq!(
            end + reg_end - reg_start,
            TotalSubnetNodes::<Test>::get(subnet_id)
        );

        let total_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
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

        // Only activate up to the churn limit
        assert_eq!(
            max_subnet_nodes,
            TotalActiveSubnetNodes::<Test>::get(subnet_id)
        );
        assert_eq!(
            end + reg_end - reg_start,
            TotalSubnetNodes::<Test>::get(subnet_id)
        );

        assert_eq!(
            SubnetNodeQueue::<Test>::get(subnet_id).len() as u32,
            reg_end - reg_start - expected_activated
        );

        // no nodes should be activated
        for n in reg_start..reg_end {
            let _n = n + 1;

            let hotkey_subnet_node_id = _n;

            // Ensure all nodes up to the churn limit were activated
            if _n <= reg_start + expected_activated {
                // Check in activation
                let subnet_node = SubnetNodesData::<Test>::get(subnet_id, hotkey_subnet_node_id);
                assert_eq!(subnet_node.classification.node_class, SubnetNodeClass::Idle);
                // assert_eq!(subnet_node.classification.start_epoch, subnet_epoch + 1);
                assert_eq!(subnet_node.classification.start_epoch, subnet_epoch);

                // Check out of queue
                assert_eq!(
                    RegisteredSubnetNodesData::<Test>::try_get(subnet_id, hotkey_subnet_node_id),
                    Err(())
                );
                let reg_queue = SubnetNodeQueue::<Test>::get(subnet_id);
                let found = reg_queue
                    .iter()
                    .find(|node| node.id == hotkey_subnet_node_id);
                assert_eq!(found, None);
            } else {
                // Other nodes still in queue
                assert_eq!(
                    SubnetNodesData::<Test>::try_get(subnet_id, hotkey_subnet_node_id),
                    Err(())
                );
                let reg_queue = SubnetNodeQueue::<Test>::get(subnet_id);
                let found = reg_queue
                    .iter()
                    .find(|node| node.id == hotkey_subnet_node_id);
                assert_eq!(found.unwrap().id, hotkey_subnet_node_id);
            }
        }
    })
}
