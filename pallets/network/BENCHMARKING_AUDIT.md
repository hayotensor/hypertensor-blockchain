# Network pallet benchmarking audit

Date: 2026-08-30

## Automated Overwatch eligibility-removal addendum (2026-08-31)

Validator reputation and the automated Overwatch qualification policy were deleted directly. The
benchmark and weight interfaces consequently removed the five reputation-factor and qualification
setters while preserving every surviving dispatch index. The remaining schema has 220 ordinary
benchmarks, 220 `WeightInfo` methods, and 220 methods in each generated implementation. Two
`#[benchmark(extra)]` cases measure sole-pending-participant owner and collective removal without
adding public weight methods.

Thirty-seven affected public weights were regenerated from the release runtime with compiled Wasm,
50 steps, 20 repeats, proof recording enabled, and verification enabled. These runs used the
bencher's default min-squares timing analysis and median-slopes measured-PoV analysis. Fixtures,
not statistical extrapolation, establish the protocol maxima: 64 Overwatch nodes, 17 subnets, a
1,088-record reveal cohort, maximum prior latest-effective inputs/cache, and maximum current plus
pending removal state.

The lifecycle fixtures now enforce the following reachable states:

- commit grows a cumulative 16-entry row to its 17-entry bound;
- reveal starts from the maximum reachable 64-node by 17-subnet cohort while leaving the measured
  entries absent;
- rollover snapshots 64 canonical active owners and exact close-time stakes;
- all four settlement regions replace maximum prior retained inputs and effective cache state;
- allocation reads a valid 17-key latest-effective cache rather than finalized history;
- normal removal purges maximum current, pending, peer-index, and latest-effective state; and
- the two extra cases remove the sole pending participant and finalize an explicit empty epoch.

The public removal weights are component-wise envelopes of the normal and sole-pending cases. The
owner envelope is `(2_789_295_000 ref-time, 38_357 proof bytes, 22 reads, 35 writes)`; the collective
envelope is `(2_935_629_000 ref-time, 37_726 proof bytes, 20 reads, 35 writes)`. The same values and
storage annotations are present in the generic and RocksDB implementations.

Final validation after the guarded 37-method, two-implementation splice was:

- ordinary pallet regression tests: 785 passed, 0 failed;
- generated benchmark harness: 222 passed, 0 failed (220 public plus two extras);
- RPC wire-shape tests: 6 passed, 0 failed; custom RPC tests: 3 passed, 0 failed;
- pallet, runtime-benchmark runtime, runtime API, and precompile checks passed;
- Solidity compilation and TypeScript/Markdown formatting checks passed;
- structural parity and component-wise removal-envelope assertions passed;
- `cargo fmt --all -- --check` and `git diff --check` passed; and
- the repository-wide search found no deleted eligibility/reputation symbol.

The checked-in EVM TypeScript project still requires its generated
`@polkadot-api/descriptors` package before `tsc --noEmit` or chain-backed tests can run. Solidity
compilation and ABI/Solidity/precompile selector parity do not depend on that missing generated
package and were verified separately.

## Reward-first pending-removal addendum

Reward settlement no longer performs physical node removal. It writes at most one bounded active
set and one bounded registered set per subnet settlement, emits one bounded event, and leaves
physical cleanup to independently admitted post-election work or authenticated node calls. The
old four-removals-per-settlement model and `MaxConsensusNodeRemovalsPerSettlement` selector have
therefore been retired.

The benchmark fixtures now assert logical quarantine rather than inline deletion:

- rejected and emergency settlements stop after marker persistence and retain all physical nodes;
- accepted queue removal retains the registered record, removes it from `SubnetNodeQueue`, and
  writes `PendingRegisteredNodeRemovals`;
- regular election uses the physical candidate count for selection while persisted eligibility is
  filtered through one pending-set snapshot;
- proposal fixtures exercise the frozen elected-round source and the two pending-set reads.

The focused compiled-Wasm regeneration was run at 50 steps and 20 repeats. The two pending-set scan
selectors, regular/emergency/fallback election, regular/emergency proposal, and every affected
node-addressed dispatch now use measured storage metadata and timing constants. Election and
proposal still compose the scan selectors at their reachable maximums because their normal
fixtures cannot simultaneously maximize pending-set decoding and successful output persistence.
`elect_validator_expired(x, e)` likewise keeps the complete emergency-election envelope, safely
covering an active emergency scan that falls back to the regular set.

