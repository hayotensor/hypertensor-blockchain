//! Shared fixtures for opt-in capacity research within the unchanged production bounds.
use super::*;

/// Maximum SCALE payloads accepted by the current peer/address validators. Each role and
/// node has distinct reverse-index keys. These are prepared outside measured execution.
pub(super) fn max_node_bytes<T: Config>(subnet_id: u32, node_id: u32, role: u8) -> NetworkBytes<T> {
    let mut bytes = vec![role; T::MaxVectorLength::get() as usize];
    bytes[..4].copy_from_slice(&subnet_id.to_le_bytes());
    bytes[4..8].copy_from_slice(&node_id.to_le_bytes());
    bytes.try_into().unwrap()
}

pub(super) fn max_node_peer<T: Config>(subnet_id: u32, node_id: u32, role: u8) -> PeerInfo<T> {
    let mut peer = format!("Qm{subnet_id:08x}{node_id:08x}{role:02x}").into_bytes();
    peer.resize(128, b'a');
    let peer_id = PeerId(peer.clone());
    assert!(Network::<T>::validate_peer_id(&peer_id));
    // DNS4 length prefix (1 + 2 bytes), followed by P2P (2 + 2 + 128 bytes).
    let dns_len = T::MaxVectorLength::get() as usize - 135;
    let mut address = Vec::new();
    encode_varint(DNS4, &mut address);
    encode_varint(dns_len as u64, &mut address);
    address.extend(vec![b'a'; dns_len]);
    encode_varint(P2P, &mut address);
    encode_varint(peer.len() as u64, &mut address);
    address.extend(peer);
    assert_eq!(address.len(), T::MaxVectorLength::get() as usize);
    assert!(Multiaddr::verify(&address).is_ok());
    PeerInfo {
        peer_id,
        multiaddr: Some(address.try_into().unwrap()),
    }
}

pub(super) fn max_fill_node<T: Config>(subnet_id: u32, node_id: u32) {
    assert_ok!(Network::<T>::do_update_node_peer_info(
        subnet_id,
        node_id,
        Some(max_node_peer::<T>(subnet_id, node_id, 1))
    ));
    assert_ok!(Network::<T>::do_update_node_bootnode_peer_info(
        subnet_id,
        node_id,
        Some(max_node_peer::<T>(subnet_id, node_id, 2))
    ));
    assert_ok!(Network::<T>::do_update_node_client_peer_info(
        subnet_id,
        node_id,
        Some(max_node_peer::<T>(subnet_id, node_id, 3))
    ));
    assert_ok!(Network::<T>::do_update_node_unique(
        subnet_id,
        node_id,
        Some(max_node_bytes::<T>(subnet_id, node_id, 21))
    ));
    assert_ok!(Network::<T>::do_update_node_non_unique(
        subnet_id,
        node_id,
        Some(max_node_bytes::<T>(subnet_id, node_id, 22))
    ));
}

pub(super) fn max_queue<T: Config>(subnet_id: u32, nodes: u32) -> (u32, u32) {
    let queue_len = T::MaxRegisteredNodesUpperBound::get();
    MaxRegisteredNodes::<T>::insert(subnet_id, queue_len);
    build_queued_subnet_nodes::<T>(
        subnet_id,
        nodes,
        nodes + queue_len,
        DEFAULT_DEPOSIT_AMOUNT,
        DEFAULT_SUBNET_NODE_STAKE,
        true,
    );
    for id in nodes + 1..=nodes + queue_len {
        max_fill_node::<T>(subnet_id, id);
    }
    let epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
    // Represent an existing mature queue, so both tail actions pass the actual proposal guard.
    for id in nodes + 1..=nodes + queue_len {
        QueuedSubnetNodesData::<T>::mutate(subnet_id, id, |node| {
            node.classification.start_epoch = epoch.saturating_sub(1);
        });
    }
    SubnetNodeQueue::<T>::mutate(subnet_id, |queue| {
        for node in queue {
            node.classification.start_epoch = epoch.saturating_sub(1);
        }
    });
    SubnetElectedValidator::<T>::mutate(subnet_id, epoch, |round| {
        round.as_mut().unwrap().policy.queue_immunity_epochs = 0;
    });
    (nodes + queue_len, nodes + queue_len - 1)
}

