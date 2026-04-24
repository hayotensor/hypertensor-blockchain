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
    pub fn do_add_overwatch_node_stake(
        origin: T::RuntimeOrigin,
        overwatch_node_id: u32,
        stake_to_be_added: u128,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        // Resolve the validator that owns this subnet node, then ensure the caller is that
        // validator's coldkey. Only the owner is allowed to add stake.
        let validator_id = OverwatchNodeValidatorId::<T>::try_get(overwatch_node_id)
            .map_err(|_| Error::<T>::InvalidSubnetNodeId)?;

        let validator_coldkey = ValidatorColdkey::<T>::try_get(validator_id)
            .map_err(|_| Error::<T>::InvalidValidatorId)?;

        ensure!(coldkey == validator_coldkey, Error::<T>::NotKeyOwner);

        ensure!(stake_to_be_added != 0, Error::<T>::InvalidAmount);

        let balance = match Self::u128_to_balance(stake_to_be_added) {
            Some(b) => b,
            None => return Err(Error::<T>::CouldNotConvertToBalance.into()),
        };

        let account_stake_balance: u128 = OverwatchNodeStakeBalance::<T>::get(overwatch_node_id);

        ensure!(
            account_stake_balance.saturating_add(stake_to_be_added)
                >= OverwatchMinStakeBalance::<T>::get(),
            Error::<T>::MinStakeNotReached
        );

        // --- Ensure the callers coldkey has enough stake to perform the transaction.
        ensure!(
            Self::can_remove_balance_from_coldkey_account(&coldkey, balance),
            Error::<T>::NotEnoughBalanceToStake
        );

        // --- Ensure the remove operation from the coldkey is a success.
        ensure!(
            Self::remove_balance_from_coldkey_account(&coldkey, balance) == true,
            Error::<T>::BalanceWithdrawalError
        );

        Self::increase_overwatch_node_stake(overwatch_node_id, stake_to_be_added);

        // Self::deposit_event(Event::StakeAdded(subnet_id, coldkey, hotkey, stake_to_be_added));

        Ok(())
    }

    pub fn do_remove_overwatch_node_stake(
        origin: T::RuntimeOrigin,
        overwatch_node_id: u32,
        is_overwatch_node: bool,
        stake_to_be_removed: u128,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        // Resolve the validator that owns this subnet node, then ensure the caller is that
        // validator's coldkey. Only the owner is allowed to add stake.
        let validator_id = OverwatchNodeValidatorId::<T>::try_get(overwatch_node_id)
            .map_err(|_| Error::<T>::InvalidSubnetNodeId)?;

        let validator_coldkey = ValidatorColdkey::<T>::try_get(validator_id)
            .map_err(|_| Error::<T>::InvalidValidatorId)?;

        ensure!(coldkey == validator_coldkey, Error::<T>::NotKeyOwner);

        // --- Ensure that the stake amount to be removed is above zero.
        ensure!(stake_to_be_removed > 0, Error::<T>::AmountZero);

        let account_stake_balance: u128 = OverwatchNodeStakeBalance::<T>::get(overwatch_node_id);

        // --- Ensure that the account has enough stake to withdraw.
        ensure!(
            account_stake_balance >= stake_to_be_removed,
            Error::<T>::NotEnoughStakeToWithdraw
        );

        // if user is still an overwatch node they must keep the required minimum balance
        if is_overwatch_node {
            ensure!(
                account_stake_balance.saturating_sub(stake_to_be_removed)
                    >= OverwatchMinStakeBalance::<T>::get(),
                Error::<T>::MinStakeNotReached
            );
        }

        // --- Ensure that we can convert this u128 to a balance.
        match Self::u128_to_balance(stake_to_be_removed) {
            Some(b) => b,
            None => return Err(Error::<T>::CouldNotConvertToBalance.into()),
        };

        let block: u32 = Self::get_current_block_as_u32();

        // --- 7. We remove the balance from the hotkey.
        Self::decrease_overwatch_node_stake(overwatch_node_id, stake_to_be_removed);

        // --- 9. We add the balancer to the coldkey.  If the above fails we will not credit this coldkey.
        Self::add_balance_to_unbonding_ledger(
            &coldkey,
            stake_to_be_removed,
            StakeCooldownEpochs::<T>::get() * T::EpochLength::get(),
            block,
        )
        .map_err(|e| e)?;

        // Self::deposit_event(Event::StakeRemoved(subnet_id, coldkey, hotkey, stake_to_be_removed));

        Ok(())
    }

    pub fn increase_overwatch_node_stake(overwatch_node_id: u32, amount: u128) {
        // -- increase account overwatch staking balance
        OverwatchNodeStakeBalance::<T>::mutate(overwatch_node_id, |mut n| {
            n.saturating_accrue(amount)
        });

        // -- increase total overwatch stake
        TotalOverwatchNodeStakeBalance::<T>::mutate(|mut n| n.saturating_accrue(amount));
    }

    pub fn decrease_overwatch_node_stake(overwatch_node_id: u32, amount: u128) {
        // -- decrease account overwatch staking balance
        OverwatchNodeStakeBalance::<T>::mutate(overwatch_node_id, |mut n| {
            n.saturating_reduce(amount)
        });

        // -- decrease total overwatch stake
        TotalOverwatchNodeStakeBalance::<T>::mutate(|mut n| n.saturating_reduce(amount));
    }
}
