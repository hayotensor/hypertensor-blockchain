use super::{mock::*, test_utils::*};
use crate::*;
use frame_support::{
    assert_noop, assert_ok,
    traits::{Currency, OnInitialize},
};

fn tick(block: u32) {
    System::set_block_number(block);
    Network::on_initialize(block);
}

fn register(id: u32, stake: u128) -> u32 {
    manual_insert_validator(id, 100 + id, 200 + id);
    OverwatchValidatorWhitelist::<Test>::insert(id, ());
    let _ = Balances::deposit_creating(&account(100 + id), 100_000);
    assert_ok!(Network::register_overwatch_node(
        RuntimeOrigin::signed(account(100 + id)),
        stake
    ));
    ValidatorOverwatchNodeId::<Test>::get(id).unwrap()
}

/// Use actual rollover hooks: register during epoch 1, then open epoch 2.
fn setup() -> (u32, u32, u32) {
    OverwatchEpochLengthMultiplier::<Test>::put(1);
    ActiveOverwatchEpochLengthMultiplier::<Test>::put(1);
    OverwatchCommitCutoffPercent::<Test>::put(PERCENTAGE_FACTOR_U128 / 2);
    OverwatchMinStakeBalance::<Test>::put(1);
    OverwatchStakeWeightFactor::<Test>::put(PERCENTAGE_FACTOR_U128);
    let length = EpochLength::get();
    tick(length);
    tick(length + 1);
    let a = register(1, 1_000);
    let b = register(2, 1_000);
    tick(2 * length);
    tick(2 * length + 1);
    insert_subnet(1, SubnetState::Active, 0);
    assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 2);
    (a, b, length)
}

fn commit(node: u32, validator: u32, rating: u128) {
    commit_subnet(node, validator, 1, rating);
}

fn commit_subnet(node: u32, validator: u32, subnet_id: u32, rating: u128) {
    let epoch = CurrentOverwatchEpoch::<Test>::get();
    let hash = Network::hash_overwatch_commitment(node, subnet_id, epoch, rating, b"opening-test");
    assert_ok!(Network::commit_overwatch_subnet_weights(
        RuntimeOrigin::signed(account(200 + validator)),
        node,
        vec![OverwatchCommit {
            subnet_id,
            weight: hash
        }],
    ));
}

fn reveal(node: u32, validator: u32, rating: u128) {
    reveal_subnet(node, validator, 1, rating);
}

fn reveal_subnet(node: u32, validator: u32, subnet_id: u32, rating: u128) {
    assert_ok!(Network::reveal_overwatch_subnet_weights(
        RuntimeOrigin::signed(account(200 + validator)),
        node,
        vec![OverwatchReveal {
            subnet_id,
            weight: rating,
            salt: b"opening-test".to_vec().try_into().unwrap()
        }],
    ));
}

