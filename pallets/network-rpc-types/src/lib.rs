//! Shared wire types for the network runtime API and JSON-RPC server.
//!
//! These types deliberately contain no FRAME configuration parameters. The network pallet can
//! construct them, the runtime API can SCALE encode them, and the native RPC server can serialize
//! the same values to JSON without a conversion layer or a circular crate dependency.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// Default number of records returned by a paged network query.
pub const DEFAULT_PAGE_SIZE: u32 = 50;

/// Hard ceiling for any caller-controlled network query page.
pub const MAX_PAGE_SIZE: u32 = 100;

/// A SCALE-native `u128` that is represented as a decimal string in JSON.
///
/// JSON numbers cannot safely represent all values used for balances, shares, percentages, and
/// reputation. Deserialization intentionally accepts only a string so the contract cannot become
/// dependent on a client's numeric precision.
#[derive(
    Default,
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
#[serde(transparent)]
pub struct RpcU128(#[serde(with = "decimal_u128")] pub u128);

impl From<u128> for RpcU128 {
    fn from(value: u128) -> Self {
        Self(value)
    }
}

impl From<RpcU128> for u128 {
    fn from(value: RpcU128) -> Self {
        value.0
    }
}

/// Arbitrary bytes represented as a `0x`-prefixed hex string in JSON.
#[derive(
    Default,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
#[serde(transparent)]
pub struct RpcBytes(#[serde(with = "hex_bytes")] pub Vec<u8>);

impl From<Vec<u8>> for RpcBytes {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl From<RpcBytes> for Vec<u8> {
    fn from(value: RpcBytes) -> Self {
        value.0
    }
}

/// Input shared by every paginated method.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageRequest<Cursor> {
    pub cursor: Option<Cursor>,
    pub limit: u32,
}

impl<Cursor> Default for PageRequest<Cursor> {
    fn default() -> Self {
        Self {
            cursor: None,
            limit: DEFAULT_PAGE_SIZE,
        }
    }
}

impl<Cursor> PageRequest<Cursor> {
    /// Validate the caller-controlled page size before any storage iteration starts.
    pub fn validated_limit(&self) -> Result<usize, NetworkQueryError> {
        if self.limit == 0 || self.limit > MAX_PAGE_SIZE {
            return Err(NetworkQueryError::InvalidPageLimit {
                requested: self.limit,
                max: MAX_PAGE_SIZE,
            });
        }
        Ok(self.limit as usize)
    }
}

/// Output shared by every paginated method.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<Item, Cursor> {
    pub items: Vec<Item>,
    pub next_cursor: Option<Cursor>,
}

/// Continuation cursor for lists keyed by `(subnet_id, subnet_node_id)`.
///
/// Callers must treat this value as opaque: some endpoints traverse storage-key order rather than
/// numeric tuple order.
#[derive(
    Default,
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct SubnetNodeCursor {
    pub subnet_id: u32,
    pub subnet_node_id: u32,
}

/// Domain failures returned by the runtime query before the native RPC layer maps them to a JSON
/// error object.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "details",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum NetworkQueryError {
    InvalidPageLimit { requested: u32, max: u32 },
    SubnetNotFound { subnet_id: u32 },
    ValidatorNotFound { validator_id: u32 },
    InconsistentState,
}

#[derive(
    Default,
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum ConsensusMechanism {
    #[default]
    Attestation,
}

#[derive(
    Default,
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum SubnetState {
    #[default]
    Registered,
    Active,
    Paused,
}

#[derive(
    Default,
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum SubnetNodeClass {
    #[default]
    Registered,
    Idle,
    Included,
    Validator,
}

mod decimal_u128 {
    use alloc::string::{String, ToString};
    use core::str::FromStr;
    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        u128::from_str(&value).map_err(D::Error::custom)
    }
}

mod hex_bytes {
    use alloc::{string::String, vec::Vec};
    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        impl_serde::serialize::serialize(value, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let Some(hex) = value.strip_prefix("0x") else {
            return Err(D::Error::custom("expected a 0x-prefixed hex string"));
        };
        if hex.len() % 2 != 0 {
            return Err(D::Error::custom("hex byte strings must contain full bytes"));
        }
        impl_serde::serialize::from_hex(&value).map_err(D::Error::custom)
    }
}

