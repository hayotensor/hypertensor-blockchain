use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{assert_err, assert_ok, weights::WeightMeter};

const BOOTSTRAP_NODES: u32 = 4;
const NEW_NODE: u32 = BOOTSTRAP_NODES + 1;

fn queued_fixture() -> (u32, u32) {
    increase_epochs(50);
    let name = b"node-classifications".to_vec();
    let stake = MinSubnetMinStake::<Test>::get();
    let deposit = 10_000 * Network::percentage_factor_as_u128();
    build_activated_subnet(name.clone(), 0, BOOTSTRAP_NODES, deposit, stake);
    let subnet = SubnetName::<Test>::get(name).unwrap();
    build_queued_subnet_nodes(subnet, BOOTSTRAP_NODES, NEW_NODE, deposit, stake);
    seed_equal_validator_delegate_stake_for_subnet(subnet);
    ChurnLimit::<Test>::insert(subnet, 1);
    ChurnLimitMultiplier::<Test>::insert(subnet, 1);
    ProbationRounds::<Test>::insert(subnet, 2);
    let start = QueuedSubnetNodesData::<Test>::get(subnet, NEW_NODE)
        .classification
        .start_epoch;
    let maturity = start + SubnetNodeQueueEpochs::<Test>::get(subnet) + 1;
    (subnet, maturity)
}

fn probation_fixture() -> (u32, u32) {
    let (subnet, epoch) = queued_fixture();
    set_block_to_subnet_slot_epoch(epoch, subnet);
    Network::handle_registration_queue(&mut WeightMeter::new(), subnet, epoch);
    assert_eq!(
        SubnetNodesData::<Test>::get(subnet, NEW_NODE)
            .classification
            .node_class,
        SubnetNodeClass::Probation
    );
    assert_eq!(TotalAdmittedSubnetNodes::<Test>::get(subnet), NEW_NODE);
    assert_eq!(
        TotalSubnetElectableNodes::<Test>::get(subnet),
        BOOTSTRAP_NODES
    );
    (subnet, epoch)
}

fn elect(subnet: u32, epoch: u32) -> ElectedConsensusRound {
    set_block_to_subnet_slot_epoch(epoch, subnet);
    Network::elect_validator(subnet, epoch, System::block_number());
    SubnetElectedValidator::<Test>::get(subnet, epoch).expect("round elected")
}

fn propose_round(
    subnet: u32,
    epoch: u32,
    target_score: Option<u128>,
    zero_total: bool,
    votes: usize,
) -> ConsensusSubmissionData<Test> {
    let round = elect(subnet, epoch);
    let proposer = round.validator_subnet_node_id;
    let mut scores: Vec<_> = (1..=BOOTSTRAP_NODES)
        .map(|id| SubnetNodeConsensusData {
            subnet_node_id: id,
            score: if zero_total { 0 } else { 100 },
        })
        .collect();
    if let Some(score) = target_score {
        scores.push(SubnetNodeConsensusData {
            subnet_node_id: NEW_NODE,
            score,
        });
    }
    let hotkey = Network::get_subnet_node_associated_hotkey(subnet, proposer).unwrap();
    assert_ok!(Network::propose_attestation(
        RuntimeOrigin::signed(hotkey),
        subnet,
        scores,
        None,
        None,
        None,
        None
    ));
    for id in round
        .eligible_subnet_node_ids
        .iter()
        .copied()
        .filter(|id| *id != proposer)
        .take(votes.saturating_sub(1))
    {
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, id).unwrap();
        assert_ok!(Network::attest(
            RuntimeOrigin::signed(hotkey),
            subnet,
            id,
            None
        ));
    }
    let (submission, _) = Network::precheck_subnet_consensus_submission(
        subnet,
        epoch,
        Network::get_current_epoch_as_u32(),
    );
    submission.expect("proposal prechecks")
}

fn settle(subnet: u32, epoch: u32, submission: ConsensusSubmissionData<Test>) {
    set_block_to_subnet_slot_epoch(epoch + 1, subnet);
    Network::distribute_rewards(
        &mut WeightMeter::new(),
        subnet,
        epoch + 1,
        submission,
        RewardsData {
            subnet_node_rewards: Network::percentage_factor_as_u128(),
            ..Default::default()
        },
    );
}

fn promotion_events(subnet: u32) -> usize {
    System::events()
        .iter()
        .filter(|record| {
            matches!(
                &record.event,
                RuntimeEvent::Network(Event::SubnetNodePromoted { subnet_id, subnet_node_id, .. })
                    if *subnet_id == subnet && *subnet_node_id == NEW_NODE
            )
        })
        .count()
}

