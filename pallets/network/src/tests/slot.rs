use super::mock::*;
use crate::tests::test_utils::*;
use crate::{
    CurrentOverwatchEpoch, FinalSubnetEmissionWeights, LastFinalizedOverwatchEpoch, MaxSubnetNodes,
    MaxSubnets, MinSubnetMinStake, NewRegistrationCostMultiplier, OverwatchEpochLengthMultiplier,
    OverwatchNodeStakeBalance, OverwatchNodeValidatorId, OverwatchNodeWeights,
    OverwatchSubnetWeights, PendingOverwatchSettlement, QueueImmunityEpochs,
    RegisteredSubnetNodesData, SubnetConsensusSubmission, SubnetDelegateStakeRewardsPercentage,
    SubnetElectedValidator, SubnetName, SubnetNetFlow, SubnetNetFlowSmoothedWeight,
    SubnetNetFlowSmoothingAlpha, SubnetNodeQueue, SubnetRemovalReason, SubnetWeightFactors,
    SubnetWeightFactorsData, SubnetsData, TotalActiveSubnets, TotalDelegateStake,
    TotalElectableNodes, TotalSubnetDelegateStakeBalance, TotalSubnetElectableNodes,
};
use frame_support::traits::OnInitialize;
use frame_support::weights::WeightMeter;
use sp_std::collections::btree_map::BTreeMap;

// Overwatch node functions in the slot.rs file are in tests/overwatch_nodes.rs

// calculate_overwatch_rewards: test_calculate_overwatch_rewards
// emission_step: See incentives_protocol.rs
// 	- test_distribute_rewards_prioritized_queue_node_id: Tests node registration queue prioritization
// 	- test_distribute_rewards_remove_queue_node_id: Tests removing node from registration
// 	- test_distribute_rewards_graduate_idle_to_included: Tests graduating nodes
// 	- test_distribute_rewards_graduate_included_to_validator: Tests emissions generation
// handle_registration_queue
// See:
//  - test_distribute_rewards_prioritized_queue_node_id
//  - test_distribute_rewards_remove_queue_node_id
// handle_subnet_emission_weights: test_handle_subnet_emission_weights
// calculate_subnet_weights: test_calculate_subnet_weights
// precheck_subnet_consensus_submission: test_precheck_subnet_consensus_submission
// calculate_rewards: test_calculate_rewards

fn build_active_subnet_ids(count: u32) -> Vec<u32> {
    NewRegistrationCostMultiplier::<Test>::set(Network::percentage_factor_as_u128());

    let deposit_amount: u128 = 10000000000000000000000;
    let amount: u128 = 1000000000000000000000;
    let end = 12;

    for s in 0..count {
        let subnet_name: Vec<u8> = format!("net-flow-subnet-{s}").into();
        build_activated_subnet(subnet_name.clone().into(), 0, end, deposit_amount, amount);
    }

    let subnet_ids: Vec<u32> = (0..count)
        .map(|s| {
            let subnet_name: Vec<u8> = format!("net-flow-subnet-{s}").into();
            SubnetName::<Test>::get(subnet_name).unwrap()
        })
        .collect();
    set_to_first_reward_weight_epoch(&subnet_ids);
    subnet_ids
}

fn set_to_first_reward_weight_epoch(subnet_ids: &[u32]) -> u32 {
    let first_reward_epoch = subnet_ids
        .iter()
        .filter_map(|subnet_id| {
            SubnetsData::<Test>::get(subnet_id)?.consensus_eligible_from_subnet_epoch
        })
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    set_epoch(first_reward_epoch, 2);
    first_reward_epoch
}

fn seed_exact_prior_election(subnet_id: u32, epoch: u32) {
    let previous_epoch = epoch
        .checked_sub(1)
        .expect("reward weight tests require an epoch after genesis");
    insert_elected_subnet_node(subnet_id, previous_epoch, 1);
}

