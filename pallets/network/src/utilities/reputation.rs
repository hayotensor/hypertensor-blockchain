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
    /// Record the epoch represented by a newly persisted subnet-validator election.
    ///
    /// Election metadata is independent of the outcome, which is not settled until the next
    /// subnet epoch. Keeping this update on the election path also records validators that never
    /// submit a proposal.
    pub fn record_validator_election(validator_id: u32, election_epoch: u32) {
        ValidatorReputation::<T>::mutate(validator_id, |reputation| {
            reputation.start_epoch = Some(
                reputation
                    .start_epoch
                    .map_or(election_epoch, |epoch| epoch.min(election_epoch)),
            );
            reputation.last_validator_epoch = Some(
                reputation
                    .last_validator_epoch
                    .map_or(election_epoch, |epoch| epoch.max(election_epoch)),
            );
        });
    }

    pub fn get_reputation_factors_for_epoch(
        subnet_id: u32,
        evaluated_subnet_epoch: u32,
    ) -> SubnetReputationFactors {
        SubnetReputationFactorSchedules::<T>::get(subnet_id)
            .factors_for_epoch(evaluated_subnet_epoch)
    }

    fn append_validator_identity_support(
        validator_reputation: &mut Reputation,
        identity_attestation_ratio: u128,
    ) {
        // Freeze the arithmetic mean once its bounded sample counter saturates. Updating the mean
        // while leaving the denominator at `u32::MAX` would give later samples inconsistent
        // historical weight.
        if validator_reputation.identity_support_samples == u32::MAX {
            return;
        }

        let identity_attestation_ratio =
            identity_attestation_ratio.min(Self::percentage_factor_as_u128());
        let previous_samples = validator_reputation.identity_support_samples as u128;

        validator_reputation.average_proposal_identity_support = if previous_samples == 0 {
            identity_attestation_ratio
        } else {
            validator_reputation
                .average_proposal_identity_support
                .saturating_mul(previous_samples)
                .saturating_add(identity_attestation_ratio)
                .saturating_div(previous_samples.saturating_add(1))
        };
        validator_reputation.identity_support_samples += 1;
    }

    /// Record a settled election's distinct-validator-identity support without changing score.
    pub fn record_validator_identity_support(validator_id: u32, identity_attestation_ratio: u128) {
        if !ValidatorReputation::<T>::contains_key(validator_id) {
            return;
        }

        let mut validator_reputation = ValidatorReputation::<T>::get(validator_id);
        Self::append_validator_identity_support(
            &mut validator_reputation,
            identity_attestation_ratio,
        );
        ValidatorReputation::<T>::insert(validator_id, validator_reputation);
    }

    /// Record an accepted proposal and increase its validator identity's score only when a
    /// supermajority of distinct eligible validator identities endorsed it.
    pub fn increase_validator_reputation(
        validator_id: u32,
        identity_attestation_ratio: u128,
        identity_super_majority_threshold: u128,
        increase_weight_factor: u128,
    ) {
        if !ValidatorReputation::<T>::contains_key(validator_id) {
            return;
        }

        let mut validator_reputation = ValidatorReputation::<T>::get(validator_id);
        Self::append_validator_identity_support(
            &mut validator_reputation,
            identity_attestation_ratio,
        );

        if identity_attestation_ratio >= identity_super_majority_threshold
            && increase_weight_factor > 0
        {
            validator_reputation.score =
                Self::increase_rep(validator_reputation.score, increase_weight_factor, None);
            validator_reputation.total_increases =
                validator_reputation.total_increases.saturating_add(1);
        }

        ValidatorReputation::<T>::insert(validator_id, validator_reputation);
    }

    /// Record a rejected proposal and decrease its validator identity's score only by the supplied
    /// distinct-identity strong-rejection shortfall.
    pub fn decrease_validator_reputation(
        validator_id: u32,
        identity_attestation_ratio: u128,
        identity_shortfall: Option<u128>,
        decrease_weight_factor: u128,
    ) {
        if !ValidatorReputation::<T>::contains_key(validator_id) {
            return;
        }

        let mut validator_reputation = ValidatorReputation::<T>::get(validator_id);
        Self::append_validator_identity_support(
            &mut validator_reputation,
            identity_attestation_ratio,
        );

        let identity_shortfall =
            identity_shortfall.map(|value| value.min(Self::percentage_factor_as_u128()));
        if identity_shortfall
            .is_some_and(|shortfall| Self::percent_mul(decrease_weight_factor, shortfall) > 0)
        {
            validator_reputation.score = Self::decrease_rep(
                validator_reputation.score,
                decrease_weight_factor,
                identity_shortfall,
            );
            validator_reputation.total_decreases =
                validator_reputation.total_decreases.saturating_add(1);
        }

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
}
