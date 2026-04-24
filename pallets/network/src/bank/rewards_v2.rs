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
use frame_support::pallet_prelude::{DispatchError, Weight};

impl<T: Config> Pallet<T> {
    pub fn distribute_rewards_v2(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        block: u32,
        current_epoch: u32,
        current_subnet_epoch: u32,
        consensus_submission_data: ConsensusSubmissionDataV2,
        rewards_data: RewardsData,
        min_attestation_percentage: u128,
        coldkey_reputation_increase_factor: u128,
        coldkey_reputation_decrease_factor: u128,
        super_majority_threshold: u128,
    ) {
        let db_weight = T::DbWeight::get();

        let percentage_factor = Self::percentage_factor_as_u128();
        let min_validator_reputation = MinSubnetNodeReputation::<T>::get(subnet_id);
        let subnet_reputation = SubnetReputation::<T>::get(subnet_id);
        // MinSubnetNodeReputation | SubnetReputation
        weight_meter.consume(db_weight.reads(2));

        // We run this here because any epoch where a validator submits data, whether in consensus
        // or not, we increment the forks `total_epochs`
        let forked_subnet_node_ids: Option<BTreeSet<u32>> =
            Self::maybe_get_forked_subnet_node_ids_v2(weight_meter, subnet_id);

        let electable_nodes_count = SubnetNodeElectionSlots::<T>::get(subnet_id).len() as u32;
        weight_meter.consume(db_weight.reads(1));

        // --- If under minimum attestation ratio, penalize validator, skip rewards
        if consensus_submission_data.attestation_ratio < min_attestation_percentage {
            Self::handle_non_consensus_v2(
                subnet_id,
                consensus_submission_data,
                min_attestation_percentage,
                coldkey_reputation_decrease_factor,
                min_validator_reputation,
                electable_nodes_count,
                current_epoch,
                subnet_reputation,
                percentage_factor,
                weight_meter,
            );
            return;
        } else if let Some(validator_id) = SubnetNodeValidatorId::<T>::get(
            subnet_id,
            consensus_submission_data.validator_subnet_node_id,
        ) {
            //
            // In consensus: Increase validators stake
            //

            // In case the hotkey was updated since the consensus data was submitted we
            // get the current hotkey for the subnet node id (elected validator) above.

            Self::handle_validator_reward_v2(
                weight_meter,
                validator_id,
                subnet_id,
                consensus_submission_data.validator_subnet_node_id,
                &consensus_submission_data,
                min_attestation_percentage,
                coldkey_reputation_increase_factor,
                current_epoch,
            );
        } else {
            // Validator left subnet before distribution of rewards (not possible but
            // this logic stays here in case of future updates to allowing validators to exit
            // on the epoch they're elected for)

            // We read `SubnetNodeIdHotkey` (else if) if we got to this point
            weight_meter.consume(db_weight.reads(1));
        }

        //
        // --- We are now in consensus (>=66% attestation ratio)
        //

        let idle_epochs = IdleClassificationEpochs::<T>::get(subnet_id);
        let included_epochs = IncludedClassificationEpochs::<T>::get(subnet_id);
        let weight_threshold = SubnetNodeMinWeightDecreaseReputationThreshold::<T>::get(subnet_id);
        let absent_factor = AbsentDecreaseReputationFactor::<T>::get(subnet_id);
        let included_factor = IncludedIncreaseReputationFactor::<T>::get(subnet_id);
        let min_weight_factor = BelowMinWeightDecreaseReputationFactor::<T>::get(subnet_id);
        let non_attestor_factor = NonAttestorDecreaseReputationFactor::<T>::get(subnet_id);
        weight_meter.consume(db_weight.reads(7));

        // Super majority, update queue to prioritize node ID that subnet form a consensus to cut the line
        // and or update queue to remove a node ID the subnet forms a consensus to be removed (if passed immunity period)
        Self::handle_node_queue_consensus_v2(
            weight_meter,
            subnet_id,
            &consensus_submission_data,
            super_majority_threshold,
            electable_nodes_count,
        );

        // MinSubnetNodes
        weight_meter.consume(db_weight.reads(1));

        // Increase reputation because subnet consensus is in consensus
        // Only increase if subnet has >= min subnet nodes
        if subnet_reputation != percentage_factor
            && consensus_submission_data.data_length >= MinSubnetNodes::<T>::get()
        {
            Self::increase_subnet_reputation(
                subnet_id,
                InConsensusSubnetReputationFactor::<T>::get(),
                consensus_submission_data.attestation_ratio,
            );
            weight_meter.consume(db_weight.reads_writes(2, 1));
        }

        // --- Check if we should hold off rewards and increase the capacitor vault
        // If weight_sum is 0, and in consensus, this means the subnet agrees to hold off rewards
        // for now, so we increase the rewards capacitor
        if consensus_submission_data.weight_sum == 0 {
            // We increase the rewards capacitor
            RewardsCapacitor::<T>::mutate(subnet_id, |total| {
                *total = total.saturating_add(rewards_data.overall_subnet_reward)
            });
            weight_meter.consume(db_weight.reads_writes(1, 1));

            // Return before any rewards are distributed
            // The only node that gets rewards when weight_sum is 0 is the validator
            // But we already handled the validator reward above
            return;
        }

        // If we proceed to distribute rewards, reset the capacitor to 0
        RewardsCapacitor::<T>::insert(subnet_id, 0);
        weight_meter.consume(db_weight.writes(1));

        // --- Reward owner
        Self::handle_subnet_owner_reward_v2(
            weight_meter,
            subnet_id,
            rewards_data.subnet_owner_reward,
        );

        // Loop iteration overhead
        weight_meter.consume(Weight::from_parts(
            1_000 * consensus_submission_data.subnet_nodes.len() as u64,
            0,
        ));

        // --- Events variables

        // Node -> reward
        let mut node_rewards: Vec<(u32, u128)> = Vec::new();
        // Node -> delegate stake reward
        let mut validator_delegate_stake_rewards: Vec<(u32, u128)> = Vec::new();
        // Node -> (account -> amount)
        let mut node_delegate_account_allocations: Vec<(u32, (T::AccountId, u128))> = Vec::new();

        // Iterate each node, emit rewards, graduate, or penalize
        for subnet_node in &consensus_submission_data.subnet_nodes {
            // We need to check if the node exists, since we need to get `SubnetNodeReputation`, we will use
            // that to check the node is still active and has not been removed.
            // Note: `SubnetNodeReputation` is removed when a node is removed
            //
            // We check this to enable the node receives rewards, if eligible, but skip all removal and
            // reputation logic.
            let (mut reputation, node_exists): (u128, bool) =
                match SubnetNodeReputation::<T>::try_get(subnet_id, subnet_node.id) {
                    Ok(r) => (r, true),
                    Err(_) => (0, false),
                };

            // SubnetNodeReputation
            weight_meter.consume(db_weight.reads(1));

            if node_exists && reputation < min_validator_reputation {
                // Remove node if they haven't already been removed
                Self::handle_consensus_remove_active_node_v2(
                    weight_meter,
                    subnet_id,
                    subnet_node.id,
                    electable_nodes_count,
                );

                continue;
            }

            // If node is Idle class and subnet is not temporarily forked via temp validator set,
            // upgrade to Included class
            if node_exists
                && subnet_node.classification.node_class == SubnetNodeClass::Idle
                && forked_subnet_node_ids.is_none()
            {
                Self::handle_idle_node_v2(
                    weight_meter,
                    subnet_id,
                    subnet_node.id,
                    idle_epochs,
                    current_subnet_epoch,
                );

                continue;
            }

            //
            // All nodes are at least SubnetNodeClass::Included from here
            //

            let subnet_node_data_find = consensus_submission_data
                .data
                .iter()
                .find(|data| data.subnet_node_id == subnet_node.id);

            // Handle case where node is found in consensus data
            let subnet_node_data = if let Some(data) = subnet_node_data_find {
                // --- Is in consensus data, increase reputation if not at max
                if node_exists && reputation != percentage_factor {
                    // If the validator submits themselves in the data and passes consensus, this also
                    // increases the validators reputation
                    reputation = Self::increase_and_return_node_reputation(
                        subnet_id,
                        subnet_node.id,
                        reputation,
                        included_factor,
                        None,
                    );

                    // `increase_and_return_node_reputation`: SubnetNodeReputation (w)
                    weight_meter.consume(db_weight.writes(1));
                }
                data
            } else {
                if node_exists {
                    // Not included in consensus, decrease reputation
                    reputation = Self::decrease_and_return_node_reputation(
                        subnet_id,
                        subnet_node.id,
                        reputation,
                        absent_factor,
                        None,
                    );
                    // `decrease_and_return_node_reputation`: SubnetNodeReputation (w)
                    weight_meter.consume(db_weight.writes(1));

                    // Break count of consecutive epochs of being included in in-consensus data
                    if subnet_node.classification.node_class == SubnetNodeClass::Included {
                        SubnetNodeConsecutiveIncludedEpochs::<T>::insert(
                            subnet_id,
                            subnet_node.id,
                            0,
                        );
                        // SubnetNodeConsecutiveIncludedEpochs
                        weight_meter.consume(db_weight.writes(1));
                    }
                }

                // Not in consensus data, skip to next node
                continue;
            };

            // If node is Included class and subnet is not temporarily forked, upgrade to Validator class
            //
            // This is ran after we check if the node is included in the consensus data to ensure the node
            // gets its reputation decreased if it was not included in the consensus data
            if node_exists
                && subnet_node.classification.node_class == SubnetNodeClass::Included
                && forked_subnet_node_ids.is_none()
            {
                Self::handle_included_node_v2(
                    weight_meter,
                    subnet_id,
                    subnet_node.id,
                    reputation,
                    percentage_factor,
                    included_epochs,
                    current_subnet_epoch,
                );

                // SubnetNodeClass::Included does not get rewards yet, they must pass the gauntlet
                continue;
            }

            //
            // --- Consensus formed on node
            //

            let node_score = subnet_node_data.score;

            // We don't `continue` here because we want to calculate the weight percentage of the
            // node and possibly slash reputation if below the weight threshold

            // --- Calculate node weight percentage of peer versus the weighted sum
            let node_weight: u128 =
                Self::percent_div(node_score, consensus_submission_data.weight_sum);

            // * Optional logic:
            // Decrease reputation if under subnets weight threshold
            // We don't automatically decrease reputation if a node is at ZERO
            // This is an optional feature for subnets
            if node_exists && node_weight < weight_threshold {
                reputation = Self::decrease_and_return_node_reputation(
                    subnet_id,
                    subnet_node.id,
                    reputation,
                    min_weight_factor,
                    None,
                );
                // `decrease_and_return_node_reputation`: SubnetNodeReputation (w)
                weight_meter.consume(db_weight.writes(1));
            }

            //
            // All nodes are at least SubnetNodeClass::Validator from here and in consensus data
            //

            // Get the nodes reward factor
            let reward_factor = if let Some(forked_node_ids) = &forked_subnet_node_ids {
                if forked_node_ids.get(&subnet_node.id).is_some() {
                    // If one of the temporary fork nodes
                    match consensus_submission_data.attests.get(&subnet_node.id) {
                        Some(data) => data.reward_factor,
                        None => {
                            // If node didn't attest in super majority, decrease reputation
                            // We can likely assume the validator is offline in the current epoch and
                            // failed to attest. The `non_attestor_factor` is suggested to the be lowest
                            // decreasing factor of all node reputation factors.
                            if node_exists
                                && consensus_submission_data.attestation_ratio
                                    >= super_majority_threshold
                            {
                                reputation = Self::decrease_and_return_node_reputation(
                                    subnet_id,
                                    subnet_node.id,
                                    reputation,
                                    non_attestor_factor,
                                    None,
                                );
                                // `decrease_and_return_node_reputation`: SubnetNodeReputation (w)
                                weight_meter.consume(db_weight.writes(1));
                            }
                            percentage_factor
                        }
                    }
                } else {
                    percentage_factor
                }
            } else if let Some(data) = consensus_submission_data.attests.get(&subnet_node.id) {
                // Subnet is not forked and node attested
                data.reward_factor
            } else {
                // Node not attested but in in-consensus data, decrease reputation, return 1.0 reward factor
                if node_exists
                    && consensus_submission_data.attestation_ratio >= super_majority_threshold
                {
                    reputation = Self::decrease_and_return_node_reputation(
                        subnet_id,
                        subnet_node.id,
                        reputation,
                        non_attestor_factor,
                        None,
                    );
                    // `decrease_and_return_node_reputation`: SubnetNodeReputation (w)

                    weight_meter.consume(db_weight.writes(1));
                }

                percentage_factor
            };

            if node_exists && reputation < min_validator_reputation {
                // Remove node if they haven't already due to reputation decreases logic above
                Self::handle_consensus_remove_active_node_v2(
                    weight_meter,
                    subnet_id,
                    subnet_node.id,
                    electable_nodes_count,
                );

                continue;
            }

            // Reward factor is zero, no need to continue
            if reward_factor == 0 {
                continue;
            }

            // Skip and do *not* penalize if node weight is 0
            if node_weight == 0 {
                continue;
            }

            // --- Calculate node_score percentage of total subnet generated epoch rewards
            let mut account_reward: u128 =
                Self::percent_mul(node_weight, rewards_data.subnet_node_rewards);

            account_reward = Self::percent_mul(account_reward, reward_factor);

            // --- Skip if no rewards to give
            if account_reward == 0 {
                continue;
            }

            // We allow the node to not exist here and still increase the delegate reward pool
            // --- Increase delegate account balance and emit event
            if let Ok(validator_data) = &ValidatorsData::<T>::try_get(subnet_node.validator_id) {
                if validator_data.delegate_reward_rate != 0 {
                    if let Some((updated_account_reward, (subnet_node_id, node_delegate_reward))) =
                        Self::handle_validator_delegate_stake_v2(
                            weight_meter,
                            subnet_node.validator_id,
                            validator_data.delegate_reward_rate,
                            account_reward,
                        )
                    {
                        // Update account reward with the substracted amount that was given to the delegates
                        account_reward = updated_account_reward;
                        // Add the node delegate reward to the list for event
                        validator_delegate_stake_rewards
                            .push((subnet_node.validator_id, node_delegate_reward));
                    }
                }

                if let Some(delegate_account) = &validator_data.delegate_account {
                    // We don't check if the rate is > 0 because the rate can't
                    // be set to 0.
                    let (updated_account_reward, delegate_account_deposit) =
                        Self::handle_delegate_account_v2(
                            weight_meter,
                            account_reward,
                            &delegate_account.account_id,
                            delegate_account.rate,
                        );
                    account_reward = updated_account_reward;

                    node_delegate_account_allocations.push((
                        subnet_node.id,
                        (
                            delegate_account.account_id.clone(),
                            delegate_account_deposit,
                        ),
                    ));
                }
            }

            Self::increase_node_stake(subnet_node.id, subnet_id, account_reward);
            // AccountSubnetStake | TotalSubnetStake | TotalStake
            weight_meter.consume(db_weight.reads_writes(3, 3));

            node_rewards.push((subnet_node.id, account_reward));
        }

        // --- Increase the delegate stake pool balance
        if rewards_data.delegate_stake_rewards != 0 {
            Self::do_increase_delegate_stake(subnet_id, rewards_data.delegate_stake_rewards);
            // reads::
            // TotalSubnetDelegateStakeShares | TotalSubnetDelegateStakeBalance | TotalDelegateStake
            //
            // writes::
            // TotalSubnetDelegateStakeBalance | | TotalSubnetDelegateStakeShares|
            // TotalSubnetDelegateStakeShares| TotalSubnetDelegateStakeBalance| TotalDelegateStake
            weight_meter.consume(db_weight.reads_writes(3, 5));
        }

        Self::deposit_event(Event::SubnetRewards {
            subnet_id,
            node_rewards,
            delegate_stake_reward: rewards_data.delegate_stake_rewards,
            node_delegate_stake_rewards: validator_delegate_stake_rewards,
            node_delegate_account_allocations,
        });
    }

