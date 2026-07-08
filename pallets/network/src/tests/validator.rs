use super::mock::*;
use crate::tests::test_utils::*;
use crate::Event;
use crate::{
    BootnodePeerIdSubnetNodeId, ClientPeerIdSubnetNodeId, ColdkeyValidatorId, CurrentNodeBurnRate,
    DelegateAccount, Error, HotkeyValidatorId, IdentityData, MaxDelegateStakePercentage,
    MaxRegisteredNodes, MaxRewardRateDecrease, MaxSubnetNodes, MaxSubnets, MinSubnetMinStake,
    MinSubnetNodes, MultiaddrSubnetNodeId, NodeRewardRateUpdatePeriod, NodeSlotIndex,
    NodeSubnetStake, OverwatchNodeValidatorId, OverwatchNodes, PeerIdSubnetNodeId, PeerInfo,
    RegisteredSubnetNodesData, SubnetElectedValidator, SubnetMinStakeBalance, SubnetName,
    SubnetNode, SubnetNodeClass, SubnetNodeClassification, SubnetNodeElectionSlots,
    SubnetNodeIdHotkey, SubnetNodeQueueEpochs, SubnetNodeReputation, SubnetNodeValidatorId,
    SubnetNodesData, SubnetOwner, SubnetPauseCooldownEpochs, SubnetRegistrationEpochs, SubnetState,
    TotalActiveNodes, TotalActiveSubnetNodes, TotalActiveSubnets, TotalElectableNodes, TotalNodes,
    TotalOverwatchNodeUids, TotalOverwatchNodes, TotalStake, TotalSubnetElectableNodes,
    TotalSubnetNodeUids, TotalSubnetNodes, TotalSubnetStake, TotalSubnetUids, TotalValidatorIds,
    UniqueParamSubnetNodeId, ValidatorColdkey, ValidatorColdkeyHotkey, ValidatorData,
    ValidatorIdHotkey, ValidatorsData,
};
use frame_support::traits::{Currency, ExistenceRequirement};
use frame_support::weights::WeightMeter;
use frame_support::BoundedVec;
use frame_support::{assert_err, assert_ok};
use sp_core::OpaquePeerId as PeerId;
use sp_std::collections::{btree_map::BTreeMap, btree_set::BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
struct ValidatorStorageSnapshot {
    coldkey_validator_ids: Vec<(AccountId, Option<u32>)>,
    coldkey_hotkeys: Vec<(AccountId, Option<AccountId>)>,
    hotkey_validator_ids: Vec<(AccountId, Option<u32>)>,
    validator_coldkey: Option<AccountId>,
    validator_hotkey: Option<AccountId>,
    validators_data: ValidatorData<Test>,
}

fn register_validator_for_rotation(coldkey: &AccountId, hotkey: &AccountId) -> u32 {
    let total_before = TotalValidatorIds::<Test>::get();
    let reward_rate = MaxDelegateStakePercentage::<Test>::get().saturating_sub(1);

    assert_ok!(Network::do_register_validator(
        RuntimeOrigin::signed(coldkey.clone()),
        hotkey.clone(),
        reward_rate,
        None,
        None,
    ));

    let validator_id = TotalValidatorIds::<Test>::get();
    assert!(validator_id > total_before);
    assert_eq!(ColdkeyValidatorId::<Test>::get(coldkey), Some(validator_id));
    validator_id
}

fn validator_storage_snapshot(
    validator_id: u32,
    coldkeys: &[AccountId],
) -> ValidatorStorageSnapshot {
    ValidatorStorageSnapshot {
        coldkey_validator_ids: coldkeys
            .iter()
            .cloned()
            .map(|coldkey| {
                let value = ColdkeyValidatorId::<Test>::get(&coldkey);
                (coldkey, value)
            })
            .collect(),
        coldkey_hotkeys: coldkeys
            .iter()
            .cloned()
            .map(|coldkey| {
                let value = ValidatorColdkeyHotkey::<Test>::get(&coldkey);
                (coldkey, value)
            })
            .collect(),
        hotkey_validator_ids: coldkeys
            .iter()
            .cloned()
            .map(|hotkey| {
                let value = HotkeyValidatorId::<Test>::get(&hotkey);
                (hotkey, value)
            })
            .collect(),
        validator_coldkey: ValidatorColdkey::<Test>::get(validator_id),
        validator_hotkey: ValidatorIdHotkey::<Test>::get(validator_id),
        validators_data: ValidatorsData::<Test>::get(validator_id),
    }
}

#[test]
fn test_register_validator_invalid_delegate_account_does_not_commit_partial_state() {
    new_test_ext().execute_with(|| {
        let coldkey = account(10_000);
        let hotkey = account(10_001);
        let delegate_account = DelegateAccount {
            account_id: account(10_002),
            rate: 0,
        };

        let total_validator_ids = TotalValidatorIds::<Test>::get();
        let next_validator_id = total_validator_ids.saturating_add(1);
        let validator_id_hotkey = ValidatorIdHotkey::<Test>::get(next_validator_id);
        let validators_data_exists = ValidatorsData::<Test>::contains_key(next_validator_id);
        let coldkey_validator_id = ColdkeyValidatorId::<Test>::get(&coldkey);
        let validator_coldkey = ValidatorColdkey::<Test>::get(next_validator_id);
        let validator_coldkey_hotkey = ValidatorColdkeyHotkey::<Test>::get(&coldkey);
        let hotkey_validator_id = HotkeyValidatorId::<Test>::get(&hotkey);

        assert_err!(
            Network::do_register_validator(
                RuntimeOrigin::signed(coldkey.clone()),
                hotkey.clone(),
                test_percent(1, 20),
                Some(delegate_account),
                None,
            ),
            Error::<Test>::InvalidDelegateAccountRate
        );

        assert_eq!(TotalValidatorIds::<Test>::get(), total_validator_ids);
        assert_eq!(
            ValidatorIdHotkey::<Test>::get(next_validator_id),
            validator_id_hotkey
        );
        assert_eq!(
            ValidatorsData::<Test>::contains_key(next_validator_id),
            validators_data_exists
        );
        assert_eq!(
            ColdkeyValidatorId::<Test>::get(&coldkey),
            coldkey_validator_id
        );
        assert_eq!(
            ValidatorColdkey::<Test>::get(next_validator_id),
            validator_coldkey
        );
        assert_eq!(
            ValidatorColdkeyHotkey::<Test>::get(&coldkey),
            validator_coldkey_hotkey
        );
        assert_eq!(HotkeyValidatorId::<Test>::get(&hotkey), hotkey_validator_id);
    });
}

#[test]
fn test_register_validator() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let reward_rate = test_percent(1, 20); // 5%
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
fn test_update_validator_coldkey_revokes_old_coldkey() {
    new_test_ext().execute_with(|| {
        let old_coldkey = account(10_000);
        let old_hotkey = account(10_001);
        let new_coldkey = account(10_002);
        let replacement_hotkey = account(10_003);

        let validator_id = register_validator_for_rotation(&old_coldkey, &old_hotkey);
        let original_hotkey = ValidatorIdHotkey::<Test>::get(validator_id).unwrap();
        let original_validator_data = ValidatorsData::<Test>::get(validator_id);

        assert_ok!(Network::update_validator_coldkey(
            RuntimeOrigin::signed(old_coldkey.clone()),
            validator_id,
            new_coldkey.clone(),
        ));

        assert_eq!(ColdkeyValidatorId::<Test>::get(&old_coldkey), None);
        assert_eq!(ValidatorColdkeyHotkey::<Test>::get(&old_coldkey), None);
        assert_eq!(
            ColdkeyValidatorId::<Test>::get(&new_coldkey),
            Some(validator_id)
        );
        assert_eq!(
            ValidatorColdkey::<Test>::get(validator_id),
            Some(new_coldkey.clone())
        );
        assert_eq!(
            ValidatorColdkeyHotkey::<Test>::get(&new_coldkey),
            Some(original_hotkey.clone())
        );
        assert_eq!(
            ValidatorIdHotkey::<Test>::get(validator_id),
            Some(original_hotkey.clone())
        );
        assert_eq!(
            ValidatorsData::<Test>::get(validator_id),
            original_validator_data
        );

        let before_old_hotkey_attempt =
            validator_storage_snapshot(validator_id, &[old_coldkey.clone(), new_coldkey.clone()]);
        assert_err!(
            Network::update_validator_hotkey(
                RuntimeOrigin::signed(old_coldkey.clone()),
                validator_id,
                replacement_hotkey.clone(),
            ),
            Error::<Test>::NotKeyOwner
        );
        assert_eq!(
            validator_storage_snapshot(validator_id, &[old_coldkey.clone(), new_coldkey.clone()]),
            before_old_hotkey_attempt
        );

        assert_ok!(Network::update_validator_hotkey(
            RuntimeOrigin::signed(new_coldkey.clone()),
            validator_id,
            replacement_hotkey.clone(),
        ));
        assert_eq!(
            ValidatorIdHotkey::<Test>::get(validator_id),
            Some(replacement_hotkey.clone())
        );
        assert_eq!(
            ValidatorColdkeyHotkey::<Test>::get(&new_coldkey),
            Some(replacement_hotkey)
        );
    })
}

#[test]
fn test_old_validator_coldkey_cannot_rotate_again_after_rotation() {
    new_test_ext().execute_with(|| {
        let old_coldkey = account(10_010);
        let old_hotkey = account(10_011);
        let new_coldkey = account(10_012);
        let second_new_coldkey = account(10_013);

        let validator_id = register_validator_for_rotation(&old_coldkey, &old_hotkey);

        assert_ok!(Network::update_validator_coldkey(
            RuntimeOrigin::signed(old_coldkey.clone()),
            validator_id,
            new_coldkey.clone(),
        ));

        let before = validator_storage_snapshot(
            validator_id,
            &[
                old_coldkey.clone(),
                new_coldkey.clone(),
                second_new_coldkey.clone(),
            ],
        );
        assert_err!(
            Network::update_validator_coldkey(
                RuntimeOrigin::signed(old_coldkey.clone()),
                validator_id,
                second_new_coldkey.clone(),
            ),
            Error::<Test>::NotKeyOwner
        );
        assert_eq!(
            validator_storage_snapshot(
                validator_id,
                &[old_coldkey, new_coldkey, second_new_coldkey]
            ),
            before
        );
    })
}

#[test]
fn test_stale_old_coldkey_index_does_not_authorize_validator_owner_calls() {
    new_test_ext().execute_with(|| {
        let old_coldkey = account(10_020);
        let old_hotkey = account(10_021);
        let new_coldkey = account(10_022);
        let attempted_hotkey = account(10_023);

        let validator_id = register_validator_for_rotation(&old_coldkey, &old_hotkey);

        assert_ok!(Network::update_validator_coldkey(
            RuntimeOrigin::signed(old_coldkey.clone()),
            validator_id,
            new_coldkey.clone(),
        ));

        ColdkeyValidatorId::<Test>::insert(old_coldkey.clone(), validator_id);
        assert_eq!(
            ColdkeyValidatorId::<Test>::get(&old_coldkey),
            Some(validator_id)
        );
        assert_eq!(
            ValidatorColdkey::<Test>::get(validator_id),
            Some(new_coldkey.clone())
        );

        let before_hotkey_update =
            validator_storage_snapshot(validator_id, &[old_coldkey.clone(), new_coldkey.clone()]);
        assert_err!(
            Network::update_validator_hotkey(
                RuntimeOrigin::signed(old_coldkey.clone()),
                validator_id,
                attempted_hotkey,
            ),
            Error::<Test>::NotKeyOwner
        );
        assert_eq!(
            validator_storage_snapshot(validator_id, &[old_coldkey.clone(), new_coldkey.clone()]),
            before_hotkey_update
        );

        let subnet_id = TotalSubnetUids::<Test>::get().saturating_add(1);
        insert_subnet(subnet_id, SubnetState::Active, 0);

        let register_peer_id = peer(10_024);
        let next_subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id).saturating_add(1);
        let before_register_node = (
            TotalSubnetNodeUids::<Test>::get(subnet_id),
            TotalSubnetNodes::<Test>::get(subnet_id),
            TotalNodes::<Test>::get(),
            PeerIdSubnetNodeId::<Test>::get(subnet_id, &register_peer_id),
            RegisteredSubnetNodesData::<Test>::get(subnet_id, next_subnet_node_id),
            SubnetNodesData::<Test>::get(subnet_id, next_subnet_node_id),
            SubnetNodeValidatorId::<Test>::get(subnet_id, next_subnet_node_id),
        );

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(old_coldkey.clone()),
                validator_id,
                subnet_id,
                None,
                Some(PeerInfo::<Test> {
                    peer_id: register_peer_id.clone(),
                    multiaddr: None,
                }),
                None,
                None,
                1,
                None,
                None,
                0,
            ),
            Error::<Test>::NotKeyOwner
        );
        assert_eq!(
            (
                TotalSubnetNodeUids::<Test>::get(subnet_id),
                TotalSubnetNodes::<Test>::get(subnet_id),
                TotalNodes::<Test>::get(),
                PeerIdSubnetNodeId::<Test>::get(subnet_id, &register_peer_id),
                RegisteredSubnetNodesData::<Test>::get(subnet_id, next_subnet_node_id),
                SubnetNodesData::<Test>::get(subnet_id, next_subnet_node_id),
                SubnetNodeValidatorId::<Test>::get(subnet_id, next_subnet_node_id),
            ),
            before_register_node
        );

        insert_subnet_node(
            validator_id,
            subnet_id,
            10_025,
            10_026,
            10_027,
            SubnetNodeClass::Registered,
            0,
        );
        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);
        let before_remove_node = (
            TotalSubnetNodeUids::<Test>::get(subnet_id),
            TotalSubnetNodes::<Test>::get(subnet_id),
            TotalNodes::<Test>::get(),
            RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id),
            SubnetNodesData::<Test>::get(subnet_id, subnet_node_id),
            SubnetNodeValidatorId::<Test>::get(subnet_id, subnet_node_id),
        );

        assert_err!(
            Network::remove_subnet_node(
                RuntimeOrigin::signed(old_coldkey.clone()),
                subnet_id,
                subnet_node_id,
            ),
            Error::<Test>::NotKeyOwner
        );
        assert_eq!(
            (
                TotalSubnetNodeUids::<Test>::get(subnet_id),
                TotalSubnetNodes::<Test>::get(subnet_id),
                TotalNodes::<Test>::get(),
                RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id),
                SubnetNodesData::<Test>::get(subnet_id, subnet_node_id),
                SubnetNodeValidatorId::<Test>::get(subnet_id, subnet_node_id),
            ),
            before_remove_node
        );

        let before_overwatch_register = (
            TotalOverwatchNodeUids::<Test>::get(),
            TotalOverwatchNodes::<Test>::get(),
            OverwatchNodes::<Test>::iter().collect::<Vec<_>>(),
            OverwatchNodeValidatorId::<Test>::iter().collect::<Vec<_>>(),
        );
        assert_err!(
            Network::register_overwatch_node(RuntimeOrigin::signed(old_coldkey.clone()), 1),
            Error::<Test>::NotKeyOwner
        );
        assert_eq!(
            (
                TotalOverwatchNodeUids::<Test>::get(),
                TotalOverwatchNodes::<Test>::get(),
                OverwatchNodes::<Test>::iter().collect::<Vec<_>>(),
                OverwatchNodeValidatorId::<Test>::iter().collect::<Vec<_>>(),
            ),
            before_overwatch_register
        );
    })
}

