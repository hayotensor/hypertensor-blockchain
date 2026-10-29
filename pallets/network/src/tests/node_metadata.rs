use super::{mock::*, test_utils::*};
use crate::*;
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::Currency, weights::WeightMeter};
use sp_core::OpaquePeerId;

const DEPOSIT: u128 = 10_000_000_000_000_000_000_000;

fn fixture(queued: bool) -> (u32, u32, AccountId) {
    build_activated_subnet(
        b"node-metadata".to_vec(),
        0,
        4,
        DEPOSIT,
        MinSubnetMinStake::<Test>::get(),
    );
    let subnet_id = SubnetName::<Test>::get(b"node-metadata".to_vec()).unwrap();
    let node_id = if queued {
        build_queued_subnet_nodes(subnet_id, 4, 5, DEPOSIT, MinSubnetMinStake::<Test>::get());
        5
    } else {
        4
    };
    let coldkey = Network::get_subnet_node_associated_coldkey(subnet_id, node_id).unwrap();
    (subnet_id, node_id, coldkey)
}

fn maximum(seed: u8) -> NodeMetadata<Test> {
    use crate::utilities::multiaddr::{encode_varint, Multiaddr, DNS4, P2P};
    let peer = |role: u8| {
        let mut id = vec![b'a'; 128];
        id[..2].copy_from_slice(b"Qm");
        id[2] = seed;
        id[3] = role;
        let mut addr = Vec::new();
        encode_varint(DNS4, &mut addr);
        encode_varint(889, &mut addr);
        addr.extend(vec![b'a'; 889]);
        encode_varint(P2P, &mut addr);
        encode_varint(128, &mut addr);
        addr.extend(&id);
        assert!(Multiaddr::verify(&addr).is_ok());
        PeerInfo {
            peer_id: OpaquePeerId(id),
            multiaddr: Some(addr.try_into().unwrap()),
        }
    };
    NodeMetadata {
        peer_info: Some(peer(1)),
        bootnode_peer_info: Some(peer(2)),
        client_peer_info: Some(peer(3)),
        unique: Some(vec![seed; 1024].try_into().unwrap()),
        non_unique: Some(vec![seed; 1024].try_into().unwrap()),
    }
}

fn set(subnet: u32, node: u32, coldkey: &AccountId, m: NodeMetadata<Test>) {
    assert_ok!(Network::update_node_peer_info(
        RuntimeOrigin::signed(coldkey.clone()),
        subnet,
        node,
        m.peer_info
    ));
    assert_ok!(Network::update_node_bootnode_peer_info(
        RuntimeOrigin::signed(coldkey.clone()),
        subnet,
        node,
        m.bootnode_peer_info
    ));
    assert_ok!(Network::update_node_client_peer_info(
        RuntimeOrigin::signed(coldkey.clone()),
        subnet,
        node,
        m.client_peer_info
    ));
    assert_ok!(Network::update_node_unique(
        RuntimeOrigin::signed(coldkey.clone()),
        subnet,
        node,
        m.unique
    ));
    assert_ok!(Network::update_node_non_unique(
        RuntimeOrigin::signed(coldkey.clone()),
        subnet,
        node,
        m.non_unique
    ));
}

fn assert_indexes(subnet: u32, node: u32, m: &NodeMetadata<Test>, present: bool) {
    for (role, peer) in [&m.peer_info, &m.bootnode_peer_info, &m.client_peer_info]
        .into_iter()
        .enumerate()
    {
        if let Some(peer) = peer {
            let exists = match role {
                0 => PeerIdSubnetNodeId::<Test>::contains_key(subnet, &peer.peer_id),
                1 => BootnodePeerIdSubnetNodeId::<Test>::contains_key(subnet, &peer.peer_id),
                _ => ClientPeerIdSubnetNodeId::<Test>::contains_key(subnet, &peer.peer_id),
            };
            assert_eq!(exists, present);
            if let Some(addr) = &peer.multiaddr {
                assert_eq!(
                    MultiaddrSubnetNodeId::<Test>::contains_key(subnet, addr),
                    present
                );
                if present {
                    assert_eq!(MultiaddrSubnetNodeId::<Test>::get(subnet, addr), node);
                }
            }
        }
    }
    if let Some(unique) = &m.unique {
        assert_eq!(
            UniqueParamSubnetNodeId::<Test>::contains_key(subnet, unique),
            present
        );
        if present {
            assert_eq!(UniqueParamSubnetNodeId::<Test>::get(subnet, unique), node);
        }
    }
}

