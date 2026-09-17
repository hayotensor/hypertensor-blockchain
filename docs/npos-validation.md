# NPoS implementation and validation

Completed on 2026-09-16. See [the inspection inventory](npos-migration.md) and [validator guide](validators.md).

## Architecture

- The existing standalone chain now runs BABE block production and GRANDPA finality.
- H160/AccountId20 remains the account type for users, validators and nominators.
- Staking manages bonds, validator intent, nominations, exposures and era rewards.
- Bounded on-chain Sequential Phragmén elects the validator set.
- Session queues and activates that set and its BABE/GRANDPA keys.
- BABE uses sr25519 session keys; GRANDPA uses ed25519 session keys.
- Authorship attributes block reward points to the staking validator account.
- Historical session proofs identify the owners of both consensus key types.
- Both equivocation report systems feed Offences, then Staking's deferred slashes.
- Frontier retains Ethereum execution, transaction conversion, RPC, storage and indexing.
- A BABE pending-digest adapter replaces Frontier's Aura pending-digest adapter.
- Existing AuthorSubsidy emissions and payout ownership proofs now resolve BABE keys.
- Existing application pallets, precompiles, fees and unrelated pallet indices are retained.
- This targets regenerated genesis for the explicitly unlaunched project, with no legacy bridge.

## Pallets and dependencies

| Added pallet | Index | Why required |
| --- | --- | --- |
| BABE | 25 | Epochs, author digests, authority updates and equivocation reports. |
| Authorship | 26 | Credit authored blocks to staking reward points. |
| Staking | 27 | Validator/nominator bonds, elections, eras, rewards and slashes. |
| Session | 28 | Activate staking-selected validators and their session keys. |
| Historical | 29 | Prove historical ownership of consensus keys for offences; supplied by Session's historical feature. |
| Offences | 30 | Route equivocation offences to Staking. |

GRANDPA remains at index 3 with session-managed membership and enabled equivocation reporting. Aura at index 2 and the embedded ManualSeal pallet at index 11 were removed because they implement the replaced PoA/sealing path; those indices remain unused. All retained pallets keep their original indices. No election pallet, bags-list, pools, governance, im-online, XCM or parachain pallet was added.

`sc-consensus-babe`, `sp-consensus-babe` and `pallet-babe` replace their Aura counterparts; `sc-consensus-manual-seal` and Frontier's `fc-rpc/aura` feature are removed. New runtime dependencies are `pallet-session` (historical enabled), `pallet-staking`, `pallet-authorship`, `pallet-offences`, `frame-election-provider-support` and `sp-staking`. Existing GRANDPA and `sp-session` dependencies remain. The existing `ethereum` workspace dependency is also used by a runtime sender-recovery test. New features are propagated where supported, including `fp-self-contained/try-runtime`, which is required to enable Frontier's matching SDK `Checkable` implementation.

SDK revision remains `823792b53d7179be3857992fb8478acc9202ee02` on stable2412; Frontier remains `84996e1fde9657a58b0ad6339980319a012c3489`. Comparing old/new lockfiles found identical Git source revision sets and no new versions of existing dependency names. `sp-consensus-aura` remains solely as a transitive dependency of the SDK facade used by retained pallets, not an active consensus engine.

## NPoS settings

| Setting | Implemented default |
| --- | --- |
| Election | OnChainExecution + SequentialPhragmen with Perbill accuracy; complete bounded voter/target maps |
| Candidates / nominators / targets per nominator | 32 / 256 / 16 |
| Election input bounds | 288 voters, 32 targets, at most 32 winners |
| Genesis desired / minimum validators | dev and eth_dev: 1; local: 2; Hoskinson: 4 / minimum 1 |
| Minimum validator / nominator bond | 1,000 / 20 TENSOR |
| BABE | 6-second slots, `c = 1/4`, primary and plain secondary slots, external session trigger |
| Session / era | 20 slots / 3 sessions: approximately 2 / 6 minutes |
| Bonding / slash deferral | 28 / 7 eras: approximately 2h48m / 42m |
| Reward history | 84 eras |
| Report longevity / GRANDPA set-session entries | 1,680 nominal slots/blocks / 84 |
| Exposure page / unlocking chunks | 64 / 32 |
| Disabling | SDK UpToLimitDisablingStrategy; no added downtime/heartbeat offences |

