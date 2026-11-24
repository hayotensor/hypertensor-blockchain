use super::mock::*;
use crate::tests::test_utils::*;
use crate::{
    ActiveOverwatchRevealStats, Error, MinSubnetMinStake, MinSubnetNodes, OverwatchCommit,
    OverwatchCommits, OverwatchNodeIdHotkey, OverwatchNodeValidatorId, OverwatchNodes,
    OverwatchReveal, OverwatchRevealStats, OverwatchReveals, OverwatchValidatorWhitelist,
    SubnetData, SubnetName, SubnetState, SubnetsData, TotalOverwatchNodeUids, TotalValidatorIds,
    ValidatorColdkey, ValidatorIdHotkey, ValidatorOverwatchNodeId,
};
use frame_support::traits::Currency;
use frame_support::{assert_err, assert_noop, assert_ok};
use sp_std::collections::btree_map::BTreeMap;

fn insert_commit(epoch: u32, node_id: u32, subnet_id: u32, hash: sp_core::H256) {
    seed_overwatch_epoch_eligibility(node_id);
    OverwatchCommits::<Test>::mutate(epoch, node_id, |commits| {
        commits
            .try_insert(subnet_id, hash)
            .expect("test commit row fits the subnet bound");
    });
}

fn stored_commit(epoch: u32, node_id: u32, subnet_id: u32) -> Option<sp_core::H256> {
    OverwatchCommits::<Test>::get(epoch, node_id)
        .get(&subnet_id)
        .copied()
}

fn stored_reveal(epoch: u32, node_id: u32, subnet_id: u32) -> Option<u128> {
    OverwatchReveals::<Test>::get(epoch, node_id)
        .get(&subnet_id)
        .copied()
}

//
//
//
//
//
//
//
// Overwatch Commit-Reveal
//
//RuntimeOrigin::signed(coldkey.clone()),
//
//
//
//
//

#[test]
fn test_do_commit_and_reveal_weights_success() {
    new_test_ext().execute_with(|| {
        let coldkey: AccountId = account(1);
        let hotkey: AccountId = account(2);
        let overwatch_node_id = 1;
        let subnet_id = 99;

        let reward_rate = test_percent(1, 20); // 5%
        fund_validator_registration(&coldkey);
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey.clone(),
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, ());

        let overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();

        // Setup: assign ownership and create subnet
        let subnet_data = SubnetData {
            id: 1,
            friendly_id: 1,
            name: "subnet_name".into(),
            repo: "github".into(),
            description: "description".into(),
            misc: "misc".into(),
            consensus_mechanism: Default::default(),
            state: SubnetState::Active,
            consensus_eligible_from_subnet_epoch: Some(0),
        };

        SubnetsData::<Test>::insert(subnet_id, subnet_data);

        TotalOverwatchNodeUids::<Test>::mutate(|n: &mut u32| *n += 1);
        let current_uid = TotalOverwatchNodeUids::<Test>::get();

        OverwatchNodes::<Test>::insert(current_uid, ());
        OverwatchNodeIdHotkey::<Test>::insert(current_uid, hotkey.clone());
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, ());
        OverwatchNodeValidatorId::<Test>::insert(current_uid, validator_id);
        ValidatorOverwatchNodeId::<Test>::insert(validator_id, current_uid);

        // Weight + salt
        let weight: u128 = 123456;
        let salt: Vec<u8> = b"secret-salt".to_vec();
        let commit_hash = make_commit(
            overwatch_node_id,
            subnet_id,
            overwatch_epoch,
            weight,
            salt.clone(),
        );

        // Commit
        seed_overwatch_epoch_eligibility(overwatch_node_id);
        assert_ok!(Network::perform_commit_overwatch_subnet_weights(
            overwatch_node_id,
            vec![OverwatchCommit {
                subnet_id,
                weight: commit_hash
            }]
        ));

        // Ensure it's stored
        assert_eq!(
            stored_commit(overwatch_epoch, overwatch_node_id, subnet_id),
            Some(commit_hash)
        );

        // Reveal
        assert_ok!(Network::perform_reveal_overwatch_subnet_weights(
            overwatch_node_id,
            vec![OverwatchReveal {
                subnet_id,
                weight,
                salt: salt.clone().try_into().unwrap()
            }]
        ));

        // Ensure revealed weight is correct
        assert_eq!(
            stored_reveal(overwatch_epoch, overwatch_node_id, subnet_id),
            Some(weight)
        );
    });
}

#[test]
fn test_reveal_batch_validation_is_atomic() {
    new_test_ext().execute_with(|| {
        let overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();
        let overwatch_node_id = 1;
        let valid_subnet_id = 11;
        let missing_commit_subnet_id = 12;
        let weight = 123_456;
        let salt = b"atomic-reveal".to_vec();
        let commit_hash = make_commit(
            overwatch_node_id,
            valid_subnet_id,
            overwatch_epoch,
            weight,
            salt.clone(),
        );

        insert_commit(
            overwatch_epoch,
            overwatch_node_id,
            valid_subnet_id,
            commit_hash,
        );
        let initial_stats = ActiveOverwatchRevealStats::<Test>::get();

        assert_err!(
            Network::perform_reveal_overwatch_subnet_weights(
                overwatch_node_id,
                vec![
                    OverwatchReveal {
                        subnet_id: valid_subnet_id,
                        weight,
                        salt: salt.clone().try_into().unwrap(),
                    },
                    OverwatchReveal {
                        subnet_id: missing_commit_subnet_id,
                        weight,
                        salt: salt.try_into().unwrap(),
                    },
                ],
            ),
            Error::<Test>::RevealSetMismatch
        );

        assert_eq!(
            stored_reveal(overwatch_epoch, overwatch_node_id, valid_subnet_id),
            None
        );
        assert_eq!(
            stored_reveal(overwatch_epoch, overwatch_node_id, missing_commit_subnet_id),
            None
        );
        assert_eq!(ActiveOverwatchRevealStats::<Test>::get(), initial_stats);
    });
}

#[test]
fn test_commit_batch_validation_is_atomic() {
    new_test_ext().execute_with(|| {
        let epoch = Network::get_current_overwatch_epoch_as_u32();
        let node_id = 1;
        let new_subnet_id = 1;
        let duplicate_subnet_id = 2;
        insert_subnet(new_subnet_id, SubnetState::Active, 0);
        insert_subnet(duplicate_subnet_id, SubnetState::Active, 0);

        let existing_hash = make_commit(
            node_id,
            duplicate_subnet_id,
            epoch,
            10,
            b"existing".to_vec(),
        );
        let new_hash = make_commit(node_id, new_subnet_id, epoch, 20, b"new".to_vec());
        insert_commit(epoch, node_id, duplicate_subnet_id, existing_hash);

        seed_overwatch_epoch_eligibility(node_id);

        assert_err!(
            Network::perform_commit_overwatch_subnet_weights(
                node_id,
                vec![
                    OverwatchCommit {
                        subnet_id: new_subnet_id,
                        weight: new_hash,
                    },
                    OverwatchCommit {
                        subnet_id: duplicate_subnet_id,
                        weight: existing_hash,
                    },
                ],
            ),
            Error::<Test>::AlreadyCommitted
        );

        assert_eq!(stored_commit(epoch, node_id, new_subnet_id), None);
        assert_eq!(
            stored_commit(epoch, node_id, duplicate_subnet_id),
            Some(existing_hash)
        );
    });
}

