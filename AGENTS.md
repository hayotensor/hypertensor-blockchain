# AGENTS.md — Hypertensor network pallet

This guide helps AI agents use and build on the Hypertensor network. It documents
the `Network` pallet: how to query state, construct transactions, operate validator
identities and subnet nodes, manage stake, and build a subnet that reaches
consensus and distributes rewards.

## Scope

Use this guide to build client integrations and subnet applications and to
operate on the network. Follow the workflow for the task: connect and inspect
the target chain, identify the required account role, construct the appropriate
extrinsics, and verify the resulting state.

Runtime defaults and implementation details below describe the code shipped
with this repository. Check the connected network's runtime metadata and current
configuration before applying these examples.

## Source of truth and repository map

For a running chain, inspect its genesis hash, runtime version, metadata, and
current storage before constructing calls. For this checkout, executable Rust
and tests take precedence over comments and older examples. Some comments still
describe former parameter names or simplified lifecycle rules.

| Need | Read |
| --- | --- |
| Extrinsics, argument types, events, errors, storage, registration | [pallets/network/src/lib.rs](pallets/network/src/lib.rs) |
| Complete call/signature/origin inventory | [precompiles/network/coverage.json](precompiles/network/coverage.json); verify against `#[pallet::call]` in `lib.rs` |
| Ownership, key resolution, subnet timing, queues, owner controls | [pallets/network/src/utilities](pallets/network/src/utilities) |
| Staking, shares, withdrawal ledgers, swaps | [pallets/network/src/stake](pallets/network/src/stake) |
| Proposals, attestations, immutable round snapshots | [pallets/network/src/consensus/attestation/subnet_validator.rs](pallets/network/src/consensus/attestation/subnet_validator.rs) |
| Acceptance, rewards, penalties, node progression | [pallets/network/src/bank/rewards.rs](pallets/network/src/bank/rewards.rs) |
| Typed read RPC contract | [pallets/network/rpc/API.md](pallets/network/rpc/API.md) |
| RPC response types | [pallets/network-rpc-types/src/lib.rs](pallets/network-rpc-types/src/lib.rs) |
| Runtime wiring and economics | [runtime/src/lib.rs](runtime/src/lib.rs), [runtime/src/configs](runtime/src/configs), [runtime/src/tokenomics/mod.rs](runtime/src/tokenomics/mod.rs) |
| Executable protocol examples | [pallets/network/src/tests](pallets/network/src/tests) |
| Native signing/client helpers | [native-examples/scripts/helpers.mjs](native-examples/scripts/helpers.mjs) |
| Block-producing validators | [docs/validators.md](docs/validators.md) |

Call names below use the Rust/metadata spelling and omit `origin` from argument
lists. Polkadot-API uses `api.tx.Network.call_name({ argument_name: value })`.
Other SDKs may expose camelCase names; derive their interface from metadata.
Never hardcode call indices or manually assemble a signed extrinsic from this
document.

## Accounts, IDs, and units

| Role | Authority and purpose |
| --- | --- |
| Subnet owner | Exact account in `SubnetOwner(subnet_id)`. Registers/activates the subnet, changes owner settings, transfers ownership, or deactivates. Owning a validator identity does not confer subnet ownership. |
| Network validator identity | `validator_id` registered by a coldkey. Owns nodes across subnets and a validator delegate pool. This registration alone does not produce chain blocks. |
| Validator coldkey | Funds node stake; registers nodes; manages identity, keys, reward splits, and allocation weights. Financial operations require this signer, not merely a hotkey. |
| Validator hotkey | Default operational signing account for the identity's nodes. Must differ from its coldkey. |
| Node hotkey | Optional override in `SubnetNodeIdHotkey(subnet_id, subnet_node_id)`. If absent, the validator hotkey is effective. Consensus uses the effective hotkey. |
| Subnet node | Identified by `(subnet_id, subnet_node_id)` and owned by a validator identity. The `Active` classification makes it a consensus candidate; an elected candidate proposes a particular round. |
| Delegate staker | Any funded signed account owning shares in a subnet or validator pool. It need not register a validator or node. |
| Delegate account | Optional reward beneficiary configured by a validator. Its accrued `DelegateAccountStake` is a separate balance, not pool shares. |
| Overwatch node | Separate whitelisted network assessment role; evaluates subnets with commit/reveal. Ordinary subnet participation does not require this role. |
| Chain validator | NPoS/BABE/GRANDPA operator using `Staking` and `Session`. Follow the validator guide; `Network.register_validator` does not replace that workflow. |

The bundled runtime uses native `AccountId32` accounts, SS58 prefix 42, and TENSOR
with 18 decimals: `1 TENSOR = 1_000_000_000_000_000_000` atomic units. Verify token
properties and address format on the target chain. Q18 proportions
use the same scale: `1e18 = 100%`, `1e17 = 10%`. An amount, a Q18 proportion,
and a pool share count are different quantities even when all are `u128`.

Use integer/BigInt arithmetic. JSON-RPC `u128` values are decimal strings; byte
fields are `0x` hex; accounts are SS58 strings. Native SCALE types follow runtime
metadata instead of JSON-RPC conventions. In Polkadot-API examples, `u128` is
`bigint`, byte fields use `Binary`, and absent options use `undefined`.

IDs are assigned on chain. Read registration events or ownership indexes instead
of predicting counters. Keep subnet IDs, subnet-node IDs, validator IDs, and
Overwatch IDs in separately named variables.

## Connect, read, submit, and verify

For a disposable local chain, from the repository root:

```sh
cargo build --release --locked --features fast-runtime
./target/release/hypertensor-node --dev --base-path /tmp/hypertensor-network-dev
```

Native HTTP and WebSocket RPC normally share port 9944. Set the endpoint
explicitly: the existing native-example helper defaults to port 9945. The
`fast-runtime` feature changes NPoS session/era timing; do not use those timings
for network-pallet epochs. The current runtime sets a network epoch to 20 blocks
and targets 6-second blocks. Read runtime constants and subnet status instead of
scheduling by wall-clock minutes.

For every state-changing operation:

