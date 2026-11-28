use super::{mock::*, test_utils::*};
use crate::{consensus::matrix::math, *};
use codec::Encode;
use frame_support::{
    assert_noop, assert_ok,
    traits::{Currency, Get, Hooks},
    weights::Weight,
};

const Q: u128 = PERCENTAGE_FACTOR_U128;

#[test]
fn commitment_matches_independently_encoded_scale_test_vector() {
    // Independently encoded with little-endian integers, SCALE vector lengths,
    // fixed-size tag/salt, and Python hashlib.blake2b(digest_size=32).
    let round = pure_round(3);
    let scores: MatrixScores = vec![
        SubnetNodeConsensusData {
            subnet_node_id: 2,
            score: 5,
        },
        SubnetNodeConsensusData {
            subnet_node_id: 3,
            score: 10,
        },
    ]
    .try_into()
    .unwrap();
    let hash = Network::matrix_commitment(7, 11, 1, &round, &scores, &[0x42; 32]);
    assert_eq!(
        format!("{hash:x}"),
        "fee5e7c5835d70e94b468107b8f1bade5dda6ba3d6f85f2ff94406105b7f443c"
    );
}

fn pure_round(count: u32) -> MatrixRound {
    MatrixRound {
        start_block: 0,
        reveal_block: 5,
        end_block: 10,
        nodes: (1..=count)
            .map(|id| SubnetNode {
                id,
                validator_id: id,
                classification: SubnetNodeClassification {
                    node_class: SubnetNodeClass::Active,
                    start_epoch: 0,
                },
            })
            .collect(),
        identities: (1..=count).map(|id| (id, id)).collect(),
        stake: ConsensusAttestorWeightSnapshot {
            weights: (1..=count).map(|id| (id, 1)).collect(),
            total_weight: count as u128,
        },
        policy: ConsensusPolicySnapshot::default(),
        parameters: MatrixParameters {
            accuracy_weight: Q / 2,
            ..DefaultMatrixConfig::<Test>::get()
        },
    }
}

fn scores(entries: &[(u32, u128)]) -> MatrixScores {
    entries
        .iter()
        .map(|&(subnet_node_id, score)| SubnetNodeConsensusData {
            subnet_node_id,
            score,
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

fn normalized(entries: &[(u32, u128)]) -> MatrixScores {
    math::normalize(
        &entries
            .iter()
            .map(|(id, value)| (*id, U256::from(*value)))
            .collect(),
    )
    .into_iter()
    .map(|(subnet_node_id, score)| SubnetNodeConsensusData {
        subnet_node_id,
        score,
    })
    .collect::<Vec<_>>()
    .try_into()
    .unwrap()
}

fn uniform(round: &MatrixRound) -> BTreeMap<u32, MatrixScores> {
    round
        .identities
        .keys()
        .map(|&id| {
            (
                id,
                normalized(
                    &round
                        .nodes
                        .iter()
                        .filter(|n| n.id != id)
                        .map(|n| (n.id, 1))
                        .collect::<Vec<_>>(),
                ),
            )
        })
        .collect()
}

fn fixture() -> (u32, u32) {
    increase_epochs(50);
    let name = b"matrix-fixture".to_vec();
    let mut data = default_registration_subnet_data(
        get_subnet_id_key_offset(TotalSubnetUids::<Test>::get() + 1),
        MaxSubnetNodes::<Test>::get(),
        name.clone(),
        0,
        4,
    );
    data.consensus_mechanism = ConsensusMechanism::Matrix;
    build_registered_subnet(name.clone(), 0, 4, 10_000 * Q, 100 * Q, true, Some(data));
    let subnet = SubnetName::<Test>::get(name).unwrap();
    let owner = SubnetOwner::<Test>::get(subnet).unwrap();
    assert_eq!(
        SubnetsData::<Test>::get(subnet)
            .unwrap()
            .consensus_mechanism,
        ConsensusMechanism::Matrix
    );
    let registration = SubnetRegistrationEpoch::<Test>::get(subnet).unwrap();
    System::set_block_number(
        (registration + MinSubnetRegistrationEpochs::<Test>::get()) * EpochLength::get(),
    );
    assert_ok!(Network::activate_subnet(
        RuntimeOrigin::signed(owner),
        subnet
    ));
    for node in 1..=4 {
        let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
        assert_ok!(Network::handle_increase_account_validator_delegate_stake(
            &account(99),
            validator,
            100 * Q
        ));
    }
    let epoch = SubnetsData::<Test>::get(subnet)
        .unwrap()
        .consensus_eligible_from_subnet_epoch
        .unwrap();
    set_block_to_subnet_slot_epoch(epoch, subnet);
    let consumed = Network::on_initialize(System::block_number());
    assert!(consumed.all_lte(MaximumHooksWeight::get()));
    assert!(MatrixRounds::<Test>::contains_key(subnet, epoch));
    assert!(!SubnetElectedValidator::<Test>::contains_key(subnet, epoch));
    FinalSubnetEmissionWeights::<Test>::insert(
        epoch + 1,
        DistributionData {
            subnets_emissions: 10 * Q,
            subnet_weights: BTreeMap::from([(subnet, Q)]),
        },
    );
    (subnet, epoch)
}

fn raw_row(round: &MatrixRound, node: u32, score: u128) -> MatrixScores {
    round
        .nodes
        .iter()
        .filter(|n| n.id != node)
        .map(|n| SubnetNodeConsensusData {
            subnet_node_id: n.id,
            score,
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

fn submit(subnet: u32, epoch: u32, nodes: &[u32]) {
    let round = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
    let rows = nodes
        .iter()
        .map(|&node| (node, raw_row(&round, node, Q / 2)))
        .collect();
    submit_rows(subnet, epoch, &rows);
}

fn submit_rows(subnet: u32, epoch: u32, rows: &BTreeMap<u32, MatrixScores>) {
    let round = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
    System::set_block_number(round.start_block);
    for (&node, row) in rows {
        let hash = Network::matrix_commitment(subnet, epoch, node, &round, row, &[node as u8; 32]);
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
        assert_ok!(Network::commit_matrix_scores(
            RuntimeOrigin::signed(hotkey),
            subnet,
            epoch,
            node,
            hash
        ));
    }
    System::set_block_number(round.reveal_block);
    for (&node, row) in rows {
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
        assert_ok!(Network::reveal_matrix_scores(
            RuntimeOrigin::signed(hotkey),
            subnet,
            epoch,
            node,
            row.clone(),
            [node as u8; 32]
        ));
    }
}

#[test]
fn normalization_is_exact_scale_invariant_and_full_width() {
    let a = normalized(&[(1, 1), (2, 2), (3, 3)]);
    assert_eq!(a, normalized(&[(1, 100), (2, 200), (3, 300)]));
    assert_eq!(a.iter().map(|s| s.score).sum::<u128>(), Q);
    let maximum = normalized(&[(1, u128::MAX), (2, u128::MAX), (3, u128::MAX)]);
    assert_eq!(maximum.iter().map(|s| s.score).sum::<u128>(), Q);
    assert!(maximum[0].score >= maximum[1].score);
    assert!(normalized(&[(1, 0), (2, 0)]).iter().all(|s| s.score == 0));
}

#[test]
fn matching_independent_vectors_have_perfect_accuracy() {
    let round = pure_round(3);
    let result = math::calculate(&round, &uniform(&round));
    assert_eq!(result.outcome, MatrixOutcome::Accepted);
    assert!(result
        .nodes
        .values()
        .all(|n| n.accuracy == Q && n.performance == Q / 2));
    assert_eq!(result.nodes.values().map(|n| n.weight).sum::<u128>(), Q);
}

#[test]
fn received_performance_uses_scorers_stake_and_excludes_self() {
    let mut round = pure_round(3);
    round.stake.weights = BTreeMap::from([(1, 100), (2, 1), (3, 3)]);
    round.stake.total_weight = 104;
    let rows = BTreeMap::from([
        (1, normalized(&[(2, 1), (3, 1)])),
        (2, normalized(&[(1, 1), (3, 0)])),
        (3, normalized(&[(1, 0), (2, 1)])),
    ]);
    let result = math::calculate(&round, &rows);
    assert_eq!(result.nodes[&1].performance, Q / 4);
}

#[test]
fn partial_scores_use_only_each_targets_assessors_and_preserve_absolute_values() {
    let mut round = pure_round(10);
    round.stake.weights.insert(1, 1000);
    round.stake.weights.insert(2, 500);
    round.stake.total_weight = 1508;
    let rows = BTreeMap::from([
        (1, scores(&[(3, Q / 20), (4, 7 * Q / 10)])),
        (2, scores(&[(3, Q / 5)])),
        (5, scores(&[(6, Q)])), // An unrelated sector cannot dilute targets 3 or 4.
    ]);
    let result = math::calculate(&round, &rows);
    assert_eq!(result.nodes[&3].performance, Q / 10);
    assert_eq!(result.nodes[&4].performance, 7 * Q / 10);
    assert_eq!(result.nodes[&6].performance, Q);
    assert_eq!(result.nodes[&1].accuracy, 85 * Q / 100);
    assert_eq!(result.nodes[&2].accuracy, 85 * Q / 100);
    assert!(!result.nodes[&5].has_accuracy_reference);
    for id in [1, 2, 5, 7, 8, 9, 10] {
        assert!(!result.nodes[&id].has_received_assessment);
        assert_eq!(result.nodes[&id].weight, 0);
    }
    assert_eq!(result.nodes.values().map(|n| n.weight).sum::<u128>(), Q);

    let mut with_zero = rows;
    with_zero.insert(2, scores(&[(3, Q / 5), (4, 0)]));
    let result = math::calculate(&round, &with_zero);
    assert_eq!(result.nodes[&4].performance, 7 * Q / 15);
    // Two comparable targets: |.05-.2| and |.7-0|, mean error .425.
    assert_eq!(result.nodes[&1].accuracy, 575 * Q / 1000);
}

#[test]
fn receiving_nine_assessments_and_scoring_one_target_has_no_coverage_penalty() {
    let round = pure_round(10);
    let mut rows: BTreeMap<_, _> = (2..=10)
        .map(|id| (id, scores(&[(1, 7 * Q / 10)])))
        .collect();
    rows.insert(1, scores(&[(2, Q / 5)]));
    rows.insert(3, scores(&[(1, 7 * Q / 10), (2, Q / 5)]));
    let result = math::calculate(&round, &rows);
    let node = &result.nodes[&1];
    assert_eq!(node.performance, 7 * Q / 10);
    assert_eq!(node.accuracy, Q);
    assert!(node.has_received_assessment && node.has_accuracy_reference && node.weight > 0);
    assert_eq!(result.nodes[&2].performance, Q / 5);
    assert_eq!(result.nodes[&3].weight, 0); // Accurate scoring cannot replace being assessed.
}

#[test]
fn disjoint_sectors_and_single_assessor_targets_need_no_accuracy_reference() {
    let round = pure_round(6);
    let rows = BTreeMap::from([
        (1, scores(&[(2, Q / 4)])),
        (2, scores(&[(1, Q / 2)])),
        (3, scores(&[(4, Q)])),
        (4, scores(&[(3, Q / 2)])),
    ]);
    let result = math::calculate(&round, &rows);
    assert_eq!(result.outcome, MatrixOutcome::Accepted);
    for id in 1..=4 {
        assert!(result.nodes[&id].has_received_assessment);
        assert!(!result.nodes[&id].has_accuracy_reference);
        assert!(result.nodes[&id].weight > 0);
    }
    for id in [5, 6] {
        assert!(!result.nodes[&id].has_received_assessment);
        assert_eq!(result.nodes[&id].weight, 0);
    }
    assert_eq!(result.nodes.values().map(|n| n.weight).sum::<u128>(), Q);
}

#[test]
fn zero_only_rows_are_assessments_and_zero_references_are_usable() {
    let round = pure_round(3);
    let rows = BTreeMap::from([
        (1, scores(&[(2, 0), (3, 0)])),
        (2, scores(&[(1, 0), (3, 0)])),
        (3, scores(&[(1, 0), (2, 0)])),
    ]);
    let result = math::calculate(&round, &rows);
    assert_eq!(result.outcome, MatrixOutcome::Accepted);
    assert_eq!(result.revealed_nodes, 3);
    assert!(result.nodes.values().all(|n| n.performance == 0
        && n.has_received_assessment
        && n.has_accuracy_reference
        && n.accuracy == Q
        && n.weight > 0));
    let mut disagrees = rows;
    disagrees.insert(1, scores(&[(2, Q), (3, Q)]));
    assert_eq!(math::calculate(&round, &disagrees).nodes[&1].accuracy, 0);
}

#[test]
fn sparse_weighted_arithmetic_handles_maximum_stake_and_zero_weight_assessors() {
    let mut round = pure_round(4);
    round.stake.weights = BTreeMap::from([(1, u128::MAX - 1), (2, 1), (3, 0), (4, 0)]);
    round.stake.total_weight = u128::MAX;
    let rows = BTreeMap::from([
        (1, scores(&[(3, Q)])),
        (2, scores(&[(3, Q)])),
        (3, scores(&[(1, Q), (4, Q)])),
    ]);
    let result = math::calculate(&round, &rows);
    assert_eq!(result.nodes[&3].performance, Q);
    assert_eq!(result.nodes[&3].weight, Q);
    assert_eq!(result.nodes[&1].accuracy, Q);
    assert_eq!(result.nodes[&2].accuracy, Q);
    assert!(!result.nodes[&1].has_received_assessment);
    assert!(!result.nodes[&4].has_received_assessment);
}

#[test]
fn partial_reveals_settle_and_omitted_nodes_keep_reputation_and_receive_no_rewards() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        SubnetNodeReputation::<Test>::insert(subnet, 4, 80 * Q / 100);
        let before: Vec<_> = (1..=4)
            .map(|id| NodeSubnetStake::<Test>::get(id, subnet))
            .collect();
        let rows = BTreeMap::from([
            (1, scores(&[(2, Q / 5)])),
            (2, scores(&[(1, 7 * Q / 10)])),
            (3, scores(&[(1, 7 * Q / 10), (2, Q / 5)])),
        ]);
        submit_rows(subnet, epoch, &rows);
        for (&id, row) in &rows {
            assert_eq!(
                MatrixRows::<Test>::get((subnet, epoch, id)),
                Some(row.clone())
            );
        }
        Network::settle_matrix_round(subnet, epoch + 1);
        let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(result.outcome, MatrixOutcome::Accepted);
        assert_eq!(result.nodes[&1].performance, 7 * Q / 10);
        assert_eq!(result.nodes[&2].performance, Q / 5);
        for id in [1, 2] {
            assert_eq!(result.nodes[&id].accuracy, Q);
            assert!(NodeSubnetStake::<Test>::get(id, subnet) > before[id as usize - 1]);
        }
        for id in [3, 4] {
            assert!(!result.nodes[&id].has_received_assessment);
            assert_eq!(result.nodes[&id].weight, 0);
            assert_eq!(
                NodeSubnetStake::<Test>::get(id, subnet),
                before[id as usize - 1]
            );
        }
        assert_eq!(
            SubnetNodeReputation::<Test>::get(subnet, 4),
            Some(80 * Q / 100)
        );
    });
}

#[test]
fn sparse_validation_rejects_invalid_entries_at_reveal_and_settlement() {
    for invalid in [
        scores(&[(1, Q)]),     // Self.
        scores(&[(99, Q)]),    // Outside the roster.
        scores(&[(2, Q + 1)]), // Above the absolute scale.
        scores(&[(2, u128::MAX)]),
        scores(&[(2, Q), (2, 0)]), // Duplicate.
        scores(&[(3, Q), (2, Q)]), // Out of order.
    ] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = fixture();
            let round = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
            let commitment =
                Network::matrix_commitment(subnet, epoch, 1, &round, &invalid, &[1; 32]);
            let hotkey = Network::get_subnet_node_associated_hotkey(subnet, 1).unwrap();
            assert_ok!(Network::commit_matrix_scores(
                RuntimeOrigin::signed(hotkey.clone()),
                subnet,
                epoch,
                1,
                commitment
            ));
            System::set_block_number(round.reveal_block);
            assert_noop!(
                Network::reveal_matrix_scores(
                    RuntimeOrigin::signed(hotkey),
                    subnet,
                    epoch,
                    1,
                    invalid.clone(),
                    [1; 32]
                ),
                Error::<Test>::InvalidMatrixScores
            );
            assert!(!MatrixRows::<Test>::contains_key((subnet, epoch, 1)));
            MatrixRows::<Test>::insert((subnet, epoch, 1), invalid.clone());
            Network::settle_matrix_round(subnet, epoch + 1);
            assert_eq!(
                MatrixResults::<Test>::get(subnet, epoch).unwrap().outcome,
                MatrixOutcome::Cancelled
            );
        });
    }
}

