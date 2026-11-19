//! Elapsed-block EMA used exclusively by the `delegate_stake` emission factor.
//!
//! Checkpoint the old pool balance before changing it. A deposit therefore cannot buy
//! historical influence, and repeated transactions in one block cannot advance the EMA.
//! This tracks aggregate pool balances, independently of the signed net-flow EMA
//! accounting used by the separate `net_flow` factor.

use super::*;
use sp_core::U256;

impl<T: Config> Pallet<T> {
    /// Advance a Q18-scaled balance through a constant-balance interval. All products fit
    /// U256: even `u128::MAX * Q18 * Q18` is less than 2^248. Binary exponentiation takes
    /// at most 32 iterations for a u32 block gap. Each fixed-point operation rounds down.
    pub(crate) fn checkpoint_subnet_delegate_stake_ema(
        previous: SubnetDelegateStakeEma,
        old_balance: u128,
        block: u32,
        alpha: u128,
    ) -> SubnetDelegateStakeEma {
        let scale = U256::from(Self::percentage_factor_as_u128());
        if previous.last_updated_block > block
            || previous.balance > U256::from(u128::MAX) * scale
            || alpha == 0
            || alpha >= Self::percentage_factor_as_u128()
        {
            return SubnetDelegateStakeEma {
                last_updated_block: block,
                ..Default::default()
            };
        }

        let mut elapsed = block - previous.last_updated_block;
        if elapsed == 0 {
            return previous;
        }
        let mut base = scale - U256::from(alpha);
        let mut decay = scale;
        while elapsed != 0 {
            if elapsed & 1 != 0 {
                decay = decay * base / scale;
            }
            elapsed >>= 1;
            if elapsed != 0 {
                base = base * base / scale;
            }
        }

        SubnetDelegateStakeEma {
            last_updated_block: block,
            balance: (previous.balance * decay + U256::from(old_balance) * scale * (scale - decay))
                / scale,
        }
    }

    /// Read an existing subnet's checkpoint, without inventing history for a missing record.
    /// Callers already establish subnet existence; allocation and governance can reuse alpha.
    pub(crate) fn read_subnet_delegate_stake_ema(
        subnet_id: u32,
        old_balance: u128,
        block: u32,
        alpha: u128,
    ) -> SubnetDelegateStakeEma {
        let previous =
            SubnetDelegateStakeEmas::<T>::get(subnet_id).unwrap_or(SubnetDelegateStakeEma {
                last_updated_block: block,
                ..Default::default()
            });
        Self::checkpoint_subnet_delegate_stake_ema(previous, old_balance, block, alpha)
    }

    /// Prepare before principal-bearing writes. Withdrawals from a deleted subnet must not
    /// recreate reward accounting. The caller saves this only when the balance change succeeds.
    pub(crate) fn prepare_subnet_delegate_stake_ema(
        subnet_id: u32,
        old_balance: u128,
        block: u32,
    ) -> Option<SubnetDelegateStakeEma> {
        if !SubnetsData::<T>::contains_key(subnet_id) {
            return None;
        }
        Some(Self::read_subnet_delegate_stake_ema(
            subnet_id,
            old_balance,
            block,
            SubnetDelegateStakeSmoothingAlpha::<T>::get(),
        ))
    }
}
