# Election-time attestor snapshot measurements

Measured locally on 2026-11-14 (America/New_York), using Rust 1.93.0 and benchmark
CLI 60.0.0. All runs use disposable runtime state, compiled Wasm, proof recording,
and `max` analysis for both execution time and PoV. No live network is involved.

## Subnet-slot budget review (2026-11-14)

The election snapshot and mandatory settlement fit the current subnet-slot budget,
and signed user calls remain admissible in the tested reservation scenarios.
**A separate, preexisting history-pruning accounting defect prevents an
unqualified guarantee for every full-hook workload.** This review does not change
the 512-node limit, the Network hook allowance, or the runtime block limits.
The results below supersede the initial production reservations recorded later
in this document. All validation is authorized defensive testing in disposable
local state; no live network was contacted.

### Benchmark corrections and installed weights

- Both election fixtures previously used exponent `0.5`, which takes libm's
  square-root shortcut. They now use node-count decay `1/3` (penalty exponent
  `2/3`) and stake power `1/3`, exercising general fractional powers. Maximum
  ownership/allocation records, distinct identities, shared identities, and
  snapshot/liability verification remain enabled.
- Election now also reserves the independently benchmarked maximum pending-set
  scans (512 admitted and 64 queued entries). Pending Probation nodes can make
  these sets large even when few candidates are electable. This adds
  0.106582898 ms and 9,451 proof bytes, preserving the prior 32-KiB allowance.
- Fresh fitted models exceeded the prior election model slightly at larger
  counts and the accepted-settlement model at smaller counts. Only election,
  accepted settlement, and submitted precheck were refreshed; every previous
  envelope remains a componentwise floor.
- One precheck timing sample at 351 nodes took 0.698975 ms, above its previous
  0.557619255-ms reservation. A fixed 0.25-ms margin now covers this observed
  outlier independently of the fitted regression. Settlement already benchmarks
  precheck within its own envelope; the standalone margin is not charged twice
  by the outer hook.
- The merge script supports selective refreshes and preserves the new allowances
  exactly once. Two subsequent disposable merges produced numerically identical
  weights at every count from 3 through 512, leaving other methods byte-identical.

All 1,030 fresh timing samples, 103 storage-tracked samples, recorded proofs, and
fresh fitted values at every supported count fit the final installed weights.
The highest raw timing utilization is 86.55% (submitted precheck); the highest
recorded-proof utilization is 84.98% (distinct-identity election). These checks
bound the observed fixtures, not every possible state or hardware configuration.
See `comparison-final.json` in the new weights artifact directory below.

| Reservation / limit | Time weight | Proof allowance |
| --- | ---: | ---: |
| Election alone, 512 candidates | 91.109945090 ms | 8,739,268 B |
| Maximum mandatory subnet slot, including settlement and election | 469.011659975 ms | 10,868,091 B (10.365 MiB) |
| Network hook allowance | 1,000 ms | 16 MiB |
| Configured whole-block budget | 2,000 ms | 32 MiB |

The mandatory slot uses 46.90% of Network's time allowance and 64.78% of its proof
allowance. It is 0.354717946 ms and 9,451 bytes above the previously installed
snapshot version. Election's charged weight before the snapshot move was
16.524881222 ms / 1,313,809 bytes; the new maximum-map reservation is deliberately
larger. These are admission formulas with database charges and safety margins,
not measured wall-time deltas.

Before other hooks, the maximum mandatory slot leaves 1,530.988340025 ms and
22,686,341 proof bytes in the whole block. Normal dispatch separately caps its
class total at 1,500 ms / 24 MiB. Optional Network cleanup, history work, maintenance,
and swaps can consume further allowance: the mandatory-slot figure is not a
worst-case total for all Network work. A full correctly accounted Network allowance
would leave 1,000 ms / 16 MiB before other hooks and extrinsics.

### User extrinsic admission

