use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{assert_ok, traits::Hooks};
use sp_std::collections::btree_map::BTreeMap;

const TOKEN: u128 = 1_000_000_000_000_000_000;
const NODES: u32 = 4;
const ALLOCATION: u128 = 10 * TOKEN;

fn fixture() -> (u32, u32, u32) {
    increase_epochs(50);
    let name = b"consensus-liveness".to_vec();
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
    BaseSlashPercentage::<Test>::put(TOKEN / 10);
    MaxSlashAmount::<Test>::put(TOKEN);
    let epoch = SubnetsData::<Test>::get(subnet)
        .unwrap()
        .consensus_eligible_from_subnet_epoch
        .unwrap();
    run_slot(subnet, epoch);
    let proposer = SubnetElectedValidator::<Test>::get(subnet, epoch)
        .unwrap()
        .validator_subnet_node_id;

    let emission_epoch = epoch.saturating_add(1);
    assert_eq!(emission_epoch, epoch + 1);
    FinalSubnetEmissionWeights::<Test>::insert(
        emission_epoch,
        DistributionData {
            subnets_emissions: ALLOCATION,
            subnet_weights: BTreeMap::from([(subnet, TOKEN)]),
        },
    );
    SubnetRewardCapacitor::<Test>::insert(subnet, TOKEN);
    (subnet, epoch, proposer)
}

fn submit(subnet: u32, proposer: u32) {
    let hotkey = Network::get_subnet_node_associated_hotkey(subnet, proposer).unwrap();
    assert_ok!(Network::propose_attestation(
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
    ));
    for node in (1..=NODES).filter(|node| *node != proposer) {
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
        assert_ok!(Network::attest(
            RuntimeOrigin::signed(hotkey),
            subnet,
            node,
            None,
        ));
    }
}

fn run_slot(subnet: u32, epoch: u32) {
    set_block_to_subnet_slot_epoch(epoch, subnet);
    let consumed = Network::on_initialize(System::block_number());
    assert!(consumed.all_lte(MaximumHooksWeight::get()));
}

#[derive(Debug, PartialEq, Eq)]
struct EconomicState {
    owner_balance: u128,
    issuance: u128,
    subnet_delegation: u128,
    total_stake: u128,
    total_delegation: u128,
    node_stakes: Vec<(u32, u32, u128)>,
    validator_pools: Vec<(u32, u128)>,
    subnet_reputation: u128,
    node_reputations: Vec<(u32, u128)>,
    probation: Vec<(u32, u32)>,
}

fn economic_state(subnet: u32) -> EconomicState {
    EconomicState {
        owner_balance: Balances::free_balance(SubnetOwner::<Test>::get(subnet).unwrap()),
        issuance: Balances::total_issuance(),
        subnet_delegation: TotalSubnetDelegateStakeBalance::<Test>::get(subnet),
        total_stake: TotalStake::<Test>::get(),
        total_delegation: TotalDelegateStake::<Test>::get(),
        node_stakes: NodeSubnetStake::<Test>::iter().collect(),
        validator_pools: ValidatorDelegateStakeBalance::<Test>::iter().collect(),
        subnet_reputation: SubnetReputation::<Test>::get(subnet),
        node_reputations: SubnetNodeReputation::<Test>::iter_prefix(subnet).collect(),
        probation: SubnetNodeProbationProgress::<Test>::iter_prefix(subnet).collect(),
    }
}

fn cancellation_count(subnet: u32, epoch: u32) -> usize {
    network_events()
        .iter()
        .filter(|event| {
            matches!(event, Event::ConsensusRoundCancelled { subnet_id, subnet_epoch }
                if *subnet_id == subnet && *subnet_epoch == epoch)
        })
        .count()
}

fn reward_count(subnet: u32) -> usize {
    network_events()
        .iter()
        .filter(
            |event| matches!(event, Event::SubnetRewards { subnet_id, .. } if *subnet_id == subnet),
        )
        .count()
}

fn assert_progressed(subnet: u32, old_epoch: u32) -> u32 {
    assert_progressed_at(subnet, old_epoch, old_epoch + 1)
}

fn assert_progressed_at(subnet: u32, old_epoch: u32, next_epoch: u32) -> u32 {
    assert_eq!(
        ConsensusRoundSettlementStatus::<Test>::get(subnet, old_epoch),
        Some(true)
    );

    assert_eq!(
        ConsensusRoundSettlementStatus::<Test>::get(subnet, next_epoch),
        Some(false)
    );
    let next = SubnetElectedValidator::<Test>::get(subnet, next_epoch)
        .expect("the same hook must elect the next round")
        .validator_subnet_node_id;

    next
}

