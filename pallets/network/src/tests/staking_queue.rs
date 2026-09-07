use super::mock::*;
use crate::tests::test_utils::*;
use crate::Event;
use crate::{
    AccountSubnetDelegateStakeShares, AccountValidatorDelegateStakeShares,
    DelegateStakeCooldownEpochs, Error, MaxSubnetNodes, MaxSubnets, MaxUnbondings,
    MinDelegateStakeDeposit, MinSubnetMinStake, NextSwapQueueId, NodeDelegateStakeCooldownEpochs,
    QueuedSwapCall, QueuedSwapItem, QueuedSwapRefundBalance, QueuedSwapSource,
    StakeUnbondingLedger, SubnetName, SubnetRemovalReason, SubnetsData, SwapCallQueue,
    SwapQueueCount, SwapQueueOrder, SwapRefundReason, TotalDelegateStake,
    TotalNetworkUnbondingBalance, TotalQueuedSwapPrincipal, TotalQueuedSwapRefundBalance,
    TotalSubnetDelegateStakeBalance, TotalSubnetDelegateStakeCirculatingShares,
    TotalSubnetDelegateStakeShares, TotalValidatorDelegateStakeBalance, TxRateLimit,
    UnbondingEntry, ValidatorDelegateStakeBalance, ValidatorDelegateStakeCirculatingShares,
    ValidatorDelegateStakeShares,
};
use frame_support::assert_err;
use frame_support::assert_ok;
use frame_support::traits::Currency;
use frame_support::traits::OnInitialize;
use frame_support::weights::WeightMeter;
use sp_runtime::ArithmeticError;

//
//
//
//
//
//
//
// Staking queue
//
//
//
//
//
//
//

fn queued_swap_principal_from_items() -> u128 {
    SwapCallQueue::<Test>::iter().fold(0u128, |total, (_, item)| {
        total
            .checked_add(item.call.get_queue_balance())
            .expect("test queue principal fits u128")
    })
}

fn assert_queued_swap_principal_invariant() {
    assert_eq!(
        TotalQueuedSwapPrincipal::<Test>::get(),
        queued_swap_principal_from_items()
    );
}

fn insert_to_subnet_swap_call_queue(account_id: AccountIdOf<Test>, subnet_id: u32, balance: u128) {
    let id = NextSwapQueueId::<Test>::get();

    let call = QueuedSwapCall::SwapToSubnetDelegateStake {
        account_id: account_id,
        to_subnet_id: subnet_id,
        balance: balance,
        min_shares_out: 1,
        execute_before_block: u32::MAX,
    };

    let queued_item = QueuedSwapItem {
        id,
        call,
        queued_at_block: Network::get_current_block_as_u32(),
        execute_after_blocks: EpochLength::get(),
    };

    // Add to data storage
    SwapCallQueue::<Test>::insert(&id, &queued_item);

    // Add ID to the end of the queue
    SwapQueueOrder::<Test>::mutate(|queue| {
        queue.try_push(id).expect("test queue must have capacity");
        SwapQueueCount::<Test>::set(queue.len() as u32);
    });

    NextSwapQueueId::<Test>::mutate(|next_id| *next_id = next_id.saturating_add(1));
    TotalQueuedSwapPrincipal::<Test>::mutate(|total| {
        *total = total
            .checked_add(balance)
            .expect("test queue principal fits u128")
    });
    assert_queued_swap_principal_invariant();
}

fn insert_to_validator_swap_call_queue(
    account_id: AccountIdOf<Test>,
    validator_id: u32,
    balance: u128,
) {
    let id = NextSwapQueueId::<Test>::get();

    let call = QueuedSwapCall::SwapToValidatorDelegateStake {
        account_id: account_id,
        to_validator_id: validator_id,
        balance: balance,
        min_shares_out: 1,
        execute_before_block: u32::MAX,
    };

    let queued_item = QueuedSwapItem {
        id,
        call,
        queued_at_block: Network::get_current_block_as_u32(),
        execute_after_blocks: EpochLength::get(),
    };

    // Add to data storage
    SwapCallQueue::<Test>::insert(&id, &queued_item);

    // Add ID to the end of the queue
    SwapQueueOrder::<Test>::mutate(|queue| {
        queue.try_push(id).expect("test queue must have capacity");
        SwapQueueCount::<Test>::set(queue.len() as u32);
    });

    NextSwapQueueId::<Test>::mutate(|next_id| *next_id = next_id.saturating_add(1));
    TotalQueuedSwapPrincipal::<Test>::mutate(|total| {
        *total = total
            .checked_add(balance)
            .expect("test queue principal fits u128")
    });
    assert_queued_swap_principal_invariant();
}

fn setup_all_swap_sources() -> (u32, u32, u32, [AccountIdOf<Test>; 4], [u128; 4]) {
    const SOURCE_BALANCE: u128 = 1_000_000_000_000_000_000_000;

    let subnet_name: Vec<u8> = "queued-principal-source".into();
    build_activated_subnet(
        subnet_name.clone(),
        0,
        4,
        10_000_000_000_000_000_000_000,
        MinSubnetMinStake::<Test>::get(),
    );
    let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
    let from_validator_id = 1;
    let to_validator_id = 2;
    let stakers = [account(960), account(961), account(962), account(963)];

    for staker in stakers.iter() {
        let _ = Balances::deposit_creating(staker, SOURCE_BALANCE.saturating_add(500));
    }

    assert_ok!(Network::add_subnet_delegate_stake(
        RuntimeOrigin::signed(stakers[0].clone()),
        subnet_id,
        SOURCE_BALANCE,
        1,
    ));
    assert_ok!(Network::add_subnet_delegate_stake(
        RuntimeOrigin::signed(stakers[1].clone()),
        subnet_id,
        SOURCE_BALANCE,
        1,
    ));
    assert_ok!(Network::add_validator_delegate_stake(
        RuntimeOrigin::signed(stakers[2].clone()),
        from_validator_id,
        SOURCE_BALANCE,
        1,
    ));
    assert_ok!(Network::add_validator_delegate_stake(
        RuntimeOrigin::signed(stakers[3].clone()),
        from_validator_id,
        SOURCE_BALANCE,
        1,
    ));

    let shares = [
        AccountSubnetDelegateStakeShares::<Test>::get(&stakers[0], subnet_id),
        AccountSubnetDelegateStakeShares::<Test>::get(&stakers[1], subnet_id),
        AccountValidatorDelegateStakeShares::<Test>::get(&stakers[2], from_validator_id),
        AccountValidatorDelegateStakeShares::<Test>::get(&stakers[3], from_validator_id),
    ];
    assert!(shares.iter().all(|shares| *shares > 0));

    (
        subnet_id,
        from_validator_id,
        to_validator_id,
        stakers,
        shares,
    )
}

#[test]
fn test_update_swap_queue_requires_existing_queue_owner() {
    new_test_ext().execute_with(|| {
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let subnet_name: Vec<u8> = "subnet-name".into();
        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();

        let owner = account(10);
        let attacker = account(11);
        let balance = MinDelegateStakeDeposit::<Test>::get();

        let subnet_queue_id = NextSwapQueueId::<Test>::get();
        insert_to_subnet_swap_call_queue(owner.clone(), subnet_id, balance);
        let original_subnet_item = SwapCallQueue::<Test>::get(subnet_queue_id).unwrap();

        assert_err!(
            Network::update_swap_queue(
                RuntimeOrigin::signed(attacker.clone()),
                subnet_queue_id,
                QueuedSwapCall::SwapToSubnetDelegateStake {
                    account_id: attacker.clone(),
                    to_subnet_id: subnet_id,
                    balance: u128::MAX,
                    min_shares_out: 1,
                    execute_before_block: u32::MAX,
                },
            ),
            Error::<Test>::NotKeyOwner
        );
        assert_eq!(
            SwapCallQueue::<Test>::get(subnet_queue_id).unwrap(),
            original_subnet_item
        );

        let validator_queue_id = NextSwapQueueId::<Test>::get();
        insert_to_validator_swap_call_queue(owner, 1, balance);
        let original_validator_item = SwapCallQueue::<Test>::get(validator_queue_id).unwrap();

        assert_err!(
            Network::update_swap_queue(
                RuntimeOrigin::signed(attacker.clone()),
                validator_queue_id,
                QueuedSwapCall::SwapToValidatorDelegateStake {
                    account_id: attacker,
                    to_validator_id: 1,
                    balance: u128::MAX,
                    min_shares_out: 1,
                    execute_before_block: u32::MAX,
                },
            ),
            Error::<Test>::NotKeyOwner
        );
        assert_eq!(
            SwapCallQueue::<Test>::get(validator_queue_id).unwrap(),
            original_validator_item
        );
        assert_queued_swap_principal_invariant();
    });
}

#[test]
fn test_swap_queue_order_accepts_configured_capacity_and_rejects_overflow() {
    new_test_ext().execute_with(|| {
        let max_len = <Test as crate::Config>::MaxSwapQueueLength::get();

        SwapQueueOrder::<Test>::mutate(|queue| {
            for id in 0..max_len {
                queue.try_push(id).expect("configured capacity should fit");
            }
        });

        assert_eq!(SwapQueueOrder::<Test>::get().len(), max_len as usize);

        let next_id = NextSwapQueueId::<Test>::get();
        let call = QueuedSwapCall::SwapToSubnetDelegateStake {
            account_id: account(1),
            to_subnet_id: 1,
            balance: 1,
            min_shares_out: 1,
            execute_before_block: u32::MAX,
        };

        assert_err!(
            Network::queue_swap(account(1), QueuedSwapSource::SubnetDelegate, call),
            Error::<Test>::SwapQueueFull
        );
        assert_eq!(NextSwapQueueId::<Test>::get(), next_id);
        assert!(SwapCallQueue::<Test>::get(next_id).is_none());
        assert_eq!(SwapCallQueue::<Test>::iter().count(), 0);
        assert_queued_swap_principal_invariant();
    });
}

#[test]
fn test_queue_swap_rejects_zero_principal_without_allocating_an_id() {
    new_test_ext().execute_with(|| {
        let staker = account(12);
        let next_id = NextSwapQueueId::<Test>::get();
        let execute_before_block = System::block_number()
            .checked_add(EpochLength::get())
            .unwrap();

        assert_err!(
            Network::queue_swap(
                staker.clone(),
                QueuedSwapSource::SubnetDelegate,
                QueuedSwapCall::SwapToSubnetDelegateStake {
                    account_id: staker,
                    to_subnet_id: 1,
                    balance: 0,
                    min_shares_out: 1,
                    execute_before_block,
                },
            ),
            Error::<Test>::ZeroSwapBalance
        );
        assert_eq!(NextSwapQueueId::<Test>::get(), next_id);
        assert!(SwapCallQueue::<Test>::get(next_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
    });
}

#[test]
fn test_public_swap_rejects_one_share_that_redeems_to_zero_before_source_debit() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "zero-redemption-source".into();
        let stake_amount = 1_000_000_000_000_000_000_000u128;
        build_activated_subnet(
            subnet_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let staker = account(15);
        let _ = Balances::deposit_creating(&staker, stake_amount.saturating_add(500));

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(staker.clone()),
            subnet_id,
            stake_amount,
            1,
        ));

        let account_shares_before =
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id);
        let pool_shares_before = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let pool_balance_before = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let total_delegate_stake_before = TotalDelegateStake::<Test>::get();
        let next_queue_id_before = NextSwapQueueId::<Test>::get();
        assert!(account_shares_before > 1);
        assert_eq!(
            Network::convert_to_balance(1, pool_shares_before, pool_balance_before),
            0
        );

        assert_err!(
            Network::swap_from_subnet_to_subnet(
                RuntimeOrigin::signed(staker.clone()),
                subnet_id,
                subnet_id,
                1,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::CouldNotConvertToBalance
        );

        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id),
            account_shares_before
        );
        assert_eq!(
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id),
            pool_shares_before
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            pool_balance_before
        );
        assert_eq!(
            TotalDelegateStake::<Test>::get(),
            total_delegate_stake_before
        );
        assert_eq!(NextSwapQueueId::<Test>::get(), next_queue_id_before);
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
    });
}

