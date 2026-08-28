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
    #[frame_support::transactional]
    pub fn do_register_overwatch_node(
        origin: T::RuntimeOrigin,
        stake_to_be_added: u128,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin.clone())?;

        let validator_id = Self::get_canonical_validator_id_for_coldkey(&coldkey)?;

        ensure!(
            OverwatchValidatorWhitelist::<T>::get(validator_id),
            Error::<T>::ValidatorNotOverwatchWhitelisted
        );

        ensure!(
            !ValidatorOverwatchNodeId::<T>::contains_key(validator_id),
            Error::<T>::ValidatorAlreadyHasOverwatchNode
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
        ensure!(
            Self::is_validator_overwatch_qualified_read_only(validator_id),
            Error::<T>::ValidatorNotOverwatchQualified
        );

        let current_uid = TotalOverwatchNodeUids::<T>::get()
            .checked_add(1)
            .ok_or(Error::<T>::OverwatchNodeIdExhausted)?;

        // IDs are monotonic and the historical owner mapping is intentionally retained after
        // removal. Refuse to overwrite either active or historical state if the counter is ever
        // inconsistent.
        ensure!(
            !OverwatchNodes::<T>::contains_key(current_uid)
                && !OverwatchNodeValidatorId::<T>::contains_key(current_uid),
            Error::<T>::OverwatchNodeIdExhausted
        );

        ensure!(stake_to_be_added != 0, Error::<T>::InvalidAmount);

        let balance = match Self::u128_to_balance(stake_to_be_added) {
            Some(b) => b,
            None => return Err(Error::<T>::CouldNotConvertToBalance.into()),
        };

        let account_stake_balance: u128 = OverwatchNodeStakeBalance::<T>::get(current_uid);

        ensure!(
            account_stake_balance.saturating_add(stake_to_be_added)
                >= OverwatchMinStakeBalance::<T>::get(),
            Error::<T>::MinStakeNotReached
        );

        ensure!(
            Self::can_remove_balance_from_coldkey_account(&coldkey, balance),
            Error::<T>::NotEnoughBalanceToStake
        );

        // ⸺ Stake
        ensure!(
            Self::remove_balance_from_coldkey_account(&coldkey, balance) == true,
            Error::<T>::BalanceWithdrawalError
        );
        Self::increase_overwatch_node_stake(current_uid, stake_to_be_added);

        let overwatch_node = OverwatchNode { id: current_uid };

        // ⸺ Register
        TotalOverwatchNodeUids::<T>::put(current_uid);
        OverwatchNodeValidatorId::<T>::insert(current_uid, validator_id);
        ValidatorOverwatchNodeId::<T>::insert(validator_id, current_uid);
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

        // Preserve peer-ID uniqueness without treating a subnet-node ID as Overwatch identity.
        ensure!(
            Self::is_overwatch_peer_owner_or_ownerless(subnet_id, overwatch_node_id, &peer_id),
            Error::<T>::PeerIdExist
        );

        let previous_peer_id = OverwatchNodeIndex::<T>::get(overwatch_node_id)
            .get(&subnet_id)
            .cloned();

        // A node has at most one peer ID per subnet. Remove the prior reverse entry when replacing
        // it, but only if that reverse entry still belongs to this Overwatch node.
        if let Some(previous_peer_id) = previous_peer_id {
            if previous_peer_id != peer_id
                && PeerIdOverwatchNodeId::<T>::try_get(subnet_id, &previous_peer_id)
                    == Ok(overwatch_node_id)
            {
                PeerIdOverwatchNodeId::<T>::remove(subnet_id, previous_peer_id);
            }
        }

        PeerIdOverwatchNodeId::<T>::insert(subnet_id, &peer_id, overwatch_node_id);

        // Add or replace PeerID under subnet ID
        OverwatchNodeIndex::<T>::mutate(overwatch_node_id, |map| {
            map.insert(subnet_id, peer_id);
        });

        Ok(Pays::No.into())
    }

    pub fn is_validator_overwatch_qualified_read_only(validator_id: u32) -> bool {
        let reputation = match ValidatorReputation::<T>::try_get(validator_id) {
            Ok(value) => value,
            Err(_) => return false,
        };
        let min_score = OverwatchMinRepScore::<T>::get();
        let min_avg_attestation = OverwatchMinAvgAttestationRatio::<T>::get();
        let min_age = OverwatchMinAge::<T>::get();

        let Some(start_epoch) = reputation.start_epoch else {
            return false;
        };

        let current_epoch = Self::get_current_epoch_as_u32();
        let age = current_epoch.saturating_sub(start_epoch);

        if age < min_age {
            return false;
        }

        if reputation.score < min_score {
            return false;
        }

        if reputation.average_proposal_identity_support < min_avg_attestation {
            return false;
        }

        true
    }
}
