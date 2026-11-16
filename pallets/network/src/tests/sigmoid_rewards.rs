use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{
    assert_ok,
    traits::{Currency, Hooks},
};
use sp_std::collections::btree_map::BTreeMap;

const TOKEN: u128 = 1_000_000_000_000_000_000;

#[test]
fn sigmoid_is_finite_and_monotone_for_valid_steepness_boundaries() {
    for k in [1u64, 10, 10_000, u64::MAX] {
        for midpoint in [0.0, 0.5, 1.0] {
            let mut previous = 1.0;
            for progress in [0.0, 0.01, 0.25, 0.5, 0.75, 0.99, 1.0] {
                let factor = Network::sigmoid_decreasing(progress, midpoint, k as f64, 0.0, 1.0);
                assert!(
                    factor.is_finite(),
                    "k={k}, midpoint={midpoint}, progress={progress}"
                );
                assert!((0.0..=1.0).contains(&factor));
                assert!(factor <= previous);
                previous = factor;
            }
            assert_eq!(
                Network::sigmoid_decreasing(midpoint, midpoint, k as f64, 0.0, 1.0),
                0.5
            );
        }
    }
    for k in [10_000u64, u64::MAX] {
        assert_eq!(
            Network::sigmoid_decreasing(0.0, 0.5, k as f64, 0.0, 1.0),
            1.0
        );
        assert_eq!(
            Network::sigmoid_decreasing(1.0, 0.5, k as f64, 0.0, 1.0),
            0.0
        );
    }
}

#[test]
fn sigmoid_preserves_the_finite_curve_within_floating_point_precision() {
    for k in [1u64, 10, 100] {
        for midpoint in [0.0, 0.5, 1.0] {
            for progress in [0.0f64, 0.01, 0.25, 0.5, 0.75, 0.99, 1.0] {
                let exponential = libm::exp(k as f64 * (progress - midpoint).abs());
                let reference = if progress > midpoint {
                    1.0 / (1.0 + exponential)
                } else {
                    exponential / (1.0 + exponential)
                };
                let factor = Network::sigmoid_decreasing(progress, midpoint, k as f64, 0.0, 1.0);
                assert!((factor - reference).abs() <= 2.0 * f64::EPSILON);
            }
        }
    }
}

#[test]
fn valid_large_reward_k_survives_snapshot_proposal_and_next_election() {
    new_test_ext().execute_with(|| {
        increase_epochs(50);
        let name = b"large-reward-steepness".to_vec();
        build_activated_subnet(name.clone(), 0, 4, 10_000 * TOKEN, 100 * TOKEN);
        let subnet = SubnetName::<Test>::get(name).unwrap();
        let delegator = account(99);
        let _ = Balances::deposit_creating(&delegator, 1_000 * TOKEN);
        for node in 1..=4 {
            let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
            assert_ok!(Network::add_validator_delegate_stake(
                RuntimeOrigin::signed(delegator.clone()),
                validator,
                100 * TOKEN,
                1,
            ));
        }
        assert_ok!(Network::set_validator_reward_k(
            RuntimeOrigin::from(pallet_collective::RawOrigin::Members(2, 3)),
            10_000,
        ));
        assert_ok!(Network::set_validator_reward_midpoint(
            RuntimeOrigin::from(pallet_collective::RawOrigin::Members(2, 3)),
            TOKEN / 2,
        ));
        let epoch = SubnetsData::<Test>::get(subnet)
            .unwrap()
            .consensus_eligible_from_subnet_epoch
            .unwrap();
        set_block_to_subnet_slot_epoch(epoch, subnet);
        Network::on_initialize(System::block_number());
        let round = SubnetElectedValidator::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(round.policy.validator_reward_k, 10_000);
        assert_eq!(round.policy.validator_reward_midpoint, TOKEN / 2);
        // The proposal must use the elected policy, even if governance changes the live value.
        assert_ok!(Network::set_validator_reward_k(
            RuntimeOrigin::from(pallet_collective::RawOrigin::Members(2, 3)),
            10,
        ));
        let proposer = round.validator_subnet_node_id;
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, proposer).unwrap();
        assert_ok!(Network::propose_attestation(
            RuntimeOrigin::signed(hotkey),
            subnet,
            (1..=4)
                .map(|subnet_node_id| SubnetNodeConsensusData {
                    subnet_node_id,
                    score: 100
                })
                .collect(),
            None,
            None,
            None,
            None,
        ));
        let submission = SubnetConsensusSubmission::<Test>::get(subnet, epoch).unwrap();
        assert_eq!(submission.validator_epoch_progress, 0);
        assert_eq!(submission.validator_reward_factor, TOKEN);
        for node in (1..=4).filter(|node| *node != proposer) {
            let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
            assert_ok!(Network::attest(
                RuntimeOrigin::signed(hotkey),
                subnet,
                node,
                None
            ));
        }
        FinalSubnetEmissionWeights::<Test>::insert(
            epoch + 1,
            DistributionData {
                subnets_emissions: 10 * TOKEN,
                subnet_weights: BTreeMap::from([(subnet, TOKEN)]),
            },
        );
        let stake_before = NodeSubnetStake::<Test>::get(proposer, subnet);
        set_block_to_subnet_slot_epoch(epoch + 1, subnet);
        let consumed = Network::on_initialize(System::block_number());
        assert!(consumed.all_lte(MaximumHooksWeight::get()));
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
            Some(true)
        );
        assert!(NodeSubnetStake::<Test>::get(proposer, subnet) > stake_before);
        assert!(SubnetElectedValidator::<Test>::contains_key(
            subnet,
            epoch + 1
        ));
    });
}
