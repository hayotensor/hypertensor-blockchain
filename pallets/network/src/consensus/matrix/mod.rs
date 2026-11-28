//! Matrix consensus: frozen stake, committed score rows, independent accuracy references.
use crate::*;
use frame_support::{
    dispatch::{DispatchResultWithPostInfo, Pays},
    weights::Weight,
};
use sp_runtime::traits::Hash;

pub mod types;
pub use types::*;
mod history;
pub(crate) mod math;
mod settlement;

impl<T: Config> Pallet<T> {
    /// Provisional, deliberately separate from generated weights. Includes 64-node snapshot
    /// reads, hashing, score validation and trie proof allowance. Benchmark calibration is pending.
    pub fn matrix_submission_weight() -> Weight {
        Weight::from_parts(25_000_000_000, 128 * 1024)
            .saturating_add(T::DbWeight::get().reads_writes(150, 4))
    }

    pub(crate) fn matrix_slot_weight() -> Weight {
        // 0.45 seconds for bounded matrix work plus the existing 64-node financial envelope.
        let shared = Self::subnet_slot_mandatory_weight(MATRIX_MAX_NODES, 0, MATRIX_MAX_NODES);
        shared
            .saturating_add(Weight::from_parts(450_000_000_000, 2 * 1024 * 1024))
            .saturating_add(T::DbWeight::get().reads_writes(600, 150))
    }

    pub(crate) fn is_matrix_subnet(subnet_id: u32) -> bool {
        Self::subnet_uses_consensus(subnet_id, ConsensusMechanism::Matrix)
    }

    pub(crate) fn subnet_node_capacity(subnet_id: u32) -> u32 {
        let limit = MaxSubnetNodes::<T>::get();
        if Self::is_matrix_subnet(subnet_id) {
            limit.min(MatrixConfig::<T>::get().max_nodes)
        } else {
            limit
        }
    }

    /// Shared preconditions for registration and preactivation selection.
    pub(crate) fn ensure_consensus_mechanism_supported(
        mechanism: ConsensusMechanism,
        admitted_nodes: u32,
    ) -> DispatchResult {
        if mechanism == ConsensusMechanism::Matrix {
            ensure!(
                T::EpochLength::get() >= 2,
                Error::<T>::InvalidMatrixEpochLength
            );
            ensure!(
                admitted_nodes <= MatrixConfig::<T>::get().max_nodes,
                Error::<T>::MaxSubnetNodes
            );
            ensure!(
                MinSubnetNodes::<T>::get() <= MatrixConfig::<T>::get().max_nodes,
                Error::<T>::MaxSubnetNodes
            );
        }
        Ok(())
    }

