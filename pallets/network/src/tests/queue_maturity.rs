use super::mock::*;
use crate::tests::test_utils::*;
use crate::{
    ChurnLimit, MinSubnetMinStake, PendingOwnerU32Update, PendingSubnetNodeQueueEpochs,
    QueueImmunityEpochs, RegisteredSubnetNodesData, SubnetConsensusSubmission,
    SubnetElectedValidator, SubnetName, SubnetNodeClass, SubnetNodeQueue, SubnetNodeQueueEpochs,
    SubnetNodesData,
};
use frame_support::weights::WeightMeter;

#[test]
fn epoch_period_elapsed_is_strict_and_saturating() {
    new_test_ext().execute_with(|| {
        assert!(!Network::has_epoch_period_elapsed(10, 3, 13));
        assert!(Network::has_epoch_period_elapsed(10, 3, 14));

        assert!(!Network::has_epoch_period_elapsed(10, u32::MAX, u32::MAX));
    });
}

#[test]
fn default_queue_immunity_covers_activation_boundary() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "queue-immunity-boundary".into();
        let deposit_amount = 10_000_000_000_000_000_000_000;
        let stake_amount = MinSubnetMinStake::<Test>::get();
        let node_stake = 1_000_000_000_000_000_000_000;
        let initial_node_count = 4;

        build_activated_subnet(
            subnet_name.clone(),
            0,
            initial_node_count,
            deposit_amount,
            stake_amount,
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        build_registered_nodes_in_queue(
            subnet_id,
            initial_node_count,
            initial_node_count + 1,
            deposit_amount,
            node_stake,
        );

        let queue_epochs = SubnetNodeQueueEpochs::<Test>::get(subnet_id);
        let immunity_epochs = QueueImmunityEpochs::<Test>::get(subnet_id);
        assert_eq!(immunity_epochs, queue_epochs);

        let queued_node = SubnetNodeQueue::<Test>::get(subnet_id)
            .first()
            .cloned()
            .unwrap();
        let boundary_epoch = queued_node
            .classification
            .start_epoch
            .saturating_add(queue_epochs);

        // At equality, the node is neither removable nor activation-eligible.
        set_block_to_subnet_slot_epoch(boundary_epoch, subnet_id);
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        assert_eq!(subnet_epoch, boundary_epoch);
        Network::elect_validator(subnet_id, subnet_epoch, System::block_number());
        assert!(SubnetElectedValidator::<Test>::contains_key(
            subnet_id,
            subnet_epoch
        ));
        run_subnet_consensus_step_v2(subnet_id, None, Some(queued_node.id));

        let boundary_submission =
            SubnetConsensusSubmission::<Test>::get(subnet_id, subnet_epoch).unwrap();
        assert_eq!(boundary_submission.remove_queue_node_id, None);

        Network::handle_registration_queue(&mut WeightMeter::new(), subnet_id, boundary_epoch);
        assert!(RegisteredSubnetNodesData::<Test>::contains_key(
            subnet_id,
            queued_node.id
        ));
        assert!(SubnetNodeQueue::<Test>::get(subnet_id)
            .iter()
            .any(|node| node.id == queued_node.id));

        // On the following epoch, queue processing runs in `on_initialize` before
        // consensus extrinsics and activates the node at its first opportunity.
        let eligible_epoch = boundary_epoch.saturating_add(1);
        set_block_to_subnet_slot_epoch(eligible_epoch, subnet_id);
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        assert_eq!(subnet_epoch, eligible_epoch);
        Network::handle_registration_queue(&mut WeightMeter::new(), subnet_id, eligible_epoch);
        assert!(!SubnetNodeQueue::<Test>::get(subnet_id)
            .iter()
            .any(|node| node.id == queued_node.id));
        let active_node = SubnetNodesData::<Test>::get(subnet_id, queued_node.id);
        assert_eq!(active_node.classification.node_class, SubnetNodeClass::Idle);

        // Once activated, it cannot be selected as a queue removal even if a
        // validator supplies the old queue ID later in the epoch.
        Network::elect_validator(subnet_id, subnet_epoch, System::block_number());
        run_subnet_consensus_step_v2(subnet_id, None, Some(queued_node.id));
        let eligible_submission =
            SubnetConsensusSubmission::<Test>::get(subnet_id, subnet_epoch).unwrap();
        assert_eq!(eligible_submission.remove_queue_node_id, None);
    });
}

#[test]
fn registration_queue_processing_uses_duration_scheduled_for_evaluated_epoch() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "pending-queue-duration".into();
        let deposit_amount = 10_000_000_000_000_000_000_000;
        let stake_amount = MinSubnetMinStake::<Test>::get();
        let initial_node_count = 4;

        build_activated_subnet(
            subnet_name.clone(),
            0,
            initial_node_count,
            deposit_amount,
            stake_amount,
        );
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        build_registered_nodes_in_queue(
            subnet_id,
            initial_node_count,
            initial_node_count + 2,
            deposit_amount,
            stake_amount,
        );

        let queue = SubnetNodeQueue::<Test>::get(subnet_id);
        let start_epoch = queue.first().unwrap().classification.start_epoch;
        assert!(queue
            .iter()
            .all(|node| node.classification.start_epoch == start_epoch));

        SubnetNodeQueueEpochs::<Test>::insert(subnet_id, 1);
        ChurnLimit::<Test>::insert(subnet_id, 1);
        let effective_subnet_epoch = start_epoch.saturating_add(3);
        PendingSubnetNodeQueueEpochs::<Test>::insert(
            subnet_id,
            PendingOwnerU32Update {
                value: 3,
                effective_subnet_epoch,
                owner: account(1),
            },
        );

        // Before activation, the old one-epoch duration applies and one node advances.
        Network::handle_registration_queue(
            &mut WeightMeter::new(),
            subnet_id,
            effective_subnet_epoch.saturating_sub(1),
        );
        assert_eq!(SubnetNodeQueue::<Test>::get(subnet_id).len(), 1);

        // At activation, the pending three-epoch duration applies. Equality is still waiting,
        // so a raw read of the old value would incorrectly activate this remaining node.
        Network::handle_registration_queue(
            &mut WeightMeter::new(),
            subnet_id,
            effective_subnet_epoch,
        );
        assert_eq!(SubnetNodeQueue::<Test>::get(subnet_id).len(), 1);

        Network::handle_registration_queue(
            &mut WeightMeter::new(),
            subnet_id,
            effective_subnet_epoch.saturating_add(1),
        );
        assert!(SubnetNodeQueue::<Test>::get(subnet_id).is_empty());
    });
}
