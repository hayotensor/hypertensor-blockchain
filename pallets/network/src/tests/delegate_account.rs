use super::mock::*;
use crate::tests::test_utils::*;
use crate::Event;
use crate::{
    AccountOverwatchStake, AccountSubnetStake, ColdkeyHotkeys, ColdkeyIdentity,
    ColdkeyIdentityNameOwner, ColdkeyReputation, DefaultMaxSocialIdLength, DefaultMaxUrlLength,
    DefaultMaxVectorLength, DelegateAccount, Error, HotkeyOverwatchNodeId, HotkeyOwner,
    HotkeySubnetId, HotkeySubnetNodeId, MaxSubnetNodes, MaxSubnets, MinActiveNodeStakeEpochs,
    MinSubnetMinStake, OverwatchMinStakeBalance, OverwatchNodeIdHotkey, OverwatchNodes,
    StakeUnbondingLedger, SubnetName, SubnetNodeClass, SubnetNodeIdHotkey, SubnetNodesData,
    SubnetState, TotalActiveSubnets, TotalSubnetNodes,
};
use frame_support::traits::Currency;
use frame_support::{assert_err, assert_ok};
use sp_std::collections::btree_map::BTreeMap;

#[test]
fn test_update_delegate_account() {
    new_test_ext().execute_with(|| {
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;

        manual_insert_subnet_node(
            subnet_id,
            subnet_node_id,
            coldkey_n,
            hotkey_n,
            2, // peer
            SubnetNodeClass::Validator,
            0, // start epoch,
            None,
        );

        // sanity check
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.delegate_account, None);

        let new_delegate_account_id = account(100);
        let delegate_rate = 400000000000000000; // 40%
        assert_ok!(Network::update_delegate_account(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            Some(new_delegate_account_id),
            Some(delegate_rate),
        ));

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(
            subnet_node.delegate_account.clone().unwrap().account_id,
            new_delegate_account_id
        );
        assert_eq!(
            subnet_node.delegate_account.clone().unwrap().rate,
            delegate_rate
        );
    })
}

#[test]
fn test_update_delegate_account_not_key_owner_error() {
    new_test_ext().execute_with(|| {
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;

        manual_insert_subnet_node(
            subnet_id,
            subnet_node_id,
            coldkey_n,
            hotkey_n,
            2, // peer
            SubnetNodeClass::Validator,
            0, // start epoch,
            None,
        );

        // sanity check
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.delegate_account, None);

        let new_delegate_account_id = account(100);
        let delegate_rate = 400000000000000000; // 40%
        assert_err!(
            Network::update_delegate_account(
                RuntimeOrigin::signed(account(100)),
                subnet_id,
                subnet_node_id,
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
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;

        manual_insert_subnet_node(
            subnet_id,
            subnet_node_id,
            coldkey_n,
            hotkey_n,
            2, // peer
            SubnetNodeClass::Validator,
            0, // start epoch,
            None,
        );

        // sanity check
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.delegate_account, None);

        assert_err!(
            Network::update_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
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
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;

        manual_insert_subnet_node(
            subnet_id,
            subnet_node_id,
            coldkey_n,
            hotkey_n,
            2, // peer
            SubnetNodeClass::Validator,
            0, // start epoch,
            None,
        );

        // sanity check
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.delegate_account, None);

        assert_err!(
            Network::update_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
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
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;

        manual_insert_subnet_node(
            subnet_id,
            subnet_node_id,
            coldkey_n,
            hotkey_n,
            2, // peer
            SubnetNodeClass::Validator,
            0, // start epoch,
            None,
        );

        // sanity check
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.delegate_account, None);

        assert_err!(
            Network::update_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
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
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;

        manual_insert_subnet_node(
            subnet_id,
            subnet_node_id,
            coldkey_n,
            hotkey_n,
            2, // peer
            SubnetNodeClass::Validator,
            0, // start epoch,
            None,
        );

        // sanity check
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.delegate_account, None);

        assert_err!(
            Network::update_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                Some(account(hotkey_n)),
                Some(1),
            ),
            Error::<Test>::DelegateAccountCannotBeHotkey
        );
    })
}

