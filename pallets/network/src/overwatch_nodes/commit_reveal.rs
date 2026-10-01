use super::*;
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
use frame_support::pallet_prelude::Pays;
use sp_runtime::traits::Hash;

impl<T: Config> Pallet<T> {
    /// Hash the SCALE tuple `(b"overwatch/subnet-weight/v1", overwatch_node_id,
    /// subnet_id, overwatch_epoch, weight, salt)` using the runtime's `T::Hashing`.
    /// The tag is a fixed byte array (no length prefix), IDs and epoch are `u32`,
    /// weight is `u128`, and salt is a byte sequence with a compact length prefix.
    ///
    /// Node IDs are unique and never reused; dispatch authenticates their current
    /// hotkey. Binding the ID preserves commitments across hotkey rotation while
    /// preventing another evaluator from opening a copied commitment. Subnet and
    /// epoch binding prevent replay in another assessment context.
    pub fn hash_overwatch_commitment(
        overwatch_node_id: u32,
        subnet_id: u32,
        overwatch_epoch: u32,
        weight: u128,
        salt: &[u8],
    ) -> T::Hash {
        T::Hashing::hash_of(&(
            b"overwatch/subnet-weight/v1",
            overwatch_node_id,
            subnet_id,
            overwatch_epoch,
            weight,
            salt,
        ))
    }

    pub fn do_commit_overwatch_subnet_weights(
        origin: T::RuntimeOrigin,
        overwatch_node_id: u32,
        commit_weights: Vec<OverwatchCommit<T::Hash>>,
    ) -> DispatchResultWithPostInfo {
        ensure!(
            commit_weights.len() as u32
                <= MaxSubnets::<T>::get().saturating_add(SUBNET_ROTATION_ALLOWANCE),
            Error::<T>::MaxSubnets
        );
        let hotkey: T::AccountId = ensure_signed(origin)?;

        // Resolve active ownership and authentication from the same canonical validator identity.
        let (_, overwatch_hotkey) =
            Self::get_active_overwatch_validator_id_and_hotkey(overwatch_node_id)?;

        ensure!(overwatch_hotkey == hotkey, Error::<T>::NotKeyOwner);

        // --- Check if we are in commit period
        ensure!(
            Self::in_overwatch_commit_period(),
            Error::<T>::NotCommitPeriod
        );

        Self::perform_commit_overwatch_subnet_weights(overwatch_node_id, commit_weights)
    }

    pub fn perform_commit_overwatch_subnet_weights(
        overwatch_node_id: u32,
        mut commit_weights: Vec<OverwatchCommit<T::Hash>>,
    ) -> DispatchResultWithPostInfo {
        ensure!(!commit_weights.is_empty(), Error::<T>::CommitsEmpty);

        // Remove duplicate subnet IDs regardless of their position in the submitted vector.
        let mut seen_subnets = BTreeSet::new();
        commit_weights.retain(|commit| {
            seen_subnets.insert(commit.subnet_id)
                && SubnetsData::<T>::contains_key(commit.subnet_id)
        });

        ensure!(!commit_weights.is_empty(), Error::<T>::CommitsEmpty);

        let overwatch_epoch = Self::get_current_overwatch_epoch_as_u32();
        Self::ensure_overwatch_epoch_eligible(overwatch_node_id, overwatch_epoch)?;
        let mut row = OverwatchCommits::<T>::get(overwatch_epoch, overwatch_node_id);

        // Validate and stage the complete cumulative row before its single storage write. This
        // prevents both prefix writes on failure and repeated-call subnet churn beyond the bound.
        for commit in commit_weights {
            ensure!(
                !row.contains_key(&commit.subnet_id),
                Error::<T>::AlreadyCommitted
            );
            row.try_insert(commit.subnet_id, commit.weight)
                .map_err(|_| Error::<T>::MaxSubnets)?;
        }

        OverwatchCommits::<T>::insert(overwatch_epoch, overwatch_node_id, row);

        Ok(Pays::No.into())
    }