#[test]
fn test_update_validator_coldkey_rejects_collisions_without_mutating_state() {
    new_test_ext().execute_with(|| {
        let first_coldkey = account(10_040);
        let first_hotkey = account(10_041);
        let second_coldkey = account(10_042);
        let second_hotkey = account(10_043);

        let first_validator_id = register_validator_for_rotation(&first_coldkey, &first_hotkey);
        let second_validator_id = register_validator_for_rotation(&second_coldkey, &second_hotkey);

        let tracked_coldkeys = vec![
            first_coldkey.clone(),
            first_hotkey.clone(),
            second_coldkey.clone(),
            second_hotkey.clone(),
        ];

        let assert_failed_rotation_preserves_state =
            |new_coldkey: AccountId, expected_error: Error<Test>| {
                let first_before =
                    validator_storage_snapshot(first_validator_id, &tracked_coldkeys);
                let second_before =
                    validator_storage_snapshot(second_validator_id, &tracked_coldkeys);

                assert_err!(
                    Network::update_validator_coldkey(
                        RuntimeOrigin::signed(first_coldkey.clone()),
                        first_validator_id,
                        new_coldkey,
                    ),
                    expected_error
                );

                assert_eq!(
                    validator_storage_snapshot(first_validator_id, &tracked_coldkeys),
                    first_before
                );
                assert_eq!(
                    validator_storage_snapshot(second_validator_id, &tracked_coldkeys),
                    second_before
                );
            };

        assert_failed_rotation_preserves_state(first_coldkey.clone(), Error::<Test>::NotKeyOwner);
        assert_failed_rotation_preserves_state(
            first_hotkey.clone(),
            Error::<Test>::ColdkeyMatchesHotkey,
        );
        assert_failed_rotation_preserves_state(second_coldkey.clone(), Error::<Test>::NotKeyOwner);
        assert_failed_rotation_preserves_state(second_hotkey.clone(), Error::<Test>::NotKeyOwner);
    })
}

