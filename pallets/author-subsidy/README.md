# Author subsidy

For setup instructions, see [Set your block reward address](SET_REWARD_ADDRESS.md).

This pallet pays block subsidies to verified EVM accounts. Aura authorities remain
sr25519 consensus keys; token balances remain `AccountId20` accounts shared with EVM.
No token account is created from an SS58 address.

## Set a reward destination

Submit the native pallet call `authorSubsidy.setRewardAddress` with:

| Argument | Type | Meaning |
| --- | --- | --- |
| `aura_key` | 32-byte sr25519 public key | A member of the current Aura authority set |
| `reward_address` | `H160` | Receiving EVM account; must be nonzero |
| `nonce` | `u64` | Next ownership-proof nonce for this Aura key |
| `valid_until` | Runtime block number (`u32`) | Last block in which this proof can be accepted, inclusive |
| `aura_signature` | 64-byte sr25519 signature | Signature of the exact payload below |

The receiving EVM account must sign the enclosing transaction and have enough
native balance to pay its fee. This is a **native Substrate pallet extrinsic** using
this chain's `EthereumSignature` scheme. It is not a Solidity call or an
`eth_sendTransaction` request. Use signing tooling that supports this runtime's
native extrinsics. No new precompile, RPC, or node command is introduced.

1. Read the chain's genesis block hash, current block number, and
   `authorSubsidy.rewardAddresses(aura_key)` using current runtime metadata.
2. If no record exists, use nonce `0`; otherwise use the record's `next_nonce`.
   This is separate from the transaction sender's normal account nonce.
3. Choose the receiving EVM address and an expiry that leaves enough time for
   inclusion (for example, current block plus 100).
4. Encode and sign the Aura proof described below.
5. Submit `setRewardAddress` signed by the receiving EVM account with the same
   Aura key, destination, proof nonce, expiry, and proof signature.
6. Wait for successful inclusion and the `RewardAddressScheduled` event. Its
   `activation_block` is the first block eligible to pay the new destination.

The proof authorizes an association between two accounts. The keys may come from
the same secret or different secrets; neither secret is submitted to the chain.

## Exact Aura proof encoding

Sign the SCALE encoding of this tuple, in this order:

```text
(
    Vec<u8>(ASCII "hypertensor/author-subsidy/set-reward-address/v1"),
    genesis_hash: H256,
    aura_key: sr25519::Public,
    reward_address: H160,
    nonce: u64,
    valid_until: u32
)
```

The domain is a SCALE vector (compact length prefix followed by ASCII bytes), not
an unprefixed byte array or a SCALE tuple of characters. Hash and public-key fields
are raw fixed-length bytes. Nonce and block number use SCALE integer encoding.
Sign these bytes directly with sr25519's standard Substrate signing context; do
not hash them first, sign their hex text, or add `<Bytes>`/Ethereum message wrappers.

The public Rust helper `AuthorSubsidy::reward_address_payload` produces these bytes
inside runtime externalities. `AuthorSubsidy::next_nonce` provides the proof nonce.
These are Rust helpers, not additional RPC endpoints. Clients can encode the tuple
locally from the genesis hash and storage data.

```rust,ignore
let payload = AuthorSubsidy::reward_address_payload(
    &aura_pair.public(), reward_address, nonce, valid_until,
);
let aura_signature = aura_pair.sign(&payload);
```

The receiving account then signs the **native extrinsic's** normal `SignedPayload`
with the runtime's Ethereum signature scheme. When implementing a local signer,
use `SignedPayload::using_encoded` (including its standard handling of payloads
over 256 bytes), Keccak256 the resulting bytes, and sign that digest with ECDSA:

```rust,ignore
let signature = payload.using_encoded(|bytes| {
    evm_pair.sign_prehashed(&sp_io::hashing::keccak_256(bytes))
});
```

This outer signature is distinct from the attached Aura signature. A working
end-to-end example, including the signed extensions, is in
[`runtime/src/author_subsidy_tests.rs`](../../runtime/src/author_subsidy_tests.rs).

## Updates and block rewards

Every successful call increments the Aura key's proof nonce and schedules the
address for the next block. An update requires a fresh Aura proof and authorization
from the **new** receiving account. Approval from the old receiving account is not
required: the Aura key controls the choice of payout destination.

The record contains `current_address`, `pending_address`, `activation_block`, and
`next_nonce`. At block numbers below `activation_block`, resolve `current_address`;
at or above it, resolve `pending_address`. No activation transaction is required.
Further changes in the same block replace the pending address while preserving the
current block's destination. Proof expiry governs acceptance of the configuration,
not the lifetime of an accepted payout address.

Both the subsidy hook and EVM author lookup use `FindAuthorRewardAddress`. The
original `FindAuthorTruncated` struct is retained but is not configured for these
lookups. Lookup validates the Aura digest and safely indexes the current authority
set. It never falls back to a truncated key.

Before the first configuration activates, the validator earns no block subsidy.
Missing/malformed Aura digests and missing mappings also produce no subsidy, no
issuance increase, and no `AuthorSubsidy` event. Unpaid subsidies do not accumulate.
The configured subsidy amount is unchanged. Frontier's existing fee handling when
no author resolves is unchanged; this pallet does not add a deferred tip balance.

Multiple Aura authorities may select the same EVM account. Configuration is only
accepted for current authorities. Records and nonces are retained if a key leaves
the authority set; they are not used for rewards while that authority is absent.
There is no root bypass, legacy-address recovery, genesis preconfiguration, or
private-key conversion in this pallet.

## Validation and weights

```sh
cargo test -p pallet-author-subsidy --lib --features runtime-benchmarks --locked
SKIP_WASM_BUILD=1 cargo test -p hypertensor-runtime --lib author_subsidy_tests --locked
cargo build -p hypertensor-runtime --release --features runtime-benchmarks --locked
frame-omni-bencher v1 benchmark pallet \
  --runtime target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm \
  --pallet pallet_author_subsidy --extrinsic '*' \
  --output pallets/author-subsidy/src/weights.rs \
  --template .maintain/frame-weight-template.hbs
```

Benchmarks cover first configuration, updates, successful payouts, and skipped
payouts. Runtime setup supplies a real Aura slot digest and the maximum authority
set, placing the signing authority last to exercise the full membership scan.
