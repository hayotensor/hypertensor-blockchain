use super::*;

impl<T: Config> Pallet<T> {
    pub(crate) fn settle_matrix_round(subnet_id: u32, current_epoch: u32) {
        let Some(epoch) = current_epoch.checked_sub(1) else {
            return;
        };
        if Self::is_consensus_round_settled(subnet_id, epoch) {
            return;
        }
        let Some(round) = MatrixRounds::<T>::get(subnet_id, epoch) else {
            return;
        };
        let rows: BTreeMap<_, _> = MatrixRows::<T>::iter_prefix((subnet_id, epoch)).collect();
        // Validate stored evidence before arithmetic; malformed state never invents a decision.
        let sum = round
            .stake
            .weights
            .values()
            .fold(U256::zero(), |sum, w| sum + U256::from(*w));
        let valid = round.nodes.len() >= round.parameters.min_nodes as usize
            && round.nodes.len() <= MATRIX_MAX_NODES as usize
            && round.nodes.windows(2).all(|n| n[0].id < n[1].id)
            && round.parameters.is_valid()
            && round.identities.keys().eq(round.stake.weights.keys())
            && round.identities.iter().all(|(id, validator)| {
                round.nodes.iter().any(|n| {
                    n.id == *id
                        && n.validator_id == *validator
                        && n.classification.node_class == SubnetNodeClass::Active
                })
            })
            && sum == U256::from(round.stake.total_weight)
            && rows
                .iter()
                .all(|(id, row)| round.identities.contains_key(id) && round.valid_scores(*id, row));
        let result = if valid && !Self::consensus_round_precedes_network_resume(subnet_id, epoch) {
            math::calculate(&round, &rows)
        } else {
            MatrixResult {
                outcome: MatrixOutcome::Cancelled,
                revealed_weight: 0,
                revealed_nodes: 0,
                nodes: BTreeMap::new(),
            }
        };
        if result.outcome != MatrixOutcome::Accepted {
            Self::cancel_consensus_round(subnet_id, epoch);
            if result.outcome == MatrixOutcome::NoScores {
                // Scoring is optional. An empty valid round has no behavioral reputation
                // changes, but existing below-minimum reputations still stage removal.
                // Fence the outcome even if the shared retention helper found no marker.
                Self::finalize_consensus_round(subnet_id, epoch);
                Self::update_matrix_reputations(
                    &mut WeightMeter::new(),
                    subnet_id,
                    current_epoch,
                    &round,
                    &result,
                );
            }
        } else {
            let mut meter = WeightMeter::new();
            let allocation = FinalSubnetEmissionWeights::<T>::get(current_epoch);
            let (mut rewards, _) = Self::calculate_rewards_with_policy(
                allocation.subnets_emissions,
                allocation
                    .subnet_weights
                    .get(&subnet_id)
                    .copied()
                    .unwrap_or(0),
                &round.policy,
            );
            let remaining = Self::prepare_consensus_capacitor_release(
                &mut meter,
                subnet_id,
                &round.policy,
                result.nodes.values().any(|n| n.weight > 0),
                &mut rewards,
            );
            // Fence the outcome before credits. Every failed credit is retained independently.
            Self::finalize_consensus_round(subnet_id, epoch);
            if let Some(balance) = remaining {
                SubnetRewardCapacitor::<T>::insert(subnet_id, balance);
            }
            let mut deferred = Vec::new();
            let reward_eligible = Self::update_matrix_reputations(
                &mut meter,
                subnet_id,
                current_epoch,
                &round,
                &result,
            );
            let participation_ratio = Self::percent_div(
                result.revealed_nodes as u128,
                round.identities.len() as u128,
            );
            let mut node_rewards = Vec::new();
            let mut node_delegate_stake_rewards = Vec::new();
            let mut node_delegate_account_allocations = Vec::new();
            if result
                .nodes
                .values()
                .filter(|n| n.performance > round.parameters.performance_threshold)
                .count()
                >= round.policy.min_subnet_nodes as usize
            {
                Self::increase_subnet_reputation(
                    subnet_id,
                    round.policy.in_consensus_subnet_reputation_factor,
                    participation_ratio,
                );
            }
            Self::settle_subnet_owner_reward(
                &mut meter,
                &mut deferred,
                subnet_id,
                rewards.subnet_owner_reward,
            );
            for node in &round.nodes {
                if !reward_eligible.contains(&node.id) {
                    continue;
                }
                let metrics = &result.nodes[&node.id];
                let amount = Self::percent_mul(metrics.weight, rewards.subnet_node_rewards);
                if amount == 0 {
                    continue;
                }
                if let Some((credited, pool, account)) = Self::settle_consensus_node_reward(
                    &mut meter,
                    &mut deferred,
                    subnet_id,
                    node.id,
                    node.validator_id,
                    amount,
                ) {
                    node_rewards.push((node.id, credited));
                    if let Some(pool) = pool {
                        node_delegate_stake_rewards.push(pool);
                    }
                    if let Some(account) = account {
                        node_delegate_account_allocations.push((node.id, account));
                    }
                }
            }
            let delegate_stake_reward = Self::handle_subnet_delegate_stake_reward(
                &mut meter,
                &mut deferred,
                subnet_id,
                rewards.delegate_stake_rewards,
            );
            Self::enqueue_consensus_rewards(subnet_id, &deferred);
            Self::deposit_event(Event::SubnetRewards {
                subnet_id,
                node_rewards,
                delegate_stake_reward,
                node_delegate_stake_rewards,
                node_delegate_account_allocations,
            });
        }
        // Weights are the original allocation. Withheld credits do not redistribute those shares.
        let outcome = result.outcome;
        MatrixResults::<T>::insert(subnet_id, epoch, &result);
        Self::deposit_event(Event::MatrixRoundSettled {
            subnet_id,
            subnet_epoch: epoch,
            outcome,
        });
    }

