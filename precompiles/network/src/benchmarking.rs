//! Wrapper work only; mutating business logic uses pallet-network's dispatch weights.
//! These benchmarks include ABI decoding, conversion and return encoding. Storage fixtures use
//! runtime ceilings rather than the small happy-path fixtures used by ordinary extrinsic tests.
use crate::{
    abi, convert,
    pallet::{Config, Pallet},
    reads,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec,
};
use frame_benchmarking::v2::*;
use frame_support::{dispatch::GetDispatchInfo, traits::Get};
use pallet_network as n;
use pallet_revive::precompiles::alloy::sol_types::{SolCall, SolInterface};
use sp_runtime::AccountId32;

fn optional_bytes(len: u32) -> abi::OptionalBytes {
    abi::OptionalBytes {
        present: true,
        value: vec![1; len as usize].into(),
    }
}
fn full_identity<T: Config>() -> abi::OptionalIdentity {
    let bytes = optional_bytes(T::MaxVectorLength::get());
    let url = optional_bytes(T::MaxUrlLength::get());
    let social = optional_bytes(T::MaxSocialIdLength::get());
    abi::OptionalIdentity {
        present: true,
        value: abi::Identity {
            name: bytes.clone(),
            url: url.clone(),
            image: url.clone(),
            discord: social.clone(),
            x: social.clone(),
            telegram: social,
            github: url.clone(),
            huggingFace: url,
            description: bytes.clone(),
            misc: bytes,
        },
    }
}
fn empty_node<T: Config>() -> n::SubnetNode<T> {
    n::SubnetNode {
        id: 0,
        validator_id: 0,
        peer_info: None,
        bootnode_peer_info: None,
        client_peer_info: None,
        classification: Default::default(),
        unique: None,
        non_unique: None,
    }
}
fn full_peer<T: Config>() -> n::PeerInfo<T> {
    n::PeerInfo {
        peer_id: sp_core::OpaquePeerId(vec![b'1'; 128]),
        multiaddr: Some(
            vec![1; T::MaxVectorLength::get() as usize]
                .try_into()
                .unwrap(),
        ),
    }
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn abi_identity() {
        let input = abi::IValidators::registerValidatorCall {
            hotkey: [7; 32].into(),
            delegateRewardRate: 123,
            delegateAccount: Default::default(),
            identity: full_identity::<T>(),
        }
        .abi_encode();
        #[block]
        {
            let envelope = crate::input::MeteredInput::<abi::IValidators::IValidatorsCalls>::abi_decode_validate(&input).unwrap();
            let decoded = envelope.decode_charged().unwrap();
            assert!(crate::validators::to_call::<T>(&decoded).unwrap().is_some());
        }
    }

    #[benchmark]
    fn abi_registration() {
        let mut value = abi::SubnetRegistration::default();
        let bytes = vec![1; T::MaxVectorLength::get() as usize];
        value.name = bytes.clone().into();
        value.description = bytes.clone().into();
        value.misc = bytes.into();
        value.repo = vec![1; T::MaxUrlLength::get() as usize].into();
        value.initialValidators = (0..T::MaxRegisteredNodesUpperBound::get())
            .map(|key| abi::U32Entry { key, value: 1 })
            .collect();
        value.bootnodes = (0..T::MaxBootnodesUpperBound::get())
            .map(|i| {
                let mut peer = vec![b'1'; 128];
                peer[124..].copy_from_slice(&i.to_be_bytes());
                abi::Bootnode {
                    peerId: peer.into(),
                    multiaddr: vec![1; T::MaxVectorLength::get() as usize].into(),
                }
            })
            .collect();
        let input = abi::ISubnets::registerSubnetCall {
            maxCost: 0,
            subnetData: value,
        }
        .abi_encode();
        #[block]
        {
            let envelope =
                crate::input::MeteredInput::<abi::ISubnets::ISubnetsCalls>::abi_decode_validate(
                    &input,
                )
                .unwrap();
            let decoded = envelope.decode_charged().unwrap();
            assert!(crate::subnets::to_call::<T>(&decoded).unwrap().is_some());
        }
    }

    #[benchmark]
    fn abi_attestation() {
        let input = abi::IConsensus::proposeAttestationCall {
            subnetId: 1,
            data: (0..T::MaxSubnetNodesUpperBound::get())
                .map(|id| abi::NodeScore {
                    subnetNodeId: id,
                    score: id as u128,
                })
                .collect(),
            prioritizeQueueNodeId: Default::default(),
            removeQueueNodeId: Default::default(),
            args: optional_bytes(T::ValidatorArgsLimit::get()),
            attestData: optional_bytes(T::ValidatorArgsLimit::get()),
        }
        .abi_encode();
        #[block]
        {
            let envelope = crate::input::MeteredInput::<abi::IConsensus::IConsensusCalls>::abi_decode_validate(&input).unwrap();
            let decoded = envelope.decode_charged().unwrap();
            assert!(crate::consensus::to_call::<T>(&decoded).unwrap().is_some());
        }
    }

    #[benchmark]
    fn dispatch_weight_selection() {
        let active: BTreeSet<_> = (1..=T::MaxSubnetNodesUpperBound::get()).collect();
        let registered: BTreeSet<_> = (1..=T::MaxRegisteredNodesUpperBound::get()).collect();
        let active: frame_support::BoundedBTreeSet<_, T::MaxSubnetNodesUpperBound> =
            active.try_into().unwrap();
        n::PendingActiveNodeRemovals::<T>::insert(1, active);
        let registered: frame_support::BoundedBTreeSet<_, T::MaxRegisteredNodesUpperBound> =
            registered.try_into().unwrap();
        n::PendingRegisteredNodeRemovals::<T>::insert(1, registered);
        n::SubnetNodeValidatorId::<T>::insert(1, 1, 1);
        n::TotalValidatorNodes::<T>::insert(1, T::MaxValidatorNodesUpperBound::get());
        n::TotalSubnetElectableNodes::<T>::insert(1, T::MaxSubnetNodesUpperBound::get());
        n::SubnetNodesData::<T>::insert(1, 1, empty_node::<T>());
        let call = n::Call::<T>::remove_subnet_node {
            subnet_id: 1,
            subnet_node_id: 1,
        };
        #[block]
        {
            core::hint::black_box(call.get_dispatch_info());
        }
    }

    #[benchmark]
    fn staking_read() {
        let account = AccountId32::new([7; 32]);
        let ledger: BTreeMap<_, _> = (0..T::MaxUnbondingsUpperBound::get())
            .map(|i| {
                (
                    i,
                    n::UnbondingEntry {
                        network: 1,
                        overwatch: 1,
                    },
                )
            })
            .collect();
        n::StakeUnbondingLedger::<T>::insert(&account, ledger);
        let input = abi::IStaking::IStakingCalls::getUnbondings(abi::IStaking::getUnbondingsCall {
            accountId: convert::account_out(account),
        });
        let mut setup = pallet_revive::call_builder::CallSetup::<T>::new(
            pallet_revive::call_builder::VmBinaryModule::evm_init_code_for_runtime_size(1),
        );
        let (mut ext, _) = setup.ext();
        let output;
        #[block]
        {
            output = reads::staking::<T>(&input, &mut ext).unwrap();
        }
        let decoded = abi::IStaking::getUnbondingsCall::abi_decode_returns(&output).unwrap();
        assert_eq!(decoded.len(), T::MaxUnbondingsUpperBound::get() as usize);
    }

    #[benchmark]
    fn validator_read() {
        let coldkey = AccountId32::new([7; 32]);
        let hotkey = AccountId32::new([8; 32]);
        n::Pallet::<T>::do_register_validator(
            frame_system::RawOrigin::Signed(coldkey.clone()).into(),
            hotkey,
            0,
            None,
            convert::identity::<T>(&full_identity::<T>()).unwrap(),
        )
        .unwrap();
        let input = abi::IValidators::IValidatorsCalls::getValidatorByColdkey(
            abi::IValidators::getValidatorByColdkeyCall {
                accountId: convert::account_out(coldkey),
            },
        );
        let mut setup = pallet_revive::call_builder::CallSetup::<T>::new(
            pallet_revive::call_builder::VmBinaryModule::evm_init_code_for_runtime_size(1),
        );
        let (mut ext, _) = setup.ext();
        let output;
        #[block]
        {
            output = reads::validators::<T>(&input, &mut ext).unwrap();
        }
        let decoded =
            abi::IValidators::getValidatorByColdkeyCall::abi_decode_returns(&output).unwrap();
        assert!(decoded.exists);
        assert_eq!(
            decoded.info.identity.value.name.value.len(),
            T::MaxVectorLength::get() as usize
        );
    }

    #[benchmark]
    fn allocation_read() {
        let allocations: BTreeMap<_, _> = (0..T::MaxValidatorNodesUpperBound::get())
            .map(|id| ((1, id), 1u128))
            .collect();
        n::ValidatorNodeDelegateStakeWeights::<T>::insert(1, allocations);
        let input = abi::IValidators::IValidatorsCalls::getNodeAllocation(
            abi::IValidators::getNodeAllocationCall {
                validatorId: 1,
                subnetId: 1,
                subnetNodeId: T::MaxValidatorNodesUpperBound::get().saturating_sub(1),
            },
        );
        let mut setup = pallet_revive::call_builder::CallSetup::<T>::new(
            pallet_revive::call_builder::VmBinaryModule::evm_init_code_for_runtime_size(1),
        );
        let (mut ext, _) = setup.ext();
        let output;
        #[block]
        {
            output = reads::validators::<T>(&input, &mut ext).unwrap();
        }
        let decoded = abi::IValidators::getNodeAllocationCall::abi_decode_returns(&output).unwrap();
        assert!(decoded.exists);
        assert_eq!(decoded.weight, 1);
    }

    #[benchmark]
    fn subnet_read() {
        let bytes = vec![1; T::MaxVectorLength::get() as usize];
        n::SubnetsData::<T>::insert(
            1,
            n::SubnetData {
                id: 1,
                name: bytes.clone(),
                repo: vec![1; T::MaxUrlLength::get() as usize],
                description: bytes.clone(),
                misc: bytes,
                ..Default::default()
            },
        );
        let input =
            abi::ISubnets::ISubnetsCalls::getSubnet(abi::ISubnets::getSubnetCall { subnetId: 1 });
        let mut setup = pallet_revive::call_builder::CallSetup::<T>::new(
            pallet_revive::call_builder::VmBinaryModule::evm_init_code_for_runtime_size(1),
        );
        let (mut ext, _) = setup.ext();
        let output;
        #[block]
        {
            output = reads::subnets::<T>(&input, &mut ext).unwrap();
        }
        let decoded = abi::ISubnets::getSubnetCall::abi_decode_returns(&output).unwrap();
        assert!(decoded.exists);
        assert_eq!(decoded.info.name.len(), T::MaxVectorLength::get() as usize);
    }

    #[benchmark]
    fn node_read() {
        let coldkey = AccountId32::new([7; 32]);
        let hotkey = AccountId32::new([8; 32]);
        n::Pallet::<T>::do_register_validator(
            frame_system::RawOrigin::Signed(coldkey.clone()).into(),
            hotkey,
            0,
            None,
            None,
        )
        .unwrap();
        let validator_id = n::ColdkeyValidatorId::<T>::get(coldkey).unwrap();
        n::SubnetNodeValidatorId::<T>::insert(1, 1, validator_id);
        let mut node = empty_node::<T>();
        node.id = 1;
        node.validator_id = validator_id;
        node.peer_info = Some(full_peer::<T>());
        node.bootnode_peer_info = Some(full_peer::<T>());
        node.client_peer_info = Some(full_peer::<T>());
        node.unique = Some(
            vec![1; T::MaxVectorLength::get() as usize]
                .try_into()
                .unwrap(),
        );
        node.non_unique = node.unique.clone();
        n::SubnetNodesData::<T>::insert(1, 1, node);
        let input = abi::ISubnetNodes::ISubnetNodesCalls::getSubnetNode(
            abi::ISubnetNodes::getSubnetNodeCall {
                subnetId: 1,
                subnetNodeId: 1,
            },
        );
        let mut setup = pallet_revive::call_builder::CallSetup::<T>::new(
            pallet_revive::call_builder::VmBinaryModule::evm_init_code_for_runtime_size(1),
        );
        let (mut ext, _) = setup.ext();
        let output;
        #[block]
        {
            output = reads::subnet_nodes::<T>(&input, &mut ext).unwrap();
        }
        let decoded = abi::ISubnetNodes::getSubnetNodeCall::abi_decode_returns(&output).unwrap();
        assert!(decoded.exists);
        assert_eq!(
            decoded.info.unique.value.len(),
            T::MaxVectorLength::get() as usize
        );
    }

    #[benchmark]
    fn consensus_read() {
        let mut round = n::ElectedConsensusRound::default();
        round.eligible_subnet_node_ids = (0..T::MaxSubnetNodesUpperBound::get()).collect();
        round.eligible_validator_identity_ids = (0..T::MaxSubnetNodesUpperBound::get())
            .map(|i| (i, i))
            .collect();
        n::SubnetElectedValidator::<T>::insert(1, 1, round);
        let input = abi::IConsensus::IConsensusCalls::getConsensusRound(
            abi::IConsensus::getConsensusRoundCall {
                subnetId: 1,
                subnetEpoch: 1,
            },
        );
        let mut setup = pallet_revive::call_builder::CallSetup::<T>::new(
            pallet_revive::call_builder::VmBinaryModule::evm_init_code_for_runtime_size(1),
        );
        let (mut ext, _) = setup.ext();
        let output;
        #[block]
        {
            output = reads::consensus::<T>(&input, &mut ext).unwrap();
        }
        let decoded = abi::IConsensus::getConsensusRoundCall::abi_decode_returns(&output).unwrap();
        assert!(decoded.exists);
    }

    #[benchmark]
    fn overwatch_read() {
        let commits: BTreeMap<_, _> = (0..T::MaxPhysicalSubnetsUpperBound::get())
            .map(|i| (i, sp_core::H256::repeat_byte(1)))
            .collect();
        let commits: frame_support::BoundedBTreeMap<_, _, T::MaxPhysicalSubnetsUpperBound> =
            commits.try_into().unwrap();
        n::OverwatchCommits::<T>::insert(0, 1, commits);
        let input =
            abi::IOverwatch::IOverwatchCalls::getCommitment(abi::IOverwatch::getCommitmentCall {
                overwatchNodeId: 1,
                subnetId: T::MaxPhysicalSubnetsUpperBound::get().saturating_sub(1),
            });
        let mut setup = pallet_revive::call_builder::CallSetup::<T>::new(
            pallet_revive::call_builder::VmBinaryModule::evm_init_code_for_runtime_size(1),
        );
        let (mut ext, _) = setup.ext();
        let output;
        #[block]
        {
            output = reads::overwatch::<T>(&input, &mut ext).unwrap();
        }
        let decoded = abi::IOverwatch::getCommitmentCall::abi_decode_returns(&output).unwrap();
        assert!(decoded.exists);
    }

    /// Runs the same FRAME benchmark bodies against a caller-supplied recording implementation.
    /// Used by the runtime integration suite with its production bounds.
    pub fn run_case<T: Config>(
        name: &str,
        recording: &mut impl frame_benchmarking::Recording,
    ) -> Result<(), frame_benchmarking::BenchmarkError> {
        match name {
            "abi_identity" => <abi_identity as frame_benchmarking::BenchmarkingSetup<T>>::instance(
                &abi_identity,
                recording,
                &[],
                true,
            ),
            "abi_registration" => {
                <abi_registration as frame_benchmarking::BenchmarkingSetup<T>>::instance(
                    &abi_registration,
                    recording,
                    &[],
                    true,
                )
            }
            "abi_attestation" => {
                <abi_attestation as frame_benchmarking::BenchmarkingSetup<T>>::instance(
                    &abi_attestation,
                    recording,
                    &[],
                    true,
                )
            }
            "dispatch_weight_selection" => {
                <dispatch_weight_selection as frame_benchmarking::BenchmarkingSetup<T>>::instance(
                    &dispatch_weight_selection,
                    recording,
                    &[],
                    true,
                )
            }
            "staking_read" => <staking_read as frame_benchmarking::BenchmarkingSetup<T>>::instance(
                &staking_read,
                recording,
                &[],
                true,
            ),
            "validator_read" => {
                <validator_read as frame_benchmarking::BenchmarkingSetup<T>>::instance(
                    &validator_read,
                    recording,
                    &[],
                    true,
                )
            }
            "allocation_read" => {
                <allocation_read as frame_benchmarking::BenchmarkingSetup<T>>::instance(
                    &allocation_read,
                    recording,
                    &[],
                    true,
                )
            }
            "subnet_read" => <subnet_read as frame_benchmarking::BenchmarkingSetup<T>>::instance(
                &subnet_read,
                recording,
                &[],
                true,
            ),
            "node_read" => <node_read as frame_benchmarking::BenchmarkingSetup<T>>::instance(
                &node_read,
                recording,
                &[],
                true,
            ),
            "consensus_read" => {
                <consensus_read as frame_benchmarking::BenchmarkingSetup<T>>::instance(
                    &consensus_read,
                    recording,
                    &[],
                    true,
                )
            }
            "overwatch_read" => {
                <overwatch_read as frame_benchmarking::BenchmarkingSetup<T>>::instance(
                    &overwatch_read,
                    recording,
                    &[],
                    true,
                )
            }
            _ => Err("Unknown network precompile benchmark".into()),
        }
    }
}
pub use benchmarks::run_case;
