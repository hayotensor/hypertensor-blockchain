// SPDX-License-Identifier: Apache-2.0
pragma solidity >=0.8.24;

import "./NetworkTypes.sol";

/// Native TENSOR base units; all mutations act as the immediate caller.
interface IConsensus {
    /// Network::propose_attestation (call index 69).
    function proposeAttestation(
        uint32 subnetId,
        NodeScore[] calldata data,
        OptionalU32 calldata prioritizeQueueNodeId,
        OptionalU32 calldata removeQueueNodeId,
        OptionalBytes calldata args,
        OptionalBytes calldata attestData
    ) external;
    /// Network::attest (call index 70).
    function attest(uint32 subnetId, uint32 subnetNodeId, OptionalBytes calldata data) external;

    // Bounded contract-focused queries.
    function getEpochStatus(uint32 subnetId) external view returns (bool exists, EpochStatus memory status);
    function getConsensusRound(uint32 subnetId, uint32 subnetEpoch)
        external
        view
        returns (bool exists, ConsensusRound memory round);
}
