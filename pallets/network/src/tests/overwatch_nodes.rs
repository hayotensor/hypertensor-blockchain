use super::mock::*;
use crate::tests::test_utils::*;
use crate::{
    Error, MaxOverwatchNodes, MaxSubnetNodes, MaxSubnets, MinSubnetMinStake, MinSubnetNodes,
    OverwatchEpochLengthMultiplier, OverwatchMinAge, OverwatchMinStakeBalance,
    OverwatchNodeBlacklist, OverwatchNodeIdHotkey, OverwatchNodeIndex, OverwatchNodeStakeBalance,
    OverwatchNodeValidatorId, OverwatchNodeWeights, OverwatchNodes, OverwatchStakeWeightFactor,
    OverwatchSubnetWeights, OverwatchValidatorWhitelist, PeerId, PeerIdOverwatchNodeId,
    StakeCooldownEpochs, StakeUnbondingLedger, SubnetName, SubnetNodesData, SubnetState,
    TotalOverwatchNodeStakeBalance, TotalOverwatchNodeUids, TotalOverwatchNodes, TotalValidatorIds,
    ValidatorSubnetNodes,
};
use frame_support::traits::Currency;
use frame_support::{assert_err, assert_ok};
use sp_std::collections::{btree_map::BTreeMap, btree_set::BTreeSet};

//
//
//
//
//
//
//
// Overwatch Nodes
//
//
//
//
//
//
//
//

#[test]
fn test_register_overwatch_node() {
    new_test_ext().execute_with(|| {
        let amount = 100000000000000000000;

        let coldkey = account(1);
        let hotkey = account(2);
        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        assert_err!(
            Network::register_overwatch_node(RuntimeOrigin::signed(coldkey.clone()), amount,),
            Error::<Test>::OverwatchEpochIsZero
        );

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get();

        make_overwatch_qualified_v2(validator_id, overwatch_node_id);

        let init_total_overwatch_nodes = TotalOverwatchNodes::<Test>::get();

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get();

        assert_eq!(
            init_total_overwatch_nodes + 1,
            TotalOverwatchNodes::<Test>::get()
        );

        // assert_eq!(
        //     OverwatchNodes::<Test>::get(overwatch_node_id).unwrap().hotkey,
        //     hotkey.clone()
        // );
        // assert_eq!(
        //     OverwatchNodeIdHotkey::<Test>::get(overwatch_node_id),
        //     Some(hotkey.clone())
        // );
        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id),
            amount
        );
    });
}

#[test]
fn test_register_overwatch_node_blacklisted() {
    new_test_ext().execute_with(|| {
        let amount = 100000000000000000000;

        let coldkey = account(1);
        let hotkey = account(2);
        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));
        let validator_id = TotalValidatorIds::<Test>::get();

        OverwatchValidatorWhitelist::<Test>::insert(validator_id, false);

        assert_err!(
            Network::register_overwatch_node(RuntimeOrigin::signed(coldkey.clone()), amount,),
            Error::<Test>::ColdkeyBlacklisted
        );
    });
}

#[test]
fn test_register_overwatch_node_min_stake_error() {
    new_test_ext().execute_with(|| {
        let coldkey = account(1);
        let hotkey = account(2);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;
        make_overwatch_qualified_v2(validator_id, overwatch_node_id);

        assert_err!(
            Network::register_overwatch_node(
                RuntimeOrigin::signed(coldkey.clone()),
                OverwatchMinStakeBalance::<Test>::get() - 1,
            ),
            Error::<Test>::MinStakeNotReached
        );

        assert_err!(
            Network::register_overwatch_node(
                RuntimeOrigin::signed(coldkey.clone()),
                OverwatchMinStakeBalance::<Test>::get(),
            ),
            Error::<Test>::NotEnoughBalanceToStake
        );

        let _ =
            Balances::deposit_creating(&coldkey.clone(), OverwatchMinStakeBalance::<Test>::get());

        assert_err!(
            Network::register_overwatch_node(
                RuntimeOrigin::signed(coldkey.clone()),
                OverwatchMinStakeBalance::<Test>::get(),
            ),
            Error::<Test>::BalanceWithdrawalError
        );
    });
}

#[test]
fn test_register_overwatch_node_stake_failure_does_not_commit_partial_state_or_clean_validator_nodes(
) {
    new_test_ext().execute_with(|| {
        let coldkey_n = 10_030;
        let hotkey_n = 10_031;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);

        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            test_percent(1, 20),
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        increase_epochs(OverwatchEpochLengthMultiplier::<Test>::get() as u32);
        make_overwatch_qualified_v2(validator_id, coldkey_n);

        let stale_subnet_id = TotalOverwatchNodeUids::<Test>::get()
            .saturating_add(MaxSubnets::<Test>::get())
            .saturating_add(1);
        let mut stale_nodes = BTreeSet::new();
        stale_nodes.insert(TotalOverwatchNodeUids::<Test>::get().saturating_add(1));
        ValidatorSubnetNodes::<Test>::mutate(validator_id, |nodes| {
            nodes.insert(stale_subnet_id, stale_nodes);
        });

        let total_overwatch_node_uids = TotalOverwatchNodeUids::<Test>::get();
        let next_overwatch_node_id = total_overwatch_node_uids.saturating_add(1);
        let total_overwatch_nodes = TotalOverwatchNodes::<Test>::get();
        let overwatch_node_validator_exists =
            OverwatchNodeValidatorId::<Test>::contains_key(next_overwatch_node_id);
        let overwatch_node_exists = OverwatchNodes::<Test>::contains_key(next_overwatch_node_id);
        let overwatch_node_stake = OverwatchNodeStakeBalance::<Test>::get(next_overwatch_node_id);
        let total_overwatch_stake = TotalOverwatchNodeStakeBalance::<Test>::get();
        let validator_subnet_nodes = ValidatorSubnetNodes::<Test>::get(validator_id);

        assert_err!(
            Network::register_overwatch_node(
                RuntimeOrigin::signed(coldkey.clone()),
                OverwatchMinStakeBalance::<Test>::get(),
            ),
            Error::<Test>::NotEnoughBalanceToStake
        );

        assert_eq!(
            TotalOverwatchNodeUids::<Test>::get(),
            total_overwatch_node_uids
        );
        assert_eq!(TotalOverwatchNodes::<Test>::get(), total_overwatch_nodes);
        assert_eq!(
            OverwatchNodeValidatorId::<Test>::contains_key(next_overwatch_node_id),
            overwatch_node_validator_exists
        );
        assert_eq!(
            OverwatchNodes::<Test>::contains_key(next_overwatch_node_id),
            overwatch_node_exists
        );
        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(next_overwatch_node_id),
            overwatch_node_stake
        );
        assert_eq!(
            TotalOverwatchNodeStakeBalance::<Test>::get(),
            total_overwatch_stake
        );
        assert_eq!(
            ValidatorSubnetNodes::<Test>::get(validator_id),
            validator_subnet_nodes
        );
    });
}

