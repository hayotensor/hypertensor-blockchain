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
// Handles all owner related subnet operations
// See all storage elements for docs in `lib.rs`

use super::*;

impl<T: Config> Pallet<T> {
    fn next_owner_parameter_effective_subnet_epoch(current_subnet_epoch: u32) -> u32 {
        current_subnet_epoch.saturating_add(OWNER_PARAMETER_ACTIVATION_DELAY_EPOCHS)
    }

    fn prepare_pending_owner_u32_update(
        current_subnet_epoch: u32,
        pending: Option<PendingOwnerU32Update<T>>,
        materialize: impl FnOnce(u32),
    ) -> DispatchResult {
        if let Some(pending) = pending {
            ensure!(
                pending.effective_subnet_epoch != current_subnet_epoch,
                Error::<T>::OwnerParameterUpdatePendingActivation
            );

            if pending.effective_subnet_epoch < current_subnet_epoch {
                materialize(pending.value);
            }
        }

        Ok(())
    }

    fn prepare_pending_owner_u128_update(
        current_subnet_epoch: u32,
        pending: Option<PendingOwnerU128Update<T>>,
        materialize: impl FnOnce(u128),
    ) -> DispatchResult {
        if let Some(pending) = pending {
            ensure!(
                pending.effective_subnet_epoch != current_subnet_epoch,
                Error::<T>::OwnerParameterUpdatePendingActivation
            );

            if pending.effective_subnet_epoch < current_subnet_epoch {
                materialize(pending.value);
            }
        }

        Ok(())
    }

    fn pending_owner_u32_value_for_epoch(
        live_value: u32,
        pending: Option<PendingOwnerU32Update<T>>,
        subnet_epoch: u32,
    ) -> u32 {
        pending
            .filter(|pending| pending.effective_subnet_epoch <= subnet_epoch)
            .map(|pending| pending.value)
            .unwrap_or(live_value)
    }

    fn pending_owner_u128_value_for_epoch(
        live_value: u128,
        pending: Option<PendingOwnerU128Update<T>>,
        subnet_epoch: u32,
    ) -> u128 {
        pending
            .filter(|pending| pending.effective_subnet_epoch <= subnet_epoch)
            .map(|pending| pending.value)
            .unwrap_or(live_value)
    }

    pub fn get_min_subnet_node_reputation_for_epoch(subnet_id: u32, subnet_epoch: u32) -> u128 {
        Self::pending_owner_u128_value_for_epoch(
            MinSubnetNodeReputation::<T>::get(subnet_id),
            PendingMinSubnetNodeReputation::<T>::get(subnet_id),
            subnet_epoch,
        )
    }

    pub fn get_subnet_node_min_weight_decrease_reputation_threshold_for_epoch(
        subnet_id: u32,
        subnet_epoch: u32,
    ) -> u128 {
        Self::pending_owner_u128_value_for_epoch(
            SubnetNodeMinWeightDecreaseReputationThreshold::<T>::get(subnet_id),
            PendingSubnetNodeMinWeightDecreaseReputationThreshold::<T>::get(subnet_id),
            subnet_epoch,
        )
    }

    pub fn get_subnet_node_queue_epochs_for_epoch(subnet_id: u32, subnet_epoch: u32) -> u32 {
        Self::pending_owner_u32_value_for_epoch(
            SubnetNodeQueueEpochs::<T>::get(subnet_id),
            PendingSubnetNodeQueueEpochs::<T>::get(subnet_id),
            subnet_epoch,
        )
    }

    pub fn get_probation_rounds_for_epoch(subnet_id: u32, subnet_epoch: u32) -> u32 {
        Self::pending_owner_u32_value_for_epoch(
            ProbationRounds::<T>::get(subnet_id),
            PendingProbationRounds::<T>::get(subnet_id),
            subnet_epoch,
        )
    }

    pub fn get_queue_immunity_epochs_for_epoch(subnet_id: u32, subnet_epoch: u32) -> u32 {
        Self::pending_owner_u32_value_for_epoch(
            QueueImmunityEpochs::<T>::get(subnet_id),
            PendingQueueImmunityEpochs::<T>::get(subnet_id),
            subnet_epoch,
        )
    }

    pub fn get_subnet_delegate_stake_rewards_percentage_for_epoch(
        subnet_id: u32,
        subnet_epoch: u32,
    ) -> u128 {
        PendingSubnetDelegateStakeRewardsPercentage::<T>::get(subnet_id)
            .filter(|pending| pending.effective_subnet_epoch <= subnet_epoch)
            .map(|pending| pending.value)
            .unwrap_or_else(|| SubnetDelegateStakeRewardsPercentage::<T>::get(subnet_id))
    }

    pub fn get_consensus_validator_node_count_decay_for_epoch(
        subnet_id: u32,
        subnet_epoch: u32,
    ) -> u128 {
        Self::pending_owner_u128_value_for_epoch(
            ConsensusValidatorNodeCountDecay::<T>::get(subnet_id),
            PendingConsensusValidatorNodeCountDecay::<T>::get(subnet_id),
            subnet_epoch,
        )
    }

