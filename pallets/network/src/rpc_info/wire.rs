// Copyright (C) Hypertensor.
// SPDX-License-Identifier: Apache-2.0

//! Stable wire-format views used by the runtime API.

use super::*;
use frame_support::traits::Get;
use network_rpc_types as rpc;

fn rpc_bytes(value: impl Into<Vec<u8>>) -> rpc::RpcBytes {
    rpc::RpcBytes(value.into())
}

fn rpc_consensus_mechanism(value: ConsensusMechanism) -> rpc::ConsensusMechanism {
    match value {
        ConsensusMechanism::Attestation => rpc::ConsensusMechanism::Attestation,
    }
}

fn rpc_subnet_state(value: SubnetState) -> rpc::SubnetState {
    match value {
        SubnetState::Registered => rpc::SubnetState::Registered,
        SubnetState::Active => rpc::SubnetState::Active,
        SubnetState::Paused => rpc::SubnetState::Paused,
    }
}

fn rpc_node_class(value: SubnetNodeClass) -> rpc::SubnetNodeClass {
    match value {
        SubnetNodeClass::Registered => rpc::SubnetNodeClass::Registered,
        SubnetNodeClass::Idle => rpc::SubnetNodeClass::Idle,
        SubnetNodeClass::Included => rpc::SubnetNodeClass::Included,
        SubnetNodeClass::Validator => rpc::SubnetNodeClass::Validator,
    }
}

fn rpc_reputation_factors(value: SubnetReputationFactors) -> rpc::SubnetReputationFactors {
    rpc::SubnetReputationFactors {
        absent_decrease: value.absent_decrease.into(),
        included_increase: value.included_increase.into(),
        below_min_weight_decrease: value.below_min_weight_decrease.into(),
        non_attestor_decrease: value.non_attestor_decrease.into(),
        non_consensus_attestor_decrease: value.non_consensus_attestor_decrease.into(),
        validator_absent_decrease: value.validator_absent_decrease.into(),
        validator_non_consensus_decrease: value.validator_non_consensus_decrease.into(),
    }
}

fn rpc_peer_info<T: Config>(value: PeerInfo<T>) -> rpc::PeerInfo {
    rpc::PeerInfo {
        peer_id: rpc_bytes(value.peer_id.0),
        multiaddr: value
            .multiaddr
            .map(|address| rpc_bytes(address.into_inner())),
    }
}

fn rpc_subnet_node_info<T: Config>(
    value: SubnetNodeInfo<T>,
) -> Option<rpc::SubnetNodeInfo<T::AccountId>> {
    Some(rpc::SubnetNodeInfo {
        validator_id: value.validator_id?,
        subnet_id: value.subnet_id,
        subnet_node_id: value.subnet_node_id,
        coldkey: value.coldkey,
        hotkey: value.hotkey,
        peer_info: value.peer_info.map(rpc_peer_info::<T>),
        bootnode_peer_info: value.bootnode_peer_info.map(rpc_peer_info::<T>),
        client_peer_info: value.client_peer_info.map(rpc_peer_info::<T>),
        classification: rpc::SubnetNodeClassification {
            node_class: rpc_node_class(value.classification.node_class),
            start_epoch: value.classification.start_epoch,
        },
        unique: value.unique.map(|bytes| rpc_bytes(bytes.into_inner())),
        non_unique: value.non_unique.map(|bytes| rpc_bytes(bytes.into_inner())),
        stake_balance: value.stake_balance.into(),
        subnet_node_reputation: value.subnet_node_reputation.map(Into::into),
        node_slot_index: value.node_slot_index,
        consecutive_idle_epochs: value.consecutive_idle_epochs,
        consecutive_included_epochs: value.consecutive_included_epochs,
    })
}

fn page_sorted<Item, Cursor, Key>(
    mut items: Vec<Item>,
    request: rpc::PageRequest<Cursor>,
    key: Key,
) -> Result<rpc::Page<Item, Cursor>, rpc::NetworkQueryError>
where
    Cursor: Copy + Ord,
    Key: Fn(&Item) -> Cursor,
{
    let limit = request.validated_limit()?;
    items.sort_by_key(|item| key(item));
    if let Some(cursor) = request.cursor {
        items.retain(|item| key(item) > cursor);
    }

    let has_more = items.len() > limit;
    items.truncate(limit);
    let next_cursor = has_more.then(|| key(items.last().expect("a non-zero page limit; qed")));
    Ok(rpc::Page { items, next_cursor })
}

