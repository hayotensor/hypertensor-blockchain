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
//
// Handles all slot block steps

use super::*;
use frame_support::pallet_prelude::Weight;

impl<T: Config> Pallet<T> {
    pub const MIN_CONSENSUS_VALIDATOR_IDENTITIES: u32 = 3;

    pub fn has_minimum_consensus_validator_identity_set(
        eligible_validator_identity_count: u32,
    ) -> bool {
        eligible_validator_identity_count >= Self::MIN_CONSENSUS_VALIDATOR_IDENTITIES
    }

    pub fn min_consensus_identity_attestation_count(
        eligible_validator_identity_count: u32,
        min_identity_attestation_percentage: u128,
    ) -> u32 {
        match eligible_validator_identity_count {
            0 => return 0,
            1 => return 1,
            2 | 3 => return 2,
            _ => {}
        }

        let percentage_factor = Self::percentage_factor_as_u128();
        let numerator = (eligible_validator_identity_count as u128)
            .saturating_mul(min_identity_attestation_percentage.min(percentage_factor))
            .saturating_add(percentage_factor.saturating_sub(1));
        let mut required = numerator / percentage_factor;

        required = required.max(Self::MIN_CONSENSUS_VALIDATOR_IDENTITIES as u128);

        required.min(eligible_validator_identity_count as u128) as u32
    }

    /// Smallest number of distinct eligible identities whose integer fixed-point ratio reaches
    /// `required_ratio`.
    ///
    /// This is also the minimum number of node attestations needed because one node attestation
    /// can represent at most one validator identity. Saturating arithmetic keeps hook weight
    /// selection conservative if a malformed ratio ever exceeds the percentage factor.
    pub fn min_identity_attestors_for_ratio(
        eligible_validator_identity_count: u32,
        required_ratio: u128,
    ) -> u32 {
        if eligible_validator_identity_count == 0 || required_ratio == 0 {
            return 0;
        }

        let percentage_factor = Self::percentage_factor_as_u128();
        let numerator = (eligible_validator_identity_count as u128)
            .saturating_mul(required_ratio.min(percentage_factor));
        numerator
            .saturating_add(percentage_factor.saturating_sub(1))
            .saturating_div(percentage_factor)
            .min(eligible_validator_identity_count as u128) as u32
    }

    pub fn effective_min_consensus_identity_attestation_percentage(
        eligible_validator_identity_count: u32,
        min_identity_attestation_percentage: u128,
    ) -> u128 {
        let required = Self::min_consensus_identity_attestation_count(
            eligible_validator_identity_count,
            min_identity_attestation_percentage,
        );

        Self::percent_div(required as u128, eligible_validator_identity_count as u128)
            .clamp(0, Self::percentage_factor_as_u128())
    }

