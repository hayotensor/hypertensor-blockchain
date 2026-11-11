use super::mock::*;
use crate::tests::test_utils::*;
use crate::Event;
use crate::{
    ChurnLimit, ChurnLimitMultiplier, ConsensusValidatorNodeCountDecay,
    ConsensusValidatorNodeCountDecayUpdateInterval, ConsensusValidatorStakeWeightPower,
    ConsensusValidatorStakeWeightPowerUpdateInterval, Error, FinalSubnetEmissionWeights,
    InitialValidatorData, LastConsensusValidatorNodeCountDecayUpdate,
    LastConsensusValidatorStakeWeightPowerUpdate, LastSubnetDelegateStakeRewardsUpdate,
    MaxChurnLimit, MaxChurnLimitMultiplier, MaxConsensusValidatorStakeWeightPower,
    MaxDelegateStakePercentage, MaxMaxRegisteredNodes, MaxProbationRounds, MaxQueueEpochs,
    MaxRegisteredNodes, MaxSubnetBootnodeAccess, MaxSubnetMinStake,
    MaxSubnetNodeMinWeightDecreaseReputationThreshold, MaxSubnetNodes, MaxSubnets, MinChurnLimit,
    MinChurnLimitMultiplier, MinConsensusValidatorStakeWeightPower, MinDelegateStakePercentage,
    MinMaxRegisteredNodes, MinNodeReputationFactor, MinProbationRounds, MinQueueEpochs,
    MinSubnetMinStake, MinSubnetNodeReputation, MinSubnetNodes, NetworkMaxStakeBalance,
    NodeBurnRateAlpha, NodeRegistrationInitialValidatorIds, PeerInfo,
    PendingConsensusValidatorNodeCountDecay, PendingConsensusValidatorStakeWeightPower,
    PendingMinSubnetNodeReputation, PendingProbationRounds, PendingQueueImmunityEpochs,
    PendingSubnetDelegateStakeRewardsPercentage,
    PendingSubnetNodeMinWeightDecreaseReputationThreshold, PendingSubnetNodeQueueEpochs,
    PendingSubnetOwner, ProbationRounds, QueueImmunityEpochs, QueuedSubnetNodesData,
    SubnetBootnodeAccess, SubnetData, SubnetDelegateStakeRewardsPercentage,
    SubnetDelegateStakeRewardsUpdatePeriod, SubnetElectedValidator, SubnetMaxStakeBalance,
    SubnetMinStakeBalance, SubnetName, SubnetNode, SubnetNodeClass, SubnetNodeClassification,
    SubnetNodeElectionSlots, SubnetNodeMinWeightDecreaseReputationThreshold, SubnetNodeQueue,
    SubnetNodeQueueEpochs, SubnetNodeReputation, SubnetNodesData, SubnetOwner, SubnetRemovalReason,
    SubnetRepo, SubnetReputation, SubnetReputationFactorSchedules, SubnetReputationFactorUpdates,
    SubnetState, SubnetsData, TargetNodeRegistrationsPerEpoch, TotalElectableNodes,
    TotalSubnetElectableNodes, NETWORK_SUBNET_EMISSION_SLOT,
};
use crate::{NodeMetadata, SubnetNodeMetadata};
use codec::Decode;
use frame_support::{
    assert_err, assert_ok,
    traits::{Get, Hooks},
};
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
// do_owner_deactivate_subnet
// do_owner_update_name
// do_owner_update_repo
// do_owner_update_description
// do_owner_update_misc
// do_owner_update_churn_limit
// do_owner_update_registration_queue_epochs
// do_owner_update_probation_rounds
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
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_value
        ));

        assert_eq!(ChurnLimit::<Test>::get(subnet_id), new_value);

        assert_err!(
            Network::owner_update_churn_limit(
                RuntimeOrigin::signed(original_owner.clone()),
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
fn test_do_owner_update_probation_rounds() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let original_owner = account(1);
        SubnetOwner::<Test>::insert(subnet_id, &original_owner);

        let old_value = ProbationRounds::<Test>::get(subnet_id);
        let current_subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);
        let new_value = ProbationRounds::<Test>::get(subnet_id) + 1;

        assert_ok!(Network::owner_update_probation_rounds(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_value
        ));

        assert_eq!(ProbationRounds::<Test>::get(subnet_id), old_value);
        assert_eq!(
            Network::get_probation_rounds_for_epoch(subnet_id, current_subnet_epoch),
            old_value
        );
        assert_eq!(
            Network::get_probation_rounds_for_epoch(subnet_id, current_subnet_epoch + 1),
            new_value
        );
        assert_eq!(
            PendingProbationRounds::<Test>::get(subnet_id)
                .unwrap()
                .value,
            new_value
        );

        assert_err!(
            Network::owner_update_probation_rounds(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                MinProbationRounds::<Test>::get() - 1
            ),
            Error::<Test>::InvalidProbationRounds
        );

        assert_err!(
            Network::owner_update_probation_rounds(
                RuntimeOrigin::signed(original_owner),
                subnet_id,
                MaxProbationRounds::<Test>::get() + 1
            ),
            Error::<Test>::InvalidProbationRounds
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
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_value
        ));

        assert_eq!(
            TargetNodeRegistrationsPerEpoch::<Test>::get(subnet_id),
            new_value
        );

        assert_err!(
            Network::owner_update_target_node_registrations_per_epoch(
                RuntimeOrigin::signed(original_owner.clone()),
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
            RuntimeOrigin::signed(original_owner.clone()),
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
                0
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
fn test_owner_update_probation_rounds() {
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

        let probation_rounds = ProbationRounds::<Test>::get(subnet_id);

        let new_probation_rounds = probation_rounds + 1;
        assert_ok!(Network::owner_update_probation_rounds(
            RuntimeOrigin::signed(original_owner.clone()),
            subnet_id,
            new_probation_rounds
        ));

        assert_eq!(ProbationRounds::<Test>::get(subnet_id), probation_rounds);
        assert_eq!(
            Network::get_probation_rounds_for_epoch(subnet_id, current_subnet_epoch),
            probation_rounds
        );
        assert_eq!(
            Network::get_probation_rounds_for_epoch(subnet_id, current_subnet_epoch + 1),
            new_probation_rounds
        );

        let pending = PendingProbationRounds::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_probation_rounds);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);
        assert_eq!(pending.owner, original_owner.clone());

        assert_eq!(
            *network_events().last().unwrap(),
            Event::ProbationRoundsUpdateScheduled {
                subnet_id: subnet_id,
                owner: original_owner.clone(),
                value: new_probation_rounds,
                effective_subnet_epoch: current_subnet_epoch + 1
            }
        );

        let replacement_probation_rounds = MaxProbationRounds::<Test>::get();
        assert_ok!(Network::owner_update_probation_rounds(
            RuntimeOrigin::signed(original_owner),
            subnet_id,
            replacement_probation_rounds
        ));
        let pending = PendingProbationRounds::<Test>::get(subnet_id).unwrap();
        assert_eq!(pending.value, replacement_probation_rounds);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);
    });
}