#[test]
fn test_register_overwatch_node_errors() {
    new_test_ext().execute_with(|| {
        let amount = 100000000000000000000;

        let coldkey = account(1);
        let hotkey = account(2);

        let coldkey = account(1);
        let hotkey = account(2);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        // make_overwatch_qualified_v2(validator_id, overwatch_node_id);

        set_overwatch_epoch(1);

        TotalOverwatchNodes::<Test>::set(MaxOverwatchNodes::<Test>::get());
        assert_err!(
            Network::register_overwatch_node(RuntimeOrigin::signed(coldkey.clone()), amount,),
            Error::<Test>::MaxOverwatchNodes
        );

        TotalOverwatchNodes::<Test>::set(0);

        assert_err!(
            Network::register_overwatch_node(RuntimeOrigin::signed(coldkey.clone()), amount,),
            Error::<Test>::ColdkeyNotOverwatchQualified
        );

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;
        make_overwatch_qualified_v2(validator_id, overwatch_node_id);

        assert_err!(
            Network::register_overwatch_node(RuntimeOrigin::signed(coldkey.clone()), 1,),
            Error::<Test>::MinStakeNotReached
        );

        assert_err!(
            Network::register_overwatch_node(RuntimeOrigin::signed(coldkey.clone()), amount,),
            Error::<Test>::NotEnoughBalanceToStake
        );

        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000);
        assert_err!(
            Network::register_overwatch_node(RuntimeOrigin::signed(coldkey.clone()), amount,),
            Error::<Test>::BalanceWithdrawalError
        );

        let _ = Balances::deposit_creating(&coldkey.clone(), 500);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));
    });
}

#[test]
fn test_set_overwatch_peer_id_v2() {
    new_test_ext().execute_with(|| {
        // subnet
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let min_subnet_nodes = MinSubnetNodes::<Test>::get();
        let end = min_subnet_nodes;
        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        // overwatch
        let coldkey = account(1);
        let hotkey = account(2);
        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;

        make_overwatch_qualified_v2(validator_id, overwatch_node_id);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            stake_amount,
        ));

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get();
        let peer_id = peer(1);

        assert_ok!(Network::set_overwatch_node_peer_id(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            overwatch_node_id,
            peer_id.clone(),
        ));

        assert_eq!(
            PeerIdOverwatchNodeId::<Test>::get(subnet_id, peer_id.clone()),
            overwatch_node_id
        );

        let exists = OverwatchNodeIndex::<Test>::get(overwatch_node_id)
            .get(&subnet_id)
            .map_or(false, |x_peer_id| *x_peer_id == peer_id);
        assert!(exists);
    });
}

#[test]
fn test_update_overwatch_hotkey_override_and_clear() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "overwatch-hotkey-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            MinSubnetNodes::<Test>::get(),
            deposit_amount,
            stake_amount,
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();

        let coldkey = account(1);
        let validator_hotkey = account(2);
        let overwatch_hotkey = account(3);
        let _ = Balances::deposit_creating(&coldkey, OverwatchMinStakeBalance::<Test>::get() + 500);

        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_hotkey.clone(),
            test_percent(1, 20),
            None,
            None,
        ));
        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        let expected_overwatch_node_id = TotalOverwatchNodeUids::<Test>::get().saturating_add(1);
        make_overwatch_qualified_v2(validator_id, expected_overwatch_node_id);
        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            OverwatchMinStakeBalance::<Test>::get(),
        ));
        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get();

        assert_err!(
            Network::update_overwatch_hotkey(
                RuntimeOrigin::signed(account(99)),
                overwatch_node_id,
                Some(overwatch_hotkey.clone()),
            ),
            Error::<Test>::NotKeyOwner
        );

        assert_ok!(Network::update_overwatch_hotkey(
            RuntimeOrigin::signed(coldkey.clone()),
            overwatch_node_id,
            Some(overwatch_hotkey.clone()),
        ));
        assert_eq!(
            OverwatchNodeIdHotkey::<Test>::get(overwatch_node_id),
            Some(overwatch_hotkey.clone())
        );

        assert_err!(
            Network::set_overwatch_node_peer_id(
                RuntimeOrigin::signed(validator_hotkey.clone()),
                subnet_id,
                overwatch_node_id,
                peer(101),
            ),
            Error::<Test>::NotKeyOwner
        );
        assert_ok!(Network::set_overwatch_node_peer_id(
            RuntimeOrigin::signed(overwatch_hotkey),
            subnet_id,
            overwatch_node_id,
            peer(101),
        ));

        assert_ok!(Network::update_overwatch_hotkey(
            RuntimeOrigin::signed(coldkey),
            overwatch_node_id,
            None,
        ));
        assert_eq!(OverwatchNodeIdHotkey::<Test>::get(overwatch_node_id), None);
        assert_ok!(Network::set_overwatch_node_peer_id(
            RuntimeOrigin::signed(validator_hotkey),
            subnet_id,
            overwatch_node_id,
            peer(102),
        ));
    });
}

