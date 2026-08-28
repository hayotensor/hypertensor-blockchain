use super::mock::*;
use crate::tests::test_utils::*;
use crate::{
    AccountSubnetDelegateStakeShares, AccountValidatorDelegateStakeShares, DelegateAccountStake,
    DelegateStakeCooldownEpochs, Error, MaxSubnetNodes, MaxSubnets, MaxUnbondings,
    MinSubnetMinStake, NodeSubnetStake, OverwatchMinStakeBalance, OverwatchNodeStakeBalance,
    PeerInfo, RegisteredSubnetNodesData, StakeCooldownEpochs, StakeUnbondingLedger, SubnetName,
    SubnetNodeQueueEpochs, SubnetState, TotalAccountDelegateStake, TotalActiveSubnets,
    TotalDelegateStake, TotalNetworkUnbondingBalance, TotalOverwatchNodeStakeBalance, TotalStake,
    TotalSubnetDelegateStakeBalance, TotalSubnetDelegateStakeShares, TotalSubnetNodeUids,
    TotalSubnetNodes, TotalSubnetStake, TotalValidatorDelegateStakeBalance, TotalValidatorIds,
    TxRateLimit, UnbondingEntry, ValidatorDelegateStakeBalance, ValidatorDelegateStakeShares,
    ValidatorSubnetNodes,
};
use frame_support::traits::Currency;
use frame_support::weights::WeightMeter;
use frame_support::{assert_err, assert_ok};
use sp_runtime::ArithmeticError;
use sp_std::collections::btree_map::BTreeMap;

fn set_full_unbonding_ledger(
    account_id: &AccountIdOf<Test>,
    first_claim_block: u32,
) -> BTreeMap<u32, UnbondingEntry> {
    let mut ledger = BTreeMap::new();
    for n in 0..MaxUnbondings::<Test>::get() {
        ledger.insert(
            first_claim_block.saturating_add(n),
            UnbondingEntry {
                network: 100 + n as u128,
                overwatch: 0,
            },
        );
    }
    let total = ledger.values().map(|entry| entry.network).sum();
    StakeUnbondingLedger::<Test>::insert(account_id, ledger.clone());
    TotalNetworkUnbondingBalance::<Test>::set(total);
    ledger
}

#[test]
fn test_mixed_unbonding_entry_counts_only_network_principal() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        let account_id = account(899);
        let network_amount = 70;
        let overwatch_amount = 30;
        let cooldown_blocks = 5;
        let claim_block = System::block_number() + cooldown_blocks;
        let _ = Balances::deposit_creating(&account_id, EXISTENTIAL_DEPOSIT);
        let balance_before = Balances::free_balance(&account_id);

        assert_ok!(Network::add_balance_to_unbonding_ledger(
            &account_id,
            network_amount,
            cooldown_blocks,
            System::block_number(),
            crate::UnbondingSource::Network,
        ));
        assert_ok!(Network::add_balance_to_unbonding_ledger(
            &account_id,
            overwatch_amount,
            cooldown_blocks,
            System::block_number(),
            crate::UnbondingSource::Overwatch,
        ));

        assert_eq!(
            StakeUnbondingLedger::<Test>::get(&account_id).get(&claim_block),
            Some(&UnbondingEntry {
                network: network_amount,
                overwatch: overwatch_amount,
            })
        );
        assert_eq!(TotalNetworkUnbondingBalance::<Test>::get(), network_amount);

        System::set_block_number(claim_block);
        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            account_id.clone()
        )));

        assert!(StakeUnbondingLedger::<Test>::get(&account_id).is_empty());
        assert_eq!(TotalNetworkUnbondingBalance::<Test>::get(), 0);
        assert_eq!(
            Balances::free_balance(&account_id),
            balance_before + network_amount + overwatch_amount
        );
    });
}

#[test]
fn test_network_stake_unbond_and_claim_move_tvl_exactly_once() {
    new_test_ext().execute_with(|| {
        const SUBNET_ID: u32 = 1;
        const STAKE: u128 = 10_000;
        let staker = account(892);
        insert_subnet(SUBNET_ID, SubnetState::Active, 0);
        let _ = Balances::deposit_creating(&staker, STAKE + EXISTENTIAL_DEPOSIT);
        System::set_block_number(TxRateLimit::<Test>::get() + 1);

        let tvl_before_stake = Network::get_total_network_tvl();
        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(staker.clone()),
            SUBNET_ID,
            STAKE,
        ));
        assert_eq!(Network::get_total_network_tvl(), tvl_before_stake + STAKE);

        System::set_block_number(
            System::block_number()
                .saturating_add(TxRateLimit::<Test>::get())
                .saturating_add(1),
        );
        let shares = AccountSubnetDelegateStakeShares::<Test>::get(&staker, SUBNET_ID);
        let tvl_before_unbonding = Network::get_total_network_tvl();
        assert_ok!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(staker.clone()),
            SUBNET_ID,
            shares,
        ));

        let ledger = StakeUnbondingLedger::<Test>::get(&staker);
        let (&claim_block, entry) = ledger.iter().next().expect("unstake creates one entry");
        assert!(entry.network > 0);
        assert_eq!(entry.overwatch, 0);
        assert_eq!(Network::get_total_network_tvl(), tvl_before_unbonding);

        System::set_block_number(claim_block);
        let wallet_before_claim = Balances::free_balance(&staker);
        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            staker.clone()
        )));
        assert_eq!(
            Network::get_total_network_tvl(),
            tvl_before_unbonding - entry.network
        );
        assert_eq!(
            Balances::free_balance(&staker),
            wallet_before_claim + entry.network
        );
    });
}

