//! Incremental deletion of nonfinancial state belonging to permanently removed subnets.
//! No lifecycle, counter or balance updates belong here: logical removal has already completed.
use super::*;
use frame_support::weights::{Weight, WeightMeter};

pub const MAX_REMOVED_SUBNET_STEPS_PER_IDLE: u32 = 256;

impl<T: Config> Pallet<T> {
    pub(crate) fn removed_subnet_cleanup_step_weight(stage: RemovedSubnetCleanupStage) -> Weight {
        T::WeightInfo::removed_subnet_cleanup_step(stage as u32)
    }

    /// Delete one key or advance past one empty prefix. Selecting keys never decodes their values.
    /// A local cursor avoids revisiting backend keys deleted in this block's overlay.
    pub(crate) fn removed_subnet_cleanup_step(
        subnet_id: u32,
        stage: RemovedSubnetCleanupStage,
        last_key: &mut Option<Vec<u8>>,
    ) -> Option<RemovedSubnetCleanupStage> {
        use RemovedSubnetCleanupStage::*;
        macro_rules! singleton {
            ($storage:ident, $next:ident) => {{
                crate::$storage::<T>::remove(subnet_id);
                Some($next)
            }};
        }
        macro_rules! prefix {
            ($storage:ident, $next:expr) => {{
                let next = match last_key.take() {
                    Some(key) => crate::$storage::<T>::iter_key_prefix_from(subnet_id, key).next(),
                    None => crate::$storage::<T>::iter_key_prefix(subnet_id).next(),
                };
                if let Some(key) = next {
                    *last_key = Some(crate::$storage::<T>::hashed_key_for(subnet_id, &key));
                    crate::$storage::<T>::remove(subnet_id, key);
                    Some(stage)
                } else {
                    $next
                }
            }};
        }
        match stage {
            ChurnLimit => singleton!(ChurnLimit, ChurnLimitMultiplier),
            ChurnLimitMultiplier => singleton!(ChurnLimitMultiplier, SubnetNodeQueueEpochs),
            SubnetNodeQueueEpochs => {
                singleton!(SubnetNodeQueueEpochs, PendingSubnetNodeQueueEpochs)
            }
            PendingSubnetNodeQueueEpochs => {
                singleton!(PendingSubnetNodeQueueEpochs, ProbationRounds)
            }
            ProbationRounds => singleton!(ProbationRounds, PendingProbationRounds),
            PendingProbationRounds => singleton!(PendingProbationRounds, SubnetMinStakeBalance),
            SubnetMinStakeBalance => singleton!(SubnetMinStakeBalance, SubnetMaxStakeBalance),
            SubnetMaxStakeBalance => {
                singleton!(SubnetMaxStakeBalance, SubnetDelegateStakeRewardsPercentage)
            }
            SubnetDelegateStakeRewardsPercentage => singleton!(
                SubnetDelegateStakeRewardsPercentage,
                PendingSubnetDelegateStakeRewardsPercentage
            ),
            PendingSubnetDelegateStakeRewardsPercentage => singleton!(
                PendingSubnetDelegateStakeRewardsPercentage,
                LastSubnetDelegateStakeRewardsUpdate
            ),
            LastSubnetDelegateStakeRewardsUpdate => singleton!(
                LastSubnetDelegateStakeRewardsUpdate,
                NodeRegistrationInitialValidatorIds
            ),
            NodeRegistrationInitialValidatorIds => {
                singleton!(NodeRegistrationInitialValidatorIds, InitialValidatorData)
            }
            InitialValidatorData => singleton!(InitialValidatorData, MaxRegisteredNodes),
            MaxRegisteredNodes => singleton!(MaxRegisteredNodes, TargetNodeRegistrationsPerEpoch),
            TargetNodeRegistrationsPerEpoch => {
                singleton!(TargetNodeRegistrationsPerEpoch, NodeBurnRateAlpha)
            }
            NodeBurnRateAlpha => singleton!(NodeBurnRateAlpha, CurrentNodeBurnRate),
            CurrentNodeBurnRate => singleton!(CurrentNodeBurnRate, QueueImmunityEpochs),
            QueueImmunityEpochs => singleton!(QueueImmunityEpochs, PendingQueueImmunityEpochs),
            PendingQueueImmunityEpochs => {
                singleton!(PendingQueueImmunityEpochs, ConsensusValidatorNodeCountDecay)
            }
            ConsensusValidatorNodeCountDecay => singleton!(
                ConsensusValidatorNodeCountDecay,
                PendingConsensusValidatorNodeCountDecay
            ),
            PendingConsensusValidatorNodeCountDecay => singleton!(
                PendingConsensusValidatorNodeCountDecay,
                LastConsensusValidatorNodeCountDecayUpdate
            ),
            LastConsensusValidatorNodeCountDecayUpdate => singleton!(
                LastConsensusValidatorNodeCountDecayUpdate,
                ConsensusValidatorStakeWeightPower
            ),
            ConsensusValidatorStakeWeightPower => singleton!(
                ConsensusValidatorStakeWeightPower,
                PendingConsensusValidatorStakeWeightPower
            ),
            PendingConsensusValidatorStakeWeightPower => singleton!(
                PendingConsensusValidatorStakeWeightPower,
                LastConsensusValidatorStakeWeightPowerUpdate
            ),
            LastConsensusValidatorStakeWeightPowerUpdate => singleton!(
                LastConsensusValidatorStakeWeightPowerUpdate,
                SubnetBootnodeAccess
            ),
            SubnetBootnodeAccess => singleton!(SubnetBootnodeAccess, SubnetBootnodes),
            SubnetBootnodes => singleton!(SubnetBootnodes, SubnetReputation),
            SubnetReputation => singleton!(SubnetReputation, MinSubnetNodeReputation),
            MinSubnetNodeReputation => {
                singleton!(MinSubnetNodeReputation, PendingMinSubnetNodeReputation)
            }
            PendingMinSubnetNodeReputation => {
                singleton!(PendingMinSubnetNodeReputation, NodeRegistrationsThisEpoch)
            }
            NodeRegistrationsThisEpoch => singleton!(
                NodeRegistrationsThisEpoch,
                SubnetNodeMinWeightDecreaseReputationThreshold
            ),
            SubnetNodeMinWeightDecreaseReputationThreshold => singleton!(
                SubnetNodeMinWeightDecreaseReputationThreshold,
                PendingSubnetNodeMinWeightDecreaseReputationThreshold
            ),
            PendingSubnetNodeMinWeightDecreaseReputationThreshold => singleton!(
                PendingSubnetNodeMinWeightDecreaseReputationThreshold,
                SubnetReputationFactorSchedules
            ),
            SubnetReputationFactorSchedules => {
                singleton!(SubnetReputationFactorSchedules, SubnetNetFlows)
            }
            SubnetNetFlows => singleton!(SubnetNetFlows, SubnetDelegateStakeEmas),
            SubnetDelegateStakeEmas => singleton!(SubnetDelegateStakeEmas, SubnetRewardCapacitor),
            SubnetRewardCapacitor => singleton!(SubnetRewardCapacitor, TotalSubnetNodeUids),
            TotalSubnetNodeUids => singleton!(TotalSubnetNodeUids, SubnetNodeElectionSlots),
            SubnetNodeElectionSlots => singleton!(SubnetNodeElectionSlots, SubnetNodeQueue),
            SubnetNodeQueue => singleton!(SubnetNodeQueue, PendingAdmittedNodeRemovals),
            PendingAdmittedNodeRemovals => {
                singleton!(PendingAdmittedNodeRemovals, PendingQueuedNodeRemovals)
            }
            PendingQueuedNodeRemovals => {
                singleton!(PendingQueuedNodeRemovals, ConsensusHistoryPruneCursor)
            }
            ConsensusHistoryPruneCursor => singleton!(ConsensusHistoryPruneCursor, Nodes),
            Nodes => prefix!(SubnetNodesData, Some(QueuedNodes)),
            QueuedNodes => prefix!(QueuedSubnetNodesData, Some(NodeMetadata)),
            NodeMetadata => prefix!(SubnetNodeMetadata, Some(Peers)),
            Peers => prefix!(PeerIdSubnetNodeId, Some(BootnodePeers)),
            BootnodePeers => prefix!(BootnodePeerIdSubnetNodeId, Some(ClientPeers)),
            ClientPeers => prefix!(ClientPeerIdSubnetNodeId, Some(Multiaddrs)),
            Multiaddrs => prefix!(MultiaddrSubnetNodeId, Some(UniqueParams)),
            UniqueParams => prefix!(UniqueParamSubnetNodeId, Some(NodeHotkeys)),
            NodeHotkeys => prefix!(SubnetNodeIdHotkey, Some(NodeReputations)),
            NodeReputations => prefix!(SubnetNodeReputation, Some(NodeProbationProgress)),
            NodeProbationProgress => prefix!(SubnetNodeProbationProgress, Some(OverwatchPeers)),
            OverwatchPeers => prefix!(PeerIdOverwatchNodeId, Some(NodeElectionIndexes)),
            NodeElectionIndexes => prefix!(NodeElectionSlotIndex, Some(Attestations)),
            Attestations => {
                let next = match last_key.take() {
                    Some(key) => {
                        SubnetConsensusAttestationData::<T>::iter_key_prefix_from((subnet_id,), key)
                            .next()
                    }
                    None => {
                        SubnetConsensusAttestationData::<T>::iter_key_prefix((subnet_id,)).next()
                    }
                };
                if let Some((epoch, node)) = next {
                    *last_key = Some(SubnetConsensusAttestationData::<T>::hashed_key_for((
                        subnet_id, epoch, node,
                    )));
                    SubnetConsensusAttestationData::<T>::remove((subnet_id, epoch, node));
                    Some(stage)
                } else {
                    Some(Submissions)
                }
            }
            Submissions => prefix!(SubnetConsensusSubmission, Some(SubmissionMaxItems)),
            SubmissionMaxItems => prefix!(SubnetConsensusSubmissionMaxItems, Some(ProposalArgs)),
            ProposalArgs => prefix!(SubnetConsensusProposalArgs, Some(AttestorWeights)),
            AttestorWeights => prefix!(SubnetConsensusAttestorWeights, Some(SettlementStatus)),
            SettlementStatus => prefix!(ConsensusRoundSettlementStatus, Some(Elections)),
            Elections => prefix!(SubnetElectedValidator, None),
        }
    }

