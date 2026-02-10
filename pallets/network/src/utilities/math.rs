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

use super::*;
use libm::{exp, pow, round};
use sp_core::U256;

impl<T: Config> Pallet<T> {
    // 1e18 as 1.0
    pub const PERCENTAGE_FACTOR: U256 = U256([0xde0b6b3a7640000, 0x0, 0x0, 0x0]);
    // 0.5e18 as 0.5
    pub const HALF_PERCENT: U256 = U256([0x06f05b59d3b20000, 0x0, 0x0, 0x0]);

    /// `x` is value
    /// `y` is percentage
    /// Rounds down to the nearest 10th decimal
    pub fn percent_mul(x: u128, y: u128) -> u128 {
        if x == 0 || y == 0 {
            return 0;
        }

        let x = U256::from(x);
        let y = U256::from(y);

        if x > ((U256::MAX - Self::HALF_PERCENT) / y) {
            return 0;
        }

        // x * y / 100.0
        let result = x * y / Self::PERCENTAGE_FACTOR;

        if result > U256::from(u128::MAX) {
            // return 0
            return u128::MAX;
        }

        result.try_into().unwrap_or(u128::MAX)
    }

    /// `x` is value
    /// `y` is percentage
    /// Rounds down to the nearest 10th decimal
    pub fn percent_div(x: u128, y: u128) -> u128 {
        if x == 0 || y == 0 {
            return 0;
        }

        let x = U256::from(x);
        let y = U256::from(y);

        // x * 100.0 / y
        let result = x * Self::PERCENTAGE_FACTOR / y;

        result.try_into().unwrap_or(u128::MAX)
    }

    pub fn percentage_factor_as_u128() -> u128 {
        1_000_000_000_000_000_000
    }

    /// Get percentage as f64 (full 1e18 with decimals)
    pub fn percentage_factor_as_f64() -> f64 {
        1_000_000_000_000_000_000.0
    }

    /// Get percentage in decimal format that uses `PERCENTAGE_FACTOR` as f64
    pub fn get_percent_as_f64(v: u128) -> f64 {
        v as f64 / Self::percentage_factor_as_u128() as f64
    }

    /// Get decimal f64 1.0 converted to u128 1e18
    pub fn get_f64_as_percentage(v: f64) -> u128 {
        (v * 1_000_000_000_000_000_000.0) as u128
    }

    pub fn pow(x: f64, exp: f64) -> f64 {
        pow(x, exp)
    }

    pub fn checked_mul_div(x: U256, y: U256, z: U256) -> Option<U256> {
        if z.is_zero() {
            return None;
        }
        x.checked_mul(y)?.checked_div(z)
    }

    /// Computes a symmetric, decreasing sigmoid curve scaled to a specified output range.
    ///
    /// # Parameters
    /// - `x`: The input value to evaluate the sigmoid at. Should be in the range `[0.0, 1.0]`.
    /// - `mid`: The midpoint of the sigmoid. The curve is symmetric around this value.
    /// - `k`: Controls the steepness of the sigmoid. Larger values make the transition sharper.
    /// - `min`: Minimum value of the output range. The sigmoid will not go below this value.
    /// - `max`: Maximum value of the output range. The sigmoid will not exceed this value.
    ///
    /// # Returns
    /// - A `f64` representing the value of the scaled sigmoid at `x`. Guaranteed to be within `[min, max]`.
    pub fn sigmoid_decreasing(x: f64, mid: f64, k: f64, min: f64, max: f64) -> f64 {
        let c = (x - mid).abs();
        let d = k * c;
        let exp = exp(d);

        // symmetric sigmoid around mid
        let sigmoid = if x > mid {
            1.0 / (1.0 + exp)
        } else {
            exp / (1.0 + exp)
        };

        sigmoid.clamp(min, max)
    }

    pub fn sigmoid_decreasing_v2(x: f64, mid: f64, k: f64) -> f64 {
        let c = (x - mid).abs();
        let d = k * c;
        let exp = exp(d);

        // symmetric sigmoid around mid
        let sigmoid = if x > mid {
            1.0 / (1.0 + exp)
        } else {
            exp / (1.0 + exp)
        };

        sigmoid.clamp(0.0, 1.0)
    }

    /// Offset and scale the sigmoid curve
    ///
    /// # Parameters
    /// - `x`: The input value to evaluate the sigmoid at. Should be in the range `[0.0, 1.0]`.
    /// - `mid`: The midpoint of the sigmoid. The curve is symmetric around this value.
    /// - `k`: Controls the steepness of the sigmoid. Larger values make the transition sharper.
    /// - `min`: Minimum value of the output range. The sigmoid will not go below this value.
    /// - `max`: Maximum value of the output range. The sigmoid will not exceed this value.
    ///
    /// # Returns
    /// - A `f64` representing the value of the scaled sigmoid at `x`. Guaranteed to be within `[min, max]`.
    pub fn sigmoid_decreasing_v3(x: f64, mid: f64, k: f64, min: f64, max: f64) -> f64 {
        let c = (x - mid).abs();
        let d = k * c;
        let exp = exp(d);

        // Symmetric sigmoid around mid (produces value in [0, 1])
        // When x < mid: sigmoid approaches 1 (high)
        // When x > mid: sigmoid approaches 0 (low)
        let sigmoid = if x > mid {
            1.0 / (1.0 + exp)
        } else {
            exp / (1.0 + exp)
        };

        // Offset and scale the sigmoid curve
        // Original sigmoid is in [0, 1]
        // Scale by (max - min) and shift by min
        // Result: sigmoid curve operates in range [min, max]
        // Example: min=0, max=2 → curve goes from 0 to 2 (2x scale)
        // Example: min=0.5, max=1.5 → curve goes from 0.5 to 1.5 (1x scale, 0.5 offset)
        min + sigmoid * (max - min)
    }

