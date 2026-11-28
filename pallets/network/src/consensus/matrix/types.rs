use crate::*;
use codec::{Decode, DecodeWithMemTracking, Encode};
use scale_info::TypeInfo;

pub const MATRIX_MAX_NODES: u32 = 64;
pub const MATRIX_MIN_NODES: u32 = 3;
pub type MatrixScores = BoundedVec<SubnetNodeConsensusData, sp_core::ConstU32<MATRIX_MAX_NODES>>;

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, Debug, TypeInfo)]
pub struct MatrixParameters {
    pub accuracy_weight: u128,
    /// Q18 disagreement boundary between accuracy rewards and penalties; strictly below Q.
    pub reputation_tolerance: u128,
    /// Maximum fraction of current reputation lost or gained in one round.
    pub max_reputation_decrease: u128,
    pub max_reputation_increase: u128,
    /// Positive integer powers controlling the reputation gain/loss curves.
    pub reputation_increase_exponent: u32,
    pub reputation_decrease_exponent: u32,
    /// Q18 fraction of the subnet epoch reserved for commits.
    pub commit_phase_percentage: u128,
    /// Performance must exceed this Q18 threshold to qualify as positive work.
    pub performance_threshold: u128,
    pub probation_promotion_reputation: u128,
    /// Operational population bounds, within the protocol's supported roster bounds.
    pub min_nodes: u32,
    pub max_nodes: u32,
}

impl MatrixParameters {
    pub(crate) fn is_valid(&self) -> bool {
        let q = PERCENTAGE_FACTOR_U128;
        self.accuracy_weight <= q
            && self.reputation_tolerance < q
            && self.max_reputation_decrease <= q
            && self.max_reputation_increase <= q
            && self.reputation_increase_exponent > 0
            && self.reputation_decrease_exponent > 0
            && self.commit_phase_percentage > 0
            && self.commit_phase_percentage < q
            && self.performance_threshold < q
            && self.probation_promotion_reputation <= q
            && self.min_nodes >= MATRIX_MIN_NODES
            && self.min_nodes <= self.max_nodes
            && self.max_nodes <= MATRIX_MAX_NODES
    }

    pub(crate) fn commit_blocks(&self, epoch_length: u32) -> Option<u32> {
        let blocks = (U256::from(epoch_length) * U256::from(self.commit_phase_percentage)
            / U256::from(PERCENTAGE_FACTOR_U128))
        .as_u32();
        (blocks > 0 && blocks < epoch_length).then_some(blocks)
    }
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, Debug, TypeInfo)]
pub struct MatrixRound {
    pub start_block: u32,
    pub reveal_block: u32,
    pub end_block: u32,
    pub nodes: Vec<SubnetNode>,
    /// Eligible scorer node ID -> owning validator ID, for authorization and stake allocation.
    /// The number of distinct validator IDs imposes no Matrix consensus requirement.
    pub identities: BTreeMap<u32, u32>,
    pub stake: ConsensusAttestorWeightSnapshot,
    pub policy: ConsensusPolicySnapshot,
    pub parameters: MatrixParameters,
}

impl MatrixRound {
    /// Partial, sorted target lists use absolute Q18 scores. Empty means abstention;
    /// an explicit zero is an assessment. Both reveal and settlement use this contract.
    pub(crate) fn valid_scores(&self, scorer: u32, scores: &MatrixScores) -> bool {
        scores
            .windows(2)
            .all(|pair| pair[0].subnet_node_id < pair[1].subnet_node_id)
            && scores.iter().all(|score| {
                score.subnet_node_id != scorer
                    && score.score <= PERCENTAGE_FACTOR_U128
                    && self
                        .nodes
                        .binary_search_by_key(&score.subnet_node_id, |n| n.id)
                        .is_ok()
            })
    }
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, Debug, TypeInfo)]
pub enum MatrixOutcome {
    Accepted,
    NoScores,
    Cancelled,
}

#[derive(Default, Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, Debug, TypeInfo)]
pub struct MatrixNodeResult {
    pub performance: u128,
    /// At least one other node with positive voting weight assessed this node.
    /// True even for an explicit zero score; false for an unavailable assessment.
    pub has_received_assessment: bool,
    pub accuracy: u128,
    /// Distinguishes maximal disagreement from an unavailable independent reference.
    pub has_accuracy_reference: bool,
    pub weight: u128,
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, Debug, TypeInfo)]
pub struct MatrixResult {
    pub outcome: MatrixOutcome,
    pub revealed_weight: u128,
    pub revealed_nodes: u32,
    pub nodes: BTreeMap<u32, MatrixNodeResult>,
}
