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
        validator_ids: &[u32],
    ) -> Result<ConsensusAttestorWeightSnapshot, Error<T>> {
        let mut validator_nodes: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        let mut unmapped_nodes = Vec::new();

        for subnet_node_id in validator_ids {
            if let Some(validator_id) = SubnetNodeValidatorId::<T>::get(subnet_id, *subnet_node_id)
            {
                validator_nodes
                    .entry(validator_id)
                    .or_default()
                    .push(*subnet_node_id);
            } else {
                unmapped_nodes.push(*subnet_node_id);
            }
        }

        let mut weights = BTreeMap::new();
        let mut total_weight = 0u128;
        let node_count_decay = ConsensusValidatorNodeCountDecay::<T>::get();
        let percentage_factor = Self::percentage_factor_as_u128();

        for subnet_node_id in unmapped_nodes {
            total_weight = total_weight
                .checked_add(1)
                .ok_or(Error::<T>::AttestorWeightOverflow)?;
            weights.insert(subnet_node_id, 1);
        }

        for (validator_id, subnet_node_ids) in validator_nodes {
            let validator_weight = ValidatorDelegateStakeBalance::<T>::get(validator_id).max(1);
            let node_count = subnet_node_ids.len() as u128;
            let effective_validator_weight =
                if node_count > 1 && node_count_decay < percentage_factor {
                    let penalty_exponent = 1.0 - Self::get_percent_as_f64(node_count_decay);
                    let divisor = Self::pow(node_count as f64, penalty_exponent);
                    let decayed_weight = validator_weight as f64 / divisor;

                    if decayed_weight.is_finite() {
                        (decayed_weight as u128).max(1)
                    } else {
                        validator_weight
                    }
                } else {
                    validator_weight
                };
            let base_node_weight = effective_validator_weight / node_count;
            let remainder = effective_validator_weight % node_count;

            total_weight = total_weight
                .checked_add(effective_validator_weight)
                .ok_or(Error::<T>::AttestorWeightOverflow)?;

            for (index, subnet_node_id) in subnet_node_ids.into_iter().enumerate() {
                let mut node_weight = base_node_weight;
                if (index as u128) < remainder {
                    node_weight = node_weight.saturating_add(1);
                }
                weights.insert(subnet_node_id, node_weight);
            }
        }

        Ok(ConsensusAttestorWeightSnapshot {
            weights,
            total_weight,
        })
    }

    /// Proposes attestation and submits consensus data for a subnet epoch.
    ///
    /// This function allows an elected validator to submit consensus data for their subnet,
    /// including peer scores, queue management decisions, and optional attestation data.
    ///
    /// The validator automatically attests to their own submission.
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
    /// 6. Stores the consensus submission with the validator's auto-attestation
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
        subnet_node_id: u32,
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

        // 1. Ensure caller owns the hotkey of the subnet node ID entered
        // 2. Ensure the subnet node Id owned by the caller is the elected validator

        // Ensure caller (hotkey) is the hotkey for the subnet node ID
        ensure!(
            Self::get_subnet_node_associated_hotkey(subnet_id, subnet_node_id,)? == hotkey,
            Error::<T>::InvalidValidator
        );

        // Ensure the subnet node ID is the elected validator

        // Get the current subnet epoch and subnet epoch progression for this specific subnet
        let subnet_epoch_data = Self::get_current_subnet_epoch_data(subnet_id)
            .ok_or(Error::<T>::SubnetEpochDataIsNone)?;

        let subnet_epoch = subnet_epoch_data.subnet_epoch;
        let subnet_epoch_progression = subnet_epoch_data.subnet_epoch_progression;

        // --- Ensure validator was elected
        let validator_subnet_node_id = SubnetElectedValidator::<T>::get(subnet_id, subnet_epoch)
            .ok_or(Error::<T>::NoElectedValidator)?;

        ensure!(
            subnet_node_id == validator_subnet_node_id,
            Error::<T>::InvalidSubnetNodeId
        );

        // // The elected validator can act with its node-specific override hotkey, or with the
        // // validator hotkey when no node-specific override exists.
        // ensure!(
        //     Self::get_hotkey_associated_subnet_node(
        //         subnet_id,
        //         subnet_node_id,
        //         validator_id,
        //         hotkey.clone(),
        //     )? == subnet_node_id,
        //     Error::<T>::InvalidValidator
        // );

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

        // --- Validator auto-attests the epoch
        // let attests: BTreeMap<u32, (u32, Option<ValidatorArgs<T>>)> =
        //     BTreeMap::from([(validator_subnet_node_id, (block, attest_data))]);
        let attests: BTreeMap<u32, AttestEntry<T>> = BTreeMap::from([(
            validator_subnet_node_id,
            AttestEntry::<T> {
                block: block,
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
        let validator_ids: Vec<u32> = if let Some(emergency_validator_data) =
            EmergencySubnetNodeElectionData::<T>::get(subnet_id)
        {
            emergency_validator_data
                .subnet_node_ids
                .into_iter()
                .collect()
        } else {
            SubnetNodeElectionSlots::<T>::get(subnet_id)
        };
        let validator_ids = Self::canonicalize_consensus_validator_ids(validator_ids);
        let attestor_weight_snapshot =
            Self::snapshot_consensus_attestor_weights(subnet_id, &validator_ids)?;

        // Check if validator sent through queue priority or removal node IDs
        if prioritize_queue_node_id.is_some() || remove_queue_node_id.is_some() {
            let queue = SubnetNodeQueue::<T>::get(subnet_id);
            let immunity_epochs = QueueImmunityEpochs::<T>::get(subnet_id); // Move outside loop

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
                        remove_allowed =
                            node.classification.start_epoch + immunity_epochs <= subnet_epoch;
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
            validator_reward_factor: Self::get_validator_reward_multiplier(
                subnet_epoch_progression,
            ),
            attests: attests,
            validator_ids,
            subnet_nodes: subnet_nodes,
            prioritize_queue_node_id: prioritize_queue_node_id,
            remove_queue_node_id: remove_queue_node_id,
            data: data,
            args: args,
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

        // --- Ensure subnet node is authorized under either its override hotkey or the
        //     validator hotkey when no override exists.
        ensure!(
            Self::get_subnet_node_associated_hotkey(subnet_id, subnet_node_id,)? == hotkey,
            Error::<T>::InvalidValidator
        );

        // --- Ensure node classified to attest
        // This is redundant because we check this later.
        match SubnetNodesData::<T>::try_get(subnet_id, subnet_node_id) {
            Ok(subnet_node) => {
                ensure!(
                    subnet_node.has_classification(&SubnetNodeClass::Validator, subnet_epoch),
                    Error::<T>::InvalidSubnetNodeClassification
                );
            }
            Err(()) => return Err(Error::<T>::InvalidSubnetNodeId.into()),
        };

        // - Note: we don't check stake balance here

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
                let reward_factor = Self::get_attestor_reward_multiplier(subnet_epoch_progression);

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
        Self::get_f64_as_percentage(Self::sigmoid_decreasing(
            Self::get_percent_as_f64(progress),
            Self::get_percent_as_f64(ValidatorRewardMidpoint::<T>::get()),
            ValidatorRewardK::<T>::get() as f64,
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
        Self::get_f64_as_percentage(Self::concave_down_decreasing(
            Self::get_percent_as_f64(progress),
            Self::get_percent_as_f64(AttestorMinRewardFactor::<T>::get()),
            1.0,
            AttestorRewardExponent::<T>::get() as f64,
        ))
        .clamp(0, Self::percentage_factor_as_u128())
    }

    /// Return the validators reward that submitted data on the previous epoch
    // The attestation percentage must be greater than the MinAttestationPercentage
    pub fn get_validator_reward(attestation_percentage: u128, reward_factor: u128) -> u128 {
        if MinAttestationPercentage::<T>::get() > attestation_percentage {
            return 0;
        }
        Self::percent_mul(BaseValidatorReward::<T>::get(), reward_factor)
    }

    /// Slash subnet validator node
    ///
    /// # Arguments
    ///
    /// * `subnet_id` - Subnet ID
    /// * `subnet_node_id` - Subnet node ID
    /// * `attestation_percentage` - The attestation ratio of the validator nodes consensus
    /// * `min_attestation_percentage` - Blockchains minimum attestation percentage (66%)
    /// * `coldkey_reputation_decrease_factor`: `ValidatorReputationDecreaseFactor`
    /// * `epoch`: The blockchains general epoch
    /// * `validator_non_consensus_reputation_factor`: Resolved subnet node factor for this epoch
    pub fn slash_validator(
        subnet_id: u32,
        subnet_node_id: u32,
        attestation_percentage: u128,
        min_attestation_percentage: u128,
        coldkey_reputation_decrease_factor: u128,
        min_validator_reputation: u128,
        electable_nodes: u32,
        epoch: u32,
        validator_non_consensus_reputation_factor: u128,
    ) -> Weight {
        let mut weight = Weight::zero();
        let db_weight = T::DbWeight::get();

        // Redundant
        if attestation_percentage >= min_attestation_percentage {
            return weight;
        }

        // --- Get percent difference between attestation ratio and min attestation ratio
        // 1.0 - attestation ratio / min attestation ratio
        let attestation_delta = Self::percentage_factor_as_u128().saturating_sub(
            Self::percent_div(attestation_percentage, min_attestation_percentage),
        );

        let account_subnet_stake: u128 = NodeSubnetStake::<T>::get(subnet_node_id, subnet_id);
        let slash_amount = Self::get_slash_amount(
            account_subnet_stake,
            attestation_percentage,
            min_attestation_percentage,
            attestation_delta,
        );

        if slash_amount > 0 {
            // --- Decrease account stake
            Self::decrease_node_stake(subnet_node_id, subnet_id, slash_amount);

            // NodeSubnetStake | TotalSubnetStake | TotalStake
            weight = weight.saturating_add(db_weight.writes(3));
            weight = weight.saturating_add(db_weight.reads(3));
        }

        let reputation = SubnetNodeReputation::<T>::get(subnet_id, subnet_node_id).map(|rep| {
            Self::decrease_and_return_node_reputation(
                subnet_id,
                subnet_node_id,
                rep,
                validator_non_consensus_reputation_factor,
                Some(attestation_delta),
            )
        });
        weight = weight.saturating_add(db_weight.reads_writes(2, 1));

        weight = weight.saturating_add(db_weight.reads(1));
        let validator_id =
            SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id).map(|validator_id| {
                // Decrease validator reputation
                Self::decrease_validator_reputation(
                    validator_id,
                    attestation_percentage,
                    min_attestation_percentage,
                    coldkey_reputation_decrease_factor,
                    epoch,
                );

                validator_id
            });

        // Remove validator if below min node reputation
        if let (Some(reputation), Some(validator_id)) = (reputation, validator_id) {
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

        // Self::deposit_event(Event::Slashing {
        //     subnet_id: subnet_id,
        //     account_id: hotkey,
        //     amount: slash_amount,
        // });

        weight
    }

    pub fn get_slash_amount(
        account_subnet_stake: u128,
        attestation_percentage: u128,
        min_attestation_percentage: u128,
        attestation_delta: u128,
    ) -> u128 {
        // --- Get slash amount up to max slash
        // --- Base slash amount
        // stake balance * BaseSlashPercentage
        let base_slash: u128 =
            Self::percent_mul(account_subnet_stake, BaseSlashPercentage::<T>::get());

        // --- Update slash amount based on delta
        // base_slash * attestation_delta
        let mut slash_amount = Self::percent_mul(base_slash, attestation_delta);

        // --- Update slash amount up to max slash
        let max_slash: u128 = MaxSlashAmount::<T>::get();

        if slash_amount > max_slash {
            slash_amount = max_slash
        }

        slash_amount
    }
}
