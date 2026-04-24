use super::mock::*;
use crate::tests::test_utils::*;
use crate::Event;
use crate::{
    AccountSubnetStake, BootnodePeerIdSubnetNodeId, ClientPeerIdSubnetNodeId, ColdkeyReputation,
    ColdkeySubnetNodes, ColdkeyValidatorId, CurrentNodeBurnRate, DefaultMaxVectorLength, Error,
    HotkeyOwner, HotkeySubnetId, HotkeySubnetNodeId, HotkeyValidatorId, MaxDelegateStakePercentage,
    MaxRegisteredNodes, MaxRewardRateDecrease, MaxSubnetNodes, MaxSubnets, MinSubnetMinStake,
    MinSubnetNodes, MultiaddrSubnetNodeId, NodeRewardRateUpdatePeriod, NodeSlotIndex,
    NodeSubnetStake, PeerIdSubnetNodeId, PeerInfo, RegisteredSubnetNodesData,
    RegisteredSubnetNodesDataV2, SubnetElectedValidator, SubnetMinStakeBalance, SubnetName,
    SubnetNode, SubnetNodeClass, SubnetNodeClassification, SubnetNodeElectionSlots,
    SubnetNodeIdHotkey, SubnetNodeQueueEpochs, SubnetNodeReputation, SubnetNodeV2,
    SubnetNodeValidatorId, SubnetNodesData, SubnetNodesDataV2, SubnetOwner,
    SubnetPauseCooldownEpochs, SubnetRegistrationEpochs, SubnetState, TotalActiveNodes,
    TotalActiveSubnetNodes, TotalActiveSubnets, TotalElectableNodes, TotalNodes, TotalStake,
    TotalSubnetElectableNodes, TotalSubnetNodeUids, TotalSubnetNodes, TotalSubnetStake,
    TotalSubnetUids, TotalValidatorIds, UniqueParamSubnetNodeId, ValidatorColdkey,
    ValidatorColdkeyHotkey, ValidatorIdHotkey, ValidatorsData,
};
use frame_support::traits::Currency;
use frame_support::traits::ExistenceRequirement;
use frame_support::weights::WeightMeter;
use frame_support::BoundedVec;
use frame_support::{assert_err, assert_ok};
use sp_core::OpaquePeerId as PeerId;
use sp_std::collections::{btree_map::BTreeMap, btree_set::BTreeSet};

#[test]
fn test_register_validator() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let reward_rate = 50000000000000000; // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();
        assert!(current_id > 0);
        assert_eq!(
            ValidatorIdHotkey::<Test>::get(current_id).unwrap(),
            hotkey.clone()
        );
        let v_data = ValidatorsData::<Test>::get(current_id);

        let v_id = v_data.id;
        let v_hotkey = v_data.hotkey;
        let v_delegate_reward_rate = v_data.delegate_reward_rate;
        let v_last_delegate_reward_rate_update = v_data.last_delegate_reward_rate_update;
        let v_delegate_account = v_data.delegate_account;
        let v_identity = v_data.identity;

        assert_eq!(v_id, current_id);
        assert_eq!(v_hotkey, hotkey.clone());
        assert_eq!(v_delegate_reward_rate, reward_rate);
        assert_eq!(v_last_delegate_reward_rate_update, 0);
        assert_eq!(v_delegate_account, None);
        assert_eq!(v_identity, None);

        assert_eq!(
            ColdkeyValidatorId::<Test>::get(coldkey.clone()).unwrap(),
            current_id
        );
        assert_eq!(
            ValidatorColdkeyHotkey::<Test>::get(coldkey.clone()).unwrap(),
            hotkey.clone()
        );
        assert_eq!(
            HotkeyValidatorId::<Test>::get(hotkey.clone()).unwrap(),
            current_id
        );

        // Try to register under same coldkey
        assert_err!(
            Network::do_register_validator(
                RuntimeOrigin::signed(coldkey.clone()),
                hotkey,
                reward_rate,
                None,
                None,
            ),
            Error::<Test>::NotKeyOwner
        );

        // Try to register under same coldkey with new hotkey
        assert_err!(
            Network::do_register_validator(
                RuntimeOrigin::signed(coldkey.clone()),
                account(999),
                reward_rate,
                None,
                None,
            ),
            Error::<Test>::NotKeyOwner
        );
    })
}

