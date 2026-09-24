//! Typed, permission-preserving Network interfaces for Revive contracts.
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

pub mod abi {
    include!(concat!(env!("OUT_DIR"), "/abi.rs"));
}
pub mod addresses;
pub mod consensus;
pub mod convert;
pub mod dispatch;
pub mod input;
pub mod overwatch;
pub mod reads;
pub mod staking;
pub mod subnet_nodes;
pub mod subnets;
pub mod validators;
pub mod weights;
pub use pallet::*;

/// Stateless FRAME component for runtime configuration and benchmark discovery.
#[frame_support::pallet]
pub mod pallet {
    #[pallet::config]
    pub trait Config:
        pallet_network::Config
        + pallet_revive::Config
        + frame_system::Config<AccountId = sp_runtime::AccountId32, Hash = sp_core::H256>
    {
        type PrecompileWeightInfo: crate::weights::WeightInfo;
        fn network_call(
            call: pallet_network::Call<Self>,
        ) -> <Self as pallet_revive::Config>::RuntimeCall;
    }
    #[pallet::pallet]
    pub struct Pallet<T>(_);
}

pub type Precompiles<T> = (
    staking::Staking<T>,
    validators::Validators<T>,
    subnets::Subnets<T>,
    subnet_nodes::SubnetNodes<T>,
    consensus::Consensus<T>,
    overwatch::Overwatch<T>,
);

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
#[cfg(test)]
mod tests;
