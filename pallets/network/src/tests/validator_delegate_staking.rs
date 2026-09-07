use super::mock::*;
use crate::tests::test_utils::*;
use crate::{
    AccountSubnetDelegateStakeShares, AccountValidatorDelegateStakeGeneration,
    AccountValidatorDelegateStakeShares, BaseSlashPercentage,
    BaseValidatorDelegateStakeSlashPercentage, BaseValidatorReward,
    ConsensusRoundSettlementEmissionEpoch, ConsensusRoundSettlementStatus, ConsensusSubmissionData,
    DelegateStakeCooldownEpochs, DistributionData, ElectedConsensusRound, Error, Event,
    FinalSubnetEmissionWeights, MaxSlashAmount, MaxSubnetNodes,
    MaxValidatorDelegateStakeSlashAmount, MinDelegateStakeDeposit, MinSubnetMinStake,
    NextSwapQueueId, NodeDelegateStakeCooldownEpochs, NodeStakePendingSlashLiabilityCount,
    NodeSubnetStake, PendingConsensusRoundSettlementEpoch, QueuedSwapCall, RewardsData,
    StakeUnbondingLedger, SubnetElectedValidator, SubnetName, SubnetNodeElectionSlots,
    SubnetNodeReputation, SubnetNodeValidatorId, SubnetReputation, SwapCallQueue, SwapQueueOrder,
    TotalActiveSubnets, TotalStake, TotalSubnetDelegateStakeBalance, TotalSubnetNodes,
    TotalSubnetStake, TotalValidatorDelegateStakeBalance, TxRateLimit,
    ValidatorDelegatePoolGeneration, ValidatorDelegateStakeBalance,
    ValidatorDelegateStakeCirculatingShares, ValidatorDelegateStakePendingSlashLiabilityCount,
    ValidatorDelegateStakeShares, ValidatorDelegateStakeSlashLockUntil,
    ValidatorDelegateStakeSlashThreshold,
};
use frame_support::traits::Currency;
use frame_support::weights::WeightMeter;
use frame_support::{assert_err, assert_ok};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;

fn build_pending_full_slash_round(
    subnet_name: Vec<u8>,
    staker_id: u32,
    pool_balance: u128,
) -> (u32, u32, ElectedConsensusRound) {
    let factor = Network::percentage_factor_as_u128();
    BaseSlashPercentage::<Test>::put(factor);
    MaxSlashAmount::<Test>::put(u128::MAX);
    ValidatorDelegateStakeSlashThreshold::<Test>::put(test_percent(1, 2));
    BaseValidatorDelegateStakeSlashPercentage::<Test>::put(factor);
    MaxValidatorDelegateStakeSlashAmount::<Test>::put(u128::MAX);

    build_activated_subnet(
        subnet_name.clone(),
        0,
        16,
        10_000_000_000_000_000_000_000,
        MinSubnetMinStake::<Test>::get(),
    );
    let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
    let validator_ids = (1..=TotalSubnetNodes::<Test>::get(subnet_id))
        .filter_map(|node_id| SubnetNodeValidatorId::<Test>::get(subnet_id, node_id))
        .collect::<BTreeSet<_>>();
    for validator_id in validator_ids {
        Network::handle_increase_account_validator_delegate_stake(
            &account(staker_id),
            validator_id,
            pool_balance,
        )
        .expect("valid validator-pool initialization must succeed");
    }
    seed_equal_validator_delegate_stake_for_subnet(subnet_id);

    let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
    Network::elect_validator(subnet_id, subnet_epoch, System::block_number());
    let round = SubnetElectedValidator::<Test>::get(subnet_id, subnet_epoch)
        .expect("the slashable fixture must elect a validator");
    (subnet_id, subnet_epoch, round)
}

fn rejected_round_submission(
    subnet_id: u32,
    round: &ElectedConsensusRound,
) -> ConsensusSubmissionData<Test> {
    ConsensusSubmissionData::<Test> {
        policy: round.policy,
        validator_subnet_node_id: round.validator_subnet_node_id,
        validator_node_stake_balance: round.validator_node_stake_balance,
        validator_delegate_stake_balance: round.validator_delegate_stake_balance,
        validator_epoch_progress: 0,
        validator_reward_factor: 0,
        attestation_ratio: 0,
        identity_attestation_ratio: 0,
        identity_attestation_count: 0,
        eligible_validator_identity_count: TotalSubnetNodes::<Test>::get(subnet_id),
        weight_sum: 0,
        data_length: 0,
        data: Vec::new(),
        attests: Default::default(),
        subnet_nodes: Vec::new(),
        prioritize_queue_node_id: None,
        remove_queue_node_id: None,
        emergency: round.emergency.clone(),
    }
}

#[test]
fn test_validator_delegate_pool_slash_formula_boundaries_and_caps() {
    new_test_ext().execute_with(|| {
        let factor = Network::percentage_factor_as_u128();
        let threshold = test_percent(1, 2);
        let base_percentage = test_percent(1, 5);

        assert_eq!(
            Network::get_validator_delegate_stake_slash_amount(
                1_000,
                1_000,
                threshold,
                threshold,
                base_percentage,
                1_000,
            ),
            0
        );

        // Halfway from the threshold to zero applies half of the configured base slash.
        assert_eq!(
            Network::get_validator_delegate_stake_slash_amount(
                1_000,
                1_000,
                threshold / 2,
                threshold,
                base_percentage,
                1_000,
            ),
            100
        );

        // Zero support reaches the full configured base percentage.
        assert_eq!(
            Network::get_validator_delegate_stake_slash_amount(
                1_000,
                1_000,
                0,
                threshold,
                base_percentage,
                1_000,
            ),
            200
        );

        // The live pool and absolute maximum independently cap settlement liability.
        assert_eq!(
            Network::get_validator_delegate_stake_slash_amount(
                1_000, 75, 0, threshold, factor, 1_000,
            ),
            75
        );
        assert_eq!(
            Network::get_validator_delegate_stake_slash_amount(
                1_000, 1_000, 0, threshold, factor, 80,
            ),
            80
        );

        // Disabled configuration, empty pools, and sub-unit rounding never create a loss.
        assert_eq!(
            Network::get_validator_delegate_stake_slash_amount(
                1_000, 1_000, 0, threshold, 0, 1_000,
            ),
            0
        );
        assert_eq!(
            Network::get_validator_delegate_stake_slash_amount(
                1_000,
                1_000,
                0,
                threshold,
                base_percentage,
                0,
            ),
            0
        );
        assert_eq!(
            Network::get_validator_delegate_stake_slash_amount(
                0,
                1_000,
                0,
                threshold,
                base_percentage,
                1_000,
            ),
            0
        );
        assert_eq!(
            Network::get_validator_delegate_stake_slash_amount(
                1,
                1,
                0,
                threshold,
                test_percent(1, 2),
                1,
            ),
            0
        );

        // Arithmetic saturates safely at the balance type's upper bound.
        assert_eq!(
            Network::get_validator_delegate_stake_slash_amount(
                u128::MAX,
                u128::MAX,
                0,
                threshold,
                factor,
                u128::MAX,
            ),
            u128::MAX
        );

        // A representable rate immediately below the threshold is slashable.
        let large_pool = 1_000_000_000_000_000_000_000_000_000_000_000_000_u128;
        assert!(
            Network::get_validator_delegate_stake_slash_amount(
                large_pool,
                large_pool,
                threshold - 1,
                threshold,
                factor,
                u128::MAX,
            ) > 0
        );
    });
}

#[test]
fn active_zero_rounded_pool_slash_still_validates_accounting_shape() {
    new_test_ext().execute_with(|| {
        let subnet_id = 76;
        let subnet_node_id = 8;
        let validator_id = 4;
        let factor = Network::percentage_factor_as_u128();
        SubnetNodeValidatorId::<Test>::insert(subnet_id, subnet_node_id, validator_id);

        // One atomic unit at a 50% base rate rounds the actual loss to zero. The active slash
        // branch must nevertheless reject a malformed pool instead of blessing its round.
        ValidatorDelegateStakeBalance::<Test>::insert(validator_id, 1);
        ValidatorDelegateStakeShares::<Test>::insert(validator_id, 1);
        ValidatorDelegateStakeCirculatingShares::<Test>::insert(validator_id, 0);
        TotalValidatorDelegateStakeBalance::<Test>::put(1);
        let events_before = network_events();
        let (slash_result, _) = Network::apply_validator_economic_slashes(
            subnet_id,
            subnet_node_id,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            factor,
            test_percent(1, 2),
            1,
        );
        assert_eq!(
            slash_result,
            Err(Error::<Test>::DelegatePoolInvariantViolation.into())
        );
        assert_eq!(ValidatorDelegateStakeBalance::<Test>::get(validator_id), 1);
        assert_eq!(ValidatorDelegateStakeShares::<Test>::get(validator_id), 1);
        assert_eq!(network_events(), events_before);

        // A valid pool with the same zero-rounded result commits normally. Disabled pool policy
        // likewise does not require a retained validator mapping merely to settle a node decision.
        ValidatorDelegateStakeShares::<Test>::insert(
            validator_id,
            Network::DELEGATE_POOL_MIN_LIQUIDITY,
        );
        let (slash_result, _) = Network::apply_validator_economic_slashes(
            subnet_id,
            subnet_node_id,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            factor,
            test_percent(1, 2),
            1,
        );
        assert_eq!(slash_result, Ok((Some(validator_id), 0, 0)));

        SubnetNodeValidatorId::<Test>::remove(subnet_id, subnet_node_id);
        let (slash_result, _) = Network::apply_validator_economic_slashes(
            subnet_id,
            subnet_node_id,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            factor,
            0,
            1,
        );
        assert_eq!(slash_result, Ok((None, 0, 0)));
    });
}

