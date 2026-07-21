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
use frame_support::pallet_prelude::Weight;
use frame_system::pallet_prelude::BlockNumberFor;

impl<T: Config> Pallet<T> {
    pub fn consensus_policy_snapshot(subnet_id: u32, subnet_epoch: u32) -> ConsensusPolicySnapshot {
        ConsensusPolicySnapshot {
            min_attestation_percentage: MinAttestationPercentage::<T>::get(),
            super_majority_attestation_ratio: SuperMajorityAttestationRatio::<T>::get(),
            base_validator_reward: BaseValidatorReward::<T>::get(),
            subnet_owner_percentage: SubnetOwnerPercentage::<T>::get(),
            validator_reward_k: ValidatorRewardK::<T>::get(),
            validator_reward_midpoint: ValidatorRewardMidpoint::<T>::get(),
            attestor_reward_exponent: AttestorRewardExponent::<T>::get(),
            attestor_min_reward_factor: AttestorMinRewardFactor::<T>::get(),
            base_slash_percentage: BaseSlashPercentage::<T>::get(),
            max_slash_amount: MaxSlashAmount::<T>::get(),
            validator_delegate_stake_slash_threshold:
                ValidatorDelegateStakeSlashThreshold::<T>::get(),
            base_validator_delegate_stake_slash_percentage:
                BaseValidatorDelegateStakeSlashPercentage::<T>::get(),
            max_validator_delegate_stake_slash_amount:
                MaxValidatorDelegateStakeSlashAmount::<T>::get(),
            validator_reputation_increase_factor: ValidatorReputationIncreaseFactor::<T>::get(),
            validator_reputation_decrease_factor: ValidatorReputationDecreaseFactor::<T>::get(),
            validator_absent_subnet_reputation_factor:
                ValidatorAbsentSubnetReputationFactor::<T>::get(),
            in_consensus_subnet_reputation_factor: InConsensusSubnetReputationFactor::<T>::get(),
            not_in_consensus_subnet_reputation_factor:
                NotInConsensusSubnetReputationFactor::<T>::get(),
            min_subnet_nodes: MinSubnetNodes::<T>::get(),
            validator_identity_attestation_percentage:
                ConsensusValidatorIdentityAttestationPercentage::<T>::get(),
            min_subnet_node_reputation: Self::get_min_subnet_node_reputation_for_epoch(
                subnet_id,
                subnet_epoch,
            ),
            min_weight_decrease_reputation_threshold:
                Self::get_subnet_node_min_weight_decrease_reputation_threshold_for_epoch(
                    subnet_id,
                    subnet_epoch,
                ),
            subnet_delegate_stake_rewards_percentage:
                Self::get_subnet_delegate_stake_rewards_percentage_for_epoch(
                    subnet_id,
                    subnet_epoch,
                ),
            consensus_validator_node_count_decay:
                Self::get_consensus_validator_node_count_decay_for_epoch(subnet_id, subnet_epoch),
            consensus_validator_stake_weight_power:
                Self::get_consensus_validator_stake_weight_power_for_epoch(subnet_id, subnet_epoch),
            idle_classification_epochs: Self::get_idle_classification_epochs_for_epoch(
                subnet_id,
                subnet_epoch,
            ),
            included_classification_epochs: Self::get_included_classification_epochs_for_epoch(
                subnet_id,
                subnet_epoch,
            ),
            queue_immunity_epochs: Self::get_queue_immunity_epochs_for_epoch(
                subnet_id,
                subnet_epoch,
            ),
            reputation_factors: Self::get_reputation_factors_for_epoch(subnet_id, subnet_epoch),
        }
    }

    pub fn get_current_block_as_u64() -> u64 {
        TryInto::try_into(<frame_system::Pallet<T>>::block_number())
            .ok()
            .expect("blockchain will not exceed 2^64 blocks; QED.")
    }

    pub fn convert_block_as_u64(block: BlockNumberFor<T>) -> u64 {
        TryInto::try_into(block)
            .ok()
            .expect("blockchain will not exceed 2^64 blocks; QED.")
    }

