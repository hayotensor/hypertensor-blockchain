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
use sp_core::U256;

/// Immutable delegate balances for the subnets that are economically live at one block.
pub(crate) struct LiveSubnetDelegateStakeSnapshot {
    pub balances: BTreeMap<u32, u128>,
}

impl<T: Config> Pallet<T> {
    /// Returns the common delegate-stake minimum required for a subnet to stay active.
    ///
    /// The requirement is the greater of the configured absolute minimum and the configured
    /// percentage of average delegation across live subnets. The queried subnet affects the
    /// result only when it already belongs to that shared live cohort.
    pub fn get_min_subnet_delegate_stake_balance(_subnet_id: u32) -> u128 {
        let block = Self::get_current_block_as_u32();
        let subnets: Vec<_> = SubnetsData::<T>::iter().collect();
        let snapshot = Self::live_subnet_delegate_stake_snapshot(block, &subnets);

        Self::min_subnet_delegate_stake_balance_from_snapshot(&snapshot)
    }

    /// Snapshots delegate balances for active subnets whose consensus start epoch has arrived.
    pub(crate) fn live_subnet_delegate_stake_snapshot(
        block: u32,
        subnets: &[(u32, SubnetData)],
    ) -> LiveSubnetDelegateStakeSnapshot {
        let mut balances = BTreeMap::new();

        for (subnet_id, data) in subnets {
            if data.state != SubnetState::Active {
                continue;
            }
            let subnet_epoch = Self::get_subnet_epoch_with_block_as_u32(*subnet_id, block);
            if Self::_is_subnet_active_and_live(data, subnet_epoch) {
                balances.insert(
                    *subnet_id,
                    TotalSubnetDelegateStakeBalance::<T>::get(subnet_id),
                );
            }
        }

        LiveSubnetDelegateStakeSnapshot { balances }
    }

    /// Calculates `max(absolute_minimum, factor * total / count)` from one immutable snapshot.
    ///
    /// All arithmetic is performed in U256. An impossible arithmetic or conversion failure returns
    /// `u128::MAX`, so an accounting failure cannot lower the survival requirement.
    pub(crate) fn min_subnet_delegate_stake_balance_from_snapshot(
        snapshot: &LiveSubnetDelegateStakeSnapshot,
    ) -> u128 {
        let absolute_minimum = MinSubnetDelegateStakeBalance::<T>::get();
        let live_subnet_count = snapshot.balances.len() as u128;

        if live_subnet_count == 0 {
            return absolute_minimum;
        }

        let total_live_delegation = snapshot
            .balances
            .values()
            .try_fold(U256::zero(), |total, balance| {
                total.checked_add(U256::from(*balance))
            });
        let divisor = U256::from(live_subnet_count).checked_mul(Self::PERCENTAGE_FACTOR);

        let dynamic_minimum = total_live_delegation
            .zip(divisor)
            .and_then(|(total, divisor)| {
                Self::checked_mul_div(
                    total,
                    U256::from(MinSubnetDelegateStakeFactor::<T>::get()),
                    divisor,
                )
            })
            .and_then(|value| value.try_into().ok())
            .unwrap_or(u128::MAX);

        absolute_minimum.max(dynamic_minimum)
    }
}
