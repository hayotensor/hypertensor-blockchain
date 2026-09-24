//! Overwatch contract queries.
use super::*;

pub fn overwatch<T: Config>(
    input: &abi::IOverwatch::IOverwatchCalls,
    env: &mut impl Ext<T = T>,
) -> Result<Vec<u8>, Error> {
    use abi::IOverwatch::IOverwatchCalls as C;
    let (reads, bytes) = match input {
        C::getEpoch(_) => (4, 128),
        C::getOverwatchNode(_) => (10, 1024),
        C::getCommitment(_) => (
            2,
            T::MaxPhysicalSubnetsUpperBound::get()
                .saturating_mul(64)
                .saturating_add(128),
        ),
        C::getEffectiveSubnetWeight(_) => (
            3,
            T::MaxPhysicalSubnetsUpperBound::get()
                .saturating_mul(64)
                .saturating_add(256),
        ),
        _ => return Err(unknown()),
    };
    charge::<T>(env, reads, bytes)?;
    Ok(match input {
        C::getOverwatchNode(a) => {
            // Do not expand the RPC's subnet peer list for this individual identity lookup.
            let identity =
                n::Pallet::<T>::get_active_overwatch_validator_id_and_hotkey(a.overwatchNodeId)
                    .ok();
            let value = identity.and_then(|(validator_id, hotkey)| {
                let coldkey = n::ValidatorColdkey::<T>::get(validator_id)?;
                n::Pallet::<T>::ensure_canonical_validator_coldkey(&coldkey, validator_id).ok()?;
                Some(abi::OverwatchNodeInfo {
                    id: a.overwatchNodeId,
                    validatorId: validator_id,
                    coldkey: convert::account_out(coldkey),
                    hotkey: convert::account_out(hotkey),
                    stakeBalance: n::OverwatchNodeStakeBalance::<T>::get(a.overwatchNodeId),
                })
            });
            (value.is_some(), value.unwrap_or_default()).abi_encode_params()
        }
        C::getEpoch(_) => abi::OverwatchEpoch {
            epoch: n::CurrentOverwatchEpoch::<T>::get(),
            startBlock: n::OverwatchEpochStartBlock::<T>::get(),
            lengthMultiplier: n::ActiveOverwatchEpochLengthMultiplier::<T>::get(),
            commitCutoffPercent: n::ActiveOverwatchCommitCutoffPercent::<T>::get(),
        }
        .abi_encode(),
        C::getCommitment(a) => {
            let commits = n::OverwatchCommits::<T>::get(
                n::CurrentOverwatchEpoch::<T>::get(),
                a.overwatchNodeId,
            );
            let value = commits.get(&a.subnetId);
            (
                value.is_some(),
                pallet_revive::precompiles::alloy::primitives::B256::from(
                    value.map(|v| v.0).unwrap_or_default(),
                ),
            )
                .abi_encode_params()
        }
        C::getEffectiveSubnetWeight(a) => {
            let v = n::Pallet::<T>::rpc_get_effective_overwatch_subnet_weight(a.subnetId);
            abi::EffectiveWeight {
                rawWeightExists: v.raw_weight_exists,
                rawWeight: v.raw_weight.0,
                resolvedWeight: v.resolved_weight.0,
            }
            .abi_encode()
        }
        _ => return Err(unknown()),
    })
}
