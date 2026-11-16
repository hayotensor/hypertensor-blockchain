use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{assert_ok, traits::Hooks};
use sp_std::collections::btree_map::BTreeMap;

const TOKEN: u128 = 1_000_000_000_000_000_000;
const NODES: u32 = 4;

fn run_slot(subnet: u32, epoch: u32) {
    set_block_to_subnet_slot_epoch(epoch, subnet);
    let consumed = Network::on_initialize(System::block_number());
    assert!(consumed.all_lte(MaximumHooksWeight::get()));
}

fn elected_round() -> (u32, u32, u32) {
    increase_epochs(50);
    let name = b"reward-failure-liveness".to_vec();
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
    let emission_epoch = ConsensusRoundSettlementEmissionEpoch::<Test>::get(subnet, epoch).unwrap();
    FinalSubnetEmissionWeights::<Test>::insert(
        emission_epoch,
        DistributionData {
            subnets_emissions: 10 * TOKEN,
            subnet_weights: BTreeMap::from([(subnet, TOKEN)]),
        },
    );
    (subnet, epoch, proposer)
}

fn assert_next_round(subnet: u32, old_epoch: u32) {
    assert_eq!(
        ConsensusRoundSettlementStatus::<Test>::get(subnet, old_epoch),
        Some(true)
    );
    assert!(!ConsensusRoundSettlementEmissionEpoch::<Test>::contains_key(subnet, old_epoch));
    assert_eq!(
        PendingConsensusRoundSettlementEpoch::<Test>::get(subnet),
        Some(old_epoch + 1)
    );
    assert_eq!(
        ConsensusRoundSettlementStatus::<Test>::get(subnet, old_epoch + 1),
        Some(false)
    );
    let next_proposer = SubnetElectedValidator::<Test>::get(subnet, old_epoch + 1)
        .expect("a failed credit must not prevent the next election")
        .validator_subnet_node_id;
    assert_eq!(
        PendingConsensusRoundSlashLiabilityNode::<Test>::get(subnet),
        Some(next_proposer)
    );
    assert_eq!(
        NodeStakePendingSlashLiabilityCount::<Test>::iter_prefix(subnet).collect::<Vec<_>>(),
        vec![(next_proposer, 1)]
    );
    assert_eq!(
        NodeStakeSlashLockUntil::<Test>::iter_prefix(subnet).collect::<Vec<_>>(),
        vec![(next_proposer, System::block_number() + EpochLength::get())]
    );
}

#[derive(Clone, Copy, Debug)]
enum CreditFault {
    SubnetDelegateAggregateOverflow,
    SubnetDelegatePoolInvariant,
    NodeAggregateOverflow,
}

