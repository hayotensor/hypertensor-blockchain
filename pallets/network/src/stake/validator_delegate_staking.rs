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
// Enables accounts to delegate stake to validators for a portion of emissions

use super::*;
use sp_runtime::Saturating;

impl<T: Config> Pallet<T> {
    pub fn do_add_validator_delegate_stake(
        origin: T::RuntimeOrigin,
        validator_id: u32,
        delegate_stake_to_be_added: u128,
    ) -> DispatchResult {
        let account_id: T::AccountId = ensure_signed(origin)?;

        let (result, balance, shares) = Self::perform_do_add_validator_delegate_stake(
            &account_id,
            validator_id,
            delegate_stake_to_be_added,
            false,
        );

        result?;

        let block: u32 = Self::get_current_block_as_u32();

        // Set last block for rate limiting
        Self::set_last_tx_block(&account_id, block);

        // Self::deposit_event(Event::ValidatorDelegateStakeAdded(
        //     validator_id,
        //     account_id,
        //     delegate_stake_to_be_added,
        // ));

        Ok(())
    }

    /// Add to the validator delegate stake balance of a user
    ///
    /// # Arguments
    ///
    /// * `account_id` - Account adding to balance of validator.
    /// * `validator_id` - Validator ID adding stake to.
    /// * `delegate_stake_to_be_added` - Balance to add or swap.
    /// * `swap` - If we are swapping between validators or nodes.
    ///              - True: Don't remove balance from users account
    ///              - False: Check user balance is withdrawable and withdraw balance
    ///
    pub fn perform_do_add_validator_delegate_stake(
        account_id: &T::AccountId,
        validator_id: u32,
        delegate_stake_to_be_added: u128,
        swap: bool,
    ) -> (DispatchResult, u128, u128) {
        let balance = match Self::u128_to_balance(delegate_stake_to_be_added) {
            Some(b) => b,
            None => return (Err(Error::<T>::CouldNotConvertToBalance.into()), 0, 0),
        };

        if delegate_stake_to_be_added < MinDelegateStakeDeposit::<T>::get() {
            return (
                Err(Error::<T>::MinDelegateStakeDepositNotReached.into()),
                0,
                0,
            );
        }

        // --- Ensure the callers account_id has enough delegate_stake to perform the transaction.
        if !swap {
            if !Self::can_remove_balance_from_coldkey_account(&account_id, balance) {
                return (Err(Error::<T>::NotEnoughBalanceToStake.into()), 0, 0);
            }
        }

        // to-do: add AddStakeRateLimit instead of universal rate limiter
        //        this allows peers to come in freely
        let block: u32 = Self::get_current_block_as_u32();
        if Self::exceeds_tx_rate_limit(Self::get_last_tx_block(&account_id), block) {
            return (Err(Error::<T>::TxRateLimitExceeded.into()), 0, 0);
        }

        // --- Ensure the remove operation from the account_id is a success.
        if !swap {
            if Self::remove_balance_from_coldkey_account(&account_id, balance) == false {
                return (Err(Error::<T>::BalanceWithdrawalError.into()), 0, 0);
            }
        }

        Self::handle_increase_account_validator_delegate_stake(
            account_id,
            validator_id,
            delegate_stake_to_be_added,
        )
    }

    // Infallible
    pub fn handle_increase_account_validator_delegate_stake(
        account_id: &T::AccountId,
        validator_id: u32,
        delegate_stake_to_be_added: u128,
    ) -> (DispatchResult, u128, u128) {
        let total_validator_delegated_stake_shares =
            match TotalValidatorDelegateStakeShares::<T>::get(validator_id) {
                0 => {
                    // --- Mitigate inflation attack
                    TotalValidatorDelegateStakeShares::<T>::mutate(validator_id, |mut n| {
                        n.saturating_accrue(Self::MIN_LIQUIDITY)
                    });
                    0
                }
                shares => shares,
            };
        let total_validator_delegated_stake_balance =
            TotalValidatorDelegateStakeBalance::<T>::get(validator_id);

        // --- Get amount to be added as shares based on stake to balance added to account
        let delegate_stake_to_be_added_as_shares = Self::convert_to_shares(
            delegate_stake_to_be_added,
            total_validator_delegated_stake_shares,
            total_validator_delegated_stake_balance,
        );

        // --- Check rounding errors, mitigates donation attacks that round to zero
        if delegate_stake_to_be_added_as_shares == 0 {
            return (Err(Error::<T>::CouldNotConvertToShares.into()), 0, 0);
        }

        Self::increase_account_validator_delegate_stake(
            &account_id,
            validator_id,
            delegate_stake_to_be_added,
            delegate_stake_to_be_added_as_shares,
        );

        (
            Ok(()),
            delegate_stake_to_be_added,
            delegate_stake_to_be_added_as_shares,
        )
    }

