//! Conservative resource envelopes for ABI conversion and bounded read models.
//! Network dispatch weights are charged separately and come from pallet-network.
use core::marker::PhantomData;
use frame_support::{traits::Get, weights::Weight};

pub trait WeightInfo {
    fn entry() -> Weight;
    fn input(bytes: u32) -> Weight;
    fn dispatch_info() -> Weight;
    fn read(reads: u32, bytes: u32) -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: pallet_network::Config> WeightInfo for SubstrateWeight<T> {
    fn entry() -> Weight {
        Weight::from_parts(5_000_000, 0)
    }
    fn input(bytes: u32) -> Weight {
        Weight::from_parts(20_000, 0).saturating_mul(bytes.into())
    }
    fn dispatch_info() -> Weight {
        // Node cleanup selectors can decode two bounded removal sets and perform branch reads.
        {
            use pallet_network::WeightInfo as _;
            Weight::from_parts(100_000_000, 16_384)
                .saturating_add(T::DbWeight::get().reads(32))
                .saturating_add(
                    T::WeightInfo::pending_active_removal_scan(T::MaxSubnetNodesUpperBound::get())
                        .saturating_mul(2),
                )
                .saturating_add(
                    T::WeightInfo::pending_registered_removal_scan(
                        T::MaxRegisteredNodesUpperBound::get(),
                    )
                    .saturating_mul(2),
                )
        }
    }
    fn read(reads: u32, bytes: u32) -> Weight {
        Weight::from_parts(20_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(reads.into()))
            .saturating_add(Weight::from_parts(20_000, 1).saturating_mul(bytes.into()))
            .saturating_add(Weight::from_parts(0, 3_500).saturating_mul(reads.into()))
    }
}
