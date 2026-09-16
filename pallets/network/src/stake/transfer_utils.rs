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
// Enables users to swap bidirectionally subnets <-> nodes

use super::*;

impl<T: Config> Pallet<T> {
    /// Swap stake from a validator to a subnet
    ///
    /// # Arguments
    ///
    /// * `from_subnet_id` - Subnet ID unstaking from.
    /// * `from_subnet_node_id` - Subnet node ID unstaking from.
    /// * `to_subnet_id` - Subnet ID staking to in relation to subnet node ID .
    /// * `delegate_stake_shares_to_swap` - Shares to remove (from node) to (to subnet) then be added as converted balance.
    ///
    #[frame_support::transactional]
    pub(crate) fn do_swap_from_validator_to_subnet(
        origin: T::RuntimeOrigin,
        from_validator_id: u32,
        to_subnet_id: u32,
        delegate_stake_shares_to_swap: u128,
        min_balance_out: u128,
        min_shares_out: u128,
        execute_before_block: u32,
    ) -> DispatchResult {
        let account_id: T::AccountId = ensure_signed(origin)?;

        // Perform removal of stake AND ensure success
        // Return the balance we removed
        let (result, balance, _) = Self::perform_do_remove_validator_delegate_stake(
            &account_id,
            from_validator_id,
            delegate_stake_shares_to_swap,
            min_balance_out,
            false,
        );

        result?;
        ensure!(balance > 0, Error::<T>::ZeroSwapBalance);

        let call = QueuedSwapCall::SwapToSubnetDelegateStake {
            account_id: account_id.clone(),
            to_subnet_id,
            balance,
            min_shares_out,
            execute_before_block,
        };

        Self::queue_swap(
            account_id.clone(),
            QueuedSwapSource::ValidatorDelegate,
            call,
        )?;

        // A swap debits the same stake position as an unstake. Record it in the shared limiter so
        // callers cannot bypass the configured transaction interval through the queued path.
        Self::set_last_tx_block(&account_id, Self::get_current_block_as_u32());

        // Self::deposit_event(Event::ValidatorToSubnetQueuedSwapCall {
        //     account_id: account_id.clone(),
        //     from_validator_id: from_validator_id,
        //     to_subnet_id: to_subnet_id,
        //     amount: balance,
        // });

        Ok(())
    }

    #[frame_support::transactional]
    pub(crate) fn do_swap_from_subnet_to_validator(
        origin: T::RuntimeOrigin,
        from_subnet_id: u32,
        to_validator_id: u32,
        delegate_stake_shares_to_swap: u128,
        min_balance_out: u128,
        min_shares_out: u128,
        execute_before_block: u32,
    ) -> DispatchResult {
        let account_id: T::AccountId = ensure_signed(origin)?;

        let (result, balance, _) = Self::perform_do_remove_subnet_delegate_stake(
            &account_id,
            from_subnet_id,
            delegate_stake_shares_to_swap,
            min_balance_out,
            false,
        );

        result?;
        ensure!(balance > 0, Error::<T>::ZeroSwapBalance);

        let call = QueuedSwapCall::SwapToValidatorDelegateStake {
            account_id: account_id.clone(),
            to_validator_id,
            balance,
            min_shares_out,
            execute_before_block,
        };

        Self::queue_swap(account_id.clone(), QueuedSwapSource::SubnetDelegate, call)?;

        Self::set_last_tx_block(&account_id, Self::get_current_block_as_u32());

        // Self::deposit_event(Event::SubnetToValidatorQueuedSwapCall {
        //     account_id: account_id,
        //     from_subnet_id: from_subnet_id,
        //     to_validator_id: to_validator_id,
        //     amount: balance,
        // });

        Ok(())
    }

    #[frame_support::transactional]
    pub(crate) fn do_swap_from_subnet_to_subnet(
        origin: T::RuntimeOrigin,
        from_subnet_id: u32,
        to_subnet_id: u32,
        delegate_stake_shares_to_swap: u128,
        min_balance_out: u128,
        min_shares_out: u128,
        execute_before_block: u32,
    ) -> DispatchResult {
        let account_id: T::AccountId = ensure_signed(origin)?;

        // Perform removal of stake AND ensure success
        // Return the balance we removed
        let (result, balance, _) = Self::perform_do_remove_subnet_delegate_stake(
            &account_id,
            from_subnet_id,
            delegate_stake_shares_to_swap,
            min_balance_out,
            false,
        );

        result?;
        ensure!(balance > 0, Error::<T>::ZeroSwapBalance);

        let call = QueuedSwapCall::SwapToSubnetDelegateStake {
            account_id: account_id.clone(),
            to_subnet_id,
            balance,
            min_shares_out,
            execute_before_block,
        };

        Self::queue_swap(account_id.clone(), QueuedSwapSource::SubnetDelegate, call)?;

        Self::set_last_tx_block(&account_id, Self::get_current_block_as_u32());

        // Self::deposit_event(Event::SubnetToSubnetQueuedSwapCall {
        //     account_id: account_id,
        //     from_subnet_id: from_subnet_id,
        //     to_validator_id: to_validator_id,
        //     amount: balance,
        // });

        Ok(())
    }

    #[frame_support::transactional]
    pub(crate) fn do_swap_from_validator_to_validator(
        origin: T::RuntimeOrigin,
        from_validator_id: u32,
        to_validator_id: u32,
        delegate_stake_shares_to_swap: u128,
        min_balance_out: u128,
        min_shares_out: u128,
        execute_before_block: u32,
    ) -> DispatchResult {
        let account_id: T::AccountId = ensure_signed(origin)?;

        // Perform removal of stake AND ensure success
        // Return the balance we removed
        let (result, balance, _) = Self::perform_do_remove_validator_delegate_stake(
            &account_id,
            from_validator_id,
            delegate_stake_shares_to_swap,
            min_balance_out,
            false,
        );

        result?;
        ensure!(balance > 0, Error::<T>::ZeroSwapBalance);

        let call = QueuedSwapCall::SwapToValidatorDelegateStake {
            account_id: account_id.clone(),
            to_validator_id,
            balance,
            min_shares_out,
            execute_before_block,
        };

        Self::queue_swap(
            account_id.clone(),
            QueuedSwapSource::ValidatorDelegate,
            call,
        )?;

        Self::set_last_tx_block(&account_id, Self::get_current_block_as_u32());

        // Self::deposit_event(Event::ValidatorToValidatorQueuedSwapCall {
        //     account_id: account_id,
        //     from_subnet_id: from_subnet_id,
        //     to_validator_id: to_validator_id,
        //     amount: balance,
        // });

        Ok(())
    }
}