Settlement keeps one small conservative marker envelope in addition to its generated branch. This
represents the independent maximum active-marker insertion case, which is mutually exclusive with
paying every healthy node in the accepted fixture. Per-node physical cleanup reserves the complete
generated active/registered removal branch plus marker clearing before mutation; dispatch cleanup
uses the dynamic maximum branch surcharge. These deliberate compositions favor a simple auditable
upper bound without coupling reward completion to cleanup capacity.

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
| Overwatch reveal records | 1,088 (`64 * 17`) | `PendingOverwatchSettlement.reveal_records` |
| Ready-swap queue | 1,000 | `SwapQueueCount` |
| Pending active removals per subnet | 512 | `PendingActiveNodeRemovals` bounded set |
| Pending registered removals per subnet | 64 | `PendingRegisteredNodeRemovals` bounded set |
| Overwatch reveal salt | 64 bytes | `BoundedVec` in `OverwatchReveal` |
| Validator-owned nodes repaired lazily after subnet removal | 512 | `TotalValidatorNodes`; owner-local dispatches reserve the maximum cleaner weight |

Integrity checks reject incompatible runtime configurations. Registration and owner updates also
check caller-controlled map/set cardinality before iteration. The union of the stored initial
whitelist and historical `InitialValidatorData` keys cannot exceed 64, preventing repeated
whitelist rotation from growing a removal proof outside the generated domain.

## Function-by-function review

