use crate::{npos, opaque::SessionKeys, AccountId, Balance, Signature};
use alloc::{format, vec, vec::Vec};
use serde_json::Value;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{sr25519, Pair, Public};
use sp_genesis_builder::{self, PresetId};
use sp_runtime::traits::{IdentifyAccount, Verify};

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Generate an account ID from seed.
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate native validator accounts and their separate consensus keys.
pub fn authority_keys_from_seed(seed: &str) -> (AccountId, BabeId, GrandpaId) {
    (
        get_account_id_from_seed::<sr25519::Public>(seed),
        get_from_seed::<BabeId>(seed),
        get_from_seed::<GrandpaId>(seed),
    )
}

fn consensus_genesis(initial_authorities: &[(AccountId, BabeId, GrandpaId)]) -> Value {
    serde_json::json!({
        "session": {
            "keys": initial_authorities.iter().map(|(account, babe, grandpa)| (
                account, account, SessionKeys { babe: babe.clone(), grandpa: grandpa.clone() }
            )).collect::<Vec<_>>()
        },
        "staking": {
            "validatorCount": initial_authorities.len() as u32,
            "minimumValidatorCount": npos::MIN_VALIDATOR_COUNT,
            "stakers": initial_authorities.iter().map(|(account, _, _)| (
                account, account, npos::VALIDATOR_BOND, pallet_staking::StakerStatus::<AccountId>::Validator
            )).collect::<Vec<_>>(),
            "invulnerables": [],
            "minValidatorBond": npos::VALIDATOR_BOND,
            "minNominatorBond": npos::MIN_NOMINATOR_BOND,
            "maxValidatorCount": npos::MAX_VALIDATORS,
            "maxNominatorCount": npos::MAX_NOMINATORS,
            "slashRewardFraction": sp_runtime::Perbill::from_percent(10),
        },
        "babe": { "epochConfig": npos::BABE_GENESIS_EPOCH_CONFIG },
    })
}

const UNITS: Balance = 1_000_000_000_000_000_000;

fn testnet_genesis(authority_seeds: &[&str]) -> Value {
    let endowed_accounts: Vec<AccountId> = ["Alice", "Bob", "Charlie", "Dave", "Eve", "Ferdie"]
        .iter()
        .flat_map(|seed| {
            [
                get_account_id_from_seed::<sr25519::Public>(seed),
                get_account_id_from_seed::<sr25519::Public>(&format!("{seed}//stash")),
            ]
        })
        .collect();
    let authorities = authority_seeds
        .iter()
        .map(|seed| authority_keys_from_seed(seed))
        .collect::<Vec<_>>();
    let mut genesis = serde_json::json!({
        "sudo": { "key": Some(get_account_id_from_seed::<sr25519::Public>("Alice")) },
        "balances": {
            "balances": endowed_accounts.iter().cloned().map(|account| (account, 1_000_000 * UNITS)).collect::<Vec<_>>()
        },
    });
    genesis
        .as_object_mut()
        .unwrap()
        .extend(consensus_genesis(&authorities).as_object().unwrap().clone());
    genesis
}

pub fn development_config_genesis() -> Value {
    testnet_genesis(&["Alice"])
}

pub fn local_config_genesis() -> Value {
    testnet_genesis(&["Alice", "Bob"])
}

#[cfg(test)]
pub fn four_validator_test_genesis() -> Value {
    testnet_genesis(&["Alice", "Bob", "Charlie", "Dave"])
}

pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
    let patch = match id.as_ref() {
        sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
        sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => local_config_genesis(),
        _ => return None,
    };
    Some(serde_json::to_vec(&patch).expect("genesis preset serializes"))
}

pub fn preset_names() -> Vec<PresetId> {
    vec![
        PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET),
        PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET),
    ]
}