#[test]
fn queued_is_not_score_or_vote_eligible_and_admission_starts_probation() {
    new_test_ext().execute_with(|| {
        let (subnet, maturity) = queued_fixture();
        assert_eq!(
            QueuedSubnetNodesData::<Test>::get(subnet, NEW_NODE)
                .classification
                .node_class,
            SubnetNodeClass::Queued
        );
        assert_eq!(TotalSubnetNodes::<Test>::get(subnet), NEW_NODE);
        assert_eq!(
            TotalAdmittedSubnetNodes::<Test>::get(subnet),
            BOOTSTRAP_NODES
        );
        Network::handle_registration_queue(&mut WeightMeter::new(), subnet, maturity - 1);
        assert!(QueuedSubnetNodesData::<Test>::contains_key(
            subnet, NEW_NODE
        ));
        let submission = propose_round(subnet, maturity - 1, Some(100), false, 4);
        assert!(!submission
            .data
            .iter()
            .any(|entry| entry.subnet_node_id == NEW_NODE));
        assert!(!submission
            .subnet_nodes
            .iter()
            .any(|node| node.id == NEW_NODE));
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, NEW_NODE).unwrap();
        assert_err!(
            Network::attest(RuntimeOrigin::signed(hotkey), subnet, NEW_NODE, None),
            Error::<Test>::InvalidSubnetNodeId
        );
        settle(subnet, maturity - 1, submission);
        Network::handle_registration_queue(&mut WeightMeter::new(), subnet, maturity);
        let node = SubnetNodesData::<Test>::get(subnet, NEW_NODE);
        assert_eq!(
            node.classification,
            SubnetNodeClassification {
                node_class: SubnetNodeClass::Probation,
                start_epoch: maturity
            }
        );
        assert!(!QueuedSubnetNodesData::<Test>::contains_key(
            subnet, NEW_NODE
        ));
        assert!(SubnetNodeQueue::<Test>::get(subnet).is_empty());
        assert!(!SubnetNodeProbationProgress::<Test>::contains_key(
            subnet, NEW_NODE
        ));
        assert!(!NodeSlotIndex::<Test>::contains_key(subnet, NEW_NODE));
        let submission = propose_round(subnet, maturity, Some(100), false, 4);
        assert!(submission
            .data
            .iter()
            .any(|entry| entry.subnet_node_id == NEW_NODE));
        assert!(!SubnetConsensusSubmission::<Test>::get(subnet, maturity)
            .unwrap()
            .validator_ids
            .contains(&NEW_NODE));
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, NEW_NODE).unwrap();
        assert_err!(
            Network::attest(RuntimeOrigin::signed(hotkey), subnet, NEW_NODE, None),
            Error::<Test>::InvalidSubnetNodeClassification
        );
    });
}

