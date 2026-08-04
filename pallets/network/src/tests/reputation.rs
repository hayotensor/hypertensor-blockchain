use super::mock::*;
use crate::tests::test_utils::*;
use crate::{Reputation, ValidatorReputation};

fn validator_reputation(score: u128, election_epoch: u32) -> Reputation {
    Reputation {
        start_epoch: Some(election_epoch),
        score,
        lifetime_node_count: 0,
        total_active_nodes: 0,
        total_increases: 0,
        total_decreases: 0,
        average_proposal_identity_support: 0,
        identity_support_samples: 0,
        last_validator_epoch: Some(election_epoch),
        ow_score: score,
    }
}

#[test]
fn identity_supermajority_increases_validator_reputation_and_records_support() {
    new_test_ext().execute_with(|| {
        let validator_id = 1;
        let election_epoch = 7;
        let identity_supermajority = test_percent(7, 8);
        let identity_support = identity_supermajority;
        let increase_factor = test_percent(1, 2);
        let starting_score = test_percent(1, 2);

        ValidatorReputation::<Test>::insert(
            validator_id,
            validator_reputation(starting_score, election_epoch),
        );

        Network::increase_validator_reputation(
            validator_id,
            identity_support,
            identity_supermajority,
            increase_factor,
        );

        let rep = ValidatorReputation::<Test>::get(validator_id);

        assert_eq!(
            rep.score,
            Network::increase_rep(starting_score, increase_factor, None)
        );
        assert_eq!(rep.total_increases, 1);
        assert_eq!(rep.total_decreases, 0);
        assert_eq!(rep.start_epoch, Some(election_epoch));
        assert_eq!(rep.last_validator_epoch, Some(election_epoch));
        assert_eq!(rep.average_proposal_identity_support, identity_support);
        assert_eq!(rep.identity_support_samples, 1);
    });
}

#[test]
fn below_identity_supermajority_is_neutral_but_records_support() {
    new_test_ext().execute_with(|| {
        let validator_id = 1;
        let identity_supermajority = test_percent(7, 8);
        let identity_support = identity_supermajority - 1;
        let increase_factor = test_percent(1, 2);
        let starting_score = test_percent(1, 2);

        ValidatorReputation::<Test>::insert(validator_id, validator_reputation(starting_score, 5));

        Network::increase_validator_reputation(
            validator_id,
            identity_support,
            identity_supermajority,
            increase_factor,
        );

        let rep = ValidatorReputation::<Test>::get(validator_id);
        assert_eq!(rep.score, starting_score);
        assert_eq!(rep.total_increases, 0);
        assert_eq!(rep.total_decreases, 0);
        assert_eq!(rep.average_proposal_identity_support, identity_support);
        assert_eq!(rep.identity_support_samples, 1);
    });
}

#[test]
fn identity_shortfall_scales_validator_reputation_decrease_and_records_support() {
    new_test_ext().execute_with(|| {
        let validator_id = 1;
        let identity_support = test_percent(1, 6);
        let identity_shortfall = test_percent(1, 2);
        let decrease_factor = test_percent(1, 5);
        let start_score = test_percent(4, 5);

        ValidatorReputation::<Test>::insert(validator_id, validator_reputation(start_score, 6));

        Network::decrease_validator_reputation(
            validator_id,
            identity_support,
            Some(identity_shortfall),
            decrease_factor,
        );

        let rep = ValidatorReputation::<Test>::get(validator_id);
        assert_eq!(
            rep.score,
            Network::decrease_rep(start_score, decrease_factor, Some(identity_shortfall))
        );
        assert_eq!(rep.total_increases, 0);
        assert_eq!(rep.total_decreases, 1);
        assert_eq!(rep.average_proposal_identity_support, identity_support);
        assert_eq!(rep.identity_support_samples, 1);
        assert_eq!(rep.start_epoch, Some(6));
        assert_eq!(rep.last_validator_epoch, Some(6));
    });
}

#[test]
fn neutral_rejected_proposal_records_support_without_decreasing_score() {
    new_test_ext().execute_with(|| {
        let validator_id = 1;
        let identity_support = test_percent(1, 2);
        let decrease_factor = test_percent(1, 5);
        let start_score = test_percent(4, 5);

        ValidatorReputation::<Test>::insert(validator_id, validator_reputation(start_score, 8));

        Network::decrease_validator_reputation(
            validator_id,
            identity_support,
            None,
            decrease_factor,
        );

        let rep = ValidatorReputation::<Test>::get(validator_id);
        assert_eq!(rep.score, start_score);
        assert_eq!(rep.total_increases, 0);
        assert_eq!(rep.total_decreases, 0);
        assert_eq!(rep.average_proposal_identity_support, identity_support);
        assert_eq!(rep.identity_support_samples, 1);
    });
}

#[test]
fn identity_support_average_includes_increase_decrease_and_neutral_samples() {
    new_test_ext().execute_with(|| {
        let validator_id = 1;
        let starting_score = test_percent(1, 2);
        let factor = test_percent(1, 10);
        ValidatorReputation::<Test>::insert(validator_id, validator_reputation(starting_score, 8));

        Network::increase_validator_reputation(
            validator_id,
            test_percent(9, 10),
            test_percent(7, 8),
            factor,
        );
        Network::decrease_validator_reputation(
            validator_id,
            test_percent(1, 10),
            Some(test_percent(7, 10)),
            factor,
        );
        Network::decrease_validator_reputation(validator_id, test_percent(1, 2), None, factor);

        let rep = ValidatorReputation::<Test>::get(validator_id);
        assert_eq!(rep.total_increases, 1);
        assert_eq!(rep.total_decreases, 1);
        assert_eq!(rep.average_proposal_identity_support, test_percent(1, 2));
        assert_eq!(rep.identity_support_samples, 3);
    });
}

