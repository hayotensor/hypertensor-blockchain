use super::{mock::*, test_utils::account};
use crate::*;
use frame_support::{
    assert_noop, assert_ok,
    traits::{Currency, LockableCurrency, WithdrawReasons},
};

#[test]
fn registration_burn_is_flat_and_reduces_coldkey_balance_and_issuance() {
    new_test_ext().execute_with(|| {
        let burn = TestEconomics::get().validator_registration_burn;
        assert!(burn > 0);

        // A burst and a later registration all pay exactly the same identity fee.
        for (index, block) in [(0, 1), (1, 1), (2, 10_000)] {
            System::set_block_number(block);
            let coldkey = account(index * 2);
            let hotkey = account(index * 2 + 1);
            let _ = Balances::deposit_creating(&coldkey, burn + EXISTENTIAL_DEPOSIT);
            let issuance = Balances::total_issuance();

            assert_ok!(Network::register_validator(
                RuntimeOrigin::signed(coldkey.clone()),
                hotkey.clone(),
                0,
                None,
                None,
            ));

            assert_eq!(Balances::free_balance(&coldkey), EXISTENTIAL_DEPOSIT);
            assert_eq!(Balances::free_balance(&hotkey), 0);
            assert_eq!(Balances::total_issuance(), issuance - burn);
            assert_eq!(ColdkeyValidatorId::<Test>::get(&coldkey), Some(index + 1));
            assert_eq!(HotkeyValidatorId::<Test>::get(&hotkey), Some(index + 1));
            assert_eq!(TotalValidatorIds::<Test>::get(), index + 1);
            assert_eq!(TotalStake::<Test>::get(), 0);
            assert_eq!(TotalValidatorDelegateStakeBalance::<Test>::get(), 0);
        }
    });
}

#[test]
fn insufficient_balance_or_keep_alive_failure_does_not_register_or_burn() {
    let burn = TestEconomics::get().validator_registration_burn;
    for balance in [0, burn - 1, burn, burn + EXISTENTIAL_DEPOSIT - 1] {
        new_test_ext().execute_with(|| {
            let coldkey = account(0);
            let hotkey = account(1);
            let _ = Balances::deposit_creating(&coldkey, balance);
            // Funding the hotkey must not pay for the coldkey's registration.
            let _ = Balances::deposit_creating(&hotkey, burn * 2);
            assert_noop!(
                Network::register_validator(RuntimeOrigin::signed(coldkey), hotkey, 0, None, None,),
                Error::<Test>::BalanceBurnError
            );
        });
    }
}

#[test]
fn locked_balance_cannot_pay_for_identity_registration() {
    new_test_ext().execute_with(|| {
        let burn = TestEconomics::get().validator_registration_burn;
        let coldkey = account(0);
        let balance = burn + EXISTENTIAL_DEPOSIT;
        let _ = Balances::deposit_creating(&coldkey, balance);
        Balances::set_lock(*b"identity", &coldkey, balance, WithdrawReasons::all());

        assert_noop!(
            Network::register_validator(RuntimeOrigin::signed(coldkey), account(1), 0, None, None,),
            Error::<Test>::BalanceBurnError
        );
    });
}

#[test]
fn invalid_or_duplicate_registration_does_not_charge_a_funded_coldkey() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let burn = TestEconomics::get().validator_registration_burn;
        let _ = Balances::deposit_creating(&coldkey, 3 * burn + EXISTENTIAL_DEPOSIT);

        assert_noop!(
            Network::register_validator(
                RuntimeOrigin::signed(coldkey.clone()),
                coldkey.clone(),
                0,
                None,
                None,
            ),
            Error::<Test>::ColdkeyMatchesHotkey
        );
        assert_noop!(
            Network::register_validator(
                RuntimeOrigin::signed(coldkey.clone()),
                hotkey.clone(),
                MaxDelegateStakePercentage::<Test>::get() + 1,
                None,
                None,
            ),
            Error::<Test>::InvalidDelegateRewardRate
        );
        assert_noop!(
            Network::register_validator(
                RuntimeOrigin::signed(coldkey.clone()),
                hotkey.clone(),
                0,
                Some(DelegateAccount {
                    account_id: account(2),
                    rate: 0
                }),
                None,
            ),
            Error::<Test>::InvalidDelegateAccountRate
        );

        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey.clone(),
            0,
            None,
            None,
        ));
        assert_noop!(
            Network::register_validator(RuntimeOrigin::signed(coldkey), account(3), 0, None, None,),
            Error::<Test>::NotKeyOwner
        );

        let second_coldkey = account(4);
        let _ = Balances::deposit_creating(&second_coldkey, burn + EXISTENTIAL_DEPOSIT);
        assert_noop!(
            Network::register_validator(
                RuntimeOrigin::signed(second_coldkey.clone()),
                hotkey,
                0,
                None,
                None,
            ),
            Error::<Test>::NotKeyOwner
        );

        TotalValidatorIds::<Test>::put(u32::MAX);
        assert_noop!(
            Network::register_validator(
                RuntimeOrigin::signed(second_coldkey),
                account(5),
                0,
                None,
                None,
            ),
            sp_runtime::ArithmeticError::Overflow
        );
    });
}
