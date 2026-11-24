use super::*;
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
use frame_support::pallet_prelude::Pays;
use frame_support::BoundedBTreeMap;
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
            commit_weights.len()
                <= MaxSubnets::<T>::get().saturating_add(SUBNET_ROTATION_ALLOWANCE) as usize,
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
        commit_weights: Vec<OverwatchCommit<T::Hash>>,
    ) -> DispatchResultWithPostInfo {
        ensure!(!commit_weights.is_empty(), Error::<T>::CommitsEmpty);

        ensure!(
            commit_weights.len() <= T::MaxPhysicalSubnetsUpperBound::get() as usize,
            Error::<T>::MaxSubnets
        );

        let overwatch_epoch = Self::get_current_overwatch_epoch_as_u32();
        Self::ensure_overwatch_epoch_eligible(overwatch_node_id, overwatch_epoch)?;
        ensure!(
            !OverwatchCommits::<T>::contains_key(overwatch_epoch, overwatch_node_id),
            Error::<T>::AlreadyCommitted
        );
        let mut row = BoundedBTreeMap::<u32, T::Hash, T::MaxPhysicalSubnetsUpperBound>::new();

        // The successful row is the epoch's submission marker. Reject the whole batch on an
        // invalid entry so a corrected retry can still submit the node's complete commitment set.
        for commit in &commit_weights {
            ensure!(
                !row.contains_key(&commit.subnet_id),
                Error::<T>::DuplicateOverwatchSubnetId
            );
            ensure!(
                SubnetsData::<T>::contains_key(commit.subnet_id),
                Error::<T>::InvalidSubnetId
            );
            row.try_insert(commit.subnet_id, commit.weight)
                .map_err(|_| Error::<T>::MaxSubnets)?;
        }

        OverwatchCommits::<T>::insert(overwatch_epoch, overwatch_node_id, row);

        Self::deposit_event(Event::OverwatchWeightsCommitted {
            overwatch_node_id,
            overwatch_epoch,
            commits: commit_weights,
        });

        Ok(Pays::No.into())
    }

    pub fn do_reveal_overwatch_subnet_weights(
        origin: T::RuntimeOrigin,
        overwatch_node_id: u32,
        reveals: Vec<OverwatchReveal<T>>,
    ) -> DispatchResultWithPostInfo {
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
        // A later governance reduction of MaxSubnets must not strand accepted commitments.
        ensure!(
            reveals.len() <= T::MaxPhysicalSubnetsUpperBound::get() as usize,
            Error::<T>::MaxSubnets
        );

        let overwatch_epoch = Self::get_current_overwatch_epoch_as_u32();
        Self::ensure_overwatch_epoch_eligible(overwatch_node_id, overwatch_epoch)?;
        ensure!(
            !OverwatchReveals::<T>::contains_key(overwatch_epoch, overwatch_node_id),
            Error::<T>::AlreadyRevealed
        );
        let percentage_factor = Self::percentage_factor_as_u128();
        let commits = OverwatchCommits::<T>::get(overwatch_epoch, overwatch_node_id);
        ensure!(!commits.is_empty(), Error::<T>::NoCommitFound);
        let mut submitted_ids = BTreeSet::new();
        for reveal in &reveals {
            ensure!(
                submitted_ids.insert(reveal.subnet_id),
                Error::<T>::DuplicateOverwatchSubnetId
            );
        }
        ensure!(
            submitted_ids.iter().eq(commits.keys()),
            Error::<T>::RevealSetMismatch
        );
        let mut reveal_row = BoundedBTreeMap::<u32, u128, T::MaxPhysicalSubnetsUpperBound>::new();

        // Validate the complete batch before writing either the reveal map or its cardinality
        // index. The dispatchable may contain several reveals, so returning from the middle of the
        // loop would otherwise leave a valid prefix committed when a later reveal is invalid.
        for reveal in reveals {
            let subnet_id = reveal.subnet_id;
            let weight = reveal.weight;
            ensure!(weight <= percentage_factor, Error::<T>::InvalidWeight);
            let salt = reveal.salt;
            let commit_hash = commits
                .get(&subnet_id)
                .ok_or(Error::<T>::RevealSetMismatch)?;

            // Derive context from the authenticated node and active round, not the preimage.
            let actual_hash = Self::hash_overwatch_commitment(
                overwatch_node_id,
                subnet_id,
                overwatch_epoch,
                weight,
                &salt,
            );

            ensure!(actual_hash == *commit_hash, Error::<T>::RevealMismatch);

            reveal_row
                .try_insert(subnet_id, weight)
                .map_err(|_| Error::<T>::MaxSubnets)?;
        }

        let mut stats = ActiveOverwatchRevealStats::<T>::get();
        for &subnet_id in reveal_row.keys() {
            // Removed subnets still complete the reveal but take no distinct-subnet slot.
            if SubnetsData::<T>::contains_key(subnet_id) {
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
        }

        let max_records = T::MaxOverwatchNodesUpperBound::get()
            .saturating_mul(T::MaxPhysicalSubnetsUpperBound::get());
        stats.records = stats
            .records
            .checked_add(reveal_row.len() as u32)
            .filter(|records| *records <= max_records)
            .ok_or(Error::<T>::MaxOverwatchRevealRecords)?;
        ActiveOverwatchRevealStats::<T>::put(stats);
        OverwatchReveals::<T>::insert(overwatch_epoch, overwatch_node_id, &reveal_row);

        Self::deposit_event(Event::OverwatchWeightsRevealed {
            overwatch_node_id,
            overwatch_epoch,
            weights: reveal_row.into_iter().collect(),
        });

        Ok(Pays::No.into())
    }
}
