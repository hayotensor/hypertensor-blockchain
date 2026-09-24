//! Tests enter through the real Revive engine and runtime dispatch filter.
use super::*;
use network_precompiles::{abi, addresses, convert};
use pallet_network as n;
use pallet_revive::{precompiles::alloy::sol_types::SolCall, TransactionLimits};

fn address(id: u16) -> H160 {
    H160(addresses::address(id))
}
fn setup() -> AccountId {
    initialize_block();
    map_alice();
    AccountId::from(alice().public())
}
fn read<C: SolCall>(id: u16, call: C) -> C::Return {
    let result = Revive::bare_call(
        RuntimeOrigin::signed(AccountId::from(alice().public())),
        address(id),
        U256::zero(),
        TransactionLimits::WeightAndDeposit {
            weight_limit: CONTRACT_WEIGHT,
            deposit_limit: TENSOR,
        },
        call.abi_encode(),
        &Default::default(),
    );
    let output = result.result.expect("query executes");
    assert!(!output.did_revert(), "query reverted: {:?}", output.data);
    C::abi_decode_returns(&output.data).expect("valid Solidity return ABI")
}
fn registration(hotkey: u8) -> abi::IValidators::registerValidatorCall {
    abi::IValidators::registerValidatorCall {
        hotkey: [hotkey; 32].into(),
        delegateRewardRate: 0,
        delegateAccount: Default::default(),
        identity: Default::default(),
    }
}
fn invoke<C: SolCall>(id: u16, call: C) -> sp_runtime::DispatchOutcome {
    native_call(contract_call(address(id), 0, call.abi_encode()))
}

#[test]
fn direct_native_and_precompile_registration_have_identical_network_state_and_events() {
    let execute = |precompile: bool| {
        npos_tests::ext().execute_with(|| {
            let who = setup();
            if precompile {
                assert_ok!(invoke(addresses::VALIDATORS, registration(42)));
            } else {
                assert_ok!(Network::register_validator(
                    RuntimeOrigin::signed(who.clone()),
                    AccountId::new([42; 32]),
                    0,
                    None,
                    None
                ));
            }
            let id = n::ColdkeyValidatorId::<Runtime>::get(&who).unwrap();
            let events: Vec<_> = System::events()
                .into_iter()
                .filter_map(|e| match e.event {
                    RuntimeEvent::Network(event) => Some(event.encode()),
                    _ => None,
                })
                .collect();
            (
                id,
                n::ValidatorsData::<Runtime>::get(id).encode(),
                n::ValidatorColdkey::<Runtime>::get(id),
                events,
            )
        })
    };
    assert_eq!(execute(false), execute(true));
}

/// Forward calldata to a precompile, propagating revert data. No Solidity compiler required.
fn forwarder(id: u16, opcode: u8, revert_after: bool) -> Vec<u8> {
    let mut code = vec![
        0x36, 0x60, 0, 0x60, 0, 0x37, 0x60, 0, 0x60, 0, 0x36, 0x60, 0,
    ];
    if opcode == 0xf1 {
        code.extend([0x60, 0]);
    }
    code.push(0x73);
    code.extend(addresses::address(id));
    code.extend([0x5a, opcode, 0x15, 0x60, 0, 0x57]);
    let jump_pos = code.len() - 2;
    code.extend([
        0x3d,
        0x60,
        0,
        0x60,
        0,
        0x3e,
        0x3d,
        0x60,
        0,
        if revert_after { 0xfd } else { 0xf3 },
    ]);
    code[jump_pos] = code.len() as u8;
    code.extend([0x5b, 0x3d, 0x60, 0, 0x60, 0, 0x3e, 0x3d, 0x60, 0, 0xfd]);
    evm_init(&code)
}

#[test]
fn contract_is_the_network_caller_and_outer_reverts_undo_network_changes() {
    for revert_after in [false, true] {
        npos_tests::ext().execute_with(|| {
            let who = setup();
            let contract = deploy(forwarder(addresses::VALIDATORS, 0xf1, revert_after));
            let account = Mapper::to_account_id(&contract);
            let result = native_call(contract_call(contract, 0, registration(43).abi_encode()));
            assert_eq!(result.is_ok(), !revert_after);
            assert_eq!(
                n::ColdkeyValidatorId::<Runtime>::contains_key(&account),
                !revert_after
            );
            assert!(!n::ColdkeyValidatorId::<Runtime>::contains_key(&who));
            assert_eq!(
                n::HotkeyValidatorId::<Runtime>::contains_key(AccountId::new([43; 32])),
                !revert_after
            );
        });
    }
}