fn rpc_identity<T: Config>(value: IdentityData<T>) -> rpc::IdentityInfo {
    rpc::IdentityInfo {
        name: value.name.map(|value| rpc_bytes(value.into_inner())),
        url: value.url.map(|value| rpc_bytes(value.into_inner())),
        image: value.image.map(|value| rpc_bytes(value.into_inner())),
        discord: value.discord.map(|value| rpc_bytes(value.into_inner())),
        x: value.x.map(|value| rpc_bytes(value.into_inner())),
        telegram: value.telegram.map(|value| rpc_bytes(value.into_inner())),
        github: value.github.map(|value| rpc_bytes(value.into_inner())),
        hugging_face: value
            .hugging_face
            .map(|value| rpc_bytes(value.into_inner())),
        description: value.description.map(|value| rpc_bytes(value.into_inner())),
        misc: value.misc.map(|value| rpc_bytes(value.into_inner())),
    }
}

fn rpc_validator_reputation(value: Reputation) -> rpc::ValidatorReputationInfo {
    rpc::ValidatorReputationInfo {
        start_epoch: value.start_epoch,
        score: value.score.into(),
        lifetime_node_count: value.lifetime_node_count,
        total_active_nodes: value.total_active_nodes,
        total_increases: value.total_increases,
        total_decreases: value.total_decreases,
        average_proposal_identity_support: value.average_proposal_identity_support.into(),
        identity_support_samples: value.identity_support_samples,
        last_validator_epoch: value.last_validator_epoch,
        overwatch_score: value.ow_score.into(),
    }
}

fn rpc_consensus_policy(value: ConsensusPolicySnapshot) -> rpc::ConsensusPolicyInfo {
    rpc::ConsensusPolicyInfo {
        min_attestation_percentage: value.min_attestation_percentage.into(),
        super_majority_attestation_ratio: value.super_majority_attestation_ratio.into(),
        base_validator_reward: value.base_validator_reward.into(),
        subnet_owner_percentage: value.subnet_owner_percentage.into(),
        validator_reward_k: value.validator_reward_k,
        validator_reward_midpoint: value.validator_reward_midpoint.into(),
        attestor_reward_exponent: value.attestor_reward_exponent,
        attestor_min_reward_factor: value.attestor_min_reward_factor.into(),
        base_slash_percentage: value.base_slash_percentage.into(),
        max_slash_amount: value.max_slash_amount.into(),
        validator_delegate_stake_slash_threshold: value
            .validator_delegate_stake_slash_threshold
            .into(),
        base_validator_delegate_stake_slash_percentage: value
            .base_validator_delegate_stake_slash_percentage
            .into(),
        max_validator_delegate_stake_slash_amount: value
            .max_validator_delegate_stake_slash_amount
            .into(),
        validator_reputation_increase_factor: value.validator_reputation_increase_factor.into(),
        validator_reputation_decrease_factor: value.validator_reputation_decrease_factor.into(),
        validator_absent_subnet_reputation_factor: value
            .validator_absent_subnet_reputation_factor
            .into(),
        in_consensus_subnet_reputation_factor: value.in_consensus_subnet_reputation_factor.into(),
        not_in_consensus_subnet_reputation_factor: value
            .not_in_consensus_subnet_reputation_factor
            .into(),
        min_subnet_nodes: value.min_subnet_nodes,
        validator_identity_attestation_percentage: value
            .validator_identity_attestation_percentage
            .into(),
        min_subnet_node_reputation: value.min_subnet_node_reputation.into(),
        min_weight_decrease_reputation_threshold: value
            .min_weight_decrease_reputation_threshold
            .into(),
        subnet_delegate_stake_rewards_percentage: value
            .subnet_delegate_stake_rewards_percentage
            .into(),
        consensus_validator_node_count_decay: value.consensus_validator_node_count_decay.into(),
        consensus_validator_stake_weight_power: value.consensus_validator_stake_weight_power.into(),
        idle_classification_epochs: value.idle_classification_epochs,
        included_classification_epochs: value.included_classification_epochs,
        queue_immunity_epochs: value.queue_immunity_epochs,
        reputation_factors: rpc_reputation_factors(value.reputation_factors),
    }
}