#[test]
fn full_queued_removal_set_preserves_unmarked_nodes_and_consensus_progress() {
    for already_pending in [false, true] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch, _) = fixture();
            insert_subnet_node(
                930,
                subnet,
                93_001,
                93_002,
                93_003,
                SubnetNodeClass::Queued,
                0,
            );
            let queued_id = TotalSubnetNodeUids::<Test>::get(subnet);
            // Keep normal admission from changing the queue while testing the removal decision.
            SubnetNodeQueueEpochs::<Test>::insert(subnet, u32::MAX);
            run_subnet_consensus_step_v2(subnet, None, Some(queued_id));
            assert_eq!(
                SubnetConsensusSubmission::<Test>::get(subnet, epoch)
                    .unwrap()
                    .remove_queue_node_id,
                Some(queued_id)
            );

            // Corrupt the pending set after a valid proposal has been accepted. The queue target
            // either needs a new marker in the full set, or already has one from an earlier action.
            let bound = NetworkMaxRegisteredNodesUpperBound::get();
            let mut pending =
                (10_000..10_000 + bound).collect::<sp_std::collections::btree_set::BTreeSet<_>>();
            if already_pending {
                pending.remove(&10_000);
                pending.insert(queued_id);
            }
            let pending: frame_support::BoundedBTreeSet<u32, NetworkMaxRegisteredNodesUpperBound> =
                pending.try_into().unwrap();
            PendingQueuedNodeRemovals::<Test>::insert(subnet, pending);
            let rewards_before = reward_count(subnet);

            run_slot(subnet, epoch + 1);
            assert_progressed(subnet, epoch);
            assert_eq!(reward_count(subnet), rewards_before + 1);
            assert_eq!(
                SubnetNodeQueue::<Test>::get(subnet)
                    .iter()
                    .any(|node| node.id == queued_id),
                !already_pending
            );
            assert_eq!(
                network_events().iter().any(|event| matches!(
                    event,
                    Event::QueuedNodeRemoved { subnet_id, subnet_node_id }
                        if *subnet_id == subnet && *subnet_node_id == queued_id
                )),
                already_pending
            );
            if !already_pending {
                assert!(QueuedSubnetNodesData::<Test>::contains_key(
                    subnet, queued_id
                ));
                assert!(!PendingQueuedNodeRemovals::<Test>::get(subnet).contains(&queued_id));
            }
        });
    }
}

#[derive(Clone, Copy, Debug)]
enum FrozenFault {
    MissingElection,
    MissingWeights,
    WrongWeightTotal,
    DuplicateMember,
    UnknownMember,
}

#[test]
fn unusable_frozen_rounds_cancel_once_and_the_hook_elects_again() {
    for fault in [
        FrozenFault::MissingElection,
        FrozenFault::MissingWeights,
        FrozenFault::WrongWeightTotal,
        FrozenFault::DuplicateMember,
        FrozenFault::UnknownMember,
    ] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch, proposer) = fixture();
            submit(subnet, proposer);
            let before = economic_state(subnet);
            let rewards_before = reward_count(subnet);
            match fault {
                FrozenFault::MissingElection => {
                    SubnetElectedValidator::<Test>::remove(subnet, epoch)
                }
                FrozenFault::MissingWeights => {
                    SubnetConsensusAttestorWeights::<Test>::remove(subnet, epoch)
                }

                FrozenFault::WrongWeightTotal => {
                    SubnetConsensusAttestorWeights::<Test>::mutate(subnet, epoch, |snapshot| {
                        snapshot.as_mut().unwrap().total_weight += 1
                    })
                }
                FrozenFault::DuplicateMember => {
                    SubnetConsensusSubmission::<Test>::mutate(subnet, epoch, |submission| {
                        let submission = submission.as_mut().unwrap();
                        submission.validator_ids.push(submission.validator_ids[0]);
                    })
                }
                FrozenFault::UnknownMember => {
                    SubnetConsensusSubmission::<Test>::mutate(subnet, epoch, |submission| {
                        submission.as_mut().unwrap().validator_ids[0] = u32::MAX;
                    })
                }
            }

            run_slot(subnet, epoch + 1);
            assert_progressed(subnet, epoch);
            assert_eq!(economic_state(subnet), before, "{fault:?}");
            assert_eq!(reward_count(subnet), rewards_before);
            assert_eq!(
                SubnetRewardCapacitor::<Test>::get(subnet),
                TOKEN + ALLOCATION
            );
            assert_eq!(cancellation_count(subnet, epoch), 1);

            // Running the same slot again must preserve the newly elected, still-open round.
            let next_round = SubnetElectedValidator::<Test>::get(subnet, epoch + 1);
            run_slot(subnet, epoch + 1);
            assert_progressed(subnet, epoch);
            assert_eq!(
                SubnetElectedValidator::<Test>::get(subnet, epoch + 1),
                next_round
            );
            assert_eq!(economic_state(subnet), before);
            assert_eq!(
                SubnetRewardCapacitor::<Test>::get(subnet),
                TOKEN + ALLOCATION
            );
            assert_eq!(cancellation_count(subnet, epoch), 1);
            assert_eq!(cancellation_count(subnet, epoch + 1), 0);
            assert_eq!(reward_count(subnet), rewards_before);
        });
    }
}