The isolated era payout allocates 1,000 TENSOR/year prorated by actual era duration. Claims mint inflationary rewards, apportioned by validator points, commission and elected exposure. Reward remainder is zero; unclaimed/rounding residue is not routed to treasury. Deferred slashes burn stake except the SDK reporter reward, using a configured 10% slash-reward fraction. Existing Network and AuthorSubsidy emissions remain separate. SDK-generated session/staking/election weights are used; author-subsidy weights were regenerated. BABE/GRANDPA use this SDK's nonzero default `()` implementations, which still require target-hardware validation before release.

## EVM compatibility

| Component | Result |
| --- | --- |
| AccountId20/H160, Address and IdentityLookup | Unchanged; native staking extrinsics also worked with Ethereum signatures. |
| Ethereum private keys/signatures | Unchanged; runtime legacy-transaction recovery and live signed transactions recover Alith's existing H160. |
| Chain ID / denomination / existential deposit | Unchanged: 42 / 18 decimals / 500 base units. |
| Frontier revisions, backend paths, KV/SQL configuration, mapping/cache/filter/fee workers | Retained; `node/src/eth.rs` is unchanged. Live tests used the default KV backend. |
| pallet_evm | Configuration unchanged except its required Aura-to-BABE author resolver. |
| pallet_ethereum, self-contained transactions, conversion and Ethereum runtime APIs | Unchanged in source comparison. |
| Ethereum RPC | Existing methods retained; pending digests use BABE. The removed engine RPC belongs to manual sealing. |
| Precompile addresses and implementations | Unchanged: 1–5, 1024, 1025, 2048–2051. |
| Balances backing EVM | Unchanged; consensus staking now locks bonded funds in the same Balances accounts. |
| Gas/weight mapping, block gas limit, base fee and elasticity | Unchanged: 75,000,000 block gas, initial 1 gwei base fee, 12.5% elasticity. |
| Genesis | Regenerated Hoskinson specs preserve balances, sudo, EVM accounts, EVM chain ID and Network configuration exactly; raw/plain runtime code matches. |

The active EVM configuration, Ethereum/base-fee configurations, Ethereum runtime APIs, transaction conversion and account/signature/extrinsic aliases were compared against the original source. The only EVM configuration difference is the consensus author lookup. AuthorSubsidy's consensus-specific metadata names change to BABE, while its call index, SCALE field types/order, proof domain and payout activation rules remain; weights are regenerated for the larger BABE authority entries.

## Commands and results

Commands below ran with `CARGO_BUILD_JOBS=6`; Cargo checks/builds/tests also used `--offline --locked`. Native executables used the debug profile; the final embedded Wasm used the release build type and compression. A release native executable was not built or tested.