#[test]
fn test_update_validator_hotkey_replaces_reverse_index() {
    new_test_ext().execute_with(|| {
        let coldkey = account(10_080);
        let initial_hotkey = account(10_081);
        let new_hotkey = account(10_082);

        let validator_id = register_validator_for_rotation(&coldkey, &initial_hotkey);
        let current_hotkey = ValidatorIdHotkey::<Test>::get(validator_id).unwrap();
        let original_validator_data = ValidatorsData::<Test>::get(validator_id);

        assert_ok!(Network::update_validator_hotkey(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            new_hotkey.clone(),
        ));

        let mut expected_validator_data = original_validator_data;
        expected_validator_data.hotkey = new_hotkey.clone();

        assert_eq!(HotkeyValidatorId::<Test>::get(&current_hotkey), None);
        assert_eq!(
            HotkeyValidatorId::<Test>::get(&new_hotkey),
            Some(validator_id)
        );
        assert_eq!(
            ValidatorIdHotkey::<Test>::get(validator_id),
            Some(new_hotkey.clone())
        );
        assert_eq!(
            ValidatorColdkeyHotkey::<Test>::get(&coldkey),
            Some(new_hotkey.clone())
        );
        assert_eq!(
            ValidatorsData::<Test>::get(validator_id),
            expected_validator_data
        );
    })
}

