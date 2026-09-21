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
    genesis_builder_helper::{build_state, get_preset},
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
pub mod genesis_config_presets;
// This module is compiled into this same runtime. It groups consensus/staking
// configuration; pallet composition and runtime APIs remain in this file.
pub mod npos;
pub use npos::*;
pub mod revive;
pub use revive::EthExtraImpl;

#[cfg(test)]
mod npos_tests;
#[cfg(test)]
mod production_tests;
#[cfg(test)]
mod revive_tests;

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

pub const OVERWATCH_YEARLY_EMISSIONS: u128 = 10_000 * TENSOR;
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

parameter_types! {
    pub const Version: RuntimeVersion = VERSION;
    pub const BlockHashCount: BlockNumber = 256;
    pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights
        ::with_sensible_defaults(MAXIMUM_BLOCK_WEIGHT, NORMAL_DISPATCH_RATIO);
    pub BlockLength: frame_system::limits::BlockLength = frame_system::limits::BlockLength
        ::builder()
        .max_length(MAXIMUM_BLOCK_LENGTH)
        .modify_max_length_for_class(
            frame_support::dispatch::DispatchClass::Normal,
            |max| *max = NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_LENGTH,
        )
        .build();
    pub const SS58Prefix: u8 = 42;
}

// Configure FRAME pallets to include in runtime.
#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
    type BaseCallFilter = TxPause;
    /// Block & extrinsics weights: base values and limits.
    type BlockWeights = BlockWeights;
    /// The maximum length of a block (in bytes).
    type BlockLength = BlockLength;
    /// The index type for storing how many extrinsics an account has signed.
    type Nonce = Nonce;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// The hashing algorithm used.
    type Hashing = Hashing;
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
    type Lookup = IdentityLookup<AccountId>;
    /// The block type.
    type Block = Block;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// The weight of database operations that the runtime can invoke.
    type DbWeight = RuntimeDbWeight;
    type SystemWeightInfo = frame_system::weights::SubstrateWeight<Runtime>;
    type ExtensionsWeightInfo = frame_system::SubstrateExtensionsWeight<Runtime>;
    /// Version of the runtime.
    type Version = Version;
    /// The data to be stored in an account.
    type AccountData = pallet_balances::AccountData<Balance>;
    /// This is used as an identifier of the chain. 42 is the generic substrate prefix.
    type SS58Prefix = SS58Prefix;
    type MaxConsumers = ConstU32<16>;
}

impl pallet_timestamp::Config for Runtime {
    type Moment = u64;
    type OnTimestampSet = Babe;
    type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
    type WeightInfo = pallet_timestamp::weights::SubstrateWeight<Runtime>;
}

pub const MILLI_TENSOR: Balance = TENSOR / 1_000;
pub const MICRO_TENSOR: Balance = TENSOR / 1_000_000;
pub const EXISTENTIAL_DEPOSIT: Balance = MILLI_TENSOR;

impl pallet_balances::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Self>;
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ConstU32<50>;
    type MaxFreezes = ConstU32<50>;
    type DoneSlashHandler = ();
}

parameter_types! {
    pub const TransactionByteFee: Balance = MICRO_TENSOR;
    pub TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
    pub AdjustmentVariable: Multiplier = Multiplier::from_rational(1, 100_000);
    // Revive requires minimum_multiplier * NativeToEthRatio >= 1.
    pub MinimumMultiplier: Multiplier = Multiplier::one();
    pub MaximumMultiplier: Multiplier = Multiplier::from_u32(1_000_000);
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = FungibleAdapter<Balances, ()>;
    // A full block costs 0.002 TENSOR in weight fees at the minimum multiplier.
    // Proof-size consumption is priced against the same block budget.
    type WeightToFee = pallet_revive::evm::fees::BlockRatioFee<1000, 1, Runtime, Balance>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type FeeMultiplierUpdate = TargetedFeeAdjustment<
        Runtime,
        TargetBlockFullness,
        AdjustmentVariable,
        MinimumMultiplier,
        MaximumMultiplier,
    >;
    type OperationalFeeMultiplier = ConstU8<5>;
    type WeightInfo = pallet_transaction_payment::weights::SubstrateWeight<Runtime>;
}

impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_sudo::weights::SubstrateWeight<Self>;
}

impl pallet_atomic_swap::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SwapAction = pallet_atomic_swap::BalanceSwapAction<AccountId, Balances>;
    type ProofLimit = ConstU32<1024>;
}

impl pallet_randomness::Config for Runtime {}

impl pallet_utility::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type PalletsOrigin = OriginCaller;
    type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
}

pub const fn deposit(items: u32, bytes: u32) -> Balance {
    const ITEMS_FEE: Balance = TENSOR / 10;
    const BYTES_FEE: Balance = 10 * MICRO_TENSOR;
    (items as Balance)
        .saturating_mul(ITEMS_FEE)
        .saturating_add((bytes as Balance).saturating_mul(BYTES_FEE))
}

parameter_types! {
    // One storage item; key size 32, value size 8; .
    pub const ProxyDepositBase: Balance = deposit(1, 8);
    // Additional storage item size of 33 bytes.
    pub const ProxyDepositFactor: Balance = deposit(0, 33);
    pub const AnnouncementDepositBase: Balance = deposit(1, 8);
    pub const AnnouncementDepositFactor: Balance = deposit(0, 66);
}

