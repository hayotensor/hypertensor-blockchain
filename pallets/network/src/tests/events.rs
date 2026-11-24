use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{assert_noop, assert_ok, traits::Currency, weights::WeightMeter};
use sp_std::collections::btree_map::BTreeMap;

const FUNDING: u128 = 10_000_000_000_000_000_000_000;

fn register_identity(coldkey: &AccountId, hotkey: &AccountId) -> u32 {
    let _ = Balances::deposit_creating(coldkey, FUNDING);
    assert_ok!(Network::register_validator(
        RuntimeOrigin::signed(coldkey.clone()),
        hotkey.clone(),
        0,
        None,
        None,
    ));
    ColdkeyValidatorId::<Test>::get(coldkey).unwrap()
}

fn subnet_fixture() -> u32 {
    let name = b"event-coverage".to_vec();
    build_activated_subnet(
        name.clone(),
        0,
        4,
        FUNDING,
        MinSubnetMinStake::<Test>::get(),
    );
    SubnetName::<Test>::get(name).unwrap()
}

#[test]
fn identity_events_record_registration_resolved_beneficiary_and_key_rotations() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let coldkey = account(900);
        let hotkey = account(901);
        let validator_id = register_identity(&coldkey, &hotkey);
        System::assert_last_event(
            Event::ValidatorRegistered {
                validator_id,
                coldkey: coldkey.clone(),
                hotkey: hotkey.clone(),
                delegate_reward_rate: 0,
                delegate_account: None,
                burn_amount: TestEconomics::get().validator_registration_burn,
            }
            .into(),
        );

        let beneficiary = account(902);
        let rate = test_percent(1, 10);
        assert_ok!(Network::update_validator_delegate_account(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            Some(beneficiary.clone()),
            Some(rate),
        ));
        // A partial update must report the complete resolved configuration.
        let new_rate = test_percent(1, 5);
        assert_ok!(Network::update_validator_delegate_account(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            None,
            Some(new_rate),
        ));
        let expected = DelegateAccount {
            account_id: beneficiary,
            rate: new_rate,
        };
        assert_eq!(
            ValidatorsData::<Test>::get(validator_id).delegate_account,
            Some(expected.clone())
        );
        System::assert_last_event(
            Event::ValidatorDelegateAccountUpdated {
                validator_id,
                delegate_account: expected,
            }
            .into(),
        );
        assert_noop!(
            Network::update_validator_delegate_account(
                RuntimeOrigin::signed(coldkey.clone()),
                validator_id,
                None,
                Some(0),
            ),
            Error::<Test>::InvalidDelegateAccountRate
        );

        let new_hotkey = account(903);
        assert_ok!(Network::update_validator_hotkey(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            new_hotkey.clone(),
        ));
        System::assert_last_event(
            Event::UpdateHotkey {
                validator_id,
                hotkey,
                new_hotkey: new_hotkey.clone(),
            }
            .into(),
        );
        let events = System::events();
        assert_ok!(Network::update_validator_hotkey(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            new_hotkey,
        ));
        assert_eq!(System::events(), events);

        let new_coldkey = account(904);
        assert_ok!(Network::update_validator_coldkey(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            new_coldkey.clone(),
        ));
        System::assert_last_event(
            Event::UpdateColdkey {
                validator_id,
                coldkey: coldkey.clone(),
                new_coldkey: new_coldkey.clone(),
            }
            .into(),
        );
        assert_eq!(
            ValidatorColdkey::<Test>::get(validator_id),
            Some(new_coldkey)
        );
        assert_noop!(
            Network::update_validator_hotkey(
                RuntimeOrigin::signed(coldkey),
                validator_id,
                account(905),
            ),
            Error::<Test>::NotKeyOwner
        );
    });
}

