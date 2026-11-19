use super::*;

pub(super) fn node_reward<T: Config>() -> ConsensusOperation<T::AccountId> {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let liquidity = Network::<T>::DELEGATE_POOL_MIN_LIQUIDITY;
    ValidatorDelegateStakeBalance::<T>::insert(81, 1_000);
    ValidatorDelegateStakeShares::<T>::insert(81, 2 * liquidity);
    ValidatorDelegateStakeCirculatingShares::<T>::insert(81, liquidity);
    TotalValidatorDelegateStakeBalance::<T>::put(1_000);
    ValidatorsData::<T>::insert(
        81,
        ValidatorData {
            id: 81,
            hotkey: get_account::<T>("pending-validator", 0),
            delegate_reward_rate: Network::<T>::percentage_factor_as_u128() / 2,
            last_delegate_reward_rate_update: 0,
            delegate_account: Some(DelegateAccount {
                account_id: get_account::<T>("pending-beneficiary", 0),
                rate: Network::<T>::percentage_factor_as_u128() / 2,
            }),
        },
    );
    Network::<T>::prepare_consensus_node_reward(7, 9, 81, 400)
}

pub(super) fn subnet_reward<T: Config>() -> ConsensusOperation<T::AccountId> {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let liquidity = Network::<T>::DELEGATE_POOL_MIN_LIQUIDITY;
    TotalSubnetDelegateStakeBalance::<T>::insert(7, 1_000);
    TotalSubnetDelegateStakeShares::<T>::insert(7, 2 * liquidity);
    TotalSubnetDelegateStakeCirculatingShares::<T>::insert(7, liquidity);
    TotalDelegateStake::<T>::put(1_000);
    // Exercise balance-time checkpoint writes for an existing subnet.
    let mut subnet = SubnetData::default();
    subnet.id = 7;
    SubnetsData::<T>::insert(7, subnet);
    ConsensusOperation::SubnetDelegateReward {
        subnet_id: 7,
        generation: 0,
        amount: 400,
    }
}

/// Maximum bounded withdrawal index, with the matching reservation at its end.
pub(super) fn slash_reservations<T: Config>() -> BlockNumberFor<T> {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let deadline = T::ConsensusOperationRetryPeriod::get().saturating_add(1u32.into());
    let count = MaxPendingNodeSlashReservations::<T>::get();
    let reservations: Vec<_> = (0..count)
        .map(|i| PendingNodeSlashReservation {
            expires_at: deadline.saturating_add((count - 1 - i).into()),
            amount: 1,
        })
        .collect();
    PendingNodeSlashReservations::<T>::insert(
        7,
        9,
        BoundedVec::<_, MaxPendingNodeSlashReservations<T>>::try_from(reservations).unwrap(),
    );
    NodeSubnetStake::<T>::insert(9, 7, u128::from(count) + 1_000);
    TotalSubnetStake::<T>::insert(7, u128::from(count) + 1_000);
    TotalStake::<T>::put(u128::from(count) + 1_000);
    deadline
}

pub(super) fn slash<T: Config>() -> ConsensusOperation<T::AccountId> {
    ConsensusOperation::NodeSlash {
        subnet_id: 7,
        subnet_node_id: 9,
        amount: 1,
        attestation_percentage: 0,
    }
}

/// Append a second item so success/expiry measure updating a nonempty queue too.
pub(super) fn queue_for_one_retry<T: Config>(
    operation: ConsensusOperation<T::AccountId>,
) -> Weight {
    assert_ok!(Network::<T>::enqueue_consensus_operation(
        7,
        operation.clone()
    ));
    assert_ok!(Network::<T>::enqueue_consensus_operation(
        8,
        ConsensusOperation::OwnerReward {
            recipient: get_account::<T>("other-pending-owner", 0),
            amount: 1,
        }
    ));
    T::WeightInfo::pending_consensus_operations_empty()
        .saturating_add(T::WeightInfo::pending_consensus_operation_read())
        .saturating_add(Network::<T>::pending_consensus_operation_step_weight(
            &operation,
        ))
}
