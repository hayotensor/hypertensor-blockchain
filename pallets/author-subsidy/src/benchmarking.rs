//! Benchmarks include Babe lookup, ownership verification and the largest payout record.
#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::traits::Hooks;
use frame_system::RawOrigin;

fn setup<T: Config>() -> (sr25519::Public, H160) {
    frame_system::Pallet::<T>::set_block_number(2u32.into());
    let key = sp_io::crypto::sr25519_generate(
        sp_core::crypto::KeyTypeId(*b"babe"),
        Some(b"//AuthorSubsidyBenchmark".to_vec()),
    );
    T::BenchmarkHelper::setup_author(key);
    (key, H160::repeat_byte(42))
}
fn signature<T: Config>(key: &sr25519::Public, address: H160, nonce: u64) -> sr25519::Signature {
    let payload = Pallet::<T>::reward_address_payload(key, address, nonce, 100u32.into());
    sp_io::crypto::sr25519_sign(sp_core::crypto::KeyTypeId(*b"babe"), key, &payload).unwrap()
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn set_reward_address() {
        let (key, address) = setup::<T>();
        let caller = T::AddressMapping::into_account_id(address);
        let proof = signature::<T>(&key, address, 0);
        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller),
            key,
            address,
            0,
            100u32.into(),
            proof,
        );
        assert_eq!(Pallet::<T>::next_nonce(&key), 1);
        assert_eq!(Pallet::<T>::reward_address_at(&key, 2u32.into()), None);
        assert_eq!(
            Pallet::<T>::reward_address_at(&key, 3u32.into()),
            Some(address)
        );
    }

    #[benchmark]
    fn update_reward_address() {
        let (key, address) = setup::<T>();
        RewardAddresses::<T>::insert(
            key,
            RewardAddressRecord {
                current_address: Some(H160::repeat_byte(1)),
                pending_address: H160::repeat_byte(2),
                activation_block: 2u32.into(),
                next_nonce: 1,
            },
        );
        let caller = T::AddressMapping::into_account_id(address);
        let proof = signature::<T>(&key, address, 1);
        #[extrinsic_call]
        set_reward_address(
            RawOrigin::Signed(caller),
            key,
            address,
            1,
            100u32.into(),
            proof,
        );
        assert_eq!(Pallet::<T>::next_nonce(&key), 2);
        assert_eq!(
            Pallet::<T>::reward_address_at(&key, 2u32.into()),
            Some(H160::repeat_byte(2))
        );
        assert_eq!(
            Pallet::<T>::reward_address_at(&key, 3u32.into()),
            Some(address)
        );
    }

    #[benchmark]
    fn on_initialize() {
        let (key, address) = setup::<T>();
        RewardAddresses::<T>::insert(
            key,
            RewardAddressRecord {
                current_address: Some(H160::repeat_byte(1)),
                pending_address: address,
                activation_block: 2u32.into(),
                next_nonce: 1,
            },
        );
        let who = T::AddressMapping::into_account_id(address);
        let before = T::Currency::free_balance(&who);
        #[block]
        {
            Pallet::<T>::on_initialize(2u32.into());
        }
        assert_eq!(
            T::Currency::free_balance(&who),
            before + T::AuthorBlockEmissions::get().saturated_into::<BalanceOf<T>>()
        );
        let event: <T as Config>::RuntimeEvent = Event::<T>::AuthorSubsidy {
            who,
            subsidy: T::AuthorBlockEmissions::get(),
        }
        .into();
        frame_system::Pallet::<T>::assert_last_event(event.into());
    }

    #[benchmark]
    fn on_initialize_skipped() {
        let (key, address) = setup::<T>();
        // An identified author with a pending first configuration exercises lookup without minting.
        RewardAddresses::<T>::insert(
            key,
            RewardAddressRecord {
                current_address: None,
                pending_address: address,
                activation_block: 3u32.into(),
                next_nonce: 1,
            },
        );
        let before = T::Currency::total_issuance();
        #[block]
        {
            Pallet::<T>::on_initialize(2u32.into());
        }
        assert_eq!(T::Currency::total_issuance(), before);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
