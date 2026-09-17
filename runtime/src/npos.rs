//! Consensus staking for a small, bounded standalone network (SDK stable2412).
use super::*;
use frame_election_provider_support::{
    bounds::{ElectionBounds, ElectionBoundsBuilder},
    onchain, BoundedSupportsOf, ElectionProvider, ElectionProviderBase, SequentialPhragmen,
};
use sp_runtime::traits::OpaqueKeys;

pub const MAX_NOMINATIONS: u32 = 16;
pub const MAX_VALIDATORS: u32 = 32;
pub const MAX_NOMINATORS: u32 = 256;
pub const MIN_VALIDATOR_COUNT: u32 = 1;
pub const VALIDATOR_BOND: Balance = 1_000 * TENSOR;
// U128CurrencyToVote divides by issuance / u64::MAX. Keep the minimum
// above its largest possible divisor (about 18.45 TENSOR), including the
// preserved high-issuance eth_dev fixture, so a minimum bond has voting weight.
pub const MIN_NOMINATOR_BOND: Balance = 20 * TENSOR;
/// Separate from the existing application/author emissions. Replace before launch
/// if product tokenomics specify a different annual consensus reward budget.
pub const STAKING_YEARLY_EMISSIONS: Balance = 1_000 * TENSOR;
pub const BABE_GENESIS_EPOCH_CONFIG: sp_consensus_babe::BabeEpochConfiguration =
    sp_consensus_babe::BabeEpochConfiguration {
        c: (1, 4),
        allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryPlainSlots,
    };

parameter_types! {
    // Development timing. Set BEFORE generating a production genesis: BABE epoch
    // duration cannot subsequently be changed by a normal runtime upgrade.
    pub const EpochDuration: u64 = 20;
    pub const ExpectedBlockTime: u64 = MILLISECS_PER_BLOCK;
    pub const SessionsPerEra: u32 = 3;
    pub const BondingDuration: u32 = 28;
    pub const SlashDeferDuration: u32 = 7;
    pub const HistoryDepth: u32 = 84;
    pub const MaxAuthorities: u32 = MAX_VALIDATORS;
    pub const MaxNominators: u32 = MAX_NOMINATORS;
    pub const ReportLongevity: u64 =
        BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
    pub const MaxSetIdSessionEntries: u64 =
        BondingDuration::get() as u64 * SessionsPerEra::get() as u64;
    pub ElectionInputBounds: ElectionBounds = ElectionBoundsBuilder::default()
        .voters_count((MAX_VALIDATORS + MAX_NOMINATORS).into())
        .targets_count(MAX_VALIDATORS.into())
        .build();
}

/// Complete elections fit the registration caps: no voters are intentionally
/// discarded by an unsorted map, and no offchain miner is needed for liveness.
pub struct OnChainElection;
impl onchain::Config for OnChainElection {
    type System = Runtime;
    type Solver = SequentialPhragmen<AccountId, Perbill>;
    type DataProvider = Staking;
    type WeightInfo = frame_election_provider_support::weights::SubstrateWeight<Runtime>;
    type MaxWinners = MaxAuthorities;
    type Bounds = ElectionInputBounds;
}

/// The SDK's mutable registration limits can be raised or removed by Root.
/// Its voter snapshot then truncates the unsorted map at `Bounds`, which would
/// silently discard stake. Reject that state before taking a snapshot; Staking
/// retains the previous validator set until registration is within bounds again.
pub struct BoundedStakingElection;
impl ElectionProviderBase for BoundedStakingElection {
    type AccountId = AccountId;
    type BlockNumber = BlockNumber;
    type Error = onchain::Error;
    type MaxWinners = MaxAuthorities;
    type DataProvider = Staking;
}

impl ElectionProvider for BoundedStakingElection {
    fn ongoing() -> bool {
        false
    }

    fn elect() -> Result<BoundedSupportsOf<Self>, Self::Error> {
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
        onchain::OnChainExecution::<OnChainElection>::elect()
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
    type ValidatorIdOf = pallet_staking::StashOf<Self>;
    type ShouldEndSession = Babe;
    type NextSessionRotation = Babe;
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
    type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
    type Keys = opaque::SessionKeys;
    type WeightInfo = pallet_session::weights::SubstrateWeight<Runtime>;
}

impl pallet_session::historical::Config for Runtime {
    type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
    type FullIdentificationOf = pallet_staking::ExposureOf<Runtime>;
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
    type Currency = Balances;
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
    type DisablingStrategy = pallet_staking::UpToLimitDisablingStrategy;
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

impl<C> frame_system::offchain::CreateInherent<C> for Runtime
where
    RuntimeCall: From<C>,
{
    fn create_inherent(call: RuntimeCall) -> UncheckedExtrinsic {
        UncheckedExtrinsic::new_bare(call)
    }
}