#[test]
fn test_node_rewards_and_slashes_change_tvl_once() {
    new_test_ext().execute_with(|| {
        const SUBNET_ID: u32 = 1;
        const NODE_ID: u32 = 1;
        let tvl_before = Network::get_total_network_tvl();

        Network::increase_node_stake(NODE_ID, SUBNET_ID, 100);
        assert_eq!(Network::get_total_network_tvl(), tvl_before + 100);

        // Node rewards use the same canonical stake increase path.
        Network::increase_node_stake(NODE_ID, SUBNET_ID, 25);
        assert_eq!(Network::get_total_network_tvl(), tvl_before + 125);

        // Economic slashing uses the canonical stake decrease path.
        Network::decrease_node_stake(NODE_ID, SUBNET_ID, 40);
        assert_eq!(Network::get_total_network_tvl(), tvl_before + 85);
    });
}

#[test]
fn test_claim_retains_principal_when_reaped_wallet_is_below_existential_deposit() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        let account_id = account(893);
        let entry = UnbondingEntry {
            network: 100,
            overwatch: 0,
        };
        let ledger = BTreeMap::from([(System::block_number(), entry)]);
        StakeUnbondingLedger::<Test>::insert(&account_id, ledger.clone());
        TotalNetworkUnbondingBalance::<Test>::set(entry.network);

        assert_eq!(Network::do_claim_unbondings(&account_id), 0);
        assert_eq!(StakeUnbondingLedger::<Test>::get(&account_id), ledger);
        assert_eq!(TotalNetworkUnbondingBalance::<Test>::get(), entry.network);

        let _ = Balances::deposit_creating(&account_id, EXISTENTIAL_DEPOSIT);
        let balance_before = Balances::free_balance(&account_id);
        assert_eq!(Network::do_claim_unbondings(&account_id), 1);
        assert!(StakeUnbondingLedger::<Test>::get(&account_id).is_empty());
        assert_eq!(TotalNetworkUnbondingBalance::<Test>::get(), 0);
        assert_eq!(
            Balances::free_balance(&account_id),
            balance_before + entry.network
        );
    });
}

#[test]
fn test_claim_retains_principal_when_wallet_credit_would_overflow() {
    new_test_ext().execute_with(|| {
        let account_id = account(891);
        let entry = UnbondingEntry {
            network: 100,
            overwatch: 25,
        };
        let ledger = BTreeMap::from([(System::block_number(), entry)]);
        let _ = Balances::make_free_balance_be(&account_id, u128::MAX - 10);
        let wallet_before = Balances::free_balance(&account_id);
        StakeUnbondingLedger::<Test>::insert(&account_id, ledger.clone());
        TotalNetworkUnbondingBalance::<Test>::set(entry.network);

        assert_eq!(Network::do_claim_unbondings(&account_id), 0);
        assert_eq!(StakeUnbondingLedger::<Test>::get(&account_id), ledger);
        assert_eq!(TotalNetworkUnbondingBalance::<Test>::get(), entry.network);
        assert_eq!(Balances::free_balance(&account_id), wallet_before);
    });
}

#[test]
fn test_claim_retains_complete_entry_when_network_accounting_is_inconsistent() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        let account_id = account(898);
        let entry = UnbondingEntry {
            network: 70,
            overwatch: 30,
        };
        let ledger = BTreeMap::from([(System::block_number(), entry.clone())]);
        StakeUnbondingLedger::<Test>::insert(&account_id, ledger.clone());
        TotalNetworkUnbondingBalance::<Test>::set(entry.network - 1);
        let balance_before = Balances::free_balance(&account_id);

        assert_eq!(Network::do_claim_unbondings(&account_id), 0);
        assert_eq!(StakeUnbondingLedger::<Test>::get(&account_id), ledger);
        assert_eq!(
            TotalNetworkUnbondingBalance::<Test>::get(),
            entry.network - 1
        );
        assert_eq!(Balances::free_balance(&account_id), balance_before);
    });
}

#[test]
fn test_claim_retains_complete_entry_when_mixed_principal_overflows() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        let account_id = account(894);
        let entry = UnbondingEntry {
            network: u128::MAX,
            overwatch: 1,
        };
        let ledger = BTreeMap::from([(System::block_number(), entry)]);
        StakeUnbondingLedger::<Test>::insert(&account_id, ledger.clone());
        TotalNetworkUnbondingBalance::<Test>::set(u128::MAX);
        let balance_before = Balances::free_balance(&account_id);

        assert_eq!(Network::do_claim_unbondings(&account_id), 0);
        assert_eq!(StakeUnbondingLedger::<Test>::get(&account_id), ledger);
        assert_eq!(TotalNetworkUnbondingBalance::<Test>::get(), u128::MAX);
        assert_eq!(Balances::free_balance(&account_id), balance_before);
    });
}

#[test]
fn test_network_unbonding_overflow_rolls_back_delegate_account_debit() {
    new_test_ext().execute_with(|| {
        let account_id = account(897);
        Network::increase_delegate_account_balance(&account_id, 100);
        TotalNetworkUnbondingBalance::<Test>::set(u128::MAX);

        assert_err!(
            Network::remove_delegate_account_balance(
                RuntimeOrigin::signed(account_id.clone()),
                25,
            ),
            ArithmeticError::Overflow
        );

        assert_eq!(DelegateAccountStake::<Test>::get(&account_id), 100);
        assert_eq!(TotalAccountDelegateStake::<Test>::get(), 100);
        assert!(StakeUnbondingLedger::<Test>::get(&account_id).is_empty());
        assert_eq!(TotalNetworkUnbondingBalance::<Test>::get(), u128::MAX);
    });
}

