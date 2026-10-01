//! The Substrate Node Template runtime. This can be compiled with `#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]
#![allow(clippy::new_without_default, clippy::or_fun_call)]
#![cfg_attr(feature = "runtime-benchmarks", warn(unused_crate_dependencies))]

extern crate alloc;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use alloc::{borrow::Cow, vec, vec::Vec};
use codec::{Decode, Encode, MaxEncodedLen};
use sp_api::impl_runtime_apis;
use sp_consensus_grandpa::{AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList};
use sp_core::{crypto::KeyTypeId, ConstU128, OpaqueMetadata, H256};
use sp_runtime::{
    generic, impl_opaque_keys,
    traits::{
        BlakeTwo256, Block as BlockT, IdentifyAccount, IdentityLookup, NumberFor, One, Verify,
    },
    transaction_validity::{TransactionSource, TransactionValidity},
    ApplyExtrinsicResult, ExtrinsicInclusionMode, Perbill, Permill, Perquintill,
};
use sp_version::RuntimeVersion;
// Substrate FRAME
use frame_support::traits::{
    fungible::HoldConsideration, EqualPrivilegeOnly, InstanceFilter, LinearStoragePrice,
};
use frame_support::traits::{Contains, KeyOwnerProofSystem};
#[cfg(feature = "with-paritydb-weights")]
use frame_support::weights::constants::ParityDbWeight as RuntimeDbWeight;
#[cfg(feature = "with-rocksdb-weights")]
use frame_support::weights::constants::RocksDbWeight as RuntimeDbWeight;
use frame_support::{
    derive_impl,
    genesis_builder_helper::get_preset,
    parameter_types,
    traits::{
        tokens::{PayFromAccount, UnityAssetBalanceConversion},
        ConstU32, ConstU64, ConstU8,
    },
    weights::{constants::WEIGHT_REF_TIME_PER_MILLIS, ConstantMultiplier, Weight},
    PalletId,
};
use pallet_transaction_payment::{FungibleAdapter, TargetedFeeAdjustment};
use pallet_tx_pause::RuntimeCallNameOf;
mod apis;
pub mod configs;
pub mod genesis;
pub mod tokenomics;

pub use apis::*;
pub use configs::{common::*, npos::*, revive::EthExtraImpl};

#[cfg(test)]
mod tests;

// A few exports that help ease life for downstream crates.
pub use frame_system::Call as SystemCall;
pub use frame_system::{EnsureRoot, EnsureRootWithSuccess, EnsureWithSuccess};

pub use pallet_balances::Call as BalancesCall;
pub use pallet_timestamp::Call as TimestampCall;
use pallet_transaction_payment::Multiplier;

/// Type of block number.
pub type BlockNumber = u32;

/// Native signature supporting the SDK signing schemes.
pub type Signature = sp_runtime::MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The type for looking up accounts. We don't expect more than 4 billion of them, but you
/// never know...
pub type AccountIndex = u32;

/// Balance of an account.
pub type Balance = u128;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = H256;

/// The hashing algorithm used by the chain.
pub type Hashing = BlakeTwo256;

/// Digest item type.
pub type DigestItem = generic::DigestItem;

/// The address format for describing accounts.
pub type Address = AccountId;

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;

/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;

