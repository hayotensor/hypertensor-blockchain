use super::mock::*;
use crate::tests::test_utils::*;
use crate::Event;
use crate::{
    ChurnLimit, ChurnLimitMultiplier, ConsensusValidatorNodeCountDecay,
    ConsensusValidatorNodeCountDecayUpdateInterval, ConsensusValidatorStakeWeightPower,
    ConsensusValidatorStakeWeightPowerUpdateInterval, EmergencySubnetNodeElectionData,
    EmergencySubnetValidatorData, EmergencyValidatorCooldownEpochs, Error,
    FinalSubnetEmissionWeights, IdleClassificationEpochs, IncludedClassificationEpochs,
    LastConsensusValidatorNodeCountDecayUpdate, LastConsensusValidatorStakeWeightPowerUpdate,
    LastEmergencyValidatorEndEpoch, LastSubnetDelegateStakeRewardsUpdate, MaxChurnLimit,
    MaxChurnLimitMultiplier, MaxConsensusValidatorStakeWeightPower, MaxDelegateStakePercentage,
    MaxEmergencySubnetNodes, MaxIdleClassificationEpochs, MaxIncludedClassificationEpochs,
    MaxMaxRegisteredNodes, MaxQueueEpochs, MaxRegisteredNodes, MaxSubnetBootnodeAccess,
    MaxSubnetMinStake, MaxSubnetNodeMinWeightDecreaseReputationThreshold, MaxSubnetNodes,
    MaxSubnets, MinChurnLimit, MinChurnLimitMultiplier, MinConsensusValidatorStakeWeightPower,
    MinDelegateStakePercentage, MinIdleClassificationEpochs, MinIncludedClassificationEpochs,
    MinMaxRegisteredNodes, MinNodeReputationFactor, MinQueueEpochs, MinSubnetMinStake,
    MinSubnetNodeReputation, MinSubnetNodes, NetworkMaxStakeBalance, NodeBurnRateAlpha,
    NodeRegistrationInitialValidatorIds, PeerInfo, PendingConsensusValidatorNodeCountDecay,
    PendingConsensusValidatorStakeWeightPower, PendingIdleClassificationEpochs,
    PendingIncludedClassificationEpochs, PendingMinSubnetNodeReputation,
    PendingQueueImmunityEpochs, PendingSubnetDelegateStakeRewardsPercentage,
    PendingSubnetNodeMinWeightDecreaseReputationThreshold, PendingSubnetNodeQueueEpochs,
    PendingSubnetOwner, QueueImmunityEpochs, RegisteredSubnetNodesData, SubnetBootnodeAccess,
    SubnetData, SubnetDelegateStakeRewardsPercentage, SubnetDelegateStakeRewardsUpdatePeriod,
    SubnetElectedValidator, SubnetMaxStakeBalance, SubnetMinStakeBalance, SubnetName, SubnetNode,
    SubnetNodeClass, SubnetNodeClassification, SubnetNodeElectionSlots,
    SubnetNodeMinWeightDecreaseReputationThreshold, SubnetNodeQueue, SubnetNodeQueueEpochs,
    SubnetNodeReputation, SubnetNodesData, SubnetOwner, SubnetPauseCooldownEpochs, SubnetPauseData,
    SubnetRemovalReason, SubnetRepo, SubnetReputation, SubnetReputationFactorSchedules,
    SubnetReputationFactorUpdates, SubnetState, SubnetsData, TargetNodeRegistrationsPerEpoch,
    TotalElectableNodes, TotalSubnetElectableNodes,
};
use codec::Decode;
use frame_support::{assert_err, assert_ok, traits::Hooks};
use sp_core::OpaquePeerId as PeerId;
use sp_runtime::traits::TrailingZeroInput;
use sp_runtime::BoundedVec;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;

//
//
//
//
//
//
//
// Subnets Add/Remove
//
//
//
//
//
//
//
// do_owner_pause_subnet
// do_owner_unpause_subnet
// do_owner_deactivate_subnet
// do_owner_update_name
// do_owner_update_repo
// do_owner_update_description
// do_owner_update_misc
// do_owner_update_churn_limit
// do_owner_update_registration_queue_epochs
// do_owner_update_idle_classification_epochs
// do_owner_update_included_classification_epochs
// do_owner_add_or_update_initial_validators -
// do_owner_remove_initial_validators
// do_owner_update_min_max_stake
// do_owner_update_delegate_stake_percentage
// do_owner_update_max_registered_nodes
// do_transfer_subnet_ownership
// do_accept_subnet_ownership
// do_owner_add_bootnode_access
// do_owner_remove_bootnode_access
// do_owner_update_target_node_registrations_per_epoch -
// do_owner_update_node_burn_rate_alpha -
// do_owner_update_queue_immunity_epochs -
// do_owner_update_subnet_node_min_weight_decrease_reputation_threshold -

#[test]
fn test_do_owner_update_name() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_subnet_name: Vec<u8> = "new-subnet-name".into();

        assert_ok!(Network::owner_update_name(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            new_subnet_name.clone()
        ));

        let _subnet_id = SubnetName::<Test>::get(new_subnet_name.clone()).unwrap();
        assert_eq!(subnet_id, _subnet_id);

        let data = SubnetsData::<Test>::get(_subnet_id).unwrap();
        assert_eq!(new_subnet_name, data.name);
    })
}

#[test]
fn test_do_owner_update_repo() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_value: Vec<u8> = "new-val".into();

        assert_ok!(Network::owner_update_repo(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            new_value.clone()
        ));

        let _subnet_id = SubnetRepo::<Test>::get(new_value.clone()).unwrap();
        assert_eq!(subnet_id, _subnet_id);

        let data = SubnetsData::<Test>::get(_subnet_id).unwrap();
        assert_eq!(new_value, data.repo);
    })
}

#[test]
fn test_do_owner_update_description() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_value: Vec<u8> = "new-val".into();

        assert_ok!(Network::owner_update_description(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            new_value.clone()
        ));

        let data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(new_value, data.description);
    })
}

#[test]
fn test_do_owner_update_misc() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_value: Vec<u8> = "new-val".into();

        assert_ok!(Network::owner_update_misc(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            new_value.clone()
        ));

        let data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(new_value, data.misc);
    })
}

#[test]
fn test_owner_metadata_updates_reject_oversized_native_values() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let too_long_vector =
            vec![b'a'; (<Test as crate::Config>::MaxVectorLength::get() + 1) as usize];
        let too_long_url = vec![b'a'; (<Test as crate::Config>::MaxUrlLength::get() + 1) as usize];

        assert_err!(
            Network::owner_update_name(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                too_long_vector.clone()
            ),
            Error::<Test>::SubnetNameTooLong
        );
        assert_err!(
            Network::owner_update_repo(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                too_long_url
            ),
            Error::<Test>::SubnetRepoTooLong
        );
        assert_err!(
            Network::owner_update_description(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                too_long_vector.clone()
            ),
            Error::<Test>::SubnetDescriptionTooLong
        );
        assert_err!(
            Network::owner_update_misc(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                too_long_vector
            ),
            Error::<Test>::SubnetMiscTooLong
        );
    })
}

#[test]
fn test_do_owner_update_churn_limit() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_value = ChurnLimit::<Test>::get(subnet_id) + 1;

        assert_ok!(Network::owner_update_churn_limit(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            new_value
        ));

        assert_eq!(ChurnLimit::<Test>::get(subnet_id), new_value);

        assert_err!(
            Network::owner_update_churn_limit(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                MinChurnLimit::<Test>::get() - 1
            ),
            Error::<Test>::InvalidChurnLimit
        );

        assert_err!(
            Network::owner_update_churn_limit(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                MaxChurnLimit::<Test>::get() + 1
            ),
            Error::<Test>::InvalidChurnLimit
        );
    })
}

#[test]
fn test_do_owner_update_registration_queue_epochs() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let old_value = SubnetNodeQueueEpochs::<Test>::get(subnet_id);
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let new_value = old_value + 1;

        assert_ok!(Network::owner_update_registration_queue_epochs(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_value
        ));

        assert_eq!(SubnetNodeQueueEpochs::<Test>::get(subnet_id), old_value);
        assert_eq!(
            Network::get_subnet_node_queue_epochs_for_epoch(subnet_id, current_subnet_epoch),
            old_value
        );
        assert_eq!(
            Network::get_subnet_node_queue_epochs_for_epoch(subnet_id, current_subnet_epoch + 1),
            new_value
        );

        let pending = PendingSubnetNodeQueueEpochs::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_value);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);
        assert_eq!(pending.owner, original_owner.clone());

        let replacement_value = new_value + 1;
        assert_ok!(Network::owner_update_registration_queue_epochs(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            replacement_value
        ));
        let pending = PendingSubnetNodeQueueEpochs::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, replacement_value);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);

        assert_err!(
            Network::owner_update_registration_queue_epochs(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                MinQueueEpochs::<Test>::get() - 1
            ),
            Error::<Test>::InvalidRegistrationQueueEpochs
        );

        assert_err!(
            Network::owner_update_registration_queue_epochs(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                MaxQueueEpochs::<Test>::get() + 1
            ),
            Error::<Test>::InvalidRegistrationQueueEpochs
        );
    })
}

#[test]
fn test_registration_queue_update_cannot_be_replaced_during_activation_epoch() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &owner);

        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let value = SubnetNodeQueueEpochs::<Test>::get(subnet_id) + 1;
        assert_ok!(Network::owner_update_registration_queue_epochs(
            RuntimeOrigin::signed(owner.clone()),
            subnet_id,
            value
        ));

        PendingSubnetNodeQueueEpochs::<Test>::mutate(subnet_id, |pending| {
            pending.as_mut().unwrap().effective_subnet_epoch = current_subnet_epoch;
        });

        assert_err!(
            Network::owner_update_registration_queue_epochs(
                RuntimeOrigin::signed(owner),
                subnet_id,
                value + 1
            ),
            Error::<Test>::OwnerParameterUpdatePendingActivation
        );

        let pending = PendingSubnetNodeQueueEpochs::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, value);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch);
        assert_eq!(
            Network::get_subnet_node_queue_epochs_for_epoch(subnet_id, current_subnet_epoch),
            value
        );
    })
}

#[test]
fn test_do_owner_update_idle_classification_epochs() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let old_value = IdleClassificationEpochs::<Test>::get(subnet_id);
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let new_value = IdleClassificationEpochs::<Test>::get(subnet_id) + 1;

        assert_ok!(Network::owner_update_idle_classification_epochs(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            new_value
        ));

        assert_eq!(IdleClassificationEpochs::<Test>::get(subnet_id), old_value);
        assert_eq!(
            Network::get_idle_classification_epochs_for_epoch(subnet_id, current_subnet_epoch),
            old_value
        );
        assert_eq!(
            Network::get_idle_classification_epochs_for_epoch(subnet_id, current_subnet_epoch + 1),
            new_value
        );
        assert_eq!(
            PendingIdleClassificationEpochs::<Test>::get(subnet_id)
                .unwrap()
                .value,
            new_value
        );

        assert_err!(
            Network::owner_update_idle_classification_epochs(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                MinIdleClassificationEpochs::<Test>::get() - 1
            ),
            Error::<Test>::InvalidIdleClassificationEpochs
        );

        assert_err!(
            Network::owner_update_idle_classification_epochs(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                MaxIdleClassificationEpochs::<Test>::get() + 1
            ),
            Error::<Test>::InvalidIdleClassificationEpochs
        );
    })
}

#[test]
fn test_do_owner_update_included_classification_epochs() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let old_value = IncludedClassificationEpochs::<Test>::get(subnet_id);
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let new_value = IncludedClassificationEpochs::<Test>::get(subnet_id) + 1;

        assert_ok!(Network::owner_update_included_classification_epochs(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            new_value
        ));

        assert_eq!(
            IncludedClassificationEpochs::<Test>::get(subnet_id),
            old_value
        );
        assert_eq!(
            Network::get_included_classification_epochs_for_epoch(subnet_id, current_subnet_epoch),
            old_value
        );
        assert_eq!(
            Network::get_included_classification_epochs_for_epoch(
                subnet_id,
                current_subnet_epoch + 1
            ),
            new_value
        );
        assert_eq!(
            PendingIncludedClassificationEpochs::<Test>::get(subnet_id)
                .unwrap()
                .value,
            new_value
        );

        assert_err!(
            Network::owner_update_included_classification_epochs(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                MinIncludedClassificationEpochs::<Test>::get() - 1
            ),
            Error::<Test>::InvalidIncludedClassificationEpochs
        );

        assert_err!(
            Network::owner_update_included_classification_epochs(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                MaxIncludedClassificationEpochs::<Test>::get() + 1
            ),
            Error::<Test>::InvalidIncludedClassificationEpochs
        );
    })
}

#[test]
fn test_do_owner_update_target_node_registrations_per_epoch() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_value = TargetNodeRegistrationsPerEpoch::<Test>::get(subnet_id) - 1;

        assert_ok!(Network::owner_update_target_node_registrations_per_epoch(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            new_value
        ));

        assert_eq!(
            TargetNodeRegistrationsPerEpoch::<Test>::get(subnet_id),
            new_value
        );

        assert_err!(
            Network::owner_update_target_node_registrations_per_epoch(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                MaxRegisteredNodes::<Test>::get(subnet_id) + 1
            ),
            Error::<Test>::InvalidTargetNodeRegistrationsPerEpoch
        );

        assert_err!(
            Network::owner_update_target_node_registrations_per_epoch(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                0
            ),
            Error::<Test>::InvalidTargetNodeRegistrationsPerEpoch
        );
    })
}

#[test]
fn test_do_owner_update_node_burn_rate_alpha() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_value = NodeBurnRateAlpha::<Test>::get(subnet_id) - 1;

        assert_ok!(Network::owner_update_node_burn_rate_alpha(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            new_value
        ));

        assert_eq!(NodeBurnRateAlpha::<Test>::get(subnet_id), new_value);

        assert_err!(
            Network::owner_update_node_burn_rate_alpha(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                Network::percentage_factor_as_u128() + 1
            ),
            Error::<Test>::InvalidPercent
        );
    })
}

#[test]
fn test_owner_update_node_burn_rate_alpha_allows_pending_emergency_set() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        EmergencySubnetNodeElectionData::<Test>::insert(
            subnet_id,
            EmergencySubnetValidatorData {
                subnet_node_ids: vec![1, 2, 3],
                target_emergency_validators_epochs: 1,
                activated: false,
                ..Default::default()
            },
        );

        let new_value = NodeBurnRateAlpha::<Test>::get(subnet_id) - 1;
        assert_ok!(Network::owner_update_node_burn_rate_alpha(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            new_value
        ));
        assert_eq!(NodeBurnRateAlpha::<Test>::get(subnet_id), new_value);
    })
}

#[test]
fn test_do_owner_update_queue_immunity_epochs() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        assert_err!(
            Network::owner_update_queue_immunity_epochs(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                MinQueueEpochs::<Test>::get() - 1
            ),
            Error::<Test>::InvalidQueueImmunityEpochs
        );

        assert_err!(
            Network::owner_update_queue_immunity_epochs(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                MaxQueueEpochs::<Test>::get() + 1
            ),
            Error::<Test>::InvalidQueueImmunityEpochs
        );

        let old_value = QueueImmunityEpochs::<Test>::get(subnet_id);
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let new_value = MinQueueEpochs::<Test>::get();

        assert_ok!(Network::owner_update_queue_immunity_epochs(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_value
        ));

        assert_eq!(QueueImmunityEpochs::<Test>::get(subnet_id), old_value);
        assert_eq!(
            Network::get_queue_immunity_epochs_for_epoch(subnet_id, current_subnet_epoch),
            old_value
        );
        assert_eq!(
            Network::get_queue_immunity_epochs_for_epoch(subnet_id, current_subnet_epoch + 1),
            new_value
        );

        let pending = PendingQueueImmunityEpochs::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_value);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);
        assert_eq!(pending.owner, original_owner.clone());

        let replacement_value = new_value + 1;
        assert_ok!(Network::owner_update_queue_immunity_epochs(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            replacement_value
        ));
        let pending = PendingQueueImmunityEpochs::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, replacement_value);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);
    })
}