#[test]
fn test_overwatch_unbonding_never_enters_network_tvl() {
    new_test_ext().execute_with(|| {
        let validator_id = 1;
        manual_insert_validator(validator_id, 895, 896);
        let coldkey = account(895);
        let overwatch_node_id = insert_overwatch_node_v2(validator_id);
        let amount = 100;
        let stake = OverwatchMinStakeBalance::<Test>::get() + amount;
        set_overwatch_node_stake(overwatch_node_id, stake);
        let network_unbonding_before = TotalNetworkUnbondingBalance::<Test>::get();
        let _ = Balances::deposit_creating(&coldkey, EXISTENTIAL_DEPOSIT);
        let balance_before = Balances::free_balance(&coldkey);

        assert_ok!(Network::remove_overwatch_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            overwatch_node_id,
            amount,
        ));

        let ledger = StakeUnbondingLedger::<Test>::get(&coldkey);
        assert_eq!(ledger.len(), 1);
        let entry = ledger.values().next().unwrap();
        assert_eq!(entry.network, 0);
        assert_eq!(entry.overwatch, amount);
        assert_eq!(
            TotalNetworkUnbondingBalance::<Test>::get(),
            network_unbonding_before
        );

        increase_epochs(StakeCooldownEpochs::<Test>::get() + 1);
        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            coldkey.clone()
        )));
        assert_eq!(Balances::free_balance(&coldkey), balance_before + amount);
        assert_eq!(
            TotalNetworkUnbondingBalance::<Test>::get(),
            network_unbonding_before
        );
    });
}

#[test]
fn test_full_unbonding_ledger_blocks_subnet_delegate_unstake_without_debit() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-delegate-full-ledger".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let delegate_amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet(subnet_name.clone(), 0, 4, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();

        let delegate = account(900);
        let _ = Balances::deposit_creating(&delegate, delegate_amount + 500);
        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(delegate.clone()),
            subnet_id,
            delegate_amount,
        ));

        let shares = AccountSubnetDelegateStakeShares::<Test>::get(&delegate, subnet_id);
        let ledger =
            set_full_unbonding_ledger(&delegate, System::block_number().saturating_add(10_000));
        let total_unbonding = TotalNetworkUnbondingBalance::<Test>::get();
        let total_delegate_stake = TotalDelegateStake::<Test>::get();
        let total_subnet_delegate_balance = TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id);
        let total_subnet_delegate_shares = TotalSubnetDelegateStakeShares::<Test>::get(subnet_id);

        assert_err!(
            Network::remove_delegate_stake(
                RuntimeOrigin::signed(delegate.clone()),
                subnet_id,
                shares,
            ),
            Error::<Test>::MaxUnlockingsReached
        );

        assert_eq!(
            AccountSubnetDelegateStakeShares::<Test>::get(&delegate, subnet_id),
            shares
        );
        assert_eq!(TotalDelegateStake::<Test>::get(), total_delegate_stake);
        assert_eq!(
            TotalSubnetDelegateStakeBalance::<Test>::get(subnet_id),
            total_subnet_delegate_balance
        );
        assert_eq!(
            TotalSubnetDelegateStakeShares::<Test>::get(subnet_id),
            total_subnet_delegate_shares
        );
        assert_eq!(StakeUnbondingLedger::<Test>::get(&delegate), ledger);
        assert_eq!(TotalNetworkUnbondingBalance::<Test>::get(), total_unbonding);
    });
}

#[test]
fn test_full_unbonding_ledger_blocks_validator_delegate_unstake_without_debit() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "validator-delegate-full-ledger".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let delegate_amount: u128 = 1000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        build_activated_subnet_with_delegator_rewards(
            subnet_name,
            0,
            16,
            deposit_amount,
            stake_amount,
            DEFAULT_DELEGATE_REWARD_RATE,
        );

        let validator_id = 1;
        let delegate = account(901);
        let _ = Balances::deposit_creating(&delegate, delegate_amount + 500);
        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(delegate.clone()),
            validator_id,
            delegate_amount,
        ));

        let shares = AccountValidatorDelegateStakeShares::<Test>::get(&delegate, validator_id);
        let ledger =
            set_full_unbonding_ledger(&delegate, System::block_number().saturating_add(10_000));
        let total_unbonding = TotalNetworkUnbondingBalance::<Test>::get();
        let validator_delegate_balance = ValidatorDelegateStakeBalance::<Test>::get(validator_id);
        let validator_delegate_shares = ValidatorDelegateStakeShares::<Test>::get(validator_id);
        let total_validator_delegate_balance = TotalValidatorDelegateStakeBalance::<Test>::get();

        assert_err!(
            Network::remove_validator_delegate_stake(
                RuntimeOrigin::signed(delegate.clone()),
                validator_id,
                shares,
            ),
            Error::<Test>::MaxUnlockingsReached
        );

        assert_eq!(
            AccountValidatorDelegateStakeShares::<Test>::get(&delegate, validator_id),
            shares
        );
        assert_eq!(
            ValidatorDelegateStakeBalance::<Test>::get(validator_id),
            validator_delegate_balance
        );
        assert_eq!(
            ValidatorDelegateStakeShares::<Test>::get(validator_id),
            validator_delegate_shares
        );
        assert_eq!(
            TotalValidatorDelegateStakeBalance::<Test>::get(),
            total_validator_delegate_balance
        );
        assert_eq!(StakeUnbondingLedger::<Test>::get(&delegate), ledger);
        assert_eq!(TotalNetworkUnbondingBalance::<Test>::get(), total_unbonding);
    });
}

