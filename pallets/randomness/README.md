# BABE randomness pallet

`pallet-randomness` is a standalone, stateless FRAME pallet that derives application
randomness from BABE's primary-block VRF contributions. It uses
`pallet_babe::RandomnessFromTwoEpochsAgo`, with an application domain prefix.

The Hypertensor runtime configures this pallet as Network's randomness provider.
The crate remains independently testable through the root workspace's `pallets/*`
glob.

## How it works

1. BABE block authors produce sr25519 VRF outputs. The consensus client verifies
   their proofs before executing imported blocks.
2. The BABE pallet collects primary-block VRF contributions and mixes them into
   epoch randomness. Secondary plain blocks do not contribute VRF entropy;
   secondary VRF outputs are not included in this epoch accumulator either.
3. This pallet uses BABE's delayed epoch source. It SCALE-encodes a fixed domain
   tag and the caller's subject, and passes them to the SDK provider.
4. The result is a runtime hash plus the provider's commitment cutoff block.

There are no dispatchables, hooks, offchain workers, events, storage items,
request queues, fees, or additional keys. No custom cryptography, block-hash
fallback, or commit–reveal protocol is introduced. A fixed subject produces the
same result throughout an epoch; the pallet is not a fresh draw on each call.

## API

- `Pallet::<T>::random(subject)` returns `(T::Hash, BlockNumberFor<T>)`.
- `Pallet::<T>::random_after(subject, committed_at)` returns `Some((hash, cutoff))`
  only when `committed_at < cutoff`; otherwise it returns `None`.
- The pallet implements FRAME's `Randomness<T::Hash, BlockNumberFor<T>>` trait.
  Its default `random_seed()` uses an empty subject.

Prefer `random_after` when consuming randomness for a decision. It rejects
BABE's initial zero cutoff and commitments at or after the returned cutoff.
The raw `random` and trait APIs intentionally preserve the SDK interface,
including deterministic bootstrap output. A nonzero hash alone does **not**
prove that VRF entropy is available.

## Availability and entropy

The raw API always returns a hash during normal runtime execution. It does not
wait for a VRF, return an optional seed, or retry. This includes genesis, epochs
containing only secondary blocks, and recovery after skipped epochs. Repeated
subjects within an epoch return the same value. Zero is also a valid output.
`random_after` intentionally remains optional because an unsatisfied commitment
cutoff must not be silently treated as satisfied.

**Pinned SDK limitation:** `polkadot-stable2606-2` consumes
`UnderConstruction[0..SegmentIndex]` when computing the next seed, excluding the
current segment. Each segment holds 256 primary VRF contributions. With
`SegmentIndex == 0`, no VRF bytes enter that epoch update, even if up to 256 primary
contributions have been buffered. Segment zero can carry over across epochs.
The seed still changes by hashing the prior announced seed and epoch index, which
is publicly predictable. The tests reproduce this at 1, 255, and 256 contributions
and separately prove that 257 contributions cause the first segment to be mixed.

This pallet retains the upstream consensus source; it does not modify BABE's
accumulator. Short development epochs and sparse production can therefore delay
actual VRF incorporation beyond the nominal two-epoch source delay. Neither a
changed hash, an advancing cutoff, nor a successful `random_after` call certifies
fresh entropy. See the [audit findings](AUDIT.md) before relying on this provider
for a security-sensitive decision.

## Consumer requirements

This pallet is a randomness provider, not a selection or request scheduler.
The consuming pallet must implement the following rules:

- Fix all inputs before the random seed is knowable: eligible candidates, their
  ordering, subject, selection rules, and the intended draw epoch. Persist the
  actual commitment block; do not accept an arbitrary user-supplied earlier date.
- Follow BABE's conservative rule: inputs for `RandomnessFromTwoEpochsAgo` should
  be committed at least **three epochs** before use. For example, commit in epoch
  E and draw in E+3 after BABE has processed that epoch transition. With the current
  four-hour production BABE epochs, this means roughly 8–12 hours of waiting.
