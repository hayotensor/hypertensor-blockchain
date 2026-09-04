// Copyright (C) Hypertensor.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;
use frame_support::pallet_prelude::DispatchResultWithPostInfo;

impl<T: Config> Pallet<T> {
    pub fn do_pause() -> DispatchResult {
        if !TxPause::<T>::get() {
            OverwatchTxPauseStartBlock::<T>::put(Self::get_current_block_as_u32());
        }
        TxPause::<T>::put(true);
        Self::deposit_event(Event::SetTxPause());
        Ok(())
    }
    pub fn do_unpause() -> DispatchResult {
        if TxPause::<T>::get() {
            if let Some(pause_start_block) = OverwatchTxPauseStartBlock::<T>::take() {
                let pause_duration =
                    Self::get_current_block_as_u32().saturating_sub(pause_start_block);
                let resumed_start_block = OverwatchEpochStartBlock::<T>::mutate(|start_block| {
                    *start_block = (*start_block).saturating_add(pause_duration);
                    *start_block
                });
                Self::deposit_event(Event::OverwatchEpochResumed {
                    epoch: CurrentOverwatchEpoch::<T>::get(),
                    start_block: resumed_start_block,
                });
            }
        }
        TxPause::<T>::put(false);
        Self::deposit_event(Event::SetTxUnpause());
        Ok(())
    }
    pub fn do_set_subnet_owner_percentage(value: u128) -> DispatchResult {
        ensure!(
            value <= MAX_SUBNET_OWNER_PERCENTAGE,
            Error::<T>::InvalidPercent
        );

        SubnetOwnerPercentage::<T>::put(value);

        Self::deposit_event(Event::SetSubnetOwnerPercentage(value));

        Ok(())
    }
    pub fn do_set_max_subnets(value: u32) -> DispatchResult {
        // Account for designated general-chain work and the temporary rotation capacity.
        let available_slots = T::EpochLength::get()
            .checked_sub(T::DesignatedEpochSlots::get())
            .unwrap_or(0);
        let bounded_slots = available_slots.min(T::MaxPhysicalSubnetsUpperBound::get());
        ensure!(
            value <= bounded_slots.saturating_sub(SUBNET_ROTATION_ALLOWANCE),
            Error::<T>::InvalidMaxSubnets
        );

        MaxSubnets::<T>::set(value);

        Self::deposit_event(Event::SetMaxSubnets(value));

        Ok(())
    }
    pub fn do_set_max_bootnodes(value: u32) -> DispatchResult {
        ensure!(
            value <= T::MaxBootnodesUpperBound::get(),
            Error::<T>::InvalidMaxBootnodes
        );

        MaxBootnodes::<T>::set(value);

        Self::deposit_event(Event::SetMaxBootnodes(value));

        Ok(())
    }
    pub fn do_set_max_subnet_bootnodes_access(value: u32) -> DispatchResult {
        ensure!(
            value <= T::MaxSubnetBootnodeAccessUpperBound::get(),
            Error::<T>::InvalidMaxSubnetBootnodeAccess
        );

        MaxSubnetBootnodeAccess::<T>::set(value);

        Self::deposit_event(Event::SetMaxSubnetBootnodeAccess(value));

        Ok(())
    }
    pub fn do_set_max_pause_epochs(value: u32) -> DispatchResult {
        ensure!(value > 0, Error::<T>::InvalidMaxSubnetPauseEpochs);

        MaxSubnetPauseEpochs::<T>::set(value);

        Self::deposit_event(Event::SetMaxSubnetPauseEpochs(value));

        Ok(())
    }
    pub fn do_set_min_registration_cost(value: u128) -> DispatchResult {
        MinRegistrationCost::<T>::set(value);

        Self::deposit_event(Event::SetMinRegistrationCost(value));

        Ok(())
    }
    pub fn do_set_registration_cost_delay_blocks(value: u32) -> DispatchResult {
        RegistrationCostDecayBlocks::<T>::set(value);

        Self::deposit_event(Event::SetRegistrationCostDecayBlocks(value));

        Ok(())
    }
    pub fn do_set_registration_cost_alpha(value: u128) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );
        RegistrationCostAlpha::<T>::set(value);

        Self::deposit_event(Event::SetRegistrationCostAlpha(value));

        Ok(())
    }
    pub fn do_set_new_registration_cost_multiplier(value: u128) -> DispatchResult {
        NewRegistrationCostMultiplier::<T>::set(value);

        Self::deposit_event(Event::SetNewRegistrationCostMultiplier(value));

        Ok(())
    }
    pub fn do_set_min_subnet_delegate_stake_balance(value: u128) -> DispatchResult {
        MinSubnetDelegateStakeBalance::<T>::set(value);

        Self::deposit_event(Event::SetMinSubnetDelegateStakeBalance(value));

        Ok(())
    }

    pub fn do_set_churn_limits(min: u32, max: u32) -> DispatchResult {
        ensure!(
            min < max && max <= T::MaxChurnLimitUpperBound::get(),
            Error::<T>::InvalidValues
        );

        MinChurnLimit::<T>::set(min);
        MaxChurnLimit::<T>::set(max);

        Self::deposit_event(Event::SetChurnLimits(min, max));

        Ok(())
    }

    pub fn do_set_churn_limit_multipliers(min: u32, max: u32) -> DispatchResult {
        ensure!(
            min >= MIN_CHURN_LIMIT_MULTIPLIER && min < max,
            Error::<T>::InvalidValues
        );

        MinChurnLimitMultiplier::<T>::set(min);
        MaxChurnLimitMultiplier::<T>::set(max);

        Self::deposit_event(Event::SetChurnLimitMultipliers(min, max));

        Ok(())
    }

    pub fn do_set_queue_epochs(min: u32, max: u32) -> DispatchResult {
        ensure!(min < max, Error::<T>::InvalidValues);

        MinQueueEpochs::<T>::set(min);
        MaxQueueEpochs::<T>::set(max);

        Self::deposit_event(Event::SetQueueEpochs(min, max));

        Ok(())
    }
    pub fn do_set_min_idle_classification_epochs(value: u32) -> DispatchResult {
        MinIdleClassificationEpochs::<T>::set(value);

        Self::deposit_event(Event::SetMinIdleClassificationEpochs(value));

        Ok(())
    }
    pub fn do_set_max_idle_classification_epochs(value: u32) -> DispatchResult {
        MaxIdleClassificationEpochs::<T>::set(value);

        Self::deposit_event(Event::SetMaxIdleClassificationEpochs(value));

        Ok(())
    }
    pub fn do_set_subnet_activation_enactment_epochs(value: u32) -> DispatchResult {
        SubnetEnactmentEpochs::<T>::set(value);

        Self::deposit_event(Event::SetSubnetEnactmentEpochs(value));

        Ok(())
    }
    pub fn do_set_included_classification_epochs(min: u32, max: u32) -> DispatchResult {
        ensure!(min < max, Error::<T>::InvalidValues);

        MinIncludedClassificationEpochs::<T>::set(min);
        MaxIncludedClassificationEpochs::<T>::set(max);

        Self::deposit_event(Event::SetIncludedClassificationEpochs(min, max));

        Ok(())
    }
    pub fn do_set_subnet_stakes(min: u128, max: u128) -> DispatchResult {
        ensure!(min < max, Error::<T>::InvalidValues);

        MinSubnetMinStake::<T>::set(min);
        MaxSubnetMinStake::<T>::set(max);

        Self::deposit_event(Event::SetSubnetStakesLimits(min, max));

        Ok(())
    }
    pub fn do_set_delegate_stake_percentages(min: u128, max: u128) -> DispatchResult {
        ensure!(min < max, Error::<T>::InvalidValues);

        ensure!(
            max <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        MinDelegateStakePercentage::<T>::set(min);
        MaxDelegateStakePercentage::<T>::set(max);

        Self::deposit_event(Event::SetDelegateStakePercentages(min, max));

        Ok(())
    }
    pub fn do_set_min_max_registered_nodes(min: u32, max: u32) -> DispatchResult {
        ensure!(min < max, Error::<T>::InvalidValues);
        ensure!(
            max <= T::MaxRegisteredNodesUpperBound::get(),
            Error::<T>::InvalidMaxRegisteredNodes
        );

        MinMaxRegisteredNodes::<T>::set(min);
        MaxMaxRegisteredNodes::<T>::set(max);

        Self::deposit_event(Event::SetMinMaxRegisteredNodes(min, max));

        Ok(())
    }
    pub fn do_set_max_subnet_delegate_stake_rewards_percentage_change(
        value: u128,
    ) -> DispatchResult {
        MaxSubnetDelegateStakeRewardsPercentageChange::<T>::set(value);

        Self::deposit_event(Event::SetMaxSubnetDelegateStakeRewardsPercentageChange(
            value,
        ));

        Ok(())
    }
    pub fn do_set_subnet_delegate_stake_rewards_update_period(value: u32) -> DispatchResult {
        SubnetDelegateStakeRewardsUpdatePeriod::<T>::set(value);

        Self::deposit_event(Event::SetSubnetDelegateStakeRewardsUpdatePeriod(value));

        Ok(())
    }
    pub fn do_set_base_validator_reward(value: u128) -> DispatchResult {
        BaseValidatorReward::<T>::set(value);

        Self::deposit_event(Event::SetBaseValidatorReward(value));

        Ok(())
    }
    pub fn do_set_base_slash_percentage(value: u128) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        BaseSlashPercentage::<T>::set(value);

        Self::deposit_event(Event::SetBaseSlashPercentage(value));

        Ok(())
    }
    pub fn do_set_max_slash_amount(value: u128) -> DispatchResult {
        MaxSlashAmount::<T>::set(value);

        Self::deposit_event(Event::SetMaxSlashAmount(value));

        Ok(())
    }
    pub fn do_set_validator_delegate_stake_slash_config(
        threshold: u128,
        base_percentage: u128,
        max_amount: u128,
    ) -> DispatchResult {
        let percentage_factor = Self::percentage_factor_as_u128();
        let config_disabled = base_percentage == 0 && max_amount == 0;
        let config_enabled = base_percentage > 0 && max_amount > 0;

        ensure!(
            threshold > 0
                && threshold < T::MinAttestationPercentage::get()
                && base_percentage <= percentage_factor
                && (config_disabled || config_enabled),
            Error::<T>::InvalidValidatorDelegateStakeSlashConfig
        );

        ValidatorDelegateStakeSlashThreshold::<T>::set(threshold);
        BaseValidatorDelegateStakeSlashPercentage::<T>::set(base_percentage);
        MaxValidatorDelegateStakeSlashAmount::<T>::set(max_amount);

        Self::deposit_event(Event::SetValidatorDelegateStakeSlashConfig {
            threshold,
            base_percentage,
            max_amount,
        });

        Ok(())
    }
    pub fn do_set_network_max_stake_balance(value: u128) -> DispatchResult {
        NetworkMaxStakeBalance::<T>::set(value);

        Self::deposit_event(Event::SetNetworkMaxStakeBalance(value));

        Ok(())
    }
    pub fn do_set_min_delegate_stake_deposit(value: u128) -> DispatchResult {
        ensure!(
            value >= MIN_DELEGATE_STAKE_DEPOSIT_LOWER_BOUND,
            Error::<T>::InvalidMinDelegateStakeDeposit
        );

        MinDelegateStakeDeposit::<T>::set(value);

        Self::deposit_event(Event::SetMinDelegateStakeDeposit(value));

        Ok(())
    }
    pub fn do_set_node_reward_rate_update_period(value: u32) -> DispatchResult {
        NodeRewardRateUpdatePeriod::<T>::set(value);

        Self::deposit_event(Event::SetNodeRewardRateUpdatePeriod(value));

        Ok(())
    }
    pub fn do_set_max_reward_rate_decrease(value: u128) -> DispatchResult {
        MaxRewardRateDecrease::<T>::set(value);

        Self::deposit_event(Event::SetMaxRewardRateDecrease(value));

        Ok(())
    }
    pub fn do_set_subnet_distribution_power(value: u128) -> DispatchResult {
        SubnetDistributionPower::<T>::set(value);

        Self::deposit_event(Event::SetSubnetDistributionPower(value));

        Ok(())
    }
    pub fn do_set_delegate_stake_weight_factor(value: u128) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        DelegateStakeWeightFactor::<T>::set(value);

        Self::deposit_event(Event::SetDelegateStakeWeightFactor(value));

        Ok(())
    }
    pub fn do_set_consensus_validator_node_count_decay_update_interval(
        value: u32,
    ) -> DispatchResult {
        ConsensusValidatorNodeCountDecayUpdateInterval::<T>::set(value);

        Self::deposit_event(Event::SetConsensusValidatorNodeCountDecayUpdateInterval(
            value,
        ));

        Ok(())
    }
    pub fn do_set_consensus_validator_stake_weight_power_update_interval(
        value: u32,
    ) -> DispatchResult {
        ConsensusValidatorStakeWeightPowerUpdateInterval::<T>::set(value);

        Self::deposit_event(Event::SetConsensusValidatorStakeWeightPowerUpdateInterval(
            value,
        ));

        Ok(())
    }
    pub fn do_set_min_max_consensus_validator_stake_weight_power(
        min: u128,
        max: u128,
    ) -> DispatchResult {
        ensure!(min <= max, Error::<T>::InvalidValues);
        ensure!(
            max <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        MinConsensusValidatorStakeWeightPower::<T>::set(min);
        MaxConsensusValidatorStakeWeightPower::<T>::set(max);

        Self::deposit_event(Event::SetMinMaxConsensusValidatorStakeWeightPower(min, max));

        Ok(())
    }
    pub fn do_set_consensus_validator_identity_attestation_percentage(
        value: u128,
    ) -> DispatchResult {
        ensure!(
            value > 0 && value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        ConsensusValidatorIdentityAttestationPercentage::<T>::set(value);

        Self::deposit_event(Event::SetConsensusValidatorIdentityAttestationPercentage(
            value,
        ));

        Ok(())
    }
    pub fn do_set_validator_node_delegate_stake_weight_update_interval(
        value: u32,
    ) -> DispatchResult {
        ValidatorNodeDelegateStakeWeightUpdateInterval::<T>::set(value);

        Self::deposit_event(Event::SetValidatorNodeDelegateStakeWeightUpdateInterval(
            value,
        ));

        Ok(())
    }
    pub fn do_set_max_overwatch_nodes(value: u32) -> DispatchResult {
        ensure!(
            value <= T::MaxOverwatchNodesUpperBound::get(),
            Error::<T>::MaxOverwatchNodes
        );
        MaxOverwatchNodes::<T>::set(value);

        Self::deposit_event(Event::SetMaxOverwatchNodes(value));

        Ok(())
    }
    pub fn do_set_overwatch_epoch_length_multiplier(value: u32) -> DispatchResult {
        ensure!(value > 0, Error::<T>::InvalidOverwatchEpochLengthMultiplier);
        ensure!(
            T::EpochLength::get().checked_mul(value).is_some(),
            Error::<T>::InvalidOverwatchEpochLengthMultiplier
        );
        ensure!(
            T::OverwatchEpochEmissions::get()
                .checked_mul(value as u128)
                .is_some(),
            Error::<T>::InvalidOverwatchEpochLengthMultiplier
        );

        OverwatchEpochLengthMultiplier::<T>::set(value);

        Self::deposit_event(Event::SetOverwatchEpochLengthMultiplier(value));

        Ok(())
    }
    pub fn do_set_overwatch_commit_cutoff_percent(value: u128) -> DispatchResult {
        ensure!(
            Self::is_usable_overwatch_commit_cutoff_percent(value),
            Error::<T>::InvalidPercent
        );

        OverwatchCommitCutoffPercent::<T>::set(value);

        Self::deposit_event(Event::SetOverwatchCommitCutoffPercent(value));

        Ok(())
    }
    pub fn do_set_overwatch_min_stake_balance(value: u128) -> DispatchResult {
        OverwatchMinStakeBalance::<T>::set(value);

        Self::deposit_event(Event::SetOverwatchMinStakeBalance(value));

        Ok(())
    }

    pub fn do_set_min_max_subnet_node(min: u32, max: u32) -> DispatchResult {
        ensure!(min < max && min > 0, Error::<T>::InvalidValues);
        ensure!(
            max <= T::MaxSubnetNodesUpperBound::get(),
            Error::<T>::InvalidMaxSubnetNodes
        );

        MinSubnetNodes::<T>::set(min);
        MaxSubnetNodes::<T>::set(max);

        Self::deposit_event(Event::SetMinMaxSubnetNodes(min, max));

        Ok(())
    }
    pub fn do_set_tx_rate_limit(value: u32) -> DispatchResult {
        TxRateLimit::<T>::set(value);

        Self::deposit_event(Event::SetTxRateLimit(value));

        Ok(())
    }
    pub fn do_set_min_subnet_delegate_stake_factor(value: u128) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        MinSubnetDelegateStakeFactor::<T>::set(value);

        Self::deposit_event(Event::SetMinSubnetDelegateStakeFactor(value));

        Ok(())
    }
    pub fn do_collective_remove_subnet(subnet_id: u32) -> DispatchResultWithPostInfo {
        let _ = Self::do_remove_subnet(subnet_id, SubnetRemovalReason::Council);
        // Keep the declared benchmark weight. The manual cleanup accumulator does not include
        // proof-size or the complete variable-prefix model and is not safe as a refund value.
        Ok(None.into())
    }
    pub fn do_collective_remove_subnet_node(subnet_id: u32, subnet_node_id: u32) -> DispatchResult {
        Self::deposit_event(Event::CollectiveRemoveSubnetNode(subnet_id, subnet_node_id));
        Self::do_remove_subnet_node_v2(subnet_id, subnet_node_id)
    }
    pub fn do_collective_remove_overwatch_node(overwatch_node_id: u32) -> DispatchResult {
        Self::perform_remove_overwatch_node(overwatch_node_id)?;
        Self::deposit_event(Event::CollectiveRemoveOverwatchNode(overwatch_node_id));
        Ok(())
    }
    pub fn do_set_min_subnet_registration_epochs(value: u32) -> DispatchResult {
        let registration_epochs = SubnetRegistrationEpochs::<T>::get();
        // Must be less than the registration period itself
        ensure!(
            value < registration_epochs,
            Error::<T>::InvalidMinSubnetRegistrationEpochs
        );

        MinSubnetRegistrationEpochs::<T>::put(value);

        Self::deposit_event(Event::SetMinSubnetRegistrationEpochs(value));

        Ok(())
    }
    pub fn do_set_subnet_registration_epochs(value: u32) -> DispatchResult {
        let min_registration_epochs = MinSubnetRegistrationEpochs::<T>::get();
        ensure!(
            value > min_registration_epochs,
            Error::<T>::InvalidSubnetRegistrationEpochs
        );
        SubnetRegistrationEpochs::<T>::put(value);

        Self::deposit_event(Event::SetSubnetRegistrationEpochs(value));

        Ok(())
    }
    pub fn do_set_min_active_node_stake_epochs(value: u32) -> DispatchResult {
        MinActiveNodeStakeEpochs::<T>::put(value);

        Self::deposit_event(Event::SetMinActiveNodeStakeEpochs(value));

        Ok(())
    }

    pub fn do_set_delegate_stake_cooldown_epochs(value: u32) -> DispatchResult {
        ensure!(value > 0, Error::<T>::InvalidDelegateStakeCooldownEpochs);

        DelegateStakeCooldownEpochs::<T>::set(value);

        Self::deposit_event(Event::SetDelegateStakeCooldownEpochs(value));

        Ok(())
    }
    pub fn do_set_node_delegate_stake_cooldown_epochs(value: u32) -> DispatchResult {
        ensure!(
            value > 0,
            Error::<T>::InvalidNodeDelegateStakeCooldownEpochs
        );

        NodeDelegateStakeCooldownEpochs::<T>::set(value);

        Self::deposit_event(Event::SetNodeDelegateStakeCooldownEpochs(value));

        Ok(())
    }
    pub fn do_set_min_stake_cooldown_epochs(value: u32) -> DispatchResult {
        ensure!(value > 0, Error::<T>::InvalidStakeCooldownEpochs);

        StakeCooldownEpochs::<T>::set(value);

        Self::deposit_event(Event::SetStakeCooldownEpochs(value));

        Ok(())
    }
    pub fn do_set_max_unbondings(value: u32) -> DispatchResult {
        ensure!(
            value <= T::MaxUnbondingsUpperBound::get(),
            Error::<T>::InvalidMaxUnbondings
        );

        MaxUnbondings::<T>::set(value);

        Self::deposit_event(Event::SetMaxUnbondings(value));

        Ok(())
    }
    pub fn do_set_base_node_burn_amount(value: u128) -> DispatchResult {
        BaseNodeBurnAmount::<T>::put(value);

        Self::deposit_event(Event::SetBaseNodeBurnAmount(value));

        Ok(())
    }
    pub fn do_set_node_burn_rates(min: u128, max: u128) -> DispatchResult {
        ensure!(min < max && min > 0, Error::<T>::InvalidValues);

        ensure!(
            max <= DefaultMaxNodeBurnRate::get(),
            Error::<T>::InvalidPercent
        );

        MinNodeBurnRate::<T>::put(min);
        MaxNodeBurnRate::<T>::put(max);

        Self::deposit_event(Event::SetNodeBurnRates(min, max));

        Ok(())
    }
    pub fn do_set_delegate_stake_subnet_removal_interval(value: u32) -> DispatchResult {
        ensure!(
            value > 0,
            Error::<T>::InvalidDelegateStakeSubnetRemovalInterval
        );

        DelegateStakeSubnetRemovalInterval::<T>::put(value);

        Self::deposit_event(Event::SetDelegateStakeSubnetRemovalInterval(value));

        Ok(())
    }
    pub fn do_set_subnet_removal_intervals(
        activation_cooldown_epochs: u32,
        check_interval_epochs: u32,
    ) -> DispatchResult {
        ensure!(
            check_interval_epochs > 0,
            Error::<T>::InvalidSubnetRemovalCheckInterval
        );

        SubnetRemovalActivationCooldown::<T>::put(activation_cooldown_epochs);
        SubnetRemovalCheckInterval::<T>::put(check_interval_epochs);

        Self::deposit_event(Event::SetSubnetRemovalIntervals(
            activation_cooldown_epochs,
            check_interval_epochs,
        ));

        Ok(())
    }
    pub fn do_set_subnet_pause_cooldown_epochs(value: u32) -> DispatchResult {
        ensure!(value > 0, Error::<T>::InvalidSubnetPauseCooldownEpochs);
        SubnetPauseCooldownEpochs::<T>::put(value);

        Self::deposit_event(Event::SetSubnetPauseCooldownEpochs(value));

        Ok(())
    }
    pub fn do_set_max_swap_queue_calls_per_block(value: u32) -> DispatchResult {
        ensure!(
            value <= T::MaxSwapCallsPerBlockUpperBound::get(),
            Error::<T>::InvalidValues
        );

        MaxSwapQueueCallsPerBlock::<T>::put(value);

        Self::deposit_event(Event::SetMaxSwapQueueCallsPerBlock(value));

        Ok(())
    }
    pub fn do_set_max_subnet_node_min_weight_decrease_reputation_threshold(
        value: u128,
    ) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        MaxSubnetNodeMinWeightDecreaseReputationThreshold::<T>::put(value);

        Self::deposit_event(Event::SetMaxSubnetNodeMinWeightDecreaseReputationThreshold(
            value,
        ));

        Ok(())
    }
    pub fn do_set_validator_reward_k(value: u64) -> DispatchResult {
        ensure!(value > 0, Error::<T>::InvalidValidatorRewardK);

        ValidatorRewardK::<T>::put(value);

        Self::deposit_event(Event::SetValidatorRewardK(value));

        Ok(())
    }
    pub fn do_set_validator_reward_midpoint(value: u128) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        ValidatorRewardMidpoint::<T>::put(value);

        Self::deposit_event(Event::SetValidatorRewardMidpoint(value));

        Ok(())
    }

    pub fn do_set_attestor_reward_exponent(value: u64) -> DispatchResult {
        ensure!(value > 0, Error::<T>::InvalidAttestorRewardExponent);

        AttestorRewardExponent::<T>::put(value);

        Self::deposit_event(Event::SetAttestorRewardExponent(value));

        Ok(())
    }

    pub fn do_set_attestor_min_reward_factor(value: u128) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        AttestorMinRewardFactor::<T>::put(value);

        Self::deposit_event(Event::SetAttestorMinRewardFactor(value));

        Ok(())
    }

    pub fn do_set_min_max_node_reputation(min: u128, max: u128) -> DispatchResult {
        ensure!(min < max, Error::<T>::InvalidValues);

        ensure!(
            max <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        MinMinSubnetNodeReputation::<T>::put(min);
        MaxMinSubnetNodeReputation::<T>::put(max);

        Self::deposit_event(Event::SetNodeReputationLimits(min, max));

        Ok(())
    }

    pub fn do_set_min_max_node_reputation_factor(min: u128, max: u128) -> DispatchResult {
        ensure!(min < max, Error::<T>::InvalidValues);

        ensure!(
            max <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        MinNodeReputationFactor::<T>::put(min);
        MaxNodeReputationFactor::<T>::put(max);

        Self::deposit_event(Event::SetNodeReputationFactors(min, max));

        Ok(())
    }

    pub fn do_set_min_subnet_reputation(value: u128) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        MinSubnetReputation::<T>::put(value);

        Self::deposit_event(Event::SetMinSubnetReputation(value));

        Ok(())
    }

    pub fn do_set_not_in_consensus_subnet_reputation_factor(value: u128) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        NotInConsensusSubnetReputationFactor::<T>::put(value);

        Self::deposit_event(Event::SetNotInConsensusSubnetReputationFactor(value));

        Ok(())
    }

    pub fn do_set_max_pause_epochs_subnet_reputation_factor(value: u128) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        MaxPauseEpochsSubnetReputationFactor::<T>::put(value);

        Self::deposit_event(Event::SetMaxPauseEpochsSubnetReputationFactor(value));

        Ok(())
    }

    pub fn do_set_less_than_min_nodes_subnet_reputation_factor(value: u128) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        LessThanMinNodesSubnetReputationFactor::<T>::put(value);

        Self::deposit_event(Event::SetLessThanMinNodesSubnetReputationFactor(value));

        Ok(())
    }

    pub fn do_set_validator_proposal_absent_subnet_reputation_factor(
        value: u128,
    ) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        ValidatorAbsentSubnetReputationFactor::<T>::put(value);

        Self::deposit_event(Event::SetValidatorAbsentSubnetReputationFactor(value));

        Ok(())
    }

    pub fn do_set_in_consensus_subnet_reputation_factor(value: u128) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        InConsensusSubnetReputationFactor::<T>::put(value);

        Self::deposit_event(Event::SetInConsensusSubnetReputationFactor(value));

        Ok(())
    }

    pub fn do_set_overwatch_weight_factor(value: u128) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        OverwatchWeightFactor::<T>::put(value);

        Self::deposit_event(Event::SetOverwatchWeightFactor(value));

        Ok(())
    }

    pub fn do_set_max_emergency_validator_epochs_multiplier(value: u128) -> DispatchResult {
        // Must be greater than or equal to 1.0
        ensure!(
            value >= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        MaxEmergencyValidatorEpochsMultiplier::<T>::put(value);

        Self::deposit_event(Event::SetMaxEmergencyValidatorEpochsMultiplier(value));

        Ok(())
    }

    pub fn do_set_max_emergency_subnet_nodes(value: u32) -> DispatchResult {
        ensure!(
            value >= MinSubnetNodes::<T>::get()
                && value <= T::MaxEmergencySubnetNodesUpperBound::get(),
            Error::<T>::InvalidMaxEmergencySubnetNodes
        );

        MaxEmergencySubnetNodes::<T>::put(value);

        Self::deposit_event(Event::SetMaxEmergencySubnetNodes(value));

        Ok(())
    }

    pub fn do_set_emergency_validator_cooldown_epochs(value: u32) -> DispatchResult {
        EmergencyValidatorCooldownEpochs::<T>::put(value);

        Self::deposit_event(Event::SetEmergencyValidatorCooldownEpochs(value));

        Ok(())
    }

    pub fn do_set_overwatch_stake_weight_factor(value: u128) -> DispatchResult {
        let max_value = Self::percentage_factor_as_u128();

        // The exponent may range from the 0.9 dampening floor to linear weighting at 1.0.
        ensure!(
            value >= MIN_OVERWATCH_STAKE_WEIGHT_FACTOR && value <= max_value,
            Error::<T>::InvalidPercent
        );

        OverwatchStakeWeightFactor::<T>::put(value);

        Self::deposit_event(Event::SetOverwatchStakeWeightFactor(value));

        Ok(())
    }

    pub fn do_set_subnet_weight_factors(value: SubnetWeightFactorsData) -> DispatchResult {
        let sum: u128 = value
            .delegate_stake
            .saturating_add(value.node_count)
            .saturating_add(value.net_flow);

        ensure!(
            sum <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        SubnetWeightFactors::<T>::put(&value);

        Self::deposit_event(Event::SetSubnetWeightFactors(value));

        Ok(())
    }

    pub fn do_set_subnet_net_flow_smoothing_alpha(value: u128) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        SubnetNetFlowSmoothingAlpha::<T>::put(value);

        Self::deposit_event(Event::SetSubnetNetFlowSmoothingAlpha(value));

        Ok(())
    }

    pub fn do_set_default_overwatch_subnet_weight(value: u128) -> DispatchResult {
        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        DefaultOverwatchSubnetWeight::<T>::put(&value);

        Self::deposit_event(Event::SetDefaultOverwatchSubnetWeight(value));

        Ok(())
    }

    pub fn do_set_overwatch_validator_whitelist(validator_id: u32, value: bool) -> DispatchResult {
        ensure!(
            ValidatorsData::<T>::contains_key(validator_id),
            Error::<T>::InvalidValidatorId
        );

        if value {
            OverwatchValidatorWhitelist::<T>::insert(validator_id, ());
        } else {
            ensure!(
                !ValidatorOverwatchNodeId::<T>::contains_key(validator_id),
                Error::<T>::ActiveOverwatchNodeCannotBeUnwhitelisted
            );
            OverwatchValidatorWhitelist::<T>::remove(validator_id);
        }

        Self::deposit_event(Event::SetOverwatchValidatorWhitelist(validator_id, value));

        Ok(())
    }
}
