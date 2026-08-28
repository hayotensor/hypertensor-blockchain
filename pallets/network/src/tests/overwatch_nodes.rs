use super::mock::*;
use crate::tests::test_utils::*;
use crate::{
    ActiveOverwatchEpochLengthMultiplier, Error, LastFinalizedOverwatchEpoch, MaxOverwatchNodes,
    MaxSubnetNodes, MaxSubnets, MinSubnetMinStake, MinSubnetNodes, OverwatchEpochLengthMultiplier,
    OverwatchEpochSettlementSnapshots, OverwatchEpochStartBlock, OverwatchMinAge,
    OverwatchMinStakeBalance, OverwatchNode, OverwatchNodeIdHotkey, OverwatchNodeIndex,
    OverwatchNodeStakeBalance, OverwatchNodeValidatorId, OverwatchNodeWeights, OverwatchNodes,
    OverwatchStakeWeightFactor, OverwatchSubnetWeights, OverwatchValidatorWhitelist, PeerId,
    PeerIdOverwatchNodeId, PendingOverwatchSettlement, PendingOverwatchSettlementData,
    StakeCooldownEpochs, StakeUnbondingLedger, SubnetName, SubnetNodesData, SubnetState,
    TotalOverwatchNodeStakeBalance, TotalOverwatchNodeUids, TotalOverwatchNodes, TotalValidatorIds,
    ValidatorOverwatchNodeId, ValidatorSubnetNodes, NETWORK_OVERWATCH_SETTLEMENT_SLOT,
};
use frame_support::traits::{Currency, OnInitialize};
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

fn setup_qualified_overwatch_validator(
    coldkey_n: u32,
    hotkey_n: u32,
    funding: u128,
) -> (u32, AccountId) {
    let coldkey = account(coldkey_n);
    assert_ok!(Network::do_register_validator(
        RuntimeOrigin::signed(coldkey.clone()),
        account(hotkey_n),
        test_percent(1, 20),
        None,
        None,
    ));

    let validator_id = TotalValidatorIds::<Test>::get();
    OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);
    make_overwatch_qualified_v2(validator_id);
    let _ = Balances::deposit_creating(&coldkey, funding);

    (validator_id, coldkey)
}

/// Run a minimal two-node settlement whose opposing reveals expose the normalized stake shares.
/// If the first node reveals 100% and the second reveals 0%, both the subnet result and the final
/// node scores are the powered stake proportions (subject only to fixed-point flooring).
fn run_two_node_overwatch_stake_weight_case(
    exponent: u128,
    first_stake: u128,
    second_stake: u128,
) -> (u128, u128, u128) {
    new_test_ext().execute_with(|| {
        OverwatchStakeWeightFactor::<Test>::set(exponent);
        let epoch = Network::get_current_overwatch_epoch_as_u32();
        let subnet_id = 1;

        manual_insert_validator(1, 101, 201);
        manual_insert_validator(2, 102, 202);
        let first_node_id = insert_overwatch_node_v2(1);
        let second_node_id = insert_overwatch_node_v2(2);
        set_overwatch_node_stake(first_node_id, first_stake);
        set_overwatch_node_stake(second_node_id, second_stake);

        submit_weight(
            epoch,
            subnet_id,
            first_node_id,
            Network::percentage_factor_as_u128(),
        );
        submit_weight(epoch, subnet_id, second_node_id, 0);
        queue_overwatch_settlement(epoch);
        Network::calculate_overwatch_rewards();

        (
            OverwatchSubnetWeights::<Test>::get(epoch, subnet_id)
                .expect("the revealed subnet must be finalized"),
            OverwatchNodeWeights::<Test>::get(epoch, first_node_id).unwrap_or_default(),
            OverwatchNodeWeights::<Test>::get(epoch, second_node_id).unwrap_or_default(),
        )
    })
}

fn assert_normalized_pair(first: u128, second: u128) {
    let percentage_factor = Network::percentage_factor_as_u128();
    assert!(first <= percentage_factor);
    assert!(second <= percentage_factor);
    let sum = first
        .checked_add(second)
        .expect("two normalized percentages cannot overflow u128");
    assert!(sum <= percentage_factor);
    assert!(percentage_factor.saturating_sub(sum) <= 2);
}

fn close_active_overwatch_epoch() -> u32 {
    let epoch = Network::get_current_overwatch_epoch_as_u32();
    let close_block = OverwatchEpochStartBlock::<Test>::get().saturating_add(
        EpochLength::get().saturating_mul(ActiveOverwatchEpochLengthMultiplier::<Test>::get()),
    );
    System::set_block_number(close_block);
    Network::advance_overwatch_epoch(close_block);
    assert_eq!(
        PendingOverwatchSettlement::<Test>::get().map(|settlement| settlement.epoch),
        Some(epoch)
    );
    epoch
}

#[test]
fn test_overwatch_close_snapshots_exact_economics_and_revealers() {
    new_test_ext().execute_with(|| {
        let multiplier = 3;
        let stake_weight_factor = test_percent(9, 10);
        OverwatchEpochLengthMultiplier::<Test>::set(multiplier);
        ActiveOverwatchEpochLengthMultiplier::<Test>::set(multiplier);
        OverwatchStakeWeightFactor::<Test>::set(stake_weight_factor);
        set_overwatch_epoch(7);

        manual_insert_validator(11, 101, 201);
        manual_insert_validator(12, 102, 202);
        let first_node_id = insert_overwatch_node_v2(11);
        let second_node_id = insert_overwatch_node_v2(12);
        set_overwatch_node_stake(first_node_id, 400);
        set_overwatch_node_stake(second_node_id, 125);
        submit_weight(7, 1, first_node_id, test_percent(1, 2));
        submit_weight(7, 2, second_node_id, test_percent(3, 5));

        let closed_epoch = close_active_overwatch_epoch();
        let snapshot = OverwatchEpochSettlementSnapshots::<Test>::get(closed_epoch)
            .expect("rollover must atomically store its settlement snapshot");

        assert_eq!(snapshot.stake_weight_factor, stake_weight_factor);
        assert_eq!(
            snapshot.reward_budget,
            OVERWATCH_EPOCH_EMISSIONS.saturating_mul(multiplier as u128)
        );
        assert_eq!(snapshot.nodes.len(), 2);
        assert_eq!(
            snapshot.nodes.get(&first_node_id),
            Some(&crate::OverwatchNodeSettlementSnapshot {
                validator_id: 11,
                stake: 400,
            })
        );
        assert_eq!(
            snapshot.nodes.get(&second_node_id),
            Some(&crate::OverwatchNodeSettlementSnapshot {
                validator_id: 12,
                stake: 125,
            })
        );
    });
}

