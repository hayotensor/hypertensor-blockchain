use super::mock::*;
use crate::tests::test_utils::*;
use crate::{
    FinalSubnetEmissionWeights, MaxOverwatchNodes, MaxSubnets, MinSubnetNodes,
    NewRegistrationCostMultiplier, NodeSubnetStake, OverwatchCommit, OverwatchCommits,
    OverwatchEpochLengthMultiplier, OverwatchNodeStakeBalance, OverwatchReveal, OverwatchReveals,
    OverwatchSubnetWeights, OverwatchValidatorWhitelist, SlotAssignment, SubnetConsensusSubmission,
    SubnetElectedValidator, SubnetName, SubnetReputation, TotalSubnetDelegateStakeBalance,
};
use frame_support::assert_ok;
use frame_support::traits::{Currency, OnInitialize};
use sp_std::collections::btree_map::BTreeMap;

//
//
//
//
//
//
//
// On Initialize Hook
//
//
//
//
//
//
//

/// Verifies:
/// - Emmissions to nodes
/// - Subnets stay active

// Helper to change the overwatch weights
fn is_even(num: u32) -> bool {
    if num % 2 == 0 {
        return true;
    }
    return false;
}

// Simulated commit that bounces between 1e18 and 0.5e18
fn get_commit(num: u32) -> (u128, Vec<u8>, sp_core::H256) {
    // default onode weights
    let weights: Vec<u128> = vec![1000000000000000000, 500000000000000000];

    let mut weight: u128 = 1000000000000000000;
    if is_even(num) {
        weight = weights[0];
    } else {
        weight = weights[1];
    }
    let salt: Vec<u8> = b"secret-salt".to_vec();
    let commit_hash = make_commit(weight, salt.clone());

    (weight, salt, commit_hash)
}