| Requested function | Model now used | Storage/proof state represented | Result |
|---|---|---|---|
| `commit_overwatch_subnet_weights` | `x = 1..17` distinct live subnets | `SubnetsData` is max-filled for each per-item existence read. The node's bounded commit row starts with `17 - x` disjoint entries, so every sample decodes and rewrites a cumulatively full 17-entry row. Duplicate/invalid subnet filtering cannot increase work beyond `x`. | Correct after update. Repeated-call subnet churn cannot exceed the row bound, and validation completes before the single row write. |
| `reveal_overwatch_subnet_weights` | `x = 1..17` new reveal records | Reads `x` maximum-length committed hashes/salts and rewrites one bounded reveal row. `ActiveOverwatchRevealStats` and all other reveal rows start at the largest reachable subnet/record pre-state while leaving exactly the measured entries absent. | Correct after update. The salt, row, and aggregate proof sizes are bounded and maximized, and validation completes before either aggregate or row storage is written. |
| `set_validator_node_delegate_stake_weights` | `x = 1..512` validator-owned nodes and exact update entries | `TotalValidatorNodes` selects `x`; `ValidatorSubnetNodes` is populated across the largest reachable outer-map domain, and both the old and replacement `ValidatorNodeDelegateStakeWeights` maps contain `x` normalized entries. | Correct after update. Runtime requires a complete, duplicate-free, exact allocation, matching the benchmark. |
| `elect_validator` | Regular `x = 3..512`; emergency `e = 3..64`; expired/fallback `x,e` | Regular election maxes the physical election-slot vector. Both bounded pending sets are loaded once; candidate filtering and the circular successor scan are then in memory. Emergency election decodes `e` complete node records. | Regenerated. Hook admission takes the maximum reachable branch; the expired model additionally composes a complete emergency scan before regular fallback. |
| `do_remove_subnet` | `a = 1..512` active nodes, `r = 1..64` registered nodes, `o = 1..64` subnet-local Overwatch peer entries; two fixed cleanup components | Measures all subnet-keyed `clear_prefix` operations and scalar/index updates. It does not scan validator-wide ownership/allocation maps or the global Overwatch forward index. Maximum bootnodes, bootnode access, paused metadata, slot assignments, initial-validator maps, and emergency snapshots are represented. | Correct after update. Validator indexes are repaired by a separately benchmarked owner-local cleaner using a fixed 512-node worst case; Overwatch forward entries are repaired by the affected node's peer update (bounded by 17 physical subnets). `NodeRegistrationInitialValidatorIds`/`InitialValidatorData` and emergency cleanup remain fixed maximum components because those lifecycle values are not independent of `a/r`. |
| `remove_active_subnet_node` | `n = 1..512` total nodes owned by the validator; small and large `e` election-state regions plus dispatch wrappers | Max-fills `ValidatorSubnetNodes`, `ValidatorNodeDelegateStakeWeights`, node/index records, election slots, and emergency data. A separate `SubnetNodeValidatorId` selector is charged before the branch model. | Correct after update. Models no longer use outer-subnet count and same-subnet count as incomplete proxies for total encoded ownership. |
| `remove_registered_subnet_node` | `n = 1..512` total validator ownership and `r = 1..64` registered queue entries; dispatch wrapper included | Covers queue decode/rewrite, registered node value, ownership and allocation maps, scalar counters, and indexes. | Correct after update. Runtime selector values are maintained on registration and all removal paths. |
| `handle_subnet_emission_weights` | `x = 1..17` eligible subnet rounds plus an empty branch | Iterates subnet keys, reads every elected round, and uses maximum 512-entry eligible node/identity collections so `contains_key` proofs are measured at the reachable encoded maximum. Both branches independently seed a valid 17-key effective Overwatch cache, including raw keys for subnets no longer live. | Correct after update. Empty live state has its own fixed benchmark without assuming an empty latest-only cache. |
| `execute_ready_swap_calls` | Queue `q = 1..1000`; homogeneous ready prefixes `x = 1..1000`; three mixed vertices `x = 3..1000` | Separates `SwapQueueOrder` decode/rewrite from per-call work. Validator and subnet destinations use max-filled metadata; refunds use maximum unbonding ledgers. Mixed validator-, subnet-, and refund-dominant fixtures include the union of all branch proofs. Public swap benchmarks enqueue into a queue at `MaxSwapQueueLength - 1`. | Correct after update. Hook binary search and final consumption use the same homogeneous/mixed envelope, including the 1,000-call boundary. Existing-destination credit rejection followed by an exact refund is covered by adding one complete refund model per admitted call. |
| `do_epoch_preliminaries` | `x = 0..17` physical subnet records | Uses maximum `SubnetsData` values, captures the six-component locked-TVL base once on delegate-stake removal epochs, and includes the compact removal selectors read for each subnet before any variable cleanup is admitted. Actual removal is composed through `try_do_remove_subnet`. | Correct and conservative. Every subnet in one pass uses the same funding base. Selector reads are also reserved by the removal helper, producing safe double reservation when removal occurs. |
| `advance_overwatch_epoch` | Fixed maximum close with 64 revealing nodes; separate no-op branch | The close fixture seeds 64 canonical validator-to-Overwatch-node relationships with unequal 18-decimal stakes, then verifies the complete bounded settlement snapshot. | Correct after snapshot update. The fixed weight covers every membership and stake read needed to build the close-time snapshot; later node removal may purge an entry before settlement. |
| `calculate_overwatch_rewards` | Empty; `r = 1..17`; `r = 17..64`; `r = 64..1088` | The three regions reflect the correlation between at most 64 revealing nodes and 17 revealed subnets. Fixtures seed an exact reveal matrix, compact pending header, matching close-time stakes with exponent `0.9`, and the maximum prior 64-node by 17-subnet latest-only input/cache that finalization replaces. | Correct after snapshot update. Boundary values use the component-wise maximum of adjacent regressions, and even an empty new epoch measures replacement of maximum prior latest-only state. |
| `emission_step` | Accepted/rejected `h = 3..512`; emergency `h = 64..512`; accepted queue `q = 1..64`; below-min reputation `h = 3..512`; non-attestors `a = 1..509`; missing and selector branches | Settlement fixtures include bounded marker writes and no physical deletion. Election, cleanup, queue activation, and burn maintenance are independently admitted after the complete settlement envelope. | Regenerated. The accepted path composes both reputation loops and the settlement-only marker reserve; physical-removal weights are consumed only by deferred cleanup. |
| `propose_attestation` | Regular 512-node and emergency 64-validator frozen rounds | Loads both pending sets once, filters scores before canonical duplicate/overflow handling, and filters reward/quorum/attestor snapshots from the frozen elected round. | Regenerated. Both generated proposal weights retain measured maximum pending-set scan compositions. |
| pending cleanup and node calls | Active scan `a = 1..512`; registered scan `r = 1..64`; physical active `n,e`; physical registered `n,r`; normal and pending dispatch branches | `pending_active_removal_scan(a)` and `pending_registered_removal_scan(r)` cover bounded-set decode/materialization plus a full deterministic membership scan. Slot cleanup then uses the existing complete generated physical removal branch per admitted ID. Node-addressed dispatches select the pending branch dynamically after authentication and reserve maximum active/registered cleanup plus marker proof. | Correct after focused regeneration and conservative maximum-branch composition. |
| `precheck_subnet_consensus_submission` | `x = 3..512` snapshot entries plus a missing branch | Uses the elected round and consensus snapshot at `x`, with proposal args and attestation payloads stored separately from the compact main submission. `SubnetConsensusSubmissionMaxItems` lets the hook reserve `x` without decoding the large value first. | Correct after update. All caller-controlled consensus vectors are capped at 512 before iteration. |
| `calculate_subnet_weights` | `x = 0..17` eligible subnets | Uses maximum-size elected rounds for each historical subnet-election membership check and exact latest-effective Overwatch/default inputs. | Correct after update. Zero-subnet state remains explicitly sampled, and historical Overwatch outputs are not an allocation fallback. |

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

