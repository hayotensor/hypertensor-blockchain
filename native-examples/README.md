# Native account examples for Talaris

Deploy the same Solidity `SimpleStorage` contract as `ethereum-examples/`, update
its value, and read it back using a native **sr25519 account**. This standalone
JavaScript project uses Polkadot-API to sign native transactions and connects
directly to a node's WebSocket RPC. An `eth-rpc` adapter is not needed.

## Setup

Use Node.js 22 or newer. From the repository root, in the same WSL environment
as your locally running nodes:

```sh
cd native-examples
npm ci
npm run wallet
```

The wallet command generates an sr25519 mnemonic, saves it in `.env`, and prints
the **native SS58 funding address**. It does not print the mnemonic or overwrite
an existing `.env`. The file is excluded from Git and created with owner-only
permissions.

Send TENSOR to that native address using `balances.transferKeepAlive` or your
existing transfer script. For this small local example, start with 10 TENSOR to
cover transaction fees, account mapping, and contract storage deposits. Genesis
changes and validator keys are not needed.

To use an existing funded sr25519 account instead, run `cp .env.example .env`
instead of the wallet command and set `NATIVE_SURI` to its mnemonic, secret seed,
or secret URI. For example, the setting for a mnemonic has this form:

```dotenv
NATIVE_SURI="your mnemonic words here"
```

Set the native RPC endpoint in `.env`:

```dotenv
NATIVE_RPC_URL=ws://127.0.0.1:9945
```

Use the RPC port of any reachable node on your chain; it does not need to be a
validator. Keep enough validators running for the chain to finalize blocks.
For a separate disposable `--dev` chain, `NATIVE_SURI=//Alice` uses its funded
development account. That public development key is only suitable for testing.

Check the account's address, balance, and mapping status:

```sh
npm run account
```

Free, reserved, and frozen balances are printed separately; free balance is not
necessarily all spendable. Fund the printed SS58 address. The displayed Revive
address identifies this native account inside contracts.

## Deploy, update, and verify

With the chain running:

```sh
npm run demo
```

This compiles `contracts/SimpleStorage.sol`, registers the native account's
Revive mapping if needed, deploys the contract with `value = 0`, submits
`setValue(42)`, then reads `value()` and asserts that it equals `42`. Transactions
are signed locally with the native account and the script waits for successful
finalization before proceeding.

The output includes native transaction hashes, the deployed contract address,
and:

```text
Value before: 0
Value after: 42
PASS: deployed, updated, and read back the expected value.
```

Each demo deploys a new contract. There are two transactions, plus a one-time
mapping transaction for an unregistered account. Contract addresses remain
20-byte `0x` addresses even when the deployer uses a native SS58 account. The
sample contract lets any account change its value.

## Interact with the same contract

Copy the printed contract address into `.env`:

```dotenv
CONTRACT_ADDRESS=0xYourDeployedContractAddress
```

Read without submitting a transaction:

```sh
npm run read
```

Update the value and automatically verify it:

```sh
NEW_VALUE=123 npm run set
npm run read
```

`npm run set` defaults to `100`. `NEW_VALUE=7 npm run demo` changes the demo's
update value. Values must be unsigned uint256 integers; the demo requires a
nonzero update so it changes the initial value.

For reading without loading a private key, use the **public SS58 address of an
already mapped native account** as the call origin:

```sh
READ_ORIGIN=5YourMappedNativeAddress NATIVE_SURI= npm run read
```

Alternatively, put `READ_ORIGIN` in `.env` and leave `NATIVE_SURI` empty. Reads
execute against finalized state without sending transactions or charging fees.
After resetting your chain, deploy again and update `CONTRACT_ADDRESS`.

## How it works

- `solc` compiles the Solidity source into EVM bytecode supported by this
  runtime's Revive configuration. The pinned compiler is installed by `npm ci`;
  `npm run compile` only compiles and does not connect to a node.
- Polkadot-API reads the node's runtime metadata and submits `Revive.map_account`,
  `Revive.instantiate_with_code`, and `Revive.call` as native extrinsics.
- `ReviveApi.instantiate` and `ReviveApi.call` dry runs supply execution weight
  and peak storage deposit estimates. The scripts add 20% headroom and pass
  explicit limits to the native transactions.
- `ethers` only encodes and decodes the Solidity ABI. It does not provide an
  Ethereum wallet, Ethereum provider, or Ethereum transaction transport here.

The scripts use Polkadot-API's dynamic `getUnsafeApi()` and the Revive interface
in this repository's runtime. No generated chain descriptors are required;
the scripts should be reviewed if that runtime interface changes. The commands
connect to an existing node and do not start one. Connection and finalization
waits are bounded to three minutes per command; a timeout does not cancel a
transaction already submitted to the chain.

References: [Polkadot-API client](https://papi.how/client/),
[Polkadot-API dynamic API](https://papi.how/unsafe/),
[Polkadot-API v3 transaction creation](https://papi.how/v3migration/),
[Revive source for this SDK version](https://github.com/paritytech/polkadot-sdk/blob/72284b37a234f8a1565f2043ddbf8280a69fcf29/substrate/frame/revive/src/lib.rs),
[Solidity compiler JavaScript API](https://github.com/ethereum/solc-js#example-usage-with-standard-json-input).