#[test]
fn test_set_overwatch_peer_id_errors() {
    new_test_ext().execute_with(|| {
        // overwatch
        let amount = 100000000000000000000;
        let coldkey = account(1);
        let hotkey = account(2);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        assert_err!(
            Network::register_overwatch_node(RuntimeOrigin::signed(coldkey.clone()), amount,),
            Error::<Test>::OverwatchEpochIsZero
        );

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;

        make_overwatch_qualified_v2(validator_id, overwatch_node_id);

        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        let peer_id = peer(1);

        let subnet_id = 999;

        assert_err!(
            Network::set_overwatch_node_peer_id(
                RuntimeOrigin::signed(coldkey.clone()),
                999,
                overwatch_node_id,
                peer_id.clone(),
            ),
            Error::<Test>::InvalidSubnetId
        );

        insert_subnet(subnet_id, SubnetState::Active, 0);

        assert_err!(
            Network::set_overwatch_node_peer_id(
                RuntimeOrigin::signed(account(999)),
                subnet_id,
                overwatch_node_id,
                peer_id.clone(),
            ),
            Error::<Test>::NotKeyOwner
        );

        let bad_peer_id = format!("2");
        let bad_peer: PeerId = PeerId(bad_peer_id.clone().into());

        assert_err!(
            Network::set_overwatch_node_peer_id(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                overwatch_node_id,
                bad_peer,
            ),
            Error::<Test>::InvalidPeerId
        );

        // add subnet to get existing peer ids
        // subnet
        let subnet_name: Vec<u8> = "subnet-name-999".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let min_subnet_nodes = MinSubnetNodes::<Test>::get();
        let end = min_subnet_nodes;
        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let subnet_id_key_offset = get_subnet_id_key_offset(subnet_id);

        let max_subnets = MaxSubnets::<Test>::get();
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let snn_hotkey = get_hotkey(subnet_id_key_offset, max_subnet_nodes, max_subnets, end);

        let subnet_node_data = SubnetNodesData::<Test>::try_get(subnet_id, end).unwrap();
        let snn_peer_id = subnet_node_data.peer_info.as_ref().unwrap().peer_id.clone();

        assert_err!(
            Network::set_overwatch_node_peer_id(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                overwatch_node_id,
                snn_peer_id.clone(),
            ),
            Error::<Test>::PeerIdExist
        );
    });
}

#[test]
fn test_remove_overwatch_node() {
    new_test_ext().execute_with(|| {
        // subnet
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let min_subnet_nodes = MinSubnetNodes::<Test>::get();
        let end = min_subnet_nodes;
        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        // overwatch
        let amount = 100000000000000000000;
        let coldkey = account(1);
        let hotkey = account(2);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;

        make_overwatch_qualified_v2(validator_id, validator_id);

        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        assert_err!(
            Network::remove_overwatch_node(RuntimeOrigin::signed(coldkey.clone()), 0),
            Error::<Test>::InvalidOverwatchNodeId
        );

        assert_err!(
            Network::remove_overwatch_node(RuntimeOrigin::signed(account(999)), 1),
            Error::<Test>::NotKeyOwner
        );

        let init_total_overwatch_nodes = TotalOverwatchNodes::<Test>::get();

        let peer_id = peer(1);

        assert_ok!(Network::set_overwatch_node_peer_id(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            overwatch_node_id,
            peer_id.clone(),
        ));

        assert_ok!(Network::remove_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            overwatch_node_id,
        ));

        assert_eq!(OverwatchNodes::<Test>::try_get(overwatch_node_id), Err(()));
        assert_eq!(
            init_total_overwatch_nodes - 1,
            TotalOverwatchNodes::<Test>::get()
        );
        assert_eq!(
            OverwatchNodeIdHotkey::<Test>::try_get(overwatch_node_id),
            Err(())
        );
        assert_eq!(
            PeerIdOverwatchNodeId::<Test>::try_get(subnet_id, peer_id.clone()),
            Err(())
        );
        let map = OverwatchNodeIndex::<Test>::take(overwatch_node_id);
        for (subnet_id, map_peer_id) in map {
            assert_ne!(peer_id.clone(), map_peer_id);
        }
    });
}

#[test]
fn test_equal_stake_equal_weights_v3() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let epoch = Network::get_current_overwatch_epoch_as_u32();

        let validator_id_1 = 1;
        let validator_id_2 = 2;

        // Setup
        manual_insert_validator(validator_id_1, validator_id_1, validator_id_1);
        manual_insert_validator(validator_id_2, validator_id_2, validator_id_2);

        let node_id_1 = insert_overwatch_node_v2(validator_id_1);
        let node_id_2 = insert_overwatch_node_v2(validator_id_2);
        set_overwatch_node_stake(1, 100);
        set_overwatch_node_stake(2, 100);

        submit_weight(epoch, subnet_id, node_id_1, test_percent(1, 2));
        submit_weight(epoch, subnet_id, node_id_2, test_percent(1, 2));

        let mut ostake_snapshot: BTreeMap<u32, u128> = BTreeMap::new();
        for n in 0..2 {
            let hotkey = account(n + 1);
            let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(n + 1);
            assert_ne!(overwatch_stake, 0);
            ostake_snapshot.insert(n + 1, overwatch_stake);
        }

        let block_weight = Network::calculate_overwatch_rewards();

        for n in 0..2 {
            let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(n + 1);

            if let Some(old_stake) = ostake_snapshot.get(&(n + 1)) {
                assert!(overwatch_stake > *old_stake);
            } else {
                assert!(false); // auto-fail
            }
        }

        let subnet_weight = OverwatchSubnetWeights::<Test>::get(epoch, subnet_id);

        assert_eq!(subnet_weight, Some(test_percent(1, 2)));

        let score_1 = OverwatchNodeWeights::<Test>::get(epoch, node_id_1);
        let score_2 = OverwatchNodeWeights::<Test>::get(epoch, node_id_2);

        // Same scores submitted, same rewards
        assert_eq!(score_1, score_2);
        assert_eq!(score_1, Some(test_percent(1, 2)));
        assert_eq!(score_2, Some(test_percent(1, 2)));

        let mut score_sum = 0;
        for (id, _) in OverwatchNodes::<Test>::iter() {
            let weight = OverwatchNodeWeights::<Test>::get(epoch, id);
            score_sum += weight.unwrap();
        }

        assert_eq!(score_sum, Network::percentage_factor_as_u128());
    });
}

