use super::mock::*;
use crate::tests::test_utils::*;
use crate::{
    AccountSubnetDelegateStakeShares, SubnetOwner, TotalSubnetDelegateStakeBalance,
    TotalSubnetDelegateStakeShares,
};
use frame_support::{assert_err, traits::Currency, weights::WeightMeter};
use sp_core::U256;
use sp_runtime::ArithmeticError;

fn u256_to_u128(value: U256) -> u128 {
    value.try_into().expect("test value must fit in u128")
}

#[test]
fn subnet_owner_reward_reports_total_issuance_overflow_without_crediting() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let owner = account(980);
        let filler = account(981);
        let reward = 1_000u128;

        drop(Balances::deposit_creating(&owner, EXISTENTIAL_DEPOSIT));
        SubnetOwner::<Test>::insert(subnet_id, &owner);

        let issuance_headroom = reward - 1;
        let filler_amount = u128::MAX
            .checked_sub(Balances::total_issuance())
            .and_then(|remaining| remaining.checked_sub(issuance_headroom))
            .expect("test issuance must have room for the filler account");
        drop(Balances::deposit_creating(&filler, filler_amount));

        let owner_balance_before = Balances::free_balance(&owner);
        let issuance_before = Balances::total_issuance();
        assert_err!(
            Network::handle_subnet_owner_reward(&mut WeightMeter::new(), subnet_id, reward,),
            ArithmeticError::Overflow
        );
        assert_eq!(Balances::free_balance(&owner), owner_balance_before);
        assert_eq!(Balances::total_issuance(), issuance_before);
    });
}

#[test]
fn foundation_reward_reports_total_issuance_overflow_without_crediting() {
    new_test_ext().execute_with(|| {
        let filler = account(982);
        let treasury = TreasuryAccount::get();
        let reward = 1_000u128;
        let issuance_headroom = reward - 1;
        let filler_amount = u128::MAX
            .checked_sub(Balances::total_issuance())
            .and_then(|remaining| remaining.checked_sub(issuance_headroom))
            .expect("test issuance must have room for the filler account");
        drop(Balances::deposit_creating(&filler, filler_amount));

        let treasury_balance_before = Balances::free_balance(&treasury);
        let issuance_before = Balances::total_issuance();
        assert_err!(
            Network::add_balance_to_treasury(reward),
            ArithmeticError::Overflow
        );
        assert_eq!(Balances::free_balance(&treasury), treasury_balance_before);
        assert_eq!(Balances::total_issuance(), issuance_before);
    });
}

#[test]
fn virtual_offset_applies_to_empty_pool_quotes() {
    new_test_ext().execute_with(|| {
        let assets = 123_456_789u128;
        let shares = 9_876_543_210u128;
        let virtual_shares = Network::DELEGATE_POOL_VIRTUAL_SHARES;
        let virtual_assets = Network::DELEGATE_POOL_VIRTUAL_BALANCE;

        let quoted_shares = Network::try_convert_to_shares(assets, 0, 0).unwrap();
        let expected_shares = u256_to_u128(
            U256::from(assets) * U256::from(virtual_shares) / U256::from(virtual_assets),
        );
        assert_eq!(quoted_shares, expected_shares);
        assert_ne!(
            quoted_shares, assets,
            "the empty pool must not bypass its virtual offset"
        );

        let quoted_assets = Network::try_convert_to_balance(shares, 0, 0).unwrap();
        let expected_assets = u256_to_u128(
            U256::from(shares) * U256::from(virtual_assets) / U256::from(virtual_shares),
        );
        assert_eq!(quoted_assets, expected_assets);
        assert_eq!(Network::convert_to_shares(0, 0, 0), 0);
        assert_eq!(Network::convert_to_balance(0, 0, 0), 0);
    });
}

#[test]
fn first_deposit_allocates_gross_user_and_dead_shares_exactly() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let depositor = account(1);
        let deposit = 1_000u128;

        let (expected_user_shares, expected_gross_shares) =
            Network::preview_delegate_pool_deposit(deposit, 0, 0, 1).unwrap();
        let (credited_assets, credited_user_shares) =
            Network::handle_increase_account_delegate_stake(&depositor, subnet_id, deposit)
                .unwrap();

        assert_eq!(credited_assets, deposit);
        assert_eq!(credited_user_shares, expected_user_shares);
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&depositor, subnet_id),
            expected_user_shares
        );
        assert_eq!(
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id),
            expected_gross_shares
        );
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            deposit
        );
        assert_eq!(
            expected_gross_shares - expected_user_shares,
            Network::DELEGATE_POOL_MIN_LIQUIDITY,
            "the first deposit must permanently separate the dead-share allocation"
        );
    });
}

