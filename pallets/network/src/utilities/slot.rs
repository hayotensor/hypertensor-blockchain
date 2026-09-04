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
use frame_support::{pallet_prelude::Weight, BoundedBTreeMap, BoundedVec};

impl<T: Config> Pallet<T> {
    pub const MIN_CONSENSUS_VALIDATOR_IDENTITIES: u32 = crate::MIN_CONSENSUS_VALIDATOR_IDENTITIES;
    /// The minimum three-identity set uses an explicit two-attestor threshold.
    pub const MIN_CONSENSUS_IDENTITY_ATTESTORS: u32 = Self::MIN_CONSENSUS_VALIDATOR_IDENTITIES - 1;

    pub fn has_minimum_consensus_validator_identity_set(
        eligible_validator_identity_count: u32,
    ) -> bool {
        eligible_validator_identity_count >= Self::MIN_CONSENSUS_VALIDATOR_IDENTITIES
    }

    pub fn min_consensus_identity_attestation_count(
        eligible_validator_identity_count: u32,
        min_identity_attestation_percentage: u128,
    ) -> u32 {
        if eligible_validator_identity_count <= 1 {
            return eligible_validator_identity_count;
        }
        if eligible_validator_identity_count <= Self::MIN_CONSENSUS_VALIDATOR_IDENTITIES {
            return Self::MIN_CONSENSUS_IDENTITY_ATTESTORS;
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

    /// Converts raw Overwatch stakes into normalized Q18 reward coefficients.
    ///
    /// The common maximum-stake scale cancels during the final normalization, while keeping every
    /// fractional-power input in `[0, 1]`. Consequently, no powered value can exceed the Q18
    /// percentage factor and the intermediate sum cannot approach `u128::MAX`.
    pub(crate) fn normalize_overwatch_stake_weights(
        node_stake_weights: &mut BTreeMap<u32, u128>,
        stake_weight_factor: u128,
    ) {
        let percentage_factor = Self::percentage_factor_as_u128();
        let max_stake = node_stake_weights
            .values()
            .copied()
            .max()
            .unwrap_or_default();

        if max_stake == 0 {
            node_stake_weights
                .values_mut()
                .for_each(|weight| *weight = 0);
            return;
        }

        let exponent = Self::get_percent_as_f64(stake_weight_factor);
        let total_powered_weight: u128 = node_stake_weights
            .values_mut()
            .map(|stake| {
                let powered_weight = if *stake == 0 {
                    0
                } else if stake_weight_factor == percentage_factor {
                    // The linear exponent uses integer fixed-point arithmetic throughout.
                    Self::percent_div(*stake, max_stake)
                } else {
                    let relative_stake = (*stake as f64 / max_stake as f64).clamp(0.0, 1.0);
                    let powered_stake = Self::pow(relative_stake, exponent);

                    if powered_stake.is_finite() {
                        Self::get_f64_as_percentage(powered_stake.clamp(0.0, 1.0))
                            .min(percentage_factor)
                    } else {
                        0
                    }
                };

                *stake = powered_weight;
                powered_weight
            })
            .sum();

        if total_powered_weight == 0 {
            node_stake_weights
                .values_mut()
                .for_each(|weight| *weight = 0);
            return;
        }

        node_stake_weights.values_mut().for_each(|weight| {
            *weight = Self::percent_div(*weight, total_powered_weight);
        });
    }

    /// Derive the raw subnet signal and unnormalized node scores from complete close-time inputs.
    /// The same function is used by finalization, removal recomputation, and cache repair.
    pub(crate) fn derive_overwatch_signal(
        inputs: &LatestFinalizedOverwatchSignalInput<T>,
    ) -> Result<DerivedOverwatchSignal<T>, ()> {
        let percentage_factor = Self::percentage_factor_as_u128();
        if inputs.stake_weight_factor < MIN_OVERWATCH_STAKE_WEIGHT_FACTOR
            || inputs.stake_weight_factor > percentage_factor
            || inputs.nodes.values().any(|input| {
                input
                    .reveals
                    .values()
                    .any(|weight| *weight > percentage_factor)
            })
        {
            return Err(());
        }

        let mut node_stake_weights: BTreeMap<u32, u128> = inputs
            .nodes
            .iter()
            .map(|(node_id, input)| (*node_id, input.stake))
            .collect();
        Self::normalize_overwatch_stake_weights(
            &mut node_stake_weights,
            inputs.stake_weight_factor,
        );

        let mut subnet_reveals: BTreeMap<u32, BTreeMap<u32, u128>> = BTreeMap::new();
        for (node_id, input) in inputs.nodes.iter() {
            for (subnet_id, raw_weight) in input.reveals.iter() {
                subnet_reveals
                    .entry(*subnet_id)
                    .or_default()
                    .insert(*node_id, *raw_weight);
            }
        }

        let mut subnet_weights =
            BoundedBTreeMap::<u32, u128, T::MaxPhysicalSubnetsUpperBound>::new();
        let mut node_scores = BTreeMap::<u32, u128>::new();
        for (subnet_id, node_weights) in subnet_reveals {
            let total_adjusted = node_weights
                .iter()
                .filter_map(|(node_id, subnet_weight)| {
                    node_stake_weights
                        .get(node_id)
                        .map(|stake_weight| Self::percent_mul(*subnet_weight, *stake_weight))
                })
                .fold(0u128, |total, value| total.saturating_add(value))
                .min(percentage_factor);
            subnet_weights
                .try_insert(subnet_id, total_adjusted)
                .map_err(|_| ())?;

            for (node_id, subnet_weight) in node_weights {
                let deviation = subnet_weight.abs_diff(total_adjusted);
                let closeness_score = percentage_factor.saturating_sub(deviation);
                let node_final_score = Self::percent_mul(closeness_score, total_adjusted);
                node_scores
                    .entry(node_id)
                    .and_modify(|score| *score = score.saturating_add(node_final_score))
                    .or_insert(node_final_score);
            }
        }

        Ok(DerivedOverwatchSignal {
            subnet_weights,
            node_scores,
        })
    }

    /// Finalize the pending epoch from fixed close-time economics and the remaining participant
    /// stake and raw reveal rows after any approved removals.
    pub fn calculate_overwatch_rewards() -> Weight {
        let mut weight = Weight::zero();
        let db_weight = T::DbWeight::get();

        let Some(settlement) = PendingOverwatchSettlement::<T>::get() else {
            return db_weight.reads(1);
        };
        weight = weight.saturating_add(db_weight.reads(1));

        // A missing snapshot is an incomplete close, not an empty round. Leave every input in
        // place so finalization can be retried after repair.
        let Some(settlement_snapshot) =
            OverwatchEpochSettlementSnapshots::<T>::get(settlement.epoch)
        else {
            return weight.saturating_add(db_weight.reads(1));
        };
        weight = weight.saturating_add(db_weight.reads(1));

        let Some(revision) = LatestOverwatchSignalRevision::<T>::get().checked_add(1) else {
            return weight.saturating_add(db_weight.reads(1));
        };
        weight = weight.saturating_add(db_weight.reads(1));

        // Stage bounded reproducible inputs before changing historical or pending state.
        let mut retained_nodes = BoundedBTreeMap::<
            u32,
            LatestOverwatchNodeSignalInput<T>,
            T::MaxOverwatchNodesUpperBound,
        >::new();
        let mut reveal_row_nodes = Vec::<u32>::new();
        for (node_id, reveals) in OverwatchReveals::<T>::iter_prefix(settlement.epoch) {
            reveal_row_nodes.push(node_id);
            weight = weight.saturating_add(db_weight.reads(1));
            if reveals.is_empty() {
                continue;
            }
            let Some(snapshot) = settlement_snapshot.nodes.get(&node_id) else {
                continue;
            };
            if retained_nodes
                .try_insert(
                    node_id,
                    LatestOverwatchNodeSignalInput {
                        stake: snapshot.stake,
                        reveals,
                    },
                )
                .is_err()
            {
                return weight;
            }
        }

        let retained_inputs = LatestFinalizedOverwatchSignalInput::<T> {
            source_epoch: settlement.epoch,
            stake_weight_factor: settlement_snapshot.stake_weight_factor,
            nodes: retained_nodes,
        };
        let Ok(derived) = Self::derive_overwatch_signal(&retained_inputs) else {
            return weight;
        };

        // Historical results are immutable. Explicit zero values are persisted and remain
        // distinguishable from missing subnet keys.
        for (subnet_id, raw_weight) in derived.subnet_weights.iter() {
            OverwatchSubnetWeights::<T>::insert(settlement.epoch, subnet_id, raw_weight);
            weight = weight.saturating_add(db_weight.writes(1));
        }

        let total_final_score = derived
            .node_scores
            .values()
            .fold(0u128, |total, score| total.saturating_add(*score));
        let mut node_rewards = Vec::<(u32, u128)>::new();
        if total_final_score != 0 {
            for (node_id, score) in derived.node_scores.iter() {
                if *score == 0 {
                    continue;
                }
                let normalized_score = Self::percent_div(*score, total_final_score);
                OverwatchNodeWeights::<T>::insert(settlement.epoch, node_id, normalized_score);
                weight = weight.saturating_add(db_weight.writes(1));

                let amount = Self::percent_mul(normalized_score, settlement_snapshot.reward_budget);
                if amount != 0 {
                    Self::increase_overwatch_node_stake(*node_id, amount);
                    weight = weight.saturating_add(db_weight.reads_writes(2, 2));
                    node_rewards.push((*node_id, amount));
                }
            }
        }

        let effective_signal = EffectiveOverwatchSignal::<T> {
            source_epoch: settlement.epoch,
            valid: true,
            subnet_weights: derived.subnet_weights,
        };
        LatestFinalizedOverwatchSignalInputs::<T>::put(retained_inputs);
        LatestEffectiveOverwatchSignal::<T>::put(effective_signal);
        LatestOverwatchSignalRevision::<T>::put(revision);
        LastFinalizedOverwatchEpoch::<T>::put(settlement.epoch);
        PendingOverwatchSettlement::<T>::kill();
        OverwatchEpochSettlementSnapshots::<T>::remove(settlement.epoch);
        weight = weight.saturating_add(db_weight.writes(6));

        // Reveals remain available until every score, reward, historical output, and effective
        // cache write has succeeded. They are then consumed as ephemeral round material.
        for node_id in reveal_row_nodes {
            OverwatchReveals::<T>::remove(settlement.epoch, node_id);
            weight = weight.saturating_add(db_weight.writes(1));
        }

        Self::deposit_event(Event::EffectiveOverwatchSignalUpdated {
            source_epoch: settlement.epoch,
            revision,
            valid: true,
        });
        Self::deposit_event(Event::OverwatchEpochFinalized {
            epoch: settlement.epoch,
            node_rewards,
        });

        weight
    }

    /// - Generates emissions variables to distribute emissions: `precheck_subnet_consensus_submission`
    /// - Distributes emissions: `distribute_rewards`
    /// - Elects validator: `elect_validator`
    /// - Drains pending physical removals with the remaining subnet-slot weight
    /// - Handles registration queue (i.e., activates nodes from the queue): `handle_registration_queue`
    /// - Updates burn rate EMA: `update_burn_rate_for_epoch`
    pub fn emission_step(
        weight_meter: &mut WeightMeter,
        block: u32,
        current_epoch: u32,
        current_subnet_epoch: u32,
        subnet_id: u32,
    ) {
        Self::emission_settlement_step(
            weight_meter,
            block,
            current_epoch,
            current_subnet_epoch,
            subnet_id,
        );
        Self::emission_operational_step(weight_meter, block, current_subnet_epoch, subnet_id);
    }

    /// Settle the prior subnet epoch under a reward-core envelope that is independent from
    /// election, physical cleanup, registration, and burn maintenance.
    pub fn emission_settlement_step(
        weight_meter: &mut WeightMeter,
        _block: u32,
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
                                current_subnet_epoch, // used for graduating nodes
                                consensus_submission_data,
                                rewards_data,
                            );
                        }

                        // Pending removal logic.
                    }
                }
            }
        }
    }

    /// Run independently admitted subnet operations after reward settlement. Election sees the
    /// quarantine markers first; physical cleanup then spends only genuinely remaining weight.
    pub fn emission_operational_step(
        weight_meter: &mut WeightMeter,
        block: u32,
        current_subnet_epoch: u32,
        subnet_id: u32,
    ) {
        let db_weight = T::DbWeight::get();
        // Operational subnet-epoch work must not depend on a reward allocation. Queue and
        // burn maintenance run while Active so the preparation epoch is useful, whereas a
        // validator election additionally requires the consensus eligibility epoch to be reached.
        let subnet_read_weight = db_weight.reads(1);
        if !weight_meter.can_consume(subnet_read_weight) {
            return;
        }
        weight_meter.consume(subnet_read_weight);
        if let Ok(subnet) = SubnetsData::<T>::try_get(subnet_id) {
            if subnet.state == SubnetState::Active
                && Self::_is_subnet_active_and_live(&subnet, current_subnet_epoch)
            {
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

            // Drain quarantined physical state only after the conditional election. This preserves
            // reward/election priority while retrying every marker for any still-existing subnet.
            Self::cleanup_pending_node_removals(weight_meter, subnet_id);

            if subnet.state == SubnetState::Active {
                // Registration and burn maintenance are the final slot priority. Select one
                // complete generated envelope from the physical registered-node count. Pending
                // registered nodes remain physical but are absent from the queue, so this is a
                // conservative queue bound.
                let maintenance_selector_weight = db_weight.reads(2);
                if !weight_meter.can_consume(maintenance_selector_weight) {
                    return;
                }
                weight_meter.consume(maintenance_selector_weight);

                let registered_nodes = TotalSubnetNodes::<T>::get(subnet_id)
                    .saturating_sub(TotalActiveSubnetNodes::<T>::get(subnet_id))
                    .clamp(1, T::MaxRegisteredNodesUpperBound::get());
                let maintenance_weight = T::WeightInfo::emission_step_queue(registered_nodes)
                    // Queue activation now checks the bounded pending set once per candidate.
                    // Compose that new scan until the queue benchmark is regenerated.
                    .saturating_add(
                        T::WeightInfo::pending_registered_removal_scan(
                            T::MaxRegisteredNodesUpperBound::get(),
                        )
                        .saturating_mul(registered_nodes.into()),
                    );

                if weight_meter.can_consume(maintenance_weight) {
                    // Charge the admitted envelope up front. The local meter keeps both helpers
                    // inside that same budget without allowing maintenance to borrow from an
                    // earlier priority.
                    weight_meter.consume(maintenance_weight);
                    let mut maintenance_meter = WeightMeter::with_limit(maintenance_weight);
                    Self::handle_registration_queue(
                        &mut maintenance_meter,
                        subnet_id,
                        current_subnet_epoch,
                    );
                    Self::update_burn_rate_for_epoch(&mut maintenance_meter, subnet_id);
                }
            }
        }
    }

    /// Attempt deterministic physical cleanup of every persisted quarantine marker. Active nodes
    /// are considered first, then registered nodes. A marker remains durable whenever its complete
    /// selector/removal/write path does not fit, so the next assigned subnet slot can retry it.
    pub fn cleanup_pending_node_removals(weight_meter: &mut WeightMeter, subnet_id: u32) {
        Self::cleanup_pending_active_node_removals(weight_meter, subnet_id);
        Self::cleanup_pending_registered_node_removals(weight_meter, subnet_id);
    }

    fn cleanup_pending_active_node_removals(weight_meter: &mut WeightMeter, subnet_id: u32) {
        let db_weight = T::DbWeight::get();
        // The cardinality is encoded inside the set itself, so admit the generated maximum scan
        // before decoding it. Per-node physical branches below remain independently metered.
        let pending_read_weight =
            T::WeightInfo::pending_active_removal_scan(T::MaxSubnetNodesUpperBound::get());
        if !weight_meter.can_consume(pending_read_weight) {
            return;
        }
        weight_meter.consume(pending_read_weight);

        let pending = PendingActiveNodeRemovals::<T>::get(subnet_id);
        if pending.is_empty() {
            return;
        }

        // Select the generated active-removal model from the compact physical election counter.
        let election_count_read_weight = db_weight.reads(1);
        if !weight_meter.can_consume(election_count_read_weight) {
            return;
        }
        weight_meter.consume(election_count_read_weight);
        let electable_nodes_count = TotalSubnetElectableNodes::<T>::get(subnet_id);

        let validator_selector_weight = T::WeightInfo::subnet_node_validator_id_selector();
        let ownership_selector_weight = db_weight.reads(1);
        // Physical node removal clears both quarantine maps. Until those writes are folded into
        // regenerated removal models, reserve them explicitly for every deletion attempt.
        let marker_clear_weight = Self::pending_node_removal_marker_clear_weight();
        for subnet_node_id in pending.iter().copied() {
            if !weight_meter.can_consume(validator_selector_weight) {
                break;
            }
            weight_meter.consume(validator_selector_weight);
            let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id);

            let validator_nodes = if let Some(validator_id) = validator_id {
                if !weight_meter.can_consume(ownership_selector_weight) {
                    break;
                }
                weight_meter.consume(ownership_selector_weight);
                Self::validator_owned_nodes_weight_param(validator_id)
            } else {
                // Missing reverse ownership is not proof that the physical node is absent. Select
                // the maximum removal model and let the typed remover prove/delete physical state.
                T::MaxValidatorNodesUpperBound::get()
            };
            let removal_weight =
                Self::active_subnet_node_removal_weight(validator_nodes, electable_nodes_count)
                    .saturating_add(marker_clear_weight);
            if !weight_meter.can_consume(removal_weight) {
                // Branch cost varies with validator ownership. Keep the deterministic scan going
                // so one expensive low-ID entry cannot block a later affordable entry forever.
                continue;
            }

            weight_meter.consume(removal_weight);
            Self::remove_active_subnet_node(subnet_id, subnet_node_id);
        }
    }

    fn cleanup_pending_registered_node_removals(weight_meter: &mut WeightMeter, subnet_id: u32) {
        let db_weight = T::DbWeight::get();
        let pending_read_weight =
            T::WeightInfo::pending_registered_removal_scan(T::MaxRegisteredNodesUpperBound::get());
        if !weight_meter.can_consume(pending_read_weight) {
            return;
        }
        weight_meter.consume(pending_read_weight);

        let pending = PendingRegisteredNodeRemovals::<T>::get(subnet_id);
        if pending.is_empty() {
            return;
        }

        // The queue no longer contains quarantined registered nodes. Use compact physical counts
        // as a conservative upper bound for the generated queue/removal component.
        let registered_count_read_weight = db_weight.reads(2);
        if !weight_meter.can_consume(registered_count_read_weight) {
            return;
        }
        weight_meter.consume(registered_count_read_weight);
        let registered_nodes_count = TotalSubnetNodes::<T>::get(subnet_id)
            .saturating_sub(TotalActiveSubnetNodes::<T>::get(subnet_id))
            .clamp(1, T::MaxRegisteredNodesUpperBound::get());

        let validator_selector_weight = T::WeightInfo::subnet_node_validator_id_selector();
        let ownership_selector_weight = db_weight.reads(1);
        let marker_clear_weight = Self::pending_node_removal_marker_clear_weight();
        for subnet_node_id in pending.iter().copied() {
            if !weight_meter.can_consume(validator_selector_weight) {
                break;
            }
            weight_meter.consume(validator_selector_weight);
            let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id);

            let validator_nodes = if let Some(validator_id) = validator_id {
                if !weight_meter.can_consume(ownership_selector_weight) {
                    break;
                }
                weight_meter.consume(ownership_selector_weight);
                Self::validator_owned_nodes_weight_param(validator_id)
            } else {
                T::MaxValidatorNodesUpperBound::get()
            };
            let removal_weight = T::WeightInfo::remove_registered_subnet_node(
                validator_nodes,
                registered_nodes_count,
            )
            .saturating_add(marker_clear_weight);
            if !weight_meter.can_consume(removal_weight) {
                continue;
            }

            weight_meter.consume(removal_weight);
            Self::remove_registered_subnet_node(subnet_id, subnet_node_id);
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

            // The generated `emission_step_queue(q)` model covers bounded CPU/vector work. This
            // internal meter only needs to reserve the storage work that the helper consumes and
            // the one final queue write.
            let storage_write_weight = if activated_nodes == 0 {
                db_weight.writes(1) // Only count the storage write once
            } else {
                Weight::zero()
            };

            let pending_lookup_weight = db_weight
                .reads(1)
                .saturating_add(Self::pending_subnet_node_removal_proof_weight());
            let total_weight_needed = storage_write_weight
                .saturating_add(pending_lookup_weight)
                .saturating_add(db_weight.reads_writes(4, 4)); // Account for do_activate_subnet_node weight consumption

            // Check if we can consume the complete operation (activation + cleanup + db updates)
            if !weight_meter.can_consume(total_weight_needed) {
                break;
            }

            // Attempt activation
            let can_consume = Self::do_activate_subnet_node(
                weight_meter,
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

        // Allocation consumes only the latest effective cache. Historical finalized weights are
        // immutable public records and are never an economic fallback after a removal.
        let effective_overwatch_signal = LatestEffectiveOverwatchSignal::<T>::get();
        let overwatch_weight_factor = OverwatchWeightFactor::<T>::get();
        let default_overwatch_weight = DefaultOverwatchSubnetWeight::<T>::get();
        weight = weight.saturating_add(db_weight.reads(3));

        // Only subnet IDs are needed below. Avoid decoding and proving every variable-size
        // `SubnetData` value on this hook-critical path.
        let subnet_ids: Vec<u32> = SubnetsData::<T>::iter_keys().collect();

        // At the global subnet-emission slot, every assigned subnet is still in the local epoch
        // whose election completed at its slot in the preceding general epoch. Derive that phase
        // from the allocation epoch itself so this calculation is deterministic even when invoked
        // by a benchmark or test outside the hook.
        let allocation_block = epoch
            .saturating_mul(T::EpochLength::get())
            .saturating_add(NETWORK_SUBNET_EMISSION_SLOT);
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
            // SubnetElectedValidator. The immutable round already excludes quarantined candidates,
            // so its eligible-node cardinality is the reward-allocation authority. The live
            // physical counter intentionally remains reserved for election/removal weight selection.
            eligibility_reads = eligibility_reads.saturating_add(1);
            if let Some(round) = SubnetElectedValidator::<T>::get(subnet_id, current_subnet_epoch) {
                let delegate_stake = TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);
                let electable_nodes = round.eligible_subnet_node_ids.len() as u32;
                eligibility_reads = eligibility_reads.saturating_add(1);
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
            let raw_overwatch_weight = effective_overwatch_signal
                .as_ref()
                .filter(|signal| signal.valid)
                .and_then(|signal| signal.subnet_weights.get(&subnet_id).copied());
            let overwatch_subnet_weight = match raw_overwatch_weight {
                Some(raw_weight) => Self::get_percent_as_f64(
                    Self::percent_mul(raw_weight, overwatch_weight_factor)
                        .min(Self::percentage_factor_as_u128()),
                ),
                None => Self::get_percent_as_f64(default_overwatch_weight),
            };

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
        _current_epoch: u32,
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

                    // Apply economic losses while leaving reputation loss to the existing
                    // absence-specific node and subnet penalties below.
                    let (_, _, _, slash_weight) = Self::apply_validator_economic_slashes(
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
                    let mut newly_pending_active_removals = BoundedVec::default();
                    let reputation_factors = round.policy.reputation_factors;
                    if let Some(rep) =
                        SubnetNodeReputation::<T>::get(subnet_id, validator_subnet_node_id)
                    {
                        let new_reputation = Self::decrease_and_return_node_reputation(
                            subnet_id,
                            validator_subnet_node_id,
                            rep,
                            reputation_factors.validator_absent_decrease,
                            None,
                        );

                        let min_node_reputation = round
                            .emergency
                            .as_ref()
                            .map(|snapshot| snapshot.min_subnet_node_reputation)
                            .unwrap_or(round.policy.min_subnet_node_reputation);
                        if new_reputation < min_node_reputation {
                            let (pending_weight, inserted) =
                                Self::persist_pending_active_node_removal(
                                    subnet_id,
                                    validator_subnet_node_id,
                                );
                            weight = weight.saturating_add(pending_weight);
                            if inserted {
                                newly_pending_active_removals
                                    .try_push(validator_subnet_node_id)
                                    .expect("one missing proposer fits the active-removal event");
                            }
                        }
                    }

                    weight = weight.saturating_add(db_weight.reads_writes(1, 1));
                    if !newly_pending_active_removals.is_empty() {
                        Self::deposit_event(Event::SubnetNodesPendingRemoval {
                            subnet_id,
                            active_subnet_node_ids: newly_pending_active_removals,
                            registered_subnet_node_ids: BoundedVec::default(),
                        });
                    }
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
