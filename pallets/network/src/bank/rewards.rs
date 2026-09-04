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
use frame_support::{pallet_prelude::DispatchError, weights::Weight, BoundedBTreeSet, BoundedVec};

impl<T: Config> Pallet<T> {
    pub(crate) fn validator_owned_nodes_weight_param(validator_id: u32) -> u32 {
        TotalValidatorNodes::<T>::get(validator_id).clamp(1, T::MaxValidatorNodesUpperBound::get())
    }

    /// Add an active node to the in-memory quarantine set used by one settlement.
    ///
    /// The set has the same bound as the active-node population. Every physical node removal must
    /// clear its marker, so inserting a live node cannot exceed this bound.
    fn stage_pending_active_node_removal(
        pending: &mut BoundedBTreeSet<u32, T::MaxSubnetNodesUpperBound>,
        newly_pending: &mut BoundedVec<u32, T::MaxSubnetNodesUpperBound>,
        subnet_node_id: u32,
    ) -> bool {
        if pending.contains(&subnet_node_id) {
            return false;
        }

        pending
            .try_insert(subnet_node_id)
            .expect("pending active-node removals are bounded by the active-node population");
        newly_pending
            .try_push(subnet_node_id)
            .expect("new active-node removals are bounded by the active-node population");
        true
    }

    fn stage_pending_registered_node_removal(
        pending: &mut BoundedBTreeSet<u32, T::MaxRegisteredNodesUpperBound>,
        newly_pending: &mut BoundedVec<u32, T::MaxRegisteredNodesUpperBound>,
        subnet_node_id: u32,
    ) -> bool {
        if pending.contains(&subnet_node_id) {
            return false;
        }

        pending
            .try_insert(subnet_node_id)
            .expect("pending registered-node removals are bounded by the registered population");
        newly_pending
            .try_push(subnet_node_id)
            .expect("new registered-node removals are bounded by the registered population");
        true
    }

    /// Persist a single active-node quarantine marker and return its diagnostic storage weight.
    /// This is used outside reward settlement where no settlement-local set is available.
    pub(crate) fn persist_pending_active_node_removal(
        subnet_id: u32,
        subnet_node_id: u32,
    ) -> (Weight, bool) {
        let db_weight = T::DbWeight::get();
        let mut weight =
            T::WeightInfo::pending_active_removal_scan(T::MaxSubnetNodesUpperBound::get());
        let mut pending = PendingActiveNodeRemovals::<T>::get(subnet_id);
        let mut newly_pending = BoundedVec::default();

        let inserted = Self::stage_pending_active_node_removal(
            &mut pending,
            &mut newly_pending,
            subnet_node_id,
        );
        if inserted {
            PendingActiveNodeRemovals::<T>::insert(subnet_id, pending);
            weight = weight.saturating_add(db_weight.writes(1));
        }

        (weight, inserted)
    }

    fn deposit_pending_node_removals(
        subnet_id: u32,
        active_subnet_node_ids: BoundedVec<u32, T::MaxSubnetNodesUpperBound>,
        registered_subnet_node_ids: BoundedVec<u32, T::MaxRegisteredNodesUpperBound>,
    ) {
        if active_subnet_node_ids.is_empty() && registered_subnet_node_ids.is_empty() {
            return;
        }

        Self::deposit_event(Event::SubnetNodesPendingRemoval {
            subnet_id,
            active_subnet_node_ids,
            registered_subnet_node_ids,
        });
    }

