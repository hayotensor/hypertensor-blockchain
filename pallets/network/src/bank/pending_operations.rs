use super::*;
use frame_support::{
    pallet_prelude::DispatchError,
    storage::{with_transaction, TransactionOutcome},
    weights::Weight,
};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::ArithmeticError;

pub(crate) type NodeRewardAllocations<AccountId> =
    (u128, Option<(u32, u128)>, Option<(AccountId, u128)>);

impl<T: Config> Pallet<T> {
    pub(crate) fn defer_consensus_operation(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        operation: ConsensusOperation<T::AccountId>,
    ) {
        weight_meter.consume(T::WeightInfo::pending_consensus_operation_enqueue());
        if matches!(operation, ConsensusOperation::NodeSlash { .. }) {
            weight_meter.consume(T::WeightInfo::pending_node_slash_reservation_update());
        }
        if let Err(error) = Self::enqueue_consensus_operation(subnet_id, operation) {
            log::error!(target: "runtime::network", "Failed to retain consensus financial operation for subnet {subnet_id}: {error:?}");
        }
    }

    pub(crate) fn prepare_consensus_node_reward(
        subnet_id: u32,
        subnet_node_id: u32,
        validator_id: u32,
        amount: u128,
    ) -> ConsensusOperation<T::AccountId> {
        let mut validator_pool = None;
        let mut delegate_account = None;
        if let Ok(validator) = ValidatorsData::<T>::try_get(validator_id) {
            if validator.delegate_reward_rate != 0
                && Self::delegate_pool_has_circulating_shares(
                    ValidatorDelegateStakeCirculatingShares::<T>::get(validator_id),
                )
            {
                validator_pool = Some(ConsensusValidatorPoolAllocation {
                    validator_id,
                    generation: ValidatorDelegatePoolGeneration::<T>::get(validator_id),
                    rate: validator.delegate_reward_rate,
                });
            }
            delegate_account = validator
                .delegate_account
                .map(|account| (account.account_id, account.rate));
        }
        ConsensusOperation::NodeReward {
            subnet_id,
            subnet_node_id,
            amount,
            validator_pool,
            delegate_account,
        }
    }

    pub(crate) fn enqueue_consensus_operation(
        subnet_id: u32,
        operation: ConsensusOperation<T::AccountId>,
    ) -> DispatchResult {
        let expires_at = frame_system::Pallet::<T>::block_number()
            .saturating_add(T::ConsensusOperationRetryPeriod::get());
        with_transaction(|| {
            if let ConsensusOperation::NodeSlash {
                subnet_id,
                subnet_node_id,
                amount,
                ..
            } = &operation
            {
                if let Err(error) = Self::reserve_pending_node_slash(
                    *subnet_id,
                    *subnet_node_id,
                    *amount,
                    expires_at,
                ) {
                    return TransactionOutcome::Rollback(Err(error));
                }
            }
            let mut queue = PendingConsensusOperationsQueue::<T>::get();
            PendingConsensusOperations::<T>::insert(
                queue.tail,
                PendingConsensusOperation {
                    operation: operation.clone(),
                    expires_at,
                },
            );
            queue.tail = queue.tail.wrapping_add(1);
            PendingConsensusOperationsQueue::<T>::put(queue);
            Self::deposit_event(Event::ConsensusOperationQueued {
                subnet_id,
                operation,
                expires_at,
            });
            TransactionOutcome::Commit(Ok(()))
        })
    }

    pub(crate) fn pending_node_slash_locked_amount(subnet_id: u32, node_id: u32) -> u128 {
        let block = frame_system::Pallet::<T>::block_number();
        PendingNodeSlashReservations::<T>::get(subnet_id, node_id)
            .iter()
            .filter(|reservation| block < reservation.expires_at)
            .fold(0u128, |total, reservation| {
                total.saturating_add(reservation.amount)
            })
    }

    pub(crate) fn reserve_pending_node_slash(
        subnet_id: u32,
        node_id: u32,
        amount: u128,
        expires_at: BlockNumberFor<T>,
    ) -> DispatchResult {
        let block = frame_system::Pallet::<T>::block_number();
        let mut reservations = PendingNodeSlashReservations::<T>::get(subnet_id, node_id);
        reservations.retain(|reservation| block < reservation.expires_at);
        let locked = reservations.iter().try_fold(0u128, |total, reservation| {
            total
                .checked_add(reservation.amount)
                .ok_or(ArithmeticError::Overflow)
        })?;
        ensure!(
            locked
                .checked_add(amount)
                .ok_or(ArithmeticError::Overflow)?
                <= NodeSubnetStake::<T>::get(node_id, subnet_id),
            Error::<T>::NotEnoughStakeToWithdraw
        );
        if let Some(reservation) = reservations
            .iter_mut()
            .find(|reservation| reservation.expires_at == expires_at)
        {
            reservation.amount = reservation
                .amount
                .checked_add(amount)
                .ok_or(ArithmeticError::Overflow)?;
        } else {
            reservations
                .try_push(PendingNodeSlashReservation { expires_at, amount })
                .map_err(|_| Error::<T>::PendingNodeSlashReservationLimit)?;
        }
        PendingNodeSlashReservations::<T>::insert(subnet_id, node_id, reservations);
        Ok(())
    }