#[test]
fn test_register_subnet_node_delegate_account_cannot_be_hotkey_error() {
    new_test_ext().execute_with(|| {
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 0;
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        insert_subnet(subnet_id, SubnetState::Active, 0);

        let coldkey = get_coldkey(subnet_id, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnet_id, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnet_id, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnet_id, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnet_id, max_subnet_nodes, max_subnets, end + 1);

        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount);

        let delegate_account = DelegateAccount {
            account_id: hotkey.clone(),
            rate: 1,
        };
        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                hotkey.clone(),
                peer_id,
                bootnode_peer_id,
                client_peer_id,
                None,
                0,
                amount,
                None,
                None,
                Some(delegate_account),
                u128::MAX
            ),
            Error::<Test>::DelegateAccountCannotBeHotkey
        );
    })
}

#[test]
fn test_update_delegate_account_delegate_account_cannot_be_coldkey_error() {
    new_test_ext().execute_with(|| {
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;

        manual_insert_subnet_node(
            subnet_id,
            subnet_node_id,
            coldkey_n,
            hotkey_n,
            2, // peer
            SubnetNodeClass::Validator,
            0, // start epoch,
            None,
        );

        // sanity check
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.delegate_account, None);

        assert_err!(
            Network::update_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                Some(account(coldkey_n)),
                Some(1),
            ),
            Error::<Test>::DelegateAccountCannotBeColdkey
        );
    })
}

#[test]
fn test_register_subnet_node_delegate_account_cannot_be_coldkey_error() {
    new_test_ext().execute_with(|| {
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 0;
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        insert_subnet(subnet_id, SubnetState::Active, 0);

        let coldkey = get_coldkey(subnet_id, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnet_id, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnet_id, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnet_id, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnet_id, max_subnet_nodes, max_subnets, end + 1);

        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount);

        let delegate_account = DelegateAccount {
            account_id: coldkey.clone(),
            rate: 1,
        };
        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                hotkey.clone(),
                peer_id,
                bootnode_peer_id,
                client_peer_id,
                None,
                0,
                amount,
                None,
                None,
                Some(delegate_account),
                u128::MAX
            ),
            Error::<Test>::DelegateAccountCannotBeColdkey
        );
    })
}

#[test]
fn test_update_delegate_account_invalid_delegate_account_rate_error() {
    new_test_ext().execute_with(|| {
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;

        manual_insert_subnet_node(
            subnet_id,
            subnet_node_id,
            coldkey_n,
            hotkey_n,
            2, // peer
            SubnetNodeClass::Validator,
            0, // start epoch,
            None,
        );

        // sanity check
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.delegate_account, None);

        assert_err!(
            Network::update_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                Some(account(100)),
                Some(0),
            ),
            Error::<Test>::InvalidDelegateAccountRate
        );

        assert_err!(
            Network::update_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                Some(account(100)),
                Some(1000000000000000001),
            ),
            Error::<Test>::InvalidDelegateAccountRate
        );
    })
}

#[test]
fn test_register_subnet_node_delegate_account_invalid_delegate_accountrate_error() {
    new_test_ext().execute_with(|| {
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 0;
        let subnet_name: Vec<u8> = "subnet-name".into();

        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        insert_subnet(subnet_id, SubnetState::Active, 0);

        let coldkey = get_coldkey(subnet_id, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnet_id, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnet_id, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnet_id, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnet_id, max_subnet_nodes, max_subnets, end + 1);

        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount);

        let delegate_account = DelegateAccount {
            account_id: account(99),
            rate: 0,
        };
        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                hotkey.clone(),
                peer_id,
                bootnode_peer_id,
                client_peer_id,
                None,
                0,
                amount,
                None,
                None,
                Some(delegate_account),
                u128::MAX
            ),
            Error::<Test>::InvalidDelegateAccountRate
        );

        let delegate_account = DelegateAccount {
            account_id: account(99),
            rate: 1000000000000000001,
        };

        assert_err!(
            Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                hotkey.clone(),
                peer_id,
                bootnode_peer_id,
                client_peer_id,
                None,
                0,
                amount,
                None,
                None,
                Some(delegate_account),
                u128::MAX
            ),
            Error::<Test>::InvalidDelegateAccountRate
        );

    })
}

