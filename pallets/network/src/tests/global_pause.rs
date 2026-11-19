use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{assert_noop, assert_ok, traits::Hooks, weights::WeightMeter};
use sp_std::collections::btree_map::BTreeMap;

const TOKEN: u128 = 1_000_000_000_000_000_000;
const NODES: u32 = 4;

fn registered_subnet(subnet: u32, registration_epoch: u32) {
    MinSubnetRegistrationEpochs::<Test>::put(2);
    SubnetRegistrationEpochs::<Test>::put(4);
    SubnetEnactmentEpochs::<Test>::put(3);
    insert_subnet(subnet, SubnetState::Registered, registration_epoch);
    SubnetRegistrationEpoch::<Test>::insert(subnet, registration_epoch);
    SubnetOwner::<Test>::insert(subnet, account(0));
    SubnetReputation::<Test>::insert(subnet, TOKEN);
    TotalAdmittedSubnetNodes::<Test>::insert(subnet, MinSubnetNodes::<Test>::get());
    TotalSubnetDelegateStakeBalance::<Test>::insert(
        subnet,
        MinSubnetDelegateStakeBalance::<Test>::get(),
    );
}

#[test]
fn resume_preserves_registration_and_enactment_activation_windows() {
    // The extra grace epoch also moves the minimum activation time forward by one.
    for elapsed_epochs in [3, 4, 5, 7] {
        new_test_ext().execute_with(|| {
            registered_subnet(1, 10);
            set_epoch(10 + elapsed_epochs, 0);
            resume_after(20 * EpochLength::get());

            assert_ok!(Network::activate_subnet(
                RuntimeOrigin::signed(account(0)),
                1
            ));
            assert_eq!(
                SubnetsData::<Test>::get(1).unwrap().state,
                SubnetState::Active
            );
            assert!(!SubnetRegistrationEpoch::<Test>::contains_key(1));
            assert_eq!(
                network_events().last(),
                Some(&Event::SubnetActivated { subnet_id: 1 })
            );
        });
    }
}

#[test]
fn resume_preserves_minimum_activation_wait() {
    new_test_ext().execute_with(|| {
        registered_subnet(1, 10);
        set_epoch(11, 0);
        resume_after(20 * EpochLength::get());
        assert_noop!(
            Network::activate_subnet(RuntimeOrigin::signed(account(0)), 1),
            Error::<Test>::MinSubnetRegistrationEpochsNotMet
        );
        set_epoch(32, 0);
        assert_noop!(
            Network::activate_subnet(RuntimeOrigin::signed(account(0)), 1),
            Error::<Test>::MinSubnetRegistrationEpochsNotMet
        );
        set_epoch(33, 0);
        assert_ok!(Network::activate_subnet(
            RuntimeOrigin::signed(account(0)),
            1
        ));
        assert_eq!(
            SubnetsData::<Test>::get(1).unwrap().state,
            SubnetState::Active
        );
    });
}

#[test]
fn resume_preserves_registration_through_hooks_and_expires_at_shifted_deadline() {
    for remove_via_activation in [false, true] {
        new_test_ext().execute_with(|| {
            registered_subnet(1, 10);
            // No nodes or funding yet: premature enactment would remove this subnet.
            TotalAdmittedSubnetNodes::<Test>::remove(1);
            TotalSubnetDelegateStakeBalance::<Test>::remove(1);
            set_epoch(12, 0);
            resume_after(20 * EpochLength::get());
            set_epoch(33, NETWORK_EPOCH_PRELIMINARIES_SLOT);
            Network::on_initialize(System::block_number());
            assert!(SubnetsData::<Test>::contains_key(1));
            assert!(Network::is_subnet_registering(
                1,
                SubnetState::Registered,
                33
            ));
            assert_noop!(
                Network::activate_subnet(RuntimeOrigin::signed(account(0)), 1),
                Error::<Test>::SubnetActivationConditionsNotMetYet
            );

            // Enactment allows time to fund delegation once the nodes are present.
            TotalAdmittedSubnetNodes::<Test>::insert(1, MinSubnetNodes::<Test>::get());
            set_epoch(38, NETWORK_EPOCH_PRELIMINARIES_SLOT);
            Network::on_initialize(System::block_number());
            assert!(Network::is_subnet_in_enactment(
                1,
                SubnetState::Registered,
                38
            ));
            assert!(!Network::is_subnet_registering(
                1,
                SubnetState::Registered,
                38
            ));
            assert_noop!(
                Network::activate_subnet(RuntimeOrigin::signed(account(0)), 1),
                Error::<Test>::SubnetActivationConditionsNotMetYet
            );

            set_epoch(39, NETWORK_EPOCH_PRELIMINARIES_SLOT);
            if remove_via_activation {
                assert_ok!(Network::activate_subnet(
                    RuntimeOrigin::signed(account(0)),
                    1
                ));
            } else {
                Network::on_initialize(System::block_number());
            }
            assert!(!SubnetsData::<Test>::contains_key(1));
            assert!(!SubnetRegistrationEpoch::<Test>::contains_key(1));
            assert!(network_events().contains(&Event::SubnetDeactivated {
                subnet_id: 1,
                reason: SubnetRemovalReason::EnactmentPeriod,
            }));
        });
    }
}

