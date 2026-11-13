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

//! Retention and removal cleanup for per-epoch subnet consensus history.
//!
//! `propose_attestation` and `attest` are feeless and store per-round records keyed by
//! `(subnet_id, subnet_epoch)`, including opaque payloads bounded by `ValidatorArgsLimit`.
//! Without a retention policy that state would grow for ever. Each subnet slot prunes rounds
//! older than [`CONSENSUS_HISTORY_RETENTION_EPOCHS`] (bounded by the hook weight meter), and a
//! removed subnet's remaining history is cleared incrementally with leftover block weight.

use super::*;
use frame_support::{pallet_prelude::Weight, weights::WeightMeter};

/// Number of most recent subnet epochs whose consensus history is retained (for settlement
/// retries, RPC and off-chain inspection). Settlement only needs the previous epoch and the
/// single pending round, which is never pruned.
pub const CONSENSUS_HISTORY_RETENTION_EPOCHS: u32 = 16;

/// Upper bound on epochs pruned per subnet slot. Steady state prunes one per subnet epoch; the
/// surplus lets a cursor catch up after slots that ran out of weight. Empty epochs are skipped
/// for one read each and are bounded only by the weight meter.
pub const MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT: u32 = 4;

/// Keys removed per storage prefix per block when clearing a removed subnet's history.
pub const REMOVED_SUBNET_HISTORY_CLEAR_LIMIT: u32 = 256;

impl<T: Config> Pallet<T> {
    /// Fixed single-key removals per pruned round (elected round, status, emission epoch,
    /// submission, max items, proposal args, attestor weights).
    fn consensus_round_fixed_prune_weight() -> Weight {
        T::DbWeight::get().reads_writes(1, 7)
    }

    /// Prune consensus history for `subnet_id` older than the retention window, never touching
    /// the round that is still pending settlement. Work is bounded by `weight_meter` and by
    /// [`MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT`].
    pub(crate) fn prune_consensus_history(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        current_subnet_epoch: u32,
    ) {
        let db_weight = T::DbWeight::get();
        let selector_weight = db_weight.reads(2);
        if !weight_meter.can_consume(selector_weight) {
            return;
        }
        weight_meter.consume(selector_weight);

        // Rounds strictly before `horizon` are outside the retention window.
        let horizon = current_subnet_epoch.saturating_sub(CONSENSUS_HISTORY_RETENTION_EPOCHS);
        let stop = match PendingConsensusRoundSettlementEpoch::<T>::get(subnet_id) {
            Some(pending_epoch) => pending_epoch.min(horizon),
            None => horizon,
        };

        let Some(mut cursor) = ConsensusHistoryPruneCursor::<T>::get(subnet_id) else {
            // One-time init at the oldest epoch that can hold history: no round is elected before
            // the subnet's consensus-eligibility epoch. History written before this pruning
            // existed (e.g. before an upgrade) is therefore reached too; empty epochs are skipped
            // cheaply below, so catching up stays bounded per slot.
            let init_weight = db_weight.reads_writes(1, 1);
            if weight_meter.can_consume(init_weight) {
                weight_meter.consume(init_weight);
                let oldest = SubnetsData::<T>::get(subnet_id)
                    .and_then(|subnet| subnet.consensus_eligible_from_subnet_epoch)
                    .unwrap_or(0);
                ConsensusHistoryPruneCursor::<T>::insert(subnet_id, oldest);
            }
            return;
        };

        let attestation_limit = T::MaxSubnetNodesUpperBound::get().max(1);
        let worst_case_epoch_weight = Self::consensus_round_fixed_prune_weight()
            .saturating_add(db_weight.reads_writes(1, attestation_limit as u64));
        let cursor_write_weight = db_weight.writes(1);

        let probe_weight = db_weight.reads(1);
        let start = cursor;
        let mut pruned = 0u32;
        while cursor < stop && pruned < MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT {
            // Every per-epoch record is written only after that epoch's election, so an epoch
            // without an elected round holds no history and costs a single read to skip.
            if !weight_meter.can_consume(probe_weight.saturating_add(cursor_write_weight)) {
                break;
            }
            weight_meter.consume(probe_weight);
            if !SubnetElectedValidator::<T>::contains_key(subnet_id, cursor) {
                cursor = cursor.saturating_add(1);
                continue;
            }
            if !weight_meter
                .can_consume(worst_case_epoch_weight.saturating_add(cursor_write_weight))
            {
                break;
            }

            let removed = SubnetConsensusAttestationData::<T>::clear_prefix(
                (subnet_id, cursor),
                attestation_limit,
                None,
            );
            weight_meter.consume(
                db_weight.reads_writes(removed.loops.max(1) as u64, removed.unique as u64),
            );
            if removed.maybe_cursor.is_some() {
                // More attestation entries than the per-call limit (inconsistent state). The same
                // prefix must not be cleared twice in one block; resume on a later slot.
                break;
            }

            SubnetElectedValidator::<T>::remove(subnet_id, cursor);
            ConsensusRoundSettlementStatus::<T>::remove(subnet_id, cursor);
            ConsensusRoundSettlementEmissionEpoch::<T>::remove(subnet_id, cursor);
            SubnetConsensusSubmission::<T>::remove(subnet_id, cursor);
            SubnetConsensusSubmissionMaxItems::<T>::remove(subnet_id, cursor);
            SubnetConsensusProposalArgs::<T>::remove(subnet_id, cursor);
            SubnetConsensusAttestorWeights::<T>::remove(subnet_id, cursor);
            weight_meter.consume(Self::consensus_round_fixed_prune_weight());

            cursor = cursor.saturating_add(1);
            pruned = pruned.saturating_add(1);
        }

        if cursor != start {
            weight_meter.consume(cursor_write_weight);
            ConsensusHistoryPruneCursor::<T>::insert(subnet_id, cursor);
        }
    }

