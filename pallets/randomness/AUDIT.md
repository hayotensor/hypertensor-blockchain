# Randomness availability audit — 2026-09-21

Scope: the new provider, the pinned BABE implementation, Network's sampling and
election call sites, and runtime/node wiring. This is a source and regression-test
review, not a proof of consensus security or an independent cryptographic audit.

## Result

During normal execution, `random(subject)` always returns a 256-bit runtime hash.
Network's sampler now always returns an in-range index for a nonempty `u32` pool.
Neither property guarantees fresh, unpredictable entropy. The current integration
must not be presented as a fully secured mainnet election protocol.

## Findings and changes

### Fixed: sampling exhaustion could suppress a nonempty draw

Network previously truncated derived hashes to 64 bits and tried rejection
sampling eight times. All eight rejected samples returned `None`, causing the
election to return without selecting anyone. Although extraordinarily unlikely
with the production hash, this contradicted unconditional sample availability.

The sampler now interprets the entire provider hash as a big-endian integer and
reduces it modulo the pool size. It has no retries, allocations, or truncation.
A `NonZeroU32` divisor prevents division by zero; each intermediate is below
`2^40`, so `u64` arithmetic cannot overflow. The result is always less than the
pool size. The production H256 requires exactly 32 loop iterations. Zero hashes
and all-one hashes remain valid inputs. The empty-pool result is still `None`.

Exact uniformity and a guaranteed, finite mapping from a fixed number of uniform
bits cannot both hold for arbitrary non-power-of-two bounds. With a uniform
256-bit input, each index here has either `floor(2^256/n)` or `ceil(2^256/n)`
preimages. Statistical distance from uniform is below `n/2^256 < 2^-224` for
`u32` pool sizes. This is an explicit negligible-bias tradeoff for availability,
not a fallback to block hashes or caller-controlled entropy.

### Open: the pinned BABE accumulator can defer primary VRF incorporation

At SDK commit `72284b37a234f8a1565f2043ddbf8280a69fcf29`, BABE stores primary
contributions in 256-entry segments. `deposit_randomness` increments
`SegmentIndex` only when advancing to another segment. However,
`randomness_change_epoch` consumes `(0..segment_idx)`, which excludes the current
segment, and resets the index to zero. With segment index zero, the new announced
seed hashes only the prior announced seed and epoch index; segment zero remains
buffered. Thus a small primary batch does not necessarily affect the next seed.

Regressions reproduce this with 1, 255, and 256 real primary VRF contributions.
A separate 257-contribution case verifies the exact hash of the first segment's
VRF bytes, then verifies its promotion into the application output. This replaces
the earlier insufficient test that only checked a changed seed and promotion.

This dependency behavior remains unchanged. Correcting it requires an explicit
BABE dependency/protocol change, with consensus and epoch-transition validation;
silently mixing current-block data into the application provider would weaken
the intended delay. Do not assume that a short or sparsely populated epoch has
contributed fresh entropy merely because its seed or cutoff changed.

### Open: Network does not enforce candidate commitment timing

Network uses the raw FRAME API and resolves current regular/emergency candidates
at election time. The delayed source is already public before it is used. Input
changes after the seed becomes knowable can influence the selection. Network
also does not pin a draw across epoch-changing retries or gate bootstrap output.
The three-epoch commitment protocol described in the README remains necessary
before treating this selection as resistant to manipulation.

The circular successor rule for quarantined candidates is also intentionally
not a uniform draw over only eligible candidates: healthy candidates following
quarantined slots inherit those slots' selection probability. This audit changes
sample availability, not that existing eligibility policy.

### Expected limits: availability is not entropy or finality

- Genesis output is deterministic, derived from BABE's initial zero seed.
- Epochs containing only secondary blocks contribute no primary VRF entropy,
  including secondary blocks that carry their own VRF proof.
- `random_after` checks only `committed_at < cutoff`. It deliberately returns
  `None` when that check fails; success does not prove entropy or finality.
- Skipped slot epochs do not prove completed entropy-collection rounds.
- Repeating a subject within an epoch repeats the output. Zero and collisions
  must not be treated as errors or trigger selective retries.
- BABE validator withholding, sole-validator control, and stalled GRANDPA
  finality remain protocol assumptions. Hashing cannot manufacture new entropy.
- An empty pool, missing validator identities, quarantine, pending settlement,
  or exhausted liability counters can still legitimately prevent an election.
- Out-of-memory execution, invalid consensus state, and a halted chain are outside
  the normal-execution guarantee. Network uses a small, fixed-size subject; other
  consumers must bound their inputs and charge the operation's weight.

## Validation

Completed: 13 randomness pallet tests and all 832 Network tests passed. The
runtime's release `no_std` check for `wasm32v1-none` passed, as did scoped
formatting and whitespace checks. Existing unrelated compiler warnings remain.
Passing tests include reproductions of the open findings; they do not close them.

The pallet tests use actual sr25519 VRF signatures and epoch hooks. They cover
exact consumed-contribution hashing, partial-segment behavior, plain/VRF secondary
epochs, genesis and skipped-epoch availability, strict cutoff boundaries, domain
separation, stable reads, FRAME trait compatibility, and absence of storage writes.

Network tests check all-zero/all-one hashes, pool sizes through `u32::MAX`, an
independent U256 remainder calculation, use of all 32 digest bytes, exhaustive
16-bit preimage counts, actual bootstrap elections, round determinism, and
independence from draw-time parent hashes/timestamps. Pending-removal election
tests exercise the existing successor and wraparound policy with the new sampler.

The runtime configures this provider for Network and orders BABE/Session before
Network. The node uses the SDK BABE import queue and block authoring pipeline.
Unit tests do not certify live peer import, slot lotteries, seals, fork choice,
finality under outage, or target-machine weight calibration.

Reproduce:

```sh
cargo test -p pallet-randomness --release --offline -j 2
cargo test -p pallet-network --release --offline -j 2 --lib
cargo check -p hypertensor-runtime --release --offline --no-default-features \
  --features with-rocksdb-weights --target wasm32v1-none -j 2
```

Upstream references:

- [Pinned BABE accumulator](https://github.com/paritytech/polkadot-sdk/blob/72284b37a234f8a1565f2043ddbf8280a69fcf29/substrate/frame/babe/src/lib.rs)
- [Pinned randomness providers](https://github.com/paritytech/polkadot-sdk/blob/72284b37a234f8a1565f2043ddbf8280a69fcf29/substrate/frame/babe/src/randomness.rs)
- [Published BABE source with the same segment logic](https://crates.parity.io/src/pallet_babe/lib.rs.html)