1. Confirm the chain and signer, read the relevant entity and current parameters,
   and verify authorization. Check both network pause state and runtime call
   filtering when a call is unexpectedly unavailable.
2. Keep enough spendable native balance for the amount, any registration burn,
   transaction fees, and account preservation. Free balance alone can include
   funds unavailable because of locks/freezes. Fund operational hotkeys for fees.
3. Construct the call from live metadata. Set explicit spending ceilings and
   positive minimum outputs for delegation. Serialize submissions per signing
   account, or coordinate nonces between workers.
4. Submit with the authorized signer, retain the transaction hash, and wait for
   finalization. Inspect dispatch errors and any inner batch/proxy result.
5. Read resulting state and relevant events. `ExtrinsicSuccess` does not always
   mean the requested business outcome occurred: activation can remove a subnet,
   and pending-removal cleanup can consume a node operation.
6. On timeout, reconcile the transaction and state before retrying. A timeout
   does not cancel a submitted transaction. Persist progress so restarts do not
   repeat registrations, deposits, proposals, or withdrawals.

Load signing keys from the operator's configured signer or secret store. Keep
seeds, mnemonics, keystores, and secret salts out of source and logs. Public
development keys are suitable only for disposable development chains.

The repository already pins a native client in `native-examples/package.json`
(Node.js 22+; `npm ci` in that directory). Its `nativeWallet`, `withChain`, and
`confirm` helpers provide sr25519 signing, metadata access, and dispatch/finality
checks. Direct `Network` calls require no Revive account mapping or contract.

This example is the body of an optional script placed beside `helpers.mjs`;
configure `NATIVE_SURI` privately and `NATIVE_RPC_URL` for the intended chain.
`VALIDATOR_HOTKEY` is a public SS58 address. The example registers an identity
with a zero validator-pool reward allocation; choose the actual reward policy
before operating a subnet.

```js
import { nativeWallet, withChain, confirm } from "./helpers.mjs";

const wallet = await nativeWallet(); // Validator coldkey.
const hotkey = process.env.VALIDATOR_HOTKEY;
if (!hotkey || hotkey === wallet.address) throw new Error("Set a distinct hotkey");

await withChain(async (api) => {
  let validatorId = await api.query.Network.ColdkeyValidatorId.getValue(wallet.address);
  if (validatorId === undefined) {
    await confirm(api.tx.Network.register_validator({
      hotkey,
      delegate_reward_rate: 0n,
      delegate_account: undefined,
      identity: undefined,
    }), wallet.signer);
    validatorId = await api.query.Network.ColdkeyValidatorId.getValue(wallet.address);
  }
  if (validatorId === undefined) throw new Error("Validator identity not found");
  console.log({ validatorId });
});
```

`ValidatorRegistered` reports the assigned ID, coldkey, hotkey, reward settings,
and registration burn. Resolve or verify the ID with `ColdkeyValidatorId` or
`network_getValidatorByColdkey` after finalization.
For another call, use the exact argument names; for example, an authorized
node-stake top-up is:

```js
await confirm(api.tx.Network.add_node_stake({
  subnet_id: subnetId,
  subnet_node_id: subnetNodeId,
  stake_to_be_added: amountAtomic, // bigint, in native atomic units.
}), coldkeyWallet.signer);
```

### Read API and consistent snapshots

Use the typed `network_*` JSON-RPC methods for summaries, and metadata-backed
storage queries for configuration, queues, user positions, and unbondings.
Custom network RPC defaults to the **best** block. For a consistent or finalized
view, obtain `chain_getFinalizedHead` and pass that hash as the last argument to
every request and every page. JSON-RPC errors and missing entities are different:
point lookups may return `null`; a missing parent for a paged method is an error.

| Task | RPC/storage |
| --- | --- |
| Find subnet and its requirements | `network_getSubnetInfo(subnetId, at)`, `network_getSubnets(page, at)` |
| Find validator identity and ownership | `network_getValidatorByColdkey(account, at)`, `network_getValidatorByHotkey(account, at)`, `network_getValidatorInfo(validatorId, at)` |
| Find node, including a queued node | `network_getSubnetNodeInfo(subnetId, nodeId, at)` |
| Enumerate admitted nodes / endpoints | `network_getSubnetNodes(subnetId, page, at)`, `network_getBootnodes(subnetId, at)` |
| Inspect validator's nodes, stake, allocations | `network_getValidatorNodes`, `network_getValidatorNodeStakes`, `network_getValidatorNodeAllocations` with `(validatorId, page, at)` |
| Decide whether to propose/attest | `network_getSubnetEpochStatus(subnetId, at)`, `network_getConsensusRound(subnetId, subnetEpoch, at)` |
| Inspect current validator candidates | `network_getSubnetValidatorNodes(subnetId, page, at)`; this is not a list of attestations |
| Inspect queue / bootstrap whitelist | `QueuedSubnetNodesData`, `SubnetNodeQueue`, `NodeRegistrationInitialValidatorIds`, `InitialValidatorData` |
| Inspect withdrawable positions | Pool share/generation storage, `NodeSubnetStake`, `StakeUnbondingLedger`, `QueuedSwapRefundBalance` |

Pages use `{ "cursor": null, "limit": 50 }` with limits 1–100. Reuse the opaque
`nextCursor` until it is `null`. `network_getSubnetNodes` excludes the registration
queue. There is no RPC that enumerates every validator or every account's
delegations; track the IDs used by the application and use storage or an indexer
for discovery. An archive node may be necessary for older block hashes.

## Register validator identities and nodes

1. Fund separate coldkey and hotkey accounts. The coldkey signs
   `register_validator(hotkey, delegate_reward_rate, delegate_account, identity)`.
   Each successful identity registration burns the flat
   `Network.Economics.validator_registration_burn` amount from the coldkey
   (1 TENSOR at the configured launch supply), in addition to transaction fees.
   Keep enough unlocked balance to pay the burn and preserve the account. This
   fee is separate from subnet-node registration burns and direct node stake.
   The rate is the Q18 share allocated to validator-pool delegates, bounded by
   `MaxDelegateStakePercentage` and Q18. Optional `delegate_account` has
   `{ account_id, rate }`; use `None` if no beneficiary is intended. Read the new
   `validator_id` from its ownership index.
