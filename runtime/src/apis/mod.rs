//! Runtime APIs exposed to the node and RPC clients.

use crate::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;

pallet_revive::impl_runtime_apis_plus_revive_traits! {
    Runtime, Revive, Executive, EthExtraImpl,
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: <Block as BlockT>::LazyBlock) {
            Executive::execute_block(block)
        }

        fn initialize_block(header: &<Block as BlockT>::Header) -> ExtrinsicInclusionMode {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: <Block as BlockT>::LazyBlock,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
        fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
            tokenomics::build_genesis_state(config)
        }

        fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
            get_preset::<RuntimeGenesisConfig>(id, crate::genesis::presets::get_preset)
        }

        fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
            crate::genesis::presets::preset_names()
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(
            owner: Vec<u8>,
            seed: Option<Vec<u8>>,
        ) -> sp_session::OpaqueGeneratedSessionKeys {
            opaque::SessionKeys::generate(&owner, seed).into()
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl sp_consensus_babe::BabeApi<Block> for Runtime {
        fn configuration() -> sp_consensus_babe::BabeConfiguration {
            let epoch_config = Babe::epoch_config().unwrap_or(BABE_GENESIS_EPOCH_CONFIG);
            sp_consensus_babe::BabeConfiguration {
                slot_duration: Babe::slot_duration(),
                epoch_length: EpochDuration::get(),
                c: epoch_config.c,
                authorities: Babe::authorities().to_vec(),
                randomness: Babe::randomness(),
                allowed_slots: epoch_config.allowed_slots,
            }
        }

        fn current_epoch_start() -> sp_consensus_babe::Slot {
            Babe::current_epoch_start()
        }

        fn current_epoch() -> sp_consensus_babe::Epoch {
            Babe::current_epoch()
        }

        fn next_epoch() -> sp_consensus_babe::Epoch {
            Babe::next_epoch()
        }

        fn generate_key_ownership_proof(
            _slot: sp_consensus_babe::Slot,
            authority_id: sp_consensus_babe::AuthorityId,
        ) -> Option<sp_consensus_babe::OpaqueKeyOwnershipProof> {
            use codec::Encode;

            Historical::prove((sp_consensus_babe::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(sp_consensus_babe::OpaqueKeyOwnershipProof::new)
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: sp_consensus_babe::EquivocationProof<<Block as BlockT>::Header>,
            key_owner_proof: sp_consensus_babe::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;

            Babe::submit_unsigned_equivocation_report(
                equivocation_proof,
                key_owner_proof,
            )
        }
    }


    impl sp_consensus_grandpa::GrandpaApi<Block> for Runtime {
        fn grandpa_authorities() -> GrandpaAuthorityList {
            Grandpa::grandpa_authorities()
        }

        fn current_set_id() -> sp_consensus_grandpa::SetId {
            Grandpa::current_set_id()
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: sp_consensus_grandpa::EquivocationProof<
                <Block as BlockT>::Hash,
                NumberFor<Block>,
            >,
            key_owner_proof: sp_consensus_grandpa::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            Grandpa::submit_unsigned_equivocation_report(equivocation_proof, key_owner_proof.decode()?)
        }

        fn generate_key_ownership_proof(
            _set_id: sp_consensus_grandpa::SetId,
            authority_id: GrandpaId,
        ) -> Option<sp_consensus_grandpa::OpaqueKeyOwnershipProof> {
            Historical::prove((sp_consensus_grandpa::KEY_TYPE, authority_id))
                .map(|proof| sp_consensus_grandpa::OpaqueKeyOwnershipProof::new(proof.encode()))
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
        fn account_nonce(account: AccountId) -> Nonce {
            System::account_nonce(account)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
        Block,
        Balance,
    > for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32
        ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_info(uxt, len)
        }

        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_fee_details(uxt, len)
        }

        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }

        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl network_custom_rpc_runtime_api::NetworkRuntimeApi<Block> for Runtime {
        fn get_subnet_info(
            subnet_id: u32,
        ) -> Option<network_rpc_types::SubnetInfo<AccountId>> {
            Network::rpc_get_subnet_info(subnet_id)
        }

        fn get_subnets(
            request: network_rpc_types::PageRequest<u32>,
        ) -> Result<
            network_rpc_types::SubnetsPage<AccountId>,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_subnets(request)
        }

        fn get_subnet_node_info(
            subnet_id: u32,
            subnet_node_id: u32,
        ) -> Option<network_rpc_types::SubnetNodeInfo<AccountId>> {
            Network::rpc_get_subnet_node_info(subnet_id, subnet_node_id)
        }

        fn get_subnet_nodes(
            subnet_id: u32,
            request: network_rpc_types::PageRequest<u32>,
        ) -> Result<
            network_rpc_types::SubnetNodesPage<AccountId>,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_subnet_nodes(subnet_id, request)
        }

        fn get_bootnodes(subnet_id: u32) -> Option<network_rpc_types::SubnetBootnodes> {
            Network::rpc_get_bootnodes(subnet_id)
        }

        fn get_validator_info(
            validator_id: u32,
        ) -> Option<network_rpc_types::ValidatorInfo<AccountId>> {
            Network::rpc_get_validator_info(validator_id)
        }

        fn get_validator_by_coldkey(
            coldkey: AccountId,
        ) -> Option<network_rpc_types::ValidatorInfo<AccountId>> {
            Network::rpc_get_validator_by_coldkey(&coldkey)
        }

        fn get_validator_by_hotkey(
            hotkey: AccountId,
        ) -> Option<network_rpc_types::ValidatorInfo<AccountId>> {
            Network::rpc_get_validator_by_hotkey(&hotkey)
        }

        fn get_validator_nodes(
            validator_id: u32,
            request: network_rpc_types::PageRequest<network_rpc_types::SubnetNodeCursor>,
        ) -> Result<
            network_rpc_types::ValidatorNodesPage<AccountId>,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_validator_nodes(validator_id, request)
        }

        fn get_validator_node_stakes(
            validator_id: u32,
            request: network_rpc_types::PageRequest<network_rpc_types::SubnetNodeCursor>,
        ) -> Result<
            network_rpc_types::ValidatorNodeStakesPage,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_validator_node_stakes(validator_id, request)
        }

        fn get_validator_node_allocations(
            validator_id: u32,
            request: network_rpc_types::PageRequest<network_rpc_types::SubnetNodeCursor>,
        ) -> Result<
            network_rpc_types::ValidatorNodeAllocationsPage,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_validator_node_allocations(validator_id, request)
        }

        fn get_consensus_round(
            subnet_id: u32,
            subnet_epoch: u32,
        ) -> Result<
            Option<network_rpc_types::ConsensusRoundInfo>,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_consensus_round(subnet_id, subnet_epoch)
        }

        fn get_subnet_validator_nodes(
            subnet_id: u32,
            request: network_rpc_types::PageRequest<u32>,
        ) -> Result<
            network_rpc_types::SubnetValidatorNodesPage<AccountId>,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_subnet_validator_nodes(subnet_id, request)
        }

        fn get_subnet_epoch_status(
            subnet_id: u32,
        ) -> Result<
            network_rpc_types::SubnetEpochStatus,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_subnet_epoch_status(subnet_id)
        }

        fn get_overwatch_node_info(
            overwatch_node_id: u32,
        ) -> Option<network_rpc_types::OverwatchNodeInfo<AccountId>> {
            Network::rpc_get_overwatch_node_info(overwatch_node_id)
        }

        fn get_overwatch_nodes(
            request: network_rpc_types::PageRequest<u32>,
        ) -> Result<
            network_rpc_types::OverwatchNodesPage<AccountId>,
            network_rpc_types::NetworkQueryError,
        > {
            Network::rpc_get_overwatch_nodes(request)
        }

        fn get_effective_overwatch_signal_meta(
        ) -> network_rpc_types::EffectiveOverwatchSignalMeta {
            Network::rpc_get_effective_overwatch_signal_meta()
        }

        fn get_effective_overwatch_subnet_weight(
            subnet_id: u32,
        ) -> network_rpc_types::EffectiveOverwatchSubnetWeight {
            Network::rpc_get_effective_overwatch_subnet_weight(subnet_id)
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::{baseline, BenchmarkList};
            use frame_support::traits::StorageInfoTrait;

            use baseline::Pallet as BaselineBench;
            use frame_system_benchmarking::Pallet as SystemBench;
            use frame_system_benchmarking::extensions::Pallet as SystemExtensionsBench;
            use pallet_session_benchmarking::Pallet as SessionBench;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);

            let storage_info = AllPalletsWithSystem::storage_info();
            (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, alloc::string::String> {
            use frame_benchmarking::{baseline, BenchmarkBatch};
            use frame_support::traits::TrackedStorageKey;

            use baseline::Pallet as BaselineBench;
            use frame_system_benchmarking::Pallet as SystemBench;
            use frame_system_benchmarking::extensions::Pallet as SystemExtensionsBench;
            use pallet_session_benchmarking::Pallet as SessionBench;

            let whitelist: Vec<TrackedStorageKey> = Vec::new();

            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);
            add_benchmarks!(params, batches);
            Ok(batches)
        }
    }

    #[cfg(feature = "try-runtime")]
    impl frame_try_runtime::TryRuntime<Block> for Runtime {
        fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here. If any of the pre/post migration checks fail, we shall stop
            // right here and right now.
            let weight = Executive::try_runtime_upgrade(checks).unwrap();
            (weight, BlockWeights::get().max_block)
        }

        fn execute_block(
            block: <Block as BlockT>::LazyBlock,
            state_root_check: bool,
            signature_check: bool,
            select: frame_try_runtime::TryStateSelect
        ) -> Weight {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here.
            Executive::try_execute_block(block, state_root_check, signature_check, select).expect("execute-block failed")
        }
    }
}