#[test]
fn test_calculate_overwatch_rewards() {
    new_test_ext().execute_with(|| {
        NewRegistrationCostMultiplier::<Test>::set(Network::percentage_factor_as_u128());

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let max_subnets = MaxSubnets::<Test>::get();

        let end = 4;

        for s in 0..max_subnets {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            build_activated_subnet(subnet_name.clone().into(), 0, end, deposit_amount, amount);
        }

        set_overwatch_epoch(1);

        let default_weight = Network::percentage_factor_as_u128();
        let overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();

        let overwatch_node_id = insert_overwatch_node_v2(1);
        let starting_stake = 100;
        set_overwatch_node_stake(overwatch_node_id, starting_stake);

        for s in 0..max_subnets {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

            submit_weight(
                overwatch_epoch,
                subnet_id,
                overwatch_node_id,
                default_weight,
            );
        }

        let multiplier = OverwatchEpochLengthMultiplier::<Test>::get();
        let boundary =
            System::block_number().saturating_add(EpochLength::get().saturating_mul(multiplier));
        System::set_block_number(boundary);
        Network::on_initialize(boundary);

        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), overwatch_epoch + 1);
        assert_eq!(
            PendingOverwatchSettlement::<Test>::get().map(|settlement| settlement.epoch),
            Some(overwatch_epoch)
        );

        System::set_block_number(boundary + 1);
        Network::on_initialize(boundary + 1);

        let expected_reward = OVERWATCH_EPOCH_EMISSIONS.saturating_mul(multiplier as u128);
        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id),
            starting_stake + expected_reward
        );
        assert_eq!(
            LastFinalizedOverwatchEpoch::<Test>::get(),
            Some(overwatch_epoch)
        );
        assert!(PendingOverwatchSettlement::<Test>::get().is_none());
        assert!(OverwatchNodeWeights::<Test>::contains_key(
            overwatch_epoch,
            overwatch_node_id
        ));

        // A duplicate invocation cannot pay the same epoch again.
        Network::calculate_overwatch_rewards();
        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id),
            starting_stake + expected_reward
        );
    });
}

// #[test]
// fn test_emission_step() {
//   new_test_ext().execute_with(|| {
// 		// See:
// 		// 	test_distribute_rewards_prioritized_queue_node_id: Tests node registration queue prioritization
// 		// 	test_distribute_rewards_remove_queue_node_id: Tests removing node from registration
// 		// 	test_distribute_rewards_graduate_idle_to_included: Tests graduating nodes
// 		// 	test_distribute_rewards_graduate_included_to_validator: Tests emissions generation
// 	});
// }

#[test]
fn test_handle_subnet_emission_weights() {
    new_test_ext().execute_with(|| {
        NewRegistrationCostMultiplier::<Test>::set(Network::percentage_factor_as_u128());

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 12;

        for s in 0..max_subnets {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            build_activated_subnet(subnet_name.clone().into(), 0, end, deposit_amount, amount);
        }
        let subnet_ids: Vec<u32> = (0..max_subnets)
            .map(|s| {
                let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
                SubnetName::<Test>::get(subnet_name).unwrap()
            })
            .collect();
        let current_epoch = set_to_first_reward_weight_epoch(&subnet_ids);
        for s in 0..max_subnets {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
            seed_exact_prior_election(subnet_id, current_epoch);
        }

        let _ = Network::handle_subnet_emission_weights(current_epoch);

        let subnet_emission_weights = FinalSubnetEmissionWeights::<Test>::get(current_epoch);

        for s in 0..max_subnets {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

            let subnet_weight = subnet_emission_weights.subnet_weights.get(&subnet_id);
            assert!(subnet_weight.is_some());
            assert!(*subnet_weight.unwrap() > 0);
            assert!(*subnet_weight.unwrap() <= Network::percentage_factor_as_u128());
        }
    });
}

#[test]
fn test_calculate_subnet_weights() {
    new_test_ext().execute_with(|| {
        NewRegistrationCostMultiplier::<Test>::set(Network::percentage_factor_as_u128());

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 12;

        for s in 0..max_subnets {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            build_activated_subnet(subnet_name.clone().into(), 0, end, deposit_amount, amount);
        }
        let subnet_ids: Vec<u32> = (0..max_subnets)
            .map(|s| {
                let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
                SubnetName::<Test>::get(subnet_name).unwrap()
            })
            .collect();
        let current_epoch = set_to_first_reward_weight_epoch(&subnet_ids);
        for s in 0..max_subnets {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
            seed_exact_prior_election(subnet_id, current_epoch);
        }

        let (subnet_weights, mut weight) = Network::calculate_subnet_weights(current_epoch);

        for s in 0..max_subnets {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

            let subnet_weight = subnet_weights.get(&subnet_id);
            assert!(subnet_weight.is_some());
            assert!(*subnet_weight.unwrap() > 0);
            assert!(*subnet_weight.unwrap() <= Network::percentage_factor_as_u128());
        }
    });
}

