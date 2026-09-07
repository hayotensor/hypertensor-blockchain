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
// Enables accounts to delegate stake to subnets for a portion of emissions

use super::*;
use frame_support::pallet_prelude::DispatchError;
use sp_runtime::ArithmeticError;

impl<T: Config> Pallet<T> {
    /// Shares owned by an account in the subnet pool's current generation.
    pub fn current_account_subnet_delegate_stake_shares(
        account_id: &T::AccountId,
        subnet_id: u32,
    ) -> u128 {
        if AccountSubnetDelegateStakeGeneration::<T>::get(account_id, subnet_id)
            == SubnetDelegatePoolGeneration::<T>::get(subnet_id)
        {
            AccountSubnetDelegateStakeShares::<T>::get(account_id, subnet_id)
        } else {
            0
        }
    }

    fn set_current_account_subnet_delegate_stake_shares(
        account_id: &T::AccountId,
        subnet_id: u32,
        shares: u128,
    ) {
        if shares == 0 {
            AccountSubnetDelegateStakeShares::<T>::remove(account_id, subnet_id);
            AccountSubnetDelegateStakeGeneration::<T>::remove(account_id, subnet_id);
        } else {
            AccountSubnetDelegateStakeShares::<T>::insert(account_id, subnet_id, shares);
            AccountSubnetDelegateStakeGeneration::<T>::insert(
                account_id,
                subnet_id,
                SubnetDelegatePoolGeneration::<T>::get(subnet_id),
            );
        }
    }

    #[frame_support::transactional]
    pub(crate) fn do_add_subnet_delegate_stake(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        delegate_stake_to_be_added: u128,
        min_shares_out: u128,
    ) -> DispatchResult {
        let account_id: T::AccountId = ensure_signed(origin)?;

        let (result, credited_balance, credited_shares) =
            Self::perform_do_add_subnet_delegate_stake(
                &account_id,
                subnet_id,
                delegate_stake_to_be_added,
                min_shares_out,
                false,
            );

        result?;

        let block: u32 = Self::get_current_block_as_u32();

        // Set last block for rate limiting
        Self::set_last_tx_block(&account_id, block);

        Self::deposit_event(Event::SubnetDelegateStakeAdded {
            subnet_id,
            account_id,
            balance: credited_balance,
            shares_minted: credited_shares,
        });

        Ok(())
    }

    /// Add to the subnet delegate stake balance of a user
    ///
    /// # Arguments
    ///
    /// * `account_id` - Account adding to balance of subnet.
    /// * `subnet_id` - Subnet ID adding stake to.
    /// * `delegate_stake_to_be_added` - Balance to add or swap.
    /// * `swap` - If we are swapping between subnets or nodes.
    ///              - True: Don't remove balance from users account
    ///              - False: Check user balance is withdrawable and withdraw balance
    ///
    pub(crate) fn perform_do_add_subnet_delegate_stake(
        account_id: &T::AccountId,
        subnet_id: u32,
        delegate_stake_to_be_added: u128,
        min_shares_out: u128,
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

        let total_shares = TotalSubnetDelegateStakeShares::<T>::get(subnet_id);
        let total_balance = TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);
        let circulating_shares = TotalSubnetDelegateStakeCirculatingShares::<T>::get(subnet_id);
        if let Err(error) =
            Self::validate_delegate_pool_accounting(total_shares, total_balance, circulating_shares)
        {
            return (Err(error), 0, 0);
        }
        if let Err(error) = Self::preview_delegate_pool_deposit(
            delegate_stake_to_be_added,
            total_shares,
            total_balance,
            min_shares_out,
        ) {
            return (Err(error), 0, 0);
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

        match Self::handle_increase_account_delegate_stake_with_limit(
            account_id,
            subnet_id,
            delegate_stake_to_be_added,
            min_shares_out,
        ) {
            Ok((credited_balance, credited_shares)) => (Ok(()), credited_balance, credited_shares),
            Err(error) => (Err(error), 0, 0),
        }
    }

