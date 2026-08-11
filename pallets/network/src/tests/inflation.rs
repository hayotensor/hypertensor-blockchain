use super::mock::*;
use crate::inflation::Inflation;
use crate::{MaxSubnetNodes, MaxSubnets, TotalActiveNodes};

const TOKEN: u128 = 1_000_000_000_000_000_000;

#[test]
fn inflation_follows_the_annual_decay_schedule_and_terminal_floor() {
    let inflation = Inflation::default();

    assert_eq!(inflation.initial_annual_emissions, 100_000 * TOKEN);
    assert_eq!(inflation.terminal_annual_emissions, 75_000 * TOKEN);
    assert_eq!(inflation.inflation(0), 100_000 * TOKEN);
    assert_eq!(inflation.inflation(1), 90_000 * TOKEN);
    assert_eq!(inflation.inflation(2), 81_000 * TOKEN);
    assert_eq!(inflation.inflation(3), 75_000 * TOKEN);
    assert_eq!(inflation.inflation(4), 75_000 * TOKEN);
    assert_eq!(inflation.inflation(u32::MAX), 75_000 * TOKEN);
}

#[test]
fn integer_decay_does_not_overflow_at_u128_max() {
    let inflation = Inflation {
        initial_annual_emissions: u128::MAX,
        terminal_annual_emissions: 0,
    };
    let expected = (u128::MAX / 100) * 90 + ((u128::MAX % 100) * 90) / 100;

    assert_eq!(inflation.inflation(1), expected);
}

#[test]
fn get_inflation_changes_only_at_year_boundaries() {
    new_test_ext().execute_with(|| {
        let epochs_per_year = EPOCHS_PER_YEAR;

        assert!(epochs_per_year > 0);
        assert_eq!(epochs_per_year, YEAR / EPOCH_LENGTH);
        assert_eq!(Network::get_inflation(0), 100_000 * TOKEN);
        assert_eq!(Network::get_inflation(epochs_per_year - 1), 100_000 * TOKEN);
        assert_eq!(Network::get_inflation(epochs_per_year), 90_000 * TOKEN);
        assert_eq!(
            Network::get_inflation(2 * epochs_per_year - 1),
            90_000 * TOKEN
        );
        assert_eq!(Network::get_inflation(2 * epochs_per_year), 81_000 * TOKEN);
        assert_eq!(Network::get_inflation(3 * epochs_per_year), 75_000 * TOKEN);
    });
}

#[test]
fn epoch_emissions_preserve_the_annual_95_5_split() {
    new_test_ext().execute_with(|| {
        let epochs_per_year = EPOCHS_PER_YEAR as u128;

        for epoch in [
            0,
            EPOCHS_PER_YEAR - 1,
            EPOCHS_PER_YEAR,
            2 * EPOCHS_PER_YEAR,
            3 * EPOCHS_PER_YEAR,
            u32::MAX,
        ] {
            let annual_emissions = Network::get_inflation(epoch);
            let annual_foundation_emissions = annual_emissions * 5 / 100;
            let annual_subnet_emissions = annual_emissions - annual_foundation_emissions;
            let (subnet_emissions, foundation_emissions) = Network::get_epoch_emissions(epoch);

            assert_eq!(annual_subnet_emissions, annual_emissions * 95 / 100);
            assert_eq!(subnet_emissions, annual_subnet_emissions / epochs_per_year);
            assert_eq!(
                foundation_emissions,
                annual_foundation_emissions / epochs_per_year
            );

            let combined_epoch_emissions = subnet_emissions + foundation_emissions;
            let epoch_budget = annual_emissions / epochs_per_year;
            assert!(combined_epoch_emissions <= epoch_budget);
            assert!(epoch_budget - combined_epoch_emissions <= 1);
        }
    });
}

#[test]
fn epoch_emissions_are_independent_of_subnet_node_utilization() {
    new_test_ext().execute_with(|| {
        let epoch = 2 * EPOCHS_PER_YEAR;
        let expected = Network::get_epoch_emissions(epoch);

        MaxSubnets::<Test>::put(0);
        MaxSubnetNodes::<Test>::put(0);
        TotalActiveNodes::<Test>::put(0);
        assert_eq!(Network::get_epoch_emissions(epoch), expected);

        MaxSubnets::<Test>::put(u32::MAX);
        MaxSubnetNodes::<Test>::put(u32::MAX);
        TotalActiveNodes::<Test>::put(u32::MAX);
        assert_eq!(Network::get_epoch_emissions(epoch), expected);
    });
}