#[test]
fn test_calculate_subnet_weights_never_exceeds_full_allocation() {
    new_test_ext().execute_with(|| {
        let subnet_ids = build_active_subnet_ids(11);
        SubnetWeightFactors::<Test>::put(SubnetWeightFactorsData {
            delegate_stake: 0,
            node_count: Network::percentage_factor_as_u128(),
            net_flow: 0,
        });

        let current_epoch = Network::get_current_epoch_as_u32();
        for subnet_id in subnet_ids.iter().copied() {
            seed_exact_prior_election(subnet_id, current_epoch);
        }

        let (subnet_weights, _) = Network::calculate_subnet_weights(current_epoch);
        let total_weight = subnet_weights
            .values()
            .copied()
            .fold(0u128, u128::saturating_add);

        assert_eq!(subnet_weights.len(), subnet_ids.len());
        assert!(total_weight <= Network::percentage_factor_as_u128());
    });
}

#[test]
fn test_calculate_subnet_weights_reuses_last_finalized_overwatch_epoch() {
    new_test_ext().execute_with(|| {
        let subnet_ids = build_active_subnet_ids(2);
        let first_subnet = subnet_ids[0];
        let second_subnet = subnet_ids[1];
        let finalized_overwatch_epoch = 5;

        CurrentOverwatchEpoch::<Test>::put(99);
        LastFinalizedOverwatchEpoch::<Test>::put(finalized_overwatch_epoch);

        OverwatchSubnetWeights::<Test>::insert(
            finalized_overwatch_epoch,
            first_subnet,
            Network::percentage_factor_as_u128(),
        );
        OverwatchSubnetWeights::<Test>::insert(
            finalized_overwatch_epoch,
            second_subnet,
            test_percent(1, 2),
        );

        // Conflicting weights under the old derived `current - 1` key prove that it is no longer
        // consulted.
        OverwatchSubnetWeights::<Test>::insert(98, first_subnet, test_percent(1, 10));
        OverwatchSubnetWeights::<Test>::insert(
            98,
            second_subnet,
            Network::percentage_factor_as_u128(),
        );

        for _ in 0..2 {
            let current_epoch = Network::get_current_epoch_as_u32();
            seed_exact_prior_election(first_subnet, current_epoch);
            seed_exact_prior_election(second_subnet, current_epoch);

            let (weights, _) = Network::calculate_subnet_weights(current_epoch);
            assert!(weights[&first_subnet] > weights[&second_subnet]);
            assert_eq!(LastFinalizedOverwatchEpoch::<Test>::get(), Some(5));

            increase_epochs(1);
        }
    });
}

#[test]
fn test_empty_finalized_overwatch_epoch_replaces_stale_signal_with_default() {
    fn calculate_with_prior_scores(
        first_prior_score: u128,
        second_prior_score: u128,
        use_empty_finalized_epoch: bool,
    ) -> BTreeMap<u32, u128> {
        new_test_ext().execute_with(|| {
            let subnet_ids = build_active_subnet_ids(2);
            let first_subnet = subnet_ids[0];
            let second_subnet = subnet_ids[1];
            let stale_epoch = 5;
            let empty_epoch = 6;

            OverwatchSubnetWeights::<Test>::insert(stale_epoch, first_subnet, first_prior_score);
            OverwatchSubnetWeights::<Test>::insert(stale_epoch, second_subnet, second_prior_score);
            LastFinalizedOverwatchEpoch::<Test>::put(stale_epoch);

            // Finalizing an empty round advances the explicit marker even though it creates no
            // subnet keys.
            queue_overwatch_settlement(empty_epoch);
            Network::calculate_overwatch_rewards();
            assert_eq!(
                LastFinalizedOverwatchEpoch::<Test>::get(),
                Some(empty_epoch)
            );
            assert!(!OverwatchSubnetWeights::<Test>::contains_key(
                empty_epoch,
                first_subnet
            ));

            if !use_empty_finalized_epoch {
                LastFinalizedOverwatchEpoch::<Test>::put(stale_epoch);
            }

            let current_epoch = Network::get_current_epoch_as_u32();
            seed_exact_prior_election(first_subnet, current_epoch);
            seed_exact_prior_election(second_subnet, current_epoch);
            Network::calculate_subnet_weights(current_epoch).0
        })
    }

    let fallback_weights = calculate_with_prior_scores(
        Network::percentage_factor_as_u128(),
        test_percent(1, 10),
        true,
    );
    let fallback_after_stale_change =
        calculate_with_prior_scores(0, Network::percentage_factor_as_u128(), true);
    assert_eq!(fallback_weights, fallback_after_stale_change);

    let stale_weights = calculate_with_prior_scores(0, Network::percentage_factor_as_u128(), false);
    assert_ne!(fallback_weights, stale_weights);
}