#[test]
fn test_expired_swap_is_terminally_refunded() {
    new_test_ext().execute_with(|| {
        const QUEUED_BALANCE: u128 = 10_000;
        let staker = account(13);
        let queue_id = NextSwapQueueId::<Test>::get();
        let execute_before_block = System::block_number()
            .checked_add(EpochLength::get())
            .unwrap();

        assert_ok!(Network::queue_swap(
            staker.clone(),
            QueuedSwapSource::SubnetDelegate,
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id: staker.clone(),
                to_subnet_id: u32::MAX,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block,
            },
        ));

        let execution_block = execute_before_block.checked_add(1).unwrap();
        System::set_block_number(execution_block);
        Network::execute_ready_swap_calls_with_limit(execution_block, 1, &mut WeightMeter::new());

        assert!(SwapCallQueue::<Test>::get(queue_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(&staker),
            QUEUED_BALANCE
        );
        assert_eq!(TotalQueuedSwapRefundBalance::<Test>::get(), QUEUED_BALANCE);
        assert!(network_events().iter().any(|event| matches!(
            event,
            Event::SwapCallRefunded {
                id,
                account_id,
                balance,
                reason: SwapRefundReason::Expired,
            } if *id == queue_id && account_id == &staker && *balance == QUEUED_BALANCE
        )));
    });
}

#[test]
fn test_destination_exchange_rate_slippage_refunds_without_minting_shares() {
    new_test_ext().execute_with(|| {
        const QUEUED_BALANCE: u128 = 10_000;
        const MIN_SHARES_OUT: u128 = 500_000_000;
        let subnet_name: Vec<u8> = "swap-slippage-destination".into();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let staker = account(14);
        TotalSubnetDelegateStakeShares::<Test>::insert(subnet_id, 2_000_000_000);
        TotalSubnetDelegateStakeBalance::<Test>::insert(subnet_id, 10_000);
        TotalSubnetDelegateStakeCirculatingShares::<Test>::insert(subnet_id, 0);
        let queue_id = NextSwapQueueId::<Test>::get();

        assert_ok!(Network::queue_swap(
            staker.clone(),
            QueuedSwapSource::SubnetDelegate,
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id: staker.clone(),
                to_subnet_id: subnet_id,
                balance: QUEUED_BALANCE,
                min_shares_out: MIN_SHARES_OUT,
                execute_before_block: u32::MAX,
            },
        ));

        // Simulate an adverse exchange-rate movement while the call waits in the queue.
        TotalSubnetDelegateStakeBalance::<Test>::insert(subnet_id, 100_000);
        assert_eq!(
            Network::handle_increase_account_delegate_stake_with_limit(
                &staker,
                subnet_id,
                QUEUED_BALANCE,
                MIN_SHARES_OUT,
            ),
            Err(Error::<Test>::StakeSlippageExceeded.into()),
        );
        let destination_shares_before = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let execution_block = System::block_number()
            .checked_add(EpochLength::get())
            .unwrap();
        System::set_block_number(execution_block);
        Network::execute_ready_swap_calls_with_limit(execution_block, 1, &mut WeightMeter::new());

        assert!(SwapCallQueue::<Test>::get(queue_id).is_none());
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id),
            0
        );
        assert_eq!(
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id),
            destination_shares_before
        );
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(&staker),
            QUEUED_BALANCE
        );
        let events = network_events();
        assert!(
            events.iter().any(|event| matches!(
                event,
                Event::SwapCallRefunded {
                    id,
                    account_id,
                    balance,
                    reason: SwapRefundReason::MinimumSharesNotMet,
                } if *id == queue_id && account_id == &staker && *balance == QUEUED_BALANCE
            )),
            "unexpected events: {events:?}"
        );
    });
}

#[test]
fn test_below_minimum_subnet_swap_is_refunded_without_destination_credit() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "below-minimum-subnet-swap".into();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let staker = account(15);
        let queued_balance = MinDelegateStakeDeposit::<Test>::get()
            .checked_sub(1)
            .expect("the configured minimum deposit is positive");
        let account_shares_before =
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id);
        let pool_shares_before = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let circulating_shares_before =
            TotalSubnetDelegateStakeCirculatingShares::<Test>::get(subnet_id);
        let pool_balance_before = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let total_delegate_stake_before = TotalDelegateStake::<Test>::get();
        let queue_id = NextSwapQueueId::<Test>::get();

        assert_ok!(Network::queue_swap(
            staker.clone(),
            QueuedSwapSource::SubnetDelegate,
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id: staker.clone(),
                to_subnet_id: subnet_id,
                balance: queued_balance,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));

        let execution_block = System::block_number()
            .checked_add(EpochLength::get())
            .unwrap();
        System::set_block_number(execution_block);
        Network::execute_ready_swap_calls_with_limit(execution_block, 1, &mut WeightMeter::new());

        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id),
            account_shares_before
        );
        assert_eq!(
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id),
            pool_shares_before
        );
        assert_eq!(
            TotalSubnetDelegateStakeCirculatingShares::<Test>::get(subnet_id),
            circulating_shares_before
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            pool_balance_before
        );
        assert_eq!(
            TotalDelegateStake::<Test>::get(),
            total_delegate_stake_before
        );
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(&staker),
            queued_balance
        );
        assert_eq!(TotalQueuedSwapRefundBalance::<Test>::get(), queued_balance);
        assert!(SwapCallQueue::<Test>::get(queue_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(SwapQueueCount::<Test>::get(), 0);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_queued_swap_principal_invariant();
        assert!(network_events().iter().any(|event| matches!(
            event,
            Event::SwapCallRefunded {
                id,
                account_id,
                balance,
                reason: SwapRefundReason::MinimumDepositNotMet,
            } if *id == queue_id && account_id == &staker && *balance == queued_balance
        )));
    });
}

#[test]
fn test_below_minimum_validator_swap_is_refunded_without_destination_credit() {
    new_test_ext().execute_with(|| {
        const VALIDATOR_ID: u32 = 89;
        manual_insert_validator(VALIDATOR_ID, 960, 961);
        let staker = account(962);
        let queued_balance = MinDelegateStakeDeposit::<Test>::get()
            .checked_sub(1)
            .expect("the configured minimum deposit is positive");
        let queue_id = NextSwapQueueId::<Test>::get();

        assert_ok!(Network::queue_swap(
            staker.clone(),
            QueuedSwapSource::ValidatorDelegate,
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id: staker.clone(),
                to_validator_id: VALIDATOR_ID,
                balance: queued_balance,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));

        let execution_block = System::block_number()
            .checked_add(EpochLength::get())
            .unwrap();
        System::set_block_number(execution_block);
        Network::execute_ready_swap_calls_with_limit(execution_block, 1, &mut WeightMeter::new());

        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&staker, VALIDATOR_ID),
            0
        );
        assert_eq!(ValidatorDelegateStakeShares::<Test>::get(VALIDATOR_ID), 0);
        assert_eq!(
            ValidatorDelegateStakeCirculatingShares::<Test>::get(VALIDATOR_ID),
            0
        );
        assert_eq!(ValidatorDelegateStakeBalance::<Test>::get(VALIDATOR_ID), 0);
        assert_eq!(TotalValidatorDelegateStakeBalance::<Test>::get(), 0);
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(&staker),
            queued_balance
        );
        assert_eq!(TotalQueuedSwapRefundBalance::<Test>::get(), queued_balance);
        assert!(SwapCallQueue::<Test>::get(queue_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(SwapQueueCount::<Test>::get(), 0);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_queued_swap_principal_invariant();
        assert!(network_events().iter().any(|event| matches!(
            event,
            Event::SwapCallRefunded {
                id,
                account_id,
                balance,
                reason: SwapRefundReason::MinimumDepositNotMet,
            } if *id == queue_id && account_id == &staker && *balance == queued_balance
        )));
    });
}

#[test]
fn test_update_swap_queue_delegate_stake() {
    new_test_ext().execute_with(|| {
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let from_subnet_name: Vec<u8> = "subnet-name".into();
        build_activated_subnet(from_subnet_name.clone(), 0, 0, deposit_amount, stake_amount);
        let from_subnet_id = SubnetName::<Test>::get(from_subnet_name.clone()).unwrap();

        let to_subnet_name: Vec<u8> = "subnet-name-2".into();
        build_activated_subnet(to_subnet_name.clone(), 0, 0, deposit_amount, stake_amount);
        let to_subnet_id = SubnetName::<Test>::get(to_subnet_name.clone()).unwrap();

        let n_account = 255;

        let _ = Balances::deposit_creating(&account(n_account), amount + 500);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(from_subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(from_subnet_id);

        let mut delegate_stake_to_be_added_as_shares = Network::convert_to_shares(
            amount,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        System::set_block_number(
            System::block_number()
                + DelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get(),
        );

        let starting_delegator_balance = Balances::free_balance(&account(n_account));

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            from_subnet_id,
            amount,
            1,
        ));

        let delegate_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(account(n_account), from_subnet_id);
        assert_eq!(delegate_shares, delegate_stake_to_be_added_as_shares);
        assert_ne!(delegate_shares, 0);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(from_subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(from_subnet_id);

        let mut from_delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );
        // The first depositor will lose a percentage of their deposit depending on the size
        // https://docs.openzeppelin.com/contracts/4.x/erc4626#inflation-attack
        // assert_eq!(from_delegate_balance, delegate_stake_to_be_added_as_shares);

        let prev_total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(from_subnet_id);
        let prev_next_id = NextSwapQueueId::<Test>::get();

        assert_ok!(Network::swap_from_subnet_to_subnet(
            RuntimeOrigin::signed(account(n_account)),
            from_subnet_id,
            to_subnet_id,
            delegate_shares,
            1,
            1,
            u32::MAX,
        ));

        // Check ledger doesn't have any unbondings and is empty
        assert!(StakeUnbondingLedger::<Test>::get(account(n_account)).is_empty());

        let from_delegate_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(account(n_account), from_subnet_id);
        assert_eq!(from_delegate_shares, 0);

        assert_ne!(
            prev_total_subnet_delegate_stake_balance,
            TotalSubnetDelegateStakeBalance::<Test>::get(from_subnet_id)
        );
        assert!(
            prev_total_subnet_delegate_stake_balance
                > TotalSubnetDelegateStakeBalance::<Test>::get(from_subnet_id)
        );

        // Check the queue
        let starting_to_subnet_id = to_subnet_id;
        let call_queue = SwapCallQueue::<Test>::get(prev_next_id);
        assert_eq!(call_queue.clone().unwrap().id, prev_next_id);
        match &call_queue.clone().unwrap().call {
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id,
                to_subnet_id,
                balance,
                ..
            } => {
                assert_eq!(*account_id, account(n_account));
                assert_eq!(*to_subnet_id, starting_to_subnet_id);
                assert_ne!(*balance, 0);
            }
            QueuedSwapCall::SwapToValidatorDelegateStake { .. } => assert!(false),
        };
        assert_queued_swap_principal_invariant();
        let queued_principal_before_updates = TotalQueuedSwapPrincipal::<Test>::get();

        let next_id = NextSwapQueueId::<Test>::get();
        assert_eq!(prev_next_id + 1, next_id);
        let queue = SwapQueueOrder::<Test>::get();
        assert!(queue
            .first()
            .map_or(false, |&first_id| first_id == prev_next_id));

        // UPDATE

        // Update back to the `from_subnet_id` staying as a `SwapToSubnetDelegateStake`
        let call = QueuedSwapCall::SwapToSubnetDelegateStake {
            account_id: account(n_account),
            to_subnet_id: from_subnet_id,
            balance: u128::MAX,
            min_shares_out: 1,
            execute_before_block: u32::MAX,
        };

        assert_ok!(Network::update_swap_queue(
            RuntimeOrigin::signed(account(n_account)),
            prev_next_id,
            call.clone(),
        ));

        let event_exists = network_events().iter().any(|event| {
            matches!(event,
                Event::SwapCallQueueUpdated {
                    id: prev_next_id_val,
                    account_id: account_id_val,
                    call: QueuedSwapCall::SwapToSubnetDelegateStake {
                        account_id: account_id_val2,
                        to_subnet_id: from_subnet_id_val,
                        balance: _, // Ignore balance
                        ..
                    }
                } if *prev_next_id_val == prev_next_id
                && *account_id_val == account(n_account)
                && *account_id_val2 == account(n_account)
                && *from_subnet_id_val == from_subnet_id
            )
        });
        assert!(event_exists);

        let call_queue = SwapCallQueue::<Test>::get(prev_next_id);
        assert_eq!(call_queue.clone().unwrap().id, prev_next_id);
        match &call_queue.clone().unwrap().call {
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id,
                to_subnet_id,
                balance,
                ..
            } => {
                assert_eq!(*account_id, account(n_account));
                assert_eq!(*to_subnet_id, from_subnet_id);
                assert_ne!(*balance, 0);
                assert_ne!(*balance, u128::MAX);
            }
            QueuedSwapCall::SwapToValidatorDelegateStake { .. } => assert!(false),
        };
        assert_eq!(
            TotalQueuedSwapPrincipal::<Test>::get(),
            queued_principal_before_updates
        );
        assert_queued_swap_principal_invariant();

        //
        // Update back to the `starting_to_subnet_id` with node ID as a `SwapToValidatorDelegateStake`
        //
        let call = QueuedSwapCall::SwapToValidatorDelegateStake {
            account_id: account(n_account),
            to_validator_id: 1,
            balance: u128::MAX,
            min_shares_out: 1,
            execute_before_block: u32::MAX,
        };

        assert_ok!(Network::update_swap_queue(
            RuntimeOrigin::signed(account(n_account)),
            prev_next_id,
            call.clone(),
        ));

        let event_exists = network_events().iter().any(|event| {
            matches!(event,
                Event::SwapCallQueueUpdated {
                    id: prev_next_id_val,
                    account_id: account_id_val,
                    call: QueuedSwapCall::SwapToValidatorDelegateStake {
                        account_id: account_id_val2,
                        to_validator_id: 1,
                        balance: _, // Ignore balance
                        ..
                    }
                } if *prev_next_id_val == prev_next_id
                && *account_id_val == account(n_account)
                && *account_id_val2 == account(n_account)
                // && *from_subnet_id_val == from_subnet_id
            )
        });
        assert!(event_exists);

        let call_queue = SwapCallQueue::<Test>::get(prev_next_id);
        assert_eq!(call_queue.clone().unwrap().id, prev_next_id);
        match &call_queue.clone().unwrap().call {
            QueuedSwapCall::SwapToSubnetDelegateStake { .. } => assert!(false),
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id,
                to_validator_id,
                balance,
                ..
            } => {
                assert_eq!(*account_id, account(n_account));
                assert_ne!(*balance, 0);
                assert_ne!(*balance, u128::MAX);
            }
        };
        assert_eq!(
            TotalQueuedSwapPrincipal::<Test>::get(),
            queued_principal_before_updates
        );
        assert_queued_swap_principal_invariant();
    });
}

