use super::mock::*;
use crate::{
    ActiveOverwatchCommitCutoffPercent, ActiveOverwatchEpochLengthMultiplier, BaseSlashPercentage,
    ConsensusValidatorIdentityAttestationPercentage, CurrentOverwatchEpoch,
    InConsensusSubnetReputationFactor, LastFinalizedOverwatchEpoch,
    NotInConsensusSubnetReputationFactor, OverwatchCommitCutoffPercent,
    OverwatchEpochLengthMultiplier, OverwatchEpochStartBlock, PendingOverwatchSettlement,
    PendingOverwatchSettlementData, SubnetElectedValidator, SubnetNodeElectionSlots,
    SubnetNodeMinWeightDecreaseReputationThreshold, SubnetNodeValidatorId,
    SubnetReputationFactorSchedules, SubnetReputationFactors, SubnetSlot, ValidatorReputation,
    ValidatorReputationDecreaseFactor, ValidatorReputationIncreaseFactor,
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
fn validator_election_records_first_and_last_election_epochs() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let subnet_node_id = 10;
        let validator_id = 20;

        SubnetNodeElectionSlots::<Test>::insert(subnet_id, vec![subnet_node_id]);
        SubnetNodeValidatorId::<Test>::insert(subnet_id, subnet_node_id, validator_id);

        let reputation = ValidatorReputation::<Test>::get(validator_id);
        assert_eq!(reputation.start_epoch, None);
        assert_eq!(reputation.last_validator_epoch, None);

        // Epoch zero is a real election epoch, not an "unset" sentinel.
        Network::elect_validator(subnet_id, 0, 0);
        assert_eq!(
            SubnetElectedValidator::<Test>::get(subnet_id, 0)
                .map(|round| round.validator_subnet_node_id),
            Some(subnet_node_id)
        );
        let reputation = ValidatorReputation::<Test>::get(validator_id);
        assert_eq!(reputation.start_epoch, Some(0));
        assert_eq!(reputation.last_validator_epoch, Some(0));

        // Repeating the same election is idempotent.
        Network::elect_validator(subnet_id, 0, 0);
        let reputation = ValidatorReputation::<Test>::get(validator_id);
        assert_eq!(reputation.start_epoch, Some(0));
        assert_eq!(reputation.last_validator_epoch, Some(0));

        Network::elect_validator(subnet_id, 1, EpochLength::get());
        assert_eq!(
            SubnetElectedValidator::<Test>::get(subnet_id, 1)
                .map(|round| round.validator_subnet_node_id),
            Some(subnet_node_id)
        );
        let reputation = ValidatorReputation::<Test>::get(validator_id);
        assert_eq!(reputation.start_epoch, Some(0));
        assert_eq!(reputation.last_validator_epoch, Some(1));
    });
}

#[test]
fn validator_election_metadata_uses_the_general_chain_epoch() {
    new_test_ext().execute_with(|| {
        let subnet_id = 2;
        let subnet_node_id = 11;
        let validator_id = 21;
        let target_subnet_epoch = 42;
        let election_epoch = 5;
        let election_block = election_epoch * EpochLength::get() + 3;

        SubnetNodeElectionSlots::<Test>::insert(subnet_id, vec![subnet_node_id]);
        SubnetNodeValidatorId::<Test>::insert(subnet_id, subnet_node_id, validator_id);

        // Internal callers supply both the target subnet epoch and the election block. Reputation
        // age is a general-chain concept, so its metadata must be derived from the latter.
        Network::elect_validator(subnet_id, target_subnet_epoch, election_block);

        let reputation = ValidatorReputation::<Test>::get(validator_id);
        assert_eq!(reputation.start_epoch, Some(election_epoch));
        assert_eq!(reputation.last_validator_epoch, Some(election_epoch));
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
        let elected_validator_reputation_increase = Network::percent_div(8, 100);
        let elected_validator_reputation_decrease = Network::percent_div(9, 100);
        let elected_subnet_reputation_increase = Network::percent_div(10, 100);
        let elected_subnet_reputation_decrease = Network::percent_div(11, 100);
        let elected_min_weight_threshold = Network::percent_div(12, 100);

        SubnetNodeElectionSlots::<Test>::insert(subnet_id, vec![subnet_node_id]);
        SubnetNodeValidatorId::<Test>::insert(subnet_id, subnet_node_id, validator_id);
        BaseSlashPercentage::<Test>::put(elected_slash_percentage);
        SubnetReputationFactorSchedules::<Test>::mutate(subnet_id, |schedule| {
            schedule.current = elected_reputation_factors;
        });
        ValidatorReputationIncreaseFactor::<Test>::put(elected_validator_reputation_increase);
        ValidatorReputationDecreaseFactor::<Test>::put(elected_validator_reputation_decrease);
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
        ValidatorReputationIncreaseFactor::<Test>::put(Network::percent_div(28, 100));
        ValidatorReputationDecreaseFactor::<Test>::put(Network::percent_div(29, 100));
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
            round.policy.validator_reputation_increase_factor,
            elected_validator_reputation_increase
        );
        assert_eq!(
            round.policy.validator_reputation_decrease_factor,
            elected_validator_reputation_decrease
        );
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
        assert_eq!(settlement.epoch_length_multiplier, old_multiplier);
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
        assert_eq!(settlement.epoch_length_multiplier, next_multiplier);
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

        System::set_block_number(aligned_boundary + 1);
        Network::on_initialize(aligned_boundary + 1);

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
        PendingOverwatchSettlement::<Test>::put(PendingOverwatchSettlementData {
            epoch: 7,
            epoch_length_multiplier: 1,
            reveal_records: 0,
            revealing_nodes: 0,
            revealed_subnets: 0,
        });

        // Slot two is reserved for global subnet-emission calculation. A settlement delayed by a
        // global pause must remain durable instead of pre-empting that work.
        let slot_two = epoch_length + 2;
        System::set_block_number(slot_two);
        Network::on_initialize(slot_two);
        assert!(PendingOverwatchSettlement::<Test>::get().is_some());
        assert_eq!(LastFinalizedOverwatchEpoch::<Test>::get(), None);

        let next_slot_one = epoch_length * 2 + 1;
        System::set_block_number(next_slot_one);
        Network::on_initialize(next_slot_one);
        assert!(PendingOverwatchSettlement::<Test>::get().is_none());
        assert_eq!(LastFinalizedOverwatchEpoch::<Test>::get(), Some(7));
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
