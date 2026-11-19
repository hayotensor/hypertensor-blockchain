use super::mock::*;
use super::test_utils::*;
use crate::utilities::consensus_history::{
    CONSENSUS_HISTORY_RETENTION_EPOCHS, MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT,
};
use crate::{
    ConsensusAttestorWeightSnapshot, ConsensusHistoryCleanupQueue, ConsensusHistoryPruneCursor,
    ConsensusRoundSettlementStatus, ElectedConsensusRound, MinSubnetMinStake,
    SubnetConsensusAttestationData, SubnetConsensusAttestorWeights, SubnetConsensusProposalArgs,
    SubnetConsensusSubmissionMaxItems, SubnetElectedValidator, SubnetName, SubnetRemovalReason,
    ValidatorArgs,
};
use frame_support::traits::Hooks;
use frame_support::weights::{Weight, WeightMeter};

const ATTESTORS_PER_ROUND: u32 = 3;

fn payload(byte: u8) -> ValidatorArgs<Test> {
    vec![byte; 64].try_into().unwrap()
}

fn insert_round(subnet_id: u32, epoch: u32) {
    SubnetElectedValidator::<Test>::insert(subnet_id, epoch, ElectedConsensusRound::default());
    ConsensusRoundSettlementStatus::<Test>::insert(subnet_id, epoch, true);
    SubnetConsensusSubmissionMaxItems::<Test>::insert(subnet_id, epoch, 4);
    SubnetConsensusProposalArgs::<Test>::insert(subnet_id, epoch, payload(1));
    SubnetConsensusAttestorWeights::<Test>::insert(
        subnet_id,
        epoch,
        ConsensusAttestorWeightSnapshot::default(),
    );
    for subnet_node_id in 1..=ATTESTORS_PER_ROUND {
        SubnetConsensusAttestationData::<Test>::insert(
            (subnet_id, epoch, subnet_node_id),
            payload(2),
        );
    }
}

fn round_fully_present(subnet_id: u32, epoch: u32) -> bool {
    SubnetElectedValidator::<Test>::contains_key(subnet_id, epoch)
        && ConsensusRoundSettlementStatus::<Test>::contains_key(subnet_id, epoch)
        && SubnetConsensusSubmissionMaxItems::<Test>::contains_key(subnet_id, epoch)
        && SubnetConsensusProposalArgs::<Test>::contains_key(subnet_id, epoch)
        && SubnetConsensusAttestorWeights::<Test>::contains_key(subnet_id, epoch)
        && SubnetConsensusAttestationData::<Test>::iter_prefix((subnet_id, epoch)).count()
            == ATTESTORS_PER_ROUND as usize
}

fn round_fully_absent(subnet_id: u32, epoch: u32) -> bool {
    !SubnetElectedValidator::<Test>::contains_key(subnet_id, epoch)
        && !ConsensusRoundSettlementStatus::<Test>::contains_key(subnet_id, epoch)
        && !SubnetConsensusSubmissionMaxItems::<Test>::contains_key(subnet_id, epoch)
        && !SubnetConsensusProposalArgs::<Test>::contains_key(subnet_id, epoch)
        && !SubnetConsensusAttestorWeights::<Test>::contains_key(subnet_id, epoch)
        && SubnetConsensusAttestationData::<Test>::iter_prefix((subnet_id, epoch))
            .next()
            .is_none()
}

