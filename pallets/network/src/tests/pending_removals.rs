use super::mock::*;
use super::test_utils::*;
use crate::weights::WeightInfo;
use crate::{
    AttestEntry, ConsensusPolicySnapshot, ConsensusSubmissionData, ConsensusSubnetNode,
    EmergencySubnetNodeElectionData, EmergencySubnetValidatorData, Error, Event, MinSubnetMinStake,
    MinSubnetNodes, PendingActiveNodeRemovals, PendingRegisteredNodeRemovals,
    RegisteredSubnetNodesData, RewardsData, SubnetConsensusAttestorWeights,
    SubnetConsensusSubmission, SubnetElectedValidator, SubnetName, SubnetNodeClass,
    SubnetNodeConsensusData, SubnetNodeElectionSlots, SubnetNodeReputation, SubnetNodeValidatorId,
    SubnetNodesData, SubnetOwner, SubnetReputationFactors, SubnetState,
};
use frame_support::pallet_prelude::Pays;
use frame_support::traits::{Get, Hooks};
use frame_support::weights::{Weight, WeightMeter};
use frame_support::{assert_err, assert_ok};
use sp_std::collections::btree_map::BTreeMap;

fn mark_active_pending(subnet_id: u32, subnet_node_id: u32) {
    PendingActiveNodeRemovals::<Test>::mutate(subnet_id, |pending| {
        pending
            .try_insert(subnet_node_id)
            .expect("test pending-active set fits its bound");
    });
}

fn mark_registered_pending(subnet_id: u32, subnet_node_id: u32) {
    PendingRegisteredNodeRemovals::<Test>::mutate(subnet_id, |pending| {
        pending
            .try_insert(subnet_node_id)
            .expect("test pending-registered set fits its bound");
    });
}

fn seed_active_pending(subnet_id: u32) -> (u32, AccountId) {
    let validator_id = subnet_id.saturating_mul(10);
    let coldkey_number = 100_000u32.saturating_add(subnet_id.saturating_mul(10));
    insert_subnet_node(
        validator_id,
        subnet_id,
        coldkey_number,
        coldkey_number.saturating_add(1),
        coldkey_number.saturating_add(2),
        SubnetNodeClass::Validator,
        0,
    );
    let subnet_node_id = 1;
    mark_active_pending(subnet_id, subnet_node_id);
    (subnet_node_id, account(coldkey_number))
}

fn assert_active_pending_cleaned(subnet_id: u32, subnet_node_id: u32) {
    assert!(!SubnetNodesData::<Test>::contains_key(
        subnet_id,
        subnet_node_id
    ));
    assert!(PendingActiveNodeRemovals::<Test>::get(subnet_id).is_empty());
}

#[test]
fn mandatory_reward_and_election_envelope_fits_the_hook_budget() {
    <Network as Hooks<BlockNumber>>::integrity_test();
}

fn seed_election_candidates(subnet_id: u32, candidates: &[u32]) {
    SubnetNodeElectionSlots::<Test>::insert(subnet_id, candidates.to_vec());
    for subnet_node_id in candidates {
        SubnetNodeValidatorId::<Test>::insert(
            subnet_id,
            subnet_node_id,
            subnet_id
                .saturating_mul(1_000)
                .saturating_add(*subnet_node_id),
        );
    }
}

fn find_election_block(
    subnet_id: u32,
    subnet_epoch: u32,
    candidate_count: usize,
    wanted_index: impl Fn(usize) -> bool,
) -> (u32, usize) {
    (0..10_000)
        .find_map(|block| {
            let index = Network::get_bounded_random_index(
                (subnet_id, subnet_epoch, block),
                candidate_count as u32,
            )? as usize;
            wanted_index(index).then_some((block, index))
        })
        .expect("bounded election sampler reaches the requested test index")
}

#[test]
fn election_scans_to_the_next_healthy_candidate_wraps_and_handles_all_pending() {
    new_test_ext().execute_with(|| {
        // The mock collective-flip provider is initialized from the first post-genesis block.
        System::set_block_number(1);
        let candidates = vec![10, 20, 30, 40];

        // A pending random selection advances exactly once through the canonical physical list.
        let successor_subnet_id = 11;
        let successor_epoch = 7;
        seed_election_candidates(successor_subnet_id, &candidates);
        let (successor_block, selected_index) = find_election_block(
            successor_subnet_id,
            successor_epoch,
            candidates.len(),
            |index| index + 1 < candidates.len(),
        );
        mark_active_pending(successor_subnet_id, candidates[selected_index]);

        Network::elect_validator(successor_subnet_id, successor_epoch, successor_block);

        let successor_round =
            SubnetElectedValidator::<Test>::get(successor_subnet_id, successor_epoch).unwrap();
        assert_eq!(
            successor_round.validator_subnet_node_id,
            candidates[selected_index + 1]
        );
        assert_eq!(
            successor_round.eligible_subnet_node_ids,
            candidates
                .iter()
                .copied()
                .filter(|id| *id != candidates[selected_index])
                .collect::<Vec<_>>()
        );
        assert_eq!(
            SubnetNodeElectionSlots::<Test>::get(successor_subnet_id),
            candidates
        );

        // The same scan wraps from the final physical index back to zero without rerandomizing.
        let wrap_subnet_id = 12;
        let wrap_epoch = 8;
        seed_election_candidates(wrap_subnet_id, &candidates);
        let (wrap_block, selected_index) =
            find_election_block(wrap_subnet_id, wrap_epoch, candidates.len(), |index| {
                index + 1 == candidates.len()
            });
        mark_active_pending(wrap_subnet_id, candidates[selected_index]);

        Network::elect_validator(wrap_subnet_id, wrap_epoch, wrap_block);

        assert_eq!(
            SubnetElectedValidator::<Test>::get(wrap_subnet_id, wrap_epoch)
                .unwrap()
                .validator_subnet_node_id,
            candidates[0]
        );
        assert_eq!(
            SubnetNodeElectionSlots::<Test>::get(wrap_subnet_id),
            candidates
        );

        // One complete cycle with no healthy candidate stores no partial election.
        let all_pending_subnet_id = 13;
        let all_pending_epoch = 9;
        seed_election_candidates(all_pending_subnet_id, &candidates);
        for subnet_node_id in &candidates {
            mark_active_pending(all_pending_subnet_id, *subnet_node_id);
        }

        Network::elect_validator(all_pending_subnet_id, all_pending_epoch, 0);

        assert!(!SubnetElectedValidator::<Test>::contains_key(
            all_pending_subnet_id,
            all_pending_epoch
        ));
        assert_eq!(
            SubnetNodeElectionSlots::<Test>::get(all_pending_subnet_id),
            candidates
        );
    });
}