/// The type used to represent the kinds of proxying allowed.
#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Encode,
    Decode,
    codec::DecodeWithMemTracking,
    Debug,
    MaxEncodedLen,
    scale_info::TypeInfo,
)]
pub enum ProxyType {
    Any,
    NonTransfer,
    Transfer,
    SubNetworkStaking,
    SubNetworkDelegateStaking,
    // SubnetworkOwner,
    Governance,
    // Sudo,
    CancelProxy,
}
impl Default for ProxyType {
    fn default() -> Self {
        Self::Any
    }
}

fn is_network_transfer_call(call: &pallet_network::Call<Runtime>) -> bool {
    matches!(
        call,
        pallet_network::Call::transfer_delegate_stake { .. }
            | pallet_network::Call::transfer_validator_delegate_stake { .. }
    )
}

fn is_network_staking_call(call: &pallet_network::Call<Runtime>) -> bool {
    matches!(
        call,
        pallet_network::Call::add_node_stake { .. }
            | pallet_network::Call::remove_node_stake { .. }
            | pallet_network::Call::add_overwatch_node_stake { .. }
            | pallet_network::Call::remove_overwatch_node_stake { .. }
    )
}

fn is_network_delegate_staking_call(call: &pallet_network::Call<Runtime>) -> bool {
    matches!(
        call,
        pallet_network::Call::add_subnet_delegate_stake { .. }
            | pallet_network::Call::swap_from_subnet_to_subnet { .. }
            | pallet_network::Call::remove_delegate_stake { .. }
            | pallet_network::Call::add_validator_delegate_stake { .. }
            | pallet_network::Call::remove_validator_delegate_stake { .. }
            | pallet_network::Call::swap_from_validator_to_validator { .. }
            | pallet_network::Call::swap_from_validator_to_subnet { .. }
            | pallet_network::Call::swap_from_subnet_to_validator { .. }
            | pallet_network::Call::update_swap_queue { .. }
            | pallet_network::Call::remove_delegate_account_balance { .. }
    )
}

impl InstanceFilter<RuntimeCall> for ProxyType {
    fn filter(&self, c: &RuntimeCall) -> bool {
        match self {
            ProxyType::Any => true,
            ProxyType::NonTransfer => match c {
                // Fail closed: privileged dispatchers, multisig, swaps, contracts,
                // reward redirection, and future pallets must not inherit permission.
                RuntimeCall::System(
                    frame_system::Call::remark { .. }
                    | frame_system::Call::remark_with_event { .. },
                ) => true,
                RuntimeCall::Session(
                    pallet_session::Call::set_keys { .. } | pallet_session::Call::purge_keys { .. },
                ) => true,
                RuntimeCall::Staking(
                    pallet_staking::Call::bond_extra { .. }
                    | pallet_staking::Call::unbond { .. }
                    | pallet_staking::Call::withdraw_unbonded { .. }
                    | pallet_staking::Call::validate { .. }
                    | pallet_staking::Call::nominate { .. }
                    | pallet_staking::Call::chill { .. }
                    | pallet_staking::Call::rebond { .. }
                    | pallet_staking::Call::payout_stakers { .. }
                    | pallet_staking::Call::payout_stakers_by_page { .. },
                ) => true,
                RuntimeCall::Proxy(pallet_proxy::Call::reject_announcement { .. }) => true,
                _ => false,
            },
            ProxyType::Transfer => match c {
                RuntimeCall::Balances(
                    pallet_balances::Call::transfer_keep_alive { .. }
                    | pallet_balances::Call::transfer_allow_death { .. }
                    | pallet_balances::Call::transfer_all { .. },
                ) => true,
                RuntimeCall::Network(call) => is_network_transfer_call(call),
                _ => false,
            },
            ProxyType::Governance => matches!(
                c,
                RuntimeCall::Collective(..) | RuntimeCall::Treasury(..) | RuntimeCall::Utility(..)
            ),
            ProxyType::SubNetworkStaking => {
                matches!(c, RuntimeCall::Network(call) if is_network_staking_call(call))
            }
            ProxyType::SubNetworkDelegateStaking => {
                matches!(c, RuntimeCall::Network(call) if is_network_delegate_staking_call(call))
            }
            ProxyType::CancelProxy => {
                matches!(
                    c,
                    RuntimeCall::Proxy(pallet_proxy::Call::reject_announcement { .. })
                )
            }
        }
    }
    fn is_superset(&self, o: &Self) -> bool {
        match (self, o) {
            (x, y) if x == y => true,
            (ProxyType::Any, _) => true,
            (_, ProxyType::Any) => false,
            (ProxyType::NonTransfer, ProxyType::CancelProxy) => true,
            _ => false,
        }
    }
}

impl pallet_proxy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type ProxyType = ProxyType;
    type ProxyDepositBase = ProxyDepositBase;
    type ProxyDepositFactor = ProxyDepositFactor;
    type MaxProxies = ConstU32<32>;
    type WeightInfo = pallet_proxy::weights::SubstrateWeight<Runtime>;
    type MaxPending = ConstU32<32>;
    type CallHasher = BlakeTwo256;
    type AnnouncementDepositBase = AnnouncementDepositBase;
    type AnnouncementDepositFactor = AnnouncementDepositFactor;
    type BlockNumberProvider = System;
}