#[test]
fn test_total_validator_delegate_pool_loss_starts_a_fresh_generation() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let subnet_id = 77;
        let subnet_node_id = 9;
        let validator_id = 5;
        let old_staker = account(940);
        let new_staker = account(941);
        let pool_balance = 1_000u128;
        let factor = Network::percentage_factor_as_u128();

        SubnetNodeValidatorId::<Test>::insert(subnet_id, subnet_node_id, validator_id);
        Network::handle_increase_account_validator_delegate_stake(
            &old_staker,
            validator_id,
            pool_balance,
        )
        .expect("initial validator pool position must be accepted");

        let old_raw_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(&old_staker, validator_id);
        let old_circulating = ValidatorDelegateStakeCirculatingShares::<Test>::get(validator_id);
        assert_eq!(old_raw_shares, old_circulating);
        assert_eq!(
            ValidatorDelegatePoolGeneration::<Test>::get(validator_id),
            0
        );
        assert_eq!(
            AccountValidatorDelegateStakeGeneration::<Test>::get(&old_staker, validator_id),
            0
        );

        let (slash_result, _) = Network::apply_validator_economic_slashes(
            subnet_id,
            subnet_node_id,
            0,
            0,
            0,
            0,
            0,
            0,
            pool_balance,
            factor,
            factor,
            pool_balance,
        );
        let (_, _, delegate_loss) = slash_result.expect("valid full slash must commit");
        assert_eq!(delegate_loss, pool_balance);
        assert_eq!(ValidatorDelegateStakeBalance::<Test>::get(validator_id), 0);
        assert_eq!(ValidatorDelegateStakeShares::<Test>::get(validator_id), 0);
        assert_eq!(
            ValidatorDelegateStakeCirculatingShares::<Test>::get(validator_id),
            0
        );
        assert_eq!(TotalValidatorDelegateStakeBalance::<Test>::get(), 0);
        assert_eq!(
            ValidatorDelegatePoolGeneration::<Test>::get(validator_id),
            1
        );

        // The raw value may remain in storage, but generation-aware accounting gives it no claim
        // on a newly initialized pool.
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&old_staker, validator_id),
            old_raw_shares
        );
        assert_eq!(
            Network::current_account_validator_delegate_stake_shares(&old_staker, validator_id),
            0
        );

        Network::handle_increase_account_validator_delegate_stake(
            &new_staker,
            validator_id,
            pool_balance,
        )
        .expect("a fresh generation must accept a normal deposit");
        assert!(
            Network::current_account_validator_delegate_stake_shares(&new_staker, validator_id) > 0
        );
        assert_eq!(
            AccountValidatorDelegateStakeGeneration::<Test>::get(&new_staker, validator_id),
            1
        );
        assert_eq!(
            Network::current_account_validator_delegate_stake_shares(&old_staker, validator_id),
            0
        );
        assert!(network_events().iter().any(|event| {
            matches!(
                event,
                Event::ValidatorDelegatePoolReset {
                    validator_id: event_validator_id,
                    old_generation: 0,
                    new_generation: 1,
                    invalidated_shares,
                } if *event_validator_id == validator_id && *invalidated_shares == old_circulating
            )
        }));
    });
}

#[test]
fn test_validator_pool_generation_overflow_rolls_back_full_loss() {
    new_test_ext().execute_with(|| {
        let subnet_id = 78;
        let subnet_node_id = 10;
        let validator_id = 6;
        let staker = account(942);
        let pool_balance = 1_000u128;
        let factor = Network::percentage_factor_as_u128();

        SubnetNodeValidatorId::<Test>::insert(subnet_id, subnet_node_id, validator_id);
        Network::handle_increase_account_validator_delegate_stake(
            &staker,
            validator_id,
            pool_balance,
        )
        .expect("initial validator pool position must be accepted");
        ValidatorDelegatePoolGeneration::<Test>::insert(validator_id, u64::MAX);
        AccountValidatorDelegateStakeGeneration::<Test>::insert(&staker, validator_id, u64::MAX);

        let account_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(&staker, validator_id);
        let total_shares = ValidatorDelegateStakeShares::<Test>::get(validator_id);
        let circulating_shares = ValidatorDelegateStakeCirculatingShares::<Test>::get(validator_id);
        let total_balance = TotalValidatorDelegateStakeBalance::<Test>::get();

        let (slash_result, _) = Network::apply_validator_economic_slashes(
            subnet_id,
            subnet_node_id,
            0,
            0,
            0,
            0,
            0,
            0,
            pool_balance,
            factor,
            factor,
            pool_balance,
        );
        assert_eq!(
            slash_result,
            Err(sp_runtime::ArithmeticError::Overflow.into())
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            pool_balance
        );
        assert_eq!(
            ValidatorDelegateStakeShares::<Test>::get(validator_id),
            total_shares
        );
        assert_eq!(
            ValidatorDelegateStakeCirculatingShares::<Test>::get(validator_id),
            circulating_shares
        );
        assert_eq!(
            TotalValidatorDelegateStakeBalance::<Test>::get(),
            total_balance
        );
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&staker, validator_id),
            account_shares
        );
        assert_eq!(
            ValidatorDelegatePoolGeneration::<Test>::get(validator_id),
            u64::MAX
        );
    });
}

#[test]
fn corrupt_validator_pool_aggregate_keeps_round_pending_until_atomic_retry() {
    new_test_ext().execute_with(|| {
        let pool_balance = 1_000_000u128;
        let (subnet_id, subnet_epoch, round) =
            build_pending_full_slash_round(b"slash-corrupt-aggregate".to_vec(), 943, pool_balance);
        let validator_id = round.validator_id;
        let node_id = round.validator_subnet_node_id;
        let rejected = rejected_round_submission(subnet_id, &round);

        let node_stake_before = NodeSubnetStake::<Test>::get(node_id, subnet_id);
        let subnet_stake_before = TotalSubnetStake::<Test>::get(subnet_id);
        let total_stake_before = TotalStake::<Test>::get();
        let pool_before = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let valid_pool_aggregate = TotalValidatorDelegateStakeBalance::<Test>::get();
        let node_reputation_before = SubnetNodeReputation::<Test>::get(subnet_id, node_id);
        let subnet_reputation_before = SubnetReputation::<Test>::get(subnet_id);
        let issuance_before = Balances::total_issuance();
        let events_before = network_events();

        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet_id, subnet_epoch),
            Some(false)
        );
        assert_eq!(
            NodeStakePendingSlashLiabilityCount::<Test>::get(subnet_id, node_id),
            1
        );
        assert_eq!(
            ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
            1
        );

        // Force the shared aggregate below the pool loss. The node debit is evaluated first, but
        // the later aggregate underflow must roll the complete economic decision back.
        TotalValidatorDelegateStakeBalance::<Test>::put(0);
        Network::distribute_rewards(
            &mut WeightMeter::new(),
            subnet_id,
            subnet_epoch + 1,
            rejected.clone(),
            RewardsData::default(),
        );

        assert_eq!(
            NodeSubnetStake::<Test>::get(node_id, subnet_id),
            node_stake_before
        );
        assert_eq!(
            TotalSubnetStake::<Test>::get(subnet_id),
            subnet_stake_before
        );
        assert_eq!(TotalStake::<Test>::get(), total_stake_before);
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            pool_before
        );
        assert_eq!(TotalValidatorDelegateStakeBalance::<Test>::get(), 0);
        assert_eq!(
            SubnetNodeReputation::<Test>::get(subnet_id, node_id),
            node_reputation_before
        );
        assert_eq!(
            SubnetReputation::<Test>::get(subnet_id),
            subnet_reputation_before
        );
        assert_eq!(Balances::total_issuance(), issuance_before);
        assert_eq!(network_events(), events_before);
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet_id, subnet_epoch),
            Some(false)
        );
        assert_eq!(
            PendingConsensusRoundSettlementEpoch::<Test>::get(subnet_id),
            Some(subnet_epoch)
        );
        assert_eq!(
            NodeStakePendingSlashLiabilityCount::<Test>::get(subnet_id, node_id),
            1
        );
        assert_eq!(
            ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
            1
        );
        assert!(Network::is_node_stake_slash_locked(subnet_id, node_id));
        assert!(Network::is_validator_delegate_stake_slash_locked(
            validator_id
        ));

        // Repair only the corrupt aggregate. The exact same immutable round can now commit both
        // losses once and release both liabilities.
        TotalValidatorDelegateStakeBalance::<Test>::put(valid_pool_aggregate);
        Network::distribute_rewards(
            &mut WeightMeter::new(),
            subnet_id,
            subnet_epoch + 1,
            rejected,
            RewardsData::default(),
        );
        assert!(NodeSubnetStake::<Test>::get(node_id, subnet_id) < node_stake_before);
        assert!(ValidatorDelegateStakeBalance::<Test>::get(validator_id) < pool_before);
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet_id, subnet_epoch),
            Some(true)
        );
        assert_eq!(
            PendingConsensusRoundSettlementEpoch::<Test>::get(subnet_id),
            None
        );
        assert_eq!(
            NodeStakePendingSlashLiabilityCount::<Test>::get(subnet_id, node_id),
            0
        );
        assert_eq!(
            ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
            0
        );
    });
}

#[test]
fn validator_pool_generation_overflow_keeps_round_pending_until_atomic_retry() {
    new_test_ext().execute_with(|| {
        let staker_id = 944;
        let pool_balance = 1_000_000u128;
        let (subnet_id, subnet_epoch, round) = build_pending_full_slash_round(
            b"slash-generation-overflow".to_vec(),
            staker_id,
            pool_balance,
        );
        let validator_id = round.validator_id;
        let node_id = round.validator_subnet_node_id;

        ValidatorDelegatePoolGeneration::<Test>::insert(validator_id, u64::MAX);
        AccountValidatorDelegateStakeGeneration::<Test>::insert(
            account(staker_id),
            validator_id,
            u64::MAX,
        );
        let node_stake_before = NodeSubnetStake::<Test>::get(node_id, subnet_id);
        let subnet_stake_before = TotalSubnetStake::<Test>::get(subnet_id);
        let total_stake_before = TotalStake::<Test>::get();
        let pool_before = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let pool_aggregate_before = TotalValidatorDelegateStakeBalance::<Test>::get();
        let node_reputation_before = SubnetNodeReputation::<Test>::get(subnet_id, node_id);
        let subnet_reputation_before = SubnetReputation::<Test>::get(subnet_id);
        let events_before = network_events();

        let (submission, _) = Network::precheck_subnet_consensus_submission(
            subnet_id,
            subnet_epoch,
            Network::get_current_epoch_as_u32(),
        );
        assert!(submission.is_none());

        assert_eq!(
            NodeSubnetStake::<Test>::get(node_id, subnet_id),
            node_stake_before
        );
        assert_eq!(
            TotalSubnetStake::<Test>::get(subnet_id),
            subnet_stake_before
        );
        assert_eq!(TotalStake::<Test>::get(), total_stake_before);
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            pool_before
        );
        assert_eq!(
            TotalValidatorDelegateStakeBalance::<Test>::get(),
            pool_aggregate_before
        );
        assert_eq!(
            ValidatorDelegatePoolGeneration::<Test>::get(validator_id),
            u64::MAX
        );
        assert_eq!(
            SubnetNodeReputation::<Test>::get(subnet_id, node_id),
            node_reputation_before
        );
        assert_eq!(
            SubnetReputation::<Test>::get(subnet_id),
            subnet_reputation_before
        );
        assert_eq!(network_events(), events_before);
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet_id, subnet_epoch),
            Some(false)
        );
        assert_eq!(
            PendingConsensusRoundSettlementEpoch::<Test>::get(subnet_id),
            Some(subnet_epoch)
        );
        assert_eq!(
            NodeStakePendingSlashLiabilityCount::<Test>::get(subnet_id, node_id),
            1
        );
        assert_eq!(
            ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
            1
        );

        // Restore a representable successor generation and retry. The full pool loss advances to
        // MAX exactly; the earlier rolled-back node loss is applied only once.
        ValidatorDelegatePoolGeneration::<Test>::insert(validator_id, u64::MAX - 1);
        AccountValidatorDelegateStakeGeneration::<Test>::insert(
            account(staker_id),
            validator_id,
            u64::MAX - 1,
        );
        let (submission, _) = Network::precheck_subnet_consensus_submission(
            subnet_id,
            subnet_epoch,
            Network::get_current_epoch_as_u32(),
        );
        assert!(submission.is_none());
        assert!(NodeSubnetStake::<Test>::get(node_id, subnet_id) < node_stake_before);
        assert_eq!(ValidatorDelegateStakeBalance::<Test>::get(validator_id), 0);
        assert_eq!(
            ValidatorDelegatePoolGeneration::<Test>::get(validator_id),
            u64::MAX
        );
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet_id, subnet_epoch),
            Some(true)
        );
        assert_eq!(
            PendingConsensusRoundSettlementEpoch::<Test>::get(subnet_id),
            None
        );
        assert_eq!(
            NodeStakePendingSlashLiabilityCount::<Test>::get(subnet_id, node_id),
            0
        );
        assert_eq!(
            ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
            0
        );
    });
}

