use crate::{abi::IStaking::IStakingCalls as Calls, convert, Config};
use pallet_revive::precompiles::Error;
pub(super) fn convert<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    Ok(Some(match input {
        Calls::addSubnetDelegateStake(args) => pallet_network::Call::add_subnet_delegate_stake {
            subnet_id: args.subnetId,
            stake_to_be_added: args.stakeToBeAdded,
            min_shares_out: args.minSharesOut,
        },
        Calls::transferDelegateStake(args) => pallet_network::Call::transfer_delegate_stake {
            subnet_id: args.subnetId,
            to_account_id: convert::account(&args.toAccountId),
            delegate_stake_shares_to_transfer: args.delegateStakeSharesToTransfer,
        },
        Calls::removeDelegateStake(args) => pallet_network::Call::remove_delegate_stake {
            subnet_id: args.subnetId,
            shares_to_be_removed: args.sharesToBeRemoved,
            min_balance_out: args.minBalanceOut,
        },
        _ => return Ok(None),
    }))
}