#[test]
fn test_update_validator_hotkey_rejects_another_validator_hotkey_without_mutating_state() {
    new_test_ext().execute_with(|| {
        let first_coldkey = account(10_090);
        let first_hotkey = account(10_091);
        let second_coldkey = account(10_092);
        let second_hotkey = account(10_093);

        let first_validator_id = register_validator_for_rotation(&first_coldkey, &first_hotkey);
        let second_validator_id = register_validator_for_rotation(&second_coldkey, &second_hotkey);
        let first_current_hotkey = ValidatorIdHotkey::<Test>::get(first_validator_id).unwrap();
        let second_current_hotkey = ValidatorIdHotkey::<Test>::get(second_validator_id).unwrap();
        let tracked_accounts = vec![
            first_coldkey.clone(),
            first_current_hotkey.clone(),
            second_coldkey.clone(),
            second_current_hotkey.clone(),
        ];

        let first_before = validator_storage_snapshot(first_validator_id, &tracked_accounts);
        let second_before = validator_storage_snapshot(second_validator_id, &tracked_accounts);

        assert_err!(
            Network::update_validator_hotkey(
                RuntimeOrigin::signed(first_coldkey.clone()),
                first_validator_id,
                second_current_hotkey.clone(),
            ),
            Error::<Test>::HotkeyHasOwner
        );

        assert_eq!(
            validator_storage_snapshot(first_validator_id, &tracked_accounts),
            first_before
        );
        assert_eq!(
            validator_storage_snapshot(second_validator_id, &tracked_accounts),
            second_before
        );
        assert_eq!(
            HotkeyValidatorId::<Test>::get(&first_current_hotkey),
            Some(first_validator_id)
        );
        assert_eq!(
            HotkeyValidatorId::<Test>::get(&second_current_hotkey),
            Some(second_validator_id)
        );
    })
}

