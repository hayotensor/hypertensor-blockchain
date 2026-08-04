# Validators

A validator is an on-chain identity that owns and operates subnet nodes. Validators register subnet nodes, maintain those nodes, manage their keys and staking settings, and receive validator-related rewards when their nodes perform correctly.

The validator identity itself does not participate directly in subnet consensus. Subnet nodes do. A validator is the parent identity and controller for those subnet nodes.

The validator is separate from any single subnet node. One validator can own multiple subnet nodes, including nodes across different subnets. Each subnet node remains tied back to the validator that registered it, so the validator identity acts as the operator and economic owner for those nodes.

Validators can also receive delegated stake from users. Delegators stake to the validator identity, while the validator controls how that delegated stake is associated with its owned subnet nodes for consensus weighting and reward distribution at the node level.

## Becoming a Validator

A user becomes a validator by registering a validator identity. Registration creates a validator ID and links it to a coldkey and hotkey.

The coldkey is the controlling account for the validator. It manages validator configuration, key updates, subnet node registration, node settings, and other owner-level actions for the validator's nodes.

The hotkey is the default operational key for subnet nodes owned by the validator when a node-specific hotkey is not set. This separation lets validators keep the controlling account separate from the key used by nodes for routine network operations.

During validator registration, the validator can also define initial delegation-related settings and optional identity metadata.

## Validator Responsibilities

Validators are responsible for the nodes they register. They should keep node networking information accurate, maintain enough node stake, and manage keys carefully so their subnet nodes can participate in consensus when those nodes are eligible or elected.

A validator's operation of its nodes can affect node reputation, validator reputation, rewards, slashing, and whether those nodes continue progressing through subnet classifications. Validators should treat their identity as a long-lived operating role rather than a single machine or temporary account.

## Validator Features

### Validator Identity

A validator identity is the root identity for a validator's subnet nodes. It is identified by a validator ID and associated with a coldkey, a hotkey, delegation settings, and optional identity information.

The identity lets the network group a validator's owned nodes and related settings under one operator. This matters for ownership checks, delegated stake, reward policy, validator reputation, and node consensus weight allocation.

### Subnet Node Ownership

Validators register and own subnet nodes. When a validator registers a subnet node, that node is linked to the validator ID and can then move through the subnet's registration, queue, active, included, and validator classifications according to subnet rules.

The validator coldkey controls the node's administrative settings. This includes node networking information, node-specific identifiers, node stake, node hotkeys, and removal of the node when removal is allowed.

Subnet owners may restrict early registration through initial validator rules while a subnet is being bootstrapped. Once a subnet is active, normal subnet registration, queue, classification, stake, and reputation rules apply.

### Subnet Node Consensus

Validator-owned subnet nodes can participate in subnet consensus once they reach the validator class. A subnet node may be elected to propose consensus data for an epoch, and eligible subnet nodes in the validator class can attest to that proposal.

Consensus submissions contain the elected subnet node's view of subnet node scores and may include queue decisions, such as prioritizing or removing queued nodes when the subnet rules allow it. Attestations from other eligible subnet nodes signal agreement with the submitted data.

Timely and accurate subnet node participation affects rewards and reputation. Proposal-derived
reputation uses distinct validator identities so a large delegated-stake position cannot make its
subjective score vector reputation-authoritative by itself. The identity ratio is:

```text
unique eligible validator identities with an attestation
/ all unique eligible validator identities
```

Multiple attesting nodes owned by one validator count as one identity, and the proposer's automatic
attestation contributes its identity once. An accepted proposal is **identity-verified** when this
ratio reaches the round's snapshotted supermajority threshold, 87.5% by default; equality qualifies.

Only an identity-verified accepted proposal can apply proposal-derived reputation-score and
classification changes. This includes node `included_increase`, `absent_decrease`,
`below_min_weight_decrease`, and `non_attestor_decrease`; the proposer validator identity's
reputation increase; the subnet-reputation increase; and Included-to-Validator consecutive
progression or reset based on score-vector presence. Identity deduplication controls the gate, but
responsibility remains node-level. An omitted sibling can receive `absent_decrease`, while a
low-scored or non-attesting Validator-class sibling can receive its corresponding consequence even
if another node owned by the same validator attested. The subnet-reputation increase is scaled by
identity support; the node and validator-identity factors are not increased for support above the
gate.