#[test]
fn two_qualifying_rounds_promote_without_retroactive_node_or_delegate_rewards() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = probation_fixture();
        let one = Network::percentage_factor_as_u128();
        let validator = SubnetNodeValidatorId::<Test>::get(subnet, NEW_NODE).unwrap();
        let beneficiary = account(990_001);
        ValidatorsData::<Test>::mutate(validator, |data| {
            data.delegate_reward_rate = one / 2;
            data.delegate_account = Some(DelegateAccount {
                account_id: beneficiary.clone(),
                rate: one / 2,
            });
        });
        // Replace the voting-only fixture balance with a funded pool and real circulating shares.
        let seeded_balance = ValidatorDelegateStakeBalance::<Test>::take(validator);
        TotalValidatorDelegateStakeBalance::<Test>::mutate(|total| *total -= seeded_balance);
        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(account(0)),
            validator,
            one,
            1,
        ));
        let stake_before = NodeSubnetStake::<Test>::get(NEW_NODE, subnet);
        let pool_before = ValidatorDelegateStakeBalance::<Test>::get(validator);
        System::reset_events();
        // Individual zero scores count when the canonical round total is positive.
        let submission = propose_round(subnet, epoch, Some(0), false, 4);
        settle(subnet, epoch, submission);
        assert_eq!(
            SubnetNodeProbationProgress::<Test>::get(subnet, NEW_NODE),
            1
        );
        assert_eq!(promotion_events(subnet), 0);
        let submission = propose_round(subnet, epoch + 1, Some(100), false, 4);
        assert!(!SubnetConsensusSubmission::<Test>::get(subnet, epoch + 1)
            .unwrap()
            .validator_ids
            .contains(&NEW_NODE));
        settle(subnet, epoch + 1, submission);
        let node = SubnetNodesData::<Test>::get(subnet, NEW_NODE);
        assert_eq!(
            node.classification,
            SubnetNodeClassification {
                node_class: SubnetNodeClass::Active,
                start_epoch: epoch + 2
            }
        );
        assert!(!SubnetNodeProbationProgress::<Test>::contains_key(
            subnet, NEW_NODE
        ));
        assert_eq!(TotalSubnetElectableNodes::<Test>::get(subnet), NEW_NODE);
        assert_eq!(TotalElectableNodes::<Test>::get(), NEW_NODE);
        assert_eq!(TotalAdmittedSubnetNodes::<Test>::get(subnet), NEW_NODE);
        let slots = SubnetNodeElectionSlots::<Test>::get(subnet);
        assert_eq!(
            slots[NodeSlotIndex::<Test>::get(subnet, NEW_NODE).unwrap() as usize],
            NEW_NODE
        );
        assert_eq!(promotion_events(subnet), 1);
        assert_eq!(NodeSubnetStake::<Test>::get(NEW_NODE, subnet), stake_before);
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator),
            pool_before
        );
        assert_eq!(DelegateAccountStake::<Test>::get(&beneficiary), 0);
        // This is the operational phase of the same subnet hook that promoted the node.
        Network::emission_operational_step(
            &mut WeightMeter::new(),
            System::block_number(),
            epoch + 2,
            subnet,
        );
        assert!(SubnetElectedValidator::<Test>::get(subnet, epoch + 2)
            .unwrap()
            .eligible_subnet_node_ids
            .contains(&NEW_NODE));
        assert!(!SubnetElectedValidator::<Test>::get(subnet, epoch + 1)
            .unwrap()
            .eligible_subnet_node_ids
            .contains(&NEW_NODE));
        // Once captured as Active, this same node and both configured reward destinations pay.
        let submission = propose_round(subnet, epoch + 2, Some(100), false, 5);
        settle(subnet, epoch + 2, submission);
        assert!(NodeSubnetStake::<Test>::get(NEW_NODE, subnet) > stake_before);
        assert!(ValidatorDelegateStakeBalance::<Test>::get(validator) > pool_before);
        assert!(DelegateAccountStake::<Test>::get(&beneficiary) > 0);
        assert_eq!(promotion_events(subnet), 1);
    });
}

#[test]
fn promotion_requires_full_reputation_and_endorsed_omission_resets_progress() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = probation_fixture();
        SubnetReputationFactorSchedules::<Test>::mutate(subnet, |schedule| {
            schedule.current.included_increase = 0
        });
        SubnetNodeReputation::<Test>::insert(
            subnet,
            NEW_NODE,
            Network::percentage_factor_as_u128() / 2,
        );
        for offset in 0..2 {
            let submission = propose_round(subnet, epoch + offset, Some(100), false, 4);
            settle(subnet, epoch + offset, submission);
        }
        assert_eq!(
            SubnetNodeProbationProgress::<Test>::get(subnet, NEW_NODE),
            2
        );
        assert_eq!(
            SubnetNodesData::<Test>::get(subnet, NEW_NODE)
                .classification
                .node_class,
            SubnetNodeClass::Probation
        );
        assert_eq!(promotion_events(subnet), 0);
        let submission = propose_round(subnet, epoch + 2, None, false, 4);
        settle(subnet, epoch + 2, submission);
        assert!(!SubnetNodeProbationProgress::<Test>::contains_key(
            subnet, NEW_NODE
        ));
        assert!(!NodeSlotIndex::<Test>::contains_key(subnet, NEW_NODE));
    });
}

