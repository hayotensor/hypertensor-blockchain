use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{assert_ok, weights::WeightMeter};
use sp_std::collections::btree_map::BTreeMap;

const NODES: u32 = 4;
const TOKEN: u128 = 1_000_000_000_000_000_000;

fn fixture() -> (u32, u32) {
    increase_epochs(50);
    let name = b"reward-capacitor".to_vec();
    build_activated_subnet(
        name.clone(),
        0,
        NODES,
        10_000 * TOKEN,
        MinSubnetMinStake::<Test>::get(),
    );
    let subnet = SubnetName::<Test>::get(name).unwrap();
    seed_equal_validator_delegate_stake_for_subnet(subnet);
    BaseValidatorReward::<Test>::put(0);
    SubnetOwnerPercentage::<Test>::put(TOKEN / 5);
    SubnetDelegateStakeRewardsPercentage::<Test>::insert(subnet, TOKEN / 4);
    for node in 1..=NODES {
        let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
        ValidatorsData::<Test>::mutate(validator, |data| data.delegate_reward_rate = 0);
    }
    (subnet, Network::get_current_subnet_epoch_as_u32(subnet))
}

fn allocation(subnet: u32, emission_epoch: u32, budget: u128) {
    FinalSubnetEmissionWeights::<Test>::insert(
        emission_epoch,
        DistributionData {
            subnets_emissions: budget,
            subnet_weights: BTreeMap::from([(subnet, TOKEN)]),
        },
    );
}

fn elect(subnet: u32, epoch: u32, budget: u128) -> ElectedConsensusRound {
    set_block_to_subnet_slot_epoch(epoch, subnet);
    Network::elect_validator(subnet, epoch, System::block_number());
    let round = SubnetElectedValidator::<Test>::get(subnet, epoch).expect("round elected");
    let emission_epoch = ConsensusRoundSettlementEmissionEpoch::<Test>::get(subnet, epoch).unwrap();
    allocation(subnet, emission_epoch, budget);
    round
}

fn propose(subnet: u32, epoch: u32, budget: u128, score: u128, votes: usize) {
    let round = elect(subnet, epoch, budget);
    let proposer = round.validator_subnet_node_id;
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
    for node in round
        .eligible_subnet_node_ids
        .into_iter()
        .filter(|node| *node != proposer)
        .take(votes.saturating_sub(1))
    {
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
        assert_ok!(Network::attest(
            RuntimeOrigin::signed(hotkey),
            subnet,
            node,
            None,
        ));
    }
}

fn settle(subnet: u32, current_subnet_epoch: u32) {
    set_block_to_subnet_slot_epoch(current_subnet_epoch, subnet);
    Network::emission_settlement_step(
        &mut WeightMeter::new(),
        System::block_number(),
        Network::get_current_epoch_as_u32(),
        current_subnet_epoch,
        subnet,
    );
}

fn balances(subnet: u32) -> (u128, u128, u128) {
    (
        Balances::free_balance(SubnetOwner::<Test>::get(subnet).unwrap()),
        TotalSubnetStake::<Test>::get(subnet),
        TotalSubnetDelegateStakeBalance::<Test>::get(subnet),
    )
}

fn assert_paid(subnet: u32, before: (u128, u128, u128), budget: u128) {
    let after = balances(subnet);
    // The elected policy gives the owner 20%, delegates 25% of the remainder,
    // and the four equally scored nodes the remaining 60%.
    assert_eq!(after.0 - before.0, budget / 5);
    assert_eq!(after.1 - before.1, budget * 3 / 5);
    assert_eq!(after.2 - before.2, budget / 5);
}

#[test]
fn failed_consensus_stores_the_full_budget_and_success_releases_ten_percent() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        propose(subnet, epoch, 100 * TOKEN, 100, 1);
        settle(subnet, epoch + 1);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 100 * TOKEN);
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
            Some(true)
        );
        let after_failure = balances(subnet);
        settle(subnet, epoch + 1);
        assert_eq!(balances(subnet), after_failure);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 100 * TOKEN);

        for offset in 1..=2 {
            propose(subnet, epoch + offset, 100 * TOKEN, 100, NODES as usize);
            let before = balances(subnet);
            settle(subnet, epoch + offset + 1);
            assert_paid(subnet, before, 110 * TOKEN);
            assert_eq!(
                SubnetRewardCapacitor::<Test>::get(subnet),
                (100 - 10 * u128::from(offset)) * TOKEN
            );
            let after_success = balances(subnet);
            settle(subnet, epoch + offset + 1);
            assert_eq!(balances(subnet), after_success);
            assert_eq!(
                SubnetRewardCapacitor::<Test>::get(subnet),
                (100 - 10 * u128::from(offset)) * TOKEN
            );
        }
    });
}

