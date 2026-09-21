use super::mock::*;
use crate::Config;
use codec::Encode;
use core::num::NonZeroU32;
use frame_support::traits::Randomness as RandomnessT;
use sp_core::{H256, U256};

use crate::utilities::randomness::index_from_hash;

// Network tests use the real randomness pallet with controlled BABE storage.
// Actual VRF accumulation and epoch transitions are tested by pallet-randomness.
fn seed_babe_randomness(seed: u8) {
    pallet_babe::Randomness::<Test>::put([seed; 32]);
    pallet_babe::EpochStart::<Test>::put((20, 40));
}

#[test]
fn network_provider_is_the_babe_randomness_pallet() {
    new_test_ext().execute_with(|| {
        seed_babe_randomness(7);
        let subject = (1u32, 2u32).encode();
        assert_eq!(
            <<Test as Config>::Randomness as RandomnessT<H256, BlockNumber>>::random(&subject),
            BabeRandomness::random(&subject)
        );
        assert_eq!(BabeRandomness::random(&subject).1, 20);
    });
}

#[test]
fn bounded_random_index_handles_empty_and_singleton_pools() {
    new_test_ext().execute_with(|| {
        assert_eq!(Network::get_bounded_random_index((1, 1), 0), None);
        assert_eq!(Network::get_bounded_random_index((1, 1), 1), Some(0));
    });
}

#[test]
fn bounded_random_index_is_deterministic_and_bounded() {
    new_test_ext().execute_with(|| {
        seed_babe_randomness(7);
        for upper_bound in (2..=512).chain([u32::MAX]) {
            let first = Network::get_bounded_random_index((1u32, 2u32), upper_bound).unwrap();
            assert!(first < upper_bound);
            assert_eq!(
                Network::get_bounded_random_index((1u32, 2u32), upper_bound),
                Some(first)
            );
        }
    });
}

#[test]
fn full_hash_reduction_always_returns_an_index_even_for_extreme_hashes() {
    for upper_bound in (1..=512).chain([1 << 31, u32::MAX - 1, u32::MAX]) {
        let bound = NonZeroU32::new(upper_bound).unwrap();
        for hash in [[0; 32], [255; 32], [128; 32], [1; 32]] {
            let index = index_from_hash(&hash, bound);
            assert!(index < upper_bound);
            let expected = (U256::from_big_endian(&hash) % U256::from(upper_bound)).low_u32();
            assert_eq!(index, expected);
        }
    }
}

#[test]
fn full_hash_reduction_uses_the_entire_digest() {
    let bound = NonZeroU32::new(257).unwrap();
    for byte_index in 0..32 {
        let mut hash = [0u8; 32];
        hash[byte_index] = 1;
        assert_ne!(index_from_hash(&hash, bound), 0);
    }
}

#[test]
fn full_hash_reduction_has_the_expected_distribution_in_an_exhaustive_small_domain() {
    // Independently count preimages: modulo reduction gives floor(M/n) or
    // ceil(M/n) preimages per index. In production M = 2^256, rather than 2^16.
    for upper_bound in [1, 2, 3, 7, 256, 257, 511] {
        let bound = NonZeroU32::new(upper_bound).unwrap();
        let mut counts = vec![0u32; upper_bound as usize];
        for value in 0..=u16::MAX {
            counts[index_from_hash(&value.to_be_bytes(), bound) as usize] += 1;
        }
        assert!(counts.iter().max().unwrap() - counts.iter().min().unwrap() <= 1);
        assert_eq!(counts.iter().sum::<u32>(), 1 << 16);
    }
}

#[test]
fn bounded_random_index_is_available_with_genesis_or_unchanging_babe_seed() {
    new_test_ext().execute_with(|| {
        assert_eq!(pallet_babe::Randomness::<Test>::get(), [0; 32]);
        assert_eq!(pallet_babe::EpochStart::<Test>::get(), (0, 0));
        for epoch in 0..=32u32 {
            for upper_bound in [1, 2, 3, 64, 512, u32::MAX] {
                let index = Network::get_bounded_random_index((1u32, epoch), upper_bound)
                    .expect("a nonempty pool always gets an index, even without fresh entropy");
                assert!(index < upper_bound);
            }
        }
    });
}

#[test]
fn nonempty_attributable_pool_elects_during_babe_bootstrap() {
    new_test_ext().execute_with(|| {
        assert_eq!(pallet_babe::EpochStart::<Test>::get(), (0, 0));
        let subnet_id = 11u32;
        let candidates = vec![10u32, 20, 30, 40];
        crate::SubnetNodeElectionSlots::<Test>::insert(subnet_id, &candidates);
        for node_id in &candidates {
            crate::SubnetNodeValidatorId::<Test>::insert(subnet_id, *node_id, *node_id);
        }
        Network::elect_validator(subnet_id, 0, System::block_number());
        let elected = crate::SubnetElectedValidator::<Test>::get(subnet_id, 0)
            .expect("bootstrap randomness does not suppress an otherwise eligible election");
        assert!(candidates.contains(&elected.validator_subnet_node_id));
    });
}

#[test]
fn draw_ignores_parent_hash_block_number_and_timestamp() {
    new_test_ext().execute_with(|| {
        seed_babe_randomness(7);
        let first = Network::get_bounded_random_index((1u32, 2u32), 512);
        System::set_block_number(99);
        System::initialize(&100, &H256::repeat_byte(99), &Default::default());
        pallet_timestamp::Now::<Test>::put(999_999u64);
        assert_eq!(Network::get_bounded_random_index((1u32, 2u32), 512), first);
    });
}

#[test]
fn draws_depend_on_the_babe_seed_and_round_domain() {
    new_test_ext().execute_with(|| {
        seed_babe_randomness(7);
        let first = Network::get_bounded_random_index((1u32, 2u32), u32::MAX);
        assert_ne!(
            Network::get_bounded_random_index((2u32, 2u32), u32::MAX),
            first
        );
        assert_ne!(
            Network::get_bounded_random_index((1u32, 3u32), u32::MAX),
            first
        );
        seed_babe_randomness(8);
        assert_ne!(
            Network::get_bounded_random_index((1u32, 2u32), u32::MAX),
            first
        );
    });
}

#[test]
fn electing_the_same_round_at_another_block_does_not_change_the_draw() {
    let elect_at = |block: u32| {
        new_test_ext().execute_with(|| {
            seed_babe_randomness(7);
            System::set_block_number(block - 1);
            System::initialize(
                &block,
                &H256::from_low_u64_be(block as u64),
                &Default::default(),
            );
            let subnet_id = 11u32;
            let subnet_epoch = 7u32;
            crate::SubnetNodeElectionSlots::<Test>::insert(subnet_id, vec![10, 20, 30, 40]);
            for node_id in [10u32, 20, 30, 40] {
                crate::SubnetNodeValidatorId::<Test>::insert(subnet_id, node_id, node_id);
            }
            Network::elect_validator(subnet_id, subnet_epoch, block);
            crate::SubnetElectedValidator::<Test>::get(subnet_id, subnet_epoch)
                .expect("a nonempty attributable candidate pool elects a validator")
                .validator_subnet_node_id
        })
    };
    assert_eq!(elect_at(41), elect_at(42));
}
