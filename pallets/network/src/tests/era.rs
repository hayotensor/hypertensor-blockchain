use super::mock::*;
use crate::tests::test_utils::{
    insert_overwatch_node_v2, insert_subnet, make_commit, manual_insert_validator,
    queue_overwatch_settlement, set_overwatch_epoch, set_overwatch_node_stake, test_percent,
};
use crate::{
    ActiveOverwatchCommitCutoffPercent, ActiveOverwatchEpochLengthMultiplier,
    ActiveOverwatchRevealStats, BaseSlashPercentage,
    ConsensusValidatorIdentityAttestationPercentage, CurrentOverwatchEpoch,
    InConsensusSubnetReputationFactor, LastFinalizedOverwatchEpoch, LatestEffectiveOverwatchSignal,
    LatestFinalizedOverwatchSignalInputs, NotInConsensusSubnetReputationFactor, OverwatchCommit,
    OverwatchCommitCutoffPercent, OverwatchCommits, OverwatchEpochLengthMultiplier,
    OverwatchEpochSettlementSnapshots, OverwatchEpochStartBlock, OverwatchNodeWeights,
    OverwatchReveal, OverwatchReveals, OverwatchSubnetWeights, PendingOverwatchSettlement,
    SubnetElectedValidator, SubnetNodeElectionSlots,
    SubnetNodeMinWeightDecreaseReputationThreshold, SubnetNodeValidatorId,
    SubnetReputationFactorSchedules, SubnetReputationFactors, SubnetSlot, SubnetState,
    NETWORK_OVERWATCH_SETTLEMENT_SLOT, NETWORK_SUBNET_EMISSION_SLOT,
};
use frame_support::{assert_ok, traits::OnInitialize};

#[test]
fn test_get_current_subnet_epoch_as_u32() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let slot = 5;
        let epoch_length = EpochLength::get();

        SubnetSlot::<Test>::insert(subnet_id, slot);
        let current_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // Epoch 0
        System::set_block_number(slot);
        assert_eq!(Network::get_current_subnet_epoch_as_u32(subnet_id), 0);

        System::set_block_number(epoch_length + slot - 1);
        assert_eq!(Network::get_current_subnet_epoch_as_u32(subnet_id), 0);

        // Epoch 1
        System::set_block_number(epoch_length + slot);
        assert_eq!(Network::get_current_subnet_epoch_as_u32(subnet_id), 1);

        log::error!("subnet epoch {:?}", epoch_length * 2 + slot - 1);
        log::error!("subnet epoch {:?}", epoch_length + slot + epoch_length - 1);

        System::set_block_number(epoch_length * 2 + slot - 1);
        assert_eq!(Network::get_current_subnet_epoch_as_u32(subnet_id), 1);

        // Epoch 2
        System::set_block_number(epoch_length * 2 + slot);
        assert_eq!(Network::get_current_subnet_epoch_as_u32(subnet_id), 2);

        System::set_block_number(epoch_length * 3 + slot - 1);
        assert_eq!(Network::get_current_subnet_epoch_as_u32(subnet_id), 2);

        // Epoch 3
        System::set_block_number(epoch_length * 3 + slot);
        assert_eq!(Network::get_current_subnet_epoch_as_u32(subnet_id), 3);

        System::set_block_number(epoch_length * 4 + slot - 1);
        assert_eq!(Network::get_current_subnet_epoch_as_u32(subnet_id), 3);
    })
}

#[test]
fn can_propose_or_attest_saturates_next_epoch_boundary_at_u32_max() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        SubnetSlot::<Test>::insert(subnet_id, DesignatedEpochSlots::get());

        System::set_block_number(u32::MAX - 1);
        assert!(Network::can_propose_or_attest_attestation(subnet_id));

        System::set_block_number(u32::MAX);
        assert!(!Network::can_propose_or_attest_attestation(subnet_id));
    });
}

