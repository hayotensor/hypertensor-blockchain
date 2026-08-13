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

use crate::*;
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
use frame_support::pallet_prelude::Pays;
use frame_support::pallet_prelude::Weight;
use libm::{exp, fmax, fmin};

impl<T: Config> Pallet<T> {
    pub(crate) fn canonicalize_consensus_data_entries(
        data: Vec<SubnetNodeConsensusData>,
    ) -> Result<(Vec<SubnetNodeConsensusData>, u128), Error<T>> {
        let mut lowest_scores: BTreeMap<u32, u128> = BTreeMap::new();

        for entry in data {
            lowest_scores
                .entry(entry.subnet_node_id)
                .and_modify(|score| {
                    if entry.score < *score {
                        *score = entry.score;
                    }
                })
                .or_insert(entry.score);
        }

        let mut weight_sum = 0u128;
        let mut canonical_data = Vec::with_capacity(lowest_scores.len());

        for (subnet_node_id, score) in lowest_scores {
            weight_sum = weight_sum
                .checked_add(score)
                .ok_or(Error::<T>::ScoreOverflow)?;
            canonical_data.push(SubnetNodeConsensusData {
                subnet_node_id,
                score,
            });
        }

        Ok((canonical_data, weight_sum))
    }

    fn canonicalize_consensus_data_for_submission(
        subnet_id: u32,
        subnet_epoch: u32,
        data: Vec<SubnetNodeConsensusData>,
    ) -> Result<(Vec<SubnetNodeConsensusData>, u128), Error<T>> {
        let filtered_data = data
            .into_iter()
            .filter(|entry| {
                SubnetNodesData::<T>::try_get(subnet_id, entry.subnet_node_id)
                    .map(|subnet_node| {
                        subnet_node.has_classification(&SubnetNodeClass::Included, subnet_epoch)
                    })
                    .unwrap_or(false)
            })
            .collect();

        Self::canonicalize_consensus_data_entries(filtered_data)
    }

    pub(crate) fn canonicalize_consensus_validator_ids(mut validator_ids: Vec<u32>) -> Vec<u32> {
        validator_ids.sort_unstable();
        validator_ids.dedup();
        validator_ids
    }

    pub(crate) fn snapshot_consensus_attestor_weights(
        subnet_id: u32,
        subnet_epoch: u32,
        validator_ids: &[u32],
    ) -> Result<ConsensusAttestorWeightSnapshot, Error<T>> {
        let policy = match SubnetElectedValidator::<T>::get(subnet_id, subnet_epoch) {
            Some(round) => round.policy,
            None => {
                #[cfg(test)]
                {
                    Self::consensus_policy_snapshot(subnet_id, subnet_epoch)
                }
                #[cfg(not(test))]
                {
                    return Err(Error::<T>::NoElectedValidator);
                }
            }
        };
        let mut validator_nodes: BTreeMap<u32, Vec<u32>> = BTreeMap::new();

        for subnet_node_id in validator_ids {
            let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, *subnet_node_id)
                .ok_or(Error::<T>::InvalidSubnetNodeId)?;
            validator_nodes
                .entry(validator_id)
                .or_default()
                .push(*subnet_node_id);
        }

        let mut weights = BTreeMap::new();
        let mut total_weight = 0u128;
        let node_count_decay = policy.consensus_validator_node_count_decay;
        let percentage_factor = Self::percentage_factor_as_u128();

        for (validator_id, subnet_node_ids) in validator_nodes {
            let validator_delegate_stake = ValidatorDelegateStakeBalance::<T>::get(validator_id);
            let validator_node_weights = ValidatorNodeDelegateStakeWeights::<T>::get(validator_id);
            let snapshotted_node_count = subnet_node_ids.len() as u128;
            let node_count = ValidatorSubnetNodes::<T>::get(validator_id)
                .get(&subnet_id)
                .map(|subnet_nodes| subnet_nodes.len() as u128)
                .unwrap_or(snapshotted_node_count)
                .max(snapshotted_node_count);

            for subnet_node_id in subnet_node_ids {
                let allocation = validator_node_weights
                    .get(&(subnet_id, subnet_node_id))
                    .copied()
                    .unwrap_or(0);
                let allocated_weight = Self::percent_mul(validator_delegate_stake, allocation);

                let node_weight = if allocated_weight > 0
                    && node_count > 1
                    && node_count_decay < percentage_factor
                {
                    let penalty_exponent = Self::get_percent_as_f64(
                        percentage_factor.saturating_sub(node_count_decay.min(percentage_factor)),
                    );
                    let divisor = Self::pow(node_count as f64, penalty_exponent);

                    if divisor.is_finite() && divisor > 0.0 {
                        let decayed_weight = allocated_weight as f64 / divisor;
                        if decayed_weight.is_finite() && decayed_weight > 0.0 {
                            decayed_weight as u128
                        } else {
                            0
                        }
                    } else {
                        allocated_weight
                    }
                } else {
                    allocated_weight
                };

                total_weight = total_weight
                    .checked_add(node_weight)
                    .ok_or(Error::<T>::AttestorWeightOverflow)?;
                weights.insert(subnet_node_id, node_weight);
            }
        }

