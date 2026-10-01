//! Launch monetary policy. Change STARTING_SUPPLY and rebuild before creating genesis.
//!
//! Rates use the fixed premine reference, not the changing Balances total issuance.
//! TENSOR remains a denomination of 10^18 atomic units at every supply.

use crate::{Balance, TENSOR};
use pallet_network::NetworkEconomics;

/// Provisional launch premine. Genesis allocations must sum to this amount.
pub const STARTING_SUPPLY: Balance = 50_000_000 * TENSOR;

// Parts per million of STARTING_SUPPLY per year: 10_000 ppm = 1%.
// These preserve the existing amounts at the provisional 50 million supply.
pub const CONSENSUS_INFLATION_PPM: u32 = 20; // 0.002%
pub const NETWORK_INITIAL_INFLATION_PPM: u32 = 2_000; // 0.2%, including foundation
pub const NETWORK_TERMINAL_INFLATION_PPM: u32 = 1_500; // 0.15%
pub const OVERWATCH_INFLATION_PPM: u32 = 200; // 0.02%
pub const NETWORK_ANNUAL_RETENTION_PERCENT: u8 = 90;
pub const FOUNDATION_SHARE_PERCENT: u8 = 5;
pub const CODE_HASH_LOCKUP_PERCENT: u32 = 30;

pub const TARGET_BLOCK_FULLNESS_PERCENT: u64 = 25;
pub const FEE_ADJUSTMENT_DENOMINATOR: u128 = 100_000;
pub const MAXIMUM_FEE_MULTIPLIER: u32 = 1_000_000;

/// Weight price = starting supply / this denominator per unit of reference time.
/// With the current 2-second block budget, the full-block weight fee is supply / 25 billion.
/// Keep the rational intact: rounding the per-unit price to an integer breaks small supplies.
pub const WEIGHT_FEE_DENOMINATOR: u128 = 50_000_000 * TENSOR / 1_000;
pub type WeightToFee = pallet_revive::evm::fees::BlockRatioFee<
    STARTING_SUPPLY,
    WEIGHT_FEE_DENOMINATOR,
    crate::Runtime,
    Balance,
>;

/// Supply-derived monetary settings. All `Balance` fields use atomic TENSOR units
/// (one TENSOR is 10^18 units), not whole tokens or percentage rates.
#[derive(Clone, Copy, Debug)]
pub struct Tokenomics {
    /// Fixed starting supply reference used to scale monetary settings; does not track live issuance.
    pub starting_supply: Balance,
    /// Annual consensus staking reward budget shared by validators and nominators,
    /// prorated by era duration and separate from Network and Overwatch rewards.
    pub consensus_annual_emissions: Balance,
    /// Annual Overwatch reward budget, converted into per-epoch allocations.
    pub overwatch_annual_emissions: Balance,
    /// Transaction length fee per encoded byte, separate from execution fees and tips.
    pub transaction_byte_fee: Balance,
    /// Deposit per stored item used by the shared deposit calculation and Revive contracts.
    pub storage_item_deposit: Balance,
    /// Deposit per stored byte used by the shared deposit calculation and Revive contracts.
    pub storage_byte_deposit: Balance,
    /// Revive deposit per child-trie storage item, configured separately from ordinary items.
    pub storage_child_item_deposit: Balance,
    /// Initial minimum consensus validator stake, also bonded by each preset genesis validator.
    pub validator_bond: Balance,
    /// Initial minimum consensus stake required to nominate validators.
    pub nominator_bond: Balance,
    /// Refundable deposit held when an account registers its consensus session keys.
    pub session_key_deposit: Balance,
    /// Network emission schedule and monetary defaults, including subnet stakes,
    /// registration costs, burns, slash limits, and the additional per-proposal reward.
    pub network: NetworkEconomics,
}

/// Multiply by a proper fraction, rounding down without overflowing for large supplies.
pub const fn fraction(value: u128, numerator: u128, denominator: u128) -> u128 {
    assert!(denominator > 0 && numerator <= denominator);
    // Callers use small denominators; the remainder product must also fit.
    let remainder = match (value % denominator).checked_mul(numerator) {
        Some(value) => value,
        None => panic!("tokenomics fraction denominator is too large"),
    };
    (value / denominator) * numerator + remainder / denominator
}

