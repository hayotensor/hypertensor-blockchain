use crate::{abi::IStaking::IStakingCalls as Calls, Config};
use pallet_revive::precompiles::Error;
mod node_staking;
mod subnet_delegation;
mod swaps;
mod validator_delegation;
mod withdrawals;
pub fn to_call<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    if let Some(call) = node_staking::convert::<T>(input)? {
        return Ok(Some(call));
    }
    if let Some(call) = subnet_delegation::convert::<T>(input)? {
        return Ok(Some(call));
    }
    if let Some(call) = validator_delegation::convert::<T>(input)? {
        return Ok(Some(call));
    }
    if let Some(call) = swaps::convert::<T>(input)? {
        return Ok(Some(call));
    }
    if let Some(call) = withdrawals::convert::<T>(input)? {
        return Ok(Some(call));
    }
    Ok(None)
}
crate::domain_precompile!(
    Staking,
    Calls,
    crate::addresses::STAKING,
    to_call,
    crate::reads::staking
);