    // Returns subnet weights, node scores, and db weight
    pub fn calculate_overwatch_rewards() -> Weight {
        let mut weight = Weight::zero();
        let db_weight = T::DbWeight::get();

        // Taking the settlement makes finalization idempotent: a second invocation cannot score or
        // mint rewards for the same epoch. Empty epochs are finalized as well so consumers can
        // distinguish "no score" from "not processed yet".
        let Some(settlement) = PendingOverwatchSettlement::<T>::take() else {
            return db_weight.reads(1);
        };
        weight = weight.saturating_add(db_weight.reads_writes(1, 1));

        LastFinalizedOverwatchEpoch::<T>::put(settlement.epoch);
        weight = weight.saturating_add(db_weight.writes(1));

        let percentage_factor = Self::percentage_factor_as_u128();

        let stake_weight_pow: f64 =
            Self::get_percent_as_f64(OverwatchStakeWeightFactor::<T>::get());
        weight = weight.saturating_add(db_weight.reads(1));
        let mut total_stake_weight: u128 = 0;

        // {node_id, score}
        let mut node_total_scores: BTreeMap<u32, u128> = BTreeMap::new();
        // {node_id, account_id}
        let mut node_hotkeys: BTreeMap<u32, T::AccountId> = BTreeMap::new();

        let total_stake = TotalOverwatchNodeStakeBalance::<T>::get();
        // TotalOverwatchNodeStakeBalance
        weight = weight.saturating_add(db_weight.reads(1));

        // Step 1: Group reveals by subnet
        // {node_id, stake_weight}
        let mut node_stake_weights: BTreeMap<u32, u128> = BTreeMap::new();
        // {subnet_id, (subnet_weight sum, {node_id, subnet_weight})}
        let mut subnet_reveals: BTreeMap<u32, (u128, BTreeMap<u32, u128>)> = BTreeMap::new();
        for ((subnet_id, overwatch_node_id), subnet_weight) in
            OverwatchReveals::<T>::iter_prefix((settlement.epoch,))
        {
            // OverwatchReveals
            // Get stake weights of all revealing nodes
            weight = weight.saturating_add(db_weight.reads(1));

            if node_stake_weights.get(&overwatch_node_id).is_none() {
                weight = weight.saturating_add(db_weight.reads(1));
                let Some(overwatch_node) = OverwatchNodes::<T>::get(overwatch_node_id) else {
                    continue;
                };

                let stake_balance = OverwatchNodeStakeBalance::<T>::get(overwatch_node_id);
                // OverwatchNodeStakeBalance
                weight = weight.saturating_add(db_weight.reads(1));

                let stake_weight_adj =
                    Self::get_f64_as_percentage(Self::pow(stake_balance as f64, stake_weight_pow));

                total_stake_weight = total_stake_weight.saturating_add(stake_weight_adj);

                node_stake_weights.insert(overwatch_node_id, stake_weight_adj);
                node_hotkeys.insert(overwatch_node_id, overwatch_node.hotkey.clone());
            }

            let entry = subnet_reveals
                .entry(subnet_id)
                .or_insert((0, BTreeMap::new()));
            entry.0 = entry.0.saturating_add(subnet_weight); // sum all weights for this subnet
            entry.1.insert(overwatch_node_id, subnet_weight); // store each node's weight per subnet (subnet weight the overwatch submitted)
        }

        // Normalize stake weights
        if total_stake_weight == 0 {
            for stake_weight in node_stake_weights.values_mut() {
                *stake_weight = 0;
            }
        } else {
            for stake_weight in node_stake_weights.values_mut() {
                *stake_weight = Self::percent_div(*stake_weight, total_stake_weight);
            }
        }

        // Step 2: Iterate each subnet
        // - Get subnet weights from nodes
        // - Score nodes
        for (&subnet_id, (_sum_weights, node_weights)) in subnet_reveals.iter() {
            // Get node stake weight
            let total_adjusted: u128 = node_weights
                .iter()
                .filter_map(|(&node_id, subnet_weight)| {
                    node_stake_weights
                        .get(&node_id)
                        .map(|stake_weight| Self::percent_mul(*subnet_weight, *stake_weight))
                })
                .fold(0u128, |acc, value| acc.saturating_add(value))
                .min(percentage_factor);

            //
            // --- Score subnets
            //

            // Data only (currently)
            OverwatchSubnetWeights::<T>::insert(settlement.epoch, subnet_id, total_adjusted);
            weight = weight.saturating_add(db_weight.writes(1));

            // Step 2c: Score nodes and accumulate
            for (&node_id, &subnet_weight) in node_weights.iter() {
                // Get the deviation from the resulting score.
                // We check the abs diff since the submitted weights can only be between 0.0-1.0 [*1e18]
                let deviation = subnet_weight.abs_diff(total_adjusted);
                let closeness_score = percentage_factor.saturating_sub(deviation);
                let node_final_score = Self::percent_mul(closeness_score, total_adjusted);

                // Step 3: Accumulate score
                let score = node_total_scores.entry(node_id).or_insert(0);
                *score = score.saturating_add(node_final_score);
            }
        }

        //
        // Step 4: Normalize node scores
        //
        let total_final_score: u128 = node_total_scores
            .values()
            .fold(0u128, |acc, score| acc.saturating_add(*score));
        let mut node_rewards: Vec<(u32, u128)> = Vec::new();
        if total_final_score == 0 {
            Self::deposit_event(Event::OverwatchEpochFinalized {
                epoch: settlement.epoch,
                node_rewards,
            });
            return weight;
        }

        //
        // Step 5: Reward nodes
        //
        // `OverwatchEpochEmissions` is the budget for one general blockchain epoch. Overwatch
        // nodes submit only once per multiplied interval, so the completed interval receives all
        // general-epoch budgets that it spans.
        let ow_emissions = T::OverwatchEpochEmissions::get()
            .saturating_mul(settlement.epoch_length_multiplier as u128);

        for (node_id, score) in node_total_scores.iter() {
            if *score == 0 {
                continue;
            }

            let node_final_score = Self::percent_div(*score, total_final_score);

            // For data purposes only
            OverwatchNodeWeights::<T>::insert(settlement.epoch, node_id, node_final_score);
            weight = weight.saturating_add(db_weight.writes(1));

            // Skip if no hotkey
            let Some(hotkey) = node_hotkeys.get(&node_id) else {
                continue;
            };

            let amount = Self::percent_mul(node_final_score, ow_emissions);
            if amount == 0 {
                continue;
            }

            Self::increase_overwatch_node_stake(*node_id, amount);
            weight = weight.saturating_add(db_weight.reads_writes(2, 2));

            node_rewards.push((*node_id, amount));
        }

        Self::deposit_event(Event::OverwatchEpochFinalized {
            epoch: settlement.epoch,
            node_rewards,
        });

        weight
    }

