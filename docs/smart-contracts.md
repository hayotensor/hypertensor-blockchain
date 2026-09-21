# Smart contracts with Revive

The runtime includes `pallet-revive` from the same `polkadot-stable2606-2` release
as the rest of the SDK. It accepts EVM creation bytecode and PolkaVM (`.polkavm`)
programs. Contract execution uses native TENSOR balances, BABE block authors, and
the chain's transaction-payment and nonce system.

## Native accounts

Users keep their SS58 address and sr25519 key. They do not need an Ethereum key
or a separate funded account to call contracts through native extrinsics.

1. Submit `Revive.map_account` once using the native account. This holds a
   refundable address-mapping deposit from its native balance.
2. Use the `ReviveApi.address` runtime API to obtain the account's 20-byte contract
   address. Contract calls use `H160` destinations; SS58 remains the native account
   representation. Inside a contract, the mapped address is the caller.
3. Deploy with `Revive.instantiate_with_code`, passing EVM **creation** bytecode
   or a PolkaVM program, constructor data, value, weight and storage-deposit limits.
   The `Instantiated` event contains the deployed contract address.
4. Call with `Revive.call`, supplying the contract address, ABI-encoded data,
   value, and both limits. Values and deposit limits use native base units:
   one TENSOR is `10^18` units.

Use runtime metadata to construct and sign native calls. The signed extensions
include Revive's `SetOrigin`; clients must use the current metadata. Use the
`ReviveApi` dry-run methods to estimate execution and deposits before submitting.
`Revive.unmap_account` releases the mapping deposit; keep the mapping while using
that account's contract identity.

Uploads and instantiations are permissionless for signed users. Contract storage
and code consume deposits in addition to transaction fees. Execution failure
reverts contract state and value transfers but still consumes a nonce and fees
when the transaction is included. Native mapping is explicit (`AutoMap = false`).

## Ethereum transactions and wallets

The runtime also validates Ethereum-signed transactions submitted in
`Revive.eth_transact`. This route requires the Ethereum account's ECDSA key;
it is optional for native users. Ethereum accounts use the same balances pallet:
their native account ID is the 20-byte Ethereum address followed by twelve `0xee`
bytes. Fund that account ID through a native balance transfer when testing.

The configured Ethereum chain ID is **1337**, a development default. Native and
Ethereum values both have 18 decimal places (`NativeToEthRatio = 1`).

The node's port 9944 serves native RPC and runtime APIs. Ethereum JSON-RPC for
MetaMask, ethers, or Foundry is supplied by the SDK's separate `eth-rpc` service.
Build it from the **matching SDK release**:

```sh
cargo install --locked --git https://github.com/paritytech/polkadot-sdk \
  --tag polkadot-stable2606-2 pallet-revive-eth-rpc --bin eth-rpc
```

For a fresh local development network, run the node and optional service in
separate terminals:

```sh
./target/release/hypertensor-node --dev --state-pruning archive --blocks-pruning archive
eth-rpc --node-rpc-url ws://127.0.0.1:9944 --rpc-port 8545 \
  --eth-pruning archive --base-path ./eth-rpc-data
```

Keep the receipt database associated with its chain; use a new directory after
resetting development genesis state. Archive receipt indexing requires an archive
node. This service is not embedded in the Hypertensor node binary.

## Runtime policy and launch configuration

- `NonTransfer` proxies cannot dispatch Revive calls. An unrestricted `Any` proxy
  can act for its owner, including moving funds through contracts.
- The runtime call filter enforces `TxPause`. To stop contract execution, pause
  native `call`, `instantiate`, and `instantiate_with_code`, plus Ethereum
  `eth_call` and `eth_instantiate_with_code`, under the pallet name `Revive`.
  Ethereum transactions are decoded into those calls before dispatch; pausing
  only `eth_transact` does not cover them. Root can still bypass call filters.
- Weight fees account for both execution time and proof size. The block proof
  budget is 32 MiB, with half available to the network pallet's bounded hooks.
  The encoded block-length limit remains 5 MiB. Revive uses the SDK's published
  weights and the runtime's storage-deposit schedule. Ethereum transactions are
  capped at 60% of normal extrinsic capacity so one can fit alongside Network's
  maximum hook budget, with at least 10% of the block left for other work.
  Additional election/scheduled work can still defer inclusion.
- Standard EVM and Revive system precompiles are available. There are no custom
  Hypertensor precompiles exposing the network pallet.

Fees target 25% of normal block capacity and increase under sustained congestion.
The multiplier floor is 1, preserving Revive's nonzero gas-price requirement with
18-decimal TENSOR. At that floor, a full block's weight costs 0.002 TENSOR; encoded
transactions additionally cost 0.000001 TENSOR per byte. The existential deposit
is 0.001 TENSOR. Standard storage deposits are 0.1 TENSOR per item plus 0.00001
TENSOR per byte; Revive child-trie items cost 0.001 TENSOR each. Deposits are distinct
from transaction fees and may be refundable.

Always estimate Ethereum gas through the matching RPC service. Gas covers native
fees and storage deposits; old hardcoded gas limits may be insufficient with this
schedule. Native callers likewise need adequate storage-deposit limits.

Before a public launch, assign a unique Ethereum chain ID in
[`runtime/src/revive.rs`](../runtime/src/revive.rs), benchmark on the intended
validator hardware, and validate this baseline pricing against TENSOR economics.
See the [production review](production-review.md) for remaining release requirements.
Upstream audits do not replace review of this runtime configuration.

Runtime integration tests are in
[`runtime/src/revive_tests.rs`](../runtime/src/revive_tests.rs):

```sh
SKIP_WASM_BUILD=1 cargo test --locked -p hypertensor-runtime
```

They exercise signed native and Ethereum transactions, both execution engines,
mapping deposits, state/value rollback, and proxy and pause enforcement. The EVM
and PolkaVM fixtures are generated directly in the tests, without external contract
compilers. SDK benchmark builds additionally use upstream compiler-generated
fixtures; see the [validation guide](npos-validation.md).
