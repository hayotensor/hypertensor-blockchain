use super::mock::*;
use crate::tests::test_utils::*;
use crate::{
    ActiveOverwatchEpochLengthMultiplier, CurrentOverwatchEpoch, FinalSubnetEmissionWeights,
    LastFinalizedOverwatchEpoch, MaxOverwatchNodes, MaxSubnets, MinDelegateStakeDeposit,
    MinSubnetNodes, MinSubnetReputation, NewRegistrationCostMultiplier, NodeSubnetStake,
    OverwatchCommit, OverwatchCommits, OverwatchEpochLengthMultiplier, OverwatchEpochStartBlock,
    OverwatchNodeStakeBalance, OverwatchReveal, OverwatchReveals, OverwatchSubnetWeights,
    OverwatchValidatorWhitelist, PrevSubnetActivationEpoch, SlotAssignment,
    SubnetConsensusSubmission, SubnetElectedValidator, SubnetName, SubnetOwner, SubnetPauseData,
    SubnetRemovalCheckInterval, SubnetReputation, SubnetState, SubnetsData, TotalActiveSubnets,
    TotalSubnetDelegateStakeBalance, TotalSubnetElectableNodes, TotalSubnets,
    NETWORK_EPOCH_PRELIMINARIES_SLOT, NETWORK_OVERWATCH_SETTLEMENT_SLOT,
    NETWORK_SUBNET_EMISSION_SLOT,
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
    let weights: Vec<u128> = vec![Network::percentage_factor_as_u128(), test_percent(1, 2)];

    let mut weight: u128 = Network::percentage_factor_as_u128();
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
fn test_on_initialize_health_removal_at_max_plus_one_does_not_evict_healthy_subnet() {
    new_test_ext().execute_with(|| {
        MaxSubnets::<Test>::put(2);
        PrevSubnetActivationEpoch::<Test>::put(0);

        let percentage_factor = Network::percentage_factor_as_u128();
        for (subnet_id, delegate_stake) in [(1, 1_000), (2, 500), (3, 700)] {
            insert_subnet(subnet_id, SubnetState::Active, 0);
            TotalSubnetDelegateStakeBalance::<Test>::insert(
                subnet_id,
                delegate_stake * percentage_factor,
            );
            TotalSubnetElectableNodes::<Test>::insert(subnet_id, MinSubnetNodes::<Test>::get());
            SubnetReputation::<Test>::insert(subnet_id, percentage_factor);
        }
        SubnetReputation::<Test>::insert(1, MinSubnetReputation::<Test>::get().saturating_sub(1));
        TotalSubnets::<Test>::put(3);
        TotalActiveSubnets::<Test>::put(3);

        let epoch = SubnetRemovalCheckInterval::<Test>::get();
        set_epoch(epoch, NETWORK_EPOCH_PRELIMINARIES_SLOT);
        let block = System::block_number();
        Network::on_initialize(block);

        assert!(!SubnetsData::<Test>::contains_key(1));
        assert!(SubnetsData::<Test>::contains_key(2));
        assert!(SubnetsData::<Test>::contains_key(3));
        assert_eq!(TotalSubnets::<Test>::get(), 2);
    });
}

#[test]
fn test_on_initialize() {
    new_test_ext().execute_with(|| {
        NewRegistrationCostMultiplier::<Test>::put(1200000000000000000);
        OverwatchEpochLengthMultiplier::<Test>::set(2);
        ActiveOverwatchEpochLengthMultiplier::<Test>::set(2);

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
            OverwatchValidatorWhitelist::<Test>::insert(validator_id, ());
            let overwatch_node_id = insert_overwatch_node_v2(validator_id);
            set_overwatch_node_stake(overwatch_node_id, 100);
            assert_ne!(OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id), 0);
            overwatch_node_ids.push(overwatch_node_id);
        }

        let epoch_length = EpochLength::get();
        let multiplier = OverwatchEpochLengthMultiplier::<Test>::get();
        let overwatch_epoch_length = epoch_length.saturating_mul(multiplier);
        let overwatch_epochs_to_simulate = 2;
        // The fixture has advanced the block directly while building subnets. Start the simulated
        // anchored round at the next complete interval without rewinding chain time.
        let first_overwatch_epoch = System::block_number()
            .saturating_div(overwatch_epoch_length)
            .saturating_add(1);
        let start_block = first_overwatch_epoch.saturating_mul(overwatch_epoch_length);
        let last_simulated_overwatch_epoch =
            first_overwatch_epoch.saturating_add(overwatch_epochs_to_simulate);
        CurrentOverwatchEpoch::<Test>::put(first_overwatch_epoch);
        OverwatchEpochStartBlock::<Test>::put(start_block);

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
            .saturating_add(NETWORK_OVERWATCH_SETTLEMENT_SLOT)
        {
            let block = start_block.saturating_add(offset);
            System::set_block_number(block);

            let current_epoch = block.saturating_div(epoch_length);
            let current_overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();
            let epoch_slot = block % epoch_length;

            let runs_epoch_preliminaries =
                block >= epoch_length && block % epoch_length == NETWORK_EPOCH_PRELIMINARIES_SLOT;
            let runs_overwatch_rewards = !runs_epoch_preliminaries
                && block.saturating_sub(NETWORK_OVERWATCH_SETTLEMENT_SLOT)
                    >= overwatch_epoch_length
                && block.saturating_sub(NETWORK_OVERWATCH_SETTLEMENT_SLOT) % overwatch_epoch_length
                    == NETWORK_EPOCH_PRELIMINARIES_SLOT;
            let runs_emission_weights = !runs_epoch_preliminaries
                && !runs_overwatch_rewards
                && block.saturating_sub(NETWORK_SUBNET_EMISSION_SLOT) >= epoch_length
                && block.saturating_sub(NETWORK_SUBNET_EMISSION_SLOT) % epoch_length
                    == NETWORK_EPOCH_PRELIMINARIES_SLOT;
            let slot_subnet_id =
                if runs_epoch_preliminaries || runs_overwatch_rewards || runs_emission_weights {
                    None
                } else {
                    SlotAssignment::<Test>::get(epoch_slot)
                };

            if runs_epoch_preliminaries {
                // A top-up changes the live-subnet average. Recompute until every subnet satisfies
                // the inclusive boundary before the hook snapshots the common requirement.
                for pass in 0u32..16 {
                    let mut all_funded = true;
                    for subnet_id in subnet_ids.iter().copied() {
                        let total_delegate_stake_balance =
                            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
                        let min_subnet_delegate_stake =
                            Network::get_min_subnet_delegate_stake_balance(subnet_id);
                        if total_delegate_stake_balance < min_subnet_delegate_stake {
                            all_funded = false;
                            let amount = min_subnet_delegate_stake
                                .saturating_sub(total_delegate_stake_balance)
                                .max(MinDelegateStakeDeposit::<Test>::get());
                            let delegate =
                                account(10_000 + pass.saturating_mul(max_subnets) + subnet_id);
                            let _ =
                                Balances::deposit_creating(&delegate, amount.saturating_add(500));
                            assert_ok!(Network::add_subnet_delegate_stake(
                                RuntimeOrigin::signed(delegate),
                                subnet_id,
                                amount,
                            ));
                        }
                    }
                    if all_funded {
                        break;
                    }
                }

                for subnet_id in subnet_ids.iter().copied() {
                    assert!(
                        TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id)
                            >= Network::get_min_subnet_delegate_stake_balance(subnet_id)
                    );
                    let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
                    if get_elected_subnet_node_id(subnet_id, subnet_epoch).is_some() {
                        run_subnet_consensus_step_v2(subnet_id, None, None);
                    }
                }
            }

            let overwatch_stake_snapshot: BTreeMap<u32, u128> = if runs_overwatch_rewards {
                let previous_overwatch_epoch = current_overwatch_epoch.saturating_sub(1);
                if OverwatchReveals::<Test>::iter_prefix(previous_overwatch_epoch)
                    .any(|(_, reveals)| !reveals.is_empty())
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
                let subnet_emission_weights =
                    FinalSubnetEmissionWeights::<Test>::get(current_epoch);
                let Some(subnet_weight) = subnet_emission_weights
                    .subnet_weights
                    .get(&subnet_id)
                    .copied()
                else {
                    return None;
                };
                let (rewards_data, _) = Network::calculate_rewards(
                    subnet_id,
                    subnet_emission_weights.subnets_emissions,
                    subnet_weight,
                );
                if rewards_data.subnet_node_rewards == 0 {
                    return None;
                }
                let prev_subnet_epoch = subnet_epoch.saturating_sub(1);
                if SubnetConsensusSubmission::<Test>::get(subnet_id, prev_subnet_epoch).is_none() {
                    return None;
                }
                let (consensus_submission_data, _) = Network::precheck_subnet_consensus_submission(
                    subnet_id,
                    prev_subnet_epoch,
                    current_epoch,
                );
                let Some(consensus_submission_data) = consensus_submission_data else {
                    return None;
                };
                if consensus_submission_data.attestation_ratio
                    < <Test as crate::Config>::MinAttestationPercentage::get()
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
                for subnet_id in subnet_ids.iter().copied() {
                    let subnet_weight = subnet_emission_weights
                        .subnet_weights
                        .get(&subnet_id)
                        .copied();
                    let has_exact_prior_election = current_epoch
                        .checked_sub(1)
                        .map(|previous_epoch| {
                            SubnetElectedValidator::<Test>::contains_key(subnet_id, previous_epoch)
                        })
                        .unwrap_or(false);

                    assert_eq!(subnet_weight.is_some(), has_exact_prior_election);
                    if let Some(subnet_weight) = subnet_weight {
                        assert!(subnet_weight > 0);
                        assert!(subnet_weight <= Network::percentage_factor_as_u128());
                    }
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
                                OverwatchCommits::<Test>::get(
                                    current_overwatch_epoch,
                                    overwatch_node_id,
                                )
                                .get(subnet_id)
                                .copied(),
                                Some(commit_hash)
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
                                salt: salt.try_into().unwrap(),
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
                                OverwatchReveals::<Test>::get(
                                    current_overwatch_epoch,
                                    overwatch_node_id,
                                )
                                .get(subnet_id)
                                .copied(),
                                Some(weight)
                            );
                            reveals_checked = true;
                        }
                    }

                    reveals += 1;
                    last_revealed_overwatch_epoch = current_overwatch_epoch;
                }
            }

            for subnet_id in subnet_ids.iter().copied() {
                assert!(
                    SubnetReputation::<Test>::get(subnet_id) >= MinSubnetReputation::<Test>::get()
                );
            }
        }

        assert!(epoch_preliminaries_ran >= overwatch_epochs_to_simulate * multiplier);
        assert_eq!(commits, overwatch_epochs_to_simulate);
        assert_eq!(reveals, overwatch_epochs_to_simulate);
        assert_eq!(overwatch_rewards_ran, overwatch_epochs_to_simulate);
        assert!(emission_weights_ran >= overwatch_epochs_to_simulate * multiplier);
        assert!(emission_step_ran > 0);
        assert!(
            subnet_nodes_rewarded,
            "no subnet node reward after {emission_step_ran} emission slot steps"
        );
        assert!(overwatch_nodes_rewarded);
        assert!(commits_checked);
        assert!(reveals_checked);
        assert!(overwatch_weights_checked);
        assert_eq!(
            LastFinalizedOverwatchEpoch::<Test>::get(),
            Some(last_simulated_overwatch_epoch.saturating_sub(1))
        );

        for subnet_id in subnet_ids {
            assert!(SubnetName::<Test>::iter().any(|(_, id)| id == subnet_id));
        }
    });
}