#[test]
fn sparse_commitment_binds_omissions_and_zero_reveal_is_not_abstention() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        let round = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
        let row = scores(&[(2, 0)]);
        let commitment = Network::matrix_commitment(subnet, epoch, 1, &round, &row, &[1; 32]);
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, 1).unwrap();
        assert_ok!(Network::commit_matrix_scores(
            RuntimeOrigin::signed(hotkey.clone()),
            subnet,
            epoch,
            1,
            commitment
        ));
        System::set_block_number(round.reveal_block);
        for changed in [scores(&[]), scores(&[(2, 0), (3, 0)])] {
            assert_noop!(
                Network::reveal_matrix_scores(
                    RuntimeOrigin::signed(hotkey.clone()),
                    subnet,
                    epoch,
                    1,
                    changed,
                    [1; 32]
                ),
                Error::<Test>::RevealMismatch
            );
        }
        assert_ok!(Network::reveal_matrix_scores(
            RuntimeOrigin::signed(hotkey),
            subnet,
            epoch,
            1,
            row,
            [1; 32]
        ));
        System::assert_last_event(
            Event::<Test>::MatrixScoresRevealed {
                subnet_id: subnet,
                subnet_epoch: epoch,
                subnet_node_id: 1,
                abstained: false,
            }
            .into(),
        );
        SubnetNodeReputation::<Test>::insert(subnet, 2, 80 * Q / 100);
        SubnetNodeReputation::<Test>::insert(subnet, 3, 80 * Q / 100);
        Network::settle_matrix_round(subnet, epoch + 1);
        let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
        assert!(result.nodes[&2].has_received_assessment);
        assert!(!result.nodes[&3].has_received_assessment);
        assert!(SubnetNodeReputation::<Test>::get(subnet, 2).unwrap() < 80 * Q / 100);
        assert_eq!(
            SubnetNodeReputation::<Test>::get(subnet, 3),
            Some(80 * Q / 100)
        );
    });
}

#[test]
fn assessed_workers_share_performance_rewards_without_submitting_scores() {
    let mut round = pure_round(3);
    let rows = BTreeMap::from([(1, normalized(&[(2, 1), (3, 3)]))]);
    for accuracy_weight in [0, Q / 2, Q] {
        round.parameters.accuracy_weight = accuracy_weight;
        let result = math::calculate(&round, &rows);
        assert_eq!(result.outcome, MatrixOutcome::Accepted);
        assert!(!result.nodes[&1].has_received_assessment);
        assert_eq!(result.nodes[&1].weight, 0);
        for (id, performance) in [(2, Q / 4), (3, 3 * Q / 4)] {
            let node = &result.nodes[&id];
            assert!(node.has_received_assessment);
            assert_eq!(node.performance, performance);
            assert_eq!(node.accuracy, 0);
            assert!(!node.has_accuracy_reference);
            // A deliberately disabled performance component cannot pay performance rewards.
            assert_eq!(
                node.weight,
                if accuracy_weight == Q { 0 } else { performance }
            );
        }
    }
}

#[test]
fn explicit_zero_assessment_allows_accuracy_rewards_but_missing_assessment_does_not() {
    let round = pure_round(3);
    let rows = BTreeMap::from([
        (1, normalized(&[(2, 1), (3, 1)])),
        (2, normalized(&[(1, 0), (3, 1)])),
        (3, normalized(&[(1, 0), (2, 1)])),
    ]);
    let assessed = math::calculate(&round, &rows);
    let node = &assessed.nodes[&1];
    assert!(node.has_received_assessment);
    assert_eq!(node.performance, 0);
    assert_eq!(node.accuracy, Q / 2);
    assert!(node.weight > 0);
    let alone = math::calculate(&round, &BTreeMap::from([(1, rows[&1].clone())]));
    assert_eq!(alone.nodes[&1].performance, 0);
    assert!(!alone.nodes[&1].has_received_assessment);
    assert_eq!(alone.nodes[&1].weight, 0);
}

#[test]
fn changing_sibling_scores_changes_the_node_based_accuracy_reference() {
    let mut round = pure_round(4);
    round.identities.insert(2, 1);
    round.nodes[1].validator_id = 1;
    let mut rows = uniform(&round);
    let before = math::calculate(&round, &rows).nodes[&1].accuracy;
    rows.insert(2, normalized(&[(1, 1000), (3, 0), (4, 0)]));
    assert_ne!(before, math::calculate(&round, &rows).nodes[&1].accuracy);
}

