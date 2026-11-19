use super::{mock::*, test_utils::*};
use crate::utilities::consensus_history::CONSENSUS_HISTORY_RETENTION_EPOCHS;
use crate::*;
use codec::Encode;
use frame_support::weights::{RuntimeDbWeight, Weight, WeightMeter};

const TOKEN: u128 = 1_000_000_000_000_000_000;

// The ordinary mock keeps database work free. These boundary tests need nonzero costs to
// exercise reservations, and restore the thread-local value even when an assertion fails.
struct DbWeightGuard(RuntimeDbWeight);

impl DbWeightGuard {
    fn new() -> Self {
        let previous = TestDbWeight::get();
        TestDbWeight::set(RuntimeDbWeight { read: 1, write: 2 });
        Self(previous)
    }
}

impl Drop for DbWeightGuard {
    fn drop(&mut self) {
        TestDbWeight::set(self.0);
    }
}

#[test]
fn registration_queue_preserves_the_final_write_budget_after_each_admission() {
    new_test_ext().execute_with(|| {
        let name = b"queue-final-write-budget".to_vec();
        let stake = MinSubnetMinStake::<Test>::get();
        build_activated_subnet(name.clone(), 0, 4, 10_000 * TOKEN, stake);
        let subnet = SubnetName::<Test>::get(name).unwrap();
        build_queued_subnet_nodes(subnet, 4, 6, 10_000 * TOKEN, stake);
        SubnetNodeQueueEpochs::<Test>::insert(subnet, 1);
        ChurnLimit::<Test>::insert(subnet, 2);
        ChurnLimitMultiplier::<Test>::insert(subnet, 1);
        let queue = SubnetNodeQueue::<Test>::get(subnet);
        assert_eq!(queue.len(), 2);
        let epoch = queue[0].classification.start_epoch + 2;
        assert!(queue.iter().all(|node| Network::has_epoch_period_elapsed(
            node.classification.start_epoch,
            1,
            epoch,
        )));
        let admitted_before = TotalAdmittedSubnetNodes::<Test>::get(subnet);
        let total_admitted_before = TotalAdmittedNodes::<Test>::get();

        let _guard = DbWeightGuard::new();
        let db = TestDbWeight::get();
        let selector = db.reads(7);
        let admission = db
            .reads(1)
            .saturating_add(Network::pending_subnet_node_removal_proof_weight())
            .saturating_add(db.reads_writes(4, 4));
        let final_write = db.writes(1);

        // This can pay for both admissions but cannot also persist the shortened queue.
        // The second admission must wait, preserving the final write for the first one.
        let mut meter =
            WeightMeter::with_limit(selector.saturating_add(admission.saturating_mul(2)));
        Network::handle_registration_queue(&mut meter, subnet, epoch);
        assert_eq!(
            meter.consumed(),
            selector
                .saturating_add(admission)
                .saturating_add(final_write),
        );
        assert_eq!(SubnetNodeQueue::<Test>::get(subnet), vec![queue[1].clone()]);
        assert_eq!(
            SubnetNodesData::<Test>::get(subnet, queue[0].id)
                .classification
                .node_class,
            SubnetNodeClass::Probation,
        );
        assert!(!QueuedSubnetNodesData::<Test>::contains_key(
            subnet,
            queue[0].id
        ));
        assert!(QueuedSubnetNodesData::<Test>::contains_key(
            subnet,
            queue[1].id
        ));
        assert!(!SubnetNodesData::<Test>::contains_key(subnet, queue[1].id));
        assert_eq!(
            TotalAdmittedSubnetNodes::<Test>::get(subnet),
            admitted_before + 1
        );
        assert_eq!(TotalAdmittedNodes::<Test>::get(), total_admitted_before + 1);

        // The retained entry succeeds with exactly one admission and its final queue write.
        let exact_budget = selector
            .saturating_add(admission)
            .saturating_add(final_write);
        let mut meter = WeightMeter::with_limit(exact_budget);
        Network::handle_registration_queue(&mut meter, subnet, epoch + 1);
        assert_eq!(meter.consumed(), exact_budget);
        assert!(SubnetNodeQueue::<Test>::get(subnet).is_empty());
        assert!(!QueuedSubnetNodesData::<Test>::contains_key(
            subnet,
            queue[1].id
        ));
        assert_eq!(
            SubnetNodesData::<Test>::get(subnet, queue[1].id)
                .classification
                .node_class,
            SubnetNodeClass::Probation,
        );
        assert_eq!(
            TotalAdmittedSubnetNodes::<Test>::get(subnet),
            admitted_before + 2
        );
        assert_eq!(TotalAdmittedNodes::<Test>::get(), total_admitted_before + 2);
    });
}