    /// Credit balance to an account's subnet delegate stake position.
    ///
    /// Returns the credited `(balance, shares)`. Validation completes before any storage is
    /// changed, including initialization of the pool's minimum liquidity.
    pub(crate) fn handle_increase_account_delegate_stake(
        account_id: &T::AccountId,
        subnet_id: u32,
        delegate_stake_to_be_added: u128,
    ) -> Result<(u128, u128), DispatchError> {
        Self::handle_increase_account_delegate_stake_with_limit(
            account_id,
            subnet_id,
            delegate_stake_to_be_added,
            1,
        )
    }

    pub(crate) fn handle_increase_account_delegate_stake_with_limit(
        account_id: &T::AccountId,
        subnet_id: u32,
        delegate_stake_to_be_added: u128,
        min_shares_out: u128,
    ) -> Result<(u128, u128), DispatchError> {
        ensure!(
            delegate_stake_to_be_added >= MinDelegateStakeDeposit::<T>::get(),
            Error::<T>::MinDelegateStakeDepositNotReached
        );

        let total_subnet_delegated_stake_shares =
            TotalSubnetDelegateStakeShares::<T>::get(subnet_id);
        let total_subnet_delegated_stake_balance =
            TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);
        let circulating_shares = TotalSubnetDelegateStakeCirculatingShares::<T>::get(subnet_id);
        Self::validate_delegate_pool_accounting(
            total_subnet_delegated_stake_shares,
            total_subnet_delegated_stake_balance,
            circulating_shares,
        )?;

        let (delegate_stake_to_be_added_as_shares, gross_shares) =
            Self::preview_delegate_pool_deposit(
                delegate_stake_to_be_added,
                total_subnet_delegated_stake_shares,
                total_subnet_delegated_stake_balance,
                min_shares_out,
            )?;

        // Compute every principal-bearing write before changing storage. A queued swap must not
        // report a complete credit if any destination balance or share counter would saturate.
        let account_shares =
            Self::current_account_subnet_delegate_stake_shares(account_id, subnet_id)
                .checked_add(delegate_stake_to_be_added_as_shares)
                .ok_or(ArithmeticError::Overflow)?;
        let total_balance = total_subnet_delegated_stake_balance
            .checked_add(delegate_stake_to_be_added)
            .ok_or(ArithmeticError::Overflow)?;
        let total_shares = total_subnet_delegated_stake_shares
            .checked_add(gross_shares)
            .ok_or(ArithmeticError::Overflow)?;
        let next_circulating_shares = circulating_shares
            .checked_add(delegate_stake_to_be_added_as_shares)
            .ok_or(ArithmeticError::Overflow)?;
        let total_delegate_stake = TotalDelegateStake::<T>::get()
            .checked_add(delegate_stake_to_be_added)
            .ok_or(ArithmeticError::Overflow)?;
        Self::validate_delegate_pool_accounting(
            total_shares,
            total_balance,
            next_circulating_shares,
        )?;

        let next_flow = if SubnetsData::<T>::contains_key(subnet_id) {
            let flow_delta = i128::try_from(delegate_stake_to_be_added)
                .map_err(|_| ArithmeticError::Overflow)?;
            Some(
                SubnetNetFlow::<T>::get(subnet_id)
                    .checked_add(flow_delta)
                    .ok_or(ArithmeticError::Overflow)?,
            )
        } else {
            None
        };

        Self::set_current_account_subnet_delegate_stake_shares(
            account_id,
            subnet_id,
            account_shares,
        );
        TotalSubnetDelegateStakeBalance::<T>::insert(subnet_id, total_balance);
        TotalSubnetDelegateStakeShares::<T>::insert(subnet_id, total_shares);
        TotalSubnetDelegateStakeCirculatingShares::<T>::insert(subnet_id, next_circulating_shares);
        TotalDelegateStake::<T>::put(total_delegate_stake);

        if let Some(flow) = next_flow {
            SubnetNetFlow::<T>::insert(subnet_id, flow);
        }