    pub fn sigmoid_decreasing_asymmetric(
        x: f64,
        mid: f64,
        k_front: f64,
        k_back: f64,
        min: f64,
        max: f64,
    ) -> f64 {
        let c = (x - mid).abs();
        let d = if x > mid { k_back * c } else { k_front * c };

        let exp = exp(d);

        // symmetric sigmoid around mid
        let sigmoid = if x > mid {
            1.0 / (1.0 + exp)
        } else {
            exp / (1.0 + exp)
        };

        // scale sigmoid from [0, 1] → [min, max]
        let scaled = min + (max - min) * sigmoid;

        scaled.clamp(min, max)
    }

    /// Symmetric sigmoid decreasing function with x-axis offset support.
    ///
    /// This is specifically meant for the rewards factor for validator/attestor
    ///
    /// This function normalizes the input `x` from the range `[x_start, 1.0]` to `[0.0, 1.0]`
    /// and applies a symmetric sigmoid curve. This allows the sigmoid to "start" at `x_start`
    /// instead of 0.0 while maintaining the full sigmoid behavior across the compressed range.
    ///
    /// # Parameters
    /// - `x`: The input value, expected to be in range `[x_start, 1.0]`
    /// - `mid`: The midpoint of the sigmoid in normalized space `[0.0, 1.0]`
    /// - `k`: Controls the steepness of the sigmoid. Larger values make the transition sharper
    /// - `x_start`: The starting point of the sigmoid on the x-axis. Values below this are normalized to 0
    /// - `round_decimal_places`: The number of decimal places to round the output to
    ///
    /// # Returns
    /// A value in `[0.0, 1.0]` representing the sigmoid output
    ///
    /// # Example
    /// With `x_start = 0.05`:
    /// - `x = 0.05` → normalized to 0.0 → output ≈ 1.0 (start of curve)
    /// - `x = 0.06` → normalized to ~0.0105 → output ≈ 0.99999
    /// - `x = 1.0` → normalized to 1.0 → output ≈ 0.0 (end of curve)
    pub fn sigmoid_decreasing_start_offset(
        x: f64,
        mid: f64,
        k: f64,
        x_start: f64,
        round_decimal_places: f64,
    ) -> f64 {
        // Normalize x from [x_start, 1.0] to [0.0, 1.0]
        let range = 1.0 - x_start;
        let normalized_x = (x - x_start) / range;

        let c = (normalized_x - mid).abs();
        let d = k * c;
        let exp = exp(d);

        // symmetric sigmoid around mid
        let sigmoid = if normalized_x > mid {
            1.0 / (1.0 + exp)
        } else {
            exp / (1.0 + exp)
        };

        Self::round_f64(sigmoid.clamp(0.0, 1.0), round_decimal_places)
    }

    /// Rounds a floating-point number to a specified number of decimal places.
    ///
    /// # Parameters
    /// - `value`: The floating-point number to round.
    /// - `decimal_places`: The number of decimal places to round to.
    ///
    /// # Returns
    /// The rounded floating-point number.
    fn round_f64(value: f64, decimal_places: f64) -> f64 {
        let factor = pow(10.0f64, decimal_places);
        round(value * factor) / factor
    }

    /// Computes a concave-down decreasing curve scaled to a specified output range.
    ///
    /// # Parameters
    /// - `x`: Input value in the range `[0.0, 1.0]`. Represents the normalized progress along the curve.
    /// - `min`: Minimum value of the output range. Returned when `x = 1.0`.
    /// - `max`: Maximum value of the output range. Returned when `x = 0.0`.
    /// - `power`: Controls the steepness of the curve. Values > 1.0 make the curve flatter at the start
    ///            and steeper at the end. Must be positive.
    ///
    /// # Returns
    /// - `y` in the range `[min, max]` corresponding to the concave-down decreasing curve.
    pub fn concave_down_decreasing(x: f64, min: f64, max: f64, power: f64) -> f64 {
        // Ensure power is positive to avoid undefined behavior
        let p = if power <= 0.0 { 1.0 } else { power };

        // Compute concave-down decreasing curve
        let curve = 1.0 - pow(x, p);

        // Scale to [min, max]
        (min + (max - min) * curve).clamp(min, max)
    }
}
