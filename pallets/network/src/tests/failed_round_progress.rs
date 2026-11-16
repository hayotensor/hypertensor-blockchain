use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{
    assert_noop, assert_ok,
    traits::{Currency, Hooks},
};
use sp_std::collections::btree_map::BTreeMap;

const TOKEN: u128 = 1_000_000_000_000_000_000;
const NODES: u32 = 4;
const ALLOCATION: u128 = 10 * TOKEN;

#[derive(Clone, Copy, Debug)]
enum Outcome {
    MissingProposal,
    StakeQuorumFailure,
    IdentityQuorumFailure,
    ZeroVotingWeight,
    ZeroScores,
    RejectedScoreOverflow,
    ZeroAllocation,
    AllowedBaseRewardOverflow,
}

fn run_slot(subnet: u32, epoch: u32) {
    set_block_to_subnet_slot_epoch(epoch, subnet);
    let weight = Network::on_initialize(System::block_number());
    assert!(weight.all_lte(MaximumHooksWeight::get()));
}

fn propose(subnet: u32, proposer: u32, score: u128) {
    let hotkey = Network::get_subnet_node_associated_hotkey(subnet, proposer).unwrap();
    assert_ok!(Network::propose_attestation(
        RuntimeOrigin::signed(hotkey),
        subnet,
        (1..=NODES)
            .map(|subnet_node_id| SubnetNodeConsensusData {
                subnet_node_id,
                score,
            })
            .collect(),
        None,
        None,
        None,
        None,
    ));
}

