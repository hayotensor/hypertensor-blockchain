use crate::{AccountId, ProxyType, Runtime, RuntimeCall};
use frame_support::traits::InstanceFilter;
use sp_core::H256;

fn account(id: u64) -> AccountId {
    sp_core::H256::from_low_u64_be(id).to_fixed_bytes().into()
}

fn network_call(call: pallet_network::Call<Runtime>) -> RuntimeCall {
    RuntimeCall::Network(call)
}

fn swap_call() -> pallet_network::QueuedSwapCall<AccountId> {
    pallet_network::QueuedSwapCall::SwapToSubnetDelegateStake {
        account_id: account(1),
        to_subnet_id: 2,
        balance: 10,
        min_shares_out: 1,
        execute_before_block: 100,
    }
}

#[test]
fn non_transfer_rejects_network_value_calls() {
    let value_calls = vec![
        network_call(pallet_network::Call::update_validator_coldkey {
            validator_id: 1,
            new_coldkey: account(2),
        }),
        network_call(
            pallet_network::Call::update_validator_delegate_reward_rate {
                validator_id: 1,
                new_delegate_reward_rate: 10,
            },
        ),
        network_call(pallet_network::Call::update_validator_delegate_account {
            validator_id: 1,
            delegate_account_id: Some(account(2)),
            delegate_rate: Some(10),
        }),
        network_call(pallet_network::Call::register_subnet {
            max_cost: 10,
            subnet_data: pallet_network::RegistrationSubnetData::<Runtime> {
                name: Vec::new(),
                repo: Vec::new(),
                description: Vec::new(),
                misc: Vec::new(),
                min_stake: 0,
                max_stake: 0,
                delegate_stake_percentage: 0,
                initial_validators: Default::default(),
                bootnodes: Default::default(),
            },
        }),
        network_call(pallet_network::Call::activate_subnet { subnet_id: 1 }),
        network_call(pallet_network::Call::owner_deactivate_subnet { subnet_id: 1 }),
        network_call(pallet_network::Call::owner_update_min_max_stake {
            subnet_id: 1,
            min: 10,
            max: 20,
        }),
        network_call(
            pallet_network::Call::owner_update_delegate_stake_percentage {
                subnet_id: 1,
                value: 10,
            },
        ),
        network_call(pallet_network::Call::transfer_subnet_ownership {
            subnet_id: 1,
            new_owner: account(2),
        }),
        network_call(pallet_network::Call::accept_subnet_ownership { subnet_id: 1 }),
        network_call(
            pallet_network::Call::owner_update_target_node_registrations_per_epoch {
                subnet_id: 1,
                value: 1,
            },
        ),
        network_call(pallet_network::Call::owner_update_node_burn_rate_alpha {
            subnet_id: 1,
            value: 10,
        }),
        network_call(pallet_network::Call::register_subnet_node {
            validator_id: 1,
            subnet_id: 1,
            hotkey: Some(account(3)),
            peer_info: Some(pallet_network::PeerInfo::<Runtime> {
                peer_id: sp_core::OpaquePeerId(Vec::new()),
                multiaddr: None,
            }),
            bootnode_peer_info: None,
            client_peer_info: None,
            stake_to_be_added: 10,
            unique: None,
            non_unique: None,
            max_burn_amount: 10,
        }),
        network_call(pallet_network::Call::remove_subnet_node {
            subnet_id: 1,
            subnet_node_id: 1,
        }),
        network_call(pallet_network::Call::add_node_stake {
            subnet_id: 1,
            subnet_node_id: 1,
            stake_to_be_added: 10,
        }),
        network_call(pallet_network::Call::remove_node_stake {
            subnet_id: 1,
            subnet_node_id: 1,
            stake_to_be_removed: 10,
        }),
        network_call(pallet_network::Call::add_subnet_delegate_stake {
            subnet_id: 1,
            stake_to_be_added: 10,
            min_shares_out: 1,
        }),
        network_call(pallet_network::Call::swap_from_subnet_to_subnet {
            from_subnet_id: 1,
            to_subnet_id: 2,
            delegate_stake_shares_to_swap: 10,
            min_balance_out: 1,
            min_shares_out: 1,
            execute_before_block: u32::MAX,
        }),
        network_call(pallet_network::Call::transfer_delegate_stake {
            subnet_id: 1,
            to_account_id: account(2),
            delegate_stake_shares_to_transfer: 10,
        }),
        network_call(pallet_network::Call::remove_delegate_stake {
            subnet_id: 1,
            shares_to_be_removed: 10,
            min_balance_out: 1,
        }),
        network_call(pallet_network::Call::add_validator_delegate_stake {
            validator_id: 1,
            delegate_stake_to_be_added: 10,
            min_shares_out: 1,
        }),
        network_call(pallet_network::Call::transfer_validator_delegate_stake {
            validator_id: 1,
            to_account_id: account(2),
            validator_delegate_stake_shares_to_transfer: 10,
        }),
        network_call(pallet_network::Call::remove_validator_delegate_stake {
            validator_id: 1,
            validator_delegate_stake_shares_to_be_removed: 10,
            min_balance_out: 1,
        }),
        network_call(pallet_network::Call::swap_from_validator_to_validator {
            from_validator_id: 1,
            to_validator_id: 2,
            stake_to_be_removed: 10,
            min_balance_out: 1,
            min_shares_out: 1,
            execute_before_block: u32::MAX,
        }),
        network_call(pallet_network::Call::swap_from_validator_to_subnet {
            from_validator_id: 1,
            to_subnet_id: 1,
            node_delegate_stake_shares_to_swap: 10,
            min_balance_out: 1,
            min_shares_out: 1,
            execute_before_block: u32::MAX,
        }),
        network_call(pallet_network::Call::swap_from_subnet_to_validator {
            from_subnet_id: 1,
            to_validator_id: 1,
            subnet_delegate_stake_shares_to_swap: 10,
            min_balance_out: 1,
            min_shares_out: 1,
            execute_before_block: u32::MAX,
        }),
        network_call(pallet_network::Call::update_swap_queue {
            id: 1,
            new_call: swap_call(),
        }),
        network_call(pallet_network::Call::remove_delegate_account_balance {
            amount_to_remove: 10,
        }),
        network_call(pallet_network::Call::claim_unbondings {}),
        network_call(pallet_network::Call::register_overwatch_node {
            stake_to_be_added: 10,
        }),
        network_call(pallet_network::Call::remove_overwatch_node {
            overwatch_node_id: 1,
        }),
        network_call(pallet_network::Call::add_overwatch_node_stake {
            overwatch_node_id: 1,
            stake_to_be_added: 10,
        }),
        network_call(pallet_network::Call::remove_overwatch_node_stake {
            overwatch_node_id: 1,
            stake_to_be_removed: 10,
        }),
    ];

    for call in value_calls {
        assert!(
            !ProxyType::NonTransfer.filter(&call),
            "NonTransfer unexpectedly allowed {call:?}"
        );
    }
}

