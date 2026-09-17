# Run Tests

## Install

```bash
npm i
```

## Seed

Some tests have time constraints conditions, set the `build` function with seeded storage parameters in `pallets/network/src/lib.rs`.

### Example

```rust
fn build(&self) {
    MinSubnetRegistrationEpochs::<T>::set(0);
    OverwatchEpochLengthMultiplier::<T>::set(1);
    DelegateStakeCooldownEpochs::<T>::set(0);
    NodeDelegateStakeCooldownEpochs::<T>::put(0);
    StakeCooldownEpochs::<T>::put(0);
    MinActiveNodeStakeEpochs::<T>::put(0);
    SubnetDelegateStakeRewardsUpdatePeriod::<T>::put(0);
    NodeRewardRateUpdatePeriod::<T>::put(0);
    MinSubnetDelegateStakeFactor::<T>::put(0);
    MinSubnetDelegateStakeBalance::<T>::put(0);
    SubnetPauseCooldownEpochs::<T>::put(0);

    // ...rest of code
}
```

## Build

```bash
cargo build --release
```

## Run the node locally

```bash
./target/release/hypertensor-node --dev
```

## Use the polkadot api via papi (run while chain is running)

```bash
npx papi add dev -w ws://127.0.0.1:9944
```

## Run locally with BABE and GRANDPA

```bash
./target/release/hypertensor-node --chain eth_dev --alice --validator --tmp --unsafe-force-node-key-generation
```

Blocks arrive approximately every six seconds. Helpers wait for actual blocks/finality.
For the consensus migration integration test, start the two-validator `local` network
from [the validator guide](../docs/validators.md), then run `node evm-tests/npos-smoke.cjs`.
This submits real Ethereum and staking transactions and chills a development validator;
use an isolated test chain. It waits for real era transitions and can take 15–25 minutes.

## Build smart contracts

```bash
npm run build
```

## Run tests

```bash
npm test
```

## To run a particular test case, you can pass an argument with the name or part of the name. For example:

Most tests must be performed independently because of time based logic on some functionality like unbonding

```bash
test -- -g "testing register subnet-0xzmghoq5702"
```

## Note

- Some tests require isolation due to subnet registration intervals.
- These test suites only verify the precompiles call the functions and they do and store the data that is expected. For logic tests see the pallets directory.
- The raw `overwatchCommits` and `overwatchReveals` views expose ephemeral protocol rows, not
  history: commits are consumed at epoch close, reveals at settlement, and both disappear when a
  participating node is structurally removed. The views revert when the requested row is absent.

## Todos

- Auto-chain restart for tests
