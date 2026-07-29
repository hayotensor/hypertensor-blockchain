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

Subnet owners configure subnet-level policy inside network bounds. For consensus, the most important owner controls are validator node-count decay, per-node validator stake-weight power, reputation factors, classification timing, queue settings, and emergency validator sets. The admin collective separately controls the network-wide validator-identity attestation percentage.

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

Validator identity election metadata is updated during step 4. Reputation score and attestation
statistics for that election are updated during step 3 of the following subnet epoch, once the
outcome is known. Consequently, settlement in epoch `E` evaluates epoch `E - 1`, while the stored
first and last validator-election epochs continue to identify the actual election epochs.

Consensus eligibility and reward eligibility are separate. An active, live subnet receives
an election at its slot even when it has no emission allocation. Emission weights for general
epoch `G` only include subnets with an elected validator for subnet epoch `G - 1`, the exact
epoch being settled. This lets a new or resumed subnet complete its first consensus round before
it can affect reward normalization. Once that exact election exists it remains eligible for its
following allocation and settlement if the owner pauses; pausing only prevents new elections and
operational maintenance.

Each subnet's epoch changes at its own assigned slot. If an owner unpauses while that subnet's
phase-aware epoch is `E`, `consensus_eligible_from_subnet_epoch` is set to `E + 2`. The remainder of `E`
is extra preparation time and all of `E + 1` is a complete local preparation epoch. Queue processing
and burn-rate maintenance continue during preparation, but no validator is elected and no new
consensus round begins. At the assigned slot starting `E + 2`, the subnet becomes consensus-live
and elects its first post-unpause validator. That election can first receive an emission allocation
at the following general epoch and is settled at the next assigned subnet slot.

General-epoch preliminary processing runs at slot zero, before subnet slots. Ordinary reputation,
minimum-node, and stake checks skip an active subnet whose phase-aware local epoch has not reached
`consensus_eligible_from_subnet_epoch`. The global boundary immediately before the first `E + 2` subnet slot therefore
cannot penalize the preparing subnet; those checks resume at the following global boundary, after
the subnet has reached its first live slot.

Pausing uses both clocks for different purposes. The phase-aware subnet epoch records skipped local
slots for queue compensation, and the global pause-start epoch governs maximum-pause reputation
decay and removal. Paused subnets remain eligible for lowest-stake capacity removal when the network
exceeds `MaxSubnets`. Re-pause cooldowns are local, count only from the first consensus-eligible epoch, and
must be at least one subnet epoch. Because subnet-slot `on_initialize` runs before extrinsics, the
round satisfying the cooldown is settled before the owner can pause again.

### Overwatch Epochs

Overwatch scoring runs on a slower, global schedule so Overwatch nodes do not have to perform and
submit their off-chain work during every general blockchain epoch. An Overwatch epoch has its own
monotonic ID and anchored start block. Its length is the active
`OverwatchEpochLengthMultiplier` multiplied by the general epoch length.

Collective updates to the multiplier and commit cutoff take effect immediately. A mid-epoch update
can therefore extend or shorten the active round and can move it between commit and reveal phases.
The current Overwatch epoch ID and anchored start block are not reset by a configuration update.

When an Overwatch epoch closes, the pallet queues the closed ID and its multiplier as an explicit
settlement snapshot. Rollover is aligned with general epoch slot zero and settlement runs in the
reserved slot one, normally the following block. If global transaction pause skips a boundary, the
active Overwatch clock is frozen by shifting its anchored start block by the pause duration. Nodes
therefore retain the same commit or reveal time that remained when pause began. Any shifted end is
then rounded forward to the next slot-zero boundary, keeping Overwatch work away from subnet slots.
A delayed settlement remains queued until the next slot one. Settlement is idempotent, and empty
epochs are still marked finalized, which distinguishes a processed epoch with no subnet score from
an epoch that has not been processed. Consumers use `LastFinalizedOverwatchEpoch` rather than
deriving `CurrentOverwatchEpoch - 1`.

