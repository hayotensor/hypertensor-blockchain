use crate::*;
use frame_support::{
    assert_noop, assert_ok,
    dispatch::{DispatchClass, GetDispatchInfo},
    traits::{Hooks, PalletInfoAccess},
    weights::WeightToFee,
};
use sp_runtime::traits::{Convert, Dispatchable};

fn signed_capacity_extrinsic(call: RuntimeCall) -> UncheckedExtrinsic {
    use pallet_revive::evm::runtime::EthExtra;
    use sp_core::Pair;
    let signer = sp_core::sr25519::Pair::from_string("//Alice", None).unwrap();
    let who = AccountId::from(signer.public());
    let mut extra = EthExtraImpl::get_eth_extension(System::account_nonce(&who), 0);
    extra.9 = Default::default();
    let payload = SignedPayload::new(call.clone(), extra.clone()).unwrap();
    let signature = payload.using_encoded(|bytes| signer.sign(bytes));
    generic::UncheckedExtrinsic::new_signed(call, who, signature.into(), extra).into()
}

fn apply_capacity_call(label: &str, call: RuntimeCall) {
    let refunds_fee = matches!(
        &call,
        RuntimeCall::Network(
            pallet_network::Call::propose_attestation { .. } | pallet_network::Call::attest { .. }
        )
    );
    let extrinsic = signed_capacity_extrinsic(call);
    let info = extrinsic.get_dispatch_info();
    let length = extrinsic.encoded_size();
    let booked = info
        .total_weight()
        .saturating_add(BlockWeights::get().get(info.class).base_extrinsic)
        .saturating_add_proof_size(length as u64);
    assert_eq!(info.class, DispatchClass::Normal);
    assert!(info.extension_weight.ref_time() > 0);
    let events_before = System::events().len();
    eprintln!("signed {label}: declared_ref_time={}, declared_proof={}, encoded_len={}, extension_ref_time={}",
        booked.ref_time(), booked.proof_size(), length, info.extension_weight.ref_time());
    assert_ok!(Executive::apply_extrinsic(extrinsic).expect("signed call fits the remaining block"));
    let actual_fee = System::events()[events_before..]
        .iter()
        .find_map(|record| match &record.event {
            RuntimeEvent::TransactionPayment(
                pallet_transaction_payment::Event::TransactionFeePaid {
                    who, actual_fee, ..
                },
            ) if who == &account("Alice") => Some(*actual_fee),
            _ => None,
        })
        .expect("signed dispatch executes the transaction-payment extension");
    if refunds_fee {
        assert_eq!(actual_fee, 0);
    } else {
        assert!(actual_fee > 0);
    }
}

fn maximum_network_slot_reservation() -> Weight {
    use pallet_network::weights::WeightInfo as _;
    type W = <Runtime as pallet_network::Config>::WeightInfo;
    let nodes = NetworkMaxSubnetNodesUpperBound::get();
    let queued = NetworkMaxRegisteredNodesUpperBound::get();
    let minimum_attestors = Network::min_identity_attestors_for_ratio(
        3,
        <Runtime as pallet_network::Config>::SuperMajorityAttestationRatio::get(),
    );
    let accepted = W::emission_step(nodes)
        .saturating_add(
            W::emission_step_accepted_queue_mutations(queued)
                .max(W::emission_step_accepted_queue_mutations_front(queued)),
        )
        .saturating_add(W::emission_step_accepted_below_min_weight_reputation(nodes))
        .saturating_add(
            W::pending_consensus_operation_enqueue().saturating_mul(u64::from(nodes) + 3),
        )
        .saturating_add(W::consensus_node_reward_snapshot().saturating_mul(u64::from(nodes)))
        .saturating_add(W::emission_step_accepted_non_attestor_reputation(
            nodes - minimum_attestors,
        ));
    let db = <Runtime as frame_system::Config>::DbWeight::get();
    let pending_slash = W::pending_node_slash_lock_read()
        .saturating_add(W::pending_node_slash_reservation_update())
        .saturating_add(W::pending_consensus_operation_enqueue());
    let settlement = accepted
        .max(W::emission_step_rejected(nodes).saturating_add(pending_slash))
        .max(W::emission_step_missing().saturating_add(pending_slash))
        .max(W::emission_step_cancelled())
        .saturating_add(W::consensus_settlement_recovery())
        .saturating_add(db.reads_writes(3, 3))
        .saturating_add(Weight::from_parts(0, 8192 + 4 * u64::from(nodes + queued)));
    let metadata_proof = 3 * u64::from(<Runtime as pallet_network::Config>::MaxVectorLength::get())
        + u64::from(<Runtime as pallet_network::Config>::MaxUrlLength::get())
        + 1_024
        + 4_096;
    W::on_initialize_base()
        .saturating_add(W::advance_overwatch_epoch_noop())
        .saturating_add(W::emission_slot_selector())
        .saturating_add(W::emission_step_selectors())
        .saturating_add(settlement)
        .saturating_add(db.reads(1))
        .saturating_add(Weight::from_parts(0, metadata_proof))
        .saturating_add(W::elect_validator(nodes))
}

