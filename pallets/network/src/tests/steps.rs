use super::mock::*;
use crate::tests::test_utils::*;
use crate::{
    DelegateStakeSubnetRemovalInterval, Event, MaxSubnets, MinDelegateStakeDeposit, MinSubnetNodes,
    MinSubnetReputation, NewRegistrationCostMultiplier, PrevSubnetActivationEpoch,
    SubnetEnactmentEpochs, SubnetName, SubnetOwner, SubnetRegistrationEpoch,
    SubnetRegistrationEpochs, SubnetRemovalActivationCooldown, SubnetRemovalCheckInterval,
    SubnetRemovalReason, SubnetReputation, SubnetState, SubnetsData, TotalAdmittedSubnetNodes,
    TotalSubnetDelegateStakeBalance, NETWORK_EPOCH_PRELIMINARIES_SLOT,
};
use frame_support::assert_ok;
use frame_support::traits::{Currency, OnInitialize};
use frame_support::weights::WeightMeter;

fn top_up_subnet_delegate_stake(subnet_id: u32, margin: u128) {
    let current = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
    let required = crate::MinSubnetDelegateStakeBalance::<Test>::get().saturating_add(margin);
    if current < required {
        let amount = required
            .saturating_sub(current)
            .max(MinDelegateStakeDeposit::<Test>::get());
        if Balances::free_balance(account(0)) <= amount {
            let _ = Balances::deposit_creating(&account(0), amount.saturating_add(500));
        }
        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(account(0)),
            subnet_id,
            amount,
            1,
        ));
    }

    assert!(
        TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id)
            >= crate::MinSubnetDelegateStakeBalance::<Test>::get().saturating_add(margin)
    );
}

#[test]
fn test_excess_subnet_removal_schedule_combines_cooldown_and_cadence() {
    let activation_epoch = 7;
    let activation_cooldown_epochs = 10;
    let check_interval_epochs = 10;

    assert!(!Network::can_remove_excess_subnet_at_epoch(
        10,
        activation_epoch,
        activation_cooldown_epochs,
        check_interval_epochs,
    ));
    assert!(!Network::can_remove_excess_subnet_at_epoch(
        19,
        activation_epoch,
        activation_cooldown_epochs,
        check_interval_epochs,
    ));
    assert!(Network::can_remove_excess_subnet_at_epoch(
        20,
        activation_epoch,
        activation_cooldown_epochs,
        check_interval_epochs,
    ));
    assert!(!Network::can_remove_excess_subnet_at_epoch(
        20,
        activation_epoch,
        activation_cooldown_epochs,
        0,
    ));

    assert!(!Network::can_remove_excess_subnet_at_epoch(
        u32::MAX - 1,
        u32::MAX - 5,
        10,
        1,
    ));
    assert!(Network::can_remove_excess_subnet_at_epoch(
        u32::MAX,
        u32::MAX - 5,
        10,
        1,
    ));
}

#[test]
fn test_do_epoch_preliminaries_waits_for_first_local_consensus_slot() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "phase-aware-health-subnet".into();
        let deposit_amount = 10_000_000_000_000_000_000_000u128;
        let stake_amount = 1_000_000_000_000_000_000_000u128;

        build_activated_subnet(
            subnet_name.clone(),
            0,
            MinSubnetNodes::<Test>::get(),
            deposit_amount,
            stake_amount,
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let consensus_eligible_from_subnet_epoch = SubnetsData::<Test>::get(subnet_id)
            .unwrap()
            .consensus_eligible_from_subnet_epoch
            .unwrap();

        SubnetReputation::<Test>::insert(
            subnet_id,
            MinSubnetReputation::<Test>::get().saturating_sub(1),
        );
        TotalSubnetDelegateStakeBalance::<Test>::insert(subnet_id, u128::MAX);

        // At global slot zero the subnet is still in the preceding local epoch, so ordinary
        // health enforcement must not run before its first consensus opportunity.
        set_epoch(
            consensus_eligible_from_subnet_epoch,
            NETWORK_EPOCH_PRELIMINARIES_SLOT,
        );
        let preparation_boundary = System::block_number();
        Network::on_initialize(preparation_boundary);
        assert!(SubnetsData::<Test>::contains_key(subnet_id));

        // At the following global boundary the target subnet slot has passed. Normal health
        // enforcement resumes and observes the already-low reputation.
        let enforcement_epoch = consensus_eligible_from_subnet_epoch.saturating_add(1);
        set_epoch(enforcement_epoch, NETWORK_EPOCH_PRELIMINARIES_SLOT);
        Network::on_initialize(System::block_number());
        assert_eq!(SubnetsData::<Test>::try_get(subnet_id), Err(()));
    });
}

