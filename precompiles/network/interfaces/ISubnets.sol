// SPDX-License-Identifier: Apache-2.0
pragma solidity >=0.8.24;

import "./NetworkTypes.sol";

/// Native TENSOR base units; all mutations act as the immediate caller.
interface ISubnets {
    /// Network::register_subnet (call index 6).
    function registerSubnet(uint128 maxCost, SubnetRegistration calldata subnetData) external;
    /// Network::activate_subnet (call index 7).
    function activateSubnet(uint32 subnetId) external;
    /// Network::owner_pause_subnet (call index 8).
    function ownerPauseSubnet(uint32 subnetId) external;
    /// Network::owner_unpause_subnet (call index 9).
    function ownerUnpauseSubnet(uint32 subnetId) external;
    /// Network::owner_deactivate_subnet (call index 10).
    function ownerDeactivateSubnet(uint32 subnetId) external;
    /// Network::owner_update_name (call index 11).
    function ownerUpdateName(uint32 subnetId, bytes calldata value) external;
    /// Network::owner_update_repo (call index 12).
    function ownerUpdateRepo(uint32 subnetId, bytes calldata value) external;
    /// Network::owner_update_description (call index 13).
    function ownerUpdateDescription(uint32 subnetId, bytes calldata value) external;
    /// Network::owner_update_misc (call index 14).
    function ownerUpdateMisc(uint32 subnetId, bytes calldata value) external;
    /// Network::owner_update_churn_limit (call index 15).
    function ownerUpdateChurnLimit(uint32 subnetId, uint32 value) external;
    /// Network::owner_update_churn_limit_multiplier (call index 16).
    function ownerUpdateChurnLimitMultiplier(uint32 subnetId, uint32 value) external;
    /// Network::owner_update_registration_queue_epochs (call index 17).
    function ownerUpdateRegistrationQueueEpochs(uint32 subnetId, uint32 value) external;
    /// Network::owner_update_idle_classification_epochs (call index 18).
    function ownerUpdateIdleClassificationEpochs(uint32 subnetId, uint32 value) external;
    /// Network::owner_update_included_classification_epochs (call index 19).
    function ownerUpdateIncludedClassificationEpochs(uint32 subnetId, uint32 value) external;
    /// Network::owner_update_subnet_node_min_weight_decrease_reputation_threshold (call index 27).
    function ownerUpdateSubnetNodeMinWeightDecreaseReputationThreshold(uint32 subnetId, uint128 value) external;
    /// Network::owner_update_min_subnet_node_reputation (call index 28).
    function ownerUpdateMinSubnetNodeReputation(uint32 subnetId, uint128 value) external;
    /// Network::owner_add_or_update_initial_validators (call index 29).
    function ownerAddOrUpdateInitialValidators(uint32 subnetId, U32Entry[] calldata validators) external;
    /// Network::owner_remove_initial_validators (call index 30).
    function ownerRemoveInitialValidators(uint32 subnetId, uint32[] calldata validators) external;
    /// Network::owner_set_emergency_validator_set (call index 31).
    function ownerSetEmergencyValidatorSet(uint32 subnetId, uint32[] calldata subnetNodeIds) external;
    /// Network::owner_revert_emergency_validator_set (call index 32).
    function ownerRevertEmergencyValidatorSet(uint32 subnetId) external;
    /// Network::owner_update_min_max_stake (call index 33).
    function ownerUpdateMinMaxStake(uint32 subnetId, uint128 min, uint128 max) external;
    /// Network::owner_update_delegate_stake_percentage (call index 34).
    function ownerUpdateDelegateStakePercentage(uint32 subnetId, uint128 value) external;
    /// Network::owner_update_max_registered_nodes (call index 35).
    function ownerUpdateMaxRegisteredNodes(uint32 subnetId, uint32 value) external;
    /// Network::transfer_subnet_ownership (call index 36).
    function transferSubnetOwnership(uint32 subnetId, bytes32 newOwner) external;
    /// Network::accept_subnet_ownership (call index 37).
    function acceptSubnetOwnership(uint32 subnetId) external;
    /// Network::owner_add_bootnode_access (call index 38).
    function ownerAddBootnodeAccess(uint32 subnetId, bytes32 newAccount) external;
    /// Network::owner_remove_bootnode_access (call index 39).
    function ownerRemoveBootnodeAccess(uint32 subnetId, bytes32 removeAccount) external;
    /// Network::owner_update_target_node_registrations_per_epoch (call index 40).
    function ownerUpdateTargetNodeRegistrationsPerEpoch(uint32 subnetId, uint32 value) external;
    /// Network::owner_update_node_burn_rate_alpha (call index 41).
    function ownerUpdateNodeBurnRateAlpha(uint32 subnetId, uint128 value) external;
    /// Network::owner_update_queue_immunity_epochs (call index 42).
    function ownerUpdateQueueImmunityEpochs(uint32 subnetId, uint32 value) external;
    /// Network::update_bootnodes (call index 43).
    function updateBootnodes(uint32 subnetId, Bootnode[] calldata add, bytes[] calldata remove) external;
    /// Network::owner_update_reputation_factors (call index 169).
    function ownerUpdateReputationFactors(uint32 subnetId, ReputationUpdates calldata updates) external;
    /// Network::owner_update_consensus_validator_node_count_decay (call index 170).
    function ownerUpdateConsensusValidatorNodeCountDecay(uint32 subnetId, uint128 value) external;
    /// Network::cancel_subnet_ownership_transfer (call index 178).
    function cancelSubnetOwnershipTransfer(uint32 subnetId) external;
    /// Network::owner_update_consensus_validator_stake_weight_power (call index 179).
    function ownerUpdateConsensusValidatorStakeWeightPower(uint32 subnetId, uint128 value) external;

    // Bounded contract-focused queries.
    function getSubnet(uint32 subnetId) external view returns (bool exists, SubnetInfo memory info);
    function getOwnership(uint32 subnetId)
        external
        view
        returns (OptionalAccount memory owner, OptionalAccount memory pendingOwner);
    function getRegistrationCost() external view returns (uint128);
}