#[test]
fn non_transfer_rejects_utility_and_proxy_escalation() {
    let blocked_network_call = network_call(pallet_network::Call::add_subnet_delegate_stake {
        subnet_id: 1,
        stake_to_be_added: 10,
        min_shares_out: 1,
    });
    let utility_call = RuntimeCall::Utility(pallet_utility::Call::batch_all {
        calls: vec![blocked_network_call],
    });
    let add_proxy_call = RuntimeCall::Proxy(pallet_proxy::Call::add_proxy {
        delegate: account(2),
        proxy_type: ProxyType::Transfer,
        delay: 0,
    });
    let reject_announcement_call = RuntimeCall::Proxy(pallet_proxy::Call::reject_announcement {
        delegate: account(2),
        call_hash: H256::zero(),
    });

    assert!(!ProxyType::NonTransfer.filter(&utility_call));
    assert!(!ProxyType::NonTransfer.filter(&add_proxy_call));
    assert!(ProxyType::NonTransfer.filter(&reject_announcement_call));
    assert!(ProxyType::CancelProxy.filter(&reject_announcement_call));
}

#[test]
fn dedicated_proxy_types_allow_only_their_scoped_network_calls() {
    let balance_transfer = RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
        dest: account(2),
        value: 10,
    });
    let network_transfer = network_call(pallet_network::Call::transfer_delegate_stake {
        subnet_id: 1,
        to_account_id: account(2),
        delegate_stake_shares_to_transfer: 10,
    });
    let delegate_stake = network_call(pallet_network::Call::add_subnet_delegate_stake {
        subnet_id: 1,
        stake_to_be_added: 10,
        min_shares_out: 1,
    });
    let node_stake = network_call(pallet_network::Call::add_node_stake {
        subnet_id: 1,
        subnet_node_id: 1,
        stake_to_be_added: 10,
    });
    let overwatch_stake = network_call(pallet_network::Call::add_overwatch_node_stake {
        overwatch_node_id: 1,
        stake_to_be_added: 10,
    });
    let validator_delegate_stake =
        network_call(pallet_network::Call::add_validator_delegate_stake {
            validator_id: 1,
            delegate_stake_to_be_added: 10,
            min_shares_out: 1,
        });
    let swap_queue_update = network_call(pallet_network::Call::update_swap_queue {
        id: 1,
        new_call: swap_call(),
    });
    let delegate_account_balance_removal =
        network_call(pallet_network::Call::remove_delegate_account_balance {
            amount_to_remove: 10,
        });

    assert!(ProxyType::Transfer.filter(&balance_transfer));
    assert!(ProxyType::Transfer.filter(&network_transfer));
    assert!(!ProxyType::Transfer.filter(&delegate_stake));

    assert!(ProxyType::SubNetworkStaking.filter(&node_stake));
    assert!(ProxyType::SubNetworkStaking.filter(&overwatch_stake));
    assert!(!ProxyType::SubNetworkStaking.filter(&delegate_stake));

    assert!(ProxyType::SubNetworkDelegateStaking.filter(&delegate_stake));
    assert!(ProxyType::SubNetworkDelegateStaking.filter(&validator_delegate_stake));
    assert!(ProxyType::SubNetworkDelegateStaking.filter(&swap_queue_update));
    assert!(ProxyType::SubNetworkDelegateStaking.filter(&delegate_account_balance_removal));
    assert!(!ProxyType::SubNetworkDelegateStaking.filter(&network_transfer));
}

#[test]
fn non_transfer_is_not_superset_of_value_proxy_types() {
    assert!(!ProxyType::NonTransfer.is_superset(&ProxyType::Transfer));
    assert!(!ProxyType::NonTransfer.is_superset(&ProxyType::SubNetworkStaking));
    assert!(!ProxyType::NonTransfer.is_superset(&ProxyType::SubNetworkDelegateStaking));
    assert!(ProxyType::NonTransfer.is_superset(&ProxyType::CancelProxy));
}