#[test]
fn test_maximum_commit_batch_cannot_be_extended() {
    new_test_ext().execute_with(|| {
        let (node, commits, _) =
            single_call_fixture(<Test as crate::Config>::MaxPhysicalSubnetsUpperBound::get());
        let extra = <Test as crate::Config>::MaxPhysicalSubnetsUpperBound::get() + 1;
        insert_subnet(extra, SubnetState::Active, 0);
        assert_ok!(Network::perform_commit_overwatch_subnet_weights(
            node,
            commits.clone()
        ));
        assert_noop!(
            Network::perform_commit_overwatch_subnet_weights(node, commits),
            Error::<Test>::AlreadyCommitted
        );
        assert_noop!(
            Network::perform_commit_overwatch_subnet_weights(
                node,
                vec![OverwatchCommit {
                    subnet_id: extra,
                    weight: sp_core::H256::zero(),
                }]
            ),
            Error::<Test>::AlreadyCommitted
        );
    });
}

#[test]
fn test_reveal_rejects_new_subnet_after_epoch_subnet_bound() {
    new_test_ext().execute_with(|| {
        let overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();
        let overwatch_node_id = 1;
        let max_subnets = <Test as crate::Config>::MaxPhysicalSubnetsUpperBound::get();
        let new_subnet_id = max_subnets.saturating_add(1);
        let weight = 123_456;
        let salt = b"subnet-bound".to_vec();
        // Only live subnets take a slot in the distinct-subnet index.
        insert_subnet(new_subnet_id, SubnetState::Active, 0);
        let commit_hash = make_commit(
            overwatch_node_id,
            new_subnet_id,
            overwatch_epoch,
            weight,
            salt.clone(),
        );
        insert_commit(
            overwatch_epoch,
            overwatch_node_id,
            new_subnet_id,
            commit_hash,
        );

        let initial_stats = OverwatchRevealStats::<Test> {
            records: max_subnets,
            subnet_revealer_counts: (1..=max_subnets)
                .map(|subnet_id| (subnet_id, 1))
                .collect::<BTreeMap<_, _>>()
                .try_into()
                .unwrap(),
        };
        ActiveOverwatchRevealStats::<Test>::put(initial_stats.clone());

        assert_err!(
            Network::perform_reveal_overwatch_subnet_weights(
                overwatch_node_id,
                vec![OverwatchReveal {
                    subnet_id: new_subnet_id,
                    weight,
                    salt: salt.try_into().unwrap(),
                }],
            ),
            Error::<Test>::MaxOverwatchRevealSubnets
        );
        assert_eq!(
            stored_reveal(overwatch_epoch, overwatch_node_id, new_subnet_id),
            None
        );
        assert_eq!(ActiveOverwatchRevealStats::<Test>::get(), initial_stats);
    });
}

#[test]
fn test_reveal_rejects_unique_record_after_epoch_product_bound() {
    new_test_ext().execute_with(|| {
        let overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();
        let overwatch_node_id = 1;
        let subnet_id = 1;
        let max_nodes = <Test as crate::Config>::MaxOverwatchNodesUpperBound::get();
        let max_subnets = <Test as crate::Config>::MaxPhysicalSubnetsUpperBound::get();
        let max_records = max_nodes.saturating_mul(max_subnets);
        let weight = 123_456;
        let salt = b"record-bound".to_vec();
        let commit_hash = make_commit(
            overwatch_node_id,
            subnet_id,
            overwatch_epoch,
            weight,
            salt.clone(),
        );
        insert_commit(overwatch_epoch, overwatch_node_id, subnet_id, commit_hash);

        let initial_stats = OverwatchRevealStats::<Test> {
            records: max_records,
            subnet_revealer_counts: (1..=max_subnets)
                .map(|subnet_id| (subnet_id, max_nodes))
                .collect::<BTreeMap<_, _>>()
                .try_into()
                .unwrap(),
        };
        ActiveOverwatchRevealStats::<Test>::put(initial_stats.clone());

        assert_err!(
            Network::perform_reveal_overwatch_subnet_weights(
                overwatch_node_id,
                vec![OverwatchReveal {
                    subnet_id,
                    weight,
                    salt: salt.try_into().unwrap(),
                }],
            ),
            Error::<Test>::MaxOverwatchRevealRecords
        );
        assert_eq!(
            stored_reveal(overwatch_epoch, overwatch_node_id, subnet_id),
            None
        );
        assert_eq!(ActiveOverwatchRevealStats::<Test>::get(), initial_stats);
    });
}