    /// - Generates emissions variables to distribute emissions: `precheck_subnet_consensus_submission`
    /// - Distributes emissions: `distribute_rewards`
    /// - Elects validator: `elect_validator`
    /// - Handles registration queue (i.e., activates nodes from the queue): `handle_registration_queue`
    /// = Updates burn rate EMA: `update_burn_rate_for_epoch`
    pub fn emission_step(
        weight_meter: &mut WeightMeter,
        block: u32,
        current_epoch: u32,
        current_subnet_epoch: u32,
        subnet_id: u32,
    ) {
        let db_weight = T::DbWeight::get();

        // We know the subnet exists because to call this function `SlotAssignment` must exist
        // for the given subnet_id called in `on_initialize` on this block step.

        // Get allocations calculated at the start of the general epoch. Allocation authority
        // is an exact historical election, so a round remains settleable even if its subnet is
        // paused by this block. Current lifecycle state only gates the operational work below.

        // FinalSubnetEmissionWeights
        weight_meter.consume(db_weight.reads(1));

        if let Some(previous_subnet_epoch) = current_subnet_epoch.checked_sub(1) {
            if let Ok(subnet_emission_weights) =
                FinalSubnetEmissionWeights::<T>::try_get(current_epoch)
            {
                // Get the subnet's allocation for settling the exact previous subnet epoch.
                if let Some(&subnet_weight) = subnet_emission_weights.subnet_weights.get(&subnet_id)
                {
                    weight_meter.consume(db_weight.reads(1));
                    let historical_items = SubnetConsensusSubmissionMaxItems::<T>::get(
                        subnet_id,
                        previous_subnet_epoch,
                    )
                    .min(T::MaxSubnetNodesUpperBound::get());
                    weight_meter.consume(db_weight.reads(1));
                    let precheck_weight = if historical_items == 0 {
                        T::WeightInfo::precheck_subnet_consensus_submission_missing()
                    } else {
                        T::WeightInfo::precheck_subnet_consensus_submission(historical_items)
                    };

                    if weight_meter.can_consume(precheck_weight) {
                        let (consensus_submission_data, _) =
                            Self::precheck_subnet_consensus_submission(
                                subnet_id,
                                previous_subnet_epoch,
                                current_epoch,
                            );
                        weight_meter.consume(precheck_weight);

                        if let Some(consensus_submission_data) = consensus_submission_data {
                            let policy = consensus_submission_data.policy;
                            // Calculate rewards
                            let (rewards_data, rewards_block_weight) =
                                Self::calculate_rewards_with_policy(
                                    subnet_emission_weights.subnets_emissions,
                                    subnet_weight,
                                    &consensus_submission_data.policy,
                                );
                            weight_meter.consume(rewards_block_weight);

                            // Distribute rewards
                            Self::distribute_rewards(
                                weight_meter,
                                subnet_id,
                                block,
                                current_epoch,
                                current_subnet_epoch, // used for graduating nodes
                                consensus_submission_data,
                                rewards_data,
                                policy.min_attestation_percentage,
                                policy.validator_reputation_increase_factor,
                                policy.validator_reputation_decrease_factor,
                                policy.super_majority_attestation_ratio,
                            );
                        }
                    }
                }
            }
        }

        // Operational subnet-epoch work must not depend on a reward allocation. Queue and
        // burn maintenance run while Active so the preparation epoch is useful, whereas a
        // validator election additionally requires the consensus eligibility epoch to be reached.
        weight_meter.consume(db_weight.reads(1));
        if let Ok(subnet) = SubnetsData::<T>::try_get(subnet_id) {
            if subnet.state == SubnetState::Active {
                if Self::_is_subnet_active_and_live(&subnet, current_subnet_epoch) {
                    // Read the regular candidate cardinality before mutating election state. The
                    // selector uses the configured emergency maximum rather than decoding the
                    // variable-sized emergency vector before weight has been reserved.
                    let cardinality_read_weight = db_weight.reads(1);
                    if weight_meter.can_consume(cardinality_read_weight) {
                        weight_meter.consume(cardinality_read_weight);
                        let candidate_count = Self::elect_validator_weight_component(subnet_id);
                        let max_emergency = T::MaxEmergencySubnetNodesUpperBound::get();
                        let election_weight = T::WeightInfo::elect_validator(candidate_count)
                            .max(T::WeightInfo::elect_validator_emergency(max_emergency))
                            .max(T::WeightInfo::elect_validator_expired(
                                candidate_count,
                                max_emergency,
                            ));

                        if weight_meter.can_consume(election_weight) {
                            weight_meter.consume(election_weight);
                            Self::elect_validator(subnet_id, current_subnet_epoch, block);
                        }
                    }
                }

                // Keep node readiness and registration pricing moving during preparation.
                Self::handle_registration_queue(weight_meter, subnet_id, current_subnet_epoch);

                // This will run if there is block weight remaining to call.
                Self::update_burn_rate_for_epoch(weight_meter, subnet_id);
            }
        }
    }