#[test]
fn test_on_initialize_bootstraps_election_before_emission_weight() {
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

        let first_consensus_epoch = Network::get_current_epoch_as_u32().saturating_add(1);
        set_block_to_subnet_slot_epoch(first_consensus_epoch, subnet_id);
        let first_slot_block = System::block_number();
        let first_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        assert!(
            FinalSubnetEmissionWeights::<Test>::get(first_consensus_epoch)
                .subnet_weights
                .is_empty()
        );
        assert!(get_elected_subnet_node_id(subnet_id, first_subnet_epoch).is_none());

        Network::on_initialize(first_slot_block);

        assert!(get_elected_subnet_node_id(subnet_id, first_subnet_epoch).is_some());

        let reward_epoch = first_consensus_epoch.saturating_add(1);
        let emission_weight_block = reward_epoch
            .saturating_mul(EpochLength::get())
            .saturating_add(NETWORK_SUBNET_EMISSION_SLOT);
        System::set_block_number(emission_weight_block);

        assert!(FinalSubnetEmissionWeights::<Test>::get(reward_epoch)
            .subnet_weights
            .is_empty());

        Network::on_initialize(emission_weight_block);

        let weights = FinalSubnetEmissionWeights::<Test>::get(reward_epoch);
        assert!(weights.subnet_weights.contains_key(&subnet_id));
    });
}

