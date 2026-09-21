// Copyright (C) Hypertensor.
// SPDX-License-Identifier: Apache-2.0

use crate::mock::*;
use codec::Encode;
use frame_support::traits::Randomness as RandomnessT;
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, Hash};

#[test]
fn bootstrap_is_not_available_for_committed_decisions() {
    new_test_ext().execute_with(|| {
        assert_eq!(Randomness::random_after(b"draw", 0), None);
        run_to_block(EPOCH_DURATION);
        assert_eq!(Randomness::random_after(b"draw", 0), None);
        run_to_block(EPOCH_DURATION + 1);
        // The first rotation still promotes BABE's genesis seed.
        assert_eq!(pallet_babe::Randomness::<Test>::get(), [0; 32]);
        assert_eq!(Randomness::random_after(b"draw", 0), None);
    });
}

#[test]
fn primary_vrf_contributions_reach_the_delayed_provider() {
    new_test_ext().execute_with(|| {
        // The pinned SDK consumes only segments before SegmentIndex. More than
        // 256 primary contributions are needed to move beyond the first segment.
        BabeEpochDuration::set(300);
        run_to_block(1);
        let contribution = Babe::author_vrf_randomness().unwrap();
        assert_eq!(
            pallet_babe::UnderConstruction::<Test>::get(0).as_slice(),
            &[contribution]
        );
        assert_eq!(pallet_babe::Randomness::<Test>::get(), [0; 32]);

        run_to_block(257);
        assert_eq!(pallet_babe::SegmentIndex::<Test>::get(), 1);
        let contributions = pallet_babe::UnderConstruction::<Test>::get(0);
        assert_eq!(contributions.len(), 256);
        let mut expected_input = vec![0u8; 32];
        expected_input.extend_from_slice(&2u64.to_le_bytes());
        for contribution in contributions {
            expected_input.extend_from_slice(&contribution);
        }
        let expected_seed = sp_io::hashing::blake2_256(&expected_input);

        run_block(258, 301);
        let announced_seed = pallet_babe::NextRandomness::<Test>::get();
        // Prove actual VRF bytes were mixed in, not merely that a hash changed.
        assert_eq!(announced_seed, expected_seed);
        assert_ne!(announced_seed, seed_without_contributions([0; 32], 2));
        assert_eq!(pallet_babe::Randomness::<Test>::get(), [0; 32]);

        run_block(259, 601);
        assert_eq!(pallet_babe::Randomness::<Test>::get(), announced_seed);
        let subject = b"subnet/draw/42";
        let mut input = (crate::DOMAIN, subject.as_slice()).encode();
        input.extend_from_slice(&announced_seed);
        assert_eq!(
            Randomness::random(subject),
            (BlakeTwo256::hash(&input), 258)
        );
    });
}

fn seed_without_contributions(previous: [u8; 32], epoch: u64) -> [u8; 32] {
    let mut input = previous.to_vec();
    input.extend_from_slice(&epoch.to_le_bytes());
    sp_io::hashing::blake2_256(&input)
}

#[test]
fn pinned_sdk_can_buffer_primary_vrfs_without_using_them_in_the_epoch_seed() {
    for primary_count in [1, 255, 256] {
        new_test_ext().execute_with(|| {
            BabeEpochDuration::set(300);
            run_to_block(primary_count);
            assert_eq!(pallet_babe::SegmentIndex::<Test>::get(), 0);
            assert_eq!(
                pallet_babe::UnderConstruction::<Test>::get(0).len(),
                primary_count as usize
            );
            run_block_with_kind(primary_count + 1, 301, BlockKind::SecondaryPlain);
            // This is a dependency limitation, not proof of healthy entropy.
            assert_eq!(
                pallet_babe::NextRandomness::<Test>::get(),
                seed_without_contributions([0; 32], 2)
            );
            assert_eq!(
                pallet_babe::UnderConstruction::<Test>::get(0).len(),
                primary_count as usize
            );
            run_block_with_kind(primary_count + 2, 601, BlockKind::SecondaryPlain);
            assert_eq!(
                Randomness::random_after(b"draw", 0),
                Some(Randomness::random(b"draw")),
                "a passed cutoff check does not certify that VRF bytes were consumed"
            );
        });
    }
}

#[test]
fn secondary_only_epochs_return_output_without_adding_vrf_entropy() {
    for kind in [BlockKind::SecondaryPlain, BlockKind::SecondaryVrf] {
        new_test_ext().execute_with(|| {
            if matches!(kind, BlockKind::SecondaryVrf) {
                pallet_babe::EpochConfig::<Test>::mutate(|config| {
                    config.as_mut().unwrap().allowed_slots =
                        sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryVRFSlots;
                });
            }
            let mut predicted_current = [0; 32];
            let mut predicted_next = [0; 32];
            for number in 1..=4 * EPOCH_DURATION + 1 {
                let epoch = (number - 1) / EPOCH_DURATION;
                if number > 1 && (number - 1) % EPOCH_DURATION == 0 {
                    predicted_current = predicted_next;
                    predicted_next = seed_without_contributions(predicted_next, epoch + 1);
                }
                run_block_with_kind(number, number, kind);
                assert!(pallet_babe::UnderConstruction::<Test>::iter()
                    .next()
                    .is_none());
                assert_eq!(pallet_babe::Randomness::<Test>::get(), predicted_current);
                assert_eq!(pallet_babe::NextRandomness::<Test>::get(), predicted_next);
                let mut input = (crate::DOMAIN, b"draw".as_slice()).encode();
                input.extend_from_slice(&predicted_current);
                assert_eq!(Randomness::random(b"draw").0, BlakeTwo256::hash(&input));
            }
        });
    }
}

