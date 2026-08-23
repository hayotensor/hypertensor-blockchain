//! Benchmarking setup for pallet-authory-subsidy
// frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm --extrinsic "" --pallet "pallet_author_subsidy" --output pallets/author-subsidy/src/weights.rs --template ./.maintain/frame-weight-template.hbs

// frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm --extrinsic "" --pallet "pallet_author_subsidy"

// cargo build --release --features runtime-benchmarks
// cargo test --release --features runtime-benchmarks
// Build only this pallet
// cargo build --package pallet-network --features runtime-benchmarks
// cargo build --package pallet-collective --features runtime-benchmarks
// cargo +nightly build --release --features runtime-benchmarks

#![cfg(feature = "runtime-benchmarks")]
use super::*;
#[allow(unused_imports)]
use crate::Pallet as AuthorSubsidy;
use frame_benchmarking::v2::*;
use frame_support::{
    sp_runtime::traits::{SaturatedConversion, Saturating},
    traits::{Currency, FindAuthor, Get, Hooks},
};
use pallet_evm::AddressMapping;

fn assert_last_event<T: Config>(event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(event.into());
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn on_initialize() {
        let block_number = 1u32.into();
        frame_system::Pallet::<T>::set_block_number(block_number);

        let digest = frame_system::Pallet::<T>::digest();
        let author =
            T::FindAuthor::find_author(digest.logs.iter().filter_map(|item| item.as_pre_runtime()))
                .unwrap_or_default();
        let who = T::AddressMapping::into_account_id(author);
        let balance_before = T::Currency::free_balance(&who);
        let subsidy_u128 = T::AuthorBlockEmissions::get();
        let subsidy: BalanceOf<T> = subsidy_u128.saturated_into();

        #[block]
        {
            let _ = Pallet::<T>::on_initialize(block_number);
        }

        assert_eq!(
            T::Currency::free_balance(&who),
            balance_before.saturating_add(subsidy)
        );
        assert_last_event::<T>(
            Event::<T>::AuthorSubsidy {
                who,
                subsidy: subsidy_u128,
            }
            .into(),
        );
    }

    impl_benchmark_test_suite!(
        AuthorSubsidy,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}
