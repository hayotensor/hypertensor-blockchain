use super::mock::*;
use crate::tests::test_utils::{
    insert_overwatch_node_v2, insert_subnet, manual_insert_validator, set_overwatch_node_stake,
    submit_weight, test_percent,
};
use crate::{
    ActiveOverwatchEpochLengthMultiplier, CurrentOverwatchEpoch, LastFinalizedOverwatchEpoch,
    LatestEffectiveOverwatchSignal, OverwatchEpochLengthMultiplier,
    OverwatchEpochSettlementSnapshots, OverwatchEpochStartBlock, OverwatchNodeStakeBalance,
    OverwatchNodeWeights, OverwatchStakeWeightFactor, OverwatchSubnetWeights,
    PendingOverwatchSettlement, SubnetState,
};
use frame_support::assert_ok;

const MAXIMUM_STAKE: u128 = 900;
const FIRST_SURVIVOR_STAKE: u128 = 300;
const SECOND_SURVIVOR_STAKE: u128 = 100;

#[derive(Debug, PartialEq, Eq)]
struct ParticipantIds {
    maximum: Option<u32>,
    first_survivor: u32,
    second_survivor: u32,
}

#[derive(Debug, PartialEq, Eq)]
struct FinalizedRemovalOutcome {
    effective_subnet_weights: Vec<(u32, u128)>,
    historical_subnet_weights: Vec<(u32, u128)>,
}

#[derive(Debug, PartialEq, Eq)]
struct PendingRemovalOutcome {
    finalized_subnet_weights: Vec<(u32, u128)>,
    survivor_node_weights: [u128; 2],
    survivor_reward_deltas: [u128; 2],
}

fn configure_counterfactual_round() {
    OverwatchEpochLengthMultiplier::<Test>::put(1);
    ActiveOverwatchEpochLengthMultiplier::<Test>::put(1);
    OverwatchStakeWeightFactor::<Test>::put(Network::percentage_factor_as_u128());
    insert_subnet(1, SubnetState::Active, 0);
    insert_subnet(2, SubnetState::Active, 0);
}

fn seed_counterfactual_participants(include_maximum: bool) -> ParticipantIds {
    let maximum = include_maximum.then(|| {
        manual_insert_validator(1, 101, 201);
        let node_id = insert_overwatch_node_v2(1);
        set_overwatch_node_stake(node_id, MAXIMUM_STAKE);
        node_id
    });

    manual_insert_validator(2, 102, 202);
    manual_insert_validator(3, 103, 203);
    let first_survivor = insert_overwatch_node_v2(2);
    let second_survivor = insert_overwatch_node_v2(3);
    set_overwatch_node_stake(first_survivor, FIRST_SURVIVOR_STAKE);
    set_overwatch_node_stake(second_survivor, SECOND_SURVIVOR_STAKE);

    ParticipantIds {
        maximum,
        first_survivor,
        second_survivor,
    }
}

fn submit_counterfactual_weights(epoch: u32, participants: &ParticipantIds) {
    if let Some(maximum) = participants.maximum {
        submit_weight(epoch, 1, maximum, Network::percentage_factor_as_u128());
        // Keep an explicit zero in the second subnet so missing-key semantics are also identical
        // after the maximum-stake participant is removed.
        submit_weight(epoch, 2, maximum, 0);
    }

    submit_weight(epoch, 1, participants.first_survivor, test_percent(1, 5));
    submit_weight(epoch, 2, participants.first_survivor, test_percent(4, 5));
    submit_weight(epoch, 1, participants.second_survivor, test_percent(9, 10));
    submit_weight(epoch, 2, participants.second_survivor, test_percent(1, 10));
}

fn close_counterfactual_round(epoch: u32, expected_records: u32) {
    let close_block = OverwatchEpochStartBlock::<Test>::get().saturating_add(EpochLength::get());
    System::set_block_number(close_block);
    Network::advance_overwatch_epoch(close_block);

    let pending = PendingOverwatchSettlement::<Test>::get()
        .expect("the counterfactual round must close successfully");
    assert_eq!(pending.epoch, epoch);
    assert_eq!(pending.reveal_records, expected_records);
    assert!(OverwatchEpochSettlementSnapshots::<Test>::contains_key(
        epoch
    ));
    assert_eq!(CurrentOverwatchEpoch::<Test>::get(), epoch + 1);
}

fn effective_subnet_weights() -> Vec<(u32, u128)> {
    let effective = LatestEffectiveOverwatchSignal::<Test>::get()
        .expect("a finalized round must publish an effective signal");
    assert!(effective.valid);
    effective
        .subnet_weights
        .iter()
        .map(|(subnet_id, weight)| (*subnet_id, *weight))
        .collect()
}

fn finalized_subnet_weights(epoch: u32) -> Vec<(u32, u128)> {
    [1, 2]
        .into_iter()
        .map(|subnet_id| {
            (
                subnet_id,
                OverwatchSubnetWeights::<Test>::get(epoch, subnet_id)
                    .expect("both explicitly revealed subnets must be finalized"),
            )
        })
        .collect()
}