#[test]
fn election_falls_back_when_too_few_emergency_candidates_are_healthy() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let subnet_id = 21;
        let subnet_epoch = 0;
        let minimum = MinSubnetNodes::<Test>::get();
        let candidate_count = minimum.saturating_add(2);

        for offset in 0..candidate_count {
            let ordinal = offset.saturating_add(1);
            insert_subnet_node(
                1_000 + ordinal,
                subnet_id,
                10_000 + ordinal,
                20_000 + ordinal,
                30_000 + ordinal,
                SubnetNodeClass::Validator,
                subnet_epoch,
            );
        }

        let regular_candidates = (1..=candidate_count).collect::<Vec<_>>();
        let emergency_candidates = (1..=minimum).collect::<Vec<_>>();
        SubnetNodeElectionSlots::<Test>::insert(subnet_id, regular_candidates.clone());
        EmergencySubnetNodeElectionData::<Test>::insert(
            subnet_id,
            EmergencySubnetValidatorData {
                subnet_node_ids: emergency_candidates.clone(),
                target_emergency_validators_epochs: 10,
                max_emergency_validators_epoch: u32::MAX,
                activated: true,
                started_subnet_epoch: subnet_epoch,
                ..Default::default()
            },
        );

        // Quarantining one emergency candidate takes that set below MinSubnetNodes, while the
        // larger regular set remains viable.
        mark_active_pending(subnet_id, emergency_candidates[0]);
        Network::elect_validator(subnet_id, subnet_epoch, 0);

        let round = SubnetElectedValidator::<Test>::get(subnet_id, subnet_epoch).unwrap();
        assert!(round.emergency.is_none());
        assert_eq!(
            round.eligible_subnet_node_ids,
            regular_candidates
                .iter()
                .copied()
                .filter(|id| *id != emergency_candidates[0])
                .collect::<Vec<_>>()
        );
        assert_eq!(
            SubnetNodeElectionSlots::<Test>::get(subnet_id),
            regular_candidates
        );
        assert!(!EmergencySubnetNodeElectionData::<Test>::contains_key(
            subnet_id
        ));
    });
}

#[test]
fn accepted_queue_removal_quarantines_and_dequeues_before_physical_cleanup() {
    new_test_ext().execute_with(|| {
        let subnet_id = 31;
        let validator_id = 301;
        let coldkey_number = 30_001;
        let subnet_node_id = 1;

        insert_subnet_node(
            validator_id,
            subnet_id,
            coldkey_number,
            30_002,
            30_003,
            SubnetNodeClass::Registered,
            0,
        );
        let registered_node = RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        let accepted_submission = ConsensusSubmissionData::<Test> {
            policy: Default::default(),
            validator_subnet_node_id: 0,
            validator_delegate_stake_balance: 0,
            validator_epoch_progress: 0,
            validator_reward_factor: 0,
            attestation_ratio: 1,
            identity_attestation_ratio: 0,
            identity_attestation_count: 0,
            eligible_validator_identity_count: 0,
            weight_sum: 0,
            data_length: 0,
            data: vec![],
            attests: Default::default(),
            subnet_nodes: vec![],
            prioritize_queue_node_id: None,
            remove_queue_node_id: Some(subnet_node_id),
            emergency: None,
        };

        let newly_pending = Network::handle_node_queue_consensus(
            &mut WeightMeter::new(),
            subnet_id,
            &accepted_submission,
            1,
        );

        assert_eq!(newly_pending.as_slice(), &[subnet_node_id]);
        assert!(PendingRegisteredNodeRemovals::<Test>::get(subnet_id).contains(&subnet_node_id));
        assert!(crate::SubnetNodeQueue::<Test>::get(subnet_id).is_empty());
        assert!(RegisteredSubnetNodesData::<Test>::contains_key(
            subnet_id,
            subnet_node_id
        ));

        // Even a stale direct activation attempt cannot bypass logical quarantine.
        assert!(!Network::do_activate_subnet_node(
            &mut WeightMeter::new(),
            subnet_id,
            SubnetState::Active,
            registered_node,
            1,
            true,
        ));
        assert!(!SubnetNodesData::<Test>::contains_key(
            subnet_id,
            subnet_node_id
        ));

        // An authenticated node update becomes cleanup-only and clears the durable marker.
        let requested_metadata = Some(vec![1, 2, 3].try_into().unwrap());
        assert_ok!(Network::update_node_non_unique(
            RuntimeOrigin::signed(account(coldkey_number)),
            subnet_id,
            subnet_node_id,
            requested_metadata,
        ));
        assert!(!RegisteredSubnetNodesData::<Test>::contains_key(
            subnet_id,
            subnet_node_id
        ));
        assert!(PendingRegisteredNodeRemovals::<Test>::get(subnet_id).is_empty());
    });
}

