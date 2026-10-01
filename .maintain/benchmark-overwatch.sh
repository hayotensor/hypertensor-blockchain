#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
output_dir="${1:-/tmp/overwatch-benchmarks}"
mkdir -p "$output_dir"

# Build the matching node with runtime-benchmarks before invoking this script. Older standalone
# frame-omni-bencher versions may not understand this runtime's storage-whitelist host interface.
target/release/hypertensor-node benchmark pallet \
  --runtime target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm \
  --genesis-builder runtime \
  --pallets pallet_network \
  --extrinsic advance_overwatch_epoch,advance_overwatch_epoch_noop,commit_overwatch_subnet_weights,reveal_overwatch_subnet_weights,calculate_overwatch_rewards_empty,calculate_overwatch_rewards_small,calculate_overwatch_rewards_medium,calculate_overwatch_rewards,calculate_overwatch_rewards_per_subnet,remove_overwatch_node,remove_overwatch_node_last_pending,collective_remove_overwatch_node,collective_remove_overwatch_node_last_pending,collective_remove_overwatch_node_after_exit,collective_remove_overwatch_node_after_exit_last_pending,collective_remove_overwatch_node_finalized_only \
  --extra --steps 50 --repeat 20 --min-duration 0 --output-analysis max \
  --template .maintain/frame-weight-template.hbs \
  --output "$output_dir/weights.rs" \
  --json-file "$output_dir/results.json" \
  > "$output_dir/run.log" 2>&1

# This is a partial weight file. Never replace the complete pallet weight table with it.
echo "Overwatch benchmark results: $output_dir"
