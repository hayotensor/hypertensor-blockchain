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
use frame_support::{
    pallet_prelude::DispatchError,
    storage::{with_transaction, TransactionOutcome},
    weights::Weight,
    BoundedBTreeSet, BoundedVec,
};
use sp_runtime::ArithmeticError;

impl<T: Config> Pallet<T> {
    pub(crate) fn validator_owned_nodes_weight_param(validator_id: u32) -> u32 {
        TotalValidatorNodes::<T>::get(validator_id).clamp(1, T::MaxValidatorNodesUpperBound::get())
    }

    /// Add an admitted node to the in-memory quarantine set used by one settlement.
    ///
    /// The set has the same bound as the admitted-node population. Every physical node removal must
    /// clear its marker, so inserting a live node cannot exceed this bound.
    fn stage_pending_admitted_node_removal(
        pending: &mut BoundedBTreeSet<u32, T::MaxSubnetNodesUpperBound>,
        newly_pending: &mut BoundedVec<u32, T::MaxSubnetNodesUpperBound>,
        subnet_node_id: u32,
    ) -> bool {
        if pending.contains(&subnet_node_id) {
            return false;
        }

        pending
            .try_insert(subnet_node_id)
            .expect("pending admitted-node removals are bounded by the admitted-node population");
        newly_pending
            .try_push(subnet_node_id)
            .expect("new admitted-node removals are bounded by the admitted-node population");
        true
    }

    fn stage_pending_queued_node_removal(
        pending: &mut BoundedBTreeSet<u32, T::MaxRegisteredNodesUpperBound>,
        newly_pending: &mut BoundedVec<u32, T::MaxRegisteredNodesUpperBound>,
        subnet_node_id: u32,
    ) -> bool {
        if pending.contains(&subnet_node_id) {
            return false;
        }

        pending
            .try_insert(subnet_node_id)
            .expect("pending queued-node removals are bounded by the queued population");
        newly_pending
            .try_push(subnet_node_id)
            .expect("new queued-node removals are bounded by the queued population");
        true
    }

    /// Persist a single admitted-node quarantine marker and return its diagnostic storage weight.
    /// This is used outside reward settlement where no settlement-local set is available.
    pub(crate) fn persist_pending_admitted_node_removal(
        subnet_id: u32,
        subnet_node_id: u32,
    ) -> (Weight, bool) {
        let db_weight = T::DbWeight::get();
        let mut weight =
            T::WeightInfo::pending_admitted_removal_scan(T::MaxSubnetNodesUpperBound::get());
        let mut pending = PendingAdmittedNodeRemovals::<T>::get(subnet_id);
        let mut newly_pending = BoundedVec::default();

        let inserted = Self::stage_pending_admitted_node_removal(
            &mut pending,
            &mut newly_pending,
            subnet_node_id,
        );
        if inserted {
            PendingAdmittedNodeRemovals::<T>::insert(subnet_id, pending);
            weight = weight.saturating_add(db_weight.writes(1));
        }

        (weight, inserted)
    }

    fn deposit_pending_node_removals(
        subnet_id: u32,
        admitted_subnet_node_ids: BoundedVec<u32, T::MaxSubnetNodesUpperBound>,
        queued_subnet_node_ids: BoundedVec<u32, T::MaxRegisteredNodesUpperBound>,
    ) {
        if admitted_subnet_node_ids.is_empty() && queued_subnet_node_ids.is_empty() {
            return;
        }

        Self::deposit_event(Event::SubnetNodesPendingRemoval {
            subnet_id,
            admitted_subnet_node_ids,
            queued_subnet_node_ids,
        });
    }

    /// Settle rewards and reputation for one completed subnet consensus round.
    ///
    /// Processing is logically ordered as follows:
    /// 1. Load the round rules and existing pending removals.
    ///    Use the policy frozen for this round and load the admitted nodes already awaiting removal.
    /// 2. Evaluate consensus and node eligibility.
    ///    Apply rejection penalties or accepted-round reputation and queue changes, marking any
    ///    newly ineligible admitted or queued nodes as pending removal.
    /// 3. Distribute eligible node rewards for an accepted round.
    ///    Use the round's original score total and withhold all node-related rewards from pending
    ///    nodes and a pending proposer without redistributing their shares.
    /// 4. Distribute rewards that are not tied to individual nodes.
    ///    Pay the subnet owner and subnet-wide delegate pool where the accepted branch allows it.
    /// 5. Finalize the pending-removal state.
    ///    Store all newly pending node IDs and emit one batched event before returning.
    ///
    /// Physical node deletion is intentionally deferred to separately metered cleanup paths.
    pub(crate) fn distribute_rewards(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        current_subnet_epoch: u32,
        consensus_submission_data: ConsensusSubmissionData<T>,
        rewards_data: RewardsData,
    ) {
        Self::distribute_rewards_for_round(
            weight_meter,
            subnet_id,
            current_subnet_epoch,
            current_subnet_epoch.saturating_sub(1),
            consensus_submission_data,
            rewards_data,
        );
    }

