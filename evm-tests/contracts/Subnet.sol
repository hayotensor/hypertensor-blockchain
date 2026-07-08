// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

interface Subnet {
    struct Identity {
        string name;
        string url;
        string image;
        string discord;
        string x;
        string telegram;
        string github;
        string huggingFace;
        string description;
        string misc;
    }

    struct InitialValidator {
        uint256 validatorId;
        uint256 count;
    }

    struct Bootnode {
        string peerId;
        bytes multiaddr;
    }

    struct BootnodeData {
        bytes peerId;
        bytes multiaddr;
    }

    struct PeerInfo {
        string peerId;
        bytes multiaddr;
    }

    struct ConsensusData {
        uint256 subnetNodeId;
        uint256 score;
    }

    function registerValidator(
        address hotkey,
        uint256 delegateRewardRate,
        bool hasDelegateAccount,
        address delegateAccountId,
        uint256 delegateRate,
        bool hasIdentity,
        string memory name,
        string memory url,
        string memory image,
        string memory discord,
        string memory x,
        string memory telegram,
        string memory github,
        string memory huggingFace,
        string memory description,
        string memory misc
    ) external payable;

    function updateValidatorColdkey(
        uint256 validatorId,
        address newColdkey
    ) external payable;

    function updateValidatorHotkey(
        uint256 validatorId,
        address newHotkey
    ) external payable;

    function updateValidatorDelegateAccount(
        uint256 validatorId,
        bool hasDelegateAccountId,
        address delegateAccountId,
        bool hasDelegateRate,
        uint256 delegateRate
    ) external payable;

    function updateValidatorIdentity(
        uint256 validatorId,
        bool hasIdentity,
        string memory name,
        string memory url,
        string memory image,
        string memory discord,
        string memory x,
        string memory telegram,
        string memory github,
        string memory huggingFace,
        string memory description,
        string memory misc
    ) external payable;

    function registerSubnet(
        uint256 maxCost,
        string memory name,
        string memory repo,
        string memory description,
        string memory misc,
        uint256 minStake,
        uint256 maxStake,
        uint256 delegateStakePercentage,
        InitialValidator[] calldata initialValidators,
        Bootnode[] calldata bootnodes
    ) external payable;

    function getCurrentRegistrationCost(
        uint256 blockNumber
    ) external view returns (uint256);

    function activateSubnet(uint256 subnetId) external payable;

    function getSubnetId(string memory name) external view returns (uint256);

    function getMinSubnetDelegateStakeBalance(
        uint256 subnetId
    ) external view returns (uint256);

    function registerSubnetNode(
        uint256 validatorId,
        uint256 subnetId,
        address hotkey,
        PeerInfo calldata peerInfo,
        PeerInfo calldata bootnodePeerInfo,
        PeerInfo calldata clientPeerInfo,
        uint256 stakeToBeAdded,
        string memory unique,
        string memory nonUnique,
        uint256 maxBurnAmount
    ) external payable;

    function removeSubnetNode(
        uint256 subnetId,
        uint256 subnetNodeId
    ) external payable;

    function updateValidatorDelegateRewardRate(
        uint256 validatorId,
        uint256 newDelegateRewardRate
    ) external payable;

    function updateNodeUnique(
        uint256 subnetId,
        uint256 subnetNodeId,
        string memory unique
    ) external payable;

    function updateNonUnique(
        uint256 subnetId,
        uint256 subnetNodeId,
        string memory nonUnique
    ) external payable;

    function updateNodeHotkey(
        uint256 subnetId,
        uint256 subnetNodeId,
        address newHotkey
    ) external payable;

    function updateNodePeerInfo(
        uint256 subnetId,
        uint256 subnetNodeId,
        PeerInfo calldata newPeerInfo
    ) external payable;

    function updateNodeBootnodePeerInfo(
        uint256 subnetId,
        uint256 subnetNodeId,
        PeerInfo calldata newPeerInfo
    ) external payable;