    pub fn get_consensus_validator_stake_weight_power_for_epoch(
        subnet_id: u32,
        subnet_epoch: u32,
    ) -> u128 {
        Self::pending_owner_u128_value_for_epoch(
            ConsensusValidatorStakeWeightPower::<T>::get(subnet_id),
            PendingConsensusValidatorStakeWeightPower::<T>::get(subnet_id),
            subnet_epoch,
        )
    }

    /// Return every subnet node that is logically quarantined while physical cleanup is pending.
    ///
    /// Callers that scan more than one node should take this single snapshot and filter in memory,
    /// rather than decoding both bounded pending-removal sets once per candidate.
    pub(crate) fn pending_subnet_node_removal_ids(subnet_id: u32) -> BTreeSet<u32> {
        let pending_admitted = PendingAdmittedNodeRemovals::<T>::get(subnet_id);
        let pending_queued = PendingQueuedNodeRemovals::<T>::get(subnet_id);
        let mut pending_ids = BTreeSet::new();

        pending_ids.extend(pending_admitted.iter().copied());
        pending_ids.extend(pending_queued.iter().copied());
        pending_ids
    }

    pub(crate) fn filter_pending_subnet_node_ids(
        subnet_node_ids: Vec<u32>,
        pending_ids: &BTreeSet<u32>,
    ) -> Vec<u32> {
        subnet_node_ids
            .into_iter()
            .filter(|subnet_node_id| !pending_ids.contains(subnet_node_id))
            .collect()
    }

    /// Return the canonical validator candidates, excluding nodes pending removal.
    pub fn effective_consensus_validator_ids(subnet_id: u32) -> Vec<u32> {
        let pending_ids = Self::pending_subnet_node_removal_ids(subnet_id);
        Self::filter_pending_subnet_node_ids(
            Self::resolve_consensus_validator_ids(subnet_id),
            &pending_ids,
        )
    }

    /// Return the candidate cardinality used to reserve weight before election mutates storage.
    pub fn elect_validator_weight_component(subnet_id: u32) -> u32 {
        TotalSubnetElectableNodes::<T>::get(subnet_id)
            .min(T::MaxSubnetNodesUpperBound::get())
            .max(Self::MIN_CONSENSUS_VALIDATOR_IDENTITIES)
    }

    /// Resolve the canonical physical election slots without filtering quarantined nodes.
    /// Election keeps this domain intact for its random draw and skips quarantined candidates.
    pub fn resolve_consensus_validator_ids(subnet_id: u32) -> Vec<u32> {
        Self::canonicalize_consensus_validator_ids(SubnetNodeElectionSlots::<T>::get(subnet_id))
    }

    /// Owner can fully remove the subnet
    ///
    /// # Arguments
    /// * `origin` - The origin of the transaction
    /// * `subnet_id` - The id of the subnet
    ///
    /// # Returns
    /// * `DispatchResult` - The result of the transaction
    pub fn do_owner_deactivate_subnet(origin: T::RuntimeOrigin, subnet_id: u32) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        // Redundant
        ensure!(
            SubnetsData::<T>::contains_key(subnet_id),
            Error::<T>::InvalidSubnetId
        );
        ensure!(
            !PendingConsensusRoundSettlementEpoch::<T>::contains_key(subnet_id),
            Error::<T>::ConsensusRoundPendingSettlement
        );

        Self::do_remove_subnet(subnet_id, SubnetRemovalReason::Owner);