#[test]
fn resume_preserves_the_last_partial_registration_and_enactment_epoch() {
    for pause_epoch in [14, 17] {
        new_test_ext().execute_with(|| {
            registered_subnet(1, 10);
            if pause_epoch == 14 {
                // An unfinished registration must not enter enactment and be removed.
                TotalAdmittedSubnetNodes::<Test>::remove(1);
            }
            set_epoch(pause_epoch, 0);
            resume_after(EpochLength::get() - 1);
            assert_eq!(SubnetRegistrationEpoch::<Test>::get(1), Some(11));

            // The first block after resume crosses the old phase deadline.
            set_epoch(pause_epoch + 1, NETWORK_EPOCH_PRELIMINARIES_SLOT);
            Network::on_initialize(System::block_number());
            assert!(SubnetsData::<Test>::contains_key(1));
            if pause_epoch == 14 {
                assert!(Network::is_subnet_registering(
                    1,
                    SubnetState::Registered,
                    15
                ));
                assert_noop!(
                    Network::activate_subnet(RuntimeOrigin::signed(account(0)), 1),
                    Error::<Test>::SubnetActivationConditionsNotMetYet
                );
            } else {
                assert!(Network::is_subnet_in_enactment(
                    1,
                    SubnetState::Registered,
                    18
                ));
                assert_ok!(Network::activate_subnet(
                    RuntimeOrigin::signed(account(0)),
                    1
                ));
                assert_eq!(
                    SubnetsData::<Test>::get(1).unwrap().state,
                    SubnetState::Active
                );
            }
        });
    }
}

#[test]
fn resume_grants_the_extra_epoch_to_a_recently_expired_registration() {
    new_test_ext().execute_with(|| {
        registered_subnet(1, 10);
        set_epoch(18, 0);
        resume_after(20 * EpochLength::get());
        assert_eq!(SubnetRegistrationEpoch::<Test>::get(1), Some(31));
        assert_ok!(Network::activate_subnet(
            RuntimeOrigin::signed(account(0)),
            1
        ));
        assert_eq!(
            SubnetsData::<Test>::get(1).unwrap().state,
            SubnetState::Active
        );
    });
}

#[test]
fn resume_does_not_revive_registration_beyond_the_extra_grace_epoch() {
    new_test_ext().execute_with(|| {
        registered_subnet(1, 10);
        set_epoch(19, 0);
        resume_after(20 * EpochLength::get());
        assert_ok!(Network::activate_subnet(
            RuntimeOrigin::signed(account(0)),
            1
        ));
        assert!(!SubnetsData::<Test>::contains_key(1));
        assert_eq!(
            network_events().last(),
            Some(&Event::SubnetDeactivated {
                subnet_id: 1,
                reason: SubnetRemovalReason::EnactmentPeriod,
            })
        );
    });
}

