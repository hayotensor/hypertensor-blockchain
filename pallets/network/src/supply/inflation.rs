// Copyright (C) Hypertensor.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Defines the deterministic annual emissions schedule and foundation/subnet split.

use super::*;

pub struct Inflation {
    /// Annual emissions at network launch, in atomic token units.
    pub initial_annual_emissions: u128,
    /// Minimum annual emissions after decay, in atomic token units.
    pub terminal_annual_emissions: u128,
}

const TOKEN: u128 = 1_000_000_000_000_000_000;
const DEFAULT_INITIAL_ANNUAL_EMISSIONS: u128 = 100_000 * TOKEN;
const DEFAULT_TERMINAL_ANNUAL_EMISSIONS: u128 = 75_000 * TOKEN;

/// Retain 90% of the previous year's emissions (a 10% annual decay).
const ANNUAL_RETENTION_NUMERATOR: u128 = 90;
const ANNUAL_RETENTION_DENOMINATOR: u128 = 100;

/// Reserve 5% of the annual emissions budget for the foundation.
const FOUNDATION_NUMERATOR: u128 = 5;
const EMISSIONS_SPLIT_DENOMINATOR: u128 = 100;

impl Default for Inflation {
    fn default() -> Self {
        Self {
            initial_annual_emissions: DEFAULT_INITIAL_ANNUAL_EMISSIONS,
            terminal_annual_emissions: DEFAULT_TERMINAL_ANNUAL_EMISSIONS,
        }
    }
}

impl Inflation {
    /// Multiply `value` by a proper fraction without overflowing `u128`.
    fn mul_ratio(value: u128, numerator: u128, denominator: u128) -> u128 {
        debug_assert!(denominator > 0);
        debug_assert!(numerator <= denominator);

        let whole = value / denominator;
        let remainder = value % denominator;

        whole
            .saturating_mul(numerator)
            .saturating_add(remainder.saturating_mul(numerator) / denominator)
    }

    /// Return the annual emissions budget after `elapsed_years` of geometric decay.
    pub fn inflation(&self, elapsed_years: u32) -> u128 {
        let terminal = self.terminal_annual_emissions;
        let mut emissions = self.initial_annual_emissions.max(terminal);
        let mut remaining_years = elapsed_years;

        // The loop terminates as soon as the terminal floor is reached. With the default
        // parameters this requires at most three iterations, regardless of chain age.
        while remaining_years > 0 && emissions > terminal {
            emissions = Self::mul_ratio(
                emissions,
                ANNUAL_RETENTION_NUMERATOR,
                ANNUAL_RETENTION_DENOMINATOR,
            )
            .max(terminal);
            remaining_years = remaining_years.saturating_sub(1);
        }

        emissions
    }
}

impl<T: Config> Pallet<T> {
    /// Return the annual emissions budget applicable to `epoch`.
    pub fn get_inflation(epoch: u32) -> u128 {
        let epochs_per_year = T::EpochsPerYear::get();
        if epochs_per_year == 0 {
            return 0;
        }

        let elapsed_years = epoch / epochs_per_year;
        Inflation::default().inflation(elapsed_years)
    }

    /// Return `(subnet_emissions, foundation_emissions)` for `epoch`.
    pub fn get_epoch_emissions(epoch: u32) -> (u128, u128) {
        let epochs_per_year = T::EpochsPerYear::get() as u128;
        if epochs_per_year == 0 {
            return (0, 0);
        }

        let annual_emissions = Self::get_inflation(epoch);
        let annual_foundation_emissions = Inflation::mul_ratio(
            annual_emissions,
            FOUNDATION_NUMERATOR,
            EMISSIONS_SPLIT_DENOMINATOR,
        );
        let annual_subnet_emissions = annual_emissions.saturating_sub(annual_foundation_emissions);

        (
            annual_subnet_emissions / epochs_per_year,
            annual_foundation_emissions / epochs_per_year,
        )
    }
}