#[test]
fn reveal_stats_increment_once_and_removal_decrements_shared_and_sole_subnets() {
    new_test_ext().execute_with(|| {
        let epoch = Network::get_current_overwatch_epoch_as_u32();
        let shared_subnet_id = 1;
        let sole_subnet_id = 2;
        insert_subnet(shared_subnet_id, SubnetState::Active, 0);
        insert_subnet(sole_subnet_id, SubnetState::Active, 0);

        manual_insert_validator(1, 101, 201);
        manual_insert_validator(2, 102, 202);
        let first_node_id = insert_overwatch_node(1, None);
        let second_node_id = insert_overwatch_node(2, None);

        let first_shared_weight = test_percent(1, 4);
        let first_shared_salt = b"first-shared".to_vec();
        let first_sole_weight = test_percent(1, 3);
        let first_sole_salt = b"first-sole".to_vec();
        let second_shared_weight = test_percent(3, 4);
        let second_shared_salt = b"second-shared".to_vec();

        seed_overwatch_epoch_eligibility(first_node_id);

        assert_ok!(Network::perform_commit_overwatch_subnet_weights(
            first_node_id,
            vec![
                OverwatchCommit {
                    subnet_id: shared_subnet_id,
                    weight: make_commit(
                        first_node_id,
                        shared_subnet_id,
                        epoch,
                        first_shared_weight,
                        first_shared_salt.clone()
                    ),
                },
                OverwatchCommit {
                    subnet_id: sole_subnet_id,
                    weight: make_commit(
                        first_node_id,
                        sole_subnet_id,
                        epoch,
                        first_sole_weight,
                        first_sole_salt.clone()
                    ),
                },
            ],
        ));
        seed_overwatch_epoch_eligibility(second_node_id);
        assert_ok!(Network::perform_commit_overwatch_subnet_weights(
            second_node_id,
            vec![OverwatchCommit {
                subnet_id: shared_subnet_id,
                weight: make_commit(
                    second_node_id,
                    shared_subnet_id,
                    epoch,
                    second_shared_weight,
                    second_shared_salt.clone()
                ),
            }],
        ));

        assert_ok!(Network::perform_reveal_overwatch_subnet_weights(
            first_node_id,
            vec![
                OverwatchReveal {
                    subnet_id: shared_subnet_id,
                    weight: first_shared_weight,
                    salt: first_shared_salt.try_into().unwrap(),
                },
                OverwatchReveal {
                    subnet_id: sole_subnet_id,
                    weight: first_sole_weight,
                    salt: first_sole_salt.try_into().unwrap(),
                },
            ],
        ));
        let stats_after_first = ActiveOverwatchRevealStats::<Test>::get();
        assert_eq!(stats_after_first.records, 2);
        assert_eq!(
            stats_after_first
                .subnet_revealer_counts
                .get(&shared_subnet_id),
            Some(&1)
        );
        assert_eq!(
            stats_after_first
                .subnet_revealer_counts
                .get(&sole_subnet_id),
            Some(&1)
        );

        assert_ok!(Network::perform_reveal_overwatch_subnet_weights(
            second_node_id,
            vec![OverwatchReveal {
                subnet_id: shared_subnet_id,
                weight: second_shared_weight,
                salt: second_shared_salt.clone().try_into().unwrap(),
            }],
        ));
        let stats_after_both = ActiveOverwatchRevealStats::<Test>::get();
        assert_eq!(stats_after_both.records, 3);
        assert_eq!(
            stats_after_both
                .subnet_revealer_counts
                .get(&shared_subnet_id),
            Some(&2)
        );
        assert_eq!(
            stats_after_both.subnet_revealer_counts.get(&sole_subnet_id),
            Some(&1)
        );

        // Replaying a completed reveal is rejected without changing either counter.
        assert_noop!(
            Network::perform_reveal_overwatch_subnet_weights(
                second_node_id,
                vec![OverwatchReveal {
                    subnet_id: shared_subnet_id,
                    weight: second_shared_weight,
                    salt: second_shared_salt.try_into().unwrap(),
                }],
            ),
            Error::<Test>::AlreadyRevealed
        );
        assert_eq!(ActiveOverwatchRevealStats::<Test>::get(), stats_after_both);

        assert_ok!(Network::perform_disqualify_overwatch_node(first_node_id));

        let remaining_stats = ActiveOverwatchRevealStats::<Test>::get();
        assert_eq!(remaining_stats.records, 1);
        assert_eq!(
            remaining_stats
                .subnet_revealer_counts
                .get(&shared_subnet_id),
            Some(&1)
        );
        assert!(!remaining_stats
            .subnet_revealer_counts
            .contains_key(&sole_subnet_id));
        assert!(OverwatchCommits::<Test>::get(epoch, first_node_id).is_empty());
        assert!(OverwatchReveals::<Test>::get(epoch, first_node_id).is_empty());
        assert_eq!(
            stored_reveal(epoch, second_node_id, shared_subnet_id),
            Some(second_shared_weight)
        );
    });
}

#[test]
fn test_do_commit_and_reveal_weights_not_key_owner_error() {
    new_test_ext().execute_with(|| {
        let coldkey: AccountId = account(1);
        let hotkey: AccountId = account(2);
        let overwatch_node_id = 1;
        let subnet_id = 99;
        let overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();

        // Setup: assign ownership and create subnet
        let subnet_data = SubnetData {
            id: 1,
            friendly_id: 1,
            name: "subnet_name".into(),
            repo: "github".into(),
            description: "description".into(),
            misc: "misc".into(),
            consensus_mechanism: Default::default(),
            state: SubnetState::Active,
            consensus_eligible_from_subnet_epoch: Some(0),
        };

        SubnetsData::<Test>::insert(subnet_id, subnet_data);

        let reward_rate = test_percent(1, 20); // 5%
        fund_validator_registration(&coldkey);
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey.clone(),
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, ());

        TotalOverwatchNodeUids::<Test>::mutate(|n: &mut u32| *n += 1);
        let current_uid = TotalOverwatchNodeUids::<Test>::get();

        OverwatchNodes::<Test>::insert(current_uid, ());
        OverwatchNodeIdHotkey::<Test>::insert(current_uid, hotkey.clone());
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, ());
        OverwatchNodeValidatorId::<Test>::insert(current_uid, validator_id);
        ValidatorOverwatchNodeId::<Test>::insert(validator_id, current_uid);

        // Weight + salt
        let weight: u128 = 123456;
        let salt: Vec<u8> = b"secret-salt".to_vec();
        let commit_hash = make_commit(
            overwatch_node_id,
            subnet_id,
            overwatch_epoch,
            weight,
            salt.clone(),
        );

        // Commit
        seed_overwatch_epoch_eligibility(1);
        assert_err!(
            Network::commit_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(999)),
                1,
                vec![OverwatchCommit {
                    subnet_id,
                    weight: commit_hash
                }]
            ),
            Error::<Test>::NotKeyOwner
        );
    });
}

#[test]
fn test_do_commit_and_reveal_weights_commits_empty_error() {
    new_test_ext().execute_with(|| {
        let coldkey: AccountId = account(1);
        let hotkey: AccountId = account(2);
        let subnet_id = 99;

        let reward_rate = test_percent(1, 20); // 5%
        fund_validator_registration(&coldkey);
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey.clone(),
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, ());

        let overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();

        // Setup: assign ownership and create subnet
        let subnet_data = SubnetData {
            id: 1,
            friendly_id: 1,
            name: "subnet_name".into(),
            repo: "github".into(),
            description: "description".into(),
            misc: "misc".into(),
            consensus_mechanism: Default::default(),
            state: SubnetState::Active,
            consensus_eligible_from_subnet_epoch: Some(0),
        };

        SubnetsData::<Test>::insert(subnet_id, subnet_data);

        TotalOverwatchNodeUids::<Test>::mutate(|n: &mut u32| *n += 1);
        let current_uid = TotalOverwatchNodeUids::<Test>::get();

        OverwatchNodes::<Test>::insert(current_uid, ());
        OverwatchNodeIdHotkey::<Test>::insert(current_uid, hotkey.clone());
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, ());
        OverwatchNodeValidatorId::<Test>::insert(current_uid, validator_id);
        ValidatorOverwatchNodeId::<Test>::insert(validator_id, current_uid);

        // Weight + salt
        let weight: u128 = 123456;
        let salt: Vec<u8> = b"secret-salt".to_vec();
        let commit_hash = make_commit(
            current_uid,
            subnet_id,
            overwatch_epoch,
            weight,
            salt.clone(),
        );

        // Commit
        seed_overwatch_epoch_eligibility(current_uid);
        assert_err!(
            Network::commit_overwatch_subnet_weights(
                RuntimeOrigin::signed(hotkey.clone()),
                current_uid,
                vec![]
            ),
            Error::<Test>::CommitsEmpty
        );

        assert_err!(
            Network::perform_reveal_overwatch_subnet_weights(current_uid, vec![]),
            Error::<Test>::RevealsEmpty
        );
        assert!(!OverwatchReveals::<Test>::contains_key(
            overwatch_epoch,
            current_uid
        ));
    });
}