#[test]
fn test_direct_overwatch_settlement_fixture_uses_active_multiplier_and_exact_counts() {
    new_test_ext().execute_with(|| {
        OverwatchEpochLengthMultiplier::<Test>::set(9);
        ActiveOverwatchEpochLengthMultiplier::<Test>::set(2);
        set_overwatch_epoch(1);

        manual_insert_validator(1, 101, 201);
        manual_insert_validator(2, 102, 202);
        let first_node_id = insert_overwatch_node_v2(1);
        let second_node_id = insert_overwatch_node_v2(2);
        set_overwatch_node_stake(first_node_id, 100);
        set_overwatch_node_stake(second_node_id, 200);
        submit_weight(1, 1, first_node_id, test_percent(1, 2));
        submit_weight(1, 2, first_node_id, test_percent(1, 2));
        submit_weight(1, 2, second_node_id, test_percent(1, 2));

        queue_overwatch_settlement(1);

        let pending = PendingOverwatchSettlement::<Test>::get().unwrap();
        assert_eq!(pending.epoch_length_multiplier, 2);
        assert_eq!(pending.reveal_records, 3);
        assert_eq!(pending.revealing_nodes, 2);
        assert_eq!(pending.revealed_subnets, 2);
        let snapshot = OverwatchEpochSettlementSnapshots::<Test>::get(1).unwrap();
        assert_eq!(
            snapshot.reward_budget,
            OVERWATCH_EPOCH_EMISSIONS.saturating_mul(2)
        );
        assert_eq!(snapshot.nodes.len(), 2);
    });
}

