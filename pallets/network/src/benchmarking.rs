//! Benchmarking setup for pallet-network

// * Build *
// cargo build --release --features runtime-benchmarks
// cargo test --release --features runtime-benchmarks

// * Build only this pallet *
// cargo build --package pallet-network --features runtime-benchmarks
// cargo build --package pallet-collective --features runtime-benchmarks
// cargo +nightly build --release --features runtime-benchmarks
// ./target/release/hypertensor-node benchmark machine

// * Copy & Paste to run *
// frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm --extrinsic "" --pallet "pallet_network" --output pallets/network/src/weights.rs --template ./.maintain/frame-weight-template.hbs

// * Quick run *
// frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm --extrinsic "" --pallet "pallet_network" --output pallets/network/src/weights.rs --template ./.maintain/frame-weight-template.hbs --steps 2

// * Run without committing weights *
// frame-omni-bencher v1 benchmark pallet --runtime target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm --extrinsic "" --pallet "pallet_network"

#![cfg(feature = "runtime-benchmarks")]
use super::*;

use crate::utilities::multiaddr::{encode_varint, Multiaddr, DNS4, IP4, P2P, TCP};
#[allow(unused)]
use crate::Pallet as Network;
use crate::*;
use fp_account::AccountId20;
use frame_benchmarking::v2::*;
use frame_support::{
    assert_noop, assert_ok,
    pallet_prelude::{DispatchError, Zero},
    traits::{EnsureOrigin, Get, OnInitialize, UnfilteredDispatchable},
    weights::{Weight, WeightMeter},
    BoundedBTreeMap, BoundedBTreeSet, Callable,
};
use frame_system::{limits::BlockWeights, pallet_prelude::BlockNumberFor, RawOrigin};
pub use pallet::*;
use pallet_collective::{Instance1, Members};
use pallet_evm::{AddressMapping, IdentityAddressMapping};
use pallet_treasury::Pallet as Treasury;
use scale_info::prelude::{format, vec};
use sp_core::{blake2_128, keccak_256, OpaquePeerId as PeerId, H160};
use sp_runtime::{
    traits::{Hash, Header},
    SaturatedConversion, Vec,
};

const SEED: u32 = 0;
const DEFAULT_SCORE: u128 = 100e+18 as u128;
const DEFAULT_SUBNET_INIT_COST: u128 = 100e+18 as u128;
const DEFAULT_SUBNET_NAME: &str = "subnet-name";
const DEFAULT_SUBNET_NAME_2: &str = "subnet-name-2";
const DEFAULT_SUBNET_NODE_STAKE: u128 = 100e+18 as u128;
const DEFAULT_SUBNET_REGISTRATION_BLOCKS: u64 = 130_000;
const DEFAULT_STAKE_TO_BE_ADDED: u128 = 100e+18 as u128;
const DEFAULT_DELEGATE_STAKE_TO_BE_ADDED: u128 = 100e+18 as u128;
const DEFAULT_DEPOSIT_AMOUNT: u128 = 1000e+18 as u128;
const DEFAULT_VALIDATOR_REWARD_RATE: u128 = 50_000_000_000_000_000; // 5%
const ALICE_EXPECTED_BALANCE: u128 = 1000000000000000000000000; // 1,000,000
const BENCHMARK_ACCOUNT_FUNDING_BUFFER: u128 = 10_000_000_000_000;
const BENCHMARK_REGISTRATION_COST_BUFFER: u128 = 1_000;
const BENCHMARK_TRANSFER_SURPLUS: u128 = 500;
const PRIOR_OVERWATCH_SIGNAL_REVISION: u64 = 41;
pub type BalanceOf<T> = <T as Config>::Currency;
type TreasuryPallet<T> = pallet_treasury::Pallet<T, ()>;

fn peer(id: u32) -> PeerId {
    let peer_id = format!("QmYyQSo1c1Ym7orWxLYvCrM2EmxFTANf8wXmmE7DWjhx5N{id}");
    PeerId(peer_id.into())
}

fn get_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
    let caller: T::AccountId = account(name, index, SEED);
    caller
}

fn get_alice<T: Config>() -> T::AccountId {
    let alice: T::AccountId = get_account::<T>("alice", 0);
    let alice_balance = T::Currency::free_balance(&alice.clone());
    if alice_balance < ALICE_EXPECTED_BALANCE.try_into().ok().expect("REASON") {
        let _ = T::Currency::deposit_creating(
            &alice.clone(),
            ALICE_EXPECTED_BALANCE.try_into().ok().expect("REASON"),
        );
    }
    alice
}

fn next_subnet_id<T: Config>() -> u32 {
    TotalSubnetUids::<T>::get()
        .checked_add(1)
        .expect("benchmark subnet identifier space must not be exhausted")
}

fn subnet_id_key_offset<T: Config>(subnet_id: u32) -> u32 {
    subnet_id.saturating_sub(T::InitialSubnetUid::get())
}

fn subnet_owner<T: Config>(subnet_id: u32) -> T::AccountId {
    SubnetOwner::<T>::get(subnet_id).expect("subnet owner exists")
}

fn funded_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
    let caller: T::AccountId = account(name, index, SEED);
    // Give the account half of the maximum value of the `Balance` type.
    // Otherwise some transfers will fail with an overflow error.
    let deposit_amount =
        MinSubnetMinStake::<T>::get().saturating_add(BENCHMARK_ACCOUNT_FUNDING_BUFFER);
    T::Currency::deposit_creating(&caller, deposit_amount.try_into().ok().expect("REASON"));
    caller
}

fn funded_initializer<T: Config>(name: &'static str, index: u32) -> T::AccountId {
    let caller: T::AccountId = account(name, index, SEED);
    // Give the account half of the maximum value of the `Balance` type.
    // Otherwise some transfers will fail with an overflow error.
    let block_number = get_current_block_as_u32::<T>();
    let cost = Network::<T>::get_current_registration_cost(block_number)
        .saturating_add(BENCHMARK_REGISTRATION_COST_BUFFER);
    let alice = get_alice::<T>();
    assert_ok!(T::Currency::transfer(
        &alice, // alice
        &caller.clone(),
        cost.saturating_add(BENCHMARK_TRANSFER_SURPLUS)
            .try_into()
            .ok()
            .expect("REASON"),
        ExistenceRequirement::KeepAlive,
    ));

    caller
}

fn validator_coldkey<T: Config>(validator_id: u32) -> T::AccountId {
    get_account::<T>("validator_coldkey", validator_id)
}

fn validator_hotkey<T: Config>(validator_id: u32) -> T::AccountId {
    get_account::<T>("validator_hotkey", validator_id)
}

fn ensure_validator<T: Config>(validator_id: u32) -> (T::AccountId, T::AccountId) {
    if let Some(coldkey) = ValidatorColdkey::<T>::get(validator_id) {
        let hotkey = ValidatorIdHotkey::<T>::get(validator_id).unwrap();
        return (coldkey, hotkey);
    }

    assert!(validator_id > 0);

    let next_validator_id = TotalValidatorIds::<T>::get().saturating_add(1);
    for id in next_validator_id..=validator_id {
        let coldkey = validator_coldkey::<T>(id);
        let hotkey = validator_hotkey::<T>(id);

        assert_ok!(Network::<T>::register_validator(
            RawOrigin::Signed(coldkey.clone()).into(),
            hotkey.clone(),
            DEFAULT_VALIDATOR_REWARD_RATE,
            None,
            None,
        ));

        assert_eq!(ColdkeyValidatorId::<T>::get(&coldkey), Some(id));
    }

    let coldkey = ValidatorColdkey::<T>::get(validator_id).unwrap();
    let hotkey = ValidatorIdHotkey::<T>::get(validator_id).unwrap();
    (coldkey, hotkey)
}

fn benchmark_queued_swap_principal_from_items<T: Config>() -> u128 {
    SwapCallQueue::<T>::iter().fold(0u128, |total, (_, item)| {
        total
            .checked_add(item.call.get_queue_balance())
            .expect("benchmark queue principal fits u128")
    })
}

fn assert_benchmark_queued_swap_principal<T: Config>() {
    assert_eq!(
        TotalQueuedSwapPrincipal::<T>::get(),
        benchmark_queued_swap_principal_from_items::<T>()
    );
}

/// Fill the swap queue to one entry below its configured bound so enqueue benchmarks measure the
/// maximum successful `SwapQueueOrder` decode and rewrite. Existing items use consecutive IDs and
/// the next ID is deliberately outside that range, matching a valid production queue.
fn prime_near_full_swap_queue<T: Config>() {
    let max_queue_len = T::MaxSwapQueueLength::get();
    let queued_items = max_queue_len
        .checked_sub(1)
        .expect("swap queue benchmark requires a non-zero queue bound");
    let queued_at_block = get_current_block_as_u32::<T>();
    let account_id = get_account::<T>("near_full_swap_queue_account", 0);
    let mut queue: SwapQueueIds<T> = BoundedVec::new();

    for id in 0..queued_items {
        assert!(!SwapCallQueue::<T>::contains_key(id));
        SwapCallQueue::<T>::insert(
            id,
            QueuedSwapItem {
                id,
                call: QueuedSwapCall::SwapToSubnetDelegateStake {
                    account_id: account_id.clone(),
                    to_subnet_id: u32::MAX,
                    balance: 1,
                },
                queued_at_block,
                execute_after_blocks: T::EpochLength::get(),
            },
        );
        queue
            .try_push(id)
            .expect("near-full swap queue remains within its configured bound");
    }

    let next_id = queued_items;
    assert!(!SwapCallQueue::<T>::contains_key(next_id));
    SwapQueueOrder::<T>::set(queue);
    SwapQueueCount::<T>::set(queued_items);
    NextSwapQueueId::<T>::set(next_id);
    TotalQueuedSwapPrincipal::<T>::set(queued_items as u128);
    assert_benchmark_queued_swap_principal::<T>();
}

fn fund_account<T: Config>(account: &T::AccountId, amount: u128) {
    T::Currency::deposit_creating(account, amount.try_into().ok().expect("REASON"));
}

/// Fill one account's unbonding ledger to the configured protocol bound while retaining the
/// supplied claim block. Unstake benchmarks use this to measure the largest successful value
/// rewrite: the new principal merges into the existing entry instead of triggering auto-claim.
fn prime_max_unbonding_ledger_for_merge<T: Config>(
    account: &T::AccountId,
    claim_block: u32,
) -> BTreeMap<u32, UnbondingEntry> {
    let max_unbondings = T::MaxUnbondingsUpperBound::get();
    assert!(max_unbondings > 0);
    MaxUnbondings::<T>::set(max_unbondings);

    let mut ledger = BTreeMap::new();
    for offset in 0..max_unbondings {
        let entry_block = claim_block
            .checked_add(offset)
            .expect("benchmark claim blocks fit u32");
        ledger.insert(
            entry_block,
            UnbondingEntry {
                network: u128::from(offset).saturating_add(1),
                overwatch: u128::from(offset).saturating_add(1),
            },
        );
    }

    let total_network = ledger.values().fold(0u128, |total, entry| {
        total
            .checked_add(entry.network)
            .expect("benchmark unbonding principal fits u128")
    });
    StakeUnbondingLedger::<T>::insert(account, ledger.clone());
    TotalNetworkUnbondingBalance::<T>::set(total_network);
    ledger
}

fn benchmark_peer_info<T: Config>(
    subnet_id: u32,
    subnet_node_index: u32,
    offset: Option<u32>,
) -> PeerInfo<T> {
    let max_subnet_nodes = MaxSubnetNodes::<T>::get();
    let max_subnets = MaxSubnets::<T>::get();
    let peer_id = match offset {
        Some(1) => {
            get_bootnode_peer_id::<T>(subnet_id, max_subnet_nodes, max_subnets, subnet_node_index)
        }
        Some(2) => {
            get_client_peer_id::<T>(subnet_id, max_subnet_nodes, max_subnets, subnet_node_index)
        }
        _ => get_peer_id::<T>(subnet_id, max_subnet_nodes, max_subnets, subnet_node_index),
    };

    PeerInfo::<T> {
        peer_id,
        multiaddr: get_multiaddr::<T>(Some(subnet_id), Some(subnet_node_index), offset),
    }
}

fn register_benchmark_subnet_node<T: Config>(
    subnet_id: u32,
    validator_id: u32,
    subnet_node_index: u32,
    stake_to_be_added: u128,
    node_hotkey: Option<T::AccountId>,
) -> (u32, T::AccountId, T::AccountId, PeerInfo<T>) {
    let (coldkey, validator_hotkey) = ensure_validator::<T>(validator_id);
    let burn_amount = Network::<T>::calculate_burn_amount(subnet_id);
    fund_account::<T>(
        &coldkey,
        stake_to_be_added
            .saturating_add(burn_amount)
            .saturating_add(DEFAULT_DEPOSIT_AMOUNT),
    );

    let peer_info = benchmark_peer_info::<T>(subnet_id, subnet_node_index, None);
    let bootnode_peer_info = benchmark_peer_info::<T>(subnet_id, subnet_node_index, Some(1));
    let client_peer_info = benchmark_peer_info::<T>(subnet_id, subnet_node_index, Some(2));

    assert_ok!(Network::<T>::register_subnet_node(
        RawOrigin::Signed(coldkey.clone()).into(),
        validator_id,
        subnet_id,
        node_hotkey.clone(),
        Some(peer_info.clone()),
        Some(bootnode_peer_info),
        Some(client_peer_info),
        stake_to_be_added,
        None,
        None,
        u128::MAX,
    ));

    let subnet_node_id = TotalSubnetNodeUids::<T>::get(subnet_id);
    let hotkey = node_hotkey.unwrap_or(validator_hotkey);
    (subnet_node_id, coldkey, hotkey, peer_info)
}

pub fn get_coldkey_n<T: Config>(subnets: u32, max_subnet_nodes: u32, n: u32) -> u32 {
    subnets * max_subnet_nodes + n
}

pub fn get_coldkey<T: Config>(subnets: u32, max_subnet_nodes: u32, n: u32) -> T::AccountId {
    get_account::<T>("coldkey", get_coldkey_n::<T>(subnets, max_subnet_nodes, n))
}

pub fn get_hotkey_n<T: Config>(
    subnets: u32,
    max_subnet_nodes: u32,
    max_subnets: u32,
    n: u32,
) -> u32 {
    max_subnets * max_subnet_nodes + (subnets * max_subnet_nodes) + n
}

pub fn get_hotkey<T: Config>(
    subnets: u32,
    max_subnet_nodes: u32,
    max_subnets: u32,
    n: u32,
) -> T::AccountId {
    get_account::<T>(
        "hotkey",
        get_hotkey_n::<T>(subnets, max_subnet_nodes, max_subnets, n),
    )
}

pub fn get_peer_id<T: Config>(
    subnets: u32,
    max_subnet_nodes: u32,
    max_subnets: u32,
    n: u32,
) -> PeerId {
    peer(max_subnets * max_subnet_nodes + (subnets * max_subnet_nodes) + n)
}

pub fn get_bootnode_peer_id<T: Config>(
    subnets: u32,
    max_subnet_nodes: u32,
    max_subnets: u32,
    n: u32,
) -> PeerId {
    peer(
        (max_subnets * max_subnet_nodes * 2)
            + (max_subnets * max_subnet_nodes + (subnets * max_subnet_nodes) + n),
    )
}

pub fn get_client_peer_id<T: Config>(
    subnets: u32,
    max_subnet_nodes: u32,
    max_subnets: u32,
    n: u32,
) -> PeerId {
    peer(
        (max_subnets * max_subnet_nodes * 3)
            + (max_subnets * max_subnet_nodes + (subnets * max_subnet_nodes) + n),
    )
}

pub fn get_overwatch_coldkey<T: Config>(
    max_subnet_nodes: u32,
    max_subnets: u32,
    max_onodes: u32,
    n: u32,
) -> T::AccountId {
    get_account::<T>(
        "overwatch_coldkey",
        max_subnets * max_subnet_nodes + max_subnets * max_subnet_nodes + n,
    )
}

pub fn get_overwatch_hotkey<T: Config>(
    max_subnet_nodes: u32,
    max_subnets: u32,
    max_onodes: u32,
    n: u32,
) -> T::AccountId {
    get_account::<T>(
        "overwatch_hotkey",
        max_subnets * max_subnet_nodes + max_subnets * max_subnet_nodes + max_onodes + n,
    )
}

pub fn increase_epochs<T: Config>(n: u32) {
    if n == 0 {
        return;
    }

    let block = get_current_block_as_u32::<T>();
    let epoch_length = T::EpochLength::get();

    let advance_blocks = epoch_length.saturating_mul(n);
    let new_block = block.saturating_add(advance_blocks);

    frame_system::Pallet::<T>::set_block_number(new_block.into());
}

pub fn increase_overwatch_epochs<T: Config>(n: u32) {
    if n == 0 {
        return;
    }

    let block = get_current_block_as_u32::<T>();
    let multiplier = OverwatchEpochLengthMultiplier::<T>::get();

    let advance_blocks = n * multiplier * T::EpochLength::get();
    let new_block = block.saturating_add(advance_blocks);

    frame_system::Pallet::<T>::set_block_number(new_block.into());
    CurrentOverwatchEpoch::<T>::mutate(|epoch| *epoch = epoch.saturating_add(n));
    OverwatchEpochStartBlock::<T>::put(new_block);
}

/// Establish an anchored benchmark epoch under a static multiplier.
pub fn set_overwatch_epoch<T: Config>(n: u32) {
    let multiplier = OverwatchEpochLengthMultiplier::<T>::get();
    let start_block = n
        .saturating_mul(multiplier)
        .saturating_mul(T::EpochLength::get());
    frame_system::Pallet::<T>::set_block_number(start_block.into());
    CurrentOverwatchEpoch::<T>::put(n);
    OverwatchEpochStartBlock::<T>::put(start_block);
}

fn build_activated_subnet<T: Config>(
    name: Vec<u8>,
    start: u32,
    mut end: u32,
    deposit_amount: u128,
    amount: u128,
) {
    let alice = get_alice::<T>();

    let epoch_length = T::EpochLength::get();
    let block_number = get_current_block_as_u32::<T>();
    let epoch = block_number.saturating_div(epoch_length);

    let min_nodes = MinSubnetNodes::<T>::get();
    let subnets = subnet_id_key_offset::<T>(next_subnet_id::<T>());
    let max_subnets = MaxSubnets::<T>::get();
    let max_subnet_nodes = MaxSubnetNodes::<T>::get();

    if end == 0 {
        end = min_nodes;
    }
    let initial_end = start.saturating_add(
        end.saturating_sub(start)
            .min(T::MaxRegisteredNodesUpperBound::get()),
    );

    let owner_coldkey =
        funded_initializer::<T>("subnet_owner", subnets * max_subnets * max_subnet_nodes);
    let owner_hotkey =
        get_account::<T>("subnet_owner", subnets * max_subnets * max_subnet_nodes + 1);

    let register_subnet_data = default_registration_subnet_data::<T>(
        subnets,
        max_subnet_nodes,
        name.clone().into(),
        start,
        initial_end,
    );

    // --- Register subnet for activation
    assert_ok!(Network::<T>::register_subnet(
        RawOrigin::Signed(owner_coldkey.clone()).into(),
        100000000000000000000000,
        register_subnet_data,
    ));

    let subnet_id = SubnetName::<T>::get(name.clone()).unwrap();
    let subnet = SubnetsData::<T>::get(subnet_id).unwrap();
    let owner_coldkey = subnet_owner::<T>(subnet_id);

    let subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
    let deposit_amount: u128 = MinSubnetMinStake::<T>::get() + 10000;

    // --- Add subnet nodes
    let mut amount_staked = 0;
    for n in start..initial_end {
        let _n = n + 1;
        amount_staked += amount;
        let validator_id = _n;
        let (subnet_node_id, _coldkey, _hotkey, peer_info) =
            register_benchmark_subnet_node::<T>(subnet_id, validator_id, _n, amount, None);

        let subnet_node_data = SubnetNodesData::<T>::try_get(subnet_id, subnet_node_id).unwrap();
        assert_eq!(subnet_node_data.validator_id, validator_id);

        assert_eq!(
            subnet_node_data.peer_info.as_ref().unwrap().peer_id,
            peer_info.peer_id.clone()
        );

        // --- Is ``Validator`` if registered before subnet activation
        assert_eq!(
            subnet_node_data.classification.node_class,
            SubnetNodeClass::Validator
        );
        assert!(subnet_node_data.has_classification(&SubnetNodeClass::Validator, subnet_epoch));

        let peer_subnet_node_account =
            PeerIdSubnetNodeId::<T>::get(subnet_id, peer_info.peer_id.clone());
        assert_eq!(peer_subnet_node_account, subnet_node_id);

        let account_subnet_stake = NodeSubnetStake::<T>::get(subnet_node_id, subnet_id);
        assert_eq!(account_subnet_stake, amount);

        let mut is_electable = false;
        for node_id in SubnetNodeElectionSlots::<T>::get(subnet_id).iter() {
            if *node_id == subnet_node_id {
                is_electable = true;
            }
        }
        assert!(is_electable);

        let validator_subnet_nodes = ValidatorSubnetNodes::<T>::get(validator_id);
        assert!(validator_subnet_nodes
            .get(&subnet_id)
            .unwrap()
            .contains(&subnet_node_id))
    }

    let active_nodes = TotalActiveSubnetNodes::<T>::get(subnet_id);
    assert_eq!(active_nodes, initial_end - start);

    let slot_list = SubnetNodeElectionSlots::<T>::get(subnet_id);
    assert_eq!(slot_list.len(), active_nodes as usize);

    let total_subnet_stake = TotalSubnetStake::<T>::get(subnet_id);
    assert_eq!(total_subnet_stake, amount_staked);

    // Use an unfunded destination because the transfer below creates the account. Minting funds
    // after calculating this issuance-based minimum would leave the pool just below that minimum.
    let delegate_staker_account: T::AccountId = get_account::<T>("delegate_staker", 1);
    let min_subnet_delegate_stake = Network::<T>::get_min_subnet_delegate_stake_balance(subnet_id)
        + (1000e+18 as u128 * subnets.saturating_add(1) as u128);
    // --- Add the minimum required delegate stake balance to activate the subnet

    assert_ok!(T::Currency::transfer(
        &alice, // alice
        &delegate_staker_account.clone(),
        (min_subnet_delegate_stake + 500)
            .try_into()
            .ok()
            .expect("REASON"),
        ExistenceRequirement::KeepAlive,
    ));
    assert_ok!(Network::<T>::add_subnet_delegate_stake(
        RawOrigin::Signed(delegate_staker_account.clone()).into(),
        subnet_id,
        min_subnet_delegate_stake,
    ));

    let total_delegate_stake_balance = TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);
    assert_eq!(total_delegate_stake_balance, min_subnet_delegate_stake);

    let delegate_shares =
        AccountSubnetDelegateStakeShares::<T>::get(&delegate_staker_account, subnet_id);

    let min_registration_epochs = MinSubnetRegistrationEpochs::<T>::get();
    increase_epochs::<T>(min_registration_epochs + 1);

    assert_ok!(Network::<T>::activate_subnet(
        RawOrigin::Signed(owner_coldkey.clone()).into(),
        subnet_id,
    ));

    let subnet = SubnetsData::<T>::get(subnet_id).unwrap();
    assert_eq!(subnet.state, SubnetState::Active);

    // A Registered subnet can whitelist at most 64 identities, but an Active subnet can
    // reach the full 512-node ceiling with distinct validators as registrations mature. Build
    // that reachable post-activation suffix one node at a time so active benchmark fixtures do
    // not collapse their identity cardinality merely to satisfy the registration-phase bound.
    for n in initial_end..end {
        let node_index = n.saturating_add(1);
        let validator_id = node_index;
        let (subnet_node_id, _coldkey, _hotkey, _peer_info) =
            register_benchmark_subnet_node::<T>(subnet_id, validator_id, node_index, amount, None);
        amount_staked = amount_staked.saturating_add(amount);

        let queued_node = RegisteredSubnetNodesData::<T>::get(subnet_id, subnet_node_id);
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
        assert!(Network::<T>::do_activate_subnet_node(
            &mut WeightMeter::new(),
            subnet_id,
            SubnetState::Active,
            queued_node,
            current_subnet_epoch,
            true,
        ));
        SubnetNodeQueue::<T>::mutate(subnet_id, |queue| {
            queue.retain(|node| node.id != subnet_node_id)
        });
        assert!(Network::<T>::graduate_class(
            subnet_id,
            subnet_node_id,
            current_subnet_epoch,
        ));
        assert!(Network::<T>::graduate_to_validator_class(
            subnet_id,
            subnet_node_id,
            current_subnet_epoch,
        ));
    }

    assert_eq!(TotalActiveSubnetNodes::<T>::get(subnet_id), end - start);
    assert_eq!(TotalSubnetNodes::<T>::get(subnet_id), end - start);
    assert_eq!(
        SubnetNodeElectionSlots::<T>::get(subnet_id).len() as u32,
        end - start
    );
    assert_eq!(SubnetNodeQueue::<T>::get(subnet_id).len(), 0);
    assert_eq!(TotalSubnetStake::<T>::get(subnet_id), amount_staked);
}

/// Move a benchmark to the first local subnet epoch in which an owner pause is valid.
///
/// Activation and unpause can occur on either side of the subnet's assigned slot, so deriving
/// this from the current global epoch can leave the fixture one local epoch early. Advancing by
/// whole chain epochs preserves the current slot phase while advancing the subnet epoch exactly.
fn advance_to_subnet_pause_eligibility<T: Config>(subnet_id: u32) {
    let subnet = SubnetsData::<T>::get(subnet_id).expect("benchmark subnet must exist");
    let consensus_eligible_from_subnet_epoch = subnet
        .consensus_eligible_from_subnet_epoch
        .expect("benchmark subnet must be active");
    let pause_epoch =
        consensus_eligible_from_subnet_epoch.saturating_add(SubnetPauseCooldownEpochs::<T>::get());
    let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);

    increase_epochs::<T>(pause_epoch.saturating_sub(current_subnet_epoch));
}

fn build_registered_subnet<T: Config>(
    name: Vec<u8>,
    start: u32,
    mut end: u32,
    deposit_amount: u128,
    amount: u128,
    use_unique_coldkey: bool, // if to use unique coldkeys for each subnet
) {
    let alice = get_alice::<T>();

    let epoch_length = T::EpochLength::get();
    let block_number = get_current_block_as_u32::<T>();
    let epoch = block_number.saturating_div(epoch_length);

    let min_nodes = MinSubnetNodes::<T>::get();
    let subnets = subnet_id_key_offset::<T>(next_subnet_id::<T>());
    let max_subnets = MaxSubnets::<T>::get();
    let max_subnet_nodes = MaxSubnetNodes::<T>::get();

    if end == 0 {
        end = min_nodes;
    }

    let owner_coldkey =
        funded_initializer::<T>("subnet_owner", subnets * max_subnets * max_subnet_nodes);
    let owner_hotkey =
        get_account::<T>("subnet_owner", subnets * max_subnets * max_subnet_nodes + 1);

    let register_subnet_data = default_registration_subnet_data::<T>(
        subnets,
        max_subnet_nodes,
        name.clone().into(),
        start,
        end,
    );

    // --- Register subnet for activation
    assert_ok!(Network::<T>::register_subnet(
        RawOrigin::Signed(owner_coldkey.clone()).into(),
        100000000000000000000000,
        register_subnet_data,
    ));

    let subnet_id = SubnetName::<T>::get(name.clone()).unwrap();
    let subnet = SubnetsData::<T>::get(subnet_id).unwrap();

    let subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
    let deposit_amount: u128 = MinSubnetMinStake::<T>::get() + 10000;

    // --- Add subnet nodes
    let mut amount_staked = 0;
    for n in start..end {
        let _n = n + 1;
        amount_staked += amount;
        let validator_id = bounded_initial_validator_id::<T>(start, end, n);
        let (subnet_node_id, _coldkey, _hotkey, peer_info) =
            register_benchmark_subnet_node::<T>(subnet_id, validator_id, _n, amount, None);

        let subnet_node_data = SubnetNodesData::<T>::try_get(subnet_id, subnet_node_id).unwrap();
        assert_eq!(subnet_node_data.validator_id, validator_id);

        assert_eq!(
            subnet_node_data.peer_info.as_ref().unwrap().peer_id,
            peer_info.peer_id.clone()
        );

        // --- Is ``Validator`` if registered before subnet activation
        assert_eq!(
            subnet_node_data.classification.node_class,
            SubnetNodeClass::Validator
        );
        assert!(subnet_node_data.has_classification(&SubnetNodeClass::Validator, subnet_epoch));

        let peer_subnet_node_account =
            PeerIdSubnetNodeId::<T>::get(subnet_id, peer_info.peer_id.clone());
        assert_eq!(peer_subnet_node_account, subnet_node_id);

        let account_subnet_stake = NodeSubnetStake::<T>::get(subnet_node_id, subnet_id);
        assert_eq!(account_subnet_stake, amount);

        let mut is_electable = false;
        for node_id in SubnetNodeElectionSlots::<T>::get(subnet_id).iter() {
            if *node_id == subnet_node_id {
                is_electable = true;
            }
        }
        assert!(is_electable);

        let validator_subnet_nodes = ValidatorSubnetNodes::<T>::get(validator_id);
        assert!(validator_subnet_nodes
            .get(&subnet_id)
            .unwrap()
            .contains(&subnet_node_id))
    }

    let active_nodes = TotalActiveSubnetNodes::<T>::get(subnet_id);
    assert_eq!(active_nodes, end - start);

    let slot_list = SubnetNodeElectionSlots::<T>::get(subnet_id);
    assert_eq!(slot_list.len(), active_nodes as usize);

    let total_subnet_stake = TotalSubnetStake::<T>::get(subnet_id);
    assert_eq!(total_subnet_stake, amount_staked);

    let min_subnet_delegate_stake = Network::<T>::get_min_subnet_delegate_stake_balance(subnet_id)
        + (1000e+18 as u128 * subnets as u128);
    // --- Add the minimum required delegate stake balance to activate the subnet

    let delegate_staker_account: T::AccountId = funded_account::<T>("delegate_staker", 1);
    let alice = get_alice::<T>();
    assert_ok!(T::Currency::transfer(
        &alice, // alice
        &delegate_staker_account.clone(),
        (min_subnet_delegate_stake + 500)
            .try_into()
            .ok()
            .expect("REASON"),
        ExistenceRequirement::KeepAlive,
    ));
    assert_ok!(Network::<T>::add_subnet_delegate_stake(
        RawOrigin::Signed(delegate_staker_account.clone()).into(),
        subnet_id,
        min_subnet_delegate_stake,
    ));

    let total_delegate_stake_balance = TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);
    assert_eq!(total_delegate_stake_balance, min_subnet_delegate_stake);

    let delegate_shares =
        AccountSubnetDelegateStakeShares::<T>::get(&delegate_staker_account, subnet_id);

    let subnet = SubnetsData::<T>::get(subnet_id).unwrap();
    assert_eq!(subnet.state, SubnetState::Registered);
}

fn build_registered_subnet_nodes<T: Config>(
    subnet_id: u32,
    start: u32,
    mut end: u32,
    deposit_amount: u128,
    amount: u128,
    use_unique_coldkey: bool, // if to use unique coldkeys for each subnet
) {
    let alice = get_alice::<T>();

    let epoch_length = T::EpochLength::get();
    let block_number = get_current_block_as_u32::<T>();
    let epoch = block_number.saturating_div(epoch_length);

    let min_nodes = MinSubnetNodes::<T>::get();
    let max_subnets = MaxSubnets::<T>::get();
    let max_subnet_nodes = MaxSubnetNodes::<T>::get();

    if end == 0 {
        end = min_nodes;
    }

    let subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
    let deposit_amount: u128 = MinSubnetMinStake::<T>::get() + 10000;

    // --- Add subnet nodes
    let mut amount_staked = 0;
    for n in start..end {
        let _n = n + 1;
        amount_staked += amount;
        let validator_id = _n;
        let (subnet_node_id, _coldkey, _hotkey, peer_info) =
            register_benchmark_subnet_node::<T>(subnet_id, validator_id, _n, amount, None);

        let subnet_node_data =
            RegisteredSubnetNodesData::<T>::try_get(subnet_id, subnet_node_id).unwrap();
        assert_eq!(subnet_node_data.validator_id, validator_id);

        assert_eq!(
            subnet_node_data.peer_info.as_ref().unwrap().peer_id,
            peer_info.peer_id.clone()
        );

        // --- Is ``Validator`` if registered before subnet activation
        assert_eq!(
            subnet_node_data.classification.node_class,
            SubnetNodeClass::Registered
        );
        assert!(subnet_node_data
            .has_classification(&SubnetNodeClass::Registered, subnet_epoch.saturating_add(1),));

        let peer_subnet_node_account =
            PeerIdSubnetNodeId::<T>::get(subnet_id, peer_info.peer_id.clone());
        assert_eq!(peer_subnet_node_account, subnet_node_id);

        let account_subnet_stake = NodeSubnetStake::<T>::get(subnet_node_id, subnet_id);
        assert_eq!(account_subnet_stake, amount);

        let mut is_electable = false;
        for node_id in SubnetNodeElectionSlots::<T>::get(subnet_id).iter() {
            if *node_id == subnet_node_id {
                is_electable = true;
            }
        }
        assert!(!is_electable);

        let validator_subnet_nodes = ValidatorSubnetNodes::<T>::get(validator_id);
        assert!(validator_subnet_nodes
            .get(&subnet_id)
            .unwrap()
            .contains(&subnet_node_id))
    }
}

pub fn get_multiaddr<T: Config>(
    subnet_id: Option<u32>,
    subnet_node_id: Option<u32>,
    offset: Option<u32>,
) -> Option<NetworkBytes<T>> {
    let ip_append = if let Some(subnet_id) = subnet_id {
        &[127, 0, offset.unwrap_or(0) as u8, subnet_id as u8]
    } else {
        &[127, 0, 0, 1]
    };

    let port_append = if let Some(subnet_node_id) = subnet_node_id {
        (30303u16 + subnet_node_id as u16).to_be_bytes()
    } else {
        30303u16.to_be_bytes()
    };

    let peer = PeerId::new([1u8; 32].to_vec());
    let mut bytes = vec![];
    encode_varint(IP4, &mut bytes);
    bytes.extend_from_slice(ip_append);
    encode_varint(TCP, &mut bytes);
    bytes.extend_from_slice(&port_append);
    encode_varint(P2P, &mut bytes);
    encode_varint(peer.0.len() as u64, &mut bytes);
    bytes.extend_from_slice(&peer.0);

    let ma = Multiaddr::verify(&bytes).unwrap();
    let _ = ma.to_vec().unwrap();

    Some(bytes.try_into().expect("multiaddr too long"))
}

pub fn default_registration_subnet_data<T: Config>(
    subnets: u32,
    max_subnet_nodes: u32,
    name: Vec<u8>,
    start: u32,
    end: u32,
) -> RegistrationSubnetData<T> {
    let seed_bytes: &[u8] = &name;
    let add_subnet_data = RegistrationSubnetData::<T> {
        name: name.clone(),
        repo: blake2_128(seed_bytes).to_vec(), // must be unique
        description: Vec::new(),
        misc: Vec::new(),
        min_stake: MinSubnetMinStake::<T>::get(),
        max_stake: NetworkMaxStakeBalance::<T>::get(),
        delegate_stake_percentage: 100000000000000000, // 10%
        initial_validators: bounded_initial_validator_ids::<T>(start, end),
        bootnodes: BTreeMap::from([(
            peer(0),
            get_multiaddr::<T>(None, None, None).expect("valid multiaddr"),
        )]),
    };
    add_subnet_data
}

pub fn insert_overwatch_node<T: Config>(validator_id: u32, hotkey_n: u32) -> u32 {
    ensure_validator::<T>(validator_id);
    OverwatchValidatorWhitelist::<T>::insert(validator_id, ());
    let hotkey = get_account::<T>("overwatch_node", hotkey_n);

    TotalOverwatchNodeUids::<T>::mutate(|n: &mut u32| *n += 1);
    let current_uid = TotalOverwatchNodeUids::<T>::get();
    TotalOverwatchNodes::<T>::mutate(|n: &mut u32| *n = n.saturating_add(1));

    OverwatchNodes::<T>::insert(current_uid, ());
    OverwatchNodeIdHotkey::<T>::insert(current_uid, hotkey.clone());
    OverwatchNodeValidatorId::<T>::insert(current_uid, validator_id);
    ValidatorOverwatchNodeId::<T>::insert(validator_id, current_uid);

    current_uid
}

pub fn set_overwatch_stake<T: Config>(overwatch_node_id: u32, amount: u128) {
    // -- increase overwatch node staking balance
    OverwatchNodeStakeBalance::<T>::mutate(overwatch_node_id, |mut n| *n += amount);
    // -- increase total stake
    TotalOverwatchNodeStakeBalance::<T>::mutate(|mut n| *n += amount);
}

pub fn submit_overwatch_reveal<T: Config>(
    overwatch_epoch: u32,
    subnet_id: u32,
    node_id: u32,
    weight: u128,
) {
    OverwatchReveals::<T>::mutate(overwatch_epoch, node_id, |reveals| {
        reveals
            .try_insert(subnet_id, weight)
            .expect("benchmark reveal row fits its runtime bound");
    });
}

fn benchmark_overwatch_settlement_snapshot<T: Config>(
    overwatch_nodes: &[(u32, u128)],
    epoch_length_multiplier: u32,
    stake_weight_factor: u128,
) -> OverwatchEpochSettlementSnapshot<T> {
    let nodes = overwatch_nodes
        .iter()
        .map(|&(node_id, stake)| (node_id, OverwatchNodeSettlementSnapshot { stake }))
        .collect::<BTreeMap<_, _>>()
        .try_into()
        .expect("benchmark settlement node map fits its runtime bound");
    let reward_budget = T::OverwatchEpochEmissions::get()
        .checked_mul(epoch_length_multiplier as u128)
        .expect("benchmark settlement reward budget fits u128");

    OverwatchEpochSettlementSnapshot::<T> {
        stake_weight_factor,
        reward_budget,
        nodes,
    }
}

/// Seed the maximum latest-only signal value that a subsequent finalization must overwrite.
fn seed_max_prior_overwatch_signal<T: Config>(source_epoch: u32) {
    let max_nodes = T::MaxOverwatchNodesUpperBound::get();
    let max_subnets = T::MaxPhysicalSubnetsUpperBound::get();
    assert_eq!(max_nodes, MAX_OVERWATCH_NODES_BENCHMARK_DOMAIN);
    assert_eq!(max_subnets, MAX_PHYSICAL_SUBNETS_BENCHMARK_DOMAIN);

    let percentage_factor = Network::<T>::percentage_factor_as_u128();
    let base_stake = OverwatchMinStakeBalance::<T>::get();
    let subnet_ids = (1..=max_subnets).collect::<Vec<_>>();
    let nodes = (1..=max_nodes)
        .map(|node_id| {
            let reveals: BoundedBTreeMap<u32, u128, T::MaxPhysicalSubnetsUpperBound> = subnet_ids
                .iter()
                .copied()
                .map(|subnet_id| {
                    let raw_weight = percentage_factor
                        .saturating_sub(node_id as u128)
                        .saturating_sub(subnet_id as u128)
                        .max(1);
                    (subnet_id, raw_weight)
                })
                .collect::<BTreeMap<_, _>>()
                .try_into()
                .expect("maximum prior signal reveal row fits its runtime bound");
            let stake = base_stake
                .checked_mul(node_id as u128)
                .expect("maximum prior signal stake fits u128");
            (
                node_id,
                LatestOverwatchNodeSignalInput::<T> { stake, reveals },
            )
        })
        .collect::<BTreeMap<_, _>>()
        .try_into()
        .expect("maximum prior signal node map fits its runtime bound");
    let retained_inputs = LatestFinalizedOverwatchSignalInput::<T> {
        source_epoch,
        stake_weight_factor: DefaultOverwatchStakeWeightFactor::get(),
        nodes,
    };
    let derived = Network::<T>::derive_overwatch_signal(&retained_inputs)
        .expect("maximum prior signal inputs remain bounded");
    LastFinalizedOverwatchEpoch::<T>::put(source_epoch);
    LatestFinalizedOverwatchSignalInputs::<T>::put(retained_inputs);
    LatestEffectiveOverwatchSignal::<T>::put(EffectiveOverwatchSignal::<T> {
        source_epoch,
        valid: true,
        subnet_weights: derived.subnet_weights,
    });
    LatestOverwatchSignalRevision::<T>::put(PRIOR_OVERWATCH_SIGNAL_REVISION);
}

/// Fill the effective cache independently of the live subnet count. Removed subnets can leave
/// all 17 raw keys in the latest finalized signal until the next successful finalization.
fn seed_max_effective_overwatch_cache<T: Config>(
    source_epoch: u32,
    mut subnet_weights: BTreeMap<u32, u128>,
) {
    let max_subnets = T::MaxPhysicalSubnetsUpperBound::get();
    let raw_weight = Network::<T>::percentage_factor_as_u128();
    let mut candidate = u32::MAX;
    while subnet_weights.len() < max_subnets as usize {
        subnet_weights.entry(candidate).or_insert(raw_weight);
        candidate = candidate.saturating_sub(1);
    }
    assert_eq!(subnet_weights.len(), max_subnets as usize);
    LatestEffectiveOverwatchSignal::<T>::put(EffectiveOverwatchSignal::<T> {
        source_epoch,
        valid: true,
        subnet_weights: subnet_weights
            .try_into()
            .expect("maximum effective cache fits its runtime bound"),
    });
}

/// Seed a closed Overwatch epoch with an exact, reachable reveal cardinality.
///
/// `reveal_records` is deliberately kept as the only independent component. The node and subnet
/// counts are selected by each piecewise benchmark so the fixture never measures an impossible
/// Cartesian combination of records, revealers and subnets.
fn prepare_overwatch_reward_benchmark<T: Config>(
    reveal_records: u32,
    revealing_nodes: u32,
    revealed_subnets: u32,
) -> (u32, Vec<(u32, u128)>, Vec<u32>) {
    assert!(reveal_records > 0);
    assert!(revealing_nodes > 0);
    assert!(revealed_subnets > 0);
    assert!(revealing_nodes <= T::MaxOverwatchNodesUpperBound::get());
    assert!(revealed_subnets <= T::MaxPhysicalSubnetsUpperBound::get());
    assert!(reveal_records <= revealing_nodes.saturating_mul(revealed_subnets));

    let end = MinSubnetNodes::<T>::get();
    NewRegistrationCostMultiplier::<T>::set(Network::<T>::percentage_factor_as_u128());

    // Exercise the configured 0.9 diminishing-return path, rather than the cheaper exact-linear
    // endpoint. Stakes use the runtime's 18-decimal denomination and increase per node so the
    // benchmark also covers finding and normalizing against a unique maximum stake.
    let percentage_factor = Network::<T>::percentage_factor_as_u128();
    let stake_weight_factor = DefaultOverwatchStakeWeightFactor::get();
    assert_eq!(
        stake_weight_factor,
        percentage_factor.saturating_mul(9) / 10
    );
    OverwatchStakeWeightFactor::<T>::set(stake_weight_factor);
    let base_stake = OverwatchMinStakeBalance::<T>::get();
    assert_eq!(base_stake, 100u128.saturating_mul(percentage_factor));

    let mut subnet_ids = Vec::with_capacity(revealed_subnets as usize);
    for subnet_index in 0..revealed_subnets {
        let path: Vec<u8> = format!("overwatch-reward-subnet-{subnet_index}").into();
        build_activated_subnet::<T>(
            path.clone(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        subnet_ids.push(
            SubnetName::<T>::get(path).expect("reward benchmark subnet must be indexed by name"),
        );
    }

    let mut overwatch_nodes = Vec::with_capacity(revealing_nodes as usize);
    for node_index in 0..revealing_nodes {
        let validator_id = node_index.saturating_add(1);
        let hotkey_index = validator_id.saturating_add(revealing_nodes);
        let node_id = insert_overwatch_node::<T>(validator_id, hotkey_index);
        let stake = base_stake
            .checked_mul(validator_id as u128)
            .expect("reward benchmark stake fits u128");
        set_overwatch_stake::<T>(node_id, stake);
        overwatch_nodes.push((node_id, stake));
    }
    // `insert_overwatch_node` is a lightweight benchmark helper. Keep the live-count and both
    // active ownership indexes coherent with the exact node set.
    TotalOverwatchNodes::<T>::set(revealing_nodes);

    // Overwatch registration is only permitted after epoch zero. Use a completed epoch of one
    // and leave the active epoch at two below, matching the state immediately after rollover.
    let overwatch_epoch = Network::<T>::get_current_overwatch_epoch_as_u32().max(1);
    CurrentOverwatchEpoch::<T>::put(overwatch_epoch);
    let mut reveal_pairs = BTreeSet::new();
    let mut revealing_node_ids = BTreeSet::new();
    let mut revealed_subnet_ids = BTreeSet::new();
    let mut subnet_revealer_counts = BTreeMap::<u32, u32>::new();
    for record_index in 0..reveal_records {
        let node_index = record_index % revealing_nodes;
        let subnet_index =
            node_index.saturating_add(record_index / revealing_nodes) % revealed_subnets;
        let node_id = overwatch_nodes[node_index as usize].0;
        let subnet_id = subnet_ids[subnet_index as usize];

        assert!(
            reveal_pairs.insert((subnet_id, node_id)),
            "reward benchmark records must be unique"
        );
        revealing_node_ids.insert(node_id);
        revealed_subnet_ids.insert(subnet_id);
        subnet_revealer_counts
            .entry(subnet_id)
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
        submit_overwatch_reveal::<T>(
            overwatch_epoch,
            subnet_id,
            node_id,
            Network::<T>::percentage_factor_as_u128() / 2,
        );
    }

    assert_eq!(reveal_pairs.len() as u32, reveal_records);
    assert_eq!(revealing_node_ids.len() as u32, revealing_nodes);
    assert_eq!(revealed_subnet_ids.len() as u32, revealed_subnets);

    // Build the same bounded aggregate that reveal dispatches maintain, then consume it exactly
    // as epoch rollover does. This leaves the next epoch's active aggregate empty while the
    // pending snapshot and reveal prefix describe the closed epoch precisely.
    ActiveOverwatchRevealStats::<T>::put(OverwatchRevealStats::<T> {
        records: reveal_records,
        subnet_revealer_counts: subnet_revealer_counts
            .try_into()
            .expect("reward benchmark subnet-count map fits its runtime bound"),
    });
    let reveal_stats = ActiveOverwatchRevealStats::<T>::take();
    let epoch_length_multiplier = ActiveOverwatchEpochLengthMultiplier::<T>::get();
    PendingOverwatchSettlement::<T>::put(PendingOverwatchSettlementData {
        epoch: overwatch_epoch,
        reveal_records: reveal_stats.records,
    });
    OverwatchEpochSettlementSnapshots::<T>::insert(
        overwatch_epoch,
        benchmark_overwatch_settlement_snapshot::<T>(
            &overwatch_nodes,
            epoch_length_multiplier,
            stake_weight_factor,
        ),
    );
    CurrentOverwatchEpoch::<T>::put(overwatch_epoch.saturating_add(1));
    seed_max_prior_overwatch_signal::<T>(overwatch_epoch.saturating_sub(1));

    (overwatch_epoch, overwatch_nodes, subnet_ids)
}

fn assert_overwatch_reward_benchmark_result<T: Config>(
    overwatch_epoch: u32,
    overwatch_nodes: &[(u32, u128)],
    subnet_ids: &[u32],
) {
    assert!(PendingOverwatchSettlement::<T>::get().is_none());
    assert!(!OverwatchEpochSettlementSnapshots::<T>::contains_key(
        overwatch_epoch
    ));
    assert_eq!(
        LastFinalizedOverwatchEpoch::<T>::get(),
        Some(overwatch_epoch)
    );
    assert_eq!(
        LatestOverwatchSignalRevision::<T>::get(),
        PRIOR_OVERWATCH_SIGNAL_REVISION.saturating_add(1)
    );

    let percentage_factor = Network::<T>::percentage_factor_as_u128();
    let mut normalized_score_sum = 0u128;
    for &(node_id, initial_stake) in overwatch_nodes {
        let node_weight = OverwatchNodeWeights::<T>::get(overwatch_epoch, node_id)
            .expect("every seeded revealer receives a normalized score");
        assert!(node_weight > 0 && node_weight <= percentage_factor);
        normalized_score_sum = normalized_score_sum
            .checked_add(node_weight)
            .expect("normalized reward-score sum fits u128");
        assert!(OverwatchNodeStakeBalance::<T>::get(node_id) > initial_stake);
    }
    assert!(normalized_score_sum <= percentage_factor);
    assert!(percentage_factor.saturating_sub(normalized_score_sum) < overwatch_nodes.len() as u128);

    for &subnet_id in subnet_ids {
        let subnet_weight = OverwatchSubnetWeights::<T>::get(overwatch_epoch, subnet_id)
            .expect("every seeded subnet receives an aggregate weight");
        // Every reveal is exactly 0.5. A normalized subset of revealer stake cannot aggregate
        // above that submitted value, even when the settlement does not contain every node/subnet
        // pair.
        assert!(subnet_weight > 0 && subnet_weight <= percentage_factor / 2);
    }
}

pub fn insert_subnet<T: Config>(id: u32, state: SubnetState, epoch: u32) {
    let data = new_subnet_data::<T>(id, state, epoch);
    SubnetsData::<T>::insert(id, data);
}

pub fn new_subnet_data<T: Config>(id: u32, state: SubnetState, epoch: u32) -> SubnetData {
    let (consensus_eligible_from_subnet_epoch, pause) = match state {
        SubnetState::Registered => (None, None),
        SubnetState::Active => (Some(epoch), None),
        SubnetState::Paused => (
            None,
            Some(SubnetPauseData {
                started_global_epoch: epoch,
                started_subnet_epoch: epoch,
            }),
        ),
    };

    SubnetData {
        id,
        friendly_id: id,
        name: vec![],
        repo: vec![],
        description: vec![],
        misc: vec![],
        consensus_mechanism: Default::default(),
        state,
        consensus_eligible_from_subnet_epoch,
        pause,
    }
}

/// Replace the variable-size fields of an existing subnet with their configured maxima while
/// preserving the name/repository indexes that production maintains alongside `SubnetsData`.
fn max_fill_benchmark_subnet_data<T: Config>(subnet_id: u32) {
    let mut subnet = SubnetsData::<T>::get(subnet_id).expect("benchmark subnet must exist");
    SubnetName::<T>::remove(&subnet.name);
    SubnetRepo::<T>::remove(&subnet.repo);

    let tagged_max = |fill: u8, len: u32| {
        let mut value = vec![fill; len as usize];
        for (slot, byte) in value.iter_mut().zip(subnet_id.to_le_bytes()) {
            *slot = byte;
        }
        value
    };
    // Keep every uniqueness-constrained field distinct across a multi-subnet fixture while still
    // decoding the configured maximum length.
    subnet.name = tagged_max(41, T::MaxVectorLength::get());
    subnet.repo = tagged_max(42, T::MaxUrlLength::get());
    subnet.description = tagged_max(43, T::MaxVectorLength::get());
    subnet.misc = tagged_max(44, T::MaxVectorLength::get());

    SubnetName::<T>::insert(&subnet.name, subnet_id);
    SubnetRepo::<T>::insert(&subnet.repo, subnet_id);
    SubnetsData::<T>::insert(subnet_id, subnet);
}

/// Insert one economically-live subnet with a coherent physical-slot assignment.
///
/// The delegate-stake minimum scans complete `SubnetsData` values and then reads the slot and
/// delegate balance of every live member. Keep the variable-size metadata maximal so benchmarks
/// cover the proof and decode cost of the bounded live cohort, not merely its key count.
fn insert_live_benchmark_subnet<T: Config>(subnet_id: u32, slot: u32, balance: u128) {
    assert!(slot >= T::DesignatedEpochSlots::get());
    assert!(slot < T::EpochLength::get());

    insert_subnet::<T>(subnet_id, SubnetState::Active, 0);
    max_fill_benchmark_subnet_data::<T>(subnet_id);
    SubnetSlot::<T>::insert(subnet_id, slot);
    SlotAssignment::<T>::insert(slot, subnet_id);
    TotalSubnetDelegateStakeBalance::<T>::insert(subnet_id, balance);

    let subnet = SubnetsData::<T>::get(subnet_id).expect("live benchmark subnet exists");
    let subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
    assert!(Network::<T>::_is_subnet_active_and_live(
        &subnet,
        subnet_epoch
    ));
}

/// Build a production-bounded active/live cohort occupying consecutive physical subnet slots.
fn seed_live_subnet_delegate_stake_cohort<T: Config>(
    subnet_count: u32,
    mut balance_for_index: impl FnMut(u32) -> u128,
) -> Vec<u32> {
    let epoch_length = T::EpochLength::get();
    let first_slot = T::DesignatedEpochSlots::get();
    assert!(epoch_length > 0);
    assert!(subnet_count <= T::MaxPhysicalSubnetsUpperBound::get());
    assert!(first_slot
        .checked_add(subnet_count)
        .is_some_and(|end| end <= epoch_length));

    // Two complete epochs make eligibility epoch zero live even for the last physical slot.
    let benchmark_block = epoch_length
        .checked_mul(2)
        .expect("benchmark block fits u32");
    frame_system::Pallet::<T>::set_block_number(u32_to_block::<T>(benchmark_block));

    let first_subnet_id = T::InitialSubnetUid::get()
        .checked_add(1)
        .expect("benchmark subnet identifier fits u32");
    let mut assigned_slots = BTreeSet::new();
    let mut subnet_ids = Vec::with_capacity(subnet_count as usize);
    for index in 0..subnet_count {
        let subnet_id = first_subnet_id
            .checked_add(index)
            .expect("benchmark subnet identifier fits u32");
        let slot = first_slot
            .checked_add(index)
            .expect("benchmark subnet slot fits u32");
        insert_live_benchmark_subnet::<T>(subnet_id, slot, balance_for_index(index));
        assert!(assigned_slots.insert(slot));
        subnet_ids.push(subnet_id);
    }

    AssignedSlots::<T>::put(assigned_slots);
    TotalSubnets::<T>::put(subnet_count);
    TotalActiveSubnets::<T>::put(subnet_count);
    if let Some(&last_subnet_id) = subnet_ids.last() {
        TotalSubnetUids::<T>::put(last_subnet_id);
    }

    subnet_ids
}

/// Populate every non-target physical slot from the maximal removal fixture as a live subnet.
/// This makes `activate_subnet` cover the same bounded delegation scan that it invokes through
/// `can_subnet_be_active` before entering its maximal expired-enactment cleanup branch.
fn seed_live_delegation_peers_for_activation<T: Config>(target_subnet_id: u32) -> Vec<u32> {
    let assigned_slots = AssignedSlots::<T>::get();
    assert_eq!(
        assigned_slots.len() as u32,
        T::MaxPhysicalSubnetsUpperBound::get()
    );

    let mut peer_ids = Vec::with_capacity(assigned_slots.len().saturating_sub(1));
    for slot in assigned_slots.iter().copied() {
        let subnet_id = SlotAssignment::<T>::get(slot)
            .expect("every assigned benchmark slot has a reverse assignment");
        if subnet_id == target_subnet_id {
            continue;
        }
        assert!(!SubnetsData::<T>::contains_key(subnet_id));
        insert_live_benchmark_subnet::<T>(subnet_id, slot, DEFAULT_DELEGATE_STAKE_TO_BE_ADDED);
        peer_ids.push(subnet_id);
    }

    assert_eq!(
        peer_ids.len() as u32,
        T::MaxPhysicalSubnetsUpperBound::get().saturating_sub(1)
    );
    TotalSubnets::<T>::put(T::MaxPhysicalSubnetsUpperBound::get());
    TotalActiveSubnets::<T>::put(peer_ids.len() as u32);

    peer_ids
}

fn maximum_valid_benchmark_multiaddr<T: Config>() -> NetworkBytes<T> {
    let maximum_length = T::MaxVectorLength::get().min(T::MaxUrlLength::get()) as usize;
    let peer_payload = vec![1u8; 32];

    // A length-prefixed DNS segment can occupy every otherwise-unused byte while retaining the
    // production requirement that the address end in a P2P segment.
    for dns_length in (0..=maximum_length).rev() {
        let mut bytes = Vec::new();
        encode_varint(DNS4, &mut bytes);
        encode_varint(dns_length as u64, &mut bytes);
        bytes.resize(bytes.len().saturating_add(dns_length), b'a');
        encode_varint(P2P, &mut bytes);
        encode_varint(peer_payload.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer_payload);
        if bytes.len() <= maximum_length && Multiaddr::verify(&bytes).is_ok() {
            return bytes
                .try_into()
                .expect("maximum valid multiaddr fits NetworkBytes");
        }
    }

    panic!("configured network byte bound cannot hold a valid multiaddr")
}

fn max_fill_emergency_subnet_election_data<T: Config>(subnet_id: u32) {
    MaxEmergencySubnetNodes::<T>::set(T::MaxEmergencySubnetNodesUpperBound::get());
    let maximum_reputation_factors = SubnetReputationFactors {
        absent_decrease: u128::MAX,
        included_increase: u128::MAX,
        below_min_weight_decrease: u128::MAX,
        non_attestor_decrease: u128::MAX,
        non_consensus_attestor_decrease: u128::MAX,
        validator_absent_decrease: u128::MAX,
        validator_non_consensus_decrease: u128::MAX,
    };
    EmergencySubnetNodeElectionData::<T>::insert(
        subnet_id,
        EmergencySubnetValidatorData {
            subnet_node_ids: (1..=T::MaxEmergencySubnetNodesUpperBound::get()).collect(),
            target_emergency_validators_epochs: u32::MAX,
            max_emergency_validators_epoch: u32::MAX,
            total_epochs: u32::MAX,
            activated: true,
            started_subnet_epoch: Network::<T>::get_current_subnet_epoch_as_u32(subnet_id),
            reputation_factors: maximum_reputation_factors,
            min_subnet_node_reputation: u128::MAX,
            min_weight_decrease_reputation_threshold: u128::MAX,
        },
    );
}

fn maximum_benchmark_bootnodes<T: Config>() -> BTreeMap<PeerId, NetworkBytes<T>> {
    let maximum_multiaddr = maximum_valid_benchmark_multiaddr::<T>();
    let mut bootnodes = BTreeMap::new();
    for index in 0..T::MaxBootnodesUpperBound::get() {
        let mut peer_bytes = vec![b'z'; 128usize.min(T::MaxVectorLength::get() as usize)];
        assert!(peer_bytes.len() >= 32);
        for (slot, byte) in peer_bytes.iter_mut().skip(1).zip(index.to_le_bytes()) {
            *slot = byte;
        }
        let peer_id = PeerId(peer_bytes);
        assert!(Network::<T>::validate_peer_id(&peer_id));
        bootnodes.insert(peer_id, maximum_multiaddr.clone());
    }
    assert_eq!(bootnodes.len() as u32, T::MaxBootnodesUpperBound::get());
    bootnodes
}

/// Fill the variable-size keyed values removed by `do_remove_subnet` independently of its node
/// prefix dimensions. `include_emergency` is false for the Registered lifecycle, where emergency
/// election state is unreachable, and true for a maximal Paused lifecycle fixture.
fn max_fill_remove_subnet_keyed_state<T: Config>(subnet_id: u32, include_emergency: bool) {
    MaxBootnodes::<T>::set(T::MaxBootnodesUpperBound::get());
    SubnetBootnodes::<T>::insert(subnet_id, maximum_benchmark_bootnodes::<T>());

    MaxSubnetBootnodeAccess::<T>::set(T::MaxSubnetBootnodeAccessUpperBound::get());
    let bootnode_access: BTreeSet<T::AccountId> = (0..T::MaxSubnetBootnodeAccessUpperBound::get())
        .map(|index| get_account::<T>("remove-subnet-bootnode-access", index))
        .collect();
    assert_eq!(
        bootnode_access.len() as u32,
        T::MaxSubnetBootnodeAccessUpperBound::get()
    );
    SubnetBootnodeAccess::<T>::insert(subnet_id, bootnode_access);

    if include_emergency {
        max_fill_emergency_subnet_election_data::<T>(subnet_id);
    }

    // `free_slot_of_subnet` decodes and rewrites the complete physical-slot set. Retain the
    // target's real assignment and fill every other reachable slot with coherent reverse indexes.
    let target_slot = SubnetSlot::<T>::get(subnet_id).expect("benchmark subnet has a slot");
    let mut assigned_slots = BTreeSet::from([target_slot]);
    for slot in T::DesignatedEpochSlots::get()..T::EpochLength::get() {
        if assigned_slots.len() as u32 >= T::MaxPhysicalSubnetsUpperBound::get() {
            break;
        }
        if assigned_slots.insert(slot) {
            let synthetic_subnet_id = 90_000u32.saturating_add(slot);
            SubnetSlot::<T>::insert(synthetic_subnet_id, slot);
            SlotAssignment::<T>::insert(slot, synthetic_subnet_id);
        }
    }
    assert_eq!(
        assigned_slots.len() as u32,
        T::MaxPhysicalSubnetsUpperBound::get()
    );
    // Restore the target reverse entry in case the generic fill encountered its slot.
    SlotAssignment::<T>::insert(target_slot, subnet_id);
    AssignedSlots::<T>::put(assigned_slots);
}

fn max_fill_initial_validator_cleanup_state<T: Config>(subnet_id: u32) {
    let identity_count = T::MaxRegisteredNodesUpperBound::get();
    assert!(identity_count > 0);
    let maximum_nodes = T::MaxSubnetNodesUpperBound::get();
    let whitelist: BTreeMap<u32, u32> = (1..=identity_count)
        .map(|validator_id| {
            let registrations = maximum_nodes
                .saturating_sub(validator_id - 1)
                .saturating_add(identity_count - 1)
                / identity_count;
            (validator_id, registrations.max(1))
        })
        .collect();
    let registration_counts = whitelist
        .iter()
        .map(|(&validator_id, &registrations)| (validator_id, registrations))
        .collect::<BTreeMap<_, _>>();
    NodeRegistrationInitialValidatorIds::<T>::insert(subnet_id, whitelist);
    InitialValidatorData::<T>::insert(subnet_id, registration_counts);
}

fn max_fill_overwatch_node_index<T: Config>(overwatch_node_id: u32) {
    let mut index = BTreeMap::new();
    for subnet_id in 1..=T::MaxPhysicalSubnetsUpperBound::get() {
        let mut bytes = vec![7u8; 128];
        for (slot, byte) in bytes.iter_mut().zip(
            overwatch_node_id
                .to_le_bytes()
                .into_iter()
                .chain(subnet_id.to_le_bytes()),
        ) {
            *slot = byte;
        }
        let peer_id = PeerId(bytes);
        PeerIdOverwatchNodeId::<T>::insert(subnet_id, &peer_id, overwatch_node_id);
        index.insert(subnet_id, peer_id);
    }
    OverwatchNodeIndex::<T>::insert(overwatch_node_id, index);
}

/// Seed every bounded lifecycle surface touched by owner and collective Overwatch removal.
///
/// The target is the unique maximum-stake participant and submits a different raw weight from
/// the rest of the 64-node by 17-subnet cohort. Removing it therefore forces a full globally
/// normalized effective-signal recomputation in addition to current and pending row cleanup.
/// `sole_pending` also forces explicit empty-epoch finalization after the shared removal.
fn seed_max_overwatch_removal_lifecycle<T: Config>(
    target_node_id: u32,
    sole_pending: bool,
) -> (u32, u32, u64) {
    let max_nodes = T::MaxOverwatchNodesUpperBound::get();
    let max_subnets = T::MaxPhysicalSubnetsUpperBound::get();
    assert_eq!(max_nodes, MAX_OVERWATCH_NODES_BENCHMARK_DOMAIN);
    assert_eq!(max_subnets, MAX_PHYSICAL_SUBNETS_BENCHMARK_DOMAIN);

    let source_epoch = 0u32;
    let pending_epoch = 1u32;
    let active_epoch = 2u32;
    let initial_revision = 41u64;
    let percentage_factor = Network::<T>::percentage_factor_as_u128();
    let base_stake = OverwatchMinStakeBalance::<T>::get();
    let subnet_ids = (1..=max_subnets).collect::<Vec<_>>();

    let mut snapshot_nodes = BTreeMap::new();
    let mut retained_nodes = BTreeMap::new();
    let mut total_live_stake = 0u128;
    for node_id in 1..=max_nodes {
        if node_id != target_node_id {
            let validator_id = node_id;
            let (_coldkey, hotkey) = ensure_validator::<T>(validator_id);
            OverwatchValidatorWhitelist::<T>::insert(validator_id, ());
            OverwatchNodes::<T>::insert(node_id, ());
            OverwatchNodeIdHotkey::<T>::insert(node_id, hotkey);
            OverwatchNodeValidatorId::<T>::insert(node_id, validator_id);
            ValidatorOverwatchNodeId::<T>::insert(validator_id, node_id);
        }

        let raw_weight = if node_id == target_node_id {
            percentage_factor
        } else {
            percentage_factor / 2
        };
        let reveals: BoundedBTreeMap<u32, u128, T::MaxPhysicalSubnetsUpperBound> = subnet_ids
            .iter()
            .copied()
            .map(|subnet_id| (subnet_id, raw_weight))
            .collect::<BTreeMap<_, _>>()
            .try_into()
            .expect("maximum removal reveal row fits its runtime bound");
        let commits: BoundedBTreeMap<u32, T::Hash, T::MaxPhysicalSubnetsUpperBound> = subnet_ids
            .iter()
            .copied()
            .map(|subnet_id| (subnet_id, T::Hashing::hash_of(&(node_id, subnet_id))))
            .collect::<BTreeMap<_, _>>()
            .try_into()
            .expect("maximum removal commit row fits its runtime bound");
        OverwatchCommits::<T>::insert(active_epoch, node_id, commits);
        OverwatchReveals::<T>::insert(active_epoch, node_id, reveals.clone());

        let stake_multiplier = if node_id == target_node_id {
            max_nodes.saturating_add(1)
        } else {
            node_id
        };
        let stake = base_stake
            .checked_mul(stake_multiplier as u128)
            .expect("maximum removal stake fits u128");
        OverwatchNodeStakeBalance::<T>::insert(node_id, stake);
        total_live_stake = total_live_stake
            .checked_add(stake)
            .expect("maximum removal total stake fits u128");
        if !sole_pending || node_id == target_node_id {
            OverwatchReveals::<T>::insert(pending_epoch, node_id, reveals.clone());
            snapshot_nodes.insert(node_id, OverwatchNodeSettlementSnapshot { stake });
        }
        retained_nodes.insert(
            node_id,
            LatestOverwatchNodeSignalInput::<T> { stake, reveals },
        );
    }

    CurrentOverwatchEpoch::<T>::put(active_epoch);
    TotalOverwatchNodeUids::<T>::set(max_nodes);
    TotalOverwatchNodes::<T>::set(max_nodes);
    TotalOverwatchNodeStakeBalance::<T>::set(total_live_stake);
    ActiveOverwatchRevealStats::<T>::put(OverwatchRevealStats::<T> {
        records: max_nodes.saturating_mul(max_subnets),
        subnet_revealer_counts: subnet_ids
            .iter()
            .copied()
            .map(|subnet_id| (subnet_id, max_nodes))
            .collect::<BTreeMap<_, _>>()
            .try_into()
            .expect("maximum removal subnet counts fit their runtime bound"),
    });
    PendingOverwatchSettlement::<T>::put(PendingOverwatchSettlementData {
        epoch: pending_epoch,
        reveal_records: if sole_pending {
            max_subnets
        } else {
            max_nodes.saturating_mul(max_subnets)
        },
    });
    OverwatchEpochSettlementSnapshots::<T>::insert(
        pending_epoch,
        OverwatchEpochSettlementSnapshot::<T> {
            stake_weight_factor: DefaultOverwatchStakeWeightFactor::get(),
            reward_budget: T::OverwatchEpochEmissions::get(),
            nodes: snapshot_nodes
                .try_into()
                .expect("maximum removal settlement snapshot fits its runtime bound"),
        },
    );

    let retained_inputs = LatestFinalizedOverwatchSignalInput::<T> {
        source_epoch,
        stake_weight_factor: DefaultOverwatchStakeWeightFactor::get(),
        nodes: retained_nodes
            .try_into()
            .expect("maximum retained removal inputs fit their runtime bound"),
    };
    let derived = Network::<T>::derive_overwatch_signal(&retained_inputs)
        .expect("maximum retained removal inputs derive a bounded signal");
    LastFinalizedOverwatchEpoch::<T>::put(source_epoch);
    LatestFinalizedOverwatchSignalInputs::<T>::put(retained_inputs);
    LatestEffectiveOverwatchSignal::<T>::put(EffectiveOverwatchSignal::<T> {
        source_epoch,
        valid: true,
        subnet_weights: derived.subnet_weights,
    });
    LatestOverwatchSignalRevision::<T>::put(initial_revision);

    (active_epoch, pending_epoch, initial_revision)
}

fn assert_max_overwatch_removal_lifecycle<T: Config>(
    target_node_id: u32,
    active_epoch: u32,
    pending_epoch: u32,
    initial_revision: u64,
    sole_pending: bool,
) {
    assert!(OverwatchCommits::<T>::get(active_epoch, target_node_id).is_empty());
    assert!(OverwatchReveals::<T>::get(active_epoch, target_node_id).is_empty());
    assert!(OverwatchReveals::<T>::get(pending_epoch, target_node_id).is_empty());
    let retained = LatestFinalizedOverwatchSignalInputs::<T>::get()
        .expect("valid latest inputs remain after removal");
    assert!(!retained.nodes.contains_key(&target_node_id));
    let derived = Network::<T>::derive_overwatch_signal(&retained)
        .expect("purged retained inputs remain derivable");
    let cache = LatestEffectiveOverwatchSignal::<T>::get()
        .expect("effective cache remains available after recomputation");
    assert!(cache.valid);
    assert_eq!(cache.subnet_weights, derived.subnet_weights);
    if sole_pending {
        assert!(PendingOverwatchSettlement::<T>::get().is_none());
        assert!(OverwatchEpochSettlementSnapshots::<T>::get(pending_epoch).is_none());
        assert_eq!(retained.source_epoch, pending_epoch);
        assert!(retained.nodes.is_empty());
        assert_eq!(cache.source_epoch, pending_epoch);
        assert!(cache.subnet_weights.is_empty());
        assert_eq!(LastFinalizedOverwatchEpoch::<T>::get(), Some(pending_epoch));
        assert_eq!(
            LatestOverwatchSignalRevision::<T>::get(),
            initial_revision.saturating_add(2)
        );
    } else {
        assert!(!OverwatchEpochSettlementSnapshots::<T>::get(pending_epoch)
            .expect("pending snapshot remains for the surviving cohort")
            .nodes
            .contains_key(&target_node_id));
        assert_eq!(
            LatestOverwatchSignalRevision::<T>::get(),
            initial_revision.saturating_add(1)
        );
    }
}

pub fn insert_subnet_node<T: Config>(
    subnet_id: u32,
    node_id: u32,
    coldkey_n: u32,
    _hotkey_n: u32,
    peer_n: u32,
    class: SubnetNodeClass,
    start_epoch: u32,
) {
    let validator_id = coldkey_n;
    let (_coldkey, hotkey) = ensure_validator::<T>(validator_id);
    let peer_info = PeerInfo::<T> {
        peer_id: peer(peer_n),
        multiaddr: get_multiaddr::<T>(Some(subnet_id), Some(node_id), None),
    };

    SubnetNodesData::<T>::insert(
        subnet_id,
        node_id,
        SubnetNode::<T> {
            id: node_id,
            validator_id,
            peer_info: Some(peer_info),
            bootnode_peer_info: None,
            client_peer_info: None,
            classification: SubnetNodeClassification {
                node_class: class,
                start_epoch,
            },
            unique: Some(BoundedVec::new()),
            non_unique: Some(BoundedVec::new()),
        },
    );
    SubnetNodeValidatorId::<T>::insert(subnet_id, node_id, validator_id);
    SubnetNodeIdHotkey::<T>::insert(subnet_id, node_id, hotkey);
    TotalActiveSubnetNodes::<T>::mutate(subnet_id, |n: &mut u32| *n += 1);
    TotalActiveNodes::<T>::mutate(|n: &mut u32| *n += 1);
}

fn seed_common_remove_subnet_node_state<T: Config>(
    subnet_id: u32,
    subnet_node_id: u32,
    active: bool,
) -> NetworkBytes<T> {
    fn max_bytes<T: Config>(subnet_id: u32, node_id: u32, role: u8) -> NetworkBytes<T> {
        let mut bytes = vec![role; T::MaxVectorLength::get() as usize];
        let identity = subnet_id
            .to_le_bytes()
            .into_iter()
            .chain(node_id.to_le_bytes());
        for (slot, byte) in bytes.iter_mut().zip(identity) {
            *slot = byte;
        }
        bytes.try_into().expect("bounded benchmark payload")
    }

    fn max_peer(subnet_id: u32, node_id: u32, role: u8) -> PeerId {
        let mut bytes = vec![role; 128];
        let identity = subnet_id
            .to_le_bytes()
            .into_iter()
            .chain(node_id.to_le_bytes());
        for (slot, byte) in bytes.iter_mut().zip(identity) {
            *slot = byte;
        }
        PeerId(bytes)
    }

    let mut subnet_node = if active {
        SubnetNodesData::<T>::get(subnet_id, subnet_node_id)
    } else {
        RegisteredSubnetNodesData::<T>::get(subnet_id, subnet_node_id)
    };

    // Remove the compact registration keys before replacing them with maximum-sized distinct
    // payloads; otherwise the cleanup prefix would contain impossible duplicate indexes.
    if let Some(peer_info) = &subnet_node.peer_info {
        PeerIdSubnetNodeId::<T>::remove(subnet_id, &peer_info.peer_id);
        if let Some(multiaddr) = &peer_info.multiaddr {
            MultiaddrSubnetNodeId::<T>::remove(subnet_id, multiaddr);
        }
    }
    if let Some(peer_info) = &subnet_node.bootnode_peer_info {
        BootnodePeerIdSubnetNodeId::<T>::remove(subnet_id, &peer_info.peer_id);
        if let Some(multiaddr) = &peer_info.multiaddr {
            MultiaddrSubnetNodeId::<T>::remove(subnet_id, multiaddr);
        }
    }
    if let Some(peer_info) = &subnet_node.client_peer_info {
        ClientPeerIdSubnetNodeId::<T>::remove(subnet_id, &peer_info.peer_id);
        if let Some(multiaddr) = &peer_info.multiaddr {
            MultiaddrSubnetNodeId::<T>::remove(subnet_id, multiaddr);
        }
    }

    let peer_info = PeerInfo::<T> {
        peer_id: max_peer(subnet_id, subnet_node_id, 1),
        multiaddr: Some(max_bytes::<T>(subnet_id, subnet_node_id, 11)),
    };
    let bootnode_peer_info = PeerInfo::<T> {
        peer_id: max_peer(subnet_id, subnet_node_id, 2),
        multiaddr: Some(max_bytes::<T>(subnet_id, subnet_node_id, 12)),
    };
    let client_peer_info = PeerInfo::<T> {
        peer_id: max_peer(subnet_id, subnet_node_id, 3),
        multiaddr: Some(max_bytes::<T>(subnet_id, subnet_node_id, 13)),
    };
    let unique = max_bytes::<T>(subnet_id, subnet_node_id, 21);
    subnet_node.peer_info = Some(peer_info.clone());
    subnet_node.bootnode_peer_info = Some(bootnode_peer_info.clone());
    subnet_node.client_peer_info = Some(client_peer_info.clone());
    subnet_node.unique = Some(unique.clone());
    subnet_node.non_unique = Some(max_bytes::<T>(subnet_id, subnet_node_id, 22));

    if active {
        SubnetNodesData::<T>::insert(subnet_id, subnet_node_id, &subnet_node);
    } else {
        RegisteredSubnetNodesData::<T>::insert(subnet_id, subnet_node_id, &subnet_node);
        SubnetNodeQueue::<T>::mutate(subnet_id, |queue| {
            if let Some(queued_node) = queue.iter_mut().find(|node| node.id == subnet_node_id) {
                *queued_node = subnet_node.clone();
            }
        });
    }

    PeerIdSubnetNodeId::<T>::insert(subnet_id, &peer_info.peer_id, subnet_node_id);
    BootnodePeerIdSubnetNodeId::<T>::insert(subnet_id, &bootnode_peer_info.peer_id, subnet_node_id);
    ClientPeerIdSubnetNodeId::<T>::insert(subnet_id, &client_peer_info.peer_id, subnet_node_id);
    MultiaddrSubnetNodeId::<T>::insert(subnet_id, peer_info.multiaddr.unwrap(), subnet_node_id);
    MultiaddrSubnetNodeId::<T>::insert(
        subnet_id,
        bootnode_peer_info.multiaddr.unwrap(),
        subnet_node_id,
    );
    MultiaddrSubnetNodeId::<T>::insert(
        subnet_id,
        client_peer_info.multiaddr.unwrap(),
        subnet_node_id,
    );
    UniqueParamSubnetNodeId::<T>::insert(subnet_id, &unique, subnet_node_id);
    SubnetNodeIdleConsecutiveEpochs::<T>::insert(subnet_id, subnet_node_id, 1);
    SubnetNodeConsecutiveIncludedEpochs::<T>::insert(subnet_id, subnet_node_id, 1);

    unique
}

fn seed_validator_owned_nodes<T: Config>(
    validator_id: u32,
    subnet_id: u32,
    subnet_node_id: u32,
    n: u32,
) {
    assert!((1..=T::MaxValidatorNodesUpperBound::get()).contains(&n));

    let mut validator_subnet_nodes: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut target_nodes = BTreeSet::new();
    target_nodes.insert(subnet_node_id);
    validator_subnet_nodes.insert(subnet_id, target_nodes);

    // Production permits at most one entry for each currently addressable subnet. Spread the
    // remaining nodes over every available non-target subnet key while keeping the cumulative
    // ownership invariant at exactly `n`.
    let external_subnet_count = n.saturating_sub(1).min(
        T::MaxPhysicalSubnetsUpperBound::get()
            .saturating_sub(1)
            .max(1),
    );
    for external_subnet_index in 0..external_subnet_count {
        let external_subnet_id = 20_000 + external_subnet_index;
        SubnetsData::<T>::insert(
            external_subnet_id,
            new_subnet_data::<T>(external_subnet_id, SubnetState::Active, 0),
        );
    }
    for i in 0..n.saturating_sub(1) {
        let external_subnet_id = 20_000 + (i % external_subnet_count.max(1));
        validator_subnet_nodes
            .entry(external_subnet_id)
            .or_default()
            .insert(10_000 + i);
    }

    let mut weights = BTreeMap::new();
    for (owned_subnet_id, node_ids) in &validator_subnet_nodes {
        for owned_node_id in node_ids {
            // A deliberately non-normalized full map makes removal traverse and rewrite every
            // surviving ownership entry.
            weights.insert((*owned_subnet_id, *owned_node_id), 1u128);
        }
    }

    assert_eq!(weights.len() as u32, n);
    ValidatorSubnetNodes::<T>::insert(validator_id, validator_subnet_nodes);
    TotalValidatorNodes::<T>::insert(validator_id, n);
    ValidatorNodeDelegateStakeWeights::<T>::insert(validator_id, weights);
}

fn seed_active_remove_subnet_node_state<T: Config>(
    n: u32,
    e: u32,
    m: u32,
) -> (u32, u32, u32, NetworkBytes<T>) {
    assert!((3..=T::MaxSubnetNodesUpperBound::get()).contains(&e));
    assert!((MinSubnetNodes::<T>::get()..=e).contains(&m));

    NewRegistrationCostMultiplier::<T>::set(1_000_000_000_000_000_000);
    MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());

    let path: Vec<u8> = b"subnet-name-0".to_vec();
    build_activated_subnet::<T>(
        path.clone(),
        0,
        e,
        DEFAULT_DEPOSIT_AMOUNT,
        DEFAULT_SUBNET_NODE_STAKE,
    );
    let subnet_id = SubnetName::<T>::get(path).unwrap();

    // Emergency validator IDs are sorted by production validation. Use the lowest node ID as the
    // target so retaining it forces maximum compaction, while arranging the independently stored
    // election slots with that same target second-last. Such slot permutations are reachable via
    // swap-removals followed by later insertions.
    let subnet_node_id = 1;
    let mut election_slots: Vec<u32> = (2..e).collect();
    election_slots.push(subnet_node_id);
    election_slots.push(e);
    assert_eq!(election_slots.len() as u32, e);
    SubnetNodeElectionSlots::<T>::insert(subnet_id, &election_slots);
    for (index, node_id) in election_slots.iter().copied().enumerate() {
        NodeSlotIndex::<T>::insert(subnet_id, node_id, index as u32);
    }

    let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id).unwrap();
    let unique = seed_common_remove_subnet_node_state::<T>(subnet_id, subnet_node_id, true);
    seed_validator_owned_nodes::<T>(validator_id, subnet_id, subnet_node_id, n);

    let emergency_node_ids: Vec<u32> = (1..=m).collect();
    EmergencySubnetNodeElectionData::<T>::insert(
        subnet_id,
        EmergencySubnetValidatorData {
            subnet_node_ids: emergency_node_ids,
            target_emergency_validators_epochs: 0,
            max_emergency_validators_epoch: 0,
            total_epochs: 0,
            ..Default::default()
        },
    );

    (subnet_id, subnet_node_id, validator_id, unique)
}

fn seed_registered_remove_subnet_node_state<T: Config>(
    n: u32,
    r: u32,
) -> (u32, u32, u32, NetworkBytes<T>) {
    NewRegistrationCostMultiplier::<T>::set(1_000_000_000_000_000_000);

    build_activated_subnet::<T>(
        DEFAULT_SUBNET_NAME.into(),
        0,
        MinSubnetNodes::<T>::get(),
        DEFAULT_DEPOSIT_AMOUNT,
        DEFAULT_SUBNET_NODE_STAKE,
    );
    let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

    build_registered_subnet_nodes::<T>(
        subnet_id,
        MinSubnetNodes::<T>::get() + 1,
        MinSubnetNodes::<T>::get() + 1 + r,
        DEFAULT_DEPOSIT_AMOUNT,
        DEFAULT_SUBNET_NODE_STAKE,
        false,
    );

    let queue_before = SubnetNodeQueue::<T>::get(subnet_id);
    assert_eq!(queue_before.len() as u32, r);
    // `retain` scans every entry regardless of target position. Removing the first maximum-sized
    // entry additionally compacts every survivor and is the reachable worst case.
    let remove_subnet_node_id = queue_before
        .first()
        .expect("registered queue is non-empty")
        .id;
    assert!(RegisteredSubnetNodesData::<T>::contains_key(
        subnet_id,
        remove_subnet_node_id
    ));

    let mut target_unique = None;
    for queued_node in queue_before {
        let unique = seed_common_remove_subnet_node_state::<T>(subnet_id, queued_node.id, false);
        if queued_node.id == remove_subnet_node_id {
            target_unique = Some(unique);
        }
    }
    let unique = target_unique.expect("target queue node must be seeded");
    let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, remove_subnet_node_id).unwrap();
    seed_validator_owned_nodes::<T>(validator_id, subnet_id, remove_subnet_node_id, n);

    assert_eq!(TotalValidatorNodes::<T>::get(validator_id), n);
    assert_eq!(
        ValidatorNodeDelegateStakeWeights::<T>::get(validator_id).len() as u32,
        n
    );

    (subnet_id, remove_subnet_node_id, validator_id, unique)
}

fn seed_remove_subnet_cleanup_state<T: Config>(
    subnet_id: u32,
    active_nodes: u32,
    registered_nodes: u32,
    overwatch_nodes: u32,
) {
    let mut subnet = SubnetsData::<T>::get(subnet_id).expect("benchmark subnet must exist");
    SubnetName::<T>::remove(&subnet.name);
    SubnetRepo::<T>::remove(&subnet.repo);
    subnet.name = vec![41u8; T::MaxVectorLength::get() as usize];
    subnet.repo = vec![42u8; T::MaxUrlLength::get() as usize];
    subnet.description = vec![43u8; T::MaxVectorLength::get() as usize];
    subnet.misc = vec![44u8; T::MaxVectorLength::get() as usize];
    SubnetName::<T>::insert(&subnet.name, subnet_id);
    SubnetRepo::<T>::insert(&subnet.repo, subnet_id);
    SubnetsData::<T>::insert(subnet_id, subnet);

    let mut target_nodes: Vec<(u32, u32)> = SubnetNodesData::<T>::iter_prefix(subnet_id)
        .map(|(node_id, node)| (node.validator_id, node_id))
        .collect();
    target_nodes.extend(
        RegisteredSubnetNodesData::<T>::iter_prefix(subnet_id)
            .map(|(node_id, node)| (node.validator_id, node_id)),
    );
    assert_eq!(
        target_nodes.len() as u32,
        active_nodes.saturating_add(registered_nodes)
    );

    // Exercise every optional node prefix cleared by clean_subnet_nodes, in addition to the peer,
    // multiaddr, hotkey, validator, reputation and election entries created by registration.
    for (index, (_, node_id)) in target_nodes.iter().enumerate() {
        seed_common_remove_subnet_node_state::<T>(
            subnet_id,
            *node_id,
            (index as u32) < active_nodes,
        );
    }

    for overwatch_node_id in 1..=overwatch_nodes {
        let mut bytes = vec![31u8; 128];
        for (slot, byte) in bytes.iter_mut().zip(
            overwatch_node_id
                .to_le_bytes()
                .into_iter()
                .chain(subnet_id.to_le_bytes()),
        ) {
            *slot = byte;
        }
        PeerIdOverwatchNodeId::<T>::insert(subnet_id, PeerId(bytes), overwatch_node_id);
    }
    TotalOverwatchNodes::<T>::set(overwatch_nodes);
}

pub fn prepare_overwatch_validator<T: Config>(validator_id: u32) {
    let (_coldkey, _hotkey) = ensure_validator::<T>(validator_id);
    OverwatchValidatorWhitelist::<T>::insert(validator_id, ());
    CurrentOverwatchEpoch::<T>::put(1);
    OverwatchEpochStartBlock::<T>::put(get_current_block_as_u32::<T>());
}

fn register_benchmark_overwatch_node<T: Config>(
    validator_id: u32,
    stake_to_be_added: u128,
) -> (u32, T::AccountId) {
    prepare_overwatch_validator::<T>(validator_id);
    let coldkey = ValidatorColdkey::<T>::get(validator_id).unwrap();
    fund_account::<T>(
        &coldkey,
        stake_to_be_added.saturating_add(DEFAULT_DEPOSIT_AMOUNT),
    );
    assert_ok!(Network::<T>::register_overwatch_node(
        RawOrigin::Signed(coldkey.clone()).into(),
        stake_to_be_added,
    ));
    (TotalOverwatchNodeUids::<T>::get(), coldkey)
}

// fn get_subnet_node_data(start: u32, end: u32) -> Vec<SubnetNodeConsensusData> {
//   // initialize peer consensus data array
//   let mut subnet_node_data: Vec<SubnetNodeConsensusData> = Vec::new();
//   for n in start..end {
//     let peer_subnet_node_data: SubnetNodeConsensusData = SubnetNodeConsensusData {
//       peer_id: peer(n),
//       score: DEFAULT_SCORE,
//     };
//     subnet_node_data.push(peer_subnet_node_data);
//   }
//   subnet_node_data
// }

pub fn get_subnet_node_consensus_data<T: frame_system::Config>(
    subnets: u32,
    max_subnet_nodes: u32,
    start: u32,
    end: u32,
) -> Vec<SubnetNodeConsensusData> {
    // initialize peer consensus data array
    let mut subnet_node_data: Vec<SubnetNodeConsensusData> = Vec::new();
    for n in start..end {
        let peer_subnet_node_data: SubnetNodeConsensusData = SubnetNodeConsensusData {
            subnet_node_id: n + 1,
            score: DEFAULT_SCORE,
        };

        subnet_node_data.push(peer_subnet_node_data);
    }
    subnet_node_data
}

pub fn u32_to_block<T: frame_system::Config>(input: u32) -> BlockNumberFor<T> {
    input.try_into().ok().expect("REASON")
}

pub fn block_to_u32<T: frame_system::Config>(block: BlockNumberFor<T>) -> u32 {
    TryInto::try_into(block)
        .ok()
        .expect("blockchain will not exceed 2^64 blocks; QED.")
}

pub fn set_block_to_subnet_slot_epoch<T: Config>(epoch: u32, subnet_id: u32) {
    let epoch_length = T::EpochLength::get();
    let slot =
        SubnetSlot::<T>::get(subnet_id).expect("SubnetSlot must be assigned before setting block");
    let block = u32_to_block::<T>(slot.saturating_add(epoch.saturating_mul(epoch_length)));
    frame_system::Pallet::<T>::set_block_number(block);
}

pub fn get_current_block_as_u32<T: frame_system::Config>() -> u32 {
    TryInto::try_into(<frame_system::Pallet<T>>::block_number())
        .ok()
        .expect("blockchain will not exceed u32::MAX blocks; QED.")
}

pub fn set_block_to_overwatch_reveal_block<T: Config>(epoch: u32) {
    assert_eq!(CurrentOverwatchEpoch::<T>::get(), epoch);
    let epoch_length = T::EpochLength::get();
    let multiplier = OverwatchEpochLengthMultiplier::<T>::get();
    let cutoff_percentage = OverwatchCommitCutoffPercent::<T>::get();
    let overwatch_epoch_length = epoch_length.saturating_mul(multiplier);
    let block_increase_cutoff =
        Network::<T>::percent_mul(overwatch_epoch_length as u128, cutoff_percentage);
    let block = u32_to_block::<T>(
        OverwatchEpochStartBlock::<T>::get().saturating_add(block_increase_cutoff as u32),
    );
    frame_system::Pallet::<T>::set_block_number(block);
}

pub fn set_block_to_overwatch_commit_block<T: Config>(epoch: u32) {
    assert_eq!(CurrentOverwatchEpoch::<T>::get(), epoch);
    let block = u32_to_block::<T>(OverwatchEpochStartBlock::<T>::get());
    frame_system::Pallet::<T>::set_block_number(block);
}

pub fn u128_to_balance<T: frame_system::Config + pallet::Config>(
    input: u128,
) -> Option<
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance,
> {
    input.try_into().ok()
}

pub fn get_initial_validator_ids(
    subnets: u32,
    max_subnet_nodes: u32,
    start: u32,
    end: u32,
) -> BTreeMap<u32, u32> {
    let mut whitelist = BTreeMap::new();
    for n in start..end {
        let _n = n + 1;
        whitelist.insert(_n, 1);
    }
    whitelist
}

/// Build a registration whitelist for `start..end` without exceeding the bounded identity
/// domain. Per-identity registration allowances let a Registered subnet still reach the full
/// active-node ceiling.
fn bounded_initial_validator_ids<T: Config>(start: u32, end: u32) -> BTreeMap<u32, u32> {
    let node_count = end.saturating_sub(start);
    let identity_count = node_count.min(T::MaxRegisteredNodesUpperBound::get());
    if identity_count == 0 {
        return BTreeMap::new();
    }

    (0..identity_count)
        .map(|identity_offset| {
            let registrations = node_count
                .saturating_sub(identity_offset)
                .saturating_add(identity_count - 1)
                / identity_count;
            (
                start.saturating_add(identity_offset).saturating_add(1),
                registrations.max(1),
            )
        })
        .collect()
}

fn bounded_initial_validator_id<T: Config>(start: u32, end: u32, node: u32) -> u32 {
    let identity_count = end
        .saturating_sub(start)
        .min(T::MaxRegisteredNodesUpperBound::get());
    assert!(
        identity_count > 0,
        "registration fixture must contain an identity"
    );
    start
        .saturating_add(node.saturating_sub(start) % identity_count)
        .saturating_add(1)
}

pub fn get_simulated_consensus_data<T: Config>(
    subnet_id: u32,
    node_count: u32,
) -> ConsensusData<T> {
    let mut attests = BTreeMap::new();
    let mut data = Vec::new();

    let max_subnet_nodes = MaxSubnetNodes::<T>::get();

    let block_number = get_current_block_as_u32::<T>();
    let epoch_length = T::EpochLength::get();
    let epoch = get_current_block_as_u32::<T>() / epoch_length;

    for n in 0..node_count {
        // let node_id = subnet_id*max_subnet_nodes+n+1;
        let node_id = subnet_id * max_subnet_nodes - max_subnet_nodes + n + 1;

        // Simulate some score and block number
        let score = 1e+18 as u128;

        attests.insert(
            node_id,
            AttestEntry::<T> {
                block: block_number,
                attestor_progress: 0,
                reward_factor: Network::<T>::percentage_factor_as_u128(),
                data: None,
            },
        );
        data.push(SubnetNodeConsensusData {
            subnet_node_id: node_id,
            score,
        });
    }

    let included_subnet_nodes: Vec<ConsensusSubnetNode> =
        Network::<T>::get_active_classified_subnet_nodes(
            subnet_id,
            &SubnetNodeClass::Included,
            epoch,
        )
        .iter()
        .map(ConsensusSubnetNode::from)
        .collect();

    let (validator_ids, emergency_active) =
        Network::<T>::effective_consensus_validator_ids(subnet_id, epoch);
    let emergency = if emergency_active {
        EmergencySubnetNodeElectionData::<T>::get(subnet_id).map(|emergency_validator_data| {
            Network::<T>::emergency_consensus_snapshot(
                &emergency_validator_data,
                validator_ids.clone(),
            )
        })
    } else {
        None
    };
    let validator_identity_ids = validator_ids
        .iter()
        .filter_map(|subnet_node_id| {
            SubnetNodeValidatorId::<T>::get(subnet_id, *subnet_node_id)
                .map(|validator_id| (*subnet_node_id, validator_id))
        })
        .collect();

    ConsensusData::<T> {
        validator_id: subnet_id * max_subnet_nodes,
        block: block_number,
        validator_epoch_progress: 0,
        validator_reward_factor: Network::<T>::percentage_factor_as_u128(),
        validator_ids,
        validator_identity_ids,
        attests,
        data,
        prioritize_queue_node_id: None,
        remove_queue_node_id: None,
        subnet_nodes: included_subnet_nodes,
        args: None,
        emergency,
    }
}

pub fn run_subnet_consensus_step<T: Config>(
    subnet_id: u32,
    prioritize_queue_node_id: Option<u32>,
    remove_queue_node_id: Option<u32>,
) {
    let max_subnets = MaxSubnets::<T>::get();
    let max_subnet_nodes = MaxSubnetNodes::<T>::get();

    let block_number = Network::<T>::get_current_block_as_u32();
    let epoch = Network::<T>::get_current_epoch_as_u32();

    let subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);

    let validator_node_id = SubnetElectedValidator::<T>::get(subnet_id, subnet_epoch)
        .map(|round| round.validator_subnet_node_id);
    assert!(validator_node_id != None, "Validator is None");
    assert!(validator_node_id != Some(0), "Validator is 0");

    let validator =
        Network::<T>::get_subnet_node_associated_hotkey(subnet_id, validator_node_id.unwrap())
            .unwrap();

    let total_subnet_nodes = TotalSubnetNodes::<T>::get(subnet_id);

    let subnet_node_data_vec =
        get_subnet_node_consensus_data::<T>(subnet_id, max_subnet_nodes, 0, total_subnet_nodes);

    assert_ok!(Network::<T>::propose_attestation(
        RawOrigin::Signed(validator.clone()).into(),
        subnet_id,
        subnet_node_data_vec.clone(),
        prioritize_queue_node_id,
        remove_queue_node_id,
        None,
        None,
    ));

    let mut attested_nodes = 0;
    for n in 0..total_subnet_nodes {
        let _n = n + 1;
        if SubnetNodeReputation::<T>::get(subnet_id, _n).is_some() {
            let is_validator = match SubnetNodesData::<T>::try_get(subnet_id, _n) {
                Ok(subnet_node) => {
                    subnet_node.has_classification(&SubnetNodeClass::Validator, subnet_epoch)
                }
                Err(()) => false,
            };
            if !is_validator {
                continue;
            }
            attested_nodes += 1;
            if SubnetConsensusSubmission::<T>::get(subnet_id, subnet_epoch)
                .is_some_and(|submission| submission.attests.contains_key(&_n))
            {
                // `propose_attestation` automatically records the elected proposer.
                continue;
            }
            let hotkey = Network::<T>::get_subnet_node_associated_hotkey(subnet_id, _n).unwrap();
            assert_ok!(Network::<T>::attest(
                RawOrigin::Signed(hotkey.clone()).into(),
                subnet_id,
                _n,
                None,
            ));
        }
    }

    let submission = SubnetConsensusSubmission::<T>::get(subnet_id, subnet_epoch).unwrap();
    assert_eq!(submission.attests.len(), attested_nodes as usize);
    assert_ne!(submission.attests.len(), 0);

    for n in 0..total_subnet_nodes {
        let _n = n + 1;

        if SubnetNodeReputation::<T>::get(subnet_id, _n).is_some() {
            let is_validator = match SubnetNodesData::<T>::try_get(subnet_id, _n) {
                Ok(subnet_node) => {
                    subnet_node.has_classification(&SubnetNodeClass::Validator, subnet_epoch)
                }
                Err(()) => false,
            };
            if !is_validator {
                continue;
            }
        } else {
            continue;
        }

        let subnet_node_id = _n;

        if _n == validator_node_id.unwrap() {
            assert_ne!(submission.attests.get(&(subnet_node_id)), None);
            assert_eq!(
                submission.attests.get(&(subnet_node_id)).unwrap().block,
                Network::<T>::get_current_block_as_u32()
            );
        } else {
            assert_ne!(submission.attests.get(&(subnet_node_id)), None);
            assert_eq!(
                submission.attests.get(&(subnet_node_id)).unwrap().block,
                Network::<T>::get_current_block_as_u32()
            );
        }
    }
}

fn to_bounded<Len: frame_support::traits::Get<u32>>(s: &str) -> BoundedVec<u8, Len> {
    BoundedVec::try_from(s.as_bytes().to_vec()).expect("String too long")
}

fn benchmark_identity<T: Config>() -> IdentityData<T> {
    IdentityData::<T> {
        name: Some(to_bounded::<<T as Config>::MaxVectorLength>(
            "benchmark-validator",
        )),
        url: Some(to_bounded::<<T as Config>::MaxUrlLength>(
            "https://hypertensor.example",
        )),
        image: None,
        discord: Some(to_bounded::<<T as Config>::MaxSocialIdLength>("validator")),
        x: None,
        telegram: None,
        github: Some(to_bounded::<<T as Config>::MaxUrlLength>(
            "https://github.com/hypertensor",
        )),
        hugging_face: None,
        description: Some(to_bounded::<<T as Config>::MaxVectorLength>(
            "benchmark identity",
        )),
        misc: None,
    }
}

fn build_owner_benchmark_subnet<T: Config>() -> (u32, T::AccountId) {
    let end = MinSubnetNodes::<T>::get();
    build_activated_subnet::<T>(
        DEFAULT_SUBNET_NAME.into(),
        0,
        end,
        DEFAULT_DEPOSIT_AMOUNT,
        DEFAULT_SUBNET_NODE_STAKE,
    );
    let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
    let owner = SubnetOwner::<T>::get(subnet_id).unwrap();
    (subnet_id, owner)
}

pub fn make_commit<T: Config>(weight: u128, salt: Vec<u8>) -> T::Hash {
    T::Hashing::hash_of(&(weight, salt))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AlternateEmissionMode {
    Missing,
    Rejected,
    Emergency,
}

struct AlternateEmissionContext {
    subnet_id: u32,
    current_epoch: u32,
    current_subnet_epoch: u32,
    previous_subnet_epoch: u32,
    validator_id: u32,
    initial_delegate_pool: u128,
    historical_nodes: u32,
}

fn max_emission_identity<T: Config>() -> IdentityData<T> {
    let bytes: NetworkBytes<T> = vec![1; T::MaxVectorLength::get() as usize]
        .try_into()
        .expect("maximum identity bytes fit their configured bound");
    let url: NetworkUrl<T> = vec![2; T::MaxUrlLength::get() as usize]
        .try_into()
        .expect("maximum identity URL fits its configured bound");
    let social: NetworkSocialId<T> = vec![3; T::MaxSocialIdLength::get() as usize]
        .try_into()
        .expect("maximum social ID fits its configured bound");

    IdentityData::<T> {
        name: Some(bytes.clone()),
        url: Some(url.clone()),
        image: Some(url.clone()),
        discord: Some(social.clone()),
        x: Some(social.clone()),
        telegram: Some(social),
        github: Some(url.clone()),
        hugging_face: Some(url),
        description: Some(bytes.clone()),
        misc: Some(bytes),
    }
}

fn max_fill_emission_queue<T: Config>(subnet_id: u32) {
    let max_bytes: NetworkBytes<T> = vec![9; T::MaxVectorLength::get() as usize]
        .try_into()
        .expect("maximum queue payload fits its configured bound");
    SubnetNodeQueue::<T>::mutate(subnet_id, |queue| {
        for node in queue.iter_mut() {
            let peer_info = node
                .peer_info
                .clone()
                .expect("registered benchmark queue node has peer information");
            node.peer_info = Some(PeerInfo {
                peer_id: peer_info.peer_id.clone(),
                multiaddr: Some(max_bytes.clone()),
            });
            node.bootnode_peer_info = Some(PeerInfo {
                peer_id: peer_info.peer_id.clone(),
                multiaddr: Some(max_bytes.clone()),
            });
            node.client_peer_info = Some(PeerInfo {
                peer_id: peer_info.peer_id,
                multiaddr: Some(max_bytes.clone()),
            });
            node.unique = Some(max_bytes.clone());
            node.non_unique = Some(max_bytes.clone());
        }
    });

    for node in SubnetNodeQueue::<T>::get(subnet_id) {
        RegisteredSubnetNodesData::<T>::insert(subnet_id, node.id, node);
    }
}

fn prepare_accepted_queue_mutations<T: Config>(
    q: u32,
    remove_from_front: bool,
) -> (u32, u32, u32, ConsensusSubmissionData<T>) {
    assert!((1..=T::MaxRegisteredNodesUpperBound::get()).contains(&q));
    NewRegistrationCostMultiplier::<T>::set(1_000_000_000_000_000_000);
    MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
    // A full active set prevents the operational queue from consuming these registrations while
    // they age through the proposal-removal immunity period. Registered capacity is independent,
    // so the reachable total is `MaxSubnetNodes + q` (at most 576).
    let active_nodes = T::MaxSubnetNodesUpperBound::get();
    build_activated_subnet::<T>(
        DEFAULT_SUBNET_NAME.into(),
        0,
        active_nodes,
        DEFAULT_DEPOSIT_AMOUNT,
        DEFAULT_SUBNET_NODE_STAKE,
    );
    let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
    MaxRegisteredNodes::<T>::insert(subnet_id, q);
    build_registered_subnet_nodes::<T>(
        subnet_id,
        active_nodes,
        active_nodes.saturating_add(q),
        DEFAULT_DEPOSIT_AMOUNT,
        DEFAULT_SUBNET_NODE_STAKE,
        true,
    );
    max_fill_emission_queue::<T>(subnet_id);

    let immunity_epochs = QueueImmunityEpochs::<T>::get(subnet_id);
    increase_epochs::<T>(immunity_epochs.saturating_add(2));
    let proposal_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);

    let queue = SubnetNodeQueue::<T>::get(subnet_id);
    assert_eq!(queue.len() as u32, q);
    assert_eq!(TotalActiveSubnetNodes::<T>::get(subnet_id), active_nodes);
    assert_eq!(
        TotalSubnetNodes::<T>::get(subnet_id),
        active_nodes.saturating_add(q)
    );
    assert!(queue
        .iter()
        .all(|node| Network::<T>::has_epoch_period_elapsed(
            node.classification.start_epoch,
            immunity_epochs,
            proposal_subnet_epoch,
        )));
    let prioritize_queue_node_id = queue.last().expect("the benchmark queue is non-empty").id;
    let remove_queue_node_id = if q == 1 {
        prioritize_queue_node_id
    } else if remove_from_front {
        queue[0].id
    } else {
        queue[queue.len().saturating_sub(2)].id
    };
    seed_common_remove_subnet_node_state::<T>(subnet_id, remove_queue_node_id, false);
    let remove_validator_id =
        SubnetNodeValidatorId::<T>::get(subnet_id, remove_queue_node_id).unwrap();
    seed_validator_owned_nodes::<T>(
        remove_validator_id,
        subnet_id,
        remove_queue_node_id,
        T::MaxValidatorNodesUpperBound::get(),
    );

    let consensus_submission_data = ConsensusSubmissionData::<T> {
        policy: ConsensusPolicySnapshot::default(),
        validator_subnet_node_id: 0,
        validator_delegate_stake_balance: 0,
        validator_epoch_progress: 0,
        validator_reward_factor: 0,
        attestation_ratio: Network::<T>::percentage_factor_as_u128(),
        identity_attestation_ratio: 0,
        identity_attestation_count: 0,
        eligible_validator_identity_count: 0,
        weight_sum: 0,
        data_length: 0,
        data: Vec::new(),
        attests: BTreeMap::new(),
        subnet_nodes: Vec::new(),
        prioritize_queue_node_id: Some(prioritize_queue_node_id),
        remove_queue_node_id: Some(remove_queue_node_id),
        emergency: None,
    };
    (
        subnet_id,
        prioritize_queue_node_id,
        remove_queue_node_id,
        consensus_submission_data,
    )
}

/// Build a reachable alternate settlement step. Historical work is parameterized by `h`, every
/// snapshotted node remains live, and the active/electable counters and indexes describe exactly
/// those `h` records. The subnet is paused before the timed call so settlement is measured without
/// mixing in independently benchmarked current-election and ready-queue work.
fn prepare_alternate_emission_step<T: Config>(
    h: u32,
    mode: AlternateEmissionMode,
) -> AlternateEmissionContext {
    let max_h = T::MaxSubnetNodesUpperBound::get();
    let emergency_nodes = T::MaxEmergencySubnetNodesUpperBound::get().min(h);
    assert!((3..=max_h).contains(&h));
    if mode == AlternateEmissionMode::Emergency {
        assert_eq!(emergency_nodes, T::MaxEmergencySubnetNodesUpperBound::get());
    }

    NewRegistrationCostMultiplier::<T>::set(1_000_000_000_000_000_000);
    MaxSubnetNodes::<T>::set(max_h);
    build_activated_subnet::<T>(
        DEFAULT_SUBNET_NAME.into(),
        0,
        h,
        DEFAULT_DEPOSIT_AMOUNT,
        DEFAULT_SUBNET_NODE_STAKE,
    );
    let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
    let percentage = Network::<T>::percentage_factor_as_u128();
    let identity = max_emission_identity::<T>();
    let delegate_pool = DEFAULT_DELEGATE_STAKE_TO_BE_ADDED.max(percentage);

    for subnet_node_id in 1..=h {
        let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id)
            .expect("historical benchmark node has a validator identity");
        ValidatorDelegateStakeBalance::<T>::insert(validator_id, delegate_pool);
        ValidatorDelegateStakeShares::<T>::insert(validator_id, 1);
        let delegate_account = get_account::<T>("alternate-emission-delegate", validator_id);
        ValidatorsData::<T>::mutate(validator_id, |validator| {
            validator.delegate_reward_rate = percentage.saturating_div(2);
            validator.delegate_account = Some(DelegateAccount {
                account_id: delegate_account,
                rate: percentage.saturating_div(10),
            });
            validator.identity = Some(identity.clone());
        });
        SubnetNodeReputation::<T>::insert(subnet_id, subnet_node_id, percentage);
    }
    TotalValidatorDelegateStakeBalance::<T>::set(delegate_pool.saturating_mul(h as u128));

    increase_epochs::<T>(1);
    set_block_to_subnet_slot_epoch::<T>(Network::<T>::get_current_epoch_as_u32(), subnet_id);
    let previous_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
    let validator_subnet_node_id = 1;
    let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, validator_subnet_node_id)
        .expect("benchmark proposer has a validator identity");

    let mut policy = Network::<T>::consensus_policy_snapshot(subnet_id, previous_subnet_epoch);
    policy.min_attestation_percentage = percentage;
    policy.super_majority_attestation_ratio = percentage;
    policy.validator_identity_attestation_percentage = percentage;
    policy.base_slash_percentage = percentage;
    policy.max_slash_amount = u128::MAX;
    policy.validator_delegate_stake_slash_threshold = percentage;
    policy.base_validator_delegate_stake_slash_percentage = percentage;
    policy.max_validator_delegate_stake_slash_amount = u128::MAX;
    policy.validator_absent_subnet_reputation_factor = percentage;
    policy.not_in_consensus_subnet_reputation_factor = percentage;
    policy.min_subnet_node_reputation = percentage;
    policy.reputation_factors.absent_decrease = percentage;
    policy.reputation_factors.non_attestor_decrease = percentage;
    policy.reputation_factors.non_consensus_attestor_decrease = percentage;
    policy.reputation_factors.validator_absent_decrease = percentage;
    policy.reputation_factors.validator_non_consensus_decrease = percentage;

    let eligible_count = if mode == AlternateEmissionMode::Emergency {
        emergency_nodes
    } else {
        h
    };
    let eligible_subnet_node_ids: Vec<u32> = (1..=eligible_count).collect();
    let eligible_validator_identity_ids: BTreeMap<u32, u32> = eligible_subnet_node_ids
        .iter()
        .map(|node_id| {
            (
                *node_id,
                SubnetNodeValidatorId::<T>::get(subnet_id, *node_id)
                    .expect("eligible node has a validator identity"),
            )
        })
        .collect();

    SubnetElectedValidator::<T>::insert(
        subnet_id,
        previous_subnet_epoch,
        ElectedConsensusRound {
            validator_subnet_node_id,
            validator_id,
            emergency: (mode == AlternateEmissionMode::Emergency).then(|| {
                EmergencyConsensusSnapshot {
                    subnet_node_ids: eligible_subnet_node_ids.clone(),
                    reputation_factors: policy.reputation_factors,
                    min_subnet_node_reputation: policy.min_subnet_node_reputation,
                    min_weight_decrease_reputation_threshold: policy
                        .min_weight_decrease_reputation_threshold,
                }
            }),
            eligible_subnet_node_ids: eligible_subnet_node_ids.clone(),
            eligible_validator_identity_ids: eligible_validator_identity_ids.clone(),
            validator_delegate_stake_balance: delegate_pool,
            policy,
        },
    );

    if mode != AlternateEmissionMode::Missing {
        let attest_count = if mode == AlternateEmissionMode::Rejected {
            eligible_count.saturating_sub(1)
        } else {
            eligible_count
        };
        let attests: BTreeMap<u32, AttestEntry<T>> = (1..=attest_count)
            .map(|node_id| {
                (
                    node_id,
                    AttestEntry::<T> {
                        block: Network::<T>::get_current_block_as_u32(),
                        attestor_progress: 0,
                        reward_factor: percentage,
                        data: None,
                    },
                )
            })
            .collect();
        let subnet_nodes: Vec<ConsensusSubnetNode> = (1..=h)
            .map(|node_id| {
                let node = SubnetNodesData::<T>::get(subnet_id, node_id);
                ConsensusSubnetNode::from(&node)
            })
            .collect();
        let data: Vec<SubnetNodeConsensusData> = (1..=h)
            .map(|node_id| SubnetNodeConsensusData {
                subnet_node_id: node_id,
                score: 1,
            })
            .collect();

        let emergency = if mode == AlternateEmissionMode::Emergency {
            let emergency_data = EmergencySubnetValidatorData {
                subnet_node_ids: eligible_subnet_node_ids.clone(),
                target_emergency_validators_epochs: 1,
                max_emergency_validators_epoch: u32::MAX,
                total_epochs: 0,
                activated: true,
                started_subnet_epoch: previous_subnet_epoch,
                reputation_factors: policy.reputation_factors,
                min_subnet_node_reputation: policy.min_subnet_node_reputation,
                min_weight_decrease_reputation_threshold: u128::MAX,
            };
            EmergencySubnetNodeElectionData::<T>::insert(subnet_id, emergency_data.clone());
            Some(Network::<T>::emergency_consensus_snapshot(
                &emergency_data,
                eligible_subnet_node_ids.clone(),
            ))
        } else {
            None
        };

        SubnetConsensusSubmission::<T>::insert(
            subnet_id,
            previous_subnet_epoch,
            ConsensusData::<T> {
                validator_id: validator_subnet_node_id,
                block: Network::<T>::get_current_block_as_u32(),
                validator_epoch_progress: 0,
                validator_reward_factor: percentage,
                validator_ids: eligible_subnet_node_ids.clone(),
                validator_identity_ids: eligible_validator_identity_ids,
                attests,
                subnet_nodes,
                prioritize_queue_node_id: None,
                remove_queue_node_id: None,
                data,
                args: None,
                emergency,
            },
        );
        SubnetConsensusSubmissionMaxItems::<T>::insert(subnet_id, previous_subnet_epoch, h);
        SubnetConsensusAttestorWeights::<T>::insert(
            subnet_id,
            previous_subnet_epoch,
            ConsensusAttestorWeightSnapshot {
                weights: eligible_subnet_node_ids
                    .iter()
                    .map(|node_id| (*node_id, 1))
                    .collect(),
                total_weight: eligible_count as u128,
            },
        );
    } else {
        SubnetConsensusSubmission::<T>::remove(subnet_id, previous_subnet_epoch);
        SubnetConsensusSubmissionMaxItems::<T>::remove(subnet_id, previous_subnet_epoch);
        SubnetConsensusAttestorWeights::<T>::remove(subnet_id, previous_subnet_epoch);
    }

    increase_epochs::<T>(1);
    set_block_to_subnet_slot_epoch::<T>(Network::<T>::get_current_epoch_as_u32(), subnet_id);
    let current_epoch = Network::<T>::get_current_epoch_as_u32();
    let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
    assert_eq!(
        current_subnet_epoch,
        previous_subnet_epoch.saturating_add(1)
    );

    // A validator slot vector may have any insertion/removal order. Put the rejected attestors at
    // the tail so each of the bounded removal operations scans the largest reachable prefix while
    // every index and cardinality still matches the live h-node state.
    if mode == AlternateEmissionMode::Rejected {
        let election_slots: Vec<u32> = (1..=h).rev().collect();
        for (index, subnet_node_id) in election_slots.iter().enumerate() {
            NodeSlotIndex::<T>::insert(subnet_id, subnet_node_id, index as u32);
        }
        SubnetNodeElectionSlots::<T>::insert(subnet_id, election_slots);
        TotalSubnetElectableNodes::<T>::insert(subnet_id, h);
    }

    let mut subnet_weights = BTreeMap::from([(subnet_id, percentage)]);
    for index in 0..T::MaxPhysicalSubnetsUpperBound::get().saturating_sub(1) {
        subnet_weights.insert(100_000 + index, u128::MAX.saturating_sub(index as u128));
    }
    FinalSubnetEmissionWeights::<T>::insert(
        current_epoch,
        DistributionData {
            subnets_emissions: DEFAULT_SCORE,
            subnet_weights,
        },
    );
    max_fill_benchmark_subnet_data::<T>(subnet_id);
    SubnetsData::<T>::mutate(subnet_id, |maybe_subnet| {
        let subnet = maybe_subnet.as_mut().expect("benchmark subnet must exist");
        subnet.state = SubnetState::Paused;
        subnet.pause = Some(SubnetPauseData {
            started_global_epoch: current_epoch,
            started_subnet_epoch: current_subnet_epoch,
        });
    });

    assert_eq!(TotalActiveSubnetNodes::<T>::get(subnet_id), h);
    assert_eq!(TotalSubnetNodes::<T>::get(subnet_id), h);
    assert_eq!(TotalSubnetElectableNodes::<T>::get(subnet_id), h);
    assert!(SubnetNodeQueue::<T>::get(subnet_id).is_empty());

    AlternateEmissionContext {
        subnet_id,
        current_epoch,
        current_subnet_epoch,
        previous_subnet_epoch,
        validator_id,
        initial_delegate_pool: delegate_pool,
        historical_nodes: h,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MixedSwapBranch {
    Validator,
    Subnet,
    Refund,
}

struct MixedSwapBenchmarkContext {
    block_number: u32,
    claim_block: u32,
    subnet_ids: Vec<u32>,
    validator_calls: u32,
    subnet_calls: u32,
    refund_calls: u32,
}

/// Select one call from every swap execution branch before filling the remainder with the
/// requested dominant branch. The three resulting benchmark models are the vertices of the
/// affine `(validator, subnet, refund)` composition for a prefix of length `x`.
fn mixed_swap_branch(queue_id: u32, dominant: MixedSwapBranch) -> MixedSwapBranch {
    match queue_id {
        0 => MixedSwapBranch::Validator,
        1 => MixedSwapBranch::Subnet,
        2 => MixedSwapBranch::Refund,
        _ => dominant,
    }
}

fn mixed_swap_account<T: Config>(branch: MixedSwapBranch, queue_id: u32) -> T::AccountId {
    match branch {
        MixedSwapBranch::Validator => {
            get_account::<T>("mixed_ready_validator_swap_account", queue_id)
        }
        MixedSwapBranch::Subnet => get_account::<T>("mixed_ready_subnet_swap_account", queue_id),
        MixedSwapBranch::Refund => get_account::<T>("mixed_ready_refund_swap_account", queue_id),
    }
}

/// Build a storage-consistent ready queue whose first `x` calls contain all three execution
/// branches. Queue-vector decode/rewrite is benchmarked independently by
/// `execute_ready_swap_queue`; this fixture times only the item prefix while retaining the exact
/// `SwapQueueOrder`, count and item-map relationship that the hook observes.
fn prepare_mixed_swap_benchmark<T: Config>(
    x: u32,
    dominant: MixedSwapBranch,
) -> MixedSwapBenchmarkContext {
    assert!((MIN_MIXED_SWAP_BENCHMARK_DOMAIN..=MAX_SWAP_QUEUE_BENCHMARK_DOMAIN).contains(&x));
    assert!(x <= T::MaxSwapQueueLength::get());

    let mut validator_calls = 0u32;
    let mut subnet_calls = 0u32;
    let mut refund_calls = 0u32;
    for queue_id in 0..x {
        match mixed_swap_branch(queue_id, dominant) {
            MixedSwapBranch::Validator => validator_calls = validator_calls.saturating_add(1),
            MixedSwapBranch::Subnet => subnet_calls = subnet_calls.saturating_add(1),
            MixedSwapBranch::Refund => refund_calls = refund_calls.saturating_add(1),
        }
    }
    assert_eq!(
        validator_calls
            .saturating_add(subnet_calls)
            .saturating_add(refund_calls),
        x
    );
    assert!(validator_calls > 0 && subnet_calls > 0 && refund_calls > 0);

    // Use every reachable physical destination before cycling. `contains_key` still proves the
    // complete trie leaf, so fill the variable-sized subnet value even though it is not decoded.
    let destination_count = subnet_calls.min(T::MaxPhysicalSubnetsUpperBound::get());
    assert!(destination_count > 0);
    let mut subnet_ids = Vec::with_capacity(destination_count as usize);
    for destination in 0..destination_count {
        let path: Vec<u8> = format!("mixed-ready-swap-subnet-{destination}").into();
        build_activated_subnet::<T>(
            path.clone(),
            0,
            MinSubnetNodes::<T>::get(),
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id =
            SubnetName::<T>::get::<Vec<u8>>(path).expect("mixed swap subnet must be active");
        subnet_ids.push(subnet_id);
    }
    // Defer metadata replacement until every production-shaped destination is activated. The
    // registration helper intentionally consults live subnet economics while constructing later
    // destinations; only the timed `contains_key` calls need the final maximum-sized leaves.
    for subnet_id in subnet_ids.iter().copied() {
        max_fill_benchmark_subnet_data::<T>(subnet_id);
    }

    // Validator existence uses `contains_key` as well. Fill both optional fields, including every
    // bounded identity payload, so a mixed prefix cannot escape the generated proof-size model.
    for validator_index in 0..validator_calls {
        let validator_id = validator_index.saturating_add(1);
        ensure_validator::<T>(validator_id);
        ValidatorsData::<T>::mutate(validator_id, |validator| {
            validator.delegate_account = Some(DelegateAccount {
                account_id: get_account::<T>(
                    "mixed_ready_validator_delegate_account",
                    validator_id,
                ),
                rate: u128::MAX,
            });
            validator.identity = Some(max_emission_identity::<T>());
        });
    }

    let queued_at_block = get_current_block_as_u32::<T>();
    let execute_after_blocks = T::EpochLength::get();
    let block_number = queued_at_block.saturating_add(execute_after_blocks);
    assert_eq!(
        block_number.saturating_sub(queued_at_block),
        execute_after_blocks
    );
    frame_system::Pallet::<T>::set_block_number(u32_to_block::<T>(block_number));

    let max_unbondings = T::MaxUnbondingsUpperBound::get();
    assert!(max_unbondings > 0);
    MaxUnbondings::<T>::set(max_unbondings);
    let cooldown_blocks =
        DelegateStakeCooldownEpochs::<T>::get().saturating_mul(T::EpochLength::get());
    let claim_block = block_number.saturating_add(cooldown_blocks);
    let missing_subnet_id = u32::MAX;
    assert!(!SubnetsData::<T>::contains_key(missing_subnet_id));

    let balance = DEFAULT_DELEGATE_STAKE_TO_BE_ADDED;
    let mut validator_index = 0u32;
    let mut subnet_index = 0u32;
    let mut queue: SwapQueueIds<T> = BoundedVec::new();
    for queue_id in 0..x {
        let branch = mixed_swap_branch(queue_id, dominant);
        let account_id = mixed_swap_account::<T>(branch, queue_id);
        let call = match branch {
            MixedSwapBranch::Validator => {
                let validator_id = validator_index.saturating_add(1);
                validator_index = validator_index.saturating_add(1);
                QueuedSwapCall::SwapToValidatorDelegateStake {
                    account_id,
                    to_validator_id: validator_id,
                    balance,
                }
            }
            MixedSwapBranch::Subnet => {
                let to_subnet_id = subnet_ids[(subnet_index % destination_count) as usize];
                subnet_index = subnet_index.saturating_add(1);
                QueuedSwapCall::SwapToSubnetDelegateStake {
                    account_id,
                    to_subnet_id,
                    balance,
                }
            }
            MixedSwapBranch::Refund => {
                // Merge into an existing target block in an otherwise maximum-sized ledger. This
                // is the largest successful refund value rewrite; a full ledger without the target
                // would stop the prefix before exercising later branches.
                let mut ledger = BTreeMap::new();
                ledger.insert(
                    claim_block,
                    UnbondingEntry {
                        network: 1,
                        overwatch: 0,
                    },
                );
                for offset in 1..max_unbondings {
                    ledger.insert(
                        claim_block.saturating_add(offset),
                        UnbondingEntry {
                            network: 1,
                            overwatch: 0,
                        },
                    );
                }
                assert_eq!(ledger.len() as u32, max_unbondings);
                StakeUnbondingLedger::<T>::insert(&account_id, ledger);
                QueuedSwapCall::SwapToSubnetDelegateStake {
                    account_id,
                    to_subnet_id: missing_subnet_id,
                    balance,
                }
            }
        };
        SwapCallQueue::<T>::insert(
            queue_id,
            QueuedSwapItem {
                id: queue_id,
                call,
                queued_at_block,
                execute_after_blocks,
            },
        );
        queue
            .try_push(queue_id)
            .expect("mixed swap fixture remains inside the queue bound");
    }
    assert_eq!(validator_index, validator_calls);
    assert_eq!(subnet_index, subnet_calls);
    SwapQueueOrder::<T>::set(queue);
    SwapQueueCount::<T>::set(x);
    NextSwapQueueId::<T>::set(x);
    TotalQueuedSwapPrincipal::<T>::set(
        (x as u128)
            .checked_mul(balance)
            .expect("benchmark queue principal fits u128"),
    );
    TotalNetworkUnbondingBalance::<T>::set(
        (refund_calls as u128).saturating_mul(max_unbondings as u128),
    );

    assert_eq!(SwapQueueOrder::<T>::get().len() as u32, x);
    assert_eq!(SwapQueueCount::<T>::get(), x);
    assert_eq!(SwapCallQueue::<T>::iter().count() as u32, x);
    assert_benchmark_queued_swap_principal::<T>();

    MixedSwapBenchmarkContext {
        block_number,
        claim_block,
        subnet_ids,
        validator_calls,
        subnet_calls,
        refund_calls,
    }
}

fn verify_mixed_swap_benchmark<T: Config>(
    x: u32,
    dominant: MixedSwapBranch,
    context: &MixedSwapBenchmarkContext,
) {
    assert_eq!(SwapCallQueue::<T>::iter().count(), 0);
    assert_eq!(TotalQueuedSwapPrincipal::<T>::get(), 0);
    // The independent item component deliberately leaves the queue vector for the separately
    // composed q benchmark. Its prefix must nevertheless describe the exact executed items.
    assert_eq!(
        SwapQueueOrder::<T>::get().as_slice(),
        (0..x).collect::<Vec<_>>()
    );
    assert_eq!(SwapQueueCount::<T>::get(), x);

    let balance = DEFAULT_DELEGATE_STAKE_TO_BE_ADDED;
    let mut validator_index = 0u32;
    let mut subnet_index = 0u32;
    let mut observed_refunds = 0u32;
    for queue_id in 0..x {
        let branch = mixed_swap_branch(queue_id, dominant);
        let account_id = mixed_swap_account::<T>(branch, queue_id);
        match branch {
            MixedSwapBranch::Validator => {
                let validator_id = validator_index.saturating_add(1);
                validator_index = validator_index.saturating_add(1);
                assert_ne!(
                    AccountValidatorDelegateStakeShares::<T>::get(account_id, validator_id),
                    0
                );
            }
            MixedSwapBranch::Subnet => {
                let destination_count = context.subnet_ids.len() as u32;
                let subnet_id = context.subnet_ids[(subnet_index % destination_count) as usize];
                subnet_index = subnet_index.saturating_add(1);
                assert_ne!(
                    AccountSubnetDelegateStakeShares::<T>::get(account_id, subnet_id),
                    0
                );
            }
            MixedSwapBranch::Refund => {
                observed_refunds = observed_refunds.saturating_add(1);
                assert_eq!(
                    StakeUnbondingLedger::<T>::get(account_id)
                        .get(&context.claim_block)
                        .map(|entry| entry.network),
                    Some(balance.saturating_add(1))
                );
            }
        }
    }
    assert_eq!(validator_index, context.validator_calls);
    assert_eq!(subnet_index, context.subnet_calls);
    assert_eq!(observed_refunds, context.refund_calls);
    assert_eq!(get_current_block_as_u32::<T>(), context.block_number);
}

// Collective pallet functions

fn is_member<T: Config>(account: T::AccountId)
where
    T: pallet_collective::Config<Instance1>,
{
    let is_member = pallet_collective::Pallet::<T, Instance1>::is_member(&account);
}

fn get_collective_members<T: Config>() -> Vec<T::AccountId>
where
    T: pallet_collective::Config<Instance1>,
{
    let members = pallet_collective::Members::<T, Instance1>::get();
    members
}

fn set_members<T: Config>()
where
    T: pallet_collective::Config<Instance1>,
{
    let members = vec![
        get_account::<T>("collective", 1),
        get_account::<T>("collective", 2),
        get_account::<T>("collective", 3),
        get_account::<T>("collective", 4),
        get_account::<T>("collective", 5),
    ];
    assert_ok!(pallet_collective::Pallet::<T, Instance1>::set_members(
        RawOrigin::Root.into(),
        members.clone(),
        Some(members[0].clone()),
        T::MaxMembers::get()
    ));
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn register_validator() {
        let coldkey = get_account::<T>("benchmark_validator_coldkey", 0);
        let hotkey = get_account::<T>("benchmark_validator_hotkey", 0);
        let reward_rate = DEFAULT_VALIDATOR_REWARD_RATE;

        #[extrinsic_call]
        register_validator(
            RawOrigin::Signed(coldkey.clone()),
            hotkey.clone(),
            reward_rate,
            None,
            None,
        );

        let validator_id = ColdkeyValidatorId::<T>::get(&coldkey).unwrap();
        assert_eq!(ValidatorColdkey::<T>::get(validator_id), Some(coldkey));
        assert_eq!(ValidatorIdHotkey::<T>::get(validator_id), Some(hotkey));
    }

    #[benchmark]
    fn update_validator_coldkey() {
        let validator_id = 1;
        let (coldkey, hotkey) = ensure_validator::<T>(validator_id);
        let new_coldkey = get_account::<T>("new_validator_coldkey", 0);

        #[extrinsic_call]
        update_validator_coldkey(
            RawOrigin::Signed(coldkey.clone()),
            validator_id,
            new_coldkey.clone(),
        );

        assert_eq!(
            ValidatorColdkey::<T>::get(validator_id),
            Some(new_coldkey.clone())
        );
        assert_eq!(
            ColdkeyValidatorId::<T>::get(&new_coldkey),
            Some(validator_id)
        );
        assert_eq!(ColdkeyValidatorId::<T>::get(&coldkey), None);
        assert_eq!(ValidatorColdkeyHotkey::<T>::get(&coldkey), None);
        assert_eq!(
            ValidatorColdkeyHotkey::<T>::get(&new_coldkey),
            Some(hotkey.clone())
        );
        assert_eq!(ValidatorIdHotkey::<T>::get(validator_id), Some(hotkey));
    }

    #[benchmark]
    fn update_validator_hotkey() {
        let validator_id = 1;
        let (coldkey, hotkey) = ensure_validator::<T>(validator_id);
        let new_hotkey = get_account::<T>("new_validator_hotkey", 0);

        #[extrinsic_call]
        update_validator_hotkey(
            RawOrigin::Signed(coldkey.clone()),
            validator_id,
            new_hotkey.clone(),
        );

        assert_eq!(
            ValidatorIdHotkey::<T>::get(validator_id),
            Some(new_hotkey.clone())
        );
        assert_eq!(
            ValidatorColdkeyHotkey::<T>::get(&coldkey),
            Some(new_hotkey.clone())
        );
        assert_eq!(ValidatorsData::<T>::get(validator_id).hotkey, new_hotkey);
        assert_eq!(HotkeyValidatorId::<T>::get(&hotkey), None);
        assert_eq!(
            HotkeyValidatorId::<T>::get(ValidatorsData::<T>::get(validator_id).hotkey),
            Some(validator_id)
        );
    }

    #[benchmark]
    fn update_validator_delegate_account() {
        let validator_id = 1;
        let (coldkey, _hotkey) = ensure_validator::<T>(validator_id);
        let delegate_account = get_account::<T>("validator_delegate_account", 0);
        let delegate_rate = 100_000_000_000_000_000u128;

        #[extrinsic_call]
        update_validator_delegate_account(
            RawOrigin::Signed(coldkey.clone()),
            validator_id,
            Some(delegate_account.clone()),
            Some(delegate_rate),
        );

        assert_eq!(
            ValidatorsData::<T>::get(validator_id).delegate_account,
            Some(DelegateAccount {
                account_id: delegate_account,
                rate: delegate_rate,
            })
        );
    }

    #[benchmark]
    fn update_validator_identity() {
        let validator_id = 1;
        let (coldkey, _hotkey) = ensure_validator::<T>(validator_id);
        let identity = benchmark_identity::<T>();

        #[extrinsic_call]
        update_validator_identity(
            RawOrigin::Signed(coldkey.clone()),
            validator_id,
            Some(identity.clone()),
        );

        assert_eq!(
            ValidatorsData::<T>::get(validator_id).identity,
            Some(identity)
        );
    }

    #[benchmark]
    fn register_subnet() {
        let block_number = get_current_block_as_u32::<T>();
        let cost = Network::<T>::get_current_registration_cost(block_number)
            .saturating_add(BENCHMARK_REGISTRATION_COST_BUFFER);

        let funded_initializer = funded_initializer::<T>("funded_initializer", 0);

        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        let subnets = subnet_id_key_offset::<T>(next_subnet_id::<T>());
        let mut register_subnet_data = default_registration_subnet_data::<T>(
            subnets,
            max_subnet_nodes,
            DEFAULT_SUBNET_NAME.into(),
            0,
            T::MaxRegisteredNodesUpperBound::get(),
        );
        register_subnet_data.name = vec![41; T::MaxVectorLength::get() as usize];
        register_subnet_data.repo = vec![42; T::MaxUrlLength::get() as usize];
        register_subnet_data.description = vec![43; T::MaxVectorLength::get() as usize];
        register_subnet_data.misc = vec![44; T::MaxVectorLength::get() as usize];
        register_subnet_data.initial_validators = (1..=T::MaxRegisteredNodesUpperBound::get())
            .map(|validator_id| (validator_id, u32::MAX))
            .collect();
        MaxBootnodes::<T>::set(T::MaxBootnodesUpperBound::get());
        register_subnet_data.bootnodes = maximum_benchmark_bootnodes::<T>();
        let expected_name = register_subnet_data.name.clone();

        let current_block_number = get_current_block_as_u32::<T>();

        #[extrinsic_call]
        register_subnet(
            RawOrigin::Signed(funded_initializer.clone()),
            100000000000000000000000,
            register_subnet_data,
        );

        let subnet_id = SubnetName::<T>::get(&expected_name).unwrap();
        let owner = SubnetOwner::<T>::get(subnet_id).unwrap();
        assert_eq!(owner, funded_initializer.clone());

        let subnet = SubnetsData::<T>::get(subnet_id).unwrap();
        assert_eq!(subnet.id, subnet_id);
        assert_eq!(subnet.name, expected_name);
    }

    #[benchmark]
    fn activate_subnet() {
        // Whitelist *identity* cardinality is bounded, but each identity has an independent
        // registration allowance. Distribute the full Registered-state node ceiling across the
        // bounded identity set so the removal-failure branch sees its reachable maximum.
        MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
        let active_nodes = T::MaxSubnetNodesUpperBound::get();
        // A subnet cannot have post-activation queued registrations while it is still in the
        // Registered state. The activation failure branch therefore removes only the initial
        // validator set (plus the independent overwatch/physical-subnet cleanup state).
        let registered_nodes = 0;
        build_registered_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            active_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
            false,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let owner_coldkey = subnet_owner::<T>(subnet_id);

        seed_remove_subnet_cleanup_state::<T>(
            subnet_id,
            active_nodes,
            registered_nodes,
            T::MaxOverwatchNodesUpperBound::get(),
        );
        max_fill_remove_subnet_keyed_state::<T>(subnet_id, false);

        assert_eq!(
            NodeRegistrationInitialValidatorIds::<T>::get(subnet_id)
                .expect("registered subnet retains its whitelist")
                .len() as u32,
            T::MaxRegisteredNodesUpperBound::get()
        );
        assert_eq!(
            InitialValidatorData::<T>::get(subnet_id)
                .expect("registered subnet tracks every initial identity")
                .len() as u32,
            T::MaxRegisteredNodesUpperBound::get()
        );

        increase_epochs::<T>(
            SubnetRegistrationEpochs::<T>::get()
                .saturating_add(SubnetEnactmentEpochs::<T>::get())
                .saturating_add(1),
        );

        // `can_subnet_be_active` computes the shared minimum before this expired-enactment branch
        // performs its maximal cleanup. Populate every other physical slot with a live subnet so
        // the benchmark includes the complete bounded delegation scan.
        MinSubnetDelegateStakeBalance::<T>::put(1);
        MinSubnetDelegateStakeFactor::<T>::put(DefaultMinSubnetDelegateStakeFactor::get());
        let live_peer_ids = seed_live_delegation_peers_for_activation::<T>(subnet_id);

        #[extrinsic_call]
        activate_subnet(RawOrigin::Signed(owner_coldkey.clone()), subnet_id);

        assert_eq!(SubnetsData::<T>::try_get(subnet_id), Err(()));
        assert!(live_peer_ids
            .iter()
            .all(|peer_id| SubnetsData::<T>::contains_key(peer_id)));
    }

    #[benchmark]
    fn owner_pause_subnet() {
        SubnetPauseCooldownEpochs::<T>::set(1);
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);
        advance_to_subnet_pause_eligibility::<T>(subnet_id);

        #[extrinsic_call]
        owner_pause_subnet(RawOrigin::Signed(owner_coldkey.clone()), subnet_id);

        let subnet = SubnetsData::<T>::get(subnet_id).unwrap();
        assert_eq!(subnet.state, SubnetState::Paused);
        assert!(subnet.consensus_eligible_from_subnet_epoch.is_none());
        assert!(subnet.pause.is_some());
    }

    #[benchmark]
    fn owner_unpause_subnet() {
        SubnetPauseCooldownEpochs::<T>::set(1);
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        // Unpause shifts every queued node in both the canonical registration map and the
        // ordered queue copy. Populate the maximum configured queue so a future live benchmark
        // measures that per-node path instead of the empty-queue fast path.
        let max_registered_nodes = MaxRegisteredNodes::<T>::get(subnet_id);
        let queued_start = max_subnet_nodes;
        let queued_end = queued_start.saturating_add(max_registered_nodes);
        build_registered_subnet_nodes::<T>(
            subnet_id,
            queued_start,
            queued_end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
            false,
        );
        let original_queue_starts: BTreeMap<u32, u32> =
            RegisteredSubnetNodesData::<T>::iter_prefix(subnet_id)
                .map(|(node_id, node)| (node_id, node.classification.start_epoch))
                .collect();
        assert_eq!(original_queue_starts.len(), max_registered_nodes as usize);
        assert_eq!(
            SubnetNodeQueue::<T>::get(subnet_id).len(),
            max_registered_nodes as usize
        );

        let owner_coldkey = subnet_owner::<T>(subnet_id);
        advance_to_subnet_pause_eligibility::<T>(subnet_id);

        assert_ok!(Network::<T>::owner_pause_subnet(
            RawOrigin::Signed(owner_coldkey.clone()).into(),
            subnet_id
        ));

        increase_epochs::<T>(1);

        let subnet = SubnetsData::<T>::get(subnet_id).unwrap();
        assert_eq!(subnet.state, SubnetState::Paused);
        let pause_started_subnet_epoch = subnet.pause.unwrap().started_subnet_epoch;
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
        let queue_shift = current_subnet_epoch.saturating_sub(pause_started_subnet_epoch);

        #[extrinsic_call]
        owner_unpause_subnet(RawOrigin::Signed(owner_coldkey.clone()), subnet_id);

        let subnet = SubnetsData::<T>::get(subnet_id).unwrap();
        assert_eq!(subnet.state, SubnetState::Active);
        assert_eq!(
            subnet.consensus_eligible_from_subnet_epoch,
            Some(current_subnet_epoch.saturating_add(2))
        );
        assert!(subnet.pause.is_none());

        let shifted_queue: BTreeMap<u32, u32> = SubnetNodeQueue::<T>::get(subnet_id)
            .into_iter()
            .map(|node| (node.id, node.classification.start_epoch))
            .collect();
        assert_eq!(shifted_queue.len(), original_queue_starts.len());
        for (node_id, original_start) in original_queue_starts {
            let expected_start = original_start.saturating_add(queue_shift);
            assert_eq!(
                RegisteredSubnetNodesData::<T>::get(subnet_id, node_id)
                    .classification
                    .start_epoch,
                expected_start
            );
            assert_eq!(shifted_queue.get(&node_id), Some(&expected_start));
        }
    }

    #[benchmark]
    fn owner_deactivate_subnet() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        let max_registered_nodes = T::MaxRegisteredNodesUpperBound::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        build_registered_subnet_nodes::<T>(
            subnet_id,
            max_subnet_nodes,
            max_subnet_nodes.saturating_add(max_registered_nodes),
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
            false,
        );
        seed_remove_subnet_cleanup_state::<T>(
            subnet_id,
            max_subnet_nodes,
            max_registered_nodes,
            T::MaxOverwatchNodesUpperBound::get(),
        );
        max_fill_remove_subnet_keyed_state::<T>(subnet_id, true);
        let current_epoch = Network::<T>::get_current_epoch_as_u32();
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
        SubnetsData::<T>::mutate(subnet_id, |maybe_subnet| {
            let subnet = maybe_subnet.as_mut().expect("benchmark subnet must exist");
            subnet.state = SubnetState::Paused;
            subnet.consensus_eligible_from_subnet_epoch = None;
            subnet.pause = Some(SubnetPauseData {
                started_global_epoch: current_epoch,
                started_subnet_epoch: current_subnet_epoch,
            });
        });

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        #[extrinsic_call]
        owner_deactivate_subnet(RawOrigin::Signed(owner_coldkey.clone()), subnet_id);

        assert_eq!(SubnetsData::<T>::try_get(subnet_id), Err(()));
    }

    #[benchmark]
    fn owner_update_name() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let new_value: Vec<u8> = "new-subnet-name".into();

        #[extrinsic_call]
        owner_update_name(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value.clone(),
        );

        let subnet_data = SubnetsData::<T>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.name, new_value.clone());

        assert_eq!(SubnetName::<T>::get(&new_value.clone()).unwrap(), subnet_id);
    }

    #[benchmark]
    fn owner_update_repo() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let new_value: Vec<u8> = "new-subnet-repo".into();

        #[extrinsic_call]
        owner_update_repo(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value.clone(),
        );

        let subnet_data = SubnetsData::<T>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.repo, new_value.clone());

        assert_eq!(SubnetRepo::<T>::get(&new_value.clone()).unwrap(), subnet_id);
    }

    #[benchmark]
    fn owner_update_description() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let new_value: Vec<u8> = "new-subnet-description".into();

        #[extrinsic_call]
        owner_update_description(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value.clone(),
        );

        let subnet_data = SubnetsData::<T>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.description, new_value.clone());
    }

    #[benchmark]
    fn owner_update_misc() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let new_value: Vec<u8> = "new-subnet-misc".into();

        #[extrinsic_call]
        owner_update_misc(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value.clone(),
        );

        let subnet_data = SubnetsData::<T>::get(subnet_id).unwrap();
        assert_eq!(subnet_data.misc, new_value.clone());
    }

    #[benchmark]
    fn owner_update_churn_limit() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let current_value = ChurnLimit::<T>::get(subnet_id);

        let new_value = current_value + 1;

        #[extrinsic_call]
        owner_update_churn_limit(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value,
        );

        let value = ChurnLimit::<T>::get(subnet_id);
        assert_eq!(value, new_value);
    }

    #[benchmark]
    fn owner_update_registration_queue_epochs() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let current_value = SubnetNodeQueueEpochs::<T>::get(subnet_id);
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);

        let new_value = current_value + 1;

        #[extrinsic_call]
        owner_update_registration_queue_epochs(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value,
        );

        let pending = PendingSubnetNodeQueueEpochs::<T>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_value);
        assert_eq!(
            pending.effective_subnet_epoch,
            current_subnet_epoch.saturating_add(1)
        );
    }

    #[benchmark]
    fn owner_update_idle_classification_epochs() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let current_value = IdleClassificationEpochs::<T>::get(subnet_id);
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);

        let new_value = current_value + 1;

        #[extrinsic_call]
        owner_update_idle_classification_epochs(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value,
        );

        let value = IdleClassificationEpochs::<T>::get(subnet_id);
        assert_eq!(value, current_value);
        let pending = PendingIdleClassificationEpochs::<T>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_value);
        assert_eq!(
            pending.effective_subnet_epoch,
            current_subnet_epoch.saturating_add(1)
        );
    }

    #[benchmark]
    fn owner_update_included_classification_epochs() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let current_value = IncludedClassificationEpochs::<T>::get(subnet_id);
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);

        let new_value = current_value + 1;

        #[extrinsic_call]
        owner_update_included_classification_epochs(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value,
        );

        let value = IncludedClassificationEpochs::<T>::get(subnet_id);
        assert_eq!(value, current_value);
        let pending = PendingIncludedClassificationEpochs::<T>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_value);
        assert_eq!(
            pending.effective_subnet_epoch,
            current_subnet_epoch.saturating_add(1)
        );
    }

    #[benchmark]
    fn owner_add_or_update_initial_validators() {
        let block_number = get_current_block_as_u32::<T>();
        let cost = Network::<T>::get_current_registration_cost(block_number)
            .saturating_add(BENCHMARK_REGISTRATION_COST_BUFFER);

        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let subnets = subnet_id_key_offset::<T>(next_subnet_id::<T>());
        let mut register_subnet_data = default_registration_subnet_data::<T>(
            subnets,
            max_subnet_nodes,
            DEFAULT_SUBNET_NAME.into(),
            0,
            T::MaxRegisteredNodesUpperBound::get(),
        );

        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        register_subnet_data.initial_validators = (1..=T::MaxRegisteredNodesUpperBound::get())
            .map(|validator_id| (validator_id, u32::MAX))
            .collect();

        let owner_coldkey =
            funded_initializer::<T>("subnet_owner", subnets * max_subnets * max_subnet_nodes);
        let owner_hotkey =
            get_account::<T>("subnet_owner", subnets * max_subnets * max_subnet_nodes + 1);

        assert_ok!(Network::<T>::register_subnet(
            RawOrigin::Signed(owner_coldkey.clone()).into(),
            100000000000000000000000,
            register_subnet_data,
        ));

        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let owner_coldkey = subnet_owner::<T>(subnet_id);
        max_fill_initial_validator_cleanup_state::<T>(subnet_id);

        let new_value: BTreeMap<u32, u32> = (1..=T::MaxRegisteredNodesUpperBound::get())
            .map(|validator_id| (validator_id, u32::MAX.saturating_sub(1)))
            .collect();

        #[extrinsic_call]
        owner_add_or_update_initial_validators(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value.clone(),
        );

        let validators = NodeRegistrationInitialValidatorIds::<T>::get(subnet_id).unwrap();
        assert_eq!(validators, new_value);
    }

    #[benchmark]
    fn owner_remove_initial_validators() {
        let block_number = get_current_block_as_u32::<T>();
        let cost = Network::<T>::get_current_registration_cost(block_number)
            .saturating_add(BENCHMARK_REGISTRATION_COST_BUFFER);

        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let subnets = subnet_id_key_offset::<T>(next_subnet_id::<T>());
        let mut register_subnet_data = default_registration_subnet_data::<T>(
            subnets,
            max_subnet_nodes,
            DEFAULT_SUBNET_NAME.into(),
            0,
            T::MaxRegisteredNodesUpperBound::get(),
        );

        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey =
            funded_initializer::<T>("subnet_owner", subnets * max_subnets * max_subnet_nodes);
        let owner_hotkey =
            get_account::<T>("subnet_owner", subnets * max_subnets * max_subnet_nodes + 1);

        register_subnet_data.initial_validators = (1..=T::MaxRegisteredNodesUpperBound::get())
            .map(|validator_id| (validator_id, u32::MAX))
            .collect();

        assert_ok!(Network::<T>::register_subnet(
            RawOrigin::Signed(owner_coldkey.clone()).into(),
            100000000000000000000000,
            register_subnet_data,
        ));

        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let expected_validators = NodeRegistrationInitialValidatorIds::<T>::get(subnet_id).unwrap();
        let remove_value: BTreeSet<u32> = (0..T::MaxRegisteredNodesUpperBound::get())
            .map(|offset| 100_000u32.saturating_add(offset))
            .collect();

        #[extrinsic_call]
        owner_remove_initial_validators(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            remove_value.clone(),
        );

        let validators = NodeRegistrationInitialValidatorIds::<T>::get(subnet_id).unwrap();
        assert_eq!(validators, expected_validators);
    }

    #[benchmark]
    fn owner_update_min_max_stake() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let min = SubnetMinStakeBalance::<T>::get(subnet_id);
        let new_min = min + 1;

        let max = SubnetMaxStakeBalance::<T>::get(subnet_id);
        let new_max = max - 1;

        #[extrinsic_call]
        owner_update_min_max_stake(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_min,
            new_max,
        );

        let value = SubnetMinStakeBalance::<T>::get(subnet_id);
        assert_eq!(value, new_min);

        let value = SubnetMaxStakeBalance::<T>::get(subnet_id);
        assert_eq!(value, new_max);
    }

    #[benchmark]
    fn owner_update_delegate_stake_percentage() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let current_value = SubnetDelegateStakeRewardsPercentage::<T>::get(subnet_id);

        let new_value = current_value + 1;

        let block_number = get_current_block_as_u32::<T>();

        let last_update = LastSubnetDelegateStakeRewardsUpdate::<T>::get(subnet_id);
        let update_period = SubnetDelegateStakeRewardsUpdatePeriod::<T>::get();

        let update_to_block = if block_number - last_update < update_period {
            last_update + update_period
        } else {
            block_number
        };

        frame_system::Pallet::<T>::set_block_number(u32_to_block::<T>(update_to_block + 1));

        #[extrinsic_call]
        owner_update_delegate_stake_percentage(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value,
        );

        let active_value = SubnetDelegateStakeRewardsPercentage::<T>::get(subnet_id);
        assert_eq!(active_value, current_value);
        let pending_value = PendingSubnetDelegateStakeRewardsPercentage::<T>::get(subnet_id)
            .map(|pending| pending.value);
        assert_eq!(pending_value, Some(new_value));
    }

    #[benchmark]
    fn owner_update_max_registered_nodes() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let current_value = MaxRegisteredNodes::<T>::get(subnet_id);

        let new_value = current_value - 1;
        TargetNodeRegistrationsPerEpoch::<T>::insert(subnet_id, new_value);

        #[extrinsic_call]
        owner_update_max_registered_nodes(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value,
        );

        let value = MaxRegisteredNodes::<T>::get(subnet_id);
        assert_eq!(value, new_value);
    }

    #[benchmark]
    fn transfer_subnet_ownership() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let current_value = MaxRegisteredNodes::<T>::get(subnet_id);

        let new_owner = funded_initializer::<T>("new_subnet_owner", 0);

        #[extrinsic_call]
        transfer_subnet_ownership(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_owner.clone(),
        );

        let pending_owner = PendingSubnetOwner::<T>::get(subnet_id).unwrap();
        assert_eq!(new_owner.clone(), pending_owner.clone());
    }

    #[benchmark]
    fn cancel_subnet_ownership_transfer() {
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            MinSubnetNodes::<T>::get(),
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let owner = subnet_owner::<T>(subnet_id);
        let new_owner = get_account::<T>("new_subnet_owner", 0);

        assert_ok!(Network::<T>::transfer_subnet_ownership(
            RawOrigin::Signed(owner.clone()).into(),
            subnet_id,
            new_owner,
        ));
        assert!(PendingSubnetOwner::<T>::contains_key(subnet_id));

        #[extrinsic_call]
        cancel_subnet_ownership_transfer(RawOrigin::Signed(owner), subnet_id);

        assert!(!PendingSubnetOwner::<T>::contains_key(subnet_id));
    }

    #[benchmark]
    fn accept_subnet_ownership() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let current_value = MaxRegisteredNodes::<T>::get(subnet_id);

        let new_owner = funded_initializer::<T>("new_subnet_owner", 0);

        assert_ok!(Network::<T>::transfer_subnet_ownership(
            RawOrigin::Signed(owner_coldkey.clone()).into(),
            subnet_id,
            new_owner.clone(),
        ));

        #[extrinsic_call]
        accept_subnet_ownership(RawOrigin::Signed(new_owner.clone()), subnet_id);

        let owner = SubnetOwner::<T>::get(subnet_id).unwrap();
        assert_eq!(new_owner.clone(), owner);
    }

    #[benchmark]
    fn owner_add_bootnode_access() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let new_access = funded_initializer::<T>("new", 0);

        // sanity check
        assert!(SubnetBootnodeAccess::<T>::get(subnet_id)
            .get(&new_access.clone())
            .is_none());

        #[extrinsic_call]
        owner_add_bootnode_access(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_access.clone(),
        );

        let new_access_set = SubnetBootnodeAccess::<T>::get(subnet_id);
        assert!(new_access_set.get(&new_access.clone()).is_some());
    }

    #[benchmark]
    fn owner_remove_bootnode_access() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let new_access = funded_initializer::<T>("new", 0);

        // sanity check
        assert!(SubnetBootnodeAccess::<T>::get(subnet_id)
            .get(&new_access.clone())
            .is_none());

        assert_ok!(Network::<T>::owner_add_bootnode_access(
            RawOrigin::Signed(owner_coldkey.clone()).into(),
            subnet_id,
            new_access.clone()
        ));

        #[extrinsic_call]
        owner_remove_bootnode_access(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_access.clone(),
        );

        let new_access_set = SubnetBootnodeAccess::<T>::get(subnet_id);
        assert!(new_access_set.get(&new_access.clone()).into_iter().count() == 0);
    }

    #[benchmark]
    fn owner_update_target_node_registrations_per_epoch() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let new_value = TargetNodeRegistrationsPerEpoch::<T>::get(subnet_id) - 1;

        #[extrinsic_call]
        owner_update_target_node_registrations_per_epoch(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value,
        );

        assert_eq!(
            TargetNodeRegistrationsPerEpoch::<T>::get(subnet_id),
            new_value
        );
    }

    #[benchmark]
    fn owner_update_node_burn_rate_alpha() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let new_value = NodeBurnRateAlpha::<T>::get(subnet_id) - 1;

        #[extrinsic_call]
        owner_update_node_burn_rate_alpha(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value,
        );

        assert_eq!(NodeBurnRateAlpha::<T>::get(subnet_id), new_value);
    }

    #[benchmark]
    fn owner_update_queue_immunity_epochs() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let new_value = MinQueueEpochs::<T>::get();
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);

        #[extrinsic_call]
        owner_update_queue_immunity_epochs(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value,
        );

        let pending = PendingQueueImmunityEpochs::<T>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_value);
        assert_eq!(
            pending.effective_subnet_epoch,
            current_subnet_epoch.saturating_add(1)
        );
    }

    #[benchmark]
    fn owner_update_subnet_node_min_weight_decrease_reputation_threshold() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let new_value = 1;
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);

        #[extrinsic_call]
        owner_update_subnet_node_min_weight_decrease_reputation_threshold(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value,
        );

        let pending =
            PendingSubnetNodeMinWeightDecreaseReputationThreshold::<T>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_value);
        assert_eq!(
            pending.effective_subnet_epoch,
            current_subnet_epoch.saturating_add(1)
        );
    }

    #[benchmark]
    fn owner_update_churn_limit_multiplier() {
        let (subnet_id, owner_coldkey) = build_owner_benchmark_subnet::<T>();
        let new_value = MinChurnLimitMultiplier::<T>::get();

        #[extrinsic_call]
        owner_update_churn_limit_multiplier(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value,
        );

        assert_eq!(ChurnLimitMultiplier::<T>::get(subnet_id), new_value);
    }

    #[benchmark]
    fn owner_update_min_subnet_node_reputation() {
        let (subnet_id, owner_coldkey) = build_owner_benchmark_subnet::<T>();
        let new_value = MinMinSubnetNodeReputation::<T>::get();
        let current_value = MinSubnetNodeReputation::<T>::get(subnet_id);
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);

        #[extrinsic_call]
        owner_update_min_subnet_node_reputation(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            new_value,
        );

        assert_eq!(MinSubnetNodeReputation::<T>::get(subnet_id), current_value);
        let pending = PendingMinSubnetNodeReputation::<T>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_value);
        assert_eq!(
            pending.effective_subnet_epoch,
            current_subnet_epoch.saturating_add(1)
        );
    }

    #[benchmark]
    fn owner_update_reputation_factors() {
        let (subnet_id, owner_coldkey) = build_owner_benchmark_subnet::<T>();
        let new_value = MinNodeReputationFactor::<T>::get();

        #[extrinsic_call]
        owner_update_reputation_factors(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            SubnetReputationFactorUpdates {
                absent_decrease: Some(new_value),
                included_increase: Some(new_value),
                below_min_weight_decrease: Some(new_value),
                non_attestor_decrease: Some(new_value),
                non_consensus_attestor_decrease: Some(new_value),
                validator_absent_decrease: Some(new_value),
                validator_non_consensus_decrease: Some(new_value),
            },
        );

        let schedule = SubnetReputationFactorSchedules::<T>::get(subnet_id);
        assert_eq!(schedule.pending.unwrap().factors.absent_decrease, new_value);
    }

    #[benchmark]
    fn owner_set_emergency_validator_set() {
        let (subnet_id, owner_coldkey) = build_owner_benchmark_subnet::<T>();
        advance_to_subnet_pause_eligibility::<T>(subnet_id);
        assert_ok!(Network::<T>::owner_pause_subnet(
            RawOrigin::Signed(owner_coldkey.clone()).into(),
            subnet_id
        ));
        let subnet_node_ids = SubnetNodeElectionSlots::<T>::get(subnet_id).to_vec();

        #[extrinsic_call]
        owner_set_emergency_validator_set(
            RawOrigin::Signed(owner_coldkey.clone()),
            subnet_id,
            subnet_node_ids.clone(),
        );

        let emergency_data = EmergencySubnetNodeElectionData::<T>::get(subnet_id).unwrap();
        assert_eq!(emergency_data.subnet_node_ids, subnet_node_ids);
    }

    #[benchmark]
    fn owner_revert_emergency_validator_set() {
        let (subnet_id, owner_coldkey) = build_owner_benchmark_subnet::<T>();
        advance_to_subnet_pause_eligibility::<T>(subnet_id);
        assert_ok!(Network::<T>::owner_pause_subnet(
            RawOrigin::Signed(owner_coldkey.clone()).into(),
            subnet_id
        ));
        let subnet_node_ids = SubnetNodeElectionSlots::<T>::get(subnet_id).to_vec();
        assert_ok!(Network::<T>::owner_set_emergency_validator_set(
            RawOrigin::Signed(owner_coldkey.clone()).into(),
            subnet_id,
            subnet_node_ids
        ));

        #[extrinsic_call]
        owner_revert_emergency_validator_set(RawOrigin::Signed(owner_coldkey.clone()), subnet_id);

        assert!(EmergencySubnetNodeElectionData::<T>::try_get(subnet_id).is_err());
    }

    #[benchmark]
    fn update_bootnodes() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        let min_nodes = MinSubnetNodes::<T>::get();
        let max_subnets = MaxSubnets::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let owner_coldkey = subnet_owner::<T>(subnet_id);

        let new_access = funded_initializer::<T>("new", 0);
        SubnetBootnodeAccess::<T>::insert(subnet_id, BTreeSet::from([new_access.clone()]));
        let addr = get_multiaddr::<T>(Some(subnet_id), Some(99), None).expect("valid multiaddr");
        let peer_id = peer(99);
        let add = BTreeMap::from([(peer_id.clone(), addr.clone())]);

        #[extrinsic_call]
        update_bootnodes(
            RawOrigin::Signed(new_access.clone()),
            subnet_id,
            add.clone(),
            BTreeSet::new(),
        );

        let stored = SubnetBootnodes::<T>::get(subnet_id);
        assert_eq!(stored.get(&peer_id), Some(&addr));
    }

    #[benchmark]
    fn register_subnet_node() {
        let end = MinSubnetNodes::<T>::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let validator_id = end + 1;
        let (coldkey, _validator_hotkey) = ensure_validator::<T>(validator_id);

        // Exercise the largest validator-wide ownership/allocation values accepted by the
        // registration path. These synthetic IDs size both nested storage values.
        let max_validator_nodes = T::MaxValidatorNodesUpperBound::get();
        assert!(max_validator_nodes > 0);
        let existing_node_count = max_validator_nodes.saturating_sub(1);
        let existing_node_ids: BTreeSet<u32> = (0..existing_node_count)
            .map(|offset| 1_000_000u32.saturating_add(offset))
            .collect();
        ValidatorSubnetNodes::<T>::insert(
            validator_id,
            BTreeMap::from([(subnet_id, existing_node_ids.clone())]),
        );
        TotalValidatorNodes::<T>::insert(validator_id, existing_node_count);
        ValidatorNodeDelegateStakeWeights::<T>::insert(
            validator_id,
            existing_node_ids
                .into_iter()
                .map(|subnet_node_id| ((subnet_id, subnet_node_id), 0))
                .collect::<BTreeMap<_, _>>(),
        );

        let node_hotkey = get_account::<T>("benchmark_node_hotkey", 0);
        let stake_to_be_added = DEFAULT_SUBNET_NODE_STAKE;
        let burn_amount = Network::<T>::calculate_burn_amount(subnet_id);
        fund_account::<T>(
            &coldkey,
            stake_to_be_added
                .saturating_add(burn_amount)
                .saturating_add(DEFAULT_DEPOSIT_AMOUNT),
        );

        let peer_info = benchmark_peer_info::<T>(subnet_id, end + 1, None);
        let bootnode_peer_info = benchmark_peer_info::<T>(subnet_id, end + 1, Some(1));
        let client_peer_info = benchmark_peer_info::<T>(subnet_id, end + 1, Some(2));

        #[extrinsic_call]
        register_subnet_node(
            RawOrigin::Signed(coldkey.clone()),
            validator_id,
            subnet_id,
            Some(node_hotkey.clone()),
            Some(peer_info.clone()),
            Some(bootnode_peer_info),
            Some(client_peer_info),
            stake_to_be_added,
            None,
            None,
            u128::MAX,
        );

        let subnet_node_id = TotalSubnetNodeUids::<T>::get(subnet_id);
        assert_eq!(
            SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id),
            Some(validator_id)
        );
        let registered_subnet_node = RegisteredSubnetNodesData::<T>::get(subnet_id, subnet_node_id);
        assert_eq!(registered_subnet_node.peer_info, Some(peer_info));
        assert!(SubnetNodeQueue::<T>::get(subnet_id)
            .iter()
            .any(|subnet_node| subnet_node.id == subnet_node_id));
        assert_eq!(
            SubnetNodeIdHotkey::<T>::get(subnet_id, subnet_node_id),
            Some(node_hotkey)
        );
        assert_eq!(
            ValidatorSubnetNodes::<T>::get(validator_id)
                .values()
                .map(BTreeSet::len)
                .sum::<usize>(),
            max_validator_nodes as usize
        );
        assert_eq!(
            ValidatorNodeDelegateStakeWeights::<T>::get(validator_id).len(),
            max_validator_nodes as usize
        );
        assert_eq!(
            TotalValidatorNodes::<T>::get(validator_id),
            max_validator_nodes
        );
    }

    #[benchmark]
    fn remove_subnet_node() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let subnet_node_id = TotalSubnetNodeUids::<T>::get(subnet_id);
        let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id).unwrap();
        let coldkey = ValidatorColdkey::<T>::get(validator_id).unwrap();

        // Benchmark only the signed/auth wrapper. The generated active/registered branch model is
        // added by the dispatch annotation, so retaining that work here would double-charge it.
        // Keep the ownership mapping needed by auth, but make the internal branch selector a
        // no-op; this also measures both presence checks performed by `perform_remove_subnet_node`.
        SubnetNodesData::<T>::remove(subnet_id, subnet_node_id);
        assert!(!RegisteredSubnetNodesData::<T>::contains_key(
            subnet_id,
            subnet_node_id
        ));

        #[extrinsic_call]
        remove_subnet_node(
            RawOrigin::Signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
        );

        assert_eq!(TotalSubnetNodes::<T>::get(subnet_id), end);
        assert_eq!(
            SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id),
            Some(validator_id)
        );
    }

    #[benchmark]
    fn update_node_hotkey() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let subnet_node_id = end;
        let coldkey =
            Network::<T>::get_subnet_node_associated_coldkey(subnet_id, subnet_node_id).unwrap();
        let new_hotkey = get_account::<T>("updated_node_hotkey", 0);

        #[extrinsic_call]
        update_node_hotkey(
            RawOrigin::Signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            Some(new_hotkey.clone()),
        );

        assert_eq!(
            SubnetNodeIdHotkey::<T>::get(subnet_id, subnet_node_id),
            Some(new_hotkey)
        );
    }

    #[benchmark]
    fn update_validator_delegate_reward_rate() {
        let validator_id = 1;
        let (coldkey, _hotkey) = ensure_validator::<T>(validator_id);
        let validator = ValidatorsData::<T>::get(validator_id);
        let current_value = validator.delegate_reward_rate;
        let new_value = current_value + 1;

        let reward_rate_update_period = NodeRewardRateUpdatePeriod::<T>::get();
        let block_number = get_current_block_as_u32::<T>();
        frame_system::Pallet::<T>::set_block_number(u32_to_block::<T>(
            block_number + reward_rate_update_period,
        ));

        #[extrinsic_call]
        update_validator_delegate_reward_rate(
            RawOrigin::Signed(coldkey.clone()),
            validator_id,
            new_value,
        );

        assert_eq!(
            ValidatorsData::<T>::get(validator_id).delegate_reward_rate,
            new_value
        );
    }

    #[benchmark]
    fn add_node_stake() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let subnet_node_id = end;
        let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id).unwrap();
        let coldkey = ValidatorColdkey::<T>::get(validator_id).unwrap();
        fund_account::<T>(&coldkey, DEFAULT_STAKE_TO_BE_ADDED);
        frame_system::Pallet::<T>::set_block_number(u32_to_block::<T>(
            get_current_block_as_u32::<T>() + TxRateLimit::<T>::get() + 1,
        ));
        let previous_stake = NodeSubnetStake::<T>::get(subnet_node_id, subnet_id);

        #[extrinsic_call]
        add_node_stake(
            RawOrigin::Signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            DEFAULT_STAKE_TO_BE_ADDED,
        );

        assert_eq!(
            NodeSubnetStake::<T>::get(subnet_node_id, subnet_id),
            previous_stake + DEFAULT_STAKE_TO_BE_ADDED
        );
    }

    #[benchmark]
    fn remove_node_stake() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let subnet_node_id = end;
        let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id).unwrap();
        let coldkey = ValidatorColdkey::<T>::get(validator_id).unwrap();
        fund_account::<T>(&coldkey, DEFAULT_STAKE_TO_BE_ADDED);
        assert_ok!(Network::<T>::add_node_stake(
            RawOrigin::Signed(coldkey.clone()).into(),
            subnet_id,
            subnet_node_id,
            DEFAULT_STAKE_TO_BE_ADDED
        ));
        frame_system::Pallet::<T>::set_block_number(u32_to_block::<T>(
            get_current_block_as_u32::<T>() + TxRateLimit::<T>::get() + 1,
        ));
        let amount_to_remove = DEFAULT_STAKE_TO_BE_ADDED;
        let previous_stake = NodeSubnetStake::<T>::get(subnet_node_id, subnet_id);
        let block = get_current_block_as_u32::<T>();
        let claim_block = block
            .checked_add(
                StakeCooldownEpochs::<T>::get()
                    .checked_mul(T::EpochLength::get())
                    .expect("benchmark cooldown fits u32"),
            )
            .expect("benchmark claim block fits u32");
        let seeded_ledger = prime_max_unbonding_ledger_for_merge::<T>(&coldkey, claim_block);
        let seeded_entry = *seeded_ledger
            .get(&claim_block)
            .expect("target claim block is seeded");
        let total_network_unbonding_before = TotalNetworkUnbondingBalance::<T>::get();

        #[extrinsic_call]
        remove_node_stake(
            RawOrigin::Signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            amount_to_remove,
        );

        assert_eq!(
            NodeSubnetStake::<T>::get(subnet_node_id, subnet_id),
            previous_stake - amount_to_remove
        );
        let unbondings = StakeUnbondingLedger::<T>::get(&coldkey);
        assert_eq!(unbondings.len() as u32, T::MaxUnbondingsUpperBound::get());
        assert_eq!(
            unbondings.get(&claim_block),
            Some(&UnbondingEntry {
                network: seeded_entry.network + amount_to_remove,
                overwatch: seeded_entry.overwatch,
            })
        );
        assert_eq!(
            TotalNetworkUnbondingBalance::<T>::get(),
            total_network_unbonding_before + amount_to_remove
        );
    }

    #[benchmark]
    fn claim_unbondings() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let delegate_account: T::AccountId = funded_account::<T>("delegate_account", 0);
        assert_ok!(T::Currency::transfer(
            &get_alice::<T>(), // alice
            &delegate_account.clone(),
            (DEFAULT_DELEGATE_STAKE_TO_BE_ADDED + 500)
                .try_into()
                .ok()
                .expect("REASON"),
            ExistenceRequirement::KeepAlive,
        ));

        assert_ok!(Network::<T>::add_subnet_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()).into(),
            subnet_id,
            DEFAULT_DELEGATE_STAKE_TO_BE_ADDED
        ));
        let delegate_shares =
            AccountSubnetDelegateStakeShares::<T>::get(delegate_account.clone(), subnet_id);

        let total_subnet_delegated_stake_shares =
            TotalSubnetDelegateStakeShares::<T>::get(subnet_id);
        let total_subnet_delegated_stake_balance =
            TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);

        let delegate_balance = Network::<T>::convert_to_balance(
            delegate_shares,
            total_subnet_delegated_stake_shares,
            total_subnet_delegated_stake_balance,
        );

        let block = get_current_block_as_u32::<T>();
        let claim_block = block
            .checked_add(
                DelegateStakeCooldownEpochs::<T>::get()
                    .checked_mul(T::EpochLength::get())
                    .expect("benchmark cooldown fits u32"),
            )
            .expect("benchmark claim block fits u32");
        prime_max_unbonding_ledger_for_merge::<T>(&delegate_account, claim_block);

        assert_ok!(Network::<T>::remove_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()).into(),
            subnet_id,
            delegate_shares
        ));

        let unbondings: BTreeMap<u32, UnbondingEntry> =
            StakeUnbondingLedger::<T>::get(delegate_account.clone());
        assert_eq!(unbondings.len() as u32, T::MaxUnbondingsUpperBound::get());
        let balance = unbondings
            .get(&claim_block)
            .expect("removed stake merges at the target claim block");
        assert!(balance.network > delegate_balance);

        let total_claim_balance = unbondings.values().fold(0u128, |total, entry| {
            total
                .checked_add(entry.network)
                .and_then(|value| value.checked_add(entry.overwatch))
                .expect("benchmark claim principal fits u128")
        });
        let total_network_unbonding = unbondings.values().fold(0u128, |total, entry| {
            total
                .checked_add(entry.network)
                .expect("benchmark network principal fits u128")
        });
        assert_eq!(
            TotalNetworkUnbondingBalance::<T>::get(),
            total_network_unbonding
        );

        let pre_delegator_balance: u128 = T::Currency::free_balance(&delegate_account.clone())
            .try_into()
            .ok()
            .expect("REASON");

        let last_claim_block = *unbondings
            .last_key_value()
            .expect("maximum benchmark ledger is non-empty")
            .0;
        frame_system::Pallet::<T>::set_block_number(u32_to_block::<T>(
            last_claim_block
                .checked_add(1)
                .expect("benchmark claim block fits u32"),
        ));

        #[extrinsic_call]
        claim_unbondings(RawOrigin::Signed(delegate_account.clone()));

        let post_delegator_balance: u128 = T::Currency::free_balance(&delegate_account.clone())
            .try_into()
            .ok()
            .expect("REASON");

        assert_eq!(
            post_delegator_balance,
            pre_delegator_balance + total_claim_balance
        );
        assert!(StakeUnbondingLedger::<T>::get(&delegate_account).is_empty());
        assert_eq!(TotalNetworkUnbondingBalance::<T>::get(), 0);
    }

    #[benchmark]
    fn add_subnet_delegate_stake() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let delegate_account: T::AccountId = funded_account::<T>("delegate_account", 0);

        let _ = T::Currency::deposit_creating(
            &delegate_account.clone(),
            (DEFAULT_STAKE_TO_BE_ADDED + 500)
                .try_into()
                .ok()
                .expect("REASON"),
        );
        let starting_delegator_balance = T::Currency::free_balance(&delegate_account.clone());

        #[extrinsic_call]
        add_subnet_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()),
            subnet_id,
            DEFAULT_STAKE_TO_BE_ADDED,
        );

        let post_delegator_balance = T::Currency::free_balance(&delegate_account.clone());
        assert_eq!(
            post_delegator_balance,
            starting_delegator_balance - DEFAULT_STAKE_TO_BE_ADDED.try_into().ok().expect("REASON")
        );

        let total_subnet_delegated_stake_shares =
            TotalSubnetDelegateStakeShares::<T>::get(subnet_id);
        let total_subnet_delegated_stake_balance =
            TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);
        let delegate_shares =
            AccountSubnetDelegateStakeShares::<T>::get(delegate_account.clone(), subnet_id);
        let delegate_balance = Network::<T>::convert_to_balance(
            delegate_shares,
            total_subnet_delegated_stake_shares,
            total_subnet_delegated_stake_balance,
        );

        // Ensure balance is within <= 0.01% of deposited balance, and less than deposited balance
        assert!(
            (delegate_balance
                >= Network::<T>::percent_mul(DEFAULT_STAKE_TO_BE_ADDED, 990000000000000000))
                && (delegate_balance < DEFAULT_STAKE_TO_BE_ADDED)
        );
    }

    #[benchmark]
    fn swap_from_subnet_to_subnet() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let from_subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME_2.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let to_subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME_2.into()).unwrap();

        let delegate_account: T::AccountId = funded_account::<T>("delegate_account", 0);
        assert_ok!(T::Currency::transfer(
            &get_alice::<T>(), // alice
            &delegate_account.clone(),
            (DEFAULT_DELEGATE_STAKE_TO_BE_ADDED + 500)
                .try_into()
                .ok()
                .expect("REASON"),
            ExistenceRequirement::KeepAlive,
        ));

        assert_ok!(Network::<T>::add_subnet_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()).into(),
            from_subnet_id,
            DEFAULT_DELEGATE_STAKE_TO_BE_ADDED
        ));

        let delegate_shares =
            AccountSubnetDelegateStakeShares::<T>::get(delegate_account.clone(), from_subnet_id);
        let total_subnet_delegated_stake_shares =
            TotalSubnetDelegateStakeShares::<T>::get(from_subnet_id);
        let total_subnet_delegated_stake_balance =
            TotalSubnetDelegateStakeBalance::<T>::get(from_subnet_id);

        let from_delegate_balance = Network::<T>::convert_to_balance(
            delegate_shares,
            total_subnet_delegated_stake_shares,
            total_subnet_delegated_stake_balance,
        );
        let prev_total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<T>::get(from_subnet_id);
        prime_near_full_swap_queue::<T>();
        let prev_next_id = NextSwapQueueId::<T>::get();
        let queued_principal_before = TotalQueuedSwapPrincipal::<T>::get();

        #[extrinsic_call]
        swap_from_subnet_to_subnet(
            RawOrigin::Signed(delegate_account.clone()),
            from_subnet_id,
            to_subnet_id,
            delegate_shares,
        );

        let from_delegate_shares =
            AccountSubnetDelegateStakeShares::<T>::get(delegate_account.clone(), from_subnet_id);
        assert_eq!(from_delegate_shares, 0);

        assert_ne!(
            prev_total_subnet_delegate_stake_balance,
            TotalSubnetDelegateStakeBalance::<T>::get(from_subnet_id)
        );
        assert!(
            prev_total_subnet_delegate_stake_balance
                > TotalSubnetDelegateStakeBalance::<T>::get(from_subnet_id)
        );

        // Check the queue
        let starting_to_subnet_id = to_subnet_id;
        let call_queue = SwapCallQueue::<T>::get(prev_next_id);
        assert_eq!(call_queue.clone().unwrap().id, prev_next_id);
        match &call_queue.clone().unwrap().call {
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id,
                to_subnet_id,
                balance,
            } => {
                assert_eq!(*account_id, delegate_account.clone());
                assert_eq!(*to_subnet_id, starting_to_subnet_id);
                assert_ne!(*balance, 0);
            }
            QueuedSwapCall::SwapToValidatorDelegateStake { .. } => assert!(false),
        };

        let next_id = NextSwapQueueId::<T>::get();
        assert_eq!(prev_next_id + 1, next_id);
        let queue = SwapQueueOrder::<T>::get();
        assert!(queue
            .last()
            .map_or(false, |&last_id| last_id == prev_next_id));
        assert_eq!(queue.len() as u32, T::MaxSwapQueueLength::get());
        assert_eq!(SwapQueueCount::<T>::get(), T::MaxSwapQueueLength::get());
        assert_eq!(
            TotalQueuedSwapPrincipal::<T>::get(),
            queued_principal_before
                .checked_add(call_queue.unwrap().call.get_queue_balance())
                .expect("benchmark queue principal fits u128")
        );
        assert_benchmark_queued_swap_principal::<T>();
    }

    #[benchmark]
    fn transfer_delegate_stake() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let delegate_account: T::AccountId = funded_account::<T>("delegate_account", 0);
        assert_ok!(T::Currency::transfer(
            &get_alice::<T>(), // alice
            &delegate_account.clone(),
            (DEFAULT_DELEGATE_STAKE_TO_BE_ADDED + 500)
                .try_into()
                .ok()
                .expect("REASON"),
            ExistenceRequirement::KeepAlive,
        ));

        assert_ok!(Network::<T>::add_subnet_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()).into(),
            subnet_id,
            DEFAULT_DELEGATE_STAKE_TO_BE_ADDED
        ));

        let to_delegate_account: T::AccountId = funded_account::<T>("to_delegate_account", 0);

        let delegate_shares =
            AccountSubnetDelegateStakeShares::<T>::get(delegate_account.clone(), subnet_id);

        #[extrinsic_call]
        transfer_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()),
            subnet_id,
            to_delegate_account.clone(),
            delegate_shares,
        );

        assert_eq!(
            0,
            AccountSubnetDelegateStakeShares::<T>::get(delegate_account.clone(), subnet_id)
        );
        assert_eq!(
            delegate_shares,
            AccountSubnetDelegateStakeShares::<T>::get(to_delegate_account.clone(), subnet_id)
        )
    }

    #[benchmark]
    fn remove_delegate_stake() {
        let end = 12;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let delegate_account: T::AccountId = funded_account::<T>("delegate_account", 0);
        assert_ok!(T::Currency::transfer(
            &get_alice::<T>(), // alice
            &delegate_account.clone(),
            (DEFAULT_DELEGATE_STAKE_TO_BE_ADDED + 500)
                .try_into()
                .ok()
                .expect("REASON"),
            ExistenceRequirement::KeepAlive,
        ));
        assert_ok!(Network::<T>::add_subnet_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()).into(),
            subnet_id,
            DEFAULT_DELEGATE_STAKE_TO_BE_ADDED
        ));
        let delegate_shares =
            AccountSubnetDelegateStakeShares::<T>::get(delegate_account.clone(), subnet_id);

        let total_subnet_delegated_stake_shares =
            TotalSubnetDelegateStakeShares::<T>::get(subnet_id);
        let total_subnet_delegated_stake_balance =
            TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);

        let delegate_balance = Network::<T>::convert_to_balance(
            delegate_shares,
            total_subnet_delegated_stake_shares,
            total_subnet_delegated_stake_balance,
        );

        let block = get_current_block_as_u32::<T>();
        let claim_block = block
            .checked_add(
                DelegateStakeCooldownEpochs::<T>::get()
                    .checked_mul(T::EpochLength::get())
                    .expect("benchmark cooldown fits u32"),
            )
            .expect("benchmark claim block fits u32");
        let seeded_ledger =
            prime_max_unbonding_ledger_for_merge::<T>(&delegate_account, claim_block);
        let seeded_entry = *seeded_ledger
            .get(&claim_block)
            .expect("target claim block is seeded");
        let total_network_unbonding_before = TotalNetworkUnbondingBalance::<T>::get();

        #[extrinsic_call]
        remove_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()),
            subnet_id,
            delegate_shares,
        );

        let unbondings: BTreeMap<u32, UnbondingEntry> =
            StakeUnbondingLedger::<T>::get(delegate_account.clone());
        assert_eq!(unbondings.len() as u32, T::MaxUnbondingsUpperBound::get());
        assert_eq!(
            unbondings.get(&claim_block),
            Some(&UnbondingEntry {
                network: seeded_entry.network + delegate_balance,
                overwatch: seeded_entry.overwatch,
            })
        );
        assert_eq!(
            TotalNetworkUnbondingBalance::<T>::get(),
            total_network_unbonding_before + delegate_balance
        );
    }

    #[benchmark]
    fn donate_delegate_stake() {
        let end = 12;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let delegate_account: T::AccountId = funded_account::<T>("delegate_account", 0);
        assert_ok!(T::Currency::transfer(
            &get_alice::<T>(), // alice
            &delegate_account.clone(),
            (DEFAULT_DELEGATE_STAKE_TO_BE_ADDED + 500)
                .try_into()
                .ok()
                .expect("REASON"),
            ExistenceRequirement::KeepAlive,
        ));

        assert_ok!(Network::<T>::add_subnet_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()).into(),
            subnet_id,
            DEFAULT_DELEGATE_STAKE_TO_BE_ADDED
        ));

        let delegate_shares =
            AccountSubnetDelegateStakeShares::<T>::get(delegate_account.clone(), subnet_id);
        let total_subnet_delegated_stake_shares =
            TotalSubnetDelegateStakeShares::<T>::get(subnet_id);
        let total_subnet_delegated_stake_balance =
            TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);

        let delegate_balance = Network::<T>::convert_to_balance(
            delegate_shares,
            total_subnet_delegated_stake_shares,
            total_subnet_delegated_stake_balance,
        );

        let funder = funded_account::<T>("funder", 0);

        #[extrinsic_call]
        donate_delegate_stake(
            RawOrigin::Signed(funder),
            subnet_id,
            DEFAULT_SUBNET_NODE_STAKE,
        );

        let increased_delegate_shares =
            AccountSubnetDelegateStakeShares::<T>::get(delegate_account.clone(), subnet_id);
        let increased_total_subnet_delegated_stake_shares =
            TotalSubnetDelegateStakeShares::<T>::get(subnet_id);
        let increased_total_subnet_delegated_stake_balance =
            TotalSubnetDelegateStakeBalance::<T>::get(subnet_id);

        let increased_delegate_balance = Network::<T>::convert_to_balance(
            increased_delegate_shares,
            increased_total_subnet_delegated_stake_shares,
            increased_total_subnet_delegated_stake_balance,
        );
        assert_eq!(
            increased_total_subnet_delegated_stake_balance,
            total_subnet_delegated_stake_balance + DEFAULT_SUBNET_NODE_STAKE
        );

        assert_ne!(increased_delegate_balance, delegate_balance);
        assert!(increased_delegate_balance > delegate_balance);
    }

    #[benchmark]
    fn add_validator_delegate_stake() {
        let validator_id = 1;
        ensure_validator::<T>(validator_id);

        let delegate_account: T::AccountId = funded_account::<T>("delegate_account", 0);
        fund_account::<T>(&delegate_account, DEFAULT_DELEGATE_STAKE_TO_BE_ADDED);

        #[extrinsic_call]
        add_validator_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()),
            validator_id,
            DEFAULT_DELEGATE_STAKE_TO_BE_ADDED,
        );

        let account_validator_delegate_stake_shares =
            AccountValidatorDelegateStakeShares::<T>::get(&delegate_account, validator_id);
        let total_validator_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<T>::get(validator_id);
        let total_validator_delegate_stake_shares =
            ValidatorDelegateStakeShares::<T>::get(validator_id);

        let account_validator_delegate_stake_balance = Network::<T>::convert_to_balance(
            account_validator_delegate_stake_shares,
            total_validator_delegate_stake_shares,
            total_validator_delegate_stake_balance,
        );

        assert!(
            account_validator_delegate_stake_balance
                >= Network::<T>::percent_mul(
                    DEFAULT_DELEGATE_STAKE_TO_BE_ADDED,
                    990000000000000000
                )
        );
        assert!(account_validator_delegate_stake_balance <= DEFAULT_DELEGATE_STAKE_TO_BE_ADDED);
    }

    #[benchmark]
    fn swap_from_validator_to_validator() {
        let from_validator_id = 1;
        let to_validator_id = 2;
        ensure_validator::<T>(from_validator_id);
        ensure_validator::<T>(to_validator_id);

        let delegate_account: T::AccountId = funded_account::<T>("delegate_account", 0);
        fund_account::<T>(&delegate_account, DEFAULT_DELEGATE_STAKE_TO_BE_ADDED);

        assert_ok!(Network::<T>::add_validator_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()).into(),
            from_validator_id,
            DEFAULT_DELEGATE_STAKE_TO_BE_ADDED
        ));

        frame_system::Pallet::<T>::set_block_number(u32_to_block::<T>(
            get_current_block_as_u32::<T>() + TxRateLimit::<T>::get() + 1,
        ));

        let delegate_shares =
            AccountValidatorDelegateStakeShares::<T>::get(&delegate_account, from_validator_id);
        let delegate_shares_to_swap = delegate_shares / 2;
        prime_near_full_swap_queue::<T>();
        let prev_next_id = NextSwapQueueId::<T>::get();
        let queued_principal_before = TotalQueuedSwapPrincipal::<T>::get();

        #[extrinsic_call]
        swap_from_validator_to_validator(
            RawOrigin::Signed(delegate_account.clone()),
            from_validator_id,
            to_validator_id,
            delegate_shares_to_swap,
        );

        assert_eq!(
            AccountValidatorDelegateStakeShares::<T>::get(&delegate_account, from_validator_id),
            delegate_shares - delegate_shares_to_swap
        );

        let call_queue = SwapCallQueue::<T>::get(prev_next_id).unwrap();
        assert_eq!(call_queue.id, prev_next_id);
        match &call_queue.call {
            QueuedSwapCall::SwapToSubnetDelegateStake { .. } => assert!(false),
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id,
                to_validator_id: queued_to_validator_id,
                balance,
            } => {
                assert_eq!(*account_id, delegate_account.clone());
                assert_eq!(*queued_to_validator_id, to_validator_id);
                assert_ne!(*balance, 0);
            }
        };

        assert_eq!(NextSwapQueueId::<T>::get(), prev_next_id + 1);
        let queue = SwapQueueOrder::<T>::get();
        assert!(queue
            .last()
            .map_or(false, |&last_id| last_id == prev_next_id));
        assert_eq!(queue.len() as u32, T::MaxSwapQueueLength::get());
        assert_eq!(SwapQueueCount::<T>::get(), T::MaxSwapQueueLength::get());
        assert_eq!(
            TotalQueuedSwapPrincipal::<T>::get(),
            queued_principal_before
                .checked_add(call_queue.call.get_queue_balance())
                .expect("benchmark queue principal fits u128")
        );
        assert_benchmark_queued_swap_principal::<T>();
    }

    #[benchmark]
    fn transfer_validator_delegate_stake() {
        let validator_id = 1;
        ensure_validator::<T>(validator_id);

        let delegate_account: T::AccountId = funded_account::<T>("delegate_account", 0);
        fund_account::<T>(&delegate_account, DEFAULT_DELEGATE_STAKE_TO_BE_ADDED);

        assert_ok!(Network::<T>::add_validator_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()).into(),
            validator_id,
            DEFAULT_DELEGATE_STAKE_TO_BE_ADDED
        ));

        let to_delegate_account: T::AccountId = funded_account::<T>("to_delegate_account", 0);
        let delegate_shares =
            AccountValidatorDelegateStakeShares::<T>::get(&delegate_account, validator_id);

        #[extrinsic_call]
        transfer_validator_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()),
            validator_id,
            to_delegate_account.clone(),
            delegate_shares,
        );

        assert_eq!(
            0,
            AccountValidatorDelegateStakeShares::<T>::get(&delegate_account, validator_id)
        );
        assert_eq!(
            delegate_shares,
            AccountValidatorDelegateStakeShares::<T>::get(&to_delegate_account, validator_id)
        );
    }

    #[benchmark]
    fn remove_validator_delegate_stake() {
        let validator_id = 1;
        ensure_validator::<T>(validator_id);

        let delegate_account: T::AccountId = funded_account::<T>("delegate_account", 0);
        fund_account::<T>(&delegate_account, DEFAULT_DELEGATE_STAKE_TO_BE_ADDED);

        assert_ok!(Network::<T>::add_validator_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()).into(),
            validator_id,
            DEFAULT_DELEGATE_STAKE_TO_BE_ADDED
        ));

        frame_system::Pallet::<T>::set_block_number(u32_to_block::<T>(
            get_current_block_as_u32::<T>() + TxRateLimit::<T>::get() + 1,
        ));

        let delegate_shares =
            AccountValidatorDelegateStakeShares::<T>::get(&delegate_account, validator_id);
        let delegate_balance = Network::<T>::convert_to_balance(
            delegate_shares,
            ValidatorDelegateStakeShares::<T>::get(validator_id),
            ValidatorDelegateStakeBalance::<T>::get(validator_id),
        );
        let block = get_current_block_as_u32::<T>();
        let claim_block = block
            .checked_add(
                DelegateStakeCooldownEpochs::<T>::get()
                    .checked_mul(T::EpochLength::get())
                    .expect("benchmark cooldown fits u32"),
            )
            .expect("benchmark claim block fits u32");
        let seeded_ledger =
            prime_max_unbonding_ledger_for_merge::<T>(&delegate_account, claim_block);
        let seeded_entry = *seeded_ledger
            .get(&claim_block)
            .expect("target claim block is seeded");
        let total_network_unbonding_before = TotalNetworkUnbondingBalance::<T>::get();

        #[extrinsic_call]
        remove_validator_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()),
            validator_id,
            delegate_shares,
        );

        assert_eq!(
            0,
            AccountValidatorDelegateStakeShares::<T>::get(&delegate_account, validator_id)
        );
        let unbondings = StakeUnbondingLedger::<T>::get(&delegate_account);
        assert_eq!(unbondings.len() as u32, T::MaxUnbondingsUpperBound::get());
        assert_eq!(
            unbondings.get(&claim_block),
            Some(&UnbondingEntry {
                network: seeded_entry.network + delegate_balance,
                overwatch: seeded_entry.overwatch,
            })
        );
        assert_eq!(
            TotalNetworkUnbondingBalance::<T>::get(),
            total_network_unbonding_before + delegate_balance
        );
    }

    #[benchmark]
    fn donate_validator_delegate_stake() {
        let validator_id = 1;
        ensure_validator::<T>(validator_id);

        let funder: T::AccountId = funded_account::<T>("funder", 0);
        fund_account::<T>(&funder, DEFAULT_DELEGATE_STAKE_TO_BE_ADDED);

        let pre_total_validator_delegate_stake_balance =
            ValidatorDelegateStakeBalance::<T>::get(validator_id);

        #[extrinsic_call]
        donate_validator_delegate_stake(
            RawOrigin::Signed(funder),
            validator_id,
            DEFAULT_DELEGATE_STAKE_TO_BE_ADDED,
        );

        assert_eq!(
            ValidatorDelegateStakeBalance::<T>::get(validator_id),
            pre_total_validator_delegate_stake_balance + DEFAULT_DELEGATE_STAKE_TO_BE_ADDED
        );
    }

    #[benchmark]
    fn swap_from_validator_to_subnet() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let to_subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let from_validator_id = 1;
        ensure_validator::<T>(from_validator_id);

        let delegate_account: T::AccountId = funded_account::<T>("delegate_account", 0);
        fund_account::<T>(&delegate_account, DEFAULT_DELEGATE_STAKE_TO_BE_ADDED);

        assert_ok!(Network::<T>::add_validator_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()).into(),
            from_validator_id,
            DEFAULT_DELEGATE_STAKE_TO_BE_ADDED
        ));

        frame_system::Pallet::<T>::set_block_number(u32_to_block::<T>(
            get_current_block_as_u32::<T>() + TxRateLimit::<T>::get() + 1,
        ));

        let delegate_shares =
            AccountValidatorDelegateStakeShares::<T>::get(&delegate_account, from_validator_id);
        let delegate_shares_to_swap = delegate_shares / 2;
        prime_near_full_swap_queue::<T>();
        let prev_next_id = NextSwapQueueId::<T>::get();
        let queued_principal_before = TotalQueuedSwapPrincipal::<T>::get();

        #[extrinsic_call]
        swap_from_validator_to_subnet(
            RawOrigin::Signed(delegate_account.clone()),
            from_validator_id,
            to_subnet_id,
            delegate_shares_to_swap,
        );

        assert_eq!(
            AccountValidatorDelegateStakeShares::<T>::get(&delegate_account, from_validator_id),
            delegate_shares - delegate_shares_to_swap
        );

        let call_queue = SwapCallQueue::<T>::get(prev_next_id).unwrap();
        assert_eq!(call_queue.id, prev_next_id);
        match &call_queue.call {
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id,
                to_subnet_id: queued_to_subnet_id,
                balance,
            } => {
                assert_eq!(*account_id, delegate_account.clone());
                assert_eq!(*queued_to_subnet_id, to_subnet_id);
                assert_ne!(*balance, 0);
            }
            QueuedSwapCall::SwapToValidatorDelegateStake { .. } => assert!(false),
        };

        assert_eq!(NextSwapQueueId::<T>::get(), prev_next_id + 1);
        let queue = SwapQueueOrder::<T>::get();
        assert!(queue
            .last()
            .map_or(false, |&last_id| last_id == prev_next_id));
        assert_eq!(queue.len() as u32, T::MaxSwapQueueLength::get());
        assert_eq!(SwapQueueCount::<T>::get(), T::MaxSwapQueueLength::get());
        assert_eq!(
            TotalQueuedSwapPrincipal::<T>::get(),
            queued_principal_before
                .checked_add(call_queue.call.get_queue_balance())
                .expect("benchmark queue principal fits u128")
        );
        assert_benchmark_queued_swap_principal::<T>();
    }

    #[benchmark]
    fn swap_from_subnet_to_validator() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let from_subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let to_validator_id = 1;
        ensure_validator::<T>(to_validator_id);

        let delegate_account: T::AccountId = funded_account::<T>("delegate_account", 0);
        fund_account::<T>(&delegate_account, DEFAULT_DELEGATE_STAKE_TO_BE_ADDED);

        assert_ok!(Network::<T>::add_subnet_delegate_stake(
            RawOrigin::Signed(delegate_account.clone()).into(),
            from_subnet_id,
            DEFAULT_DELEGATE_STAKE_TO_BE_ADDED
        ));

        frame_system::Pallet::<T>::set_block_number(u32_to_block::<T>(
            get_current_block_as_u32::<T>() + TxRateLimit::<T>::get() + 1,
        ));

        let delegate_shares =
            AccountSubnetDelegateStakeShares::<T>::get(delegate_account.clone(), from_subnet_id);
        prime_near_full_swap_queue::<T>();
        let prev_next_id = NextSwapQueueId::<T>::get();
        let queued_principal_before = TotalQueuedSwapPrincipal::<T>::get();

        #[extrinsic_call]
        swap_from_subnet_to_validator(
            RawOrigin::Signed(delegate_account.clone()),
            from_subnet_id,
            to_validator_id,
            delegate_shares,
        );

        assert_eq!(
            AccountSubnetDelegateStakeShares::<T>::get(delegate_account.clone(), from_subnet_id),
            0
        );

        let call_queue = SwapCallQueue::<T>::get(prev_next_id).unwrap();
        assert_eq!(call_queue.id, prev_next_id);
        match &call_queue.call {
            QueuedSwapCall::SwapToSubnetDelegateStake { .. } => assert!(false),
            QueuedSwapCall::SwapToValidatorDelegateStake {
                account_id,
                to_validator_id: queued_to_validator_id,
                balance,
            } => {
                assert_eq!(*account_id, delegate_account.clone());
                assert_eq!(*queued_to_validator_id, to_validator_id);
                assert_ne!(*balance, 0);
            }
        };

        assert_eq!(NextSwapQueueId::<T>::get(), prev_next_id + 1);
        let queue = SwapQueueOrder::<T>::get();
        assert!(queue
            .last()
            .map_or(false, |&last_id| last_id == prev_next_id));
        assert_eq!(queue.len() as u32, T::MaxSwapQueueLength::get());
        assert_eq!(SwapQueueCount::<T>::get(), T::MaxSwapQueueLength::get());
        assert_eq!(
            TotalQueuedSwapPrincipal::<T>::get(),
            queued_principal_before
                .checked_add(call_queue.call.get_queue_balance())
                .expect("benchmark queue principal fits u128")
        );
        assert_benchmark_queued_swap_principal::<T>();
    }

    #[benchmark]
    fn remove_delegate_account_balance() {
        let delegate_account: T::AccountId = funded_account::<T>("delegate_account", 0);
        let amount_to_remove = DEFAULT_DELEGATE_STAKE_TO_BE_ADDED;
        Network::<T>::increase_delegate_account_balance(&delegate_account, amount_to_remove);
        frame_system::Pallet::<T>::set_block_number(u32_to_block::<T>(1));
        let block = get_current_block_as_u32::<T>();
        let claim_block = block
            .checked_add(
                StakeCooldownEpochs::<T>::get()
                    .checked_mul(T::EpochLength::get())
                    .expect("benchmark cooldown fits u32"),
            )
            .expect("benchmark claim block fits u32");
        let seeded_ledger =
            prime_max_unbonding_ledger_for_merge::<T>(&delegate_account, claim_block);
        let seeded_entry = *seeded_ledger
            .get(&claim_block)
            .expect("target claim block is seeded");
        let total_network_unbonding_before = TotalNetworkUnbondingBalance::<T>::get();

        #[extrinsic_call]
        remove_delegate_account_balance(
            RawOrigin::Signed(delegate_account.clone()),
            amount_to_remove,
        );

        assert_eq!(DelegateAccountStake::<T>::get(&delegate_account), 0);
        let unbondings = StakeUnbondingLedger::<T>::get(delegate_account.clone());
        assert_eq!(unbondings.len() as u32, T::MaxUnbondingsUpperBound::get());
        assert_eq!(
            unbondings.get(&claim_block),
            Some(&UnbondingEntry {
                network: seeded_entry.network + amount_to_remove,
                overwatch: seeded_entry.overwatch,
            })
        );
        assert_eq!(
            TotalNetworkUnbondingBalance::<T>::get(),
            total_network_unbonding_before + amount_to_remove
        );
    }

    #[benchmark]
    fn propose_attestation() {
        MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        let end = max_subnet_nodes;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        ConsensusValidatorStakeWeightPower::<T>::insert(
            subnet_id,
            Network::<T>::percentage_factor_as_u128() / 2,
        );

        let epoch_length = T::EpochLength::get();
        let epoch = get_current_block_as_u32::<T>() / epoch_length as u32;

        set_block_to_subnet_slot_epoch::<T>(epoch, subnet_id);
        let subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);

        Network::<T>::elect_validator(subnet_id, subnet_epoch, get_current_block_as_u32::<T>());

        let subnet_node_id = SubnetElectedValidator::<T>::get(subnet_id, subnet_epoch as u32)
            .map(|round| round.validator_subnet_node_id);
        assert!(subnet_node_id != None, "Validator is None");

        let hotkey =
            Network::<T>::get_subnet_node_associated_hotkey(subnet_id, subnet_node_id.unwrap())
                .unwrap();

        // The regular proposal reads every submitted active node while filtering consensus data
        // and iterates the full values again to snapshot reward eligibility. Max-fill every live
        // record; the active-emergency branch is measured independently below.
        for node_id in 1..=end {
            seed_common_remove_subnet_node_state::<T>(subnet_id, node_id, true);
        }

        // Queue validation decodes the entire aggregate value. Put both requested mutations at
        // the tail of a maximum-size queue so a single pass visits every maximum-filled record.
        SubnetElectedValidator::<T>::mutate(subnet_id, subnet_epoch, |round| {
            round
                .as_mut()
                .expect("benchmark election must exist")
                .policy
                .queue_immunity_epochs = 0;
        });
        let queue_len = T::MaxRegisteredNodesUpperBound::get();
        let mut queue = Vec::with_capacity(queue_len as usize);
        for index in 0..queue_len {
            let mut queued_node = SubnetNodesData::<T>::get(subnet_id, index + 1);
            queued_node.id = 10_000 + index;
            queued_node.classification = SubnetNodeClassification {
                node_class: SubnetNodeClass::Registered,
                start_epoch: 0,
            };
            queue.push(queued_node);
        }
        let remove_queue_node_id = queue
            .get(queue.len().saturating_sub(2))
            .expect("benchmark queue has at least two nodes")
            .id;
        let prioritize_queue_node_id = queue
            .last()
            .expect("benchmark queue has at least one node")
            .id;
        SubnetNodeQueue::<T>::insert(subnet_id, queue);

        let subnet_node_data_vec =
            get_subnet_node_consensus_data::<T>(subnet_id, max_subnet_nodes, 0, end);

        for subnet_node_id in 1..=end {
            if let Some(validator_id) = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id) {
                ValidatorDelegateStakeBalance::<T>::insert(
                    validator_id,
                    DEFAULT_DELEGATE_STAKE_TO_BE_ADDED,
                );
            }
        }

        let max_args: ValidatorArgs<T> = vec![0xff; T::ValidatorArgsLimit::get() as usize]
            .try_into()
            .expect("validator args benchmark payload must fit its configured bound");

        #[extrinsic_call]
        propose_attestation(
            RawOrigin::Signed(hotkey.clone()),
            subnet_id,
            subnet_node_data_vec.clone(),
            Some(prioritize_queue_node_id),
            Some(remove_queue_node_id),
            Some(max_args.clone()),
            Some(max_args.clone()),
        );

        let submission =
            SubnetConsensusSubmission::<T>::get(subnet_id, subnet_epoch as u32).unwrap();
        let snapshot =
            SubnetConsensusAttestorWeights::<T>::get(subnet_id, subnet_epoch as u32).unwrap();

        assert_eq!(
            submission.validator_id,
            subnet_node_id.unwrap(),
            "Err: validator"
        );
        assert_eq!(
            submission.data.len(),
            subnet_node_data_vec.clone().len(),
            "Err: data len"
        );
        assert_eq!(submission.attests.len(), 1, "proposer must auto-attest");
        assert_eq!(
            submission.prioritize_queue_node_id,
            Some(prioritize_queue_node_id)
        );
        assert_eq!(submission.remove_queue_node_id, Some(remove_queue_node_id));
        assert_eq!(snapshot.weights.len() as u32, end, "Err: weights");
        assert_ne!(snapshot.total_weight, 0, "Err: total weight");
        assert_eq!(
            SubnetConsensusProposalArgs::<T>::get(subnet_id, subnet_epoch),
            Some(max_args.clone())
        );
        assert_eq!(
            SubnetConsensusAttestationData::<T>::get((
                subnet_id,
                subnet_epoch,
                subnet_node_id.unwrap(),
            )),
            Some(max_args)
        );
    }

    #[benchmark]
    fn propose_attestation_emergency() {
        MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
        let end = T::MaxSubnetNodesUpperBound::get();
        let emergency_nodes = T::MaxEmergencySubnetNodesUpperBound::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        ConsensusValidatorStakeWeightPower::<T>::insert(
            subnet_id,
            Network::<T>::percentage_factor_as_u128() / 2,
        );

        let epoch = get_current_block_as_u32::<T>() / T::EpochLength::get();
        set_block_to_subnet_slot_epoch::<T>(epoch, subnet_id);
        let subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);

        // A live emergency set may coexist with the complete active reward snapshot, but queue
        // mutations are forbidden on this path. Fill every active record and all 64 reachable
        // emergency candidates before electing from that set.
        for node_id in 1..=end {
            seed_common_remove_subnet_node_state::<T>(subnet_id, node_id, true);
        }
        EmergencySubnetNodeElectionData::<T>::insert(
            subnet_id,
            EmergencySubnetValidatorData {
                subnet_node_ids: (1..=emergency_nodes).rev().collect(),
                target_emergency_validators_epochs: u32::MAX,
                total_epochs: 0,
                max_emergency_validators_epoch: u32::MAX,
                activated: true,
                started_subnet_epoch: subnet_epoch,
                ..Default::default()
            },
        );
        Network::<T>::elect_validator(subnet_id, subnet_epoch, get_current_block_as_u32::<T>());
        let round = SubnetElectedValidator::<T>::get(subnet_id, subnet_epoch)
            .expect("emergency benchmark election must persist");
        assert!(round.emergency.is_some());
        let elected_node_id = round.validator_subnet_node_id;
        let hotkey = Network::<T>::get_subnet_node_associated_hotkey(subnet_id, elected_node_id)
            .expect("elected emergency node has a hotkey");

        let data = get_subnet_node_consensus_data::<T>(subnet_id, end, 0, end);
        for subnet_node_id in 1..=emergency_nodes {
            let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id)
                .expect("emergency candidate has a validator identity");
            ValidatorDelegateStakeBalance::<T>::insert(
                validator_id,
                DEFAULT_DELEGATE_STAKE_TO_BE_ADDED,
            );
        }
        let max_args: ValidatorArgs<T> = vec![0xff; T::ValidatorArgsLimit::get() as usize]
            .try_into()
            .expect("validator args benchmark payload fits its configured bound");

        #[extrinsic_call]
        propose_attestation(
            RawOrigin::Signed(hotkey),
            subnet_id,
            data.clone(),
            None,
            None,
            Some(max_args.clone()),
            Some(max_args.clone()),
        );

        let submission = SubnetConsensusSubmission::<T>::get(subnet_id, subnet_epoch)
            .expect("emergency proposal must persist");
        let snapshot = SubnetConsensusAttestorWeights::<T>::get(subnet_id, subnet_epoch)
            .expect("emergency attestor snapshot must persist");
        assert_eq!(submission.data.len() as u32, end);
        assert_eq!(submission.validator_ids.len() as u32, emergency_nodes);
        assert_eq!(snapshot.weights.len() as u32, emergency_nodes);
        assert!(submission.emergency.is_some());
        assert_eq!(submission.prioritize_queue_node_id, None);
        assert_eq!(submission.remove_queue_node_id, None);
        assert_eq!(
            SubnetConsensusProposalArgs::<T>::get(subnet_id, subnet_epoch),
            Some(max_args.clone())
        );
        assert_eq!(
            SubnetConsensusAttestationData::<T>::get((subnet_id, subnet_epoch, elected_node_id,)),
            Some(max_args)
        );
    }

    #[benchmark]
    fn attest() {
        MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        let end = max_subnet_nodes;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let epoch_length = T::EpochLength::get();
        let epoch = get_current_block_as_u32::<T>() / epoch_length as u32;

        set_block_to_subnet_slot_epoch::<T>(epoch, subnet_id);
        let subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);

        Network::<T>::elect_validator(subnet_id, subnet_epoch, get_current_block_as_u32::<T>());

        let subnet_node_id = SubnetElectedValidator::<T>::get(subnet_id, subnet_epoch as u32)
            .map(|round| round.validator_subnet_node_id);
        assert!(subnet_node_id != None, "Validator is None");

        let hotkey =
            Network::<T>::get_subnet_node_associated_hotkey(subnet_id, subnet_node_id.unwrap())
                .unwrap();

        let subnet_node_data_vec =
            get_subnet_node_consensus_data::<T>(subnet_id, max_subnet_nodes, 0, end);

        for subnet_node_id in 1..=end {
            if let Some(validator_id) = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id) {
                ValidatorDelegateStakeBalance::<T>::insert(
                    validator_id,
                    DEFAULT_DELEGATE_STAKE_TO_BE_ADDED,
                );
            }
        }

        assert_ok!(Network::<T>::propose_attestation(
            RawOrigin::Signed(hotkey.clone()).into(),
            subnet_id,
            subnet_node_data_vec.clone(),
            None,
            None,
            None,
            None,
        ));

        let attester_subnet_node_id = (1..=end)
            .find(|node_id| *node_id != subnet_node_id.unwrap())
            .unwrap();
        let attester =
            Network::<T>::get_subnet_node_associated_hotkey(subnet_id, attester_subnet_node_id)
                .unwrap();

        // `attest` rewrites the compact aggregate map. Fill every other validator entry so the
        // timed call inserts the 512th entry, while the arbitrary payload remains one independent
        // per-attestor storage write.
        SubnetConsensusSubmission::<T>::mutate(subnet_id, subnet_epoch, |submission| {
            let submission = submission.as_mut().expect("benchmark proposal must exist");
            submission.attests = (1..=end)
                .filter(|node_id| *node_id != attester_subnet_node_id)
                .map(|node_id| {
                    (
                        node_id,
                        AttestEntry::<T> {
                            block: get_current_block_as_u32::<T>(),
                            attestor_progress: 0,
                            reward_factor: Network::<T>::percentage_factor_as_u128(),
                            data: None,
                        },
                    )
                })
                .collect();
        });
        assert_eq!(
            SubnetConsensusSubmission::<T>::get(subnet_id, subnet_epoch)
                .unwrap()
                .attests
                .len(),
            (end - 1) as usize,
        );

        let max_attest_data: ValidatorArgs<T> = vec![0xff; T::ValidatorArgsLimit::get() as usize]
            .try_into()
            .expect("attestation benchmark payload must fit its configured bound");

        // Authority and classification checks decode the attester's full live node record.
        seed_common_remove_subnet_node_state::<T>(subnet_id, attester_subnet_node_id, true);

        let current_block_number = get_current_block_as_u32::<T>();

        #[extrinsic_call]
        attest(
            RawOrigin::Signed(attester.clone()),
            subnet_id,
            attester_subnet_node_id,
            Some(max_attest_data.clone()),
        );

        let submission =
            SubnetConsensusSubmission::<T>::get(subnet_id, subnet_epoch as u32).unwrap();

        assert_eq!(submission.attests.len(), end as usize);
        assert_eq!(
            submission
                .attests
                .get(&(attester_subnet_node_id))
                .unwrap()
                .block,
            current_block_number
        );
        assert_eq!(
            SubnetConsensusAttestationData::<T>::get((
                subnet_id,
                subnet_epoch,
                attester_subnet_node_id,
            )),
            Some(max_attest_data)
        );
    }

    #[benchmark]
    fn update_node_unique() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let subnet_node_id = end;
        let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id).unwrap();
        let coldkey = ValidatorColdkey::<T>::get(validator_id).unwrap();

        let unique: Vec<u8> = "a".into();
        let bounded_unique: NetworkBytes<T> = unique.try_into().expect("String too long");

        #[extrinsic_call]
        update_node_unique(
            RawOrigin::Signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            Some(bounded_unique.clone()),
        );

        assert_eq!(
            SubnetNodesData::<T>::get(subnet_id, subnet_node_id).unique,
            Some(bounded_unique.clone())
        )
    }

    #[benchmark]
    fn update_node_non_unique() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let subnet_node_id = end;
        let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id).unwrap();
        let coldkey = ValidatorColdkey::<T>::get(validator_id).unwrap();

        let non_unique: Vec<u8> = "a".into();
        let bounded_non_unique: NetworkBytes<T> = non_unique.try_into().expect("String too long");

        #[extrinsic_call]
        update_node_non_unique(
            RawOrigin::Signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            Some(bounded_non_unique.clone()),
        );

        assert_eq!(
            SubnetNodesData::<T>::get(subnet_id, subnet_node_id).non_unique,
            Some(bounded_non_unique.clone())
        )
    }

    // #[benchmark]
    // fn update_coldkey() {
    //     let max_subnet_nodes = MaxSubnetNodes::<T>::get();
    //     let end = 4;
    //     build_activated_subnet::<T>(
    //         DEFAULT_SUBNET_NAME.into(),
    //         0,
    //         end,
    //         DEFAULT_DEPOSIT_AMOUNT,
    //         DEFAULT_SUBNET_NODE_STAKE,
    //     );
    //     let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

    //     let min_nodes = MinSubnetNodes::<T>::get();
    //     let max_subnets = MaxSubnets::<T>::get();
    //     let max_subnet_nodes = MaxSubnetNodes::<T>::get();

    //     let coldkey = get_coldkey::<T>(subnet_id, max_subnet_nodes, end);
    //     let hotkey = get_hotkey::<T>(subnet_id, max_subnet_nodes, max_subnets, end);
    //     let new_coldkey: T::AccountId = get_account::<T>("new_coldkey", 0);

    //     #[extrinsic_call]
    //     update_coldkey(
    //         RawOrigin::Signed(coldkey.clone()),
    //         hotkey.clone(),
    //         new_coldkey.clone(),
    //     );

    // }

    // #[benchmark]
    // fn update_hotkey() {
    //     let max_subnet_nodes = MaxSubnetNodes::<T>::get();
    //     let end = 4;
    //     build_activated_subnet::<T>(
    //         DEFAULT_SUBNET_NAME.into(),
    //         0,
    //         end,
    //         DEFAULT_DEPOSIT_AMOUNT,
    //         DEFAULT_SUBNET_NODE_STAKE,
    //     );
    //     let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

    //     let min_nodes = MinSubnetNodes::<T>::get();
    //     let max_subnets = MaxSubnets::<T>::get();
    //     let max_subnet_nodes = MaxSubnetNodes::<T>::get();

    //     let coldkey = get_coldkey::<T>(subnet_id, max_subnet_nodes, end);
    //     let hotkey = get_hotkey::<T>(subnet_id, max_subnet_nodes, max_subnets, end);
    //     let new_hotkey: T::AccountId = get_account::<T>("new_coldkey", 0);

    //     #[extrinsic_call]
    //     update_hotkey(
    //         RawOrigin::Signed(coldkey.clone()),
    //         hotkey.clone(),
    //         new_hotkey.clone(),
    //     );

    // }

    #[benchmark]
    fn update_node_peer_info() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let subnet_node_id = end;
        let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id).unwrap();
        let coldkey = ValidatorColdkey::<T>::get(validator_id).unwrap();
        let new_peer_info = PeerInfo::<T> {
            peer_id: peer(100),
            multiaddr: get_multiaddr::<T>(Some(subnet_id), Some(100), None),
        };

        #[extrinsic_call]
        update_node_peer_info(
            RawOrigin::Signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            Some(new_peer_info.clone()),
        );

        assert_eq!(
            SubnetNodesData::<T>::get(subnet_id, subnet_node_id).peer_info,
            Some(new_peer_info.clone())
        )
    }

    #[benchmark]
    fn update_node_bootnode_peer_info() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let subnet_node_id = end;
        let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id).unwrap();
        let coldkey = ValidatorColdkey::<T>::get(validator_id).unwrap();
        let new_peer_info = PeerInfo::<T> {
            peer_id: peer(101),
            multiaddr: get_multiaddr::<T>(Some(subnet_id), Some(101), Some(1)),
        };

        #[extrinsic_call]
        update_node_bootnode_peer_info(
            RawOrigin::Signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            Some(new_peer_info.clone()),
        );

        assert_eq!(
            SubnetNodesData::<T>::get(subnet_id, subnet_node_id).bootnode_peer_info,
            Some(new_peer_info.clone())
        )
    }

    #[benchmark]
    fn update_node_client_peer_info() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let subnet_node_id = end;
        let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id).unwrap();
        let coldkey = ValidatorColdkey::<T>::get(validator_id).unwrap();
        let new_peer_info = PeerInfo::<T> {
            peer_id: peer(102),
            multiaddr: get_multiaddr::<T>(Some(subnet_id), Some(102), Some(2)),
        };

        #[extrinsic_call]
        update_node_client_peer_info(
            RawOrigin::Signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            Some(new_peer_info.clone()),
        );

        assert_eq!(
            SubnetNodesData::<T>::get(subnet_id, subnet_node_id).client_peer_info,
            Some(new_peer_info.clone())
        )
    }

    #[benchmark]
    fn register_overwatch_node() {
        let validator_id = 1;
        prepare_overwatch_validator::<T>(validator_id);

        let coldkey = ValidatorColdkey::<T>::get(validator_id).unwrap();
        fund_account::<T>(&coldkey, DEFAULT_SUBNET_NODE_STAKE + DEFAULT_DEPOSIT_AMOUNT);

        #[extrinsic_call]
        register_overwatch_node(
            RawOrigin::Signed(coldkey.clone()),
            DEFAULT_SUBNET_NODE_STAKE,
        );

        assert_eq!(
            OverwatchNodeStakeBalance::<T>::get(1),
            DEFAULT_SUBNET_NODE_STAKE
        );
        assert_eq!(OverwatchNodeValidatorId::<T>::get(1), Some(validator_id));
        assert_eq!(ValidatorOverwatchNodeId::<T>::get(validator_id), Some(1));
    }

    #[benchmark]
    fn update_overwatch_hotkey() {
        let (id, coldkey) = register_benchmark_overwatch_node::<T>(1, DEFAULT_SUBNET_NODE_STAKE);
        let old_hotkey = get_account::<T>("old_overwatch_hotkey", 0);
        let new_hotkey = get_account::<T>("updated_overwatch_hotkey", 0);
        OverwatchNodeIdHotkey::<T>::insert(id, old_hotkey);

        #[extrinsic_call]
        update_overwatch_hotkey(
            RawOrigin::Signed(coldkey.clone()),
            id,
            Some(new_hotkey.clone()),
        );

        assert_eq!(OverwatchNodeIdHotkey::<T>::get(id), Some(new_hotkey));
    }

    #[benchmark]
    fn remove_overwatch_node() {
        let (id, coldkey) = register_benchmark_overwatch_node::<T>(1, DEFAULT_SUBNET_NODE_STAKE);
        let validator_id = OverwatchNodeValidatorId::<T>::get(id).unwrap();
        OverwatchNodeIdHotkey::<T>::insert(id, get_account::<T>("overwatch_override", 0));
        max_fill_overwatch_node_index::<T>(id);
        let (active_epoch, pending_epoch, initial_revision) =
            seed_max_overwatch_removal_lifecycle::<T>(id, false);

        // Sanity check
        assert_ne!(OverwatchNodes::<T>::try_get(id), Err(()));

        #[extrinsic_call]
        remove_overwatch_node(RawOrigin::Signed(coldkey.clone()), id);

        assert_eq!(OverwatchNodes::<T>::try_get(id), Err(()));
        assert_eq!(OverwatchNodeValidatorId::<T>::get(id), Some(validator_id));
        assert_eq!(ValidatorOverwatchNodeId::<T>::get(validator_id), None);
        assert!(!OverwatchNodeIdHotkey::<T>::contains_key(id));
        assert!(!OverwatchValidatorWhitelist::<T>::contains_key(
            validator_id
        ));
        assert_max_overwatch_removal_lifecycle::<T>(
            id,
            active_epoch,
            pending_epoch,
            initial_revision,
            false,
        );
    }

    /// Compare the sole-pending-participant branch against the maximal pending-cohort branch.
    /// Public removal weights use the component-wise maximum of both generated measurements.
    #[benchmark(extra)]
    fn remove_overwatch_node_last_pending() {
        let (id, coldkey) = register_benchmark_overwatch_node::<T>(1, DEFAULT_SUBNET_NODE_STAKE);
        let validator_id = OverwatchNodeValidatorId::<T>::get(id).unwrap();
        OverwatchNodeIdHotkey::<T>::insert(id, get_account::<T>("overwatch_override", 0));
        max_fill_overwatch_node_index::<T>(id);
        let (active_epoch, pending_epoch, initial_revision) =
            seed_max_overwatch_removal_lifecycle::<T>(id, true);

        #[extrinsic_call]
        remove_overwatch_node(RawOrigin::Signed(coldkey.clone()), id);

        assert_eq!(OverwatchNodes::<T>::try_get(id), Err(()));
        assert_eq!(OverwatchNodeValidatorId::<T>::get(id), Some(validator_id));
        assert_eq!(ValidatorOverwatchNodeId::<T>::get(validator_id), None);
        assert!(!OverwatchNodeIdHotkey::<T>::contains_key(id));
        assert!(!OverwatchValidatorWhitelist::<T>::contains_key(
            validator_id
        ));
        assert_max_overwatch_removal_lifecycle::<T>(
            id,
            active_epoch,
            pending_epoch,
            initial_revision,
            true,
        );
    }

    #[benchmark]
    fn set_overwatch_node_peer_id() {
        let end = 4;
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let (id, coldkey) = register_benchmark_overwatch_node::<T>(1, DEFAULT_SUBNET_NODE_STAKE);
        max_fill_overwatch_node_index::<T>(id);

        // Force the measured call down the replacement branch while keeping the node index at
        // the maximum reachable number of physical subnets.
        let previous_peer_id = peer(899);
        let mut index = OverwatchNodeIndex::<T>::get(id);
        if !index.contains_key(&subnet_id) {
            let removed_subnet_id = *index
                .keys()
                .next_back()
                .expect("maximum Overwatch index is non-empty");
            let removed_peer_id = index
                .remove(&removed_subnet_id)
                .expect("selected Overwatch peer exists");
            PeerIdOverwatchNodeId::<T>::remove(removed_subnet_id, removed_peer_id);
        }
        index.insert(subnet_id, previous_peer_id.clone());
        OverwatchNodeIndex::<T>::insert(id, index);
        PeerIdOverwatchNodeId::<T>::insert(subnet_id, &previous_peer_id, id);

        let peer_id = peer(900);
        assert_ne!(previous_peer_id, peer_id);

        #[extrinsic_call]
        set_overwatch_node_peer_id(
            RawOrigin::Signed(coldkey.clone()),
            subnet_id,
            id,
            peer_id.clone(),
        );

        let exists = OverwatchNodeIndex::<T>::get(id)
            .get(&subnet_id)
            .map_or(false, |x_peer_id| *x_peer_id == peer_id.clone());
        assert!(exists);
        assert!(!PeerIdOverwatchNodeId::<T>::contains_key(
            subnet_id,
            previous_peer_id
        ));
    }

    #[benchmark]
    fn commit_overwatch_subnet_weights(x: Linear<1, { MAX_PHYSICAL_SUBNETS_BENCHMARK_DOMAIN }>) {
        let coldkey_n = 1;

        let end = 4;
        let mut subnet_ids = Vec::new();
        for s in 0..x {
            let path: Vec<u8> = format!("subnet-name-{s}").into();
            build_activated_subnet::<T>(
                path.clone(),
                0,
                end,
                DEFAULT_DEPOSIT_AMOUNT,
                DEFAULT_SUBNET_NODE_STAKE,
            );
            let subnet_id = SubnetName::<T>::get::<Vec<u8>>(path)
                .expect("benchmark subnet must be indexed by name");
            subnet_ids.push(subnet_id);
            increase_epochs::<T>(10000);
        }

        // Resolve the builder's name indexes before replacing each subnet with maximum-sized
        // metadata, so every measured per-item existence proof decodes the largest permitted
        // subnet value.
        prepare_overwatch_validator::<T>(coldkey_n);
        for subnet_id in subnet_ids.iter().copied() {
            max_fill_benchmark_subnet_data::<T>(subnet_id);
        }

        let coldkey = ValidatorColdkey::<T>::get(coldkey_n).unwrap();
        fund_account::<T>(&coldkey, DEFAULT_SUBNET_NODE_STAKE + DEFAULT_DEPOSIT_AMOUNT);

        let overwatch_epoch = Network::<T>::get_current_overwatch_epoch_as_u32();
        if overwatch_epoch == 0 {
            increase_epochs::<T>(OverwatchEpochLengthMultiplier::<T>::get() + 1);
        }
        assert_ok!(Network::<T>::register_overwatch_node(
            RawOrigin::Signed(coldkey.clone()).into(),
            DEFAULT_SUBNET_NODE_STAKE
        ));

        let id = TotalOverwatchNodeUids::<T>::get();
        let hotkey = Network::<T>::get_overwatch_node_associated_hotkey(id).unwrap();

        let weight: u128 = 123456;
        let salt: Vec<u8> = b"secret-salt".to_vec();
        let commit_hash = make_commit::<T>(weight, salt.clone());

        let mut commits: Vec<OverwatchCommit<T::Hash>> = Vec::new();
        for subnet_id in subnet_ids.iter().copied() {
            commits.push(OverwatchCommit {
                subnet_id,
                weight: commit_hash,
            });
        }

        let overwatch_epoch = Network::<T>::get_current_overwatch_epoch_as_u32();
        set_block_to_overwatch_commit_block::<T>(overwatch_epoch);

        // A validator can fill this bounded row over repeated calls. Seed the disjoint prefix so
        // x=1 measures decoding and rewriting a 16-entry row, while every x finishes at the
        // cumulative 17-subnet bound.
        let existing_count = T::MaxPhysicalSubnetsUpperBound::get().saturating_sub(x);
        let existing_commits: BoundedBTreeMap<u32, T::Hash, T::MaxPhysicalSubnetsUpperBound> = (0
            ..existing_count)
            .map(|offset| {
                let subnet_id = u32::MAX.saturating_sub(offset);
                assert!(!subnet_ids.contains(&subnet_id));
                (subnet_id, T::Hashing::hash_of(&(id, subnet_id)))
            })
            .collect::<BTreeMap<_, _>>()
            .try_into()
            .expect("cumulative commit prefix fits the runtime bound");
        OverwatchCommits::<T>::insert(overwatch_epoch, id, existing_commits);

        #[extrinsic_call]
        commit_overwatch_subnet_weights(RawOrigin::Signed(hotkey.clone()), id, commits);

        let stored = OverwatchCommits::<T>::get(overwatch_epoch, id);
        assert_eq!(stored.len() as u32, T::MaxPhysicalSubnetsUpperBound::get());
        for subnet_id in subnet_ids {
            assert_eq!(stored.get(&subnet_id), Some(&commit_hash));
        }
    }

    #[benchmark]
    fn reveal_overwatch_subnet_weights(x: Linear<1, { MAX_PHYSICAL_SUBNETS_BENCHMARK_DOMAIN }>) {
        // ENSURE EPOCH LENGTH IS BOAVE MAX LINEAR
        /// x: subnets
        // overwatch nodes
        let coldkey_n = 1;

        // Activate subnets
        let end = 4;
        for s in 0..x {
            let path: Vec<u8> = format!("subnet-name-{s}").into();
            build_activated_subnet::<T>(
                path,
                0,
                end,
                DEFAULT_DEPOSIT_AMOUNT,
                DEFAULT_SUBNET_NODE_STAKE,
            );
            increase_epochs::<T>(10000);
        }

        prepare_overwatch_validator::<T>(coldkey_n);

        let coldkey = ValidatorColdkey::<T>::get(coldkey_n).unwrap();
        fund_account::<T>(&coldkey, DEFAULT_SUBNET_NODE_STAKE + DEFAULT_DEPOSIT_AMOUNT);

        let overwatch_epoch = Network::<T>::get_current_overwatch_epoch_as_u32();
        if overwatch_epoch == 0 {
            increase_epochs::<T>(OverwatchEpochLengthMultiplier::<T>::get() + 1);
        }
        assert_ok!(Network::<T>::register_overwatch_node(
            RawOrigin::Signed(coldkey.clone()).into(),
            DEFAULT_SUBNET_NODE_STAKE
        ));

        let id = TotalOverwatchNodeUids::<T>::get();
        let hotkey = Network::<T>::get_overwatch_node_associated_hotkey(id).unwrap();

        // universal commits for testing
        let weight: u128 = 123456;
        let salt: OverwatchRevealSalt<T> =
            vec![0xff; T::MaxOverwatchRevealSaltLength::get() as usize]
                .try_into()
                .expect("salt uses the configured maximum length");
        let commit_hash = T::Hashing::hash_of(&(weight, salt.clone()));

        let mut commits: Vec<OverwatchCommit<T::Hash>> = Vec::new();
        let mut reveals: Vec<OverwatchReveal<T>> = Vec::new();
        let mut revealed_subnet_ids = BTreeSet::new();
        for s in 0..x {
            let path: Vec<u8> = format!("subnet-name-{s}").into();
            let subnet_id = SubnetName::<T>::get::<Vec<u8>>(path.clone()).unwrap();
            revealed_subnet_ids.insert(subnet_id);
            commits.push(OverwatchCommit {
                subnet_id,
                weight: commit_hash,
            });
            reveals.push(OverwatchReveal {
                subnet_id,
                weight,
                salt: salt.clone(),
            })
        }
        let measured_subnet_ids = revealed_subnet_ids.clone();

        let overwatch_epoch = Network::<T>::get_current_overwatch_epoch_as_u32();
        set_block_to_overwatch_commit_block::<T>(overwatch_epoch);

        assert_ok!(Network::<T>::commit_overwatch_subnet_weights(
            RawOrigin::Signed(hotkey.clone()).into(),
            id,
            commits
        ));

        // Every successful reveal mutates one bounded row and the compact per-subnet aggregate.
        // Seed the largest reachable pre-state while leaving this call's row entries absent.
        let max_revealing_nodes = T::MaxOverwatchNodesUpperBound::get() as usize;
        let max_revealed_subnets = T::MaxPhysicalSubnetsUpperBound::get() as usize;
        assert_eq!(id, 1);
        for expected_node_id in 2..=max_revealing_nodes as u32 {
            let validator_id = 10_000u32.saturating_add(expected_node_id);
            let inserted_node_id = insert_overwatch_node::<T>(
                validator_id,
                max_revealing_nodes as u32 + expected_node_id,
            );
            assert_eq!(inserted_node_id, expected_node_id);
            set_overwatch_stake::<T>(inserted_node_id, OverwatchMinStakeBalance::<T>::get());
        }
        assert_eq!(TotalOverwatchNodes::<T>::get(), max_revealing_nodes as u32);

        let mut candidate_subnet_id = 1u32;
        while revealed_subnet_ids.len() < max_revealed_subnets {
            revealed_subnet_ids.insert(candidate_subnet_id);
            candidate_subnet_id = candidate_subnet_id.saturating_add(1);
        }

        let max_records = (max_revealing_nodes as u32).saturating_mul(max_revealed_subnets as u32);
        let mut preexisting_records = 0u32;
        let mut subnet_revealer_counts = BTreeMap::<u32, u32>::new();
        for node_id in 1..=max_revealing_nodes as u32 {
            let mut reveal_row = BTreeMap::<u32, u128>::new();
            for subnet_id in revealed_subnet_ids.iter().copied() {
                if node_id == id && measured_subnet_ids.contains(&subnet_id) {
                    continue;
                }
                reveal_row.insert(subnet_id, weight);
                subnet_revealer_counts
                    .entry(subnet_id)
                    .and_modify(|count| *count = count.saturating_add(1))
                    .or_insert(1);
                preexisting_records = preexisting_records.saturating_add(1);
            }
            let reveal_row: BoundedBTreeMap<u32, u128, T::MaxPhysicalSubnetsUpperBound> =
                reveal_row
                    .try_into()
                    .expect("maximum benchmark reveal row fits its runtime bound");
            OverwatchReveals::<T>::insert(overwatch_epoch, node_id, reveal_row);
        }
        assert_eq!(preexisting_records, max_records.saturating_sub(x));
        // Every seeded node remains a canonical, whitelisted active member. Extra subnet keys may
        // be historical because subnet removal does not invalidate the bounded active reveal row.
        assert_eq!(
            TotalOverwatchNodeUids::<T>::get(),
            max_revealing_nodes as u32
        );
        TotalSubnetUids::<T>::set(max_revealed_subnets as u32);
        ActiveOverwatchRevealStats::<T>::put(OverwatchRevealStats::<T> {
            records: max_records.saturating_sub(x),
            subnet_revealer_counts: subnet_revealer_counts
                .try_into()
                .expect("subnet-count fixture fits the runtime bound"),
        });

        set_block_to_overwatch_reveal_block::<T>(overwatch_epoch);

        #[extrinsic_call]
        reveal_overwatch_subnet_weights(RawOrigin::Signed(hotkey.clone()), id, reveals);

        let revealed_row = OverwatchReveals::<T>::get(overwatch_epoch, id);
        for s in 0..x {
            let path: Vec<u8> = format!("subnet-name-{s}").into();
            let subnet_id = SubnetName::<T>::get::<Vec<u8>>(path.clone()).unwrap();
            assert_eq!(revealed_row.get(&subnet_id), Some(&weight));
        }
    }

    #[benchmark]
    fn add_overwatch_node_stake() {
        let (id, coldkey) = register_benchmark_overwatch_node::<T>(1, DEFAULT_SUBNET_NODE_STAKE);
        fund_account::<T>(&coldkey, DEFAULT_SUBNET_NODE_STAKE);
        let prev_balance = OverwatchNodeStakeBalance::<T>::get(id);

        #[extrinsic_call]
        add_overwatch_node_stake(
            RawOrigin::Signed(coldkey.clone()),
            id,
            DEFAULT_SUBNET_NODE_STAKE,
        );

        assert_eq!(
            prev_balance + DEFAULT_SUBNET_NODE_STAKE,
            OverwatchNodeStakeBalance::<T>::get(id)
        );
    }

    #[benchmark]
    fn remove_overwatch_node_stake() {
        let (id, coldkey) = register_benchmark_overwatch_node::<T>(1, DEFAULT_SUBNET_NODE_STAKE);
        fund_account::<T>(&coldkey, DEFAULT_SUBNET_NODE_STAKE);
        assert_ok!(Network::<T>::add_overwatch_node_stake(
            RawOrigin::Signed(coldkey.clone()).into(),
            id,
            DEFAULT_SUBNET_NODE_STAKE
        ));

        let prev_balance = OverwatchNodeStakeBalance::<T>::get(id);
        let block = get_current_block_as_u32::<T>();
        let claim_block = block
            .checked_add(
                StakeCooldownEpochs::<T>::get()
                    .checked_mul(T::EpochLength::get())
                    .expect("benchmark cooldown fits u32"),
            )
            .expect("benchmark claim block fits u32");
        let seeded_ledger = prime_max_unbonding_ledger_for_merge::<T>(&coldkey, claim_block);
        let total_overwatch_unbonding_before =
            seeded_ledger.values().fold(0u128, |total, entry| {
                total
                    .checked_add(entry.overwatch)
                    .expect("benchmark Overwatch unbonding principal fits u128")
            });
        let total_network_unbonding_before = TotalNetworkUnbondingBalance::<T>::get();

        #[extrinsic_call]
        remove_overwatch_node_stake(
            RawOrigin::Signed(coldkey.clone()),
            id,
            DEFAULT_SUBNET_NODE_STAKE,
        );

        assert_eq!(
            prev_balance - DEFAULT_SUBNET_NODE_STAKE,
            OverwatchNodeStakeBalance::<T>::get(id)
        );
        let unbondings = StakeUnbondingLedger::<T>::get(&coldkey);
        assert_eq!(unbondings.len() as u32, T::MaxUnbondingsUpperBound::get());
        let total_overwatch_unbonding_after = unbondings.values().fold(0u128, |total, entry| {
            total
                .checked_add(entry.overwatch)
                .expect("benchmark Overwatch unbonding principal fits u128")
        });
        assert_eq!(
            total_overwatch_unbonding_after,
            total_overwatch_unbonding_before + DEFAULT_SUBNET_NODE_STAKE,
        );
        assert_eq!(
            TotalNetworkUnbondingBalance::<T>::get(),
            total_network_unbonding_before
        );
    }

    #[benchmark]
    fn pause() {
        assert!(!TxPause::<T>::get());

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        pause(origin as T::RuntimeOrigin);

        // Verify the network is now paused
        assert!(TxPause::<T>::get());
        assert_eq!(
            OverwatchTxPauseStartBlock::<T>::get(),
            Some(get_current_block_as_u32::<T>())
        );
    }

    #[benchmark]
    fn unpause() {
        // sanity check
        assert!(!TxPause::<T>::get());

        assert_ok!(Network::<T>::do_pause());
        assert!(TxPause::<T>::get());

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        unpause(origin as T::RuntimeOrigin);

        assert!(!TxPause::<T>::get());
        assert!(OverwatchTxPauseStartBlock::<T>::get().is_none());
    }

    #[benchmark]
    fn collective_remove_subnet() {
        let max_subnet_nodes = MaxSubnetNodes::<T>::get();
        let max_registered_nodes = T::MaxRegisteredNodesUpperBound::get();
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            max_subnet_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        build_registered_subnet_nodes::<T>(
            subnet_id,
            max_subnet_nodes,
            max_subnet_nodes.saturating_add(max_registered_nodes),
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
            false,
        );
        seed_remove_subnet_cleanup_state::<T>(
            subnet_id,
            max_subnet_nodes,
            max_registered_nodes,
            T::MaxOverwatchNodesUpperBound::get(),
        );
        max_fill_remove_subnet_keyed_state::<T>(subnet_id, true);
        let current_epoch = Network::<T>::get_current_epoch_as_u32();
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
        SubnetsData::<T>::mutate(subnet_id, |maybe_subnet| {
            let subnet = maybe_subnet.as_mut().expect("benchmark subnet must exist");
            subnet.state = SubnetState::Paused;
            subnet.consensus_eligible_from_subnet_epoch = None;
            subnet.pause = Some(SubnetPauseData {
                started_global_epoch: current_epoch,
                started_subnet_epoch: current_subnet_epoch,
            });
        });

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        collective_remove_subnet(origin as T::RuntimeOrigin, subnet_id);

        assert_eq!(SubnetsData::<T>::try_get(subnet_id), Err(()));
    }

    #[benchmark]
    fn collective_remove_subnet_node() {
        // Benchmark the collective-origin/event wrapper independently. The dynamic branch model
        // is composed by the dispatch annotation for both active and registered nodes.
        let subnet_id = 1;
        let subnet_node_id = 1;
        assert!(!SubnetNodesData::<T>::contains_key(
            subnet_id,
            subnet_node_id
        ));
        assert!(!RegisteredSubnetNodesData::<T>::contains_key(
            subnet_id,
            subnet_node_id
        ));

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        collective_remove_subnet_node(origin as T::RuntimeOrigin, subnet_id, subnet_node_id);

        assert!(!SubnetNodesData::<T>::contains_key(
            subnet_id,
            subnet_node_id
        ));
        assert!(!RegisteredSubnetNodesData::<T>::contains_key(
            subnet_id,
            subnet_node_id
        ));
    }

    #[benchmark]
    fn collective_remove_overwatch_node() {
        let (id, _coldkey) = register_benchmark_overwatch_node::<T>(1, DEFAULT_SUBNET_NODE_STAKE);
        let validator_id = OverwatchNodeValidatorId::<T>::get(id).unwrap();
        OverwatchNodeIdHotkey::<T>::insert(id, get_account::<T>("overwatch_override", 0));
        max_fill_overwatch_node_index::<T>(id);
        let (active_epoch, pending_epoch, initial_revision) =
            seed_max_overwatch_removal_lifecycle::<T>(id, false);

        // Sanity check
        assert_ne!(OverwatchNodes::<T>::try_get(id), Err(()));

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        collective_remove_overwatch_node(origin as T::RuntimeOrigin, id);

        assert_eq!(OverwatchNodes::<T>::try_get(id), Err(()));
        assert!(!OverwatchNodeIdHotkey::<T>::contains_key(id));
        assert!(!OverwatchValidatorWhitelist::<T>::contains_key(
            validator_id
        ));
        assert_max_overwatch_removal_lifecycle::<T>(
            id,
            active_epoch,
            pending_epoch,
            initial_revision,
            false,
        );
    }

    /// Collective-origin comparison for the shared last-pending-participant removal branch.
    #[benchmark(extra)]
    fn collective_remove_overwatch_node_last_pending() {
        let (id, _coldkey) = register_benchmark_overwatch_node::<T>(1, DEFAULT_SUBNET_NODE_STAKE);
        let validator_id = OverwatchNodeValidatorId::<T>::get(id).unwrap();
        OverwatchNodeIdHotkey::<T>::insert(id, get_account::<T>("overwatch_override", 0));
        max_fill_overwatch_node_index::<T>(id);
        let (active_epoch, pending_epoch, initial_revision) =
            seed_max_overwatch_removal_lifecycle::<T>(id, true);

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        collective_remove_overwatch_node(origin as T::RuntimeOrigin, id);

        assert_eq!(OverwatchNodes::<T>::try_get(id), Err(()));
        assert!(!OverwatchNodeIdHotkey::<T>::contains_key(id));
        assert!(!OverwatchValidatorWhitelist::<T>::contains_key(
            validator_id
        ));
        assert_max_overwatch_removal_lifecycle::<T>(
            id,
            active_epoch,
            pending_epoch,
            initial_revision,
            true,
        );
    }

    #[benchmark]
    fn set_min_subnet_delegate_stake_factor() {
        let new_value = Network::<T>::percentage_factor_as_u128();

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_min_subnet_delegate_stake_factor(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MinSubnetDelegateStakeFactor::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_subnet_owner_percentage() {
        let value = SubnetOwnerPercentage::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_subnet_owner_percentage(origin as T::RuntimeOrigin, new_value);

        assert_eq!(SubnetOwnerPercentage::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_max_subnets() {
        let epoch_length = T::EpochLength::get();
        let designated_epoch_slots = T::DesignatedEpochSlots::get();
        // Keep one physical subnet slot free for the `MaxSubnets + 1` rotation subnet,
        // matching the validation performed by `do_set_max_subnets`.
        let new_value = epoch_length
            .saturating_sub(designated_epoch_slots)
            .min(T::MaxPhysicalSubnetsUpperBound::get())
            .saturating_sub(1);

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_subnets(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MaxSubnets::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_max_bootnodes() {
        let value = MaxBootnodes::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_bootnodes(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MaxBootnodes::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_max_subnet_bootnodes_access() {
        let value = MaxSubnetBootnodeAccess::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_subnet_bootnodes_access(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MaxSubnetBootnodeAccess::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_max_pause_epochs() {
        let value = MaxSubnetPauseEpochs::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_pause_epochs(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MaxSubnetPauseEpochs::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_delegate_stake_subnet_removal_interval() {
        let new_value = 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_delegate_stake_subnet_removal_interval(origin as T::RuntimeOrigin, new_value);

        assert_eq!(DelegateStakeSubnetRemovalInterval::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_subnet_removal_intervals() {
        let activation_cooldown_epochs = 10;
        let check_interval_epochs = 10;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_subnet_removal_intervals(
            origin as T::RuntimeOrigin,
            activation_cooldown_epochs,
            check_interval_epochs,
        );

        assert_eq!(
            SubnetRemovalActivationCooldown::<T>::get(),
            activation_cooldown_epochs
        );
        assert_eq!(
            SubnetRemovalCheckInterval::<T>::get(),
            check_interval_epochs
        );
    }

    #[benchmark]
    fn set_subnet_pause_cooldown_epochs() {
        let new_value = 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_subnet_pause_cooldown_epochs(origin as T::RuntimeOrigin, new_value);

        assert_eq!(SubnetPauseCooldownEpochs::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_min_registration_cost() {
        let value = MinRegistrationCost::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_min_registration_cost(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MinRegistrationCost::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_registration_cost_delay_blocks() {
        let value = RegistrationCostDecayBlocks::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_registration_cost_delay_blocks(origin as T::RuntimeOrigin, new_value);

        assert_eq!(RegistrationCostDecayBlocks::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_registration_cost_alpha() {
        let value = RegistrationCostAlpha::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_registration_cost_alpha(origin as T::RuntimeOrigin, new_value);

        assert_eq!(RegistrationCostAlpha::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_new_registration_cost_multiplier() {
        let value = NewRegistrationCostMultiplier::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_new_registration_cost_multiplier(origin as T::RuntimeOrigin, new_value);

        assert_eq!(NewRegistrationCostMultiplier::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_min_subnet_delegate_stake_balance() {
        let new_value = u128::MAX;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_min_subnet_delegate_stake_balance(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MinSubnetDelegateStakeBalance::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_churn_limits() {
        let min = 1;
        let max = 2;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_churn_limits(origin as T::RuntimeOrigin, min, max);

        assert_eq!(MinChurnLimit::<T>::get(), min);
        assert_eq!(MaxChurnLimit::<T>::get(), max);
    }

    #[benchmark]
    fn set_queue_epochs() {
        let min = 1;
        let max = 2;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_queue_epochs(origin as T::RuntimeOrigin, min, max);

        assert_eq!(MinQueueEpochs::<T>::get(), min);
        assert_eq!(MaxQueueEpochs::<T>::get(), max);
    }

    #[benchmark]
    fn set_max_swap_queue_calls_per_block() {
        let value = MaxSwapQueueCallsPerBlock::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_swap_queue_calls_per_block(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MaxSwapQueueCallsPerBlock::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_min_idle_classification_epochs() {
        let value = MinIdleClassificationEpochs::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_min_idle_classification_epochs(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MinIdleClassificationEpochs::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_max_idle_classification_epochs() {
        let value = MaxIdleClassificationEpochs::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_idle_classification_epochs(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MaxIdleClassificationEpochs::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_subnet_activation_enactment_epochs() {
        let value = SubnetEnactmentEpochs::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_subnet_activation_enactment_epochs(origin as T::RuntimeOrigin, new_value);

        assert_eq!(SubnetEnactmentEpochs::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_included_classification_epochs() {
        let min = 1;
        let max = 2;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_included_classification_epochs(origin as T::RuntimeOrigin, min, max);

        assert_eq!(MinIncludedClassificationEpochs::<T>::get(), min);
        assert_eq!(MaxIncludedClassificationEpochs::<T>::get(), max);
    }

    #[benchmark]
    fn set_subnet_stakes() {
        let min = 5;
        let max = 6;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_subnet_stakes(origin as T::RuntimeOrigin, min, max);

        assert_eq!(MinSubnetMinStake::<T>::get(), min);
        assert_eq!(MaxSubnetMinStake::<T>::get(), max);
    }

    #[benchmark]
    fn set_delegate_stake_percentages() {
        let min = 5;
        let max = 6;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_delegate_stake_percentages(origin as T::RuntimeOrigin, min, max);

        assert_eq!(MinDelegateStakePercentage::<T>::get(), min);
        assert_eq!(MaxDelegateStakePercentage::<T>::get(), max);
    }

    #[benchmark]
    fn set_min_max_registered_nodes() {
        let min = 5;
        let max = 6;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_min_max_registered_nodes(origin as T::RuntimeOrigin, min, max);

        assert_eq!(MinMaxRegisteredNodes::<T>::get(), min);
        assert_eq!(MaxMaxRegisteredNodes::<T>::get(), max);
    }

    #[benchmark]
    fn set_max_subnet_delegate_stake_rewards_percentage_change() {
        let value = MaxSubnetDelegateStakeRewardsPercentageChange::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_subnet_delegate_stake_rewards_percentage_change(
            origin as T::RuntimeOrigin,
            new_value,
        );

        assert_eq!(
            MaxSubnetDelegateStakeRewardsPercentageChange::<T>::get(),
            new_value
        );
    }

    #[benchmark]
    fn set_subnet_delegate_stake_rewards_update_period() {
        let value = SubnetDelegateStakeRewardsUpdatePeriod::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_subnet_delegate_stake_rewards_update_period(origin as T::RuntimeOrigin, new_value);

        assert_eq!(
            SubnetDelegateStakeRewardsUpdatePeriod::<T>::get(),
            new_value
        );
    }

    #[benchmark]
    fn set_base_validator_reward() {
        let value = BaseValidatorReward::<T>::get();
        let new_value = value - 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_base_validator_reward(origin as T::RuntimeOrigin, new_value);

        assert_eq!(BaseValidatorReward::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_base_slash_percentage() {
        let value = BaseSlashPercentage::<T>::get();
        let new_value = value - 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_base_slash_percentage(origin as T::RuntimeOrigin, new_value);

        assert_eq!(BaseSlashPercentage::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_max_slash_amount() {
        let value = MaxSlashAmount::<T>::get();
        let new_value = value - 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_slash_amount(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MaxSlashAmount::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_validator_delegate_stake_slash_config() {
        let threshold = Network::<T>::percentage_factor_as_u128() / 3;
        let base_percentage = Network::<T>::percentage_factor_as_u128() / 10;
        let max_amount = 1_000_000_000_000_000_000_u128;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_validator_delegate_stake_slash_config(
            origin as T::RuntimeOrigin,
            threshold,
            base_percentage,
            max_amount,
        );

        assert_eq!(ValidatorDelegateStakeSlashThreshold::<T>::get(), threshold);
        assert_eq!(
            BaseValidatorDelegateStakeSlashPercentage::<T>::get(),
            base_percentage
        );
        assert_eq!(MaxValidatorDelegateStakeSlashAmount::<T>::get(), max_amount);
    }

    #[benchmark]
    fn set_network_max_stake_balance() {
        let value = NetworkMaxStakeBalance::<T>::get();
        let new_value = value - 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_network_max_stake_balance(origin as T::RuntimeOrigin, new_value);

        assert_eq!(NetworkMaxStakeBalance::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_min_delegate_stake_deposit() {
        let value = MinDelegateStakeDeposit::<T>::get();
        let new_value = value + 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_min_delegate_stake_deposit(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MinDelegateStakeDeposit::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_node_reward_rate_update_period() {
        let value = NodeRewardRateUpdatePeriod::<T>::get();
        let new_value = value + 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_node_reward_rate_update_period(origin as T::RuntimeOrigin, new_value);

        assert_eq!(NodeRewardRateUpdatePeriod::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_max_reward_rate_decrease() {
        let value = MaxRewardRateDecrease::<T>::get();
        let new_value = value - 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_reward_rate_decrease(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MaxRewardRateDecrease::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_subnet_distribution_power() {
        let value = SubnetDistributionPower::<T>::get();
        let new_value = value - 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_subnet_distribution_power(origin as T::RuntimeOrigin, new_value);

        assert_eq!(SubnetDistributionPower::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_delegate_stake_weight_factor() {
        let value = DelegateStakeWeightFactor::<T>::get();
        let new_value = value - 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_delegate_stake_weight_factor(origin as T::RuntimeOrigin, new_value);

        assert_eq!(DelegateStakeWeightFactor::<T>::get(), new_value);
    }

    #[benchmark]
    fn owner_update_consensus_validator_node_count_decay() {
        let subnet_name = DEFAULT_SUBNET_NAME.as_bytes().to_vec();
        build_activated_subnet::<T>(
            subnet_name.clone(),
            0,
            MinSubnetNodes::<T>::get(),
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get(subnet_name).unwrap();
        let owner = subnet_owner::<T>(subnet_id);
        let current_value = ConsensusValidatorNodeCountDecay::<T>::get(subnet_id);
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
        let new_value = Network::<T>::percentage_factor_as_u128() / 2;

        #[extrinsic_call]
        owner_update_consensus_validator_node_count_decay(
            RawOrigin::Signed(owner.clone()),
            subnet_id,
            new_value,
        );

        assert_eq!(
            ConsensusValidatorNodeCountDecay::<T>::get(subnet_id),
            current_value
        );
        let pending = PendingConsensusValidatorNodeCountDecay::<T>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_value);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);
        assert_eq!(pending.owner, owner);
    }

    #[benchmark]
    fn owner_update_consensus_validator_stake_weight_power() {
        let subnet_name = DEFAULT_SUBNET_NAME.as_bytes().to_vec();
        build_activated_subnet::<T>(
            subnet_name.clone(),
            0,
            MinSubnetNodes::<T>::get(),
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get(subnet_name).unwrap();
        let owner = subnet_owner::<T>(subnet_id);
        let current_value = ConsensusValidatorStakeWeightPower::<T>::get(subnet_id);
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
        let new_value = Network::<T>::percentage_factor_as_u128() / 2;

        #[extrinsic_call]
        owner_update_consensus_validator_stake_weight_power(
            RawOrigin::Signed(owner.clone()),
            subnet_id,
            new_value,
        );

        assert_eq!(
            ConsensusValidatorStakeWeightPower::<T>::get(subnet_id),
            current_value
        );
        let pending = PendingConsensusValidatorStakeWeightPower::<T>::get(subnet_id).unwrap();
        assert_eq!(pending.value, new_value);
        assert_eq!(pending.effective_subnet_epoch, current_subnet_epoch + 1);
        assert_eq!(pending.owner, owner);
    }

    #[benchmark]
    fn set_consensus_validator_stake_weight_power_update_interval() {
        let value = ConsensusValidatorStakeWeightPowerUpdateInterval::<T>::get();
        let new_value = value.saturating_add(1);

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_consensus_validator_stake_weight_power_update_interval(
            origin as T::RuntimeOrigin,
            new_value,
        );

        assert_eq!(
            ConsensusValidatorStakeWeightPowerUpdateInterval::<T>::get(),
            new_value
        );
    }

    #[benchmark]
    fn set_min_max_consensus_validator_stake_weight_power() {
        let percentage_factor = Network::<T>::percentage_factor_as_u128();
        let min = percentage_factor / 4;
        let max = percentage_factor.saturating_mul(3) / 4;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_min_max_consensus_validator_stake_weight_power(origin as T::RuntimeOrigin, min, max);

        assert_eq!(MinConsensusValidatorStakeWeightPower::<T>::get(), min);
        assert_eq!(MaxConsensusValidatorStakeWeightPower::<T>::get(), max);
    }

    #[benchmark]
    fn set_emergency_validator_cooldown_epochs() {
        let new_value = EmergencyValidatorCooldownEpochs::<T>::get().saturating_add(1);
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_emergency_validator_cooldown_epochs(origin as T::RuntimeOrigin, new_value);

        assert_eq!(EmergencyValidatorCooldownEpochs::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_validator_node_delegate_stake_weights(
        x: Linear<1, { MAX_VALIDATOR_NODES_BENCHMARK_DOMAIN }>,
    ) {
        NewRegistrationCostMultiplier::<T>::set(Network::<T>::percentage_factor_as_u128());

        let name: Vec<u8> = b"validator-weight-subnet".to_vec();
        build_registered_subnet::<T>(
            name.clone(),
            0,
            MinSubnetNodes::<T>::get(),
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
            false,
        );
        let subnet_id = SubnetName::<T>::get(name).unwrap();

        let validator_id = 1;
        let coldkey = ValidatorColdkey::<T>::get(validator_id).unwrap();
        // Maximize the encoded ownership map as well as its total node cardinality. A validator
        // may own nodes across every subnet, so spreading x entries over the configured subnet
        // count exercises the outer BTreeMap work omitted by a single-key fixture.
        let ownership_subnets = T::MaxPhysicalSubnetsUpperBound::get().max(1).min(x);
        for offset in 0..ownership_subnets {
            let owned_subnet_id = subnet_id.saturating_add(offset);
            if !SubnetsData::<T>::contains_key(owned_subnet_id) {
                SubnetsData::<T>::insert(
                    owned_subnet_id,
                    new_subnet_data::<T>(owned_subnet_id, SubnetState::Active, 0),
                );
            }
        }
        let mut ownership = BTreeMap::<u32, BTreeSet<u32>>::new();
        for index in 0..x {
            let owned_subnet_id = subnet_id.saturating_add(index % ownership_subnets);
            ownership
                .entry(owned_subnet_id)
                .or_default()
                .insert(index.saturating_add(1));
        }
        ValidatorSubnetNodes::<T>::insert(validator_id, ownership);
        TotalValidatorNodes::<T>::insert(validator_id, x);
        let owned_nodes = Network::<T>::collect_validator_subnet_nodes(validator_id);
        assert_eq!(owned_nodes.len(), x as usize);
        ValidatorNodeDelegateStakeWeightUpdateInterval::<T>::set(1);
        increase_epochs::<T>(1);
        LastValidatorNodeDelegateStakeWeightUpdate::<T>::insert(
            validator_id,
            Network::<T>::get_current_epoch_as_u32().saturating_sub(1),
        );

        let percentage_factor = Network::<T>::percentage_factor_as_u128();
        let base_weight = percentage_factor / x as u128;
        let remainder = percentage_factor % x as u128;
        let updates: Vec<(u32, u32, u128)> = owned_nodes
            .iter()
            .copied()
            .enumerate()
            .map(|(index, (subnet_id, subnet_node_id))| {
                let weight = base_weight + u128::from((index as u128) < remainder);
                (subnet_id, subnet_node_id, weight)
            })
            .collect();
        // A successful prior update leaves an equally large, normalized map. Seed that reachable
        // pre-state so the overwrite does not start from an unrealistically empty value.
        ValidatorNodeDelegateStakeWeights::<T>::insert(
            validator_id,
            updates
                .iter()
                .map(|(subnet_id, node_id, weight)| ((*subnet_id, *node_id), *weight))
                .collect::<BTreeMap<_, _>>(),
        );
        assert_eq!(
            ValidatorNodeDelegateStakeWeights::<T>::get(validator_id).len(),
            x as usize
        );

        #[extrinsic_call]
        set_validator_node_delegate_stake_weights(RawOrigin::Signed(coldkey), updates);

        let stored = ValidatorNodeDelegateStakeWeights::<T>::get(validator_id);
        assert_eq!(stored.len(), x as usize);
        assert_eq!(stored.values().copied().sum::<u128>(), percentage_factor);
    }

    #[benchmark]
    fn set_validator_node_delegate_stake_weight_update_interval() {
        let new_value =
            ValidatorNodeDelegateStakeWeightUpdateInterval::<T>::get().saturating_add(1);
        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_validator_node_delegate_stake_weight_update_interval(
            origin as T::RuntimeOrigin,
            new_value,
        );

        assert_eq!(
            ValidatorNodeDelegateStakeWeightUpdateInterval::<T>::get(),
            new_value
        );
    }

    #[benchmark]
    fn set_consensus_validator_node_count_decay_update_interval() {
        let new_value =
            ConsensusValidatorNodeCountDecayUpdateInterval::<T>::get().saturating_add(1);
        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_consensus_validator_node_count_decay_update_interval(
            origin as T::RuntimeOrigin,
            new_value,
        );

        assert_eq!(
            ConsensusValidatorNodeCountDecayUpdateInterval::<T>::get(),
            new_value
        );
    }

    #[benchmark]
    fn set_subnet_net_flow_smoothing_alpha() {
        let new_value = Network::<T>::percentage_factor_as_u128() / 2;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_subnet_net_flow_smoothing_alpha(origin as T::RuntimeOrigin, new_value);

        assert_eq!(SubnetNetFlowSmoothingAlpha::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_consensus_validator_identity_attestation_percentage() {
        let new_value = Network::<T>::percentage_factor_as_u128() / 2;
        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_consensus_validator_identity_attestation_percentage(
            origin as T::RuntimeOrigin,
            new_value,
        );

        assert_eq!(
            ConsensusValidatorIdentityAttestationPercentage::<T>::get(),
            new_value
        );
    }

    #[benchmark]
    fn set_max_overwatch_nodes() {
        let value = MaxOverwatchNodes::<T>::get();
        let new_value = value - 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_overwatch_nodes(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MaxOverwatchNodes::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_overwatch_epoch_length_multiplier() {
        let value = OverwatchEpochLengthMultiplier::<T>::get();
        let new_value = value.saturating_add(1);

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_overwatch_epoch_length_multiplier(origin as T::RuntimeOrigin, new_value);

        assert_eq!(OverwatchEpochLengthMultiplier::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_overwatch_commit_cutoff_percent() {
        let value = OverwatchCommitCutoffPercent::<T>::get();
        let new_value = value - 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_overwatch_commit_cutoff_percent(origin as T::RuntimeOrigin, new_value);

        assert_eq!(OverwatchCommitCutoffPercent::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_overwatch_min_stake_balance() {
        let value = OverwatchMinStakeBalance::<T>::get();
        let new_value = value - 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_overwatch_min_stake_balance(origin as T::RuntimeOrigin, new_value);

        assert_eq!(OverwatchMinStakeBalance::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_min_max_subnet_node() {
        let min = 1;
        let max = 2;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_min_max_subnet_node(origin as T::RuntimeOrigin, min, max);

        assert_eq!(MinSubnetNodes::<T>::get(), min);
        assert_eq!(MaxSubnetNodes::<T>::get(), max);
    }

    #[benchmark]
    fn set_tx_rate_limit() {
        let value = TxRateLimit::<T>::get();
        let new_value = value + 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_tx_rate_limit(origin as T::RuntimeOrigin, new_value);

        assert_eq!(TxRateLimit::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_min_subnet_registration_epochs() {
        let value = MinSubnetRegistrationEpochs::<T>::get();
        let new_value = value + 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_min_subnet_registration_epochs(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MinSubnetRegistrationEpochs::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_subnet_registration_epochs() {
        let value = SubnetRegistrationEpochs::<T>::get();
        let new_value = value + 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_subnet_registration_epochs(origin as T::RuntimeOrigin, new_value);

        assert_eq!(SubnetRegistrationEpochs::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_min_active_node_stake_epochs() {
        let value = MinActiveNodeStakeEpochs::<T>::get();
        let new_value = value + 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_min_active_node_stake_epochs(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MinActiveNodeStakeEpochs::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_delegate_stake_cooldown_epochs() {
        let value = DelegateStakeCooldownEpochs::<T>::get();
        let new_value = value + 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_delegate_stake_cooldown_epochs(origin as T::RuntimeOrigin, new_value);

        assert_eq!(DelegateStakeCooldownEpochs::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_node_delegate_stake_cooldown_epochs() {
        let value = NodeDelegateStakeCooldownEpochs::<T>::get();
        let new_value = value + 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_node_delegate_stake_cooldown_epochs(origin as T::RuntimeOrigin, new_value);

        assert_eq!(NodeDelegateStakeCooldownEpochs::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_min_stake_cooldown_epochs() {
        let value = StakeCooldownEpochs::<T>::get();
        let new_value = value + 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_min_stake_cooldown_epochs(origin as T::RuntimeOrigin, new_value);

        assert_eq!(StakeCooldownEpochs::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_max_unbondings() {
        let value = MaxUnbondings::<T>::get();
        let new_value = value + 1;

        let origin = T::SuperMajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_unbondings(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MaxUnbondings::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_base_node_burn_amount() {
        let new_value = 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_base_node_burn_amount(origin as T::RuntimeOrigin, new_value);

        assert_eq!(BaseNodeBurnAmount::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_node_burn_rates() {
        let min = Network::<T>::percentage_factor_as_u128();
        let max = DefaultMaxNodeBurnRate::get();

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_node_burn_rates(origin as T::RuntimeOrigin, min, max);

        assert_eq!(MinNodeBurnRate::<T>::get(), min);
        assert_eq!(MaxNodeBurnRate::<T>::get(), max);
    }

    #[benchmark]
    fn set_max_subnet_node_min_weight_decrease_reputation_threshold() {
        let new_value = 1;

        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_subnet_node_min_weight_decrease_reputation_threshold(
            origin as T::RuntimeOrigin,
            new_value,
        );

        assert_eq!(
            MaxSubnetNodeMinWeightDecreaseReputationThreshold::<T>::get(),
            new_value
        );
    }

    #[benchmark]
    fn set_validator_reward_k() {
        let new_value = ValidatorRewardK::<T>::get() + 1;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_validator_reward_k(origin as T::RuntimeOrigin, new_value);

        assert_eq!(ValidatorRewardK::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_validator_reward_midpoint() {
        let new_value = 500_000_000_000_000_000u128;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_validator_reward_midpoint(origin as T::RuntimeOrigin, new_value);

        assert_eq!(ValidatorRewardMidpoint::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_attestor_reward_exponent() {
        let new_value = AttestorRewardExponent::<T>::get() + 1;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_attestor_reward_exponent(origin as T::RuntimeOrigin, new_value);

        assert_eq!(AttestorRewardExponent::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_attestor_min_reward_factor() {
        let new_value = 500_000_000_000_000_000u128;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_attestor_min_reward_factor(origin as T::RuntimeOrigin, new_value);

        assert_eq!(AttestorMinRewardFactor::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_min_max_node_reputation() {
        let min = 1;
        let max = 2;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_min_max_node_reputation(origin as T::RuntimeOrigin, min, max);

        assert_eq!(MinMinSubnetNodeReputation::<T>::get(), min);
        assert_eq!(MaxMinSubnetNodeReputation::<T>::get(), max);
    }

    #[benchmark]
    fn set_min_max_node_reputation_factor() {
        let min = 1;
        let max = 2;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_min_max_node_reputation_factor(origin as T::RuntimeOrigin, min, max);

        assert_eq!(MinNodeReputationFactor::<T>::get(), min);
        assert_eq!(MaxNodeReputationFactor::<T>::get(), max);
    }

    #[benchmark]
    fn set_min_subnet_reputation() {
        let new_value = 500_000_000_000_000_000u128;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_min_subnet_reputation(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MinSubnetReputation::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_not_in_consensus_subnet_reputation_factor() {
        let new_value = 500_000_000_000_000_000u128;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_not_in_consensus_subnet_reputation_factor(origin as T::RuntimeOrigin, new_value);

        assert_eq!(NotInConsensusSubnetReputationFactor::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_max_pause_epochs_subnet_reputation_factor() {
        let new_value = 500_000_000_000_000_000u128;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_pause_epochs_subnet_reputation_factor(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MaxPauseEpochsSubnetReputationFactor::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_less_than_min_nodes_subnet_reputation_factor() {
        let new_value = 500_000_000_000_000_000u128;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_less_than_min_nodes_subnet_reputation_factor(origin as T::RuntimeOrigin, new_value);

        assert_eq!(
            LessThanMinNodesSubnetReputationFactor::<T>::get(),
            new_value
        );
    }

    #[benchmark]
    fn set_validator_proposal_absent_subnet_reputation_factor() {
        let new_value = 500_000_000_000_000_000u128;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_validator_proposal_absent_subnet_reputation_factor(
            origin as T::RuntimeOrigin,
            new_value,
        );

        assert_eq!(ValidatorAbsentSubnetReputationFactor::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_in_consensus_subnet_reputation_factor() {
        let new_value = 500_000_000_000_000_000u128;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_in_consensus_subnet_reputation_factor(origin as T::RuntimeOrigin, new_value);

        assert_eq!(InConsensusSubnetReputationFactor::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_overwatch_weight_factor() {
        let new_value = 500_000_000_000_000_000u128;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_overwatch_weight_factor(origin as T::RuntimeOrigin, new_value);

        assert_eq!(OverwatchWeightFactor::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_max_emergency_validator_epochs_multiplier() {
        let new_value = 1_000_000_000_000_000_000u128;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_emergency_validator_epochs_multiplier(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MaxEmergencyValidatorEpochsMultiplier::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_max_emergency_subnet_nodes() {
        let new_value = MinSubnetNodes::<T>::get();
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_max_emergency_subnet_nodes(origin as T::RuntimeOrigin, new_value);

        assert_eq!(MaxEmergencySubnetNodes::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_overwatch_stake_weight_factor() {
        let new_value = 1_000_000_000_000_000_000u128;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_overwatch_stake_weight_factor(origin as T::RuntimeOrigin, new_value);

        assert_eq!(OverwatchStakeWeightFactor::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_subnet_weight_factors() {
        let new_value = SubnetWeightFactorsData {
            delegate_stake: 300_000_000_000_000_000u128,
            node_count: 300_000_000_000_000_000u128,
            net_flow: 300_000_000_000_000_000u128,
        };
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_subnet_weight_factors(origin as T::RuntimeOrigin, new_value.clone());

        assert_eq!(SubnetWeightFactors::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_churn_limit_multipliers() {
        let min = 1;
        let max = 2;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_churn_limit_multipliers(origin as T::RuntimeOrigin, min, max);

        assert_eq!(MinChurnLimitMultiplier::<T>::get(), min);
        assert_eq!(MaxChurnLimitMultiplier::<T>::get(), max);
    }

    #[benchmark]
    fn set_default_overwatch_subnet_weight() {
        let new_value = 500_000_000_000_000_000u128;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_default_overwatch_subnet_weight(origin as T::RuntimeOrigin, new_value);

        assert_eq!(DefaultOverwatchSubnetWeight::<T>::get(), new_value);
    }

    #[benchmark]
    fn set_overwatch_validator_whitelist() {
        let validator_id = 1;
        ensure_validator::<T>(validator_id);
        OverwatchValidatorWhitelist::<T>::insert(validator_id, ());
        let delegate_account = get_account::<T>("overwatch_whitelist_delegate", validator_id);
        ValidatorsData::<T>::mutate(validator_id, |validator| {
            validator.delegate_account = Some(DelegateAccount {
                account_id: delegate_account,
                rate: Network::<T>::percentage_factor_as_u128(),
            });
            validator.identity = Some(max_emission_identity::<T>());
        });
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        set_overwatch_validator_whitelist(origin as T::RuntimeOrigin, validator_id, false);

        assert!(!OverwatchValidatorWhitelist::<T>::contains_key(
            validator_id
        ));
    }

    #[benchmark]
    fn update_require_subnet_registration_whitelist() {
        let new_value = true;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        update_require_subnet_registration_whitelist(origin as T::RuntimeOrigin, new_value);

        assert_eq!(RequireSubnetRegistrationWhitelist::<T>::get(), new_value);
    }

    #[benchmark]
    fn update_subnet_registrant() {
        let coldkey = get_account::<T>("subnet_registrant", 0);
        let subnet_id = 1;
        let new_value = true;
        let origin = T::MajorityCollectiveOrigin::try_successful_origin()
            .expect("try_successful_origin failed");

        #[extrinsic_call]
        update_subnet_registrant(
            origin as T::RuntimeOrigin,
            coldkey.clone(),
            subnet_id,
            new_value,
        );

        assert_eq!(
            SubnetRegistrationWhitelist::<T>::get(coldkey, subnet_id),
            new_value
        );
    }

    #[benchmark]
    fn update_swap_queue() {
        let deposit_amount: u128 = 10000000000000000000000;
        let amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<T>::get();
        let end = MinSubnetNodes::<T>::get();

        let from_subnet_name: Vec<u8> = "subnet-name".into();
        build_activated_subnet::<T>(
            from_subnet_name.clone().into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let from_subnet_id = SubnetName::<T>::get(from_subnet_name.clone()).unwrap();

        let to_subnet_name: Vec<u8> = "subnet-name-2".into();
        build_activated_subnet::<T>(
            to_subnet_name.clone().into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let to_subnet_id = SubnetName::<T>::get(to_subnet_name.clone()).unwrap();

        let account = get_account::<T>("account", 0);

        let _ = T::Currency::deposit_creating(
            &account.clone(),
            (amount + 500).try_into().ok().expect("REASON"),
        );

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<T>::get(from_subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<T>::get(from_subnet_id);

        let mut delegate_stake_to_be_added_as_shares = Network::<T>::convert_to_shares(
            amount,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );

        if total_subnet_delegate_stake_shares == 0 {
            delegate_stake_to_be_added_as_shares = delegate_stake_to_be_added_as_shares
                .saturating_sub(Network::<T>::DELEGATE_POOL_MIN_LIQUIDITY);
        }

        frame_system::Pallet::<T>::set_block_number(u32_to_block::<T>(
            get_current_block_as_u32::<T>()
                + DelegateStakeCooldownEpochs::<T>::get() * T::EpochLength::get()
                + 1,
        ));

        let starting_delegator_balance = T::Currency::free_balance(&account.clone());

        assert_ok!(Network::<T>::add_subnet_delegate_stake(
            RawOrigin::Signed(account.clone()).into(),
            from_subnet_id,
            amount,
        ));

        let delegate_shares =
            AccountSubnetDelegateStakeShares::<T>::get(account.clone(), from_subnet_id);
        assert_eq!(delegate_shares, delegate_stake_to_be_added_as_shares);
        assert_ne!(delegate_shares, 0);

        let total_subnet_delegate_stake_shares =
            TotalSubnetDelegateStakeShares::<T>::get(from_subnet_id);
        let total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<T>::get(from_subnet_id);

        let mut from_delegate_balance = Network::<T>::convert_to_balance(
            delegate_shares,
            total_subnet_delegate_stake_shares,
            total_subnet_delegate_stake_balance,
        );
        // The first depositor will lose a percentage of their deposit depending on the size
        // https://docs.openzeppelin.com/contracts/4.x/erc4626#inflation-attack
        // assert_eq!(from_delegate_balance, delegate_stake_to_be_added_as_shares);

        let prev_total_subnet_delegate_stake_balance =
            TotalSubnetDelegateStakeBalance::<T>::get(from_subnet_id);
        let prev_next_id = NextSwapQueueId::<T>::get();

        assert_ok!(Network::<T>::swap_from_subnet_to_subnet(
            RawOrigin::Signed(account.clone()).into(),
            from_subnet_id,
            to_subnet_id,
            delegate_shares,
        ));
        let from_delegate_shares =
            AccountSubnetDelegateStakeShares::<T>::get(account.clone(), from_subnet_id);
        assert_eq!(from_delegate_shares, 0);

        assert_ne!(
            prev_total_subnet_delegate_stake_balance,
            TotalSubnetDelegateStakeBalance::<T>::get(from_subnet_id)
        );
        assert!(
            prev_total_subnet_delegate_stake_balance
                > TotalSubnetDelegateStakeBalance::<T>::get(from_subnet_id)
        );

        // Check the queue
        let starting_to_subnet_id = to_subnet_id;
        let call_queue = SwapCallQueue::<T>::get(prev_next_id);
        assert_eq!(call_queue.clone().unwrap().id, prev_next_id);
        let queued_principal = call_queue
            .as_ref()
            .expect("queued update benchmark item exists")
            .call
            .get_queue_balance();
        match &call_queue.clone().unwrap().call {
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id,
                to_subnet_id,
                balance,
            } => {
                assert_eq!(*account_id, account.clone());
                assert_eq!(*to_subnet_id, starting_to_subnet_id);
                assert_ne!(*balance, 0);
            }
            QueuedSwapCall::SwapToValidatorDelegateStake { .. } => assert!(false),
        };
        assert_eq!(TotalQueuedSwapPrincipal::<T>::get(), queued_principal);
        assert_benchmark_queued_swap_principal::<T>();

        let next_id = NextSwapQueueId::<T>::get();
        assert_eq!(prev_next_id + 1, next_id);
        let queue = SwapQueueOrder::<T>::get();
        assert!(queue
            .first()
            .map_or(false, |&first_id| first_id == prev_next_id));

        // UPDATE

        // Update back to the `from_subnet_id` staying as a `SwapToSubnetDelegateStake`
        let call = QueuedSwapCall::SwapToSubnetDelegateStake {
            account_id: account.clone(),
            to_subnet_id: from_subnet_id,
            balance: u128::MAX,
        };

        #[extrinsic_call]
        update_swap_queue(
            RawOrigin::Signed(account.clone()),
            prev_next_id,
            call.clone(),
        );

        let call_queue = SwapCallQueue::<T>::get(prev_next_id);
        assert_eq!(call_queue.clone().unwrap().id, prev_next_id);
        match &call_queue.clone().unwrap().call {
            QueuedSwapCall::SwapToSubnetDelegateStake {
                account_id,
                to_subnet_id,
                balance,
            } => {
                assert_eq!(*account_id, account.clone());
                assert_eq!(*to_subnet_id, from_subnet_id);
                assert_ne!(*balance, 0);
                assert_ne!(*balance, u128::MAX);
            }
            QueuedSwapCall::SwapToValidatorDelegateStake { .. } => assert!(false),
        };
        assert_eq!(TotalQueuedSwapPrincipal::<T>::get(), queued_principal);
        assert_benchmark_queued_swap_principal::<T>();
    }

    #[benchmark]
    fn elect_validator(
        x: Linear<{ MIN_CONSENSUS_VALIDATOR_IDENTITIES }, { MAX_SUBNET_NODES_BENCHMARK_DOMAIN }>,
    ) {
        // Regular election path with exactly `x` candidates and no emergency state.
        MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            x,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let active_nodes = TotalActiveSubnetNodes::<T>::get(subnet_id);
        assert_eq!(x, active_nodes);

        // Keep the regular list at exactly `x` and in reverse order so canonicalization measures
        // a non-trivial sort instead of receiving the builder's already-sorted vector.
        let regular_slot_list: Vec<u32> = (1..=x).rev().collect();
        SubnetNodeElectionSlots::<T>::insert(subnet_id, &regular_slot_list);

        let slot_list = SubnetNodeElectionSlots::<T>::get(subnet_id);
        assert_eq!(slot_list.len(), x as usize);

        let subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);

        BaseValidatorDelegateStakeSlashPercentage::<T>::set(
            Network::<T>::percentage_factor_as_u128() / 10,
        );
        MaxValidatorDelegateStakeSlashAmount::<T>::set(DEFAULT_DEPOSIT_AMOUNT);
        let election_block = get_current_block_as_u32::<T>();

        #[block]
        {
            // The hook reads this compact cardinality before selecting the generated branch.
            // Include its proof in the branch model; DbWeight alone has no proof-size component.
            let _ = TotalSubnetElectableNodes::<T>::get(subnet_id);
            Network::<T>::elect_validator(subnet_id, subnet_epoch, election_block);
        }

        let subnet_node_id = SubnetElectedValidator::<T>::get(subnet_id, subnet_epoch)
            .unwrap()
            .validator_subnet_node_id;
        let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id).unwrap();
        assert_eq!(
            ValidatorDelegateStakeSlashLockUntil::<T>::get(validator_id),
            election_block.saturating_add(T::EpochLength::get()),
        );
        assert!(SubnetElectedValidator::<T>::get(subnet_id, subnet_epoch)
            .unwrap()
            .emergency
            .is_none());
    }

    #[benchmark]
    fn elect_validator_emergency(
        e: Linear<
            { MIN_CONSENSUS_VALIDATOR_IDENTITIES },
            { MAX_EMERGENCY_SUBNET_NODES_BENCHMARK_DOMAIN },
        >,
    ) {
        MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            e,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);

        // Emergency resolution decodes every complete node record. Reverse the vector so the
        // canonicalization sort is non-trivial while preserving a reachable active-validator set.
        for subnet_node_id in 1..=e {
            seed_common_remove_subnet_node_state::<T>(subnet_id, subnet_node_id, true);
        }
        EmergencySubnetNodeElectionData::<T>::insert(
            subnet_id,
            EmergencySubnetValidatorData {
                subnet_node_ids: (1..=e).rev().collect(),
                target_emergency_validators_epochs: u32::MAX,
                total_epochs: 0,
                max_emergency_validators_epoch: u32::MAX,
                activated: true,
                started_subnet_epoch: subnet_epoch,
                ..Default::default()
            },
        );
        BaseValidatorDelegateStakeSlashPercentage::<T>::set(
            Network::<T>::percentage_factor_as_u128() / 10,
        );
        MaxValidatorDelegateStakeSlashAmount::<T>::set(DEFAULT_DEPOSIT_AMOUNT);
        let election_block = get_current_block_as_u32::<T>();

        #[block]
        {
            let _ = TotalSubnetElectableNodes::<T>::get(subnet_id);
            Network::<T>::elect_validator(subnet_id, subnet_epoch, election_block);
        }

        let round = SubnetElectedValidator::<T>::get(subnet_id, subnet_epoch).unwrap();
        assert!(round.emergency.is_some());
        assert_eq!(round.eligible_subnet_node_ids.len() as u32, e);
        assert!(EmergencySubnetNodeElectionData::<T>::contains_key(
            subnet_id
        ));
    }

    #[benchmark]
    fn elect_validator_expired(
        x: Linear<{ MIN_CONSENSUS_VALIDATOR_IDENTITIES }, { MAX_SUBNET_NODES_BENCHMARK_DOMAIN }>,
        e: Linear<
            { MIN_CONSENSUS_VALIDATOR_IDENTITIES },
            { MAX_EMERGENCY_SUBNET_NODES_BENCHMARK_DOMAIN },
        >,
    ) {
        MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            x,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
        SubnetNodeElectionSlots::<T>::insert(subnet_id, (1..=x).rev().collect::<Vec<_>>());
        TotalSubnetElectableNodes::<T>::insert(subnet_id, x);

        // Duration expiry is reachable with a still-full historical emergency snapshot and falls
        // back to the current regular set without an active-node scan. Snapshot IDs need not still
        // be live: emergency validators may have been removed after the snapshot was created.
        EmergencySubnetNodeElectionData::<T>::insert(
            subnet_id,
            EmergencySubnetValidatorData {
                subnet_node_ids: (1..=e).collect(),
                target_emergency_validators_epochs: 1,
                total_epochs: 1,
                max_emergency_validators_epoch: u32::MAX,
                activated: true,
                started_subnet_epoch: subnet_epoch.saturating_sub(1),
                ..Default::default()
            },
        );
        BaseValidatorDelegateStakeSlashPercentage::<T>::set(
            Network::<T>::percentage_factor_as_u128() / 10,
        );
        MaxValidatorDelegateStakeSlashAmount::<T>::set(DEFAULT_DEPOSIT_AMOUNT);
        let election_block = get_current_block_as_u32::<T>();

        #[block]
        {
            let _ = TotalSubnetElectableNodes::<T>::get(subnet_id);
            Network::<T>::elect_validator(subnet_id, subnet_epoch, election_block);
        }

        let round = SubnetElectedValidator::<T>::get(subnet_id, subnet_epoch).unwrap();
        assert!(round.emergency.is_none());
        assert_eq!(round.eligible_subnet_node_ids.len() as u32, x);
        assert!(!EmergencySubnetNodeElectionData::<T>::contains_key(
            subnet_id
        ));
        assert_eq!(
            LastEmergencyValidatorEndEpoch::<T>::get(subnet_id),
            Network::<T>::get_current_epoch_as_u32()
        );
    }

    #[benchmark]
    fn handle_increase_account_delegate_stake() {
        let account_id: T::AccountId = get_account::<T>("account", 0);
        let subnet_id = 1;
        let delegate_stake_to_be_added = 100e+18 as u128;

        // Sanity check
        assert_eq!(
            AccountSubnetDelegateStakeShares::<T>::get(&account_id, subnet_id),
            0
        );
        #[block]
        {
            Network::<T>::handle_increase_account_delegate_stake(
                &account_id,
                subnet_id,
                delegate_stake_to_be_added,
            )
            .expect("benchmark delegate stake credit must succeed");
        }

        assert_ne!(
            AccountSubnetDelegateStakeShares::<T>::get(&account_id, subnet_id),
            0
        );
    }

    #[benchmark]
    fn handle_increase_account_validator_delegate_stake() {
        let account_id: T::AccountId = get_account::<T>("account", 0);
        let validator_id = 1;
        let delegate_stake_to_be_added = 100e+18 as u128;
        ensure_validator::<T>(validator_id);

        // Sanity check
        assert_eq!(
            AccountValidatorDelegateStakeShares::<T>::get(&account_id, validator_id),
            0
        );
        #[block]
        {
            Network::<T>::handle_increase_account_validator_delegate_stake(
                &account_id,
                validator_id,
                delegate_stake_to_be_added,
            )
            .expect("benchmark validator delegate stake credit must succeed");
        }

        assert_ne!(
            AccountValidatorDelegateStakeShares::<T>::get(&account_id, validator_id),
            0
        );
    }

    #[benchmark]
    fn do_remove_subnet(
        a: Linear<1, { MAX_SUBNET_NODES_BENCHMARK_DOMAIN }>,
        r: Linear<1, { MAX_REGISTERED_NODES_BENCHMARK_DOMAIN }>,
        o: Linear<1, { MAX_OVERWATCH_NODES_BENCHMARK_DOMAIN }>,
    ) {
        // a/r independently cover active and registered node prefixes. o covers the target
        // PeerIdOverwatchNodeId prefix. Validator-wide and Overwatch-wide indexes are deliberately
        // repaired later by the affected owner and are not scanned by this removal path.
        MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
        let fixture_active_nodes = a.max(MinSubnetNodes::<T>::get());
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            fixture_active_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        for subnet_node_id in a.saturating_add(1)..=fixture_active_nodes {
            Network::<T>::remove_active_subnet_node(subnet_id, subnet_node_id);
        }
        build_registered_subnet_nodes::<T>(
            subnet_id,
            a,
            a.saturating_add(r),
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
            false,
        );
        seed_remove_subnet_cleanup_state::<T>(subnet_id, a, r, o);
        max_fill_remove_subnet_keyed_state::<T>(subnet_id, false);
        let pending_active: BoundedBTreeSet<u32, T::MaxSubnetNodesUpperBound> = (1..=a)
            .collect::<BTreeSet<_>>()
            .try_into()
            .expect("active removal markers fit the benchmark bound");
        PendingActiveNodeRemovals::<T>::insert(subnet_id, pending_active);
        let pending_registered: BoundedBTreeSet<u32, T::MaxRegisteredNodesUpperBound> =
            (a.saturating_add(1)..=a.saturating_add(r))
                .collect::<BTreeSet<_>>()
                .try_into()
                .expect("registered removal markers fit the benchmark bound");
        PendingRegisteredNodeRemovals::<T>::insert(subnet_id, pending_registered);
        let current_epoch = Network::<T>::get_current_epoch_as_u32();
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
        SubnetsData::<T>::mutate(subnet_id, |maybe_subnet| {
            let subnet = maybe_subnet.as_mut().expect("benchmark subnet must exist");
            subnet.state = SubnetState::Paused;
            subnet.consensus_eligible_from_subnet_epoch = None;
            subnet.pause = Some(SubnetPauseData {
                started_global_epoch: current_epoch,
                started_subnet_epoch: current_subnet_epoch,
            });
        });

        assert_eq!(TotalActiveSubnetNodes::<T>::get(subnet_id), a);
        assert_eq!(TotalSubnetNodes::<T>::get(subnet_id), a.saturating_add(r));
        assert_eq!(SubnetNodeQueue::<T>::get(subnet_id).len() as u32, r);
        assert_eq!(
            PendingActiveNodeRemovals::<T>::get(subnet_id).len() as u32,
            a
        );
        assert_eq!(
            PendingRegisteredNodeRemovals::<T>::get(subnet_id).len() as u32,
            r
        );

        #[block]
        {
            // `try_do_remove_subnet` selects o from this scalar before reserving the generated
            // cleanup weight. Include the selector-only key in this model so its proof is covered.
            let _ = TotalOverwatchNodes::<T>::get();
            let _ = Network::<T>::do_remove_subnet(subnet_id, SubnetRemovalReason::MinReputation);
        }

        assert_eq!(SubnetsData::<T>::try_get(subnet_id), Err(()));
        assert_eq!(SubnetNodesData::<T>::iter_prefix(subnet_id).count(), 0);
        assert_eq!(
            RegisteredSubnetNodesData::<T>::iter_prefix(subnet_id).count(),
            0
        );
        assert_eq!(
            SubnetNodeValidatorId::<T>::iter_prefix(subnet_id).count(),
            fixture_active_nodes.saturating_add(r) as usize
        );
        assert_eq!(SubnetNodeIdHotkey::<T>::iter_prefix(subnet_id).count(), 0);
        assert!(!PendingActiveNodeRemovals::<T>::contains_key(subnet_id));
        assert!(!PendingRegisteredNodeRemovals::<T>::contains_key(subnet_id));
        assert_eq!(
            UniqueParamSubnetNodeId::<T>::iter_prefix(subnet_id).count(),
            0
        );
        assert_eq!(
            PeerIdOverwatchNodeId::<T>::iter_prefix(subnet_id).count(),
            0
        );
        assert!(ValidatorSubnetNodes::<T>::iter()
            .any(|(_, node_map)| node_map.contains_key(&subnet_id)));
        assert_eq!(TotalNodes::<T>::get(), 0);
    }

    #[benchmark]
    fn clean_validator_subnet_nodes() {
        let n = MAX_VALIDATOR_NODES_BENCHMARK_DOMAIN;
        let validator_id = 1;
        ensure_validator::<T>(validator_id);

        let stale_subnet_id = 30_000;
        SubnetsData::<T>::remove(stale_subnet_id);

        // A validator can own nodes in every physical subnet. After one of those subnets is
        // removed, lazy cleanup scans that stale key plus all surviving live keys and then checks
        // every surviving node to recompute the validator's active-node count.
        let live_node_count = n.saturating_sub(1);
        let live_subnet_count = if live_node_count == 0 {
            0
        } else {
            T::MaxPhysicalSubnetsUpperBound::get()
                .saturating_sub(1)
                .max(1)
                .min(live_node_count)
        };
        let mut ownership = BTreeMap::<u32, BTreeSet<u32>>::new();
        for live_subnet_index in 0..live_subnet_count {
            let live_subnet_id = 20_000 + live_subnet_index;
            SubnetsData::<T>::insert(
                live_subnet_id,
                new_subnet_data::<T>(live_subnet_id, SubnetState::Active, 0),
            );
            max_fill_benchmark_subnet_data::<T>(live_subnet_id);
            ownership.insert(live_subnet_id, BTreeSet::new());
        }

        let mut allocations = BTreeMap::<(u32, u32), u128>::new();
        for index in 0..live_node_count {
            let live_subnet_id = 20_000 + (index % live_subnet_count.max(1));
            let subnet_node_id = index.saturating_add(1);
            ownership
                .get_mut(&live_subnet_id)
                .expect("live ownership subnet was seeded")
                .insert(subnet_node_id);
            allocations.insert((live_subnet_id, subnet_node_id), 1);

            SubnetNodesData::<T>::insert(
                live_subnet_id,
                subnet_node_id,
                SubnetNode::<T> {
                    id: subnet_node_id,
                    validator_id,
                    peer_info: None,
                    bootnode_peer_info: None,
                    client_peer_info: None,
                    classification: SubnetNodeClassification {
                        node_class: SubnetNodeClass::Validator,
                        start_epoch: 0,
                    },
                    unique: None,
                    non_unique: None,
                },
            );
            let _ = seed_common_remove_subnet_node_state::<T>(live_subnet_id, subnet_node_id, true);
        }

        let stale_node_id = 1_000_000;
        ownership.insert(stale_subnet_id, BTreeSet::from([stale_node_id]));
        ValidatorSubnetNodes::<T>::insert(validator_id, ownership);
        TotalValidatorNodes::<T>::insert(validator_id, n);

        allocations.insert((stale_subnet_id, stale_node_id), 1);
        ValidatorNodeDelegateStakeWeights::<T>::insert(validator_id, allocations);

        #[block]
        {
            Network::<T>::clean_validator_subnet_nodes(validator_id);
        }

        let expected_live_nodes = n.saturating_sub(1);
        assert_eq!(
            TotalValidatorNodes::<T>::get(validator_id),
            expected_live_nodes
        );
        assert!(!ValidatorSubnetNodes::<T>::get(validator_id).contains_key(&stale_subnet_id));
        assert_eq!(
            ValidatorNodeDelegateStakeWeights::<T>::get(validator_id).len() as u32,
            expected_live_nodes
        );
    }

    #[benchmark]
    fn do_remove_registered_subnet_initial_validator_cleanup() {
        let subnet_id = 1;
        SubnetsData::<T>::insert(
            subnet_id,
            new_subnet_data::<T>(subnet_id, SubnetState::Registered, 0),
        );
        max_fill_initial_validator_cleanup_state::<T>(subnet_id);

        assert_eq!(
            NodeRegistrationInitialValidatorIds::<T>::get(subnet_id)
                .expect("cleanup whitelist exists")
                .len() as u32,
            T::MaxRegisteredNodesUpperBound::get()
        );
        assert_eq!(
            InitialValidatorData::<T>::get(subnet_id)
                .expect("cleanup counters exist")
                .len() as u32,
            T::MaxRegisteredNodesUpperBound::get()
        );

        #[block]
        {
            NodeRegistrationInitialValidatorIds::<T>::remove(subnet_id);
            InitialValidatorData::<T>::remove(subnet_id);
        }

        assert!(!NodeRegistrationInitialValidatorIds::<T>::contains_key(
            subnet_id
        ));
        assert!(!InitialValidatorData::<T>::contains_key(subnet_id));
    }

    #[benchmark]
    fn do_remove_subnet_emergency_cleanup() {
        let subnet_id = 1;
        SubnetsData::<T>::insert(
            subnet_id,
            new_subnet_data::<T>(subnet_id, SubnetState::Paused, 0),
        );
        max_fill_emergency_subnet_election_data::<T>(subnet_id);
        assert_eq!(
            EmergencySubnetNodeElectionData::<T>::get(subnet_id)
                .expect("emergency cleanup value exists")
                .subnet_node_ids
                .len() as u32,
            T::MaxEmergencySubnetNodesUpperBound::get()
        );

        #[block]
        {
            EmergencySubnetNodeElectionData::<T>::remove(subnet_id);
        }

        assert!(!EmergencySubnetNodeElectionData::<T>::contains_key(
            subnet_id
        ));
    }

    #[benchmark]
    fn add_balance_to_treasury() {
        let amount = 100e+18 as u128;
        let amount_as_balance = Network::<T>::u128_to_balance(amount).unwrap();
        let treasury_account = T::TreasuryAccount::get();
        let starting_pot =
            Network::<T>::balance_to_u128(T::Currency::free_balance(&treasury_account)).unwrap();
        #[block]
        {
            let _ = Network::<T>::add_balance_to_treasury(amount_as_balance);
        }

        let pot =
            Network::<T>::balance_to_u128(T::Currency::free_balance(&treasury_account)).unwrap();
        assert_eq!(starting_pot.saturating_add(amount), pot);
    }

    #[benchmark]
    fn subnet_node_validator_id_selector() {
        let subnet_id = u32::MAX;
        let subnet_node_id = u32::MAX;
        SubnetNodeValidatorId::<T>::insert(subnet_id, subnet_node_id, u32::MAX);

        #[block]
        {
            let _ = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id);
        }
    }

    #[benchmark]
    fn remove_active_subnet_node_small(
        n: Linear<1, { MAX_VALIDATOR_NODES_BENCHMARK_DOMAIN }>,
        e: Linear<{ MIN_CONSENSUS_VALIDATOR_IDENTITIES }, { ACTIVE_REMOVAL_ELECTION_MODEL_SPLIT }>,
    ) {
        // Reachable small-election envelope: every emergency validator is an existing electable
        // Validator node, so the maximum emergency cardinality is exactly e.
        let (subnet_id, subnet_node_id, validator_id, unique) =
            seed_active_remove_subnet_node_state::<T>(n, e, e);

        #[block]
        {
            Network::<T>::remove_active_subnet_node(subnet_id, subnet_node_id);
        }

        assert_eq!(
            SubnetNodesData::<T>::try_get(subnet_id, subnet_node_id),
            Err(())
        );
        assert_eq!(
            UniqueParamSubnetNodeId::<T>::try_get(subnet_id, unique),
            Err(())
        );
        assert_eq!(
            SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id),
            Some(validator_id)
        );
        assert_eq!(TotalValidatorNodes::<T>::get(validator_id), n - 1);
        assert_eq!(
            ValidatorNodeDelegateStakeWeights::<T>::get(validator_id).len() as u32,
            n - 1
        );
        assert_eq!(
            NodeSlotIndex::<T>::try_get(subnet_id, subnet_node_id),
            Err(())
        );
        assert_eq!(NodeSlotIndex::<T>::get(subnet_id, e), Some(e - 2));
        assert!(!EmergencySubnetNodeElectionData::<T>::get(subnet_id)
            .unwrap()
            .subnet_node_ids
            .contains(&subnet_node_id));
    }

    #[benchmark]
    fn remove_active_subnet_node_large(
        n: Linear<1, { MAX_VALIDATOR_NODES_BENCHMARK_DOMAIN }>,
        e: Linear<{ ACTIVE_REMOVAL_ELECTION_MODEL_SPLIT }, { MAX_SUBNET_NODES_BENCHMARK_DOMAIN }>,
    ) {
        // Reachable large-election envelope: emergency membership is capped at 64 while the
        // election vector continues to grow to the 512-node protocol bound.
        let (subnet_id, subnet_node_id, validator_id, unique) =
            seed_active_remove_subnet_node_state::<T>(
                n,
                e,
                MAX_EMERGENCY_SUBNET_NODES_BENCHMARK_DOMAIN,
            );

        #[block]
        {
            Network::<T>::remove_active_subnet_node(subnet_id, subnet_node_id);
        }

        assert_eq!(
            SubnetNodesData::<T>::try_get(subnet_id, subnet_node_id),
            Err(())
        );
        assert_eq!(
            UniqueParamSubnetNodeId::<T>::try_get(subnet_id, unique),
            Err(())
        );
        assert_eq!(
            SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id),
            Some(validator_id)
        );
        assert_eq!(TotalValidatorNodes::<T>::get(validator_id), n - 1);
        assert_eq!(
            ValidatorNodeDelegateStakeWeights::<T>::get(validator_id).len() as u32,
            n - 1
        );
        assert_eq!(
            NodeSlotIndex::<T>::try_get(subnet_id, subnet_node_id),
            Err(())
        );
        assert_eq!(NodeSlotIndex::<T>::get(subnet_id, e), Some(e - 2));
        assert!(!EmergencySubnetNodeElectionData::<T>::get(subnet_id)
            .unwrap()
            .subnet_node_ids
            .contains(&subnet_node_id));
    }

    #[benchmark]
    fn remove_active_subnet_node_dispatch_small(
        n: Linear<1, { MAX_VALIDATOR_NODES_BENCHMARK_DOMAIN }>,
        e: Linear<{ MIN_CONSENSUS_VALIDATOR_IDENTITIES }, { ACTIVE_REMOVAL_ELECTION_MODEL_SPLIT }>,
    ) {
        let (subnet_id, subnet_node_id, validator_id, _) =
            seed_active_remove_subnet_node_state::<T>(n, e, e);

        #[block]
        {
            let selected_validator_id =
                SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id).unwrap();
            let _ = TotalValidatorNodes::<T>::get(selected_validator_id);
            let _ = SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id);
            let _ = TotalSubnetElectableNodes::<T>::get(subnet_id);
            Network::<T>::perform_remove_subnet_node(subnet_id, subnet_node_id);
        }

        assert!(!SubnetNodesData::<T>::contains_key(
            subnet_id,
            subnet_node_id
        ));
        assert_eq!(
            SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id),
            Some(validator_id)
        );
    }

    #[benchmark]
    fn remove_active_subnet_node_dispatch_large(
        n: Linear<1, { MAX_VALIDATOR_NODES_BENCHMARK_DOMAIN }>,
        e: Linear<{ ACTIVE_REMOVAL_ELECTION_MODEL_SPLIT }, { MAX_SUBNET_NODES_BENCHMARK_DOMAIN }>,
    ) {
        let (subnet_id, subnet_node_id, validator_id, _) = seed_active_remove_subnet_node_state::<T>(
            n,
            e,
            MAX_EMERGENCY_SUBNET_NODES_BENCHMARK_DOMAIN,
        );

        #[block]
        {
            let selected_validator_id =
                SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id).unwrap();
            let _ = TotalValidatorNodes::<T>::get(selected_validator_id);
            let _ = SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id);
            let _ = TotalSubnetElectableNodes::<T>::get(subnet_id);
            Network::<T>::perform_remove_subnet_node(subnet_id, subnet_node_id);
        }

        assert!(!SubnetNodesData::<T>::contains_key(
            subnet_id,
            subnet_node_id
        ));
        assert_eq!(
            SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id),
            Some(validator_id)
        );
    }

    #[benchmark]
    fn remove_registered_subnet_node(
        n: Linear<1, { MAX_VALIDATOR_NODES_BENCHMARK_DOMAIN }>,
        r: Linear<1, { MAX_REGISTERED_NODES_BENCHMARK_DOMAIN }>,
    ) {
        // n = cumulative validator ownership and full delegate-weight map size
        // r = number of registered nodes in the subnet (for SubnetNodeQueue retain operation)
        let (subnet_id, remove_subnet_node_id, validator_id, unique) =
            seed_registered_remove_subnet_node_state::<T>(n, r);

        #[block]
        {
            Network::<T>::remove_registered_subnet_node(subnet_id, remove_subnet_node_id);
        }

        // Verify node was removed from SubnetNodeQueue
        let queue_after = SubnetNodeQueue::<T>::get(subnet_id);
        assert!(!queue_after
            .iter()
            .any(|node| node.id == remove_subnet_node_id));
        let validator_subnet_nodes = ValidatorSubnetNodes::<T>::get(validator_id);
        assert!(!validator_subnet_nodes
            .get(&subnet_id)
            .map(|nodes| nodes.contains(&remove_subnet_node_id))
            .unwrap_or(false));
        assert_eq!(
            UniqueParamSubnetNodeId::<T>::try_get(subnet_id, unique),
            Err(())
        );
        assert_eq!(
            SubnetNodeValidatorId::<T>::get(subnet_id, remove_subnet_node_id),
            Some(validator_id)
        );
        assert_eq!(TotalValidatorNodes::<T>::get(validator_id), n - 1);
        assert_eq!(
            ValidatorNodeDelegateStakeWeights::<T>::get(validator_id).len() as u32,
            n - 1
        );
    }

    #[benchmark]
    fn remove_registered_subnet_node_dispatch(
        n: Linear<1, { MAX_VALIDATOR_NODES_BENCHMARK_DOMAIN }>,
        r: Linear<1, { MAX_REGISTERED_NODES_BENCHMARK_DOMAIN }>,
    ) {
        let (subnet_id, subnet_node_id, validator_id, _) =
            seed_registered_remove_subnet_node_state::<T>(n, r);

        #[block]
        {
            let selected_validator_id =
                SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id).unwrap();
            let _ = TotalValidatorNodes::<T>::get(selected_validator_id);
            let _ = SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id);
            let _ = TotalActiveSubnetNodes::<T>::get(subnet_id);
            let _ = TotalSubnetNodes::<T>::get(subnet_id);
            Network::<T>::perform_remove_subnet_node(subnet_id, subnet_node_id);
        }

        assert!(!RegisteredSubnetNodesData::<T>::contains_key(
            subnet_id,
            subnet_node_id
        ));
        assert_eq!(
            SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id),
            Some(validator_id)
        );
    }

    // #[benchmark]
    // fn slash_validator() {
    //     let max_subnets = MaxSubnets::<T>::get();
    //     let max_subnet_nodes = MaxSubnetNodes::<T>::get();
    //     let min_subnet_nodes = MinSubnetNodes::<T>::get();
    //     let end = min_subnet_nodes;

    //     build_activated_subnet::<T>(
    //         DEFAULT_SUBNET_NAME.into(),
    //         0,
    //         end,
    //         DEFAULT_DEPOSIT_AMOUNT,
    //         DEFAULT_SUBNET_NODE_STAKE,
    //     );
    //     let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

    //     let hotkey = get_hotkey::<T>(subnet_id, max_subnet_nodes, max_subnets, end - 1);
    //     let subnet_node_id = end - 1;

    //     #[block]
    //     {
    //         Network::<T>::slash_validator(
    //             subnet_id,
    //             subnet_node_id,
    //             1, // attestation percentage
    //             T::MinAttestationPercentage::get(),
    //             0,
    //             0,
    //             Network::<T>::get_current_epoch_as_u32(),
    //         );
    //     }
    // }

    #[benchmark]
    fn add_balance_to_coldkey_account() {
        let coldkey = get_account::<T>("coldkey", 0);
        let amount = 100e+18 as u128;
        let amount_as_balance = Network::<T>::u128_to_balance(amount).unwrap();

        // Sanity
        let balance = T::Currency::free_balance(&coldkey.clone());
        assert_eq!(balance, Network::<T>::u128_to_balance(0).unwrap());

        #[block]
        {
            Network::<T>::add_balance_to_coldkey_account(&coldkey.clone(), amount_as_balance);
        }

        let balance = T::Currency::free_balance(&coldkey.clone());
        assert_eq!(balance, amount_as_balance);
    }

    #[benchmark]
    fn graduate_class() {
        let min_subnet_nodes = MinSubnetNodes::<T>::get();
        let end = min_subnet_nodes;

        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            end,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        let (hotkey_subnet_node_id, _coldkey, _hotkey, _peer_info) =
            register_benchmark_subnet_node::<T>(
                subnet_id,
                end + 1,
                end + 1,
                DEFAULT_STAKE_TO_BE_ADDED,
                None,
            );

        let subnet_node = RegisteredSubnetNodesData::<T>::get(subnet_id, hotkey_subnet_node_id);
        let validator_id = subnet_node.validator_id;
        let mut weight_meter = WeightMeter::new();
        Network::<T>::do_activate_subnet_node(
            &mut weight_meter,
            subnet_id,
            SubnetState::Active,
            subnet_node,
            Network::<T>::get_current_subnet_epoch_as_u32(subnet_id),
            true,
        );

        let subnet_node = SubnetNodesData::<T>::get(subnet_id, hotkey_subnet_node_id);
        let old_node_class = subnet_node.classification.node_class;

        #[block]
        {
            Network::<T>::graduate_class(
                subnet_id,
                hotkey_subnet_node_id,
                Network::<T>::get_current_epoch_as_u32(),
            );
        }

        let subnet_node = SubnetNodesData::<T>::get(subnet_id, hotkey_subnet_node_id);
        let node_class = subnet_node.classification.node_class;
        assert!(node_class > old_node_class);
    }

    #[benchmark]
    fn insert_node_into_election_slot() {
        let subnet_id = 1u32;
        let subnet_node_id = 42u32;

        #[block]
        {
            Network::<T>::insert_node_into_election_slot(subnet_id, subnet_node_id);
        }
    }
    #[benchmark]
    fn get_min_subnet_delegate_stake_balance() {
        let live_subnet_count = T::MaxPhysicalSubnetsUpperBound::get();
        assert!(live_subnet_count > 0);
        MinSubnetDelegateStakeBalance::<T>::put(1);
        MinSubnetDelegateStakeFactor::<T>::put(DefaultMinSubnetDelegateStakeFactor::get());

        let subnet_ids = seed_live_subnet_delegate_stake_cohort::<T>(live_subnet_count, |index| {
            DEFAULT_DELEGATE_STAKE_TO_BE_ADDED
                .checked_mul(index.saturating_add(1) as u128)
                .expect("benchmark delegation fits u128")
        });
        let subnet_id = subnet_ids[0];
        // The even token-denominated base makes this arithmetic-series average exact.
        let total_delegation = DEFAULT_DELEGATE_STAKE_TO_BE_ADDED
            .checked_mul(live_subnet_count as u128)
            .and_then(|value| value.checked_mul(live_subnet_count.saturating_add(1) as u128))
            .and_then(|value| value.checked_div(2))
            .expect("benchmark delegation sum fits u128");
        let expected_minimum = Network::<T>::percent_mul(
            total_delegation / live_subnet_count as u128,
            DefaultMinSubnetDelegateStakeFactor::get(),
        )
        .max(1);

        let result;

        #[block]
        {
            result = Network::<T>::get_min_subnet_delegate_stake_balance(subnet_id);
        }

        assert_eq!(result, expected_minimum);
    }

    /// Fixed hook selectors read on every block before any early return or branch admission.
    #[benchmark]
    fn on_initialize_base() {
        TxPause::<T>::put(false);
        PendingOverwatchSettlement::<T>::put(PendingOverwatchSettlementData {
            epoch: u32::MAX,
            reveal_records: 1_088,
        });
        OverwatchEpochStartBlock::<T>::put(u32::MAX);
        ActiveOverwatchEpochLengthMultiplier::<T>::put(u32::MAX);

        #[block]
        {
            let _ = TxPause::<T>::get();
            let _ = PendingOverwatchSettlement::<T>::get();
            let _ = OverwatchEpochStartBlock::<T>::get();
            let _ = ActiveOverwatchEpochLengthMultiplier::<T>::get();
        }
    }

    /// Compact subnet-count selector shared by the epoch-preliminary and emission-weight slots.
    #[benchmark]
    fn total_subnets_selector() {
        TotalSubnets::<T>::put(T::MaxPhysicalSubnetsUpperBound::get());

        #[block]
        {
            let _ = TotalSubnets::<T>::get();
        }
    }

    #[benchmark]
    fn advance_overwatch_epoch() {
        let multiplier = OverwatchEpochLengthMultiplier::<T>::get();
        let rollover_block = T::EpochLength::get().saturating_mul(multiplier);
        let max_revealing_nodes = T::MaxOverwatchNodesUpperBound::get();
        assert_eq!(max_revealing_nodes, MAX_OVERWATCH_NODES_BENCHMARK_DOMAIN);
        let max_revealed_subnets = T::MaxPhysicalSubnetsUpperBound::get();
        let max_records = max_revealing_nodes.saturating_mul(max_revealed_subnets);
        let percentage_factor = Network::<T>::percentage_factor_as_u128();
        let stake_weight_factor = DefaultOverwatchStakeWeightFactor::get();
        assert_eq!(
            stake_weight_factor,
            percentage_factor.saturating_mul(9) / 10
        );
        OverwatchStakeWeightFactor::<T>::set(stake_weight_factor);
        let base_stake = OverwatchMinStakeBalance::<T>::get();
        assert_eq!(base_stake, 100u128.saturating_mul(percentage_factor));
        let expected_reward_budget = T::OverwatchEpochEmissions::get()
            .checked_mul(multiplier as u128)
            .expect("maximum rollover reward budget fits u128");

        let mut overwatch_nodes = Vec::with_capacity(max_revealing_nodes as usize);
        for node_index in 0..max_revealing_nodes {
            let validator_id = node_index.saturating_add(1);
            let hotkey_index = validator_id.saturating_add(max_revealing_nodes);
            let node_id = insert_overwatch_node::<T>(validator_id, hotkey_index);
            let stake = base_stake
                .checked_mul(validator_id as u128)
                .expect("maximum rollover node stake fits u128");
            set_overwatch_stake::<T>(node_id, stake);
            overwatch_nodes.push((node_id, validator_id, stake));
        }
        TotalOverwatchNodes::<T>::set(max_revealing_nodes);

        CurrentOverwatchEpoch::<T>::put(0);
        OverwatchEpochStartBlock::<T>::put(0);
        ActiveOverwatchEpochLengthMultiplier::<T>::put(multiplier);
        PendingOverwatchSettlement::<T>::kill();
        OverwatchEpochSettlementSnapshots::<T>::remove(0);
        let maximum_subnet_ids = (1..=max_revealed_subnets).collect::<BTreeSet<_>>();
        ActiveOverwatchRevealStats::<T>::put(OverwatchRevealStats::<T> {
            records: max_records,
            subnet_revealer_counts: maximum_subnet_ids
                .iter()
                .copied()
                .map(|subnet_id| (subnet_id, max_revealing_nodes))
                .collect::<BTreeMap<_, _>>()
                .try_into()
                .expect("maximum subnet-count fixture fits its type bound"),
        });
        for (node_id, _, _) in &overwatch_nodes {
            let reveals: BoundedBTreeMap<u32, u128, T::MaxPhysicalSubnetsUpperBound> =
                maximum_subnet_ids
                    .iter()
                    .copied()
                    .map(|subnet_id| (subnet_id, percentage_factor / 2))
                    .collect::<BTreeMap<_, _>>()
                    .try_into()
                    .expect("maximum rollover reveal row fits its type bound");
            let commits: BoundedBTreeMap<u32, T::Hash, T::MaxPhysicalSubnetsUpperBound> =
                maximum_subnet_ids
                    .iter()
                    .copied()
                    .map(|subnet_id| (subnet_id, T::Hashing::hash_of(&subnet_id)))
                    .collect::<BTreeMap<_, _>>()
                    .try_into()
                    .expect("maximum rollover commit row fits its type bound");
            OverwatchReveals::<T>::insert(0, node_id, reveals);
            OverwatchCommits::<T>::insert(0, node_id, commits);
        }

        #[block]
        {
            Network::<T>::advance_overwatch_epoch(rollover_block);
        }

        assert_eq!(CurrentOverwatchEpoch::<T>::get(), 1);
        assert_eq!(
            PendingOverwatchSettlement::<T>::get().map(|settlement| settlement.epoch),
            Some(0)
        );
        assert_eq!(OverwatchEpochStartBlock::<T>::get(), rollover_block);
        let settlement = PendingOverwatchSettlement::<T>::get().unwrap();
        assert_eq!(settlement.reveal_records, max_records);
        let snapshot = OverwatchEpochSettlementSnapshots::<T>::get(0)
            .expect("rollover stores the completed epoch settlement snapshot");
        assert_eq!(snapshot.stake_weight_factor, stake_weight_factor);
        assert_eq!(snapshot.reward_budget, expected_reward_budget);
        assert_eq!(snapshot.nodes.len() as u32, max_revealing_nodes);
        for (node_id, _validator_id, stake) in overwatch_nodes {
            let node_snapshot = snapshot
                .nodes
                .get(&node_id)
                .expect("every canonical Overwatch node is snapshotted");
            assert_eq!(node_snapshot.stake, stake);
            assert!(OverwatchCommits::<T>::get(0, node_id).is_empty());
        }
    }

    #[benchmark]
    fn advance_overwatch_epoch_noop() {
        let multiplier = OverwatchEpochLengthMultiplier::<T>::get();
        let rollover_block = T::EpochLength::get().saturating_mul(multiplier);
        let max_revealing_nodes = T::MaxOverwatchNodesUpperBound::get();
        let max_revealed_subnets = T::MaxPhysicalSubnetsUpperBound::get();
        CurrentOverwatchEpoch::<T>::put(0);
        OverwatchEpochStartBlock::<T>::put(0);
        ActiveOverwatchEpochLengthMultiplier::<T>::put(multiplier);
        PendingOverwatchSettlement::<T>::put(PendingOverwatchSettlementData {
            epoch: 0,
            reveal_records: max_revealing_nodes.saturating_mul(max_revealed_subnets),
        });

        #[block]
        {
            Network::<T>::advance_overwatch_epoch(rollover_block);
        }

        // An already-pending settlement is the longest no-op path: it reaches the third storage
        // read but must not overwrite the prior epoch or mutate the active round.
        assert_eq!(CurrentOverwatchEpoch::<T>::get(), 0);
        assert_eq!(OverwatchEpochStartBlock::<T>::get(), 0);
        assert_eq!(
            PendingOverwatchSettlement::<T>::get().map(|settlement| settlement.epoch),
            Some(0)
        );
    }

    // Informational purposes only
    #[benchmark]
    fn handle_subnet_emission_weights(x: Linear<1, { MAX_PHYSICAL_SUBNETS_BENCHMARK_DOMAIN }>) {
        NewRegistrationCostMultiplier::<T>::set(1000000000000000000);
        let end = MinSubnetNodes::<T>::get();

        for s in 0..x {
            let path: Vec<u8> = format!("subnet-name-{s}").into();
            build_activated_subnet::<T>(
                path,
                0,
                end,
                DEFAULT_DEPOSIT_AMOUNT,
                DEFAULT_SUBNET_NODE_STAKE,
            );
        }

        // Exercise the terminal-floor path, which performs the maximum number of annual decay
        // iterations used by the launch schedule.
        increase_epochs::<T>(T::EpochsPerYear::get().saturating_mul(3).max(1));

        let epoch = Network::<T>::get_current_epoch_as_u32();
        let finalized_overwatch_epoch = CurrentOverwatchEpoch::<T>::get();
        LastFinalizedOverwatchEpoch::<T>::put(finalized_overwatch_epoch);
        let max_historical_nodes = T::MaxSubnetNodesUpperBound::get();
        let mut effective_subnet_weights = BTreeMap::new();
        for s in 0..x {
            let path: Vec<u8> = format!("subnet-name-{s}").into();
            let subnet_id = SubnetName::<T>::get::<Vec<u8>>(path).unwrap();
            SubnetElectedValidator::<T>::insert(
                subnet_id,
                epoch.saturating_sub(1),
                ElectedConsensusRound {
                    validator_subnet_node_id: 1,
                    validator_id: SubnetNodeValidatorId::<T>::get(subnet_id, 1).unwrap_or_default(),
                    emergency: None,
                    // The hot path uses `contains_key`, whose measured trie proof includes the
                    // complete external value. A prior round can retain all 512 historical IDs
                    // after those nodes leave the live subnet, so maximize both collections.
                    eligible_subnet_node_ids: (1..=max_historical_nodes).collect(),
                    eligible_validator_identity_ids: (1..=max_historical_nodes)
                        .map(|node_id| (node_id, node_id))
                        .collect(),
                    validator_delegate_stake_balance: 0,
                    policy: Network::<T>::consensus_policy_snapshot(
                        subnet_id,
                        epoch.saturating_sub(1),
                    ),
                },
            );
            effective_subnet_weights.insert(subnet_id, Network::<T>::percentage_factor_as_u128());
            let magnitude = (s as i128 + 1).saturating_mul(1_000_000);
            SubnetNetFlow::<T>::insert(subnet_id, if s % 2 == 0 { -magnitude } else { magnitude });
            SubnetNetFlowSmoothedWeight::<T>::insert(
                subnet_id,
                Network::<T>::percentage_factor_as_u128()
                    .saturating_div(s as u128 + 2)
                    .max(1),
            );
        }
        seed_max_effective_overwatch_cache::<T>(
            finalized_overwatch_epoch,
            effective_subnet_weights,
        );

        #[block]
        {
            let _ = Network::<T>::handle_subnet_emission_weights(epoch);
        }

        let subnet_emission_weights = FinalSubnetEmissionWeights::<T>::get(epoch);
        assert_eq!(subnet_emission_weights.subnet_weights.len() as u32, x);
    }

    #[benchmark]
    fn handle_subnet_emission_weights_empty() {
        let epoch = Network::<T>::get_current_epoch_as_u32();
        assert_eq!(SubnetsData::<T>::iter_keys().count(), 0);
        seed_max_effective_overwatch_cache::<T>(CurrentOverwatchEpoch::<T>::get(), BTreeMap::new());

        #[block]
        {
            let _ = Network::<T>::handle_subnet_emission_weights(epoch);
        }

        assert!(!FinalSubnetEmissionWeights::<T>::contains_key(epoch));
    }

    #[benchmark]
    fn execute_ready_swap_selectors() {
        #[block]
        {
            let _ = MaxSwapQueueCallsPerBlock::<T>::get();
            let _ = SwapQueueCount::<T>::get();
        }
    }

    // Informational purposes only
    #[benchmark]
    fn execute_ready_swap_queue(q: Linear<1, { MAX_SWAP_QUEUE_BENCHMARK_DOMAIN }>) {
        let mut queue: SwapQueueIds<T> = BoundedVec::new();
        for queue_id in 0..q {
            assert!(queue.try_push(queue_id).is_ok());
        }
        SwapQueueOrder::<T>::set(queue);
        SwapQueueCount::<T>::set(q);
        TotalQueuedSwapPrincipal::<T>::set(0);
        assert_benchmark_queued_swap_principal::<T>();
        let mut weight_meter = WeightMeter::new();

        #[block]
        {
            let queue = Network::<T>::take_swap_queue(&mut weight_meter);
            // Model the maximum retained rotation: every original ID was scanned, preserved, and
            // appended behind the now-empty unscanned suffix.
            let scanned = queue.len();
            let mut rotated = SwapQueueIds::<T>::default();
            for queue_id in queue.iter().copied() {
                rotated
                    .try_push(queue_id)
                    .expect("rotated IDs are a subset of the bounded queue");
            }
            Network::<T>::finish_swap_queue(queue, scanned, rotated, &mut weight_meter);
        }

        assert_eq!(SwapQueueOrder::<T>::get().len() as u32, q);
        assert!(SwapQueueOrder::<T>::get().iter().copied().eq(0..q));
        assert_eq!(SwapQueueCount::<T>::get(), q);
        assert_benchmark_queued_swap_principal::<T>();
    }

    // Informational purposes only
    #[benchmark]
    fn execute_ready_swap_calls(x: Linear<1, { MAX_SWAP_QUEUE_BENCHMARK_DOMAIN }>) {
        let block_number = get_current_block_as_u32::<T>();
        let balance = DEFAULT_DELEGATE_STAKE_TO_BE_ADDED;
        for queue_id in 0..x {
            let validator_id = queue_id.saturating_add(1);
            ensure_validator::<T>(validator_id);
            // `contains_key` proves the complete trie leaf even though it does not decode it.
            // Max-fill both optional validator payloads so the one-item hook envelope is safe.
            ValidatorsData::<T>::mutate(validator_id, |validator| {
                validator.delegate_account = Some(DelegateAccount {
                    account_id: get_account::<T>("ready_validator_delegate_account", validator_id),
                    rate: u128::MAX,
                });
                validator.identity = Some(max_emission_identity::<T>());
            });
            let account_id = get_account::<T>("ready_validator_swap_account", queue_id);
            SwapCallQueue::<T>::insert(
                queue_id,
                QueuedSwapItem {
                    id: queue_id,
                    call: QueuedSwapCall::SwapToValidatorDelegateStake {
                        account_id,
                        to_validator_id: validator_id,
                        balance,
                    },
                    queued_at_block: block_number,
                    execute_after_blocks: 0,
                },
            );
        }
        TotalQueuedSwapPrincipal::<T>::set(
            (x as u128)
                .checked_mul(balance)
                .expect("benchmark queue principal fits u128"),
        );
        assert_benchmark_queued_swap_principal::<T>();
        let mut weight_meter = WeightMeter::new();

        #[block]
        {
            for queue_id in 0..x {
                let _ = Network::<T>::execute_ready_swap_call_item(
                    queue_id,
                    block_number,
                    &mut weight_meter,
                );
            }
        }

        assert_eq!(SwapCallQueue::<T>::iter().count(), 0);
        assert_eq!(TotalQueuedSwapPrincipal::<T>::get(), 0);
        for queue_id in 0..x {
            let validator_id = queue_id.saturating_add(1);
            let account_id = get_account::<T>("ready_validator_swap_account", queue_id);
            assert_ne!(
                AccountValidatorDelegateStakeShares::<T>::get(account_id, validator_id),
                0
            );
        }
    }

    // Informational purposes only
    #[benchmark]
    fn execute_ready_swap_subnet_calls(x: Linear<1, { MAX_SWAP_QUEUE_BENCHMARK_DOMAIN }>) {
        let destination_count = x.min(T::MaxPhysicalSubnetsUpperBound::get());
        let mut subnet_ids = Vec::with_capacity(destination_count as usize);
        for destination in 0..destination_count {
            let path: Vec<u8> = format!("ready-swap-subnet-{destination}").into();
            build_activated_subnet::<T>(
                path.clone(),
                0,
                MinSubnetNodes::<T>::get(),
                DEFAULT_DEPOSIT_AMOUNT,
                DEFAULT_SUBNET_NODE_STAKE,
            );
            let subnet_id =
                SubnetName::<T>::get::<Vec<u8>>(path).expect("benchmark subnet must be active");
            subnet_ids.push(subnet_id);
        }
        for subnet_id in subnet_ids.iter().copied() {
            max_fill_benchmark_subnet_data::<T>(subnet_id);
        }
        let block_number = get_current_block_as_u32::<T>();
        let balance = DEFAULT_DELEGATE_STAKE_TO_BE_ADDED;
        for queue_id in 0..x {
            let account_id = get_account::<T>("ready_subnet_swap_account", queue_id);
            let to_subnet_id = subnet_ids[(queue_id % destination_count) as usize];
            SwapCallQueue::<T>::insert(
                queue_id,
                QueuedSwapItem {
                    id: queue_id,
                    call: QueuedSwapCall::SwapToSubnetDelegateStake {
                        account_id,
                        to_subnet_id,
                        balance,
                    },
                    queued_at_block: block_number,
                    execute_after_blocks: 0,
                },
            );
        }
        TotalQueuedSwapPrincipal::<T>::set(
            (x as u128)
                .checked_mul(balance)
                .expect("benchmark queue principal fits u128"),
        );
        assert_benchmark_queued_swap_principal::<T>();
        let mut weight_meter = WeightMeter::new();

        #[block]
        {
            for queue_id in 0..x {
                let _ = Network::<T>::execute_ready_swap_call_item(
                    queue_id,
                    block_number,
                    &mut weight_meter,
                );
            }
        }

        assert_eq!(SwapCallQueue::<T>::iter().count(), 0);
        assert_eq!(TotalQueuedSwapPrincipal::<T>::get(), 0);
        for queue_id in 0..x {
            let account_id = get_account::<T>("ready_subnet_swap_account", queue_id);
            let subnet_id = subnet_ids[(queue_id % destination_count) as usize];
            assert_ne!(
                AccountSubnetDelegateStakeShares::<T>::get(account_id, subnet_id),
                0
            );
        }
    }

    // Informational purposes only
    #[benchmark]
    fn execute_ready_swap_refunds(x: Linear<1, { MAX_SWAP_QUEUE_BENCHMARK_DOMAIN }>) {
        let block_number = get_current_block_as_u32::<T>();
        let balance = DEFAULT_DELEGATE_STAKE_TO_BE_ADDED;
        let max_unbondings = T::MaxUnbondingsUpperBound::get();
        MaxUnbondings::<T>::set(max_unbondings);
        let cooldown_blocks =
            DelegateStakeCooldownEpochs::<T>::get().saturating_mul(T::EpochLength::get());
        let claim_block = block_number.saturating_add(cooldown_blocks);
        let missing_subnet_id = u32::MAX;
        assert!(!SubnetsData::<T>::contains_key(missing_subnet_id));

        for id in 0..x {
            let account_id = get_account::<T>("ready_swap_account", id);

            // The invalid destination exercises the refund branch. Fill every ledger that will
            // execute to its bound while retaining the target claim block, so the benchmark pays
            // to decode and rewrite the maximum value without triggering an unbounded auto-claim.
            let mut ledger = BTreeMap::new();
            ledger.insert(
                claim_block,
                UnbondingEntry {
                    network: 1,
                    overwatch: 0,
                },
            );
            for offset in 1..max_unbondings {
                ledger.insert(
                    claim_block.saturating_add(offset),
                    UnbondingEntry {
                        network: 1,
                        overwatch: 0,
                    },
                );
            }
            assert_eq!(ledger.len() as u32, max_unbondings);
            StakeUnbondingLedger::<T>::insert(&account_id, ledger);

            SwapCallQueue::<T>::insert(
                id,
                QueuedSwapItem {
                    id,
                    call: QueuedSwapCall::SwapToSubnetDelegateStake {
                        account_id,
                        to_subnet_id: missing_subnet_id,
                        balance,
                    },
                    queued_at_block: block_number,
                    execute_after_blocks: 0,
                },
            );
        }
        TotalQueuedSwapPrincipal::<T>::set(
            (x as u128)
                .checked_mul(balance)
                .expect("benchmark queue principal fits u128"),
        );
        TotalNetworkUnbondingBalance::<T>::set((x as u128).saturating_mul(max_unbondings as u128));
        assert_benchmark_queued_swap_principal::<T>();

        let mut weight_meter = WeightMeter::new();

        #[block]
        {
            for queue_id in 0..x {
                let _ = Network::<T>::execute_ready_swap_call_item(
                    queue_id,
                    block_number,
                    &mut weight_meter,
                );
            }
        }

        assert_eq!(SwapCallQueue::<T>::iter().count(), 0);
        assert_eq!(TotalQueuedSwapPrincipal::<T>::get(), 0);
        for id in 0..x {
            let account_id = get_account::<T>("ready_swap_account", id);
            assert_eq!(
                StakeUnbondingLedger::<T>::get(account_id)
                    .get(&claim_block)
                    .map(|entry| entry.network),
                Some(balance.saturating_add(1))
            );
        }
    }

    /// Mixed ready-prefix vertex with `x - 2` validator calls plus one subnet call and one
    /// maximum-ledger refund. Together with the other two vertices this bounds every reachable
    /// affine composition of the three item branches for a fixed prefix length.
    #[benchmark]
    fn execute_ready_swap_mixed_validator(
        x: Linear<{ MIN_MIXED_SWAP_BENCHMARK_DOMAIN }, { MAX_SWAP_QUEUE_BENCHMARK_DOMAIN }>,
    ) {
        let context = prepare_mixed_swap_benchmark::<T>(x, MixedSwapBranch::Validator);
        let mut weight_meter = WeightMeter::new();

        #[block]
        {
            for queue_id in 0..x {
                let _ = Network::<T>::execute_ready_swap_call_item(
                    queue_id,
                    context.block_number,
                    &mut weight_meter,
                );
            }
        }

        verify_mixed_swap_benchmark::<T>(x, MixedSwapBranch::Validator, &context);
    }

    /// Mixed ready-prefix vertex with `x - 2` subnet calls plus one validator call and one
    /// maximum-ledger refund.
    #[benchmark]
    fn execute_ready_swap_mixed_subnet(
        x: Linear<{ MIN_MIXED_SWAP_BENCHMARK_DOMAIN }, { MAX_SWAP_QUEUE_BENCHMARK_DOMAIN }>,
    ) {
        let context = prepare_mixed_swap_benchmark::<T>(x, MixedSwapBranch::Subnet);
        let mut weight_meter = WeightMeter::new();

        #[block]
        {
            for queue_id in 0..x {
                let _ = Network::<T>::execute_ready_swap_call_item(
                    queue_id,
                    context.block_number,
                    &mut weight_meter,
                );
            }
        }

        verify_mixed_swap_benchmark::<T>(x, MixedSwapBranch::Subnet, &context);
    }

    /// Mixed ready-prefix vertex with `x - 2` maximum-ledger refunds plus one validator call and
    /// one subnet call.
    #[benchmark]
    fn execute_ready_swap_mixed_refund(
        x: Linear<{ MIN_MIXED_SWAP_BENCHMARK_DOMAIN }, { MAX_SWAP_QUEUE_BENCHMARK_DOMAIN }>,
    ) {
        let context = prepare_mixed_swap_benchmark::<T>(x, MixedSwapBranch::Refund);
        let mut weight_meter = WeightMeter::new();

        #[block]
        {
            for queue_id in 0..x {
                let _ = Network::<T>::execute_ready_swap_call_item(
                    queue_id,
                    context.block_number,
                    &mut weight_meter,
                );
            }
        }

        verify_mixed_swap_benchmark::<T>(x, MixedSwapBranch::Refund, &context);
    }

    // Informational purposes only
    #[benchmark]
    fn do_epoch_preliminaries(x: Linear<0, { MAX_PHYSICAL_SUBNETS_BENCHMARK_DOMAIN }>) {
        assert!(x <= T::MaxPhysicalSubnetsUpperBound::get());
        let subnet_ids =
            seed_live_subnet_delegate_stake_cohort::<T>(x, |_| DEFAULT_DELEGATE_STAKE_TO_BE_ADDED);

        MaxSubnets::<T>::put(0);
        SubnetRemovalCheckInterval::<T>::put(1);
        SubnetRemovalActivationCooldown::<T>::put(0);
        PrevSubnetActivationEpoch::<T>::put(0);
        DelegateStakeSubnetRemovalInterval::<T>::put(1);
        MinSubnetDelegateStakeBalance::<T>::put(1);
        MinSubnetDelegateStakeFactor::<T>::put(DefaultMinSubnetDelegateStakeFactor::get());
        LessThanMinNodesSubnetReputationFactor::<T>::put(0);
        for subnet_id in &subnet_ids {
            TotalSubnetElectableNodes::<T>::insert(subnet_id, 0);
            SubnetReputation::<T>::insert(subnet_id, Network::<T>::percentage_factor_as_u128());
        }

        #[block]
        {
            // Any subnet may enter `try_do_remove_subnet`. Its three compact selectors are read
            // before a cleanup reservation can be rejected, so model their per-subnet proof and
            // ref-time here. The zero inner limit makes every attempted removal `Deferred`, which
            // keeps the snapshotted subnet count intact and exercises the final capacity recheck
            // without performing the independently benchmarked cleanup.
            for subnet_id in &subnet_ids {
                let _ = TotalActiveSubnetNodes::<T>::get(subnet_id);
                let _ = TotalSubnetNodes::<T>::get(subnet_id);
                let _ = TotalOverwatchNodes::<T>::get();
            }
            Network::<T>::do_epoch_preliminaries(
                &mut WeightMeter::with_limit(Weight::zero()),
                get_current_block_as_u32::<T>(),
                Network::<T>::get_current_epoch_as_u32(),
            );
        }

        for subnet_id in subnet_ids {
            assert!(SubnetsData::<T>::contains_key(subnet_id));
        }
    }

    // Small settlements can add at most one distinct revealer and one distinct subnet per record.
    // Make both cardinalities equal to `r` so the model covers their worst reachable growth rather
    // than charging the 64-node/17-subnet fixture used by the large-record region.
    #[benchmark]
    fn calculate_overwatch_rewards_small(r: Linear<1, { MAX_PHYSICAL_SUBNETS_BENCHMARK_DOMAIN }>) {
        let (overwatch_epoch, overwatch_nodes, subnet_ids) =
            prepare_overwatch_reward_benchmark::<T>(r, r, r);

        #[block]
        {
            Network::<T>::calculate_overwatch_rewards();
        }

        assert_overwatch_reward_benchmark_result::<T>(
            overwatch_epoch,
            &overwatch_nodes,
            &subnet_ids,
        );
    }

    // Once all 17 physical subnets can be represented, the next worst-case dimension is one new
    // revealer per record. This region keeps all 17 subnets present and grows revealers with `r`.
    #[benchmark]
    fn calculate_overwatch_rewards_medium(
        r: Linear<
            { MAX_PHYSICAL_SUBNETS_BENCHMARK_DOMAIN },
            { MAX_OVERWATCH_NODES_BENCHMARK_DOMAIN },
        >,
    ) {
        let (overwatch_epoch, overwatch_nodes, subnet_ids) =
            prepare_overwatch_reward_benchmark::<T>(r, r, MAX_PHYSICAL_SUBNETS_BENCHMARK_DOMAIN);

        #[block]
        {
            Network::<T>::calculate_overwatch_rewards();
        }

        assert_overwatch_reward_benchmark_result::<T>(
            overwatch_epoch,
            &overwatch_nodes,
            &subnet_ids,
        );
    }

    // Large settlements have saturated both independent bounded cardinalities. Additional records
    // fill the remaining unique node/subnet pairs, up to the reachable 64 * 17 matrix.
    #[benchmark]
    fn calculate_overwatch_rewards(
        r: Linear<
            { MAX_OVERWATCH_NODES_BENCHMARK_DOMAIN },
            { MAX_OVERWATCH_REVEAL_RECORDS_BENCHMARK_DOMAIN },
        >,
    ) {
        let (overwatch_epoch, overwatch_nodes, subnet_ids) = prepare_overwatch_reward_benchmark::<T>(
            r,
            MAX_OVERWATCH_NODES_BENCHMARK_DOMAIN,
            MAX_PHYSICAL_SUBNETS_BENCHMARK_DOMAIN,
        );

        #[block]
        {
            Network::<T>::calculate_overwatch_rewards();
        }

        assert_overwatch_reward_benchmark_result::<T>(
            overwatch_epoch,
            &overwatch_nodes,
            &subnet_ids,
        );
    }

    #[benchmark]
    fn calculate_overwatch_rewards_empty() {
        let current_overwatch_epoch = 1u32;
        CurrentOverwatchEpoch::<T>::put(current_overwatch_epoch);
        seed_max_prior_overwatch_signal::<T>(current_overwatch_epoch.saturating_sub(1));
        let epoch_length_multiplier = ActiveOverwatchEpochLengthMultiplier::<T>::get();
        let stake_weight_factor = OverwatchStakeWeightFactor::<T>::get();
        let reward_budget = T::OverwatchEpochEmissions::get()
            .checked_mul(epoch_length_multiplier as u128)
            .expect("empty settlement reward budget fits u128");
        PendingOverwatchSettlement::<T>::put(PendingOverwatchSettlementData {
            epoch: current_overwatch_epoch,
            reveal_records: 0,
        });
        OverwatchEpochSettlementSnapshots::<T>::insert(
            current_overwatch_epoch,
            OverwatchEpochSettlementSnapshot::<T> {
                stake_weight_factor,
                reward_budget,
                nodes: BTreeMap::new()
                    .try_into()
                    .expect("empty settlement node map fits its runtime bound"),
            },
        );

        #[block]
        {
            Network::<T>::calculate_overwatch_rewards();
        }

        assert!(PendingOverwatchSettlement::<T>::get().is_none());
        assert!(!OverwatchEpochSettlementSnapshots::<T>::contains_key(
            current_overwatch_epoch
        ));
        assert_eq!(
            LastFinalizedOverwatchEpoch::<T>::get(),
            Some(current_overwatch_epoch)
        );
        assert_eq!(
            LatestOverwatchSignalRevision::<T>::get(),
            PRIOR_OVERWATCH_SIGNAL_REVISION.saturating_add(1)
        );
        assert_eq!(
            OverwatchNodeWeights::<T>::iter_prefix(current_overwatch_epoch).count(),
            0
        );
        assert_eq!(
            OverwatchSubnetWeights::<T>::iter_prefix(current_overwatch_epoch).count(),
            0
        );
    }

    /// Slot assignment is read even when the slot is empty, so its trie proof must be admitted
    /// before the hook can decide whether a subnet-specific step exists.
    #[benchmark]
    fn emission_slot_selector() {
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            MinSubnetNodes::<T>::get(),
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let slot = SubnetSlot::<T>::get(subnet_id).expect("benchmark subnet must own a slot");
        assert_eq!(SlotAssignment::<T>::get(slot), Some(subnet_id));

        #[block]
        {
            let _ = SlotAssignment::<T>::get(slot);
        }
    }

    /// After a slot resolves, these compact keys select every independently generated emission
    /// component. Benchmark the maximum historical case as one always-composed proof envelope.
    #[benchmark]
    fn emission_step_selectors() {
        MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            T::MaxSubnetNodesUpperBound::get(),
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        SubnetConsensusSubmissionMaxItems::<T>::insert(
            subnet_id,
            0,
            T::MaxSubnetNodesUpperBound::get(),
        );

        #[block]
        {
            let _ = SubnetSlot::<T>::get(subnet_id);
            let _ = SubnetConsensusSubmissionMaxItems::<T>::get(subnet_id, 0);
            let _ = TotalSubnetElectableNodes::<T>::get(subnet_id);
            let _ = TotalSubnetNodes::<T>::get(subnet_id);
            let _ = TotalActiveSubnetNodes::<T>::get(subnet_id);
        }
    }

    /// Decode and deterministically materialize the active pending-removal set, then perform a
    /// complete membership scan. This is conservative for cleanup's direct iteration and also
    /// covers election/proposal filtering without coupling physical deletion to this selector.
    /// Each deletion is charged independently by the generated active-removal model.
    #[benchmark]
    fn pending_active_removal_scan(a: Linear<1, { MAX_SUBNET_NODES_BENCHMARK_DOMAIN }>) {
        let subnet_id = 1;
        let pending: BoundedBTreeSet<u32, T::MaxSubnetNodesUpperBound> = (1..=a)
            .collect::<BTreeSet<_>>()
            .try_into()
            .expect("active pending-removal benchmark fits its configured bound");
        PendingActiveNodeRemovals::<T>::insert(subnet_id, pending);

        #[block]
        {
            let pending = PendingActiveNodeRemovals::<T>::get(subnet_id);
            let pending_ids: BTreeSet<u32> = pending.iter().copied().collect();
            let mut inspected = 0u32;
            let eligible = (1..=a).find(|subnet_node_id| {
                inspected = inspected.saturating_add(1);
                !pending_ids.contains(subnet_node_id)
            });
            assert!(eligible.is_none());
            assert_eq!(inspected, a);
        }
    }

    /// Registered cleanup has its own smaller bounded set and therefore its own scan model. The
    /// logical queue dequeue happens during settlement; this benchmark covers decode,
    /// materialization, and a complete membership scan before independently metered deletion.
    #[benchmark]
    fn pending_registered_removal_scan(r: Linear<1, { MAX_REGISTERED_NODES_BENCHMARK_DOMAIN }>) {
        let subnet_id = 1;
        let pending: BoundedBTreeSet<u32, T::MaxRegisteredNodesUpperBound> = (1..=r)
            .collect::<BTreeSet<_>>()
            .try_into()
            .expect("registered pending-removal benchmark fits its configured bound");
        PendingRegisteredNodeRemovals::<T>::insert(subnet_id, pending);

        #[block]
        {
            let pending = PendingRegisteredNodeRemovals::<T>::get(subnet_id);
            let pending_ids: BTreeSet<u32> = pending.iter().copied().collect();
            let mut inspected = 0u32;
            let eligible = (1..=r).find(|subnet_node_id| {
                inspected = inspected.saturating_add(1);
                !pending_ids.contains(subnet_node_id)
            });
            assert!(eligible.is_none());
            assert_eq!(inspected, r);
        }
    }

    /// Accepted historical settlement with every snapshotted node still live. Historical queue
    /// priority/removal is independently generated and added to this accepted component. Bounded
    /// active cleanup is composed after the hook selects its accepted/rejected/emergency branch,
    /// allowing every independent protocol maximum to remain reachable. Current election, queue
    /// activation, and burn maintenance likewise use separate components.
    #[benchmark]
    fn emission_step(
        h: Linear<{ MIN_CONSENSUS_VALIDATOR_IDENTITIES }, { MAX_SUBNET_NODES_BENCHMARK_DOMAIN }>,
    ) {
        NewRegistrationCostMultiplier::<T>::set(1000000000000000000);
        MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());

        // Reward distribution decodes the complete validator record for every historical node.
        // Fill every bounded identity field so the h component includes the worst-case proof
        // size of that hot read, rather than measuring the default `identity: None` fixture.
        let max_identity_bytes: NetworkBytes<T> = vec![1; T::MaxVectorLength::get() as usize]
            .try_into()
            .unwrap();
        let max_identity_url: NetworkUrl<T> =
            vec![2; T::MaxUrlLength::get() as usize].try_into().unwrap();
        let max_identity_social: NetworkSocialId<T> = vec![3; T::MaxSocialIdLength::get() as usize]
            .try_into()
            .unwrap();
        let max_identity = IdentityData::<T> {
            name: Some(max_identity_bytes.clone()),
            url: Some(max_identity_url.clone()),
            image: Some(max_identity_url.clone()),
            discord: Some(max_identity_social.clone()),
            x: Some(max_identity_social.clone()),
            telegram: Some(max_identity_social),
            github: Some(max_identity_url.clone()),
            hugging_face: Some(max_identity_url),
            description: Some(max_identity_bytes.clone()),
            misc: Some(max_identity_bytes),
        };

        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            h,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let accepted_starting_reputation =
            Network::<T>::percentage_factor_as_u128().saturating_div(2);
        for subnet_node_id in 1..=h {
            let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id)
                .expect("benchmark node must retain its validator identity");
            if ValidatorDelegateStakeBalance::<T>::get(validator_id) == 0 {
                ValidatorDelegateStakeBalance::<T>::insert(validator_id, 1);
                ValidatorDelegateStakeShares::<T>::insert(validator_id, 1);
                TotalValidatorDelegateStakeBalance::<T>::mutate(|total| {
                    *total = total.saturating_add(1);
                });
            }
            let delegate_account = get_account::<T>("emission-delegate-account", validator_id);
            ValidatorsData::<T>::mutate(validator_id, |validator| {
                validator.delegate_reward_rate =
                    Network::<T>::percentage_factor_as_u128().saturating_div(2);
                validator.delegate_account = Some(DelegateAccount {
                    account_id: delegate_account,
                    rate: Network::<T>::percentage_factor_as_u128().saturating_div(10),
                });
                validator.identity = Some(max_identity.clone());
            });
            ValidatorNodeDelegateStakeWeights::<T>::mutate(validator_id, |weights| {
                weights.insert(
                    (subnet_id, subnet_node_id),
                    Network::<T>::percentage_factor_as_u128(),
                );
            });
            // Keep every accepted scored node below the maximum so the main h model includes the
            // normal included-node reputation mutation and event write.
            SubnetNodeReputation::<T>::insert(
                subnet_id,
                subnet_node_id,
                accepted_starting_reputation,
            );
        }

        // Get to activation epoch (not needed for this test but do anyway)
        increase_epochs::<T>(1);

        // Set to correct block to elect a validator
        set_block_to_subnet_slot_epoch::<T>(Network::<T>::get_current_epoch_as_u32(), subnet_id);
        let subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
        Network::<T>::elect_validator(
            subnet_id,
            subnet_epoch,
            Network::<T>::get_current_block_as_u32(),
        );
        // Run consensus, submit proposal, attest
        run_subnet_consensus_step::<T>(subnet_id, None, None);

        // Ensure it worked
        let submission = SubnetConsensusSubmission::<T>::try_get(
            subnet_id,
            Network::<T>::get_current_subnet_epoch_as_u32(subnet_id),
        );
        assert!(submission.is_ok());
        assert_eq!(submission.unwrap().attests.len() as u32, h);
        assert_eq!(
            SubnetConsensusSubmissionMaxItems::<T>::get(
                subnet_id,
                Network::<T>::get_current_subnet_epoch_as_u32(subnet_id),
            ),
            h,
        );

        let mut stake_snapshot: BTreeMap<T::AccountId, u128> = BTreeMap::new();
        for node_index in 0..h {
            let _n = node_index + 1;
            let hotkey = Network::<T>::get_subnet_node_associated_hotkey(subnet_id, _n).unwrap();
            let stake = NodeSubnetStake::<T>::get(_n, subnet_id);
            stake_snapshot.insert(hotkey.clone(), stake);
        }

        increase_epochs::<T>(1);
        set_block_to_subnet_slot_epoch::<T>(Network::<T>::get_current_epoch_as_u32(), subnet_id);

        // Calc subnet weights
        let _ =
            Network::<T>::handle_subnet_emission_weights(Network::<T>::get_current_epoch_as_u32());

        // The timed step decodes the complete epoch allocation even though it consumes only the
        // target entry. Fill it to the physical subnet bound, and likewise maximize the target
        // lifecycle record decoded at the end of the step.
        let current_epoch = Network::<T>::get_current_epoch_as_u32();
        FinalSubnetEmissionWeights::<T>::mutate(current_epoch, |distribution| {
            for index in 0..T::MaxPhysicalSubnetsUpperBound::get().saturating_sub(1) {
                distribution
                    .subnet_weights
                    .insert(100_000 + index, u128::MAX.saturating_sub(index as u128));
            }
        });
        max_fill_benchmark_subnet_data::<T>(subnet_id);
        SubnetsData::<T>::mutate(subnet_id, |maybe_subnet| {
            let subnet = maybe_subnet.as_mut().expect("benchmark subnet must exist");
            subnet.state = SubnetState::Paused;
            subnet.pause = Some(SubnetPauseData {
                started_global_epoch: current_epoch,
                started_subnet_epoch: Network::<T>::get_current_subnet_epoch_as_u32(subnet_id),
            });
        });

        // Verify weights exist
        let subnet_emission_weights = FinalSubnetEmissionWeights::<T>::get(current_epoch);
        let subnet_weight = subnet_emission_weights.subnet_weights.get(&subnet_id);
        assert!(subnet_weight.is_some());
        assert_eq!(
            subnet_emission_weights.subnet_weights.len() as u32,
            T::MaxPhysicalSubnetsUpperBound::get()
        );

        #[block]
        {
            let _ = Network::<T>::emission_step(
                &mut WeightMeter::new(),
                Network::<T>::get_current_block_as_u32(),
                Network::<T>::get_current_epoch_as_u32(),
                Network::<T>::get_current_subnet_epoch_as_u32(subnet_id),
                subnet_id,
            );
        }

        for node_index in 0..h {
            let _n = node_index + 1;
            let hotkey = Network::<T>::get_subnet_node_associated_hotkey(subnet_id, _n).unwrap();
            let stake = NodeSubnetStake::<T>::get(_n, subnet_id);
            assert!(
                stake
                    > *stake_snapshot
                        .get(&hotkey)
                        .expect("every benchmark node must have a pre-settlement stake snapshot")
            );
            assert!(SubnetNodeReputation::<T>::get(subnet_id, _n)
                .is_some_and(|reputation| reputation > accepted_starting_reputation));
        }
        assert_eq!(TotalActiveSubnetNodes::<T>::get(subnet_id), h);
        assert_eq!(TotalSubnetNodes::<T>::get(subnet_id), h);
        assert!(SubnetNodeQueue::<T>::get(subnet_id).is_empty());
    }

    /// Historical accepted consensus can move a maximum-payload queued node from the final vector
    /// position to the front before quarantining another registered node. This component measures
    /// both position scans, the priority rewrite, the bounded pending-set update, and the logical
    /// queue dequeue. Physical registered-node cleanup is independently metered after election.
    #[benchmark]
    fn emission_step_accepted_queue_mutations(
        q: Linear<1, { MAX_REGISTERED_NODES_BENCHMARK_DOMAIN }>,
    ) {
        let (subnet_id, prioritize_queue_node_id, remove_queue_node_id, consensus_submission_data) =
            prepare_accepted_queue_mutations::<T>(q, false);

        #[block]
        {
            Network::<T>::handle_node_queue_consensus(
                &mut WeightMeter::new(),
                subnet_id,
                &consensus_submission_data,
                Network::<T>::percentage_factor_as_u128(),
            );
        }

        let queue_after = SubnetNodeQueue::<T>::get(subnet_id);
        assert_eq!(queue_after.len() as u32, q.saturating_sub(1));
        assert!(RegisteredSubnetNodesData::<T>::contains_key(
            subnet_id,
            remove_queue_node_id,
        ));
        assert!(PendingRegisteredNodeRemovals::<T>::get(subnet_id).contains(&remove_queue_node_id));
        if prioritize_queue_node_id != remove_queue_node_id {
            assert_eq!(queue_after.first().unwrap().id, prioritize_queue_node_id);
        }
    }

    /// Alternate reachable queue layout: removing the original front entry shortens the second
    /// position scan but maximizes survivor compaction in the registered-node retain pass. The hook
    /// takes the component-wise maximum because neither layout statically dominates the other.
    #[benchmark]
    fn emission_step_accepted_queue_mutations_front(
        q: Linear<1, { MAX_REGISTERED_NODES_BENCHMARK_DOMAIN }>,
    ) {
        let (subnet_id, prioritize_queue_node_id, remove_queue_node_id, consensus_submission_data) =
            prepare_accepted_queue_mutations::<T>(q, true);

        #[block]
        {
            Network::<T>::handle_node_queue_consensus(
                &mut WeightMeter::new(),
                subnet_id,
                &consensus_submission_data,
                Network::<T>::percentage_factor_as_u128(),
            );
        }

        let queue_after = SubnetNodeQueue::<T>::get(subnet_id);
        assert_eq!(queue_after.len() as u32, q.saturating_sub(1));
        assert!(RegisteredSubnetNodesData::<T>::contains_key(
            subnet_id,
            remove_queue_node_id,
        ));
        assert!(PendingRegisteredNodeRemovals::<T>::get(subnet_id).contains(&remove_queue_node_id));
        if prioritize_queue_node_id != remove_queue_node_id {
            assert_eq!(queue_after.first().unwrap().id, prioritize_queue_node_id);
        }
    }

    /// Every scored historical node may be below the snapshotted minimum-weight threshold. The
    /// main accepted `h` model already measures its included-node increase; this component measures
    /// the additional decrease and reputation-update event for all `h` nodes.
    #[benchmark]
    fn emission_step_accepted_below_min_weight_reputation(
        h: Linear<{ MIN_CONSENSUS_VALIDATOR_IDENTITIES }, { MAX_SUBNET_NODES_BENCHMARK_DOMAIN }>,
    ) {
        MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            h,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        let percentage_factor = Network::<T>::percentage_factor_as_u128();
        let reputation_factor = percentage_factor.saturating_div(10);
        let starting_reputation = Network::<T>::increase_rep(
            percentage_factor.saturating_div(2),
            reputation_factor,
            None,
        );
        let expected_reputation =
            Network::<T>::decrease_rep(starting_reputation, reputation_factor, None);
        for subnet_node_id in 1..=h {
            SubnetNodeReputation::<T>::insert(subnet_id, subnet_node_id, starting_reputation);
        }

        #[block]
        {
            for subnet_node_id in 1..=h {
                Network::<T>::decrease_and_return_node_reputation(
                    subnet_id,
                    subnet_node_id,
                    starting_reputation,
                    reputation_factor,
                    None,
                );
            }
        }

        for subnet_node_id in 1..=h {
            assert_eq!(
                SubnetNodeReputation::<T>::get(subnet_id, subnet_node_id),
                Some(expected_reputation)
            );
        }
    }

    /// A validator identity may own several validator nodes in one subnet. With the protocol's
    /// minimum three eligible identities, one attesting node per identity gives 100% distinct-
    /// identity support and can carry all snapshotted stake weight, while every other historical
    /// validator node remains a non-attestor. Therefore the reachable maximum is `512 - 3 = 509`,
    /// not `512 * (1 - 87.5%) = 64`.
    #[benchmark]
    fn emission_step_accepted_non_attestor_reputation(
        a: Linear<1, { MAX_NON_ATTESTING_VALIDATORS_BENCHMARK_DOMAIN }>,
    ) {
        MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
        let minimum_identity_attestors = Network::<T>::min_identity_attestors_for_ratio(
            Network::<T>::MIN_CONSENSUS_VALIDATOR_IDENTITIES,
            T::SuperMajorityAttestationRatio::get(),
        );
        assert_eq!(
            minimum_identity_attestors,
            Network::<T>::MIN_CONSENSUS_VALIDATOR_IDENTITIES
        );
        let active_nodes = a.saturating_add(minimum_identity_attestors);
        assert!(active_nodes <= T::MaxSubnetNodesUpperBound::get());

        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            active_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        // Collapse the active set onto the minimum three validator identities. Nodes 1..=3 are
        // the attestors; every later node is a reachable non-attestor owned round-robin by one of
        // those same identities. Concentrating each identity's delegate-stake allocation on its
        // attesting node makes the three attestors carry the complete stake quorum as well.
        let percentage_factor = Network::<T>::percentage_factor_as_u128();
        for validator_id in 1..=active_nodes {
            ValidatorSubnetNodes::<T>::remove(validator_id);
            TotalValidatorNodes::<T>::remove(validator_id);
            ValidatorNodeDelegateStakeWeights::<T>::remove(validator_id);
        }
        for subnet_node_id in 1..=active_nodes {
            let validator_id = subnet_node_id
                .saturating_sub(1)
                .wrapping_rem(minimum_identity_attestors)
                .saturating_add(1);
            SubnetNodeValidatorId::<T>::insert(subnet_id, subnet_node_id, validator_id);
            SubnetNodesData::<T>::mutate(subnet_id, subnet_node_id, |node| {
                node.validator_id = validator_id;
            });
            ValidatorSubnetNodes::<T>::mutate(validator_id, |ownership| {
                ownership
                    .entry(subnet_id)
                    .or_insert_with(BTreeSet::new)
                    .insert(subnet_node_id);
            });
            TotalValidatorNodes::<T>::mutate(validator_id, |count| {
                *count = count.saturating_add(1)
            });
            ValidatorNodeDelegateStakeWeights::<T>::mutate(validator_id, |weights| {
                weights.insert(
                    (subnet_id, subnet_node_id),
                    if subnet_node_id == validator_id {
                        percentage_factor
                    } else {
                        0
                    },
                );
            });
        }
        for validator_id in 1..=minimum_identity_attestors {
            ValidatorDelegateStakeBalance::<T>::insert(validator_id, DEFAULT_SUBNET_NODE_STAKE);
            ValidatorDelegateStakeShares::<T>::insert(validator_id, DEFAULT_SUBNET_NODE_STAKE);
        }
        TotalValidatorDelegateStakeBalance::<T>::set(
            DEFAULT_SUBNET_NODE_STAKE.saturating_mul(minimum_identity_attestors as u128),
        );

        let eligible_validator_identities: BTreeSet<u32> = (1..=active_nodes)
            .filter_map(|subnet_node_id| SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id))
            .collect();
        let attesting_validator_identities: BTreeSet<u32> = (1..=minimum_identity_attestors)
            .filter_map(|subnet_node_id| SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id))
            .collect();
        assert_eq!(
            eligible_validator_identities.len() as u32,
            minimum_identity_attestors
        );
        assert_eq!(
            attesting_validator_identities,
            eligible_validator_identities
        );
        assert!(
            Network::<T>::percent_div(
                attesting_validator_identities.len() as u128,
                eligible_validator_identities.len() as u128,
            ) >= T::SuperMajorityAttestationRatio::get()
        );

        let reputation_factor = percentage_factor.saturating_div(10);
        let after_included_increase = Network::<T>::increase_rep(
            percentage_factor.saturating_div(2),
            reputation_factor,
            None,
        );
        let starting_reputation =
            Network::<T>::decrease_rep(after_included_increase, reputation_factor, None);
        let expected_reputation =
            Network::<T>::decrease_rep(starting_reputation, reputation_factor, None);
        for subnet_node_id in minimum_identity_attestors.saturating_add(1)..=active_nodes {
            SubnetNodeReputation::<T>::insert(subnet_id, subnet_node_id, starting_reputation);
        }

        #[block]
        {
            for subnet_node_id in minimum_identity_attestors.saturating_add(1)..=active_nodes {
                Network::<T>::decrease_and_return_node_reputation(
                    subnet_id,
                    subnet_node_id,
                    starting_reputation,
                    reputation_factor,
                    None,
                );
            }
        }

        for subnet_node_id in minimum_identity_attestors.saturating_add(1)..=active_nodes {
            assert_eq!(
                SubnetNodeReputation::<T>::get(subnet_id, subnet_node_id),
                Some(expected_reputation)
            );
        }
    }

    /// Ready-queue activation and burn maintenance in a reachable preparation-phase state. The
    /// subnet has exactly `MaxSubnetNodes - q` live nodes and `q` registered nodes, so every queued
    /// node can be activated without exceeding any configured node bound. Election is deliberately
    /// unavailable until the following subnet epoch and is composed separately in the hook.
    #[benchmark]
    fn emission_step_queue(q: Linear<1, { MAX_REGISTERED_NODES_BENCHMARK_DOMAIN }>) {
        let max_nodes = T::MaxSubnetNodesUpperBound::get();
        assert!(q <= T::MaxRegisteredNodesUpperBound::get());
        assert!(q < max_nodes);

        NewRegistrationCostMultiplier::<T>::set(1_000_000_000_000_000_000);
        MaxSubnetNodes::<T>::set(max_nodes);
        let active_nodes = max_nodes.saturating_sub(q);
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            active_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        MaxRegisteredNodes::<T>::insert(subnet_id, q);
        ChurnLimit::<T>::insert(subnet_id, q);
        ChurnLimitMultiplier::<T>::insert(subnet_id, 1);
        SubnetNodeQueueEpochs::<T>::insert(subnet_id, 0);
        build_registered_subnet_nodes::<T>(
            subnet_id,
            active_nodes,
            max_nodes,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
            true,
        );
        max_fill_emission_queue::<T>(subnet_id);

        // Active-subnet registrations start at the following local epoch, and the queue waiting
        // predicate treats that boundary epoch as still waiting even when the configured delay is
        // zero. Advance two whole epochs so every entry is genuinely ready (`start < current`).
        increase_epochs::<T>(2);
        let current_epoch = Network::<T>::get_current_epoch_as_u32();
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
        max_fill_benchmark_subnet_data::<T>(subnet_id);
        SubnetsData::<T>::mutate(subnet_id, |maybe_subnet| {
            let subnet = maybe_subnet.as_mut().expect("benchmark subnet must exist");
            subnet.state = SubnetState::Active;
            subnet.pause = None;
            subnet.consensus_eligible_from_subnet_epoch =
                Some(current_subnet_epoch.saturating_add(1));
        });

        // No historical allocation is available in this preparation epoch, so the timed call
        // executes only the operational queue and fixed burn-maintenance paths.
        FinalSubnetEmissionWeights::<T>::remove(current_epoch);
        assert_eq!(TotalActiveSubnetNodes::<T>::get(subnet_id), active_nodes);
        assert_eq!(TotalSubnetNodes::<T>::get(subnet_id), max_nodes);
        assert_eq!(SubnetNodeQueue::<T>::get(subnet_id).len() as u32, q);

        #[block]
        {
            // Measure the independently admitted maintenance envelope directly. Calling the
            // whole emission step here makes this benchmark depend circularly on its currently
            // checked-in weight and can cause maintenance to be skipped before regeneration.
            let mut maintenance_meter = WeightMeter::new();
            Network::<T>::handle_registration_queue(
                &mut maintenance_meter,
                subnet_id,
                current_subnet_epoch,
            );
            Network::<T>::update_burn_rate_for_epoch(&mut maintenance_meter, subnet_id);
        }

        assert_eq!(TotalActiveSubnetNodes::<T>::get(subnet_id), max_nodes);
        assert_eq!(TotalSubnetNodes::<T>::get(subnet_id), max_nodes);
        assert!(SubnetNodeQueue::<T>::get(subnet_id).is_empty());
        assert_eq!(NodeRegistrationsThisEpoch::<T>::get(subnet_id), 0);
    }

    /// Missing proposals decode the maximum elected-round snapshot and execute both validator
    /// node-stake and delegate-pool economic slashes while the paused subnet retains a coherent
    /// maximum-size live/electable set.
    #[benchmark]
    fn emission_step_missing() {
        let h = T::MaxSubnetNodesUpperBound::get();
        let context = prepare_alternate_emission_step::<T>(h, AlternateEmissionMode::Missing);

        #[block]
        {
            Network::<T>::emission_settlement_step(
                &mut WeightMeter::new(),
                Network::<T>::get_current_block_as_u32(),
                context.current_epoch,
                context.current_subnet_epoch,
                context.subnet_id,
            );
        }

        assert!(!SubnetConsensusSubmission::<T>::contains_key(
            context.subnet_id,
            context.previous_subnet_epoch,
        ));
        assert!(
            ValidatorDelegateStakeBalance::<T>::get(context.validator_id)
                < context.initial_delegate_pool
        );
        assert!(SubnetElectedValidator::<T>::contains_key(
            context.subnet_id,
            context.previous_subnet_epoch,
        ));
        assert_eq!(
            TotalActiveSubnetNodes::<T>::get(context.subnet_id),
            context.historical_nodes,
        );
        assert_eq!(
            TotalSubnetNodes::<T>::get(context.subnet_id),
            context.historical_nodes,
        );
        assert!(SubnetNodeQueue::<T>::get(context.subnet_id).is_empty());
    }

    /// Strong rejection retains every eligible identity but omits one attestation, maximizing the
    /// accountable attestor prefix while remaining below the 100% snapshotted threshold. Every
    /// penalized attestor crosses the reputation boundary and is written to the bounded active
    /// quarantine set. Settlement deliberately stops before independently metered physical cleanup.
    #[benchmark]
    fn emission_step_rejected(
        h: Linear<{ MIN_CONSENSUS_VALIDATOR_IDENTITIES }, { MAX_SUBNET_NODES_BENCHMARK_DOMAIN }>,
    ) {
        let context = prepare_alternate_emission_step::<T>(h, AlternateEmissionMode::Rejected);

        #[block]
        {
            Network::<T>::emission_settlement_step(
                &mut WeightMeter::new(),
                Network::<T>::get_current_block_as_u32(),
                context.current_epoch,
                context.current_subnet_epoch,
                context.subnet_id,
            );
        }

        let pending = PendingActiveNodeRemovals::<T>::get(context.subnet_id);
        assert_eq!(pending.len() as u32, h.saturating_sub(1));
        for subnet_node_id in 1..h {
            assert!(pending.contains(&subnet_node_id));
            assert!(SubnetNodeReputation::<T>::contains_key(
                context.subnet_id,
                subnet_node_id,
            ));
        }
        assert!(SubnetNodeReputation::<T>::contains_key(
            context.subnet_id,
            h,
        ));
        assert!(SubnetElectedValidator::<T>::contains_key(
            context.subnet_id,
            context.previous_subnet_epoch,
        ));
        assert_eq!(
            TotalActiveSubnetNodes::<T>::get(context.subnet_id),
            context.historical_nodes,
        );
        assert_eq!(
            TotalSubnetNodes::<T>::get(context.subnet_id),
            context.historical_nodes,
        );
        assert!(SubnetNodeQueue::<T>::get(context.subnet_id).is_empty());
    }

    /// An accepted emergency round snapshots and scans all 64 emergency validators, completes the
    /// emergency lifecycle, and still settles up to 512 coherent historical/live reward nodes.
    #[benchmark]
    fn emission_step_emergency(
        h: Linear<{ ACTIVE_REMOVAL_ELECTION_MODEL_SPLIT }, { MAX_SUBNET_NODES_BENCHMARK_DOMAIN }>,
    ) {
        let context = prepare_alternate_emission_step::<T>(h, AlternateEmissionMode::Emergency);

        #[block]
        {
            Network::<T>::emission_settlement_step(
                &mut WeightMeter::new(),
                Network::<T>::get_current_block_as_u32(),
                context.current_epoch,
                context.current_subnet_epoch,
                context.subnet_id,
            );
        }

        assert!(!EmergencySubnetNodeElectionData::<T>::contains_key(
            context.subnet_id,
        ));
        assert!(SubnetElectedValidator::<T>::contains_key(
            context.subnet_id,
            context.previous_subnet_epoch,
        ));
        let pending = PendingActiveNodeRemovals::<T>::get(context.subnet_id);
        assert_eq!(pending.len() as u32, context.historical_nodes);
        assert_eq!(
            TotalActiveSubnetNodes::<T>::get(context.subnet_id),
            context.historical_nodes,
        );
        assert_eq!(
            TotalSubnetNodes::<T>::get(context.subnet_id),
            context.historical_nodes,
        );
        assert!(SubnetNodeQueue::<T>::get(context.subnet_id).is_empty());
    }

    // Informational purposes only
    #[benchmark]
    fn precheck_subnet_consensus_submission(
        x: Linear<{ MIN_CONSENSUS_VALIDATOR_IDENTITIES }, { MAX_SUBNET_NODES_BENCHMARK_DOMAIN }>,
    ) {
        MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            x,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();

        increase_epochs::<T>(1);

        let epoch = Network::<T>::get_current_epoch_as_u32();
        let subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
        let benchmark_round = ElectedConsensusRound {
            validator_subnet_node_id: 1,
            validator_id: SubnetNodeValidatorId::<T>::get(subnet_id, 1).unwrap_or_default(),
            emergency: None,
            eligible_subnet_node_ids: (1..=x).collect(),
            eligible_validator_identity_ids: (1..=x)
                .filter_map(|node_id| {
                    SubnetNodeValidatorId::<T>::get(subnet_id, node_id)
                        .map(|validator_id| (node_id, validator_id))
                })
                .collect(),
            validator_delegate_stake_balance: 0,
            policy: Network::<T>::consensus_policy_snapshot(
                subnet_id,
                subnet_epoch.saturating_sub(1),
            ),
        };
        SubnetElectedValidator::<T>::insert(
            subnet_id,
            subnet_epoch.saturating_sub(1),
            benchmark_round.clone(),
        );

        // Emission allocation resolves the subnet epoch at the epoch's allocation block rather
        // than from the benchmark's current block. Seed that exact round independently from the
        // previous round consumed by the precheck below.
        let allocation_block = epoch
            .saturating_mul(T::EpochLength::get())
            .saturating_add(NETWORK_SUBNET_EMISSION_SLOT);
        let allocation_subnet_epoch =
            Network::<T>::get_subnet_epoch_with_block_as_u32(subnet_id, allocation_block);
        SubnetElectedValidator::<T>::insert(subnet_id, allocation_subnet_epoch, benchmark_round);

        // ⸺ Generate subnet weights
        let _ = Network::<T>::handle_subnet_emission_weights(epoch);
        let subnet_emission_weights = FinalSubnetEmissionWeights::<T>::get(epoch);

        let subnet_weight = subnet_emission_weights.subnet_weights.get(&subnet_id);
        assert!(subnet_weight.is_some());

        // ⸺ Submit consnesus data
        let subnet_nodes: Vec<SubnetNode<T>> = Network::<T>::get_active_classified_subnet_nodes(
            subnet_id,
            &SubnetNodeClass::Included,
            epoch,
        );
        let subnet_node_count = subnet_nodes.len() as u128;

        let mut consensus_data =
            get_simulated_consensus_data::<T>(subnet_id, subnet_node_count as u32);
        let emergency_node_count = x.min(T::MaxEmergencySubnetNodesUpperBound::get());
        let emergency_node_ids: Vec<u32> = (1..=emergency_node_count).collect();
        let emergency_data = EmergencySubnetValidatorData {
            subnet_node_ids: emergency_node_ids.clone(),
            reputation_factors: Network::<T>::consensus_policy_snapshot(
                subnet_id,
                subnet_epoch.saturating_sub(1),
            )
            .reputation_factors,
            min_subnet_node_reputation: u128::MAX,
            min_weight_decrease_reputation_threshold: u128::MAX,
            ..Default::default()
        };
        consensus_data.emergency = Some(Network::<T>::emergency_consensus_snapshot(
            &emergency_data,
            emergency_node_ids,
        ));

        let current_epoch = Network::<T>::get_current_epoch_as_u32();

        // submit data for the previous epoch
        let validator_ids = Network::<T>::canonicalize_consensus_validator_ids(
            consensus_data.validator_ids.clone(),
        );
        let snapshot = Network::<T>::snapshot_consensus_attestor_weights(
            subnet_id,
            subnet_epoch - 1,
            &validator_ids,
        )
        .unwrap();
        SubnetConsensusSubmission::<T>::insert(subnet_id, subnet_epoch - 1, consensus_data);
        SubnetConsensusSubmissionMaxItems::<T>::insert(subnet_id, subnet_epoch - 1, x);
        SubnetConsensusAttestorWeights::<T>::insert(subnet_id, subnet_epoch - 1, snapshot);
        let stored_submission =
            SubnetConsensusSubmission::<T>::get(subnet_id, subnet_epoch - 1).unwrap();
        assert_eq!(stored_submission.validator_ids.len() as u32, x);
        assert_eq!(stored_submission.validator_identity_ids.len() as u32, x);
        assert_eq!(stored_submission.attests.len() as u32, x);
        assert_eq!(stored_submission.subnet_nodes.len() as u32, x);
        assert_eq!(stored_submission.data.len() as u32, x);
        assert_eq!(
            stored_submission
                .emergency
                .as_ref()
                .expect("success benchmark includes the emergency settlement snapshot")
                .subnet_node_ids
                .len() as u32,
            emergency_node_count,
        );
        assert_eq!(
            SubnetConsensusAttestorWeights::<T>::get(subnet_id, subnet_epoch - 1)
                .unwrap()
                .weights
                .len() as u32,
            x,
        );

        #[block]
        {
            let (result, weight) = Network::<T>::precheck_subnet_consensus_submission(
                subnet_id,
                subnet_epoch - 1,
                current_epoch,
            );

            // assert SubnetConsensusSubmission exists
            assert!(result.is_some(), "Precheck consensus failed");
        }

        let rep = SubnetReputation::<T>::get(subnet_id);
        assert_eq!(rep, Network::<T>::percentage_factor_as_u128());
    }

    #[benchmark]
    fn precheck_subnet_consensus_submission_missing() {
        // The missing-proposal path still decodes the full elected-round snapshot. Max-fill both
        // candidate collections so the fixed weight covers the largest proof at h=0.
        let x = T::MaxSubnetNodesUpperBound::get();
        MaxSubnetNodes::<T>::set(T::MaxSubnetNodesUpperBound::get());
        build_activated_subnet::<T>(
            DEFAULT_SUBNET_NAME.into(),
            0,
            x,
            DEFAULT_DEPOSIT_AMOUNT,
            DEFAULT_SUBNET_NODE_STAKE,
        );
        let subnet_id = SubnetName::<T>::get::<Vec<u8>>(DEFAULT_SUBNET_NAME.into()).unwrap();
        increase_epochs::<T>(1);
        let current_epoch = Network::<T>::get_current_epoch_as_u32();
        let current_subnet_epoch = Network::<T>::get_current_subnet_epoch_as_u32(subnet_id);
        let previous_subnet_epoch = current_subnet_epoch.saturating_sub(1);
        let validator_subnet_node_id = 1;
        let validator_id =
            SubnetNodeValidatorId::<T>::get(subnet_id, validator_subnet_node_id).unwrap();
        let percentage_factor = Network::<T>::percentage_factor_as_u128();
        let delegate_pool_balance = DEFAULT_DEPOSIT_AMOUNT;
        ValidatorDelegateStakeSlashThreshold::<T>::set(percentage_factor);
        BaseValidatorDelegateStakeSlashPercentage::<T>::set(percentage_factor / 2);
        MaxValidatorDelegateStakeSlashAmount::<T>::set(delegate_pool_balance);
        ValidatorDelegateStakeBalance::<T>::insert(validator_id, delegate_pool_balance);
        ValidatorDelegateStakeShares::<T>::insert(validator_id, delegate_pool_balance);
        TotalValidatorDelegateStakeBalance::<T>::set(delegate_pool_balance);
        SubnetElectedValidator::<T>::insert(
            subnet_id,
            previous_subnet_epoch,
            ElectedConsensusRound {
                validator_subnet_node_id,
                validator_id,
                emergency: None,
                eligible_subnet_node_ids: (1..=x).collect(),
                eligible_validator_identity_ids: (1..=x)
                    .filter_map(|node_id| {
                        SubnetNodeValidatorId::<T>::get(subnet_id, node_id)
                            .map(|identity_id| (node_id, identity_id))
                    })
                    .collect(),
                validator_delegate_stake_balance: delegate_pool_balance,
                policy: Network::<T>::consensus_policy_snapshot(subnet_id, previous_subnet_epoch),
            },
        );
        let stored_round =
            SubnetElectedValidator::<T>::get(subnet_id, previous_subnet_epoch).unwrap();
        assert_eq!(stored_round.eligible_subnet_node_ids.len() as u32, x);
        assert_eq!(stored_round.eligible_validator_identity_ids.len() as u32, x);
        assert_eq!(
            stored_round.validator_delegate_stake_balance,
            delegate_pool_balance
        );
        assert!(
            stored_round
                .policy
                .base_validator_delegate_stake_slash_percentage
                > 0
        );
        SubnetConsensusSubmission::<T>::remove(subnet_id, previous_subnet_epoch);

        #[block]
        {
            let (result, _) = Network::<T>::precheck_subnet_consensus_submission(
                subnet_id,
                previous_subnet_epoch,
                current_epoch,
            );
            assert!(result.is_none());
        }
        assert!(ValidatorDelegateStakeBalance::<T>::get(validator_id) < delegate_pool_balance);
    }

    // Informational purposes only
    // x is capped by available subnet slots: EpochLength - DesignatedEpochSlots.
    #[benchmark]
    fn calculate_subnet_weights(x: Linear<0, { MAX_PHYSICAL_SUBNETS_BENCHMARK_DOMAIN }>) {
        // Activate subnets
        let end = MinSubnetNodes::<T>::get();
        NewRegistrationCostMultiplier::<T>::set(1000000000000000000);
        for s in 0..x {
            let path: Vec<u8> = format!("subnet-name-{s}").into();
            build_activated_subnet::<T>(
                path,
                0,
                end,
                DEFAULT_DEPOSIT_AMOUNT,
                DEFAULT_SUBNET_NODE_STAKE,
            );
        }

        increase_epochs::<T>(1);
        let epoch = Network::<T>::get_current_epoch_as_u32();

        let current_overwatch_epoch = Network::<T>::get_current_overwatch_epoch_as_u32();
        LastFinalizedOverwatchEpoch::<T>::put(current_overwatch_epoch);
        let max_historical_nodes = T::MaxSubnetNodesUpperBound::get();
        let mut effective_subnet_weights = BTreeMap::new();

        // Simulate overwatch subnet weights
        for s in 0..x {
            let path: Vec<u8> = format!("subnet-name-{s}").into();
            let subnet_id = SubnetName::<T>::get::<Vec<u8>>(path.clone().into()).unwrap();

            effective_subnet_weights.insert(subnet_id, 500000000000000000);
            let magnitude = (s as i128 + 1).saturating_mul(1_000_000);
            SubnetNetFlow::<T>::insert(subnet_id, if s % 2 == 0 { -magnitude } else { magnitude });
            SubnetNetFlowSmoothedWeight::<T>::insert(
                subnet_id,
                Network::<T>::percentage_factor_as_u128()
                    .saturating_div(s as u128 + 2)
                    .max(1),
            );
            SubnetElectedValidator::<T>::insert(
                subnet_id,
                epoch.saturating_sub(1),
                ElectedConsensusRound {
                    validator_subnet_node_id: 1,
                    validator_id: SubnetNodeValidatorId::<T>::get(subnet_id, 1).unwrap_or_default(),
                    emergency: None,
                    eligible_subnet_node_ids: (1..=max_historical_nodes).collect(),
                    eligible_validator_identity_ids: (1..=max_historical_nodes)
                        .map(|node_id| (node_id, node_id))
                        .collect(),
                    validator_delegate_stake_balance: 0,
                    policy: Network::<T>::consensus_policy_snapshot(
                        subnet_id,
                        epoch.saturating_sub(1),
                    ),
                },
            );
        }
        seed_max_effective_overwatch_cache::<T>(current_overwatch_epoch, effective_subnet_weights);

        #[block]
        {
            let (stake_weights_normalized, stake_weights_weight) =
                Network::<T>::calculate_subnet_weights(epoch);
            assert!(stake_weights_normalized.len() as u32 == x);
        }
    }

    impl_benchmark_test_suite!(Network, tests::mock::new_test_ext(), tests::mock::Test);
}