#[test]
fn operational_and_event_encodings_keep_their_intended_shapes() {
    let core = SubnetNode {
        id: 7,
        validator_id: 9,
        classification: SubnetNodeClassification {
            node_class: SubnetNodeClass::Active,
            start_epoch: 11,
        },
    };
    assert_eq!(core.encode().len(), 13);
    assert_eq!(ConsensusSubnetNode::from(&core).encode(), core.encode());
    let m = maximum(42);
    assert_eq!(m.encode().len(), 5528);
    let payload = SubnetNodeRegistrationData::<Test> {
        id: core.id,
        validator_id: core.validator_id,
        peer_info: m.peer_info.clone(),
        bootnode_peer_info: m.bootnode_peer_info.clone(),
        client_peer_info: m.client_peer_info.clone(),
        classification: core.classification.clone(),
        unique: m.unique.clone(),
        non_unique: m.non_unique.clone(),
    };
    assert_eq!(payload.encode().len(), 5541);
    assert_eq!(
        payload.encode(),
        (
            core.id,
            core.validator_id,
            m.peer_info,
            m.bootnode_peer_info,
            m.client_peer_info,
            core.classification,
            m.unique,
            m.non_unique
        )
            .encode()
    );
}

#[test]
fn registration_stores_absent_empty_and_maximum_metadata_in_both_branches() {
    for immediate in [false, true] {
        for m in [
            NodeMetadata::<Test>::default(),
            NodeMetadata {
                unique: Some(Default::default()),
                non_unique: Some(Default::default()),
                ..Default::default()
            },
            maximum(42),
        ] {
            new_test_ext().execute_with(|| {
                let subnet_id = if immediate {
                    build_registered_subnet(
                        b"node-metadata".to_vec(),
                        0,
                        4,
                        DEPOSIT,
                        MinSubnetMinStake::<Test>::get(),
                        true,
                        None,
                    );
                    SubnetName::<Test>::get(b"node-metadata".to_vec()).unwrap()
                } else {
                    fixture(false).0
                };
                let coldkey = account(900_000);
                fund_validator_registration(&coldkey);
                assert_ok!(Network::register_validator(
                    RuntimeOrigin::signed(coldkey.clone()),
                    account(900_001),
                    0,
                    None,
                    None
                ));
                let validator = ColdkeyValidatorId::<Test>::get(&coldkey).unwrap();
                if immediate {
                    NodeRegistrationInitialValidatorIds::<Test>::mutate(subnet_id, |map| {
                        map.as_mut().unwrap().insert(validator, 1);
                    });
                }
                let _ = Balances::deposit_creating(&coldkey, DEPOSIT);
                assert_ok!(Network::register_subnet_node(
                    RuntimeOrigin::signed(coldkey),
                    validator,
                    subnet_id,
                    None,
                    m.peer_info.clone(),
                    m.bootnode_peer_info.clone(),
                    m.client_peer_info.clone(),
                    MinSubnetMinStake::<Test>::get(),
                    m.unique.clone(),
                    m.non_unique.clone(),
                    u128::MAX
                ));
                let node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);
                assert_eq!(
                    SubnetNodeMetadata::<Test>::get(subnet_id, node_id),
                    (!m.is_empty()).then_some(m.clone())
                );
                assert_eq!(
                    SubnetNodesData::<Test>::contains_key(subnet_id, node_id),
                    immediate
                );
                assert_eq!(
                    QueuedSubnetNodesData::<Test>::contains_key(subnet_id, node_id),
                    !immediate
                );
                assert_indexes(subnet_id, node_id, &m, true);
                assert_eq!(
                    Network::get_subnet_node_info(subnet_id, node_id)
                        .unwrap()
                        .unique,
                    m.unique
                );
            });
        }
    }
}