#[test]
fn test_owner_update_probation_rounds_invalid_probation_rounds() {
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

        let epochs = MinProbationRounds::<Test>::get() - 1;

        assert_err!(
            Network::owner_update_probation_rounds(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                epochs
            ),
            Error::<Test>::InvalidProbationRounds
        );

        let epochs = MaxProbationRounds::<Test>::get() + 1;

        assert_err!(
            Network::owner_update_probation_rounds(
                RuntimeOrigin::signed(original_owner.clone()),
                subnet_id,
                epochs
            ),
            Error::<Test>::InvalidProbationRounds
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
            consensus_mechanism: Default::default(),
            state: SubnetState::Registered,
            consensus_eligible_from_subnet_epoch: None,
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
            consensus_mechanism: Default::default(),
            state: SubnetState::Registered,
            consensus_eligible_from_subnet_epoch: None,
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
fn test_owner_add_initial_validators_rejects_oversized_input_before_mutation() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let owner = account(1);
        SubnetsData::<Test>::insert(
            subnet_id,
            SubnetData {
                id: subnet_id,
                friendly_id: subnet_id,
                name: b"bounded-initial-validators".to_vec(),
                repo: b"bounded-initial-validators".to_vec(),
                description: Vec::new(),
                misc: Vec::new(),
                consensus_mechanism: Default::default(),
                state: SubnetState::Registered,
                consensus_eligible_from_subnet_epoch: None,
            },
        );
        SubnetOwner::<Test>::insert(subnet_id, &owner);
        let original = BTreeMap::from([(1, 1)]);
        NodeRegistrationInitialValidatorIds::<Test>::insert(subnet_id, &original);
        let oversized: BTreeMap<u32, u32> = (1..=NetworkMaxRegisteredNodesUpperBound::get()
            .saturating_add(1))
            .map(|validator_id| (validator_id, 1))
            .collect();

        assert_err!(
            Network::owner_add_or_update_initial_validators(
                RuntimeOrigin::signed(owner),
                subnet_id,
                oversized,
            ),
            Error::<Test>::InvalidSubnetRegistrationInitialColdkeys
        );
        assert_eq!(
            NodeRegistrationInitialValidatorIds::<Test>::get(subnet_id),
            Some(original)
        );
    });
}