`OverwatchEpochEmissions` is the budget for one general blockchain epoch. A completed Overwatch
epoch spanning `M` general epochs therefore pays `M * OverwatchEpochEmissions`. Its finalized
subnet weights remain the latest available Overwatch signal and are reused by each general emission
calculation until a later Overwatch epoch is finalized. A subnet missing a score in an otherwise
finalized Overwatch epoch uses the configured default Overwatch subnet weight.

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
- optional subnet-specific arguments.

The `propose_attestation` call must originate from the hotkey associated with the elected subnet
node. Only the elected node for the current subnet epoch can submit the proposal, and only one
proposal can be stored for a subnet epoch.

Submitting a proposal automatically records the elected validator's attestation. The successful
extrinsic call therefore records both authorship and endorsement of the submitted consensus data;
there is no separate proposal-signature or attestation-signature payload.

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

- submit the `attest` extrinsic from the hotkey associated with that subnet node;
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

Any value below `1e18` enables the policy, and lower values diminish a multi-node validator's effective consensus weight more strongly. A validator with only one node in the subnet is not diminished, even when the value is `0`. The setting changes only how validator delegate stake contributes to the stake-weighted quorum; it does not reduce the validator's actual delegated stake balance or change the validator-identity participation floor.

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

## Validator-Identity Participation

The pallet also enforces participation by distinct validator identities. Multiple subnet nodes
owned by the same validator identity count once. This prevents one large stake position from being
the only meaningful signal.

At least three eligible validator identities are required for normal settlement. The admin
collective controls the network-wide identity-attestation percentage stored in
`ConsensusValidatorIdentityAttestationPercentage`. Its default is 10%, and each election snapshots
the active value so an update affects newly elected rounds without changing rounds in progress.

For three eligible identities, two must attest. For larger sets, the required count is:

```text
required_identities = max(3, ceil(eligible_identity_count * identity_attestation_percentage))
```

The required count is capped at the eligible identity count. Thus the default rule requires three
of four identities, three of seventeen, four of thirty-one, and ten of one hundred. A set with
fewer than three eligible identities cannot enter normal settlement.

A proposal must satisfy both quorum checks:

- stake-weighted attestation ratio must be at least `MinAttestationPercentage`;
- the eligible set must contain at least three distinct validator identities;
- identity attestation count must be at least the required participation floor.

## The Two-Thirds Threshold

`MinAttestationPercentage` is the network-level stake-weighted consensus threshold. Its default
value is `0.666666666666666666e18`, the runtime's fixed-point representation of two-thirds.

A proposal with attestation below this threshold is not in consensus, even if the proposer submitted validly. When the stake-weighted threshold fails, rewards are skipped and penalties are applied.

The validator-identity participation requirement can also fail independently. If either
participation threshold fails, the epoch is treated as not in consensus.

## Supermajority Threshold

Some actions require a stronger signal than the normal two-thirds consensus threshold. `SuperMajorityAttestationRatio` defaults to `0.875e18`, or 87.5%.

The same snapshotted threshold supplies two separate supermajority gates:

- queue prioritization or removal requires the **stake-weighted** attestation ratio to be at least
  the threshold;
- `non_attestor_decrease` requires the **distinct-validator-identity** attestation ratio to be at
  least the threshold.

Equality qualifies for both gates. The identity ratio is:

```text
identity_attestation_ratio =
  unique eligible validator identities that attested
  / unique eligible validator identities
```

The eligible nodes and their parent validator identities are fixed by the proposal-time snapshot,
while the threshold comes from the elected round's policy snapshot. Multiple attesting nodes owned
by one validator identity contribute one identity to the numerator. The proposer's automatic
attestation contributes its identity once.

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
- scored validator-class nodes that fail to attest can lose reputation when distinct
  validator-identity participation reaches the snapshotted supermajority threshold.

The identity supermajority establishes that participation was broad enough to hold individual
non-attesting nodes accountable. Once it is met, each applicable non-attesting node receives the
full configured `non_attestor_decrease`; the factor is not scaled by stake, identity participation,
or distance above the threshold. Identity deduplication affects only the gate. Attestation duty
remains node-level, so a non-attesting node can be penalized even if another node belonging to the
same validator identity attested. The decrease changes node reputation only and does not slash
direct node stake or a validator delegate pool.