#[test]
fn node_registration_events_cover_bootstrap_and_queue_and_hotkey_reset() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let subnet_id = subnet_fixture();
        let registrations: Vec<_> = network_events()
            .into_iter()
            .filter(|event| matches!(event, Event::SubnetNodeRegistered { .. }))
            .collect();
        assert_eq!(registrations.len(), 4);
        for event in registrations {
            let Event::SubnetNodeRegistered {
                subnet_id: registered_subnet,
                subnet_node_id,
                validator_id,
                coldkey,
                hotkey,
                stake,
                ..
            } = event
            else {
                unreachable!()
            };
            assert_eq!(registered_subnet, subnet_id);
            assert!(SubnetNodesData::<Test>::contains_key(
                subnet_id,
                subnet_node_id
            ));
            assert_eq!(
                SubnetNodeValidatorId::<Test>::get(subnet_id, subnet_node_id),
                Some(validator_id)
            );
            assert_eq!(ValidatorColdkey::<Test>::get(validator_id), Some(coldkey));
            assert_eq!(
                SubnetNodeIdHotkey::<Test>::get(subnet_id, subnet_node_id),
                hotkey
            );
            assert_eq!(
                NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id),
                stake
            );
        }

        System::reset_events();
        let stake = MinSubnetMinStake::<Test>::get();
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        build_queued_subnet_nodes(subnet_id, 4, 5, FUNDING, stake);
        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);
        let validator_id = SubnetNodeValidatorId::<Test>::get(subnet_id, subnet_node_id).unwrap();
        let coldkey = ValidatorColdkey::<Test>::get(validator_id).unwrap();
        System::assert_last_event(
            Event::SubnetNodeRegistered {
                subnet_id,
                subnet_node_id,
                validator_id,
                coldkey: coldkey.clone(),
                hotkey: None,
                stake,
                burn_amount,
            }
            .into(),
        );
        assert!(QueuedSubnetNodesData::<Test>::contains_key(
            subnet_id,
            subnet_node_id
        ));
        assert!(!network_events()
            .iter()
            .any(|e| matches!(e, Event::SubnetNodeAdmitted { .. })));
        for hotkey in [Some(account(990)), None] {
            assert_ok!(Network::update_node_hotkey(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                hotkey.clone(),
            ));
            assert_eq!(
                SubnetNodeIdHotkey::<Test>::get(subnet_id, subnet_node_id),
                hotkey
            );
            System::assert_last_event(
                Event::NodeHotkeyUpdated {
                    subnet_id,
                    subnet_node_id,
                    hotkey,
                }
                .into(),
            );
        }
    });
}

#[test]
fn delegate_transfer_events_report_shares_and_both_owners() {
    new_test_ext().execute_with(|| {
        let subnet_id = subnet_fixture();
        let validator_id = SubnetNodeValidatorId::<Test>::get(subnet_id, 1).unwrap();
        let from = account(900);
        let to = account(901);
        let _ = Balances::deposit_creating(&from, FUNDING);
        let amount = 100_000_000_000_000_000_000;
        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(from.clone()),
            subnet_id,
            amount,
            1
        ));
        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(from.clone()),
            validator_id,
            amount,
            1
        ));
        let subnet_shares = AccountSubnetDelegateStakeShares::<Test>::get(&from, subnet_id) / 2;
        let validator_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(&from, validator_id) / 2;
        assert_ok!(Network::transfer_delegate_stake(
            RuntimeOrigin::signed(from.clone()),
            subnet_id,
            to.clone(),
            subnet_shares
        ));
        System::assert_last_event(
            Event::SubnetDelegateStakeTransferred {
                subnet_id,
                from_account: from.clone(),
                to_account: to.clone(),
                shares: subnet_shares,
            }
            .into(),
        );
        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&to, subnet_id),
            subnet_shares
        );
        assert_ok!(Network::transfer_validator_delegate_stake(
            RuntimeOrigin::signed(from.clone()),
            validator_id,
            to.clone(),
            validator_shares
        ));
        System::assert_last_event(
            Event::ValidatorDelegateStakeTransferred {
                validator_id,
                from_account: from.clone(),
                to_account: to.clone(),
                shares: validator_shares,
            }
            .into(),
        );
        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&to, validator_id),
            validator_shares
        );
        assert_noop!(
            Network::transfer_delegate_stake(
                RuntimeOrigin::signed(from.clone()),
                subnet_id,
                from,
                subnet_shares
            ),
            Error::<Test>::TransferToSelfNotAllowed
        );
    });
}

#[test]
fn claim_events_only_describe_entries_and_refunds_actually_paid() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        let coldkey = account(900);
        let small = EXISTENTIAL_DEPOSIT - 1;
        let network = EXISTENTIAL_DEPOSIT + 7;
        let overwatch = 30;
        let refund = 50;
        assert_eq!(Balances::free_balance(&coldkey), 0);
        StakeUnbondingLedger::<Test>::insert(
            &coldkey,
            BTreeMap::from([
                (
                    1,
                    UnbondingEntry {
                        network: small,
                        overwatch: 0,
                    },
                ),
                (2, UnbondingEntry { network, overwatch }),
                (
                    20,
                    UnbondingEntry {
                        network: 5,
                        overwatch: 0,
                    },
                ),
            ]),
        );
        TotalNetworkUnbondingBalance::<Test>::put(small + network + 5);
        QueuedSwapRefundBalance::<Test>::insert(&coldkey, refund);
        TotalQueuedSwapRefundBalance::<Test>::put(refund);
        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            coldkey.clone()
        )));
        assert_eq!(
            network_events(),
            vec![
                Event::UnbondingClaimed {
                    coldkey: coldkey.clone(),
                    claim_block: 2,
                    network_amount: network,
                    overwatch_amount: overwatch
                },
                Event::SwapRefundClaimed {
                    account_id: coldkey.clone(),
                    amount: refund
                },
            ]
        );
        assert_eq!(
            Balances::free_balance(&coldkey),
            network + overwatch + refund
        );
        assert_eq!(StakeUnbondingLedger::<Test>::get(&coldkey).len(), 2);
        // The initially too-small entry is now payable because the wallet exists.
        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            coldkey.clone()
        )));
        System::assert_last_event(
            Event::UnbondingClaimed {
                coldkey: coldkey.clone(),
                claim_block: 1,
                network_amount: small,
                overwatch_amount: 0,
            }
            .into(),
        );
        assert_noop!(
            Network::claim_unbondings(RuntimeOrigin::signed(coldkey)),
            Error::<Test>::NoStakeUnbondingsOrCooldownNotMet
        );
    });
}

