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
use sp_runtime::Saturating;

impl<T: Config> Pallet<T> {
    pub fn do_remove_delegate_balance(
        origin: T::RuntimeOrigin,
        amount_to_remove: u128,
    ) -> DispatchResult {
        let account_id: T::AccountId = ensure_signed(origin)?;

        let account_delegate_balance: u128 = AccountDelegateStake::<T>::get(&account_id);

        ensure!(amount_to_remove > 0, Error::<T>::AmountZero);

        // --- Ensure that the stake amount to be removed is above zero.
        // --- Ensure that the account has enough stake to withdraw.
        ensure!(
            account_delegate_balance >= amount_to_remove,
            Error::<T>::NotEnoughStakeToWithdraw
        );

        // --- Ensure that we can convert this u128 to a balance.
        match Self::u128_to_balance(amount_to_remove) {
            Some(b) => b,
            None => return Err(Error::<T>::CouldNotConvertToBalance.into()),
        };

        Self::decrease_delegate_account_balance(&account_id, amount_to_remove);

        let block: u32 = Self::get_current_block_as_u32();

        // Add to ledger and always match the stake cooldown epochs (or greater cooldown)
        Self::add_balance_to_unbonding_ledger(
            &account_id,
            amount_to_remove,
            StakeCooldownEpochs::<T>::get() * T::EpochLength::get(),
            block,
        )
        .map_err(|e| e)?;

        Self::deposit_event(Event::DelegateBalanceRemoved {
            account_id,
            amount: amount_to_remove,
        });

        Ok(())
    }

    pub fn increase_delegate_account_balance(account_id: &T::AccountId, amount: u128) {
        // -- increase delegate account balance
        AccountDelegateStake::<T>::mutate(account_id, |mut n| n.saturating_accrue(amount));

        // -- increase total account delegate stake
        TotalAccountDelegateStake::<T>::mutate(|mut n| n.saturating_accrue(amount));
    }

    pub fn decrease_delegate_account_balance(account_id: &T::AccountId, amount: u128) {
        // -- decrease delegate account balance
        AccountDelegateStake::<T>::mutate(account_id, |mut n| n.saturating_reduce(amount));

        // -- decrease total account delegate stake
        TotalAccountDelegateStake::<T>::mutate(|mut n| n.saturating_reduce(amount));
    }
}