#[test]
fn missing_retained_validator_mapping_keeps_exposed_round_pending() {
    new_test_ext().execute_with(|| {
        let pool_balance = 1_000_000u128;
        let (subnet_id, subnet_epoch, round) = build_pending_full_slash_round(
            b"slash-missing-validator-map".to_vec(),
            945,
            pool_balance,
        );
        let validator_id = round.validator_id;
        let node_id = round.validator_subnet_node_id;
        let rejected = rejected_round_submission(subnet_id, &round);
        let node_stake_before = NodeSubnetStake::<Test>::get(node_id, subnet_id);
        let pool_before = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let node_reputation_before = SubnetNodeReputation::<Test>::get(subnet_id, node_id);
        let subnet_reputation_before = SubnetReputation::<Test>::get(subnet_id);

        SubnetNodeValidatorId::<Test>::remove(subnet_id, node_id);
        let events_before = network_events();
        Network::distribute_rewards(
            &mut WeightMeter::new(),
            subnet_id,
            subnet_epoch + 1,
            rejected.clone(),
            RewardsData::default(),
        );

        assert_eq!(
            NodeSubnetStake::<Test>::get(node_id, subnet_id),
            node_stake_before
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            pool_before
        );
        assert_eq!(
            SubnetNodeReputation::<Test>::get(subnet_id, node_id),
            node_reputation_before
        );
        assert_eq!(
            SubnetReputation::<Test>::get(subnet_id),
            subnet_reputation_before
        );
        assert_eq!(network_events(), events_before);
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet_id, subnet_epoch),
            Some(false)
        );
        assert_eq!(
            PendingConsensusRoundSettlementEpoch::<Test>::get(subnet_id),
            Some(subnet_epoch)
        );
        assert_eq!(
            NodeStakePendingSlashLiabilityCount::<Test>::get(subnet_id, node_id),
            1
        );
        assert_eq!(
            ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
            1
        );

        SubnetNodeValidatorId::<Test>::insert(subnet_id, node_id, validator_id);
        Network::distribute_rewards(
            &mut WeightMeter::new(),
            subnet_id,
            subnet_epoch + 1,
            rejected,
            RewardsData::default(),
        );
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet_id, subnet_epoch),
            Some(true)
        );
        assert_eq!(
            PendingConsensusRoundSettlementEpoch::<Test>::get(subnet_id),
            None
        );
        assert!(NodeSubnetStake::<Test>::get(node_id, subnet_id) < node_stake_before);
        assert!(ValidatorDelegateStakeBalance::<Test>::get(validator_id) < pool_before);
    });
}

#[test]
fn test_validator_delegate_pool_slashing_launch_defaults_do_not_lock_elected_pool() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            ValidatorDelegateStakeSlashThreshold::<Test>::get(),
            333_333_333_333_333_333
        );
        assert_eq!(BaseValidatorDelegateStakeSlashPercentage::<Test>::get(), 0);
        assert_eq!(MaxValidatorDelegateStakeSlashAmount::<Test>::get(), 0);

        let subnet_name: Vec<u8> = "delegate-slash-disabled".into();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            16,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();

        let pool_balance = 1_000_000_u128;
        let validator_ids = (1..=TotalSubnetNodes::<Test>::get(subnet_id))
            .filter_map(|subnet_node_id| {
                SubnetNodeValidatorId::<Test>::get(subnet_id, subnet_node_id)
            })
            .collect::<BTreeSet<_>>();
        for validator_id in validator_ids {
            let (balance_added, shares_added) =
                Network::handle_increase_account_validator_delegate_stake(
                    &account(920),
                    validator_id,
                    pool_balance,
                )
                .expect("validator delegate stake credit must succeed");
            assert_eq!(balance_added, pool_balance);
            assert!(shares_added > 0);
        }
        seed_equal_validator_delegate_stake_for_subnet(subnet_id);

        let election_block = System::block_number();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        Network::elect_validator(subnet_id, subnet_epoch, election_block);

        let elected_node_id = get_elected_subnet_node_id(subnet_id, subnet_epoch).unwrap();
        let validator_id = SubnetNodeValidatorId::<Test>::get(subnet_id, elected_node_id).unwrap();
        let starting_node_stake = NodeSubnetStake::<Test>::get(elected_node_id, subnet_id);
        let starting_node_reputation =
            SubnetNodeReputation::<Test>::get(subnet_id, elected_node_id).unwrap();
        let starting_pool_balance = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let starting_total_pool_balance = TotalValidatorDelegateStakeBalance::<Test>::get();
        let starting_total_shares = ValidatorDelegateStakeShares::<Test>::get(validator_id);
        let starting_account_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(account(920), validator_id);

        assert_eq!(
            ValidatorDelegateStakeSlashLockUntil::<Test>::get(validator_id),
            0
        );

        // Settle a missing proposal at 0% support. Launch defaults must preserve the existing
        // node-stake and absence-reputation penalties without touching delegator principal.
        let (submission, _) = Network::precheck_subnet_consensus_submission(
            subnet_id,
            subnet_epoch,
            Network::get_current_epoch_as_u32(),
        );
        assert!(submission.is_none());
        assert!(NodeSubnetStake::<Test>::get(elected_node_id, subnet_id) < starting_node_stake);
        assert!(
            SubnetNodeReputation::<Test>::get(subnet_id, elected_node_id).unwrap()
                < starting_node_reputation
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            starting_pool_balance
        );
        assert_eq!(
            TotalValidatorDelegateStakeBalance::<Test>::get(),
            starting_total_pool_balance
        );
        assert_eq!(
            ValidatorDelegateStakeShares::<Test>::get(validator_id),
            starting_total_shares
        );
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(account(920), validator_id),
            starting_account_shares
        );
        assert!(network_events().iter().any(|event| {
            matches!(
                event,
                Event::ValidatorSlashApplied {
                    subnet_id: event_subnet_id,
                    validator_id: event_validator_id,
                    subnet_node_id,
                    node_stake_amount,
                    validator_delegate_stake_amount: 0,
                    ..
                } if *event_subnet_id == subnet_id
                    && *event_validator_id == validator_id
                    && *subnet_node_id == elected_node_id
                    && *node_stake_amount > 0
            )
        }));
    });
}

#[test]
fn test_election_snapshots_delegate_slash_policy_and_extends_pool_lock() {
    new_test_ext().execute_with(|| {
        let threshold = test_percent(1, 3);
        let base_percentage = test_percent(1, 10);
        let max_amount = 500_000;
        ValidatorDelegateStakeSlashThreshold::<Test>::put(threshold);
        BaseValidatorDelegateStakeSlashPercentage::<Test>::put(base_percentage);
        MaxValidatorDelegateStakeSlashAmount::<Test>::put(max_amount);

        let subnet_name: Vec<u8> = "delegate-slash-snapshot".into();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            16,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let second_subnet_name: Vec<u8> = "delegate-slash-overlap".into();
        build_activated_subnet(
            second_subnet_name.clone(),
            0,
            16,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let second_subnet_id = SubnetName::<Test>::get(second_subnet_name).unwrap();
        let pool_balance = 1_000_000_u128;
        let validator_ids = (1..=TotalSubnetNodes::<Test>::get(subnet_id))
            .filter_map(|node_id| SubnetNodeValidatorId::<Test>::get(subnet_id, node_id))
            .collect::<BTreeSet<_>>();
        for validator_id in validator_ids {
            Network::handle_increase_account_validator_delegate_stake(
                &account(925),
                validator_id,
                pool_balance,
            )
            .expect("valid pool initialization must succeed");
        }
        seed_equal_validator_delegate_stake_for_subnet(subnet_id);
        seed_equal_validator_delegate_stake_for_subnet(second_subnet_id);

        // Force the same validator identity in both subnets. One pending round per subnet is
        // permitted, while the shared identity pool must retain both liabilities.
        let shared_validator_id = 1;
        let first_node_id = (1..=TotalSubnetNodes::<Test>::get(subnet_id))
            .find(|node_id| {
                SubnetNodeValidatorId::<Test>::get(subnet_id, *node_id) == Some(shared_validator_id)
            })
            .unwrap();
        let second_node_id = (1..=TotalSubnetNodes::<Test>::get(second_subnet_id))
            .find(|node_id| {
                SubnetNodeValidatorId::<Test>::get(second_subnet_id, *node_id)
                    == Some(shared_validator_id)
            })
            .unwrap();
        SubnetNodeElectionSlots::<Test>::mutate(subnet_id, |slots| {
            slots.retain(|node_id| *node_id == first_node_id);
        });
        SubnetNodeElectionSlots::<Test>::mutate(second_subnet_id, |slots| {
            slots.retain(|node_id| *node_id == second_node_id);
        });

        let election_block = System::block_number();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        Network::elect_validator(subnet_id, subnet_epoch, election_block);

        let round = SubnetElectedValidator::<Test>::get(subnet_id, subnet_epoch).unwrap();
        let validator_id =
            SubnetNodeValidatorId::<Test>::get(subnet_id, round.validator_subnet_node_id).unwrap();
        assert_eq!(
            round.policy.validator_delegate_stake_slash_threshold,
            threshold
        );
        assert_eq!(
            round.policy.base_validator_delegate_stake_slash_percentage,
            base_percentage
        );
        assert_eq!(
            round.policy.max_validator_delegate_stake_slash_amount,
            max_amount
        );
        assert_eq!(
            round.validator_delegate_stake_balance,
            ValidatorDelegateStakeBalance::<Test>::get(validator_id)
        );
        assert_eq!(
            ValidatorDelegateStakeSlashLockUntil::<Test>::get(validator_id),
            election_block + EpochLength::get()
        );
        assert_eq!(
            ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
            1
        );
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet_id, subnet_epoch),
            Some(false)
        );

        ValidatorDelegateStakeSlashThreshold::<Test>::put(test_percent(1, 4));
        BaseValidatorDelegateStakeSlashPercentage::<Test>::put(test_percent(1, 5));
        MaxValidatorDelegateStakeSlashAmount::<Test>::put(max_amount * 2);

        let unchanged_round = SubnetElectedValidator::<Test>::get(subnet_id, subnet_epoch).unwrap();
        assert_eq!(
            unchanged_round
                .policy
                .validator_delegate_stake_slash_threshold,
            threshold
        );
        assert_eq!(
            unchanged_round
                .policy
                .base_validator_delegate_stake_slash_percentage,
            base_percentage
        );
        assert_eq!(
            unchanged_round
                .policy
                .max_validator_delegate_stake_slash_amount,
            max_amount
        );

        // Elect the same identity in another subnet. Its later settlement extends the shared
        // identity-level pool lock instead of shortening it.
        let overlapping_election_block = election_block + 1;
        let second_subnet_epoch = Network::get_current_subnet_epoch_as_u32(second_subnet_id);
        Network::elect_validator(
            second_subnet_id,
            second_subnet_epoch,
            overlapping_election_block,
        );
        let later_settlement = overlapping_election_block + EpochLength::get();
        assert_eq!(
            ValidatorDelegateStakeSlashLockUntil::<Test>::get(validator_id),
            later_settlement
        );
        assert_eq!(
            ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
            2
        );
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(second_subnet_id, second_subnet_epoch),
            Some(false)
        );

        // Neither wall-clock expiry nor settling one of two rounds can unlock the shared pool.
        System::set_block_number(later_settlement.saturating_add(5));
        assert!(Network::is_validator_delegate_stake_slash_locked(
            validator_id
        ));
        let pool_before_first_settlement = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let (first_submission, _) = Network::precheck_subnet_consensus_submission(
            subnet_id,
            subnet_epoch,
            Network::get_current_epoch_as_u32(),
        );
        assert!(first_submission.is_none());
        assert_eq!(
            ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
            1
        );
        assert!(Network::is_validator_delegate_stake_slash_locked(
            validator_id
        ));
        let pool_after_first_settlement = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        assert!(pool_after_first_settlement < pool_before_first_settlement);

        let (duplicate_submission, _) = Network::precheck_subnet_consensus_submission(
            subnet_id,
            subnet_epoch,
            Network::get_current_epoch_as_u32(),
        );
        assert!(duplicate_submission.is_none());
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            pool_after_first_settlement
        );
        assert_eq!(
            ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
            1
        );

        let (second_submission, _) = Network::precheck_subnet_consensus_submission(
            second_subnet_id,
            second_subnet_epoch,
            Network::get_current_epoch_as_u32(),
        );
        assert!(second_submission.is_none());
        assert_eq!(
            ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
            0
        );
        assert!(!Network::is_validator_delegate_stake_slash_locked(
            validator_id
        ));
    });
}