#[test]
fn valid_failed_rounds_close_once_and_the_next_proposer_can_submit() {
    for outcome in [
        Outcome::MissingProposal,
        Outcome::StakeQuorumFailure,
        Outcome::IdentityQuorumFailure,
        Outcome::ZeroVotingWeight,
        Outcome::ZeroScores,
        Outcome::RejectedScoreOverflow,
        Outcome::ZeroAllocation,
        Outcome::AllowedBaseRewardOverflow,
    ] {
        new_test_ext().execute_with(|| {
            increase_epochs(50);
            let name = b"failed-round-progress".to_vec();
            build_activated_subnet(name.clone(), 0, NODES, 10_000 * TOKEN, 100 * TOKEN);
            let subnet = SubnetName::<Test>::get(name).unwrap();
            let epoch = SubnetsData::<Test>::get(subnet).unwrap()
                .consensus_eligible_from_subnet_epoch.unwrap();
            if matches!(outcome, Outcome::AllowedBaseRewardOverflow) {
                // Governance permits these values. This exercises a real checked-credit failure,
                // without corrupting a stake counter or the immutable election snapshot.
                assert_ok!(Network::set_base_validator_reward(
                    RuntimeOrigin::from(pallet_collective::RawOrigin::Members(4, 5)),
                    u128::MAX,
                ));
                assert_ok!(Network::set_validator_reward_k(
                    RuntimeOrigin::from(pallet_collective::RawOrigin::Members(4, 5)),
                    10_000,
                ));
                assert_ok!(Network::set_validator_reward_midpoint(
                    RuntimeOrigin::from(pallet_collective::RawOrigin::Members(4, 5)),
                    TOKEN / 2,
                ));
            }

            // The election draw is independent of voting stake. Its stable round-domain seed
            // lets this fixture fund the intended supporter/non-supporter before election.
            set_block_to_subnet_slot_epoch(epoch, subnet);
            let candidates = Network::resolve_consensus_validator_ids(subnet);
            let index = Network::get_bounded_random_index((subnet, epoch), NODES).unwrap();
            let expected_proposer = candidates[index as usize];
            let heavy_node = match outcome {
                Outcome::StakeQuorumFailure => (1..=NODES).find(|id| *id != expected_proposer).unwrap(),
                _ => expected_proposer,
            };
            let delegator = account(99);
            let _ = Balances::deposit_creating(&delegator, 20_000 * TOKEN);
            if !matches!(outcome, Outcome::ZeroVotingWeight) {
                for node in 1..=NODES {
                    let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
                    let unequal = matches!(outcome, Outcome::StakeQuorumFailure | Outcome::IdentityQuorumFailure);
                    let amount = if unequal && node == heavy_node { 1_000 * TOKEN } else { 10 * TOKEN };
                    // Real deposits preserve pool shares, backing, and aggregate accounting.
                    assert_ok!(Network::add_validator_delegate_stake(
                        RuntimeOrigin::signed(delegator.clone()), validator, amount, 1,
                    ));
                }
            }

            run_slot(subnet, epoch);
            let round = SubnetElectedValidator::<Test>::get(subnet, epoch).unwrap();
            assert_eq!(round.validator_subnet_node_id, expected_proposer);
            let proposer = round.validator_subnet_node_id;
            let budget = if matches!(outcome, Outcome::ZeroAllocation) { 0 } else { ALLOCATION };
            FinalSubnetEmissionWeights::<Test>::insert(epoch + 1, DistributionData {
                subnets_emissions: budget,
                subnet_weights: if budget == 0 { BTreeMap::new() } else { BTreeMap::from([(subnet, TOKEN)]) },
            });
            assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 0);

            if matches!(outcome, Outcome::RejectedScoreOverflow) {
                let hotkey = Network::get_subnet_node_associated_hotkey(subnet, proposer).unwrap();
                assert_noop!(Network::propose_attestation(
                    RuntimeOrigin::signed(hotkey), subnet,
                    vec![
                        SubnetNodeConsensusData { subnet_node_id: 1, score: u128::MAX },
                        SubnetNodeConsensusData { subnet_node_id: 2, score: 1 },
                    ],
                    None, None, None, None,
                ), Error::<Test>::ScoreOverflow);
                assert!(!SubnetConsensusSubmission::<Test>::contains_key(subnet, epoch));
            } else if !matches!(outcome, Outcome::MissingProposal) {
                propose(subnet, proposer, if matches!(outcome, Outcome::ZeroScores) { 0 } else { 100 });
                for node in (1..=NODES).filter(|node| *node != proposer) {
                    if matches!(outcome, Outcome::IdentityQuorumFailure)
                        || matches!(outcome, Outcome::StakeQuorumFailure) && node == heavy_node {
                        continue;
                    }
                    let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
                    assert_ok!(Network::attest(RuntimeOrigin::signed(hotkey), subnet, node, None));
                }
                let (submission, _) = Network::precheck_subnet_consensus_submission(subnet, epoch, epoch + 1);
                let submission = submission.unwrap();
                let stake_passes = submission.attestation_ratio >= submission.policy.min_attestation_percentage;
                let identity_passes = submission.identity_attestation_count >= Network::min_consensus_identity_attestation_count(
                    submission.eligible_validator_identity_count,
                    submission.policy.validator_identity_attestation_percentage,
                );
                match outcome {
                    Outcome::StakeQuorumFailure => assert!(!stake_passes && identity_passes),
                    Outcome::IdentityQuorumFailure => assert!(stake_passes && !identity_passes),
                    Outcome::ZeroVotingWeight => {
                        assert_eq!(SubnetConsensusAttestorWeights::<Test>::get(subnet, epoch).unwrap().total_weight, 0);
                        assert!(!stake_passes && identity_passes);
                    }
                    Outcome::ZeroScores => {
                        assert!(stake_passes && identity_passes);
                        assert_eq!(submission.weight_sum, 0);
                    }
                    Outcome::ZeroAllocation | Outcome::AllowedBaseRewardOverflow => assert!(stake_passes && identity_passes),
                    _ => unreachable!(),
                }
            }

            let total_stake_before = TotalStake::<Test>::get();
            let subnet_stake_before = TotalSubnetStake::<Test>::get(subnet);
            let expected_score_credits = if matches!(outcome, Outcome::AllowedBaseRewardOverflow) {
                let submission = SubnetConsensusSubmission::<Test>::get(subnet, epoch).unwrap();
                assert_eq!(round.policy.base_validator_reward, u128::MAX);
                assert_eq!(submission.validator_reward_factor, TOKEN);
                assert!(submission.attests.values().all(|entry| entry.reward_factor == TOKEN));
                let (rewards, _) = Network::calculate_rewards_with_policy(ALLOCATION, TOKEN, &round.policy);
                let equal_score_reward = rewards.subnet_node_rewards / u128::from(NODES);
                (1..=NODES).map(|node| {
                    let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
                    let rate = ValidatorsData::<Test>::get(validator).delegate_reward_rate;
                    let credit = equal_score_reward - Network::percent_mul(equal_score_reward, rate);
                    assert!(credit > 0);
                    (node, NodeSubnetStake::<Test>::get(node, subnet), credit)
                }).collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            run_slot(subnet, epoch + 1);
            if matches!(outcome, Outcome::AllowedBaseRewardOverflow) {
                let score_total: u128 = expected_score_credits.iter().map(|(_, _, credit)| *credit).sum();
                for (node, before, credit) in expected_score_credits {
                    assert_eq!(NodeSubnetStake::<Test>::get(node, subnet), before + credit);
                }
                assert_eq!(TotalStake::<Test>::get(), total_stake_before + score_total);
                assert_eq!(TotalSubnetStake::<Test>::get(subnet), subnet_stake_before + score_total);
            }
            assert_eq!(ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch), Some(true), "{outcome:?}");
            assert_eq!(PendingConsensusRoundSettlementEpoch::<Test>::get(subnet), Some(epoch + 1), "{outcome:?}");
            let next = SubnetElectedValidator::<Test>::get(subnet, epoch + 1).unwrap();
            assert_eq!(ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch + 1), Some(false));
            assert_eq!(NodeStakePendingSlashLiabilityCount::<Test>::iter_prefix(subnet).collect::<Vec<_>>(),
                vec![(next.validator_subnet_node_id, 1)]);
            let expected_capacitor = if matches!(outcome, Outcome::ZeroScores | Outcome::ZeroAllocation | Outcome::AllowedBaseRewardOverflow) { 0 } else { ALLOCATION };
            assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), expected_capacitor, "{outcome:?}");
            assert!(!network_events().iter().any(|event| matches!(event,
                Event::ConsensusRoundCancelled { subnet_id, subnet_epoch } if *subnet_id == subnet && *subnet_epoch == epoch)));

            // Repeating the same slot must neither repay the closed round nor settle the new
            // open round. The newly elected validator can then submit through the real call.
            let stakes: Vec<_> = NodeSubnetStake::<Test>::iter().collect();
            run_slot(subnet, epoch + 1);
            assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), expected_capacitor);
            assert_eq!(NodeSubnetStake::<Test>::iter().collect::<Vec<_>>(), stakes);
            assert_eq!(ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch + 1), Some(false));
            propose(subnet, next.validator_subnet_node_id, 100);
            assert!(SubnetConsensusSubmission::<Test>::contains_key(subnet, epoch + 1));
        });
    }
}