#[test]
fn test_on_initialize() {
    new_test_ext().execute_with(|| {
        NewRegistrationCostMultiplier::<Test>::put(1200000000000000000);
        OverwatchEpochLengthMultiplier::<Test>::set(2);

        let max_overwatch_nodes = MaxOverwatchNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let min_subnet_nodes = MinSubnetNodes::<Test>::get();
        let deposit_amount: u128 = get_min_stake_balance() + 500;
        let stake_amount: u128 = get_min_stake_balance();

        for s in 0..max_subnets {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            build_activated_subnet(
                subnet_name,
                0,
                min_subnet_nodes,
                deposit_amount,
                stake_amount,
            );
        }

        let subnet_ids: Vec<u32> = (0..max_subnets)
            .map(|s| {
                let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
                SubnetName::<Test>::get(subnet_name).unwrap()
            })
            .collect();

        let overwatch_count = max_overwatch_nodes.min(min_subnet_nodes).max(1);
        let mut overwatch_node_ids = Vec::new();
        for validator_id in 1..=overwatch_count {
            OverwatchValidatorWhitelist::<Test>::insert(validator_id, true);
            let overwatch_node_id = insert_overwatch_node_v2(validator_id);
            set_overwatch_node_stake(overwatch_node_id, 100);
            assert_ne!(OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id), 0);
            overwatch_node_ids.push(overwatch_node_id);
        }

        let epoch_length = EpochLength::get();
        let multiplier = OverwatchEpochLengthMultiplier::<Test>::get();
        let overwatch_epoch_length = epoch_length.saturating_mul(multiplier);
        let overwatch_epochs_to_simulate = 2;
        let first_overwatch_epoch = Network::get_current_overwatch_epoch_as_u32().saturating_add(1);
        let start_block = first_overwatch_epoch.saturating_mul(overwatch_epoch_length);
        let last_simulated_overwatch_epoch =
            first_overwatch_epoch.saturating_add(overwatch_epochs_to_simulate);

        let mut epoch_preliminaries_ran = 0;
        let mut overwatch_rewards_ran = 0;
        let mut emission_weights_ran = 0;
        let mut emission_step_ran = 0;
        let mut subnet_nodes_rewarded = false;
        let mut overwatch_nodes_rewarded = false;
        let mut commits_checked = false;
        let mut reveals_checked = false;
        let mut overwatch_weights_checked = false;

        let mut last_committed_overwatch_epoch = u32::MAX;
        let mut last_revealed_overwatch_epoch = u32::MAX;
        let mut commits = 0;
        let mut reveals = 0;

        for offset in 0..=overwatch_epochs_to_simulate
            .saturating_mul(overwatch_epoch_length)
            .saturating_add(1)
        {
            let block = start_block.saturating_add(offset);
            System::set_block_number(block);

            let current_epoch = block.saturating_div(epoch_length);
            let current_overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();
            let epoch_slot = block % epoch_length;

            let runs_epoch_preliminaries = block >= epoch_length && block % epoch_length == 0;
            let runs_overwatch_rewards = !runs_epoch_preliminaries
                && block.saturating_sub(1) >= overwatch_epoch_length
                && block.saturating_sub(1) % overwatch_epoch_length == 0;
            let runs_emission_weights = !runs_epoch_preliminaries
                && !runs_overwatch_rewards
                && block.saturating_sub(2) >= epoch_length
                && block.saturating_sub(2) % epoch_length == 0;
            let slot_subnet_id =
                if runs_epoch_preliminaries || runs_overwatch_rewards || runs_emission_weights {
                    None
                } else {
                    SlotAssignment::<Test>::get(epoch_slot)
                };

            if runs_epoch_preliminaries {
                for subnet_id in subnet_ids.iter().copied() {
                    let total_delegate_stake_balance =
                        TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
                    let min_subnet_delegate_stake =
                        Network::get_min_subnet_delegate_stake_balance(subnet_id);
                    if total_delegate_stake_balance < min_subnet_delegate_stake {
                        let delta = min_subnet_delegate_stake - total_delegate_stake_balance;
                        let delegate = account(10000 + subnet_id);
                        let _ = Balances::deposit_creating(&delegate, delta + 500);
                        assert_ok!(Network::add_subnet_delegate_stake(
                            RuntimeOrigin::signed(delegate),
                            subnet_id,
                            delta,
                        ));
                    }

                    let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
                    if SubnetElectedValidator::<Test>::get(subnet_id, subnet_epoch).is_some() {
                        run_subnet_consensus_step_v2(subnet_id, None, None);
                    }
                }
            }

            let overwatch_stake_snapshot: BTreeMap<u32, u128> = if runs_overwatch_rewards {
                let previous_overwatch_epoch = current_overwatch_epoch.saturating_sub(1);
                if OverwatchReveals::<Test>::iter_prefix((previous_overwatch_epoch,))
                    .next()
                    .is_some()
                {
                    overwatch_node_ids
                        .iter()
                        .map(|node_id| (*node_id, OverwatchNodeStakeBalance::<Test>::get(*node_id)))
                        .collect()
                } else {
                    BTreeMap::new()
                }
            } else {
                BTreeMap::new()
            };

            let node_stake_snapshot = slot_subnet_id.and_then(|subnet_id| {
                let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
                if subnet_epoch == 0 {
                    return None;
                }
                if !FinalSubnetEmissionWeights::<Test>::get(current_epoch)
                    .subnet_weights
                    .contains_key(&subnet_id)
                {
                    return None;
                }
                if SubnetConsensusSubmission::<Test>::get(subnet_id, subnet_epoch.saturating_sub(1))
                    .is_none()
                {
                    return None;
                }

                let total_stake = (1..=min_subnet_nodes)
                    .map(|subnet_node_id| NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id))
                    .fold(0u128, |acc, stake| acc.saturating_add(stake));
                Some((subnet_id, total_stake))
            });

            Network::on_initialize(block);

            if runs_epoch_preliminaries {
                epoch_preliminaries_ran += 1;
            } else if runs_overwatch_rewards {
                if !overwatch_stake_snapshot.is_empty() {
                    overwatch_rewards_ran += 1;
                    let previous_overwatch_epoch = current_overwatch_epoch.saturating_sub(1);
                    for subnet_id in subnet_ids.iter().copied() {
                        assert!(OverwatchSubnetWeights::<Test>::get(
                            previous_overwatch_epoch,
                            subnet_id
                        )
                        .is_some());
                    }
                    overwatch_weights_checked = true;

                    for (node_id, old_stake) in overwatch_stake_snapshot {
                        let new_stake = OverwatchNodeStakeBalance::<Test>::get(node_id);
                        assert!(new_stake > old_stake);
                        overwatch_nodes_rewarded = true;
                    }
                }
            } else if runs_emission_weights {
                let subnet_emission_weights =
                    FinalSubnetEmissionWeights::<Test>::get(current_epoch);
                assert!(!subnet_emission_weights.subnet_weights.is_empty());
                for subnet_id in subnet_ids.iter().copied() {
                    let subnet_weight = subnet_emission_weights
                        .subnet_weights
                        .get(&subnet_id)
                        .copied();
                    assert!(subnet_weight.is_some());
                    assert!(subnet_weight.unwrap() > 0);
                    assert!(subnet_weight.unwrap() <= Network::percentage_factor_as_u128());
                }
                emission_weights_ran += 1;
            } else if let Some((subnet_id, old_total_stake)) = node_stake_snapshot {
                let new_total_stake = (1..=min_subnet_nodes)
                    .map(|subnet_node_id| NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id))
                    .fold(0u128, |acc, stake| acc.saturating_add(stake));
                assert!(new_total_stake > old_total_stake);
                subnet_nodes_rewarded = true;
                emission_step_ran += 1;
            } else if slot_subnet_id.is_some() {
                emission_step_ran += 1;
            }

            if current_overwatch_epoch < last_simulated_overwatch_epoch {
                if Network::in_overwatch_commit_period()
                    && last_committed_overwatch_epoch != current_overwatch_epoch
                {
                    let commit_payload: Vec<_> = subnet_ids
                        .iter()
                        .enumerate()
                        .map(|(idx, subnet_id)| {
                            let (_, _, commit_hash) = get_commit(idx as u32);
                            OverwatchCommit {
                                subnet_id: *subnet_id,
                                weight: commit_hash,
                            }
                        })
                        .collect();

                    for overwatch_node_id in overwatch_node_ids.iter().copied() {
                        let hotkey =
                            Network::get_overwatch_node_associated_hotkey(overwatch_node_id)
                                .unwrap();
                        assert_ok!(Network::commit_overwatch_subnet_weights(
                            RuntimeOrigin::signed(hotkey),
                            overwatch_node_id,
                            commit_payload.clone(),
                        ));

                        for (idx, subnet_id) in subnet_ids.iter().enumerate() {
                            let (_, _, commit_hash) = get_commit(idx as u32);
                            assert_eq!(
                                OverwatchCommits::<Test>::get((
                                    current_overwatch_epoch,
                                    overwatch_node_id,
                                    *subnet_id,
                                ))
                                .unwrap(),
                                commit_hash
                            );
                            commits_checked = true;
                        }
                    }

                    commits += 1;
                    last_committed_overwatch_epoch = current_overwatch_epoch;
                } else if !Network::in_overwatch_commit_period()
                    && last_revealed_overwatch_epoch != current_overwatch_epoch
                {
                    let reveal_payload: Vec<_> = subnet_ids
                        .iter()
                        .enumerate()
                        .map(|(idx, subnet_id)| {
                            let (weight, salt, _) = get_commit(idx as u32);
                            OverwatchReveal {
                                subnet_id: *subnet_id,
                                weight,
                                salt,
                            }
                        })
                        .collect();

                    for overwatch_node_id in overwatch_node_ids.iter().copied() {
                        let hotkey =
                            Network::get_overwatch_node_associated_hotkey(overwatch_node_id)
                                .unwrap();
                        assert_ok!(Network::reveal_overwatch_subnet_weights(
                            RuntimeOrigin::signed(hotkey),
                            overwatch_node_id,
                            reveal_payload.clone(),
                        ));

                        for (idx, subnet_id) in subnet_ids.iter().enumerate() {
                            let (weight, _, _) = get_commit(idx as u32);
                            assert_eq!(
                                OverwatchReveals::<Test>::get((
                                    current_overwatch_epoch,
                                    *subnet_id,
                                    overwatch_node_id,
                                ))
                                .unwrap(),
                                weight
                            );
                            reveals_checked = true;
                        }
                    }

                    reveals += 1;
                    last_revealed_overwatch_epoch = current_overwatch_epoch;
                }
            }

            for subnet_id in subnet_ids.iter().copied() {
                assert!(SubnetReputation::<Test>::get(subnet_id) >= 990000000000000000);
            }
        }

        assert!(epoch_preliminaries_ran >= overwatch_epochs_to_simulate * multiplier);
        assert_eq!(commits, overwatch_epochs_to_simulate);
        assert_eq!(reveals, overwatch_epochs_to_simulate);
        assert_eq!(overwatch_rewards_ran, overwatch_epochs_to_simulate);
        assert!(emission_weights_ran >= overwatch_epochs_to_simulate * multiplier);
        assert!(emission_step_ran > 0);
        assert!(subnet_nodes_rewarded);
        assert!(overwatch_nodes_rewarded);
        assert!(commits_checked);
        assert!(reveals_checked);
        assert!(overwatch_weights_checked);

        for subnet_id in subnet_ids {
            assert!(SubnetName::<Test>::iter().any(|(_, id)| id == subnet_id));
        }
    });
}