    function updateNodeClientPeerInfo(
        uint256 subnetId,
        uint256 subnetNodeId,
        PeerInfo calldata newPeerInfo
    ) external payable;

    function proposeAttestation(
        uint256 subnetId,
        uint256 subnetNodeId,
        ConsensusData[] calldata data,
        bool hasPrioritizeQueueNodeId,
        uint256 prioritizeQueueNodeId,
        bool hasRemoveQueueNodeId,
        uint256 removeQueueNodeId,
        bytes calldata args,
        bytes calldata attestData
    ) external payable;

    function attest(
        uint256 subnetId,
        uint256 subnetNodeId,
        bytes calldata data
    ) external payable;

    function ownerPauseSubnet(uint256 subnetId) external;

    function ownerUnpauseSubnet(uint256 subnetId) external;

    function ownerSetEmergencyValidatorSet(
        uint256 subnetId,
        uint256[] calldata subnetNodeIds
    ) external;

    function ownerRevertEmergencyValidatorSet(uint256 subnetId) external;

    function ownerDeactivateSubnet(uint256 subnetId) external;

    function ownerUpdateName(uint256 subnetId, string memory value) external;

    function ownerUpdateRepo(uint256 subnetId, string memory value) external;

    function ownerUpdateDescription(
        uint256 subnetId,
        string memory value
    ) external;

    function ownerUpdateMisc(uint256 subnetId, string memory value) external;

    function ownerUpdateChurnLimit(uint256 subnetId, uint256 value) external;

