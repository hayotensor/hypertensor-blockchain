use core::marker::PhantomData;
use frame_support::dispatch::{GetDispatchInfo, PostDispatchInfo};
use frame_system::RawOrigin;
use pallet_evm::{AddressMapping, ExitError, PrecompileFailure, PrecompileHandle};
use pallet_network::QueuedSwapCall;
use precompile_utils::{EvmResult, prelude::*};
use sp_core::U256;
use sp_runtime::traits::{Dispatchable, StaticLookup};

pub(crate) struct StakingPrecompile<R>(PhantomData<R>);

impl<R> StakingPrecompile<R>
where
    R: frame_system::Config + pallet_evm::Config + pallet_network::Config,
    R::AccountId: From<[u8; 20]> + Into<[u8; 20]>,
    <R as frame_system::Config>::RuntimeCall:
        From<pallet_network::Call<R>> + GetDispatchInfo + Dispatchable<PostInfo = PostDispatchInfo>,
    <R as pallet_evm::Config>::AddressMapping: AddressMapping<R::AccountId>,
    <<R as frame_system::Config>::Lookup as StaticLookup>::Source: From<R::AccountId>,
{
    pub const HASH_N: u64 = 2048;
}

#[precompile_utils::precompile]
impl<R> StakingPrecompile<R>
where
    R: frame_system::Config + pallet_evm::Config + pallet_network::Config,
    R::AccountId: From<[u8; 20]> + Into<[u8; 20]>,
    <R as frame_system::Config>::RuntimeCall:
        From<pallet_network::Call<R>> + GetDispatchInfo + Dispatchable<PostInfo = PostDispatchInfo>,
    <R as pallet_evm::Config>::AddressMapping: AddressMapping<R::AccountId>,
    <<R as frame_system::Config>::Lookup as StaticLookup>::Source: From<R::AccountId>,
{
    #[precompile::public("addNodeStake(uint256,uint256,uint256)")]
    fn add_node_stake(
        handle: &mut impl PrecompileHandle,
        subnet_id: U256,
        subnet_node_id: U256,
        stake_to_be_added: U256,
    ) -> EvmResult<()> {
        let stake_to_be_added = try_u256_to_u128(stake_to_be_added)?;
        let subnet_id = try_u256_to_u32(subnet_id)?;
        let subnet_node_id = try_u256_to_u32(subnet_node_id)?;

        let origin = R::AddressMapping::into_account_id(handle.context().caller);
        let call = pallet_network::Call::<R>::add_node_stake {
            subnet_id,
            subnet_node_id,
            stake_to_be_added,
        };

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public("removeNodeStake(uint256,uint256,uint256)")]
    fn remove_node_stake(
        handle: &mut impl PrecompileHandle,
        subnet_id: U256,
        subnet_node_id: U256,
        stake_to_be_removed: U256,
    ) -> EvmResult<()> {
        let stake_to_be_removed = try_u256_to_u128(stake_to_be_removed)?;
        let subnet_id = try_u256_to_u32(subnet_id)?;
        let subnet_node_id = try_u256_to_u32(subnet_node_id)?;

        let origin = R::AddressMapping::into_account_id(handle.context().caller);
        let call = pallet_network::Call::<R>::remove_node_stake {
            subnet_id,
            subnet_node_id,
            stake_to_be_removed,
        };

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public("claimUnbondings()")]
    fn claim_unbondings(handle: &mut impl PrecompileHandle) -> EvmResult<()> {
        let origin = R::AddressMapping::into_account_id(handle.context().caller);
        let call = pallet_network::Call::<R>::claim_unbondings {};

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public("addToDelegateStake(uint256,uint256,uint256)")]
    fn add_subnet_delegate_stake(
        handle: &mut impl PrecompileHandle,
        subnet_id: U256,
        stake_to_be_added: U256,
        min_shares_out: U256,
    ) -> EvmResult<()> {
        let origin = R::AddressMapping::into_account_id(handle.context().caller);

        let subnet_id = try_u256_to_u32(subnet_id)?;
        let stake_to_be_added = try_u256_to_u128(stake_to_be_added)?;
        let min_shares_out = try_u256_to_u128(min_shares_out)?;

        let call = pallet_network::Call::<R>::add_subnet_delegate_stake {
            subnet_id,
            stake_to_be_added,
            min_shares_out,
        };

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public("swapDelegateStake(uint256,uint256,uint256,uint256,uint256,uint256)")]
    fn swap_from_subnet_to_subnet(
        handle: &mut impl PrecompileHandle,
        from_subnet_id: U256,
        to_subnet_id: U256,
        delegate_stake_shares_to_swap: U256,
        min_balance_out: U256,
        min_shares_out: U256,
        execute_before_block: U256,
    ) -> EvmResult<()> {
        let delegate_stake_shares_to_swap = try_u256_to_u128(delegate_stake_shares_to_swap)?;
        let min_balance_out = try_u256_to_u128(min_balance_out)?;
        let min_shares_out = try_u256_to_u128(min_shares_out)?;
        let execute_before_block = try_u256_to_u32(execute_before_block)?;
        let from_subnet_id = try_u256_to_u32(from_subnet_id)?;
        let to_subnet_id = try_u256_to_u32(to_subnet_id)?;

        let origin = R::AddressMapping::into_account_id(handle.context().caller);
        let call = pallet_network::Call::<R>::swap_from_subnet_to_subnet {
            from_subnet_id,
            to_subnet_id,
            delegate_stake_shares_to_swap,
            min_balance_out,
            min_shares_out,
            execute_before_block,
        };

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public("transferDelegateStake(uint256,address,uint256)")]
    fn transfer_delegate_stake(
        handle: &mut impl PrecompileHandle,
        subnet_id: U256,
        to_account_id: Address,
        delegate_stake_shares_to_transfer: U256,
    ) -> EvmResult<()> {
        let delegate_stake_shares_to_transfer =
            try_u256_to_u128(delegate_stake_shares_to_transfer)?;
        let subnet_id = try_u256_to_u32(subnet_id)?;
        let to_account_id = R::AddressMapping::into_account_id(to_account_id.into());

        let origin = R::AddressMapping::into_account_id(handle.context().caller);
        let call = pallet_network::Call::<R>::transfer_delegate_stake {
            subnet_id,
            to_account_id,
            delegate_stake_shares_to_transfer,
        };

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public("removeDelegateStake(uint256,uint256,uint256)")]
    fn remove_delegate_stake(
        handle: &mut impl PrecompileHandle,
        subnet_id: U256,
        shares_to_be_removed: U256,
        min_balance_out: U256,
    ) -> EvmResult<()> {
        let shares_to_be_removed = try_u256_to_u128(shares_to_be_removed)?;
        let min_balance_out = try_u256_to_u128(min_balance_out)?;
        let subnet_id = try_u256_to_u32(subnet_id)?;

        let origin = R::AddressMapping::into_account_id(handle.context().caller);
        let call = pallet_network::Call::<R>::remove_delegate_stake {
            subnet_id,
            shares_to_be_removed,
            min_balance_out,
        };

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public("addValidatorDelegateStake(uint256,uint256,uint256)")]
    fn add_validator_delegate_stake(
        handle: &mut impl PrecompileHandle,
        validator_id: U256,
        delegate_stake_to_be_added: U256,
        min_shares_out: U256,
    ) -> EvmResult<()> {
        let delegate_stake_to_be_added = try_u256_to_u128(delegate_stake_to_be_added)?;
        let min_shares_out = try_u256_to_u128(min_shares_out)?;
        let validator_id = try_u256_to_u32(validator_id)?;

        let origin = R::AddressMapping::into_account_id(handle.context().caller);
        let call = pallet_network::Call::<R>::add_validator_delegate_stake {
            validator_id,
            delegate_stake_to_be_added,
            min_shares_out,
        };

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public("swapNodeDelegateStake(uint256,uint256,uint256,uint256,uint256,uint256)")]
    fn swap_from_validator_to_validator(
        handle: &mut impl PrecompileHandle,
        from_validator_id: U256,
        to_validator_id: U256,
        stake_to_be_removed: U256,
        min_balance_out: U256,
        min_shares_out: U256,
        execute_before_block: U256,
    ) -> EvmResult<()> {
        let stake_to_be_removed = try_u256_to_u128(stake_to_be_removed)?;
        let min_balance_out = try_u256_to_u128(min_balance_out)?;
        let min_shares_out = try_u256_to_u128(min_shares_out)?;
        let execute_before_block = try_u256_to_u32(execute_before_block)?;
        let from_validator_id = try_u256_to_u32(from_validator_id)?;
        let to_validator_id = try_u256_to_u32(to_validator_id)?;

        let origin = R::AddressMapping::into_account_id(handle.context().caller);
        let call = pallet_network::Call::<R>::swap_from_validator_to_validator {
            from_validator_id,
            to_validator_id,
            stake_to_be_removed,
            min_balance_out,
            min_shares_out,
            execute_before_block,
        };

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public("transferValidatorDelegateStake(uint256,address,uint256)")]
    fn transfer_validator_delegate_stake(
        handle: &mut impl PrecompileHandle,
        validator_id: U256,
        to_account_id: Address,
        validator_delegate_stake_shares_to_transfer: U256,
    ) -> EvmResult<()> {
        let validator_delegate_stake_shares_to_transfer =
            try_u256_to_u128(validator_delegate_stake_shares_to_transfer)?;
        let validator_id = try_u256_to_u32(validator_id)?;
        let to_account_id = R::AddressMapping::into_account_id(to_account_id.into());

        let origin = R::AddressMapping::into_account_id(handle.context().caller);
        let call = pallet_network::Call::<R>::transfer_validator_delegate_stake {
            validator_id,
            to_account_id,
            validator_delegate_stake_shares_to_transfer,
        };

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public("removeValidatorDelegateStake(uint256,uint256,uint256)")]
    fn remove_validator_delegate_stake(
        handle: &mut impl PrecompileHandle,
        validator_id: U256,
        validator_delegate_stake_shares_to_be_removed: U256,
        min_balance_out: U256,
    ) -> EvmResult<()> {
        let validator_delegate_stake_shares_to_be_removed =
            try_u256_to_u128(validator_delegate_stake_shares_to_be_removed)?;
        let min_balance_out = try_u256_to_u128(min_balance_out)?;
        let validator_id = try_u256_to_u32(validator_id)?;

        let origin = R::AddressMapping::into_account_id(handle.context().caller);
        let call = pallet_network::Call::<R>::remove_validator_delegate_stake {
            validator_id,
            validator_delegate_stake_shares_to_be_removed,
            min_balance_out,
        };

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public(
        "transferFromValidatorToSubnet(uint256,uint256,uint256,uint256,uint256,uint256)"
    )]
    fn swap_from_validator_to_subnet(
        handle: &mut impl PrecompileHandle,
        from_validator_id: U256,
        to_subnet_id: U256,
        node_delegate_stake_shares_to_swap: U256,
        min_balance_out: U256,
        min_shares_out: U256,
        execute_before_block: U256,
    ) -> EvmResult<()> {
        let node_delegate_stake_shares_to_swap =
            try_u256_to_u128(node_delegate_stake_shares_to_swap)?;
        let min_balance_out = try_u256_to_u128(min_balance_out)?;
        let min_shares_out = try_u256_to_u128(min_shares_out)?;
        let execute_before_block = try_u256_to_u32(execute_before_block)?;
        let from_validator_id = try_u256_to_u32(from_validator_id)?;
        let to_subnet_id = try_u256_to_u32(to_subnet_id)?;

        let origin = R::AddressMapping::into_account_id(handle.context().caller);
        let call = pallet_network::Call::<R>::swap_from_validator_to_subnet {
            from_validator_id,
            to_subnet_id,
            node_delegate_stake_shares_to_swap,
            min_balance_out,
            min_shares_out,
            execute_before_block,
        };

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public(
        "transferFromSubnetToValidator(uint256,uint256,uint256,uint256,uint256,uint256)"
    )]
    fn swap_from_subnet_to_validator(
        handle: &mut impl PrecompileHandle,
        from_subnet_id: U256,
        to_validator_id: U256,
        subnet_delegate_stake_shares_to_swap: U256,
        min_balance_out: U256,
        min_shares_out: U256,
        execute_before_block: U256,
    ) -> EvmResult<()> {
        let subnet_delegate_stake_shares_to_swap =
            try_u256_to_u128(subnet_delegate_stake_shares_to_swap)?;
        let min_balance_out = try_u256_to_u128(min_balance_out)?;
        let min_shares_out = try_u256_to_u128(min_shares_out)?;
        let execute_before_block = try_u256_to_u32(execute_before_block)?;
        let from_subnet_id = try_u256_to_u32(from_subnet_id)?;
        let to_validator_id = try_u256_to_u32(to_validator_id)?;

        let origin = R::AddressMapping::into_account_id(handle.context().caller);
        let call = pallet_network::Call::<R>::swap_from_subnet_to_validator {
            from_subnet_id,
            to_validator_id,
            subnet_delegate_stake_shares_to_swap,
            min_balance_out,
            min_shares_out,
            execute_before_block,
        };

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public("updateSwapQueue(uint256,uint256,uint256,uint256,uint256,uint256)")]
    fn update_swap_queue(
        handle: &mut impl PrecompileHandle,
        id: U256,
        call_type: U256,
        to_validator_id: U256,
        to_subnet_id: U256,
        min_shares_out: U256,
        execute_before_block: U256,
    ) -> EvmResult<()> {
        let id = try_u256_to_u32(id)?;
        let call_type = try_u256_to_u32(call_type)?;
        let to_validator_id = try_u256_to_u32(to_validator_id)?;
        let to_subnet_id = try_u256_to_u32(to_subnet_id)?;
        let min_shares_out = try_u256_to_u128(min_shares_out)?;
        let execute_before_block = try_u256_to_u32(execute_before_block)?;
        let origin = R::AddressMapping::into_account_id(handle.context().caller);

        let new_call = match call_type {
            0 => QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id: origin.clone(),
                to_subnet_id,
                balance: 0,
                min_shares_out,
                execute_before_block,
            },
            1 => QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id: origin.clone(),
                to_validator_id,
                balance: 0,
                min_shares_out,
                execute_before_block,
            },
            _ => {
                return Err(revert(
                    "Invalid call type. Must be 0 (SwapToSubnetDelegateStake) or 1 (SwapToValidatorDelegateStake)",
                ));
            }
        };

        let call = pallet_network::Call::<R>::update_swap_queue { id, new_call };

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public("removeDelegateAccountBalance(uint256)")]
    fn remove_delegate_account_balance(
        handle: &mut impl PrecompileHandle,
        amount_to_remove: U256,
    ) -> EvmResult<()> {
        let amount_to_remove = try_u256_to_u128(amount_to_remove)?;

        let origin = R::AddressMapping::into_account_id(handle.context().caller);
        let call = pallet_network::Call::<R>::remove_delegate_account_balance { amount_to_remove };

        RuntimeHelper::<R>::try_dispatch(
            handle,
            RawOrigin::Signed(origin.clone()).into(),
            call,
            0,
        )?;

        Ok(())
    }

    #[precompile::public("getQueuedSwapCall(uint256)")]
    #[precompile::view]
    fn get_queued_swap_call(
        handle: &mut impl PrecompileHandle,
        queue_id: U256,
    ) -> EvmResult<(u32, Address, u8, u32, u32, u128, u128, u32, u32, u32)> {
        // Returns: (id, account_id, call_type, to_validator_id, to_subnet_id, balance,
        // min_shares_out, execute_before_block, queued_at_block, execute_after_blocks)
        let queue_id = try_u256_to_u32(queue_id)?;
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;

        let queued_item = pallet_network::SwapCallQueue::<R>::get(queue_id);

        match queued_item {
            Some(item) => {
                let (
                    call_type,
                    account_id,
                    to_validator_id,
                    to_subnet_id,
                    balance,
                    min_shares_out,
                    execute_before_block,
                ) = match &item.call {
                    QueuedSwapCall::SwapToSubnetDelegateStake {
                        account_id,
                        to_subnet_id,
                        balance,
                        min_shares_out,
                        execute_before_block,
                    } => (
                        0u8,
                        account_id,
                        0u32,
                        *to_subnet_id,
                        *balance,
                        *min_shares_out,
                        *execute_before_block,
                    ),
                    QueuedSwapCall::SwapToValidatorDelegateStake {
                        account_id,
                        to_validator_id,
                        balance,
                        min_shares_out,
                        execute_before_block,
                    } => (
                        1u8,
                        account_id,
                        *to_validator_id,
                        0u32,
                        *balance,
                        *min_shares_out,
                        *execute_before_block,
                    ),
                };

                let account_address = Address(sp_core::H160::from((account_id.clone()).into()));

                Ok((
                    item.id,                   // id
                    account_address,           // account_id (as Address)
                    call_type,                 // type (0=subnet, 1=validator)
                    to_validator_id,           // to_validator_id (0 is swapping to subnet)
                    to_subnet_id,              // to_subnet_id (0 is swapping to validator)
                    balance,                   // balance
                    min_shares_out,            // caller's destination share floor
                    execute_before_block,      // inclusive execution deadline
                    item.queued_at_block,      // queued_at_block
                    item.execute_after_blocks, // execute_after_blocks
                ))
            }
            None => Err(revert("Queue item not found")),
        }
    }

    #[precompile::public("totalSubnetStake(uint256)")]
    #[precompile::view]
    fn total_subnet_stake(handle: &mut impl PrecompileHandle, subnet_id: U256) -> EvmResult<u128> {
        let subnet_id = try_u256_to_u32(subnet_id)?;

        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_subnet_stake: u128 = pallet_network::TotalSubnetStake::<R>::get(subnet_id);

        Ok(total_subnet_stake)
    }

    #[precompile::public("nodeSubnetStake(uint256,uint256)")]
    #[precompile::view]
    fn node_subnet_stake(
        handle: &mut impl PrecompileHandle,
        subnet_id: U256,
        subnet_node_id: U256,
    ) -> EvmResult<u128> {
        let subnet_id = try_u256_to_u32(subnet_id)?;
        let subnet_node_id = try_u256_to_u32(subnet_node_id)?;

        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let account_subnet_stake: u128 =
            pallet_network::NodeSubnetStake::<R>::get(subnet_node_id, subnet_id);

        Ok(account_subnet_stake)
    }

    #[precompile::public("totalSubnetDelegateStakeBalance(uint256)")]
    #[precompile::view]
    fn total_subnet_delegate_stake_balance(
        handle: &mut impl PrecompileHandle,
        subnet_id: U256,
    ) -> EvmResult<u128> {
        let subnet_id = try_u256_to_u32(subnet_id)?;
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_assets = pallet_network::TotalSubnetDelegateStakeBalance::<R>::get(subnet_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_shares = pallet_network::TotalSubnetDelegateStakeShares::<R>::get(subnet_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let circulating_shares =
            pallet_network::TotalSubnetDelegateStakeCirculatingShares::<R>::get(subnet_id);

        pallet_network::Pallet::<R>::validate_delegate_pool_accounting(
            total_shares,
            total_assets,
            circulating_shares,
        )
        .map_err(|_| revert("Invalid subnet delegate stake pool"))?;

        Ok(total_assets)
    }

    #[precompile::public("totalSubnetDelegateStakeShares(uint256)")]
    #[precompile::view]
    fn total_subnet_delegate_stake_shares(
        handle: &mut impl PrecompileHandle,
        subnet_id: U256,
    ) -> EvmResult<u128> {
        let subnet_id = try_u256_to_u32(subnet_id)?;
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_shares = pallet_network::TotalSubnetDelegateStakeShares::<R>::get(subnet_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_assets = pallet_network::TotalSubnetDelegateStakeBalance::<R>::get(subnet_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let circulating_shares =
            pallet_network::TotalSubnetDelegateStakeCirculatingShares::<R>::get(subnet_id);

        pallet_network::Pallet::<R>::validate_delegate_pool_accounting(
            total_shares,
            total_assets,
            circulating_shares,
        )
        .map_err(|_| revert("Invalid subnet delegate stake pool"))?;

        Ok(total_shares)
    }

    #[precompile::public("totalValidatorDelegateStakeBalance(uint256)")]
    #[precompile::view]
    fn total_validator_delegate_stake_balance(
        handle: &mut impl PrecompileHandle,
        validator_id: U256,
    ) -> EvmResult<u128> {
        let validator_id = try_u256_to_u32(validator_id)?;
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_assets = pallet_network::ValidatorDelegateStakeBalance::<R>::get(validator_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_shares = pallet_network::ValidatorDelegateStakeShares::<R>::get(validator_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let circulating_shares =
            pallet_network::ValidatorDelegateStakeCirculatingShares::<R>::get(validator_id);

        pallet_network::Pallet::<R>::validate_delegate_pool_accounting(
            total_shares,
            total_assets,
            circulating_shares,
        )
        .map_err(|_| revert("Invalid validator delegate stake pool"))?;

        Ok(total_assets)
    }

    #[precompile::public("totalValidatorDelegateStakeShares(uint256)")]
    #[precompile::view]
    fn total_validator_delegate_stake_shares(
        handle: &mut impl PrecompileHandle,
        validator_id: U256,
    ) -> EvmResult<u128> {
        let validator_id = try_u256_to_u32(validator_id)?;
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_shares = pallet_network::ValidatorDelegateStakeShares::<R>::get(validator_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_assets = pallet_network::ValidatorDelegateStakeBalance::<R>::get(validator_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let circulating_shares =
            pallet_network::ValidatorDelegateStakeCirculatingShares::<R>::get(validator_id);

        pallet_network::Pallet::<R>::validate_delegate_pool_accounting(
            total_shares,
            total_assets,
            circulating_shares,
        )
        .map_err(|_| revert("Invalid validator delegate stake pool"))?;

        Ok(total_shares)
    }

    /// Preview the user-owned shares minted by a subnet delegate-pool deposit.
    ///
    /// On the first deposit this excludes the permanently locked minimum-liquidity shares, so
    /// the returned value can be passed directly as the caller's `minSharesOut` expectation.
    #[precompile::public("previewSubnetDelegateStakeDeposit(uint256,uint256)")]
    #[precompile::view]
    fn preview_subnet_delegate_stake_deposit(
        handle: &mut impl PrecompileHandle,
        subnet_id: U256,
        assets: U256,
    ) -> EvmResult<u128> {
        let subnet_id = try_u256_to_u32(subnet_id)?;
        let assets = try_u256_to_u128(assets)?;

        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        if assets < pallet_network::MinDelegateStakeDeposit::<R>::get() {
            return Err(revert("Subnet delegate stake deposit is below the minimum"));
        }
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        if !pallet_network::SubnetsData::<R>::contains_key(subnet_id) {
            return Err(revert("Subnet delegate stake pool does not exist"));
        }
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_shares = pallet_network::TotalSubnetDelegateStakeShares::<R>::get(subnet_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_assets = pallet_network::TotalSubnetDelegateStakeBalance::<R>::get(subnet_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let circulating_shares =
            pallet_network::TotalSubnetDelegateStakeCirculatingShares::<R>::get(subnet_id);

        pallet_network::Pallet::<R>::validate_delegate_pool_accounting(
            total_shares,
            total_assets,
            circulating_shares,
        )
        .map_err(|_| revert("Invalid subnet delegate stake pool"))?;

        let (user_shares, _) = pallet_network::Pallet::<R>::preview_delegate_pool_deposit(
            assets,
            total_shares,
            total_assets,
            1,
        )
        .map_err(|_| revert("Subnet delegate stake deposit cannot be previewed"))?;

        Ok(user_shares)
    }

    /// Preview the assets returned by redeeming subnet delegate-pool shares.
    #[precompile::public("previewSubnetDelegateStakeRedeem(uint256,uint256)")]
    #[precompile::view]
    fn preview_subnet_delegate_stake_redeem(
        handle: &mut impl PrecompileHandle,
        subnet_id: U256,
        shares: U256,
    ) -> EvmResult<u128> {
        let subnet_id = try_u256_to_u32(subnet_id)?;
        let shares = try_u256_to_u128(shares)?;

        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_shares = pallet_network::TotalSubnetDelegateStakeShares::<R>::get(subnet_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_assets = pallet_network::TotalSubnetDelegateStakeBalance::<R>::get(subnet_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let circulating_shares =
            pallet_network::TotalSubnetDelegateStakeCirculatingShares::<R>::get(subnet_id);

        // A removed subnet can retain a pool while delegators redeem its remaining principal.
        // Reject only an identifier that has neither a live subnet nor initialized pool state.
        if total_shares == 0 && total_assets == 0 {
            handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
            if !pallet_network::SubnetsData::<R>::contains_key(subnet_id) {
                return Err(revert("Subnet delegate stake pool does not exist"));
            }
        }

        pallet_network::Pallet::<R>::validate_delegate_pool_accounting(
            total_shares,
            total_assets,
            circulating_shares,
        )
        .map_err(|_| revert("Invalid subnet delegate stake pool"))?;
        if shares > circulating_shares {
            return Err(revert("Subnet delegate stake shares exceed circulation"));
        }
        if shares == 0 {
            return Ok(0);
        }
        let (assets, _) = pallet_network::Pallet::<R>::preview_delegate_pool_redemption(
            shares,
            total_shares,
            total_assets,
            1,
        )
        .map_err(|_| revert("Subnet delegate stake redemption cannot be previewed"))?;

        Ok(assets)
    }

    /// Preview the user-owned shares minted by a validator delegate-pool deposit.
    ///
    /// On the first deposit this excludes the permanently locked minimum-liquidity shares, so
    /// the returned value can be passed directly as the caller's `minSharesOut` expectation.
    #[precompile::public("previewValidatorDelegateStakeDeposit(uint256,uint256)")]
    #[precompile::view]
    fn preview_validator_delegate_stake_deposit(
        handle: &mut impl PrecompileHandle,
        validator_id: U256,
        assets: U256,
    ) -> EvmResult<u128> {
        let validator_id = try_u256_to_u32(validator_id)?;
        let assets = try_u256_to_u128(assets)?;

        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        if assets < pallet_network::MinDelegateStakeDeposit::<R>::get() {
            return Err(revert(
                "Validator delegate stake deposit is below the minimum",
            ));
        }
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        if !pallet_network::ValidatorsData::<R>::contains_key(validator_id) {
            return Err(revert("Validator delegate stake pool does not exist"));
        }
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_shares = pallet_network::ValidatorDelegateStakeShares::<R>::get(validator_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_assets = pallet_network::ValidatorDelegateStakeBalance::<R>::get(validator_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let circulating_shares =
            pallet_network::ValidatorDelegateStakeCirculatingShares::<R>::get(validator_id);

        pallet_network::Pallet::<R>::validate_delegate_pool_accounting(
            total_shares,
            total_assets,
            circulating_shares,
        )
        .map_err(|_| revert("Invalid validator delegate stake pool"))?;

        let (user_shares, _) = pallet_network::Pallet::<R>::preview_delegate_pool_deposit(
            assets,
            total_shares,
            total_assets,
            1,
        )
        .map_err(|_| revert("Validator delegate stake deposit cannot be previewed"))?;

        Ok(user_shares)
    }

    /// Preview the assets returned by redeeming validator delegate-pool shares.
    #[precompile::public("previewValidatorDelegateStakeRedeem(uint256,uint256)")]
    #[precompile::view]
    fn preview_validator_delegate_stake_redeem(
        handle: &mut impl PrecompileHandle,
        validator_id: U256,
        shares: U256,
    ) -> EvmResult<u128> {
        let validator_id = try_u256_to_u32(validator_id)?;
        let shares = try_u256_to_u128(shares)?;

        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_shares = pallet_network::ValidatorDelegateStakeShares::<R>::get(validator_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_assets = pallet_network::ValidatorDelegateStakeBalance::<R>::get(validator_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let circulating_shares =
            pallet_network::ValidatorDelegateStakeCirculatingShares::<R>::get(validator_id);

        // A removed validator can retain a pool while delegators redeem its remaining principal.
        // Reject only an identifier that has neither a live validator nor initialized pool state.
        if total_shares == 0 && total_assets == 0 {
            handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
            if !pallet_network::ValidatorsData::<R>::contains_key(validator_id) {
                return Err(revert("Validator delegate stake pool does not exist"));
            }
        }

        pallet_network::Pallet::<R>::validate_delegate_pool_accounting(
            total_shares,
            total_assets,
            circulating_shares,
        )
        .map_err(|_| revert("Invalid validator delegate stake pool"))?;
        if shares > circulating_shares {
            return Err(revert("Validator delegate stake shares exceed circulation"));
        }
        if shares == 0 {
            return Ok(0);
        }
        let (assets, _) = pallet_network::Pallet::<R>::preview_delegate_pool_redemption(
            shares,
            total_shares,
            total_assets,
            1,
        )
        .map_err(|_| revert("Validator delegate stake redemption cannot be previewed"))?;

        Ok(assets)
    }

    #[precompile::public("accountSubnetDelegateStakeShares(address,uint256)")]
    #[precompile::view]
    fn account_subnet_delegate_stake_shares(
        handle: &mut impl PrecompileHandle,
        hotkey: Address,
        subnet_id: U256,
    ) -> EvmResult<u128> {
        let hotkey = R::AddressMapping::into_account_id(hotkey.into());
        let subnet_id = try_u256_to_u32(subnet_id)?;
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost().saturating_mul(3))?;
        let account_subnet_delegate_stake_shares =
            pallet_network::Pallet::<R>::current_account_subnet_delegate_stake_shares(
                &hotkey, subnet_id,
            );
        Ok(account_subnet_delegate_stake_shares)
    }

    #[precompile::public("accountSubnetDelegateStakeBalance(address,uint256)")]
    #[precompile::view]
    fn account_subnet_delegate_stake_balance(
        handle: &mut impl PrecompileHandle,
        hotkey: Address,
        subnet_id: U256,
    ) -> EvmResult<u128> {
        let hotkey = R::AddressMapping::into_account_id(hotkey.into());

        let subnet_id = try_u256_to_u32(subnet_id)?;

        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost().saturating_mul(3))?;
        let account_delegate_stake_shares =
            pallet_network::Pallet::<R>::current_account_subnet_delegate_stake_shares(
                &hotkey, subnet_id,
            );
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_subnet_delegated_stake_shares =
            pallet_network::TotalSubnetDelegateStakeShares::<R>::get(subnet_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_subnet_delegated_stake_balance =
            pallet_network::TotalSubnetDelegateStakeBalance::<R>::get(subnet_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let circulating_shares =
            pallet_network::TotalSubnetDelegateStakeCirculatingShares::<R>::get(subnet_id);

        pallet_network::Pallet::<R>::validate_delegate_pool_accounting(
            total_subnet_delegated_stake_shares,
            total_subnet_delegated_stake_balance,
            circulating_shares,
        )
        .map_err(|_| revert("Invalid subnet delegate stake pool"))?;
        let balance = pallet_network::Pallet::<R>::try_convert_to_balance(
            account_delegate_stake_shares,
            total_subnet_delegated_stake_shares,
            total_subnet_delegated_stake_balance,
        )
        .map_err(|_| revert("Subnet delegate stake balance cannot be quoted"))?;

        Ok(balance)
    }

    #[precompile::public("accountValidatorDelegateStakeShares(address,uint256)")]
    #[precompile::view]
    fn account_validator_delegate_stake_shares(
        handle: &mut impl PrecompileHandle,
        hotkey: Address,
        validator_id: U256,
    ) -> EvmResult<u128> {
        let hotkey = R::AddressMapping::into_account_id(hotkey.into());
        let validator_id = try_u256_to_u32(validator_id)?;
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost().saturating_mul(3))?;
        Ok(
            pallet_network::Pallet::<R>::current_account_validator_delegate_stake_shares(
                &hotkey,
                validator_id,
            ),
        )
    }

    #[precompile::public("accountValidatorDelegateStakeBalance(address,uint256)")]
    #[precompile::view]
    fn account_validator_delegate_stake_balance(
        handle: &mut impl PrecompileHandle,
        hotkey: Address,
        validator_id: U256,
    ) -> EvmResult<u128> {
        let hotkey = R::AddressMapping::into_account_id(hotkey.into());
        let validator_id = try_u256_to_u32(validator_id)?;

        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost().saturating_mul(3))?;
        let account_shares =
            pallet_network::Pallet::<R>::current_account_validator_delegate_stake_shares(
                &hotkey,
                validator_id,
            );
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_shares = pallet_network::ValidatorDelegateStakeShares::<R>::get(validator_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let total_assets = pallet_network::ValidatorDelegateStakeBalance::<R>::get(validator_id);
        handle.record_cost(RuntimeHelper::<R>::db_read_gas_cost())?;
        let circulating_shares =
            pallet_network::ValidatorDelegateStakeCirculatingShares::<R>::get(validator_id);

        pallet_network::Pallet::<R>::validate_delegate_pool_accounting(
            total_shares,
            total_assets,
            circulating_shares,
        )
        .map_err(|_| revert("Invalid validator delegate stake pool"))?;
        pallet_network::Pallet::<R>::try_convert_to_balance(
            account_shares,
            total_shares,
            total_assets,
        )
        .map_err(|_| revert("Validator delegate stake balance cannot be quoted"))
    }
}

fn try_u256_to_u32(value: U256) -> Result<u32, PrecompileFailure> {
    value.try_into().map_err(|_| PrecompileFailure::Error {
        exit_status: ExitError::Other("u32 out of bounds".into()),
    })
}

fn try_u256_to_u128(value: U256) -> Result<u128, PrecompileFailure> {
    value.try_into().map_err(|_| PrecompileFailure::Error {
        exit_status: ExitError::Other("u128 out of bounds".into()),
    })
}