#[test]
fn pending_owner_reward_uses_seven_day_expiry_and_metered_idle_payment() {
    use frame_support::traits::Currency;
    use pallet_network::weights::WeightInfo;
    type W = <Runtime as pallet_network::Config>::WeightInfo;
    assert_eq!(ConsensusOperationRetryPeriod::get(), 100_800);
    assert_eq!(ConsensusOperationRetryPeriod::get(), 7 * DAYS);
    crate::tests::npos::ext().execute_with(|| {
        System::set_block_number(1);
        let recipient = AccountId::new([96; 32]);
        drop(Balances::deposit_creating(&recipient, EXISTENTIAL_DEPOSIT));
        pallet_network::PendingConsensusOperations::<Runtime>::insert(
            0,
            pallet_network::PendingConsensusOperation {
                operation: pallet_network::ConsensusOperation::OwnerReward {
                    recipient: recipient.clone(),
                    amount: 123,
                },
                expires_at: 100_801,
            },
        );
        pallet_network::PendingConsensusOperationsQueue::<Runtime>::put(
            pallet_network::ConsensusOperationQueueState { head: 0, tail: 1 },
        );
        let db = <Runtime as frame_system::Config>::DbWeight::get();
        let limit = db
            .reads(1)
            .saturating_add(W::pending_consensus_operations_empty())
            .saturating_add(W::pending_consensus_operation_read())
            .saturating_add(
                W::pending_consensus_operation_paid()
                    .max(W::pending_consensus_operation_retry())
                    .max(W::pending_consensus_operation_expired()),
            );
        assert!(Network::on_idle(1, Weight::zero()).all_lte(Weight::zero()));
        assert_eq!(Balances::free_balance(&recipient), EXISTENTIAL_DEPOSIT);
        assert!(Network::on_idle(1, limit).all_lte(limit));
        assert_eq!(
            Balances::free_balance(&recipient),
            EXISTENTIAL_DEPOSIT + 123
        );
        assert!(!pallet_network::PendingConsensusOperationsQueue::<Runtime>::exists());
    });
}