#[test]
fn test_do_epoch_preliminaries_remove_under_min_delegate_stake() {
    new_test_ext().execute_with(|| {
        NewRegistrationCostMultiplier::<Test>::put(Network::percentage_factor_as_u128());

        let dstake_epoch_interval = DelegateStakeSubnetRemovalInterval::<Test>::get();

        let deposit_amount: u128 = 1000000000000000000000;
        let amount: u128 = 100000000000000000000;

        let max_subnets = MaxSubnets::<Test>::get();
        let end = MinSubnetNodes::<Test>::get();
        let last_subnet = 2;

        let mut remove_subnet_id = 0;
        let mut first_subnet_id = 0;
        for s in 0..last_subnet {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            build_activated_subnet(subnet_name.clone().into(), 0, end, deposit_amount, amount);
            let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
            remove_subnet_id = subnet_id;

            if first_subnet_id == 0 {
                first_subnet_id = subnet_id;
            }
        }

        // increase past n+1 epoch so they are on their activation epochs
        let current_epoch = Network::get_current_epoch_as_u32();
        increase_epochs(current_epoch % dstake_epoch_interval + dstake_epoch_interval);

        // ensure first subnet has enough dstake
        top_up_subnet_delegate_stake(first_subnet_id, 0);

        // ensure removal subnet has less than minimum dstake
        let min_subnet_delegate_stake = crate::MinSubnetDelegateStakeBalance::<Test>::get();
        TotalSubnetDelegateStakeBalance::<Test>::insert(
            remove_subnet_id,
            min_subnet_delegate_stake - 1,
        );

        let current_epoch = Network::get_current_epoch_as_u32();
        Network::do_epoch_preliminaries(
            &mut WeightMeter::new(),
            System::block_number(),
            current_epoch,
        );

        assert_eq!(SubnetsData::<Test>::try_get(remove_subnet_id), Err(()));
        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetDeactivated {
                subnet_id: remove_subnet_id,
                reason: SubnetRemovalReason::MinSubnetDelegateStake
            }
        );
    });
}

#[test]
fn test_do_epoch_preliminaries_remove_under_min_delegate_stake_fail() {
    new_test_ext().execute_with(|| {
        NewRegistrationCostMultiplier::<Test>::put(Network::percentage_factor_as_u128());

        let dstake_epoch_interval = DelegateStakeSubnetRemovalInterval::<Test>::get();

        let deposit_amount: u128 = 1000000000000000000000;
        let amount: u128 = 100000000000000000000;

        let max_subnets = MaxSubnets::<Test>::get();
        let end = MinSubnetNodes::<Test>::get();
        let last_subnet = 2;

        let mut remove_subnet_id = 0;
        let mut first_subnet_id = 0;
        for s in 0..last_subnet {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            build_activated_subnet(subnet_name.clone().into(), 0, end, deposit_amount, amount);
            let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
            remove_subnet_id = subnet_id;

            if first_subnet_id == 0 {
                first_subnet_id = subnet_id;
            }
        }

        // increase past n+1 epoch so they are on their activation epochs
        let current_epoch = Network::get_current_epoch_as_u32();
        increase_epochs(current_epoch % dstake_epoch_interval + dstake_epoch_interval - 1);

        // ensure first subnet has enough dstake
        top_up_subnet_delegate_stake(first_subnet_id, 0);

        // ensure removal subnet has less than minimum dstake
        let min_subnet_delegate_stake = crate::MinSubnetDelegateStakeBalance::<Test>::get();
        TotalSubnetDelegateStakeBalance::<Test>::insert(
            remove_subnet_id,
            min_subnet_delegate_stake - 1,
        );

        let current_epoch = Network::get_current_epoch_as_u32();
        Network::do_epoch_preliminaries(
            &mut WeightMeter::new(),
            System::block_number(),
            current_epoch,
        );

        assert!(SubnetsData::<Test>::contains_key(remove_subnet_id));
    });
}

