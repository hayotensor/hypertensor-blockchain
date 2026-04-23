use super::mock::*;
use crate::tests::test_utils::*;
use crate::Event;
use crate::{
    DelegateAccountStake, AccountOverwatchStake, AccountSubnetStake, ColdkeyHotkeys,
    ColdkeyIdentity, ColdkeyIdentityNameOwner, ColdkeyReputation, DefaultMaxSocialIdLength,
    DefaultMaxUrlLength, DefaultMaxVectorLength, DelegateAccount, Error, HotkeyOverwatchNodeId,
    HotkeyOwner, HotkeySubnetId, HotkeySubnetNodeId, MaxSubnetNodes, MaxSubnets,
    MinActiveNodeStakeEpochs, MinSubnetMinStake, OverwatchMinStakeBalance, OverwatchNodeIdHotkey,
    OverwatchNodes, PeerInfo, StakeCooldownEpochs, StakeUnbondingLedger, SubnetName,
    SubnetNodeClass, SubnetNodeIdHotkey, SubnetNodesData, SubnetState, TotalAccountDelegateStake,
    TotalActiveSubnets, TotalSubnetNodes, TotalValidatorIds, ValidatorsData,
};
use frame_support::traits::Currency;
use frame_support::{assert_err, assert_ok};
use sp_std::collections::btree_map::BTreeMap;

#[test]
fn test_update_delegate_account() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let reward_rate = 50000000000000000; // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();

        let new_delegate_account_id = account(100);
        let delegate_rate = 400000000000000000; // 40%
        assert_ok!(Network::update_validator_delegate_account(
            RuntimeOrigin::signed(coldkey.clone()),
            current_id,
            Some(new_delegate_account_id),
            Some(delegate_rate),
        ));

        let data = ValidatorsData::<Test>::get(current_id);
        assert_eq!(
            data.delegate_account.clone().unwrap().account_id,
            new_delegate_account_id
        );
        assert_eq!(data.delegate_account.clone().unwrap().rate, delegate_rate);
    })
}

#[test]
fn test_update_delegate_account_not_key_owner_error() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let reward_rate = 50000000000000000; // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();

        // sanity check
        let validator = ValidatorsData::<Test>::get(current_id);
        assert_eq!(validator.delegate_account, None);

        let new_delegate_account_id = account(100);
        let delegate_rate = 400000000000000000; // 40%
        assert_err!(
            Network::update_validator_delegate_account(
                RuntimeOrigin::signed(account(100)),
                current_id,
                Some(new_delegate_account_id),
                Some(delegate_rate),
            ),
            Error::<Test>::NotKeyOwner
        );
    })
}

#[test]
fn test_update_delegate_account_invalid_delegate_account_parameters_error() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let reward_rate = 50000000000000000; // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();

        // sanity check
        let validator = ValidatorsData::<Test>::get(current_id);
        assert_eq!(validator.delegate_account, None);

        assert_err!(
            Network::update_validator_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                current_id,
                None,
                None,
            ),
            Error::<Test>::InvalidDelegateAccountParameters
        );
    })
}

#[test]
fn test_update_delegate_account_delegate_account_id_none_error() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let reward_rate = 50000000000000000; // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();

        // sanity check
        let validator = ValidatorsData::<Test>::get(current_id);
        assert_eq!(validator.delegate_account, None);

        assert_err!(
            Network::update_validator_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                current_id,
                None,
                Some(1),
            ),
            Error::<Test>::DelegateAccountIdIsNone
        );
    })
}

#[test]
fn test_update_delegate_account_delegate_account_rate_none_error() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let reward_rate = 50000000000000000; // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();

        // sanity check
        let validator = ValidatorsData::<Test>::get(current_id);
        assert_eq!(validator.delegate_account, None);

        assert_err!(
            Network::update_validator_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                current_id,
                Some(account(100)),
                None,
            ),
            Error::<Test>::DelegateAccountRateIsNone
        );
    })
}

#[test]
fn test_update_delegate_account_delegate_account_cannot_be_hotkey_error() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let reward_rate = 50000000000000000; // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();

        // sanity check
        let validator = ValidatorsData::<Test>::get(current_id);
        assert_eq!(validator.delegate_account, None);

        assert_err!(
            Network::update_validator_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                current_id,
                Some(hotkey),
                Some(1),
            ),
            Error::<Test>::DelegateAccountCannotBeHotkey
        );
    })
}

#[test]
fn test_register_subnet_node_delegate_account_cannot_be_hotkey_error() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);

        let delegate_account = DelegateAccount {
            account_id: hotkey,
            rate: 0,
        };

        let reward_rate = 50000000000000000; // 5%
        assert_err!(
            Network::do_register_validator(
                RuntimeOrigin::signed(coldkey.clone()),
                hotkey,
                reward_rate,
                Some(delegate_account),
                None,
            ),
            Error::<Test>::DelegateAccountCannotBeHotkey
        );
    })
}

#[test]
fn test_update_delegate_account_delegate_account_cannot_be_coldkey_error() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let reward_rate = 50000000000000000; // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();

        // sanity check
        let validator = ValidatorsData::<Test>::get(current_id);
        assert_eq!(validator.delegate_account, None);

        assert_err!(
            Network::update_validator_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                current_id,
                Some(coldkey),
                Some(1),
            ),
            Error::<Test>::DelegateAccountCannotBeColdkey
        );
    })
}