#[test]
fn signed_extrinsics_remain_admissible_after_network_and_scheduler_reservations() {
    use pallet_network as n;
    use sp_runtime::traits::Header as _;
    use std::collections::BTreeMap;

    for scenario in ["maximum_subnet_slot", "full_network_cap", "busy_scheduler"] {
        crate::tests::npos::ext().execute_with(|| {
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
                    generic::Digest { logs: vec![DigestItem::PreRuntime(
                        sp_consensus_babe::BABE_ENGINE_ID, digest.encode(),
                    )] },
                ));
                if number < block {
                    assert_ok!(Executive::apply_extrinsic(
                        generic::UncheckedExtrinsic::new_bare(RuntimeCall::Timestamp(
                            pallet_timestamp::Call::set { now: u64::from(number) * SLOT_DURATION },
                        )).into(),
                    ).unwrap());
                    parent = Executive::finalize_block().hash();
                }
            }

            // Retain actual base/other hook charges, then reserve the complete modeled Network
            // work. This conservatively also keeps its already-charged empty-hook cost.
            let network = if scenario == "maximum_subnet_slot" {
                maximum_network_slot_reservation()
            } else {
                MaximumHooksWeight::get()
            };
            assert!(network.all_lte(MaximumHooksWeight::get()));
            System::register_extra_weight_unchecked(network, DispatchClass::Mandatory);
            if scenario == "busy_scheduler" {
                System::register_extra_weight_unchecked(
                    Perbill::from_percent(10) * MAXIMUM_BLOCK_WEIGHT,
                    DispatchClass::Mandatory,
                );
                // Reserve Scheduler's whole dynamic allowance, including its service overhead.
                let scheduler = MaximumSchedulerWeight::get();
                System::register_extra_weight_unchecked(scheduler, DispatchClass::Mandatory);
                assert_eq!(System::block_weight().total(), Perbill::from_percent(90) * MAXIMUM_BLOCK_WEIGHT);
            }
            assert_ok!(Executive::apply_extrinsic(
                generic::UncheckedExtrinsic::new_bare(RuntimeCall::Timestamp(
                    pallet_timestamp::Call::set { now: u64::from(block) * SLOT_DURATION },
                )).into(),
            ).unwrap());
            let remaining = MAXIMUM_BLOCK_WEIGHT.saturating_sub(System::block_weight().total());
            eprintln!("reserved {scenario}: network_ref_time={}, network_proof={}, after_hooks_and_timestamp_ref_time={}, after_hooks_and_timestamp_proof={}",
                network.ref_time(), network.proof_size(), remaining.ref_time(), remaining.proof_size());

            // Build a 512-node round to execute a full-size proposal through signed Executive
            // dispatch. This fixture is separate from the conservative hook reservation above.
            let subnet = 1;
            let nodes = NetworkMaxSubnetNodesUpperBound::get();
            n::SubnetSlot::<Runtime>::insert(subnet, block);
            for node in 1..=nodes {
                n::SubnetNodesData::<Runtime>::insert(subnet, node, n::SubnetNode {
                    id: node, validator_id: node,
                    classification: n::SubnetNodeClassification { node_class: n::SubnetNodeClass::Active, start_epoch: 0 },
                });
                n::SubnetNodeValidatorId::<Runtime>::insert(subnet, node, node);
                n::SubnetNodeIdHotkey::<Runtime>::insert(subnet, node, account("Alice"));
                n::ValidatorDelegateStakeBalance::<Runtime>::insert(node, TENSOR);
                n::ValidatorNodeDelegateStakeWeights::<Runtime>::insert(node, BTreeMap::from([((subnet, node), TENSOR)]));
            }
            n::SubnetNodeElectionSlots::<Runtime>::insert(subnet, (1..=nodes).collect::<Vec<_>>());
            Network::elect_validator(subnet, 0);
            let proposer = n::SubnetElectedValidator::<Runtime>::get(subnet, 0).unwrap().validator_subnet_node_id;

            let bob_before = Balances::free_balance(account("Bob"));
            apply_capacity_call("balances_transfer", pallet_balances::Call::transfer_keep_alive {
                dest: account("Bob"), value: TENSOR,
            }.into());
            assert_eq!(Balances::free_balance(account("Bob")), bob_before + TENSOR);
            apply_capacity_call("network_proposal_512", n::Call::propose_attestation {
                subnet_id: subnet,
                data: (1..=nodes).map(|subnet_node_id| n::SubnetNodeConsensusData { subnet_node_id, score: 1 }).collect(),
                prioritize_queue_node_id: None, remove_queue_node_id: None, args: None, attest_data: None,
            }.into());
            let attestor = if proposer == 1 { 2 } else { 1 };
            apply_capacity_call("network_attest", n::Call::attest {
                subnet_id: subnet, subnet_node_id: attestor, data: None,
            }.into());
            let submission = n::SubnetConsensusSubmission::<Runtime>::get(subnet, 0).unwrap();
            assert_eq!(submission.data.len(), nodes as usize);
            assert!(submission.attests.contains_key(&attestor));

            apply_capacity_call("revive_map_account", pallet_revive::Call::map_account {}.into());
            let destination = sp_core::H160::repeat_byte(0x77);
            apply_capacity_call("revive_value_transfer", pallet_revive::Call::call {
                dest: destination, value: TENSOR, data: vec![],
                weight_limit: Weight::from_parts(50 * WEIGHT_REF_TIME_PER_MILLIS, 128 * 1024),
                storage_deposit_limit: TENSOR,
            }.into());
            assert_eq!(Revive::evm_balance(&destination), TENSOR.into());
            assert!(System::block_weight().total().all_lte(MAXIMUM_BLOCK_WEIGHT));
            assert!(System::block_weight().get(DispatchClass::Normal).all_lte(BlockWeights::get().get(DispatchClass::Normal).max_total.unwrap()));
            eprintln!("{scenario}: after_signed_calls_ref_time={}, after_signed_calls_proof={}",
                MAXIMUM_BLOCK_WEIGHT.saturating_sub(System::block_weight().total()).ref_time(),
                MAXIMUM_BLOCK_WEIGHT.saturating_sub(System::block_weight().total()).proof_size());
        });
    }
}