#[test]
fn raw_api_returns_output_at_genesis_without_a_vrf_or_epoch_cutoff() {
    new_test_ext().execute_with(|| {
        assert_eq!(Babe::author_vrf_randomness(), None);
        for subject in [b"".as_slice(), b"draw", &[0u8; 256]] {
            let mut input = (crate::DOMAIN, subject).encode();
            input.extend_from_slice(&[0; 32]);
            assert_eq!(Randomness::random(subject), (BlakeTwo256::hash(&input), 0));
            assert_eq!(
                <Randomness as RandomnessT<H256, u64>>::random(subject),
                Randomness::random(subject)
            );
        }
    });
}

#[test]
fn freshness_cutoff_is_preserved_and_equality_is_rejected() {
    new_test_ext().execute_with(|| {
        // Inputs fixed in epoch zero, draw in epoch three as required by BABE.
        run_to_block(3 * EPOCH_DURATION + 1);
        let (seed, cutoff) = Randomness::random(b"draw");
        assert_eq!(cutoff, 2 * EPOCH_DURATION + 1);
        assert_eq!(Randomness::random_after(b"draw", 1), Some((seed, cutoff)));
        assert_eq!(
            Randomness::random_after(b"draw", cutoff - 1),
            Some((seed, cutoff))
        );
        assert_eq!(Randomness::random_after(b"draw", cutoff), None);
        assert_eq!(Randomness::random_after(b"draw", cutoff + 1), None);
        assert_eq!(Randomness::random_after(b"draw", u64::MAX), None);
    });
}

#[test]
fn subjects_are_separated_and_outputs_are_stable_within_an_epoch() {
    new_test_ext().execute_with(|| {
        run_to_block(3 * EPOCH_DURATION + 1);
        let first = Randomness::random(b"subnet/1");
        assert_ne!(first.0, Randomness::random(b"subnet/2").0);
        assert_ne!(
            first.0,
            pallet_babe::RandomnessFromTwoEpochsAgo::<Test>::random(b"subnet/1").0
        );
        run_to_block(4 * EPOCH_DURATION);
        assert_eq!(Randomness::random(b"subnet/1"), first);
        run_to_block(4 * EPOCH_DURATION + 1);
        let next = Randomness::random(b"subnet/1");
        assert_ne!(next.0, first.0);
        assert!(next.1 > first.1);
    });
}

#[test]
fn local_block_inputs_and_next_epoch_seed_do_not_change_current_output() {
    new_test_ext().execute_with(|| {
        run_to_block(3 * EPOCH_DURATION + 1);
        let before = Randomness::random(b"draw");
        System::initialize(
            &(System::block_number() + 1),
            &H256::repeat_byte(99),
            &Default::default(),
        );
        pallet_timestamp::Now::<Test>::put(1_000_000);
        pallet_babe::NextRandomness::<Test>::put([99; 32]);
        assert_eq!(Randomness::random(b"draw"), before);
    });
}

#[test]
fn standard_trait_matches_inherent_api_including_empty_subject() {
    new_test_ext().execute_with(|| {
        run_to_block(3 * EPOCH_DURATION + 1);
        assert_eq!(
            <Randomness as RandomnessT<H256, u64>>::random(b"draw"),
            Randomness::random(b"draw")
        );
        assert_eq!(
            <Randomness as RandomnessT<H256, u64>>::random_seed(),
            Randomness::random(b"")
        );
    });
}

#[test]
fn skipped_slot_epochs_do_not_fabricate_a_new_block_cutoff() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        run_block(2, 3 * EPOCH_DURATION + 1);
        assert_eq!(Babe::epoch_index(), 3);
        assert_eq!(Randomness::random_after(b"draw", 1), None);
        // The raw API is still available, although no new entropy was promoted.
        let mut input = (crate::DOMAIN, b"draw".as_slice()).encode();
        input.extend_from_slice(&[0; 32]);
        assert_eq!(Randomness::random(b"draw"), (BlakeTwo256::hash(&input), 0));
    });
}

#[test]
fn reading_randomness_does_not_write_storage() {
    new_test_ext().execute_with(|| {
        run_to_block(3 * EPOCH_DURATION + 1);
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        let _ = Randomness::random(b"draw");
        let _ = Randomness::random_after(b"draw", 1);
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
    });
}