#[test]
fn test_owner_pending_parameter_update_cannot_be_replaced_during_activation_epoch() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &owner);

        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let value = MinQueueEpochs::<Test>::get();

        assert_ok!(Network::owner_update_queue_immunity_epochs(
            RuntimeOrigin::signed(owner.clone()),
            subnet_id,
            value
        ));

        PendingQueueImmunityEpochs::<Test>::mutate(subnet_id, |pending| {
            pending.as_mut().unwrap().effective_subnet_epoch = current_subnet_epoch;
        });

        assert_err!(
            Network::owner_update_queue_immunity_epochs(
                RuntimeOrigin::signed(owner),
                subnet_id,
                value + 1
            ),
            Error::<Test>::OwnerParameterUpdatePendingActivation
        );

        let pending = PendingQueueImmunityEpochs::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, value);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch);
        assert_eq!(
            Network::get_queue_immunity_epochs_for_epoch(subnet_id, current_subnet_epoch),
            value
        );
    })
}

#[test]
fn do_owner_update_subnet_node_min_weight_decrease_reputation_threshold() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_value = 1;

        let old_value = SubnetNodeMinWeightDecreaseReputationThreshold::<Test>::get(subnet_id);
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        assert_ok!(
            Network::owner_update_subnet_node_min_weight_decrease_reputation_threshold(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                new_value
            )
        );

        assert_eq!(
            SubnetNodeMinWeightDecreaseReputationThreshold::<Test>::get(subnet_id),
            old_value
        );
        assert_eq!(
            Network::get_subnet_node_min_weight_decrease_reputation_threshold_for_epoch(
                subnet_id,
                current_subnet_epoch
            ),
            old_value
        );
        assert_eq!(
            Network::get_subnet_node_min_weight_decrease_reputation_threshold_for_epoch(
                subnet_id,
                current_subnet_epoch + 1
            ),
            new_value
        );
        let pending =
            PendingSubnetNodeMinWeightDecreaseReputationThreshold::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_value);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);
        assert_eq!(pending.owner, original_owner.clone());

        let replacement_value = new_value + 1;
        assert_ok!(
            Network::owner_update_subnet_node_min_weight_decrease_reputation_threshold(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                replacement_value
            )
        );
        let pending =
            PendingSubnetNodeMinWeightDecreaseReputationThreshold::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, replacement_value);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);

        assert_err!(
            Network::owner_update_subnet_node_min_weight_decrease_reputation_threshold(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                MaxSubnetNodeMinWeightDecreaseReputationThreshold::<Test>::get() + 1
            ),
            Error::<Test>::InvalidPercent
        );
    })
}

#[test]
fn test_owner_pause_subnet() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        run_to_first_pause_eligible_subnet_slot(subnet_id);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let epoch = Network::get_current_epoch_as_u32();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // Transfer to new owner
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetPaused {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
            }
        );

        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.state, SubnetState::Paused);
        assert_eq!(
            subnet_data.pause,
            Some(SubnetPauseData {
                started_global_epoch: epoch,
                started_subnet_epoch: subnet_epoch,
            })
        );
        assert_eq!(subnet_data.consensus_eligible_from_subnet_epoch, None);
    });
}

#[test]
fn test_owner_pause_subnet_must_be_active_error() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_registered_subnet(
            subnet_name.clone(),
            0,
            4,
            deposit_amount,
            stake_amount,
            true,
            None,
        );

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        // Transfer to new owner
        assert_err!(
            Network::owner_pause_subnet(RuntimeOrigin::signed(original_owner.clone()), subnet_id),
            Error::<Test>::SubnetMustBeActive
        );
    });
}

#[test]
fn test_owner_unpause_subnet() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let validator_id = 1;

        run_to_first_pause_eligible_subnet_slot(subnet_id);

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let pause_epoch = Network::get_current_epoch_as_u32();
        let pause_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        let coldkey = account(1000);
        let hotkey = account(1001);
        let start_epoch = pause_subnet_epoch + 100;

        let hotkey_subnet_node_id = 1000;
        let queued_node = SubnetNode::<Test> {
            id: hotkey_subnet_node_id,
            validator_id: validator_id,
            peer_info: Some(PeerInfo::<Test> {
                peer_id: peer(0),
                multiaddr: None,
            }),
            bootnode_peer_info: None,
            client_peer_info: None,
            classification: SubnetNodeClassification {
                node_class: SubnetNodeClass::Validator,
                start_epoch: start_epoch,
            },
            unique: Some(BoundedVec::new()),
            non_unique: Some(BoundedVec::new()),
        };
        RegisteredSubnetNodesData::<Test>::insert(subnet_id, hotkey_subnet_node_id, &queued_node);
        SubnetNodeQueue::<Test>::insert(subnet_id, vec![queued_node]);

        // Transfer to new owner
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.state, SubnetState::Paused);
        assert_eq!(
            subnet_data.pause,
            Some(SubnetPauseData {
                started_global_epoch: pause_epoch,
                started_subnet_epoch: pause_subnet_epoch,
            })
        );

        increase_epochs(10);

        let curr_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let delta = curr_subnet_epoch - pause_subnet_epoch;

        assert_ok!(Network::owner_unpause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetUnpaused {
                subnet_id: subnet_id,
                owner: original_owner.clone()
            }
        );

        // Ensure was activated
        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.state, SubnetState::Active);
        assert_eq!(
            subnet_data.consensus_eligible_from_subnet_epoch,
            Some(curr_subnet_epoch + 2)
        );
        assert_eq!(subnet_data.pause, None);

        let node = RegisteredSubnetNodesData::<Test>::get(subnet_id, hotkey_subnet_node_id);
        assert_eq!(node.classification.start_epoch, start_epoch + delta);
        assert_eq!(SubnetNodeQueue::<Test>::get(subnet_id), vec![node]);
    });
}

#[test]
fn test_owner_unpause_reserves_full_epoch_before_consensus() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "full-preparation-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &owner);

        run_to_first_pause_eligible_subnet_slot(subnet_id);
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(owner.clone()),
            subnet_id,
        ));

        let unpause_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        assert_ok!(Network::owner_unpause_subnet(
            RuntimeOrigin::signed(owner),
            subnet_id,
        ));

        let preparation_epoch = unpause_subnet_epoch.saturating_add(1);
        let first_consensus_epoch = unpause_subnet_epoch.saturating_add(2);
        assert_eq!(
            SubnetsData::<Test>::get(subnet_id)
                .unwrap()
                .consensus_eligible_from_subnet_epoch,
            Some(first_consensus_epoch)
        );
        let subnet_reputation_before_historical_settlement =
            SubnetReputation::<Test>::get(subnet_id);

        // The complete following local epoch is preparation-only for new work. An exact election
        // made before the pause remains allocation-eligible and settles here, but the preparation
        // slot must not elect a replacement validator.
        set_epoch(preparation_epoch, 2);
        Network::on_initialize(System::block_number());
        assert!(FinalSubnetEmissionWeights::<Test>::get(preparation_epoch)
            .subnet_weights
            .contains_key(&subnet_id));

        set_block_to_subnet_slot_epoch(preparation_epoch, subnet_id);
        Network::on_initialize(System::block_number());
        assert!(!SubnetElectedValidator::<Test>::contains_key(
            subnet_id,
            preparation_epoch
        ));
        assert!(
            SubnetReputation::<Test>::get(subnet_id)
                < subnet_reputation_before_historical_settlement
        );
        let subnet_reputation_after_historical_settlement =
            SubnetReputation::<Test>::get(subnet_id);
        let node_reputations_after_historical_settlement: BTreeMap<u32, u128> =
            SubnetNodeReputation::<Test>::iter_prefix(subnet_id).collect();

        // The first live epoch still has no prior work to fund, but its subnet slot must
        // elect a validator and begin a complete consensus round.
        set_epoch(first_consensus_epoch, 2);
        Network::on_initialize(System::block_number());
        assert!(!FinalSubnetEmissionWeights::<Test>::contains_key(
            first_consensus_epoch
        ));

        set_block_to_subnet_slot_epoch(first_consensus_epoch, subnet_id);
        Network::on_initialize(System::block_number());
        assert!(SubnetElectedValidator::<Test>::contains_key(
            subnet_id,
            first_consensus_epoch
        ));
        assert_eq!(
            SubnetReputation::<Test>::get(subnet_id),
            subnet_reputation_after_historical_settlement
        );
        assert_eq!(
            SubnetNodeReputation::<Test>::iter_prefix(subnet_id).collect::<BTreeMap<_, _>>(),
            node_reputations_after_historical_settlement
        );

        // The following distribution now sees exact prior work and includes the subnet.
        let first_reward_epoch = first_consensus_epoch.saturating_add(1);
        set_epoch(first_reward_epoch, 2);
        Network::on_initialize(System::block_number());
        assert!(FinalSubnetEmissionWeights::<Test>::get(first_reward_epoch)
            .subnet_weights
            .contains_key(&subnet_id));
    });
}

#[test]
fn test_owner_unpause_rejects_invalid_pending_emergency_set() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        run_to_first_pause_eligible_subnet_slot(subnet_id);
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        EmergencySubnetNodeElectionData::<Test>::insert(
            subnet_id,
            EmergencySubnetValidatorData {
                subnet_node_ids: vec![1, 2, 999],
                target_emergency_validators_epochs: 1,
                ..Default::default()
            },
        );

        // A failed unpause must not partially apply lifecycle, emergency, or queue
        // compensation writes.
        let queued_node_id = 1_000;
        let queued_node = SubnetNode::<Test> {
            id: queued_node_id,
            validator_id: 1,
            peer_info: Some(PeerInfo::<Test> {
                peer_id: peer(0),
                multiaddr: None,
            }),
            bootnode_peer_info: None,
            client_peer_info: None,
            classification: SubnetNodeClassification {
                node_class: SubnetNodeClass::Registered,
                start_epoch: Network::get_current_subnet_epoch_as_u32(subnet_id),
            },
            unique: Some(BoundedVec::new()),
            non_unique: Some(BoundedVec::new()),
        };
        RegisteredSubnetNodesData::<Test>::insert(subnet_id, queued_node_id, &queued_node);
        SubnetNodeQueue::<Test>::insert(subnet_id, vec![queued_node]);

        let lifecycle_before = SubnetsData::<Test>::get(subnet_id);
        let emergency_before = EmergencySubnetNodeElectionData::<Test>::get(subnet_id);
        let canonical_queue_node_before =
            RegisteredSubnetNodesData::<Test>::get(subnet_id, queued_node_id);
        let queue_before = SubnetNodeQueue::<Test>::get(subnet_id);

        assert_err!(
            Network::owner_unpause_subnet(RuntimeOrigin::signed(original_owner), subnet_id),
            Error::<Test>::InvalidEmergencySubnetNodeId
        );
        assert_eq!(SubnetsData::<Test>::get(subnet_id), lifecycle_before);
        assert_eq!(
            EmergencySubnetNodeElectionData::<Test>::get(subnet_id),
            emergency_before
        );
        assert_eq!(
            RegisteredSubnetNodesData::<Test>::get(subnet_id, queued_node_id),
            canonical_queue_node_before
        );
        assert_eq!(SubnetNodeQueue::<Test>::get(subnet_id), queue_before);
    });
}

#[test]
fn test_owner_unpause_finishes_expired_active_emergency_set() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        EmergencySubnetNodeElectionData::<Test>::insert(
            subnet_id,
            EmergencySubnetValidatorData {
                subnet_node_ids: vec![1, 2, 3],
                target_emergency_validators_epochs: 1,
                total_epochs: 1,
                activated: true,
                started_subnet_epoch: Network::get_current_subnet_epoch_as_u32(subnet_id),
                max_emergency_validators_epoch: u32::MAX,
                ..Default::default()
            },
        );

        run_to_first_pause_eligible_subnet_slot(subnet_id);
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));
        assert_ok!(Network::owner_unpause_subnet(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
        ));

        assert!(EmergencySubnetNodeElectionData::<Test>::get(subnet_id).is_none());
        assert_eq!(
            LastEmergencyValidatorEndEpoch::<Test>::get(subnet_id),
            Network::get_current_epoch_as_u32()
        );
    });
}

#[test]
fn test_owner_unpause_subnet_repause_cooldown_error() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        SubnetPauseCooldownEpochs::<Test>::put(10);

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let validator_id = 1;

        run_to_first_pause_eligible_subnet_slot(subnet_id);

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let pause_epoch = Network::get_current_epoch_as_u32();
        let pause_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        let coldkey = account(1000);
        let hotkey = account(1001);
        let start_epoch = pause_subnet_epoch + 100;

        let hotkey_subnet_node_id = 1000;
        RegisteredSubnetNodesData::<Test>::insert(
            subnet_id,
            hotkey_subnet_node_id,
            SubnetNode::<Test> {
                id: hotkey_subnet_node_id,
                validator_id: validator_id,
                peer_info: Some(PeerInfo::<Test> {
                    peer_id: peer(0),
                    multiaddr: None,
                }),
                bootnode_peer_info: None,
                client_peer_info: None,
                classification: SubnetNodeClassification {
                    node_class: SubnetNodeClass::Validator,
                    start_epoch: start_epoch,
                },
                unique: Some(BoundedVec::new()),
                non_unique: Some(BoundedVec::new()),
            },
        );

        // Transfer to new owner
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.state, SubnetState::Paused);
        assert_eq!(
            subnet_data.pause,
            Some(SubnetPauseData {
                started_global_epoch: pause_epoch,
                started_subnet_epoch: pause_subnet_epoch,
            })
        );

        increase_epochs(10);

        let curr_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let delta = curr_subnet_epoch - pause_subnet_epoch;

        assert_ok!(Network::owner_unpause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetUnpaused {
                subnet_id: subnet_id,
                owner: original_owner.clone()
            }
        );

        // Ensure was activated
        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.state, SubnetState::Active);
        let consensus_eligible_from_subnet_epoch = curr_subnet_epoch.saturating_add(2);
        assert_eq!(
            subnet_data.consensus_eligible_from_subnet_epoch,
            Some(consensus_eligible_from_subnet_epoch)
        );
        assert_eq!(subnet_data.pause, None);

        let node = RegisteredSubnetNodesData::<Test>::get(subnet_id, hotkey_subnet_node_id);
        assert_eq!(node.classification.start_epoch, start_epoch + delta);

        assert_err!(
            Network::owner_pause_subnet(RuntimeOrigin::signed(original_owner.clone()), subnet_id,),
            Error::<Test>::SubnetPauseCooldownActive
        );

        let pause_cooldown_epochs = SubnetPauseCooldownEpochs::<Test>::get();
        let repause_epoch =
            consensus_eligible_from_subnet_epoch.saturating_add(pause_cooldown_epochs);
        set_block_to_subnet_slot_epoch(repause_epoch, subnet_id);
        System::set_block_number(System::block_number().saturating_sub(1));
        assert_err!(
            Network::owner_pause_subnet(RuntimeOrigin::signed(original_owner.clone()), subnet_id,),
            Error::<Test>::SubnetPauseCooldownActive
        );

        set_block_to_subnet_slot_epoch(repause_epoch, subnet_id);
        Network::on_initialize(System::block_number());

        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));
    });
}

#[test]
fn test_owner_unpause_default_cooldown_requires_first_live_round_to_settle() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "one-round-cooldown-subnet".into();
        let deposit_amount = 10_000_000_000_000_000_000_000u128;
        let stake_amount = MinSubnetMinStake::<Test>::get();

        assert_eq!(SubnetPauseCooldownEpochs::<Test>::get(), 1);
        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &owner);
        run_to_first_pause_eligible_subnet_slot(subnet_id);

        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(owner.clone()),
            subnet_id,
        ));
        assert_ok!(Network::owner_unpause_subnet(
            RuntimeOrigin::signed(owner.clone()),
            subnet_id,
        ));
        let consensus_eligible_from_subnet_epoch = SubnetsData::<Test>::get(subnet_id)
            .unwrap()
            .consensus_eligible_from_subnet_epoch
            .unwrap();

        // The first live slot elects a validator, but the round is not complete until the next
        // subnet slot settles it. Cooldown one therefore remains active throughout this epoch.
        set_block_to_subnet_slot_epoch(consensus_eligible_from_subnet_epoch, subnet_id);
        Network::on_initialize(System::block_number());
        assert!(SubnetElectedValidator::<Test>::contains_key(
            subnet_id,
            consensus_eligible_from_subnet_epoch
        ));
        assert_err!(
            Network::owner_pause_subnet(RuntimeOrigin::signed(owner.clone()), subnet_id),
            Error::<Test>::SubnetPauseCooldownActive
        );

        let settlement_epoch = consensus_eligible_from_subnet_epoch.saturating_add(1);

        // Allocation runs at global slot two before the subnet's assigned settlement slot.
        // Without this hook the later slot would merely advance the local epoch and would not
        // prove that the missing round was evaluated before the owner could pause again.
        set_epoch(settlement_epoch, 2);
        Network::on_initialize(System::block_number());
        assert!(FinalSubnetEmissionWeights::<Test>::get(settlement_epoch)
            .subnet_weights
            .contains_key(&subnet_id));
        let reputation_before_settlement = SubnetReputation::<Test>::get(subnet_id);

        set_block_to_subnet_slot_epoch(settlement_epoch, subnet_id);
        System::set_block_number(System::block_number().saturating_sub(1));
        assert_err!(
            Network::owner_pause_subnet(RuntimeOrigin::signed(owner.clone()), subnet_id),
            Error::<Test>::SubnetPauseCooldownActive
        );

        set_block_to_subnet_slot_epoch(settlement_epoch, subnet_id);
        Network::on_initialize(System::block_number());
        assert!(
            SubnetReputation::<Test>::get(subnet_id) < reputation_before_settlement,
            "the missing first live round must be penalized before the pause extrinsic"
        );
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(owner),
            subnet_id,
        ));
    });
}

