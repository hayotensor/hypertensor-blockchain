use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{assert_noop, assert_ok, traits::Hooks, weights::WeightMeter};
use sp_std::collections::btree_map::BTreeMap;

const TOKEN: u128 = 1_000_000_000_000_000_000;
const NODES: u32 = 4;

#[test]
fn resume_cutoff_respects_assigned_slots_including_the_first_epoch() {
    new_test_ext().execute_with(|| {
        let subnet = 1;
        let slot = DesignatedEpochSlots::get();
        SubnetSlot::<Test>::insert(subnet, slot);
        LastNetworkUnpauseBlock::<Test>::put(slot - 1);
        assert!(!Network::consensus_round_precedes_network_resume(subnet, 0));
        LastNetworkUnpauseBlock::<Test>::put(slot);
        assert!(Network::consensus_round_precedes_network_resume(subnet, 0));
        assert!(!Network::consensus_round_precedes_network_resume(subnet, 1));
        LastNetworkUnpauseBlock::<Test>::put(slot + EpochLength::get());
        assert!(Network::consensus_round_precedes_network_resume(subnet, 1));
        assert!(!Network::consensus_round_precedes_network_resume(subnet, 2));
    });
}

fn fixture(emergency: bool) -> (u32, u32) {
    increase_epochs(50);
    let name = b"global-pause".to_vec();
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
    ValidatorDelegateStakeSlashThreshold::<Test>::put(TOKEN / 2);
    BaseValidatorDelegateStakeSlashPercentage::<Test>::put(TOKEN / 100);
    MaxValidatorDelegateStakeSlashAmount::<Test>::put(u128::MAX);
    let epoch = Network::get_current_subnet_epoch_as_u32(subnet);
    if emergency {
        EmergencySubnetNodeElectionData::<Test>::insert(
            subnet,
            EmergencySubnetValidatorData {
                subnet_node_ids: (1..=NODES).collect(),
                target_emergency_validators_epochs: 100,
                max_emergency_validators_epoch: epoch + 200,
                total_epochs: 0,
                activated: true,
                started_subnet_epoch: epoch,
                reputation_factors: Network::get_reputation_factors_for_epoch(subnet, epoch),
                min_subnet_node_reputation: MinSubnetNodeReputation::<Test>::get(subnet),
                min_weight_decrease_reputation_threshold:
                    SubnetNodeMinWeightDecreaseReputationThreshold::<Test>::get(subnet),
            },
        );
    }
    Network::elect_validator(subnet, epoch, System::block_number());
    let round = SubnetElectedValidator::<Test>::get(subnet, epoch).unwrap();
    assert_eq!(round.emergency.is_some(), emergency);
    let emission_epoch = ConsensusRoundSettlementEmissionEpoch::<Test>::get(subnet, epoch).unwrap();
    FinalSubnetEmissionWeights::<Test>::insert(
        emission_epoch,
        DistributionData {
            subnets_emissions: 10 * TOKEN,
            subnet_weights: BTreeMap::from([(subnet, TOKEN)]),
        },
    );
    (subnet, epoch)
}

fn propose(subnet: u32, epoch: u32) -> frame_support::pallet_prelude::DispatchResultWithPostInfo {
    let node = SubnetElectedValidator::<Test>::get(subnet, epoch)
        .unwrap()
        .validator_subnet_node_id;
    let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
    Network::propose_attestation(
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
    )
}

fn submit(subnet: u32, epoch: u32, votes: u32) {
    if votes == 0 {
        return;
    }
    assert_ok!(propose(subnet, epoch));
    let proposer = SubnetElectedValidator::<Test>::get(subnet, epoch)
        .unwrap()
        .validator_subnet_node_id;
    for node in (1..=NODES)
        .filter(|node| *node != proposer)
        .take((votes - 1) as usize)
    {
        let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
        assert_ok!(Network::attest(
            RuntimeOrigin::signed(hotkey),
            subnet,
            node,
            None
        ));
    }
}

fn resume_after(blocks: u32) {
    assert_ok!(Network::do_pause());
    System::set_block_number(System::block_number() + blocks);
    Network::on_initialize(System::block_number());
    assert_ok!(Network::do_unpause());
}

fn settle(subnet: u32, epoch: u32) {
    set_block_to_subnet_slot_epoch(epoch, subnet);
    Network::emission_settlement_step(
        &mut WeightMeter::new(),
        System::block_number(),
        Network::get_current_epoch_as_u32(),
        epoch,
        subnet,
    );
}

