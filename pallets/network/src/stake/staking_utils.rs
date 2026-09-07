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
use frame_support::storage::{with_transaction, TransactionOutcome};
use frame_support::traits::Imbalance;
use sp_core::U256;
use sp_runtime::traits::CheckedAdd;
use sp_runtime::ArithmeticError;

impl<T: Config> Pallet<T> {
    /// Shares permanently locked by the first depositor of each delegate pool.
    ///
    /// This equals the virtual-share offset so the initial depositor pays exactly one atomic unit
    /// of balance to initialize the pool instead of receiving the entire initial exchange rate.
    pub const DELEGATE_POOL_MIN_LIQUIDITY: u128 = 1_000_000_000;

    /// Minimum user-owned shares minted by a deposit. This bounds the real-valued conversion loss
    /// below `deposit / 1e9`; because balances are integral, a small deposit can still lose one
    /// atomic unit. Caller-provided minimum output remains the authoritative transaction bound.
    pub const DELEGATE_POOL_MIN_MINTED_SHARES: u128 = 1_000_000_000;

    /// Immutable ERC-4626-style virtual deposit with a nine-decimal share precision offset.
    pub const DELEGATE_POOL_VIRTUAL_SHARES: u128 = 1_000_000_000;
    pub const DELEGATE_POOL_VIRTUAL_BALANCE: u128 = 1;