    pub fn get_current_block_as_u32() -> u32 {
        TryInto::try_into(<frame_system::Pallet<T>>::block_number())
            .ok()
            .expect("blockchain will not exceed 2^32 blocks; QED.")
    }

    pub fn convert_block_as_u32(block: BlockNumberFor<T>) -> u32 {
        TryInto::try_into(block)
            .ok()
            .expect("blockchain will not exceed 2^32 blocks; QED.")
    }

    pub fn get_current_epoch_as_u32() -> u32 {
        let current_block = Self::get_current_block_as_u32();
        let epoch_length: u32 = T::EpochLength::get();
        current_block.saturating_div(epoch_length)
    }

    pub fn get_current_overwatch_epoch_as_u32() -> u32 {
        CurrentOverwatchEpoch::<T>::get()
    }

    pub fn in_overwatch_commit_period() -> bool {
        let current_block = Self::get_current_block_as_u32();
        let epoch_start_block = OverwatchEpochStartBlock::<T>::get();
        let overwatch_epoch_length = T::EpochLength::get()
            .checked_mul(ActiveOverwatchEpochLengthMultiplier::<T>::get())
            .unwrap_or(u32::MAX);
        let cutoff_percentage = ActiveOverwatchCommitCutoffPercent::<T>::get();
        let block_increase_cutoff =
            Self::percent_mul(overwatch_epoch_length as u128, cutoff_percentage);
        let epoch_cutoff_block = epoch_start_block.saturating_add(block_increase_cutoff as u32);
        current_block < epoch_cutoff_block
    }

    /// Close the active Overwatch epoch once its snapshotted interval has elapsed.
    ///
    /// The closed epoch is settled on the following block. Configuration queued during this epoch
    /// becomes active only after the old epoch has been closed, keeping epoch IDs and phases stable
    /// for all commits and reveals already submitted.
    pub fn advance_overwatch_epoch(current_block: u32) -> Weight {
        let db_weight = T::DbWeight::get();
        let mut weight = Weight::zero();

        let general_epoch_length = T::EpochLength::get();
        let epoch_start_block = OverwatchEpochStartBlock::<T>::get();
        let multiplier = ActiveOverwatchEpochLengthMultiplier::<T>::get();
        weight = weight.saturating_add(db_weight.reads(2));

        let Some(overwatch_epoch_length) = general_epoch_length.checked_mul(multiplier) else {
            // The setter rejects this configuration. Keeping the current epoch open is safer than
            // silently changing its duration if storage is ever corrupted.
            return weight;
        };
        let epoch_end_block = epoch_start_block.saturating_add(overwatch_epoch_length);
        if current_block < epoch_end_block {
            return weight;
        }

        // Overwatch boundaries share the general epoch's reserved hook schedule: rollover is slot
        // zero, settlement is slot one, and global emission calculation is slot two. If global
        // transaction pause delayed this hook past its original end, leave the old round open
        // through the next slot-zero boundary. This gives revealers a grace period after unpause
        // and prevents a permanently unaligned Overwatch cadence from pre-empting subnet slots.
        if general_epoch_length == 0 || current_block % general_epoch_length != 0 {
            return weight;
        }

        // A settlement is consumed on the block after rollover. Never overwrite it, even if hook
        // processing was delayed, because doing so could skip or double-pay an epoch.
        if PendingOverwatchSettlement::<T>::exists() {
            return weight.saturating_add(db_weight.reads(1));
        }
        weight = weight.saturating_add(db_weight.reads(1));

        let completed_epoch = CurrentOverwatchEpoch::<T>::get();
        weight = weight.saturating_add(db_weight.reads(1));

        PendingOverwatchSettlement::<T>::put(PendingOverwatchSettlementData {
            epoch: completed_epoch,
            epoch_length_multiplier: multiplier,
        });
        CurrentOverwatchEpoch::<T>::put(completed_epoch.saturating_add(1));

        // Snapshot the latest configuration for the epoch that begins now. From this point until
        // the next rollover, phase boundaries and the settlement budget use only these values.
        let next_multiplier = OverwatchEpochLengthMultiplier::<T>::get();
        let next_cutoff = OverwatchCommitCutoffPercent::<T>::get();
        ActiveOverwatchEpochLengthMultiplier::<T>::put(next_multiplier);
        ActiveOverwatchCommitCutoffPercent::<T>::put(next_cutoff);

        // Under normal block-by-block execution this equals `epoch_end_block`. If hook processing
        // was paused, the alignment check above delays rollover to the next general boundary so
        // the new epoch still receives its full configured interval.
        OverwatchEpochStartBlock::<T>::put(current_block);
        weight = weight.saturating_add(db_weight.reads(2));
        weight = weight.saturating_add(db_weight.writes(5));

        Self::deposit_event(Event::OverwatchEpochStarted {
            epoch: completed_epoch.saturating_add(1),
            start_block: current_block,
            epoch_length_multiplier: next_multiplier,
            commit_cutoff_percent: next_cutoff,
        });

        weight
    }