#[test]
fn test_update_validator_hotkey_rejects_coldkey_collisions_without_mutating_state() {
    new_test_ext().execute_with(|| {
        let first_coldkey = account(10_100);
        let first_hotkey = account(10_101);
        let second_coldkey = account(10_102);
        let second_hotkey = account(10_103);

        let first_validator_id = register_validator_for_rotation(&first_coldkey, &first_hotkey);
        let second_validator_id = register_validator_for_rotation(&second_coldkey, &second_hotkey);
        let first_current_hotkey = ValidatorIdHotkey::<Test>::get(first_validator_id).unwrap();
        let second_current_hotkey = ValidatorIdHotkey::<Test>::get(second_validator_id).unwrap();
        let tracked_accounts = vec![
            first_coldkey.clone(),
            first_current_hotkey,
            second_coldkey.clone(),
            second_current_hotkey,
        ];

        let assert_failed_hotkey_rotation_preserves_state =
            |new_hotkey: AccountId, expected_error: Error<Test>| {
                let first_before =
                    validator_storage_snapshot(first_validator_id, &tracked_accounts);
                let second_before =
                    validator_storage_snapshot(second_validator_id, &tracked_accounts);

                assert_err!(
                    Network::update_validator_hotkey(
                        RuntimeOrigin::signed(first_coldkey.clone()),
                        first_validator_id,
                        new_hotkey,
                    ),
                    expected_error
                );

                assert_eq!(
                    validator_storage_snapshot(first_validator_id, &tracked_accounts),
                    first_before
                );
                assert_eq!(
                    validator_storage_snapshot(second_validator_id, &tracked_accounts),
                    second_before
                );
            };

        assert_failed_hotkey_rotation_preserves_state(
            first_coldkey.clone(),
            Error::<Test>::ColdkeyMatchesHotkey,
        );
        assert_failed_hotkey_rotation_preserves_state(
            second_coldkey.clone(),
            Error::<Test>::HotkeyHasOwner,
        );
    })
}

