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

/// Number of most recent subnet epochs retained for RPC and off-chain inspection.
/// Settlement only needs the previous epoch; unfinished older rounds expire with this history.
pub const CONSENSUS_HISTORY_RETENTION_EPOCHS: u32 = 16;

/// Upper bound on epochs visited per subnet slot, including epochs without an election.
/// Steady state prunes one per subnet epoch; the surplus lets a cursor catch up after skipped
/// slots without making one hook's work depend on the length of the gap.
pub const MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT: u32 = 4;

/// Keys removed per storage prefix per block when clearing a removed subnet's history.
pub const REMOVED_SUBNET_HISTORY_CLEAR_LIMIT: u32 = 256;

impl<T: Config> Pallet<T> {
    /// Conservative trie allowance for a compact storage key. These manual proof bounds
    /// supplement database ref-time until pruning has dedicated benchmark measurements.
    fn consensus_history_key_proof(keys: u64) -> Weight {
        Weight::from_parts(0, 4_096u64.saturating_mul(keys))
    }

    fn consensus_round_probe_weight() -> Weight {
        // Election data stores one four-byte node ID and one eight-byte identity-map entry
        // per candidate. The fixed margin covers the policy, principal, and SCALE prefixes.
        T::DbWeight::get()
            .reads(1)
            .saturating_add(Weight::from_parts(
                0,
                12 * T::MaxSubnetNodesUpperBound::get() as u64 + 1_024 + 4_096,
            ))
    }

    fn consensus_attestation_prune_weight(loops: u64, unique: u64) -> Weight {
        // Prefix clearing removes overlay entries as well as iterating backend entries.
        // Reserve the larger count, plus the end probe, even when overlay reads are free.
        let proof_per_entry = T::ValidatorArgsLimit::get() as u64 + 5 + 4_096;
        T::DbWeight::get()
            .reads_writes(loops.saturating_add(1), unique)
            .saturating_add(Weight::from_parts(
                0,
                loops
                    .max(unique)
                    .saturating_add(1)
                    .saturating_mul(proof_per_entry),
            ))
    }

    /// Fixed single-key removals per pruned round (elected round, status,
    /// submission, max items, proposal args, attestor weights).
    fn consensus_round_fixed_prune_weight() -> Weight {
        // Canonical stored proposals keep opaque args/attest data in separate keys. Their
        // compact per-node fields use 86 bytes, elections use 12, and weights use 20. Round
        // up to 128 per node, plus fixed fields, six trie paths, and the proposal payload.
        let proof = 128 * T::MaxSubnetNodesUpperBound::get() as u64
            + 1_024
            + 6 * 4_096
            + T::ValidatorArgsLimit::get() as u64
            + 5;
        T::DbWeight::get()
            .reads_writes(1, 6)
            .saturating_add(Weight::from_parts(0, proof))
    }

    /// Prune consensus history for `subnet_id` older than the retention window.
    /// Work is bounded by `weight_meter` and by
    /// [`MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT`].
    pub(crate) fn prune_consensus_history(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        current_subnet_epoch: u32,
    ) {
        let db_weight = T::DbWeight::get();
        let selector_weight = db_weight
            .reads(1)
            .saturating_add(Self::consensus_history_key_proof(1));
        if !weight_meter.can_consume(selector_weight) {
            return;
        }
        weight_meter.consume(selector_weight);

        // Rounds strictly before `horizon` are outside the retention window.
        let horizon = current_subnet_epoch.saturating_sub(CONSENSUS_HISTORY_RETENTION_EPOCHS);
        let stop = horizon;

        let Some(mut cursor) = ConsensusHistoryPruneCursor::<T>::get(subnet_id) else {
            // Initialize at the oldest epoch that can hold history: no round is elected before
            // the subnet's consensus-eligibility epoch. Empty epochs are skipped within the
            // same per-slot visit bound as elected rounds.
            let init_weight = Self::subnet_lifecycle_read_weight()
                .saturating_add(db_weight.writes(1))
                .saturating_add(Self::consensus_history_key_proof(1));
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
            // Backend-resident attestations require one read per removed entry and an end
            // probe, unlike fresh overlay writes. Reserve them before clearing the prefix.
            .saturating_add(Self::consensus_attestation_prune_weight(
                attestation_limit as u64,
                attestation_limit as u64,
            ));
        let cursor_write_weight = db_weight
            .writes(1)
            .saturating_add(Self::consensus_history_key_proof(1));

        let probe_weight = Self::consensus_round_probe_weight();
        let start = cursor;
        let mut visited = 0u32;
        while cursor < stop && visited < MAX_PRUNED_CONSENSUS_EPOCHS_PER_SLOT {
            // Every per-epoch record is written only after that epoch's election, so an epoch
            // without an elected round holds no history and costs a single read to skip.
            if !weight_meter.can_consume(probe_weight.saturating_add(cursor_write_weight)) {
                break;
            }
            weight_meter.consume(probe_weight);
            visited += 1;
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
            // The SDK's current clear_prefix wrapper reports backend removals only, omitting
            // overlay deletions from `unique`. Charge the reserved valid-round write/proof
            // bound so both storage locations are covered; backend reads still use its count.
            weight_meter.consume(Self::consensus_attestation_prune_weight(
                removed.loops as u64,
                attestation_limit as u64,
            ));
            if removed.maybe_cursor.is_some() {
                // More attestation entries than the per-call limit (inconsistent state). The same
                // prefix must not be cleared twice in one block; resume on a later slot.
                break;
            }

            SubnetElectedValidator::<T>::remove(subnet_id, cursor);
            ConsensusRoundSettlementStatus::<T>::remove(subnet_id, cursor);
            SubnetConsensusSubmission::<T>::remove(subnet_id, cursor);
            SubnetConsensusSubmissionMaxItems::<T>::remove(subnet_id, cursor);
            SubnetConsensusProposalArgs::<T>::remove(subnet_id, cursor);
            SubnetConsensusAttestorWeights::<T>::remove(subnet_id, cursor);
            weight_meter.consume(Self::consensus_round_fixed_prune_weight());

            cursor = cursor.saturating_add(1);
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