// Only subnets that are active and live get weights (no registering or paused subnets)
#[test]
fn test_calculate_subnet_weights_active_live_only() {
    new_test_ext().execute_with(|| {
        NewRegistrationCostMultiplier::<Test>::set(Network::percentage_factor_as_u128());

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 12;

        for s in 0..max_subnets - 1 {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            build_activated_subnet(subnet_name.clone().into(), 0, end, deposit_amount, amount);
        }

        // add a registering subnet
        let registering_n = max_subnets - 1;
        let registering_subnet_name: Vec<u8> = format!("subnet-name-{registering_n}").into();
        build_registered_subnet(
            registering_subnet_name.clone(),
            0,
            4,
            deposit_amount,
            amount,
            true,
            None,
        );
        let registering_subnet_id =
            SubnetName::<Test>::get(registering_subnet_name.clone()).unwrap();

        let active_subnet_ids: Vec<u32> = (0..max_subnets - 1)
            .map(|s| {
                let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
                SubnetName::<Test>::get(subnet_name).unwrap()
            })
            .collect();
        let current_epoch = set_to_first_reward_weight_epoch(&active_subnet_ids);
        for s in 0..max_subnets - 1 {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
            seed_exact_prior_election(subnet_id, current_epoch);
        }

        let (subnet_weights, mut weight) = Network::calculate_subnet_weights(current_epoch);

        for s in 0..max_subnets {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
            let subnet_weight = subnet_weights.get(&subnet_id);

            if subnet_id == registering_subnet_id {
                assert!(subnet_weight.is_none());
            } else {
                assert!(subnet_weight.is_some());
                assert!(*subnet_weight.unwrap() > 0);
                assert!(*subnet_weight.unwrap() <= Network::percentage_factor_as_u128());
            }
        }
    });
}

#[test]
fn test_calculate_subnet_weights_requires_exact_prior_election() {
    new_test_ext().execute_with(|| {
        let subnet_ids = build_active_subnet_ids(3);
        let current_epoch = Network::get_current_epoch_as_u32();
        let previous_epoch = current_epoch
            .checked_sub(1)
            .expect("active subnet test must be past genesis");
        let stale_epoch = previous_epoch
            .checked_sub(1)
            .expect("active subnet test must have a stale epoch");

        let no_election_subnet_id = subnet_ids[0];
        let exact_election_subnet_id = subnet_ids[1];
        let stale_election_subnet_id = subnet_ids[2];

        for subnet_id in subnet_ids.iter().copied() {
            assert_eq!(
                Network::is_subnet_active_and_live(subnet_id, current_epoch),
                Some(true)
            );
        }

        insert_elected_subnet_node(exact_election_subnet_id, previous_epoch, 1);
        insert_elected_subnet_node(stale_election_subnet_id, stale_epoch, 1);

        let (subnet_weights, _) = Network::calculate_subnet_weights(current_epoch);

        assert!(!subnet_weights.contains_key(&no_election_subnet_id));
        assert!(subnet_weights.contains_key(&exact_election_subnet_id));
        assert!(!subnet_weights.contains_key(&stale_election_subnet_id));
    });
}

