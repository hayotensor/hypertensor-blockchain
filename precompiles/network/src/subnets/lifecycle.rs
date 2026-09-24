use crate::{abi::ISubnets::ISubnetsCalls as Calls, convert, Config};
use pallet_revive::precompiles::Error;
pub(super) fn convert<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    Ok(Some(match input {
        Calls::registerSubnet(args) => pallet_network::Call::register_subnet {
            max_cost: args.maxCost,
            subnet_data: convert::registration::<T>(&args.subnetData)?,
        },
        Calls::activateSubnet(args) => pallet_network::Call::activate_subnet {
            subnet_id: args.subnetId,
        },
        Calls::ownerPauseSubnet(args) => pallet_network::Call::owner_pause_subnet {
            subnet_id: args.subnetId,
        },
        Calls::ownerUnpauseSubnet(args) => pallet_network::Call::owner_unpause_subnet {
            subnet_id: args.subnetId,
        },
        Calls::ownerDeactivateSubnet(args) => pallet_network::Call::owner_deactivate_subnet {
            subnet_id: args.subnetId,
        },
        _ => return Ok(None),
    }))
}