#[test]
fn election_weights_without_a_proposal_are_retained_until_settlement_then_pruned() {
    new_test_ext().execute_with(|| {
        let name = b"unproposed-weight-history".to_vec();
        build_activated_subnet(
            name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(name).unwrap();
        seed_equal_validator_delegate_stake_for_subnet(subnet_id);
        let epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        Network::elect_validator(subnet_id, epoch);
        let weights = SubnetConsensusAttestorWeights::<Test>::get(subnet_id, epoch).unwrap();
        assert_eq!(weights.total_weight, 4);
        assert!(!crate::SubnetConsensusSubmission::<Test>::contains_key(
            subnet_id, epoch
        ));

        let prune_epoch = epoch + CONSENSUS_HISTORY_RETENTION_EPOCHS + 1;
        ConsensusHistoryPruneCursor::<Test>::insert(subnet_id, epoch);
        Network::prune_consensus_history(&mut WeightMeter::new(), subnet_id, epoch + 1);
        assert_eq!(
            SubnetConsensusAttestorWeights::<Test>::get(subnet_id, epoch),
            Some(weights.clone())
        );

        set_block_to_subnet_slot_epoch(epoch + 1, subnet_id);
        let (submission, _) = Network::precheck_subnet_consensus_submission(subnet_id, epoch);
        assert!(submission.is_none());
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet_id, epoch),
            Some(true)
        );
        assert_eq!(
            SubnetConsensusAttestorWeights::<Test>::get(subnet_id, epoch),
            Some(weights)
        );

        Network::prune_consensus_history(&mut WeightMeter::new(), subnet_id, prune_epoch);
        assert!(round_fully_absent(subnet_id, epoch));
    });
}

#[test]
fn missing_proposal_with_invalid_election_weights_cancels_once() {
    for corruption in 0..3 {
        new_test_ext().execute_with(|| {
            let name = b"invalid-unproposed-weights".to_vec();
            let pool_balance = 1_000_000;
            build_activated_subnet(
                name.clone(),
                0,
                4,
                10_000_000_000_000_000_000_000,
                MinSubnetMinStake::<Test>::get(),
            );
            let subnet_id = SubnetName::<Test>::get(name).unwrap();
            for node_id in 1..=4 {
                let validator_id =
                    crate::SubnetNodeValidatorId::<Test>::get(subnet_id, node_id).unwrap();
                Network::handle_increase_account_validator_delegate_stake(
                    &account(9_200),
                    validator_id,
                    pool_balance,
                )
                .unwrap();
            }
            seed_equal_validator_delegate_stake_for_subnet(subnet_id);
            let epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
            Network::elect_validator(subnet_id, epoch);
            let round = SubnetElectedValidator::<Test>::get(subnet_id, epoch).unwrap();
            let mut weights =
                SubnetConsensusAttestorWeights::<Test>::get(subnet_id, epoch).unwrap();
            match corruption {
                0 => SubnetConsensusAttestorWeights::<Test>::remove(subnet_id, epoch),
                1 => {
                    weights.total_weight += 1;
                    SubnetConsensusAttestorWeights::<Test>::insert(subnet_id, epoch, weights);
                }
                _ => {
                    weights.weights.remove(&round.validator_subnet_node_id);
                    SubnetConsensusAttestorWeights::<Test>::insert(subnet_id, epoch, weights);
                }
            }
            let node_stake =
                crate::NodeSubnetStake::<Test>::get(round.validator_subnet_node_id, subnet_id);
            let reputation =
                crate::SubnetNodeReputation::<Test>::get(subnet_id, round.validator_subnet_node_id);
            let subnet_reputation = crate::SubnetReputation::<Test>::get(subnet_id);
            let events = System::events();
            set_block_to_subnet_slot_epoch(epoch + 1, subnet_id);

            let (submission, _) = Network::precheck_subnet_consensus_submission(subnet_id, epoch);
            assert!(submission.is_none());
            assert_eq!(
                crate::NodeSubnetStake::<Test>::get(round.validator_subnet_node_id, subnet_id),
                node_stake
            );
            assert_eq!(
                crate::ValidatorDelegateStakeBalance::<Test>::get(round.validator_id),
                pool_balance
            );
            assert_eq!(
                crate::SubnetNodeReputation::<Test>::get(subnet_id, round.validator_subnet_node_id),
                reputation
            );
            assert_eq!(
                crate::SubnetReputation::<Test>::get(subnet_id),
                subnet_reputation
            );
            assert_eq!(System::events().len(), events.len() + 1);
            System::assert_last_event(
                crate::Event::ConsensusRoundCancelled {
                    subnet_id,
                    subnet_epoch: epoch,
                }
                .into(),
            );
            assert_eq!(
                ConsensusRoundSettlementStatus::<Test>::get(subnet_id, epoch),
                Some(true)
            );

            Network::elect_validator(subnet_id, epoch + 1);
            assert!(SubnetElectedValidator::<Test>::contains_key(
                subnet_id,
                epoch + 1
            ));
        });
    }
}

