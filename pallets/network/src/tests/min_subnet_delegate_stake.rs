use super::mock::*;
use super::test_utils::{account, insert_subnet, set_epoch};
use crate::{
    AccountSubnetDelegateStakeShares, DelegateStakeSubnetRemovalInterval, Error, Event, MaxSubnets,
    MinSubnetDelegateStakeBalance, MinSubnetNodes, MinSubnetRegistrationEpochs,
    SubnetEnactmentEpochs, SubnetOwner, SubnetRegistrationEpoch, SubnetRegistrationEpochs,
    SubnetRemovalReason, SubnetReputation, SubnetSlot, SubnetState, SubnetsData,
    TotalAdmittedSubnetNodes, TotalSubnetDelegateStakeBalance, TotalSubnetElectableNodes,
    TotalValidatorDelegateStakeBalance, TxRateLimit,
};
use frame_support::{assert_noop, assert_ok, traits::Currency, weights::WeightMeter};

fn healthy_subnet(id: u32, state: SubnetState, start_epoch: u32, balance: u128) {
    insert_subnet(id, state, start_epoch);
    SubnetSlot::<Test>::insert(id, DesignatedEpochSlots::get().saturating_add(id));
    TotalSubnetDelegateStakeBalance::<Test>::insert(id, balance);
    SubnetReputation::<Test>::insert(id, Network::percentage_factor_as_u128());
    TotalAdmittedSubnetNodes::<Test>::insert(id, MinSubnetNodes::<Test>::get());
    TotalSubnetElectableNodes::<Test>::insert(id, MinSubnetNodes::<Test>::get());
}

fn assert_requirement(id: u32, minimum: u128, eligible: bool) {
    assert_eq!(
        Network::rpc_get_subnet_info(id)
            .unwrap()
            .current_min_delegate_stake
            .0,
        minimum
    );
    assert_eq!(
        Network::can_subnet_be_active(id),
        if eligible {
            (true, None)
        } else {
            (false, Some(SubnetRemovalReason::MinSubnetDelegateStake))
        }
    );
}

#[test]
fn activation_uses_base_boundary_in_registration_and_enactment() {
    for epoch in [1, 3] {
        for balance in [99, 100, 101] {
            new_test_ext().execute_with(|| {
                MinSubnetDelegateStakeBalance::<Test>::put(100);
                MinSubnetRegistrationEpochs::<Test>::put(1);
                SubnetRegistrationEpochs::<Test>::put(2);
                SubnetEnactmentEpochs::<Test>::put(2);
                healthy_subnet(1, SubnetState::Registered, 0, balance);
                SubnetRegistrationEpoch::<Test>::insert(1, 0);
                SubnetOwner::<Test>::insert(1, account(0));
                set_epoch(epoch, 0);

                if balance < 100 && epoch == 1 {
                    assert_noop!(
                        Network::activate_subnet(RuntimeOrigin::signed(account(0)), 1),
                        Error::<Test>::SubnetActivationConditionsNotMetYet
                    );
                    assert_eq!(
                        SubnetsData::<Test>::get(1).unwrap().state,
                        SubnetState::Registered
                    );
                } else {
                    assert_ok!(Network::activate_subnet(
                        RuntimeOrigin::signed(account(0)),
                        1
                    ));
                    if balance < 100 {
                        assert!(!SubnetsData::<Test>::contains_key(1));
                        assert_eq!(
                            network_events().last(),
                            Some(&Event::SubnetDeactivated {
                                subnet_id: 1,
                                reason: SubnetRemovalReason::MinSubnetDelegateStake,
                            })
                        );
                    } else {
                        assert_eq!(
                            SubnetsData::<Test>::get(1).unwrap().state,
                            SubnetState::Active
                        );
                        assert_eq!(
                            network_events().last(),
                            Some(&Event::SubnetActivated { subnet_id: 1 })
                        );
                    }
                }
            });
        }
    }
}

#[test]
fn requirement_is_independent_of_other_subnets_and_validator_delegation() {
    new_test_ext().execute_with(|| {
        set_epoch(10, EpochLength::get() - 1);
        MinSubnetDelegateStakeBalance::<Test>::put(100);
        healthy_subnet(1, SubnetState::Active, 0, 100);
        assert_requirement(1, 100, true);

        for state in [
            SubnetState::Registered,
            SubnetState::Paused,
            SubnetState::Active,
        ] {
            for start_epoch in [0, 100] {
                for balance in [0, 1, u128::MAX] {
                    healthy_subnet(2, state, start_epoch, balance);
                    TotalValidatorDelegateStakeBalance::<Test>::put(balance);
                    assert_requirement(1, 100, true);
                    assert_eq!(
                        Network::rpc_get_subnet_info(2)
                            .unwrap()
                            .current_min_delegate_stake
                            .0,
                        100
                    );
                    // Retained exit-pool balances are also irrelevant after subnet removal.
                    SubnetsData::<Test>::remove(2);
                    assert_requirement(1, 100, true);
                }
            }
        }
    });
}

