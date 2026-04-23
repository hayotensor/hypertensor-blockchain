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
// Handles all reputation based logic for coldkeys, subnets, and subnet nodes
// Note: All calls to update reputation must first check if the entity exists
// before calling these functions.
//
// E.g. if !SubnetNodeIdHotkey::<T>::contains_key(subnet_id, subnet_node_id) { return; }

use super::*;
use frame_support::pallet_prelude::DispatchError;

impl<T: Config> Pallet<T> {
    /// Increase coldkey reptuation
    ///
    /// # Arguments
    ///
    /// * `coldkey` - Nodes coldkey
    /// * `attestation_percentage` - The attestation ratio of the validator nodes consensus
    /// * `min_attestation_percentage` - Blockchains minimum attestation percentage (66%)
    /// * `decrease_weight_factor` - `ColdkeyReputationIncreaseFactor`.
    /// * `epoch`: The blockchains general epoch
    pub fn increase_coldkey_reputation(
        coldkey: T::AccountId,
        attestation_percentage: u128,
        min_attestation_percentage: u128,
        increase_weight_factor: u128,
        epoch: u32,
    ) {
        if !ColdkeyReputation::<T>::contains_key(&coldkey) {
            return;
        }

        if attestation_percentage < min_attestation_percentage {
            return;
        }

        // Safe get, has Default value
        let mut coldkey_reputation = ColdkeyReputation::<T>::get(&coldkey);
        let current_score = coldkey_reputation.score;

        let new_score = Self::increase_rep(current_score, increase_weight_factor, None);

        // Update fields
        coldkey_reputation.score = new_score;
        coldkey_reputation.total_increases += 1;
        coldkey_reputation.last_validator_epoch = epoch;

        if coldkey_reputation.start_epoch == 0 {
            coldkey_reputation.start_epoch = epoch;
        }

        // Update average attestation
        let prev_total = coldkey_reputation
            .total_increases
            .saturating_add(coldkey_reputation.total_decreases)
            .saturating_sub(1) as u128;

        coldkey_reputation.average_attestation = if prev_total == 0 {
            attestation_percentage
        } else {
            (coldkey_reputation
                .average_attestation
                .saturating_mul(prev_total)
                .saturating_add(attestation_percentage))
            .saturating_div(prev_total + 1)
        };

        ColdkeyReputation::<T>::insert(&coldkey, coldkey_reputation);
    }

    /// Decrease coldkey reptuation
    ///
    /// # Arguments
    ///
    /// * `coldkey` - Nodes coldkey
    /// * `attestation_percentage` - The attestation ratio of the validator nodes consensus
    /// * `min_attestation_percentage` - Blockchains minimum attestation percentage (66%)
    /// * `decrease_weight_factor` - `ColdkeyReputationDecreaseFactor`.
    /// * `epoch`: The blockchains general epoch
    pub fn decrease_coldkey_reputation(
        coldkey: T::AccountId,
        attestation_percentage: u128,
        min_attestation_percentage: u128,
        decrease_weight_factor: u128, // <- slope/steepness control
        epoch: u32,
    ) {
        if !ColdkeyReputation::<T>::contains_key(&coldkey) {
            return;
        }

        if attestation_percentage >= min_attestation_percentage {
            return;
        }

        // Safe get, has Default value
        let mut coldkey_reputation = ColdkeyReputation::<T>::get(&coldkey);
        let current_score = coldkey_reputation.score;

        // Penalty increases as score increases (same pattern as reward logic)
        let new_score = Self::decrease_rep(current_score, decrease_weight_factor, None);

        coldkey_reputation.score = new_score;
        coldkey_reputation.total_decreases += 1;
        coldkey_reputation.last_validator_epoch = epoch;

        if coldkey_reputation.start_epoch == 0 {
            coldkey_reputation.start_epoch = epoch;
        }

        let prev_total = coldkey_reputation
            .total_increases
            .saturating_add(coldkey_reputation.total_decreases)
            .saturating_sub(1) as u128;

        coldkey_reputation.average_attestation = if prev_total == 0 {
            attestation_percentage
        } else {
            (coldkey_reputation
                .average_attestation
                .saturating_mul(prev_total)
                .saturating_add(attestation_percentage))
            .saturating_div(prev_total + 1)
        };

        ColdkeyReputation::<T>::insert(&coldkey, coldkey_reputation);
    }