    /// Return epoch, overwatch epoch
    pub fn get_current_epochs_as_u32() -> (u32, u32) {
        let current_block = Self::get_current_block_as_u32();
        let epoch_length: u32 = T::EpochLength::get();
        let epoch = current_block.saturating_div(epoch_length);
        (epoch, CurrentOverwatchEpoch::<T>::get())
    }

    pub fn get_current_epoch_with_block_as_u32(current_block: u32) -> u32 {
        let epoch_length: u32 = T::EpochLength::get();
        current_block.saturating_div(epoch_length)
    }

    pub fn get_current_subnet_epoch_as_u32(subnet_id: u32) -> u32 {
        Self::get_subnet_epoch_with_block_as_u32(subnet_id, Self::get_current_block_as_u32())
    }

    /// Return the phase-aware epoch for `subnet_id` at an explicit block.
    ///
    /// A subnet epoch advances at that subnet's assigned slot, not at the general epoch's
    /// slot-zero boundary. Lifecycle consumers that run in general-epoch hooks must therefore
    /// use this conversion rather than comparing a subnet marker with the general epoch label.
    pub fn get_subnet_epoch_with_block_as_u32(subnet_id: u32, current_block: u32) -> u32 {
        let epoch_length = T::EpochLength::get();
        let subnet_slot = match SubnetSlot::<T>::try_get(subnet_id) {
            Ok(slot) => slot,
            Err(_) => 0,
        };
        if subnet_slot == 0 || epoch_length == 0 {
            return 0;
        }

        if current_block < subnet_slot {
            return 0;
        }

        // Example: 150 = 200-50
        let offset_block = current_block.saturating_sub(subnet_slot);

        // Example: 1 = 150 / 100
        offset_block.saturating_div(epoch_length)
    }

    pub fn get_subnet_epoch_progression(subnet_id: u32) -> u128 {
        let epoch_length = T::EpochLength::get();
        let subnet_slot = match SubnetSlot::<T>::try_get(subnet_id) {
            Ok(slot) => slot,
            Err(_) => 0,
        };
        if subnet_slot == 0 {
            return 0;
        }

        let current_block = Self::get_current_block_as_u32();

        if current_block < subnet_slot {
            return 0;
        }

        let offset_block = current_block.saturating_sub(subnet_slot);
        let blocks_into_epoch = offset_block % epoch_length;
        Self::percent_div(blocks_into_epoch as u128, epoch_length as u128)
    }

    /// Returns true if < last subnet epoch block
    pub fn can_propose_or_attest_attestation(subnet_id: u32) -> bool {
        let epoch_length = T::EpochLength::get();
        let subnet_slot = match SubnetSlot::<T>::try_get(subnet_id) {
            Ok(slot) => slot,
            Err(_) => 0,
        };
        if subnet_slot == 0 {
            return false;
        }

        let current_block = Self::get_current_block_as_u32();

        if current_block < subnet_slot {
            return false;
        }

        let offset_block = current_block.saturating_sub(subnet_slot);

        let current_subnet_epoch = offset_block.saturating_div(epoch_length);

        // Get last subnet epoch block, start of next epoch
        let last_epoch_block = subnet_slot.saturating_add(
            current_subnet_epoch
                .saturating_add(1)
                .saturating_mul(epoch_length),
        );

        // Check if we are at the last block
        current_block < last_epoch_block
    }