#[test]
fn test_update_swap_queue_node_delegate_stake() {
    new_test_ext().execute_with(|| {
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let end = 4;

        let from_subnet_name: Vec<u8> = "subnet-name".into();
        build_activated_subnet(
            from_subnet_name.clone(),
            0,
            end,
            deposit_amount,
            stake_amount,
        );
        let from_subnet_id = SubnetName::<Test>::get(from_subnet_name.clone()).unwrap();
        let from_subnet_node_id = 1;

        let to_subnet_name: Vec<u8> = "subnet-name-2".into();
        build_activated_subnet(to_subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let to_subnet_id = SubnetName::<Test>::get(to_subnet_name.clone()).unwrap();
        let to_subnet_node_id = 1;

        let from_validator_id = 1;
        let to_validator_id = 2;
        let starting_to_validator_id = to_validator_id;

        let n_account = 255;

        let _ = Balances::deposit_creating(&account(n_account), amount + 500);

        let total_subnet_node_delegate_stake_shares =
            ValidatorDelegateStakeShares::<Test>::get(from_validator_id);
        let total_subnet_node_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(from_validator_id);

        let node_delegate_stake_to_be_added_as_shares = Network::preview_delegate_pool_deposit(
            amount,
            total_subnet_node_delegate_stake_shares,
            total_subnet_node_delegate_stake_balance,
            1,
        )
        .unwrap()
        .0;

        System::set_block_number(
            System::block_number()
                + DelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get(),
        );

        let starting_delegator_balance = Balances::free_balance(&account(n_account));

        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            from_validator_id,
            amount,
            1,
        ));

        let validator_delegate_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(account(n_account), from_validator_id);
        assert_eq!(
            validator_delegate_shares,
            node_delegate_stake_to_be_added_as_shares
        );
        assert_ne!(validator_delegate_shares, 0);

        let total_subnet_node_delegate_stake_shares =
            ValidatorDelegateStakeShares::<Test>::get(from_validator_id);
        let total_subnet_node_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(from_validator_id);

        let mut from_node_delegate_balance = Network::convert_to_balance(
            validator_delegate_shares,
            total_subnet_node_delegate_stake_shares,
            total_subnet_node_delegate_stake_balance,
        );
        // The first depositor will lose a percentage of their deposit depending on the size
        // https://docs.openzeppelin.com/contracts/4.x/erc4626#inflation-attack
        // assert_eq!(from_delegate_balance, delegate_stake_to_be_added_as_shares);

        let prev_total_subnet_node_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<Test>::get(from_validator_id);
        let prev_next_id = NextSwapQueueId::<Test>::get();

        assert_ok!(Network::swap_from_validator_to_validator(
            RuntimeOrigin::signed(account(n_account)),
            from_validator_id,
            to_validator_id,
            validator_delegate_shares,
            1,
            1,
            u32::MAX,
        ));

        // Check ledger doesn't have any unbondings and is empty
        assert!(StakeUnbondingLedger::<Test>::get(account(n_account)).is_empty());

        let from_validator_delegate_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(account(n_account), from_validator_id);
        assert_eq!(from_validator_delegate_shares, 0);

        assert_ne!(
            prev_total_subnet_node_delegate_stake_balance,
            ValidatorDelegateStakeBalance::<Test>::get(from_validator_id)
        );
        assert!(
            prev_total_subnet_node_delegate_stake_balance
                > ValidatorDelegateStakeBalance::<Test>::get(from_validator_id)
        );

        // Check the queue
        let starting_to_subnet_id = to_subnet_id;
        let call_queue = SwapCallQueue::<Test>::get(prev_next_id);
        assert_eq!(call_queue.clone().unwrap().id, prev_next_id);
        match &call_queue.clone().unwrap().call {
            QueuedSwapCall::SwapToSubnetDelegateStake { .. } => assert!(false),
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id,
                to_validator_id,
                balance,
                ..
            } => {
                assert_eq!(*account_id, account(n_account));
                assert_eq!(*to_validator_id, starting_to_validator_id);
            }
        };
        assert_queued_swap_principal_invariant();
        let queued_principal_before_updates = TotalQueuedSwapPrincipal::<Test>::get();

        let next_id = NextSwapQueueId::<Test>::get();
        assert_eq!(prev_next_id + 1, next_id);
        let queue = SwapQueueOrder::<Test>::get();
        assert!(queue
            .first()
            .map_or(false, |&first_id| first_id == prev_next_id));

        // UPDATE

        // Update back to the `from_subnet_id` staying as a `SwapToSubnetDelegateStake`
        let call = QueuedSwapCall::SwapToSubnetDelegateStake {
            account_id: account(n_account),
            to_subnet_id: from_subnet_id,
            balance: u128::MAX,
            min_shares_out: 1,
            execute_before_block: u32::MAX,
        };

        assert_ok!(Network::update_swap_queue(
            RuntimeOrigin::signed(account(n_account)),
            prev_next_id,
            call.clone(),
        ));

        let event_exists = network_events().iter().any(|event| {
            matches!(event,
                Event::SwapCallQueueUpdated {
                    id: prev_next_id_val,
                    account_id: account_id_val,
                    call: QueuedSwapCall::SwapToSubnetDelegateStake {
                        account_id: account_id_val2,
                        to_subnet_id: from_subnet_id_val,
                        balance: _, // Ignore balance
                        ..
                    }
                } if *prev_next_id_val == prev_next_id
                && *account_id_val == account(n_account)
                && *account_id_val2 == account(n_account)
                && *from_subnet_id_val == from_subnet_id
            )
        });
        assert!(event_exists);

        let call_queue = SwapCallQueue::<Test>::get(prev_next_id);
        assert_eq!(call_queue.clone().unwrap().id, prev_next_id);
        match &call_queue.clone().unwrap().call {
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id,
                to_subnet_id,
                balance,
                ..
            } => {
                assert_eq!(*account_id, account(n_account));
                assert_eq!(*to_subnet_id, from_subnet_id);
                assert_ne!(*balance, 0);
                assert_ne!(*balance, u128::MAX);
            }
            QueuedSwapCall::SwapToValidatorDelegateStake { .. } => assert!(false),
        };
        assert_eq!(
            TotalQueuedSwapPrincipal::<Test>::get(),
            queued_principal_before_updates
        );
        assert_queued_swap_principal_invariant();

        //
        // Update back to the `starting_to_subnet_id` with node ID as a `SwapToValidatorDelegateStake`
        //
        let call = QueuedSwapCall::SwapToValidatorDelegateStake {
            account_id: account(n_account),
            to_validator_id: 1,
            balance: u128::MAX,
            min_shares_out: 1,
            execute_before_block: u32::MAX,
        };

        assert_ok!(Network::update_swap_queue(
            RuntimeOrigin::signed(account(n_account)),
            prev_next_id,
            call.clone(),
        ));

        let event_exists = network_events().iter().any(|event| {
            matches!(event,
                Event::SwapCallQueueUpdated {
                    id: prev_next_id_val,
                    account_id: account_id_val,
                    call: QueuedSwapCall::SwapToSubnetDelegateStake {
                        account_id: account_id_val2,
                        to_subnet_id: from_subnet_id_val,
                        balance: _, // Ignore balance
                        ..
                    }
                } if *prev_next_id_val == prev_next_id
                && *account_id_val == account(n_account)
                && *account_id_val2 == account(n_account)
                && *from_subnet_id_val == from_subnet_id
            )
        });
        assert!(event_exists);

        let call_queue = SwapCallQueue::<Test>::get(prev_next_id);
        assert_eq!(call_queue.clone().unwrap().id, prev_next_id);
        match &call_queue.clone().unwrap().call {
            QueuedSwapCall::SwapToSubnetDelegateStake { .. } => assert!(false),
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id,
                to_validator_id,
                balance,
                ..
            } => {
                assert_eq!(*account_id, account(n_account));
                assert_ne!(*balance, u128::MAX);
            }
        };
        assert_eq!(
            TotalQueuedSwapPrincipal::<Test>::get(),
            queued_principal_before_updates
        );
        assert_queued_swap_principal_invariant();
    });
}

#[test]
fn test_total_queued_swap_principal_tracks_all_four_swap_directions() {
    new_test_ext().execute_with(|| {
        let (subnet_id, from_validator_id, to_validator_id, stakers, shares) =
            setup_all_swap_sources();
        let mut expected_principal = 0u128;
        let tvl_before_swaps = Network::get_total_network_tvl();
        let survival_minimum_before = Network::get_min_subnet_delegate_stake_balance(subnet_id);

        let queue_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::swap_from_subnet_to_subnet(
            RuntimeOrigin::signed(stakers[0].clone()),
            subnet_id,
            subnet_id,
            shares[0],
            1,
            1,
            u32::MAX,
        ));
        expected_principal = expected_principal
            .checked_add(
                SwapCallQueue::<Test>::get(queue_id)
                    .unwrap()
                    .call
                    .get_queue_balance(),
            )
            .unwrap();
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), expected_principal);
        assert_eq!(Network::get_total_network_tvl(), tvl_before_swaps);
        assert_queued_swap_principal_invariant();

        let queue_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::swap_from_subnet_to_validator(
            RuntimeOrigin::signed(stakers[1].clone()),
            subnet_id,
            to_validator_id,
            shares[1],
            1,
            1,
            u32::MAX,
        ));
        expected_principal = expected_principal
            .checked_add(
                SwapCallQueue::<Test>::get(queue_id)
                    .unwrap()
                    .call
                    .get_queue_balance(),
            )
            .unwrap();
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), expected_principal);
        assert_eq!(Network::get_total_network_tvl(), tvl_before_swaps);
        assert_queued_swap_principal_invariant();

        let queue_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::swap_from_validator_to_subnet(
            RuntimeOrigin::signed(stakers[2].clone()),
            from_validator_id,
            subnet_id,
            shares[2],
            1,
            1,
            u32::MAX,
        ));
        expected_principal = expected_principal
            .checked_add(
                SwapCallQueue::<Test>::get(queue_id)
                    .unwrap()
                    .call
                    .get_queue_balance(),
            )
            .unwrap();
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), expected_principal);
        assert_eq!(Network::get_total_network_tvl(), tvl_before_swaps);
        assert_queued_swap_principal_invariant();

        let queue_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::swap_from_validator_to_validator(
            RuntimeOrigin::signed(stakers[3].clone()),
            from_validator_id,
            to_validator_id,
            shares[3],
            1,
            1,
            u32::MAX,
        ));
        expected_principal = expected_principal
            .checked_add(
                SwapCallQueue::<Test>::get(queue_id)
                    .unwrap()
                    .call
                    .get_queue_balance(),
            )
            .unwrap();
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), expected_principal);
        assert_eq!(Network::get_total_network_tvl(), tvl_before_swaps);
        assert_eq!(
            Network::get_min_subnet_delegate_stake_balance(subnet_id),
            survival_minimum_before
        );
        assert_eq!(SwapQueueCount::<Test>::get(), 4);
        assert_queued_swap_principal_invariant();
    });
}