    /// Subnet is not in consensus
    pub fn handle_non_consensus_v2(
        subnet_id: u32,
        consensus_submission_data: ConsensusSubmissionDataV2,
        min_attestation_percentage: u128,
        coldkey_reputation_decrease_factor: u128,
        min_validator_reputation: u128,
        electable_nodes_count: u32,
        current_epoch: u32,
        subnet_reputation: u128,
        percentage_factor: u128,
        weight_meter: &mut WeightMeter,
    ) {
        let db_weight = T::DbWeight::get();

        // --- Slash validator
        // Slashes stake balance
        // Decreases reputation
        // Possibly removes node if under min reputation
        let slash_validator_weight = Self::slash_validator_v2(
            subnet_id,
            consensus_submission_data.validator_subnet_node_id,
            consensus_submission_data.attestation_ratio,
            min_attestation_percentage,
            coldkey_reputation_decrease_factor,
            min_validator_reputation,
            electable_nodes_count,
            current_epoch,
        );
        weight_meter.consume(slash_validator_weight);

        // Decrease subnet reputation
        let factor_2 = percentage_factor.saturating_sub(Self::percent_div(
            consensus_submission_data.attestation_ratio,
            min_attestation_percentage,
        ));

        Self::decrease_subnet_reputation(
            subnet_id,
            NotInConsensusSubnetReputationFactor::<T>::get(),
            Some(factor_2),
        );
        // NotInConsensusSubnetReputationFactor | SubnetReputation
        weight_meter.consume(db_weight.reads_writes(2, 1));

        // Get the decrease factor based on the attestation ratio
        let non_consensus_attestor_factor = Self::get_non_consensus_attestor_factor(
            subnet_id,
            consensus_submission_data.attestation_ratio,
            min_attestation_percentage,
            percentage_factor,
        );
        // NonConsensusAttestorDecreaseReputationFactor
        weight_meter.consume(db_weight.reads(1));

        // --- Decrease reputation of attestors
        for (subnet_node_id, attest_data) in consensus_submission_data.attests {
            if let Some(rep) = SubnetNodeReputation::<T>::get(subnet_id, subnet_node_id) {
                // if let Some(hotkey) = SubnetNodeIdHotkey::<T>::get(subnet_id, subnet_node_id) {
                // We read the hotkey for 2 reasons:
                // 1. Make sure the node currently is active
                // 2. Get the latest hotkey for the node in case it was updated between when
                //    the validator submitted consensus data, and rewards distribution (now)
                //
                // Note: It's possible for the node to had been removed in this step
                // if the node was the elecated validator and was removed in the ``slash_validator`` step, or
                // if the node removed itself prior to this rewards distribution call.
                let new_reputation = Self::decrease_and_return_node_reputation(
                    subnet_id,
                    subnet_node_id,
                    rep,
                    non_consensus_attestor_factor,
                    None,
                );

                // `decrease_and_return_node_reputation`: SubnetNodeReputation (r/w)
                weight_meter.consume(db_weight.reads_writes(2, 1));

                if new_reputation < min_validator_reputation {
                    weight_meter.consume(db_weight.reads(1));
                    weight_meter.consume(db_weight.reads(1));
                    let coldkey_subnet_nodes = ValidatorSubnetNodes::<T>::get(subnet_node_id);
                    // x = number of subnets (outer BTreeMap size)
                    let x = coldkey_subnet_nodes.len() as u32;
                    // c = number of nodes in the specific subnet (inner BTreeSet size)
                    let c = coldkey_subnet_nodes
                        .get(&subnet_id)
                        .map(|nodes| nodes.len() as u32)
                        .unwrap_or(0);

                    if weight_meter.can_consume(T::WeightInfo::remove_active_subnet_node(
                        x,
                        electable_nodes_count,
                        c,
                    )) {
                        Self::remove_active_subnet_node_v2(subnet_id, subnet_node_id);
                        weight_meter.consume(T::WeightInfo::remove_active_subnet_node(
                            x,
                            electable_nodes_count,
                            c,
                        ));
                    }

                    // if let Ok(coldkey) = HotkeyOwner::<T>::try_get(&hotkey) {
                    //     weight_meter.consume(db_weight.reads(1));
                    //     let coldkey_subnet_nodes = ValidatorSubnetNodes::<T>::get(subnet_node_id);
                    //     // x = number of subnets (outer BTreeMap size)
                    //     let x = coldkey_subnet_nodes.len() as u32;
                    //     // c = number of nodes in the specific subnet (inner BTreeSet size)
                    //     let c = coldkey_subnet_nodes
                    //         .get(&subnet_id)
                    //         .map(|nodes| nodes.len() as u32)
                    //         .unwrap_or(0);

                    //     if weight_meter.can_consume(T::WeightInfo::remove_active_subnet_node(
                    //         x,
                    //         electable_nodes_count,
                    //         c,
                    //     )) {
                    //         Self::remove_active_subnet_node_v2(subnet_id, subnet_node_id);
                    //         weight_meter.consume(T::WeightInfo::remove_active_subnet_node(
                    //             x,
                    //             electable_nodes_count,
                    //             c,
                    //         ));
                    //     }
                    // }
                }
            }
            continue;
        }
    }