An accepted proposal below the identity-verification gate can still distribute rewards. Its score
vector is simply neutral for reputation scores and Included-to-Validator classification. Queue
decisions use their separate stake-weighted supermajority check. Idle-to-Included time progression,
minimum-reputation removal, and other objective lifecycle processing are unchanged. Accepted
zero-score proposals retain their early-return behavior, including skipping the per-node
distribution loop. In an emergency round, only the snapshotted emergency identities establish the
identity gate, non-attestor accountability is limited to emergency validators, and ordinary
classification progression remains disabled.

Queue decisions use a separate stake-weighted supermajority check. Reaching the identity
supermajority for reputation scores does not by itself authorize queue prioritization or removal.

The elected proposer's node receives `validator_non_consensus_decrease` only when distinct
validator-identity support is strictly below the round's snapshotted configurable
strong-rejection threshold, which defaults to one-third. The decrease scales linearly with the
identity-support shortfall: it is zero at the threshold and reaches the configured maximum at 0%
identity support. A failed proposal at or above the threshold does not apply this proposer-node
decrease. The proposer validator identity and subnet use the same one-third identity gate and
shortfall for their submitted-proposal reputation decreases. A stake-only rejection with at least
one-third identity support can still slash the proposer economically, but causes no
submitted-proposal reputation loss.

Under the same strong-rejection condition, every attesting node receives the separately configured
supporter decrease. Each identity counts once even if several of its nodes attest, but all of those
attesting nodes are processed. The elected proposer first receives its proposer-role decrease and
then, because submitting creates its automatic attestation, receives the supporter decrease
against its remaining reputation. Minimum-reputation removal is evaluated after both. The
supporter penalty affects node reputation only; attestors are not economically slashed for
attesting, while proposer direct-node-stake and delegate-pool slashing remain specific to the
elected proposer. A missing proposal has no attestors to penalize and follows the separate absence
and proposer-economic-penalty path. Specifically, it records a zero identity-support sample and
applies the objective proposer-node and subnet absence factors exactly once; it does not apply the
general validator-identity or submitted-proposal strong-rejection reputation decreases.

The validator identity records its first and most recent election epochs when one of its subnet
nodes is elected. These election timestamps are written immediately, including when the elected
node never submits a proposal. At settlement,
`average_proposal_identity_support` records the average distinct-identity support across elected
rounds, and `identity_support_samples` records the number of rounds represented. Submitted
proposals contribute their actual identity ratio; missing proposals contribute zero. If the
bounded counter ever reaches `u32::MAX`, both the count and average freeze together.
`total_increases` counts identity-verified accepted proposals with a nonzero configured increase
factor, while `total_decreases` counts proposals below the one-third identity threshold with a
nonzero effective decrease factor. Reputation-score-neutral submitted rounds still update the
support average without changing either score counter. The runtime's Overwatch
minimum-average-attestation eligibility check reads this identity-support average. Settlement does
not relabel the election as belonging to the later epoch.

### Key Management

Validators use a coldkey and hotkey model.

The coldkey controls sensitive and economic actions. It can update validator settings, rotate validator keys, register subnet nodes, manage node metadata, add or remove node stake, and remove owned nodes.

The validator hotkey is the default operational key used by owned subnet nodes when no node-specific hotkey is set. It lets nodes perform routine operational actions while keeping the coldkey separate from routine signing.

Validators can also assign a hotkey to an individual subnet node. A node-specific hotkey overrides the validator hotkey for that node's operational and consensus actions. This lets a validator isolate operational keys per node or per subnet without creating a separate validator identity.

Coldkeys, validator hotkeys, and node-specific hotkeys are expected to be distinct where required by the protocol. This avoids ambiguous ownership and reduces the blast radius of an operational key compromise.

### Validator Metadata

Validators can maintain identity metadata for their validator ID. This metadata helps users, subnet owners, and delegators recognize the validator and evaluate who is operating the nodes behind the identity.