2. Read the target subnet's state, stake bounds, registration whitelist/queue,
   and current node burn. During subnet registration, the identity must appear
   in `initial_validators` and stay within its node allowance. During enactment,
   new nodes cannot register.
3. The canonical validator coldkey signs:

   ```text
   register_subnet_node(
     validator_id, subnet_id, hotkey: Option<AccountId>,
     peer_info: Option<PeerInfo>, bootnode_peer_info: Option<PeerInfo>,
     client_peer_info: Option<PeerInfo>, stake_to_be_added: u128,
     unique: Option<NetworkBytes>, non_unique: Option<NetworkBytes>,
     max_burn_amount: u128
   )
   ```

   `stake_to_be_added` funds direct node stake. `max_burn_amount` caps a separate,
   dynamic registration burn; it does not cap the stake or transaction fee.
   `None` for `hotkey` uses the validator hotkey. `PeerInfo` is
   `{ peer_id, multiaddr: Option<NetworkBytes> }`. Match actual running endpoints.
   Text PeerIDs are encoded as bytes; multiaddresses must be the binary multiaddr
   encoding, ending in `/p2p/<peer-id>`, not UTF-8 address text. Respect metadata
   length bounds and uniqueness requirements for peer IDs and `unique` data.
   Address validation accepts at most 1,024 bytes in the fixed order
   `ip4|ip6|dns4|dns6|dnsaddr`, optional `tcp|udp`, optional `ws|wss`, then `p2p`.
   DNS and peer payloads must be nonempty; their contents are not validated.
   Repeated/reordered protocols, trailing bytes, and noncanonical varints fail.
4. Read `SubnetNodeRegistered` for the subnet/node IDs, validator ID, coldkey,
   optional hotkey override, initial stake, and actual burn. Verify through
   `ValidatorSubnetNodes(validator_id)` or `network_getValidatorNodes`.
   Initial nodes registered while the subnet is `Registered` fast-track to
   `Active` and emit `SubnetNodeAdmitted`. Nodes joining an active subnet enter
   `Queued`, are admitted as `Probation`, and become `Active` after qualifying
   score-vector inclusions and full reputation.
5. Operate the off-chain service immediately and monitor classification. There
   is no public `activate_subnet_node` extrinsic. Queue maturity, churn limits,
   and available capacity govern admission; accepted-round progression governs promotion.
   Merely waiting a fixed number of blocks does not guarantee validator eligibility.

Queued nodes use a logical queue clock: subtract `TotalNetworkPausedBlocks` from the
chain block, freezing at `NetworkPauseStartBlock` during an ongoing global pause, then
apply the subnet's slot offset and epoch length. Their `classification.start_epoch` is
in this clock. Both admission maturity and removal immunity use its value at the evaluated
subnet slot, so neither ages during a pause or expires midway through a consensus round.
Resume updates one global counter of actual paused blocks and never rewrites queued nodes.
This clock excludes the extra grace epoch used for subnet registration and capacity removal.
Admission replaces the logical timestamp with an ordinary subnet epoch for Probation/Active.

Coldkeys use `update_node_hotkey(subnet_id, subnet_node_id, new_hotkey)` to set
or clear an override, and the `update_node_peer_info`,
`update_node_bootnode_peer_info`, `update_node_client_peer_info`,
`update_node_unique`, and `update_node_non_unique` calls for node metadata.
Resolve current ownership before signing; an old hotkey ceases to be authoritative
after rotation. `update_validator_hotkey` changes the default for nodes without
overrides. `update_validator_coldkey` transfers identity control; it does not
move the old account's wallet balance, delegation positions, or unbonding ledger.
Key changes emit `UpdateColdkey`/`UpdateHotkey` with the validator ID and both keys,
or `NodeHotkeyUpdated` with the node IDs and optional override. A cleared override
is recorded as `None`. `ValidatorDelegateAccountUpdated` reports the complete
resulting beneficiary configuration, including fields retained by partial updates.
Governance registration access changes emit `SubnetRegistrationWhitelistRequired`
and `SubnetRegistrantUpdated` for the affected policy or account/subnet-ID pair.

## Build and launch a subnet

### Off-chain work required

The pallet manages membership, stake, consensus records, and rewards. It does
not host the subnet's model/workload, run its peer network, or generate scores.
Implement the following in the subnet application:

- Peer discovery and authenticated transport using advertised peer information
  and actual running bootnodes.
- Work execution and a reproducible scoring protocol: task selection, validation,
  scoring window, failure handling, and a deterministic mapping to node IDs.
- A consensus worker that reads chain state, computes proposals, independently
  verifies proposals before attesting, and signs with the effective hotkey.
- Persistent round/transaction tracking, key rotation support, reconnection, and
  state reconciliation after finality or runtime changes.
- Monitoring of membership, delegation requirements, quorum, reputation,
  penalties, reward settlement, and operator balances.

Optional `args` and `attest_data` are bounded application-defined bytes; define
their encoding and verification in the subnet protocol. They do not cause the
runtime to execute a custom scoring algorithm. `Attestation` is the implemented
`ConsensusMechanism` in this checkout.

### Registration and activation sequence

1. Register the bootstrap validator identities first. Use at least
   `MinSubnetNodes` distinct IDs in the registration map, each with a positive
   node allowance. Normal consensus needs at least three distinct validator
   identities; many nodes controlled by one identity do not satisfy this.
2. Read `RequireSubnetRegistrationWhitelist`, `SubnetRegistrationWhitelist`,
   `MaxSubnets`, available slots, registration cost state, stake bounds, delegate
   percentage bounds, and registration/enactment lengths. If registration is
   gated, the whitelist must approve `(owner_account, prospective_next_subnet_id)`;
   approval for the address alone is insufficient. Coordinate an authorized
   governance update and recheck the next ID before submission. A regular signer
   cannot grant itself governance authority.
