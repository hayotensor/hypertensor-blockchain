use super::mock::*;
use crate::tests::test_utils::*;
use crate::{
    AccountSubnetDelegateStakeGeneration, AccountSubnetDelegateStakeShares,
    DelegateStakeCooldownEpochs, Error, Event, MaxUnbondings, MinDelegateStakeDeposit,
    MinSubnetMinStake, NextSwapQueueId, QueuedSwapCall, StakeUnbondingLedger,
    SubnetDelegatePoolGeneration, SubnetName, SubnetRemovalReason, SubnetsData, SwapCallQueue,
    SwapQueueOrder, TotalDelegateStake, TotalSubnetDelegateStakeBalance,
    TotalSubnetDelegateStakeCirculatingShares, TotalSubnetDelegateStakeShares, TotalSubnetNodes,
    TxRateLimit,
};
use frame_support::traits::Currency;
use frame_support::{assert_err, assert_ok};

//
//
//
//
//
//
//
// Delegate staking
//
//
//
//
//
//
//

#[test]
fn test_subnet_pool_final_asset_redemption_starts_a_fresh_generation() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let subnet_id = 99;
        let old_staker = account(950);
        let new_staker = account(951);
        let deposit = 1_000u128;

        Network::handle_increase_account_delegate_stake(&old_staker, subnet_id, deposit)
            .expect("initial subnet pool position must be accepted");
        let old_raw_shares = AccountSubnetDelegateStakeShares::<Test>::get(&old_staker, subnet_id);

        // Model a severe external pool loss that leaves one atomic asset. Redeeming that final
        // unit returns the pool to an empty generation.
        TotalSubnetDelegateStakeBalance::<Test>::insert(subnet_id, 1);
        TotalDelegateStake::<Test>::put(1);
        let total_shares = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let (assets_out, supply_shares_burned) =
            Network::preview_delegate_pool_redemption(old_raw_shares, total_shares, 1, 1).unwrap();
        assert_eq!(assets_out, 1);

        let (result, balance_removed, shares_removed) =
            Network::perform_do_remove_subnet_delegate_stake(
                &old_staker,
                subnet_id,
                old_raw_shares,
                assets_out,
                false,
            );
        assert_ok!(result);
        assert_eq!(balance_removed, assets_out);
        assert_eq!(shares_removed, old_raw_shares);
        assert!(supply_shares_burned <= shares_removed);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id), 0);
        assert_eq!(TotalSubnetDelegateStakeShares::<Test>::get(subnet_id), 0);
        assert_eq!(
            TotalSubnetDelegateStakeCirculatingShares::<Test>::get(subnet_id),
            0
        );
        assert_eq!(SubnetDelegatePoolGeneration::<Test>::get(subnet_id), 1);
        assert_eq!(
            Network::current_account_subnet_delegate_stake_shares(&old_staker, subnet_id),
            0
        );

        Network::handle_increase_account_delegate_stake(&new_staker, subnet_id, deposit)
            .expect("a fresh subnet pool generation must accept a normal deposit");
        assert!(Network::current_account_subnet_delegate_stake_shares(&new_staker, subnet_id) > 0);
        assert_eq!(
            Network::current_account_subnet_delegate_stake_shares(&old_staker, subnet_id),
            0
        );
        assert!(network_events().iter().any(|event| {
            matches!(
                event,
                Event::SubnetDelegatePoolReset {
                    subnet_id: event_subnet_id,
                    old_generation: 0,
                    new_generation: 1,
                    invalidated_shares: 0,
                } if *event_subnet_id == subnet_id
            )
        }));
    });
}

#[test]
fn test_subnet_pool_generation_overflow_keeps_final_asset_accounting_unchanged() {
    new_test_ext().execute_with(|| {
        let subnet_id = 100;
        let staker = account(952);
        let deposit = 1_000u128;

        Network::handle_increase_account_delegate_stake(&staker, subnet_id, deposit)
            .expect("initial subnet pool position must be accepted");
        let account_shares = AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id);
        TotalSubnetDelegateStakeBalance::<Test>::insert(subnet_id, 1);
        TotalDelegateStake::<Test>::put(1);
        SubnetDelegatePoolGeneration::<Test>::insert(subnet_id, u64::MAX);
        AccountSubnetDelegateStakeGeneration::<Test>::insert(&staker, subnet_id, u64::MAX);

        let total_shares = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let circulating_shares = TotalSubnetDelegateStakeCirculatingShares::<Test>::get(subnet_id);
        let (assets_out, supply_burn) =
            Network::preview_delegate_pool_redemption(account_shares, total_shares, 1, 1).unwrap();

        let (result, balance_removed, shares_removed) =
            Network::perform_do_remove_subnet_delegate_stake(
                &staker,
                subnet_id,
                account_shares,
                assets_out,
                false,
            );
        assert_err!(result, sp_runtime::ArithmeticError::Overflow);
        assert_eq!(balance_removed, 0);
        assert_eq!(shares_removed, 0);
        assert!(supply_burn <= account_shares);
        assert_eq!(TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id), 1);
        assert_eq!(
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id),
            total_shares
        );
        assert_eq!(
            TotalSubnetDelegateStakeCirculatingShares::<Test>::get(subnet_id),
            circulating_shares
        );
        assert_eq!(TotalDelegateStake::<Test>::get(), 1);
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id),
            account_shares
        );
        assert_eq!(
            SubnetDelegatePoolGeneration::<Test>::get(subnet_id),
            u64::MAX
        );
    });
}

#[test]
fn test_add_to_delegate_stake() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000e+18 as u128;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let n_account = total_subnet_nodes + 1;

        let _ = Balances::deposit_creating(&account(n_account), amount + 500);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let prev_total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let prev_total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        let (delegate_stake_to_be_added_as_shares, gross_shares) =
            Network::preview_delegate_pool_deposit(
                amount,
                prev_total_subnet_delegate_stake_shares,
                prev_total_subnet_delegate_stake_balance,
                1,
            )
            .unwrap();

        let starting_delegator_balance = Balances::free_balance(&account(n_account));

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            subnet_id,
            amount,
            1,
        ));

        // Wallet
        let post_delegator_balance = Balances::free_balance(&account(n_account));
        assert_eq!(post_delegator_balance, starting_delegator_balance - amount);

        // Expected shares
        let delegate_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(account(n_account), subnet_id);
        assert_eq!(delegate_shares, delegate_stake_to_be_added_as_shares);
        assert_ne!(delegate_shares, 0);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        // Expected balance in subnet
        assert_eq!(
            amount + prev_total_subnet_delegate_stake_balance,
            total_subnet_delegate_stake_balance
        );

        // Expected shares
        assert_eq!(
            gross_shares + prev_total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_shares
        );

        let delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );
        // Virtual assets and locked shares may cost the first depositor one or more atomic units,
        // but never credit more assets than were deposited.
        assert!(
            (delegate_balance >= Network::percent_mul(amount, test_percent(99, 100)))
                && (delegate_balance <= amount)
        );
    });
}

