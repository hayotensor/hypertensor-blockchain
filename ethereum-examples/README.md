# Hardhat examples for Talaris

Deploy a Solidity contract through your running `eth-rpc` adapter, change its
stored value, and read it back. This is a standalone Hardhat 3 + ethers project.

For calling network pallet precompiles from your own Solidity contracts, see
[contracts/precompiles/](contracts/precompiles/README.md). It contains the six
interfaces, shared structs, precompile addresses, and import instructions.

## Setup

Use Node.js 22.13+ (an even-numbered supported release). Run these commands from
the repository root, in the same WSL environment as your locally running adapter:

```sh
cd ethereum-examples
npm ci
npm run wallet
```

The wallet command creates a new Ethereum key in `.env` and prints its Ethereum
address and **native funding address**. It does not print the key and will not
overwrite an existing `.env`. The file is excluded from Git.

Send TENSOR from one of your funded native accounts to the printed **native funding
address**, using `balances.transferKeepAlive` or your existing transfer script.
For a local test, 10 TENSOR is a reasonable starting balance for this small example.
This funds the Ethereum deployer; no genesis changes or validator keys are needed.

If you already have a funded Ethereum account, use `cp .env.example .env` instead
of `npm run wallet`, then set `PRIVATE_KEY` to that account's `0x`-prefixed ECDSA
private key. An sr25519 validator/staking key cannot sign Ethereum transactions.

The defaults in `.env` are:

```dotenv
ETH_RPC_URL=http://127.0.0.1:8545
ETH_CHAIN_ID=1337
```

`8545` is the adapter's Ethereum RPC port. The adapter itself connects to your
validator's native WebSocket RPC, for example `ws://127.0.0.1:9945`.
Update the URL or chain ID if your adapter/runtime uses different values.

Check the deployer's addresses and spendable balance:

```sh
npm run account
```

The native funding address is this runtime's Ethereum address mapping:
the 20-byte Ethereum address followed by twelve `0xee` bytes, encoded as SS58.
The Ethereum balance excludes the native existential deposit.

## Deploy, update, and verify

Keep your chain and adapter running, then run:

```sh
npm run demo
```

This compiles `contracts/SimpleStorage.sol`, deploys it with `value = 0`, submits
`setValue(42)`, then calls `value()` and asserts that the returned value is `42`.
It waits for successful transaction receipts before proceeding. A failed
transaction or unexpected value makes the command exit with an error.

The output includes transaction hashes, the deployed contract address, and:

```text
Value before: 0
Value after: 42
PASS: deployed, updated, and read back the expected value.
```

Each `npm run demo` deploys a new contract and sends two transactions. Allow time
for your chain to produce blocks and for the adapter to index the receipts.
Gas is estimated through the adapter; no Ethereum gas limits are hardcoded.
The sample contract deliberately lets any account update its value.

## Interact with the same contract

Copy the printed contract address into `.env`:

```dotenv
CONTRACT_ADDRESS=0xYourDeployedContractAddress
```

Read without sending a transaction:

```sh
npm run read
```

Update to another value, then automatically read it back and verify:

```sh
NEW_VALUE=123 npm run set
npm run read
```

`npm run set` defaults to `100` if `NEW_VALUE` is omitted. `NEW_VALUE=7 npm run demo`
changes the demo's update value. The values are unsigned uint256 integers.
After resetting your chain, deploy again and update `CONTRACT_ADDRESS`.

`npm run compile` only compiles; `npm run check` also checks TypeScript.
The pinned solc package supplies the compiler locally after `npm ci`.
The example commands always select the `talaris` HTTP network in
`hardhat.config.ts`; they do not start a node or the RPC adapter.

References: [Hardhat deployment scripts](https://hardhat.org/docs/guides/deployment/using-scripts),
[ethers plugin](https://hardhat.org/docs/plugins/hardhat-ethers),
[Hardhat configuration](https://hardhat.org/docs/reference/configuration).