#[test]
fn subnet_source_swap_snapshots_longer_cooldown_for_credit_and_refund() {
    new_test_ext().execute_with(|| {
        const COOLDOWN_EPOCHS: u32 = 3;
        DelegateStakeCooldownEpochs::<Test>::put(COOLDOWN_EPOCHS);
        let (subnet_id, _, to_validator_id, stakers, shares) = setup_all_swap_sources();
        let credited_staker = &stakers[0];
        let refunded_staker = &stakers[1];
        let queued_at_block = System::block_number();
        let source_cooldown_blocks = COOLDOWN_EPOCHS.checked_mul(EpochLength::get()).unwrap();

        // A deadline that covered the old one-epoch delay must not debit source principal when
        // the configured source cooldown is longer.
        let too_early_deadline = queued_at_block.checked_add(EpochLength::get()).unwrap();
        assert_err!(
            Network::swap_from_subnet_to_subnet(
                RuntimeOrigin::signed(credited_staker.clone()),
                subnet_id,
                subnet_id,
                shares[0],
                1,
                1,
                too_early_deadline,
            ),
            Error::<Test>::InvalidSwapDeadline
        );
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(credited_staker, subnet_id),
            shares[0]
        );
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);

        let credited_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::swap_from_subnet_to_subnet(
            RuntimeOrigin::signed(credited_staker.clone()),
            subnet_id,
            subnet_id,
            shares[0],
            1,
            1,
            u32::MAX,
        ));
        let refunded_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::swap_from_subnet_to_validator(
            RuntimeOrigin::signed(refunded_staker.clone()),
            subnet_id,
            to_validator_id,
            shares[1],
            1,
            u128::MAX,
            u32::MAX,
        ));

        let credited_item = SwapCallQueue::<Test>::get(credited_id).unwrap();
        let refunded_item = SwapCallQueue::<Test>::get(refunded_id).unwrap();
        let refunded_principal = refunded_item.call.get_queue_balance();
        assert_eq!(credited_item.queued_at_block, queued_at_block);
        assert_eq!(refunded_item.queued_at_block, queued_at_block);
        assert_eq!(credited_item.execute_after_blocks, source_cooldown_blocks);
        assert_eq!(refunded_item.execute_after_blocks, source_cooldown_blocks);
        assert_queued_swap_principal_invariant();

        // The delay is a queue-item snapshot; a later governance reduction cannot shorten it.
        DelegateStakeCooldownEpochs::<Test>::put(1);
        let wallet_before_claim = Balances::free_balance(refunded_staker);
        let issuance_before_claim = Balances::total_issuance();
        let one_epoch_block = queued_at_block.checked_add(EpochLength::get()).unwrap();
        System::set_block_number(one_epoch_block);
        Network::execute_ready_swap_calls_with_limit(one_epoch_block, 2, &mut WeightMeter::new());

        assert_eq!(
            SwapCallQueue::<Test>::get(credited_id),
            Some(credited_item.clone())
        );
        assert_eq!(
            SwapCallQueue::<Test>::get(refunded_id),
            Some(refunded_item.clone())
        );
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(credited_staker, subnet_id),
            0
        );
        assert_eq!(QueuedSwapRefundBalance::<Test>::get(refunded_staker), 0);
        assert_err!(
            Network::claim_unbondings(RuntimeOrigin::signed(refunded_staker.clone())),
            Error::<Test>::NoStakeUnbondingsOrCooldownNotMet
        );
        assert_eq!(Balances::free_balance(refunded_staker), wallet_before_claim);
        assert_eq!(Balances::total_issuance(), issuance_before_claim);

        let unlock_block = queued_at_block.checked_add(source_cooldown_blocks).unwrap();
        let block_before_unlock = unlock_block.checked_sub(1).unwrap();
        System::set_block_number(block_before_unlock);
        Network::execute_ready_swap_calls_with_limit(
            block_before_unlock,
            2,
            &mut WeightMeter::new(),
        );
        assert_eq!(SwapCallQueue::<Test>::get(credited_id), Some(credited_item));
        assert_eq!(SwapCallQueue::<Test>::get(refunded_id), Some(refunded_item));
        assert_err!(
            Network::claim_unbondings(RuntimeOrigin::signed(refunded_staker.clone())),
            Error::<Test>::NoStakeUnbondingsOrCooldownNotMet
        );

        System::set_block_number(unlock_block);
        Network::execute_ready_swap_calls_with_limit(unlock_block, 2, &mut WeightMeter::new());
        assert!(SwapCallQueue::<Test>::get(credited_id).is_none());
        assert!(SwapCallQueue::<Test>::get(refunded_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert!(AccountSubnetDelegateStakeShares::<Test>::get(credited_staker, subnet_id) > 0);
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(refunded_staker),
            refunded_principal
        );
        assert_eq!(
            TotalQueuedSwapRefundBalance::<Test>::get(),
            refunded_principal
        );
        assert!(network_events().iter().any(|event| matches!(
            event,
            Event::SwapCallRefunded {
                id,
                account_id,
                balance,
                reason: SwapRefundReason::MinimumSharesNotMet,
            } if *id == refunded_id
                && account_id == refunded_staker
                && *balance == refunded_principal
        )));

        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            refunded_staker.clone()
        )));
        assert_eq!(
            Balances::free_balance(refunded_staker),
            wallet_before_claim.checked_add(refunded_principal).unwrap()
        );
        assert_eq!(
            Balances::total_issuance(),
            issuance_before_claim
                .checked_add(refunded_principal)
                .unwrap()
        );
        assert_eq!(QueuedSwapRefundBalance::<Test>::get(refunded_staker), 0);
        assert_eq!(TotalQueuedSwapRefundBalance::<Test>::get(), 0);
    });
}

#[test]
fn validator_source_swap_snapshots_longer_cooldown_for_credit_and_refund() {
    new_test_ext().execute_with(|| {
        const COOLDOWN_EPOCHS: u32 = 4;
        NodeDelegateStakeCooldownEpochs::<Test>::put(COOLDOWN_EPOCHS);
        let (subnet_id, from_validator_id, to_validator_id, stakers, shares) =
            setup_all_swap_sources();
        let credited_staker = &stakers[2];
        let refunded_staker = &stakers[3];
        let queued_at_block = System::block_number();
        let source_cooldown_blocks = COOLDOWN_EPOCHS.checked_mul(EpochLength::get()).unwrap();

        let too_early_deadline = queued_at_block.checked_add(EpochLength::get()).unwrap();
        assert_err!(
            Network::swap_from_validator_to_subnet(
                RuntimeOrigin::signed(credited_staker.clone()),
                from_validator_id,
                subnet_id,
                shares[2],
                1,
                1,
                too_early_deadline,
            ),
            Error::<Test>::InvalidSwapDeadline
        );
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(credited_staker, from_validator_id),
            shares[2]
        );
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);

        let credited_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::swap_from_validator_to_subnet(
            RuntimeOrigin::signed(credited_staker.clone()),
            from_validator_id,
            subnet_id,
            shares[2],
            1,
            1,
            u32::MAX,
        ));
        let refunded_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::swap_from_validator_to_validator(
            RuntimeOrigin::signed(refunded_staker.clone()),
            from_validator_id,
            to_validator_id,
            shares[3],
            1,
            u128::MAX,
            u32::MAX,
        ));

        let credited_item = SwapCallQueue::<Test>::get(credited_id).unwrap();
        let refunded_item = SwapCallQueue::<Test>::get(refunded_id).unwrap();
        let refunded_principal = refunded_item.call.get_queue_balance();
        assert_eq!(credited_item.queued_at_block, queued_at_block);
        assert_eq!(refunded_item.queued_at_block, queued_at_block);
        assert_eq!(credited_item.execute_after_blocks, source_cooldown_blocks);
        assert_eq!(refunded_item.execute_after_blocks, source_cooldown_blocks);
        assert_queued_swap_principal_invariant();

        NodeDelegateStakeCooldownEpochs::<Test>::put(1);
        let wallet_before_claim = Balances::free_balance(refunded_staker);
        let issuance_before_claim = Balances::total_issuance();
        let one_epoch_block = queued_at_block.checked_add(EpochLength::get()).unwrap();
        System::set_block_number(one_epoch_block);
        Network::execute_ready_swap_calls_with_limit(one_epoch_block, 2, &mut WeightMeter::new());

        assert_eq!(
            SwapCallQueue::<Test>::get(credited_id),
            Some(credited_item.clone())
        );
        assert_eq!(
            SwapCallQueue::<Test>::get(refunded_id),
            Some(refunded_item.clone())
        );
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(credited_staker, subnet_id),
            0
        );
        assert_eq!(QueuedSwapRefundBalance::<Test>::get(refunded_staker), 0);
        assert_err!(
            Network::claim_unbondings(RuntimeOrigin::signed(refunded_staker.clone())),
            Error::<Test>::NoStakeUnbondingsOrCooldownNotMet
        );
        assert_eq!(Balances::free_balance(refunded_staker), wallet_before_claim);
        assert_eq!(Balances::total_issuance(), issuance_before_claim);

        let unlock_block = queued_at_block.checked_add(source_cooldown_blocks).unwrap();
        let block_before_unlock = unlock_block.checked_sub(1).unwrap();
        System::set_block_number(block_before_unlock);
        Network::execute_ready_swap_calls_with_limit(
            block_before_unlock,
            2,
            &mut WeightMeter::new(),
        );
        assert_eq!(SwapCallQueue::<Test>::get(credited_id), Some(credited_item));
        assert_eq!(SwapCallQueue::<Test>::get(refunded_id), Some(refunded_item));
        assert_err!(
            Network::claim_unbondings(RuntimeOrigin::signed(refunded_staker.clone())),
            Error::<Test>::NoStakeUnbondingsOrCooldownNotMet
        );

        System::set_block_number(unlock_block);
        Network::execute_ready_swap_calls_with_limit(unlock_block, 2, &mut WeightMeter::new());
        assert!(SwapCallQueue::<Test>::get(credited_id).is_none());
        assert!(SwapCallQueue::<Test>::get(refunded_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert!(AccountSubnetDelegateStakeShares::<Test>::get(credited_staker, subnet_id) > 0);
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(refunded_staker),
            refunded_principal
        );
        assert_eq!(
            TotalQueuedSwapRefundBalance::<Test>::get(),
            refunded_principal
        );
        assert!(network_events().iter().any(|event| matches!(
            event,
            Event::SwapCallRefunded {
                id,
                account_id,
                balance,
                reason: SwapRefundReason::MinimumSharesNotMet,
            } if *id == refunded_id
                && account_id == refunded_staker
                && *balance == refunded_principal
        )));

        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            refunded_staker.clone()
        )));
        assert_eq!(
            Balances::free_balance(refunded_staker),
            wallet_before_claim.checked_add(refunded_principal).unwrap()
        );
        assert_eq!(
            Balances::total_issuance(),
            issuance_before_claim
                .checked_add(refunded_principal)
                .unwrap()
        );
        assert_eq!(QueuedSwapRefundBalance::<Test>::get(refunded_staker), 0);
        assert_eq!(TotalQueuedSwapRefundBalance::<Test>::get(), 0);
    });
}