impl Tokenomics {
    pub const fn for_supply(supply: Balance) -> Self {
        // Explicit supported precision floor; never silently create zero fees or deposits.
        assert!(
            supply >= TENSOR,
            "starting supply must be at least one token"
        );
        assert!(NETWORK_TERMINAL_INFLATION_PPM <= NETWORK_INITIAL_INFLATION_PPM);
        assert!(NETWORK_ANNUAL_RETENTION_PERCENT <= 100);
        assert!(FOUNDATION_SHARE_PERCENT <= 100);
        Self {
            starting_supply: supply,
            consensus_annual_emissions: fraction(
                supply,
                CONSENSUS_INFLATION_PPM as u128,
                1_000_000,
            ),
            overwatch_annual_emissions: fraction(
                supply,
                OVERWATCH_INFLATION_PPM as u128,
                1_000_000,
            ),
            // All denominators below are fractions of the starting supply, not token decimals.
            transaction_byte_fee: supply / 50_000_000_000_000,
            storage_item_deposit: supply / 500_000_000,
            storage_byte_deposit: supply / 5_000_000_000_000,
            storage_child_item_deposit: supply / 50_000_000_000,
            validator_bond: supply / 50_000,
            nominator_bond: supply / 2_500_000,
            session_key_deposit: supply / 50_000_000,
            network: NetworkEconomics {
                initial_annual_emissions: fraction(
                    supply,
                    NETWORK_INITIAL_INFLATION_PPM as u128,
                    1_000_000,
                ),
                terminal_annual_emissions: fraction(
                    supply,
                    NETWORK_TERMINAL_INFLATION_PPM as u128,
                    1_000_000,
                ),
                annual_retention_percent: NETWORK_ANNUAL_RETENTION_PERCENT,
                foundation_share_percent: FOUNDATION_SHARE_PERCENT,
                subnet_min_stake: supply / 500_000,
                max_stake: supply / 50_000,
                max_subnet_min_stake: supply / 200_000,
                min_subnet_delegate_stake: supply / 500_000,
                overwatch_min_stake: supply / 500_000,
                base_validator_reward: supply / 50_000_000,
                max_slash: supply / 50_000_000,
                base_node_burn: supply / 5_000_000_000_000,
                initial_registration_cost: supply / 50_000_000,
                min_registration_cost: supply / 500_000_000,
            },
        }
    }
}

pub const POLICY: Tokenomics = Tokenomics::for_supply(STARTING_SUPPLY);

/// Reject a chain spec calibrated to a different premine before building its storage.
pub fn validate_genesis(config: &crate::RuntimeGenesisConfig) -> Result<(), alloc::string::String> {
    let supply = config
        .balances
        .balances
        .iter()
        .try_fold(0u128, |total, (_, balance)| {
            total
                .checked_add(*balance)
                .ok_or_else(|| alloc::string::String::from("genesis balances overflow"))
        })?;
    if supply != STARTING_SUPPLY {
        return Err(alloc::format!(
            "genesis balances total {supply} must equal tokenomics starting supply {STARTING_SUPPLY} (atomic units)"
        ));
    }
    for account in [crate::Treasury::account_id(), crate::Revive::account_id()] {
        if !config
            .balances
            .balances
            .iter()
            .any(|(who, balance)| who == &account && *balance >= crate::EXISTENTIAL_DEPOSIT)
        {
            return Err(alloc::string::String::from(
                "genesis balances must include existential deposits for Treasury and Revive inside the premine"
            ));
        }
    }
    if config.staking.min_validator_bond != POLICY.validator_bond
        || config.staking.min_nominator_bond != POLICY.nominator_bond
    {
        return Err(alloc::string::String::from(
            "genesis staking minimum bonds must match tokenomics policy",
        ));
    }
    Ok(())
}

/// Shared by the runtime GenesisBuilder API and tests.
pub fn build_genesis_state(config: alloc::vec::Vec<u8>) -> sp_genesis_builder::Result {
    let parsed: crate::RuntimeGenesisConfig = serde_json::from_slice(&config)
        .map_err(|error| alloc::format!("invalid genesis config: {error}"))?;
    validate_genesis(&parsed)?;
    // Pallet genesis logic (including optional EVM accounts) can alter issuance.
    // Reject and roll back any extra minting or burning instead of accepting a drifted premine.
    frame_support::storage::with_transaction_unchecked(|| {
        let result = frame_support::genesis_builder_helper::build_state::<crate::RuntimeGenesisConfig>(config)
            .and_then(|()| {
                if crate::Balances::total_issuance() == STARTING_SUPPLY {
                    Ok(())
                } else {
                    Err(alloc::string::String::from("pallet genesis changed issuance outside the configured premine; fund all genesis accounts through balances"))
                }
            });
        if result.is_ok() {
            frame_support::storage::TransactionOutcome::Commit(result)
        } else {
            frame_support::storage::TransactionOutcome::Rollback(result)
        }
    })
}