impl<T: Config> Pallet<T> {
    fn to_rpc_subnet_info(value: SubnetInfo<T>) -> rpc::SubnetInfo<T::AccountId> {
        let reputation_factors = rpc::SubnetReputationFactors {
            absent_decrease: value.absent_decrease_reputation_factor.into(),
            included_increase: value.included_increase_reputation_factor.into(),
            below_min_weight_decrease: value.below_min_weight_decrease_reputation_factor.into(),
            non_attestor_decrease: value.non_attestor_decrease_reputation_factor.into(),
            non_consensus_attestor_decrease: value
                .non_consensus_attestor_decrease_reputation_factor
                .into(),
            validator_absent_decrease: value.validator_absent_subnet_node_reputation_factor.into(),
            validator_non_consensus_decrease: value
                .validator_non_consensus_subnet_node_reputation_factor
                .into(),
        };

        rpc::SubnetInfo {
            id: value.id,
            friendly_id: value.friendly_id,
            name: rpc_bytes(value.name),
            repo: rpc_bytes(value.repo),
            description: rpc_bytes(value.description),
            misc: rpc_bytes(value.misc),
            consensus_mechanism: rpc_consensus_mechanism(value.consensus_mechanism),
            state: rpc_subnet_state(value.state),
            consensus_eligible_from_subnet_epoch: value.consensus_eligible_from_subnet_epoch,
            pause_started_global_epoch: value.pause_started_global_epoch,
            pause_started_subnet_epoch: value.pause_started_subnet_epoch,
            churn_limit: value.churn_limit,
            churn_limit_multiplier: value.churn_limit_multiplier,
            min_stake: value.min_stake.into(),
            max_stake: value.max_stake.into(),
            queue_immunity_epochs: value.queue_immunity_epochs,
            pending_queue_immunity_epochs: value.pending_queue_immunity_epochs.map(|pending| {
                rpc::PendingValueUpdate {
                    value: pending.value,
                    effective_subnet_epoch: pending.effective_subnet_epoch,
                    owner: pending.owner,
                }
            }),
            target_node_registrations_per_epoch: value.target_node_registrations_per_epoch,
            node_registrations_this_epoch: value.node_registrations_this_epoch,
            subnet_node_queue_epochs: value.subnet_node_queue_epochs,
            pending_subnet_node_queue_epochs: value.pending_subnet_node_queue_epochs.map(
                |pending| rpc::PendingValueUpdate {
                    value: pending.value,
                    effective_subnet_epoch: pending.effective_subnet_epoch,
                    owner: pending.owner,
                },
            ),
            idle_classification_epochs: value.idle_classification_epochs,
            pending_idle_classification_epochs: value.pending_idle_classification_epochs.map(
                |pending| rpc::PendingValueUpdate {
                    value: pending.value,
                    effective_subnet_epoch: pending.effective_subnet_epoch,
                    owner: pending.owner,
                },
            ),
            included_classification_epochs: value.included_classification_epochs,
            pending_included_classification_epochs: value
                .pending_included_classification_epochs
                .map(|pending| rpc::PendingValueUpdate {
                    value: pending.value,
                    effective_subnet_epoch: pending.effective_subnet_epoch,
                    owner: pending.owner,
                }),
            delegate_stake_percentage: value.delegate_stake_percentage.into(),
            pending_delegate_stake_percentage: value.pending_delegate_stake_percentage.map(
                |pending| rpc::PendingValueUpdate {
                    value: pending.value.into(),
                    effective_subnet_epoch: pending.effective_subnet_epoch,
                    owner: pending.owner,
                },
            ),
            last_delegate_stake_rewards_update: value.last_delegate_stake_rewards_update,
            consensus_validator_node_count_decay: value.consensus_validator_node_count_decay.into(),
            pending_consensus_validator_node_count_decay: value
                .pending_consensus_validator_node_count_decay
                .map(|pending| rpc::PendingValueUpdate {
                    value: pending.value.into(),
                    effective_subnet_epoch: pending.effective_subnet_epoch,
                    owner: pending.owner,
                }),
            last_consensus_validator_node_count_decay_update: value
                .last_consensus_validator_node_count_decay_update,
            consensus_validator_stake_weight_power: value
                .consensus_validator_stake_weight_power
                .into(),
            pending_consensus_validator_stake_weight_power: value
                .pending_consensus_validator_stake_weight_power
                .map(|pending| rpc::PendingValueUpdate {
                    value: pending.value.into(),
                    effective_subnet_epoch: pending.effective_subnet_epoch,
                    owner: pending.owner,
                }),
            last_consensus_validator_stake_weight_power_update: value
                .last_consensus_validator_stake_weight_power_update,
            node_burn_rate_alpha: value.node_burn_rate_alpha.into(),
            current_node_burn_rate: value.current_node_burn_rate.into(),
            max_registered_nodes: value.max_registered_nodes,
            owner: value.owner,
            pending_owner: value.pending_owner,
            registration_epoch: value.registration_epoch,
            slot_index: value.slot_index,
            subnet_node_min_weight_decrease_reputation_threshold: value
                .subnet_node_min_weight_decrease_reputation_threshold
                .into(),
            pending_subnet_node_min_weight_decrease_reputation_threshold: value
                .pending_subnet_node_min_weight_decrease_reputation_threshold
                .map(|pending| rpc::PendingValueUpdate {
                    value: pending.value.into(),
                    effective_subnet_epoch: pending.effective_subnet_epoch,
                    owner: pending.owner,
                }),
            reputation: value.reputation.into(),
            min_subnet_node_reputation: value.min_subnet_node_reputation.into(),
            pending_min_subnet_node_reputation: value.pending_min_subnet_node_reputation.map(
                |pending| rpc::PendingValueUpdate {
                    value: pending.value.into(),
                    effective_subnet_epoch: pending.effective_subnet_epoch,
                    owner: pending.owner,
                },
            ),
            reputation_factors,
            pending_reputation_factors: value.pending_reputation_factors.map(|pending| {
                rpc::PendingSubnetReputationFactors {
                    effective_subnet_epoch: pending.effective_subnet_epoch,
                    factors: rpc_reputation_factors(pending.factors),
                }
            }),
            bootnode_access: value.bootnode_access.into_iter().collect(),
            bootnodes: value
                .bootnodes
                .into_iter()
                .map(|(peer_id, multiaddr)| rpc::BootnodeInfo {
                    peer_id: rpc_bytes(peer_id.0),
                    multiaddr: Some(rpc_bytes(multiaddr.into_inner())),
                })
                .collect(),
            total_nodes: value.total_nodes,
            total_active_nodes: value.total_active_nodes,
            total_electable_nodes: value.total_electable_nodes,
            current_min_delegate_stake: value.current_min_delegate_stake.into(),
            total_subnet_stake: value.total_subnet_stake.into(),
            total_subnet_delegate_stake_shares: value.total_subnet_delegate_stake_shares.into(),
            total_subnet_delegate_stake_balance: value.total_subnet_delegate_stake_balance.into(),
        }
    }