#[test]
fn test_transfer_delegate_account() {
    new_test_ext().execute_with(|| {
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;

        manual_insert_subnet_node(
            subnet_id,
            subnet_node_id,
            coldkey_n,
            hotkey_n,
            2, // peer
            SubnetNodeClass::Validator,
            0, // start epoch,
            Some(DelegateAccount {
                account_id: account(100),
                rate: 300000000000000000, // 30%
            }),
        );

        // sanity check
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(
            subnet_node.delegate_account.clone().unwrap().account_id,
            account(100)
        );
        assert_eq!(
            subnet_node.delegate_account.clone().unwrap().rate,
            300000000000000000
        );

        assert_ok!(Network::transfer_delegate_account(
            RuntimeOrigin::signed(account(100)),
            subnet_id,
            subnet_node_id,
            account(200),
        ));

        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(
            subnet_node.delegate_account.clone().unwrap().account_id,
            account(200)
        );
        assert_eq!(
            subnet_node.delegate_account.clone().unwrap().rate,
            300000000000000000
        );
    })
}

#[test]
fn test_transfer_delegate_account_not_delegate_account_owner_error() {
    new_test_ext().execute_with(|| {
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;

        manual_insert_subnet_node(
            subnet_id,
            subnet_node_id,
            coldkey_n,
            hotkey_n,
            2, // peer
            SubnetNodeClass::Validator,
            0, // start epoch,
            Some(DelegateAccount {
                account_id: account(100),
                rate: 300000000000000000, // 30%
            }),
        );

        // sanity check
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(
            subnet_node.delegate_account.clone().unwrap().account_id,
            account(100)
        );
        assert_eq!(
            subnet_node.delegate_account.clone().unwrap().rate,
            300000000000000000
        );

        assert_err!(
            Network::transfer_delegate_account(
                RuntimeOrigin::signed(account(200)),
                subnet_id,
                subnet_node_id,
                account(300),
            ),
            Error::<Test>::NotDelegateAccountOwner
        );
    })
}

#[test]
fn test_transfer_delegate_account_delegate_account_not_set_error() {
    new_test_ext().execute_with(|| {
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;

        manual_insert_subnet_node(
            subnet_id,
            subnet_node_id,
            coldkey_n,
            hotkey_n,
            2, // peer
            SubnetNodeClass::Validator,
            0, // start epoch,
            None,
        );

        // sanity check
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(subnet_node.delegate_account, None);

        assert_err!(
            Network::transfer_delegate_account(
                RuntimeOrigin::signed(account(200)),
                subnet_id,
                subnet_node_id,
                account(300),
            ),
            Error::<Test>::NoDelegateAccountSet
        );
    })
}

#[test]
fn test_transfer_delegate_account_invalid_subnet_node_id_error() {
    new_test_ext().execute_with(|| {
        let coldkey_n = 1;
        let hotkey_n = 2;
        let coldkey = account(coldkey_n);
        let hotkey = account(hotkey_n);
        let subnet_id = 1;
        let subnet_node_id = 100;

        manual_insert_subnet_node(
            subnet_id,
            subnet_node_id,
            coldkey_n,
            hotkey_n,
            2, // peer
            SubnetNodeClass::Validator,
            0, // start epoch,
            Some(DelegateAccount {
                account_id: account(100),
                rate: 300000000000000000, // 30%
            }),
        );

        // sanity check
        let subnet_node = SubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        assert_eq!(
            subnet_node.delegate_account.clone().unwrap().account_id,
            account(100)
        );
        assert_eq!(
            subnet_node.delegate_account.clone().unwrap().rate,
            300000000000000000
        );

        assert_err!(
            Network::transfer_delegate_account(
                RuntimeOrigin::signed(account(200)),
                0,
                0,
                account(300),
            ),
            Error::<Test>::InvalidSubnetNodeId
        );
    })
}
