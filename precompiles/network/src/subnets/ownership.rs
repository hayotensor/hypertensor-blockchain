use crate::{abi::ISubnets::ISubnetsCalls as Calls, convert, Config};
use pallet_revive::precompiles::Error;
pub(super) fn convert<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    Ok(Some(match input {
        Calls::transferSubnetOwnership(args) => pallet_network::Call::transfer_subnet_ownership {
            subnet_id: args.subnetId,
            new_owner: convert::account(&args.newOwner),
        },
        Calls::acceptSubnetOwnership(args) => pallet_network::Call::accept_subnet_ownership {
            subnet_id: args.subnetId,
        },
        Calls::cancelSubnetOwnershipTransfer(args) => {
            pallet_network::Call::cancel_subnet_ownership_transfer {
                subnet_id: args.subnetId,
            }
        }
        _ => return Ok(None),
    }))
}