#[test]
fn test_ineligible_subnet_totals_do_not_dilute_eligible_weights() {
    new_test_ext().execute_with(|| {
        let subnet_ids = build_active_subnet_ids(3);
        let current_epoch = Network::get_current_epoch_as_u32();
        let first_eligible_subnet_id = subnet_ids[0];
        let second_eligible_subnet_id = subnet_ids[1];
        let ineligible_subnet_id = subnet_ids[2];

        seed_exact_prior_election(first_eligible_subnet_id, current_epoch);
        seed_exact_prior_election(second_eligible_subnet_id, current_epoch);

        TotalSubnetDelegateStakeBalance::<Test>::insert(first_eligible_subnet_id, 300);
        TotalSubnetDelegateStakeBalance::<Test>::insert(second_eligible_subnet_id, 100);
        TotalSubnetDelegateStakeBalance::<Test>::insert(ineligible_subnet_id, 0);
        TotalDelegateStake::<Test>::set(400);

        TotalSubnetElectableNodes::<Test>::insert(first_eligible_subnet_id, 1);
        TotalSubnetElectableNodes::<Test>::insert(second_eligible_subnet_id, 3);
        TotalSubnetElectableNodes::<Test>::insert(ineligible_subnet_id, 0);
        TotalElectableNodes::<Test>::set(4);

        for subnet_id in subnet_ids.iter().copied() {
            SubnetNetFlow::<Test>::remove(subnet_id);
            SubnetNetFlowSmoothedWeight::<Test>::remove(subnet_id);
        }

        let (baseline_weights, _) = Network::calculate_subnet_weights(current_epoch);
        assert_eq!(baseline_weights.len(), 2);
        assert_eq!(
            baseline_weights.get(&first_eligible_subnet_id),
            baseline_weights.get(&second_eligible_subnet_id)
        );
        assert!(!baseline_weights.contains_key(&ineligible_subnet_id));

        let ineligible_delegate_stake = 1_000_000;
        let ineligible_electable_nodes = 100_000;
        TotalSubnetDelegateStakeBalance::<Test>::insert(
            ineligible_subnet_id,
            ineligible_delegate_stake,
        );
        TotalDelegateStake::<Test>::set(400 + ineligible_delegate_stake);
        TotalSubnetElectableNodes::<Test>::insert(ineligible_subnet_id, ineligible_electable_nodes);
        TotalElectableNodes::<Test>::set(4 + ineligible_electable_nodes);

        let (weights_with_ineligible_extremes, _) =
            Network::calculate_subnet_weights(current_epoch);

        assert_eq!(weights_with_ineligible_extremes, baseline_weights);
        assert!(!weights_with_ineligible_extremes.contains_key(&ineligible_subnet_id));
    });
}

#[test]
fn test_emission_step_elects_live_subnet_without_final_emission_weights() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "bootstrap-election-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let first_consensus_epoch = Network::get_current_epoch_as_u32().saturating_add(1);

        set_block_to_subnet_slot_epoch(first_consensus_epoch, subnet_id);
        let block = System::block_number();
        let current_epoch = Network::get_current_epoch_as_u32();
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        assert_eq!(current_epoch, first_consensus_epoch);
        assert_eq!(
            Network::is_subnet_active_and_live(subnet_id, current_epoch),
            Some(true)
        );
        assert!(FinalSubnetEmissionWeights::<Test>::get(current_epoch)
            .subnet_weights
            .is_empty());
        assert!(get_elected_subnet_node_id(subnet_id, current_subnet_epoch).is_none());

        Network::emission_step(
            &mut WeightMeter::new(),
            block,
            current_epoch,
            current_subnet_epoch,
            subnet_id,
        );

        assert!(get_elected_subnet_node_id(subnet_id, current_subnet_epoch).is_some());
    });
}