New runtime regressions use real signed `Executive::apply_extrinsic` calls after
actual base/empty-hook initialization plus conservative synthetic reservations:
the maximum 512-node subnet slot, the full Network allowance, and a busy-block
case with Scheduler consuming its dynamic allowance until aggregate hook
reservations reach 90% of the whole block.
Each case successfully executes a balance transfer, a 512-node proposal, an
attestation, Revive account mapping, and a small Revive value transfer. Assertions
cover resulting state, fees/refunds, extension/base weight, encoded length charged
to proof size, the
Normal class limit, and the aggregate budget. These are admission tests after
modeled heavy hooks; the capacity benchmarks separately execute populated hooks.

The busy-Scheduler case leaves 199.738027 ms / 3,353,942 bytes after Timestamp and
127.685478 ms / 1,581,462 bytes after all five successful calls and their refunds.
A separate boundary test admits a large Revive declaration alongside the full
Network cap, and rejects it after aggregate hook reservations reach 90% or when either remaining
time or proof is exceeded by one unit. Its complete declaration is
781.036361800 ms / 13,102,740 bytes, including extensions and base cost.
Thus ordinary tested calls fit even in the busy case; arbitrary large calls are
not guaranteed inclusion in every block. Operational dispatch has a separate
reserve and is not covered by these Normal-dispatch assertions.

Proposal quarantine is also checked: `SubnetConsensusSubmissionMaxItems` retains
the full election cardinality when proposal membership shrinks, so settlement's
reservation still covers decoding all frozen weights.

### Unresolved history-pruning finding

`pallets/network/src/utilities/consensus_history.rs:86` admits a pruning iteration
with one prefix read, but line 115 subsequently charges `removed.loops` reads.
With 512 persisted backend iterations, the deficit is 511 RocksDb reads, or
**12.775 ms**. The fixed pruning read is included on both sides and does not cover
this gap. The pinned SDK's `WeightMeter::consume` accumulates weight and then
debug-asserts the limit; it does not clamp consumption. A sufficiently tight
remaining meter can therefore panic in a debug build or exceed its limit in a
release build. This is a source-level finding; a populated production-hook
reproduction was not run.

Retention pruning and removed-subnet history cleanup also charge database weight
with zero proof bytes, while persisted prefix traversal can contribute trie
proof. The capacity fixtures do not contain a retention backlog. Conservative
slack elsewhere may absorb some cases, but no maximum history-proof envelope has
been established. The snapshot timing move did not introduce these accounting
paths. Before certifying the entire hook, benchmark persisted maximum history
payloads/backlogs, reserve complete time and proof before mutation, and test
near-exhausted meters. This remains unresolved in this review.

### Fresh compiled-Wasm evidence

```sh
cargo build --release --locked --offline -p hypertensor-runtime --features runtime-benchmarks
bash .maintain/benchmark-election-snapshots.sh /tmp/election-snapshot-review-weights-20261114 weights
python3 -B .maintain/merge-election-snapshot-weights.py \
  /tmp/election-snapshot-review-weights-20261114/weights.rs \
  /tmp/election-snapshot-review-weights-20261114/merge-audit.json \
  /tmp/election-snapshot-review-weights-20261114/baseline-weights.rs \
  elect_validator,emission_step,precheck_subnet_consensus_submission
cargo build --release --locked --offline -p hypertensor-runtime --features runtime-benchmarks
```

The eight-profile weight run used 20 steps and 10 repeats with verification,
compiled Wasm, and `max` time/PoV analysis. Runtime SHA-256:
`ab9836f11b4ab51fba56d23f3334cea91980876d95eee8d25f3aa254f3b2e402`.
The directory archives the runtime blob, source, toolchain, commands, raw data,
models, comparisons, and repeat-merge checks.

| Profile | Maximum timing-only sample | Median timing at 512 | Maximum recorded proof |
| --- | ---: | ---: | ---: |
| Distinct-identity election | 53.403511 ms | 38.494936 ms | 7,427,029 B |
| Shared-identity election | 58.615711 ms | 29.329079 ms | 3,718,400 B |
| Proposal | 3.905517 ms | 3.198626 ms | 32,327 B |
| Accepted settlement | 57.016888 ms | 37.383073 ms | 552,862 B |
| Rejected settlement | 5.635988 ms | 4.499625 ms | 295,261 B |
| Missing-proposal settlement | 1.000073 ms | 0.741871 ms | 240,588 B |
| Submitted precheck | 0.698975 ms | 0.445799 ms | 68,757 B |
| Missing-proposal precheck | 0.635253 ms | 0.549737 ms | 236,239 B |