#[test]
fn test_stake_no_dampening_effect() {
    new_test_ext().execute_with(|| {
        OverwatchStakeWeightFactor::<Test>::set(test_percent(9, 10));
        let subnet_id = 1;
        let epoch = Network::get_current_overwatch_epoch_as_u32();

        let validator_id_1 = 1;
        let validator_id_2 = 2;

        // Setup
        manual_insert_validator(validator_id_1, validator_id_1, validator_id_1);
        manual_insert_validator(validator_id_2, validator_id_2, validator_id_2);

        let node_id_1 = insert_overwatch_node_v2(validator_id_1);
        let node_id_2 = insert_overwatch_node_v2(validator_id_2);
        set_overwatch_node_stake(1, 90);
        set_overwatch_node_stake(2, 10);

        submit_weight(epoch, subnet_id, node_id_1, test_percent(1, 2));
        submit_weight(epoch, subnet_id, node_id_2, test_percent(1, 2));

        let mut ostake_snapshot: BTreeMap<u32, u128> = BTreeMap::new();
        for n in 0..2 {
            let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(n + 1);
            assert_ne!(overwatch_stake, 0);
            ostake_snapshot.insert(n + 1, overwatch_stake);
        }

        let block_weight = Network::calculate_overwatch_rewards();

        for n in 0..2 {
            let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(n + 1);

            if let Some(old_stake) = ostake_snapshot.get(&(n + 1)) {
                assert!(overwatch_stake > *old_stake);
            } else {
                assert!(false); // auto-fail
            }
        }

        let subnet_weight = OverwatchSubnetWeights::<Test>::get(epoch, subnet_id);

        // Both users submitted the same score, subnet should be the score
        assert_eq!(subnet_weight, Some(test_percent(1, 2).saturating_sub(1)));

        let score_1 = OverwatchNodeWeights::<Test>::get(epoch, node_id_1);
        let score_2 = OverwatchNodeWeights::<Test>::get(epoch, node_id_2);

        // Both users submitted the same score, each node score should be equal
        assert_eq!(score_1, score_2);

        let mut score_sum = 0;
        for (id, _) in OverwatchNodes::<Test>::iter() {
            let weight = OverwatchNodeWeights::<Test>::get(epoch, id);
            score_sum += weight.unwrap();
        }

        assert_eq!(score_sum, Network::percentage_factor_as_u128());
    });
}

#[test]
fn test_two_noces_same_stake_dif_weights_v3() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let epoch = Network::get_current_overwatch_epoch_as_u32();

        let validator_id_1 = 1;
        let validator_id_2 = 2;

        // Setup
        manual_insert_validator(validator_id_1, validator_id_1, validator_id_1);
        manual_insert_validator(validator_id_2, validator_id_2, validator_id_2);

        let node_id_1 = insert_overwatch_node_v2(validator_id_1);
        let node_id_2 = insert_overwatch_node_v2(validator_id_2);
        set_overwatch_node_stake(1, 50);
        set_overwatch_node_stake(2, 50);

        submit_weight(epoch, subnet_id, node_id_1, test_percent(1, 2));
        submit_weight(epoch, subnet_id, node_id_2, 100);

        let mut ostake_snapshot: BTreeMap<u32, u128> = BTreeMap::new();
        for n in 0..2 {
            let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(n + 1);
            assert_ne!(overwatch_stake, 0);
            ostake_snapshot.insert(n + 1, overwatch_stake);
        }

        let block_weight = Network::calculate_overwatch_rewards();

        for n in 0..2 {
            let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(n + 1);

            if let Some(old_stake) = ostake_snapshot.get(&(n + 1)) {
                assert!(overwatch_stake > *old_stake);
            } else {
                assert!(false); // auto-fail
            }
        }

        let subnet_weight = OverwatchSubnetWeights::<Test>::get(epoch, subnet_id);

        assert_eq!(subnet_weight, Some((test_percent(1, 2) + 100) / 2));

        let score_1 = OverwatchNodeWeights::<Test>::get(epoch, node_id_1);
        let score_2 = OverwatchNodeWeights::<Test>::get(epoch, node_id_2);

        // Nodes have same stake weight, only 2 nodes, should be same scores
        assert_eq!(Some(score_1), Some(score_2));

        let mut score_sum = 0;
        for (id, _) in OverwatchNodes::<Test>::iter() {
            let weight = OverwatchNodeWeights::<Test>::get(epoch, id);
            score_sum += weight.unwrap();
        }

        assert_eq!(score_sum, Network::percentage_factor_as_u128());
    });
}