    pub fn rpc_get_subnet_info(subnet_id: u32) -> Option<rpc::SubnetInfo<T::AccountId>> {
        Self::get_subnet_info(subnet_id).map(Self::to_rpc_subnet_info)
    }

    pub fn rpc_get_subnets(
        request: rpc::PageRequest<u32>,
    ) -> Result<rpc::Page<rpc::SubnetInfo<T::AccountId>, u32>, rpc::NetworkQueryError> {
        let items = SubnetsData::<T>::iter_keys()
            .filter_map(Self::rpc_get_subnet_info)
            .collect();
        page_sorted(items, request, |item| item.id)
    }

    pub fn rpc_get_subnet_node_info(
        subnet_id: u32,
        subnet_node_id: u32,
    ) -> Option<rpc::SubnetNodeInfo<T::AccountId>> {
        Self::get_subnet_node_info(subnet_id, subnet_node_id).and_then(rpc_subnet_node_info::<T>)
    }

    pub fn rpc_get_subnet_nodes(
        subnet_id: u32,
        request: rpc::PageRequest<u32>,
    ) -> Result<rpc::Page<rpc::SubnetNodeInfo<T::AccountId>, u32>, rpc::NetworkQueryError> {
        if !SubnetsData::<T>::contains_key(subnet_id) {
            return Err(rpc::NetworkQueryError::SubnetNotFound { subnet_id });
        }
        let items = SubnetNodesData::<T>::iter_prefix(subnet_id)
            .filter_map(|(subnet_node_id, _)| {
                Self::rpc_get_subnet_node_info(subnet_id, subnet_node_id)
            })
            .collect();
        page_sorted(items, request, |item| item.subnet_node_id)
    }

