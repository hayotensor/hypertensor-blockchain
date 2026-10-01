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
use frame_support::BoundedBTreeMap;
use sp_runtime::Saturating;

impl<T: Config> Pallet<T> {
    pub fn do_remove_overwatch_node(
        origin: T::RuntimeOrigin,
        overwatch_node_id: u32,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin.clone())?;

        let overwatch_coldkey = Self::get_overwatch_node_associated_coldkey(overwatch_node_id)?;

        ensure!(coldkey == overwatch_coldkey, Error::<T>::NotKeyOwner);

        Self::perform_remove_overwatch_node(overwatch_node_id)
    }

    fn publish_effective_overwatch_signal(
        source_epoch: u32,
        valid: bool,
        subnet_weights: BoundedBTreeMap<u32, u128, T::MaxPhysicalSubnetsUpperBound>,
    ) {
        let revision = LatestOverwatchSignalRevision::<T>::get().saturating_add(1);
        LatestEffectiveOverwatchSignal::<T>::put(EffectiveOverwatchSignal::<T> {
            source_epoch,
            valid,
            subnet_weights,
        });
        LatestOverwatchSignalRevision::<T>::put(revision);
        Self::deposit_event(Event::EffectiveOverwatchSignalUpdated {
            source_epoch,
            revision,
            valid,
        });
    }

    /// Remove a node from the reproducible latest inputs and rebuild the per-subnet normalized
    /// signal. Missing or inconsistent inputs invalidate the cache instead of exposing stale
    /// historical influence.
    fn refresh_effective_overwatch_signal_after_removal(overwatch_node_id: u32) {
        let finalized_epoch = LastFinalizedOverwatchEpoch::<T>::get();
        let existing_inputs = LatestFinalizedOverwatchSignalInputs::<T>::get();
        let existing_cache = LatestEffectiveOverwatchSignal::<T>::get();

        if finalized_epoch.is_none() && existing_inputs.is_none() && existing_cache.is_none() {
            return;
        }

        let source_epoch = finalized_epoch
            .or_else(|| existing_inputs.as_ref().map(|inputs| inputs.source_epoch))
            .or_else(|| existing_cache.as_ref().map(|cache| cache.source_epoch))
            .unwrap_or_default();

        let Some(mut inputs) = existing_inputs else {
            Self::publish_effective_overwatch_signal(source_epoch, false, BoundedBTreeMap::new());
            return;
        };

        if finalized_epoch != Some(inputs.source_epoch) {
            LatestFinalizedOverwatchSignalInputs::<T>::kill();
            Self::publish_effective_overwatch_signal(source_epoch, false, BoundedBTreeMap::new());
            return;
        }

        let Ok(before_removal) = Self::derive_overwatch_signal(&inputs) else {
            inputs.nodes.remove(&overwatch_node_id);
            LatestFinalizedOverwatchSignalInputs::<T>::put(inputs);
            Self::publish_effective_overwatch_signal(source_epoch, false, BoundedBTreeMap::new());
            return;
        };
        let cache_was_current = existing_cache.as_ref().is_some_and(|cache| {
            cache.source_epoch == source_epoch
                && cache.valid
                && cache.subnet_weights == before_removal.subnet_weights
        });

        let removed_input = inputs.nodes.remove(&overwatch_node_id).is_some();
        if !removed_input && cache_was_current {
            return;
        }

        let Ok(after_removal) = Self::derive_overwatch_signal(&inputs) else {
            LatestFinalizedOverwatchSignalInputs::<T>::put(inputs);
            Self::publish_effective_overwatch_signal(source_epoch, false, BoundedBTreeMap::new());
            return;
        };
        LatestFinalizedOverwatchSignalInputs::<T>::put(inputs);

        // Retained inputs still purge the node, but an already-valid cache needs no revision when
        // the counterfactual signal is byte-for-byte identical.
        if removed_input
            && cache_was_current
            && before_removal.subnet_weights == after_removal.subnet_weights
        {
            Self::deposit_event(Event::EffectiveOverwatchSignalUpdated {
                source_epoch,
                revision: LatestOverwatchSignalRevision::<T>::get(),
                valid: true,
            });
            return;
        }
        Self::publish_effective_overwatch_signal(source_epoch, true, after_removal.subnet_weights);
    }

    #[frame_support::transactional]
    pub fn perform_remove_overwatch_node(overwatch_node_id: u32) -> DispatchResult {
        let validator_id = Self::get_active_overwatch_validator_id(overwatch_node_id)?;

        // Exit ends authentication immediately. Preserve commitments until rollover so exit
        // cannot hide an incomplete submission; fully revealed work retains its opening stake.
        Self::clear_overwatch_active_membership(overwatch_node_id, validator_id);
        Ok(())
    }

    fn clear_overwatch_active_membership(overwatch_node_id: u32, validator_id: u32) {
        OverwatchNodes::<T>::remove(overwatch_node_id);
        for (subnet_id, peer_id) in OverwatchNodeIndex::<T>::take(overwatch_node_id) {
            PeerIdOverwatchNodeId::<T>::remove(subnet_id, peer_id);
        }
        OverwatchNodeIdHotkey::<T>::remove(overwatch_node_id);
        TotalOverwatchNodes::<T>::mutate(|n| n.saturating_dec());
        ValidatorOverwatchNodeId::<T>::remove(validator_id);
        OverwatchValidatorWhitelist::<T>::remove(validator_id);
        // Historical ownership and stake remain available for later rewards and withdrawals.
    }

    #[frame_support::transactional]
    pub fn perform_disqualify_overwatch_node(overwatch_node_id: u32) -> DispatchResult {
        let validator_id = Self::get_historical_overwatch_validator_id(overwatch_node_id)?;
        let active = OverwatchNodes::<T>::contains_key(overwatch_node_id);
        if active {
            Self::get_active_overwatch_validator_id(overwatch_node_id)?;
        }
        let active_epoch = CurrentOverwatchEpoch::<T>::get();
        let pending = PendingOverwatchSettlement::<T>::get();
        let has_epoch_input = |epoch| {
            OverwatchEpochSnapshots::<T>::get(epoch)
                .is_some_and(|snapshot| snapshot.nodes.contains_key(&overwatch_node_id))
        };
        let relevant = active
            || has_epoch_input(active_epoch)
            || pending.as_ref().is_some_and(|p| has_epoch_input(p.epoch))
            || LatestFinalizedOverwatchSignalInputs::<T>::get()
                .is_some_and(|inputs| inputs.nodes.contains_key(&overwatch_node_id));
        ensure!(relevant, Error::<T>::InvalidOverwatchNodeId);

        OverwatchEpochSnapshots::<T>::mutate(active_epoch, |snapshot| {
            if let Some(snapshot) = snapshot {
                snapshot.nodes.remove(&overwatch_node_id);
            }
        });

        // Purge every active-round submission and update its compact cardinality index.
        OverwatchCommits::<T>::remove(active_epoch, overwatch_node_id);
        let active_reveals = OverwatchReveals::<T>::take(active_epoch, overwatch_node_id);
        if !active_reveals.is_empty() {
            ActiveOverwatchRevealStats::<T>::mutate(|stats| {
                stats.records = stats.records.saturating_sub(active_reveals.len() as u32);
                for subnet_id in active_reveals.keys() {
                    if let Some(count) = stats.subnet_revealer_counts.get_mut(subnet_id) {
                        if *count <= 1 {
                            stats.subnet_revealer_counts.remove(subnet_id);
                        } else {
                            *count = count.saturating_sub(1);
                        }
                    }
                }
            });
        }

        // Only governance may purge pending participation. Purging prevents both scoring and
        // reward credit while retaining the historical validator ID and principal balance.
        let mut finalize_empty_pending = false;
        if let Some(mut pending) = pending {
            OverwatchEpochRevealEligibility::<T>::mutate_exists(pending.epoch, |eligible| {
                if let Some(eligible) = eligible {
                    eligible.remove(&overwatch_node_id);
                }
            });
            let pending_reveals = OverwatchReveals::<T>::take(pending.epoch, overwatch_node_id);
            pending.reveal_records = pending
                .reveal_records
                .saturating_sub(pending_reveals.len() as u32);
            finalize_empty_pending = pending.reveal_records == 0;
            PendingOverwatchSettlement::<T>::put(pending);

            if let Some(mut snapshot) = OverwatchEpochSnapshots::<T>::get(pending.epoch) {
                snapshot.nodes.remove(&overwatch_node_id);
                OverwatchEpochSnapshots::<T>::insert(pending.epoch, snapshot);
            }
        }

        // An already-exited ID may have a replacement under the same validator. Disqualifying
        // its historical work must not clear the replacement's indexes or fresh approval.
        if active {
            Self::clear_overwatch_active_membership(overwatch_node_id, validator_id);
        }

        Self::refresh_effective_overwatch_signal_after_removal(overwatch_node_id);

        // Finalize an explicit empty epoch immediately when removal consumed its last pending
        // participant. The settlement path writes valid-empty state and cannot issue a reward.
        if finalize_empty_pending {
            let _ = Self::calculate_overwatch_rewards();
        }

        // NOTE: We never delete `OverwatchNodeValidatorId` or the node's stake balance.
        Ok(())
    }
}
