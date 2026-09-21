# Production review: NPoS and Revive

Reviewed on 2026-09-18 against the pinned `polkadot-stable2606-2` SDK. This is an
integration review with regression tests, not a security audit or production
certification. The SDK release remains pinned; no compatibility adapters or
storage migrations have been introduced for this unlaunched chain.

## Findings fixed

| Finding | Impact and change |
| --- | --- |
| `NonTransfer` proxy defaulted to allowing new pallets | A delegate could invoke Sudo for a sudo owner, reserve funds through AtomicSwap, use Multisig dispatch, or redirect staking rewards. Replaced the denylist with a narrow allowlist: system remarks, session key management, selected NPoS operations, and announcement rejection. Sudo, Multisig, AtomicSwap, Revive, reward destination changes, and Network calls are denied. Use the dedicated Network proxy types for their explicitly scoped operations. |
| TxPause could pause mandatory/recovery calls | Root could pause `Timestamp.set`, preventing valid blocks, or Sudo, losing the root recovery path. Timestamp and Sudo are now unpausable; ordinary calls remain filtered. |
| Scheduler ran before other expensive hooks | Its 80% budget could overlap Network's 50% budget and election work. Scheduler now runs last, meters remaining weight, and leaves 10% of total block capacity for inherents and extrinsics. Its pallet index is now 25; regenerate genesis specifications. |
| Maximum Ethereum transaction could exceed capacity left by Network | The previous 90% fraction of normal extrinsic capacity could not fit alongside maximum Network initialization. Reduced it to 60% of normal extrinsic capacity; a regression checks that the combined weight leaves at least 10% of the block in both dimensions. Exceptional election or scheduled work can still defer a transaction. |
| Default staking horizons were development settings | Two-minute sessions and six-minute eras gave a roughly 2.8-hour bonding period. Default builds now use four-hour sessions, daily eras, 28-day bonding, seven-day slash deferral, and 84-day reward history. The explicit `fast-runtime` feature retains accelerated development timings. |
| Fees and storage deposits were effectively negligible for 18-decimal TENSOR | Added a congestion-responsive multiplier with a nonzero floor compatible with Revive, byte fees, a meaningful existential deposit, and a common storage-price schedule including Multisig. These are baseline prices requiring economic calibration, not a claim that denial-of-service attacks are economically impossible. |
| Built-in Live preset used public validator/sudo keys | Removed the runtime's Hoskinson preset and made `--chain hoskinson` fail with an explanation. Development/local presets remain explicitly disposable. Production needs an operator-reviewed JSON genesis. |
| BABE ignored sustained finality lag | Enabled the SDK's standard finality-lag authoring backoff. |
| Treasury burned half its pot every two blocks | Disabled automatic burning, changed the spend cycle to one day, and extended the payout window from ten blocks to 30 days. A regression funds the treasury, processes spend boundaries, and claims an approved payout a day later. |
| Metadata builder advertised `UNIT` with 12 decimals | Corrected it to `TENSOR` with 18 decimals. This alone does not implement or certify metadata-hash signing by hardware wallets. |

Prices are documented in the [Revive guide](smart-contracts.md); timings and
validator limits are in the [NPoS configuration](npos.md).

## Unresolved release requirements

1. **Network candidate commitment timing needs a protocol decision.**
   The runtime now selects `pallet-randomness`, backed by BABE's
   `RandomnessFromTwoEpochsAgo`. The old provider, unused random-number helpers,
   parent-hash input, and election-block input have been removed. Full-hash range
   reduction now guarantees an index for nonempty pools without retry exhaustion.
   Network's hooks now run after BABE/Session epoch rotation.
   Candidate pools are still resolved at election time; no three-epoch eligibility
   snapshots, bootstrap gate, or fixed-epoch retry policy have been introduced.
   Those protocol changes affect activation and emergency elections and remain
   required before relying on selection for value-bearing security. A source
   replacement alone does not prevent candidate/input grinding. See the
   [randomness pallet requirements](../pallets/randomness/README.md).
   The subsequent [randomness audit](../pallets/randomness/AUDIT.md) also reproduces
   an upstream accumulation limitation: the pinned BABE version excludes its
   current 256-entry VRF segment from epoch updates. Small batches can remain
   buffered across epochs while outputs change predictably. A cutoff check alone
   cannot certify incorporation of fresh VRF entropy. This remains unresolved in
   the consensus dependency.
2. **Assign the Ethereum chain ID and production genesis.** `ChainId = 1337`
   remains a development value. Choose a unique ID and operator-controlled
   validator/sudo keys, funded accounts, validator count/minimum, and reward
   policy before creating the production genesis. Never distribute the built-in
   public-key presets as a production network.
3. **Measure and calibrate.** Generate weights on the intended minimum validator
   hardware, including worst-case Network settlement plus staking elections and
   Revive execution. SDK reference weights and runtime integrity tests do not
   establish hardware capacity. Validate fee/deposit prices and consensus reward
   amounts against token economics. Review custom pallet weights as well.
4. **Validate the external Ethereum RPC deployment.** The matching SDK `eth-rpc`
   sidecar is optional and separate from the node. Native runtime tests do not
   establish its wallet/indexer compatibility, archival operation, or recovery
   behavior. Test that service if Ethereum wallets are part of launch scope.

The chain should not be described as unconditionally production-ready while these
items are unresolved. In particular, a successful build or an upstream Revive
audit does not resolve the application's randomness design.

## Reproduce the checks

```sh
SKIP_WASM_BUILD=1 cargo test --locked -p hypertensor-runtime
SKIP_WASM_BUILD=1 cargo test --locked -p hypertensor-runtime --features fast-runtime --test production_config
cargo build --locked -p hypertensor-node
SKIP_WASM_BUILD=1 cargo check --locked -p hypertensor-runtime --features try-runtime
SKIP_WASM_BUILD=1 SKIP_PALLET_REVIVE_FIXTURES=1 cargo check --locked -p hypertensor-node --features runtime-benchmarks
cargo fmt --all -- --check
```

Validation completed: all 37 runtime tests and the production-configuration test
passed, as did the separate fast-configuration test, node/Wasm build, `try-runtime`,
benchmark-feature check, formatting, and whitespace checks. A temporary node ran
the final compiled Wasm (verified by its on-chain code hash), reported 2,400-slot
BABE epochs, executed Ethereum and native-account contract calls, and finalized
them with GRANDPA. The node was stopped after testing.

The configuration integration test links the actual runtime library without the
unit tests' accelerated timings. The authorization regressions execute proxy
dispatch and root recovery, rather than only inspecting filter predicates.
The benchmark command checks feature integration; it does not execute hardware
benchmarks or build upstream contract fixtures.

Reference: [pinned SDK release](https://github.com/paritytech/polkadot-sdk/releases/tag/polkadot-stable2606-2).