#[test]
fn static_delegate_nonpayable_malformed_and_unknown_calls_cannot_mutate() {
    npos_tests::ext().execute_with(|| {
        let who = setup();
        for opcode in [0xfa, 0xf4] {
            let contract = deploy(forwarder(addresses::VALIDATORS, opcode, false));
            assert!(
                native_call(contract_call(contract, 0, registration(44).abi_encode())).is_err()
            );
            assert!(!n::ColdkeyValidatorId::<Runtime>::contains_key(
                Mapper::to_account_id(&contract)
            ));
        }
        assert!(native_call(contract_call(
            address(addresses::VALIDATORS),
            TENSOR,
            registration(44).abi_encode()
        ))
        .is_err());
        for data in [
            vec![],
            vec![0xff; 4],
            registration(44).abi_encode()[..15].to_vec(),
        ] {
            assert!(native_call(contract_call(address(addresses::VALIDATORS), 0, data)).is_err());
        }
        assert!(!n::ColdkeyValidatorId::<Runtime>::contains_key(who));
    });
}

#[test]
fn both_pause_mechanisms_and_low_gas_block_network_dispatch() {
    npos_tests::ext().execute_with(|| {
        let who = setup();
        n::TxPause::<Runtime>::put(true);
        assert!(invoke(addresses::VALIDATORS, registration(45)).is_err());
        n::TxPause::<Runtime>::put(false);
        assert_ok!(TxPause::pause(
            RuntimeOrigin::root(),
            (
                b"Network".to_vec().try_into().unwrap(),
                b"register_validator".to_vec().try_into().unwrap()
            )
        ));
        assert!(invoke(addresses::VALIDATORS, registration(45)).is_err());
        assert_ok!(TxPause::unpause(
            RuntimeOrigin::root(),
            (
                b"Network".to_vec().try_into().unwrap(),
                b"register_validator".to_vec().try_into().unwrap()
            )
        ));
        let result = Revive::bare_call(
            RuntimeOrigin::signed(who.clone()),
            address(addresses::VALIDATORS),
            U256::zero(),
            TransactionLimits::WeightAndDeposit {
                weight_limit: Weight::from_parts(1, 1),
                deposit_limit: TENSOR,
            },
            registration(45).abi_encode(),
            &Default::default(),
        );
        assert!(result.result.is_err());
        assert!(!n::ColdkeyValidatorId::<Runtime>::contains_key(who));
        assert_ok!(invoke(addresses::VALIDATORS, registration(45)));
    });
}

#[test]
fn read_abis_preserve_missing_records_retained_stake_and_pool_generations() {
    npos_tests::ext().execute_with(|| {
        let who = setup();
        assert!(
            !read(
                addresses::VALIDATORS,
                abi::IValidators::getValidatorCall { validatorId: 42 }
            )
            .exists
        );
        assert!(
            !read(
                addresses::SUBNETS,
                abi::ISubnets::getSubnetCall { subnetId: 42 }
            )
            .exists
        );
        assert!(
            !read(
                addresses::SUBNET_NODES,
                abi::ISubnetNodes::getSubnetNodeCall {
                    subnetId: 42,
                    subnetNodeId: 7
                }
            )
            .exists
        );
        assert!(
            !read(
                addresses::CONSENSUS,
                abi::IConsensus::getConsensusRoundCall {
                    subnetId: 42,
                    subnetEpoch: 0
                }
            )
            .exists
        );
        assert!(
            !read(
                addresses::CONSENSUS,
                abi::IConsensus::getEpochStatusCall { subnetId: 42 }
            )
            .exists
        );
        assert!(
            !read(
                addresses::OVERWATCH,
                abi::IOverwatch::getOverwatchNodeCall {
                    overwatchNodeId: 42
                }
            )
            .exists
        );
        n::NodeSubnetStake::<Runtime>::insert(7, 42, 123u128);
        assert_eq!(
            read(
                addresses::STAKING,
                abi::IStaking::getNodeStakeCall {
                    subnetId: 42,
                    subnetNodeId: 7
                }
            ),
            123
        );
        n::AccountSubnetDelegateStakeShares::<Runtime>::insert(&who, 42, 987u128);
        n::AccountSubnetDelegateStakeGeneration::<Runtime>::insert(&who, 42, 1u64);
        n::SubnetDelegatePoolGeneration::<Runtime>::insert(42, 2u64);
        let call = abi::IStaking::getSubnetPoolCall {
            subnetId: 42,
            accountId: convert::account_out(who.clone()),
        };
        assert_eq!(read(addresses::STAKING, call.clone()).accountShares, 0);
        n::AccountSubnetDelegateStakeGeneration::<Runtime>::insert(&who, 42, 2u64);
        assert_eq!(read(addresses::STAKING, call).accountShares, 987);
        assert_ok!(invoke(addresses::VALIDATORS, registration(46)));
        let info = read(
            addresses::VALIDATORS,
            abi::IValidators::getValidatorByColdkeyCall {
                accountId: convert::account_out(who.clone()),
            },
        );
        assert!(info.exists);
        assert_eq!(info.info.coldkey, convert::account_out(who));
    });
}