#[test]
fn test_get_net_flow_weights_smoothes_relative_weights() {
    new_test_ext().execute_with(|| {
        let subnet_ids = build_active_subnet_ids(3);
        let alpha = test_percent(1, 2);
        SubnetNetFlowSmoothingAlpha::<Test>::set(alpha);

        SubnetNetFlow::<Test>::insert(subnet_ids[0], -100);
        SubnetNetFlow::<Test>::insert(subnet_ids[1], 0);
        SubnetNetFlow::<Test>::insert(subnet_ids[2], 100);

        let (weights, _) = Network::get_net_flow_weights(
            SubnetsData::<Test>::iter().collect(),
            Network::get_current_epoch_as_u32(),
        );

        let raw_middle_weight = Network::percent_div(100, 300);
        let raw_high_weight = Network::percent_div(200, 300);
        let expected_middle_weight = Network::percent_mul(raw_middle_weight, alpha);
        let expected_high_weight = Network::percent_mul(raw_high_weight, alpha);

        assert_eq!(weights.get(&subnet_ids[0]).copied().unwrap_or(0), 0);
        assert_eq!(
            weights.get(&subnet_ids[1]).copied().unwrap_or(0),
            expected_middle_weight
        );
        assert_eq!(
            weights.get(&subnet_ids[2]).copied().unwrap_or(0),
            expected_high_weight
        );
        assert!(expected_high_weight > expected_middle_weight);

        for subnet_id in subnet_ids.iter().copied() {
            assert_eq!(SubnetNetFlow::<Test>::get(subnet_id), 0);
            assert_eq!(
                SubnetNetFlowSmoothedWeight::<Test>::get(subnet_id),
                weights.get(&subnet_id).copied().unwrap_or(0)
            );
        }
    });
}

#[test]
fn test_get_net_flow_weights_decays_on_equal_flow_epoch() {
    new_test_ext().execute_with(|| {
        let subnet_ids = build_active_subnet_ids(3);
        let alpha = test_percent(1, 2);
        SubnetNetFlowSmoothingAlpha::<Test>::set(alpha);

        SubnetNetFlow::<Test>::insert(subnet_ids[0], -100);
        SubnetNetFlow::<Test>::insert(subnet_ids[1], 0);
        SubnetNetFlow::<Test>::insert(subnet_ids[2], 100);

        let (first_weights, _) = Network::get_net_flow_weights(
            SubnetsData::<Test>::iter().collect(),
            Network::get_current_epoch_as_u32(),
        );
        let first_high_weight = first_weights.get(&subnet_ids[2]).copied().unwrap_or(0);
        assert!(first_high_weight > 0);

        let (second_weights, _) = Network::get_net_flow_weights(
            SubnetsData::<Test>::iter().collect(),
            Network::get_current_epoch_as_u32(),
        );

        let expected_decayed_weight = Network::percent_mul(first_high_weight, alpha);
        assert_eq!(
            second_weights.get(&subnet_ids[2]).copied().unwrap_or(0),
            expected_decayed_weight
        );
        assert!(expected_decayed_weight < first_high_weight);
    });
}

#[test]
fn test_get_net_flow_weights_excludes_non_live_subnets_and_clears_storage() {
    new_test_ext().execute_with(|| {
        let _active_subnet_ids = build_active_subnet_ids(2);

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let registering_subnet_name: Vec<u8> = "net-flow-registering-subnet".into();
        build_registered_subnet(
            registering_subnet_name.clone(),
            0,
            4,
            deposit_amount,
            amount,
            true,
            None,
        );
        let registering_subnet_id =
            SubnetName::<Test>::get(registering_subnet_name.clone()).unwrap();

        SubnetNetFlow::<Test>::insert(registering_subnet_id, 1000);
        SubnetNetFlowSmoothedWeight::<Test>::insert(
            registering_subnet_id,
            Network::percentage_factor_as_u128(),
        );

        let (weights, _) = Network::get_net_flow_weights(
            SubnetsData::<Test>::iter().collect(),
            Network::get_current_epoch_as_u32(),
        );

        assert!(!weights.contains_key(&registering_subnet_id));
        assert_eq!(SubnetNetFlow::<Test>::get(registering_subnet_id), 0);
        assert_eq!(
            SubnetNetFlowSmoothedWeight::<Test>::get(registering_subnet_id),
            0
        );
    });
}

#[test]
fn test_subnet_removal_clears_net_flow_storage() {
    new_test_ext().execute_with(|| {
        let subnet_id = build_active_subnet_ids(1)[0];

        SubnetNetFlow::<Test>::insert(subnet_id, -100);
        SubnetNetFlowSmoothedWeight::<Test>::insert(
            subnet_id,
            Network::percentage_factor_as_u128(),
        );

        Network::do_remove_subnet(subnet_id, SubnetRemovalReason::Owner);

        assert_eq!(SubnetNetFlow::<Test>::get(subnet_id), 0);
        assert_eq!(SubnetNetFlowSmoothedWeight::<Test>::get(subnet_id), 0);
    });
}

