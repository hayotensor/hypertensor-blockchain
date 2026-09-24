# Network precompile interfaces

Import these Solidity interfaces from your own contracts to call the network
pallet's precompiles. They are already part of the runtime at the addresses below;
there is nothing to deploy for the interfaces themselves.

| Interface | Address | Operations |
| --- | --- | --- |
| `IStaking` | `0x0000000000000000000000000000000010010000` | Stake, delegation, swaps, withdrawals and stake queries |
| `IValidators` | `0x0000000000000000000000000000000010020000` | Network validator registration, keys, identity and reward settings |
| `ISubnets` | `0x0000000000000000000000000000000010030000` | Subnet registration, ownership, configuration and queries |
| `ISubnetNodes` | `0x0000000000000000000000000000000010040000` | Subnet-node registration, keys, peers, metadata and queries |
| `IConsensus` | `0x0000000000000000000000000000000010050000` | Network attestations and subnet consensus queries |
| `IOverwatch` | `0x0000000000000000000000000000000010060000` | Overwatch registration, staking information and commit/reveal |

`NetworkTypes.sol` defines shared structs. `NetworkAddresses.sol` provides address
constants. Its `GOVERNANCE` address is reserved and has **no registered precompile**.

For a Solidity file in `ethereum-examples/contracts/`, import:

```solidity
import {IStaking} from "./precompiles/IStaking.sol";
import {NetworkAddresses} from "./precompiles/NetworkAddresses.sol";
```

Then, inside your contract, call the interface directly:

```solidity
IStaking(NetworkAddresses.STAKING).addSubnetDelegateStake(
    subnetId, amount, minSharesOut
);
```

- The precompile acts as the **calling contract's native account**. Fund that
  contract before staking; its caller's balance and permissions are not inherited.
- Methods are nonpayable. Do not attach `msg.value` to the precompile call.
- Account arguments are native `bytes32` account IDs. Use Revive's system
  precompile `toAccountId(address)` when converting an Ethereum address; casting
  it to `bytes32` does not produce the correct native account ID.
- Amounts and shares use `uint128`; one TENSOR is `10^18` base units. Percentage
  fields use the pallet's Q18 scale. IDs use `uint32`.
- `Optional*` structs use `present` to distinguish absence from a zero or empty
  value. Optional query records return an `exists` flag.
- Successful mutations return no value. Network failures revert with
  `Error(string)`; native network events are not mirrored into Ethereum logs.

The `.sol` files are copies of the runtime's
[canonical interfaces](../../../precompiles/network/interfaces/). Keep them in
sync when changing that ABI. See the
[precompile documentation](../../../precompiles/network/README.md) for complete
calling conventions and query semantics.

Run `npm run compile` from `ethereum-examples/` to compile these interfaces with
the existing Hardhat project. You can also copy this entire directory into
another Solidity project; its Solidity imports are all local.