    pub fn attestor_subnet_epoch_data(
        subnet_id: u32,
        block_proposed: u32,
    ) -> Option<SubnetEpochData> {
        let epoch_length = T::EpochLength::get();
        let subnet_slot = match SubnetSlot::<T>::try_get(subnet_id) {
            Ok(slot) => slot,
            Err(_) => 0,
        };
        if subnet_slot == 0 {
            return None;
        }

        let current_block = Self::get_current_block_as_u32();

        if current_block < block_proposed {
            return None; // can't attest before proposal
        }

        // The validator's epoch offset at submission
        let proposed_offset = block_proposed.saturating_sub(subnet_slot);
        let subnet_epoch = proposed_offset.saturating_div(epoch_length);

        // Blocks from submission to current block
        let blocks_since_submission = current_block.saturating_sub(block_proposed);

        // Remaining blocks in this epoch
        let blocks_into_epoch = proposed_offset % epoch_length;
        let remaining_blocks_in_epoch = epoch_length.saturating_sub(blocks_into_epoch);

        // How far from submission to epoch end (percentage)
        // If current block > epoch end, clamp to 100%
        let progress_from_submission = if blocks_since_submission >= remaining_blocks_in_epoch {
            Self::percentage_factor_as_u128()
        } else {
            Self::percent_div(
                blocks_since_submission as u128,
                remaining_blocks_in_epoch as u128,
            )
        };

        Some(SubnetEpochData {
            subnet_epoch,
            subnet_epoch_progression: progress_from_submission,
        })
    }

    /// Returns the current subnet epoch and the subnet epoch progression.
    ///
    /// This function is used to determine the current subnet epoch and the subnet epoch progression.
    pub fn get_current_subnet_epoch_data(subnet_id: u32) -> Option<SubnetEpochData> {
        let epoch_length = T::EpochLength::get();
        let subnet_slot = match SubnetSlot::<T>::try_get(subnet_id) {
            Ok(slot) => slot,
            Err(_) => 0,
        };
        if subnet_slot == 0 {
            return None;
        }

        let current_block = Self::get_current_block_as_u32();

        if current_block < subnet_slot {
            return None;
        }

        // Example: 150 = 200-50
        let offset_block = current_block.saturating_sub(subnet_slot);

        // Example: 1 = 150 / 100
        let subnet_epoch = offset_block.saturating_div(epoch_length);

        let blocks_into_epoch = offset_block % epoch_length;
        let subnet_epoch_progression =
            Self::percent_div(blocks_into_epoch as u128, epoch_length as u128);

        Some(SubnetEpochData {
            subnet_epoch,
            subnet_epoch_progression,
        })
    }