#[test]
fn missing_proposal_records_zero_identity_support_without_changing_score() {
    new_test_ext().execute_with(|| {
        let validator_id = 1;
        let start_score = test_percent(4, 5);
        let previous_average = test_percent(3, 5);
        let mut reputation = validator_reputation(start_score, 8);
        reputation.total_increases = 4;
        reputation.total_decreases = 2;
        reputation.average_proposal_identity_support = previous_average;
        reputation.identity_support_samples = 2;
        ValidatorReputation::<Test>::insert(validator_id, reputation);

        Network::record_validator_identity_support(validator_id, 0);

        let rep = ValidatorReputation::<Test>::get(validator_id);
        assert_eq!(rep.score, start_score);
        assert_eq!(rep.total_increases, 4);
        assert_eq!(rep.total_decreases, 2);
        assert_eq!(
            rep.average_proposal_identity_support,
            previous_average.saturating_mul(2).saturating_div(3)
        );
        assert_eq!(rep.identity_support_samples, 3);
    });
}

#[test]
fn saturated_identity_support_sample_count_freezes_the_average() {
    new_test_ext().execute_with(|| {
        let validator_id = 1;
        let starting_average = test_percent(3, 5);
        let mut reputation = validator_reputation(test_percent(4, 5), 8);
        reputation.average_proposal_identity_support = starting_average;
        reputation.identity_support_samples = u32::MAX;
        ValidatorReputation::<Test>::insert(validator_id, reputation);

        Network::record_validator_identity_support(validator_id, 0);

        let rep = ValidatorReputation::<Test>::get(validator_id);
        assert_eq!(rep.average_proposal_identity_support, starting_average);
        assert_eq!(rep.identity_support_samples, u32::MAX);
    });
}

// #[test]
// fn test_increase_node_reputation_basic() {
//     new_test_ext().execute_with(|| {
//         let new = Network::get_increase_reputation(test_percent(1, 2), test_percent(1, 10));
//         assert_eq!(new, 550000000000000000);

//         let new = Network::get_increase_reputation(test_percent(9, 10), test_percent(1, 2));
//         assert_eq!(new, test_percent(95, 100));

//         let new = Network::get_increase_reputation(
//             Network::percentage_factor_as_u128(),
//             test_percent(1, 2),
//         );
//         assert_eq!(new, Network::percentage_factor_as_u128());

//         let new = Network::get_increase_reputation(0, Network::percentage_factor_as_u128());
//         assert_eq!(new, Network::percentage_factor_as_u128());
//     });
// }

// #[test]
// fn test_decrease_node_reputation_basic() {
//     new_test_ext().execute_with(|| {
//         let new = Network::get_decrease_reputation(test_percent(1, 2), test_percent(1, 10));
//         assert_eq!(new, 450000000000000000);

//         let new = Network::get_decrease_reputation(test_percent(9, 10), test_percent(1, 2));
//         assert_eq!(new, 450000000000000000);

//         let new = Network::get_decrease_reputation(
//             Network::percentage_factor_as_u128(),
//             Network::percentage_factor_as_u128(),
//         );
//         assert_eq!(new, 0);

//         let new = Network::get_decrease_reputation(0, test_percent(4, 5));
//         assert_eq!(new, 0);
//     });
// }

// #[test]
// fn test_reputation_bounds() {
//     new_test_ext().execute_with(|| {
//         let new = Network::get_increase_reputation(
//             Network::percentage_factor_as_u128() - 1,
//             Network::percentage_factor_as_u128(),
//         );
//         assert_eq!(new, Network::percentage_factor_as_u128());

//         let new = Network::get_decrease_reputation(1, Network::percentage_factor_as_u128());
//         assert_eq!(new, 0);
//     });
// }

// #[test]
// fn test_factor_clamping() {
//     new_test_ext().execute_with(|| {
//         let over_factor = Network::percentage_factor_as_u128() * 10;
//         let new_inc = Network::get_increase_reputation(test_percent(1, 2), over_factor);
//         let new_dec = Network::get_decrease_reputation(test_percent(1, 2), over_factor);
//         assert_eq!(new_inc, Network::percentage_factor_as_u128());
//         assert_eq!(new_dec, 0);
//     });
// }

#[test]
fn test_get_increase_reputation_v2() {
    new_test_ext().execute_with(|| {
        let factor = test_percent(1, 20); // 5%
        let mut reputation = test_percent(1, 10); // 10%

        for i in 0..64 {
            reputation = Network::increase_rep(reputation, factor, None);
            log::error!(
                "new {:?}, {:?}",
                i + 1,
                (reputation as f64 / Network::percentage_factor_as_u128() as f64)
            );
        }

        // assert!(false)
    });
}

#[test]
fn test_get_decrease_reputation_v2() {
    new_test_ext().execute_with(|| {
        let factor = test_percent(1, 20); // 5%
        let mut reputation = Network::percentage_factor_as_u128();

        for i in 0..64 {
            reputation = Network::decrease_rep(reputation, factor, None);
            log::error!(
                "new {:?}, {:?}",
                i + 1,
                (reputation as f64 / Network::percentage_factor_as_u128() as f64)
            );
        }

        // assert!(false)
    });
}