#[test]
fn resume_always_adds_an_extra_epoch_to_pending_registration_windows() {
    let length = EpochLength::get();
    for (offset, duration, extension) in [
        (0, 0, 1),
        (0, 1, 1),
        (0, length - 1, 1),
        (length - 1, 1, 2),
        (0, length, 2),
        (0, 2 * length + 1, 3),
    ] {
        new_test_ext().execute_with(|| {
            registered_subnet(1, 10);
            // The extra epoch reopens registration at the first enactment epoch.
            PrevSubnetActivationEpoch::<Test>::put(12);
            set_epoch(15, offset);
            resume_after(duration);
            let epoch = Network::get_current_epoch_as_u32();
            assert_eq!(
                SubnetRegistrationEpoch::<Test>::get(1),
                Some(10 + extension)
            );
            assert_eq!(PrevSubnetActivationEpoch::<Test>::get(), 12 + extension);
            assert!(Network::is_subnet_registering(
                1,
                SubnetState::Registered,
                epoch
            ));
            assert!(!Network::is_subnet_in_enactment(
                1,
                SubnetState::Registered,
                epoch
            ));
            assert_ok!(Network::activate_subnet(
                RuntimeOrigin::signed(account(0)),
                1
            ));
            assert!(SubnetsData::<Test>::contains_key(1));
        });
    }
}

#[test]
fn repeated_pauses_shift_all_pending_anchors_once_and_leave_active_subnets_unchanged() {
    new_test_ext().execute_with(|| {
        // Exercise the full physical bound, reserving one entry for an active subnet.
        let active_id = NetworkMaxPhysicalSubnetsUpperBound::get();
        for subnet_id in 1..active_id {
            registered_subnet(subnet_id, 10 + subnet_id % 2);
        }
        insert_subnet(active_id, SubnetState::Active, 8);
        PrevSubnetActivationEpoch::<Test>::put(8);
        let active = SubnetsData::<Test>::get(active_id);
        set_epoch(12, 0);
        assert_ok!(Network::do_pause());
        set_epoch(20, 0);
        assert_ok!(Network::do_pause());
        set_epoch(32, 0);
        assert_ok!(Network::do_unpause());
        set_epoch(33, 0);
        assert_ok!(Network::do_unpause());
        resume_after(3 * EpochLength::get());

        for subnet_id in 1..active_id {
            let expected = Some(35 + subnet_id % 2);
            assert_eq!(SubnetRegistrationEpoch::<Test>::get(subnet_id), expected);
            assert_eq!(
                Network::rpc_get_subnet_info(subnet_id)
                    .unwrap()
                    .registration_epoch,
                expected
            );
        }
        assert_eq!(SubnetsData::<Test>::get(active_id), active);
        assert_eq!(PrevSubnetActivationEpoch::<Test>::get(), 33);
        assert!(!SubnetRegistrationEpoch::<Test>::contains_key(active_id));
        assert_eq!(NetworkPauseStartBlock::<Test>::get(), None);
    });
}

#[test]
fn resume_preserves_capacity_removal_cooldown_and_check_cadence() {
    new_test_ext().execute_with(|| {
        // Two healthy active subnets compete for one slot. No pending registrations exist.
        for subnet in 1..=2 {
            insert_subnet(subnet, SubnetState::Active, 0);
            SubnetSlot::<Test>::insert(subnet, DesignatedEpochSlots::get() + subnet);
            SubnetReputation::<Test>::insert(subnet, TOKEN);
            TotalAdmittedSubnetNodes::<Test>::insert(subnet, MinSubnetNodes::<Test>::get());
            TotalSubnetElectableNodes::<Test>::insert(subnet, MinSubnetNodes::<Test>::get());
            TotalSubnetDelegateStakeBalance::<Test>::insert(
                subnet,
                MinSubnetDelegateStakeBalance::<Test>::get() * (subnet as u128 + 1),
            );
        }
        assert_eq!(SubnetRegistrationEpoch::<Test>::iter().count(), 0);
        TotalSubnets::<Test>::put(2);
        TotalActiveSubnets::<Test>::put(2);
        MaxSubnets::<Test>::put(1);
        PrevSubnetActivationEpoch::<Test>::put(10);
        SubnetRemovalActivationCooldown::<Test>::put(4);
        SubnetRemovalCheckInterval::<Test>::put(3);

        set_epoch(11, 0);
        resume_after(20 * EpochLength::get());
        assert_eq!(PrevSubnetActivationEpoch::<Test>::get(), 31);

        // The shifted cooldown ends at epoch 35. Epoch 33 is a removal-check epoch,
        // but still protected; epoch 35 is unprotected, but not a removal-check epoch.
        for epoch in 32..=35 {
            set_epoch(epoch, NETWORK_EPOCH_PRELIMINARIES_SLOT);
            Network::on_initialize(System::block_number());
            assert!(SubnetsData::<Test>::contains_key(1));
            assert!(SubnetsData::<Test>::contains_key(2));
        }

        set_epoch(36, NETWORK_EPOCH_PRELIMINARIES_SLOT);
        Network::on_initialize(System::block_number());
        assert!(!SubnetsData::<Test>::contains_key(1));
        assert!(SubnetsData::<Test>::contains_key(2));
        assert!(network_events().contains(&Event::SubnetDeactivated {
            subnet_id: 1,
            reason: SubnetRemovalReason::MaxSubnets,
        }));
    });
}

