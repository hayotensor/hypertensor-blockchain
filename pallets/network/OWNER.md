# Subnet Owners

A subnet owner is the account that creates and controls a subnet. The owner is responsible for the subnet's lifecycle, public metadata, operational settings, and recovery actions. The owner also receives the protocol-defined subnet owner share of subnet rewards while the subnet exists.

Subnet ownership is an on-chain role. Only the current owner account can use owner-only subnet controls, unless the owner has delegated a narrower permission such as bootnode management access.

## Becoming a Subnet Owner

A user becomes a subnet owner by registering a subnet. During registration, the registering account becomes the owner of the new subnet and defines the subnet's initial configuration, including its name, repository, description, bootnodes, stake limits, delegate reward split, and initial validator registration rules.

After registration, the subnet enters its registration and enactment flow. The owner is responsible for bringing the subnet to a state where it can activate, including ensuring that enough nodes and delegate stake are available. Once the activation requirements are met, the owner can activate the subnet so it can participate in normal network operation.

An account can also become a subnet owner when the current owner transfers ownership to it and the receiving account accepts the transfer.

## Owner Responsibilities

Subnet owners act as operators and stewards for their subnets. They should keep subnet information accurate, maintain usable bootnodes, choose sensible node and validator parameters, and react when the subnet needs maintenance or emergency recovery.

Owner settings are still bounded by network rules. Many parameters have minimums, maximums, cooldowns, or delayed activation so a subnet owner cannot arbitrarily bypass protocol-level safety limits.

## Owner Features

### Lifecycle Management

Owners can activate, pause, unpause, and deactivate their subnets.

Activation moves a registered subnet into normal operation once the subnet meets the network's requirements. Pausing stops new validator elections and operational maintenance while the subnet is paused, so the owner can handle maintenance or recovery. Unpausing starts a controlled preparation period before the subnet returns to consensus and adjusts queued-node timing so skipped subnet slots do not unfairly affect registration queues.

Deactivation removes the subnet. This is the owner's final lifecycle control and should be treated as a permanent removal action.

### Pausing and Unpausing

Only the current subnet owner can pause or unpause a subnet. A subnet must be active before it can be paused and paused before it can be unpaused. After activation or an unpause, the owner must also wait for the configured subnet pause cooldown before pausing it again. The cooldown is measured in that subnet's own epochs, not general blockchain epochs, and must be at least one. Epochs count from the subnet's first consensus-eligible epoch, so preparation time cannot satisfy the cooldown. At the assigned slot where the cooldown expires, `on_initialize` settles the preceding round before an owner pause extrinsic can execute; consequently, a cooldown of one requires one completed live subnet round.

While a subnet is paused:

- No new validator is elected and no new consensus round begins.
- Registration-queue processing and burn-rate maintenance do not run.
- New node registration is unavailable.
- The owner can configure, revert, or replace a pending emergency validator set, subject to the emergency-set rules.
- An already-elected historical consensus round is preserved. Its exact election remains eligible for the following general-epoch emission allocation even if the owner pauses first, and an allocation already recorded still settles while paused.

A subnet cannot remain paused indefinitely without consequences. The pause records both the global epoch in which it began and the subnet's phase-aware epoch at that block. Once the global maximum pause duration is exceeded, general-epoch preliminary processing begins reducing its subnet reputation. The subnet can be removed if its reputation falls below the network minimum; it is not automatically unpaused. A paused subnet also remains eligible for lowest-stake capacity removal when the network exceeds `MaxSubnets`, including during the maximum-pause grace period.

#### Unpause Timeline

Each subnet's epoch advances at its own assigned slot. If an owner unpauses while that subnet's current phase-aware epoch is `E`, the subnet becomes `Active` immediately for preparation purposes, but `consensus_eligible_from_subnet_epoch` is set to `E + 2`.

| Subnet epoch | Subnet behavior |
| --- | --- |
| `E` | The owner unpauses. Any time remaining before the subnet's next assigned slot is additional preparation time. |
| `E + 1` | Full local preparation epoch. Queue and burn-rate maintenance run, but there is no validator election or new consensus round. |
| `E + 2` | At the subnet's assigned slot it becomes consensus-live, elects a validator, and begins its first post-unpause round. It has no exact prior election, so this first round has not yet received an emission allocation. |
| Following general epoch | The exact `E + 2` election can receive an emission allocation. Its round is settled at the next assigned subnet slot, where normal consensus rewards or penalties apply. |

Emission allocation requires an elected validator for the exact previous subnet epoch being
settled. Creating that election requires the subnet to be active and consensus-live at its assigned
slot. Once created, the historical election remains allocation-eligible even if the subnet is later
paused or is preparing after an unpause.

This prevents a newly unpaused subnet from receiving an unused allocation, diluting other subnets' rewards, or being penalized before it has had a complete local epoch in which to prepare.