#[test]
fn test_add_subnet_delegate_stake_respects_tx_rate_limit() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "delegate-rate-limit".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000e+18 as u128;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let delegator = account(900);
        let rate_limit = 3;
        TxRateLimit::<Test>::put(rate_limit);
        System::set_block_number(1);
        let _ = Balances::deposit_creating(&delegator, amount.saturating_mul(3) + 500);

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(delegator.clone()),
            subnet_id,
            amount,
            1,
        ));
        let shares_after_first =
            AccountSubnetDelegateStakeShares::<Test>::get(delegator.clone(), subnet_id);
        let balance_after_first = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        assert_err!(
            Network::add_subnet_delegate_stake(
                RuntimeOrigin::signed(delegator.clone()),
                subnet_id,
                amount,
                1,
            ),
            Error::<Test>::TxRateLimitExceeded
        );
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(delegator.clone(), subnet_id),
            shares_after_first
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            balance_after_first
        );

        System::set_block_number(System::block_number() + rate_limit + 1);
        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(delegator.clone()),
            subnet_id,
            amount,
            1,
        ));
        assert!(
            AccountSubnetDelegateStakeShares::<Test>::get(delegator, subnet_id)
                > shares_after_first
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            balance_after_first + amount
        );
    });
}

#[test]
fn test_subnet_delegate_stake_minimum_outputs_are_atomic() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-minimum-output".into();
        let amount = 1_000_000_000_000_000_000_000_u128;
        build_activated_subnet(
            subnet_name.clone(),
            0,
            0,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );

        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let delegator = account(903);
        let _ = Balances::deposit_creating(&delegator, amount + 500);
        let wallet_before = Balances::free_balance(&delegator);
        let initial_pool_balance = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let initial_pool_shares = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let (expected_shares, expected_gross_shares) = Network::preview_delegate_pool_deposit(
            amount,
            initial_pool_shares,
            initial_pool_balance,
            1,
        )
        .unwrap();

        assert_err!(
            Network::add_subnet_delegate_stake(
                RuntimeOrigin::signed(delegator.clone()),
                subnet_id,
                amount,
                0,
            ),
            Error::<Test>::InvalidStakeMinimumOutput
        );
        assert_err!(
            Network::add_subnet_delegate_stake(
                RuntimeOrigin::signed(delegator.clone()),
                subnet_id,
                amount,
                expected_shares + 1,
            ),
            Error::<Test>::StakeSlippageExceeded
        );
        assert_eq!(Balances::free_balance(&delegator), wallet_before);
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&delegator, subnet_id),
            0
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            initial_pool_balance
        );
        assert_eq!(
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id),
            initial_pool_shares
        );

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(delegator.clone()),
            subnet_id,
            amount,
            expected_shares,
        ));
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&delegator, subnet_id),
            expected_shares
        );
        assert_eq!(
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id),
            initial_pool_shares + expected_gross_shares
        );

        let post_deposit_pool_shares = initial_pool_shares + expected_gross_shares;
        let post_deposit_pool_balance = initial_pool_balance + amount;
        let expected_balance = Network::try_convert_to_balance(
            expected_shares,
            post_deposit_pool_shares,
            post_deposit_pool_balance,
        )
        .unwrap();
        System::set_block_number(System::block_number() + TxRateLimit::<Test>::get() + 1);

        assert_err!(
            Network::remove_delegate_stake(
                RuntimeOrigin::signed(delegator.clone()),
                subnet_id,
                expected_shares,
                expected_balance + 1,
            ),
            Error::<Test>::StakeSlippageExceeded
        );
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&delegator, subnet_id),
            expected_shares
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            post_deposit_pool_balance
        );
        assert!(StakeUnbondingLedger::<Test>::get(&delegator).is_empty());

        assert_ok!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(delegator.clone()),
            subnet_id,
            expected_shares,
            expected_balance,
        ));
        assert_eq!(
            StakeUnbondingLedger::<Test>::get(delegator)
                .values()
                .next()
                .unwrap()
                .network,
            expected_balance
        );
    });
}

#[test]
fn test_add_to_delegate_stake_not_enough_balance_error() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000e+18 as u128;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let account_n = 5;

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let starting_delegator_balance = Balances::free_balance(&account(account_n));

        assert_err!(
            Network::add_subnet_delegate_stake(
                RuntimeOrigin::signed(account(account_n)),
                subnet_id,
                amount,
                1,
            ),
            Error::<Test>::NotEnoughBalanceToStake
        );

        let delegator_balance = Balances::free_balance(&account(account_n));
        assert_eq!(starting_delegator_balance, delegator_balance);
    });
}

#[test]
fn test_add_to_delegate_stake_balance_withdraw_error() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000e+18 as u128;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let account_n = 5;

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let _ = Balances::deposit_creating(&account(account_n), amount + 500);

        let starting_delegator_balance = Balances::free_balance(&account(account_n));

        assert_err!(
            Network::add_subnet_delegate_stake(
                RuntimeOrigin::signed(account(account_n)),
                subnet_id,
                amount + 100,
                1,
            ),
            Error::<Test>::BalanceWithdrawalError
        );

        let delegator_balance = Balances::free_balance(&account(account_n));
        assert_eq!(starting_delegator_balance, delegator_balance);
    });
}

#[test]
fn test_add_to_delegate_stake_min_delegate_stake_deposit_not_reached_error() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000e+18 as u128;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let account_n = 5;

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let _ = Balances::deposit_creating(&account(account_n), amount + 500);

        let starting_delegator_balance = Balances::free_balance(&account(account_n));

        assert_err!(
            Network::add_subnet_delegate_stake(
                RuntimeOrigin::signed(account(account_n)),
                subnet_id,
                MinDelegateStakeDeposit::<Test>::get() - 1,
                1,
            ),
            Error::<Test>::MinDelegateStakeDepositNotReached
        );

        assert_err!(
            Network::add_subnet_delegate_stake(
                RuntimeOrigin::signed(account(account_n)),
                subnet_id,
                0,
                1,
            ),
            Error::<Test>::MinDelegateStakeDepositNotReached
        );

        let delegator_balance = Balances::free_balance(&account(account_n));
        assert_eq!(starting_delegator_balance, delegator_balance);
    });
}

#[test]
fn test_delegate_math() {
    new_test_ext().execute_with(|| {
        let delegate_stake_to_be_added = 1000e+18 as u128;
        let (user_shares, gross_shares) =
            Network::preview_delegate_pool_deposit(delegate_stake_to_be_added, 0, 0, 1).unwrap();

        assert_eq!(
            gross_shares,
            Network::try_convert_to_shares(delegate_stake_to_be_added, 0, 0).unwrap()
        );
        assert_eq!(
            gross_shares - user_shares,
            Network::DELEGATE_POOL_MIN_LIQUIDITY
        );
        assert_eq!(
            Network::try_convert_to_balance(user_shares, gross_shares, delegate_stake_to_be_added,)
                .unwrap(),
            delegate_stake_to_be_added - 1
        );
    });
}