3. Choose an owner account, unique name and repository, meaningful metadata,
   and at least one live bootnode. Build this exact `RegistrationSubnetData`:

   | Field | Meaning |
   | --- | --- |
   | `name`, `repo`, `description`, `misc` | Byte vectors; name/repo must be unique and all fields satisfy bounds |
   | `min_stake`, `max_stake` | Atomic direct-node stake limits within network bounds; minimum ≤ maximum |
   | `delegate_stake_percentage` | Q18 subnet reward allocation to subnet delegates |
   | `initial_validators` | `BTreeMap<validator_id: u32, maximum_node_registrations: u32>`; IDs, not coldkey addresses |
   | `bootnodes` | Nonempty `BTreeMap<PeerId, NetworkBytes>` with valid binary multiaddresses |

4. Owner signs `register_subnet(max_cost, subnet_data)`. The current dynamic
   registration cost must fit the explicit `max_cost`; it is separate from node
   burns and stake. Read the assigned ID from `SubnetRegistered`. Record the
   registration epoch and deadlines immediately.
5. Before registration closes, bootstrap coldkeys register their nodes with
   sufficient direct stake. Bring those nodes and bootnodes online. The owner
   can adjust initial allowances through `owner_add_or_update_initial_validators`
   and `owner_remove_initial_validators` while permitted.
6. Fund the **subnet delegate pool** using `add_subnet_delegate_stake` to meet the
   activation requirement, `MinSubnetDelegateStakeBalance` (initially 100 TENSOR).
   This is one base balance for every subnet, adjustable by collective governance;
   read its current value from `currentMinDelegateStake` in subnet RPC information.
   Maintain sufficient delegation after activation as well.
7. Fund **validator delegate pools** using `add_validator_delegate_stake` and
   verify each identity's node allocations. These determine consensus voting
   weight. Direct node stake and subnet delegate stake do not substitute for
   validator-pool attestor weight. Fund and allocate before the first election
   because each round freezes its voting weights at election.
8. Verify minimum node and distinct-identity counts, delegate balance, subnet
   reputation against `MinSubnetReputation`, service readiness, and timing.
   The exact owner signs `activate_subnet(subnet_id)`.
   Require both `SubnetActivated` and a surviving `Active` subnet record.
9. Read `consensus_eligible_from_subnet_epoch`/RPC status and wait for an actual
   election. Activation schedules consensus eligibility for the next general
   epoch's assigned subnet slot; it does not elect a proposer immediately.

For registration epoch `R`, `SubnetRegistrationEpochs = L`, and
`SubnetEnactmentEpochs = E`, registration lasts through global epoch `R + L`
inclusive. Enactment is `R + L < epoch <= R + L + E`. Activation also requires
`epoch >= R + MinSubnetRegistrationEpochs`. During enactment, delegation can
finish funding the subnet, but new node registrations are blocked.

**Activation is not a harmless readiness probe.** During registration, unmet
requirements return an error. During enactment, a delegate-stake shortfall alone
returns `SubnetActivationConditionsNotMetYet`, preserving the subnet for funding
and retry before the deadline. Insufficient reputation or node count still removes
the subnet and returns dispatch success, even if delegation is also insufficient.
After enactment expires, activation can also remove it. Periodic maintenance can
remove expired or underqualified subnets. Read the state before calling and verify
the result afterward.

### Consensus worker

Subnet epochs are offset by the subnet's assigned slot. Use
`network_getSubnetEpochStatus` (`timing`, `withinProposalAttestationWindow`,
`electedValidatorSubnetNodeId`, `proposalSubmitted`) and
`network_getConsensusRound`; never infer the current round solely from
`block / EpochLength` or NPoS eras. With slot `s` and epoch length `L`, local
round `e` occupies `[s + e*L, s + (e+1)*L)`. The assigned-slot block performs
maintenance and can accept its newly elected round afterward. The RPC window
flag is a timing hint, not a dispatch guard; actual election/proposal state and
the call's checks determine whether it can execute.

For each open round:

1. Read its election snapshot and policy. Only the elected node's effective
   hotkey proposes. Compute scores from the subnet's off-chain protocol.
2. Submit once using:

   ```text
   propose_attestation(
     subnet_id,
     data: Vec<{ subnet_node_id: u32, score: u128 }>,
     prioritize_queue_node_id: Option<u32>, remove_queue_node_id: Option<u32>,
     args: Option<ValidatorArgs>, attest_data: Option<ValidatorArgs>
   )
   ```

   Scores are relative unsigned weights, normalized by the protocol; they need
   not sum to Q18. Avoid overflow. Submit unique IDs with justified scores.
   The runtime filters scores to eligible `Probation`-or-higher nodes and excludes
   pending removals. Duplicate IDs collapse to their lowest score. Queued nodes
   cannot be made score-eligible by listing them. After filtering and deduplication,
   each node's Q18 weight `floor(score * 1e18 / total_score)` must be at most
   `MaxSubnetNodeConsensusWeight` (50% by default), or the proposal fails with
   `MaxSubnetNodeConsensusWeightExceeded`. Scores remain in their submitted units;
   exactly the cap is allowed, and empty/all-zero vectors have zero weight.
   The supermajority collective can change the network-wide cap with
   `set_max_subnet_node_consensus_weight(value)`, where `0 < value <= 1e18`.
   Changes apply to subsequent submissions, including already elected rounds,
   without altering accepted proposals. Usually leave queue actions
   `None`; use them only when the subnet's rules justify prioritization/removal.
3. Each eligible attestor independently checks the recorded proposal and signs
   `attest(subnet_id, subnet_node_id, data)` using that node's effective hotkey.
   The proposer is automatically recorded as an attestor. Do not submit a
   duplicate attestation. The call has no epoch, proposal hash, or negative-vote
   argument: it endorses the proposal in the current subnet epoch. Re-read near
   an epoch boundary before signing or retrying. An identity with multiple
   eligible nodes may have multiple node attestations, but counts once for
   identity quorum.
4. Track both stake-weighted and distinct-identity quorum using the round's
   snapshotted policy and eligible-attestor set. At least three eligible
   identities are required: three need two attesting identities; larger sets
   need `max(3, ceil(N * identity_percentage / Q18))`, capped at `N`.
   Zero total validator-delegate weight produces a zero weighted attestation
   ratio even if everyone signs. One identity with many nodes cannot bypass
   the identity requirement.