    pub fn rpc_get_bootnodes(subnet_id: u32) -> Option<rpc::SubnetBootnodes> {
        if !SubnetsData::<T>::contains_key(subnet_id) {
            return None;
        }

        Some(rpc::SubnetBootnodes {
            official: SubnetBootnodes::<T>::get(subnet_id)
                .into_iter()
                .map(|(peer_id, multiaddr)| rpc::BootnodeInfo {
                    peer_id: rpc_bytes(peer_id.0),
                    multiaddr: Some(rpc_bytes(multiaddr.into_inner())),
                })
                .collect(),
            active_nodes: SubnetNodesData::<T>::iter_prefix(subnet_id)
                .filter_map(|(_, node)| {
                    let peer_info = node.bootnode_peer_info?;
                    let multiaddr = peer_info.multiaddr?;
                    Some(rpc::BootnodeInfo {
                        peer_id: rpc_bytes(peer_info.peer_id.0),
                        multiaddr: Some(rpc_bytes(multiaddr.into_inner())),
                    })
                })
                .collect(),
        })
    }

    pub fn rpc_get_validator_info(validator_id: u32) -> Option<rpc::ValidatorInfo<T::AccountId>> {
        let validator = ValidatorsData::<T>::try_get(validator_id).ok()?;
        let coldkey = ValidatorColdkey::<T>::get(validator_id)?;
        if ColdkeyValidatorId::<T>::get(&coldkey) != Some(validator_id)
            || HotkeyValidatorId::<T>::get(&validator.hotkey) != Some(validator_id)
        {
            return None;
        }

        Some(rpc::ValidatorInfo {
            id: validator_id,
            coldkey,
            hotkey: validator.hotkey,
            delegate_reward_rate: validator.delegate_reward_rate.into(),
            last_delegate_reward_rate_update: validator.last_delegate_reward_rate_update,
            delegate_account: validator
                .delegate_account
                .map(|delegate| rpc::DelegateAccountInfo {
                    account_id: delegate.account_id,
                    rate: delegate.rate.into(),
                }),
            identity: validator.identity.map(rpc_identity::<T>),
            reputation: rpc_validator_reputation(ValidatorReputation::<T>::get(validator_id)),
            delegate_pool_shares: ValidatorDelegateStakeShares::<T>::get(validator_id).into(),
            delegate_pool_balance: ValidatorDelegateStakeBalance::<T>::get(validator_id).into(),
            delegate_pool_slash_lock_until: ValidatorDelegateStakeSlashLockUntil::<T>::get(
                validator_id,
            ),
            last_node_allocation_update: LastValidatorNodeDelegateStakeWeightUpdate::<T>::get(
                validator_id,
            ),
        })
    }

    pub fn rpc_get_validator_by_coldkey(
        coldkey: &T::AccountId,
    ) -> Option<rpc::ValidatorInfo<T::AccountId>> {
        let validator_id = ColdkeyValidatorId::<T>::get(coldkey)?;
        let info = Self::rpc_get_validator_info(validator_id)?;
        (info.coldkey == *coldkey).then_some(info)
    }

    pub fn rpc_get_validator_by_hotkey(
        hotkey: &T::AccountId,
    ) -> Option<rpc::ValidatorInfo<T::AccountId>> {
        let validator_id = HotkeyValidatorId::<T>::get(hotkey)?;
        let info = Self::rpc_get_validator_info(validator_id)?;
        (info.hotkey == *hotkey).then_some(info)
    }