#[test]
fn registrations_created_after_resume_start_in_current_epoch() {
    new_test_ext().execute_with(|| {
        set_epoch(10, 0);
        resume_after(20 * EpochLength::get());
        build_registered_subnet(
            b"after-resume".to_vec(),
            0,
            0,
            10_000 * TOKEN,
            100 * TOKEN,
            false,
            None,
        );
        let subnet = SubnetName::<Test>::get(b"after-resume".to_vec()).unwrap();
        assert_eq!(SubnetRegistrationEpoch::<Test>::get(subnet), Some(30));
        assert!(Network::is_subnet_registering(
            subnet,
            SubnetState::Registered,
            30
        ));
    });
}

#[test]
fn resume_cutoff_respects_assigned_slots_including_the_first_epoch() {
    new_test_ext().execute_with(|| {
        let subnet = 1;
        let slot = DesignatedEpochSlots::get();
        SubnetSlot::<Test>::insert(subnet, slot);
        LastNetworkUnpauseBlock::<Test>::put(slot - 1);
        assert!(!Network::consensus_round_precedes_network_resume(subnet, 0));
        LastNetworkUnpauseBlock::<Test>::put(slot);
        assert!(Network::consensus_round_precedes_network_resume(subnet, 0));
        assert!(!Network::consensus_round_precedes_network_resume(subnet, 1));
        LastNetworkUnpauseBlock::<Test>::put(slot + EpochLength::get());
        assert!(Network::consensus_round_precedes_network_resume(subnet, 1));
        assert!(!Network::consensus_round_precedes_network_resume(subnet, 2));
    });
}

fn fixture() -> (u32, u32) {
    increase_epochs(50);
    let name = b"global-pause".to_vec();
    build_activated_subnet(name.clone(), 0, NODES, 10_000 * TOKEN, 100 * TOKEN);
    let subnet = SubnetName::<Test>::get(name).unwrap();
    for node in 1..=NODES {
        let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
        assert_ok!(Network::handle_increase_account_validator_delegate_stake(
            &account(99),
            validator,
            100 * TOKEN,
        ));
    }
    seed_equal_validator_delegate_stake_for_subnet(subnet);
    StrongRejectionThreshold::<Test>::put(TOKEN / 2);
    let epoch = Network::get_current_subnet_epoch_as_u32(subnet);
    Network::elect_validator(subnet, epoch);
    let emission_epoch = epoch.saturating_add(1);
    FinalSubnetEmissionWeights::<Test>::insert(
        emission_epoch,
        DistributionData {
            subnets_emissions: 10 * TOKEN,
            subnet_weights: BTreeMap::from([(subnet, TOKEN)]),
        },
    );
    (subnet, epoch)
}