#[test]
fn blend_endpoints_and_surviving_factor_are_normalized() {
    let mut round = pure_round(3);
    let rows = BTreeMap::from([
        (1, normalized(&[(2, 1), (3, 0)])),
        (2, normalized(&[(1, 1), (3, 0)])),
        (3, normalized(&[(1, 1), (2, 0)])),
    ]);
    round.parameters.accuracy_weight = 0;
    let performance_only = math::calculate(&round, &rows);
    assert_eq!(performance_only.nodes[&3].weight, 0);
    assert_eq!(
        performance_only
            .nodes
            .values()
            .map(|n| n.weight)
            .sum::<u128>(),
        Q
    );
    round.parameters.accuracy_weight = Q;
    let accuracy_only = math::calculate(&round, &rows);
    assert_eq!(
        accuracy_only.nodes.values().map(|n| n.weight).sum::<u128>(),
        Q
    );
    // All performance is assigned to a Probation target, so the accuracy contribution survives.
    let mut probation = pure_round(4);
    probation.nodes[3].classification.node_class = SubnetNodeClass::Probation;
    probation.identities.remove(&4);
    probation.stake.weights.remove(&4);
    probation.stake.total_weight = 3;
    let rows = (1..=3)
        .map(|id| {
            (
                id,
                normalized(
                    &probation
                        .nodes
                        .iter()
                        .filter(|n| n.id != id)
                        .map(|n| (n.id, u128::from(n.id == 4)))
                        .collect::<Vec<_>>(),
                ),
            )
        })
        .collect();
    let result = math::calculate(&probation, &rows);
    assert_eq!(result.nodes[&4].weight, 0);
    assert_eq!(result.nodes.values().map(|n| n.weight).sum::<u128>(), Q);
}

#[test]
fn any_nonzero_reveal_is_accepted_regardless_of_turnout_stake_share_or_identity_count() {
    let mut round = pure_round(64);
    // Two revealers have a negligible share of total frozen voting stake.
    round.stake.weights.values_mut().for_each(|w| *w = Q);
    round.stake.weights.insert(1, 1);
    round.stake.weights.insert(2, 1);
    round.stake.total_weight = 62 * Q + 2;
    for node in &mut round.nodes {
        node.validator_id = 1;
        round.identities.insert(node.id, 1);
    }
    let all = uniform(&round);
    for count in [1, 2] {
        let rows = all
            .iter()
            .take(count)
            .map(|(id, row)| (*id, row.clone()))
            .collect();
        let result = math::calculate(&round, &rows);
        assert_eq!(result.outcome, MatrixOutcome::Accepted);
        assert_eq!(result.revealed_nodes, count as u32);
        assert_eq!(result.revealed_weight, count as u128);
        assert!(result
            .nodes
            .range(3..)
            .all(|(_, node)| node.weight > 0 && node.has_received_assessment));
        assert_eq!(result.nodes.values().map(|n| n.weight).sum::<u128>(), Q);
        if count == 1 {
            assert!(!result.nodes[&1].has_received_assessment);
            assert_eq!(result.nodes[&1].weight, 0);
        }
    }
}

#[test]
fn identical_node_rows_and_weights_produce_identical_results_under_any_ownership() {
    let mut round = pure_round(4);
    let rows = BTreeMap::from([
        (1, normalized(&[(2, 8), (3, 1), (4, 1)])),
        (2, normalized(&[(1, 3), (3, 5), (4, 2)])),
        (3, normalized(&[(1, 2), (2, 4), (4, 4)])),
        (4, normalized(&[(1, 6), (2, 1), (3, 3)])),
    ]);
    let expected = math::calculate(&round, &rows);
    for node in &mut round.nodes {
        node.validator_id = 1;
        round.identities.insert(node.id, 1);
    }
    assert_eq!(math::calculate(&round, &rows), expected);
}

#[test]
fn empty_and_abstaining_rounds_have_no_scores_but_zero_stake_reveals_are_valid() {
    let mut round = pure_round(4);
    let empty = BTreeMap::new();
    assert_eq!(
        math::calculate(&round, &empty).outcome,
        MatrixOutcome::NoScores
    );
    let abstentions = round
        .nodes
        .iter()
        .map(|node| (node.id, MatrixScores::default()))
        .collect();
    let result = math::calculate(&round, &abstentions);
    assert_eq!(result.outcome, MatrixOutcome::NoScores);
    assert_eq!(result.revealed_nodes, 0);
    round.stake.weights.values_mut().for_each(|w| *w = 0);
    round.stake.total_weight = 0;
    let result = math::calculate(&round, &uniform(&round));
    assert_eq!(result.outcome, MatrixOutcome::Accepted);
    assert_eq!(result.revealed_nodes, 4);
    assert_eq!(result.revealed_weight, 0);
    assert!(result.nodes.values().all(|node| node.weight == 0));
}

#[test]
fn zero_voting_weight_can_earn_without_influencing_other_nodes() {
    let mut round = pure_round(4);
    round.stake.weights.insert(4, 0);
    round.stake.total_weight = 3;
    let rows = uniform(&round);
    let result = math::calculate(&round, &rows);
    assert_eq!(result.outcome, MatrixOutcome::Accepted);
    assert!(result.nodes[&4].weight > 0);
    assert_eq!(result.revealed_nodes, 4);
}

#[test]
fn no_external_voting_weight_means_no_node_reward_at_any_blend() {
    let mut round = pure_round(3);
    round.stake.weights = BTreeMap::from([(1, 100), (2, 0), (3, 0)]);
    round.stake.total_weight = 100;
    let rows = uniform(&round);
    for accuracy_weight in [0, Q / 2, Q] {
        round.parameters.accuracy_weight = accuracy_weight;
        let result = math::calculate(&round, &rows);
        assert_eq!(result.outcome, MatrixOutcome::Accepted);
        assert_eq!(result.nodes[&1].performance, 0);
        assert!(!result.nodes[&1].has_accuracy_reference);
        assert_eq!(result.nodes[&1].weight, 0);
        // Nodes 2 and 3 are assessed by node 1 and can earn despite their own zero stake.
        assert!(result.nodes[&2].performance > 0 && result.nodes[&3].performance > 0);
        assert_eq!(result.nodes[&2].weight + result.nodes[&3].weight, Q);
    }
}

#[test]
fn matrix_snapshot_reuses_attestation_weight_calculation() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        let round = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(
            round.stake,
            Network::snapshot_consensus_attestor_weights(subnet, &round.policy, &round.identities)
        );
        let validator = round.identities[&1];
        ValidatorDelegateStakeBalance::<Test>::insert(validator, u128::MAX);
        ValidatorNodeDelegateStakeWeights::<Test>::mutate(validator, |weights| {
            weights.insert((subnet, 1), Q / 7);
        });
        assert_eq!(MatrixRounds::<Test>::get(subnet, epoch).unwrap(), round);
        let mut policy = round.policy;
        for decay in [0, Q / 2, Q] {
            for power in [Q / 2, Q] {
                policy.consensus_validator_node_count_decay = decay;
                policy.consensus_validator_stake_weight_power = power;
                ConsensusValidatorNodeCountDecay::<Test>::insert(subnet, decay);
                ConsensusValidatorStakeWeightPower::<Test>::insert(subnet, power);
                Network::open_matrix_round(subnet, epoch + 1);
                let next = MatrixRounds::<Test>::take(subnet, epoch + 1).unwrap();
                assert_eq!(
                    next.stake,
                    Network::snapshot_consensus_attestor_weights(
                        subnet,
                        &next.policy,
                        &next.identities
                    )
                );
            }
        }
    });
}

#[test]
fn a_single_validator_can_register_activate_and_complete_a_three_node_matrix_round() {
    new_test_ext().execute_with(|| {
        increase_epochs(50);
        RequireSubnetRegistrationWhitelist::<Test>::put(false);
        let coldkey = account(777);
        drop(Balances::deposit_creating(&coldkey, 1_000_000 * Q));
        assert_ok!(Network::register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            account(778),
            0,
            None,
            None,
        ));
        let validator = ColdkeyValidatorId::<Test>::get(&coldkey).unwrap();
        let mut data = default_registration_subnet_data(
            0,
            MaxSubnetNodes::<Test>::get(),
            b"single-validator-matrix".to_vec(),
            0,
            3,
        );
        data.consensus_mechanism = ConsensusMechanism::Matrix;
        data.initial_validators = BTreeMap::from([(validator, 3)]);
        assert_ok!(Network::register_subnet(
            RuntimeOrigin::signed(coldkey.clone()),
            u128::MAX,
            data
        ));
        let subnet = TotalSubnetUids::<Test>::get();
        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet,
            100 * Q,
            1,
        ));
        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            validator,
            300 * Q,
            1,
        ));
        let registration = SubnetRegistrationEpoch::<Test>::get(subnet).unwrap();
        System::set_block_number(
            (registration + MinSubnetRegistrationEpochs::<Test>::get()) * EpochLength::get(),
        );
        for id in 1..=3 {
            assert_ok!(Network::register_subnet_node(
                RuntimeOrigin::signed(coldkey.clone()),
                validator,
                subnet,
                None,
                Some(PeerInfo::<Test> {
                    peer_id: peer(id),
                    multiaddr: None
                }),
                None,
                None,
                100 * Q,
                None,
                None,
                u128::MAX,
            ));
            if id == 2 {
                // A lowered network minimum must not allow a two-node Matrix subnet.
                MinSubnetNodes::<Test>::put(1);
                assert_noop!(
                    Network::activate_subnet(RuntimeOrigin::signed(coldkey.clone()), subnet),
                    Error::<Test>::SubnetActivationConditionsNotMetYet
                );
                MinSubnetNodes::<Test>::put(3);
            }
        }
        assert_ok!(Network::set_validator_node_delegate_stake_weights(
            RuntimeOrigin::signed(coldkey.clone()),
            vec![
                (subnet, 1, Q / 3 + Q % 3),
                (subnet, 2, Q / 3),
                (subnet, 3, Q / 3)
            ],
        ));
        assert_ok!(Network::activate_subnet(
            RuntimeOrigin::signed(coldkey),
            subnet
        ));
        let epoch = SubnetsData::<Test>::get(subnet)
            .unwrap()
            .consensus_eligible_from_subnet_epoch
            .unwrap();
        set_block_to_subnet_slot_epoch(epoch, subnet);
        Network::on_initialize(System::block_number());
        let round = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(round.nodes.len(), 3);
        assert!(round.identities.values().all(|id| *id == validator));
        assert!(round.stake.weights.values().all(|weight| *weight > 0));
        assert_eq!(
            round.stake,
            Network::snapshot_consensus_attestor_weights(subnet, &round.policy, &round.identities)
        );
        FinalSubnetEmissionWeights::<Test>::insert(
            epoch + 1,
            DistributionData {
                subnets_emissions: 10 * Q,
                subnet_weights: BTreeMap::from([(subnet, Q)]),
            },
        );
        SubnetNodeReputation::<Test>::insert(subnet, 1, 80 * Q / 100);
        let before = NodeSubnetStake::<Test>::get(1, subnet);
        submit(subnet, epoch, &[1, 2, 3]);
        Network::settle_matrix_round(subnet, epoch + 1);
        let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(result.outcome, MatrixOutcome::Accepted);
        assert!(result
            .nodes
            .values()
            .all(|n| n.has_accuracy_reference && n.accuracy == Q && n.weight > 0));
        assert_eq!(
            SubnetNodeReputation::<Test>::get(subnet, 1),
            Some(88 * Q / 100)
        );
        assert!(NodeSubnetStake::<Test>::get(1, subnet) > before);
    });
}