    pub fn increase_validator_reputation(
        validator_id: u32,
        attestation_percentage: u128,
        min_attestation_percentage: u128,
        increase_weight_factor: u128,
        epoch: u32,
    ) {
        if !ValidatorReputation::<T>::contains_key(validator_id) {
            return;
        }

        if attestation_percentage < min_attestation_percentage {
            return;
        }

        // Safe get, has Default value
        let mut validator_reputation = ValidatorReputation::<T>::get(validator_id);
        let current_score = validator_reputation.score;

        let new_score = Self::increase_rep(current_score, increase_weight_factor, None);

        // Update fields
        validator_reputation.score = new_score;
        validator_reputation.total_increases += 1;
        validator_reputation.last_validator_epoch = epoch;

        if validator_reputation.start_epoch == 0 {
            validator_reputation.start_epoch = epoch;
        }

        // Update average attestation
        let prev_total = validator_reputation
            .total_increases
            .saturating_add(validator_reputation.total_decreases)
            .saturating_sub(1) as u128;

        validator_reputation.average_attestation = if prev_total == 0 {
            attestation_percentage
        } else {
            (validator_reputation
                .average_attestation
                .saturating_mul(prev_total)
                .saturating_add(attestation_percentage))
            .saturating_div(prev_total + 1)
        };

        ValidatorReputation::<T>::insert(validator_id, validator_reputation);
    }

    /// Decrease coldkey reptuation
    ///
    /// # Arguments
    ///
    /// * `coldkey` - Nodes coldkey
    /// * `attestation_percentage` - The attestation ratio of the validator nodes consensus
    /// * `min_attestation_percentage` - Blockchains minimum attestation percentage (66%)
    /// * `decrease_weight_factor` - `ColdkeyReputationDecreaseFactor`.
    /// * `epoch`: The blockchains general epoch
    pub fn decrease_validator_reputation(
        validator_id: u32,
        attestation_percentage: u128,
        min_attestation_percentage: u128,
        decrease_weight_factor: u128, // <- slope/steepness control
        epoch: u32,
    ) {
        if !ValidatorReputation::<T>::contains_key(validator_id) {
            return;
        }

        if attestation_percentage >= min_attestation_percentage {
            return;
        }

        // Safe get, has Default value
        let mut validator_reputation = ValidatorReputation::<T>::get(validator_id);
        let current_score = validator_reputation.score;

        // Penalty increases as score increases (same pattern as reward logic)
        let new_score = Self::decrease_rep(current_score, decrease_weight_factor, None);

        validator_reputation.score = new_score;
        validator_reputation.total_decreases += 1;
        validator_reputation.last_validator_epoch = epoch;

        if validator_reputation.start_epoch == 0 {
            validator_reputation.start_epoch = epoch;
        }

        let prev_total = validator_reputation
            .total_increases
            .saturating_add(validator_reputation.total_decreases)
            .saturating_sub(1) as u128;

        validator_reputation.average_attestation = if prev_total == 0 {
            attestation_percentage
        } else {
            (validator_reputation
                .average_attestation
                .saturating_mul(prev_total)
                .saturating_add(attestation_percentage))
            .saturating_div(prev_total + 1)
        };

        ValidatorReputation::<T>::insert(validator_id, validator_reputation);
    }

    pub fn increase_subnet_reputation(subnet_id: u32, factor_1: u128, factor_2: u128) {
        SubnetReputation::<T>::try_mutate(
            subnet_id,
            |n: &mut u128| -> Result<u128, DispatchError> {
                let prev_reputation = *n;
                *n = Self::increase_rep(*n, factor_1, Some(factor_2));
                Self::deposit_event(Event::SubnetReputationUpdate {
                    subnet_id,
                    prev_reputation,
                    new_reputation: *n,
                });
                Ok(*n)
            },
        );
    }

    pub fn decrease_subnet_reputation(subnet_id: u32, factor_1: u128, factor_2: Option<u128>) {
        SubnetReputation::<T>::try_mutate(
            subnet_id,
            |n: &mut u128| -> Result<u128, DispatchError> {
                let prev_reputation = *n;
                *n = Self::decrease_rep(*n, factor_1, factor_2);
                Self::deposit_event(Event::SubnetReputationUpdate {
                    subnet_id,
                    prev_reputation,
                    new_reputation: *n,
                });
                Ok(*n)
            },
        );
    }

    pub fn increase_node_reputation(subnet_id: u32, subnet_node_id: u32, factor: u128) {
        SubnetNodeReputation::<T>::mutate_exists(subnet_id, subnet_node_id, |maybe_reputation| {
            if let Some(reputation) = maybe_reputation {
                let prev_reputation = *reputation;
                *reputation = Self::increase_rep(prev_reputation, factor, None);
                Self::deposit_event(Event::NodeReputationUpdate {
                    subnet_id,
                    subnet_node_id,
                    prev_reputation,
                    new_reputation: *reputation,
                });
            }
        });
    }

