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

Activation moves a registered subnet into normal operation once the subnet meets the network's requirements. Pausing temporarily stops key subnet progression, such as new validator election and queue activation at the next subnet epoch, so the owner can handle maintenance or recovery. Unpausing returns the subnet to active operation and adjusts queued node timing so paused time does not unfairly affect registration queues.

Deactivation removes the subnet. This is the owner's final lifecycle control and should be treated as a permanent removal action.

### Metadata Management

Owners can update the public metadata associated with a subnet. This includes the subnet name, repository, description, and miscellaneous metadata.

These fields help users, node operators, and delegates understand what the subnet is, where its code or resources live, and how to evaluate its purpose. Name and repository values are treated as unique identifiers, so they cannot conflict with another registered subnet.

### Ownership Transfer

Owners can transfer a subnet to another account through a two-step ownership handoff. The current owner nominates a new owner, and the nominated account must accept before ownership changes.

This protects both parties from accidental transfers. Until the pending owner accepts, the current owner remains in control and can cancel the pending transfer.

### Node Registration and Queue Settings

Owners can configure how nodes enter and progress through the subnet. These controls include the registration queue duration, queue immunity period, target node registrations per epoch, maximum registered nodes, churn limits, and churn multipliers.

Together, these settings shape how quickly the subnet admits new nodes, how much protection queued nodes receive before evaluation, and how much turnover the subnet allows. They help owners balance growth, stability, and competition among node operators.

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

### Consensus and Attestation Settings

Owners can adjust consensus-related policy for their subnet, including the minimum consensus-node attestation percentage and the validator node count decay used in consensus calculations.

These settings influence how the subnet evaluates consensus participation and how validator/node-count history affects consensus behavior over time. They are constrained by network-level limits and, where applicable, update intervals.

### Node Burn Rate Settings

Owners can update the node burn rate alpha. This setting influences how node registration burn rates respond to registration activity over time.

At a high level, this gives owners a way to tune the subnet's registration pressure so node entry costs can respond to demand without being fully static.

## Boundaries

Subnet owners have broad control over their own subnet, but they do not control the whole network. Network governance and runtime configuration still define global limits such as allowed parameter ranges, pause limits, cooldowns, registration cost behavior, and reward percentages.

Owner privileges are meant to let a subnet operate independently while keeping its behavior compatible with the wider network.