    pub fn handle_validator_reward_v2(
        weight_meter: &mut WeightMeter,
        validator_id: u32,
        subnet_id: u32,
        subnet_node_id: u32,
        consensus_submission_data: &ConsensusSubmissionDataV2,
        min_attestation_percentage: u128,
        coldkey_reputation_increase_factor: u128,
        current_epoch: u32,
    ) {
        let db_weight = T::DbWeight::get();

        weight_meter.consume(db_weight.reads(1));

        // --- Increase validator reward
        let validator_reward = Self::get_validator_reward(
            consensus_submission_data.attestation_ratio,
            consensus_submission_data.validator_reward_factor,
        );
        // Add get_validator_reward (At least 1 read, up to 2)
        // MinAttestationPercentage | BaseValidatorReward
        weight_meter.consume(db_weight.reads(2));

        Self::increase_validator_reputation(
            validator_id,
            consensus_submission_data.attestation_ratio,
            min_attestation_percentage,
            coldkey_reputation_increase_factor,
            current_epoch,
        );

        // weight_meter.consume(T::WeightInfo::increase_validator_reputation());

        // HotkeyOwner
        weight_meter.consume(db_weight.reads(1));

        // Give validator rewards to their stake
        Self::increase_node_stake(subnet_node_id, subnet_id, validator_reward);
    }