#[test]
fn test_owner_pause_subnet_cooldown_uses_saturating_add() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        SubnetPauseCooldownEpochs::<Test>::put(10);

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        increase_epochs(20);
        SubnetsData::<Test>::mutate(subnet_id, |maybe_subnet| {
            maybe_subnet
                .as_mut()
                .unwrap()
                .consensus_eligible_from_subnet_epoch = Some(u32::MAX - 5);
        });

        assert_err!(
            Network::owner_pause_subnet(RuntimeOrigin::signed(original_owner), subnet_id),
            Error::<Test>::SubnetPauseCooldownActive
        );
        assert_eq!(
            SubnetsData::<Test>::get(subnet_id).unwrap().state,
            SubnetState::Active
        );
    });
}

#[test]
fn test_owner_unpause_saturates_resume_and_queue_epochs() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            Network::get_unpause_consensus_eligible_from_subnet_epoch(u32::MAX - 1),
            u32::MAX
        );
        assert_eq!(
            Network::get_unpause_consensus_eligible_from_subnet_epoch(u32::MAX),
            u32::MAX
        );

        let subnet_name: Vec<u8> = "unpause-saturation-subnet".into();
        let deposit_amount = 10_000_000_000_000_000_000_000u128;
        let stake_amount = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &owner);
        run_to_first_pause_eligible_subnet_slot(subnet_id);
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(owner.clone()),
            subnet_id,
        ));

        let pause_started_subnet_epoch = SubnetsData::<Test>::get(subnet_id)
            .unwrap()
            .pause
            .unwrap()
            .started_subnet_epoch;
        let queued_node_id = 1_000;
        let queued_node = SubnetNode::<Test> {
            id: queued_node_id,
            validator_id: 1,
            peer_info: Some(PeerInfo::<Test> {
                peer_id: peer(1_000),
                multiaddr: None,
            }),
            bootnode_peer_info: None,
            client_peer_info: None,
            classification: SubnetNodeClassification {
                node_class: SubnetNodeClass::Registered,
                start_epoch: u32::MAX - 1,
            },
            unique: Some(BoundedVec::new()),
            non_unique: Some(BoundedVec::new()),
        };
        RegisteredSubnetNodesData::<Test>::insert(subnet_id, queued_node_id, queued_node.clone());
        SubnetNodeQueue::<Test>::insert(subnet_id, vec![queued_node]);

        increase_epochs(2);
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        assert!(
            current_subnet_epoch.saturating_sub(pause_started_subnet_epoch) >= 2,
            "fixture must produce a queue-compensation delta that saturates the start epoch"
        );

        assert_ok!(Network::owner_unpause_subnet(
            RuntimeOrigin::signed(owner),
            subnet_id,
        ));

        let subnet = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(subnet.state, SubnetState::Active);
        assert_eq!(
            subnet.consensus_eligible_from_subnet_epoch,
            Some(Network::get_unpause_consensus_eligible_from_subnet_epoch(
                current_subnet_epoch
            ))
        );
        assert_eq!(subnet.pause, None);
        assert_eq!(
            RegisteredSubnetNodesData::<Test>::get(subnet_id, queued_node_id)
                .classification
                .start_epoch,
            u32::MAX
        );
        assert_eq!(
            SubnetNodeQueue::<Test>::get(subnet_id)[0]
                .classification
                .start_epoch,
            u32::MAX
        );
    });
}

#[test]
fn test_owner_unpause_subnet_must_be_paused_error() {
    new_test_ext().execute_with(|| {
    let subnet_name: Vec<u8> = "subnet-name".into();
    let deposit_amount: u128 = 10000000000000000000000;
    let amount: u128 = 1000000000000000000000;
    let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

    build_registered_subnet(
      subnet_name.clone(),
      0,
      4,
      deposit_amount,
      stake_amount,
      true,
      None,
    );

    let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

    let original_owner = account(1);

    // Set initial owner
    SubnetOwner::<Test>::insert(subnet_id, &original_owner);

    // Transfer to new owner
    assert_err!(
      Network::owner_unpause_subnet(
        RuntimeOrigin::signed(original_owner.clone()),
        subnet_id,
      ),
      Error::<Test>::SubnetMustBePaused
    );
  });
}

fn assert_registered_queue_copies_match(subnet_id: u32) -> BTreeMap<u32, u32> {
    let registered: BTreeMap<u32, SubnetNode<Test>> =
        RegisteredSubnetNodesData::<Test>::iter_prefix(subnet_id).collect();
    let queue = SubnetNodeQueue::<Test>::get(subnet_id);

    assert_eq!(registered.len(), queue.len());
    for queued_node in &queue {
        assert_eq!(registered.get(&queued_node.id), Some(queued_node));
    }

    registered
        .into_iter()
        .map(|(node_id, node)| (node_id, node.classification.start_epoch))
        .collect()
}

fn assert_owner_unpause_queue_compensation(pause_before_slot: bool) {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &owner);

        increase_epochs(SubnetPauseCooldownEpochs::<Test>::get().saturating_add(1));

        let churn_limit = ChurnLimit::<Test>::get(subnet_id);
        build_registered_nodes_in_queue(
            subnet_id,
            4,
            4 + churn_limit,
            deposit_amount,
            stake_amount,
        );

        let pause_general_epoch = Network::get_current_epoch_as_u32().saturating_add(1);
        set_block_to_subnet_slot_epoch(pause_general_epoch, subnet_id);
        if pause_before_slot {
            System::set_block_number(System::block_number().saturating_sub(1));
        } else {
            System::set_block_number(System::block_number().saturating_add(1));
        }

        let paused_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let unpause_general_epoch = pause_general_epoch.saturating_add(2);
        let expected_unpause_subnet_epoch = if pause_before_slot {
            unpause_general_epoch
        } else {
            unpause_general_epoch.saturating_sub(1)
        };
        let consensus_eligible_from_epoch = expected_unpause_subnet_epoch.saturating_add(2);
        let queue_epochs = SubnetNodeQueueEpochs::<Test>::get(subnet_id);

        // Crossing the current slot while paused adds it to the missed count. Thus a
        // before-slot pause followed by an after-slot unpause misses G, G+1, and G+2;
        // the inverse positions miss only G+1 because the G and G+2 slots remain live.
        let expected_missed_slots = if pause_before_slot { 3 } else { 1 };
        let expected_shifted_start = consensus_eligible_from_epoch
            .checked_sub(queue_epochs.saturating_add(1))
            .expect("test epoch must leave room for queue readiness");
        let original_start = expected_shifted_start
            .checked_sub(expected_missed_slots)
            .expect("test epoch must leave room for pause compensation");

        let queued_node_ids: Vec<u32> = RegisteredSubnetNodesData::<Test>::iter_prefix(subnet_id)
            .map(|(node_id, _)| node_id)
            .collect();
        for node_id in queued_node_ids {
            RegisteredSubnetNodesData::<Test>::mutate(subnet_id, node_id, |node| {
                node.classification.start_epoch = original_start;
            });
        }
        SubnetNodeQueue::<Test>::mutate(subnet_id, |queue| {
            for node in queue {
                node.classification.start_epoch = original_start;
            }
        });
        let original_starts = assert_registered_queue_copies_match(subnet_id);

        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(owner.clone()),
            subnet_id,
        ));
        let paused_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(paused_data.state, SubnetState::Paused);
        assert_eq!(
            paused_data.pause,
            Some(SubnetPauseData {
                started_global_epoch: pause_general_epoch,
                started_subnet_epoch: paused_subnet_epoch,
            })
        );
        let recorded_pause_subnet_epoch = paused_data.pause.unwrap().started_subnet_epoch;

        set_block_to_subnet_slot_epoch(unpause_general_epoch, subnet_id);
        if pause_before_slot {
            System::set_block_number(System::block_number().saturating_add(1));
        } else {
            System::set_block_number(System::block_number().saturating_sub(1));
        }

        assert_eq!(Network::get_current_epoch_as_u32(), unpause_general_epoch);
        let unpause_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        assert_eq!(
            unpause_subnet_epoch.saturating_sub(recorded_pause_subnet_epoch),
            expected_missed_slots
        );

        assert_ok!(Network::owner_unpause_subnet(
            RuntimeOrigin::signed(owner),
            subnet_id,
        ));

        let unpaused_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(unpaused_data.state, SubnetState::Active);
        assert_eq!(
            unpaused_data.consensus_eligible_from_subnet_epoch,
            Some(consensus_eligible_from_epoch)
        );
        assert_eq!(unpaused_data.pause, None);

        let shifted_starts = assert_registered_queue_copies_match(subnet_id);
        assert_eq!(shifted_starts.len(), original_starts.len());
        for (node_id, original_node_start) in original_starts {
            assert_eq!(
                shifted_starts.get(&node_id),
                Some(&original_node_start.saturating_add(expected_missed_slots))
            );
            assert_eq!(shifted_starts.get(&node_id), Some(&expected_shifted_start));
        }

        // The local epoch immediately before consensus eligibility is preparation-only and counts
        // normally toward queue maturity.
        let preparation_epoch = consensus_eligible_from_epoch.saturating_sub(1);
        set_block_to_subnet_slot_epoch(preparation_epoch, subnet_id);
        let preparation_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        assert_eq!(preparation_subnet_epoch, preparation_epoch);
        for node in SubnetNodeQueue::<Test>::get(subnet_id) {
            assert_eq!(
                node.classification.start_epoch.saturating_add(queue_epochs),
                preparation_subnet_epoch
            );
        }

        set_block_to_subnet_slot_epoch(consensus_eligible_from_epoch, subnet_id);
        let first_consensus_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        assert_eq!(first_consensus_subnet_epoch, consensus_eligible_from_epoch);
        for node in SubnetNodeQueue::<Test>::get(subnet_id) {
            assert!(
                node.classification.start_epoch.saturating_add(queue_epochs)
                    < first_consensus_subnet_epoch
            );
        }
    });
}

#[test]
fn test_owner_unpause_compensates_queue_when_paused_before_slot() {
    assert_owner_unpause_queue_compensation(true);
}

#[test]
fn test_owner_unpause_compensates_queue_when_paused_after_slot() {
    assert_owner_unpause_queue_compensation(false);
}

fn assert_emergency_validator_starts_at_phase_aware_resume(unpause_before_slot: bool) {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = if unpause_before_slot {
            "emergency-resume-before-slot".into()
        } else {
            "emergency-resume-after-slot".into()
        };
        let deposit_amount = 10_000_000_000_000_000_000_000u128;
        let stake_amount = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &owner);
        run_to_first_pause_eligible_subnet_slot(subnet_id);
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(owner.clone()),
            subnet_id,
        ));

        let emergency_node_ids: Vec<u32> = SubnetNodeElectionSlots::<Test>::get(subnet_id)
            .into_iter()
            .take(MinSubnetNodes::<Test>::get() as usize)
            .collect();
        assert_ok!(Network::owner_set_emergency_validator_set(
            RuntimeOrigin::signed(owner.clone()),
            subnet_id,
            emergency_node_ids,
        ));

        let unpause_global_epoch = Network::get_current_epoch_as_u32().saturating_add(1);
        set_block_to_subnet_slot_epoch(unpause_global_epoch, subnet_id);
        if unpause_before_slot {
            System::set_block_number(System::block_number().saturating_sub(1));
        } else {
            System::set_block_number(System::block_number().saturating_add(1));
        }

        let current_global_epoch = Network::get_current_epoch_as_u32();
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        assert_eq!(current_global_epoch, unpause_global_epoch);
        if unpause_before_slot {
            assert_eq!(current_subnet_epoch.saturating_add(1), current_global_epoch);
        } else {
            assert_eq!(current_subnet_epoch, current_global_epoch);
        }
        let expected_consensus_eligible_from_epoch =
            Network::get_unpause_consensus_eligible_from_subnet_epoch(current_subnet_epoch);

        assert_ok!(Network::owner_unpause_subnet(
            RuntimeOrigin::signed(owner),
            subnet_id,
        ));

        let subnet = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(
            subnet.consensus_eligible_from_subnet_epoch,
            Some(expected_consensus_eligible_from_epoch)
        );
        let emergency = EmergencySubnetNodeElectionData::<Test>::get(subnet_id).unwrap();
        assert!(emergency.activated);
        assert_eq!(
            emergency.started_subnet_epoch,
            expected_consensus_eligible_from_epoch
        );
        assert!(emergency.max_emergency_validators_epoch >= expected_consensus_eligible_from_epoch);
    });
}

#[test]
fn test_emergency_validator_starts_at_local_resume_when_unpaused_before_slot() {
    assert_emergency_validator_starts_at_phase_aware_resume(true);
}

#[test]
fn test_emergency_validator_starts_at_local_resume_when_unpaused_after_slot() {
    assert_emergency_validator_starts_at_phase_aware_resume(false);
}

#[test]
fn test_emergency_validator_duration_formula() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            Network::get_emergency_validator_duration_epochs(
                test_percent(1, 10),
                test_percent(1, 10)
            )
            .unwrap(),
            23
        );
        assert_eq!(
            Network::get_emergency_validator_duration_epochs(
                test_percent(1, 20),
                test_percent(1, 10)
            )
            .unwrap(),
            46
        );
        assert_eq!(
            Network::get_emergency_validator_duration_epochs(
                test_percent(1, 2),
                test_percent(1, 10)
            )
            .unwrap(),
            5
        );
        assert_eq!(
            Network::get_emergency_validator_duration_epochs(
                test_percent(1, 10),
                test_percent(1, 20)
            )
            .unwrap(),
            30
        );
        assert_eq!(
            Network::get_emergency_validator_duration_epochs(
                test_percent(1, 10),
                test_percent(1, 2)
            )
            .unwrap(),
            8
        );
        assert_eq!(
            Network::get_emergency_validator_duration_epochs(
                Network::percentage_factor_as_u128(),
                test_percent(1, 10)
            )
            .unwrap(),
            2
        );
        assert_eq!(
            Network::get_emergency_validator_duration_epochs(
                test_percent(1, 10),
                Network::percentage_factor_as_u128()
            )
            .unwrap(),
            2
        );
    });
}

#[test]
fn test_emergency_validator_duration_formula_rejects_invalid_values() {
    new_test_ext().execute_with(|| {
        assert!(matches!(
            Network::get_emergency_validator_duration_epochs(0, test_percent(1, 10)),
            Err(Error::<Test>::InvalidEmergencyValidatorDuration)
        ));
        assert!(matches!(
            Network::get_emergency_validator_duration_epochs(test_percent(1, 10), 0),
            Err(Error::<Test>::InvalidEmergencyValidatorDuration)
        ));
        assert!(matches!(
            Network::get_emergency_validator_duration_epochs(
                test_percent(1, 100_000),
                test_percent(1, 10)
            ),
            Err(Error::<Test>::InvalidEmergencyValidatorDuration)
        ));
    });
}