#[test]
fn test_subnet_net_flow_large_amount_does_not_wrap_signed() {
    new_test_ext().execute_with(|| {
        let subnet_id = build_active_subnet_ids(1)[0];

        Network::increase_account_delegate_stake(&account(1), subnet_id, u128::MAX, 0);
        assert_eq!(SubnetNetFlow::<Test>::get(subnet_id), i128::MAX);

        SubnetNetFlow::<Test>::remove(subnet_id);
        Network::decrease_account_delegate_stake(&account(1), subnet_id, u128::MAX, 0);
        assert_eq!(SubnetNetFlow::<Test>::get(subnet_id), -i128::MAX);
    });
}

#[test]
fn test_precheck_subnet_consensus_submission() {
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

        let new_start = end;
        let new_end = new_start + 4;
        build_registered_nodes_in_queue(subnet_id, new_start, new_end, deposit_amount, amount);

        QueueImmunityEpochs::<Test>::insert(subnet_id, 1);

        // Push passed immunity period so node can be removed from queue
        let immunity_epochs = QueueImmunityEpochs::<Test>::get(subnet_id);
        let removal_epoch = SubnetNodeQueue::<Test>::get(subnet_id)
            .first()
            .unwrap()
            .classification
            .start_epoch
            .saturating_add(immunity_epochs)
            .saturating_add(1);
        set_block_to_subnet_slot_epoch(removal_epoch, subnet_id);

        // Store data
        let mut registered_nodes_data: BTreeMap<u32, u32> = BTreeMap::new(); // node ID => start_epoch
        for n in new_start..new_end {
            let _n = n + 1;
            let hotkey = get_hotkey(subnet_id, max_subnet_nodes, max_subnets, _n);
            let hotkey_subnet_node_id = _n;
            let subnet_node_data =
                RegisteredSubnetNodesData::<Test>::try_get(subnet_id, hotkey_subnet_node_id)
                    .unwrap();
            registered_nodes_data.insert(
                hotkey_subnet_node_id,
                subnet_node_data.classification.start_epoch,
            );
        }

        let queue = SubnetNodeQueue::<Test>::get(subnet_id);
        assert_eq!(queue.len() as u32, new_end - new_start);

        let first = queue.first().unwrap();
        let last = queue.last().unwrap();
        // Sanity check
        assert_ne!(first.id, last.id);

        let exists = queue.iter().any(|node| node.id == last.id);

        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        Network::elect_validator(subnet_id, subnet_epoch, System::block_number());
        let validator_id = get_elected_subnet_node_id(subnet_id, subnet_epoch);
        assert!(validator_id != None, "Validator is None");
        assert!(validator_id != Some(0), "Validator is 0");

        run_subnet_consensus_step_v2(subnet_id, Some(last.id), Some(first.id));

        let submission = SubnetConsensusSubmission::<Test>::get(
            subnet_id,
            Network::get_current_subnet_epoch_as_u32(subnet_id),
        );
        assert!(submission
            .clone()
            .unwrap()
            .prioritize_queue_node_id
            .is_some());
        assert_eq!(
            submission
                .clone()
                .unwrap()
                .prioritize_queue_node_id
                .unwrap(),
            last.id
        );
        assert!(submission.clone().unwrap().remove_queue_node_id.is_some());
        assert_eq!(
            submission.clone().unwrap().remove_queue_node_id.unwrap(),
            first.id
        );

        set_block_to_subnet_slot_epoch(removal_epoch.saturating_add(1), subnet_id);

        let (consensus_submission_data, consensus_submission_block_weight) =
            Network::precheck_subnet_consensus_submission(
                subnet_id,
                Network::get_current_epoch_as_u32() - 1,
                Network::get_current_epoch_as_u32(),
            );

        let consensus_results = consensus_submission_data.unwrap();

        let validator_subnet_node_id = consensus_results.validator_subnet_node_id;
        let attestation_ratio = consensus_results.attestation_ratio;
        let weight_sum = consensus_results.weight_sum;
        let data_length = consensus_results.data_length;
        let data = consensus_results.data;
        let attests = consensus_results.attests;
        let subnet_nodes = consensus_results.subnet_nodes;
        let prioritize_queue_node_id = consensus_results.prioritize_queue_node_id;
        let remove_queue_node_id = consensus_results.remove_queue_node_id;

        assert_eq!(validator_subnet_node_id, validator_id.unwrap());
        assert_eq!(attestation_ratio, Network::percentage_factor_as_u128());
        assert_ne!(weight_sum, 0);
        assert_eq!(data_length, end);
        // assert_eq!(data, Network::percentage_factor_as_u128());
        assert_eq!(attests.len(), end as usize);
        assert_eq!(subnet_nodes.len(), end as usize);
        assert_eq!(prioritize_queue_node_id, Some(last.id));
        assert_eq!(remove_queue_node_id, Some(first.id));
    });
}