#[test]
fn edits_replace_indexes_without_rewriting_core_or_queue() {
    for queued in [false, true] {
        new_test_ext().execute_with(|| {
            let (subnet, node, coldkey) = fixture(queued);
            let core = if queued {
                QueuedSubnetNodesData::<Test>::get(subnet, node)
            } else {
                SubnetNodesData::<Test>::get(subnet, node)
            };
            let queue = SubnetNodeQueue::<Test>::get(subnet);
            set(subnet, node, &coldkey, Default::default());
            assert!(!SubnetNodeMetadata::<Test>::contains_key(subnet, node));
            let first = maximum(42);
            set(subnet, node, &coldkey, first.clone());
            assert_indexes(subnet, node, &first, true);
            let second = maximum(43);
            set(subnet, node, &coldkey, second.clone());
            assert_indexes(subnet, node, &first, false);
            assert_indexes(subnet, node, &second, true);
            set(subnet, node, &coldkey, Default::default());
            assert_indexes(subnet, node, &second, false);
            assert!(!SubnetNodeMetadata::<Test>::contains_key(subnet, node));
            assert_ok!(Network::update_node_unique(
                RuntimeOrigin::signed(coldkey),
                subnet,
                node,
                Some(Default::default())
            ));
            assert_eq!(
                SubnetNodeMetadata::<Test>::get(subnet, node)
                    .unwrap()
                    .unique,
                Some(Default::default())
            );
            assert_eq!(
                if queued {
                    QueuedSubnetNodesData::<Test>::get(subnet, node)
                } else {
                    SubnetNodesData::<Test>::get(subnet, node)
                },
                core
            );
            assert_eq!(SubnetNodeQueue::<Test>::get(subnet), queue);
        });
    }
}

#[test]
fn errors_preserve_metadata_indexes_and_events() {
    for queued in [false, true] {
        new_test_ext().execute_with(|| {
            let (subnet, node, coldkey) = fixture(queued);
            let m = maximum(42);
            set(subnet, node, &coldkey, m.clone());
            assert_noop!(
                Network::update_node_unique(
                    RuntimeOrigin::signed(account(999_000)),
                    subnet,
                    node,
                    None
                ),
                Error::<Test>::NotKeyOwner
            );
            assert_noop!(
                Network::update_node_peer_info(
                    RuntimeOrigin::signed(coldkey.clone()),
                    subnet,
                    node,
                    Some(PeerInfo {
                        peer_id: OpaquePeerId(vec![0]),
                        multiaddr: None
                    })
                ),
                Error::<Test>::InvalidPeerId
            );
            let mut invalid_address = maximum(43).peer_info.unwrap();
            invalid_address.multiaddr = Some(vec![0xff].try_into().unwrap());
            assert!(
                Network::do_verify_multiaddr(invalid_address.multiaddr.as_ref().unwrap()).is_err()
            );
            let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert!(Network::update_node_peer_info(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet,
                node,
                Some(invalid_address)
            )
            .is_err());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
            // A different live node owns this value. The old unique index must survive rejection.
            let conflict: NetworkBytes<Test> = vec![44; 1024].try_into().unwrap();
            assert_ok!(Network::do_update_node_unique(
                subnet,
                1,
                Some(conflict.clone())
            ));
            assert_noop!(
                Network::update_node_unique(
                    RuntimeOrigin::signed(coldkey.clone()),
                    subnet,
                    node,
                    Some(conflict.clone())
                ),
                Error::<Test>::UniqueParameterTaken
            );
            // The internal entry point is transactional too, not only the extrinsic wrapper.
            assert_noop!(
                Network::do_update_unique(subnet, node, Some(conflict)),
                Error::<Test>::UniqueParameterTaken
            );
            assert_indexes(subnet, node, &m, true);
            let other = SubnetNodeMetadata::<Test>::get(subnet, 1)
                .unwrap()
                .peer_info;
            assert_noop!(
                Network::update_node_peer_info(
                    RuntimeOrigin::signed(coldkey.clone()),
                    subnet,
                    node,
                    other
                ),
                Error::<Test>::PeerIdExist
            );
            SubnetNodesData::<Test>::remove(subnet, node);
            QueuedSubnetNodesData::<Test>::remove(subnet, node);
            assert_noop!(
                Network::update_node_unique(RuntimeOrigin::signed(coldkey), subnet, node, None),
                Error::<Test>::InvalidSubnetNodeId
            );
            assert!(Network::get_subnet_node_info(subnet, node).is_none());
            assert!(Network::rpc_get_subnet_node_info(subnet, node).is_none());
        });
    }
}

