# Subnet Owners

A subnet owner is the account that creates and controls a subnet. The owner is responsible for the subnet's lifecycle, public metadata, and operational settings. When an allocated consensus round is accepted with a nonzero score sum, the current owner receives the protocol-defined subnet owner share of that round's subnet rewards.

Subnet ownership is an on-chain role. Every owner-only extrinsic requires the current owner account. Bootnode updates are a separate operation that also accepts accounts on the owner's bounded bootnode-access list; those accounts do not gain any other owner control.

## Becoming a Subnet Owner

A user becomes a subnet owner by registering a subnet. During registration, the registering account becomes the owner of the new subnet and defines the subnet's initial configuration, including its name, repository, description, bootnodes, stake limits, delegate reward split, and initial validator registration rules.

After registration, the subnet enters its registration and enactment flow. The owner is responsible for bringing the subnet to a state where it can activate, including ensuring that enough nodes and delegate stake are available. An activation attempt is not allowed until `MinSubnetRegistrationEpochs` has elapsed. After that point, an unmet activation condition during the registration period returns an error and leaves the subnet registered. During enactment, a delegate-stake shortfall alone returns `SubnetActivationConditionsNotMetYet` and preserves the subnet for funding and retry before the deadline. Insufficient reputation or node count still removes it, even if delegation is also insufficient; new nodes cannot register during enactment. An activation attempt after the enactment deadline still removes the subnet regardless of funding. A successful activation makes the subnet `Active` immediately and schedules its first validator-election attempt for its assigned slot in the following general epoch.

An account can also become a subnet owner when the current owner transfers ownership to it and the receiving account accepts the transfer.

## Owner Responsibilities

Subnet owners act as operators and stewards for their subnets. They should keep subnet information accurate, maintain usable bootnodes, choose sensible node and validator parameters, and react when the subnet needs maintenance.

Owner settings are still bounded by network rules. Many parameters have minimums, maximums, cooldowns, or delayed activation so a subnet owner cannot arbitrarily bypass protocol-level safety limits.

## Owner Features

### Lifecycle Management

Owners can activate and deactivate their subnets.

Activation moves a registered subnet into active preparation once the subnet meets the network's requirements; it becomes consensus-live at its assigned slot in the following general epoch.

Deactivation removes the subnet. This is the owner's final lifecycle control and should be treated as a permanent removal action.

### Metadata Management

Owners can update the public metadata associated with a subnet. This includes the subnet name, repository, description, and miscellaneous metadata.

These fields help users, node operators, and delegates understand what the subnet is, where its code or resources live, and how to evaluate its purpose. Name and repository values are treated as unique identifiers, so they cannot conflict with another registered subnet.

### Ownership Transfer

Owners can transfer a subnet to another account through a two-step ownership handoff. The current owner nominates a new owner, and the nominated account must accept before ownership changes.

This protects both parties from accidental transfers. Until the pending owner accepts, the current owner remains in control and can cancel the pending transfer.

### Node Registration and Queue Settings

Owners can configure how nodes enter and progress through the subnet. These controls include the registration queue duration, queue immunity period, target node registrations per epoch, maximum queued registrations (`MaxRegisteredNodes`), churn limits, and churn multipliers. `MaxRegisteredNodes` caps the active subnet's registration-queue length; the separate network `MaxSubnetNodes` value caps admitted Probation and Active nodes.

Together, these settings shape how quickly the subnet admits new nodes, how much protection queued nodes receive before evaluation, and how much turnover the subnet allows. They help owners balance growth, stability, and competition among node operators.

Registration queue duration and queue immunity changes are scheduled for the next subnet epoch. The live values continue to govern the current subnet epoch, giving queued nodes and validators a complete epoch boundary before the new timing applies.

A queued node remains in its waiting and immunity periods while the current subnet epoch is equal to `start_epoch + configured_epochs`. The period has passed only in a later subnet epoch. Consequently, when queue duration and immunity are equal, a removal cannot be authorized before the node reaches duration-based admission eligibility. Actual admission can still wait for queue capacity and the configured churn cadence.

### Initial Validators

At subnet registration, the owner supplies a map from validator ID to the maximum number of nodes that validator may register during the registration period. While the subnet remains in the `Registered` state, the owner can add, replace, or remove entries. Removing the final entry deletes the whitelist, and the node-registration path treats an absent whitelist as unrestricted registration. Node registration is nevertheless disabled once the subnet reaches the enactment period.

Successful activation deletes both the whitelist and its registration counters, and further initial-validator updates fail because the subnet is no longer `Registered`. Normal queue and classification rules then govern new nodes.

### Stake and Delegation Settings

Owners can set the minimum and maximum stake checked when subnet nodes register or add stake. A
withdrawal from an existing node must leave at least the subnet minimum, while the maximum does not
need to be rechecked when stake is reduced. Updating either limit changes future checks immediately
but does not rebalance or remove existing node stake.

Owners can also adjust the delegate stake reward percentage. This setting controls how subnet rewards are split between delegate stakers and other subnet reward recipients. A change is limited by the configured absolute-change bound and block-based update period, and is scheduled for the next subnet epoch rather than changing the current round's snapshotted policy.

### Bootnode Management

Owners manage the subnet's official bootnodes. Bootnodes are network entry points that help subnet nodes and overwatch nodes discover and connect to the subnet.

