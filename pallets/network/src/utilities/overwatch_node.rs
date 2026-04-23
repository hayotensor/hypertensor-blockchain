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

use super::*;
use frame_support::pallet_prelude::DispatchError;

impl<T: Config> Pallet<T> {
    pub fn get_overwatch_node_hotkey_coldkey(
        overwatch_node_id: u32,
    ) -> Option<(T::AccountId, T::AccountId)> {
        let hotkey = OverwatchNodeIdHotkey::<T>::try_get(overwatch_node_id).ok()?;
        let coldkey = HotkeyOwner::<T>::try_get(&hotkey).ok()?;

        Some((hotkey, coldkey))
    }

    pub fn is_overwatch_node_keys_owner(overwatch_node_id: u32, key: T::AccountId) -> bool {
        let (hotkey, coldkey) = match Self::get_overwatch_node_hotkey_coldkey(overwatch_node_id) {
            Some((hotkey, coldkey)) => (hotkey, coldkey),
            None => return false,
        };

        key == hotkey || key == coldkey
    }

    pub fn get_overwatch_associated_coldkey_and_hotkey(
        overwatch_node_id: u32
    ) -> Result<(T::AccountId, T::AccountId), DispatchError> {
        let validator_id = OverwatchNodeValidatorId::<T>::try_get(overwatch_node_id)
            .map_err(|_| Error::<T>::InvalidOverwatchNodeId)?;

        let validator_coldkey = ValidatorColdkey::<T>::try_get(validator_id)
            .map_err(|_| Error::<T>::InvalidValidatorId)?;

        // An overwatch node-specific hotkey overrides the validator hotkey when present.
        if let Some(overwatch_node_hotkey) = OverwatchNodeIdHotkey::<T>::get(overwatch_node_id) {
            return Ok((validator_coldkey, overwatch_node_hotkey));
        }

        let validator_hotkey =
            ValidatorIdHotkey::<T>::get(validator_id).ok_or(Error::<T>::InvalidValidator)?;

        Ok((validator_coldkey, validator_hotkey))
    }

    /// Get a hotkeys associated overwatch node.
    /// The first check is to see if the overwatch node has a hotkey which overrides the validator hotkey.
    /// If there is no hotkey associated with the overwatch node, then we check if the validator ID has a
    /// hotkey and if it matches the caller's hotkey.
    pub fn get_overwatch_node_associated_hotkey(
        overwatch_node_id: u32,
    ) -> Result<T::AccountId, DispatchError> {
        // An overwatch node-specific hotkey overrides the validator hotkey when present.
        if let Some(overwatch_node_hotkey) = OverwatchNodeIdHotkey::<T>::get(overwatch_node_id) {
            return Ok(overwatch_node_hotkey);
        }

        let validator_id = OverwatchNodeValidatorId::<T>::try_get(overwatch_node_id)
            .map_err(|_| Error::<T>::InvalidOverwatchNodeId)?;

        let validator_hotkey =
            ValidatorIdHotkey::<T>::get(validator_id).ok_or(Error::<T>::InvalidValidator)?;

        Ok(validator_hotkey)
    }

    /// Get the coldkey of the validator that owns the overwatch node.
    pub fn get_overwatch_node_associated_coldkey(
        overwatch_node_id: u32,
    ) -> Result<T::AccountId, DispatchError> {
        let validator_id = OverwatchNodeValidatorId::<T>::try_get(overwatch_node_id)
            .map_err(|_| Error::<T>::InvalidOverwatchNodeId)?;

        let validator_coldkey = ValidatorColdkey::<T>::try_get(validator_id)
            .map_err(|_| Error::<T>::InvalidValidatorId)?;

        Ok(validator_coldkey)
    }
}