| Command / check | Result |
| --- | --- |
| `SKIP_WASM_BUILD=1 cargo check -p hypertensor-runtime` | Passed during incremental implementation. |
| `SKIP_WASM_BUILD=1 cargo check -p hypertensor-node` | Passed. |
| `WASM_BUILD_TOOLCHAIN=stable cargo build --workspace` | Passed, including runtime Wasm and node. |
| `WASM_BUILD_TYPE=release WASM_BUILD_TOOLCHAIN=stable cargo build --workspace` | Final build passed after regenerated weights; emits compressed release Wasm. |
| `SKIP_WASM_BUILD=1 cargo test -p hypertensor-runtime -p pallet-author-subsidy` | Initial migration run: 20 runtime + 9 subsidy tests passed, zero failures. |
| `WASM_BUILD_TOOLCHAIN=stable cargo build -p hypertensor-runtime --features runtime-benchmarks` | Passed, including benchmark-enabled Wasm. |
| `SKIP_WASM_BUILD=1 cargo check -p hypertensor-node --features runtime-benchmarks` | Passed. |
| `SKIP_WASM_BUILD=1 cargo check -p hypertensor-runtime --features try-runtime` | Passed after propagating Frontier's own try-runtime feature. |
| `SKIP_WASM_BUILD=1 cargo clippy -p hypertensor-node -p hypertensor-runtime -p pallet-author-subsidy --no-deps` | Passed with warnings; no lint errors. |
| `frame-omni-bencher v1 benchmark pallet --runtime target/debug/wbuild/hypertensor-runtime/hypertensor_runtime.wasm --genesis-builder runtime --pallet pallet_author_subsidy --extrinsic '*' --steps 50 --repeat 20 --output pallets/author-subsidy/src/weights.rs --template .maintain/frame-weight-template.hbs --json-file /tmp/npos-author-subsidy-benchmarks.json` | All four benchmarks passed with verification enabled; generated weights now read Babe::Authorities. |
| `NPOS_RPC=http://127.0.0.1:19944 node evm-tests/npos-smoke.cjs` | Passed against two real local validator processes. |
| `TS_NODE_TRANSPILE_ONLY=true FRONTIER_BUILD=debug FRONTIER_SPAWNING_TIME=300000 mocha -r ts-node/register tests/test-rpc-constants.ts tests/test-web3api.ts tests/test-deprecated.ts` | 10 existing RPC regression tests passed. |
| `hypertensor-node build-spec --chain hoskinson --disable-default-bootnode` and the same command with `--raw` | Both passed; outputs installed as hoskinsonSpec.json and hoskinsonSpecRaw.json. |
| Changed Rust formatting, changed TypeScript transpilation, JS syntax, `git diff --check` | Passed. |

The live validators used isolated temporary base paths, Alice/Bob session seeds, loopback P2P ports 31333/31334 and RPC ports 19944/19945, with GRANDPA enabled. They were connected through `system_addReservedPeer` and stopped after validation. The smoke test generated/checked a 64-byte session-key bundle, observed both BABE author indices in finalized headers, submitted Ethereum-signed `bond`, `bondExtra`, `nominate` and `chill`, observed the nomination in era 2, and saw Bob become the sole active validator in session 9 / era 3 with finality advancing. It successfully claimed staking rewards and unbonded the nominator.

The Ethereum checks exercised `eth_chainId`, `eth_blockNumber`, `eth_getBalance`, `eth_sendRawTransaction`, `eth_getTransactionReceipt`, `eth_call`, `eth_estimateGas`, `eth_getCode`, `eth_getLogs`, contract deployment/event decoding, pending blocks/calls, fee history, filters and newHeads pubsub. Pending calls and contract state remained correct after validator removal. The Solidity fixture was compiled with 0.8.25 for the Paris EVM target. JS checks used the already installed test dependencies plus the declared missing test helpers installed in `/tmp`; package manifests/lockfiles were not changed for test tooling.

Runtime tests additionally cover all four presets, H160 identity, session-key encoding, original pallet indices and precompile addresses, candidate/nominator caps at the maximum election size, validation/key registration and validator replacement, rewards and withdrawal after the entire bonding period, both historical key proofs, a real signed BABE report followed by a deferred slash, and a real signed GRANDPA report queuing an H160 validator slash. The election cap test includes all 288 voters with 16 targets per nominator and checks the registered election weight remains below half the block's reference-time budget.

## Review corrections

A follow-up correctness review found and fixed two runtime issues:

- Authorship ran before Session and resolved a new BABE author index against the previous validator list. A focused failing regression credited Charlie’s first block to Alice. Session now activates the validators and era before Authorship; the same test verifies Charlie receives 20 points in the new era and the ending era’s points remain unchanged. Existing pallet indices are preserved. The block test driver also selects indices from the upcoming set at epoch boundaries.
- The 10-TENSOR minimum nominator bond rounded to zero votes with the preserved extreme `eth_dev` issuance. The minimum is now 20 TENSOR, above the maximum `U128CurrencyToVote` divisor. Tests check nonzero minimum-bond voting weight for all four presets and at `u128::MAX` issuance. Existing EVM allocations and the SDK conversion stay unchanged.

The review also corrected test assumptions left over from manual sealing: block/tag/receipt checks now follow actual heads and receipt block hashes; transaction inclusion waits tolerate session-boundary blocks without user transactions; pending-pool assertions account for automatic inclusion and use nonce gaps when a transaction must stay pending; real-slot timeouts replace short manual-seal deadlines; test shutdown waits for process exit before reusing ports; and mDNS is disabled so independent test chains sharing development keys cannot discover one another. Startup failures reject the test and stop its child process. These changes do not add a sealing compatibility path.

Review validation: `SKIP_WASM_BUILD=1 CARGO_BUILD_JOBS=6 cargo test -p hypertensor-runtime -p pallet-author-subsidy --offline --locked` passed **21 runtime + 9 subsidy tests**. The final `WASM_BUILD_TYPE=release WASM_BUILD_TOOLCHAIN=stable CARGO_BUILD_JOBS=6 cargo build --workspace --offline --locked` passed. Clippy passed with warnings. All 35 changed TypeScript files transpiled; the selected RPC tests also passed type checking. A deliberately invalid node argument verified that startup failure rejects promptly and cleans up the child process. The selected 12 RPC source files and their imports type-checked; a broader check needs the locally missing declared `@types/chai-as-promised` package. Solidity test artifacts were built with the declared solc 0.8.25 in the ignored build directory.

The live review checks covered 30 distinct RPC cases across the main runs and focused reruns:

- `tests/test-block-tags.ts tests/test-block.ts tests/test-bloom.ts tests/test-transaction-version.ts tests/test-revert-receipt.ts`: the initial run passed 24 cases and timed out waiting for one reverted deployment receipt. After disabling mDNS to isolate test chains, the focused constructor suite passed both cases. This covers 25 distinct cases, including all three Ethereum transaction types.
- `tests/test-pending-pool.ts tests/test-pending-transactions-rpc.ts --grep '\[SsTp\]'`: four cases passed initially. The remaining count assertion incorrectly compared ready-pool size to hypothetical block contents. This pinned Frontier RPC explicitly returns the ready-pool count; a session boundary can leave ready transactions out of a hypothetical block. The corrected comparison against stable `txpool_status` snapshots passed on rerun, completing five distinct cases. Frontier RPC implementation/semantics were preserved. The fork-aware variants were not rerun in this review.

These commands used `TS_NODE_TRANSPILE_ONLY=true FRONTIER_BUILD=debug FRONTIER_SPAWNING_TIME=300000` with the existing test dependencies. The separate runs used loopback RPC/P2P port pairs 19932/19931, 19942/19941 and 19952/19951. All temporary validator processes were stopped after verification. Both Hoskinson specs were regenerated from the final compressed runtime, checked for identical embedded Wasm and a 20-TENSOR nomination minimum, and compared against the prior balances, sudo, EVM, Network and session-key allocations.

## Production review

A second review reproduced silent voter truncation after Root removed the SDK's mutable nominator registration cap. `BoundedStakingElection` now checks both actual registration counts before calling the SDK's on-chain election, charging the extra database reads. Both genesis and subsequent elections use it. The regression verifies rejection, retention of the current validator set, and automatic election recovery after excess registrations are chilled and the cap restored. This also keeps elected exposures within the configured equivocation-weight nominator bound.

