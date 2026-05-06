use core::marker::PhantomData;
use frame_support::dispatch::{GetDispatchInfo, PostDispatchInfo};
use frame_system::RawOrigin;
use pallet_evm::{AddressMapping, ExitError, PrecompileFailure, PrecompileHandle};
use pallet_network::SubnetWeightFactorsData;
use precompile_utils::{EvmResult, prelude::*};
use sp_core::U256;
use sp_runtime::traits::{Dispatchable, StaticLookup};

pub(crate) struct AdminPrecompile<R>(PhantomData<R>);

impl<R> AdminPrecompile<R>
where
    R: frame_system::Config + pallet_evm::Config + pallet_network::Config,
    R::AccountId: From<[u8; 20]> + Into<[u8; 20]>,
    <R as frame_system::Config>::RuntimeCall:
        From<pallet_network::Call<R>> + GetDispatchInfo + Dispatchable<PostInfo = PostDispatchInfo>,
    <R as pallet_evm::Config>::AddressMapping: AddressMapping<R::AccountId>,
    <<R as frame_system::Config>::Lookup as StaticLookup>::Source: From<R::AccountId>,
{
    pub const HASH_N: u64 = 2051;
}

#[precompile_utils::precompile]
impl<R> AdminPrecompile<R>
where
    R: frame_system::Config + pallet_evm::Config + pallet_network::Config,
    R::AccountId: From<[u8; 20]> + Into<[u8; 20]>,
    <R as frame_system::Config>::RuntimeCall:
        From<pallet_network::Call<R>> + GetDispatchInfo + Dispatchable<PostInfo = PostDispatchInfo>,
    <R as pallet_evm::Config>::AddressMapping: AddressMapping<R::AccountId>,
    <<R as frame_system::Config>::Lookup as StaticLookup>::Source: From<R::AccountId>,
{
    #[precompile::public("pause()")]
    fn pause(handle: &mut impl PrecompileHandle) -> EvmResult<()> {
        dispatch_call::<R>(handle, pallet_network::Call::<R>::pause {})
    }

    #[precompile::public("unpause()")]
    fn unpause(handle: &mut impl PrecompileHandle) -> EvmResult<()> {
        dispatch_call::<R>(handle, pallet_network::Call::<R>::unpause {})
    }

    #[precompile::public("collectiveRemoveSubnet(uint256)")]
    fn collective_remove_subnet(
        handle: &mut impl PrecompileHandle,
        subnet_id: U256,
    ) -> EvmResult<()> {
        let subnet_id = try_u256_to_u32(subnet_id)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::collective_remove_subnet { subnet_id },
        )
    }

    #[precompile::public("collectiveRemoveSubnetNode(uint256,uint256)")]
    fn collective_remove_subnet_node(
        handle: &mut impl PrecompileHandle,
        subnet_id: U256,
        subnet_node_id: U256,
    ) -> EvmResult<()> {
        let subnet_id = try_u256_to_u32(subnet_id)?;
        let subnet_node_id = try_u256_to_u32(subnet_node_id)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::collective_remove_subnet_node {
                subnet_id,
                subnet_node_id,
            },
        )
    }

    #[precompile::public("collectiveRemoveOverwatchNode(uint256)")]
    fn collective_remove_overwatch_node(
        handle: &mut impl PrecompileHandle,
        overwatch_node_id: U256,
    ) -> EvmResult<()> {
        let overwatch_node_id = try_u256_to_u32(overwatch_node_id)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::collective_remove_overwatch_node { overwatch_node_id },
        )
    }

    #[precompile::public("setMinSubnetDelegateStakeFactor(uint256)")]
    fn set_min_subnet_delegate_stake_factor(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_min_subnet_delegate_stake_factor { value },
        )
    }

    #[precompile::public("setSubnetOwnerPercentage(uint256)")]
    fn set_subnet_owner_percentage(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_subnet_owner_percentage { value },
        )
    }

    #[precompile::public("setMaxSubnets(uint256)")]
    fn set_max_subnets(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(handle, pallet_network::Call::<R>::set_max_subnets { value })
    }

    #[precompile::public("setMaxBootnodes(uint256)")]
    fn set_max_bootnodes(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_bootnodes { value },
        )
    }

    #[precompile::public("setMaxSubnetBootnodesAccess(uint256)")]
    fn set_max_subnet_bootnodes_access(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_subnet_bootnodes_access { value },
        )
    }

    #[precompile::public("setMaxPauseEpochs(uint256)")]
    fn set_max_pause_epochs(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_pause_epochs { value },
        )
    }

    #[precompile::public("setDelegateStakeSubnetRemovalInterval(uint256)")]
    fn set_delegate_stake_subnet_removal_interval(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_delegate_stake_subnet_removal_interval { value },
        )
    }

    #[precompile::public("setSubnetRemovalIntervals(uint256,uint256)")]
    fn set_subnet_removal_intervals(
        handle: &mut impl PrecompileHandle,
        min: U256,
        max: U256,
    ) -> EvmResult<()> {
        let min = try_u256_to_u32(min)?;
        let max = try_u256_to_u32(max)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_subnet_removal_intervals { min, max },
        )
    }

    #[precompile::public("setSubnetPauseCooldownEpochs(uint256)")]
    fn set_subnet_pause_cooldown_epochs(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_subnet_pause_cooldown_epochs { value },
        )
    }

    #[precompile::public("setMinRegistrationCost(uint256)")]
    fn set_min_registration_cost(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_min_registration_cost { value },
        )
    }

    #[precompile::public("setRegistrationCostDelayBlocks(uint256)")]
    fn set_registration_cost_delay_blocks(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_registration_cost_delay_blocks { value },
        )
    }

    #[precompile::public("setRegistrationCostAlpha(uint256)")]
    fn set_registration_cost_alpha(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_registration_cost_alpha { value },
        )
    }

    #[precompile::public("setNewRegistrationCostMultiplier(uint256)")]
    fn set_new_registration_cost_multiplier(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_new_registration_cost_multiplier { value },
        )
    }

    #[precompile::public("setMaxMinDelegateStakeMultiplier(uint256)")]
    fn set_max_min_delegate_stake_multiplier(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_min_delegate_stake_multiplier { value },
        )
    }

    #[precompile::public("setChurnLimits(uint256,uint256)")]
    fn set_churn_limits(handle: &mut impl PrecompileHandle, min: U256, max: U256) -> EvmResult<()> {
        let min = try_u256_to_u32(min)?;
        let max = try_u256_to_u32(max)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_churn_limits { min, max },
        )
    }

    #[precompile::public("setQueueEpochs(uint256,uint256)")]
    fn set_queue_epochs(handle: &mut impl PrecompileHandle, min: U256, max: U256) -> EvmResult<()> {
        let min = try_u256_to_u32(min)?;
        let max = try_u256_to_u32(max)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_queue_epochs { min, max },
        )
    }

    #[precompile::public("setMaxSwapQueueCallsPerBlock(uint256)")]
    fn set_max_swap_queue_calls_per_block(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_swap_queue_calls_per_block { value },
        )
    }

    #[precompile::public("setMinIdleClassificationEpochs(uint256)")]
    fn set_min_idle_classification_epochs(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_min_idle_classification_epochs { value },
        )
    }

    #[precompile::public("setMaxIdleClassificationEpochs(uint256)")]
    fn set_max_idle_classification_epochs(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_idle_classification_epochs { value },
        )
    }

    #[precompile::public("setSubnetActivationEnactmentEpochs(uint256)")]
    fn set_subnet_activation_enactment_epochs(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_subnet_activation_enactment_epochs { value },
        )
    }

    #[precompile::public("setIncludedClassificationEpochs(uint256,uint256)")]
    fn set_included_classification_epochs(
        handle: &mut impl PrecompileHandle,
        min: U256,
        max: U256,
    ) -> EvmResult<()> {
        let min = try_u256_to_u32(min)?;
        let max = try_u256_to_u32(max)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_included_classification_epochs { min, max },
        )
    }

    #[precompile::public("setSubnetStakes(uint256,uint256)")]
    fn set_subnet_stakes(
        handle: &mut impl PrecompileHandle,
        min: U256,
        max: U256,
    ) -> EvmResult<()> {
        let min = try_u256_to_u128(min)?;
        let max = try_u256_to_u128(max)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_subnet_stakes { min, max },
        )
    }

    #[precompile::public("setDelegateStakePercentages(uint256,uint256)")]
    fn set_delegate_stake_percentages(
        handle: &mut impl PrecompileHandle,
        min: U256,
        max: U256,
    ) -> EvmResult<()> {
        let min = try_u256_to_u128(min)?;
        let max = try_u256_to_u128(max)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_delegate_stake_percentages { min, max },
        )
    }

    #[precompile::public("setMinMaxRegisteredNodes(uint256,uint256)")]
    fn set_min_max_registered_nodes(
        handle: &mut impl PrecompileHandle,
        min: U256,
        max: U256,
    ) -> EvmResult<()> {
        let min = try_u256_to_u32(min)?;
        let max = try_u256_to_u32(max)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_min_max_registered_nodes { min, max },
        )
    }

    #[precompile::public("setMaxSubnetDelegateStakeRewardsPercentageChange(uint256)")]
    fn set_max_subnet_delegate_stake_rewards_percentage_change(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_subnet_delegate_stake_rewards_percentage_change {
                value,
            },
        )
    }

    #[precompile::public("setSubnetDelegateStakeRewardsUpdatePeriod(uint256)")]
    fn set_subnet_delegate_stake_rewards_update_period(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_subnet_delegate_stake_rewards_update_period { value },
        )
    }

    #[precompile::public("setMinAttestationPercentage(uint256)")]
    fn set_min_attestation_percentage(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_min_attestation_percentage { value },
        )
    }

    #[precompile::public("setSuperMajorityAttestationRatio(uint256)")]
    fn set_super_majority_attestation_ratio(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_super_majority_attestation_ratio { value },
        )
    }

    #[precompile::public("setBaseValidatorReward(uint256)")]
    fn set_base_validator_reward(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_base_validator_reward { value },
        )
    }

    #[precompile::public("setBaseSlashPercentage(uint256)")]
    fn set_base_slash_percentage(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_base_slash_percentage { value },
        )
    }

    #[precompile::public("setMaxSlashAmount(uint256)")]
    fn set_max_slash_amount(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_slash_amount { value },
        )
    }

    #[precompile::public("setReputationIncreaseFactor(uint256)")]
    fn set_reputation_increase_factor(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_reputation_increase_factor { value },
        )
    }

    #[precompile::public("setReputationDecreaseFactor(uint256)")]
    fn set_reputation_decrease_factor(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_reputation_decrease_factor { value },
        )
    }

    #[precompile::public("setNetworkMaxStakeBalance(uint256)")]
    fn set_network_max_stake_balance(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_network_max_stake_balance { value },
        )
    }

    #[precompile::public("setMinDelegateStakeDeposit(uint256)")]
    fn set_min_delegate_stake_deposit(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_min_delegate_stake_deposit { value },
        )
    }

    #[precompile::public("setNodeRewardRateUpdatePeriod(uint256)")]
    fn set_node_reward_rate_update_period(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_node_reward_rate_update_period { value },
        )
    }

    #[precompile::public("setMaxRewardRateDecrease(uint256)")]
    fn set_max_reward_rate_decrease(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_reward_rate_decrease { value },
        )
    }

    #[precompile::public("setSubnetDistributionPower(uint256)")]
    fn set_subnet_distribution_power(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_subnet_distribution_power { value },
        )
    }

    #[precompile::public("setDelegateStakeWeightFactor(uint256)")]
    fn set_delegate_stake_weight_factor(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_delegate_stake_weight_factor { value },
        )
    }

    #[precompile::public("setInflationSigmoidSteepness(uint256)")]
    fn set_inflation_sigmoid_steepness(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_inflation_sigmoid_steepness { value },
        )
    }

    #[precompile::public("setMaxOverwatchNodes(uint256)")]
    fn set_max_overwatch_nodes(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_overwatch_nodes { value },
        )
    }

    #[precompile::public("setOverwatchEpochLengthMultiplier(uint256)")]
    fn set_overwatch_epoch_length_multiplier(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_overwatch_epoch_length_multiplier { value },
        )
    }

    #[precompile::public("setOverwatchCommitCutoffPercent(uint256)")]
    fn set_overwatch_commit_cutoff_percent(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_overwatch_commit_cutoff_percent { value },
        )
    }

    #[precompile::public("setOverwatchMinDiversificationRatio(uint256)")]
    fn set_overwatch_min_diversification_ratio(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_overwatch_min_diversification_ratio { value },
        )
    }

    #[precompile::public("setOverwatchMinRepScore(uint256)")]
    fn set_overwatch_min_rep_score(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_overwatch_min_rep_score { value },
        )
    }

    #[precompile::public("setOverwatchMinAvgAttestationRatio(uint256)")]
    fn set_overwatch_min_avg_attestation_ratio(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_overwatch_min_avg_attestation_ratio { value },
        )
    }

    #[precompile::public("setOverwatchMinAge(uint256)")]
    fn set_overwatch_min_age(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_overwatch_min_age { value },
        )
    }

    #[precompile::public("setOverwatchMinStakeBalance(uint256)")]
    fn set_overwatch_min_stake_balance(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_overwatch_min_stake_balance { value },
        )
    }

    #[precompile::public("setMinMaxSubnetNode(uint256,uint256)")]
    fn set_min_max_subnet_node(
        handle: &mut impl PrecompileHandle,
        min: U256,
        max: U256,
    ) -> EvmResult<()> {
        let min = try_u256_to_u32(min)?;
        let max = try_u256_to_u32(max)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_min_max_subnet_node { min, max },
        )
    }

    #[precompile::public("setTxRateLimit(uint256)")]
    fn set_tx_rate_limit(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_tx_rate_limit { value },
        )
    }

    #[precompile::public("collectiveSetColdkeyOverwatchNodeEligibility(address,bool)")]
    fn collective_set_coldkey_overwatch_node_eligibility(
        handle: &mut impl PrecompileHandle,
        coldkey: Address,
        value: bool,
    ) -> EvmResult<()> {
        let coldkey = R::AddressMapping::into_account_id(coldkey.into());
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::collective_set_coldkey_overwatch_node_eligibility {
                coldkey,
                value,
            },
        )
    }

    #[precompile::public("setMinSubnetRegistrationEpochs(uint256)")]
    fn set_min_subnet_registration_epochs(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_min_subnet_registration_epochs { value },
        )
    }

    #[precompile::public("setSubnetRegistrationEpochs(uint256)")]
    fn set_subnet_registration_epochs(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_subnet_registration_epochs { value },
        )
    }

    #[precompile::public("setMinActiveNodeStakeEpochs(uint256)")]
    fn set_min_active_node_stake_epochs(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_min_active_node_stake_epochs { value },
        )
    }

    #[precompile::public("setDelegateStakeCooldownEpochs(uint256)")]
    fn set_delegate_stake_cooldown_epochs(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_delegate_stake_cooldown_epochs { value },
        )
    }

    #[precompile::public("setNodeDelegateStakeCooldownEpochs(uint256)")]
    fn set_node_delegate_stake_cooldown_epochs(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_node_delegate_stake_cooldown_epochs { value },
        )
    }

    #[precompile::public("setMinStakeCooldownEpochs(uint256)")]
    fn set_min_stake_cooldown_epochs(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_min_stake_cooldown_epochs { value },
        )
    }

    #[precompile::public("setMaxUnbondings(uint256)")]
    fn set_max_unbondings(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_unbondings { value },
        )
    }

    #[precompile::public("setSigmoidMidpoint(uint256)")]
    fn set_sigmoid_midpoint(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_sigmoid_midpoint { value },
        )
    }

    #[precompile::public("setMaximumHooksWeight(uint256)")]
    fn set_maximum_hooks_weight(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_maximum_hooks_weight { value },
        )
    }

    #[precompile::public("setBaseNodeBurnAmount(uint256)")]
    fn set_base_node_burn_amount(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_base_node_burn_amount { value },
        )
    }

    #[precompile::public("setNodeBurnRates(uint256,uint256)")]
    fn set_node_burn_rates(
        handle: &mut impl PrecompileHandle,
        min: U256,
        max: U256,
    ) -> EvmResult<()> {
        let min = try_u256_to_u128(min)?;
        let max = try_u256_to_u128(max)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_node_burn_rates { min, max },
        )
    }

    #[precompile::public("setMaxSubnetNodeMinWeightDecreaseReputationThreshold(uint256)")]
    fn set_max_subnet_node_min_weight_decrease_reputation_threshold(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_subnet_node_min_weight_decrease_reputation_threshold {
                value,
            },
        )
    }

    #[precompile::public("setValidatorRewardK(uint256)")]
    fn set_validator_reward_k(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u64(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_validator_reward_k { value },
        )
    }

    #[precompile::public("setValidatorRewardMidpoint(uint256)")]
    fn set_validator_reward_midpoint(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_validator_reward_midpoint { value },
        )
    }

    #[precompile::public("setAttestorRewardExponent(uint256)")]
    fn set_attestor_reward_exponent(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u64(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_attestor_reward_exponent { value },
        )
    }

    #[precompile::public("setAttestorMinRewardFactor(uint256)")]
    fn set_attestor_min_reward_factor(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_attestor_min_reward_factor { value },
        )
    }

    #[precompile::public("setMinMaxNodeReputation(uint256,uint256)")]
    fn set_min_max_node_reputation(
        handle: &mut impl PrecompileHandle,
        min: U256,
        max: U256,
    ) -> EvmResult<()> {
        let min = try_u256_to_u128(min)?;
        let max = try_u256_to_u128(max)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_min_max_node_reputation { min, max },
        )
    }

    #[precompile::public("setMinMaxNodeReputationFactor(uint256,uint256)")]
    fn set_min_max_node_reputation_factor(
        handle: &mut impl PrecompileHandle,
        min: U256,
        max: U256,
    ) -> EvmResult<()> {
        let min = try_u256_to_u128(min)?;
        let max = try_u256_to_u128(max)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_min_max_node_reputation_factor { min, max },
        )
    }

    #[precompile::public("setMinSubnetReputation(uint256)")]
    fn set_min_subnet_reputation(handle: &mut impl PrecompileHandle, value: U256) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_min_subnet_reputation { value },
        )
    }

    #[precompile::public("setNotInConsensusSubnetReputationFactor(uint256)")]
    fn set_not_in_consensus_subnet_reputation_factor(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_not_in_consensus_subnet_reputation_factor { value },
        )
    }

    #[precompile::public("setMaxPauseEpochsSubnetReputationFactor(uint256)")]
    fn set_max_pause_epochs_subnet_reputation_factor(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_pause_epochs_subnet_reputation_factor { value },
        )
    }

    #[precompile::public("setLessThanMinNodesSubnetReputationFactor(uint256)")]
    fn set_less_than_min_nodes_subnet_reputation_factor(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_less_than_min_nodes_subnet_reputation_factor { value },
        )
    }

    #[precompile::public("setValidatorProposalAbsentSubnetReputationFactor(uint256)")]
    fn set_validator_proposal_absent_subnet_reputation_factor(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_validator_proposal_absent_subnet_reputation_factor {
                value,
            },
        )
    }

    #[precompile::public("setInConsensusSubnetReputationFactor(uint256)")]
    fn set_in_consensus_subnet_reputation_factor(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_in_consensus_subnet_reputation_factor { value },
        )
    }

    #[precompile::public("setOverwatchWeightFactor(uint256)")]
    fn set_overwatch_weight_factor(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_overwatch_weight_factor { value },
        )
    }

    #[precompile::public("setMaxEmergencyValidatorEpochsMultiplier(uint256)")]
    fn set_max_emergency_validator_epochs_multiplier(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_emergency_validator_epochs_multiplier { value },
        )
    }

    #[precompile::public("setMaxEmergencySubnetNodes(uint256)")]
    fn set_max_emergency_subnet_nodes(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u32(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_max_emergency_subnet_nodes { value },
        )
    }

    #[precompile::public("setOverwatchStakeWeightFactor(uint256)")]
    fn set_overwatch_stake_weight_factor(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_overwatch_stake_weight_factor { value },
        )
    }

    #[precompile::public("setSubnetWeightFactors(uint256,uint256,uint256)")]
    fn set_subnet_weight_factors(
        handle: &mut impl PrecompileHandle,
        delegate_stake: U256,
        node_count: U256,
        net_flow: U256,
    ) -> EvmResult<()> {
        let value = SubnetWeightFactorsData {
            delegate_stake: try_u256_to_u128(delegate_stake)?,
            node_count: try_u256_to_u128(node_count)?,
            net_flow: try_u256_to_u128(net_flow)?,
        };
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_subnet_weight_factors { value },
        )
    }

    #[precompile::public("setChurnLimitMultipliers(uint256,uint256)")]
    fn set_churn_limit_multipliers(
        handle: &mut impl PrecompileHandle,
        min: U256,
        max: U256,
    ) -> EvmResult<()> {
        let min = try_u256_to_u32(min)?;
        let max = try_u256_to_u32(max)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_churn_limit_multipliers { min, max },
        )
    }

    #[precompile::public("setDefaultOverwatchSubnetWeight(uint256)")]
    fn set_default_overwatch_subnet_weight(
        handle: &mut impl PrecompileHandle,
        value: U256,
    ) -> EvmResult<()> {
        let value = try_u256_to_u128(value)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_default_overwatch_subnet_weight { value },
        )
    }

    #[precompile::public("setOverwatchValidatorWhitelist(uint256,bool)")]
    fn set_overwatch_validator_whitelist(
        handle: &mut impl PrecompileHandle,
        validator_id: U256,
        value: bool,
    ) -> EvmResult<()> {
        let validator_id = try_u256_to_u32(validator_id)?;
        dispatch_call::<R>(
            handle,
            pallet_network::Call::<R>::set_overwatch_validator_whitelist {
                validator_id,
                value,
            },
        )
    }
}

