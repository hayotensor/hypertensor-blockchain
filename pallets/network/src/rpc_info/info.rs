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
            state: subnet_data.state,
            start_epoch: subnet_data.start_epoch,
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
            subnet_node_queue_epochs: SubnetNodeQueueEpochs::<T>::get(subnet_id),
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
            initial_validators: NodeRegistrationInitialValidatorIds::<T>::get(subnet_id),
            initial_validator_data: InitialValidatorData::<T>::get(subnet_id),
            max_registered_nodes: MaxRegisteredNodes::<T>::get(subnet_id),
            owner: SubnetOwner::<T>::get(subnet_id),
            pending_owner: PendingSubnetOwner::<T>::get(subnet_id),
            registration_epoch: SubnetRegistrationEpoch::<T>::get(subnet_id),
            prev_pause_epoch: PreviousSubnetPauseEpoch::<T>::get(subnet_id),
            slot_index: SubnetSlot::<T>::get(subnet_id),
            slot_assignment: SlotAssignment::<T>::get(subnet_id),
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
            min_consensus_node_attestation_percentage:
                Self::get_min_consensus_node_attestation_percentage_for_epoch(
                    subnet_id,
                    current_subnet_epoch,
                ),
            pending_min_consensus_node_attestation_percentage:
                PendingSubnetMinConsensusNodeAttestationPercentage::<T>::get(subnet_id)
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

    pub fn get_all_subnets_info() -> Vec<SubnetInfo<T>> {
        let mut infos: Vec<SubnetInfo<T>> = Vec::new();

        for (subnet_id, subnet_data) in SubnetsData::<T>::iter() {
            if let Some(subnet_info) = Self::get_subnet_info(subnet_id) {
                infos.push(subnet_info);
            }
        }

        infos
    }

    pub fn get_subnet_node_info(subnet_id: u32, subnet_node_id: u32) -> Option<SubnetNodeInfo<T>> {
        let subnet_node = if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            SubnetNodesData::<T>::get(subnet_id, subnet_node_id)
        } else if RegisteredSubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            RegisteredSubnetNodesData::<T>::get(subnet_id, subnet_node_id)
        } else {
            return None;
        };

        let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id);
        let coldkey = ValidatorColdkey::<T>::get(validator_id.unwrap()).unwrap();
        let info = SubnetNodeInfo {
            validator_id: validator_id,
            subnet_id: subnet_id,
            subnet_node_id: subnet_node_id,
            coldkey: coldkey,
            hotkey: Self::get_subnet_node_associated_hotkey(subnet_id, subnet_node_id).unwrap(),
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

    /// Get subnet ID nodes info
    pub fn get_subnet_nodes_info(subnet_id: u32) -> Vec<SubnetNodeInfo<T>> {
        let mut infos: Vec<SubnetNodeInfo<T>> = Vec::new();

        for (subnet_node_id, _) in SubnetNodeReputation::<T>::iter_prefix(subnet_id) {
            if let Some(subnet_node_info) = Self::get_subnet_node_info(subnet_id, subnet_node_id) {
                infos.push(subnet_node_info);
            }
        }

        infos
    }

    /// Get all subnet ID nodes info
    pub fn get_all_subnet_nodes_info() -> Vec<SubnetNodeInfo<T>> {
        let mut infos: Vec<SubnetNodeInfo<T>> = Vec::new();

        for (subnet_id, _) in SubnetsData::<T>::iter() {
            for (subnet_node_id, _) in SubnetNodeReputation::<T>::iter_prefix(subnet_id) {
                if let Some(subnet_node_info) =
                    Self::get_subnet_node_info(subnet_id, subnet_node_id)
                {
                    infos.push(subnet_node_info);
                }
            }
        }

        infos
    }

    /// Get the elected validators node info
    pub fn get_elected_validator_info(
        subnet_id: u32,
        subnet_epoch: u32,
    ) -> Option<SubnetNodeInfo<T>> {
        match SubnetElectedValidator::<T>::try_get(subnet_id, subnet_epoch) {
            Ok(subnet_node_id) => Self::get_subnet_node_info(subnet_id, subnet_node_id),
            Err(()) => None,
        }
    }

    pub fn get_validators_and_attestors(subnet_id: u32) -> Vec<SubnetNodeInfo<T>> {
        let mut infos: Vec<SubnetNodeInfo<T>> = Vec::new();
        if let Some(emergency_validator_data) = EmergencySubnetNodeElectionData::<T>::get(subnet_id)
        {
            for subnet_node_id in emergency_validator_data.subnet_node_ids {
                if let Some(subnet_node_info) =
                    Self::get_subnet_node_info(subnet_id, subnet_node_id)
                {
                    infos.push(subnet_node_info);
                }
            }
        } else {
            for subnet_node_id in SubnetNodeElectionSlots::<T>::get(subnet_id) {
                if let Some(subnet_node_info) =
                    Self::get_subnet_node_info(subnet_id, subnet_node_id)
                {
                    infos.push(subnet_node_info);
                }
            }
        };

        infos
    }

    /// Returns whether a node identified by either a peer ID or hotkey satisfies a subnet's
    /// proof-of-stake requirements. Peer ID takes precedence when both identifiers are supplied.
    pub fn proof_of_stake_v2(
        subnet_id: u32,
        peer_id: Option<Vec<u8>>,
        hotkey: Option<T::AccountId>,
        min_class: u8,
        min_stake: Option<u128>,
    ) -> bool {
        if let Some(peer_id) = peer_id {
            return Self::proof_of_stake_peer(subnet_id, peer_id, min_class, min_stake);
        } else if let Some(hotkey) = hotkey {
            return Self::proof_of_stake_hotkey(subnet_id, hotkey, min_class, min_stake);
        }

        false
    }

    /// Returns whether a node identified by a main, bootnode, or client peer ID satisfies the
    /// requested subnet-node classification and stake. Overwatch and official subnet bootnode
    /// peer IDs retain their trusted-role behavior.
    pub fn proof_of_stake_peer(
        subnet_id: u32,
        peer_id: Vec<u8>,
        min_class: u8,
        min_stake: Option<u128>,
    ) -> bool {
        if !SubnetsData::<T>::contains_key(subnet_id) {
            return false;
        }

        let class = if let Some(subnet_node_class) = SubnetNodeClass::from_repr(min_class.into()) {
            subnet_node_class
        } else {
            return false;
        };

        let min_stake = min_stake.unwrap_or(SubnetMinStakeBalance::<T>::get(subnet_id));
        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        let peer_id = PeerId(peer_id);

        let check_mapping = |mapping: fn(u32, PeerId) -> Result<u32, ()>| -> bool {
            mapping(subnet_id, peer_id.clone())
                .ok()
                .and_then(|subnet_node_id| {
                    SubnetNodesData::<T>::try_get(subnet_id, subnet_node_id)
                        .ok()
                        .or_else(|| {
                            if class == SubnetNodeClass::Registered {
                                RegisteredSubnetNodesData::<T>::try_get(subnet_id, subnet_node_id)
                                    .ok()
                            } else {
                                None
                            }
                        })
                })
                .map(|subnet_node| {
                    Self::subnet_node_has_proof_of_stake(
                        subnet_id,
                        &subnet_node,
                        &class,
                        current_subnet_epoch,
                        min_stake,
                    )
                })
                .unwrap_or(false)
        };

        if check_mapping(PeerIdSubnetNodeId::<T>::try_get)
            || check_mapping(BootnodePeerIdSubnetNodeId::<T>::try_get)
            || check_mapping(ClientPeerIdSubnetNodeId::<T>::try_get)
        {
            return true;
        }

        if PeerIdOverwatchNodeId::<T>::try_get(subnet_id, &peer_id).is_ok() {
            return true;
        }

        SubnetBootnodes::<T>::get(subnet_id).contains_key(&peer_id)
    }

    /// Returns whether any subnet node whose effective hotkey matches `hotkey` satisfies the
    /// requested classification and stake. A node-specific hotkey overrides its validator hotkey.
    pub fn proof_of_stake_hotkey(
        subnet_id: u32,
        hotkey: T::AccountId,
        min_class: u8,
        min_stake: Option<u128>,
    ) -> bool {
        if !SubnetsData::<T>::contains_key(subnet_id) {
            return false;
        }

        let class = if let Some(subnet_node_class) = SubnetNodeClass::from_repr(min_class.into()) {
            subnet_node_class
        } else {
            return false;
        };

        let min_stake = min_stake.unwrap_or(SubnetMinStakeBalance::<T>::get(subnet_id));
        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);

        let has_proof = |subnet_node_id: u32, subnet_node: &SubnetNode<T>| -> bool {
            Self::get_subnet_node_associated_hotkey(subnet_id, subnet_node_id)
                .map(|node_hotkey| node_hotkey == hotkey)
                .unwrap_or(false)
                && Self::subnet_node_has_proof_of_stake(
                    subnet_id,
                    subnet_node,
                    &class,
                    current_subnet_epoch,
                    min_stake,
                )
        };

        if SubnetNodesData::<T>::iter_prefix(subnet_id)
            .any(|(subnet_node_id, subnet_node)| has_proof(subnet_node_id, &subnet_node))
        {
            return true;
        }

        class == SubnetNodeClass::Registered
            && RegisteredSubnetNodesData::<T>::iter_prefix(subnet_id)
                .any(|(subnet_node_id, subnet_node)| has_proof(subnet_node_id, &subnet_node))
    }

    fn subnet_node_has_proof_of_stake(
        subnet_id: u32,
        subnet_node: &SubnetNode<T>,
        min_class: &SubnetNodeClass,
        current_subnet_epoch: u32,
        min_stake: u128,
    ) -> bool {
        subnet_node.has_classification(min_class, current_subnet_epoch)
            && NodeSubnetStake::<T>::get(subnet_node.id, subnet_id) >= min_stake
    }

    /// Get all bootnodes organized by the official bootnodes, node bootnodes, and registered bootnodes
    pub fn get_bootnodes(subnet_id: u32) -> AllSubnetBootnodes<T> {
        let subnet_bootnodes: BTreeMap<PeerId, NetworkBytes<T>> =
            SubnetBootnodes::<T>::get(subnet_id);

        let node_bootnodes: BTreeMap<PeerId, Option<NetworkBytes<T>>> =
            SubnetNodesData::<T>::iter_prefix(subnet_id)
                .filter_map(|(_, node)| {
                    if let Some(peer_info) = node.bootnode_peer_info {
                        if let Some(multiaddr) = peer_info.multiaddr {
                            return Some((peer_info.peer_id, Some(multiaddr)));
                        }
                    }
                    None
                })
                .collect();

        let registered_bootnodes: BTreeMap<PeerId, Option<NetworkBytes<T>>> =
            RegisteredSubnetNodesData::<T>::iter_prefix(subnet_id)
                .filter_map(|(_, node)| {
                    if let Some(peer_info) = node.bootnode_peer_info {
                        if let Some(multiaddr) = peer_info.multiaddr {
                            return Some((peer_info.peer_id, Some(multiaddr)));
                        }
                    }
                    None
                })
                .collect();

        AllSubnetBootnodes::<T> {
            subnet_bootnodes,
            node_bootnodes,
            registered_bootnodes,
        }
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

    /// Get an accounts delegate stake across the entire network
    pub fn get_delegate_stakes(account_id: T::AccountId) -> Vec<DelegateStakeInfo> {
        let mut delegate_stake: Vec<DelegateStakeInfo> = Vec::new();

        for (subnet_id, shares) in AccountSubnetDelegateStakeShares::<T>::iter_prefix(&account_id) {
            let balance = Self::convert_to_balance(
                shares,
                TotalSubnetDelegateStakeShares::<T>::get(subnet_id),
                TotalSubnetDelegateStakeBalance::<T>::get(subnet_id),
            );

            delegate_stake.push(DelegateStakeInfo {
                subnet_id,
                shares,
                balance,
            })
        }

        delegate_stake
    }

    /// Get an accounts node delegate stake across the entire network
    pub fn get_node_delegate_stakes(account_id: T::AccountId) -> Vec<NodeDelegateStakeInfo> {
        let mut node_delegate_stake: Vec<NodeDelegateStakeInfo> = Vec::new();

        for ((subnet_id, subnet_node_id), shares) in
            AccountNodeDelegateStakeShares::<T>::iter_prefix((&account_id,))
        {
            let balance = Self::convert_to_balance(
                shares,
                TotalNodeDelegateStakeShares::<T>::get(subnet_id, subnet_node_id),
                TotalNodeDelegateStakeBalance::<T>::get(subnet_id, subnet_node_id),
            );

            node_delegate_stake.push(NodeDelegateStakeInfo {
                subnet_id,
                subnet_node_id,
                shares,
                balance,
            })
        }
        node_delegate_stake
    }

    pub fn get_validator_delegate_stakes(
        account_id: T::AccountId,
    ) -> Vec<ValidatorDelegateStakeInfo> {
        let mut validator_delegate_stake: Vec<ValidatorDelegateStakeInfo> = Vec::new();

        for (validator_id, shares) in
            AccountValidatorDelegateStakeShares::<T>::iter_prefix(&account_id)
        {
            let balance = Self::convert_to_balance(
                shares,
                ValidatorDelegateStakeShares::<T>::get(validator_id),
                ValidatorDelegateStakeBalance::<T>::get(validator_id),
            );

            validator_delegate_stake.push(ValidatorDelegateStakeInfo {
                validator_id,
                shares,
                balance,
            })
        }
        validator_delegate_stake
    }

    pub fn get_overwatch_node_info(
        overwatch_node_id: u32,
    ) -> Option<OverwatchNodeInfo<T::AccountId>> {
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

    pub fn get_overwatch_commits_for_epoch_and_node(
        epoch: u32,
        overwatch_node_id: u32,
    ) -> Vec<(u32, T::Hash)> {
        // Returns (subnet_id, commit_hash) pairs
        OverwatchCommits::<T>::iter_prefix((epoch, overwatch_node_id)).collect()
    }

    pub fn get_overwatch_reveals_for_epoch_and_node(
        epoch: u32,
        overwatch_node_id: u32,
    ) -> Vec<(u32, u128)> {
        // Reveals are keyed by (epoch, subnet_id, overwatch_node_id), so a node query
        // must scan the epoch prefix and filter by overwatch node.
        OverwatchReveals::<T>::iter_prefix((epoch,))
            .filter_map(|((subnet_id, node_id), weight)| {
                if node_id == overwatch_node_id {
                    Some((subnet_id, weight))
                } else {
                    None
                }
            })
            .collect()
    }
}
