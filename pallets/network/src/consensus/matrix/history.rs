use super::*;

impl<T: Config> Pallet<T> {
    /// Incremental deletion, sharing the network's rotating history cursors and idle budget.
    pub(crate) fn prune_matrix_round(meter: &mut WeightMeter, subnet: u32, epoch: u32) -> bool {
        let row_weight = T::DbWeight::get()
            .reads_writes(2, 1)
            .saturating_add(Weight::from_parts(2_000_000, 8_192));
        for commitments in [false, true] {
            let mut exhausted = false;
            for _ in 0..MATRIX_MAX_NODES {
                if !meter.can_consume(row_weight) {
                    return false;
                }
                meter.consume(row_weight);
                let next = if commitments {
                    MatrixCommitments::<T>::iter_key_prefix((subnet, epoch)).next()
                } else {
                    MatrixRows::<T>::iter_key_prefix((subnet, epoch)).next()
                };
                let Some(node) = next else {
                    exhausted = true;
                    break;
                };
                if commitments {
                    MatrixCommitments::<T>::remove((subnet, epoch, node));
                } else {
                    MatrixRows::<T>::remove((subnet, epoch, node));
                }
            }
            if !exhausted {
                return false;
            }
        }
        let finish = T::DbWeight::get()
            .writes(3)
            .saturating_add(Weight::from_parts(2_000_000, 32_768));
        if !meter.can_consume(finish) {
            return false;
        }
        meter.consume(finish);
        MatrixResults::<T>::remove(subnet, epoch);
        MatrixRounds::<T>::remove(subnet, epoch);
        ConsensusRoundSettlementStatus::<T>::remove(subnet, epoch);
        true
    }
}