Metadata is descriptive. It does not replace the on-chain key relationships that determine control over the validator and its subnet nodes.

### Delegate Stake

Users can delegate stake to validators. Validator delegate stake is tracked against the validator identity, not against one individual subnet node.

Delegators receive shares in the validator's delegated stake pool. Those shares represent their position in the pool and are used when adding, removing, transferring, or swapping validator delegate stake.

Validator delegate stake can influence the consensus weight of the validator's owned subnet nodes. The validator can control how that delegated stake weight is distributed across its nodes.

#### Delegate Stake Slashing Risk

The validator identity's entire delegate pool can be exposed when one of its elected subnet nodes
submits a proposal that receives stake-weighted attestation below the network's strong-rejection
threshold, or fails to submit a proposal. This exposure is identity-wide: it is not limited to the
delegate weight allocated to the elected node.

The penalty reduces the pool balance while leaving all delegator shares unchanged. Losses are
therefore shared proportionally through a lower share redemption value. Liability is calculated
from the pool balance and slashing configuration snapshotted when the node is elected, and is also
capped by the pool's current balance and the configured absolute maximum.

The delegate-pool penalty launches disabled: its base percentage and maximum slash amount both
default to zero. The strong-rejection threshold defaults to one-third. A supermajority collective
may later update the threshold, base percentage, and maximum amount atomically, including enabling
or disabling the economic tier. Delegators should inspect the current on-chain configuration and
remember that an already elected round keeps its snapshot even if governance changes the live
settings afterward.

For an enabled elected round, outgoing unstaking and swaps from the pool are temporarily locked
until its next settlement slot. Incoming stake and share transfers remain permitted, and
overlapping elections can extend the lock. This prevents pool value that was present at election
from exiting before the corresponding consensus result is settled.

### Delegate Reward Rate

Validators control a delegate reward rate. This rate determines what portion of eligible node rewards is directed into the validator's delegate stake pool for users who delegated to that validator.

The rate lets validators define how they share node rewards with delegators. It is bounded by network limits, and decreases are constrained so validators cannot sharply reduce delegator rewards without respecting protocol rules.

### Delegate Account

Validators may configure a delegate account. This is a separate account that can receive a validator-defined share of node rewards.

The delegate account is different from public validator delegate staking. Public delegation is a user stake pool attached to the validator, while the delegate account is an optional reward destination controlled by the validator's configuration.

The delegate account cannot be the validator coldkey or hotkey. Keeping it separate makes reward routing explicit and avoids mixing operational control with delegated reward collection.

### Delegate Stake Weight Allocation

When a validator owns multiple subnet nodes, it can control how validator delegate stake weight is allocated across those nodes. This allocation affects how delegated stake contributes to each owned node's consensus weight.

If a validator owns only one node, the allocation is naturally concentrated on that node. If it owns multiple nodes, the validator can distribute weight among them, subject to protocol rules that require the allocation to remain complete and valid.

These controls let a validator decide which of its nodes should carry more delegated-stake weight, while still keeping the validator identity as the single destination for delegators.

### Node Stake and Node Settings

Validators control the direct stake attached to their subnet nodes. Node stake is separate from validator delegate stake and is subject to each subnet's minimum and maximum stake rules.

Validators also maintain node settings such as peer information, bootnode peer information, client peer information, and node-specific unique or non-unique identifiers. These settings help the subnet identify, connect to, and evaluate the node.

### Staking Movement

Validator delegate stake can be added, removed, transferred, donated, or swapped between validators or between validator and subnet delegation pools, subject to network staking rules.

Removing delegated stake uses the network's unbonding process before balances become claimable. This gives the network predictable stake movement instead of immediate withdrawal from active staking pools.

## Boundaries

Validators control their own identity and the subnet nodes they own, but they do not control subnet ownership or global network policy. Subnet owners define subnet-level configuration, while network governance defines global limits such as stake bounds, cooldowns, reward limits, and rate-change constraints.

A validator can own and operate subnet nodes across many subnets, but each subnet's rules still determine whether each node can register, activate, remain in good standing, or participate in consensus.
