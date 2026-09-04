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
use frame_support::pallet_prelude::{DispatchError, Zero};
use frame_support::traits::Imbalance;
use sp_core::U256;
use sp_runtime::ArithmeticError;

impl<T: Config> Pallet<T> {
    /// Dead shares minted on the first delegate-pool deposit to make donation attacks uneconomic.
    pub const DELEGATE_POOL_MIN_LIQUIDITY: u128 = 1_000;

    /// Immutable 10:1 virtual share/balance offset used by delegate-pool accounting.
    pub const DELEGATE_POOL_VIRTUAL_SHARES: u128 = 10;
    pub const DELEGATE_POOL_VIRTUAL_BALANCE: u128 = 1;

    #[frame_support::transactional]
    pub fn add_balance_to_unbonding_ledger(
        coldkey: &T::AccountId,
        amount: u128,
        cooldown_blocks: u32,
        block: u32,
        source: UnbondingSource,
    ) -> DispatchResult {
        let claim_block = Self::prepare_unbonding_ledger_entry(coldkey, cooldown_blocks, block)?;

        let next_total_network_unbonding = match source {
            UnbondingSource::Network => Some(
                TotalNetworkUnbondingBalance::<T>::get()
                    .checked_add(amount)
                    .ok_or(ArithmeticError::Overflow)?,
            ),
            UnbondingSource::Overwatch => None,
        };

        StakeUnbondingLedger::<T>::try_mutate(coldkey, |ledger| -> DispatchResult {
            let entry = ledger.entry(claim_block).or_default();
            match source {
                UnbondingSource::Network => {
                    entry.network = entry
                        .network
                        .checked_add(amount)
                        .ok_or(ArithmeticError::Overflow)?;
                }
                UnbondingSource::Overwatch => {
                    entry.overwatch = entry
                        .overwatch
                        .checked_add(amount)
                        .ok_or(ArithmeticError::Overflow)?;
                }
            }
            Ok(())
        })?;

        if let Some(total) = next_total_network_unbonding {
            TotalNetworkUnbondingBalance::<T>::put(total);
        }

        Ok(())
    }

    pub fn prepare_unbonding_ledger_entry(
        coldkey: &T::AccountId,
        cooldown_blocks: u32,
        block: u32,
    ) -> Result<u32, DispatchError> {
        let claim_block = block
            .checked_add(cooldown_blocks)
            .ok_or(ArithmeticError::Overflow)?;
        let max_unbondings = MaxUnbondings::<T>::get();
        let mut unbondings = StakeUnbondingLedger::<T>::get(coldkey);

        if unbondings.contains_key(&claim_block) {
            return Ok(claim_block);
        }

        // --- Ensure we don't surpass max unlockings by attempting to unlock unbondings
        if unbondings.len() as u32 >= max_unbondings {
            Self::do_claim_unbondings(coldkey);
            unbondings = StakeUnbondingLedger::<T>::get(coldkey);
        }

        ensure!(
            unbondings.contains_key(&claim_block) || (unbondings.len() as u32) < max_unbondings,
            Error::<T>::MaxUnlockingsReached
        );

        Ok(claim_block)
    }

    pub fn do_claim_unbondings(coldkey: &T::AccountId) -> u32 {
        let block = Self::get_current_block_as_u32();
        let unbondings = StakeUnbondingLedger::<T>::get(&coldkey);

        let mut unbondings_copy = unbondings.clone();

        let mut successful_unbondings = 0;

        for (unbonding_block, entry) in unbondings.iter() {
            if block < *unbonding_block {
                continue;
            }

            let Some(amount) = entry.network.checked_add(entry.overwatch) else {
                continue;
            };
            let stake_to_be_added_as_currency = match Self::u128_to_balance(amount) {
                Some(b) => b,
                None => continue,
            };
            let Some(total_network_unbonding) =
                TotalNetworkUnbondingBalance::<T>::get().checked_sub(entry.network)
            else {
                continue;
            };

            // A reaped currency account cannot be recreated below the existential deposit.
            // Retain the entry until the full principal can actually be credited.
            if T::Currency::total_balance(coldkey).is_zero()
                && stake_to_be_added_as_currency < T::Currency::minimum_balance()
            {
                continue;
            }

            let credited = T::Currency::deposit_creating(coldkey, stake_to_be_added_as_currency);
            if credited.peek() != stake_to_be_added_as_currency {
                continue;
            }
            drop(credited);

            TotalNetworkUnbondingBalance::<T>::put(total_network_unbonding);
            unbondings_copy.remove(&unbonding_block);
            successful_unbondings += 1;
        }

        if unbondings.len() != unbondings_copy.len() {
            StakeUnbondingLedger::<T>::insert(&coldkey, unbondings_copy);
        }
        successful_unbondings
    }