#[test]
fn fewer_than_three_targets_cannot_open_or_settle_a_matrix_round() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        for id in [3, 4] {
            SubnetNodesData::<Test>::remove(subnet, id);
        }
        Network::open_matrix_round(subnet, epoch + 1);
        assert!(!MatrixRounds::<Test>::contains_key(subnet, epoch + 1));
        MatrixRounds::<Test>::mutate(subnet, epoch, |round| {
            let round = round.as_mut().unwrap();
            round.nodes.truncate(2);
            round.identities.retain(|id, _| *id <= 2);
            round.stake.weights.retain(|id, _| *id <= 2);
            round.stake.total_weight = round.stake.weights.values().sum();
        });
        Network::settle_matrix_round(subnet, epoch + 1);
        assert_eq!(
            MatrixResults::<Test>::get(subnet, epoch).unwrap().outcome,
            MatrixOutcome::Cancelled
        );
        assert_eq!(SubnetNodeReputation::<Test>::get(subnet, 1), Some(Q));
    });
}

#[test]
fn commits_are_bound_and_reveal_failures_are_retryable() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        let round = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
        let row = raw_row(&round, 1, Q);
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, 1).unwrap();
        let hash = Network::matrix_commitment(subnet, epoch, 1, &round, &row, &[9; 32]);
        assert_ne!(
            hash,
            Network::matrix_commitment(subnet, epoch + 1, 1, &round, &row, &[9; 32])
        );
        assert_ok!(Network::commit_matrix_scores(
            RuntimeOrigin::signed(hotkey.clone()),
            subnet,
            epoch,
            1,
            hash
        ));
        assert_noop!(
            Network::commit_matrix_scores(
                RuntimeOrigin::signed(hotkey.clone()),
                subnet,
                epoch,
                1,
                hash
            ),
            Error::<Test>::AlreadyCommitted
        );
        assert_noop!(
            Network::reveal_matrix_scores(
                RuntimeOrigin::signed(hotkey.clone()),
                subnet,
                epoch,
                1,
                row.clone(),
                [9; 32]
            ),
            Error::<Test>::NotRevealPeriod
        );
        System::set_block_number(round.reveal_block);
        assert_noop!(
            Network::reveal_matrix_scores(
                RuntimeOrigin::signed(hotkey.clone()),
                subnet,
                epoch,
                1,
                row.clone(),
                [8; 32]
            ),
            Error::<Test>::RevealMismatch
        );
        assert_ok!(Network::reveal_matrix_scores(
            RuntimeOrigin::signed(hotkey.clone()),
            subnet,
            epoch,
            1,
            row.clone(),
            [9; 32]
        ));
        assert_eq!(
            MatrixRows::<Test>::get((subnet, epoch, 1)),
            Some(row.clone())
        );
        assert_noop!(
            Network::reveal_matrix_scores(
                RuntimeOrigin::signed(hotkey),
                subnet,
                epoch,
                1,
                row,
                [9; 32]
            ),
            Error::<Test>::AlreadyRevealed
        );
    });
}

#[test]
fn invalid_roster_and_wrong_hotkeys_never_write() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        let round = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
        assert_noop!(
            Network::commit_matrix_scores(
                RuntimeOrigin::signed(account(99999)),
                subnet,
                epoch,
                1,
                Default::default()
            ),
            Error::<Test>::InvalidValidator
        );
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, 1).unwrap();
        let row = raw_row(&round, 1, 1);
        let hash = Network::matrix_commitment(subnet, epoch, 1, &round, &row, &[1; 32]);
        assert_ok!(Network::commit_matrix_scores(
            RuntimeOrigin::signed(hotkey.clone()),
            subnet,
            epoch,
            1,
            hash
        ));
        System::set_block_number(round.reveal_block);
        let mut bad = row.clone();
        bad[0].subnet_node_id = 1;
        assert_noop!(
            Network::reveal_matrix_scores(
                RuntimeOrigin::signed(hotkey.clone()),
                subnet,
                epoch,
                1,
                bad,
                [1; 32]
            ),
            Error::<Test>::InvalidMatrixScores
        );
        let mut bad = row.clone();
        bad[0].subnet_node_id = bad[1].subnet_node_id;
        assert_noop!(
            Network::reveal_matrix_scores(
                RuntimeOrigin::signed(hotkey.clone()),
                subnet,
                epoch,
                1,
                bad,
                [1; 32]
            ),
            Error::<Test>::InvalidMatrixScores
        );
        System::set_block_number(round.end_block);
        assert_noop!(
            Network::reveal_matrix_scores(
                RuntimeOrigin::signed(hotkey),
                subnet,
                epoch,
                1,
                row,
                [1; 32]
            ),
            Error::<Test>::NotRevealPeriod
        );
    });
}

#[test]
fn collective_configuration_is_global_immediate_and_open_rounds_stay_frozen() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        let owner = SubnetOwner::<Test>::get(subnet).unwrap();
        let initial = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
        assert_noop!(
            Network::owner_set_consensus_mechanism(
                RuntimeOrigin::signed(owner.clone()),
                subnet,
                ConsensusMechanism::Attestation
            ),
            Error::<Test>::SubnetActivatedAlready
        );
        let updated = MatrixParameters {
            accuracy_weight: Q,
            ..initial.parameters
        };
        for origin in [
            RuntimeOrigin::signed(owner),
            RuntimeOrigin::signed(account(5555)),
            RuntimeOrigin::none(),
            RuntimeOrigin::root(),
            RuntimeOrigin::from(pallet_collective::RawOrigin::Members(3, 5)),
        ] {
            assert_noop!(
                Network::set_matrix_config(origin, updated),
                sp_runtime::DispatchError::BadOrigin
            );
        }
        assert_ok!(Network::set_matrix_config(governance_origin(), updated));
        assert_eq!(MatrixConfig::<Test>::get(), updated);
        assert_eq!(MatrixRounds::<Test>::get(subnet, epoch).unwrap(), initial);
        // A different subnet reads the same global settings, with no per-subnet override.
        let other = subnet + 1;
        let mut data = SubnetsData::<Test>::get(subnet).unwrap();
        data.id = other;
        SubnetsData::<Test>::insert(other, data);
        SubnetSlot::<Test>::insert(other, SubnetSlot::<Test>::get(subnet).unwrap());
        for node in &initial.nodes {
            SubnetNodesData::<Test>::insert(other, node.id, node);
        }
        SubnetNodeElectionSlots::<Test>::insert(other, (1..=4).collect::<Vec<_>>());
        Network::open_matrix_round(other, epoch);
        assert_eq!(
            MatrixRounds::<Test>::get(other, epoch).unwrap().parameters,
            updated
        );
        Network::open_matrix_round(subnet, epoch + 1);
        assert_eq!(
            MatrixRounds::<Test>::get(subnet, epoch + 1)
                .unwrap()
                .parameters,
            updated
        );
        assert_eq!(ConsensusMechanism::Matrix.encode(), vec![1]);
    });
}

#[test]
fn successful_round_uses_existing_reward_splits_and_is_exact_once() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        submit(subnet, epoch, &[1, 2, 3, 4]);
        let before: BTreeMap<_, _> = (1..=4)
            .map(|id| (id, NodeSubnetStake::<Test>::get(id, subnet)))
            .collect();
        let policy = MatrixRounds::<Test>::get(subnet, epoch).unwrap().policy;
        let (rewards, _) = Network::calculate_rewards_with_policy(10 * Q, Q, &policy);
        Network::settle_matrix_round(subnet, epoch + 1);
        let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(result.outcome, MatrixOutcome::Accepted);
        for id in 1..=4 {
            let gross = Network::percent_mul(result.nodes[&id].weight, rewards.subnet_node_rewards);
            let validator = SubnetNodeValidatorId::<Test>::get(subnet, id).unwrap();
            let operation = Network::prepare_consensus_node_reward(subnet, id, validator, gross);
            let ConsensusOperation::NodeReward {
                amount,
                validator_pool,
                delegate_account,
                ..
            } = operation
            else {
                panic!()
            };
            let (net, _, _) = Network::consensus_node_reward_allocations(
                amount,
                &validator_pool,
                &delegate_account,
            )
            .unwrap();
            assert_eq!(NodeSubnetStake::<Test>::get(id, subnet), before[&id] + net);
        }
        let issuance = Balances::total_issuance();
        Network::settle_matrix_round(subnet, epoch + 1);
        assert_eq!(Balances::total_issuance(), issuance);
        set_block_to_subnet_slot_epoch(epoch + 1, subnet);
        Network::on_initialize(System::block_number());
        assert!(MatrixRounds::<Test>::contains_key(subnet, epoch + 1));
        assert!(!SubnetElectedValidator::<Test>::contains_key(
            subnet,
            epoch + 1
        ));
    });
}

#[test]
fn empty_and_abstaining_rounds_retain_allocation_and_leave_reputation_unchanged() {
    for abstain in [false, true] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = fixture();
            MatrixRounds::<Test>::mutate(subnet, epoch, |round| {
                let policy = &mut round.as_mut().unwrap().policy;
                policy.min_weight_decrease_reputation_threshold = Q;
                policy.reputation_factors.non_attestor_decrease = Q / 10;
            });
            if abstain {
                let rows = (1..=4).map(|id| (id, MatrixScores::default())).collect();
                submit_rows(subnet, epoch, &rows);
            }
            let before = NodeSubnetStake::<Test>::get(1, subnet);
            SubnetNodeReputation::<Test>::insert(subnet, 1, 80 * Q / 100);
            Network::settle_matrix_round(subnet, epoch + 1);
            assert_eq!(
                MatrixResults::<Test>::get(subnet, epoch).unwrap().outcome,
                MatrixOutcome::NoScores
            );
            assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 10 * Q);
            assert_eq!(NodeSubnetStake::<Test>::get(1, subnet), before);
            // No weighted assessment exists, so absence/minimum-weight penalties do not apply.
            assert_eq!(
                SubnetNodeReputation::<Test>::get(subnet, 1),
                Some(80 * Q / 100)
            );
            Network::settle_matrix_round(subnet, epoch + 1);
            assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 10 * Q);
            assert_eq!(
                SubnetNodeReputation::<Test>::get(subnet, 1),
                Some(80 * Q / 100)
            );
        });
    }
}