#[test]
fn queued_swaps_cannot_bypass_the_shared_transaction_rate_limit() {
    new_test_ext().execute_with(|| {
        let (subnet_id, from_validator_id, to_validator_id, stakers, shares) =
            setup_all_swap_sources();
        TxRateLimit::<Test>::put(3);
        System::set_block_number(
            System::block_number()
                .saturating_add(TxRateLimit::<Test>::get())
                .saturating_add(1),
        );

        let subnet_chunk = shares[0] / 4;
        assert!(subnet_chunk > 0);
        assert_ok!(Network::swap_from_subnet_to_subnet(
            RuntimeOrigin::signed(stakers[0].clone()),
            subnet_id,
            subnet_id,
            subnet_chunk,
            1,
            1,
            u32::MAX,
        ));
        let subnet_shares_after_first =
            AccountSubnetDelegateStakeShares::<Test>::get(&stakers[0], subnet_id);
        assert_err!(
            Network::swap_from_subnet_to_subnet(
                RuntimeOrigin::signed(stakers[0].clone()),
                subnet_id,
                subnet_id,
                subnet_chunk,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::TxRateLimitExceeded
        );
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&stakers[0], subnet_id),
            subnet_shares_after_first
        );

        let subnet_to_validator_chunk = shares[1] / 4;
        assert_ok!(Network::swap_from_subnet_to_validator(
            RuntimeOrigin::signed(stakers[1].clone()),
            subnet_id,
            to_validator_id,
            subnet_to_validator_chunk,
            1,
            1,
            u32::MAX,
        ));
        assert_err!(
            Network::swap_from_subnet_to_validator(
                RuntimeOrigin::signed(stakers[1].clone()),
                subnet_id,
                to_validator_id,
                subnet_to_validator_chunk,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::TxRateLimitExceeded
        );

        let validator_to_subnet_chunk = shares[2] / 4;
        assert_ok!(Network::swap_from_validator_to_subnet(
            RuntimeOrigin::signed(stakers[2].clone()),
            from_validator_id,
            subnet_id,
            validator_to_subnet_chunk,
            1,
            1,
            u32::MAX,
        ));
        assert_err!(
            Network::swap_from_validator_to_subnet(
                RuntimeOrigin::signed(stakers[2].clone()),
                from_validator_id,
                subnet_id,
                validator_to_subnet_chunk,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::TxRateLimitExceeded
        );

        let validator_chunk = shares[3] / 4;
        assert_ok!(Network::swap_from_validator_to_validator(
            RuntimeOrigin::signed(stakers[3].clone()),
            from_validator_id,
            to_validator_id,
            validator_chunk,
            1,
            1,
            u32::MAX,
        ));
        assert_err!(
            Network::swap_from_validator_to_validator(
                RuntimeOrigin::signed(stakers[3].clone()),
                from_validator_id,
                to_validator_id,
                validator_chunk,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::TxRateLimitExceeded
        );
        assert_queued_swap_principal_invariant();
    });
}

#[test]
fn test_queued_principal_overflow_rolls_back_all_four_swap_sources() {
    new_test_ext().execute_with(|| {
        let (subnet_id, from_validator_id, to_validator_id, stakers, shares) =
            setup_all_swap_sources();
        let subnet_pool_balance = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let subnet_pool_shares = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let validator_pool_balance = ValidatorDelegateStakeBalance::<Test>::get(from_validator_id);
        let validator_pool_shares = ValidatorDelegateStakeShares::<Test>::get(from_validator_id);
        let total_delegate_stake = TotalDelegateStake::<Test>::get();
        let total_validator_stake = TotalValidatorDelegateStakeBalance::<Test>::get();
        let next_id = NextSwapQueueId::<Test>::get();
        let event_count = System::events().len();

        TotalQueuedSwapPrincipal::<Test>::set(u128::MAX);

        assert_err!(
            Network::swap_from_subnet_to_subnet(
                RuntimeOrigin::signed(stakers[0].clone()),
                subnet_id,
                subnet_id,
                shares[0],
                1,
                1,
                u32::MAX,
            ),
            ArithmeticError::Overflow
        );
        assert_err!(
            Network::swap_from_subnet_to_validator(
                RuntimeOrigin::signed(stakers[1].clone()),
                subnet_id,
                to_validator_id,
                shares[1],
                1,
                1,
                u32::MAX,
            ),
            ArithmeticError::Overflow
        );
        assert_err!(
            Network::swap_from_validator_to_subnet(
                RuntimeOrigin::signed(stakers[2].clone()),
                from_validator_id,
                subnet_id,
                shares[2],
                1,
                1,
                u32::MAX,
            ),
            ArithmeticError::Overflow
        );
        assert_err!(
            Network::swap_from_validator_to_validator(
                RuntimeOrigin::signed(stakers[3].clone()),
                from_validator_id,
                to_validator_id,
                shares[3],
                1,
                1,
                u32::MAX,
            ),
            ArithmeticError::Overflow
        );

        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&stakers[0], subnet_id),
            shares[0]
        );
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&stakers[1], subnet_id),
            shares[1]
        );
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&stakers[2], from_validator_id),
            shares[2]
        );
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&stakers[3], from_validator_id),
            shares[3]
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            subnet_pool_balance
        );
        assert_eq!(
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id),
            subnet_pool_shares
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(from_validator_id),
            validator_pool_balance
        );
        assert_eq!(
            ValidatorDelegateStakeShares::<Test>::get(from_validator_id),
            validator_pool_shares
        );
        assert_eq!(TotalDelegateStake::<Test>::get(), total_delegate_stake);
        assert_eq!(
            TotalValidatorDelegateStakeBalance::<Test>::get(),
            total_validator_stake
        );
        assert!(SwapCallQueue::<Test>::iter().next().is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(SwapQueueCount::<Test>::get(), 0);
        assert_eq!(NextSwapQueueId::<Test>::get(), next_id);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), u128::MAX);
        assert_eq!(System::events().len(), event_count);
    });
}

#[test]
fn test_execute_ready_swap_calls() {
    new_test_ext().execute_with(|| {
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let end = 4;

        let name_1: Vec<u8> = "subnet-name".into();
        build_activated_subnet(name_1.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id_1 = SubnetName::<Test>::get(name_1.clone()).unwrap();
        let subnet_id_1_key_offset = get_subnet_id_key_offset(subnet_id_1);
        let validator_id = 1;

        let queues_count = 12;

        for n in 0..queues_count {
            let _ = Balances::deposit_creating(&account(n), amount + 500);
            if n % queues_count == 0 {
                // nothing in queue
                insert_to_subnet_swap_call_queue(account(n), subnet_id_1, amount);
                // Sanity check
                let user_shares =
                    AccountSubnetDelegateStakeShares::<Test>::get(&account(n), subnet_id_1);
                assert_eq!(user_shares, 0);
            } else {
                let subnet_node_id = end - 1;
                insert_to_validator_swap_call_queue(account(n), validator_id, amount);
                let user_shares =
                    AccountValidatorDelegateStakeShares::<Test>::get(&account(n), validator_id);
                assert_eq!(user_shares, 0);
            }
        }

        // SANITY CHECK EVERYTHING IS THERE QUEUED
        assert_eq!(SwapQueueOrder::<Test>::get().len(), queues_count as usize);
        assert_eq!(SwapCallQueue::<Test>::iter().count(), queues_count as usize);
        assert_eq!(
            TotalQueuedSwapPrincipal::<Test>::get(),
            amount.saturating_mul(queues_count as u128)
        );
        let queued_tvl = Network::get_total_network_tvl();
        assert_queued_swap_principal_invariant();
        let mut n = 0;
        for (_, call_queue) in SwapCallQueue::<Test>::iter() {
            let _n = n + 1;
            if n % queues_count == 0 {
                match &call_queue.call {
                    QueuedSwapCall::SwapToSubnetDelegateStake {
                        account_id,
                        to_subnet_id,
                        balance,
                        ..
                    } => {
                        assert_eq!(*account_id, account(n));
                        assert_eq!(*to_subnet_id, subnet_id_1);
                        assert_ne!(*balance, 0);
                        assert_ne!(*balance, u128::MAX);
                    }
                    QueuedSwapCall::SwapToValidatorDelegateStake { .. } => assert!(false),
                };
            } else {
                //
            }
            n += 1;
        }

        // NOTHING SHOULD BE EXECUTED
        let _ = Network::execute_ready_swap_calls(System::block_number(), &mut WeightMeter::new());
        assert_eq!(SwapQueueOrder::<Test>::get().len(), queues_count as usize);
        assert_eq!(SwapCallQueue::<Test>::iter().count(), queues_count as usize);
        assert_eq!(Network::get_total_network_tvl(), queued_tvl);
        assert_queued_swap_principal_invariant();

        // INCREASE BLOCKS TO BE ABLE TO EXECUTE
        System::set_block_number(System::block_number() + EpochLength::get() + 1);

        // Swaps SHOULD be executed
        let _ = Network::execute_ready_swap_calls(System::block_number(), &mut WeightMeter::new());

        // Ensure swaps removed from queue
        assert_eq!(SwapQueueOrder::<Test>::get().len(), 0 as usize);
        assert_eq!(SwapCallQueue::<Test>::iter().count(), 0 as usize);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_eq!(Network::get_total_network_tvl(), queued_tvl);
        assert_queued_swap_principal_invariant();

        // Ensure swaps were executed
        for n in 0..queues_count {
            if n % queues_count == 0 {
                // check subnet delegate stake balance
                let user_shares =
                    AccountSubnetDelegateStakeShares::<Test>::get(&account(n), subnet_id_1);
                assert!(user_shares > 0);
            } else {
                // check node delegate stake balance
                let subnet_node_id = end - 1;
                let user_shares =
                    AccountValidatorDelegateStakeShares::<Test>::get(&account(n), validator_id);
                assert!(user_shares > 0);
            }
        }
    });
}

#[test]
fn test_queued_principal_underflow_rotates_and_allows_trailing_credit() {
    new_test_ext().execute_with(|| {
        const VALIDATOR_ID: u32 = 71;
        let trailing_balance = MinDelegateStakeDeposit::<Test>::get();
        let blocked_balance = trailing_balance
            .checked_add(1)
            .expect("the minimum deposit leaves room for a larger head item");

        manual_insert_validator(VALIDATOR_ID, 970, 971);
        let blocked_staker = account(972);
        let trailing_staker = account(973);
        let blocked_queue_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            blocked_staker.clone(),
            QueuedSwapSource::ValidatorDelegate,
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id: blocked_staker.clone(),
                to_validator_id: VALIDATOR_ID,
                balance: blocked_balance,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        let trailing_queue_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            trailing_staker.clone(),
            QueuedSwapSource::ValidatorDelegate,
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id: trailing_staker.clone(),
                to_validator_id: VALIDATOR_ID,
                balance: trailing_balance,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        let blocked_item = SwapCallQueue::<Test>::get(blocked_queue_id).unwrap();

        // The corrupted total cannot cover the head, but it can cover the trailing item.
        TotalQueuedSwapPrincipal::<Test>::set(trailing_balance);
        let execution_block = blocked_item
            .queued_at_block
            .saturating_add(blocked_item.execute_after_blocks);
        System::set_block_number(execution_block);
        Network::execute_ready_swap_calls_with_limit(execution_block, 2, &mut WeightMeter::new());

        assert_eq!(
            SwapCallQueue::<Test>::get(blocked_queue_id),
            Some(blocked_item)
        );
        assert!(SwapCallQueue::<Test>::get(trailing_queue_id).is_none());
        assert_eq!(
            SwapQueueOrder::<Test>::get().as_slice(),
            &[blocked_queue_id]
        );
        assert_eq!(SwapQueueCount::<Test>::get(), 1);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&blocked_staker, VALIDATOR_ID),
            0
        );
        assert!(
            AccountValidatorDelegateStakeShares::<Test>::get(&trailing_staker, VALIDATOR_ID) > 0
        );
    });
}

#[test]
fn test_queued_principal_underflow_defers_before_refund() {
    new_test_ext().execute_with(|| {
        const QUEUED_BALANCE: u128 = 1_000;

        let staker = account(971);
        let queue_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            staker.clone(),
            QueuedSwapSource::SubnetDelegate,
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id: staker.clone(),
                to_subnet_id: u32::MAX,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        let queued_item = SwapCallQueue::<Test>::get(queue_id).unwrap();
        let queue_order = SwapQueueOrder::<Test>::get();
        let event_count = System::events().len();

        TotalQueuedSwapPrincipal::<Test>::set(QUEUED_BALANCE - 1);
        let execution_block = queued_item
            .queued_at_block
            .saturating_add(queued_item.execute_after_blocks);
        System::set_block_number(execution_block);
        Network::execute_ready_swap_calls(execution_block, &mut WeightMeter::new());

        assert_eq!(SwapCallQueue::<Test>::get(queue_id), Some(queued_item));
        assert_eq!(SwapQueueOrder::<Test>::get(), queue_order);
        assert_eq!(SwapQueueCount::<Test>::get(), 1);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), QUEUED_BALANCE - 1);
        assert!(StakeUnbondingLedger::<Test>::get(&staker).is_empty());
        assert_eq!(TotalNetworkUnbondingBalance::<Test>::get(), 0);
        assert_eq!(System::events().len(), event_count);
    });
}