    pub fn handle_subnet_owner_reward_v2(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        amount: u128,
    ) {
        // SubnetOwner
        weight_meter.consume(T::DbWeight::get().reads(1));
        if let Ok(owner) = SubnetOwner::<T>::try_get(subnet_id) {
            if let Some(balance) = Self::u128_to_balance(amount) {
                Self::add_balance_to_coldkey_account(&owner, balance);
                weight_meter.consume(T::WeightInfo::add_balance_to_coldkey_account());
            }
        }
    }

    /// Handles node queue operations based on consensus data.
    ///
    /// This function allows the validator to prioritize or remove nodes from the registration queue.
    ///
    /// # Parameters
    ///
    /// * `weight_meter` - Weight meter for tracking weight consumption
    /// * `subnet_id` - The ID of the subnet
    /// * `consensus_submission_data` - Consensus submission data containing queue operations
    /// * `super_majority_threshold` - The super majority threshold for consensus
    /// * `electable_nodes_count` - The number of electable nodes in the subnet
    ///
    /// # Behavior
    ///
    /// The function performs the following steps:
    /// 1. Checks if the consensus submission has super majority
    /// 2. Retrieves the node queue for the subnet
    /// 3. Handles prioritize node operation if specified
    /// 4. Handles remove node operation if specified
    ///
    /// # Errors
    ///
    /// * `SubnetNodeNotFoundInQueue` - Node not found in queue
    /// * `SubnetNodeNotImmune` - Node not immune from removal
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success
    pub fn handle_node_queue_consensus_v2(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        consensus_submission_data: &ConsensusSubmissionDataV2,
        super_majority_threshold: u128,
        electable_nodes_count: u32,
    ) {
        if consensus_submission_data.attestation_ratio >= super_majority_threshold {
            let db_weight = T::DbWeight::get();

            let mut queue = SubnetNodeQueueV2::<T>::get(subnet_id);

            // Handle prioritize node - move to front
            if let Some(prioritize_queue_node_id) =
                consensus_submission_data.prioritize_queue_node_id
            {
                weight_meter.consume(db_weight.reads(1));

                if let Some(index) = queue
                    .iter()
                    .position(|node| node.id == prioritize_queue_node_id)
                {
                    let node = queue.remove(index); // Remove from current position
                    queue.insert(0, node); // Insert at front (index 0)

                    // Add computational weight for vector operations
                    weight_meter.consume(Weight::from_parts(
                        queue.len() as u64 * 100, // Linear cost based on queue size
                        0,
                    ));

                    SubnetNodeQueueV2::<T>::insert(subnet_id, &queue);
                    weight_meter.consume(db_weight.writes(1));

                    Self::deposit_event(Event::QueuedNodePrioritized {
                        subnet_id,
                        subnet_node_id: prioritize_queue_node_id,
                    });
                }
            }

            // Handle remove node - remove from queue entirely
            // These are not yet activated nodes so this does not impact the emissions distribution
            if let Some(remove_queue_node_id) = consensus_submission_data.remove_queue_node_id {
                weight_meter.consume(db_weight.reads(1));
                // We read the hotkey for 2 reasons:
                // 1. Make sure the node currently is active
                // 2. Get the latest hotkey for the node in case it was updated between when
                //    the validator submitted consensus data, and rewards distribution (now)
                weight_meter.consume(db_weight.reads(1));
                let coldkey_subnet_nodes = ValidatorSubnetNodes::<T>::get(remove_queue_node_id);
                // x = number of subnets (outer BTreeMap size)
                let x = coldkey_subnet_nodes.len() as u32;
                // c = number of nodes in the specific subnet (inner BTreeSet size)
                let c = coldkey_subnet_nodes
                    .get(&subnet_id)
                    .map(|nodes| nodes.len() as u32)
                    .unwrap_or(0);

                if weight_meter.can_consume(T::WeightInfo::remove_registered_subnet_node(
                    x,
                    electable_nodes_count,
                    c,
                )) {
                    Self::remove_registered_subnet_node_v2(subnet_id, remove_queue_node_id);
                    weight_meter.consume(T::WeightInfo::remove_registered_subnet_node(
                        x,
                        electable_nodes_count,
                        c,
                    ));
                    Self::deposit_event(Event::QueuedNodeRemoved {
                        subnet_id,
                        subnet_node_id: remove_queue_node_id,
                    });
                }

                // if let Some(hotkey) = SubnetNodeIdHotkey::<T>::get(subnet_id, remove_queue_node_id)
                // {
                //     weight_meter.consume(db_weight.reads(1));
                //     if let Ok(coldkey) = HotkeyOwner::<T>::try_get(&hotkey) {
                //         weight_meter.consume(db_weight.reads(1));
                //         let coldkey_subnet_nodes = ValidatorSubnetNodes::<T>::get(remove_queue_node_id);
                //         // x = number of subnets (outer BTreeMap size)
                //         let x = coldkey_subnet_nodes.len() as u32;
                //         // c = number of nodes in the specific subnet (inner BTreeSet size)
                //         let c = coldkey_subnet_nodes
                //             .get(&subnet_id)
                //             .map(|nodes| nodes.len() as u32)
                //             .unwrap_or(0);

                //         if weight_meter.can_consume(T::WeightInfo::remove_registered_subnet_node(
                //             x,
                //             electable_nodes_count,
                //             c,
                //         )) {
                //             Self::remove_registered_subnet_node_v2(subnet_id, remove_queue_node_id);
                //             weight_meter.consume(T::WeightInfo::remove_registered_subnet_node(
                //                 x,
                //                 electable_nodes_count,
                //                 c,
                //             ));
                //             Self::deposit_event(Event::QueuedNodeRemoved {
                //                 subnet_id,
                //                 subnet_node_id: remove_queue_node_id,
                //             });
                //         }
                //     }
                // }
            }
        }
    }