### Final-weight compiled-Wasm capacity

```sh
bash .maintain/benchmark-election-snapshots.sh /tmp/election-snapshot-review-capacity-20261114 capacity
```

All five profiles passed with verification enabled at 64, 128, 192, 256, 320,
384, 448, and 512 nodes, with 10 repeats per size. Both full-hook profiles verify
prior-round settlement, the next election, its complete weight map, and the next
pending settlement marker. The source-matched runtime includes the final weights;
SHA-256: `a2012c0507772ab77e6fdb00fcb97ea0603c408b8456fd2597881ec8a94683bb`.
No competing test or build workloads ran during this sweep.

| Profile | Maximum timing-only sample | Median timing at 512 | Maximum recorded proof |
| --- | ---: | ---: | ---: |
| Precheck | 0.675744 ms | 0.571117 ms | 62,251 B |
| Settlement | 45.935688 ms | 23.358547 ms | 209,617 B |
| Full healthy hook | 33.407109 ms | 30.675795 ms | 257,054 B |
| Reputation settlement | 20.142757 ms | 18.731347 ms | 85,736 B |
| Full reputation hook | 22.104019 ms | 20.518129 ms | 113,133 B |

All 400 timing-only samples, 40 storage-tracked samples, and fresh fitted models
at every tested count fit their final time and proof reservations. Independent runs can have different
timing spikes; the smaller whole-hook maximum does not imply that adding election
reduces settlement cost. These integrated fixtures use smaller ownership maps
than the standalone maximum-map election fixture and have no historical pruning
backlog. Their recorded proof sizes must not replace the much larger production
proof reservations. Raw results, source/runtime evidence, final reservations,
and comparisons are retained in `summary.json`, `results.json`, and `run.log`
under the capacity artifact directory.

### Final-weight native validation

```sh
cargo test --locked --offline -p pallet-network --features runtime-benchmarks --lib
SKIP_WASM_BUILD=1 cargo test --locked --offline -p hypertensor-runtime --lib \
  tests::production -- --nocapture
SKIP_WASM_BUILD=1 cargo test --locked --offline -p hypertensor-runtime --lib \
  runtime_integrity_tests -- --nocapture
cargo fmt --all -- --check
git diff --check
```

- Full pallet and benchmark fixture suite: **1,111 passed, 1 ignored**, including
  the assigned-slot ordering and quarantine/cardinality regression;
  `/tmp/election-snapshot-review-pallet-tests.log`.
- Runtime production suite: **14 passed**, including both new signed-admission
  and resource-boundary tests;
  `/tmp/runtime-network-production-final-weights.log`.
- Generated runtime integrity: **1 passed**;
  `/tmp/runtime-network-integrity-final-weights.log`.
- Final source-matched benchmark runtime build: passed;
  `/tmp/election-snapshot-review-final-build.log`.
- Workspace formatting check still reports preexisting differences in unrelated
  files/lines, including rewards, era, Overwatch, queues, and an unchanged portion
  of `pending_removals.rs` and `runtime/src/tests/production.rs`. Those were preserved;
  `/tmp/election-snapshot-review-format.log`.
- Whitespace check: `git diff --check` passed.

## Initial production envelopes

The weight run covers seven production methods: election, proposal, submitted
precheck, missing-proposal precheck, and accepted/rejected/missing settlement.
`elect_validator_multi_node` is an additional measurement profile; the production
`WeightInfo` interface is unchanged. Both implementations of each production
method were updated (14 method blocks), with unrelated methods preserved. Static
parity checks confirm 226 production trait methods and exactly two implementations
of each; the additional measurement profile is absent from that interface.

Election profiles fill each accessed ownership/allocation map to its independent
512-node bound. This is a conservative envelope: network-wide capacity prevents
every identity's maximum from coexisting. One profile uses distinct identities;
the other shares identities and enables both node-count decay and nonlinear stake
power. The installed election weight takes the componentwise maximum of both.