#[test]
fn test_owner_initial_validator_rotation_cannot_grow_tracking_union() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let owner = account(1);
        SubnetsData::<Test>::insert(
            subnet_id,
            SubnetData {
                id: subnet_id,
                friendly_id: subnet_id,
                name: b"bounded-validator-rotation".to_vec(),
                repo: b"bounded-validator-rotation".to_vec(),
                description: Vec::new(),
                misc: Vec::new(),
                consensus_mechanism: Default::default(),
                state: SubnetState::Registered,
                consensus_eligible_from_subnet_epoch: None,
            },
        );
        SubnetOwner::<Test>::insert(subnet_id, &owner);

        let tracked: BTreeMap<u32, u32> = (1..=NetworkMaxRegisteredNodesUpperBound::get())
            .map(|validator_id| (validator_id, 1))
            .collect();
        InitialValidatorData::<Test>::insert(subnet_id, &tracked);
        let current: BTreeMap<u32, u32> = (2..=NetworkMaxRegisteredNodesUpperBound::get())
            .map(|validator_id| (validator_id, 1))
            .collect();
        NodeRegistrationInitialValidatorIds::<Test>::insert(subnet_id, &current);
        let replacement_id = NetworkMaxRegisteredNodesUpperBound::get().saturating_add(1);

        assert_err!(
            Network::owner_add_or_update_initial_validators(
                RuntimeOrigin::signed(owner),
                subnet_id,
                BTreeMap::from([(replacement_id, 1)]),
            ),
            Error::<Test>::InvalidSubnetRegistrationInitialColdkeys
        );
        assert_eq!(
            NodeRegistrationInitialValidatorIds::<Test>::get(subnet_id),
            Some(current)
        );
        assert_eq!(InitialValidatorData::<Test>::get(subnet_id), Some(tracked));
    });
}