#[test]
fn overwatch_events_track_membership_overrides_and_peer_replacement() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let coldkey = account(900);
        let validator_id = register_identity(&coldkey, &account(901));
        prepare_overwatch_validator(validator_id);
        let stake = OverwatchMinStakeBalance::<Test>::get();
        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            stake
        ));
        let overwatch_node_id = ValidatorOverwatchNodeId::<Test>::get(validator_id).unwrap();
        System::assert_last_event(
            Event::OverwatchNodeRegistered {
                overwatch_node_id,
                validator_id,
                coldkey: coldkey.clone(),
                stake,
            }
            .into(),
        );
        for hotkey in [Some(account(902)), None] {
            assert_ok!(Network::update_overwatch_hotkey(
                RuntimeOrigin::signed(coldkey.clone()),
                overwatch_node_id,
                hotkey.clone()
            ));
            System::assert_last_event(
                Event::OverwatchNodeHotkeyUpdated {
                    overwatch_node_id,
                    hotkey: hotkey.clone(),
                }
                .into(),
            );
            assert_eq!(
                OverwatchNodeIdHotkey::<Test>::get(overwatch_node_id),
                hotkey
            );
        }
        let subnet_id = 42;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        for peer_id in [peer(90_001), peer(90_002)] {
            assert_ok!(Network::set_overwatch_node_peer_id(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                overwatch_node_id,
                peer_id.clone()
            ));
            System::assert_last_event(
                Event::OverwatchNodePeerIdUpdated {
                    subnet_id,
                    overwatch_node_id,
                    peer_id: peer_id.clone(),
                }
                .into(),
            );
            assert_eq!(
                PeerIdOverwatchNodeId::<Test>::get(subnet_id, peer_id),
                overwatch_node_id
            );
        }
        assert_ok!(Network::remove_overwatch_node(
            RuntimeOrigin::signed(coldkey),
            overwatch_node_id
        ));
        System::assert_last_event(
            Event::OverwatchNodeRemoved {
                overwatch_node_id,
                validator_id,
            }
            .into(),
        );
        assert!(!OverwatchNodes::<Test>::contains_key(overwatch_node_id));
        assert!(!OverwatchValidatorWhitelist::<Test>::contains_key(
            validator_id
        ));
        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id),
            stake
        );
    });
}

#[test]
fn commit_reveal_events_report_complete_batches_only_after_validation() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let overwatch_node_id = 1;
        let overwatch_epoch = CurrentOverwatchEpoch::<Test>::get();
        seed_overwatch_epoch_eligibility(overwatch_node_id);
        insert_subnet(11, SubnetState::Active, 0);
        insert_subnet(12, SubnetState::Active, 0);
        let salt = b"event-reveal".to_vec();
        let hash =
            Network::hash_overwatch_commitment(overwatch_node_id, 11, overwatch_epoch, 100, &salt);
        let accepted = OverwatchCommit {
            subnet_id: 11,
            weight: hash,
        };
        assert_noop!(
            Network::perform_commit_overwatch_subnet_weights(
                overwatch_node_id,
                vec![
                    accepted.clone(),
                    OverwatchCommit {
                        subnet_id: 99,
                        weight: hash
                    }
                ],
            ),
            Error::<Test>::InvalidSubnetId
        );
        assert_noop!(
            Network::perform_commit_overwatch_subnet_weights(
                overwatch_node_id,
                vec![accepted.clone(), accepted.clone()],
            ),
            Error::<Test>::DuplicateOverwatchSubnetId
        );
        assert_ok!(Network::perform_commit_overwatch_subnet_weights(
            overwatch_node_id,
            vec![accepted.clone()],
        ));
        System::assert_last_event(
            Event::OverwatchWeightsCommitted {
                overwatch_node_id,
                overwatch_epoch,
                commits: vec![accepted.clone()],
            }
            .into(),
        );
        assert_noop!(
            Network::perform_commit_overwatch_subnet_weights(
                overwatch_node_id,
                vec![
                    OverwatchCommit {
                        subnet_id: 12,
                        weight: hash
                    },
                    accepted,
                ]
            ),
            Error::<Test>::AlreadyCommitted
        );
        let reveal = OverwatchReveal::<Test> {
            subnet_id: 11,
            weight: 100,
            salt: salt.try_into().unwrap(),
        };
        assert_noop!(
            Network::perform_reveal_overwatch_subnet_weights(
                overwatch_node_id,
                vec![
                    reveal.clone(),
                    OverwatchReveal::<Test> {
                        subnet_id: 12,
                        weight: 100,
                        salt: reveal.salt.clone()
                    },
                ]
            ),
            Error::<Test>::RevealSetMismatch
        );
        assert_noop!(
            Network::perform_reveal_overwatch_subnet_weights(
                overwatch_node_id,
                vec![reveal.clone(), reveal.clone()]
            ),
            Error::<Test>::DuplicateOverwatchSubnetId
        );
        assert_ok!(Network::perform_reveal_overwatch_subnet_weights(
            overwatch_node_id,
            vec![reveal]
        ));
        System::assert_last_event(
            Event::OverwatchWeightsRevealed {
                overwatch_node_id,
                overwatch_epoch,
                weights: vec![(11, 100)],
            }
            .into(),
        );
        assert_eq!(
            OverwatchReveals::<Test>::get(overwatch_epoch, overwatch_node_id).len(),
            1
        );
    });
}