#[test]
fn one_revealer_is_accepted_without_fabricated_accuracy_or_self_assessment() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        MatrixRounds::<Test>::mutate(subnet, epoch, |round| {
            let policy = &mut round.as_mut().unwrap().policy;
            policy.min_weight_decrease_reputation_threshold = Q / 100;
            policy.reputation_factors.non_attestor_decrease = Q / 10;
        });
        SubnetNodeReputation::<Test>::insert(subnet, 1, 80 * Q / 100);
        let before = NodeSubnetStake::<Test>::get(1, subnet);
        submit(subnet, epoch, &[1]);
        Network::settle_matrix_round(subnet, epoch + 1);
        let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(result.outcome, MatrixOutcome::Accepted);
        assert_eq!(result.revealed_nodes, 1);
        assert!(result.nodes.range(2..).all(|(_, n)| n.performance > 0));
        assert_eq!(result.nodes[&1].weight, 0);
        assert!(!result.nodes[&1].has_received_assessment);
        assert!(result
            .nodes
            .range(2..)
            .all(|(_, n)| n.weight > 0 && n.has_received_assessment));
        assert_eq!(result.nodes.values().map(|n| n.weight).sum::<u128>(), Q);
        assert_eq!(result.nodes[&1].performance, 0);
        assert!(!result.nodes[&1].has_accuracy_reference);
        assert_eq!(
            SubnetNodeReputation::<Test>::get(subnet, 1),
            Some(80 * Q / 100)
        );
        assert_eq!(NodeSubnetStake::<Test>::get(1, subnet), before);
        assert_eq!(SubnetNodeReputation::<Test>::get(subnet, 2), Some(Q));
    });
}

#[test]
fn missing_settlement_marker_still_finalizes_an_empty_round_without_penalties() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        ConsensusRoundSettlementStatus::<Test>::remove(subnet, epoch);
        Network::settle_matrix_round(subnet, epoch + 1);
        let after = SubnetNodeReputation::<Test>::get(subnet, 1).unwrap();
        assert_eq!(after, Q);
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
            Some(true)
        );
        Network::settle_matrix_round(subnet, epoch + 1);
        assert_eq!(SubnetNodeReputation::<Test>::get(subnet, 1), Some(after));
    });
}

#[test]
fn two_revealers_and_assessed_workers_receive_rewards_without_quorum() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        MatrixRounds::<Test>::mutate(subnet, epoch, |round| {
            let round = round.as_mut().unwrap();
            // These Attestation participation settings do not constrain Matrix.
            round.policy.super_majority_attestation_ratio = Q;
            round.policy.reputation_factors.non_attestor_decrease = Q / 10;
            round.policy.min_weight_decrease_reputation_threshold = 0;
            round.stake.weights.insert(1, 1);
            round.stake.weights.insert(2, 1);
            round.stake.total_weight = 200 * Q + 2;
        });
        let before: Vec<_> = (1..=4)
            .map(|id| NodeSubnetStake::<Test>::get(id, subnet))
            .collect();
        submit(subnet, epoch, &[1, 2]);
        Network::settle_matrix_round(subnet, epoch + 1);
        let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(result.outcome, MatrixOutcome::Accepted);
        assert_eq!(result.revealed_weight, 2);
        assert_eq!(result.revealed_nodes, 2);
        assert_eq!(result.nodes.values().map(|n| n.weight).sum::<u128>(), Q);
        for id in 1..=4 {
            assert!(NodeSubnetStake::<Test>::get(id, subnet) > before[id as usize - 1]);
            if id > 2 {
                assert!(result.nodes[&id].weight > 0);
                assert_eq!(result.nodes[&id].accuracy, 0);
                assert_eq!(SubnetNodeReputation::<Test>::get(subnet, id), Some(Q));
            }
        }
    });
}

#[test]
fn zero_stake_reveals_are_participation_without_fabricated_assessments() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        MatrixRounds::<Test>::mutate(subnet, epoch, |round| {
            let round = round.as_mut().unwrap();
            round
                .stake
                .weights
                .values_mut()
                .for_each(|weight| *weight = 0);
            round.stake.total_weight = 0;
            round.policy.min_weight_decrease_reputation_threshold = Q;
        });
        SubnetNodeReputation::<Test>::insert(subnet, 1, 80 * Q / 100);
        submit(subnet, epoch, &[1, 2, 3, 4]);
        let before = NodeSubnetStake::<Test>::get(1, subnet);
        Network::settle_matrix_round(subnet, epoch + 1);
        let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(result.outcome, MatrixOutcome::Accepted);
        assert_eq!(result.revealed_nodes, 4);
        assert_eq!(result.revealed_weight, 0);
        assert!(result
            .nodes
            .values()
            .all(|n| n.performance == 0 && !n.has_accuracy_reference && n.weight == 0));
        assert_eq!(
            SubnetNodeReputation::<Test>::get(subnet, 1),
            Some(80 * Q / 100)
        );
        assert_eq!(NodeSubnetStake::<Test>::get(1, subnet), before);
    });
}

#[test]
fn resumed_round_is_cancelled_and_commits_cannot_be_replayed() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        submit(subnet, epoch, &[1, 2, 3, 4]);
        let start = MatrixRounds::<Test>::get(subnet, epoch)
            .unwrap()
            .start_block;
        LastNetworkUnpauseBlock::<Test>::put(start);
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, 1).unwrap();
        assert_noop!(
            Network::commit_matrix_scores(
                RuntimeOrigin::signed(hotkey),
                subnet,
                epoch,
                1,
                Default::default()
            ),
            Error::<Test>::ConsensusRoundCancelled
        );
        Network::settle_matrix_round(subnet, epoch + 1);
        assert_eq!(
            MatrixResults::<Test>::get(subnet, epoch).unwrap().outcome,
            MatrixOutcome::Cancelled
        );
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 10 * Q);
    });
}

#[test]
fn matrix_history_is_pruned_incrementally_without_touching_current_round() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        submit(subnet, epoch, &[1, 2, 3, 4]);
        Network::settle_matrix_round(subnet, epoch + 1);
        Network::open_matrix_round(subnet, epoch + 129);
        ConsensusHistoryPruneCursor::<Test>::insert(subnet, epoch);
        let mut too_small = WeightMeter::with_limit(Weight::zero());
        Network::prune_consensus_history(&mut too_small, subnet, epoch + 129);
        assert!(MatrixRounds::<Test>::contains_key(subnet, epoch));
        Network::prune_consensus_history(&mut WeightMeter::new(), subnet, epoch + 129);
        assert!(!MatrixRounds::<Test>::contains_key(subnet, epoch));
        assert!(!MatrixResults::<Test>::contains_key(subnet, epoch));
        assert_eq!(MatrixRows::<Test>::iter_prefix((subnet, epoch)).count(), 0);
        assert_eq!(
            MatrixCommitments::<Test>::iter_prefix((subnet, epoch)).count(),
            0
        );
        assert!(MatrixRounds::<Test>::contains_key(subnet, epoch + 129));
    });
}

#[test]
fn sixty_four_node_matrix_is_bounded_and_slot_reservation_fits() {
    new_test_ext().execute_with(|| {
        let round = pure_round(64);
        let result = math::calculate(&round, &uniform(&round));
        assert_eq!(result.nodes.len(), 64);
        assert_eq!(result.nodes.values().map(|n| n.weight).sum::<u128>(), Q);
        assert!(Network::matrix_slot_weight().all_lte(MaximumHooksWeight::get()));
        let (subnet, _) = fixture();
        assert_eq!(Network::subnet_node_capacity(subnet), 64);
        TotalAdmittedSubnetNodes::<Test>::insert(subnet, 64);
        assert_noop!(
            Network::perform_admit_subnet_node(
                subnet,
                SubnetState::Active,
                SubnetNode {
                    id: 65,
                    ..Default::default()
                },
                1
            ),
            Error::<Test>::MaxSubnetNodes
        );
    });
}

#[test]
fn abstention_is_recorded_once_and_has_no_voting_influence_but_allows_performance_rewards() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        let round = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
        let row = MatrixScores::default();
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, 4).unwrap();
        let hash = Network::matrix_commitment(subnet, epoch, 4, &round, &row, &[4; 32]);
        assert_ok!(Network::commit_matrix_scores(
            RuntimeOrigin::signed(hotkey.clone()),
            subnet,
            epoch,
            4,
            hash
        ));
        submit(subnet, epoch, &[1, 2, 3]);
        assert_ok!(Network::reveal_matrix_scores(
            RuntimeOrigin::signed(hotkey),
            subnet,
            epoch,
            4,
            row,
            [4; 32]
        ));
        let before = NodeSubnetStake::<Test>::get(4, subnet);
        Network::settle_matrix_round(subnet, epoch + 1);
        let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(result.outcome, MatrixOutcome::Accepted);
        assert_eq!(result.revealed_weight, 300 * Q);
        assert_eq!(result.revealed_nodes, 3);
        assert!(result.nodes[&4].weight > 0);
        assert_eq!(result.nodes[&4].accuracy, 0);
        assert!(NodeSubnetStake::<Test>::get(4, subnet) > before);
    });
}

#[test]
fn rotated_hotkey_can_finish_existing_commitment_and_old_key_is_revoked() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        let round = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
        let row = raw_row(&round, 1, 100);
        let old = Network::get_subnet_node_associated_hotkey(subnet, 1).unwrap();
        let hash = Network::matrix_commitment(subnet, epoch, 1, &round, &row, &[1; 32]);
        assert_ok!(Network::commit_matrix_scores(
            RuntimeOrigin::signed(old.clone()),
            subnet,
            epoch,
            1,
            hash
        ));
        let new = account(99881);
        // Same authority resolver as the real key-rotation extrinsic.
        SubnetNodeIdHotkey::<Test>::insert(subnet, 1, &new);
        System::set_block_number(round.reveal_block);
        assert_noop!(
            Network::reveal_matrix_scores(
                RuntimeOrigin::signed(old),
                subnet,
                epoch,
                1,
                row.clone(),
                [1; 32]
            ),
            Error::<Test>::InvalidValidator
        );
        assert_ok!(Network::reveal_matrix_scores(
            RuntimeOrigin::signed(new),
            subnet,
            epoch,
            1,
            row,
            [1; 32]
        ));
    });
}

