//! Nonzero wire fixtures detect argument reordering and integer/account truncation.
use super::*;

#[test]
fn scalar_account_and_byte_arguments_preserve_the_native_call_payload() {
    macro_rules! case {
        ($domain:ident, $interface:ident, $calls:ident, $call:ident => $native:ident; $( $abi:ident => $field:ident: $value:expr ),* $(,)?) => {{
            use pallet_revive::precompiles::alloy::sol_types::SolInterface;
            let wire = abi::$interface::$call { $($abi: ($value).into()),* }.abi_encode();
            let decoded = abi::$interface::$calls::abi_decode_validate(&wire).unwrap();
            let actual = network_precompiles::$domain::to_call::<Runtime>(&decoded).unwrap().unwrap();
            let expected = n::Call::<Runtime>::$native { $($field: ($value).into()),* };
            assert_eq!(actual.encode(), expected.encode(), stringify!($native));
        }};
    }
    case!(validators, IValidators, IValidatorsCalls, updateValidatorColdkeyCall => update_validator_coldkey;
        validatorId => validator_id: 101u32,
        newColdkey => new_coldkey: [102u8; 32],
    );
    case!(validators, IValidators, IValidatorsCalls, updateValidatorHotkeyCall => update_validator_hotkey;
        validatorId => validator_id: 101u32,
        newHotkey => new_hotkey: [102u8; 32],
    );
    case!(validators, IValidators, IValidatorsCalls, updateValidatorDelegateRewardRateCall => update_validator_delegate_reward_rate;
        validatorId => validator_id: 101u32,
        newDelegateRewardRate => new_delegate_reward_rate: ((1u128 << 100) + 102),
    );
    case!(subnets, ISubnets, ISubnetsCalls, activateSubnetCall => activate_subnet;
        subnetId => subnet_id: 101u32,
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerPauseSubnetCall => owner_pause_subnet;
        subnetId => subnet_id: 101u32,
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUnpauseSubnetCall => owner_unpause_subnet;
        subnetId => subnet_id: 101u32,
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerDeactivateSubnetCall => owner_deactivate_subnet;
        subnetId => subnet_id: 101u32,
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateNameCall => owner_update_name;
        subnetId => subnet_id: 101u32,
        value => value: vec![102u8, 0, 255],
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateRepoCall => owner_update_repo;
        subnetId => subnet_id: 101u32,
        value => value: vec![102u8, 0, 255],
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateDescriptionCall => owner_update_description;
        subnetId => subnet_id: 101u32,
        value => value: vec![102u8, 0, 255],
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateMiscCall => owner_update_misc;
        subnetId => subnet_id: 101u32,
        value => value: vec![102u8, 0, 255],
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateChurnLimitCall => owner_update_churn_limit;
        subnetId => subnet_id: 101u32,
        value => value: 102u32,
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateChurnLimitMultiplierCall => owner_update_churn_limit_multiplier;
        subnetId => subnet_id: 101u32,
        value => value: 102u32,
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateRegistrationQueueEpochsCall => owner_update_registration_queue_epochs;
        subnetId => subnet_id: 101u32,
        value => value: 102u32,
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateIdleClassificationEpochsCall => owner_update_idle_classification_epochs;
        subnetId => subnet_id: 101u32,
        value => value: 102u32,
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateIncludedClassificationEpochsCall => owner_update_included_classification_epochs;
        subnetId => subnet_id: 101u32,
        value => value: 102u32,
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateSubnetNodeMinWeightDecreaseReputationThresholdCall => owner_update_subnet_node_min_weight_decrease_reputation_threshold;
        subnetId => subnet_id: 101u32,
        value => value: ((1u128 << 100) + 102),
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateMinSubnetNodeReputationCall => owner_update_min_subnet_node_reputation;
        subnetId => subnet_id: 101u32,
        value => value: ((1u128 << 100) + 102),
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerRevertEmergencyValidatorSetCall => owner_revert_emergency_validator_set;
        subnetId => subnet_id: 101u32,
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateMinMaxStakeCall => owner_update_min_max_stake;
        subnetId => subnet_id: 101u32,
        min => min: ((1u128 << 100) + 102),
        max => max: ((1u128 << 100) + 103),
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateDelegateStakePercentageCall => owner_update_delegate_stake_percentage;
        subnetId => subnet_id: 101u32,
        value => value: ((1u128 << 100) + 102),
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateMaxRegisteredNodesCall => owner_update_max_registered_nodes;
        subnetId => subnet_id: 101u32,
        value => value: 102u32,
    );
    case!(subnets, ISubnets, ISubnetsCalls, transferSubnetOwnershipCall => transfer_subnet_ownership;
        subnetId => subnet_id: 101u32,
        newOwner => new_owner: [102u8; 32],
    );
    case!(subnets, ISubnets, ISubnetsCalls, acceptSubnetOwnershipCall => accept_subnet_ownership;
        subnetId => subnet_id: 101u32,
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerAddBootnodeAccessCall => owner_add_bootnode_access;
        subnetId => subnet_id: 101u32,
        newAccount => new_account: [102u8; 32],
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerRemoveBootnodeAccessCall => owner_remove_bootnode_access;
        subnetId => subnet_id: 101u32,
        removeAccount => remove_account: [102u8; 32],
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateTargetNodeRegistrationsPerEpochCall => owner_update_target_node_registrations_per_epoch;
        subnetId => subnet_id: 101u32,
        value => value: 102u32,
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateNodeBurnRateAlphaCall => owner_update_node_burn_rate_alpha;
        subnetId => subnet_id: 101u32,
        value => value: ((1u128 << 100) + 102),
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateQueueImmunityEpochsCall => owner_update_queue_immunity_epochs;
        subnetId => subnet_id: 101u32,
        value => value: 102u32,
    );
    case!(subnet_nodes, ISubnetNodes, ISubnetNodesCalls, removeSubnetNodeCall => remove_subnet_node;
        subnetId => subnet_id: 101u32,
        subnetNodeId => subnet_node_id: 102u32,
    );
    case!(staking, IStaking, IStakingCalls, addNodeStakeCall => add_node_stake;
        subnetId => subnet_id: 101u32,
        subnetNodeId => subnet_node_id: 102u32,
        stakeToBeAdded => stake_to_be_added: ((1u128 << 100) + 103),
    );
    case!(staking, IStaking, IStakingCalls, removeNodeStakeCall => remove_node_stake;
        subnetId => subnet_id: 101u32,
        subnetNodeId => subnet_node_id: 102u32,
        stakeToBeRemoved => stake_to_be_removed: ((1u128 << 100) + 103),
    );
    case!(staking, IStaking, IStakingCalls, addSubnetDelegateStakeCall => add_subnet_delegate_stake;
        subnetId => subnet_id: 101u32,
        stakeToBeAdded => stake_to_be_added: ((1u128 << 100) + 102),
        minSharesOut => min_shares_out: ((1u128 << 100) + 103),
    );
    case!(staking, IStaking, IStakingCalls, swapFromSubnetToSubnetCall => swap_from_subnet_to_subnet;
        fromSubnetId => from_subnet_id: 101u32,
        toSubnetId => to_subnet_id: 102u32,
        delegateStakeSharesToSwap => delegate_stake_shares_to_swap: ((1u128 << 100) + 103),
        minBalanceOut => min_balance_out: ((1u128 << 100) + 104),
        minSharesOut => min_shares_out: ((1u128 << 100) + 105),
        executeBeforeBlock => execute_before_block: 106u32,
    );
    case!(staking, IStaking, IStakingCalls, transferDelegateStakeCall => transfer_delegate_stake;
        subnetId => subnet_id: 101u32,
        toAccountId => to_account_id: [102u8; 32],
        delegateStakeSharesToTransfer => delegate_stake_shares_to_transfer: ((1u128 << 100) + 103),
    );
    case!(staking, IStaking, IStakingCalls, removeDelegateStakeCall => remove_delegate_stake;
        subnetId => subnet_id: 101u32,
        sharesToBeRemoved => shares_to_be_removed: ((1u128 << 100) + 102),
        minBalanceOut => min_balance_out: ((1u128 << 100) + 103),
    );
    case!(staking, IStaking, IStakingCalls, addValidatorDelegateStakeCall => add_validator_delegate_stake;
        validatorId => validator_id: 101u32,
        delegateStakeToBeAdded => delegate_stake_to_be_added: ((1u128 << 100) + 102),
        minSharesOut => min_shares_out: ((1u128 << 100) + 103),
    );
    case!(staking, IStaking, IStakingCalls, transferValidatorDelegateStakeCall => transfer_validator_delegate_stake;
        validatorId => validator_id: 101u32,
        toAccountId => to_account_id: [102u8; 32],
        validatorDelegateStakeSharesToTransfer => validator_delegate_stake_shares_to_transfer: ((1u128 << 100) + 103),
    );
    case!(staking, IStaking, IStakingCalls, removeValidatorDelegateStakeCall => remove_validator_delegate_stake;
        validatorId => validator_id: 101u32,
        validatorDelegateStakeSharesToBeRemoved => validator_delegate_stake_shares_to_be_removed: ((1u128 << 100) + 102),
        minBalanceOut => min_balance_out: ((1u128 << 100) + 103),
    );
    case!(staking, IStaking, IStakingCalls, swapFromValidatorToValidatorCall => swap_from_validator_to_validator;
        fromValidatorId => from_validator_id: 101u32,
        toValidatorId => to_validator_id: 102u32,
        stakeToBeRemoved => stake_to_be_removed: ((1u128 << 100) + 103),
        minBalanceOut => min_balance_out: ((1u128 << 100) + 104),
        minSharesOut => min_shares_out: ((1u128 << 100) + 105),
        executeBeforeBlock => execute_before_block: 106u32,
    );
    case!(staking, IStaking, IStakingCalls, swapFromValidatorToSubnetCall => swap_from_validator_to_subnet;
        fromValidatorId => from_validator_id: 101u32,
        toSubnetId => to_subnet_id: 102u32,
        nodeDelegateStakeSharesToSwap => node_delegate_stake_shares_to_swap: ((1u128 << 100) + 103),
        minBalanceOut => min_balance_out: ((1u128 << 100) + 104),
        minSharesOut => min_shares_out: ((1u128 << 100) + 105),
        executeBeforeBlock => execute_before_block: 106u32,
    );
    case!(staking, IStaking, IStakingCalls, swapFromSubnetToValidatorCall => swap_from_subnet_to_validator;
        fromSubnetId => from_subnet_id: 101u32,
        toValidatorId => to_validator_id: 102u32,
        subnetDelegateStakeSharesToSwap => subnet_delegate_stake_shares_to_swap: ((1u128 << 100) + 103),
        minBalanceOut => min_balance_out: ((1u128 << 100) + 104),
        minSharesOut => min_shares_out: ((1u128 << 100) + 105),
        executeBeforeBlock => execute_before_block: 106u32,
    );
    case!(staking, IStaking, IStakingCalls, removeDelegateAccountBalanceCall => remove_delegate_account_balance;
        amountToRemove => amount_to_remove: ((1u128 << 100) + 101),
    );
    case!(staking, IStaking, IStakingCalls, claimUnbondingsCall => claim_unbondings;
    );
    case!(overwatch, IOverwatch, IOverwatchCalls, registerOverwatchNodeCall => register_overwatch_node;
        stakeToBeAdded => stake_to_be_added: ((1u128 << 100) + 101),
    );
    case!(overwatch, IOverwatch, IOverwatchCalls, removeOverwatchNodeCall => remove_overwatch_node;
        overwatchNodeId => overwatch_node_id: 101u32,
    );
    case!(staking, IStaking, IStakingCalls, addOverwatchNodeStakeCall => add_overwatch_node_stake;
        overwatchNodeId => overwatch_node_id: 101u32,
        stakeToBeAdded => stake_to_be_added: ((1u128 << 100) + 102),
    );
    case!(staking, IStaking, IStakingCalls, removeOverwatchNodeStakeCall => remove_overwatch_node_stake;
        overwatchNodeId => overwatch_node_id: 101u32,
        stakeToBeRemoved => stake_to_be_removed: ((1u128 << 100) + 102),
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateConsensusValidatorNodeCountDecayCall => owner_update_consensus_validator_node_count_decay;
        subnetId => subnet_id: 101u32,
        value => value: ((1u128 << 100) + 102),
    );
    case!(subnets, ISubnets, ISubnetsCalls, cancelSubnetOwnershipTransferCall => cancel_subnet_ownership_transfer;
        subnetId => subnet_id: 101u32,
    );
    case!(subnets, ISubnets, ISubnetsCalls, ownerUpdateConsensusValidatorStakeWeightPowerCall => owner_update_consensus_validator_stake_weight_power;
        subnetId => subnet_id: 101u32,
        value => value: ((1u128 << 100) + 102),
    );
}
