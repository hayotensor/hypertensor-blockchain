use futures::TryFutureExt;
// Substrate
use sc_cli::{ChainSpec, SubstrateCli};

use crate::{
    chain_spec,
    cli::{Cli, Subcommand},
    service,
};

#[cfg(feature = "runtime-benchmarks")]
use hypertensor_runtime::genesis_config_presets::get_account_id_from_seed;
// use crate::chain_spec::get_account_id_from_seed;

impl SubstrateCli for Cli {
    fn impl_name() -> String {
        "Hypertensor Node".into()
    }

    fn impl_version() -> String {
        env!("SUBSTRATE_CLI_IMPL_VERSION").into()
    }

    fn description() -> String {
        env!("CARGO_PKG_DESCRIPTION").into()
    }

    fn author() -> String {
        env!("CARGO_PKG_AUTHORS").into()
    }

    fn support_url() -> String {
        "support.anonymous.an".into()
    }

    fn copyright_start_year() -> i32 {
        2021
    }

    fn load_spec(&self, id: &str) -> Result<Box<dyn ChainSpec>, String> {
        Ok(match id {
            "dev" => Box::new(chain_spec::development_chain_spec()?),
            "hoskinson" => Box::new(chain_spec::hoskinson_chain_spec()?),
            "" | "local" => Box::new(chain_spec::local_chain_spec()?),
            path => Box::new(chain_spec::ChainSpec::from_json_file(
                std::path::PathBuf::from(path),
            )?),
        })
    }
}

/// Parse and run command line arguments
pub fn run() -> sc_cli::Result<()> {
    let cli = Cli::from_args();

    match &cli.subcommand {
        Some(Subcommand::Key(cmd)) => cmd.run(&cli),
        Some(Subcommand::BuildSpec(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
        }
        Some(Subcommand::CheckBlock(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|mut config| {
                let (client, _, import_queue, task_manager) = service::new_chain_ops(&mut config)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        }
        Some(Subcommand::ExportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|mut config| {
                let (client, _, _, task_manager) = service::new_chain_ops(&mut config)?;
                Ok((cmd.run(client, config.database), task_manager))
            })
        }
        Some(Subcommand::ExportState(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|mut config| {
                let (client, _, _, task_manager) = service::new_chain_ops(&mut config)?;
                Ok((cmd.run(client, config.chain_spec), task_manager))
            })
        }
        Some(Subcommand::ImportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|mut config| {
                let (client, _, import_queue, task_manager) = service::new_chain_ops(&mut config)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        }
        Some(Subcommand::PurgeChain(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run(config.database))
        }
        Some(Subcommand::Revert(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|mut config| {
                let (client, backend, _, task_manager) = service::new_chain_ops(&mut config)?;
                let aux_revert = Box::new(
                    move |client: std::sync::Arc<service::Client>, backend, blocks| {
                        sc_consensus_babe::revert(client.clone(), backend, blocks)?;
                        sc_consensus_grandpa::revert(client, blocks)?;
                        Ok(())
                    },
                );
                Ok((cmd.run(client, backend, Some(aux_revert)), task_manager))
            })
        }
        #[cfg(feature = "runtime-benchmarks")]
        Some(Subcommand::Benchmark(cmd)) => {
            use crate::benchmarking::{
                inherent_benchmark_data, RemarkBuilder, TransferKeepAliveBuilder,
            };
            use frame_benchmarking_cli::{
                BenchmarkCmd, ExtrinsicFactory, SUBSTRATE_REFERENCE_HARDWARE,
            };
            use hypertensor_runtime::{Hashing, EXISTENTIAL_DEPOSIT};

            let runner = cli.create_runner(cmd)?;
            match cmd {
                BenchmarkCmd::Pallet(cmd) => runner
                    .sync_run(|config| cmd.run_with_spec::<Hashing, ()>(Some(config.chain_spec))),
                BenchmarkCmd::Block(cmd) => runner.sync_run(|mut config| {
                    let (client, _, _, _) = service::new_chain_ops(&mut config)?;
                    cmd.run(client)
                }),
                BenchmarkCmd::Storage(cmd) => runner.sync_run(|mut config| {
                    let (client, backend, _, _) = service::new_chain_ops(&mut config)?;
                    let db = backend.expose_db();
                    let storage = backend.expose_storage();
                    let shared_trie_cache = backend.expose_shared_trie_cache();
                    cmd.run(config, client, db, storage, shared_trie_cache)
                }),
                BenchmarkCmd::Overhead(cmd) => runner.sync_run(|mut config| {
                    let (client, _, _, _) = service::new_chain_ops(&mut config)?;
                    let ext_builder = RemarkBuilder::new(client.clone());
                    cmd.run(
                        config.chain_spec.name().into(),
                        client,
                        inherent_benchmark_data()?,
                        Vec::new(),
                        &ext_builder,
                        false,
                    )
                }),
                BenchmarkCmd::Extrinsic(cmd) => runner.sync_run(|mut config| {
                    let (client, _, _, _) = service::new_chain_ops(&mut config)?;
                    // Register the *Remark* and *TKA* builders.
                    let ext_factory = ExtrinsicFactory(vec![
                        Box::new(RemarkBuilder::new(client.clone())),
                        Box::new(TransferKeepAliveBuilder::new(
                            client.clone(),
                            get_account_id_from_seed::<sp_core::sr25519::Public>("Alice"),
                            EXISTENTIAL_DEPOSIT,
                        )),
                    ]);

                    cmd.run(client, inherent_benchmark_data()?, Vec::new(), &ext_factory)
                }),
                BenchmarkCmd::Machine(cmd) => {
                    runner.sync_run(|config| cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone()))
                }
            }
        }
        #[cfg(not(feature = "runtime-benchmarks"))]
        Some(Subcommand::Benchmark) => Err("Benchmarking wasn't enabled when building the node. \
			You can enable it with `--features runtime-benchmarks`."
            .into()),
        None => {
            let runner = cli.create_runner(&cli.run)?;
            runner.run_node_until_exit(|config| async move {
                service::build_full(config).map_err(Into::into).await
            })
        }
    }
}
