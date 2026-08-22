# Network Pallet Architecture

The network pallet manages on-chain roles, subnet lifecycle state, epoch scheduling, consensus,
staking, reputation, and rewards. The diagram shows those logical relationships; it is not a
peer-to-peer topology.

```mermaid
flowchart TB
    B[(Blockchain)] --- P[Network pallet]

    O[Overwatch nodes] -->|register, stake, and commit/reveal subnet weights| P
    P -->|reputation and rewards| O

    V[Validator identities]
    D[Delegators] -->|delegate stake| P

    subgraph S[Each registered subnet]
        SO[Subnet owner] -->|lifecycle and policy controls| SD[Subnet state, metadata, and assigned slot]
        SD --- N[Subnet nodes: Registered, Idle, Included, or Validator]
        SD --- BN[Official bootnode set]
    end

    V -->|register and own| N
    SO -->|owner extrinsics| P
    N -->|proposals and attestations| P
    P -->|elections, queue processing, reputation, and rewards| N
    P -->|stores and schedules| SD
    SO -->|manages directly or delegates access| BN
    BN -.->|peer-discovery metadata| N
    BN -.->|peer-discovery metadata| O
```

Subnet nodes and overwatch nodes interact with the blockchain through pallet extrinsics. The
pallet stores official bootnode sets and node peer information, but it does not store or require
links between every pair of nodes. In particular, subnet nodes are not required by on-chain logic
to form a full mesh.

Each subnet receives an assigned slot that defines its local epoch boundary. At that slot, the
pallet can settle an allocated exact prior round even if the subnet has since been paused. If the
subnet is `Active`, it attempts a new proposer election once consensus-eligible and stores one when
the effective candidate set is valid and nonempty, then processes its registration queue and
updates its node burn rate. Overwatch nodes run a separate commit-reveal process; finalized
overwatch subnet weights are one input to subnet emission weighting.
