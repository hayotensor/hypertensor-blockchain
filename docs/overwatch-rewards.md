# Overwatch rewards

Overwatch nodes earn rewards for agreement with the aggregate assessment only if
they successfully reveal every assessment they committed to during that epoch.
If any commitment remains unrevealed at close, the node receives no epoch reward
and all its assessments are excluded before subnet averages and reward shares
are calculated. The completion rule applies regardless of assessment values or
agreement. A high subnet rating does not give an assessment a larger reward multiplier.

For an eligible node's revealed rating `x` and the subnet's finalized aggregate `W`, both in `[0, 1]`:

```text
assessment_score = 1 - abs(x - W)
node_score = sum(assessment_score for each subnet the node revealed)
reward_share = node_score / sum(eligible node scores)
reward = snapshotted_epoch_budget * reward_share
```

The runtime uses Q18 fixed-point arithmetic and compounds rewards into Overwatch
stake. A node with no valid reveals receives no reward. If all node scores are
zero, no reward is distributed.

For each subnet with positive revealer stake, unanimous ratings of `0`, `0.1`,
`0.9`, and `1` each earn the same agreement score, even when other Overwatch nodes
do not assess that subnet. Nodes with identical ratings and coverage split the
budget equally, even with different stakes. Unanimous zero ratings earn rewards;
they remain explicit zero subnet signals for emission allocation.

Each revealed subnet contributes its agreement score equally. Matching a
low-rated subnet is worth as much as matching a high-rated subnet. Scores are
summed, so coverage also affects total rewards.

Within an epoch, nodes with the same coverage earn larger reward shares when
their total absolute deviation from the finalized stake-weighted aggregates is
smaller. Equal deviations earn equal shares. Fixed-point and token rounding can
make very small score differences result in equal payouts.

Stake is normalized independently for each subnet using only its eligible revealers
and their epoch-opening stake snapshots. The stake exponent is applied within that
group. Stake influences the comparison aggregate, but does not multiply the
evaluator's reward score. Subnets a node never committed to contribute neither
weight nor reward points; an unrevealed commitment excludes that node from every
subnet in the epoch. Explicit zero assessments participate in the average.

For example, with two equally staked nodes, A commits to and reveals X and Y at
`1.0`, while B commits to and reveals only Y at `1.0`. Both subnet aggregates are
`1.0`. A earns two agreement points and B earns one, so they receive two-thirds and one-third of the epoch's
reward pool, subject to rounding. B gets no points for X.

If a subnet has no eligible reveals, it has no aggregate entry and emission allocation
uses the configured whole-subnet fallback modifier. No default assessment is
credited to an evaluator. Rewards measure agreement with the subnet's revealers,
not independently proven service quality; low coverage can still leave a subnet
assessed by very few nodes.

## Epoch-opening stake and exits

Before commits begin, each epoch snapshots every canonical active node's raw stake,
the stake exponent, and its reward budget. Registration during the epoch becomes
eligible at the next boundary. Deposits and withdrawals remain available under the
existing minimum-stake and withdrawal-cooldown rules; they affect later snapshots.

At rollover (global slot 0), the closing epoch keeps its original snapshot and the
next epoch receives a new snapshot. Settlement runs in slot 1. Reward compounding
therefore affects the following snapshot, not the epoch already opened. Pauses and
delayed settlement do not recapture balances. At close, a bounded eligibility set
records opening-cohort nodes whose nonempty commitment and verified reveal sets
contain exactly the same subnet IDs. Settlement requires both snapshots; a missing
record blocks processing, while an explicit empty eligibility set finalizes no
rewards and an empty subnet signal. Subnets without eligible assessments use the
existing fallback modifier. Successful settlement consumes both snapshots and all
reveal rows, including excluded rows.

Voluntary exit ends authentication but preserves commitments for the completion
check. Fully revealed submissions retain reward eligibility. Incomplete submissions
are excluded even after exit or replacement registration. The latest finalized
signal remains unchanged by voluntary exit.
Earned rewards still credit the historical node's stake balance and remain withdrawable.
Only governance disqualification may purge active/pending assessments and recompute
the latest effective signal. It can target an already-exited node without removing
its replacement. Finalized history, credited rewards, and written allocations remain
unchanged.

Final normalized shares depend only on complete, eligible submissions for each
subnet. Missing any reveal forfeits the entire epoch's reward and assessment
influence; it does not slash stake, deregister the node, or impose a new withdrawal
lock. This prevents retaining selected assessments and Overwatch rewards through
partial disclosure. It does not force disclosure or eliminate possible incentives
outside this reward pool.

## Benchmark weights

`.maintain/benchmark-overwatch.sh` runs the bounded rollover, settlement and removal
fixtures against a freshly built runtime with `runtime-benchmarks` enabled. Its
output is a partial weight table. Integrate only the affected methods into
`pallets/network/src/weights.rs`, preserving the component-wise maximum of the
record and dense-subnet settlement models, both voluntary-exit fixtures, and all
five governance-disqualification paths.

Rollover benchmarks include all commitment and reveal rows and the completion
snapshot write. Settlement weights include reading and consuming that snapshot.
The pending header counts all accepted reveal records, including excluded nodes,
so filtering does not undercharge the scan and cleanup. Incomplete submissions
skip aggregation and payment work; their stored rows still get cleaned up.

The completion-rule weights were regenerated on 2026-09-30 with CLI 60.0.0,
50 steps and 20 repeats. At the runtime bounds (64 evaluators and 17 subnets),
rollover reserves 20.351 ms and 231,916 proof bytes; settlement reserves at most
28.534 ms and 189,453 proof bytes, including RocksDB read/write charges. The
runtime's hook-budget test also includes the other work that shares those slots.