#[test]
fn test_immature_head_stops_without_reordering_ready_tail() {
    new_test_ext().execute_with(|| {
        const VALIDATOR_ID: u32 = 72;
        const QUEUED_BALANCE: u128 = 1_000;

        manual_insert_validator(VALIDATOR_ID, 974, 975);
        let head_staker = account(976);
        let tail_staker = account(977);
        let head_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            head_staker.clone(),
            QueuedSwapSource::ValidatorDelegate,
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id: head_staker.clone(),
                to_validator_id: VALIDATOR_ID,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        let tail_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            tail_staker.clone(),
            QueuedSwapSource::ValidatorDelegate,
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id: tail_staker.clone(),
                to_validator_id: VALIDATOR_ID,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        SwapCallQueue::<Test>::mutate(tail_id, |maybe_item| {
            maybe_item.as_mut().unwrap().execute_after_blocks = 0;
        });

        let head_item = SwapCallQueue::<Test>::get(head_id).unwrap();
        let tail_item = SwapCallQueue::<Test>::get(tail_id).unwrap();
        let block = System::block_number();
        Network::execute_ready_swap_calls_with_limit(block, 2, &mut WeightMeter::new());

        assert_eq!(SwapCallQueue::<Test>::get(head_id), Some(head_item));
        assert_eq!(SwapCallQueue::<Test>::get(tail_id), Some(tail_item));
        assert_eq!(
            SwapQueueOrder::<Test>::get().as_slice(),
            &[head_id, tail_id]
        );
        assert_eq!(SwapQueueCount::<Test>::get(), 2);
        assert_eq!(
            TotalQueuedSwapPrincipal::<Test>::get(),
            QUEUED_BALANCE.saturating_mul(2)
        );
        assert_queued_swap_principal_invariant();
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&tail_staker, VALIDATOR_ID),
            0
        );
    });
}

#[test]
fn test_stale_queue_id_is_dropped_and_ready_tail_progresses() {
    new_test_ext().execute_with(|| {
        const VALIDATOR_ID: u32 = 73;
        const QUEUED_BALANCE: u128 = 1_000;

        manual_insert_validator(VALIDATOR_ID, 978, 979);
        let stale_staker = account(980);
        let valid_staker = account(981);
        let stale_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            stale_staker.clone(),
            QueuedSwapSource::ValidatorDelegate,
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id: stale_staker.clone(),
                to_validator_id: VALIDATOR_ID,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        let valid_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            valid_staker.clone(),
            QueuedSwapSource::ValidatorDelegate,
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id: valid_staker.clone(),
                to_validator_id: VALIDATOR_ID,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));

        // Simulate an order/map inconsistency while keeping principal accounting tied to the map.
        SwapCallQueue::<Test>::remove(stale_id);
        TotalQueuedSwapPrincipal::<Test>::set(QUEUED_BALANCE);
        assert_queued_swap_principal_invariant();

        let execution_block = System::block_number()
            .saturating_add(EpochLength::get())
            .saturating_add(1);
        System::set_block_number(execution_block);
        Network::execute_ready_swap_calls_with_limit(execution_block, 2, &mut WeightMeter::new());

        assert!(SwapCallQueue::<Test>::get(valid_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(SwapQueueCount::<Test>::get(), 0);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_queued_swap_principal_invariant();
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&stale_staker, VALIDATOR_ID),
            0
        );
        assert!(AccountValidatorDelegateStakeShares::<Test>::get(&valid_staker, VALIDATOR_ID) > 0);
    });
}

#[test]
fn test_zero_unbonding_capacity_cannot_block_refund_or_ready_tail() {
    new_test_ext().execute_with(|| {
        const VALIDATOR_ID: u32 = 74;
        const QUEUED_BALANCE: u128 = 1_000;

        manual_insert_validator(VALIDATOR_ID, 982, 983);
        MaxUnbondings::<Test>::set(0);
        let blocked_staker = account(984);
        let valid_staker = account(985);
        let blocked_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            blocked_staker.clone(),
            QueuedSwapSource::SubnetDelegate,
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id: blocked_staker.clone(),
                to_subnet_id: u32::MAX,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        let valid_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            valid_staker.clone(),
            QueuedSwapSource::ValidatorDelegate,
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id: valid_staker.clone(),
                to_validator_id: VALIDATOR_ID,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        let execution_block = System::block_number()
            .saturating_add(EpochLength::get())
            .saturating_add(1);
        System::set_block_number(execution_block);
        Network::execute_ready_swap_calls_with_limit(execution_block, 2, &mut WeightMeter::new());

        assert!(SwapCallQueue::<Test>::get(blocked_id).is_none());
        assert!(SwapCallQueue::<Test>::get(valid_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(SwapQueueCount::<Test>::get(), 0);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_queued_swap_principal_invariant();
        assert!(StakeUnbondingLedger::<Test>::get(&blocked_staker).is_empty());
        assert_eq!(TotalNetworkUnbondingBalance::<Test>::get(), 0);
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(&blocked_staker),
            QUEUED_BALANCE
        );
        assert_eq!(TotalQueuedSwapRefundBalance::<Test>::get(), QUEUED_BALANCE);
        assert!(AccountValidatorDelegateStakeShares::<Test>::get(&valid_staker, VALIDATOR_ID) > 0);
    });
}

#[test]
fn test_execute_ready_swap_refunds_when_destination_subnet_removed() {
    new_test_ext().execute_with(|| {
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let end = 4;

        let from_subnet_name: Vec<u8> = "from-subnet".into();
        build_activated_subnet(
            from_subnet_name.clone(),
            0,
            end,
            deposit_amount,
            stake_amount,
        );
        let from_subnet_id = SubnetName::<Test>::get(from_subnet_name.clone()).unwrap();

        let to_subnet_name: Vec<u8> = "to-subnet".into();
        build_activated_subnet(to_subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let to_subnet_id = SubnetName::<Test>::get(to_subnet_name.clone()).unwrap();

        let staker = account(255);
        let _ = Balances::deposit_creating(&staker, amount + 500);

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(staker.clone()),
            from_subnet_id,
            amount,
            1,
        ));

        let from_delegate_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, from_subnet_id);
        assert!(from_delegate_shares > 0);

        let queue_id = NextSwapQueueId::<Test>::get();
        let tvl_before_swap = Network::get_total_network_tvl();
        assert_ok!(Network::swap_from_subnet_to_subnet(
            RuntimeOrigin::signed(staker.clone()),
            from_subnet_id,
            to_subnet_id,
            from_delegate_shares,
            1,
            1,
            u32::MAX,
        ));

        let queued_balance = SwapCallQueue::<Test>::get(queue_id)
            .unwrap()
            .call
            .get_queue_balance();
        assert!(queued_balance > 0);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), queued_balance);
        assert_eq!(Network::get_total_network_tvl(), tvl_before_swap);
        assert_queued_swap_principal_invariant();
        assert!(StakeUnbondingLedger::<Test>::get(&staker).is_empty());

        Network::do_remove_subnet(to_subnet_id, SubnetRemovalReason::Owner);
        assert!(!SubnetsData::<Test>::contains_key(to_subnet_id));
        let tvl_before_refund = Network::get_total_network_tvl();

        System::set_block_number(System::block_number() + EpochLength::get() + 1);
        let execution_block = System::block_number();
        Network::execute_ready_swap_calls(execution_block, &mut WeightMeter::new());

        assert!(SwapCallQueue::<Test>::get(queue_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_eq!(Network::get_total_network_tvl(), tvl_before_refund);
        assert_queued_swap_principal_invariant();
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, to_subnet_id),
            0
        );

        assert!(StakeUnbondingLedger::<Test>::get(&staker).is_empty());
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(&staker),
            queued_balance
        );
        assert_eq!(TotalQueuedSwapRefundBalance::<Test>::get(), queued_balance);
    });
}

#[test]
fn test_on_initialize_refunds_under_minimum_subnet_swap_without_mutating_destination() {
    new_test_ext().execute_with(|| {
        const QUEUED_BALANCE: u128 = 1_000;
        const POOL_SHARES: u128 = 1_000_000_001;
        const POOL_BALANCE: u128 = 10_000_000_000_000;

        let subnet_name: Vec<u8> = "zero-share-subnet-destination".into();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let staker = account(900);

        TotalSubnetDelegateStakeShares::<Test>::insert(subnet_id, POOL_SHARES);
        TotalSubnetDelegateStakeBalance::<Test>::insert(subnet_id, POOL_BALANCE);
        TotalSubnetDelegateStakeCirculatingShares::<Test>::insert(subnet_id, 0);
        assert_eq!(
            Network::convert_to_shares(QUEUED_BALANCE, POOL_SHARES, POOL_BALANCE),
            0
        );

        let destination_shares_before =
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id);
        let total_delegate_stake_before = TotalDelegateStake::<Test>::get();
        let total_unbonding_before = TotalNetworkUnbondingBalance::<Test>::get();
        let queue_id = NextSwapQueueId::<Test>::get();

        assert_ok!(Network::queue_swap(
            staker.clone(),
            QueuedSwapSource::SubnetDelegate,
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id: staker.clone(),
                to_subnet_id: subnet_id,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), QUEUED_BALANCE);
        assert_queued_swap_principal_invariant();
        assert_eq!(SwapQueueCount::<Test>::get(), 1);

        // Exercise the full hook at general-epoch slot zero, where preliminaries and swap queue
        // processing share the bounded on_initialize budget.
        let execution_block = System::block_number()
            .saturating_div(EpochLength::get())
            .saturating_add(2)
            .saturating_mul(EpochLength::get());
        assert_eq!(execution_block % EpochLength::get(), 0);
        System::set_block_number(execution_block);
        Network::on_initialize(execution_block);

        assert!(SwapCallQueue::<Test>::get(queue_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(SwapQueueCount::<Test>::get(), 0);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_queued_swap_principal_invariant();
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id),
            destination_shares_before
        );
        assert_eq!(
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id),
            POOL_SHARES
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            POOL_BALANCE
        );
        assert_eq!(
            TotalDelegateStake::<Test>::get(),
            total_delegate_stake_before
        );

        assert!(StakeUnbondingLedger::<Test>::get(&staker).is_empty());
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(&staker),
            QUEUED_BALANCE
        );
        assert_eq!(TotalQueuedSwapRefundBalance::<Test>::get(), QUEUED_BALANCE);
        assert_eq!(
            TotalNetworkUnbondingBalance::<Test>::get(),
            total_unbonding_before
        );
        let events = network_events();
        assert!(
            events.iter().any(|event| {
                matches!(
                    event,
                    Event::SwapCallRefunded {
                        id,
                        account_id,
                        balance,
                        reason: SwapRefundReason::MinimumSharesNotMet,
                    } if *id == queue_id
                        && account_id == &staker
                        && *balance == QUEUED_BALANCE
                )
            }),
            "unexpected events: {events:?}"
        );
    });
}