#[test]
fn test_register_validator_subnet_node() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let reward_rate = 50000000000000000; // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();
        assert!(current_id > 0);
        assert_eq!(
            ValidatorIdHotkey::<Test>::get(current_id).unwrap(),
            hotkey.clone()
        );
        let v_data = ValidatorsData::<Test>::get(current_id);

        let v_id = v_data.id;
        let v_hotkey = v_data.hotkey;
        let v_delegate_reward_rate = v_data.delegate_reward_rate;
        let v_last_delegate_reward_rate_update = v_data.last_delegate_reward_rate_update;
        let v_delegate_account = v_data.delegate_account;
        let v_identity = v_data.identity;

        assert_eq!(v_id, current_id);
        assert_eq!(v_hotkey, hotkey.clone());
        assert_eq!(v_delegate_reward_rate, reward_rate);
        assert_eq!(v_last_delegate_reward_rate_update, 0);
        assert_eq!(v_delegate_account, None);
        assert_eq!(v_identity, None);

        assert_eq!(
            ColdkeyValidatorId::<Test>::get(coldkey.clone()).unwrap(),
            current_id
        );
        assert_eq!(
            ValidatorColdkeyHotkey::<Test>::get(coldkey.clone()).unwrap(),
            hotkey.clone()
        );
        assert_eq!(
            HotkeyValidatorId::<Test>::get(hotkey.clone()).unwrap(),
            current_id
        );

        // Insert mock subnet
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);

        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = 1000000000000000000000;
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount);

        // Wrong coldkey
        assert_err!(
            Network::do_register_subnet_node_v2(
                RuntimeOrigin::signed(account(999)),
                current_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: peer(1),
                    multiaddr: None,
                },
                None,
                None,
                stake_amount,
                None,
                None,
                burn_amount + 100000000,
            ),
            Error::<Test>::NotKeyOwner
        );

        // Wrong validator_id
        assert_err!(
            Network::do_register_subnet_node_v2(
                RuntimeOrigin::signed(coldkey.clone()),
                999,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: peer(1),
                    multiaddr: None,
                },
                None,
                None,
                stake_amount,
                None,
                None,
                burn_amount + 100000000,
            ),
            Error::<Test>::NotKeyOwner
        );

        // Wrong validator_id
        assert_err!(
            Network::do_register_subnet_node_v2(
                RuntimeOrigin::signed(coldkey.clone()),
                current_id,
                subnet_id,
                None,
                PeerInfo {
                    peer_id: peer(1),
                    multiaddr: None,
                },
                None,
                None,
                stake_amount,
                None,
                None,
                0,
            ),
            Error::<Test>::MaxBurnAmountExceeded
        );

        assert_ok!(Network::do_register_subnet_node_v2(
            RuntimeOrigin::signed(coldkey.clone()),
            current_id,
            subnet_id,
            None,
            PeerInfo {
                peer_id: peer(999),
                multiaddr: None,
            },
            None,
            None,
            stake_amount,
            None,
            None,
            burn_amount + 100000000,
        ));

        let node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);
        let node = RegisteredSubnetNodesDataV2::<Test>::get(subnet_id, node_id);
        assert_eq!(node.id, node_id);
        assert_eq!(node.validator_id, current_id);
        assert_eq!(
            node.peer_info,
            PeerInfo {
                peer_id: peer(999),
                multiaddr: None,
            }
        );
        assert_eq!(node.bootnode_peer_info, None);
        assert_eq!(node.client_peer_info, None);
        assert_eq!(node.classification.node_class, SubnetNodeClass::Registered);
        // assert_eq!(node.classification.start_epoch, 0);
        assert_eq!(node.unique, None);
        assert_eq!(node.non_unique, None);

        assert_eq!(
            SubnetNodeValidatorId::<Test>::get(subnet_id, node_id).unwrap(),
            current_id
        );

        // Stake balance for node
        assert_eq!(
            NodeSubnetStake::<Test>::get(node_id, subnet_id),
            stake_amount
        );
    })
}