#[test]
fn test_do_commit_and_reveal_weights_already_committed_error() {
    new_test_ext().execute_with(|| {
        let coldkey: AccountId = account(1);
        let hotkey: AccountId = account(2);

        let reward_rate = test_percent(1, 20); // 5%
        fund_validator_registration(&coldkey);
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey.clone(),
            reward_rate,
            None,
            None,
        ));

        let overwatch_node_id = 1;
        let validator_id = TotalValidatorIds::<Test>::get();
        let subnet_id = 99;
        let overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();

        // Setup: assign ownership and create subnet
        let subnet_data = SubnetData {
            id: 1,
            friendly_id: 1,
            name: "subnet_name".into(),
            repo: "github".into(),
            description: "description".into(),
            misc: "misc".into(),
            consensus_mechanism: Default::default(),
            state: SubnetState::Active,
            consensus_eligible_from_subnet_epoch: Some(0),
        };

        SubnetsData::<Test>::insert(subnet_id, subnet_data);

        TotalOverwatchNodeUids::<Test>::mutate(|n: &mut u32| *n += 1);
        let current_uid = TotalOverwatchNodeUids::<Test>::get();

        OverwatchNodes::<Test>::insert(current_uid, ());
        OverwatchNodeIdHotkey::<Test>::insert(current_uid, hotkey.clone());
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, ());
        OverwatchNodeValidatorId::<Test>::insert(current_uid, validator_id);
        ValidatorOverwatchNodeId::<Test>::insert(validator_id, current_uid);

        // Weight + salt
        let weight: u128 = 123456;
        let salt: Vec<u8> = b"secret-salt".to_vec();
        let commit_hash = make_commit(
            overwatch_node_id,
            subnet_id,
            overwatch_epoch,
            weight,
            salt.clone(),
        );

        // Commit
        seed_overwatch_epoch_eligibility(current_uid);
        assert_ok!(Network::commit_overwatch_subnet_weights(
            RuntimeOrigin::signed(hotkey.clone()),
            current_uid,
            vec![OverwatchCommit {
                subnet_id,
                weight: commit_hash
            }]
        ));

        // Ensure it's stored
        assert_eq!(
            stored_commit(overwatch_epoch, current_uid, subnet_id),
            Some(commit_hash)
        );

        // Reveal
        assert_ok!(Network::perform_reveal_overwatch_subnet_weights(
            current_uid,
            vec![OverwatchReveal {
                subnet_id,
                weight,
                salt: salt.clone().try_into().unwrap()
            }]
        ));

        // Ensure revealed weight is correct
        assert_eq!(
            stored_reveal(overwatch_epoch, current_uid, subnet_id),
            Some(weight)
        );

        seed_overwatch_epoch_eligibility(current_uid);

        assert_err!(
            Network::commit_overwatch_subnet_weights(
                RuntimeOrigin::signed(hotkey.clone()),
                current_uid,
                vec![OverwatchCommit {
                    subnet_id,
                    weight: commit_hash
                }]
            ),
            Error::<Test>::AlreadyCommitted
        );
    });
}

#[test]
fn test_commit_and_reveal_extrinsics() {
    new_test_ext().execute_with(|| {
        // subnet
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let min_subnet_nodes = MinSubnetNodes::<Test>::get();
        let end = min_subnet_nodes;
        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let validator_id = 1;
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, ());
        let hotkey = ValidatorIdHotkey::<Test>::get(validator_id).unwrap();
        let coldkey = ValidatorColdkey::<Test>::get(validator_id).unwrap();
        let amount = 100000000000000000000;

        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;
        prepare_overwatch_validator(validator_id);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        let hotkey = Network::get_overwatch_node_associated_hotkey(overwatch_node_id).unwrap();

        let subnet_id = 99;
        let overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();

        // Setup: assign ownership and create subnet
        let subnet_data = SubnetData {
            id: 1,
            friendly_id: 1,
            name: "subnet_name".into(),
            repo: "github".into(),
            description: "description".into(),
            misc: "misc".into(),
            consensus_mechanism: Default::default(),
            state: SubnetState::Active,
            consensus_eligible_from_subnet_epoch: Some(0),
        };

        SubnetsData::<Test>::insert(subnet_id, subnet_data);

        // Weight + salt
        let weight: u128 = 123456;
        let salt: Vec<u8> = b"secret-salt".to_vec();
        let commit_hash = make_commit(
            overwatch_node_id,
            subnet_id,
            overwatch_epoch,
            weight,
            salt.clone(),
        );

        // Commit
        seed_overwatch_epoch_eligibility(overwatch_node_id);
        assert_ok!(Network::commit_overwatch_subnet_weights(
            RuntimeOrigin::signed(hotkey.clone()),
            overwatch_node_id,
            vec![OverwatchCommit {
                subnet_id,
                weight: commit_hash
            }]
        ));

        // Ensure it's stored
        assert_eq!(
            stored_commit(overwatch_epoch, overwatch_node_id, subnet_id),
            Some(commit_hash)
        );

        set_block_to_overwatch_reveal_block(overwatch_epoch);

        // Reveal
        assert_ok!(Network::reveal_overwatch_subnet_weights(
            RuntimeOrigin::signed(hotkey.clone()),
            overwatch_node_id,
            vec![OverwatchReveal {
                subnet_id,
                weight,
                salt: salt.clone().try_into().unwrap()
            }]
        ));

        // Ensure revealed weight is correct
        assert_eq!(
            stored_reveal(overwatch_epoch, overwatch_node_id, subnet_id),
            Some(weight)
        );
    });
}

#[test]
fn test_reveal_overwatch_subnet_weights_not_key_owner_error() {
    new_test_ext().execute_with(|| {
        // subnet
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let min_subnet_nodes = MinSubnetNodes::<Test>::get();
        let end = min_subnet_nodes;
        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let validator_id = 1;
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, ());
        let hotkey = ValidatorIdHotkey::<Test>::get(validator_id).unwrap();
        let coldkey = ValidatorColdkey::<Test>::get(validator_id).unwrap();
        let amount = 100000000000000000000;
        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;
        prepare_overwatch_validator(validator_id);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        let subnet_id = 99;
        let overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();

        // Setup: assign ownership and create subnet
        let subnet_data = SubnetData {
            id: 1,
            friendly_id: 1,
            name: "subnet_name".into(),
            repo: "github".into(),
            description: "description".into(),
            misc: "misc".into(),
            consensus_mechanism: Default::default(),
            state: SubnetState::Active,
            consensus_eligible_from_subnet_epoch: Some(0),
        };

        SubnetsData::<Test>::insert(subnet_id, subnet_data);

        // Weight + salt
        let weight: u128 = 123456;
        let salt: Vec<u8> = b"secret-salt".to_vec();
        let commit_hash = make_commit(
            overwatch_node_id,
            subnet_id,
            overwatch_epoch,
            weight,
            salt.clone(),
        );

        // Commit
        seed_overwatch_epoch_eligibility(overwatch_node_id);
        assert_ok!(Network::commit_overwatch_subnet_weights(
            RuntimeOrigin::signed(hotkey.clone()),
            overwatch_node_id,
            vec![OverwatchCommit {
                subnet_id,
                weight: commit_hash
            }]
        ));

        // Ensure it's stored
        assert_eq!(
            stored_commit(overwatch_epoch, overwatch_node_id, subnet_id),
            Some(commit_hash)
        );

        set_block_to_overwatch_reveal_block(overwatch_epoch);

        // Reveal
        assert_err!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(0)),
                overwatch_node_id,
                vec![OverwatchReveal {
                    subnet_id,
                    weight: weight.clone(),
                    salt: salt.clone().try_into().unwrap()
                }]
            ),
            Error::<Test>::NotKeyOwner
        );

        // assert_err!(
        //     Network::reveal_overwatch_subnet_weights(
        //         RuntimeOrigin::signed(hotkey.clone()),
        //         123,
        //         vec![OverwatchReveal {
        //             subnet_id,
        //             weight: weight.clone(),
        //             salt: salt.clone()
        //         }]
        //     ),
        //     Error::<Test>::NotKeyOwner
        // );
    });
}