pub(super) struct SettlementFixture {
    pub subnet_id: u32,
    pub epoch: u32,
    pub nodes: u32,
    pub direct_stakes: Vec<u128>,
    pub pool_balances: Vec<u128>,
    pub beneficiary_balances: Vec<u128>,
    pub owner_balance: u128,
    pub subnet_pool_balance: u128,
}

pub(super) fn elected_fixture<T: Config>(nodes: u32) -> u32 {
    elected_fixture_with_metadata::<T>(nodes, true)
}

fn elected_fixture_with_metadata<T: Config>(nodes: u32, keep_node_metadata: bool) -> u32 {
    elected_fixture_with_reputation::<T>(nodes, keep_node_metadata, false)
}

fn elected_fixture_with_reputation<T: Config>(
    nodes: u32,
    keep_node_metadata: bool,
    combined: bool,
) -> u32 {
    MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
    build_activated_subnet::<T>(
        DEFAULT_SUBNET_NAME.into(),
        0,
        nodes,
        DEFAULT_DEPOSIT_AMOUNT,
        DEFAULT_SUBNET_NODE_STAKE,
    );
    let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
    let bytes: NetworkBytes<T> = vec![1; T::MaxVectorLength::get() as usize]
        .try_into()
        .unwrap();
    let url: NetworkUrl<T> = vec![2; T::MaxUrlLength::get() as usize].try_into().unwrap();
    let social: NetworkSocialId<T> = vec![3; T::MaxSocialIdLength::get() as usize]
        .try_into()
        .unwrap();
    let identity = IdentityData::<T> {
        name: Some(bytes.clone()),
        url: Some(url.clone()),
        image: Some(url.clone()),
        discord: Some(social.clone()),
        x: Some(social.clone()),
        telegram: Some(social),
        github: Some(url.clone()),
        hugging_face: Some(url),
        description: Some(bytes.clone()),
        misc: Some(bytes),
    };
    for id in 1..=nodes {
        max_fill_node::<T>(subnet_id, id);
        let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, id).unwrap();
        // Use the real deposit path: a balance and total shares alone do NOT make a pool
        // reward-bearing. Circulating shares and a consistent balance-time checkpoint matter.
        let depositor = get_alice::<T>();
        assert_ok!(Network::<T>::add_validator_delegate_stake(
            RawOrigin::Signed(depositor).into(),
            validator_id,
            DEFAULT_DELEGATE_STAKE_TO_BE_ADDED,
            1,
        ));
        assert!(ValidatorDelegateStakeCirculatingShares::<T>::get(validator_id) > 0);
        ValidatorsData::<T>::mutate(validator_id, |validator| {
            ValidatorMetadata::<T>::insert(validator_id, identity.clone());
            validator.delegate_reward_rate = Network::<T>::percentage_factor_as_u128() / 2;
            validator.delegate_account = Some(DelegateAccount {
                account_id: get_account::<T>("capacity-beneficiary", validator_id),
                rate: Network::<T>::percentage_factor_as_u128() / 10,
            });
        });
        SubnetNodeReputation::<T>::insert(
            subnet_id,
            id,
            Network::<T>::percentage_factor_as_u128() / 2,
        );
    }
    if !keep_node_metadata {
        for id in 1..=nodes {
            assert_ok!(Network::<T>::do_update_node_peer_info(subnet_id, id, None));
            assert_ok!(Network::<T>::do_update_node_bootnode_peer_info(
                subnet_id, id, None
            ));
            assert_ok!(Network::<T>::do_update_node_client_peer_info(
                subnet_id, id, None
            ));
            assert_ok!(Network::<T>::do_update_node_unique(subnet_id, id, None));
            assert_ok!(Network::<T>::do_update_node_non_unique(subnet_id, id, None));
        }
    }
    frame_system::Pallet::<T>::reset_events();
    increase_epochs::<T>(1);
    set_block_to_subnet_slot_epoch::<T>(Network::<T>::get_current_epoch_as_u32(), subnet_id);
    if combined {
        let epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
        let (candidates, _, _) = Network::<T>::resolve_consensus_validator_ids(subnet_id, epoch);
        let proposer = candidates
            [Network::<T>::get_bounded_random_index((subnet_id, epoch), nodes).unwrap() as usize];
        let mut attestors = [1, 2, 3];
        attestors[((proposer - 1) % 3) as usize] = proposer;
        let one = Network::<T>::percentage_factor_as_u128();
        // Three coherent identities, each allocating its funded circulating pool to one
        // attestor. The elected proposer is one of these three; all other nodes can abstain.
        for id in 1..=nodes {
            ValidatorSubnetNodes::<T>::remove(id);
            TotalValidatorNodes::<T>::remove(id);
            ValidatorNodeDelegateStakeWeights::<T>::remove(id);
        }
        for id in 1..=nodes {
            let validator = (id - 1) % 3 + 1;
            SubnetNodeValidatorId::<T>::insert(subnet_id, id, validator);
            SubnetNodesData::<T>::mutate(subnet_id, id, |node| node.validator_id = validator);
            ValidatorSubnetNodes::<T>::mutate(validator, |owned| {
                owned.entry(subnet_id).or_default().insert(id);
            });
            TotalValidatorNodes::<T>::mutate(validator, |count| *count += 1);
            ValidatorNodeDelegateStakeWeights::<T>::mutate(validator, |weights| {
                weights.insert(
                    (subnet_id, id),
                    if attestors.contains(&id) { one } else { 0 },
                );
            });
        }
        assert!(
            one / nodes as u128 + 1
                <= MaxSubnetNodeMinWeightDecreaseReputationThreshold::<T>::get()
        );
        SubnetNodeMinWeightDecreaseReputationThreshold::<T>::insert(
            subnet_id,
            one / nodes as u128 + 1,
        );
        SubnetReputationFactorSchedules::<T>::insert(
            subnet_id,
            SubnetReputationFactorSchedule {
                current: SubnetReputationFactors {
                    included_increase: one / 10,
                    below_min_weight_decrease: one / 10,
                    non_attestor_decrease: one / 10,
                    ..Default::default()
                },
                pending: None,
            },
        );
    }
    Network::<T>::elect_validator(
        subnet_id,
        Network::<T>::get_current_subnet_epoch_as_u32(subnet_id),
        get_current_block_as_u32::<T>(),
    );
    subnet_id
}