#[test]
fn normal_extrinsic_admission_counts_hooks_in_both_weight_dimensions() {
    use sp_runtime::transaction_validity::{InvalidTransaction, TransactionValidityError};

    let limits = BlockWeights::get();
    for class in [DispatchClass::Normal, DispatchClass::Operational] {
        eprintln!("class {class:?}: max_total={:?}, max_extrinsic={:?}, reserved={:?}, base_extrinsic={:?}",
            limits.get(class).max_total, limits.get(class).max_extrinsic,
            limits.get(class).reserved, limits.get(class).base_extrinsic);
    }
    for (label, hook) in [
        ("maximum_subnet_slot", maximum_network_slot_reservation()),
        ("full_network_cap", MaximumHooksWeight::get()),
    ] {
        let whole = limits.max_block.saturating_sub(hook);
        let normal = whole.min(limits.get(DispatchClass::Normal).max_total.unwrap());
        eprintln!(
            "{label} before other hooks: whole_headroom={whole:?}, normal_headroom={normal:?}"
        );
    }

    for scenario in [
        "network_cap",
        "scheduler_cap",
        "time_exhausted",
        "proof_exhausted",
    ] {
        crate::tests::npos::ext().execute_with(|| {
            System::initialize(&1, &H256::zero(), &generic::Digest::default());
            // A real signed Revive call declaration at the Ethereum transaction weight ceiling.
            // Validation alone admits it; prepare must also enforce the current block budget.
            let extrinsic = signed_capacity_extrinsic(
                pallet_revive::Call::call {
                    dest: sp_core::H160::repeat_byte(0x78),
                    value: 0,
                    data: vec![],
                    weight_limit: Revive::evm_max_extrinsic_weight(),
                    storage_deposit_limit: TENSOR,
                }
                .into(),
            );
            let info = extrinsic.get_dispatch_info();
            assert_eq!(info.class, DispatchClass::Normal);
            let length = extrinsic.encoded_size();
            let booked = info
                .total_weight()
                .saturating_add(limits.get(info.class).base_extrinsic)
                .saturating_add_proof_size(length as u64);
            let mandatory = match scenario {
                "network_cap" => MaximumHooksWeight::get().saturating_add(limits.base_block),
                "scheduler_cap" => Perbill::from_percent(90) * limits.max_block,
                "time_exhausted" => {
                    Weight::from_parts(limits.max_block.ref_time() - booked.ref_time() + 1, 0)
                }
                _ => Weight::from_parts(0, limits.max_block.proof_size() - booked.proof_size() + 1),
            };
            System::register_extra_weight_unchecked(mandatory, DispatchClass::Mandatory);
            let (_, next_length) =
                frame_system::CheckWeight::<Runtime>::do_validate(&info, length).unwrap();
            let admission =
                frame_system::CheckWeight::<Runtime>::do_prepare(&info, length, next_length);
            if scenario == "network_cap" {
                assert_ok!(admission);
                assert!(System::block_weight().total().all_lte(limits.max_block));
            } else {
                assert_eq!(
                    admission,
                    Err(TransactionValidityError::Invalid(
                        InvalidTransaction::ExhaustsResources
                    ))
                );
                assert_eq!(System::block_weight().total(), mandatory);
            }
            eprintln!(
                "large_revive {scenario}: declared={booked:?}, admitted={}",
                scenario == "network_cap"
            );
        });
    }
}