#[test]
fn quarantine_after_reveal_withholds_payment_without_rewriting_scores() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        submit(subnet, epoch, &[1, 2, 3, 4]);
        let before = NodeSubnetStake::<Test>::get(1, subnet);
        PendingAdmittedNodeRemovals::<Test>::mutate(subnet, |nodes| {
            nodes.try_insert(1).unwrap();
        });
        Network::settle_matrix_round(subnet, epoch + 1);
        let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(result.outcome, MatrixOutcome::Accepted);
        assert_eq!(result.revealed_weight, 400 * Q);
        assert!(result.nodes[&1].weight > 0);
        assert_eq!(result.nodes.values().map(|n| n.weight).sum::<u128>(), Q);
        assert_eq!(NodeSubnetStake::<Test>::get(1, subnet), before);
    });
}

#[test]
fn failed_node_credits_are_queued_and_retry_exactly_once() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        submit(subnet, epoch, &[1, 2, 3, 4]);
        let before = NodeSubnetStake::<Test>::get(1, subnet);
        let stake_total = TotalStake::<Test>::get();
        TotalStake::<Test>::put(u128::MAX);
        Network::settle_matrix_round(subnet, epoch + 1);
        assert_eq!(
            MatrixResults::<Test>::get(subnet, epoch).unwrap().outcome,
            MatrixOutcome::Accepted
        );
        assert_eq!(NodeSubnetStake::<Test>::get(1, subnet), before);
        assert_eq!(
            PendingConsensusOperations::<Test>::iter()
                .filter(|(_, op)| matches!(op.operation, ConsensusOperation::NodeReward { .. }))
                .count(),
            4
        );
        TotalStake::<Test>::put(stake_total);
        Network::retry_pending_consensus_operations(
            System::block_number(),
            &mut WeightMeter::new(),
        );
        assert!(NodeSubnetStake::<Test>::get(1, subnet) > before);
        assert_eq!(PendingConsensusOperations::<Test>::iter().count(), 0);
        let after = NodeSubnetStake::<Test>::get(1, subnet);
        Network::settle_matrix_round(subnet, epoch + 1);
        Network::retry_pending_consensus_operations(
            System::block_number(),
            &mut WeightMeter::new(),
        );
        assert_eq!(NodeSubnetStake::<Test>::get(1, subnet), after);
    });
}

#[test]
fn capacitor_release_uses_shared_split_and_current_release_percentage() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        submit(subnet, epoch, &[1, 2, 3, 4]);
        SubnetRewardCapacitor::<Test>::insert(subnet, 100 * Q);
        SubnetRewardCapacitorReleasePercentage::<Test>::put(Q / 10);
        Network::settle_matrix_round(subnet, epoch + 1);
        // Release 10% of the scheduled 10 TENSOR, not 10% of the stored 100 TENSOR.
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 99 * Q);
    });
}

#[test]
fn probation_is_scored_promoted_and_never_paid_for_its_probation_round() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        MatrixRounds::<Test>::remove(subnet, epoch);
        assert_ok!(Network::remove_node_from_election_slot(subnet, 4));
        SubnetNodesData::<Test>::mutate(subnet, 4, |node| {
            node.classification.node_class = SubnetNodeClass::Probation
        });
        Network::open_matrix_round(subnet, epoch);
        let round = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
        assert!(round.nodes.iter().any(|n| n.id == 4));
        assert!(!round.identities.contains_key(&4));
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, 4).unwrap();
        assert_noop!(
            Network::commit_matrix_scores(
                RuntimeOrigin::signed(hotkey),
                subnet,
                epoch,
                4,
                Default::default()
            ),
            Error::<Test>::InvalidValidatorId
        );
        SubnetNodeProbationProgress::<Test>::insert(
            subnet,
            4,
            round.policy.probation_rounds.saturating_sub(1),
        );
        let before = NodeSubnetStake::<Test>::get(4, subnet);
        submit(subnet, epoch, &[1]);
        Network::settle_matrix_round(subnet, epoch + 1);
        assert_eq!(NodeSubnetStake::<Test>::get(4, subnet), before);
        assert_eq!(
            MatrixResults::<Test>::get(subnet, epoch).unwrap().nodes[&4].weight,
            0
        );
        assert_eq!(
            SubnetNodesData::<Test>::get(subnet, 4)
                .classification
                .node_class,
            SubnetNodeClass::Active
        );
    });
}

#[test]
fn matrix_workers_ignore_attestation_nonparticipation_reputation_factors() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        // Matrix ignores the Attestation supermajority policy even when set to unanimity.
        MatrixRounds::<Test>::mutate(subnet, epoch, |round| {
            let policy = &mut round.as_mut().unwrap().policy;
            policy.super_majority_attestation_ratio = Q;
            policy.reputation_factors.non_attestor_decrease = Q / 10;
        });
        submit(subnet, epoch, &[1, 2, 3]);
        let previous = SubnetNodeReputation::<Test>::get(subnet, 4).unwrap();
        Network::settle_matrix_round(subnet, epoch + 1);
        assert_eq!(SubnetNodeReputation::<Test>::get(subnet, 4), Some(previous));
    });
}

#[test]
fn matrix_and_attestation_rounds_both_qualify_for_emission_allocation() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        let name = b"attestation-alongside-matrix".to_vec();
        build_activated_subnet(name.clone(), 0, 4, 10_000 * Q, 100 * Q);
        let attestation = SubnetName::<Test>::get(name).unwrap();
        seed_equal_validator_delegate_stake_for_subnet(attestation);
        let later = Network::get_current_subnet_epoch_as_u32(attestation).max(epoch) + 2;
        Network::open_matrix_round(subnet, later);
        Network::elect_validator(attestation, later);
        let (weights, _) = Network::calculate_subnet_weights(later + 1);
        assert!(weights.get(&subnet).is_some_and(|w| *w > 0));
        assert!(weights.get(&attestation).is_some_and(|w| *w > 0));
        assert!(
            !Network::rpc_get_subnet_epoch_status(subnet)
                .unwrap()
                .within_proposal_attestation_window
        );
    });
}

#[test]
fn removal_cleans_history_without_removing_global_matrix_configuration() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        submit(subnet, epoch, &[1, 2, 3, 4]);
        let parameters = MatrixParameters {
            accuracy_weight: 0,
            ..MatrixConfig::<Test>::get()
        };
        assert_ok!(Network::set_matrix_config(governance_origin(), parameters));
        let owner = SubnetOwner::<Test>::get(subnet).unwrap();
        assert_ok!(Network::owner_deactivate_subnet(
            RuntimeOrigin::signed(owner),
            subnet
        ));
        assert_eq!(MatrixConfig::<Test>::get(), parameters);
        assert_eq!(
            ConsensusHistoryCleanupQueue::<Test>::get(subnet),
            Some(ConsensusHistoryCleanupStage::Matrix)
        );
        Network::cleanup_removed_subnet_consensus_history(&mut WeightMeter::new());
        assert!(!MatrixRounds::<Test>::contains_key(subnet, epoch));
    });
}

#[test]
fn inconsistent_evidence_cancels_instead_of_overflowing_or_paying() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        submit(subnet, epoch, &[1, 2, 3, 4]);
        MatrixRounds::<Test>::mutate(subnet, epoch, |round| {
            round.as_mut().unwrap().stake.total_weight = 0
        });
        Network::settle_matrix_round(subnet, epoch + 1);
        assert_eq!(
            MatrixResults::<Test>::get(subnet, epoch).unwrap().outcome,
            MatrixOutcome::Cancelled
        );
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 10 * Q);
    });
}

#[test]
fn quadratic_reputation_rates_match_independent_examples() {
    let parameters = DefaultMatrixConfig::<Test>::get();
    // Distance, increase, decrease. Both sides have a quadratic, not a linear, slope.
    for (distance, increase, decrease) in [
        (0, Q / 10, 0),
        (Q / 40, 9 * Q / 160, 0),
        (Q / 20, Q / 40, 0),
        (3 * Q / 40, Q / 160, 0),
        (Q / 10, 0, 0),
        (13 * Q / 40, 0, 3 * Q / 160),
        (11 * Q / 20, 0, 3 * Q / 40),
        (31 * Q / 40, 0, 27 * Q / 160),
        (Q, 0, 3 * Q / 10),
    ] {
        assert_eq!(
            math::reputation_rates(&parameters, Q - distance),
            (increase, decrease)
        );
    }
}

#[test]
fn reputation_rates_are_bounded_monotone_and_allow_disabling_each_direction() {
    for tolerance in [0, Q / 10, Q - 1] {
        let parameters = MatrixParameters {
            reputation_tolerance: tolerance,
            max_reputation_increase: Q,
            max_reputation_decrease: Q,
            ..DefaultMatrixConfig::<Test>::get()
        };
        assert!(parameters.is_valid());
        let mut last = (Q, 0);
        for distance in (0..=1000).map(|i| Q * i / 1000) {
            let rates = math::reputation_rates(&parameters, Q - distance);
            assert!(rates.0 <= last.0 && rates.1 >= last.1);
            assert!(rates.0 <= Q && rates.1 <= Q);
            assert!(rates.0 == 0 || rates.1 == 0);
            last = rates;
        }
        assert_eq!(math::reputation_rates(&parameters, Q), (Q, 0));
        assert_eq!(math::reputation_rates(&parameters, 0), (0, Q));
        if tolerance > 0 {
            assert_eq!(math::reputation_rates(&parameters, Q - tolerance), (0, 0));
        }
        for accuracy in [0, Q / 2, Q] {
            let disabled = MatrixParameters {
                max_reputation_increase: 0,
                max_reputation_decrease: 0,
                ..parameters
            };
            assert_eq!(math::reputation_rates(&disabled, accuracy), (0, 0));
        }
        assert_eq!(
            math::reputation_rates(
                &MatrixParameters {
                    max_reputation_increase: 0,
                    ..parameters
                },
                0
            ),
            (0, Q)
        );
        assert_eq!(
            math::reputation_rates(
                &MatrixParameters {
                    max_reputation_decrease: 0,
                    ..parameters
                },
                Q
            ),
            (Q, 0)
        );
    }
}

// Node 1 assesses only target 2, whose independent reference is Q. Its absolute
// error is exactly `distance`. Incoming assessments separately enable rewards.
fn reputation_rows(distance: u128) -> BTreeMap<u32, MatrixScores> {
    BTreeMap::from([
        (1, scores(&[(2, Q - distance)])),
        (2, scores(&[(1, Q)])),
        (3, scores(&[(1, 0), (2, Q)])),
        (4, scores(&[(1, 0), (2, Q)])),
    ])
}

fn governance_origin() -> RuntimeOrigin {
    RuntimeOrigin::from(pallet_collective::RawOrigin::Members(4, 5))
}

fn update_reputation_config(parameters: MatrixParameters) -> DispatchResult {
    Network::set_matrix_config(governance_origin(), parameters)
}