    pub fn handle_consensus_remove_active_node_v2(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        subnet_node_id: u32,
        electable_nodes_count: u32,
    ) {
        let db_weight = T::DbWeight::get();
        // Remove node if they haven't already been removed
        weight_meter.consume(db_weight.reads(1));
        // We read the hotkey for 2 reasons:
        // 1. Make sure the node currently is active
        // 2. Get the latest hotkey for the node in case it was updated between when
        //    the validator submitted consensus data, and rewards distribution (now)
        weight_meter.consume(db_weight.reads(1));
        let coldkey_subnet_nodes = ValidatorSubnetNodes::<T>::get(subnet_node_id);
        // x = number of subnets (outer BTreeMap size)
        let x = coldkey_subnet_nodes.len() as u32;
        // c = number of nodes in the specific subnet (inner BTreeSet size)
        let c = coldkey_subnet_nodes
            .get(&subnet_id)
            .map(|nodes| nodes.len() as u32)
            .unwrap_or(0);

        if weight_meter.can_consume(T::WeightInfo::remove_active_subnet_node(
            x,
            electable_nodes_count,
            c,
        )) {
            Self::remove_active_subnet_node_v2(subnet_id, subnet_node_id);
            weight_meter.consume(T::WeightInfo::remove_active_subnet_node(
                x,
                electable_nodes_count,
                c,
            ));
        }

        // if let Some(hotkey) = SubnetNodeIdHotkey::<T>::get(subnet_id, subnet_node_id) {
        //     weight_meter.consume(db_weight.reads(1));
        //     if let Ok(coldkey) = HotkeyOwner::<T>::try_get(&hotkey) {
        //         weight_meter.consume(db_weight.reads(1));
        //         let coldkey_subnet_nodes = ValidatorSubnetNodes::<T>::get(subnet_node_id);
        //         // x = number of subnets (outer BTreeMap size)
        //         let x = coldkey_subnet_nodes.len() as u32;
        //         // c = number of nodes in the specific subnet (inner BTreeSet size)
        //         let c = coldkey_subnet_nodes
        //             .get(&subnet_id)
        //             .map(|nodes| nodes.len() as u32)
        //             .unwrap_or(0);

        //         if weight_meter.can_consume(T::WeightInfo::remove_active_subnet_node(
        //             x,
        //             electable_nodes_count,
        //             c,
        //         )) {
        //             Self::remove_active_subnet_node_v2(subnet_id, subnet_node_id);
        //             weight_meter.consume(T::WeightInfo::remove_active_subnet_node(
        //                 x,
        //                 electable_nodes_count,
        //                 c,
        //             ));
        //         }
        //     }
        // }
    }

