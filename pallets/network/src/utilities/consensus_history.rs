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
//! All history deletion runs in `on_idle`, after financial retries and node metadata cleanup.
//! Live subnets retain at least [`CONSENSUS_HISTORY_RETENTION_EPOCHS`] completed epochs; removed
//! subnets are cleared incrementally. Persistent cursors keep each idle pass independent of the
//! backlog size. Retention is best effort: busy blocks can postpone cleanup indefinitely.

use super::*;
use frame_support::{pallet_prelude::Weight, weights::WeightMeter};

/// Number of most recent subnet epochs retained for RPC and off-chain inspection.
/// Settlement only needs the previous epoch; unfinished older rounds expire with this history.
pub const CONSENSUS_HISTORY_RETENTION_EPOCHS: u32 = 128;

/// Maximum historical epochs visited for one live subnet per idle pass, including empty epochs.
pub const MAX_PRUNED_CONSENSUS_EPOCHS_PER_IDLE: u32 = 4;

/// Maximum key deletions or empty-prefix probes for one removed subnet per idle pass.
pub const MAX_REMOVED_HISTORY_STEPS_PER_IDLE: u32 = 256;

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
    /// [`MAX_PRUNED_CONSENSUS_EPOCHS_PER_IDLE`].
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
            // same per-pass visit bound as elected rounds.
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
        while cursor < stop && visited < MAX_PRUNED_CONSENSUS_EPOCHS_PER_IDLE {
            // Every per-epoch record is written only after that epoch's election, so an epoch
            // without an elected round holds no history and costs a single read to skip.
            if !weight_meter.can_consume(probe_weight.saturating_add(cursor_write_weight)) {
                break;
            }
            weight_meter.consume(probe_weight);
            visited += 1;
            if !SubnetElectedValidator::<T>::contains_key(subnet_id, cursor) {
                let matrix_probe = db_weight
                    .reads(1)
                    .saturating_add(Weight::from_parts(0, 16_384));
                if !weight_meter.can_consume(matrix_probe.saturating_add(cursor_write_weight)) {
                    break;
                }
                weight_meter.consume(matrix_probe);
                if MatrixRounds::<T>::contains_key(subnet_id, cursor) {
                    // Keep enough weight to persist the common cursor after partial cleanup.
                    let mut matrix_meter = WeightMeter::with_limit(
                        weight_meter.remaining().saturating_sub(cursor_write_weight),
                    );
                    let complete = Self::prune_matrix_round(&mut matrix_meter, subnet_id, cursor);
                    weight_meter.consume(matrix_meter.consumed());
                    if !complete {
                        break;
                    }
                }
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
                // prefix must not be cleared twice in one block; resume on a later idle pass.
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

    /// Idle is the only production caller of either history deletion worker. Alternate the
    /// first worker so a large removed-subnet backlog cannot always pre-empt live retention.
    pub(crate) fn cleanup_consensus_history_on_idle(weight_meter: &mut WeightMeter, block: u32) {
        if block % 2 == 0 {
            Self::prune_live_consensus_history(weight_meter, block);
            Self::cleanup_removed_subnet_consensus_history(weight_meter);
        } else {
            Self::cleanup_removed_subnet_consensus_history(weight_meter);
            Self::prune_live_consensus_history(weight_meter, block);
        }
    }

    /// One cursor read, at most two iterator seeks/value reads (including wraparound), and
    /// the cursor write. All selected records are scalars, independent of history/backlog size.
    pub(crate) fn history_idle_selector_weight() -> Weight {
        T::DbWeight::get()
            .reads_writes(5, 1)
            .saturating_add(Weight::from_parts(0, 5 * (4_096 + 32)))
    }

    fn prune_live_consensus_history(weight_meter: &mut WeightMeter, block: u32) {
        if weight_meter
            .try_consume(Self::history_idle_selector_weight())
            .is_err()
        {
            return;
        }
        let next = ConsensusHistoryPruneLastSlot::<T>::get()
            .and_then(|last| SlotAssignment::<T>::iter_from_key(last).next())
            .or_else(|| SlotAssignment::<T>::iter().next());
        let Some((slot, subnet_id)) = next else {
            return;
        };
        ConsensusHistoryPruneLastSlot::<T>::put(slot);
        // Use the selected subnet's slot, not the general epoch. No extra storage lookup is
        // necessary, and a removed slot is naturally skipped on the next iterator seek.
        let epoch_length = T::EpochLength::get();
        if slot == 0 || epoch_length == 0 {
            return;
        }
        let subnet_epoch = block.saturating_sub(slot) / epoch_length;
        Self::prune_consensus_history(weight_meter, subnet_id, subnet_epoch);
    }

    /// Enqueue is constant work even if idle cleanup has been starved for a long time. This
    /// replaces the vector-wide membership scan and rewrite on the subnet-removal path.
    pub(crate) fn queue_removed_subnet_consensus_history(subnet_id: u32) -> Weight {
        ConsensusHistoryPruneCursor::<T>::remove(subnet_id);
        ConsensusHistoryCleanupQueue::<T>::insert(
            subnet_id,
            ConsensusHistoryCleanupStage::Attestations,
        );
        // Retain the existing removal model's read/write allowance; the new scalar work record
        // replaces its queue read/write and cannot exceed the old encoded queue value.
        T::DbWeight::get().reads_writes(1, 2)
    }

    pub(crate) fn removed_history_progress_weight() -> Weight {
        T::DbWeight::get()
            .writes(1)
            .saturating_add(Self::consensus_history_key_proof(1))
    }

    /// Reserve a seek, a storage access and a deletion, including the maximum old value proof.
    /// Keys are selected without decoding historical values. Individual deletes also bound
    /// overlay work: clear_prefix's backend limit does not bound same-block overlay removals.
    pub(crate) fn removed_history_step_weight(stage: ConsensusHistoryCleanupStage) -> Weight {
        use ConsensusHistoryCleanupStage::*;
        let value_bytes = match stage {
            Attestations | ProposalArgs => T::ValidatorArgsLimit::get() as u64 + 5,
            SubmissionMaxItems | SettlementStatus => 32,
            Matrix => 32_768,
            // The combined fixed-round bound conservatively covers each of these value types.
            Submissions | AttestorWeights | Elections => {
                128 * T::MaxSubnetNodesUpperBound::get() as u64
                    + 1_024
                    + T::ValidatorArgsLimit::get() as u64
                    + 5
            }
        };
        T::DbWeight::get()
            .reads_writes(2, 1)
            .saturating_add(Weight::from_parts(0, value_bytes + 4_096))
    }

    /// Visit one removed subnet in round-robin storage-key order. Persist the prefix stage so
    /// a small idle allowance can make progress without repeatedly probing earlier prefixes.
    pub(crate) fn cleanup_removed_subnet_consensus_history(weight_meter: &mut WeightMeter) {
        use ConsensusHistoryCleanupStage::*;
        if weight_meter
            .try_consume(Self::history_idle_selector_weight())
            .is_err()
        {
            return;
        }
        let next = ConsensusHistoryCleanupLastSubnet::<T>::get()
            .and_then(|last| ConsensusHistoryCleanupQueue::<T>::iter_from_key(last).next())
            .or_else(|| ConsensusHistoryCleanupQueue::<T>::iter().next());
        let Some((subnet_id, mut stage)) = next else {
            return;
        };
        ConsensusHistoryCleanupLastSubnet::<T>::put(subnet_id);
        let original_stage = stage;
        let progress_weight = Self::removed_history_progress_weight();
        if stage == Matrix {
            // At most one Matrix round (128 row/commitment keys) per removed-subnet visit.
            // A second probe can finish the queue record, but cannot delete another round.
            for visit in 0..2 {
                let probe = Self::removed_history_step_weight(Matrix);
                if !weight_meter.can_consume(probe.saturating_add(progress_weight)) {
                    return;
                }
                weight_meter.consume(probe);
                let Some(epoch) = MatrixRounds::<T>::iter_key_prefix(subnet_id).next() else {
                    weight_meter.consume(progress_weight);
                    ConsensusHistoryCleanupQueue::<T>::remove(subnet_id);
                    return;
                };
                if visit != 0 {
                    return;
                }
                if !Self::prune_matrix_round(weight_meter, subnet_id, epoch) {
                    return;
                }
            }
            return;
        }
        // Advance within each prefix instead of seeking from its beginning after every delete.
        // This avoids repeatedly traversing backend keys already deleted in this block's overlay.
        let mut last_key = None;

        macro_rules! remove_next {
            ($storage:ident, $next_stage:expr) => {
                if let Some(epoch) = match last_key.take() {
                    Some(key) => $storage::<T>::iter_key_prefix_from(subnet_id, key).next(),
                    None => $storage::<T>::iter_key_prefix(subnet_id).next(),
                } {
                    last_key = Some($storage::<T>::hashed_key_for(subnet_id, epoch));
                    $storage::<T>::remove(subnet_id, epoch);
                } else {
                    stage = $next_stage;
                }
            };
        }

        for _ in 0..MAX_REMOVED_HISTORY_STEPS_PER_IDLE {
            let step_weight = Self::removed_history_step_weight(stage);
            if !weight_meter.can_consume(step_weight.saturating_add(progress_weight)) {
                break;
            }
            weight_meter.consume(step_weight);
            match stage {
                Matrix => unreachable!("Matrix cleanup is handled before the attestation stages"),
                Attestations => {
                    let next = match last_key.take() {
                        Some(key) => SubnetConsensusAttestationData::<T>::iter_key_prefix_from(
                            (subnet_id,),
                            key,
                        )
                        .next(),
                        None => SubnetConsensusAttestationData::<T>::iter_key_prefix((subnet_id,))
                            .next(),
                    };
                    if let Some((epoch, node)) = next {
                        last_key = Some(SubnetConsensusAttestationData::<T>::hashed_key_for((
                            subnet_id, epoch, node,
                        )));
                        SubnetConsensusAttestationData::<T>::remove((subnet_id, epoch, node));
                    } else {
                        stage = Submissions;
                    }
                }
                Submissions => remove_next!(SubnetConsensusSubmission, SubmissionMaxItems),
                SubmissionMaxItems => remove_next!(SubnetConsensusSubmissionMaxItems, ProposalArgs),
                ProposalArgs => remove_next!(SubnetConsensusProposalArgs, AttestorWeights),
                AttestorWeights => remove_next!(SubnetConsensusAttestorWeights, SettlementStatus),
                SettlementStatus => remove_next!(ConsensusRoundSettlementStatus, Elections),
                Elections => {
                    let next = match last_key.take() {
                        Some(key) => {
                            SubnetElectedValidator::<T>::iter_key_prefix_from(subnet_id, key).next()
                        }
                        None => SubnetElectedValidator::<T>::iter_key_prefix(subnet_id).next(),
                    };
                    if let Some(epoch) = next {
                        last_key = Some(SubnetElectedValidator::<T>::hashed_key_for(
                            subnet_id, epoch,
                        ));
                        SubnetElectedValidator::<T>::remove(subnet_id, epoch);
                    } else {
                        weight_meter.consume(progress_weight);
                        ConsensusHistoryCleanupQueue::<T>::remove(subnet_id);
                        return;
                    }
                }
            }
        }
        if stage != original_stage {
            weight_meter.consume(progress_weight);
            ConsensusHistoryCleanupQueue::<T>::insert(subnet_id, stage);
        }
    }
}