#[test]
fn submitted_round_slash_and_liability_release_are_exact_once_after_nominal_unlock() {
    new_test_ext().execute_with(|| {
        let threshold = test_percent(1, 2);
        BaseValidatorDelegateStakeSlashPercentage::<Test>::put(test_percent(1, 10));
        MaxValidatorDelegateStakeSlashAmount::<Test>::put(u128::MAX);
        ValidatorDelegateStakeSlashThreshold::<Test>::put(threshold);

        let subnet_name: Vec<u8> = "submitted-slash-settlement".into();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            16,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let pool_balance = 1_000_000_u128;
        let validator_ids = (1..=TotalSubnetNodes::<Test>::get(subnet_id))
            .filter_map(|node_id| SubnetNodeValidatorId::<Test>::get(subnet_id, node_id))
            .collect::<BTreeSet<_>>();
        for validator_id in validator_ids {
            Network::handle_increase_account_validator_delegate_stake(
                &account(930),
                validator_id,
                pool_balance,
            )
            .expect("valid pool initialization must succeed");
        }
        seed_equal_validator_delegate_stake_for_subnet(subnet_id);

        let election_block = System::block_number();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        Network::elect_validator(subnet_id, subnet_epoch, election_block);
        let round = SubnetElectedValidator::<Test>::get(subnet_id, subnet_epoch).unwrap();
        let validator_id = round.validator_id;
        let pool_before = ValidatorDelegateStakeBalance::<Test>::get(validator_id);

        System::set_block_number(
            ValidatorDelegateStakeSlashLockUntil::<Test>::get(validator_id).saturating_add(5),
        );
        assert!(Network::is_validator_delegate_stake_slash_locked(
            validator_id
        ));

        let rejected_submission = ConsensusSubmissionData::<Test> {
            policy: round.policy,
            validator_subnet_node_id: round.validator_subnet_node_id,
            validator_node_stake_balance: round.validator_node_stake_balance,
            validator_delegate_stake_balance: round.validator_delegate_stake_balance,
            validator_epoch_progress: 0,
            validator_reward_factor: 0,
            attestation_ratio: 0,
            identity_attestation_ratio: 0,
            identity_attestation_count: 0,
            eligible_validator_identity_count: TotalSubnetNodes::<Test>::get(subnet_id),
            weight_sum: 0,
            data_length: 0,
            data: Vec::new(),
            attests: Default::default(),
            subnet_nodes: Vec::new(),
            prioritize_queue_node_id: None,
            remove_queue_node_id: None,
            emergency: round.emergency,
        };

        Network::distribute_rewards(
            &mut WeightMeter::new(),
            subnet_id,
            subnet_epoch + 1,
            rejected_submission.clone(),
            RewardsData::default(),
        );
        let pool_after = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        assert!(pool_after < pool_before);
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet_id, subnet_epoch),
            Some(true)
        );
        assert_eq!(
            ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
            0
        );
        assert!(!Network::is_validator_delegate_stake_slash_locked(
            validator_id
        ));

        Network::distribute_rewards(
            &mut WeightMeter::new(),
            subnet_id,
            subnet_epoch + 1,
            rejected_submission,
            RewardsData::default(),
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            pool_after
        );
    });
}

#[test]
fn finalized_zero_weight_submitted_round_settles_and_unlocks_exactly_once() {
    // Every representation of a definitive zero allocation must settle: a missing record when
    // the earlier allocation slot did not fit, an omitted subnet key, and explicit zero factors.
    for allocation_shape in 0..4 {
        new_test_ext().execute_with(|| {
            // A nonzero snapshotted base reward proves the zero-allocation branch suppresses every
            // reward, including the proposer credit that is otherwise independent of `RewardsData`.
            BaseValidatorReward::<Test>::put(123_456);
            BaseValidatorDelegateStakeSlashPercentage::<Test>::put(test_percent(1, 10));
            MaxValidatorDelegateStakeSlashAmount::<Test>::put(u128::MAX);
            ValidatorDelegateStakeSlashThreshold::<Test>::put(test_percent(1, 2));

            let subnet_name: Vec<u8> = "submitted-zero-weight-settlement".into();
            build_activated_subnet(
                subnet_name.clone(),
                0,
                16,
                10_000_000_000_000_000_000_000,
                MinSubnetMinStake::<Test>::get(),
            );
            let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
            let pool_balance = 1_000_000_u128;
            let validator_ids = (1..=TotalSubnetNodes::<Test>::get(subnet_id))
                .filter_map(|node_id| SubnetNodeValidatorId::<Test>::get(subnet_id, node_id))
                .collect::<BTreeSet<_>>();
            for validator_id in validator_ids {
                Network::handle_increase_account_validator_delegate_stake(
                    &account(935),
                    validator_id,
                    pool_balance,
                )
                .expect("valid pool initialization must succeed");
            }
            seed_equal_validator_delegate_stake_for_subnet(subnet_id);

            let election_block = System::block_number();
            let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
            Network::elect_validator(subnet_id, subnet_epoch, election_block);
            let round = SubnetElectedValidator::<Test>::get(subnet_id, subnet_epoch).unwrap();
            let validator_id = round.validator_id;
            assert_eq!(
                ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
                1,
                "election must retain its snapshotted validator-pool liability"
            );
            run_subnet_consensus_step_v2(subnet_id, None, None);
            assert_eq!(
                ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
                1,
                "submission alone must not release slash liability"
            );

            let emission_epoch =
                ConsensusRoundSettlementEmissionEpoch::<Test>::get(subnet_id, subnet_epoch)
                    .expect("a live election records its immutable funding epoch");
            match allocation_shape {
                0 => FinalSubnetEmissionWeights::<Test>::remove(emission_epoch),
                1 => FinalSubnetEmissionWeights::<Test>::insert(
                    emission_epoch,
                    DistributionData {
                        // A nonzero global budget proves that the absent subnet key is definitive.
                        subnets_emissions: u128::MAX,
                        subnet_weights: BTreeMap::new(),
                    },
                ),
                2 => FinalSubnetEmissionWeights::<Test>::insert(
                    emission_epoch,
                    DistributionData {
                        subnets_emissions: u128::MAX,
                        subnet_weights: BTreeMap::from([(subnet_id, 0)]),
                    },
                ),
                _ => FinalSubnetEmissionWeights::<Test>::insert(
                    emission_epoch,
                    DistributionData {
                        subnets_emissions: 0,
                        subnet_weights: BTreeMap::from([(
                            subnet_id,
                            Network::percentage_factor_as_u128(),
                        )]),
                    },
                ),
            }

            System::set_block_number(
                ValidatorDelegateStakeSlashLockUntil::<Test>::get(validator_id).saturating_add(5),
            );
            assert!(Network::is_validator_delegate_stake_slash_locked(
                validator_id
            ));

            let node_stake_before =
                NodeSubnetStake::<Test>::get(round.validator_subnet_node_id, subnet_id);
            let pool_before = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
            let total_stake_before = TotalStake::<Test>::get();
            let total_subnet_stake_before = TotalSubnetStake::<Test>::get(subnet_id);
            let total_validator_delegate_stake_before =
                TotalValidatorDelegateStakeBalance::<Test>::get();
            let total_subnet_delegate_stake_before =
                TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
            let issuance_before = Balances::total_issuance();

            Network::emission_settlement_step(
                &mut WeightMeter::new(),
                System::block_number(),
                emission_epoch,
                subnet_epoch.saturating_add(1),
                subnet_id,
            );

            assert_eq!(
                ConsensusRoundSettlementStatus::<Test>::get(subnet_id, subnet_epoch),
                Some(true)
            );
            assert_eq!(
                PendingConsensusRoundSettlementEpoch::<Test>::get(subnet_id),
                None
            );
            assert_eq!(
                ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::get(validator_id),
                0
            );
            assert!(!Network::is_validator_delegate_stake_slash_locked(
                validator_id
            ));
            assert_eq!(
                NodeSubnetStake::<Test>::get(round.validator_subnet_node_id, subnet_id),
                node_stake_before
            );
            assert_eq!(
                ValidatorDelegateStakeBalance::<Test>::get(validator_id),
                pool_before
            );
            assert_eq!(TotalStake::<Test>::get(), total_stake_before);
            assert_eq!(
                TotalSubnetStake::<Test>::get(subnet_id),
                total_subnet_stake_before
            );
            assert_eq!(
                TotalValidatorDelegateStakeBalance::<Test>::get(),
                total_validator_delegate_stake_before
            );
            assert_eq!(
                TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
                total_subnet_delegate_stake_before
            );
            assert_eq!(Balances::total_issuance(), issuance_before);

            // A retained submission cannot issue or release the same round twice.
            Network::emission_settlement_step(
                &mut WeightMeter::new(),
                System::block_number(),
                emission_epoch,
                subnet_epoch.saturating_add(1),
                subnet_id,
            );
            assert_eq!(
                NodeSubnetStake::<Test>::get(round.validator_subnet_node_id, subnet_id),
                node_stake_before
            );
            assert_eq!(
                ValidatorDelegateStakeBalance::<Test>::get(validator_id),
                pool_before
            );
            assert_eq!(TotalStake::<Test>::get(), total_stake_before);
            assert_eq!(Balances::total_issuance(), issuance_before);
        });
    }
}