#[test]
fn test_overwatch_close_snapshot_accepts_empty_and_maximum_committee() {
    new_test_ext().execute_with(|| {
        OverwatchEpochLengthMultiplier::<Test>::set(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::set(1);
        set_overwatch_epoch(1);

        let closed_epoch = close_active_overwatch_epoch();
        let snapshot = OverwatchEpochSettlementSnapshots::<Test>::get(closed_epoch)
            .expect("an empty close still has a valid snapshot");
        assert!(snapshot.nodes.is_empty());

        Network::calculate_overwatch_rewards();
        assert_eq!(
            LastFinalizedOverwatchEpoch::<Test>::get(),
            Some(closed_epoch)
        );
        assert!(!OverwatchEpochSettlementSnapshots::<Test>::contains_key(
            closed_epoch
        ));
    });

    new_test_ext().execute_with(|| {
        OverwatchEpochLengthMultiplier::<Test>::set(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::set(1);
        set_overwatch_epoch(1);
        assert_eq!(NetworkMaxOverwatchNodesUpperBound::get(), 64);

        for validator_id in 1..=NetworkMaxOverwatchNodesUpperBound::get() {
            manual_insert_validator(validator_id, 1_000 + validator_id, 2_000 + validator_id);
            let node_id = insert_overwatch_node_v2(validator_id);
            set_overwatch_node_stake(node_id, validator_id as u128);
            submit_weight(1, 1, node_id, test_percent(1, 2));
        }

        let closed_epoch = close_active_overwatch_epoch();
        let snapshot = OverwatchEpochSettlementSnapshots::<Test>::get(closed_epoch)
            .expect("the maximum-size committee must fit the bounded snapshot");
        assert_eq!(snapshot.nodes.len(), 64);
        for node_id in 1..=64 {
            assert_eq!(
                snapshot.nodes.get(&node_id),
                Some(&crate::OverwatchNodeSettlementSnapshot {
                    validator_id: node_id,
                    stake: node_id as u128,
                })
            );
        }
    });
}

#[test]
fn test_post_close_stake_changes_do_not_change_closed_epoch_weights() {
    new_test_ext().execute_with(|| {
        let percentage_factor = Network::percentage_factor_as_u128();
        OverwatchEpochLengthMultiplier::<Test>::set(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::set(1);
        OverwatchStakeWeightFactor::<Test>::set(percentage_factor);
        set_overwatch_epoch(1);

        manual_insert_validator(1, 101, 201);
        manual_insert_validator(2, 102, 202);
        let first_node_id = insert_overwatch_node_v2(1);
        let second_node_id = insert_overwatch_node_v2(2);
        set_overwatch_node_stake(first_node_id, 100);
        set_overwatch_node_stake(second_node_id, 100);
        submit_weight(1, 1, first_node_id, percentage_factor);
        submit_weight(1, 1, second_node_id, 0);

        let closed_epoch = close_active_overwatch_epoch();
        Network::increase_overwatch_node_stake(first_node_id, 900);
        Network::decrease_overwatch_node_stake(second_node_id, 90);

        let snapshot = OverwatchEpochSettlementSnapshots::<Test>::get(closed_epoch).unwrap();
        assert_eq!(snapshot.nodes.get(&first_node_id).unwrap().stake, 100);
        assert_eq!(snapshot.nodes.get(&second_node_id).unwrap().stake, 100);

        Network::calculate_overwatch_rewards();
        assert_eq!(
            OverwatchSubnetWeights::<Test>::get(closed_epoch, 1),
            Some(test_percent(1, 2))
        );
        assert_eq!(
            OverwatchNodeWeights::<Test>::get(closed_epoch, first_node_id),
            Some(test_percent(1, 2))
        );
        assert_eq!(
            OverwatchNodeWeights::<Test>::get(closed_epoch, second_node_id),
            Some(test_percent(1, 2))
        );
    });
}

#[test]
fn test_node_removed_after_close_is_rewarded_under_closed_id() {
    new_test_ext().execute_with(|| {
        OverwatchEpochLengthMultiplier::<Test>::set(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::set(1);
        set_overwatch_epoch(1);

        manual_insert_validator(1, 101, 201);
        let node_id = insert_overwatch_node_v2(1);
        let starting_stake = 100;
        set_overwatch_node_stake(node_id, starting_stake);
        submit_weight(1, 1, node_id, test_percent(1, 2));

        let closed_epoch = close_active_overwatch_epoch();
        assert_ok!(Network::remove_overwatch_node(
            RuntimeOrigin::signed(account(101)),
            node_id,
        ));
        assert!(!OverwatchNodes::<Test>::contains_key(node_id));

        Network::calculate_overwatch_rewards();
        assert_eq!(
            OverwatchNodeWeights::<Test>::get(closed_epoch, node_id),
            Some(Network::percentage_factor_as_u128())
        );
        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(node_id),
            starting_stake + OVERWATCH_EPOCH_EMISSIONS
        );
    });
}

#[test]
fn test_node_removed_before_close_is_excluded_from_snapshot_and_rewards() {
    new_test_ext().execute_with(|| {
        OverwatchEpochLengthMultiplier::<Test>::set(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::set(1);
        set_overwatch_epoch(1);

        manual_insert_validator(1, 101, 201);
        manual_insert_validator(2, 102, 202);
        let active_node_id = insert_overwatch_node_v2(1);
        let removed_node_id = insert_overwatch_node_v2(2);
        set_overwatch_node_stake(active_node_id, 100);
        set_overwatch_node_stake(removed_node_id, 100);
        submit_weight(1, 1, active_node_id, test_percent(1, 2));
        submit_weight(1, 1, removed_node_id, Network::percentage_factor_as_u128());

        assert_ok!(Network::remove_overwatch_node(
            RuntimeOrigin::signed(account(102)),
            removed_node_id,
        ));
        let closed_epoch = close_active_overwatch_epoch();
        let snapshot = OverwatchEpochSettlementSnapshots::<Test>::get(closed_epoch).unwrap();
        assert!(snapshot.nodes.contains_key(&active_node_id));
        assert!(!snapshot.nodes.contains_key(&removed_node_id));

        Network::calculate_overwatch_rewards();
        assert_eq!(
            OverwatchSubnetWeights::<Test>::get(closed_epoch, 1),
            Some(test_percent(1, 2))
        );
        assert_eq!(
            OverwatchNodeWeights::<Test>::get(closed_epoch, active_node_id),
            Some(Network::percentage_factor_as_u128())
        );
        assert_eq!(
            OverwatchNodeWeights::<Test>::get(closed_epoch, removed_node_id),
            None
        );
        assert_eq!(OverwatchNodeStakeBalance::<Test>::get(removed_node_id), 100);
    });
}

#[test]
fn test_post_close_exponent_change_does_not_change_closed_epoch_weights() {
    new_test_ext().execute_with(|| {
        let linear_exponent = Network::percentage_factor_as_u128();
        OverwatchEpochLengthMultiplier::<Test>::set(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::set(1);
        OverwatchStakeWeightFactor::<Test>::set(linear_exponent);
        set_overwatch_epoch(1);

        manual_insert_validator(1, 101, 201);
        manual_insert_validator(2, 102, 202);
        let first_node_id = insert_overwatch_node_v2(1);
        let second_node_id = insert_overwatch_node_v2(2);
        set_overwatch_node_stake(first_node_id, 4);
        set_overwatch_node_stake(second_node_id, 1);
        submit_weight(1, 1, first_node_id, linear_exponent);
        submit_weight(1, 1, second_node_id, 0);

        let closed_epoch = close_active_overwatch_epoch();
        OverwatchStakeWeightFactor::<Test>::set(0);

        Network::calculate_overwatch_rewards();
        assert_eq!(
            OverwatchSubnetWeights::<Test>::get(closed_epoch, 1),
            Some(test_percent(4, 5))
        );
    });
}

#[test]
fn test_missing_snapshot_keeps_delayed_settlement_retryable_and_success_is_idempotent() {
    new_test_ext().execute_with(|| {
        OverwatchEpochLengthMultiplier::<Test>::set(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::set(1);
        set_overwatch_epoch(3);

        manual_insert_validator(1, 101, 201);
        let node_id = insert_overwatch_node_v2(1);
        let starting_stake = 100;
        set_overwatch_node_stake(node_id, starting_stake);
        submit_weight(3, 1, node_id, test_percent(1, 2));

        let pending = PendingOverwatchSettlementData {
            epoch: 3,
            epoch_length_multiplier: 1,
            reveal_records: 1,
            revealing_nodes: 1,
            revealed_subnets: 1,
        };
        PendingOverwatchSettlement::<Test>::put(pending);
        seed_overwatch_settlement_snapshot(4);
        crate::CurrentOverwatchEpoch::<Test>::put(9);

        Network::calculate_overwatch_rewards();
        assert_eq!(PendingOverwatchSettlement::<Test>::get(), Some(pending));
        assert_eq!(LastFinalizedOverwatchEpoch::<Test>::get(), None);
        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(node_id),
            starting_stake
        );
        assert!(OverwatchEpochSettlementSnapshots::<Test>::contains_key(4));

        seed_overwatch_settlement_snapshot(3);
        Network::calculate_overwatch_rewards();
        assert!(PendingOverwatchSettlement::<Test>::get().is_none());
        assert!(!OverwatchEpochSettlementSnapshots::<Test>::contains_key(3));
        assert!(OverwatchEpochSettlementSnapshots::<Test>::contains_key(4));
        assert_eq!(LastFinalizedOverwatchEpoch::<Test>::get(), Some(3));
        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(node_id),
            starting_stake + OVERWATCH_EPOCH_EMISSIONS
        );

        let settled_stake = OverwatchNodeStakeBalance::<Test>::get(node_id);
        Network::calculate_overwatch_rewards();
        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(node_id),
            settled_stake
        );
        assert_eq!(LastFinalizedOverwatchEpoch::<Test>::get(), Some(3));
    });
}

#[test]
fn test_closed_node_can_be_collectively_removed_withdrawn_replaced_and_still_settled() {
    new_test_ext().execute_with(|| {
        OverwatchEpochLengthMultiplier::<Test>::set(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::set(1);

        let registration_stake = OverwatchMinStakeBalance::<Test>::get();
        let (validator_id, coldkey) = setup_qualified_overwatch_validator(
            10_400,
            10_401,
            registration_stake.saturating_mul(2).saturating_add(500),
        );
        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            registration_stake,
        ));
        let old_node_id = TotalOverwatchNodeUids::<Test>::get();
        let epoch = Network::get_current_overwatch_epoch_as_u32();
        submit_weight(epoch, 1, old_node_id, test_percent(1, 2));

        let closed_epoch = close_active_overwatch_epoch();
        assert_ok!(Network::collective_remove_overwatch_node(
            RuntimeOrigin::from(pallet_collective::RawOrigin::Members(4, 5)),
            old_node_id,
        ));
        assert_ok!(Network::remove_overwatch_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            old_node_id,
            registration_stake,
        ));
        assert_eq!(OverwatchNodeStakeBalance::<Test>::get(old_node_id), 0);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            registration_stake,
        ));
        let replacement_node_id = TotalOverwatchNodeUids::<Test>::get();
        assert_eq!(replacement_node_id, old_node_id + 1);
        assert_eq!(
            ValidatorOverwatchNodeId::<Test>::get(validator_id),
            Some(replacement_node_id)
        );
        assert_eq!(
            OverwatchNodeValidatorId::<Test>::get(old_node_id),
            Some(validator_id)
        );
        assert_eq!(
            OverwatchNodeValidatorId::<Test>::get(replacement_node_id),
            Some(validator_id)
        );

        Network::calculate_overwatch_rewards();
        assert_eq!(
            OverwatchNodeWeights::<Test>::get(closed_epoch, old_node_id),
            Some(Network::percentage_factor_as_u128())
        );
        assert_eq!(
            OverwatchNodeWeights::<Test>::get(closed_epoch, replacement_node_id),
            None
        );
        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(old_node_id),
            OVERWATCH_EPOCH_EMISSIONS
        );
        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(replacement_node_id),
            registration_stake
        );

        // The old ID's historical node-to-validator mapping remains sufficient to withdraw the
        // close-time reward even though the validator now owns a different active node.
        assert_ok!(Network::remove_overwatch_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            old_node_id,
            OVERWATCH_EPOCH_EMISSIONS,
        ));
        assert_eq!(OverwatchNodeStakeBalance::<Test>::get(old_node_id), 0);
        assert_eq!(
            StakeUnbondingLedger::<Test>::get(coldkey)
                .values()
                .map(|entry| entry.overwatch)
                .fold(0u128, u128::saturating_add),
            registration_stake.saturating_add(OVERWATCH_EPOCH_EMISSIONS)
        );
    });
}

