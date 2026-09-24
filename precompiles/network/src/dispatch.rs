//! All stateful entrypoints use the same origin, filtering and resource accounting.
use crate::{convert::revert, weights::WeightInfo, Config};
use alloc::vec::Vec;
use frame_support::dispatch::GetDispatchInfo;
use pallet_revive::precompiles::{Error, Ext};
use sp_runtime::traits::Dispatchable;

pub fn check_context<T: Config>(env: &impl Ext<T = T>, mutation: bool) -> Result<(), Error> {
    if env.is_delegate_call() {
        return Err(revert("Network: delegate call denied"));
    }
    if !env.value_transferred().is_zero() {
        return Err(revert("Network: nonpayable"));
    }
    if mutation && env.is_read_only() {
        return Err(revert("Network: state change denied"));
    }
    Ok(())
}

pub fn dispatch<T: Config>(
    call: pallet_network::Call<T>,
    env: &mut impl Ext<T = T>,
) -> Result<Vec<u8>, Error> {
    check_context(env, true)?;
    let caller = env
        .caller()
        .account_id()
        .map_err(|_| revert("Network: signed caller required"))?
        .clone();
    // Charge before get_dispatch_info: some network annotations read storage.
    env.charge(T::PrecompileWeightInfo::dispatch_info())?;
    let call = T::network_call(call);
    let info = call.get_dispatch_info();
    let charged = env.charge(info.call_weight)?;
    // Dispatchable::dispatch preserves the runtime BaseCallFilter (including TxPause).
    let result = call.dispatch(frame_system::RawOrigin::Signed(caller).into());
    let actual = match &result {
        Ok(post) => post.calc_actual_weight(&info),
        Err(err) => err.post_info.calc_actual_weight(&info),
    };
    env.adjust_gas(charged, actual);
    result.map_err(|err| {
        let message = match err.error {
            sp_runtime::DispatchError::Module(module) => {
                module.message.unwrap_or("unknown pallet error")
            }
            other => other.into(),
        };
        Error::Revert(alloc::format!("Network: {message}").into())
    })?;
    Ok(Vec::new())
}

#[macro_export]
macro_rules! domain_precompile {
    ($name:ident, $calls:ty, $id:expr, $convert:ident, $read:path) => {
        pub struct $name<T>(core::marker::PhantomData<T>);
        impl<T: $crate::Config> pallet_revive::precompiles::Precompile for $name<T> {
            type T = T;
            type Interface = $crate::input::MeteredInput<$calls>;
            const MATCHER: pallet_revive::precompiles::AddressMatcher =
                $crate::addresses::matcher($id);
            const HAS_CONTRACT_INFO: bool = false;
            fn call(
                _address: &[u8; 20],
                input: &Self::Interface,
                env: &mut impl pallet_revive::precompiles::Ext<T = T>,
            ) -> Result<alloc::vec::Vec<u8>, pallet_revive::precompiles::Error> {
                let input = input.decode(env)?;
                if let Some(call) = $convert::<T>(&input)? {
                    $crate::dispatch::dispatch(call, env)
                } else {
                    use $read as read_impl;
                    read_impl::<T>(&input, env)
                }
            }
        }
    };
}