- Check the returned block cutoff as well. The `random_after` check alone does
  not establish the three-epoch rule or verify that a commitment exists.
- Freeze the subject too. Include a purpose and immutable request/subnet ID, but
  do not mix in the draw block's hash, timestamp, a late-selected candidate list,
  or a user-controlled nonce chosen after the seed becomes known.
- Execute at the predetermined epoch and persist the result if it is needed
  later. This pallet only exposes the current source; it does not retain history.
  Allowing someone to delay until a favorable later epoch enables selective draws.
- Account for skipped epochs and chain outages. Slot epoch indices can jump
  without the corresponding entropy-collection rounds taking place. Do not treat
  a numerical jump of three as proof of three completed collection rounds. Fail
  or defer according to a policy fixed before the draw, without enabling retries
  selected by a participant after seeing the result.
- Bound subject sizes and include the provider in the consumer's benchmarks.
  Each `random`/`random_after` call performs two BABE storage reads and work linear
  in subject size; there are no new writes. Selection-specific range sampling
  belongs in the consumer and needs an explicit availability/bias tradeoff.

The SDK's additional epoch delay assumes finality does not stall for more than
one epoch. This pallet does not check GRANDPA finality. Applications requiring
finalized results must enforce that separately. BABE randomness also retains its
validator-distribution and honest-participation assumptions: delayed hashing does
not eliminate withholding bias, create entropy in epochs without primary VRF
contributions, or make a single-validator chain resistant to its sole operator.

## Runtime integration

The runtime configures BABE and `impl pallet_randomness::Config for Runtime {}`.
This pallet is composed as `Randomness` at index 7. Network runs at index 24,
after BABE and Session epoch rotation, and Scheduler runs last at index 25.
No storage migration or compatibility path is installed; create fresh genesis
specifications for the unlaunched chain.

Network derives a draw subject from its pallet ID, subnet ID, subnet epoch, and
pool size. It no longer includes the parent hash or the election block number,
and it reduces the entire 256-bit output modulo the pool size. Every nonempty
pool receives an in-range index; only an empty pool returns `None`. This uses
32 bounded arithmetic steps without sampling retries. Under a uniform 256-bit
hash, the statistical distance from uniform is less than `2^-224` for any `u32`
pool size. This negligible arithmetic bias is distinct from validator influence
or publicly predictable input selection. Existing election eligibility and
settlement checks can still prevent an election.

**Candidate-pool timing is not yet implemented.** Network still chooses from its
current regular or emergency candidate list and uses the raw FRAME trait API.
The source replacement does not enforce the commitment requirements above, gate
bootstrap output, or pin a retry across BABE epochs. It must not be described as
completing the application's randomness security design.

## Validation

```sh
cargo test -p pallet-randomness --release
cargo check -p pallet-randomness --no-default-features --target wasm32v1-none --release
```

Tests use a separate mock runtime with real sr25519 VRF signatures and BABE epoch
transitions. They check exact VRF-byte incorporation and reproduce the pinned
SDK's partial-segment delay. They also cover genesis availability, secondary-only
epochs (plain and VRF), freshness boundaries, subject separation, epoch stability,
trait compatibility, skipped epochs, and absence of storage writes. Network tests
cover range boundaries, all-zero/all-one hashes, an exhaustive small-domain
distribution check, deterministic draws, and actual bootstrap elections. They do
not simulate the consensus client's slot lottery, block-seal verification, or
GRANDPA finality.

References:
- [Pinned SDK randomness providers](https://github.com/paritytech/polkadot-sdk/blob/polkadot-stable2606-2/substrate/frame/babe/src/randomness.rs)
- [FRAME randomness contract](https://github.com/paritytech/polkadot-sdk/blob/polkadot-stable2606-2/substrate/frame/support/src/traits/randomness.rs)