#[test]
fn check_balances() {
    new_test_ext().execute_with(|| {
        let _ = env_logger::builder().is_test(true).try_init();

        let subnet_id = 1;
        let user = account(1);

        // Initial user tokens
        // const USER_INITIAL_TOKENS: u128 = 1000000000000000000; // 1
        // const USER_INITIAL_TOKENS: u128 = 10000000000000000000; // 10
        // const USER_INITIAL_TOKENS: u128 = 100000000000000000000; // 100

        // const USER_INITIAL_BALANCE: u128 = USER_INITIAL_TOKENS + 500;

        // Balances::make_free_balance_be(&user, USER_INITIAL_BALANCE);

        // // ---- Step 1: uSER deposits minimal amount ----
        // // The MinDelegateStakeDeposit (deposit min) is 1000, otherwise reverts with CouldNotConvertToBalance
        // assert_ok!(
        //   Network::do_add_subnet_delegate_stake(
        //     RuntimeOrigin::signed(user.clone()),
        //     subnet_id,
        //     USER_INITIAL_TOKENS,
        //   )
        // );

        // let total_subnet_delegate_stake_shares = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        // let total_subnet_delegate_stake_balance = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        // // Validate initial deposit
        // let user_balance = Network::convert_to_balance(
        //   AccountSubnetDelegateStakeShares::<Test>::get(&user, subnet_id),
        //   total_subnet_delegate_stake_shares,
        //   total_subnet_delegate_stake_balance
        // );
        // // assert!(false);

        // let loss = 1.0 - user_balance as f64 / USER_INITIAL_TOKENS as f64;

        for n in 3..28 {
            // reset everything
            let _ = AccountSubnetDelegateStakeShares::<Test>::remove(user.clone(), subnet_id);
            let _ = AccountSubnetDelegateStakeGeneration::<Test>::remove(user.clone(), subnet_id);
            let _ = TotalSubnetDelegateStakeShares::<Test>::remove(subnet_id);
            let _ = TotalSubnetDelegateStakeBalance::<Test>::remove(subnet_id);
            let _ = TotalSubnetDelegateStakeCirculatingShares::<Test>::remove(subnet_id);
            let _ = SubnetDelegatePoolGeneration::<Test>::remove(subnet_id);
            TotalDelegateStake::<Test>::kill();

            let USER_INITIAL_TOKENS: u128 = 10_u128.pow(n);
            let USER_INITIAL_BALANCE: u128 = USER_INITIAL_TOKENS + 500;
            Balances::make_free_balance_be(&user, USER_INITIAL_BALANCE);

            assert_ok!(Network::do_add_subnet_delegate_stake(
                RuntimeOrigin::signed(user.clone()),
                subnet_id,
                USER_INITIAL_TOKENS,
                1,
            ));

            let total_subnet_delegate_stake_shares =
                TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
            let total_subnet_delegate_stake_balance =
                TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

            // Validate initial deposit
            let user_balance = Network::convert_to_balance(
                AccountSubnetDelegateStakeShares::<Test>::get(&user, subnet_id),
                total_subnet_delegate_stake_shares,
                total_subnet_delegate_stake_balance,
            );
            log::error!("USER_INITIAL_TOKENS  {:?}", USER_INITIAL_TOKENS);
            log::error!("user_balance         {:?}", user_balance);
            let loss = 1.0 - user_balance as f64 / USER_INITIAL_TOKENS as f64;
            log::error!(
                "Initial Deposit   {}",
                USER_INITIAL_TOKENS as f64 / 1e+18 as f64
            );
            log::error!("Resulting Balance {}", user_balance as f64 / 1e+18 as f64);
            log::error!("Loss              {}", loss);

            log::error!(" ");
        }
        // assert!(false);
    });
}

#[test]
fn test_delegate_math_with_storage_deposit() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 1000000000000000000000000;
        let amount: u128 = 1000000000000000000000; // 1000
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let _ = Balances::deposit_creating(&account(total_subnet_nodes + 1), amount + 500);
        let starting_delegator_balance = Balances::free_balance(&account(total_subnet_nodes + 1));

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
            subnet_id,
            amount,
            1,
        ));

        // ensure removes wallet balance
        let post_delegator_balance = Balances::free_balance(&account(total_subnet_nodes + 1));
        assert_eq!(post_delegator_balance, starting_delegator_balance - amount);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let delegate_shares = AccountSubnetDelegateStakeShares::<Test>::get(
            account(total_subnet_nodes + 1),
            subnet_id,
        );
        let delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        // Ensure balance is within 1% of the deposit and is never over-credited.
        assert!(
            (delegate_balance >= Network::percent_mul(amount, test_percent(99, 100)))
                && (delegate_balance <= amount)
        );

        let pre_balance = Balances::free_balance(&account(total_subnet_nodes + 1));

        let delegate_shares = AccountSubnetDelegateStakeShares::<Test>::get(
            account(total_subnet_nodes + 1),
            subnet_id,
        );
        let shares_to_remove = delegate_shares / 2;
        let expected_ledger_balance = Network::convert_to_balance(
            shares_to_remove,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        let epoch = System::block_number() / EpochLength::get();
        let block = System::block_number();

        assert_ok!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
            subnet_id,
            shares_to_remove,
            1,
        ));

        let post_balance = Balances::free_balance(&account(total_subnet_nodes + 1));
        assert_eq!(pre_balance, post_balance);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let delegate_shares = AccountSubnetDelegateStakeShares::<Test>::get(
            account(total_subnet_nodes + 1),
            subnet_id,
        );
        let delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        let unbondings = StakeUnbondingLedger::<Test>::get(account(total_subnet_nodes + 1));
        assert_eq!(unbondings.len(), 1);
        let (ledger_block, ledger_balance) = unbondings.iter().next().unwrap();
        assert_eq!(
            *ledger_block,
            &block + DelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );
        assert_eq!(ledger_balance.network, expected_ledger_balance);
        assert_eq!(ledger_balance.overwatch, 0);
    });
}

#[test]
fn test_remove_delegate_stake() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 1000000000000000000000000;
        let amount: u128 = 1000000000000000000000; // 1000
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let _ = Balances::deposit_creating(&account(total_subnet_nodes + 1), amount + 500);
        let starting_delegator_balance = Balances::free_balance(&account(total_subnet_nodes + 1));

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
            subnet_id,
            amount,
            1,
        ));

        // ensure removes wallet balance
        let post_delegator_balance = Balances::free_balance(&account(total_subnet_nodes + 1));
        assert_eq!(post_delegator_balance, starting_delegator_balance - amount);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let delegate_shares = AccountSubnetDelegateStakeShares::<Test>::get(
            account(total_subnet_nodes + 1),
            subnet_id,
        );
        let delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        // Ensure balance is within 1% of the deposit and is never over-credited.
        assert!(
            (delegate_balance >= Network::percent_mul(amount, test_percent(99, 100)))
                && (delegate_balance <= amount)
        );

        let pre_balance = Balances::free_balance(&account(total_subnet_nodes + 1));

        let delegate_shares = AccountSubnetDelegateStakeShares::<Test>::get(
            account(total_subnet_nodes + 1),
            subnet_id,
        );
        let shares_to_remove = delegate_shares / 2;
        let expected_ledger_balance = Network::convert_to_balance(
            shares_to_remove,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        let epoch = System::block_number() / EpochLength::get();
        let block = System::block_number();

        assert_ok!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
            subnet_id,
            shares_to_remove,
            1,
        ));

        // Shouldn't withdraw to wallet
        let post_balance = Balances::free_balance(&account(total_subnet_nodes + 1));
        assert_eq!(pre_balance, post_balance);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let delegate_shares = AccountSubnetDelegateStakeShares::<Test>::get(
            account(total_subnet_nodes + 1),
            subnet_id,
        );
        let delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        // Should be sent to unbondings
        let unbondings = StakeUnbondingLedger::<Test>::get(account(total_subnet_nodes + 1));
        assert_eq!(unbondings.len(), 1);
        let (ledger_block, ledger_balance) = unbondings.iter().next().unwrap();
        assert_eq!(
            *ledger_block,
            &block + DelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );
        assert_eq!(ledger_balance.network, expected_ledger_balance);
        assert_eq!(ledger_balance.overwatch, 0);
    });
}

