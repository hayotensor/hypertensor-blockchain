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
    /// Return only peer IDs attached to subnets that still exist.
    ///
    /// Removed-subnet entries are pruned from storage on the node's next peer update; filtering
    /// here keeps the interim lazy-cleanup state out of RPC responses.
    pub fn live_overwatch_node_peer_ids(overwatch_node_id: u32) -> BTreeMap<u32, PeerId> {
        OverwatchNodeIndex::<T>::get(overwatch_node_id)
            .into_iter()
            .filter(|(subnet_id, _)| SubnetsData::<T>::contains_key(subnet_id))
            .collect()
    }

    /// Return whether `peer_id` is available to this Overwatch node in `subnet_id`.
    ///
    /// Subnet-node peer maps are consulted solely to preserve subnet-wide peer-ID uniqueness. No
    /// subnet-node ID is used as an Overwatch identity or ownership sentinel.
    pub fn is_overwatch_peer_owner_or_ownerless(
        subnet_id: u32,
        overwatch_node_id: u32,
        peer_id: &PeerId,
    ) -> bool {
        if PeerIdSubnetNodeId::<T>::contains_key(subnet_id, peer_id)
            || BootnodePeerIdSubnetNodeId::<T>::contains_key(subnet_id, peer_id)
            || ClientPeerIdSubnetNodeId::<T>::contains_key(subnet_id, peer_id)
        {
            return false;
        }

        match PeerIdOverwatchNodeId::<T>::try_get(subnet_id, peer_id) {
            Ok(peer_overwatch_node_id) => peer_overwatch_node_id == overwatch_node_id,
            Err(()) => true,
        }
    }

    /// Resolve the validator identity historically associated with an Overwatch node ID.
    ///
    /// This remains valid after removal solely so the owner can withdraw residual node stake.
    pub fn get_historical_overwatch_validator_id(
        overwatch_node_id: u32,
    ) -> Result<u32, DispatchError> {
        OverwatchNodeValidatorId::<T>::try_get(overwatch_node_id)
            .map_err(|_| Error::<T>::InvalidOverwatchNodeId.into())
    }

    /// Resolve and validate the one-to-one active Overwatch ownership relationship.
    pub fn get_active_overwatch_validator_id(overwatch_node_id: u32) -> Result<u32, DispatchError> {
        ensure!(
            OverwatchNodes::<T>::contains_key(overwatch_node_id),
            Error::<T>::InvalidOverwatchNodeId
        );

        let validator_id = Self::get_historical_overwatch_validator_id(overwatch_node_id)?;
        ensure!(
            ValidatorOverwatchNodeId::<T>::get(validator_id) == Some(overwatch_node_id),
            Error::<T>::InvalidOverwatchNodeId
        );

        Ok(validator_id)
    }

    /// Resolve the canonical active validator identity and operational hotkey for its single
    /// Overwatch node in one pass.
    pub fn get_active_overwatch_validator_id_and_hotkey(
        overwatch_node_id: u32,
    ) -> Result<(u32, T::AccountId), DispatchError> {
        let validator_id = Self::get_active_overwatch_validator_id(overwatch_node_id)?;
        let hotkey = match OverwatchNodeIdHotkey::<T>::get(overwatch_node_id) {
            Some(overwatch_node_hotkey) => overwatch_node_hotkey,
            None => {
                ValidatorIdHotkey::<T>::get(validator_id).ok_or(Error::<T>::InvalidValidator)?
            }
        };

        Ok((validator_id, hotkey))
    }

    pub fn get_overwatch_associated_coldkey_and_hotkey(
        overwatch_node_id: u32,
    ) -> Result<(T::AccountId, T::AccountId), DispatchError> {
        let (validator_id, hotkey) =
            Self::get_active_overwatch_validator_id_and_hotkey(overwatch_node_id)?;

        let validator_coldkey = ValidatorColdkey::<T>::try_get(validator_id)
            .map_err(|_| Error::<T>::InvalidValidatorId)?;

        Ok((validator_coldkey, hotkey))
    }

    /// Get a hotkeys associated overwatch node.
    /// The first check is to see if the overwatch node has a hotkey which overrides the validator hotkey.
    /// If there is no hotkey associated with the overwatch node, then we check if the validator ID has a
    /// hotkey and if it matches the caller's hotkey.
    pub fn get_overwatch_node_associated_hotkey(
        overwatch_node_id: u32,
    ) -> Result<T::AccountId, DispatchError> {
        Self::get_active_overwatch_validator_id_and_hotkey(overwatch_node_id)
            .map(|(_, hotkey)| hotkey)
    }

    /// Get the coldkey of the validator that owns the overwatch node.
    pub fn get_overwatch_node_associated_coldkey(
        overwatch_node_id: u32,
    ) -> Result<T::AccountId, DispatchError> {
        let validator_id = Self::get_active_overwatch_validator_id(overwatch_node_id)?;

        let validator_coldkey = ValidatorColdkey::<T>::try_get(validator_id)
            .map_err(|_| Error::<T>::InvalidValidatorId)?;

        Ok(validator_coldkey)
    }
}