    pub fn rpc_get_validator_nodes(
        validator_id: u32,
        request: rpc::PageRequest<rpc::SubnetNodeCursor>,
    ) -> Result<
        rpc::Page<rpc::SubnetNodeInfo<T::AccountId>, rpc::SubnetNodeCursor>,
        rpc::NetworkQueryError,
    > {
        if !ValidatorsData::<T>::contains_key(validator_id) {
            return Err(rpc::NetworkQueryError::ValidatorNotFound { validator_id });
        }
        let items = ValidatorSubnetNodes::<T>::get(validator_id)
            .into_iter()
            .flat_map(|(subnet_id, node_ids)| {
                node_ids.into_iter().filter_map(move |subnet_node_id| {
                    Self::rpc_get_subnet_node_info(subnet_id, subnet_node_id)
                        .filter(|info| info.validator_id == validator_id)
                })
            })
            .collect();

        page_sorted(items, request, |item| rpc::SubnetNodeCursor {
            subnet_id: item.subnet_id,
            subnet_node_id: item.subnet_node_id,
        })
    }

    pub fn rpc_get_validator_node_stakes(
        validator_id: u32,
        request: rpc::PageRequest<rpc::SubnetNodeCursor>,
    ) -> Result<rpc::Page<rpc::ValidatorNodeStakeInfo, rpc::SubnetNodeCursor>, rpc::NetworkQueryError>
    {
        if !ValidatorsData::<T>::contains_key(validator_id) {
            return Err(rpc::NetworkQueryError::ValidatorNotFound { validator_id });
        }
        let items = ValidatorSubnetNodes::<T>::get(validator_id)
            .into_iter()
            .flat_map(|(subnet_id, node_ids)| {
                node_ids.into_iter().filter_map(move |subnet_node_id| {
                    Self::rpc_get_subnet_node_info(subnet_id, subnet_node_id)
                        .filter(|info| info.validator_id == validator_id)
                        .map(|_| rpc::ValidatorNodeStakeInfo {
                            subnet_id,
                            subnet_node_id,
                            balance: NodeSubnetStake::<T>::get(subnet_node_id, subnet_id).into(),
                        })
                })
            })
            .collect();

        page_sorted(items, request, |item| rpc::SubnetNodeCursor {
            subnet_id: item.subnet_id,
            subnet_node_id: item.subnet_node_id,
        })
    }

    pub fn rpc_get_validator_node_allocations(
        validator_id: u32,
        request: rpc::PageRequest<rpc::SubnetNodeCursor>,
    ) -> Result<rpc::ValidatorNodeAllocationsPage, rpc::NetworkQueryError> {
        if !ValidatorsData::<T>::contains_key(validator_id) {
            return Err(rpc::NetworkQueryError::ValidatorNotFound { validator_id });
        }
        let items = ValidatorNodeDelegateStakeWeights::<T>::get(validator_id)
            .into_iter()
            .map(
                |((subnet_id, subnet_node_id), weight)| rpc::ValidatorNodeAllocation {
                    subnet_id,
                    subnet_node_id,
                    weight: weight.into(),
                },
            )
            .collect();

        page_sorted(items, request, |item| rpc::SubnetNodeCursor {
            subnet_id: item.subnet_id,
            subnet_node_id: item.subnet_node_id,
        })
    }

    pub fn rpc_get_subnet_validator_nodes(
        subnet_id: u32,
        request: rpc::PageRequest<u32>,
    ) -> Result<rpc::SubnetValidatorNodesPage<T::AccountId>, rpc::NetworkQueryError> {
        if !SubnetsData::<T>::contains_key(subnet_id) {
            return Err(rpc::NetworkQueryError::SubnetNotFound { subnet_id });
        }
        let items = Self::get_validators_and_attestors(subnet_id)
            .into_iter()
            .filter_map(rpc_subnet_node_info::<T>)
            .collect();
        page_sorted(items, request, |item| item.subnet_node_id)
    }

