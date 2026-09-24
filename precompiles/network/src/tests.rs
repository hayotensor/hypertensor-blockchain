use super::*;
use pallet_revive::precompiles::alloy::sol_types::SolInterface;
use std::collections::BTreeSet;

#[test]
fn manifest_covers_every_native_extrinsic_and_only_signed_calls_have_selectors() {
    let entries: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../coverage.json")).unwrap();
    let source = include_str!("../../../pallets/network/src/lib.rs");
    let native: BTreeSet<_> = source
        .split("#[pallet::call_index(")
        .skip(1)
        .map(|part| part.split(')').next().unwrap().parse::<u64>().unwrap())
        .collect();
    assert_eq!(
        native,
        entries
            .iter()
            .map(|c| c["index"].as_u64().unwrap())
            .collect()
    );
    assert_eq!(entries.len(), 161);
    let mut implemented = 0;
    for e in &entries {
        let marker = format!("#[pallet::call_index({})]", e["index"]);
        let body = source
            .split(&marker)
            .nth(1)
            .unwrap()
            .split("#[pallet::call_index(")
            .next()
            .unwrap();
        let signed = !body.contains("CollectiveOrigin::ensure_origin");
        assert_eq!(signed, e["status"] == "implemented", "{}", e["name"]);
        if signed {
            implemented += 1;
            let hash = pallet_revive::keccak_256(e["signature"].as_str().unwrap().as_bytes());
            let selector: [u8; 4] = hash[..4].try_into().unwrap();
            let valid = match e["domain"].as_str().unwrap() {
                "Staking" => abi::IStaking::IStakingCalls::valid_selector(selector),
                "Validators" => abi::IValidators::IValidatorsCalls::valid_selector(selector),
                "Subnets" => abi::ISubnets::ISubnetsCalls::valid_selector(selector),
                "SubnetNodes" => abi::ISubnetNodes::ISubnetNodesCalls::valid_selector(selector),
                "Consensus" => abi::IConsensus::IConsensusCalls::valid_selector(selector),
                "Overwatch" => abi::IOverwatch::IOverwatchCalls::valid_selector(selector),
                _ => false,
            };
            assert!(valid, "manifest signature not in ABI: {}", e["name"]);
        } else {
            assert_eq!(e["domain"], "Governance");
            assert!(e.get("signature").is_none());
        }
    }
    assert_eq!(implemented, 75);
}

#[test]
fn address_constants_match_solidity_and_do_not_collide() {
    let sol = include_str!("../interfaces/NetworkAddresses.sol");
    let ids = [
        addresses::STAKING,
        addresses::VALIDATORS,
        addresses::SUBNETS,
        addresses::SUBNET_NODES,
        addresses::CONSENSUS,
        addresses::OVERWATCH,
        addresses::GOVERNANCE,
    ];
    let mut unique = BTreeSet::new();
    for id in ids {
        assert!(unique.insert(addresses::address(id)));
        assert!(sol.contains(&format!("address(0x{:x})", u32::from(id) << 16)));
        assert_eq!(&addresses::address(id)[18..], &[0, 0]);
        assert_eq!(&addresses::address(id)[16..18], &id.to_be_bytes());
    }
}

#[test]
fn options_preserve_zero_and_native_account_bytes() {
    let value = abi::OptionalAccount {
        present: true,
        value: [0u8; 32].into(),
    };
    assert_eq!(
        convert::optional_account(&value),
        Some(sp_runtime::AccountId32::new([0; 32]))
    );
    assert_eq!(
        convert::optional_u128(&abi::OptionalU128 {
            present: true,
            value: 0
        }),
        Some(0)
    );
    assert_eq!(
        convert::optional_u32(&abi::OptionalU32 {
            present: false,
            value: 99
        }),
        None
    );
    let native = sp_runtime::AccountId32::new([0xab; 32]);
    assert_eq!(
        convert::account(&convert::account_out(native.clone())),
        native
    );
}

#[test]
fn invalid_swap_tags_and_oversized_peers_are_rejected() {
    let swap = abi::QueuedSwap {
        kind: 2,
        ..Default::default()
    };
    assert!(convert::queued_swap(&swap).is_err());
    assert!(convert::peer_id(&[0; 129]).is_err());
}

#[test]
fn abi_envelope_defers_validation_and_rejects_overlapping_dynamic_tails() {
    use pallet_revive::precompiles::alloy::sol_types::SolCall;
    type Calls = abi::ISubnets::ISubnetsCalls;
    type Input = input::MeteredInput<Calls>;
    let mut wire = abi::ISubnets::updateBootnodesCall {
        subnetId: 1,
        add: vec![],
        remove: vec![vec![1; 8].into(), vec![2; 8].into()],
    }
    .abi_encode();
    assert!(Input::abi_decode_validate(&wire)
        .unwrap()
        .decode_charged()
        .is_ok());
    // Alias the second bytes element to the first. The loose decoder accepts this, but doing
    // so repeatedly for large tails permits work far in excess of the actual calldata length.
    let remove_offset = u32::from_be_bytes(wire[96..100].try_into().unwrap()) as usize;
    let first = 4 + remove_offset + 32;
    let offset = wire[first..first + 32].to_vec();
    wire[first + 32..first + 64].copy_from_slice(&offset);
    assert!(Calls::abi_decode_validate(&wire).is_ok());
    assert_eq!(
        Input::abi_decode_validate(&wire)
            .unwrap()
            .decode_charged()
            .unwrap_err(),
        convert::revert("Network: malformed ABI")
    );
    // Even a missing selector is transported to the metered entry point before validation.
    assert_eq!(
        Input::abi_decode_validate(&[])
            .unwrap()
            .decode_charged()
            .unwrap_err(),
        convert::revert("Network: malformed ABI")
    );
}

#[test]
fn abi_parser_rejects_trailing_bytes_and_impossible_array_lengths() {
    use pallet_revive::precompiles::alloy::sol_types::SolCall;
    type Input = input::MeteredInput<abi::ISubnets::ISubnetsCalls>;
    let mut wire = abi::ISubnets::ownerRemoveInitialValidatorsCall {
        subnetId: 1,
        validators: vec![7],
    }
    .abi_encode();
    assert!(Input::abi_decode_validate(&wire)
        .unwrap()
        .decode_charged()
        .is_ok());
    wire.push(0);
    assert!(Input::abi_decode_validate(&wire)
        .unwrap()
        .decode_charged()
        .is_err());
    wire.pop();
    wire[68..100].fill(0xff);
    assert!(Input::abi_decode_validate(&wire)
        .unwrap()
        .decode_charged()
        .is_err());
}