#[test]
fn test_reveal_overwatch_subnet_weights_no_commit_found_error() {
    new_test_ext().execute_with(|| {
        // subnet
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let min_subnet_nodes = MinSubnetNodes::<Test>::get();
        let end = min_subnet_nodes;
        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let validator_id = 1;
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, ());
        let hotkey = ValidatorIdHotkey::<Test>::get(validator_id).unwrap();
        let coldkey = ValidatorColdkey::<Test>::get(validator_id).unwrap();
        let amount = 100000000000000000000;
        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;
        prepare_overwatch_validator(validator_id);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        let subnet_id = 99;
        let overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();

        // Setup: assign ownership and create subnet
        let subnet_data = SubnetData {
            id: 1,
            friendly_id: 1,
            name: "subnet_name".into(),
            repo: "github".into(),
            description: "description".into(),
            misc: "misc".into(),
            consensus_mechanism: Default::default(),
            state: SubnetState::Active,
            consensus_eligible_from_subnet_epoch: Some(0),
        };

        SubnetsData::<Test>::insert(subnet_id, subnet_data);

        // Weight + salt
        let weight: u128 = 123456;
        let salt: Vec<u8> = b"secret-salt".to_vec();
        let commit_hash = make_commit(
            overwatch_node_id,
            subnet_id,
            overwatch_epoch,
            weight,
            salt.clone(),
        );

        set_block_to_overwatch_reveal_block(overwatch_epoch);

        // Reveal
        seed_overwatch_epoch_eligibility(overwatch_node_id);
        assert_err!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(hotkey.clone()),
                overwatch_node_id,
                vec![OverwatchReveal {
                    subnet_id,
                    weight: weight.clone(),
                    salt: salt.clone().try_into().unwrap()
                }]
            ),
            Error::<Test>::NoCommitFound
        );
    });
}

#[test]
fn test_reveal_overwatch_subnet_weights_reveal_mismatch_error() {
    new_test_ext().execute_with(|| {
        // subnet
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let min_subnet_nodes = MinSubnetNodes::<Test>::get();
        let end = min_subnet_nodes;
        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let validator_id = 1;
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, ());
        let hotkey = ValidatorIdHotkey::<Test>::get(validator_id).unwrap();
        let coldkey = ValidatorColdkey::<Test>::get(validator_id).unwrap();
        let amount = 100000000000000000000;
        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;
        prepare_overwatch_validator(validator_id);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        let subnet_id = 99;
        let overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();

        // Setup: assign ownership and create subnet
        let subnet_data = SubnetData {
            id: 1,
            friendly_id: 1,
            name: "subnet_name".into(),
            repo: "github".into(),
            description: "description".into(),
            misc: "misc".into(),
            consensus_mechanism: Default::default(),
            state: SubnetState::Active,
            consensus_eligible_from_subnet_epoch: Some(0),
        };

        SubnetsData::<Test>::insert(subnet_id, subnet_data);

        // Weight + salt
        let weight: u128 = 123456;
        let salt: Vec<u8> = b"secret-salt".to_vec();
        let commit_hash = make_commit(
            overwatch_node_id,
            subnet_id,
            overwatch_epoch,
            weight,
            salt.clone(),
        );

        let fake_salt: Vec<u8> = b"fake-salt".to_vec();

        // Commit
        seed_overwatch_epoch_eligibility(overwatch_node_id);
        assert_ok!(Network::commit_overwatch_subnet_weights(
            RuntimeOrigin::signed(hotkey.clone()),
            overwatch_node_id,
            vec![OverwatchCommit {
                subnet_id,
                weight: commit_hash
            }]
        ));

        // Ensure it's stored
        assert_eq!(
            stored_commit(overwatch_epoch, overwatch_node_id, subnet_id),
            Some(commit_hash)
        );

        set_block_to_overwatch_reveal_block(overwatch_epoch);

        // Reveal
        assert_err!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(hotkey.clone()),
                overwatch_node_id,
                vec![OverwatchReveal {
                    subnet_id,
                    weight: weight.clone(),
                    salt: fake_salt.clone().try_into().unwrap()
                }]
            ),
            Error::<Test>::RevealMismatch
        );

        assert_err!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(hotkey.clone()),
                overwatch_node_id,
                vec![OverwatchReveal {
                    subnet_id,
                    weight: weight.clone() + 1,
                    salt: salt.clone().try_into().unwrap()
                }]
            ),
            Error::<Test>::RevealMismatch
        );
    });
}

#[test]
fn test_commit_and_reveal_require_single_complete_batches() {
    use frame_support::dispatch::Pays;
    new_test_ext().execute_with(|| {
        let (node, commits, reveals) = single_call_fixture(3);
        assert_eq!(
            Network::commit_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(201)),
                node,
                commits.clone()
            )
            .unwrap()
            .pays_fee,
            Pays::No
        );
        for batch in [commits.clone(), vec![commits[2].clone()]] {
            assert_noop!(
                Network::commit_overwatch_subnet_weights(
                    RuntimeOrigin::signed(account(201)),
                    node,
                    batch
                ),
                Error::<Test>::AlreadyCommitted
            );
        }
        insert_subnet(4, SubnetState::Active, 0);
        assert_noop!(
            Network::commit_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(201)),
                node,
                vec![OverwatchCommit {
                    subnet_id: 4,
                    weight: sp_core::H256::zero()
                }],
            ),
            Error::<Test>::AlreadyCommitted
        );
        // Node identity, not its current operational key, owns the submission opportunity.
        assert_ok!(Network::update_overwatch_hotkey(
            RuntimeOrigin::signed(account(101)),
            node,
            Some(account(202))
        ));
        assert_noop!(
            Network::commit_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(202)),
                node,
                commits
            ),
            Error::<Test>::AlreadyCommitted
        );
        set_block_to_overwatch_reveal_block(1);
        let partial = vec![reveals[0].clone()];
        assert_noop!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(202)),
                node,
                partial.clone()
            ),
            Error::<Test>::RevealSetMismatch
        );
        assert_eq!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(202)),
                node,
                partial
            )
            .unwrap_err()
            .post_info
            .pays_fee,
            Pays::Yes
        );
        let mut unordered = reveals.clone();
        unordered.reverse();
        assert_eq!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(202)),
                node,
                unordered
            )
            .unwrap()
            .pays_fee,
            Pays::No
        );
        assert_ok!(Network::update_overwatch_hotkey(
            RuntimeOrigin::signed(account(101)),
            node,
            None
        ));
        assert_noop!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(201)),
                node,
                reveals
            ),
            Error::<Test>::AlreadyRevealed
        );
        assert_eq!(ActiveOverwatchRevealStats::<Test>::get().records, 3);
    });
}

