// SPDX-License-Identifier: Apache-2.0
pragma solidity >=0.8.24;

import "./NetworkTypes.sol";

/// Native TENSOR base units; all mutations act as the immediate caller.
interface IValidators {
    /// Network::register_validator (call index 0).
    function registerValidator(
        bytes32 hotkey,
        uint128 delegateRewardRate,
        OptionalDelegateAccount calldata delegateAccount,
        OptionalIdentity calldata identity
    ) external;
    /// Network::update_validator_coldkey (call index 1).
    function updateValidatorColdkey(uint32 validatorId, bytes32 newColdkey) external;
    /// Network::update_validator_hotkey (call index 2).
    function updateValidatorHotkey(uint32 validatorId, bytes32 newHotkey) external;
    /// Network::update_validator_delegate_reward_rate (call index 3).
    function updateValidatorDelegateRewardRate(uint32 validatorId, uint128 newDelegateRewardRate) external;
    /// Network::update_validator_delegate_account (call index 4).
    function updateValidatorDelegateAccount(
        uint32 validatorId,
        OptionalAccount calldata delegateAccountId,
        OptionalU128 calldata delegateRate
    ) external;
    /// Network::update_validator_identity (call index 5).
    function updateValidatorIdentity(uint32 validatorId, OptionalIdentity calldata identity) external;
    /// Network::set_validator_node_delegate_stake_weights (call index 172).
    function setValidatorNodeDelegateStakeWeights(NodeAllocation[] calldata updates) external;

    // Bounded contract-focused queries.
    function getValidator(uint32 validatorId) external view returns (bool exists, ValidatorInfo memory info);
    function getValidatorByColdkey(bytes32 accountId) external view returns (bool exists, ValidatorInfo memory info);
    function getValidatorByHotkey(bytes32 accountId) external view returns (bool exists, ValidatorInfo memory info);
    function getNodeAllocation(uint32 validatorId, uint32 subnetId, uint32 subnetNodeId)
        external
        view
        returns (bool exists, uint128 weight);
}