#[test]
fn nonqualifying_rounds_freeze_progress_including_emergency_omissions() {
    for case in [
        "rejected",
        "zero_total",
        "below_identity",
        "emergency_scored",
        "emergency_omitted",
        "missing",
    ] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = probation_fixture();
            SubnetNodeProbationProgress::<Test>::insert(subnet, NEW_NODE, 1);
            let stake_before = NodeSubnetStake::<Test>::get(NEW_NODE, subnet);
            if case.starts_with("emergency") {
                EmergencySubnetNodeElectionData::<Test>::insert(
                    subnet,
                    EmergencySubnetValidatorData {
                        subnet_node_ids: (1..=BOOTSTRAP_NODES).collect(),
                        target_emergency_validators_epochs: 10,
                        max_emergency_validators_epoch: epoch + 20,
                        total_epochs: 0,
                        activated: true,
                        started_subnet_epoch: epoch,
                        reputation_factors: Network::get_reputation_factors_for_epoch(
                            subnet, epoch,
                        ),
                        min_subnet_node_reputation: MinSubnetNodeReputation::<Test>::get(subnet),
                        min_weight_decrease_reputation_threshold: 0,
                    },
                );
            }
            if case == "missing" {
                elect(subnet, epoch);
                set_block_to_subnet_slot_epoch(epoch + 1, subnet);
                Network::emission_settlement_step(
                    &mut WeightMeter::new(),
                    System::block_number(),
                    Network::get_current_epoch_as_u32(),
                    epoch + 1,
                    subnet,
                );
            } else {
                let score = match case {
                    "emergency_omitted" | "below_identity" => None,
                    "zero_total" => Some(0),
                    _ => Some(100),
                };
                let votes = match case {
                    "rejected" => 1,
                    "below_identity" => 3,
                    _ => 4,
                };
                let submission = propose_round(subnet, epoch, score, case == "zero_total", votes);
                settle(subnet, epoch, submission);
            }
            assert_eq!(
                SubnetNodeProbationProgress::<Test>::get(subnet, NEW_NODE),
                1,
                "{case}"
            );
            assert_eq!(
                SubnetNodesData::<Test>::get(subnet, NEW_NODE)
                    .classification
                    .node_class,
                SubnetNodeClass::Probation,
                "{case}"
            );
            assert_eq!(
                NodeSubnetStake::<Test>::get(NEW_NODE, subnet),
                stake_before,
                "{case}"
            );
            assert_eq!(promotion_events(subnet), 0, "{case}");
        });
    }
}

#[test]
fn quarantined_and_removed_probation_nodes_never_progress_or_receive_rewards() {
    for remove in [false, true] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = probation_fixture();
            SubnetNodeProbationProgress::<Test>::insert(subnet, NEW_NODE, 1);
            let submission = propose_round(subnet, epoch, Some(100), false, 4);
            let stake_before = NodeSubnetStake::<Test>::get(NEW_NODE, subnet);
            if remove {
                assert!(Network::remove_admitted_subnet_node(subnet, NEW_NODE));
                assert!(!SubnetNodeProbationProgress::<Test>::contains_key(
                    subnet, NEW_NODE
                ));
            } else {
                PendingAdmittedNodeRemovals::<Test>::mutate(subnet, |pending| {
                    pending.try_insert(NEW_NODE).unwrap();
                });
            }
            settle(subnet, epoch, submission);
            assert_eq!(NodeSubnetStake::<Test>::get(NEW_NODE, subnet), stake_before);
            assert_eq!(
                SubnetNodeProbationProgress::<Test>::get(subnet, NEW_NODE),
                if remove { 0 } else { 1 }
            );
            assert!(!NodeSlotIndex::<Test>::contains_key(subnet, NEW_NODE));
            assert_eq!(promotion_events(subnet), 0);
        });
    }
}

#[test]
fn promotion_rolls_back_on_failure_and_cannot_repeat_or_bypass_quarantine() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = probation_fixture();
        SubnetNodeProbationProgress::<Test>::insert(subnet, NEW_NODE, 2);
        let before = SubnetNodesData::<Test>::get(subnet, NEW_NODE);
        let slots = SubnetNodeElectionSlots::<Test>::get(subnet);
        TotalElectableNodes::<Test>::put(u32::MAX);
        System::reset_events();
        assert!(!Network::promote_probation_node_to_active(
            subnet,
            NEW_NODE,
            epoch,
            &Default::default()
        ));
        assert_eq!(SubnetNodesData::<Test>::get(subnet, NEW_NODE), before);
        assert_eq!(SubnetNodeElectionSlots::<Test>::get(subnet), slots);
        assert_eq!(
            TotalSubnetElectableNodes::<Test>::get(subnet),
            BOOTSTRAP_NODES
        );
        assert_eq!(TotalElectableNodes::<Test>::get(), u32::MAX);
        assert_eq!(
            SubnetNodeProbationProgress::<Test>::get(subnet, NEW_NODE),
            2
        );
        assert!(!NodeSlotIndex::<Test>::contains_key(subnet, NEW_NODE));
        assert_eq!(promotion_events(subnet), 0);
        TotalElectableNodes::<Test>::put(BOOTSTRAP_NODES);
        let pending = [NEW_NODE]
            .into_iter()
            .collect::<BTreeSet<_>>()
            .try_into()
            .unwrap();
        assert!(!Network::promote_probation_node_to_active(
            subnet, NEW_NODE, epoch, &pending
        ));
        assert!(Network::promote_probation_node_to_active(
            subnet,
            NEW_NODE,
            epoch,
            &Default::default()
        ));
        assert!(!Network::promote_probation_node_to_active(
            subnet,
            NEW_NODE,
            epoch,
            &Default::default()
        ));
        assert_eq!(TotalSubnetElectableNodes::<Test>::get(subnet), NEW_NODE);
        assert_eq!(TotalElectableNodes::<Test>::get(), NEW_NODE);
        assert_eq!(promotion_events(subnet), 1);
    });
}