    /// Round-explicit settlement path used when a previously skipped epoch is retried.
    pub(crate) fn distribute_rewards_for_round(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        current_subnet_epoch: u32,
        settled_subnet_epoch: u32,
        consensus_submission_data: ConsensusSubmissionData<T>,
        mut rewards_data: RewardsData,
    ) {
        let db_weight = T::DbWeight::get();
        weight_meter.consume(db_weight.reads(1));
        if Self::is_consensus_round_settled(subnet_id, settled_subnet_epoch) {
            // Settlement (including any slash and liability release) is exact-once. This also
            // prevents a retained submission from minting the same reward budget twice.
            return;
        }

        // Quarantine is cheap and bounded independently from physical node deletion. Load it once,
        // deduplicate every newly ineligible node locally, and persist it before any settlement
        // exit. Physical cleanup is deliberately outside reward distribution.
        let mut pending_admitted_removals = PendingAdmittedNodeRemovals::<T>::get(subnet_id);
        let mut pending_admitted_removals_dirty = false;
        let mut newly_pending_admitted_removals = BoundedVec::default();
        let mut newly_pending_queued_removals = BoundedVec::default();
        weight_meter.consume(db_weight.reads(1));

        let percentage_factor = Self::percentage_factor_as_u128();
        let policy = consensus_submission_data.policy;
        let has_identity_super_majority = consensus_submission_data.identity_attestation_ratio
            >= policy.super_majority_attestation_ratio;
        let emergency_snapshot = consensus_submission_data.emergency.clone();
        let min_subnet_node_reputation = emergency_snapshot
            .as_ref()
            .map(|snapshot| snapshot.min_subnet_node_reputation)
            .unwrap_or_else(|| policy.min_subnet_node_reputation);
        let subnet_reputation = SubnetReputation::<T>::get(subnet_id);
        weight_meter.consume(db_weight.reads(1));

        let forked_subnet_node_ids: Option<BTreeSet<u32>> =
            Self::maybe_get_forked_subnet_node_ids(weight_meter, subnet_id, &emergency_snapshot);

        let min_identity_attestation_percentage = policy.validator_identity_attestation_percentage;
        let effective_identity_attestation_threshold =
            Self::effective_min_consensus_identity_attestation_percentage(
                consensus_submission_data.eligible_validator_identity_count,
                min_identity_attestation_percentage,
            );
        let min_identity_attestation_count = Self::min_consensus_identity_attestation_count(
            consensus_submission_data.eligible_validator_identity_count,
            min_identity_attestation_percentage,
        );

        let stake_quorum_failed =
            consensus_submission_data.attestation_ratio < policy.min_attestation_percentage;
        let identity_quorum_failed = !Self::has_minimum_consensus_validator_identity_set(
            consensus_submission_data.eligible_validator_identity_count,
        ) || consensus_submission_data.identity_attestation_count
            < min_identity_attestation_count;

        // --- If under either minimum attestation ratio, penalize validator, skip rewards
        if stake_quorum_failed || identity_quorum_failed {
            // Check before settlement effects so an overflow leaves the round retryable.
            let next_capacitor = if rewards_data.overall_subnet_reward > 0 {
                weight_meter.consume(db_weight.reads(1));
                let Some(balance) = SubnetRewardCapacitor::<T>::get(subnet_id)
                    .checked_add(rewards_data.overall_subnet_reward)
                else {
                    return;
                };
                Some(balance)
            } else {
                None
            };
            let stake_shortfall = if stake_quorum_failed {
                percentage_factor.saturating_sub(
                    Self::percent_div(
                        consensus_submission_data.attestation_ratio,
                        policy.min_attestation_percentage,
                    )
                    .min(percentage_factor),
                )
            } else {
                0
            };
            let identity_shortfall = if identity_quorum_failed {
                percentage_factor.saturating_sub(
                    Self::percent_div(
                        consensus_submission_data.identity_attestation_ratio,
                        effective_identity_attestation_threshold,
                    )
                    .min(percentage_factor),
                )
            } else {
                0
            };

            let (penalty_attestation_ratio, penalty_attestation_threshold) =
                if identity_shortfall > stake_shortfall {
                    (
                        consensus_submission_data.identity_attestation_ratio,
                        effective_identity_attestation_threshold,
                    )
                } else {
                    (
                        consensus_submission_data.attestation_ratio,
                        policy.min_attestation_percentage,
                    )
                };

            let slash_succeeded = Self::handle_non_consensus(
                subnet_id,
                consensus_submission_data,
                penalty_attestation_ratio,
                penalty_attestation_threshold,
                min_subnet_node_reputation,
                emergency_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.reputation_factors)
                    .unwrap_or_else(|| policy.reputation_factors),
                policy.not_in_consensus_subnet_reputation_factor,
                policy.base_slash_percentage,
                policy.max_slash_amount,
                percentage_factor,
                &mut pending_admitted_removals,
                &mut pending_admitted_removals_dirty,
                &mut newly_pending_admitted_removals,
                weight_meter,
            );
            if !slash_succeeded {
                // The combined node/pool slash rolled back. Keep the exact round pending and do
                // not retain reputation, reward, quarantine, or liability-release side effects.
                return;
            }
            if let Some(balance) = next_capacitor {
                SubnetRewardCapacitor::<T>::insert(subnet_id, balance);
                weight_meter.consume(db_weight.writes(1));
            }
            weight_meter.consume(Self::finalize_consensus_round_slash_liability(
                subnet_id,
                settled_subnet_epoch,
            ));

            if pending_admitted_removals_dirty {
                PendingAdmittedNodeRemovals::<T>::insert(subnet_id, pending_admitted_removals);
                weight_meter.consume(db_weight.writes(1));
            }
            Self::deposit_pending_node_removals(
                subnet_id,
                newly_pending_admitted_removals,
                newly_pending_queued_removals,
            );
            return;
        }

        // Release against the normal allocation, never against the stored balance. Zero-score
        // rounds cannot distribute node rewards, so retain their capacitor for a later round.
        let mut remaining_capacitor = None;
        if consensus_submission_data.weight_sum > 0 && rewards_data.overall_subnet_reward >= 10 {
            weight_meter.consume(db_weight.reads(1));
            let capacitor = SubnetRewardCapacitor::<T>::get(subnet_id);
            let supplement = capacitor.min(rewards_data.overall_subnet_reward / 10);
            if supplement > 0 {
                let Some(combined_reward) =
                    rewards_data.overall_subnet_reward.checked_add(supplement)
                else {
                    return;
                };
                let Some((combined_rewards, _)) = Self::calculate_rewards_with_policy(
                    combined_reward,
                    percentage_factor,
                    &policy,
                ) else {
                    return;
                };
                rewards_data = combined_rewards;
                remaining_capacitor = Some(capacitor - supplement);
            }
        }

