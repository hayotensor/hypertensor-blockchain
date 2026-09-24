//! Consensus contract queries.
use super::*;

pub fn consensus<T: Config>(
    input: &abi::IConsensus::IConsensusCalls,
    env: &mut impl Ext<T = T>,
) -> Result<Vec<u8>, Error> {
    use abi::IConsensus::IConsensusCalls as C;
    // Epoch status reads bounded election sets. Avoid the RPC round's per-attestor expansion.
    charge::<T>(
        env,
        24,
        T::MaxSubnetNodesUpperBound::get()
            .saturating_mul(512)
            .saturating_add(16384),
    )?;
    Ok(match input {
        C::getEpochStatus(a) => {
            let status = match n::Pallet::<T>::rpc_get_subnet_epoch_status(a.subnetId) {
                Ok(v) => Some(v),
                Err(rpc::NetworkQueryError::SubnetNotFound { .. }) => None,
                Err(_) => return Err(convert::revert("Network: inconsistent query state")),
            };
            let exists = status.is_some();
            let value = status
                .map(|v| {
                    let has_timing = v.timing.is_some();
                    let timing = v.timing.unwrap_or_default();
                    abi::EpochStatus {
                        state: v.state as u8,
                        hasTiming: has_timing,
                        subnetEpoch: timing.subnet_epoch,
                        progression: timing.progression.0,
                        startBlock: timing.start_block,
                        endBlock: timing.end_block,
                        blocksRemaining: timing.blocks_remaining,
                        consensusEligible: v.consensus_eligible,
                        withinAttestationWindow: v.within_proposal_attestation_window,
                        electedNodeId: convert::u32_option_out(v.elected_validator_subnet_node_id),
                        proposalSubmitted: v.proposal_submitted,
                        validatorSetSource: v.validator_set_source as u8,
                        pendingEmergencySet: v.pending_emergency_set,
                    }
                })
                .unwrap_or_default();
            (exists, value).abi_encode_params()
        }
        C::getConsensusRound(a) => {
            let round = n::SubnetElectedValidator::<T>::get(a.subnetId, a.subnetEpoch);
            let exists = round.is_some();
            let value = round
                .map(|v| {
                    let proposed =
                        n::SubnetConsensusSubmission::<T>::contains_key(a.subnetId, a.subnetEpoch);
                    abi::ConsensusRound {
                        subnetId: a.subnetId,
                        subnetEpoch: a.subnetEpoch,
                        status: u8::from(proposed),
                        electionSource: u8::from(v.emergency.is_some()),
                        electedNodeId: v.validator_subnet_node_id,
                        electedValidatorId: v.validator_id,
                        delegateBalanceAtElection: v.validator_delegate_stake_balance,
                        proposalSubmitted: proposed,
                    }
                })
                .unwrap_or_default();
            (exists, value).abi_encode_params()
        }
        _ => return Err(unknown()),
    })
}