#[test]
fn opening_stake_and_exponent_survive_changes_before_commit_during_reveal_and_after_close() {
    new_test_ext().execute_with(|| {
        let (a, b, length) = setup();
        let opening = OverwatchEpochSnapshots::<Test>::get(2).unwrap();
        assert_ok!(Network::add_overwatch_node_stake(
            RuntimeOrigin::signed(account(101)),
            a,
            1_000
        ));
        assert_ok!(Network::remove_overwatch_node_stake(
            RuntimeOrigin::signed(account(102)),
            b,
            100
        ));
        commit(a, 1, PERCENTAGE_FACTOR_U128);
        commit(b, 2, 0);
        tick(2 * length + length / 2);
        reveal(a, 1, PERCENTAGE_FACTOR_U128);
        reveal(b, 2, 0);
        assert_ok!(Network::add_overwatch_node_stake(
            RuntimeOrigin::signed(account(102)),
            b,
            100
        ));
        assert_ok!(Network::remove_overwatch_node_stake(
            RuntimeOrigin::signed(account(102)),
            b,
            900
        ));
        OverwatchStakeWeightFactor::<Test>::put(PERCENTAGE_FACTOR_U128 * 9 / 10);
        assert_eq!(OverwatchEpochSnapshots::<Test>::get(2).unwrap(), opening);
        tick(3 * length);
        assert_eq!(OverwatchEpochSnapshots::<Test>::get(2).unwrap(), opening);
        let next = OverwatchEpochSnapshots::<Test>::get(3).unwrap();
        assert_eq!(next.nodes[&a].stake, 2_000);
        assert_eq!(next.nodes[&b].stake, 100);
        assert_eq!(next.stake_weight_factor, PERCENTAGE_FACTOR_U128 * 9 / 10);
        assert_ok!(Network::add_overwatch_node_stake(
            RuntimeOrigin::signed(account(101)),
            a,
            1_000
        ));
        assert_ok!(Network::remove_overwatch_node_stake(
            RuntimeOrigin::signed(account(102)),
            b,
            50
        ));
        tick(3 * length + 1);
        assert_eq!(
            OverwatchSubnetWeights::<Test>::get(2, 1),
            Some(PERCENTAGE_FACTOR_U128 / 2)
        );
        assert_eq!(OverwatchEpochSnapshots::<Test>::get(3).unwrap(), next);
        assert_eq!(OverwatchEpochSnapshots::<Test>::iter().count(), 1);
        tick(4 * length);
        assert_eq!(
            OverwatchEpochSnapshots::<Test>::get(4).unwrap().nodes[&a].stake,
            OverwatchNodeStakeBalance::<Test>::get(a)
        );
    });
}

#[test]
fn mid_epoch_registration_waits_for_next_snapshot_and_missing_snapshot_fails_closed() {
    new_test_ext().execute_with(|| {
        let (a, _, length) = setup();
        let c = register(3, 500);
        let row = vec![OverwatchCommit {
            subnet_id: 1,
            weight: Default::default(),
        }];
        assert_noop!(
            Network::commit_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(203)),
                c,
                row.clone()
            ),
            Error::<Test>::OverwatchNodeNotEligibleForEpoch
        );
        let snapshot = OverwatchEpochSnapshots::<Test>::take(2).unwrap();
        assert_noop!(
            Network::commit_overwatch_subnet_weights(RuntimeOrigin::signed(account(201)), a, row),
            Error::<Test>::MissingOverwatchEpochSnapshot
        );
        tick(2 * length + 1);
        tick(3 * length);
        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 2);
        assert!(PendingOverwatchSettlement::<Test>::get().is_none());
        assert!(!OverwatchEpochSnapshots::<Test>::contains_key(3));
        OverwatchEpochSnapshots::<Test>::insert(2, snapshot);
        tick(3 * length);
        insert_subnet(1, SubnetState::Active, 0);
        commit(c, 3, PERCENTAGE_FACTOR_U128);
    });
}

#[test]
fn voluntary_exit_preserves_assessment_and_reward_at_every_settlement_stage() {
    for stage in 0..3 {
        new_test_ext().execute_with(|| {
            let (a, b, length) = setup();
            commit(a, 1, PERCENTAGE_FACTOR_U128);
            commit(b, 2, 0);
            tick(2 * length + length / 2);
            reveal(a, 1, PERCENTAGE_FACTOR_U128);
            reveal(b, 2, 0);
            if stage >= 1 {
                tick(3 * length);
            }
            if stage == 2 {
                tick(3 * length + 1);
            }
            let cached = LatestEffectiveOverwatchSignal::<Test>::get();
            let revision = LatestOverwatchSignalRevision::<Test>::get();
            assert_ok!(Network::remove_overwatch_node(
                RuntimeOrigin::signed(account(101)),
                a
            ));
            assert_eq!(LatestEffectiveOverwatchSignal::<Test>::get(), cached);
            assert_eq!(LatestOverwatchSignalRevision::<Test>::get(), revision);
            assert_ok!(Network::remove_overwatch_node_stake(
                RuntimeOrigin::signed(account(101)),
                a,
                1_000
            ));
            if stage == 0 {
                tick(3 * length);
            }
            if stage < 2 {
                tick(3 * length + 1);
            }
            assert_eq!(
                OverwatchSubnetWeights::<Test>::get(2, 1),
                Some(PERCENTAGE_FACTOR_U128 / 2)
            );
            assert_eq!(
                OverwatchNodeWeights::<Test>::get(2, a),
                OverwatchNodeWeights::<Test>::get(2, b)
            );
            assert!(OverwatchNodeStakeBalance::<Test>::get(a) > 0);
            let earned = OverwatchNodeStakeBalance::<Test>::get(a);
            assert_ok!(Network::remove_overwatch_node_stake(
                RuntimeOrigin::signed(account(101)),
                a,
                earned
            ));
            assert_eq!(OverwatchNodeStakeBalance::<Test>::get(a), 0);
            assert_noop!(
                Network::do_reveal_overwatch_subnet_weights(
                    RuntimeOrigin::signed(account(201)),
                    a,
                    vec![]
                ),
                Error::<Test>::InvalidOverwatchNodeId
            );
        });
    }
}

