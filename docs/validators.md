# Validators and nominators

The chain uses NPoS staking/election → Session validator set → BABE block production → GRANDPA finality. Previously, genesis PoA authorities drove Aura production and GRANDPA finality. Frontier still executes and indexes Ethereum transactions above the runtime execution layer.

User accounts remain Ethereum H160/AccountId20 accounts signed by secp256k1 keys. BABE sr25519 and GRANDPA ed25519 keys belong to the node keystore and are **not** wallet addresses. Never truncate either session public key into a validator account.

## Build and start a local network

```sh
cargo build --release --locked
./target/release/hypertensor-node key generate-node-key --chain local --base-path /tmp/npos-alice
./target/release/hypertensor-node key generate-node-key --chain local --base-path /tmp/npos-bob
./target/release/hypertensor-node --chain local --alice --validator --base-path /tmp/npos-alice --port 30333 --rpc-port 9944 --no-prometheus
./target/release/hypertensor-node --chain local --bob --validator --base-path /tmp/npos-bob --port 30334 --rpc-port 9945 --no-prometheus
```

Local discovery connects these nodes on the same machine; alternatively use the first node's printed `/ip4/…/tcp/30333/p2p/…` address as the second node's `--bootnodes` argument. Use separate base paths and RPC ports. P2P is TCP 30333 by default; RPC HTTP/WebSocket share TCP 9944; optional Prometheus uses TCP 9615 (choose a different `--prometheus-port` on the second node or use `--no-prometheus`). RPC/key-management examples below assume loopback access.

The local preset bonds two validators at genesis. `--dev` and `--chain eth_dev --alice --validator` have one validator; they can start independently. Local uses Alice and Bob's SDK session seeds mapped explicitly to the **existing** Ethereum development wallets:

| Session seed | Stash/wallet H160 | Existing wallet name |
| --- | --- | --- |
| `//Alice` | `0xf24ff3a9cf04c71dbc94d0b566f7a27b94566cac` | Alith |
| `//Bob` | `0x3cd0a705a2dc65e5b1e1205896baa2be8a07c6e0` | Baltathar |
| `//Charlie` | `0x798d4ba9baf0064ec19eb4f0a1a45785ae9d6dfc` | Charleth |
| `//Dave` | `0x773539d4ac0e786233d90a233654ccee26a613d9` | Dorothy |

This mapping does not derive a new wallet from `//Alice`: use the corresponding existing Ethereum development private keys from the test fixtures. The regenerated Hoskinson preset uses four public development identities pending operator configuration. Replace those tuples with funded operator H160 accounts and their session public keys before distributing a network spec; chain ID remains 42.

## Operator keys and staking

Generate separate consensus keys with this node binary:

```sh
./target/release/hypertensor-node key generate --scheme Sr25519
./target/release/hypertensor-node key generate --scheme Ed25519
```

A fresh base path also needs a P2P identity: `key generate-node-key --chain <spec> --base-path /path/to/validator`. This is separate from session keys; the command saves the secret to that node's network directory without overwriting an existing key.

Save their secret phrases in the validator's keystore by running the insert commands against its actual chain and base path:

```sh
./target/release/hypertensor-node key insert --chain local --base-path /path/to/validator --key-type babe --scheme Sr25519 --suri '<BABE secret phrase>'
./target/release/hypertensor-node key insert --chain local --base-path /path/to/validator --key-type gran --scheme Ed25519 --suri '<GRANDPA secret phrase>'
```

Alternatively, local `author_rotateKeys` generates/inserts both keys and returns the SCALE SessionKeys value; `author_hasSessionKeys` checks it. `author_insertKey` takes `["babe", secretUri, publicHex]` or `["gran", secretUri, publicHex]`. The key bundle is BABE then GRANDPA (32 bytes each); `SessionKeys` runtime API decodes it. No Aura `aura` keystore entry is used.

Using an AccountId20-aware Substrate client with **Ethereum signing**, submit these extrinsics from the funded H160 stash account:

1. `staking.bond(1000 * 10^18, Staked)` to lock the minimum validator bond. This SDK's bond call has no separate controller argument; the stash signs subsequent calls.
2. `session.setKeys({babe: babePublicHex, grandpa: grandpaPublicHex}, 0x)` with the keys installed on the node.
3. `staking.validate({commission: 0, blocked: false})` to declare validator intent.
4. Start `hypertensor-node --chain <spec> --base-path <same path> --validator` and connect it to peers.

Declaring intent does not guarantee selection; nominations and the bounded Sequential Phragmén election select the next set. There are no invulnerable genesis validators. Keep enough liquid balance for fees above the bonded amount and the unchanged 500-base-unit existential deposit. Existing EVM transfers cannot spend the balance locked by consensus staking.

Register both keys before declaring validator intent. The pinned SDK's `staking.validate` does not enforce key registration, and Session excludes elected candidates without keys; this can leave fewer active validators than the staking election selected. When rotating keys, keep the old keys in the keystore until the replacement bundle is active in both BABE and GRANDPA. To retire, chill first and wait until the account is absent from both `session.validators` and `session.queuedKeys` before purging its session keys or stopping the node.