#[test]
fn pending_proposal_and_attestation_are_authenticated_cleanup_only_and_pay_fees() {
    new_test_ext().execute_with(|| {
        let subnet_name = b"pending-consensus-call-cleanup".to_vec();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let general_epoch = Network::get_current_epoch_as_u32();
        set_block_to_subnet_slot_epoch(general_epoch, subnet_id);
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        let proposer_node_id = 1;
        insert_elected_subnet_node(subnet_id, subnet_epoch, proposer_node_id);
        let proposer_hotkey =
            Network::get_subnet_node_associated_hotkey(subnet_id, proposer_node_id).unwrap();
        mark_active_pending(subnet_id, proposer_node_id);

        assert_err!(
            Network::propose_attestation(
                RuntimeOrigin::signed(account(90_001)),
                subnet_id,
                vec![],
                None,
                None,
                None,
                None,
            ),
            Error::<Test>::InvalidValidator
        );
        assert!(SubnetNodesData::<Test>::contains_key(
            subnet_id,
            proposer_node_id
        ));
        assert!(PendingActiveNodeRemovals::<Test>::get(subnet_id).contains(&proposer_node_id));

        let proposer_post_info = Network::propose_attestation(
            RuntimeOrigin::signed(proposer_hotkey),
            subnet_id,
            (0..=NetworkMaxSubnetNodesUpperBound::get())
                .map(|subnet_node_id| SubnetNodeConsensusData {
                    subnet_node_id,
                    score: 1,
                })
                .collect(),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(proposer_post_info.pays_fee, Pays::Yes);
        assert!(!SubnetConsensusSubmission::<Test>::contains_key(
            subnet_id,
            subnet_epoch
        ));
        assert!(!SubnetNodesData::<Test>::contains_key(
            subnet_id,
            proposer_node_id
        ));
        assert!(PendingActiveNodeRemovals::<Test>::get(subnet_id).is_empty());

        let attestor_node_id = 2;
        let attestor_hotkey =
            Network::get_subnet_node_associated_hotkey(subnet_id, attestor_node_id).unwrap();
        mark_active_pending(subnet_id, attestor_node_id);

        assert_err!(
            Network::attest(
                RuntimeOrigin::signed(account(90_002)),
                subnet_id,
                attestor_node_id,
                None,
            ),
            Error::<Test>::InvalidValidator
        );
        assert!(SubnetNodesData::<Test>::contains_key(
            subnet_id,
            attestor_node_id
        ));

        let attestor_post_info = Network::attest(
            RuntimeOrigin::signed(attestor_hotkey),
            subnet_id,
            attestor_node_id,
            None,
        )
        .unwrap();
        assert_eq!(attestor_post_info.pays_fee, Pays::Yes);
        assert!(!SubnetNodesData::<Test>::contains_key(
            subnet_id,
            attestor_node_id
        ));
        assert!(PendingActiveNodeRemovals::<Test>::get(subnet_id).is_empty());
    });
}

#[test]
fn pending_scores_are_filtered_before_overflow_and_stored_snapshots_stay_frozen() {
    new_test_ext().execute_with(|| {
        let subnet_name = b"pending-score-filtering".to_vec();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let general_epoch = Network::get_current_epoch_as_u32();
        set_block_to_subnet_slot_epoch(general_epoch, subnet_id);
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        Network::elect_validator(subnet_id, subnet_epoch, System::block_number());

        let round = SubnetElectedValidator::<Test>::get(subnet_id, subnet_epoch).unwrap();
        let proposer_node_id = round.validator_subnet_node_id;
        let pending_node_id = round
            .eligible_subnet_node_ids
            .iter()
            .copied()
            .find(|id| *id != proposer_node_id)
            .unwrap();
        let healthy_scored_node_id = round
            .eligible_subnet_node_ids
            .iter()
            .copied()
            .find(|id| *id != proposer_node_id && *id != pending_node_id)
            .unwrap();
        let later_pending_node_id = round
            .eligible_subnet_node_ids
            .iter()
            .copied()
            .find(|id| {
                *id != proposer_node_id && *id != pending_node_id && *id != healthy_scored_node_id
            })
            .unwrap();
        mark_active_pending(subnet_id, pending_node_id);

        let proposer_hotkey =
            Network::get_subnet_node_associated_hotkey(subnet_id, proposer_node_id).unwrap();
        let post_info = Network::propose_attestation(
            RuntimeOrigin::signed(proposer_hotkey),
            subnet_id,
            vec![
                SubnetNodeConsensusData {
                    subnet_node_id: pending_node_id,
                    score: u128::MAX,
                },
                SubnetNodeConsensusData {
                    subnet_node_id: healthy_scored_node_id,
                    score: 1,
                },
            ],
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(post_info.pays_fee, Pays::No);

        let stored_submission =
            SubnetConsensusSubmission::<Test>::get(subnet_id, subnet_epoch).unwrap();
        assert_eq!(
            stored_submission.data,
            vec![SubnetNodeConsensusData {
                subnet_node_id: healthy_scored_node_id,
                score: 1,
            }]
        );
        assert!(!stored_submission.validator_ids.contains(&pending_node_id));
        assert!(!stored_submission
            .validator_identity_ids
            .contains_key(&pending_node_id));
        assert!(!stored_submission
            .subnet_nodes
            .iter()
            .any(|node| node.id == pending_node_id));

        let stored_weights =
            SubnetConsensusAttestorWeights::<Test>::get(subnet_id, subnet_epoch).unwrap();
        assert!(!stored_weights.weights.contains_key(&pending_node_id));

        // Quarantine changes after proposal storage do not rewrite the historical round.
        mark_active_pending(subnet_id, later_pending_node_id);
        assert_eq!(
            SubnetConsensusSubmission::<Test>::get(subnet_id, subnet_epoch).unwrap(),
            stored_submission
        );
        assert_eq!(
            SubnetConsensusAttestorWeights::<Test>::get(subnet_id, subnet_epoch).unwrap(),
            stored_weights
        );
    });
}

#[test]
fn proposal_ignores_queue_requests_for_pending_registered_nodes() {
    new_test_ext().execute_with(|| {
        let subnet_name = b"pending-registered-queue-requests".to_vec();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let general_epoch = Network::get_current_epoch_as_u32();
        set_block_to_subnet_slot_epoch(general_epoch, subnet_id);
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        Network::elect_validator(subnet_id, subnet_epoch, System::block_number());

        insert_subnet_node(
            930,
            subnet_id,
            93_001,
            93_002,
            93_003,
            SubnetNodeClass::Registered,
            subnet_epoch,
        );
        let pending_registered_node_id = crate::TotalSubnetNodeUids::<Test>::get(subnet_id);
        mark_registered_pending(subnet_id, pending_registered_node_id);
        assert!(crate::SubnetNodeQueue::<Test>::get(subnet_id)
            .iter()
            .any(|node| node.id == pending_registered_node_id));

        let proposer_node_id = SubnetElectedValidator::<Test>::get(subnet_id, subnet_epoch)
            .unwrap()
            .validator_subnet_node_id;
        let proposer_hotkey =
            Network::get_subnet_node_associated_hotkey(subnet_id, proposer_node_id).unwrap();
        let post_info = Network::propose_attestation(
            RuntimeOrigin::signed(proposer_hotkey),
            subnet_id,
            vec![],
            Some(pending_registered_node_id),
            Some(pending_registered_node_id),
            None,
            None,
        )
        .unwrap();
        assert_eq!(post_info.pays_fee, Pays::No);

        let stored = SubnetConsensusSubmission::<Test>::get(subnet_id, subnet_epoch).unwrap();
        assert_eq!(stored.prioritize_queue_node_id, None);
        assert_eq!(stored.remove_queue_node_id, None);
        assert!(RegisteredSubnetNodesData::<Test>::contains_key(
            subnet_id,
            pending_registered_node_id
        ));
        assert!(PendingRegisteredNodeRemovals::<Test>::get(subnet_id)
            .contains(&pending_registered_node_id));
    });
}

#[test]
fn pending_remove_stake_cleans_first_and_withdraws_only_on_retry() {
    new_test_ext().execute_with(|| {
        let subnet_name = b"pending-remove-stake-retry".to_vec();
        let stake = MinSubnetMinStake::<Test>::get();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            stake,
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let subnet_node_id = 1;
        let coldkey =
            Network::get_subnet_node_associated_coldkey(subnet_id, subnet_node_id).unwrap();
        let stake_before = crate::NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id);
        assert_eq!(stake_before, stake);
        mark_active_pending(subnet_id, subnet_node_id);

        assert_err!(
            Network::remove_node_stake(
                RuntimeOrigin::signed(account(91_001)),
                subnet_id,
                subnet_node_id,
                stake_before,
            ),
            Error::<Test>::NotKeyOwner
        );
        assert!(SubnetNodesData::<Test>::contains_key(
            subnet_id,
            subnet_node_id
        ));
        assert_eq!(
            crate::NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id),
            stake_before
        );

        // First call is cleanup-only: physical state and marker go away, retained stake does not.
        assert_ok!(Network::remove_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            stake_before,
        ));
        assert!(!SubnetNodesData::<Test>::contains_key(
            subnet_id,
            subnet_node_id
        ));
        assert!(PendingActiveNodeRemovals::<Test>::get(subnet_id).is_empty());
        assert_eq!(
            crate::NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id),
            stake_before
        );

        // The retained validator ownership index lets the same owner withdraw on the retry.
        assert_ok!(Network::remove_node_stake(
            RuntimeOrigin::signed(coldkey),
            subnet_id,
            subnet_node_id,
            stake_before,
        ));
        assert_eq!(
            crate::NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id),
            0
        );
    });
}

