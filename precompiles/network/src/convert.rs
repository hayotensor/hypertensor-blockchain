//! Checked ABI conversions. No authorization decisions are made here.
use crate::{abi, Config};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};
use frame_support::{traits::Get, BoundedVec};
use pallet_revive::precompiles::{alloy::primitives::B256, Error};
use sp_core::{OpaquePeerId, H256};
use sp_runtime::AccountId32;

pub fn revert(message: &'static str) -> Error {
    Error::Revert(message.into())
}
pub fn limit(len: usize, max: u32) -> Result<(), Error> {
    if len > max as usize {
        Err(revert("Network: collection limit exceeded"))
    } else {
        Ok(())
    }
}
pub fn account(value: &B256) -> AccountId32 {
    AccountId32::new(value.0)
}
pub fn account_out(value: AccountId32) -> B256 {
    B256::from(<[u8; 32]>::from(value))
}
pub fn optional_account(value: &abi::OptionalAccount) -> Option<AccountId32> {
    value.present.then(|| account(&value.value))
}
pub fn optional_u128(value: &abi::OptionalU128) -> Option<u128> {
    value.present.then_some(value.value)
}
pub fn optional_u32(value: &abi::OptionalU32) -> Option<u32> {
    value.present.then_some(value.value)
}
pub fn account_option_out(value: Option<AccountId32>) -> abi::OptionalAccount {
    abi::OptionalAccount {
        present: value.is_some(),
        value: value.map(account_out).unwrap_or_default(),
    }
}
pub fn u32_option_out(value: Option<u32>) -> abi::OptionalU32 {
    abi::OptionalU32 {
        present: value.is_some(),
        value: value.unwrap_or_default(),
    }
}
pub fn metadata<T: Config>(value: &[u8]) -> Result<Vec<u8>, Error> {
    limit(value.len(), T::MaxVectorLength::get())?;
    Ok(value.to_vec())
}
pub fn url<T: Config>(value: &[u8]) -> Result<Vec<u8>, Error> {
    limit(value.len(), T::MaxUrlLength::get())?;
    Ok(value.to_vec())
}
pub fn bounded<S: Get<u32>>(value: &[u8]) -> Result<BoundedVec<u8, S>, Error> {
    limit(value.len(), S::get())?;
    value
        .to_vec()
        .try_into()
        .map_err(|_| revert("Network: byte limit exceeded"))
}
fn optional_bytes<S: Get<u32>>(
    value: &abi::OptionalBytes,
) -> Result<Option<BoundedVec<u8, S>>, Error> {
    // Bound unused payloads too: absent options cannot carry unmetered allocations.
    limit(value.value.len(), S::get())?;
    value
        .present
        .then(|| bounded::<S>(&value.value))
        .transpose()
}
pub fn network_bytes<T: Config>(
    v: &abi::OptionalBytes,
) -> Result<Option<pallet_network::NetworkBytes<T>>, Error> {
    optional_bytes(&v)
}
pub fn validator_args<T: Config>(
    v: &abi::OptionalBytes,
) -> Result<Option<pallet_network::ValidatorArgs<T>>, Error> {
    optional_bytes(&v)
}
pub fn delegate_account(
    v: &abi::OptionalDelegateAccount,
) -> Option<pallet_network::DelegateAccount<AccountId32>> {
    v.present.then(|| pallet_network::DelegateAccount {
        account_id: account(&v.value.accountId),
        rate: v.value.rate,
    })
}
pub fn peer_id(v: &[u8]) -> Result<OpaquePeerId, Error> {
    // The network validator accepts only multihash peer IDs up to 128 bytes.
    limit(v.len(), 128)?;
    Ok(OpaquePeerId(v.to_vec()))
}
pub fn peer_info<T: Config>(
    v: &abi::OptionalPeerInfo,
) -> Result<Option<pallet_network::PeerInfo<T>>, Error> {
    let peer_id = peer_id(&v.value.peerId)?;
    let multiaddr = network_bytes::<T>(&v.value.multiaddr)?;
    Ok(v.present
        .then_some(pallet_network::PeerInfo { peer_id, multiaddr }))
}
pub fn validators<T: Config>(v: &[abi::U32Entry]) -> Result<BTreeMap<u32, u32>, Error> {
    limit(v.len(), T::MaxRegisteredNodesUpperBound::get())?;
    let mut out = BTreeMap::new();
    for e in v {
        if out.insert(e.key, e.value).is_some() {
            return Err(revert("Network: duplicate map key"));
        }
    }
    Ok(out)
}
pub fn validator_set<T: Config>(v: &[u32]) -> Result<BTreeSet<u32>, Error> {
    limit(v.len(), T::MaxRegisteredNodesUpperBound::get())?;
    let out: BTreeSet<_> = v.iter().copied().collect();
    if out.len() != v.len() {
        return Err(revert("Network: duplicate set entry"));
    }
    Ok(out)
}
pub fn emergency_set<T: Config>(v: &[u32]) -> Result<Vec<u32>, Error> {
    limit(v.len(), T::MaxEmergencySubnetNodesUpperBound::get())?;
    Ok(v.to_vec())
}
pub fn bootnodes<T: Config>(
    v: &[abi::Bootnode],
) -> Result<BTreeMap<OpaquePeerId, pallet_network::NetworkBytes<T>>, Error> {
    limit(v.len(), T::MaxBootnodesUpperBound::get())?;
    let mut out = BTreeMap::new();
    for e in v {
        if out
            .insert(peer_id(&e.peerId)?, bounded(&e.multiaddr)?)
            .is_some()
        {
            return Err(revert("Network: duplicate map key"));
        }
    }
    Ok(out)
}
pub fn peer_set<T: Config>(
    v: &[pallet_revive::precompiles::alloy::primitives::Bytes],
) -> Result<BTreeSet<OpaquePeerId>, Error> {
    limit(v.len(), T::MaxBootnodesUpperBound::get())?;
    let mut out = BTreeSet::new();
    for e in v {
        if !out.insert(peer_id(e)?) {
            return Err(revert("Network: duplicate set entry"));
        }
    }
    Ok(out)
}
pub fn registration<T: Config>(
    v: &abi::SubnetRegistration,
) -> Result<pallet_network::RegistrationSubnetData<T>, Error> {
    Ok(pallet_network::RegistrationSubnetData {
        name: metadata::<T>(&v.name)?,
        repo: url::<T>(&v.repo)?,
        description: metadata::<T>(&v.description)?,
        misc: metadata::<T>(&v.misc)?,
        min_stake: v.minStake,
        max_stake: v.maxStake,
        delegate_stake_percentage: v.delegateStakePercentage,
        initial_validators: validators::<T>(&v.initialValidators)?,
        bootnodes: bootnodes::<T>(&v.bootnodes)?,
    })
}
pub fn queued_swap(
    v: &abi::QueuedSwap,
) -> Result<pallet_network::QueuedSwapCall<AccountId32>, Error> {
    let account_id = account(&v.accountId);
    Ok(match v.kind {
        0 => pallet_network::QueuedSwapCall::SwapToSubnetDelegateStake {
            account_id,
            to_subnet_id: v.destinationId,
            balance: v.balance,
            min_shares_out: v.minSharesOut,
            execute_before_block: v.executeBeforeBlock,
        },
        1 => pallet_network::QueuedSwapCall::SwapToValidatorDelegateStake {
            account_id,
            to_validator_id: v.destinationId,
            balance: v.balance,
            min_shares_out: v.minSharesOut,
            execute_before_block: v.executeBeforeBlock,
        },
        _ => return Err(revert("Network: invalid swap kind")),
    })
}
pub fn queued_swap_out(v: pallet_network::QueuedSwapCall<AccountId32>) -> abi::QueuedSwap {
    match v {
        pallet_network::QueuedSwapCall::SwapToSubnetDelegateStake {
            account_id,
            to_subnet_id,
            balance,
            min_shares_out,
            execute_before_block,
        } => abi::QueuedSwap {
            kind: 0,
            accountId: account_out(account_id),
            destinationId: to_subnet_id,
            balance,
            minSharesOut: min_shares_out,
            executeBeforeBlock: execute_before_block,
        },
        pallet_network::QueuedSwapCall::SwapToValidatorDelegateStake {
            account_id,
            to_validator_id,
            balance,
            min_shares_out,
            execute_before_block,
        } => abi::QueuedSwap {
            kind: 1,
            accountId: account_out(account_id),
            destinationId: to_validator_id,
            balance,
            minSharesOut: min_shares_out,
            executeBeforeBlock: execute_before_block,
        },
    }
}
pub fn scores<T: Config>(
    v: &[abi::NodeScore],
) -> Result<Vec<pallet_network::SubnetNodeConsensusData>, Error> {
    limit(v.len(), T::MaxSubnetNodesUpperBound::get())?;
    Ok(v.iter()
        .map(|e| pallet_network::SubnetNodeConsensusData {
            subnet_node_id: e.subnetNodeId,
            score: e.score,
        })
        .collect())
}
pub fn commits<T: Config>(
    v: &[abi::OverwatchCommit],
) -> Result<Vec<pallet_network::OverwatchCommit<H256>>, Error> {
    limit(v.len(), T::MaxPhysicalSubnetsUpperBound::get())?;
    Ok(v.iter()
        .map(|e| pallet_network::OverwatchCommit {
            subnet_id: e.subnetId,
            weight: H256(e.weight.0),
        })
        .collect())
}
pub fn reveals<T: Config>(
    v: &[abi::OverwatchReveal],
) -> Result<Vec<pallet_network::OverwatchReveal<T>>, Error> {
    limit(v.len(), T::MaxPhysicalSubnetsUpperBound::get())?;
    v.iter()
        .map(|e| {
            Ok(pallet_network::OverwatchReveal {
                subnet_id: e.subnetId,
                weight: e.weight,
                salt: bounded(&e.salt)?,
            })
        })
        .collect()
}
pub fn allocations<T: Config>(v: &[abi::NodeAllocation]) -> Result<Vec<(u32, u32, u128)>, Error> {
    limit(v.len(), T::MaxValidatorNodesUpperBound::get())?;
    Ok(v.iter()
        .map(|e| (e.subnetId, e.subnetNodeId, e.weight))
        .collect())
}

