use crate::{abi::IStaking::IStakingCalls as Calls, Config};
use pallet_revive::precompiles::Error;
pub(super) fn convert<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    Ok(Some(match input {
        Calls::addNodeStake(args) => pallet_network::Call::add_node_stake {
            subnet_id: args.subnetId,
            subnet_node_id: args.subnetNodeId,
            stake_to_be_added: args.stakeToBeAdded,
        },
        Calls::removeNodeStake(args) => pallet_network::Call::remove_node_stake {
            subnet_id: args.subnetId,
            subnet_node_id: args.subnetNodeId,
            stake_to_be_removed: args.stakeToBeRemoved,
        },
        Calls::addOverwatchNodeStake(args) => pallet_network::Call::add_overwatch_node_stake {
            overwatch_node_id: args.overwatchNodeId,
            stake_to_be_added: args.stakeToBeAdded,
        },
        Calls::removeOverwatchNodeStake(args) => {
            pallet_network::Call::remove_overwatch_node_stake {
                overwatch_node_id: args.overwatchNodeId,
                stake_to_be_removed: args.stakeToBeRemoved,
            }
        }
        _ => return Ok(None),
    }))
}
