# Hypertensor Blockchain

Hypertensor is a blockchain for AI application subnets. Its Network pallet manages
subnet and node registration, staking and delegation, attestation consensus, and
reward distribution. Subnet applications run their workloads and compute scores
off-chain, then submit proposals and attestations for settlement on-chain.

The chain is built on the Polkadot SDK and uses nominated proof of stake (NPoS).
Staking elections select the chain validators, BABE produces blocks, and GRANDPA
finalizes them. Chain validators and subnet validators have separate roles.

The blockchain supports EVM smart contracts and RISC-V smart contracts executed through
[PolkaVM](https://github.com/paritytech/polkavm).
Contracts share the chain's native balances. Native accounts use 32-byte IDs and
SS58 addresses with prefix 42; users can sign contract calls with sr25519 keys.
Ethereum accounts are optional.

## Build and run

Install Rust through `rustup` and the native compiler, Clang/libclang, OpenSSL,
pkg-config, and Protobuf dependencies in the [build environment guide](docs/rust-setup.md).
The repository pins Rust 1.93.0 and the `wasm32v1-none` target in
[rust-toolchain.toml](rust-toolchain.toml); rustup selects the toolchain automatically.

From the repository root:

```sh
rustup show
cargo build --release --locked --features fast-runtime
./target/release/hypertensor-node --dev --base-path /tmp/hypertensor-network-dev
```

The `--dev` preset starts a single validator using Alice, who is also the sudo
account. Alice, Bob, Charlie, Dave, Eve, Ferdie, and their `//stash` accounts are
funded from public SDK development seeds. Use this preset for disposable local
development.

HTTP and WebSocket RPC share port 9944. With the node running, query its latest
block from another terminal using `curl`:

```sh
curl --fail --silent --show-error http://127.0.0.1:9944 \
  -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"chain_getHeader","params":[]}'
```

Repeat after several seconds. An increasing hexadecimal `result.number` confirms
block production; the target block time is six seconds. Native system,
transaction-payment, and [network queries](pallets/network/rpc/API.md) are available.
Submit application calls through runtime metadata using native SDK tooling.

## Build profiles

| Build | Network epoch | EVM chain ID | Validator session / staking era |
| --- | --- | --- | --- |
| Default (no profile feature) | 20 blocks | 1337 | Four hours / one day |
| `--features fast-runtime` | 20 blocks | 1337 | Two minutes / six minutes |
| `--features production` | Set before launch | Set before launch | Four hours / one day |

Times assume six-second blocks/slots. Network epochs govern subnet consensus;
validator sessions and staking eras govern block production and NPoS staking.
Subnets have assigned slot offsets; use the
[epoch-status RPC](pallets/network/rpc/API.md) to determine their consensus windows.

The default build uses development network settings despite its longer staking
timings. `fast-runtime` is for disposable networks and cannot be combined with
`production`. Choose the profile before generating genesis; use a fresh chain
specification and database when switching profiles.

## Chain specifications

The bundled presets are `dev` (Alice) and `local` (Alice and Bob), both intended
for disposable networks. See the [validator guide](docs/validators.md) to run the
two-validator local network.

Generate a local specification from the development runtime built above:

```sh
./target/release/hypertensor-node build-spec --chain local \
  --disable-default-bootnode > localSpec.json
```

### Launch profile

For a public network, set `PRODUCTION_BLOCKS_PER_EPOCH` and `PRODUCTION_CHAIN_ID`
in [runtime/src/lib.rs](runtime/src/lib.rs), then build the node with the production
runtime embedded:

```sh
cargo build --release --locked -p hypertensor-node --features production
```

The production build fails until both values are configured. They are fixed for
the life of the chain. Create a reviewed JSON specification with the production
runtime, operator-controlled keys, allocations, and validator counts, then run
with `--chain <SPEC.json>`. This project targets fresh genesis state.

### Deployment decisions

Review the [production operations guide](docs/production-review.md) for genesis
key requirements, Ethereum replay protection, slashing timing, scheduler limits,
and attestation inclusion under congestion.

## Documentation

| Task | Guide |
| --- | --- |
| Prepare the build environment | [Rust and native dependencies](docs/rust-setup.md) |
| Operate chain validators or nominate | [Validators and nominators](docs/validators.md) |
| Build and operate a subnet | [Network pallet guide](AGENTS.md) |
| Query subnets, nodes, and consensus | [Network RPC API](pallets/network/rpc/API.md) |
| Call contracts with native accounts | [Native contract examples](native-examples/README.md) |
| Call contracts with Ethereum accounts | [Hardhat examples](ethereum-examples/README.md) (requires an Ethereum RPC adapter) |
| Access the Network pallet from contracts | [Network precompiles](precompiles/network/README.md) |
| Inspect runtime configuration and run tests | [Runtime source and tests](runtime/README.md) |
| Prepare and operate a public network | [Production operations](docs/production-review.md) |