#[test]
fn test_owner_set_emergency_validator_subnet() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max = 12;

        build_activated_subnet(subnet_name.clone(), 0, max, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        run_to_first_pause_eligible_subnet_slot(subnet_id);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let epoch = Network::get_current_epoch_as_u32();

        // Transfer to new owner
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetPaused {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
            }
        );

        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.state, SubnetState::Paused);
        assert_eq!(subnet_data.pause.unwrap().started_global_epoch, epoch);

        let mut original_subnet_node_ids: Vec<u32> = Vec::new();
        for (id, _) in SubnetNodesData::<Test>::iter_prefix(subnet_id) {
            original_subnet_node_ids.push(id);
        }

        let mut subnet_node_ids: Vec<u32> = Vec::new();
        for (id, _) in SubnetNodesData::<Test>::iter_prefix(subnet_id).take((max - 1) as usize) {
            subnet_node_ids.push(id);
        }

        let pre_emergency_validator_data = EmergencySubnetNodeElectionData::<Test>::get(subnet_id);
        assert!(pre_emergency_validator_data.is_none());

        assert_ok!(Network::owner_set_emergency_validator_set(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            subnet_node_ids.clone()
        ));

        let emergency_validator_data =
            EmergencySubnetNodeElectionData::<Test>::get(subnet_id).unwrap();
        let expected_target_emergency_epochs = Network::get_emergency_validator_duration_epochs(
            emergency_validator_data.reputation_factors.absent_decrease,
            emergency_validator_data.min_subnet_node_reputation,
        )
        .unwrap();
        assert_eq!(emergency_validator_data.subnet_node_ids, subnet_node_ids);
        assert_ne!(
            emergency_validator_data.subnet_node_ids,
            original_subnet_node_ids
        );
        assert_eq!(
            emergency_validator_data.target_emergency_validators_epochs,
            expected_target_emergency_epochs
        );
        assert_eq!(emergency_validator_data.max_emergency_validators_epoch, 0);
        assert_eq!(emergency_validator_data.total_epochs, 0);

        let unpause_epoch = Network::get_current_epoch_as_u32();
        assert_ok!(Network::owner_unpause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        let emergency_validator_data =
            EmergencySubnetNodeElectionData::<Test>::get(subnet_id).unwrap();
        assert_eq!(emergency_validator_data.subnet_node_ids, subnet_node_ids);
        assert_ne!(
            emergency_validator_data.subnet_node_ids,
            original_subnet_node_ids
        );
        assert_eq!(
            emergency_validator_data.target_emergency_validators_epochs,
            expected_target_emergency_epochs
        );
        assert_ne!(emergency_validator_data.max_emergency_validators_epoch, 0);
        assert_eq!(emergency_validator_data.total_epochs, 0);
        assert_eq!(
            emergency_validator_data.started_subnet_epoch,
            unpause_epoch.saturating_add(2)
        );
        assert!(
            emergency_validator_data.max_emergency_validators_epoch
                >= emergency_validator_data.started_subnet_epoch
        );

        // G+1 is preparation-only. Position the test there so the loop's first
        // election/reward simulation lands on the G+2 emergency start epoch.
        increase_epochs(1);

        // EmergencySubnetNodeElectionData removes after being greater than total epochs
        // so use += 2 here
        for _ in 0..emergency_validator_data
            .target_emergency_validators_epochs
            .saturating_add(2)
        {
            increase_epochs(1);
            let epoch = Network::get_current_epoch_as_u32();
            set_block_to_subnet_slot_epoch(epoch, subnet_id);
            let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

            Network::elect_validator(subnet_id, subnet_epoch, System::block_number());

            // simulate calling distribute_rewards
            let forked_subnet_node_ids: Option<BTreeSet<u32>> =
                EmergencySubnetNodeElectionData::<Test>::mutate_exists(subnet_id, |maybe_data| {
                    if let Some(data) = maybe_data {
                        // Increment `total_epochs`
                        data.total_epochs = data.total_epochs.saturating_add(1);

                        Some(data.subnet_node_ids.iter().cloned().collect())
                    } else {
                        None
                    }
                });
        }

        assert_eq!(
            EmergencySubnetNodeElectionData::<Test>::try_get(subnet_id),
            Err(())
        );
    });
}

#[test]
fn test_owner_set_emergency_validator_duration_ignores_current_node_reputations() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max = 12;

        build_activated_subnet(subnet_name.clone(), 0, max, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let original_owner = account(1);

        run_to_first_pause_eligible_subnet_slot(subnet_id);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        let selected_len = MinSubnetNodes::<Test>::get() as usize;
        let election_slots = SubnetNodeElectionSlots::<Test>::get(subnet_id);
        let subnet_node_ids: Vec<u32> = election_slots.iter().take(selected_len).copied().collect();

        for subnet_node_id in election_slots.iter().skip(selected_len) {
            SubnetNodeReputation::<Test>::insert(subnet_id, subnet_node_id, 0);
        }

        assert_ok!(Network::owner_set_emergency_validator_set(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            subnet_node_ids.clone()
        ));

        let emergency_validator_data =
            EmergencySubnetNodeElectionData::<Test>::get(subnet_id).unwrap();
        let expected_target_emergency_epochs = Network::get_emergency_validator_duration_epochs(
            emergency_validator_data.reputation_factors.absent_decrease,
            emergency_validator_data.min_subnet_node_reputation,
        )
        .unwrap();

        assert_eq!(emergency_validator_data.subnet_node_ids, subnet_node_ids);
        assert_eq!(
            emergency_validator_data.target_emergency_validators_epochs,
            expected_target_emergency_epochs
        );
        assert_eq!(expected_target_emergency_epochs, 23);
    });
}

#[test]
fn test_owner_fork_subnet_max_fork_epoch() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max = 12;

        build_activated_subnet(subnet_name.clone(), 0, max, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        run_to_first_pause_eligible_subnet_slot(subnet_id);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let epoch = Network::get_current_epoch_as_u32();

        // Transfer to new owner
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetPaused {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
            }
        );

        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.state, SubnetState::Paused);
        assert_eq!(subnet_data.pause.unwrap().started_global_epoch, epoch);

        let mut original_subnet_node_ids: Vec<u32> = Vec::new();
        for (id, _) in SubnetNodesData::<Test>::iter_prefix(subnet_id) {
            original_subnet_node_ids.push(id);
        }

        let mut subnet_node_ids: Vec<u32> = Vec::new();
        for (id, _) in SubnetNodesData::<Test>::iter_prefix(subnet_id).take((max - 1) as usize) {
            subnet_node_ids.push(id);
        }

        let pre_emergency_validator_data = EmergencySubnetNodeElectionData::<Test>::get(subnet_id);
        assert!(pre_emergency_validator_data.is_none());

        assert_ok!(Network::owner_set_emergency_validator_set(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            subnet_node_ids.clone()
        ));

        let emergency_validator_data = EmergencySubnetNodeElectionData::<Test>::get(subnet_id);
        assert!(emergency_validator_data.is_some());
        assert_eq!(
            emergency_validator_data.clone().unwrap().subnet_node_ids,
            subnet_node_ids
        );
        assert_ne!(
            emergency_validator_data.clone().unwrap().subnet_node_ids,
            original_subnet_node_ids
        );
        assert_ne!(
            emergency_validator_data
                .clone()
                .unwrap()
                .target_emergency_validators_epochs,
            0
        );
        assert_eq!(
            emergency_validator_data
                .clone()
                .unwrap()
                .max_emergency_validators_epoch,
            0
        );
        assert_eq!(emergency_validator_data.clone().unwrap().total_epochs, 0);

        let unpause_epoch = Network::get_current_epoch_as_u32();
        assert_ok!(Network::owner_unpause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        let emergency_validator_data = EmergencySubnetNodeElectionData::<Test>::get(subnet_id);
        assert_eq!(
            emergency_validator_data.clone().unwrap().subnet_node_ids,
            subnet_node_ids
        );
        assert_ne!(
            emergency_validator_data.clone().unwrap().subnet_node_ids,
            original_subnet_node_ids
        );
        assert_ne!(
            emergency_validator_data
                .clone()
                .unwrap()
                .target_emergency_validators_epochs,
            0
        );
        assert_ne!(
            emergency_validator_data
                .clone()
                .unwrap()
                .max_emergency_validators_epoch,
            0
        );
        assert_eq!(emergency_validator_data.clone().unwrap().total_epochs, 0);
        assert_eq!(
            emergency_validator_data
                .clone()
                .unwrap()
                .started_subnet_epoch,
            unpause_epoch.saturating_add(2)
        );
        assert!(
            emergency_validator_data
                .clone()
                .unwrap()
                .max_emergency_validators_epoch
                >= emergency_validator_data
                    .clone()
                    .unwrap()
                    .started_subnet_epoch
        );

        let max_epochs = emergency_validator_data
            .clone()
            .unwrap()
            .max_emergency_validators_epoch
            .saturating_sub(Network::get_current_subnet_epoch_as_u32(subnet_id));
        log::error!("max_epochs {:?}", max_epochs);

        // EmergencySubnetNodeElectionData removes after being greater than `max_epochs`
        for _ in 0..max_epochs.saturating_add(1) {
            increase_epochs(1);
            let epoch = Network::get_current_epoch_as_u32();
            set_block_to_subnet_slot_epoch(epoch, subnet_id);
            let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
            Network::elect_validator(subnet_id, subnet_epoch, System::block_number());
        }

        assert_eq!(
            EmergencySubnetNodeElectionData::<Test>::try_get(subnet_id),
            Err(())
        );
    });
}

#[test]
fn test_owner_set_emergency_validator_set_strictly_validates_unique_ids() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max = 4;

        build_activated_subnet(subnet_name.clone(), 0, max, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        run_to_first_pause_eligible_subnet_slot(subnet_id);
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        MaxEmergencySubnetNodes::<Test>::put(3);
        assert_err!(
            Network::owner_set_emergency_validator_set(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                vec![1, 2, 3, 4],
            ),
            Error::<Test>::InvalidMaxEmergencySubnetNodes
        );

        MaxEmergencySubnetNodes::<Test>::put(10);
        assert_err!(
            Network::owner_set_emergency_validator_set(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                vec![1, 999, 1, 998, 1],
            ),
            Error::<Test>::InvalidEmergencySubnetNodeId
        );

        assert_err!(
            Network::owner_set_emergency_validator_set(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                vec![1, 2, 1],
            ),
            Error::<Test>::InvalidMinEmergencySubnetNodes
        );

        assert_ok!(Network::owner_set_emergency_validator_set(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            vec![3, 2, 1],
        ));

        let emergency_data = EmergencySubnetNodeElectionData::<Test>::get(subnet_id).unwrap();
        assert_eq!(emergency_data.subnet_node_ids, vec![1, 2, 3]);
        assert!(!emergency_data.activated);
    });
}

#[test]
fn test_active_emergency_validator_set_cannot_be_reset_by_pause_cycle() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max = 4;

        build_activated_subnet(subnet_name.clone(), 0, max, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        run_to_first_pause_eligible_subnet_slot(subnet_id);
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));
        assert_ok!(Network::owner_set_emergency_validator_set(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            vec![1, 2, 3],
        ));
        assert_ok!(Network::owner_unpause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        let activated_data = EmergencySubnetNodeElectionData::<Test>::get(subnet_id).unwrap();
        assert!(activated_data.activated);
        let max_emergency_epoch = activated_data.max_emergency_validators_epoch;
        let started_subnet_epoch = activated_data.started_subnet_epoch;

        run_to_first_pause_eligible_subnet_slot(subnet_id);
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));
        assert_err!(
            Network::owner_set_emergency_validator_set(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                vec![1, 2, 4],
            ),
            Error::<Test>::EmergencyValidatorsActive
        );

        let data_after_failed_reset =
            EmergencySubnetNodeElectionData::<Test>::get(subnet_id).unwrap();
        assert_eq!(
            data_after_failed_reset.max_emergency_validators_epoch,
            max_emergency_epoch
        );
        assert_eq!(
            data_after_failed_reset.started_subnet_epoch,
            started_subnet_epoch
        );
        assert_eq!(data_after_failed_reset.total_epochs, 0);
    });
}

#[test]
fn test_emergency_validator_cooldown_blocks_immediate_reactivation() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max = 4;

        EmergencyValidatorCooldownEpochs::<Test>::put(5);
        build_activated_subnet(subnet_name.clone(), 0, max, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        run_to_first_pause_eligible_subnet_slot(subnet_id);
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));
        assert_ok!(Network::owner_set_emergency_validator_set(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            vec![1, 2, 3],
        ));
        assert_ok!(Network::owner_unpause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        Network::finish_emergency_validator_set(subnet_id);
        let ended_epoch = Network::get_current_epoch_as_u32();
        assert_eq!(
            LastEmergencyValidatorEndEpoch::<Test>::get(subnet_id),
            ended_epoch
        );

        run_to_first_pause_eligible_subnet_slot(subnet_id);
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));
        assert_err!(
            Network::owner_set_emergency_validator_set(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                vec![1, 2, 3],
            ),
            Error::<Test>::EmergencyValidatorCooldownActive
        );

        increase_epochs(EmergencyValidatorCooldownEpochs::<Test>::get());
        assert_ok!(Network::owner_set_emergency_validator_set(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            vec![1, 2, 3],
        ));
    });
}

#[test]
fn test_emergency_validator_set_freezes_owner_reputation_removal_knobs() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let max = 4;

        build_activated_subnet(subnet_name.clone(), 0, max, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        run_to_first_pause_eligible_subnet_slot(subnet_id);
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));
        assert_ok!(Network::owner_set_emergency_validator_set(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            vec![1, 2, 3],
        ));
        assert_ok!(Network::owner_unpause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        assert_err!(
            Network::owner_update_min_subnet_node_reputation(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                MinSubnetNodeReputation::<Test>::get(subnet_id),
            ),
            Error::<Test>::EmergencyValidatorsSet
        );

        assert_err!(
            Network::owner_update_subnet_node_min_weight_decrease_reputation_threshold(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                SubnetNodeMinWeightDecreaseReputationThreshold::<Test>::get(subnet_id),
            ),
            Error::<Test>::EmergencyValidatorsSet
        );
    });
}

#[test]
fn test_owner_deactivate_subnet() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let removed_electable_nodes = SubnetNodeElectionSlots::<Test>::get(subnet_id).len() as u32;
        assert!(removed_electable_nodes > 0);
        assert_eq!(
            TotalSubnetElectableNodes::<Test>::get(subnet_id),
            removed_electable_nodes
        );
        let previous_total_electable_nodes = TotalElectableNodes::<Test>::get();

        assert_ok!(Network::owner_deactivate_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetDeactivated {
                subnet_id: subnet_id,
                reason: SubnetRemovalReason::Owner
            }
        );

        assert_eq!(SubnetsData::<Test>::try_get(subnet_id), Err(()));
        assert_eq!(TotalSubnetElectableNodes::<Test>::get(subnet_id), 0);
        assert_eq!(
            TotalElectableNodes::<Test>::get(),
            previous_total_electable_nodes.saturating_sub(removed_electable_nodes)
        );
    });
}

#[test]
fn test_owner_update_name() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        let prev_name = subnet_data.name;

        let original_owner = account(1);
        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        // Subnet 2
        let subnet_name_2: Vec<u8> = "subnet-name-2".into();
        build_activated_subnet(subnet_name_2.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id_2 = SubnetName::<Test>::get(subnet_name_2.clone()).unwrap();
        let owner_2 = account(2);
        SubnetOwner::<Test>::insert(subnet_id_2, &owner_2);

        let new_subnet_name: Vec<u8> = "new-subnet-name".into();
        assert_ok!(Network::owner_update_name(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_subnet_name.clone()
        ));

        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.name, new_subnet_name.clone());

        assert_eq!(
            SubnetName::<Test>::get(&new_subnet_name.clone()).unwrap(),
            subnet_id
        );

        assert_eq!(SubnetName::<Test>::try_get(&subnet_name.clone()), Err(()));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetNameUpdate {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                prev_value: prev_name,
                value: new_subnet_name.clone()
            }
        );

        // Update to a new name and check old one was removed
        let new_subnet_name_2: Vec<u8> = "new-subnet-name-2".into();
        assert_ok!(Network::owner_update_name(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_subnet_name_2.clone()
        ));
        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.name, new_subnet_name_2.clone());
        assert_eq!(
            SubnetName::<Test>::try_get(&new_subnet_name.clone()),
            Err(())
        );
        assert_eq!(
            SubnetName::<Test>::get(&new_subnet_name_2.clone()).unwrap(),
            subnet_id
        );

        // Update subnet 2 to the original name
        assert_ok!(Network::owner_update_name(
            RuntimeOrigin::signed(owner_2.clone()),
            subnet_id_2,
            new_subnet_name.clone()
        ));
        let subnet_data = SubnetsData::<Test>::get(subnet_id_2).unwrap();
        assert_eq!(subnet_data.name, new_subnet_name.clone());
        assert_eq!(
            SubnetName::<Test>::get(&new_subnet_name.clone()).unwrap(),
            subnet_id_2
        );
    });
}