#[test]
fn test_commit_and_reveal_phase_errors() {
    new_test_ext().execute_with(|| {
        // subnet
        let subnet_name: Vec<u8> = "subnet-name".into();
        let deposit_amount: u128 = 10000000000000000000000;
        let stake_amount: u128 = MinSubnetMinStake::<Test>::get();
        let min_subnet_nodes = MinSubnetNodes::<Test>::get();
        let end = min_subnet_nodes;
        build_activated_subnet(subnet_name.clone(), 0, end, deposit_amount, stake_amount);
        let subnet_id = SubnetName::<Test>::get(subnet_name.clone()).unwrap();

        let validator_id = 1;
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, ());
        let hotkey = ValidatorIdHotkey::<Test>::get(validator_id).unwrap();
        let coldkey = ValidatorColdkey::<Test>::get(validator_id).unwrap();
        let amount = 100000000000000000000;
        let _ = Balances::deposit_creating(&coldkey.clone(), 100000000000000000000 + 500);

        let overwatch_node_id = TotalOverwatchNodeUids::<Test>::get() + 1;
        prepare_overwatch_validator(validator_id);

        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(coldkey.clone()),
            amount,
        ));

        let subnet_id = 99;
        let overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();

        // Weight + salt
        let weight: u128 = 123456;
        let salt: Vec<u8> = b"secret-salt".to_vec();
        let commit_hash = make_commit(
            overwatch_node_id,
            subnet_id,
            overwatch_epoch,
            weight,
            salt.clone(),
        );

        // Reveal
        assert_err!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(hotkey.clone()),
                overwatch_node_id,
                vec![OverwatchReveal {
                    subnet_id,
                    weight,
                    salt: salt.try_into().unwrap()
                }]
            ),
            Error::<Test>::NotRevealPeriod
        );

        set_block_to_overwatch_reveal_block(overwatch_epoch);

        // Commit fail
        seed_overwatch_epoch_eligibility(overwatch_node_id);
        assert_err!(
            Network::commit_overwatch_subnet_weights(
                RuntimeOrigin::signed(hotkey.clone()),
                overwatch_node_id,
                vec![OverwatchCommit {
                    subnet_id,
                    weight: commit_hash
                }]
            ),
            Error::<Test>::NotCommitPeriod
        );
    });
}

#[test]
fn copied_commitment_cannot_be_revealed_by_another_node_even_if_submitted_first() {
    new_test_ext().execute_with(|| {
        let epoch = Network::get_current_overwatch_epoch_as_u32();
        let subnet_id = 1;
        insert_subnet(subnet_id, SubnetState::Active, 0);
        manual_insert_validator(1, 101, 201);
        manual_insert_validator(2, 102, 202);
        let honest_node = insert_overwatch_node(1, None);
        let copying_node = insert_overwatch_node(2, None);
        let weight = test_percent(3, 5);
        let salt = b"honest-private-salt".to_vec();
        let commitment = make_commit(honest_node, subnet_id, epoch, weight, salt.clone());

        set_block_to_overwatch_commit_block(epoch);
        // The attacker can front-run the honest transaction with its public hash.
        for (node, hotkey) in [(copying_node, account(202)), (honest_node, account(201))] {
            seed_overwatch_epoch_eligibility(node);
            assert_ok!(Network::commit_overwatch_subnet_weights(
                RuntimeOrigin::signed(hotkey),
                node,
                vec![OverwatchCommit {
                    subnet_id,
                    weight: commitment
                }],
            ));
        }

        set_block_to_overwatch_reveal_block(epoch);
        let reveal = || {
            vec![OverwatchReveal {
                subnet_id,
                weight,
                salt: salt.clone().try_into().unwrap(),
            }]
        };
        assert_ok!(Network::reveal_overwatch_subnet_weights(
            RuntimeOrigin::signed(account(201)),
            honest_node,
            reveal(),
        ));
        assert_eq!(stored_reveal(epoch, honest_node, subnet_id), Some(weight));
        let stats = ActiveOverwatchRevealStats::<Test>::get();
        assert_eq!(stats.records, 1);

        // Learning the honest node's weight and salt does not open the copied hash.
        assert_noop!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(202)),
                copying_node,
                reveal(),
            ),
            Error::<Test>::RevealMismatch
        );
        // Claiming the honest node ID instead fails authentication.
        assert_noop!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(202)),
                honest_node,
                reveal(),
            ),
            Error::<Test>::NotKeyOwner
        );
        // The attacker cannot replace its commitment after observing the reveal.
        seed_overwatch_epoch_eligibility(copying_node);
        assert_noop!(
            Network::commit_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(202)),
                copying_node,
                vec![OverwatchCommit {
                    subnet_id,
                    weight: make_commit(copying_node, subnet_id, epoch, weight, salt.clone()),
                }],
            ),
            Error::<Test>::NotCommitPeriod
        );
        assert_eq!(stored_reveal(epoch, copying_node, subnet_id), None);
        assert_eq!(ActiveOverwatchRevealStats::<Test>::get(), stats);
    });
}

#[test]
fn commitments_cannot_be_replayed_across_subnets_or_epochs() {
    // Change exactly one bound context field in each case.
    for replay_in_next_epoch in [false, true] {
        new_test_ext().execute_with(|| {
            let epoch = Network::get_current_overwatch_epoch_as_u32();
            let subnet_id = 1;
            let other_subnet_id = 2;
            insert_subnet(subnet_id, SubnetState::Active, 0);
            insert_subnet(other_subnet_id, SubnetState::Active, 0);
            manual_insert_validator(1, 101, 201);
            let node = insert_overwatch_node(1, None);
            let weight = test_percent(1, 2);
            let salt = b"context-bound-salt".to_vec();
            let commitment = make_commit(node, subnet_id, epoch, weight, salt.clone());
            let target_subnet = if replay_in_next_epoch {
                subnet_id
            } else {
                other_subnet_id
            };
            let target_epoch = if replay_in_next_epoch {
                epoch + 1
            } else {
                epoch
            };
            set_overwatch_epoch(target_epoch);
            set_block_to_overwatch_commit_block(target_epoch);
            seed_overwatch_epoch_eligibility(node);
            assert_ok!(Network::commit_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(201)),
                node,
                vec![OverwatchCommit {
                    subnet_id: target_subnet,
                    weight: commitment
                }],
            ));
            set_block_to_overwatch_reveal_block(target_epoch);
            assert_noop!(
                Network::reveal_overwatch_subnet_weights(
                    RuntimeOrigin::signed(account(201)),
                    node,
                    vec![OverwatchReveal {
                        subnet_id: target_subnet,
                        weight,
                        salt: salt.try_into().unwrap(),
                    }],
                ),
                Error::<Test>::RevealMismatch
            );
            assert_eq!(stored_reveal(target_epoch, node, target_subnet), None);
        });
    }
}