#[test]
fn queued_edits_survive_admission_and_probation_promotion() {
    new_test_ext().execute_with(|| {
        let (subnet, node, coldkey) = fixture(true);
        let m = maximum(42);
        set(subnet, node, &coldkey, m.clone());
        let start = QueuedSubnetNodesData::<Test>::get(subnet, node)
            .classification
            .start_epoch;
        let epoch = start + SubnetNodeQueueEpochs::<Test>::get(subnet) + 1;
        ChurnLimitMultiplier::<Test>::insert(subnet, 1);
        Network::handle_registration_queue(&mut WeightMeter::new(), subnet, epoch);
        assert!(!QueuedSubnetNodesData::<Test>::contains_key(
            subnet, node
        ));
        assert!(SubnetNodesData::<Test>::contains_key(subnet, node));
        assert!(!SubnetNodeQueue::<Test>::get(subnet)
            .iter()
            .any(|n| n.id == node));
        assert!(Network::promote_probation_node_to_active(
            subnet, node, epoch, &Default::default()
        ));
        assert_eq!(
            SubnetNodeMetadata::<Test>::get(subnet, node),
            Some(m.clone())
        );
        assert_indexes(subnet, node, &m, true);
    });
}

#[test]
fn pause_unpause_and_key_rotations_keep_metadata_attached_to_node_id() {
    new_test_ext().execute_with(|| {
        let (subnet, node, coldkey) = fixture(true);
        let m = maximum(42);
        set(subnet, node, &coldkey, m.clone());
        let epoch = SubnetsData::<Test>::get(subnet)
            .unwrap()
            .consensus_eligible_from_subnet_epoch
            .unwrap()
            + SubnetPauseCooldownEpochs::<Test>::get();
        set_block_to_subnet_slot_epoch(epoch, subnet);
        let owner = SubnetOwner::<Test>::get(subnet).unwrap();
        assert_ok!(Network::owner_pause_subnet(
            RuntimeOrigin::signed(owner.clone()),
            subnet
        ));
        let start = QueuedSubnetNodesData::<Test>::get(subnet, node)
            .classification
            .start_epoch;
        increase_epochs(2);
        assert_ok!(Network::owner_unpause_subnet(
            RuntimeOrigin::signed(owner),
            subnet
        ));
        assert_eq!(
            QueuedSubnetNodesData::<Test>::get(subnet, node)
                .classification
                .start_epoch,
            start + 2
        );
        assert_eq!(
            SubnetNodeQueue::<Test>::get(subnet)
                .iter()
                .find(|n| n.id == node)
                .unwrap()
                .classification
                .start_epoch,
            start + 2
        );
        let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
        assert_ok!(Network::update_validator_hotkey(
            RuntimeOrigin::signed(coldkey.clone()),
            validator,
            account(900_001)
        ));
        assert_ok!(Network::update_node_hotkey(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet,
            node,
            Some(account(900_002))
        ));
        assert_ok!(Network::update_validator_coldkey(
            RuntimeOrigin::signed(coldkey),
            validator,
            account(900_003)
        ));
        assert_eq!(SubnetNodeMetadata::<Test>::get(subnet, node), Some(m));
        assert_eq!(
            Network::get_subnet_node_info(subnet, node).unwrap().coldkey,
            account(900_003)
        );
    });
}

#[test]
fn quarantine_retains_metadata_until_physical_cleanup_and_retains_withdrawal_state() {
    for queued in [false, true] {
        new_test_ext().execute_with(|| {
            let (subnet, node, coldkey) = fixture(queued);
            let m = maximum(42);
            set(subnet, node, &coldkey, m.clone());
            if queued {
                PendingQueuedNodeRemovals::<Test>::mutate(subnet, |ids| {
                    ids.try_insert(node).unwrap();
                });
            } else {
                PendingAdmittedNodeRemovals::<Test>::mutate(subnet, |ids| {
                    ids.try_insert(node).unwrap();
                });
            }
            assert_eq!(
                SubnetNodeMetadata::<Test>::get(subnet, node),
                Some(m.clone())
            );
            assert_indexes(subnet, node, &m, true);
            let stake = NodeSubnetStake::<Test>::get(node, subnet);
            let validator = SubnetNodeValidatorId::<Test>::get(subnet, node);
            // Preserve the existing pending-removal short circuit: success performs cleanup.
            assert_ok!(Network::update_node_non_unique(
                RuntimeOrigin::signed(coldkey),
                subnet,
                node,
                Some(Default::default())
            ));
            assert!(!SubnetNodeMetadata::<Test>::contains_key(subnet, node));
            assert_indexes(subnet, node, &m, false);
            assert!(!SubnetNodesData::<Test>::contains_key(subnet, node));
            assert!(!QueuedSubnetNodesData::<Test>::contains_key(
                subnet, node
            ));
            assert_eq!(NodeSubnetStake::<Test>::get(node, subnet), stake);
            assert_eq!(SubnetNodeValidatorId::<Test>::get(subnet, node), validator);
            System::assert_last_event(
                Event::<Test>::SubnetNodeRemoved {
                    subnet_id: subnet,
                    subnet_node_id: node,
                }
                .into(),
            );
        });
    }
}