#[test]
fn test_get_hotkey_associated_subnet_node_prefers_subnet_node_hotkey_override() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let subnet_node_id = 7;
        let validator_id = 11;
        let validator_hotkey = account(1100);
        let subnet_node_hotkey = account(1101);

        ValidatorIdHotkey::<Test>::insert(validator_id, validator_hotkey.clone());
        SubnetNodesDataV2::<Test>::insert(
            subnet_id,
            subnet_node_id,
            SubnetNodeV2 {
                id: subnet_node_id,
                validator_id,
                peer_info: PeerInfo {
                    peer_id: peer(1),
                    multiaddr: None,
                },
                bootnode_peer_info: None,
                client_peer_info: None,
                classification: SubnetNodeClassification {
                    node_class: SubnetNodeClass::Registered,
                    start_epoch: 0,
                },
                unique: None,
                non_unique: None,
            },
        );
        HotkeySubnetNodeId::<Test>::insert(subnet_id, &subnet_node_hotkey, subnet_node_id);
        SubnetNodeIdHotkey::<Test>::insert(subnet_id, subnet_node_id, &subnet_node_hotkey);

        assert_ok!(Network::get_hotkey_associated_subnet_node(
            subnet_id,
            subnet_node_id,
            validator_id,
            subnet_node_hotkey,
        ));

        assert_err!(
            Network::get_hotkey_associated_subnet_node(
                subnet_id,
                subnet_node_id,
                validator_id,
                validator_hotkey,
            ),
            Error::<Test>::InvalidHotkeySubnetNodeId
        );
    })
}

#[test]
fn test_get_hotkey_associated_subnet_node_uses_validator_hotkey_without_override() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let subnet_node_id = 8;
        let validator_id = 12;
        let validator_hotkey = account(1200);

        ValidatorIdHotkey::<Test>::insert(validator_id, validator_hotkey.clone());
        SubnetNodesDataV2::<Test>::insert(
            subnet_id,
            subnet_node_id,
            SubnetNodeV2 {
                id: subnet_node_id,
                validator_id,
                peer_info: PeerInfo {
                    peer_id: peer(2),
                    multiaddr: None,
                },
                bootnode_peer_info: None,
                client_peer_info: None,
                classification: SubnetNodeClassification {
                    node_class: SubnetNodeClass::Registered,
                    start_epoch: 0,
                },
                unique: None,
                non_unique: None,
            },
        );

        assert_ok!(Network::get_hotkey_associated_subnet_node(
            subnet_id,
            subnet_node_id,
            validator_id,
            validator_hotkey,
        ));
    })
}

#[test]
fn test_get_subnet_node_associated_coldkey_returns_validator_coldkey() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let subnet_node_id = 9;
        let validator_id = 13;
        let validator_coldkey = account(1300);

        SubnetNodeValidatorId::<Test>::insert(subnet_id, subnet_node_id, validator_id);
        ValidatorColdkey::<Test>::insert(validator_id, validator_coldkey.clone());

        assert_eq!(
            Network::get_subnet_node_associated_coldkey(subnet_id, subnet_node_id).unwrap(),
            validator_coldkey
        );
    })
}

#[test]
fn test_get_subnet_node_associated_coldkey_errors_without_node_owner() {
    new_test_ext().execute_with(|| {
        assert_err!(
            Network::get_subnet_node_associated_coldkey(1, 9),
            Error::<Test>::InvalidSubnetNodeId
        );
    })
}