#[test]
fn first_depositor_pays_exactly_one_atomic_asset_for_initialization() {
    new_test_ext().execute_with(|| {
        let deposit = 1_000_000_000_000_000_000u128;
        let (user_shares, gross_shares) =
            Network::preview_delegate_pool_deposit(deposit, 0, 0, 1).unwrap();

        let immediately_redeemable =
            Network::try_convert_to_balance(user_shares, gross_shares, deposit).unwrap();

        assert_eq!(immediately_redeemable, deposit - 1);
        assert_eq!(deposit - immediately_redeemable, 1);
    });
}

#[test]
fn failed_checked_conversions_error_while_infallible_quotes_return_zero() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            Network::try_convert_to_shares(1, 0, 1),
            Err(ArithmeticError::Underflow)
        );
        assert_eq!(Network::convert_to_shares(1, 0, 1), 0);
        assert_eq!(
            Network::try_convert_to_balance(1, 0, 1),
            Err(ArithmeticError::Underflow)
        );
        assert_eq!(Network::convert_to_balance(1, 0, 1), 0);
        assert_eq!(
            Network::try_convert_to_shares(
                1,
                Network::DELEGATE_POOL_MIN_LIQUIDITY - 1,
                1,
            ),
            Err(ArithmeticError::Underflow)
        );
        assert_eq!(
            Network::try_convert_to_balance(
                1,
                Network::DELEGATE_POOL_MIN_LIQUIDITY - 1,
                1,
            ),
            Err(ArithmeticError::Underflow)
        );

        assert_eq!(
            Network::try_convert_to_shares(u128::MAX, u128::MAX, 1),
            Err(ArithmeticError::Overflow)
        );
        assert_eq!(Network::convert_to_shares(u128::MAX, u128::MAX, 1), 0);
        assert_eq!(
            Network::try_convert_to_balance(
                u128::MAX,
                Network::DELEGATE_POOL_MIN_LIQUIDITY,
                u128::MAX,
            ),
            Err(ArithmeticError::Overflow)
        );
        assert_eq!(
            Network::convert_to_balance(
                u128::MAX,
                Network::DELEGATE_POOL_MIN_LIQUIDITY,
                u128::MAX,
            ),
            0
        );
    });
}

#[test]
fn protocol_minimum_shares_defeats_large_repeated_rounding_attack() {
    new_test_ext().execute_with(|| {
        const ATTACKER_DEPOSIT: u128 = 1_000;
        const VICTIM_DEPOSIT: u128 = 1_000;
        const VICTIM_COUNT: usize = 100_000;

        let minimum_shares = Network::DELEGATE_POOL_MIN_MINTED_SHARES;
        let virtual_shares = Network::DELEGATE_POOL_VIRTUAL_SHARES;
        let virtual_assets = Network::DELEGATE_POOL_VIRTUAL_BALANCE;

        let (attacker_shares, mut total_shares) =
            Network::preview_delegate_pool_deposit(ATTACKER_DEPOSIT, 0, 0, 1).unwrap();
        let mut total_assets = ATTACKER_DEPOSIT;

        // Put the exchange rate immediately inside the worst accepted band: every victim receives
        // exactly the protocol minimum, with the discarded fraction as close as possible to one
        // additional share. This is the repeated-rounding shape an attacker would try to exploit.
        let victim_numerator = U256::from(VICTIM_DEPOSIT)
            * U256::from(total_shares.checked_add(virtual_shares).unwrap());
        let adversarial_denominator =
            victim_numerator / U256::from(minimum_shares + 1) + U256::one();
        let donation = u256_to_u128(adversarial_denominator)
            .checked_sub(total_assets)
            .and_then(|value| value.checked_sub(virtual_assets))
            .expect("adversarial donation must be positive");
        total_assets = total_assets.checked_add(donation).unwrap();

        for victim_index in 0..VICTIM_COUNT {
            let (victim_shares, gross_shares) = Network::preview_delegate_pool_deposit(
                VICTIM_DEPOSIT,
                total_shares,
                total_assets,
                1,
            )
            .unwrap_or_else(|error| {
                panic!("victim {victim_index} unexpectedly rejected: {error:?}")
            });
            assert_eq!(
                victim_shares, minimum_shares,
                "fixture must remain in the maximally adversarial accepted rounding band"
            );
            assert_eq!(gross_shares, victim_shares);
            total_shares = total_shares.checked_add(gross_shares).unwrap();
            total_assets = total_assets.checked_add(VICTIM_DEPOSIT).unwrap();
        }

        let attacker_redemption =
            Network::try_convert_to_balance(attacker_shares, total_shares, total_assets).unwrap();
        let attacker_cost = ATTACKER_DEPOSIT.checked_add(donation).unwrap();
        assert!(
            attacker_redemption < attacker_cost,
            "repeated victim rounding must not repay the attacker's deposit and donation: redemption={attacker_redemption}, cost={attacker_cost}"
        );
    });
}