    /// Activate nodes in the queue
    pub fn handle_registration_queue(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        current_subnet_epoch: u32,
    ) {
        let db_weight = T::DbWeight::get();

        // Initial weight check - need at least 7 reads to proceed. Resolving the pending queue
        // duration reads both its live and pending storage values.
        if !weight_meter.can_consume(db_weight.reads(7)) {
            return;
        }

        let churn_limit_multiplier = ChurnLimitMultiplier::<T>::get(subnet_id);
        weight_meter.consume(db_weight.reads(1));

        // Only process the queue based on the churn_limit_multiplier
        // E.g. If multiplier is 4, only run every 4 epochs. If 1, run every epoch.
        if current_subnet_epoch % churn_limit_multiplier != 0 {
            return;
        }

        let subnet_node_queue_epochs =
            Self::get_subnet_node_queue_epochs_for_epoch(subnet_id, current_subnet_epoch);
        let max_nodes = MaxSubnetNodes::<T>::get();
        let total_active_nodes = TotalActiveSubnetNodes::<T>::get(subnet_id);
        let churn_limit = ChurnLimit::<T>::get(subnet_id);

        // Consume weight for the 5 storage reads above
        weight_meter.consume(db_weight.reads(5));

        // Calculate how many nodes to process
        let take = if max_nodes.saturating_sub(total_active_nodes) < churn_limit {
            max_nodes.saturating_sub(total_active_nodes)
        } else {
            churn_limit
        };

        // Check if we can afford to read the queue
        if !weight_meter.can_consume(db_weight.reads(1)) {
            return;
        }

        // Get all of the nodes in the queue (Registered classified nodes)
        let mut queue = SubnetNodeQueue::<T>::get(subnet_id);
        weight_meter.consume(db_weight.reads(1));

        if queue.len() == 0 || take == 0 {
            return;
        }

        // Check if we can afford the base queue processing weight
        let base_processing_weight = Weight::from_parts(2_000, 0);
        if !weight_meter.can_consume(base_processing_weight) {
            return;
        }
        weight_meter.consume(base_processing_weight);

        let mut activated_nodes = 0;
        let nodes_to_process: Vec<_> = queue.iter().take(take as usize).collect();

        for subnet_node in nodes_to_process {
            // Check if node is eligible for activation first (early exit)
            if !Self::has_epoch_period_elapsed(
                subnet_node.classification.start_epoch,
                subnet_node_queue_epochs,
                current_subnet_epoch,
            ) {
                // Nodes are ordered by epoch, so we can break early if the first node
                // is not ready yet to be activated from the queue
                break;
            }

            // Calculate total weight needed for this activation INCLUDING guaranteed cleanup and db updates
            let per_node_processing_weight = Weight::from_parts(1_500, 0);
            let per_node_cleanup_weight = Weight::from_parts(500, 0);
            let storage_write_weight = if activated_nodes == 0 {
                db_weight.writes(1) // Only count the storage write once
            } else {
                Weight::zero()
            };

            let total_weight_needed = per_node_processing_weight
                .saturating_add(per_node_cleanup_weight)
                .saturating_add(storage_write_weight)
                .saturating_add(db_weight.reads_writes(5, 5)); // Account for do_activate_subnet_node weight consumption

            // Check if we can consume the complete operation (activation + cleanup + db updates)
            if !weight_meter.can_consume(total_weight_needed) {
                break;
            }

            // Consume the per-node processing weight
            weight_meter.consume(per_node_processing_weight);

            // Attempt activation
            let can_consume = Self::do_activate_subnet_node(
                weight_meter,
                subnet_node.validator_id,
                subnet_id,
                SubnetState::Active, // We know the subnet is active if `handle_registration_queue` is called
                subnet_node.clone(),
                current_subnet_epoch,
                true,
            );

            if !can_consume {
                break; // Stop if activation failed due to weight constraints or other reasons
            }

            activated_nodes += 1;
        }

        // Cleanup: We've pre-calculated that we can afford this
        if activated_nodes > 0 {
            // Consume the cleanup weights we reserved
            let total_drain_weight = Weight::from_parts(500 * activated_nodes as u64, 0);
            weight_meter.consume(total_drain_weight);
            queue.drain(0..activated_nodes);

            // Consume the storage write weight we reserved
            weight_meter.consume(db_weight.writes(1));
            SubnetNodeQueue::<T>::set(subnet_id, queue);
        }
    }

    /// Calculate and store emissions distribution
    ///
    pub fn handle_subnet_emission_weights(epoch: u32) -> Weight {
        // Get subnet weights
        // - Takes in general epoch (not subnet epochs)
        let (subnet_weights, mut weight): (BTreeMap<u32, u128>, Weight) =
            Self::calculate_subnet_weights(epoch);

        // Store weights and handle foundation
        if !subnet_weights.is_empty() {
            let (subnets_emissions, foundation_emissions_as_u128) =
                Self::get_epoch_emissions(epoch);

            if let Some(foundation_emissions) = Self::u128_to_balance(foundation_emissions_as_u128)
            {
                Self::add_balance_to_treasury(foundation_emissions);
                weight = weight.saturating_add(T::WeightInfo::add_balance_to_treasury());
            }

            let data = DistributionData {
                subnets_emissions,
                subnet_weights,
            };
            FinalSubnetEmissionWeights::<T>::insert(epoch, data);
            weight = weight.saturating_add(T::DbWeight::get().writes(1));
        }

        weight
    }

