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
    pub fn do_add_node_stake(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        subnet_node_id: u32,
        stake_to_be_added: u128,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            SubnetsData::<T>::contains_key(subnet_id),
            Error::<T>::InvalidSubnetId
        );

        // Resolve the validator that owns this subnet node, then ensure the caller is that
        // validator's coldkey. Only the owner is allowed to add stake.
        let validator_id = SubnetNodeValidatorId::<T>::try_get(subnet_id, subnet_node_id)
            .map_err(|_| Error::<T>::InvalidSubnetNodeId)?;

        let validator_coldkey = ValidatorColdkey::<T>::try_get(validator_id)
            .map_err(|_| Error::<T>::InvalidValidatorId)?;

        ensure!(coldkey == validator_coldkey, Error::<T>::NotKeyOwner);

        ensure!(stake_to_be_added != 0, Error::<T>::InvalidAmount);

        let stake_as_balance = Self::u128_to_balance(stake_to_be_added);

        let balance = match stake_as_balance {
            Some(b) => b,
            None => return Err(Error::<T>::CouldNotConvertToBalance.into()),
        };

        let node_stake_balance: u128 = NodeSubnetStake::<T>::get(&subnet_node_id, subnet_id);

        ensure!(
            node_stake_balance.saturating_add(stake_to_be_added)
                >= SubnetMinStakeBalance::<T>::get(subnet_id),
            Error::<T>::MinStakeNotReached
        );

        ensure!(
            node_stake_balance.saturating_add(stake_to_be_added)
                <= SubnetMaxStakeBalance::<T>::get(subnet_id),
            Error::<T>::MaxStakeReached
        );

        // --- Ensure the callers coldkey has enough stake to perform the transaction.
        ensure!(
            Self::can_remove_balance_from_coldkey_account(&coldkey, balance),
            Error::<T>::NotEnoughBalanceToStake
        );

        // to-do: add AddStakeRateLimit instead of universal rate limiter
        //        this allows peers to come in freely
        let block: u32 = Self::get_current_block_as_u32();
        ensure!(
            !Self::exceeds_tx_rate_limit(Self::get_last_tx_block(&coldkey), block),
            Error::<T>::TxRateLimitExceeded
        );

        // --- Ensure the remove operation from the coldkey is a success.
        ensure!(
            Self::remove_balance_from_coldkey_account(&coldkey, balance) == true,
            Error::<T>::BalanceWithdrawalError
        );

        Self::increase_node_stake(subnet_node_id, subnet_id, stake_to_be_added);

        // Set last block for rate limiting
        Self::set_last_tx_block(&coldkey, block);

        // Self::deposit_event(Event::StakeAdded(
        //     subnet_id,
        //     coldkey,
        //     hotkey,
        //     stake_to_be_added,
        // ));

        Ok(())
    }

    pub fn do_remove_node_stake(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        subnet_node_id: u32,
        // is_subnet_node: bool,
        stake_to_be_removed: u128,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        // Resolve the validator that owns this subnet node, then ensure the caller is that
        // validator's coldkey. Only the owner is allowed to add stake.
        let validator_id = SubnetNodeValidatorId::<T>::try_get(subnet_id, subnet_node_id)
            .map_err(|_| Error::<T>::InvalidSubnetNodeId)?;

        let validator_coldkey = ValidatorColdkey::<T>::try_get(validator_id)
            .map_err(|_| Error::<T>::InvalidValidatorId)?;

        ensure!(coldkey == validator_coldkey, Error::<T>::NotKeyOwner);

        // Check if node is currently active
        let is_subnet_node =
            if let Some(rep) = SubnetNodeReputation::<T>::get(subnet_id, subnet_node_id) {
                true
            } else {
                false
            };

        let node_stake_balance: u128 = NodeSubnetStake::<T>::get(subnet_node_id, subnet_id);

        ensure!(stake_to_be_removed > 0, Error::<T>::AmountZero);

        // --- Ensure that the stake amount to be removed is above zero.
        // --- Ensure that the account has enough stake to withdraw.
        ensure!(
            node_stake_balance >= stake_to_be_removed,
            Error::<T>::NotEnoughStakeToWithdraw
        );

        // if user is still a subnet node they must keep the required minimum balance
        if is_subnet_node {
            ensure!(
                node_stake_balance.saturating_sub(stake_to_be_removed)
                    >= SubnetMinStakeBalance::<T>::get(subnet_id),
                Error::<T>::MinStakeNotReached
            );
        } else if stake_to_be_removed >= node_stake_balance {
            Self::clean_validator_subnet_nodes(validator_id);
        }

        // --- Ensure that we can convert this u128 to a balance.
        match Self::u128_to_balance(stake_to_be_removed) {
            Some(b) => b,
            None => return Err(Error::<T>::CouldNotConvertToBalance.into()),
        };

        let block: u32 = Self::get_current_block_as_u32();
        ensure!(
            !Self::exceeds_tx_rate_limit(Self::get_last_tx_block(&coldkey), block),
            Error::<T>::TxRateLimitExceeded
        );

        // --- 7. We remove the balance from the subnet_node_id.
        Self::decrease_node_stake(subnet_node_id, subnet_id, stake_to_be_removed);

        // --- 9. We add the balancer to the coldkey.  If the above fails we will not credit this coldkey.
        Self::add_balance_to_unbonding_ledger(
            &coldkey,
            stake_to_be_removed,
            StakeCooldownEpochs::<T>::get() * T::EpochLength::get(),
            block,
        )
        .map_err(|e| e)?;

        // Set last block for rate limiting
        Self::set_last_tx_block(&coldkey, block);

        // Self::deposit_event(Event::StakeRemoved(
        //     subnet_id,
        //     coldkey,
        //     subnet_node_id,
        //     stake_to_be_removed,
        // ));

        Ok(())
    }

    pub fn increase_node_stake(subnet_node_id: u32, subnet_id: u32, amount: u128) {
        // -- increase account subnet staking balance
        NodeSubnetStake::<T>::mutate(subnet_node_id, subnet_id, |mut n| {
            n.saturating_accrue(amount)
        });

        // -- increase total subnet stake
        TotalSubnetStake::<T>::mutate(subnet_id, |mut n| n.saturating_accrue(amount));

        // -- increase total stake overall
        TotalStake::<T>::mutate(|mut n| n.saturating_accrue(amount));
    }

    pub fn decrease_node_stake(subnet_node_id: u32, subnet_id: u32, amount: u128) {
        // -- decrease account subnet staking balance
        NodeSubnetStake::<T>::mutate(subnet_node_id, subnet_id, |mut n| {
            n.saturating_reduce(amount)
        });

        // -- decrease total subnet stake
        TotalSubnetStake::<T>::mutate(subnet_id, |mut n| n.saturating_reduce(amount));

        // -- decrease total stake overall
        TotalStake::<T>::mutate(|mut n| n.saturating_reduce(amount));
    }
}