    pub(crate) fn select_removed_subnet_cleanup() -> Option<(u32, RemovedSubnetCleanupStage)> {
        let next = RemovedSubnetCleanupLastSubnet::<T>::get()
            .and_then(|last| RemovedSubnetCleanupQueue::<T>::iter_from_key(last).next())
            .or_else(|| RemovedSubnetCleanupQueue::<T>::iter().next());
        let (subnet_id, stage) = next?;
        RemovedSubnetCleanupLastSubnet::<T>::put(subnet_id);
        Some((subnet_id, stage))
    }

    /// One round-robin subnet per invocation; neither selection nor enqueue scans the backlog.
    pub(crate) fn cleanup_removed_subnets(meter: &mut WeightMeter) {
        if meter
            .try_consume(T::WeightInfo::removed_subnet_cleanup_selector())
            .is_err()
        {
            return;
        }
        let Some((subnet_id, mut stage)) = Self::select_removed_subnet_cleanup() else {
            return;
        };
        let original_stage = stage;
        let progress = T::WeightInfo::removed_subnet_cleanup_progress();
        let mut last_key = None;
        for _ in 0..MAX_REMOVED_SUBNET_STEPS_PER_IDLE {
            let step = Self::removed_subnet_cleanup_step_weight(stage);
            if !meter.can_consume(step.saturating_add(progress)) {
                break;
            }
            meter.consume(step);
            match Self::removed_subnet_cleanup_step(subnet_id, stage, &mut last_key) {
                Some(next) => stage = next,
                None => {
                    meter.consume(progress);
                    RemovedSubnetCleanupQueue::<T>::remove(subnet_id);
                    return;
                }
            }
        }
        if stage != original_stage {
            meter.consume(progress);
            RemovedSubnetCleanupQueue::<T>::insert(subnet_id, stage);
        }
    }
}