    fn release_pending_node_slash(
        pending: &PendingConsensusOperation<T::AccountId, BlockNumberFor<T>>,
        block: BlockNumberFor<T>,
    ) -> DispatchResult {
        let ConsensusOperation::NodeSlash {
            subnet_id,
            subnet_node_id,
            amount,
            ..
        } = &pending.operation
        else {
            return Ok(());
        };
        let mut reservations = PendingNodeSlashReservations::<T>::get(subnet_id, subnet_node_id);
        reservations.retain(|reservation| block < reservation.expires_at);
        if block < pending.expires_at {
            let reservation = reservations
                .iter_mut()
                .find(|reservation| reservation.expires_at == pending.expires_at)
                .ok_or(Error::<T>::PendingNodeSlashReservationMissing)?;
            reservation.amount = reservation
                .amount
                .checked_sub(*amount)
                .ok_or(ArithmeticError::Underflow)?;
            reservations.retain(|reservation| reservation.amount != 0);
        }
        if reservations.is_empty() {
            PendingNodeSlashReservations::<T>::remove(subnet_id, subnet_node_id);
        } else {
            PendingNodeSlashReservations::<T>::insert(subnet_id, subnet_node_id, reservations);
        }
        Ok(())
    }

    pub(crate) fn consensus_node_reward_allocations(
        amount: u128,
        validator_pool: &Option<ConsensusValidatorPoolAllocation>,
        delegate_account: &Option<(T::AccountId, u128)>,
    ) -> Result<NodeRewardAllocations<T::AccountId>, DispatchError> {
        let mut node_amount = amount;
        let pool = if let Some(pool) = validator_pool {
            let amount = Self::percent_mul(node_amount, pool.rate);
            node_amount = node_amount
                .checked_sub(amount)
                .ok_or(ArithmeticError::Underflow)?;
            Some((pool.validator_id, amount))
        } else {
            None
        };
        let account = if let Some((recipient, rate)) = delegate_account {
            let amount = Self::percent_mul(node_amount, *rate);
            node_amount = node_amount
                .checked_sub(amount)
                .ok_or(ArithmeticError::Underflow)?;
            Some((recipient.clone(), amount))
        } else {
            None
        };
        Ok((node_amount, pool, account))
    }

    /// Every component executes atomically, including all destinations in one node reward.
    pub(crate) fn try_apply_consensus_operation(
        operation: &ConsensusOperation<T::AccountId>,
    ) -> DispatchResult {
        with_transaction(|| {
            let result = (|| -> DispatchResult {
                match operation {
                    ConsensusOperation::OwnerReward { recipient, amount } => {
                        Self::handle_subnet_owner_reward(
                            &mut WeightMeter::new(),
                            recipient,
                            *amount,
                        )
                    }
                    ConsensusOperation::NodeReward {
                        subnet_id,
                        subnet_node_id,
                        amount,
                        validator_pool,
                        delegate_account,
                    } => {
                        let (node_amount, pool_amount, account_amount) =
                            Self::consensus_node_reward_allocations(
                                *amount,
                                validator_pool,
                                delegate_account,
                            )?;
                        if let Some(pool) = validator_pool {
                            ensure!(
                                ValidatorDelegatePoolGeneration::<T>::get(pool.validator_id)
                                    == pool.generation,
                                Error::<T>::ConsensusOperationPoolGenerationMismatch
                            );
                        }
                        if let Some((validator_id, amount)) = pool_amount {
                            Self::do_increase_validator_delegate_stake(validator_id, amount)?;
                        }
                        if let Some((recipient, amount)) = account_amount {
                            Self::increase_delegate_account_balance(&recipient, amount)?;
                        }
                        Self::increase_node_stake(*subnet_node_id, *subnet_id, node_amount)
                    }
                    ConsensusOperation::ValidatorReward {
                        subnet_id,
                        subnet_node_id,
                        amount,
                    } => Self::increase_node_stake(*subnet_node_id, *subnet_id, *amount),
                    ConsensusOperation::SubnetDelegateReward {
                        subnet_id,
                        generation,
                        amount,
                    } => {
                        ensure!(
                            SubnetDelegatePoolGeneration::<T>::get(subnet_id) == *generation,
                            Error::<T>::ConsensusOperationPoolGenerationMismatch
                        );
                        Self::do_increase_delegate_stake(*subnet_id, *amount)
                    }
                    ConsensusOperation::NodeSlash {
                        subnet_id,
                        subnet_node_id,
                        amount,
                        attestation_percentage,
                    } => {
                        if *amount > 0 {
                            Self::decrease_node_stake(*subnet_node_id, *subnet_id, *amount)?;
                        }
                        Self::deposit_event(Event::SubnetNodeSlashApplied {
                            subnet_id: *subnet_id,
                            subnet_node_id: *subnet_node_id,
                            attestation_percentage: *attestation_percentage,
                            node_stake_amount: *amount,
                        });
                        Ok(())
                    }
                }
            })();
            match result {
                Ok(()) => TransactionOutcome::Commit(Ok(())),
                Err(error) => TransactionOutcome::Rollback(Err(error)),
            }
        })
    }