// Subnet wire types.

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingValueUpdate<Value, AccountId> {
    pub value: Value,
    pub effective_subnet_epoch: u32,
    pub owner: AccountId,
}

#[derive(
    Default, Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct SubnetReputationFactors {
    pub absent_decrease: RpcU128,
    pub included_increase: RpcU128,
    pub below_min_weight_decrease: RpcU128,
    pub non_attestor_decrease: RpcU128,
    pub non_consensus_attestor_decrease: RpcU128,
    pub validator_absent_decrease: RpcU128,
    pub validator_non_consensus_decrease: RpcU128,
}

#[derive(
    Default, Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct PendingSubnetReputationFactors {
    pub effective_subnet_epoch: u32,
    pub factors: SubnetReputationFactors,
}

#[derive(
    Default, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct BootnodeInfo {
    pub peer_id: RpcBytes,
    pub multiaddr: Option<RpcBytes>,
}

/// Stable, bounded public view of a subnet. High-cardinality registration-whitelist entries are
/// intentionally excluded and should be read through archive-node storage queries or an indexer.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubnetInfo<AccountId> {
    pub id: u32,
    pub friendly_id: Option<u32>,
    pub name: RpcBytes,
    pub repo: RpcBytes,
    pub description: RpcBytes,
    pub misc: RpcBytes,
    pub consensus_mechanism: ConsensusMechanism,
    pub state: SubnetState,
    pub consensus_eligible_from_subnet_epoch: Option<u32>,
    pub pause_started_global_epoch: Option<u32>,
    pub pause_started_subnet_epoch: Option<u32>,
    pub churn_limit: u32,
    pub churn_limit_multiplier: u32,
    pub min_stake: RpcU128,
    pub max_stake: RpcU128,
    pub queue_immunity_epochs: u32,
    pub pending_queue_immunity_epochs: Option<PendingValueUpdate<u32, AccountId>>,
    pub target_node_registrations_per_epoch: u32,
    pub node_registrations_this_epoch: u32,
    pub subnet_node_queue_epochs: u32,
    pub pending_subnet_node_queue_epochs: Option<PendingValueUpdate<u32, AccountId>>,
    pub idle_classification_epochs: u32,
    pub pending_idle_classification_epochs: Option<PendingValueUpdate<u32, AccountId>>,
    pub included_classification_epochs: u32,
    pub pending_included_classification_epochs: Option<PendingValueUpdate<u32, AccountId>>,
    pub delegate_stake_percentage: RpcU128,
    pub pending_delegate_stake_percentage: Option<PendingValueUpdate<RpcU128, AccountId>>,
    pub last_delegate_stake_rewards_update: u32,
    pub consensus_validator_node_count_decay: RpcU128,
    pub pending_consensus_validator_node_count_decay:
        Option<PendingValueUpdate<RpcU128, AccountId>>,
    pub last_consensus_validator_node_count_decay_update: Option<u32>,
    pub consensus_validator_stake_weight_power: RpcU128,
    pub pending_consensus_validator_stake_weight_power:
        Option<PendingValueUpdate<RpcU128, AccountId>>,
    pub last_consensus_validator_stake_weight_power_update: Option<u32>,
    pub node_burn_rate_alpha: RpcU128,
    pub current_node_burn_rate: RpcU128,
    pub max_registered_nodes: u32,
    pub owner: Option<AccountId>,
    pub pending_owner: Option<AccountId>,
    pub registration_epoch: Option<u32>,
    pub slot_index: Option<u32>,
    pub subnet_node_min_weight_decrease_reputation_threshold: RpcU128,
    pub pending_subnet_node_min_weight_decrease_reputation_threshold:
        Option<PendingValueUpdate<RpcU128, AccountId>>,
    pub reputation: RpcU128,
    pub min_subnet_node_reputation: RpcU128,
    pub pending_min_subnet_node_reputation: Option<PendingValueUpdate<RpcU128, AccountId>>,
    pub reputation_factors: SubnetReputationFactors,
    pub pending_reputation_factors: Option<PendingSubnetReputationFactors>,
    pub bootnode_access: Vec<AccountId>,
    pub bootnodes: Vec<BootnodeInfo>,
    pub total_nodes: u32,
    pub total_active_nodes: u32,
    pub total_electable_nodes: u32,
    pub current_min_delegate_stake: RpcU128,
    pub total_subnet_stake: RpcU128,
    pub total_subnet_delegate_stake_shares: RpcU128,
    pub total_subnet_delegate_stake_balance: RpcU128,
}

