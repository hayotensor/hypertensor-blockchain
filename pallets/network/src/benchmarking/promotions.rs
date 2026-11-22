//! Research only: coherent mixed membership before election, real proposals and attestations.
//! Case numbers enumerate scenarios; fitting a regression against the number is meaningless.
use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct Case {
    pub nodes: u32,
    pub probation: u32,
    // 0 ready; 1 reputation recovery; 2 progress only; 3 omission; 4 shared identities;
    // 5 full queue; 6 shared identities + full queue; 7 departure after attestation.
    pub profile: u32,
}

pub(super) const LAST_CASE: u32 = 804;

pub(super) fn case(index: u32) -> Case {
    assert!(index <= LAST_CASE);
    if index < 254 {
        return Case {
            nodes: 256,
            probation: index,
            profile: 0,
        };
    }
    if index < 764 {
        return Case {
            nodes: 512,
            probation: index - 254,
            profile: 0,
        };
    }
    if index < 773 {
        let nodes = [4, 64, 128][((index - 764) / 3) as usize];
        let probation = [1, ((nodes - 3) / 2).max(1), nodes - 3][((index - 764) % 3) as usize];
        return Case {
            nodes,
            probation,
            profile: 0,
        };
    }
    if index < 801 {
        let offset = index - 773;
        let nodes = if offset / 14 == 0 { 256 } else { 512 };
        let profile = (offset % 14) / 2 + 1;
        let probation = if offset % 2 == 0 { 1 } else { nodes - 3 };
        return Case {
            nodes,
            probation,
            profile,
        };
    }
    // Progress/omission alongside both rewarded and promoted nodes.
    let nodes = if index < 803 { 256 } else { 512 };
    Case {
        nodes,
        probation: nodes - 3,
        profile: 8 + (index - 801) % 2,
    }
}

pub(super) struct Fixture {
    pub round: capacity::SettlementFixture,
    pub scenario: Case,
    pub active: u32,
    pub expected_promotions: BTreeSet<u32>,
    pub departed: Option<u32>,
    pub electable_before: u32,
    pub global_before: u32,
}

fn included(scenario: Case, active: u32, id: u32) -> bool {
    id <= active || (scenario.profile != 3 && !(scenario.profile == 9 && id % 2 == 0))
}

fn ready(scenario: Case, id: u32) -> bool {
    !matches!(scenario.profile, 2 | 3) && (!matches!(scenario.profile, 8 | 9) || id % 2 == 1)
}

pub(super) fn fixture<T: Config>(scenario: Case) -> Fixture {
    // Recovery and shared-identity/full-queue boundaries retain maximum public metadata.
    fixture_with_metadata::<T>(scenario, matches!(scenario.profile, 1 | 6))
}

