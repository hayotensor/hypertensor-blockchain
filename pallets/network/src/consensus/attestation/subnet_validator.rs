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
use frame_support::pallet_prelude::Pays;
use frame_support::pallet_prelude::Weight;
use frame_support::pallet_prelude::{DispatchError, DispatchResultWithPostInfo};
use sp_runtime::ArithmeticError;

const MIN_VALIDATOR_REWARD_MULTIPLIER: f64 = 0.0;
const MAX_REWARD_MULTIPLIER: f64 = 1.0;

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
        pending_ids: &BTreeSet<u32>,
    ) -> Result<(Vec<SubnetNodeConsensusData>, u128), Error<T>> {
        let filtered_data = data
            .into_iter()
            .filter(|entry| {
                !pending_ids.contains(&entry.subnet_node_id)
                    && SubnetNodesData::<T>::try_get(subnet_id, entry.subnet_node_id)
                        .map(|subnet_node| {
                            subnet_node
                                .has_classification(&SubnetNodeClass::Probation, subnet_epoch)
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

    /// Calculate final voting weights from the inputs selected by election.
    pub(crate) fn snapshot_consensus_attestor_weights(
        subnet_id: u32,
        policy: &ConsensusPolicySnapshot,
        validator_identity_ids: &BTreeMap<u32, u32>,
    ) -> ConsensusAttestorWeightSnapshot {
        let mut validator_nodes: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for (subnet_node_id, validator_id) in validator_identity_ids {
            validator_nodes
                .entry(*validator_id)
                .or_default()
                .push(*subnet_node_id);
        }

        let mut weights = BTreeMap::new();
        // A snapshot can contain several u128 balances. Accumulate them at full width rather
        // than rejecting an election when their sum cannot be stored directly as a u128.
        // Node IDs are u32, so even the entire key space sums to less than 2^160.
        let mut total_weight = U256::zero();
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
                    .unwrap_or(0)
                    .min(percentage_factor);
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

                total_weight += U256::from(node_weight);
                weights.insert(subnet_node_id, node_weight);
            }
        }

        let stake_weight_power = policy
            .consensus_validator_stake_weight_power
            .min(percentage_factor);

        // Preserve representable raw weights exactly. If the aggregate exceeds u128, rescale
        // every weight by the same fraction using a full-width product. Rounding down ensures
        // that the stored sum fits and remains exactly the sum of the stored node weights.
        if stake_weight_power == percentage_factor || total_weight.is_zero() {
            let maximum = U256::from(u128::MAX);
            let total_weight = if total_weight > maximum {
                let mut normalized_total = 0u128;
                for node_weight in weights.values_mut() {
                    *node_weight = (U256::from(*node_weight) * maximum / total_weight).as_u128();
                    normalized_total += *node_weight;
                }
                normalized_total
            } else {
                total_weight.as_u128()
            };
            return ConsensusAttestorWeightSnapshot {
                weights,
                total_weight,
            };
        }

        let exponent = Self::get_percent_as_f64(stake_weight_power);
        let raw_total_weight = if total_weight <= U256::from(u128::MAX) {
            total_weight.as_u128() as f64
        } else {
            // Diminishing power can use raw totals wider than u128. At most 2^32 finite
            // u128 inputs also have a finite, positive f64 sum.
            weights.values().map(|weight| *weight as f64).sum()
        };
        let mut powered_total_weight = 0u128;

        for node_weight in weights.values_mut() {
            if *node_weight == 0 {
                continue;
            }

            let normalized_weight = *node_weight as f64 / raw_total_weight;
            let powered_weight =
                Self::get_f64_as_percentage(Self::pow(normalized_weight, exponent))
                    .min(percentage_factor);

            // Each transformed weight is at most Q18 and there are at most 2^32 node IDs,
            // so their aggregate fits in u128 without a fallible arithmetic branch.
            powered_total_weight += powered_weight;
            *node_weight = powered_weight;
        }

        ConsensusAttestorWeightSnapshot {
            weights,
            total_weight: powered_total_weight,
        }
    }

    /// Validate the complete election snapshot without consulting mutable stake or membership.
    pub(crate) fn validate_consensus_attestor_weight_snapshot(
        round: &ElectedConsensusRound,
        snapshot: &ConsensusAttestorWeightSnapshot,
    ) -> Result<(), Error<T>> {
        let invalid = || Error::<T>::InvalidConsensusAttestorWeightSnapshot;
        ensure!(
            snapshot.weights.len() == round.eligible_subnet_node_ids.len()
                && snapshot.weights.len() == round.eligible_validator_identity_ids.len(),
            invalid()
        );
        let mut seen = BTreeSet::new();
        let mut total = 0u128;
        for node_id in &round.eligible_subnet_node_ids {
            ensure!(
                seen.insert(*node_id)
                    && round.eligible_validator_identity_ids.contains_key(node_id),
                invalid()
            );
            let weight = snapshot.weights.get(node_id).ok_or_else(invalid)?;
            total = total.checked_add(*weight).ok_or_else(invalid)?;
        }
        ensure!(total == snapshot.total_weight, invalid());
        Ok(())
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
    ///   and only Probation and Active peers are retained.
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
    ///    - Filtering out Queued peers
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
        ensure!(
            !Self::consensus_round_precedes_network_resume(subnet_id, subnet_epoch),
            Error::<T>::ConsensusRoundCancelled
        );
        let validator_subnet_node_id = elected_round.validator_subnet_node_id;
        let policy = elected_round.policy;

        // The caller does not supply authority-bearing node identity. The elected subnet node is
        // derived from storage, then the signed hotkey must own that exact node.
        Self::ensure_hotkey_owns_subnet_node(subnet_id, validator_subnet_node_id, &hotkey)?;

        // Snapshot both bounded quarantine sets once. Score, reward, quorum, and weight filtering
        // below must all observe the same pending-removal view.
        let pending_ids = Self::pending_subnet_node_removal_ids(subnet_id);

        // A quarantined elected validator may only use this authenticated call to finish its own
        // cleanup. It must not create a proposal or receive the fee waiver for normal consensus
        // work.
        if pending_ids.contains(&validator_subnet_node_id) {
            let _ = Self::finalize_pending_subnet_node_removal(subnet_id, validator_subnet_node_id);
            return Ok(Pays::Yes.into());
        }

        // `propose_attestation` has a fixed, worst-case benchmark. Bound the caller-controlled
        // vector before any per-entry storage lookup. Authentication and pending cleanup must run
        // first so malformed proposal contents cannot keep an authenticated quarantined proposer
        // alive.
        ensure!(
            data.len() as u32 <= T::MaxSubnetNodesUpperBound::get(),
            Error::<T>::MaxSubnetNodes
        );

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

        // Remove Queued entries, collapse duplicate node IDs to the lowest score,
        // and ensure the canonical score sum does not overflow.
        let (data, weight_sum) = Self::canonicalize_consensus_data_for_submission(
            subnet_id,
            subnet_epoch,
            data,
            &pending_ids,
        )?;

        let block: u32 = Self::get_current_block_as_u32();
        let attests: BTreeMap<u32, AttestEntry<T>> = BTreeMap::from([(
            validator_subnet_node_id,
            AttestEntry::<T> {
                block,
                attestor_progress: 0,
                reward_factor: Self::percentage_factor_as_u128(),
                data: None,
            },
        )]);

        // Snapshot admitted Probation and Active nodes for scoring and historical settlement.
        // Capturing membership here avoids a live node scan during on_initialize. The historical
        // classification determines settlement: Probation can progress but earns no node rewards;
        // Active can receive rewards for its contribution even if it leaves before settlement.
        let subnet_nodes: Vec<ConsensusSubnetNode> = Self::get_admitted_classified_subnet_nodes(
            subnet_id,
            &SubnetNodeClass::Probation,
            subnet_epoch,
        )
        .iter()
        .filter(|subnet_node| !pending_ids.contains(&subnet_node.id))
        .map(ConsensusSubnetNode::from)
        .collect();

        // --- Get all validators
        // Note: This is triggered here when the validator submits their data, not at the start block of the epoch
        //
        // These are the nodes that can attest to the consensus data
        //
        // The elected round is the immutable source of membership and validator identity. Proposal
        // may only subtract nodes that are currently quarantined; it must not re-resolve a live
        // candidate set and silently change the round being attested.
        let validator_ids: Vec<u32> = elected_round
            .eligible_subnet_node_ids
            .iter()
            .copied()
            .filter(|subnet_node_id| !pending_ids.contains(subnet_node_id))
            .collect();
        let validator_identity_ids = validator_ids
            .iter()
            .map(|subnet_node_id| {
                elected_round
                    .eligible_validator_identity_ids
                    .get(subnet_node_id)
                    .copied()
                    .map(|validator_id| (*subnet_node_id, validator_id))
                    .ok_or(Error::<T>::InvalidSubnetNodeId)
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let eligible_validator_identity_count = validator_identity_ids
            .values()
            .copied()
            .collect::<BTreeSet<_>>()
            .len() as u32;
        // Quarantined registrations are logically absent from the queue. Ignore stale requests
        // naming them even if an inconsistent queue copy still contains the physical record.
        if prioritize_queue_node_id
            .map(|subnet_node_id| pending_ids.contains(&subnet_node_id))
            .unwrap_or(false)
        {
            prioritize_queue_node_id = None;
        }
        if remove_queue_node_id
            .map(|subnet_node_id| pending_ids.contains(&subnet_node_id))
            .unwrap_or(false)
        {
            remove_queue_node_id = None;
        }

        // Voting power is fixed before the election block's extrinsics. Quarantine only
        // subtracts proposal members; it never recalculates the remaining nodes' weights.
        let attestor_weight_snapshot =
            SubnetConsensusAttestorWeights::<T>::get(subnet_id, subnet_epoch)
                .ok_or(Error::<T>::InvalidConsensusAttestorWeightSnapshot)?;
        Self::validate_consensus_attestor_weight_snapshot(
            &elected_round,
            &attestor_weight_snapshot,
        )?;

        // Check if validator sent through queue priority or removal node IDs
        if prioritize_queue_node_id.is_some() || remove_queue_node_id.is_some() {
            let queue = SubnetNodeQueue::<T>::get(subnet_id);
            let immunity_epochs = policy.queue_immunity_epochs;
            // The resume guard above guarantees no completed pause has occurred since this
            // election. Pin immunity to its slot so it cannot expire ahead of queue admission.
            let queue_epoch = Self::queue_epoch_at_subnet_slot(subnet_id, subnet_epoch);

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
                            queue_epoch,
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
        let max_submission_items = (validator_ids.len() as u32)
            .max(attests.len() as u32)
            .max(subnet_nodes.len() as u32)
            .max(data.len() as u32)
            .max(elected_round.eligible_subnet_node_ids.len() as u32)
            .max(elected_round.eligible_validator_identity_ids.len() as u32);
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
            args: None,
            weight_sum,
            eligible_validator_identity_count,
        };

        // --- Store the data
        SubnetConsensusSubmission::<T>::insert(subnet_id, subnet_epoch, consensus_data);
        SubnetConsensusSubmissionMaxItems::<T>::insert(
            subnet_id,
            subnet_epoch,
            max_submission_items,
        );
        if let Some(args) = args {
            SubnetConsensusProposalArgs::<T>::insert(subnet_id, subnet_epoch, args);
        }
        if let Some(attest_data) = attest_data {
            SubnetConsensusAttestationData::<T>::insert(
                (subnet_id, subnet_epoch, validator_subnet_node_id),
                attest_data,
            );
        }

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

        // Pending nodes can authenticate this call only to complete their own removal. They do not
        // attest, write opaque attestation data, emit an attestation event, or receive a fee waiver.
        if Self::finalize_pending_subnet_node_removal(subnet_id, subnet_node_id) {
            return Ok(Pays::Yes.into());
        }

        // --- Ensure node classified to attest
        ensure!(
            subnet_node.has_classification(&SubnetNodeClass::Active, subnet_epoch),
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
        ensure!(
            !Self::consensus_round_precedes_network_resume(subnet_id, subnet_epoch),
            Error::<T>::ConsensusRoundCancelled
        );

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
                // Only Active nodes can attest
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

                // Ensure they haven't attested already
                ensure!(
                    params.attests.insert(
                        subnet_node_id,
                        AttestEntry::<T> {
                            block,
                            attestor_progress: subnet_epoch_progression,
                            reward_factor,
                            data: None,
                        }
                    ) == None,
                    Error::<T>::AlreadyAttested
                );
                Ok(())
            },
        )?;

        if let Some(data) = data {
            SubnetConsensusAttestationData::<T>::insert(
                (subnet_id, subnet_epoch, subnet_node_id),
                data,
            );
        }

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
            MIN_VALIDATOR_REWARD_MULTIPLIER,
            MAX_REWARD_MULTIPLIER,
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
            MAX_REWARD_MULTIPLIER,
            policy.attestor_reward_exponent as f64,
        ))
        // Float conversion must not put an attestation below the exact integer fallback.
        .clamp(
            policy.attestor_min_reward_factor,
            Self::percentage_factor_as_u128(),
        )
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
    /// * `validator_non_consensus_reputation_factor`: Resolved subnet node factor for this epoch
    /// * `identity_reputation_shortfall`: Precomputed distinct-identity rejection severity
    pub fn slash_subnet_node(
        subnet_id: u32,
        subnet_node_id: u32,
        attestation_percentage: u128,
        min_attestation_percentage: u128,
        min_subnet_node_reputation: u128,
        validator_non_consensus_reputation_factor: u128,
        identity_reputation_shortfall: Option<u128>,
    ) -> Weight {
        Self::slash_subnet_node_with_policy(
            subnet_id,
            subnet_node_id,
            attestation_percentage,
            min_attestation_percentage,
            min_subnet_node_reputation,
            validator_non_consensus_reputation_factor,
            identity_reputation_shortfall,
            BaseSlashPercentage::<T>::get(),
            MaxSlashAmount::<T>::get(),
        )
    }

    pub fn slash_subnet_node_with_policy(
        subnet_id: u32,
        subnet_node_id: u32,
        attestation_percentage: u128,
        min_attestation_percentage: u128,
        min_subnet_node_reputation: u128,
        validator_non_consensus_reputation_factor: u128,
        identity_reputation_shortfall: Option<u128>,
        base_slash_percentage: u128,
        max_slash_amount: u128,
    ) -> Weight {
        // Direct calls use current node principal; round settlement supplies its election snapshot.
        let snapshotted_validator_node_stake_balance =
            NodeSubnetStake::<T>::get(subnet_node_id, subnet_id);
        Self::slash_subnet_node_for_round_with_policy(
            subnet_id,
            subnet_node_id,
            attestation_percentage,
            min_attestation_percentage,
            min_subnet_node_reputation,
            validator_non_consensus_reputation_factor,
            identity_reputation_shortfall,
            base_slash_percentage,
            max_slash_amount,
            snapshotted_validator_node_stake_balance,
        )
    }

    /// Apply snapshotted economic policy and reputation penalties to a submitted round.
    /// Direct node-stake severity uses the selected consensus-failure ratio against election-time
    /// principal. Each node's direct stake is independent of its validator identity's other nodes.
    /// Proposer node reputation uses only the caller-provided distinct-identity support and
    /// strong-rejection shortfall.
    pub fn slash_subnet_node_for_round_with_policy(
        subnet_id: u32,
        subnet_node_id: u32,
        attestation_percentage: u128,
        min_attestation_percentage: u128,
        min_subnet_node_reputation: u128,
        validator_non_consensus_reputation_factor: u128,
        identity_reputation_shortfall: Option<u128>,
        base_slash_percentage: u128,
        max_slash_amount: u128,
        snapshotted_validator_node_stake_balance: u128,
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

        if economic_consensus_failed {
            let (_, slash_weight) = Self::apply_subnet_node_slash(
                subnet_id,
                subnet_node_id,
                attestation_percentage,
                min_attestation_percentage,
                base_slash_percentage,
                max_slash_amount,
                snapshotted_validator_node_stake_balance,
            );
            weight = weight.saturating_add(slash_weight);
        }
        let validator_exists = if proposer_node_reputation_shortfall.is_some() {
            weight = weight.saturating_add(db_weight.reads(1));
            SubnetNodeValidatorId::<T>::contains_key(subnet_id, subnet_node_id)
        } else {
            false
        };

        let reputation = if let Some(identity_shortfall) = proposer_node_reputation_shortfall {
            weight = weight.saturating_add(db_weight.reads(1));
            SubnetNodeReputation::<T>::get(subnet_id, subnet_node_id).map(|rep| {
                let new_reputation = Self::decrease_rep(
                    rep,
                    validator_non_consensus_reputation_factor,
                    Some(identity_shortfall),
                );
                weight = weight.saturating_add(Self::persist_node_reputation_change(
                    subnet_id,
                    subnet_node_id,
                    rep,
                    new_reputation,
                ));
                new_reputation
            })
        } else {
            None
        };

        // Quarantine an Active-class subnet node below the minimum reputation. Physical
        // deletion is reserved for an assigned subnet slot with spare hook weight or an
        // authenticated node-specific call.
        // Callers that need the proposer to receive a subsequent attestor-role decrease pass a
        // zero minimum and perform the final check later.
        if let (true, Some(reputation)) = (validator_exists, reputation) {
            if reputation < min_subnet_node_reputation {
                let (pending_weight, _) =
                    Self::persist_pending_admitted_node_removal(subnet_id, subnet_node_id);
                weight = weight.saturating_add(pending_weight);
            }
        }

        weight
    }

    /// Slash only this subnet node's direct stake using its election-time principal.
    /// A failed deduction rolls back and queues only the financial penalty. Reserved direct
    /// stake remains available to that retry while consensus and reputation processing continue.
    pub(crate) fn apply_subnet_node_slash(
        subnet_id: u32,
        subnet_node_id: u32,
        attestation_percentage: u128,
        slash_threshold: u128,
        base_slash_percentage: u128,
        max_slash_amount: u128,
        snapshotted_node_stake_balance: u128,
    ) -> (u128, Weight) {
        let db_weight = T::DbWeight::get();
        let mut weight =
            T::WeightInfo::pending_node_slash_lock_read().saturating_add(db_weight.reads(1));
        let current_stake = NodeSubnetStake::<T>::get(subnet_node_id, subnet_id);
        let available = current_stake.saturating_sub(Self::pending_node_slash_locked_amount(
            subnet_id,
            subnet_node_id,
        ));
        let amount = if attestation_percentage < slash_threshold {
            let shortfall = Self::percentage_factor_as_u128().saturating_sub(
                Self::percent_div(attestation_percentage, slash_threshold)
                    .min(Self::percentage_factor_as_u128()),
            );
            Self::get_slash_amount_with_policy(
                snapshotted_node_stake_balance,
                shortfall,
                base_slash_percentage,
                max_slash_amount,
            )
            .min(available)
        } else {
            0
        };
        weight = weight.saturating_add(db_weight.reads_writes(7, 5));
        let operation = ConsensusOperation::NodeSlash {
            subnet_id,
            subnet_node_id,
            amount,
            attestation_percentage,
        };
        if let Err(error) = Self::try_apply_consensus_operation(&operation) {
            Self::deposit_event(Event::SubnetNodeSlashFailed {
                subnet_id,
                subnet_node_id,
                error,
            });
            let mut retry_weight = WeightMeter::new();
            Self::defer_consensus_operation(&mut retry_weight, subnet_id, operation);
            weight = weight.saturating_add(retry_weight.consumed());
            (0, weight)
        } else {
            (amount, weight)
        }
    }

    pub fn get_slash_amount(account_subnet_stake: u128, attestation_delta: u128) -> u128 {
        Self::get_slash_amount_with_policy(
            account_subnet_stake,
            attestation_delta,
            BaseSlashPercentage::<T>::get(),
            MaxSlashAmount::<T>::get(),
        )
    }

    pub fn get_slash_amount_with_policy(
        account_subnet_stake: u128,
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
