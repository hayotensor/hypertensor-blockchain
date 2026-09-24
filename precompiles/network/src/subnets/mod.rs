use crate::{abi::ISubnets::ISubnetsCalls as Calls, Config};
use pallet_revive::precompiles::Error;
mod bootnodes;
mod configuration;
mod lifecycle;
mod ownership;
pub fn to_call<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    if let Some(call) = lifecycle::convert::<T>(input)? {
        return Ok(Some(call));
    }
    if let Some(call) = ownership::convert::<T>(input)? {
        return Ok(Some(call));
    }
    if let Some(call) = bootnodes::convert::<T>(input)? {
        return Ok(Some(call));
    }
    if let Some(call) = configuration::convert::<T>(input)? {
        return Ok(Some(call));
    }
    Ok(None)
}
crate::domain_precompile!(
    Subnets,
    Calls,
    crate::addresses::SUBNETS,
    to_call,
    crate::reads::subnets
);