#[test]
fn test_owner_update_name_allows_same_value_noop_and_rejects_other_subnet_name() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        let prev_name = subnet_data.name;

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let other_subnet_name: Vec<u8> = "other-subnet-name".into();
        build_activated_subnet(
            other_subnet_name.clone(),
            0,
            4,
            deposit_amount,
            stake_amount,
        );

        let event_count = network_events().len();

        assert_ok!(Network::owner_update_name(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            prev_name.clone()
        ));

        assert_eq!(network_events().len(), event_count);
        assert_eq!(
            SubnetsData::<Test>::get(subnet_id).unwrap().name,
            prev_name.clone()
        );
        assert_eq!(SubnetName::<Test>::get(&prev_name), Some(subnet_id));

        assert_err!(
            Network::owner_update_name(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                other_subnet_name.clone()
            ),
            Error::<Test>::SubnetNameExist
        );
    });
}

#[test]
fn test_owner_update_repo() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        let prev_repo = subnet_data.repo;

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let subnet_name_2: Vec<u8> = "subnet-name-2".into();
        build_activated_subnet(subnet_name_2.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id_2 = SubnetName::<Test>::get(subnet_name_2.clone()).unwrap();
        let owner_2 = account(2);
        SubnetOwner::<Test>::insert(subnet_id_2, &owner_2);

        let new_subnet_repo: Vec<u8> = "new-subnet-repo".into();
        assert_ok!(Network::owner_update_repo(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_subnet_repo.clone()
        ));

        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.repo, new_subnet_repo.clone());

        assert_eq!(
            SubnetRepo::<Test>::get(&new_subnet_repo.clone()).unwrap(),
            subnet_id
        );

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetRepoUpdate {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                prev_value: prev_repo,
                value: new_subnet_repo.clone()
            }
        );

        // Update to a new repo and check old one was removed
        let new_subnet_repo_2: Vec<u8> = "new-subnet-repo_2".into();
        assert_ok!(Network::owner_update_repo(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_subnet_repo_2.clone()
        ));
        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.repo, new_subnet_repo_2.clone());
        assert_eq!(
            SubnetRepo::<Test>::try_get(&new_subnet_repo.clone()),
            Err(())
        );
        assert_eq!(
            SubnetRepo::<Test>::get(&new_subnet_repo_2.clone()).unwrap(),
            subnet_id
        );

        // Update subnet 2 to the original repo
        assert_ok!(Network::owner_update_repo(
            RuntimeOrigin::signed(owner_2.clone()),
            subnet_id_2,
            new_subnet_repo.clone()
        ));
        let subnet_data = SubnetsData::<Test>::get(subnet_id_2).unwrap();
        assert_eq!(subnet_data.repo, new_subnet_repo.clone());
        assert_eq!(
            SubnetRepo::<Test>::get(&new_subnet_repo.clone()).unwrap(),
            subnet_id_2
        );
    });
}

#[test]
fn test_owner_update_repo_allows_same_value_noop_and_rejects_other_subnet_repo() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        let prev_repo = subnet_data.repo;

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let other_subnet_name: Vec<u8> = "other-subnet-name".into();
        build_activated_subnet(
            other_subnet_name.clone(),
            0,
            4,
            deposit_amount,
            stake_amount,
        );
        let other_subnet_id = SubnetName::<Test>::get(other_subnet_name).unwrap();
        let other_repo = SubnetsData::<Test>::get(other_subnet_id).unwrap().repo;

        let event_count = network_events().len();

        assert_ok!(Network::owner_update_repo(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            prev_repo.clone()
        ));

        assert_eq!(network_events().len(), event_count);
        assert_eq!(
            SubnetsData::<Test>::get(subnet_id).unwrap().repo,
            prev_repo.clone()
        );
        assert_eq!(SubnetRepo::<Test>::get(&prev_repo), Some(subnet_id));

        assert_err!(
            Network::owner_update_repo(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                other_repo
            ),
            Error::<Test>::SubnetRepoExist
        );
    });
}

#[test]
fn test_owner_update_description() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        let prev_description = subnet_data.description;

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let epoch = Network::get_current_epoch_as_u32();

        let new_subnet_description: Vec<u8> = "new-subnet-description".into();
        assert_ok!(Network::owner_update_description(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_subnet_description.clone()
        ));

        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.description, new_subnet_description.clone());

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetDescriptionUpdate {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                prev_value: prev_description,
                value: new_subnet_description.clone()
            }
        );
    });
}

#[test]
fn test_owner_update_misc() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        let prev_misc = subnet_data.misc;

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let epoch = Network::get_current_epoch_as_u32();

        let new_subnet_misc: Vec<u8> = "new-subnet-misc".into();
        assert_ok!(Network::owner_update_misc(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_subnet_misc.clone()
        ));

        let subnet_data = SubnetsData::<Test>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.misc, new_subnet_misc.clone());

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetMiscUpdate {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                prev_value: prev_misc,
                value: new_subnet_misc.clone()
            }
        );
    });
}

#[test]
fn test_owner_update_churn_limit() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let epoch = Network::get_current_epoch_as_u32();

        let current_churn_limit = ChurnLimit::<Test>::get(subnet_id);

        let new_churn_limit = current_churn_limit + 1;
        assert_ok!(Network::owner_update_churn_limit(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_churn_limit
        ));

        let churn_limit = ChurnLimit::<Test>::get(subnet_id);
        assert_eq!(churn_limit, new_churn_limit);

        assert_eq!(
            *network_events().last().unwrap(),
            Event::ChurnLimitUpdate {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                value: new_churn_limit
            }
        );
    });
}

#[test]
fn test_owner_update_churn_limit_multiplier() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-churn-multiplier".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let min_multiplier = MinChurnLimitMultiplier::<Test>::get();
        let max_multiplier = MaxChurnLimitMultiplier::<Test>::get();
        let new_value = if min_multiplier < max_multiplier {
            min_multiplier.saturating_add(1)
        } else {
            min_multiplier
        };
        assert_ok!(Network::owner_update_churn_limit_multiplier(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_value
        ));
        assert_eq!(ChurnLimitMultiplier::<Test>::get(subnet_id), new_value);
        assert_eq!(
            *network_events().last().unwrap(),
            Event::ChurnLimitMultiplierUpdate {
                subnet_id,
                owner: original_owner.clone(),
                value: new_value
            }
        );

        assert_err!(
            Network::owner_update_churn_limit_multiplier(
                RuntimeOrigin::signed(account(999)),
                subnet_id,
                new_value
            ),
            Error::<Test>::NotSubnetOwner
        );
        assert_err!(
            Network::owner_update_churn_limit_multiplier(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                min_multiplier.saturating_sub(1)
            ),
            Error::<Test>::InvalidChurnLimitMultiplier
        );
        assert_err!(
            Network::owner_update_churn_limit_multiplier(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                max_multiplier.saturating_add(1)
            ),
            Error::<Test>::InvalidChurnLimitMultiplier
        );
    });
}

#[test]
fn test_owner_update_registration_queue_epochs() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        let reg_queue_epochs = SubnetNodeQueueEpochs::<Test>::get(subnet_id);

        let new_reg_queue_epochs = reg_queue_epochs + 1;
        assert_ok!(Network::owner_update_registration_queue_epochs(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_reg_queue_epochs
        ));

        assert_eq!(
            SubnetNodeQueueEpochs::<Test>::get(subnet_id),
            reg_queue_epochs
        );
        let pending = PendingSubnetNodeQueueEpochs::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_reg_queue_epochs);
        assert_eq!(
            pending.effective_subnet_epoch,
            current_subnet_epoch.saturating_add(1)
        );

        assert_eq!(
            *network_events().last().unwrap(),
            Event::RegistrationQueueEpochsUpdateScheduled {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                value: new_reg_queue_epochs,
                effective_subnet_epoch: current_subnet_epoch.saturating_add(1),
            }
        );
    });
}

#[test]
fn test_owner_update_registration_queue_epochs_invalid_registration_queue_epochs_error() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let epochs = MinQueueEpochs::<Test>::get() - 1;

        assert_err!(
            Network::owner_update_registration_queue_epochs(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                epochs
            ),
            Error::<Test>::InvalidRegistrationQueueEpochs
        );

        let epochs = MaxQueueEpochs::<Test>::get() + 1;

        assert_err!(
            Network::owner_update_registration_queue_epochs(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                epochs
            ),
            Error::<Test>::InvalidRegistrationQueueEpochs
        );
    });
}

#[test]
fn test_owner_update_idle_classification_epochs() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        let idle_epochs = IdleClassificationEpochs::<Test>::get(subnet_id);

        let new_idle_epochs = idle_epochs + 1;
        assert_ok!(Network::owner_update_idle_classification_epochs(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_idle_epochs
        ));

        assert_eq!(
            IdleClassificationEpochs::<Test>::get(subnet_id),
            idle_epochs
        );
        assert_eq!(
            Network::get_idle_classification_epochs_for_epoch(subnet_id, current_subnet_epoch),
            idle_epochs
        );
        assert_eq!(
            Network::get_idle_classification_epochs_for_epoch(subnet_id, current_subnet_epoch + 1),
            new_idle_epochs
        );

        let pending = PendingIdleClassificationEpochs::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_idle_epochs);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);
        assert_eq!(pending.owner, original_owner.clone());

        assert_eq!(
            *network_events().last().unwrap(),
            Event::IdleClassificationEpochsUpdateScheduled {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                value: new_idle_epochs,
                effective_subnet_epoch: current_subnet_epoch + 1
            }
        );

        let replacement_idle_epochs = MaxIdleClassificationEpochs::<Test>::get();
        assert_ok!(Network::owner_update_idle_classification_epochs(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            replacement_idle_epochs
        ));
        let pending = PendingIdleClassificationEpochs::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, replacement_idle_epochs);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);
    });
}

#[test]
fn test_owner_update_idle_classification_epochs_invalid_idle_classification_epochs() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let epochs = MinIdleClassificationEpochs::<Test>::get() - 1;

        assert_err!(
            Network::owner_update_idle_classification_epochs(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                epochs
            ),
            Error::<Test>::InvalidIdleClassificationEpochs
        );

        let epochs = MaxIdleClassificationEpochs::<Test>::get() + 1;

        assert_err!(
            Network::owner_update_idle_classification_epochs(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                epochs
            ),
            Error::<Test>::InvalidIdleClassificationEpochs
        );
    });
}

#[test]
fn test_owner_update_included_classification_epochs() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        let included_epochs = IncludedClassificationEpochs::<Test>::get(subnet_id);

        let new_included_epochs = included_epochs + 1;
        assert_ok!(Network::owner_update_included_classification_epochs(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_included_epochs
        ));

        assert_eq!(
            IncludedClassificationEpochs::<Test>::get(subnet_id),
            included_epochs
        );
        assert_eq!(
            Network::get_included_classification_epochs_for_epoch(subnet_id, current_subnet_epoch),
            included_epochs
        );
        assert_eq!(
            Network::get_included_classification_epochs_for_epoch(
                subnet_id,
                current_subnet_epoch + 1
            ),
            new_included_epochs
        );

        let pending = PendingIncludedClassificationEpochs::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_included_epochs);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);
        assert_eq!(pending.owner, original_owner.clone());

        assert_eq!(
            *network_events().last().unwrap(),
            Event::IncludedClassificationEpochsUpdateScheduled {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                value: new_included_epochs,
                effective_subnet_epoch: current_subnet_epoch + 1
            }
        );

        let replacement_included_epochs = MaxIncludedClassificationEpochs::<Test>::get();
        assert_ok!(Network::owner_update_included_classification_epochs(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            replacement_included_epochs
        ));
        let pending = PendingIncludedClassificationEpochs::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, replacement_included_epochs);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);
    });
}

#[test]
fn test_owner_update_included_classification_epochs_invalid_included_classification_epochs() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let epochs = MinIncludedClassificationEpochs::<Test>::get() - 1;

        assert_err!(
            Network::owner_update_included_classification_epochs(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                epochs
            ),
            Error::<Test>::InvalidIncludedClassificationEpochs
        );

        let epochs = MaxIncludedClassificationEpochs::<Test>::get() + 1;

        assert_err!(
            Network::owner_update_included_classification_epochs(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                epochs
            ),
            Error::<Test>::InvalidIncludedClassificationEpochs
        );
    });
}

#[test]
fn test_owner_add_or_update_initial_validators() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnet_id = 1;
        let subnet_data = SubnetData {
            id: subnet_id,
            friendly_id: subnet_id,
            name: subnet_name.clone(),
            repo: subnet_name.clone(),
            description: subnet_name.clone(),
            misc: subnet_name.clone(),
            state: SubnetState::Registered,
            consensus_eligible_from_subnet_epoch: None,
            pause: None,
        };

        // Store subnet data
        SubnetsData::<Test>::insert(subnet_id, &subnet_data);

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_validators = BTreeMap::from([(1, 1)]);
        assert_ok!(Network::owner_add_or_update_initial_validators(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_validators.clone()
        ));

        let validators = NodeRegistrationInitialValidatorIds::<Test>::get(subnet_id).unwrap();
        assert_eq!(validators.clone(), new_validators.clone());

        assert_eq!(
            *network_events().last().unwrap(),
            Event::AddSubnetRegistrationInitialValidators {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                validators: validators.clone()
            }
        );

        let new_validators = BTreeMap::from([(1, 2)]);
        assert_ok!(Network::owner_add_or_update_initial_validators(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_validators.clone()
        ));

        let validators = NodeRegistrationInitialValidatorIds::<Test>::get(subnet_id).unwrap();
        assert_eq!(validators.clone(), new_validators.clone());
    });
}

#[test]
fn test_owner_add_initial_validators_must_be_registering() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_validators = BTreeMap::from([(1, 1)]);
        assert_err!(
            Network::owner_add_or_update_initial_validators(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                new_validators.clone()
            ),
            Error::<Test>::SubnetMustBeRegistering
        );
    });
}

#[test]
fn test_owner_add_initial_validators_invalid_registration_slots() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let subnet_id = 1;
        let subnet_data = SubnetData {
            id: subnet_id,
            friendly_id: subnet_id,
            name: subnet_name.clone(),
            repo: subnet_name.clone(),
            description: subnet_name.clone(),
            misc: subnet_name.clone(),
            state: SubnetState::Registered,
            consensus_eligible_from_subnet_epoch: None,
            pause: None,
        };

        SubnetsData::<Test>::insert(subnet_id, &subnet_data);

        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_validators = BTreeMap::from([(1, 0)]);
        assert_err!(
            Network::owner_add_or_update_initial_validators(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                new_validators
            ),
            Error::<Test>::InvalidSubnetRegistrationInitialColdkeys
        );

        assert!(NodeRegistrationInitialValidatorIds::<Test>::get(subnet_id).is_none());
    });
}

#[test]
fn test_owner_remove_initial_validators() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);

        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnet_id = 1;
        let subnet_data = SubnetData {
            id: subnet_id,
            friendly_id: subnet_id,
            name: subnet_name.clone(),
            repo: subnet_name.clone(),
            description: subnet_name.clone(),
            misc: subnet_name.clone(),
            state: SubnetState::Registered,
            consensus_eligible_from_subnet_epoch: None,
            pause: None,
        };

        // Store subnet data
        SubnetsData::<Test>::insert(subnet_id, &subnet_data);

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_validators = BTreeMap::from([(1, 1), (2, 1)]);
        assert_ok!(Network::owner_add_or_update_initial_validators(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_validators.clone()
        ));

        let validators = NodeRegistrationInitialValidatorIds::<Test>::get(subnet_id).unwrap();
        assert_eq!(validators, new_validators.clone());

        let remove_validators = BTreeSet::from([1]);
        assert_ok!(Network::owner_remove_initial_validators(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            remove_validators.clone()
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::RemoveSubnetRegistrationInitialValidators {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                validators: remove_validators.clone()
            }
        );

        let expected_validators = BTreeMap::from([(2, 1)]);
        let validators = NodeRegistrationInitialValidatorIds::<Test>::get(subnet_id).unwrap();
        assert_eq!(validators, expected_validators.clone());
    });
}

