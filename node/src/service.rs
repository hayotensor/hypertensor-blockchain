//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use std::{sync::Arc, time::Duration};

use futures::prelude::*;
// Substrate
use sc_client_api::{Backend as BackendT, BlockBackend};
use sc_consensus::{BoxBlockImport, DefaultImportQueue};
use sc_consensus_grandpa::BlockNumberOps;
use sc_executor::HostFunctions as HostFunctionsT;
use sc_network_sync::strategy::warp::{WarpSyncConfig, WarpSyncProvider};
use sc_service::{error::Error as ServiceError, Configuration, PartialComponents, TaskManager};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sc_transaction_pool::TransactionPoolHandle;
use sc_transaction_pool_api::OffchainTransactionPoolFactory;
use sp_api::ConstructRuntimeApi;
use sp_core::H256;
use sp_runtime::traits::{Block as BlockT, NumberFor};
// Runtime
use hypertensor_runtime::{opaque::Block, AccountId, Balance, Nonce, RuntimeApi};

use crate::client::{FullBackend, FullClient, RuntimeApiCollection};

/// Only enable the benchmarking host functions when we actually want to benchmark.
#[cfg(feature = "runtime-benchmarks")]
pub type HostFunctions = (
    sp_io::SubstrateHostFunctions,
    frame_benchmarking::benchmarking::HostFunctions,
);
/// Otherwise we use empty host functions for ext host functions.
#[cfg(not(feature = "runtime-benchmarks"))]
pub type HostFunctions = sp_io::SubstrateHostFunctions;

pub type Backend = FullBackend<Block>;
pub type Client = FullClient<Block, RuntimeApi, HostFunctions>;

type FullSelectChain<B> = sc_consensus::LongestChain<FullBackend<B>, B>;
type GrandpaLinkHalf<B, C> = sc_consensus_grandpa::LinkHalf<B, C, FullSelectChain<B>>;

/// The minimum period of blocks on which justifications will be
/// imported and generated.
const GRANDPA_JUSTIFICATION_PERIOD: u32 = 512;

pub fn new_partial<B, RA, HF>(
    config: &Configuration,
) -> Result<
    PartialComponents<
        FullClient<B, RA, HF>,
        FullBackend<B>,
        FullSelectChain<B>,
        DefaultImportQueue<B>,
        sc_transaction_pool::TransactionPoolHandle<B, FullClient<B, RA, HF>>,
        (
            Option<Telemetry>,
            BoxBlockImport<B>,
            GrandpaLinkHalf<B, FullClient<B, RA, HF>>,
            sc_consensus_babe::BabeLink<B>,
        ),
    >,
    ServiceError,
>
where
    B: BlockT<Hash = H256>,
    RA: ConstructRuntimeApi<B, FullClient<B, RA, HF>>,
    RA: Send + Sync + 'static,
    RA::RuntimeApi: RuntimeApiCollection<B, AccountId, Nonce, Balance>,
    NumberFor<B>: BlockNumberOps,
    HF: HostFunctionsT + 'static,
{
    let telemetry = config
        .telemetry_endpoints
        .clone()
        .filter(|x| !x.is_empty())
        .map(|endpoints| -> Result<_, sc_telemetry::Error> {
            let worker = TelemetryWorker::new(16)?;
            let telemetry = worker.handle().new_telemetry(endpoints);
            Ok((worker, telemetry))
        })
        .transpose()?;

    let executor = sc_service::new_wasm_executor(&config.executor);

    let (client, backend, keystore_container, mut task_manager) =
        sc_service::new_full_parts_record_import::<B, RA, _>(
            config,
            telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
            executor,
            true,
            vec![Arc::new(sc_consensus_grandpa::GrandpaPruningFilter)],
        )?;
    let client = Arc::new(client);

    let telemetry = telemetry.map(|(worker, telemetry)| {
        task_manager
            .spawn_handle()
            .spawn("telemetry", None, worker.run());
        telemetry
    });

    let select_chain = sc_consensus::LongestChain::new(backend.clone());
    let (grandpa_block_import, grandpa_link) = sc_consensus_grandpa::block_import(
        client.clone(),
        GRANDPA_JUSTIFICATION_PERIOD,
        &client,
        select_chain.clone(),
        telemetry.as_ref().map(|x| x.handle()),
    )?;

    let transaction_pool: Arc<TransactionPoolHandle<B, FullClient<B, RA, HF>>> = Arc::from(
        sc_transaction_pool::Builder::new(
            task_manager.spawn_essential_handle(),
            client.clone(),
            config.role.is_authority().into(),
        )
        .with_options(config.transaction_pool.clone())
        .with_prometheus(config.prometheus_registry())
        .build(),
    );

    let babe_config = sc_consensus_babe::configuration(&*client)?;
    let slot_duration = babe_config.slot_duration();
    let (block_import, babe_link) = sc_consensus_babe::block_import(
        babe_config,
        grandpa_block_import.clone(),
        client.clone(),
        move |_: B::Hash, ()| async move {
            let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
            let slot = sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                *timestamp, slot_duration,
            );
            Ok((slot, timestamp))
        },
        select_chain.clone(),
        OffchainTransactionPoolFactory::new(transaction_pool.clone()),
    )?;
    let (import_queue, babe_worker_handle) =
        sc_consensus_babe::import_queue(sc_consensus_babe::ImportQueueParams {
            link: babe_link.clone(),
            block_import: block_import.clone(),
            justification_import: Some(Box::new(grandpa_block_import)),
            client: client.clone(),
            slot_duration,
            spawner: &task_manager.spawn_essential_handle(),
            registry: config.prometheus_registry(),
            telemetry: telemetry.as_ref().map(|x| x.handle()),
        })?;
    // Closing the last sender terminates the essential BABE request worker.
    task_manager.keep_alive(babe_worker_handle);

    Ok(PartialComponents {
        client,
        backend,
        keystore_container,
        task_manager,
        select_chain,
        import_queue,
        transaction_pool,
        other: (telemetry, Box::new(block_import), grandpa_link, babe_link),
    })
}