#[test]
fn deposits_and_withdrawals_do_not_move_the_requirement() {
    new_test_ext().execute_with(|| {
        set_epoch(10, EpochLength::get() - 1);
        TxRateLimit::<Test>::put(0);
        MinSubnetDelegateStakeBalance::<Test>::put(100);
        healthy_subnet(1, SubnetState::Active, 0, 100);
        healthy_subnet(2, SubnetState::Active, 0, 0);
        let delegator = account(1000);
        let deposit = 1_000_000 * 1_000_000_000_000_000_000u128;
        let _ = Balances::deposit_creating(&delegator, deposit + 500);
        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(delegator.clone()),
            2,
            deposit,
            1,
        ));
        assert_requirement(1, 100, true);
        assert_requirement(2, 100, true);

        let shares = AccountSubnetDelegateStakeShares::<Test>::get(&delegator, 2);
        assert_ok!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(delegator),
            2,
            shares,
            1,
        ));
        assert_requirement(1, 100, true);
        assert_requirement(2, 100, false);
        // Withdrawing below the base remains allowed; maintenance handles survival later.
        assert!(SubnetsData::<Test>::contains_key(2));
    });
}

#[test]
fn collective_updates_apply_to_rpc_and_checks_without_changing_removal_timing() {
    new_test_ext().execute_with(|| {
        set_epoch(9, EpochLength::get() - 1);
        DelegateStakeSubnetRemovalInterval::<Test>::put(2);
        MinSubnetDelegateStakeBalance::<Test>::put(100);
        healthy_subnet(1, SubnetState::Active, 0, 100);
        assert_noop!(
            Network::set_min_subnet_delegate_stake_balance(RuntimeOrigin::signed(account(0)), 101),
            sp_runtime::DispatchError::BadOrigin
        );
        assert_noop!(
            Network::set_min_subnet_delegate_stake_balance(
                RuntimeOrigin::from(pallet_collective::RawOrigin::Members(1, 5)),
                101,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
        for (minimum, eligible) in [(101, false), (99, true), (100, true), (101, false)] {
            assert_ok!(Network::set_min_subnet_delegate_stake_balance(
                RuntimeOrigin::from(pallet_collective::RawOrigin::Members(5, 5)),
                minimum,
            ));
            assert_eq!(
                network_events().last(),
                Some(&Event::SetMinSubnetDelegateStakeBalance(minimum))
            );
            assert_requirement(1, minimum, eligible);
        }
        Network::do_epoch_preliminaries(&mut WeightMeter::new(), System::block_number(), 9);
        assert!(SubnetsData::<Test>::contains_key(1));
        set_epoch(10, EpochLength::get() - 1);
        Network::do_epoch_preliminaries(&mut WeightMeter::new(), System::block_number(), 10);
        assert!(!SubnetsData::<Test>::contains_key(1));
    });
}

#[test]
fn removal_preserves_exemptions_cadence_and_inclusive_boundary() {
    new_test_ext().execute_with(|| {
        set_epoch(10, EpochLength::get() - 1);
        MaxSubnets::<Test>::put(10);
        MinSubnetDelegateStakeBalance::<Test>::put(100);
        for (id, balance) in [(1, 0), (2, 99), (3, 100), (4, 101)] {
            healthy_subnet(id, SubnetState::Active, 0, balance);
        }
        healthy_subnet(5, SubnetState::Registered, 0, 0);
        SubnetRegistrationEpoch::<Test>::insert(5, 10);
        healthy_subnet(6, SubnetState::Paused, 10, 0);
        healthy_subnet(7, SubnetState::Active, 100, 0);
        // Corrupt zero interval must not trap the hook or trigger funding removal.
        DelegateStakeSubnetRemovalInterval::<Test>::put(0);
        Network::do_epoch_preliminaries(&mut WeightMeter::new(), System::block_number(), 10);
        assert!((1..=7).all(SubnetsData::<Test>::contains_key));
        DelegateStakeSubnetRemovalInterval::<Test>::put(2);
        Network::do_epoch_preliminaries(&mut WeightMeter::new(), System::block_number(), 10);
        assert!(!SubnetsData::<Test>::contains_key(1));
        assert!(!SubnetsData::<Test>::contains_key(2));
        assert!((3..=7).all(SubnetsData::<Test>::contains_key));
        for id in [1, 2] {
            assert!(network_events().contains(&Event::SubnetDeactivated {
                subnet_id: id,
                reason: SubnetRemovalReason::MinSubnetDelegateStake,
            }));
        }
    });
}

#[test]
fn configured_zero_and_maximum_keep_their_existing_meaning() {
    for minimum in [0, u128::MAX] {
        new_test_ext().execute_with(|| {
            set_epoch(10, EpochLength::get() - 1);
            assert_ok!(Network::set_min_subnet_delegate_stake_balance(
                RuntimeOrigin::from(pallet_collective::RawOrigin::Members(5, 5)),
                minimum,
            ));
            healthy_subnet(1, SubnetState::Active, 0, minimum);
            assert_requirement(1, minimum, true);
            if minimum > 0 {
                healthy_subnet(2, SubnetState::Active, 0, minimum - 1);
                assert_requirement(2, minimum, false);
            }
            Network::do_epoch_preliminaries(&mut WeightMeter::new(), System::block_number(), 10);
            assert!(SubnetsData::<Test>::contains_key(1));
            assert!(!SubnetsData::<Test>::contains_key(2));
        });
    }
}