/// Transaction validation and post-dispatch weight reclamation.
pub type TxExtension = (
    frame_system::AuthorizeCall<Runtime>,
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckMortality<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
    pallet_revive::evm::tx_extension::SetOrigin<Runtime>,
    frame_system::WeightReclaim<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    pallet_revive::evm::runtime::UncheckedExtrinsic<Address, Signature, EthExtraImpl>;

/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, TxExtension>;

/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, TxExtension>;

type Migrations = ();

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
    Migrations,
>;

// Time is measured by number of blocks.
pub const MILLISECS_PER_BLOCK: u64 = 6000;
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;
pub const YEAR: BlockNumber = DAYS * 365; // 5256000

// Blocks per epoch
// pub const BLOCKS_PER_EPOCH: u32 = 300; // Mainnet | 30 minutes/e
// pub const BLOCKS_PER_EPOCH: u32 = 100; // Testnet | 10 minutes/e
pub const BLOCKS_PER_EPOCH: u32 = 20; // Local | 2 minutes/e
pub const EPOCHS_PER_YEAR: u32 = (YEAR as u32) / BLOCKS_PER_EPOCH;

pub const TENSOR: u128 = 1_000_000_000_000_000_000; // 1e18

pub const OVERWATCH_YEARLY_EMISSIONS: u128 = tokenomics::POLICY.overwatch_annual_emissions;
pub const OVERWATCH_EPOCH_EMISSIONS: u128 = OVERWATCH_YEARLY_EMISSIONS / (EPOCHS_PER_YEAR as u128);

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
    use super::*;

    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;

    impl_opaque_keys! {
        pub struct SessionKeys {
            pub babe: Babe,
            pub grandpa: Grandpa,
        }
    }
}

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: Cow::Borrowed("hypertensor-node"),
    impl_name: Cow::Borrowed("hypertensor-node"),
    authoring_version: 1,
    spec_version: 3,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
    system_version: 1,
};

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> sp_version::NativeVersion {
    sp_version::NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2000ms of compute with a 6 second average block time.
pub const WEIGHT_MILLISECS_PER_BLOCK: u64 = 2000;
pub const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_parts(
    WEIGHT_MILLISECS_PER_BLOCK * WEIGHT_REF_TIME_PER_MILLIS,
    // Revive needs a finite proof budget. Half remains available to Network's
    // bounded settlement/election hooks, whose maximum exceeds 5 MiB.
    32 * 1024 * 1024,
);
pub const MAXIMUM_BLOCK_LENGTH: u32 = 5 * 1024 * 1024;

// Create the runtime by composing the FRAME pallets that were previously configured.
// Balances and Staking initialize before Session; Session rotates before
// Authorship resolves the author.
#[frame_support::runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeEvent,
        RuntimeCall,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask
    )]
    pub struct Runtime;

    #[runtime::pallet_index(0)]
    pub type System = frame_system;

    #[runtime::pallet_index(1)]
    pub type Timestamp = pallet_timestamp;

    #[runtime::pallet_index(2)]
    pub type Grandpa = pallet_grandpa;

    #[runtime::pallet_index(3)]
    pub type Balances = pallet_balances;

    #[runtime::pallet_index(4)]
    pub type TransactionPayment = pallet_transaction_payment;

    #[runtime::pallet_index(5)]
    pub type Sudo = pallet_sudo;

    #[runtime::pallet_index(6)]
    pub type AtomicSwap = pallet_atomic_swap;

    #[runtime::pallet_index(7)]
    pub type Randomness = pallet_randomness;

    #[runtime::pallet_index(8)]
    pub type Utility = pallet_utility;

    #[runtime::pallet_index(9)]
    pub type Proxy = pallet_proxy;

    #[runtime::pallet_index(10)]
    pub type Preimage = pallet_preimage;

    #[runtime::pallet_index(12)]
    pub type Treasury = pallet_treasury;

    #[runtime::pallet_index(13)]
    pub type Multisig = pallet_multisig;

    #[runtime::pallet_index(14)]
    pub type TxPause = pallet_tx_pause;

    #[runtime::pallet_index(15)]
    pub type Collective = pallet_collective::Pallet<Runtime, Instance1>;

    #[runtime::pallet_index(17)]
    pub type Babe = pallet_babe;

    #[runtime::pallet_index(18)]
    pub type Staking = pallet_staking;

    #[runtime::pallet_index(19)]
    pub type Session = pallet_session;

    // A BABE boundary block is signed by the newly activated set. Resolve its
    // author only after Session updates both validator indices and the active era.
    #[runtime::pallet_index(20)]
    pub type Authorship = pallet_authorship;

    #[runtime::pallet_index(21)]
    pub type Historical = pallet_session::historical;

    #[runtime::pallet_index(22)]
    pub type Offences = pallet_offences;

    #[runtime::pallet_index(23)]
    pub type Revive = pallet_revive;

    // Network reads BABE epoch randomness. Its hooks must run after BABE and
    // Session have initialized and completed any epoch rotation for this block.
    #[runtime::pallet_index(24)]
    pub type Network = pallet_network;

    // The runtime macro orders hooks by pallet index. Run Scheduler last so its
    // remaining-block-weight meter includes Network and consensus initialization.
    #[runtime::pallet_index(25)]
    pub type Scheduler = pallet_scheduler;

    // Stateless configuration and benchmarks; no hooks or contract storage.
    #[runtime::pallet_index(26)]
    pub type NetworkPrecompiles = network_precompiles;
}
