//! Benchmark registration and supporting pallet configuration.

use super::*;

impl frame_benchmarking::baseline::Config for Runtime {}
impl frame_system_benchmarking::Config for Runtime {}
impl pallet_session_benchmarking::Config for Runtime {
    fn generate_session_keys_and_proof(owner: AccountId) -> (opaque::SessionKeys, Vec<u8>) {
        let generated = opaque::SessionKeys::generate(&owner.encode(), None);
        (generated.keys, generated.proof.encode())
    }
}

frame_benchmarking::define_benchmarks!(
    [frame_benchmarking, BaselineBench::<Runtime>]
    [frame_system, SystemBench::<Runtime>]
    [frame_system_extensions, SystemExtensionsBench::<Runtime>]
    [pallet_session, SessionBench::<Runtime>]
    [pallet_balances, Balances]
    [pallet_timestamp, Timestamp]
    [pallet_babe, Babe]
    [pallet_grandpa, Grandpa]
    [pallet_staking, Staking]
    [pallet_sudo, Sudo]
    [pallet_collective, Collective]
    [pallet_network, Network]
    [pallet_revive, Revive]
    [network_precompiles, NetworkPrecompiles]
    // [pallet_treasury, Treasury]
);