#[test]
fn empty_emission_epoch_is_explicitly_finalized() {
    new_test_ext().execute_with(|| {
        let epoch = Network::get_current_epoch_as_u32();
        assert!(!FinalSubnetEmissionWeights::<Test>::contains_key(epoch));

        let _ = Network::handle_subnet_emission_weights(epoch);

        assert!(FinalSubnetEmissionWeights::<Test>::contains_key(epoch));
        assert_eq!(
            FinalSubnetEmissionWeights::<Test>::get(epoch),
            DistributionData::default()
        );
    });
}

#[test]
fn malformed_validator_delegate_reward_rate_cannot_overcredit_pool() {
    new_test_ext().execute_with(|| {
        let validator_id = 77;
        Network::handle_increase_account_validator_delegate_stake(
            &account(940),
            validator_id,
            1_000_000,
        )
        .expect("valid pool initialization must succeed");

        let pool_before = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let total_before = TotalValidatorDelegateStakeBalance::<Test>::get();
        let account_reward = 1_000;
        let malformed_rate = Network::percentage_factor_as_u128().saturating_mul(2);

        assert_eq!(
            Network::handle_validator_delegate_stake(
                &mut WeightMeter::new(),
                validator_id,
                malformed_rate,
                account_reward,
            ),
            Err(sp_runtime::ArithmeticError::Underflow.into())
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            pool_before
        );
        assert_eq!(
            TotalValidatorDelegateStakeBalance::<Test>::get(),
            total_before
        );

        // A corrupt aggregate must likewise fail rather than treating the failed pool credit as
        // an inactive pool and redirecting the full allocation to the node.
        TotalValidatorDelegateStakeBalance::<Test>::put(u128::MAX);
        assert_eq!(
            Network::handle_validator_delegate_stake(
                &mut WeightMeter::new(),
                validator_id,
                test_percent(1, 2),
                account_reward,
            ),
            Err(sp_runtime::ArithmeticError::Overflow.into())
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            pool_before
        );
        assert_eq!(TotalValidatorDelegateStakeBalance::<Test>::get(), u128::MAX);
    });
}

#[test]
fn test_validator_delegate_pool_lock_blocks_balance_changes_but_allows_share_transfer() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "delegate-slash-lock".into();
        let amount = 1_000_000_000_000_000_000_000_u128;
        build_activated_subnet_with_delegator_rewards(
            subnet_name.clone(),
            0,
            16,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
            DEFAULT_DELEGATE_REWARD_RATE,
        );

        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let validator_id = 1;
        let to_validator_id = 2;
        let delegator = account(900);
        let recipient = account(901);
        let incoming_delegator = account(902);
        let _ = Balances::deposit_creating(&delegator, amount + 500);
        let _ = Balances::deposit_creating(&incoming_delegator, amount + 500);

        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(delegator.clone()),
            validator_id,
            amount,
            1,
        ));
        let shares = AccountValidatorDelegateStakeShares::<Test>::get(&delegator, validator_id);
        assert!(shares > 4);

        System::set_block_number(System::block_number() + TxRateLimit::<Test>::get() + 1);
        let unlock_block = System::block_number() + EpochLength::get();
        ValidatorDelegateStakeSlashLockUntil::<Test>::insert(validator_id, unlock_block);

        assert_err!(
            Network::remove_validator_delegate_stake(
                RuntimeOrigin::signed(delegator.clone()),
                validator_id,
                shares / 4,
                1,
            ),
            Error::<Test>::ValidatorDelegateStakeSlashLocked
        );
        assert_err!(
            Network::swap_from_validator_to_validator(
                RuntimeOrigin::signed(delegator.clone()),
                validator_id,
                to_validator_id,
                shares / 4,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::ValidatorDelegateStakeSlashLocked
        );
        assert_err!(
            Network::swap_from_validator_to_subnet(
                RuntimeOrigin::signed(delegator.clone()),
                validator_id,
                subnet_id,
                shares / 4,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::ValidatorDelegateStakeSlashLocked
        );

        let locked_pool_balance = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        assert_err!(
            Network::add_validator_delegate_stake(
                RuntimeOrigin::signed(incoming_delegator.clone()),
                validator_id,
                amount,
                1,
            ),
            Error::<Test>::ValidatorDelegateStakeSlashLocked
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            locked_pool_balance
        );

        assert_ok!(Network::transfer_validator_delegate_stake(
            RuntimeOrigin::signed(delegator.clone()),
            validator_id,
            recipient.clone(),
            shares / 4,
        ));
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(recipient, validator_id),
            shares / 4
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            locked_pool_balance
        );

        // The settlement block itself is unlocked (`current_block < lock_until`).
        System::set_block_number(unlock_block);
        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(incoming_delegator),
            validator_id,
            amount,
            1,
        ));
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            locked_pool_balance + amount
        );
        assert_ok!(Network::remove_validator_delegate_stake(
            RuntimeOrigin::signed(delegator),
            validator_id,
            shares / 4,
            1,
        ));
    });
}

//
//
//
//
//
//
//
// Node delegate staking
//
//
//
//
//
//
//

#[test]
fn test_add_to_node_delegate_stake() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 100e+18 as u128;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet_with_delegator_rewards(
            subnet_name.clone(),
            0,
            16,
            deposit_amount,
            stake_amount,
            DEFAULT_DELEGATE_REWARD_RATE,
        );

        let validator_id = 1;

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let prev_account_node_delegate_stake_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(
                account(total_subnet_nodes + 1),
                validator_id,
            );

        assert_eq!(prev_account_node_delegate_stake_shares, 0);

        let prev_total_node_delegate_stake_shares =
            ValidatorDelegateStakeShares::<Test>::get(validator_id);

        assert_eq!(prev_total_node_delegate_stake_shares, 0);

        let prev_total_node_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        assert_eq!(prev_total_node_delegate_stake_balance, 0);

        let prev_total_node_delegate_stake = TotalValidatorDelegateStakeBalance::<Test>::get();
        assert_eq!(prev_total_node_delegate_stake, 0);

        let (node_delegate_stake_to_be_added_as_shares, gross_shares) =
            Network::preview_delegate_pool_deposit(
                amount,
                prev_total_node_delegate_stake_shares,
                prev_total_node_delegate_stake_balance,
                1,
            )
            .unwrap();

        let _ = Balances::deposit_creating(&account(total_subnet_nodes + 1), amount + 500);

        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
            validator_id,
            amount,
            1,
        ));

        // Ensure user shares changed and is expected
        let account_node_delegate_stake_shares = AccountValidatorDelegateStakeShares::<Test>::get(
            account(total_subnet_nodes + 1),
            validator_id,
        );
        assert_eq!(
            account_node_delegate_stake_shares,
            node_delegate_stake_to_be_added_as_shares
        );

        // Ensure node balance changed and is expected
        let total_node_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        assert_eq!(
            total_node_delegate_stake_balance,
            prev_total_node_delegate_stake_balance + amount
        );

        // Ensure network balance changed and is expected
        let total_node_delegate_stake = TotalValidatorDelegateStakeBalance::<Test>::get();
        assert_eq!(
            total_node_delegate_stake,
            prev_total_node_delegate_stake + amount
        );

        let total_node_delegate_stake_shares =
            ValidatorDelegateStakeShares::<Test>::get(validator_id);
        assert_eq!(
            total_node_delegate_stake_shares,
            prev_total_node_delegate_stake_shares + gross_shares
        );
        assert_eq!(
            total_node_delegate_stake_shares - account_node_delegate_stake_shares,
            Network::DELEGATE_POOL_MIN_LIQUIDITY
        );

        let account_node_delegate_stake_balance = Network::convert_to_balance(
            account_node_delegate_stake_shares,
            total_node_delegate_stake_shares,
            total_node_delegate_stake_balance,
        );

        // Ensure user balance changed and is expected
        assert!(
            (account_node_delegate_stake_balance
                >= Network::percent_mul(amount, test_percent(99, 100)))
                && (account_node_delegate_stake_balance <= amount)
        );
    })
}

#[test]
fn test_add_validator_delegate_stake_respects_tx_rate_limit() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "validator-delegate-rate-limit".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 100e+18 as u128;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet_with_delegator_rewards(
            subnet_name,
            0,
            16,
            deposit_amount,
            stake_amount,
            DEFAULT_DELEGATE_REWARD_RATE,
        );

        let validator_id = 1;
        let delegator = account(901);
        let rate_limit = 3;
        TxRateLimit::<Test>::put(rate_limit);
        System::set_block_number(1);
        let _ = Balances::deposit_creating(&delegator, amount.saturating_mul(3) + 500);

        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(delegator.clone()),
            validator_id,
            amount,
            1,
        ));
        let shares_after_first =
            AccountValidatorDelegateStakeShares::<Test>::get(delegator.clone(), validator_id);
        let balance_after_first = ValidatorDelegateStakeBalance::<Test>::get(validator_id);

        assert_err!(
            Network::add_validator_delegate_stake(
                RuntimeOrigin::signed(delegator.clone()),
                validator_id,
                amount,
                1,
            ),
            Error::<Test>::TxRateLimitExceeded
        );
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(delegator.clone(), validator_id),
            shares_after_first
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            balance_after_first
        );

        System::set_block_number(System::block_number() + rate_limit + 1);
        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(delegator.clone()),
            validator_id,
            amount,
            1,
        ));
        assert!(
            AccountValidatorDelegateStakeShares::<Test>::get(delegator, validator_id)
                > shares_after_first
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            balance_after_first + amount
        );
    })
}

#[test]
fn test_validator_delegate_stake_minimum_outputs_are_atomic() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "validator-minimum-output".into();
        let amount = 1_000_000_000_000_000_000_000_u128;
        build_activated_subnet_with_delegator_rewards(
            subnet_name,
            0,
            16,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
            DEFAULT_DELEGATE_REWARD_RATE,
        );

        let validator_id = 1;
        let delegator = account(904);
        let _ = Balances::deposit_creating(&delegator, amount + 500);
        let wallet_before = Balances::free_balance(&delegator);
        let (expected_shares, expected_gross_shares) =
            Network::preview_delegate_pool_deposit(amount, 0, 0, 1).unwrap();

        assert_err!(
            Network::add_validator_delegate_stake(
                RuntimeOrigin::signed(delegator.clone()),
                validator_id,
                amount,
                0,
            ),
            Error::<Test>::InvalidStakeMinimumOutput
        );
        assert_err!(
            Network::add_validator_delegate_stake(
                RuntimeOrigin::signed(delegator.clone()),
                validator_id,
                amount,
                expected_shares + 1,
            ),
            Error::<Test>::StakeSlippageExceeded
        );
        assert_eq!(Balances::free_balance(&delegator), wallet_before);
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&delegator, validator_id),
            0
        );
        assert_eq!(ValidatorDelegateStakeBalance::<Test>::get(validator_id), 0);
        assert_eq!(ValidatorDelegateStakeShares::<Test>::get(validator_id), 0);

        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(delegator.clone()),
            validator_id,
            amount,
            expected_shares,
        ));
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&delegator, validator_id),
            expected_shares
        );
        assert_eq!(
            ValidatorDelegateStakeShares::<Test>::get(validator_id),
            expected_gross_shares
        );

        let expected_balance =
            Network::try_convert_to_balance(expected_shares, expected_gross_shares, amount)
                .unwrap();
        System::set_block_number(System::block_number() + TxRateLimit::<Test>::get() + 1);

        assert_err!(
            Network::remove_validator_delegate_stake(
                RuntimeOrigin::signed(delegator.clone()),
                validator_id,
                expected_shares,
                expected_balance + 1,
            ),
            Error::<Test>::StakeSlippageExceeded
        );
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&delegator, validator_id),
            expected_shares
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            amount
        );
        assert!(StakeUnbondingLedger::<Test>::get(&delegator).is_empty());

        assert_ok!(Network::remove_validator_delegate_stake(
            RuntimeOrigin::signed(delegator.clone()),
            validator_id,
            expected_shares,
            expected_balance,
        ));
        assert_eq!(
            StakeUnbondingLedger::<Test>::get(delegator)
                .values()
                .next()
                .unwrap()
                .network,
            expected_balance
        );
    });
}

