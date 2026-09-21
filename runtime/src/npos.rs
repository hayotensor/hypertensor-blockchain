//! Consensus staking for a small, bounded standalone network (SDK stable2606).
use super::*;
use frame_election_provider_support::{
    bounds::{ElectionBounds, ElectionBoundsBuilder},
    onchain, BoundedSupportsOf, ElectionProvider, PageIndex, SequentialPhragmen,
};
use sp_runtime::traits::OpaqueKeys;

/// Maximum number of validators one nominator can choose to back.
pub const MAX_NOMINATIONS: u32 = 16;
/// Maximum supported validator candidates and elected validators.
pub const MAX_VALIDATORS: u32 = 32;
/// Maximum supported number of accounts that nominate validators.
pub const MAX_NOMINATORS: u32 = 256;
/// Initial minimum number of validators required for a successful election.
pub const MIN_VALIDATOR_COUNT: u32 = 1;
/// Initial minimum validator stake, also bonded by each genesis validator.
pub const VALIDATOR_BOND: Balance = 1_000 * TENSOR;
/// Initial minimum stake an account must bond to nominate validators.
// U128CurrencyToVote divides by issuance / u64::MAX. Keep the minimum
// above its largest possible divisor (about 18.45 TENSOR), so a minimum
// bond has voting weight at every possible total issuance.
pub const MIN_NOMINATOR_BOND: Balance = 20 * TENSOR;
/// Annual consensus reward budget, divided among eras by their duration.
/// Separate from Network application rewards; confirm the amount before launch.
pub const STAKING_YEARLY_EMISSIONS: Balance = 1_000 * TENSOR;
/// Initial BABE block-author selection rules, including fallback authors.
pub const BABE_GENESIS_EPOCH_CONFIG: sp_consensus_babe::BabeEpochConfiguration =
    sp_consensus_babe::BabeEpochConfiguration {
        c: (1, 4),
        allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryPlainSlots,
    };

/// Slots per production epoch/session: four hours at the configured slot duration.
/// Choose before genesis; an ordinary runtime upgrade cannot change this duration.
pub const PRODUCTION_EPOCH_SLOTS: u64 = 4 * 60 * 60 * 1_000 / SLOT_DURATION;
/// Sessions per production staking era: six four-hour sessions make one day.
pub const PRODUCTION_SESSIONS_PER_ERA: u32 = 6;

parameter_types! {
    /// Slots per epoch/session: production timing normally, 20 slots in tests or fast builds.
    // Unit tests exercise many eras using the same accelerated configuration as
    // explicit fast-runtime development builds. Default Wasm uses daily eras.
    pub const EpochDuration: u64 = if cfg!(any(test, feature = "fast-runtime")) {
        20
    } else {
        PRODUCTION_EPOCH_SLOTS
    };
    /// Target time between blocks, in milliseconds.
    pub const ExpectedBlockTime: u64 = MILLISECS_PER_BLOCK;
    /// Sessions per staking era: six normally, three in tests or fast builds.
    pub const SessionsPerEra: u32 = if cfg!(any(test, feature = "fast-runtime")) {
        3
    } else {
        PRODUCTION_SESSIONS_PER_ERA
    };
    /// Eras to wait after unbonding before stake can be withdrawn.
    pub const BondingDuration: u32 = 28;
    /// Delay in eras before a reported staking penalty is applied.
    pub const SlashDeferDuration: u32 = 7;
    /// Past eras retained for staking records and reward claims.
    pub const HistoryDepth: u32 = 84;
    /// Validator limit shared by consensus, staking, and election configuration.
    pub const MaxAuthorities: u32 = MAX_VALIDATORS;
    /// Nominator limit used by consensus checks and staking benchmarks.
    pub const MaxNominators: u32 = MAX_NOMINATORS;
    /// Backers allowed per elected validator: all nominators plus its own stake.
    pub const MaxBackersPerWinner: u32 = MAX_NOMINATORS + 1;
    /// Refundable balance held when an account registers its consensus keys.
    pub const SessionKeyDeposit: Balance = TENSOR;
    /// Block lifetime of a pending validator double-signing report.
    pub const ReportLongevity: u64 =
        BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
    /// GRANDPA validator-set history entries kept to verify past double-signing reports.
    pub const MaxSetIdSessionEntries: u64 =
        BondingDuration::get() as u64 * SessionsPerEra::get() as u64;
    /// Maximum voters and validator candidates that one election may process.
    pub ElectionInputBounds: ElectionBounds = ElectionBoundsBuilder::default()
        .voters_count((MAX_VALIDATORS + MAX_NOMINATORS).into())
        .targets_count(MAX_VALIDATORS.into())
        .build();
}

