//! Matrix capacity fixtures. All setup is outside measured execution.
//! The runner lifts only MATRIX_MAX_NODES in an isolated research checkout.
use super::*;

pub(super) struct Fixture {
    pub subnet: u32,
    pub epoch: u32,
    pub nodes: u32,
    pub active: u32,
}

pub(super) fn raw_scores(nodes: u32, scorer: u32) -> MatrixScores {
    (1..=nodes)
        .filter(|id| *id != scorer)
        .map(|id| SubnetNodeConsensusData {
            subnet_node_id: id,
            // Dense absolute scores spanning the supported Q18 range.
            score: u128::from_le_bytes(blake2_128(&(scorer, id).encode())) % PERCENTAGE_FACTOR_U128
                + 1,
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

// The other physical subnets can be full. Distribute their memberships over this
// round's identities, respecting the per-validator cap. Unlike filling every map
// to 512 independently, this is reachable within the network's total node bound.
fn load_allocation_maps<T: Config>(subnet: u32, nodes: u32) {
    let per_subnet = T::MaxSubnetNodesUpperBound::get();
    let external = T::MaxPhysicalSubnetsUpperBound::get()
        .saturating_sub(1)
        .saturating_mul(per_subnet)
        .min(nodes.saturating_mul(T::MaxValidatorNodesUpperBound::get().saturating_sub(1)));
    for id in 1..=nodes {
        let validator = SubnetNodeValidatorId::<T>::get(subnet, id).unwrap();
        let mut owned = ValidatorSubnetNodes::<T>::get(validator);
        let mut keys = vec![(subnet, id)];
        for index in (id - 1..external).step_by(nodes as usize) {
            let other_subnet = subnet + 1 + index / per_subnet;
            let other_node = 1 + index % per_subnet;
            owned.entry(other_subnet).or_default().insert(other_node);
            keys.push((other_subnet, other_node));
        }
        let count = keys.len() as u128;
        let base = PERCENTAGE_FACTOR_U128 / count;
        let mut weights: BTreeMap<_, _> = keys.into_iter().map(|key| (key, base)).collect();
        *weights.get_mut(&(subnet, id)).unwrap() += PERCENTAGE_FACTOR_U128 % count;
        ValidatorSubnetNodes::<T>::insert(validator, owned);
        ValidatorNodeDelegateStakeWeights::<T>::insert(validator, weights);
        TotalValidatorNodes::<T>::insert(validator, count as u32);
    }
}

pub(super) fn fixture<T: Config>(nodes: u32, probation: u32, rows: bool) -> Fixture {
    assert!(nodes <= MATRIX_MAX_NODES && nodes <= T::MaxSubnetNodesUpperBound::get());
    let subnet = capacity::promotion_elected_fixture::<T>(nodes, false);
    // Reuse the shared funded fixture; mechanism selection itself is outside timing.
    SubnetsData::<T>::mutate(subnet, |data| {
        data.as_mut().unwrap().consensus_mechanism = ConsensusMechanism::Matrix;
    });
    load_allocation_maps::<T>(subnet, nodes);
    let active = nodes - probation;
    assert!(active >= 3);
    for id in active + 1..=nodes {
        assert_ok!(Network::<T>::remove_node_from_election_slot(subnet, id));
        SubnetNodesData::<T>::mutate(subnet, id, |node| {
            node.classification.node_class = SubnetNodeClass::Probation;
        });
        SubnetNodeReputation::<T>::insert(subnet, id, PERCENTAGE_FACTOR_U128);
        SubnetNodeProbationProgress::<T>::insert(subnet, id, u32::MAX - 1);
    }
    let epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet);
    Network::<T>::open_matrix_round(subnet, epoch);
    let round = MatrixRounds::<T>::get(subnet, epoch).expect("Matrix round opens");
    assert_eq!(round.identities.len(), active as usize);
    assert_eq!(round.nodes.len(), nodes as usize);
    if rows {
        for id in 1..=active {
            let scores = raw_scores(nodes, id);
            let commitment =
                Network::<T>::matrix_commitment(subnet, epoch, id, &round, &scores, &[27; 32]);
            MatrixCommitments::<T>::insert((subnet, epoch, id), commitment);
            MatrixRows::<T>::insert((subnet, epoch, id), scores);
        }
    }
    FinalSubnetEmissionWeights::<T>::insert(
        epoch + 1,
        DistributionData {
            subnets_emissions: 10_000 * PERCENTAGE_FACTOR_U128,
            subnet_weights: BTreeMap::from([(subnet, PERCENTAGE_FACTOR_U128)]),
        },
    );
    SubnetRewardCapacitor::<T>::insert(subnet, 1_000 * PERCENTAGE_FACTOR_U128);
    frame_system::Pallet::<T>::reset_events();
    Fixture {
        subnet,
        epoch,
        nodes,
        active,
    }
}

pub(super) fn advance<T: Config>(f: &Fixture) {
    set_block_to_subnet_slot_epoch::<T>(f.epoch + 1, f.subnet);
    frame_system::Pallet::<T>::reset_events();
}

pub(super) fn verify<T: Config>(f: &Fixture, failed_credits: bool) {
    assert_eq!(
        ConsensusRoundSettlementStatus::<T>::get(f.subnet, f.epoch),
        Some(true)
    );
    let result = MatrixResults::<T>::get(f.subnet, f.epoch).unwrap();
    assert_eq!(result.outcome, MatrixOutcome::Accepted);
    assert_eq!(result.nodes.len(), f.nodes as usize);
    assert_eq!(
        result.nodes.values().map(|n| n.weight).sum::<u128>(),
        PERCENTAGE_FACTOR_U128
    );
    let next = MatrixRounds::<T>::get(f.subnet, f.epoch + 1).expect("next snapshot persisted");
    assert_eq!(next.identities.len(), f.nodes as usize);
    if failed_credits {
        assert_eq!(
            PendingConsensusOperations::<T>::iter()
                .filter(|(_, op)| matches!(op.operation, ConsensusOperation::NodeReward { .. }))
                .count(),
            f.active as usize
        );
    } else {
        assert_eq!(PendingConsensusOperations::<T>::iter().count(), 0);
        for id in 1..=f.active {
            assert!(NodeSubnetStake::<T>::get(id, f.subnet) > DEFAULT_SUBNET_NODE_STAKE);
        }
    }
}

/// Maximum successful submission reads: full subnet metadata, previous resume,
/// all other admitted nodes quarantined after the snapshot, and a full queued set.
pub(super) fn max_submission_state<T: Config>(f: &Fixture) {
    SubnetsData::<T>::mutate(f.subnet, |subnet| {
        let subnet = subnet.as_mut().unwrap();
        subnet.name = vec![b'n'; T::MaxVectorLength::get() as usize];
        subnet.repo = vec![b'r'; T::MaxUrlLength::get() as usize];
        subnet.description = vec![b'd'; T::MaxVectorLength::get() as usize];
        subnet.misc = vec![b'm'; T::MaxVectorLength::get() as usize];
    });
    let pending: BoundedBTreeSet<u32, T::MaxSubnetNodesUpperBound> =
        (2..=f.nodes).collect::<BTreeSet<_>>().try_into().unwrap();
    PendingAdmittedNodeRemovals::<T>::insert(f.subnet, pending);
    let pending: BoundedBTreeSet<u32, T::MaxRegisteredNodesUpperBound> = (f.nodes + 1
        ..=f.nodes + T::MaxRegisteredNodesUpperBound::get())
        .collect::<BTreeSet<_>>()
        .try_into()
        .unwrap();
    PendingQueuedNodeRemovals::<T>::insert(f.subnet, pending);
    let round = MatrixRounds::<T>::get(f.subnet, f.epoch).unwrap();
    LastNetworkUnpauseBlock::<T>::put(round.start_block - 1);
}