#[test]
fn consensus_history_is_pruned_to_the_retention_window_in_bounded_steps() {
    new_test_ext().execute_with(|| {
        let subnet_id = 7;
        let current_subnet_epoch = 40;
        for epoch in 0..=current_subnet_epoch {
            insert_round(subnet_id, epoch);
        }

        // The first slot only initializes the cursor, at the oldest epoch that can hold history
        // (no subnet record here, so epoch 0), which reaches history written before pruning
        // existed.
        let horizon = current_subnet_epoch - CONSENSUS_HISTORY_RETENTION_EPOCHS;
        Network::prune_consensus_history(&mut WeightMeter::new(), subnet_id, current_subnet_epoch);
        assert_eq!(ConsensusHistoryPruneCursor::<Test>::get(subnet_id), Some(0));
        assert!(round_fully_present(subnet_id, 0));

        // One slot prunes at most MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT rounds.
        Network::prune_consensus_history(&mut WeightMeter::new(), subnet_id, current_subnet_epoch);
        assert_eq!(
            ConsensusHistoryPruneCursor::<Test>::get(subnet_id),
            Some(MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT)
        );
        assert!(round_fully_absent(
            subnet_id,
            MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT - 1
        ));
        assert!(round_fully_present(
            subnet_id,
            MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT
        ));

        for _ in 0..horizon {
            Network::prune_consensus_history(
                &mut WeightMeter::new(),
                subnet_id,
                current_subnet_epoch,
            );
        }

        for epoch in 0..horizon {
            assert!(round_fully_absent(subnet_id, epoch), "epoch {epoch} kept");
        }
        for epoch in horizon..=current_subnet_epoch {
            assert!(
                round_fully_present(subnet_id, epoch),
                "epoch {epoch} pruned"
            );
        }
        assert_eq!(
            SubnetElectedValidator::<Test>::iter_prefix(subnet_id).count() as u32,
            CONSENSUS_HISTORY_RETENTION_EPOCHS + 1
        );
    });
}

#[test]
fn consensus_history_pruning_expires_unfinished_rounds() {
    new_test_ext().execute_with(|| {
        let subnet_id = 7;
        let current_subnet_epoch = 40;
        for epoch in 0..=current_subnet_epoch {
            insert_round(subnet_id, epoch);
        }
        let pending_epoch = 5;
        ConsensusRoundSettlementStatus::<Test>::insert(subnet_id, pending_epoch, false);

        ConsensusHistoryPruneCursor::<Test>::insert(subnet_id, 0);

        for _ in 0..20 {
            Network::prune_consensus_history(
                &mut WeightMeter::new(),
                subnet_id,
                current_subnet_epoch,
            );
        }

        let horizon = current_subnet_epoch - CONSENSUS_HISTORY_RETENTION_EPOCHS;
        for epoch in 0..horizon {
            assert!(round_fully_absent(subnet_id, epoch));
        }
        assert!(round_fully_absent(subnet_id, pending_epoch));
        assert!(round_fully_present(subnet_id, horizon));
        assert_eq!(
            ConsensusHistoryPruneCursor::<Test>::get(subnet_id),
            Some(horizon)
        );
    });
}