    /// Performs preliminary subnet checks and maintenance at the start of each epoch.
    ///
    /// This function iterates over all registered subnets and enforces several rules:
    ///
    /// - Subnets in the **registration period** are allowed to exist without reputation decrease.
    /// - Subnets in the **enactment period** must meet minimum active node counts or get removed.
    /// - Subnets **out of enactment period** but not activated are removed.
    /// - Subnets in the **paused state** are penalized if they exceed allowed pause duration, potentially leading to removal.
    /// - Activated subnets are checked to ensure they meet minimum delegate stake requirements; otherwise they are removed.
    /// - Activated subnets with insufficient active nodes decrease reputation.  
    /// - Subnets exceeding the minimum reputation are removed.
    /// - If the total number of subnets exceeds the configured maximum, the subnet with the lowest delegate stake is removed.
    ///
    /// Reputations are global and can be increased or decreased by other runtime logic as well, so this function enforces removal
    /// conditions based on the current reputation regardless of its origin.
    ///
    /// # Arguments
    ///
    /// * `block` - The current block number, used to resolve each subnet's phase-aware epoch.
    /// * `epoch` - The current epoch number.
    ///
    /// # Returns
    ///
    /// The accumulated weight consumed by database reads and writes during the operation.
    ///
    /// # Notes
    ///
    /// - The function uses storage reads and writes extensively; weights are accumulated accordingly.
    /// - Subnet removal triggers are delegated to `try_do_remove_subnet`.
    ///
    pub fn do_epoch_preliminaries(weight_meter: &mut WeightMeter, block: u32, epoch: u32) {
        let db_weight = T::DbWeight::get();

        // Min reputation a subnet can have
        let min_reputation = MinSubnetReputation::<T>::get();
        // Total epochs of the registration phase
        let subnet_registration_epochs = SubnetRegistrationEpochs::<T>::get();
        // Total epochs of the enactment phase
        let subnet_enactment_epochs = SubnetEnactmentEpochs::<T>::get();
        // Min nodes a subnet can have, if under reputation is decreased
        let min_subnet_nodes = MinSubnetNodes::<T>::get();
        // Max subnets allowed in the network
        let max_subnets = MaxSubnets::<T>::get();
        // Max epochs a subnet can be paused
        let max_pause_epochs = MaxSubnetPauseEpochs::<T>::get();
        // Epoch interval for delegate stake removal
        let dstake_epoch_interval = DelegateStakeSubnetRemovalInterval::<T>::get();
        // Epoch of the previous subnet activation which will push back the removal interval
        let prev_activation_epoch = PrevSubnetActivationEpoch::<T>::get();
        // Whether the current epoch is a removal epoch
        let is_removal_epoch: bool = epoch % MaxSubnetRemovalInterval::<T>::get() == 0;
        // Whether the current epoch is a removal epoch for excess subnets
        let can_remove: bool =
            epoch >= prev_activation_epoch + MinSubnetRemovalInterval::<T>::get();
        // Whether the current epoch is a removal epoch for delegate stake
        let dstake_epoch_interval_can_remove: bool = epoch % dstake_epoch_interval == 0;

        let subnets: Vec<_> = SubnetsData::<T>::iter().collect();
        let total_subnets: u32 = subnets.len() as u32;

        weight_meter.consume(db_weight.reads((10 + total_subnets).into()));

        let excess_subnets: bool = total_subnets > max_subnets;
        let mut subnet_delegate_stake: Vec<(u32, u128)> = Vec::new();

        if excess_subnets {
            subnet_delegate_stake.reserve(total_subnets as usize);
            // --- Get expected weight for `subnet_delegate_stake`
            weight_meter.consume(
                db_weight.reads(total_subnets as u64)
                    + Weight::from_parts(5_000 * total_subnets as u64, 0),
            );
        }

        // Main loop computational overhead
        weight_meter.consume(Weight::from_parts(1_000 * total_subnets as u64, 0));

        for (subnet_id, data) in &subnets {
            // --- Registration logic
            if data.state == SubnetState::Registered {
                // SubnetRegistrationEpoch
                weight_meter.consume(db_weight.reads(1));
                if let Ok(registered_epoch) = SubnetRegistrationEpoch::<T>::try_get(subnet_id) {
                    // --- Do the registration and enactment period math manually instead of using helper functions to avoid duplicate lookups
                    let max_registration_epoch =
                        registered_epoch.saturating_add(subnet_registration_epochs);
                    let max_enactment_epoch =
                        max_registration_epoch.saturating_add(subnet_enactment_epochs);

                    if epoch <= max_registration_epoch {
                        // --- Registration Period: do nothing
                        // We wait for the owner to activate the subnet to ensure the subnet is ready to begin
                        continue;
                    }

                    if epoch <= max_enactment_epoch {
                        // --- Enactment Period
                        // - Check min nodes
                        // We don't check delegate stake here because users can continue to stake in this phase
                        let active_nodes = TotalActiveSubnetNodes::<T>::get(subnet_id);
                        weight_meter.consume(db_weight.reads(1));

                        if active_nodes < min_subnet_nodes {
                            Self::try_do_remove_subnet(
                                weight_meter,
                                *subnet_id,
                                SubnetRemovalReason::MinSubnetNodes,
                            );
                        }
                        continue;
                    }

                    // --- Out of Enactment Period: not activated → remove
                    Self::try_do_remove_subnet(
                        weight_meter,
                        *subnet_id,
                        SubnetRemovalReason::EnactmentPeriod,
                    );
                    continue;
                }
                continue;
            }

            // --- Pause logic
            if data.state == SubnetState::Paused {
                // Maximum-pause enforcement is intentionally global: it runs at general slot
                // zero and is based on the global epoch in which this pause episode began.
                if let Some(pause) = data.pause {
                    if pause.started_global_epoch.saturating_add(max_pause_epochs) < epoch {
                        let new_subnet_reputation = Self::decrease_rep(
                            SubnetReputation::<T>::get(subnet_id),
                            MaxPauseEpochsSubnetReputationFactor::<T>::get(),
                            None,
                        );
                        SubnetReputation::<T>::insert(subnet_id, new_subnet_reputation);
                        weight_meter.consume(db_weight.reads_writes(2, 1));

                        if new_subnet_reputation < min_reputation {
                            // --- Remove
                            Self::try_do_remove_subnet(
                                weight_meter,
                                *subnet_id,
                                SubnetRemovalReason::PauseExpired,
                            );
                            continue;
                        }
                    }
                }

                // Pausing exempts a subnet from operational health checks, but must not shield a
                // low-stake subnet from the network-wide excess-capacity eviction cohort.
                if excess_subnets && is_removal_epoch && can_remove {
                    subnet_delegate_stake.push((
                        *subnet_id,
                        TotalSubnetDelegateStakeBalance::<T>::get(subnet_id),
                    ));
                    weight_meter.consume(db_weight.reads(1));
                }
                continue;
            }

            // At general slot zero the subnet may still be in the preceding local epoch. Skip
            // ordinary health checks until its first post-activation/unpause consensus slot has
            // occurred; checks resume at the following general boundary.
            let current_subnet_epoch = Self::get_subnet_epoch_with_block_as_u32(*subnet_id, block);
            weight_meter.consume(db_weight.reads(1));
            if !Self::_is_subnet_active_and_live(data, current_subnet_epoch) {
                continue;
            }

            // --- Activated subnet checks and conditionals
            let min_subnet_delegate_stake_balance =
                Self::get_min_subnet_delegate_stake_balance(*subnet_id);
            weight_meter.consume(T::WeightInfo::get_min_subnet_delegate_stake_balance());

            let subnet_delegate_stake_balance =
                TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);
            weight_meter.consume(db_weight.reads(1));

            // Remove if below delegate stake requirement
            if subnet_delegate_stake_balance < min_subnet_delegate_stake_balance
                && dstake_epoch_interval_can_remove
            {
                Self::try_do_remove_subnet(
                    weight_meter,
                    *subnet_id,
                    SubnetRemovalReason::MinSubnetDelegateStake,
                );
                continue;
            }

            // Check min nodes (we don't kick active subnet for this to give them time to recoup)
            // We decrease reputation only
            // A subnet can have n-1 min electable nodes, we'll allow them to get more nodes until
            // they read the min nodes count
            let electable_nodes = TotalSubnetElectableNodes::<T>::get(subnet_id);

            if electable_nodes < min_subnet_nodes {
                let new_subnet_reputation = Self::decrease_rep(
                    SubnetReputation::<T>::get(subnet_id),
                    LessThanMinNodesSubnetReputationFactor::<T>::get(),
                    None,
                );
                SubnetReputation::<T>::insert(subnet_id, new_subnet_reputation);
                weight_meter.consume(db_weight.reads_writes(2, 1));
            }

            let subnet_reputation = SubnetReputation::<T>::get(subnet_id);
            // TotalSubnetElectableNodes | SubnetReputation
            weight_meter.consume(db_weight.reads(2));

            if subnet_reputation < min_reputation {
                // --- Remove
                Self::try_do_remove_subnet(
                    weight_meter,
                    *subnet_id,
                    SubnetRemovalReason::MinReputation,
                );
                continue;
            }

            // Store delegate stake for possible excess removal
            if excess_subnets && is_removal_epoch && can_remove {
                subnet_delegate_stake.push((*subnet_id, subnet_delegate_stake_balance));
            }
        }