Subnet removal immediately clears all subnet-keyed state with `clear_prefix`. It intentionally
leaves owner-wide validator ownership/allocation entries and Overwatch forward peer entries for
bounded owner-local repair. Validator node registration and validator self-removal reserve the fixed
maximum `clean_validator_subnet_nodes()` weight; Overwatch peer updates scan only that node's bounded
17-subnet map. RPC output filters stale Overwatch forward entries until the owner next updates.

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
- Election and emission admission take the maximum complete regular, emergency, expired/fallback,
  accepted, rejected, and missing branch envelopes when compact state cannot identify the branch
  cheaply. Independent pending-set or marker envelopes remain composed where maxima are mutually
  exclusive in a single benchmark fixture.
- Ready-swap admission uses the maximum homogeneous/mixed affine vertex, adds the one-item
  boundary reserve at a 1,000-call mixed prefix, and composes one complete refund allowance per
  call so failed destination credit plus refund is covered.
- `do_epoch_preliminaries` includes removal-selector reads in its per-subnet model while
  `try_do_remove_subnet` also charges those reads before cleanup.
- Empty historical queues are occasionally charged at the one-item queue boundary. This is an
  overcharge only; no variable collection is read without a prior reservation.

These choices trade some block capacity for a simple, auditable upper bound. None is an
undercharge.

## Validation and weight generation

Validation was repeated after the affected generated weights were merged into the complete weight
file:

- `cargo check -p pallet-network --features runtime-benchmarks`
- `cargo check -p hypertensor-runtime --features runtime-benchmarks`
- all generated pallet benchmark tests: 223 passed, 0 failed
- ordinary pallet unit/regression tests: 765 passed, 0 failed
- `cargo fmt --all -- --check`
- `git diff --check`
- release runtime rebuilt with `runtime-benchmarks`; its benchmark list contains all 223 methods

For the 2026-08-30 pending-removal addendum, final validation is:

- `cargo check -p pallet-network --lib`
- `cargo check -p pallet-network --features runtime-benchmarks`
- `cargo check -p hypertensor-runtime --features runtime-benchmarks`
- `cargo build --release -p hypertensor-node --features runtime-benchmarks`
- all generated pallet benchmark tests: 225 passed, 0 failed
- ordinary pallet unit/regression tests: 787 passed, 0 failed, including all 18 focused
  pending-removal regressions
- focused compiled-Wasm regeneration: 2 pending scans, 3 election branches, 2 proposal
  branches, 10 node-addressed dispatches, and 8 settlement branches at 50 steps and 20 repeats
- structural parity: 225 active benchmarks and 225 methods in the trait and each `WeightInfo`
  implementation
- `cargo fmt --all -- --check`
- `git diff --check`

The focused pending-removal regeneration rebuilt the benchmark runtime and used the same settings
as the existing generated file. The reproducible command shape is:

```bash
cargo build --release -p hypertensor-node --features runtime-benchmarks
/home/bob/.cargo/bin/frame-omni-bencher v1 benchmark pallet \
  --runtime target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm \
  --pallet pallet_network \
  --extrinsic elect_validator,elect_validator_emergency,elect_validator_expired,pending_active_removal_scan,pending_registered_removal_scan,propose_attestation,propose_attestation_emergency,attest,remove_subnet_node,update_node_hotkey,add_node_stake,remove_node_stake,update_node_unique,update_node_non_unique,update_node_peer_info,update_node_bootnode_peer_info,update_node_client_peer_info,emission_step,emission_step_accepted_queue_mutations,emission_step_accepted_queue_mutations_front,emission_step_accepted_below_min_weight_reputation,emission_step_accepted_non_attestor_reputation,emission_step_missing,emission_step_rejected,emission_step_emergency \
  --steps 50 \
  --repeat 20 \
  --wasm-execution compiled \
  --output-analysis max \
  --output-pov-analysis max \
  --output /tmp/pallet_network_pending_weights.rs \
  --template ./.maintain/frame-weight-template.hbs
```

