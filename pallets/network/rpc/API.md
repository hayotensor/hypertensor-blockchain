# Network JSON-RPC v1

This is the clean, typed network RPC contract. It intentionally has no legacy aliases and does not
return SCALE-encoded response blobs.

Validator identity metadata is stored in `Network.ValidatorMetadata(validatorId)`, separately
from the operational `Network.ValidatorsData(validatorId)` record. Validator RPC responses
continue to include the optional `identity` field by combining these records. A missing metadata
entry means `identity: null`; an explicitly stored identity with all fields absent remains an
identity object. Clients querying storage directly must use the separate map. This is the
prelaunch storage layout; no migration or fallback to the previous embedded identity is provided.

Live node networking and descriptive fields are stored in
`Network.SubnetNodeMetadata(subnetId, subnetNodeId)`. Its `NodeMetadata` value contains
`peer_info`, `bootnode_peer_info`, `client_peer_info`, `unique`, and `non_unique`, with the same
field limits as the registration and update calls. A missing entry means all five fields are
absent. An explicitly present empty byte vector remains distinct from an absent field.

The raw `SubnetNodesData` and `QueuedSubnetNodesData` values now contain only `id`,
`validator_id`, and `classification`, in that order. `SubnetNodeQueue` is an ordered vector of
the same compact records. The metadata map uses `Identity` for both `u32` keys and `OptionQuery`.
One entry survives the transition from Queued to Probation to Active; quarantine retains it and physical
node removal deletes it. Whole-subnet removal clears the metadata prefix. Direct storage clients
must join the new map with an existing operational record. Metadata alone does not establish
that a node exists.

Node metadata remains available in RPC and contract responses. The declared
`SubnetNodeRegistered` event has a separate `SubnetNodeRegistrationData` payload type.
Historical `ConsensusSubnetNode` snapshots retain proposal-time classification.
All classifications use the three-state schema below, with no migration or fallback reader.

Node `classification.nodeClass` is `queued`, `probation`, or `active`; SCALE and contract
codes are 0, 1, and 2 respectively. Subnet lifecycle states are unchanged. Node information
exposes `probationProgress`; subnet configuration exposes `probationRounds` and
`pendingProbationRounds`; consensus policy exposes its snapshotted `probationRounds`.
Probation progress counts consecutive qualifying inclusions, not elapsed epochs. It
advances only on ordinary accepted nonzero-total-score rounds with identity supermajority;
endorsed omission resets it. Emergency and other non-qualifying rounds leave it unchanged.
A zero-score entry still counts in a nonzero-total-score round. Promotion additionally
requires full reputation. Probation nodes earn no node-related rewards and cannot vote.

`totalAdmittedNodes` counts Probation and Active nodes occupying subnet capacity.
`totalElectableNodes` is the separate physical election-candidate count; eligibility resolution
excludes pending removals. Bootnode results expose `admittedNodes`, and historical proposal
results expose `admittedSubnetNodeIds`, both covering Probation and Active nodes.
Storage uses `TotalAdmittedSubnetNodes`, `TotalAdmittedNodes`, `SubnetNodeProbationProgress`,
`ProbationRounds`, and `PendingProbationRounds`. Quarantine uses
`PendingAdmittedNodeRemovals` and `PendingQueuedNodeRemovals`. Admission emits
`SubnetNodeAdmitted`; promotion emits `SubnetNodePromoted`.

`Network.SubnetConsensusSubmission(subnetId, subnetEpoch)` appends `weight_sum: u128`
and `eligible_validator_identity_count: u32` to its stored `ConsensusData`, in that order
after `emergency`. This adds 20 SCALE bytes. The runtime computes these immutable summaries
from the validated proposal scores and the final eligible-identity map after quarantine
filtering. Attestations change only votes; settlement reuses the summaries while computing
actual participation and checking live payment/removal state. Direct storage readers must
use the new runtime metadata. Composed RPC responses and the Solidity ABI remain unchanged.
This prelaunch layout has no migration, legacy reader, or malformed-storage repair fallback.

## Wire conventions

- `u128` values (balances, shares, fixed-point percentages, weights, and reputation) are decimal
  JSON strings, for example `"1000000000000000000"`.
- Arbitrary byte fields are lowercase, `0x`-prefixed hex strings. `H256` values
  use their standard `0x`-prefixed JSON forms.
- Enums and object fields use `camelCase`.
- Optional entity and historical-round lookups return `null` when the requested record does not
  exist. Methods with non-optional page or status results return a domain error when their parent
  subnet or validator does not exist. Invalid page limits and inconsistent stored snapshots are
  also JSON-RPC errors.
- The RPC surface does not enumerate unbounded user-generated collections. Validator identity
  enumeration, account delegation-position discovery, and Overwatch commit/reveal history belong
  in archive-node storage queries or an external indexer. Direct entity lookups and selected
  protocol-bounded current collections remain available.