/// Complete elections fit the registration caps: no voters are intentionally
/// discarded by an unsorted map, and no offchain miner is needed for liveness.
pub struct OnChainElection;
impl onchain::Config for OnChainElection {
    type Sort = frame_support::traits::ConstBool<false>;
    type System = Runtime;
    type Solver = SequentialPhragmen<AccountId, Perbill>;
    type DataProvider = Staking;
    type WeightInfo = frame_election_provider_support::weights::SubstrateWeight<Runtime>;
    type MaxWinnersPerPage = MaxAuthorities;
    type MaxBackersPerWinner = MaxBackersPerWinner;
    type Bounds = ElectionInputBounds;
}

/// The SDK's mutable registration limits can be raised or removed by Root.
/// Its voter snapshot then truncates the unsorted map at `Bounds`, which would
/// silently discard stake. Reject that state before taking a snapshot; Staking
/// retains the previous validator set until registration is within bounds again.
pub struct BoundedStakingElection;
impl ElectionProvider for BoundedStakingElection {
    type AccountId = AccountId;
    type BlockNumber = BlockNumber;
    type Error = onchain::Error;
    type MaxWinnersPerPage = MaxAuthorities;
    type MaxBackersPerWinner = MaxBackersPerWinner;
    type MaxBackersPerWinnerFinal = MaxBackersPerWinner;
    type Pages = ConstU32<1>;
    type DataProvider = Staking;
    fn start() -> Result<(), Self::Error> {
        Ok(())
    }

    fn duration() -> BlockNumber {
        0
    }

    fn status() -> Result<Option<Weight>, ()> {
        onchain::OnChainExecution::<OnChainElection>::status()
    }

    fn elect(page: PageIndex) -> Result<BoundedSupportsOf<Self>, Self::Error> {
        if page != 0 {
            return Err(onchain::Error::DataProvider(
                "only one election page is supported",
            ));
        }
        System::register_extra_weight_unchecked(
            <Runtime as frame_system::Config>::DbWeight::get().reads(2),
            frame_support::dispatch::DispatchClass::Mandatory,
        );
        if pallet_staking::Validators::<Runtime>::count() > MAX_VALIDATORS
            || pallet_staking::Nominators::<Runtime>::count() > MAX_NOMINATORS
        {
            return Err(onchain::Error::DataProvider(
                "staking registrations exceed the runtime election bounds",
            ));
        }
        onchain::OnChainExecution::<OnChainElection>::elect(page)
    }
}

pub struct StakingEraPayout;
impl pallet_staking::EraPayout<Balance> for StakingEraPayout {
    fn era_payout(_: Balance, _: Balance, era_duration_millis: u64) -> (Balance, Balance) {
        let payout = STAKING_YEARLY_EMISSIONS.saturating_mul(era_duration_millis as Balance)
            / (365 * 24 * 60 * 60 * 1_000u128);
        (payout, 0)
    }
}