#[test]
fn reward_credit_failures_leave_balances_atomic_and_the_hook_elects_again() {
    for fault in [
        CreditFault::SubnetDelegateAggregateOverflow,
        CreditFault::SubnetDelegatePoolInvariant,
        CreditFault::NodeAggregateOverflow,
    ] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch, proposer) = elected_round();
            run_subnet_consensus_step_v2(subnet, None, None);
            let round = SubnetElectedValidator::<Test>::get(subnet, epoch).unwrap();
            let submission = SubnetConsensusSubmission::<Test>::get(subnet, epoch).unwrap();
            assert!(round.policy.base_validator_reward > 0);
            assert!(submission.validator_reward_factor > 0);
            let (rewards, _) =
                Network::calculate_rewards_with_policy(10 * TOKEN, TOKEN, &round.policy);
            assert!(rewards.delegate_stake_rewards > 0);
            assert!(rewards.subnet_node_rewards > 0);

            match fault {
                CreditFault::SubnetDelegateAggregateOverflow => {
                    TotalDelegateStake::<Test>::put(u128::MAX);
                }
                CreditFault::SubnetDelegatePoolInvariant => {
                    // A funded, circulating pool cannot have zero total shares.
                    assert!(TotalSubnetDelegateStakeCirculatingShares::<Test>::get(subnet) > 0);
                    TotalSubnetDelegateStakeShares::<Test>::insert(subnet, 0);
                }
                CreditFault::NodeAggregateOverflow => TotalStake::<Test>::put(u128::MAX),
            }
            let nodes_before: Vec<_> = NodeSubnetStake::<Test>::iter().collect();
            let proposer_before = NodeSubnetStake::<Test>::get(proposer, subnet);
            let validators_before: Vec<_> = ValidatorDelegateStakeBalance::<Test>::iter().collect();
            let validator_total_before = TotalValidatorDelegateStakeBalance::<Test>::get();
            let pool_before = TotalSubnetDelegateStakeBalance::<Test>::get(subnet);
            let delegation_before = TotalDelegateStake::<Test>::get();
            let owner = SubnetOwner::<Test>::get(subnet).unwrap();
            let owner_before = Balances::free_balance(&owner);

            run_slot(subnet, epoch + 1);
            assert_next_round(subnet, epoch);
            assert_eq!(
                Balances::free_balance(&owner),
                owner_before + rewards.subnet_owner_reward
            );

            match fault {
                CreditFault::NodeAggregateOverflow => {
                    // Score rewards and the proposer base reward both fail their checked credit.
                    // Validator-pool allocations inside a failed node credit also roll back.
                    assert_eq!(
                        NodeSubnetStake::<Test>::iter().collect::<Vec<_>>(),
                        nodes_before
                    );
                    assert_eq!(TotalStake::<Test>::get(), u128::MAX);
                    assert_eq!(
                        ValidatorDelegateStakeBalance::<Test>::iter().collect::<Vec<_>>(),
                        validators_before
                    );
                    assert_eq!(
                        TotalValidatorDelegateStakeBalance::<Test>::get(),
                        validator_total_before
                    );
                    assert_eq!(
                        TotalSubnetDelegateStakeBalance::<Test>::get(subnet),
                        pool_before + rewards.delegate_stake_rewards
                    );
                    assert_eq!(
                        TotalDelegateStake::<Test>::get(),
                        delegation_before + rewards.delegate_stake_rewards
                    );
                }
                CreditFault::SubnetDelegateAggregateOverflow
                | CreditFault::SubnetDelegatePoolInvariant => {
                    assert_eq!(
                        TotalSubnetDelegateStakeBalance::<Test>::get(subnet),
                        pool_before
                    );
                    assert_eq!(TotalDelegateStake::<Test>::get(), delegation_before);
                    assert!(NodeSubnetStake::<Test>::get(proposer, subnet) > proposer_before);
                }
            }
        });
    }
}

#[test]
fn slash_accounting_failure_preserves_stake_and_the_hook_elects_again() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch, proposer) = elected_round();
        let nodes_before: Vec<_> = NodeSubnetStake::<Test>::iter().collect();
        let subnet_stake_before = TotalSubnetStake::<Test>::get(subnet);
        let node_reputation_before = SubnetNodeReputation::<Test>::get(subnet, proposer).unwrap();
        TotalStake::<Test>::put(0);
        // No proposal: the hook tries a positive slash, whose aggregate deduction underflows.
        run_slot(subnet, epoch + 1);

        assert_next_round(subnet, epoch);
        assert_eq!(
            NodeSubnetStake::<Test>::iter().collect::<Vec<_>>(),
            nodes_before
        );
        assert_eq!(TotalSubnetStake::<Test>::get(subnet), subnet_stake_before);
        assert_eq!(TotalStake::<Test>::get(), 0);
        assert!(
            SubnetNodeReputation::<Test>::get(subnet, proposer).unwrap() < node_reputation_before
        );
        assert!(network_events().iter().any(|event| matches!(
            event,
            Event::SubnetNodeSlashFailed { subnet_id, subnet_node_id, error }
                if *subnet_id == subnet && *subnet_node_id == proposer
                    && *error == sp_runtime::ArithmeticError::Underflow.into()
        )));
    });
}