- Every method accepts an optional `at` block hash as its final parameter. If omitted, the node's
  best block is used.

## Pagination

Paged methods accept `{ "cursor": null, "limit": 50 }`. `limit` must be in `1..=100`.

`nextCursor` is an exclusive, opaque continuation token. Pass it back unchanged. Continue until it
is `null`. To obtain a consistent multi-page snapshot, resolve a block hash first and pass that
same `at` hash to every request in the traversal.

Validator-wide node, stake, and allocation collections are protocol-bounded to at most 512 entries
per validator identity, while their JSON responses remain paged to at most 100 entries.

Subnet lists contain only the live protocol-bounded subnet set. `network_getSubnetNodes` contains
only admitted Probation and Active nodes and excludes the registration queue; the runtime ceiling is 512 admitted nodes per
subnet. `network_getBootnodes` is unpaged and returns two separately bounded arrays: up to 256
official subnet bootnodes and bootnode endpoints advertised by up to 512 admitted subnet nodes.
Overwatch membership is capped at 64. `network_getSubnetInfo` deliberately omits the registration
validator whitelist and registration tracking maps.

## Excluded collections

The API intentionally has no methods that enumerate all validator identities, account subnet/node/
validator delegation positions, or Overwatch commit/reveal history. Those collections can grow with
user activity or historical churn and must be queried through archive-node storage or an external
indexer. The API also omits registration queues, even though each subnet queue is bounded by its
`MaxRegisteredNodes` setting and the runtime's corresponding upper bound. Point lookups such as
validator and admitted or queued subnet-node information remain available.

## Methods

| Method | Parameters before optional `at` | Result |
| --- | --- | --- |
| `network_getSubnetInfo` | `subnetId` | Subnet details or `null` |
| `network_getSubnets` | `page` | Page of subnets |
| `network_getSubnetNodeInfo` | `subnetId`, `subnetNodeId` | Node details or `null` |
| `network_getSubnetNodes` | `subnetId`, `page` | Page of admitted subnet nodes (maximum 512) |
| `network_getBootnodes` | `subnetId` | Official and admitted-node bootnodes or `null` |
| `network_getValidatorInfo` | `validatorId` | Validator identity/economic summary or `null` |
| `network_getValidatorByColdkey` | `coldkey` | Validator summary or `null` |
| `network_getValidatorByHotkey` | `hotkey` | Validator summary or `null` |
| `network_getValidatorNodes` | `validatorId`, `page` | Page of nodes canonically owned by a validator |
| `network_getValidatorNodeStakes` | `validatorId`, `page` | Page of the validator's node stake balances |
| `network_getValidatorNodeAllocations` | `validatorId`, `page` | Page of validator-delegate-pool node allocations |
| `network_getConsensusRound` | `subnetId`, `subnetEpoch` | Election/policy snapshot and current proposal state, or `null` |
| `network_getSubnetValidatorNodes` | `subnetId`, `page` | Current effective validator-node page |
| `network_getSubnetEpochStatus` | `subnetId` | Current phase, timing, election, proposal, and validator-set status |
| `network_getOverwatchNodeInfo` | `overwatchNodeId` | Active Overwatch member details or `null` |
| `network_getOverwatchNodes` | `page` | Page of active Overwatch members |
| `network_getEffectiveOverwatchSignalMeta` | none | Latest effective signal source epoch, revision, and validity |
| `network_getEffectiveOverwatchSubnetWeight` | `subnetId` | Raw-entry presence/value and resolved weight used for future emissions |

The effective Overwatch point lookup reports `rawWeightExists` separately so a stored zero is not
confused with a missing entry. For a valid raw entry, `resolvedWeight` applies the current
Overwatch factor and caps the result at Q18. It uses the protocol default directly when no valid
signal or no raw entry exists. Historical epoch weights remain available through pallet storage
and are not rewritten when active membership changes.

`network_getSubnetValidatorNodes` means the current effective candidate set, not nodes that have
attested. A pending or expired emergency set does not replace the regular set. An active emergency
set includes only members that remain Active-class nodes. Actual proposal-time eligibility and
attestations are exposed by `network_getConsensusRound`.

The consensus-round result preserves the elected node and validator identity, election source,
complete candidate snapshot, delegate balance, and policy at election. Once submitted, the
proposal-time eligible-attestor snapshot is also fixed, but the recorded attestation entries can
accumulate while that round remains open. After the round closes, the result is historical. It does
not reconstruct old rounds from current node metadata.

## Errors

| Code | Meaning |
| --- | --- |
| `-32602` | Invalid parameters; the network adapter uses this for a page limit outside `1..=100` |
| `-32001` | Runtime API invocation failed at the selected block |
| `-32010` | Network domain error, such as a missing parent entity or inconsistent state |

Domain errors include a typed JSON value in the error `data` field.

Accounts use the SDK `AccountId32` type and serialize as SS58 strings.