    #[frame_support::transactional]
    pub(crate) fn add_balance_to_unbonding_ledger(
        coldkey: &T::AccountId,
        amount: u128,
        cooldown_blocks: u32,
        block: u32,
        source: UnbondingSource,
    ) -> DispatchResult {
        ensure!(amount > 0, Error::<T>::AmountZero);
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
            let mut next_entry = ledger.get(&claim_block).copied().unwrap_or_default();
            match source {
                UnbondingSource::Network => {
                    next_entry.network = next_entry
                        .network
                        .checked_add(amount)
                        .ok_or(ArithmeticError::Overflow)?;
                }
                UnbondingSource::Overwatch => {
                    next_entry.overwatch = next_entry
                        .overwatch
                        .checked_add(amount)
                        .ok_or(ArithmeticError::Overflow)?;
                }
            }

            // A ledger entry is claimed as one currency credit. Validate the complete merged
            // value now so independently valid network/Overwatch fields cannot form an entry that
            // can never be claimed.
            let merged_amount = next_entry
                .network
                .checked_add(next_entry.overwatch)
                .ok_or(ArithmeticError::Overflow)?;
            Self::u128_to_balance(merged_amount).ok_or(Error::<T>::CouldNotConvertToBalance)?;

            ledger.insert(claim_block, next_entry);
            Ok(())
        })?;

        if let Some(total) = next_total_network_unbonding {
            TotalNetworkUnbondingBalance::<T>::put(total);
        }

        Ok(())
    }

    pub(crate) fn prepare_unbonding_ledger_entry(
        coldkey: &T::AccountId,
        cooldown_blocks: u32,
        block: u32,
    ) -> Result<u32, DispatchError> {
        let claim_block = block
            .checked_add(cooldown_blocks)
            .ok_or(ArithmeticError::Overflow)?;
        let max_unbondings = MaxUnbondings::<T>::get();
        let unbondings = StakeUnbondingLedger::<T>::get(coldkey);

        if unbondings.contains_key(&claim_block) {
            return Ok(claim_block);
        }

        // Claims are deliberately explicit. Calling the O(MaxUnbondings) claim loop from a
        // fixed-weight removal would undercharge the removal and make its execution cost depend on
        // unrelated matured entries.
        ensure!(
            (unbondings.len() as u32) < max_unbondings,
            Error::<T>::MaxUnlockingsReached
        );

        Ok(claim_block)
    }

    pub(crate) fn do_claim_unbondings(coldkey: &T::AccountId) -> u32 {
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

            if Self::deposit_balance_exact(coldkey, stake_to_be_added_as_currency).is_err() {
                continue;
            }

            TotalNetworkUnbondingBalance::<T>::put(total_network_unbonding);
            unbondings_copy.remove(&unbonding_block);
            successful_unbondings += 1;
        }

        if unbondings.len() != unbondings_copy.len() {
            StakeUnbondingLedger::<T>::insert(&coldkey, unbondings_copy);
        }

        // Failed/expired queued swaps use an unbounded-by-entries scalar refund so an account with
        // a full unbonding ledger cannot permanently pin the global queue. Claim it through the
        // same user entry point, and clear accounting only after an exact currency credit.
        let swap_refund = QueuedSwapRefundBalance::<T>::get(coldkey);
        if swap_refund != 0 {
            let refund_as_currency = Self::u128_to_balance(swap_refund);
            let remaining_total = TotalQueuedSwapRefundBalance::<T>::get().checked_sub(swap_refund);

            if let (Some(refund_as_currency), Some(remaining_total)) =
                (refund_as_currency, remaining_total)
            {
                let can_create_account = !T::Currency::total_balance(coldkey).is_zero()
                    || refund_as_currency >= T::Currency::minimum_balance();
                if can_create_account {
                    if Self::deposit_balance_exact(coldkey, refund_as_currency).is_ok() {
                        QueuedSwapRefundBalance::<T>::remove(coldkey);
                        TotalQueuedSwapRefundBalance::<T>::put(remaining_total);
                        successful_unbondings = successful_unbondings.saturating_add(1);
                    }
                }
            }
        }
        successful_unbondings
    }

    /// Credit exactly `amount` or roll the currency mutation back. Some `Currency`
    /// implementations saturate `deposit_creating`; retaining the liability after a partial credit
    /// would otherwise let the same principal be claimed twice.
    pub(crate) fn deposit_balance_exact(
        coldkey: &T::AccountId,
        amount: <<T as pallet::Config>::Currency as Currency<
            <T as frame_system::Config>::AccountId,
        >>::Balance,
    ) -> DispatchResult {
        // `deposit_creating` reports the amount credited to the account through its imbalance,
        // but dropping that imbalance may still saturate a Currency implementation's issuance
        // counter. Require enough issuance headroom before touching the account, then verify that
        // dropping the imbalance changed issuance by exactly the same amount.
        let expected_total_issuance = T::Currency::total_issuance()
            .checked_add(&amount)
            .ok_or(ArithmeticError::Overflow)?;

        with_transaction::<(), DispatchError, _>(|| {
            let credited = T::Currency::deposit_creating(coldkey, amount);
            if credited.peek() != amount {
                drop(credited);
                return TransactionOutcome::Rollback(Err(ArithmeticError::Overflow.into()));
            }

            drop(credited);
            if T::Currency::total_issuance() != expected_total_issuance {
                return TransactionOutcome::Rollback(Err(ArithmeticError::Overflow.into()));
            }

            TransactionOutcome::Commit(Ok(()))
        })
    }

    pub(crate) fn can_remove_balance_from_coldkey_account(
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

    pub(crate) fn remove_balance_from_coldkey_account(
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

    pub(crate) fn add_balance_to_coldkey_account(
        coldkey: &T::AccountId,
        amount: <<T as pallet::Config>::Currency as Currency<
            <T as frame_system::Config>::AccountId,
        >>::Balance,
    ) -> DispatchResult {
        Self::deposit_balance_exact(coldkey, amount)
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
    pub fn try_convert_to_shares(
        balance: u128,
        total_shares: u128,
        total_balance: u128,
    ) -> Result<u128, ArithmeticError> {
        // Assets without shares cannot be assigned fairly. This state is unreachable through the
        // staking API and must fail closed if corrupt storage or a faulty migration creates it.
        if (total_shares == 0 && total_balance != 0)
            || (total_shares != 0 && total_shares < Self::DELEGATE_POOL_MIN_LIQUIDITY)
        {
            return Err(ArithmeticError::Underflow);
        }
        let balance = U256::from(balance);
        let total_shares = U256::from(total_shares)
            .checked_add(U256::from(Self::DELEGATE_POOL_VIRTUAL_SHARES))
            .ok_or(ArithmeticError::Overflow)?;
        let total_balance = U256::from(total_balance)
            .checked_add(U256::from(Self::DELEGATE_POOL_VIRTUAL_BALANCE))
            .ok_or(ArithmeticError::Overflow)?;

        let result = Self::checked_mul_div(balance, total_shares, total_balance)
            .ok_or(ArithmeticError::Overflow)?;
        result.try_into().map_err(|_| ArithmeticError::Overflow)
    }

    /// Convert balance to shares, rounding toward positive infinity.
    ///
    /// This is used only for the pool's total-supply accounting. Accounts are credited with the
    /// floor quote, while a non-zero remainder is retained as non-circulating pool liquidity.
    pub fn try_convert_to_shares_round_up(
        balance: u128,
        total_shares: u128,
        total_balance: u128,
    ) -> Result<u128, ArithmeticError> {
        if (total_shares == 0 && total_balance != 0)
            || (total_shares != 0 && total_shares < Self::DELEGATE_POOL_MIN_LIQUIDITY)
        {
            return Err(ArithmeticError::Underflow);
        }

        let numerator = U256::from(balance)
            .checked_mul(
                U256::from(total_shares)
                    .checked_add(U256::from(Self::DELEGATE_POOL_VIRTUAL_SHARES))
                    .ok_or(ArithmeticError::Overflow)?,
            )
            .ok_or(ArithmeticError::Overflow)?;
        let denominator = U256::from(total_balance)
            .checked_add(U256::from(Self::DELEGATE_POOL_VIRTUAL_BALANCE))
            .ok_or(ArithmeticError::Overflow)?;
        let quotient = numerator
            .checked_div(denominator)
            .ok_or(ArithmeticError::Underflow)?;
        let rounded = if numerator % denominator == U256::zero() {
            quotient
        } else {
            quotient
                .checked_add(U256::one())
                .ok_or(ArithmeticError::Overflow)?
        };

        rounded.try_into().map_err(|_| ArithmeticError::Overflow)
    }

    /// Infallible quote helper for RPCs and tests. State-changing paths use the checked variant and
    /// therefore never turn a failed conversion into an artificial maximum balance.
    pub fn convert_to_shares(balance: u128, total_shares: u128, total_balance: u128) -> u128 {
        Self::try_convert_to_shares(balance, total_shares, total_balance).unwrap_or(0)
    }

    /// Convert vault shares to TENSOR balance
    ///
    /// # Arguments
    ///
    /// * `shares` - Amount of shares to convert to TENSOR.
    /// * `total_shares` - Total shares in the vault.
    /// * `total_balance` - Total balance of TENSOR in the vault.
    ///
    pub fn try_convert_to_balance(
        shares: u128,
        total_shares: u128,
        total_balance: u128,
    ) -> Result<u128, ArithmeticError> {
        if (total_shares == 0 && total_balance != 0)
            || (total_shares != 0 && total_shares < Self::DELEGATE_POOL_MIN_LIQUIDITY)
        {
            return Err(ArithmeticError::Underflow);
        }
        let shares = U256::from(shares);
        let total_balance = U256::from(total_balance)
            .checked_add(U256::from(Self::DELEGATE_POOL_VIRTUAL_BALANCE))
            .ok_or(ArithmeticError::Overflow)?;
        let total_shares = U256::from(total_shares)
            .checked_add(U256::from(Self::DELEGATE_POOL_VIRTUAL_SHARES))
            .ok_or(ArithmeticError::Overflow)?;

        let result = Self::checked_mul_div(shares, total_balance, total_shares)
            .ok_or(ArithmeticError::Overflow)?;
        result.try_into().map_err(|_| ArithmeticError::Overflow)
    }

    pub fn convert_to_balance(shares: u128, total_shares: u128, total_balance: u128) -> u128 {
        Self::try_convert_to_balance(shares, total_shares, total_balance).unwrap_or(0)
    }

    /// Validate the storage-level shape of a delegate pool.
    ///
    /// A virgin pool is exactly `(shares, assets) == (0, 0)`. An initialized generation always
    /// has positive assets and at least the permanently locked minimum-liquidity shares. A total
    /// loss is represented by advancing the generation and returning to the virgin shape.
    pub fn validate_delegate_pool_state(total_shares: u128, total_balance: u128) -> DispatchResult {
        if total_shares == 0 {
            ensure!(
                total_balance == 0,
                Error::<T>::DelegatePoolInvariantViolation
            );
        } else {
            ensure!(
                total_balance != 0 && total_shares >= Self::DELEGATE_POOL_MIN_LIQUIDITY,
                Error::<T>::DelegatePoolInvariantViolation
            );
        }
        Ok(())
    }

    /// Validate total, circulating, and permanently locked shares as one accounting unit.
    pub fn validate_delegate_pool_accounting(
        total_shares: u128,
        total_balance: u128,
        circulating_shares: u128,
    ) -> DispatchResult {
        Self::validate_delegate_pool_state(total_shares, total_balance)?;
        if total_shares == 0 {
            ensure!(
                circulating_shares == 0,
                Error::<T>::DelegatePoolInvariantViolation
            );
        } else {
            let maximum_circulating = total_shares
                .checked_sub(Self::DELEGATE_POOL_MIN_LIQUIDITY)
                .ok_or(Error::<T>::DelegatePoolInvariantViolation)?;
            ensure!(
                circulating_shares <= maximum_circulating,
                Error::<T>::DelegatePoolInvariantViolation
            );
        }
        Ok(())
    }

    /// Whether an initialized pool has any shares owned by delegators rather than only its locked
    /// minimum liquidity.
    pub fn delegate_pool_has_circulating_shares(circulating_shares: u128) -> bool {
        circulating_shares != 0
    }

    /// Quote the user-owned and gross shares for a deposit and enforce both protocol and caller
    /// loss bounds. On an empty pool, `gross - user` is locked forever as dead liquidity.
    pub fn preview_delegate_pool_deposit(
        balance: u128,
        total_shares: u128,
        total_balance: u128,
        min_shares_out: u128,
    ) -> Result<(u128, u128), DispatchError> {
        ensure!(balance > 0, Error::<T>::AmountZero);
        ensure!(min_shares_out > 0, Error::<T>::InvalidStakeMinimumOutput);
        Self::validate_delegate_pool_state(total_shares, total_balance)?;

        let floor_shares = Self::try_convert_to_shares(balance, total_shares, total_balance)?;
        let gross_shares =
            Self::try_convert_to_shares_round_up(balance, total_shares, total_balance)?;
        let locked_shares = if total_shares == 0 {
            Self::DELEGATE_POOL_MIN_LIQUIDITY
        } else {
            0
        };
        let user_shares = floor_shares
            .checked_sub(locked_shares)
            .ok_or(Error::<T>::CouldNotConvertToShares)?;
        let required_shares = min_shares_out.max(Self::DELEGATE_POOL_MIN_MINTED_SHARES);
        ensure!(
            user_shares >= required_shares,
            Error::<T>::StakeSlippageExceeded
        );

        Ok((user_shares, gross_shares))
    }

    /// Quote a share redemption and the smaller amount removed from total supply.
    ///
    /// The account relinquishes `shares`, receives the floor asset quote, and total supply is
    /// reduced by the floor share value of those paid assets. Any share remainder becomes
    /// non-circulating, so a user conversion can never improve the exchange rate for an incumbent.
    pub fn preview_delegate_pool_redemption(
        shares: u128,
        total_shares: u128,
        total_balance: u128,
        min_balance_out: u128,
    ) -> Result<(u128, u128), DispatchError> {
        ensure!(shares > 0, Error::<T>::SharesZero);
        ensure!(min_balance_out > 0, Error::<T>::InvalidStakeMinimumOutput);
        Self::validate_delegate_pool_state(total_shares, total_balance)?;

        let balance = Self::try_convert_to_balance(shares, total_shares, total_balance)?;
        ensure!(balance > 0, Error::<T>::CouldNotConvertToBalance);
        ensure!(
            balance >= min_balance_out,
            Error::<T>::StakeSlippageExceeded
        );

        let supply_shares_burned =
            Self::try_convert_to_shares(balance, total_shares, total_balance)?;
        ensure!(
            supply_shares_burned <= shares,
            Error::<T>::DelegatePoolInvariantViolation
        );

        Ok((balance, supply_shares_burned))
    }
}