pub(super) fn propose<T: Config>(subnet_id: u32, nodes: u32) {
    let epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
    let proposer = SubnetElectedValidator::<T>::get(subnet_id, epoch)
        .unwrap()
        .validator_subnet_node_id;
    let hotkey = Network::<T>::get_subnet_node_associated_hotkey(subnet_id, proposer).unwrap();
    let data = get_subnet_node_consensus_data::<T>(subnet_id, nodes, 0, nodes);
    let args: ValidatorArgs<T> = vec![0xff; T::ValidatorArgsLimit::get() as usize]
        .try_into()
        .unwrap();
    assert_ok!(Network::<T>::propose_attestation(
        RawOrigin::Signed(hotkey).into(),
        subnet_id,
        data,
        None,
        None,
        Some(args.clone()),
        Some(args),
    ));
}

pub(super) fn settlement_fixture<T: Config>(nodes: u32) -> SettlementFixture {
    settlement_fixture_with_metadata::<T>(nodes, true)
}

fn settlement_fixture_with_metadata<T: Config>(
    nodes: u32,
    keep_node_metadata: bool,
) -> SettlementFixture {
    settlement_fixture_with_reputation::<T>(nodes, keep_node_metadata, false)
}

pub(super) fn settlement_fixture_with_reputation<T: Config>(
    nodes: u32,
    keep_node_metadata: bool,
    combined: bool,
) -> SettlementFixture {
    let subnet_id = elected_fixture_with_reputation::<T>(nodes, keep_node_metadata, combined);
    if combined {
        propose::<T>(subnet_id, nodes);
        let epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
        let proposer = SubnetElectedValidator::<T>::get(subnet_id, epoch)
            .unwrap()
            .validator_subnet_node_id;
        let mut attestors = [1, 2, 3];
        attestors[((proposer - 1) % 3) as usize] = proposer;
        for id in attestors {
            if id != proposer {
                assert_ok!(Network::<T>::attest(
                    RawOrigin::Signed(
                        Network::<T>::get_subnet_node_associated_hotkey(subnet_id, id).unwrap()
                    )
                    .into(),
                    subnet_id,
                    id,
                    None,
                ));
            }
        }
        let (submission, _) = Network::<T>::precheck_subnet_consensus_submission(
            subnet_id,
            epoch,
            Network::<T>::get_current_epoch_as_u32(),
        );
        let submission = submission.unwrap();
        assert_eq!(submission.attests.len(), 3);
        assert_eq!(submission.eligible_validator_identity_count, 3);
        assert_eq!(
            submission.identity_attestation_ratio,
            Network::<T>::percentage_factor_as_u128()
        );
        assert_eq!(
            submission.attestation_ratio,
            Network::<T>::percentage_factor_as_u128()
        );
    } else {
        run_subnet_consensus_step::<T>(subnet_id, None, None);
    }
    let fixture = SettlementFixture {
        subnet_id,
        epoch: Network::<T>::get_current_subnet_epoch_as_u32(subnet_id),
        nodes,
        direct_stakes: (1..=nodes)
            .map(|id| NodeSubnetStake::<T>::get(id, subnet_id))
            .collect(),
        pool_balances: (1..=nodes)
            .map(|id| {
                ValidatorDelegateStakeBalance::<T>::get(
                    SubnetNodeValidatorId::<T>::get(subnet_id, id).unwrap(),
                )
            })
            .collect(),
        beneficiary_balances: (1..=nodes)
            .map(|id| {
                DelegateAccountStake::<T>::get(get_account::<T>(
                    "capacity-beneficiary",
                    SubnetNodeValidatorId::<T>::get(subnet_id, id).unwrap(),
                ))
            })
            .collect(),
        owner_balance: Network::<T>::balance_to_u128(T::Currency::free_balance(
            &SubnetOwner::<T>::get(subnet_id).unwrap(),
        ))
        .unwrap(),
        subnet_pool_balance: TotalSubnetDelegateStakeBalance::<T>::get(subnet_id),
    };
    increase_epochs::<T>(1);
    set_block_to_subnet_slot_epoch::<T>(Network::<T>::get_current_epoch_as_u32(), subnet_id);
    let epoch = Network::<T>::get_current_epoch_as_u32();
    Network::<T>::handle_subnet_emission_weights(epoch);
    assert!(FinalSubnetEmissionWeights::<T>::get(epoch)
        .subnet_weights
        .contains_key(&subnet_id));
    max_fill_benchmark_subnet_data::<T>(subnet_id);
    FinalSubnetEmissionWeights::<T>::mutate(epoch, |distribution| {
        for index in 0..T::MaxPhysicalSubnetsUpperBound::get().saturating_sub(1) {
            distribution.subnet_weights.insert(100_000 + index, 1);
        }
    });
    seed_subnet_balance_time_checkpoint::<T>(subnet_id, 1);
    fixture
}

