# NPoS migration inventory

Inspected before implementation. The repository, Cargo.lock and locally checked-out dependencies are the API authority.

## Versions and existing architecture

- Polkadot SDK: `stable2412`, locked to `823792b53d7179be3857992fb8478acc9202ee02` (`sp-runtime 40.1.0`).
- Frontier: locked to `84996e1fde9657a58b0ad6339980319a012c3489`. Its workspace also selects SDK `stable2412`. The manifests select a branch/unqualified Frontier Git source; **Cargo.lock supplies the exact compatible revisions**. Keep the lockfile and use `--locked`; no dependency upgrade is required.
- EVM interpreter: `fc83ff905215291339e67d788932605007ea4ef8`; Ethereum types: `3be0d8fd4c2ad1ba216b69ef65b9382612efc8ba`.
- Aura has a fixed genesis authority list, six-second slots, and no session or staking pallet. GRANDPA provides finality with equivocation reports disabled. An optional manual/instant-seal service bypasses Aura/GRANDPA.
- Opaque keys currently contain Aura sr25519 and GRANDPA ed25519. User signatures are `fp_account::EthereumSignature`; the signer derives `AccountId20`. `Address = AccountId`, `Lookup = IdentityLookup<AccountId>`, EVM origins are `EnsureAccountId20`, and address mapping is `IdentityAddressMapping`.
- `runtime/src/lib.rs` contains the active pallet configurations and runtime APIs. `runtime/src/apis.rs`, `configs/mod.rs`, and `precompiles.rs` are disconnected duplicate template modules, not compiled by lib.rs.
- Frontier uses self-contained Ethereum transactions, Ethereum transaction conversion and the full Ethereum runtime API. RPC retains call/estimate/send/receipt/log/filter/pubsub, fee history, block caches, txpool and pending execution. `node/src/eth.rs` owns KV/SQL backend configuration and mapping/filter/fee-history workers.
- Native Balances back EVM accounts directly; 18 decimals, chain ID 42 in all presets, 75,000,000 block gas limit, fixed gas/weight mapping, 1 gwei initial base fee, 12.5% elasticity. Dynamic-fee configuration exists but its pallet is not composed into the runtime; preserve that existing behavior.
- The active precompile set is in `precompiles/src/lib.rs`: Ethereum 1–5, 1024/1025, and existing network staking/subnet/overwatch/admin addresses.
- Custom Network, AtomicSwap, Collective and AuthorSubsidy pallets remain. Network's application staking and epochs are independent of consensus staking. AuthorSubsidy authorizes payout destinations with an Aura key and must be adapted to BABE without changing its payout policy.
- Existing runtime indices: System 0, Timestamp 1, Aura 2, Grandpa 3, Balances 4, TransactionPayment 5, Sudo 6, Ethereum 7, EVM 8, EVMChainId 9, BaseFee 10, ManualSeal 11, AtomicSwap 13, InsecureRandomnessCollectiveFlip 14, Utility 15, Proxy 16, Preimage 17, Scheduler 18, Treasury 19, Multisig 20, TxPause 21, Collective 22, Network 23, AuthorSubsidy 24.
- Node service already integrates telemetry, offchain workers, GRANDPA warp sync, transaction pool and Frontier tasks. CLI includes key management, chain operations, Frontier DB and optional benchmarking. Retain these, updating consensus import/authoring/revert and removing sealing controls.
- Tests include custom pallet tests, runtime author-subsidy tests, application EVM tests and a Frontier TS suite using manual sealing. Benchmark and try-runtime features already exist; propagate new dependencies through them where supported.

## Minimal target and constraints