#[test]
fn test_add_to_node_delegate_stake_min_node_delegate_stake_deposit_not_reached_error() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 100e+18 as u128;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet_with_delegator_rewards(
            subnet_name.clone(),
            0,
            16,
            deposit_amount,
            stake_amount,
            DEFAULT_DELEGATE_REWARD_RATE,
        );

        let validator_id = 1;

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let _ = Balances::deposit_creating(&account(total_subnet_nodes + 1), amount + 500);

        assert_err!(
            Network::add_validator_delegate_stake(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                validator_id,
                MinDelegateStakeDeposit::<Test>::get() - 1,
                1,
            ),
            Error::<Test>::MinDelegateStakeDepositNotReached
        );
    })
}

#[test]
fn test_add_to_node_delegate_stake_not_enough_balance_to_stake_error() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 100e+18 as u128;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet_with_delegator_rewards(
            subnet_name.clone(),
            0,
            16,
            deposit_amount,
            stake_amount,
            DEFAULT_DELEGATE_REWARD_RATE,
        );

        let validator_id = 1;

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let _ = Balances::deposit_creating(&account(total_subnet_nodes + 1), amount + 500);

        assert_err!(
            Network::add_validator_delegate_stake(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                validator_id,
                amount + 501,
                1,
            ),
            Error::<Test>::NotEnoughBalanceToStake
        );
    })
}

#[test]
fn test_add_to_node_delegate_stake_balance_withdraw_error() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 100e+18 as u128;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet_with_delegator_rewards(
            subnet_name.clone(),
            0,
            16,
            deposit_amount,
            stake_amount,
            DEFAULT_DELEGATE_REWARD_RATE,
        );

        let validator_id = 1;

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let _ = Balances::deposit_creating(&account(total_subnet_nodes + 1), amount + 500);

        assert_err!(
            Network::add_validator_delegate_stake(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                validator_id,
                amount + 499,
                1,
            ),
            Error::<Test>::BalanceWithdrawalError
        );
    })
}

#[test]
fn test_remove_validator_delegate_stake() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet_with_delegator_rewards(
            subnet_name.clone(),
            0,
            16,
            deposit_amount,
            stake_amount,
            DEFAULT_DELEGATE_REWARD_RATE,
        );

        let validator_id = 1;
        let to_validator_id = 2;

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        let subnet_node_id = 1;

        let _ = Balances::deposit_creating(&account(total_subnet_nodes + 1), amount + 500);

        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
            validator_id,
            amount,
            1,
        ));

        let account_node_delegate_stake_shares = AccountValidatorDelegateStakeShares::<Test>::get(
            account(total_subnet_nodes + 1),
            validator_id,
        );
        let total_node_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let total_node_delegate_stake_shares =
            ValidatorDelegateStakeShares::<Test>::get(validator_id);

        let account_node_delegate_stake_balance = Network::convert_to_balance(
            account_node_delegate_stake_shares,
            total_node_delegate_stake_shares,
            total_node_delegate_stake_balance,
        );

        assert!(
            (account_node_delegate_stake_balance
                >= Network::percent_mul(amount, test_percent(99, 100)))
                && (account_node_delegate_stake_balance <= amount)
        );

        let account_node_delegate_stake_shares_to_be_removed =
            account_node_delegate_stake_shares / 2;

        let expected_balance_to_be_removed = Network::convert_to_balance(
            account_node_delegate_stake_shares_to_be_removed,
            total_node_delegate_stake_shares,
            total_node_delegate_stake_balance,
        );

        let expected_post_balance = Network::convert_to_balance(
            account_node_delegate_stake_shares_to_be_removed,
            total_node_delegate_stake_shares - account_node_delegate_stake_shares_to_be_removed,
            total_node_delegate_stake_balance - expected_balance_to_be_removed,
        );

        let epoch = System::block_number() / EpochLength::get();
        // Validator-pool stake has its own cooldown. Keep the subnet-pool value deliberately
        // different so this regression cannot pass by accidentally reading the wrong setting.
        DelegateStakeCooldownEpochs::<Test>::put(2);
        NodeDelegateStakeCooldownEpochs::<Test>::put(7);
        let block = System::block_number();

        assert_ok!(Network::remove_validator_delegate_stake(
            RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
            validator_id,
            account_node_delegate_stake_shares_to_be_removed,
            1,
        ));

        assert_err!(
            Network::remove_validator_delegate_stake(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                validator_id,
                0,
                1,
            ),
            Error::<Test>::SharesZero
        );

        assert_err!(
            Network::swap_from_validator_to_validator(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                validator_id, // unstaking from validator 1
                to_validator_id,
                0,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::SharesZero
        );

        assert_err!(
            Network::swap_from_validator_to_subnet(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                to_validator_id,
                subnet_id,
                0,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::SharesZero
        );

        let account_node_delegate_stake_shares = AccountValidatorDelegateStakeShares::<Test>::get(
            account(total_subnet_nodes + 1),
            validator_id,
        );
        let total_node_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let total_node_delegate_stake_shares =
            ValidatorDelegateStakeShares::<Test>::get(validator_id);

        assert_eq!(
            account_node_delegate_stake_shares,
            account_node_delegate_stake_shares_to_be_removed
        );

        let post_account_node_delegate_stake_balance = Network::convert_to_balance(
            account_node_delegate_stake_shares,
            total_node_delegate_stake_shares,
            total_node_delegate_stake_balance,
        );

        // Ensure expected balance
        assert_eq!(
            expected_post_balance,
            post_account_node_delegate_stake_balance
        );

        // Ensure stake in ledger
        let unbondings = StakeUnbondingLedger::<Test>::get(account(total_subnet_nodes + 1));
        assert_eq!(unbondings.len(), 1);
        let (ledger_block, ledger_balance) = unbondings.iter().next().unwrap();
        assert_eq!(
            *ledger_block,
            &block + NodeDelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );
        assert_eq!(ledger_balance.network, expected_balance_to_be_removed);
        assert_eq!(ledger_balance.overwatch, 0);
    })
}

#[test]
fn test_remove_validator_delegate_stake_not_enough_stake_to_withdraw() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet_with_delegator_rewards(
            subnet_name.clone(),
            0,
            16,
            deposit_amount,
            stake_amount,
            DEFAULT_DELEGATE_REWARD_RATE,
        );

        let validator_id: u32 = 1;
        let to_validator_id: u32 = 2;

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);
        let subnet_node_id = 1;

        assert_err!(
            Network::remove_validator_delegate_stake(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                validator_id,
                amount,
                1,
            ),
            Error::<Test>::NotEnoughStakeToWithdraw
        );

        assert_err!(
            Network::swap_from_validator_to_validator(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                validator_id,
                to_validator_id,
                amount,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::NotEnoughStakeToWithdraw
        );

        assert_err!(
            Network::swap_from_validator_to_subnet(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                validator_id,
                subnet_id,
                amount,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::NotEnoughStakeToWithdraw
        );
    })
}

#[test]
fn test_swap_validator_delegate_stake() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();

        let from_account_n = max_subnet_nodes + 1 * subnets;

        build_activated_subnet_with_delegator_rewards(
            subnet_name.clone(),
            0,
            16,
            deposit_amount,
            stake_amount,
            DEFAULT_DELEGATE_REWARD_RATE,
        );

        let validator_id = 1;
        let to_validator_id = 2;
        let starting_to_validator_id = to_validator_id;

        let from_subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_from_subnet_nodes = TotalSubnetNodes::<Test>::get(from_subnet_id);

        let to_subnet_name: Vec<u8> = "to-subnet-name".into();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;

        let to_account_n = max_subnet_nodes + 1 * subnets;

        build_activated_subnet_with_delegator_rewards(
            to_subnet_name.clone(),
            0,
            16,
            deposit_amount,
            stake_amount,
            DEFAULT_DELEGATE_REWARD_RATE,
        );

        let to_subnet_id = SubnetName::<Test>::get(to_subnet_name.clone()).unwrap();

        let _ = Balances::deposit_creating(&account(total_from_subnet_nodes + 1), amount + 500);

        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(account(total_from_subnet_nodes + 1)),
            validator_id,
            amount,
            1,
        ));

        let account_node_delegate_stake_shares = AccountValidatorDelegateStakeShares::<Test>::get(
            account(total_from_subnet_nodes + 1),
            validator_id,
        );
        let total_node_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let total_node_delegate_stake_shares =
            ValidatorDelegateStakeShares::<Test>::get(validator_id);

        let account_node_delegate_stake_balance = Network::convert_to_balance(
            account_node_delegate_stake_shares,
            total_node_delegate_stake_shares,
            total_node_delegate_stake_balance,
        );

        assert!(
            (account_node_delegate_stake_balance
                >= Network::percent_mul(amount, test_percent(99, 100)))
                && (account_node_delegate_stake_balance <= amount)
        );

        let account_node_delegate_stake_shares_to_be_removed =
            account_node_delegate_stake_shares / 2;
        let expected_node_delegate_stake_shares_balance =
            account_node_delegate_stake_shares - account_node_delegate_stake_shares_to_be_removed;

        // Get expected balance to be removed from subnet node 1
        let expected_balance_to_be_removed = Network::convert_to_balance(
            account_node_delegate_stake_shares_to_be_removed,
            total_node_delegate_stake_shares,
            total_node_delegate_stake_balance,
        );

        // Get expected balance after removal from subnet node 1
        let expected_post_balance = Network::convert_to_balance(
            account_node_delegate_stake_shares - account_node_delegate_stake_shares_to_be_removed,
            total_node_delegate_stake_shares - account_node_delegate_stake_shares_to_be_removed,
            total_node_delegate_stake_balance - expected_balance_to_be_removed,
        );
        let unbondings = StakeUnbondingLedger::<Test>::get(account(total_from_subnet_nodes + 1));
        assert_eq!(unbondings.len(), 0);

        let pre_transfer_balance = Balances::free_balance(&account(total_from_subnet_nodes + 1));
        let prev_next_id = NextSwapQueueId::<Test>::get();

        let starting_to_subnet_node_id = 2;

        assert_ok!(Network::swap_from_validator_to_validator(
            RuntimeOrigin::signed(account(total_from_subnet_nodes + 1)),
            validator_id, // unstaking from node 1
            to_validator_id,
            account_node_delegate_stake_shares_to_be_removed,
            1,
            1,
            u32::MAX,
        ));

        let post_transfer_balance = Balances::free_balance(&account(total_from_subnet_nodes + 1));
        assert_eq!(pre_transfer_balance, post_transfer_balance);

        //
        // from subnet ID and Subnet node 1
        // Get accounts delegate stake info from staking to node 1 (now removed partial)
        //
        let account_node_delegate_stake_shares = AccountValidatorDelegateStakeShares::<Test>::get(
            account(total_from_subnet_nodes + 1),
            validator_id,
        );
        let total_node_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let total_node_delegate_stake_shares =
            ValidatorDelegateStakeShares::<Test>::get(validator_id);

        let account_node_delegate_stake_balance = Network::convert_to_balance(
            account_node_delegate_stake_shares,
            total_node_delegate_stake_shares,
            total_node_delegate_stake_balance,
        );

        assert_eq!(account_node_delegate_stake_balance, expected_post_balance);

        //
        // Check queue
        //
        let starting_to_subnet_id = to_subnet_id;
        let call_queue = SwapCallQueue::<Test>::get(prev_next_id);
        assert_eq!(call_queue.clone().unwrap().id, prev_next_id);
        match &call_queue.clone().unwrap().call {
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id,
                to_subnet_id,
                balance,
                ..
            } => assert!(false),
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id,
                to_validator_id,
                balance,
                ..
            } => {
                assert_eq!(*account_id, account(total_from_subnet_nodes + 1));
                assert_eq!(*to_validator_id, starting_to_validator_id);
                assert_ne!(*balance, 0);
            }
        };

        let next_id = NextSwapQueueId::<Test>::get();
        assert_eq!(prev_next_id + 1, next_id);
        let queue = SwapQueueOrder::<Test>::get();
        assert!(queue
            .first()
            .map_or(false, |&first_id| first_id == prev_next_id));
    })
}

