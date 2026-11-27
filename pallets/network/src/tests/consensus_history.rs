use super::mock::*;
use super::test_utils::*;
use crate::utilities::consensus_history::{
    CONSENSUS_HISTORY_RETENTION_EPOCHS, MAX_PRUNED_CONSENSUS_EPOCHS_PER_IDLE,
};
use crate::{
    ConsensusAttestorWeightSnapshot, ConsensusHistoryPruneCursor, ConsensusRoundSettlementStatus,
    ElectedConsensusRound, MinSubnetMinStake, RemovedSubnetCleanupQueue,
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
        let current_subnet_epoch = CONSENSUS_HISTORY_RETENTION_EPOCHS + 24;
        for epoch in 0..=current_subnet_epoch {
            insert_round(subnet_id, epoch);
        }

        // The first idle pass only initializes the cursor, at the oldest epoch that can hold history
        // (no subnet record here, so epoch 0).
        let horizon = current_subnet_epoch - CONSENSUS_HISTORY_RETENTION_EPOCHS;
        Network::prune_consensus_history(&mut WeightMeter::new(), subnet_id, current_subnet_epoch);
        assert_eq!(ConsensusHistoryPruneCursor::<Test>::get(subnet_id), Some(0));
        assert!(round_fully_present(subnet_id, 0));

        // One idle pass prunes at most MAX_PRUNED_CONSENSUS_EPOCHS_PER_IDLE rounds.
        Network::prune_consensus_history(&mut WeightMeter::new(), subnet_id, current_subnet_epoch);
        assert_eq!(
            ConsensusHistoryPruneCursor::<Test>::get(subnet_id),
            Some(MAX_PRUNED_CONSENSUS_EPOCHS_PER_IDLE)
        );
        assert!(round_fully_absent(
            subnet_id,
            MAX_PRUNED_CONSENSUS_EPOCHS_PER_IDLE - 1
        ));
        assert!(round_fully_present(
            subnet_id,
            MAX_PRUNED_CONSENSUS_EPOCHS_PER_IDLE
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
        let current_subnet_epoch = CONSENSUS_HISTORY_RETENTION_EPOCHS + 24;
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
        assert_eq!(
            RemovedSubnetCleanupQueue::<Test>::get(subnet_id),
            Some(crate::RemovedSubnetCleanupStage::ChurnLimit),
        );
        assert!(ConsensusHistoryPruneCursor::<Test>::get(subnet_id).is_none());

        // (The mock's DbWeight is zero, so weight metering itself is not exercised here.)
        for _ in 0..5 {
            Network::on_idle(System::block_number(), Weight::MAX);
        }

        assert!(RemovedSubnetCleanupQueue::<Test>::iter_keys()
            .next()
            .is_none());
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

        // The first idle pass visits at most the configured number of epochs, including empty ones.
        Network::prune_consensus_history(&mut WeightMeter::new(), subnet_id, current_subnet_epoch);
        for epoch in old_rounds.clone() {
            assert!(round_fully_absent(subnet_id, epoch));
        }
        assert_eq!(
            ConsensusHistoryPruneCursor::<Test>::get(subnet_id),
            Some(eligible_from + MAX_PRUNED_CONSENSUS_EPOCHS_PER_IDLE)
        );
        assert!(round_fully_present(subnet_id, *recent_rounds.start()));

        // Later idle passes traverse the gap in bounded steps and eventually reach the horizon.
        let horizon = current_subnet_epoch - CONSENSUS_HISTORY_RETENTION_EPOCHS;
        for _ in 0..(horizon - eligible_from).div_ceil(MAX_PRUNED_CONSENSUS_EPOCHS_PER_IDLE) {
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
                    .saturating_add(MAX_PRUNED_CONSENSUS_EPOCHS_PER_IDLE)
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

#[test]
fn initialization_preserves_old_history_and_idle_prunes_only_expired_rounds() {
    new_test_ext().execute_with(|| {
        let name = b"idle-history-retention".to_vec();
        build_activated_subnet(
            name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet = SubnetName::<Test>::get(name).unwrap();
        let epoch = CONSENSUS_HISTORY_RETENTION_EPOCHS + 24;
        let horizon = epoch - CONSENSUS_HISTORY_RETENTION_EPOCHS;
        insert_round(subnet, 1);
        insert_round(subnet, horizon);
        ConsensusHistoryPruneCursor::<Test>::insert(subnet, 1);
        set_block_to_subnet_slot_epoch(epoch, subnet);

        Network::on_initialize(System::block_number());
        assert!(round_fully_present(subnet, 1));
        assert_eq!(ConsensusHistoryPruneCursor::<Test>::get(subnet), Some(1));
        let current_round = SubnetElectedValidator::<Test>::get(subnet, epoch);
        assert!(current_round.is_some());

        frame_support::assert_storage_noop!({
            assert_eq!(
                Network::on_idle(System::block_number(), Weight::zero()),
                Weight::zero()
            );
        });
        Network::on_idle(System::block_number(), Weight::MAX);
        assert!(round_fully_absent(subnet, 1));
        assert!(round_fully_present(subnet, horizon));
        assert_eq!(
            SubnetElectedValidator::<Test>::get(subnet, epoch),
            current_round
        );
    });
}

#[test]
fn idle_pruning_rotates_subnets_and_uses_each_subnets_slot_epoch() {
    new_test_ext().execute_with(|| {
        for (slot, subnet) in [(4, 21), (5, 22)] {
            crate::SlotAssignment::<Test>::insert(slot, subnet);
            crate::SubnetSlot::<Test>::insert(subnet, slot);
            ConsensusHistoryPruneCursor::<Test>::insert(subnet, 0);
            insert_round(subnet, 0);
        }
        let block = (CONSENSUS_HISTORY_RETENTION_EPOCHS + 1) * NetworkEpochLength::get() + 4;
        System::set_block_number(block);
        for _ in 0..2 {
            Network::on_idle(System::block_number(), Weight::MAX);
        }
        assert!(round_fully_absent(21, 0));
        assert!(round_fully_present(22, 0));

        System::set_block_number(block + 1);
        for _ in 0..2 {
            Network::on_idle(System::block_number(), Weight::MAX);
        }
        assert!(round_fully_absent(22, 0));

        // Removing the last visited slot does not pin the iterator or leave a stale live job.
        crate::SlotAssignment::<Test>::remove(5);
        Network::on_idle(System::block_number(), Weight::MAX);
        assert_eq!(crate::ConsensusHistoryPruneLastSlot::<Test>::get(), Some(4));
    });
}

#[test]
fn removed_history_deletes_a_bounded_batch_and_rotates_large_backlogs() {
    use crate::utilities::subnet_cleanup::MAX_REMOVED_SUBNET_STEPS_PER_IDLE;
    for commit in [false, true] {
        let mut ext = new_test_ext();
        let entries = MAX_REMOVED_SUBNET_STEPS_PER_IDLE + 10;
        ext.execute_with(|| {
            RemovedSubnetCleanupQueue::<Test>::insert(
                41,
                crate::RemovedSubnetCleanupStage::Attestations,
            );
            RemovedSubnetCleanupQueue::<Test>::insert(
                42,
                crate::RemovedSubnetCleanupStage::Attestations,
            );
            for node in 0..entries {
                SubnetConsensusAttestationData::<Test>::insert((41, 0, node), payload(1));
            }
            insert_round(42, 0);
            insert_round(43, 0); // Unscheduled history must survive.
        });
        if commit {
            ext.commit_all().unwrap();
        }
        ext.execute_with(|| {
            Network::cleanup_removed_subnets(&mut WeightMeter::new());
            assert_eq!(
                SubnetConsensusAttestationData::<Test>::iter_key_prefix((41,)).count(),
                (entries - MAX_REMOVED_SUBNET_STEPS_PER_IDLE) as usize,
            );
            assert!(RemovedSubnetCleanupQueue::<Test>::contains_key(41));

            // A large first subnet cannot monopolize subsequent idle passes.
            Network::cleanup_removed_subnets(&mut WeightMeter::new());
            assert!(round_fully_absent(42, 0));
            assert!(!RemovedSubnetCleanupQueue::<Test>::contains_key(42));
            assert!(RemovedSubnetCleanupQueue::<Test>::contains_key(41));
            Network::cleanup_removed_subnets(&mut WeightMeter::new());
            assert!(round_fully_absent(41, 0));
            assert!(RemovedSubnetCleanupQueue::<Test>::iter_keys()
                .next()
                .is_none());
            assert!(round_fully_present(43, 0));
        });
    }
}

#[test]
fn removed_history_respects_both_weight_limits_and_resumes_partial_prefix_cleanup() {
    use crate::RemovedSubnetCleanupStage;
    use frame_support::weights::RuntimeDbWeight;
    struct RestoreDbWeight(RuntimeDbWeight);
    impl Drop for RestoreDbWeight {
        fn drop(&mut self) {
            TestDbWeight::set(self.0);
        }
    }
    new_test_ext().execute_with(|| {
        let _restore = RestoreDbWeight(TestDbWeight::get());
        TestDbWeight::set(RuntimeDbWeight { read: 1, write: 2 });
        RemovedSubnetCleanupQueue::<Test>::insert(
            41,
            crate::RemovedSubnetCleanupStage::Attestations,
        );
        SubnetConsensusAttestationData::<Test>::insert((41, 0, 1), payload(1));
        let selector = <() as crate::weights::WeightInfo>::removed_subnet_cleanup_selector();
        let step =
            Network::removed_subnet_cleanup_step_weight(RemovedSubnetCleanupStage::Attestations);
        let progress = <() as crate::weights::WeightInfo>::removed_subnet_cleanup_progress();
        let exact = selector.saturating_add(step).saturating_add(progress);
        for limit in [
            Weight::zero(),
            Weight::from_parts(u64::MAX, 0),
            exact.saturating_sub(Weight::from_parts(1, 0)),
            exact.saturating_sub(Weight::from_parts(0, 1)),
        ] {
            let mut meter = WeightMeter::with_limit(limit);
            Network::cleanup_removed_subnets(&mut meter);
            assert!(meter.consumed().all_lte(limit));
            assert!(SubnetConsensusAttestationData::<Test>::contains_key((
                41, 0, 1
            )));
            assert_eq!(
                RemovedSubnetCleanupQueue::<Test>::get(41),
                Some(RemovedSubnetCleanupStage::Attestations)
            );
        }

        let mut meter = WeightMeter::with_limit(exact);
        Network::cleanup_removed_subnets(&mut meter);
        assert!(!SubnetConsensusAttestationData::<Test>::contains_key((
            41, 0, 1
        )));
        assert!(meter.consumed().all_lte(exact));

        // The next small allowance advances just the prefix stage and saves it for later.
        let mut meter = WeightMeter::with_limit(exact);
        Network::cleanup_removed_subnets(&mut meter);
        assert_eq!(meter.consumed(), exact);
        assert_eq!(
            RemovedSubnetCleanupQueue::<Test>::get(41),
            Some(RemovedSubnetCleanupStage::Submissions)
        );
        Network::cleanup_removed_subnets(&mut WeightMeter::new());
        assert!(!RemovedSubnetCleanupQueue::<Test>::contains_key(41));
    });
}
