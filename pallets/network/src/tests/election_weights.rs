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
        assert_eq!(
            ValidatorDelegateStakePendingSlashLiabilityCount::<Test>::iter().count(),
            0
        );
    });
}
