// This file is part of Hypertensor.

// Copyright (C) 2023 Parity Technologies (UK) Ltd.
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

//! Runtime API definition for the network pallet.

#![cfg_attr(not(feature = "std"), no_std)]
use fp_account::AccountId20;
use network_rpc_types::{
    ConsensusRoundInfo, EffectiveOverwatchSignalMeta, EffectiveOverwatchSubnetWeight,
    NetworkQueryError, OverwatchNodeInfo, OverwatchNodesPage, PageRequest, SubnetBootnodes,
    SubnetEpochStatus, SubnetInfo, SubnetNodeCursor, SubnetNodeInfo, SubnetNodesPage,
    SubnetValidatorNodesPage, SubnetsPage, ValidatorInfo, ValidatorNodeAllocationsPage,
    ValidatorNodeStakesPage, ValidatorNodesPage,
};

sp_api::decl_runtime_apis! {
  pub trait NetworkRuntimeApi {
    fn get_subnet_info(subnet_id: u32) -> Option<SubnetInfo<AccountId20>>;
    fn get_subnets(request: PageRequest<u32>)
      -> Result<SubnetsPage<AccountId20>, NetworkQueryError>;
    fn get_subnet_node_info(
      subnet_id: u32,
      subnet_node_id: u32,
    ) -> Option<SubnetNodeInfo<AccountId20>>;
    fn get_subnet_nodes(
      subnet_id: u32,
      request: PageRequest<u32>,
    ) -> Result<SubnetNodesPage<AccountId20>, NetworkQueryError>;
    fn get_bootnodes(subnet_id: u32) -> Option<SubnetBootnodes>;
    fn get_validator_info(validator_id: u32) -> Option<ValidatorInfo<AccountId20>>;
    fn get_validator_by_coldkey(coldkey: AccountId20) -> Option<ValidatorInfo<AccountId20>>;
    fn get_validator_by_hotkey(hotkey: AccountId20) -> Option<ValidatorInfo<AccountId20>>;
    fn get_validator_nodes(
      validator_id: u32,
      request: PageRequest<SubnetNodeCursor>,
    ) -> Result<ValidatorNodesPage<AccountId20>, NetworkQueryError>;
    fn get_validator_node_stakes(
      validator_id: u32,
      request: PageRequest<SubnetNodeCursor>,
    ) -> Result<ValidatorNodeStakesPage, NetworkQueryError>;
    fn get_validator_node_allocations(
      validator_id: u32,
      request: PageRequest<SubnetNodeCursor>,
    ) -> Result<ValidatorNodeAllocationsPage, NetworkQueryError>;
    fn get_consensus_round(
      subnet_id: u32,
      subnet_epoch: u32,
    ) -> Result<Option<ConsensusRoundInfo>, NetworkQueryError>;
    fn get_subnet_validator_nodes(
      subnet_id: u32,
      request: PageRequest<u32>,
    ) -> Result<SubnetValidatorNodesPage<AccountId20>, NetworkQueryError>;
    fn get_subnet_epoch_status(subnet_id: u32)
      -> Result<SubnetEpochStatus, NetworkQueryError>;
    fn get_overwatch_node_info(overwatch_node_id: u32)
      -> Option<OverwatchNodeInfo<AccountId20>>;
    fn get_overwatch_nodes(request: PageRequest<u32>)
      -> Result<OverwatchNodesPage<AccountId20>, NetworkQueryError>;
    fn get_effective_overwatch_signal_meta() -> EffectiveOverwatchSignalMeta;
    fn get_effective_overwatch_subnet_weight(subnet_id: u32)
      -> EffectiveOverwatchSubnetWeight;
  }
}