impl pallet_babe::Config for Runtime {
    type EpochDuration = EpochDuration;
    type ExpectedBlockTime = ExpectedBlockTime;
    type EpochChangeTrigger = pallet_babe::ExternalTrigger;
    type DisabledValidators = Session;
    // This SDK only provides its nonzero default_weights implementation for ().
    // Regenerate chain-specific equivocation weights before production release.
    type WeightInfo = ();
    type MaxAuthorities = MaxAuthorities;
    type MaxNominators = MaxNominators;
    type KeyOwnerProof = sp_session::MembershipProof;
    type EquivocationReportSystem =
        pallet_babe::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    // This SDK only provides nonzero default_weights for ().
    type WeightInfo = ();
    type MaxAuthorities = MaxAuthorities;
    type MaxNominators = MaxNominators;
    type MaxSetIdSessionEntries = MaxSetIdSessionEntries;
    type KeyOwnerProof = sp_session::MembershipProof;
    type EquivocationReportSystem =
        pallet_grandpa::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

impl pallet_session::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = AccountId;
    type ValidatorIdOf = sp_runtime::traits::ConvertInto;
    type ShouldEndSession = Babe;
    type NextSessionRotation = Babe;
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
    type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
    type Keys = opaque::SessionKeys;
    type DisablingStrategy = pallet_session::disabling::UpToLimitWithReEnablingDisablingStrategy;
    type Currency = Balances;
    type KeyDeposit = SessionKeyDeposit;
    type WeightInfo = pallet_session::weights::SubstrateWeight<Runtime>;
}

impl pallet_session::historical::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type FullIdentification = ();
    type FullIdentificationOf = pallet_staking::UnitIdentificationOf<Runtime>;
}

impl pallet_authorship::Config for Runtime {
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
    type EventHandler = Staking;
}

impl pallet_offences::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
    type OnOffenceHandler = Staking;
}

pub struct StakingBenchmarkingConfig;
impl pallet_staking::BenchmarkingConfig for StakingBenchmarkingConfig {
    type MaxNominators = MaxNominators;
    type MaxValidators = MaxAuthorities;
}

impl pallet_staking::Config for Runtime {
    // Required by the SDK trait; fresh genesis uses holds and installs no migrations.
    type OldCurrency = Balances;
    type Currency = Balances;
    type RuntimeHoldReason = RuntimeHoldReason;
    type CurrencyBalance = Balance;
    type UnixTime = Timestamp;
    type CurrencyToVote = sp_staking::currency_to_vote::U128CurrencyToVote;
    type ElectionProvider = BoundedStakingElection;
    type GenesisElectionProvider = BoundedStakingElection;
    type NominationsQuota = pallet_staking::FixedNominationsQuota<MAX_NOMINATIONS>;
    type HistoryDepth = HistoryDepth;
    type RewardRemainder = ();
    type RuntimeEvent = RuntimeEvent;
    type Slash = (); // Burn slashed balances; no treasury routing.
    type Reward = (); // Mint rewards when claimed.
    type SessionsPerEra = SessionsPerEra;
    type BondingDuration = BondingDuration;
    type SlashDeferDuration = SlashDeferDuration;
    type AdminOrigin = EnsureRoot<AccountId>;
    type SessionInterface = Self;
    type EraPayout = StakingEraPayout;
    type NextNewSession = Session;
    type MaxExposurePageSize = ConstU32<64>;
    type VoterList = pallet_staking::UseNominatorsAndValidatorsMap<Self>;
    type TargetList = pallet_staking::UseValidatorsMap<Self>;
    type MaxUnlockingChunks = ConstU32<32>;
    type MaxControllersInDeprecationBatch = ConstU32<32>;
    type EventListeners = ();
    type Filter = frame_support::traits::Nothing;
    type MaxValidatorSet = MaxAuthorities;
    type BenchmarkingConfig = StakingBenchmarkingConfig;
    type WeightInfo = pallet_staking::weights::SubstrateWeight<Runtime>;
}

impl<C> frame_system::offchain::CreateTransactionBase<C> for Runtime
where
    RuntimeCall: From<C>,
{
    type Extrinsic = UncheckedExtrinsic;
    type RuntimeCall = RuntimeCall;
}

impl<C> frame_system::offchain::CreateBare<C> for Runtime
where
    RuntimeCall: From<C>,
{
    fn create_bare(call: RuntimeCall) -> UncheckedExtrinsic {
        generic::UncheckedExtrinsic::new_bare(call).into()
    }
}