        // Validate the reward checkpoint before finalizing or paying any accepted-round rewards.
        // Missing/corrupt accounting leaves the round pending for an explicit repair.
        if rewards_data.delegate_stake_rewards > 0 {
            let circulating = TotalSubnetDelegateStakeCirculatingShares::<T>::get(subnet_id);
            weight_meter.consume(db_weight.reads(1));
            if Self::delegate_pool_has_circulating_shares(circulating) {
                let balance = TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);
                let block = Self::get_current_block_as_u32();
                weight_meter.consume(db_weight.reads(4));
                if Self::prepare_subnet_balance_time(subnet_id, balance, block).is_err() {
                    return;
                }
            }
        }

        // Both quorum gates passed, so this round has no economic slash to apply. Release its
        // snapshotted liabilities before any later reward-specific early return.
        weight_meter.consume(Self::finalize_consensus_round_slash_liability(
            subnet_id,
            settled_subnet_epoch,
        ));
        if let Some(balance) = remaining_capacitor {
            SubnetRewardCapacitor::<T>::insert(subnet_id, balance);
            weight_meter.consume(db_weight.writes(1));
        }

        let consensus_validator_id = SubnetNodeValidatorId::<T>::get(
            subnet_id,
            consensus_submission_data.validator_subnet_node_id,
        );
        if consensus_validator_id.is_none() {
            // Validator left subnet before distribution of rewards (not possible but
            // this logic stays here in case of future updates to allowing validators to exit
            // on the epoch they're elected for)
            weight_meter.consume(db_weight.reads(1));
        }

        let validator_subnet_node_id = consensus_submission_data.validator_subnet_node_id;
        let validator_node_reputation =
            if !pending_admitted_removals.contains(&validator_subnet_node_id) {
                let validator_node_reputation =
                    SubnetNodeReputation::<T>::get(subnet_id, validator_subnet_node_id);
                weight_meter.consume(db_weight.reads(1));
                if validator_node_reputation
                    .is_some_and(|reputation| reputation < min_subnet_node_reputation)
                {
                    pending_admitted_removals_dirty |= Self::stage_pending_admitted_node_removal(
                        &mut pending_admitted_removals,
                        &mut newly_pending_admitted_removals,
                        validator_subnet_node_id,
                    );
                }
                validator_node_reputation
            } else {
                None
            };

        //
        // --- We are now in consensus (both the stake and distinct-identity quorums passed)
        //

        let probation_rounds = policy.probation_rounds;
        let weight_threshold = emergency_snapshot
            .as_ref()
            .map(|snapshot| snapshot.min_weight_decrease_reputation_threshold)
            .unwrap_or_else(|| policy.min_weight_decrease_reputation_threshold);
        let reputation_factors = emergency_snapshot
            .as_ref()
            .map(|snapshot| snapshot.reputation_factors)
            .unwrap_or_else(|| policy.reputation_factors);
        let absent_factor = reputation_factors.absent_decrease;
        let included_factor = reputation_factors.included_increase;
        let min_weight_factor = reputation_factors.below_min_weight_decrease;
        let non_attestor_factor = reputation_factors.non_attestor_decrease;

        // Super majority, update queue to prioritize node ID that subnet form a consensus to cut the line
        // and or update queue to remove a node ID the subnet forms a consensus to be removed (if passed immunity period)
        newly_pending_queued_removals = Self::handle_node_queue_consensus(
            weight_meter,
            subnet_id,
            &consensus_submission_data,
            policy.super_majority_attestation_ratio,
        );

        // Increase reputation because subnet consensus is in consensus
        // Only a distinct-identity supermajority can endorse this proposal strongly enough to
        // increase subnet reputation, and only when the subnet has >= min subnet nodes.
        if has_identity_super_majority
            && subnet_reputation != percentage_factor
            && consensus_submission_data.data_length >= policy.min_subnet_nodes
        {
            Self::increase_subnet_reputation(
                subnet_id,
                policy.in_consensus_subnet_reputation_factor,
                consensus_submission_data.identity_attestation_ratio,
            );
            weight_meter.consume(db_weight.reads_writes(2, 1));
        }

        // An accepted zero-score round has no rewardable subnet contribution. A healthy proposer
        // still receives its base reward; a quarantined proposer receives nothing.
        if consensus_submission_data.weight_sum == 0 {
            if pending_admitted_removals_dirty {
                PendingAdmittedNodeRemovals::<T>::insert(subnet_id, &pending_admitted_removals);
                weight_meter.consume(db_weight.writes(1));
            }
            Self::deposit_pending_node_removals(
                subnet_id,
                newly_pending_admitted_removals,
                newly_pending_queued_removals,
            );

            if consensus_validator_id.is_some() {
                if pending_admitted_removals.contains(&validator_subnet_node_id) {
                    // Account for the `SubnetNodeValidatorId` selector that would otherwise be
                    // charged by `handle_validator_reward`.
                    weight_meter.consume(db_weight.reads(1));
                } else {
                    if Self::handle_validator_reward(
                        weight_meter,
                        subnet_id,
                        validator_subnet_node_id,
                        &consensus_submission_data,
                        policy.min_attestation_percentage,
                        policy.base_validator_reward,
                    )
                    .is_err()
                    {
                        return;
                    }
                }
            }

            // Zero node score does not invalidate rewards unrelated to a node or proposer.
            if Self::handle_subnet_owner_reward(
                weight_meter,
                subnet_id,
                rewards_data.subnet_owner_reward,
            )
            .is_err()
            {
                return;
            }
            let credited_delegate_stake_reward = match Self::handle_subnet_delegate_stake_reward(
                weight_meter,
                subnet_id,
                rewards_data.delegate_stake_rewards,
            ) {
                Some(reward) => reward,
                None => return,
            };
            Self::deposit_event(Event::SubnetRewards {
                subnet_id,
                node_rewards: Vec::new(),
                delegate_stake_reward: credited_delegate_stake_reward,
                node_delegate_stake_rewards: Vec::new(),
                node_delegate_account_allocations: Vec::new(),
            });
            return;
        }

        // --- Reward owner
        if Self::handle_subnet_owner_reward(
            weight_meter,
            subnet_id,
            rewards_data.subnet_owner_reward,
        )
        .is_err()
        {
            return;
        }

        // CPU cost for this bounded loop is covered by the generated `emission_step(h)` model;
        // only branch-specific storage work is tracked by this internal admission meter.

        // --- Events variables

        // Node -> reward
        let mut node_rewards: Vec<(u32, u128)> = Vec::new();
        // Node -> delegate stake reward
        let mut validator_delegate_stake_rewards: Vec<(u32, u128)> = Vec::new();
        // Node -> (account -> amount)
        let mut node_delegate_account_allocations: Vec<(u32, (T::AccountId, u128))> = Vec::new();

        // Canonical score entries are unique by subnet-node ID. Index them once so reward
        // settlement remains O(n log n) instead of rescanning the full score vector for every
        // historical node (O(n^2)).
        let consensus_data_by_node: BTreeMap<u32, &SubnetNodeConsensusData> =
            consensus_submission_data
                .data
                .iter()
                .map(|data| (data.subnet_node_id, data))
                .collect();

        // Iterate each node, emit rewards, promote, or penalize
        for subnet_node in &consensus_submission_data.subnet_nodes {
            // Quarantine is effective immediately, even when physical cleanup did not fit in an
            // earlier subnet slot. Do not update, promote, or reward this node or its delegates.
            if pending_admitted_removals.contains(&subnet_node.id) {
                continue;
            }

            // We need to check if the node exists, since we need to get `SubnetNodeReputation`, we will use
            // that to check the node is still admitted and has not been removed.
            // Note: `SubnetNodeReputation` is removed when a node is removed
            //
            // We check this to enable the node receives rewards, if eligible, but skip all removal and
            // reputation logic.
            let starting_reputation = if subnet_node.id == validator_subnet_node_id {
                validator_node_reputation
            } else {
                weight_meter.consume(db_weight.reads(1));
                SubnetNodeReputation::<T>::get(subnet_id, subnet_node.id)
            };
            let node_exists = starting_reputation.is_some();
            let mut reputation = starting_reputation.unwrap_or_default();

            if node_exists && reputation < min_subnet_node_reputation {
                pending_admitted_removals_dirty |= Self::stage_pending_admitted_node_removal(
                    &mut pending_admitted_removals,
                    &mut newly_pending_admitted_removals,
                    subnet_node.id,
                );

                continue;
            }

            // Queued nodes are never score-eligible, even in a malformed historical snapshot.
            if subnet_node.classification.node_class == SubnetNodeClass::Queued {
                continue;
            }

            let subnet_node_data_find = consensus_data_by_node.get(&subnet_node.id).copied();

            // Handle case where node is found in consensus data
            let subnet_node_data = if let Some(data) = subnet_node_data_find {
                // --- Is in consensus data, increase reputation if not at max
                if node_exists && has_identity_super_majority && reputation != percentage_factor {
                    // Accepted inclusion can recover both Probation and Active reputation.
                    reputation = Self::increase_rep(reputation, included_factor, None);
                }
                data
            } else {
                if node_exists && has_identity_super_majority {
                    // A distinct-identity supermajority endorsed this node's omission.
                    reputation = Self::decrease_rep(reputation, absent_factor, None);

                    // Only ordinary, strongly endorsed evaluations change probation progress.
                    // Emergency rounds retain reputation handling but freeze the progress counter.
                    if subnet_node.classification.node_class == SubnetNodeClass::Probation
                        && forked_subnet_node_ids.is_none()
                    {
                        let progress =
                            SubnetNodeProbationProgress::<T>::get(subnet_id, subnet_node.id);
                        weight_meter.consume(db_weight.reads(1));
                        if progress != 0 {
                            SubnetNodeProbationProgress::<T>::remove(subnet_id, subnet_node.id);
                            weight_meter.consume(db_weight.writes(1));
                        }
                    }
                }

                if node_exists && reputation < min_subnet_node_reputation {
                    pending_admitted_removals_dirty |= Self::stage_pending_admitted_node_removal(
                        &mut pending_admitted_removals,
                        &mut newly_pending_admitted_removals,
                        subnet_node.id,
                    );
                }

                if let Some(previous) = starting_reputation {
                    weight_meter.consume(Self::persist_node_reputation_change(
                        subnet_id,
                        subnet_node.id,
                        previous,
                        reputation,
                    ));
                }

                // Not in consensus data, skip to next node
                continue;
            };

            // Historical Probation never earns node-related rewards, even after promotion,
            // removal, or during emergency consensus. Only surviving nodes can make progress.
            if subnet_node.classification.node_class == SubnetNodeClass::Probation {
                if let Some(previous) = starting_reputation {
                    weight_meter.consume(Self::persist_node_reputation_change(
                        subnet_id,
                        subnet_node.id,
                        previous,
                        reputation,
                    ));
                }

                if node_exists && has_identity_super_majority && forked_subnet_node_ids.is_none() {
                    Self::handle_probation_node(
                        weight_meter,
                        subnet_id,
                        subnet_node.id,
                        reputation,
                        percentage_factor,
                        probation_rounds,
                        current_subnet_epoch,
                        &pending_admitted_removals,
                    );
                }

                continue;
            }

            //
            // --- Consensus formed on node
            //

            let node_score = subnet_node_data.score;

            // We don't `continue` here because we want to calculate the weight percentage of the
            // node and possibly slash reputation if below the weight threshold

            // --- Calculate node weight percentage of peer versus the weighted sum
            let node_weight: u128 =
                Self::percent_div(node_score, consensus_submission_data.weight_sum);

            // * Optional logic:
            // Decrease reputation if under subnets weight threshold
            // A zero score is below any enabled positive threshold.
            // This is an optional feature for subnets and requires identity-supermajority
            // endorsement of the accepted score vector.
            if node_exists && has_identity_super_majority && node_weight < weight_threshold {
                reputation = Self::decrease_rep(reputation, min_weight_factor, None);
            }

            //
            // All remaining nodes were Active in the proposal snapshot and are in consensus data
            //

            // Eligible non-attestors retain score-based rewards at the round's timing floor.
            // This fallback is independent of the identity gate for reputation penalties.
            let reward_factor = if let Some(forked_node_ids) = &forked_subnet_node_ids {
                if forked_node_ids.get(&subnet_node.id).is_some() {
                    // If one of the temporary fork nodes
                    match consensus_submission_data.attests.get(&subnet_node.id) {
                        Some(data) => data.reward_factor,
                        None => {
                            // When a supermajority of distinct eligible validator identities
                            // participated, treat this emergency validator node as offline for
                            // failing to attest. The `non_attestor_factor` is intended to be the
                            // lowest decreasing factor of all node reputation factors.
                            if node_exists && has_identity_super_majority {
                                reputation =
                                    Self::decrease_rep(reputation, non_attestor_factor, None);
                            }
                            policy.attestor_min_reward_factor
                        }
                    }
                } else {
                    // Ordinary nodes outside the emergency set cannot attest in this round.
                    percentage_factor
                }
            } else if let Some(data) = consensus_submission_data.attests.get(&subnet_node.id) {
                // Subnet is not forked and node attested
                data.reward_factor
            } else {
                // A distinct-identity supermajority makes node-level non-participation
                // attributable for reputation purposes. The reward fallback applies even below
                // that gate and uses the elected policy rather than the current setting.
                if node_exists && has_identity_super_majority {
                    reputation = Self::decrease_rep(reputation, non_attestor_factor, None);
                }

                policy.attestor_min_reward_factor
            };

            if let Some(previous) = starting_reputation {
                weight_meter.consume(Self::persist_node_reputation_change(
                    subnet_id,
                    subnet_node.id,
                    previous,
                    reputation,
                ));
            }

            if node_exists && reputation < min_subnet_node_reputation {
                pending_admitted_removals_dirty |= Self::stage_pending_admitted_node_removal(
                    &mut pending_admitted_removals,
                    &mut newly_pending_admitted_removals,
                    subnet_node.id,
                );

                continue;
            }

            // Reward factor is zero, no need to continue
            if reward_factor == 0 {
                continue;
            }

            // Skip and do *not* penalize if node weight is 0
            if node_weight == 0 {
                continue;
            }

            // --- Calculate node_score percentage of total subnet generated epoch rewards
            let mut account_reward: u128 =
                Self::percent_mul(node_weight, rewards_data.subnet_node_rewards);

            account_reward = Self::percent_mul(account_reward, reward_factor);

            // --- Skip if no rewards to give
            if account_reward == 0 {
                continue;
            }

            // Apply every balance belonging to this node inside one storage transaction. If any
            // checked aggregate cannot accept its allocation, none of the node, validator-pool,
            // or delegate-account rewards are retained and no event reports a partial credit.
            let reward_result: Result<
                (u128, Option<(u32, u128)>, Option<(T::AccountId, u128)>),
                DispatchError,
            > = with_transaction(|| {
                let mut credited_node_reward = account_reward;
                let mut validator_delegate_allocation = None;
                let mut delegate_account_allocation = None;

                if let Ok(validator_data) = ValidatorsData::<T>::try_get(subnet_node.validator_id) {
                    if validator_data.delegate_reward_rate != 0 {
                        match Self::handle_validator_delegate_stake(
                            weight_meter,
                            subnet_node.validator_id,
                            validator_data.delegate_reward_rate,
                            credited_node_reward,
                        ) {
                            Ok(Some((updated_account_reward, node_delegate_reward))) => {
                                credited_node_reward = updated_account_reward;
                                validator_delegate_allocation =
                                    Some((subnet_node.validator_id, node_delegate_reward));
                            }
                            Ok(None) => {}
                            Err(error) => return TransactionOutcome::Rollback(Err(error)),
                        }
                    }

                    if let Some(delegate_account) = validator_data.delegate_account {
                        match Self::handle_delegate_account(
                            credited_node_reward,
                            &delegate_account.account_id,
                            delegate_account.rate,
                        ) {
                            Ok((updated_account_reward, delegate_account_deposit)) => {
                                credited_node_reward = updated_account_reward;
                                delegate_account_allocation =
                                    Some((delegate_account.account_id, delegate_account_deposit));
                            }
                            Err(error) => return TransactionOutcome::Rollback(Err(error)),
                        }
                    }
                }

                match Self::increase_node_stake(subnet_node.id, subnet_id, credited_node_reward) {
                    Ok(()) => TransactionOutcome::Commit(Ok((
                        credited_node_reward,
                        validator_delegate_allocation,
                        delegate_account_allocation,
                    ))),
                    Err(error) => TransactionOutcome::Rollback(Err(error)),
                }
            });

            let Ok((account_reward, validator_allocation, delegate_account_allocation)) =
                reward_result
            else {
                continue;
            };
            // NodeSubnetStake | TotalSubnetStake | TotalStake
            weight_meter.consume(db_weight.reads_writes(3, 3));

            if let Some(allocation) = validator_allocation {
                validator_delegate_stake_rewards.push(allocation);
            }
            if let Some(allocation) = delegate_account_allocation {
                node_delegate_account_allocations.push((subnet_node.id, allocation));
            }
            node_rewards.push((subnet_node.id, account_reward));
        }

        // Persist every newly quarantined node before any rewards outside the node loop are paid.
        if pending_admitted_removals_dirty {
            PendingAdmittedNodeRemovals::<T>::insert(subnet_id, &pending_admitted_removals);
            weight_meter.consume(db_weight.writes(1));
        }
        Self::deposit_pending_node_removals(
            subnet_id,
            newly_pending_admitted_removals,
            newly_pending_queued_removals,
        );

        // The validator base reward is intentionally deferred until all proposer reputation
        // changes have been evaluated, so crossing the threshold in this settlement withholds it.
        if consensus_validator_id.is_some() {
            if pending_admitted_removals.contains(&validator_subnet_node_id) {
                // Account for the `SubnetNodeValidatorId` selector that would otherwise be charged
                // by `handle_validator_reward`.
                weight_meter.consume(db_weight.reads(1));
            } else {
                if Self::handle_validator_reward(
                    weight_meter,
                    subnet_id,
                    validator_subnet_node_id,
                    &consensus_submission_data,
                    policy.min_attestation_percentage,
                    policy.base_validator_reward,
                )
                .is_err()
                {
                    return;
                }
            }
        }

        // --- Increase the delegate stake pool balance
        let credited_delegate_stake_reward = match Self::handle_subnet_delegate_stake_reward(
            weight_meter,
            subnet_id,
            rewards_data.delegate_stake_rewards,
        ) {
            Some(reward) => reward,
            None => return,
        };

        Self::deposit_event(Event::SubnetRewards {
            subnet_id,
            node_rewards,
            delegate_stake_reward: credited_delegate_stake_reward,
            node_delegate_stake_rewards: validator_delegate_stake_rewards,
            node_delegate_account_allocations,
        });
    }

    /// Subnet is not in consensus
    pub(crate) fn handle_non_consensus(
        subnet_id: u32,
        consensus_submission_data: ConsensusSubmissionData<T>,
        penalty_attestation_ratio: u128,
        penalty_attestation_threshold: u128,
        min_subnet_node_reputation: u128,
        reputation_factors: SubnetReputationFactors,
        not_in_consensus_subnet_reputation_factor: u128,
        base_slash_percentage: u128,
        max_slash_amount: u128,
        percentage_factor: u128,
        pending_admitted_removals: &mut BoundedBTreeSet<u32, T::MaxSubnetNodesUpperBound>,
        pending_admitted_removals_dirty: &mut bool,
        newly_pending_admitted_removals: &mut BoundedVec<u32, T::MaxSubnetNodesUpperBound>,
        weight_meter: &mut WeightMeter,
    ) -> bool {
        let db_weight = T::DbWeight::get();
        let validator_subnet_node_id = consensus_submission_data.validator_subnet_node_id;
        let stake_attestation_ratio = consensus_submission_data.attestation_ratio;
        let identity_attestation_ratio = consensus_submission_data.identity_attestation_ratio;
        let strong_rejection_threshold = consensus_submission_data
            .policy
            .validator_delegate_stake_slash_threshold;
        let strong_rejection_identity_shortfall = if strong_rejection_threshold > 0
            && identity_attestation_ratio < strong_rejection_threshold
        {
            Some(
                percentage_factor.saturating_sub(
                    Self::percent_div(identity_attestation_ratio, strong_rejection_threshold)
                        .min(percentage_factor),
                ),
            )
        } else {
            None
        };
        // Economic settlement must commit before any reputation or quarantine change.
        if penalty_attestation_ratio < penalty_attestation_threshold {
            let (slash_result, slash_weight) = Self::apply_validator_economic_slashes(
                subnet_id,
                validator_subnet_node_id,
                penalty_attestation_ratio,
                penalty_attestation_threshold,
                base_slash_percentage,
                max_slash_amount,
                consensus_submission_data.validator_node_stake_balance,
                stake_attestation_ratio,
                consensus_submission_data.validator_delegate_stake_balance,
                consensus_submission_data
                    .policy
                    .validator_delegate_stake_slash_threshold,
                consensus_submission_data
                    .policy
                    .base_validator_delegate_stake_slash_percentage,
                consensus_submission_data
                    .policy
                    .max_validator_delegate_stake_slash_amount,
            );
            weight_meter.consume(slash_weight);
            if slash_result.is_err() {
                return false;
            }
        }

        // The proposer may owe both role penalties. Keep their sequential rounding, but load
        // and persist this node only once, including when its automatic attestation is absent.
        weight_meter.consume(db_weight.reads(1));
        let proposer_start = SubnetNodeReputation::<T>::get(subnet_id, validator_subnet_node_id);
        let attestor_shortfall = strong_rejection_identity_shortfall.filter(|shortfall| {
            Self::percent_mul(
                reputation_factors.non_consensus_attestor_decrease,
                *shortfall,
            ) > 0
        });
        let proposer_reputation = proposer_start.map(|previous| {
            let mut reputation = previous;
            if let Some(shortfall) = strong_rejection_identity_shortfall.filter(|shortfall| {
                Self::percent_mul(
                    reputation_factors.validator_non_consensus_decrease,
                    *shortfall,
                ) > 0
            }) {
                reputation = Self::decrease_rep(
                    reputation,
                    reputation_factors.validator_non_consensus_decrease,
                    Some(shortfall),
                );
            }
            if consensus_submission_data
                .attests
                .contains_key(&validator_subnet_node_id)
            {
                if let Some(shortfall) = attestor_shortfall {
                    reputation = Self::decrease_rep(
                        reputation,
                        reputation_factors.non_consensus_attestor_decrease,
                        Some(shortfall),
                    );
                }
            }
            weight_meter.consume(Self::persist_node_reputation_change(
                subnet_id,
                validator_subnet_node_id,
                previous,
                reputation,
            ));
            reputation
        });

        // Submitted proposals can decrease subnet reputation only when a distinct-identity
        // strong rejection exists. Stake-only rejection retains its economic consequences but
        // cannot damage subnet reputation.
        if strong_rejection_identity_shortfall.is_some_and(|identity_shortfall| {
            Self::percent_mul(
                not_in_consensus_subnet_reputation_factor,
                identity_shortfall,
            ) > 0
        }) {
            Self::decrease_subnet_reputation(
                subnet_id,
                not_in_consensus_subnet_reputation_factor,
                strong_rejection_identity_shortfall,
            );
            // NotInConsensusSubnetReputationFactor | SubnetReputation
            weight_meter.consume(db_weight.reads_writes(2, 1));
        }

        // Process the remaining supporters once. Stage the proposer at its original position
        // in the attestor ordering, using its final local value rather than reading it again.
        if let Some(identity_shortfall) = attestor_shortfall {
            for (subnet_node_id, _) in consensus_submission_data.attests {
                let reputation = if subnet_node_id == validator_subnet_node_id {
                    proposer_reputation
                } else {
                    weight_meter.consume(db_weight.reads(1));
                    SubnetNodeReputation::<T>::get(subnet_id, subnet_node_id).map(|previous| {
                        let current = Self::decrease_rep(
                            previous,
                            reputation_factors.non_consensus_attestor_decrease,
                            Some(identity_shortfall),
                        );
                        weight_meter.consume(Self::persist_node_reputation_change(
                            subnet_id,
                            subnet_node_id,
                            previous,
                            current,
                        ));
                        current
                    })
                };
                if reputation.is_some_and(|value| value < min_subnet_node_reputation) {
                    *pending_admitted_removals_dirty |= Self::stage_pending_admitted_node_removal(
                        pending_admitted_removals,
                        newly_pending_admitted_removals,
                        subnet_node_id,
                    );
                }
            }
        }

        // Also covers a missing proposer attestation or a reputation-neutral rejection.
        if proposer_reputation.is_some_and(|value| value < min_subnet_node_reputation) {
            *pending_admitted_removals_dirty |= Self::stage_pending_admitted_node_removal(
                pending_admitted_removals,
                newly_pending_admitted_removals,
                validator_subnet_node_id,
            );
        }
        true
    }

    pub(crate) fn handle_validator_reward(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        subnet_node_id: u32,
        consensus_submission_data: &ConsensusSubmissionData<T>,
        min_attestation_percentage: u128,
        base_validator_reward: u128,
    ) -> DispatchResult {
        let db_weight = T::DbWeight::get();

        weight_meter.consume(db_weight.reads(1));

        // --- Increase validator reward
        let validator_reward = Self::get_validator_reward_with_policy(
            consensus_submission_data.attestation_ratio,
            consensus_submission_data.validator_reward_factor,
            min_attestation_percentage,
            base_validator_reward,
        );
        weight_meter.consume(db_weight.reads(1));

        // Give validator rewards to their stake
        Self::increase_node_stake(subnet_node_id, subnet_id, validator_reward)
    }

    pub(crate) fn handle_subnet_owner_reward(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        amount: u128,
    ) -> DispatchResult {
        // SubnetOwner
        weight_meter.consume(T::DbWeight::get().reads(1));
        if let Ok(owner) = SubnetOwner::<T>::try_get(subnet_id) {
            let balance =
                Self::u128_to_balance(amount).ok_or(Error::<T>::CouldNotConvertToBalance)?;
            weight_meter.consume(T::WeightInfo::add_balance_to_coldkey_account());
            Self::add_balance_to_coldkey_account(&owner, balance)?;
        }
        Ok(())
    }

    /// Handles node queue operations based on stake-weighted consensus data.
    ///
    /// This function allows the validator to prioritize or remove nodes from the registration queue.
    ///
    /// # Parameters
    ///
    /// * `weight_meter` - Weight meter for tracking weight consumption
    /// * `subnet_id` - The ID of the subnet
    /// * `consensus_submission_data` - Consensus submission data containing queue operations
    /// * `super_majority_threshold` - The stake-weighted supermajority threshold for queue actions
    /// # Behavior
    ///
    /// The function performs the following steps:
    /// 1. Checks if the consensus submission has a stake-weighted supermajority
    /// 2. Retrieves the node queue for the subnet
    /// 3. Handles prioritize node operation if specified
    /// 4. Handles remove node operation if specified
    ///
    /// # Errors
    ///
    /// * `SubnetNodeNotFoundInQueue` - Node not found in queue
    /// * `SubnetNodeNotImmune` - Node not immune from removal
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success
    pub(crate) fn handle_node_queue_consensus(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        consensus_submission_data: &ConsensusSubmissionData<T>,
        super_majority_threshold: u128,
    ) -> BoundedVec<u32, T::MaxRegisteredNodesUpperBound> {
        let mut newly_pending_queued_removals = BoundedVec::default();
        if consensus_submission_data.attestation_ratio >= super_majority_threshold {
            let db_weight = T::DbWeight::get();

            let mut queue = SubnetNodeQueue::<T>::get(subnet_id);

            // Handle prioritize node - move to front
            if let Some(prioritize_queue_node_id) =
                consensus_submission_data.prioritize_queue_node_id
            {
                weight_meter.consume(db_weight.reads(1));

                if let Some(index) = queue
                    .iter()
                    .position(|node| node.id == prioritize_queue_node_id)
                {
                    let node = queue.remove(index); // Remove from current position
                    queue.insert(0, node); // Insert at front (index 0)

                    // The generated `emission_step_accepted_queue_mutations_front(q)` model
                    // covers the scan and front insertion; do not synthesize ref-time here.
                    SubnetNodeQueue::<T>::insert(subnet_id, &queue);
                    weight_meter.consume(db_weight.writes(1));

                    Self::deposit_event(Event::QueuedNodePrioritized {
                        subnet_id,
                        subnet_node_id: prioritize_queue_node_id,
                    });
                }
            }

            // Logically remove the node from the registration queue and quarantine its physical data.
            // Cleanup is attempted after election, outside reward settlement, and retried in future
            // assigned subnet slots when the remaining meter is insufficient.
            if let Some(remove_queue_node_id) = consensus_submission_data.remove_queue_node_id {
                if let Some(index) = queue
                    .iter()
                    .position(|node| node.id == remove_queue_node_id)
                {
                    let mut pending = PendingQueuedNodeRemovals::<T>::get(subnet_id);
                    weight_meter.consume(db_weight.reads(1));

                    if Self::stage_pending_queued_node_removal(
                        &mut pending,
                        &mut newly_pending_queued_removals,
                        remove_queue_node_id,
                    ) {
                        PendingQueuedNodeRemovals::<T>::insert(subnet_id, pending);
                        weight_meter.consume(db_weight.writes(1));
                    }

                    queue.remove(index);
                    SubnetNodeQueue::<T>::insert(subnet_id, &queue);
                    weight_meter.consume(db_weight.writes(1));

                    Self::deposit_event(Event::QueuedNodeRemoved {
                        subnet_id,
                        subnet_node_id: remove_queue_node_id,
                    });
                }
            }
        }

        newly_pending_queued_removals
    }

    pub(crate) fn handle_probation_node(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        subnet_node_id: u32,
        reputation: u128,
        percentage_factor: u128,
        probation_rounds: u32,
        current_subnet_epoch: u32,
        pending_removals: &BoundedBTreeSet<u32, T::MaxSubnetNodesUpperBound>,
    ) {
        if pending_removals.contains(&subnet_node_id) {
            return;
        }
        let db_weight = T::DbWeight::get();
        let previous = SubnetNodeProbationProgress::<T>::get(subnet_id, subnet_node_id);
        let progress = previous.saturating_add(1);
        weight_meter.consume(db_weight.reads(1));

        if reputation >= percentage_factor && progress >= probation_rounds {
            // Promotion commits its counter cleanup together with node and election state.
            weight_meter.consume(db_weight.reads_writes(5, 7));
            if Self::promote_probation_node_to_active(
                subnet_id,
                subnet_node_id,
                current_subnet_epoch,
                pending_removals,
            ) {
                return;
            }
        }
        if progress != previous {
            SubnetNodeProbationProgress::<T>::insert(subnet_id, subnet_node_id, progress);
            weight_meter.consume(db_weight.writes(1));
        }
    }

    pub(crate) fn maybe_get_forked_subnet_node_ids(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        emergency_snapshot: &Option<EmergencyConsensusSnapshot>,
    ) -> Option<BTreeSet<u32>> {
        let Some(snapshot) = emergency_snapshot else {
            return None;
        };

        let db_weight = T::DbWeight::get();
        let mut should_finish = false;

        weight_meter.consume(db_weight.reads(1));
        EmergencySubnetNodeElectionData::<T>::mutate_exists(subnet_id, |maybe_data| {
            if let Some(data) = maybe_data {
                if data.activated && data.subnet_node_ids == snapshot.subnet_node_ids {
                    weight_meter.consume(db_weight.writes(1));
                    data.total_epochs = data.total_epochs.saturating_add(1);
                    should_finish = data.total_epochs >= data.target_emergency_validators_epochs;
                }
            }
        });

        if should_finish {
            Self::finish_emergency_validator_set(subnet_id);
            weight_meter.consume(db_weight.writes(2));
        }

        Some(snapshot.subnet_node_ids.iter().cloned().collect())
    }

    pub(crate) fn handle_validator_delegate_stake(
        weight_meter: &mut WeightMeter,
        validator_id: u32,
        delegate_reward_rate: u128,
        account_reward: u128,
    ) -> Result<Option<(u128, u128)>, DispatchError> {
        let db_weight = T::DbWeight::get();
        // --- Ensure users are staked to subnet node
        let circulating_shares = ValidatorDelegateStakeCirculatingShares::<T>::get(validator_id);
        // ValidatorDelegateStakeCirculatingShares
        weight_meter.consume(db_weight.reads(1));

        // Locked minimum-liquidity shares have no owner. Rewards may only increase the exchange
        // rate when at least one circulating share can receive them.
        if Self::delegate_pool_has_circulating_shares(circulating_shares) {
            let node_delegate_reward = Self::percent_mul(account_reward, delegate_reward_rate);
            // A malformed stored rate must never mint a delegate allocation larger than the
            // reward it is carved from. Registration/setters enforce the percentage bound; this
            // checked subtraction is the accounting boundary's defense in depth.
            let updated_account_reward = account_reward
                .checked_sub(node_delegate_reward)
                .ok_or(ArithmeticError::Underflow)?;
            Self::do_increase_validator_delegate_stake(validator_id, node_delegate_reward)?;
            // reads:
            // ValidatorDelegateStakeBalance | ValidatorDelegateStakeShares |
            // TotalValidatorDelegateStakeBalance
            //
            // writes:
            // ValidatorDelegateStakeShares | ValidatorDelegateStakeBalance |
            // TotalValidatorDelegateStakeBalance
            weight_meter.consume(db_weight.reads_writes(5, 3));

            return Ok(Some((updated_account_reward, node_delegate_reward)));
        }
        Ok(None)
    }

    /// Credit a subnet-wide delegate reward only when the pool has user-circulating shares.
    ///
    /// A pool containing only the permanently locked minimum-liquidity shares has no economic
    /// owner. Crediting it would strand issuance and let a later depositor capture historical
    /// rewards. `None` is reserved for an accounting failure so callers can abort settlement;
    /// `Some(0)` means that no reward was issued to an inactive pool.
    fn handle_subnet_delegate_stake_reward(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        delegate_stake_reward: u128,
    ) -> Option<u128> {
        if delegate_stake_reward == 0 {
            return Some(0);
        }

        let db_weight = T::DbWeight::get();
        let circulating_shares = TotalSubnetDelegateStakeCirculatingShares::<T>::get(subnet_id);
        weight_meter.consume(db_weight.reads(1));

        if !Self::delegate_pool_has_circulating_shares(circulating_shares) {
            return Some(0);
        }

        // The reward checkpoint reads the block, subnet existence and balance-time record,
        // in addition to the four existing pool/total reads. Charge before a possible error.
        // Writes update the pool balance, total delegation, and balance-time record.
        weight_meter.consume(db_weight.reads_writes(7, 3));
        if Self::do_increase_delegate_stake(subnet_id, delegate_stake_reward).is_err() {
            return None;
        }
        Some(delegate_stake_reward)
    }

    pub(crate) fn handle_delegate_account(
        account_reward: u128,
        delegate_account_id: &T::AccountId,
        rate: u128,
    ) -> Result<(u128, u128), DispatchError> {
        let delegate_account_deposit = Self::percent_mul(account_reward, rate);
        let updated_account_reward = account_reward
            .checked_sub(delegate_account_deposit)
            .ok_or(ArithmeticError::Underflow)?;
        Self::increase_delegate_account_balance(delegate_account_id, delegate_account_deposit)?;

        Ok((updated_account_reward, delegate_account_deposit))
    }
}
