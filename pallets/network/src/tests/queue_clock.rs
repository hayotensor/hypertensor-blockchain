use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{assert_ok, traits::Hooks};

const TOKEN: u128 = 1_000_000_000_000_000_000;

fn active_subnet_with_queue() -> u32 {
    let name = b"pause-aware-queue".to_vec();
    build_activated_subnet(
        name.clone(),
        0,
        4,
        10_000 * TOKEN,
        MinSubnetMinStake::<Test>::get(),
    );
    let subnet_id = SubnetName::<Test>::get(name).unwrap();
    build_queued_subnet_nodes(
        subnet_id,
        4,
        5,
        10_000 * TOKEN,
        MinSubnetMinStake::<Test>::get(),
    );
    subnet_id
}

#[test]
fn queue_clock_freezes_across_partial_and_repeated_pauses_without_grace() {
    new_test_ext().execute_with(|| {
        let length = EpochLength::get();
        SubnetSlot::<Test>::insert(1, 1);
        SubnetSlot::<Test>::insert(2, 3);
        let mut block = 10 * length + 2;
        let mut total_paused = 0;
        System::set_block_number(block.into());

        for duration in [0, 1, length - 1, length + 1, 20 * length] {
            let before = [
                Network::queue_epoch_at(1, block),
                Network::queue_epoch_at(2, block),
            ];
            assert_ok!(Network::do_pause());
            System::set_block_number((block + duration / 2).into());
            assert_ok!(Network::do_pause());
            assert_eq!(NetworkPauseStartBlock::<Test>::get(), Some(block));
            block += duration;
            System::set_block_number(block.into());
            for subnet in 1..=2 {
                assert_eq!(
                    Network::queue_epoch_at(subnet, block),
                    before[subnet as usize - 1]
                );
            }

            assert_ok!(Network::do_unpause());
            total_paused += duration;
            assert_eq!(TotalNetworkPausedBlocks::<Test>::get(), total_paused);
            for subnet in 1..=2 {
                assert_eq!(
                    Network::queue_epoch_at(subnet, block),
                    before[subnet as usize - 1]
                );
            }

            // Running for one full epoch advances each clock exactly once. A redundant resume
            // must neither count that running time nor add the registration window's grace.
            block += length;
            System::set_block_number(block.into());
            assert_ok!(Network::do_unpause());
            assert_eq!(TotalNetworkPausedBlocks::<Test>::get(), total_paused);
            for subnet in 1..=2 {
                assert_eq!(
                    Network::queue_epoch_at(subnet, block),
                    before[subnet as usize - 1] + 1
                );
            }
        }
    });
}

#[test]
fn resume_preserves_queue_records_and_new_registrations_share_the_same_clock() {
    new_test_ext().execute_with(|| {
        let subnet_id = active_subnet_with_queue();
        let queue_before = SubnetNodeQueue::<Test>::get(subnet_id);
        let queued_before = QueuedSubnetNodesData::<Test>::get(subnet_id, queue_before[0].id);
        let block = Network::get_current_block_as_u32();
        assert_ok!(Network::do_pause());
        System::set_block_number((block + 20 * EpochLength::get() + 1).into());
        Network::on_initialize(System::block_number());
        assert_ok!(Network::do_unpause());
        assert_eq!(SubnetNodeQueue::<Test>::get(subnet_id), queue_before);
        assert_eq!(
            QueuedSubnetNodesData::<Test>::get(subnet_id, queued_before.id),
            queued_before
        );

        build_queued_subnet_nodes(
            subnet_id,
            5,
            6,
            10_000 * TOKEN,
            MinSubnetMinStake::<Test>::get(),
        );
        let queue = SubnetNodeQueue::<Test>::get(subnet_id);
        assert_eq!(queue.len(), 2);
        assert_eq!(
            queue[0].classification.start_epoch,
            queue[1].classification.start_epoch
        );
        assert!(
            queue[1].classification.start_epoch
                < Network::get_current_subnet_epoch_as_u32(subnet_id)
        );
        let rpc = Network::rpc_get_subnet_node_info(subnet_id, queue[1].id).unwrap();
        assert_eq!(
            rpc.classification.start_epoch,
            queue[1].classification.start_epoch
        );
    });
}

#[test]
fn partial_epoch_pause_keeps_immunity_until_the_next_admission_opportunity() {
    for allow_admission in [false, true] {
        new_test_ext().execute_with(|| {
            let subnet_id = active_subnet_with_queue();
            let queued = SubnetNodeQueue::<Test>::get(subnet_id)[0].clone();
            SubnetNodeQueueEpochs::<Test>::insert(subnet_id, 3);
            QueueImmunityEpochs::<Test>::insert(subnet_id, 3);
            ChurnLimitMultiplier::<Test>::insert(subnet_id, 1);
            if !allow_admission {
                MaxSubnetNodes::<Test>::put(4);
            }
            seed_equal_validator_delegate_stake_for_subnet(subnet_id);
            let boundary = queued.classification.start_epoch + 3;
            set_block_to_subnet_slot_epoch(boundary, subnet_id);
            let pause_start = Network::get_current_block_as_u32();
            assert_ok!(Network::do_pause());
            let duration = 21 * EpochLength::get() - 1;
            System::set_block_number((pause_start + duration).into());
            assert_ok!(Network::do_unpause());

            // One running block reaches a fresh election slot, but the queue's age has barely
            // advanced. Equality is still immature and immune, despite 21 physical epochs.
            let resume_epoch = boundary + 21;
            set_block_to_subnet_slot_epoch(resume_epoch, subnet_id);
            Network::on_initialize(System::block_number());
            assert!(QueuedSubnetNodesData::<Test>::contains_key(
                subnet_id, queued.id
            ));
            assert!(SubnetElectedValidator::<Test>::contains_key(
                subnet_id,
                resume_epoch
            ));
            let election_block = Network::get_current_block_as_u32();
            assert_eq!(Network::queue_epoch_at(subnet_id, election_block), boundary);

            // The logical epoch changes late in this round. Immunity must remain pinned to
            // its election slot, since queue admission cannot run again until the next slot.
            let late_block = election_block + EpochLength::get() - 1;
            System::set_block_number(late_block.into());
            assert_eq!(Network::queue_epoch_at(subnet_id, late_block), boundary + 1);
            run_subnet_consensus_step(subnet_id, None, Some(queued.id));
            assert_eq!(
                SubnetConsensusSubmission::<Test>::get(subnet_id, resume_epoch)
                    .unwrap()
                    .remove_queue_node_id,
                None,
            );

            set_block_to_subnet_slot_epoch(resume_epoch + 1, subnet_id);
            Network::on_initialize(System::block_number());
            if allow_admission {
                assert!(!QueuedSubnetNodesData::<Test>::contains_key(
                    subnet_id, queued.id
                ));
                let admitted = SubnetNodesData::<Test>::get(subnet_id, queued.id);
                assert_eq!(
                    admitted.classification.node_class,
                    SubnetNodeClass::Probation
                );
                // Admitted nodes return to the ordinary consensus clock.
                assert_eq!(admitted.classification.start_epoch, resume_epoch + 1);
            } else {
                assert!(QueuedSubnetNodesData::<Test>::contains_key(
                    subnet_id, queued.id
                ));
                run_subnet_consensus_step(subnet_id, None, Some(queued.id));
                assert_eq!(
                    SubnetConsensusSubmission::<Test>::get(subnet_id, resume_epoch + 1)
                        .unwrap()
                        .remove_queue_node_id,
                    Some(queued.id),
                );
            }
        });
    }
}