#[test]
fn test_multiple_subnets_score_accumulation_v3() {
    new_test_ext().execute_with(|| {
        OverwatchStakeWeightFactor::<Test>::set(test_percent(9, 10));
        let subnet_id_1 = 1;
        let subnet_id_2 = 2;
        let epoch = Network::get_current_overwatch_epoch_as_u32();

        let validator_id_1 = 1;
        let validator_id_2 = 2;

        // Setup
        manual_insert_validator(validator_id_1, validator_id_1, validator_id_1);
        manual_insert_validator(validator_id_2, validator_id_2, validator_id_2);

        let node_id_1 = insert_overwatch_node_v2(validator_id_1);
        let node_id_2 = insert_overwatch_node_v2(validator_id_2);
        set_overwatch_node_stake(1, 50);
        set_overwatch_node_stake(2, 100);

        // Subnet 1
        submit_weight(epoch, subnet_id_1, node_id_1, test_percent(1, 2));
        submit_weight(epoch, subnet_id_1, node_id_2, test_percent(1, 2));
        // Subnet 2
        submit_weight(epoch, subnet_id_2, node_id_1, test_percent(1, 2));
        submit_weight(epoch, subnet_id_2, node_id_2, test_percent(3, 5)); // Node 2 slightly deviates

        let mut ostake_snapshot: BTreeMap<u32, u128> = BTreeMap::new();
        for n in 0..2 {
            let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(n + 1);
            assert_ne!(overwatch_stake, 0);
            ostake_snapshot.insert(n + 1, overwatch_stake);
        }

        let block_weight = Network::calculate_overwatch_rewards();

        for n in 0..2 {
            let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(n + 1);

            if let Some(old_stake) = ostake_snapshot.get(&(n + 1)) {
                assert!(overwatch_stake > *old_stake);
            } else {
                assert!(false); // auto-fail
            }
        }

        let subnet_weight_1 = OverwatchSubnetWeights::<Test>::get(epoch, subnet_id_1);
        let subnet_weight_2 = OverwatchSubnetWeights::<Test>::get(epoch, subnet_id_2);

        assert_eq!(subnet_weight_1, Some(test_percent(1, 2).saturating_sub(1))); // Rounding err
        assert_eq!(subnet_weight_2, Some(565108967975413320)); // Rounding err

        let score_1 = OverwatchNodeWeights::<Test>::get(epoch, node_id_1);
        let score_2 = OverwatchNodeWeights::<Test>::get(epoch, node_id_2);

        // 2 has higher stake weight
        assert!(score_2 > score_1);

        let mut score_sum = 0;
        for (id, _) in OverwatchNodes::<Test>::iter() {
            let weight = OverwatchNodeWeights::<Test>::get(epoch, id);
            score_sum += weight.unwrap();
        }

        assert!(
            score_sum <= Network::percentage_factor_as_u128()
                && score_sum.abs_diff(Network::percentage_factor_as_u128()) <= 10
        );
    });
}

#[test]
fn test_multiple_subnets_score_accumulation_v3_2() {
    new_test_ext().execute_with(|| {
        let subnet_id_1 = 1;
        let subnet_id_2 = 2;
        let epoch = Network::get_current_overwatch_epoch_as_u32();

        let validator_id_1 = 1;
        let validator_id_2 = 2;

        // Setup
        manual_insert_validator(validator_id_1, validator_id_1, validator_id_1);
        manual_insert_validator(validator_id_2, validator_id_2, validator_id_2);

        let node_id_1 = insert_overwatch_node_v2(validator_id_1);
        let node_id_2 = insert_overwatch_node_v2(validator_id_2);
        set_overwatch_node_stake(1, 100);
        set_overwatch_node_stake(2, 50);

        // Subnet 1
        submit_weight(epoch, subnet_id_1, node_id_1, test_percent(1, 2));
        submit_weight(epoch, subnet_id_1, node_id_2, test_percent(1, 2));
        // Subnet 2
        submit_weight(epoch, subnet_id_2, node_id_1, test_percent(1, 2));
        submit_weight(epoch, subnet_id_2, node_id_2, test_percent(3, 5)); // Node 2 slightly deviates

        let mut ostake_snapshot: BTreeMap<u32, u128> = BTreeMap::new();
        for n in 0..2 {
            let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(n + 1);
            assert_ne!(overwatch_stake, 0);
            ostake_snapshot.insert(n + 1, overwatch_stake);
        }

        let block_weight = Network::calculate_overwatch_rewards();

        for n in 0..2 {
            let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(n + 1);

            if let Some(old_stake) = ostake_snapshot.get(&(n + 1)) {
                assert!(overwatch_stake > *old_stake);
            } else {
                assert!(false); // auto-fail
            }
        }

        let score_1 = OverwatchNodeWeights::<Test>::get(epoch, node_id_1);
        let score_2 = OverwatchNodeWeights::<Test>::get(epoch, node_id_2);

        // 1 has higher stake weight
        assert!(score_1 > score_2);

        let mut score_sum = 0;
        for (id, _) in OverwatchNodes::<Test>::iter() {
            let weight = OverwatchNodeWeights::<Test>::get(epoch, id);
            score_sum += weight.unwrap();
        }

        assert!(
            score_sum <= Network::percentage_factor_as_u128()
                && score_sum.abs_diff(Network::percentage_factor_as_u128()) <= 10
        );
    });
}

#[test]
fn test_multiple_subnets_score_accumulation_v3_2_v2() {
    new_test_ext().execute_with(|| {
        let subnet_id_1 = 1;
        let subnet_id_2 = 2;
        let epoch = Network::get_current_overwatch_epoch_as_u32();

        let validator_id_1 = 1;
        let validator_id_2 = 2;

        // Setup
        manual_insert_validator(validator_id_1, validator_id_1, validator_id_1);
        manual_insert_validator(validator_id_2, validator_id_2, validator_id_2);

        let node_id_1 = insert_overwatch_node_v2(validator_id_1);
        let node_id_2 = insert_overwatch_node_v2(validator_id_2);
        set_overwatch_node_stake(1, 100);
        set_overwatch_node_stake(2, 50);

        // Subnet 1
        submit_weight(epoch, subnet_id_1, node_id_1, test_percent(1, 2));
        submit_weight(epoch, subnet_id_1, node_id_2, test_percent(1, 2));
        // Subnet 2
        submit_weight(epoch, subnet_id_2, node_id_1, test_percent(1, 2));
        submit_weight(epoch, subnet_id_2, node_id_2, test_percent(3, 5)); // Node 2 slightly deviates

        let mut ostake_snapshot: BTreeMap<u32, u128> = BTreeMap::new();
        for n in 0..2 {
            let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(n + 1);
            assert_ne!(overwatch_stake, 0);
            ostake_snapshot.insert(n + 1, overwatch_stake);
        }

        let block_weight = Network::calculate_overwatch_rewards();

        for n in 0..2 {
            let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(n + 1);

            if let Some(old_stake) = ostake_snapshot.get(&(n + 1)) {
                assert!(overwatch_stake > *old_stake);
            } else {
                assert!(false); // auto-fail
            }
        }

        let score_1 = OverwatchNodeWeights::<Test>::get(epoch, node_id_1);
        let score_2 = OverwatchNodeWeights::<Test>::get(epoch, node_id_2);

        // 1 has higher stake weight
        assert!(score_1 > score_2);

        let mut score_sum = 0;
        for (id, _) in OverwatchNodes::<Test>::iter() {
            let weight = OverwatchNodeWeights::<Test>::get(epoch, id);
            score_sum += weight.unwrap();
        }

        assert!(
            score_sum <= Network::percentage_factor_as_u128()
                && score_sum.abs_diff(Network::percentage_factor_as_u128()) <= 10
        );
    });
}

