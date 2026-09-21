# Standalone NPoS runtime

The workspace pins Polkadot SDK `polkadot-stable2606-2` (`72284b37a234f8a1565f2043ddbf8280a69fcf29`)
and Rust 1.93.0. This was the latest stable SDK release reviewed on 2026-09-18.
The runtime is a fresh standalone chain with BABE block production and GRANDPA finality.
There are no storage migrations or compatibility adapters for previous deployments.

The consensus pallet set is BABE, GRANDPA, Session, Session Historical, Staking,
Authorship, and Offences, using the existing System, Timestamp, and Balances foundations.
The existing application and governance pallets are separate from consensus. No election,
bags-list, nomination-pools, fast-unstake, im-online, authority-discovery, or parachain
pallets are needed for this bounded configuration.

Native accounts use `AccountId32`, `MultiSignature`, direct account lookup, and
Revive's SDK unchecked-extrinsic wrapper. Transaction extensions include call authorization,
nonce/payment checks, and post-dispatch weight reclamation. Token precision is 18 decimals;
SS58 prefix is 42.

Staking uses on-chain Sequential Phragmén with a single election page. Registration
caps are 32 validator candidates and 256 nominators, with at most 16 nomination targets.
The output allows 32 winners and 257 backers per winner, including the validator's
self-vote. Sorting/truncation is disabled. Elections reject registrations above the
runtime bounds even if Root raises the mutable registration limits. Staking retains
the previous validator set until a successful election. These small bounds are essential:
expanding them requires fresh weight measurements and an election design review.

Staking uses balance holds. Session keys use account identities directly, require
proofs of possession bound to the registering account, and hold a refundable 1 TENSOR
key deposit for new registrations. Session controls validator disabling using the SDK's
severity-aware strategy with re-enabling. Historical proofs identify validator accounts
with `UnitIdentificationOf`; Staking reads the relevant era exposures when slashing.
Both consensus engines submit equivocation reports through Offences to Staking.
This configuration does not automatically report offline validators; operators
must monitor validator availability.

Default epochs last 2,400 six-second slots (four hours). An era spans six sessions
(one day). Bonding lasts 28 eras (28 days), deferred slashing seven eras (seven days),
and reward history 84 eras (84 days), assuming normal block production. Validator
bonds start at 1,000 TENSOR and nominator bonds at 20 TENSOR. Consensus rewards have
a separate 1,000 TENSOR annual budget prorated by era duration.

The explicit `fast-runtime` build feature uses 20-slot sessions and three sessions
per era for disposable local networks; unit tests use these accelerated timings.
The separate configuration integration test checks the actual library build without
`cfg(test)`. Set epoch duration before creating genesis; it cannot be changed by an
ordinary runtime upgrade. The default timings do not turn the supplied public-key
development/local genesis presets into production specifications.

Hook execution follows pallet indices. Balances and Staking initialize before
Session; Session rotates before Authorship resolves the current author. All imported
and authored blocks pass through BABE and GRANDPA block imports. BABE/GRANDPA tasks
are essential, historical ownership proofs enable equivocation reporting, and both
engines' auxiliary state is reverted by the node's revert command. Scheduler runs
last (index 25), after Network, consensus and Revive hooks, and reserves 10% of the
block budget for mandatory inherents and extrinsics. Its remaining-weight meter
prevents the former 80% Scheduler / 50% Network budgets from independently consuming
the same capacity.

The node exposes native system, payment, and Network RPC methods. Network startup
honors the SDK's Libp2p/Litep2p selection. BABE uses the SDK's finality-lag authoring
backoff. GRANDPA's pruning filter preserves blocks needed for finality proofs; GRANDPA warp sync is enabled. Chain operations, telemetry,
and offchain workers use SDK services. On-chain elections do not require an offchain miner.

The runtime uses the SDK's supplied weights for consensus. BABE and GRANDPA expose
nonzero default weights through `WeightInfo = ()`. Runtime tests check the bounded
election's registered weight; they do not replace benchmarking on production hardware.

Reference sources:

- [SDK release and support registry](https://github.com/paritytech/release-registry)
- [Pinned SDK release](https://github.com/paritytech/polkadot-sdk/releases/tag/polkadot-stable2606-2)
- [Standalone reference runtime](https://github.com/paritytech/polkadot-sdk/blob/polkadot-stable2606-2/substrate/bin/node/runtime/src/lib.rs)
- [Bounded on-chain elections](https://github.com/paritytech/polkadot-sdk/blob/polkadot-stable2606-2/substrate/frame/election-provider-support/src/onchain.rs)
- [Fresh-chain historical identification](https://github.com/paritytech/polkadot-sdk/blob/polkadot-stable2606-2/substrate/frame/staking/src/lib.rs)
- [Session key ownership and deposits](https://github.com/paritytech/polkadot-sdk/blob/polkadot-stable2606-2/substrate/frame/session/src/lib.rs)
