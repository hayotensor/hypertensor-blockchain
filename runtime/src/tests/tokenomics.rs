use crate::configs::revive;
use crate::tokenomics::*;
use crate::*;
use frame_support::weights::WeightToFee as WeightToFeeT;
use sp_staking::currency_to_vote::{CurrencyToVote, U128CurrencyToVote};

#[test]
fn small_and_large_supplies_preserve_affordability_and_voting_power() {
    for supply in [
        TENSOR,
        10 * TENSOR,
        50_000_000 * TENSOR,
        1_000_000_000 * TENSOR,
    ] {
        let policy = Tokenomics::for_supply(supply);
        policy.network.validate();
        assert!(policy.transaction_byte_fee > 0);
        assert_eq!(policy.transaction_byte_fee * 50_000_000_000_000, supply);
        assert_eq!(policy.validator_bond * 50_000, supply);
        assert_eq!(policy.network.initial_annual_emissions * 500, supply);
        assert_eq!(policy.consensus_annual_emissions * 50_000, supply);
        assert!(policy.storage_child_item_deposit > 0);
        assert!(policy.storage_byte_deposit > 0);
        // Even at 100 times launch issuance, a minimum nomination must retain voting power.
        assert!(U128CurrencyToVote::to_vote(policy.nominator_bond, supply * 100) > 0);
    }
    let tiny = Tokenomics::for_supply(10 * TENSOR);
    assert_eq!(tiny.transaction_byte_fee, 200_000);
    assert_eq!(tiny.validator_bond, 200_000_000_000_000);
}

#[test]
fn sdk_weight_fees_scale_without_truncating_small_supply_prices_to_zero() {
    type SmallFees = pallet_revive::evm::fees::BlockRatioFee<
        { 10 * TENSOR },
        WEIGHT_FEE_DENOMINATOR,
        Runtime,
        Balance,
    >;
    type LargeFees = pallet_revive::evm::fees::BlockRatioFee<
        { 50_000_000 * TENSOR },
        WEIGHT_FEE_DENOMINATOR,
        Runtime,
        Balance,
    >;
    let compute = Weight::from_parts(MAXIMUM_BLOCK_WEIGHT.ref_time(), 0);
    let proof = Weight::from_parts(0, MAXIMUM_BLOCK_WEIGHT.proof_size());
    for weight in [compute, proof, MAXIMUM_BLOCK_WEIGHT] {
        let small = SmallFees::weight_to_fee(&weight);
        let large = LargeFees::weight_to_fee(&weight);
        // Proof coefficient fixed-point rounding can lose one atomic unit.
        assert!(small.abs_diff(400_000_000) <= 1);
        assert!(large.abs_diff(2_000_000_000_000_000) <= 1);
    }
    assert!(SmallFees::weight_to_fee(&Weight::from_parts(1_000_000, 0)) > 0);
}

#[test]
fn runtime_consumers_use_the_central_policy() {
    crate::tests::npos::ext().execute_with(|| {
        assert_eq!(Balances::total_issuance(), STARTING_SUPPLY);
        assert_eq!(TransactionByteFee::get(), POLICY.transaction_byte_fee);
        assert_eq!(VALIDATOR_BOND, POLICY.validator_bond);
        assert_eq!(MIN_NOMINATOR_BOND, POLICY.nominator_bond);
        assert_eq!(SessionKeyDeposit::get(), POLICY.session_key_deposit);
        assert_eq!(revive::DepositPerItem::get(), POLICY.storage_item_deposit);
        assert_eq!(revive::DepositPerByte::get(), POLICY.storage_byte_deposit);
        assert_eq!(
            revive::DepositPerChildTrieItem::get(),
            POLICY.storage_child_item_deposit
        );
        assert_eq!(
            pallet_network::MinSubnetMinStake::<Runtime>::get(),
            POLICY.network.subnet_min_stake
        );
        assert_eq!(
            pallet_network::MinSubnetDelegateStakeBalance::<Runtime>::get(),
            POLICY.network.min_subnet_delegate_stake
        );
        assert_eq!(
            pallet_network::BaseValidatorReward::<Runtime>::get(),
            POLICY.network.base_validator_reward
        );
        assert_eq!(
            pallet_network::BaseNodeBurnAmount::<Runtime>::get(),
            POLICY.network.base_node_burn
        );
        assert_eq!(
            pallet_network::MinRegistrationCost::<Runtime>::get(),
            POLICY.network.min_registration_cost
        );
        assert_eq!(
            Network::get_inflation(0),
            POLICY.network.initial_annual_emissions
        );
        assert_eq!(
            Network::get_inflation(3 * EPOCHS_PER_YEAR),
            POLICY.network.terminal_annual_emissions
        );
        let (subnets, foundation) = Network::get_epoch_emissions(0);
        assert_eq!(
            foundation,
            fraction(
                POLICY.network.initial_annual_emissions,
                FOUNDATION_SHARE_PERCENT as u128,
                100
            ) / EPOCHS_PER_YEAR as u128
        );
        assert!(
            subnets + foundation
                <= POLICY.network.initial_annual_emissions / EPOCHS_PER_YEAR as u128
        );
        assert_eq!(
            OVERWATCH_EPOCH_EMISSIONS,
            POLICY.overwatch_annual_emissions / EPOCHS_PER_YEAR as u128
        );
        let (consensus, remainder) =
            <StakingEraPayout as pallet_staking::EraPayout<Balance>>::era_payout(
                0,
                STARTING_SUPPLY,
                365 * 24 * 60 * 60 * 1_000,
            );
        assert_eq!(
            (consensus, remainder),
            (POLICY.consensus_annual_emissions, 0)
        );
    });
}