#[test]
fn validator_election_snapshots_policy_against_later_governance_changes() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let subnet_node_id = 10;
        let validator_id = 20;
        let subnet_epoch = 3;
        let elected_slash_percentage = Network::percent_div(1, 100);
        let elected_supermajority = <Test as crate::Config>::SuperMajorityAttestationRatio::get();
        let elected_reputation_factors = SubnetReputationFactors {
            absent_decrease: Network::percent_div(1, 100),
            included_increase: Network::percent_div(2, 100),
            below_min_weight_decrease: Network::percent_div(3, 100),
            non_attestor_decrease: Network::percent_div(4, 100),
            non_consensus_attestor_decrease: Network::percent_div(5, 100),
            validator_absent_decrease: Network::percent_div(6, 100),
            validator_non_consensus_decrease: Network::percent_div(7, 100),
        };
        let elected_subnet_reputation_increase = Network::percent_div(10, 100);
        let elected_subnet_reputation_decrease = Network::percent_div(11, 100);
        let elected_min_weight_threshold = Network::percent_div(12, 100);

        SubnetNodeElectionSlots::<Test>::insert(subnet_id, vec![subnet_node_id]);
        SubnetNodeValidatorId::<Test>::insert(subnet_id, subnet_node_id, validator_id);
        BaseSlashPercentage::<Test>::put(elected_slash_percentage);
        SubnetReputationFactorSchedules::<Test>::mutate(subnet_id, |schedule| {
            schedule.current = elected_reputation_factors;
        });
        InConsensusSubnetReputationFactor::<Test>::put(elected_subnet_reputation_increase);
        NotInConsensusSubnetReputationFactor::<Test>::put(elected_subnet_reputation_decrease);
        SubnetNodeMinWeightDecreaseReputationThreshold::<Test>::insert(
            subnet_id,
            elected_min_weight_threshold,
        );

        Network::elect_validator(subnet_id, subnet_epoch, 0);
        BaseSlashPercentage::<Test>::put(Network::percent_div(9, 100));
        SubnetReputationFactorSchedules::<Test>::mutate(subnet_id, |schedule| {
            schedule.current = SubnetReputationFactors {
                absent_decrease: Network::percent_div(21, 100),
                included_increase: Network::percent_div(22, 100),
                below_min_weight_decrease: Network::percent_div(23, 100),
                non_attestor_decrease: Network::percent_div(24, 100),
                non_consensus_attestor_decrease: Network::percent_div(25, 100),
                validator_absent_decrease: Network::percent_div(26, 100),
                validator_non_consensus_decrease: Network::percent_div(27, 100),
            };
        });
        InConsensusSubnetReputationFactor::<Test>::put(Network::percent_div(30, 100));
        NotInConsensusSubnetReputationFactor::<Test>::put(Network::percent_div(31, 100));
        SubnetNodeMinWeightDecreaseReputationThreshold::<Test>::insert(
            subnet_id,
            Network::percent_div(32, 100),
        );

        let round = SubnetElectedValidator::<Test>::get(subnet_id, subnet_epoch).unwrap();
        assert_eq!(round.validator_subnet_node_id, subnet_node_id);
        assert_eq!(round.policy.base_slash_percentage, elected_slash_percentage);
        assert_eq!(
            round.policy.super_majority_attestation_ratio,
            elected_supermajority
        );
        assert_eq!(round.policy.reputation_factors, elected_reputation_factors);
        assert_eq!(
            round.policy.in_consensus_subnet_reputation_factor,
            elected_subnet_reputation_increase
        );
        assert_eq!(
            round.policy.not_in_consensus_subnet_reputation_factor,
            elected_subnet_reputation_decrease
        );
        assert_eq!(
            round.policy.min_weight_decrease_reputation_threshold,
            elected_min_weight_threshold
        );
    });
}

#[test]
fn validator_election_snapshots_collective_identity_attestation_percentage() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let subnet_node_id = 10;
        let validator_id = 20;
        let old_value = Network::percent_div(1, 2);
        let pending_value = Network::percent_div(3, 4);

        SubnetNodeElectionSlots::<Test>::insert(subnet_id, vec![subnet_node_id]);
        SubnetNodeValidatorId::<Test>::insert(subnet_id, subnet_node_id, validator_id);
        ConsensusValidatorIdentityAttestationPercentage::<Test>::put(old_value);

        Network::elect_validator(subnet_id, 3, 0);
        ConsensusValidatorIdentityAttestationPercentage::<Test>::put(pending_value);
        Network::elect_validator(subnet_id, 4, EpochLength::get());

        assert_eq!(
            SubnetElectedValidator::<Test>::get(subnet_id, 3)
                .unwrap()
                .policy
                .validator_identity_attestation_percentage,
            old_value
        );
        assert_eq!(
            SubnetElectedValidator::<Test>::get(subnet_id, 4)
                .unwrap()
                .policy
                .validator_identity_attestation_percentage,
            pending_value
        );
    });
}

