use crate::{abi::IStaking::IStakingCalls as Calls, Config};
use pallet_revive::precompiles::Error;
pub(super) fn convert<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    Ok(Some(match input {
        Calls::removeDelegateAccountBalance(args) => {
            pallet_network::Call::remove_delegate_account_balance {
                amount_to_remove: args.amountToRemove,
            }
        }
        Calls::claimUnbondings(_) => pallet_network::Call::claim_unbondings {},
        _ => return Ok(None),
    }))
}