fn run_finalized_removal_scenario(include_maximum: bool) -> FinalizedRemovalOutcome {
    new_test_ext().execute_with(|| {
        configure_counterfactual_round();
        let epoch = CurrentOverwatchEpoch::<Test>::get();
        let participants = seed_counterfactual_participants(include_maximum);
        submit_counterfactual_weights(epoch, &participants);
        close_counterfactual_round(epoch, if include_maximum { 6 } else { 4 });
        Network::calculate_overwatch_rewards();

        assert_eq!(LastFinalizedOverwatchEpoch::<Test>::get(), Some(epoch));
        let historical_before_removal = finalized_subnet_weights(epoch);
        if let Some(maximum) = participants.maximum {
            assert_ok!(Network::perform_remove_overwatch_node(maximum));
            assert_eq!(
                finalized_subnet_weights(epoch),
                historical_before_removal,
                "removing finalized influence must not rewrite public history"
            );
        }

        FinalizedRemovalOutcome {
            effective_subnet_weights: effective_subnet_weights(),
            historical_subnet_weights: historical_before_removal,
        }
    })
}

fn run_pending_removal_scenario(include_maximum: bool) -> PendingRemovalOutcome {
    new_test_ext().execute_with(|| {
        configure_counterfactual_round();
        let epoch = CurrentOverwatchEpoch::<Test>::get();
        let participants = seed_counterfactual_participants(include_maximum);
        submit_counterfactual_weights(epoch, &participants);
        close_counterfactual_round(epoch, if include_maximum { 6 } else { 4 });

        if let Some(maximum) = participants.maximum {
            assert_ok!(Network::perform_remove_overwatch_node(maximum));
            let pending = PendingOverwatchSettlement::<Test>::get().unwrap();
            assert_eq!(pending.reveal_records, 4);
            assert_eq!(
                OverwatchEpochSettlementSnapshots::<Test>::get(epoch)
                    .unwrap()
                    .nodes
                    .len(),
                2
            );
            assert_eq!(
                OverwatchNodeStakeBalance::<Test>::get(maximum),
                MAXIMUM_STAKE
            );
        }

        Network::calculate_overwatch_rewards();
        assert_eq!(LastFinalizedOverwatchEpoch::<Test>::get(), Some(epoch));
        assert!(PendingOverwatchSettlement::<Test>::get().is_none());

        let first_final_stake = OverwatchNodeStakeBalance::<Test>::get(participants.first_survivor);
        let second_final_stake =
            OverwatchNodeStakeBalance::<Test>::get(participants.second_survivor);
        if let Some(maximum) = participants.maximum {
            assert_eq!(OverwatchNodeWeights::<Test>::get(epoch, maximum), None);
            assert_eq!(
                OverwatchNodeStakeBalance::<Test>::get(maximum),
                MAXIMUM_STAKE
            );
        }

        PendingRemovalOutcome {
            finalized_subnet_weights: finalized_subnet_weights(epoch),
            survivor_node_weights: [
                OverwatchNodeWeights::<Test>::get(epoch, participants.first_survivor)
                    .expect("the first survivor must be scored"),
                OverwatchNodeWeights::<Test>::get(epoch, participants.second_survivor)
                    .expect("the second survivor must be scored"),
            ],
            survivor_reward_deltas: [
                first_final_stake
                    .checked_sub(FIRST_SURVIVOR_STAKE)
                    .expect("settlement cannot reduce survivor principal"),
                second_final_stake
                    .checked_sub(SECOND_SURVIVOR_STAKE)
                    .expect("settlement cannot reduce survivor principal"),
            ],
        }
    })
}

#[test]
fn finalized_max_stake_removal_matches_round_where_node_never_existed() {
    let removed_after_finalization = run_finalized_removal_scenario(true);
    let never_present = run_finalized_removal_scenario(false);

    assert_eq!(removed_after_finalization.effective_subnet_weights.len(), 2);
    assert_eq!(never_present.effective_subnet_weights.len(), 2);
    for (removed_weight, never_present_weight) in removed_after_finalization
        .effective_subnet_weights
        .iter()
        .zip(never_present.effective_subnet_weights.iter())
    {
        assert_eq!(removed_weight, never_present_weight);
    }
    assert_ne!(
        removed_after_finalization.historical_subnet_weights,
        removed_after_finalization.effective_subnet_weights,
        "the max-stake node must have influenced immutable history before removal"
    );
}

#[test]
fn pending_max_stake_removal_matches_round_where_node_never_existed() {
    let removed_before_finalization = run_pending_removal_scenario(true);
    let never_present = run_pending_removal_scenario(false);

    assert_eq!(
        removed_before_finalization.finalized_subnet_weights,
        never_present.finalized_subnet_weights
    );
    assert_eq!(
        removed_before_finalization.survivor_node_weights,
        never_present.survivor_node_weights
    );
    assert_eq!(
        removed_before_finalization.survivor_reward_deltas,
        never_present.survivor_reward_deltas
    );
}