fn account(seed: &str) -> AccountId {
    genesis::presets::get_account_id_from_seed::<sp_core::sr25519::Public>(seed)
}

#[test]
fn overwatch_genesis_has_an_explicit_empty_opening_snapshot() {
    crate::tests::npos::ext().execute_with(|| {
        let snapshot = pallet_network::OverwatchEpochSnapshots::<Runtime>::get(0).unwrap();
        assert!(snapshot.nodes.is_empty());
        assert_eq!(
            snapshot.stake_weight_factor,
            pallet_network::OverwatchStakeWeightFactor::<Runtime>::get()
        );
        assert_eq!(
            snapshot.reward_budget,
            OverwatchEpochEmissions::get().saturating_mul(
                pallet_network::ActiveOverwatchEpochLengthMultiplier::<Runtime>::get() as u128,
            )
        );
    });
}

#[test]
fn overwatch_opening_and_settlement_weights_fit_the_hook_budget() {
    use pallet_network::weights::WeightInfo as _;
    type W = <Runtime as pallet_network::Config>::WeightInfo;
    let subnets = NetworkMaxPhysicalSubnetsUpperBound::get();
    let nodes = NetworkMaxOverwatchNodesUpperBound::get();
    let settlement = W::calculate_overwatch_rewards_empty()
        .max(W::calculate_overwatch_rewards_small(subnets))
        .max(W::calculate_overwatch_rewards_medium(nodes))
        .max(W::calculate_overwatch_rewards(nodes * subnets));
    assert!(W::on_initialize_base()
        .saturating_add(W::advance_overwatch_epoch_noop())
        .saturating_add(settlement)
        .all_lte(MaximumHooksWeight::get()));
    assert!(W::on_initialize_base()
        .saturating_add(W::advance_overwatch_epoch())
        .saturating_add(W::total_subnets_selector())
        .saturating_add(W::do_epoch_preliminaries(subnets))
        .all_lte(MaximumHooksWeight::get()));
}

#[test]
fn time_weighted_subnet_allocation_fits_the_hook_budget() {
    use pallet_network::weights::WeightInfo as _;
    type W = <Runtime as pallet_network::Config>::WeightInfo;
    let subnets = NetworkMaxPhysicalSubnetsUpperBound::get();
    // Slot 2 always selects the Overwatch no-op path. Its mandatory work ends after
    // allocation; any following swap queue processing is admitted by the remaining meter.
    let allocation = W::handle_subnet_emission_weights(subnets)
        .max(W::handle_subnet_emission_weights_empty());
    let envelope = W::on_initialize_base()
        .saturating_add(W::advance_overwatch_epoch_noop())
        .saturating_add(W::total_subnets_selector())
        .saturating_add(allocation);
    let limit = MaximumHooksWeight::get();
    eprintln!(
        "slot2 allocation: subnets={subnets}, ref_time={}, proof_size={}, limit_ref_time={}, limit_proof_size={}",
        envelope.ref_time(),
        envelope.proof_size(),
        limit.ref_time(),
        limit.proof_size(),
    );
    assert!(
        envelope.all_lte(limit),
        "maximum subnet allocation must fit both hook weight dimensions"
    );
}

#[test]
fn network_reads_babe_randomness_after_consensus_rotation() {
    use frame_support::traits::Randomness as RandomnessT;
    type NetworkRandomness = <Runtime as pallet_network::Config>::Randomness;

    crate::tests::npos::ext().execute_with(|| {
        assert!(Network::index() > Babe::index());
        assert!(Network::index() > Session::index());
        assert!(Network::index() < Scheduler::index());

        let subject = b"network/provider-regression";
        pallet_babe::Randomness::<Runtime>::put([7; 32]);
        pallet_babe::EpochStart::<Runtime>::put((20, 40));
        let first = <NetworkRandomness as RandomnessT<Hash, BlockNumber>>::random(subject);
        assert_eq!(first, Randomness::random(subject));
        assert_eq!(first.1, 20);

        // Query the provider through Network's actual runtime configuration.
        pallet_babe::Randomness::<Runtime>::put([8; 32]);
        let next = <NetworkRandomness as RandomnessT<Hash, BlockNumber>>::random(subject);
        assert_ne!(next.0, first.0);
        assert_eq!(next.1, first.1);
    });
}

