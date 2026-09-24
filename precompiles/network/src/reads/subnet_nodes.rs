//! Subnet Nodes contract queries.
use super::*;

pub fn subnet_nodes<T: Config>(
    input: &abi::ISubnetNodes::ISubnetNodesCalls,
    env: &mut impl Ext<T = T>,
) -> Result<Vec<u8>, Error> {
    use abi::ISubnetNodes::ISubnetNodesCalls as C;
    let (reads, bytes) = match input {
        C::getAssociatedValidator(_) => (1, 64),
        C::getSubnetNode(_) => (
            32,
            T::MaxVectorLength::get()
                .saturating_mul(12)
                .saturating_add(8192),
        ),
        _ => return Err(unknown()),
    };
    charge::<T>(env, reads, bytes)?;
    Ok(match input {
        C::getSubnetNode(a) => {
            let node = n::Pallet::<T>::rpc_get_subnet_node_info(a.subnetId, a.subnetNodeId);
            let exists = node.is_some();
            let info = node
                .map(|v| abi::SubnetNodeInfo {
                    subnetId: v.subnet_id,
                    subnetNodeId: v.subnet_node_id,
                    validatorId: v.validator_id,
                    coldkey: convert::account_out(v.coldkey),
                    hotkey: convert::account_out(v.hotkey),
                    nodeClass: v.classification.node_class as u8,
                    classStartEpoch: v.classification.start_epoch,
                    peerInfo: peer_out(v.peer_info),
                    bootnodePeerInfo: peer_out(v.bootnode_peer_info),
                    clientPeerInfo: peer_out(v.client_peer_info),
                    unique: bytes_out(v.unique),
                    nonUnique: bytes_out(v.non_unique),
                    stakeBalance: v.stake_balance.0,
                })
                .unwrap_or_default();
            (exists, info).abi_encode_params()
        }
        C::getAssociatedValidator(a) => convert::u32_option_out(
            n::SubnetNodeValidatorId::<T>::get(a.subnetId, a.subnetNodeId),
        )
        .abi_encode(),
        _ => return Err(unknown()),
    })
}