#[test]
fn test_owner_remove_initial_validators_cleans_empty_storage() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let subnet_id = 1;
        let subnet_data = SubnetData {
            id: subnet_id,
            friendly_id: subnet_id,
            name: subnet_name.clone(),
            repo: subnet_name.clone(),
            description: subnet_name.clone(),
            misc: subnet_name.clone(),
            state: SubnetState::Registered,
            consensus_eligible_from_subnet_epoch: None,
            pause: None,
        };

        SubnetsData::<Test>::insert(subnet_id, &subnet_data);

        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_validators = BTreeMap::from([(1, 1)]);
        assert_ok!(Network::owner_add_or_update_initial_validators(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_validators
        ));

        let remove_validators = BTreeSet::from([1]);
        assert_ok!(Network::owner_remove_initial_validators(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            remove_validators
        ));

        assert!(NodeRegistrationInitialValidatorIds::<Test>::get(subnet_id).is_none());
    });
}

#[test]
fn test_owner_remove_initial_validators_must_be_registering() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_validators = BTreeSet::from([1]);
        assert_err!(
            Network::owner_remove_initial_validators(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                new_validators
            ),
            Error::<Test>::SubnetMustBeRegistering
        );
    });
}

#[test]
fn test_owner_remove_subnet_node() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
    });
}

// #[test]
// fn test_owner_update_subnet_node_consecutive_included_epochs() {
//     new_test_ext().execute_with(|| {
//         let subnet_name: Vec<u8> = "subnet-name".into();
//         let deposit_amount: u128 = 10000000000000000000000;
//         let amount: u128 = 1000000000000000000000;
//         let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

//         build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
//         let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

//         let original_owner = account(1);

//         // Set initial owner
//         SubnetOwner::<Test>::insert(subnet_id, &original_owner);
//         let epoch = Network::get_current_epoch_as_u32();

//         let min = MinSubnetNodeConsecutiveIncludedEpochs::<Test>::get(subnet_id);
//         let max = MaxSubnetNodeConsecutiveIncludedEpochs::<Test>::get();

//         let new_min = min + 1;
//         let new_max = max - 1;

//         assert_ok!(Network::owner_update_subnet_node_consecutive_included_epochs(
//             RuntimeOrigin::signed(original_owner.clone()),
//             subnet_id,
//             new_min
//         ));

//         let value = SubnetNodeConsecutiveIncludedEpochs::<Test>::get(subnet_id);
//         assert_eq!(value, new_min);

//         assert_eq!(
//             *network_events().last().unwrap(),
//             Event::SubnetNodeConsecutiveIncludedEpochsUpdate {
//                 subnet_id: subnet_id,
//                 owner: original_owner.clone(),
//                 value: min_stake
//             }
//         );

//         assert_err!(
//             Network::owner_update_subnet_node_consecutive_included_epochs(
//                 RuntimeOrigin::signed(original_owner.clone()),
//                 subnet_id,
//                 min-1
//             ),
//             Error::<Teset>::InvalidSubnetNodeConsecutiveIncludedEpochs
//         );

//         assert_err!(
//             Network::owner_update_subnet_node_consecutive_included_epochs(
//                 RuntimeOrigin::signed(original_owner.clone()),
//                 subnet_id,
//                 max+1
//             ),
//             Error::<Teset>::InvalidSubnetNodeConsecutiveIncludedEpochs
//         );
//     });
// }

// #[test]
// fn test_owner_update_min_stake() {
//     new_test_ext().execute_with(|| {
//         let subnet_name: Vec<u8> = "subnet-name".into();
//         let deposit_amount: u128 = 10000000000000000000000;
//         let amount: u128 = 1000000000000000000000;
//         let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

//         build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
//         let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

//         let original_owner = account(1);

//         // Set initial owner
//         SubnetOwner::<Test>::insert(subnet_id, &original_owner);
//         let epoch = Network::get_current_epoch_as_u32();

//         let min_stake = SubnetMinStakeBalance::<Test>::get(subnet_id);

//         let new_min_stake = min_stake + 1;
//         assert_ok!(Network::owner_update_min_stake(
//             RuntimeOrigin::signed(original_owner.clone()),
//             subnet_id,
//             new_min_stake
//         ));

//         let min_stake = SubnetMinStakeBalance::<Test>::get(subnet_id);
//         assert_eq!(min_stake, new_min_stake);

//         assert_eq!(
//             *network_events().last().unwrap(),
//             Event::SubnetMinStakeBalanceUpdate {
//                 subnet_id: subnet_id,
//                 owner: original_owner.clone(),
//                 value: min_stake
//             }
//         );
//     });
// }

// #[test]
// fn test_owner_update_min_stake_invalid_min_stake() {
//     new_test_ext().execute_with(|| {
//         let subnet_name: Vec<u8> = "subnet-name".into();
//         let deposit_amount: u128 = 10000000000000000000000;
//         let amount: u128 = 1000000000000000000000;
//         let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

//         build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
//         let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

//         let original_owner = account(1);

//         // Set initial owner
//         SubnetOwner::<Test>::insert(subnet_id, &original_owner);
//         let epoch = Network::get_current_epoch_as_u32();

//         let value = MinSubnetMinStake::<Test>::get() - 1;

//         assert_err!(
//             Network::owner_update_min_stake(
//                 RuntimeOrigin::signed(original_owner.clone()),
//                 subnet_id,
//                 value
//             ),
//             Error::<Test>::InvalidSubnetMinStake
//         );

//         let value = MaxSubnetMinStake::<Test>::get() + 1;

//         assert_err!(
//             Network::owner_update_min_stake(
//                 RuntimeOrigin::signed(original_owner.clone()),
//                 subnet_id,
//                 value
//             ),
//             Error::<Test>::InvalidSubnetMinStake
//         );
//     });
// }

// #[test]
// fn test_owner_update_max_stake() {
//     new_test_ext().execute_with(|| {
//         let subnet_name: Vec<u8> = "subnet-name".into();
//         let deposit_amount: u128 = 10000000000000000000000;
//         let amount: u128 = 1000000000000000000000;
//         let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

//         build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
//         let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

//         let original_owner = account(1);

//         // Set initial owner
//         SubnetOwner::<Test>::insert(subnet_id, &original_owner);
//         let epoch = Network::get_current_epoch_as_u32();

//         let max_stake = SubnetMaxStakeBalance::<Test>::get(subnet_id);

//         let new_max_stake = max_stake - 1;
//         assert_ok!(Network::owner_update_max_stake(
//             RuntimeOrigin::signed(original_owner.clone()),
//             subnet_id,
//             new_max_stake
//         ));

//         let max_stake = SubnetMaxStakeBalance::<Test>::get(subnet_id);
//         assert_eq!(max_stake, new_max_stake);

//         assert_eq!(
//             *network_events().last().unwrap(),
//             Event::SubnetMaxStakeBalanceUpdate {
//                 subnet_id: subnet_id,
//                 owner: original_owner.clone(),
//                 value: max_stake
//             }
//         );
//     });
// }

// #[test]
// fn test_owner_update_max_stake_invalid_max_stake() {
//     new_test_ext().execute_with(|| {
//         let subnet_name: Vec<u8> = "subnet-name".into();
//         let deposit_amount: u128 = 10000000000000000000000;
//         let amount: u128 = 1000000000000000000000;
//         let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

//         build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
//         let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

//         let original_owner = account(1);

//         // Set initial owner
//         SubnetOwner::<Test>::insert(subnet_id, &original_owner);
//         let epoch = Network::get_current_epoch_as_u32();

//         let value = NetworkMaxStakeBalance::<Test>::get() + 1;

//         assert_err!(
//             Network::owner_update_max_stake(
//                 RuntimeOrigin::signed(original_owner.clone()),
//                 subnet_id,
//                 value
//             ),
//             Error::<Test>::InvalidSubnetMaxStake
//         );

//         let value = NetworkMaxStakeBalance::<Test>::get() + 1;

//         assert_err!(
//             Network::owner_update_max_stake(
//                 RuntimeOrigin::signed(original_owner.clone()),
//                 subnet_id,
//                 value
//             ),
//             Error::<Test>::InvalidSubnetMaxStake
//         );
//     });
// }

#[test]
fn test_owner_update_min_max_stake() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let epoch = Network::get_current_epoch_as_u32();

        let min_stake = SubnetMinStakeBalance::<Test>::get(subnet_id);
        let max_stake = NetworkMaxStakeBalance::<Test>::get();

        let new_min_stake = min_stake + 1;
        let new_max_stake = max_stake - 1;

        assert_ok!(Network::owner_update_min_max_stake(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_min_stake,
            new_max_stake
        ));

        let result_min_stake = SubnetMinStakeBalance::<Test>::get(subnet_id);
        assert_eq!(result_min_stake, new_min_stake);

        let result_max_stake = SubnetMaxStakeBalance::<Test>::get(subnet_id);
        assert_eq!(result_max_stake, new_max_stake);

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetMinMaxStakeBalanceUpdate {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                min: new_min_stake,
                max: new_max_stake
            }
        );

        assert_err!(
            Network::owner_update_min_max_stake(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                100,
                99
            ),
            Error::<Test>::InvalidValues
        );

        assert_err!(
            Network::owner_update_min_max_stake(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                min_stake - 1,
                max_stake
            ),
            Error::<Test>::InvalidSubnetMinStake
        );

        assert_err!(
            Network::owner_update_min_max_stake(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                min_stake,
                max_stake + 1
            ),
            Error::<Test>::InvalidSubnetMaxStake
        );
    });
}

#[test]
fn test_owner_update_delegate_stake_percentage() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let epoch = Network::get_current_epoch_as_u32();
        let block = System::block_number();

        let last_update = LastSubnetDelegateStakeRewardsUpdate::<Test>::get(subnet_id);
        let update_period = SubnetDelegateStakeRewardsUpdatePeriod::<Test>::get();

        let update_to_block = if block - last_update < update_period {
            last_update + update_period
        } else {
            System::block_number()
        };

        System::set_block_number(update_to_block + 1);

        let dstake_perc = SubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id);

        let new_dstake_perc = dstake_perc + 1;
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        assert_ok!(Network::owner_update_delegate_stake_percentage(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_dstake_perc
        ));

        assert_eq!(
            SubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id),
            dstake_perc
        );
        let pending = PendingSubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_dstake_perc);
        assert_eq!(
            pending.effective_subnet_epoch,
            current_subnet_epoch.saturating_add(1)
        );
        assert_eq!(pending.owner, original_owner.clone());

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetDelegateStakeRewardsPercentageUpdateScheduled {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                value: new_dstake_perc,
                effective_subnet_epoch: current_subnet_epoch.saturating_add(1)
            }
        );
    });
}

#[test]
fn test_owner_delegate_stake_percentage_applies_to_next_consensus_epoch_rewards() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let last_update = LastSubnetDelegateStakeRewardsUpdate::<Test>::get(subnet_id);
        let update_period = SubnetDelegateStakeRewardsUpdatePeriod::<Test>::get();
        System::set_block_number(last_update + update_period + 1);

        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let active_rate = SubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id);
        let new_rate = active_rate + 1;

        assert_ok!(Network::owner_update_delegate_stake_percentage(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_rate
        ));

        let (current_epoch_rewards, _) = Network::calculate_rewards_for_epoch(
            subnet_id,
            1_000_000_000_000_000_000,
            Network::percentage_factor_as_u128(),
            current_subnet_epoch.saturating_add(1),
        );
        assert_eq!(
            current_epoch_rewards.delegate_stake_rewards,
            Network::percent_mul(current_epoch_rewards.subnet_rewards, active_rate)
        );
        assert_eq!(
            SubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id),
            active_rate
        );
        assert!(PendingSubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id).is_some());

        let (next_epoch_rewards, _) = Network::calculate_rewards_for_epoch(
            subnet_id,
            1_000_000_000_000_000_000,
            Network::percentage_factor_as_u128(),
            current_subnet_epoch.saturating_add(2),
        );
        assert_eq!(
            next_epoch_rewards.delegate_stake_rewards,
            Network::percent_mul(next_epoch_rewards.subnet_rewards, new_rate)
        );
        assert_eq!(
            SubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id),
            new_rate
        );
        assert!(PendingSubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id).is_none());
        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetDelegateStakeRewardsPercentageUpdate {
                subnet_id,
                owner: original_owner,
                value: new_rate,
            }
        );
    });
}

#[test]
fn test_remove_subnet_clears_pending_delegate_stake_percentage_update() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let last_update = LastSubnetDelegateStakeRewardsUpdate::<Test>::get(subnet_id);
        let update_period = SubnetDelegateStakeRewardsUpdatePeriod::<Test>::get();
        System::set_block_number(last_update + update_period + 1);

        let new_rate = SubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id) + 1;
        assert_ok!(Network::owner_update_delegate_stake_percentage(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            new_rate
        ));
        assert!(PendingSubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id).is_some());

        Network::do_remove_subnet(subnet_id, SubnetRemovalReason::Owner);
        assert!(PendingSubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id).is_none());
    });
}

#[test]
fn test_owner_update_delegate_stake_percentage_update_too_soon() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let epoch = Network::get_current_epoch_as_u32();
        let block = System::block_number();

        let last_update = LastSubnetDelegateStakeRewardsUpdate::<Test>::get(subnet_id);
        let update_period = SubnetDelegateStakeRewardsUpdatePeriod::<Test>::get();

        let update_to_block = if block - last_update < update_period {
            last_update + update_period
        } else {
            System::block_number()
        };

        System::set_block_number(update_to_block + 1);

        let dstake_perc = SubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id);

        let new_dstake_perc = dstake_perc + 1;
        assert_ok!(Network::owner_update_delegate_stake_percentage(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_dstake_perc
        ));

        assert_err!(
            Network::owner_update_delegate_stake_percentage(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                new_dstake_perc
            ),
            Error::<Test>::DelegateStakePercentageUpdateTooSoon
        );
    });
}

#[test]
fn test_owner_update_delegate_stake_percentage_period_uses_saturating_add() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        SubnetDelegateStakeRewardsUpdatePeriod::<Test>::put(10);
        LastSubnetDelegateStakeRewardsUpdate::<Test>::insert(subnet_id, u32::MAX - 5);
        System::set_block_number(20);

        let new_dstake_perc = SubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id) + 1;

        assert_err!(
            Network::owner_update_delegate_stake_percentage(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                new_dstake_perc
            ),
            Error::<Test>::DelegateStakePercentageUpdateTooSoon
        );
        assert!(PendingSubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id).is_none());
    });
}

#[test]
fn test_owner_update_delegate_stake_percentage_update_too_large() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let epoch = Network::get_current_epoch_as_u32();
        let block = System::block_number();

        let last_update = LastSubnetDelegateStakeRewardsUpdate::<Test>::get(subnet_id);
        let update_period = SubnetDelegateStakeRewardsUpdatePeriod::<Test>::get();

        let update_to_block = if last_update + update_period > block {
            last_update + update_period
        } else {
            System::block_number()
        };

        System::set_block_number(update_to_block + 1);

        let dstake_perc = SubnetDelegateStakeRewardsPercentage::<Test>::get(subnet_id);

        let new_dstake_perc = dstake_perc + 1;
        assert_ok!(Network::owner_update_delegate_stake_percentage(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_dstake_perc
        ));

        let block = System::block_number();
        let last_update = LastSubnetDelegateStakeRewardsUpdate::<Test>::get(subnet_id);
        let update_to_block = if last_update + update_period > block {
            last_update + update_period
        } else {
            System::block_number()
        };

        System::set_block_number(update_to_block + 1);

        assert_err!(
            Network::owner_update_delegate_stake_percentage(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                test_percent(95, 100)
            ),
            Error::<Test>::DelegateStakePercentageAbsDiffTooLarge
        );

        assert_err!(
            Network::owner_update_delegate_stake_percentage(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                0
            ),
            Error::<Test>::DelegateStakePercentageAbsDiffTooLarge
        );

        // insert to max
        PendingSubnetDelegateStakeRewardsPercentage::<Test>::remove(subnet_id);
        SubnetDelegateStakeRewardsPercentage::<Test>::insert(
            subnet_id,
            MaxDelegateStakePercentage::<Test>::get(),
        );
        assert_err!(
            Network::owner_update_delegate_stake_percentage(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                MaxDelegateStakePercentage::<Test>::get() + 1
            ),
            Error::<Test>::InvalidDelegateStakePercentage
        );

        PendingSubnetDelegateStakeRewardsPercentage::<Test>::remove(subnet_id);
        SubnetDelegateStakeRewardsPercentage::<Test>::insert(
            subnet_id,
            MinDelegateStakePercentage::<Test>::get(),
        );
        assert_err!(
            Network::owner_update_delegate_stake_percentage(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                MinDelegateStakePercentage::<Test>::get() - 1
            ),
            Error::<Test>::InvalidDelegateStakePercentage
        );
    });
}