#[test]
fn test_precheck_queue_removal_uses_saturating_immunity_epoch() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        build_registered_nodes_in_queue(subnet_id, end, end + 4, deposit_amount, amount);

        let mut queue = SubnetNodeQueue::<Test>::get(subnet_id);
        let remove_id = queue.first().unwrap().id;
        queue[0].classification.start_epoch = 10;
        SubnetNodeQueue::<Test>::set(subnet_id, queue);
        QueueImmunityEpochs::<Test>::insert(subnet_id, u32::MAX);

        increase_epochs(20);
        set_block_to_subnet_slot_epoch(Network::get_current_epoch_as_u32(), subnet_id);
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        assert!(subnet_epoch > 10);
        Network::elect_validator(subnet_id, subnet_epoch, System::block_number());
        let validator_id = get_elected_subnet_node_id(subnet_id, subnet_epoch);
        assert!(validator_id.is_some());

        let prioritize_id = SubnetNodeQueue::<Test>::get(subnet_id).last().unwrap().id;
        let _ = max_subnet_nodes;
        let _ = max_subnets;
        run_subnet_consensus_step_v2(subnet_id, Some(prioritize_id), Some(remove_id));

        let submission = SubnetConsensusSubmission::<Test>::get(subnet_id, subnet_epoch).unwrap();
        assert_eq!(submission.remove_queue_node_id, None);
    });
}

#[test]
fn test_calculate_rewards() {
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
        increase_epochs(1);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        seed_equal_validator_delegate_stake_for_subnet(subnet_id);
        // The first live election is allocated at the following global epoch.
        increase_epochs(1);

        let current_epoch = Network::get_current_epoch_as_u32();
        seed_exact_prior_election(subnet_id, current_epoch);

        let _ = Network::handle_subnet_emission_weights(current_epoch);

        let subnet_emission_weights = FinalSubnetEmissionWeights::<Test>::get(current_epoch);
        let subnet_weight = subnet_emission_weights.subnet_weights.get(&subnet_id);

        let delegate_stake_rewards_percentage =
            SubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id);

        let (rewards_data, rewards_block_weight) = Network::calculate_rewards(
            subnet_id,
            subnet_emission_weights.subnets_emissions,
            *subnet_weight.unwrap(),
        );

        let overall_subnet_reward = rewards_data.overall_subnet_reward;
        let subnet_owner_reward = rewards_data.subnet_owner_reward;
        let subnet_rewards = rewards_data.subnet_rewards;
        let delegate_stake_rewards = rewards_data.delegate_stake_rewards;
        let subnet_node_rewards = rewards_data.subnet_node_rewards;

        let expected_delegate_stake_rewards: u128 =
            Network::percent_mul(subnet_rewards, delegate_stake_rewards_percentage);
        let expected_subnet_node_rewards: u128 =
            subnet_rewards.saturating_sub(expected_delegate_stake_rewards);

        assert!(overall_subnet_reward > 0);
        assert!(subnet_owner_reward > 0);
        assert!(subnet_rewards > 0);
        assert_eq!(delegate_stake_rewards, expected_delegate_stake_rewards);
        assert_eq!(subnet_node_rewards, expected_subnet_node_rewards);
    });
}
