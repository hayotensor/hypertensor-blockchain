//! Monetary policy supplied by the runtime. Amounts are atomic native balances.

#[derive(Clone, Copy, Debug, PartialEq, Eq, codec::Encode, codec::Decode, scale_info::TypeInfo)]
pub struct NetworkEconomics {
    pub initial_annual_emissions: u128,
    pub terminal_annual_emissions: u128,
    /// Whole percent retained each year, in 0..=100.
    pub annual_retention_percent: u8,
    /// Whole percent of the network emission budget, in 0..=100.
    pub foundation_share_percent: u8,
    pub subnet_min_stake: u128,
    pub max_stake: u128,
    pub max_subnet_min_stake: u128,
    pub min_subnet_delegate_stake: u128,
    pub overwatch_min_stake: u128,
    /// Additional reward per accepted subnet proposal, outside the annual budget.
    pub base_validator_reward: u128,
    pub max_slash: u128,
    pub base_node_burn: u128,
    pub initial_registration_cost: u128,
    pub min_registration_cost: u128,
}

impl NetworkEconomics {
    pub fn validate(&self) {
        assert!(self.terminal_annual_emissions <= self.initial_annual_emissions);
        assert!(self.annual_retention_percent <= 100);
        assert!(self.foundation_share_percent <= 100);
        assert!(self.subnet_min_stake > 0);
        assert!(self.subnet_min_stake <= self.max_subnet_min_stake);
        assert!(self.max_subnet_min_stake <= self.max_stake);
        assert!(self.min_subnet_delegate_stake > 0);
        assert!(self.overwatch_min_stake > 0);
        assert!(self.min_registration_cost > 0);
        assert!(self.min_registration_cost <= self.initial_registration_cost);
    }
}