5. Observe settlement and `SubnetRewards`/penalty events before treating the
   round as successful. Extrinsic acceptance does not imply round acceptance.
   Settlement is performed by hooks; there is no subnet `claim_rewards` call.

Failed quorum or a missing elected proposal retains the full scheduled subnet
allocation in `SubnetRewardCapacitor`, saturating at `u128::MAX`. Accepted nonzero-score
rounds supplement their normal allocation by `SubnetRewardCapacitorReleasePercentage`
(10% by default), drawn from that balance and limited by reward-budget capacity.
The supermajority-collective call `set_subnet_reward_capacitor_release_percentage(value)`
sets a Q18 percentage from 0% to 100%, effective at the next settlement; zero disables releases.
The supplement uses the round's reward split. Zero-score/zero-allocation rounds leave it stored;
permanent subnet removal clears it. See [REWARDS.md](pallets/network/REWARDS.md).

Global `Network.unpause` starts fresh consensus: proposals and attestations for rounds elected
before or in the resume block are rejected immediately. Each assigned slot settles only the
preceding subnet epoch. Cancellation retains that round's recorded allocation in its capacitor
and applies no rewards, penalties, or probation progression. Older skipped rounds are not
retried and expire with consensus history; skipped allocations are not recreated.
`ConsensusRoundCancelled` reports processed cancellations. Unusable election, voting, or proposal
evidence also cancels the processed round and retains its recorded allocation. Per-round
settlement status prevents duplicate outcomes but never blocks a new election or subnet removal.
Missing net-flow records restart without invented history; reward credits never count as inflow. Epoch-status RPC hides
globally invalidated work. Global resume shifts each pending `SubnetRegistrationEpoch` forward
by `resume_epoch - pause_epoch + 1`. The extra epoch protects partial-epoch time and is granted
even for a same-block pause/unpause. The shared anchor shifts the minimum activation time and
both registration/enactment deadlines; the extra grace can move a subnet back into registration
or restore an activation window that was only one epoch past expiry before the pause.
The same adjustment advances `PrevSubnetActivationEpoch`, preserving the activation cooldown
before capacity-based subnet removal; the global removal-check cadence stays unchanged.
Repeated pause calls retain the original pause start; unpause while running adds no extension.
Re-read registration anchors after resume.

Consensus-history deletion runs only in `on_idle`, after financial retries and node-metadata
cleanup. Live subnets retain at least 128 completed subnet epochs plus the current epoch.
Pruning and removed-subnet cleanup rotate through persistent cursors with bounded work and
execution-time/proof-size budgets. Removed subnets have individual cleanup records, so processing
does not scan or decode the entire backlog. Retention is best effort: without idle block weight,
history can accumulate beyond the retention window without delaying consensus.

Subnet reward allocation combines the delegate-balance EMA, elected-node count, and `net_flow`.
The net-flow factor uses a signed EMA of external subnet-pool deposits minus withdrawals per
completed general epoch; it does not divide by pool balance. Rewards and same-pool share transfers
are excluded. Outgoing swaps count at debit and incoming swaps at successful credit. Softmax
with `SubnetNetFlowK` (default `500`) gives the least negative EMA the highest flow share even
when all subnets have outflows. `SubnetNetFlowSmoothingAlpha` defaults to Q18
`66_967_008_463_192_584` (about ten epochs of half-life). Majority-collective calls
`set_subnet_net_flow_smoothing_alpha(value)` and `set_subnet_net_flow_k(value)` configure
`0 < alpha <= 1e18` and positive `K`. K is expressed in whole native units; stake is converted
from its atomic denomination for this calculation. Alpha changes complete past epochs under
the old policy and apply to the currently open epoch when it completes. See
[pallets/network/REWARDS.md](pallets/network/REWARDS.md).

Election candidates, policy, and all attestor weights freeze together at the
subnet's assigned-slot election, before that block's extrinsics. Delegation,
allocation, and node-count changes after election affect voting power only in future rounds.
`ConsensusValidatorElected` identifies each newly stored round and proposer.
`SubnetEmissionWeightsFinalized` identifies the global epoch's stored allocation,
budget, and subnet count, including an empty allocation. `NodeBurnRateUpdated`
reports automatic registration-rate changes with the previous and new rates.
Proposal submission excludes currently quarantined nodes from effective quorum,
using the surviving nodes' original frozen weights and their sum; it does not
recompute weights or alter the election snapshot. Later quarantine does not
rewrite the proposal's recorded eligibility or quorum denominator.
Missing proposals or insufficient agreement can affect
reputation and slash only the proposer's direct node stake. Validator delegate pools and
other nodes owned by the same identity are unaffected. Use the recorded round policy
to calculate the node's exposure and thresholds.

Admitted newcomers are Probation nodes: they occupy capacity and can appear in
scores, but cannot propose, attest, or receive node rewards or associated delegate
allocations. `ProbationRounds` defaults to two qualifying inclusions and promotion
also requires full reputation. Only ordinary accepted rounds with a nonzero total
score and distinct-identity supermajority update `SubnetNodeProbationProgress`:
inclusion increments it and omission resets it. A zero-score entry can count when
the round total is nonzero. Other rounds freeze progress.
Quarantined nodes cannot progress. Promotion emits `SubnetNodePromoted`, clears the
counter, and enables the next election, including one later in the same hook; it
never grants rewards for the completed Probation round. Owner changes use
`owner_update_probation_rounds(subnet_id, value)` and take effect through the
existing delayed, snapshotted policy. `TotalAdmittedSubnetNodes` counts Probation
and Active nodes; election counters are separate. Inspect
[bank/rewards.rs](pallets/network/src/bank/rewards.rs) for settlement behavior.

## Staking, delegation, swaps, and exits

### Choose the correct position