    /// Increase node reputation and return new reputation
    /// This takes in the current reputation and updates the nodes reputation
    /// *based on the input parameter* being the source of truth of the reputation
    pub fn increase_and_return_node_reputation(
        subnet_id: u32,
        subnet_node_id: u32,
        current_reputation: u128,
        factor_1: u128,
        factor_2: Option<u128>,
    ) -> u128 {
        let new_reputation = SubnetNodeReputation::<T>::try_mutate_exists(
            subnet_id,
            subnet_node_id,
            |maybe_reputation| -> Result<u128, DispatchError> {
                if let Some(reputation) = maybe_reputation {
                    *reputation = Self::increase_rep(current_reputation, factor_1, factor_2);
                    Self::deposit_event(Event::NodeReputationUpdate {
                        subnet_id,
                        subnet_node_id,
                        prev_reputation: current_reputation,
                        new_reputation: *reputation,
                    });
                    Ok(*reputation)
                } else {
                    Ok(current_reputation)
                }
            },
        );

        new_reputation.unwrap_or(current_reputation)
    }

    /// Decrease from submitted node reputation and return new reputation
    /// This function is used to track reputations locally to lessen db reads
    pub fn decrease_and_return_node_reputation(
        subnet_id: u32,
        subnet_node_id: u32,
        current_reputation: u128,
        factor_1: u128,
        factor_2: Option<u128>,
    ) -> u128 {
        let new_reputation = SubnetNodeReputation::<T>::try_mutate_exists(
            subnet_id,
            subnet_node_id,
            |maybe_reputation| -> Result<u128, DispatchError> {
                if let Some(reputation) = maybe_reputation {
                    *reputation = Self::decrease_rep(current_reputation, factor_1, factor_2);
                    Self::deposit_event(Event::NodeReputationUpdate {
                        subnet_id,
                        subnet_node_id,
                        prev_reputation: current_reputation,
                        new_reputation: *reputation,
                    });
                    Ok(*reputation)
                } else {
                    Ok(current_reputation)
                }
            },
        );

        new_reputation.unwrap_or(current_reputation)
    }

    /// Increase reputation function designed to get a reputation back to 1.0
    ///
    /// # Formula
    ///
    /// Uses a pow function to calculate the new reputation
    ///
    /// # Arguments
    /// * `prev_reputation` - The previous reputation
    /// * `factor_1` - The first factor to apply
    /// * `factor_2` - The second factor to apply
    ///
    /// # Returns
    /// The new reputation
    pub fn increase_rep(prev_reputation: u128, factor_1: u128, factor_2: Option<u128>) -> u128 {
        let one = Self::percentage_factor_as_u128();
        if prev_reputation == one {
            return prev_reputation;
        }
        let factor = Self::percent_mul(factor_1, factor_2.unwrap_or(one));
        let one_f64 = Self::get_percent_as_f64(one);
        let factor_f64 = Self::get_percent_as_f64(factor);
        let prev_reputation_f64 = Self::get_percent_as_f64(prev_reputation);

        let x = Self::pow(prev_reputation_f64, one_f64 + factor_f64);
        let increase = x * factor_f64;
        (((prev_reputation_f64 + increase) * Self::percentage_factor_as_f64()) as u128)
            .min(Self::percentage_factor_as_u128())
    }

    /// Decrease reputation function designed to get a reputation back to 0.0
    ///
    /// # Formula
    ///
    /// Uses a simple multiplication to calculate the new reputation
    ///
    /// # Arguments
    /// * `prev_reputation` - The previous reputation
    /// * `factor_1` - The first factor to apply
    /// * `factor_2` - The second factor to apply
    ///
    /// # Returns
    /// The new reputation
    pub fn decrease_rep(prev_reputation: u128, factor_1: u128, factor_2: Option<u128>) -> u128 {
        if prev_reputation == 0 {
            return prev_reputation;
        }
        let one = Self::percentage_factor_as_u128();
        let factor = Self::percent_mul(factor_1, factor_2.unwrap_or(one));
        let delta = Self::percent_mul(prev_reputation, factor);
        prev_reputation.saturating_sub(delta).min(one)
    }

    /// Get the non consensus attestor factor
    ///
    /// # Arguments
    /// * `subnet_id` - The subnet id
    /// * `attestation_ratio` - The attestation ratio
    /// * `min_attestation_percentage` - The minimum attestation percentage
    /// * `percentage_factor` - The percentage factor (1e18)
    ///
    /// # Returns
    /// The non consensus attestor factor
    pub fn get_non_consensus_attestor_factor(
        subnet_id: u32,
        attestation_ratio: u128,
        min_attestation_percentage: u128,
        percentage_factor: u128,
    ) -> u128 {
        Self::percent_mul(
            NonConsensusAttestorDecreaseReputationFactor::<T>::get(subnet_id),
            percentage_factor.saturating_sub(Self::percent_div(
                attestation_ratio,
                min_attestation_percentage,
            )),
        )
    }
}