    /// Calculate emissions distribution weights
    ///
    /// # Based On
    /// - Delegate stake weight
    /// - Node count weight
    /// - Overwatch weight
    ///
    /// This calculates the distribution of emissions to each subnet
    ///
    pub fn calculate_subnet_weights(epoch: u32) -> (BTreeMap<u32, u128>, Weight) {
        let mut weight = Weight::zero();
        let db_weight = T::DbWeight::get();

        let subnet_distribution_power =
            Self::get_percent_as_f64(SubnetDistributionPower::<T>::get());

        // {subnet_id, weight}
        let mut subnet_weights: BTreeMap<u32, f64> = BTreeMap::new();
        // {subnet_id, count}
        let mut subnet_weight_sum: f64 = 0.0;
        let mut total_subnet_reads = 0u64;

        let weight_factors = SubnetWeightFactors::<T>::get();
        weight = weight.saturating_add(db_weight.reads(1));
        let delegate_stake_factor = Self::get_percent_as_f64(weight_factors.delegate_stake);
        let node_count_factor = Self::get_percent_as_f64(weight_factors.node_count);
        let net_flow_factor = Self::get_percent_as_f64(weight_factors.net_flow);

        // SubnetDistributionPower
        weight = weight.saturating_add(db_weight.reads(1));

        let last_finalized_overwatch_epoch = LastFinalizedOverwatchEpoch::<T>::get();
        // LastFinalizedOverwatchEpoch
        weight = weight.saturating_add(db_weight.reads(1));

        // Only subnet IDs are needed below. Avoid decoding and proving every variable-size
        // `SubnetData` value on this hook-critical path.
        let subnet_ids: Vec<u32> = SubnetsData::<T>::iter_keys().collect();

        // At general slot two, every assigned subnet is still in the local epoch whose election
        // completed at its slot in the preceding general epoch. Derive that phase from the
        // allocation epoch itself so this calculation is deterministic even when invoked by a
        // benchmark or test outside the hook. In `on_initialize`, this is exactly the current
        // block (`epoch * EpochLength + 2`).
        let allocation_block = epoch
            .saturating_mul(T::EpochLength::get())
            .saturating_add(2);
        let mut eligible_subnet_totals: BTreeMap<u32, (u128, u32)> = BTreeMap::new();
        let mut total_delegate_stake = 0u128;
        let mut total_electable_nodes = 0u32;
        let mut eligibility_reads = 0u64;
        for subnet_id in subnet_ids.iter() {
            let current_subnet_epoch =
                Self::get_subnet_epoch_with_block_as_u32(*subnet_id, allocation_block);
            // SubnetSlot
            eligibility_reads = eligibility_reads.saturating_add(1);

            // Lifecycle changes after an election must not erase that historical round. An exact
            // election is the allocation authority: newly activated/preparing subnets still have
            // no exact election, while a subnet paused after electing remains eligible to settle.
            // SubnetElectedValidator
            eligibility_reads = eligibility_reads.saturating_add(1);
            if SubnetElectedValidator::<T>::contains_key(subnet_id, current_subnet_epoch) {
                let delegate_stake = TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);
                let electable_nodes = TotalSubnetElectableNodes::<T>::get(subnet_id);
                eligibility_reads = eligibility_reads.saturating_add(2);
                total_delegate_stake = total_delegate_stake.saturating_add(delegate_stake);
                total_electable_nodes = total_electable_nodes.saturating_add(electable_nodes);
                eligible_subnet_totals.insert(*subnet_id, (delegate_stake, electable_nodes));
            }
        }
        weight = weight.saturating_add(db_weight.reads(eligibility_reads));

        // Every factor denominator must use the same settlement cohort. Otherwise a
        // preparing subnet could still change existing subnets' relative rewards merely
        // by holding stake or electable nodes, despite being excluded from the numerators.
        let reward_eligible_subnets: BTreeSet<u32> =
            eligible_subnet_totals.keys().copied().collect();

        let (inflow_weights, inflow_weight_calc_weight) = Self::get_net_flow_weights_for_eligible(
            subnet_ids.clone(),
            epoch,
            &reward_eligible_subnets,
        );
        weight = weight.saturating_add(inflow_weight_calc_weight);

        for subnet_id in subnet_ids {
            total_subnet_reads += 1;
            // A subnet must have an exact prior election to receive an allocation for the
            // consensus epoch being settled. Its current lifecycle state cannot erase that work.
            let Some((total_subnet_delegate_stake, electable_nodes_count)) =
                eligible_subnet_totals.get(&subnet_id).copied()
            else {
                continue;
            };

            // - Get delegate stake weight in f64
            let subnet_dstake_weight: f64 = if total_delegate_stake == 0 {
                0.0
            } else {
                (total_subnet_delegate_stake as f64 / total_delegate_stake as f64).clamp(0.0, 1.0)
            };

            // - Get node count weight in f64
            let subnet_nodes_weight = if total_electable_nodes == 0 {
                0.0
            } else {
                electable_nodes_count as f64 / total_electable_nodes as f64
            };

            // - Get Overwatch weight in f64
            let overwatch_subnet_weight =
                match last_finalized_overwatch_epoch.and_then(|overwatch_epoch| {
                    OverwatchSubnetWeights::<T>::try_get(overwatch_epoch, subnet_id).ok()
                }) {
                    Some(weight) => (Self::get_percent_as_f64(weight)
                        * Self::get_percent_as_f64(OverwatchWeightFactor::<T>::get()))
                    .min(1.0),
                    None => Self::get_percent_as_f64(DefaultOverwatchSubnetWeight::<T>::get()),
                };

            // OverwatchSubnetWeights
            weight = weight.saturating_add(db_weight.reads(1));

            // - Get combined weight (stake + node count + inflow) * overwatchers weight

            let subnet_inflow_weight =
                Self::get_percent_as_f64(inflow_weights.get(&subnet_id).cloned().unwrap_or(0));
            let subnet_weight = ((subnet_dstake_weight * delegate_stake_factor
                + subnet_nodes_weight * node_count_factor
                + subnet_inflow_weight * net_flow_factor)
                * overwatch_subnet_weight)
                .clamp(0.0, 1.0);

            // - Adj weight (to later be normalized)
            let adj_subnet_weight: f64 = Self::pow(subnet_weight, subnet_distribution_power);

            if !adj_subnet_weight.is_finite() {
                continue;
            }

            subnet_weights.insert(subnet_id, adj_subnet_weight);
            subnet_weight_sum += adj_subnet_weight;
            weight = weight.saturating_add(Weight::from_parts(400_000, 0));
        }