Global subnet-health checks run at general epoch slot zero, before every subnet's assigned slot. An active subnet that has not yet reached `consensus_eligible_from_subnet_epoch` is treated as preparing, so ordinary reputation, minimum-node, and stake checks are skipped. The global boundary immediately before its first post-unpause consensus slot therefore cannot penalize it. These checks resume at the next global boundary, after the subnet has reached its first live slot. Maximum-pause reputation processing remains global because it applies only while the subnet is paused.

Queued-node classification times are shifted by exactly the subnet slots missed while paused. Whether the pause occurs before or after the subnet's assigned slot is taken into account. The full `E + 1` preparation epoch is not treated as missed time, so it counts normally toward queue maturity.

If a pending emergency validator set exists when the subnet is unpaused, it is validated and activated as part of the unpause. Its duration starts at the first consensus-eligible epoch, `E + 2`, so the preparation epoch does not consume emergency-validator time. An invalid pending set causes the unpause to fail and leaves the subnet paused.

### Metadata Management

Owners can update the public metadata associated with a subnet. This includes the subnet name, repository, description, and miscellaneous metadata.

These fields help users, node operators, and delegates understand what the subnet is, where its code or resources live, and how to evaluate its purpose. Name and repository values are treated as unique identifiers, so they cannot conflict with another registered subnet.

### Ownership Transfer

Owners can transfer a subnet to another account through a two-step ownership handoff. The current owner nominates a new owner, and the nominated account must accept before ownership changes.

This protects both parties from accidental transfers. Until the pending owner accepts, the current owner remains in control and can cancel the pending transfer.

### Node Registration and Queue Settings

Owners can configure how nodes enter and progress through the subnet. These controls include the registration queue duration, queue immunity period, target node registrations per epoch, maximum registered nodes, churn limits, and churn multipliers.

Together, these settings shape how quickly the subnet admits new nodes, how much protection queued nodes receive before evaluation, and how much turnover the subnet allows. They help owners balance growth, stability, and competition among node operators.

Registration queue duration and queue immunity changes are scheduled for the next subnet epoch. The live values continue to govern the current subnet epoch, giving queued nodes and validators a complete epoch boundary before the new timing applies.

A queued node remains in its waiting and immunity periods while the current subnet epoch is equal to `start_epoch + configured_epochs`. The period has passed only in a later subnet epoch. Consequently, when queue duration and immunity are equal, a removal cannot be authorized before the node reaches duration-based activation eligibility. Actual activation can still wait for queue capacity and the configured churn cadence.

### Initial Validators

During the registration phase, owners can manage the initial validators or allowed node operators that are permitted to register early nodes. These initial validator settings help a new subnet bootstrap with a known set of participants before it becomes active.

Once the subnet moves beyond registration, initial validator controls no longer serve as the primary way to manage participation. Normal subnet rules and node classifications take over.

### Stake and Delegation Settings

Owners can set the minimum and maximum stake allowed for subnet nodes. These limits affect how much stake each node must provide and how much stake concentration the subnet permits.

Owners can also adjust the delegate stake reward percentage. This setting controls how subnet rewards are split between delegate stakers and other subnet reward recipients. Changes to delegation rewards are constrained by network limits and update timing rules so reward policy cannot shift too abruptly.

### Bootnode Management

Owners manage the subnet's official bootnodes. Bootnodes are network entry points that help subnet nodes and overwatch nodes discover and connect to the subnet.

The owner can update bootnodes directly and can also grant or remove bootnode management access for other accounts. This lets an operator team maintain networking details without handing over full subnet ownership.

### Emergency Validator Sets

Owners can set an emergency validator set while the subnet is paused. This feature is intended for recovery situations where the normal validator set is unhealthy, unavailable, or needs temporary replacement.

Emergency validators are selected from valid subnet validator nodes and operate under protocol limits, including size limits, duration limits, and cooldowns. When the emergency period expires, or when the owner reverts the emergency set while the subnet is paused, the subnet returns to normal validator behavior.

### Node Reputation and Classification Policy

Owners can tune several reputation and classification parameters that affect how subnet nodes progress and are evaluated. These include idle and included classification timing, the minimum reputation required for subnet nodes, and reputation thresholds related to node weight decreases.

These controls allow subnet owners to define how strict or permissive their subnet is toward node performance. Some changes are scheduled for a future subnet epoch so they do not disrupt the current consensus period.

### Reputation Factors

Owners can update subnet reputation factors. These factors determine how node reputation changes in response to behavior such as being absent, being included, falling below weight expectations, failing to attest, or participating outside consensus.

Reputation factors are a core part of subnet quality control. They define the incentives and penalties that shape long-term node behavior, and updates are bounded and scheduled so changes remain predictable.