    function ownerUpdateChurnLimitMultiplier(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateRegistrationQueueEpochs(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateIdleClassificationEpochs(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateIncludedClassificationEpochs(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerAddOrUpdateInitialValidators(
        uint256 subnetId,
        InitialValidator[] calldata validators
    ) external;

    function ownerRemoveInitialValidators(
        uint256 subnetId,
        uint256[] calldata validators
    ) external;

    function ownerUpdateMinMaxStake(
        uint256 subnetId,
        uint256 min,
        uint256 max
    ) external;

    function ownerUpdateDelegateStakePercentage(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateMaxRegisteredNodes(
        uint256 subnetId,
        uint256 value
    ) external;

    function transferSubnetOwnership(
        uint256 subnetId,
        address newOwner
    ) external;

    function acceptSubnetOwnership(uint256 subnetId) external;

    function cancelSubnetOwnershipTransfer(uint256 subnetId) external;

    function ownerAddBootnodeAccess(
        uint256 subnetId,
        address newAccount
    ) external;

    function ownerRemoveBootnodeAccess(
        uint256 subnetId,
        address removeAccount
    ) external;

    function ownerUpdateTargetNodeRegistrationsPerEpoch(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateNodeBurnRateAlpha(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateQueueImmunityEpochs(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateMinSubnetNodeReputation(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateMinConsensusNodeAttestationPercentage(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateSubnetNodeMinWeightDecreaseReputationThreshold(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateAbsentDecreaseReputationFactor(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateIncludedIncreaseReputationFactor(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateBelowMinWeightDecreaseReputationFactor(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateNonAttestorDecreaseReputationFactor(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateNonConsensusAttestorDecreaseReputationFactor(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateValidatorAbsentDecreaseReputationFactor(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateValidatorNonConsensusDecreaseReputationFactor(
        uint256 subnetId,
        uint256 value
    ) external;

    function updateBootnodes(
        uint256 subnetId,
        Bootnode[] calldata add,
        string[] calldata remove
    ) external;

    function getSubnetName(
        uint256 subnetId
    ) external view returns (string memory);

    function getSubnetIdFromFriendlyId(
        uint256 friendlyId
    ) external view returns (uint256);

    function getFriendlyIdFromSubnetId(
        uint256 subnetId
    ) external view returns (uint256);

    function getSubnetRepo(
        uint256 subnetId
    ) external view returns (string memory);

    function getSubnetDescription(
        uint256 subnetId
    ) external view returns (string memory);

    function getSubnetMisc(
        uint256 subnetId
    ) external view returns (string memory);

    function getChurnLimit(uint256 subnetId) external view returns (uint256);

    function getChurnLimitMultiplier(
        uint256 subnetId
    ) external view returns (uint256);

    function getRegistrationQueueEpochs(
        uint256 subnetId
    ) external view returns (uint256);

    function getIdleClassificationEpochs(
        uint256 subnetId
    ) external view returns (uint256);

    function getIncludedClassificationEpochs(
        uint256 subnetId
    ) external view returns (uint256);

    function getInitialValidators(
        uint256 subnetId
    ) external view returns (InitialValidator[] memory);

    function getInitialValidatorData(
        uint256 subnetId
    ) external view returns (InitialValidator[] memory);

    function getMinStake(uint256 subnetId) external view returns (uint256);

    function getMaxStake(uint256 subnetId) external view returns (uint256);

    function getDelegateStakePercentage(
        uint256 subnetId
    ) external view returns (uint256);

    function getLastDelegateStakeRewardsUpdate(
        uint256 subnetId
    ) external view returns (uint256);

    function getMaxRegisteredNodes(
        uint256 subnetId
    ) external view returns (uint256);

    function getQueueImmunityEpochs(
        uint256 subnetId
    ) external view returns (uint256);

    function getNodeRegistrationsThisEpoch(
        uint256 subnetId
    ) external view returns (uint256);

    function getNodeBurnRateAlpha(
        uint256 subnetId
    ) external view returns (uint256);

    function getCurrentNodeBurnRate(
        uint256 subnetId
    ) external view returns (uint256);

    function getRegistrationEpoch(
        uint256 subnetId
    ) external view returns (uint256);

    function getPrevPauseEpoch(
        uint256 subnetId
    ) external view returns (uint256);

    function getSlotIndex(uint256 subnetId) external view returns (uint256);

    function getSlotAssignment(
        uint256 subnetId
    ) external view returns (uint256);

    function getSubnetNodeMinWeightDecreaseReputationThreshold(
        uint256 subnetId
    ) external view returns (uint256);

    function getReputation(uint256 subnetId) external view returns (uint256);

    function getMinSubnetNodeReputation(
        uint256 subnetId
    ) external view returns (uint256);

    function getAbsentDecreaseReputationFactor(
        uint256 subnetId
    ) external view returns (uint256);

    function getIncludedIncreaseReputationFactor(
        uint256 subnetId
    ) external view returns (uint256);

    function getBelowMinWeightDecreaseReputationFactor(
        uint256 subnetId
    ) external view returns (uint256);

    function getNonAttestorDecreaseReputationFactor(
        uint256 subnetId
    ) external view returns (uint256);

    function getNonConsensusAttestorDecreaseReputationFactor(
        uint256 subnetId
    ) external view returns (uint256);

    function getValidatorAbsentDecreaseReputationFactor(
        uint256 subnetId
    ) external view returns (uint256);

    function getValidatorNonConsensusDecreaseReputationFactor(
        uint256 subnetId
    ) external view returns (uint256);

    function getBootnodeAccess(
        uint256 subnetId
    ) external view returns (address[] memory);

    function getBootnodes(
        uint256 subnetId
    ) external view returns (BootnodeData[] memory);

    function getTotalNodes(uint256 subnetId) external view returns (uint256);

    function getTotalActiveNodes(
        uint256 subnetId
    ) external view returns (uint256);

    function getTotalElectableNodes(
        uint256 subnetId
    ) external view returns (uint256);

    function getTotalSubnetStake(
        uint256 subnetId
    ) external view returns (uint256);

    function getTotalSubnetDelegateStakeShares(
        uint256 subnetId
    ) external view returns (uint256);

    function getTotalSubnetDelegateStakeBalance(
        uint256 subnetId
    ) external view returns (uint256);
}
