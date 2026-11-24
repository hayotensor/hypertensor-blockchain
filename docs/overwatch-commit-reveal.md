# Overwatch commitments

Commit and reveal require current active-node authentication and membership in the
epoch-opening stake snapshot. A node registered mid-epoch waits until the next epoch.
Every accepted commitment must be successfully revealed before the reveal deadline.
If a node leaves even one commitment unrevealed, it receives no rewards for that
epoch and all its assessments are excluded before subnet averages are calculated.
Each node submits one successful commitment batch and one successful reveal batch
per epoch. A node may choose a subset of subnets, but its reveal must contain every
committed subnet exactly once, in any order. Commit batches reject unknown subnet
IDs and duplicate IDs; reveal batches reject duplicate, missing, or extra IDs.
An invalid batch stores nothing, emits no success event, and can be corrected and
retried. A subnet's removal does not cancel an accepted commitment to it.

Both successful native calls return `Pays::No`; rejected calls pay normal fees.
This fee waiver does not waive contract gas when calling through a precompile.
The stored node/epoch rows prevent repeated successful calls, including after
hotkey rotation or pause/resume. `AlreadyCommitted` and `AlreadyRevealed` report a
consumed submission opportunity. `DuplicateOverwatchSubnetId` rejects duplicate
entries; `RevealSetMismatch` rejects an incomplete or unexpected reveal set.

Commit admission uses the current subnet limit. Reveals use the accepted commitment
set and `MaxPhysicalSubnetsUpperBound`, so a later reduction of `MaxSubnets` cannot
strand a larger batch. The bundled runtime bounds batches at 16 entries and salts
at 64 bytes. Subnets created after the commitment batch must wait for the next epoch.
Maximum signed batch capacity is checked against runtime weight and length limits.

Voluntary exit ends further submissions but preserves commitment records until
epoch close. Fully revealed submissions remain eligible; incomplete submissions
are excluded even after exit or replacement registration. Governance may disqualify
contributions, including after exit. Incompletion does not slash stake or deregister
the node, and does not prevent participation in later epochs.

An Overwatch commitment is the runtime hash of this SCALE-encoded tuple:

```text
(fixed_bytes("overwatch/subnet-weight/v1"), node_id: u32, subnet_id: u32,
 overwatch_epoch: u32, weight: u128, salt: Vec<u8>)
```

The current runtime uses Blake2b-256. The tag is exactly 25 ASCII bytes, with no
length prefix or terminator. The three `u32` fields and the `u128` weight use
little-endian encoding. Salt uses a SCALE compact byte-length prefix followed by
the salt bytes. Solidity ABI encoding and Keccak are not the commitment format.
Rust callers can use `Pallet::hash_overwatch_commitment`.

Read the active Overwatch epoch before constructing the commitment. Use the
Overwatch node ID authenticated by the signing hotkey, and a fresh secret random
salt within `MaxOverwatchRevealSaltLength`. Build and privately persist the complete
set of weights, salts, and commitments before submitting the commitment batch.
Submit all hashes together during commit, then all weights and salts together
during the same epoch's reveal phase. The pallet obtains the node ID from the
authenticated call and the epoch from active round state when verifying the reveal.
On an uncertain transaction result, reconcile finalized events and the node's
commit/reveal row before retrying. A successful submission cannot be amended.

Copying a hash is possible, but opening it under another node, subnet, or epoch
fails with `RevealMismatch`. Node IDs are never reused, and binding the stable ID
allows the owning validator to rotate its hotkey without invalidating its
commitments. Only this bound format is accepted. The extrinsic arguments and
commitment format are unchanged by the completion requirement.

At close, `OverwatchEpochRevealEligibility` records opening-cohort nodes whose
nonempty commitment set exactly matches their verified reveal set. Commitments
are then removed. Settlement requires this eligibility record and the opening
snapshot, filters every assessment through it, and removes the eligibility record
and all reveal rows after successful settlement. A missing eligibility record
blocks settlement; an explicit empty set produces no rewards and no subnet scores.

## Encoding vector

For node `7`, subnet `11`, epoch `13`, weight `123456`, and salt `secret-salt`
(the salt here is only a test fixture):

```text
SCALE preimage: 6f76657277617463682f7375626e65742d7765696768742f7631070000000b0000000d00000040e201000000000000000000000000002c7365637265742d73616c74
Blake2b-256:    4d7785ea48191e23d6ced10ee06e3868053183c32665e98e5df169f6b3fcb49c
```
