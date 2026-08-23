# Network pallet benchmarking audit

Date: 2026-08-22

## Outcome

The requested benchmark paths were compared with their runtime implementations, hook-side
weight selection, storage access patterns, and all protocol paths that can grow the measured
collections. The previous models were not uniformly safe: several `Linear` components described
the wrong cardinality, some unbounded encoded values had no enforced runtime ceiling, correlated
dimensions were sampled as if they were independent, and several hook branches were not composed.

Those gaps are now corrected. Every variable collection used by the requested paths is either:

- bounded by a compile-time protocol limit that matches the benchmark domain;
- selected through a compact scalar maintained with the collection; or
- covered by a fixed maximum-value benchmark when its cost does not vary independently.

No known undercharged branch remains in the fourteen requested areas. Some hook reservations are
intentionally conservative; those cases are listed below.

## Enforced benchmark domains and selectors

The following limits are now protocol invariants rather than assumptions made only by fixtures:

| Domain | Bound | Weight-selection storage |
|---|---:|---|
| Physical subnet records, including the rotation subnet | 17 | `TotalSubnets` |
| Active/electable nodes per subnet | 512 | `TotalActiveSubnetNodes`, `TotalSubnetElectableNodes` |
| Nodes owned by one validator | 512 | `TotalValidatorNodes` |
| Registered queue entries and initial-validator identities | 64 | `TotalSubnetNodes - TotalActiveSubnetNodes`; bounded registration inputs |
| Emergency validator snapshot | 64 | fixed maximum branch / emergency branch envelope |
| Overwatch nodes | 64 | `TotalOverwatchNodes` |
| Overwatch reveal subnets | 17 | `ActiveOverwatchRevealStats` |
| Overwatch reveal records | 1,088 (`64 * 17`) | `PendingOverwatchSettlement.records` |
| Ready-swap queue | 1,000 | `SwapQueueCount` |
| Node removals during one consensus settlement | 4 | fixed configuration bound |
| Overwatch reveal salt | 64 bytes | `BoundedVec` in `OverwatchReveal` |
| Surviving validator ownership entries during subnet removal | 9,216 (`16 * (512 + 64)`) | `TotalNodes - target TotalSubnetNodes` |

Integrity checks reject incompatible runtime configurations. Registration and owner updates also
check caller-controlled map/set cardinality before iteration. The union of the stored initial
whitelist and historical `InitialValidatorData` keys cannot exceed 64, preventing repeated
whitelist rotation from growing a removal proof outside the generated domain.

## Function-by-function review