#[test]
fn test_delayed_slot_one_settlement_uses_close_snapshot_after_stake_mutation() {
    new_test_ext().execute_with(|| {
        let percentage_factor = Network::percentage_factor_as_u128();
        OverwatchEpochLengthMultiplier::<Test>::set(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::set(1);
        OverwatchStakeWeightFactor::<Test>::set(percentage_factor);
        set_overwatch_epoch(1);

        manual_insert_validator(1, 101, 201);
        manual_insert_validator(2, 102, 202);
        let first_node_id = insert_overwatch_node_v2(1);
        let second_node_id = insert_overwatch_node_v2(2);
        set_overwatch_node_stake(first_node_id, 100);
        set_overwatch_node_stake(second_node_id, 100);
        submit_weight(1, 1, first_node_id, percentage_factor);
        submit_weight(1, 1, second_node_id, 0);

        let rollover_block = OverwatchEpochStartBlock::<Test>::get() + EpochLength::get();
        System::set_block_number(rollover_block);
        Network::on_initialize(rollover_block);
        assert_eq!(
            PendingOverwatchSettlement::<Test>::get().map(|settlement| settlement.epoch),
            Some(1)
        );

        // Skip the immediately following settlement slot and exercise another reserved slot.
        let emission_slot = rollover_block + crate::NETWORK_SUBNET_EMISSION_SLOT;
        System::set_block_number(emission_slot);
        Network::on_initialize(emission_slot);
        assert!(PendingOverwatchSettlement::<Test>::get().is_some());

        Network::increase_overwatch_node_stake(first_node_id, 900);
        Network::decrease_overwatch_node_stake(second_node_id, 90);

        let delayed_settlement_block =
            rollover_block + EpochLength::get() + NETWORK_OVERWATCH_SETTLEMENT_SLOT;
        System::set_block_number(delayed_settlement_block);
        Network::on_initialize(delayed_settlement_block);

        assert!(PendingOverwatchSettlement::<Test>::get().is_none());
        assert!(!OverwatchEpochSettlementSnapshots::<Test>::contains_key(1));
        assert_eq!(
            OverwatchSubnetWeights::<Test>::get(1, 1),
            Some(test_percent(1, 2))
        );
        assert_eq!(
            OverwatchNodeWeights::<Test>::get(1, first_node_id),
            Some(test_percent(1, 2))
        );
        assert_eq!(
            OverwatchNodeWeights::<Test>::get(1, second_node_id),
            Some(test_percent(1, 2))
        );
    });
}

#[test]
fn test_post_close_whitelist_and_hotkey_changes_do_not_affect_settlement() {
    new_test_ext().execute_with(|| {
        OverwatchEpochLengthMultiplier::<Test>::set(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::set(1);

        let registration_stake = OverwatchMinStakeBalance::<Test>::get();
        let (validator_id, coldkey) = setup_qualified_overwatch_validator(
            10_500,
            10_501,
            registration_stake.saturating_add(500),
        );
        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            registration_stake,
        ));
        let node_id = TotalOverwatchNodeUids::<Test>::get();
        let epoch = Network::get_current_overwatch_epoch_as_u32();
        submit_weight(epoch, 1, node_id, test_percent(1, 2));

        let closed_epoch = close_active_overwatch_epoch();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, false);
        let replacement_hotkey = account(10_599);
        assert_ok!(Network::update_overwatch_hotkey(
            RuntimeOrigin::signed(coldkey),
            node_id,
            Some(replacement_hotkey.clone()),
        ));
        assert_eq!(
            OverwatchNodeIdHotkey::<Test>::get(node_id),
            Some(replacement_hotkey)
        );

        Network::calculate_overwatch_rewards();
        assert_eq!(
            OverwatchNodeWeights::<Test>::get(closed_epoch, node_id),
            Some(Network::percentage_factor_as_u128())
        );
        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(node_id),
            registration_stake.saturating_add(OVERWATCH_EPOCH_EMISSIONS)
        );
    });
}

