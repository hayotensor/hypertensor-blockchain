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
        uint128 minSharesOut;
        uint32 executeBeforeBlock;
        uint32 queuedAtBlock;
        uint32 executeAfterBlocks;
    }

    function addNodeStake(
        uint256 subnetId,
        uint256 subnetNodeId,
        uint256 stakeToBeAdded
    ) external;

    function removeNodeStake(
        uint256 subnetId,
        uint256 subnetNodeId,
        uint256 stakeToBeRemoved
    ) external;

    function claimUnbondings() external;

    function addToDelegateStake(
        uint256 subnetId,
        uint256 stakeToBeAdded,
        uint256 minSharesOut
    ) external;

    function swapDelegateStake(
        uint256 fromSubnetId,
        uint256 toSubnetId,
        uint256 delegateStakeSharesToSwap,
        uint256 minBalanceOut,
        uint256 minSharesOut,
        uint256 executeBeforeBlock
    ) external;

    function transferDelegateStake(
        uint256 subnetId,
        address toAccount,
        uint256 delegateStakeSharesToTransfer
    ) external;

    function removeDelegateStake(
        uint256 subnetId,
        uint256 sharesToBeRemoved,
        uint256 minBalanceOut
    ) external;

    function addValidatorDelegateStake(
        uint256 validatorId,
        uint256 delegateStakeToBeAdded,
        uint256 minSharesOut
    ) external;

    function swapNodeDelegateStake(
        uint256 fromValidatorId,
        uint256 toValidatorId,
        uint256 stakeToBeRemoved,
        uint256 minBalanceOut,
        uint256 minSharesOut,
        uint256 executeBeforeBlock
    ) external;

    function transferValidatorDelegateStake(
        uint256 validatorId,
        address toAccount,
        uint256 validatorDelegateStakeSharesToTransfer
    ) external;

    function removeValidatorDelegateStake(
        uint256 validatorId,
        uint256 validatorDelegateStakeSharesToBeRemoved,
        uint256 minBalanceOut
    ) external;

    function transferFromValidatorToSubnet(
        uint256 fromValidatorId,
        uint256 toSubnetId,
        uint256 nodeDelegateStakeSharesToSwap,
        uint256 minBalanceOut,
        uint256 minSharesOut,
        uint256 executeBeforeBlock
    ) external;

    function transferFromSubnetToValidator(
        uint256 fromSubnetId,
        uint256 toValidatorId,
        uint256 subnetDelegateStakeSharesToSwap,
        uint256 minBalanceOut,
        uint256 minSharesOut,
        uint256 executeBeforeBlock
    ) external;

    function updateSwapQueue(
        uint256 id,
        uint256 callType,
        uint256 toValidatorId,
        uint256 toSubnetId,
        uint256 minSharesOut,
        uint256 executeBeforeBlock
    ) external;

    function removeDelegateAccountBalance(
        uint256 amountToRemove
    ) external;

    function getQueuedSwapCall(
        uint256 id
    ) external view returns (QueuedSwapData memory);

    function totalSubnetStake(
        uint256 subnetId
    ) external view returns (uint256);

    function nodeSubnetStake(
        uint256 subnetId,
        uint256 subnetNodeId
    ) external view returns (uint256);

    function totalSubnetDelegateStakeBalance(
        uint256 subnetId
    ) external view returns (uint256);

    function totalSubnetDelegateStakeShares(
        uint256 subnetId
    ) external view returns (uint256);

    function totalValidatorDelegateStakeBalance(
        uint256 validatorId
    ) external view returns (uint256);

    function totalValidatorDelegateStakeShares(
        uint256 validatorId
    ) external view returns (uint256);

    function previewSubnetDelegateStakeDeposit(
        uint256 subnetId,
        uint256 assets
    ) external view returns (uint256 shares);

    function previewSubnetDelegateStakeRedeem(
        uint256 subnetId,
        uint256 shares
    ) external view returns (uint256 assets);

    function previewValidatorDelegateStakeDeposit(
        uint256 validatorId,
        uint256 assets
    ) external view returns (uint256 shares);

    function previewValidatorDelegateStakeRedeem(
        uint256 validatorId,
        uint256 shares
    ) external view returns (uint256 assets);

    function accountSubnetDelegateStakeShares(
        address account,
        uint256 subnetId
    ) external view returns (uint256);

    function accountSubnetDelegateStakeBalance(
        address account,
        uint256 subnetId
    ) external view returns (uint256);

    function accountValidatorDelegateStakeShares(
        address account,
        uint256 validatorId
    ) external view returns (uint256);

    function accountValidatorDelegateStakeBalance(
        address account,
        uint256 validatorId
    ) external view returns (uint256);

}
