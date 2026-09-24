use crate::{abi::ISubnetNodes::ISubnetNodesCalls as Calls, convert, Config};
use pallet_revive::precompiles::Error;
pub fn to_call<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    Ok(Some(match input {
        Calls::registerSubnetNode(args) => pallet_network::Call::register_subnet_node {
            validator_id: args.validatorId,
            subnet_id: args.subnetId,
            hotkey: convert::optional_account(&args.hotkey),
            peer_info: convert::peer_info::<T>(&args.peerInfo)?,
            bootnode_peer_info: convert::peer_info::<T>(&args.bootnodePeerInfo)?,
            client_peer_info: convert::peer_info::<T>(&args.clientPeerInfo)?,
            stake_to_be_added: args.stakeToBeAdded,
            unique: convert::network_bytes::<T>(&args.unique)?,
            non_unique: convert::network_bytes::<T>(&args.nonUnique)?,
            max_burn_amount: args.maxBurnAmount,
        },
        Calls::updateNodeHotkey(args) => pallet_network::Call::update_node_hotkey {
            subnet_id: args.subnetId,
            subnet_node_id: args.subnetNodeId,
            new_hotkey: convert::optional_account(&args.newHotkey),
        },
        Calls::updateNodePeerInfo(args) => pallet_network::Call::update_node_peer_info {
            subnet_id: args.subnetId,
            subnet_node_id: args.subnetNodeId,
            new_peer_info: convert::peer_info::<T>(&args.newPeerInfo)?,
        },
        Calls::updateNodeBootnodePeerInfo(args) => {
            pallet_network::Call::update_node_bootnode_peer_info {
                subnet_id: args.subnetId,
                subnet_node_id: args.subnetNodeId,
                new_peer_info: convert::peer_info::<T>(&args.newPeerInfo)?,
            }
        }
        Calls::updateNodeClientPeerInfo(args) => {
            pallet_network::Call::update_node_client_peer_info {
                subnet_id: args.subnetId,
                subnet_node_id: args.subnetNodeId,
                new_peer_info: convert::peer_info::<T>(&args.newPeerInfo)?,
            }
        }
        Calls::updateNodeUnique(args) => pallet_network::Call::update_node_unique {
            subnet_id: args.subnetId,
            subnet_node_id: args.subnetNodeId,
            unique: convert::network_bytes::<T>(&args.unique)?,
        },
        Calls::updateNodeNonUnique(args) => pallet_network::Call::update_node_non_unique {
            subnet_id: args.subnetId,
            subnet_node_id: args.subnetNodeId,
            non_unique: convert::network_bytes::<T>(&args.nonUnique)?,
        },
        Calls::removeSubnetNode(args) => pallet_network::Call::remove_subnet_node {
            subnet_id: args.subnetId,
            subnet_node_id: args.subnetNodeId,
        },
        _ => return Ok(None),
    }))
}
crate::domain_precompile!(
    SubnetNodes,
    Calls,
    crate::addresses::SUBNET_NODES,
    to_call,
    crate::reads::subnet_nodes
);