#[test]
fn pending_self_removal_bypasses_the_elected_validator_guard_after_authentication() {
    new_test_ext().execute_with(|| {
        let subnet_name = b"pending-elected-self-removal".to_vec();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let subnet_node_id = 1;
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let owner = Network::get_subnet_node_associated_coldkey(subnet_id, subnet_node_id).unwrap();
        insert_elected_subnet_node(subnet_id, subnet_epoch, subnet_node_id);

        // The ordinary self-removal path still rejects the currently elected validator.
        assert_err!(
            Network::remove_subnet_node(
                RuntimeOrigin::signed(owner.clone()),
                subnet_id,
                subnet_node_id,
            ),
            Error::<Test>::ElectedValidatorCannotRemove
        );
        assert!(SubnetNodesData::<Test>::contains_key(
            subnet_id,
            subnet_node_id
        ));

        mark_active_pending(subnet_id, subnet_node_id);
        assert!(Network::remove_subnet_node(
            RuntimeOrigin::signed(account(92_001)),
            subnet_id,
            subnet_node_id,
        )
        .is_err());
        assert!(SubnetNodesData::<Test>::contains_key(
            subnet_id,
            subnet_node_id
        ));
        assert!(PendingActiveNodeRemovals::<Test>::get(subnet_id).contains(&subnet_node_id));

        // Exact owner authentication reaches cleanup before the elected-validator prohibition.
        assert_ok!(Network::remove_subnet_node(
            RuntimeOrigin::signed(owner),
            subnet_id,
            subnet_node_id,
        ));
        assert_active_pending_cleaned(subnet_id, subnet_node_id);
    });
}

#[test]
fn direct_physical_removal_clears_both_marker_classes() {
    new_test_ext().execute_with(|| {
        let subnet_id = 41;
        let subnet_node_id = 1;
        insert_subnet_node(
            401,
            subnet_id,
            40_001,
            40_002,
            40_003,
            SubnetNodeClass::Validator,
            0,
        );
        mark_active_pending(subnet_id, subnet_node_id);
        // A mismatched stale marker must not survive the common terminal removal path either.
        mark_registered_pending(subnet_id, subnet_node_id);

        assert!(Network::remove_active_subnet_node(
            subnet_id,
            subnet_node_id
        ));

        assert!(PendingActiveNodeRemovals::<Test>::get(subnet_id).is_empty());
        assert!(PendingRegisteredNodeRemovals::<Test>::get(subnet_id).is_empty());
        assert!(!SubnetNodesData::<Test>::contains_key(
            subnet_id,
            subnet_node_id
        ));
    });
}