Only the selected generated regions were merged. The pending-set scan selectors remain composed
into election/proposal and cleanup because maximum pending-set decoding and maximum successful
output persistence cannot be reached in one fixture. The emergency election composition likewise
retains the complete fallback branch. Settlement retains its explicit marker reserve because its
maximum healthy payout fixture and maximum threshold-crossing marker writes are mutually exclusive.

A post-generation structural audit also confirmed that all 225 active `#[benchmark]` functions
match the 225 `WeightInfo` methods and both generated implementations. Every one of the 47 `u32`
components has a positive measured reference-time slope, every component argument is consumed,
and no placeholder, alias, zero-weight, or retired method remains. All runtime `WeightInfo` call
sites resolve.

The fourteen weights affected by queued-swap principal safety were regenerated individually from
the rebuilt compact WASM with proof recording and verification enabled: the four public swap calls,
the two wallet-funded add calls, the two destination-credit helpers, and the six homogeneous/mixed
execution branches. Unaffected methods retain their previously generated values. Each affected
benchmark used:

```bash
/home/bob/.cargo/bin/frame-omni-bencher v1 benchmark pallet \
  --runtime target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm \
  --pallet pallet_network \
  --extrinsic <affected-benchmark> \
  --steps 50 \
  --repeat 20 \
  --wasm-execution compiled \
  --output-analysis max \
  --output-pov-analysis max \
  --output pallets/network/src/weights.rs \
  --template ./.maintain/frame-weight-template.hbs
```

The fourteen weights affected by validator-only Overwatch ownership were regenerated with the
same settings: registration, hotkey update, removal, peer update, commit, reveal, stake add/remove,
collective removal, validator whitelist, and the four reward-settlement envelopes. These weights
include the authoritative `ValidatorOverwatchNodeId` reads and writes and no longer charge the
removed subnet-node diversification scan.

The rollover weight and four reward-settlement envelopes were regenerated again after adding the
bounded close-time settlement snapshot. Rollover now measures the maximum 64-node snapshot, while
settlement reads the matching post-removal snapshot instead of live membership, stake, or exponent
storage.

Nineteen weights were regenerated in one comma-selected run on 2026-08-27 after introducing locked
non-Overwatch TVL accounting: the three subnet-funding threshold consumers, five network unstake
paths, the Overwatch unstake path, the maximum-entry unbonding claim, all four public swap calls,
and the six homogeneous/mixed swap execution models. The run used the same 50-step, 20-repeat,
compiled-Wasm, `max` timing/PoV settings shown above and wrote a standalone subset before its
guarded method-region merge. The unstake fixtures merge into a maximum-capacity ledger, and the
claim fixture processes the maximum matured entry count. The generated storage metadata includes
`TotalNetworkUnbondingBalance` and `TotalQueuedSwapPrincipal`; removed unbonding and node-delegate
aggregates no longer appear. Only those nineteen generated method regions were merged, leaving the
other 203 methods byte-for-byte unchanged.

The resulting file records 50 steps and 20 repeats using compiled Wasm. The `max` timing and PoV
analyses select the larger built-in regression result and are deliberately conservative. Proof
recording and benchmark verification are not disabled.

Twelve weights were regenerated on 2026-08-28 for loop-free subnet removal and lazy owner-local
cleanup: subnet activation and the two public subnet-removal calls, `do_remove_subnet`,
`clean_validator_subnet_nodes`, `do_epoch_preliminaries`, the three subnet-node mutation calls,
node unstaking, validator node-allocation updates, and Overwatch peer updates. A subsequent
four-method run regenerated validator self-removal, collective node removal, node unstaking, and
validator allocation updates after narrowing validator cleanup to node registration and validator
self-removal only. The removal model now has only `a/r/o` subnet-local components; its maximum
RocksDB ref-time reservation is approximately `888.5e9`, below the runtime's `1e12` hook ref-time
limit. The validator cleaner uses a fixed maximum 512-node model and is never executed by the
consensus or collective removal paths.

Seven queued-swap execution weights were regenerated on 2026-08-30 after introducing non-blocking
rotation for mature items whose refunds cannot be recorded. The queue-rebuild benchmark measures
the all-rotated case using the same bounded per-ID pushes as production. The six homogeneous and
mixed execution envelopes were regenerated in the same 50-step, 20-repeat compiled-Wasm run with
proof recording and verification enabled. Only those seven generated method regions were merged.