fn fixture_with_metadata<T: Config>(scenario: Case, full_metadata: bool) -> Fixture {
    let Case {
        nodes,
        probation,
        profile,
    } = scenario;
    assert!((4..=T::MaxSubnetNodesUpperBound::get()).contains(&nodes));
    assert!(probation <= nodes - 3);
    assert!(profile <= 9);
    let active = nodes - probation;
    let subnet = capacity::promotion_elected_fixture::<T>(nodes, full_metadata);
    let one = Network::<T>::percentage_factor_as_u128();
    // Seed the state reached by earlier admissions/progression. Do not rewrite an existing
    // round's classification or voting snapshot; the measured round is elected below.
    for id in (active + 1..=nodes).rev() {
        assert!(Network::<T>::remove_node_from_election_slot(subnet, id).unwrap());
        NodeElectionSlotIndex::<T>::remove(subnet, id);
        SubnetNodesData::<T>::mutate(subnet, id, |node| {
            node.classification.node_class = SubnetNodeClass::Probation;
        });
        SubnetNodeProbationProgress::<T>::insert(
            subnet,
            id,
            if ready(scenario, id) { 1 } else { 0 },
        );
        // Omission must actually remove an existing counter.
        if !included(scenario, active, id) {
            SubnetNodeProbationProgress::<T>::insert(subnet, id, 1);
        }
        // 99% exercises exponentiation without rounding the input to exactly 1.0.
        SubnetNodeReputation::<T>::insert(
            subnet,
            id,
            if profile == 1 { one * 99 / 100 } else { one },
        );
    }
    ProbationRounds::<T>::insert(subnet, 2);
    if matches!(profile, 4 | 6) {
        // Keep three independently funded identities and coherent ownership/allocation maps.
        for id in 1..=nodes {
            ValidatorSubnetNodes::<T>::remove(id);
            TotalValidatorNodes::<T>::remove(id);
            ValidatorNodeDelegateStakeWeights::<T>::remove(id);
        }
        for id in 1..=nodes {
            let validator = (id - 1) % 3 + 1;
            SubnetNodeValidatorId::<T>::insert(subnet, id, validator);
            SubnetNodesData::<T>::mutate(subnet, id, |node| node.validator_id = validator);
            ValidatorSubnetNodes::<T>::mutate(validator, |owned| {
                owned.entry(subnet).or_default().insert(id);
            });
            TotalValidatorNodes::<T>::mutate(validator, |count| *count += 1);
            let owned = (nodes + 3 - validator) / 3;
            ValidatorNodeDelegateStakeWeights::<T>::mutate(validator, |weights| {
                weights.insert((subnet, id), one / u128::from(owned));
            });
        }
    }
    increase_epochs::<T>(1);
    set_block_to_subnet_slot_epoch::<T>(Network::<T>::get_current_epoch_as_u32(), subnet);
    let epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet);
    Network::<T>::elect_validator(subnet, epoch);
    let election = SubnetElectedValidator::<T>::get(subnet, epoch).unwrap();
    assert_eq!(election.eligible_subnet_node_ids.len(), active as usize);
    assert_eq!(election.policy.probation_rounds, 2);
    let (priority, removal) = if matches!(profile, 5 | 6) {
        let (priority, removal) = capacity::max_queue::<T>(subnet, nodes);
        (Some(priority), Some(removal))
    } else {
        (None, None)
    };
    let proposer = election.validator_subnet_node_id;
    let args: ValidatorArgs<T> = vec![0xff; T::ValidatorArgsLimit::get() as usize]
        .try_into()
        .unwrap();
    assert_ok!(Network::<T>::propose_attestation(
        RawOrigin::Signed(
            Network::<T>::get_subnet_node_associated_hotkey(subnet, proposer).unwrap()
        )
        .into(),
        subnet,
        (1..=nodes)
            .filter(|id| included(scenario, active, *id))
            .map(|subnet_node_id| SubnetNodeConsensusData {
                subnet_node_id,
                score: DEFAULT_SCORE
            })
            .collect(),
        priority,
        removal,
        Some(args.clone()),
        Some(args),
    ));
    for id in 1..=active {
        if id != proposer {
            assert_ok!(Network::<T>::attest(
                RawOrigin::Signed(
                    Network::<T>::get_subnet_node_associated_hotkey(subnet, id).unwrap()
                )
                .into(),
                subnet,
                id,
                None,
            ));
        }
    }
    let (submission, _) = Network::<T>::precheck_subnet_consensus_submission(subnet, epoch);
    let submission = submission.unwrap();
    assert_eq!(submission.identity_attestation_ratio, one);
    assert_eq!(submission.attestation_ratio, one);
    assert_eq!(submission.subnet_nodes.len(), nodes as usize);
    assert!(submission.weight_sum > 0);
    let round = capacity::capture_settlement::<T>(subnet, nodes);
    let departed = if profile == 7 {
        let id = (1..=active).find(|id| *id != proposer).unwrap();
        let coldkey = Network::<T>::get_subnet_node_associated_coldkey(subnet, id).unwrap();
        assert_ok!(Network::<T>::remove_subnet_node(
            RawOrigin::Signed(coldkey).into(),
            subnet,
            id
        ));
        assert!(!SubnetNodesData::<T>::contains_key(subnet, id));
        // Removal does not change the already accepted voting/membership evidence.
        assert_eq!(
            SubnetConsensusSubmission::<T>::get(subnet, epoch)
                .unwrap()
                .subnet_nodes
                .len(),
            nodes as usize
        );
        Some(id)
    } else {
        None
    };
    capacity::prepare_settlement::<T>(subnet);
    let expected_promotions = (active + 1..=nodes)
        .filter(|id| ready(scenario, *id))
        .collect();
    Fixture {
        round,
        scenario,
        active,
        expected_promotions,
        departed,
        electable_before: TotalSubnetElectableNodes::<T>::get(subnet),
        global_before: TotalElectableNodes::<T>::get(),
    }
}