#[test]
fn test_on_initialize_paused_skips_scheduled_work_and_early_blocks_are_safe() {
    new_test_ext().execute_with(|| {
        for block in NETWORK_EPOCH_PRELIMINARIES_SLOT..=NETWORK_SUBNET_EMISSION_SLOT {
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
            .saturating_add(NETWORK_SUBNET_EMISSION_SLOT);
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

#[test]
fn test_paused_subnet_settles_allocated_history_without_new_election() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "paused-history-subnet".into();
        let deposit_amount = 10_000_000_000_000_000_000_000u128;
        let stake_amount = get_min_stake_balance();

        build_activated_subnet(
            subnet_name.clone(),
            0,
            MinSubnetNodes::<Test>::get(),
            deposit_amount,
            stake_amount,
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let election_epoch = SubnetsData::<Test>::get(subnet_id)
            .unwrap()
            .consensus_eligible_from_subnet_epoch
            .unwrap();

        set_block_to_subnet_slot_epoch(election_epoch, subnet_id);
        Network::on_initialize(System::block_number());
        assert!(SubnetElectedValidator::<Test>::contains_key(
            subnet_id,
            election_epoch
        ));

        let settlement_epoch = election_epoch.saturating_add(1);
        set_epoch(settlement_epoch, NETWORK_SUBNET_EMISSION_SLOT);
        Network::on_initialize(System::block_number());
        assert!(FinalSubnetEmissionWeights::<Test>::get(settlement_epoch)
            .subnet_weights
            .contains_key(&subnet_id));

        SubnetsData::<Test>::mutate(subnet_id, |maybe_subnet| {
            let subnet = maybe_subnet.as_mut().unwrap();
            subnet.state = SubnetState::Paused;
            subnet.consensus_eligible_from_subnet_epoch = None;
            subnet.pause = Some(SubnetPauseData {
                started_global_epoch: settlement_epoch,
                started_subnet_epoch: election_epoch,
            });
        });
        let reputation_before = SubnetReputation::<Test>::get(subnet_id);
        assert!(!SubnetConsensusSubmission::<Test>::contains_key(
            subnet_id,
            election_epoch
        ));

        set_block_to_subnet_slot_epoch(settlement_epoch, subnet_id);
        Network::on_initialize(System::block_number());

        // The exact allocated election is still evaluated, while operational work remains
        // stopped and no validator is elected for the paused epoch.
        assert!(SubnetReputation::<Test>::get(subnet_id) < reputation_before);
        assert!(!SubnetElectedValidator::<Test>::contains_key(
            subnet_id,
            settlement_epoch
        ));
    });
}

#[test]
fn test_pause_after_election_preserves_allocation_and_settlement() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "pause-before-allocation-subnet".into();
        let deposit_amount = 10_000_000_000_000_000_000_000u128;
        let stake_amount = get_min_stake_balance();
        let owner = account(1);

        build_activated_subnet(
            subnet_name.clone(),
            0,
            MinSubnetNodes::<Test>::get(),
            deposit_amount,
            stake_amount,
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        SubnetOwner::<Test>::insert(subnet_id, &owner);
        let first_election_epoch = SubnetsData::<Test>::get(subnet_id)
            .unwrap()
            .consensus_eligible_from_subnet_epoch
            .unwrap();

        // Complete the first live round so the default one-round pause cooldown expires at the
        // following subnet slot, which also elects the historical round tested below.
        set_block_to_subnet_slot_epoch(first_election_epoch, subnet_id);
        Network::on_initialize(System::block_number());

        let historical_election_epoch = first_election_epoch.saturating_add(1);
        set_epoch(historical_election_epoch, NETWORK_SUBNET_EMISSION_SLOT);
        Network::on_initialize(System::block_number());
        set_block_to_subnet_slot_epoch(historical_election_epoch, subnet_id);
        Network::on_initialize(System::block_number());
        assert!(SubnetElectedValidator::<Test>::contains_key(
            subnet_id,
            historical_election_epoch
        ));

        // Pause after the election but before the next global slot-two allocation. The lifecycle
        // transition must stop new rounds without erasing this exact historical election.
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(owner),
            subnet_id,
        ));

        let settlement_epoch = historical_election_epoch.saturating_add(1);
        set_epoch(settlement_epoch, NETWORK_SUBNET_EMISSION_SLOT);
        Network::on_initialize(System::block_number());
        assert!(FinalSubnetEmissionWeights::<Test>::get(settlement_epoch)
            .subnet_weights
            .contains_key(&subnet_id));

        let reputation_before = SubnetReputation::<Test>::get(subnet_id);
        set_block_to_subnet_slot_epoch(settlement_epoch, subnet_id);
        Network::on_initialize(System::block_number());

        // No proposal was submitted, proving the allocated historical round was evaluated. The
        // subnet remains paused, so the same hook must not start a replacement round.
        assert!(SubnetReputation::<Test>::get(subnet_id) < reputation_before);
        assert!(!SubnetElectedValidator::<Test>::contains_key(
            subnet_id,
            settlement_epoch
        ));
    });
}