#[test]
fn test_collective_overwatch_config_is_snapshotted_at_next_epoch() {
    new_test_ext().execute_with(|| {
        let old_multiplier = OverwatchEpochLengthMultiplier::<Test>::get();
        let next_multiplier = 2;
        let next_cutoff = Network::percent_div(1, 10);
        let old_epoch_length = EpochLength::get().saturating_mul(old_multiplier);

        // At this block a 10% cutoff would already be in reveal, while the active epoch's original
        // 80% cutoff is still in commit. Configuration changes must not alter the live round.
        System::set_block_number(old_epoch_length / 5);
        assert!(Network::in_overwatch_commit_period());
        assert_ok!(Network::do_set_overwatch_epoch_length_multiplier(
            next_multiplier
        ));
        assert_ok!(Network::do_set_overwatch_commit_cutoff_percent(next_cutoff));

        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 0);
        assert_eq!(
            OverwatchEpochLengthMultiplier::<Test>::get(),
            next_multiplier
        );
        assert_eq!(OverwatchCommitCutoffPercent::<Test>::get(), next_cutoff);
        assert!(Network::in_overwatch_commit_period());
        assert_eq!(
            ActiveOverwatchEpochLengthMultiplier::<Test>::get(),
            old_multiplier
        );

        System::set_block_number(old_epoch_length);
        Network::advance_overwatch_epoch(old_epoch_length);

        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 1);
        assert_eq!(OverwatchEpochStartBlock::<Test>::get(), old_epoch_length);
        assert_eq!(
            OverwatchEpochLengthMultiplier::<Test>::get(),
            next_multiplier
        );
        assert_eq!(OverwatchCommitCutoffPercent::<Test>::get(), next_cutoff);
        assert_eq!(
            ActiveOverwatchEpochLengthMultiplier::<Test>::get(),
            next_multiplier
        );
        assert_eq!(
            ActiveOverwatchCommitCutoffPercent::<Test>::get(),
            next_cutoff
        );
        let settlement = PendingOverwatchSettlement::<Test>::get().unwrap();
        assert_eq!(settlement.epoch, 0);
        assert_eq!(
            network_events().last(),
            Some(&crate::Event::OverwatchEpochStarted {
                epoch: 1,
                start_block: old_epoch_length,
                epoch_length_multiplier: next_multiplier,
                commit_cutoff_percent: next_cutoff,
            })
        );

        // Empty rounds still finalize once, allowing the next epoch to close normally.
        Network::calculate_overwatch_rewards();
        assert_eq!(LastFinalizedOverwatchEpoch::<Test>::get(), Some(0));
        assert!(PendingOverwatchSettlement::<Test>::get().is_none());
        assert_eq!(
            network_events().last(),
            Some(&crate::Event::OverwatchEpochFinalized {
                epoch: 0,
                node_rewards: Vec::new(),
            })
        );

        let next_epoch_length = EpochLength::get().saturating_mul(next_multiplier);
        System::set_block_number(old_epoch_length + next_epoch_length - 1);
        Network::advance_overwatch_epoch(System::block_number());
        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 1);

        System::set_block_number(old_epoch_length + next_epoch_length);
        Network::advance_overwatch_epoch(System::block_number());
        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 2);
        let settlement = PendingOverwatchSettlement::<Test>::get().unwrap();
        assert_eq!(settlement.epoch, 1);
    });
}

#[test]
fn test_overwatch_commit_cutoff_uses_anchored_start_block() {
    new_test_ext().execute_with(|| {
        OverwatchEpochLengthMultiplier::<Test>::put(2);
        OverwatchCommitCutoffPercent::<Test>::put(Network::percent_div(4, 5));
        ActiveOverwatchEpochLengthMultiplier::<Test>::put(2);
        ActiveOverwatchCommitCutoffPercent::<Test>::put(Network::percent_div(4, 5));
        CurrentOverwatchEpoch::<Test>::put(7);
        OverwatchEpochStartBlock::<Test>::put(1_000);

        let cutoff_blocks = Network::percent_mul(
            EpochLength::get().saturating_mul(2) as u128,
            Network::percent_div(4, 5),
        ) as u32;

        System::set_block_number(1_000 + cutoff_blocks - 1);
        assert!(Network::in_overwatch_commit_period());
        assert_eq!(Network::get_current_overwatch_epoch_as_u32(), 7);

        System::set_block_number(1_000 + cutoff_blocks);
        assert!(!Network::in_overwatch_commit_period());
        assert_eq!(Network::get_current_overwatch_epoch_as_u32(), 7);
    });
}

