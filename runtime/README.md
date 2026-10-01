# Runtime source layout

- `src/lib.rs`: runtime types, shared constants, version, and pallet composition.
- `src/configs/`: pallet configuration and supporting policies (`common`, consensus/staking in `npos`, and contracts in `revive`).
- `src/apis/`: runtime API implementations and benchmark registration.
- `src/genesis/`: built-in chain genesis presets.
- `src/tokenomics/`: supply, fees, emissions, and genesis validation.
- `src/tests/`: runtime unit tests for consensus, production settings, monetary policy, proxy filters, and contracts; contract tests include Network precompile tests and ABI argument fixtures.
- `tests/`: integration tests that compile the runtime without `cfg(test)` to verify production configuration.

Run native runtime tests with:

```sh
SKIP_WASM_BUILD=1 cargo test -p hypertensor-runtime --lib --tests
```
