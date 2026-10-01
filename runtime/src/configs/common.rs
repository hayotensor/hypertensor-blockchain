//! Core, utility, governance, and Network pallet configuration.

use crate::*;

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
    pub const TransactionByteFee: Balance = tokenomics::POLICY.transaction_byte_fee;
    pub TargetBlockFullness: Perquintill = Perquintill::from_percent(tokenomics::TARGET_BLOCK_FULLNESS_PERCENT);
    pub AdjustmentVariable: Multiplier = Multiplier::from_rational(1, tokenomics::FEE_ADJUSTMENT_DENOMINATOR);
    // Revive requires minimum_multiplier * NativeToEthRatio >= 1.
    pub MinimumMultiplier: Multiplier = Multiplier::one();
    pub MaximumMultiplier: Multiplier = Multiplier::from_u32(tokenomics::MAXIMUM_FEE_MULTIPLIER);
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = FungibleAdapter<Balances, ()>;
    // Execution and proof-size prices scale with the fixed starting supply.
    type WeightToFee = tokenomics::WeightToFee;
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
    const ITEMS_FEE: Balance = tokenomics::POLICY.storage_item_deposit;
    const BYTES_FEE: Balance = tokenomics::POLICY.storage_byte_deposit;
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
    pub const NetworkEconomics: pallet_network::NetworkEconomics = tokenomics::POLICY.network;
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
    type Economics = NetworkEconomics;
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