#[test]
fn test_owner_update_max_registered_nodes() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let epoch = Network::get_current_epoch_as_u32();

        let max_reg_nodes = MaxRegisteredNodes::<Test>::get(subnet_id);

        let new_max_reg_nodes = max_reg_nodes - 1;
        assert_ok!(Network::owner_update_target_node_registrations_per_epoch(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_max_reg_nodes
        ));
        assert_ok!(Network::owner_update_max_registered_nodes(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_max_reg_nodes
        ));

        let max_reg_nodes = MaxRegisteredNodes::<Test>::get(subnet_id);
        assert_eq!(max_reg_nodes, new_max_reg_nodes);

        assert_eq!(
            *network_events().last().unwrap(),
            Event::MaxRegisteredNodesUpdate {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                value: max_reg_nodes
            }
        );
    });
}

#[test]
fn test_owner_update_max_registered_nodes_invalid_max_registered_nodes() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);
        let epoch = Network::get_current_epoch_as_u32();

        let value = MinMaxRegisteredNodes::<Test>::get() - 1;

        assert_err!(
            Network::owner_update_max_registered_nodes(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                value
            ),
            Error::<Test>::InvalidMaxRegisteredNodes
        );

        let value = MaxMaxRegisteredNodes::<Test>::get() + 1;

        assert_err!(
            Network::owner_update_max_registered_nodes(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                value
            ),
            Error::<Test>::InvalidMaxRegisteredNodes
        );

        let value = TargetNodeRegistrationsPerEpoch::<Test>::get(subnet_id) - 1;
        assert_err!(
            Network::owner_update_max_registered_nodes(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                value
            ),
            Error::<Test>::InvalidMaxRegisteredNodes
        );
    });
}

#[test]
fn test_transfer_and_accept_ownership_works() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);

        let subnet_id = 1;
        let original_owner = account(1);
        let new_owner = account(2);

        // Set initial owner
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        // Transfer to new owner
        assert_ok!(Network::transfer_subnet_ownership(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_owner.clone()
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::TransferPendingSubnetOwner {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                new_owner: new_owner.clone()
            }
        );

        // Accept by new owner
        assert_ok!(Network::accept_subnet_ownership(
            RuntimeOrigin::signed(new_owner.clone()),
            subnet_id
        ));

        // Check ownership
        assert_eq!(PendingSubnetOwner::<Test>::try_get(subnet_id), Err(()));
        assert_eq!(SubnetOwner::<Test>::get(subnet_id), Some(new_owner.clone()));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::AcceptPendingSubnetOwner {
                subnet_id: subnet_id,
                new_owner: new_owner.clone()
            }
        );
    });
}

#[test]
fn test_transfer_cannot_be_accepted_by_wrong_account() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let original_owner = account(3);
        let new_owner = account(4);
        let wrong_account = account(5);

        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        assert_ok!(Network::transfer_subnet_ownership(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_owner
        ));

        assert_err!(
            Network::accept_subnet_ownership(RuntimeOrigin::signed(wrong_account), subnet_id),
            Error::<Test>::NotPendingSubnetOwner
        );
    });
}

#[test]
fn test_owner_can_cancel_transfer() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);

        let subnet_id = 1;
        let original_owner = account(6);
        let new_owner = account(7);

        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        assert_ok!(Network::transfer_subnet_ownership(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_owner.clone()
        ));

        assert_eq!(
            PendingSubnetOwner::<Test>::get(subnet_id),
            Some(new_owner.clone())
        );

        assert_ok!(Network::cancel_subnet_ownership_transfer(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id
        ));

        assert_eq!(PendingSubnetOwner::<Test>::try_get(subnet_id), Err(()));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::CancelPendingSubnetOwner {
                subnet_id,
                owner: original_owner.clone()
            }
        );

        assert_err!(
            Network::accept_subnet_ownership(RuntimeOrigin::signed(new_owner.clone()), subnet_id),
            Error::<Test>::NoPendingSubnetOwner
        );
    });
}

#[test]
fn test_transfer_subnet_ownership_rejects_zero_owner() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let original_owner = account(6);
        let zero_address =
            <Test as frame_system::Config>::AccountId::decode(&mut TrailingZeroInput::zeroes())
                .unwrap();

        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        assert_err!(
            Network::transfer_subnet_ownership(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                zero_address
            ),
            Error::<Test>::InvalidPendingSubnetOwner
        );

        assert_eq!(PendingSubnetOwner::<Test>::try_get(subnet_id), Err(()));
    });
}

#[test]
fn test_accept_without_pending_transfer_should_fail() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let user = account(8);

        assert_err!(
            Network::accept_subnet_ownership(RuntimeOrigin::signed(user), subnet_id),
            Error::<Test>::NoPendingSubnetOwner
        );
    });
}

#[test]
fn test_cancel_without_pending_transfer_should_fail() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let owner = account(8);

        SubnetOwner::<Test>::insert(subnet_id, &owner);

        assert_err!(
            Network::cancel_subnet_ownership_transfer(RuntimeOrigin::signed(owner), subnet_id),
            Error::<Test>::NoPendingSubnetOwner
        );
    });
}

#[test]
fn test_non_owner_cannot_cancel_transfer() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let actual_owner = account(9);
        let new_owner = account(10);
        let fake_owner = account(11);

        SubnetOwner::<Test>::insert(subnet_id, &actual_owner);

        assert_ok!(Network::transfer_subnet_ownership(
            RuntimeOrigin::signed(actual_owner.clone()),
            subnet_id,
            new_owner.clone()
        ));

        assert_err!(
            Network::cancel_subnet_ownership_transfer(RuntimeOrigin::signed(fake_owner), subnet_id),
            Error::<Test>::NotSubnetOwner
        );

        assert_eq!(PendingSubnetOwner::<Test>::get(subnet_id), Some(new_owner));
    });
}

#[test]
fn test_non_owner_cannot_transfer() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let actual_owner = account(9);
        let fake_owner = account(10);
        let target = account(11);

        SubnetOwner::<Test>::insert(subnet_id, &actual_owner);

        assert_err!(
            Network::transfer_subnet_ownership(
                RuntimeOrigin::signed(fake_owner),
                subnet_id,
                target
            ),
            Error::<Test>::NotSubnetOwner
        );
    });
}

#[test]
fn test_owner_add_bootnode_access() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);
        let subnet_id = 1;

        let subnet_name: Vec<u8> = "subnet-name".into();
        let subnet_data = SubnetData {
            id: subnet_id,
            friendly_id: subnet_id,
            name: subnet_name.clone(),
            repo: subnet_name.clone(),
            description: subnet_name.clone(),
            misc: subnet_name.clone(),
            state: SubnetState::Registered,
            consensus_eligible_from_subnet_epoch: None,
            pause: None,
        };

        // Store subnet data
        SubnetsData::<Test>::insert(subnet_id, &subnet_data);

        let original_owner = account(60);

        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_access = account(70);

        let access_set = SubnetBootnodeAccess::<Test>::get(subnet_id);

        assert_ok!(Network::owner_add_bootnode_access(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_access.clone()
        ));

        let new_access_set = SubnetBootnodeAccess::<Test>::get(subnet_id);

        assert!(new_access_set.get(&new_access.clone()).is_some());

        assert_eq!(
            *network_events().last().unwrap(),
            Event::AddSubnetBootnodeAccess {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                new_account: new_access.clone()
            }
        );

        // let bv = |b: u8| NetworkBytes::<Test>::try_from(vec![b]).unwrap();

        // // Add a bootnode using the added account
        // let add_set = BTreeSet::from([bv(1), bv(2)]);
        // assert_ok!(Network::update_bootnodes(
        //     RuntimeOrigin::signed(new_access.clone()),
        //     subnet_id,
        //     add_set.clone(),
        //     BTreeSet::new(),
        // ));
        // Helper to build a bounded vec from bytes
        let bv = |b: u8| NetworkBytes::<Test>::try_from(vec![b]).unwrap();

        // --- Case 1: Add bootnodes ---
        // let add_map = BTreeMap::from([(peer(1), bv(1)), (peer(2), bv(2))]);
        let add_map = BTreeMap::from([
            (
                peer(1),
                get_multiaddr(Some(subnet_id), Some(1), None).unwrap(),
            ),
            (
                peer(2),
                get_multiaddr(Some(subnet_id), Some(2), None).unwrap(),
            ),
        ]);
        assert_ok!(Network::update_bootnodes(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            add_map.clone(),
            BTreeSet::new(),
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::BootnodesUpdated {
                subnet_id,
                added: add_map.clone(),
                removed: BTreeSet::new(),
            }
        );

        // Fail if access already granted
        assert_err!(
            Network::owner_add_bootnode_access(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                new_access.clone()
            ),
            Error::<Test>::InBootnodeAccessList
        );

        SubnetBootnodeAccess::<Test>::remove(subnet_id);

        let max_access_nodes = MaxSubnetBootnodeAccess::<Test>::get();

        let mut touched = false; // make sure logic is touched

        for n in 0..max_access_nodes + 2 {
            let _n = n + 1;
            let account = account(n);
            if _n > max_access_nodes {
                touched = true;
                assert_err!(
                    Network::owner_add_bootnode_access(
                        RuntimeOrigin::signed(original_owner.clone()),
                        subnet_id,
                        account
                    ),
                    Error::<Test>::MaxSubnetBootnodeAccess
                );
            } else {
                assert_ok!(Network::owner_add_bootnode_access(
                    RuntimeOrigin::signed(original_owner.clone()),
                    subnet_id,
                    account
                ));
            }
        }

        assert!(touched);
    });
}

#[test]
fn test_update_bootnodes_rejects_oversized_native_peer_id() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let oversized_peer_id = PeerId(vec![
            b'a';
            (<Test as crate::Config>::MaxVectorLength::get() + 1)
                as usize
        ]);

        assert_err!(
            Network::update_bootnodes(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                BTreeMap::new(),
                BTreeSet::from([oversized_peer_id])
            ),
            Error::<Test>::InvalidBootnodePeerId
        );
    });
}

#[test]
fn test_owner_remove_bootnode_access() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);
        let subnet_id = 1;

        let subnet_name: Vec<u8> = "subnet-name".into();
        let subnet_data = SubnetData {
            id: subnet_id,
            friendly_id: subnet_id,
            name: subnet_name.clone(),
            repo: subnet_name.clone(),
            description: subnet_name.clone(),
            misc: subnet_name.clone(),
            state: SubnetState::Registered,
            consensus_eligible_from_subnet_epoch: None,
            pause: None,
        };

        // Store subnet data
        SubnetsData::<Test>::insert(subnet_id, &subnet_data);

        let original_owner = account(60);

        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let new_access = account(70);

        let access_set = SubnetBootnodeAccess::<Test>::get(subnet_id);

        assert_ok!(Network::owner_add_bootnode_access(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_access.clone()
        ));

        let new_access_set = SubnetBootnodeAccess::<Test>::get(subnet_id);

        assert!(new_access_set.get(&new_access.clone()).is_some());

        assert_ok!(Network::owner_remove_bootnode_access(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_access.clone()
        ));

        let new_access_set = SubnetBootnodeAccess::<Test>::get(subnet_id);

        assert!(new_access_set.get(&new_access.clone()).into_iter().count() == 0);
    });
}

#[test]
fn test_not_subnet_owner_and_invalid_subnet_id() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let actual_owner = account(9);
        let fake_owner = account(10);
        let target = account(11);

        SubnetOwner::<Test>::insert(subnet_id, &actual_owner);

        assert_err!(
            Network::owner_pause_subnet(RuntimeOrigin::signed(fake_owner), subnet_id),
            Error::<Test>::NotSubnetOwner
        );

        assert_err!(
            Network::owner_unpause_subnet(RuntimeOrigin::signed(fake_owner), subnet_id),
            Error::<Test>::NotSubnetOwner
        );

        assert_err!(
            Network::owner_deactivate_subnet(RuntimeOrigin::signed(fake_owner), subnet_id),
            Error::<Test>::NotSubnetOwner
        );

        let new_subnet_name: Vec<u8> = "new-subnet-name".into();

        assert_err!(
            Network::owner_update_name(
                RuntimeOrigin::signed(fake_owner),
                subnet_id,
                new_subnet_name.clone()
            ),
            Error::<Test>::NotSubnetOwner
        );

        let new_subnet_repo: Vec<u8> = "new-subnet-repo".into();

        assert_err!(
            Network::owner_update_repo(
                RuntimeOrigin::signed(fake_owner),
                subnet_id,
                new_subnet_name.clone()
            ),
            Error::<Test>::NotSubnetOwner
        );

        let new_subnet_description: Vec<u8> = "new-subnet-description".into();

        assert_err!(
            Network::owner_update_description(
                RuntimeOrigin::signed(fake_owner),
                subnet_id,
                new_subnet_description
            ),
            Error::<Test>::NotSubnetOwner
        );

        let new_subnet_misc: Vec<u8> = "new-subnet-misc".into();

        assert_err!(
            Network::owner_update_misc(
                RuntimeOrigin::signed(fake_owner),
                subnet_id,
                new_subnet_misc
            ),
            Error::<Test>::NotSubnetOwner
        );

        assert_err!(
            Network::owner_update_churn_limit(RuntimeOrigin::signed(fake_owner), subnet_id, 1),
            Error::<Test>::NotSubnetOwner
        );

        assert_err!(
            Network::owner_update_registration_queue_epochs(
                RuntimeOrigin::signed(fake_owner),
                subnet_id,
                1
            ),
            Error::<Test>::NotSubnetOwner
        );

        assert_err!(
            Network::owner_update_idle_classification_epochs(
                RuntimeOrigin::signed(fake_owner),
                subnet_id,
                1
            ),
            Error::<Test>::NotSubnetOwner
        );

        assert_err!(
            Network::owner_update_included_classification_epochs(
                RuntimeOrigin::signed(fake_owner),
                subnet_id,
                1
            ),
            Error::<Test>::NotSubnetOwner
        );

        let new_coldkeys = BTreeMap::from([(1, 1)]);
        assert_err!(
            Network::owner_add_or_update_initial_validators(
                RuntimeOrigin::signed(fake_owner),
                subnet_id,
                new_coldkeys.clone()
            ),
            Error::<Test>::NotSubnetOwner
        );

        let remove_coldkeys = BTreeSet::from([1]);
        assert_err!(
            Network::owner_remove_initial_validators(
                RuntimeOrigin::signed(fake_owner),
                subnet_id,
                remove_coldkeys.clone()
            ),
            Error::<Test>::NotSubnetOwner
        );

        assert_err!(
            Network::owner_update_min_max_stake(RuntimeOrigin::signed(fake_owner), subnet_id, 1, 2),
            Error::<Test>::NotSubnetOwner
        );
        assert_err!(
            Network::owner_update_delegate_stake_percentage(
                RuntimeOrigin::signed(fake_owner),
                subnet_id,
                1
            ),
            Error::<Test>::NotSubnetOwner
        );
        assert_err!(
            Network::owner_update_max_registered_nodes(
                RuntimeOrigin::signed(fake_owner),
                subnet_id,
                1
            ),
            Error::<Test>::NotSubnetOwner
        );

        assert_err!(
            Network::owner_add_bootnode_access(
                RuntimeOrigin::signed(fake_owner),
                subnet_id,
                account(1)
            ),
            Error::<Test>::NotSubnetOwner
        );
        assert_err!(
            Network::owner_remove_bootnode_access(
                RuntimeOrigin::signed(fake_owner),
                subnet_id,
                account(1)
            ),
            Error::<Test>::NotSubnetOwner
        );
    });
}

