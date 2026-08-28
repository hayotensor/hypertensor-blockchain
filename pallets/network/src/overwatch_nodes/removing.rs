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
    pub fn do_remove_overwatch_node(
        origin: T::RuntimeOrigin,
        overwatch_node_id: u32,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin.clone())?;

        let overwatch_coldkey = Self::get_overwatch_node_associated_coldkey(overwatch_node_id)?;

        ensure!(coldkey == overwatch_coldkey, Error::<T>::NotKeyOwner);

        Self::perform_remove_overwatch_node(overwatch_node_id);

        Ok(())
    }

    pub fn perform_remove_overwatch_node(overwatch_node_id: u32) {
        if OverwatchNodes::<T>::contains_key(overwatch_node_id) {
            OverwatchNodes::<T>::remove(overwatch_node_id)
        } else {
            return;
        }

        // Remove all peer IDs in all subnets
        let map = OverwatchNodeIndex::<T>::take(overwatch_node_id);
        for (subnet_id, peer_id) in map {
            PeerIdOverwatchNodeId::<T>::remove(subnet_id, peer_id);
        }

        // Node-scoped authentication ends with active ownership; it has no historical staking use.
        OverwatchNodeIdHotkey::<T>::remove(overwatch_node_id);

        TotalOverwatchNodes::<T>::mutate(|n: &mut u32| n.saturating_dec());

        // Release only the active validator-to-node ownership entry. The historical inverse entry
        // remains so the validator can withdraw stake after its node is removed.
        if let Some(validator_id) = OverwatchNodeValidatorId::<T>::get(overwatch_node_id) {
            if ValidatorOverwatchNodeId::<T>::get(validator_id) == Some(overwatch_node_id) {
                ValidatorOverwatchNodeId::<T>::remove(validator_id);
            }
        }

        // NOTE: We never delete `OverwatchNodeValidatorId`.
    }
}