fn cancellation_count(subnet: u32, epoch: u32) -> usize {
    network_events()
        .iter()
        .filter(|event| {
            matches!(event,
        Event::ConsensusRoundCancelled { subnet_id, subnet_epoch }
        if *subnet_id == subnet && *subnet_epoch == epoch)
        })
        .count()
}

#[test]
fn global_resume_cancels_missing_partial_and_quorate_regular_and_emergency_rounds() {
    for emergency in [false, true] {
        for votes in [0, 1, NODES] {
            new_test_ext().execute_with(|| {
                let (subnet, epoch) = fixture(emergency);
                submit(subnet, epoch, votes);
                let round = SubnetElectedValidator::<Test>::get(subnet, epoch).unwrap();
                let submission = SubnetConsensusSubmission::<Test>::get(subnet, epoch);
                let owner = SubnetOwner::<Test>::get(subnet).unwrap();
                let owner_balance = Balances::free_balance(owner.clone());
                let issuance = Balances::total_issuance();
                let subnet_delegation = TotalSubnetDelegateStakeBalance::<Test>::get(subnet);
                SubnetRewardCapacitor::<Test>::insert(subnet, TOKEN);
                resume_after(2 * EpochLength::get());
                settle(subnet, epoch + 3);

                assert_eq!(
                    ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
                    Some(true)
                );
                assert_eq!(
                    PendingConsensusRoundSettlementEpoch::<Test>::get(subnet),
                    None
                );
                assert!(
                    !ConsensusRoundSettlementEmissionEpoch::<Test>::contains_key(subnet, epoch)
                );
                assert!(!Network::is_node_stake_slash_locked(
                    subnet,
                    round.validator_subnet_node_id
                ));
                assert!(!Network::is_validator_delegate_stake_slash_locked(
                    round.validator_id
                ));
                assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 11 * TOKEN);
                assert_eq!(Balances::free_balance(owner), owner_balance);
                assert_eq!(Balances::total_issuance(), issuance);
                assert_eq!(
                    TotalSubnetDelegateStakeBalance::<Test>::get(subnet),
                    subnet_delegation
                );
                assert_eq!(SubnetReputation::<Test>::get(subnet), TOKEN);
                for node in 1..=NODES {
                    let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
                    assert_eq!(NodeSubnetStake::<Test>::get(node, subnet), 100 * TOKEN);
                    assert_eq!(
                        ValidatorDelegateStakeBalance::<Test>::get(validator),
                        100 * TOKEN
                    );
                    assert_eq!(SubnetNodeReputation::<Test>::get(subnet, node), Some(TOKEN));
                    assert_eq!(SubnetNodeProbationProgress::<Test>::get(subnet, node), 0);
                }
                assert_eq!(
                    SubnetElectedValidator::<Test>::get(subnet, epoch),
                    Some(round)
                );
                assert_eq!(
                    SubnetConsensusSubmission::<Test>::get(subnet, epoch),
                    submission
                );
                if emergency {
                    assert_eq!(
                        EmergencySubnetNodeElectionData::<Test>::get(subnet)
                            .unwrap()
                            .total_epochs,
                        0
                    );
                }
                assert_eq!(cancellation_count(subnet, epoch), 1);
                settle(subnet, epoch + 3);
                assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 11 * TOKEN);
                assert_eq!(cancellation_count(subnet, epoch), 1);

                Network::on_initialize(System::block_number());
                assert_eq!(
                    PendingConsensusRoundSettlementEpoch::<Test>::get(subnet),
                    Some(epoch + 3)
                );
                assert_ok!(propose(subnet, epoch + 3));
            });
        }
    }
}

