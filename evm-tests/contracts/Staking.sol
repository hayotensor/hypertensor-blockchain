// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

interface Staking {
    struct QueuedSwapData {
        uint32 id;
        address accountId;
        uint8 callType;
        uint32 toValidatorId;
        uint32 toSubnetId;
        uint128 balance;
        uint32 queuedAtBlock;
        uint32 executeAfterBlocks;
    }

    function addNodeStake(
        uint256 subnetId,
        uint256 subnetNodeId,
        uint256 stakeToBeAdded
    ) external payable;

    function removeNodeStake(
        uint256 subnetId,
        uint256 subnetNodeId,
        uint256 stakeToBeRemoved
    ) external payable;

    function claimUnbondings() external payable;

    function addToDelegateStake(
        uint256 subnetId,
        uint256 stakeToBeAdded
    ) external payable;

    function swapDelegateStake(
        uint256 fromSubnetId,
        uint256 toSubnetId,
        uint256 delegateStakeSharesToSwap
    ) external payable;

    function transferDelegateStake(
        uint256 subnetId,
        address toAccount,
        uint256 delegateStakeSharesToTransfer
    ) external payable;

    function removeDelegateStake(
        uint256 subnetId,
        uint256 sharesToBeRemoved
    ) external payable;

    function increaseDelegateStake(
        uint256 subnetId,
        uint256 amount
    ) external payable;

    function addValidatorDelegateStake(
        uint256 validatorId,
        uint256 delegateStakeToBeAdded
    ) external payable;

    function swapNodeDelegateStake(
        uint256 fromValidatorId,
        uint256 toValidatorId,
        uint256 stakeToBeRemoved
    ) external payable;

    function transferValidatorDelegateStake(
        uint256 validatorId,
        address toAccount,
        uint256 validatorDelegateStakeSharesToTransfer
    ) external payable;

    function removeValidatorDelegateStake(
        uint256 validatorId,
        uint256 validatorDelegateStakeSharesToBeRemoved
    ) external payable;

    function donateValidatorDelegateStake(
        uint256 validatorId,
        uint256 amount
    ) external payable;

    function transferFromValidatorToSubnet(
        uint256 fromValidatorId,
        uint256 toSubnetId,
        uint256 nodeDelegateStakeSharesToSwap
    ) external payable;

    function transferFromSubnetToValidator(
        uint256 fromSubnetId,
        uint256 toValidatorId,
        uint256 subnetDelegateStakeSharesToSwap
    ) external payable;

    function updateSwapQueue(
        uint256 id,
        uint256 callType,
        uint256 toValidatorId,
        uint256 toSubnetId
    ) external payable;

    function removeDelegateAccountBalance(
        uint256 amountToRemove
    ) external payable;

    function getQueuedSwapCall(
        uint256 id
    ) external view returns (QueuedSwapData memory);

    function totalSubnetStake(
        uint256 subnetId
    ) external view returns (uint256);

    function nodeSubnetStake(
        uint256 subnetNodeId,
        uint256 subnetId
    ) external view returns (uint256);

    function totalSubnetDelegateStakeBalance(
        uint256 subnetId
    ) external view returns (uint256);

    function totalSubnetDelegateStakeShares(
        uint256 subnetId
    ) external view returns (uint256);

    function accountSubnetDelegateStakeShares(
        address account,
        uint256 subnetId
    ) external view returns (uint256);

    function accountSubnetDelegateStakeBalance(
        address account,
        uint256 subnetId
    ) external view returns (uint256);

}