#[test]
fn test_overwatch_min_age_uses_general_epochs_and_exact_boundary() {
    new_test_ext().execute_with(|| {
        let validator_id = 1;
        make_overwatch_qualified_v2(validator_id);

        let min_age = OverwatchMinAge::<Test>::get();
        assert_eq!(min_age, EpochsPerYear::get() / 4);

        System::set_block_number(min_age.saturating_sub(1).saturating_mul(EpochLength::get()));
        assert!(!Network::is_validator_overwatch_qualified_read_only(
            validator_id
        ));

        System::set_block_number(min_age.saturating_mul(EpochLength::get()));
        assert!(Network::is_validator_overwatch_qualified_read_only(
            validator_id
        ));
    });
}

#[test]
fn test_validator_cannot_register_two_active_overwatch_nodes_but_can_replace_removed_node() {
    new_test_ext().execute_with(|| {
        let amount = OverwatchMinStakeBalance::<Test>::get();
        let (validator_id, coldkey) = setup_qualified_overwatch_validator(
            10_100,
            10_101,
            amount.saturating_mul(2).saturating_add(500),
        );

        // Active Overwatch ownership is validator-only; the validator needs no subnet nodes.
        assert!(ValidatorSubnetNodes::<Test>::get(validator_id).is_empty());

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));
        let first_node_id = TotalOverwatchNodeUids::<Test>::get();
        assert_eq!(
            ValidatorOverwatchNodeId::<Test>::get(validator_id),
            Some(first_node_id)
        );
        assert_eq!(
            OverwatchNodeValidatorId::<Test>::get(first_node_id),
            Some(validator_id)
        );

        let free_balance = Balances::free_balance(&coldkey);
        let total_stake = TotalOverwatchNodeStakeBalance::<Test>::get();
        assert_err!(
            Network::register_overwatch_node(RuntimeOrigin::signed(coldkey.clone()), amount),
            Error::<Test>::ValidatorAlreadyHasOverwatchNode
        );
        assert_eq!(TotalOverwatchNodeUids::<Test>::get(), first_node_id);
        assert_eq!(TotalOverwatchNodes::<Test>::get(), 1);
        assert_eq!(Balances::free_balance(&coldkey), free_balance);
        assert_eq!(TotalOverwatchNodeStakeBalance::<Test>::get(), total_stake);
        assert_eq!(
            ValidatorOverwatchNodeId::<Test>::get(validator_id),
            Some(first_node_id)
        );

        assert_ok!(Network::remove_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            first_node_id,
        ));
        assert_eq!(ValidatorOverwatchNodeId::<Test>::get(validator_id), None);
        assert_eq!(
            OverwatchNodeValidatorId::<Test>::get(first_node_id),
            Some(validator_id)
        );

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey),
            amount,
        ));
        let replacement_node_id = TotalOverwatchNodeUids::<Test>::get();
        assert_eq!(replacement_node_id, first_node_id + 1);
        assert_eq!(
            ValidatorOverwatchNodeId::<Test>::get(validator_id),
            Some(replacement_node_id)
        );
        assert_eq!(
            OverwatchNodeValidatorId::<Test>::get(replacement_node_id),
            Some(validator_id)
        );
        assert_eq!(TotalOverwatchNodes::<Test>::get(), 1);
    });
}

