use super::*;
use frame_support::weights::Weight;

impl<T: Config> Pallet<T> {
    /// Conservative queue read/decode/write envelope. Two existing 64-u32 scan models cover
    /// the 64 pairs; keep their CPU/proof allowances until this path is benchmarked separately.
    /// Reserve two extra writes for unconditional residual deletes on the full-queue fallback.
    pub(crate) fn node_metadata_cleanup_queue_weight() -> Weight {
        T::WeightInfo::pending_queued_removal_scan(64)
            .saturating_mul(2)
            .saturating_add(T::DbWeight::get().writes(3))
    }

    /// Residual cleanup is a strict subset of the existing maximum-metadata node remover,
    /// whose envelope includes hotkey, probation-progress and slot-index deletes.
    /// Retain its full measured envelope rather than inventing a smaller execution-time weight.
    pub(crate) fn node_metadata_cleanup_item_weight() -> Weight {
        T::WeightInfo::remove_admitted_subnet_node_small(1, 3)
    }

    /// Called only after the live node has been taken. Metadata updates require that live
    /// record, so the residual keys cannot change while they await cleanup. Reverse indexes
    /// remain reserved until cleanup; registration cannot reuse them in the meantime.
    pub(crate) fn defer_node_metadata_cleanup(subnet_id: u32, subnet_node_id: u32) {
        let mut queue = NodeMetadataCleanupQueue::<T>::get();
        if queue.try_push((subnet_id, subnet_node_id)).is_ok() {
            NodeMetadataCleanupQueue::<T>::put(queue);
        } else {
            // Removal and unstaking must never depend on idle work making progress.
            Self::remove_node_residuals(subnet_id, subnet_node_id);
        }
    }

    fn remove_node_residuals(subnet_id: u32, subnet_node_id: u32) {
        // These records grant no membership without the live node. Clear them even when the
        // optional metadata record is absent; stake ownership must remain available indefinitely.
        SubnetNodeIdHotkey::<T>::remove(subnet_id, subnet_node_id);
        SubnetNodeProbationProgress::<T>::remove(subnet_id, subnet_node_id);
        NodeSlotIndex::<T>::remove(subnet_id, subnet_node_id);
        let metadata = SubnetNodeMetadata::<T>::take(subnet_id, subnet_node_id).unwrap_or_default();
        if let Some(unique) = metadata.unique {
            UniqueParamSubnetNodeId::<T>::remove(subnet_id, unique);
        }
        for (role, peer) in [
            metadata.peer_info,
            metadata.bootnode_peer_info,
            metadata.client_peer_info,
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(peer) = peer {
                match role {
                    0 => PeerIdSubnetNodeId::<T>::remove(subnet_id, &peer.peer_id),
                    1 => BootnodePeerIdSubnetNodeId::<T>::remove(subnet_id, &peer.peer_id),
                    _ => ClientPeerIdSubnetNodeId::<T>::remove(subnet_id, &peer.peer_id),
                }
                if let Some(multiaddr) = peer.multiaddr {
                    MultiaddrSubnetNodeId::<T>::remove(subnet_id, multiaddr);
                }
            }
        }
    }

    pub(crate) fn cleanup_deferred_node_metadata(weight_meter: &mut WeightMeter) {
        // Reserve the final queue write before doing any cleanup, including when only a prefix
        // fits. Each item is atomic and never touches membership, reputation, ownership or funds.
        if weight_meter
            .try_consume(Self::node_metadata_cleanup_queue_weight())
            .is_err()
        {
            return;
        }
        let mut queue = NodeMetadataCleanupQueue::<T>::get();
        let mut completed = 0;
        for &(subnet_id, subnet_node_id) in &queue {
            if weight_meter
                .try_consume(Self::node_metadata_cleanup_item_weight())
                .is_err()
            {
                break;
            }
            Self::remove_node_residuals(subnet_id, subnet_node_id);
            completed += 1;
        }
        if completed != 0 {
            queue.drain(..completed);
            if queue.is_empty() {
                NodeMetadataCleanupQueue::<T>::kill();
            } else {
                NodeMetadataCleanupQueue::<T>::put(queue);
            }
        }
    }

    /// Whole-subnet removal also clears residual metadata. Charge it independently from the
    /// live-node counters that bound the ordinary subnet prefix cleanup. The extra queue envelope
    /// covers selecting this weight, in addition to the queue scan/write during execution.
    pub fn subnet_residual_metadata_cleanup_weight(subnet_id: u32) -> Weight {
        let count = NodeMetadataCleanupQueue::<T>::get()
            .iter()
            .filter(|(id, _)| *id == subnet_id)
            .count() as u64;
        Self::node_metadata_cleanup_queue_weight()
            .saturating_mul(2)
            .saturating_add(Self::node_metadata_cleanup_item_weight().saturating_mul(count))
    }

    pub(crate) fn cleanup_subnet_residual_metadata(subnet_id: u32) -> Weight {
        let mut queue = NodeMetadataCleanupQueue::<T>::get();
        let mut removed = 0u64;
        queue.retain(|&(id, node_id)| {
            if id != subnet_id {
                return true;
            }
            Self::remove_node_residuals(id, node_id);
            removed += 1;
            false
        });
        if removed != 0 {
            if queue.is_empty() {
                NodeMetadataCleanupQueue::<T>::kill();
            } else {
                NodeMetadataCleanupQueue::<T>::put(queue);
            }
        }
        Self::node_metadata_cleanup_queue_weight()
            .saturating_add(Self::node_metadata_cleanup_item_weight().saturating_mul(removed))
    }
}
