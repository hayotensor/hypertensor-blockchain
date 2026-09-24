//! Staking contract queries.
use super::*;

pub fn staking<T: Config>(
    input: &abi::IStaking::IStakingCalls,
    env: &mut impl Ext<T = T>,
) -> Result<Vec<u8>, Error> {
    use abi::IStaking::IStakingCalls as C;
    let (reads, bytes) = match input {
        C::getUnbondings(_) => (
            1,
            1024u32.saturating_add(T::MaxUnbondingsUpperBound::get().saturating_mul(128)),
        ),
        C::getSubnetPool(_) | C::getValidatorPool(_) => (6, 512),
        C::getQueuedSwap(_) => (1, 512),
        C::getNodeStake(_)
        | C::getOverwatchStake(_)
        | C::getDelegateAccountBalance(_)
        | C::getSwapRefund(_) => (1, 32),
        _ => return Err(unknown()),
    };
    charge::<T>(env, reads, bytes)?;
    Ok(match input {
        C::getNodeStake(a) => n::NodeSubnetStake::<T>::get(a.subnetNodeId, a.subnetId).abi_encode(),
        C::getOverwatchStake(a) => {
            n::OverwatchNodeStakeBalance::<T>::get(a.overwatchNodeId).abi_encode()
        }
        C::getSubnetPool(a) => abi::Pool {
            balance: n::TotalSubnetDelegateStakeBalance::<T>::get(a.subnetId),
            totalShares: n::TotalSubnetDelegateStakeShares::<T>::get(a.subnetId),
            circulatingShares: n::TotalSubnetDelegateStakeCirculatingShares::<T>::get(a.subnetId),
            accountShares: n::Pallet::<T>::current_account_subnet_delegate_stake_shares(
                &convert::account(&a.accountId),
                a.subnetId,
            ),
        }
        .abi_encode(),
        C::getValidatorPool(a) => abi::Pool {
            balance: n::ValidatorDelegateStakeBalance::<T>::get(a.validatorId),
            totalShares: n::ValidatorDelegateStakeShares::<T>::get(a.validatorId),
            circulatingShares: n::ValidatorDelegateStakeCirculatingShares::<T>::get(a.validatorId),
            accountShares: n::Pallet::<T>::current_account_validator_delegate_stake_shares(
                &convert::account(&a.accountId),
                a.validatorId,
            ),
        }
        .abi_encode(),
        C::getDelegateAccountBalance(a) => {
            n::DelegateAccountStake::<T>::get(convert::account(&a.accountId)).abi_encode()
        }
        C::getSwapRefund(a) => {
            n::QueuedSwapRefundBalance::<T>::get(convert::account(&a.accountId)).abi_encode()
        }
        C::getUnbondings(a) => {
            let ledger = n::StakeUnbondingLedger::<T>::get(convert::account(&a.accountId));
            convert::limit(ledger.len(), T::MaxUnbondingsUpperBound::get())?;
            ledger
                .into_iter()
                .map(|(block, e)| abi::Unbonding {
                    unlockBlock: block,
                    network: e.network,
                    overwatch: e.overwatch,
                })
                .collect::<Vec<_>>()
                .abi_encode()
        }
        C::getQueuedSwap(a) => {
            let item = n::SwapCallQueue::<T>::get(a.id);
            let exists = item.is_some();
            let info = item
                .map(|v| abi::QueuedSwapInfo {
                    call: convert::queued_swap_out(v.call),
                    queuedAtBlock: v.queued_at_block,
                    executeAfterBlocks: v.execute_after_blocks,
                })
                .unwrap_or_default();
            (exists, info).abi_encode_params()
        }
        _ => return Err(unknown()),
    })
}
