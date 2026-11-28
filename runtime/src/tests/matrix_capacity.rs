//! Signed-transaction overhead used alongside the opt-in Matrix Wasm capacity probes.
use crate::*;
use frame_support::{
    assert_ok,
    dispatch::{DispatchClass, GetDispatchInfo},
    traits::{Get, Hooks},
};
use pallet_revive::evm::runtime::EthExtra;

#[test]
fn matrix_signed_transaction_overhead() {
    super::npos::ext().execute_with(|| {
        let limits = BlockWeights::get();
        for (name, weight) in [
            ("block", limits.max_block),
            ("network_hook", MaximumHooksWeight::get()),
            (
                "normal",
                limits.get(DispatchClass::Normal).max_total.unwrap(),
            ),
            ("base_block", limits.base_block),
        ] {
            println!(
                "matrix_weight_limit,{name},{},{}",
                weight.ref_time(),
                weight.proof_size()
            );
        }
        println!(
            "matrix_normal_length,{}",
            BlockLength::get().max.get(DispatchClass::Normal)
        );
        let scores: pallet_network::MatrixScores = (2..=64)
            .map(|subnet_node_id| pallet_network::SubnetNodeConsensusData {
                subnet_node_id,
                score: pallet_network::PERCENTAGE_FACTOR_U128,
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let calls = [
            (
                "commit",
                pallet_network::Call::commit_matrix_scores {
                    subnet_id: 128001,
                    subnet_epoch: 100,
                    subnet_node_id: 1,
                    commitment: Default::default(),
                },
            ),
            (
                "reveal",
                pallet_network::Call::reveal_matrix_scores {
                    subnet_id: 128001,
                    subnet_epoch: 100,
                    subnet_node_id: 1,
                    scores,
                    salt: [27; 32],
                },
            ),
        ];
        for (name, call) in calls {
            let call = RuntimeCall::Network(call);
            let mut extra = EthExtraImpl::get_eth_extension(0, 0);
            extra.9 = Default::default();
            // Encoding/declared-weight inspection only; this signature is never dispatched.
            let signature: Signature = sp_core::sr25519::Signature::from_raw([0; 64]).into();
            let extrinsic: UncheckedExtrinsic = generic::UncheckedExtrinsic::new_signed(
                call,
                AccountId::new([1; 32]),
                signature,
                extra,
            )
            .into();
            let info = extrinsic.get_dispatch_info();
            let overhead = info
                .extension_weight
                .saturating_add(BlockWeights::get().get(info.class).base_extrinsic);
            assert!(overhead.ref_time() > 0);
            println!(
                "matrix_signed_overhead,{name},{},{},{}",
                overhead.ref_time(),
                overhead.proof_size(),
                extrinsic.encoded_size()
            );
            let booked = info
                .total_weight()
                .saturating_add(limits.get(info.class).base_extrinsic)
                .saturating_add_proof_size(extrinsic.encoded_size() as u64);
            println!(
                "matrix_current_declared,{name},{},{},{}",
                booked.ref_time(),
                booked.proof_size(),
                extrinsic.encoded_size()
            );
        }
    });
}

/// Exercise actual signed admission with today's provisional Matrix weights.
/// This measures charged weight, independently of the Wasm execution benchmarks.
#[test]
fn matrix_current_signed_block_capacity() {
    use pallet_network as n;
    use sp_core::Pair;
    use sp_runtime::{
        traits::Header as _,
        transaction_validity::{InvalidTransaction, TransactionValidityError},
    };

    for reveal in [false, true] {
        super::npos::ext().execute_with(|| {
            let signer = sp_core::sr25519::Pair::from_string("//Alice", None).unwrap();
            let who = AccountId::from(signer.public());
            let block = n::NETWORK_DESIGNATED_EPOCH_SLOTS;
            let mut parent = H256::repeat_byte(1);
            for number in 1..=block {
                let digest = sp_consensus_babe::digests::PreDigest::SecondaryPlain(
                    sp_consensus_babe::digests::SecondaryPlainPreDigest {
                        authority_index: 0,
                        slot: u64::from(number).into(),
                    },
                );
                Executive::initialize_block(&Header::new(
                    number,
                    H256::zero(),
                    H256::zero(),
                    parent,
                    generic::Digest {
                        logs: vec![DigestItem::PreRuntime(
                            sp_consensus_babe::BABE_ENGINE_ID,
                            digest.encode(),
                        )],
                    },
                ));
                assert_ok!(Executive::apply_extrinsic(
                    generic::UncheckedExtrinsic::new_bare(RuntimeCall::Timestamp(
                        pallet_timestamp::Call::set {
                            now: u64::from(number) * SLOT_DURATION
                        },
                    ))
                    .into(),
                )
                .unwrap());
                if number < block {
                    parent = Executive::finalize_block().hash();
                }
            }
            // Retain actual base/hooks/inherent charges and reserve the complete Network cap.
            System::register_extra_weight_unchecked(
                MaximumHooksWeight::get(),
                DispatchClass::Mandatory,
            );
            let before = System::block_weight().total();
            let subnet = 1;
            let nodes = n::MATRIX_MAX_NODES;
            n::SubnetsData::<Runtime>::insert(
                subnet,
                n::SubnetData {
                    id: subnet,
                    consensus_mechanism: n::ConsensusMechanism::Matrix,
                    state: n::SubnetState::Active,
                    consensus_eligible_from_subnet_epoch: Some(0),
                    ..Default::default()
                },
            );
            n::SubnetSlot::<Runtime>::insert(subnet, block);
            let members: Vec<_> = (1..=nodes)
                .map(|id| n::SubnetNode {
                    id,
                    validator_id: id,
                    classification: n::SubnetNodeClassification {
                        node_class: n::SubnetNodeClass::Active,
                        start_epoch: 0,
                    },
                })
                .collect();
            let round = n::MatrixRound {
                start_block: 0,
                reveal_block: if reveal { block } else { block + 10 },
                end_block: block + 20,
                nodes: members.clone(),
                identities: (1..=nodes).map(|id| (id, id)).collect(),
                stake: n::ConsensusAttestorWeightSnapshot {
                    weights: (1..=nodes).map(|id| (id, 1)).collect(),
                    total_weight: nodes.into(),
                },
                policy: Default::default(),
                parameters: n::MatrixParameters {
                    accuracy_weight: n::PERCENTAGE_FACTOR_U128 / 2,
                    ..n::DefaultMatrixConfig::<Runtime>::get()
                },
            };
            n::MatrixRounds::<Runtime>::insert(subnet, 0, &round);
            for node in members {
                n::SubnetNodesData::<Runtime>::insert(subnet, node.id, &node);
                n::SubnetNodeValidatorId::<Runtime>::insert(subnet, node.id, node.validator_id);
                n::SubnetNodeIdHotkey::<Runtime>::insert(subnet, node.id, &who);
            }
            let mut accepted = 0;
            let mut rejected = false;
            for node in 1..=nodes {
                let scores: n::MatrixScores = (1..=nodes)
                    .filter(|id| *id != node)
                    .map(|subnet_node_id| n::SubnetNodeConsensusData {
                        subnet_node_id,
                        score: n::PERCENTAGE_FACTOR_U128 / 2,
                    })
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap();
                let commitment =
                    Network::matrix_commitment(subnet, 0, node, &round, &scores, &[27; 32]);
                let call = if reveal {
                    n::MatrixCommitments::<Runtime>::insert((subnet, 0, node), commitment);
                    n::Call::reveal_matrix_scores {
                        subnet_id: subnet,
                        subnet_epoch: 0,
                        subnet_node_id: node,
                        scores,
                        salt: [27; 32],
                    }
                } else {
                    n::Call::commit_matrix_scores {
                        subnet_id: subnet,
                        subnet_epoch: 0,
                        subnet_node_id: node,
                        commitment,
                    }
                };
                let call = RuntimeCall::Network(call);
                let mut extra = EthExtraImpl::get_eth_extension(System::account_nonce(&who), 0);
                extra.9 = Default::default();
                let payload = SignedPayload::new(call.clone(), extra.clone()).unwrap();
                let signature = payload.using_encoded(|bytes| signer.sign(bytes));
                let extrinsic: UncheckedExtrinsic = generic::UncheckedExtrinsic::new_signed(
                    call,
                    who.clone(),
                    signature.into(),
                    extra,
                )
                .into();
                // A block builder rolls back runtime API storage changes when it rejects
                // an invalid transaction. Reproduce that boundary for direct Executive calls.
                let result =
                    frame_support::storage::transactional::with_transaction_opaque_err(|| {
                        let result = Executive::apply_extrinsic(extrinsic);
                        if result.is_ok() {
                            frame_support::storage::TransactionOutcome::Commit(result)
                        } else {
                            frame_support::storage::TransactionOutcome::Rollback(result)
                        }
                    })
                    .expect("test transaction layer is available");
                match result {
                    Ok(result) => {
                        assert_ok!(result);
                        accepted += 1;
                    }
                    Err(TransactionValidityError::Invalid(
                        InvalidTransaction::ExhaustsResources,
                    )) => {
                        assert!(!n::MatrixRows::<Runtime>::contains_key((subnet, 0, node)));
                        if !reveal {
                            assert!(!n::MatrixCommitments::<Runtime>::contains_key((
                                subnet, 0, node
                            )));
                        }
                        rejected = true;
                        break;
                    }
                    Err(error) => panic!("unexpected signed Matrix failure: {error:?}"),
                }
            }
            assert!(
                accepted > 0 && rejected,
                "must reach the actual weight limit"
            );
            let after = System::block_weight().total();
            let normal = *System::block_weight().get(DispatchClass::Normal);
            assert!(after.all_lte(BlockWeights::get().max_block));
            assert!(normal.all_lte(
                BlockWeights::get()
                    .get(DispatchClass::Normal)
                    .max_total
                    .unwrap()
            ));
            assert_eq!(System::account_nonce(&who), accepted);
            assert_eq!(
                if reveal {
                    n::MatrixRows::<Runtime>::iter_prefix((subnet, 0)).count()
                } else {
                    n::MatrixCommitments::<Runtime>::iter_prefix((subnet, 0)).count()
                },
                accepted as usize
            );
            println!(
                "matrix_signed_block,{},{accepted},{},{},{},{}",
                if reveal { "reveal" } else { "commit" },
                before.ref_time(),
                before.proof_size(),
                after.ref_time(),
                after.proof_size()
            );
            if reveal {
                n::SlotAssignment::<Runtime>::insert(block, subnet);
                n::TotalSubnetNodes::<Runtime>::insert(subnet, nodes);
                n::TotalAdmittedSubnetNodes::<Runtime>::insert(subnet, nodes);
                n::SubnetNodeElectionSlots::<Runtime>::insert(
                    subnet,
                    (1..=nodes).collect::<Vec<_>>(),
                );
                let next_block = block + <Runtime as n::Config>::EpochLength::get();
                System::set_block_number(next_block);
                let charged = Network::on_initialize(next_block);
                assert!(charged.all_lte(MaximumHooksWeight::get()));
                assert_eq!(
                    n::MatrixResults::<Runtime>::get(subnet, 0).unwrap().outcome,
                    n::MatrixOutcome::Accepted
                );
                assert_eq!(
                    n::MatrixRounds::<Runtime>::get(subnet, 1)
                        .unwrap()
                        .identities
                        .len(),
                    nodes as usize
                );
                // Fixed reservation plus empty-queue maintenance, not a timed Wasm sample.
                println!(
                    "matrix_current_hook_charge,{nodes},{},{}",
                    charged.ref_time(),
                    charged.proof_size()
                );
            }
        });
    }
}