    pub(crate) fn pending_consensus_operation_step_weight(
        operation: &ConsensusOperation<T::AccountId>,
    ) -> Weight {
        match operation {
            ConsensusOperation::OwnerReward { .. } => {
                T::WeightInfo::pending_consensus_operation_paid()
                    .max(T::WeightInfo::pending_consensus_operation_retry())
            }
            ConsensusOperation::NodeReward { .. } => {
                T::WeightInfo::pending_consensus_operation_node_reward()
            }
            ConsensusOperation::ValidatorReward { .. } => {
                T::WeightInfo::pending_consensus_operation_validator_reward()
            }
            ConsensusOperation::SubnetDelegateReward { .. } => {
                T::WeightInfo::pending_consensus_operation_subnet_delegate_reward()
            }
            ConsensusOperation::NodeSlash { .. } => {
                T::WeightInfo::pending_consensus_operation_node_slash()
            }
        }
        .max(Self::pending_consensus_operation_expiry_weight(operation))
    }

    fn pending_consensus_operation_expiry_weight(
        operation: &ConsensusOperation<T::AccountId>,
    ) -> Weight {
        if matches!(operation, ConsensusOperation::NodeSlash { .. }) {
            T::WeightInfo::pending_consensus_operation_slash_expired()
        } else {
            T::WeightInfo::pending_consensus_operation_expired()
        }
    }

    /// Read each original FIFO entry at most once and reserve its variant's weight before mutation.
    pub(crate) fn retry_pending_consensus_operations(
        block: BlockNumberFor<T>,
        weight_meter: &mut WeightMeter,
    ) {
        let selector = T::WeightInfo::pending_consensus_operations_empty();
        if !weight_meter.can_consume(selector) {
            return;
        }
        weight_meter.consume(selector);
        let mut queue = PendingConsensusOperationsQueue::<T>::get();
        let original_len = queue.tail.wrapping_sub(queue.head);
        let read_weight = T::WeightInfo::pending_consensus_operation_read();
        for _ in 0..original_len {
            if !weight_meter.can_consume(read_weight) {
                break;
            }
            weight_meter.consume(read_weight);
            let pending = PendingConsensusOperations::<T>::get(queue.head);
            let step_weight = pending
                .as_ref()
                .map(|pending| {
                    if block >= pending.expires_at {
                        Self::pending_consensus_operation_expiry_weight(&pending.operation)
                    } else {
                        Self::pending_consensus_operation_step_weight(&pending.operation)
                    }
                })
                .unwrap_or_else(T::WeightInfo::pending_consensus_operation_expired);
            if !weight_meter.can_consume(step_weight) {
                break;
            }
            weight_meter.consume(step_weight);
            let result: Result<ConsensusOperationQueueState, DispatchError> =
                with_transaction(|| {
                    let mut next = queue;
                    next.head = next.head.wrapping_add(1);
                    PendingConsensusOperations::<T>::remove(queue.head);
                    if let Some(pending) = pending {
                        if block >= pending.expires_at {
                            if let Err(error) = Self::release_pending_node_slash(&pending, block) {
                                return TransactionOutcome::Rollback(Err(error));
                            }
                            Self::deposit_event(Event::ConsensusOperationExpired {
                                operation: pending.operation,
                            });
                        } else if with_transaction(|| {
                            let result = Self::try_apply_consensus_operation(&pending.operation)
                                .and_then(|()| Self::release_pending_node_slash(&pending, block));
                            match result {
                                Ok(()) => TransactionOutcome::Commit(Ok(())),
                                Err(error) => TransactionOutcome::Rollback(Err(error)),
                            }
                        })
                        .is_ok()
                        {
                            Self::deposit_event(Event::ConsensusOperationApplied {
                                operation: pending.operation,
                            });
                        } else {
                            PendingConsensusOperations::<T>::insert(next.tail, pending);
                            next.tail = next.tail.wrapping_add(1);
                        }
                    }
                    if next.head == next.tail {
                        PendingConsensusOperationsQueue::<T>::kill();
                        next = ConsensusOperationQueueState::default();
                    } else {
                        PendingConsensusOperationsQueue::<T>::put(next);
                    }
                    TransactionOutcome::Commit(Ok(next))
                });
            match result {
                Ok(next) => queue = next,
                Err(_) => break,
            }
        }
    }
}