fn propose(subnet: u32, epoch: u32) -> frame_support::pallet_prelude::DispatchResultWithPostInfo {
    let node = SubnetElectedValidator::<Test>::get(subnet, epoch)
        .unwrap()
        .validator_subnet_node_id;
    let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
    Network::propose_attestation(
        RuntimeOrigin::signed(hotkey),
        subnet,
        (1..=NODES)
            .map(|subnet_node_id| SubnetNodeConsensusData {
                subnet_node_id,
                score: 100,
            })
            .collect(),
        None,
        None,
        None,
        None,
    )
}

fn submit(subnet: u32, epoch: u32, votes: u32) {
    if votes == 0 {
        return;
    }
    assert_ok!(propose(subnet, epoch));
    let proposer = SubnetElectedValidator::<Test>::get(subnet, epoch)
        .unwrap()
        .validator_subnet_node_id;
    for node in (1..=NODES)
        .filter(|node| *node != proposer)
        .take((votes - 1) as usize)
    {
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
        assert_ok!(Network::attest(
            RuntimeOrigin::signed(hotkey),
            subnet,
            node,
            None
        ));
    }
}

fn resume_after(blocks: u32) {
    assert_ok!(Network::do_pause());
    System::set_block_number(System::block_number() + blocks);
    Network::on_initialize(System::block_number());
    assert_ok!(Network::do_unpause());
}

fn settle(subnet: u32, epoch: u32) {
    set_block_to_subnet_slot_epoch(epoch, subnet);
    Network::emission_settlement_step(&mut WeightMeter::new(), epoch, subnet);
}

fn cancellation_count(subnet: u32, epoch: u32) -> usize {
    network_events()
        .iter()
        .filter(|event| {
            matches!(event,
        Event::ConsensusRoundCancelled { subnet_id, subnet_epoch }
        if *subnet_id == subnet && *subnet_epoch == epoch)
        })
        .count()
}

#[test]
fn global_resume_cancels_missing_partial_and_quorate_rounds() {
    for votes in [0, 1, NODES] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = fixture();
            submit(subnet, epoch, votes);
            let round = SubnetElectedValidator::<Test>::get(subnet, epoch).unwrap();
            let weights = SubnetConsensusAttestorWeights::<Test>::get(subnet, epoch).unwrap();
            let submission = SubnetConsensusSubmission::<Test>::get(subnet, epoch);
            let owner = SubnetOwner::<Test>::get(subnet).unwrap();
            let owner_balance = Balances::free_balance(owner.clone());
            let issuance = Balances::total_issuance();
            let subnet_delegation = TotalSubnetDelegateStakeBalance::<Test>::get(subnet);
            SubnetRewardCapacitor::<Test>::insert(subnet, TOKEN);
            resume_after(1);
            settle(subnet, epoch + 1);

            assert_eq!(
                ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
                Some(true)
            );

            assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 11 * TOKEN);
            assert_eq!(Balances::free_balance(owner), owner_balance);
            assert_eq!(Balances::total_issuance(), issuance);
            assert_eq!(
                TotalSubnetDelegateStakeBalance::<Test>::get(subnet),
                subnet_delegation
            );
            assert_eq!(SubnetReputation::<Test>::get(subnet), TOKEN);
            for node in 1..=NODES {
                let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
                assert_eq!(NodeSubnetStake::<Test>::get(node, subnet), 100 * TOKEN);
                assert_eq!(
                    ValidatorDelegateStakeBalance::<Test>::get(validator),
                    100 * TOKEN
                );
                assert_eq!(SubnetNodeReputation::<Test>::get(subnet, node), Some(TOKEN));
                assert_eq!(SubnetNodeProbationProgress::<Test>::get(subnet, node), 0);
            }
            assert_eq!(
                SubnetElectedValidator::<Test>::get(subnet, epoch),
                Some(round)
            );
            assert_eq!(
                SubnetConsensusAttestorWeights::<Test>::get(subnet, epoch),
                Some(weights)
            );
            assert_eq!(
                SubnetConsensusSubmission::<Test>::get(subnet, epoch),
                submission
            );
            assert_eq!(cancellation_count(subnet, epoch), 1);
            settle(subnet, epoch + 1);
            assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 11 * TOKEN);
            assert_eq!(cancellation_count(subnet, epoch), 1);

            Network::on_initialize(System::block_number());

            assert_ok!(propose(subnet, epoch + 1));
        });
    }
}

