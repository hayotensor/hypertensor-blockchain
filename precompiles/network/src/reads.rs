//! Bounded views. Charge the entire storage/encoding envelope before the first read.
use crate::{abi, convert, weights::WeightInfo, Config};
use alloc::vec::Vec;
use frame_support::traits::Get;
use network_rpc_types as rpc;
use pallet_network as n;
use pallet_revive::precompiles::{alloy::sol_types::SolValue, Error, Ext};

fn charge<T: Config>(env: &mut impl Ext<T = T>, reads: u32, bytes: u32) -> Result<(), Error> {
    env.charge(T::PrecompileWeightInfo::read(reads, bytes))?;
    Ok(())
}
fn unknown() -> Error {
    convert::revert("Network: unknown selector")
}
fn bytes_out(value: Option<rpc::RpcBytes>) -> abi::OptionalBytes {
    abi::OptionalBytes {
        present: value.is_some(),
        value: value.map(|v| v.0.into()).unwrap_or_default(),
    }
}
fn peer_out(value: Option<rpc::PeerInfo>) -> abi::OptionalPeerInfo {
    abi::OptionalPeerInfo {
        present: value.is_some(),
        value: value
            .map(|v| abi::PeerInfo {
                peerId: v.peer_id.0.into(),
                multiaddr: bytes_out(v.multiaddr),
            })
            .unwrap_or_default(),
    }
}

mod staking;
pub use staking::staking;

fn validator_out(value: Option<rpc::ValidatorInfo<sp_runtime::AccountId32>>) -> Vec<u8> {
    let exists = value.is_some();
    let info = value
        .map(|v| {
            let delegate_account = abi::OptionalDelegateAccount {
                present: v.delegate_account.is_some(),
                value: v
                    .delegate_account
                    .map(|d| abi::DelegateAccount {
                        accountId: convert::account_out(d.account_id),
                        rate: d.rate.0,
                    })
                    .unwrap_or_default(),
            };
            let identity = identity_out(v.identity);
            abi::ValidatorInfo {
                id: v.id,
                coldkey: convert::account_out(v.coldkey),
                hotkey: convert::account_out(v.hotkey),
                delegateRewardRate: v.delegate_reward_rate.0,
                delegateAccount: delegate_account,
                identity,
                poolBalance: v.delegate_pool_balance.0,
                poolShares: v.delegate_pool_shares.0,
                slashLockUntil: v.delegate_pool_slash_lock_until,
            }
        })
        .unwrap_or_default();
    (exists, info).abi_encode_params()
}
mod validators;
pub use validators::validators;

mod subnets;
pub use subnets::subnets;

mod subnet_nodes;
pub use subnet_nodes::subnet_nodes;

mod consensus;
pub use consensus::consensus;

mod overwatch;
pub use overwatch::overwatch;

fn identity_out(v: Option<rpc::IdentityInfo>) -> abi::OptionalIdentity {
    abi::OptionalIdentity {
        present: v.is_some(),
        value: v
            .map(|v| abi::Identity {
                name: bytes_out(v.name),
                url: bytes_out(v.url),
                image: bytes_out(v.image),
                discord: bytes_out(v.discord),
                x: bytes_out(v.x),
                telegram: bytes_out(v.telegram),
                github: bytes_out(v.github),
                huggingFace: bytes_out(v.hugging_face),
                description: bytes_out(v.description),
                misc: bytes_out(v.misc),
            })
            .unwrap_or_default(),
    }
}
