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

impl<T: Config> Pallet<T> {
    // pub fn do_update_validator_identity(
    //     coldkey: T::AccountId,
    //     validator_id: u32,
    //     identity: Option<IdentityData>,
    // ) -> DispatchResult {
    //     // --- Ensure is or has had a subnet node
    //     // This will not completely stop non-subnet-node users from registering identities but prevents it
    //     // Accounts that have never registered a subnet node will not have a hotkey stored
    //     let validator_coldkey = ValidatorColdkey::<T>::try_get(validator_id)
    //         .map_err(|_| Error::<T>::InvalidValidatorId)?;

    //     ensure!(coldkey == validator_coldkey, Error::<T>::NotKeyOwner);

    //     ValidatorsData::<T>::try_mutate_exists(validator_id, |maybe_params| -> DispatchResult {
    //         let params = maybe_params
    //             .as_mut()
    //             .ok_or(Error::<T>::InvalidOverwatchNodeId)?;
    //         params.identity = identity;
    //         Ok(())
    //     });

    //     Self::deposit_event(Event::IdentityUpdated {
    //         coldkey: coldkey,
    //         identity: coldkey_identity,
    //     });

    //     Ok(())
    // }
}
