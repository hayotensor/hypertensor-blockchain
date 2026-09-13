//! Block subsidies paid to EVM accounts authorized by their Aura authority.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;
pub use pallet::*;
pub use weights::WeightInfo;

use alloc::vec::Vec;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
    sp_runtime::{
        traits::{CheckedAdd, One, Zero},
        RuntimeDebug, SaturatedConversion,
    },
    traits::{Contains, Currency, FindAuthor, Get},
    weights::Weight,
};
use frame_system::pallet_prelude::*;
use pallet_evm::AddressMapping;
use scale_info::TypeInfo;
use sp_core::{sr25519, H160};

/// SCALE-encoded as a `Vec<u8>` in the ownership proof, not a fixed-size array.
pub const REWARD_ADDRESS_DOMAIN: &[u8] = b"hypertensor/author-subsidy/set-reward-address/v1";

/// One bounded record per Aura key. Changes take effect at the next block boundary.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct RewardAddressRecord<BlockNumber> {
    pub current_address: Option<H160>,
    pub pending_address: H160,
    pub activation_block: BlockNumber,
    pub next_nonce: u64,
}

impl<BlockNumber: PartialOrd> RewardAddressRecord<BlockNumber> {
    pub fn address_at(&self, block: &BlockNumber) -> Option<H160> {
        if block >= &self.activation_block {
            Some(self.pending_address)
        } else {
            self.current_address
        }
    }
}

/// Supplies a real author digest and the worst-case authority set for runtime benchmarks.
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper {
    fn setup_author(authority: sr25519::Public);
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId>;
        /// Resolves the current block's author to an effective, verified payout address.
        type FindAuthor: FindAuthor<H160>;
        type AddressMapping: AddressMapping<Self::AccountId>;
        /// Read-only membership check against the current Aura authority set.
        type IsAuraAuthority: Contains<sr25519::Public>;
        #[pallet::constant]
        type AuthorBlockEmissions: Get<u128>;
        type WeightInfo: WeightInfo;
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: BenchmarkHelper;
    }

    #[pallet::storage]
    #[pallet::getter(fn reward_addresses)]
    pub type RewardAddresses<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        sr25519::Public,
        RewardAddressRecord<BlockNumberFor<T>>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AuthorSubsidy {
            who: T::AccountId,
            subsidy: u128,
        },
        RewardAddressScheduled {
            aura_key: sr25519::Public,
            reward_address: H160,
            activation_block: BlockNumberFor<T>,
            nonce: u64,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        WrongRewardAccount,
        UnknownAuraAuthority,
        ZeroRewardAddress,
        InvalidNonce,
        ExpiredProof,
        InvalidAuraSignature,
        NonceOverflow,
        BlockNumberOverflow,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Configure a payout using authorization from the receiving account and the Aura key.
        /// The receiving account signs the extrinsic; the Aura key signs `reward_address_payload`.
        /// A new destination becomes effective in the next block, including for EVM author lookup.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_reward_address().max(T::WeightInfo::update_reward_address()))]
        pub fn set_reward_address(
            origin: OriginFor<T>,
            aura_key: sr25519::Public,
            reward_address: H160,
            nonce: u64,
            valid_until: BlockNumberFor<T>,
            aura_signature: sr25519::Signature,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!reward_address.is_zero(), Error::<T>::ZeroRewardAddress);
            ensure!(
                who == T::AddressMapping::into_account_id(reward_address),
                Error::<T>::WrongRewardAccount
            );
            ensure!(
                T::IsAuraAuthority::contains(&aura_key),
                Error::<T>::UnknownAuraAuthority
            );
            let block = frame_system::Pallet::<T>::block_number();
            ensure!(block <= valid_until, Error::<T>::ExpiredProof);
            let previous = RewardAddresses::<T>::get(aura_key);
            ensure!(
                nonce == previous.as_ref().map_or(0, |r| r.next_nonce),
                Error::<T>::InvalidNonce
            );
            let next_nonce = nonce.checked_add(1).ok_or(Error::<T>::NonceOverflow)?;
            let activation_block = block
                .checked_add(&One::one())
                .ok_or(Error::<T>::BlockNumberOverflow)?;
            let payload =
                Self::reward_address_payload(&aura_key, reward_address, nonce, valid_until);
            ensure!(
                sp_io::crypto::sr25519_verify(&aura_signature, &payload, &aura_key),
                Error::<T>::InvalidAuraSignature
            );

            RewardAddresses::<T>::insert(
                aura_key,
                RewardAddressRecord {
                    current_address: previous.and_then(|r| r.address_at(&block)),
                    pending_address: reward_address,
                    activation_block,
                    next_nonce,
                },
            );
            Self::deposit_event(Event::RewardAddressScheduled {
                aura_key,
                reward_address,
                activation_block,
                nonce,
            });
            Ok(())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_block_number: BlockNumberFor<T>) -> Weight {
            let digest = frame_system::Pallet::<T>::digest();
            let digests = digest.logs.iter().filter_map(|d| d.as_pre_runtime());
            let Some(author) = T::FindAuthor::find_author(digests).filter(|a| !a.is_zero()) else {
                return T::WeightInfo::on_initialize_skipped();
            };
            let account_id = T::AddressMapping::into_account_id(author);
            let subsidy = T::AuthorBlockEmissions::get();
            drop(T::Currency::deposit_creating(
                &account_id,
                subsidy.saturated_into::<BalanceOf<T>>(),
            ));
            Self::deposit_event(Event::AuthorSubsidy {
                who: account_id,
                subsidy,
            });
            T::WeightInfo::on_initialize()
        }
    }

    impl<T: Config> Pallet<T> {
        /// Resolve an already-identified authority. The runtime checks authority membership.
        pub fn reward_address_at(
            aura_key: &sr25519::Public,
            block: BlockNumberFor<T>,
        ) -> Option<H160> {
            RewardAddresses::<T>::get(aura_key).and_then(|r| r.address_at(&block))
        }

        pub fn next_nonce(aura_key: &sr25519::Public) -> u64 {
            RewardAddresses::<T>::get(aura_key).map_or(0, |r| r.next_nonce)
        }

        /// Canonical SCALE tuple: `(Vec<u8>, T::Hash, sr25519::Public, H160, u64, BlockNumber)`.
        /// No additional hashing or wallet-specific message wrapping is applied to this proof.
        pub fn reward_address_payload(
            aura_key: &sr25519::Public,
            reward_address: H160,
            nonce: u64,
            valid_until: BlockNumberFor<T>,
        ) -> Vec<u8> {
            (
                REWARD_ADDRESS_DOMAIN.to_vec(),
                frame_system::Pallet::<T>::block_hash(BlockNumberFor::<T>::zero()),
                aura_key,
                reward_address,
                nonce,
                valid_until,
            )
                .encode()
        }
    }
}