    pub fn do_remove_validator_delegate_stake(
        origin: T::RuntimeOrigin,
        validator_id: u32,
        delegate_stake_shares_to_be_removed: u128,
    ) -> DispatchResult {
        let account_id: T::AccountId = ensure_signed(origin)?;

        let (result, delegate_stake_to_be_removed, _) =
            Self::perform_do_remove_validator_delegate_stake(
                &account_id,
                validator_id,
                delegate_stake_shares_to_be_removed,
                true,
            );

        result?;

        let block: u32 = Self::get_current_block_as_u32();

        // Set last block for rate limiting
        Self::set_last_tx_block(&account_id, block);

        // Self::deposit_event(Event::ValidatorDelegateStakeRemoved(
        //     validator_id,
        //     account_id,
        //     delegate_stake_to_be_removed,
        // ));

        Ok(())
    }

    /// Remove the validator delegate stake balance of a user
    ///
    /// # Arguments
    ///
    /// * `account_id` - Account removing balance from validator.
    /// * `validator_id` - Validator ID removing stake from.
    /// * `delegate_stake_shares_to_be_removed` - Shares of pool to remove.
    /// * `add_to_ledger` - If we are unstaking from network and not swapping between staking options.
    ///              - True: Unstake user to unstaking ledger.
    ///              - False: Don't add balance to unstaking ledger.
    ///
    pub fn perform_do_remove_validator_delegate_stake(
        account_id: &T::AccountId,
        validator_id: u32,
        delegate_stake_shares_to_be_removed: u128,
        add_to_ledger: bool,
    ) -> (DispatchResult, u128, u128) {
        // --- Ensure that the delegate_stake amount to be removed is above zero.
        if delegate_stake_shares_to_be_removed == 0 {
            return (Err(Error::<T>::SharesZero.into()), 0, 0);
        }

        let account_validator_delegate_stake_shares: u128 =
            AccountValidatorDelegateStakeShares::<T>::get(&account_id, validator_id);

        // --- Ensure that the account has enough delegate_stake to withdraw.
        if account_validator_delegate_stake_shares < delegate_stake_shares_to_be_removed {
            return (Err(Error::<T>::NotEnoughStakeToWithdraw.into()), 0, 0);
        }

        let total_validator_delegated_stake_shares =
            TotalValidatorDelegateStakeShares::<T>::get(validator_id);
        let total_validator_delegated_stake_balance =
            TotalValidatorDelegateStakeBalance::<T>::get(validator_id);

        // --- Get accounts current balance
        let delegate_stake_to_be_removed = Self::convert_to_balance(
            delegate_stake_shares_to_be_removed,
            total_validator_delegated_stake_shares,
            total_validator_delegated_stake_balance,
        );

        // --- Ensure that we can convert this u128 to a balance.
        // Redunant
        let delegate_stake_to_be_added_as_currency =
            match Self::u128_to_balance(delegate_stake_to_be_removed) {
                Some(b) => b,
                None => return (Err(Error::<T>::CouldNotConvertToBalance.into()), 0, 0),
            };

        let block: u32 = Self::get_current_block_as_u32();
        if Self::exceeds_tx_rate_limit(Self::get_last_tx_block(&account_id), block) {
            return (Err(Error::<T>::TxRateLimitExceeded.into()), 0, 0);
        }

        // --- We remove the shares from the account and balance from the pool
        Self::decrease_account_validator_delegate_stake(
            &account_id,
            validator_id,
            delegate_stake_to_be_removed,
            delegate_stake_shares_to_be_removed,
        );

        // --- We add the balancer to the account_id.  If the above fails we will not credit this account_id.
        if add_to_ledger {
            let result = Self::add_balance_to_unbonding_ledger(
                &account_id,
                delegate_stake_to_be_removed,
                DelegateStakeCooldownEpochs::<T>::get() * T::EpochLength::get(),
                block,
            );

            if let Err(e) = result {
                return (Err(e), 0, 0);
            }
        }

        (
            Ok(()),
            delegate_stake_to_be_removed,
            delegate_stake_shares_to_be_removed,
        )
    }

