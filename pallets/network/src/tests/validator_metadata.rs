use super::{mock::*, test_utils::*};
use crate::*;
use codec::Encode;
use frame_support::{assert_noop, assert_ok};

fn empty_identity() -> IdentityData<Test> {
    IdentityData {
        name: None,
        url: None,
        image: None,
        discord: None,
        x: None,
        telegram: None,
        github: None,
        hugging_face: None,
        description: None,
        misc: None,
    }
}

fn maximum_identity() -> IdentityData<Test> {
    let bytes: NetworkBytes<Test> = vec![1; NetworkMaxVectorLength::get() as usize]
        .try_into()
        .unwrap();
    let url: NetworkUrl<Test> = vec![2; NetworkMaxUrlLength::get() as usize]
        .try_into()
        .unwrap();
    let social: NetworkSocialId<Test> = vec![3; NetworkMaxSocialIdLength::get() as usize]
        .try_into()
        .unwrap();
    IdentityData {
        name: Some(bytes.clone()),
        url: Some(url.clone()),
        image: Some(url.clone()),
        discord: Some(social.clone()),
        x: Some(social.clone()),
        telegram: Some(social),
        github: Some(url.clone()),
        hugging_face: Some(url),
        description: Some(bytes.clone()),
        misc: Some(bytes),
    }
}

fn register(identity: Option<IdentityData<Test>>) -> u32 {
    System::set_block_number(1);
    fund_validator_registration(&account(100));
    assert_ok!(Network::register_validator(
        RuntimeOrigin::signed(account(100)),
        account(101),
        0,
        None,
        identity,
    ));
    ColdkeyValidatorId::<Test>::get(account(100)).unwrap()
}

#[test]
fn registration_preserves_absent_empty_and_maximum_metadata() {
    for identity in [None, Some(empty_identity()), Some(maximum_identity())] {
        new_test_ext().execute_with(|| {
            let id = register(identity.clone());
            assert_eq!(
                ValidatorMetadata::<Test>::contains_key(id),
                identity.is_some()
            );
            assert_eq!(ValidatorMetadata::<Test>::get(id), identity);
            assert_eq!(Network::get_validator_info(id).unwrap().identity, identity);
            let wire = Network::rpc_get_validator_info(id).unwrap();
            assert_eq!(wire.identity.is_some(), identity.is_some());
            assert_eq!(
                Network::rpc_get_validator_by_coldkey(&account(100)).unwrap(),
                wire
            );
            assert_eq!(
                Network::rpc_get_validator_by_hotkey(&account(101)).unwrap(),
                wire
            );
        });
    }
}

#[test]
fn metadata_changes_do_not_rewrite_operational_state() {
    new_test_ext().execute_with(|| {
        let id = register(None);
        let core = ValidatorsData::<Test>::get(id).encode();
        for identity in [Some(maximum_identity()), Some(empty_identity()), None, None] {
            assert_ok!(Network::update_validator_identity(
                RuntimeOrigin::signed(account(100)),
                id,
                identity.clone()
            ));
            assert_eq!(ValidatorsData::<Test>::get(id).encode(), core);
            assert_eq!(ValidatorMetadata::<Test>::get(id), identity);
            assert_eq!(Network::get_validator_info(id).unwrap().identity, identity);
            System::assert_last_event(
                Event::<Test>::IdentityUpdated {
                    validator_id: id,
                    identity,
                }
                .into(),
            );
        }
    });
}

#[test]
fn rejected_metadata_calls_leave_storage_and_events_unchanged() {
    new_test_ext().execute_with(|| {
        let id = register(Some(maximum_identity()));
        for identity in [None, Some(empty_identity())] {
            assert_noop!(
                Network::update_validator_identity(
                    RuntimeOrigin::signed(account(102)),
                    id,
                    identity.clone()
                ),
                Error::<Test>::NotKeyOwner
            );
            assert_noop!(
                Network::update_validator_identity(
                    RuntimeOrigin::signed(account(100)),
                    id + 1,
                    identity.clone()
                ),
                Error::<Test>::InvalidValidatorId
            );
        }
        // A retained ownership key must not authorize metadata for a missing core record.
        ValidatorsData::<Test>::remove(id);
        assert_noop!(
            Network::update_validator_identity(RuntimeOrigin::signed(account(100)), id, None),
            Error::<Test>::InvalidValidatorId
        );
        assert_noop!(
            Network::update_validator_identity(
                RuntimeOrigin::signed(account(100)),
                id,
                Some(maximum_identity())
            ),
            Error::<Test>::InvalidValidatorId
        );
        assert!(Network::get_validator_info(id).is_none());
        assert!(Network::rpc_get_validator_info(id).is_none());
    });
}

