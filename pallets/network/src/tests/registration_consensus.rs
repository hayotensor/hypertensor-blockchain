use super::{mock::*, test_utils::*};
use crate::*;
use codec::{Decode, Encode};
use frame_support::{assert_noop, assert_ok, traits::Currency};

fn data(mechanism: ConsensusMechanism) -> RegistrationSubnetData<Test> {
    let mut data = default_registration_subnet_data(
        0,
        MaxSubnetNodes::<Test>::get(),
        b"selection".to_vec(),
        0,
        MinSubnetNodes::<Test>::get(),
    );
    data.consensus_mechanism = mechanism;
    data
}

#[test]
fn registration_persists_explicit_choice_through_call_and_helper() {
    for mechanism in [ConsensusMechanism::Attestation, ConsensusMechanism::Matrix] {
        for direct in [false, true] {
            new_test_ext().execute_with(|| {
                increase_epochs(50);
                RequireSubnetRegistrationWhitelist::<Test>::put(false);
                let owner = account(777);
                drop(Balances::deposit_creating(
                    &owner,
                    1_000_000 * PERCENTAGE_FACTOR_U128,
                ));
                let input = data(mechanism);
                let id = TotalSubnetUids::<Test>::get() + 1;
                if direct {
                    assert_ok!(Network::do_register_subnet(owner.clone(), u128::MAX, input));
                } else {
                    assert_ok!(Network::register_subnet(
                        RuntimeOrigin::signed(owner.clone()),
                        u128::MAX,
                        input
                    ));
                }
                let subnet = SubnetsData::<Test>::get(id).unwrap();
                assert_eq!(subnet.consensus_mechanism, mechanism);
                assert_eq!(subnet.state, SubnetState::Registered);
                assert_eq!(SubnetOwner::<Test>::get(id), Some(owner));
                assert!(!MatrixRounds::<Test>::contains_key(id, 0));
                assert_eq!(
                    MatrixConfig::<Test>::get().accuracy_weight,
                    PERCENTAGE_FACTOR_U128 / 2
                );
            });
        }
    }
}

#[test]
fn invalid_matrix_registration_has_no_storage_or_balance_effects() {
    for direct in [false, true] {
        for invalid_epoch in [false, true] {
            new_test_ext().execute_with(|| {
                increase_epochs(50);
                RequireSubnetRegistrationWhitelist::<Test>::put(false);
                let owner = account(777);
                drop(Balances::deposit_creating(
                    &owner,
                    1_000_000 * PERCENTAGE_FACTOR_U128,
                ));
                let input = data(ConsensusMechanism::Matrix);
                let old_epoch = NetworkEpochLength::get();
                let error = if invalid_epoch {
                    NetworkEpochLength::set(1);
                    Error::<Test>::InvalidMatrixEpochLength
                } else {
                    MinSubnetNodes::<Test>::put(MATRIX_MAX_NODES + 1);
                    Error::<Test>::MaxSubnetNodes
                };
                // assert_noop compares all storage, including balances, treasury, IDs and slots.
                if direct {
                    assert_noop!(Network::do_register_subnet(owner, u128::MAX, input), error);
                } else {
                    assert_noop!(
                        Network::register_subnet(RuntimeOrigin::signed(owner), u128::MAX, input),
                        error
                    );
                }
                NetworkEpochLength::set(old_epoch);
            });
        }
    }
}

#[test]
fn matrix_registration_requires_three_node_slots_without_an_identity_minimum() {
    for slots in [0, 1, 2, 3] {
        new_test_ext().execute_with(|| {
            increase_epochs(50);
            RequireSubnetRegistrationWhitelist::<Test>::put(false);
            MinSubnetNodes::<Test>::put(1);
            let owner = account(777);
            drop(Balances::deposit_creating(
                &owner,
                1_000_000 * PERCENTAGE_FACTOR_U128,
            ));
            let mut input = data(ConsensusMechanism::Matrix);
            input.initial_validators = BTreeMap::from([(1, slots)]);
            if slots < 3 {
                assert_noop!(
                    Network::register_subnet(RuntimeOrigin::signed(owner), u128::MAX, input),
                    Error::<Test>::InvalidSubnetRegistrationInitialColdkeys
                );
            } else {
                assert_ok!(Network::register_subnet(
                    RuntimeOrigin::signed(owner),
                    u128::MAX,
                    input
                ));
            }
        });
    }
}

#[test]
fn registration_scale_requires_a_valid_mechanism() {
    new_test_ext().execute_with(|| {
        let input = data(ConsensusMechanism::Matrix);
        let mut encoded = input.encode();
        assert_eq!(
            RegistrationSubnetData::<Test>::decode(&mut &encoded[..]).unwrap(),
            input
        );
        assert_eq!(encoded.pop(), Some(1));
        assert!(RegistrationSubnetData::<Test>::decode(&mut &encoded[..]).is_err());
        encoded.push(2);
        assert!(RegistrationSubnetData::<Test>::decode(&mut &encoded[..]).is_err());
    });
}