#[test]
fn non_transfer_proxy_cannot_escalate_or_redirect_staking_rewards() {
    crate::tests::npos::ext().execute_with(|| {
        System::set_block_number(1);
        let owner = account("Alice");
        let delegate = account("Bob");
        assert_ok!(Proxy::add_proxy(
            RuntimeOrigin::signed(owner.clone()),
            delegate.clone(),
            ProxyType::NonTransfer,
            0,
        ));
        let transfer: RuntimeCall = pallet_balances::Call::transfer_allow_death {
            dest: delegate.clone(),
            value: TENSOR,
        }
        .into();
        let calls: Vec<RuntimeCall> = vec![
            pallet_sudo::Call::sudo_as {
                who: owner.clone(),
                call: Box::new(transfer.clone()),
            }
            .into(),
            pallet_sudo::Call::sudo {
                call: Box::new(
                    pallet_balances::Call::force_transfer {
                        source: owner.clone(),
                        dest: delegate.clone(),
                        value: TENSOR,
                    }
                    .into(),
                ),
            }
            .into(),
            pallet_atomic_swap::Call::create_swap {
                target: delegate.clone(),
                hashed_proof: [1; 32],
                hash_type: pallet_atomic_swap::HashType::Blake2256,
                action: pallet_atomic_swap::BalanceSwapAction::new(TENSOR),
                duration: 10,
            }
            .into(),
            pallet_multisig::Call::as_multi_threshold_1 {
                other_signatories: vec![delegate.clone()],
                call: Box::new(transfer),
            }
            .into(),
            pallet_staking::Call::set_payee {
                payee: pallet_staking::RewardDestination::Account(delegate.clone()),
            }
            .into(),
            pallet_staking::Call::bond {
                value: TENSOR,
                payee: pallet_staking::RewardDestination::Account(delegate.clone()),
            }
            .into(),
        ];
        let before = (
            Balances::free_balance(&owner),
            Balances::free_balance(&delegate),
        );
        for call in calls {
            assert_ok!(Proxy::proxy(
                RuntimeOrigin::signed(delegate.clone()),
                owner.clone(),
                None,
                Box::new(call)
            ));
            System::assert_last_event(
                pallet_proxy::Event::ProxyExecuted {
                    result: Err(frame_system::Error::<Runtime>::CallFiltered.into()),
                }
                .into(),
            );
            assert_eq!(
                (
                    Balances::free_balance(&owner),
                    Balances::free_balance(&delegate)
                ),
                before
            );
        }
        // The restricted proxy remains useful for ordinary NPoS management.
        assert_ok!(Proxy::proxy(
            RuntimeOrigin::signed(delegate),
            owner,
            None,
            Box::new(pallet_staking::Call::chill {}.into())
        ));
        System::assert_last_event(pallet_proxy::Event::ProxyExecuted { result: Ok(()) }.into());
    });
}

#[test]
fn pausing_preserves_the_timestamp_and_root_recovery_path() {
    crate::tests::npos::ext().execute_with(|| {
        System::set_block_number(1);
        let name = |pallet: &[u8], call: &[u8]| -> RuntimeCallNameOf<Runtime> {
            (
                pallet.to_vec().try_into().unwrap(),
                call.to_vec().try_into().unwrap(),
            )
        };
        for protected in [
            name(b"Timestamp", b"set"),
            name(b"Sudo", b"sudo"),
            name(b"Sudo", b"sudo_as"),
        ] {
            assert_noop!(
                TxPause::pause(RuntimeOrigin::root(), protected),
                pallet_tx_pause::Error::<Runtime>::Unpausable
            );
        }
        let paused = name(b"Balances", b"transfer_allow_death");
        assert_ok!(TxPause::pause(RuntimeOrigin::root(), paused.clone()));
        let transfer: RuntimeCall = pallet_balances::Call::transfer_allow_death {
            dest: account("Bob"),
            value: TENSOR,
        }
        .into();
        assert!(transfer
            .clone()
            .dispatch(RuntimeOrigin::signed(account("Alice")))
            .is_err());
        let recover: RuntimeCall = pallet_sudo::Call::sudo {
            call: Box::new(pallet_tx_pause::Call::unpause { ident: paused }.into()),
        }
        .into();
        assert_ok!(recover.dispatch(RuntimeOrigin::signed(account("Alice"))));
        assert_ok!(transfer.dispatch(RuntimeOrigin::signed(account("Alice"))));
    });
}

