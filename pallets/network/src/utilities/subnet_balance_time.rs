use super::*;
use frame_support::pallet_prelude::DispatchError;
use sp_core::U256;
use sp_runtime::ArithmeticError;

impl<T: Config> Pallet<T> {
    /// Accumulate the unchanged balance up to `block`, retaining only the current and previous
    /// general epochs. Skipped epochs are handled in constant time because the caller checkpoints
    /// every balance change.
    pub(crate) fn checkpoint_subnet_balance_time(
        previous: SubnetBalanceTime,
        old_balance: u128,
        block: u32,
    ) -> Result<SubnetBalanceTime, DispatchError> {
        let epoch_length = T::EpochLength::get();
        ensure!(epoch_length != 0, ArithmeticError::DivisionByZero);
        let elapsed = block
            .checked_sub(previous.last_updated_block)
            .ok_or(ArithmeticError::Underflow)?;
        let previous_epoch = previous.last_updated_block / epoch_length;
        let current_epoch = block / epoch_length;
        let balance = U256::from(old_balance);
        let balance_time = |blocks: u32| {
            balance
                .checked_mul(U256::from(blocks))
                .ok_or(ArithmeticError::Overflow)
        };

        if current_epoch == previous_epoch {
            return Ok(SubnetBalanceTime {
                last_updated_block: block,
                current_epoch_area: previous
                    .current_epoch_area
                    .checked_add(balance_time(elapsed)?)
                    .ok_or(ArithmeticError::Overflow)?,
                previous_epoch_area: previous.previous_epoch_area,
            });
        }

        let current_epoch_start = block - block % epoch_length;
        let previous_epoch_area = if current_epoch - previous_epoch == 1 {
            previous
                .current_epoch_area
                .checked_add(balance_time(
                    current_epoch_start - previous.last_updated_block,
                )?)
                .ok_or(ArithmeticError::Overflow)?
        } else {
            // The whole immediately preceding epoch elapsed without a balance change.
            balance_time(epoch_length)?
        };

        Ok(SubnetBalanceTime {
            last_updated_block: block,
            current_epoch_area: balance_time(block - current_epoch_start)?,
            previous_epoch_area,
        })
    }

    /// Prepare accounting before any principal-bearing writes. Deleted subnets retain withdrawable
    /// balances but must not recreate emission accounting after their subnet state was removed.
    pub(crate) fn prepare_subnet_balance_time(
        subnet_id: u32,
        old_balance: u128,
        block: u32,
    ) -> Result<Option<SubnetBalanceTime>, DispatchError> {
        if !SubnetsData::<T>::contains_key(subnet_id) {
            return Ok(None);
        }
        let previous =
            SubnetBalanceTimes::<T>::get(subnet_id).ok_or(Error::<T>::MissingSubnetBalanceTime)?;
        Self::checkpoint_subnet_balance_time(previous, old_balance, block).map(Some)
    }
}
