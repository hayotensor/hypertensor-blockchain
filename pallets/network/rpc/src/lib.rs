use fp_account::AccountId20;
use jsonrpsee::{
    core::RpcResult,
    proc_macros::rpc,
    types::{error::ErrorObject, ErrorObjectOwned},
};
use network_rpc_types::{
    ConsensusRoundInfo, NetworkQueryError, OverwatchNodeInfo, OverwatchNodesPage, PageRequest,
    SubnetBootnodes, SubnetEpochStatus, SubnetInfo, SubnetNodeCursor, SubnetNodeInfo,
    SubnetNodesPage, SubnetValidatorNodesPage, SubnetsPage, ValidatorInfo,
    ValidatorNodeAllocationsPage, ValidatorNodeStakesPage, ValidatorNodesPage,
};
use sp_api::{ApiError, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use std::{fmt::Debug, sync::Arc};

pub use network_custom_rpc_runtime_api::NetworkRuntimeApi;

const RUNTIME_API_ERROR_CODE: i32 = -32001;
const NETWORK_QUERY_ERROR_CODE: i32 = -32010;
const INVALID_PARAMS_CODE: i32 = -32602;

#[rpc(client, server)]
pub trait NetworkCustomApi<BlockHash> {
    #[method(name = "network_getSubnetInfo")]
    fn get_subnet_info(
        &self,
        subnet_id: u32,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<SubnetInfo<AccountId20>>>;

    #[method(name = "network_getSubnets")]
    fn get_subnets(
        &self,
        request: PageRequest<u32>,
        at: Option<BlockHash>,
    ) -> RpcResult<SubnetsPage<AccountId20>>;

    #[method(name = "network_getSubnetNodeInfo")]
    fn get_subnet_node_info(
        &self,
        subnet_id: u32,
        subnet_node_id: u32,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<SubnetNodeInfo<AccountId20>>>;

    #[method(name = "network_getSubnetNodes")]
    fn get_subnet_nodes(
        &self,
        subnet_id: u32,
        request: PageRequest<u32>,
        at: Option<BlockHash>,
    ) -> RpcResult<SubnetNodesPage<AccountId20>>;

    #[method(name = "network_getBootnodes")]
    fn get_bootnodes(
        &self,
        subnet_id: u32,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<SubnetBootnodes>>;

    #[method(name = "network_getValidatorInfo")]
    fn get_validator_info(
        &self,
        validator_id: u32,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<ValidatorInfo<AccountId20>>>;

    #[method(name = "network_getValidatorByColdkey")]
    fn get_validator_by_coldkey(
        &self,
        coldkey: AccountId20,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<ValidatorInfo<AccountId20>>>;

    #[method(name = "network_getValidatorByHotkey")]
    fn get_validator_by_hotkey(
        &self,
        hotkey: AccountId20,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<ValidatorInfo<AccountId20>>>;

    #[method(name = "network_getValidatorNodes")]
    fn get_validator_nodes(
        &self,
        validator_id: u32,
        request: PageRequest<SubnetNodeCursor>,
        at: Option<BlockHash>,
    ) -> RpcResult<ValidatorNodesPage<AccountId20>>;

    #[method(name = "network_getValidatorNodeStakes")]
    fn get_validator_node_stakes(
        &self,
        validator_id: u32,
        request: PageRequest<SubnetNodeCursor>,
        at: Option<BlockHash>,
    ) -> RpcResult<ValidatorNodeStakesPage>;

    #[method(name = "network_getValidatorNodeAllocations")]
    fn get_validator_node_allocations(
        &self,
        validator_id: u32,
        request: PageRequest<SubnetNodeCursor>,
        at: Option<BlockHash>,
    ) -> RpcResult<ValidatorNodeAllocationsPage>;

    #[method(name = "network_getConsensusRound")]
    fn get_consensus_round(
        &self,
        subnet_id: u32,
        subnet_epoch: u32,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<ConsensusRoundInfo>>;

    #[method(name = "network_getSubnetValidatorNodes")]
    fn get_subnet_validator_nodes(
        &self,
        subnet_id: u32,
        request: PageRequest<u32>,
        at: Option<BlockHash>,
    ) -> RpcResult<SubnetValidatorNodesPage<AccountId20>>;

    #[method(name = "network_getSubnetEpochStatus")]
    fn get_subnet_epoch_status(
        &self,
        subnet_id: u32,
        at: Option<BlockHash>,
    ) -> RpcResult<SubnetEpochStatus>;

    #[method(name = "network_getOverwatchNodeInfo")]
    fn get_overwatch_node_info(
        &self,
        overwatch_node_id: u32,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<OverwatchNodeInfo<AccountId20>>>;

    #[method(name = "network_getOverwatchNodes")]
    fn get_overwatch_nodes(
        &self,
        request: PageRequest<u32>,
        at: Option<BlockHash>,
    ) -> RpcResult<OverwatchNodesPage<AccountId20>>;
}

/// Native JSON-RPC adapter for the network runtime API.
pub struct NetworkCustom<C, Block> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<Block>,
}

impl<C, Block> NetworkCustom<C, Block> {
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block> NetworkCustom<C, Block>
where
    Block: BlockT,
    C: HeaderBackend<Block>,
{
    fn resolve_at(&self, at: Option<Block::Hash>) -> Block::Hash {
        at.unwrap_or_else(|| self.client.info().best_hash)
    }
}

fn runtime_error(error: impl Debug) -> ErrorObjectOwned {
    ErrorObject::owned(
        RUNTIME_API_ERROR_CODE,
        "Network runtime API invocation failed",
        Some(format!("{error:?}")),
    )
}

fn query_error(error: NetworkQueryError) -> ErrorObjectOwned {
    let (code, message) = match &error {
        NetworkQueryError::InvalidPageLimit { .. } => {
            (INVALID_PARAMS_CODE, "Invalid network query parameters")
        }
        _ => (NETWORK_QUERY_ERROR_CODE, "Network query failed"),
    };

    ErrorObject::owned(code, message, Some(error))
}

fn runtime_result<T>(result: Result<T, ApiError>) -> RpcResult<T> {
    result.map_err(runtime_error)
}

fn query_result<T>(result: Result<Result<T, NetworkQueryError>, ApiError>) -> RpcResult<T> {
    runtime_result(result)?.map_err(query_error)
}

fn validate_page<Cursor>(request: &PageRequest<Cursor>) -> RpcResult<()> {
    request.validated_limit().map(|_| ()).map_err(query_error)
}

impl<C, Block> NetworkCustomApiServer<<Block as BlockT>::Hash> for NetworkCustom<C, Block>
where
    Block: BlockT,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: NetworkRuntimeApi<Block>,
{
    fn get_subnet_info(
        &self,
        subnet_id: u32,
        at: Option<Block::Hash>,
    ) -> RpcResult<Option<SubnetInfo<AccountId20>>> {
        let at = self.resolve_at(at);
        runtime_result(self.client.runtime_api().get_subnet_info(at, subnet_id))
    }

    fn get_subnets(
        &self,
        request: PageRequest<u32>,
        at: Option<Block::Hash>,
    ) -> RpcResult<SubnetsPage<AccountId20>> {
        validate_page(&request)?;
        let at = self.resolve_at(at);
        query_result(self.client.runtime_api().get_subnets(at, request))
    }

    fn get_subnet_node_info(
        &self,
        subnet_id: u32,
        subnet_node_id: u32,
        at: Option<Block::Hash>,
    ) -> RpcResult<Option<SubnetNodeInfo<AccountId20>>> {
        let at = self.resolve_at(at);
        runtime_result(self.client.runtime_api().get_subnet_node_info(
            at,
            subnet_id,
            subnet_node_id,
        ))
    }

    fn get_subnet_nodes(
        &self,
        subnet_id: u32,
        request: PageRequest<u32>,
        at: Option<Block::Hash>,
    ) -> RpcResult<SubnetNodesPage<AccountId20>> {
        validate_page(&request)?;
        let at = self.resolve_at(at);
        query_result(
            self.client
                .runtime_api()
                .get_subnet_nodes(at, subnet_id, request),
        )
    }

    fn get_bootnodes(
        &self,
        subnet_id: u32,
        at: Option<Block::Hash>,
    ) -> RpcResult<Option<SubnetBootnodes>> {
        let at = self.resolve_at(at);
        runtime_result(self.client.runtime_api().get_bootnodes(at, subnet_id))
    }

    fn get_validator_info(
        &self,
        validator_id: u32,
        at: Option<Block::Hash>,
    ) -> RpcResult<Option<ValidatorInfo<AccountId20>>> {
        let at = self.resolve_at(at);
        runtime_result(
            self.client
                .runtime_api()
                .get_validator_info(at, validator_id),
        )
    }

    fn get_validator_by_coldkey(
        &self,
        coldkey: AccountId20,
        at: Option<Block::Hash>,
    ) -> RpcResult<Option<ValidatorInfo<AccountId20>>> {
        let at = self.resolve_at(at);
        runtime_result(
            self.client
                .runtime_api()
                .get_validator_by_coldkey(at, coldkey),
        )
    }

    fn get_validator_by_hotkey(
        &self,
        hotkey: AccountId20,
        at: Option<Block::Hash>,
    ) -> RpcResult<Option<ValidatorInfo<AccountId20>>> {
        let at = self.resolve_at(at);
        runtime_result(
            self.client
                .runtime_api()
                .get_validator_by_hotkey(at, hotkey),
        )
    }

    fn get_validator_nodes(
        &self,
        validator_id: u32,
        request: PageRequest<SubnetNodeCursor>,
        at: Option<Block::Hash>,
    ) -> RpcResult<ValidatorNodesPage<AccountId20>> {
        validate_page(&request)?;
        let at = self.resolve_at(at);
        query_result(
            self.client
                .runtime_api()
                .get_validator_nodes(at, validator_id, request),
        )
    }

    fn get_validator_node_stakes(
        &self,
        validator_id: u32,
        request: PageRequest<SubnetNodeCursor>,
        at: Option<Block::Hash>,
    ) -> RpcResult<ValidatorNodeStakesPage> {
        validate_page(&request)?;
        let at = self.resolve_at(at);
        query_result(
            self.client
                .runtime_api()
                .get_validator_node_stakes(at, validator_id, request),
        )
    }

    fn get_validator_node_allocations(
        &self,
        validator_id: u32,
        request: PageRequest<SubnetNodeCursor>,
        at: Option<Block::Hash>,
    ) -> RpcResult<ValidatorNodeAllocationsPage> {
        validate_page(&request)?;
        let at = self.resolve_at(at);
        query_result(self.client.runtime_api().get_validator_node_allocations(
            at,
            validator_id,
            request,
        ))
    }

    fn get_consensus_round(
        &self,
        subnet_id: u32,
        subnet_epoch: u32,
        at: Option<Block::Hash>,
    ) -> RpcResult<Option<ConsensusRoundInfo>> {
        let at = self.resolve_at(at);
        query_result(
            self.client
                .runtime_api()
                .get_consensus_round(at, subnet_id, subnet_epoch),
        )
    }

    fn get_subnet_validator_nodes(
        &self,
        subnet_id: u32,
        request: PageRequest<u32>,
        at: Option<Block::Hash>,
    ) -> RpcResult<SubnetValidatorNodesPage<AccountId20>> {
        validate_page(&request)?;
        let at = self.resolve_at(at);
        query_result(
            self.client
                .runtime_api()
                .get_subnet_validator_nodes(at, subnet_id, request),
        )
    }

    fn get_subnet_epoch_status(
        &self,
        subnet_id: u32,
        at: Option<Block::Hash>,
    ) -> RpcResult<SubnetEpochStatus> {
        let at = self.resolve_at(at);
        query_result(
            self.client
                .runtime_api()
                .get_subnet_epoch_status(at, subnet_id),
        )
    }

    fn get_overwatch_node_info(
        &self,
        overwatch_node_id: u32,
        at: Option<Block::Hash>,
    ) -> RpcResult<Option<OverwatchNodeInfo<AccountId20>>> {
        let at = self.resolve_at(at);
        runtime_result(
            self.client
                .runtime_api()
                .get_overwatch_node_info(at, overwatch_node_id),
        )
    }

    fn get_overwatch_nodes(
        &self,
        request: PageRequest<u32>,
        at: Option<Block::Hash>,
    ) -> RpcResult<OverwatchNodesPage<AccountId20>> {
        validate_page(&request)?;
        let at = self.resolve_at(at);
        query_result(self.client.runtime_api().get_overwatch_nodes(at, request))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use network_rpc_types::MAX_PAGE_SIZE;

    #[test]
    fn page_limits_are_rejected_before_runtime_dispatch() {
        for limit in [0, MAX_PAGE_SIZE + 1] {
            let error = validate_page(&PageRequest::<u32> {
                cursor: None,
                limit,
            })
            .expect_err("invalid page size must fail");

            assert_eq!(error.code(), INVALID_PARAMS_CODE);
            assert_eq!(error.message(), "Invalid network query parameters");
            assert!(error
                .data()
                .expect("domain error data is present")
                .get()
                .contains("invalidPageLimit"));
        }
    }

    #[test]
    fn domain_errors_use_stable_codes() {
        let missing_subnet = query_error(NetworkQueryError::SubnetNotFound { subnet_id: 7 });
        assert_eq!(missing_subnet.code(), NETWORK_QUERY_ERROR_CODE);
        assert_eq!(missing_subnet.message(), "Network query failed");
        assert!(missing_subnet
            .data()
            .expect("domain error data is present")
            .get()
            .contains("subnetNotFound"));
    }

    #[test]
    fn runtime_failures_use_the_runtime_api_code() {
        let error = runtime_error("mock runtime failure");
        assert_eq!(error.code(), RUNTIME_API_ERROR_CODE);
        assert_eq!(error.message(), "Network runtime API invocation failed");
        assert!(error
            .data()
            .expect("runtime diagnostic data is present")
            .get()
            .contains("mock runtime failure"));
    }
}