fn check_history_pruning_budget(commit: bool) {
    let mut ext = new_test_ext();
    let subnet = 7;
    let epoch = 0;
    let attestors = <Test as Config>::MaxSubnetNodesUpperBound::get();
    ext.execute_with(|| {
        let elected = ElectedConsensusRound {
            validator_subnet_node_id: 1,
            validator_id: 1,
            eligible_subnet_node_ids: (1..=attestors).collect(),
            eligible_validator_identity_ids: (1..=attestors).map(|node| (node, node)).collect(),
            ..Default::default()
        };
        let submission = ConsensusData::<Test> {
            validator_id: 1,
            block: 1,
            validator_epoch_progress: 0,
            validator_reward_factor: TOKEN,
            validator_ids: elected.eligible_subnet_node_ids.clone(),
            validator_identity_ids: elected.eligible_validator_identity_ids.clone(),
            attests: (1..=attestors)
                .map(|node| {
                    (
                        node,
                        AttestEntry::<Test> {
                            block: 1,
                            attestor_progress: 0,
                            reward_factor: TOKEN,
                            data: None,
                        },
                    )
                })
                .collect(),
            subnet_nodes: (1..=attestors)
                .map(|node| ConsensusSubnetNode {
                    id: node,
                    validator_id: node,
                    classification: SubnetNodeClassification {
                        node_class: SubnetNodeClass::Active,
                        start_epoch: 0,
                    },
                })
                .collect(),
            data: (1..=attestors)
                .map(|node| SubnetNodeConsensusData {
                    subnet_node_id: node,
                    score: 1,
                })
                .collect(),
            prioritize_queue_node_id: Some(1),
            remove_queue_node_id: Some(2),
            weight_sum: attestors as u128,
            eligible_validator_identity_count: attestors,
            args: None,
        };
        let weights = ConsensusAttestorWeightSnapshot {
            weights: (1..=attestors).map(|node| (node, 1)).collect(),
            total_weight: attestors as u128,
        };
        let payload = ValidatorArgs::<Test>::try_from(vec![
            1;
            <Test as Config>::ValidatorArgsLimit::get()
                as usize
        ])
        .unwrap();
        // Guard the manual encoded-value bounds against future changes to the stored types.
        // Stored proposal args and attestation payloads live in separate keys, so their
        // corresponding optional fields in ConsensusData remain None.
        assert!(elected.encoded_size() <= 12 * attestors as usize + 1_024);
        let fixed_encoded_size = elected.encoded_size()
            + submission.encoded_size()
            + weights.encoded_size()
            + true.encoded_size()
            + attestors.encoded_size()
            + payload.encoded_size();
        assert!(
            fixed_encoded_size
                <= 128 * attestors as usize
                    + 1_024
                    + <Test as Config>::ValidatorArgsLimit::get() as usize
                    + 5
        );
        SubnetElectedValidator::<Test>::insert(subnet, epoch, elected);
        SubnetConsensusSubmission::<Test>::insert(subnet, epoch, submission);
        SubnetConsensusSubmissionMaxItems::<Test>::insert(subnet, epoch, attestors);
        SubnetConsensusAttestorWeights::<Test>::insert(subnet, epoch, weights);
        SubnetConsensusProposalArgs::<Test>::insert(subnet, epoch, payload.clone());
        ConsensusRoundSettlementStatus::<Test>::insert(subnet, epoch, true);
        ConsensusHistoryPruneCursor::<Test>::insert(subnet, epoch);
        for node in 1..=attestors {
            SubnetConsensusAttestationData::<Test>::insert((subnet, epoch, node), payload.clone());
        }
    });
    // clear_prefix reports backend iteration costs only when entries are committed; an
    // overlay-only fixture would miss the per-entry reads that caused this under-reservation.
    if commit {
        ext.commit_all().unwrap();
    }

    ext.execute_with(|| {
        let _guard = DbWeightGuard::new();
        let db = TestDbWeight::get();
        let current_epoch = CONSENSUS_HISTORY_RETENTION_EPOCHS + 1;
        // One selector, one election probe, one fixed-round read, and all attestor reads
        // plus the prefix end probe; six round writes and one cursor write follow removal.
        let payload_limit = <Test as Config>::ValidatorArgsLimit::get() as u64;
        let probe_proof = 12 * attestors as u64 + 1_024 + 4_096;
        let fixed_proof = 128 * attestors as u64 + 1_024 + 6 * 4_096 + payload_limit + 5;
        let prefix_proof = (attestors as u64 + 1) * (payload_limit + 5 + 4_096);
        let exact_budget = db
            .reads_writes(attestors as u64 + 4, attestors as u64 + 7)
            .saturating_add(Weight::from_parts(
                0,
                4_096 + probe_proof + fixed_proof + prefix_proof + 4_096,
            ));
        for shortfall in [db.reads(1), Weight::from_parts(0, 1)] {
            let mut short_meter = WeightMeter::with_limit(exact_budget.saturating_sub(shortfall));
            Network::prune_consensus_history(&mut short_meter, subnet, current_epoch);
            assert_eq!(
                short_meter.consumed(),
                db.reads(2)
                    .saturating_add(Weight::from_parts(0, 4_096 + probe_proof))
            );
            assert!(SubnetElectedValidator::<Test>::contains_key(subnet, epoch));
            assert_eq!(
                ConsensusHistoryPruneCursor::<Test>::get(subnet),
                Some(epoch)
            );
            assert_eq!(
                SubnetConsensusAttestationData::<Test>::iter_prefix((subnet, epoch)).count(),
                attestors as usize,
            );
        }

        let mut exact_meter = WeightMeter::with_limit(exact_budget);
        Network::prune_consensus_history(&mut exact_meter, subnet, current_epoch);
        let expected_consumed = if commit {
            exact_budget
        } else {
            // Overlay clearing avoids backend loops, but still charges every payload's proof.
            exact_budget.saturating_sub(db.reads(attestors as u64))
        };
        assert_eq!(exact_meter.consumed(), expected_consumed);
        assert!(!SubnetElectedValidator::<Test>::contains_key(subnet, epoch));
        assert!(!ConsensusRoundSettlementStatus::<Test>::contains_key(
            subnet, epoch
        ));
        assert_eq!(
            ConsensusHistoryPruneCursor::<Test>::get(subnet),
            Some(epoch + 1)
        );
        assert!(
            SubnetConsensusAttestationData::<Test>::iter_prefix((subnet, epoch))
                .next()
                .is_none()
        );
    });
}

#[test]
fn history_pruning_reserves_every_backend_attestation_read_before_mutating() {
    check_history_pruning_budget(true);
}

#[test]
fn history_pruning_accounts_for_overlay_payload_proofs_without_backend_reads() {
    check_history_pruning_budget(false);
}
