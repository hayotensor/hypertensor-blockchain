use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{
    assert_ok,
    traits::Hooks,
    weights::{Weight, WeightMeter},
};
use sp_std::collections::btree_map::BTreeMap;

const TOKEN: u128 = 1_000_000_000_000_000_000;
const BOOTSTRAP: u32 = 4;
const NEW_NODE: u32 = BOOTSTRAP + 1;

fn probation_round() -> (u32, u32) {
    increase_epochs(50);
    let name = b"mandatory-subnet-slot".to_vec();
    let stake = MinSubnetMinStake::<Test>::get();
    build_activated_subnet(name.clone(), 0, BOOTSTRAP, 10_000 * TOKEN, stake);
    let subnet = SubnetName::<Test>::get(name).unwrap();
    build_queued_subnet_nodes(subnet, BOOTSTRAP, NEW_NODE, 10_000 * TOKEN, stake);
    seed_equal_validator_delegate_stake_for_subnet(subnet);
    ChurnLimit::<Test>::insert(subnet, 1);
    ChurnLimitMultiplier::<Test>::insert(subnet, 1);
    ProbationRounds::<Test>::insert(subnet, 1);
    let start = QueuedSubnetNodesData::<Test>::get(subnet, NEW_NODE)
        .classification
        .start_epoch;
    let epoch = start + SubnetNodeQueueEpochs::<Test>::get(subnet) + 1;
    set_block_to_subnet_slot_epoch(epoch, subnet);
    Network::handle_registration_queue(&mut WeightMeter::new(), subnet, epoch);
    assert_eq!(TotalAdmittedSubnetNodes::<Test>::get(subnet), NEW_NODE);
    assert_eq!(TotalSubnetElectableNodes::<Test>::get(subnet), BOOTSTRAP);
    assert_eq!(
        SubnetNodesData::<Test>::get(subnet, NEW_NODE)
            .classification
            .node_class,
        SubnetNodeClass::Probation
    );

    let consumed = Network::on_initialize(System::block_number());
    assert!(consumed.all_lte(MaximumHooksWeight::get()));
    let round = SubnetElectedValidator::<Test>::get(subnet, epoch).unwrap();
    assert_eq!(round.eligible_subnet_node_ids.len(), BOOTSTRAP as usize);
    let proposer = round.validator_subnet_node_id;
    let hotkey = Network::get_subnet_node_associated_hotkey(subnet, proposer).unwrap();
    assert_ok!(Network::propose_attestation(
        RuntimeOrigin::signed(hotkey),
        subnet,
        (1..=NEW_NODE)
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
    for node in (1..=BOOTSTRAP).filter(|node| *node != proposer) {
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
            subnets_emissions: 100 * TOKEN,
            subnet_weights: BTreeMap::from([(subnet, TOKEN)]),
        },
    );
    (subnet, epoch)
}

fn exact_budget(subnet: u32, epoch: u32) -> Weight {
    let h = SubnetConsensusSubmissionMaxItems::<Test>::get(subnet, epoch);
    let admitted = TotalAdmittedSubnetNodes::<Test>::get(subnet);
    let queued = TotalSubnetNodes::<Test>::get(subnet).saturating_sub(admitted);
    <Test as Config>::WeightInfo::emission_step_selectors()
        .saturating_add(Network::subnet_slot_mandatory_weight(h, queued, admitted))
}

#[test]
fn exact_mandatory_budget_settles_promotes_and_elects_before_optional_work() {
    new_test_ext().execute_with(|| {
        let (subnet, epoch) = probation_round();
        let limit = exact_budget(subnet, epoch);
        let old_candidate_count = TotalSubnetElectableNodes::<Test>::get(subnet);
        assert_eq!(old_candidate_count, BOOTSTRAP);
        let stake_before = NodeSubnetStake::<Test>::get(NEW_NODE, subnet);
        set_block_to_subnet_slot_epoch(epoch + 1, subnet);
        let mut meter = WeightMeter::with_limit(limit);
        assert!(Network::run_assigned_subnet_slot(
            &mut meter,
            System::block_number(),
            epoch + 1,
            subnet
        ));
        assert_eq!(meter.consumed(), limit);
        assert_eq!(
            ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
            Some(true)
        );
        assert_eq!(
            PendingConsensusRoundSettlementEpoch::<Test>::get(subnet),
            Some(epoch + 1)
        );
        assert_eq!(TotalSubnetElectableNodes::<Test>::get(subnet), NEW_NODE);
        assert_eq!(
            SubnetNodesData::<Test>::get(subnet, NEW_NODE)
                .classification
                .node_class,
            SubnetNodeClass::Active
        );
        let next = SubnetElectedValidator::<Test>::get(subnet, epoch + 1).unwrap();
        assert_eq!(next.eligible_subnet_node_ids.len(), NEW_NODE as usize);
        assert!(next.eligible_subnet_node_ids.contains(&NEW_NODE));
        // Promotion changes the next election; it cannot pay the completed Probation round.
        assert_eq!(NodeSubnetStake::<Test>::get(NEW_NODE, subnet), stake_before);
    });
}

#[test]
fn incomplete_mandatory_budget_does_not_partially_settle_or_elect() {
    for short_proof in [false, true] {
        new_test_ext().execute_with(|| {
            let (subnet, epoch) = probation_round();
            let complete = exact_budget(subnet, epoch);
            let limit = if short_proof {
                Weight::from_parts(complete.ref_time(), complete.proof_size() - 1)
            } else {
                Weight::from_parts(complete.ref_time() - 1, complete.proof_size())
            };
            set_block_to_subnet_slot_epoch(epoch + 1, subnet);
            let mut meter = WeightMeter::with_limit(limit);
            frame_support::assert_storage_noop!(assert!(!Network::run_assigned_subnet_slot(
                &mut meter,
                System::block_number(),
                epoch + 1,
                subnet,
            )));
            assert!(meter.consumed().all_lte(limit));
            assert_eq!(
                ConsensusRoundSettlementStatus::<Test>::get(subnet, epoch),
                Some(false)
            );
            assert_eq!(
                PendingConsensusRoundSettlementEpoch::<Test>::get(subnet),
                Some(epoch)
            );
            assert!(!SubnetElectedValidator::<Test>::contains_key(
                subnet,
                epoch + 1
            ));
        });
    }
}

#[test]
fn mandatory_hook_envelope_fits_runtime_configuration_and_bounds_each_branch() {
    new_test_ext().execute_with(|| {
        let max_nodes = <Test as Config>::MaxSubnetNodesUpperBound::get();
        let max_queue = <Test as Config>::MaxRegisteredNodesUpperBound::get();
        let maximum = Network::subnet_slot_mandatory_weight(max_nodes, max_queue, max_nodes)
            .max(Network::subnet_slot_mandatory_weight(0, 0, max_nodes));
        for h in [0, 1, 3, max_nodes] {
            for q in [0, 1, max_queue] {
                for e in [0, 3, max_nodes] {
                    assert!(Network::subnet_slot_mandatory_weight(h, q, e).all_lte(maximum));
                }
            }
        }
        assert!(Network::mandatory_subnet_hook_weight().all_lte(MaximumHooksWeight::get()));
        Network::integrity_test();
    });
}