#[test]
fn bootstrap_registration_enters_active_without_probation() {
    new_test_ext().execute_with(|| {
        let name = b"classification-bootstrap".to_vec();
        build_registered_subnet(
            name.clone(),
            0,
            BOOTSTRAP_NODES,
            10_000 * Network::percentage_factor_as_u128(),
            MinSubnetMinStake::<Test>::get(),
            true,
            None,
        );
        let subnet = SubnetName::<Test>::get(name).unwrap();
        assert_eq!(
            SubnetsData::<Test>::get(subnet).unwrap().state,
            SubnetState::Registered
        );
        for id in 1..=BOOTSTRAP_NODES {
            assert_eq!(
                SubnetNodesData::<Test>::get(subnet, id)
                    .classification
                    .node_class,
                SubnetNodeClass::Active
            );
            assert!(!QueuedSubnetNodesData::<Test>::contains_key(subnet, id));
            assert!(!SubnetNodeProbationProgress::<Test>::contains_key(
                subnet, id
            ));
            assert!(NodeSlotIndex::<Test>::contains_key(subnet, id));
        }
        assert_eq!(
            TotalAdmittedSubnetNodes::<Test>::get(subnet),
            BOOTSTRAP_NODES
        );
        assert_eq!(
            TotalSubnetElectableNodes::<Test>::get(subnet),
            BOOTSTRAP_NODES
        );
        assert!(SubnetNodeQueue::<Test>::get(subnet).is_empty());
        assert_eq!(ProbationRounds::<Test>::get(subnet), 2);
    });
}

#[test]
fn native_and_rpc_classification_encoding_and_effective_epoch_agree() {
    use codec::{Decode, Encode};
    for (class, rpc_class, code) in [
        (
            SubnetNodeClass::Queued,
            network_rpc_types::SubnetNodeClass::Queued,
            0u8,
        ),
        (
            SubnetNodeClass::Probation,
            network_rpc_types::SubnetNodeClass::Probation,
            1,
        ),
        (
            SubnetNodeClass::Active,
            network_rpc_types::SubnetNodeClass::Active,
            2,
        ),
    ] {
        assert_eq!(class.encode(), vec![code]);
        assert_eq!(class.encode(), rpc_class.encode());
        assert_eq!(SubnetNodeClass::decode(&mut &[code][..]).unwrap(), class);
        let node = SubnetNode {
            classification: SubnetNodeClassification {
                node_class: class,
                start_epoch: 10,
            },
            ..Default::default()
        };
        assert!(!node.has_classification(&class, 9));
        assert!(node.has_classification(&class, 10));
        assert_eq!(
            node.has_classification(&SubnetNodeClass::Probation, 10),
            code >= 1
        );
        assert_eq!(
            node.has_classification(&SubnetNodeClass::Active, 10),
            code == 2
        );
    }
    assert!(SubnetNodeClass::decode(&mut &[3u8][..]).is_err());
}

#[test]
fn scheduled_probation_requirement_does_not_rewrite_an_elected_round() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = probation_fixture();
        assert_eq!(elect(subnet, epoch).policy.probation_rounds, 2);
        let owner = SubnetOwner::<Test>::get(subnet).unwrap();
        assert_ok!(Network::owner_update_probation_rounds(
            RuntimeOrigin::signed(owner),
            subnet,
            1
        ));
        assert_eq!(Network::get_probation_rounds_for_epoch(subnet, epoch), 2);
        assert_eq!(
            Network::get_probation_rounds_for_epoch(subnet, epoch + 1),
            1
        );
        let submission = propose_round(subnet, epoch, Some(100), false, 4);
        assert_eq!(submission.policy.probation_rounds, 2);
        settle(subnet, epoch, submission);
        assert_eq!(
            SubnetNodesData::<Test>::get(subnet, NEW_NODE)
                .classification
                .node_class,
            SubnetNodeClass::Probation
        );
        assert_eq!(
            SubnetNodeProbationProgress::<Test>::get(subnet, NEW_NODE),
            1
        );
        let submission = propose_round(subnet, epoch + 1, Some(100), false, 4);
        assert_eq!(submission.policy.probation_rounds, 1);
        settle(subnet, epoch + 1, submission);
        assert_eq!(
            SubnetNodesData::<Test>::get(subnet, NEW_NODE)
                .classification
                .node_class,
            SubnetNodeClass::Active
        );
    });
}