#[test]
fn governance_can_disqualify_departed_work_without_removing_replacement() {
    new_test_ext().execute_with(|| {
        let (a, b, length) = setup();
        commit(a, 1, PERCENTAGE_FACTOR_U128);
        commit(b, 2, 0);
        tick(2 * length + length / 2);
        reveal(a, 1, PERCENTAGE_FACTOR_U128);
        reveal(b, 2, 0);
        assert_ok!(Network::remove_overwatch_node(
            RuntimeOrigin::signed(account(101)),
            a
        ));
        OverwatchValidatorWhitelist::<Test>::insert(1, ());
        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(account(101)),
            100
        ));
        let replacement = ValidatorOverwatchNodeId::<Test>::get(1).unwrap();
        assert_noop!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(201)),
                replacement,
                vec![OverwatchReveal {
                    subnet_id: 1,
                    weight: PERCENTAGE_FACTOR_U128,
                    salt: b"opening-test".to_vec().try_into().unwrap(),
                }],
            ),
            Error::<Test>::OverwatchNodeNotEligibleForEpoch
        );
        assert_eq!(OverwatchReveals::<Test>::get(2, a).len(), 1);
        tick(3 * length);
        assert!(OverwatchEpochSnapshots::<Test>::get(3)
            .unwrap()
            .nodes
            .contains_key(&replacement));
        assert_ok!(Network::do_collective_remove_overwatch_node(a));
        assert_eq!(ValidatorOverwatchNodeId::<Test>::get(1), Some(replacement));
        assert!(OverwatchValidatorWhitelist::<Test>::contains_key(1));
        assert_eq!(TotalOverwatchNodes::<Test>::get(), 2);
        assert!(!OverwatchEpochSnapshots::<Test>::get(2)
            .unwrap()
            .nodes
            .contains_key(&a));
        tick(3 * length + 1);
        assert_eq!(OverwatchSubnetWeights::<Test>::get(2, 1), Some(0));
        assert!(OverwatchNodeWeights::<Test>::get(2, a).is_none());
        assert_eq!(OverwatchNodeStakeBalance::<Test>::get(a), 1_000);
    });
}

#[test]
fn governance_disqualifies_only_retained_work_after_exit_without_clawing_back_rewards() {
    new_test_ext().execute_with(|| {
        let (a, b, length) = setup();
        commit(a, 1, PERCENTAGE_FACTOR_U128);
        commit(b, 2, 0);
        tick(2 * length + length / 2);
        reveal(a, 1, PERCENTAGE_FACTOR_U128);
        reveal(b, 2, 0);
        assert_ok!(Network::remove_overwatch_node(
            RuntimeOrigin::signed(account(101)),
            a
        ));
        assert_eq!(OverwatchCommits::<Test>::get(2, a).len(), 1);
        tick(3 * length);
        tick(3 * length + 1);
        assert!(!OverwatchEpochSnapshots::<Test>::get(3)
            .unwrap()
            .nodes
            .contains_key(&a));
        let paid = OverwatchNodeStakeBalance::<Test>::get(a);
        let historical = OverwatchSubnetWeights::<Test>::get(2, 1);
        assert_ok!(Network::do_collective_remove_overwatch_node(a));
        assert_eq!(OverwatchNodeStakeBalance::<Test>::get(a), paid);
        assert_eq!(OverwatchSubnetWeights::<Test>::get(2, 1), historical);
        assert_eq!(
            LatestEffectiveOverwatchSignal::<Test>::get()
                .unwrap()
                .subnet_weights[&1],
            0
        );
    });
}

