pub mod attestation;
pub mod matrix;

use crate::*;

impl<T: Config> Pallet<T> {
    /// Only an existing subnet configured for the requested mechanism matches.
    pub(crate) fn subnet_uses_consensus(subnet_id: u32, mechanism: ConsensusMechanism) -> bool {
        SubnetsData::<T>::get(subnet_id)
            .is_some_and(|subnet| subnet.consensus_mechanism == mechanism)
    }
}