#[test]
fn registered_collective_and_whole_subnet_removals_clear_pending_markers() {
    new_test_ext().execute_with(|| {
        // Direct registered-node removal clears both its real marker and any stale mismatched one.
        let registered_subnet_id = 42;
        let registered_node_id = 1;
        insert_subnet_node(
            402,
            registered_subnet_id,
            42_001,
            42_002,
            42_003,
            SubnetNodeClass::Registered,
            0,
        );
        mark_registered_pending(registered_subnet_id, registered_node_id);
        mark_active_pending(registered_subnet_id, registered_node_id);

        Network::remove_registered_subnet_node(registered_subnet_id, registered_node_id);

        assert!(!RegisteredSubnetNodesData::<Test>::contains_key(
            registered_subnet_id,
            registered_node_id
        ));
        assert!(PendingRegisteredNodeRemovals::<Test>::get(registered_subnet_id).is_empty());
        assert!(PendingActiveNodeRemovals::<Test>::get(registered_subnet_id).is_empty());

        // Collective removal of one active node also terminates both marker classes.
        let collective_name = b"pending-collective-node-removal".to_vec();
        build_activated_subnet(
            collective_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let collective_subnet_id = SubnetName::<Test>::get(collective_name).unwrap();
        let collective_node_id = 1;
        mark_active_pending(collective_subnet_id, collective_node_id);
        mark_registered_pending(collective_subnet_id, collective_node_id);

        assert_ok!(Network::collective_remove_subnet_node(
            RuntimeOrigin::from(pallet_collective::RawOrigin::Members(4, 5)),
            collective_subnet_id,
            collective_node_id,
        ));
        assert!(!SubnetNodesData::<Test>::contains_key(
            collective_subnet_id,
            collective_node_id
        ));
        assert!(PendingActiveNodeRemovals::<Test>::get(collective_subnet_id).is_empty());
        assert!(PendingRegisteredNodeRemovals::<Test>::get(collective_subnet_id).is_empty());

        // Whole-subnet deletion drops every outstanding active and registered marker together.
        let whole_name = b"pending-whole-subnet-removal".to_vec();
        build_activated_subnet(
            whole_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let whole_subnet_id = SubnetName::<Test>::get(whole_name).unwrap();
        insert_subnet_node(
            499,
            whole_subnet_id,
            49_001,
            49_002,
            49_003,
            SubnetNodeClass::Registered,
            0,
        );
        let registered_node_id = crate::TotalSubnetNodeUids::<Test>::get(whole_subnet_id);
        mark_active_pending(whole_subnet_id, 1);
        mark_registered_pending(whole_subnet_id, registered_node_id);

        assert_ok!(Network::collective_remove_subnet(
            RuntimeOrigin::from(pallet_collective::RawOrigin::Members(4, 5)),
            whole_subnet_id,
        ));
        assert!(PendingActiveNodeRemovals::<Test>::get(whole_subnet_id).is_empty());
        assert!(PendingRegisteredNodeRemovals::<Test>::get(whole_subnet_id).is_empty());
    });
}

#[test]
fn pending_cleanup_obeys_the_exact_meter_boundary_and_subnet_scope() {
    new_test_ext().execute_with(|| {
        let subnet_id = 51;
        let other_subnet_id = 52;
        let subnet_node_id = 1;
        let validator_id = 501;
        insert_subnet_node(
            validator_id,
            subnet_id,
            50_001,
            50_002,
            50_003,
            SubnetNodeClass::Validator,
            0,
        );
        mark_active_pending(subnet_id, subnet_node_id);

        let db_weight: frame_support::weights::RuntimeDbWeight =
            <Test as frame_system::Config>::DbWeight::get();
        let scan_weight =
            <() as WeightInfo>::pending_active_removal_scan(NetworkMaxSubnetNodesUpperBound::get());
        let selector_weight = <() as WeightInfo>::subnet_node_validator_id_selector();
        let validator_nodes = Network::validator_owned_nodes_weight_param(validator_id);
        let election_nodes = crate::TotalSubnetElectableNodes::<Test>::get(subnet_id);
        let marker_clear_weight = Network::pending_node_removal_marker_clear_weight();
        let exact_active_cleanup_weight = scan_weight
            .saturating_add(db_weight.reads(1))
            .saturating_add(selector_weight)
            .saturating_add(db_weight.reads(1))
            .saturating_add(Network::active_subnet_node_removal_weight(
                validator_nodes,
                election_nodes,
            ))
            .saturating_add(marker_clear_weight);

        let one_ref_time_short = Weight::from_parts(
            exact_active_cleanup_weight.ref_time().saturating_sub(1),
            exact_active_cleanup_weight.proof_size(),
        );
        let mut insufficient_meter = WeightMeter::with_limit(one_ref_time_short);
        Network::cleanup_pending_node_removals(&mut insufficient_meter, subnet_id);
        assert!(SubnetNodesData::<Test>::contains_key(
            subnet_id,
            subnet_node_id
        ));
        assert!(PendingActiveNodeRemovals::<Test>::get(subnet_id).contains(&subnet_node_id));

        // Work admitted for a different subnet never drains this subnet's durable marker.
        Network::cleanup_pending_node_removals(&mut WeightMeter::new(), other_subnet_id);
        assert!(PendingActiveNodeRemovals::<Test>::get(subnet_id).contains(&subnet_node_id));

        let mut exact_meter = WeightMeter::with_limit(exact_active_cleanup_weight);
        Network::cleanup_pending_node_removals(&mut exact_meter, subnet_id);
        assert!(!SubnetNodesData::<Test>::contains_key(
            subnet_id,
            subnet_node_id
        ));
        assert!(PendingActiveNodeRemovals::<Test>::get(subnet_id).is_empty());
        assert_eq!(exact_meter.consumed(), exact_active_cleanup_weight);
    });
}

#[test]
fn assigned_slot_cleanup_retries_on_the_next_assigned_slot_without_a_proposal() {
    new_test_ext().execute_with(|| {
        let subnet_name = b"pending-assigned-slot-retry".to_vec();
        build_activated_subnet(
            subnet_name.clone(),
            0,
            4,
            10_000_000_000_000_000_000_000,
            MinSubnetMinStake::<Test>::get(),
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let subnet_node_id = 1;
        mark_active_pending(subnet_id, subnet_node_id);

        let first_epoch = Network::get_current_epoch_as_u32();
        set_block_to_subnet_slot_epoch(first_epoch, subnet_id);
        let first_assigned_block = System::block_number();

        // Run the assigned step with less than the minimum active-pending scan. Settlement still
        // gets first priority, while the durable cleanup marker must survive for a later retry.
        let pending_scan =
            <() as WeightInfo>::pending_active_removal_scan(NetworkMaxSubnetNodesUpperBound::get());
        let one_ref_time_short = Weight::from_parts(
            pending_scan.ref_time().saturating_sub(1),
            pending_scan.proof_size(),
        );
        let mut insufficient_meter = WeightMeter::with_limit(one_ref_time_short);
        let first_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        Network::emission_step(
            &mut insufficient_meter,
            first_assigned_block,
            first_epoch,
            first_subnet_epoch,
            subnet_id,
        );

        assert!(SubnetNodesData::<Test>::contains_key(
            subnet_id,
            subnet_node_id
        ));
        assert!(PendingActiveNodeRemovals::<Test>::get(subnet_id).contains(&subnet_node_id));

        // Restoring weight on a block that is not this subnet's slot cannot drain its marker.
        let unrelated_block = first_assigned_block.saturating_add(1);
        assert_ne!(
            crate::SlotAssignment::<Test>::get(unrelated_block % EpochLength::get()),
            Some(subnet_id)
        );
        System::set_block_number(unrelated_block);
        Network::on_initialize(unrelated_block);
        assert!(PendingActiveNodeRemovals::<Test>::get(subnet_id).contains(&subnet_node_id));

        // There is deliberately no successful proposal for the intervening round. Cleanup is an
        // independent operational step and therefore succeeds at this subnet's next slot anyway.
        let next_epoch = first_epoch.saturating_add(1);
        set_block_to_subnet_slot_epoch(next_epoch, subnet_id);
        let next_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        assert!(next_subnet_epoch
            .checked_sub(1)
            .is_some_and(|previous_epoch| {
                !SubnetConsensusSubmission::<Test>::contains_key(subnet_id, previous_epoch)
            }));

        Network::on_initialize(System::block_number());

        assert_active_pending_cleaned(subnet_id, subnet_node_id);
    });
}

#[test]
fn missing_proposal_threshold_crossing_persists_pending_marker_and_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let subnet_id = 53;
        let subnet_node_id = 1;
        insert_subnet_node(
            530,
            subnet_id,
            53_001,
            53_002,
            53_003,
            SubnetNodeClass::Validator,
            0,
        );

        let round_epoch = 7;
        insert_elected_subnet_node(subnet_id, round_epoch, subnet_node_id);
        let minimum_reputation = test_percent(9, 10);
        SubnetNodeReputation::<Test>::insert(subnet_id, subnet_node_id, minimum_reputation);
        SubnetElectedValidator::<Test>::mutate(subnet_id, round_epoch, |maybe_round| {
            let round = maybe_round
                .as_mut()
                .expect("the missing-proposal round was seeded");
            round.policy.min_subnet_node_reputation = minimum_reputation;
            round.policy.reputation_factors.validator_absent_decrease = test_percent(1, 10);
        });
        System::reset_events();

        let (submission, _) =
            Network::precheck_subnet_consensus_submission(subnet_id, round_epoch, round_epoch + 1);

        assert!(submission.is_none());
        assert!(
            SubnetNodeReputation::<Test>::get(subnet_id, subnet_node_id).unwrap()
                < minimum_reputation
        );
        assert!(SubnetNodesData::<Test>::contains_key(
            subnet_id,
            subnet_node_id
        ));
        assert!(PendingActiveNodeRemovals::<Test>::get(subnet_id).contains(&subnet_node_id));
        assert!(network_events().iter().any(|event| {
            matches!(
                event,
                Event::SubnetNodesPendingRemoval {
                    subnet_id: event_subnet_id,
                    active_subnet_node_ids,
                    registered_subnet_node_ids,
                } if *event_subnet_id == subnet_id
                    && active_subnet_node_ids.as_slice() == [subnet_node_id]
                    && registered_subnet_node_ids.is_empty()
            )
        }));
    });
}