fn genesis(preset: serde_json::Value) -> RuntimeGenesisConfig {
    let mut base = serde_json::to_value(RuntimeGenesisConfig::default()).unwrap();
    for (key, value) in preset.as_object().unwrap() {
        base[key]
            .as_object_mut()
            .unwrap()
            .extend(value.as_object().unwrap().clone());
    }
    serde_json::from_value(base).unwrap()
}

#[test]
fn genesis_api_checks_the_exact_premine_before_writing_storage() {
    for preset in [
        genesis::presets::development_config_genesis(),
        genesis::presets::local_config_genesis(),
    ] {
        let mut config = genesis(preset);
        validate_genesis(&config).unwrap();
        frame_support::__private::TestExternalities::default().execute_with(|| {
            build_genesis_state(serde_json::to_vec(&config).unwrap()).unwrap();
            assert_eq!(Balances::total_issuance(), STARTING_SUPPLY);
        });
        config.balances.balances[0].1 += 1;
        frame_support::__private::TestExternalities::default().execute_with(|| {
            let error = build_genesis_state(serde_json::to_vec(&config).unwrap()).unwrap_err();
            assert!(error.contains("must equal tokenomics starting supply"));
            assert_eq!(Balances::total_issuance(), 0);
        });
        config.balances.balances[0].1 = u128::MAX;
        assert!(validate_genesis(&config).unwrap_err().contains("overflow"));
    }
}

#[test]
fn annual_fraction_is_safe_at_the_balance_limit() {
    assert_eq!(fraction(u128::MAX, 1, 1), u128::MAX);
    assert_eq!(fraction(u128::MAX, 2_000, 1_000_000), u128::MAX / 500);
}

#[test]
#[should_panic(expected = "starting supply must be at least one token")]
fn unsupported_supply_is_rejected_instead_of_creating_zero_prices() {
    Tokenomics::for_supply(0);
}

#[test]
fn genesis_requires_pallet_endowments_and_configured_staking_minima() {
    let mut config = genesis(genesis::presets::local_config_genesis());
    let index = config
        .balances
        .balances
        .iter()
        .position(|(who, _)| who == &Treasury::account_id())
        .unwrap();
    let (_, removed) = config.balances.balances.remove(index);
    config.balances.balances[0].1 += removed;
    assert!(validate_genesis(&config)
        .unwrap_err()
        .contains("existential deposits"));
    config.balances.balances[0].1 -= removed;
    config
        .balances
        .balances
        .push((Treasury::account_id(), removed));
    config.staking.min_validator_bond += 1;
    assert!(validate_genesis(&config)
        .unwrap_err()
        .contains("minimum bonds"));
}

#[test]
fn extra_ethereum_genesis_funding_is_rejected_and_rolled_back() {
    let mut config = genesis(genesis::presets::local_config_genesis());
    config
        .revive
        .accounts
        .push(pallet_revive::genesis::Account::<Runtime> {
            address: sp_core::H160::repeat_byte(0x99),
            balance: (7 * TENSOR).into(),
            nonce: 0,
            contract_data: None,
        });
    validate_genesis(&config).unwrap();
    frame_support::__private::TestExternalities::default().execute_with(|| {
        let error = build_genesis_state(serde_json::to_vec(&config).unwrap()).unwrap_err();
        assert!(error.contains("pallet genesis changed issuance"));
        assert_eq!(Balances::total_issuance(), 0);
        assert_eq!(Balances::free_balance(&config.balances.balances[0].0), 0);
    });
}
