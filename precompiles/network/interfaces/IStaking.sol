// SPDX-License-Identifier: Apache-2.0
pragma solidity >=0.8.24;

import "./NetworkTypes.sol";

/// Native TENSOR base units; all mutations act as the immediate caller.
interface IStaking {
    /// Network::add_node_stake (call index 52).
    function addNodeStake(uint32 subnetId, uint32 subnetNodeId, uint128 stakeToBeAdded) external;
    /// Network::remove_node_stake (call index 53).
    function removeNodeStake(uint32 subnetId, uint32 subnetNodeId, uint128 stakeToBeRemoved) external;
    /// Network::add_subnet_delegate_stake (call index 54).
    function addSubnetDelegateStake(uint32 subnetId, uint128 stakeToBeAdded, uint128 minSharesOut) external;
    /// Network::swap_from_subnet_to_subnet (call index 55).
    function swapFromSubnetToSubnet(
        uint32 fromSubnetId,
        uint32 toSubnetId,
        uint128 delegateStakeSharesToSwap,
        uint128 minBalanceOut,
        uint128 minSharesOut,
        uint32 executeBeforeBlock
    ) external;
    /// Network::transfer_delegate_stake (call index 56).
    function transferDelegateStake(uint32 subnetId, bytes32 toAccountId, uint128 delegateStakeSharesToTransfer)
        external;
    /// Network::remove_delegate_stake (call index 57).
    function removeDelegateStake(uint32 subnetId, uint128 sharesToBeRemoved, uint128 minBalanceOut) external;
    /// Network::add_validator_delegate_stake (call index 59).
    function addValidatorDelegateStake(uint32 validatorId, uint128 delegateStakeToBeAdded, uint128 minSharesOut)
        external;
    /// Network::transfer_validator_delegate_stake (call index 60).
    function transferValidatorDelegateStake(
        uint32 validatorId,
        bytes32 toAccountId,
        uint128 validatorDelegateStakeSharesToTransfer
    ) external;
    /// Network::remove_validator_delegate_stake (call index 61).
    function removeValidatorDelegateStake(
        uint32 validatorId,
        uint128 validatorDelegateStakeSharesToBeRemoved,
        uint128 minBalanceOut
    ) external;
    /// Network::swap_from_validator_to_validator (call index 62).
    function swapFromValidatorToValidator(
        uint32 fromValidatorId,
        uint32 toValidatorId,
        uint128 stakeToBeRemoved,
        uint128 minBalanceOut,
        uint128 minSharesOut,
        uint32 executeBeforeBlock
    ) external;
    /// Network::swap_from_validator_to_subnet (call index 64).
    function swapFromValidatorToSubnet(
        uint32 fromValidatorId,
        uint32 toSubnetId,
        uint128 nodeDelegateStakeSharesToSwap,
        uint128 minBalanceOut,
        uint128 minSharesOut,
        uint32 executeBeforeBlock
    ) external;
    /// Network::swap_from_subnet_to_validator (call index 65).
    function swapFromSubnetToValidator(
        uint32 fromSubnetId,
        uint32 toValidatorId,
        uint128 subnetDelegateStakeSharesToSwap,
        uint128 minBalanceOut,
        uint128 minSharesOut,
        uint32 executeBeforeBlock
    ) external;
    /// Network::update_swap_queue (call index 66).
    function updateSwapQueue(uint32 id, QueuedSwap calldata newCall) external;
    /// Network::remove_delegate_account_balance (call index 67).
    function removeDelegateAccountBalance(uint128 amountToRemove) external;
    /// Network::claim_unbondings (call index 68).
    function claimUnbondings() external;
    /// Network::add_overwatch_node_stake (call index 77).
    function addOverwatchNodeStake(uint32 overwatchNodeId, uint128 stakeToBeAdded) external;
    /// Network::remove_overwatch_node_stake (call index 78).
    function removeOverwatchNodeStake(uint32 overwatchNodeId, uint128 stakeToBeRemoved) external;

    // Bounded contract-focused queries.
    function getNodeStake(uint32 subnetId, uint32 subnetNodeId) external view returns (uint128);
    function getOverwatchStake(uint32 overwatchNodeId) external view returns (uint128);
    function getSubnetPool(uint32 subnetId, bytes32 accountId) external view returns (Pool memory);
    function getValidatorPool(uint32 validatorId, bytes32 accountId) external view returns (Pool memory);
    function getDelegateAccountBalance(bytes32 accountId) external view returns (uint128);
    function getUnbondings(bytes32 accountId) external view returns (Unbonding[] memory);
    function getSwapRefund(bytes32 accountId) external view returns (uint128);
    function getQueuedSwap(uint32 id) external view returns (bool exists, QueuedSwapInfo memory info);
}
