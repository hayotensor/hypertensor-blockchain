mod admin;
mod delegate_account;
mod delegate_staking;
mod era;
mod incentives_protocol;
mod inflation;
mod keys;
mod math;
pub mod mock;
mod multiaddr;
mod node_staking;
mod on_initialize;
mod overwatch_counterfactual;
mod overwatch_nodes;
mod overwatch_nodes_commit_reveal;
mod owner;
mod pending_removals;
mod queue_maturity;
mod randomization;
mod registration_queue;
mod rpc;
mod slot;
mod staking_queue;
mod staking_utils;
mod steps;
mod subnet;
mod subnet_cost;
mod subnet_node;
mod test_utils;
mod unbonding;
mod validator;
mod validator_delegate_staking;

#[test]
fn physical_subnet_upper_bound_respects_epoch_capacity_and_benchmark_domain() {
    assert_eq!(crate::physical_subnet_upper_bound(3), 0);
    assert_eq!(crate::physical_subnet_upper_bound(4), 1);
    assert_eq!(crate::physical_subnet_upper_bound(10), 7);
    assert_eq!(crate::physical_subnet_upper_bound(20), 17);
    assert_eq!(crate::physical_subnet_upper_bound(100), 17);
}