#[test]
fn test_full_unbonding_ledger_blocks_node_unstake_without_debit() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "node-full-ledger".into();
        let deposit_amount: u128 = 1000000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let node_stake: u128 = stake_amount.saturating_add(1000);

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name).unwrap();

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey, node_stake + burn_amount + 500);

        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            test_percent(1, 20),
            None,
            None,
        ));
        let validator_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            validator_id,
            subnet_id,
            None,
            Some(PeerInfo::<Test> {
                peer_id: peer(999),
                multiaddr: None,
            }),
            None,
            None,
            node_stake,
            None,
            None,
            u128::MAX,
        ));
        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        assert_ok!(Network::remove_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
        ));

        let ledger =
            set_full_unbonding_ledger(&coldkey, System::block_number().saturating_add(10_000));
        let total_unbonding = TotalNetworkUnbondingBalance::<Test>::get();
        let node_stake_balance = NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id);
        let total_subnet_stake = TotalSubnetStake::<Test>::get(subnet_id);
        let total_stake = TotalStake::<Test>::get();
        let validator_subnet_nodes = ValidatorSubnetNodes::<Test>::get(validator_id);

        assert_err!(
            Network::remove_node_stake(
                RuntimeOrigin::signed(coldkey.clone()),
                subnet_id,
                subnet_node_id,
                node_stake_balance,
            ),
            Error::<Test>::MaxUnlockingsReached
        );

        assert_eq!(
            NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id),
            node_stake_balance
        );
        assert_eq!(TotalSubnetStake::<Test>::get(subnet_id), total_subnet_stake);
        assert_eq!(TotalStake::<Test>::get(), total_stake);
        assert_eq!(
            ValidatorSubnetNodes::<Test>::get(validator_id),
            validator_subnet_nodes
        );
        assert_eq!(StakeUnbondingLedger::<Test>::get(&coldkey), ledger);
        assert_eq!(TotalNetworkUnbondingBalance::<Test>::get(), total_unbonding);
    });
}

#[test]
fn test_full_unbonding_ledger_blocks_delegate_account_balance_removal_without_debit() {
    new_test_ext().execute_with(|| {
        let account_id = account(902);
        Network::increase_delegate_account_balance(&account_id, 1000);

        let ledger =
            set_full_unbonding_ledger(&account_id, System::block_number().saturating_add(10_000));
        let total_unbonding = TotalNetworkUnbondingBalance::<Test>::get();
        let delegate_account_stake = DelegateAccountStake::<Test>::get(&account_id);
        let total_account_delegate_stake = TotalAccountDelegateStake::<Test>::get();

        assert_err!(
            Network::remove_delegate_account_balance(
                RuntimeOrigin::signed(account_id.clone()),
                100,
            ),
            Error::<Test>::MaxUnlockingsReached
        );

        assert_eq!(
            DelegateAccountStake::<Test>::get(&account_id),
            delegate_account_stake
        );
        assert_eq!(
            TotalAccountDelegateStake::<Test>::get(),
            total_account_delegate_stake
        );
        assert_eq!(StakeUnbondingLedger::<Test>::get(&account_id), ledger);
        assert_eq!(TotalNetworkUnbondingBalance::<Test>::get(), total_unbonding);
    });
}

#[test]
fn test_full_unbonding_ledger_blocks_overwatch_unstake_without_debit() {
    new_test_ext().execute_with(|| {
        let validator_id = 1;
        manual_insert_validator(validator_id, 903, 904);
        let coldkey = account(903);
        let overwatch_node_id = insert_overwatch_node_v2(validator_id);
        let stake = OverwatchMinStakeBalance::<Test>::get().saturating_add(1000);
        set_overwatch_node_stake(overwatch_node_id, stake);

        let ledger =
            set_full_unbonding_ledger(&coldkey, System::block_number().saturating_add(10_000));
        let total_unbonding = TotalNetworkUnbondingBalance::<Test>::get();
        let overwatch_stake = OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id);
        let total_overwatch_stake = TotalOverwatchNodeStakeBalance::<Test>::get();

        assert_err!(
            Network::remove_overwatch_node_stake(
                RuntimeOrigin::signed(coldkey.clone()),
                overwatch_node_id,
                100,
            ),
            Error::<Test>::MaxUnlockingsReached
        );

        assert_eq!(
            OverwatchNodeStakeBalance::<Test>::get(overwatch_node_id),
            overwatch_stake
        );
        assert_eq!(
            TotalOverwatchNodeStakeBalance::<Test>::get(),
            total_overwatch_stake
        );
        assert_eq!(StakeUnbondingLedger::<Test>::get(&coldkey), ledger);
        assert_eq!(TotalNetworkUnbondingBalance::<Test>::get(), total_unbonding);
    });
}

#[test]
fn test_full_unbonding_ledger_claims_matured_entry_before_delegate_account_removal() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);

        let account_id = account(904);
        let matured_amount = 77;
        let amount_to_remove = 25;
        Network::increase_delegate_account_balance(&account_id, 1000);
        let _ = Balances::deposit_creating(&account_id, EXISTENTIAL_DEPOSIT);
        let balance_before = Balances::free_balance(&account_id);

        let ledger_before = set_full_unbonding_ledger_with_entry(
            &account_id,
            System::block_number(),
            matured_amount,
        );
        let total_unbonding_before = TotalNetworkUnbondingBalance::<Test>::get();
        let stake_before = DelegateAccountStake::<Test>::get(&account_id);
        let claim_block = System::block_number()
            .saturating_add(StakeCooldownEpochs::<Test>::get() * EpochLength::get());

        assert_ok!(Network::remove_delegate_account_balance(
            RuntimeOrigin::signed(account_id.clone()),
            amount_to_remove,
        ));

        let ledger_after = StakeUnbondingLedger::<Test>::get(&account_id);
        assert_eq!(ledger_after.len() as u32, MaxUnbondings::<Test>::get());
        assert!(!ledger_after.contains_key(&System::block_number()));
        assert_eq!(
            ledger_after.get(&claim_block).unwrap().network,
            amount_to_remove
        );
        assert_eq!(ledger_after.get(&claim_block).unwrap().overwatch, 0);
        assert_eq!(
            DelegateAccountStake::<Test>::get(&account_id),
            stake_before - amount_to_remove
        );
        assert_eq!(
            Balances::free_balance(&account_id),
            balance_before + matured_amount
        );
        assert_eq!(
            TotalNetworkUnbondingBalance::<Test>::get(),
            total_unbonding_before - matured_amount + amount_to_remove
        );
        assert_eq!(ledger_before.len() as u32, MaxUnbondings::<Test>::get());
    });
}

