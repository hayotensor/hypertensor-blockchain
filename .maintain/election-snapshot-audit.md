# Election-time attestor snapshot measurements

Measured locally on 2026-11-14 (America/New_York), using Rust 1.93.0 and benchmark
CLI 60.0.0. All runs use disposable runtime state, compiled Wasm, proof recording,
and `max` analysis for both execution time and PoV. No live network is involved.

## Production envelopes

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

At 512 nodes, the installed mandatory settlement-plus-election reservation is
**468,656,942,029 ps (468.657 ms)** and **10,858,640 proof bytes**. This fits the
unchanged **1 second / 16 MiB** hook budget. The budget check passes at every
64-node interval through 512.

## Commands and evidence

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

## Compiled-Wasm capacity results

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

## Native validation

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
