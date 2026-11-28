use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{assert_noop, assert_ok};

const Q: u128 = PERCENTAGE_FACTOR_U128;

fn attestation_fixture() -> (u32, u32, u32, u32) {
    increase_epochs(50);
    let name = b"mechanism-gates".to_vec();
    build_activated_subnet(name.clone(), 0, 4, 10_000 * Q, 100 * Q);
    let subnet = SubnetName::<Test>::get(name).unwrap();
    let epoch = Network::get_current_subnet_epoch_as_u32(subnet);
    Network::elect_validator(subnet, epoch);
    let proposer = SubnetElectedValidator::<Test>::get(subnet, epoch)
        .unwrap()
        .validator_subnet_node_id;
    let attestor = if proposer == 1 { 2 } else { 1 };
    let hotkey = Network::get_subnet_node_associated_hotkey(subnet, proposer).unwrap();
    assert_ok!(Network::propose_attestation(
        RuntimeOrigin::signed(hotkey),
        subnet,
        (1..=4)
            .map(|subnet_node_id| SubnetNodeConsensusData {
                subnet_node_id,
                score: 1
            })
            .collect(),
        None,
        None,
        None,
        None,
    ));
    (subnet, epoch, proposer, attestor)
}

#[test]
fn matrix_rejects_attestation_even_with_round_evidence_and_pending_cleanup() {
    for direct in [false, true] {
        for quarantined in [false, true] {
            new_test_ext().execute_with(|| {
                let (subnet, _, proposer, attestor) = attestation_fixture();
                // Retain usable attestation evidence: a mechanism mismatch must be the guard.
                SubnetsData::<Test>::mutate(subnet, |s| {
                    s.as_mut().unwrap().consensus_mechanism = ConsensusMechanism::Matrix
                });
                if quarantined {
                    PendingAdmittedNodeRemovals::<Test>::mutate(subnet, |ids| {
                        ids.try_insert(proposer).unwrap();
                        ids.try_insert(attestor).unwrap();
                    });
                }
                let proposer_hotkey =
                    Network::get_subnet_node_associated_hotkey(subnet, proposer).unwrap();
                let attestor_hotkey =
                    Network::get_subnet_node_associated_hotkey(subnet, attestor).unwrap();
                if direct {
                    assert_noop!(
                        Network::do_propose_attestation(
                            proposer_hotkey,
                            subnet,
                            vec![],
                            None,
                            None,
                            None,
                            None
                        ),
                        Error::<Test>::InvalidConsensusMechanism
                    );
                    assert_noop!(
                        Network::do_attest(attestor_hotkey, subnet, attestor, None),
                        Error::<Test>::InvalidConsensusMechanism
                    );
                } else {
                    assert_noop!(
                        Network::propose_attestation(
                            RuntimeOrigin::signed(proposer_hotkey),
                            subnet,
                            vec![],
                            None,
                            None,
                            None,
                            None
                        ),
                        Error::<Test>::InvalidConsensusMechanism
                    );
                    assert_noop!(
                        Network::attest(
                            RuntimeOrigin::signed(attestor_hotkey),
                            subnet,
                            attestor,
                            None
                        ),
                        Error::<Test>::InvalidConsensusMechanism
                    );
                }
            });
        }
    }
}

#[test]
fn missing_subnet_cannot_use_attestation_even_with_round_evidence() {
    new_test_ext().execute_with(|| {
        let (subnet, _, proposer, attestor) = attestation_fixture();
        let proposer_hotkey = Network::get_subnet_node_associated_hotkey(subnet, proposer).unwrap();
        let attestor_hotkey = Network::get_subnet_node_associated_hotkey(subnet, attestor).unwrap();
        SubnetsData::<Test>::remove(subnet);
        assert_noop!(
            Network::propose_attestation(
                RuntimeOrigin::signed(proposer_hotkey),
                subnet,
                vec![],
                None,
                None,
                None,
                None
            ),
            Error::<Test>::InvalidConsensusMechanism
        );
        assert_noop!(
            Network::attest(
                RuntimeOrigin::signed(attestor_hotkey),
                subnet,
                attestor,
                None
            ),
            Error::<Test>::InvalidConsensusMechanism
        );
    });
}

#[test]
fn attestation_rejects_matrix_submissions_without_mutation() {
    for direct in [false, true] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch, proposer, _) = attestation_fixture();
            let hotkey = Network::get_subnet_node_associated_hotkey(subnet, proposer).unwrap();
            if direct {
                assert_noop!(
                    Network::do_commit_matrix_scores(
                        hotkey.clone(),
                        subnet,
                        epoch,
                        proposer,
                        Default::default()
                    ),
                    Error::<Test>::InvalidConsensusMechanism
                );
                assert_noop!(
                    Network::do_reveal_matrix_scores(
                        hotkey,
                        subnet,
                        epoch,
                        proposer,
                        MatrixScores::default(),
                        [0; 32]
                    ),
                    Error::<Test>::InvalidConsensusMechanism
                );
            } else {
                assert_noop!(
                    Network::commit_matrix_scores(
                        RuntimeOrigin::signed(hotkey.clone()),
                        subnet,
                        epoch,
                        proposer,
                        Default::default()
                    ),
                    Error::<Test>::InvalidConsensusMechanism
                );
                assert_noop!(
                    Network::reveal_matrix_scores(
                        RuntimeOrigin::signed(hotkey),
                        subnet,
                        epoch,
                        proposer,
                        MatrixScores::default(),
                        [0; 32]
                    ),
                    Error::<Test>::InvalidConsensusMechanism
                );
            }
        });
    }
}

#[test]
fn selected_attestation_mechanism_still_accepts_proposal_and_attestation() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch, _, attestor) = attestation_fixture();
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, attestor).unwrap();
        assert_ok!(Network::attest(
            RuntimeOrigin::signed(hotkey),
            subnet,
            attestor,
            None
        ));
        assert!(SubnetConsensusSubmission::<Test>::get(subnet, epoch).is_some());
    });
}