#[test]
fn every_active_node_mutation_can_only_authenticate_then_cleanup() {
    new_test_ext().execute_with(|| {
        let wrong_owner = account(999_999);

        let (node_id, owner) = seed_active_pending(61);
        assert!(Network::update_node_hotkey(
            RuntimeOrigin::signed(wrong_owner.clone()),
            61,
            node_id,
            Some(account(1_000_001)),
        )
        .is_err());
        assert!(SubnetNodesData::<Test>::contains_key(61, node_id));
        assert_ok!(Network::update_node_hotkey(
            RuntimeOrigin::signed(owner),
            61,
            node_id,
            Some(account(1_000_001)),
        ));
        assert_active_pending_cleaned(61, node_id);

        let (node_id, owner) = seed_active_pending(62);
        assert!(Network::update_node_peer_info(
            RuntimeOrigin::signed(wrong_owner.clone()),
            62,
            node_id,
            None,
        )
        .is_err());
        assert!(SubnetNodesData::<Test>::contains_key(62, node_id));
        assert_ok!(Network::update_node_peer_info(
            RuntimeOrigin::signed(owner),
            62,
            node_id,
            None,
        ));
        assert_active_pending_cleaned(62, node_id);

        let (node_id, owner) = seed_active_pending(63);
        assert!(Network::update_node_bootnode_peer_info(
            RuntimeOrigin::signed(wrong_owner.clone()),
            63,
            node_id,
            None,
        )
        .is_err());
        assert!(SubnetNodesData::<Test>::contains_key(63, node_id));
        assert_ok!(Network::update_node_bootnode_peer_info(
            RuntimeOrigin::signed(owner),
            63,
            node_id,
            None,
        ));
        assert_active_pending_cleaned(63, node_id);

        let (node_id, owner) = seed_active_pending(64);
        assert!(Network::update_node_client_peer_info(
            RuntimeOrigin::signed(wrong_owner.clone()),
            64,
            node_id,
            None,
        )
        .is_err());
        assert!(SubnetNodesData::<Test>::contains_key(64, node_id));
        assert_ok!(Network::update_node_client_peer_info(
            RuntimeOrigin::signed(owner),
            64,
            node_id,
            None,
        ));
        assert_active_pending_cleaned(64, node_id);

        let (node_id, owner) = seed_active_pending(65);
        assert!(Network::update_node_unique(
            RuntimeOrigin::signed(wrong_owner.clone()),
            65,
            node_id,
            Some(vec![9].try_into().unwrap()),
        )
        .is_err());
        assert!(SubnetNodesData::<Test>::contains_key(65, node_id));
        assert_ok!(Network::update_node_unique(
            RuntimeOrigin::signed(owner),
            65,
            node_id,
            Some(vec![9].try_into().unwrap()),
        ));
        assert_active_pending_cleaned(65, node_id);

        let (node_id, owner) = seed_active_pending(66);
        assert!(Network::update_node_non_unique(
            RuntimeOrigin::signed(wrong_owner.clone()),
            66,
            node_id,
            Some(vec![8].try_into().unwrap()),
        )
        .is_err());
        assert!(SubnetNodesData::<Test>::contains_key(66, node_id));
        assert_ok!(Network::update_node_non_unique(
            RuntimeOrigin::signed(owner),
            66,
            node_id,
            Some(vec![8].try_into().unwrap()),
        ));
        assert_active_pending_cleaned(66, node_id);

        let (node_id, owner) = seed_active_pending(67);
        let stake_before = crate::NodeSubnetStake::<Test>::get(node_id, 67);
        assert!(Network::add_node_stake(
            RuntimeOrigin::signed(wrong_owner.clone()),
            67,
            node_id,
            1,
        )
        .is_err());
        assert!(SubnetNodesData::<Test>::contains_key(67, node_id));
        assert_ok!(Network::add_node_stake(
            RuntimeOrigin::signed(owner),
            67,
            node_id,
            1,
        ));
        assert_active_pending_cleaned(67, node_id);
        assert_eq!(
            crate::NodeSubnetStake::<Test>::get(node_id, 67),
            stake_before
        );

        let (node_id, owner) = seed_active_pending(68);
        assert!(
            Network::remove_subnet_node(RuntimeOrigin::signed(wrong_owner), 68, node_id,).is_err()
        );
        assert!(SubnetNodesData::<Test>::contains_key(68, node_id));
        assert_ok!(Network::remove_subnet_node(
            RuntimeOrigin::signed(owner),
            68,
            node_id,
        ));
        assert_active_pending_cleaned(68, node_id);
    });
}

