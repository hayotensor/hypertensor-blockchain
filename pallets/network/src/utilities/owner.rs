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
use libm::{ceil, log};

impl<T: Config> Pallet<T> {
    pub const MAX_EMERGENCY_VALIDATOR_DURATION_STEPS: u32 = 10_000;

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

    pub fn get_min_consensus_node_attestation_percentage_for_epoch(
        subnet_id: u32,
        subnet_epoch: u32,
    ) -> u128 {
        Self::pending_owner_u128_value_for_epoch(
            SubnetMinConsensusNodeAttestationPercentage::<T>::get(subnet_id),
            PendingSubnetMinConsensusNodeAttestationPercentage::<T>::get(subnet_id),
            subnet_epoch,
        )
    }

    pub fn get_idle_classification_epochs_for_epoch(subnet_id: u32, subnet_epoch: u32) -> u32 {
        Self::pending_owner_u32_value_for_epoch(
            IdleClassificationEpochs::<T>::get(subnet_id),
            PendingIdleClassificationEpochs::<T>::get(subnet_id),
            subnet_epoch,
        )
    }

    pub fn get_included_classification_epochs_for_epoch(subnet_id: u32, subnet_epoch: u32) -> u32 {
        Self::pending_owner_u32_value_for_epoch(
            IncludedClassificationEpochs::<T>::get(subnet_id),
            PendingIncludedClassificationEpochs::<T>::get(subnet_id),
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

    /// Owner pause subnet for up to max period
    ///
    /// This will pause the following logic on the next subnet epoch start block step:
    /// - Elect validator
    /// - Activate nodes from the queue
    /// - Update the node burn rate
    ///
    /// If a validator is currently elected, it will not stop consensus from proceeding
    /// in the current epoch.
    pub fn do_owner_pause_subnet(origin: T::RuntimeOrigin, subnet_id: u32) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            Self::is_subnet_active(subnet_id).unwrap_or(false),
            Error::<T>::SubnetMustBeActive
        );

        let epoch = Self::get_current_epoch_as_u32();

        // Ensure subnet pause period has been reached to pause again
        ensure!(
            PreviousSubnetPauseEpoch::<T>::get(subnet_id)
                .saturating_add(SubnetPauseCooldownEpochs::<T>::get())
                <= epoch,
            Error::<T>::SubnetPauseCooldownActive
        );

        SubnetsData::<T>::try_mutate_exists(subnet_id, |maybe_params| -> DispatchResult {
            let params = maybe_params.as_mut().ok_or(Error::<T>::InvalidSubnetId)?;

            // Update state
            params.state = SubnetState::Paused;

            // We use the current epoch as the `start_epoch` when pausing
            // This enables us to know the delta when reactivating for updating the node registration pool node start epochs
            // see `do_owner_unpause_subnet`
            params.start_epoch = epoch;

            Ok(())
        })?;

        // ---
        // We don't need to remove SubnetConsensusSubmission here because
        // precheck_subnet_consensus_submission already checks if the subnet is active and not paused
        // ---

        Self::deposit_event(Event::SubnetPaused {
            subnet_id: subnet_id,
            owner: coldkey,
        });

        Ok(())
    }