#[test]
fn test_delayed_overwatch_boundary_realigns_before_settlement() {
    new_test_ext().execute_with(|| {
        let epoch_length = EpochLength::get();
        let epoch_start = 1;
        let unaligned_end = epoch_start + epoch_length;
        let aligned_boundary = epoch_length * 2;

        OverwatchEpochLengthMultiplier::<Test>::put(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::put(1);
        CurrentOverwatchEpoch::<Test>::put(3);
        OverwatchEpochStartBlock::<Test>::put(epoch_start);

        // A delayed boundary is intentionally one block after a general epoch boundary. It must
        // remain open until slot zero so rollover cannot permanently collide with subnet slots.
        System::set_block_number(unaligned_end);
        Network::on_initialize(unaligned_end);

        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 3);
        assert_eq!(OverwatchEpochStartBlock::<Test>::get(), epoch_start);
        assert!(PendingOverwatchSettlement::<Test>::get().is_none());

        System::set_block_number(aligned_boundary);
        Network::on_initialize(aligned_boundary);

        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 4);
        assert_eq!(OverwatchEpochStartBlock::<Test>::get(), aligned_boundary);
        assert_eq!(
            PendingOverwatchSettlement::<Test>::get().map(|settlement| settlement.epoch),
            Some(3)
        );
        assert_eq!(LastFinalizedOverwatchEpoch::<Test>::get(), None);

        let settlement_block = aligned_boundary + NETWORK_OVERWATCH_SETTLEMENT_SLOT;
        System::set_block_number(settlement_block);
        Network::on_initialize(settlement_block);

        assert!(PendingOverwatchSettlement::<Test>::get().is_none());
        assert_eq!(LastFinalizedOverwatchEpoch::<Test>::get(), Some(3));
    });
}

#[test]
fn test_overwatch_settlement_waits_for_reserved_slot_one() {
    new_test_ext().execute_with(|| {
        let epoch_length = EpochLength::get();
        CurrentOverwatchEpoch::<Test>::put(8);
        OverwatchEpochStartBlock::<Test>::put(epoch_length);
        queue_overwatch_settlement(7);

        // The subnet-emission slot remains reserved. A settlement delayed by a global pause must
        // remain durable instead of pre-empting that work.
        let emission_block = epoch_length + NETWORK_SUBNET_EMISSION_SLOT;
        System::set_block_number(emission_block);
        Network::on_initialize(emission_block);
        assert!(PendingOverwatchSettlement::<Test>::get().is_some());
        assert_eq!(LastFinalizedOverwatchEpoch::<Test>::get(), None);

        let next_settlement_block = epoch_length * 2 + NETWORK_OVERWATCH_SETTLEMENT_SLOT;
        System::set_block_number(next_settlement_block);
        Network::on_initialize(next_settlement_block);
        assert!(PendingOverwatchSettlement::<Test>::get().is_none());
        assert_eq!(LastFinalizedOverwatchEpoch::<Test>::get(), Some(7));
    });
}