#[test]
fn test_do_epoch_preliminaries_remove_under_min_reputation() {
    new_test_ext().execute_with(|| {
        NewRegistrationCostMultiplier::<Test>::put(Network::percentage_factor_as_u128());

        let deposit_amount: u128 = 1000000000000000000000;
        let amount: u128 = 100000000000000000000;

        let max_subnets = MaxSubnets::<Test>::get();
        let end = MinSubnetNodes::<Test>::get();
        let last_subnet = 2;

        let mut remove_subnet_id = 0;
        let mut first_subnet_id = 0;
        for s in 0..last_subnet {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            build_activated_subnet(subnet_name.clone().into(), 0, end, deposit_amount, amount);
            let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
            remove_subnet_id = subnet_id;

            if first_subnet_id == 0 {
                first_subnet_id = subnet_id;
            }
        }

        // increase past n+1 epoch so they are on their activation epochs
        increase_epochs(1);

        // ensure subnets have enough dstake
        for s in 0..last_subnet {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
            top_up_subnet_delegate_stake(subnet_id, 0);
        }

        // ensure removal subnet has n-1 min reputation
        let min_rep = MinSubnetReputation::<Test>::get();
        SubnetReputation::<Test>::insert(remove_subnet_id, min_rep - 1);

        // Health enforcement resumes at the global boundary after the first local consensus
        // slot, not at slot zero of the consensus-eligibility global epoch.
        let current_epoch = SubnetsData::<Test>::get(remove_subnet_id)
            .unwrap()
            .consensus_eligible_from_subnet_epoch
            .unwrap()
            .saturating_add(1);
        set_epoch(current_epoch, NETWORK_EPOCH_PRELIMINARIES_SLOT);
        Network::do_epoch_preliminaries(
            &mut WeightMeter::new(),
            System::block_number(),
            current_epoch,
        );

        assert_eq!(SubnetsData::<Test>::try_get(remove_subnet_id), Err(()));
        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetDeactivated {
                subnet_id: remove_subnet_id,
                reason: SubnetRemovalReason::MinReputation
            }
        );
    });
}

#[test]
fn test_do_epoch_preliminaries_remove_max_subnets() {
    new_test_ext().execute_with(|| {
        NewRegistrationCostMultiplier::<Test>::put(Network::percentage_factor_as_u128());

        let deposit_amount: u128 = 1000000000000000000000;
        let amount: u128 = 100000000000000000000;

        let max_subnets = MaxSubnets::<Test>::get();
        let end = MinSubnetNodes::<Test>::get();

        let mut subnet_ids = Vec::new();
        for s in 0..max_subnets.saturating_add(1) {
            let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
            build_activated_subnet_new_excess_subnets(
                subnet_name.clone().into(),
                0,
                end,
                deposit_amount,
                amount,
                1,
            );
            let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
            subnet_ids.push(subnet_id);
        }

        // Each newly built subnet adds to TVL, so later subnets no longer begin with the same
        // delegate balance. Select the actual weakest subnet, then keep every other subnet above
        // it while exercising the excess-capacity schedule.
        let remove_subnet_id = *subnet_ids
            .iter()
            .min_by_key(|subnet_id| TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id))
            .unwrap();

        // increase past n+1 epoch so they are on their activation epochs
        increase_epochs(1);

        let removal_interval = SubnetRemovalCheckInterval::<Test>::get();
        let current_epoch = Network::get_current_epoch_as_u32();

        // Set on an epoch where a subnet should not be removed
        let offset_epoch = removal_interval - current_epoch % removal_interval;

        set_epoch(
            current_epoch + offset_epoch + 1,
            NETWORK_EPOCH_PRELIMINARIES_SLOT,
        );
        let mut non_removal_epoch_touched = false;

        let current_epoch = Network::get_current_epoch_as_u32();

        for e in 0..removal_interval {
            let current_epoch = Network::get_current_epoch_as_u32();
            let is_removal_epoch: bool = current_epoch % removal_interval == 0;

            if !is_removal_epoch {
                // always ensure subnet is at the minimum required delegate stake
                for _ in 0..16 {
                    for s in 0..max_subnets.saturating_add(1) {
                        let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();

                        // This will revert if subnet doesn't exist so we know the removal subnet is still alive
                        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
                        let margin = if subnet_id == remove_subnet_id {
                            0
                        } else {
                            MinDelegateStakeDeposit::<Test>::get().saturating_mul(10)
                        };
                        top_up_subnet_delegate_stake(subnet_id, margin);
                    }

                    let all_funded = (0..max_subnets.saturating_add(1)).all(|s| {
                        let subnet_name: Vec<u8> = format!("subnet-name-{s}").into();
                        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
                        let margin = if subnet_id == remove_subnet_id {
                            0
                        } else {
                            MinDelegateStakeDeposit::<Test>::get().saturating_mul(10)
                        };
                        TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id)
                            >= crate::MinSubnetDelegateStakeBalance::<Test>::get()
                                .saturating_add(margin)
                    });
                    if all_funded {
                        break;
                    }
                }
            }
            Network::do_epoch_preliminaries(
                &mut WeightMeter::new(),
                System::block_number(),
                current_epoch,
            );
            if !is_removal_epoch {
                non_removal_epoch_touched = true;
                assert_eq!(
                    SubnetsData::<Test>::get(remove_subnet_id).unwrap().id,
                    remove_subnet_id
                );
            }
            set_epoch(current_epoch + 1, NETWORK_EPOCH_PRELIMINARIES_SLOT);
        }
        assert!(non_removal_epoch_touched);
        assert_eq!(SubnetsData::<Test>::try_get(remove_subnet_id), Err(()));
        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetDeactivated {
                subnet_id: remove_subnet_id,
                reason: SubnetRemovalReason::MaxSubnets
            }
        );
    });
}

