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
    pub annual_retention_percent: u8,
}

impl Inflation {
    pub fn from_config<T: Config>() -> Self {
        let economics = T::Economics::get();
        Self {
            initial_annual_emissions: economics.initial_annual_emissions,
            terminal_annual_emissions: economics.terminal_annual_emissions,
            annual_retention_percent: economics.annual_retention_percent,
        }
    }

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
        assert!(self.annual_retention_percent <= 100);
        let terminal = self.terminal_annual_emissions;
        if self.annual_retention_percent == 100 {
            return self.initial_annual_emissions.max(terminal);
        }
        let mut emissions = self.initial_annual_emissions.max(terminal);
        let mut remaining_years = elapsed_years;

        // The loop terminates as soon as the terminal floor is reached. With the default
        // parameters this requires at most three iterations, regardless of chain age.
        while remaining_years > 0 && emissions > terminal {
            emissions = Self::mul_ratio(emissions, self.annual_retention_percent as u128, 100)
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
        Inflation::from_config::<T>().inflation(elapsed_years)
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
            T::Economics::get().foundation_share_percent as u128,
            100,
        );
        let annual_subnet_emissions = annual_emissions.saturating_sub(annual_foundation_emissions);

        (
            annual_subnet_emissions / epochs_per_year,
            annual_foundation_emissions / epochs_per_year,
        )
    }
}
