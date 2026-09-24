use crate::{abi::IValidators::IValidatorsCalls as Calls, convert, Config};
use pallet_revive::precompiles::Error;
pub fn to_call<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    Ok(Some(match input {
        Calls::registerValidator(args) => pallet_network::Call::register_validator {
            hotkey: convert::account(&args.hotkey),
            delegate_reward_rate: args.delegateRewardRate,
            delegate_account: convert::delegate_account(&args.delegateAccount),
            identity: convert::identity::<T>(&args.identity)?,
        },
        Calls::updateValidatorColdkey(args) => pallet_network::Call::update_validator_coldkey {
            validator_id: args.validatorId,
            new_coldkey: convert::account(&args.newColdkey),
        },
        Calls::updateValidatorHotkey(args) => pallet_network::Call::update_validator_hotkey {
            validator_id: args.validatorId,
            new_hotkey: convert::account(&args.newHotkey),
        },
        Calls::updateValidatorDelegateRewardRate(args) => {
            pallet_network::Call::update_validator_delegate_reward_rate {
                validator_id: args.validatorId,
                new_delegate_reward_rate: args.newDelegateRewardRate,
            }
        }
        Calls::updateValidatorDelegateAccount(args) => {
            pallet_network::Call::update_validator_delegate_account {
                validator_id: args.validatorId,
                delegate_account_id: convert::optional_account(&args.delegateAccountId),
                delegate_rate: convert::optional_u128(&args.delegateRate),
            }
        }
        Calls::updateValidatorIdentity(args) => pallet_network::Call::update_validator_identity {
            validator_id: args.validatorId,
            identity: convert::identity::<T>(&args.identity)?,
        },
        Calls::setValidatorNodeDelegateStakeWeights(args) => {
            pallet_network::Call::set_validator_node_delegate_stake_weights {
                updates: convert::allocations::<T>(&args.updates)?,
            }
        }
        _ => return Ok(None),
    }))
}
crate::domain_precompile!(
    Validators,
    Calls,
    crate::addresses::VALIDATORS,
    to_call,
    crate::reads::validators
);