#[test]
fn test_register_subnet_node_delegate_account_cannot_be_coldkey_error() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);

        let delegate_account = DelegateAccount {
            account_id: coldkey,
            rate: 0,
        };

        let hotkey = account(1);
        let reward_rate = 50000000000000000; // 5%
        assert_err!(
            Network::do_register_validator(
                RuntimeOrigin::signed(coldkey.clone()),
                hotkey,
                reward_rate,
                Some(delegate_account),
                None,
            ),
            Error::<Test>::DelegateAccountCannotBeColdkey
        );
    })
}

#[test]
fn test_update_delegate_account_invalid_delegate_account_rate_error() {
    new_test_ext().execute_with(|| {
        let coldkey = account(0);
        let hotkey = account(1);
        let reward_rate = 50000000000000000; // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();

        // sanity check
        let validator = ValidatorsData::<Test>::get(current_id);
        assert_eq!(validator.delegate_account, None);

        assert_err!(
            Network::update_validator_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                current_id,
                Some(account(100)),
                Some(0),
            ),
            Error::<Test>::InvalidDelegateAccountRate
        );

        assert_err!(
            Network::update_validator_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                current_id,
                Some(account(100)),
                Some(1000000000000000001),
            ),
            Error::<Test>::InvalidDelegateAccountRate
        );
    })
}

#[test]
fn test_register_subnet_node_delegate_account_invalid_delegate_account_rate_error() {
    new_test_ext().execute_with(|| {
        let delegate_account = DelegateAccount {
            account_id: account(99),
            rate: 0,
        };

        let coldkey = account(0);
        let hotkey = account(1);
        let reward_rate = 50000000000000000; // 5%
        assert_err!(
            Network::do_register_validator(
                RuntimeOrigin::signed(coldkey.clone()),
                hotkey,
                reward_rate,
                Some(delegate_account),
                None,
            ),
            Error::<Test>::InvalidDelegateAccountRate
        );

        let delegate_account = DelegateAccount {
            account_id: account(99),
            rate: 1000000000000000001,
        };

        assert_err!(
            Network::do_register_validator(
                RuntimeOrigin::signed(coldkey.clone()),
                hotkey,
                reward_rate,
                Some(delegate_account),
                None,
            ),
            Error::<Test>::InvalidDelegateAccountRate
        );
    })
}

#[test]
fn test_remove_delegate_balance() {
    new_test_ext().execute_with(|| {
        System::set_block_number(System::block_number() + 1);

        let account_id = account(100);

        assert_eq!(DelegateAccountStake::<Test>::get(&account_id), 0);
        assert_eq!(TotalAccountDelegateStake::<Test>::get(), 0);

        Network::increase_delegate_account_balance(&account_id, 100);

        assert_eq!(DelegateAccountStake::<Test>::get(&account_id), 100);
        assert_eq!(TotalAccountDelegateStake::<Test>::get(), 100);

        let block = System::block_number();

        assert_ok!(Network::remove_delegate_balance(
            RuntimeOrigin::signed(account_id.clone()),
            100,
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::DelegateBalanceRemoved {
                account_id: account_id.clone(),
                amount: 100,
            }
        );

        assert_eq!(DelegateAccountStake::<Test>::get(&account_id), 0);
        assert_eq!(TotalAccountDelegateStake::<Test>::get(), 0);

        let unbondings: BTreeMap<u32, u128> = StakeUnbondingLedger::<Test>::get(&account_id);
        assert_eq!(unbondings.len(), 1);
        let (ledger_block, ledger_balance) = unbondings.iter().next().unwrap();
        assert_eq!(
            *ledger_block,
            &block + StakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );
        assert_eq!(*ledger_balance, 100);
    })
}

#[test]
fn test_remove_delegate_balance_amount_zero_error() {
    new_test_ext().execute_with(|| {
        let account_id = account(100);

        assert_eq!(DelegateAccountStake::<Test>::get(&account_id), 0);
        assert_eq!(TotalAccountDelegateStake::<Test>::get(), 0);

        Network::increase_delegate_account_balance(&account_id, 100);

        assert_eq!(DelegateAccountStake::<Test>::get(&account_id), 100);
        assert_eq!(TotalAccountDelegateStake::<Test>::get(), 100);

        assert_err!(
            Network::remove_delegate_balance(RuntimeOrigin::signed(account_id.clone()), 0,),
            Error::<Test>::AmountZero
        );

        assert_eq!(DelegateAccountStake::<Test>::get(&account_id), 100);
        assert_eq!(TotalAccountDelegateStake::<Test>::get(), 100);
    })
}

#[test]
fn test_remove_delegate_balance_not_enough_stake_error() {
    new_test_ext().execute_with(|| {
        let account_id = account(100);

        assert_eq!(DelegateAccountStake::<Test>::get(&account_id), 0);
        assert_eq!(TotalAccountDelegateStake::<Test>::get(), 0);

        Network::increase_delegate_account_balance(&account_id, 100);

        assert_eq!(DelegateAccountStake::<Test>::get(&account_id), 100);
        assert_eq!(TotalAccountDelegateStake::<Test>::get(), 100);

        assert_err!(
            Network::remove_delegate_balance(RuntimeOrigin::signed(account_id.clone()), 101,),
            Error::<Test>::NotEnoughStakeToWithdraw
        );

        assert_eq!(DelegateAccountStake::<Test>::get(&account_id), 100);
        assert_eq!(TotalAccountDelegateStake::<Test>::get(), 100);
    })
}