#[test]
fn test_multiple_subnets_check_percent_acccuracy() {
    new_test_ext().execute_with(|| {
        let subnet_id_1 = 1;
        let subnet_id_2 = 2;
        let subnet_id_3 = 3;
        let subnet_id_4 = 4;
        let subnet_id_5 = 5;
        let epoch = Network::get_current_overwatch_epoch_as_u32();

        let validator_id_1 = 1;
        let validator_id_2 = 2;
        let validator_id_3 = 3;
        let validator_id_4 = 4;
        let validator_id_5 = 5;
        let validator_id_6 = 6;
        let validator_id_7 = 7;
        let validator_id_8 = 8;

        // Setup
        manual_insert_validator(validator_id_1, validator_id_1, validator_id_1);
        manual_insert_validator(validator_id_2, validator_id_2, validator_id_2);
        manual_insert_validator(validator_id_3, validator_id_3, validator_id_3);
        manual_insert_validator(validator_id_4, validator_id_4, validator_id_4);
        manual_insert_validator(validator_id_5, validator_id_5, validator_id_5);
        manual_insert_validator(validator_id_6, validator_id_6, validator_id_6);
        manual_insert_validator(validator_id_7, validator_id_7, validator_id_7);
        manual_insert_validator(validator_id_8, validator_id_8, validator_id_8);

        // --- Generate a bunch of subnets, nodes, and entries and ensure ~1.0
        let node_id_1 = insert_overwatch_node_v2(validator_id_1);
        let node_id_2 = insert_overwatch_node_v2(validator_id_2);
        let node_id_3 = insert_overwatch_node_v2(validator_id_3);
        let node_id_4 = insert_overwatch_node_v2(validator_id_4);
        let node_id_5 = insert_overwatch_node_v2(validator_id_5);
        let node_id_6 = insert_overwatch_node_v2(validator_id_6);
        let node_id_7 = insert_overwatch_node_v2(validator_id_7);
        let node_id_8 = insert_overwatch_node_v2(validator_id_8);

        set_overwatch_node_stake(1, 100);
        set_overwatch_node_stake(2, 50);
        set_overwatch_node_stake(3, 25);
        set_overwatch_node_stake(4, 500);
        set_overwatch_node_stake(5, 200);
        set_overwatch_node_stake(6, 340);
        set_overwatch_node_stake(7, 1);
        set_overwatch_node_stake(8, 9);

        // Subnet 1
        submit_weight(epoch, subnet_id_1, node_id_1, test_percent(1, 2));
        submit_weight(epoch, subnet_id_1, node_id_2, test_percent(2, 5));
        submit_weight(epoch, subnet_id_1, node_id_3, test_percent(3, 5));
        submit_weight(epoch, subnet_id_1, node_id_4, test_percent(1, 2));
        submit_weight(epoch, subnet_id_1, node_id_5, test_percent(2, 5));
        submit_weight(epoch, subnet_id_1, node_id_6, test_percent(3, 5));
        submit_weight(epoch, subnet_id_1, node_id_7, test_percent(3, 5));
        submit_weight(epoch, subnet_id_1, node_id_8, test_percent(3, 10));
        // Subnet 2
        submit_weight(epoch, subnet_id_2, node_id_1, test_percent(1, 2));
        submit_weight(epoch, subnet_id_2, node_id_2, test_percent(3, 5));
        submit_weight(epoch, subnet_id_2, node_id_3, test_percent(4, 5));
        submit_weight(epoch, subnet_id_2, node_id_4, test_percent(9, 10));
        submit_weight(epoch, subnet_id_2, node_id_5, test_percent(3, 5));
        submit_weight(epoch, subnet_id_2, node_id_6, test_percent(4, 5));
        submit_weight(epoch, subnet_id_2, node_id_7, test_percent(9, 10));
        submit_weight(epoch, subnet_id_2, node_id_8, test_percent(3, 5));
        // Subnet 3
        submit_weight(epoch, subnet_id_3, node_id_1, test_percent(1, 2));
        submit_weight(epoch, subnet_id_3, node_id_2, test_percent(3, 5));
        submit_weight(epoch, subnet_id_3, node_id_3, test_percent(4, 5));
        submit_weight(epoch, subnet_id_3, node_id_4, test_percent(9, 10));
        submit_weight(epoch, subnet_id_3, node_id_5, test_percent(3, 5));
        submit_weight(epoch, subnet_id_3, node_id_6, test_percent(4, 5));
        submit_weight(epoch, subnet_id_3, node_id_7, test_percent(9, 10));
        submit_weight(epoch, subnet_id_3, node_id_8, test_percent(3, 5));
        // Subnet 4
        submit_weight(epoch, subnet_id_4, node_id_1, test_percent(1, 2));
        submit_weight(epoch, subnet_id_4, node_id_2, test_percent(3, 5));
        submit_weight(epoch, subnet_id_4, node_id_3, test_percent(4, 5));
        submit_weight(epoch, subnet_id_4, node_id_4, test_percent(9, 10));
        submit_weight(epoch, subnet_id_4, node_id_5, test_percent(3, 5));
        submit_weight(epoch, subnet_id_4, node_id_6, test_percent(4, 5));
        submit_weight(epoch, subnet_id_4, node_id_7, test_percent(9, 10));
        submit_weight(epoch, subnet_id_4, node_id_8, test_percent(3, 5));
        // Subnet 5
        submit_weight(epoch, subnet_id_5, node_id_1, test_percent(1, 2));
        submit_weight(epoch, subnet_id_5, node_id_2, test_percent(3, 5));
        submit_weight(epoch, subnet_id_5, node_id_3, test_percent(4, 5));
        submit_weight(epoch, subnet_id_5, node_id_4, test_percent(9, 10));
        submit_weight(epoch, subnet_id_5, node_id_5, test_percent(3, 5));
        submit_weight(epoch, subnet_id_5, node_id_6, test_percent(4, 5));
        submit_weight(epoch, subnet_id_5, node_id_7, test_percent(9, 10));
        submit_weight(epoch, subnet_id_5, node_id_8, test_percent(3, 5));

        let mut ostake_snapshot: BTreeMap<u32, u128> = BTreeMap::new();
        for n in 0..8 {
            let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(n + 1);
            assert_ne!(overwatch_stake, 0);
            ostake_snapshot.insert(n + 1, overwatch_stake);
        }

        let block_weight = Network::calculate_overwatch_rewards();

        for n in 0..8 {
            let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(n + 1);

            if let Some(old_stake) = ostake_snapshot.get(&(n + 1)) {
                assert!(overwatch_stake > *old_stake);
            } else {
                assert!(false); // auto-fail
            }
        }

        // let subnet_weight_1 = OverwatchSubnetWeights::<Test>::get(epoch, subnet_id_1);
        // let subnet_weight_2 = OverwatchSubnetWeights::<Test>::get(epoch, subnet_id_2);
        // let subnet_weight_3 = OverwatchSubnetWeights::<Test>::get(epoch, subnet_id_3);
        // let subnet_weight_4 = OverwatchSubnetWeights::<Test>::get(epoch, subnet_id_4);
        // let subnet_weight_5 = OverwatchSubnetWeights::<Test>::get(epoch, subnet_id_5);

        let mut score_sum = 0;
        let mut nodes = 0;
        for (id, _) in OverwatchNodes::<Test>::iter() {
            nodes += 1;
            let weight = OverwatchNodeWeights::<Test>::get(epoch, id);
            score_sum += weight.unwrap();
        }

        assert_eq!(nodes, 8);
        assert!(
            score_sum <= Network::percentage_factor_as_u128()
                && score_sum.abs_diff(Network::percentage_factor_as_u128()) <= 10
        );
    });
}