        Ok((
            delegate_stake_to_be_added,
            delegate_stake_to_be_added_as_shares,
        ))
    }

    #[frame_support::transactional]
    pub(crate) fn do_remove_delegate_stake(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        delegate_stake_shares_to_be_removed: u128,
        min_balance_out: u128,
    ) -> DispatchResult {
        let account_id: T::AccountId = ensure_signed(origin)?;

        let (result, delegate_stake_to_be_removed, shares_burned) =
            Self::perform_do_remove_subnet_delegate_stake(
                &account_id,
                subnet_id,
                delegate_stake_shares_to_be_removed,
                min_balance_out,
                true,
            );

        result?;

        let block: u32 = Self::get_current_block_as_u32();

        // Set last block for rate limiting
        Self::set_last_tx_block(&account_id, block);

        Self::deposit_event(Event::SubnetDelegateStakeRemoved {
            subnet_id,
            account_id,
            balance: delegate_stake_to_be_removed,
            shares_burned,
        });

        Ok(())
    }

    /// Remove the subnet delegate stake balance of a user
    ///
    /// # Arguments
    ///
    /// * `account_id` - Account removing balance from subnet.
    /// * `subnet_id` - Subnet ID removing stake from.
    /// * `delegate_stake_shares_to_be_removed` - Shares of pool to remove.
    /// * `add_to_ledger` - If we are unstaking from network and not swapping between staking options.
    ///              - True: Unstake user to unstaking ledger.
    ///              - False: Don't add balance to unstaking ledger.
    ///
    pub(crate) fn perform_do_remove_subnet_delegate_stake(
        account_id: &T::AccountId,
        subnet_id: u32,
        delegate_stake_shares_to_be_removed: u128,
        min_balance_out: u128,
        add_to_ledger: bool,
    ) -> (DispatchResult, u128, u128) {
        // --- Ensure that the delegate_stake amount to be removed is above zero.
        if delegate_stake_shares_to_be_removed == 0 {
            return (Err(Error::<T>::SharesZero.into()), 0, 0);
        }
        if min_balance_out == 0 {
            return (Err(Error::<T>::InvalidStakeMinimumOutput.into()), 0, 0);
        }

        let account_delegate_stake_shares =
            Self::current_account_subnet_delegate_stake_shares(account_id, subnet_id);

        // --- Ensure that the account has enough delegate_stake to withdraw.
        if account_delegate_stake_shares < delegate_stake_shares_to_be_removed {
            return (Err(Error::<T>::NotEnoughStakeToWithdraw.into()), 0, 0);
        }

        let total_subnet_delegated_stake_shares =
            TotalSubnetDelegateStakeShares::<T>::get(subnet_id);
        let total_subnet_delegated_stake_balance =
            TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);
        let circulating_shares = TotalSubnetDelegateStakeCirculatingShares::<T>::get(subnet_id);
        if let Err(error) = Self::validate_delegate_pool_accounting(
            total_subnet_delegated_stake_shares,
            total_subnet_delegated_stake_balance,
            circulating_shares,
        ) {
            return (Err(error), 0, 0);
        }

        // Quote both the account debit and the supply-side burn before changing storage.
        let (delegate_stake_to_be_removed, supply_shares_to_be_burned) =
            match Self::preview_delegate_pool_redemption(
                delegate_stake_shares_to_be_removed,
                total_subnet_delegated_stake_shares,
                total_subnet_delegated_stake_balance,
                min_balance_out,
            ) {
                Ok(quote) => quote,
                Err(error) => return (Err(error.into()), 0, 0),
            };

        // --- Ensure that we can convert this u128 to a balance.
        // Redunant
        let _delegate_stake_to_be_added_as_currency =
            match Self::u128_to_balance(delegate_stake_to_be_removed) {
                Some(b) => b,
                None => return (Err(Error::<T>::CouldNotConvertToBalance.into()), 0, 0),
            };

        let block: u32 = Self::get_current_block_as_u32();
        if Self::exceeds_tx_rate_limit(Self::get_last_tx_block(&account_id), block) {
            return (Err(Error::<T>::TxRateLimitExceeded.into()), 0, 0);
        }

        // --- We remove the shares from the account and balance from the pool
        if let Err(error) = Self::decrease_account_delegate_stake_with_supply_burn(
            &account_id,
            subnet_id,
            delegate_stake_to_be_removed,
            delegate_stake_shares_to_be_removed,
            supply_shares_to_be_burned,
        ) {
            return (Err(error), 0, 0);
        }

        // Keep the source debit and ledger credit atomic in the unstake dispatch transaction.
        if add_to_ledger {
            let cooldown_blocks =
                match DelegateStakeCooldownEpochs::<T>::get().checked_mul(T::EpochLength::get()) {
                    Some(blocks) => blocks,
                    None => return (Err(ArithmeticError::Overflow.into()), 0, 0),
                };
            if let Err(error) = Self::add_balance_to_unbonding_ledger(
                &account_id,
                delegate_stake_to_be_removed,
                cooldown_blocks,
                block,
                UnbondingSource::Network,
            ) {
                return (Err(error), 0, 0);
            }
        }

        (
            Ok(()),
            delegate_stake_to_be_removed,
            delegate_stake_shares_to_be_removed,
        )
    }

    #[frame_support::transactional]
    pub(crate) fn do_transfer_delegate_stake(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
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

        let account_delegate_stake_shares =
            Self::current_account_subnet_delegate_stake_shares(&account_id, subnet_id);

        ensure!(
            account_delegate_stake_shares >= delegate_stake_shares_to_transfer,
            Error::<T>::NotEnoughStakeToWithdraw
        );

        let total_subnet_delegated_stake_shares =
            TotalSubnetDelegateStakeShares::<T>::get(subnet_id);
        let total_subnet_delegated_stake_balance =
            TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);
        let circulating_shares = TotalSubnetDelegateStakeCirculatingShares::<T>::get(subnet_id);
        Self::validate_delegate_pool_accounting(
            total_subnet_delegated_stake_shares,
            total_subnet_delegated_stake_balance,
            circulating_shares,
        )?;

        // --- Get accounts current balance
        let delegate_stake_to_be_transferred = Self::try_convert_to_balance(
            delegate_stake_shares_to_transfer,
            total_subnet_delegated_stake_shares,
            total_subnet_delegated_stake_balance,
        )?;

        // --- Ensure transfer balance is greater than the min
        ensure!(
            delegate_stake_to_be_transferred >= MinDelegateStakeDeposit::<T>::get(),
            Error::<T>::MinDelegateStakeDepositNotReached
        );

        let source_shares = account_delegate_stake_shares
            .checked_sub(delegate_stake_shares_to_transfer)
            .ok_or(ArithmeticError::Underflow)?;
        let destination_shares =
            Self::current_account_subnet_delegate_stake_shares(&to_account_id, subnet_id)
                .checked_add(delegate_stake_shares_to_transfer)
                .ok_or(ArithmeticError::Overflow)?;

        // A transfer changes ownership only; pool supply, assets, and circulating shares stay
        // exactly constant.
        Self::set_current_account_subnet_delegate_stake_shares(
            &account_id,
            subnet_id,
            source_shares,
        );
        Self::set_current_account_subnet_delegate_stake_shares(
            &to_account_id,
            subnet_id,
            destination_shares,
        );

        Ok(())
    }

    fn decrease_account_delegate_stake_with_supply_burn(
        account_id: &T::AccountId,
        subnet_id: u32,
        amount: u128,
        account_shares_to_remove: u128,
        supply_shares_to_burn: u128,
    ) -> DispatchResult {
        let current_balance = TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);
        let current_total_shares = TotalSubnetDelegateStakeShares::<T>::get(subnet_id);
        let current_circulating_shares =
            TotalSubnetDelegateStakeCirculatingShares::<T>::get(subnet_id);
        Self::validate_delegate_pool_accounting(
            current_total_shares,
            current_balance,
            current_circulating_shares,
        )?;

        let account_shares =
            Self::current_account_subnet_delegate_stake_shares(account_id, subnet_id)
                .checked_sub(account_shares_to_remove)
                .ok_or(ArithmeticError::Underflow)?;
        let total_balance = current_balance
            .checked_sub(amount)
            .ok_or(ArithmeticError::Underflow)?;
        let total_shares = current_total_shares
            .checked_sub(supply_shares_to_burn)
            .ok_or(ArithmeticError::Underflow)?;
        let circulating_shares = current_circulating_shares
            .checked_sub(account_shares_to_remove)
            .ok_or(ArithmeticError::Underflow)?;
        let total_delegate_stake = TotalDelegateStake::<T>::get()
            .checked_sub(amount)
            .ok_or(ArithmeticError::Underflow)?;
        let next_generation = if total_balance == 0 {
            Some(
                SubnetDelegatePoolGeneration::<T>::get(subnet_id)
                    .checked_add(1)
                    .ok_or(ArithmeticError::Overflow)?,
            )
        } else {
            Self::validate_delegate_pool_accounting(
                total_shares,
                total_balance,
                circulating_shares,
            )?;
            None
        };
        let next_flow = if SubnetsData::<T>::contains_key(subnet_id) {
            let delta = i128::try_from(amount).map_err(|_| ArithmeticError::Overflow)?;
            Some(
                SubnetNetFlow::<T>::get(subnet_id)
                    .checked_sub(delta)
                    .ok_or(ArithmeticError::Underflow)?,
            )
        } else {
            None
        };

        if let Some(new_generation) = next_generation {
            let old_generation = SubnetDelegatePoolGeneration::<T>::get(subnet_id);
            TotalSubnetDelegateStakeBalance::<T>::remove(subnet_id);
            TotalSubnetDelegateStakeShares::<T>::remove(subnet_id);
            TotalSubnetDelegateStakeCirculatingShares::<T>::remove(subnet_id);
            SubnetDelegatePoolGeneration::<T>::insert(subnet_id, new_generation);
            Self::deposit_event(Event::SubnetDelegatePoolReset {
                subnet_id,
                old_generation,
                new_generation,
                invalidated_shares: circulating_shares,
            });
        } else {
            Self::set_current_account_subnet_delegate_stake_shares(
                account_id,
                subnet_id,
                account_shares,
            );
            TotalSubnetDelegateStakeBalance::<T>::insert(subnet_id, total_balance);
            TotalSubnetDelegateStakeShares::<T>::insert(subnet_id, total_shares);
            TotalSubnetDelegateStakeCirculatingShares::<T>::insert(subnet_id, circulating_shares);
        }
        TotalDelegateStake::<T>::put(total_delegate_stake);
        if let Some(flow) = next_flow {
            SubnetNetFlow::<T>::insert(subnet_id, flow);
        }
        Ok(())
    }

    /// Rewards are deposited here from `rewards.rs`.
    /// Note: We don't count SubnetNetFlow here
    pub(crate) fn do_increase_delegate_stake(subnet_id: u32, amount: u128) -> DispatchResult {
        if amount == 0 {
            return Ok(());
        }
        let current_balance = TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);
        let current_shares = TotalSubnetDelegateStakeShares::<T>::get(subnet_id);
        let circulating_shares = TotalSubnetDelegateStakeCirculatingShares::<T>::get(subnet_id);
        Self::validate_delegate_pool_accounting(
            current_shares,
            current_balance,
            circulating_shares,
        )?;
        ensure!(
            Self::delegate_pool_has_circulating_shares(circulating_shares),
            Error::<T>::DelegatePoolNotActive
        );

        let next_balance = current_balance
            .checked_add(amount)
            .ok_or(ArithmeticError::Overflow)?;
        let next_total = TotalDelegateStake::<T>::get()
            .checked_add(amount)
            .ok_or(ArithmeticError::Overflow)?;
        TotalSubnetDelegateStakeBalance::<T>::insert(subnet_id, next_balance);
        TotalDelegateStake::<T>::put(next_total);
        Ok(())
    }

    pub fn convert_account_shares_to_balance(account_id: &T::AccountId, subnet_id: u32) -> u128 {
        let account_delegate_stake_shares =
            Self::current_account_subnet_delegate_stake_shares(account_id, subnet_id);
        if account_delegate_stake_shares == 0 {
            return 0;
        }
        let total_subnet_delegated_stake_shares =
            TotalSubnetDelegateStakeShares::<T>::get(subnet_id);
        let total_subnet_delegated_stake_balance =
            TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);
        let circulating_shares = TotalSubnetDelegateStakeCirculatingShares::<T>::get(subnet_id);
        if Self::validate_delegate_pool_accounting(
            total_subnet_delegated_stake_shares,
            total_subnet_delegated_stake_balance,
            circulating_shares,
        )
        .is_err()
        {
            return 0;
        }

        // --- Get accounts current balance
        Self::convert_to_balance(
            account_delegate_stake_shares,
            total_subnet_delegated_stake_shares,
            total_subnet_delegated_stake_balance,
        )
    }
}