#[test]
fn same_block_and_short_pauses_reject_old_consensus_and_rpc_work() {
    for duration in [0, 1] {
        for votes in [0, 1] {
            new_test_ext().execute_with(|| {
                let (subnet, epoch) = fixture();
                submit(subnet, epoch, votes);
                resume_after(duration);
                assert_eq!(Network::get_current_subnet_epoch_as_u32(subnet), epoch);
                assert_noop!(
                    propose(subnet, epoch),
                    Error::<Test>::ConsensusRoundCancelled
                );
                if votes > 0 {
                    let proposer = SubnetElectedValidator::<Test>::get(subnet, epoch)
                        .unwrap()
                        .validator_subnet_node_id;
                    let node = (1..=NODES).find(|node| *node != proposer).unwrap();
                    let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
                    assert_noop!(
                        Network::attest(RuntimeOrigin::signed(hotkey), subnet, node, None),
                        Error::<Test>::ConsensusRoundCancelled
                    );
                }
                let status = Network::rpc_get_subnet_epoch_status(subnet).unwrap();
                assert_eq!(status.elected_validator_subnet_node_id, None);
                assert!(!status.proposal_submitted);
                assert!(!status.within_proposal_attestation_window);
                // The actual hook must cancel and elect a fresh round in one assigned slot.
                set_block_to_subnet_slot_epoch(epoch + 1, subnet);
                Network::on_initialize(System::block_number());
                assert_eq!(cancellation_count(subnet, epoch), 1);

                assert_ok!(propose(subnet, epoch + 1));
            });
        }
    }
}

#[test]
fn resume_leaves_skipped_history_inert_and_noop_unpause_keeps_fresh_rounds() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        // Skipped history is not retried after resume and cannot block a fresh election.
        set_block_to_subnet_slot_epoch(epoch + 2, subnet);
        resume_after(1);
        settle(subnet, epoch + 3);
        assert_eq!(cancellation_count(subnet, epoch), 0);
        assert_eq!(NodeSubnetStake::<Test>::get(1, subnet), 100 * TOKEN);
        Network::on_initialize(System::block_number());
        let cutoff = LastNetworkUnpauseBlock::<Test>::get();
        assert_ok!(Network::do_unpause());
        assert_eq!(LastNetworkUnpauseBlock::<Test>::get(), cutoff);
        assert_ok!(propose(subnet, epoch + 3));
    });
}

#[test]
fn repeated_pauses_cancel_once_and_missing_allocation_never_creates_rewards() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        let emission_epoch = epoch.saturating_add(1);
        FinalSubnetEmissionWeights::<Test>::remove(emission_epoch);
        resume_after(1);
        resume_after(1);
        settle(subnet, epoch + 1);
        assert_eq!(cancellation_count(subnet, epoch), 1);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 0);
    });
}

#[test]
fn cancellation_overflow_saturates_and_finalizes_once() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        SubnetRewardCapacitor::<Test>::insert(subnet, u128::MAX);
        resume_after(1);
        settle(subnet, epoch + 1);
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
            Some(true)
        );

        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), u128::MAX);

        assert_eq!(cancellation_count(subnet, epoch), 1);
        settle(subnet, epoch + 2);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), u128::MAX);
        assert_eq!(cancellation_count(subnet, epoch), 1);
    });
}

#[test]
fn global_resume_does_not_reprocess_settled_history() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        settle(subnet, epoch + 1);
        let stake = TotalSubnetStake::<Test>::get(subnet);
        let capacitor = SubnetRewardCapacitor::<Test>::get(subnet);
        let reputation = SubnetReputation::<Test>::get(subnet);
        resume_after(1);
        Network::precheck_subnet_consensus_submission(subnet, epoch);
        assert_eq!(TotalSubnetStake::<Test>::get(subnet), stake);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), capacitor);
        assert_eq!(SubnetReputation::<Test>::get(subnet), reputation);
        assert_eq!(cancellation_count(subnet, epoch), 0);
    });
}
