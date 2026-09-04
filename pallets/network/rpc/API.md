# Network JSON-RPC v1

This is the clean, typed network RPC contract. It intentionally has no legacy aliases and does not
return SCALE-encoded response blobs.

## Wire conventions

- `u128` values (balances, shares, fixed-point percentages, weights, and reputation) are decimal
  JSON strings, for example `"1000000000000000000"`.
- Arbitrary byte fields are lowercase, `0x`-prefixed hex strings. `AccountId20` and `H256` values
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
only active nodes and excludes the registration queue; the runtime ceiling is 512 active nodes per
subnet. `network_getBootnodes` is unpaged and returns two separately bounded arrays: up to 256
official subnet bootnodes and bootnode endpoints advertised by up to 512 active subnet nodes.
Overwatch membership is capped at 64. `network_getSubnetInfo` deliberately omits the registration
validator whitelist and registration tracking maps.

## Excluded collections

The API intentionally has no methods that enumerate all validator identities, account subnet/node/
validator delegation positions, or Overwatch commit/reveal history. Those collections can grow with
user activity or historical churn and must be queried through archive-node storage or an external
indexer. The API also omits registered-node queues, even though each subnet queue is bounded by its
`MaxRegisteredNodes` setting and the runtime's corresponding upper bound. Point lookups such as
validator and active or registered subnet-node information remain available.

## Methods

| Method | Parameters before optional `at` | Result |
| --- | --- | --- |
| `network_getSubnetInfo` | `subnetId` | Subnet details or `null` |
| `network_getSubnets` | `page` | Page of subnets |
| `network_getSubnetNodeInfo` | `subnetId`, `subnetNodeId` | Node details or `null` |
| `network_getSubnetNodes` | `subnetId`, `page` | Page of active subnet nodes (maximum 512) |
| `network_getBootnodes` | `subnetId` | Official and active-node bootnodes or `null` |
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
set includes only members that remain validator-class nodes. Actual proposal-time eligibility and
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
