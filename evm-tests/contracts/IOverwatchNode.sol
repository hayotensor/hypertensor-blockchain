// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

interface IOverwatchNode {
    struct OverwatchCommit {
        uint256 subnetId;
        bytes32 weight;
    }

    struct OverwatchReveal {
        uint256 subnetId;
        uint256 weight;
        uint8[] salt;
    }

    function registerOverwatchNode(
        uint256 stakeToBeAdded
    ) external payable;

    function removeOverwatchNode(uint256 overwatchNodeId) external;

    function updateOverwatchHotkey(
        uint256 overwatchNodeId,
        bool hasNewHotkey,
        address newHotkey
    ) external;

    function setOverwatchNodePeerId(
        uint256 subnetId,
        uint256 overwatchNodeId,
        string memory peerId
    ) external;

    function commitOverwatchSubnetWeights(
        uint256 overwatchNodeId,
        OverwatchCommit[] calldata commits
    ) external;

    function revealOverwatchSubnetWeights(
        uint256 overwatchNodeId,
        OverwatchReveal[] calldata reveals
    ) external;

    function addOverwatchStake(
        uint256 overwatchNodeId,
        uint256 stakeToBeAdded
    ) external payable;

    function removeOverwatchStake(
        uint256 overwatchNodeId,
        uint256 stakeToBeRemoved
    ) external;

    function accountOverwatchStake(
        uint256 overwatchNodeId
    ) external view returns (uint256);

    function totalOverwatchStake() external view returns (uint256);

    function validatorOverwatchNodeId(
        uint256 validatorId
    ) external view returns (bool exists, uint256 overwatchNodeId);

    function maxOverwatchNodes() external view returns (uint256);

    function totalOverwatchNodes() external view returns (uint256);

    function totalOverwatchNodeUids() external view returns (uint256);

    function overwatchEpochLengthMultiplier() external view returns (uint256);

    function overwatchEpochStartBlock() external view returns (uint256);

    function overwatchCommitCutoffPercent() external view returns (uint256);

    function lastFinalizedOverwatchEpoch()
        external
        view
        returns (bool exists, uint256 epoch);

    function overwatchNodes(
        uint256 overwatchNodeId
    ) external view returns (uint256, address);

    function overwatchNodeIdHotkey(
        uint256 overwatchNodeId
    ) external view returns (address);

    function peerIdOverwatchNode(
        uint256 subnetId,
        string memory peerId
    ) external view returns (uint256);

    /// @notice Ephemeral commit-row lookup. Entries disappear after a successful epoch close and
    ///         the call reverts when the requested record is absent.
    function overwatchCommits(
        uint256 overwatchEpoch,
        uint256 overwatchNodeId,
        uint256 subnetId
    ) external view returns (bytes32);

    /// @notice Ephemeral reveal-row lookup. Entries disappear after successful settlement or
    ///         structural node removal and the call reverts when the requested record is absent.
    function overwatchReveals(
        uint256 overwatchEpoch,
        uint256 subnetId,
        uint256 overwatchNodeId
    ) external view returns (uint256);

    function overwatchSubnetWeights(
        uint256 overwatchEpoch,
        uint256 subnetId
    ) external view returns (uint256);

    function overwatchNodeWeights(
        uint256 overwatchEpoch,
        uint256 overwatchNodeId
    ) external view returns (uint256);

    function effectiveOverwatchSignalMeta()
        external
        view
        returns (
            bool exists,
            uint256 sourceEpoch,
            uint256 revision,
            bool valid
        );

    function effectiveOverwatchSubnetWeight(
        uint256 subnetId
    )
        external
        view
        returns (
            bool rawWeightExists,
            uint256 rawWeight,
            uint256 resolvedWeight
        );

    function overwatchMinStakeBalance() external view returns (uint256);

    function getCurrentOverwatchEpoch() external view returns (uint256);
}