#[test]
fn registered_node_calls_require_the_owner_and_then_only_cleanup() {
    new_test_ext().execute_with(|| {
        let wrong_owner = account(999_998);

        let hotkey_subnet_id = 91;
        let hotkey_node_id = 1;
        let hotkey_owner = account(91_001);
        insert_subnet_node(
            910,
            hotkey_subnet_id,
            91_001,
            91_002,
            91_003,
            SubnetNodeClass::Registered,
            0,
        );
        mark_registered_pending(hotkey_subnet_id, hotkey_node_id);

        assert!(Network::update_node_hotkey(
            RuntimeOrigin::signed(wrong_owner.clone()),
            hotkey_subnet_id,
            hotkey_node_id,
            Some(account(91_004)),
        )
        .is_err());
        assert!(RegisteredSubnetNodesData::<Test>::contains_key(
            hotkey_subnet_id,
            hotkey_node_id
        ));
        assert_ok!(Network::update_node_hotkey(
            RuntimeOrigin::signed(hotkey_owner),
            hotkey_subnet_id,
            hotkey_node_id,
            Some(account(91_004)),
        ));
        assert!(!RegisteredSubnetNodesData::<Test>::contains_key(
            hotkey_subnet_id,
            hotkey_node_id
        ));
        assert!(PendingRegisteredNodeRemovals::<Test>::get(hotkey_subnet_id).is_empty());

        let removal_subnet_id = 92;
        let removal_node_id = 1;
        let removal_owner = account(92_001);
        insert_subnet_node(
            920,
            removal_subnet_id,
            92_001,
            92_002,
            92_003,
            SubnetNodeClass::Registered,
            0,
        );
        mark_registered_pending(removal_subnet_id, removal_node_id);

        assert!(Network::remove_subnet_node(
            RuntimeOrigin::signed(wrong_owner),
            removal_subnet_id,
            removal_node_id,
        )
        .is_err());
        assert!(RegisteredSubnetNodesData::<Test>::contains_key(
            removal_subnet_id,
            removal_node_id
        ));
        assert_ok!(Network::remove_subnet_node(
            RuntimeOrigin::signed(removal_owner),
            removal_subnet_id,
            removal_node_id,
        ));
        assert!(!RegisteredSubnetNodesData::<Test>::contains_key(
            removal_subnet_id,
            removal_node_id
        ));
        assert!(PendingRegisteredNodeRemovals::<Test>::get(removal_subnet_id).is_empty());
    });
}

#[test]
fn more_than_four_threshold_crossings_withhold_without_redistribution() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let subnet_id = 71;
        let owner = account(710_000);
        let percentage_factor = Network::percentage_factor_as_u128();
        let minimum_reputation = test_percent(9, 10);
        let total_nodes = 16u32;
        let crossing_nodes = 5u32;
        let node_reward_pool = 1_600u128;

        for subnet_node_id in 1..=total_nodes {
            let account_base = 710_000u32.saturating_add(subnet_node_id.saturating_mul(10));
            insert_subnet_node(
                subnet_node_id,
                subnet_id,
                account_base,
                account_base.saturating_add(1),
                account_base.saturating_add(2),
                SubnetNodeClass::Validator,
                0,
            );
            SubnetNodeReputation::<Test>::insert(
                subnet_id,
                subnet_node_id,
                if subnet_node_id <= crossing_nodes {
                    minimum_reputation
                } else {
                    percentage_factor
                },
            );
        }
        SubnetOwner::<Test>::insert(subnet_id, &owner);

        let reputation_factors = SubnetReputationFactors {
            non_attestor_decrease: test_percent(1, 10),
            ..Default::default()
        };
        let policy = ConsensusPolicySnapshot {
            min_attestation_percentage: test_percent(2, 3),
            super_majority_attestation_ratio: test_percent(2, 3),
            validator_identity_attestation_percentage: test_percent(2, 3),
            min_subnet_node_reputation: minimum_reputation,
            min_weight_decrease_reputation_threshold: 0,
            min_subnet_nodes: 3,
            base_validator_reward: 0,
            reputation_factors,
            ..Default::default()
        };
        let attests = ((crossing_nodes + 1)..=total_nodes)
            .map(|subnet_node_id| {
                (
                    subnet_node_id,
                    AttestEntry::<Test> {
                        block: 1,
                        attestor_progress: 0,
                        reward_factor: percentage_factor,
                        data: None,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let subnet_nodes = (1..=total_nodes)
            .map(|subnet_node_id| {
                ConsensusSubnetNode::from(&SubnetNodesData::<Test>::get(subnet_id, subnet_node_id))
            })
            .collect::<Vec<_>>();
        let scores = (1..=total_nodes)
            .map(|subnet_node_id| SubnetNodeConsensusData {
                subnet_node_id,
                score: 1,
            })
            .collect::<Vec<_>>();
        let submission = ConsensusSubmissionData::<Test> {
            policy,
            validator_subnet_node_id: total_nodes,
            validator_delegate_stake_balance: 0,
            validator_epoch_progress: 0,
            validator_reward_factor: percentage_factor,
            attestation_ratio: test_percent(11, 16),
            identity_attestation_ratio: test_percent(11, 16),
            identity_attestation_count: 11,
            eligible_validator_identity_count: total_nodes,
            weight_sum: total_nodes as u128,
            data_length: total_nodes,
            data: scores,
            attests,
            subnet_nodes,
            prioritize_queue_node_id: None,
            remove_queue_node_id: None,
            emergency: None,
        };
        let rewards = RewardsData {
            overall_subnet_reward: 2_900,
            subnet_owner_reward: 1_000,
            subnet_rewards: 1_900,
            delegate_stake_rewards: 300,
            subnet_node_rewards: node_reward_pool,
        };
        let owner_balance_before = Balances::free_balance(&owner);
        let delegate_balance_before =
            crate::TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        Network::distribute_rewards(
            &mut WeightMeter::new(),
            subnet_id,
            1,
            submission,
            rewards.clone(),
        );

        let pending = PendingActiveNodeRemovals::<Test>::get(subnet_id);
        assert_eq!(
            pending.iter().copied().collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
        assert_eq!(SubnetNodesData::<Test>::iter_prefix(subnet_id).count(), 16);
        for subnet_node_id in 1..=crossing_nodes {
            assert_eq!(
                crate::NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id),
                0
            );
        }
        for subnet_node_id in (crossing_nodes + 1)..=total_nodes {
            assert_eq!(
                crate::NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id),
                100,
                "each healthy node keeps its original 1/16 denominator share"
            );
        }
        assert_eq!(
            Balances::free_balance(&owner),
            owner_balance_before + rewards.subnet_owner_reward
        );
        assert_eq!(
            crate::TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            delegate_balance_before + rewards.delegate_stake_rewards
        );

        let reward_event = network_events()
            .into_iter()
            .rev()
            .find(|event| matches!(event, Event::SubnetRewards { .. }))
            .expect("reward settlement emits its summary");
        let Event::SubnetRewards { node_rewards, .. } = reward_event else {
            unreachable!()
        };
        assert_eq!(node_rewards.len(), 11);
        assert_eq!(
            node_rewards.iter().map(|(_, reward)| reward).sum::<u128>(),
            1_100
        );
    });
}

