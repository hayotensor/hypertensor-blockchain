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

impl<T: Config> Pallet<T> {
    pub fn get_subnet_info(subnet_id: u32) -> Option<SubnetInfo<T>> {
        let subnet_data = SubnetsData::<T>::try_get(subnet_id).ok()?;
        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        let reputation_factor_schedule = SubnetReputationFactorSchedules::<T>::get(subnet_id);
        let reputation_factors = reputation_factor_schedule.factors_for_epoch(current_subnet_epoch);
        let pending_reputation_factors =
            reputation_factor_schedule.pending_after(current_subnet_epoch);

        Some(SubnetInfo {
            id: subnet_data.id,
            friendly_id: SubnetIdFriendlyUid::<T>::get(subnet_id),
            name: subnet_data.name,
            repo: subnet_data.repo,
            description: subnet_data.description,
            misc: subnet_data.misc,
            consensus_mechanism: subnet_data.consensus_mechanism,
            state: subnet_data.state,
            consensus_eligible_from_subnet_epoch: subnet_data.consensus_eligible_from_subnet_epoch,
            pause_started_global_epoch: subnet_data.pause.map(|pause| pause.started_global_epoch),
            pause_started_subnet_epoch: subnet_data.pause.map(|pause| pause.started_subnet_epoch),
            churn_limit: ChurnLimit::<T>::get(subnet_id),
            churn_limit_multiplier: ChurnLimitMultiplier::<T>::get(subnet_id),
            min_stake: SubnetMinStakeBalance::<T>::get(subnet_id),
            max_stake: SubnetMaxStakeBalance::<T>::get(subnet_id),
            queue_immunity_epochs: Self::get_queue_immunity_epochs_for_epoch(
                subnet_id,
                current_subnet_epoch,
            ),
            pending_queue_immunity_epochs: PendingQueueImmunityEpochs::<T>::get(subnet_id)
                .filter(|pending| pending.effective_subnet_epoch > current_subnet_epoch),
            target_node_registrations_per_epoch: TargetNodeRegistrationsPerEpoch::<T>::get(
                subnet_id,
            ),
            node_registrations_this_epoch: NodeRegistrationsThisEpoch::<T>::get(subnet_id),
            subnet_node_queue_epochs: Self::get_subnet_node_queue_epochs_for_epoch(
                subnet_id,
                current_subnet_epoch,
            ),
            pending_subnet_node_queue_epochs: PendingSubnetNodeQueueEpochs::<T>::get(subnet_id)
                .filter(|pending| pending.effective_subnet_epoch > current_subnet_epoch),
            idle_classification_epochs: Self::get_idle_classification_epochs_for_epoch(
                subnet_id,
                current_subnet_epoch,
            ),
            pending_idle_classification_epochs: PendingIdleClassificationEpochs::<T>::get(
                subnet_id,
            )
            .filter(|pending| pending.effective_subnet_epoch > current_subnet_epoch),
            included_classification_epochs: Self::get_included_classification_epochs_for_epoch(
                subnet_id,
                current_subnet_epoch,
            ),
            pending_included_classification_epochs: PendingIncludedClassificationEpochs::<T>::get(
                subnet_id,
            )
            .filter(|pending| pending.effective_subnet_epoch > current_subnet_epoch),
            delegate_stake_percentage: Self::get_subnet_delegate_stake_rewards_percentage_for_epoch(
                subnet_id,
                current_subnet_epoch,
            ),
            pending_delegate_stake_percentage:
                PendingSubnetDelegateStakeRewardsPercentage::<T>::get(subnet_id)
                    .filter(|pending| pending.effective_subnet_epoch > current_subnet_epoch),
            last_delegate_stake_rewards_update: LastSubnetDelegateStakeRewardsUpdate::<T>::get(
                subnet_id,
            ),
            consensus_validator_node_count_decay:
                Self::get_consensus_validator_node_count_decay_for_epoch(
                    subnet_id,
                    current_subnet_epoch,
                ),
            pending_consensus_validator_node_count_decay:
                PendingConsensusValidatorNodeCountDecay::<T>::get(subnet_id)
                    .filter(|pending| pending.effective_subnet_epoch > current_subnet_epoch),
            last_consensus_validator_node_count_decay_update:
                LastConsensusValidatorNodeCountDecayUpdate::<T>::get(subnet_id),
            consensus_validator_stake_weight_power:
                Self::get_consensus_validator_stake_weight_power_for_epoch(
                    subnet_id,
                    current_subnet_epoch,
                ),
            pending_consensus_validator_stake_weight_power:
                PendingConsensusValidatorStakeWeightPower::<T>::get(subnet_id)
                    .filter(|pending| pending.effective_subnet_epoch > current_subnet_epoch),
            last_consensus_validator_stake_weight_power_update:
                LastConsensusValidatorStakeWeightPowerUpdate::<T>::get(subnet_id),
            node_burn_rate_alpha: NodeBurnRateAlpha::<T>::get(subnet_id),
            current_node_burn_rate: CurrentNodeBurnRate::<T>::get(subnet_id),
            max_registered_nodes: MaxRegisteredNodes::<T>::get(subnet_id),
            owner: SubnetOwner::<T>::get(subnet_id),
            pending_owner: PendingSubnetOwner::<T>::get(subnet_id),
            registration_epoch: SubnetRegistrationEpoch::<T>::get(subnet_id),
            slot_index: SubnetSlot::<T>::get(subnet_id),
            subnet_node_min_weight_decrease_reputation_threshold:
                Self::get_subnet_node_min_weight_decrease_reputation_threshold_for_epoch(
                    subnet_id,
                    current_subnet_epoch,
                ),
            pending_subnet_node_min_weight_decrease_reputation_threshold:
                PendingSubnetNodeMinWeightDecreaseReputationThreshold::<T>::get(subnet_id)
                    .filter(|pending| pending.effective_subnet_epoch > current_subnet_epoch),
            reputation: SubnetReputation::<T>::get(subnet_id),
            min_subnet_node_reputation: Self::get_min_subnet_node_reputation_for_epoch(
                subnet_id,
                current_subnet_epoch,
            ),
            pending_min_subnet_node_reputation: PendingMinSubnetNodeReputation::<T>::get(subnet_id)
                .filter(|pending| pending.effective_subnet_epoch > current_subnet_epoch),
            absent_decrease_reputation_factor: reputation_factors.absent_decrease,
            included_increase_reputation_factor: reputation_factors.included_increase,
            below_min_weight_decrease_reputation_factor: reputation_factors
                .below_min_weight_decrease,
            non_attestor_decrease_reputation_factor: reputation_factors.non_attestor_decrease,
            non_consensus_attestor_decrease_reputation_factor: reputation_factors
                .non_consensus_attestor_decrease,
            validator_absent_subnet_node_reputation_factor: reputation_factors
                .validator_absent_decrease,
            validator_non_consensus_subnet_node_reputation_factor: reputation_factors
                .validator_non_consensus_decrease,
            pending_reputation_factors,
            bootnode_access: SubnetBootnodeAccess::<T>::get(subnet_id),
            bootnodes: SubnetBootnodes::<T>::get(subnet_id),
            total_nodes: TotalSubnetNodes::<T>::get(subnet_id),
            total_active_nodes: TotalActiveSubnetNodes::<T>::get(subnet_id),
            total_electable_nodes: TotalSubnetElectableNodes::<T>::get(subnet_id),
            current_min_delegate_stake: Self::get_min_subnet_delegate_stake_balance(subnet_id),
            total_subnet_stake: TotalSubnetStake::<T>::get(subnet_id),
            total_subnet_delegate_stake_shares: TotalSubnetDelegateStakeShares::<T>::get(subnet_id),
            total_subnet_delegate_stake_balance: TotalSubnetDelegateStakeBalance::<T>::get(
                subnet_id,
            ),
        })
    }

