use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{
    assert_ok,
    traits::{Currency, Hooks},
};

const TOKEN: u128 = 1_000_000_000_000_000_000;

#[test]
fn assigned_slot_snapshots_after_settlement_before_swaps_and_extrinsics() {
    new_test_ext().execute_with(|| {
        increase_epochs(50);
        let name = b"election-weight-order".to_vec();
        build_activated_subnet(name.clone(), 0, 4, 10_000 * TOKEN, 100 * TOKEN);
        let subnet = SubnetName::<Test>::get(name).unwrap();
        for node in 1..=4 {
            let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
            let depositor = account(95_000 + node);
            let _ = Balances::deposit_creating(&depositor, 100 * TOKEN + 500);
            assert_ok!(Network::add_validator_delegate_stake(
                RuntimeOrigin::signed(depositor),
                validator,
                100 * TOKEN,
                1,
            ));
        }
        let epoch = SubnetsData::<Test>::get(subnet)
            .unwrap()
            .consensus_eligible_from_subnet_epoch
            .unwrap();
        set_block_to_subnet_slot_epoch(epoch, subnet);
        Network::on_initialize(System::block_number());
        let first = SubnetConsensusAttestorWeights::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(first.total_weight, 400 * TOKEN);
        let round = SubnetElectedValidator::<Test>::get(subnet, epoch).unwrap();
        let hotkey =
            Network::get_subnet_node_associated_hotkey(subnet, round.validator_subnet_node_id)
                .unwrap();
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
        for node in (1..=4).filter(|node| *node != round.validator_subnet_node_id) {
            let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
            assert_ok!(Network::attest(
                RuntimeOrigin::signed(hotkey),
                subnet,
                node,
                None
            ));
        }
        let emission_epoch =
            ConsensusRoundSettlementEmissionEpoch::<Test>::get(subnet, epoch).unwrap();
        FinalSubnetEmissionWeights::<Test>::insert(
            emission_epoch,
            DistributionData {
                subnets_emissions: 100 * TOKEN,
                subnet_weights: BTreeMap::from([(subnet, TOKEN)]),
            },
        );

        // A real queued swap matures exactly at the next assigned slot.
        DelegateStakeCooldownEpochs::<Test>::put(0);
        let swapper = account(96_000);
        let _ = Balances::deposit_creating(&swapper, 10 * TOKEN + 500);
        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(swapper.clone()),
            subnet,
            10 * TOKEN,
            1
        ));
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(&swapper, subnet);
        let destination = SubnetNodeValidatorId::<Test>::get(subnet, 1).unwrap();
        let queue_id = NextSwapQueueId::<Test>::get();
        assert_ok!(Network::swap_from_subnet_to_validator(
            RuntimeOrigin::signed(swapper),
            subnet,
            destination,
            shares,
            1,
            1,
            u32::MAX,
        ));
        let queued = SwapCallQueue::<Test>::get(queue_id).unwrap();
        assert_eq!(queued.execute_after_blocks, EpochLength::get());
        let swapped_balance = queued.call.get_queue_balance();

        set_block_to_subnet_slot_epoch(epoch + 1, subnet);
        Network::on_initialize(System::block_number());
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
            Some(true)
        );
        assert!(SwapCallQueue::<Test>::get(queue_id).is_none());
        let next = SubnetConsensusAttestorWeights::<Test>::get(subnet, epoch + 1).unwrap();
        for node in 1..=4 {
            let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
            assert!(
                next.weights[&node] > first.weights[&node],
                "prior-round pool rewards must be included"
            );
            let post_election_swap = if node == 1 { swapped_balance } else { 0 };
            assert_eq!(
                ValidatorDelegateStakeBalance::<Test>::get(validator),
                next.weights[&node] + post_election_swap
            );
        }
        let depositor = account(97_000);
        let _ = Balances::deposit_creating(&depositor, TOKEN + 500);
        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(depositor),
            destination,
            TOKEN,
            1
        ));
        // A duplicate election invocation must not refresh voting power after that deposit.
        let elected = SubnetElectedValidator::<Test>::get(subnet, epoch + 1);
        Network::elect_validator(subnet, epoch + 1, System::block_number());
        assert_eq!(
            SubnetElectedValidator::<Test>::get(subnet, epoch + 1),
            elected
        );
        assert_eq!(
            SubnetConsensusAttestorWeights::<Test>::get(subnet, epoch + 1),
            Some(next)
        );
        assert_eq!(
            SubnetConsensusAttestorWeights::<Test>::get(subnet, epoch),
            Some(first)
        );
    });
}