#[test]
fn unbound_commitment_is_rejected_atomically_with_a_valid_reveal_prefix() {
    use sp_runtime::traits::Hash;

    new_test_ext().execute_with(|| {
        let epoch = Network::get_current_overwatch_epoch_as_u32();
        let node = 1;
        let weight = test_percent(1, 2);
        let salt = b"unbound-salt".to_vec();
        insert_commit(
            epoch,
            node,
            1,
            make_commit(node, 1, epoch, weight, salt.clone()),
        );
        insert_commit(epoch, node, 2, Hashing::hash_of(&(weight, salt.clone())));
        assert_noop!(
            Network::perform_reveal_overwatch_subnet_weights(
                node,
                vec![
                    OverwatchReveal {
                        subnet_id: 1,
                        weight,
                        salt: salt.clone().try_into().unwrap()
                    },
                    OverwatchReveal {
                        subnet_id: 2,
                        weight,
                        salt: salt.try_into().unwrap()
                    },
                ],
            ),
            Error::<Test>::RevealMismatch
        );
    });
}

#[test]
fn overwatch_commitment_matches_client_encoding_vector() {
    // Independently encoded SCALE preimage and Blake2b-256 digest, also in the client docs.
    let expected = sp_core::H256::from([
        0x4d, 0x77, 0x85, 0xea, 0x48, 0x19, 0x1e, 0x23, 0xd6, 0xce, 0xd1, 0x0e, 0xe0, 0x6e, 0x38,
        0x68, 0x05, 0x31, 0x83, 0xc3, 0x26, 0x65, 0xe9, 0x8e, 0x5d, 0xf1, 0x69, 0xf6, 0xb3, 0xfc,
        0xb4, 0x9c,
    ]);
    assert_eq!(
        Network::hash_overwatch_commitment(7, 11, 13, 123456, b"secret-salt"),
        expected,
    );
}

/// Subnet churn during the commit phase must not make an accepted commitment unrevealable,
/// and the removed subnet's reveals must not push finalization over the subnet bound.
#[test]
fn test_overwatch_subnet_churn_during_commit_keeps_full_revealers_eligible() {
    new_test_ext().execute_with(|| {
        let pf = Network::percentage_factor_as_u128();
        crate::OverwatchEpochLengthMultiplier::<Test>::set(1);
        crate::ActiveOverwatchEpochLengthMultiplier::<Test>::set(1);
        let bound = <Test as crate::Config>::MaxPhysicalSubnetsUpperBound::get();
        let min_stake = crate::OverwatchMinStakeBalance::<Test>::get().max(1);
        for s in 1..=bound {
            insert_subnet(s, SubnetState::Active, 0);
        }
        let mut hotkeys = BTreeMap::new();
        for v in 1..=2u32 {
            manual_insert_validator(v, 10_000 + v, 20_000 + v);
            OverwatchValidatorWhitelist::<Test>::insert(v, ());
            let id = insert_overwatch_node(v, None);
            set_overwatch_node_stake(id, min_stake);
            hotkeys.insert(id, account(20_000 + v));
        }
        let (a, b) = (1u32, 2u32);
        set_overwatch_epoch(1);
        let epoch = crate::CurrentOverwatchEpoch::<Test>::get();
        let start = crate::OverwatchEpochStartBlock::<Test>::get();
        let len = EpochLength::get() * crate::ActiveOverwatchEpochLengthMultiplier::<Test>::get();
        let salt = |n: u32, s: u32| format!("s-{n}-{s}").into_bytes();
        let commit = |n: u32, s: u32, w: u128| OverwatchCommit {
            subnet_id: s,
            weight: make_commit(n, s, epoch, w, salt(n, s)),
        };
        let reveal = |n: u32, s: u32, w: u128| OverwatchReveal::<Test> {
            subnet_id: s,
            weight: w,
            salt: salt(n, s).try_into().unwrap(),
        };

        // A commits to subnets 1 and 2; B commits to subnets 2..=bound.
        System::set_block_number(start + 1);
        assert_ok!(Network::commit_overwatch_subnet_weights(
            RuntimeOrigin::signed(hotkeys[&a].clone()),
            a,
            vec![commit(a, 1, pf), commit(a, 2, pf / 2)]
        ));

        // Still in the commit phase: subnet 1 is removed and a new subnet takes its slot.
        System::set_block_number(start + 10);
        Network::do_remove_subnet(1, crate::SubnetRemovalReason::Owner);
        let new_subnet = bound + 1;
        insert_subnet(new_subnet, SubnetState::Active, 0);
        assert_ok!(Network::commit_overwatch_subnet_weights(
            RuntimeOrigin::signed(hotkeys[&b].clone()),
            b,
            (2..=bound)
                .chain([new_subnet])
                .map(|s| commit(b, s, pf / 2))
                .collect()
        ));
        assert!(Network::in_overwatch_commit_period());

        // Reveal phase: A reveals its removed-subnet commitment first, then B reveals everything.
        System::set_block_number(start + len - 1);
        assert_ok!(Network::reveal_overwatch_subnet_weights(
            RuntimeOrigin::signed(hotkeys[&a].clone()),
            a,
            vec![reveal(a, 1, pf), reveal(a, 2, pf / 2)]
        ));
        assert_ok!(Network::reveal_overwatch_subnet_weights(
            RuntimeOrigin::signed(hotkeys[&b].clone()),
            b,
            (2..=bound)
                .chain([new_subnet])
                .map(|s| reveal(b, s, pf / 2))
                .collect()
        ));
        assert!(!ActiveOverwatchRevealStats::<Test>::get()
            .subnet_revealer_counts
            .contains_key(&1));

        System::set_block_number(start + len);
        Network::advance_overwatch_epoch(start + len);
        let eligible: Vec<u32> = crate::OverwatchEpochRevealEligibility::<Test>::get(epoch)
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(eligible, vec![a, b]);

        // Finalization skips the removed subnet, settles, and pays both nodes.
        let (stake_a, stake_b) = (
            crate::OverwatchNodeStakeBalance::<Test>::get(a),
            crate::OverwatchNodeStakeBalance::<Test>::get(b),
        );
        Network::calculate_overwatch_rewards();
        assert!(crate::PendingOverwatchSettlement::<Test>::get().is_none());
        assert_eq!(
            crate::LastFinalizedOverwatchEpoch::<Test>::get(),
            Some(epoch)
        );
        assert!(crate::OverwatchNodeStakeBalance::<Test>::get(a) > stake_a);
        assert!(crate::OverwatchNodeStakeBalance::<Test>::get(b) > stake_b);
        assert!(!crate::OverwatchSubnetWeights::<Test>::contains_key(
            epoch, 1
        ));

        // The next rollover is not blocked by a stuck settlement.
        System::set_block_number(start + 2 * len);
        Network::advance_overwatch_epoch(start + 2 * len);
        assert_eq!(crate::CurrentOverwatchEpoch::<Test>::get(), epoch + 2);
    });
}