    pub fn rpc_get_consensus_round(
        subnet_id: u32,
        subnet_epoch: u32,
    ) -> Result<Option<rpc::ConsensusRoundInfo>, rpc::NetworkQueryError> {
        let Some(round) = SubnetElectedValidator::<T>::get(subnet_id, subnet_epoch) else {
            return Ok(None);
        };
        let election_candidates = round
            .eligible_subnet_node_ids
            .iter()
            .map(|subnet_node_id| {
                round
                    .eligible_validator_identity_ids
                    .get(subnet_node_id)
                    .copied()
                    .map(|validator_id| rpc::ConsensusCandidateInfo {
                        subnet_node_id: *subnet_node_id,
                        validator_id,
                    })
                    .ok_or(rpc::NetworkQueryError::InconsistentState)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let proposal_args = SubnetConsensusProposalArgs::<T>::get(subnet_id, subnet_epoch);
        let proposal = SubnetConsensusSubmission::<T>::get(subnet_id, subnet_epoch)
            .map(|submission| {
                let weight_snapshot =
                    SubnetConsensusAttestorWeights::<T>::get(subnet_id, subnet_epoch)
                        .ok_or(rpc::NetworkQueryError::InconsistentState)?;
                let eligible_attestors = submission
                    .validator_ids
                    .iter()
                    .map(|subnet_node_id| {
                        let validator_id = submission
                            .validator_identity_ids
                            .get(subnet_node_id)
                            .copied()
                            .ok_or(rpc::NetworkQueryError::InconsistentState)?;
                        let attestation = submission.attests.get(subnet_node_id).map(|attest| {
                            rpc::ConsensusAttestationInfo {
                                block: attest.block,
                                progress: attest.attestor_progress.into(),
                                reward_factor: attest.reward_factor.into(),
                                data: SubnetConsensusAttestationData::<T>::get((
                                    subnet_id,
                                    subnet_epoch,
                                    *subnet_node_id,
                                ))
                                .map(|data| rpc_bytes(data.into_inner())),
                            }
                        });
                        let weight = weight_snapshot
                            .weights
                            .get(subnet_node_id)
                            .copied()
                            .ok_or(rpc::NetworkQueryError::InconsistentState)?;
                        Ok(rpc::ConsensusAttestorInfo {
                            subnet_node_id: *subnet_node_id,
                            validator_id,
                            weight: weight.into(),
                            attestation,
                        })
                    })
                    .collect::<Result<Vec<_>, rpc::NetworkQueryError>>()?;
                let emergency = submission
                    .emergency
                    .map(|emergency| rpc::EmergencyConsensusInfo {
                        subnet_node_ids: emergency.subnet_node_ids,
                        reputation_factors: rpc_reputation_factors(emergency.reputation_factors),
                        min_subnet_node_reputation: emergency.min_subnet_node_reputation.into(),
                        min_weight_decrease_reputation_threshold: emergency
                            .min_weight_decrease_reputation_threshold
                            .into(),
                    });

                Ok(rpc::ConsensusProposalInfo {
                    block: submission.block,
                    validator_epoch_progress: submission.validator_epoch_progress.into(),
                    validator_reward_factor: submission.validator_reward_factor.into(),
                    eligible_attestors,
                    active_subnet_node_ids: submission
                        .subnet_nodes
                        .into_iter()
                        .map(|node| node.id)
                        .collect(),
                    prioritize_queue_node_id: submission.prioritize_queue_node_id,
                    remove_queue_node_id: submission.remove_queue_node_id,
                    scores: submission
                        .data
                        .into_iter()
                        .map(|score| rpc::ConsensusNodeScore {
                            subnet_node_id: score.subnet_node_id,
                            score: score.score.into(),
                        })
                        .collect(),
                    args: proposal_args
                        .clone()
                        .map(|args| rpc_bytes(args.into_inner())),
                    emergency,
                })
            })
            .transpose()?;

        Ok(Some(rpc::ConsensusRoundInfo {
            subnet_id,
            subnet_epoch,
            status: if proposal.is_some() {
                rpc::ConsensusRoundStatus::Proposed
            } else {
                rpc::ConsensusRoundStatus::Elected
            },
            election_source: if round.emergency {
                rpc::ConsensusElectionSource::Emergency
            } else {
                rpc::ConsensusElectionSource::Regular
            },
            elected_subnet_node_id: round.validator_subnet_node_id,
            elected_validator_id: round.validator_id,
            validator_delegate_balance_at_election: round.validator_delegate_stake_balance.into(),
            election_candidates,
            policy: rpc_consensus_policy(round.policy),
            proposal,
        }))
    }

    pub fn rpc_get_subnet_epoch_status(
        subnet_id: u32,
    ) -> Result<rpc::SubnetEpochStatus, rpc::NetworkQueryError> {
        let subnet = SubnetsData::<T>::get(subnet_id)
            .ok_or(rpc::NetworkQueryError::SubnetNotFound { subnet_id })?;
        let epoch_length = T::EpochLength::get();
        if epoch_length == 0 {
            return Err(rpc::NetworkQueryError::InconsistentState);
        }
        let current_block = Self::get_current_block_as_u32();
        let timing = SubnetSlot::<T>::get(subnet_id).and_then(|slot| {
            if slot == 0 || current_block < slot {
                return None;
            }
            let offset = current_block.saturating_sub(slot);
            let subnet_epoch = offset / epoch_length;
            let start_block = slot.saturating_add(subnet_epoch.saturating_mul(epoch_length));
            let end_block = start_block.saturating_add(epoch_length);
            Some(rpc::SubnetEpochTiming {
                subnet_epoch,
                progression: Self::percent_div(
                    (offset % epoch_length) as u128,
                    epoch_length as u128,
                )
                .into(),
                start_block,
                end_block,
                blocks_remaining: end_block.saturating_sub(current_block),
            })
        });
        let current_epoch = timing.as_ref().map(|timing| timing.subnet_epoch);
        let consensus_eligible = subnet.state == SubnetState::Active
            && current_epoch
                .zip(subnet.consensus_eligible_from_subnet_epoch)
                .map(|(current, eligible_from)| current >= eligible_from)
                .unwrap_or(false);
        let elected_validator_subnet_node_id = current_epoch.and_then(|epoch| {
            SubnetElectedValidator::<T>::get(subnet_id, epoch)
                .map(|round| round.validator_subnet_node_id)
        });
        let proposal_submitted = current_epoch
            .map(|epoch| SubnetConsensusSubmission::<T>::contains_key(subnet_id, epoch))
            .unwrap_or(false);
        let emergency_data = EmergencySubnetNodeElectionData::<T>::get(subnet_id);
        let emergency_active = current_epoch
            .zip(emergency_data.as_ref())
            .map(|(epoch, data)| {
                data.activated && !Self::is_emergency_validator_set_expired(data, subnet_id, epoch)
            })
            .unwrap_or(false);

        Ok(rpc::SubnetEpochStatus {
            subnet_id,
            state: rpc_subnet_state(subnet.state),
            timing,
            consensus_eligible,
            within_proposal_attestation_window: Self::can_propose_or_attest_attestation(subnet_id),
            elected_validator_subnet_node_id,
            proposal_submitted,
            validator_set_source: if emergency_active {
                rpc::ConsensusElectionSource::Emergency
            } else {
                rpc::ConsensusElectionSource::Regular
            },
            pending_emergency_set: emergency_data.map(|data| !data.activated).unwrap_or(false),
        })
    }

    pub fn rpc_get_overwatch_node_info(
        overwatch_node_id: u32,
    ) -> Option<rpc::OverwatchNodeInfo<T::AccountId>> {
        OverwatchNodes::<T>::get(overwatch_node_id)?;
        let validator_id = OverwatchNodeValidatorId::<T>::get(overwatch_node_id)?;
        let validator = ValidatorsData::<T>::try_get(validator_id).ok()?;
        let coldkey = ValidatorColdkey::<T>::get(validator_id)?;
        if ColdkeyValidatorId::<T>::get(&coldkey) != Some(validator_id)
            || HotkeyValidatorId::<T>::get(&validator.hotkey) != Some(validator_id)
        {
            return None;
        }
        let hotkey = Self::get_overwatch_node_associated_hotkey(overwatch_node_id).ok()?;

        Some(rpc::OverwatchNodeInfo {
            overwatch_node_id,
            validator_id,
            coldkey,
            hotkey,
            peer_ids: OverwatchNodeIndex::<T>::get(overwatch_node_id)
                .into_iter()
                .map(|(subnet_id, peer_id)| rpc::OverwatchPeerInfo {
                    subnet_id,
                    peer_id: rpc_bytes(peer_id.0),
                })
                .collect(),
            reputation: rpc_validator_reputation(ValidatorReputation::<T>::get(validator_id)),
            stake_balance: OverwatchNodeStakeBalance::<T>::get(overwatch_node_id).into(),
        })
    }

    pub fn rpc_get_overwatch_nodes(
        request: rpc::PageRequest<u32>,
    ) -> Result<rpc::OverwatchNodesPage<T::AccountId>, rpc::NetworkQueryError> {
        let items = OverwatchNodes::<T>::iter_keys()
            .filter_map(Self::rpc_get_overwatch_node_info)
            .collect();
        page_sorted(items, request, |item| item.overwatch_node_id)
    }
}