pub(super) fn verify<T: Config>(fixture: &Fixture, hook: bool) {
    let subnet = fixture.round.subnet_id;
    let epoch = fixture.round.epoch;
    assert_eq!(
        ConsensusRoundSettlementStatus::<T>::get(subnet, epoch),
        Some(true)
    );
    assert!(TotalSubnetDelegateStakeBalance::<T>::get(subnet) > fixture.round.subnet_pool_balance);
    assert!(
        Network::<T>::balance_to_u128(T::Currency::free_balance(
            &SubnetOwner::<T>::get(subnet).unwrap()
        ))
        .unwrap()
            > fixture.round.owner_balance
    );
    let promoted = fixture.expected_promotions.len() as u32;
    assert_eq!(
        TotalSubnetElectableNodes::<T>::get(subnet),
        fixture.electable_before + promoted
    );
    assert_eq!(
        TotalElectableNodes::<T>::get(),
        fixture.global_before + promoted
    );
    let slots = SubnetNodeElectionSlots::<T>::get(subnet);
    assert_eq!(slots.len() as u32, fixture.electable_before + promoted);
    for (index, id) in slots.iter().enumerate() {
        assert_eq!(NodeElectionSlotIndex::<T>::get(subnet, id), Some(index as u32));
    }
    for id in 1..=fixture.scenario.nodes {
        let index = (id - 1) as usize;
        if fixture.departed == Some(id) {
            continue;
        }
        let validator = SubnetNodeValidatorId::<T>::get(subnet, id).unwrap();
        if id <= fixture.active {
            assert!(NodeSubnetStake::<T>::get(id, subnet) > fixture.round.direct_stakes[index]);
            assert!(
                ValidatorDelegateStakeBalance::<T>::get(validator)
                    > fixture.round.pool_balances[index]
            );
            assert!(
                DelegateAccountStake::<T>::get(get_account::<T>("capacity-beneficiary", validator))
                    > fixture.round.beneficiary_balances[index]
            );
        } else {
            assert_eq!(
                NodeSubnetStake::<T>::get(id, subnet),
                fixture.round.direct_stakes[index]
            );
            if !matches!(fixture.scenario.profile, 4 | 6) {
                assert_eq!(
                    ValidatorDelegateStakeBalance::<T>::get(validator),
                    fixture.round.pool_balances[index]
                );
                assert_eq!(
                    DelegateAccountStake::<T>::get(get_account::<T>(
                        "capacity-beneficiary",
                        validator
                    )),
                    fixture.round.beneficiary_balances[index]
                );
            }
            let is_promoted = fixture.expected_promotions.contains(&id);
            let node = SubnetNodesData::<T>::get(subnet, id);
            assert_eq!(
                node.classification.node_class,
                if is_promoted {
                    SubnetNodeClass::Active
                } else {
                    SubnetNodeClass::Probation
                }
            );
            if is_promoted {
                assert_eq!(node.classification.start_epoch, epoch + 1);
                assert!(slots.contains(&id));
                assert!(!SubnetNodeProbationProgress::<T>::contains_key(subnet, id));
                assert_eq!(
                    SubnetNodeReputation::<T>::get(subnet, id),
                    Some(Network::<T>::percentage_factor_as_u128())
                );
            } else {
                assert!(!slots.contains(&id));
                assert_eq!(
                    SubnetNodeProbationProgress::<T>::get(subnet, id),
                    u32::from(included(fixture.scenario, fixture.active, id))
                );
            }
        }
    }
    let expected_events: Vec<_> = fixture
        .expected_promotions
        .iter()
        .map(|id| {
            <T as Config>::RuntimeEvent::from(Event::<T>::SubnetNodePromoted {
                subnet_id: subnet,
                subnet_node_id: *id,
                classification: SubnetNodeClassification {
                    node_class: SubnetNodeClass::Active,
                    start_epoch: epoch + 1,
                },
            })
            .into()
        })
        .collect();
    let events = frame_system::Pallet::<T>::events();
    for expected in expected_events {
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event == expected)
                .count(),
            1
        );
    }
    if hook {
        let election =
            SubnetElectedValidator::<T>::get(subnet, epoch + 1).expect("hook must elect");
        let weights = SubnetConsensusAttestorWeights::<T>::get(subnet, epoch + 1)
            .expect("hook must snapshot weights");
        assert_eq!(election.eligible_subnet_node_ids.len(), slots.len());
        assert_eq!(weights.weights.len(), slots.len());
        for id in &fixture.expected_promotions {
            assert!(election.eligible_subnet_node_ids.contains(id));
        }
    }
}

