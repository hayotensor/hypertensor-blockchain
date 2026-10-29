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
// Handles all reputation based logic for subnets and subnet nodes
// Node callers load the existing reputation once, calculate locally, and persist the
// final change before leaving that node's settlement branch.

use super::*;
use frame_support::{pallet_prelude::DispatchError, weights::Weight};

impl<T: Config> Pallet<T> {
    pub fn get_reputation_factors_for_epoch(
        subnet_id: u32,
        evaluated_subnet_epoch: u32,
    ) -> SubnetReputationFactors {
        SubnetReputationFactorSchedules::<T>::get(subnet_id)
            .factors_for_epoch(evaluated_subnet_epoch)
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

    /// Persist one completed node calculation, without re-reading its reputation.
    /// The caller must have loaded an existing entry and must not remove it before this call.
    /// Unchanged results produce neither a storage write nor a reputation event.
    pub(crate) fn persist_node_reputation_change(
        subnet_id: u32,
        subnet_node_id: u32,
        previous: u128,
        current: u128,
    ) -> Weight {
        if previous == current {
            return Weight::zero();
        }
        SubnetNodeReputation::<T>::insert(subnet_id, subnet_node_id, current);
        Self::deposit_event(Event::NodeReputationUpdate {
            subnet_id,
            subnet_node_id,
            prev_reputation: previous,
            new_reputation: current,
        });
        // Reputation write plus System event storage. This diagnostic cost includes repeated
        // event accesses; the generated settlement model accounts for their shared keys.
        T::DbWeight::get().reads_writes(4, 3)
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