#[test]
fn conversions_are_monotonic_and_immediate_round_trips_never_over_credit() {
    new_test_ext().execute_with(|| {
        let virtual_shares = Network::DELEGATE_POOL_VIRTUAL_SHARES;
        let dead_shares = Network::DELEGATE_POOL_MIN_LIQUIDITY;
        let large_assets = 1_000_000_000_000_000_000u128;
        let large_shares = large_assets.checked_mul(virtual_shares).unwrap();

        // All nonempty fixtures are reachable from an initial deposit, optionally followed by a
        // donation/reward that increases assets without minting shares.
        let pool_states = [
            (0u128, 0u128),
            (
                2u128.checked_mul(virtual_shares).unwrap(),
                2u128,
            ),
            (
                1_000u128.checked_mul(virtual_shares).unwrap(),
                1_000u128,
            ),
            (
                1_000u128.checked_mul(virtual_shares).unwrap(),
                1_000_000u128,
            ),
            (large_shares, large_assets),
        ];
        let asset_inputs = [0u128, 1, 2, 999, 1_000, 1_000_000, large_assets];

        for (total_shares, total_assets) in pool_states {
            let mut previous_share_quote = 0u128;
            for assets in asset_inputs {
                let share_quote =
                    Network::try_convert_to_shares(assets, total_shares, total_assets).unwrap();
                assert!(
                    share_quote >= previous_share_quote,
                    "share quote decreased for pool S={total_shares}, A={total_assets}"
                );
                previous_share_quote = share_quote;
            }

            let maximum_user_shares = total_shares.saturating_sub(dead_shares.min(total_shares));
            let share_inputs = [
                0u128,
                1u128.min(maximum_user_shares),
                maximum_user_shares / 1_000,
                maximum_user_shares / 2,
                maximum_user_shares,
            ];
            let mut previous_asset_quote = 0u128;
            for shares in share_inputs {
                let asset_quote =
                    Network::try_convert_to_balance(shares, total_shares, total_assets).unwrap();
                assert!(
                    asset_quote >= previous_asset_quote,
                    "asset quote decreased for pool S={total_shares}, A={total_assets}"
                );
                assert!(asset_quote <= total_assets);
                previous_asset_quote = asset_quote;
            }

            for deposit in [1_000u128, 1_001, 1_000_000, large_assets] {
                let preview = Network::preview_delegate_pool_deposit(
                    deposit,
                    total_shares,
                    total_assets,
                    1,
                );
                let Ok((user_shares, gross_shares)) = preview else {
                    // Rejection is the expected fail-closed result when an inflated pool would mint
                    // fewer than the protocol minimum shares.
                    let raw_quote = Network::try_convert_to_shares(
                        deposit,
                        total_shares,
                        total_assets,
                    )
                    .unwrap();
                    assert!(raw_quote < Network::DELEGATE_POOL_MIN_MINTED_SHARES);
                    continue;
                };

                let post_deposit_shares = total_shares.checked_add(gross_shares).unwrap();
                let post_deposit_assets = total_assets.checked_add(deposit).unwrap();
                let immediate_redemption = Network::try_convert_to_balance(
                    user_shares,
                    post_deposit_shares,
                    post_deposit_assets,
                )
                .unwrap();
                assert!(
                    immediate_redemption <= deposit,
                    "deposit/redeem over-credited for pool S={total_shares}, A={total_assets}, deposit={deposit}"
                );
            }
        }
    });
}