#[test]
fn test_full_unbonding_ledger_merges_existing_claim_block_at_capacity() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);

        let account_id = account(905);
        let existing_amount = 50;
        let amount_to_remove = 25;
        Network::increase_delegate_account_balance(&account_id, 1000);

        let claim_block = System::block_number()
            .saturating_add(StakeCooldownEpochs::<Test>::get() * EpochLength::get());
        let ledger_before =
            set_full_unbonding_ledger_with_entry(&account_id, claim_block, existing_amount);
        let total_unbonding_before = TotalNetworkUnbondingBalance::<Test>::get();
        let stake_before = DelegateAccountStake::<Test>::get(&account_id);

        assert_ok!(Network::remove_delegate_account_balance(
            RuntimeOrigin::signed(account_id.clone()),
            amount_to_remove,
        ));

        let ledger_after = StakeUnbondingLedger::<Test>::get(&account_id);
        assert_eq!(ledger_after.len(), ledger_before.len());
        assert_eq!(
            ledger_after.get(&claim_block).unwrap().network,
            existing_amount + amount_to_remove
        );
        assert_eq!(ledger_after.get(&claim_block).unwrap().overwatch, 0);
        assert_eq!(
            DelegateAccountStake::<Test>::get(&account_id),
            stake_before - amount_to_remove
        );
        assert_eq!(
            TotalNetworkUnbondingBalance::<Test>::get(),
            total_unbonding_before + amount_to_remove
        );
    });
}

fn set_full_unbonding_ledger_with_entry(
    account_id: &AccountIdOf<Test>,
    claim_block: u32,
    amount: u128,
) -> BTreeMap<u32, UnbondingEntry> {
    let mut ledger = BTreeMap::new();
    ledger.insert(
        claim_block,
        UnbondingEntry {
            network: amount,
            overwatch: 0,
        },
    );

    let mut next_claim_block = System::block_number().saturating_add(10_000);
    while ledger.len() < MaxUnbondings::<Test>::get() as usize {
        if !ledger.contains_key(&next_claim_block) {
            ledger.insert(
                next_claim_block,
                UnbondingEntry {
                    network: 100 + ledger.len() as u128,
                    overwatch: 0,
                },
            );
        }
        next_claim_block = next_claim_block.saturating_add(1);
    }

    let total = ledger.values().map(|entry| entry.network).sum();
    StakeUnbondingLedger::<Test>::insert(account_id, ledger.clone());
    TotalNetworkUnbondingBalance::<Test>::set(total);
    ledger
}

///
///
///
///
///
///
///
/// Unbondings
///
///
///
///
///
///
///

#[test]
fn test_register_remove_claim_stake_unbondings() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 1000000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount + 500);

        let starting_balance = Balances::free_balance(&coldkey.clone());
        assert_eq!(starting_balance, deposit_amount + burn_amount + 500);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            current_id,
            subnet_id,
            None,
            Some(PeerInfo::<Test> {
                peer_id: peer(999),
                multiaddr: None,
            }),
            None,
            None,
            amount,
            None,
            None,
            u128::MAX,
        ));
        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let stake_balance = NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id);
        assert_eq!(stake_balance, amount);

        let after_stake_balance = Balances::free_balance(&coldkey.clone());
        assert_eq!(after_stake_balance, starting_balance - amount - burn_amount);

        assert_ok!(Network::remove_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
        ));

        let stake_balance = NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id);

        // remove amount ontop
        assert_ok!(Network::do_remove_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            // false,
            stake_balance,
        ));

        // assert_eq!(NodeSubnetStake::<Test>::get(subnet_node_id, 1), 0);

        let epoch_length = EpochLength::get();
        let epoch = System::block_number() / epoch_length;
        let block = System::block_number();

        let unbondings: BTreeMap<u32, UnbondingEntry> =
            StakeUnbondingLedger::<Test>::get(coldkey.clone());
        assert_eq!(unbondings.len(), 1);
        let (first_key, first_value) = unbondings.iter().next().unwrap();
        assert_eq!(
            *first_key,
            &block + StakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );
        assert_eq!(first_value.network, stake_balance);
        assert_eq!(first_value.overwatch, 0);

        let stake_cooldown_epochs = StakeCooldownEpochs::<Test>::get();

        increase_epochs(stake_cooldown_epochs + 1);

        let epoch = System::block_number() / epoch_length;

        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            coldkey.clone()
        )));

        // Check balance is in the wallet after unbonding
        let post_balance = Balances::free_balance(&coldkey.clone());
        assert_eq!(post_balance, starting_balance - burn_amount);

        // Check ledger removed the unbonding
        let unbondings: BTreeMap<u32, UnbondingEntry> =
            StakeUnbondingLedger::<Test>::get(coldkey.clone());
        assert_eq!(unbondings.len(), 0);
    });
}