#[test]
fn test_on_initialize_runs_emission_weight_step() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "hook-emission-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = get_min_stake_balance();

        build_activated_subnet(
            subnet_name.clone(),
            0,
            MinSubnetNodes::<Test>::get(),
            deposit_amount,
            stake_amount,
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();

        let block = Network::get_current_epoch_as_u32()
            .saturating_add(1)
            .saturating_mul(EpochLength::get())
            .saturating_add(2);
        let current_epoch = block.saturating_div(EpochLength::get());
        System::set_block_number(block);

        assert!(FinalSubnetEmissionWeights::<Test>::get(current_epoch)
            .subnet_weights
            .is_empty());

        Network::on_initialize(block);

        let weights = FinalSubnetEmissionWeights::<Test>::get(current_epoch);
        assert!(weights.subnet_weights.contains_key(&subnet_id));
    });
}

#[test]
fn test_on_initialize_paused_skips_scheduled_work_and_early_blocks_are_safe() {
    new_test_ext().execute_with(|| {
        for block in 0..=2 {
            System::set_block_number(block);
            Network::on_initialize(block);
        }

        let subnet_name: Vec<u8> = "hook-paused-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = get_min_stake_balance();

        build_activated_subnet(
            subnet_name,
            0,
            MinSubnetNodes::<Test>::get(),
            deposit_amount,
            stake_amount,
        );

        let block = Network::get_current_epoch_as_u32()
            .saturating_add(1)
            .saturating_mul(EpochLength::get())
            .saturating_add(2);
        let current_epoch = block.saturating_div(EpochLength::get());
        System::set_block_number(block);

        assert_ok!(Network::pause(RuntimeOrigin::from(
            pallet_collective::RawOrigin::Members(2, 3)
        )));

        Network::on_initialize(block);

        assert!(FinalSubnetEmissionWeights::<Test>::get(current_epoch)
            .subnet_weights
            .is_empty());
    });
}
