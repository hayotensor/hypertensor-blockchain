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
This project targets fresh genesis state.

### Launch profile

The default build is the development profile (20-block epoch, EVM chain ID 1337). Public
networks must be built with `--features production`, which fails to compile until the
owner sets `PRODUCTION_BLOCKS_PER_EPOCH` and `PRODUCTION_CHAIN_ID` in `runtime/src/lib.rs`.
Both are fixed for the life of a chain. Build the node with the same feature so that its
embedded runtime and the specs it emits use the production profile:

```sh
cargo build --release --locked -p hypertensor-node --features production
```

### Deployment decisions

- A `production` runtime rejects genesis that gives sudo, session keys, staking slots,
  balances or council seats to public development keys (`//Alice`..`//Ferdie` and their `//stash`), so replace all of them
  in the `--chain local` template.
- The ETH replay domain is only the compile-time `ChainId`. Every production network
  needs its own `PRODUCTION_CHAIN_ID`. Unprotected (pre-EIP-155) legacy transactions are
  rejected, so Nick's-method deployments are not supported.
- `SlashDeferDuration` is `BondingDuration - 2` (26 eras), so a slash lands before an
  offender who unbonded in the offence era can withdraw. A report filed 27 or 28 eras late
  is still accepted (BABE/GRANDPA only bound report age by historical sessions, which
  Staking keeps for `BondingDuration`), but its slash is never applied.

### Attestation block-space

`Network.propose_attestation` and `Network.attest` are Normal-class calls with no extra
transaction priority. They are refunded (`Pays::No`) only on success. Filling every Normal
block costs about 23 TENSOR a day at launch fees, so sustained filler traffic could delay
deadline-bound attestations. The runtime does not boost them: an unconditional priority
boost would also apply to invalid calls, which would let anyone buy priority cheaply. A
safe boost needs a transaction extension that checks the signer is the elected proposer or
an eligible attestor before it raises priority. Until then, validators should add a tip to
these calls when blocks are congested, and operators should watch block fullness and
`TargetedFeeAdjustment`.