#[test]
fn test_remove_delegate_stake_not_enough_stake_to_withdraw() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 1000000000000000000000000;
        let amount: u128 = 1000000000000000000000; // 1000
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let subnet_name_2: Vec<u8> = "subnet-name-2".into();
        build_activated_subnet(subnet_name_2.clone(), 0, 0, deposit_amount, stake_amount);
        let subnet_id_2 = SubnetName::<Test>::get(subnet_name_2.clone()).unwrap();

        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let _ = Balances::deposit_creating(&account(total_subnet_nodes + 1), amount + 500);
        let starting_delegator_balance = Balances::free_balance(&account(total_subnet_nodes + 1));

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
            subnet_id,
            amount,
            1,
        ));

        // ensure removes wallet balance
        let post_delegator_balance = Balances::free_balance(&account(total_subnet_nodes + 1));
        assert_eq!(post_delegator_balance, starting_delegator_balance - amount);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let delegate_shares = AccountSubnetDelegateStakeShares::<Test>::get(
            account(total_subnet_nodes + 1),
            subnet_id,
        );
        let delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        // Ensure balance is within 1% of the deposit and is never over-credited.
        assert!(
            (delegate_balance >= Network::percent_mul(amount, test_percent(99, 100)))
                && (delegate_balance <= amount)
        );

        let pre_balance = Balances::free_balance(&account(total_subnet_nodes + 1));

        let delegate_shares = AccountSubnetDelegateStakeShares::<Test>::get(
            account(total_subnet_nodes + 1),
            subnet_id,
        );
        assert!(delegate_shares > 0);

        assert_err!(
            Network::remove_delegate_stake(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                subnet_id,
                0,
                1,
            ),
            Error::<Test>::SharesZero
        );

        assert_err!(
            Network::swap_from_subnet_to_subnet(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                subnet_id,
                subnet_id_2,
                0,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::SharesZero
        );

        assert_err!(
            Network::swap_from_subnet_to_validator(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                subnet_id,
                1,
                0,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::SharesZero
        );

        assert_err!(
            Network::remove_delegate_stake(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                subnet_id,
                delegate_shares + 1,
                1,
            ),
            Error::<Test>::NotEnoughStakeToWithdraw
        );

        assert_err!(
            Network::swap_from_subnet_to_subnet(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                subnet_id,
                subnet_id_2,
                delegate_shares + 1,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::NotEnoughStakeToWithdraw
        );

        assert_err!(
            Network::swap_from_subnet_to_validator(
                RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
                subnet_id,
                1,
                delegate_shares + 1,
                1,
                1,
                u32::MAX,
            ),
            Error::<Test>::NotEnoughStakeToWithdraw
        );
    });
}

#[test]
fn test_remove_claim_delegate_stake_after_remove_subnet() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 1000000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let _ = Balances::deposit_creating(&account(total_subnet_nodes + 1), amount + 500);
        let starting_delegator_balance = Balances::free_balance(&account(total_subnet_nodes + 1));

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
            subnet_id,
            amount,
            1,
        ));

        let post_delegator_balance = Balances::free_balance(&account(total_subnet_nodes + 1));
        assert_eq!(post_delegator_balance, starting_delegator_balance - amount);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let delegate_shares = AccountSubnetDelegateStakeShares::<Test>::get(
            account(total_subnet_nodes + 1),
            subnet_id,
        );
        let delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );
        let expected_ledger_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );
        // assert_eq!(amount, delegate_balance);
        assert!(
            (delegate_balance >= Network::percent_mul(amount, test_percent(99, 100)))
                && (delegate_balance <= amount)
        );

        Network::do_remove_subnet(subnet_id, SubnetRemovalReason::MinSubnetDelegateStake);

        assert_eq!(SubnetsData::<Test>::contains_key(subnet_id), false);

        let epoch = System::block_number() / EpochLength::get();
        let block = System::block_number();

        assert_ok!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(account(total_subnet_nodes + 1)),
            subnet_id,
            delegate_shares,
            1,
        ));

        let unbondings = StakeUnbondingLedger::<Test>::get(account(total_subnet_nodes + 1));
        assert_eq!(unbondings.len(), 1);
        // let (ledger_epoch, ledger_balance) = unbondings.iter().next().unwrap();
        // assert_eq!(*ledger_epoch, &epoch + DelegateStakeCooldownEpochs::<Test>::get());
        // assert_eq!(*ledger_balance, expected_ledger_balance);
        let (ledger_block, ledger_balance) = unbondings.iter().next().unwrap();
        assert_eq!(
            *ledger_block,
            &block + DelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );
        assert_eq!(ledger_balance.network, expected_ledger_balance);
        assert_eq!(ledger_balance.overwatch, 0);

        System::set_block_number(
            System::block_number()
                + ((EpochLength::get() + 1) * DelegateStakeCooldownEpochs::<Test>::get()),
        );

        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(account(
            total_subnet_nodes + 1
        ))));

        let post_balance = Balances::free_balance(&account(total_subnet_nodes + 1));

        assert!(
            (post_balance
                >= Network::percent_mul(starting_delegator_balance, test_percent(99, 100)))
                && (post_balance <= starting_delegator_balance)
        );

        let unbondings = StakeUnbondingLedger::<Test>::get(account(total_subnet_nodes + 1));
        assert_eq!(unbondings.len(), 0);
    });
}

#[test]
fn test_add_to_delegate_stake_increase_pool_check_balance() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let n_account = total_subnet_nodes + 1;

        let _ = Balances::deposit_creating(&account(n_account), amount + 500);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        let delegate_stake_to_be_added_as_shares = Network::preview_delegate_pool_deposit(
            amount,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
            1,
        )
        .unwrap()
        .0;

        System::set_block_number(
            System::block_number()
                + DelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get(),
        );

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            subnet_id,
            amount,
            1,
        ));

        let delegate_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(account(n_account), subnet_id);
        assert_eq!(delegate_shares, delegate_stake_to_be_added_as_shares);
        assert_ne!(delegate_shares, 0);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        log::error!(
            "total_subnet_delegate_stake_shares  {:?}",
            total_subnet_delegate_stake_shares
        );
        log::error!(
            "total_subnet_delegate_stake_balance {:?}",
            total_subnet_delegate_stake_balance
        );

        let delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );
        log::error!(
            "delegate_balance                     {:?}",
            delegate_balance
        );

        // The first depositor will lose a percentage of their deposit depending on the size
        // https://docs.openzeppelin.com/contracts/4.x/erc4626#inflation-attack
        // assert_eq!(delegate_balance, delegate_stake_to_be_added_as_shares);
        assert!(
            (delegate_balance >= Network::percent_mul(amount, test_percent(99, 100)))
                && (delegate_balance <= amount)
        );

        let increase_delegate_stake_amount: u128 = 1000000000000000000000;

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let expected_post_delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance + increase_delegate_stake_amount,
        );

        Network::do_increase_delegate_stake(subnet_id, increase_delegate_stake_amount);

        // ensure balance has increase
        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        let post_delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );
        log::error!("post_delegate_balance      {:?}", post_delegate_balance);
        assert_eq!(post_delegate_balance, expected_post_delegate_balance);
    });
}