#[test]
fn genesis_opens_an_explicit_empty_snapshot_and_registration_stays_disabled_in_epoch_zero() {
    new_test_ext().execute_with(|| {
        let opening = OverwatchEpochSnapshots::<Test>::get(0).unwrap();
        assert!(opening.nodes.is_empty());
        assert_eq!(
            opening.stake_weight_factor,
            OverwatchStakeWeightFactor::<Test>::get()
        );
        manual_insert_validator(1, 101, 201);
        OverwatchValidatorWhitelist::<Test>::insert(1, ());
        assert_noop!(
            Network::register_overwatch_node(RuntimeOrigin::signed(account(101)), 100),
            Error::<Test>::OverwatchEpochIsZero
        );
    });
}

#[test]
fn delayed_settlement_and_pause_never_resample_opening_economics() {
    new_test_ext().execute_with(|| {
        let (a, b, length) = setup();
        commit(a, 1, PERCENTAGE_FACTOR_U128);
        commit(b, 2, 0);
        tick(2 * length + length / 2);
        reveal(a, 1, PERCENTAGE_FACTOR_U128);
        reveal(b, 2, 0);
        tick(3 * length);
        let opening = OverwatchEpochSnapshots::<Test>::take(2).unwrap();
        let next = OverwatchEpochSnapshots::<Test>::get(3).unwrap();
        let pending = PendingOverwatchSettlement::<Test>::get();
        let signal = LatestEffectiveOverwatchSignal::<Test>::get();
        assert_ok!(Network::add_overwatch_node_stake(
            RuntimeOrigin::signed(account(101)),
            a,
            2_000
        ));
        assert_ok!(Network::remove_overwatch_node_stake(
            RuntimeOrigin::signed(account(102)),
            b,
            900
        ));
        OverwatchStakeWeightFactor::<Test>::put(PERCENTAGE_FACTOR_U128 * 9 / 10);
        tick(3 * length + 1);
        assert_eq!(PendingOverwatchSettlement::<Test>::get(), pending);
        assert_eq!(LatestEffectiveOverwatchSignal::<Test>::get(), signal);
        assert_eq!(OverwatchNodeStakeBalance::<Test>::get(a), 3_000);
        assert_eq!(OverwatchReveals::<Test>::get(2, a).len(), 1);
        tick(4 * length);
        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 3);
        assert!(!OverwatchEpochSnapshots::<Test>::contains_key(4));
        assert_eq!(OverwatchEpochSnapshots::<Test>::get(3).unwrap(), next);

        OverwatchEpochSnapshots::<Test>::insert(2, opening.clone());
        assert_ok!(Network::do_pause());
        tick(4 * length + 1);
        assert_eq!(PendingOverwatchSettlement::<Test>::get(), pending);
        assert_eq!(OverwatchEpochSnapshots::<Test>::get(2).unwrap(), opening);
        assert_ok!(Network::do_unpause());
        tick(5 * length);
        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 3);
        tick(5 * length + 1);
        assert_eq!(
            OverwatchSubnetWeights::<Test>::get(2, 1),
            Some(PERCENTAGE_FACTOR_U128 / 2)
        );
        assert_eq!(OverwatchEpochSnapshots::<Test>::get(3).unwrap(), next);
        tick(6 * length);
        let following = OverwatchEpochSnapshots::<Test>::get(4).unwrap();
        assert_eq!(
            following.nodes[&a].stake,
            OverwatchNodeStakeBalance::<Test>::get(a)
        );
        assert_eq!(
            following.nodes[&b].stake,
            OverwatchNodeStakeBalance::<Test>::get(b)
        );
        assert_eq!(
            following.stake_weight_factor,
            PERCENTAGE_FACTOR_U128 * 9 / 10
        );
    });
}