#[test]
fn congestion_fees_recover_from_the_floor_in_both_weight_dimensions() {
    type Update = <Runtime as pallet_transaction_payment::Config>::FeeMultiplierUpdate;
    type Fees = <Runtime as pallet_transaction_payment::Config>::WeightToFee;
    crate::tests::npos::ext().execute_with(|| {
        let floor = MinimumMultiplier::get();
        let capacity = BlockWeights::get()
            .get(DispatchClass::Normal)
            .max_total
            .unwrap();
        for weight in [
            Weight::from_parts(capacity.ref_time(), 0),
            Weight::from_parts(0, capacity.proof_size()),
        ] {
            System::set_block_consumed_resources(weight, 0);
            // set_block_consumed_resources records Normal weight in the SDK.
            let increased = Update::convert(floor);
            assert!(increased > floor);
            assert!(Fees::weight_to_fee(&weight) > 0);
            System::set_block_consumed_resources(Weight::zero(), 0);
            assert!(Update::convert(increased) < increased);
            assert_eq!(Update::convert(floor), floor);
        }
        assert_eq!(
            Fees::weight_to_fee(&MAXIMUM_BLOCK_WEIGHT),
            tokenomics::STARTING_SUPPLY / 25_000_000_000
        );
        assert_eq!(
            deposit(1, 1024),
            tokenomics::POLICY.storage_item_deposit
                + 1024 * tokenomics::POLICY.storage_byte_deposit
        );
        assert_eq!(DepositBase::get(), deposit(1, 88));
    });
}

#[test]
fn scheduler_respects_prior_hooks_and_leaves_room_for_inherents() {
    crate::tests::npos::ext().execute_with(|| {
        System::set_block_number(1);
        assert!(Scheduler::index() > Revive::index());
        assert!(Scheduler::index() > Session::index());
        assert!(Scheduler::index() > Network::index());
        // Model a busy settlement / election block before Scheduler runs.
        let used = Perbill::from_percent(60) * MAXIMUM_BLOCK_WEIGHT;
        System::register_extra_weight_unchecked(used, DispatchClass::Mandatory);
        let budget = MaximumSchedulerWeight::get();
        let headroom = Perbill::from_percent(10) * MAXIMUM_BLOCK_WEIGHT;
        assert!(budget
            .saturating_add(used)
            .saturating_add(headroom)
            .all_lte(MAXIMUM_BLOCK_WEIGHT));
        let heavy: RuntimeCall = pallet_sudo::Call::sudo_unchecked_weight {
            call: Box::new(frame_system::Call::remark { remark: vec![] }.into()),
            weight: Perbill::from_percent(50) * MAXIMUM_BLOCK_WEIGHT,
        }
        .into();
        assert_ok!(Scheduler::schedule(
            RuntimeOrigin::root(),
            2,
            None,
            0,
            Box::new(heavy)
        ));
        System::set_block_number(2);
        let consumed = Scheduler::on_initialize(2);
        assert!(consumed.all_lte(budget));
        assert!(!System::events().iter().any(|event| matches!(
            event.event,
            RuntimeEvent::Scheduler(pallet_scheduler::Event::Dispatched { .. })
        )));
    });
}

#[test]
fn ethereum_extrinsics_fit_alongside_the_network_hook_budget() {
    crate::tests::npos::ext().execute_with(|| {
        let combined = Revive::evm_max_extrinsic_weight().saturating_add(MaximumHooksWeight::get());
        assert!(combined.all_lte(Perbill::from_percent(90) * MAXIMUM_BLOCK_WEIGHT));
    });
}