#[test]
fn test_register_remove_delegate_claim_stake_unbondings() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 1000000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let delegate_staker = account(999);
        let delegate_stake_amount = 1000e+18 as u128;
        let _ = Balances::deposit_creating(&delegate_staker.clone(), delegate_stake_amount + 500);

        let starting_balance = Balances::free_balance(&delegate_staker.clone());

        assert_ok!(Network::add_subnet_delegate_stake(
            RuntimeOrigin::signed(delegate_staker.clone()),
            subnet_id,
            delegate_stake_amount,
        ));

        let delegate_shares =
            AccountSubnetDelegateStakeShares::<Test>::get(delegate_staker.clone(), subnet_id);
        assert!(delegate_shares > 0);

        let after_stake_balance = Balances::free_balance(&delegate_staker.clone());
        assert_eq!(
            after_stake_balance,
            starting_balance - delegate_stake_amount
        );
        let block = System::block_number();

        let before_remove_balance = Balances::free_balance(&delegate_staker.clone());

        // remove
        assert_ok!(Network::remove_delegate_stake(
            RuntimeOrigin::signed(delegate_staker.clone()),
            subnet_id,
            delegate_shares,
        ));

        let unbondings: BTreeMap<u32, UnbondingEntry> =
            StakeUnbondingLedger::<Test>::get(delegate_staker.clone());
        assert_eq!(unbondings.len(), 1);
        let (first_key, first_value) = unbondings.iter().next().unwrap();
        assert_eq!(
            *first_key,
            &block + DelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );
        assert_ne!(first_value.network, 0);
        assert_eq!(first_value.overwatch, 0);

        let stake_cooldown_epochs = DelegateStakeCooldownEpochs::<Test>::get();

        increase_epochs(stake_cooldown_epochs + 1);

        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            delegate_staker.clone()
        )));

        // Check balance is in the wallet after unbonding
        let post_balance = Balances::free_balance(&delegate_staker.clone());
        assert!(post_balance > before_remove_balance);

        // Check ledger removed the unbonding
        let unbondings: BTreeMap<u32, UnbondingEntry> =
            StakeUnbondingLedger::<Test>::get(delegate_staker.clone());
        assert_eq!(unbondings.len(), 0);
    });
}

#[test]
fn test_register_remove_node_delegate_claim_stake_unbondings() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 1000000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let validator_id = end - 1;

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end - 1);
        let subnet_node_id = end - 1;

        let delegate_staker = account(999);
        let delegate_stake_amount = 1000e+18 as u128;
        let _ = Balances::deposit_creating(&delegate_staker.clone(), delegate_stake_amount + 500);

        let starting_balance = Balances::free_balance(&delegate_staker.clone());

        // assert_ok!(Network::add_node_delegate_stake(
        //     RuntimeOrigin::signed(delegate_staker.clone()),
        //     subnet_id,
        //     subnet_node_id,
        //     delegate_stake_amount,
        // ));

        assert_ok!(Network::add_validator_delegate_stake(
            RuntimeOrigin::signed(delegate_staker.clone()),
            validator_id,
            delegate_stake_amount,
        ));

        let delegate_shares =
            AccountValidatorDelegateStakeShares::<Test>::get(delegate_staker.clone(), validator_id);
        assert!(delegate_shares > 0);

        let after_stake_balance = Balances::free_balance(&delegate_staker.clone());
        assert_eq!(
            after_stake_balance,
            starting_balance - delegate_stake_amount
        );
        let block = System::block_number();

        let before_remove_balance = Balances::free_balance(&delegate_staker.clone());

        // remove
        assert_ok!(Network::remove_validator_delegate_stake(
            RuntimeOrigin::signed(delegate_staker.clone()),
            validator_id,
            delegate_shares,
        ));

        let unbondings: BTreeMap<u32, UnbondingEntry> =
            StakeUnbondingLedger::<Test>::get(delegate_staker.clone());
        assert_eq!(unbondings.len(), 1);
        let (first_key, first_value) = unbondings.iter().next().unwrap();
        assert_eq!(
            *first_key,
            &block + DelegateStakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );
        assert_ne!(first_value.network, 0);
        assert_eq!(first_value.overwatch, 0);

        let stake_cooldown_epochs = DelegateStakeCooldownEpochs::<Test>::get();

        increase_epochs(stake_cooldown_epochs + 1);

        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            delegate_staker.clone()
        )));

        // Check balance is in the wallet after unbonding
        let post_balance = Balances::free_balance(&delegate_staker.clone());
        assert!(post_balance > before_remove_balance);

        // Check ledger removed the unbonding
        let unbondings: BTreeMap<u32, UnbondingEntry> =
            StakeUnbondingLedger::<Test>::get(delegate_staker.clone());
        assert_eq!(unbondings.len(), 0);
    });
}