| Requested function | Model now used | Storage/proof state represented | Result |
|---|---|---|---|
| `commit_overwatch_subnet_weights` | `x = 1..17` distinct live subnets | `SubnetsData` is max-filled for each per-item existence read; every `OverwatchCommits` key is absent and then written. Duplicate/invalid subnet filtering cannot increase work beyond `x`. | Correct after update. The component now represents actual accepted commits, not arbitrary vector entries. |
| `reveal_overwatch_subnet_weights` | `x = 1..17` new reveal records | Reads `x` maximum-length committed salts/hashes, checks absent `OverwatchReveals`, and writes `x` records. `ActiveOverwatchRevealStats` starts at the largest reachable node/subnet/record pre-state while leaving exactly the measured keys absent. | Correct after update. The salt and aggregate proof sizes are bounded and maximized, and batch validation is atomic. |
| `set_validator_node_delegate_stake_weights` | `x = 1..512` validator-owned nodes and exact update entries | `TotalValidatorNodes` selects `x`; `ValidatorSubnetNodes` is populated across the largest reachable outer-map domain, and both the old and replacement `ValidatorNodeDelegateStakeWeights` maps contain `x` normalized entries. | Correct after update. Runtime requires a complete, duplicate-free, exact allocation, matching the benchmark. |
| `elect_validator` | Regular `x = 3..512`; emergency `e = 3..64`; expired emergency `x,e` | Regular election maxes the election-slot vector and candidate snapshots. Emergency election decodes `e` complete node records. Expiry measures removal of a full historical emergency value plus fallback to `x` regular candidates. `TotalSubnetElectableNodes` is included in each branch model. | Correct after branch split. Hook admission takes the maximum reachable branch envelope. |
| `do_remove_subnet` | `a = 1..512` active nodes, `r = 1..64` registered nodes, `o = 1..64` overwatch nodes, `s = 1..9216` surviving ownership entries; two fixed cleanup components | Measures active/registered node prefixes, global overwatch index cleanup, validator ownership and delegate-weight normalization, and all scalar/index updates. Maximum bootnodes, bootnode access, paused metadata, slot assignments, initial-validator maps, and emergency snapshots are represented. | Correct after update. `NodeRegistrationInitialValidatorIds`/`InitialValidatorData` and emergency cleanup are fixed maximum components because those lifecycle values are not independent of `a/r`. |
| `remove_active_subnet_node` | `n = 1..512` total nodes owned by the validator; small and large `e` election-state regions plus dispatch wrappers | Max-fills `ValidatorSubnetNodes`, `ValidatorNodeDelegateStakeWeights`, node/index records, election slots, and emergency data. A separate `SubnetNodeValidatorId` selector is charged before the branch model. | Correct after update. Models no longer use outer-subnet count and same-subnet count as incomplete proxies for total encoded ownership. |
| `remove_registered_subnet_node` | `n = 1..512` total validator ownership and `r = 1..64` registered queue entries; dispatch wrapper included | Covers queue decode/rewrite, registered node value, ownership and allocation maps, scalar counters, and indexes. | Correct after update. Runtime selector values are maintained on registration and all removal paths. |
| `handle_subnet_emission_weights` | `x = 1..17` eligible subnet rounds plus an empty branch | Iterates subnet keys, reads every elected round, and uses maximum 512-entry eligible node/identity collections so `contains_key` proofs are measured at the reachable encoded maximum. | Correct after update. Empty state has its own fixed benchmark. |
| `execute_ready_swap_calls` | Queue `q = 1..1000`; homogeneous ready prefixes `x = 1..1000`; three mixed vertices `x = 3..1000` | Separates `SwapQueueOrder` decode/rewrite from per-call work. Validator and subnet destinations use max-filled metadata; refunds use maximum unbonding ledgers. Mixed validator-, subnet-, and refund-dominant fixtures include the union of all branch proofs. | Correct after update. Hook binary search and final consumption use the same homogeneous/mixed envelope, including the 1,000-call boundary. |
| `do_epoch_preliminaries` | `x = 0..17` physical subnet records | Uses maximum `SubnetsData` values and includes the compact removal selectors read for each subnet before any variable cleanup is admitted. Actual removal is composed through `try_do_remove_subnet`. | Correct and conservative. Selector reads are also reserved by the removal helper, producing safe double reservation when removal occurs. |
| `calculate_overwatch_rewards` | Empty; `r = 1..17`; `r = 17..64`; `r = 64..1088` | The three regions reflect the correlation between at most 64 revealing nodes and 17 revealed subnets. Fixtures seed an exact record matrix and the matching compact pending-settlement snapshot. | Correct after piecewise split. Boundary values use the component-wise maximum of adjacent regressions. |
| `emission_step` | Accepted/rejected `h = 3..512`; emergency `h = 64..512`; accepted queue `q = 1..64`; below-min reputation `h = 3..512`; non-attestors `a = 1..509`; missing and selector branches | `SubnetConsensusSubmissionMaxItems`, `TotalSubnetElectableNodes`, queue cardinality, elected-round snapshots, reputation maps, ownership selectors, and bounded removal work are composed outside mutually competing settlement branches where required. | Correct after update. The accepted path now charges both below-min-weight and non-attestor reputation loops; up to four ownership-heavy removals are reserved for every quorum branch. |
| `precheck_subnet_consensus_submission` | `x = 3..512` snapshot entries plus a missing branch | Uses the elected round and consensus snapshot at `x`, with proposal args and attestation payloads stored separately from the compact main submission. `SubnetConsensusSubmissionMaxItems` lets the hook reserve `x` without decoding the large value first. | Correct after update. All caller-controlled consensus vectors are capped at 512 before iteration. |
| `calculate_subnet_weights` | `x = 0..17` eligible subnets | Uses maximum-size elected rounds for each historical membership check and exact overwatch/subnet-weight inputs. | Correct after update. Zero-subnet state remains explicitly sampled. |

