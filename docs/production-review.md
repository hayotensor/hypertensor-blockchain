# Production operations

These notes describe the runtime in this checkout. Use them alongside the
[launch profile](../README.md#launch-profile) and
[validator guide](validators.md) when preparing a fresh network.

## Genesis and network identity

Set `PRODUCTION_BLOCKS_PER_EPOCH` and `PRODUCTION_CHAIN_ID` in
[runtime/src/lib.rs](../runtime/src/lib.rs) before building with
`--features production`. Choose both values before generating genesis; they are
fixed for the life of the chain. The production profile cannot be combined with
`fast-runtime`.

Use operator-controlled accounts and consensus keys in the production genesis.
The production runtime rejects public development keys (`//Alice` through
`//Ferdie` and their `//stash` accounts) in sudo, session keys, staking slots,
balances, or council membership. Replace all such entries if using the local
preset as a template, and use the production runtime in the final specification.
The enforcement is in
[genesis validation](../runtime/src/genesis/presets.rs) and
[tokenomics validation](../runtime/src/tokenomics/mod.rs).

Ethereum replay protection uses the compile-time `ChainId`. Assign each public
network its own `PRODUCTION_CHAIN_ID`; the development ID 1337 is rejected by the
production build. Unprotected pre-EIP-155 transactions are rejected, so
deployments that rely on Nick's method are unsupported. See the
[Revive configuration](../runtime/src/configs/revive.rs).

## Slashing and withdrawal timing

`BondingDuration` is 28 staking eras and `SlashDeferDuration` is
`BondingDuration - 2`, or 26 eras. This lets a slash apply before an offender who
unbonded in the offence era can withdraw. Era lengths depend on the
[build profile](../README.md#build-profiles).

Reports filed 27 or 28 eras late can still be accepted by BABE/GRANDPA because
their report-age limit follows retained historical sessions. Their deferred
slash is never applied. Account for this limit when operating equivocation
reporting. The timing configuration is in
[npos.rs](../runtime/src/configs/npos.rs); executable checks for deferred slashes
and withdrawal ordering are in the [NPoS tests](../runtime/src/tests/npos.rs).

## Scheduled calls and block capacity

Network hooks may consume up to 50% of a block through `MaximumHooksWeight`.
The scheduler accounts for weight already consumed and reserves at least 10%
of the block for inherents and extrinsics. A scheduled Root call that exceeds
the available scheduler budget can be dropped permanently.

Keep scheduled Root calls small, or schedule them away from subnet slot blocks.
Verify scheduler events after execution. See
[scheduler and Network configuration](../runtime/src/configs/common.rs) and the
[block-budget tests](../runtime/src/tests/production.rs).

## Attestation inclusion

`Network.propose_attestation` and `Network.attest` are Normal-class calls with no
additional transaction priority. Successful calls return `Pays::No`; this
post-dispatch fee treatment does not give them priority before inclusion.
Sustained filler traffic can delay calls past their consensus window.

Subnet validators should consider adding a tip during congestion. Operators
should monitor block fullness, fee adjustment through `TargetedFeeAdjustment`,
and proposal/attestation inclusion using the
[network RPC](../pallets/network/rpc/API.md).

Any future priority boost needs transaction validation that checks the signer
is the elected proposer or an eligible attestor. An unconditional boost would
also prioritize invalid calls. Current behavior is defined by the
[Network calls](../pallets/network/src/lib.rs),
[attestation implementation](../pallets/network/src/consensus/attestation/subnet_validator.rs),
and [runtime transaction extensions](../runtime/src/lib.rs).