#[test]
fn test_owner_revert_emergency_validator_set() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        // Set emergency validator set
        let subnet_node_ids = vec![1, 2];
        let validator_data = EmergencySubnetValidatorData {
            subnet_node_ids,
            target_emergency_validators_epochs: 0,
            max_emergency_validators_epoch: 0,
            total_epochs: 0,
            ..Default::default()
        };

        EmergencySubnetNodeElectionData::<Test>::insert(subnet_id, validator_data);

        // Verify it exists
        assert!(EmergencySubnetNodeElectionData::<Test>::contains_key(
            subnet_id
        ));

        // Revert emergency validator set while active
        assert_err!(
            Network::owner_revert_emergency_validator_set(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id
            ),
            Error::<Test>::SubnetMustBePaused
        );

        // ---

        run_to_first_pause_eligible_subnet_slot(subnet_id);
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
        ));

        // Revert emergency validator set
        assert_ok!(Network::owner_revert_emergency_validator_set(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id
        ));

        // Verify it's removed
        assert!(!EmergencySubnetNodeElectionData::<Test>::contains_key(
            subnet_id
        ));

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetForkRevert {
                subnet_id: subnet_id,
                owner: original_owner.clone()
            }
        );
    });
}

#[test]
fn test_owner_update_min_subnet_node_reputation() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let old_value = MinSubnetNodeReputation::<Test>::get(subnet_id);
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let new_value = test_percent(1, 2);

        assert_ok!(Network::owner_update_min_subnet_node_reputation(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_value
        ));

        let result_value = MinSubnetNodeReputation::<Test>::get(subnet_id);
        assert_eq!(result_value, old_value);
        assert_eq!(
            Network::get_min_subnet_node_reputation_for_epoch(subnet_id, current_subnet_epoch),
            old_value
        );
        assert_eq!(
            Network::get_min_subnet_node_reputation_for_epoch(subnet_id, current_subnet_epoch + 1),
            new_value
        );

        let pending = PendingMinSubnetNodeReputation::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_value);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);
        assert_eq!(pending.owner, original_owner.clone());

        assert_eq!(
            *network_events().last().unwrap(),
            Event::MinSubnetNodeReputationUpdateScheduled {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                value: new_value,
                effective_subnet_epoch: current_subnet_epoch + 1
            }
        );

        let replacement_value = test_percent(1, 4);
        assert_ok!(Network::owner_update_min_subnet_node_reputation(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            replacement_value
        ));
        let pending = PendingMinSubnetNodeReputation::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, replacement_value);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);
    });
}

#[test]
fn test_owner_update_reputation_factors_schedules_single_factor() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let current_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let new_value = MinNodeReputationFactor::<Test>::get();

        assert_ok!(Network::owner_update_reputation_factors(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            SubnetReputationFactorUpdates {
                absent_decrease: Some(new_value),
                ..Default::default()
            }
        ));

        let schedule = SubnetReputationFactorSchedules::<Test>::get(subnet_id);
        let pending = schedule.pending.unwrap();

        assert_ne!(schedule.current.absent_decrease, new_value);
        assert_eq!(pending.factors.absent_decrease, new_value);
        assert_eq!(
            pending.factors.included_increase,
            schedule.current.included_increase
        );
        assert_eq!(pending.effective_subnet_epoch, current_epoch + 1);
        assert_eq!(
            Network::get_reputation_factors_for_epoch(subnet_id, current_epoch).absent_decrease,
            schedule.current.absent_decrease
        );
        assert_eq!(
            Network::get_reputation_factors_for_epoch(subnet_id, current_epoch + 1).absent_decrease,
            new_value
        );

        assert_eq!(
            *network_events().last().unwrap(),
            Event::SubnetReputationFactorsUpdateScheduled {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                factors: pending.factors,
                effective_subnet_epoch: current_epoch + 1
            }
        );
    });
}

#[test]
fn test_owner_update_consensus_validator_node_count_decay() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &owner);

        let percentage_factor = Network::percentage_factor_as_u128();
        let new_value = test_percent(1, 2);
        let scheduled_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        assert_eq!(
            ConsensusValidatorNodeCountDecay::<Test>::get(subnet_id),
            percentage_factor
        );

        assert_ok!(Network::owner_update_consensus_validator_node_count_decay(
            RuntimeOrigin::signed(owner.clone()),
            subnet_id,
            new_value
        ));

        assert_eq!(
            ConsensusValidatorNodeCountDecay::<Test>::get(subnet_id),
            percentage_factor
        );
        assert_eq!(
            Network::get_consensus_validator_node_count_decay_for_epoch(
                subnet_id,
                scheduled_subnet_epoch,
            ),
            percentage_factor
        );
        assert_eq!(
            Network::get_consensus_validator_node_count_decay_for_epoch(
                subnet_id,
                scheduled_subnet_epoch + 1,
            ),
            new_value
        );
        let pending = PendingConsensusValidatorNodeCountDecay::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_value);
        assert_eq!(pending.effective_subnet_epoch, scheduled_subnet_epoch + 1);
        assert_eq!(pending.owner, owner.clone());
        assert_eq!(
            LastConsensusValidatorNodeCountDecayUpdate::<Test>::get(subnet_id),
            Some(Network::get_current_epoch_as_u32())
        );
        assert_eq!(
            *network_events().last().unwrap(),
            Event::ConsensusValidatorNodeCountDecayUpdateScheduled {
                subnet_id,
                owner: owner.clone(),
                value: new_value,
                effective_subnet_epoch: scheduled_subnet_epoch + 1,
            }
        );

        assert_eq!(
            ConsensusValidatorNodeCountDecay::<Test>::get(subnet_id + 1),
            percentage_factor
        );

        assert_err!(
            Network::owner_update_consensus_validator_node_count_decay(
                RuntimeOrigin::signed(account(99)),
                subnet_id,
                0
            ),
            Error::<Test>::NotSubnetOwner
        );

        assert_err!(
            Network::owner_update_consensus_validator_node_count_decay(
                RuntimeOrigin::signed(owner.clone()),
                subnet_id,
                percentage_factor + 1
            ),
            Error::<Test>::InvalidPercent
        );

        assert_err!(
            Network::owner_update_consensus_validator_node_count_decay(
                RuntimeOrigin::signed(owner.clone()),
                subnet_id,
                0
            ),
            Error::<Test>::ConsensusValidatorNodeCountDecayUpdateTooSoon
        );

        increase_epochs(ConsensusValidatorNodeCountDecayUpdateInterval::<Test>::get());

        assert_err!(
            Network::owner_update_consensus_validator_node_count_decay(
                RuntimeOrigin::signed(owner.clone()),
                subnet_id,
                0
            ),
            Error::<Test>::OwnerParameterUpdatePendingActivation
        );

        increase_epochs(1);

        assert_ok!(Network::owner_update_consensus_validator_node_count_decay(
            RuntimeOrigin::signed(owner.clone()),
            subnet_id,
            0
        ));
        assert_eq!(
            ConsensusValidatorNodeCountDecay::<Test>::get(subnet_id),
            new_value
        );
        let replacement_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let pending = PendingConsensusValidatorNodeCountDecay::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, 0);
        assert_eq!(pending.effective_subnet_epoch, replacement_subnet_epoch + 1);
        assert_eq!(pending.owner, owner);
        assert_eq!(
            Network::get_consensus_validator_node_count_decay_for_epoch(
                subnet_id,
                replacement_subnet_epoch,
            ),
            new_value
        );
        assert_eq!(
            Network::get_consensus_validator_node_count_decay_for_epoch(
                subnet_id,
                replacement_subnet_epoch + 1,
            ),
            0
        );
    });
}

#[test]
fn test_owner_update_consensus_validator_node_count_decay_respects_admin_interval() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &owner);
        ConsensusValidatorNodeCountDecayUpdateInterval::<Test>::set(2);

        assert_ok!(Network::owner_update_consensus_validator_node_count_decay(
            RuntimeOrigin::signed(owner.clone()),
            subnet_id,
            test_percent(1, 2)
        ));

        increase_epochs(1);
        assert_err!(
            Network::owner_update_consensus_validator_node_count_decay(
                RuntimeOrigin::signed(owner.clone()),
                subnet_id,
                0
            ),
            Error::<Test>::ConsensusValidatorNodeCountDecayUpdateTooSoon
        );

        increase_epochs(1);
        assert_ok!(Network::owner_update_consensus_validator_node_count_decay(
            RuntimeOrigin::signed(owner),
            subnet_id,
            0
        ));
    });
}

#[test]
fn test_owner_update_consensus_validator_stake_weight_power() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);
        let subnet_name: Vec<u8> = "stake-weight-power-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &owner);

        let percentage_factor = Network::percentage_factor_as_u128();
        let new_value = test_percent(1, 2);
        let scheduled_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        assert_eq!(
            ConsensusValidatorStakeWeightPower::<Test>::get(subnet_id),
            percentage_factor
        );
        assert_eq!(
            ConsensusValidatorStakeWeightPower::<Test>::get(subnet_id + 1),
            percentage_factor
        );
        assert_eq!(
            LastConsensusValidatorStakeWeightPowerUpdate::<Test>::get(subnet_id),
            None
        );

        assert_ok!(
            Network::owner_update_consensus_validator_stake_weight_power(
                RuntimeOrigin::signed(owner.clone()),
                subnet_id,
                new_value
            )
        );

        assert_eq!(
            ConsensusValidatorStakeWeightPower::<Test>::get(subnet_id),
            percentage_factor
        );
        assert_eq!(
            Network::get_consensus_validator_stake_weight_power_for_epoch(
                subnet_id,
                scheduled_subnet_epoch,
            ),
            percentage_factor
        );
        assert_eq!(
            Network::get_consensus_validator_stake_weight_power_for_epoch(
                subnet_id,
                scheduled_subnet_epoch + 1,
            ),
            new_value
        );
        let pending = PendingConsensusValidatorStakeWeightPower::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_value);
        assert_eq!(pending.effective_subnet_epoch, scheduled_subnet_epoch + 1);
        assert_eq!(pending.owner, owner.clone());
        assert_eq!(
            LastConsensusValidatorStakeWeightPowerUpdate::<Test>::get(subnet_id),
            Some(Network::get_current_epoch_as_u32())
        );
        assert_eq!(
            *network_events().last().unwrap(),
            Event::ConsensusValidatorStakeWeightPowerUpdateScheduled {
                subnet_id,
                owner: owner.clone(),
                value: new_value,
                effective_subnet_epoch: scheduled_subnet_epoch + 1,
            }
        );

        assert_err!(
            Network::owner_update_consensus_validator_stake_weight_power(
                RuntimeOrigin::signed(owner.clone()),
                subnet_id,
                0
            ),
            Error::<Test>::ConsensusValidatorStakeWeightPowerUpdateTooSoon
        );

        increase_epochs(ConsensusValidatorStakeWeightPowerUpdateInterval::<Test>::get());
        assert_err!(
            Network::owner_update_consensus_validator_stake_weight_power(
                RuntimeOrigin::signed(owner.clone()),
                subnet_id,
                0
            ),
            Error::<Test>::OwnerParameterUpdatePendingActivation
        );

        increase_epochs(1);
        assert_ok!(
            Network::owner_update_consensus_validator_stake_weight_power(
                RuntimeOrigin::signed(owner.clone()),
                subnet_id,
                0
            )
        );
        assert_eq!(
            ConsensusValidatorStakeWeightPower::<Test>::get(subnet_id),
            new_value
        );
        let replacement_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let pending = PendingConsensusValidatorStakeWeightPower::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, 0);
        assert_eq!(pending.effective_subnet_epoch, replacement_subnet_epoch + 1);
        assert_eq!(pending.owner, owner);
        assert_eq!(
            Network::get_consensus_validator_stake_weight_power_for_epoch(
                subnet_id,
                replacement_subnet_epoch,
            ),
            new_value
        );
        assert_eq!(
            Network::get_consensus_validator_stake_weight_power_for_epoch(
                subnet_id,
                replacement_subnet_epoch + 1,
            ),
            0
        );
    });
}

#[test]
fn test_owner_update_consensus_validator_stake_weight_power_bounds_and_authorization() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);
        let subnet_name: Vec<u8> = "stake-weight-power-bounds-subnet".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();
        let owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &owner);

        let min = test_percent(1, 4);
        let max = test_percent(3, 4);
        MinConsensusValidatorStakeWeightPower::<Test>::set(min);
        MaxConsensusValidatorStakeWeightPower::<Test>::set(max);
        ConsensusValidatorStakeWeightPowerUpdateInterval::<Test>::set(0);

        assert_err!(
            Network::owner_update_consensus_validator_stake_weight_power(
                RuntimeOrigin::signed(account(99)),
                subnet_id,
                min
            ),
            Error::<Test>::NotSubnetOwner
        );
        assert_err!(
            Network::owner_update_consensus_validator_stake_weight_power(
                RuntimeOrigin::signed(owner.clone()),
                subnet_id,
                min - 1
            ),
            Error::<Test>::InvalidPercent
        );
        assert_err!(
            Network::owner_update_consensus_validator_stake_weight_power(
                RuntimeOrigin::signed(owner.clone()),
                subnet_id,
                max + 1
            ),
            Error::<Test>::InvalidPercent
        );

        assert_ok!(
            Network::owner_update_consensus_validator_stake_weight_power(
                RuntimeOrigin::signed(owner.clone()),
                subnet_id,
                min
            )
        );
        assert_ok!(
            Network::owner_update_consensus_validator_stake_weight_power(
                RuntimeOrigin::signed(owner),
                subnet_id,
                max
            )
        );
        assert_eq!(
            ConsensusValidatorStakeWeightPower::<Test>::get(subnet_id),
            Network::percentage_factor_as_u128()
        );
        let pending = PendingConsensusValidatorStakeWeightPower::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, max);
        assert_eq!(
            Network::get_consensus_validator_stake_weight_power_for_epoch(
                subnet_id,
                pending.effective_subnet_epoch,
            ),
            max
        );
    });
}

#[test]
fn test_owner_update_reputation_factors_schedules_multiple_factors() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let min_value = MinNodeReputationFactor::<Test>::get();
        let max_value = crate::MaxNodeReputationFactor::<Test>::get();

        assert_ok!(Network::owner_update_reputation_factors(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            SubnetReputationFactorUpdates {
                included_increase: Some(min_value),
                validator_non_consensus_decrease: Some(max_value),
                ..Default::default()
            }
        ));

        let schedule = SubnetReputationFactorSchedules::<Test>::get(subnet_id);
        let pending = schedule.pending.unwrap();

        assert_eq!(pending.factors.included_increase, min_value);
        assert_eq!(pending.factors.validator_non_consensus_decrease, max_value);
        assert_eq!(
            pending.factors.absent_decrease,
            schedule.current.absent_decrease
        );
    });
}

#[test]
fn test_owner_update_reputation_factors_merges_and_normalizes_pending_schedule() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let current_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let min_value = MinNodeReputationFactor::<Test>::get();
        let max_value = crate::MaxNodeReputationFactor::<Test>::get();

        assert_ok!(Network::owner_update_reputation_factors(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            SubnetReputationFactorUpdates {
                absent_decrease: Some(min_value),
                ..Default::default()
            }
        ));

        assert_ok!(Network::owner_update_reputation_factors(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            SubnetReputationFactorUpdates {
                included_increase: Some(max_value),
                ..Default::default()
            }
        ));

        let schedule = SubnetReputationFactorSchedules::<Test>::get(subnet_id);
        let pending = schedule.pending.unwrap();
        assert_eq!(pending.factors.absent_decrease, min_value);
        assert_eq!(pending.factors.included_increase, max_value);

        SubnetReputationFactorSchedules::<Test>::mutate(subnet_id, |schedule| {
            if let Some(pending) = schedule.pending.as_mut() {
                pending.effective_subnet_epoch = current_epoch;
            }
        });

        assert_ok!(Network::owner_update_reputation_factors(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            SubnetReputationFactorUpdates {
                non_attestor_decrease: Some(max_value),
                ..Default::default()
            }
        ));

        let schedule = SubnetReputationFactorSchedules::<Test>::get(subnet_id);
        let pending = schedule.pending.unwrap();
        assert_eq!(schedule.current.absent_decrease, min_value);
        assert_eq!(schedule.current.included_increase, max_value);
        assert_eq!(pending.factors.non_attestor_decrease, max_value);
    });
}

#[test]
fn test_owner_update_reputation_factors_requires_at_least_one_update() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        assert_err!(
            Network::owner_update_reputation_factors(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                SubnetReputationFactorUpdates::default()
            ),
            Error::<Test>::InvalidValues
        );
    });
}