#[test]
fn promotion_capacity_fixtures() {
    use crate::tests::mock::{new_test_ext, Test};
    for (nodes, probation) in [(4, 1), (64, 31), (256, 253), (512, 509)] {
        for profile in 0..=9 {
            new_test_ext().execute_with(|| {
                let fixture = fixture::<Test>(Case {
                    nodes,
                    probation,
                    profile,
                });
                frame_system::Pallet::<Test>::reset_events();
                Network::<Test>::on_initialize(frame_system::Pallet::<Test>::block_number());
                verify::<Test>(&fixture, true);
                // Shared validator pools receive Active-node rewards. Inspect the per-node
                // event as well, rather than mistaking pool growth for a Probation payout.
                let rewards = frame_system::Pallet::<Test>::events()
                    .into_iter()
                    .find_map(|event| {
                        if let crate::tests::mock::RuntimeEvent::Network(Event::SubnetRewards {
                            node_rewards,
                            node_delegate_account_allocations,
                            ..
                        }) = event.event
                        {
                            Some((node_rewards, node_delegate_account_allocations))
                        } else {
                            None
                        }
                    })
                    .expect("accepted round must report rewards");
                assert!(rewards.0.iter().all(|(id, _)| *id <= fixture.active));
                assert!(rewards.1.iter().all(|(id, _)| *id <= fixture.active));
            });
        }
    }
}

#[test]
fn promotion_capacity_metadata_does_not_change_outcomes() {
    use crate::tests::mock::{new_test_ext, Test};
    for probation in [1, 509] {
        let execute = |full_metadata| {
            new_test_ext().execute_with(|| {
                let fixture = fixture_with_metadata::<Test>(
                    Case {
                        nodes: 512,
                        probation,
                        profile: 0,
                    },
                    full_metadata,
                );
                frame_system::Pallet::<Test>::reset_events();
                Network::<Test>::on_initialize(frame_system::Pallet::<Test>::block_number());
                verify::<Test>(&fixture, true);
                let subnet = fixture.round.subnet_id;
                // Normalize only optional metadata and its reverse indexes. All economic,
                // classification, snapshot, counter, and event state must be identical.
                let _ = SubnetNodeMetadata::<Test>::clear_prefix(subnet, u32::MAX, None);
                let _ = PeerIdSubnetNodeId::<Test>::clear_prefix(subnet, u32::MAX, None);
                let _ = BootnodePeerIdSubnetNodeId::<Test>::clear_prefix(subnet, u32::MAX, None);
                let _ = ClientPeerIdSubnetNodeId::<Test>::clear_prefix(subnet, u32::MAX, None);
                let _ = MultiaddrSubnetNodeId::<Test>::clear_prefix(subnet, u32::MAX, None);
                let _ = UniqueParamSubnetNodeId::<Test>::clear_prefix(subnet, u32::MAX, None);
                for id in 1..=512 {
                    ValidatorMetadata::<Test>::remove(id);
                }
                sp_io::storage::root(sp_runtime::StateVersion::V1)
            })
        };
        assert_eq!(execute(false), execute(true));
    }
}

#[test]
fn promotion_capacity_manifest() {
    use crate::tests::mock::{new_test_ext, Test, TestDbWeight};
    new_test_ext().execute_with(|| {
        TestDbWeight::set(frame_support::weights::constants::RocksDbWeight::get());
        // The mock has an unlimited proof budget. The runner separately executes the real
        // runtime's promotion_budget_matches_research_configuration test before using this.
        let limit = Weight::from_parts(1_000_000_000_000, 16 * 1024 * 1024);
        println!(
            "promotion_limit,{},{},{}",
            limit.ref_time(),
            limit.proof_size(),
            LAST_CASE
        );
        for c in 0..=LAST_CASE {
            let scenario = case(c);
            let queued = if matches!(scenario.profile, 5 | 6) {
                64
            } else {
                0
            };
            let admitted = scenario.nodes - u32::from(scenario.profile == 7);
            let w = <Test as Config>::WeightInfo::on_initialize_base()
                .saturating_add(<Test as Config>::WeightInfo::advance_overwatch_epoch_noop())
                .saturating_add(<Test as Config>::WeightInfo::emission_slot_selector())
                .saturating_add(<Test as Config>::WeightInfo::emission_step_selectors())
                .saturating_add(Network::<Test>::subnet_slot_mandatory_weight(
                    scenario.nodes,
                    queued,
                    admitted,
                ));
            let active = scenario.nodes - scenario.probation;
            let promotions = (active + 1..=scenario.nodes)
                .filter(|id| ready(scenario, *id))
                .count();
            println!(
                "promotion_case,{c},{},{},{},{promotions},{},{},{}",
                scenario.nodes,
                scenario.probation,
                scenario.profile,
                w.ref_time(),
                w.proof_size(),
                queued
            );
        }
    });
}