## Storage correctness notes

The selectors used by hook-side weight admission are deliberately scalar storage values. Reading a
large `Vec` or `BTreeMap` to learn its length before reserving its weight would itself be
undercharged. The runtime therefore maintains `TotalValidatorNodes`, `TotalSubnetElectableNodes`,
`SwapQueueCount`, `SubnetConsensusSubmissionMaxItems`, and compact Overwatch settlement/reveal
statistics on every mutation path. Tests cover their registration, activation, removal, settlement,
and queue-consumption updates.

Some storage values remain encoded as ordinary `Vec`/`BTreeMap` types, but their growth paths now
enforce the same compile-time limits used by their benchmark. This applies in particular to the
initial-validator whitelist/history, validator ownership/allocation maps, emergency snapshots, and
consensus submissions. Maximum-value fixtures are used where a value contributes proof size but
does not provide an independent runtime selector.

Four generated helper methods (`graduate_class`, `insert_node_into_election_slot`,
`get_min_subnet_delegate_stake_balance`, and the informational `calculate_subnet_weights`) do not
currently have a direct `WeightInfo` call site. Their helpers execute inside outer benchmarked
paths, so this is harmless generated API surface rather than an uncharged runtime path.

`SubnetNodeValidatorId` is retained as exit-state after a node leaves so staking/unbonding claims
can still resolve ownership. Removal benchmarks and cleanup assertions intentionally account for
that retained state rather than assuming the mapping is deleted with live node data.

## Intentional conservative reservations

- Subnet removal always composes the fixed Registered-initial-map and emergency cleanup weights,
  although those maximum values belong to mutually exclusive lifecycle states.
- Election and emission admission take the maximum complete regular, emergency, expired, accepted,
  rejected, and missing branch envelopes when compact state cannot identify the branch cheaply.
- Ready-swap admission uses the maximum homogeneous/mixed affine vertex and adds the one-item
  boundary reserve at a 1,000-call mixed prefix.
- `do_epoch_preliminaries` includes removal-selector reads in its per-subnet model while
  `try_do_remove_subnet` also charges those reads before cleanup.
- Empty historical queues are occasionally charged at the one-item queue boundary. This is an
  overcharge only; no variable collection is read without a prior reservation.

These choices trade some block capacity for a simple, auditable upper bound. None is an
undercharge.

## Validation and weight generation

Validation was repeated after the final generated weight file was written:

- `cargo check -p pallet-network --features runtime-benchmarks`
- `cargo check -p hypertensor-runtime --features runtime-benchmarks`
- all generated pallet benchmark tests: 224 passed, 0 failed
- ordinary pallet unit/regression tests: 712 passed, 0 failed
- `cargo fmt --all -- --check`
- `git diff --check`
- release runtime rebuilt with `runtime-benchmarks`; its benchmark list contains all 224 methods

A post-generation structural audit also confirmed that all 224 active `#[benchmark]` functions
match the 224 `WeightInfo` methods and both generated implementations. Every one of the 45 `u32`
components has a positive measured reference-time slope, every component argument is consumed,
and no placeholder, alias, zero-weight, or retired method remains. All runtime `WeightInfo` call
sites resolve.

Weights are generated from the rebuilt compact WASM with proof recording and verification enabled:

```bash
/home/bob/.cargo/bin/frame-omni-bencher v1 benchmark pallet \
  --runtime target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm \
  --pallet pallet_network \
  --extrinsic '*' \
  --steps 50 \
  --repeat 20 \
  --wasm-execution compiled \
  --output-analysis max \
  --output-pov-analysis max \
  --output pallets/network/src/weights.rs \
  --template ./.maintain/frame-weight-template.hbs
```

The resulting file records 50 steps and 20 repeats using compiled Wasm. The `max` timing and PoV
analyses select the larger built-in regression result and are deliberately conservative. Proof
recording and benchmark verification are not disabled.