#[test]
fn accuracy_reputation_settlement_rewards_agreement_and_penalizes_disagreement_once() {
    for (distance, expected) in [
        (0, 88 * Q / 100),
        (Q / 20, 82 * Q / 100),
        (Q / 10, 80 * Q / 100),
        (11 * Q / 20, 74 * Q / 100),
        (Q, 56 * Q / 100),
    ] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = fixture();
            SubnetNodeReputation::<Test>::insert(subnet, 1, 80 * Q / 100);
            submit_rows(subnet, epoch, &reputation_rows(distance));
            Network::settle_matrix_round(subnet, epoch + 1);
            let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
            assert_eq!(result.outcome, MatrixOutcome::Accepted);
            assert!(result.nodes[&1].has_accuracy_reference);
            assert_eq!(result.nodes[&1].accuracy, Q - distance);
            assert!(result.nodes[&1].performance > 0);
            assert_eq!(SubnetNodeReputation::<Test>::get(subnet, 1), Some(expected));
            Network::settle_matrix_round(subnet, epoch + 1);
            assert_eq!(SubnetNodeReputation::<Test>::get(subnet, 1), Some(expected));
        });
    }
}

#[test]
fn low_turnout_applies_both_accuracy_gains_and_losses() {
    for (distance, expected) in [
        (0, 88 * Q / 100),
        (11 * Q / 20, 74 * Q / 100),
        (Q, 56 * Q / 100),
    ] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = fixture();
            MatrixRounds::<Test>::mutate(subnet, epoch, |round| {
                let policy = &mut round.as_mut().unwrap().policy;
                policy.super_majority_attestation_ratio = Q;
                // Isolate accuracy from the separate received-performance and weight penalties.
                policy.reputation_factors.absent_decrease = 0;
                policy.min_weight_decrease_reputation_threshold = 0;
            });
            SubnetNodeReputation::<Test>::insert(subnet, 1, 80 * Q / 100);
            let mut rows = reputation_rows(distance);
            rows.retain(|id, _| *id == 1 || *id == 3);
            submit_rows(subnet, epoch, &rows);
            Network::settle_matrix_round(subnet, epoch + 1);
            let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
            assert_eq!(result.outcome, MatrixOutcome::Accepted);
            assert_eq!(result.nodes[&1].accuracy, Q - distance);
            assert_eq!(SubnetNodeReputation::<Test>::get(subnet, 1), Some(expected));
        });
    }
}

#[test]
fn accuracy_reputation_clamps_and_preserves_other_penalties_and_removal() {
    for (distance, previous, min_reputation, minimum_weight, expected) in [
        (0, 99 * Q / 100, 0, 0, Q), // Gain clamps at full reputation.
        (0, 0, 0, 0, 0),            // A proportional gain cannot resurrect zero reputation.
        (Q, 80 * Q / 100, 60 * Q / 100, 0, 56 * Q / 100), // Removal threshold.
        (Q, 80 * Q / 100, 0, Q, 504 * Q / 1000), // 30% loss, then 10% low-weight loss.
    ] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = fixture();
            MatrixRounds::<Test>::mutate(subnet, epoch, |round| {
                let policy = &mut round.as_mut().unwrap().policy;
                policy.min_subnet_node_reputation = min_reputation;
                policy.min_weight_decrease_reputation_threshold = minimum_weight;
                policy.reputation_factors.below_min_weight_decrease = Q / 10;
            });
            SubnetNodeReputation::<Test>::insert(subnet, 1, previous);
            submit_rows(subnet, epoch, &reputation_rows(distance));
            Network::settle_matrix_round(subnet, epoch + 1);
            assert_eq!(SubnetNodeReputation::<Test>::get(subnet, 1), Some(expected));
            assert_eq!(
                PendingAdmittedNodeRemovals::<Test>::get(subnet).contains(&1),
                expected < min_reputation
            );
        });
    }
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        let mut rows = reputation_rows(Q);
        // Remove all received performance for node 1 without changing its reference.
        rows.insert(2, normalized(&[(1, 0), (3, Q), (4, 0)]));
        SubnetNodeReputation::<Test>::insert(subnet, 1, Q);
        submit_rows(subnet, epoch, &rows);
        Network::settle_matrix_round(subnet, epoch + 1);
        let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(result.nodes[&1].performance, 0);
        // The independent reference for target 2 remains Q, so maximum accuracy
        // loss and the existing absence loss both apply, in that order.
        let (_, loss) = math::reputation_rates(
            &DefaultMatrixConfig::<Test>::get(),
            result.nodes[&1].accuracy,
        );
        let policy = MatrixRounds::<Test>::get(subnet, epoch).unwrap().policy;
        assert_eq!(
            SubnetNodeReputation::<Test>::get(subnet, 1),
            Some(Network::decrease_rep(
                Network::decrease_rep(Q, loss, None),
                policy.reputation_factors.absent_decrease,
                None
            ))
        );
    });
}

#[test]
fn unusable_reference_does_not_count_as_maximum_disagreement_or_earn_a_gain() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        let mut rows = reputation_rows(0);
        for node in 2..=4 {
            rows.insert(node, scores(&[(1, Q)]));
        }
        // Other nodes assess only node 1, leaving its target 2 without a reference.
        SubnetNodeReputation::<Test>::insert(subnet, 1, 80 * Q / 100);
        submit_rows(subnet, epoch, &rows);
        Network::settle_matrix_round(subnet, epoch + 1);
        let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(result.outcome, MatrixOutcome::Accepted);
        assert_eq!(result.nodes[&1].accuracy, 0);
        assert!(!result.nodes[&1].has_accuracy_reference);
        assert!(result.nodes[&1].performance > 0);
        assert_eq!(
            SubnetNodeReputation::<Test>::get(subnet, 1),
            Some(80 * Q / 100)
        );
    });
}

#[test]
fn cancelled_rounds_do_not_penalize_missing_reveals() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        SubnetNodeReputation::<Test>::insert(subnet, 1, 80 * Q / 100);
        LastNetworkUnpauseBlock::<Test>::put(
            MatrixRounds::<Test>::get(subnet, epoch)
                .unwrap()
                .start_block,
        );
        Network::settle_matrix_round(subnet, epoch + 1);
        assert_eq!(
            MatrixResults::<Test>::get(subnet, epoch).unwrap().outcome,
            MatrixOutcome::Cancelled
        );
        assert_eq!(
            SubnetNodeReputation::<Test>::get(subnet, 1),
            Some(80 * Q / 100)
        );
    });
}

#[test]
fn matrix_reputation_config_is_validated_and_frozen_for_each_round() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        let initial = MatrixRounds::<Test>::get(subnet, epoch).unwrap().parameters;
        for (tolerance, decrease, increase) in
            [(Q, 0, 0), (u128::MAX, 0, 0), (0, Q + 1, 0), (0, 0, Q + 1)]
        {
            assert_noop!(
                update_reputation_config(MatrixParameters {
                    reputation_tolerance: tolerance,
                    max_reputation_decrease: decrease,
                    max_reputation_increase: increase,
                    ..initial
                }),
                Error::<Test>::InvalidPercent
            );
        }
        let updated = MatrixParameters {
            reputation_tolerance: Q / 2,
            max_reputation_decrease: Q,
            max_reputation_increase: Q / 4,
            ..initial
        };
        assert_ok!(update_reputation_config(updated));
        assert_eq!(MatrixConfig::<Test>::get(), updated);
        assert_eq!(
            MatrixRounds::<Test>::get(subnet, epoch).unwrap().parameters,
            initial
        );
        SubnetNodeReputation::<Test>::insert(subnet, 1, 80 * Q / 100);
        submit_rows(subnet, epoch, &reputation_rows(0));
        Network::settle_matrix_round(subnet, epoch + 1);
        assert_eq!(
            SubnetNodeReputation::<Test>::get(subnet, 1),
            Some(88 * Q / 100)
        );
        Network::open_matrix_round(subnet, epoch + 1);
        assert_eq!(
            MatrixRounds::<Test>::get(subnet, epoch + 1)
                .unwrap()
                .parameters,
            updated
        );
        set_block_to_subnet_slot_epoch(epoch + 1, subnet);
        assert_ok!(update_reputation_config(initial));
        assert_eq!(MatrixConfig::<Test>::get(), initial);
        assert_eq!(
            MatrixRounds::<Test>::get(subnet, epoch + 1)
                .unwrap()
                .parameters,
            updated
        );
        submit_rows(subnet, epoch + 1, &reputation_rows(0));
        Network::settle_matrix_round(subnet, epoch + 2);
        assert_eq!(SubnetNodeReputation::<Test>::get(subnet, 1), Some(Q));
    });
}

#[test]
fn malformed_reputation_parameters_cancel_the_round_without_reputation_changes() {
    for invalid in [
        MatrixParameters {
            reputation_tolerance: Q,
            ..DefaultMatrixConfig::<Test>::get()
        },
        MatrixParameters {
            max_reputation_decrease: Q + 1,
            ..DefaultMatrixConfig::<Test>::get()
        },
        MatrixParameters {
            max_reputation_increase: Q + 1,
            ..DefaultMatrixConfig::<Test>::get()
        },
    ] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = fixture();
            submit(subnet, epoch, &[1, 2, 3, 4]);
            MatrixRounds::<Test>::mutate(subnet, epoch, |round| {
                round.as_mut().unwrap().parameters = invalid
            });
            SubnetNodeReputation::<Test>::insert(subnet, 1, 80 * Q / 100);
            Network::settle_matrix_round(subnet, epoch + 1);
            assert_eq!(
                MatrixResults::<Test>::get(subnet, epoch).unwrap().outcome,
                MatrixOutcome::Cancelled
            );
            assert_eq!(
                SubnetNodeReputation::<Test>::get(subnet, 1),
                Some(80 * Q / 100)
            );
        });
    }
}

#[test]
fn global_matrix_config_is_independent_of_subnet_registration() {
    new_test_ext().execute_with(|| {
        increase_epochs(50);
        RequireSubnetRegistrationWhitelist::<Test>::put(false);
        let owner = account(777);
        drop(Balances::deposit_creating(&owner, 1_000_000 * Q));
        let mut data = default_registration_subnet_data(
            0,
            MaxSubnetNodes::<Test>::get(),
            b"matrix-config".to_vec(),
            0,
            4,
        );
        data.consensus_mechanism = ConsensusMechanism::Matrix;
        assert_ok!(Network::register_subnet(
            RuntimeOrigin::signed(owner),
            u128::MAX,
            data
        ));
        let subnet = TotalSubnetUids::<Test>::get();
        let parameters = MatrixParameters {
            reputation_tolerance: 0,
            max_reputation_decrease: Q,
            max_reputation_increase: 0,
            ..DefaultMatrixConfig::<Test>::get()
        };
        assert_ok!(update_reputation_config(parameters));
        assert_eq!(MatrixConfig::<Test>::get(), parameters);
        System::assert_last_event(Event::<Test>::MatrixConfigUpdated { parameters }.into());
    });
}

