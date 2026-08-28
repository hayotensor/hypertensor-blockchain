use super::*;
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
use frame_support::pallet_prelude::Pays;
use sp_runtime::traits::Hash;

impl<T: Config> Pallet<T> {
    pub fn do_commit_overwatch_subnet_weights(
        origin: T::RuntimeOrigin,
        overwatch_node_id: u32,
        commit_weights: Vec<OverwatchCommit<T::Hash>>,
    ) -> DispatchResultWithPostInfo {
        ensure!(
            commit_weights.len() as u32 <= MaxSubnets::<T>::get().saturating_add(1),
            Error::<T>::MaxSubnets
        );
        let hotkey: T::AccountId = ensure_signed(origin)?;

        // Resolve active ownership and authentication from the same canonical validator identity.
        let (validator_id, overwatch_hotkey) =
            Self::get_active_overwatch_validator_id_and_hotkey(overwatch_node_id)?;

        ensure!(overwatch_hotkey == hotkey, Error::<T>::NotKeyOwner);

        ensure!(
            OverwatchValidatorWhitelist::<T>::get(validator_id),
            Error::<T>::ValidatorNotOverwatchWhitelisted
        );

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

        for commit in commit_weights {
            Self::do_commit_subnet_weight(overwatch_node_id, commit, overwatch_epoch)
                .map_err(|e| e)?;
        }

        Ok(Pays::No.into())
    }

    pub fn do_commit_subnet_weight(
        overwatch_node_id: u32,
        commit: OverwatchCommit<T::Hash>,
        overwatch_epoch: u32,
    ) -> DispatchResult {
        ensure!(
            !OverwatchCommits::<T>::contains_key((
                overwatch_epoch,
                overwatch_node_id,
                commit.subnet_id
            )),
            Error::<T>::AlreadyCommitted
        );

        OverwatchCommits::<T>::insert(
            (overwatch_epoch, overwatch_node_id, commit.subnet_id),
            commit.weight,
        );

        Ok(())
    }

    pub fn do_reveal_overwatch_subnet_weights(
        origin: T::RuntimeOrigin,
        overwatch_node_id: u32,
        reveals: Vec<OverwatchReveal<T>>,
    ) -> DispatchResultWithPostInfo {
        ensure!(
            reveals.len() as u32 <= MaxSubnets::<T>::get().saturating_add(1),
            Error::<T>::MaxSubnets
        );
        let hotkey: T::AccountId = ensure_signed(origin)?;

        let (validator_id, overwatch_hotkey) =
            Self::get_active_overwatch_validator_id_and_hotkey(overwatch_node_id)?;

        ensure!(overwatch_hotkey == hotkey, Error::<T>::NotKeyOwner);

        ensure!(
            OverwatchValidatorWhitelist::<T>::get(validator_id),
            Error::<T>::ValidatorNotOverwatchWhitelisted
        );

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
        let overwatch_epoch = Self::get_current_overwatch_epoch_as_u32();
        let percentage_factor = Self::percentage_factor_as_u128();
        let mut staged_reveals = BTreeMap::<u32, u128>::new();
        let mut new_reveal_subnets = BTreeSet::<u32>::new();

        // Validate the complete batch before writing either the reveal map or its cardinality
        // index. The dispatchable may contain several reveals, so returning from the middle of the
        // loop would otherwise leave a valid prefix committed when a later reveal is invalid.
        for reveal in reveals {
            let subnet_id = reveal.subnet_id;
            let weight = reveal.weight;
            ensure!(weight <= percentage_factor, Error::<T>::InvalidWeight);
            let salt = reveal.salt;
            let Some(commit_hash) =
                OverwatchCommits::<T>::get((overwatch_epoch, overwatch_node_id, subnet_id))
            else {
                return Err(Error::<T>::NoCommitFound.into());
            };

            // Reconstruct hash from reveal
            let actual_hash = T::Hashing::hash_of(&(weight, salt.clone()));

            ensure!(actual_hash == commit_hash, Error::<T>::RevealMismatch);

            let reveal_key = (overwatch_epoch, subnet_id, overwatch_node_id);
            if !OverwatchReveals::<T>::contains_key(reveal_key) {
                // A duplicate subnet in this batch is still only one unique epoch/node/subnet
                // record. `staged_reveals` preserves the prior last-write-wins behavior.
                new_reveal_subnets.insert(subnet_id);
            }
            staged_reveals.insert(subnet_id, weight);
        }

        if !new_reveal_subnets.is_empty() {
            let mut stats = ActiveOverwatchRevealStats::<T>::get();
            stats
                .revealing_nodes
                .try_insert(overwatch_node_id)
                .map_err(|_| Error::<T>::MaxOverwatchRevealNodes)?;

            for subnet_id in &new_reveal_subnets {
                stats
                    .revealed_subnets
                    .try_insert(*subnet_id)
                    .map_err(|_| Error::<T>::MaxOverwatchRevealSubnets)?;
            }

            let max_records = T::MaxOverwatchNodesUpperBound::get()
                .saturating_mul(T::MaxPhysicalSubnetsUpperBound::get());
            let updated_records = stats
                .records
                .checked_add(new_reveal_subnets.len() as u32)
                .filter(|records| *records <= max_records)
                .ok_or(Error::<T>::MaxOverwatchRevealRecords)?;
            stats.records = updated_records;
            ActiveOverwatchRevealStats::<T>::put(stats);
        }

        for (subnet_id, weight) in staged_reveals {
            OverwatchReveals::<T>::insert((overwatch_epoch, subnet_id, overwatch_node_id), weight);
        }

        Ok(Pays::No.into())
    }
}