#[test]
fn test_distinct_validators_can_register_distinct_active_overwatch_nodes() {
    new_test_ext().execute_with(|| {
        let amount = OverwatchMinStakeBalance::<Test>::get();
        let (validator_id_1, coldkey_1) =
            setup_qualified_overwatch_validator(10_200, 10_201, amount.saturating_add(500));
        let (validator_id_2, coldkey_2) =
            setup_qualified_overwatch_validator(10_202, 10_203, amount.saturating_add(500));

        assert_ne!(validator_id_1, validator_id_2);
        assert!(Network::is_validator_overwatch_qualified_read_only(
            validator_id_1
        ));
        assert!(Network::is_validator_overwatch_qualified_read_only(
            validator_id_2
        ));

        // Keep validator and Overwatch IDs deliberately different so the test cannot pass by
        // accidentally treating an Overwatch node ID as a validator identity.
        TotalOverwatchNodeUids::<Test>::set(40);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey_1),
            amount,
        ));
        let node_id_1 = TotalOverwatchNodeUids::<Test>::get();
        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey_2),
            amount,
        ));
        let node_id_2 = TotalOverwatchNodeUids::<Test>::get();

        assert_ne!(node_id_1, node_id_2);
        assert_ne!(validator_id_1, node_id_1);
        assert_ne!(validator_id_2, node_id_2);
        assert_eq!(
            ValidatorOverwatchNodeId::<Test>::get(validator_id_1),
            Some(node_id_1)
        );
        assert_eq!(
            ValidatorOverwatchNodeId::<Test>::get(validator_id_2),
            Some(node_id_2)
        );
        assert_eq!(
            OverwatchNodeValidatorId::<Test>::get(node_id_1),
            Some(validator_id_1)
        );
        assert_eq!(
            OverwatchNodeValidatorId::<Test>::get(node_id_2),
            Some(validator_id_2)
        );
        assert_eq!(TotalOverwatchNodes::<Test>::get(), 2);
    });
}

#[test]
fn test_overwatch_node_uid_exhaustion_does_not_mutate_registration_state() {
    new_test_ext().execute_with(|| {
        let amount = OverwatchMinStakeBalance::<Test>::get();
        let (validator_id, coldkey) =
            setup_qualified_overwatch_validator(10_300, 10_301, amount.saturating_add(500));
        TotalOverwatchNodeUids::<Test>::set(u32::MAX);

        let free_balance = Balances::free_balance(&coldkey);
        let total_stake = TotalOverwatchNodeStakeBalance::<Test>::get();
        assert_err!(
            Network::register_overwatch_node(RuntimeOrigin::signed(coldkey.clone()), amount),
            Error::<Test>::OverwatchNodeIdExhausted
        );

        assert_eq!(TotalOverwatchNodeUids::<Test>::get(), u32::MAX);
        assert_eq!(TotalOverwatchNodes::<Test>::get(), 0);
        assert_eq!(TotalOverwatchNodeStakeBalance::<Test>::get(), total_stake);
        assert_eq!(Balances::free_balance(&coldkey), free_balance);
        assert_eq!(ValidatorOverwatchNodeId::<Test>::get(validator_id), None);
        assert!(OverwatchNodes::<Test>::iter_keys().next().is_none());
    });
}

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

        make_overwatch_qualified_v2(validator_id);

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
fn test_register_overwatch_node_requires_whitelisted_validator() {
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
            Error::<Test>::ValidatorNotOverwatchWhitelisted
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
        make_overwatch_qualified_v2(validator_id);

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
        make_overwatch_qualified_v2(validator_id);

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

        // make_overwatch_qualified_v2(validator_id);

        set_overwatch_epoch(1);

        TotalOverwatchNodes::<Test>::set(MaxOverwatchNodes::<Test>::get());
        assert_err!(
            Network::register_overwatch_node(RuntimeOrigin::signed(coldkey.clone()), amount,),
            Error::<Test>::MaxOverwatchNodes
        );

        TotalOverwatchNodes::<Test>::set(0);

        assert_err!(
            Network::register_overwatch_node(RuntimeOrigin::signed(coldkey.clone()), amount,),
            Error::<Test>::ValidatorNotOverwatchQualified
        );

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;
        make_overwatch_qualified_v2(validator_id);

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

        make_overwatch_qualified_v2(validator_id);

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

        // Re-submitting the node's existing peer is idempotent.
        assert_ok!(Network::set_overwatch_node_peer_id(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            overwatch_node_id,
            peer_id.clone(),
        ));

        // Replacing it updates both directions and releases the old peer ID.
        let replacement_peer_id = peer(2);
        assert_ok!(Network::set_overwatch_node_peer_id(
            RuntimeOrigin::signed(coldkey),
            subnet_id,
            overwatch_node_id,
            replacement_peer_id.clone(),
        ));
        assert!(!PeerIdOverwatchNodeId::<Test>::contains_key(
            subnet_id, &peer_id
        ));
        assert_eq!(
            PeerIdOverwatchNodeId::<Test>::get(subnet_id, &replacement_peer_id),
            overwatch_node_id
        );
        assert_eq!(
            OverwatchNodeIndex::<Test>::get(overwatch_node_id).get(&subnet_id),
            Some(&replacement_peer_id)
        );
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
        make_overwatch_qualified_v2(validator_id);
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

        make_overwatch_qualified_v2(validator_id);

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

        make_overwatch_qualified_v2(validator_id);

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
fn test_add_overwatch_node_stake_rejects_removed_overwatch_node() {
    new_test_ext().execute_with(|| {
        let amount = 100000000000000000000;
        let increase_amount = 50000000000000000000;

        let coldkey = account(1);
        let hotkey = account(2);

        let reward_rate = test_percent(1, 20);
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);

        let _ = Balances::deposit_creating(&coldkey, amount + increase_amount + 500);

        increase_epochs(OverwatchEpochLengthMultiplier::<Test>::get() as u32);

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;
        make_overwatch_qualified_v2(validator_id);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        let node_stake = OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id);
        let total_stake = TotalOverwatchNodeStakeBalance::<Test>::get();

        assert_ok!(Network::remove_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            overwatch_node_id,
        ));

        assert_err!(
            Network::add_overwatch_node_stake(
                RuntimeOrigin::signed(coldkey),
                overwatch_node_id,
                increase_amount,
            ),
            Error::<Test>::InvalidOverwatchNodeId
        );

        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id),
            node_stake
        );
        assert_eq!(TotalOverwatchNodeStakeBalance::<Test>::get(), total_stake);
    });
}