pub(super) fn verify_settlement<T: Config>(fixture: &SettlementFixture) {
    assert_eq!(
        ConsensusRoundSettlementStatus::<T>::get(fixture.subnet_id, fixture.epoch),
        Some(true)
    );
    assert!(
        Network::<T>::balance_to_u128(T::Currency::free_balance(
            &SubnetOwner::<T>::get(fixture.subnet_id).unwrap(),
        ))
        .unwrap()
            > fixture.owner_balance
    );
    assert!(
        TotalSubnetDelegateStakeBalance::<T>::get(fixture.subnet_id) > fixture.subnet_pool_balance
    );
    for id in 1..=fixture.nodes {
        let index = (id - 1) as usize;
        let validator_id = SubnetNodeValidatorId::<T>::get(fixture.subnet_id, id).unwrap();
        assert!(NodeSubnetStake::<T>::get(id, fixture.subnet_id) > fixture.direct_stakes[index]);
        assert!(
            ValidatorDelegateStakeBalance::<T>::get(validator_id) > fixture.pool_balances[index]
        );
        assert!(
            DelegateAccountStake::<T>::get(get_account::<T>("capacity-beneficiary", validator_id))
                > fixture.beneficiary_balances[index]
        );
    }
}

pub(super) fn verify_combined_reputation<T: Config>(fixture: &SettlementFixture) {
    let proposer = SubnetElectedValidator::<T>::get(fixture.subnet_id, fixture.epoch)
        .unwrap()
        .validator_subnet_node_id;
    let mut attestors = [1, 2, 3];
    attestors[((proposer - 1) % 3) as usize] = proposer;
    let one = Network::<T>::percentage_factor_as_u128();
    let increased = Network::<T>::increase_rep(one / 2, one / 10, None);
    let decreased = Network::<T>::decrease_rep(increased, one / 10, None);
    let non_attestor = Network::<T>::decrease_rep(decreased, one / 10, None);
    for id in 1..=fixture.nodes {
        assert_eq!(
            SubnetNodeReputation::<T>::get(fixture.subnet_id, id),
            Some(if attestors.contains(&id) {
                decreased
            } else {
                non_attestor
            })
        );
    }
    assert!(PendingAdmittedNodeRemovals::<T>::get(fixture.subnet_id).is_empty());
}

