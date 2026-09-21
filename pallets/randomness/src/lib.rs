// Copyright (C) Hypertensor.
// SPDX-License-Identifier: Apache-2.0

//! A stateless FRAME pallet exposing BABE's delayed epoch randomness.
//!
//! BABE collects primary-block VRF contributions; this pallet derives application
//! outputs from [`pallet_babe::RandomnessFromTwoEpochsAgo`]. It does not generate
//! new entropy or verify VRF proofs again. See the crate README for commitment
//! timing, finality assumptions, and the consumer's responsibilities.
//!
//! Prefer [`Pallet::random_after`] for application decisions. The standard FRAME
//! [`Randomness`] implementation also exposes BABE's bootstrap output at genesis
//! and must not be interpreted as fresh entropy merely because it is nonzero.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Encode;
use frame_support::traits::Randomness;
use frame_system::pallet_prelude::BlockNumberFor;

pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

/// Separates application outputs from other direct users of BABE randomness.
/// Changing this tag changes every derived output.
const DOMAIN: &[u8] = b"hypertensor/babe-randomness/v1";

#[frame_support::pallet]
pub mod pallet {
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Uses the containing runtime's BABE instance. No independent keys,
    /// authorities, genesis seed, or privileged configuration are needed.
    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_babe::Config {}
}

impl<T: Config> Pallet<T> {
    /// Return a domain-separated seed and BABE's commitment cutoff block.
    ///
    /// This is stable within a BABE epoch for a fixed subject. It deliberately
    /// excludes the current block hash, timestamp, and transaction data.
    /// It returns an output even at genesis, after skipped epochs, and without
    /// new primary VRFs. Such output can be predictable; returning a hash is not
    /// a certificate of fresh entropy. Zero and repeated values are valid outputs.
    ///
    /// This low-level API preserves the standard FRAME randomness contract,
    /// including bootstrap values. Prefer [`Self::random_after`] and follow the
    /// upstream three-epoch commitment rule documented in the README.
    ///
    /// Cost: two BABE storage reads and encoding/hashing linear in subject size.
    /// Consumers must bound the subject and benchmark the operation in their own
    /// dispatchable or hook; this pallet has no dispatchables or hooks to charge.
    pub fn random(subject: &[u8]) -> (T::Hash, BlockNumberFor<T>) {
        pallet_babe::RandomnessFromTwoEpochsAgo::<T>::random(&(DOMAIN, subject).encode())
    }

    /// Return randomness only if BABE's cutoff is strictly after `committed_at`.
    ///
    /// `committed_at` must be the actual last block at which the consumer fixed
    /// *all* inputs (candidate set, ordering, subject, and intended draw epoch).
    /// Equality is rejected because transaction ordering within that block is
    /// not enough to establish that the inputs preceded the randomness.
    /// This also rejects the zero cutoff during BABE bootstrap.
    ///
    /// This checks BABE's block-level freshness metadata, not proof of a stored
    /// commitment, epoch continuity, VRF participation, or GRANDPA finality.
    /// In the pinned SDK, primary VRFs can remain buffered across epoch changes;
    /// a passing cutoff check does not prove they were mixed into the seed.
    /// Consumers must additionally enforce BABE's three-epoch commitment rule
    /// and a predetermined draw epoch. See the README before integration.
    pub fn random_after(
        subject: &[u8],
        committed_at: BlockNumberFor<T>,
    ) -> Option<(T::Hash, BlockNumberFor<T>)> {
        let result = Self::random(subject);
        (committed_at < result.1).then_some(result)
    }
}

impl<T: Config> Randomness<T::Hash, BlockNumberFor<T>> for Pallet<T> {
    fn random(subject: &[u8]) -> (T::Hash, BlockNumberFor<T>) {
        Self::random(subject)
    }
}
