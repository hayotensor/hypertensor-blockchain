//! Subnets contract queries.
use super::*;

pub fn subnets<T: Config>(
    input: &abi::ISubnets::ISubnetsCalls,
    env: &mut impl Ext<T = T>,
) -> Result<Vec<u8>, Error> {
    use abi::ISubnets::ISubnetsCalls as C;
    let (reads, bytes) = match input {
        C::getOwnership(_) => (2, 128),
        C::getRegistrationCost(_) => (6, 256),
        C::getSubnet(_) => (
            16,
            T::MaxVectorLength::get()
                .saturating_mul(6)
                .saturating_add(T::MaxUrlLength::get().saturating_mul(2))
                .saturating_add(4096),
        ),
        _ => return Err(unknown()),
    };
    charge::<T>(env, reads, bytes)?;
    Ok(match input {
        C::getSubnet(a) => {
            let subnet = n::SubnetsData::<T>::get(a.subnetId);
            let exists = subnet.is_some();
            let info = subnet
                .map(|s| abi::SubnetInfo {
                    id: s.id,
                    state: s.state as u8,
                    name: s.name.into(),
                    repo: s.repo.into(),
                    description: s.description.into(),
                    misc: s.misc.into(),
                    minStake: n::SubnetMinStakeBalance::<T>::get(a.subnetId),
                    maxStake: n::SubnetMaxStakeBalance::<T>::get(a.subnetId),
                    delegateStakePercentage:
                        n::Pallet::<T>::get_subnet_delegate_stake_rewards_percentage_for_epoch(
                            a.subnetId,
                            n::Pallet::<T>::get_current_subnet_epoch_as_u32(a.subnetId),
                        ),
                    reputation: n::SubnetReputation::<T>::get(a.subnetId),
                })
                .unwrap_or_default();
            (exists, info).abi_encode_params()
        }
        C::getOwnership(a) => (
            convert::account_option_out(n::SubnetOwner::<T>::get(a.subnetId)),
            convert::account_option_out(n::PendingSubnetOwner::<T>::get(a.subnetId)),
        )
            .abi_encode_params(),
        C::getRegistrationCost(_) => n::Pallet::<T>::get_current_registration_cost(
            n::Pallet::<T>::get_current_block_as_u32(),
        )
        .abi_encode(),
        _ => return Err(unknown()),
    })
}
