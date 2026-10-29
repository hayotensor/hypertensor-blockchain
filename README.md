# Hypertensor Blockchain

Hypertensor is a standalone NPoS chain built on the Polkadot SDK. BABE produces blocks,
GRANDPA finalizes them, and staking elections select the session validators. The chain
uses native 32-byte accounts, SS58 addresses, and SDK signed extrinsics.
Revive executes EVM and PolkaVM smart contracts using the same native TENSOR balances.
Existing users can sign contract calls with their sr25519 keys; Ethereum accounts
are optional.

## Build and run

```sh
cargo build --release --locked --features fast-runtime
./target/release/hypertensor-node --dev
```

The development preset uses Alice as validator and sudo account. Alice, Bob, Charlie,
Dave, Eve, Ferdie, and their `//stash` accounts are funded from their standard SDK
sr25519 development seeds. Local uses Alice and Bob as validators. TENSOR has 18
decimals and the SS58 prefix is 42. `fast-runtime` is only for disposable development
networks (two-minute sessions and six-minute eras). Build without that feature for
four-hour sessions and daily eras; choose the runtime before generating genesis.

HTTP and WebSocket RPC share port 9944. Native system, transaction-payment, and
[network queries](pallets/network/rpc/API.md) are available. Submit application calls
through runtime metadata using native SDK tooling.

See the [validator guide](docs/validators.md), [runtime configuration](docs/npos.md),
[smart contract guide](docs/smart-contracts.md), [validation guide](docs/npos-validation.md),
[production review](docs/production-review.md), and [build environment](env-setup/README.md).

## Chain specifications

Generate specifications from the current runtime:

```sh
./target/release/hypertensor-node build-spec --chain local --disable-default-bootnode > localSpec.json
```

Only disposable development/local presets are supplied. `--chain hoskinson` rejects
startup because the former live preset used publicly known validator and sudo keys.
Create a reviewed JSON specification with operator-controlled keys, allocations,
validator counts, and the production runtime, then run with `--chain <SPEC.json>`.
The Ethereum chain ID must also be assigned before launch. This project targets
fresh genesis state.