#[test]
fn release_uses_remaining_reserve_and_the_elected_reward_split() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        SubnetRewardCapacitor::<Test>::insert(subnet, 3 * TOKEN);
        propose(subnet, epoch, 100 * TOKEN, 100, NODES as usize);
        // Live changes cannot rewrite the already elected round's distribution.
        SubnetOwnerPercentage::<Test>::put(0);
        SubnetDelegateStakeRewardsPercentage::<Test>::insert(subnet, 0);
        let before = balances(subnet);
        settle(subnet, epoch + 1);
        assert_paid(subnet, before, 103 * TOKEN);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 0);
    });
}

#[test]
fn delayed_missing_proposal_stores_its_original_allocation_once() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        elect(subnet, epoch, 100 * TOKEN);
        let original_emission_epoch =
            ConsensusRoundSettlementEmissionEpoch::<Test>::get(subnet, epoch).unwrap();
        set_block_to_subnet_slot_epoch(epoch + 5, subnet);
        let current_emission_epoch = Network::get_current_epoch_as_u32();
        assert_ne!(original_emission_epoch, current_emission_epoch);
        allocation(subnet, current_emission_epoch, 900 * TOKEN);

        settle(subnet, epoch + 5);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 100 * TOKEN);
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
            Some(true)
        );
        settle(subnet, epoch + 5);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 100 * TOKEN);
    });
}

#[test]
fn accepted_zero_score_and_sub_ten_unit_allocation_do_not_release_the_reserve() {
    for (budget, score) in [(100 * TOKEN, 0), (0, 100), (9, 100)] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = fixture();
            SubnetRewardCapacitor::<Test>::insert(subnet, 100 * TOKEN);
            propose(subnet, epoch, budget, score, NODES as usize);
            let before = balances(subnet);
            settle(subnet, epoch + 1);
            assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 100 * TOKEN);
            assert_eq!(
                ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
                Some(true)
            );
            if budget == 0 {
                assert_eq!(balances(subnet), before);
            }
        });
    }
}

#[test]
fn capacitor_overflow_preserves_pending_round_until_a_successful_retry() {
    for missing_proposal in [false, true] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = fixture();
            if missing_proposal {
                elect(subnet, epoch, 100 * TOKEN);
            } else {
                propose(subnet, epoch, 100 * TOKEN, 100, 1);
            }
            SubnetRewardCapacitor::<Test>::insert(subnet, u128::MAX);
            let before = balances(subnet);
            let reputation = SubnetReputation::<Test>::get(subnet);
            settle(subnet, epoch + 1);
            assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), u128::MAX);
            assert_eq!(balances(subnet), before);
            assert_eq!(SubnetReputation::<Test>::get(subnet), reputation);
            assert_eq!(
                ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
                Some(false)
            );
            assert_eq!(
                PendingConsensusRoundSettlementEpoch::<Test>::get(subnet),
                Some(epoch)
            );

            SubnetRewardCapacitor::<Test>::insert(subnet, 0);
            settle(subnet, epoch + 1);
            assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 100 * TOKEN);
            assert_eq!(
                ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
                Some(true)
            );
        });
    }
}

#[test]
fn accepted_combined_budget_overflow_preserves_pending_round_and_reserve() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        SubnetRewardCapacitor::<Test>::insert(subnet, TOKEN);
        propose(subnet, epoch, u128::MAX, 100, NODES as usize);
        let before = balances(subnet);
        settle(subnet, epoch + 1);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), TOKEN);
        assert_eq!(balances(subnet), before);
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
            Some(false)
        );
        assert_eq!(
            PendingConsensusRoundSettlementEpoch::<Test>::get(subnet),
            Some(epoch)
        );
    });
}

#[test]
fn missing_proposal_with_invalid_emission_weight_remains_pending() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture();
        elect(subnet, epoch, 100 * TOKEN);
        let emission_epoch =
            ConsensusRoundSettlementEmissionEpoch::<Test>::get(subnet, epoch).unwrap();
        FinalSubnetEmissionWeights::<Test>::mutate(emission_epoch, |allocation| {
            allocation.subnet_weights.insert(subnet, TOKEN + 1);
        });
        let before = balances(subnet);
        settle(subnet, epoch + 1);
        assert!(!SubnetRewardCapacitor::<Test>::contains_key(subnet));
        assert_eq!(balances(subnet), before);
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
            Some(false)
        );
        assert_eq!(
            PendingConsensusRoundSettlementEpoch::<Test>::get(subnet),
            Some(epoch)
        );
    });
}

#[test]
fn subnet_removal_clears_its_capacitor() {
    new_test_ext().execute_with(|| {
        let (subnet, _) = fixture();
        SubnetRewardCapacitor::<Test>::insert(subnet, 100 * TOKEN);
        let _ = Network::do_remove_subnet(subnet, SubnetRemovalReason::Owner);
        assert!(!SubnetsData::<Test>::contains_key(subnet));
        assert!(!SubnetRewardCapacitor::<Test>::contains_key(subnet));
    });
}
