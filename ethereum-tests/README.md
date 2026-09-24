# Ethereum contract integration tests

Run real Ethereum transactions against a disposable Talaris node and the matching
Revive `eth-rpc` adapter. The suite uses ethers v6, Node's test runner, solc, and
OpenZeppelin contracts. It compiles Solidity, deploys bytecode, signs transactions,
waits for finalized receipts, and queries contract state over Ethereum JSON-RPC.

No Network pallet operations or precompiles are tested. Existing validator nodes,
chain specs and accounts are not used. Random test wallets are funded in a generated
development genesis; private keys stay in memory. Ports are allocated dynamically.

## Run

Requires Node.js 22+, npm, and the Rust toolchain/dependencies used by the repository.
From the repository root, build the actual node with its embedded Wasm:

```sh
env -u SKIP_WASM_BUILD cargo build --release --locked -p hypertensor-node
cd ethereum-tests
npm ci
```

Install the adapter from the SDK release pinned by this repository:

```sh
SKIP_WASM_BUILD=1 cargo install --locked \
  --git https://github.com/paritytech/polkadot-sdk \
  --tag polkadot-stable2606-2 \
  --root "$PWD/.tools" \
  pallet-revive-eth-rpc --bin eth-rpc

ETH_RPC="$PWD/.tools/bin/eth-rpc" npm test
```

`SKIP_WASM_BUILD=1` applies only to the adapter installation, whose build uses
native development-runtime metadata. The Talaris node must include its Wasm,
which is why its build command explicitly unsets that variable.

If `eth-rpc` is already on `PATH`, simply run `npm test`. `TALARIS_NODE` can select
another built node binary; its default is `../target/release/hypertensor-node`.
`ETH_RPC` selects the adapter executable. These select binaries, not existing
network endpoints. `fast-runtime` is optional; the cases do not wait for an era.

`npm run compile` only compiles the fixtures. The suite also compiles them before
starting the chain. Dependencies and compiler versions are pinned in the lockfile.

The harness stops its child processes and removes its temporary chain/adapter
databases on completion or handled interruption. Process logs and runtime/genesis
identifiers are retained under `logs/run-*`; compiler artifacts are in `artifacts/`.
The local `.gitignore` excludes generated files and installed tools. A failed
precondition or missing binary fails the suite; there are no skipped test cases.

## Included assertions

| Case | Assertions beyond basic deploy/call coverage |
| --- | --- |
| ERC-20 delegated transfer | Third-party sender, allowance decrement, supply conservation, decoded events, balance and allowance at the earlier block |
| ERC-20 insufficient balance | An included revert restores the allowance as well as balances, removes logs, and consumes the transaction nonce |
| ERC-20 revoked approval | A previously successful gas estimate does not authorize transfer after approval is revoked |
| ERC-2612 relayed permit | EIP-712 signature sets the named spender's allowance; the relayer pays/submits; replay fails; approved transfer succeeds |
| ERC-2612 invalid permit | Wrong chain ID, wrong verifying contract, and expired deadline fail without consuming the permit nonce; a valid control still works |
| ERC-721 receiver callback | Approved operator can transfer; receiver observes correct operator/from/token/data; ownership and approval update |
| ERC-721 receiver rejection | Ownership, approval and balances roll back; no Transfer log survives; a subsequent valid transfer works |

These are compatibility regressions for running standard Ethereum applications,
not a new implementation or audit of the token standards.

## Coverage comparison

The structure follows Frontier's approach of launching an actual node and using an
Ethereum client, but does not copy its manual-seal RPCs or assume Frontier's gas
schedule. Reference:
[Frontier ts-tests at 24fcbdb](https://github.com/polkadot-evm/frontier/tree/24fcbdb175d15c4727c9e139a5e84a71270afeba/ts-tests).

Reviewed alongside the repository's `runtime/src/revive_tests.rs`, the
[pinned Revive RPC tests](https://github.com/paritytech/polkadot-sdk/blob/72284b37a234f8a1565f2043ddbf8280a69fcf29/substrate/frame/revive/rpc/src/tests.rs),
Revive's runtime tests, and
[Parity's EVM test suite at b98df5a](https://github.com/paritytech/evm-test-suite/tree/b98df5aa538f70f9e272442f71d3882943d002b0).

Basic deployment, native currency transfer, getters, gas estimation, generic
reverts/logs, state overrides, transaction types and VM instructions already have
upstream coverage and are not added as independent tests. The reviewed tests did
not exercise the token-standard workflows above. Their constituent VM operations
are of course already tested; this suite verifies complete externally signed
workflows against this chain's Wasm/runtime and adapter.

Frontier's pending-pool/replacement tests are also omitted: the pinned adapter
does not expose the same pending-transaction semantics. This suite makes no claim
of complete Ethereum JSON-RPC compatibility or multi-validator/fork coverage.