#[test]
fn whole_subnet_removal_clears_active_and_queued_metadata_and_indexes() {
    new_test_ext().execute_with(|| {
        let (subnet, node, coldkey) = fixture(true);
        let m = maximum(42);
        set(subnet, node, &coldkey, m.clone());
        let stake = NodeSubnetStake::<Test>::get(node, subnet);
        let validator = SubnetNodeValidatorId::<Test>::get(subnet, node);
        Network::do_remove_subnet(subnet, SubnetRemovalReason::MinReputation);
        assert_eq!(SubnetNodeMetadata::<Test>::iter_prefix(subnet).count(), 0);
        assert_indexes(subnet, node, &m, false);
        assert_eq!(NodeSubnetStake::<Test>::get(node, subnet), stake);
        assert_eq!(SubnetNodeValidatorId::<Test>::get(subnet, node), validator);
    });
}

#[test]
fn composed_readers_include_metadata_and_missing_metadata_does_not_hide_core() {
    new_test_ext().execute_with(|| {
        let (subnet, node, coldkey) = fixture(false);
        let m = maximum(42);
        set(subnet, node, &coldkey, m.clone());
        let info = Network::get_subnet_node_info(subnet, node).unwrap();
        assert_eq!(info.peer_info, m.peer_info);
        assert_eq!(info.bootnode_peer_info, m.bootnode_peer_info);
        assert_eq!(info.client_peer_info, m.client_peer_info);
        assert_eq!(info.unique, m.unique);
        let validator = SubnetNodeValidatorId::<Test>::get(subnet, node).unwrap();
        let nodes =
            Network::rpc_get_subnet_nodes(subnet, network_rpc_types::PageRequest::default())
                .unwrap();
        assert!(nodes.items.iter().any(|n| n.subnet_node_id == node
            && n.unique.as_ref().map(|b| b.0.as_slice())
                == m.unique.as_ref().map(|b| b.as_slice())));
        let owned =
            Network::rpc_get_validator_nodes(validator, network_rpc_types::PageRequest::default())
                .unwrap();
        assert_eq!(
            owned
                .items
                .iter()
                .find(|n| n.subnet_node_id == node)
                .unwrap(),
            nodes
                .items
                .iter()
                .find(|n| n.subnet_node_id == node)
                .unwrap()
        );
        assert!(Network::get_validator_subnet_nodes_info(validator)
            .iter()
            .any(|n| n.subnet_node_id == node && n.unique == m.unique));
        assert!(Network::get_classified_subnet_nodes_info(
            subnet,
            &SubnetNodeClass::Active,
            u32::MAX
        )
        .iter()
        .any(|n| n.subnet_node_id == node && n.unique == m.unique));
        assert!(Network::rpc_get_bootnodes(subnet)
            .unwrap()
            .admitted_nodes
            .iter()
            .any(|p| p.peer_id.0 == m.bootnode_peer_info.as_ref().unwrap().peer_id.0));
        set(subnet, node, &coldkey, Default::default());
        assert!(Network::rpc_get_subnet_node_info(subnet, node).is_some());
        let info = Network::get_subnet_node_info(subnet, node).unwrap();
        assert!(info.peer_info.is_none() && info.unique.is_none());
        SubnetNodeMetadata::<Test>::insert(subnet, 999_999, maximum(43));
        assert!(Network::get_subnet_node_info(subnet, 999_999).is_none());
    });
}
