use crate::{abi::IConsensus::IConsensusCalls as Calls, convert, Config};
use pallet_revive::precompiles::Error;
pub fn to_call<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    Ok(Some(match input {
        Calls::proposeAttestation(args) => pallet_network::Call::propose_attestation {
            subnet_id: args.subnetId,
            data: convert::scores::<T>(&args.data)?,
            prioritize_queue_node_id: convert::optional_u32(&args.prioritizeQueueNodeId),
            remove_queue_node_id: convert::optional_u32(&args.removeQueueNodeId),
            args: convert::validator_args::<T>(&args.args)?,
            attest_data: convert::validator_args::<T>(&args.attestData)?,
        },
        Calls::attest(args) => pallet_network::Call::attest {
            subnet_id: args.subnetId,
            subnet_node_id: args.subnetNodeId,
            data: convert::validator_args::<T>(&args.data)?,
        },
        _ => return Ok(None),
    }))
}
crate::domain_precompile!(
    Consensus,
    Calls,
    crate::addresses::CONSENSUS,
    to_call,
    crate::reads::consensus
);