    pub fn do_transfer_validator_delegate_stake(
        origin: T::RuntimeOrigin,
        validator_id: u32,
        to_account_id: T::AccountId,
        delegate_stake_shares_to_transfer: u128,
    ) -> DispatchResult {
        let account_id: T::AccountId = ensure_signed(origin)?;

        ensure!(
            account_id != to_account_id,
            Error::<T>::TransferToSelfNotAllowed
        );

        ensure!(
            delegate_stake_shares_to_transfer != 0,
            Error::<T>::NotEnoughStakeToWithdraw
        );

        let total_validator_delegated_stake_shares =
            TotalValidatorDelegateStakeShares::<T>::get(validator_id);
        let total_validator_delegated_stake_balance =
            TotalValidatorDelegateStakeBalance::<T>::get(validator_id);

        // --- Get accounts current balance
        let delegate_stake_to_be_transferred = Self::convert_to_balance(
            delegate_stake_shares_to_transfer,
            total_validator_delegated_stake_shares,
            total_validator_delegated_stake_balance,
        );

        // --- Ensure transfer balance is greater than the min
        ensure!(
            delegate_stake_to_be_transferred >= MinDelegateStakeDeposit::<T>::get(),
            Error::<T>::MinDelegateStakeDepositNotReached
        );

        // --- Remove shares from caller
        Self::decrease_account_validator_delegate_stake(
            &account_id,
            validator_id,
            0, // Do not mutate balance since we are transferring in the same validator
            delegate_stake_shares_to_transfer,
        );

        // --- Increase shares to `to_account_id`
        Self::increase_account_validator_delegate_stake(
            &to_account_id,
            validator_id,
            0, // Do not mutate balance since we are transferring in the same validator
            delegate_stake_shares_to_transfer,
        );

        Ok(())
    }

    pub fn increase_account_validator_delegate_stake(
        account_id: &T::AccountId,
        validator_id: u32,
        amount: u128,
        shares: u128,
    ) {
        // -- increase account validator staking shares balance
        AccountValidatorDelegateStakeShares::<T>::mutate(account_id, validator_id, |mut n| {
            n.saturating_accrue(shares)
        });

        // -- increase total validator delegate stake balance
        TotalValidatorDelegateStakeBalance::<T>::mutate(validator_id, |mut n| {
            n.saturating_accrue(amount)
        });

        // -- increase total validator delegate stake shares
        TotalValidatorDelegateStakeShares::<T>::mutate(validator_id, |mut n| {
            n.saturating_accrue(shares)
        });

        TotalDelegateStake::<T>::mutate(|mut n| n.saturating_accrue(amount));
    }

    pub fn decrease_account_validator_delegate_stake(
        account_id: &T::AccountId,
        validator_id: u32,
        amount: u128,
        shares: u128,
    ) {
        // -- decrease account validator staking shares balance
        AccountValidatorDelegateStakeShares::<T>::mutate(account_id, validator_id, |mut n| {
            n.saturating_reduce(shares)
        });

        // -- decrease total validator delegate stake balance
        TotalValidatorDelegateStakeBalance::<T>::mutate(validator_id, |mut n| {
            n.saturating_reduce(amount)
        });

        // -- decrease total validator delegate stake shares
        TotalValidatorDelegateStakeShares::<T>::mutate(validator_id, |mut n| {
            n.saturating_reduce(shares)
        });

        TotalDelegateStake::<T>::mutate(|mut n| n.saturating_reduce(amount));
    }

    /// Rewards are deposited here from the ``rewards.rs`` or by donations
    pub fn do_increase_validator_delegate_stake(validator_id: u32, amount: u128) {
        if TotalValidatorDelegateStakeBalance::<T>::get(validator_id) == 0
            || TotalValidatorDelegateStakeShares::<T>::get(validator_id) == 0
        {
            TotalValidatorDelegateStakeShares::<T>::mutate(validator_id, |mut n| {
                n.saturating_accrue(Self::MIN_LIQUIDITY)
            });
        };

        // -- increase total validator delegate stake
        TotalValidatorDelegateStakeBalance::<T>::mutate(validator_id, |mut n| {
            n.saturating_accrue(amount)
        });

        TotalDelegateStake::<T>::mutate(|mut n| n.saturating_accrue(amount));
    }
}
