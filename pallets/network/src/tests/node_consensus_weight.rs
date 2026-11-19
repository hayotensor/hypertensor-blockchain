use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{assert_noop, assert_ok, dispatch::DispatchResultWithPostInfo};

const Q18: u128 = 1_000_000_000_000_000_000;

fn fixture() -> (u32, u32, u32, AccountId) {
    increase_epochs(50);
    let name = b"node-consensus-weight".to_vec();
    build_activated_subnet(
        name.clone(),
        0,
        4,
        10_000 * Q18,
        MinSubnetMinStake::<Test>::get(),
    );
    let subnet = SubnetName::<Test>::get(name).unwrap();
    seed_equal_validator_delegate_stake_for_subnet(subnet);
    set_block_to_subnet_slot_epoch(Network::get_current_epoch_as_u32(), subnet);
    let epoch = Network::get_current_subnet_epoch_as_u32(subnet);
    Network::elect_validator(subnet, epoch);
    let proposer = SubnetElectedValidator::<Test>::get(subnet, epoch)
        .unwrap()
        .validator_subnet_node_id;
    let hotkey = Network::get_subnet_node_associated_hotkey(subnet, proposer).unwrap();
    (subnet, epoch, proposer, hotkey)
}

fn submit(subnet: u32, hotkey: &AccountId, scores: &[(u32, u128)]) -> DispatchResultWithPostInfo {
    Network::propose_attestation(
        RuntimeOrigin::signed(hotkey.clone()),
        subnet,
        scores
            .iter()
            .map(|&(subnet_node_id, score)| SubnetNodeConsensusData {
                subnet_node_id,
                score,
            })
            .collect(),
        None,
        None,
        Some(vec![1].try_into().unwrap()),
        Some(vec![2].try_into().unwrap()),
    )
}

fn set_cap(value: u128) {
    assert_ok!(Network::set_max_subnet_node_consensus_weight(
        RuntimeOrigin::from(pallet_collective::RawOrigin::Members(4, 5)),
        value,
    ));
}

#[test]
fn collective_updates_cap_and_emits_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_eq!(MaxSubnetNodeConsensusWeight::<Test>::get(), Q18 / 2);
        for value in [1, Q18 / 4, Q18] {
            set_cap(value);
            assert_eq!(MaxSubnetNodeConsensusWeight::<Test>::get(), value);
            System::assert_last_event(RuntimeEvent::Network(
                Event::SetMaxSubnetNodeConsensusWeight(value),
            ));
        }
    });
}

#[test]
fn cap_updates_reject_unauthorized_origins_and_invalid_q18_values() {
    new_test_ext().execute_with(|| {
        for origin in [
            RuntimeOrigin::signed(account(1)),
            RuntimeOrigin::root(),
            RuntimeOrigin::none(),
            RuntimeOrigin::from(pallet_collective::RawOrigin::Members(2, 3)),
        ] {
            assert_noop!(
                Network::set_max_subnet_node_consensus_weight(origin, Q18),
                sp_runtime::DispatchError::BadOrigin
            );
        }
        for value in [0, Q18 + 1, u128::MAX] {
            assert_noop!(
                Network::set_max_subnet_node_consensus_weight(
                    RuntimeOrigin::from(pallet_collective::RawOrigin::Members(4, 5)),
                    value,
                ),
                Error::<Test>::InvalidPercent
            );
        }
    });
}

#[test]
fn accepts_at_or_below_cap_with_arbitrary_scores_and_q18_floor_rounding() {
    for scores in [
        vec![],
        vec![0, 0, 0],
        vec![1, 1],
        vec![50, 25, 25],
        vec![Q18 / 2, Q18 / 4, Q18 / 4],
        vec![u128::MAX / 2, u128::MAX / 2, 1],
        // A sub-Q18 remainder rounds down, matching percent_div in settlement.
        vec![u128::MAX / 2 + 1, u128::MAX / 2],
    ] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch, _, hotkey) = fixture();
            let entries: Vec<_> = scores
                .iter()
                .enumerate()
                .map(|(i, &s)| (i as u32 + 1, s))
                .collect();
            assert_ok!(submit(subnet, &hotkey, &entries));
            let stored = SubnetConsensusSubmission::<Test>::get(subnet, epoch).unwrap();
            assert_eq!(stored.weight_sum, scores.iter().sum::<u128>());
            assert_eq!(
                stored.data.iter().map(|d| d.score).collect::<Vec<_>>(),
                scores
            );
        });
    }
}