        Ok(())
    }

    pub fn do_owner_update_name(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: Vec<u8>,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        Self::ensure_subnet_name_bounded(&value)?;

        let subnet = SubnetsData::<T>::get(subnet_id).ok_or(Error::<T>::InvalidSubnetId)?;
        if subnet.name == value {
            return Ok(());
        }

        if let Some(existing_subnet_id) = SubnetName::<T>::get(&value) {
            ensure!(existing_subnet_id == subnet_id, Error::<T>::SubnetNameExist);
        }

        let prev_name = subnet.name;
        SubnetsData::<T>::try_mutate_exists(subnet_id, |maybe_params| -> DispatchResult {
            let params = maybe_params.as_mut().ok_or(Error::<T>::InvalidSubnetId)?;

            SubnetName::<T>::remove(&prev_name);

            params.name = value.clone();

            Ok(())
        })?;

        SubnetName::<T>::insert(&value, subnet_id);

        Self::deposit_event(Event::SubnetNameUpdate {
            subnet_id: subnet_id,
            owner: coldkey,
            prev_value: prev_name,
            value: value,
        });

        Ok(())
    }

    pub fn do_owner_update_repo(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: Vec<u8>,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        Self::ensure_subnet_repo_bounded(&value)?;

        let subnet = SubnetsData::<T>::get(subnet_id).ok_or(Error::<T>::InvalidSubnetId)?;
        if subnet.repo == value {
            return Ok(());
        }

        if let Some(existing_subnet_id) = SubnetRepo::<T>::get(&value) {
            ensure!(existing_subnet_id == subnet_id, Error::<T>::SubnetRepoExist);
        }

        let prev_repo = subnet.repo;
        SubnetsData::<T>::try_mutate_exists(subnet_id, |maybe_params| -> DispatchResult {
            let params = maybe_params.as_mut().ok_or(Error::<T>::InvalidSubnetId)?;

            SubnetRepo::<T>::remove(&prev_repo);

            params.repo = value.clone();

            Ok(())
        })?;

        SubnetRepo::<T>::insert(&value, subnet_id);

        Self::deposit_event(Event::SubnetRepoUpdate {
            subnet_id: subnet_id,
            owner: coldkey,
            prev_value: prev_repo,
            value: value,
        });

        Ok(())
    }

    pub fn do_owner_update_description(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: Vec<u8>,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        Self::ensure_subnet_description_bounded(&value)?;

        let mut prev_description: Vec<u8> = Vec::new();
        SubnetsData::<T>::try_mutate_exists(subnet_id, |maybe_params| -> DispatchResult {
            let params = maybe_params.as_mut().ok_or(Error::<T>::InvalidSubnetId)?;

            prev_description = params.description.clone();
            params.description = value.clone();

            Ok(())
        })?;

        Self::deposit_event(Event::SubnetDescriptionUpdate {
            subnet_id: subnet_id,
            owner: coldkey,
            prev_value: prev_description,
            value: value,
        });

        Ok(())
    }

    pub fn do_owner_update_misc(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: Vec<u8>,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        Self::ensure_subnet_misc_bounded(&value)?;

        let mut prev_misc: Vec<u8> = Vec::new();
        SubnetsData::<T>::try_mutate_exists(subnet_id, |maybe_params| -> DispatchResult {
            let params = maybe_params.as_mut().ok_or(Error::<T>::InvalidSubnetId)?;

            prev_misc = params.misc.clone();
            params.misc = value.clone();

            Ok(())
        })?;

        Self::deposit_event(Event::SubnetMiscUpdate {
            subnet_id: subnet_id,
            owner: coldkey,
            prev_value: prev_misc,
            value: value,
        });

        Ok(())
    }

    pub fn do_owner_update_churn_limit(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: u32,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            value >= MinChurnLimit::<T>::get() && value <= MaxChurnLimit::<T>::get(),
            Error::<T>::InvalidChurnLimit
        );

        ChurnLimit::<T>::insert(subnet_id, value);

        Self::deposit_event(Event::ChurnLimitUpdate {
            subnet_id: subnet_id,
            owner: coldkey,
            value: value,
        });

        Ok(())
    }

    pub fn do_owner_update_churn_limit_multiplier(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: u32,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            value >= MIN_CHURN_LIMIT_MULTIPLIER
                && value >= MinChurnLimitMultiplier::<T>::get()
                && value <= MaxChurnLimitMultiplier::<T>::get(),
            Error::<T>::InvalidChurnLimitMultiplier
        );

        ChurnLimitMultiplier::<T>::insert(subnet_id, value);

        Self::deposit_event(Event::ChurnLimitMultiplierUpdate {
            subnet_id: subnet_id,
            owner: coldkey,
            value: value,
        });

        Ok(())
    }

    pub fn do_owner_update_registration_queue_epochs(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: u32,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            value >= MinQueueEpochs::<T>::get() && value <= MaxQueueEpochs::<T>::get(),
            Error::<T>::InvalidRegistrationQueueEpochs
        );

        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        Self::prepare_pending_owner_u32_update(
            current_subnet_epoch,
            PendingSubnetNodeQueueEpochs::<T>::get(subnet_id),
            |value| SubnetNodeQueueEpochs::<T>::insert(subnet_id, value),
        )?;
        let effective_subnet_epoch =
            Self::next_owner_parameter_effective_subnet_epoch(current_subnet_epoch);
        PendingSubnetNodeQueueEpochs::<T>::insert(
            subnet_id,
            PendingOwnerU32Update {
                value,
                effective_subnet_epoch,
                owner: coldkey.clone(),
            },
        );

        Self::deposit_event(Event::RegistrationQueueEpochsUpdateScheduled {
            subnet_id: subnet_id,
            owner: coldkey,
            value: value,
            effective_subnet_epoch,
        });

        Ok(())
    }

    pub fn do_owner_update_probation_rounds(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: u32,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            value >= MinProbationRounds::<T>::get() && value <= MaxProbationRounds::<T>::get(),
            Error::<T>::InvalidProbationRounds
        );

        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        Self::prepare_pending_owner_u32_update(
            current_subnet_epoch,
            PendingProbationRounds::<T>::get(subnet_id),
            |value| ProbationRounds::<T>::insert(subnet_id, value),
        )?;
        let effective_subnet_epoch =
            Self::next_owner_parameter_effective_subnet_epoch(current_subnet_epoch);
        PendingProbationRounds::<T>::insert(
            subnet_id,
            PendingOwnerU32Update {
                value,
                effective_subnet_epoch,
                owner: coldkey.clone(),
            },
        );

        Self::deposit_event(Event::ProbationRoundsUpdateScheduled {
            subnet_id: subnet_id,
            owner: coldkey,
            value: value,
            effective_subnet_epoch,
        });

        Ok(())
    }

    pub fn do_owner_add_or_update_initial_validators(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        validators: BTreeMap<u32, u32>,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            Self::is_subnet_registered(subnet_id).unwrap_or(false),
            Error::<T>::SubnetMustBeRegistering
        );

        ensure!(
            validators.len() <= T::MaxRegisteredNodesUpperBound::get() as usize,
            Error::<T>::InvalidSubnetRegistrationInitialColdkeys
        );

        ensure!(
            validators
                .values()
                .all(|&value| value >= MIN_INITIAL_VALIDATOR_REGISTRATIONS),
            Error::<T>::InvalidSubnetRegistrationInitialColdkeys
        );

        NodeRegistrationInitialValidatorIds::<T>::try_mutate(
            subnet_id,
            |maybe_validators| -> DispatchResult {
                let validators_set = maybe_validators.get_or_insert_with(BTreeMap::new);
                validators_set.extend(
                    validators
                        .iter()
                        .map(|(&validator_id, &max_registrations)| {
                            (validator_id, max_registrations)
                        }),
                );
                ensure!(
                    validators_set.len() <= T::MaxRegisteredNodesUpperBound::get() as usize,
                    Error::<T>::InvalidSubnetRegistrationInitialColdkeys
                );

                // Registration counters deliberately survive whitelist removals. Bound the
                // cumulative identity union so repeatedly rotating whitelist entries cannot grow
                // `InitialValidatorData` beyond the proof-size domain used by cleanup weights.
                let tracked_validators = InitialValidatorData::<T>::get(subnet_id);
                let cumulative_identity_count = validators_set
                    .keys()
                    .chain(
                        tracked_validators
                            .iter()
                            .flat_map(|registrations| registrations.keys()),
                    )
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .len();
                ensure!(
                    cumulative_identity_count <= T::MaxRegisteredNodesUpperBound::get() as usize,
                    Error::<T>::InvalidSubnetRegistrationInitialColdkeys
                );
                Ok(())
            },
        )?;

        Self::deposit_event(Event::AddSubnetRegistrationInitialValidators {
            subnet_id: subnet_id,
            owner: coldkey,
            validators: validators,
        });

        Ok(())
    }

    pub fn do_owner_remove_initial_validators(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        validators: BTreeSet<u32>,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            Self::is_subnet_registered(subnet_id).unwrap_or(false),
            Error::<T>::SubnetMustBeRegistering
        );

        // A fixed-weight call must reject an oversized caller-supplied set before its removal
        // loop, including when every supplied validator ID is absent from the stored whitelist.
        ensure!(
            validators.len() <= T::MaxRegisteredNodesUpperBound::get() as usize,
            Error::<T>::InvalidSubnetRegistrationInitialColdkeys
        );

        NodeRegistrationInitialValidatorIds::<T>::mutate(subnet_id, |maybe_validators| {
            if let Some(existing_validators) = maybe_validators {
                // Remove the requested validators from storage.
                for validator_id in &validators {
                    existing_validators.remove(validator_id);
                }

                // Clean up if the set becomes empty
                if existing_validators.is_empty() {
                    *maybe_validators = None;
                }
            }
        });

        Self::deposit_event(Event::RemoveSubnetRegistrationInitialValidators {
            subnet_id: subnet_id,
            owner: coldkey,
            validators: validators,
        });

        Ok(())
    }

    pub fn do_owner_update_min_max_stake(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        min: u128,
        max: u128,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(min <= max, Error::<T>::InvalidValues);

        ensure!(
            min >= MinSubnetMinStake::<T>::get() && min <= MaxSubnetMinStake::<T>::get(),
            Error::<T>::InvalidSubnetMinStake
        );

        ensure!(
            max <= NetworkMaxStakeBalance::<T>::get(),
            Error::<T>::InvalidSubnetMaxStake
        );

        SubnetMinStakeBalance::<T>::insert(subnet_id, min);
        SubnetMaxStakeBalance::<T>::insert(subnet_id, max);

        Self::deposit_event(Event::SubnetMinMaxStakeBalanceUpdate {
            subnet_id: subnet_id,
            owner: coldkey.clone(),
            min: min,
            max: max,
        });

        Ok(())
    }

    /// Update delegate stake percentage
    ///
    /// This function can only be called by the current owner of the subnet.  
    ///
    /// # Parameters
    /// - `origin`: The caller, must be the current subnet owner.
    /// - `subnet_id`: The ID of the subnet.
    /// - `value`: The new percentage (1e18 = 1.0) share of rewards to delegate stakers.
    ///
    /// # Errors
    /// - [`NotSubnetOwner`]: Caller is not the owner of the subnet.
    /// - [`DelegateStakePercentageUpdateTooSoon`]: Updated too soon.
    /// - [`DelegateStakePercentageAbsDiffTooLarge`]: Value change too large.
    /// - [`InvalidDelegateStakePercentage`]: Value is not in allowable range.
    pub fn do_owner_update_delegate_stake_percentage(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: u128,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        let block = Self::get_current_block_as_u32();
        let last_update = LastSubnetDelegateStakeRewardsUpdate::<T>::get(subnet_id);
        let update_period = SubnetDelegateStakeRewardsUpdatePeriod::<T>::get();

        ensure!(
            last_update.saturating_add(update_period) < block,
            Error::<T>::DelegateStakePercentageUpdateTooSoon
        );

        let current_rate = PendingSubnetDelegateStakeRewardsPercentage::<T>::get(subnet_id)
            .map(|pending| pending.value)
            .unwrap_or_else(|| SubnetDelegateStakeRewardsPercentage::<T>::get(subnet_id));
        let max_change = MaxSubnetDelegateStakeRewardsPercentageChange::<T>::get();

        ensure!(
            current_rate.abs_diff(value) <= max_change,
            Error::<T>::DelegateStakePercentageAbsDiffTooLarge
        );

        ensure!(
            value >= MinDelegateStakePercentage::<T>::get()
                && value <= MaxDelegateStakePercentage::<T>::get()
                && value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidDelegateStakePercentage
        );

        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        let effective_subnet_epoch =
            Self::next_owner_parameter_effective_subnet_epoch(current_subnet_epoch);

        LastSubnetDelegateStakeRewardsUpdate::<T>::insert(subnet_id, block);
        PendingSubnetDelegateStakeRewardsPercentage::<T>::insert(
            subnet_id,
            PendingSubnetDelegateStakeRewardsPercentageUpdate {
                value,
                effective_subnet_epoch,
                owner: coldkey.clone(),
            },
        );

        Self::deposit_event(Event::SubnetDelegateStakeRewardsPercentageUpdateScheduled {
            subnet_id: subnet_id,
            owner: coldkey,
            value: value,
            effective_subnet_epoch,
        });

        Ok(())
    }

    /// Update the maximum registration-queue length.
    ///
    /// This function can only be called by the current owner of the subnet.  
    ///
    /// # Parameters
    /// - `origin`: The caller, must be the current subnet owner.
    /// - `subnet_id`: The ID of the subnet.
    /// - `value`: The new maximum number of Queued nodes in the registration queue.
    ///
    /// # Errors
    /// - [`NotSubnetOwner`]: Caller is not the owner of the subnet.
    /// - [`InvalidMaxRegisteredNodes`]: Value is not in allowable range.
    pub fn do_owner_update_max_registered_nodes(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: u32,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            value >= MinMaxRegisteredNodes::<T>::get()
                && value <= MaxMaxRegisteredNodes::<T>::get()
                && value >= TargetNodeRegistrationsPerEpoch::<T>::get(subnet_id),
            Error::<T>::InvalidMaxRegisteredNodes
        );

        MaxRegisteredNodes::<T>::insert(subnet_id, value);

        Self::deposit_event(Event::MaxRegisteredNodesUpdate {
            subnet_id: subnet_id,
            owner: coldkey,
            value: value,
        });

        Ok(())
    }

    /// Initiates the transfer of a subnet's ownership to a new account using a 2-step model.
    ///
    /// This function can only be called by the current owner of the subnet.  
    /// It sets a pending owner, who must later explicitly accept the transfer via
    /// [`do_accept_subnet_ownership`]. Ownership is not transferred immediately.
    ///
    /// # Parameters
    /// - `origin`: The caller, must be the current subnet owner.
    /// - `subnet_id`: The ID of the subnet.
    /// - `new_owner`: The `AccountId` of the new proposed owner.
    ///
    /// # Errors
    /// - [`NotSubnetOwner`]: Caller is not the owner of the subnet.
    /// - [`InvalidPendingSubnetOwner`]: New owner is the zero account.
    pub fn do_transfer_subnet_ownership(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        new_owner: T::AccountId,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        let zero_owner = T::AccountId::decode(&mut TrailingZeroInput::zeroes()).unwrap();
        ensure!(
            new_owner != zero_owner,
            Error::<T>::InvalidPendingSubnetOwner
        );

        PendingSubnetOwner::<T>::insert(subnet_id, &new_owner);

        Self::deposit_event(Event::TransferPendingSubnetOwner {
            subnet_id: subnet_id,
            owner: coldkey,
            new_owner: new_owner,
        });

        Ok(())
    }

    /// Cancels a pending subnet ownership transfer.
    ///
    /// This function can only be called by the current owner of the subnet.
    ///
    /// # Parameters
    /// - `origin`: The caller, must be the current subnet owner.
    /// - `subnet_id`: The ID of the subnet.
    ///
    /// # Errors
    /// - [`NotSubnetOwner`]: Caller is not the owner of the subnet.
    /// - [`NoPendingSubnetOwner`]: No transfer is pending.
    pub fn do_cancel_subnet_ownership_transfer(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            PendingSubnetOwner::<T>::contains_key(subnet_id),
            Error::<T>::NoPendingSubnetOwner
        );

        PendingSubnetOwner::<T>::remove(subnet_id);

        Self::deposit_event(Event::CancelPendingSubnetOwner {
            subnet_id,
            owner: coldkey,
        });

        Ok(())
    }

    /// Accepts ownership of a subnet that was previously offered via a transfer.
    ///
    /// This function must be called by the account set as the `PendingSubnetOwner`
    /// for the specified subnet. Upon successful execution, the caller becomes
    /// the new `SubnetOwner`.
    ///
    /// # Parameters
    /// - `origin`: The caller, must match the pending owner.
    /// - `subnet_id`: The ID of the subnet being claimed.
    ///
    /// # Errors
    /// - [`NoPendingSubnetOwner`]: No transfer was initiated.
    /// - [`NotPendingSubnetOwner`]: Caller is not the designated pending owner.
    /// - [`InvalidSubnetId`]: Subnet does not exist or has no registered owner.
    pub fn do_accept_subnet_ownership(origin: T::RuntimeOrigin, subnet_id: u32) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        // Ensure is pending subnet owner
        // let pending_owner: T::AccountId = PendingSubnetOwner::<T>::get(subnet_id);
        let pending_owner: T::AccountId = match PendingSubnetOwner::<T>::try_get(subnet_id) {
            Ok(pending_owner) => pending_owner,
            Err(()) => return Err(Error::<T>::NoPendingSubnetOwner.into()),
        };

        ensure!(coldkey == pending_owner, Error::<T>::NotPendingSubnetOwner);

        SubnetOwner::<T>::try_mutate_exists(subnet_id, |maybe_owner| -> DispatchResult {
            let owner = maybe_owner.as_mut().ok_or(Error::<T>::InvalidSubnetId)?;
            *owner = pending_owner;
            Ok(())
        })?;

        PendingSubnetOwner::<T>::remove(subnet_id);

        Self::deposit_event(Event::AcceptPendingSubnetOwner {
            subnet_id: subnet_id,
            new_owner: coldkey,
        });

        Ok(())
    }

    pub fn do_owner_add_bootnode_access(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        new_account: T::AccountId,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        SubnetBootnodeAccess::<T>::try_mutate(subnet_id, |access_list| -> DispatchResult {
            if !access_list.insert(new_account.clone()) {
                return Err(Error::<T>::InBootnodeAccessList.into());
            }
            ensure!(
                access_list.len() <= MaxSubnetBootnodeAccess::<T>::get() as usize,
                Error::<T>::MaxSubnetBootnodeAccess
            );
            Ok(())
        })?;

        Self::deposit_event(Event::AddSubnetBootnodeAccess {
            subnet_id: subnet_id,
            owner: coldkey,
            new_account,
        });

        Ok(())
    }

    pub fn do_owner_remove_bootnode_access(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        remove_account: T::AccountId,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        SubnetBootnodeAccess::<T>::try_mutate(subnet_id, |access_list| -> DispatchResult {
            if !access_list.remove(&remove_account) {
                return Err(Error::<T>::NotInAccessList.into());
            }
            Ok(())
        })?;

        Self::deposit_event(Event::RemoveSubnetBootnodeAccess {
            subnet_id: subnet_id,
            owner: coldkey,
            remove_account,
        });

        Ok(())
    }

    pub fn do_owner_update_target_node_registrations_per_epoch(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: u32,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            value <= MaxRegisteredNodes::<T>::get(subnet_id) && value > 0,
            Error::<T>::InvalidTargetNodeRegistrationsPerEpoch
        );

        TargetNodeRegistrationsPerEpoch::<T>::insert(subnet_id, value);

        Self::deposit_event(Event::TargetNodeRegistrationsPerEpochUpdate {
            subnet_id: subnet_id,
            owner: coldkey,
            value,
        });

        Ok(())
    }

    pub fn do_owner_update_node_burn_rate_alpha(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: u128,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        NodeBurnRateAlpha::<T>::insert(subnet_id, value);

        Self::deposit_event(Event::NodeBurnRateAlphaUpdate {
            subnet_id: subnet_id,
            owner: coldkey,
            value,
        });

        Ok(())
    }

    pub fn do_owner_update_queue_immunity_epochs(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: u32,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            value >= MinQueueEpochs::<T>::get() && value <= MaxQueueEpochs::<T>::get(),
            Error::<T>::InvalidQueueImmunityEpochs
        );

        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        Self::prepare_pending_owner_u32_update(
            current_subnet_epoch,
            PendingQueueImmunityEpochs::<T>::get(subnet_id),
            |value| QueueImmunityEpochs::<T>::insert(subnet_id, value),
        )?;
        let effective_subnet_epoch =
            Self::next_owner_parameter_effective_subnet_epoch(current_subnet_epoch);
        PendingQueueImmunityEpochs::<T>::insert(
            subnet_id,
            PendingOwnerU32Update {
                value,
                effective_subnet_epoch,
                owner: coldkey.clone(),
            },
        );

        Self::deposit_event(Event::QueueImmunityEpochsUpdateScheduled {
            subnet_id: subnet_id,
            owner: coldkey,
            value,
            effective_subnet_epoch,
        });

        Ok(())
    }

    pub fn do_owner_update_consensus_validator_node_count_decay(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: u128,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        let current_epoch = Self::get_current_epoch_as_u32();
        let update_interval = ConsensusValidatorNodeCountDecayUpdateInterval::<T>::get();

        if let Some(last_update) = LastConsensusValidatorNodeCountDecayUpdate::<T>::get(subnet_id) {
            ensure!(
                last_update.saturating_add(update_interval) <= current_epoch,
                Error::<T>::ConsensusValidatorNodeCountDecayUpdateTooSoon
            );
        }

        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        Self::prepare_pending_owner_u128_update(
            current_subnet_epoch,
            PendingConsensusValidatorNodeCountDecay::<T>::get(subnet_id),
            |value| ConsensusValidatorNodeCountDecay::<T>::insert(subnet_id, value),
        )?;
        let effective_subnet_epoch =
            Self::next_owner_parameter_effective_subnet_epoch(current_subnet_epoch);
        PendingConsensusValidatorNodeCountDecay::<T>::insert(
            subnet_id,
            PendingOwnerU128Update {
                value,
                effective_subnet_epoch,
                owner: coldkey.clone(),
            },
        );
        LastConsensusValidatorNodeCountDecayUpdate::<T>::insert(subnet_id, current_epoch);

        Self::deposit_event(Event::ConsensusValidatorNodeCountDecayUpdateScheduled {
            subnet_id,
            owner: coldkey,
            value,
            effective_subnet_epoch,
        });

        Ok(())
    }

    pub fn do_owner_update_consensus_validator_stake_weight_power(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: u128,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            value >= MinConsensusValidatorStakeWeightPower::<T>::get()
                && value <= MaxConsensusValidatorStakeWeightPower::<T>::get(),
            Error::<T>::InvalidPercent
        );

        let current_epoch = Self::get_current_epoch_as_u32();
        let update_interval = ConsensusValidatorStakeWeightPowerUpdateInterval::<T>::get();

        if let Some(last_update) = LastConsensusValidatorStakeWeightPowerUpdate::<T>::get(subnet_id)
        {
            ensure!(
                last_update.saturating_add(update_interval) <= current_epoch,
                Error::<T>::ConsensusValidatorStakeWeightPowerUpdateTooSoon
            );
        }

        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        Self::prepare_pending_owner_u128_update(
            current_subnet_epoch,
            PendingConsensusValidatorStakeWeightPower::<T>::get(subnet_id),
            |value| ConsensusValidatorStakeWeightPower::<T>::insert(subnet_id, value),
        )?;
        let effective_subnet_epoch =
            Self::next_owner_parameter_effective_subnet_epoch(current_subnet_epoch);
        PendingConsensusValidatorStakeWeightPower::<T>::insert(
            subnet_id,
            PendingOwnerU128Update {
                value,
                effective_subnet_epoch,
                owner: coldkey.clone(),
            },
        );
        LastConsensusValidatorStakeWeightPowerUpdate::<T>::insert(subnet_id, current_epoch);

        Self::deposit_event(Event::ConsensusValidatorStakeWeightPowerUpdateScheduled {
            subnet_id,
            owner: coldkey,
            value,
            effective_subnet_epoch,
        });

        Ok(())
    }

    pub fn do_owner_update_min_subnet_node_reputation(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: u128,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            value <= Self::percentage_factor_as_u128(),
            Error::<T>::InvalidPercent
        );

        ensure!(
            value >= MinMinSubnetNodeReputation::<T>::get()
                && value <= MaxMinSubnetNodeReputation::<T>::get(),
            Error::<T>::MinSubnetNodeReputation
        );

        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        Self::prepare_pending_owner_u128_update(
            current_subnet_epoch,
            PendingMinSubnetNodeReputation::<T>::get(subnet_id),
            |value| MinSubnetNodeReputation::<T>::insert(subnet_id, value),
        )?;
        let effective_subnet_epoch =
            Self::next_owner_parameter_effective_subnet_epoch(current_subnet_epoch);
        PendingMinSubnetNodeReputation::<T>::insert(
            subnet_id,
            PendingOwnerU128Update {
                value,
                effective_subnet_epoch,
                owner: coldkey.clone(),
            },
        );

        Self::deposit_event(Event::MinSubnetNodeReputationUpdateScheduled {
            subnet_id: subnet_id,
            owner: coldkey,
            value,
            effective_subnet_epoch,
        });

        Ok(())
    }

    pub fn do_owner_update_subnet_node_min_weight_decrease_reputation_threshold(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        value: u128,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            value <= MaxSubnetNodeMinWeightDecreaseReputationThreshold::<T>::get(),
            Error::<T>::InvalidPercent
        );

        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        Self::prepare_pending_owner_u128_update(
            current_subnet_epoch,
            PendingSubnetNodeMinWeightDecreaseReputationThreshold::<T>::get(subnet_id),
            |value| SubnetNodeMinWeightDecreaseReputationThreshold::<T>::insert(subnet_id, value),
        )?;
        let effective_subnet_epoch =
            Self::next_owner_parameter_effective_subnet_epoch(current_subnet_epoch);
        PendingSubnetNodeMinWeightDecreaseReputationThreshold::<T>::insert(
            subnet_id,
            PendingOwnerU128Update {
                value,
                effective_subnet_epoch,
                owner: coldkey.clone(),
            },
        );

        Self::deposit_event(
            Event::SubnetNodeMinWeightDecreaseReputationThresholdUpdateScheduled {
                subnet_id: subnet_id,
                owner: coldkey,
                value,
                effective_subnet_epoch,
            },
        );

        Ok(())
    }

    pub fn do_owner_update_reputation_factors(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        updates: SubnetReputationFactorUpdates,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(updates.has_update(), Error::<T>::InvalidValues);

        Self::validate_reputation_factor_update(
            updates.absent_decrease,
            Error::<T>::InvalidAbsentDecreaseReputationFactor,
        )?;
        Self::validate_reputation_factor_update(
            updates.included_increase,
            Error::<T>::InvalidIncludedIncreaseReputationFactor,
        )?;
        Self::validate_reputation_factor_update(
            updates.below_min_weight_decrease,
            Error::<T>::InvalidBelowMinWeightDecreaseReputationFactor,
        )?;
        Self::validate_reputation_factor_update(
            updates.non_attestor_decrease,
            Error::<T>::InvalidNonAttestorDecreaseReputationFactor,
        )?;
        Self::validate_reputation_factor_update(
            updates.non_consensus_attestor_decrease,
            Error::<T>::InvalidNonConsensusAttestorDecreaseReputationFactor,
        )?;
        Self::validate_reputation_factor_update(
            updates.validator_absent_decrease,
            Error::<T>::InvalidNonValidatorAbsentDecreaseReputationFactor,
        )?;
        Self::validate_reputation_factor_update(
            updates.validator_non_consensus_decrease,
            Error::<T>::InvalidValidatorNonConsensusSubnetNodeReputationFactor,
        )?;

        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        let mut schedule = SubnetReputationFactorSchedules::<T>::get(subnet_id);

        if let Some(pending) = schedule.pending {
            if pending.effective_subnet_epoch <= current_subnet_epoch {
                schedule.current = pending.factors;
                schedule.pending = None;
            }
        }

        let mut next_factors = schedule
            .pending
            .map(|pending| pending.factors)
            .unwrap_or(schedule.current);

        if let Some(value) = updates.absent_decrease {
            next_factors.absent_decrease = value;
        }
        if let Some(value) = updates.included_increase {
            next_factors.included_increase = value;
        }
        if let Some(value) = updates.below_min_weight_decrease {
            next_factors.below_min_weight_decrease = value;
        }
        if let Some(value) = updates.non_attestor_decrease {
            next_factors.non_attestor_decrease = value;
        }
        if let Some(value) = updates.non_consensus_attestor_decrease {
            next_factors.non_consensus_attestor_decrease = value;
        }
        if let Some(value) = updates.validator_absent_decrease {
            next_factors.validator_absent_decrease = value;
        }
        if let Some(value) = updates.validator_non_consensus_decrease {
            next_factors.validator_non_consensus_decrease = value;
        }

        let cooldown =
            SubnetOwnerFactorCooldownEpochs::<T>::get().max(MIN_OWNER_FACTOR_COOLDOWN_EPOCHS);
        let effective_subnet_epoch = current_subnet_epoch.saturating_add(cooldown);
        schedule.pending = Some(PendingSubnetReputationFactors {
            effective_subnet_epoch,
            factors: next_factors,
        });

        SubnetReputationFactorSchedules::<T>::insert(subnet_id, schedule);

        Self::deposit_event(Event::SubnetReputationFactorsUpdateScheduled {
            subnet_id: subnet_id,
            owner: coldkey,
            factors: next_factors,
            effective_subnet_epoch,
        });

        Ok(())
    }

    fn validate_reputation_factor_update(value: Option<u128>, error: Error<T>) -> DispatchResult {
        if let Some(value) = value {
            ensure!(
                value <= Self::percentage_factor_as_u128(),
                Error::<T>::InvalidPercent
            );

            ensure!(
                value >= MinNodeReputationFactor::<T>::get()
                    && value <= MaxNodeReputationFactor::<T>::get(),
                error
            );
        }

        Ok(())
    }
}
