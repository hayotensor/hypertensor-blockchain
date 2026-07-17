# Consensus

Consensus is the per-subnet process that turns an elected validator node's view of subnet performance into on-chain rewards, reputation changes, queue decisions, and penalties.

Consensus is not global block production. It is a subnet-level incentives and attestation system. Each active subnet runs its own epoch schedule, elects one subnet node to propose consensus data for that subnet epoch, and asks the eligible validator-class subnet nodes to attest to that proposal.

At a high level, consensus answers four questions for each subnet epoch:

- Which subnet nodes performed well enough to receive emissions?
- Did enough validator-class nodes agree with the proposed scoring data?
- Should queued nodes be prioritized or removed?
- Should the proposer, attestors, subnet, or subnet nodes be rewarded or penalized?

## Participants

### Subnet Nodes

Subnet nodes are the entities being scored and rewarded. A subnet node can move through several classifications:

- `Registered`: the node is registered but not active in the subnet.
- `Idle`: the node has activated from the registration queue but is not yet included in consensus scoring.
- `Included`: the node can appear in consensus score data.
- `Validator`: the node can be elected to propose consensus data and can attest to proposals.

Only validator-class nodes are election candidates and attestors. Included and validator-class nodes can be scored by consensus data. Idle nodes can graduate into Included through epoch progression.

### Validators

A validator is the operator identity behind one or more subnet nodes. The validator identity owns the nodes, controls their coldkey and hotkey settings, and may receive delegated stake from users.

The validator identity itself is not elected. A specific subnet node owned by that validator is elected.

### Subnet Owners

Subnet owners configure subnet-level policy inside network bounds. For consensus, the most important owner controls are the minimum node-count attestation percentage, validator node-count decay, per-node validator stake-weight power, reputation factors, classification timing, queue settings, and emergency validator sets.

### Delegators

Delegators can stake to validators. Validator delegate stake is the stake source used for stake-weighted attestation. Direct node stake is still important economic collateral and can be slashed, but the stake-weighted quorum is based on validator delegate stake allocated across the validator's subnet nodes.

## Epoch Flow

Each subnet has an assigned slot inside the chain epoch. When the chain reaches that subnet's slot, the pallet processes that subnet's consensus step.

The normal flow for a subnet epoch is:

1. The previous subnet epoch's proposal is prechecked.
2. Attestation ratios are calculated from the stored proposal and attestation snapshot.
3. Rewards or penalties are applied for the previous epoch.
4. A validator-class subnet node is elected for the new subnet epoch.
5. The subnet registration queue is processed.
6. The subnet burn-rate state is updated.

The elected node for the new subnet epoch is then responsible for submitting the consensus proposal for that epoch.

Consensus eligibility and reward eligibility are separate. An active, live subnet receives
an election at its slot even when it has no emission allocation. Emission weights for general
epoch `G` only include subnets with an elected validator for subnet epoch `G - 1`, the exact
epoch being settled. This lets a new or resumed subnet complete its first consensus round before
it can affect reward normalization.

When an owner unpauses a subnet in general epoch `G`, all of `G + 1` is reserved as a preparation
epoch. Queue processing and burn-rate maintenance continue during preparation, but no validator
is elected and no new consensus round begins. The subnet first becomes consensus-live and elects
a validator at its assigned slot in `G + 2`. That consensus round is first eligible for settlement
and emissions in `G + 3`.

## Becoming Electable

A subnet node becomes electable only when it reaches `Validator` classification.

During subnet bootstrap, initial validator nodes enter directly as validator-class nodes while the subnet is still in the registration phase - this is similar to how blockchains are required to be bootstrapped by multiple validators. After the subnet is active, new nodes normally enter the registration queue, activate as Idle, progress to Included, and eventually graduate to Validator if they satisfy the subnet's consensus and reputation requirements.

When a node graduates to Validator, it is inserted into `SubnetNodeElectionSlots`. That list is the election candidate set for the subnet. Removing a validator-class node removes it from the election set.

## Validator Election

At the subnet's slot, the pallet randomly selects one node from the current election set for the new subnet epoch.

Election probability is per eligible subnet node, not stake-weighted. If a validator operates multiple validator-class nodes in the same subnet, each eligible node is separately present in the election list unless removed or replaced by an emergency validator set.

If an emergency validator set is active, election uses the active emergency validator nodes instead of the normal election list. Emergency validator sets are owner-controlled recovery tools and are only activated through the pause and unpause flow.

## Proposing Consensus Data

The elected subnet node submits the epoch proposal with `propose_attestation`.

The proposal includes:

- score data for subnet nodes;
- optional queue priority or queue removal decisions;
- optional subnet-specific arguments;
- optional attestation data for the proposer's automatic self-attestation.

The proposer must sign with the hotkey associated with the elected subnet node. Only the elected node for the current subnet epoch can submit the proposal, and only one proposal can be stored for a subnet epoch.

The proposer automatically attests to its own proposal.

### Score Data

Score data is a list of `(subnet_node_id, score)` entries. Scores are produced off-chain by the subnet's own validation logic. The pallet does not decide whether a score is semantically correct for the subnet's application; it verifies whether enough eligible validator-class nodes attest to the submitted data.

Before storage, consensus data is canonicalized:

- nodes below `Included` classification are filtered out;
- duplicate entries for the same subnet node are collapsed to the lowest submitted score;
- score totals are checked for overflow;
- the canonical data is ordered by subnet node ID.

The score sum is later used to normalize node rewards.

## Attesting

After a proposal exists, validator-class subnet nodes can call `attest` for the same subnet epoch.

An attesting node must:

- sign with the hotkey associated with that subnet node;
- currently have `Validator` classification;
- be part of the proposal's snapshotted validator set;
- not have attested already for that proposal.

An attestation records the attesting node ID, block, epoch progress from proposal to attestation, a reward factor, and optional attestation data. Attestation timing can affect the attesting node's reward factor if that node later receives subnet-node rewards. The optional data is not interpreted by on-chain logic; it exists for subnet-specific or off-chain coordination.

The validator set used for attestation is snapshotted when the proposal is submitted. This prevents later node churn, pauses, or emergency changes from rewriting who counts as an eligible attestor for that epoch.

## Stake-Weighted Attestation

Consensus uses stake-weighted attestation to decide whether a proposal has enough economic support.

Percentages are represented as fixed-point values where `1e18` is 100%.

For each eligible attestor node, the pallet snapshots an attestor weight:

```text
allocated_weight = validator_delegate_stake * node_allocation
```

`validator_delegate_stake` is the total delegated stake assigned to the validator identity. `node_allocation` is the validator-defined percentage allocation for that specific `(subnet_id, subnet_node_id)`. A validator's node allocations must form a complete 100% allocation across all nodes it owns, including nodes in different subnets, so the same delegated stake is not counted in full for every node. The validator controls this allocation; the subnet owner controls the optional node-count decay and stake-weight power applied afterward.

If the validator has multiple nodes in the same subnet, the subnet's validator node-count decay can reduce each node's effective attestor weight:

```text
effective_weight = allocated_weight / node_count ^ (1 - node_count_decay)
```

`node_count` is the number of nodes the validator owns in that subnet, with the snapshotted eligible node count used as a floor. `node_count_decay` is a subnet owner setting in the same fixed-point percentage format. The default is `1e18` (1.0), which means no decay. A lower value applies stronger reduction to validators with multiple nodes in the subnet.

### Configuring Validator Node-Count Decay

The on-chain name for this optional diminishing factor is `ConsensusValidatorNodeCountDecay`. There is no separate feature flag. The subnet owner sets or disables the policy by submitting the signed pallet extrinsic:

```text
owner_update_consensus_validator_node_count_decay(subnet_id, value)
```

The extrinsic accepts an integer `value` between `0` and `1e18`, inclusive. Convert a decimal or percentage factor before submitting it:

```text
value = decimal_factor * 1e18
      = percentage_factor * 1e16
```

- `1000000000000000000` (100%) is the default and disables decay;
- `500000000000000000` (50%) divides allocated weight by `sqrt(node_count)`;
- `0` applies the strongest decay and divides allocated weight by `node_count`.

Any value below `1e18` enables the policy, and lower values diminish a multi-node validator's effective consensus weight more strongly. A validator with only one node in the subnet is not diminished, even when the value is `0`. The setting changes only how validator delegate stake contributes to the stake-weighted quorum; it does not reduce the validator's actual delegated stake balance or change the node-count quorum.

Owner updates are rate-limited by `ConsensusValidatorNodeCountDecayUpdateInterval`, which defaults to one global epoch. A successful update is scheduled for the next subnet epoch: if it is submitted in subnet epoch `S`, the live value remains unchanged for `S` and the pending value is used beginning with subnet epoch `S + 1`. This is a next-epoch boundary, not a guaranteed full epoch of elapsed notice: depending on where the call lands in `S`, activation is between roughly one block and one epoch away. The owner may replace a future schedule before it becomes effective, but cannot replace it during its activation epoch. On a later update, the pallet materializes the already-effective value before scheduling the replacement for the following subnet epoch.