#[test]
fn test_claim_removal_of_delegate_stake() {
    new_test_ext().execute_with(|| {
        let _ = env_logger::builder().is_test(true).try_init();

        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let n_account = total_subnet_nodes + 1;

        let _ = Balances::deposit_creating(&account(n_account), amount + 500);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        let delegate_stake_to_be_added_as_shares = Network::preview_delegate_pool_deposit(
            amount,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
            1,
        )
        .unwrap()
        .0;

        let starting_delegator_balance = Balances::free_balance(&account(n_account));

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            subnet_id,
            amount,
            1,
        ));

        let delegate_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(account(n_account), subnet_id);
        assert_eq!(delegate_shares, delegate_stake_to_be_added_as_shares);
        assert_ne!(delegate_shares, 0);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        let mut delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );
        // The first depositor will lose a percentage of their deposit depending on the size
        // https://docs.openzeppelin.com/contracts/4.x/erc4626#inflation-attack
        // assert_eq!(delegate_balance, delegate_stake_to_be_added_as_shares);
        assert!(
            (delegate_balance >= Network::percent_mul(amount, test_percent(99, 100)))
                && (delegate_balance <= amount)
        );

        let epoch_length = EpochLength::get();
        let cooldown_epochs = DelegateStakeCooldownEpochs::<Test>::get();

        System::set_block_number(System::block_number() + epoch_length * cooldown_epochs);

        let balance = Balances::free_balance(&account(n_account));
        let epoch = System::block_number() / epoch_length;
        let block = System::block_number();

        assert_ok!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            subnet_id,
            delegate_shares,
            1,
        ));
        let post_balance = Balances::free_balance(&account(n_account));
        assert_eq!(post_balance, balance);

        let unbondings = StakeUnbondingLedger::<Test>::get(account(n_account));
        assert_eq!(unbondings.len(), 1);
        let (ledger_block, ledger_balance) = unbondings.iter().next().unwrap();
        assert_eq!(
            *ledger_block,
            &block + DelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );
        assert!(ledger_balance.network <= delegate_balance);
        assert_eq!(ledger_balance.overwatch, 0);

        assert_err!(
            Network::claim_unbondings(RuntimeOrigin::signed(account(n_account))),
            Error::<Test>::NoStakeUnbondingsOrCooldownNotMet
        );

        System::set_block_number(System::block_number() + ((epoch_length + 1) * cooldown_epochs));

        let pre_claim_balance = Balances::free_balance(&account(n_account));

        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(account(
            n_account
        ))));

        let after_claim_balance = Balances::free_balance(&account(n_account));

        assert_eq!(
            after_claim_balance,
            pre_claim_balance + ledger_balance.network
        );

        log::error!(
            "starting_delegator_balance {:?}",
            starting_delegator_balance
        );
        log::error!("after_claim_balance        {:?}", after_claim_balance);
        log::error!("post_balance               {:?}", post_balance);
        log::error!("ledger_balance             {:?}", ledger_balance);

        let unbondings = StakeUnbondingLedger::<Test>::get(account(n_account));
        assert_eq!(unbondings.len(), 0);
    });
}

#[test]
fn test_remove_to_delegate_stake_max_unlockings_reached_err() {
    new_test_ext().execute_with(|| {
        if DelegateStakeCooldownEpochs::<Test>::get() <= 1 {
            return;
        }

        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let n_account = total_subnet_nodes + 1;

        let _ = Balances::deposit_creating(&account(n_account), amount + 500);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        let delegate_stake_to_be_added_as_shares = Network::preview_delegate_pool_deposit(
            amount,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
            1,
        )
        .unwrap()
        .0;

        System::set_block_number(
            System::block_number()
                + DelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get(),
        );

        let starting_delegator_balance = Balances::free_balance(&account(n_account));

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            subnet_id,
            amount,
            1,
        ));

        let max_unlockings = MaxUnbondings::<Test>::get();
        for n in 1..max_unlockings + 2 {
            // increase_epochs(1);
            System::set_block_number(System::block_number() + 1);
            // System::set_block_number(System::block_number() + EpochLength::get());
            if n > max_unlockings {
                assert_err!(
                    Network::remove_delegate_stake(
                        RuntimeOrigin::signed(account(n_account)),
                        subnet_id,
                        1000,
                        1,
                    ),
                    Error::<Test>::MaxUnlockingsReached
                );
            } else {
                assert_ok!(Network::remove_delegate_stake(
                    RuntimeOrigin::signed(account(n_account)),
                    subnet_id,
                    1000,
                    1,
                ));
                let unbondings = StakeUnbondingLedger::<Test>::get(account(n_account));
                assert_eq!(unbondings.len() as u32, n);
            }
        }
    });
}

#[test]
fn test_swap_delegate_stake() {
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

        let delegate_stake_to_be_added_as_shares = Network::preview_delegate_pool_deposit(
            amount,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
            1,
        )
        .unwrap()
        .0;

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

        // assert_eq!(
        //     *network_events().last().unwrap(),
        //     Event::SwapCallQueued {
        //         id: prev_next_id,
        //         account_id: account(n_account),
        //         call: call.clone()
        //     }
        // );

        // let to_delegate_shares = AccountSubnetDelegateStakeShares::<Test>::get(account(n_account), to_subnet_id);
        // assert_ne!(to_delegate_shares, 0);

        // let total_subnet_delegate_stake_shares = TotalSubnetDelegateStakeShares::<Test>::get(to_subnet_id);
        // let total_subnet_delegate_stake_balance = TotalSubnetDelegateStakeBalance::<Test>::get(to_subnet_id);

        // let mut to_delegate_balance = Network::convert_to_balance(
        //   to_delegate_shares,
        //   total_subnet_delegate_stake_shares,
        //   total_subnet_delegate_stake_balance
        // );
        // // The first depositor will lose a percentage of their deposit depending on the size
        // // https://docs.openzeppelin.com/contracts/4.x/erc4626#inflation-attack
        // // Will lose about .01% of the transfer value on first transfer into a pool
        // // The balance should be about ~99% of the ``from`` subnet to the ``to`` subnet
        // assert!(
        //   (to_delegate_balance >= Network::percent_mul(from_delegate_balance, test_percent(99, 100))) &&
        //   (to_delegate_balance < from_delegate_balance)
        // );

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

        let next_id = NextSwapQueueId::<Test>::get();
        assert_eq!(prev_next_id + 1, next_id);
        let queue = SwapQueueOrder::<Test>::get();
        assert!(queue
            .first()
            .map_or(false, |&first_id| first_id == prev_next_id));
    });
}

