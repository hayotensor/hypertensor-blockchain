# Validators and nominators

Staking elections determine the session validator set. BABE produces blocks and
GRANDPA finalizes them. Validator accounts are native `AccountId32` identities.
BABE uses sr25519 keys and GRANDPA uses ed25519 keys held by the node keystore.

## Local network

```sh
cargo build --release --locked --features fast-runtime
./target/release/hypertensor-node key generate-node-key --chain local --base-path /tmp/npos-alice
./target/release/hypertensor-node key generate-node-key --chain local --base-path /tmp/npos-bob
./target/release/hypertensor-node --chain local --alice --validator --base-path /tmp/npos-alice --port 30333 --rpc-port 9944 --no-prometheus
./target/release/hypertensor-node --chain local --bob --validator --base-path /tmp/npos-bob --port 30334 --rpc-port 9945 --no-prometheus --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/ALICE_PEER_ID
```

Replace `ALICE_PEER_ID` with Alice's printed local peer ID before starting Bob.
Generate each persistent network key once, before the first start; keep it for restarts.
The SDK defaults to Litep2p; explicit bootnodes make peer discovery reproducible.
Libp2p remains selectable with `--network-backend libp2p`.
The single-validator `--dev` preset starts independently. `fast-runtime` is for
local testing only; default builds use four-hour sessions and daily eras. Generate
a fresh specification and database when switching between these configurations.

## Register a validator

1. Fund a native account and bond at least 1,000 TENSOR with `staking.bond`.
2. Generate its persistent network key with `key generate-node-key --chain <SPEC> --base-path <PATH>`,
   then run the node with `--validator`, that base path, and the same chain spec.
3. Call `author_rotateKeysWithOwner` over its local RPC, passing the validator account as the
   hex-encoded SCALE `AccountId32` owner (32 bytes, not the SS58 text). The current SDK
   returns both `keys` and `proof`.
4. Submit both returned values through `session.setKeys(keys, proof)`, signed by the
   same account. Leave at least 1 TENSOR free for the refundable key deposit, plus fees.
5. Submit `staking.validate` with the desired commission. Membership starts after election
   and session queue activation. Check `session.validators` and `staking.activeEra`.

To rotate keys, generate new keys and submit `session.setKeys` again. Keep the
previous keys available through queued session activation. Each proof is tied to the
account that calls `session.setKeys`; an empty proof or a proof for another account
is rejected. `session.purgeKeys` releases the key deposit.

## Nominate and receive rewards

Bond at least 20 TENSOR, then call `staking.nominate` with up to 16 validator accounts.
Consensus rewards are allocated by author points, validator commission, and exposed
stake. Claim via `staking.payoutStakers` or `staking.payoutStakersByPage` within 84 eras.

Call `staking.chill` to stop validating or nominating, `staking.unbond` to begin
unbonding, and `staking.withdrawUnbonded` after the 28-era bonding period (28 days with default production timings).

See [runtime configuration](npos.md) for election bounds, reward budgets, and timing.