#[test]
fn test_add_to_overwatch_stake() {
    new_test_ext().execute_with(|| {
        let amount = 100000000000000000000;

        let coldkey = account(1);
        let hotkey = account(2);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        increase_epochs((OverwatchEpochLengthMultiplier::<Test>::get() as u32));

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;

        make_overwatch_qualified_v2(validator_id, overwatch_node_id);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        let increase_amount = 100000000000000000000;
        let _ = Balances::deposit_creating(&coldkey.clone(), increase_amount);

        let prev_account_balance = OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id);
        let prev_total_overwatch_balance = TotalOverwatchNodeStakeBalance::<Test>::get();

        assert_ok!(Network::add_overwatch_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            overwatch_node_id,
            increase_amount,
        ));

        assert_eq!(
            prev_account_balance + increase_amount,
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id)
        );

        assert_eq!(
            prev_total_overwatch_balance + increase_amount,
            TotalOverwatchNodeStakeBalance::<Test>::get()
        );

        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id),
            amount + increase_amount
        );
    });
}

#[test]
fn test_add_to_overwatch_stake_errors() {
    new_test_ext().execute_with(|| {
        let amount = 100000000000000000000;

        let coldkey = account(1);
        let hotkey = account(2);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;

        make_overwatch_qualified_v2(validator_id, overwatch_node_id);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        let increase_amount = 100000000000000000000;

        assert_err!(
            Network::add_overwatch_node_stake(
                RuntimeOrigin::signed(coldkey.clone()),
                overwatch_node_id,
                increase_amount,
            ),
            Error::<Test>::NotEnoughBalanceToStake
        );

        let _ = Balances::deposit_creating(&coldkey.clone(), increase_amount);

        assert_err!(
            Network::add_overwatch_node_stake(
                RuntimeOrigin::signed(coldkey.clone()),
                overwatch_node_id,
                increase_amount + 500,
            ),
            Error::<Test>::BalanceWithdrawalError
        );
    });
}

#[test]
fn test_add_to_remove_overwatch_stake() {
    new_test_ext().execute_with(|| {
        let amount = 100000000000000000000;

        let coldkey = account(1);
        let hotkey = account(2);
        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;

        make_overwatch_qualified_v2(validator_id, overwatch_node_id);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        let increase_amount = 100000000000000000000;
        let _ = Balances::deposit_creating(&coldkey.clone(), increase_amount);

        assert_ok!(Network::add_overwatch_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            overwatch_node_id,
            increase_amount,
        ));

        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id),
            amount + increase_amount
        );

        let remove_amount = 50000000000000000000;

        let starting_balance = Balances::free_balance(&coldkey.clone());

        let prev_account_balance = OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id);
        let prev_total_overwatch_balance = TotalOverwatchNodeStakeBalance::<Test>::get();

        assert_ok!(Network::remove_overwatch_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            overwatch_node_id,
            remove_amount,
        ));

        assert_eq!(
            prev_account_balance - remove_amount,
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id)
        );
        assert_eq!(
            prev_total_overwatch_balance - remove_amount,
            TotalOverwatchNodeStakeBalance::<Test>::get()
        );

        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id),
            amount + increase_amount - remove_amount
        );

        assert_eq!(starting_balance, Balances::free_balance(&coldkey.clone()));
    });
}