parameter_types! {
    pub const PreimageBaseDeposit: Balance = deposit(2, 64);
    pub const PreimageByteDeposit: Balance = deposit(0, 1);
    pub const PreimageHoldReason: RuntimeHoldReason = RuntimeHoldReason::Preimage(pallet_preimage::HoldReason::Preimage);
}

impl pallet_preimage::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_preimage::weights::SubstrateWeight<Runtime>;
    type Currency = Balances;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type Consideration = HoldConsideration<
        AccountId,
        Balances,
        PreimageHoldReason,
        LinearStoragePrice<PreimageBaseDeposit, PreimageByteDeposit, Balance>,
    >;
}

parameter_types! {
    // Leave at least 10% for Timestamp and extrinsics after mandatory hooks.
    pub MaximumSchedulerWeight: Weight = (Perbill::from_percent(80) * MAXIMUM_BLOCK_WEIGHT)
        .min(MAXIMUM_BLOCK_WEIGHT
            .saturating_sub(System::block_weight().total())
            .saturating_sub(Perbill::from_percent(10) * MAXIMUM_BLOCK_WEIGHT));
}

impl pallet_scheduler::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = ConstU32<100>;
    type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Runtime>;
    type OriginPrivilegeCmp = EqualPrivilegeOnly;
    type Preimages = Preimage;
    type BlockNumberProvider = System;
}

parameter_types! {
    // Retain treasury funds unless an explicit spending/burning policy is approved.
    pub const Burn: Permill = Permill::zero();
    pub const TreasurySpendPeriod: BlockNumber = DAYS;
    pub const TreasuryPayoutPeriod: BlockNumber = 30 * DAYS;
    pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
    pub const SpendLimit: Balance = u128::MAX;
    pub TreasuryAccount: AccountId = Treasury::account_id();
}

impl pallet_treasury::Config for Runtime {
    type PalletId = TreasuryPalletId;
    type Currency = Balances;
    type RejectOrigin = EnsureRoot<AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type SpendPeriod = TreasurySpendPeriod;
    type Burn = Burn;
    type BurnDestination = (); // Just gets burned.
    type WeightInfo = ();
    type SpendFunds = ();
    type MaxApprovals = ConstU32<100>;
    type SpendOrigin = EnsureRootWithSuccess<AccountId, SpendLimit>;
    type AssetKind = ();
    type Beneficiary = AccountId;
    type BeneficiaryLookup = IdentityLookup<Self::Beneficiary>;
    type Paymaster = PayFromAccount<Balances, TreasuryAccount>;
    type BalanceConverter = UnityAssetBalanceConversion;
    type PayoutPeriod = TreasuryPayoutPeriod;
    // #[cfg(feature = "runtime-benchmarks")]
    // type BenchmarkHelper = ();
    type BlockNumberProvider = System;
}

parameter_types! {
    // One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
    pub const DepositBase: Balance = deposit(1, 88);
    // Additional storage item size of 32 bytes.
    pub const DepositFactor: Balance = deposit(0, 32);
    pub const MaxSignatories: u32 = 100;
}

impl pallet_multisig::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = pallet_multisig::weights::SubstrateWeight<Runtime>;
    type BlockNumberProvider = System;
}

parameter_types! {
    pub const MaxNameLen: u32 = 256;
}

/// Calls that cannot be paused by the tx-pause pallet.
pub struct TxPauseWhitelistedCalls;
impl Contains<RuntimeCallNameOf<Runtime>> for TxPauseWhitelistedCalls {
    fn contains(full_name: &RuntimeCallNameOf<Runtime>) -> bool {
        // Timestamp is mandatory for block validity. Sudo is the root recovery
        // path; pausing it could make every existing pause irreversible.
        matches!(
            (full_name.0.as_slice(), full_name.1.as_slice()),
            (b"Timestamp", b"set") | (b"Sudo", _)
        )
    }
}

impl pallet_tx_pause::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type PauseOrigin = EnsureRoot<AccountId>;
    type UnpauseOrigin = EnsureRoot<AccountId>;
    type WhitelistedCalls = TxPauseWhitelistedCalls;
    type MaxNameLen = MaxNameLen;
    type WeightInfo = pallet_tx_pause::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const CouncilMotionDuration: BlockNumber = 5 * DAYS;
    pub const CouncilMaxProposals: u32 = 100;
    pub const CouncilMaxMembers: u32 = 100;
    pub MaxCollectivesProposalWeight: Weight = Perbill::from_percent(50) * BlockWeights::get().max_block;
}

type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Config<CouncilCollective> for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = CouncilMotionDuration;
    type MaxProposals = CouncilMaxProposals;
    type MaxMembers = CouncilMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
    type SetMembersOrigin = EnsureRoot<AccountId>;
    type MaxProposalWeight = MaxCollectivesProposalWeight;
}

