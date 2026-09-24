//! Validators contract queries.
use super::*;

pub fn validators<T: Config>(
    input: &abi::IValidators::IValidatorsCalls,
    env: &mut impl Ext<T = T>,
) -> Result<Vec<u8>, Error> {
    use abi::IValidators::IValidatorsCalls as C;
    let identity_bytes = T::MaxVectorLength::get()
        .saturating_mul(4)
        .saturating_add(T::MaxUrlLength::get().saturating_mul(4))
        .saturating_add(T::MaxSocialIdLength::get().saturating_mul(3))
        .saturating_add(4096);
    let (reads, bytes) = match input {
        C::getNodeAllocation(_) => (
            1,
            T::MaxValidatorNodesUpperBound::get()
                .saturating_mul(64)
                .saturating_add(128),
        ),
        C::getValidator(_) => (8, identity_bytes),
        C::getValidatorByColdkey(_) | C::getValidatorByHotkey(_) => (9, identity_bytes),
        _ => return Err(unknown()),
    };
    charge::<T>(env, reads, bytes)?;
    Ok(match input {
        C::getValidator(a) => validator_out(n::Pallet::<T>::rpc_get_validator_info(a.validatorId)),
        C::getValidatorByColdkey(a) => validator_out(n::Pallet::<T>::rpc_get_validator_by_coldkey(
            &convert::account(&a.accountId),
        )),
        C::getValidatorByHotkey(a) => validator_out(n::Pallet::<T>::rpc_get_validator_by_hotkey(
            &convert::account(&a.accountId),
        )),
        C::getNodeAllocation(a) => {
            let weights = n::ValidatorNodeDelegateStakeWeights::<T>::get(a.validatorId);
            convert::limit(weights.len(), T::MaxValidatorNodesUpperBound::get())?;
            let weight = weights.get(&(a.subnetId, a.subnetNodeId)).copied();
            (weight.is_some(), weight.unwrap_or_default()).abi_encode_params()
        }
        _ => return Err(unknown()),
    })
}