fn dispatch_call<R>(
    handle: &mut impl PrecompileHandle,
    call: pallet_network::Call<R>,
) -> EvmResult<()>
where
    R: frame_system::Config + pallet_evm::Config + pallet_network::Config,
    R::AccountId: From<[u8; 20]> + Into<[u8; 20]>,
    <R as frame_system::Config>::RuntimeCall:
        From<pallet_network::Call<R>> + GetDispatchInfo + Dispatchable<PostInfo = PostDispatchInfo>,
    <R as pallet_evm::Config>::AddressMapping: AddressMapping<R::AccountId>,
{
    let origin = R::AddressMapping::into_account_id(handle.context().caller);
    RuntimeHelper::<R>::try_dispatch(handle, RawOrigin::Signed(origin).into(), call, 0)?;
    Ok(())
}

fn try_u256_to_u32(value: U256) -> Result<u32, PrecompileFailure> {
    value.try_into().map_err(|_| PrecompileFailure::Error {
        exit_status: ExitError::Other("u32 out of bounds".into()),
    })
}

fn try_u256_to_u64(value: U256) -> Result<u64, PrecompileFailure> {
    value.try_into().map_err(|_| PrecompileFailure::Error {
        exit_status: ExitError::Other("u64 out of bounds".into()),
    })
}

fn try_u256_to_u128(value: U256) -> Result<u128, PrecompileFailure> {
    value.try_into().map_err(|_| PrecompileFailure::Error {
        exit_status: ExitError::Other("u128 out of bounds".into()),
    })
}