#[test]
fn test_transfer_validator_delegate_stake() {
    new_test_ext().execute_with(|| {
        let _ = env_logger::builder().is_test(true).try_init();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnet_name: Vec<u8> = "subnet-name".into();
        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let subnet_node_id = 1;
        let validator_id = 1;

        let n_account = 1000;
        let to_n_account = 1001;

        let _ = Balances::deposit_creating(&account(n_account), amount + 500);

        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            validator_id,
            amount,
            1,
        ));

        let n_account_balance = Balances::free_balance(&account(n_account));
        let to_n_account_balance = Balances::free_balance(&account(to_n_account));

        let account_node_delegate_stake_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(account(n_account), validator_id);
        let total_node_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let total_node_delegate_stake_shares =
            ValidatorDelegateStakeShares::<Test>::get(validator_id);

        let account_node_delegate_stake_balance = Network::convert_to_balance(
            account_node_delegate_stake_shares,
            total_node_delegate_stake_shares,
            total_node_delegate_stake_balance,
        );

        assert!(
            (account_node_delegate_stake_balance
                >= Network::percent_mul(amount, test_percent(99, 100)))
                && (account_node_delegate_stake_balance <= amount)
        );

        let to_delegate_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(account(to_n_account), validator_id);

        assert_eq!(to_delegate_shares, 0);

        assert_ok!(Network::transfer_validator_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            validator_id,
            account(to_n_account),
            account_node_delegate_stake_shares,
        ));

        // no changes to balance
        let after_n_account_balance = Balances::free_balance(&account(n_account));
        assert_eq!(n_account_balance, after_n_account_balance);
        let after_to_n_account_balance = Balances::free_balance(&account(to_n_account));
        assert_eq!(to_n_account_balance, after_to_n_account_balance);

        // no ledger balances
        let n_account_unbondings = StakeUnbondingLedger::<Test>::get(account(n_account));
        assert_eq!(n_account_unbondings.len(), 0);
        let to_n_account_unbondings = StakeUnbondingLedger::<Test>::get(account(to_n_account));
        assert_eq!(to_n_account_unbondings.len(), 0);

        // from shares
        let after_delegate_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(account(n_account), validator_id);

        // to shares
        let after_to_node_delegate_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(account(to_n_account), validator_id);

        let after_total_node_delegate_stake_shares =
            ValidatorDelegateStakeShares::<Test>::get(validator_id);
        let after_total_node_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(validator_id);

        assert_eq!(after_delegate_shares, 0);
        assert_eq!(
            account_node_delegate_stake_shares,
            after_to_node_delegate_shares
        );
        assert_eq!(
            total_node_delegate_stake_shares,
            after_total_node_delegate_stake_shares
        );
        assert_eq!(
            total_node_delegate_stake_balance,
            after_total_node_delegate_stake_balance
        );
    });
}

#[test]
fn test_transfer_validator_delegate_stake_partial_balance() {
    new_test_ext().execute_with(|| {
        let _ = env_logger::builder().is_test(true).try_init();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnet_name: Vec<u8> = "subnet-name".into();
        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let subnet_node_id = 1;
        let validator_id = 1;

        let n_account = 1000;
        let to_n_account = 1001;

        let _ = Balances::deposit_creating(&account(n_account), amount + 500);

        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            validator_id,
            amount,
            1,
        ));

        let n_account_balance = Balances::free_balance(&account(n_account));
        let to_n_account_balance = Balances::free_balance(&account(to_n_account));

        let account_node_delegate_stake_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(account(n_account), validator_id);
        let total_node_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let total_node_delegate_stake_shares =
            ValidatorDelegateStakeShares::<Test>::get(validator_id);

        let account_node_delegate_stake_balance = Network::convert_to_balance(
            account_node_delegate_stake_shares,
            total_node_delegate_stake_shares,
            total_node_delegate_stake_balance,
        );

        assert!(
            (account_node_delegate_stake_balance
                >= Network::percent_mul(amount, test_percent(99, 100)))
                && (account_node_delegate_stake_balance <= amount)
        );

        let to_delegate_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(account(to_n_account), validator_id);

        assert_eq!(to_delegate_shares, 0);

        let shares_to_transfer = account_node_delegate_stake_shares / 2;

        assert_ok!(Network::transfer_validator_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            validator_id,
            account(to_n_account),
            shares_to_transfer,
        ));

        // no changes to balance
        let after_n_account_balance = Balances::free_balance(&account(n_account));
        assert_eq!(n_account_balance, after_n_account_balance);
        let after_to_n_account_balance = Balances::free_balance(&account(to_n_account));
        assert_eq!(to_n_account_balance, after_to_n_account_balance);

        // no ledger balances
        let n_account_unbondings = StakeUnbondingLedger::<Test>::get(account(n_account));
        assert_eq!(n_account_unbondings.len(), 0);
        let to_n_account_unbondings = StakeUnbondingLedger::<Test>::get(account(to_n_account));
        assert_eq!(to_n_account_unbondings.len(), 0);

        // from shares
        let after_delegate_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(account(n_account), validator_id);

        // to shares
        let after_to_node_delegate_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(account(to_n_account), validator_id);

        let after_total_node_delegate_stake_shares =
            ValidatorDelegateStakeShares::<Test>::get(validator_id);
        let after_total_node_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(validator_id);

        assert_eq!(
            after_delegate_shares,
            account_node_delegate_stake_shares - shares_to_transfer
        );
        assert_eq!(shares_to_transfer, after_to_node_delegate_shares);
        assert_eq!(
            total_node_delegate_stake_shares,
            after_total_node_delegate_stake_shares
        );
        assert_eq!(
            total_node_delegate_stake_balance,
            after_total_node_delegate_stake_balance
        );
    });
}

#[test]
fn test_transfer_validator_delegate_stake_requires_owned_shares() {
    new_test_ext().execute_with(|| {
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnet_name: Vec<u8> = "subnet-name".into();
        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);
        let validator_id = 1;

        let staker = account(1000);
        let attacker = account(1001);
        let recipient = account(1002);

        let _ = Balances::deposit_creating(&staker, amount + 500);

        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(staker.clone()),
            validator_id,
            amount,
            1,
        ));

        let staker_shares = AccountValidatorDelegateStakeShares::<Test>::get(&staker, validator_id);
        let total_shares = ValidatorDelegateStakeShares::<Test>::get(validator_id);
        let total_balance = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        assert_ne!(staker_shares, 0);
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&attacker, validator_id),
            0
        );
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&recipient, validator_id),
            0
        );

        assert_err!(
            Network::transfer_validator_delegate_stake(
                RuntimeOrigin::signed(attacker.clone()),
                validator_id,
                recipient.clone(),
                staker_shares,
            ),
            Error::<Test>::NotEnoughStakeToWithdraw
        );

        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&staker, validator_id),
            staker_shares
        );
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&attacker, validator_id),
            0
        );
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&recipient, validator_id),
            0
        );
        assert_eq!(
            ValidatorDelegateStakeShares::<Test>::get(validator_id),
            total_shares
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            total_balance
        );
    });
}

#[test]
fn test_inflation_exploit_mitigation_dead_shares() {
    new_test_ext().execute_with(|| {
        let validator_id = 1;
        let first_user = account(1);
        let second_user = account(2);
        let stake = 1_000_000_000_000;

        let _ = Balances::deposit_creating(&first_user, stake * 10);
        let _ = Balances::deposit_creating(&second_user, stake * 10);

        let (expected_first_user_shares, expected_first_gross_shares) =
            Network::preview_delegate_pool_deposit(stake, 0, 0, 1).unwrap();

        assert_ok!(Network::do_add_validator_delegate_stake(
            RuntimeOrigin::signed(first_user.clone()),
            validator_id,
            stake,
            1,
        ));

        let first_user_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(&first_user, validator_id);
        let total_shares_after_first = ValidatorDelegateStakeShares::<Test>::get(validator_id);

        assert_eq!(first_user_shares, expected_first_user_shares);
        assert_eq!(total_shares_after_first, expected_first_gross_shares);
        assert_eq!(
            total_shares_after_first - first_user_shares,
            Network::DELEGATE_POOL_MIN_LIQUIDITY
        );

        let balance_after_first = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let (expected_second_user_shares, expected_second_gross_shares) =
            Network::preview_delegate_pool_deposit(
                stake,
                total_shares_after_first,
                balance_after_first,
                1,
            )
            .unwrap();

        assert_ok!(Network::do_add_validator_delegate_stake(
            RuntimeOrigin::signed(second_user.clone()),
            validator_id,
            stake,
            1,
        ));

        let second_user_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(&second_user, validator_id);
        let total_shares_after_both = ValidatorDelegateStakeShares::<Test>::get(validator_id);
        let total_balance_after_both = ValidatorDelegateStakeBalance::<Test>::get(validator_id);

        assert_eq!(second_user_shares, expected_second_user_shares);
        assert!(first_user_shares <= second_user_shares);
        assert_eq!(
            total_shares_after_both,
            expected_first_gross_shares + expected_second_gross_shares
        );
        assert_eq!(
            total_shares_after_both,
            first_user_shares + second_user_shares + Network::DELEGATE_POOL_MIN_LIQUIDITY
        );

        let first_user_balance = Network::convert_to_balance(
            first_user_shares,
            total_shares_after_both,
            total_balance_after_both,
        );

        let second_user_balance = Network::convert_to_balance(
            second_user_shares,
            total_shares_after_both,
            total_balance_after_both,
        );

        assert!(first_user_balance <= stake);
        assert!(second_user_balance <= stake);
        assert!(first_user_balance < second_user_balance);
    });
}