    pub fn can_remove_balance_from_coldkey_account(
        coldkey: &T::AccountId,
        amount: <<T as pallet::Config>::Currency as Currency<
            <T as frame_system::Config>::AccountId,
        >>::Balance,
    ) -> bool {
        let current_balance = Self::get_coldkey_balance(coldkey);
        if amount > current_balance {
            return false;
        }

        let new_potential_balance = current_balance - amount;
        let can_withdraw = T::Currency::ensure_can_withdraw(
            &coldkey,
            amount,
            WithdrawReasons::except(WithdrawReasons::TIP),
            new_potential_balance,
        )
        .is_ok();
        can_withdraw
    }

    pub fn remove_balance_from_coldkey_account(
        coldkey: &T::AccountId,
        amount: <<T as pallet::Config>::Currency as Currency<
            <T as frame_system::Config>::AccountId,
        >>::Balance,
    ) -> bool {
        return match T::Currency::withdraw(
            &coldkey,
            amount,
            WithdrawReasons::except(WithdrawReasons::TIP),
            ExistenceRequirement::KeepAlive,
        ) {
            Ok(_result) => true,
            Err(_error) => false,
        };
    }

    pub fn add_balance_to_coldkey_account(
        coldkey: &T::AccountId,
        amount: <<T as pallet::Config>::Currency as Currency<
            <T as frame_system::Config>::AccountId,
        >>::Balance,
    ) {
        T::Currency::deposit_creating(&coldkey, amount);
    }

    pub fn get_coldkey_balance(
        coldkey: &T::AccountId,
    ) -> <<T as pallet::Config>::Currency as Currency<<T as system::Config>::AccountId>>::Balance
    {
        return T::Currency::free_balance(&coldkey);
    }

    pub fn u128_to_balance(
        input: u128,
    ) -> Option<
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance,
    >{
        input.try_into().ok()
    }

    /// Convert TENSOR balance to shares in vault
    ///
    /// # Arguments
    ///
    /// * `balance` - Amount of TENSOR to convert to shares.
    /// * `total_shares` - Total shares in the vault.
    /// * `total_balance` - Total balance of TENSOR in the vault.
    ///
    pub fn convert_to_shares(balance: u128, total_shares: u128, total_balance: u128) -> u128 {
        if total_shares == 0 {
            return balance;
        }

        let balance = U256::from(balance);
        let total_shares =
            U256::from(total_shares) + U256::from(Self::DELEGATE_POOL_VIRTUAL_SHARES);
        let total_balance =
            U256::from(total_balance) + U256::from(Self::DELEGATE_POOL_VIRTUAL_BALANCE);

        Self::checked_mul_div(balance, total_shares, total_balance)
            .and_then(|res| res.try_into().ok())
            .unwrap_or(u128::MAX)
    }

    /// Convert vault shares to TENSOR balance
    ///
    /// # Arguments
    ///
    /// * `shares` - Amount of shares to convert to TENSOR.
    /// * `total_shares` - Total shares in the vault.
    /// * `total_balance` - Total balance of TENSOR in the vault.
    ///
    pub fn convert_to_balance(shares: u128, total_shares: u128, total_balance: u128) -> u128 {
        if total_shares == 0 {
            return shares;
        }

        let shares = U256::from(shares);
        let total_balance =
            U256::from(total_balance) + U256::from(Self::DELEGATE_POOL_VIRTUAL_BALANCE);
        let total_shares =
            U256::from(total_shares) + U256::from(Self::DELEGATE_POOL_VIRTUAL_SHARES);

        Self::checked_mul_div(shares, total_balance, total_shares)
            .and_then(|res| res.try_into().ok())
            .unwrap_or(u128::MAX)
    }
}