#[test]
fn hotkey_rotation_preserves_snapshot_eligibility_and_missing_snapshot_rejects_reveal() {
    new_test_ext().execute_with(|| {
        let (a, _, length) = setup();
        commit(a, 1, PERCENTAGE_FACTOR_U128);
        assert_ok!(Network::update_overwatch_hotkey(
            RuntimeOrigin::signed(account(101)),
            a,
            Some(account(999))
        ));
        tick(2 * length + length / 2);
        let row = vec![OverwatchReveal {
            subnet_id: 1,
            weight: PERCENTAGE_FACTOR_U128,
            salt: b"opening-test".to_vec().try_into().unwrap(),
        }];
        assert_noop!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(201)),
                a,
                row.clone()
            ),
            Error::<Test>::NotKeyOwner
        );
        let snapshot = OverwatchEpochSnapshots::<Test>::take(2).unwrap();
        assert_noop!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(999)),
                a,
                row.clone()
            ),
            Error::<Test>::MissingOverwatchEpochSnapshot
        );
        OverwatchEpochSnapshots::<Test>::insert(2, snapshot.clone());
        assert_ok!(Network::reveal_overwatch_subnet_weights(
            RuntimeOrigin::signed(account(999)),
            a,
            row
        ));
        assert_eq!(OverwatchEpochSnapshots::<Test>::get(2).unwrap(), snapshot);
        tick(3 * length);
        tick(3 * length + 1);
        assert_eq!(
            OverwatchSubnetWeights::<Test>::get(2, 1),
            Some(PERCENTAGE_FACTOR_U128)
        );
    });
}

#[test]
fn missing_one_reveal_excludes_every_assessment_before_averaging_and_rewards() {
    // Compare selective withholding with the same evaluator submitting no work at all.
    let mut absent_result = None;
    for participates in [false, true] {
        new_test_ext().execute_with(|| {
            let (a, b, length) = setup();
            insert_subnet(2, SubnetState::Active, 0);
            if participates {
                commit(a, 1, PERCENTAGE_FACTOR_U128);
                commit_subnet(a, 1, 2, 0);
            }
            commit(b, 2, PERCENTAGE_FACTOR_U128 / 4);
            tick(2 * length + length / 2);
            if participates {
                reveal(a, 1, PERCENTAGE_FACTOR_U128);
                // Replaying a reveal cannot substitute for revealing the other commitment.
                reveal(a, 1, PERCENTAGE_FACTOR_U128);
            }
            reveal(b, 2, PERCENTAGE_FACTOR_U128 / 4);
            tick(3 * length);
            let eligible = OverwatchEpochRevealEligibility::<Test>::get(2).unwrap();
            assert_eq!(eligible.iter().copied().collect::<Vec<_>>(), vec![b]);
            // Cleanup must still be charged for the excluded node's accepted reveal.
            assert_eq!(
                PendingOverwatchSettlement::<Test>::get()
                    .unwrap()
                    .reveal_records,
                if participates { 2 } else { 1 }
            );
            assert!(OverwatchCommits::<Test>::iter_prefix(2).next().is_none());
            tick(3 * length + 1);
            assert_eq!(OverwatchNodeStakeBalance::<Test>::get(a), 1_000);
            assert_eq!(OverwatchNodeWeights::<Test>::get(2, a), None);
            assert_eq!(OverwatchSubnetWeights::<Test>::get(2, 2), None);
            assert_eq!(
                OverwatchSubnetWeights::<Test>::get(2, 1),
                Some(PERCENTAGE_FACTOR_U128 / 4)
            );
            let result = (
                OverwatchNodeStakeBalance::<Test>::get(b),
                OverwatchNodeWeights::<Test>::get(2, b),
                LatestEffectiveOverwatchSignal::<Test>::get(),
                LatestFinalizedOverwatchSignalInputs::<Test>::get(),
            );
            if let Some(expected) = &absent_result {
                assert_eq!(&result, expected);
            } else {
                absent_result = Some(result);
            }
            assert!(OverwatchReveals::<Test>::iter_prefix(2).next().is_none());
            assert!(!OverwatchEpochRevealEligibility::<Test>::contains_key(2));
        });
    }
}