        weight = weight.saturating_add(db_weight.reads(total_subnet_reads));
        let mut subnet_weights_normalized: BTreeMap<u32, u128> = BTreeMap::new();
        let percentage_factor = Self::percentage_factor_as_u128();
        let mut remaining_weight = percentage_factor;

        // --- Normalize delegate stake weights from power
        for (subnet_id, subnet_weight) in subnet_weights {
            if subnet_weight_sum <= 0.0 || !subnet_weight_sum.is_finite() {
                continue;
            }
            let weight_normalized_f64 =
                subnet_weight / subnet_weight_sum * percentage_factor as f64;
            if !weight_normalized_f64.is_finite() || weight_normalized_f64 <= 0.0 {
                continue;
            }
            // Independent f64-to-integer conversion can round the aggregate a few atomic units
            // above 100%. Cap each deterministic BTreeMap entry by the remaining allocation so
            // downstream subnet rewards can never exceed the subnet emissions budget.
            let weight_normalized =
                (weight_normalized_f64.min(percentage_factor as f64) as u128).min(remaining_weight);
            if weight_normalized == 0 {
                continue;
            }
            subnet_weights_normalized.insert(subnet_id, weight_normalized);
            remaining_weight = remaining_weight.saturating_sub(weight_normalized);
            weight = weight.saturating_add(Weight::from_parts(400_000, 0));
        }

        //
        // Weight calc complete
        //