#[test]
fn subnet_removal_clears_all_consensus_history_with_idle_weight() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();

        for epoch in 0..10 {
            insert_round(subnet_id, epoch + 1_000);
        }
        // An unrelated subnet's history must survive.
        insert_round(subnet_id + 1, 1_000);

        Network::do_remove_subnet(subnet_id, SubnetRemovalReason::Council);
        assert_eq!(ConsensusHistoryCleanupQueue::<Test>::get(), vec![subnet_id]);
        assert!(ConsensusHistoryPruneCursor::<Test>::get(subnet_id).is_none());

        // (The mock's DbWeight is zero, so weight metering itself is not exercised here.)
        for _ in 0..5 {
            Network::on_idle(System::block_number(), Weight::MAX);
        }

        assert!(ConsensusHistoryCleanupQueue::<Test>::get().is_empty());
        assert!(SubnetElectedValidator::<Test>::iter_prefix(subnet_id)
            .next()
            .is_none());
        assert!(
            ConsensusRoundSettlementStatus::<Test>::iter_prefix(subnet_id)
                .next()
                .is_none()
        );
        assert!(
            SubnetConsensusSubmissionMaxItems::<Test>::iter_prefix(subnet_id)
                .next()
                .is_none()
        );
        assert!(SubnetConsensusProposalArgs::<Test>::iter_prefix(subnet_id)
            .next()
            .is_none());
        assert!(
            SubnetConsensusAttestorWeights::<Test>::iter_prefix(subnet_id)
                .next()
                .is_none()
        );
        assert!(
            SubnetConsensusAttestationData::<Test>::iter_prefix((subnet_id,))
                .next()
                .is_none()
        );
        assert!(round_fully_present(subnet_id + 1, 1_000));
    });
}

#[test]
fn prune_cursor_starts_at_the_oldest_possible_epoch_and_bounds_empty_epoch_visits() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();

        ConsensusHistoryPruneCursor::<Test>::remove(subnet_id);

        let eligible_from = 100u32;
        crate::SubnetsData::<Test>::mutate(subnet_id, |subnet| {
            subnet
                .as_mut()
                .unwrap()
                .consensus_eligible_from_subnet_epoch = Some(eligible_from);
        });

        // The subnet's first rounds, a long gap without elections, then recent rounds.
        let current_subnet_epoch = 1_000u32;
        let old_rounds = eligible_from..eligible_from + 3;
        let recent_rounds = current_subnet_epoch - 20..=current_subnet_epoch;
        for epoch in old_rounds.clone().chain(recent_rounds.clone()) {
            insert_round(subnet_id, epoch);
        }

        Network::prune_consensus_history(&mut WeightMeter::new(), subnet_id, current_subnet_epoch);
        assert_eq!(
            ConsensusHistoryPruneCursor::<Test>::get(subnet_id),
            Some(eligible_from)
        );

        // The first slot visits at most the configured number of epochs, including empty ones.
        Network::prune_consensus_history(&mut WeightMeter::new(), subnet_id, current_subnet_epoch);
        for epoch in old_rounds.clone() {
            assert!(round_fully_absent(subnet_id, epoch));
        }
        assert_eq!(
            ConsensusHistoryPruneCursor::<Test>::get(subnet_id),
            Some(eligible_from + MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT)
        );
        assert!(round_fully_present(subnet_id, *recent_rounds.start()));

        // Later slots traverse the gap in bounded steps and eventually reach the horizon.
        let horizon = current_subnet_epoch - CONSENSUS_HISTORY_RETENTION_EPOCHS;
        for _ in 0..(horizon - eligible_from).div_ceil(MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT) {
            let before = ConsensusHistoryPruneCursor::<Test>::get(subnet_id).unwrap();
            Network::prune_consensus_history(
                &mut WeightMeter::new(),
                subnet_id,
                current_subnet_epoch,
            );
            let after = ConsensusHistoryPruneCursor::<Test>::get(subnet_id).unwrap();
            assert_eq!(
                after,
                before
                    .saturating_add(MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT)
                    .min(horizon)
            );
        }
        assert_eq!(
            ConsensusHistoryPruneCursor::<Test>::get(subnet_id),
            Some(horizon)
        );
        for epoch in recent_rounds {
            if epoch < horizon {
                assert!(round_fully_absent(subnet_id, epoch));
            } else {
                assert!(round_fully_present(subnet_id, epoch));
            }
        }
    });
}