#[test]
fn test_paused_subnet_can_submit_and_attest_historical_round_then_settle_successfully() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "paused-successful-history-subnet".into();
        let deposit_amount = 10_000_000_000_000_000_000_000u128;
        let stake_amount = get_min_stake_balance();

        build_activated_subnet(
            subnet_name.clone(),
            0,
            MinSubnetNodes::<Test>::get(),
            deposit_amount,
            stake_amount,
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let owner = SubnetOwner::<Test>::get(subnet_id).unwrap();
        let first_election_epoch = SubnetsData::<Test>::get(subnet_id)
            .unwrap()
            .consensus_eligible_from_subnet_epoch
            .unwrap();

        // Complete one healthy round so the default one-round cooldown expires at the next
        // subnet slot. That slot settles this round and elects the historical round that will
        // remain open across the pause.
        set_block_to_subnet_slot_epoch(first_election_epoch, subnet_id);
        Network::on_initialize(System::block_number());
        run_subnet_consensus_step_v2(subnet_id, None, None);

        let historical_election_epoch = first_election_epoch.saturating_add(1);
        set_epoch(historical_election_epoch, NETWORK_SUBNET_EMISSION_SLOT);
        Network::on_initialize(System::block_number());
        set_block_to_subnet_slot_epoch(historical_election_epoch, subnet_id);
        Network::on_initialize(System::block_number());

        let elected_node_id = get_elected_subnet_node_id(subnet_id, historical_election_epoch)
            .expect("historical round should elect before the owner pauses");
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(owner),
            subnet_id,
        ));

        // Submission and attestation belong to the already-elected round and therefore remain
        // available while paused. Lower the subnet reputation so successful settlement has an
        // observable positive effect in addition to the validator's stake reward.
        let reputation_before = Network::percentage_factor_as_u128() / 2;
        SubnetReputation::<Test>::insert(subnet_id, reputation_before);
        run_subnet_consensus_step_v2(subnet_id, None, None);
        let submission =
            SubnetConsensusSubmission::<Test>::get(subnet_id, historical_election_epoch)
                .expect("paused historical round should accept a submission");
        assert_eq!(
            submission.attests.len(),
            submission.validator_ids.len(),
            "every historical validator should attest while the subnet is paused"
        );
        let elected_stake_before = NodeSubnetStake::<Test>::get(elected_node_id, subnet_id);

        let settlement_epoch = historical_election_epoch.saturating_add(1);
        set_epoch(settlement_epoch, NETWORK_SUBNET_EMISSION_SLOT);
        Network::on_initialize(System::block_number());
        assert!(FinalSubnetEmissionWeights::<Test>::get(settlement_epoch)
            .subnet_weights
            .contains_key(&subnet_id));

        set_block_to_subnet_slot_epoch(settlement_epoch, subnet_id);
        Network::on_initialize(System::block_number());

        assert!(SubnetReputation::<Test>::get(subnet_id) > reputation_before);
        assert!(NodeSubnetStake::<Test>::get(elected_node_id, subnet_id) > elected_stake_before);
        assert!(!SubnetElectedValidator::<Test>::contains_key(
            subnet_id,
            settlement_epoch
        ));
        assert_eq!(
            SubnetsData::<Test>::get(subnet_id).unwrap().state,
            SubnetState::Paused
        );
    });
}