        (subnet_weights_normalized, weight)
    }

    pub fn get_net_flow_weights(
        subnet_ids: Vec<u32>,
        _epoch: u32,
    ) -> (BTreeMap<u32, u128>, Weight) {
        let mut lifecycle_reads = 0u64;
        let eligible_subnets: BTreeSet<u32> = subnet_ids
            .iter()
            .filter_map(|subnet_id| {
                let data = SubnetsData::<T>::get(subnet_id)?;
                let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(*subnet_id);
                // SubnetsData | SubnetSlot
                lifecycle_reads = lifecycle_reads.saturating_add(2);
                Self::_is_subnet_active_and_live(&data, current_subnet_epoch).then_some(*subnet_id)
            })
            .collect();

        let (weights, weight) =
            Self::get_net_flow_weights_for_eligible(subnet_ids, _epoch, &eligible_subnets);
        (
            weights,
            weight.saturating_add(T::DbWeight::get().reads(lifecycle_reads)),
        )
    }

    fn get_net_flow_weights_for_eligible(
        subnet_ids: Vec<u32>,
        _epoch: u32,
        eligible_subnets: &BTreeSet<u32>,
    ) -> (BTreeMap<u32, u128>, Weight) {
        let mut weight = Weight::zero();
        let db_weight = T::DbWeight::get();

        let mut inflows: BTreeMap<u32, i128> = BTreeMap::new();

        for subnet_id in subnet_ids {
            // Take/remove the netflow to restart calculation and return the net flow
            let net_flow = SubnetNetFlow::<T>::take(subnet_id);
            weight = weight.saturating_add(db_weight.reads_writes(1, 1));

            // Raw flow is reset for every subnet, but only the reward-eligible cohort
            // participates in this epoch's relative normalization and smoothing.
            if !eligible_subnets.contains(&subnet_id) {
                SubnetNetFlowSmoothedWeight::<T>::remove(subnet_id);
                weight = weight.saturating_add(db_weight.writes(1));
                continue;
            }

            inflows.insert(subnet_id, net_flow);
        }

        let min = inflows.values().cloned().min().unwrap_or(0);

        let mut shifted: BTreeMap<u32, u128> = BTreeMap::new();
        for (subnet_id, value) in inflows.iter() {
            let shifted_value = value.saturating_sub(min);
            shifted.insert(
                *subnet_id,
                if shifted_value <= 0 {
                    0
                } else {
                    shifted_value as u128
                },
            );
        }

        let sum: u128 = shifted
            .values()
            .fold(0u128, |acc, value| acc.saturating_add(*value));

        let mut current_inflow_weights: BTreeMap<u32, u128> = BTreeMap::new();
        for (subnet_id, value) in shifted.iter() {
            let inflow_weight = if sum == 0 {
                0
            } else {
                Self::percent_div(*value, sum)
            };
            current_inflow_weights.insert(*subnet_id, inflow_weight);
        }

        let smoothing_alpha = SubnetNetFlowSmoothingAlpha::<T>::get();
        let inverse_alpha = Self::percentage_factor_as_u128().saturating_sub(smoothing_alpha);
        weight = weight.saturating_add(db_weight.reads(1));

        let mut smoothed_inflow_weights: BTreeMap<u32, u128> = BTreeMap::new();
        for subnet_id in inflows.keys() {
            let current_weight = current_inflow_weights.get(subnet_id).copied().unwrap_or(0);
            let previous_weight = SubnetNetFlowSmoothedWeight::<T>::get(subnet_id);
            weight = weight.saturating_add(db_weight.reads(1));

            let smoothed_weight = Self::percent_mul(current_weight, smoothing_alpha)
                .saturating_add(Self::percent_mul(previous_weight, inverse_alpha))
                .min(Self::percentage_factor_as_u128());

            if smoothed_weight == 0 {
                SubnetNetFlowSmoothedWeight::<T>::remove(subnet_id);
            } else {
                SubnetNetFlowSmoothedWeight::<T>::insert(subnet_id, smoothed_weight);
            }
            weight = weight.saturating_add(db_weight.writes(1));

            smoothed_inflow_weights.insert(*subnet_id, smoothed_weight);
        }

        (smoothed_inflow_weights, weight)
    }

    pub fn precheck_subnet_consensus_submission(
        subnet_id: u32,
        prev_subnet_epoch: u32,
        current_epoch: u32,
    ) -> (Option<ConsensusSubmissionData<T>>, Weight) {
        let mut weight = Weight::zero();
        let db_weight = T::DbWeight::get();

        // SubnetConsensusSubmission
        weight = weight.saturating_add(db_weight.reads(1));

        let submission = match SubnetConsensusSubmission::<T>::try_get(subnet_id, prev_subnet_epoch)
        {
            Ok(submission) => submission,
            Err(()) => {
                // Check if a validator was elected
                // - Make sure they did their job and submitted consensus data
                // - If not, penalize the subnet and validator
                weight = weight.saturating_add(db_weight.reads(1));
                if let Some(round) = SubnetElectedValidator::<T>::get(subnet_id, prev_subnet_epoch)
                {
                    let validator_subnet_node_id = round.validator_subnet_node_id;

                    // A missing proposal has zero support. Apply economic losses and record a zero
                    // validator-identity support sample, while leaving score loss to the existing
                    // absence-specific node and subnet penalties below.
                    let (validator_id, _, _, slash_weight) = Self::apply_validator_economic_slashes(
                        subnet_id,
                        validator_subnet_node_id,
                        0,
                        round.policy.min_attestation_percentage,
                        round.policy.base_slash_percentage,
                        round.policy.max_slash_amount,
                        0,
                        round.validator_delegate_stake_balance,
                        round.policy.validator_delegate_stake_slash_threshold,
                        round.policy.base_validator_delegate_stake_slash_percentage,
                        round.policy.max_validator_delegate_stake_slash_amount,
                    );
                    weight = weight.saturating_add(slash_weight);
                    if let Some(validator_id) = validator_id {
                        Self::record_validator_identity_support(validator_id, 0);
                        // ValidatorReputation::contains_key + get + insert.
                        weight = weight.saturating_add(db_weight.reads_writes(2, 1));
                    }

                    //
                    // Update subnet rep
                    //
                    Self::decrease_subnet_reputation(
                        subnet_id,
                        round.policy.validator_absent_subnet_reputation_factor,
                        None,
                    );
                    weight = weight.saturating_add(db_weight.reads_writes(1, 1));

                    // The elected validator cannot remove self if elected so we don't check if they exist

                    //
                    // Update node rep
                    //
                    let reputation_factors = round.policy.reputation_factors;
                    if let Some(rep) =
                        SubnetNodeReputation::<T>::get(subnet_id, validator_subnet_node_id)
                    {
                        Self::decrease_and_return_node_reputation(
                            subnet_id,
                            validator_subnet_node_id,
                            rep,
                            reputation_factors.validator_absent_decrease,
                            None,
                        );
                    }

                    // NOTE: We don't check if below minimum node reputation here to possibly
                    // remove the node from the subnet, as this is done in the bank/rewards.rs ``distribute_rewards``

                    weight = weight.saturating_add(db_weight.reads_writes(1, 1));
                }

                return (None, weight);
            }
        };

        let Some(round) = SubnetElectedValidator::<T>::get(subnet_id, prev_subnet_epoch) else {
            return (None, weight.saturating_add(db_weight.reads(1)));
        };
        weight = weight.saturating_add(db_weight.reads(1));

        weight = weight.saturating_add(db_weight.reads(1));
        let Some(attestor_weight_snapshot) =
            SubnetConsensusAttestorWeights::<T>::get(subnet_id, prev_subnet_epoch)
        else {
            return (None, weight);
        };

        let attestation_ratio = if attestor_weight_snapshot.total_weight > 0 {
            let attested_weight =
                match submission
                    .attests
                    .keys()
                    .try_fold(0u128, |acc, subnet_node_id| {
                        acc.checked_add(
                            attestor_weight_snapshot
                                .weights
                                .get(subnet_node_id)
                                .copied()
                                .unwrap_or(0),
                        )
                    }) {
                    Some(weight) => weight,
                    None => return (None, weight),
                };

            Self::percent_div(attested_weight, attestor_weight_snapshot.total_weight)
                .clamp(0, Self::percentage_factor_as_u128())
        } else {
            0
        };

        let eligible_validator_identities: BTreeSet<u32> = submission
            .validator_identity_ids
            .values()
            .copied()
            .collect();
        let eligible_validator_identity_count = eligible_validator_identities.len() as u32;
        let attesting_validator_identities: BTreeSet<u32> = submission
            .attests
            .keys()
            .filter_map(|subnet_node_id| {
                submission
                    .validator_identity_ids
                    .get(subnet_node_id)
                    .copied()
            })
            .collect();
        let identity_attestation_count = attesting_validator_identities.len() as u32;
        let identity_attestation_ratio = if eligible_validator_identity_count > 0 {
            Self::percent_div(
                identity_attestation_count as u128,
                eligible_validator_identity_count as u128,
            )
            .clamp(0, Self::percentage_factor_as_u128())
        } else {
            0
        };

        let (data, weight_sum) = match Self::canonicalize_consensus_data_entries(submission.data) {
            Ok(canonical_data) => canonical_data,
            Err(_) => return (None, weight),
        };

        let consensus_data = ConsensusSubmissionData::<T> {
            policy: round.policy,
            validator_subnet_node_id: submission.validator_id,
            validator_delegate_stake_balance: round.validator_delegate_stake_balance,
            validator_epoch_progress: submission.validator_epoch_progress,
            validator_reward_factor: submission.validator_reward_factor,
            attestation_ratio,
            identity_attestation_ratio,
            identity_attestation_count,
            eligible_validator_identity_count,
            weight_sum,
            data_length: data.len() as u32,
            data,
            attests: submission.attests,
            subnet_nodes: submission.subnet_nodes,
            prioritize_queue_node_id: submission.prioritize_queue_node_id,
            remove_queue_node_id: submission.remove_queue_node_id,
            emergency: submission.emergency,
        };

        (Some(consensus_data), weight)
    }

    /// Calculate the subnets rewards and how they are distributed throughout the subnet
    ///
    /// # Arguments
    ///
    /// * `subnet_id` - The id of the subnet to calculate rewards for
    /// * `overall_rewards` - The total rewards for all subnets this epoch
    /// * `emission_weight` - The weight of the subnet
    ///
    /// # Returns
    ///
    /// * `RewardsData` - The rewards data for the subnet
    /// * `Weight` - The weight of the subnet
    ///
    pub fn calculate_rewards(
        subnet_id: u32,
        overall_rewards: u128,
        emission_weight: u128,
    ) -> (RewardsData, Weight) {
        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        Self::calculate_rewards_for_epoch(
            subnet_id,
            overall_rewards,
            emission_weight,
            current_subnet_epoch,
        )
    }

    pub fn calculate_rewards_for_epoch(
        subnet_id: u32,
        overall_rewards: u128,
        emission_weight: u128,
        current_subnet_epoch: u32,
    ) -> (RewardsData, Weight) {
        let mut weight = Weight::zero();
        let db_weight = T::DbWeight::get();

        let overall_subnet_reward: u128 = Self::percent_mul(overall_rewards, emission_weight);

        // --- Get owner rewards
        let subnet_owner_percentage = SubnetOwnerPercentage::<T>::get();
        weight = weight.saturating_add(db_weight.reads(1));
        let subnet_owner_reward: u128 =
            Self::percent_mul(overall_subnet_reward, subnet_owner_percentage);

        // --- Get subnet rewards minus owner cut
        let subnet_rewards: u128 = overall_subnet_reward.saturating_sub(subnet_owner_reward);

        // --- Get delegators rewards
        let mut delegate_stake_rewards_percentage =
            SubnetDelegateStakeRewardsPercentage::<T>::get(subnet_id);
        weight = weight.saturating_add(db_weight.reads(1));
        let evaluated_subnet_epoch = current_subnet_epoch.saturating_sub(1);
        if let Some(pending) = PendingSubnetDelegateStakeRewardsPercentage::<T>::get(subnet_id) {
            weight = weight.saturating_add(db_weight.reads(1));
            if pending.effective_subnet_epoch <= evaluated_subnet_epoch {
                delegate_stake_rewards_percentage = pending.value;
                SubnetDelegateStakeRewardsPercentage::<T>::insert(subnet_id, pending.value);
                PendingSubnetDelegateStakeRewardsPercentage::<T>::remove(subnet_id);
                weight = weight.saturating_add(db_weight.writes(2));
                Self::deposit_event(Event::SubnetDelegateStakeRewardsPercentageUpdate {
                    subnet_id,
                    owner: pending.owner,
                    value: pending.value,
                });
            }
        } else {
            weight = weight.saturating_add(db_weight.reads(1));
        }
        let delegate_stake_rewards =
            Self::percent_mul(subnet_rewards, delegate_stake_rewards_percentage);
        let subnet_node_rewards = subnet_rewards.saturating_sub(delegate_stake_rewards);
        let rewards_data = RewardsData {
            overall_subnet_reward,
            subnet_owner_reward,
            subnet_rewards,
            delegate_stake_rewards,
            subnet_node_rewards,
        };
        (rewards_data, weight)
    }

    pub fn calculate_rewards_with_policy(
        overall_rewards: u128,
        emission_weight: u128,
        policy: &ConsensusPolicySnapshot,
    ) -> (RewardsData, Weight) {
        let overall_subnet_reward = Self::percent_mul(overall_rewards, emission_weight);
        let subnet_owner_reward =
            Self::percent_mul(overall_subnet_reward, policy.subnet_owner_percentage);
        let subnet_rewards = overall_subnet_reward.saturating_sub(subnet_owner_reward);
        let delegate_stake_rewards: u128 = Self::percent_mul(
            subnet_rewards,
            policy.subnet_delegate_stake_rewards_percentage,
        );

        // --- Get subnet nodes rewards total
        let subnet_node_rewards: u128 = subnet_rewards.saturating_sub(delegate_stake_rewards);

        let rewards_data = RewardsData {
            overall_subnet_reward,
            subnet_owner_reward,
            subnet_rewards,
            delegate_stake_rewards,
            subnet_node_rewards,
        };

        (rewards_data, Weight::zero())
    }
}
