// SPDX-License-Identifier: Apache-2.0
pragma solidity >=0.8.24;

import "./NetworkTypes.sol";

/// Native TENSOR base units; all mutations act as the immediate caller.
interface ISubnetNodes {
    /// Network::register_subnet_node (call index 44).
    function registerSubnetNode(
        uint32 validatorId,
        uint32 subnetId,
        OptionalAccount calldata hotkey,
        OptionalPeerInfo calldata peerInfo,
        OptionalPeerInfo calldata bootnodePeerInfo,
        OptionalPeerInfo calldata clientPeerInfo,
        uint128 stakeToBeAdded,
        OptionalBytes calldata unique,
        OptionalBytes calldata nonUnique,
        uint128 maxBurnAmount
    ) external;
    /// Network::update_node_hotkey (call index 45).
    function updateNodeHotkey(uint32 subnetId, uint32 subnetNodeId, OptionalAccount calldata newHotkey) external;
    /// Network::update_node_peer_info (call index 46).
    function updateNodePeerInfo(uint32 subnetId, uint32 subnetNodeId, OptionalPeerInfo calldata newPeerInfo) external;
    /// Network::update_node_bootnode_peer_info (call index 47).
    function updateNodeBootnodePeerInfo(uint32 subnetId, uint32 subnetNodeId, OptionalPeerInfo calldata newPeerInfo)
        external;
    /// Network::update_node_client_peer_info (call index 48).
    function updateNodeClientPeerInfo(uint32 subnetId, uint32 subnetNodeId, OptionalPeerInfo calldata newPeerInfo)
        external;
    /// Network::update_node_unique (call index 49).
    function updateNodeUnique(uint32 subnetId, uint32 subnetNodeId, OptionalBytes calldata unique) external;
    /// Network::update_node_non_unique (call index 50).
    function updateNodeNonUnique(uint32 subnetId, uint32 subnetNodeId, OptionalBytes calldata nonUnique) external;
    /// Network::remove_subnet_node (call index 51).
    function removeSubnetNode(uint32 subnetId, uint32 subnetNodeId) external;

    // Bounded contract-focused queries.
    function getSubnetNode(uint32 subnetId, uint32 subnetNodeId)
        external
        view
        returns (bool exists, SubnetNodeInfo memory info);
    function getAssociatedValidator(uint32 subnetId, uint32 subnetNodeId) external view returns (OptionalU32 memory);
}