/// Builds a new service for a full client.
pub async fn new_full<B, RA, HF, NB>(config: Configuration) -> Result<TaskManager, ServiceError>
where
    B: BlockT<Hash = H256>,
    NumberFor<B>: BlockNumberOps,
    <B as BlockT>::Header: Unpin,
    RA: ConstructRuntimeApi<B, FullClient<B, RA, HF>>,
    RA: Send + Sync + 'static,
    RA::RuntimeApi: RuntimeApiCollection<B, AccountId, Nonce, Balance>,
    HF: HostFunctionsT + 'static,
    NB: sc_network::NetworkBackend<B, <B as BlockT>::Hash>,
{
    let PartialComponents {
        client,
        backend,
        mut task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other: (mut telemetry, block_import, grandpa_link, babe_link),
    } = new_partial::<B, RA, HF>(&config)?;

    let maybe_registry = config.prometheus_config.as_ref().map(|cfg| &cfg.registry);
    let mut net_config = sc_network::config::FullNetworkConfiguration::<_, _, NB>::new(
        &config.network,
        maybe_registry.cloned(),
    );
    let peer_store_handle = net_config.peer_store_handle();
    let metrics = NB::register_notification_metrics(maybe_registry);

    let grandpa_protocol_name = sc_consensus_grandpa::protocol_standard_name(
        &client
            .block_hash(0u32.into())
            .ok()
            .flatten()
            .expect("Genesis block exists; qed"),
        &config.chain_spec,
    );

    let (grandpa_protocol_config, grandpa_notification_service) =
        sc_consensus_grandpa::grandpa_peers_set_config::<_, NB>(
            grandpa_protocol_name.clone(),
            metrics.clone(),
            peer_store_handle,
        );

    let warp_sync_config = {
        net_config.add_notification_protocol(grandpa_protocol_config);
        let warp_sync: Arc<dyn WarpSyncProvider<B>> =
            Arc::new(sc_consensus_grandpa::warp_proof::NetworkProvider::new(
                backend.clone(),
                grandpa_link.shared_authority_set().clone(),
                Vec::new(),
            ));
        Some(WarpSyncConfig::WithProvider(warp_sync))
    };

    let (network, system_rpc_tx, tx_handler_controller, sync_service) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            net_config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            spawn_essential_handle: task_manager.spawn_essential_handle(),
            import_queue,
            block_announce_validator_builder: None,
            warp_sync_config,
            block_relay: None,
            metrics,
        })?;

    if config.offchain_worker.enabled {
        let offchain_workers =
            sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
                runtime_api_provider: client.clone(),
                is_validator: config.role.is_authority(),
                keystore: Some(keystore_container.keystore()),
                offchain_db: backend.offchain_storage(),
                transaction_pool: Some(OffchainTransactionPoolFactory::new(
                    transaction_pool.clone(),
                )),
                network_provider: Arc::new(network.clone()),
                enable_http_requests: true,
                custom_extensions: |_| vec![],
            })?;
        task_manager.spawn_handle().spawn(
            "offchain-workers-runner",
            "offchain-worker",
            offchain_workers
                .run(client.clone(), task_manager.spawn_handle())
                .boxed(),
        );
    }

    let role = config.role;
    let force_authoring = config.force_authoring;
    let name = config.network.node_name.clone();
    let enable_grandpa = !config.disable_grandpa;
    let prometheus_registry = config.prometheus_registry().cloned();

    let rpc_builder = {
        let client = client.clone();
        let pool = transaction_pool.clone();
        Box::new(move |_subscription_task_executor| {
            crate::rpc::create_full(crate::rpc::FullDeps {
                client: client.clone(),
                pool: pool.clone(),
            })
            .map_err(Into::into)
        })
    };

    let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        config,
        client: client.clone(),
        backend: backend.clone(),
        task_manager: &mut task_manager,
        keystore: keystore_container.keystore(),
        transaction_pool: transaction_pool.clone(),
        rpc_builder,
        network: Arc::new(network.clone()),
        system_rpc_tx,
        tx_handler_controller,
        sync_service: sync_service.clone(),
        telemetry: telemetry.as_mut(),
        tracing_execute_block: None,
    })?;

    if role.is_authority() {
        let proposer_factory = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            prometheus_registry.as_ref(),
            telemetry.as_ref().map(|x| x.handle()),
        );

        let slot_duration = babe_link.config().slot_duration();
        let create_inherent_data_providers = move |_, ()| async move {
            let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
            let slot = sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
				*timestamp,
				slot_duration,
			);
            Ok((slot, timestamp))
        };

        let babe = sc_consensus_babe::start_babe(sc_consensus_babe::BabeParams {
            client,
            select_chain,
            block_import,
            env: proposer_factory,
            sync_oracle: sync_service.clone(),
            justification_sync_link: sync_service.clone(),
            create_inherent_data_providers,
            force_authoring,
            backoff_authoring_blocks: Some(
                sc_consensus_slots::BackoffAuthoringOnFinalizedHeadLagging::default(),
            ),
            keystore: keystore_container.keystore(),
            babe_link,
            block_proposal_slot_portion: sc_consensus_babe::SlotProportion::new(2f32 / 3f32),
            max_block_proposal_slot_portion: None,
            telemetry: telemetry.as_ref().map(|x| x.handle()),
        })?;
        task_manager
            .spawn_essential_handle()
            .spawn_blocking("babe", Some("block-authoring"), babe);
    }

    if enable_grandpa {
        // if the node isn't actively participating in consensus then it doesn't
        // need a keystore, regardless of which protocol we use below.
        let keystore = if role.is_authority() {
            Some(keystore_container.keystore())
        } else {
            None
        };

        let grandpa_config = sc_consensus_grandpa::Config {
            // FIXME #1578 make this available through chainspec
            gossip_duration: Duration::from_millis(333),
            justification_generation_period: GRANDPA_JUSTIFICATION_PERIOD,
            name: Some(name),
            observer_enabled: false,
            keystore,
            local_role: role,
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            protocol_name: grandpa_protocol_name,
        };

        // start the full GRANDPA voter
        // NOTE: non-authorities could run the GRANDPA observer protocol, but at
        // this point the full voter should provide better guarantees of block
        // and vote data availability than the observer. The observer has not
        // been tested extensively yet and having most nodes in a network run it
        // could lead to finality stalls.
        let grandpa_voter =
            sc_consensus_grandpa::run_grandpa_voter(sc_consensus_grandpa::GrandpaParams {
                config: grandpa_config,
                link: grandpa_link,
                network,
                sync: Arc::new(sync_service),
                notification_service: grandpa_notification_service,
                voting_rule: sc_consensus_grandpa::VotingRulesBuilder::default().build(),
                prometheus_registry,
                shared_voter_state: sc_consensus_grandpa::SharedVoterState::empty(),
                telemetry: telemetry.as_ref().map(|x| x.handle()),
                offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool),
            })?;

        // the GRANDPA voter task is considered infallible, i.e.
        // if it fails we take down the service with it.
        task_manager
            .spawn_essential_handle()
            .spawn_blocking("grandpa-voter", None, grandpa_voter);
    }

    Ok(task_manager)
}

pub async fn build_full(config: Configuration) -> Result<TaskManager, ServiceError> {
    match config.network.network_backend {
        sc_network::config::NetworkBackendType::Libp2p => {
            new_full::<Block, RuntimeApi, HostFunctions, sc_network::NetworkWorker<_, _>>(config)
                .await
        }
        sc_network::config::NetworkBackendType::Litep2p => {
            new_full::<Block, RuntimeApi, HostFunctions, sc_network::Litep2pNetworkBackend>(config)
                .await
        }
    }
}

pub fn new_chain_ops(
    config: &mut Configuration,
) -> Result<
    (
        Arc<Client>,
        Arc<Backend>,
        DefaultImportQueue<Block>,
        TaskManager,
    ),
    ServiceError,
> {
    config.keystore = sc_service::config::KeystoreConfig::InMemory;
    let PartialComponents {
        client,
        backend,
        import_queue,
        task_manager,
        ..
    } = new_partial::<Block, RuntimeApi, HostFunctions>(config)?;
    Ok((client, backend, import_queue, task_manager))
}
