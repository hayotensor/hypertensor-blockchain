// SPDX-License-Identifier: Apache-2.0
pragma solidity >=0.8.24;

struct OptionalAccount {
    bool present;
    bytes32 value;
}

struct OptionalU128 {
    bool present;
    uint128 value;
}

struct OptionalU32 {
    bool present;
    uint32 value;
}

struct OptionalBytes {
    bool present;
    bytes value;
}

struct DelegateAccount {
    bytes32 accountId;
    uint128 rate;
}

struct OptionalDelegateAccount {
    bool present;
    DelegateAccount value;
}

struct Identity {
    OptionalBytes name;
    OptionalBytes url;
    OptionalBytes image;
    OptionalBytes discord;
    OptionalBytes x;
    OptionalBytes telegram;
    OptionalBytes github;
    OptionalBytes huggingFace;
    OptionalBytes description;
    OptionalBytes misc;
}

struct OptionalIdentity {
    bool present;
    Identity value;
}

struct PeerInfo {
    bytes peerId;
    OptionalBytes multiaddr;
}

struct OptionalPeerInfo {
    bool present;
    PeerInfo value;
}

struct U32Entry {
    uint32 key;
    uint32 value;
}

struct Bootnode {
    bytes peerId;
    bytes multiaddr;
}

struct SubnetRegistration {
    bytes name;
    bytes repo;
    bytes description;
    bytes misc;
    uint128 minStake;
    uint128 maxStake;
    uint128 delegateStakePercentage;
    U32Entry[] initialValidators;
    Bootnode[] bootnodes;
}
// kind: 0 = subnet delegation, 1 = validator delegation. accountId is checked by the pallet.

struct QueuedSwap {
    uint8 kind;
    bytes32 accountId;
    uint32 destinationId;
    uint128 balance;
    uint128 minSharesOut;
    uint32 executeBeforeBlock;
}

struct NodeScore {
    uint32 subnetNodeId;
    uint128 score;
}

struct OverwatchCommit {
    uint32 subnetId;
    bytes32 weight;
}

struct OverwatchReveal {
    uint32 subnetId;
    uint128 weight;
    bytes salt;
}

struct ReputationUpdates {
    OptionalU128 absentDecrease;
    OptionalU128 includedIncrease;
    OptionalU128 belowMinWeightDecrease;
    OptionalU128 nonAttestorDecrease;
    OptionalU128 nonConsensusAttestorDecrease;
    OptionalU128 validatorAbsentDecrease;
    OptionalU128 validatorNonConsensusDecrease;
}

struct NodeAllocation {
    uint32 subnetId;
    uint32 subnetNodeId;
    uint128 weight;
}

struct Pool {
    uint128 balance;
    uint128 totalShares;
    uint128 circulatingShares;
    uint128 accountShares;
}

struct Unbonding {
    uint32 unlockBlock;
    uint128 network;
    uint128 overwatch;
}

struct QueuedSwapInfo {
    QueuedSwap call;
    uint32 queuedAtBlock;
    uint32 executeAfterBlocks;
}

struct ValidatorInfo {
    uint32 id;
    bytes32 coldkey;
    bytes32 hotkey;
    uint128 delegateRewardRate;
    OptionalDelegateAccount delegateAccount;
    OptionalIdentity identity;
    uint128 poolBalance;
    uint128 poolShares;
    uint32 slashLockUntil;
}

struct SubnetInfo {
    uint32 id;
    uint8 state;
    bytes name;
    bytes repo;
    bytes description;
    bytes misc;
    uint128 minStake;
    uint128 maxStake;
    uint128 delegateStakePercentage;
    uint128 reputation;
}

struct SubnetNodeInfo {
    uint32 subnetId;
    uint32 subnetNodeId;
    uint32 validatorId;
    bytes32 coldkey;
    bytes32 hotkey;
    uint8 nodeClass;
    uint32 classStartEpoch;
    OptionalPeerInfo peerInfo;
    OptionalPeerInfo bootnodePeerInfo;
    OptionalPeerInfo clientPeerInfo;
    OptionalBytes unique;
    OptionalBytes nonUnique;
    uint128 stakeBalance;
}

struct EpochStatus {
    uint8 state;
    bool hasTiming;
    uint32 subnetEpoch;
    uint128 progression;
    uint32 startBlock;
    uint32 endBlock;
    uint32 blocksRemaining;
    bool consensusEligible;
    bool withinAttestationWindow;
    OptionalU32 electedNodeId;
    bool proposalSubmitted;
    uint8 validatorSetSource;
    bool pendingEmergencySet;
}

struct ConsensusRound {
    uint32 subnetId;
    uint32 subnetEpoch;
    uint8 status;
    uint8 electionSource;
    uint32 electedNodeId;
    uint32 electedValidatorId;
    uint128 delegateBalanceAtElection;
    bool proposalSubmitted;
}

struct OverwatchNodeInfo {
    uint32 id;
    uint32 validatorId;
    bytes32 coldkey;
    bytes32 hotkey;
    uint128 stakeBalance;
}

struct OverwatchEpoch {
    uint32 epoch;
    uint32 startBlock;
    uint32 lengthMultiplier;
    uint128 commitCutoffPercent;
}

struct EffectiveWeight {
    bool rawWeightExists;
    uint128 rawWeight;
    uint128 resolvedWeight;
}