    pub fn handle_idle_node_v2(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        subnet_node_id: u32,
        idle_epochs: u32,
        current_subnet_epoch: u32,
    ) {
        let db_weight = T::DbWeight::get();
        let node_idle_epochs = SubnetNodeIdleConsecutiveEpochs::<T>::try_mutate(
            subnet_id,
            subnet_node_id,
            |n: &mut u32| -> Result<u32, DispatchError> {
                *n += 1;
                Ok(*n)
            },
        );
        weight_meter.consume(db_weight.reads_writes(1, 1));

        // Idle classified nodes can't be included in consensus data and can't have a used reputation
        // so we check the class immediately.
        // --- Upgrade to Included if past the queue epochs
        match node_idle_epochs {
            Ok(node_idle_epochs) => {
                if node_idle_epochs >= idle_epochs {
                    if Self::graduate_class_v2(subnet_id, subnet_node_id, current_subnet_epoch) {
                        SubnetNodeIdleConsecutiveEpochs::<T>::remove(subnet_id, subnet_node_id);
                        weight_meter.consume(db_weight.writes(1));
                    }
                }
            }
            Err(_) => return,
        }
    }

    pub fn handle_included_node_v2(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
        subnet_node_id: u32,
        reputation: u128,
        percentage_factor: u128,
        included_epochs: u32,
        current_subnet_epoch: u32,
    ) {
        let db_weight = T::DbWeight::get();
        let node_included_epochs = SubnetNodeConsecutiveIncludedEpochs::<T>::try_mutate(
            subnet_id,
            subnet_node_id,
            |n: &mut u32| -> Result<u32, DispatchError> {
                *n += 1;
                Ok(*n)
            },
        );

        // SubnetNodeConsecutiveIncludedEpochs
        weight_meter.consume(db_weight.reads_writes(1, 1));

        // --- Upgrade to Validator if at percentage_factor reputation and included in weights
        match node_included_epochs {
            Ok(node_included_epochs) => {
                if reputation >= percentage_factor && node_included_epochs >= included_epochs {
                    if Self::graduate_to_validator_class_v2(
                        subnet_id,
                        subnet_node_id,
                        current_subnet_epoch,
                    ) {
                        // --- Remove consecutive included epochs as this node will never need this
                        // counter again
                        SubnetNodeConsecutiveIncludedEpochs::<T>::remove(subnet_id, subnet_node_id);
                        weight_meter.consume(db_weight.writes(1));
                    }
                }
            }
            Err(_) => return,
        }
    }