#[test]
fn election_without_eligible_nodes_publishes_no_snapshot_or_liabilities() {
    new_test_ext().execute_with(|| {
        Network::elect_validator(1, 0, System::block_number());
        assert!(SubnetElectedValidator::<Test>::get(1, 0).is_none());
        assert!(SubnetConsensusAttestorWeights::<Test>::get(1, 0).is_none());
        assert!(ConsensusRoundSettlementStatus::<Test>::get(1, 0).is_none());
        assert!(PendingConsensusRoundSettlementEpoch::<Test>::get(1).is_none());
        assert_eq!(
            NodeStakePendingSlashLiabilityCount::<Test>::iter().count(),
            0
        );
    });
}

fn snapshot_balances(balances: &[u128], power: u128) -> ConsensusAttestorWeightSnapshot {
    let subnet = 1;
    let mut identities = BTreeMap::new();
    for (index, balance) in balances.iter().enumerate() {
        let node = index as u32 + 1;
        identities.insert(node, node);
        ValidatorDelegateStakeBalance::<Test>::insert(node, balance);
        ValidatorNodeDelegateStakeWeights::<Test>::insert(
            node,
            BTreeMap::from([((subnet, node), TOKEN)]),
        );
    }
    let mut policy = Network::consensus_policy_snapshot(subnet, 0);
    policy.consensus_validator_stake_weight_power = power;
    Network::snapshot_consensus_attestor_weights(subnet, &policy, &identities)
}

#[test]
fn linear_voting_weights_preserve_boundaries_and_rescale_oversized_totals() {
    new_test_ext().execute_with(|| {
        for balances in [vec![u128::MAX, 0], vec![u128::MAX - 1, 1, 0]] {
            let snapshot = snapshot_balances(&balances, TOKEN);
            assert_eq!(
                snapshot.weights.values().copied().collect::<Vec<_>>(),
                balances
            );
            assert_eq!(snapshot.total_weight, u128::MAX);
        }

        let equal = snapshot_balances(&[u128::MAX, u128::MAX, 0], TOKEN);
        assert_eq!(
            equal.weights,
            BTreeMap::from([(1, u128::MAX / 2), (2, u128::MAX / 2), (3, 0)])
        );
        assert_eq!(equal.total_weight, u128::MAX - 1);
        assert_eq!(
            Network::percent_div(equal.weights[&1], equal.total_weight),
            TOKEN / 2
        );

        let unequal = snapshot_balances(
            &[u128::MAX - 1, (u128::MAX - 1) / 2, (u128::MAX - 1) / 2, 0],
            TOKEN,
        );
        assert_eq!(
            unequal.weights,
            BTreeMap::from([
                (1, u128::MAX / 2),
                (2, u128::MAX / 4),
                (3, u128::MAX / 4),
                (4, 0),
            ])
        );
        assert_eq!(unequal.total_weight, u128::MAX - 2);
        assert_eq!(
            unequal.weights.values().copied().sum::<u128>(),
            unequal.total_weight
        );
    });
}

#[test]
fn diminishing_voting_power_handles_raw_totals_larger_than_u128() {
    new_test_ext().execute_with(|| {
        for power in [0, TOKEN / 2] {
            let ordinary = snapshot_balances(&[2, 1, 1, 0], power);
            let oversized =
                snapshot_balances(&[1u128 << 127, 1u128 << 126, 1u128 << 126, 0], power);
            assert_eq!(oversized, ordinary);
            assert_eq!(oversized.weights[&4], 0);
            assert_eq!(
                oversized.weights.values().copied().sum::<u128>(),
                oversized.total_weight
            );
        }
    });
}

#[test]
fn oversized_voting_total_still_publishes_a_complete_election() {
    new_test_ext().execute_with(|| {
        increase_epochs(50);
        let name = b"wide-election-weights".to_vec();
        build_activated_subnet(name.clone(), 0, 4, 10_000 * TOKEN, 100 * TOKEN);
        let subnet = SubnetName::<Test>::get(name).unwrap();
        for node in 1..=4 {
            let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
            ValidatorDelegateStakeBalance::<Test>::insert(validator, u128::MAX);
        }
        let epoch = SubnetsData::<Test>::get(subnet)
            .unwrap()
            .consensus_eligible_from_subnet_epoch
            .unwrap();
        set_block_to_subnet_slot_epoch(epoch, subnet);
        Network::elect_validator(subnet, epoch, System::block_number());

        let round = SubnetElectedValidator::<Test>::get(subnet, epoch).unwrap();
        let weights = SubnetConsensusAttestorWeights::<Test>::get(subnet, epoch).unwrap();
        assert_ok!(Network::validate_consensus_attestor_weight_snapshot(
            &round, &weights
        ));
        assert_eq!(weights.weights.len(), 4);
        assert!(weights
            .weights
            .values()
            .all(|weight| *weight == u128::MAX / 4));
        assert_eq!(weights.total_weight, u128::MAX - 3);
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