| Position | Add / adjust | Withdraw |
| --- | --- | --- |
| Direct node stake | Coldkey: `add_node_stake(subnet_id, subnet_node_id, stake_to_be_added)` | Coldkey: `remove_node_stake(subnet_id, subnet_node_id, stake_to_be_removed)`; amounts are atomic balances |
| Subnet delegate pool | Any account: `add_subnet_delegate_stake(subnet_id, stake_to_be_added, min_shares_out)` | Share owner: `remove_delegate_stake(subnet_id, shares_to_be_removed, min_balance_out)` |
| Validator delegate pool | Any account: `add_validator_delegate_stake(validator_id, delegate_stake_to_be_added, min_shares_out)` | Share owner: `remove_validator_delegate_stake(validator_id, validator_delegate_stake_shares_to_be_removed, min_balance_out)` |
| Reward beneficiary balance | Accrues according to validator `delegate_account` configuration | Beneficiary: `remove_delegate_account_balance(amount_to_remove)` |
| Overwatch direct stake | Owner coldkey: `add_overwatch_node_stake(overwatch_node_id, stake_to_be_added)` | Owner coldkey: `remove_overwatch_node_stake(overwatch_node_id, stake_to_be_removed)` |

Successful direct-node stake changes emit `StakeAdded`/`StakeRemoved` with
`subnet_id`, `subnet_node_id`, `coldkey`, and the atomic `amount`. Overwatch uses
`OverwatchNodeStakeAdded`/`OverwatchNodeStakeRemoved` with `overwatch_node_id`,
`coldkey`, and `amount`. Removal events record movement into the unbonding ledger;
wallet credit requires claiming after maturity.

Pool deposits take **balances**; pool withdrawals, transfers, and swaps take
**shares**. `swap_from_validator_to_validator` misleadingly names its source
argument `stake_to_be_removed`; it is also shares. Both `min_shares_out` and
`min_balance_out` must be positive. Do not use zero to disable slippage checks.
Read the current share generation and position, calculate the expected output,
and apply the configured slippage tolerance using integers.

Share conversion has virtual-balance/share offsets and minimum locked liquidity;
one share is not one atomic TENSOR unit. Use the conversion logic in
[staking_utils.rs](pallets/network/src/stake/staking_utils.rs), including rounding
and first-deposit behavior. For balance `B`, total shares `S`, source shares `s`,
and deposit amount `a`, the current integer quotes are:

```text
redeem_balance = floor(s * (B + 1) / (S + 1_000_000_000))
gross_deposit_shares = floor(a * (S + 1_000_000_000) / (B + 1))
```

On an empty pool, subtract `1_000_000_000` locked shares from the gross deposit
quote to obtain user shares. Deposits must mint at least `1_000_000_000` user
shares and satisfy `MinDelegateStakeDeposit`. Share transfers also enforce a
minimum balance value. Quotes do not reserve the exchange rate until execution.
Rewards increase pool backing rather than the holder's share count; losses
decrease its value. A pool reset invalidates old
generation shares. Raw share storage without its generation is insufficient to
establish a spendable position.

For subnet positions, compare `AccountSubnetDelegateStakeGeneration(account,
subnet_id)` with `SubnetDelegatePoolGeneration(subnet_id)`; a mismatch means
effective shares are zero. Apply the corresponding account/pool generation
check for validator positions. For direct node balances, the storage key order
is `NodeSubnetStake(subnet_node_id, subnet_id)`, unlike most subnet-node maps.

### Validator pool allocation and reward settings

Coldkey signs `set_validator_node_delegate_stake_weights(updates)`, where
`updates` is `Vec<(subnet_id, subnet_node_id, Q18_weight)>`. There is no
`validator_id` argument; it is derived from the signing coldkey. List **every
owned node across all subnets**, including queued nodes, exactly once. Weights
must sum to Q18, and each must be at most Q18. Observe
`ValidatorNodeDelegateStakeWeightUpdateInterval` before updating again.

The first node defaults to 100% allocation and later nodes to zero. Reallocate
deliberately when adding nodes. Attestor weights derive from the validator pool
balance times node allocation, then apply the snapshotted decay/power policy;
direct self-stake is not the voting-weight source.

`update_validator_delegate_reward_rate` adjusts the share of node rewards going
to the validator delegate pool; observe its update period and decrease limits.
`update_validator_delegate_account` manages a separate beneficiary/rate. Do not
assume `None, None` clears it: the current implementation rejects that input.
Use the allowed combinations in [utilities/validator.rs](pallets/network/src/utilities/validator.rs).
Node rewards compound into direct node stake after the validator-pool allocation
and then the beneficiary allocation. To obtain spendable funds, follow the
corresponding stake/balance removal and unbonding claim path.

### Remove stake and claim

1. Read the position, current ownership, pending node removals, applicable minimums,
   and `StakeUnbondingLedger(account)` capacity. Election creates no stake withdrawal lock.
   Direct stake cannot fall below the enforced node minimum while the relevant live-node
   constraints apply. Failed slashes reserve their fixed unpaid amount in
   `PendingNodeSlashReservations(subnet_id, node_id)` until paid or expired; withdrawals ignore
   expired reservations even before idle cleanup. A new slash is capped by direct stake remaining
   after active reservations. The global `PendingConsensusOperations` queue retries financial
   effects without holding consensus open or replaying reputation changes.
2. Submit the corresponding removal. To fully exit an admitted node, request
   `remove_subnet_node(subnet_id, subnet_node_id)` with its coldkey, wait for any
   deferred cleanup/settlement, then remove the remaining direct stake. A live
   subnet's currently elected proposer cannot self-remove
   (`ElectedValidatorCannotRemove`); finish its duties and wait for eligibility
   to exit. Node deregistration does not itself return stake to the wallet.
3. Read the unbonding ledger after finalization. Entries are keyed by an exact
   claim block, with separate `network` and `overwatch` amounts. The removal
   snapshots a cooldown in blocks; do not recalculate an existing entry after a
   governance change.
4. At `current_block >= claim_block`, that ledger's account signs
   `claim_unbondings()` with no arguments. Verify the matured entries were
   removed and the wallet credited. Future entries remain. Removal calls do not
   automatically claim mature entries; claim first if `MaxUnbondings` is full.