    pub fn maybe_get_forked_subnet_node_ids_v2(
        weight_meter: &mut WeightMeter,
        subnet_id: u32,
    ) -> Option<BTreeSet<u32>> {
        let db_weight = T::DbWeight::get();

        // EmergencySubnetNodeElectionData
        weight_meter.consume(db_weight.reads(1));
        let forked_subnet_node_ids: Option<BTreeSet<u32>> =
            EmergencySubnetNodeElectionData::<T>::mutate_exists(subnet_id, |maybe_data| {
                if let Some(data) = maybe_data {
                    weight_meter.consume(db_weight.writes(1));

                    // Increment `total_epochs`
                    data.total_epochs = data.total_epochs.saturating_add(1);

                    Some(data.subnet_node_ids.iter().cloned().collect())
                } else {
                    None
                }
            });

        forked_subnet_node_ids
    }

    pub fn handle_validator_delegate_stake_v2(
        weight_meter: &mut WeightMeter,
        validator_id: u32,
        delegate_reward_rate: u128,
        account_reward: u128,
    ) -> Option<(u128, (u32, u128))> {
        let db_weight = T::DbWeight::get();
        // --- Ensure users are staked to subnet node
        let total_node_delegated_stake_shares =
            ValidatorDelegateStakeShares::<T>::get(validator_id);
        // TotalNodeDelegateStakeShares
        weight_meter.consume(db_weight.reads(1));

        log::error!("handle_validator_delegate_stake_v2");

        // We make sure the pool has shares before depositing into it
        if total_node_delegated_stake_shares != 0 {
            let node_delegate_reward = Self::percent_mul(account_reward, delegate_reward_rate);
            let updated_account_reward = account_reward.saturating_sub(node_delegate_reward);
            log::error!(
                "handle_validator_delegate_stake_v2 node_delegate_reward   {:?}",
                node_delegate_reward
            );
            log::error!(
                "handle_validator_delegate_stake_v2 updated_account_reward {:?}",
                updated_account_reward
            );
            Self::do_increase_validator_delegate_stake(validator_id, node_delegate_reward);
            // reads:
            // TotalNodeDelegateStakeBalance | TotalNodeDelegateStakeShares
            //
            // writes:
            // TotalNodeDelegateStakeShares | TotalNodeDelegateStakeBalance | TotalNodeDelegateStake
            weight_meter.consume(db_weight.reads_writes(5, 3));

            return Some((updated_account_reward, (validator_id, node_delegate_reward)));
        }
        None
    }

    pub fn handle_delegate_account_v2(
        weight_meter: &mut WeightMeter,
        account_reward: u128,
        delegate_account_id: &T::AccountId,
        rate: u128,
    ) -> (u128, u128) {
        let delegate_account_deposit = Self::percent_mul(account_reward, rate);
        let updated_account_reward = account_reward.saturating_sub(delegate_account_deposit);
        Self::increase_delegate_account_balance(delegate_account_id, delegate_account_deposit);

        (updated_account_reward, delegate_account_deposit)
    }
}