The global-epoch rate limit and subnet-epoch activation delay are separate checks. Scheduling never rewrites an attestor-weight snapshot that was already stored for a proposal.

### Configuring Per-Node Validator Stake-Weight Power

`ConsensusValidatorStakeWeightPower` is a separate, optional subnet policy. It does not depend on how many nodes a validator owns. Instead, it applies the subnet's exponent independently to each eligible node's existing effective weight after allocation and node-count decay.

The pallet first converts the existing effective weights into shares, applies the power, and then uses the powered weights in the final attestation normalization:

```text
base_share_i = effective_weight_i / sum(effective_weight of all eligible nodes)
powered_weight_i = base_share_i ^ stake_weight_power

attestation_ratio = sum(powered_weight of attesting nodes)
                  / sum(powered_weight of all eligible nodes)
```

Both normalization steps are automatic. Applying the power to shares makes the result independent of the stake unit, while the final attested-to-total division ensures the powered shares sum to 100%. No stake is transferred: diminishing a dominant node increases the other positive-weight nodes' relative consensus shares through normalization.

The subnet owner configures the exponent with:

```text
owner_update_consensus_validator_stake_weight_power(subnet_id, value)
```

`value` uses the same `1e18` fixed-point format. The default is `1000000000000000000` (1.0), so `share ^ 1` preserves the current stake-weighted result exactly and the feature has no impact unless the owner changes it. Lower powers flatten the distribution among nodes with positive effective weight:

- with positive effective weights in a 90/10 split and power `500000000000000000` (0.5), the powered values are `sqrt(0.9)` and `sqrt(0.1)`; final normalization changes their shares to 75/25;
- with power `0`, every positive base share becomes the same powered weight, so the same two nodes normalize to 50/50. A zero effective weight remains zero.

Subnet owner values must be within the inclusive `MinConsensusValidatorStakeWeightPower` and `MaxConsensusValidatorStakeWeightPower` bounds. These collective-controlled bounds default to `0` and `1e18`. A supermajority collective updates them with `set_min_max_consensus_validator_stake_weight_power(min, max)`.

Owner updates are rate-limited by `ConsensusValidatorStakeWeightPowerUpdateInterval`, which defaults to one global epoch. A supermajority collective can change that interval with `set_consensus_validator_stake_weight_power_update_interval(value)`. A successful update is scheduled for the next subnet epoch: an update submitted in subnet epoch `S` leaves the live value unchanged in `S` and becomes the effective power in `S + 1`. As with node-count decay, this means the remainder of `S`, not a guaranteed full epoch of elapsed notice. A future schedule may be replaced before activation, is locked during its activation epoch, and is materialized before a later replacement is scheduled.

The global-epoch rate limit and subnet-epoch activation delay are independent. The scheduled power is selected by the subnet epoch when a proposal's attestor weights are snapshotted, and an existing snapshot is never rewritten.

### Final Attestation Normalization

After applying node-count decay and the optional stake-weight power, the pallet automatically normalizes weights across the eligible attestor snapshot:

```text
normalized_weight = powered_weight / sum(powered_weight of all eligible attestor nodes)
attestation_ratio = sum(normalized_weight of attesting nodes)
```

Normalization is always part of stake-weighted attestation and does not require another owner call. It converts the configured weights into relative quorum shares; it does not restore the amount removed by node-count decay or move stake between accounts. When the stake-weight power is the default `1e18`, the attestation ratio is equivalently:

```text
attestation_ratio = sum(effective_weight of attesting nodes)
                  / sum(effective_weight of all eligible attestor nodes)
```

If the total snapshotted attestor weight is zero, the stake-weighted attestation ratio is zero.

## Node-Count Attestation

The pallet also enforces a node-count quorum. This prevents one large stake position from being the only meaningful signal.

The subnet owner sets a minimum consensus-node attestation percentage within network bounds. The default subnet value is 20%, with network-level minimum and maximum bounds.

The required attestor count is:

```text
required_nodes = ceil(eligible_validator_count * min_node_attestation_percentage)
```

For validator sets larger than one, at least two nodes are required. For a one-node validator set, one node is required. The required count is capped at the eligible validator count.

A proposal must satisfy both quorum checks:

- stake-weighted attestation ratio must be at least `MinAttestationPercentage`;
- node attestation count must be at least the required node-count quorum.

## The 66% Threshold

`MinAttestationPercentage` is the network-level stake-weighted consensus threshold. Its default value is `0.66e18`, or 66%.

