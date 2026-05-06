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
use frame_support::pallet_prelude::DispatchError;
use frame_support::pallet_prelude::Weight;

impl<T: Config> Pallet<T> {
    pub fn do_update_node_peer_info(
        subnet_id: u32,
        subnet_node_id: u32,
        new_peer_info: PeerInfo,
    ) -> DispatchResult {
        if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            SubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_node_peer_id(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        new_peer_info,
                    )
                },
            )?;

            return Ok(());
        } else if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            SubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_node_peer_id(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        new_peer_info,
                    )
                },
            )?;

            return Ok(());
        }

        Err(Error::<T>::InvalidSubnetNodeId.into())
    }

    fn perform_update_node_peer_id(
        subnet_id: u32,
        subnet_node_id: u32,
        maybe_params: &mut Option<SubnetNodeV2>,
        new_peer_info: PeerInfo,
    ) -> DispatchResult {
        let params = maybe_params
            .as_mut()
            .ok_or(Error::<T>::InvalidSubnetNodeId)?;

        Self::validate_peer_info(subnet_id, 0, 0, &new_peer_info)?;

        PeerIdSubnetNodeId::<T>::remove(subnet_id, &params.peer_info.peer_id);
        if let Some(multiaddr) = params.peer_info.multiaddr.clone() {
            MultiaddrSubnetNodeId::<T>::remove(subnet_id, multiaddr);
        }

        PeerIdSubnetNodeId::<T>::insert(subnet_id, &new_peer_info.peer_id, subnet_node_id);

        if let Some(multiaddr) = new_peer_info.multiaddr.clone() {
            // Validated in `validate_peer_info`
            MultiaddrSubnetNodeId::<T>::insert(subnet_id, &multiaddr, subnet_node_id);
        }

        params.peer_info = new_peer_info.clone();

        Self::deposit_event(Event::SubnetNodeUpdatePeerInfo {
            subnet_id,
            subnet_node_id,
            peer_info: new_peer_info,
        });

        Ok(())
    }

    pub fn do_update_peer_info(
        subnet_id: u32,
        subnet_node_id: u32,
        new_peer_info: PeerInfo,
    ) -> DispatchResult {
        if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            SubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_peer_id(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        new_peer_info,
                    )
                },
            )?;

            return Ok(());
        } else if RegisteredSubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            RegisteredSubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_peer_id(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        new_peer_info,
                    )
                },
            )?;

            return Ok(());
        }

        Err(Error::<T>::InvalidSubnetNodeId.into())
    }

    fn perform_update_peer_id(
        subnet_id: u32,
        subnet_node_id: u32,
        maybe_params: &mut Option<SubnetNodeV2>,
        new_peer_info: PeerInfo,
    ) -> DispatchResult {
        let params = maybe_params
            .as_mut()
            .ok_or(Error::<T>::InvalidSubnetNodeId)?;

        Self::validate_peer_info(subnet_id, 0, 0, &new_peer_info)?;

        PeerIdSubnetNodeId::<T>::remove(subnet_id, &params.peer_info.peer_id);
        if let Some(multiaddr) = params.peer_info.multiaddr.clone() {
            MultiaddrSubnetNodeId::<T>::remove(subnet_id, multiaddr);
        }

        PeerIdSubnetNodeId::<T>::insert(subnet_id, &new_peer_info.peer_id, subnet_node_id);

        if let Some(multiaddr) = new_peer_info.multiaddr.clone() {
            // Validated in `validate_peer_info`
            MultiaddrSubnetNodeId::<T>::insert(subnet_id, &multiaddr, subnet_node_id);
        }

        params.peer_info = new_peer_info.clone();

        Self::deposit_event(Event::SubnetNodeUpdatePeerInfo {
            subnet_id,
            subnet_node_id,
            peer_info: new_peer_info,
        });

        Ok(())
    }

    pub fn do_update_bootnode_peer_info(
        subnet_id: u32,
        subnet_node_id: u32,
        new_bootnode_peer_info: Option<PeerInfo>,
    ) -> DispatchResult {
        if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            SubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_bootnode_peer_id(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        new_bootnode_peer_info,
                    )
                },
            )?;

            return Ok(());
        } else if RegisteredSubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            RegisteredSubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_bootnode_peer_id(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        new_bootnode_peer_info,
                    )
                },
            )?;

            return Ok(());
        }

        Err(Error::<T>::InvalidSubnetNodeId.into())
    }

    fn perform_update_bootnode_peer_id(
        subnet_id: u32,
        subnet_node_id: u32,
        maybe_params: &mut Option<SubnetNodeV2>,
        new_peer_info: Option<PeerInfo>,
    ) -> DispatchResult {
        let params = maybe_params
            .as_mut()
            .ok_or(Error::<T>::InvalidSubnetNodeId)?;

        if let Some(peer_info) = &new_peer_info {
            Self::validate_peer_info(subnet_id, 0, 0, &peer_info)?;

            // Remove old peer info after validate
            if let Some(current_peer_info) = &params.bootnode_peer_info {
                BootnodePeerIdSubnetNodeId::<T>::remove(subnet_id, &current_peer_info.peer_id);
                if let Some(multiaddr) = &current_peer_info.multiaddr {
                    MultiaddrSubnetNodeId::<T>::remove(subnet_id, multiaddr);
                }
            }

            BootnodePeerIdSubnetNodeId::<T>::insert(subnet_id, &peer_info.peer_id, subnet_node_id);

            if let Some(multiaddr) = &peer_info.multiaddr {
                // Validated in `validate_peer_info`
                MultiaddrSubnetNodeId::<T>::insert(subnet_id, multiaddr, subnet_node_id);
            }
        } else {
            if let Some(peer_info) = &params.bootnode_peer_info {
                BootnodePeerIdSubnetNodeId::<T>::remove(subnet_id, &peer_info.peer_id);
                if let Some(multiaddr) = &peer_info.multiaddr {
                    MultiaddrSubnetNodeId::<T>::remove(subnet_id, multiaddr);
                }
            }
        }

        params.bootnode_peer_info = new_peer_info.clone();

        Self::deposit_event(Event::SubnetNodeUpdateBootnodePeerInfo {
            subnet_id,
            subnet_node_id,
            bootnode_peer_info: new_peer_info.clone(),
        });

        Ok(())
    }

    pub fn do_update_node_bootnode_peer_info(
        subnet_id: u32,
        subnet_node_id: u32,
        new_bootnode_peer_info: Option<PeerInfo>,
    ) -> DispatchResult {
        if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            SubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_node_bootnode_peer_id(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        new_bootnode_peer_info,
                    )
                },
            )?;

            return Ok(());
        } else if RegisteredSubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            RegisteredSubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_node_bootnode_peer_id(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        new_bootnode_peer_info,
                    )
                },
            )?;

            return Ok(());
        }

        Err(Error::<T>::InvalidSubnetNodeId.into())
    }

    fn perform_update_node_bootnode_peer_id(
        subnet_id: u32,
        subnet_node_id: u32,
        maybe_params: &mut Option<SubnetNodeV2>,
        new_peer_info: Option<PeerInfo>,
    ) -> DispatchResult {
        let params = maybe_params
            .as_mut()
            .ok_or(Error::<T>::InvalidSubnetNodeId)?;

        if let Some(peer_info) = &new_peer_info {
            Self::validate_peer_info(subnet_id, 0, 0, &peer_info)?;

            // Remove old peer info after validate
            if let Some(current_peer_info) = &params.bootnode_peer_info {
                BootnodePeerIdSubnetNodeId::<T>::remove(subnet_id, &current_peer_info.peer_id);
                if let Some(multiaddr) = &current_peer_info.multiaddr {
                    MultiaddrSubnetNodeId::<T>::remove(subnet_id, multiaddr);
                }
            }

            BootnodePeerIdSubnetNodeId::<T>::insert(subnet_id, &peer_info.peer_id, subnet_node_id);

            if let Some(multiaddr) = &peer_info.multiaddr {
                // Validated in `validate_peer_info`
                MultiaddrSubnetNodeId::<T>::insert(subnet_id, multiaddr, subnet_node_id);
            }
        } else {
            if let Some(peer_info) = &params.bootnode_peer_info {
                BootnodePeerIdSubnetNodeId::<T>::remove(subnet_id, &peer_info.peer_id);
                if let Some(multiaddr) = &peer_info.multiaddr {
                    MultiaddrSubnetNodeId::<T>::remove(subnet_id, multiaddr);
                }
            }
        }

        params.bootnode_peer_info = new_peer_info.clone();

        Self::deposit_event(Event::SubnetNodeUpdateBootnodePeerInfo {
            subnet_id,
            subnet_node_id,
            bootnode_peer_info: new_peer_info.clone(),
        });

        Ok(())
    }

    pub fn do_update_client_peer_info(
        subnet_id: u32,
        subnet_node_id: u32,
        new_peer_info: Option<PeerInfo>,
    ) -> DispatchResult {
        if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            SubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_client_peer_id(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        new_peer_info,
                    )
                },
            )?;

            return Ok(());
        } else if RegisteredSubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            RegisteredSubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_client_peer_id(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        new_peer_info,
                    )
                },
            )?;

            return Ok(());
        }

        Err(Error::<T>::InvalidSubnetNodeId.into())
    }

    fn perform_update_client_peer_id(
        subnet_id: u32,
        subnet_node_id: u32,
        maybe_params: &mut Option<SubnetNodeV2>,
        new_peer_info: Option<PeerInfo>,
    ) -> DispatchResult {
        let params = maybe_params
            .as_mut()
            .ok_or(Error::<T>::InvalidSubnetNodeId)?;

        if let Some(peer_info) = &new_peer_info {
            Self::validate_peer_info(subnet_id, 0, 0, &peer_info)?;

            if let Some(current_peer_info) = &params.client_peer_info {
                ClientPeerIdSubnetNodeId::<T>::remove(subnet_id, &current_peer_info.peer_id);
                if let Some(multiaddr) = current_peer_info.multiaddr.clone() {
                    MultiaddrSubnetNodeId::<T>::remove(subnet_id, multiaddr);
                }
            }

            ClientPeerIdSubnetNodeId::<T>::insert(subnet_id, &peer_info.peer_id, subnet_node_id);

            if let Some(multiaddr) = peer_info.multiaddr.clone() {
                // Validated in `validate_peer_info`
                MultiaddrSubnetNodeId::<T>::insert(subnet_id, &multiaddr, subnet_node_id);
            }
        } else {
            if let Some(peer_info) = &params.client_peer_info {
                ClientPeerIdSubnetNodeId::<T>::remove(subnet_id, &peer_info.peer_id);
                if let Some(multiaddr) = peer_info.multiaddr.clone() {
                    MultiaddrSubnetNodeId::<T>::remove(subnet_id, multiaddr);
                }
            }
        }

        params.client_peer_info = new_peer_info.clone();

        Self::deposit_event(Event::SubnetNodeUpdateClientPeerInfo {
            subnet_id,
            subnet_node_id,
            client_peer_info: new_peer_info,
        });

        Ok(())
    }

    pub fn do_update_node_client_peer_info(
        subnet_id: u32,
        subnet_node_id: u32,
        new_peer_info: Option<PeerInfo>,
    ) -> DispatchResult {
        if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            SubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_node_client_peer_id(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        new_peer_info,
                    )
                },
            )?;

            return Ok(());
        } else if RegisteredSubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            RegisteredSubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_node_client_peer_id(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        new_peer_info,
                    )
                },
            )?;

            return Ok(());
        }

        Err(Error::<T>::InvalidSubnetNodeId.into())
    }

    fn perform_update_node_client_peer_id(
        subnet_id: u32,
        subnet_node_id: u32,
        maybe_params: &mut Option<SubnetNodeV2>,
        new_peer_info: Option<PeerInfo>,
    ) -> DispatchResult {
        let params = maybe_params
            .as_mut()
            .ok_or(Error::<T>::InvalidSubnetNodeId)?;

        if let Some(peer_info) = &new_peer_info {
            Self::validate_peer_info(subnet_id, 0, 0, &peer_info)?;

            if let Some(current_peer_info) = &params.client_peer_info {
                ClientPeerIdSubnetNodeId::<T>::remove(subnet_id, &current_peer_info.peer_id);
                if let Some(multiaddr) = current_peer_info.multiaddr.clone() {
                    MultiaddrSubnetNodeId::<T>::remove(subnet_id, multiaddr);
                }
            }

            ClientPeerIdSubnetNodeId::<T>::insert(subnet_id, &peer_info.peer_id, subnet_node_id);

            if let Some(multiaddr) = peer_info.multiaddr.clone() {
                // Validated in `validate_peer_info`
                MultiaddrSubnetNodeId::<T>::insert(subnet_id, &multiaddr, subnet_node_id);
            }
        } else {
            if let Some(peer_info) = &params.client_peer_info {
                ClientPeerIdSubnetNodeId::<T>::remove(subnet_id, &peer_info.peer_id);
                if let Some(multiaddr) = peer_info.multiaddr.clone() {
                    MultiaddrSubnetNodeId::<T>::remove(subnet_id, multiaddr);
                }
            }
        }

        params.client_peer_info = new_peer_info.clone();

        Self::deposit_event(Event::SubnetNodeUpdateClientPeerInfo {
            subnet_id,
            subnet_node_id,
            client_peer_info: new_peer_info,
        });

        Ok(())
    }

    pub fn do_update_unique(
        subnet_id: u32,
        subnet_node_id: u32,
        unique: Option<BoundedVec<u8, DefaultMaxVectorLength>>,
    ) -> DispatchResult {
        if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            SubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_unique(subnet_id, subnet_node_id, maybe_params, unique)
                },
            )?;

            return Ok(());
        } else if RegisteredSubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            RegisteredSubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_unique(subnet_id, subnet_node_id, maybe_params, unique)
                },
            )?;

            return Ok(());
        }

        Err(Error::<T>::InvalidSubnetNodeId.into())
    }

    fn perform_update_unique(
        subnet_id: u32,
        subnet_node_id: u32,
        maybe_params: &mut Option<SubnetNodeV2>,
        unique: Option<BoundedVec<u8, DefaultMaxVectorLength>>,
    ) -> DispatchResult {
        let params = maybe_params
            .as_mut()
            .ok_or(Error::<T>::InvalidSubnetNodeId)?;

        // Remove nodes previous unique if Some
        if let Some(unique_param) = &params.unique {
            UniqueParamSubnetNodeId::<T>::remove(subnet_id, unique_param);
        }

        if let Some(unique) = unique.clone() {
            if let Ok(owner_subnet_node_id) =
                UniqueParamSubnetNodeId::<T>::try_get(subnet_id, &unique)
            {
                ensure!(
                    owner_subnet_node_id == subnet_node_id,
                    Error::<T>::UniqueParameterTaken
                );
            }

            UniqueParamSubnetNodeId::<T>::insert(subnet_id, &unique, subnet_node_id);
        }

        params.unique = unique.clone();

        Self::deposit_event(Event::SubnetNodeUpdateUnique {
            subnet_id,
            subnet_node_id,
            unique: unique,
        });

        Ok(())
    }

    pub fn do_update_node_unique(
        subnet_id: u32,
        subnet_node_id: u32,
        unique: Option<BoundedVec<u8, DefaultMaxVectorLength>>,
    ) -> DispatchResult {
        if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            SubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_node_unique(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        unique,
                    )
                },
            )?;

            return Ok(());
        } else if RegisteredSubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            RegisteredSubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_node_unique(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        unique,
                    )
                },
            )?;

            return Ok(());
        }

        Err(Error::<T>::InvalidSubnetNodeId.into())
    }

    fn perform_update_node_unique(
        subnet_id: u32,
        subnet_node_id: u32,
        maybe_params: &mut Option<SubnetNodeV2>,
        unique: Option<BoundedVec<u8, DefaultMaxVectorLength>>,
    ) -> DispatchResult {
        let params = maybe_params
            .as_mut()
            .ok_or(Error::<T>::InvalidSubnetNodeId)?;

        // Remove nodes previous unique if Some
        if let Some(unique_param) = &params.unique {
            UniqueParamSubnetNodeId::<T>::remove(subnet_id, unique_param);
        }

        if let Some(unique) = unique.clone() {
            if let Ok(owner_subnet_node_id) =
                UniqueParamSubnetNodeId::<T>::try_get(subnet_id, &unique)
            {
                ensure!(
                    owner_subnet_node_id == subnet_node_id,
                    Error::<T>::UniqueParameterTaken
                );
            }

            UniqueParamSubnetNodeId::<T>::insert(subnet_id, &unique, subnet_node_id);
        }

        params.unique = unique.clone();

        Self::deposit_event(Event::SubnetNodeUpdateUnique {
            subnet_id,
            subnet_node_id,
            unique: unique,
        });

        Ok(())
    }

    pub fn do_update_non_unique(
        subnet_id: u32,
        subnet_node_id: u32,
        non_unique: Option<BoundedVec<u8, DefaultMaxVectorLength>>,
    ) -> DispatchResult {
        if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            SubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_non_unique(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        non_unique,
                    )
                },
            )?;

            return Ok(());
        } else if RegisteredSubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            RegisteredSubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_non_unique(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        non_unique,
                    )
                },
            )?;

            return Ok(());
        }

        Err(Error::<T>::InvalidSubnetNodeId.into())
    }

    fn perform_update_non_unique(
        subnet_id: u32,
        subnet_node_id: u32,
        maybe_params: &mut Option<SubnetNodeV2>,
        non_unique: Option<BoundedVec<u8, DefaultMaxVectorLength>>,
    ) -> DispatchResult {
        let params = maybe_params
            .as_mut()
            .ok_or(Error::<T>::InvalidSubnetNodeId)?;

        params.non_unique = non_unique.clone();

        Self::deposit_event(Event::SubnetNodeUpdateNonUnique {
            subnet_id,
            subnet_node_id,
            non_unique: non_unique,
        });

        Ok(())
    }

    pub fn do_update_node_non_unique(
        subnet_id: u32,
        subnet_node_id: u32,
        non_unique: Option<BoundedVec<u8, DefaultMaxVectorLength>>,
    ) -> DispatchResult {
        if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            SubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_node_non_unique(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        non_unique,
                    )
                },
            )?;

            return Ok(());
        } else if RegisteredSubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            RegisteredSubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_params| -> DispatchResult {
                    Self::perform_update_node_non_unique(
                        subnet_id,
                        subnet_node_id,
                        maybe_params,
                        non_unique,
                    )
                },
            )?;

            return Ok(());
        }

        Err(Error::<T>::InvalidSubnetNodeId.into())
    }

    fn perform_update_node_non_unique(
        subnet_id: u32,
        subnet_node_id: u32,
        maybe_params: &mut Option<SubnetNodeV2>,
        non_unique: Option<BoundedVec<u8, DefaultMaxVectorLength>>,
    ) -> DispatchResult {
        let params = maybe_params
            .as_mut()
            .ok_or(Error::<T>::InvalidSubnetNodeId)?;

        params.non_unique = non_unique.clone();

        Self::deposit_event(Event::SubnetNodeUpdateNonUnique {
            subnet_id,
            subnet_node_id,
            non_unique: non_unique,
        });

        Ok(())
    }

    pub fn validate_peer_info(
        subnet_id: u32,
        subnet_node_id: u32,
        overwatch_node_id: u32,
        peer_info: &PeerInfo,
    ) -> DispatchResult {
        ensure!(
            Self::validate_peer_id(&peer_info.peer_id),
            Error::<T>::InvalidPeerId
        );

        ensure!(
            Self::is_owner_of_peer_or_ownerless(
                subnet_id,
                subnet_node_id,
                overwatch_node_id,
                &peer_info.peer_id
            ),
            Error::<T>::PeerIdExist
        );

        if let Some(peer_multiaddr) = peer_info.multiaddr.clone() {
            let multiaddr: &[u8] = &peer_multiaddr;

            Self::do_verify_multiaddr(multiaddr)?;

            ensure!(
                Self::is_owner_of_multiaddr_or_ownerless(
                    subnet_id,
                    subnet_node_id,
                    peer_multiaddr.clone()
                ),
                Error::<T>::MultiaddrExist
            );
        }

        Ok(())
    }

    pub fn do_verify_multiaddr(multiaddr: &[u8]) -> DispatchResult {
        multiaddr::Multiaddr::verify(multiaddr).map_err(|e| match e {
            multiaddr::MultiaddrError::InvalidVarint => Error::<T>::MultiaddrInvalidVarint,
            multiaddr::MultiaddrError::InvalidProtocol => Error::<T>::MultiaddrInvalidProtocol,
            multiaddr::MultiaddrError::InvalidAddress => Error::<T>::MultiaddrInvalidAddress,
            multiaddr::MultiaddrError::Truncated => Error::<T>::MultiaddrTruncated,
        })?;

        Ok(())
    }

    /// Inserts a node into the election slots, the list of nodes available to be chosen as validator
    /// Note: ONLY CALL THIS FUNCTION IF MAX NODES IS CHECKED
    pub fn insert_node_into_election_slot(subnet_id: u32, subnet_node_id: u32) -> bool {
        SubnetNodeElectionSlots::<T>::try_mutate(subnet_id, |slot_list| -> Result<bool, ()> {
            if !slot_list.contains(&subnet_node_id) {
                let idx = slot_list.len() as u32;
                slot_list.push(subnet_node_id);
                NodeSlotIndex::<T>::insert(subnet_id, subnet_node_id, idx);
                TotalSubnetElectableNodes::<T>::mutate(subnet_id, |mut n| n.saturating_inc());
                TotalElectableNodes::<T>::mutate(|mut n| n.saturating_inc());
                Ok(true)
            } else {
                Ok(false)
            }
        })
        .unwrap_or(false)
    }

    pub fn remove_node_from_election_slot(subnet_id: u32, subnet_node_id: u32) -> bool {
        SubnetNodeElectionSlots::<T>::try_mutate(subnet_id, |slot_list| -> Result<bool, ()> {
            if let Some(pos) = slot_list.iter().position(|id| *id == subnet_node_id) {
                let last_idx = slot_list.len() - 1;
                slot_list.swap_remove(pos);

                if pos != last_idx {
                    let moved_node_id = slot_list[pos];
                    NodeSlotIndex::<T>::insert(subnet_id, moved_node_id, pos as u32);
                }

                NodeSlotIndex::<T>::remove(subnet_id, subnet_node_id);
                TotalSubnetElectableNodes::<T>::mutate(subnet_id, |mut n| n.saturating_dec());
                TotalElectableNodes::<T>::mutate(|mut n| n.saturating_dec());
                Ok(true)
            } else {
                Ok(false)
            }
        })
        .unwrap_or(false)
    }

    pub fn remove_active_subnet_node(subnet_id: u32, subnet_node_id: u32) {
        let subnet_node = if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            SubnetNodesData::<T>::take(subnet_id, subnet_node_id)
        } else {
            return;
        };

        Self::common_remove_subnet_node(subnet_id, subnet_node_id, subnet_node.clone());

        if subnet_node.classification.node_class == SubnetNodeClass::Validator {
            // --- Try removing node from election slots (only happens if Validator classification)
            // Updates:
            // - `SubnetNodeElectionSlots`
            // - `TotalSubnetElectableNodes`
            // - `TotalElectableNodes`
            Self::remove_node_from_election_slot(subnet_id, subnet_node_id);
        }

        // Subtract from active node counts
        TotalActiveSubnetNodes::<T>::mutate(subnet_id, |n: &mut u32| n.saturating_dec());
        TotalActiveNodes::<T>::mutate(|n: &mut u32| n.saturating_dec());
        // If emergency validators set, remove node ID
        EmergencySubnetNodeElectionData::<T>::mutate_exists(subnet_id, |maybe_data| {
            if let Some(data) = maybe_data {
                data.subnet_node_ids.retain(|&id| id != subnet_node_id);
            }
        });
    }

    pub fn remove_registered_subnet_node(subnet_id: u32, subnet_node_id: u32) {
        let subnet_node = if RegisteredSubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id)
        {
            RegisteredSubnetNodesData::<T>::take(subnet_id, subnet_node_id)
        } else {
            return;
        };

        Self::common_remove_subnet_node(subnet_id, subnet_node_id, subnet_node.clone());

        SubnetNodeQueue::<T>::mutate(subnet_id, |nodes| {
            nodes.retain(|node| node.id != subnet_node_id);
        });
    }

    pub fn common_remove_subnet_node(
        subnet_id: u32,
        subnet_node_id: u32,
        subnet_node: SubnetNodeV2,
    ) {
        let peer_id = subnet_node.peer_info.peer_id.clone();

        if let Some(unique) = subnet_node.unique {
            UniqueParamSubnetNodeId::<T>::remove(subnet_id, unique);
        }

        // Remove all subnet node elements
        PeerIdSubnetNodeId::<T>::remove(subnet_id, &peer_id);
        if let Some(peer_info_multiaddr) = subnet_node.peer_info.multiaddr.clone() {
            MultiaddrSubnetNodeId::<T>::remove(subnet_id, peer_info_multiaddr);
        }

        if let Some(bootnode_peer_info) = subnet_node.bootnode_peer_info {
            BootnodePeerIdSubnetNodeId::<T>::remove(subnet_id, bootnode_peer_info.peer_id);
            if let Some(bootnode_multiaddr) = bootnode_peer_info.multiaddr.clone() {
                MultiaddrSubnetNodeId::<T>::remove(subnet_id, bootnode_multiaddr);
            }
        }

        if let Some(client_peer_info) = subnet_node.client_peer_info {
            ClientPeerIdSubnetNodeId::<T>::remove(subnet_id, client_peer_info.peer_id);
            if let Some(client_multiaddr) = client_peer_info.multiaddr.clone() {
                MultiaddrSubnetNodeId::<T>::remove(subnet_id, client_multiaddr);
            }
        }

        SubnetNodeIdHotkey::<T>::remove(subnet_id, subnet_node_id);
        SubnetNodeReputation::<T>::remove(subnet_id, subnet_node_id);
        SubnetNodeIdleConsecutiveEpochs::<T>::remove(subnet_id, subnet_node_id);
        SubnetNodeConsecutiveIncludedEpochs::<T>::remove(subnet_id, subnet_node_id);
        // We don't remove `HotkeySubnetId`. This is only removed when a node fully removes stake
        // to allow them to always unstake under their hotkey

        // Remove subnet ID from set
        ValidatorSubnetNodes::<T>::mutate(subnet_node.validator_id, |node_map| {
            if let Some(nodes) = node_map.get_mut(&subnet_id) {
                nodes.remove(&subnet_node_id);
                if nodes.is_empty() {
                    node_map.remove(&subnet_id);
                }
            }
        });

        // Subtract from coldkey reputation
        ValidatorReputation::<T>::mutate(subnet_node.validator_id, |rep| {
            rep.total_active_nodes = rep.total_active_nodes.saturating_sub(1);
        });

        // Update total subnet peers by subtracting  1
        TotalSubnetNodes::<T>::mutate(subnet_id, |n: &mut u32| n.saturating_dec());
        TotalNodes::<T>::mutate(|n: &mut u32| n.saturating_dec());

        Self::deposit_event(Event::SubnetNodeRemoved {
            subnet_id: subnet_id,
            subnet_node_id: subnet_node_id,
        });
    }

    pub fn perform_remove_subnet_node(subnet_id: u32, subnet_node_id: u32) {
        let mut is_active = false;
        let mut is_registered = false;
        let subnet_node = if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            is_active = true;
            SubnetNodesData::<T>::get(subnet_id, subnet_node_id)
        } else if RegisteredSubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
            is_registered = true;
            RegisteredSubnetNodesData::<T>::get(subnet_id, subnet_node_id)
        } else {
            return;
        };

        // The node can either be active or registered only
        if is_active {
            Self::remove_active_subnet_node(subnet_id, subnet_node_id);
        }

        if is_registered {
            Self::remove_registered_subnet_node(subnet_id, subnet_node_id);
        }
    }

    // pub fn get_subnet_node(subnet_id: u32, subnet_node_id: u32) -> Option<SubnetNodeV2> {
    //     if SubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
    //         Some(SubnetNodesData::<T>::get(subnet_id, subnet_node_id))
    //     } else if RegisteredSubnetNodesData::<T>::contains_key(subnet_id, subnet_node_id) {
    //         Some(RegisteredSubnetNodesData::<T>::get(
    //             subnet_id,
    //             subnet_node_id,
    //         ))
    //     } else {
    //         None
    //     }
    // }

    pub fn get_validator_classified_subnet_node(
        subnet_id: u32,
        subnet_node_id: u32,
        subnet_epoch: u32,
    ) -> Option<SubnetNodeV2> {
        if let Ok(subnet_node) = SubnetNodesData::<T>::try_get(subnet_id, subnet_node_id) {
            if subnet_node.has_classification(&SubnetNodeClass::Validator, subnet_epoch) {
                Some(subnet_node)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get subnet nodes by classification
    pub fn get_active_classified_subnet_nodes(
        subnet_id: u32,
        classification: &SubnetNodeClass,
        subnet_epoch: u32,
    ) -> Vec<SubnetNodeV2> {
        SubnetNodesData::<T>::iter_prefix_values(subnet_id)
            .filter(|subnet_node| subnet_node.has_classification(classification, subnet_epoch))
            .collect()
    }

    pub fn get_active_classified_subnet_nodes_v2(
        subnet_id: u32,
        classification: &SubnetNodeClass,
        subnet_epoch: u32,
    ) -> Vec<SubnetNodeV2> {
        SubnetNodesData::<T>::iter_prefix_values(subnet_id)
            .filter(|subnet_node| subnet_node.has_classification(classification, subnet_epoch))
            .collect()
    }

    pub fn get_classified_subnet_nodes_map(
        subnet_id: u32,
        classification: &SubnetNodeClass,
        subnet_epoch: u32,
    ) -> BTreeMap<u32, SubnetNodeV2> {
        SubnetNodesData::<T>::iter_prefix(subnet_id)
            .filter_map(|(subnet_node_id, subnet_node)| {
                if subnet_node.has_classification(classification, subnet_epoch) {
                    Some((subnet_node_id, subnet_node))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn get_classified_subnet_nodes_info(
        subnet_id: u32,
        classification: &SubnetNodeClass,
        subnet_epoch: u32,
    ) -> Vec<SubnetNodeInfoV2<T::AccountId>> {
        SubnetNodesData::<T>::iter_prefix(subnet_id)
            .filter(|(_subnet_node_id, subnet_node)| {
                subnet_node.has_classification(classification, subnet_epoch)
            })
            .map(|(subnet_node_id, subnet_node)| {
                let validator_id = SubnetNodeValidatorId::<T>::get(subnet_id, subnet_node_id);
                let coldkey = ValidatorColdkey::<T>::get(validator_id.unwrap()).unwrap();
                SubnetNodeInfoV2 {
                    validator_id: validator_id,
                    subnet_id: subnet_id,
                    subnet_node_id: subnet_node_id,
                    coldkey: coldkey,
                    hotkey: Self::get_subnet_node_associated_hotkey(subnet_id, subnet_node_id)
                        .unwrap(),
                    peer_info: subnet_node.peer_info,
                    bootnode_peer_info: subnet_node.bootnode_peer_info,
                    client_peer_info: subnet_node.client_peer_info,
                    classification: subnet_node.classification,
                    unique: subnet_node.unique,
                    non_unique: subnet_node.non_unique,
                    stake_balance: NodeSubnetStake::<T>::get(subnet_node_id, subnet_id),
                    subnet_node_reputation: SubnetNodeReputation::<T>::get(
                        subnet_id,
                        subnet_node_id,
                    ),
                    node_slot_index: NodeSlotIndex::<T>::get(subnet_id, subnet_node_id),
                    consecutive_idle_epochs: SubnetNodeIdleConsecutiveEpochs::<T>::get(
                        subnet_id,
                        subnet_node_id,
                    ),
                    consecutive_included_epochs: SubnetNodeConsecutiveIncludedEpochs::<T>::get(
                        subnet_id,
                        subnet_node_id,
                    ),
                }
            })
            .collect()
    }

    pub fn is_chosen_validator(subnet_id: u32, subnet_node_id: u32, subnet_epoch: u32) -> bool {
        match SubnetElectedValidator::<T>::try_get(subnet_id, subnet_epoch) {
            Ok(validator_subnet_node_id) => {
                let mut is_chosen_validator = false;
                if subnet_node_id == validator_subnet_node_id {
                    is_chosen_validator = true
                }
                is_chosen_validator
            }
            Err(()) => false,
        }
    }

    pub fn graduate_class(subnet_id: u32, subnet_node_id: u32, start_epoch: u32) -> bool {
        SubnetNodesData::<T>::try_mutate_exists(
            subnet_id,
            subnet_node_id,
            |maybe_node_data| -> Result<bool, ()> {
                if let Some(node_data) = maybe_node_data {
                    node_data.classification = SubnetNodeClassification {
                        node_class: node_data.classification.node_class.next(),
                        start_epoch,
                    };
                    Self::deposit_event(Event::NodeClassGraduation {
                        subnet_id,
                        subnet_node_id,
                        classification: node_data.classification.clone(),
                    });
                    Ok(true)
                } else {
                    Ok(false)
                }
            },
        )
        .unwrap_or(false)
    }

    /// Graduate subnet node to validator class and insert into election slot
    /// This function is called when a subnet node has reached the required reputation and included epochs
    /// to be graduated to validator class
    ///
    /// Returns true if the subnet node was graduated *AND* added to the election slots, false otherwise
    pub fn graduate_to_validator_class(
        subnet_id: u32,
        subnet_node_id: u32,
        start_epoch: u32,
    ) -> bool {
        SubnetNodeElectionSlots::<T>::try_mutate(subnet_id, |slot_list| -> Result<(), ()> {
            if slot_list.contains(&subnet_node_id) {
                return Err(());
            }

            SubnetNodesData::<T>::try_mutate_exists(
                subnet_id,
                subnet_node_id,
                |maybe_node_data| -> Result<(), ()> {
                    let node_data = maybe_node_data.as_mut().ok_or(())?;

                    node_data.classification = SubnetNodeClassification {
                        node_class: node_data.classification.node_class.next(),
                        start_epoch,
                    };
                    Self::deposit_event(Event::NodeClassGraduation {
                        subnet_id,
                        subnet_node_id,
                        classification: node_data.classification.clone(),
                    });

                    Ok(())
                },
            )?;

            let idx = slot_list.len() as u32;
            slot_list.push(subnet_node_id);
            NodeSlotIndex::<T>::insert(subnet_id, subnet_node_id, idx);
            TotalSubnetElectableNodes::<T>::mutate(subnet_id, |mut n| n.saturating_inc());
            TotalElectableNodes::<T>::mutate(|mut n| n.saturating_inc());

            Ok(())
        })
        .is_ok()
    }

    /// Check if subnet node is owner of a peer ID
    /// Main, bootnode, and client peer IDs must be unique so we check all of them to ensure
    /// that no one else owns them
    /// Returns True is no owner or the peer ID is ownerless and available
    pub fn is_owner_of_peer_or_ownerless(
        subnet_id: u32,
        subnet_node_id: u32,
        overwatch_node_id: u32,
        peer_id: &PeerId,
    ) -> bool {
        let mut is_peer_owner_or_ownerless =
            match PeerIdSubnetNodeId::<T>::try_get(subnet_id, peer_id) {
                Ok(peer_subnet_node_id) => {
                    if peer_subnet_node_id == subnet_node_id {
                        return true;
                    }
                    false
                }
                Err(()) => true,
            };

        is_peer_owner_or_ownerless = is_peer_owner_or_ownerless
            && match BootnodePeerIdSubnetNodeId::<T>::try_get(subnet_id, peer_id) {
                Ok(bootnode_subnet_node_id) => {
                    if bootnode_subnet_node_id == subnet_node_id {
                        return true;
                    }
                    false
                }
                Err(()) => true,
            };

        is_peer_owner_or_ownerless = is_peer_owner_or_ownerless
            && match ClientPeerIdSubnetNodeId::<T>::try_get(subnet_id, peer_id) {
                Ok(client_subnet_node_id) => {
                    if client_subnet_node_id == subnet_node_id {
                        return true;
                    }
                    false
                }
                Err(()) => true,
            };

        is_peer_owner_or_ownerless
            && match PeerIdOverwatchNodeId::<T>::try_get(subnet_id, peer_id) {
                Ok(peer_overwatch_node_id) => {
                    if peer_overwatch_node_id == overwatch_node_id {
                        return true;
                    }
                    false
                }
                Err(()) => true,
            }
    }

    pub fn is_owner_of_multiaddr_or_ownerless(
        subnet_id: u32,
        subnet_node_id: u32,
        multiaddr: BoundedVec<u8, DefaultMaxVectorLength>,
    ) -> bool {
        match MultiaddrSubnetNodeId::<T>::try_get(subnet_id, multiaddr) {
            Ok(node_id) => {
                if node_id == subnet_node_id {
                    return true;
                }
                false
            }
            Err(()) => true,
        }
    }

    pub fn clean_validator_subnet_nodes(validator_id: u32) {
        ValidatorSubnetNodes::<T>::mutate(validator_id, |map| {
            // Collect subnet_ids to remove (invalid subnets)
            let mut subnets_to_remove: Vec<u32> = map
                .keys()
                .filter(|&subnet_id| !Self::subnet_exists(*subnet_id))
                .copied()
                .collect();

            // Remove invalid subnets
            for subnet_id in &subnets_to_remove {
                map.remove(subnet_id);
            }
            // Note: We don't check for node IDs because this is handled in `perform_remove_subnet_node`
            // Why: ValidatorSubnetNodes is not cleaned when a subnet is removed
            // If a subnet itself is removed/deactivated, then this function will handle non-existing subnet IDs as keys
        });
    }

    /// Calculate current burn amount based on burn rate
    pub fn calculate_burn_amount(subnet_id: u32) -> u128 {
        let base_burn = Self::base_burn_amount();
        let burn_rate = CurrentNodeBurnRate::<T>::get(subnet_id);

        // Simple multiplication: burn_amount = base_burn * burn_rate
        // burn_rate is already a percentage in 1e18 format
        Self::percent_mul(base_burn, burn_rate)
    }

    /// Record a registration (increment counter)
    /// Called by `register_subnet_node`
    pub fn record_registration(subnet_id: u32) -> DispatchResult {
        let current_count = NodeRegistrationsThisEpoch::<T>::get(subnet_id);
        NodeRegistrationsThisEpoch::<T>::insert(subnet_id, current_count.saturating_add(1));
        Ok(())
    }

    /// Update burn rate based on registrations in previous epoch
    /// Note: This is only called if the subnet is active
    ///       The burn rate will stay constant if not active.
    pub fn update_burn_rate_for_epoch(weight_meter: &mut WeightMeter, subnet_id: u32) {
        let db_weight = T::DbWeight::get();

        // It's unlikely this will ever be true, but we check anyway to future-proof
        if !weight_meter.can_consume(db_weight.reads_writes(9, 2)) {
            return;
        }

        let registrations = NodeRegistrationsThisEpoch::<T>::get(subnet_id);
        let target = TargetNodeRegistrationsPerEpoch::<T>::get(subnet_id);
        let previous_burn_rate = CurrentNodeBurnRate::<T>::get(subnet_id);
        let alpha = NodeBurnRateAlpha::<T>::get(subnet_id);
        weight_meter.consume(db_weight.reads(5));

        // Calculate target burn rate based on registration activity
        let target_burn_rate = Self::calculate_target_burn_rate(registrations, target);
        // Maximum of two reads for `calculate_target_burn_rate`
        weight_meter.consume(db_weight.reads(2));

        // Rest of the function remains the same...
        let precision = Self::percentage_factor_as_u128();
        let one_minus_alpha = precision.saturating_sub(alpha);
        let alpha_component = Self::percent_mul(target_burn_rate, alpha);
        let previous_component = Self::percent_mul(previous_burn_rate, one_minus_alpha);
        let new_burn_rate = alpha_component.saturating_add(previous_component);

        // Apply min/max bounds to the rate
        let min_rate = MinNodeBurnRate::<T>::get();
        let max_rate = MaxNodeBurnRate::<T>::get();
        weight_meter.consume(db_weight.reads(2));

        let clamped_rate = new_burn_rate.max(min_rate).min(max_rate);
        weight_meter.consume(db_weight.writes(2));

        // Update current burn rate for next epoch
        CurrentNodeBurnRate::<T>::insert(subnet_id, clamped_rate);
        // Reset
        NodeRegistrationsThisEpoch::<T>::insert(subnet_id, 0);
    }

    fn calculate_target_burn_rate(registrations: u32, target: u32) -> u128 {
        if registrations == 0 {
            // No registrations -> use minimum rate
            MinNodeBurnRate::<T>::get()
        } else if registrations >= target {
            // At or above target -> use maximum rate
            MaxNodeBurnRate::<T>::get()
        } else {
            // Below target -> calculate proportional rate between min and max
            let min_rate = MinNodeBurnRate::<T>::get();
            let max_rate = MaxNodeBurnRate::<T>::get();
            let ratio = Self::percent_div(registrations as u128, target as u128);

            // Linear interpolation between min and max based on how close we are to target
            // rate = min + (max - min) * (registrations / target)
            let rate_range = max_rate.saturating_sub(min_rate);
            let rate_component = Self::percent_mul(rate_range, ratio);

            min_rate.saturating_add(rate_component)
        }
    }

    pub fn get_subnet_node_hotkey(subnet_id: u32, subnet_node_id: u32) -> Option<T::AccountId> {
        if let Some(hotkey) = SubnetNodeIdHotkey::<T>::get(subnet_id, subnet_node_id) {
            return Some(hotkey);
        }
        None
    }

    pub fn is_subnet_node_keys_owner(
        subnet_id: u32,
        subnet_node_id: u32,
        key: T::AccountId,
    ) -> bool {
        match Self::get_subnet_node_associated_coldkey_and_hotkey(subnet_id, subnet_node_id) {
            Ok((coldkey, hotkey)) => key == hotkey || key == coldkey,
            Err(_) => false,
        }
    }

    pub fn get_subnet_node_associated_coldkey_and_hotkey(
        subnet_id: u32,
        subnet_node_id: u32,
    ) -> Result<(T::AccountId, T::AccountId), DispatchError> {
        let validator_id = SubnetNodeValidatorId::<T>::try_get(subnet_id, subnet_node_id)
            .map_err(|_| Error::<T>::InvalidSubnetNodeId)?;

        let validator_coldkey = ValidatorColdkey::<T>::try_get(validator_id)
            .map_err(|_| Error::<T>::InvalidValidatorId)?;

        // An overwatch node-specific hotkey overrides the validator hotkey when present.
        if let Some(subnet_node_hotkey) = SubnetNodeIdHotkey::<T>::get(subnet_id, subnet_node_id) {
            return Ok((validator_coldkey, subnet_node_hotkey));
        }

        let validator_hotkey =
            ValidatorIdHotkey::<T>::get(validator_id).ok_or(Error::<T>::InvalidValidator)?;

        Ok((validator_coldkey, validator_hotkey))
    }

    /// Get a hotkeys associated subnet node.
    /// The first check is to see if the subnet node has a hotkey which overrides the validator hotkey.
    /// If there is no hotkey associated with the subnet node, then we check if the validator ID has a
    /// hotkey and if it matches the caller's hotkey.
    pub fn get_hotkey_associated_subnet_node(
        subnet_id: u32,
        subnet_node_id: u32,
        validator_id: u32,
        hotkey: T::AccountId,
    ) -> Result<u32, DispatchError> {
        let subnet_node = SubnetNodesData::<T>::try_get(subnet_id, subnet_node_id)
            .map_err(|_| Error::<T>::InvalidSubnetNodeId)?;

        ensure!(
            subnet_node.validator_id == validator_id,
            Error::<T>::InvalidValidator
        );

        // A subnet node-specific hotkey overrides the validator hotkey when present.
        if let Some(subnet_node_hotkey) = SubnetNodeIdHotkey::<T>::get(subnet_id, subnet_node_id) {
            ensure!(
                subnet_node_hotkey == hotkey,
                Error::<T>::InvalidHotkeySubnetNodeId
            );

            return Ok(subnet_node_id);
        }

        let validator_hotkey =
            ValidatorIdHotkey::<T>::get(validator_id).ok_or(Error::<T>::InvalidValidator)?;

        ensure!(validator_hotkey == hotkey, Error::<T>::InvalidValidator);

        Ok(subnet_node_id)
    }

    /// Get a hotkeys associated subnet node.
    /// The first check is to see if the subnet node has a hotkey which overrides the validator hotkey.
    /// If there is no hotkey associated with the subnet node, then we check if the validator ID has a
    /// hotkey and if it matches the caller's hotkey.
    pub fn get_subnet_node_associated_hotkey(
        subnet_id: u32,
        subnet_node_id: u32,
    ) -> Result<T::AccountId, DispatchError> {
        // A subnet node-specific hotkey overrides the validator hotkey when present.
        if let Some(subnet_node_hotkey) = SubnetNodeIdHotkey::<T>::get(subnet_id, subnet_node_id) {
            return Ok(subnet_node_hotkey);
        }

        let validator_id = SubnetNodeValidatorId::<T>::try_get(subnet_id, subnet_node_id)
            .map_err(|_| Error::<T>::InvalidSubnetNodeId)?;

        let validator_hotkey =
            ValidatorIdHotkey::<T>::get(validator_id).ok_or(Error::<T>::InvalidValidator)?;

        Ok(validator_hotkey)
    }

    /// Get the coldkey of the validator that owns the subnet node.
    pub fn get_subnet_node_associated_coldkey(
        subnet_id: u32,
        subnet_node_id: u32,
    ) -> Result<T::AccountId, DispatchError> {
        let validator_id = SubnetNodeValidatorId::<T>::try_get(subnet_id, subnet_node_id)
            .map_err(|_| Error::<T>::InvalidSubnetNodeId)?;

        let validator_coldkey = ValidatorColdkey::<T>::try_get(validator_id)
            .map_err(|_| Error::<T>::InvalidValidatorId)?;

        Ok(validator_coldkey)
    }

    pub fn get_subnet_node_associated_validator_info(
        subnet_id: u32,
        subnet_node_id: u32,
    ) -> Result<ValidatorInfo<T::AccountId>, DispatchError> {
        let validator_id = SubnetNodeValidatorId::<T>::try_get(subnet_id, subnet_node_id)
            .map_err(|_| Error::<T>::InvalidSubnetNodeId)?;

        let validator_coldkey = ValidatorColdkey::<T>::try_get(validator_id)
            .map_err(|_| Error::<T>::InvalidValidatorId)?;

        let data = ValidatorsData::<T>::try_get(validator_id)
            .map_err(|_| Error::<T>::InvalidValidatorId)?;

        let validator: ValidatorInfo<T::AccountId> = ValidatorInfo {
            id: validator_id,
            hotkey: data.hotkey,
            delegate_reward_rate: data.delegate_reward_rate,
            last_delegate_reward_rate_update: data.last_delegate_reward_rate_update,
            delegate_account: data.delegate_account,
            identity: data.identity,
            coldkey: Some(validator_coldkey),
        };

        Ok(validator)
    }
}