    pub fn do_reveal_overwatch_subnet_weights(
        origin: T::RuntimeOrigin,
        overwatch_node_id: u32,
        reveals: Vec<OverwatchReveal<T>>,
    ) -> DispatchResultWithPostInfo {
        ensure!(
            reveals.len() as u32
                <= MaxSubnets::<T>::get().saturating_add(SUBNET_ROTATION_ALLOWANCE),
            Error::<T>::MaxSubnets
        );
        let hotkey: T::AccountId = ensure_signed(origin)?;

        let (_, overwatch_hotkey) =
            Self::get_active_overwatch_validator_id_and_hotkey(overwatch_node_id)?;

        ensure!(overwatch_hotkey == hotkey, Error::<T>::NotKeyOwner);

        // --- Check if we are in reveal period
        ensure!(
            !Self::in_overwatch_commit_period(),
            Error::<T>::NotRevealPeriod
        );

        Self::perform_reveal_overwatch_subnet_weights(overwatch_node_id, reveals)
    }

    pub fn perform_reveal_overwatch_subnet_weights(
        overwatch_node_id: u32,
        reveals: Vec<OverwatchReveal<T>>,
    ) -> DispatchResultWithPostInfo {
        ensure!(!reveals.is_empty(), Error::<T>::RevealsEmpty);

        let overwatch_epoch = Self::get_current_overwatch_epoch_as_u32();
        Self::ensure_overwatch_epoch_eligible(overwatch_node_id, overwatch_epoch)?;
        let percentage_factor = Self::percentage_factor_as_u128();
        let mut staged_reveals = BTreeMap::<u32, u128>::new();
        let commits = OverwatchCommits::<T>::get(overwatch_epoch, overwatch_node_id);
        let mut reveal_row = OverwatchReveals::<T>::get(overwatch_epoch, overwatch_node_id);

        // Validate the complete batch before writing either the reveal map or its cardinality
        // index. The dispatchable may contain several reveals, so returning from the middle of the
        // loop would otherwise leave a valid prefix committed when a later reveal is invalid.
        for reveal in reveals {
            let subnet_id = reveal.subnet_id;
            let weight = reveal.weight;
            ensure!(weight <= percentage_factor, Error::<T>::InvalidWeight);
            let salt = reveal.salt;
            let Some(commit_hash) = commits.get(&subnet_id) else {
                return Err(Error::<T>::NoCommitFound.into());
            };

            // Derive context from the authenticated node and active round, not the preimage.
            let actual_hash = Self::hash_overwatch_commitment(
                overwatch_node_id,
                subnet_id,
                overwatch_epoch,
                weight,
                &salt,
            );

            ensure!(actual_hash == *commit_hash, Error::<T>::RevealMismatch);

            staged_reveals.insert(subnet_id, weight);
        }

        let mut stats = ActiveOverwatchRevealStats::<T>::get();
        let mut new_records = 0u32;
        for (subnet_id, weight) in staged_reveals {
            if !reveal_row.contains_key(&subnet_id) {
                new_records = new_records.saturating_add(1);
                match stats.subnet_revealer_counts.get_mut(&subnet_id) {
                    Some(count) => *count = count.saturating_add(1),
                    None => {
                        stats
                            .subnet_revealer_counts
                            .try_insert(subnet_id, 1)
                            .map_err(|_| Error::<T>::MaxOverwatchRevealSubnets)?;
                    }
                }
            }
            reveal_row
                .try_insert(subnet_id, weight)
                .map_err(|_| Error::<T>::MaxSubnets)?;
        }

        if new_records != 0 {
            let max_records = T::MaxOverwatchNodesUpperBound::get()
                .saturating_mul(T::MaxPhysicalSubnetsUpperBound::get());
            stats.records = stats
                .records
                .checked_add(new_records)
                .filter(|records| *records <= max_records)
                .ok_or(Error::<T>::MaxOverwatchRevealRecords)?;
            ActiveOverwatchRevealStats::<T>::put(stats);
        }
        OverwatchReveals::<T>::insert(overwatch_epoch, overwatch_node_id, reveal_row);

        Ok(Pays::No.into())
    }
}