#[test]
fn test_register_activate_remove_claim_stake_unbondings() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 1000000000000000000000000;
        let amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount + 500);

        let starting_balance = Balances::free_balance(&coldkey.clone());
        assert_eq!(starting_balance, deposit_amount + burn_amount + 500);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            current_id,
            subnet_id,
            None,
            Some(PeerInfo::<Test> {
                peer_id: peer(999),
                multiaddr: None,
            }),
            None,
            None,
            amount,
            None,
            None,
            u128::MAX,
        ));
        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let subnet_node = RegisteredSubnetNodesData::<Test>::get(subnet_id, subnet_node_id);
        let start_epoch = subnet_node.classification.start_epoch;

        let stake_balance = NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id);
        assert_eq!(stake_balance, amount);

        let after_stake_balance = Balances::free_balance(&coldkey.clone());
        assert_eq!(after_stake_balance, starting_balance - amount - burn_amount);

        let queue_epochs = SubnetNodeQueueEpochs::<Test>::get(subnet_id);

        let epoch = Network::get_current_epoch_as_u32();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // increase to the nodes start epoch
        set_block_to_subnet_slot_epoch(subnet_epoch + queue_epochs + 2, subnet_id);

        let epoch = Network::get_current_epoch_as_u32();
        let subnet_epoch = Network::get_current_subnet_epoch_as_u32(subnet_id);

        // Get subnet weights (nodes only activate from queue if there are weights)
        // Note: This means a subnet is active if it gets weights
        let _ = Network::handle_subnet_emission_weights(epoch);

        // Trigger the node activation
        Network::emission_step(
            &mut WeightMeter::new(),
            System::block_number(),
            Network::get_current_epoch_as_u32(),
            Network::get_current_subnet_epoch_as_u32(subnet_id),
            subnet_id,
        );

        assert_eq!(
            RegisteredSubnetNodesData::<Test>::try_get(subnet_id, subnet_node_id),
            Err(())
        );

        assert_ok!(Network::remove_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
        ));

        let stake_balance = NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id);

        // remove amount ontop
        assert_ok!(Network::do_remove_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            // false,
            stake_balance,
        ));
        // assert_ok!(Network::remove_stake(
        //     RuntimeOrigin::signed(coldkey.clone()),
        //     subnet_id,
        //     hotkey.clone(),
        //     stake_balance,
        // ));

        assert_eq!(NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id), 0);

        let epoch_length = EpochLength::get();
        let epoch = System::block_number() / epoch_length;
        let block = System::block_number();

        let unbondings: BTreeMap<u32, UnbondingEntry> =
            StakeUnbondingLedger::<Test>::get(coldkey.clone());
        assert_eq!(unbondings.len(), 1);
        let (first_key, first_value) = unbondings.iter().next().unwrap();
        assert_eq!(
            *first_key,
            &block + StakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );
        assert!(first_value.network <= stake_balance);
        assert_eq!(first_value.overwatch, 0);

        let stake_cooldown_epochs = StakeCooldownEpochs::<Test>::get();

        increase_epochs(stake_cooldown_epochs + 1);

        let epoch = System::block_number() / epoch_length;

        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            coldkey.clone()
        )));

        let post_balance = Balances::free_balance(&coldkey.clone());
        assert_eq!(post_balance, starting_balance - burn_amount);

        let unbondings: BTreeMap<u32, UnbondingEntry> =
            StakeUnbondingLedger::<Test>::get(coldkey.clone());
        assert_eq!(unbondings.len(), 0);
    });
}

#[test]
fn test_remove_stake_twice_in_epoch() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 1000000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount + 500);

        let starting_balance = Balances::free_balance(&coldkey.clone());
        assert_eq!(starting_balance, deposit_amount + burn_amount + 500);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            current_id,
            subnet_id,
            None,
            Some(PeerInfo::<Test> {
                peer_id: peer(999),
                multiaddr: None,
            }),
            None,
            None,
            stake_amount,
            None,
            None,
            u128::MAX,
        ));
        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let stake_balance = NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id);
        assert_eq!(stake_balance, stake_amount);

        let after_stake_balance = Balances::free_balance(&coldkey.clone());
        assert_eq!(
            after_stake_balance,
            starting_balance - stake_amount - burn_amount
        );

        let _ = Balances::deposit_creating(&account(1), stake_amount * 2);

        assert_ok!(Network::do_add_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            stake_amount * 3,
        ));

        let stake_balance = NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id);
        assert_eq!(stake_balance, stake_amount + stake_amount * 3);

        let epoch = System::block_number() / EpochLength::get();
        let block = System::block_number();

        // assert_ok!(Network::remove_stake(
        //     RuntimeOrigin::signed(coldkey.clone()),
        //     subnet_id,
        //     hotkey.clone(),
        //     stake_amount,
        // ));
        assert_ok!(Network::do_remove_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            // false,
            stake_amount,
        ));

        let unbondings: BTreeMap<u32, UnbondingEntry> =
            StakeUnbondingLedger::<Test>::get(coldkey.clone());
        let ledger_balance: u128 = unbondings.values().map(|entry| entry.network).sum();
        assert_eq!(unbondings.len() as u32, 1);
        assert_eq!(ledger_balance, stake_amount);
        let (ledger_block, ledger_balance) = unbondings.iter().next().unwrap();
        assert_eq!(
            *ledger_block,
            &block + StakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );

        // assert_ok!(Network::remove_stake(
        //     RuntimeOrigin::signed(coldkey.clone()),
        //     subnet_id,
        //     hotkey.clone(),
        //     stake_amount,
        // ));
        assert_ok!(Network::do_remove_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            // false,
            stake_amount,
        ));

        let unbondings: BTreeMap<u32, UnbondingEntry> =
            StakeUnbondingLedger::<Test>::get(coldkey.clone());
        let ledger_balance: u128 = unbondings.values().map(|entry| entry.network).sum();
        assert_eq!(unbondings.len() as u32, 1);
        assert_eq!(ledger_balance, stake_amount * 2);
        let (ledger_block, ledger_balance) = unbondings.iter().next().unwrap();
        assert_eq!(
            *ledger_block,
            &block + StakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );

        increase_epochs(1);

        let epoch = System::block_number() / EpochLength::get();
        let block = System::block_number();

        // assert_ok!(Network::remove_stake(
        //     RuntimeOrigin::signed(coldkey.clone()),
        //     subnet_id,
        //     hotkey.clone(),
        //     stake_amount,
        // ));
        assert_ok!(Network::do_remove_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            // false,
            stake_amount,
        ));

        let unbondings: BTreeMap<u32, UnbondingEntry> =
            StakeUnbondingLedger::<Test>::get(coldkey.clone());
        let total_ledger_balance: u128 = unbondings.values().map(|entry| entry.network).sum();
        assert_eq!(unbondings.len() as u32, 2);
        assert_eq!(total_ledger_balance, stake_amount * 3);
        let (ledger_block, ledger_balance) = unbondings.iter().last().unwrap();
        assert_eq!(
            *ledger_block,
            &block + StakeCooldownEpochs::<Test>::get() * EpochLength::get()
        );
        assert_eq!(ledger_balance.network, stake_amount);
        assert_eq!(ledger_balance.overwatch, 0);

        System::set_block_number(
            System::block_number()
                + ((EpochLength::get() + 1) * StakeCooldownEpochs::<Test>::get()),
        );

        let starting_balance = Balances::free_balance(&coldkey.clone());

        assert_ok!(Network::claim_unbondings(RuntimeOrigin::signed(
            coldkey.clone()
        )));

        let ending_balance = Balances::free_balance(&coldkey.clone());
        assert_eq!(starting_balance + total_ledger_balance, ending_balance);

        let unbondings: BTreeMap<u32, UnbondingEntry> =
            StakeUnbondingLedger::<Test>::get(coldkey.clone());
        assert_eq!(unbondings.len(), 0);
    });
}