    /// Compute each node's reputation once, without any participation or identity quorum.
    /// Missing evidence is not a poor assessment, and scoring others is optional.
    fn update_matrix_reputations(
        meter: &mut WeightMeter,
        subnet_id: u32,
        current_epoch: u32,
        round: &MatrixRound,
        result: &MatrixResult,
    ) -> BTreeSet<u32> {
        let mut pending = PendingAdmittedNodeRemovals::<T>::get(subnet_id);
        let mut new_pending = BoundedVec::default();
        let mut dirty = false;
        let mut reward_eligible = BTreeSet::new();
        let factors = round.policy.reputation_factors;
        for node in &round.nodes {
            if pending.contains(&node.id) {
                continue;
            }
            let Some(previous) = SubnetNodeReputation::<T>::get(subnet_id, node.id) else {
                continue;
            };
            let metrics = &result.nodes[&node.id];
            let has_received_assessment = metrics.has_received_assessment;
            let mut reputation = previous;
            if reputation >= round.policy.min_subnet_node_reputation {
                if node.classification.node_class == SubnetNodeClass::Active {
                    if metrics.has_accuracy_reference {
                        let (increase, decrease) =
                            math::reputation_rates(&round.parameters, metrics.accuracy);
                        reputation = reputation
                            .saturating_add(Self::percent_mul(reputation, increase))
                            .min(PERCENTAGE_FACTOR_U128);
                        reputation = Self::decrease_rep(reputation, decrease, None);
                    }
                } else if metrics.performance > round.parameters.performance_threshold {
                    reputation = Self::increase_rep(reputation, factors.included_increase, None);
                }
                // A lone revealer has no external assessment. Do not penalize it for
                // other nodes failing to provide a score or for the resulting zero weight.
                if has_received_assessment
                    && metrics.performance <= round.parameters.performance_threshold
                {
                    reputation = Self::decrease_rep(reputation, factors.absent_decrease, None);
                }
                if node.classification.node_class == SubnetNodeClass::Active {
                    if has_received_assessment
                        && metrics.weight < round.policy.min_weight_decrease_reputation_threshold
                    {
                        reputation =
                            Self::decrease_rep(reputation, factors.below_min_weight_decrease, None);
                    }
                }
            }
            Self::persist_node_reputation_change(subnet_id, node.id, previous, reputation);
            if reputation < round.policy.min_subnet_node_reputation {
                dirty |= Self::stage_pending_admitted_node_removal(
                    subnet_id,
                    &mut pending,
                    &mut new_pending,
                    node.id,
                );
                continue;
            }
            if node.classification.node_class == SubnetNodeClass::Probation {
                if has_received_assessment {
                    if metrics.performance > round.parameters.performance_threshold {
                        Self::handle_probation_node(
                            meter,
                            subnet_id,
                            node.id,
                            reputation,
                            round.parameters.probation_promotion_reputation,
                            round.policy.probation_rounds,
                            current_epoch,
                            &pending,
                        );
                    } else {
                        SubnetNodeProbationProgress::<T>::remove(subnet_id, node.id);
                    }
                }
                continue;
            }
            reward_eligible.insert(node.id);
        }
        if dirty {
            PendingAdmittedNodeRemovals::<T>::insert(subnet_id, pending);
        }
        Self::deposit_pending_node_removals(subnet_id, new_pending, BoundedVec::default());
        reward_eligible
    }
}