fn single_call_fixture(
    count: u32,
) -> (
    u32,
    Vec<OverwatchCommit<sp_core::H256>>,
    Vec<OverwatchReveal<Test>>,
) {
    manual_insert_validator(1, 101, 201);
    let node = insert_overwatch_node(1, None);
    set_overwatch_epoch(1);
    seed_overwatch_epoch_eligibility(node);
    set_block_to_overwatch_commit_block(1);
    let mut commits = Vec::new();
    let mut reveals = Vec::new();
    for subnet_id in 1..=count {
        insert_subnet(subnet_id, SubnetState::Active, 0);
        let salt = vec![
            subnet_id as u8;
            <Test as crate::Config>::MaxOverwatchRevealSaltLength::get() as usize
        ];
        let weight = subnet_id as u128;
        commits.push(OverwatchCommit {
            subnet_id,
            weight: make_commit(node, subnet_id, 1, weight, salt.clone()),
        });
        reveals.push(OverwatchReveal {
            subnet_id,
            weight,
            salt: salt.try_into().unwrap(),
        });
    }
    (node, commits, reveals)
}

#[test]
fn invalid_commit_batches_do_not_consume_the_epoch_submission() {
    use frame_support::dispatch::Pays;
    new_test_ext().execute_with(|| {
        let (node, commits, _) = single_call_fixture(3);
        let mut unknown = commits.clone();
        unknown.last_mut().unwrap().subnet_id = 999;
        let mut duplicate = commits.clone();
        duplicate.push(commits[0].clone());
        let oversized = vec![
            commits[0].clone();
            <Test as crate::Config>::MaxPhysicalSubnetsUpperBound::get() as usize
                + 1
        ];
        for (batch, expected) in [
            (vec![], Error::<Test>::CommitsEmpty),
            (unknown, Error::<Test>::InvalidSubnetId),
            (duplicate, Error::<Test>::DuplicateOverwatchSubnetId),
            (oversized, Error::<Test>::MaxSubnets),
        ] {
            assert_noop!(
                Network::perform_commit_overwatch_subnet_weights(node, batch.clone()),
                expected
            );
            let err = Network::perform_commit_overwatch_subnet_weights(node, batch).unwrap_err();
            assert_eq!(err.post_info.pays_fee, Pays::Yes);
            assert!(!OverwatchCommits::<Test>::contains_key(1, node));
        }
        assert_eq!(
            Network::perform_commit_overwatch_subnet_weights(node, commits)
                .unwrap()
                .pays_fee,
            Pays::No
        );
    });
}

#[test]
fn invalid_reveal_batches_are_atomic_and_allow_a_corrected_retry() {
    new_test_ext().execute_with(|| {
        let (node, commits, reveals) = single_call_fixture(3);
        assert_noop!(
            Network::perform_reveal_overwatch_subnet_weights(node, reveals.clone()),
            Error::<Test>::NoCommitFound
        );
        assert_ok!(Network::perform_commit_overwatch_subnet_weights(
            node, commits
        ));
        let mut extra = reveals.clone();
        extra.push(OverwatchReveal {
            subnet_id: 999,
            ..reveals[0].clone()
        });
        let mut wrong_set = reveals.clone();
        wrong_set[2].subnet_id = 999;
        let mut duplicate = reveals.clone();
        duplicate[2] = reveals[0].clone();
        let mut invalid_weight = reveals.clone();
        invalid_weight[2].weight = crate::PERCENTAGE_FACTOR_U128 + 1;
        let mut invalid_hash = reveals.clone();
        invalid_hash[2].weight += 1;
        let oversized = vec![
            reveals[0].clone();
            <Test as crate::Config>::MaxPhysicalSubnetsUpperBound::get() as usize
                + 1
        ];
        for (batch, expected) in [
            (vec![], Error::<Test>::RevealsEmpty),
            (reveals[..2].to_vec(), Error::<Test>::RevealSetMismatch),
            (extra, Error::<Test>::RevealSetMismatch),
            (wrong_set, Error::<Test>::RevealSetMismatch),
            (duplicate, Error::<Test>::DuplicateOverwatchSubnetId),
            (invalid_weight, Error::<Test>::InvalidWeight),
            (invalid_hash, Error::<Test>::RevealMismatch),
            (oversized, Error::<Test>::MaxSubnets),
        ] {
            assert_noop!(
                Network::perform_reveal_overwatch_subnet_weights(node, batch),
                expected
            );
            assert!(!OverwatchReveals::<Test>::contains_key(1, node));
        }
        // A failure after all hashes have been validated must also leave no success marker.
        let max_records = <Test as crate::Config>::MaxOverwatchNodesUpperBound::get()
            * <Test as crate::Config>::MaxPhysicalSubnetsUpperBound::get();
        ActiveOverwatchRevealStats::<Test>::mutate(|stats| stats.records = max_records);
        assert_noop!(
            Network::perform_reveal_overwatch_subnet_weights(node, reveals.clone()),
            Error::<Test>::MaxOverwatchRevealRecords
        );
        ActiveOverwatchRevealStats::<Test>::kill();
        assert_ok!(Network::perform_reveal_overwatch_subnet_weights(
            node, reveals
        ));
        assert_eq!(ActiveOverwatchRevealStats::<Test>::get().records, 3);
    });
}

#[test]
fn maximum_complete_reveal_survives_subnet_removal_and_lowered_capacity() {
    new_test_ext().execute_with(|| {
        let bound = <Test as crate::Config>::MaxPhysicalSubnetsUpperBound::get();
        let (node, commits, reveals) = single_call_fixture(bound);
        assert_ok!(Network::commit_overwatch_subnet_weights(
            RuntimeOrigin::signed(account(201)),
            node,
            commits
        ));
        SubnetsData::<Test>::remove(bound);
        crate::MaxSubnets::<Test>::put(1);
        set_block_to_overwatch_reveal_block(1);
        assert_ok!(Network::reveal_overwatch_subnet_weights(
            RuntimeOrigin::signed(account(201)),
            node,
            reveals
        ));
        assert_eq!(OverwatchReveals::<Test>::get(1, node).len(), bound as usize);
        let stats = ActiveOverwatchRevealStats::<Test>::get();
        assert_eq!(stats.records, bound);
        assert!(!stats.subnet_revealer_counts.contains_key(&bound));
    });
}

#[test]
fn pause_resume_does_not_reopen_successful_overwatch_submissions() {
    new_test_ext().execute_with(|| {
        let (node, commits, reveals) = single_call_fixture(1);
        assert_ok!(Network::commit_overwatch_subnet_weights(
            RuntimeOrigin::signed(account(201)),
            node,
            commits.clone()
        ));
        assert_ok!(Network::do_pause());
        assert_ok!(Network::do_unpause());
        assert_noop!(
            Network::commit_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(201)),
                node,
                commits
            ),
            Error::<Test>::AlreadyCommitted
        );
        set_block_to_overwatch_reveal_block(1);
        assert_ok!(Network::reveal_overwatch_subnet_weights(
            RuntimeOrigin::signed(account(201)),
            node,
            reveals.clone()
        ));
        assert_ok!(Network::do_pause());
        assert_ok!(Network::do_unpause());
        assert_noop!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(201)),
                node,
                reveals
            ),
            Error::<Test>::AlreadyRevealed
        );
    });
}
