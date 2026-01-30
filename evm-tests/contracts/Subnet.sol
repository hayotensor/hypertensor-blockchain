// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

interface Subnet {
    struct InitialColdkey {
        address coldkey;
        uint256 count;
    }

    struct Bootnode {
        string peerId;
        bytes multiaddr;
    }

    function registerSubnet(
        uint256 maxCost,
        string memory name,
        string memory repo,
        string memory description,
        string memory misc,
        uint256 minStake,
        uint256 maxStake,
        uint256 delegateStakePercentage,
        InitialColdkey[] calldata initialColdkeys,
        Bootnode[] calldata bootnodes
    ) external payable;

    struct PeerInfo {
        string peerId;
        bytes multiaddr;
    }

    struct DelegateAccount {
        address accountId;
        uint256 rate;
    }

    function registerSubnetNode(
        uint256 subnetId,
        address hotkey,
        PeerInfo calldata peerInfo,
        PeerInfo calldata bootnodePeerInfo,
        PeerInfo calldata clientPeerInfo,
        uint256 delegateRewardRate,
        uint256 stakeToBeAdded,
        string memory unique,
        string memory nonUnique,
        DelegateAccount calldata delegateAccount,
        uint256 maxBurnAmount
    ) external payable;

    function getCurrentRegistrationCost(
        uint256
    ) external view returns (uint256);

    function activateSubnet(uint256 subnetId) external;

    function getSubnetId(string memory name) external view returns (uint256);

    function getMinSubnetDelegateStakeBalance(
        uint256 subnetId
    ) external view returns (uint256);

    function updateDelegateRewardRate(
        uint256 subnetId,
        uint256 subnetNodeId,
        uint256 newDelegateRewardRate
    ) external;

    function updateUnique(
        uint256 subnetId,
        uint256 subnetNodeId,
        string memory newUnique
    ) external;

    function updateNonUnique(
        uint256 subnetId,
        uint256 subnetNodeId,
        string memory newNonUnique
    ) external;

    function updateColdkey(address hotkey, address newColdkey) external;

    function updateHotkey(address oldHotkey, address newHotkey) external;

    function updatePeerInfo(
        uint256 subnetId,
        uint256 subnetNodeId,
        PeerInfo calldata peerInfo
    ) external;

    function updateBootnodePeerInfo(
        uint256 subnetId,
        uint256 subnetNodeId,
        PeerInfo calldata peerInfo
    ) external;

    function updateClientPeerInfo(
        uint256 subnetId,
        uint256 subnetNodeId,
        PeerInfo calldata peerInfo
    ) external;

    function registerOrUpdateIdentity(
        address hotkey,
        string memory name,
        string memory url,
        string memory image,
        string memory discord,
        string memory x,
        string memory telegram,
        string memory github,
        string memory hugging_face,
        string memory description,
        string memory misc
    ) external;

    function removeIdentity() external;

    function ownerPauseSubnet(uint256 subnetId) external;

    function ownerUnpauseSubnet(uint256 subnetId) external;

    function ownerSetEmergencyValidatorSet(
        uint256 subnetId,
        uint256[] memory subnetNodeIds
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

    function ownerAddOrUpdateInitialColdkeys(
        uint256 subnetId,
        InitialColdkey[] calldata initialColdkeys
    ) external;

    function ownerRemoveInitialColdkeys(
        uint256 subnetId,
        address[] memory coldkeys
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

    function ownerAddBootnodeAccess(
        uint256 subnetId,
        address newAccount
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

    function ownerUpdateTargetRegistrationsPerEpoch(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdateMinMaxStake(
        uint256 subnetId,
        uint256 min,
        uint256 max
    ) external;

    function ownerUpdateMinSubnetNodeReputation(
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

    function ownerUpdatBelowMinWeightDecreaseReputationFactor(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdatNonAttestorDecreaseReputationFactor(
        uint256 subnetId,
        uint256 value
    ) external;

    function ownerUpdatNonConsensusAttestorDecreaseReputationFactor(
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

    function ownerRemoveBootnodeAccess(uint256 subnetId, address) external;

    function updateBootnodes(
        uint256 subnetId,
        Bootnode[] calldata add,
        string[] memory remove
    ) external;

    function getSubnetName(
        uint256 subnetId
    ) external view returns (string memory);

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

    function getRegistrationQueueEpochs(
        uint256 subnetId
    ) external view returns (uint256);

    function getIdleClassificationEpochs(
        uint256 subnetId
    ) external view returns (uint256);

    function getIncludedClassificationEpochs(
        uint256 subnetId
    ) external view returns (uint256);

    function getMaxNodePenalties(
        uint256 subnetId
    ) external view returns (uint256);

    function getInitialColdkeys(
        uint256 subnetId
    ) external view returns (InitialColdkey[] memory);

    function getMinStake(uint256 subnetId) external view returns (uint256);

    function getMaxStake(uint256 subnetId) external view returns (uint256);

    function getDelegateStakePercentage(
        uint256 subnetId
    ) external view returns (uint256);

    function getMaxRegisteredNodes(
        uint256 subnetId
    ) external view returns (uint256);
}