parameter_types! {
    pub const InitialTxRateLimit: u32 = 0;
    pub const InitialMinSubnetDelegateStakeBalance: u128 = 100_000_000_000_000_000_000;
    pub const EpochLength: u32 = BLOCKS_PER_EPOCH; // Testnet 600 blocks per erpoch / 69 mins per epoch, Local 10
    pub const EpochsPerYear: u32 = EPOCHS_PER_YEAR; // Testnet 600 blocks per erpoch / 69 mins per epoch, Local 10
    pub const NetworkPalletId: PalletId = PalletId(*b"/network");
    pub const OverwatchEpochEmissions: u128 = OVERWATCH_EPOCH_EMISSIONS;
    pub MaximumHooksWeight: Weight = Perbill::from_percent(50) *
        BlockWeights::get().max_block;
    pub const NetworkMinAttestationPercentage: u128 = 666_666_666_666_666_666;
    pub const NetworkSuperMajorityAttestationRatio: u128 = 875_000_000_000_000_000;
    pub const NetworkInitialSubnetUid: u32 = 128_000;
    pub const NetworkMaxPhysicalSubnetsUpperBound: u32 =
        pallet_network::physical_subnet_upper_bound(BLOCKS_PER_EPOCH);
    pub const NetworkMaxSubnetNodesUpperBound: u32 = 512;
    pub const NetworkMaxValidatorNodesUpperBound: u32 = 512;
    pub const NetworkMaxOverwatchNodesUpperBound: u32 = 64;
    pub const NetworkMaxOverwatchCommitCutoffPercent: u128 = 950_000_000_000_000_000;
    pub const NetworkMaxBootnodesUpperBound: u32 = 256;
    pub const NetworkMaxSubnetBootnodeAccessUpperBound: u32 = 256;
    pub const NetworkMaxChurnLimitUpperBound: u32 = 64;
    pub const NetworkMaxRegisteredNodesUpperBound: u32 = 64;
    pub const NetworkMaxUnbondingsUpperBound: u32 = 256;
    pub const NetworkMaxSwapCallsPerBlockUpperBound: u32 = 1_000;
    pub const NetworkMaxEmergencySubnetNodesUpperBound: u32 = 64;
    pub const DesignatedEpochSlots: u32 = pallet_network::NETWORK_DESIGNATED_EPOCH_SLOTS;
    pub const NetworkMaxVectorLength: u32 = 1024;
    pub const NetworkMaxUrlLength: u32 = 1024;
    pub const NetworkMaxSocialIdLength: u32 = 255;
    pub const NetworkValidatorArgsLimit: u32 = 4096;
    pub const NetworkMaxOverwatchRevealSaltLength: u32 = 64;
    pub const NetworkMaxSwapQueueLength: u32 = 1000;
}

impl pallet_network::Config for Runtime {
    type WeightInfo = pallet_network::weights::SubstrateWeight<Runtime>;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MajorityCollectiveOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>;
    type SuperMajorityCollectiveOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 4, 5>;
    type EpochLength = EpochLength;
    type EpochsPerYear = EpochsPerYear;
    type InitialTxRateLimit = InitialTxRateLimit;
    type InitialMinSubnetDelegateStakeBalance = InitialMinSubnetDelegateStakeBalance;
    type PalletId = NetworkPalletId;
    type Randomness = Randomness;
    type TreasuryAccount = TreasuryAccount;
    type OverwatchEpochEmissions = OverwatchEpochEmissions;
    type MaximumHooksWeight = MaximumHooksWeight;
    type MinAttestationPercentage = NetworkMinAttestationPercentage;
    type SuperMajorityAttestationRatio = NetworkSuperMajorityAttestationRatio;
    type InitialSubnetUid = NetworkInitialSubnetUid;
    type MaxPhysicalSubnetsUpperBound = NetworkMaxPhysicalSubnetsUpperBound;
    type MaxSubnetNodesUpperBound = NetworkMaxSubnetNodesUpperBound;
    type MaxValidatorNodesUpperBound = NetworkMaxValidatorNodesUpperBound;
    type MaxOverwatchNodesUpperBound = NetworkMaxOverwatchNodesUpperBound;
    type MaxOverwatchCommitCutoffPercent = NetworkMaxOverwatchCommitCutoffPercent;
    type MaxBootnodesUpperBound = NetworkMaxBootnodesUpperBound;
    type MaxSubnetBootnodeAccessUpperBound = NetworkMaxSubnetBootnodeAccessUpperBound;
    type MaxChurnLimitUpperBound = NetworkMaxChurnLimitUpperBound;
    type MaxRegisteredNodesUpperBound = NetworkMaxRegisteredNodesUpperBound;
    type MaxUnbondingsUpperBound = NetworkMaxUnbondingsUpperBound;
    type MaxSwapCallsPerBlockUpperBound = NetworkMaxSwapCallsPerBlockUpperBound;
    type MaxEmergencySubnetNodesUpperBound = NetworkMaxEmergencySubnetNodesUpperBound;
    type DesignatedEpochSlots = DesignatedEpochSlots;
    type MaxVectorLength = NetworkMaxVectorLength;
    type MaxUrlLength = NetworkMaxUrlLength;
    type MaxSocialIdLength = NetworkMaxSocialIdLength;
    type ValidatorArgsLimit = NetworkValidatorArgsLimit;
    type MaxOverwatchRevealSaltLength = NetworkMaxOverwatchRevealSaltLength;
    type MaxSwapQueueLength = NetworkMaxSwapQueueLength;
}

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
}

#[cfg(feature = "runtime-benchmarks")]
mod benches {
    use super::*;

    impl frame_benchmarking::baseline::Config for Runtime {}
    impl frame_system_benchmarking::Config for Runtime {}
    impl pallet_session_benchmarking::Config for Runtime {
        fn generate_session_keys_and_proof(owner: AccountId) -> (opaque::SessionKeys, Vec<u8>) {
            let generated = opaque::SessionKeys::generate(&owner.encode(), None);
            (generated.keys, generated.proof.encode())
        }
    }

