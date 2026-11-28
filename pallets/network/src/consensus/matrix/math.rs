//! Integer-only Matrix scoring. Stake transformations remain in the shared snapshot helper.
use super::*;

/// Largest-remainder normalization for reward shares. At most 64 inputs, each
/// bounded by Q^2 in the blend; totals and multiplication by Q fit in U256.
pub(crate) fn normalize(values: &BTreeMap<u32, U256>) -> BTreeMap<u32, u128> {
    let total = values.values().fold(U256::zero(), |a, b| a + *b);
    let mut result = BTreeMap::new();
    if total.is_zero() {
        return values.keys().map(|id| (*id, 0)).collect();
    }
    let mut remainders = Vec::new();
    let mut assigned = 0u128;
    for (&id, &value) in values {
        let product = value * U256::from(PERCENTAGE_FACTOR_U128);
        let share = (product / total).as_u128();
        assigned += share;
        result.insert(id, share);
        remainders.push((product % total, id));
    }
    remainders.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for (_, id) in remainders
        .into_iter()
        .take((PERCENTAGE_FACTOR_U128 - assigned) as usize)
    {
        *result.get_mut(&id).expect("input key exists") += 1;
    }
    result
}

pub(crate) fn calculate(round: &MatrixRound, rows: &BTreeMap<u32, MatrixScores>) -> MatrixResult {
    let q = PERCENTAGE_FACTOR_U128;
    let informative: BTreeMap<_, _> = rows
        .iter()
        .filter(|(_, row)| !row.is_empty())
        .filter(|(id, _)| round.identities.contains_key(id))
        .map(|(&id, row)| (id, row))
        .collect();
    let mut columns: BTreeMap<u32, U256> =
        round.nodes.iter().map(|n| (n.id, U256::zero())).collect();
    let mut assessed_weight: BTreeMap<u32, u128> = round.nodes.iter().map(|n| (n.id, 0)).collect();
    let mut total = 0u128;
    for (&id, row) in &informative {
        let weight = round.stake.weights.get(&id).copied().unwrap_or(0);
        total += weight;
        for score in *row {
            let contribution = U256::from(weight) * U256::from(score.score);
            *columns.entry(score.subnet_node_id).or_default() += contribution;
            *assessed_weight.entry(score.subnet_node_id).or_default() += weight;
        }
    }
    // Every nonempty reveal is usable, regardless of turnout, identity count,
    // or its share of the frozen voting total. Zero voting weight remains zero
    // influence; accepting a row does not guarantee a positive emission weight.
    let has_scores = !informative.is_empty();
    let mut nodes = BTreeMap::new();
    for node in &round.nodes {
        let denominator = assessed_weight[&node.id];
        let performance = if denominator == 0 {
            0
        } else {
            (columns[&node.id] / U256::from(denominator)).as_u128()
        };
        nodes.insert(
            node.id,
            MatrixNodeResult {
                performance,
                has_received_assessment: denominator > 0,
                ..Default::default()
            },
        );
    }
    for (&id, row) in &informative {
        let scorer_weight = round.stake.weights.get(&id).copied().unwrap_or(0);
        let mut error = 0u128;
        let mut comparable = 0u128;
        for score in *row {
            let target = score.subnet_node_id;
            let denominator = assessed_weight[&target] - scorer_weight;
            // Omitted targets supply no evidence. A scorer cannot define its own
            // reference; other nodes (including siblings) supply weighted evidence.
            if denominator == 0 {
                continue;
            }
            let sum = columns[&target] - U256::from(scorer_weight) * U256::from(score.score);
            let reference = (sum / U256::from(denominator)).as_u128();
            error += score.score.abs_diff(reference);
            comparable += 1;
        }
        // Scores have a common absolute Q18 scale. Mean absolute error compares
        // overlapping targets without penalizing different sector sizes. A zero
        // reference is meaningful; only missing external weight is unavailable.
        let has_accuracy_reference = comparable > 0;
        let accuracy = if has_accuracy_reference {
            q - error / comparable
        } else {
            0
        };
        let metrics = nodes.get_mut(&id).expect("submitter is a target");
        metrics.accuracy = accuracy;
        metrics.has_accuracy_reference = has_accuracy_reference;
    }
    if has_scores {
        // Being assessed enables rewards. Scoring is optional: workers have zero
        // accuracy but still contribute received performance to the reward blend.
        // An explicit zero assessment remains eligible for accuracy rewards.
        let eligible: Vec<_> = round
            .nodes
            .iter()
            .filter(|node| {
                node.classification.node_class == SubnetNodeClass::Active
                    && nodes[&node.id].has_received_assessment
            })
            .map(|node| node.id)
            .collect();
        let accuracies = eligible
            .iter()
            .map(|id| (*id, U256::from(nodes[id].accuracy)))
            .collect();
        let performances = eligible
            .iter()
            .map(|id| (*id, U256::from(nodes[id].performance)))
            .collect();
        let a = normalize(&accuracies);
        let p = normalize(&performances);
        let blend = a
            .iter()
            .map(|(&id, &accuracy)| {
                (
                    id,
                    U256::from(round.parameters.accuracy_weight) * U256::from(accuracy)
                        + U256::from(q - round.parameters.accuracy_weight) * U256::from(p[&id]),
                )
            })
            .collect();
        for (id, weight) in normalize(&blend) {
            nodes.get_mut(&id).expect("reward cohort exists").weight = weight;
        }
    }
    MatrixResult {
        outcome: if has_scores {
            MatrixOutcome::Accepted
        } else {
            MatrixOutcome::NoScores
        },
        revealed_weight: total,
        revealed_nodes: informative.len() as u32,
        nodes,
    }
}

/// Q18 power-curve rate with bounded integer arithmetic. The binary algorithm
/// takes at most 32 iterations for a u32 exponent, independent of its magnitude.
fn power_rate(maximum: u128, gap: u128, span: u128, mut exponent: u32) -> u128 {
    let q = U256::from(PERCENTAGE_FACTOR_U128);
    if exponent == 2 {
        // Exact single-division evaluation for the common quadratic case.
        return (U256::from(maximum) * U256::from(gap) * U256::from(gap)
            / (U256::from(span) * U256::from(span)))
        .as_u128();
    }
    let mut base = U256::from(gap) * q / U256::from(span);
    let mut power = q;
    while exponent > 0 {
        if exponent & 1 == 1 {
            power = power * base / q;
        }
        exponent >>= 1;
        if exponent > 0 {
            base = base * base / q;
        }
    }
    (U256::from(maximum) * power / q).as_u128()
}

/// Q18 (increase, decrease) rates for a valid independent accuracy reference.
/// Each direction uses its snapshotted governance-selected exponent.
pub(crate) fn reputation_rates(parameters: &MatrixParameters, accuracy: u128) -> (u128, u128) {
    let q = PERCENTAGE_FACTOR_U128;
    let distance = q.saturating_sub(accuracy);
    let tolerance = parameters.reputation_tolerance;
    if distance == 0 {
        // Zero tolerance still rewards a perfect match without dividing by zero.
        (parameters.max_reputation_increase, 0)
    } else if distance < tolerance {
        (
            power_rate(
                parameters.max_reputation_increase,
                tolerance - distance,
                tolerance,
                parameters.reputation_increase_exponent,
            ),
            0,
        )
    } else if distance > tolerance {
        (
            0,
            power_rate(
                parameters.max_reputation_decrease,
                distance - tolerance,
                q - tolerance,
                parameters.reputation_decrease_exponent,
            ),
        )
    } else {
        (0, 0)
    }
}
