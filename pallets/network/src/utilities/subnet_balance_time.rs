//! Tracks each subnet's total delegate-pool balance over time for emission allocation.
//!
//! This supplies the time-weighted stake factor used to divide network emissions between
//! subnets. It accumulates `pool balance * blocks held`, so a last-minute deposit cannot
//! receive the same historical credit as funds held throughout the epoch. For example,
//! 1,000 tokens held for 20 blocks contribute 20,000 token-blocks; held for one block, 1,000.
//!
//! Every deposit, withdrawal, or reward checkpoints the old pool balance before changing it.
//! Tracking the combined balance avoids iterating over individual delegators. Their rewards
//! accrue through pool shares; this module does not calculate individual holding durations.
//!
//! These helpers return a calculated checkpoint without writing storage. Balance-changing
//! callers save it in `SubnetBalanceTimes` alongside their balance updates. Emission allocation
//! reads the completed epoch's accumulated balance-time to calculate the subnet's weight.
//! Checkpoints retain only the current and previous general epochs. Deleted subnets skip this
//! accounting so remaining funds can be withdrawn without recreating emission state.

use super::*;
use frame_support::pallet_prelude::DispatchError;
use sp_core::U256;

impl<T: Config> Pallet<T> {
    /// Accumulate the unchanged balance up to `block`, retaining only the current and previous
    /// general epochs. Skipped epochs are handled in constant time because the caller checkpoints
    /// every balance change.
    ///
    /// A zero epoch length or impossible history restarts with zero credit at `block`.
    /// This function always returns Ok; callers persist the returned checkpoint.
    /// A u128 balance times a u32 duration fits within 160 bits; validating the stored areas
    /// against those bounds also makes their accumulation safe in U256.
    pub(crate) fn checkpoint_subnet_balance_time(
        previous: SubnetBalanceTime,
        old_balance: u128,
        block: u32,
    ) -> Result<SubnetBalanceTime, DispatchError> {
        let epoch_length = T::EpochLength::get();
        if epoch_length == 0 {
            return Ok(SubnetBalanceTime {
                last_updated_block: block,
                ..Default::default()
            });
        }
        let previous_epoch = previous.last_updated_block / epoch_length;
        let max_balance = U256::from(u128::MAX);
        let max_previous_epoch_area = if previous_epoch == 0 {
            U256::zero()
        } else {
            max_balance * U256::from(epoch_length)
        };
        if previous.last_updated_block > block
            || previous.current_epoch_area
                > max_balance * U256::from(previous.last_updated_block % epoch_length)
            || previous.previous_epoch_area > max_previous_epoch_area
        {
            return Ok(SubnetBalanceTime {
                last_updated_block: block,
                ..Default::default()
            });
        }

        let elapsed = block - previous.last_updated_block;
        let current_epoch = block / epoch_length;
        let balance = U256::from(old_balance);
        let balance_time = |blocks: u32| balance * U256::from(blocks);

        if current_epoch == previous_epoch {
            return Ok(SubnetBalanceTime {
                last_updated_block: block,
                current_epoch_area: previous.current_epoch_area + balance_time(elapsed),
                previous_epoch_area: previous.previous_epoch_area,
            });
        }

        let current_epoch_start = block - block % epoch_length;
        let previous_epoch_area = if current_epoch - previous_epoch == 1 {
            previous.current_epoch_area
                + balance_time(current_epoch_start - previous.last_updated_block)
        } else {
            // The whole immediately preceding epoch elapsed without a balance change.
            balance_time(epoch_length)
        };

        Ok(SubnetBalanceTime {
            last_updated_block: block,
            current_epoch_area: balance_time(block - current_epoch_start),
            previous_epoch_area,
        })
    }

    /// Prepare accounting before any principal-bearing writes. Deleted subnets retain withdrawable
    /// balances but must not recreate emission accounting after their subnet state was removed.
    /// Missing history restarts at the current block with no historical emission credit, allowing
    /// settlement and balance changes to proceed without inventing a past holding duration.
    pub(crate) fn prepare_subnet_balance_time(
        subnet_id: u32,
        old_balance: u128,
        block: u32,
    ) -> Result<Option<SubnetBalanceTime>, DispatchError> {
        if !SubnetsData::<T>::contains_key(subnet_id) {
            // SubnetBalanceTimes is removed on subnet removal
            return Ok(None);
        }
        let previous = SubnetBalanceTimes::<T>::get(subnet_id).unwrap_or(SubnetBalanceTime {
            last_updated_block: block,
            ..Default::default()
        });
        Self::checkpoint_subnet_balance_time(previous, old_balance, block).map(Some)
    }
}