#[test]
fn test_overwatch_rewards_ignore_non_authoritative_duplicate_validator_node() {
    new_test_ext().execute_with(|| {
        let validator_id = 1;
        manual_insert_validator(validator_id, validator_id, validator_id);

        let canonical_node_id = insert_overwatch_node_v2(validator_id);
        let duplicate_node_id = canonical_node_id + 1;
        TotalOverwatchNodeUids::<Test>::set(duplicate_node_id);
        TotalOverwatchNodes::<Test>::set(2);
        OverwatchNodes::<Test>::insert(
            duplicate_node_id,
            OverwatchNode {
                id: duplicate_node_id,
            },
        );
        OverwatchNodeValidatorId::<Test>::insert(duplicate_node_id, validator_id);

        let starting_stake = 100;
        set_overwatch_node_stake(canonical_node_id, starting_stake);
        set_overwatch_node_stake(duplicate_node_id, starting_stake);

        let epoch = Network::get_current_overwatch_epoch_as_u32();
        let subnet_id = 1;
        let submitted_weight = test_percent(1, 2);
        submit_weight(epoch, subnet_id, canonical_node_id, submitted_weight);
        submit_weight(epoch, subnet_id, duplicate_node_id, submitted_weight);
        queue_overwatch_settlement(epoch);

        Network::calculate_overwatch_rewards();

        assert_eq!(
            ValidatorOverwatchNodeId::<Test>::get(validator_id),
            Some(canonical_node_id)
        );
        assert_eq!(
            OverwatchNodeWeights::<Test>::get(epoch, canonical_node_id),
            Some(Network::percentage_factor_as_u128())
        );
        assert_eq!(
            OverwatchNodeWeights::<Test>::get(epoch, duplicate_node_id),
            None
        );
        assert!(OverwatchNodeStakeBalance::<Test>::get(canonical_node_id) > starting_stake);
        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(duplicate_node_id),
            starting_stake
        );
    });
}

#[test]
fn test_overwatch_stake_normalization_linear_handles_realistic_token_stakes() {
    let percentage_factor = Network::percentage_factor_as_u128();
    let (subnet_weight, first_node_weight, second_node_weight) =
        run_two_node_overwatch_stake_weight_case(
            percentage_factor,
            1_000 * percentage_factor,
            100 * percentage_factor,
        );

    let expected_first_weight = test_percent(10, 11);
    let expected_second_weight = test_percent(1, 11);
    assert_eq!(subnet_weight, expected_first_weight);
    assert_eq!(first_node_weight, expected_first_weight);
    assert_eq!(second_node_weight, expected_second_weight);
    assert_normalized_pair(first_node_weight, second_node_weight);
}

#[test]
fn test_overwatch_stake_normalization_point_nine_handles_sixty_four_large_equal_stakes() {
    new_test_ext().execute_with(|| {
        let percentage_factor = Network::percentage_factor_as_u128();
        OverwatchStakeWeightFactor::<Test>::set(test_percent(9, 10));
        assert_eq!(MaxOverwatchNodes::<Test>::get(), 64);

        let epoch = Network::get_current_overwatch_epoch_as_u32();
        let subnet_id = 1;
        for validator_id in 1..=64 {
            manual_insert_validator(validator_id, 1_000 + validator_id, 2_000 + validator_id);
            let node_id = insert_overwatch_node_v2(validator_id);
            set_overwatch_node_stake(node_id, 1_000 * percentage_factor);
            submit_weight(epoch, subnet_id, node_id, test_percent(1, 2));
        }

        queue_overwatch_settlement(epoch);
        Network::calculate_overwatch_rewards();

        assert_eq!(
            OverwatchSubnetWeights::<Test>::get(epoch, subnet_id),
            Some(test_percent(1, 2))
        );

        let expected_node_weight = percentage_factor / 64;
        let mut normalized_sum = 0u128;
        for node_id in 1..=64 {
            let node_weight = OverwatchNodeWeights::<Test>::get(epoch, node_id)
                .expect("every equally weighted revealer must receive a score");
            assert_eq!(node_weight, expected_node_weight);
            normalized_sum = normalized_sum
                .checked_add(node_weight)
                .expect("normalized Overwatch weights cannot overflow u128");
        }
        assert_eq!(normalized_sum, percentage_factor);
    });
}

#[test]
fn test_overwatch_stake_normalization_is_scale_invariant_and_handles_edges() {
    const ROUNDING_TOLERANCE: u128 = 512;
    let percentage_factor = Network::percentage_factor_as_u128();
    let large_scale = u128::MAX / 3;

    for exponent in [test_percent(9, 10), percentage_factor] {
        let small = run_two_node_overwatch_stake_weight_case(exponent, 2, 1);
        let large =
            run_two_node_overwatch_stake_weight_case(exponent, large_scale * 2, large_scale);

        assert!(small.0.abs_diff(large.0) <= ROUNDING_TOLERANCE);
        assert!(small.1.abs_diff(large.1) <= ROUNDING_TOLERANCE);
        assert!(small.2.abs_diff(large.2) <= ROUNDING_TOLERANCE);
        assert_normalized_pair(small.1, small.2);
        assert_normalized_pair(large.1, large.2);

        let max_and_zero = run_two_node_overwatch_stake_weight_case(exponent, u128::MAX, 0);
        assert_eq!(max_and_zero, (percentage_factor, percentage_factor, 0));

        let all_zero = run_two_node_overwatch_stake_weight_case(exponent, 0, 0);
        assert_eq!(all_zero, (0, 0, 0));
    }
}