The owner can update bootnodes directly and can also grant or remove bootnode management access for other accounts. This lets an operator team maintain networking details without handing over full subnet ownership.

### Node Reputation and Classification Policy

Owners can tune several reputation and classification parameters that affect how subnet nodes progress and are evaluated. These include the required qualifying probation rounds, the minimum reputation required for subnet nodes, and reputation thresholds related to node weight decreases.

These controls allow subnet owners to define how strict or permissive their subnet is toward node performance. `owner_update_probation_rounds` schedules `ProbationRounds` (default 2) for the next subnet epoch; each elected round snapshots its own value. Minimum node reputation and the minimum-weight decrease threshold are also scheduled for the next subnet epoch. Probation counts qualifying inclusions rather than elapsed epochs, and promotion additionally requires full reputation.

### Reputation Factors

Owners can update subnet reputation factors. These factors determine how node reputation changes in response to behavior such as being absent, being included, falling below weight expectations, failing to attest, or participating outside consensus.

Reputation factors are a core part of subnet quality control. They define the incentives and penalties that shape long-term node behavior. Updates are bounded and scheduled after at least one subnet epoch through `SubnetOwnerFactorCooldownEpochs`.

The proposal-derived factors `included_increase`, `absent_decrease`,
`below_min_weight_decrease`, and `non_attestor_decrease` take effect only for an accepted proposal
with a nonzero score sum whose distinct-validator-identity support reaches the round's snapshotted
network supermajority threshold, 87.5% in the current runtime. Equality qualifies. This
identity-verification gate also controls
whether a Probation node's progress can advance or reset because of the score
vector. It prevents one large delegated-stake position from making subjective score data
reputation-authoritative.

Once the gate is met, these node factors retain their configured meanings and are applied at the
node level. In a normal round, `included_increase` applies to a Probation or Active node
present in the score vector; `absent_decrease` applies to a Probation or Active node
omitted from it; and `below_min_weight_decrease` applies when a scored Active-class node's score
share is below the owner's configured threshold. In ordinary accepted nonzero-total-score rounds
with identity supermajority, score-vector presence advances probation progress and omission resets
it; a zero individual score still counts. Other rounds freeze progress. The proposal snapshot contains admitted Probation and
Active nodes; only historical Active nodes can earn node-related rewards. Identity deduplication
does not merge sibling-node duties or outcomes. Reward eligibility, queue admission,
minimum-reputation removal, and objective lifecycle penalties are not controlled by this gate.

`validator_non_consensus_decrease` is the maximum percentage of current node reputation that the
elected proposer can lose when its submitted proposal is strongly rejected. The actual loss is
linear in the distinct-validator-identity support shortfall: it is zero at the
network-controlled strong-rejection threshold and reaches the owner-configured maximum at 0%
identity support. A failed proposal at or above that threshold does not apply this proposer-node
factor. The proposer economic paths remain separate and can still apply, while the proposer-node
and subnet reputation paths use the same identity threshold and shortfall.

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

`non_attestor_decrease` is a separate fixed node-reputation factor for a scored Active-class
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
node when an allocated round reaches settlement. That missing proposal also uses the separate
network `ValidatorAbsentSubnetReputationFactor` and does not run the submitted-proposal reputation
curves. Minimum-node and other subnet lifecycle reputation losses likewise remain
independent of validator-identity support because they are not claims derived from proposal
contents.

The subnet's proposal-derived reputation is identity-based as well. An accepted proposal can apply
`InConsensusSubnetReputationFactor` only after identity verification, with the distinct-identity
support ratio as its multiplier. A rejected submitted proposal applies
`NotInConsensusSubnetReputationFactor` only below the round's snapshotted, network-controlled
strong-rejection threshold, scaled from zero loss at the threshold to the full configured factor at
0% identity support. The threshold is one-third by default but can be changed by the admin
collective. These are network factors rather than owner-controlled node factors.

### Consensus and Attestation Settings

Owners can adjust two subnet-specific inputs to stake-weighted attestation: validator node-count
decay and the validator stake-weight power. Node-count decay can diminish each node's allocated
validator delegate-stake weight when the validator owns multiple nodes in the subnet. Stake-weight
power is then applied as an exponent to normalized node weights. Both default to `1e18`: no
node-count decay and a power-one (linear) exponent, respectively.

Both owner updates take effect in the next subnet epoch and are subject to network-controlled
bounds and global-epoch update intervals. The admin collective separately controls the base minimum
distinct-validator-identity attestation percentage. That minimum is converted to a required
identity count and is distinct from the 87.5% supermajority gate used to authorize subjective score
data for reputation. A round also needs at least three eligible validator identities to pass the
minimum identity quorum.

### Node Burn Rate Settings

Owners can update the node burn rate alpha. This setting influences how node registration burn rates respond to registration activity over time.

At a high level, this gives owners a way to tune the subnet's registration pressure so node entry costs can respond to demand without being fully static.

## Boundaries

Subnet owners have broad control over their own subnet, but they do not control the whole network. Network governance and runtime configuration still define global limits such as allowed parameter ranges, cooldowns, registration cost behavior, and reward percentages.

Owner privileges are meant to let a subnet operate independently while keeping its behavior compatible with the wider network.
