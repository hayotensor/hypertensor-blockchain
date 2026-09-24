use crate::{abi::IOverwatch::IOverwatchCalls as Calls, convert, Config};
use pallet_revive::precompiles::Error;
pub fn to_call<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    Ok(Some(match input {
        Calls::registerOverwatchNode(args) => pallet_network::Call::register_overwatch_node {
            stake_to_be_added: args.stakeToBeAdded,
        },
        Calls::removeOverwatchNode(args) => pallet_network::Call::remove_overwatch_node {
            overwatch_node_id: args.overwatchNodeId,
        },
        Calls::updateOverwatchHotkey(args) => pallet_network::Call::update_overwatch_hotkey {
            overwatch_node_id: args.overwatchNodeId,
            new_hotkey: convert::optional_account(&args.newHotkey),
        },
        Calls::setOverwatchNodePeerId(args) => pallet_network::Call::set_overwatch_node_peer_id {
            subnet_id: args.subnetId,
            overwatch_node_id: args.overwatchNodeId,
            peer_id: convert::peer_id(&args.peerId)?,
        },
        Calls::commitOverwatchSubnetWeights(args) => {
            pallet_network::Call::commit_overwatch_subnet_weights {
                overwatch_node_id: args.overwatchNodeId,
                commit_weights: convert::commits::<T>(&args.commitWeights)?,
            }
        }
        Calls::revealOverwatchSubnetWeights(args) => {
            pallet_network::Call::reveal_overwatch_subnet_weights {
                overwatch_node_id: args.overwatchNodeId,
                reveals: convert::reveals::<T>(&args.reveals)?,
            }
        }
        _ => return Ok(None),
    }))
}
crate::domain_precompile!(
    Overwatch,
    Calls,
    crate::addresses::OVERWATCH,
    to_call,
    crate::reads::overwatch
);