#[test]
fn test_get_subnet_node_associated_coldkey_errors_without_validator_coldkey() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let subnet_node_id = 9;
        let validator_id = 13;

        SubnetNodeValidatorId::<Test>::insert(subnet_id, subnet_node_id, validator_id);

        assert_err!(
            Network::get_subnet_node_associated_coldkey(subnet_id, subnet_node_id),
            Error::<Test>::InvalidValidatorId
        );
    })
}

#[test]
fn test_update_validator_hotkey() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let new_hotkey = account(2);
        let new_hotkey_2 = account(3);
        let reward_rate = 50000000000000000; // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();
        assert!(current_id > 0);
        assert_eq!(
            ValidatorIdHotkey::<Test>::get(current_id).unwrap(),
            hotkey.clone()
        );
        let v_data = ValidatorsData::<Test>::get(current_id);
        let v_data_hotkey = v_data.hotkey;
        let v_hotkey = ValidatorIdHotkey::<Test>::get(current_id).unwrap();
        let c_hotkey = ValidatorColdkeyHotkey::<Test>::get(coldkey.clone()).unwrap();

        assert_eq!(v_hotkey, c_hotkey);

        assert_err!(
            Network::update_validator_hotkey(
                RuntimeOrigin::signed(coldkey.clone()),
                current_id + 1,
                new_hotkey,
            ),
            Error::<Test>::NotKeyOwner
        );

        assert_ok!(Network::update_validator_hotkey(
            RuntimeOrigin::signed(coldkey.clone()),
            current_id,
            new_hotkey,
        ));

        assert_eq!(
            new_hotkey,
            ValidatorIdHotkey::<Test>::get(current_id).unwrap()
        );
        assert_ne!(
            v_hotkey,
            ValidatorIdHotkey::<Test>::get(current_id).unwrap()
        );

        assert_eq!(
            new_hotkey,
            ValidatorColdkeyHotkey::<Test>::get(coldkey.clone()).unwrap()
        );
        assert_ne!(
            c_hotkey,
            ValidatorColdkeyHotkey::<Test>::get(coldkey.clone()).unwrap()
        );

        assert_eq!(new_hotkey, ValidatorsData::<Test>::get(current_id).hotkey);
        assert_ne!(
            v_data_hotkey,
            ValidatorsData::<Test>::get(current_id).hotkey
        );
    })
}

#[test]
fn test_update_validator_delegate_reward_rate() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let new_hotkey = account(2);
        let new_hotkey_2 = account(3);
        let reward_rate = 50000000000000000; // 5%
        let new_reward_rate = 59000000000000000; // 5.9%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();
        assert!(current_id > 0);
        assert_eq!(
            ValidatorIdHotkey::<Test>::get(current_id).unwrap(),
            hotkey.clone()
        );
        let v_data = ValidatorsData::<Test>::get(current_id);
        let v_data_hotkey = v_data.hotkey;
        let v_hotkey = ValidatorIdHotkey::<Test>::get(current_id).unwrap();
        let c_hotkey = ValidatorColdkeyHotkey::<Test>::get(coldkey.clone()).unwrap();

        assert_eq!(v_hotkey, c_hotkey);

        let reward_rate_update_period = NodeRewardRateUpdatePeriod::<Test>::get();

        System::set_block_number(System::block_number() + reward_rate_update_period);

        assert_err!(
            Network::update_validator_delegate_reward_rate(
                RuntimeOrigin::signed(coldkey.clone()),
                current_id + 1,
                new_reward_rate,
            ),
            Error::<Test>::NotKeyOwner
        );

        assert_ok!(Network::update_validator_delegate_reward_rate(
            RuntimeOrigin::signed(coldkey.clone()),
            current_id,
            new_reward_rate,
        ));

        assert_eq!(
            new_reward_rate,
            ValidatorsData::<Test>::get(current_id).delegate_reward_rate
        );
        assert_ne!(
            reward_rate,
            ValidatorsData::<Test>::get(current_id).delegate_reward_rate
        );
    })
}