        let stake_weight_power = policy
            .consensus_validator_stake_weight_power
            .min(percentage_factor);

        // Preserve the existing integer weights exactly when stake-weight diminishing is disabled.
        if stake_weight_power == percentage_factor || total_weight == 0 {
            return Ok(ConsensusAttestorWeightSnapshot {
                weights,
                total_weight,
            });
        }

        let exponent = Self::get_percent_as_f64(stake_weight_power);
        let raw_total_weight = total_weight as f64;
        let mut powered_total_weight = 0u128;

        for node_weight in weights.values_mut() {
            if *node_weight == 0 {
                continue;
            }

            let normalized_weight = *node_weight as f64 / raw_total_weight;
            let powered_weight =
                Self::get_f64_as_percentage(Self::pow(normalized_weight, exponent))
                    .min(percentage_factor);

            powered_total_weight = powered_total_weight
                .checked_add(powered_weight)
                .ok_or(Error::<T>::AttestorWeightOverflow)?;
            *node_weight = powered_weight;
        }

        Ok(ConsensusAttestorWeightSnapshot {
            weights,
            total_weight: powered_total_weight,
        })
    }

    /// Proposes attestation and submits consensus data for a subnet epoch.
    ///
    /// This function allows an elected validator to submit consensus data for their subnet,
    /// including peer scores, queue management decisions, and optional attestation data.
    ///
    /// The elected validator automatically attests to the data it submits.
    ///
    /// # Parameters
    ///
    /// * `subnet_id` - The ID of the subnet for which consensus data is being submitted.
    /// * `hotkey` - The hotkey of the elected validator submitting the consensus data.
    /// * `data` - A vector of consensus data containing scores for each peer in the subnet.
    ///   Duplicates based on `subnet_node_id` are collapsed to the lowest submitted score,
    ///   and only peers with `Included` classification are retained.
    /// * `prioritize_queue_node_id` - Optional node ID from the registration queue to move
    ///   to the front of the queue. The node must exist in the queue or this parameter is ignored.
    /// * `remove_queue_node_id` - Optional node ID from the registration queue to remove.
    ///   The node must exist in the queue and have passed the immunity period, or this
    ///   parameter is ignored.
    /// * `args` - Optional arbitrary arguments for subnet-specific use. This data is not
    ///   used in any onchain logic and is purely for subnet validator coordination.
    ///   This data can be useful within a subnet.
    /// * `attest_data` - Optional arbitrary attestation data. This data is not used in any
    ///   onchain logic but is included as part of the validator's automatic attestation
    ///   to their own consensus submission.
    ///   This data can be useful within a subnet.
    ///
    /// # Behavior
    ///
    /// The function performs the following steps:
    /// 1. Determines the current subnet epoch
    /// 2. Verifies the caller is the elected validator for this epoch
    /// 3. Ensures consensus has not already been submitted for this epoch
    /// 4. Qualifies the consensus data by:
    ///    - Filtering out non-Included peers
    ///    - Collapsing duplicate subnet node IDs to the lowest submitted score
    ///    - Validating scores don't overflow when summed
    /// 5. Validates queue operations (prioritize/remove) if specified
    /// 6. Stores the consensus submission with the proposer's automatic attestation
    ///
    /// # Errors
    ///
    /// * `NoElectedValidator` - No validator is elected for the current subnet epoch
    /// * `InvalidValidator` - The caller's hotkey doesn't match the elected validator
    /// * `SubnetRewardsAlreadySubmitted` - Consensus has already been submitted for this epoch
    /// * `ScoreOverflow` - The sum of all scores would overflow u128
    ///
    /// # Returns
    ///
    /// Returns `Ok(Pays::No.into())` on success, indicating the transaction fee is waived.
    pub fn do_propose_attestation(
        hotkey: T::AccountId,
        subnet_id: u32,
        data: Vec<SubnetNodeConsensusData>,
        mut prioritize_queue_node_id: Option<u32>,
        mut remove_queue_node_id: Option<u32>,
        args: Option<ValidatorArgs<T>>,
        attest_data: Option<ValidatorArgs<T>>,
    ) -> DispatchResultWithPostInfo {
        // The validator is elected for the next blockchain epoch where rewards will be distributed.
        // Each subnet epoch overlaps with the blockchains epochs, and can submit consensus data for epoch
        // 2 on subnet epoch 1 (if after slot) or 2 (if before slot).
        // If a subnet is on slot 3 of 5 slots, we make sure it can submit on the current blockchains epoch.

        // Get the current subnet epoch and subnet epoch progression for this specific subnet
        let subnet_epoch_data = Self::get_current_subnet_epoch_data(subnet_id)
            .ok_or(Error::<T>::SubnetEpochDataIsNone)?;

        let subnet_epoch = subnet_epoch_data.subnet_epoch;
        let subnet_epoch_progression = subnet_epoch_data.subnet_epoch_progression;

        // --- Ensure validator was elected
        let elected_round = SubnetElectedValidator::<T>::get(subnet_id, subnet_epoch)
            .ok_or(Error::<T>::NoElectedValidator)?;
        let validator_subnet_node_id = elected_round.validator_subnet_node_id;
        let policy = elected_round.policy;

        // The caller does not supply authority-bearing node identity. The elected subnet node is
        // derived from storage, then the signed hotkey must own that exact node.
        Self::ensure_hotkey_owns_subnet_node(subnet_id, validator_subnet_node_id, &hotkey)?;

        // - Note: we don't check stake balance here. It's up to subnets to come to a consensus
        // to remove nodes that are not meeting the subnet's requirements. Stake balance only matters
        // on the node registration.

        // --- Ensure not submitted already
        ensure!(
            !SubnetConsensusSubmission::<T>::contains_key(subnet_id, subnet_epoch),
            Error::<T>::SubnetRewardsAlreadySubmitted
        );

        //
        // --- Qualify the data
        //

        // Remove queue classified entries, collapse duplicate node IDs to the lowest score,
        // and ensure the canonical score sum does not overflow.
        let (data, _) =
            Self::canonicalize_consensus_data_for_submission(subnet_id, subnet_epoch, data)?;

        let block: u32 = Self::get_current_block_as_u32();
        let attests: BTreeMap<u32, AttestEntry<T>> = BTreeMap::from([(
            validator_subnet_node_id,
            AttestEntry::<T> {
                block,
                attestor_progress: 0,
                reward_factor: Self::percentage_factor_as_u128(),
                data: attest_data,
            },
        )]);

        // --- Get all (activated) Idle + consensus-eligible nodes
        // We get this here instead of in the rewards distribution to handle block weight more efficiently
        // during block steps (on_initialize). As well, we get this here to define the point of which
        // nodes are eligible for rewards. If a node were to remove itself after attesting, and is here
        // when the validator submit their data, this will enable the node to still get rewarded for contributing
        // to the subnet's consensus even if they leave -- versus calling this in the rewards distribution
        // where the node would have already been removed even if they contributed to the subnet's consensus.
        let subnet_nodes: Vec<SubnetNode<T>> = Self::get_active_classified_subnet_nodes(
            subnet_id,
            &SubnetNodeClass::Idle,
            subnet_epoch,
        );

        // --- Get all validators
        // Note: This is triggered here when the validator submits their data, not at the start block of the epoch
        //
        // These are the nodes that can attest to the consensus data
        //
        // We store `validator_ids` in `ConsensusData<T>` because the emergency validator set can be different from
        // the regular validator set and we need to know who to count as attestors officially. And we use the
        // call of this function as the official point of time of which nodes can attest on this epoch.
        //
        // This is in case the owner "suedo-forks" or pauses the subnet after the validator has submitted their data.
        let (validator_ids, emergency_active) =
            Self::effective_consensus_validator_ids(subnet_id, subnet_epoch);
        let emergency_snapshot = if emergency_active {
            EmergencySubnetNodeElectionData::<T>::get(subnet_id)
                .map(|data| Self::emergency_consensus_snapshot(&data, validator_ids.clone()))
        } else {
            None
        };

        if emergency_snapshot.is_some()
            && (prioritize_queue_node_id.is_some() || remove_queue_node_id.is_some())
        {
            return Err(Error::<T>::EmergencyQueueMutationNotAllowed.into());
        }

        let validator_identity_ids = validator_ids
            .iter()
            .map(|subnet_node_id| {
                SubnetNodeValidatorId::<T>::get(subnet_id, *subnet_node_id)
                    .map(|validator_id| (*subnet_node_id, validator_id))
                    .ok_or(Error::<T>::InvalidSubnetNodeId)
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let attestor_weight_snapshot =
            Self::snapshot_consensus_attestor_weights(subnet_id, subnet_epoch, &validator_ids)?;

        // Check if validator sent through queue priority or removal node IDs
        if prioritize_queue_node_id.is_some() || remove_queue_node_id.is_some() {
            let queue = SubnetNodeQueue::<T>::get(subnet_id);
            let immunity_epochs = policy.queue_immunity_epochs;

            let mut prioritize_exists = prioritize_queue_node_id.is_none();
            let mut remove_allowed = remove_queue_node_id.is_none(); // Rename for clarity

            // Single pass through the queue to check both nodes exist in the queue
            for node in &queue {
                if let Some(node_id) = prioritize_queue_node_id {
                    if node.id == node_id {
                        prioritize_exists = true;
                    }
                }

                if let Some(node_id) = remove_queue_node_id {
                    if node.id == node_id {
                        // Node exists AND has passed immunity period
                        remove_allowed = Self::has_epoch_period_elapsed(
                            node.classification.start_epoch,
                            immunity_epochs,
                            subnet_epoch,
                        );
                    }
                }

                if prioritize_exists && (remove_queue_node_id.is_none() || remove_allowed) {
                    break;
                }
            }

            // Update parameters based on checks
            if !prioritize_exists {
                prioritize_queue_node_id = None;
            }

            if !remove_allowed {
                remove_queue_node_id = None;
            }
        }

        // Organize all of the data into a ConsensusData<T> struct to be used later for emissions business logic.
        let consensus_data: ConsensusData<T> = ConsensusData::<T> {
            validator_id: validator_subnet_node_id,
            block,
            validator_epoch_progress: subnet_epoch_progression,
            validator_reward_factor: Self::get_validator_reward_multiplier_for_policy(
                subnet_epoch_progression,
                &policy,
            ),
            attests: attests,
            validator_ids,
            validator_identity_ids,
            subnet_nodes: subnet_nodes,
            prioritize_queue_node_id: prioritize_queue_node_id,
            remove_queue_node_id: remove_queue_node_id,
            data: data,
            args: args,
            emergency: emergency_snapshot,
        };

        // --- Store the data
        SubnetConsensusSubmission::<T>::insert(subnet_id, subnet_epoch, consensus_data);
        SubnetConsensusAttestorWeights::<T>::insert(
            subnet_id,
            subnet_epoch,
            attestor_weight_snapshot,
        );

        Self::deposit_event(Event::ValidatorSubmission {
            subnet_id: subnet_id,
            account_id: hotkey,
            epoch: subnet_epoch,
        });

        // If we make it this far, the extrinsic call is free.
        Ok(Pays::No.into())
    }

    pub fn do_attest(
        hotkey: T::AccountId,
        subnet_id: u32,
        subnet_node_id: u32,
        data: Option<ValidatorArgs<T>>,
    ) -> DispatchResultWithPostInfo {
        let subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);

        let subnet_node = Self::ensure_hotkey_owns_subnet_node(subnet_id, subnet_node_id, &hotkey)?;

        // --- Ensure node classified to attest
        ensure!(
            subnet_node.has_classification(&SubnetNodeClass::Validator, subnet_epoch),
            Error::<T>::InvalidSubnetNodeClassification
        );

        // - Note: we don't check stake balance here

        // Preserve the submission error as the primary failure for an epoch that
        // has no proposal. The elected round is only meaningful to an existing
        // submission in this path.
        ensure!(
            SubnetConsensusSubmission::<T>::contains_key(subnet_id, subnet_epoch),
            Error::<T>::InvalidSubnetConsensusSubmission
        );

        let policy = SubnetElectedValidator::<T>::get(subnet_id, subnet_epoch)
            .ok_or(Error::<T>::NoElectedValidator)?
            .policy;

        let block: u32 = Self::get_current_block_as_u32();

        // We make sure the submission exists in order to attest to it
        SubnetConsensusSubmission::<T>::try_mutate_exists(
            subnet_id,
            subnet_epoch,
            |maybe_params| -> DispatchResult {
                let params = maybe_params
                    .as_mut()
                    .ok_or(Error::<T>::InvalidSubnetConsensusSubmission)?;

                // Ensure they are in the validator list and are eligible to attest
                // Only validator classified nodes can attest
                //
                // See `do_propose_attestation` for the logic of how the validator set is determined as the
                // official point of truth.
                let validator_ids = &mut params.validator_ids;
                ensure!(
                    validator_ids
                        .iter()
                        .any(|validator_id| *validator_id == subnet_node_id),
                    Error::<T>::InvalidValidatorId
                );

                // Get the epoch progression used to determine the reward factor.
                let proposal_block = params.block;
                let subnet_epoch_data = Self::attestor_subnet_epoch_data(subnet_id, proposal_block)
                    .ok_or(Error::<T>::SubnetEpochDataIsNone)?;
                let subnet_epoch_progression = subnet_epoch_data.subnet_epoch_progression;

                // Get the reward factor.
                // The longer a node takes to attest, the lower its emissions will be.
                let reward_factor = Self::get_attestor_reward_multiplier_for_policy(
                    subnet_epoch_progression,
                    &policy,
                );

                let mut attests = &mut params.attests;

                // Ensure they haven't attested already
                ensure!(
                    attests.insert(
                        subnet_node_id,
                        AttestEntry::<T> {
                            block,
                            attestor_progress: subnet_epoch_progression,
                            reward_factor,
                            data
                        }
                    ) == None,
                    Error::<T>::AlreadyAttested
                );

                params.attests = attests.clone();
                Ok(())
            },
        )?;

        Self::deposit_event(Event::Attestation {
            subnet_id: subnet_id,
            subnet_node_id: subnet_node_id,
            epoch: subnet_epoch,
        });

        // If we make it this far, the extrinsic call is free.
        Ok(Pays::No.into())
    }

    pub fn get_validator_reward_multiplier(progress: u128) -> u128 {
        let policy = ConsensusPolicySnapshot {
            validator_reward_midpoint: ValidatorRewardMidpoint::<T>::get(),
            validator_reward_k: ValidatorRewardK::<T>::get(),
            ..Default::default()
        };
        Self::get_validator_reward_multiplier_for_policy(progress, &policy)
    }

    pub fn get_validator_reward_multiplier_for_policy(
        progress: u128,
        policy: &ConsensusPolicySnapshot,
    ) -> u128 {
        Self::get_f64_as_percentage(Self::sigmoid_decreasing(
            Self::get_percent_as_f64(progress),
            Self::get_percent_as_f64(policy.validator_reward_midpoint),
            policy.validator_reward_k as f64,
            0.0,
            1.0,
        ))
        .clamp(0, Self::percentage_factor_as_u128())

        // Self::get_f64_as_percentage(Self::sigmoid_decreasing_start_offset(
        //     Self::get_percent_as_f64(progress),
        //     Self::get_percent_as_f64(ValidatorRewardMidpoint::<T>::get()),
        //     ValidatorRewardK::<T>::get() as f64,
        //     0.05, // x offset (gives leeway for submission so it doesn't need to be on block step 0 to get 100%)
        //     4.0,
        // ))
        // .clamp(0, Self::percentage_factor_as_u128())
    }

    pub fn get_attestor_reward_multiplier(progress: u128) -> u128 {
        let policy = ConsensusPolicySnapshot {
            attestor_min_reward_factor: AttestorMinRewardFactor::<T>::get(),
            attestor_reward_exponent: AttestorRewardExponent::<T>::get(),
            ..Default::default()
        };
        Self::get_attestor_reward_multiplier_for_policy(progress, &policy)
    }

    pub fn get_attestor_reward_multiplier_for_policy(
        progress: u128,
        policy: &ConsensusPolicySnapshot,
    ) -> u128 {
        Self::get_f64_as_percentage(Self::concave_down_decreasing(
            Self::get_percent_as_f64(progress),
            Self::get_percent_as_f64(policy.attestor_min_reward_factor),
            1.0,
            policy.attestor_reward_exponent as f64,
        ))
        .clamp(0, Self::percentage_factor_as_u128())
    }

    /// Return the validators reward that submitted data on the previous epoch
    // The attestation percentage must be greater than the MinAttestationPercentage
    pub fn get_validator_reward(attestation_percentage: u128, reward_factor: u128) -> u128 {
        Self::get_validator_reward_with_policy(
            attestation_percentage,
            reward_factor,
            T::MinAttestationPercentage::get(),
            BaseValidatorReward::<T>::get(),
        )
    }

    pub fn get_validator_reward_with_policy(
        attestation_percentage: u128,
        reward_factor: u128,
        min_attestation_percentage: u128,
        base_validator_reward: u128,
    ) -> u128 {
        if min_attestation_percentage > attestation_percentage {
            return 0;
        }
        Self::percent_mul(base_validator_reward, reward_factor)
    }

    /// Slash subnet validator node
    ///
    /// # Arguments
    ///
    /// * `subnet_id` - Subnet ID
    /// * `subnet_node_id` - Subnet node ID
    /// * `attestation_percentage` - The selected consensus-failure ratio used for economics
    /// * `min_attestation_percentage` - The selected consensus-failure threshold
    /// * `identity_attestation_percentage` - Distinct-validator-identity proposal support
    /// * `coldkey_reputation_decrease_factor`: `ValidatorReputationDecreaseFactor`
    /// * `validator_non_consensus_reputation_factor`: Resolved subnet node factor for this epoch
    /// * `identity_reputation_shortfall`: Precomputed distinct-identity rejection severity
    pub fn slash_validator(
        subnet_id: u32,
        subnet_node_id: u32,
        attestation_percentage: u128,
        min_attestation_percentage: u128,
        identity_attestation_percentage: u128,
        coldkey_reputation_decrease_factor: u128,
        min_validator_reputation: u128,
        electable_nodes: u32,
        validator_non_consensus_reputation_factor: u128,
        identity_reputation_shortfall: Option<u128>,
    ) -> Weight {
        Self::slash_validator_with_policy(
            subnet_id,
            subnet_node_id,
            attestation_percentage,
            min_attestation_percentage,
            identity_attestation_percentage,
            coldkey_reputation_decrease_factor,
            min_validator_reputation,
            electable_nodes,
            validator_non_consensus_reputation_factor,
            identity_reputation_shortfall,
            BaseSlashPercentage::<T>::get(),
            MaxSlashAmount::<T>::get(),
        )
    }

    pub fn slash_validator_with_policy(
        subnet_id: u32,
        subnet_node_id: u32,
        attestation_percentage: u128,
        min_attestation_percentage: u128,
        identity_attestation_percentage: u128,
        coldkey_reputation_decrease_factor: u128,
        min_validator_reputation: u128,
        electable_nodes: u32,
        validator_non_consensus_reputation_factor: u128,
        identity_reputation_shortfall: Option<u128>,
        base_slash_percentage: u128,
        max_slash_amount: u128,
    ) -> Weight {
        Self::slash_validator_for_round_with_policy(
            subnet_id,
            subnet_node_id,
            attestation_percentage,
            min_attestation_percentage,
            identity_attestation_percentage,
            coldkey_reputation_decrease_factor,
            min_validator_reputation,
            electable_nodes,
            validator_non_consensus_reputation_factor,
            identity_reputation_shortfall,
            base_slash_percentage,
            max_slash_amount,
            attestation_percentage,
            0,
            0,
            0,
            0,
        )
    }

    /// Apply snapshotted economic policy and reputation penalties to a submitted round.
    /// Direct node-stake severity continues to use the selected consensus-failure ratio, while
    /// delegate-pool severity always uses the stake-weighted attestation ratio. Proposer node
    /// reputation uses only the caller-provided distinct-identity support and strong-rejection
    /// shortfall.
    pub fn slash_validator_for_round_with_policy(
        subnet_id: u32,
        subnet_node_id: u32,
        attestation_percentage: u128,
        min_attestation_percentage: u128,
        identity_attestation_percentage: u128,
        coldkey_reputation_decrease_factor: u128,
        min_validator_reputation: u128,
        electable_nodes: u32,
        validator_non_consensus_reputation_factor: u128,
        identity_reputation_shortfall: Option<u128>,
        base_slash_percentage: u128,
        max_slash_amount: u128,
        stake_attestation_percentage: u128,
        snapshotted_validator_delegate_stake_balance: u128,
        validator_delegate_stake_slash_threshold: u128,
        base_validator_delegate_stake_slash_percentage: u128,
        max_validator_delegate_stake_slash_amount: u128,
    ) -> Weight {
        let mut weight = Weight::zero();
        let db_weight = T::DbWeight::get();

        let identity_reputation_shortfall = identity_reputation_shortfall
            .map(|shortfall| shortfall.min(Self::percentage_factor_as_u128()));
        let proposer_node_reputation_shortfall =
            identity_reputation_shortfall.filter(|shortfall| {
                Self::percent_mul(validator_non_consensus_reputation_factor, *shortfall) > 0
            });
        let economic_consensus_failed = attestation_percentage < min_attestation_percentage;

        let validator_id = if economic_consensus_failed {
            let (validator_id, _, _, slash_weight) = Self::apply_validator_economic_slashes(
                subnet_id,
                subnet_node_id,
                attestation_percentage,
                min_attestation_percentage,
                base_slash_percentage,
                max_slash_amount,
                stake_attestation_percentage,
                snapshotted_validator_delegate_stake_balance,
                validator_delegate_stake_slash_threshold,
                base_validator_delegate_stake_slash_percentage,
                max_validator_delegate_stake_slash_amount,
            );
            weight = weight.saturating_add(slash_weight);
            validator_id
        } else {
            // Identity reputation and its support statistics are independent from economic
            // consensus, so resolve the validator even when the economic ratio passed.
            weight = weight.saturating_add(db_weight.reads(1));
            SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id)
        };

        if let Some(validator_id) = validator_id {
            Self::decrease_validator_reputation(
                validator_id,
                identity_attestation_percentage,
                identity_reputation_shortfall,
                coldkey_reputation_decrease_factor,
            );
            // ValidatorReputation::contains_key + get + insert.
            weight = weight.saturating_add(db_weight.reads_writes(2, 1));
        }

        let reputation = if let Some(identity_shortfall) = proposer_node_reputation_shortfall {
            weight = weight.saturating_add(db_weight.reads(1));
            SubnetNodeReputation::<T>::get(subnet_id, subnet_node_id).map(|rep| {
                let new_reputation = Self::decrease_and_return_node_reputation(
                    subnet_id,
                    subnet_node_id,
                    rep,
                    validator_non_consensus_reputation_factor,
                    Some(identity_shortfall),
                );
                // try_mutate_exists, plus the NodeReputationUpdate event's System event storage
                // accesses. The explicit `get` read is metered above even when no entry exists.
                weight = weight.saturating_add(db_weight.reads_writes(5, 3));
                new_reputation
            })
        } else {
            None
        };

        // Remove validator if below min node reputation. Callers that need the proposer to receive
        // a subsequent attestor-role decrease pass a zero minimum and perform the final check later.
        if let (Some(validator_id), Some(reputation)) = (validator_id, reputation) {
            if reputation < min_validator_reputation {
                weight = weight.saturating_add(db_weight.reads(1));
                let validator_subnet_nodes = ValidatorSubnetNodes::<T>::get(validator_id);
                // x = number of subnets (outer BTreeMap size)
                let x = validator_subnet_nodes.len() as u32;
                // c = number of nodes in the specific subnet (inner BTreeSet size)
                let c = validator_subnet_nodes
                    .get(&subnet_id)
                    .map(|nodes| nodes.len() as u32)
                    .unwrap_or(0);

                Self::remove_active_subnet_node(subnet_id, subnet_node_id);
                weight = weight.saturating_add(T::WeightInfo::remove_active_subnet_node(
                    x,
                    electable_nodes,
                    c,
                ));
            }
        }

        weight
    }

    /// Apply only economic losses. Missing proposals call this directly so their existing
    /// absence-reputation penalties are not duplicated by the submitted-round path above.
    pub(crate) fn apply_validator_economic_slashes(
        subnet_id: u32,
        subnet_node_id: u32,
        node_attestation_percentage: u128,
        node_slash_threshold: u128,
        base_slash_percentage: u128,
        max_slash_amount: u128,
        stake_attestation_percentage: u128,
        snapshotted_validator_delegate_stake_balance: u128,
        validator_delegate_stake_slash_threshold: u128,
        base_validator_delegate_stake_slash_percentage: u128,
        max_validator_delegate_stake_slash_amount: u128,
    ) -> (Option<u32>, u128, u128, Weight) {
        let mut weight = Weight::zero();
        let db_weight = T::DbWeight::get();

        let node_stake_amount = if node_attestation_percentage < node_slash_threshold {
            let attestation_delta = Self::percentage_factor_as_u128().saturating_sub(
                Self::percent_div(node_attestation_percentage, node_slash_threshold)
                    .min(Self::percentage_factor_as_u128()),
            );
            let account_subnet_stake = NodeSubnetStake::<T>::get(subnet_node_id, subnet_id);
            weight = weight.saturating_add(db_weight.reads(1));
            let amount = Self::get_slash_amount_with_policy(
                account_subnet_stake,
                node_attestation_percentage,
                node_slash_threshold,
                attestation_delta,
                base_slash_percentage,
                max_slash_amount,
            );

            if amount > 0 {
                Self::decrease_node_stake(subnet_node_id, subnet_id, amount);
                weight = weight.saturating_add(db_weight.reads_writes(3, 3));
            }
            amount
        } else {
            0
        };

        // Resolve the identity before reputation processing can remove the node.
        let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id);
        weight = weight.saturating_add(db_weight.reads(1));

        let validator_delegate_stake_amount = if let Some(validator_id) = validator_id {
            if base_validator_delegate_stake_slash_percentage > 0
                && max_validator_delegate_stake_slash_amount > 0
                && stake_attestation_percentage < validator_delegate_stake_slash_threshold
            {
                let current_pool_balance = ValidatorDelegateStakeBalance::<T>::get(validator_id);
                weight = weight.saturating_add(db_weight.reads(1));
                let amount = Self::get_validator_delegate_stake_slash_amount(
                    snapshotted_validator_delegate_stake_balance,
                    current_pool_balance,
                    stake_attestation_percentage,
                    validator_delegate_stake_slash_threshold,
                    base_validator_delegate_stake_slash_percentage,
                    max_validator_delegate_stake_slash_amount,
                );

                if amount > 0 {
                    ValidatorDelegateStakeBalance::<T>::insert(
                        validator_id,
                        current_pool_balance.saturating_sub(amount),
                    );
                    TotalValidatorDelegateStakeBalance::<T>::mutate(|total| {
                        total.saturating_reduce(amount)
                    });
                    weight = weight.saturating_add(db_weight.reads_writes(1, 2));
                }
                amount
            } else {
                0
            }
        } else {
            0
        };

        if let Some(validator_id) = validator_id {
            // Every economic penalty produces one combined audit event, including launch-safe
            // rounds whose snapshotted delegate-pool loss is zero.
            Self::deposit_event(Event::ValidatorSlashApplied {
                subnet_id,
                validator_id,
                subnet_node_id,
                attestation_percentage: stake_attestation_percentage,
                node_stake_amount,
                validator_delegate_stake_amount,
            });
            // System::Number | System::ExecutionPhase | System::EventCount | System::Events
            weight = weight.saturating_add(db_weight.reads_writes(4, 2));
        }

        (
            validator_id,
            node_stake_amount,
            validator_delegate_stake_amount,
            weight,
        )
    }

    /// Calculate the proportional validator-pool loss for one elected round.
    pub fn get_validator_delegate_stake_slash_amount(
        snapshotted_pool_balance: u128,
        current_pool_balance: u128,
        attestation_percentage: u128,
        validator_delegate_stake_slash_threshold: u128,
        base_validator_delegate_stake_slash_percentage: u128,
        max_validator_delegate_stake_slash_amount: u128,
    ) -> u128 {
        if snapshotted_pool_balance == 0
            || current_pool_balance == 0
            || validator_delegate_stake_slash_threshold == 0
            || base_validator_delegate_stake_slash_percentage == 0
            || max_validator_delegate_stake_slash_amount == 0
            || attestation_percentage >= validator_delegate_stake_slash_threshold
        {
            return 0;
        }

        let delegate_shortfall = Self::percentage_factor_as_u128().saturating_sub(
            Self::percent_div(
                attestation_percentage,
                validator_delegate_stake_slash_threshold,
            )
            .min(Self::percentage_factor_as_u128()),
        );
        let base_pool_slash = Self::percent_mul(
            snapshotted_pool_balance,
            base_validator_delegate_stake_slash_percentage,
        );
        let proportional_pool_slash = Self::percent_mul(base_pool_slash, delegate_shortfall);

        proportional_pool_slash
            .min(snapshotted_pool_balance)
            .min(current_pool_balance)
            .min(max_validator_delegate_stake_slash_amount)
    }

    pub fn get_slash_amount(
        account_subnet_stake: u128,
        attestation_percentage: u128,
        min_attestation_percentage: u128,
        attestation_delta: u128,
    ) -> u128 {
        Self::get_slash_amount_with_policy(
            account_subnet_stake,
            attestation_percentage,
            min_attestation_percentage,
            attestation_delta,
            BaseSlashPercentage::<T>::get(),
            MaxSlashAmount::<T>::get(),
        )
    }

    pub fn get_slash_amount_with_policy(
        account_subnet_stake: u128,
        attestation_percentage: u128,
        min_attestation_percentage: u128,
        attestation_delta: u128,
        base_slash_percentage: u128,
        max_slash_amount: u128,
    ) -> u128 {
        // --- Get slash amount up to max slash
        // --- Base slash amount
        // stake balance * BaseSlashPercentage
        let base_slash: u128 = Self::percent_mul(account_subnet_stake, base_slash_percentage);

        // --- Update slash amount based on delta
        // base_slash * attestation_delta
        let mut slash_amount = Self::percent_mul(base_slash, attestation_delta);

        // --- Update slash amount up to max slash
        if slash_amount > max_slash_amount {
            slash_amount = max_slash_amount
        }

        slash_amount
    }
}