#[test]
fn test_update_validator_hotkey_rejects_stale_reverse_index_without_mutating_state() {
    new_test_ext().execute_with(|| {
        let coldkey = account(10_110);
        let initial_hotkey = account(10_111);
        let new_hotkey = account(10_112);

        let validator_id = register_validator_for_rotation(&coldkey, &initial_hotkey);
        let current_hotkey = ValidatorIdHotkey::<Test>::get(validator_id).unwrap();
        let stale_validator_id = validator_id.saturating_add(1);

        HotkeyValidatorId::<Test>::insert(current_hotkey.clone(), stale_validator_id);

        let tracked_accounts = vec![coldkey.clone(), current_hotkey.clone(), new_hotkey.clone()];
        let before = validator_storage_snapshot(validator_id, &tracked_accounts);

        assert_err!(
            Network::update_validator_hotkey(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                new_hotkey,
            ),
            Error::<Test>::NotKeyOwner
        );

        assert_eq!(
            validator_storage_snapshot(validator_id, &tracked_accounts),
            before
        );
        assert_eq!(
            HotkeyValidatorId::<Test>::get(&current_hotkey),
            Some(stale_validator_id)
        );
    })
}

#[test]
fn test_update_validator_hotkey_allows_noop_without_mutating_state() {
    new_test_ext().execute_with(|| {
        let coldkey = account(10_120);
        let initial_hotkey = account(10_121);

        let validator_id = register_validator_for_rotation(&coldkey, &initial_hotkey);
        let current_hotkey = ValidatorIdHotkey::<Test>::get(validator_id).unwrap();
        let tracked_accounts = vec![coldkey.clone(), current_hotkey.clone()];
        let before = validator_storage_snapshot(validator_id, &tracked_accounts);

        assert_ok!(Network::update_validator_hotkey(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            current_hotkey,
        ));

        assert_eq!(
            validator_storage_snapshot(validator_id, &tracked_accounts),
            before
        );
    })
}

