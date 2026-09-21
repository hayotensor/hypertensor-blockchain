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

use super::*;
use core::num::NonZeroU32;

impl<T: Config> Pallet<T> {
    /// Sample an index in `[0, upper_bound)` using the configured BABE provider.
    ///
    /// Domains must identify the application round. Do not include a draw-time
    /// block hash, timestamp, or nonce that can be chosen after the seed is known.
    /// The provider's delayed seed is stable within a BABE epoch. Candidate-pool
    /// commitment timing is a separate requirement; this helper does not enforce it.
    ///
    /// Every nonempty pool gets an index, including during BABE bootstrap. Reduce
    /// the entire hash instead of retrying a truncated sample: for a uniform
    /// 256-bit hash and a u32 bound, statistical distance from uniform is below
    /// 2^-224. There is no retry limit that can suppress an otherwise valid draw.
    /// Availability does not imply that the seed contains fresh VRF entropy.
    pub(crate) fn get_bounded_random_index<Domain: Encode>(
        domain: Domain,
        upper_bound: u32,
    ) -> Option<u32> {
        let upper_bound = NonZeroU32::new(upper_bound)?;
        if upper_bound.get() == 1 {
            return Some(0);
        }

        let modulus = u64::from(upper_bound.get());
        let subject = (T::PalletId::get(), domain, modulus);
        let (random_seed, _) = T::Randomness::random(&subject.encode());
        Some(index_from_hash(random_seed.as_ref(), upper_bound))
    }
}

/// Interpret all hash bytes as a big-endian integer and reduce modulo the bound.
/// No allocation, fixed-width hash assumption, division by zero, or retries.
pub(crate) fn index_from_hash(hash: &[u8], upper_bound: NonZeroU32) -> u32 {
    let modulus = u64::from(upper_bound.get());
    hash.iter().fold(0u64, |remainder, byte| {
        // remainder < modulus <= u32::MAX, so this intermediate fits in 40 bits.
        (remainder * 256 + u64::from(*byte)) % modulus
    }) as u32
}