#[test]
fn multiple_commit_and_reveal_batches_require_completion_even_if_subnet_is_removed() {
    new_test_ext().execute_with(|| {
        let (a, b, length) = setup();
        insert_subnet(2, SubnetState::Active, 0);
        commit(a, 1, PERCENTAGE_FACTOR_U128);
        commit_subnet(a, 1, 2, 0);
        commit(b, 2, 0);
        tick(2 * length + length / 2);
        reveal(a, 1, PERCENTAGE_FACTOR_U128);
        reveal(b, 2, 0);
        // Removing the subnet does not cancel its accepted commitment.
        SubnetsData::<Test>::remove(2);
        System::set_block_number(3 * length - 1);
        reveal_subnet(a, 1, 2, 0);
        tick(3 * length);
        assert_eq!(
            OverwatchEpochRevealEligibility::<Test>::get(2)
                .unwrap()
                .len(),
            2
        );
        tick(3 * length + 1);
        assert_eq!(
            OverwatchSubnetWeights::<Test>::get(2, 1),
            Some(PERCENTAGE_FACTOR_U128 / 2)
        );
        assert_eq!(OverwatchSubnetWeights::<Test>::get(2, 2), Some(0));
        assert!(
            OverwatchNodeStakeBalance::<Test>::get(a) > OverwatchNodeStakeBalance::<Test>::get(b)
        );
        assert!(OverwatchNodeStakeBalance::<Test>::get(b) > 1_000);
    });
}

#[test]
fn all_incomplete_finalizes_valid_empty_and_next_epoch_can_earn_normally() {
    new_test_ext().execute_with(|| {
        let (a, b, length) = setup();
        insert_subnet(2, SubnetState::Active, 0);
        commit(a, 1, PERCENTAGE_FACTOR_U128);
        commit_subnet(a, 1, 2, 0);
        commit(b, 2, 0); // No reveals at all from B.
        tick(2 * length + length / 2);
        reveal(a, 1, PERCENTAGE_FACTOR_U128);
        tick(3 * length);
        assert!(OverwatchEpochRevealEligibility::<Test>::get(2)
            .unwrap()
            .is_empty());
        tick(3 * length + 1);
        let signal = LatestEffectiveOverwatchSignal::<Test>::get().unwrap();
        assert!(signal.valid);
        assert_eq!(signal.source_epoch, 2);
        assert!(signal.subnet_weights.is_empty());
        assert!(LatestFinalizedOverwatchSignalInputs::<Test>::get()
            .unwrap()
            .nodes
            .is_empty());
        assert!(OverwatchSubnetWeights::<Test>::iter_prefix(2)
            .next()
            .is_none());
        assert!(OverwatchNodeWeights::<Test>::iter_prefix(2)
            .next()
            .is_none());
        assert_eq!(OverwatchNodeStakeBalance::<Test>::get(a), 1_000);
        assert_eq!(OverwatchNodeStakeBalance::<Test>::get(b), 1_000);
        assert!(OverwatchNodes::<Test>::contains_key(a));
        assert!(OverwatchReveals::<Test>::iter_prefix(2).next().is_none());
        assert!(!PendingOverwatchSettlement::<Test>::exists());
        commit(a, 1, PERCENTAGE_FACTOR_U128);
        tick(3 * length + length / 2);
        reveal(a, 1, PERCENTAGE_FACTOR_U128);
        tick(4 * length);
        tick(4 * length + 1);
        assert!(OverwatchNodeStakeBalance::<Test>::get(a) > 1_000);
    });
}