#[test]
fn collection_limits_and_duplicate_maps_fail_before_dispatch() {
    assert!(convert::validators::<Runtime>(&[
        abi::U32Entry { key: 1, value: 2 },
        abi::U32Entry { key: 1, value: 3 }
    ])
    .is_err());
    assert!(
        convert::metadata::<Runtime>(&vec![0; NetworkMaxVectorLength::get() as usize + 1]).is_err()
    );
    assert!(convert::emergency_set::<Runtime>(&vec![
        1;
        NetworkMaxEmergencySubnetNodesUpperBound::get()
            as usize
            + 1
    ])
    .is_err());
    assert!(convert::validator_args::<Runtime>(&abi::OptionalBytes {
        present: true,
        value: vec![0; NetworkValidatorArgsLimit::get() as usize + 1].into()
    })
    .is_err());
}

#[test]
fn pvm_contract_can_call_a_network_precompile_as_its_own_account() {
    use polkavm_common::{
        program::{asm, InstructionSetKind, Reg},
        writer::ProgramBlobBuilder,
    };
    npos_tests::ext().execute_with(|| {
        setup();
        let input = registration(47).abi_encode();
        let base = 0x10000u64; // PolkaVM's read-only data starts after its guard page.
        let mut data = addresses::address(addresses::VALIDATORS).to_vec();
        data.extend([0xff; 32]);
        data.extend([0; 32]);
        data.extend(&input);
        let mut blob = ProgramBlobBuilder::new(InstructionSetKind::ReviveV1);
        blob.add_import(b"call");
        blob.add_export_by_basic_block(0, b"deploy");
        blob.add_export_by_basic_block(1, b"call");
        blob.set_ro_data_size(data.len() as u32);
        blob.set_ro_data(data);
        blob.set_code(
            &[
                asm::ret(),
                asm::load_imm64(Reg::A0, base),
                asm::load_imm64(Reg::A1, u64::MAX),
                asm::load_imm64(Reg::A2, u64::MAX),
                asm::load_imm64(Reg::A3, ((base + 20) << 32) | (base + 52)),
                asm::load_imm64(Reg::A4, ((input.len() as u64) << 32) | (base + 84)),
                asm::load_imm64(Reg::A5, u64::MAX),
                asm::ecalli(0),
                asm::ret(),
            ],
            &[],
        );
        let contract = deploy(blob.into_vec().unwrap());
        assert_ok!(native_call(contract_call(contract, 0, vec![])));
        let id = n::ColdkeyValidatorId::<Runtime>::get(Mapper::to_account_id(&contract))
            .expect("PVM caller owns validator");
        assert_eq!(
            n::ValidatorsData::<Runtime>::get(id).hotkey,
            AccountId::new([47; 32])
        );
    });
}

