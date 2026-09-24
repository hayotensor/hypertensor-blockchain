# Native chain validation

Use the checked-in lockfile. Native runtime tests can skip embedding Wasm:

```sh
SKIP_WASM_BUILD=1 cargo test --locked -p hypertensor-runtime
cargo test --locked -p pallet-network -p pallet-collective -p pallet-atomic-swap -p network-rpc-types -p network-custom-rpc
cargo build --locked -p hypertensor-runtime
cargo check --locked -p hypertensor-node
SKIP_WASM_BUILD=1 SKIP_PALLET_REVIVE_FIXTURES=1 cargo check --locked -p hypertensor-node --features runtime-benchmarks
SKIP_WASM_BUILD=1 cargo check --locked -p hypertensor-runtime --features try-runtime
cargo fmt --all -- --check
```

Runtime coverage includes native genesis identities, consensus key encoding,
nomination/election/validator replacement, reward payout and full unbonding,
account-bound session key ownership and refundable deposits, staking holds, key
rotation, compact historical ownership proofs, equivocation reporting and deferred
validator/nominator slashing, severity-aware validator disabling, bounded elections,
and proxy authorization. Revive tests also cover native sr25519 contract execution,
EVM and PolkaVM programs, Ethereum chain-ID/nonce validation, receipt generation,
refundable account mapping, execution/storage limits, rollback, and call filtering.

The benchmarking feature exposes Session and system transaction-extension benchmarks
alongside BABE, GRANDPA, Staking, Revive, and Network precompiles. It adds no pallets to the deployed runtime.
The command above checks benchmark integration without compiling upstream contract
fixtures or embedding Wasm. It does **not** run benchmarks. To build a functioning
benchmark runtime, unset both skip variables and install the `solc` and `resolc`
compilers and Rust fixture build prerequisites used by the matching SDK release's
`substrate/frame/revive/fixtures` builder. Benchmark and generate weights on the
intended validator hardware before launch.
All SDK dependencies must resolve to the single `polkadot-stable2606-2` source in
`Cargo.lock`; the workspace pins the release compiler in `rust-toolchain.toml`.

A node built with `SKIP_WASM_BUILD=1` cannot create the built-in chain specifications.
For a short consensus-transition check, build with Wasm enabled and `fast-runtime`,
then launch the two nodes in the validator guide,
and check that `chain_getHeader` and `chain_getFinalizedHead` advance on both nodes.
Use Litep2p for one node and `--network-backend libp2p` for the other to verify
interoperability. Check that both finalize the same block hash beyond the first
20-slot epoch, and that `BabeApi_current_epoch` at the finalized head advances.

Regenerate chain specifications from the current runtime before starting a fresh
network. Default builds now use production staking timings. Operator identities,
the Ethereum chain ID, economic calibration, and hardware-specific benchmarking
remain deployment work; see the [production review](production-review.md).

## Production review validation (2026-09-18)

- All 37 runtime tests and the production-configuration integration test passed.
  The separate `fast-runtime` configuration test also passed.
- The native node and Wasm runtime built successfully. `try-runtime` and the
  node's benchmark-feature check passed with the skip flags documented above.
- A fresh local node's runtime code hash matched the final compiled Wasm. Its
  BABE runtime API reported the default 2,400-slot production epoch length.
- Ethereum-signed deployment and invocation of an EVM contract succeeded. Checks
  confirmed bytecode, storage, caller, transferred value, and receipts. A native
  ed25519 account registered its mapping and called the same contract using its
  native key and balance. GRANDPA finalized the activity at block 4. The temporary
  node was stopped after testing.
- The live check used native RPC and Revive runtime APIs. The optional Ethereum
  JSON-RPC sidecar and hardware benchmarks were not run.
- Regression coverage includes proxy privilege escapes, pause recovery, Treasury
  fund retention and delayed payout, fee adjustment in both weight dimensions,
  Ethereum execution with fractional fee multipliers and maximum Network hook
  weight, and Scheduler headroom. PolkaVM execution/rollback remains covered by
  runtime tests.
- Formatting and whitespace checks passed. SDK dependencies resolve to the same
  pinned release; the finality backoff adds `sc-consensus-slots` from that release.

Revive is at pallet index 23. Network is at index 24, after BABE/Session epoch
rotation. Scheduler is at index 25 so its hook executes last; indices 11 and 16
are unused. The stateless Network precompile benchmark/configuration component
is at index 26 and has no hooks. Generate fresh chain specifications. See the
[production review](production-review.md) for fixed findings and unresolved release
requirements, and the [smart contract guide](smart-contracts.md) for account mapping
and optional Ethereum RPC setup.