    pub fn get_subnet_node_info(subnet_id: u32, subnet_node_id: u32) -> Option<SubnetNodeInfo<T>> {
        let subnet_node = if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            SubnetNodesData::<T>::get(subnet_id, subnet_node_id)
        } else if RegisteredSubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            RegisteredSubnetNodesData::<T>::get(subnet_id, subnet_node_id)
        } else {
            return None;
        };

        // All RPC helpers must treat inconsistent or partially-pruned storage as missing data,
        // rather than trapping the runtime API with an unwrap.
        let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id)?;
        if validator_id != subnet_node.validator_id {
            return None;
        }
        let coldkey = ValidatorColdkey::<T>::get(validator_id)?;
        let hotkey = Self::get_subnet_node_associated_hotkey(subnet_id, subnet_node_id).ok()?;
        let info = SubnetNodeInfo {
            validator_id: Some(validator_id),
            subnet_id: subnet_id,
            subnet_node_id: subnet_node_id,
            coldkey: coldkey,
            hotkey,
            peer_info: subnet_node.peer_info,
            bootnode_peer_info: subnet_node.bootnode_peer_info,
            client_peer_info: subnet_node.client_peer_info,
            classification: subnet_node.classification,
            unique: subnet_node.unique,
            non_unique: subnet_node.non_unique,
            stake_balance: NodeSubnetStake::<T>::get(subnet_node_id, subnet_id),
            subnet_node_reputation: SubnetNodeReputation::<T>::get(subnet_id, subnet_node_id),
            node_slot_index: NodeSlotIndex::<T>::get(subnet_id, subnet_node_id),
            consecutive_idle_epochs: SubnetNodeIdleConsecutiveEpochs::<T>::get(
                subnet_id,
                subnet_node_id,
            ),
            consecutive_included_epochs: SubnetNodeConsecutiveIncludedEpochs::<T>::get(
                subnet_id,
                subnet_node_id,
            ),
        };

        return Some(info);
    }

    pub fn get_validator_info(validator_id: u32) -> Option<ValidatorInfo<T>> {
        let validator = if ValidatorsData::<T>::contains_key(validator_id) {
            ValidatorsData::<T>::get(validator_id)
        } else {
            return None;
        };

        let info = ValidatorInfo {
            id: validator_id,
            coldkey: ValidatorColdkey::<T>::get(validator_id),
            hotkey: validator.hotkey,
            delegate_reward_rate: validator.delegate_reward_rate,
            last_delegate_reward_rate_update: validator.last_delegate_reward_rate_update,
            delegate_account: validator.delegate_account,
            identity: validator.identity,
        };

        return Some(info);
    }

    pub fn get_validator_info_by_coldkey(coldkey: &T::AccountId) -> Option<ValidatorInfo<T>> {
        Self::get_validator_info(ColdkeyValidatorId::<T>::get(coldkey)?)
    }

    pub fn get_validator_info_by_hotkey(hotkey: &T::AccountId) -> Option<ValidatorInfo<T>> {
        Self::get_validator_info(HotkeyValidatorId::<T>::get(hotkey)?)
    }

    /// Get the elected validators node info
    pub fn get_elected_validator_info(
        subnet_id: u32,
        subnet_epoch: u32,
    ) -> Option<SubnetNodeInfo<T>> {
        match SubnetElectedValidator::<T>::try_get(subnet_id, subnet_epoch) {
            Ok(round) => Self::get_subnet_node_info(subnet_id, round.validator_subnet_node_id),
            Err(()) => None,
        }
    }

    /// Return the immutable election record rather than rebuilding historical data from the
    /// validator's current node state.
    pub fn get_consensus_round(subnet_id: u32, subnet_epoch: u32) -> Option<ElectedConsensusRound> {
        SubnetElectedValidator::<T>::get(subnet_id, subnet_epoch)
    }

    pub fn get_validators_and_attestors(subnet_id: u32) -> Vec<SubnetNodeInfo<T>> {
        let mut infos: Vec<SubnetNodeInfo<T>> = Vec::new();
        let subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        let (subnet_node_ids, _) = Self::effective_consensus_validator_ids(subnet_id, subnet_epoch);

        for subnet_node_id in subnet_node_ids {
            if let Some(subnet_node_info) = Self::get_subnet_node_info(subnet_id, subnet_node_id) {
                infos.push(subnet_node_info);
            }
        }

        infos
    }

    /// Get all nodes from a validator
    pub fn get_validator_subnet_nodes_info(validator_id: u32) -> Vec<SubnetNodeInfo<T>> {
        let mut infos: Vec<SubnetNodeInfo<T>> = Vec::new();

        for (subnet_id, nodes) in ValidatorSubnetNodes::<T>::get(validator_id).iter() {
            for subnet_node_id in nodes {
                if let Some(subnet_node_info) =
                    Self::get_subnet_node_info(*subnet_id, *subnet_node_id)
                {
                    infos.push(subnet_node_info);
                }
            }
        }

        infos
    }

    pub fn get_validator_stakes(validator_id: u32) -> Vec<NodeStakeInfo> {
        let mut validator_id_stake: Vec<NodeStakeInfo> = Vec::new();

        for (subnet_id, nodes) in ValidatorSubnetNodes::<T>::get(validator_id).iter() {
            for subnet_node_id in nodes {
                validator_id_stake.push(NodeStakeInfo {
                    subnet_id: Some(*subnet_id),
                    subnet_node_id: Some(*subnet_node_id),
                    balance: NodeSubnetStake::<T>::get(subnet_node_id, subnet_id),
                })
            }
        }

        validator_id_stake
    }

    pub fn get_overwatch_node_info(
        overwatch_node_id: u32,
    ) -> Option<OverwatchNodeInfo<T::AccountId>> {
        // The validator mapping is deliberately retained after removal so stake can be
        // withdrawn. Only the canonical active-node map proves RPC-visible membership.
        if !OverwatchNodes::<T>::contains_key(overwatch_node_id) {
            return None;
        }

        if let Some(validator_id) = OverwatchNodeValidatorId::<T>::get(overwatch_node_id) {
            return Some(OverwatchNodeInfo {
                overwatch_node_id,
                hotkey: Some(Self::get_overwatch_node_associated_hotkey(overwatch_node_id).ok()?),
                peer_ids: OverwatchNodeIndex::<T>::get(overwatch_node_id),
                reputation: ValidatorReputation::<T>::get(validator_id),
                account_overwatch_stake: OverwatchNodeStakeBalance::<T>::get(overwatch_node_id),
            });
        }
        None
    }

    pub fn get_all_overwatch_nodes_info() -> Vec<OverwatchNodeInfo<T::AccountId>> {
        let mut infos: Vec<OverwatchNodeInfo<T::AccountId>> = Vec::new();

        for (overwatch_node_id, _) in OverwatchNodes::<T>::iter() {
            if let Some(overwatch_node_info) = Self::get_overwatch_node_info(overwatch_node_id) {
                infos.push(overwatch_node_info);
            }
        }

        infos
    }
}