#[test]
fn test_owner_remove_initial_validators_rejects_oversized_input() {
    new_test_ext().execute_with(|| {
        let subnet_id = 1;
        let owner = account(1);
        SubnetsData::<Test>::insert(
            subnet_id,
            SubnetData {
                id: subnet_id,
                friendly_id: subnet_id,
                name: b"bounded-validator-removal".to_vec(),
                repo: b"bounded-validator-removal".to_vec(),
                description: Vec::new(),
                misc: Vec::new(),
                consensus_mechanism: Default::default(),
                state: SubnetState::Registered,
                consensus_eligible_from_subnet_epoch: None,
            },
        );
        SubnetOwner::<Test>::insert(subnet_id, &owner);
        let original: BTreeMap<u32, u32> = (1..=NetworkMaxRegisteredNodesUpperBound::get())
            .map(|validator_id| (validator_id, 1))
            .collect();
        NodeRegistrationInitialValidatorIds::<Test>::insert(subnet_id, &original);
        let oversized =
            (1..=NetworkMaxRegisteredNodesUpperBound::get().saturating_add(1)).collect();

        assert_err!(
            Network::owner_remove_initial_validators(
                RuntimeOrigin::signed(owner),
                subnet_id,
                oversized,
            ),
            Error::<Test>::InvalidSubnetRegistrationInitialColdkeys
        );
        assert_eq!(
            NodeRegistrationInitialValidatorIds::<Test>::get(subnet_id),
            Some(original)
        );
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
            consensus_mechanism: Default::default(),
            state: SubnetState::Registered,
            consensus_eligible_from_subnet_epoch: None,
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
            consensus_mechanism: Default::default(),
            state: SubnetState::Registered,
            consensus_eligible_from_subnet_epoch: None,
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
fn test_accept_subnet_ownership_clears_bootnode_access() {
    new_test_ext().execute_with(|| {
        increase_epochs(1);
        let subnet_id = 1;
        let name: Vec<u8> = "subnet-name".into();
        SubnetsData::<Test>::insert(
            subnet_id,
            SubnetData {
                id: subnet_id,
                friendly_id: subnet_id,
                name: name.clone(),
                repo: name.clone(),
                description: name.clone(),
                misc: name.clone(),
                consensus_mechanism: Default::default(),
                state: SubnetState::Registered,
                consensus_eligible_from_subnet_epoch: None,
            },
        );
        let old_owner = account(60);
        let agent = account(70);
        let new_owner = account(80);
        SubnetOwner::<Test>::insert(subnet_id, &old_owner);
        assert_ok!(Network::owner_add_bootnode_access(
            RuntimeOrigin::signed(old_owner.clone()),
            subnet_id,
            agent.clone()
        ));
        assert_ok!(Network::transfer_subnet_ownership(
            RuntimeOrigin::signed(old_owner.clone()),
            subnet_id,
            new_owner.clone()
        ));
        assert_ok!(Network::accept_subnet_ownership(
            RuntimeOrigin::signed(new_owner.clone()),
            subnet_id
        ));

        assert!(SubnetBootnodeAccess::<Test>::get(subnet_id).is_empty());
        let add_map = BTreeMap::from([(
            peer(1),
            get_multiaddr(Some(subnet_id), Some(1), None).unwrap(),
        )]);
        assert_err!(
            Network::update_bootnodes(
                RuntimeOrigin::signed(agent.clone()),
                subnet_id,
                add_map.clone(),
                BTreeSet::new(),
            ),
            Error::<Test>::InvalidAccess
        );
        assert_ok!(Network::update_bootnodes(
            RuntimeOrigin::signed(new_owner.clone()),
            subnet_id,
            add_map,
            BTreeSet::new(),
        ));
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
            consensus_mechanism: Default::default(),
            state: SubnetState::Registered,
            consensus_eligible_from_subnet_epoch: None,
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
            consensus_mechanism: Default::default(),
            state: SubnetState::Registered,
            consensus_eligible_from_subnet_epoch: None,
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
            Network::owner_deactivate_subnet(RuntimeOrigin::signed(fake_owner.clone()), subnet_id),
            Error::<Test>::NotSubnetOwner
        );

        let new_subnet_name: Vec<u8> = "new-subnet-name".into();

        assert_err!(
            Network::owner_update_name(
                RuntimeOrigin::signed(fake_owner.clone()),
                subnet_id,
                new_subnet_name.clone()
            ),
            Error::<Test>::NotSubnetOwner
        );

        let new_subnet_repo: Vec<u8> = "new-subnet-repo".into();

        assert_err!(
            Network::owner_update_repo(
                RuntimeOrigin::signed(fake_owner.clone()),
                subnet_id,
                new_subnet_name.clone()
            ),
            Error::<Test>::NotSubnetOwner
        );

        let new_subnet_description: Vec<u8> = "new-subnet-description".into();

        assert_err!(
            Network::owner_update_description(
                RuntimeOrigin::signed(fake_owner.clone()),
                subnet_id,
                new_subnet_description
            ),
            Error::<Test>::NotSubnetOwner
        );

        let new_subnet_misc: Vec<u8> = "new-subnet-misc".into();

        assert_err!(
            Network::owner_update_misc(
                RuntimeOrigin::signed(fake_owner.clone()),
                subnet_id,
                new_subnet_misc
            ),
            Error::<Test>::NotSubnetOwner
        );

        assert_err!(
            Network::owner_update_churn_limit(
                RuntimeOrigin::signed(fake_owner.clone()),
                subnet_id,
                1
            ),
            Error::<Test>::NotSubnetOwner
        );

        assert_err!(
            Network::owner_update_registration_queue_epochs(
                RuntimeOrigin::signed(fake_owner.clone()),
                subnet_id,
                1
            ),
            Error::<Test>::NotSubnetOwner
        );

        assert_err!(
            Network::owner_update_probation_rounds(
                RuntimeOrigin::signed(fake_owner.clone()),
                subnet_id,
                1
            ),
            Error::<Test>::NotSubnetOwner
        );

        let new_coldkeys = BTreeMap::from([(1, 1)]);
        assert_err!(
            Network::owner_add_or_update_initial_validators(
                RuntimeOrigin::signed(fake_owner.clone()),
                subnet_id,
                new_coldkeys.clone()
            ),
            Error::<Test>::NotSubnetOwner
        );

        let remove_coldkeys = BTreeSet::from([1]);
        assert_err!(
            Network::owner_remove_initial_validators(
                RuntimeOrigin::signed(fake_owner.clone()),
                subnet_id,
                remove_coldkeys.clone()
            ),
            Error::<Test>::NotSubnetOwner
        );

        assert_err!(
            Network::owner_update_min_max_stake(
                RuntimeOrigin::signed(fake_owner.clone()),
                subnet_id,
                1,
                2
            ),
            Error::<Test>::NotSubnetOwner
        );
        assert_err!(
            Network::owner_update_delegate_stake_percentage(
                RuntimeOrigin::signed(fake_owner.clone()),
                subnet_id,
                1
            ),
            Error::<Test>::NotSubnetOwner
        );
        assert_err!(
            Network::owner_update_max_registered_nodes(
                RuntimeOrigin::signed(fake_owner.clone()),
                subnet_id,
                1
            ),
            Error::<Test>::NotSubnetOwner
        );

        assert_err!(
            Network::owner_add_bootnode_access(
                RuntimeOrigin::signed(fake_owner.clone()),
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