#[test]
fn incomplete_exit_and_replacement_cannot_erase_commitment_obligations() {
    new_test_ext().execute_with(|| {
        let (a, b, length) = setup();
        insert_subnet(2, SubnetState::Active, 0);
        commit(a, 1, PERCENTAGE_FACTOR_U128);
        commit_subnet(a, 1, 2, 0);
        commit(b, 2, 0);
        tick(2 * length + length / 2);
        reveal(a, 1, PERCENTAGE_FACTOR_U128);
        reveal(b, 2, 0);
        assert_ok!(Network::remove_overwatch_node(
            RuntimeOrigin::signed(account(101)),
            a
        ));
        assert_eq!(OverwatchCommits::<Test>::get(2, a).len(), 2);
        OverwatchValidatorWhitelist::<Test>::insert(1, ());
        assert_ok!(Network::register_overwatch_node(
            RuntimeOrigin::signed(account(101)),
            1_000
        ));
        let replacement = ValidatorOverwatchNodeId::<Test>::get(1).unwrap();
        assert_ne!(replacement, a);
        tick(3 * length);
        let eligible = OverwatchEpochRevealEligibility::<Test>::get(2).unwrap();
        assert!(!eligible.contains(&a));
        assert!(!eligible.contains(&replacement));
        tick(3 * length + 1);
        assert_eq!(OverwatchNodeStakeBalance::<Test>::get(a), 1_000);
        assert_eq!(OverwatchNodeStakeBalance::<Test>::get(replacement), 1_000);
        assert_eq!(OverwatchSubnetWeights::<Test>::get(2, 1), Some(0));
    });
}

#[test]
fn missing_completion_snapshot_blocks_settlement_and_retries_exactly_once() {
    new_test_ext().execute_with(|| {
        let (a, _, length) = setup();
        commit(a, 1, PERCENTAGE_FACTOR_U128);
        tick(2 * length + length / 2);
        reveal(a, 1, PERCENTAGE_FACTOR_U128);
        tick(3 * length);
        let eligible = OverwatchEpochRevealEligibility::<Test>::take(2).unwrap();
        let pending = PendingOverwatchSettlement::<Test>::get();
        let prior = LatestEffectiveOverwatchSignal::<Test>::get();
        tick(3 * length + 1);
        assert_eq!(OverwatchNodeStakeBalance::<Test>::get(a), 1_000);
        assert_eq!(PendingOverwatchSettlement::<Test>::get(), pending);
        assert_eq!(LatestEffectiveOverwatchSignal::<Test>::get(), prior);
        assert!(OverwatchSubnetWeights::<Test>::iter_prefix(2)
            .next()
            .is_none());
        assert_eq!(OverwatchReveals::<Test>::get(2, a).len(), 1);
        OverwatchEpochRevealEligibility::<Test>::insert(2, eligible);
        Network::calculate_overwatch_rewards();
        let balance = OverwatchNodeStakeBalance::<Test>::get(a);
        assert!(balance > 1_000);
        assert!(!OverwatchEpochRevealEligibility::<Test>::contains_key(2));
        Network::calculate_overwatch_rewards();
        assert_eq!(OverwatchNodeStakeBalance::<Test>::get(a), balance);
    });
}

#[test]
fn governance_purges_pending_completion_without_recreating_missing_evidence() {
    for missing in [false, true] {
        new_test_ext().execute_with(|| {
            let (a, b, length) = setup();
            commit(a, 1, PERCENTAGE_FACTOR_U128);
            commit(b, 2, 0);
            tick(2 * length + length / 2);
            reveal(a, 1, PERCENTAGE_FACTOR_U128);
            reveal(b, 2, 0);
            tick(3 * length);
            if missing {
                OverwatchEpochRevealEligibility::<Test>::remove(2);
            }
            assert_ok!(Network::do_collective_remove_overwatch_node(a));
            if missing {
                assert!(!OverwatchEpochRevealEligibility::<Test>::contains_key(2));
                Network::calculate_overwatch_rewards();
                assert!(PendingOverwatchSettlement::<Test>::exists());
            } else {
                let eligible = OverwatchEpochRevealEligibility::<Test>::get(2).unwrap();
                assert!(!eligible.contains(&a));
                assert!(eligible.contains(&b));
                tick(3 * length + 1);
                assert_eq!(OverwatchSubnetWeights::<Test>::get(2, 1), Some(0));
                assert!(OverwatchNodeStakeBalance::<Test>::get(b) > 1_000);
            }
            assert_eq!(OverwatchNodeStakeBalance::<Test>::get(a), 1_000);
        });
    }
}