#[test]
fn failed_registration_does_not_create_metadata() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_noop!(
            Network::register_validator(
                RuntimeOrigin::signed(account(100)),
                account(101),
                0,
                Some(DelegateAccount {
                    account_id: account(102),
                    rate: 0
                }),
                Some(maximum_identity()),
            ),
            Error::<Test>::InvalidDelegateAccountRate
        );
        assert!(!ValidatorMetadata::<Test>::contains_key(1));
        TotalValidatorIds::<Test>::put(u32::MAX);
        assert_noop!(
            Network::register_validator(
                RuntimeOrigin::signed(account(100)),
                account(101),
                0,
                None,
                Some(maximum_identity()),
            ),
            sp_runtime::ArithmeticError::Overflow
        );
    });
}

#[test]
fn key_and_reward_setting_changes_preserve_metadata() {
    new_test_ext().execute_with(|| {
        let identity = maximum_identity();
        let id = register(Some(identity.clone()));
        assert_ok!(Network::update_validator_hotkey(
            RuntimeOrigin::signed(account(100)),
            id,
            account(103)
        ));
        assert_ok!(Network::update_validator_coldkey(
            RuntimeOrigin::signed(account(100)),
            id,
            account(104)
        ));
        assert_ok!(Network::update_validator_delegate_account(
            RuntimeOrigin::signed(account(104)),
            id,
            Some(account(105)),
            Some(test_percent(1, 10))
        ));
        System::set_block_number(
            System::block_number() + NodeRewardRateUpdatePeriod::<Test>::get(),
        );
        assert_ok!(Network::update_validator_delegate_reward_rate(
            RuntimeOrigin::signed(account(104)),
            id,
            test_percent(1, 20)
        ));
        assert_eq!(ValidatorMetadata::<Test>::get(id), Some(identity));
        assert!(Network::rpc_get_validator_by_hotkey(&account(101)).is_none());
        assert!(Network::rpc_get_validator_by_coldkey(&account(100)).is_none());
        assert!(Network::rpc_get_validator_by_hotkey(&account(103))
            .unwrap()
            .identity
            .is_some());
        assert_noop!(
            Network::update_validator_identity(RuntimeOrigin::signed(account(100)), id, None),
            Error::<Test>::NotKeyOwner
        );
        assert_ok!(Network::update_validator_identity(
            RuntimeOrigin::signed(account(104)),
            id,
            None
        ));
    });
}

#[test]
fn node_removal_retains_validator_metadata_and_composed_readers_agree() {
    new_test_ext().execute_with(|| {
        build_activated_subnet(
            b"metadata-retention".to_vec(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(b"metadata-retention".to_vec()).unwrap();
        let node_id = 1;
        let validator_id = SubnetNodeValidatorId::<Test>::get(subnet_id, node_id).unwrap();
        let coldkey = ValidatorColdkey::<Test>::get(validator_id).unwrap();
        assert!(
            Network::get_subnet_node_associated_validator_info(subnet_id, node_id)
                .unwrap()
                .identity
                .is_none()
        );
        let identity = maximum_identity();
        assert_ok!(Network::update_validator_identity(
            RuntimeOrigin::signed(coldkey),
            validator_id,
            Some(identity.clone())
        ));
        assert_eq!(
            Network::get_subnet_node_associated_validator_info(subnet_id, node_id)
                .unwrap()
                .identity,
            Some(identity.clone())
        );
        Network::perform_remove_subnet_node(subnet_id, node_id);
        assert!(!SubnetNodesData::<Test>::contains_key(subnet_id, node_id));
        assert_eq!(
            ValidatorMetadata::<Test>::get(validator_id),
            Some(identity.clone())
        );
        assert_eq!(
            Network::get_validator_info(validator_id).unwrap().identity,
            Some(identity)
        );
        assert!(Network::rpc_get_validator_info(validator_id)
            .unwrap()
            .identity
            .is_some());
    });
}
