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

impl<T: Config> Pallet<T> {
    /// Returns `true` only after a complete epoch period has elapsed.
    ///
    /// The boundary epoch (`start_epoch + period_epochs`) is still part of the
    /// waiting period. Saturating addition prevents an overflowing deadline
    /// from wrapping around and becoming immediately mature.
    pub(crate) fn has_epoch_period_elapsed(
        start_epoch: u32,
        period_epochs: u32,
        current_epoch: u32,
    ) -> bool {
        start_epoch.saturating_add(period_epochs) < current_epoch
    }

    pub fn get_tx_rate_limit() -> u32 {
        TxRateLimit::<T>::get()
    }

    pub fn set_last_tx_block(key: &T::AccountId, block: u32) {
        LastTxBlock::<T>::insert(key, block)
    }

    pub fn get_last_tx_block(key: &T::AccountId) -> u32 {
        LastTxBlock::<T>::get(key)
    }

    pub fn exceeds_tx_rate_limit(prev_tx_block: u32, current_block: u32) -> bool {
        let rate_limit: u32 = Self::get_tx_rate_limit();
        if rate_limit == 0 || prev_tx_block == 0 {
            return false;
        }

        return current_block - prev_tx_block <= rate_limit;
    }

    pub fn balance_to_u128(
        input: <<T as pallet::Config>::Currency as frame_support::traits::Currency<
            <T as frame_system::Config>::AccountId,
        >>::Balance,
    ) -> Option<u128> {
        input.try_into().ok()
    }

    /// Returns all locked, non-Overwatch capital for informational TVL accounting.
    /// Subnet survival depends only on live subnet delegate balances.
    ///
    /// Liquid currency and active or unbonding Overwatch stake are intentionally excluded. Queued
    /// swap principal and ordinary network unbonding remain included so moving capital between
    /// live network pools cannot temporarily lower the minimum stake required to keep a subnet
    /// alive. Arithmetic overflow fails closed at `u128::MAX` rather than lowering that minimum.
    pub fn get_total_network_tvl() -> u128 {
        [
            TotalStake::<T>::get(),
            TotalDelegateStake::<T>::get(),
            TotalValidatorDelegateStakeBalance::<T>::get(),
            TotalAccountDelegateStake::<T>::get(),
            TotalNetworkUnbondingBalance::<T>::get(),
            TotalQueuedSwapPrincipal::<T>::get(),
        ]
        .into_iter()
        .try_fold(0u128, u128::checked_add)
        .unwrap_or(u128::MAX)
    }

    pub fn get_avg_nodes_per_subnet() -> u128 {
        let subnets = TotalActiveSubnets::<T>::get();
        let nodes = TotalActiveNodes::<T>::get();
        Self::percent_div(nodes as u128, subnets as u128)
    }

    pub fn send_to_treasury(
        who: &T::AccountId,
        amount: <<T as pallet::Config>::Currency as Currency<
            <T as frame_system::Config>::AccountId,
        >>::Balance,
    ) -> DispatchResult {
        let treasury_account = T::TreasuryAccount::get();

        T::Currency::transfer(
            who,
            &treasury_account,
            amount,
            ExistenceRequirement::KeepAlive,
        )?;

        Ok(())
    }

    /// Add balance to treasury
    /// Used for epoch inflation
    pub fn add_balance_to_treasury(
        amount: <<T as pallet::Config>::Currency as Currency<
            <T as frame_system::Config>::AccountId,
        >>::Balance,
    ) {
        let treasury_account = T::TreasuryAccount::get();
        T::Currency::deposit_creating(&treasury_account, amount);
    }

    pub fn burn(
        who: T::AccountId,
        amount: <<T as pallet::Config>::Currency as Currency<
            <T as frame_system::Config>::AccountId,
        >>::Balance,
    ) -> bool {
        Self::remove_balance_from_coldkey_account(&who, amount)
    }

    pub fn is_paused() -> DispatchResult {
        ensure!(!TxPause::<T>::get(), Error::<T>::Paused);
        Ok(())
    }

    pub fn are_all_unique<V: Ord + Clone>(values: &[V]) -> bool {
        let set: BTreeSet<_> = values.iter().cloned().collect();
        set.len() == values.len()
    }
}