    /// Settle rewards and reputation for one completed subnet consensus round.
    ///
    /// Processing is logically ordered as follows:
    /// 1. Load the round rules and existing pending removals.
    ///    Use the policy frozen for this round and load the active nodes already awaiting removal.
    /// 2. Evaluate consensus and node eligibility.
    ///    Apply rejection penalties or accepted-round reputation and queue changes, marking any
    ///    newly ineligible active or registered nodes as pending removal.
    /// 3. Distribute eligible node rewards for an accepted round.
    ///    Use the round's original score total and withhold all node-related rewards from pending
    ///    nodes and a pending proposer without redistributing their shares.
    /// 4. Distribute rewards that are not tied to individual nodes.
    ///    Pay the subnet owner and subnet-wide delegate pool where the accepted branch allows it.
    /// 5. Finalize the pending-removal state.
    ///    Store all newly pending node IDs and emit one batched event before returning.
    ///
    /// Physical node deletion is intentionally deferred to separately metered cleanup paths.
    pub fn distribute_rewards(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        current_subnet_epoch: u32,
        consensus_submission_data: ConsensusSubmissionData<T>,
        rewards_data: RewardsData,
    ) {
        let db_weight = T::DbWeight::get();
        // Quarantine is cheap and bounded independently from physical node deletion. Load it once,
        // deduplicate every newly ineligible node locally, and persist it before any settlement
        // exit. Physical cleanup is deliberately outside reward distribution.
        let mut pending_active_removals = PendingActiveNodeRemovals::<T>::get(subnet_id);
        let mut pending_active_removals_dirty = false;
        let mut newly_pending_active_removals = BoundedVec::default();
        let mut newly_pending_registered_removals = BoundedVec::default();
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

            Self::handle_non_consensus(
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
                &mut pending_active_removals,
                &mut pending_active_removals_dirty,
                &mut newly_pending_active_removals,
                weight_meter,
            );

            if pending_active_removals_dirty {
                PendingActiveNodeRemovals::<T>::insert(subnet_id, pending_active_removals);
                weight_meter.consume(db_weight.writes(1));
            }
            Self::deposit_pending_node_removals(
                subnet_id,
                newly_pending_active_removals,
                newly_pending_registered_removals,
            );
            return;
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
        if !pending_active_removals.contains(&validator_subnet_node_id) {
            let validator_node_reputation =
                SubnetNodeReputation::<T>::get(subnet_id, validator_subnet_node_id);
            weight_meter.consume(db_weight.reads(1));
            if validator_node_reputation
                .is_some_and(|reputation| reputation < min_subnet_node_reputation)
            {
                pending_active_removals_dirty |= Self::stage_pending_active_node_removal(
                    &mut pending_active_removals,
                    &mut newly_pending_active_removals,
                    validator_subnet_node_id,
                );
            }
        }

        //
        // --- We are now in consensus (both the stake and distinct-identity quorums passed)
        //

        let idle_epochs = policy.idle_classification_epochs;
        let included_epochs = policy.included_classification_epochs;
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
        newly_pending_registered_removals = Self::handle_node_queue_consensus(
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
            if pending_active_removals_dirty {
                PendingActiveNodeRemovals::<T>::insert(subnet_id, &pending_active_removals);
                weight_meter.consume(db_weight.writes(1));
            }
            Self::deposit_pending_node_removals(
                subnet_id,
                newly_pending_active_removals,
                newly_pending_registered_removals,
            );

            if consensus_validator_id.is_some() {
                if pending_active_removals.contains(&validator_subnet_node_id) {
                    // Account for the `SubnetNodeValidatorId` selector that would otherwise be
                    // charged by `handle_validator_reward`.
                    weight_meter.consume(db_weight.reads(1));
                } else {
                    Self::handle_validator_reward(
                        weight_meter,
                        subnet_id,
                        validator_subnet_node_id,
                        &consensus_submission_data,
                        policy.min_attestation_percentage,
                        policy.base_validator_reward,
                    );
                }
            }

            // Zero node score does not invalidate rewards unrelated to a node or proposer.
            Self::handle_subnet_owner_reward(
                weight_meter,
                subnet_id,
                rewards_data.subnet_owner_reward,
            );
            if rewards_data.delegate_stake_rewards != 0 {
                Self::do_increase_delegate_stake(subnet_id, rewards_data.delegate_stake_rewards);
                weight_meter.consume(db_weight.reads_writes(3, 5));
            }
            Self::deposit_event(Event::SubnetRewards {
                subnet_id,
                node_rewards: Vec::new(),
                delegate_stake_reward: rewards_data.delegate_stake_rewards,
                node_delegate_stake_rewards: Vec::new(),
                node_delegate_account_allocations: Vec::new(),
            });
            return;
        }

        // --- Reward owner
        Self::handle_subnet_owner_reward(weight_meter, subnet_id, rewards_data.subnet_owner_reward);

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

        // Iterate each node, emit rewards, graduate, or penalize
        for subnet_node in &consensus_submission_data.subnet_nodes {
            // Quarantine is effective immediately, even when physical cleanup did not fit in an
            // earlier subnet slot. Do not update, graduate, or reward this node or its delegates.
            if pending_active_removals.contains(&subnet_node.id) {
                continue;
            }

            // We need to check if the node exists, since we need to get `SubnetNodeReputation`, we will use
            // that to check the node is still active and has not been removed.
            // Note: `SubnetNodeReputation` is removed when a node is removed
            //
            // We check this to enable the node receives rewards, if eligible, but skip all removal and
            // reputation logic.
            let (mut reputation, node_exists): (u128, bool) =
                match SubnetNodeReputation::<T>::try_get(subnet_id, subnet_node.id) {
                    Ok(r) => (r, true),
                    Err(_) => (0, false),
                };

            // SubnetNodeReputation
            weight_meter.consume(db_weight.reads(1));

            if node_exists && reputation < min_subnet_node_reputation {
                pending_active_removals_dirty |= Self::stage_pending_active_node_removal(
                    &mut pending_active_removals,
                    &mut newly_pending_active_removals,
                    subnet_node.id,
                );

                continue;
            }

            // If node is Idle class and subnet is not temporarily forked via temp validator set,
            // upgrade to Included class
            if node_exists
                && subnet_node.classification.node_class == SubnetNodeClass::Idle
                && forked_subnet_node_ids.is_none()
            {
                Self::handle_idle_node(
                    weight_meter,
                    subnet_id,
                    subnet_node.id,
                    idle_epochs,
                    current_subnet_epoch,
                );

                continue;
            }

            //
            // All nodes are at least SubnetNodeClass::Included from here
            //

            let subnet_node_data_find = consensus_data_by_node.get(&subnet_node.id).copied();

            // Handle case where node is found in consensus data
            let subnet_node_data = if let Some(data) = subnet_node_data_find {
                // --- Is in consensus data, increase reputation if not at max
                if node_exists && has_identity_super_majority && reputation != percentage_factor {
                    // If the validator-class node appears in accepted data, increase that node's
                    // reputation.
                    reputation = Self::increase_and_return_node_reputation(
                        subnet_id,
                        subnet_node.id,
                        reputation,
                        included_factor,
                        None,
                    );

                    // `increase_and_return_node_reputation`: SubnetNodeReputation (w)
                    weight_meter.consume(db_weight.writes(1));
                }
                data
            } else {
                if node_exists && has_identity_super_majority {
                    // A distinct-identity supermajority endorsed this node's omission.
                    reputation = Self::decrease_and_return_node_reputation(
                        subnet_id,
                        subnet_node.id,
                        reputation,
                        absent_factor,
                        None,
                    );
                    // `decrease_and_return_node_reputation`: SubnetNodeReputation (w)
                    weight_meter.consume(db_weight.writes(1));

                    // Break count of consecutive epochs of being included in in-consensus data
                    if subnet_node.classification.node_class == SubnetNodeClass::Included {
                        SubnetNodeConsecutiveIncludedEpochs::<T>::insert(
                            subnet_id,
                            subnet_node.id,
                            0,
                        );
                        // SubnetNodeConsecutiveIncludedEpochs
                        weight_meter.consume(db_weight.writes(1));
                    }
                }

                if node_exists && reputation < min_subnet_node_reputation {
                    pending_active_removals_dirty |= Self::stage_pending_active_node_removal(
                        &mut pending_active_removals,
                        &mut newly_pending_active_removals,
                        subnet_node.id,
                    );
                }

                // Not in consensus data, skip to next node
                continue;
            };

            // If node is Included class and subnet is not temporarily forked, upgrade to Validator class
            //
            // This is ran after we check if the node is included in the consensus data to ensure the node
            // gets its reputation decreased if it was not included in the consensus data
            if node_exists
                && subnet_node.classification.node_class == SubnetNodeClass::Included
                && forked_subnet_node_ids.is_none()
            {
                if has_identity_super_majority {
                    Self::handle_included_node(
                        weight_meter,
                        subnet_id,
                        subnet_node.id,
                        reputation,
                        percentage_factor,
                        included_epochs,
                        current_subnet_epoch,
                    );
                }

                // SubnetNodeClass::Included does not get rewards yet, they must pass the gauntlet
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
                reputation = Self::decrease_and_return_node_reputation(
                    subnet_id,
                    subnet_node.id,
                    reputation,
                    min_weight_factor,
                    None,
                );
                // `decrease_and_return_node_reputation`: SubnetNodeReputation (w)
                weight_meter.consume(db_weight.writes(1));
            }

            //
            // All nodes are at least SubnetNodeClass::Validator from here and in consensus data
            //

            // Get the nodes reward factor
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
                                reputation = Self::decrease_and_return_node_reputation(
                                    subnet_id,
                                    subnet_node.id,
                                    reputation,
                                    non_attestor_factor,
                                    None,
                                );
                                // `decrease_and_return_node_reputation`: SubnetNodeReputation (w)
                                weight_meter.consume(db_weight.writes(1));
                            }
                            percentage_factor
                        }
                    }
                } else {
                    percentage_factor
                }
            } else if let Some(data) = consensus_submission_data.attests.get(&subnet_node.id) {
                // Subnet is not forked and node attested
                data.reward_factor
            } else {
                // A distinct-identity supermajority makes node-level non-participation
                // attributable. Decrease this non-attesting node's reputation while preserving its
                // existing reward factor.
                if node_exists && has_identity_super_majority {
                    reputation = Self::decrease_and_return_node_reputation(
                        subnet_id,
                        subnet_node.id,
                        reputation,
                        non_attestor_factor,
                        None,
                    );
                    // `decrease_and_return_node_reputation`: SubnetNodeReputation (w)

                    weight_meter.consume(db_weight.writes(1));
                }

                percentage_factor
            };

            if node_exists && reputation < min_subnet_node_reputation {
                pending_active_removals_dirty |= Self::stage_pending_active_node_removal(
                    &mut pending_active_removals,
                    &mut newly_pending_active_removals,
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

            // We allow the node to not exist here and still increase the delegate reward pool
            // --- Increase delegate account balance and emit event
            if let Ok(validator_data) = &ValidatorsData::<T>::try_get(subnet_node.validator_id) {
                if validator_data.delegate_reward_rate != 0 {
                    if let Some((updated_account_reward, node_delegate_reward)) =
                        Self::handle_validator_delegate_stake(
                            weight_meter,
                            subnet_node.validator_id,
                            validator_data.delegate_reward_rate,
                            account_reward,
                        )
                    {
                        // Update account reward with the substracted amount that was given to the delegates
                        account_reward = updated_account_reward;
                        // Add the node delegate reward to the list for event
                        validator_delegate_stake_rewards
                            .push((subnet_node.validator_id, node_delegate_reward));
                    }
                }

                if let Some(delegate_account) = &validator_data.delegate_account {
                    // We don't check if the rate is > 0 because the rate can't
                    // be set to 0.
                    let (updated_account_reward, delegate_account_deposit) =
                        Self::handle_delegate_account(
                            account_reward,
                            &delegate_account.account_id,
                            delegate_account.rate,
                        );
                    account_reward = updated_account_reward;

                    node_delegate_account_allocations.push((
                        subnet_node.id,
                        (
                            delegate_account.account_id.clone(),
                            delegate_account_deposit,
                        ),
                    ));
                }
            }

            Self::increase_node_stake(subnet_node.id, subnet_id, account_reward);
            // NodeSubnetStake | TotalSubnetStake | TotalStake
            weight_meter.consume(db_weight.reads_writes(3, 3));

            node_rewards.push((subnet_node.id, account_reward));
        }

        // Persist every newly quarantined node before any rewards outside the node loop are paid.
        if pending_active_removals_dirty {
            PendingActiveNodeRemovals::<T>::insert(subnet_id, &pending_active_removals);
            weight_meter.consume(db_weight.writes(1));
        }
        Self::deposit_pending_node_removals(
            subnet_id,
            newly_pending_active_removals,
            newly_pending_registered_removals,
        );

        // The validator base reward is intentionally deferred until all proposer reputation
        // changes have been evaluated, so crossing the threshold in this settlement withholds it.
        if consensus_validator_id.is_some() {
            if pending_active_removals.contains(&validator_subnet_node_id) {
                // Account for the `SubnetNodeValidatorId` selector that would otherwise be charged
                // by `handle_validator_reward`.
                weight_meter.consume(db_weight.reads(1));
            } else {
                Self::handle_validator_reward(
                    weight_meter,
                    subnet_id,
                    validator_subnet_node_id,
                    &consensus_submission_data,
                    policy.min_attestation_percentage,
                    policy.base_validator_reward,
                );
            }
        }

        // --- Increase the delegate stake pool balance
        if rewards_data.delegate_stake_rewards != 0 {
            Self::do_increase_delegate_stake(subnet_id, rewards_data.delegate_stake_rewards);
            // reads::
            // TotalSubnetDelegateStakeShares | TotalSubnetDelegateStakeBalance | TotalDelegateStake
            //
            // writes::
            // TotalSubnetDelegateStakeBalance | | TotalSubnetDelegateStakeShares|
            // TotalSubnetDelegateStakeShares| TotalSubnetDelegateStakeBalance| TotalDelegateStake
            weight_meter.consume(db_weight.reads_writes(3, 5));
        }

        Self::deposit_event(Event::SubnetRewards {
            subnet_id,
            node_rewards,
            delegate_stake_reward: rewards_data.delegate_stake_rewards,
            node_delegate_stake_rewards: validator_delegate_stake_rewards,
            node_delegate_account_allocations,
        });
    }

    /// Subnet is not in consensus
    pub fn handle_non_consensus(
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
        pending_active_removals: &mut BoundedBTreeSet<u32, T::MaxSubnetNodesUpperBound>,
        pending_active_removals_dirty: &mut bool,
        newly_pending_active_removals: &mut BoundedVec<u32, T::MaxSubnetNodesUpperBound>,
        weight_meter: &mut WeightMeter,
    ) {
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
        // --- Slash validator
        // Slashes stake balance
        // Proposer-node reputation uses only the distinct-identity strong-rejection shortfall.
        // Node removal is deliberately deferred until after the attestor-role decrease below so
        // the proposer can receive both sequential reputation penalties before removal.
        let slash_validator_weight = Self::slash_validator_for_round_with_policy(
            subnet_id,
            validator_subnet_node_id,
            penalty_attestation_ratio,
            penalty_attestation_threshold,
            0,
            reputation_factors.validator_non_consensus_decrease,
            strong_rejection_identity_shortfall,
            base_slash_percentage,
            max_slash_amount,
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
        weight_meter.consume(slash_validator_weight);

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

        // Every node that attested to this rejected proposal, including the proposer through its
        // automatic attestation, is accountable only when support by distinct validator identities
        // is below the round's snapshotted strong-rejection threshold. Stake support continues to
        // govern the proposer's economic penalties above, but does not gate or scale this decrease.
        if let Some(identity_shortfall) = strong_rejection_identity_shortfall {
            if Self::percent_mul(
                reputation_factors.non_consensus_attestor_decrease,
                identity_shortfall,
            ) > 0
            {
                // --- Decrease reputation of attestors to a strongly rejected proposal
                for (subnet_node_id, _attest_data) in consensus_submission_data.attests {
                    weight_meter.consume(db_weight.reads(1));
                    if let Some(rep) = SubnetNodeReputation::<T>::get(subnet_id, subnet_node_id) {
                        // The reputation entry also establishes that the node is still active. A
                        // node may have removed itself between attestation and settlement.
                        let new_reputation = Self::decrease_and_return_node_reputation(
                            subnet_id,
                            subnet_node_id,
                            rep,
                            reputation_factors.non_consensus_attestor_decrease,
                            Some(identity_shortfall),
                        );

                        // try_mutate_exists, plus the NodeReputationUpdate event's System event
                        // storage accesses. The explicit `get` read is metered above even when the
                        // reputation entry no longer exists.
                        weight_meter.consume(db_weight.reads_writes(5, 3));

                        if new_reputation < min_subnet_node_reputation {
                            *pending_active_removals_dirty |=
                                Self::stage_pending_active_node_removal(
                                    pending_active_removals,
                                    newly_pending_active_removals,
                                    subnet_node_id,
                                );
                        }
                    }
                }
            }
        }

        // The proposer is normally in `attests` through automatic attestation and was therefore
        // removed above only after both decreases. This final check preserves proposer removal at
        // or above the strong-rejection boundary and safely covers a malformed/missing entry.
        weight_meter.consume(db_weight.reads(1));
        if SubnetNodeReputation::<T>::get(subnet_id, validator_subnet_node_id)
            .is_some_and(|reputation| reputation < min_subnet_node_reputation)
        {
            *pending_active_removals_dirty |= Self::stage_pending_active_node_removal(
                pending_active_removals,
                newly_pending_active_removals,
                validator_subnet_node_id,
            );
        }
    }

    pub fn handle_validator_reward(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        subnet_node_id: u32,
        consensus_submission_data: &ConsensusSubmissionData<T>,
        min_attestation_percentage: u128,
        base_validator_reward: u128,
    ) {
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
        Self::increase_node_stake(subnet_node_id, subnet_id, validator_reward);
    }

    pub fn handle_subnet_owner_reward(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        amount: u128,
    ) {
        // SubnetOwner
        weight_meter.consume(T::DbWeight::get().reads(1));
        if let Ok(owner) = SubnetOwner::<T>::try_get(subnet_id) {
            if let Some(balance) = Self::u128_to_balance(amount) {
                Self::add_balance_to_coldkey_account(&owner, balance);
                weight_meter.consume(T::WeightInfo::add_balance_to_coldkey_account());
            }
        }
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
    pub fn handle_node_queue_consensus(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        consensus_submission_data: &ConsensusSubmissionData<T>,
        super_majority_threshold: u128,
    ) -> BoundedVec<u32, T::MaxRegisteredNodesUpperBound> {
        let mut newly_pending_registered_removals = BoundedVec::default();
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

            // Logically remove the node from the activation queue and quarantine its physical data.
            // Cleanup is attempted after election, outside reward settlement, and retried in future
            // assigned subnet slots when the remaining meter is insufficient.
            if let Some(remove_queue_node_id) = consensus_submission_data.remove_queue_node_id {
                if let Some(index) = queue
                    .iter()
                    .position(|node| node.id == remove_queue_node_id)
                {
                    let mut pending = PendingRegisteredNodeRemovals::<T>::get(subnet_id);
                    weight_meter.consume(db_weight.reads(1));

                    if Self::stage_pending_registered_node_removal(
                        &mut pending,
                        &mut newly_pending_registered_removals,
                        remove_queue_node_id,
                    ) {
                        PendingRegisteredNodeRemovals::<T>::insert(subnet_id, pending);
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

        newly_pending_registered_removals
    }

    pub fn handle_idle_node(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        subnet_node_id: u32,
        idle_epochs: u32,
        current_subnet_epoch: u32,
    ) {
        let db_weight = T::DbWeight::get();
        let node_idle_epochs = SubnetNodeIdleConsecutiveEpochs::<T>::try_mutate(
            subnet_id,
            subnet_node_id,
            |n: &mut u32| -> Result<u32, DispatchError> {
                *n += 1;
                Ok(*n)
            },
        );
        weight_meter.consume(db_weight.reads_writes(1, 1));

        // Idle classified nodes can't be included in consensus data and can't have a used reputation
        // so we check the class immediately.
        // --- Upgrade to Included if past the queue epochs
        match node_idle_epochs {
            Ok(node_idle_epochs) => {
                if node_idle_epochs >= idle_epochs {
                    if Self::graduate_class(subnet_id, subnet_node_id, current_subnet_epoch) {
                        SubnetNodeIdleConsecutiveEpochs::<T>::remove(subnet_id, subnet_node_id);
                        weight_meter.consume(db_weight.writes(1));
                    }
                }
            }
            Err(_) => return,
        }
    }

    pub fn handle_included_node(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        subnet_node_id: u32,
        reputation: u128,
        percentage_factor: u128,
        included_epochs: u32,
        current_subnet_epoch: u32,
    ) {
        let db_weight = T::DbWeight::get();
        let node_included_epochs = SubnetNodeConsecutiveIncludedEpochs::<T>::try_mutate(
            subnet_id,
            subnet_node_id,
            |n: &mut u32| -> Result<u32, DispatchError> {
                *n += 1;
                Ok(*n)
            },
        );

        // SubnetNodeConsecutiveIncludedEpochs
        weight_meter.consume(db_weight.reads_writes(1, 1));

        // --- Upgrade to Validator if at percentage_factor reputation and included in weights
        match node_included_epochs {
            Ok(node_included_epochs) => {
                if reputation >= percentage_factor && node_included_epochs >= included_epochs {
                    if Self::graduate_to_validator_class(
                        subnet_id,
                        subnet_node_id,
                        current_subnet_epoch,
                    ) {
                        // --- Remove consecutive included epochs as this node will never need this
                        // counter again
                        SubnetNodeConsecutiveIncludedEpochs::<T>::remove(subnet_id, subnet_node_id);
                        weight_meter.consume(db_weight.writes(1));
                    }
                }
            }
            Err(_) => return,
        }
    }

    pub fn maybe_get_forked_subnet_node_ids(
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

    pub fn handle_validator_delegate_stake(
        weight_meter: &mut WeightMeter,
        validator_id: u32,
        delegate_reward_rate: u128,
        account_reward: u128,
    ) -> Option<(u128, u128)> {
        let db_weight = T::DbWeight::get();
        // --- Ensure users are staked to subnet node
        let total_node_delegated_stake_shares =
            ValidatorDelegateStakeShares::<T>::get(validator_id);
        // ValidatorDelegateStakeShares
        weight_meter.consume(db_weight.reads(1));

        // We make sure the pool has shares before depositing into it
        if total_node_delegated_stake_shares != 0 {
            let node_delegate_reward = Self::percent_mul(account_reward, delegate_reward_rate);
            let updated_account_reward = account_reward.saturating_sub(node_delegate_reward);
            Self::do_increase_validator_delegate_stake(validator_id, node_delegate_reward);
            // reads:
            // ValidatorDelegateStakeBalance | ValidatorDelegateStakeShares |
            // TotalValidatorDelegateStakeBalance
            //
            // writes:
            // ValidatorDelegateStakeShares | ValidatorDelegateStakeBalance |
            // TotalValidatorDelegateStakeBalance
            weight_meter.consume(db_weight.reads_writes(5, 3));

            return Some((updated_account_reward, node_delegate_reward));
        }
        None
    }

    pub fn handle_delegate_account(
        account_reward: u128,
        delegate_account_id: &T::AccountId,
        rate: u128,
    ) -> (u128, u128) {
        let delegate_account_deposit = Self::percent_mul(account_reward, rate);
        let updated_account_reward = account_reward.saturating_sub(delegate_account_deposit);
        Self::increase_delegate_account_balance(delegate_account_id, delegate_account_deposit);

        (updated_account_reward, delegate_account_deposit)
    }
}
