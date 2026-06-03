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
use frame_support::pallet_prelude::DispatchError;
use frame_support::pallet_prelude::Weight;

impl<T: Config> Pallet<T> {
    pub fn do_update_validator_delegate_reward_rate(
        validator_id: u32,
        new_delegate_reward_rate: u128,
    ) -> DispatchResult {
        let block: u32 = Self::get_current_block_as_u32();
        let max_reward_rate_decrease = MaxRewardRateDecrease::<T>::get();
        let reward_rate_update_period = NodeRewardRateUpdatePeriod::<T>::get();

        // --- Ensure rate doesn't surpass 100% and MaxDelegateStakePercentage
        ensure!(
            new_delegate_reward_rate <= Self::percentage_factor_as_u128()
                && new_delegate_reward_rate <= MaxDelegateStakePercentage::<T>::get(),
            Error::<T>::InvalidDelegateRewardRate
        );

        ValidatorsData::<T>::try_mutate_exists(validator_id, |maybe_params| -> DispatchResult {
            Self::perform_update_validator_delegate_reward_rate(
                maybe_params,
                validator_id,
                block,
                new_delegate_reward_rate,
                reward_rate_update_period,
                max_reward_rate_decrease,
            )
        })?;

        Ok(())
    }

    fn perform_update_validator_delegate_reward_rate(
        maybe_params: &mut Option<ValidatorData<T::AccountId>>,
        validator_id: u32,
        block: u32,
        new_delegate_reward_rate: u128,
        reward_rate_update_period: u32,
        max_reward_rate_decrease: u128,
    ) -> DispatchResult {
        let params = maybe_params
            .as_mut()
            .ok_or(Error::<T>::InvalidSubnetNodeId)?;
        let curr_delegate_reward_rate = params.delegate_reward_rate;

        // --- Ensure rate change surpasses minimum update period
        ensure!(
            block - params.last_delegate_reward_rate_update >= reward_rate_update_period,
            Error::<T>::MaxRewardRateUpdates
        );

        // --- Ensure rate is being updated redundantly
        ensure!(
            new_delegate_reward_rate != curr_delegate_reward_rate,
            Error::<T>::NoDelegateRewardRateChange
        );

        let mut delegate_reward_rate = params.delegate_reward_rate;

        if new_delegate_reward_rate > curr_delegate_reward_rate {
            // Freely increase reward rate
            delegate_reward_rate = new_delegate_reward_rate;
        } else {
            // Ensure reward rate decrease doesn't surpass max rate of change
            let delta = curr_delegate_reward_rate - new_delegate_reward_rate;
            ensure!(
                delta <= max_reward_rate_decrease,
                Error::<T>::SurpassesMaxRewardRateDecrease
            );
            delegate_reward_rate = new_delegate_reward_rate
        }

        params.last_delegate_reward_rate_update = block;
        params.delegate_reward_rate = delegate_reward_rate;

        Self::deposit_event(Event::ValidatorUpdateDelegateRewardRate {
            validator_id,
            delegate_reward_rate: new_delegate_reward_rate,
        });

        Ok(())
    }

    pub fn do_update_validator_delegate_account(
        validator_id: u32,
        validator_coldkey: T::AccountId,
        delegate_account_id: Option<T::AccountId>,
        delegate_rate: Option<u128>,
    ) -> DispatchResult {
        ValidatorsData::<T>::try_mutate_exists(validator_id, |maybe_params| -> DispatchResult {
            Self::perform_update_validator_delegate_account(
                validator_id,
                validator_coldkey,
                maybe_params,
                delegate_account_id,
                delegate_rate,
            )
        })?;

        Ok(())
    }

    fn perform_update_validator_delegate_account(
        validator_id: u32,
        validator_coldkey: T::AccountId,
        maybe_params: &mut Option<ValidatorData<T::AccountId>>,
        delegate_account_id: Option<T::AccountId>,
        delegate_rate: Option<u128>,
    ) -> DispatchResult {
        let params = maybe_params
            .as_mut()
            .ok_or(Error::<T>::InvalidValidatorId)?;

        ensure!(
            delegate_account_id.is_some() || delegate_rate.is_some(),
            Error::<T>::InvalidDelegateAccountParameters
        );

        if delegate_account_id.is_some() || delegate_rate.is_some() {
            let account_id = if let Some(id) = delegate_account_id {
                id
            } else if let Some(existing_delegate_account) = &params.delegate_account {
                existing_delegate_account.account_id.clone()
            } else {
                return Err(Error::<T>::DelegateAccountIdIsNone.into());
            };

            let rate = if let Some(r) = delegate_rate {
                r
            } else if let Some(existing_delegate_account) = &params.delegate_account {
                existing_delegate_account.rate
            } else {
                return Err(Error::<T>::DelegateAccountRateIsNone.into());
            };

            let delegate_account = DelegateAccount { account_id, rate };

            Self::validate_validator_delegate_account(
                &delegate_account,
                &params.hotkey,
                &validator_coldkey,
            )?;

            params.delegate_account = Some(delegate_account);
        } else {
            params.delegate_account = None;
        }

        // Self::deposit_event(Event::SubnetNodeUpdateDelegateAccount {
        //     validator_id,
        //     delegate_account: params.delegate_account.clone(),
        // });

        Ok(())
    }