This rule runs only while distributing a successful, nonzero-score proposal. Rejected and missing
proposals do not apply `non_attestor_decrease`, and the existing zero-score accepted-proposal early
return remains unchanged. In an emergency-validator round, the identity ratio and applicable
non-attestors are limited to the snapshotted emergency validator set.

When consensus fails:

- subnet reputation decreases;
- the proposer's validator identity reputation decreases;
- the elected proposer loses node reputation only when distinct validator-identity support is
  strictly below the round's snapshotted strong-rejection threshold;
- every attesting node, including the proposer's automatic attestation, loses node reputation only
  when distinct validator-identity support is strictly below the round's snapshotted
  strong-rejection threshold;
- nodes that fall below the minimum reputation can be removed.

Reputation decreases generally scale with configured reputation factors. Several owner-controlled factors are resolved for the evaluated subnet epoch so parameter changes do not unexpectedly rewrite the current consensus period.

## Penalties and Slashing

The elected proposer is economically penalized when the proposal fails either quorum. The
ordinary penalty slashes the elected subnet node's direct stake using the worse normalized
shortfall among the failed stake and validator-identity thresholds.

The shortfall is calculated against the failed threshold:

```text
shortfall = 1 - actual_ratio / required_ratio
```

The selected failure ratio and threshold continue to drive the direct-stake,
validator-identity-reputation, and subnet-reputation penalty paths. They do not drive the
proposer's node-reputation decrease.

The stake slash is:

```text
base_slash = node_stake * BaseSlashPercentage
slash_amount = min(MaxSlashAmount, base_slash * shortfall)
```

By default, `BaseSlashPercentage` is 3.125%. `MaxSlashAmount` caps any single direct-node slash.

Strong rejection has a separate, governance-controlled validator delegate-pool penalty. It applies
only when the stake-weighted attestation rate is strictly below the round's snapshotted
`ValidatorDelegateStakeSlashThreshold`, which defaults to the fixed-point representation of
one-third. This economic penalty applies only to the elected proposer's validator identity; an
attesting node is not slashed merely for attesting to the proposal.

The same snapshotted configurable threshold also gates two node-reputation decreases based solely
on distinct validator-identity support rather than stake-weighted support. The proposal-time
validator snapshot fixes the eligible identity denominator. Each validator identity with one or
more attesting nodes contributes exactly one supporter, and the proposer contributes one through
its automatic attestation.

The proposer-role decrease uses the subnet's snapshotted `validator_non_consensus_decrease` as its
maximum loss factor. The supporter decrease uses the separately snapshotted
`non_consensus_attestor_decrease`:

```text
identity_support = distinct attesting validator identities
  / distinct eligible validator identities
identity_shortfall = 1 - identity_support / strong_rejection_threshold
proposer_reputation_loss = current_reputation
  * validator_non_consensus_decrease
  * identity_shortfall
supporter_reputation_loss = current_reputation
  * non_consensus_attestor_decrease
  * identity_shortfall
```

Both curves are zero at the strict threshold and reach their respective configured maximum
percentages at 0% identity support. At or above the threshold, a failed submitted proposal does not
apply either node-reputation decrease. Below it, the elected proposer first receives the
proposer-role decrease. Every attesting node then receives the supporter decrease; this includes
the proposer because proposal submission creates its automatic attestation. If several nodes
belonging to one validator identity attest, that identity still counts once when calculating
support, but every attesting node is processed. The attestor rule does not slash those nodes'
direct stake or validator delegate pools; proposer economic slashing remains role-specific. For
the proposer, the supporter decrease is applied to the reputation remaining after the
proposer-role decrease, and the minimum-reputation removal check runs after both.

The delegate-pool shortfall and slash are:

```text
delegate_shortfall = 1 - attestation_rate / delegate_slash_threshold

pool_slash = min(
  snapshotted_pool_balance,
  current_pool_balance,
  max_pool_slash_amount,
  snapshotted_pool_balance * base_pool_slash_percentage * delegate_shortfall
)
```