Each successfully paid ledger entry emits `UnbondingClaimed` with its claim block
and separate network/Overwatch amounts. A paid queued-swap refund emits
`SwapRefundClaimed`. Failed or immature entries remain stored and emit no claim
event, so one call can report only a subset of the account's liabilities.

Read `StakeCooldownEpochs`, `DelegateStakeCooldownEpochs`, and
`NodeDelegateStakeCooldownEpochs` for direct/beneficiary/Overwatch, subnet-pool,
and validator-pool cooldowns, respectively; multiply by `EpochLength` for blocks.
Network unbonding uses this ledger, not `Staking.withdraw_unbonded`.
Some node operations first clear a pending-removal marker and return success
without applying the requested top-up/withdrawal/update. Re-read the node and
position, then decide whether the intended operation is still appropriate.

Node removal clears live membership, election/queue lists, reputation,
counts, and validator allocations immediately. It preserves `SubnetNodeValidatorId` and
`NodeSubnetStake` for future coldkey withdrawals. Metadata and its unique/peer/address
reverse indexes, hotkey overrides, probation progress, and the removed node's `NodeElectionSlotIndex`
wait in the bounded `NodeMetadataCleanupQueue` for `on_idle`; unique/peer/address values
remain reserved until cleanup. A surviving node moved in the election list has its slot index
updated immediately. A full queue falls back to synchronous residual cleanup.
Whole-subnet deletion drains its residual entries with separately reserved weight. Deferred
cleanup never changes stake, ownership, membership counts, or historical consensus rounds.

### Transfer shares or swap pools

Share transfers stay in the same pool:

- `transfer_delegate_stake(subnet_id, to_account_id, delegate_stake_shares_to_transfer)`
- `transfer_validator_delegate_stake(validator_id, to_account_id, validator_delegate_stake_shares_to_transfer)`

`SubnetDelegateStakeTransferred` and `ValidatorDelegateStakeTransferred` identify
the pool, sender, recipient, and transferred shares; these are ownership transfers
and do not mint shares or credit wallet balances.

Cross-pool movement uses `swap_from_subnet_to_subnet`,
`swap_from_validator_to_validator`, `swap_from_validator_to_subnet`, or
`swap_from_subnet_to_validator`. Read exact source-share argument names from the
call inventory. Every swap includes `min_balance_out`, `min_shares_out`, and
`execute_before_running_block`.

Swap deadlines use the shared running-block clock, not chain block numbers. Compute
`min(block, NetworkPauseStartBlock if present) - TotalNetworkPausedBlocks` with saturating
subtraction, reading all values at the same block. Contracts can use
`IStaking.getRunningBlock()`. Set `execute_before_running_block` to that clock plus
the desired window, allowing at least the source cooldown. Stored swap timestamps and
events use this clock too; `queued_at_running_block` records when the swap entered it.
Global pauses freeze both the cooldown and the deadline without rewriting queue entries.
Updates remain callable while paused and validate against the frozen clock. Deadlines
are inclusive; only running blocks after the deadline expire a swap. Pauses grant no
extra grace and do not automatically revive swaps that were already expired.

Swaps withdraw source shares into queued principal; destination shares arrive
later. The queue snapshots the source cooldown in running blocks with a one-epoch minimum.
Inspect `SwapCallQueue`, queue events, and the eventual destination balance.
Each swap also emits its direction-specific event:
`SubnetToSubnetQueuedSwapCall`, `SubnetToValidatorQueuedSwapCall`,
`ValidatorToSubnetQueuedSwapCall`, or `ValidatorToValidatorQueuedSwapCall`.
These record the source/destination IDs, caller, actual redeemed `balance`, and
`shares_burned`. Their `id` matches `SwapCallQueued` and the later
`SwapCallQueueUpdated`, `SwapCallCredited`, or `SwapCallRefunded` events.
Choose a deadline that allows maturation and execution. `update_swap_queue(id,
new_call)` allows permitted owner changes; it is not an arbitrary withdrawal
or cancellation mechanism. Failed/expired execution credits principal to
`QueuedSwapRefundBalance`; call `claim_unbondings()` to receive that refund in
the wallet. Source shares are not restored. Do not count queued or refundable
principal as destination stake.

## Owner controls

- `owner_deactivate_subnet` permanently removes the subnet, including when a consensus round
  has not settled. Inspect retained
  stake/delegation positions and their withdrawal paths after removal.
- Ownership transfer is two steps: current owner calls
  `transfer_subnet_ownership(subnet_id, new_owner)`, then the designated account
  calls `accept_subnet_ownership(subnet_id)`. Current owner can call
  `cancel_subnet_ownership_transfer(subnet_id)` while pending.
- `update_bootnodes(subnet_id, add, remove)` manages advertised bootnodes;
  `owner_add_bootnode_access`/`owner_remove_bootnode_access` grant/revoke its
  permitted callers. Changing metadata does not start a peer service.
- Owner settings cover metadata, churn, queue/classification durations, stake
  limits, delegate reward percentage, registration burn policy, reputation, and
  consensus weighting. See the `owner_update_*` calls in the inventory. Many
  changes are scheduled for a later subnet epoch and have cooldowns; inspect
  pending values/effective epochs as well as active values.
- Global pause, whitelist management, forced removals, and network-wide `set_*`
  policy calls generally require configured collective origins. Read each
  extrinsic's origin guard. A funded coldkey or subnet owner does not acquire
  those permissions.

## Optional Overwatch participation

The registered `validator_id` must be whitelisted in `OverwatchValidatorWhitelist`;
its canonical coldkey signs registration.
Call `register_overwatch_node(stake_to_be_added)` after Overwatch epoch zero,
with sufficient stake and available membership capacity. An identity may have
one active Overwatch node. Read its ID and initial stake from `OverwatchNodeRegistered`
and verify through `ValidatorOverwatchNodeId`.
`update_overwatch_hotkey` sets an optional override; otherwise its validator
hotkey is used. `set_overwatch_node_peer_id` advertises a subnet-specific peer.
They emit `OverwatchNodeHotkeyUpdated` and `OverwatchNodePeerIdUpdated` with the
resulting override or peer. Voluntary exit emits `OverwatchNodeRemoved`, identifying
the node and validator while preserving historical ownership and residual stake.