    pub(crate) fn do_owner_set_consensus_mechanism(
        owner: T::AccountId,
        subnet_id: u32,
        mechanism: ConsensusMechanism,
    ) -> DispatchResult {
        ensure!(
            Self::is_subnet_owner(&owner, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );
        SubnetsData::<T>::try_mutate(subnet_id, |subnet| -> DispatchResult {
            let subnet = subnet.as_mut().ok_or(Error::<T>::InvalidSubnetId)?;
            ensure!(
                subnet.state == SubnetState::Registered,
                Error::<T>::SubnetActivatedAlready
            );
            if mechanism == ConsensusMechanism::Matrix {
                Self::ensure_consensus_mechanism_supported(
                    mechanism,
                    TotalAdmittedSubnetNodes::<T>::get(subnet_id),
                )?;
            }
            subnet.consensus_mechanism = mechanism;
            Ok(())
        })?;
        Self::deposit_event(Event::ConsensusMechanismSelected {
            subnet_id,
            mechanism,
        });
        Ok(())
    }

    pub(crate) fn do_set_matrix_config(parameters: MatrixParameters) -> DispatchResult {
        ensure!(parameters.is_valid(), Error::<T>::InvalidPercent);
        ensure!(
            parameters.commit_blocks(T::EpochLength::get()).is_some(),
            Error::<T>::InvalidMatrixEpochLength
        );
        MatrixConfig::<T>::put(parameters);
        Self::deposit_event(Event::MatrixConfigUpdated { parameters });
        Ok(())
    }

    pub(crate) fn open_matrix_round(subnet_id: u32, epoch: u32) {
        if MatrixRounds::<T>::contains_key(subnet_id, epoch) {
            return;
        }
        let Some(subnet) = SubnetsData::<T>::get(subnet_id) else {
            return;
        };
        if subnet.consensus_mechanism != ConsensusMechanism::Matrix
            || subnet.state != SubnetState::Active
            || !Self::_is_subnet_active_and_live(&subnet, epoch)
            || T::EpochLength::get() < 2
        {
            return;
        }
        let parameters = MatrixConfig::<T>::get();
        if !parameters.is_valid() {
            return;
        }
        let Some(commit_blocks) = parameters.commit_blocks(T::EpochLength::get()) else {
            return;
        };
        let pending = Self::pending_subnet_node_removal_ids(subnet_id);
        // Admitted storage is bounded by the same capacity enforced during registration/admission.
        let mut nodes: Vec<_> = SubnetNodesData::<T>::iter_prefix(subnet_id)
            .map(|(_, node)| node)
            .filter(|node| {
                node.classification.node_class != SubnetNodeClass::Queued
                    && !pending.contains(&node.id)
            })
            .take(MATRIX_MAX_NODES as usize + 1)
            .collect();
        if nodes.len() < parameters.min_nodes as usize || nodes.len() > MATRIX_MAX_NODES as usize {
            return;
        }
        nodes.sort_by_key(|node| node.id);
        let eligible = Self::effective_consensus_validator_ids(subnet_id);
        let identities: BTreeMap<_, _> = nodes
            .iter()
            .filter(|n| {
                n.classification.node_class == SubnetNodeClass::Active
                    && eligible.binary_search(&n.id).is_ok()
            })
            .map(|n| (n.id, n.validator_id))
            .collect();
        if identities.is_empty() {
            return;
        }
        let policy = Self::consensus_policy_snapshot(subnet_id, epoch);
        let stake = Self::snapshot_consensus_attestor_weights(subnet_id, &policy, &identities);
        let Some(slot) = SubnetSlot::<T>::get(subnet_id) else {
            return;
        };
        let Some(start_block) = epoch
            .checked_mul(T::EpochLength::get())
            .and_then(|b| b.checked_add(slot))
        else {
            return;
        };
        let Some(end_block) = start_block.checked_add(T::EpochLength::get()) else {
            return;
        };
        let round = MatrixRound {
            start_block,
            reveal_block: start_block + commit_blocks,
            end_block,
            nodes,
            identities,
            stake,
            policy,
            parameters,
        };
        MatrixRounds::<T>::insert(subnet_id, epoch, round);
        ConsensusRoundSettlementStatus::<T>::insert(subnet_id, epoch, false);
        Self::deposit_event(Event::MatrixRoundOpened {
            subnet_id,
            subnet_epoch: epoch,
        });
    }

    pub fn matrix_commitment(
        subnet_id: u32,
        epoch: u32,
        node: u32,
        round: &MatrixRound,
        scores: &MatrixScores,
        salt: &[u8; 32],
    ) -> T::Hash {
        let roster_hash = T::Hashing::hash_of(&round.nodes);
        T::Hashing::hash_of(&(
            b"matrix/scores/v1",
            subnet_id,
            epoch,
            node,
            roster_hash,
            scores,
            salt,
        ))
    }

    fn matrix_submission_round(
        hotkey: &T::AccountId,
        subnet_id: u32,
        epoch: u32,
        node: u32,
    ) -> Result<MatrixRound, sp_runtime::DispatchError> {
        ensure!(
            Self::is_matrix_subnet(subnet_id),
            Error::<T>::InvalidConsensusMechanism
        );
        let round =
            MatrixRounds::<T>::get(subnet_id, epoch).ok_or(Error::<T>::MatrixRoundNotFound)?;
        ensure!(
            !Self::is_consensus_round_settled(subnet_id, epoch),
            Error::<T>::MatrixRoundClosed
        );
        ensure!(
            !Self::consensus_round_precedes_network_resume(subnet_id, epoch),
            Error::<T>::ConsensusRoundCancelled
        );
        let owned = Self::ensure_hotkey_owns_subnet_node(subnet_id, node, hotkey)?;
        ensure!(
            round.identities.get(&node) == Some(&owned.validator_id),
            Error::<T>::InvalidValidatorId
        );
        ensure!(
            !Self::pending_subnet_node_removal_ids(subnet_id).contains(&node),
            Error::<T>::MatrixNodeQuarantined
        );
        Ok(round)
    }

    pub(crate) fn do_commit_matrix_scores(
        hotkey: T::AccountId,
        subnet_id: u32,
        epoch: u32,
        node: u32,
        commitment: T::Hash,
    ) -> DispatchResultWithPostInfo {
        let round = Self::matrix_submission_round(&hotkey, subnet_id, epoch, node)?;
        let block = Self::get_current_block_as_u32();
        ensure!(
            block >= round.start_block && block < round.reveal_block,
            Error::<T>::NotCommitPeriod
        );
        ensure!(
            !MatrixCommitments::<T>::contains_key((subnet_id, epoch, node)),
            Error::<T>::AlreadyCommitted
        );
        MatrixCommitments::<T>::insert((subnet_id, epoch, node), commitment);
        Self::deposit_event(Event::MatrixScoresCommitted {
            subnet_id,
            subnet_epoch: epoch,
            subnet_node_id: node,
            commitment,
        });
        Ok(Pays::No.into())
    }

    pub(crate) fn do_reveal_matrix_scores(
        hotkey: T::AccountId,
        subnet_id: u32,
        epoch: u32,
        node: u32,
        scores: MatrixScores,
        salt: [u8; 32],
    ) -> DispatchResultWithPostInfo {
        let round = Self::matrix_submission_round(&hotkey, subnet_id, epoch, node)?;
        let block = Self::get_current_block_as_u32();
        ensure!(
            block >= round.reveal_block && block < round.end_block,
            Error::<T>::NotRevealPeriod
        );
        ensure!(
            !MatrixRows::<T>::contains_key((subnet_id, epoch, node)),
            Error::<T>::AlreadyRevealed
        );
        ensure!(
            round.valid_scores(node, &scores),
            Error::<T>::InvalidMatrixScores
        );
        let commitment = MatrixCommitments::<T>::get((subnet_id, epoch, node))
            .ok_or(Error::<T>::NoCommitFound)?;
        ensure!(
            commitment == Self::matrix_commitment(subnet_id, epoch, node, &round, &scores, &salt),
            Error::<T>::RevealMismatch
        );
        let abstained = scores.is_empty();
        MatrixRows::<T>::insert((subnet_id, epoch, node), scores);
        Self::deposit_event(Event::MatrixScoresRevealed {
            subnet_id,
            subnet_epoch: epoch,
            subnet_node_id: node,
            abstained,
        });
        Ok(Pays::No.into())
    }
}