The curve is linear: the pool slash is zero at the threshold and reaches the configured base
percentage at 0% attestation. The election snapshots the threshold, base percentage, maximum
amount, and validator-pool balance. Governance changes therefore apply to later elections, while
rewards or incoming stake after election cannot increase an in-progress round's liability. The
live balance cap prevents a settlement from taking more than remains in the pool.

Pool slashing reduces `ValidatorDelegateStakeBalance` and the network-wide validator delegate
stake total without burning shares. Every share consequently loses the same proportional
redemption value. The whole validator identity pool is exposed, not merely the delegate weight
allocated to the elected subnet node.

Delegate-pool slashing is enabled only when both
`BaseValidatorDelegateStakeSlashPercentage` and `MaxValidatorDelegateStakeSlashAmount` are
nonzero. Both launch as zero, which disables only the delegate-pool balance loss and protects
delegator principal. The threshold continues to govern the distinct-identity-support
node-reputation curves. A supermajority collective can later enable, reconfigure, or atomically
disable the economic tier. The configured threshold must remain above zero and below
`MinAttestationPercentage`; the base percentage cannot exceed 100%.

When an enabled round is elected, outgoing removals and swaps from that validator pool are locked
until the round's settlement slot, `election_block + EpochLength`. Incoming delegation and
transfers of pool shares remain available because neither removes value from the slashable pool.
Overlapping elected rounds extend the lock to the latest settlement block.

Every failed-consensus result can decrease the proposer's validator identity reputation and the
subnet's reputation under their existing rules. The elected proposer's node reputation decreases
under `validator_non_consensus_decrease` only for a strongly rejected submitted proposal, using
the identity-support curve above. Every node recorded as attesting, including the proposer,
receives the separate `non_consensus_attestor_decrease` only under the same strong-rejection
condition. These attestors receive no attestor-specific economic slash. If a node's reputation
falls below the subnet's minimum, the node can be removed from the active subnet and election set.

If no proposal is submitted for an epoch where a validator was elected, the pallet treats the
attestation rate as 0% for both economic formulas. Existing absence reputation penalties are
still applied exactly once. There is no successful consensus data to reward.

## Queue Decisions

The proposer can include two optional queue decisions:

- prioritize a queued node by moving it to the front of the queue;
- remove a queued node that has passed the queue immunity period.

These decisions are validated when submitted and only executed if the proposal's stake-weighted
attestation ratio is at least the snapshotted supermajority threshold. This queue gate remains
stake-weighted and is independent of the distinct-identity gate for `non_attestor_decrease`.
Emergency validator-set consensus cannot mutate the normal registration queue.

Queue duration and immunity changes become effective at the next subnet epoch. Both periods use the same strict elapsed-period boundary:

```text
period_has_elapsed = start_epoch + configured_epochs < evaluated_subnet_epoch
```

Equality is still the final waiting or immune epoch. With equal queue-duration and immunity values, the node becomes activation-eligible by duration no later than removal becomes valid. Actual activation can still wait for queue capacity and churn cadence. Saturating epoch arithmetic keeps an overflowed deadline from becoming prematurely eligible.

## Emergency Validator Sets

An emergency validator set lets a subnet owner recover a paused subnet with a temporary validator set.

Emergency validator nodes must already be valid validator-class subnet nodes. When the emergency set activates, proposer election and attestor snapshots use the emergency node set instead of the normal election list. The emergency snapshot also carries the reputation factors and minimum reputation settings that should apply during the emergency period.

Emergency sets are bounded by size, duration, expiration rules, and cooldowns. They end automatically when their target duration is reached, their maximum epoch expires, or too few emergency validators remain valid. The owner can also revert the set while the subnet is paused.

## Consensus Boundaries

On-chain consensus verifies participation, stake-weighted agreement, validator-identity breadth,
score normalization, reward distribution, and penalties. It does not run the subnet's off-chain
evaluation logic itself.

Each subnet is responsible for defining how its nodes produce scores and how validators decide whether to attest. The chain enforces the economic result once enough eligible validator-class nodes attest under the configured thresholds.