- Remove Aura and ManualSeal, leaving their indices unused. Add BABE, Session, Historical, Staking, Authorship and Offences at new indices after 24. GRANDPA stays at 3.
- Use the SDK's bounded `onchain::OnChainExecution` with `SequentialPhragmen`, staking's `UseNominatorsAndValidatorsMap` and `UseValidatorsMap`. Explicit candidate/nominator registration limits keep complete elections small; no bags-list or offchain election mining is required.
- The exact staking Config uses `CurrencyBalance`, `NominationsQuota`, paged exposures, `TargetList`, `DisablingStrategy` and `MaxControllersInDeprecationBatch`; older example configurations are incompatible.
- Session rotation must use BABE's external trigger; session management wraps Staking in `NoteHistoricalRoot`. BABE/GRANDPA report systems use historical `MembershipProof` and Offences → Staking, with unsigned transaction construction in the runtime.
- Existing economic handlers and fees remain. Add an isolated, simple era payout default for consensus validators/nominators; document new issuance and slash behavior separately from existing author/network emissions.
- Frontier's locked revision has an Aura pending digest provider only. Implement the corresponding BABE digest provider locally using its existing `ConsensusDataProvider` interface; retain the rest of Frontier RPC execution.

## Chain lifecycle

The repository includes a `ChainType::Live` Hoskinson testnet preset, operator public keys, and serialized PoA specs with embedded runtime code. These are evidence of testnet configuration, not proof of current deployment. The explicit project instruction says to treat the software as unlaunched and not bridge legacy logic: this implementation targets regenerated NPoS genesis, with no in-place migration.

An actual deployed Aura chain cannot adopt this change as an ordinary runtime-only upgrade. It would require coordinated client/consensus activation, staking bootstrap and session initialization, authority transition, historical-proof setup, and explicit storage/runtime migration at an agreed height. No existing database is purged or migrated by this work. Old embedded PoA genesis specs must be regenerated with the new node.

## Implementation findings

- New indices are BABE 25, Authorship 26, Staking 27, Session 28, Historical 29 and Offences 30. Retained indices remain fixed as requested; removed Aura/ManualSeal leave 2/11 unused, and 12 was already unused. FRAME requires unique indices, not consecutive numbering or reservations. These gaps contain no pallet or compatibility implementation.
- The SDK's `#[frame_support::runtime]` normally sorts genesis/hooks by pallet index. Its `legacy_ordering` option selects declaration order; here that means BABE → Staking → Session → Authorship → Historical → Offences → existing AuthorSubsidy, after the other existing pallets. This is hook scheduling, not legacy consensus/state compatibility. Staking genesis must precede session genesis, authorship must resolve the newly activated validator indices and credit the active era after session rotation, and subsidy/EVM author lookup must use the rotated BABE keys. A regression test reproduces the wrong-account reward caused by the SDK example’s opposite authorship ordering at a validator replacement boundary.
- Keep the BABE worker handle alive with TaskManager; dropping its last request sender would terminate an essential service task even if no BABE-specific RPC is exposed.
- Both locally authored and imported blocks pass through BABE → Frontier → GRANDPA block imports. GRANDPA justification import and the existing Frontier background workers are retained. Chain reversion also reverts BABE auxiliary epoch data.
- The bounded on-chain election uses SDK `U128CurrencyToVote`; exposure values can differ from native bonds by rounding below one vote-conversion unit. Validator and nominator rewards use the actual elected exposure.
- `StakersElected` is emitted during era planning; active-era/session transitions happen later. Tests account for the session queue rather than expecting those events in the activation block.
- Disconnected Aura `runtime/src/apis.rs` and `runtime/src/configs/mod.rs` were removed; the existing active implementation remains in lib.rs. The other custom runtime modules and precompiles remain.
- The old sealing flag and engine RPC are removed. Existing TS helper calls are renamed to waits for real blocks/finality. Exact timestamp, paused-pool and fixed-block-number assertions in the inherited Frontier suite need evaluation under probabilistic consensus; the standalone NPoS smoke test is the primary migration integration check.
- `sp-consensus-aura` remains only as a transitive primitive dependency of `polkadot-sdk-frame`, required by the existing Proxy/Multisig pallets. There is no Aura runtime pallet, key, runtime API or node authoring/import task. Removing those retained pallets to prune that facade dependency would violate the migration's scope.