#[test]
fn test_internal_validator_reward_credit_increases_share_value_without_minting() {
    new_test_ext().execute_with(|| {
        let validator_id = 1;
        let attacker = account(1);
        let initial_balance = 1_000_000;
        let stake_amount = 100_000;
        let reward_amount = 100_000;

        // Step 0: Fund attacker
        Balances::make_free_balance_be(&attacker, initial_balance);

        // Step 1: Attacker stakes
        assert_ok!(Network::do_add_validator_delegate_stake(
            RuntimeOrigin::signed(attacker.clone()),
            validator_id,
            stake_amount,
            1,
        ));

        let shares_before =
            AccountValidatorDelegateStakeShares::<Test>::get(&attacker, validator_id);
        let shares_total_before = ValidatorDelegateStakeShares::<Test>::get(validator_id);
        let pool_balance_before = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        assert!(shares_before > 0);
        assert!(shares_total_before > 0);
        assert!(pool_balance_before > 0);

        // Protocol rewards increase assets without minting shares.
        assert_ok!(Network::do_increase_validator_delegate_stake(
            validator_id,
            reward_amount
        ));

        // Step 3: Check that no new shares were minted
        let shares_after_reward =
            AccountValidatorDelegateStakeShares::<Test>::get(&attacker, validator_id);
        let shares_total_after_reward = ValidatorDelegateStakeShares::<Test>::get(validator_id);
        let pool_balance_after = ValidatorDelegateStakeBalance::<Test>::get(validator_id);

        assert_eq!(shares_after_reward, shares_before);
        assert_eq!(shares_total_after_reward, shares_total_before);
        assert_eq!(pool_balance_after, pool_balance_before + reward_amount);

        let expected_redeemed_balance = Network::try_convert_to_balance(
            shares_after_reward,
            shares_total_after_reward,
            pool_balance_after,
        )
        .unwrap();
        assert!(expected_redeemed_balance > stake_amount);
        assert!(expected_redeemed_balance <= stake_amount + reward_amount);

        assert_ok!(Network::do_remove_validator_delegate_stake(
            RuntimeOrigin::signed(attacker.clone()),
            validator_id,
            shares_after_reward,
            1,
        ));

        let unbondings = StakeUnbondingLedger::<Test>::get(attacker);
        assert_eq!(unbondings.len(), 1);
        assert_eq!(
            unbondings.values().next().unwrap().network,
            expected_redeemed_balance
        );
    });
}

#[test]
fn test_validator_virtual_offset_limits_internal_balance_jump_rounding_loss() {
    new_test_ext().execute_with(|| {
        let _ = env_logger::builder().is_test(true).try_init();

        let validator_id = 1;

        let attacker = account(1);
        let victim = account(2);

        const ATTACKER_INITIAL_TOKENS: u128 = 10000000;
        const ATTACKER_INITIAL_DEPOSIT: u128 = 1000;
        const INTERNAL_REWARD: u128 = 9999000;
        const VICTIM_DEPOSIT: u128 = 1000000;

        Balances::make_free_balance_be(&attacker, ATTACKER_INITIAL_TOKENS);
        Balances::make_free_balance_be(&victim, VICTIM_DEPOSIT + 500);

        let (expected_attacker_shares, expected_gross_shares) =
            Network::preview_delegate_pool_deposit(ATTACKER_INITIAL_DEPOSIT, 0, 0, 1).unwrap();

        assert_ok!(Network::do_add_validator_delegate_stake(
            RuntimeOrigin::signed(attacker.clone()),
            validator_id,
            ATTACKER_INITIAL_DEPOSIT,
            1,
        ));

        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&attacker, validator_id),
            expected_attacker_shares
        );
        assert_eq!(
            ValidatorDelegateStakeShares::<Test>::get(validator_id),
            expected_gross_shares
        );
        assert_eq!(
            expected_gross_shares - expected_attacker_shares,
            Network::DELEGATE_POOL_MIN_LIQUIDITY
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            ATTACKER_INITIAL_DEPOSIT
        );

        // Simulate the protocol crediting a large reward between deposits. The public donation
        // call is intentionally absent; this exercises the internal reward path only.
        assert_ok!(Network::do_increase_validator_delegate_stake(
            validator_id,
            INTERNAL_REWARD
        ));

        let shares_before_victim = ValidatorDelegateStakeShares::<Test>::get(validator_id);
        let balance_before_victim = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let (expected_victim_shares, expected_victim_gross_shares) =
            Network::preview_delegate_pool_deposit(
                VICTIM_DEPOSIT,
                shares_before_victim,
                balance_before_victim,
                1,
            )
            .unwrap();

        assert_ok!(Network::do_add_validator_delegate_stake(
            RuntimeOrigin::signed(victim.clone()),
            validator_id,
            VICTIM_DEPOSIT,
            1,
        ));

        let victim_shares = AccountValidatorDelegateStakeShares::<Test>::get(&victim, validator_id);
        assert_eq!(victim_shares, expected_victim_shares);

        let total_subnet_delegate_stake_shares =
            ValidatorDelegateStakeShares::<Test>::get(validator_id);
        let total_subnet_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        assert_eq!(
            total_subnet_delegate_stake_shares,
            shares_before_victim + expected_victim_gross_shares
        );

        let victim_balance = Network::convert_to_balance(
            victim_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        assert!(
            (victim_balance >= Network::percent_mul(VICTIM_DEPOSIT, test_percent(99, 100)))
                && (victim_balance <= VICTIM_DEPOSIT)
        );

        let attacker_balance = Network::convert_to_balance(
            AccountValidatorDelegateStakeShares::<Test>::get(&attacker, validator_id),
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        assert!(attacker_balance < ATTACKER_INITIAL_DEPOSIT + INTERNAL_REWARD);

        assert_ok!(Network::do_remove_validator_delegate_stake(
            RuntimeOrigin::signed(attacker.clone()),
            validator_id,
            AccountValidatorDelegateStakeShares::<Test>::get(&attacker, validator_id),
            1,
        ));

        let attacker_final_balance = Balances::free_balance(&attacker);

        assert!(attacker_final_balance < ATTACKER_INITIAL_TOKENS);
    });
}

#[test]
fn test_swap_from_validator_to_subnet() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        log::error!("subnets count {:?}", subnets);
        build_activated_subnet_with_delegator_rewards(
            subnet_name.clone(),
            0,
            16,
            deposit_amount,
            stake_amount,
            DEFAULT_DELEGATE_REWARD_RATE,
        );

        let validator_id = 1;
        let from_validator_id = validator_id;

        let from_subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_from_subnet_nodes = TotalSubnetNodes::<Test>::get(from_subnet_id);

        let to_subnet_name: Vec<u8> = "subnet-name-2".into();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        log::error!("subnets count {:?}", subnets);

        build_activated_subnet_with_delegator_rewards(
            to_subnet_name.clone(),
            0,
            16,
            deposit_amount,
            stake_amount,
            DEFAULT_DELEGATE_REWARD_RATE,
        );

        let to_subnet_id = SubnetName::<Test>::get(to_subnet_name.clone()).unwrap();

        let _ = Balances::deposit_creating(&account(total_from_subnet_nodes + 1), amount + 500);

        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(account(total_from_subnet_nodes + 1)),
            validator_id,
            amount,
            1,
        ));

        let account_node_delegate_stake_shares = AccountValidatorDelegateStakeShares::<Test>::get(
            account(total_from_subnet_nodes + 1),
            validator_id,
        );
        let total_node_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let total_node_delegate_stake_shares =
            ValidatorDelegateStakeShares::<Test>::get(validator_id);

        let account_node_delegate_stake_balance = Network::convert_to_balance(
            account_node_delegate_stake_shares,
            total_node_delegate_stake_shares,
            total_node_delegate_stake_balance,
        );

        assert!(
            (account_node_delegate_stake_balance
                >= Network::percent_mul(amount, test_percent(99, 100)))
                && (account_node_delegate_stake_balance <= amount)
        );

        let account_node_delegate_stake_shares_to_be_removed =
            account_node_delegate_stake_shares / 2;

        let expected_balance_to_be_removed = Network::convert_to_balance(
            account_node_delegate_stake_shares_to_be_removed,
            total_node_delegate_stake_shares,
            total_node_delegate_stake_balance,
        );

        let before_transfer_tensor = Balances::free_balance(&account(total_from_subnet_nodes + 1));

        let unbondings = StakeUnbondingLedger::<Test>::get(account(total_from_subnet_nodes + 1));
        assert_eq!(unbondings.len(), 0);

        let prev_next_id = NextSwapQueueId::<Test>::get();

        assert_ok!(Network::swap_from_validator_to_subnet(
            RuntimeOrigin::signed(account(total_from_subnet_nodes + 1)),
            from_validator_id,
            to_subnet_id,
            account_node_delegate_stake_shares_to_be_removed,
            1,
            1,
            u32::MAX,
        ));

        let unbondings = StakeUnbondingLedger::<Test>::get(account(total_from_subnet_nodes + 1));
        assert_eq!(unbondings.len(), 0);

        let after_transfer_tensor = Balances::free_balance(&account(total_from_subnet_nodes + 1));
        assert_eq!(after_transfer_tensor, before_transfer_tensor);

        let from_delegate_shares = AccountSubnetDelegateStakeShares::<Test>::get(
            account(total_from_subnet_nodes + 1),
            from_subnet_id,
        );
        assert_eq!(from_delegate_shares, 0);

        let starting_to_subnet_id = to_subnet_id;
        let call_queue = SwapCallQueue::<Test>::get(prev_next_id);
        assert_eq!(call_queue.clone().unwrap().id, prev_next_id);
        match &call_queue.clone().unwrap().call {
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id,
                to_subnet_id,
                balance,
                ..
            } => {
                assert_eq!(*account_id, account(total_from_subnet_nodes + 1));
                assert_eq!(*to_subnet_id, starting_to_subnet_id);
                assert_ne!(*balance, 0);
            }
            QueuedSwapCall::SwapToValidatorDelegateStake { .. } => assert!(false),
        };

        let next_id = NextSwapQueueId::<Test>::get();
        assert_eq!(prev_next_id + 1, next_id);
        let queue = SwapQueueOrder::<Test>::get();
        assert!(queue
            .first()
            .map_or(false, |&first_id| first_id == prev_next_id));
    });
}
