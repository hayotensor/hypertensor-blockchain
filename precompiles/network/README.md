# Network precompiles

Six stateless Revive precompiles expose the network pallet through typed Solidity
interfaces. They are compiled into the runtime; no contract deployment is needed.
All 75 signed network extrinsics have one wrapper. The 86 collective-controlled
extrinsics are recorded as deferred in [coverage.json](coverage.json).

| Interface | Address | Responsibility |
| --- | --- | --- |
| `IStaking` | `0x0000000000000000000000000000000010010000` | Node/Overwatch staking, subnet/validator delegation, transfers, swaps, withdrawals |
| `IValidators` | `0x0000000000000000000000000000000010020000` | Validator registration, keys, identity, reward settings and allocation weights |
| `ISubnets` | `0x0000000000000000000000000000000010030000` | Subnet lifecycle, ownership, owner configuration and bootnodes |
| `ISubnetNodes` | `0x0000000000000000000000000000000010040000` | Subnet-node registration, keys, peers and metadata |
| `IConsensus` | `0x0000000000000000000000000000000010050000` | Attestation proposals and attestations |
| `IOverwatch` | `0x0000000000000000000000000000000010060000` | Overwatch registration, keys, peers and commit/reveal |

`0x0000000000000000000000000000000010070000` is reserved for governance. It is
**not registered** and has no callable governance interface. Sending a transaction
to an unregistered address does not imply that governance executed.

## Calling convention

Import the interfaces and `NetworkAddresses.sol` from `interfaces/`. The Solidity
files are the ABI source of truth: `build.rs` flattens their imports for Alloy, so
Rust and Solidity cannot drift into independently maintained signatures.

```solidity
import {IStaking} from "./interfaces/IStaking.sol";
import {NetworkAddresses} from "./interfaces/NetworkAddresses.sol";

// The calling contract must already hold native TENSOR.
IStaking(NetworkAddresses.STAKING).addValidatorDelegateStake(
    validatorId, amount, minSharesOut
);
```

- A mutation acts as the **immediate caller's** native account. A contract does not
  inherit its user's assets, coldkey, hotkey, owner or council permissions.
- Account arguments are `bytes32` native account IDs, not SS58 strings or padded
  Ethereum addresses. Use Revive's system precompile `toAccountId(address)` for
  address conversion; do not cast an address to bytes32. Native callers register
  their mapping through `Revive.map_account` as described in the chain guide.
- Unsigned integer widths and base units match the pallet: amounts and shares use
  `uint128`, identifiers use `uint32`, and one TENSOR is `10^18` units. Percentage
  fields retain the pallet's Q18 scale. Slippage minima and deadlines are passed
  through without alteration.
- All methods are nonpayable. Fund the calling contract before staking; sending
  `msg.value` to a precompile is rejected. There is no token approval mechanism.
- `Optional*` structs explicitly separate absence from zero/empty values. Bounded
  byte payloads are validated even when their presence flag is false.
- Maps use arrays of entries and reject duplicate keys; sets reject duplicate
  entries. Existing vector semantics, including the pallet's commit deduplication,
  are retained. Runtime collection/byte ceilings apply.
- `QueuedSwap.kind` is 0 for subnet delegation and 1 for validator delegation.
  Its account field is checked by the existing swap-queue ownership rules; it
  does not select the dispatch origin.
- Use canonical Solidity ABI encoding (`abi.encodeCall` or equivalent): overlapping
  dynamic tails, gaps, dirty padding and trailing bytes are rejected. Unknown
  selectors and malformed ABI revert with `Network: unknown selector` and
  `Network: malformed ABI`. Wrapper and pallet errors also use `Error(string)`
  prefixed `Network:`; out-of-resource failures remain execution failures.
  Mutations return no value on success.

## Bounded queries

Each domain owns its read interface. Optional records return `exists` and a
zero/default record when absent. Direct scalar stake/balance queries return zero
for an absent position. Retained stake remains readable after node/subnet removal.
Pool queries use the current generation's account shares, excluding stale shares
left after a complete pool loss.

Subnet and consensus records are compact contract views, not copies of the full
RPC response. In particular `ConsensusRound` reports election identity and proposal
presence without expanding every attestor's metadata. Enum codes follow:

- Subnet state: 0 registered, 1 active, 2 paused.
- Node class: 0 registered, 1 idle, 2 included, 3 validator.
- Round status: 0 elected, 1 proposed; election source: 0 regular, 1 emergency.

Unbondings are ordered by unlock block and limited by the runtime's hard ceiling.
Commitment queries use the current Overwatch epoch. Epoch timing exposes the
active snapshotted configuration. Bulk discovery stays in the existing network
RPC; no interface scans all accounts, subnets, or historical rounds.

## Execution and metering

The shared dispatcher constructs only typed network calls and uses ordinary
filtered dispatch. `TxPause`, network pause, key/owner checks, slippage, cooldowns,
slash locks, and commit/reveal rules remain authoritative in the network pallet.
There is no arbitrary runtime-call decoder and no Root or collective impersonation.
State changes under STATICCALL and all DELEGATECALL uses are rejected.

Network storage writes and native events roll back with the Revive call frame.
A failed subcall rolls back its own changes; reverting the enclosing contract also
rolls back successful network subcalls. Native pallet events remain authoritative;
these precompiles do not mirror them into Ethereum receipt logs. Index native events
when monitoring network operations.

Revive first transports calldata through a byte envelope. Entry/input overhead is
charged on its actual wire length before typed ABI decoding, including malformed
inputs. Strict decoding bounds allocations to the paid-for input size and rejects
aliased dynamic tails. Dispatch-info lookup is charged before its storage reads,
and pallet dispatch weight is reserved before dispatch. Actual-weight refunds are
bounded by that reservation. `Pays::No` does not waive contract gas. Each read
charges its own bounded storage/encoding envelope before access; scalar queries
do not pay for unrelated collection reads.
Network storage costs and economics remain those of the network pallet; these
adapters maintain no separate contract storage or storage deposits.

`weights.rs` defines conservative wrapper envelopes, with the network's existing
scan weights included for state-dependent dispatch-info lookup. FRAME benchmarks
exercise maximum-size ABI structures, cleanup selectors, and each read domain.
The envelopes are intentionally separate from the network pallet's dispatch weights.

## Verification and extension

```sh
SKIP_WASM_BUILD=1 cargo test --locked -p network-precompiles \
  -p hypertensor-runtime -p pallet-network
cargo build --locked -p hypertensor-runtime
SKIP_WASM_BUILD=1 SKIP_PALLET_REVIVE_FIXTURES=1 cargo test --locked \
  -p hypertensor-runtime --features runtime-benchmarks \
  network_precompile_benchmarks_with_production_bounds -- --nocapture
```

Our benchmark setup builds a minimal EVM fixture directly and does not need external
Solidity/PolkaVM compilers. The last command verifies and times the same benchmark
bodies with production runtime ceilings; debug test timings are not production
weight calibration. Normal node FRAME benchmarking lists these benchmarks under
`network_precompiles`. Full SDK Revive benchmarks still need the upstream fixtures
and their compiler prerequisites. Calibrate wrapper envelopes on validator hardware
alongside the existing network benchmarks before public launch.

For a new signed call, add it to exactly one domain interface and converter, update
`coverage.json` with its **canonical tuple-expanded Solidity signature**, and add
routing and behavioral coverage. Shared ABI structs belong in `NetworkTypes.sol`.
The coverage tests detect added/removed extrinsics, missing selectors, and accidental
governance exposure. Governance remains native until a separately designed council
proposal/voting interface is implemented.