#[derive(
    Default, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct SubnetBootnodes {
    pub official: Vec<BootnodeInfo>,
    pub active_nodes: Vec<BootnodeInfo>,
}

#[derive(
    Default, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub peer_id: RpcBytes,
    pub multiaddr: Option<RpcBytes>,
}

#[derive(
    Default, Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct SubnetNodeClassification {
    pub node_class: SubnetNodeClass,
    pub start_epoch: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubnetNodeInfo<AccountId> {
    pub validator_id: u32,
    pub subnet_id: u32,
    pub subnet_node_id: u32,
    pub coldkey: AccountId,
    pub hotkey: AccountId,
    pub peer_info: Option<PeerInfo>,
    pub bootnode_peer_info: Option<PeerInfo>,
    pub client_peer_info: Option<PeerInfo>,
    pub classification: SubnetNodeClassification,
    pub unique: Option<RpcBytes>,
    pub non_unique: Option<RpcBytes>,
    pub stake_balance: RpcU128,
    pub subnet_node_reputation: Option<RpcU128>,
    pub node_slot_index: Option<u32>,
    pub consecutive_idle_epochs: u32,
    pub consecutive_included_epochs: u32,
}

// Validator and stake wire types.

#[derive(
    Default, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInfo {
    pub name: Option<RpcBytes>,
    pub url: Option<RpcBytes>,
    pub image: Option<RpcBytes>,
    pub discord: Option<RpcBytes>,
    pub x: Option<RpcBytes>,
    pub telegram: Option<RpcBytes>,
    pub github: Option<RpcBytes>,
    pub hugging_face: Option<RpcBytes>,
    pub description: Option<RpcBytes>,
    pub misc: Option<RpcBytes>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegateAccountInfo<AccountId> {
    pub account_id: AccountId,
    pub rate: RpcU128,
}

#[derive(
    Default, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorReputationInfo {
    pub start_epoch: Option<u32>,
    pub score: RpcU128,
    pub lifetime_node_count: u32,
    pub total_active_nodes: u32,
    pub total_increases: u32,
    pub total_decreases: u32,
    pub average_proposal_identity_support: RpcU128,
    pub identity_support_samples: u32,
    pub last_validator_epoch: Option<u32>,
    pub overwatch_score: RpcU128,
}

#[derive(
    Default, Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorNodeAllocation {
    pub subnet_id: u32,
    pub subnet_node_id: u32,
    pub weight: RpcU128,
}

/// Validator identity and its current economic state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorInfo<AccountId> {
    pub id: u32,
    pub coldkey: AccountId,
    pub hotkey: AccountId,
    pub delegate_reward_rate: RpcU128,
    pub last_delegate_reward_rate_update: u32,
    pub delegate_account: Option<DelegateAccountInfo<AccountId>>,
    pub identity: Option<IdentityInfo>,
    pub reputation: ValidatorReputationInfo,
    pub delegate_pool_shares: RpcU128,
    pub delegate_pool_balance: RpcU128,
    pub delegate_pool_slash_lock_until: u32,
    pub last_node_allocation_update: Option<u32>,
}

#[derive(
    Default, Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorNodeStakeInfo {
    pub subnet_id: u32,
    pub subnet_node_id: u32,
    pub balance: RpcU128,
}

// Overwatch wire types.

#[derive(
    Default, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct OverwatchPeerInfo {
    pub subnet_id: u32,
    pub peer_id: RpcBytes,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverwatchNodeInfo<AccountId> {
    pub overwatch_node_id: u32,
    pub validator_id: u32,
    pub coldkey: AccountId,
    pub hotkey: AccountId,
    pub peer_ids: Vec<OverwatchPeerInfo>,
    pub reputation: ValidatorReputationInfo,
    pub stake_balance: RpcU128,
}

// Consensus wire types.

#[derive(
    Default, Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum ConsensusElectionSource {
    #[default]
    Regular,
    Emergency,
}

#[derive(
    Default, Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum ConsensusRoundStatus {
    #[default]
    Elected,
    Proposed,
}

#[derive(
    Default, Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusPolicyInfo {
    pub min_attestation_percentage: RpcU128,
    pub super_majority_attestation_ratio: RpcU128,
    pub base_validator_reward: RpcU128,
    pub subnet_owner_percentage: RpcU128,
    pub validator_reward_k: u64,
    pub validator_reward_midpoint: RpcU128,
    pub attestor_reward_exponent: u64,
    pub attestor_min_reward_factor: RpcU128,
    pub base_slash_percentage: RpcU128,
    pub max_slash_amount: RpcU128,
    pub validator_delegate_stake_slash_threshold: RpcU128,
    pub base_validator_delegate_stake_slash_percentage: RpcU128,
    pub max_validator_delegate_stake_slash_amount: RpcU128,
    pub validator_reputation_increase_factor: RpcU128,
    pub validator_reputation_decrease_factor: RpcU128,
    pub validator_absent_subnet_reputation_factor: RpcU128,
    pub in_consensus_subnet_reputation_factor: RpcU128,
    pub not_in_consensus_subnet_reputation_factor: RpcU128,
    pub min_subnet_nodes: u32,
    pub validator_identity_attestation_percentage: RpcU128,
    pub min_subnet_node_reputation: RpcU128,
    pub min_weight_decrease_reputation_threshold: RpcU128,
    pub subnet_delegate_stake_rewards_percentage: RpcU128,
    pub consensus_validator_node_count_decay: RpcU128,
    pub consensus_validator_stake_weight_power: RpcU128,
    pub idle_classification_epochs: u32,
    pub included_classification_epochs: u32,
    pub queue_immunity_epochs: u32,
    pub reputation_factors: SubnetReputationFactors,
}

#[derive(
    Default, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct EmergencyConsensusInfo {
    pub subnet_node_ids: Vec<u32>,
    pub reputation_factors: SubnetReputationFactors,
    pub min_subnet_node_reputation: RpcU128,
    pub min_weight_decrease_reputation_threshold: RpcU128,
}

#[derive(
    Default, Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusNodeScore {
    pub subnet_node_id: u32,
    pub score: RpcU128,
}

#[derive(
    Default, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusAttestationInfo {
    pub block: u32,
    pub progress: RpcU128,
    pub reward_factor: RpcU128,
    pub data: Option<RpcBytes>,
}

#[derive(
    Default, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusAttestorInfo {
    pub subnet_node_id: u32,
    pub validator_id: u32,
    pub weight: RpcU128,
    pub attestation: Option<ConsensusAttestationInfo>,
}

#[derive(
    Default, Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusCandidateInfo {
    pub subnet_node_id: u32,
    pub validator_id: u32,
}

#[derive(
    Default, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusProposalInfo {
    pub block: u32,
    pub validator_epoch_progress: RpcU128,
    pub validator_reward_factor: RpcU128,
    pub eligible_attestors: Vec<ConsensusAttestorInfo>,
    pub active_subnet_node_ids: Vec<u32>,
    pub prioritize_queue_node_id: Option<u32>,
    pub remove_queue_node_id: Option<u32>,
    pub scores: Vec<ConsensusNodeScore>,
    pub args: Option<RpcBytes>,
    pub emergency: Option<EmergencyConsensusInfo>,
}

/// Historical election and proposal snapshots. It never fetches mutable current node data, so a
/// query for an old epoch remains stable if the elected node is updated or removed later.
#[derive(
    Default, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusRoundInfo {
    pub subnet_id: u32,
    pub subnet_epoch: u32,
    pub status: ConsensusRoundStatus,
    pub election_source: ConsensusElectionSource,
    pub elected_subnet_node_id: u32,
    pub elected_validator_id: u32,
    pub validator_delegate_balance_at_election: RpcU128,
    pub election_candidates: Vec<ConsensusCandidateInfo>,
    pub policy: ConsensusPolicyInfo,
    pub proposal: Option<ConsensusProposalInfo>,
}

// Epoch/status wire types.

#[derive(
    Default, Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct SubnetEpochTiming {
    pub subnet_epoch: u32,
    pub progression: RpcU128,
    pub start_block: u32,
    pub end_block: u32,
    pub blocks_remaining: u32,
}

#[derive(
    Default, Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct SubnetEpochStatus {
    pub subnet_id: u32,
    pub state: SubnetState,
    pub timing: Option<SubnetEpochTiming>,
    pub consensus_eligible: bool,
    pub within_proposal_attestation_window: bool,
    pub elected_validator_subnet_node_id: Option<u32>,
    pub proposal_submitted: bool,
    pub validator_set_source: ConsensusElectionSource,
    pub pending_emergency_set: bool,
}

// Convenient concrete page aliases used by the runtime API trait and RPC surface.

pub type SubnetsPage<AccountId> = Page<SubnetInfo<AccountId>, u32>;
pub type SubnetNodesPage<AccountId> = Page<SubnetNodeInfo<AccountId>, u32>;
pub type ValidatorNodesPage<AccountId> = Page<SubnetNodeInfo<AccountId>, SubnetNodeCursor>;
pub type ValidatorNodeStakesPage = Page<ValidatorNodeStakeInfo, SubnetNodeCursor>;
pub type ValidatorNodeAllocationsPage = Page<ValidatorNodeAllocation, SubnetNodeCursor>;
pub type SubnetValidatorNodesPage<AccountId> = Page<SubnetNodeInfo<AccountId>, u32>;
pub type OverwatchNodesPage<AccountId> = Page<OverwatchNodeInfo<AccountId>, u32>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_u128_is_a_decimal_json_string_and_round_trips_scale() {
        let value = RpcU128(u128::MAX);
        assert_eq!(
            serde_json::to_string(&value).expect("serializes"),
            format!("\"{}\"", u128::MAX)
        );
        assert_eq!(
            serde_json::from_str::<RpcU128>(&format!("\"{}\"", u128::MAX)).expect("deserializes"),
            value
        );
        assert!(serde_json::from_str::<RpcU128>("1").is_err());
        assert_eq!(RpcU128::decode(&mut &value.encode()[..]).unwrap(), value);
    }

    #[test]
    fn rpc_bytes_are_hex_in_json_and_plain_bytes_in_scale() {
        let value = RpcBytes(vec![0, 1, 0xfe, 0xff]);
        assert_eq!(
            serde_json::to_string(&value).expect("serializes"),
            "\"0x0001feff\""
        );
        assert_eq!(
            serde_json::from_str::<RpcBytes>("\"0x0001feff\"").expect("deserializes"),
            value
        );
        assert!(serde_json::from_str::<RpcBytes>("\"0001feff\"").is_err());
        assert!(serde_json::from_str::<RpcBytes>("[0,1,254,255]").is_err());
        assert_eq!(RpcBytes::decode(&mut &value.encode()[..]).unwrap(), value);
    }

    #[test]
    fn page_limits_are_bounded() {
        assert_eq!(PageRequest::<u32>::default().validated_limit(), Ok(50));
        assert!(PageRequest::<u32> {
            cursor: None,
            limit: 0,
        }
        .validated_limit()
        .is_err());
        assert!(PageRequest::<u32> {
            cursor: None,
            limit: MAX_PAGE_SIZE + 1,
        }
        .validated_limit()
        .is_err());
    }

    #[test]
    fn page_and_error_json_names_are_stable() {
        assert_eq!(
            serde_json::to_value(Page::<u32, SubnetNodeCursor> {
                items: vec![7],
                next_cursor: Some(SubnetNodeCursor {
                    subnet_id: 9,
                    subnet_node_id: 11,
                }),
            })
            .expect("serializes"),
            serde_json::json!({
                "items": [7],
                "nextCursor": { "subnetId": 9, "subnetNodeId": 11 }
            })
        );

        assert_eq!(
            serde_json::to_value(NetworkQueryError::SubnetNotFound { subnet_id: 5 })
                .expect("serializes"),
            serde_json::json!({ "type": "subnetNotFound", "details": { "subnetId": 5 } })
        );
    }
}