#[test]
fn test_claim_stake_unbondings_no_unbondings_err() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 1000000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount + 500);

        let starting_balance = Balances::free_balance(&coldkey.clone());
        assert_eq!(starting_balance, deposit_amount + burn_amount + 500);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            current_id,
            subnet_id,
            None,
            Some(PeerInfo::<Test> {
                peer_id: peer(999),
                multiaddr: None,
            }),
            None,
            None,
            stake_amount,
            None,
            None,
            u128::MAX,
        ));
        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let stake_balance = NodeSubnetStake::<Test>::get(subnet_node_id, subnet_id);
        assert_eq!(stake_balance, stake_amount);

        let after_stake_balance = Balances::free_balance(&coldkey.clone());
        assert_eq!(
            after_stake_balance,
            starting_balance - stake_amount - burn_amount
        );

        assert_err!(
            Network::claim_unbondings(RuntimeOrigin::signed(coldkey.clone())),
            Error::<Test>::NoStakeUnbondingsOrCooldownNotMet
        );

        assert_ok!(Network::do_add_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            100,
        ));

        // assert_ok!(Network::remove_stake(
        //     RuntimeOrigin::signed(coldkey.clone()),
        //     subnet_id,
        //     hotkey.clone(),
        //     100,
        // ));
        assert_ok!(Network::do_remove_node_stake(
            RuntimeOrigin::signed(coldkey.clone()),
            subnet_id,
            subnet_node_id,
            // false,
            100,
        ));

        // No cooldown, should have same error
        assert_err!(
            Network::claim_unbondings(RuntimeOrigin::signed(coldkey.clone())),
            Error::<Test>::NoStakeUnbondingsOrCooldownNotMet
        );
    });
}

#[test]
fn test_remove_to_stake_max_unlockings_reached_err() {
    new_test_ext().execute_with(|| {
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 1000000000000000000000;

        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();

        let subnets = TotalActiveSubnets::<Test>::get() + 1;
        let max_subnet_nodes = MaxSubnetNodes::<Test>::get();
        let max_subnets = MaxSubnets::<Test>::get();
        let end = 4;

        build_activated_subnet(subnet_name.clone(), 0, 0, deposit_amount, stake_amount);

        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();
        let total_subnet_nodes = TotalSubnetNodes::<Test>::get(subnet_id);

        let coldkey = get_coldkey(subnets, max_subnet_nodes, end + 1);
        let hotkey = get_hotkey(subnets, max_subnet_nodes, max_subnets, end + 1);
        let peer_id = get_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let bootnode_peer_id =
            get_bootnode_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let client_peer_id = get_client_peer_id(subnets, max_subnet_nodes, max_subnets, end + 1);
        let burn_amount = Network::calculate_burn_amount(subnet_id);
        let _ = Balances::deposit_creating(&coldkey.clone(), deposit_amount + burn_amount + 500);

        let starting_balance = Balances::free_balance(&coldkey.clone());

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let current_id = TotalValidatorIds::<Test>::get();

        assert_ok!(Network::register_subnet_node(
            RuntimeOrigin::signed(coldkey.clone()),
            current_id,
            subnet_id,
            None,
            Some(PeerInfo::<Test> {
                peer_id: peer(999),
                multiaddr: None,
            }),
            None,
            None,
            deposit_amount,
            None,
            None,
            u128::MAX,
        ));
        let subnet_node_id = TotalSubnetNodeUids::<Test>::get(subnet_id);

        let max_unlockings = MaxUnbondings::<Test>::get();
        log::error!("max_unlockings {:?}", max_unlockings);
        for n in 0..max_unlockings + 2 {
            let _n = n + 1;
            // increase_epochs(1);
            System::set_block_number(System::block_number() + 1);
            if _n > max_unlockings {
                // assert_err!(
                //     Network::remove_stake(
                //         RuntimeOrigin::signed(coldkey.clone()),
                //         subnet_id,
                //         hotkey.clone(),
                //         1000,
                //     ),
                //     Error::<Test>::MaxUnlockingsReached
                // );
                assert_err!(
                    Network::remove_node_stake(
                        RuntimeOrigin::signed(coldkey.clone()),
                        subnet_id,
                        subnet_node_id,
                        1000,
                    ),
                    Error::<Test>::MaxUnlockingsReached
                );
            } else {
                // assert_ok!(Network::remove_stake(
                //     RuntimeOrigin::signed(coldkey.clone()),
                //     subnet_id,
                //     hotkey.clone(),
                //     1000,
                // ));
                assert_ok!(Network::remove_node_stake(
                    RuntimeOrigin::signed(coldkey.clone()),
                    subnet_id,
                    subnet_node_id,
                    1000,
                ));

                let unbondings: BTreeMap<u32, UnbondingEntry> =
                    StakeUnbondingLedger::<Test>::get(coldkey.clone());
                assert_eq!(unbondings.len() as u32, _n);
            }
        }
    });
}