The maximum-size test now registers actual session keys and runs the complete runtime hooks through activation of all 32 validators, with 256 nominators voting for 16 targets each. It checks both consensus sets and a historical ownership proof containing the full 256-nominator exposure across storage pages. Its solver-weight assertion covers the election component only: the SDK conservatively charges the entire block budget for Session rotation, and this test does not establish a production wall-clock budget for all application hooks.

Additional coverage checks that key rotation retains proofs for both old consensus keys and activates both new keys after queueing. Both equivocation report systems submit decodable Frontier-compatible unsigned extrinsics to a test transaction pool; Executive accepts local reports, rejects external unsigned submissions, and rejects duplicates after reporting. A forged BABE proof containing the same header twice is rejected without changing state. The existing tests still verify the deferred slash and full unbonding lifecycle.

The module and pallet layout are intentional: `lib.rs` includes and re-exports `npos.rs`, so its configurations are compiled into the same runtime/Wasm. Explicit pallet indices identify SCALE variants and metadata; they are not ordinary FRAME storage-key prefixes. The pinned runtime macro's `legacy_ordering` option preserves declaration order for genesis/hooks. This allows Staking before Session and Session before Authorship/AuthorSubsidy without renumbering retained pallets. Indices 2/11/12 are unused, not required reservations; the original migration instruction required preserving existing indices and adding new pallets at unused new indices.

Production-review validation passed: **23 runtime + 9 author-subsidy tests**, the complete workspace build with compressed release Wasm, and Clippy for node/runtime/subsidy (warnings remain). Rust formatting and `git diff --check` passed. Both Hoskinson specs were rebuilt and verified against the final Wasm; all plain genesis configuration is unchanged. Raw state differs only in runtime code and the expected two additional database reads recorded in genesis `System::BlockWeight`. This review did not rerun the live RPC suites; their earlier results are recorded above.

**Launch status:** this review does not turn the supplied development genesis into a production release. The public Hoskinson keys, two-minute epochs, six-minute eras, short bonding/reporting windows, validator minimum of one, and provisional annual payout still require operator/product decisions. Target-hardware consensus-report and full era-boundary benchmarks, a release-native build, and sustained multi-validator operation remain release requirements. Register keys before validator intent and retire only after leaving active and queued sets; the SDK does not enforce these operational steps in `staking.validate` or `session.purgeKeys`.

## Migration risks and remaining work

This implements a fresh NPoS genesis, as required by the explicit unlaunched-project instruction. Use fresh databases and one distributed canonical spec. No existing database was purged, upgraded or bridged. An actual deployed Aura chain would need a separate coordinated client/runtime activation, staking/session bootstrap, authority handover and storage migration at a chosen height; this is not an ordinary runtime-only upgrade.

Before production genesis, replace the Hoskinson development authority tuples with operator-owned funded H160 accounts and session keys; choose final session/era, bonding/reporting windows, minimum validator count and reward budget; and benchmark consensus reports plus full era-boundary work with application hooks on target hardware. BABE epoch duration must be finalized before genesis.

The full inherited Frontier/application TS suites were not run. Some inherited load/pool tests still assume manually controlled block contents and require evaluation under automatic authoring. Executed JS coverage consists of the migration smoke, the initial 10 regression tests, and the review checks described above. SQL backend behavior was retained in source but was not exercised live. There is no claim of production load testing or a completed live-chain storage migration.

## Changed files