#[test]
fn test_switch_delegate_stake_not_enough_stake_err() {
    new_test_ext().execute_with(|| {
        let _ = env_logger::builder().is_test(true).try_init();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let from_subnet_name: Vec<u8> = "subnet-name".into();
        build_activated_subnet(from_subnet_name.clone(), 0, 0, deposit_amount, stake_amount);
        let from_subnet_id = SubnetName::<Test>::get(from_subnet_name.clone()).unwrap();

        let to_subnet_name: Vec<u8> = "subnet-name-2".into();
        build_activated_subnet(to_subnet_name.clone(), 0, 0, deposit_amount, stake_amount);
        let to_subnet_id = SubnetName::<Test>::get(to_subnet_name.clone()).unwrap();

        // let n_account = 255;

        // let _ = Balances::deposit_creating(&account(n_account), amount+500);

        // assert_err!(
        //   Network::swap_from_subnet_to_subnet(
        //     RuntimeOrigin::signed(account(n_account)),
        //     from_subnet_id,
        //     to_subnet_id,
        //     0,
        //   ),
        //   Error::<Test>::NotEnoughStakeToWithdraw
        // );

        // assert_err!(
        //   Network::swap_from_subnet_to_subnet(
        //     RuntimeOrigin::signed(account(n_account)),
        //     from_subnet_id,
        //     to_subnet_id,
        //     1000,
        //   ),
        //   Error::<Test>::NotEnoughStakeToWithdraw
        // );
    });
}

#[test]
fn test_remove_delegate_stake_after_subnet_remove() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let n_account = total_subnet_nodes + 1;

        let _ = Balances::deposit_creating(&account(n_account), amount + 500);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        let delegate_stake_to_be_added_as_shares = Network::preview_delegate_pool_deposit(
            amount,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
            1,
        )
        .unwrap()
        .0;

        System::set_block_number(
            System::block_number()
                + DelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get(),
        );

        let starting_delegator_balance = Balances::free_balance(&account(n_account));

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            subnet_id,
            amount,
            1,
        ));

        let delegate_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(account(n_account), subnet_id);
        assert_eq!(delegate_shares, delegate_stake_to_be_added_as_shares);
        assert_ne!(delegate_shares, 0);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        let mut delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );
        // The first depositor will lose a percentage of their deposit depending on the size
        // https://docs.openzeppelin.com/contracts/4.x/erc4626#inflation-attack
        // assert_eq!(delegate_balance, delegate_stake_to_be_added_as_shares);
        assert!(
            (delegate_balance >= Network::percent_mul(amount, test_percent(99, 100)))
                && (delegate_balance <= amount)
        );

        let epoch_length = EpochLength::get();
        let cooldown_epochs = DelegateStakeCooldownEpochs::<Test>::get();

        Network::do_remove_subnet(subnet_id, SubnetRemovalReason::MinSubnetDelegateStake);

        assert_eq!(SubnetsData::<Test>::contains_key(subnet_id), false);

        // System::set_block_number(System::block_number() + epoch_length * cooldown_epochs);

        let balance = Balances::free_balance(&account(n_account));
        let epoch = System::block_number() / epoch_length;
        let block = System::block_number();

        assert_ok!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            subnet_id,
            delegate_shares,
            1,
        ));
        let post_balance = Balances::free_balance(&account(n_account));
        assert_eq!(post_balance, balance);

        let unbondings = StakeUnbondingLedger::<Test>::get(account(n_account));
        assert_eq!(unbondings.len(), 1);
        let (ledger_block, ledger_balance) = unbondings.iter().next().unwrap();
        assert_eq!(
            *ledger_block,
            &block + DelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );
        assert!(ledger_balance.network <= delegate_balance);
        assert_eq!(ledger_balance.overwatch, 0);

        assert_err!(
            Network::claim_unbondings(RuntimeOrigin::signed(account(n_account))),
            Error::<Test>::NoStakeUnbondingsOrCooldownNotMet
        );

        System::set_block_number(System::block_number() + ((epoch_length + 1) * cooldown_epochs));

        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(account(
            n_account
        ))));

        let post_balance = Balances::free_balance(&account(n_account));

        assert!(
            (post_balance
                >= Network::percent_mul(starting_delegator_balance, test_percent(99, 100)))
                && (post_balance <= starting_delegator_balance)
        );

        let unbondings = StakeUnbondingLedger::<Test>::get(account(n_account));
        assert_eq!(unbondings.len(), 0);
    });
}

#[test]
fn test_swap_from_subnet_to_node() {
    new_test_ext().execute_with(|| {
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let from_subnet_name: Vec<u8> = "subnet-name".into();
        build_activated_subnet(
            from_subnet_name.clone(),
            0,
            16,
            deposit_amount,
            stake_amount,
        );
        let from_subnet_id = SubnetName::<Test>::get(from_subnet_name.clone()).unwrap();
        let to_validator_id = 1;

        let to_subnet_name: Vec<u8> = "subnet-name-2".into();
        build_activated_subnet(to_subnet_name.clone(), 0, 16, deposit_amount, stake_amount);
        let to_subnet_id = SubnetName::<Test>::get(to_subnet_name.clone()).unwrap();
        let to_subnet_node_id = 2;

        let n_account = 255;

        let _ = Balances::deposit_creating(&account(n_account), amount + 500);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(from_subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(from_subnet_id);

        let delegate_stake_to_be_added_as_shares = Network::preview_delegate_pool_deposit(
            amount,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
            1,
        )
        .unwrap()
        .0;

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

        let unbondings = StakeUnbondingLedger::<Test>::get(account(n_account));
        assert_eq!(unbondings.len(), 0);
        let before_transfer_tensor = Balances::free_balance(&account(n_account));

        let prev_next_id = NextSwapQueueId::<Test>::get();

        assert_ok!(Network::swap_from_subnet_to_validator(
            RuntimeOrigin::signed(account(n_account)),
            from_subnet_id,
            to_validator_id,
            delegate_shares,
            1,
            1,
            u32::MAX,
        ));

        let unbondings = StakeUnbondingLedger::<Test>::get(account(n_account));
        assert_eq!(unbondings.len(), 0);
        let after_transfer_tensor = Balances::free_balance(&account(n_account));
        assert_eq!(after_transfer_tensor, before_transfer_tensor);

        let starting_to_subnet_id = to_subnet_id;
        let starting_to_subnet_node_id = to_subnet_node_id;
        let call_queue = SwapCallQueue::<Test>::get(prev_next_id);
        assert_eq!(call_queue.clone().unwrap().id, prev_next_id);
        match &call_queue.clone().unwrap().call {
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id,
                to_subnet_id,
                balance,
                ..
            } => {
                assert!(false)
            }
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id,
                to_validator_id,
                balance,
                ..
            } => {
                assert_eq!(*account_id, account(n_account));
                // assert_eq!(*to_subnet_id, starting_to_subnet_id);
                // assert_eq!(*to_subnet_node_id, starting_to_subnet_node_id);
                assert_ne!(*balance, 0);
            }
        };

        let next_id = NextSwapQueueId::<Test>::get();
        assert_eq!(prev_next_id + 1, next_id);
        let queue = SwapQueueOrder::<Test>::get();
        assert!(queue
            .first()
            .map_or(false, |&first_id| first_id == prev_next_id));
    });
}