#[test]
fn pool_supply_captures_both_conversion_remainders_without_rate_improvement() {
    new_test_ext().execute_with(|| {
        let virtual_shares = Network::DELEGATE_POOL_VIRTUAL_SHARES;
        let virtual_assets = Network::DELEGATE_POOL_VIRTUAL_BALANCE;
        let deposit = 1_000u128;

        let (incumbent_shares, mut total_shares) =
            Network::preview_delegate_pool_deposit(deposit, 0, 0, 1).unwrap();
        let mut circulating_shares = incumbent_shares;
        let mut total_assets = deposit + 1; // a protocol reward makes subsequent quotes inexact

        let floor_quote =
            Network::try_convert_to_shares(deposit, total_shares, total_assets).unwrap();
        let ceiling_quote =
            Network::try_convert_to_shares_round_up(deposit, total_shares, total_assets).unwrap();
        assert_eq!(ceiling_quote, floor_quote + 1);

        let incumbent_value_before =
            Network::try_convert_to_balance(incumbent_shares, total_shares, total_assets).unwrap();

        // Repeated participant round trips must not increase the virtualized asset/share rate.
        for _ in 0..1_000 {
            let old_shares = total_shares;
            let old_assets = total_assets;
            let (participant_shares, gross_shares) =
                Network::preview_delegate_pool_deposit(deposit, total_shares, total_assets, 1)
                    .unwrap();
            assert!(gross_shares == participant_shares || gross_shares == participant_shares + 1);

            total_shares = total_shares.checked_add(gross_shares).unwrap();
            circulating_shares = circulating_shares.checked_add(participant_shares).unwrap();
            total_assets = total_assets.checked_add(deposit).unwrap();

            let old_rate_left =
                U256::from(old_assets + virtual_assets) * U256::from(total_shares + virtual_shares);
            let new_rate_left =
                U256::from(total_assets + virtual_assets) * U256::from(old_shares + virtual_shares);
            assert!(new_rate_left <= old_rate_left);

            let old_shares = total_shares;
            let old_assets = total_assets;
            let (assets_out, supply_shares_burned) = Network::preview_delegate_pool_redemption(
                participant_shares,
                total_shares,
                total_assets,
                1,
            )
            .unwrap();
            assert!(supply_shares_burned <= participant_shares);

            total_shares = total_shares.checked_sub(supply_shares_burned).unwrap();
            circulating_shares = circulating_shares.checked_sub(participant_shares).unwrap();
            total_assets = total_assets.checked_sub(assets_out).unwrap();

            let old_rate_left =
                U256::from(old_assets + virtual_assets) * U256::from(total_shares + virtual_shares);
            let new_rate_left =
                U256::from(total_assets + virtual_assets) * U256::from(old_shares + virtual_shares);
            assert!(new_rate_left <= old_rate_left);
            assert!(total_shares - circulating_shares >= Network::DELEGATE_POOL_MIN_LIQUIDITY);
        }

        let incumbent_value_after =
            Network::try_convert_to_balance(incumbent_shares, total_shares, total_assets).unwrap();
        assert!(incumbent_value_after <= incumbent_value_before);
    });
}

#[test]
fn protocol_share_minimum_still_allows_one_atomic_unit_loss_for_small_deposits() {
    new_test_ext().execute_with(|| {
        let deposit = 1_000u128;
        let (user_shares, gross_shares) =
            Network::preview_delegate_pool_deposit(deposit, 0, 0, 1).unwrap();
        assert!(user_shares >= Network::DELEGATE_POOL_MIN_MINTED_SHARES);

        let (assets_out, supply_shares_burned) =
            Network::preview_delegate_pool_redemption(user_shares, gross_shares, deposit, 1)
                .unwrap();
        assert_eq!(assets_out, deposit - 1);
        assert!(supply_shares_burned <= user_shares);
        assert_eq!(deposit - assets_out, 1);
    });
}

#[test]
fn historical_499_participant_sequence_does_not_increase_incumbent_value() {
    new_test_ext().execute_with(|| {
        let deposit = 1_000u128;
        let (mut incumbent_shares, mut total_shares) =
            Network::preview_delegate_pool_deposit(deposit, 0, 0, 1).unwrap();
        let mut total_assets = deposit;

        // This partial redemption previously left the pool one supply share below its exact rate.
        let partial_account_burn = 500_000_000_001u128;
        let (partial_assets, partial_supply_burn) = Network::preview_delegate_pool_redemption(
            partial_account_burn,
            total_shares,
            total_assets,
            1,
        )
        .unwrap();
        assert_eq!(partial_assets, 500);
        assert_eq!(partial_supply_burn, 500_000_000_000);
        incumbent_shares -= partial_account_burn;
        total_shares -= partial_supply_burn;
        total_assets -= partial_assets;

        let incumbent_value_before =
            Network::try_convert_to_balance(incumbent_shares, total_shares, total_assets).unwrap();

        for _ in 0..499 {
            let (participant_shares, gross_shares) =
                Network::preview_delegate_pool_deposit(deposit, total_shares, total_assets, 1)
                    .unwrap();
            total_shares += gross_shares;
            total_assets += deposit;

            let (assets_out, supply_burn) = Network::preview_delegate_pool_redemption(
                participant_shares,
                total_shares,
                total_assets,
                1,
            )
            .unwrap();
            assert_eq!(assets_out, deposit);
            total_shares -= supply_burn;
            total_assets -= assets_out;

            assert!(
                Network::try_convert_to_balance(incumbent_shares, total_shares, total_assets,)
                    .unwrap()
                    <= incumbent_value_before
            );
        }

        let (final_assets, _) = Network::preview_delegate_pool_redemption(
            incumbent_shares,
            total_shares,
            total_assets,
            1,
        )
        .unwrap();
        assert_eq!(partial_assets + final_assets, 998);
        assert!(partial_assets + final_assets <= deposit);
    });
}