A proposal with attestation below this threshold is not in consensus, even if the proposer submitted validly. When the stake-weighted threshold fails, rewards are skipped and penalties are applied.

The node-count quorum can also fail independently. If either quorum fails, the epoch is treated as not in consensus.

## Supermajority Threshold

Some actions require a stronger signal than the normal 66% consensus threshold. `SuperMajorityAttestationRatio` defaults to `0.875e18`, or 87.5%.

The supermajority threshold is used for queue mutations and non-attestor reputation penalties. For example, prioritizing or removing a queued node only executes if the proposal reaches supermajority attestation.

## Rewards

When both quorum checks pass, the subnet is in consensus for the evaluated epoch.

The elected proposer can receive the base validator reward, scaled by its proposal timing factor. Earlier proposals receive a better factor than late proposals.

Subnet rewards are then calculated from the subnet's emission weight. The reward flow is:

- the subnet owner reward is paid;
- the subnet delegate-stake reward pool receives its configured share;
- the remaining subnet-node rewards are distributed by normalized consensus score;
- validator delegate stake pools and delegate accounts receive their configured shares from node rewards;
- final node rewards are added to node stake.

For each scored validator-class node:

```text
node_score_share = node_score / total_score
node_reward = subnet_node_rewards * node_score_share * reward_factor
```

If the canonical score sum is zero but consensus is reached, subnet rewards are held in the rewards capacitor for a future epoch. In that case the proposer reward has already been handled, but normal owner, delegate, and node reward distribution is skipped for that epoch.

## Reputation Updates

Consensus also drives subnet and node reputation.

When consensus succeeds:

- subnet reputation can increase;
- nodes included in consensus data can gain reputation;
- nodes absent from consensus data can lose reputation;
- included nodes can progress toward validator classification;
- nodes below the minimum reputation can be removed;
- nodes whose score share is below the subnet's minimum weight threshold can lose reputation;
- validator-class nodes that fail to attest while the proposal reaches supermajority can lose reputation.

When consensus fails:

- subnet reputation decreases;
- the elected proposer loses node reputation;
- the proposer's validator identity reputation decreases;
- attestors to the failed proposal can lose reputation;
- nodes that fall below the minimum reputation can be removed.

Reputation decreases generally scale with configured reputation factors. Several owner-controlled factors are resolved for the evaluated subnet epoch so parameter changes do not unexpectedly rewrite the current consensus period.

## Penalties and Slashing

The elected proposer is economically penalized when the proposal fails quorum.

The shortfall is calculated against the failed threshold:

```text
shortfall = 1 - actual_ratio / required_ratio
```

If both stake quorum and node-count quorum fail, the pallet uses the worse shortfall for the proposer penalty.

The stake slash is:

```text
base_slash = node_stake * BaseSlashPercentage
slash_amount = min(MaxSlashAmount, base_slash * shortfall)
```

By default, `BaseSlashPercentage` is 3.125%. `MaxSlashAmount` caps any single slash.

Slashing reduces the proposer's direct node stake. It does not directly slash validator delegate stake. The same failure also decreases node reputation and validator identity reputation. If the node's reputation falls below the subnet's minimum, the node can be removed from the active subnet and election set.

If no proposal is submitted for an epoch where a validator was elected, the pallet treats the proposer as absent. The subnet reputation decreases, and the elected node loses reputation. There is no successful consensus data to reward.

## Queue Decisions

The proposer can include two optional queue decisions:

- prioritize a queued node by moving it to the front of the queue;
- remove a queued node that has passed the queue immunity period.

These decisions are validated when submitted and only executed if the proposal reaches the supermajority threshold. Emergency validator-set consensus cannot mutate the normal registration queue.

## Emergency Validator Sets

An emergency validator set lets a subnet owner recover a paused subnet with a temporary validator set.

Emergency validator nodes must already be valid validator-class subnet nodes. When the emergency set activates, proposer election and attestor snapshots use the emergency node set instead of the normal election list. The emergency snapshot also carries the reputation factors and minimum reputation settings that should apply during the emergency period.

Emergency sets are bounded by size, duration, expiration rules, and cooldowns. They end automatically when their target duration is reached, their maximum epoch expires, or too few emergency validators remain valid. The owner can also revert the set while the subnet is paused.

## Consensus Boundaries

On-chain consensus verifies participation, stake-weighted agreement, node-count quorum, score normalization, reward distribution, and penalties. It does not run the subnet's off-chain evaluation logic itself.

Each subnet is responsible for defining how its nodes produce scores and how validators decide whether to attest. The chain enforces the economic result once enough eligible validator-class nodes attest under the configured thresholds.