The merge also takes the componentwise maximum with the complete previous weight
body. Existing manual allowances, including appended DB charges, remain present.
Explicit allowance markers permit repeat merges without adding the allowance
twice. Two consecutive merges were checked on a disposable copy; both preserved
unrelated methods and generated parseable Rust.

At 512 nodes, the initially installed mandatory settlement-plus-election reservation was
**468,656,942,029 ps (468.657 ms)** and **10,858,640 proof bytes**. This fits the
unchanged **1 second / 16 MiB** hook budget. The budget check passes at every
64-node interval through 512.

## Initial commands and evidence

These are the executed commands; choose unused artifact directories for a rerun.
Each benchmark directory retains the Wasm blob/hash, source archive/patch,
toolchain and hardware details, exact CLI command, raw JSON, and generated models.

```sh
cargo build --release --locked --offline -p hypertensor-runtime --features runtime-benchmarks
bash .maintain/benchmark-election-snapshots.sh /tmp/election-snapshot-weights-20261114 weights
python3 -B .maintain/merge-election-snapshot-weights.py \
  /tmp/election-snapshot-weights-20261114/weights.rs \
  /tmp/election-snapshot-weights-20261114/merge-audit.json \
  /tmp/election-snapshot-weights-20261114/baseline-weights.rs
cargo build --release --locked --offline -p hypertensor-runtime --features runtime-benchmarks
bash .maintain/benchmark-election-snapshots.sh /tmp/election-snapshot-capacity-20261114 capacity
```

The production weight run uses 20 steps and 10 repeats. Its compressed Wasm SHA-256
is `7662def62e0d0607de9593a0fa218e67148f8a894ab65ee0699d65c8d5ae5823`.
The capacity run uses 64 through 512 nodes in eight steps, with 10 repeats, and a
fresh runtime containing the installed weights. Its compressed Wasm SHA-256 is
`8c1530ebce3198cd9033c7cd66ef1d79846ab93b0a07cacceb70bfb25875cdf6`.

## Initial compiled-Wasm capacity results

All five profiles completed with verification enabled: 80 timing samples per
profile at 64, 128, 192, 256, 320, 384, 448, and 512 nodes. Both full-hook profiles
verify prior settlement, next-round election, and creation of its complete weight
snapshot. Their generated research models are retained as evidence and are not
installed as production admission weights.

| Profile | Maximum measured time across sweep | Maximum recorded proof bytes |
| --- | ---: | ---: |
| `capacity_precheck` | 0.798382 ms | 62,251 |
| `capacity_settlement` | 39.223419 ms | 209,617 |
| `capacity_on_initialize` | 58.229378 ms | 257,054 |
| `capacity_settlement_reputation` | 27.692673 ms | 85,736 |
| `capacity_on_initialize_reputation` | 33.843987 ms | 113,133 |

At 512 nodes the full healthy hook peaks at 58.229378 ms and the reputation hook
at 32.199341 ms. The conservative mandatory reservation above covers these
measurements, while separately accounting for the maximum election ownership maps
and existing worst-case settlement envelopes. No node-count or hook-budget limits
were changed. Raw samples and verified-run output are in
`/tmp/election-snapshot-capacity-20261114/results.json` and `run.log`.

## Initial native validation

```sh
cargo test --locked --offline -p pallet-network --features runtime-benchmarks --lib
cargo test --locked --offline -p pallet-network --features runtime-benchmarks \
  tests::capacity::proposal_summary_weight_report -- --exact --nocapture
SKIP_WASM_BUILD=1 cargo test --locked --offline -p hypertensor-runtime --lib \
  runtime_integrity_tests -- --nocapture
```

- Full pallet and benchmark fixture suite: **1,111 passed, 1 ignored**;
  `/tmp/election-snapshot-final-benchmark-tests.log`.
- Installed-weight capacity arithmetic: **passed**;
  `/tmp/election-snapshot-weight-budget.log`.
- Actual runtime generated integrity test: **1 passed**;
  `/tmp/election-snapshot-runtime-integrity.log`. `SKIP_WASM_BUILD` applies only to
  this native integrity check; the capacity measurement uses the rebuilt Wasm.
- Final benchmark runtime build:
  `/tmp/election-snapshot-runtime-build-installed.log`.