#[test]
fn permission_failures_do_not_reassign_validator_ownership() {
    npos_tests::ext().execute_with(|| {
        let who = setup();
        assert_ok!(invoke(addresses::VALIDATORS, registration(48)));
        let id = n::ColdkeyValidatorId::<Runtime>::get(&who).unwrap();
        let contract = deploy(forwarder(addresses::VALIDATORS, 0xf1, false));
        let update = abi::IValidators::updateValidatorColdkeyCall {
            validatorId: id,
            newColdkey: [49; 32].into(),
        };
        assert!(native_call(contract_call(contract, 0, update.abi_encode())).is_err());
        assert_eq!(n::ValidatorColdkey::<Runtime>::get(id), Some(who));
        assert_eq!(
            n::ColdkeyValidatorId::<Runtime>::get(AccountId::new([49; 32])),
            None
        );
    });
}

#[test]
fn every_mutation_selector_routes_to_its_declared_native_call_index() {
    macro_rules! routes {
        ($domain:ident, $interface:ident, $calls:ident; $($call:ident => $index:expr),+ $(,)?) => {
            $( {
                use pallet_revive::precompiles::alloy::sol_types::SolInterface;
                let wire=abi::$interface::$call::default().abi_encode();
                let decoded=abi::$interface::$calls::abi_decode_validate(&wire).unwrap();
                let call=network_precompiles::$domain::to_call::<Runtime>(&decoded).unwrap().expect("mutation");
                assert_eq!(call.encode()[0],$index,stringify!($call));
            } )+
        };
    }
    routes!(validators, IValidators, IValidatorsCalls;
        registerValidatorCall => 0,
        updateValidatorColdkeyCall => 1,
        updateValidatorHotkeyCall => 2,
        updateValidatorDelegateRewardRateCall => 3,
        updateValidatorDelegateAccountCall => 4,
        updateValidatorIdentityCall => 5,
        setValidatorNodeDelegateStakeWeightsCall => 172,
    );
    routes!(subnets, ISubnets, ISubnetsCalls;
        registerSubnetCall => 6,
        activateSubnetCall => 7,
        ownerPauseSubnetCall => 8,
        ownerUnpauseSubnetCall => 9,
        ownerDeactivateSubnetCall => 10,
        ownerUpdateNameCall => 11,
        ownerUpdateRepoCall => 12,
        ownerUpdateDescriptionCall => 13,
        ownerUpdateMiscCall => 14,
        ownerUpdateChurnLimitCall => 15,
        ownerUpdateChurnLimitMultiplierCall => 16,
        ownerUpdateRegistrationQueueEpochsCall => 17,
        ownerUpdateIdleClassificationEpochsCall => 18,
        ownerUpdateIncludedClassificationEpochsCall => 19,
        ownerUpdateSubnetNodeMinWeightDecreaseReputationThresholdCall => 27,
        ownerUpdateMinSubnetNodeReputationCall => 28,
        ownerAddOrUpdateInitialValidatorsCall => 29,
        ownerRemoveInitialValidatorsCall => 30,
        ownerSetEmergencyValidatorSetCall => 31,
        ownerRevertEmergencyValidatorSetCall => 32,
        ownerUpdateMinMaxStakeCall => 33,
        ownerUpdateDelegateStakePercentageCall => 34,
        ownerUpdateMaxRegisteredNodesCall => 35,
        transferSubnetOwnershipCall => 36,
        acceptSubnetOwnershipCall => 37,
        ownerAddBootnodeAccessCall => 38,
        ownerRemoveBootnodeAccessCall => 39,
        ownerUpdateTargetNodeRegistrationsPerEpochCall => 40,
        ownerUpdateNodeBurnRateAlphaCall => 41,
        ownerUpdateQueueImmunityEpochsCall => 42,
        updateBootnodesCall => 43,
        ownerUpdateReputationFactorsCall => 169,
        ownerUpdateConsensusValidatorNodeCountDecayCall => 170,
        cancelSubnetOwnershipTransferCall => 178,
        ownerUpdateConsensusValidatorStakeWeightPowerCall => 179,
    );
    routes!(subnet_nodes, ISubnetNodes, ISubnetNodesCalls;
        registerSubnetNodeCall => 44,
        updateNodeHotkeyCall => 45,
        updateNodePeerInfoCall => 46,
        updateNodeBootnodePeerInfoCall => 47,
        updateNodeClientPeerInfoCall => 48,
        updateNodeUniqueCall => 49,
        updateNodeNonUniqueCall => 50,
        removeSubnetNodeCall => 51,
    );
    routes!(staking, IStaking, IStakingCalls;
        addNodeStakeCall => 52,
        removeNodeStakeCall => 53,
        addSubnetDelegateStakeCall => 54,
        swapFromSubnetToSubnetCall => 55,
        transferDelegateStakeCall => 56,
        removeDelegateStakeCall => 57,
        addValidatorDelegateStakeCall => 59,
        transferValidatorDelegateStakeCall => 60,
        removeValidatorDelegateStakeCall => 61,
        swapFromValidatorToValidatorCall => 62,
        swapFromValidatorToSubnetCall => 64,
        swapFromSubnetToValidatorCall => 65,
        updateSwapQueueCall => 66,
        removeDelegateAccountBalanceCall => 67,
        claimUnbondingsCall => 68,
        addOverwatchNodeStakeCall => 77,
        removeOverwatchNodeStakeCall => 78,
    );
    routes!(consensus, IConsensus, IConsensusCalls;
        proposeAttestationCall => 69,
        attestCall => 70,
    );
    routes!(overwatch, IOverwatch, IOverwatchCalls;
        registerOverwatchNodeCall => 71,
        removeOverwatchNodeCall => 72,
        updateOverwatchHotkeyCall => 73,
        setOverwatchNodePeerIdCall => 74,
        commitOverwatchSubnetWeightsCall => 75,
        revealOverwatchSubnetWeightsCall => 76,
    );
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn network_precompile_benchmarks_with_production_bounds() {
    struct Timer {
        start: Option<std::time::Instant>,
        elapsed: std::time::Duration,
    }
    impl frame_benchmarking::Recording for Timer {
        fn start(&mut self) {
            self.start = Some(std::time::Instant::now());
        }
        fn stop(&mut self) {
            self.elapsed = self.start.take().unwrap().elapsed();
        }
    }
    for name in [
        "abi_identity",
        "abi_registration",
        "abi_attestation",
        "dispatch_weight_selection",
        "staking_read",
        "validator_read",
        "allocation_read",
        "subnet_read",
        "node_read",
        "consensus_read",
        "overwatch_read",
    ] {
        npos_tests::ext().execute_with(|| {
            let mut timer = Timer {
                start: None,
                elapsed: Default::default(),
            };
            network_precompiles::benchmarking::run_case::<Runtime>(name, &mut timer).unwrap();
            println!("network precompile benchmark {name}: {:?}", timer.elapsed);
        });
    }
}

#[test]
fn delegation_enforces_balance_slippage_and_unbonding_maturity() {
    npos_tests::ext().execute_with(|| {
        let who = setup();
        assert_ok!(invoke(addresses::VALIDATORS, registration(50)));
        let id = n::ColdkeyValidatorId::<Runtime>::get(&who).unwrap();
        let amount = n::MinDelegateStakeDeposit::<Runtime>::get().max(10 * TENSOR);
        let deposit = abi::IStaking::addValidatorDelegateStakeCall {
            validatorId: id,
            delegateStakeToBeAdded: amount,
            minSharesOut: 1,
        };
        let mut impossible = deposit.clone();
        impossible.minSharesOut = u128::MAX;
        assert!(invoke(addresses::STAKING, impossible).is_err());
        let mut unaffordable = deposit.clone();
        unaffordable.delegateStakeToBeAdded = Balances::free_balance(&who) + TENSOR;
        assert!(invoke(addresses::STAKING, unaffordable).is_err());
        assert_eq!(n::ValidatorDelegateStakeBalance::<Runtime>::get(id), 0);
        assert_ok!(invoke(addresses::STAKING, deposit));
        let shares = Network::current_account_validator_delegate_stake_shares(&who, id);
        assert!(shares > 0);
        assert_ok!(invoke(
            addresses::STAKING,
            abi::IStaking::removeValidatorDelegateStakeCall {
                validatorId: id,
                validatorDelegateStakeSharesToBeRemoved: shares,
                minBalanceOut: 1
            }
        ));
        let ledger = n::StakeUnbondingLedger::<Runtime>::get(&who);
        assert!(!ledger.is_empty());
        assert!(invoke(addresses::STAKING, abi::IStaking::claimUnbondingsCall {}).is_err());
        assert_eq!(n::StakeUnbondingLedger::<Runtime>::get(&who), ledger);
        System::set_block_number(*ledger.keys().max().unwrap());
        assert_ok!(invoke(
            addresses::STAKING,
            abi::IStaking::claimUnbondingsCall {}
        ));
        assert!(n::StakeUnbondingLedger::<Runtime>::get(who).is_empty());
    });
}

#[test]
fn overwatch_contract_hotkey_obeys_commit_reveal_and_pays_for_pays_no_calls() {
    use sp_runtime::traits::Hash as _;
    npos_tests::ext().execute_with(|| {
        let who = setup();
        let contract = deploy(forwarder(addresses::OVERWATCH, 0xf1, false));
        let mut reg = registration(51);
        reg.hotkey = convert::account_out(Mapper::to_account_id(&contract));
        assert_ok!(invoke(addresses::VALIDATORS, reg));
        let validator_id = n::ColdkeyValidatorId::<Runtime>::get(&who).unwrap();
        n::OverwatchValidatorWhitelist::<Runtime>::insert(validator_id, ());
        n::CurrentOverwatchEpoch::<Runtime>::put(1);
        n::OverwatchEpochStartBlock::<Runtime>::put(0);
        n::ActiveOverwatchEpochLengthMultiplier::<Runtime>::put(1);
        n::ActiveOverwatchCommitCutoffPercent::<Runtime>::put(TENSOR / 2);
        n::OverwatchMinStakeBalance::<Runtime>::put(TENSOR);
        assert_ok!(invoke(
            addresses::OVERWATCH,
            abi::IOverwatch::registerOverwatchNodeCall {
                stakeToBeAdded: TENSOR
            }
        ));
        let id = n::ValidatorOverwatchNodeId::<Runtime>::get(validator_id).unwrap();
        n::SubnetsData::<Runtime>::insert(
            1,
            n::SubnetData {
                id: 1,
                ..Default::default()
            },
        );
        let salt: n::OverwatchRevealSalt<Runtime> = vec![1, 2, 3].try_into().unwrap();
        let hash = <Runtime as frame_system::Config>::Hashing::hash_of(&(123u128, salt));
        let commit = abi::IOverwatch::commitOverwatchSubnetWeightsCall {
            overwatchNodeId: id,
            commitWeights: vec![abi::OverwatchCommit {
                subnetId: 1,
                weight: hash.0.into(),
            }],
        };
        let reveal = abi::IOverwatch::revealOverwatchSubnetWeightsCall {
            overwatchNodeId: id,
            reveals: vec![abi::OverwatchReveal {
                subnetId: 1,
                weight: 123,
                salt: vec![1, 2, 3].into(),
            }],
        };
        assert!(native_call(contract_call(contract, 0, reveal.abi_encode())).is_err());
        // The transaction origin is the coldkey, so only the contract hotkey can commit.
        assert!(invoke(addresses::OVERWATCH, commit.clone()).is_err());
        let before = Balances::free_balance(&who);
        assert_ok!(native_call(contract_call(contract, 0, commit.abi_encode())));
        assert!(
            Balances::free_balance(&who) < before,
            "Pays::No must not refund contract gas"
        );
        System::set_block_number(EpochLength::get());
        assert!(native_call(contract_call(contract, 0, commit.abi_encode())).is_err());
        let mut wrong = reveal.clone();
        wrong.reveals[0].weight = 124;
        assert!(native_call(contract_call(contract, 0, wrong.abi_encode())).is_err());
        assert!(n::OverwatchReveals::<Runtime>::get(1, id).is_empty());
        assert_ok!(native_call(contract_call(contract, 0, reveal.abi_encode())));
        assert_eq!(
            n::OverwatchReveals::<Runtime>::get(1, id).get(&1),
            Some(&123)
        );
    });
}

#[test]
fn static_reads_work_and_nonzero_swap_arguments_survive_abi_conversion() {
    use pallet_revive::precompiles::alloy::sol_types::SolInterface;
    let wire = abi::IStaking::swapFromSubnetToValidatorCall {
        fromSubnetId: 17,
        toValidatorId: 29,
        subnetDelegateStakeSharesToSwap: 123456789123456789,
        minBalanceOut: 10,
        minSharesOut: 11,
        executeBeforeBlock: 900,
    }
    .abi_encode();
    let decoded = abi::IStaking::IStakingCalls::abi_decode_validate(&wire).unwrap();
    let call = network_precompiles::staking::to_call::<Runtime>(&decoded)
        .unwrap()
        .unwrap();
    let expected = n::Call::<Runtime>::swap_from_subnet_to_validator {
        from_subnet_id: 17,
        to_validator_id: 29,
        subnet_delegate_stake_shares_to_swap: 123456789123456789,
        min_balance_out: 10,
        min_shares_out: 11,
        execute_before_block: 900,
    };
    assert_eq!(call.encode(), expected.encode());
    npos_tests::ext().execute_with(|| {
        let who = setup();
        n::NodeSubnetStake::<Runtime>::insert(7, 42, 321u128);
        let contract = deploy(forwarder(addresses::STAKING, 0xfa, false));
        let data = abi::IStaking::getNodeStakeCall {
            subnetId: 42,
            subnetNodeId: 7,
        }
        .abi_encode();
        let result = Revive::bare_call(
            RuntimeOrigin::signed(who),
            contract,
            U256::zero(),
            TransactionLimits::WeightAndDeposit {
                weight_limit: CONTRACT_WEIGHT,
                deposit_limit: TENSOR,
            },
            data,
            &Default::default(),
        )
        .result
        .unwrap();
        assert!(!result.did_revert());
        assert_eq!(
            abi::IStaking::getNodeStakeCall::abi_decode_returns(&result.data).unwrap(),
            321
        );
    });
}

#[path = "network_precompile_arguments.rs"]
mod argument_fixtures;

#[test]
fn invalid_abi_is_metered_and_returns_shared_revert_strings() {
    use pallet_revive::precompiles::alloy::sol_types::{Revert, SolError};
    npos_tests::ext().execute_with(|| {
        let who = setup();
        let call = |data| {
            Revive::bare_call(
                RuntimeOrigin::signed(who.clone()),
                address(addresses::VALIDATORS),
                U256::zero(),
                TransactionLimits::WeightAndDeposit {
                    weight_limit: CONTRACT_WEIGHT,
                    deposit_limit: TENSOR,
                },
                data,
                &Default::default(),
            )
        };
        let small = call(vec![0xff; 4]);
        let large = call(vec![0xff; 4096]);
        assert!(large.weight_consumed.ref_time() > small.weight_consumed.ref_time());
        for (result, reason) in [
            (small, "Network: unknown selector"),
            (large, "Network: unknown selector"),
            (call(vec![]), "Network: malformed ABI"),
            (
                call(registration(42).abi_encode()[..15].to_vec()),
                "Network: malformed ABI",
            ),
        ] {
            let output = result.result.unwrap();
            assert!(output.did_revert());
            assert_eq!(Revert::abi_decode(&output.data).unwrap().reason, reason);
        }
    });
}

#[test]
fn scalar_reads_do_not_pay_for_unrelated_collection_decoding() {
    npos_tests::ext().execute_with(|| {
        let who = setup();
        let measure = |data| {
            let result = Revive::bare_call(
                RuntimeOrigin::signed(who.clone()),
                address(addresses::STAKING),
                U256::zero(),
                TransactionLimits::WeightAndDeposit {
                    weight_limit: CONTRACT_WEIGHT,
                    deposit_limit: TENSOR,
                },
                data,
                &Default::default(),
            );
            assert!(!result.result.unwrap().did_revert());
            result.weight_consumed
        };
        let scalar = measure(
            abi::IStaking::getNodeStakeCall {
                subnetId: 1,
                subnetNodeId: 1,
            }
            .abi_encode(),
        );
        let ledger = measure(
            abi::IStaking::getUnbondingsCall {
                accountId: convert::account_out(who.clone()),
            }
            .abi_encode(),
        );
        assert!(scalar.ref_time() < ledger.ref_time());
        assert!(scalar.proof_size() < ledger.proof_size());
    });
}

#[test]
fn initial_validator_removal_uses_the_registered_node_bound() {
    let max = <Runtime as n::Config>::MaxRegisteredNodesUpperBound::get();
    assert!(convert::validator_set::<Runtime>(&(0..max).collect::<Vec<_>>()).is_ok());
    assert!(convert::validator_set::<Runtime>(&(0..=max).collect::<Vec<_>>()).is_err());
}
