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
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
use frame_support::pallet_prelude::Pays;

impl<T: Config> Pallet<T> {
    pub fn do_register_overwatch_node(
        origin: T::RuntimeOrigin,
        stake_to_be_added: u128,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin.clone())?;

        let validator_id = ColdkeyValidatorId::<T>::get(&coldkey).ok_or(Error::<T>::NotKeyOwner)?;

        ensure!(
            OverwatchValidatorWhitelist::<T>::get(validator_id),
            Error::<T>::ColdkeyBlacklisted
        );

        ensure!(
            Self::get_current_overwatch_epoch_as_u32() > 0,
            Error::<T>::OverwatchEpochIsZero
        );

        let total_overwatch_nodes = TotalOverwatchNodes::<T>::get();

        ensure!(
            total_overwatch_nodes < MaxOverwatchNodes::<T>::get(),
            Error::<T>::MaxOverwatchNodes
        );

        // ⸺ Ensure qualifies via reputation
        let reputation = ValidatorReputation::<T>::get(validator_id);

        ensure!(
            Self::is_validator_overwatch_qualified(validator_id),
            Error::<T>::ColdkeyNotOverwatchQualified
        );

        // ⸺ Register
        TotalOverwatchNodeUids::<T>::mutate(|n: &mut u32| *n += 1);
        let current_uid = TotalOverwatchNodeUids::<T>::get();

        OverwatchNodeValidatorId::<T>::insert(current_uid, validator_id);

        // ⸺ Stake
        Self::do_add_overwatch_node_stake(origin.clone(), current_uid, stake_to_be_added)
            .map_err(|e| e)?;

        let overwatch_node: OverwatchNode<T::AccountId> = OverwatchNode {
            id: current_uid,
            hotkey: coldkey.clone(),
        };

        OverwatchNodes::<T>::insert(current_uid, overwatch_node);

        TotalOverwatchNodes::<T>::mutate(|n: &mut u32| *n += 1);

        Ok(())
    }

    pub fn do_update_overwatch_hotkey(
        origin: T::RuntimeOrigin,
        overwatch_node_id: u32,
        new_hotkey: Option<T::AccountId>,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        let validator_coldkey = Self::get_overwatch_node_associated_coldkey(overwatch_node_id)?;

        ensure!(validator_coldkey == coldkey, Error::<T>::NotKeyOwner);

        if let Some(new_hotkey) = new_hotkey {
            OverwatchNodeIdHotkey::<T>::insert(overwatch_node_id, new_hotkey);
        } else {
            // Remove overwatch hotkey if None, the node will use the
            // validator hotkey for all hotkey features
            OverwatchNodeIdHotkey::<T>::remove(overwatch_node_id);
        }

        Ok(())
    }

    pub fn do_set_overwatch_node_peer_id(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        overwatch_node_id: u32,
        peer_id: PeerId,
    ) -> DispatchResultWithPostInfo {
        let key: T::AccountId = ensure_signed(origin)?;

        ensure!(
            SubnetsData::<T>::contains_key(subnet_id),
            Error::<T>::InvalidSubnetId
        );

        let (colkey, hotkey) =
            Self::get_overwatch_associated_coldkey_and_hotkey(overwatch_node_id)?;

        ensure!(key == colkey || key == hotkey, Error::<T>::NotKeyOwner);

        ensure!(Self::validate_peer_id(&peer_id), Error::<T>::InvalidPeerId);

        // Ensure no one owns the peer Id and we don't already own it
        ensure!(
            Self::is_owner_of_peer_or_ownerless(subnet_id, 0, 0, &peer_id),
            Error::<T>::PeerIdExist
        );

        PeerIdOverwatchNodeId::<T>::insert(subnet_id, &peer_id, overwatch_node_id);

        // Add or replace PeerID under subnet ID
        OverwatchNodeIndex::<T>::mutate(overwatch_node_id, |map| {
            map.insert(subnet_id, peer_id);
        });

        Ok(Pays::No.into())
    }

    pub fn is_council_qualified(validator_id: u32) -> bool {
        false
    }

    pub fn is_validator_overwatch_qualified(validator_id: u32) -> bool {
        let reputation = match ValidatorReputation::<T>::try_get(validator_id) {
            Ok(value) => value,
            Err(_) => return false,
        };
        let min_diversification_ratio = OverwatchMinDiversificationRatio::<T>::get();
        let min_score = OverwatchMinRepScore::<T>::get();
        let min_avg_attestation = OverwatchMinAvgAttestationRatio::<T>::get();
        let min_age = OverwatchMinAge::<T>::get();

        let current_epoch = Self::get_current_epoch_as_u32();

        // - No one can be an Overwatch Node yet
        if current_epoch <= min_age {
            return false;
        }

        let age = current_epoch.saturating_sub(reputation.start_epoch);

        if age < min_age {
            return false;
        }

        if reputation.score < min_score {
            return false;
        }

        Self::clean_validator_subnet_nodes(validator_id);

        // Get number of nodes under coldkey
        let mut active_unique_node_count = 0;
        ValidatorSubnetNodes::<T>::mutate(validator_id, |node_map| {
            for (subnet_id, nodes) in node_map.iter_mut() {
                let subnet_epoch = Self::get_current_subnet_epoch_as_u32(*subnet_id);

                let node_ids: Vec<u32> = nodes.iter().copied().collect();

                // Process each node_id one by one
                for node_id in node_ids {
                    if !Self::get_validator_classified_subnet_node(*subnet_id, node_id, subnet_epoch)
                        .is_none()
                    {
                        active_unique_node_count += 1;
                        // `break` to next subnet
                        // We are only checking for subnet uniqueness. We only need to verify
                        // there is one node per subnet to get the uniquness ratio
                        break;
                    }
                }
            }
        });

        let diversification = match active_unique_node_count >= TotalActiveSubnets::<T>::get() {
            true => Self::percentage_factor_as_u128(),
            false => Self::percent_div(
                active_unique_node_count as u128,
                TotalActiveSubnets::<T>::get() as u128,
            ),
        };

        if diversification < min_diversification_ratio {
            return false;
        }

        if reputation.average_attestation < min_avg_attestation {
            return false;
        }

        true
    }
}