#[test]
fn test_inflation_exploit_mitigation_dead_shares() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let first_user = account(1);
        let second_user = account(2);
        let stake = 1_000_000_000_000;

        let _ = Balances::deposit_creating(&first_user, stake * 10);
        let _ = Balances::deposit_creating(&second_user, stake * 10);

        let (expected_first_user_shares, expected_first_gross_shares) =
            Network::preview_delegate_pool_deposit(stake, 0, 0, 1).unwrap();

        assert_ok!(Network::do_add_subnet_delegate_stake(
            RuntimeOrigin::signed(first_user.clone()),
            subnet_id,
            stake,
            1,
        ));

        let first_user_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(&first_user, subnet_id);
        let total_shares_after_first = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);

        assert_eq!(first_user_shares, expected_first_user_shares);
        assert_eq!(total_shares_after_first, expected_first_gross_shares);
        assert_eq!(
            total_shares_after_first - first_user_shares,
            Network::DELEGATE_POOL_MIN_LIQUIDITY
        );

        let balance_after_first = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let (expected_second_user_shares, expected_second_gross_shares) =
            Network::preview_delegate_pool_deposit(
                stake,
                total_shares_after_first,
                balance_after_first,
                1,
            )
            .unwrap();

        assert_ok!(Network::do_add_subnet_delegate_stake(
            RuntimeOrigin::signed(second_user.clone()),
            subnet_id,
            stake,
            1,
        ));

        let second_user_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(&second_user, subnet_id);
        let total_shares_after_both = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_balance_after_both = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        assert_eq!(second_user_shares, expected_second_user_shares);
        assert!(first_user_shares <= second_user_shares);
        assert_eq!(
            total_shares_after_both,
            expected_first_gross_shares + expected_second_gross_shares
        );
        assert_eq!(
            total_shares_after_both,
            first_user_shares + second_user_shares + Network::DELEGATE_POOL_MIN_LIQUIDITY
        );

        let first_user_balance = Network::convert_to_balance(
            first_user_shares,
            total_shares_after_both,
            total_balance_after_both,
        );

        let second_user_balance = Network::convert_to_balance(
            second_user_shares,
            total_shares_after_both,
            total_balance_after_both,
        );

        assert!(first_user_balance <= stake);
        assert!(second_user_balance <= stake);
        assert!(first_user_balance < second_user_balance);
    });
}

#[test]
fn test_internal_reward_credit_increases_share_value_without_minting() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let attacker = account(1);
        let initial_balance = 1_000_000;
        let stake_amount = 100_000;
        let reward_amount = 100_000;

        // Step 0: Fund attacker
        Balances::make_free_balance_be(&attacker, initial_balance);

        // Step 1: Attacker stakes
        assert_ok!(Network::do_add_subnet_delegate_stake(
            RuntimeOrigin::signed(attacker.clone()),
            subnet_id,
            stake_amount,
            1,
        ));

        let shares_before = AccountSubnetDelegateStakeShares::<Test>::get(&attacker, subnet_id);
        let shares_total_before = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let pool_balance_before = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        assert!(shares_before > 0);
        assert!(shares_total_before > 0);
        assert!(pool_balance_before > 0);

        // Protocol rewards increase assets without minting shares.
        assert_ok!(Network::do_increase_delegate_stake(
            subnet_id,
            reward_amount
        ));

        // Step 3: Check that no new shares were minted
        let shares_after_reward =
            AccountSubnetDelegateStakeShares::<Test>::get(&attacker, subnet_id);
        let shares_total_after_reward = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let pool_balance_after = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        assert_eq!(shares_after_reward, shares_before);
        assert_eq!(shares_total_after_reward, shares_total_before);
        assert_eq!(pool_balance_after, pool_balance_before + reward_amount);

        let expected_redeemed_balance = Network::try_convert_to_balance(
            shares_after_reward,
            shares_total_after_reward,
            pool_balance_after,
        )
        .unwrap();
        assert!(expected_redeemed_balance > stake_amount);
        assert!(expected_redeemed_balance <= stake_amount + reward_amount);

        assert_ok!(Network::do_remove_delegate_stake(
            RuntimeOrigin::signed(attacker.clone()),
            subnet_id,
            shares_after_reward,
            1,
        ));

        let unbondings = StakeUnbondingLedger::<Test>::get(attacker);
        assert_eq!(unbondings.len(), 1);
        assert_eq!(
            unbondings.values().next().unwrap().network,
            expected_redeemed_balance
        );
    });
}

#[test]
fn test_virtual_offset_limits_internal_balance_jump_rounding_loss() {
    new_test_ext().execute_with(|| {
        let _ = env_logger::builder().is_test(true).try_init();

        let subnet_id = 1;
        let attacker = account(1);
        let victim = account(2);

        const ATTACKER_INITIAL_TOKENS: u128 = 10000000;
        const ATTACKER_INITIAL_DEPOSIT: u128 = 1000;
        const INTERNAL_REWARD: u128 = 9999000;
        const VICTIM_DEPOSIT: u128 = 1000000;

        Balances::make_free_balance_be(&attacker, ATTACKER_INITIAL_TOKENS);
        Balances::make_free_balance_be(&victim, VICTIM_DEPOSIT + 500);

        let (expected_attacker_shares, expected_gross_shares) =
            Network::preview_delegate_pool_deposit(ATTACKER_INITIAL_DEPOSIT, 0, 0, 1).unwrap();

        assert_ok!(Network::do_add_subnet_delegate_stake(
            RuntimeOrigin::signed(attacker.clone()),
            subnet_id,
            ATTACKER_INITIAL_DEPOSIT,
            1,
        ));

        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&attacker, subnet_id),
            expected_attacker_shares
        );
        assert_eq!(
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id),
            expected_gross_shares
        );
        assert_eq!(
            expected_gross_shares - expected_attacker_shares,
            Network::DELEGATE_POOL_MIN_LIQUIDITY
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            ATTACKER_INITIAL_DEPOSIT
        );

        // Simulate the protocol crediting a large reward between deposits. The public donation
        // call is intentionally absent; this exercises the internal reward path only.
        assert_ok!(Network::do_increase_delegate_stake(
            subnet_id,
            INTERNAL_REWARD
        ));

        let shares_before_victim = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let balance_before_victim = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let (expected_victim_shares, expected_victim_gross_shares) =
            Network::preview_delegate_pool_deposit(
                VICTIM_DEPOSIT,
                shares_before_victim,
                balance_before_victim,
                1,
            )
            .unwrap();

        assert_ok!(Network::do_add_subnet_delegate_stake(
            RuntimeOrigin::signed(victim.clone()),
            subnet_id,
            VICTIM_DEPOSIT,
            1,
        ));

        let victim_shares = AccountSubnetDelegateStakeShares::<Test>::get(&victim, subnet_id);
        assert_eq!(victim_shares, expected_victim_shares);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        assert_eq!(
            total_subnet_delegate_stake_shares,
            shares_before_victim + expected_victim_gross_shares
        );

        let victim_balance = Network::convert_to_balance(
            victim_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        assert!(
            (victim_balance >= Network::percent_mul(VICTIM_DEPOSIT, test_percent(99, 100)))
                && (victim_balance <= VICTIM_DEPOSIT)
        );

        let attacker_balance = Network::convert_to_balance(
            AccountSubnetDelegateStakeShares::<Test>::get(&attacker, subnet_id),
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        assert!(attacker_balance < ATTACKER_INITIAL_DEPOSIT + INTERNAL_REWARD);

        assert_ok!(Network::do_remove_delegate_stake(
            RuntimeOrigin::signed(attacker.clone()),
            subnet_id,
            AccountSubnetDelegateStakeShares::<Test>::get(&attacker, subnet_id),
            1,
        ));

        let attacker_final_balance = Balances::free_balance(&attacker);

        assert!(attacker_final_balance < ATTACKER_INITIAL_TOKENS);
    });
}