#[test]
fn rejects_overweight_scores_atomically_and_allows_corrected_retry() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch, _, hotkey) = fixture();
        for entries in [
            vec![(1, 1)],
            vec![(1, 1), (2, 0), (3, 0)],
            vec![(1, 49), (2, 51)],
            vec![(1, 1), (2, 2)],
            vec![(1, Q18 / 2 - 1), (2, Q18 / 2 + 1)],
            vec![(1, u128::MAX)],
        ] {
            assert_noop!(
                submit(subnet, &hotkey, &entries),
                Error::<Test>::MaxSubnetNodeConsensusWeightExceeded
            );
            assert!(!SubnetConsensusSubmission::<Test>::contains_key(
                subnet, epoch
            ));
            assert!(!SubnetConsensusProposalArgs::<Test>::contains_key(
                subnet, epoch
            ));
        }
        assert_ok!(submit(subnet, &hotkey, &[(1, 1), (2, 1)]));
    });
}

#[test]
fn duplicate_scores_cannot_dilute_weight_but_lowest_score_can_fix_it() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch, _, hotkey) = fixture();
        // Raw sum is 120, but the canonical vector is [60, 30].
        assert_noop!(
            submit(subnet, &hotkey, &[(1, 60), (2, 30), (2, 30)]),
            Error::<Test>::MaxSubnetNodeConsensusWeightExceeded
        );
        assert_ok!(submit(subnet, &hotkey, &[(1, u128::MAX), (2, 30), (1, 30)]));
        assert_eq!(
            SubnetConsensusSubmission::<Test>::get(subnet, epoch)
                .unwrap()
                .weight_sum,
            60
        );
    });
}

#[test]
fn missing_queued_and_pending_nodes_cannot_dilute_weight() {
    for kind in 0..3 {
        new_test_ext().execute_with(|| {
            let (subnet, _, proposer, hotkey) = fixture();
            let excluded = (1..=4).find(|id| *id != proposer).unwrap();
            let other = (1..=4)
                .find(|id| *id != proposer && *id != excluded)
                .unwrap();
            match kind {
                0 => SubnetNodesData::<Test>::remove(subnet, excluded),
                1 => SubnetNodesData::<Test>::mutate(subnet, excluded, |node| {
                    node.classification.node_class = SubnetNodeClass::Queued;
                }),
                _ => PendingAdmittedNodeRemovals::<Test>::mutate(subnet, |pending| {
                    pending.try_insert(excluded).unwrap();
                }),
            }
            assert_noop!(
                submit(
                    subnet,
                    &hotkey,
                    &[(proposer, 60), (other, 30), (excluded, 30)]
                ),
                Error::<Test>::MaxSubnetNodeConsensusWeightExceeded
            );
            // Discarded huge scores neither overflow the sum nor fail the cap check.
            assert_ok!(submit(
                subnet,
                &hotkey,
                &[(proposer, 30), (other, 30), (excluded, u128::MAX)]
            ));
        });
    }
}

#[test]
fn probation_node_scores_are_subject_to_the_cap() {
    new_test_ext().execute_with(|| {
        let (subnet, _, proposer, hotkey) = fixture();
        let probation = (1..=4).find(|id| *id != proposer).unwrap();
        SubnetNodesData::<Test>::mutate(subnet, probation, |node| {
            node.classification.node_class = SubnetNodeClass::Probation;
        });
        assert_noop!(
            submit(subnet, &hotkey, &[(proposer, 40), (probation, 60)]),
            Error::<Test>::MaxSubnetNodeConsensusWeightExceeded
        );
        assert_ok!(submit(subnet, &hotkey, &[(proposer, 50), (probation, 50)]));
    });
}

#[test]
fn updated_cap_applies_to_new_submissions_without_rewriting_accepted_proposals() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch, proposer, hotkey) = fixture();
        set_cap(3 * Q18 / 4);
        assert_noop!(
            submit(subnet, &hotkey, &[(1, 76), (2, 24)]),
            Error::<Test>::MaxSubnetNodeConsensusWeightExceeded
        );
        assert_ok!(submit(subnet, &hotkey, &[(1, 75), (2, 25)]));
        let stored = SubnetConsensusSubmission::<Test>::get(subnet, epoch).unwrap();
        set_cap(Q18 / 4);
        assert_eq!(
            SubnetConsensusSubmission::<Test>::get(subnet, epoch).unwrap(),
            stored
        );
        let attestor = (1..=4).find(|id| *id != proposer).unwrap();
        assert_ok!(Network::attest(
            RuntimeOrigin::signed(
                Network::get_subnet_node_associated_hotkey(subnet, attestor).unwrap()
            ),
            subnet,
            attestor,
            None,
        ));
    });
    new_test_ext().execute_with(|| {
        let (subnet, _, _, hotkey) = fixture();
        set_cap(Q18 / 4);
        assert_noop!(
            submit(subnet, &hotkey, &[(1, 1), (2, 1)]),
            Error::<Test>::MaxSubnetNodeConsensusWeightExceeded
        );
        assert_ok!(submit(subnet, &hotkey, &[(1, 1), (2, 1), (3, 1), (4, 1)]));
    });
    new_test_ext().execute_with(|| {
        let (subnet, _, _, hotkey) = fixture();
        set_cap(Q18);
        assert_ok!(submit(subnet, &hotkey, &[(1, u128::MAX)]));
    });
}
