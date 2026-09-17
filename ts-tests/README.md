# Functional testing for Substrate Frontier Node RPC

This folder contains a set of functional tests designed to perform functional testing on the Frontier Eth RPC.

It is written in typescript, using Mocha/Chai as Test framework.

## Test flow

Tests are separated depending on their genesis requirements.
Each group starts a BABE development node with its own temporary database. Block helpers wait for actual authoring and GRANDPA finality. `waitForBlock` returns the requested block; use `waitForReceipt` for transaction inclusion, since a session-boundary block can contain no user transactions. Assert transaction data against the receipt’s block hash and compare moving head tags with native heads read around the request. Some older pool/load tests still need timing-aware assertions; the migration smoke test is `../evm-tests/npos-smoke.cjs`.

## Build the node for tests

```bash
cargo build --release
```

## Installation

```bash
npm install
```

## Run the tests

```bash
npm run build && npm run test
```

You can also add the Frontier Node logs to the output using the `FRONTIER_LOG` env variable. Ex:

```bash
FRONTIER_LOG="warn,rpc=trace" npm run test
```

The test node defaults to RPC port 19932 and P2P port 19931; override them with `FRONTIER_RPC_PORT` and `FRONTIER_P2P_PORT` when running independent suites concurrently. For a debug node build, allow extra time for first-time Wasm compilation with `FRONTIER_BUILD=debug FRONTIER_SPAWNING_TIME=300000`.