    pub fn do_owner_unpause_subnet(origin: T::RuntimeOrigin, subnet_id: u32) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            Self::is_subnet_paused(subnet_id).unwrap_or(false),
            Error::<T>::SubnetMustBePaused
        );

        let epoch = Self::get_current_epoch_as_u32();
        let subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);

        Self::maybe_finish_expired_emergency_validator_set(subnet_id, subnet_epoch);

        if let Some(data) = EmergencySubnetNodeElectionData::<T>::get(subnet_id) {
            if !data.activated {
                let validated_ids = Self::validate_emergency_validator_ids(
                    subnet_id,
                    subnet_epoch,
                    data.subnet_node_ids.clone(),
                )?;
                ensure!(
                    validated_ids == data.subnet_node_ids,
                    Error::<T>::InvalidEmergencySubnetNodeId
                );
            }
        }

        // If the subnet is passed the max pause epochs, validators via on_initialize already
        // unpaused it. If not, we allow the owner to unpause

        // A subnet can only pause if it's active, so we re-activate it back in the Active state
        SubnetsData::<T>::try_mutate_exists(subnet_id, |maybe_params| -> DispatchResult {
            let params = maybe_params.as_mut().ok_or(Error::<T>::InvalidSubnetId)?;

            let pause_epoch = params.start_epoch;

            // Epochs the subnet was paused for
            let delta = epoch.saturating_sub(pause_epoch).saturating_add(1); // Add +1 to offset the subnet slots

            // Update each registration queued node
            // Move each nodes start_epoch forward by the amount of epochs the subnet was paused
            for (uid, _) in RegisteredSubnetNodesData::<T>::iter_prefix(subnet_id) {
                RegisteredSubnetNodesData::<T>::mutate(subnet_id, uid, |subnet_node| {
                    let curr_start_epoch = subnet_node.classification.start_epoch;
                    subnet_node.classification.start_epoch = curr_start_epoch.saturating_add(delta);
                });
            }

            // Update state
            params.state = SubnetState::Active;

            // We start them on the next epoch following the current epoch
            // This protects the network against an owner pausing a subnet and then unpausing it in a single epoch to manipulate
            // the attestation ratios (see ``precheck_subnet_consensus_submission`` `max_attestors`)
            params.start_epoch = epoch.saturating_add(1);

            Ok(())
        })?;

        PreviousSubnetPauseEpoch::<T>::insert(subnet_id, epoch);

        // Activate a pending emergency validator set. Active emergency data is intentionally
        // not reset on later pause/unpause cycles.
        EmergencySubnetNodeElectionData::<T>::mutate_exists(subnet_id, |maybe_data| {
            if let Some(data) = maybe_data {
                if !data.activated {
                    let max_emergency_delta = Self::percent_mul(
                        data.target_emergency_validators_epochs as u128,
                        MaxEmergencyValidatorEpochsMultiplier::<T>::get(),
                    )
                    .min(u32::MAX as u128) as u32;
                    data.activated = true;
                    data.started_subnet_epoch = subnet_epoch;
                    data.max_emergency_validators_epoch =
                        subnet_epoch.saturating_add(max_emergency_delta);
                }
            }
        });

        Self::deposit_event(Event::SubnetUnpaused {
            subnet_id: subnet_id,
            owner: coldkey,
        });

        Ok(())
    }

    pub fn do_owner_set_emergency_validator_set(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        subnet_node_ids: Vec<u32>,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        ensure!(
            Self::is_subnet_paused(subnet_id).unwrap_or(false),
            Error::<T>::SubnetMustBePaused
        );

        let subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        Self::maybe_finish_expired_emergency_validator_set(subnet_id, subnet_epoch);

        if let Some(data) = EmergencySubnetNodeElectionData::<T>::get(subnet_id) {
            ensure!(!data.activated, Error::<T>::EmergencyValidatorsActive);
        }

        Self::ensure_emergency_validator_cooldown_complete(subnet_id)?;

        let subnet_node_ids =
            Self::validate_emergency_validator_ids(subnet_id, subnet_epoch, subnet_node_ids)?;

        let reputation_factors = Self::get_reputation_factors_for_epoch(subnet_id, subnet_epoch);
        let min_subnet_node_reputation =
            Self::get_min_subnet_node_reputation_for_epoch(subnet_id, subnet_epoch);

        let target_emergency_epochs = Self::get_emergency_validator_duration_epochs(
            reputation_factors.absent_decrease,
            min_subnet_node_reputation,
        )?;

        // Insert emergency subnet validator data
        EmergencySubnetNodeElectionData::<T>::insert(
            subnet_id,
            EmergencySubnetValidatorData {
                subnet_node_ids: subnet_node_ids.clone(),
                target_emergency_validators_epochs: target_emergency_epochs,
                total_epochs: 0,
                max_emergency_validators_epoch: 0,
                activated: false,
                started_subnet_epoch: 0,
                reputation_factors,
                min_subnet_node_reputation,
                min_weight_decrease_reputation_threshold:
                    Self::get_subnet_node_min_weight_decrease_reputation_threshold_for_epoch(
                        subnet_id,
                        subnet_epoch,
                    ),
            },
        );

        Self::deposit_event(Event::SubnetForked {
            subnet_id: subnet_id,
            owner: coldkey,
            subnet_node_ids,
        });

        Ok(())
    }

    pub fn do_owner_set_emergency_validator_set_v2(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
        subnet_node_ids: Vec<u32>,
    ) -> DispatchResult {
        Self::do_owner_set_emergency_validator_set(origin, subnet_id, subnet_node_ids)
    }

    /// Calculate emergency validator duration from the absent decrease factor.
    ///
    /// The duration is conservative: every removed validator is assumed to start at 100%
    /// reputation, so current per-node reputation does not affect dispatch cost.
    pub fn get_emergency_validator_duration_epochs(
        absent_decrease_factor: u128,
        min_reputation: u128,
    ) -> Result<u32, Error<T>> {
        let one = Self::percentage_factor_as_u128();

        ensure!(
            absent_decrease_factor > 0 && min_reputation > 0,
            Error::<T>::InvalidEmergencyValidatorDuration
        );

        if absent_decrease_factor >= one || min_reputation >= one {
            return Ok(1u32.saturating_add(1));
        }

        let retained_ratio = (one.saturating_sub(absent_decrease_factor)) as f64 / one as f64;
        let threshold_ratio = min_reputation as f64 / one as f64;

        let steps = ceil(log(threshold_ratio) / log(retained_ratio));
        ensure!(
            steps.is_finite()
                && steps >= 1.0
                && steps <= Self::MAX_EMERGENCY_VALIDATOR_DURATION_STEPS as f64,
            Error::<T>::InvalidEmergencyValidatorDuration
        );

        Ok((steps as u32).saturating_add(1))
    }

    fn validate_emergency_validator_ids(
        subnet_id: u32,
        subnet_epoch: u32,
        mut subnet_node_ids: Vec<u32>,
    ) -> Result<Vec<u32>, Error<T>> {
        ensure!(
            subnet_node_ids.len() as u32 <= MaxEmergencySubnetNodes::<T>::get(),
            Error::<T>::InvalidMaxEmergencySubnetNodes
        );

        for subnet_node_id in subnet_node_ids.iter() {
            let subnet_node = SubnetNodesData::<T>::try_get(subnet_id, subnet_node_id)
                .map_err(|_| Error::<T>::InvalidEmergencySubnetNodeId)?;

            ensure!(
                subnet_node.has_classification(&SubnetNodeClass::Validator, subnet_epoch),
                Error::<T>::InvalidEmergencySubnetNodeId
            );
        }

        subnet_node_ids.sort_unstable();
        subnet_node_ids.dedup();

        ensure!(
            subnet_node_ids.len() as u32 >= MinSubnetNodes::<T>::get(),
            Error::<T>::InvalidMinEmergencySubnetNodes
        );

        ensure!(
            subnet_node_ids.len() as u32 <= MaxEmergencySubnetNodes::<T>::get(),
            Error::<T>::InvalidMaxEmergencySubnetNodes
        );

        Ok(subnet_node_ids)
    }

    pub fn ensure_emergency_validator_cooldown_complete(subnet_id: u32) -> DispatchResult {
        let last_end_epoch = LastEmergencyValidatorEndEpoch::<T>::get(subnet_id);
        if last_end_epoch == 0 {
            return Ok(());
        }

        let current_epoch = Self::get_current_epoch_as_u32();
        ensure!(
            last_end_epoch.saturating_add(EmergencyValidatorCooldownEpochs::<T>::get())
                <= current_epoch,
            Error::<T>::EmergencyValidatorCooldownActive
        );

        Ok(())
    }

    pub fn is_emergency_validator_set_active(subnet_id: u32) -> bool {
        EmergencySubnetNodeElectionData::<T>::get(subnet_id)
            .map(|data| data.activated)
            .unwrap_or(false)
    }

    pub fn active_emergency_validator_ids(
        data: &EmergencySubnetValidatorData,
        subnet_id: u32,
        subnet_epoch: u32,
    ) -> Vec<u32> {
        data.subnet_node_ids
            .iter()
            .filter_map(|subnet_node_id| {
                SubnetNodesData::<T>::try_get(subnet_id, subnet_node_id)
                    .ok()
                    .filter(|subnet_node| {
                        subnet_node.has_classification(&SubnetNodeClass::Validator, subnet_epoch)
                    })
                    .map(|_| *subnet_node_id)
            })
            .collect()
    }

    pub fn is_emergency_validator_set_expired(
        data: &EmergencySubnetValidatorData,
        subnet_id: u32,
        subnet_epoch: u32,
    ) -> bool {
        data.activated
            && (data.total_epochs >= data.target_emergency_validators_epochs
                || subnet_epoch > data.max_emergency_validators_epoch
                || (Self::active_emergency_validator_ids(data, subnet_id, subnet_epoch).len()
                    as u32)
                    < MinSubnetNodes::<T>::get())
    }

    pub fn maybe_finish_expired_emergency_validator_set(subnet_id: u32, subnet_epoch: u32) -> bool {
        if let Some(data) = EmergencySubnetNodeElectionData::<T>::get(subnet_id) {
            if Self::is_emergency_validator_set_expired(&data, subnet_id, subnet_epoch) {
                Self::finish_emergency_validator_set(subnet_id);
                return true;
            }
        }

        false
    }

    pub fn emergency_consensus_snapshot(
        data: &EmergencySubnetValidatorData,
    ) -> EmergencyConsensusSnapshot {
        EmergencyConsensusSnapshot {
            subnet_node_ids: data.subnet_node_ids.clone(),
            reputation_factors: data.reputation_factors,
            min_subnet_node_reputation: data.min_subnet_node_reputation,
            min_weight_decrease_reputation_threshold: data.min_weight_decrease_reputation_threshold,
        }
    }

    pub fn finish_emergency_validator_set(subnet_id: u32) {
        if EmergencySubnetNodeElectionData::<T>::take(subnet_id).is_some() {
            LastEmergencyValidatorEndEpoch::<T>::insert(
                subnet_id,
                Self::get_current_epoch_as_u32(),
            );
            Self::deposit_event(Event::EmergencyValidatorSetExpired { subnet_id });
        }
    }

    /// Owner can remove the emergency validator set at any time
    ///
    /// # Arguments
    /// * `origin` - The origin of the transaction
    /// * `subnet_id` - The id of the subnet
    ///
    /// # Returns
    /// * `DispatchResult` - The result of the transaction
    pub fn do_owner_revert_emergency_validator_set(
        origin: T::RuntimeOrigin,
        subnet_id: u32,
    ) -> DispatchResult {
        let coldkey: T::AccountId = ensure_signed(origin)?;

        ensure!(
            Self::is_subnet_owner(&coldkey, subnet_id).unwrap_or(false),
            Error::<T>::NotSubnetOwner
        );

        // Subnet must be paused to revert emergency validator set
        // This ensures owner cannot manipulate the attestation ratio
        ensure!(
            Self::is_subnet_paused(subnet_id).unwrap_or(false),
            Error::<T>::SubnetMustBePaused
        );

        if let Some(data) = EmergencySubnetNodeElectionData::<T>::take(subnet_id) {
            if data.activated {
                LastEmergencyValidatorEndEpoch::<T>::insert(
                    subnet_id,
                    Self::get_current_epoch_as_u32(),
                );
            }
        }

        Self::deposit_event(Event::SubnetForkRevert {
            subnet_id: subnet_id,
            owner: coldkey,
        });

        Ok(())
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
            value >= MinChurnLimitMultiplier::<T>::get()
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

        SubnetNodeQueueEpochs::<T>::insert(subnet_id, value);

        Self::deposit_event(Event::RegistrationQueueEpochsUpdate {
            subnet_id: subnet_id,
            owner: coldkey,
            value: value,
        });

        Ok(())
    }

    pub fn do_owner_update_idle_classification_epochs(
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
            value >= MinIdleClassificationEpochs::<T>::get()
                && value <= MaxIdleClassificationEpochs::<T>::get(),
            Error::<T>::InvalidIdleClassificationEpochs
        );

        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        Self::prepare_pending_owner_u32_update(
            current_subnet_epoch,
            PendingIdleClassificationEpochs::<T>::get(subnet_id),
            |value| IdleClassificationEpochs::<T>::insert(subnet_id, value),
        )?;
        let effective_subnet_epoch = current_subnet_epoch.saturating_add(1);
        PendingIdleClassificationEpochs::<T>::insert(
            subnet_id,
            PendingOwnerU32Update {
                value,
                effective_subnet_epoch,
                owner: coldkey.clone(),
            },
        );

        Self::deposit_event(Event::IdleClassificationEpochsUpdateScheduled {
            subnet_id: subnet_id,
            owner: coldkey,
            value: value,
            effective_subnet_epoch,
        });

        Ok(())
    }

    pub fn do_owner_update_included_classification_epochs(
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
            value >= MinIncludedClassificationEpochs::<T>::get()
                && value <= MaxIncludedClassificationEpochs::<T>::get(),
            Error::<T>::InvalidIncludedClassificationEpochs
        );

        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        Self::prepare_pending_owner_u32_update(
            current_subnet_epoch,
            PendingIncludedClassificationEpochs::<T>::get(subnet_id),
            |value| IncludedClassificationEpochs::<T>::insert(subnet_id, value),
        )?;
        let effective_subnet_epoch = current_subnet_epoch.saturating_add(1);
        PendingIncludedClassificationEpochs::<T>::insert(
            subnet_id,
            PendingOwnerU32Update {
                value,
                effective_subnet_epoch,
                owner: coldkey.clone(),
            },
        );

        Self::deposit_event(Event::IncludedClassificationEpochsUpdateScheduled {
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
            validators.values().all(|&value| value >= 1),
            Error::<T>::InvalidSubnetRegistrationInitialColdkeys
        );

        NodeRegistrationInitialValidatorIds::<T>::mutate(subnet_id, |maybe_validators| {
            let validators_set = maybe_validators.get_or_insert_with(BTreeMap::new);
            validators_set.extend(
                validators
                    .iter()
                    .map(|(&validator_id, &max_registrations)| (validator_id, max_registrations)),
            );
        });

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
        let effective_subnet_epoch = current_subnet_epoch.saturating_add(1);

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

    /// Update maximum registered nodes
    ///
    /// This function can only be called by the current owner of the subnet.  
    ///
    /// # Parameters
    /// - `origin`: The caller, must be the current subnet owner.
    /// - `subnet_id`: The ID of the subnet.
    /// - `value`: The new number maximum registered nodes.
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
        let effective_subnet_epoch = current_subnet_epoch.saturating_add(1);
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

        ConsensusValidatorNodeCountDecay::<T>::insert(subnet_id, value);
        LastConsensusValidatorNodeCountDecayUpdate::<T>::insert(subnet_id, current_epoch);

        Self::deposit_event(Event::ConsensusValidatorNodeCountDecayUpdate {
            subnet_id,
            owner: coldkey,
            value,
        });

        Ok(())
    }

    pub fn do_owner_update_min_consensus_node_attestation_percentage(
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
            value >= MinSubnetConsensusNodeAttestationPercentage::<T>::get()
                && value <= MaxSubnetConsensusNodeAttestationPercentage::<T>::get(),
            Error::<T>::InvalidPercent
        );

        let current_subnet_epoch = Self::get_current_subnet_epoch_as_u32(subnet_id);
        Self::prepare_pending_owner_u128_update(
            current_subnet_epoch,
            PendingSubnetMinConsensusNodeAttestationPercentage::<T>::get(subnet_id),
            |value| SubnetMinConsensusNodeAttestationPercentage::<T>::insert(subnet_id, value),
        )?;
        let effective_subnet_epoch = current_subnet_epoch.saturating_add(1);
        PendingSubnetMinConsensusNodeAttestationPercentage::<T>::insert(
            subnet_id,
            PendingOwnerU128Update {
                value,
                effective_subnet_epoch,
                owner: coldkey.clone(),
            },
        );

        Self::deposit_event(
            Event::MinConsensusNodeAttestationPercentageUpdateScheduled {
                subnet_id,
                owner: coldkey,
                value,
                effective_subnet_epoch,
            },
        );

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

        Self::maybe_finish_expired_emergency_validator_set(
            subnet_id,
            Self::get_current_subnet_epoch_as_u32(subnet_id),
        );

        ensure!(
            !Self::is_emergency_validator_set_active(subnet_id),
            Error::<T>::EmergencyValidatorsSet
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
        let effective_subnet_epoch = current_subnet_epoch.saturating_add(1);
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

        Self::maybe_finish_expired_emergency_validator_set(
            subnet_id,
            Self::get_current_subnet_epoch_as_u32(subnet_id),
        );

        ensure!(
            !Self::is_emergency_validator_set_active(subnet_id),
            Error::<T>::EmergencyValidatorsSet
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
        let effective_subnet_epoch = current_subnet_epoch.saturating_add(1);
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

        if updates.requires_no_emergency_validators() {
            Self::maybe_finish_expired_emergency_validator_set(
                subnet_id,
                Self::get_current_subnet_epoch_as_u32(subnet_id),
            );

            ensure!(
                !Self::is_emergency_validator_set_active(subnet_id),
                Error::<T>::EmergencyValidatorsSet
            );
        }

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

        let cooldown = SubnetOwnerFactorCooldownEpochs::<T>::get().max(1);
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