#[test]
fn test_add_to_remove_overwatch_stake_unbond() {
    new_test_ext().execute_with(|| {
        let amount = 100000000000000000000;

        let coldkey = account(1);
        let hotkey = account(2);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        increase_epochs((OverwatchEpochLengthMultiplier::<Test>::get() as u32));

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;

        make_overwatch_qualified_v2(validator_id, overwatch_node_id);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        let increase_amount = 100000000000000000000;
        let _ = Balances::deposit_creating(&coldkey.clone(), increase_amount);

        assert_ok!(Network::add_overwatch_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            overwatch_node_id,
            increase_amount,
        ));

        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id),
            amount + increase_amount
        );

        let remove_amount = 50000000000000000000;

        let starting_balance = Balances::free_balance(&coldkey.clone());
        let block = System::block_number();

        assert_ok!(Network::remove_overwatch_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            overwatch_node_id,
            remove_amount,
        ));

        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id),
            amount + increase_amount - remove_amount
        );

        assert_eq!(starting_balance, Balances::free_balance(&coldkey.clone()));

        let unbondings: BTreeMap<u32, u128> = StakeUnbondingLedger::<Test>::get(coldkey.clone());
        assert_eq!(unbondings.len(), 1);
        let (ledger_block, ledger_balance) = unbondings.iter().next().unwrap();
        assert_eq!(
            *ledger_block,
            &block + StakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );
        assert_eq!(*ledger_balance, remove_amount);

        System::set_block_number(block + StakeCooldownEpochs::<Test>::get() * EpochLength::get());

        let starting_balance = Balances::free_balance(&coldkey.clone());

        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            coldkey.clone()
        )));

        assert_eq!(
            Balances::free_balance(&coldkey.clone()),
            starting_balance + remove_amount
        );

        let unbondings: BTreeMap<u32, u128> = StakeUnbondingLedger::<Test>::get(coldkey.clone());
        assert_eq!(unbondings.len(), 0);
    });
}

#[test]
fn test_remove_overwatch_stake_after_removing_overwatch_node() {
    new_test_ext().execute_with(|| {
        let amount = 100000000000000000000;

        let coldkey = account(1);
        let hotkey = account(2);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        increase_epochs((OverwatchEpochLengthMultiplier::<Test>::get() as u32) + 1);

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;

        make_overwatch_qualified_v2(validator_id, overwatch_node_id);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        let increase_amount = 100000000000000000000;
        let _ = Balances::deposit_creating(&coldkey.clone(), increase_amount);

        assert_ok!(Network::add_overwatch_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            overwatch_node_id,
            increase_amount,
        ));

        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id),
            amount + increase_amount
        );

        assert_ok!(Network::remove_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            overwatch_node_id,
        ));

        let remove_amount = 50000000000000000000;

        let starting_balance = Balances::free_balance(&coldkey.clone());
        assert_ok!(Network::remove_overwatch_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            overwatch_node_id,
            remove_amount,
        ));

        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id),
            amount + increase_amount - remove_amount
        );
        assert_eq!(starting_balance, Balances::free_balance(&coldkey.clone()));
    });
}

#[test]
fn test_add_to_remove_overwatch_stake_errors() {
    new_test_ext().execute_with(|| {
        let amount = 100000000000000000000;

        let coldkey = account(1);
        let hotkey = account(2);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        increase_epochs((OverwatchEpochLengthMultiplier::<Test>::get() as u32));

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;

        make_overwatch_qualified_v2(validator_id, overwatch_node_id);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        let increase_amount = 100000000000000000000;
        let _ = Balances::deposit_creating(&coldkey.clone(), increase_amount);

        assert_ok!(Network::add_overwatch_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            overwatch_node_id,
            increase_amount,
        ));

        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id),
            amount + increase_amount
        );

        assert_err!(
            Network::remove_overwatch_node_stake(
                RuntimeOrigin::signed(coldkey.clone()),
                overwatch_node_id,
                0,
            ),
            Error::<Test>::AmountZero
        );

        assert_err!(
            Network::remove_overwatch_node_stake(
                RuntimeOrigin::signed(coldkey.clone()),
                overwatch_node_id,
                amount + increase_amount + increase_amount,
            ),
            Error::<Test>::NotEnoughStakeToWithdraw
        );

        assert_err!(
            Network::remove_overwatch_node_stake(
                RuntimeOrigin::signed(coldkey.clone()),
                overwatch_node_id,
                amount + increase_amount,
            ),
            Error::<Test>::MinStakeNotReached
        );
    });
}

#[test]
fn test_zero_score() {
    new_test_ext().execute_with(|| {
        OverwatchStakeWeightFactor::<Test>::set(test_percent(9, 10));
        let subnet_id = 1;
        let epoch = Network::get_current_overwatch_epoch_as_u32();

        // Setup
        let node_id_1 = insert_overwatch_node(1, 1);
        let node_id_2 = insert_overwatch_node(2, 2);
        set_overwatch_node_stake(1, 90);
        set_overwatch_node_stake(2, 10);

        submit_weight(epoch, subnet_id, node_id_1, 0);
        submit_weight(
            epoch,
            subnet_id,
            node_id_2,
            Network::percentage_factor_as_u128(),
        );

        let block_weight = Network::calculate_overwatch_rewards();

        let subnet_weight = OverwatchSubnetWeights::<Test>::get(epoch, subnet_id);

        // Score should be 0.1
        assert_eq!(subnet_weight, Some(121585365354349706));

        let score_1 = OverwatchNodeWeights::<Test>::get(epoch, node_id_1);
        let score_2 = OverwatchNodeWeights::<Test>::get(epoch, node_id_2);

        assert_eq!(score_1, Some(878414634645650299));
        assert_eq!(score_2, Some(121585365354349700));

        let mut score_sum = 0;
        let mut nodes = 0;
        for (id, _) in OverwatchNodes::<Test>::iter() {
            nodes += 1;
            let weight = OverwatchNodeWeights::<Test>::get(epoch, id);
            score_sum += weight.unwrap();
        }

        assert_eq!(nodes, 2);
        assert!(
            score_sum <= Network::percentage_factor_as_u128()
                && score_sum.abs_diff(Network::percentage_factor_as_u128()) <= 10
        );
    });
}