Use active Overwatch epoch/phase storage, not subnet epoch timing. A node joining
mid-epoch must wait for membership in the next epoch-opening snapshot. During
commit, its effective hotkey calls `commit_overwatch_subnet_weights` with
`{ subnet_id, weight: commitment_hash }` entries. During reveal, it calls
`reveal_overwatch_subnet_weights` with `{ subnet_id, weight: u128, salt }` entries.
Each node may submit one successful commit batch and one successful reveal batch
per epoch. Commit batches reject unknown or duplicate subnet IDs. Reveals must
contain every committed subnet exactly once, including removed subnets, with
weights in `0..=Q18`. The node may choose which subnets to assess before committing;
its accepted commitment set cannot be extended or replaced. Invalid batches write
nothing and may be corrected and retried. Successful native calls return `Pays::No`;
failed calls pay normal fees, and precompile calls still pay contract gas.
Hotkey rotation and pause/resume do not reset successful submissions. Reveals use
the stored commitment set and hard batch bound, so lowering `MaxSubnets` after
commit does not prevent completion. Persist the full batch and secret salts before
committing, and reconcile finalized state before retrying an uncertain submission.
`OverwatchWeightsCommitted` contains the complete accepted commitment batch.
`OverwatchWeightsRevealed` contains the complete verified subnet weights without
salts. Both identify the Overwatch node and epoch; failed batches emit neither.
Commitment hashing is the runtime hash of this SCALE tuple:

```text
(fixed_bytes("overwatch/subnet-weight/v1"), overwatch_node_id: u32,
 subnet_id: u32, overwatch_epoch: u32, weight: u128, salt: Vec<u8>)
```

The current runtime uses Blake2b-256. The fixed tag has no length prefix. Use
fresh secret random salts and the exact codec; Solidity ABI/Keccak is incorrect.
Every accepted commitment must be revealed on time for any of that epoch's
assessments or rewards to count. Removal does not cancel existing commitments.
See [docs/overwatch-commit-reveal.md](docs/overwatch-commit-reveal.md) for the test
vector and [docs/overwatch-rewards.md](docs/overwatch-rewards.md) for settlement.
Finish outstanding reveals before calling `remove_overwatch_node`: exit ends
submission authority immediately while preserving the commitment obligations.
Then use the stake removal/claim path. Voluntary removal also removes the
validator's Overwatch whitelist approval; rejoining needs a fresh approval.

## Acting on behalf of another account and contract callers

Calls authenticate the signing account: use the canonical coldkey, effective
operational hotkey, or a runtime proxy authorized for the account and call.
Passing another account's coldkey as a call argument does not grant its authority.

Inspect `ProxyType` in [runtime/src/configs/common.rs](runtime/src/configs/common.rs).
`SubNetworkStaking` and `SubNetworkDelegateStaking` are narrow filters; neither
includes registration, consensus, key updates, or `claim_unbondings`.
`NonTransfer` excludes Network calls. Validate the exact call against the proxy
filter and inspect the inner dispatch result after execution.

For smart-contract integrations, use the typed Revive precompiles
documented in [precompiles/network/README.md](precompiles/network/README.md) and
the matching Solidity interfaces. Native account arguments are `bytes32`, while
contract addresses are 20 bytes. Authorization is based on the immediate mapped
caller; a wrapper contract does not inherit the external signer's coldkey rights.
Use the Revive system's `toAccountId(address)` mapping rather than padding a
20-byte address. Precompiles require no deployment and their methods are
nonpayable: fund the mapped caller first and send no `msg.value`. Native Network
events are authoritative; they are not mirrored into Ethereum receipt logs.

## Troubleshooting and validation

| Symptom | Check before retrying |
| --- | --- |
| `NotKeyOwner` or operational authorization failure | Canonical coldkey mapping, node/Overwatch override, and effective hotkey |
| Registration rejected | Whitelist, phase, allowance, queue/capacity, stake bounds, peer encoding, burn ceiling |
| Successful activation but missing subnet | Activation removal branch and `SubnetDeactivated` reason |
| No proposer or no accepted rounds | Eligibility slot, three distinct identities, Active classification, nonzero delegated voting weight |
| Attestation rejected | Epoch rollover, current proposal, effective signer, proposal snapshot membership, duplicate attestation |
| Cannot withdraw/deposit/swap | Live-node minimum, shares versus balance, current generation, positive output bounds, rate limit, ledger capacity |
| Withdrawal succeeded but wallet unchanged | Ledger maturity and explicit `claim_unbondings`; deferred refund credit |
| Node call succeeded without intended change | Pending-removal cleanup branch; re-read before any resubmission |
| Node remains Queued/Probation | Churn/queue settings, successful settlement counters, inclusion, identity quorum, reputation |

`TxRateLimit` checks generally require more than the configured number of blocks
since the relevant previous transaction, not merely equality. Decode actual
module errors with metadata; do not match human text or retry indefinitely.

For code changes, use the pinned toolchain in `rust-toolchain.toml` and run tests
that exercise the changed behavior. Useful commands from the repository root:

```sh
cargo fmt --all -- --check
cargo test --locked -p pallet-network
cargo test --locked -p pallet-network tests::subnet
cargo test --locked -p pallet-network tests::validator_delegate_staking
cargo test --locked -p pallet-network tests::unbonding
```

Use `registration_queue`, `queue_maturity`, `pause_queue`, `pending_removals`,
`incentives_protocol`, `emissions_stake_weight`, `rpc`, and Overwatch test modules
when changing those paths. Run relevant runtime/precompile tests when changing
origins, call encoding, account mapping, or proxy filters. A documentation-only
change needs source/link/example validation, not a full blockchain build.

Before calling a subnet operational, verify registration and activation, actual
peer/work execution, positive weighted and identity quorum, a settled successful
round with expected reward positions, and a complete remove → mature → claim
cycle on a disposable chain. Keep this guide synchronized with the interfaces,
runtime configuration, and protocol behavior implemented in this repository.