#[test]
fn invalid_reveal_batch_does_not_complete_and_late_reveal_cannot_change_closed_eligibility() {
    new_test_ext().execute_with(|| {
        let (a, _, length) = setup();
        insert_subnet(2, SubnetState::Active, 0);
        commit(a, 1, PERCENTAGE_FACTOR_U128);
        commit_subnet(a, 1, 2, 0);
        tick(2 * length + length / 2);
        assert_noop!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(201)),
                a,
                vec![
                    OverwatchReveal {
                        subnet_id: 1,
                        weight: PERCENTAGE_FACTOR_U128,
                        salt: b"opening-test".to_vec().try_into().unwrap()
                    },
                    OverwatchReveal {
                        subnet_id: 2,
                        weight: 1,
                        salt: b"opening-test".to_vec().try_into().unwrap()
                    },
                ]
            ),
            Error::<Test>::RevealMismatch
        );
        assert!(OverwatchReveals::<Test>::get(2, a).is_empty());
        reveal(a, 1, PERCENTAGE_FACTOR_U128);
        tick(3 * length);
        assert_noop!(
            Network::reveal_overwatch_subnet_weights(
                RuntimeOrigin::signed(account(201)),
                a,
                vec![OverwatchReveal {
                    subnet_id: 2,
                    weight: 0,
                    salt: b"opening-test".to_vec().try_into().unwrap()
                }]
            ),
            Error::<Test>::NotRevealPeriod
        );
        assert!(OverwatchEpochRevealEligibility::<Test>::get(2)
            .unwrap()
            .is_empty());
        tick(3 * length + 1);
        assert_eq!(OverwatchNodeStakeBalance::<Test>::get(a), 1_000);
    });
}

#[test]
fn completion_checks_subnet_ids_and_opening_membership_not_just_counts() {
    new_test_ext().execute_with(|| {
        let (a, b, length) = setup();
        commit(a, 1, PERCENTAGE_FACTOR_U128);
        tick(2 * length + length / 2);
        reveal(a, 1, PERCENTAGE_FACTOR_U128);
        // Corrupt rows with equal cardinalities must not pass completion checks.
        OverwatchReveals::<Test>::mutate(2, a, |row| {
            row.remove(&1);
            row.try_insert(2, PERCENTAGE_FACTOR_U128).unwrap();
        });
        // A synthetic reveal without a commitment also cannot qualify.
        OverwatchReveals::<Test>::insert(2, b, OverwatchReveals::<Test>::get(2, a));
        let outsider = register(3, 1_000);
        OverwatchCommits::<Test>::insert(2, outsider, OverwatchCommits::<Test>::get(2, a));
        let mut row = OverwatchReveals::<Test>::get(2, a);
        row.remove(&2);
        row.try_insert(1, PERCENTAGE_FACTOR_U128).unwrap();
        OverwatchReveals::<Test>::insert(2, outsider, row);
        tick(3 * length);
        assert!(OverwatchEpochRevealEligibility::<Test>::get(2)
            .unwrap()
            .is_empty());
        tick(3 * length + 1);
        assert!(LatestEffectiveOverwatchSignal::<Test>::get()
            .unwrap()
            .subnet_weights
            .is_empty());
    });
}

#[test]
fn pause_defers_completion_check_and_preserves_remaining_reveal_time() {
    new_test_ext().execute_with(|| {
        let (a, _, length) = setup();
        insert_subnet(2, SubnetState::Active, 0);
        commit(a, 1, PERCENTAGE_FACTOR_U128);
        commit_subnet(a, 1, 2, 0);
        tick(2 * length + length / 2);
        reveal(a, 1, PERCENTAGE_FACTOR_U128);
        assert_ok!(Network::do_pause());
        tick(4 * length);
        assert_eq!(CurrentOverwatchEpoch::<Test>::get(), 2);
        assert!(!OverwatchEpochRevealEligibility::<Test>::contains_key(2));
        assert_eq!(OverwatchCommits::<Test>::get(2, a).len(), 2);
        assert_ok!(Network::do_unpause());
        tick(4 * length + 1);
        reveal_subnet(a, 1, 2, 0);
        tick(5 * length);
        assert!(OverwatchEpochRevealEligibility::<Test>::get(2)
            .unwrap()
            .contains(&a));
        tick(5 * length + 1);
        assert!(OverwatchNodeStakeBalance::<Test>::get(a) > 1_000);
    });
}