pub fn identity<T: Config>(
    v: &abi::OptionalIdentity,
) -> Result<Option<pallet_network::IdentityData<T>>, Error> {
    let value = pallet_network::IdentityData {
        name: optional_bytes::<T::MaxVectorLength>(&v.value.name)?,
        url: optional_bytes::<T::MaxUrlLength>(&v.value.url)?,
        image: optional_bytes::<T::MaxUrlLength>(&v.value.image)?,
        discord: optional_bytes::<T::MaxSocialIdLength>(&v.value.discord)?,
        x: optional_bytes::<T::MaxSocialIdLength>(&v.value.x)?,
        telegram: optional_bytes::<T::MaxSocialIdLength>(&v.value.telegram)?,
        github: optional_bytes::<T::MaxUrlLength>(&v.value.github)?,
        hugging_face: optional_bytes::<T::MaxUrlLength>(&v.value.huggingFace)?,
        description: optional_bytes::<T::MaxVectorLength>(&v.value.description)?,
        misc: optional_bytes::<T::MaxVectorLength>(&v.value.misc)?,
    };
    Ok(v.present.then_some(value))
}
pub fn reputation(v: &abi::ReputationUpdates) -> pallet_network::SubnetReputationFactorUpdates {
    pallet_network::SubnetReputationFactorUpdates {
        absent_decrease: optional_u128(&v.absentDecrease),
        included_increase: optional_u128(&v.includedIncrease),
        below_min_weight_decrease: optional_u128(&v.belowMinWeightDecrease),
        non_attestor_decrease: optional_u128(&v.nonAttestorDecrease),
        non_consensus_attestor_decrease: optional_u128(&v.nonConsensusAttestorDecrease),
        validator_absent_decrease: optional_u128(&v.validatorAbsentDecrease),
        validator_non_consensus_decrease: optional_u128(&v.validatorNonConsensusDecrease),
    }
}