#[test]
fn registration_access_events_follow_authorized_policy_changes() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let coldkey = account(900);
        for approved in [true, false] {
            assert_ok!(Network::update_require_subnet_registration_whitelist(
                RuntimeOrigin::from(pallet_collective::RawOrigin::Members(2, 3)),
                approved,
            ));
            System::assert_last_event(
                Event::SubnetRegistrationWhitelistRequired { required: approved }.into(),
            );
            assert_eq!(RequireSubnetRegistrationWhitelist::<Test>::get(), approved);
            assert_ok!(Network::update_subnet_registrant(
                RuntimeOrigin::from(pallet_collective::RawOrigin::Members(2, 3)),
                coldkey.clone(),
                7,
                approved,
            ));
            System::assert_last_event(
                Event::SubnetRegistrantUpdated {
                    coldkey: coldkey.clone(),
                    subnet_id: 7,
                    approved,
                }
                .into(),
            );
            assert_eq!(
                SubnetRegistrationWhitelist::<Test>::get(&coldkey, 7),
                approved
            );
        }
        assert_noop!(
            Network::update_subnet_registrant(
                RuntimeOrigin::signed(coldkey.clone()),
                coldkey,
                7,
                true
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn hook_events_distinguish_new_elections_rate_changes_and_empty_allocations() {
    new_test_ext().execute_with(|| {
        let subnet_id = subnet_fixture();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id) + 1;
        Network::elect_validator(subnet_id, subnet_epoch);
        let round = SubnetElectedValidator::<Test>::get(subnet_id, subnet_epoch).unwrap();
        System::assert_last_event(
            Event::ConsensusValidatorElected {
                subnet_id,
                subnet_epoch,
                subnet_node_id: round.validator_subnet_node_id,
                validator_id: round.validator_id,
            }
            .into(),
        );
        let events = System::events();
        Network::elect_validator(subnet_id, subnet_epoch);
        assert_eq!(System::events(), events);
        Network::elect_validator(99, subnet_epoch);
        assert_eq!(System::events(), events);

        let previous_rate = MaxNodeBurnRate::<Test>::get();
        let new_rate = MinNodeBurnRate::<Test>::get();
        assert_ne!(previous_rate, new_rate);
        CurrentNodeBurnRate::<Test>::insert(subnet_id, previous_rate);
        NodeBurnRateAlpha::<Test>::insert(subnet_id, Network::percentage_factor_as_u128());
        NodeRegistrationsThisEpoch::<Test>::insert(subnet_id, 0);
        Network::update_burn_rate_for_epoch(&mut WeightMeter::new(), subnet_id);
        System::assert_last_event(
            Event::NodeBurnRateUpdated {
                subnet_id,
                previous_rate,
                new_rate,
            }
            .into(),
        );
        assert_eq!(CurrentNodeBurnRate::<Test>::get(subnet_id), new_rate);
        let events = System::events();
        Network::update_burn_rate_for_epoch(&mut WeightMeter::new(), subnet_id);
        assert_eq!(System::events(), events);
    });
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let _ = Network::handle_subnet_emission_weights(1);
        System::assert_last_event(
            Event::SubnetEmissionWeightsFinalized {
                epoch: 1,
                subnets_emissions: 0,
                subnet_count: 0,
            }
            .into(),
        );
        assert!(FinalSubnetEmissionWeights::<Test>::contains_key(1));
    });
}