#[test]
fn test_register_validator_subnet_node() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let reward_rate = test_percent(1, 20); // 5%
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
            Network::do_register_subnet_node(
                RuntimeOrigin::signed(account(999)),
                current_id,
                subnet_id,
                None,
                Some(PeerInfo::<Test> {
                    peer_id: peer(1),
                    multiaddr: None,
                }),
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
            Network::do_register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                999,
                subnet_id,
                None,
                Some(PeerInfo::<Test> {
                    peer_id: peer(1),
                    multiaddr: None,
                }),
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
            Network::do_register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                current_id,
                subnet_id,
                None,
                Some(PeerInfo::<Test> {
                    peer_id: peer(1),
                    multiaddr: None,
                }),
                None,
                None,
                stake_amount,
                None,
                None,
                0,
            ),
            Error::<Test>::MaxBurnAmountExceeded
        );

        assert_ok!(Network::do_register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            current_id,
            subnet_id,
            None,
            Some(PeerInfo::<Test> {
                peer_id: peer(999),
                multiaddr: None,
            }),
            None,
            None,
            stake_amount,
            None,
            None,
            burn_amount + 100000000,
        ));
        let node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);
        let node = RegisteredSubnetNodesData::<Test>::get(subnet_id, node_id);
        assert_eq!(node.id, node_id);
        assert_eq!(node.validator_id, current_id);
        assert_eq!(
            node.peer_info,
            Some(PeerInfo::<Test> {
                peer_id: peer(999),
                multiaddr: None,
            })
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
        SubnetNodesData::<Test>::insert(
            subnet_id,
            subnet_node_id,
            SubnetNode::<Test> {
                id: subnet_node_id,
                validator_id,
                peer_info: Some(PeerInfo::<Test> {
                    peer_id: peer(1),
                    multiaddr: None,
                }),
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
        SubnetNodesData::<Test>::insert(
            subnet_id,
            subnet_node_id,
            SubnetNode::<Test> {
                id: subnet_node_id,
                validator_id,
                peer_info: Some(PeerInfo::<Test> {
                    peer_id: peer(2),
                    multiaddr: None,
                }),
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
        let reward_rate = test_percent(1, 20); // 5%
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
        let reward_rate = test_percent(1, 20); // 5%
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

#[test]
fn test_update_validator_delegate_reward_rate_validation_branches() {
    new_test_ext().execute_with(|| {
        let coldkey = account(10);
        let hotkey = account(11);
        let max_decrease = MaxRewardRateDecrease::<Test>::get();
        let reward_rate = max_decrease.saturating_mul(3);

        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));
        let validator_id = TotalValidatorIds::<Test>::get();

        assert_err!(
            Network::update_validator_delegate_reward_rate(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                MaxDelegateStakePercentage::<Test>::get().saturating_add(1),
            ),
            Error::<Test>::InvalidDelegateRewardRate
        );

        System::set_block_number(1);
        assert_err!(
            Network::update_validator_delegate_reward_rate(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                reward_rate.saturating_add(1),
            ),
            Error::<Test>::MaxRewardRateUpdates
        );

        System::set_block_number(NodeRewardRateUpdatePeriod::<Test>::get());
        assert_err!(
            Network::update_validator_delegate_reward_rate(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                reward_rate,
            ),
            Error::<Test>::NoDelegateRewardRateChange
        );

        let too_large_decrease = reward_rate.saturating_sub(max_decrease).saturating_sub(1);
        assert_err!(
            Network::update_validator_delegate_reward_rate(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                too_large_decrease,
            ),
            Error::<Test>::SurpassesMaxRewardRateDecrease
        );

        let allowed_decrease = reward_rate.saturating_sub(max_decrease);
        assert_ok!(Network::update_validator_delegate_reward_rate(
            RuntimeOrigin::signed(coldkey),
            validator_id,
            allowed_decrease,
        ));
        assert_eq!(
            ValidatorsData::<Test>::get(validator_id).delegate_reward_rate,
            allowed_decrease
        );
    })
}

#[test]
fn test_update_validator_identity() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let reward_rate = test_percent(1, 20); // 5%
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

        let name = to_bounded::<NetworkMaxVectorLength>("name");
        let url = to_bounded::<NetworkMaxUrlLength>("url");
        let image = to_bounded::<NetworkMaxUrlLength>("image");
        let discord = to_bounded::<NetworkMaxSocialIdLength>("discord");
        let x = to_bounded::<NetworkMaxSocialIdLength>("x");
        let telegram = to_bounded::<NetworkMaxSocialIdLength>("telegram");
        let github = to_bounded::<NetworkMaxUrlLength>("github");
        let hugging_face = to_bounded::<NetworkMaxUrlLength>("hugging_face");
        let description = to_bounded::<NetworkMaxVectorLength>("description");
        let misc = to_bounded::<NetworkMaxVectorLength>("misc");

        let identity: IdentityData<Test> = IdentityData::<Test> {
            name: Some(name.clone()),
            url: Some(url.clone()),
            image: Some(image.clone()),
            discord: Some(discord.clone()),
            x: Some(x.clone()),
            telegram: Some(telegram.clone()),
            github: Some(github.clone()),
            hugging_face: Some(hugging_face.clone()),
            description: Some(description.clone()),
            misc: Some(misc.clone()),
        };

        assert_ok!(Network::update_validator_identity(
            RuntimeOrigin::signed(coldkey.clone()),
            current_id,
            Some(identity),
        ));

        let v_data = ValidatorsData::<Test>::get(current_id);
        let v_identity = v_data.identity;
        assert!(v_identity.is_some());

        assert_eq!(v_identity.clone().unwrap().name, Some(name.clone()));
        assert_eq!(v_identity.clone().unwrap().url, Some(url.clone()));
        assert_eq!(v_identity.clone().unwrap().image, Some(image.clone()));
        assert_eq!(v_identity.clone().unwrap().discord, Some(discord.clone()));
        assert_eq!(v_identity.clone().unwrap().x, Some(x.clone()));
        assert_eq!(v_identity.clone().unwrap().telegram, Some(telegram.clone()));
        assert_eq!(v_identity.clone().unwrap().github, Some(github.clone()));
        assert_eq!(
            v_identity.clone().unwrap().hugging_face,
            Some(hugging_face.clone())
        );
        assert_eq!(
            v_identity.clone().unwrap().description,
            Some(description.clone())
        );
        assert_eq!(v_identity.clone().unwrap().misc, Some(misc.clone()));

        // Remove one identity parameter
        let identity: IdentityData<Test> = IdentityData::<Test> {
            name: Some(name.clone()),
            url: Some(url.clone()),
            image: Some(image.clone()),
            discord: Some(discord.clone()),
            x: None,
            telegram: Some(telegram.clone()),
            github: Some(github.clone()),
            hugging_face: Some(hugging_face.clone()),
            description: Some(description.clone()),
            misc: Some(misc.clone()),
        };

        assert_ok!(Network::update_validator_identity(
            RuntimeOrigin::signed(coldkey.clone()),
            current_id,
            Some(identity),
        ));

        let v_data = ValidatorsData::<Test>::get(current_id);
        let v_identity = v_data.identity;
        assert!(v_identity.is_some());

        assert_eq!(v_identity.clone().unwrap().name, Some(name.clone()));
        assert_eq!(v_identity.clone().unwrap().url, Some(url.clone()));
        assert_eq!(v_identity.clone().unwrap().image, Some(image.clone()));
        assert_eq!(v_identity.clone().unwrap().discord, Some(discord.clone()));
        assert_eq!(v_identity.clone().unwrap().x, None);
        assert_eq!(v_identity.clone().unwrap().telegram, Some(telegram.clone()));
        assert_eq!(v_identity.clone().unwrap().github, Some(github.clone()));
        assert_eq!(
            v_identity.clone().unwrap().hugging_face,
            Some(hugging_face.clone())
        );
        assert_eq!(
            v_identity.clone().unwrap().description,
            Some(description.clone())
        );
        assert_eq!(v_identity.clone().unwrap().misc, Some(misc.clone()));

        // Remove the identity
        assert_ok!(Network::update_validator_identity(
            RuntimeOrigin::signed(coldkey.clone()),
            current_id,
            None,
        ));

        let v_data = ValidatorsData::<Test>::get(current_id);
        let v_identity = v_data.identity;
        assert!(v_identity.is_none());
    })
}