#[test]
fn same_block_and_short_pauses_reject_old_consensus_and_rpc_work() {
    for duration in [0, 1] {
        for votes in [0, 1] {
            new_test_ext().execute_with(|| {
                let (subnet, epoch) = fixture(false);
                submit(subnet, epoch, votes);
                resume_after(duration);
                assert_eq!(Network::get_current_subnet_epoch_as_u32(subnet), epoch);
                assert_noop!(
                    propose(subnet, epoch),
                    Error::<Test>::ConsensusRoundCancelled
                );
                if votes > 0 {
                    let proposer = SubnetElectedValidator::<Test>::get(subnet, epoch)
                        .unwrap()
                        .validator_subnet_node_id;
                    let node = (1..=NODES).find(|node| *node != proposer).unwrap();
                    let hotkey = Network::get_subnet_node_associated_hotkey(subnet, node).unwrap();
                    assert_noop!(
                        Network::attest(RuntimeOrigin::signed(hotkey), subnet, node, None),
                        Error::<Test>::ConsensusRoundCancelled
                    );
                }
                let status = Network::rpc_get_subnet_epoch_status(subnet).unwrap();
                assert_eq!(status.elected_validator_subnet_node_id, None);
                assert!(!status.proposal_submitted);
                assert!(!status.within_proposal_attestation_window);
                // The actual hook must cancel and elect a fresh round in one assigned slot.
                set_block_to_subnet_slot_epoch(epoch + 1, subnet);
                Network::on_initialize(System::block_number());
                assert_eq!(cancellation_count(subnet, epoch), 1);
                assert_eq!(
                    PendingConsensusRoundSettlementEpoch::<Test>::get(subnet),
                    Some(epoch + 1)
                );
                assert_ok!(propose(subnet, epoch + 1));
            });
        }
    }
}

#[test]
fn resume_also_discards_older_unsettled_rounds_but_noop_unpause_keeps_fresh_rounds() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture(false);
        // Simulate deferred settlement before governance pauses. Fresh start includes this debt.
        set_block_to_subnet_slot_epoch(epoch + 2, subnet);
        resume_after(1);
        settle(subnet, epoch + 3);
        assert_eq!(cancellation_count(subnet, epoch), 1);
        assert_eq!(NodeSubnetStake::<Test>::get(1, subnet), 100 * TOKEN);
        Network::on_initialize(System::block_number());
        let cutoff = LastNetworkUnpauseBlock::<Test>::get();
        assert_ok!(Network::do_unpause());
        assert_eq!(LastNetworkUnpauseBlock::<Test>::get(), cutoff);
        assert_ok!(propose(subnet, epoch + 3));
    });
}

#[test]
fn repeated_pauses_cancel_once_and_missing_allocation_never_creates_rewards() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture(false);
        let emission_epoch =
            ConsensusRoundSettlementEmissionEpoch::<Test>::get(subnet, epoch).unwrap();
        FinalSubnetEmissionWeights::<Test>::remove(emission_epoch);
        resume_after(1);
        resume_after(2 * EpochLength::get());
        settle(subnet, epoch + 3);
        assert_eq!(cancellation_count(subnet, epoch), 1);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 0);
        assert_eq!(
            PendingConsensusRoundSettlementEpoch::<Test>::get(subnet),
            None
        );
    });
}

#[test]
fn cancellation_overflow_keeps_liabilities_for_an_exact_retry() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture(false);
        let round = SubnetElectedValidator::<Test>::get(subnet, epoch).unwrap();
        SubnetRewardCapacitor::<Test>::insert(subnet, u128::MAX);
        resume_after(1);
        settle(subnet, epoch + 1);
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
            Some(false)
        );
        assert!(Network::is_node_stake_slash_locked(
            subnet,
            round.validator_subnet_node_id
        ));
        assert!(Network::is_validator_delegate_stake_slash_locked(
            round.validator_id
        ));
        assert_eq!(cancellation_count(subnet, epoch), 0);
        SubnetRewardCapacitor::<Test>::insert(subnet, 0);
        settle(subnet, epoch + 2);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), 10 * TOKEN);
        assert_eq!(cancellation_count(subnet, epoch), 1);
    });
}

#[test]
fn global_resume_does_not_reprocess_settled_history() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = fixture(false);
        settle(subnet, epoch + 1);
        let stake = TotalSubnetStake::<Test>::get(subnet);
        let capacitor = SubnetRewardCapacitor::<Test>::get(subnet);
        let reputation = SubnetReputation::<Test>::get(subnet);
        resume_after(1);
        Network::precheck_subnet_consensus_submission(
            subnet,
            epoch,
            Network::get_current_epoch_as_u32(),
        );
        assert_eq!(TotalSubnetStake::<Test>::get(subnet), stake);
        assert_eq!(SubnetRewardCapacitor::<Test>::get(subnet), capacitor);
        assert_eq!(SubnetReputation::<Test>::get(subnet), reputation);
        assert_eq!(cancellation_count(subnet, epoch), 0);
    });
}
