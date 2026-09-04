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
use frame_support::{assert_err, assert_ok};
use sp_std::collections::btree_map::BTreeMap;

fn insert_commit(epoch: u32, node_id: u32, subnet_id: u32, hash: sp_core::H256) {
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
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
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
            pause: None,
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
        let commit_hash = make_commit(weight, salt.clone());

        // Commit
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
        let commit_hash = make_commit(weight, salt.clone());

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
            Error::<Test>::NoCommitFound
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

        let existing_hash = make_commit(10, b"existing".to_vec());
        let new_hash = make_commit(20, b"new".to_vec());
        insert_commit(epoch, node_id, duplicate_subnet_id, existing_hash);

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
fn test_repeated_commit_calls_respect_cumulative_subnet_bound() {
    new_test_ext().execute_with(|| {
        let epoch = Network::get_current_overwatch_epoch_as_u32();
        let node_id = 1;
        let max_subnets = <Test as crate::Config>::MaxPhysicalSubnetsUpperBound::get();

        for subnet_id in 1..=max_subnets.saturating_add(1) {
            insert_subnet(subnet_id, SubnetState::Active, 0);
        }

        for subnet_id in 1..=max_subnets {
            assert_ok!(Network::perform_commit_overwatch_subnet_weights(
                node_id,
                vec![OverwatchCommit {
                    subnet_id,
                    weight: make_commit(subnet_id as u128, vec![subnet_id as u8]),
                }],
            ));
        }

        let row_before_failure = OverwatchCommits::<Test>::get(epoch, node_id);
        let overflow_subnet_id = max_subnets.saturating_add(1);
        assert_err!(
            Network::perform_commit_overwatch_subnet_weights(
                node_id,
                vec![OverwatchCommit {
                    subnet_id: overflow_subnet_id,
                    weight: make_commit(
                        overflow_subnet_id as u128,
                        vec![overflow_subnet_id as u8],
                    ),
                }],
            ),
            Error::<Test>::MaxSubnets
        );

        assert_eq!(OverwatchCommits::<Test>::get(epoch, node_id), row_before_failure);
        assert_eq!(stored_commit(epoch, node_id, overflow_subnet_id), None);
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
        let commit_hash = make_commit(weight, salt.clone());
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
        let commit_hash = make_commit(weight, salt.clone());
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
        let first_node_id = insert_overwatch_node_v2(1);
        let second_node_id = insert_overwatch_node_v2(2);

        let first_shared_weight = test_percent(1, 4);
        let first_shared_salt = b"first-shared".to_vec();
        let first_sole_weight = test_percent(1, 3);
        let first_sole_salt = b"first-sole".to_vec();
        let second_shared_weight = test_percent(3, 4);
        let second_shared_salt = b"second-shared".to_vec();

        assert_ok!(Network::perform_commit_overwatch_subnet_weights(
            first_node_id,
            vec![
                OverwatchCommit {
                    subnet_id: shared_subnet_id,
                    weight: make_commit(first_shared_weight, first_shared_salt.clone()),
                },
                OverwatchCommit {
                    subnet_id: sole_subnet_id,
                    weight: make_commit(first_sole_weight, first_sole_salt.clone()),
                },
            ],
        ));
        assert_ok!(Network::perform_commit_overwatch_subnet_weights(
            second_node_id,
            vec![OverwatchCommit {
                subnet_id: shared_subnet_id,
                weight: make_commit(second_shared_weight, second_shared_salt.clone()),
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

        // Replaying an already-recorded reveal may replace its value but cannot grow either
        // bounded cardinality counter.
        assert_ok!(Network::perform_reveal_overwatch_subnet_weights(
            second_node_id,
            vec![OverwatchReveal {
                subnet_id: shared_subnet_id,
                weight: second_shared_weight,
                salt: second_shared_salt.try_into().unwrap(),
            }],
        ));
        assert_eq!(ActiveOverwatchRevealStats::<Test>::get(), stats_after_both);

        assert_ok!(Network::perform_remove_overwatch_node(first_node_id));

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
            pause: None,
        };

        SubnetsData::<Test>::insert(subnet_id, subnet_data);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
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
        let commit_hash = make_commit(weight, salt.clone());

        // Commit
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
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
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
            pause: None,
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
        let commit_hash = make_commit(weight, salt.clone());

        // Commit
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
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
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
            pause: None,
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
        let commit_hash = make_commit(weight, salt.clone());

        // Commit
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
            pause: None,
        };

        SubnetsData::<Test>::insert(subnet_id, subnet_data);

        // Weight + salt
        let weight: u128 = 123456;
        let salt: Vec<u8> = b"secret-salt".to_vec();
        let commit_hash = make_commit(weight, salt.clone());

        // Commit
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
            pause: None,
        };

        SubnetsData::<Test>::insert(subnet_id, subnet_data);

        // Weight + salt
        let weight: u128 = 123456;
        let salt: Vec<u8> = b"secret-salt".to_vec();
        let commit_hash = make_commit(weight, salt.clone());

        // Commit
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
            pause: None,
        };

        SubnetsData::<Test>::insert(subnet_id, subnet_data);

        // Weight + salt
        let weight: u128 = 123456;
        let salt: Vec<u8> = b"secret-salt".to_vec();
        let commit_hash = make_commit(weight, salt.clone());

        set_block_to_overwatch_reveal_block(overwatch_epoch);

        // Reveal
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
            pause: None,
        };

        SubnetsData::<Test>::insert(subnet_id, subnet_data);

        // Weight + salt
        let weight: u128 = 123456;
        let salt: Vec<u8> = b"secret-salt".to_vec();
        let commit_hash = make_commit(weight, salt.clone());

        let fake_salt: Vec<u8> = b"fake-salt".to_vec();

        // Commit
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
fn test_commit_reveal_multiple_times_in_same_epoch() {
    new_test_ext().execute_with(|| {
        // Subnet 1
        let subnet_id_1 = 1;
        let subnet_data = SubnetData {
            id: subnet_id_1,
            friendly_id: subnet_id_1,
            name: "subnet_name_1".into(),
            repo: "github-1".into(),
            description: "description-1".into(),
            misc: "misc-1".into(),
            consensus_mechanism: Default::default(),
            state: SubnetState::Active,
            consensus_eligible_from_subnet_epoch: Some(0),
            pause: None,
        };
        SubnetsData::<Test>::insert(subnet_id_1, subnet_data);
        // Subnet 2
        let subnet_id_2 = 2;
        let subnet_data = SubnetData {
            id: subnet_id_2,
            friendly_id: subnet_id_2,
            name: "subnet_name_2".into(),
            repo: "github-2".into(),
            description: "description-2".into(),
            misc: "misc-2".into(),
            consensus_mechanism: Default::default(),
            state: SubnetState::Active,
            consensus_eligible_from_subnet_epoch: Some(0),
            pause: None,
        };
        SubnetsData::<Test>::insert(subnet_id_2, subnet_data);

        let coldkey: AccountId = account(1);
        let hotkey: AccountId = account(2);

        let reward_rate = test_percent(1, 20); // 5%
        assert_ok!(Network::do_register_validator(
            RuntimeOrigin::signed(coldkey.clone()),
            hotkey,
            reward_rate,
            None,
            None,
        ));

        let validator_id = TotalValidatorIds::<Test>::get();
        OverwatchValidatorWhitelist::<Test>::insert(validator_id, ());

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

        // Weight + salt
        // Subnet 1
        let weight_1: u128 = 123456;
        let salt_1: Vec<u8> = b"secret-salt-1".to_vec();
        let commit_hash_1 = make_commit(weight_1, salt_1.clone());
        // Subnet 2
        let weight_2: u128 = 78910;
        let salt_2: Vec<u8> = b"secret-salt-2".to_vec();
        let commit_hash_2 = make_commit(weight_2, salt_2.clone());

        let overwatch_epoch = Network::get_current_overwatch_epoch_as_u32();

        // Commit
        assert_ok!(Network::commit_overwatch_subnet_weights(
            RuntimeOrigin::signed(hotkey.clone()),
            overwatch_node_id,
            vec![OverwatchCommit {
                subnet_id: subnet_id_1,
                weight: commit_hash_1
            }]
        ));

        // Ensure it's stored
        assert_eq!(
            stored_commit(overwatch_epoch, overwatch_node_id, subnet_id_1),
            Some(commit_hash_1)
        );
        assert_eq!(
            stored_commit(overwatch_epoch, overwatch_node_id, subnet_id_2),
            None
        );

        assert_ok!(Network::commit_overwatch_subnet_weights(
            RuntimeOrigin::signed(hotkey.clone()),
            overwatch_node_id,
            vec![OverwatchCommit {
                subnet_id: subnet_id_2,
                weight: commit_hash_2
            }]
        ));

        assert_eq!(
            stored_commit(overwatch_epoch, overwatch_node_id, subnet_id_1),
            Some(commit_hash_1)
        );
        assert_eq!(
            stored_commit(overwatch_epoch, overwatch_node_id, subnet_id_2),
            Some(commit_hash_2)
        );

        set_block_to_overwatch_reveal_block(overwatch_epoch);

        assert_ok!(Network::reveal_overwatch_subnet_weights(
            RuntimeOrigin::signed(hotkey.clone()),
            overwatch_node_id,
            vec![OverwatchReveal {
                subnet_id: subnet_id_1,
                weight: weight_1,
                salt: salt_1.try_into().unwrap()
            }]
        ));

        // Ensure revealed weight is correct
        assert_eq!(
            stored_reveal(overwatch_epoch, overwatch_node_id, subnet_id_1),
            Some(weight_1)
        );
        assert_eq!(
            stored_reveal(overwatch_epoch, overwatch_node_id, subnet_id_2),
            None
        );

        assert_ok!(Network::reveal_overwatch_subnet_weights(
            RuntimeOrigin::signed(hotkey.clone()),
            overwatch_node_id,
            vec![OverwatchReveal {
                subnet_id: subnet_id_2,
                weight: weight_2,
                salt: salt_2.try_into().unwrap()
            }]
        ));

        // Ensure revealed weight is correct
        assert_eq!(
            stored_reveal(overwatch_epoch, overwatch_node_id, subnet_id_1),
            Some(weight_1)
        );
        assert_eq!(
            stored_reveal(overwatch_epoch, overwatch_node_id, subnet_id_2),
            Some(weight_2)
        );
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
        let commit_hash = make_commit(weight, salt.clone());

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
