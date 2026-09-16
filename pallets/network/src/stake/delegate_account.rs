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
// Delegate account business logic
// Delegate accounts are the accounts nodes can allocate emissions to

use super::*;
use sp_runtime::ArithmeticError;

impl<T: Config> Pallet<T> {
    #[frame_support::transactional]
    pub(crate) fn do_remove_delegate_account_balance(
        origin: T::RuntimeOrigin,
        amount_to_remove: u128,
    ) -> DispatchResult {
        let account_id: T::AccountId = ensure_signed(origin)?;

        let account_delegate_balance: u128 = DelegateAccountStake::<T>::get(&account_id);

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

        let block: u32 = Self::get_current_block_as_u32();
        let cooldown_blocks = StakeCooldownEpochs::<T>::get()
            .checked_mul(T::EpochLength::get())
            .ok_or(sp_runtime::ArithmeticError::Overflow)?;

        Self::decrease_delegate_account_balance(&account_id, amount_to_remove)?;

        // Add to ledger and always match the stake cooldown epochs (or greater cooldown)
        Self::add_balance_to_unbonding_ledger(
            &account_id,
            amount_to_remove,
            cooldown_blocks,
            block,
            UnbondingSource::Network,
        )?;

        Self::deposit_event(Event::DelegateBalanceRemoved {
            account_id,
            amount: amount_to_remove,
        });

        Ok(())
    }

    /// Increase a delegate-account position and its aggregate only when both additions fit.
    pub(crate) fn increase_delegate_account_balance(
        account_id: &T::AccountId,
        amount: u128,
    ) -> DispatchResult {
        let next_account_stake = DelegateAccountStake::<T>::get(account_id)
            .checked_add(amount)
            .ok_or(ArithmeticError::Overflow)?;
        let next_total_stake = TotalAccountDelegateStake::<T>::get()
            .checked_add(amount)
            .ok_or(ArithmeticError::Overflow)?;

        DelegateAccountStake::<T>::insert(account_id, next_account_stake);
        TotalAccountDelegateStake::<T>::put(next_total_stake);
        Ok(())
    }

    /// Decrease a delegate-account position and its aggregate only when both contain the amount.
    pub(crate) fn decrease_delegate_account_balance(
        account_id: &T::AccountId,
        amount: u128,
    ) -> DispatchResult {
        let next_account_stake = DelegateAccountStake::<T>::get(account_id)
            .checked_sub(amount)
            .ok_or(ArithmeticError::Underflow)?;
        let next_total_stake = TotalAccountDelegateStake::<T>::get()
            .checked_sub(amount)
            .ok_or(ArithmeticError::Underflow)?;

        DelegateAccountStake::<T>::insert(account_id, next_account_stake);
        TotalAccountDelegateStake::<T>::put(next_total_stake);
        Ok(())
    }
}
