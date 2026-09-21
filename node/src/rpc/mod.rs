//! Native node RPC methods.

use std::sync::Arc;

use hypertensor_runtime::{AccountId, Balance, Nonce};
use jsonrpsee::RpcModule;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_runtime::traits::Block as BlockT;

pub struct FullDeps<C, P> {
    pub client: Arc<C>,
    pub pool: Arc<P>,
}

pub fn create_full<B, C, P>(
    deps: FullDeps<C, P>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    B: BlockT,
    C: ProvideRuntimeApi<B>,
    C::Api: sp_block_builder::BlockBuilder<B>,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<B, AccountId, Nonce>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<B, Balance>,
    C::Api: network_custom_rpc_runtime_api::NetworkRuntimeApi<B>,
    C: HeaderBackend<B> + HeaderMetadata<B, Error = BlockChainError> + Send + Sync + 'static,
    P: TransactionPool<Block = B> + 'static,
{
    use network_custom_rpc::{NetworkCustom, NetworkCustomApiServer};
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
    use substrate_frame_rpc_system::{System, SystemApiServer};

    let mut io = RpcModule::new(());
    let FullDeps { client, pool } = deps;
    io.merge(System::new(client.clone(), pool).into_rpc())?;
    io.merge(TransactionPayment::new(client.clone()).into_rpc())?;
    io.merge(NetworkCustom::new(client).into_rpc())?;
    Ok(io)
}
