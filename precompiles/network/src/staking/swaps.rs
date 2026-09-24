use crate::{abi::IStaking::IStakingCalls as Calls, convert, Config};
use pallet_revive::precompiles::Error;
pub(super) fn convert<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    Ok(Some(match input {
        Calls::swapFromSubnetToSubnet(args) => pallet_network::Call::swap_from_subnet_to_subnet {
            from_subnet_id: args.fromSubnetId,
            to_subnet_id: args.toSubnetId,
            delegate_stake_shares_to_swap: args.delegateStakeSharesToSwap,
            min_balance_out: args.minBalanceOut,
            min_shares_out: args.minSharesOut,
            execute_before_block: args.executeBeforeBlock,
        },
        Calls::swapFromValidatorToValidator(args) => {
            pallet_network::Call::swap_from_validator_to_validator {
                from_validator_id: args.fromValidatorId,
                to_validator_id: args.toValidatorId,
                stake_to_be_removed: args.stakeToBeRemoved,
                min_balance_out: args.minBalanceOut,
                min_shares_out: args.minSharesOut,
                execute_before_block: args.executeBeforeBlock,
            }
        }
        Calls::swapFromValidatorToSubnet(args) => {
            pallet_network::Call::swap_from_validator_to_subnet {
                from_validator_id: args.fromValidatorId,
                to_subnet_id: args.toSubnetId,
                node_delegate_stake_shares_to_swap: args.nodeDelegateStakeSharesToSwap,
                min_balance_out: args.minBalanceOut,
                min_shares_out: args.minSharesOut,
                execute_before_block: args.executeBeforeBlock,
            }
        }
        Calls::swapFromSubnetToValidator(args) => {
            pallet_network::Call::swap_from_subnet_to_validator {
                from_subnet_id: args.fromSubnetId,
                to_validator_id: args.toValidatorId,
                subnet_delegate_stake_shares_to_swap: args.subnetDelegateStakeSharesToSwap,
                min_balance_out: args.minBalanceOut,
                min_shares_out: args.minSharesOut,
                execute_before_block: args.executeBeforeBlock,
            }
        }
        Calls::updateSwapQueue(args) => pallet_network::Call::update_swap_queue {
            id: args.id,
            new_call: convert::queued_swap(&args.newCall)?,
        },
        _ => return Ok(None),
    }))
}