The proposal-derived factors `included_increase`, `absent_decrease`,
`below_min_weight_decrease`, and `non_attestor_decrease` take effect only for an accepted proposal
whose distinct-validator-identity support reaches the round's snapshotted network supermajority
threshold, 87.5% by default. Equality qualifies. This identity-verification gate also controls
whether an Included node's consecutive-inclusion state can advance or reset because of the score
vector. It prevents one large delegated-stake position from making subjective score data
reputation-authoritative.

Once the gate is met, these node factors retain their configured meanings and are applied at the
node level. `included_increase` applies to a node present in the score vector;
`absent_decrease` applies to a node omitted from it; and `below_min_weight_decrease` applies when a
scored Validator-class node's score share is below the owner's configured threshold. Identity
deduplication does not merge sibling-node duties or outcomes. Rewards, Idle-to-Included time
progression, minimum-reputation removal, and objective lifecycle penalties are not controlled by
this gate.

`validator_non_consensus_decrease` is the maximum percentage of current node reputation that the
elected proposer can lose when its submitted proposal is strongly rejected. The actual loss is
linear in the distinct-validator-identity support shortfall: it is zero at the
network-controlled strong-rejection threshold and reaches the owner-configured maximum at 0%
identity support. A failed proposal at or above that threshold does not apply this proposer-node
factor. The proposer economic paths remain separate and can still apply, but the proposer
validator-identity and subnet reputation paths use the same identity threshold and shortfall.

`non_consensus_attestor_decrease` is the maximum percentage of current reputation that a
supporting attestor can lose when a proposal is strongly rejected. The actual loss is linear: it
is zero at the network-controlled strong-rejection threshold and reaches the owner-configured
maximum at 0% distinct validator-identity support. Each validator identity counts once regardless
of how many of its nodes attest, while every attesting node receives the resulting reputation
decrease. This includes the elected proposer through its automatic attestation. The factor affects
node reputation only: it does not slash attestors' node stake or validator delegate pools, and
economic slashing remains specific to the elected proposer. For that proposer, the supporter
decrease uses the same identity shortfall and compounds after
`validator_non_consensus_decrease` has been applied to the proposer node. Minimum-reputation
removal is evaluated after both decreases.

`non_attestor_decrease` is a separate fixed node-reputation factor for a scored Validator-class
node that does not attest to an accepted, nonzero-score proposal. It is applied in full only when
the distinct-validator-identity attestation ratio is at least the round's snapshotted network
supermajority threshold; equality qualifies. The ratio counts each eligible parent validator
identity once and each identity with at least one attesting node once. The proposer contributes
through its automatic attestation. Identity deduplication protects the gate from stake
concentration, but responsibility remains node-level: if one node attests and a sibling owned by
the same validator does not, the sibling can still receive the configured decrease. This factor
does not slash node stake or validator delegate pools. Rejected, missing, and zero-score proposals
do not apply it. Queue mutations remain separately gated by stake-weighted supermajority.

`validator_absent_decrease` remains an objective missing-proposal penalty for the elected proposer
node. A missing proposal also uses the separate network
`ValidatorAbsentSubnetReputationFactor`, records zero proposal identity support for the elected
validator identity, and does not run the submitted-proposal reputation curves. Pause, minimum-node,
and other subnet lifecycle reputation losses likewise remain independent of validator-identity
support because they are not claims derived from proposal contents.

The subnet's proposal-derived reputation is identity-based as well. An accepted proposal can apply
`InConsensusSubnetReputationFactor` only after identity verification, with the distinct-identity
support ratio as its multiplier. A rejected submitted proposal applies
`NotInConsensusSubnetReputationFactor` only below the network's one-third strong-rejection
threshold, scaled from zero loss at the threshold to the full configured factor at 0% identity
support. These are network factors rather than owner-controlled node factors.

Every settled elected round also updates the proposer's validator-identity support history.
Submitted proposals record their actual distinct-identity ratio and missing proposals record zero
in `average_proposal_identity_support`; `identity_support_samples` tracks the denominator. These
network-maintained statistics are independent of whether an owner-configured node factor changed a
node's reputation. The bounded count and average freeze together at `u32::MAX`.

### Consensus and Attestation Settings

Owners can adjust subnet-specific consensus policy such as the validator node count decay used in
stake-weight calculations. The admin collective, rather than subnet owners, controls the
network-wide distinct-validator-identity attestation percentage.

These settings influence how validator/node-count history affects consensus behavior over time.
They are constrained by network-level limits and, where applicable, update intervals.

### Node Burn Rate Settings

Owners can update the node burn rate alpha. This setting influences how node registration burn rates respond to registration activity over time.

At a high level, this gives owners a way to tune the subnet's registration pressure so node entry costs can respond to demand without being fully static.

## Boundaries

Subnet owners have broad control over their own subnet, but they do not control the whole network. Network governance and runtime configuration still define global limits such as allowed parameter ranges, pause limits, cooldowns, registration cost behavior, and reward percentages.

Owner privileges are meant to let a subnet operate independently while keeping its behavior compatible with the wider network.