/// Compare every resulting storage value and event across the proposal-summary layout change.
/// Normalize only the submission encoding to its original field tuple, retaining all contents.
#[test]
fn proposal_summary_capacity_outcomes() {
    use crate::tests::mock::{new_test_ext, Test};
    for combined in [false, true] {
        new_test_ext().execute_with(|| {
            let fixture = settlement_fixture_with_reputation::<Test>(512, true, combined);
            frame_system::Pallet::<Test>::reset_events();
            let charged = Network::<Test>::on_initialize(frame_system::Pallet::<Test>::block_number());
            assert!(charged.all_lte(<Test as Config>::MaximumHooksWeight::get()));
            verify_settlement::<Test>(&fixture);
            if combined { verify_combined_reputation::<Test>(&fixture); }
            let next = Network::<Test>::get_current_subnet_epoch_as_u32(fixture.subnet_id);
            assert!(SubnetElectedValidator::<Test>::contains_key(fixture.subnet_id, next));
            for (subnet, epoch, submission) in SubnetConsensusSubmission::<Test>::iter() {
                let stored_size = submission.encoded_size();
                assert_eq!(submission.weight_sum, submission.data.iter().map(|entry| entry.score).sum::<u128>());
                assert_eq!(submission.eligible_validator_identity_count, submission.validator_identity_ids.values().copied().collect::<BTreeSet<_>>().len() as u32);
                let canonical_encoding = (
                    submission.validator_id, submission.block, submission.validator_epoch_progress,
                    submission.validator_reward_factor, submission.validator_ids,
                    submission.validator_identity_ids, submission.attests, submission.subnet_nodes,
                    submission.prioritize_queue_node_id, submission.remove_queue_node_id,
                    submission.data, submission.args, submission.emergency,
                ).encode();
                assert_eq!(stored_size, canonical_encoding.len() + 20);
                sp_io::storage::set(&SubnetConsensusSubmission::<Test>::hashed_key_for(subnet, epoch), &canonical_encoding);
            }
            println!("proposal_summary_outcome,combined={combined},nodes=512,state={:?}", sp_io::storage::root(sp_runtime::StateVersion::V1));
        });
    }
}

/// The before/after artifacts compare all state after normalizing the intentionally changed
/// event stream. This also records event counts independently of the benchmark's storage cache.
#[test]
fn reputation_capacity_outcomes() {
    use crate::tests::mock::{new_test_ext, Test};
    for combined in [false, true] {
        new_test_ext().execute_with(|| {
            let fixture = settlement_fixture_with_reputation::<Test>(512, true, combined);
            frame_system::Pallet::<Test>::reset_events();
            let charged =
                Network::<Test>::on_initialize(frame_system::Pallet::<Test>::block_number());
            assert!(charged.all_lte(<Test as Config>::MaximumHooksWeight::get()));
            verify_settlement::<Test>(&fixture);
            if combined { verify_combined_reputation::<Test>(&fixture); }
            let next = Network::<Test>::get_current_subnet_epoch_as_u32(fixture.subnet_id);
            assert!(SubnetElectedValidator::<Test>::contains_key(fixture.subnet_id, next));
            let events = frame_system::Pallet::<Test>::events();
            let reputation_events = events.iter().filter(|event| matches!(
                event.event,
                crate::tests::mock::RuntimeEvent::Network(Event::NodeReputationUpdate { .. })
            )).count();
            let changed_nodes: BTreeSet<_> = events.iter().filter_map(|event| match &event.event {
                crate::tests::mock::RuntimeEvent::Network(Event::NodeReputationUpdate { subnet_node_id, .. }) => Some(*subnet_node_id),
                _ => None,
            }).collect();
            assert_eq!(reputation_events, 512);
            assert_eq!(changed_nodes.len(), 512);
            frame_system::Pallet::<Test>::reset_events();
            println!("reputation_outcome,combined={combined},nodes=512,events={reputation_events},state={:?}", sp_io::storage::root(sp_runtime::StateVersion::V1));
        });
    }
}