#[test]
fn probation_keeps_performance_recovery_without_a_score_row() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        MatrixRounds::<Test>::remove(subnet, epoch);
        assert_ok!(Network::remove_node_from_election_slot(subnet, 4));
        SubnetNodesData::<Test>::mutate(subnet, 4, |node| {
            node.classification.node_class = SubnetNodeClass::Probation;
        });
        Network::open_matrix_round(subnet, epoch);
        let round = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
        let previous = 80 * Q / 100;
        SubnetNodeReputation::<Test>::insert(subnet, 4, previous);
        submit(subnet, epoch, &[1, 2, 3]);
        Network::settle_matrix_round(subnet, epoch + 1);
        let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
        assert!(!result.nodes[&4].has_accuracy_reference);
        assert!(result.nodes[&4].performance > 0);
        assert_eq!(
            SubnetNodeReputation::<Test>::get(subnet, 4),
            Some(Network::increase_rep(
                previous,
                round.policy.reputation_factors.included_increase,
                None
            ))
        );
    });
}

#[test]
fn probation_without_weighted_assessments_keeps_reputation_and_progress() {
    for zero_stake_reveals in [false, true] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = fixture();
            MatrixRounds::<Test>::remove(subnet, epoch);
            assert_ok!(Network::remove_node_from_election_slot(subnet, 4));
            SubnetNodesData::<Test>::mutate(subnet, 4, |node| {
                node.classification.node_class = SubnetNodeClass::Probation;
            });
            Network::open_matrix_round(subnet, epoch);
            SubnetNodeReputation::<Test>::insert(subnet, 4, 80 * Q / 100);
            SubnetNodeProbationProgress::<Test>::insert(subnet, 4, 1);
            if zero_stake_reveals {
                MatrixRounds::<Test>::mutate(subnet, epoch, |round| {
                    let round = round.as_mut().unwrap();
                    round
                        .stake
                        .weights
                        .values_mut()
                        .for_each(|weight| *weight = 0);
                    round.stake.total_weight = 0;
                });
                submit(subnet, epoch, &[1, 2, 3]);
            }
            Network::settle_matrix_round(subnet, epoch + 1);
            assert_eq!(
                SubnetNodeReputation::<Test>::get(subnet, 4),
                Some(80 * Q / 100)
            );
            assert_eq!(SubnetNodeProbationProgress::<Test>::get(subnet, 4), 1);
        });
    }
}

#[test]
fn missing_and_abstaining_rows_earn_performance_rewards_without_nonparticipation_penalties() {
    for abstain in [false, true] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = fixture();
            MatrixRounds::<Test>::mutate(subnet, epoch, |round| {
                let policy = &mut round.as_mut().unwrap().policy;
                policy.super_majority_attestation_ratio = Q;
                policy.min_weight_decrease_reputation_threshold = 0;
                policy.reputation_factors.non_attestor_decrease = Q / 10;
            });
            let round = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
            let mut rows: BTreeMap<_, _> = (1..=3).map(|id| (id, raw_row(&round, id, 1))).collect();
            if abstain {
                rows.insert(4, MatrixScores::default());
            }
            SubnetNodeReputation::<Test>::insert(subnet, 4, 80 * Q / 100);
            let before = NodeSubnetStake::<Test>::get(4, subnet);
            submit_rows(subnet, epoch, &rows);
            Network::settle_matrix_round(subnet, epoch + 1);
            let result = MatrixResults::<Test>::get(subnet, epoch).unwrap();
            assert_eq!(result.outcome, MatrixOutcome::Accepted);
            assert!(!result.nodes[&4].has_accuracy_reference);
            assert!(result.nodes[&4].performance > 0);
            assert!(result.nodes[&4].weight > 0);
            assert!(NodeSubnetStake::<Test>::get(4, subnet) > before);
            assert_eq!(
                SubnetNodeReputation::<Test>::get(subnet, 4),
                Some(80 * Q / 100)
            );
        });
    }
}

#[test]
fn governance_validates_every_matrix_policy_field_without_writes() {
    new_test_ext().execute_with(|| {
        let initial = MatrixConfig::<Test>::get();
        let mut invalids = Vec::new();
        macro_rules! invalid {
            ($field:ident, $value:expr) => {
                invalids.push(MatrixParameters {
                    $field: $value,
                    ..initial
                });
            };
        }
        invalid!(accuracy_weight, Q + 1);
        invalid!(reputation_tolerance, Q);
        invalid!(max_reputation_decrease, Q + 1);
        invalid!(max_reputation_increase, Q + 1);
        invalid!(reputation_increase_exponent, 0);
        invalid!(reputation_decrease_exponent, 0);
        invalid!(commit_phase_percentage, 0);
        invalid!(commit_phase_percentage, Q);
        invalid!(performance_threshold, Q);
        invalid!(probation_promotion_reputation, Q + 1);
        invalid!(min_nodes, MATRIX_MIN_NODES - 1);
        invalid!(min_nodes, MATRIX_MAX_NODES + 1);
        invalid!(max_nodes, MATRIX_MIN_NODES - 1);
        invalid!(max_nodes, MATRIX_MAX_NODES + 1);
        for parameters in invalids {
            assert_noop!(
                Network::set_matrix_config(governance_origin(), parameters),
                Error::<Test>::InvalidPercent
            );
        }
        assert_noop!(
            Network::set_matrix_config(
                governance_origin(),
                MatrixParameters {
                    commit_phase_percentage: 1,
                    ..initial
                }
            ),
            Error::<Test>::InvalidMatrixEpochLength
        );
        assert_eq!(MatrixConfig::<Test>::get(), initial);
        // Governance is network-wide and does not require any subnet to exist.
        let parameters = MatrixParameters {
            reputation_increase_exponent: u32::MAX,
            ..initial
        };
        assert_ok!(Network::set_matrix_config(governance_origin(), parameters));
        assert_eq!(MatrixConfig::<Test>::get(), parameters);
    });
}

#[test]
fn configured_curve_powers_control_both_directions_and_have_bounded_cost() {
    let initial = DefaultMatrixConfig::<Test>::get();
    for (exponent, expected_gain, expected_loss) in [
        (1, Q / 20, 3 * Q / 20),
        (2, Q / 40, 3 * Q / 40),
        (3, Q / 80, 3 * Q / 80),
        (u32::MAX, 0, 0),
    ] {
        let parameters = MatrixParameters {
            reputation_increase_exponent: exponent,
            reputation_decrease_exponent: exponent,
            ..initial
        };
        assert_eq!(
            math::reputation_rates(&parameters, 95 * Q / 100),
            (expected_gain, 0)
        );
        assert_eq!(
            math::reputation_rates(&parameters, 45 * Q / 100),
            (0, expected_loss)
        );
        assert_eq!(
            math::reputation_rates(&parameters, Q),
            (initial.max_reputation_increase, 0)
        );
        assert_eq!(
            math::reputation_rates(&parameters, 0),
            (0, initial.max_reputation_decrease)
        );
    }
}

#[test]
fn global_timing_and_population_limits_are_used_by_new_rounds() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        let initial = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
        let parameters = MatrixParameters {
            commit_phase_percentage: Q / 4,
            max_nodes: 3,
            ..initial.parameters
        };
        assert_ok!(Network::set_matrix_config(governance_origin(), parameters));
        assert_eq!(Network::subnet_node_capacity(subnet), 3);
        assert_eq!(MatrixRounds::<Test>::get(subnet, epoch).unwrap(), initial);
        // Lowering admission capacity does not invalidate already-admitted nodes.
        Network::open_matrix_round(subnet, epoch + 1);
        let next = MatrixRounds::<Test>::get(subnet, epoch + 1).unwrap();
        assert_eq!(next.nodes.len(), 4);
        assert_eq!(next.reveal_block - next.start_block, EpochLength::get() / 4);
        assert_eq!(next.parameters, parameters);
        assert_ok!(Network::set_matrix_config(
            governance_origin(),
            MatrixParameters {
                min_nodes: 5,
                max_nodes: 6,
                ..parameters
            }
        ));
        assert_eq!(Network::subnet_node_capacity(subnet), 6);
        assert_eq!(
            Network::can_subnet_be_active(subnet),
            (false, Some(SubnetRemovalReason::MinSubnetNodes))
        );
        Network::open_matrix_round(subnet, epoch + 2);
        assert!(!MatrixRounds::<Test>::contains_key(subnet, epoch + 2));
    });
}

#[test]
fn configured_performance_threshold_and_promotion_reputation_are_snapshotted() {
    for received in [Q / 2, Q / 2 + 1] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = fixture();
            MatrixRounds::<Test>::remove(subnet, epoch);
            assert_ok!(Network::remove_node_from_election_slot(subnet, 4));
            SubnetNodesData::<Test>::mutate(subnet, 4, |node| {
                node.classification.node_class = SubnetNodeClass::Probation;
            });
            let parameters = MatrixParameters {
                performance_threshold: Q / 2,
                probation_promotion_reputation: 80 * Q / 100,
                ..MatrixConfig::<Test>::get()
            };
            assert_ok!(Network::set_matrix_config(governance_origin(), parameters));
            SubnetNodeReputation::<Test>::insert(subnet, 4, 80 * Q / 100);
            SubnetNodeProbationProgress::<Test>::insert(
                subnet,
                4,
                ProbationRounds::<Test>::get(subnet) - 1,
            );
            Network::open_matrix_round(subnet, epoch);
            let frozen = MatrixRounds::<Test>::get(subnet, epoch).unwrap();
            // Later configuration must not alter either threshold for this round.
            assert_ok!(Network::set_matrix_config(
                governance_origin(),
                DefaultMatrixConfig::<Test>::get()
            ));
            submit_rows(
                subnet,
                epoch,
                &BTreeMap::from([(1, scores(&[(4, received)]))]),
            );
            Network::settle_matrix_round(subnet, epoch + 1);
            assert_eq!(MatrixRounds::<Test>::get(subnet, epoch).unwrap(), frozen);
            let promoted = received > parameters.performance_threshold;
            assert_eq!(
                SubnetNodesData::<Test>::get(subnet, 4)
                    .classification
                    .node_class,
                if promoted {
                    SubnetNodeClass::Active
                } else {
                    SubnetNodeClass::Probation
                }
            );
            if promoted {
                assert!(SubnetNodeReputation::<Test>::get(subnet, 4).unwrap() < Q);
            } else {
                assert!(SubnetNodeReputation::<Test>::get(subnet, 4).unwrap() < 80 * Q / 100);
                assert_eq!(SubnetNodeProbationProgress::<Test>::get(subnet, 4), 0);
            }
            assert_eq!(
                MatrixResults::<Test>::get(subnet, epoch).unwrap().nodes[&4].weight,
                0
            );
        });
    }
}