        // --- Excess subnet removal
        // We allow max+1 subnets to exist in the economy and every `x` epochs remove one
        // based on the delegate stake balance
        if excess_subnets && !subnet_delegate_stake.is_empty() && is_removal_epoch && can_remove {
            subnet_delegate_stake.sort_by_key(|&(_, value)| value);

            // Account for sorting cost (O(n log n))
            let sort_items = subnet_delegate_stake.len() as u64;
            let sort_weight = Weight::from_parts(
                sort_items * sort_items.ilog2() as u64 * 100, // Approximate O(n log n)
                0,
            );
            weight_meter.consume(sort_weight);

            let subnet_id = subnet_delegate_stake[0].0.clone();
            Self::try_do_remove_subnet(weight_meter, subnet_id, SubnetRemovalReason::MaxSubnets);
        }
    }

    pub fn elect_validator(subnet_id: u32, subnet_epoch: u32, block: u32) {
        // Redundant
        // If validator already chosen, then return
        if SubnetElectedValidator::<T>::contains_key(subnet_id, subnet_epoch) {
            return;
        }

        // Check for emergency validators
        let slot_list = if let Some(emergency_validator_data) =
            EmergencySubnetNodeElectionData::<T>::get(subnet_id)
        {
            if !emergency_validator_data.activated {
                SubnetNodeElectionSlots::<T>::get(subnet_id)
            } else {
                if Self::is_emergency_validator_set_expired(
                    &emergency_validator_data,
                    subnet_id,
                    subnet_epoch,
                ) {
                    // Temporary emergency validators is complete, remove and return default election slots
                    Self::finish_emergency_validator_set(subnet_id);
                    SubnetNodeElectionSlots::<T>::get(subnet_id)
                } else {
                    Self::active_emergency_validator_ids(
                        &emergency_validator_data,
                        subnet_id,
                        subnet_epoch,
                    )
                }
            }
        } else {
            SubnetNodeElectionSlots::<T>::get(subnet_id)
        };

        if slot_list.is_empty() {
            return;
        }

        let Some(idx) = Self::get_bounded_random_index(
            (subnet_id, subnet_epoch, block),
            slot_list.len() as u32,
        ) else {
            return;
        };

        let subnet_node_id = slot_list.get(idx as usize).cloned();

        if let Some(node_id) = subnet_node_id {
            let policy = Self::consensus_policy_snapshot(subnet_id, subnet_epoch);
            let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, node_id);
            let validator_delegate_stake_balance = validator_id
                .map(ValidatorDelegateStakeBalance::<T>::get)
                .unwrap_or_default();

            // Persist the election before recording its validator-level metadata. Settlement for
            // this election happens at the next subnet slot and must not relabel the election.
            SubnetElectedValidator::<T>::insert(
                subnet_id,
                subnet_epoch,
                ElectedConsensusRound {
                    validator_subnet_node_id: node_id,
                    policy,
                    validator_delegate_stake_balance,
                },
            );

            if let Some(validator_id) = validator_id {
                // An enabled round locks outgoing pool operations through its settlement block.
                // Incoming stake and share transfers do not use this lock. Taking the maximum
                // preserves every outstanding liability when one identity has overlapping rounds.
                if policy.base_validator_delegate_stake_slash_percentage > 0
                    && policy.max_validator_delegate_stake_slash_amount > 0
                {
                    let settlement_block = block.saturating_add(T::EpochLength::get());
                    ValidatorDelegateStakeSlashLockUntil::<T>::mutate(validator_id, |lock_until| {
                        *lock_until = (*lock_until).max(settlement_block);
                    });
                }

                let election_epoch = Self::get_current_epoch_with_block_as_u32(block);
                Self::record_validator_election(validator_id, election_epoch);
            }
        }
    }
}
