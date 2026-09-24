// SPDX-License-Identifier: Apache-2.0
pragma solidity >=0.8.24;

import "./NetworkTypes.sol";

/// Native TENSOR base units; all mutations act as the immediate caller.
interface IOverwatch {
    /// Network::register_overwatch_node (call index 71).
    function registerOverwatchNode(uint128 stakeToBeAdded) external;
    /// Network::remove_overwatch_node (call index 72).
    function removeOverwatchNode(uint32 overwatchNodeId) external;
    /// Network::update_overwatch_hotkey (call index 73).
    function updateOverwatchHotkey(uint32 overwatchNodeId, OptionalAccount calldata newHotkey) external;
    /// Network::set_overwatch_node_peer_id (call index 74).
    function setOverwatchNodePeerId(uint32 subnetId, uint32 overwatchNodeId, bytes calldata peerId) external;
    /// Network::commit_overwatch_subnet_weights (call index 75).
    function commitOverwatchSubnetWeights(uint32 overwatchNodeId, OverwatchCommit[] calldata commitWeights) external;
    /// Network::reveal_overwatch_subnet_weights (call index 76).
    function revealOverwatchSubnetWeights(uint32 overwatchNodeId, OverwatchReveal[] calldata reveals) external;

    // Bounded contract-focused queries.
    function getOverwatchNode(uint32 overwatchNodeId)
        external
        view
        returns (bool exists, OverwatchNodeInfo memory info);
    function getEpoch() external view returns (OverwatchEpoch memory);
    function getCommitment(uint32 overwatchNodeId, uint32 subnetId)
        external
        view
        returns (bool exists, bytes32 commitment);
    function getEffectiveSubnetWeight(uint32 subnetId) external view returns (EffectiveWeight memory);
}