#[test]
fn test_execute_ready_swap_refunds_under_minimum_validator_without_mutating_destination() {
    new_test_ext().execute_with(|| {
        const VALIDATOR_ID: u32 = 77;
        const QUEUED_BALANCE: u128 = 1_000;
        const POOL_SHARES: u128 = 1_000_000_001;
        const POOL_BALANCE: u128 = 10_000_000_000_000;

        manual_insert_validator(VALIDATOR_ID, 901, 902);
        ValidatorDelegateStakeShares::<Test>::insert(VALIDATOR_ID, POOL_SHARES);
        ValidatorDelegateStakeBalance::<Test>::insert(VALIDATOR_ID, POOL_BALANCE);
        ValidatorDelegateStakeCirculatingShares::<Test>::insert(VALIDATOR_ID, 0);
        TotalValidatorDelegateStakeBalance::<Test>::set(POOL_BALANCE);
        assert_eq!(
            Network::convert_to_shares(QUEUED_BALANCE, POOL_SHARES, POOL_BALANCE),
            0
        );

        let staker = account(903);
        let destination_shares_before =
            AccountValidatorDelegateStakeShares::<Test>::get(&staker, VALIDATOR_ID);
        let total_validator_stake_before = TotalValidatorDelegateStakeBalance::<Test>::get();
        let total_unbonding_before = TotalNetworkUnbondingBalance::<Test>::get();
        let queue_id = NextSwapQueueId::<Test>::get();

        assert_ok!(Network::queue_swap(
            staker.clone(),
            QueuedSwapSource::ValidatorDelegate,
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id: staker.clone(),
                to_validator_id: VALIDATOR_ID,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), QUEUED_BALANCE);
        assert_queued_swap_principal_invariant();

        let execution_block = System::block_number()
            .saturating_add(EpochLength::get())
            .saturating_add(1);
        System::set_block_number(execution_block);
        Network::execute_ready_swap_calls(execution_block, &mut WeightMeter::new());

        assert!(SwapCallQueue::<Test>::get(queue_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(SwapQueueCount::<Test>::get(), 0);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_queued_swap_principal_invariant();
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&staker, VALIDATOR_ID),
            destination_shares_before
        );
        assert_eq!(
            ValidatorDelegateStakeShares::<Test>::get(VALIDATOR_ID),
            POOL_SHARES
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(VALIDATOR_ID),
            POOL_BALANCE
        );
        assert_eq!(
            TotalValidatorDelegateStakeBalance::<Test>::get(),
            total_validator_stake_before
        );

        assert!(StakeUnbondingLedger::<Test>::get(&staker).is_empty());
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(&staker),
            QUEUED_BALANCE
        );
        assert_eq!(TotalQueuedSwapRefundBalance::<Test>::get(), QUEUED_BALANCE);
        assert_eq!(
            TotalNetworkUnbondingBalance::<Test>::get(),
            total_unbonding_before
        );
        assert!(network_events().iter().any(|event| {
            matches!(
                event,
                Event::SwapCallRefunded {
                    id,
                    account_id,
                    balance,
                    reason: SwapRefundReason::MinimumSharesNotMet,
                } if *id == queue_id
                    && account_id == &staker
                    && *balance == QUEUED_BALANCE
            )
        }));
    });
}

#[test]
fn test_full_unbonding_ledger_cannot_permanently_block_swap_refund_or_tail() {
    new_test_ext().execute_with(|| {
        const VALIDATOR_ID: u32 = 1;
        const QUEUED_BALANCE: u128 = 1_000;

        manual_insert_validator(VALIDATOR_ID, 910, 911);
        let blocked_staker = account(912);
        let trailing_staker = account(913);
        let blocked_queue_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            blocked_staker.clone(),
            QueuedSwapSource::SubnetDelegate,
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id: blocked_staker.clone(),
                to_subnet_id: u32::MAX,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        let trailing_queue_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            trailing_staker.clone(),
            QueuedSwapSource::ValidatorDelegate,
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id: trailing_staker.clone(),
                to_validator_id: VALIDATOR_ID,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));

        let execution_block = System::block_number()
            .saturating_add(EpochLength::get())
            .saturating_add(1);
        System::set_block_number(execution_block);
        assert_eq!(Balances::free_balance(&blocked_staker), 0);
        let mut full_ledger = sp_std::collections::btree_map::BTreeMap::new();
        for offset in 0..MaxUnbondings::<Test>::get() {
            full_ledger.insert(
                execution_block.saturating_sub(offset),
                UnbondingEntry {
                    network: 0,
                    overwatch: 0,
                },
            );
        }
        assert_eq!(full_ledger.len() as u32, MaxUnbondings::<Test>::get());
        let full_ledger_total = 0;
        StakeUnbondingLedger::<Test>::insert(&blocked_staker, full_ledger.clone());
        TotalNetworkUnbondingBalance::<Test>::set(full_ledger_total);

        assert_eq!(
            TotalQueuedSwapPrincipal::<Test>::get(),
            QUEUED_BALANCE.saturating_mul(2)
        );
        assert_queued_swap_principal_invariant();

        Network::execute_ready_swap_calls_with_limit(execution_block, 1, &mut WeightMeter::new());

        assert!(SwapCallQueue::<Test>::get(blocked_queue_id).is_none());
        assert!(SwapCallQueue::<Test>::contains_key(trailing_queue_id));
        assert_eq!(
            SwapQueueOrder::<Test>::get().as_slice(),
            &[trailing_queue_id]
        );
        assert_eq!(SwapQueueCount::<Test>::get(), 1);
        assert_eq!(
            StakeUnbondingLedger::<Test>::get(&blocked_staker),
            full_ledger
        );
        assert_eq!(
            TotalNetworkUnbondingBalance::<Test>::get(),
            full_ledger_total
        );
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), QUEUED_BALANCE);
        assert_queued_swap_principal_invariant();
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(&blocked_staker),
            QUEUED_BALANCE
        );
        assert_eq!(TotalQueuedSwapRefundBalance::<Test>::get(), QUEUED_BALANCE);
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&trailing_staker, VALIDATOR_ID),
            0
        );

        Network::execute_ready_swap_calls_with_limit(execution_block, 1, &mut WeightMeter::new());

        assert!(SwapCallQueue::<Test>::get(blocked_queue_id).is_none());
        assert!(SwapCallQueue::<Test>::get(trailing_queue_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(SwapQueueCount::<Test>::get(), 0);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_queued_swap_principal_invariant();
        assert!(
            AccountValidatorDelegateStakeShares::<Test>::get(&trailing_staker, VALIDATOR_ID) > 0
        );
        assert_eq!(
            StakeUnbondingLedger::<Test>::get(&blocked_staker),
            full_ledger
        );
        assert_eq!(
            TotalNetworkUnbondingBalance::<Test>::get(),
            full_ledger_total
        );
        assert!(network_events().iter().any(|event| {
            matches!(
                event,
                Event::SwapCallRefunded {
                    id,
                    account_id,
                    balance,
                    reason: SwapRefundReason::DestinationMissing,
                } if *id == blocked_queue_id
                    && account_id == &blocked_staker
                    && *balance == QUEUED_BALANCE
            )
        }));
        assert!(network_events().iter().any(|event| {
            matches!(
                event,
                Event::SwapCallCredited {
                    id,
                    account_id,
                    balance,
                    shares,
                } if *id == trailing_queue_id
                    && account_id == &trailing_staker
                    && *balance == QUEUED_BALANCE
                    && *shares > 0
            )
        }));
        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            blocked_staker.clone()
        )));
        assert_eq!(Balances::free_balance(&blocked_staker), QUEUED_BALANCE);
        assert_eq!(QueuedSwapRefundBalance::<Test>::get(&blocked_staker), 0);
        assert_eq!(TotalQueuedSwapRefundBalance::<Test>::get(), 0);
    });
}

#[test]
fn test_terminal_refunds_respect_execution_limit_and_do_not_touch_unbondings() {
    new_test_ext().execute_with(|| {
        const VALIDATOR_ID: u32 = 75;
        const QUEUED_BALANCE: u128 = 1_000;
        const POOL_SHARES: u128 = 1_000_000_001;
        const POOL_BALANCE: u128 = 100;

        let staker = account(914);
        manual_insert_validator(VALIDATOR_ID, 916, 917);
        // Make the second item consume the destination-credit path before an overflow redirects
        // it to the dedicated refund balance. The first goes directly to the refund path.
        AccountValidatorDelegateStakeShares::<Test>::insert(&staker, VALIDATOR_ID, u128::MAX);
        ValidatorDelegateStakeShares::<Test>::insert(VALIDATOR_ID, POOL_SHARES);
        ValidatorDelegateStakeBalance::<Test>::insert(VALIDATOR_ID, POOL_BALANCE);
        ValidatorDelegateStakeCirculatingShares::<Test>::insert(VALIDATOR_ID, 0);
        TotalValidatorDelegateStakeBalance::<Test>::set(POOL_BALANCE);
        assert!(Network::convert_to_shares(QUEUED_BALANCE, POOL_SHARES, POOL_BALANCE) > 0);

        let first_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            staker.clone(),
            QueuedSwapSource::SubnetDelegate,
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id: staker.clone(),
                to_subnet_id: u32::MAX,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        let second_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            staker.clone(),
            QueuedSwapSource::ValidatorDelegate,
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id: staker.clone(),
                to_validator_id: VALIDATOR_ID,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));

        let execution_block = System::block_number()
            .saturating_add(EpochLength::get())
            .saturating_add(1);
        System::set_block_number(execution_block);
        let claim_block = execution_block.saturating_add(
            DelegateStakeCooldownEpochs::<Test>::get().saturating_mul(EpochLength::get()),
        );
        let mut full_ledger = sp_std::collections::btree_map::BTreeMap::new();
        for offset in 1..=MaxUnbondings::<Test>::get() {
            full_ledger.insert(
                claim_block.saturating_add(offset),
                UnbondingEntry {
                    network: offset as u128,
                    overwatch: 0,
                },
            );
        }
        assert_eq!(full_ledger.len() as u32, MaxUnbondings::<Test>::get());
        let full_ledger_total = full_ledger
            .values()
            .map(|entry| entry.network)
            .fold(0u128, |total, balance| total.saturating_add(balance));
        StakeUnbondingLedger::<Test>::insert(&staker, full_ledger.clone());
        TotalNetworkUnbondingBalance::<Test>::set(full_ledger_total);

        let mut queue_only_meter = WeightMeter::new();
        Network::execute_ready_swap_calls_with_limit(execution_block, 0, &mut queue_only_meter);
        assert_eq!(
            SwapQueueOrder::<Test>::get().as_slice(),
            &[first_id, second_id]
        );

        let mut direct_refund_meter = WeightMeter::new();
        Network::execute_ready_swap_calls_with_limit(execution_block, 1, &mut direct_refund_meter);
        assert_eq!(SwapQueueOrder::<Test>::get().as_slice(), &[second_id]);
        assert!(SwapCallQueue::<Test>::get(first_id).is_none());
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(&staker),
            QUEUED_BALANCE
        );

        let mut credit_then_refund_meter = WeightMeter::new();
        Network::execute_ready_swap_calls_with_limit(
            execution_block,
            1,
            &mut credit_then_refund_meter,
        );
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert!(SwapCallQueue::<Test>::get(second_id).is_none());
        assert!(
            credit_then_refund_meter.consumed().ref_time()
                > direct_refund_meter.consumed().ref_time()
        );
        assert_eq!(SwapQueueCount::<Test>::get(), 0);
        assert_eq!(SwapCallQueue::<Test>::iter().count(), 0);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(&staker),
            QUEUED_BALANCE.saturating_mul(2)
        );
        assert_eq!(
            TotalQueuedSwapRefundBalance::<Test>::get(),
            QUEUED_BALANCE.saturating_mul(2)
        );
        assert_queued_swap_principal_invariant();
        assert_eq!(StakeUnbondingLedger::<Test>::get(&staker), full_ledger);
        assert_eq!(
            TotalNetworkUnbondingBalance::<Test>::get(),
            full_ledger_total
        );
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&staker, VALIDATOR_ID),
            u128::MAX
        );
        assert_eq!(
            ValidatorDelegateStakeShares::<Test>::get(VALIDATOR_ID),
            POOL_SHARES
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(VALIDATOR_ID),
            POOL_BALANCE
        );
    });
}