#[test]
fn test_transfer_delegate_stake() {
    new_test_ext().execute_with(|| {
        let _ = env_logger::builder().is_test(true).try_init();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnet_name: Vec<u8> = "subnet-name".into();
        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let n_account = 255;
        let to_n_account = 256;

        let _ = Balances::deposit_creating(&account(n_account), amount + 500);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        let delegate_stake_to_be_added_as_shares = Network::preview_delegate_pool_deposit(
            amount,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
            1,
        )
        .unwrap()
        .0;

        System::set_block_number(
            System::block_number()
                + DelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get(),
        );

        let starting_delegator_balance = Balances::free_balance(&account(n_account));

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            subnet_id,
            amount,
            1,
        ));

        let n_account_balance = Balances::free_balance(&account(n_account));
        let to_n_account_balance = Balances::free_balance(&account(to_n_account));

        let delegate_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(account(n_account), subnet_id);
        assert_eq!(delegate_shares, delegate_stake_to_be_added_as_shares);
        assert_ne!(delegate_shares, 0);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        let delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        log::error!(
            "delegate_balance                     {:?}",
            delegate_balance
        );
        log::error!(
            "total_subnet_delegate_stake_shares  {:?}",
            total_subnet_delegate_stake_shares
        );
        log::error!(
            "total_subnet_delegate_stake_balance {:?}",
            total_subnet_delegate_stake_balance
        );

        let to_delegate_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(account(to_n_account), subnet_id);

        assert_eq!(to_delegate_shares, 0);

        assert_ok!(Network::transfer_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            subnet_id,
            account(to_n_account),
            delegate_shares,
        ));

        // no changes to balance
        let after_n_account_balance = Balances::free_balance(&account(n_account));
        assert_eq!(n_account_balance, after_n_account_balance);
        let after_to_n_account_balance = Balances::free_balance(&account(to_n_account));
        assert_eq!(to_n_account_balance, after_to_n_account_balance);

        // no ledger balances
        // let n_account_unbondings: BTreeMap<u32, u128> =
        //     StakeUnbondingLedger::<Test>::get(account(n_account));
        // assert_eq!(n_account_unbondings.len(), 0);
        // let to_n_account_unbondings: BTreeMap<u32, u128> =
        //     StakeUnbondingLedger::<Test>::get(account(to_n_account));
        // assert_eq!(to_n_account_unbondings.len(), 0);

        let n_account_unbondings = StakeUnbondingLedger::<Test>::get(account(n_account));
        assert_eq!(n_account_unbondings.len(), 0);
        let to_n_account_unbondings = StakeUnbondingLedger::<Test>::get(account(to_n_account));
        assert_eq!(to_n_account_unbondings.len(), 0);

        let after_delegate_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(account(n_account), subnet_id);

        let after_to_delegate_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(account(to_n_account), subnet_id);

        let after_total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let after_total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        log::error!(
            "total_subnet_delegate_stake_shares  {:?}",
            total_subnet_delegate_stake_shares
        );
        log::error!(
            "total_subnet_delegate_stake_balance {:?}",
            total_subnet_delegate_stake_balance
        );

        assert_eq!(after_delegate_shares, 0);
        assert_eq!(delegate_shares, after_to_delegate_shares);
        assert_eq!(delegate_shares, after_to_delegate_shares);
        assert_eq!(
            total_subnet_delegate_stake_shares,
            after_total_subnet_delegate_stake_shares
        );
        assert_eq!(
            total_subnet_delegate_stake_balance,
            after_total_subnet_delegate_stake_balance
        );
    });
}

#[test]
fn test_transfer_delegate_stake_requires_owned_shares() {
    new_test_ext().execute_with(|| {
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnet_name: Vec<u8> = "subnet-name".into();
        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();

        let staker = account(255);
        let attacker = account(256);
        let recipient = account(257);

        let _ = Balances::deposit_creating(&staker, amount + 500);

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(staker.clone()),
            subnet_id,
            amount,
            1,
        ));

        let staker_shares = AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id);
        let total_shares = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_balance = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        assert_ne!(staker_shares, 0);
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&attacker, subnet_id),
            0
        );
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&recipient, subnet_id),
            0
        );

        assert_err!(
            Network::transfer_delegate_stake(
                RuntimeOrigin::signed(attacker.clone()),
                subnet_id,
                recipient.clone(),
                staker_shares,
            ),
            Error::<Test>::NotEnoughStakeToWithdraw
        );

        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&staker, subnet_id),
            staker_shares
        );
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&attacker, subnet_id),
            0
        );
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&recipient, subnet_id),
            0
        );
        assert_eq!(
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id),
            total_shares
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            total_balance
        );
    });
}

#[test]
fn test_transfer_delegate_stake_min_delegate_stake_deposit_not_reached() {
    new_test_ext().execute_with(|| {
        let _ = env_logger::builder().is_test(true).try_init();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnet_name: Vec<u8> = "subnet-name".into();
        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let n_account = 255;
        let to_n_account = 256;

        let _ = Balances::deposit_creating(&account(n_account), amount + 500);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        let delegate_stake_to_be_added_as_shares = Network::preview_delegate_pool_deposit(
            amount,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
            1,
        )
        .unwrap()
        .0;

        System::set_block_number(
            System::block_number()
                + DelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get(),
        );

        let starting_delegator_balance = Balances::free_balance(&account(n_account));

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(account(n_account)),
            subnet_id,
            amount,
            1,
        ));

        let n_account_balance = Balances::free_balance(&account(n_account));
        let to_n_account_balance = Balances::free_balance(&account(to_n_account));

        let delegate_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(account(n_account), subnet_id);
        assert_eq!(delegate_shares, delegate_stake_to_be_added_as_shares);
        assert_ne!(delegate_shares, 0);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        let delegate_balance = Network::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        let to_delegate_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(account(to_n_account), subnet_id);

        assert_eq!(to_delegate_shares, 0);

        assert_err!(
            Network::transfer_delegate_stake(
                RuntimeOrigin::signed(account(n_account)),
                subnet_id,
                account(to_n_account),
                1,
            ),
            Error::<Test>::MinDelegateStakeDepositNotReached
        );
    });
}