| File | Reason |
| --- | --- |
| [Cargo.lock](../Cargo.lock) | Resolve the new dependencies at the existing locked SDK and Frontier revisions. |
| [Cargo.toml](../Cargo.toml) | Replace Aura/manual-seal dependencies; declare only required BABE/staking/session/election dependencies. |
| [README.md](../README.md) | Describe BABE/GRANDPA NPoS and link setup/validation documentation. |
| [docs/npos-migration.md](../docs/npos-migration.md) | Record preimplementation inspection, exact versions, minimal architecture and lifecycle constraints. |
| [docs/npos-validation.md](../docs/npos-validation.md) | Record validation evidence and every changed file with its rationale. |
| [docs/validators.md](../docs/validators.md) | Document ports, H160 wallets, session keys, staking, timing, rewards, slashes and launch settings. |
| [evm-tests/README.md](../evm-tests/README.md) | Document the NPoS live smoke test and real block waits. |
| [evm-tests/build/contracts/NposSmoke.json](../evm-tests/build/contracts/NposSmoke.json) | Compiled Solidity 0.8.25 ABI/bytecode fixture for reproducible live testing. |
| [evm-tests/contracts/NposSmoke.sol](../evm-tests/contracts/NposSmoke.sol) | Minimal storage/event Solidity contract for the migration smoke test. |
| [evm-tests/npos-smoke.cjs](../evm-tests/npos-smoke.cjs) | Exercise two real validators, finality, Ethereum RPC/contracts/signatures and live staking/election/rotation. |
| [evm-tests/src/network.ts](../evm-tests/src/network.ts) | Replace engine-driven block creation helpers with waits for actual block production and finality. |
| [evm-tests/tests/overwatch-node.register-remove.test.ts](../evm-tests/tests/overwatch-node.register-remove.test.ts) | Update calls to the real-block wait helper after removing manual sealing. |
| [evm-tests/tests/overwatch-node.stake.add-remove.ts](../evm-tests/tests/overwatch-node.stake.add-remove.ts) | Update calls to the real-block wait helper after removing manual sealing. |
| [evm-tests/tests/overwatch-node.stake.commit-reveal.ts](../evm-tests/tests/overwatch-node.stake.commit-reveal.ts) | Update calls to the real-block wait helper after removing manual sealing. |
| [evm-tests/tests/overwatch-node.views.ts](../evm-tests/tests/overwatch-node.views.ts) | Update calls to the real-block wait helper after removing manual sealing. |
| [node/Cargo.toml](../node/Cargo.toml) | Use BABE client/primitives; remove manual-seal and Frontier Aura feature. |
| [node/src/chain_spec.rs](../node/src/chain_spec.rs) | Remove the obsolete sealing argument from Ethereum development genesis. |
| [node/src/cli.rs](../node/src/cli.rs) | Remove manual/instant sealing CLI controls. |
| [node/src/client.rs](../node/src/client.rs) | Require BabeApi in the client runtime API collection. |
| [node/src/command.rs](../node/src/command.rs) | Update service calls and revert BABE epoch auxiliary data with GRANDPA. |
| [node/src/rpc/eth.rs](../node/src/rpc/eth.rs) | Supply BABE data for Frontier pending execution. |
| [node/src/rpc/mod.rs](../node/src/rpc/mod.rs) | Remove the manual-seal engine RPC and use the BABE runtime API bound. |
| [node/src/rpc/pending.rs](../node/src/rpc/pending.rs) | Construct plain-secondary BABE digests using the current/next epoch for pending blocks. |
| [node/src/service.rs](../node/src/service.rs) | Wire BABE import/authoring and keep its worker alive alongside the existing Frontier/GRANDPA tasks. |
| [pallets/author-subsidy/README.md](../pallets/author-subsidy/README.md) | Document BABE ownership and retained author-payout semantics. |
| [pallets/author-subsidy/SET_REWARD_ADDRESS.md](../pallets/author-subsidy/SET_REWARD_ADDRESS.md) | Update the operator payout instructions to BABE key/metadata names. |
| [pallets/author-subsidy/src/benchmarking.rs](../pallets/author-subsidy/src/benchmarking.rs) | Use BABE key type and authority setup in existing benchmarks. |
| [pallets/author-subsidy/src/lib.rs](../pallets/author-subsidy/src/lib.rs) | Rename the consensus authority adapter and metadata fields to BABE while preserving proof layout, call index and payout policy. |
| [pallets/author-subsidy/src/mock.rs](../pallets/author-subsidy/src/mock.rs) | Update the authority adapter in the test runtime. |
| [pallets/author-subsidy/src/tests.rs](../pallets/author-subsidy/src/tests.rs) | Use the BABE adapter and field names in existing behavior tests. |
| [runtime/Cargo.toml](../runtime/Cargo.toml) | Add consensus pallets, historical session support and feature propagation; add Ethereum recovery test dependency. |
| `runtime/src/apis.rs` (deleted) | Delete the disconnected duplicate Aura runtime API implementation. |
| [runtime/src/author_subsidy_tests.rs](../runtime/src/author_subsidy_tests.rs) | Run existing payout integration assertions with BABE authorities/digests. |
| `runtime/src/configs/mod.rs` (deleted) | Delete the disconnected duplicate Aura configuration/runtime implementation. |
| [runtime/src/genesis_config_presets.rs](../runtime/src/genesis_config_presets.rs) | Map existing H160 development wallets to session keys and initialize bonded validators in every preset. |
| [runtime/src/lib.rs](../runtime/src/lib.rs) | Compose consensus pallets at new indices, order hooks/genesis, expose BABE/equivocation APIs, adapt author lookup and bump spec version. |
| [runtime/src/npos.rs](../runtime/src/npos.rs) | Isolate the version-correct NPoS configurations, bounded elections, timing, rewards and offence handlers. |
| [runtime/src/npos_tests.rs](../runtime/src/npos_tests.rs) | Test genesis, H160/signatures, keys/indices/precompiles, staking lifecycle/elections, rotation, historical proofs and slashing. |
| [ts-tests/README.md](../ts-tests/README.md) | Explain real BABE block production and inherited deterministic-test assumptions. |
| [ts-tests/tests/config.ts](../ts-tests/tests/config.ts) | Match the existing runtime name and new spec version. |
| [ts-tests/tests/test-balance.ts](../ts-tests/tests/test-balance.ts) | Update calls to the real-block wait helper after removing manual sealing. |
| [ts-tests/tests/test-block-tags.ts](../ts-tests/tests/test-block-tags.ts) | Compare moving Ethereum head tags with native consensus heads. |
| [ts-tests/tests/test-block.ts](../ts-tests/tests/test-block.ts) | Check live blocks, timestamps, pending contents and receipts without fixed mining heights. |
| [ts-tests/tests/test-bloom.ts](../ts-tests/tests/test-bloom.ts) | Wait for real transaction inclusion and use receipt block hashes where appropriate; allow BABE timing. |
| [ts-tests/tests/test-contract-methods.ts](../ts-tests/tests/test-contract-methods.ts) | Wait for real transaction inclusion and use receipt block hashes where appropriate; allow BABE timing. |
| [ts-tests/tests/test-contract-storage.ts](../ts-tests/tests/test-contract-storage.ts) | Wait for real transaction inclusion and use receipt block hashes where appropriate; allow BABE timing. |
| [ts-tests/tests/test-contract.ts](../ts-tests/tests/test-contract.ts) | Update calls to the real-block wait helper after removing manual sealing. |
| [ts-tests/tests/test-eip1153.ts](../ts-tests/tests/test-eip1153.ts) | Wait for real transaction inclusion and use receipt block hashes where appropriate; allow BABE timing. |
| [ts-tests/tests/test-execute.ts](../ts-tests/tests/test-execute.ts) | Wait for real transaction inclusion and use receipt block hashes where appropriate; allow BABE timing. |
| [ts-tests/tests/test-fee-history.ts](../ts-tests/tests/test-fee-history.ts) | Update calls to the real-block wait helper after removing manual sealing. |
| [ts-tests/tests/test-filter-api.ts](../ts-tests/tests/test-filter-api.ts) | Wait for real transaction inclusion and use receipt block hashes where appropriate; allow BABE timing. |
| [ts-tests/tests/test-gas.ts](../ts-tests/tests/test-gas.ts) | Wait for real transaction inclusion and use receipt block hashes where appropriate; allow BABE timing. |
| [ts-tests/tests/test-log-filtering.ts](../ts-tests/tests/test-log-filtering.ts) | Wait for real transaction inclusion and use receipt block hashes where appropriate; allow BABE timing. |
| [ts-tests/tests/test-log-tags.ts](../ts-tests/tests/test-log-tags.ts) | Update calls to the real-block wait helper after removing manual sealing. |
| [ts-tests/tests/test-max-priority-fee-per-gas-rpc.ts](../ts-tests/tests/test-max-priority-fee-per-gas-rpc.ts) | Update calls to the real-block wait helper after removing manual sealing. |
| [ts-tests/tests/test-nonce.ts](../ts-tests/tests/test-nonce.ts) | Update calls to the real-block wait helper after removing manual sealing. |
| [ts-tests/tests/test-pending-pool.ts](../ts-tests/tests/test-pending-pool.ts) | Check pending snapshots and actual receipts while BABE continues producing blocks. |
| [ts-tests/tests/test-pending-transactions-rpc.ts](../ts-tests/tests/test-pending-transactions-rpc.ts) | Check pending snapshots and actual receipts while BABE continues producing blocks. |
| [ts-tests/tests/test-precompiles.ts](../ts-tests/tests/test-precompiles.ts) | Update calls to the real-block wait helper after removing manual sealing. |
| [ts-tests/tests/test-revert-reason.ts](../ts-tests/tests/test-revert-reason.ts) | Wait for real transaction inclusion and use receipt block hashes where appropriate; allow BABE timing. |
| [ts-tests/tests/test-revert-receipt.ts](../ts-tests/tests/test-revert-receipt.ts) | Wait for real transaction inclusion and use receipt block hashes where appropriate; allow BABE timing. |
| [ts-tests/tests/test-rpc-constants.ts](../ts-tests/tests/test-rpc-constants.ts) | Clarify the unchanged zero coinbase before a BABE payout destination is configured. |
| [ts-tests/tests/test-state-override.ts](../ts-tests/tests/test-state-override.ts) | Wait for real transaction inclusion and use receipt block hashes where appropriate; allow BABE timing. |
| [ts-tests/tests/test-state-root.ts](../ts-tests/tests/test-state-root.ts) | Compare state roots from the actual returned blocks. |
| [ts-tests/tests/test-subscription.ts](../ts-tests/tests/test-subscription.ts) | Wait for real authoring and compare log data with its own block hash. |
| [ts-tests/tests/test-transaction-priority.ts](../ts-tests/tests/test-transaction-priority.ts) | Update calls to the real-block wait helper after removing manual sealing. |
| [ts-tests/tests/test-transaction-version.ts](../ts-tests/tests/test-transaction-version.ts) | Wait for real transaction inclusion and use receipt block hashes where appropriate; allow BABE timing. |
| [ts-tests/tests/txpool.ts](../ts-tests/tests/txpool.ts) | Update calls to the real-block wait helper after removing manual sealing. |
| [ts-tests/tests/util.ts](../ts-tests/tests/util.ts) | Start/stop a BABE validator reliably, wait for authored/finalized blocks and finalized transaction receipts. |
| [hoskinsonSpec.json](../hoskinsonSpec.json) | Regenerate the plain Hoskinson spec with the final NPoS Wasm and staking/session genesis, preserving existing application/EVM allocations. |
| [hoskinsonSpecRaw.json](../hoskinsonSpecRaw.json) | Regenerate the matching raw NPoS genesis storage and runtime code. |
| [pallets/author-subsidy/src/weights.rs](../pallets/author-subsidy/src/weights.rs) | Regenerate all four weights using the maximum BABE authority list and verified benchmark execution. |
