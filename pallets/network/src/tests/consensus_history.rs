use super::mock::*;
use super::test_utils::*;
use crate::utilities::consensus_history::{
    CONSENSUS_HISTORY_RETENTION_EPOCHS, MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT,
};
use crate::{
    ConsensusAttestorWeightSnapshot, ConsensusHistoryCleanupQueue, ConsensusHistoryPruneCursor,
    ConsensusRoundSettlementStatus, ElectedConsensusRound, MinSubnetMinStake,
    PendingConsensusRoundSettlementEpoch, SubnetConsensusAttestationData,
    SubnetConsensusAttestorWeights, SubnetConsensusProposalArgs, SubnetConsensusSubmissionMaxItems,
    SubnetElectedValidator, SubnetName, SubnetRemovalReason, ValidatorArgs,
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
fn consensus_history_pruning_never_passes_the_pending_round() {
    new_test_ext().execute_with(|| {
        let subnet_id = 7;
        let current_subnet_epoch = 40;
        for epoch in 0..=current_subnet_epoch {
            insert_round(subnet_id, epoch);
        }
        let pending_epoch = 5;
        PendingConsensusRoundSettlementEpoch::<Test>::insert(subnet_id, pending_epoch);
        ConsensusHistoryPruneCursor::<Test>::insert(subnet_id, 0);

        for _ in 0..20 {
            Network::prune_consensus_history(
                &mut WeightMeter::new(),
                subnet_id,
                current_subnet_epoch,
            );
        }

        for epoch in 0..pending_epoch {
            assert!(round_fully_absent(subnet_id, epoch));
        }
        assert!(round_fully_present(subnet_id, pending_epoch));
        assert_eq!(
            ConsensusHistoryPruneCursor::<Test>::get(subnet_id),
            Some(pending_epoch)
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
        PendingConsensusRoundSettlementEpoch::<Test>::remove(subnet_id);

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
fn prune_cursor_starts_at_the_oldest_possible_epoch_and_skips_empty_epochs_cheaply() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        PendingConsensusRoundSettlementEpoch::<Test>::remove(subnet_id);
        ConsensusHistoryPruneCursor::<Test>::remove(subnet_id);

        let eligible_from = 100u32;
        crate::SubnetsData::<Test>::mutate(subnet_id, |subnet| {
            subnet
                .as_mut()
                .unwrap()
                .consensus_eligible_from_subnet_epoch = Some(eligible_from);
        });

        // Pre-upgrade history from the subnet's first rounds, a long gap, then recent rounds.
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

        // The first slot prunes the old rounds, skips the empty gap for one read per epoch and
        // stops after MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT pruned rounds ...
        Network::prune_consensus_history(&mut WeightMeter::new(), subnet_id, current_subnet_epoch);
        for epoch in old_rounds.clone() {
            assert!(round_fully_absent(subnet_id, epoch));
        }
        let pruned_recent = MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT - old_rounds.len() as u32;
        assert_eq!(
            ConsensusHistoryPruneCursor::<Test>::get(subnet_id),
            Some(*recent_rounds.start() + pruned_recent)
        );

        // ... and later slots walk the empty gap up to the retention horizon.
        for _ in 0..50 {
            Network::prune_consensus_history(
                &mut WeightMeter::new(),
                subnet_id,
                current_subnet_epoch,
            );
        }
        let horizon = current_subnet_epoch - CONSENSUS_HISTORY_RETENTION_EPOCHS;
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
