use crate::{abi::ISubnets::ISubnetsCalls as Calls, convert, Config};
use pallet_revive::precompiles::Error;
pub(super) fn convert<T: Config>(input: &Calls) -> Result<Option<pallet_network::Call<T>>, Error> {
    Ok(Some(match input {
        Calls::ownerUpdateName(args) => pallet_network::Call::owner_update_name { subnet_id: args.subnetId, value: convert::metadata::<T>(&args.value)? },
        Calls::ownerUpdateRepo(args) => pallet_network::Call::owner_update_repo { subnet_id: args.subnetId, value: convert::url::<T>(&args.value)? },
        Calls::ownerUpdateDescription(args) => pallet_network::Call::owner_update_description { subnet_id: args.subnetId, value: convert::metadata::<T>(&args.value)? },
        Calls::ownerUpdateMisc(args) => pallet_network::Call::owner_update_misc { subnet_id: args.subnetId, value: convert::metadata::<T>(&args.value)? },
        Calls::ownerUpdateChurnLimit(args) => pallet_network::Call::owner_update_churn_limit { subnet_id: args.subnetId, value: args.value },
        Calls::ownerUpdateChurnLimitMultiplier(args) => pallet_network::Call::owner_update_churn_limit_multiplier { subnet_id: args.subnetId, value: args.value },
        Calls::ownerUpdateRegistrationQueueEpochs(args) => pallet_network::Call::owner_update_registration_queue_epochs { subnet_id: args.subnetId, value: args.value },
        Calls::ownerUpdateIdleClassificationEpochs(args) => pallet_network::Call::owner_update_idle_classification_epochs { subnet_id: args.subnetId, value: args.value },
        Calls::ownerUpdateIncludedClassificationEpochs(args) => pallet_network::Call::owner_update_included_classification_epochs { subnet_id: args.subnetId, value: args.value },
        Calls::ownerUpdateSubnetNodeMinWeightDecreaseReputationThreshold(args) => pallet_network::Call::owner_update_subnet_node_min_weight_decrease_reputation_threshold { subnet_id: args.subnetId, value: args.value },
        Calls::ownerUpdateMinSubnetNodeReputation(args) => pallet_network::Call::owner_update_min_subnet_node_reputation { subnet_id: args.subnetId, value: args.value },
        Calls::ownerAddOrUpdateInitialValidators(args) => pallet_network::Call::owner_add_or_update_initial_validators { subnet_id: args.subnetId, validators: convert::validators::<T>(&args.validators)? },
        Calls::ownerRemoveInitialValidators(args) => pallet_network::Call::owner_remove_initial_validators { subnet_id: args.subnetId, validators: convert::validator_set::<T>(&args.validators)? },
        Calls::ownerSetEmergencyValidatorSet(args) => pallet_network::Call::owner_set_emergency_validator_set { subnet_id: args.subnetId, subnet_node_ids: convert::emergency_set::<T>(&args.subnetNodeIds)? },
        Calls::ownerRevertEmergencyValidatorSet(args) => pallet_network::Call::owner_revert_emergency_validator_set { subnet_id: args.subnetId },
        Calls::ownerUpdateMinMaxStake(args) => pallet_network::Call::owner_update_min_max_stake { subnet_id: args.subnetId, min: args.min, max: args.max },
        Calls::ownerUpdateDelegateStakePercentage(args) => pallet_network::Call::owner_update_delegate_stake_percentage { subnet_id: args.subnetId, value: args.value },
        Calls::ownerUpdateMaxRegisteredNodes(args) => pallet_network::Call::owner_update_max_registered_nodes { subnet_id: args.subnetId, value: args.value },
        Calls::ownerUpdateTargetNodeRegistrationsPerEpoch(args) => pallet_network::Call::owner_update_target_node_registrations_per_epoch { subnet_id: args.subnetId, value: args.value },
        Calls::ownerUpdateNodeBurnRateAlpha(args) => pallet_network::Call::owner_update_node_burn_rate_alpha { subnet_id: args.subnetId, value: args.value },
        Calls::ownerUpdateQueueImmunityEpochs(args) => pallet_network::Call::owner_update_queue_immunity_epochs { subnet_id: args.subnetId, value: args.value },
        Calls::ownerUpdateReputationFactors(args) => pallet_network::Call::owner_update_reputation_factors { subnet_id: args.subnetId, updates: convert::reputation(&args.updates) },
        Calls::ownerUpdateConsensusValidatorNodeCountDecay(args) => pallet_network::Call::owner_update_consensus_validator_node_count_decay { subnet_id: args.subnetId, value: args.value },
        Calls::ownerUpdateConsensusValidatorStakeWeightPower(args) => pallet_network::Call::owner_update_consensus_validator_stake_weight_power { subnet_id: args.subnetId, value: args.value },
        _ => return Ok(None),
    }))
}
