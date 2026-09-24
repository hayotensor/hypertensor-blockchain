use crate::{abi::IStaking::IStakingCalls as Calls, convert, Config};
use pallet_revive::precompiles::Error;
pub(super) fn convert<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    Ok(Some(match input {
        Calls::addValidatorDelegateStake(args) => {
            pallet_network::Call::add_validator_delegate_stake {
                validator_id: args.validatorId,
                delegate_stake_to_be_added: args.delegateStakeToBeAdded,
                min_shares_out: args.minSharesOut,
            }
        }
        Calls::transferValidatorDelegateStake(args) => {
            pallet_network::Call::transfer_validator_delegate_stake {
                validator_id: args.validatorId,
                to_account_id: convert::account(&args.toAccountId),
                validator_delegate_stake_shares_to_transfer: args
                    .validatorDelegateStakeSharesToTransfer,
            }
        }
        Calls::removeValidatorDelegateStake(args) => {
            pallet_network::Call::remove_validator_delegate_stake {
                validator_id: args.validatorId,
                validator_delegate_stake_shares_to_be_removed: args
                    .validatorDelegateStakeSharesToBeRemoved,
                min_balance_out: args.minBalanceOut,
            }
        }
        _ => return Ok(None),
    }))
}