Nominators use the same H160 signing model: `staking.bond(amount, Staked)` (minimum 20 TENSOR), then `staking.nominate([validatorH160, ...])`, with up to 16 targets. `bondExtra`, `unbond`, `withdrawUnbonded`, `chill`, and `setPayee` are the SDK staking calls. `chill` removes future intent, not the current/queued session immediately. After unbonding matures, call `withdrawUnbonded` with the stash's recorded slashing-span count (zero if none). Existing Network staking precompiles continue to manage application staking; they do not replace these consensus staking calls.

## Timing, bounds and economics

All defaults are grouped in `runtime/src/npos.rs`. They are development settings, not a final production economic schedule.

| Setting | Default |
| --- | --- |
| BABE slot / expected interval | 6 seconds, primary probability parameter `c = 1/4`, plain secondary slots |
| Session / BABE epoch | 20 slots, approximately 2 minutes |
| Era | 3 sessions, approximately 6 minutes |
| Bonding / slash deferral | 28 eras (about 2h48m) / 7 eras (about 42m) |
| Reward history | 84 eras (about 8h24m) |
| Candidates / nominators / nominations | 32 / 256 / 16 per nominator |
| Desired validators | Genesis authority count: dev 1, local 2, Hoskinson 4 |
| Minimum validators | 1 for development availability |
| Exposure page / unlocking chunks | 64 nominators / 32 chunks |

Queued sessions delay membership and key changes; observe `session.currentIndex`, `session.validators`, `staking.currentEra`, `staking.activeEra` and staking events. BABE epoch duration must be fixed before genesis and cannot be changed by a normal runtime upgrade. Set longer bonding/reporting windows and an appropriate validator minimum before a production genesis.

The 20-TENSOR minimum nominator bond stays above the largest possible SDK balance-to-vote divisor, so it has nonzero voting weight even in the preserved high-issuance `eth_dev` fixture. Use `local` for representative staking economics; `eth_dev` deliberately retains its extreme EVM balances.

Elections run on-chain at genesis and when staking plans a new era. They use all registered voters within the explicit caps, including validator self-votes. No offchain election miner, election pallet, bags-list or nomination pool is needed. Root must not raise registration caps beyond the configured election bounds without adjusting and benchmarking the runtime. The runtime also checks actual registration counts before each election: above 32 candidates or 256 nominators, it rejects the election instead of letting the SDK silently truncate the unsorted voter map. A failed election retains the previous set and stalls new eras; monitor `StakingElectionFailed`. Restoring a configuration limit alone does not remove excess registrations: reduce actual registrations within the bounds, then restore the limits. Elections resume automatically.

Consensus rewards use an isolated fixed budget of 1,000 TENSOR/year prorated by actual era duration, independent of the existing Network and AuthorSubsidy emissions. Authorship earns staking reward points; the SDK divides each era payout among validators by points, deducts each validator's commission, and shares the remainder with exposed nominators by stake. Anyone can submit `staking.payoutStakers(validator, era)` / `payoutStakersByPage`; paged rewards must be claimed within history depth. Rewards are inflationary and minted when claimed. The era payout returns zero reward remainder; unclaimed/rounding residue is not redirected to treasury. Slashes burn stake after the deferral period, except the configured SDK reporter reward (10% of the applicable reward budget) paid from slashed funds. Existing author-subsidy payout configuration is retained, using the BABE key: see [the payout guide](../pallets/author-subsidy/SET_REWARD_ADDRESS.md).

BABE and GRANDPA both submit equivocation reports with historical session ownership proofs through Offences to Staking. Reports cover the bonding window (1,680 nominal slots/blocks); GRANDPA retains 84 set/session mappings. This minimal configuration has no im-online heartbeat pallet and does not introduce downtime slashing. The SDK's `UpToLimitDisablingStrategy` handles disabling reported offenders.

SDK-generated staking, session and election weights are used, and author-subsidy weights were regenerated against the BABE lookup. BABE/GRANDPA supply only nonzero `()` default weights at this revision; benchmark consensus reports on target hardware before release. Also measure maximum-cap election/era-boundary work alongside Network hooks before increasing caps.

## Compatibility and validation

See [the migration inventory](npos-migration.md) for exact dependency revisions, preserved pallet indices and the old-chain activation requirements. Use a fresh database and regenerated chain spec for this unlaunched chain. A deployed Aura database requires a separately designed coordinated client/runtime consensus transition; this change does not implement that transition.

```sh
SKIP_WASM_BUILD=1 cargo test --locked -p hypertensor-runtime -p pallet-author-subsidy
# With the two local nodes above running and evm-tests dependencies installed:
node evm-tests/npos-smoke.cjs
```

The smoke test sends real Ethereum-signed transactions, deploys/calls a Solidity contract, checks logs/receipts/pending RPC and finality, and exercises live nomination and session/era rotation. Runtime tests cover the full unbonding period without waiting hours in real time.