#[test]
fn partial_reveal_close_snapshots_only_revealed_records_and_participants() {
    new_test_ext().execute_with(|| {
        OverwatchEpochLengthMultiplier::<Test>::put(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::put(1);
        set_overwatch_epoch(3);

        let epoch = CurrentOverwatchEpoch::<Test>::get();
        let revealed_subnet_id = 1;
        let commit_only_subnet_id = 2;
        insert_subnet(revealed_subnet_id, SubnetState::Active, 0);
        insert_subnet(commit_only_subnet_id, SubnetState::Active, 0);

        manual_insert_validator(1, 101, 201);
        manual_insert_validator(2, 102, 202);
        let revealing_node_id = insert_overwatch_node_v2(1);
        let commit_only_node_id = insert_overwatch_node_v2(2);
        set_overwatch_node_stake(revealing_node_id, 100);
        set_overwatch_node_stake(commit_only_node_id, 200);

        let revealed_weight = test_percent(3, 5);
        let revealed_salt = b"partial-reveal".to_vec();
        let unrevealed_salt = b"unrevealed-subnet".to_vec();
        let commit_only_salt = b"commit-only-node".to_vec();
        assert_ok!(Network::perform_commit_overwatch_subnet_weights(
            revealing_node_id,
            vec![
                OverwatchCommit {
                    subnet_id: revealed_subnet_id,
                    weight: make_commit(revealed_weight, revealed_salt.clone()),
                },
                OverwatchCommit {
                    subnet_id: commit_only_subnet_id,
                    weight: make_commit(test_percent(1, 4), unrevealed_salt),
                },
            ],
        ));
        assert_ok!(Network::perform_commit_overwatch_subnet_weights(
            commit_only_node_id,
            vec![OverwatchCommit {
                subnet_id: revealed_subnet_id,
                weight: make_commit(test_percent(1, 2), commit_only_salt),
            }],
        ));
        assert_ok!(Network::perform_reveal_overwatch_subnet_weights(
            revealing_node_id,
            vec![OverwatchReveal {
                subnet_id: revealed_subnet_id,
                weight: revealed_weight,
                salt: revealed_salt.try_into().unwrap(),
            }],
        ));

        assert_eq!(ActiveOverwatchRevealStats::<Test>::get().records, 1);
        let close_block = OverwatchEpochStartBlock::<Test>::get() + EpochLength::get();
        System::set_block_number(close_block);
        Network::advance_overwatch_epoch(close_block);

        let pending = PendingOverwatchSettlement::<Test>::get()
            .expect("a successful close must publish a pending settlement");
        assert_eq!(pending.epoch, epoch);
        assert_eq!(pending.reveal_records, 1);
        let snapshot = OverwatchEpochSettlementSnapshots::<Test>::get(epoch)
            .expect("a successful close must publish its participant snapshot");
        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(snapshot.nodes.get(&revealing_node_id).unwrap().stake, 100);
        assert!(!snapshot.nodes.contains_key(&commit_only_node_id));
        assert!(OverwatchCommits::<Test>::get(epoch, revealing_node_id).is_empty());
        assert!(OverwatchCommits::<Test>::get(epoch, commit_only_node_id).is_empty());
        assert_eq!(
            OverwatchReveals::<Test>::get(epoch, revealing_node_id).get(&revealed_subnet_id),
            Some(&revealed_weight)
        );
        assert!(OverwatchReveals::<Test>::get(epoch, commit_only_node_id).is_empty());
        assert_eq!(ActiveOverwatchRevealStats::<Test>::get().records, 0);

        Network::calculate_overwatch_rewards();

        assert!(PendingOverwatchSettlement::<Test>::get().is_none());
        assert_eq!(LastFinalizedOverwatchEpoch::<Test>::get(), Some(epoch));
        assert_eq!(
            OverwatchSubnetWeights::<Test>::get(epoch, revealed_subnet_id),
            Some(revealed_weight)
        );
        assert_eq!(
            OverwatchSubnetWeights::<Test>::get(epoch, commit_only_subnet_id),
            None
        );
        assert_eq!(
            OverwatchNodeWeights::<Test>::get(epoch, revealing_node_id),
            Some(Network::percentage_factor_as_u128())
        );
        assert_eq!(
            OverwatchNodeWeights::<Test>::get(epoch, commit_only_node_id),
            None
        );
        assert!(OverwatchReveals::<Test>::get(epoch, revealing_node_id).is_empty());

        let retained = LatestFinalizedOverwatchSignalInputs::<Test>::get()
            .expect("finalization must retain the latest reproducible inputs");
        assert_eq!(retained.source_epoch, epoch);
        assert_eq!(retained.nodes.len(), 1);
        assert_eq!(
            retained
                .nodes
                .get(&revealing_node_id)
                .unwrap()
                .reveals
                .get(&revealed_subnet_id),
            Some(&revealed_weight)
        );
        assert!(!retained.nodes.contains_key(&commit_only_node_id));
        let effective = LatestEffectiveOverwatchSignal::<Test>::get().unwrap();
        assert!(effective.valid);
        assert_eq!(effective.source_epoch, epoch);
        assert_eq!(
            effective.subnet_weights.get(&revealed_subnet_id),
            Some(&revealed_weight)
        );
    });
}

#[test]
fn pending_settlement_blocks_rollover_without_consuming_active_round_state() {
    new_test_ext().execute_with(|| {
        OverwatchEpochLengthMultiplier::<Test>::put(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::put(1);
        set_overwatch_epoch(5);

        let active_epoch = CurrentOverwatchEpoch::<Test>::get();
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        manual_insert_validator(1, 101, 201);
        let node_id = insert_overwatch_node_v2(1);
        set_overwatch_node_stake(node_id, 100);

        let weight = test_percent(2, 5);
        let salt = b"retryable-rollover".to_vec();
        assert_ok!(Network::perform_commit_overwatch_subnet_weights(
            node_id,
            vec![OverwatchCommit {
                subnet_id,
                weight: make_commit(weight, salt.clone()),
            }],
        ));
        assert_ok!(Network::perform_reveal_overwatch_subnet_weights(
            node_id,
            vec![OverwatchReveal {
                subnet_id,
                weight,
                salt: salt.try_into().unwrap(),
            }],
        ));

        queue_overwatch_settlement(active_epoch - 1);
        let pending_before = PendingOverwatchSettlement::<Test>::get().unwrap();
        let commits_before = OverwatchCommits::<Test>::get(active_epoch, node_id);
        let reveals_before = OverwatchReveals::<Test>::get(active_epoch, node_id);
        let stats_before = ActiveOverwatchRevealStats::<Test>::get();
        let start_before = OverwatchEpochStartBlock::<Test>::get();
        let close_block = start_before + EpochLength::get();

        System::set_block_number(close_block);
        Network::advance_overwatch_epoch(close_block);

        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), active_epoch);
        assert_eq!(OverwatchEpochStartBlock::<Test>::get(), start_before);
        assert_eq!(
            PendingOverwatchSettlement::<Test>::get(),
            Some(pending_before)
        );
        assert_eq!(
            OverwatchCommits::<Test>::get(active_epoch, node_id),
            commits_before
        );
        assert_eq!(
            OverwatchReveals::<Test>::get(active_epoch, node_id),
            reveals_before
        );
        assert_eq!(ActiveOverwatchRevealStats::<Test>::get(), stats_before);
        assert!(!OverwatchEpochSettlementSnapshots::<Test>::contains_key(
            active_epoch
        ));

        // Once the older settlement is consumed, retrying the exact same boundary closes the
        // untouched active round and preserves its reveal for finalization.
        Network::calculate_overwatch_rewards();
        assert_eq!(
            LastFinalizedOverwatchEpoch::<Test>::get(),
            Some(active_epoch - 1)
        );
        Network::advance_overwatch_epoch(close_block);

        let retried_pending = PendingOverwatchSettlement::<Test>::get().unwrap();
        assert_eq!(retried_pending.epoch, active_epoch);
        assert_eq!(retried_pending.reveal_records, 1);
        assert!(OverwatchCommits::<Test>::get(active_epoch, node_id).is_empty());
        assert_eq!(
            OverwatchReveals::<Test>::get(active_epoch, node_id).get(&subnet_id),
            Some(&weight)
        );

        Network::calculate_overwatch_rewards();
        assert_eq!(
            LastFinalizedOverwatchEpoch::<Test>::get(),
            Some(active_epoch)
        );
        assert_eq!(
            OverwatchSubnetWeights::<Test>::get(active_epoch, subnet_id),
            Some(weight)
        );
    });
}

