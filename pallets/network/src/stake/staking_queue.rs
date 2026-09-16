// Copyright (C) Hypertensor.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;
impl<T: Config> Pallet<T> {
    /// Queue a swap call
    ///
    /// # Description
    ///
    /// Queues a swap call to be executed after a certain number of blocks.
    ///
    /// Used after source principal has been removed by one of the four transactional
    /// subnet/validator swap paths.
    ///
    /// # Arguments
    ///
    /// * `account_id` - Account ID of the caller.
    /// * `source` - Delegate pool whose withdrawal cooldown must be preserved.
    /// * `call` - Swap call to queue.
    ///
    pub(crate) fn queue_swap(
        account_id: T::AccountId,
        source: QueuedSwapSource,
        call: QueuedSwapCall<T::AccountId>,
    ) -> DispatchResult {
        ensure!(
            call.get_queue_account() == &account_id,
            Error::<T>::NotKeyOwner
        );

        let queued_at_block = Self::get_current_block_as_u32();
        let source_cooldown_epochs = match source {
            QueuedSwapSource::SubnetDelegate => DelegateStakeCooldownEpochs::<T>::get(),
            QueuedSwapSource::ValidatorDelegate => NodeDelegateStakeCooldownEpochs::<T>::get(),
        };
        let source_cooldown_blocks = source_cooldown_epochs
            .checked_mul(T::EpochLength::get())
            .ok_or(sp_runtime::ArithmeticError::Overflow)?;
        // A queue is never executable in less than one epoch, and it must not let either a
        // successful destination credit or an immediately claimable refund shorten the source
        // pool's withdrawal cooldown. Store the value on the item so later governance changes do
        // not retroactively alter already-debited principal.
        let execute_after_blocks = T::EpochLength::get().max(source_cooldown_blocks);
        let earliest_execution_block = queued_at_block
            .checked_add(execute_after_blocks)
            .ok_or(Error::<T>::InvalidSwapDeadline)?;
        ensure!(
            call.get_execute_before_block() >= earliest_execution_block,
            Error::<T>::InvalidSwapDeadline
        );
        ensure!(
            call.get_min_shares_out() > 0,
            Error::<T>::InvalidSwapMinimumShares
        );

        let id = NextSwapQueueId::<T>::get();
        let next_id = id.checked_add(1).ok_or(Error::<T>::SwapQueueIdExhausted)?;
        let queued_balance = call.get_queue_balance();
        ensure!(queued_balance > 0, Error::<T>::ZeroSwapBalance);
        let next_queued_principal = TotalQueuedSwapPrincipal::<T>::get()
            .checked_add(queued_balance)
            .ok_or(sp_runtime::ArithmeticError::Overflow)?;

        ensure!(
            !SwapCallQueue::<T>::contains_key(id),
            Error::<T>::SwapQueueIdExhausted
        );

        let queued_item = QueuedSwapItem {
            id,
            call: call.clone(),
            queued_at_block,
            execute_after_blocks,
        };

        let queue_count = SwapQueueOrder::<T>::try_mutate(|queue| -> Result<u32, Error<T>> {
            queue.try_push(id).map_err(|_| Error::<T>::SwapQueueFull)?;
            Ok(queue.len() as u32)
        })?;

        SwapCallQueue::<T>::insert(&id, &queued_item);
        SwapQueueCount::<T>::put(queue_count);
        NextSwapQueueId::<T>::put(next_id);
        TotalQueuedSwapPrincipal::<T>::put(next_queued_principal);

        Self::deposit_event(Event::SwapCallQueued {
            id,
            account_id,
            call: call.clone(),
        });

        Ok(())
    }

    pub(crate) fn do_update_swap_queue(
        key: T::AccountId,
        id: u32,
        new_call: QueuedSwapCall<T::AccountId>,
    ) -> DispatchResult {
        SwapCallQueue::<T>::mutate(&id, |item_opt| -> DispatchResult {
            let item = item_opt.as_mut().ok_or(Error::<T>::SwapCallNotFound)?;
            ensure!(
                item.call.get_queue_account() == &key,
                Error::<T>::NotKeyOwner
            );
            let call_balance = item.call.get_queue_balance();
            let earliest_execution_block = item
                .queued_at_block
                .checked_add(item.execute_after_blocks)
                .ok_or(Error::<T>::InvalidSwapDeadline)?;
            let next_block = Self::get_current_block_as_u32()
                .checked_add(1)
                .ok_or(Error::<T>::InvalidSwapDeadline)?;

            match new_call {
                QueuedSwapCall::SwapToSubnetDelegateStake {
                    account_id,
                    to_subnet_id,
                    balance: _,
                    min_shares_out,
                    execute_before_block,
                } => {
                    ensure!(&account_id == &key, Error::<T>::NotKeyOwner);
                    ensure!(min_shares_out > 0, Error::<T>::InvalidSwapMinimumShares);
                    ensure!(
                        execute_before_block >= earliest_execution_block
                            && execute_before_block >= next_block,
                        Error::<T>::InvalidSwapDeadline
                    );
                    ensure!(
                        SubnetsData::<T>::contains_key(to_subnet_id),
                        Error::<T>::InvalidSubnetId
                    );

                    // Update queue balance "to" subnet
                    item.call = QueuedSwapCall::SwapToSubnetDelegateStake {
                        account_id,
                        to_subnet_id,
                        balance: call_balance,
                        min_shares_out,
                        execute_before_block,
                    };

                    Self::deposit_event(Event::SwapCallQueueUpdated {
                        id,
                        account_id: key,
                        call: item.call.clone(),
                    });
                }
                QueuedSwapCall::SwapToValidatorDelegateStake {
                    account_id,
                    to_validator_id,
                    balance: _,
                    min_shares_out,
                    execute_before_block,
                } => {
                    ensure!(&account_id == &key, Error::<T>::NotKeyOwner);
                    ensure!(min_shares_out > 0, Error::<T>::InvalidSwapMinimumShares);
                    ensure!(
                        execute_before_block >= earliest_execution_block
                            && execute_before_block >= next_block,
                        Error::<T>::InvalidSwapDeadline
                    );
                    ensure!(
                        ValidatorsData::<T>::contains_key(to_validator_id),
                        Error::<T>::InvalidValidatorId
                    );

                    // Update queue balance "to" subnet node
                    item.call = QueuedSwapCall::SwapToValidatorDelegateStake {
                        account_id,
                        to_validator_id,
                        balance: call_balance,
                        min_shares_out,
                        execute_before_block,
                    };

                    Self::deposit_event(Event::SwapCallQueueUpdated {
                        id,
                        account_id: key,
                        call: item.call.clone(),
                    });
                }
            }
            Ok(())
        })?;

        Ok(())
    }
}
