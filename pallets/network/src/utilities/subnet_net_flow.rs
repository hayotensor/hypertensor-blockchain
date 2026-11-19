//! Signed external token flows, smoothed once per completed general epoch.
//! Deposits and withdrawals record opposite deltas; rewards and share transfers record none.
//! Amounts are Q18-scaled atomic units, never percentages of pool balances.

use super::*;
use core::cmp::Ordering;
use sp_core::U256;
use sp_runtime::{ArithmeticError, DispatchError};

impl SignedNetFlow {
    pub(crate) fn new(negative: bool, magnitude: U256) -> Self {
        Self {
            negative: negative && !magnitude.is_zero(),
            magnitude,
        }
    }

    pub(crate) fn from_amount(negative: bool, amount: u128) -> Self {
        Self::new(
            negative,
            U256::from(amount) * U256::from(PERCENTAGE_FACTOR_U128),
        )
    }

    pub(crate) fn checked_add(self, other: Self) -> Option<Self> {
        if self.negative == other.negative {
            Some(Self::new(
                self.negative,
                self.magnitude.checked_add(other.magnitude)?,
            ))
        } else if self.magnitude >= other.magnitude {
            Some(Self::new(self.negative, self.magnitude - other.magnitude))
        } else {
            Some(Self::new(other.negative, other.magnitude - self.magnitude))
        }
    }

    /// Convex combination with one truncation toward zero. U512 intermediates preserve
    /// the full balance range, including withdrawals greater than i128::MAX.
    pub(crate) fn blend(self, next: Self, alpha: u128) -> Self {
        let scale = U256::from(PERCENTAGE_FACTOR_U128);
        let left = self
            .magnitude
            .full_mul(U256::from(PERCENTAGE_FACTOR_U128 - alpha));
        let right = next.magnitude.full_mul(U256::from(alpha));
        let (negative, magnitude) = if self.negative == next.negative {
            (self.negative, left + right)
        } else if left >= right {
            (self.negative, left - right)
        } else {
            (next.negative, right - left)
        };
        Self::new(
            negative,
            (magnitude / scale.full_mul(U256::one()))
                .try_into()
                .expect("convex combination fits U256"),
        )
    }
}

impl Ord for SignedNetFlow {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.negative, other.negative) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => self.magnitude.cmp(&other.magnitude),
            (true, true) => other.magnitude.cmp(&self.magnitude),
        }
    }
}

impl PartialOrd for SignedNetFlow {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Config> Pallet<T> {
    /// Finalize pending flow, then decay through skipped zero-flow epochs in O(log gap).
    /// The open epoch never contributes to the completed-epoch EMA.
    pub(crate) fn checkpoint_subnet_net_flow(
        mut previous: SubnetNetFlow,
        epoch: u32,
        alpha: u128,
    ) -> SubnetNetFlow {
        let scale = Self::percentage_factor_as_u128();
        if previous.epoch > epoch || alpha == 0 || alpha > scale {
            return SubnetNetFlow {
                epoch,
                ..Default::default()
            };
        }
        if previous.epoch == epoch {
            return previous;
        }
        previous.ema = previous.ema.blend(previous.flow, alpha);
        let mut gap = epoch - previous.epoch - 1;
        let mut decay = U256::from(scale);
        let mut base = U256::from(scale - alpha);
        while gap != 0 {
            if gap & 1 != 0 {
                decay = decay * base / U256::from(scale);
            }
            gap >>= 1;
            if gap != 0 {
                base = base * base / U256::from(scale);
            }
        }
        previous.ema = previous
            .ema
            .blend(SignedNetFlow::default(), scale - decay.as_u128());
        SubnetNetFlow {
            epoch,
            flow: SignedNetFlow::default(),
            ema: previous.ema,
        }
    }

    pub(crate) fn read_subnet_net_flow(subnet_id: u32, epoch: u32, alpha: u128) -> SubnetNetFlow {
        let previous = SubnetNetFlows::<T>::get(subnet_id).unwrap_or(SubnetNetFlow {
            epoch,
            ..Default::default()
        });
        Self::checkpoint_subnet_net_flow(previous, epoch, alpha)
    }

    /// Prepare before financial writes. Failed operations must not persist the returned delta.
    /// A deleted subnet's residual withdrawals must not recreate emission accounting.
    pub(crate) fn prepare_subnet_net_flow(
        subnet_id: u32,
        amount: u128,
        withdrawal: bool,
    ) -> Result<Option<SubnetNetFlow>, DispatchError> {
        if !SubnetsData::<T>::contains_key(subnet_id) {
            return Ok(None);
        }
        let epoch = Self::get_current_block_as_u32() / T::EpochLength::get().max(1);
        let mut checkpoint =
            Self::read_subnet_net_flow(subnet_id, epoch, SubnetNetFlowSmoothingAlpha::<T>::get());
        checkpoint.flow = checkpoint
            .flow
            .checked_add(SignedNetFlow::from_amount(withdrawal, amount))
            .ok_or(ArithmeticError::Overflow)?;
        Ok(Some(checkpoint))
    }

    /// Stable softmax: subtract signed scores before converting to f64, preserving small
    /// differences between large amounts. K uses whole native units (18 decimal places).
    /// Q18 quantization may round negligible shares to zero; a maximum always survives.
    pub(crate) fn normalize_subnet_net_flows(
        scores: BTreeMap<u32, SignedNetFlow>,
        k: u128,
    ) -> BTreeMap<u32, u128> {
        let Some(highest) = scores.values().copied().max() else {
            return BTreeMap::new();
        };
        if k == 0 {
            return BTreeMap::new();
        }
        let q = U256::from(Self::percentage_factor_as_u128());
        // One factor converts whole native units to atomic units; the other matches the EMA's
        // fractional Q18 precision. u128::MAX * 1e36 * 42 fits U256.
        let denominator = U256::from(k) * U256::from(NET_FLOW_ATOMIC_UNITS_PER_UNIT) * q;
        let mut positives = BTreeMap::new();
        let mut total = U256::zero();
        for (id, score) in scores {
            let gap = highest.checked_add(SignedNetFlow::new(!score.negative, score.magnitude));
            let positive = match gap {
                Some(gap) if gap.magnitude < denominator * U256::from(42u32) => {
                    let ratio: U256 = (gap.magnitude.full_mul(q)
                        / denominator.full_mul(U256::one()))
                    .try_into()
                    .expect("ratio is below 42 Q18");
                    (libm::exp(-(ratio.as_u128() as f64 / Self::percentage_factor_as_f64()))
                        * Self::percentage_factor_as_f64()) as u128
                }
                _ => 0,
            }
            .min(Self::percentage_factor_as_u128());
            total += U256::from(positive);
            positives.insert(id, positive);
        }
        positives
            .into_iter()
            .map(|(id, value)| (id, (U256::from(value) * q / total).as_u128()))
            .collect()
    }
}