    frame_benchmarking::define_benchmarks!(
        [frame_benchmarking, BaselineBench::<Runtime>]
        [frame_system, SystemBench::<Runtime>]
        [frame_system_extensions, SystemExtensionsBench::<Runtime>]
        [pallet_session, SessionBench::<Runtime>]
        [pallet_balances, Balances]
        [pallet_timestamp, Timestamp]
        [pallet_babe, Babe]
        [pallet_grandpa, Grandpa]
        [pallet_staking, Staking]
        [pallet_sudo, Sudo]
        [pallet_collective, Collective]
        [pallet_network, Network]
        [pallet_revive, Revive]
        // [pallet_treasury, Treasury]
    );
}

pallet_revive::impl_runtime_apis_plus_revive_traits! {
    Runtime, Revive, Executive, EthExtraImpl,
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: <Block as BlockT>::LazyBlock) {
            Executive::execute_block(block)
        }

        fn initialize_block(header: &<Block as BlockT>::Header) -> ExtrinsicInclusionMode {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: <Block as BlockT>::LazyBlock,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
        fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
            build_state::<RuntimeGenesisConfig>(config)
        }

        fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
            get_preset::<RuntimeGenesisConfig>(id, crate::genesis_config_presets::get_preset)
        }

        fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
            crate::genesis_config_presets::preset_names()
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(
            owner: Vec<u8>,
            seed: Option<Vec<u8>>,
        ) -> sp_session::OpaqueGeneratedSessionKeys {
            opaque::SessionKeys::generate(&owner, seed).into()
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl sp_consensus_babe::BabeApi<Block> for Runtime {
        fn configuration() -> sp_consensus_babe::BabeConfiguration {
            let epoch_config = Babe::epoch_config().unwrap_or(BABE_GENESIS_EPOCH_CONFIG);
            sp_consensus_babe::BabeConfiguration {
                slot_duration: Babe::slot_duration(),
                epoch_length: EpochDuration::get(),
                c: epoch_config.c,
                authorities: Babe::authorities().to_vec(),
                randomness: Babe::randomness(),
                allowed_slots: epoch_config.allowed_slots,
            }
        }

        fn current_epoch_start() -> sp_consensus_babe::Slot {
            Babe::current_epoch_start()
        }

        fn current_epoch() -> sp_consensus_babe::Epoch {
            Babe::current_epoch()
        }

        fn next_epoch() -> sp_consensus_babe::Epoch {
            Babe::next_epoch()
        }

        fn generate_key_ownership_proof(
            _slot: sp_consensus_babe::Slot,
            authority_id: sp_consensus_babe::AuthorityId,
        ) -> Option<sp_consensus_babe::OpaqueKeyOwnershipProof> {
            use codec::Encode;

            Historical::prove((sp_consensus_babe::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(sp_consensus_babe::OpaqueKeyOwnershipProof::new)
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: sp_consensus_babe::EquivocationProof<<Block as BlockT>::Header>,
            key_owner_proof: sp_consensus_babe::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;

            Babe::submit_unsigned_equivocation_report(
                equivocation_proof,
                key_owner_proof,
            )
        }
    }


    impl sp_consensus_grandpa::GrandpaApi<Block> for Runtime {
        fn grandpa_authorities() -> GrandpaAuthorityList {
            Grandpa::grandpa_authorities()
        }

        fn current_set_id() -> sp_consensus_grandpa::SetId {
            Grandpa::current_set_id()
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: sp_consensus_grandpa::EquivocationProof<
                <Block as BlockT>::Hash,
                NumberFor<Block>,
            >,
            key_owner_proof: sp_consensus_grandpa::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            Grandpa::submit_unsigned_equivocation_report(equivocation_proof, key_owner_proof.decode()?)
        }

        fn generate_key_ownership_proof(
            _set_id: sp_consensus_grandpa::SetId,
            authority_id: GrandpaId,
        ) -> Option<sp_consensus_grandpa::OpaqueKeyOwnershipProof> {
            Historical::prove((sp_consensus_grandpa::KEY_TYPE, authority_id))
                .map(|proof| sp_consensus_grandpa::OpaqueKeyOwnershipProof::new(proof.encode()))
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
        fn account_nonce(account: AccountId) -> Nonce {
            System::account_nonce(account)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
        Block,
        Balance,
    > for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32
        ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_info(uxt, len)
        }

        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_fee_details(uxt, len)
        }

        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }

        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl network_custom_rpc_runtime_api::NetworkRuntimeApi<Block> for Runtime {
        fn get_subnet_info(
            subnet_id: u32,
        ) -> Option<network_rpc_types::SubnetInfo<AccountId>> {
            Network::rpc_get_subnet_info(subnet_id)
        }

        fn get_subnets(
            request: network_rpc_types::PageRequest<u32>,
        ) -> Result<
            network_rpc_types::SubnetsPage<AccountId>,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_subnets(request)
        }

        fn get_subnet_node_info(
            subnet_id: u32,
            subnet_node_id: u32,
        ) -> Option<network_rpc_types::SubnetNodeInfo<AccountId>> {
            Network::rpc_get_subnet_node_info(subnet_id, subnet_node_id)
        }

        fn get_subnet_nodes(
            subnet_id: u32,
            request: network_rpc_types::PageRequest<u32>,
        ) -> Result<
            network_rpc_types::SubnetNodesPage<AccountId>,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_subnet_nodes(subnet_id, request)
        }

        fn get_bootnodes(subnet_id: u32) -> Option<network_rpc_types::SubnetBootnodes> {
            Network::rpc_get_bootnodes(subnet_id)
        }

        fn get_validator_info(
            validator_id: u32,
        ) -> Option<network_rpc_types::ValidatorInfo<AccountId>> {
            Network::rpc_get_validator_info(validator_id)
        }

        fn get_validator_by_coldkey(
            coldkey: AccountId,
        ) -> Option<network_rpc_types::ValidatorInfo<AccountId>> {
            Network::rpc_get_validator_by_coldkey(&coldkey)
        }

        fn get_validator_by_hotkey(
            hotkey: AccountId,
        ) -> Option<network_rpc_types::ValidatorInfo<AccountId>> {
            Network::rpc_get_validator_by_hotkey(&hotkey)
        }

        fn get_validator_nodes(
            validator_id: u32,
            request: network_rpc_types::PageRequest<network_rpc_types::SubnetNodeCursor>,
        ) -> Result<
            network_rpc_types::ValidatorNodesPage<AccountId>,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_validator_nodes(validator_id, request)
        }

        fn get_validator_node_stakes(
            validator_id: u32,
            request: network_rpc_types::PageRequest<network_rpc_types::SubnetNodeCursor>,
        ) -> Result<
            network_rpc_types::ValidatorNodeStakesPage,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_validator_node_stakes(validator_id, request)
        }

        fn get_validator_node_allocations(
            validator_id: u32,
            request: network_rpc_types::PageRequest<network_rpc_types::SubnetNodeCursor>,
        ) -> Result<
            network_rpc_types::ValidatorNodeAllocationsPage,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_validator_node_allocations(validator_id, request)
        }

        fn get_consensus_round(
            subnet_id: u32,
            subnet_epoch: u32,
        ) -> Result<
            Option<network_rpc_types::ConsensusRoundInfo>,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_consensus_round(subnet_id, subnet_epoch)
        }

        fn get_subnet_validator_nodes(
            subnet_id: u32,
            request: network_rpc_types::PageRequest<u32>,
        ) -> Result<
            network_rpc_types::SubnetValidatorNodesPage<AccountId>,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_subnet_validator_nodes(subnet_id, request)
        }

        fn get_subnet_epoch_status(
            subnet_id: u32,
        ) -> Result<
            network_rpc_types::SubnetEpochStatus,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_subnet_epoch_status(subnet_id)
        }

        fn get_overwatch_node_info(
            overwatch_node_id: u32,
        ) -> Option<network_rpc_types::OverwatchNodeInfo<AccountId>> {
            Network::rpc_get_overwatch_node_info(overwatch_node_id)
        }

        fn get_overwatch_nodes(
            request: network_rpc_types::PageRequest<u32>,
        ) -> Result<
            network_rpc_types::OverwatchNodesPage<AccountId>,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_overwatch_nodes(request)
        }

        fn get_effective_overwatch_signal_meta(
        ) -> network_rpc_types::EffectiveOverwatchSignalMeta {
            Network::rpc_get_effective_overwatch_signal_meta()
        }

        fn get_effective_overwatch_subnet_weight(
            subnet_id: u32,
        ) -> network_rpc_types::EffectiveOverwatchSubnetWeight {
            Network::rpc_get_effective_overwatch_subnet_weight(subnet_id)
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::{baseline, BenchmarkList};
            use frame_support::traits::StorageInfoTrait;

            use baseline::Pallet as BaselineBench;
            use frame_system_benchmarking::Pallet as SystemBench;
            use frame_system_benchmarking::extensions::Pallet as SystemExtensionsBench;
            use pallet_session_benchmarking::Pallet as SessionBench;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);

            let storage_info = AllPalletsWithSystem::storage_info();
            (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, alloc::string::String> {
            use frame_benchmarking::{baseline, BenchmarkBatch};
            use frame_support::traits::TrackedStorageKey;

            use baseline::Pallet as BaselineBench;
            use frame_system_benchmarking::Pallet as SystemBench;
            use frame_system_benchmarking::extensions::Pallet as SystemExtensionsBench;
            use pallet_session_benchmarking::Pallet as SessionBench;

            let whitelist: Vec<TrackedStorageKey> = Vec::new();

            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);
            add_benchmarks!(params, batches);
            Ok(batches)
        }
    }

    #[cfg(feature = "try-runtime")]
    impl frame_try_runtime::TryRuntime<Block> for Runtime {
        fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here. If any of the pre/post migration checks fail, we shall stop
            // right here and right now.
            let weight = Executive::try_runtime_upgrade(checks).unwrap();
            (weight, BlockWeights::get().max_block)
        }

        fn execute_block(
            block: <Block as BlockT>::LazyBlock,
            state_root_check: bool,
            signature_check: bool,
            select: frame_try_runtime::TryStateSelect
        ) -> Weight {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here.
            Executive::try_execute_block(block, state_root_check, signature_check, select).expect("execute-block failed")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AccountId, ProxyType, Runtime, RuntimeCall};
    use frame_support::traits::InstanceFilter;
    use sp_core::H256;

    fn account(id: u64) -> AccountId {
        sp_core::H256::from_low_u64_be(id).to_fixed_bytes().into()
    }

    fn network_call(call: pallet_network::Call<Runtime>) -> RuntimeCall {
        RuntimeCall::Network(call)
    }

    fn swap_call() -> pallet_network::QueuedSwapCall<AccountId> {
        pallet_network::QueuedSwapCall::SwapToSubnetDelegateStake {
            account_id: account(1),
            to_subnet_id: 2,
            balance: 10,
            min_shares_out: 1,
            execute_before_block: 100,
        }
    }

    #[test]
    fn non_transfer_rejects_network_value_calls() {
        let value_calls = vec![
            network_call(pallet_network::Call::update_validator_coldkey {
                validator_id: 1,
                new_coldkey: account(2),
            }),
            network_call(
                pallet_network::Call::update_validator_delegate_reward_rate {
                    validator_id: 1,
                    new_delegate_reward_rate: 10,
                },
            ),
            network_call(pallet_network::Call::update_validator_delegate_account {
                validator_id: 1,
                delegate_account_id: Some(account(2)),
                delegate_rate: Some(10),
            }),
            network_call(pallet_network::Call::register_subnet {
                max_cost: 10,
                subnet_data: pallet_network::RegistrationSubnetData::<Runtime> {
                    name: Vec::new(),
                    repo: Vec::new(),
                    description: Vec::new(),
                    misc: Vec::new(),
                    min_stake: 0,
                    max_stake: 0,
                    delegate_stake_percentage: 0,
                    initial_validators: Default::default(),
                    bootnodes: Default::default(),
                },
            }),
            network_call(pallet_network::Call::activate_subnet { subnet_id: 1 }),
            network_call(pallet_network::Call::owner_deactivate_subnet { subnet_id: 1 }),
            network_call(pallet_network::Call::owner_update_min_max_stake {
                subnet_id: 1,
                min: 10,
                max: 20,
            }),
            network_call(
                pallet_network::Call::owner_update_delegate_stake_percentage {
                    subnet_id: 1,
                    value: 10,
                },
            ),
            network_call(pallet_network::Call::transfer_subnet_ownership {
                subnet_id: 1,
                new_owner: account(2),
            }),
            network_call(pallet_network::Call::accept_subnet_ownership { subnet_id: 1 }),
            network_call(
                pallet_network::Call::owner_update_target_node_registrations_per_epoch {
                    subnet_id: 1,
                    value: 1,
                },
            ),
            network_call(pallet_network::Call::owner_update_node_burn_rate_alpha {
                subnet_id: 1,
                value: 10,
            }),
            network_call(pallet_network::Call::register_subnet_node {
                validator_id: 1,
                subnet_id: 1,
                hotkey: Some(account(3)),
                peer_info: Some(pallet_network::PeerInfo::<Runtime> {
                    peer_id: sp_core::OpaquePeerId(Vec::new()),
                    multiaddr: None,
                }),
                bootnode_peer_info: None,
                client_peer_info: None,
                stake_to_be_added: 10,
                unique: None,
                non_unique: None,
                max_burn_amount: 10,
            }),
            network_call(pallet_network::Call::remove_subnet_node {
                subnet_id: 1,
                subnet_node_id: 1,
            }),
            network_call(pallet_network::Call::add_node_stake {
                subnet_id: 1,
                subnet_node_id: 1,
                stake_to_be_added: 10,
            }),
            network_call(pallet_network::Call::remove_node_stake {
                subnet_id: 1,
                subnet_node_id: 1,
                stake_to_be_removed: 10,
            }),
            network_call(pallet_network::Call::add_subnet_delegate_stake {
                subnet_id: 1,
                stake_to_be_added: 10,
                min_shares_out: 1,
            }),
            network_call(pallet_network::Call::swap_from_subnet_to_subnet {
                from_subnet_id: 1,
                to_subnet_id: 2,
                delegate_stake_shares_to_swap: 10,
                min_balance_out: 1,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            }),
            network_call(pallet_network::Call::transfer_delegate_stake {
                subnet_id: 1,
                to_account_id: account(2),
                delegate_stake_shares_to_transfer: 10,
            }),
            network_call(pallet_network::Call::remove_delegate_stake {
                subnet_id: 1,
                shares_to_be_removed: 10,
                min_balance_out: 1,
            }),
            network_call(pallet_network::Call::add_validator_delegate_stake {
                validator_id: 1,
                delegate_stake_to_be_added: 10,
                min_shares_out: 1,
            }),
            network_call(pallet_network::Call::transfer_validator_delegate_stake {
                validator_id: 1,
                to_account_id: account(2),
                validator_delegate_stake_shares_to_transfer: 10,
            }),
            network_call(pallet_network::Call::remove_validator_delegate_stake {
                validator_id: 1,
                validator_delegate_stake_shares_to_be_removed: 10,
                min_balance_out: 1,
            }),
            network_call(pallet_network::Call::swap_from_validator_to_validator {
                from_validator_id: 1,
                to_validator_id: 2,
                stake_to_be_removed: 10,
                min_balance_out: 1,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            }),
            network_call(pallet_network::Call::swap_from_validator_to_subnet {
                from_validator_id: 1,
                to_subnet_id: 1,
                node_delegate_stake_shares_to_swap: 10,
                min_balance_out: 1,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            }),
            network_call(pallet_network::Call::swap_from_subnet_to_validator {
                from_subnet_id: 1,
                to_validator_id: 1,
                subnet_delegate_stake_shares_to_swap: 10,
                min_balance_out: 1,
                min_shares_out: 1,
                execute_before_block: u32::MAX,
            }),
            network_call(pallet_network::Call::update_swap_queue {
                id: 1,
                new_call: swap_call(),
            }),
            network_call(pallet_network::Call::remove_delegate_account_balance {
                amount_to_remove: 10,
            }),
            network_call(pallet_network::Call::claim_unbondings {}),
            network_call(pallet_network::Call::register_overwatch_node {
                stake_to_be_added: 10,
            }),
            network_call(pallet_network::Call::remove_overwatch_node {
                overwatch_node_id: 1,
            }),
            network_call(pallet_network::Call::add_overwatch_node_stake {
                overwatch_node_id: 1,
                stake_to_be_added: 10,
            }),
            network_call(pallet_network::Call::remove_overwatch_node_stake {
                overwatch_node_id: 1,
                stake_to_be_removed: 10,
            }),
        ];

        for call in value_calls {
            assert!(
                !ProxyType::NonTransfer.filter(&call),
                "NonTransfer unexpectedly allowed {call:?}"
            );
        }
    }

    #[test]
    fn non_transfer_rejects_utility_and_proxy_escalation() {
        let blocked_network_call = network_call(pallet_network::Call::add_subnet_delegate_stake {
            subnet_id: 1,
            stake_to_be_added: 10,
            min_shares_out: 1,
        });
        let utility_call = RuntimeCall::Utility(pallet_utility::Call::batch_all {
            calls: vec![blocked_network_call],
        });
        let add_proxy_call = RuntimeCall::Proxy(pallet_proxy::Call::add_proxy {
            delegate: account(2),
            proxy_type: ProxyType::Transfer,
            delay: 0,
        });
        let reject_announcement_call =
            RuntimeCall::Proxy(pallet_proxy::Call::reject_announcement {
                delegate: account(2),
                call_hash: H256::zero(),
            });

        assert!(!ProxyType::NonTransfer.filter(&utility_call));
        assert!(!ProxyType::NonTransfer.filter(&add_proxy_call));
        assert!(ProxyType::NonTransfer.filter(&reject_announcement_call));
        assert!(ProxyType::CancelProxy.filter(&reject_announcement_call));
    }

    #[test]
    fn dedicated_proxy_types_allow_only_their_scoped_network_calls() {
        let balance_transfer = RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
            dest: account(2),
            value: 10,
        });
        let network_transfer = network_call(pallet_network::Call::transfer_delegate_stake {
            subnet_id: 1,
            to_account_id: account(2),
            delegate_stake_shares_to_transfer: 10,
        });
        let delegate_stake = network_call(pallet_network::Call::add_subnet_delegate_stake {
            subnet_id: 1,
            stake_to_be_added: 10,
            min_shares_out: 1,
        });
        let node_stake = network_call(pallet_network::Call::add_node_stake {
            subnet_id: 1,
            subnet_node_id: 1,
            stake_to_be_added: 10,
        });
        let overwatch_stake = network_call(pallet_network::Call::add_overwatch_node_stake {
            overwatch_node_id: 1,
            stake_to_be_added: 10,
        });
        let validator_delegate_stake =
            network_call(pallet_network::Call::add_validator_delegate_stake {
                validator_id: 1,
                delegate_stake_to_be_added: 10,
                min_shares_out: 1,
            });
        let swap_queue_update = network_call(pallet_network::Call::update_swap_queue {
            id: 1,
            new_call: swap_call(),
        });
        let delegate_account_balance_removal =
            network_call(pallet_network::Call::remove_delegate_account_balance {
                amount_to_remove: 10,
            });

        assert!(ProxyType::Transfer.filter(&balance_transfer));
        assert!(ProxyType::Transfer.filter(&network_transfer));
        assert!(!ProxyType::Transfer.filter(&delegate_stake));

        assert!(ProxyType::SubNetworkStaking.filter(&node_stake));
        assert!(ProxyType::SubNetworkStaking.filter(&overwatch_stake));
        assert!(!ProxyType::SubNetworkStaking.filter(&delegate_stake));

        assert!(ProxyType::SubNetworkDelegateStaking.filter(&delegate_stake));
        assert!(ProxyType::SubNetworkDelegateStaking.filter(&validator_delegate_stake));
        assert!(ProxyType::SubNetworkDelegateStaking.filter(&swap_queue_update));
        assert!(ProxyType::SubNetworkDelegateStaking.filter(&delegate_account_balance_removal));
        assert!(!ProxyType::SubNetworkDelegateStaking.filter(&network_transfer));
    }

    #[test]
    fn non_transfer_is_not_superset_of_value_proxy_types() {
        assert!(!ProxyType::NonTransfer.is_superset(&ProxyType::Transfer));
        assert!(!ProxyType::NonTransfer.is_superset(&ProxyType::SubNetworkStaking));
        assert!(!ProxyType::NonTransfer.is_superset(&ProxyType::SubNetworkDelegateStaking));
        assert!(ProxyType::NonTransfer.is_superset(&ProxyType::CancelProxy));
    }
}