#[test]
fn pending_proposer_forfeits_every_node_related_allocation_without_redistribution() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let subnet_id = 72;
        let proposer_node_id: u32 = 1;
        let healthy_node_ids = [2, 3];
        let owner = account(720_000);
        let delegate_account = account(721_000);
        let percentage_factor = Network::percentage_factor_as_u128();
        let half = test_percent(1, 2);
        let minimum_reputation = test_percent(9, 10);
        let initial_node_stake = 1_000;
        let initial_validator_delegate_stake = 1_000;

        for subnet_node_id in [proposer_node_id, healthy_node_ids[0], healthy_node_ids[1]] {
            let account_base = 720_000u32.saturating_add(subnet_node_id.saturating_mul(10));
            insert_subnet_node(
                subnet_node_id,
                subnet_id,
                account_base,
                account_base.saturating_add(1),
                account_base.saturating_add(2),
                SubnetNodeClass::Validator,
                0,
            );
            crate::NodeSubnetStake::<Test>::insert(subnet_node_id, subnet_id, initial_node_stake);
            SubnetNodeReputation::<Test>::insert(subnet_id, subnet_node_id, percentage_factor);
        }
        crate::ValidatorsData::<Test>::mutate(proposer_node_id, |validator| {
            validator.delegate_reward_rate = half;
            validator.delegate_account = Some(crate::DelegateAccount {
                account_id: delegate_account.clone(),
                rate: half,
            });
        });
        crate::ValidatorDelegateStakeShares::<Test>::insert(proposer_node_id, 100);
        crate::ValidatorDelegateStakeBalance::<Test>::insert(
            proposer_node_id,
            initial_validator_delegate_stake,
        );
        crate::TotalValidatorDelegateStakeBalance::<Test>::put(initial_validator_delegate_stake);
        SubnetOwner::<Test>::insert(subnet_id, &owner);

        let owner_balance_before = Balances::free_balance(&owner);
        let subnet_delegate_stake_before =
            crate::TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);

        let reputation_factors = SubnetReputationFactors {
            included_increase: 0,
            below_min_weight_decrease: test_percent(1, 5),
            ..Default::default()
        };
        let policy = ConsensusPolicySnapshot {
            min_attestation_percentage: test_percent(2, 3),
            super_majority_attestation_ratio: test_percent(2, 3),
            validator_identity_attestation_percentage: test_percent(2, 3),
            min_subnet_node_reputation: minimum_reputation,
            min_weight_decrease_reputation_threshold: test_percent(1, 5),
            min_subnet_nodes: 3,
            base_validator_reward: 100,
            reputation_factors,
            ..Default::default()
        };
        let scores = vec![
            SubnetNodeConsensusData {
                subnet_node_id: proposer_node_id,
                score: 1,
            },
            SubnetNodeConsensusData {
                subnet_node_id: healthy_node_ids[0],
                score: 4,
            },
            SubnetNodeConsensusData {
                subnet_node_id: healthy_node_ids[1],
                score: 5,
            },
        ];
        let attests = [proposer_node_id, healthy_node_ids[0], healthy_node_ids[1]]
            .into_iter()
            .map(|subnet_node_id| {
                (
                    subnet_node_id,
                    AttestEntry::<Test> {
                        block: 1,
                        attestor_progress: 0,
                        reward_factor: percentage_factor,
                        data: None,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let subnet_nodes = [proposer_node_id, healthy_node_ids[0], healthy_node_ids[1]]
            .into_iter()
            .map(|subnet_node_id| {
                ConsensusSubnetNode::from(&SubnetNodesData::<Test>::get(subnet_id, subnet_node_id))
            })
            .collect::<Vec<_>>();
        let submission = ConsensusSubmissionData::<Test> {
            policy,
            validator_subnet_node_id: proposer_node_id,
            validator_delegate_stake_balance: initial_validator_delegate_stake,
            validator_epoch_progress: 0,
            validator_reward_factor: percentage_factor,
            attestation_ratio: percentage_factor,
            identity_attestation_ratio: percentage_factor,
            identity_attestation_count: 3,
            eligible_validator_identity_count: 3,
            weight_sum: 10,
            data_length: 3,
            data: scores,
            attests,
            subnet_nodes,
            prioritize_queue_node_id: None,
            remove_queue_node_id: None,
            emergency: None,
        };
        let rewards = RewardsData {
            overall_subnet_reward: 1_600,
            subnet_owner_reward: 1_000,
            subnet_rewards: 600,
            delegate_stake_rewards: 300,
            subnet_node_rewards: 300,
        };

        Network::distribute_rewards(
            &mut WeightMeter::new(),
            subnet_id,
            1,
            submission,
            rewards.clone(),
        );

        let pending = PendingActiveNodeRemovals::<Test>::get(subnet_id);
        assert!(pending.contains(&proposer_node_id));
        assert_eq!(
            SubnetNodeReputation::<Test>::get(subnet_id, proposer_node_id),
            Some(test_percent(4, 5))
        );
        assert_eq!(
            crate::NodeSubnetStake::<Test>::get(proposer_node_id, subnet_id),
            initial_node_stake,
            "the triggering node share and base validator reward are both forfeited"
        );
        assert_eq!(
            crate::ValidatorDelegateStakeBalance::<Test>::get(proposer_node_id),
            initial_validator_delegate_stake
        );
        assert_eq!(
            crate::DelegateAccountStake::<Test>::get(&delegate_account),
            0
        );
        // Healthy nodes keep their historical 4/10 and 5/10 shares. The proposer's 1/10 share is
        // forfeited rather than redistributed between them.
        assert_eq!(
            crate::NodeSubnetStake::<Test>::get(healthy_node_ids[0], subnet_id),
            initial_node_stake + 120
        );
        assert_eq!(
            crate::NodeSubnetStake::<Test>::get(healthy_node_ids[1], subnet_id),
            initial_node_stake + 150
        );
        assert_eq!(
            Balances::free_balance(&owner),
            owner_balance_before + rewards.subnet_owner_reward
        );
        assert_eq!(
            crate::TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            subnet_delegate_stake_before + rewards.delegate_stake_rewards
        );

        let reward_event = network_events()
            .into_iter()
            .rev()
            .find(|event| matches!(event, Event::SubnetRewards { .. }))
            .expect("reward settlement emits its summary");
        let Event::SubnetRewards {
            node_rewards,
            node_delegate_stake_rewards,
            node_delegate_account_allocations,
            ..
        } = reward_event
        else {
            unreachable!()
        };
        assert_eq!(
            node_rewards,
            vec![(healthy_node_ids[0], 120), (healthy_node_ids[1], 150)]
        );
        assert!(node_delegate_stake_rewards.is_empty());
        assert!(node_delegate_account_allocations.is_empty());
    });
}