    pub fn validate_validator_delegate_account(
        delegate_account: &DelegateAccount<T::AccountId>,
        hotkey: &T::AccountId,
        coldkey: &T::AccountId,
    ) -> DispatchResult {
        ensure!(
            delegate_account.account_id != *hotkey,
            Error::<T>::DelegateAccountCannotBeHotkey
        );
        ensure!(
            delegate_account.account_id != *coldkey,
            Error::<T>::DelegateAccountCannotBeColdkey
        );
        ensure!(
            delegate_account.rate <= Self::percentage_factor_as_u128() && delegate_account.rate > 0,
            Error::<T>::InvalidDelegateAccountRate
        );

        Ok(())
    }

    fn perform_update_delegate_account(
        validator_id: u32,
        maybe_params: &mut Option<ValidatorData<T::AccountId>>,
        delegate_account_id: Option<T::AccountId>,
        delegate_rate: Option<u128>,
    ) -> DispatchResult {
        let params = maybe_params
            .as_mut()
            .ok_or(Error::<T>::InvalidSubnetNodeId)?;

        ensure!(
            delegate_account_id.is_some() || delegate_rate.is_some(),
            Error::<T>::InvalidDelegateAccountParameters
        );

        if delegate_account_id.is_some() || delegate_rate.is_some() {
            let account_id = if let Some(id) = delegate_account_id {
                id
            } else if let Some(existing_delegate_account) = &params.delegate_account {
                existing_delegate_account.account_id.clone()
            } else {
                return Err(Error::<T>::DelegateAccountIdIsNone.into());
            };

            let rate = if let Some(r) = delegate_rate {
                r
            } else if let Some(existing_delegate_account) = &params.delegate_account {
                existing_delegate_account.rate
            } else {
                return Err(Error::<T>::DelegateAccountRateIsNone.into());
            };

            let delegate_account = DelegateAccount { account_id, rate };

            Self::validate_delegate_account(
                &delegate_account,
                &params.hotkey,
                &ValidatorColdkey::<T>::get(validator_id).unwrap(),
            )?;

            params.delegate_account = Some(delegate_account);
        } else {
            params.delegate_account = None;
        }

        // Self::deposit_event(Event::SubnetNodeUpdateDelegateAccount {
        //     subnet_id,
        //     subnet_node_id,
        //     delegate_account: params.delegate_account.clone(),
        // });

        Ok(())
    }

    pub fn validate_delegate_account(
        delegate_account: &DelegateAccount<T::AccountId>,
        hotkey: &T::AccountId,
        coldkey: &T::AccountId,
    ) -> DispatchResult {
        ensure!(
            delegate_account.account_id != *hotkey,
            Error::<T>::DelegateAccountCannotBeHotkey
        );
        ensure!(
            delegate_account.account_id != *coldkey,
            Error::<T>::DelegateAccountCannotBeColdkey
        );
        ensure!(
            delegate_account.rate <= Self::percentage_factor_as_u128() && delegate_account.rate > 0,
            Error::<T>::InvalidDelegateAccountRate
        );

        Ok(())
    }

    pub fn do_update_validator_identity(
        origin: OriginFor<T>,
        validator_id: u32,
        identity: Option<IdentityData>,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        // --- Ensure is or has had a subnet node
        // This will not completely stop non-subnet-node users from registering identities but prevents it
        // Accounts that have never registered a subnet node will not have a hotkey stored
        let validator_coldkey = ValidatorColdkey::<T>::try_get(validator_id)
            .map_err(|_| Error::<T>::InvalidValidatorId)?;

        ensure!(coldkey == validator_coldkey, Error::<T>::NotKeyOwner);

        ValidatorsData::<T>::try_mutate_exists(validator_id, |maybe_params| -> DispatchResult {
            let params = maybe_params
                .as_mut()
                .ok_or(Error::<T>::InvalidOverwatchNodeId)?;
            params.identity = identity.clone();
            Ok(())
        });

        Self::deposit_event(Event::IdentityUpdated {
            validator_id,
            identity: identity,
        });

        Ok(())
    }
}