#[test]
fn test_full_unbonding_ledger_is_untouched_by_dedicated_swap_refund() {
    new_test_ext().execute_with(|| {
        const QUEUED_BALANCE: u128 = 1_000;

        let staker = account(915);
        let queue_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            staker.clone(),
            QueuedSwapSource::SubnetDelegate,
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id: staker.clone(),
                to_subnet_id: u32::MAX,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        let execution_block = System::block_number()
            .saturating_add(EpochLength::get())
            .saturating_add(1);
        System::set_block_number(execution_block);
        let claim_block = execution_block.saturating_add(
            DelegateStakeCooldownEpochs::<Test>::get().saturating_mul(EpochLength::get()),
        );

        assert!(MaxUnbondings::<Test>::get() > 0);
        let mut full_ledger = sp_std::collections::btree_map::BTreeMap::new();
        for offset in 0..MaxUnbondings::<Test>::get() {
            full_ledger.insert(
                claim_block.saturating_add(offset),
                UnbondingEntry {
                    network: offset as u128 + 1,
                    overwatch: 0,
                },
            );
        }
        let full_ledger_total = full_ledger
            .values()
            .map(|entry| entry.network)
            .fold(0u128, |total, balance| total.saturating_add(balance));
        StakeUnbondingLedger::<Test>::insert(&staker, full_ledger.clone());
        TotalNetworkUnbondingBalance::<Test>::set(full_ledger_total);

        Network::execute_ready_swap_calls_with_limit(execution_block, 1, &mut WeightMeter::new());

        assert!(SwapCallQueue::<Test>::get(queue_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(SwapQueueCount::<Test>::get(), 0);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_queued_swap_principal_invariant();
        assert_eq!(StakeUnbondingLedger::<Test>::get(&staker), full_ledger);
        assert_eq!(
            TotalNetworkUnbondingBalance::<Test>::get(),
            full_ledger_total
        );
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(&staker),
            QUEUED_BALANCE
        );
        assert_eq!(TotalQueuedSwapRefundBalance::<Test>::get(), QUEUED_BALANCE);
    });
}

#[test]
fn test_queue_full_rolls_back_all_four_swap_sources() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "transactional-swap-source".into();
        let amount = 1_000_000_000_000_000_000_000u128;
        build_activated_subnet(
            subnet_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let from_validator_id = 1;
        let to_validator_id = 2;
        let staker = account(920);
        let _ = Balances::deposit_creating(&staker, amount.saturating_mul(2).saturating_add(500));

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(staker.clone()),
            subnet_id,
            amount,
            1,
        ));
        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(staker.clone()),
            from_validator_id,
            amount,
            1,
        ));

        let subnet_source_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id);
        let validator_source_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(&staker, from_validator_id);
        assert!(subnet_source_shares > 0);
        assert!(validator_source_shares > 0);

        let max_queue_len = <Test as crate::Config>::MaxSwapQueueLength::get();
        SwapQueueOrder::<Test>::mutate(|queue| {
            for id in 0..max_queue_len {
                queue
                    .try_push(id)
                    .expect("test queue remains inside its bound");
            }
        });
        SwapQueueCount::<Test>::set(max_queue_len);
        NextSwapQueueId::<Test>::set(max_queue_len);

        let queue_before = SwapQueueOrder::<Test>::get();
        let queue_count_before = SwapQueueCount::<Test>::get();
        let next_id_before = NextSwapQueueId::<Test>::get();
        let queued_principal_before = TotalQueuedSwapPrincipal::<Test>::get();
        let queued_item_count_before = SwapCallQueue::<Test>::iter().count();
        let event_count_before = System::events().len();
        let subnet_account_shares_before =
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id);
        let subnet_pool_shares_before = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let subnet_pool_balance_before = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let total_delegate_stake_before = TotalDelegateStake::<Test>::get();
        let validator_account_shares_before =
            AccountValidatorDelegateStakeShares::<Test>::get(&staker, from_validator_id);
        let validator_pool_shares_before =
            ValidatorDelegateStakeShares::<Test>::get(from_validator_id);
        let validator_pool_balance_before =
            ValidatorDelegateStakeBalance::<Test>::get(from_validator_id);
        let total_validator_stake_before = TotalValidatorDelegateStakeBalance::<Test>::get();

        assert_err!(
            Network::swap_from_subnet_to_subnet(
                RuntimeOrigin::signed(staker.clone()),
                subnet_id,
                subnet_id,
                subnet_source_shares,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::SwapQueueFull
        );
        assert_err!(
            Network::swap_from_subnet_to_validator(
                RuntimeOrigin::signed(staker.clone()),
                subnet_id,
                to_validator_id,
                subnet_source_shares,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::SwapQueueFull
        );
        assert_err!(
            Network::swap_from_validator_to_subnet(
                RuntimeOrigin::signed(staker.clone()),
                from_validator_id,
                subnet_id,
                validator_source_shares,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::SwapQueueFull
        );
        assert_err!(
            Network::swap_from_validator_to_validator(
                RuntimeOrigin::signed(staker.clone()),
                from_validator_id,
                to_validator_id,
                validator_source_shares,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::SwapQueueFull
        );

        assert_eq!(SwapQueueOrder::<Test>::get(), queue_before);
        assert_eq!(SwapQueueCount::<Test>::get(), queue_count_before);
        assert_eq!(NextSwapQueueId::<Test>::get(), next_id_before);
        assert_eq!(
            TotalQueuedSwapPrincipal::<Test>::get(),
            queued_principal_before
        );
        assert_queued_swap_principal_invariant();
        assert_eq!(
            SwapCallQueue::<Test>::iter().count(),
            queued_item_count_before
        );
        assert_eq!(System::events().len(), event_count_before);
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id),
            subnet_account_shares_before
        );
        assert_eq!(
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id),
            subnet_pool_shares_before
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            subnet_pool_balance_before
        );
        assert_eq!(
            TotalDelegateStake::<Test>::get(),
            total_delegate_stake_before
        );
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&staker, from_validator_id),
            validator_account_shares_before
        );
        assert_eq!(
            ValidatorDelegateStakeShares::<Test>::get(from_validator_id),
            validator_pool_shares_before
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(from_validator_id),
            validator_pool_balance_before
        );
        assert_eq!(
            TotalValidatorDelegateStakeBalance::<Test>::get(),
            total_validator_stake_before
        );
    });
}

#[test]
fn test_swap_queue_id_exhaustion_does_not_overwrite_existing_item() {
    new_test_ext().execute_with(|| {
        let queue_id = u32::MAX;
        let owner = account(930);
        let original_item = QueuedSwapItem {
            id: queue_id,
            call: QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id: owner.clone(),
                to_subnet_id: 1,
                balance: 1_000,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
            queued_at_block: System::block_number(),
            execute_after_blocks: EpochLength::get(),
        };
        SwapCallQueue::<Test>::insert(queue_id, original_item.clone());
        SwapQueueOrder::<Test>::mutate(|queue| {
            queue.try_push(queue_id).unwrap();
        });
        SwapQueueCount::<Test>::set(1);
        NextSwapQueueId::<Test>::set(queue_id);
        TotalQueuedSwapPrincipal::<Test>::set(original_item.call.get_queue_balance());
        assert_queued_swap_principal_invariant();

        let queue_before = SwapQueueOrder::<Test>::get();
        let event_count_before = System::events().len();
        assert_err!(
            Network::queue_swap(
                account(931),
                QueuedSwapSource::ValidatorDelegate,
                QueuedSwapCall::SwapToValidatorDelegateStake {
                    account_id: account(931),
                    to_validator_id: 2,
                    balance: 9_999,
                    min_shares_out: 1,
                    execute_before_block: u32::MAX,
                },
            ),
            Error::<Test>::SwapQueueIdExhausted
        );

        assert_eq!(SwapCallQueue::<Test>::get(queue_id), Some(original_item));
        assert_eq!(SwapCallQueue::<Test>::iter().count(), 1);
        assert_eq!(SwapQueueOrder::<Test>::get(), queue_before);
        assert_eq!(SwapQueueCount::<Test>::get(), 1);
        assert_eq!(NextSwapQueueId::<Test>::get(), u32::MAX);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 1_000);
        assert_queued_swap_principal_invariant();
        assert_eq!(System::events().len(), event_count_before);
    });
}

#[test]
fn test_subnet_credit_overflow_refunds_without_destination_mutation() {
    new_test_ext().execute_with(|| {
        const QUEUED_BALANCE: u128 = 1_000;
        const POOL_SHARES: u128 = 1_000_000_001;
        const POOL_BALANCE: u128 = 100;

        let subnet_name: Vec<u8> = "overflow-subnet-destination".into();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let staker = account(940);
        AccountSubnetDelegateStakeShares::<Test>::insert(&staker, subnet_id, u128::MAX);
        TotalSubnetDelegateStakeShares::<Test>::insert(subnet_id, POOL_SHARES);
        TotalSubnetDelegateStakeBalance::<Test>::insert(subnet_id, POOL_BALANCE);
        TotalSubnetDelegateStakeCirculatingShares::<Test>::insert(subnet_id, 0);
        assert!(Network::convert_to_shares(QUEUED_BALANCE, POOL_SHARES, POOL_BALANCE) > 0);

        let destination_account_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id);
        let total_delegate_stake = TotalDelegateStake::<Test>::get();
        let queue_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            staker.clone(),
            QueuedSwapSource::SubnetDelegate,
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id: staker.clone(),
                to_subnet_id: subnet_id,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), QUEUED_BALANCE);
        assert_queued_swap_principal_invariant();

        let execution_block = System::block_number()
            .saturating_add(EpochLength::get())
            .saturating_add(1);
        System::set_block_number(execution_block);
        Network::execute_ready_swap_calls(execution_block, &mut WeightMeter::new());

        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id),
            destination_account_shares
        );
        assert_eq!(
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id),
            POOL_SHARES
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            POOL_BALANCE
        );
        assert_eq!(TotalDelegateStake::<Test>::get(), total_delegate_stake);

        assert!(StakeUnbondingLedger::<Test>::get(&staker).is_empty());
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(&staker),
            QUEUED_BALANCE
        );
        assert_eq!(TotalQueuedSwapRefundBalance::<Test>::get(), QUEUED_BALANCE);
        assert!(SwapCallQueue::<Test>::get(queue_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(SwapQueueCount::<Test>::get(), 0);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_queued_swap_principal_invariant();
        assert!(network_events().iter().any(|event| {
            matches!(
                event,
                Event::SwapCallRefunded {
                    id,
                    account_id,
                    balance,
                    reason: SwapRefundReason::DestinationCreditOverflow,
                } if *id == queue_id && account_id == &staker && *balance == QUEUED_BALANCE
            )
        }));
    });
}

#[test]
fn test_validator_credit_overflow_refunds_without_destination_mutation() {
    new_test_ext().execute_with(|| {
        const VALIDATOR_ID: u32 = 88;
        const QUEUED_BALANCE: u128 = 1_000;
        const POOL_SHARES: u128 = 1_000_000_001;
        const POOL_BALANCE: u128 = 100;

        manual_insert_validator(VALIDATOR_ID, 950, 951);
        let staker = account(952);
        AccountValidatorDelegateStakeShares::<Test>::insert(&staker, VALIDATOR_ID, u128::MAX);
        ValidatorDelegateStakeShares::<Test>::insert(VALIDATOR_ID, POOL_SHARES);
        ValidatorDelegateStakeBalance::<Test>::insert(VALIDATOR_ID, POOL_BALANCE);
        ValidatorDelegateStakeCirculatingShares::<Test>::insert(VALIDATOR_ID, 0);
        TotalValidatorDelegateStakeBalance::<Test>::set(POOL_BALANCE);
        assert!(Network::convert_to_shares(QUEUED_BALANCE, POOL_SHARES, POOL_BALANCE) > 0);

        let queue_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::queue_swap(
            staker.clone(),
            QueuedSwapSource::ValidatorDelegate,
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id: staker.clone(),
                to_validator_id: VALIDATOR_ID,
                balance: QUEUED_BALANCE,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            },
        ));
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), QUEUED_BALANCE);
        assert_queued_swap_principal_invariant();

        let execution_block = System::block_number()
            .saturating_add(EpochLength::get())
            .saturating_add(1);
        System::set_block_number(execution_block);
        Network::execute_ready_swap_calls(execution_block, &mut WeightMeter::new());

        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&staker, VALIDATOR_ID),
            u128::MAX
        );
        assert_eq!(
            ValidatorDelegateStakeShares::<Test>::get(VALIDATOR_ID),
            POOL_SHARES
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(VALIDATOR_ID),
            POOL_BALANCE
        );
        assert_eq!(
            TotalValidatorDelegateStakeBalance::<Test>::get(),
            POOL_BALANCE
        );

        assert!(StakeUnbondingLedger::<Test>::get(&staker).is_empty());
        assert_eq!(
            QueuedSwapRefundBalance::<Test>::get(&staker),
            QUEUED_BALANCE
        );
        assert_eq!(TotalQueuedSwapRefundBalance::<Test>::get(), QUEUED_BALANCE);
        assert!(SwapCallQueue::<Test>::get(queue_id).is_none());
        assert!(SwapQueueOrder::<Test>::get().is_empty());
        assert_eq!(SwapQueueCount::<Test>::get(), 0);
        assert_eq!(TotalQueuedSwapPrincipal::<Test>::get(), 0);
        assert_queued_swap_principal_invariant();
        assert!(network_events().iter().any(|event| {
            matches!(
                event,
                Event::SwapCallRefunded {
                    id,
                    account_id,
                    balance,
                    reason: SwapRefundReason::DestinationCreditOverflow,
                } if *id == queue_id && account_id == &staker && *balance == QUEUED_BALANCE
            )
        }));
    });
}
