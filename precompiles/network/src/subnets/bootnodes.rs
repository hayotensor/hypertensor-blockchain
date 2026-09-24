use crate::{abi::ISubnets::ISubnetsCalls as Calls, convert, Config};
use pallet_revive::precompiles::Error;
pub(super) fn convert<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    Ok(Some(match input {
        Calls::ownerAddBootnodeAccess(args) => pallet_network::Call::owner_add_bootnode_access {
            subnet_id: args.subnetId,
            new_account: convert::account(&args.newAccount),
        },
        Calls::ownerRemoveBootnodeAccess(args) => {
            pallet_network::Call::owner_remove_bootnode_access {
                subnet_id: args.subnetId,
                remove_account: convert::account(&args.removeAccount),
            }
        }
        Calls::updateBootnodes(args) => pallet_network::Call::update_bootnodes {
            subnet_id: args.subnetId,
            add: convert::bootnodes::<T>(&args.add)?,
            remove: convert::peer_set::<T>(&args.remove)?,
        },
        _ => return Ok(None),
    }))
}