#[test]
fn test_global_pause_freezes_overwatch_round_before_unpause_edge() {
    new_test_ext().execute_with(|| {
        let epoch_length = EpochLength::get();
        let pause_block = epoch_length - 2;
        let unpause_block = epoch_length * 2 - 1;

        OverwatchEpochLengthMultiplier::<Test>::put(1);
        ActiveOverwatchEpochLengthMultiplier::<Test>::put(1);
        CurrentOverwatchEpoch::<Test>::put(3);
        OverwatchEpochStartBlock::<Test>::put(0);

        // Pause in the reveal period.
        System::set_block_number(pause_block);
        assert!(!Network::in_overwatch_commit_period());
        assert_ok!(Network::do_pause());

        // on_initialize runs before the unpause extrinsic and therefore still observes pause.
        System::set_block_number(unpause_block);
        Network::on_initialize(unpause_block);
        assert_ok!(Network::do_unpause());

        let shifted_start = unpause_block - pause_block;
        assert_eq!(OverwatchEpochStartBlock::<Test>::get(), shifted_start);
        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 3);

        // Unpausing one block before slot zero must still leave a post-unpause reveal block. The
        // frozen clock has not reached its shifted end, so on_initialize cannot close the round.
        let first_post_unpause_block = unpause_block + 1;
        System::set_block_number(first_post_unpause_block);
        Network::on_initialize(first_post_unpause_block);
        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 3);
        assert!(!Network::in_overwatch_commit_period());

        // The shifted end is unaligned; rollover waits for the next reserved slot-zero boundary.
        let aligned_boundary = epoch_length * 3;
        System::set_block_number(aligned_boundary - 1);
        Network::on_initialize(aligned_boundary - 1);
        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 3);

        System::set_block_number(aligned_boundary);
        Network::on_initialize(aligned_boundary);
        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 4);
    });
}