#[test]
fn test_overwatch_stake_normalization_linear_runs_through_on_initialize() {
    new_test_ext().execute_with(|| {
        let percentage_factor = Network::percentage_factor_as_u128();
        OverwatchStakeWeightFactor::<Test>::set(percentage_factor);
        OverwatchEpochLengthMultiplier::<Test>::set(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::set(1);
        set_overwatch_epoch(1);

        let epoch = Network::get_current_overwatch_epoch_as_u32();
        let subnet_id = 1;
        manual_insert_validator(1, 101, 201);
        manual_insert_validator(2, 102, 202);
        let first_node_id = insert_overwatch_node_v2(1);
        let second_node_id = insert_overwatch_node_v2(2);
        set_overwatch_node_stake(first_node_id, 1_000 * percentage_factor);
        set_overwatch_node_stake(second_node_id, 1_000 * percentage_factor);
        submit_weight(epoch, subnet_id, first_node_id, test_percent(1, 2));
        submit_weight(epoch, subnet_id, second_node_id, test_percent(1, 2));

        let rollover_block = System::block_number() + EpochLength::get();
        System::set_block_number(rollover_block);
        Network::on_initialize(rollover_block);
        assert_eq!(
            PendingOverwatchSettlement::<Test>::get().map(|settlement| settlement.epoch),
            Some(epoch)
        );
        let snapshot = OverwatchEpochSettlementSnapshots::<Test>::get(epoch)
            .expect("the hook rollover must store a close-time snapshot");
        assert_eq!(snapshot.nodes.len(), 2);
        assert_eq!(
            snapshot.nodes.get(&first_node_id).unwrap().stake,
            1_000 * percentage_factor
        );
        assert_eq!(
            snapshot.nodes.get(&second_node_id).unwrap().stake,
            1_000 * percentage_factor
        );

        let settlement_block = rollover_block + NETWORK_OVERWATCH_SETTLEMENT_SLOT;
        System::set_block_number(settlement_block);
        Network::on_initialize(settlement_block);

        assert!(PendingOverwatchSettlement::<Test>::get().is_none());
        assert!(!OverwatchEpochSettlementSnapshots::<Test>::contains_key(
            epoch
        ));
        assert_eq!(
            OverwatchSubnetWeights::<Test>::get(epoch, subnet_id),
            Some(test_percent(1, 2))
        );
        let first_node_weight = OverwatchNodeWeights::<Test>::get(epoch, first_node_id)
            .expect("the first node must be finalized by the hook");
        let second_node_weight = OverwatchNodeWeights::<Test>::get(epoch, second_node_id)
            .expect("the second node must be finalized by the hook");
        assert_eq!(first_node_weight, test_percent(1, 2));
        assert_eq!(second_node_weight, test_percent(1, 2));
        assert_normalized_pair(first_node_weight, second_node_weight);
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

        queue_overwatch_settlement(epoch);
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

        queue_overwatch_settlement(epoch);
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

        queue_overwatch_settlement(epoch);
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

        queue_overwatch_settlement(epoch);
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

        queue_overwatch_settlement(epoch);
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

        queue_overwatch_settlement(epoch);
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

        queue_overwatch_settlement(epoch);
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

        make_overwatch_qualified_v2(validator_id);

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

        make_overwatch_qualified_v2(validator_id);

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

        make_overwatch_qualified_v2(validator_id);

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

        make_overwatch_qualified_v2(validator_id);

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

        let unbondings = StakeUnbondingLedger::<Test>::get(coldkey.clone());
        assert_eq!(unbondings.len(), 1);
        let (ledger_block, ledger_balance) = unbondings.iter().next().unwrap();
        assert_eq!(
            *ledger_block,
            &block + StakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );
        assert_eq!(ledger_balance.network, 0);
        assert_eq!(ledger_balance.overwatch, remove_amount);

        System::set_block_number(block + StakeCooldownEpochs::<Test>::get() * EpochLength::get());

        let starting_balance = Balances::free_balance(&coldkey.clone());

        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            coldkey.clone()
        )));

        assert_eq!(
            Balances::free_balance(&coldkey.clone()),
            starting_balance + remove_amount
        );

        let unbondings = StakeUnbondingLedger::<Test>::get(coldkey.clone());
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

        make_overwatch_qualified_v2(validator_id);

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

        make_overwatch_qualified_v2(validator_id);

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

        queue_overwatch_settlement(epoch);
        let block_weight = Network::calculate_overwatch_rewards();

        let subnet_weight = OverwatchSubnetWeights::<Test>::get(epoch, subnet_id);

        const ROUNDING_TOLERANCE: u128 = 256;
        let expected_low_weight = 121_585_365_354_349_700;
        let expected_high_weight = 878_414_634_645_650_300;

        // Floating-point exponentiation can move the final fixed-point result by a few units.
        assert!(
            subnet_weight
                .expect("the revealed subnet must be finalized")
                .abs_diff(expected_low_weight)
                <= ROUNDING_TOLERANCE
        );

        let score_1 = OverwatchNodeWeights::<Test>::get(epoch, node_id_1);
        let score_2 = OverwatchNodeWeights::<Test>::get(epoch, node_id_2);

        assert!(
            score_1
                .expect("the first node must receive a score")
                .abs_diff(expected_high_weight)
                <= ROUNDING_TOLERANCE
        );
        assert!(
            score_2
                .expect("the second node must receive a score")
                .abs_diff(expected_low_weight)
                <= ROUNDING_TOLERANCE
        );

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