    /// Schedule a removed subnet's per-epoch consensus history for incremental cleanup. Subnet
    /// IDs are never reused, so deferred clearing cannot touch a later subnet's state.
    pub(crate) fn queue_removed_subnet_consensus_history(subnet_id: u32) -> Weight {
        ConsensusHistoryPruneCursor::<T>::remove(subnet_id);
        ConsensusHistoryCleanupQueue::<T>::mutate(|queue| {
            if !queue.contains(&subnet_id) {
                queue.push(subnet_id);
            }
        });
        T::DbWeight::get().reads_writes(1, 2)
    }

    /// Clear the history of the oldest queued removed subnet using leftover block weight. Each
    /// prefix is cleared at most once per block (a limited `clear_prefix` must not be repeated
    /// within one block); the subnet leaves the queue once every prefix reports completion.
    pub(crate) fn cleanup_removed_subnet_consensus_history(weight_meter: &mut WeightMeter) {
        let db_weight = T::DbWeight::get();
        let selector_weight = db_weight.reads(1);
        if !weight_meter.can_consume(selector_weight) {
            return;
        }
        weight_meter.consume(selector_weight);

        let mut queue = ConsensusHistoryCleanupQueue::<T>::get();
        let Some(subnet_id) = queue.first().copied() else {
            return;
        };

        let limit = REMOVED_SUBNET_HISTORY_CLEAR_LIMIT;
        let step_weight = db_weight.reads_writes(limit as u64, limit as u64);
        let queue_write_weight = db_weight.writes(1);

        macro_rules! clear_step {
            ($clear:expr) => {{
                if !weight_meter.can_consume(step_weight.saturating_add(queue_write_weight)) {
                    return;
                }
                let removed = $clear;
                weight_meter.consume(
                    db_weight.reads_writes(removed.loops.max(1) as u64, removed.unique as u64),
                );
                if removed.maybe_cursor.is_some() {
                    return;
                }
            }};
        }

        clear_step!(SubnetConsensusAttestationData::<T>::clear_prefix(
            (subnet_id,),
            limit,
            None
        ));
        clear_step!(SubnetConsensusSubmission::<T>::clear_prefix(
            subnet_id, limit, None
        ));
        clear_step!(SubnetConsensusSubmissionMaxItems::<T>::clear_prefix(
            subnet_id, limit, None
        ));
        clear_step!(SubnetConsensusProposalArgs::<T>::clear_prefix(
            subnet_id, limit, None
        ));
        clear_step!(SubnetConsensusAttestorWeights::<T>::clear_prefix(
            subnet_id, limit, None
        ));
        clear_step!(ConsensusRoundSettlementEmissionEpoch::<T>::clear_prefix(
            subnet_id, limit, None
        ));
        clear_step!(ConsensusRoundSettlementStatus::<T>::clear_prefix(
            subnet_id, limit, None
        ));
        clear_step!(SubnetElectedValidator::<T>::clear_prefix(
            subnet_id, limit, None
        ));

        queue.remove(0);
        weight_meter.consume(queue_write_weight);
        if queue.is_empty() {
            ConsensusHistoryCleanupQueue::<T>::kill();
        } else {
            ConsensusHistoryCleanupQueue::<T>::put(queue);
        }
    }
}