#[test]
fn maximum_metadata_does_not_change_512_node_hook_outcomes() {
    use crate::tests::mock::{new_test_ext, Test};

    let execute = |keep_metadata: bool| {
        new_test_ext().execute_with(|| {
            let fixture = settlement_fixture::<Test>(512);
            let validator_ids: Vec<_> = (1..=fixture.nodes)
                .map(|id| SubnetNodeValidatorId::<Test>::get(fixture.subnet_id, id).unwrap())
                .collect();
            if !keep_metadata {
                for id in &validator_ids {
                    ValidatorMetadata::<Test>::remove(id);
                }
            }
            Network::<Test>::on_initialize(frame_system::Pallet::<Test>::block_number());
            verify_settlement::<Test>(&fixture);
            let epoch = Network::<Test>::get_current_subnet_epoch_as_u32(fixture.subnet_id);
            assert!(SubnetElectedValidator::<Test>::contains_key(
                fixture.subnet_id,
                epoch
            ));
            for id in validator_ids {
                ValidatorMetadata::<Test>::remove(id);
            }
            // With the optional metadata removed, every resulting state value and event must
            // match, including all reward amounts, settlement fences and the next election.
            sp_io::storage::root(sp_runtime::StateVersion::V1)
        })
    };
    assert_eq!(execute(false), execute(true));
}

#[test]
fn node_metadata_does_not_change_512_node_snapshots_election_or_rewards() {
    use crate::tests::mock::{new_test_ext, Test};
    let execute = |keep_metadata| {
        new_test_ext().execute_with(|| {
            let fixture = settlement_fixture_with_metadata::<Test>(512, keep_metadata);
            let subnet = fixture.subnet_id;
            let epoch = fixture.epoch;
            let snapshots = (
                SubnetElectedValidator::<Test>::get(subnet, epoch).encode(),
                SubnetConsensusSubmission::<Test>::get(subnet, epoch).encode(),
                SubnetConsensusAttestorWeights::<Test>::get(subnet, epoch).encode(),
            );
            Network::<Test>::on_initialize(frame_system::Pallet::<Test>::block_number());
            verify_settlement::<Test>(&fixture);
            let next = Network::<Test>::get_current_subnet_epoch_as_u32(subnet);
            assert!(SubnetElectedValidator::<Test>::contains_key(subnet, next));
            // Normalize only metadata and its reverse indexes. Everything else, including all
            // balances, classifications, historical snapshots, election state and events, must match.
            let _ = SubnetNodeMetadata::<Test>::clear_prefix(subnet, u32::MAX, None);
            let _ = PeerIdSubnetNodeId::<Test>::clear_prefix(subnet, u32::MAX, None);
            let _ = BootnodePeerIdSubnetNodeId::<Test>::clear_prefix(subnet, u32::MAX, None);
            let _ = ClientPeerIdSubnetNodeId::<Test>::clear_prefix(subnet, u32::MAX, None);
            let _ = MultiaddrSubnetNodeId::<Test>::clear_prefix(subnet, u32::MAX, None);
            let _ = UniqueParamSubnetNodeId::<Test>::clear_prefix(subnet, u32::MAX, None);
            (
                snapshots,
                sp_io::storage::root(sp_runtime::StateVersion::V1),
            )
        })
    };
    assert_eq!(execute(false), execute(true));
}