#[test]
fn unfinished_round_does_not_block_election_or_mutate_the_next_round() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch, proposer) = fixture();
        submit(subnet, proposer);
        let before = economic_state(subnet);
        SubnetNodeElectionSlots::<Test>::insert(subnet, vec![proposer]);
        set_block_to_subnet_slot_epoch(epoch + 1, subnet);
        // Elect without first settling the previous round, including the very same node.
        Network::elect_validator(subnet, epoch + 1);
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
            Some(false)
        );
        let next = SubnetElectedValidator::<Test>::get(subnet, epoch + 1).unwrap();
        assert_eq!(next.validator_subnet_node_id, proposer);
        let weights = SubnetConsensusAttestorWeights::<Test>::get(subnet, epoch + 1);
        Network::finalize_consensus_round(subnet, epoch);
        assert_eq!(assert_progressed(subnet, epoch), proposer);
        assert_eq!(
            SubnetElectedValidator::<Test>::get(subnet, epoch + 1),
            Some(next)
        );
        assert_eq!(
            SubnetConsensusAttestorWeights::<Test>::get(subnet, epoch + 1),
            weights
        );
        assert_eq!(economic_state(subnet), before);
        frame_support::assert_storage_noop!(Network::finalize_consensus_round(subnet, epoch));
    });
}

#[test]
fn resume_cancellation_uses_the_next_subnet_epoch_allocation() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch, proposer) = fixture();
        submit(subnet, proposer);
        let before = economic_state(subnet);

        FinalSubnetEmissionWeights::<Test>::insert(
            epoch + 2,
            DistributionData {
                subnets_emissions: 999 * TOKEN,
                subnet_weights: BTreeMap::from([(subnet, TOKEN)]),
            },
        );
        assert_ok!(Network::do_pause());
        assert_ok!(Network::do_unpause());
        run_slot(subnet, epoch + 1);
        assert_progressed(subnet, epoch);
        assert_eq!(economic_state(subnet), before);
        assert_eq!(
            SubnetRewardCapacitor::<Test>::get(subnet),
            TOKEN + ALLOCATION
        );
        assert_eq!(cancellation_count(subnet, epoch), 1);
    });
}

#[test]
fn settled_round_does_not_repeat_its_economic_outcome() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch, _) = fixture();
        let before = economic_state(subnet);
        Network::finalize_consensus_round(subnet, epoch);
        run_slot(subnet, epoch + 1);
        assert_progressed(subnet, epoch);
        assert_eq!(economic_state(subnet), before);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), TOKEN);
        assert_eq!(cancellation_count(subnet, epoch), 0);
    });
}

#[test]
fn election_skips_only_the_candidate_with_a_missing_identity_mapping() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch, proposer) = fixture();
        submit(subnet, proposer);
        SubnetConsensusAttestorWeights::<Test>::remove(subnet, epoch);
        SubnetNodeValidatorId::<Test>::remove(subnet, 1);
        run_slot(subnet, epoch + 1);
        let next = assert_progressed(subnet, epoch);
        assert_ne!(next, 1);
        let round = SubnetElectedValidator::<Test>::get(subnet, epoch + 1).unwrap();
        assert_eq!(round.eligible_subnet_node_ids, vec![2, 3, 4]);
        assert_eq!(round.eligible_validator_identity_ids.len(), 3);
        assert!(!round.eligible_validator_identity_ids.contains_key(&1));
        let weights = SubnetConsensusAttestorWeights::<Test>::get(subnet, epoch + 1).unwrap();
        assert_eq!(
            weights.weights.keys().copied().collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
    });
}

#[test]
fn skipped_settlement_does_not_create_a_backlog_or_block_fresh_consensus() {
    for proposed in [false, true] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch, proposer) = fixture();
            if proposed {
                submit(subnet, proposer);
            }
            let before = economic_state(subnet);
            let later_epoch = epoch + 3;
            run_slot(subnet, later_epoch);
            assert_eq!(
                ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
                Some(false)
            );
            assert_eq!(
                ConsensusRoundSettlementStatus::<Test>::get(subnet, later_epoch),
                Some(false)
            );
            assert_eq!(economic_state(subnet), before);
            assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), TOKEN);
            assert_eq!(cancellation_count(subnet, epoch), 0);
            assert_eq!(reward_count(subnet), 0);
            let next_proposer = SubnetElectedValidator::<Test>::get(subnet, later_epoch)
                .unwrap()
                .validator_subnet_node_id;
            submit(subnet, next_proposer);
            run_slot(subnet, later_epoch + 1);
            assert_progressed(subnet, later_epoch);
            assert_eq!(
                ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
                Some(false)
            );
        });
    }
}

#[test]
fn unfinished_round_does_not_block_subnet_removal() {
    for owner_removal in [false, true] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch, _) = fixture();
            assert_eq!(
                ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
                Some(false)
            );
            if owner_removal {
                let owner = SubnetOwner::<Test>::get(subnet).unwrap();
                assert_ok!(Network::do_owner_deactivate_subnet(
                    RuntimeOrigin::signed(owner),
                    subnet
                ));
            } else {
                assert_ok!(Network::do_collective_remove_subnet(subnet));
            }
            assert!(!SubnetsData::<Test>::contains_key(subnet));
            assert!(!SubnetSlot::<Test>::contains_key(subnet));
        });
    }
}