#[test]
fn production_staking_timings_and_genesis_are_not_development_defaults() {
    let era_millis = PRODUCTION_EPOCH_SLOTS * SLOT_DURATION * PRODUCTION_SESSIONS_PER_ERA as u64;
    assert_eq!(era_millis, 24 * 60 * 60 * 1000);
    assert_eq!(
        era_millis * BondingDuration::get() as u64,
        28 * 24 * 60 * 60 * 1000
    );
    assert!(SlashDeferDuration::get() < BondingDuration::get());
    assert!(genesis::presets::get_preset(&"HOSKINSON_RUNTIME_PRESET".into()).is_none());
}

#[test]
fn treasury_retains_funds_and_allows_payouts_beyond_the_development_window() {
    crate::tests::npos::ext().execute_with(|| {
        System::set_block_number(1);
        let treasury = Treasury::account_id();
        let beneficiary = account("Bob");
        assert_ok!(Balances::transfer_allow_death(
            RuntimeOrigin::signed(account("Alice")),
            treasury.clone(),
            100 * TENSOR,
        ));
        let funded = Balances::free_balance(&treasury);
        for block in [2, 10, TreasurySpendPeriod::get()] {
            System::set_block_number(block);
            Treasury::on_initialize(block);
            assert_eq!(Balances::free_balance(&treasury), funded);
        }
        assert_ok!(Treasury::spend(
            RuntimeOrigin::root(),
            Box::new(()),
            10 * TENSOR,
            Box::new(beneficiary.clone()),
            None,
        ));
        let before = Balances::free_balance(&beneficiary);
        System::set_block_number(System::block_number() + DAYS);
        assert_ok!(Treasury::payout(
            RuntimeOrigin::signed(beneficiary.clone()),
            0
        ));
        assert_eq!(Balances::free_balance(&beneficiary), before + 10 * TENSOR);
        assert_eq!(Balances::free_balance(&treasury), funded - 10 * TENSOR);
    });
}

#[test]
fn production_genesis_rejects_public_development_keys() {
    use genesis::presets::{authority_keys_from_seed, reject_development_keys};
    let mut base = serde_json::to_value(RuntimeGenesisConfig::default()).unwrap();
    for (key, value) in genesis::presets::local_config_genesis()
        .as_object()
        .unwrap()
    {
        base[key]
            .as_object_mut()
            .unwrap()
            .extend(value.as_object().unwrap().clone());
    }
    let mut config: RuntimeGenesisConfig = serde_json::from_value(base).unwrap();
    assert!(reject_development_keys(&config).is_err());

    // Operator-controlled replacements pass.
    let operators = [
        authority_keys_from_seed("Operator1"),
        authority_keys_from_seed("Operator2"),
    ];
    config.sudo.key = Some(account("Governance"));
    config.session.keys = operators
        .iter()
        .map(|(who, babe, grandpa)| {
            (
                who.clone(),
                who.clone(),
                opaque::SessionKeys {
                    babe: babe.clone(),
                    grandpa: grandpa.clone(),
                },
            )
        })
        .collect();
    for ((stash, controller, _, _), (who, _, _)) in
        config.staking.stakers.iter_mut().zip(&operators)
    {
        *stash = who.clone();
        *controller = who.clone();
    }
    for (i, (who, _)) in config.balances.balances.iter_mut().enumerate() {
        *who = account(&format!("Holder{i}"));
    }
    config.collective.members = vec![account("Councillor")];
    assert_ok!(reject_development_keys(&config));

    // Any single leftover development key is rejected.
    config.sudo.key = Some(account("Alice"));
    assert!(reject_development_keys(&config).is_err());
    config.sudo.key = Some(account("Governance"));
    let grandpa = core::mem::replace(
        &mut config.session.keys[0].2.grandpa,
        authority_keys_from_seed("Bob").2,
    );
    assert!(reject_development_keys(&config).is_err());
    config.session.keys[0].2.grandpa = grandpa;
    config.staking.stakers[1].0 = account("Charlie//stash");
    assert!(reject_development_keys(&config).is_err());
    config.staking.stakers[1].0 = operators[1].0.clone();
    assert_ok!(reject_development_keys(&config));
    config.balances.balances[0].0 = account("Alice");
    assert!(reject_development_keys(&config).is_err());
    config.balances.balances[0].0 = account("Holder0");
    config.collective.members.push(account("Bob"));
    assert!(reject_development_keys(&config).is_err());
}