#[test]
fn test_do_epoch_preliminaries_remove_registered_min_nodes() {
    new_test_ext().execute_with(|| {
        NewRegistrationCostMultiplier::<Test>::put(Network::percentage_factor_as_u128());

        let subnet_registration_epochs = SubnetRegistrationEpochs::<Test>::get();
        let subnet_enactment_epochs = SubnetEnactmentEpochs::<Test>::get();

        let deposit_amount: u128 = 1000000000000000000000;
        let amount: u128 = 100000000000000000000;

        let max_subnets = MaxSubnets::<Test>::get();
        let end = MinSubnetNodes::<Test>::get();

        let subnet_name: Vec<u8> = format!("subnet-name-remove").into();
        build_registered_subnet(
            subnet_name.clone(),
            0,
            end,
            deposit_amount,
            amount,
            false,
            None,
        );

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let max_enactment_epoch = SubnetsData::<Test>::get(subnet_id);

        let max_registration_epoch = SubnetRegistrationEpoch::<Test>::get(subnet_id)
            .unwrap()
            .saturating_add(subnet_registration_epochs);
        let max_enactment_epoch = max_registration_epoch.saturating_add(subnet_enactment_epochs);

        // Force set node count below minimum
        TotalAdmittedSubnetNodes::<Test>::insert(subnet_id, MinSubnetNodes::<Test>::get() - 1);

        // push into enactment period
        set_epoch(max_enactment_epoch, NETWORK_EPOCH_PRELIMINARIES_SLOT);

        let current_epoch = Network::get_current_epoch_as_u32();

        Network::do_epoch_preliminaries(
            &mut WeightMeter::new(),
            System::block_number(),
            current_epoch,
        );

        assert_eq!(SubnetsData::<Test>::try_get(subnet_id), Err(()));
        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetDeactivated {
                subnet_id,
                reason: SubnetRemovalReason::MinSubnetNodes
            }
        );
    });
}

#[test]
fn test_do_epoch_preliminaries_remove_past_enactment_phase() {
    new_test_ext().execute_with(|| {
        NewRegistrationCostMultiplier::<Test>::put(Network::percentage_factor_as_u128());

        let subnet_registration_epochs = SubnetRegistrationEpochs::<Test>::get();
        let subnet_enactment_epochs = SubnetEnactmentEpochs::<Test>::get();

        let deposit_amount: u128 = 1000000000000000000000;
        let amount: u128 = 100000000000000000000;

        let max_subnets = MaxSubnets::<Test>::get();
        let end = MinSubnetNodes::<Test>::get();

        let subnet_name: Vec<u8> = format!("subnet-name-remove").into();
        build_registered_subnet(
            subnet_name.clone(),
            0,
            end,
            deposit_amount,
            amount,
            false,
            None,
        );

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let max_enactment_epoch = SubnetsData::<Test>::get(subnet_id);

        let max_registration_epoch = SubnetRegistrationEpoch::<Test>::get(subnet_id)
            .unwrap()
            .saturating_add(subnet_registration_epochs);
        let max_enactment_epoch = max_registration_epoch.saturating_add(subnet_enactment_epochs);

        // push into enactment period
        set_epoch(max_enactment_epoch + 1, NETWORK_EPOCH_PRELIMINARIES_SLOT);

        let current_epoch = Network::get_current_epoch_as_u32();

        Network::do_epoch_preliminaries(
            &mut WeightMeter::new(),
            System::block_number(),
            current_epoch,
        );

        assert_eq!(SubnetsData::<Test>::try_get(subnet_id), Err(()));
        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetDeactivated {
                subnet_id,
                reason: SubnetRemovalReason::EnactmentPeriod
            }
        );
    });
}
