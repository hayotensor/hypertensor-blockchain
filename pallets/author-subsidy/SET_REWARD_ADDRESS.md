# Set your block reward address

You need your current **Babe sr25519 key**, the **receiving EVM account**, and enough
TENSOR in that EVM account to pay the configuration transaction fee.

1. **Confirm your Babe public key.** Check that its 32-byte public key appears in
   `babe.authorities`. Use this key as `babe_key` and your receiving `0x…` EVM address
   as `reward_address`.

2. **Read the proof inputs.** Get the genesis hash with `chain_getBlockHash(0)` and
   the current block number. Read `authorSubsidy.rewardAddresses(babe_key)`:
   use its `next_nonce`, or `0` if no record exists. Set `valid_until` to the current
   block number plus `100`. This proof nonce is separate from your wallet's
   transaction nonce.

3. **Encode the ownership-proof payload.** SCALE-encode this exact tuple:

   ```text
   (
       Vec<u8>(ASCII "hypertensor/author-subsidy/set-reward-address/v1"),
       genesis_hash: H256,
       babe_key: sr25519::Public,
       reward_address: H160,
       nonce: u64,
       valid_until: u32
   )
   ```

   The domain includes the SCALE vector length prefix. Hash, public key, and
   address are raw bytes. See the [encoding reference](README.md#exact-babe-proof-encoding).

4. **Sign with your Babe key.** Sign the encoded bytes directly with sr25519 to
   produce `babe_signature`. Do not sign the hex text, pre-hash the payload, or add
   a wallet message wrapper.

5. **Submit from the receiving EVM account.** Build this pallet call using the
   current runtime metadata:

   ```text
   authorSubsidy.setRewardAddress(
       babe_key, reward_address, nonce, valid_until, babe_signature
   )
   ```

   Sign and submit the **native Substrate extrinsic** with the receiving account's
   ECDSA key, using the chain's `EthereumSignature` support. This requires native
   extrinsic signing tooling; a normal MetaMask `eth_sendTransaction` is not this
   call. The [working Rust example](../../runtime/src/author_subsidy_tests.rs)
   shows construction and signing.

6. **Confirm activation.** Wait for successful inclusion and the
   `RewardAddressScheduled` event, then finality. The address activates at the
   event's `activation_block`, which is the block after inclusion. Subsequent
   blocks authored by your Babe key pay this address; confirm the `AuthorSubsidy`
   event and your EVM balance.

To change the address, repeat these steps with a fresh proof nonce and sign the
transaction from the **new** receiving account. An expired proof must be recreated
with a new expiry. Subsidies before the first activation are skipped and cannot be
claimed later.
