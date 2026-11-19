#!/usr/bin/env bash
# Measure event-producing paths, including alternative election/removal profiles.
# Merge with merge-event-weights.py after reviewing the results.
set -euo pipefail
cd "$(dirname "$0")/.."
output_dir="${1:-/tmp/hypertensor-event-weights}"
mkdir -p "$output_dir"
output_dir="$(realpath "$output_dir")"
if [[ "${EVENT_WEIGHTS_SKIP_BUILD:-0}" != 1 ]]; then
  cargo build --release --locked -p hypertensor-runtime --features runtime-benchmarks \
    > "$output_dir/build.log" 2>&1
fi
runtime=target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm
sha256sum "$runtime" > "$output_dir/runtime.sha256"
git rev-parse HEAD > "$output_dir/commit.txt"
git diff -- pallets/network runtime > "$output_dir/source.patch"
methods=register_validator,register_subnet_node,register_subnet_node_immediate,update_validator_coldkey,update_validator_hotkey,update_node_hotkey,update_validator_delegate_account,transfer_delegate_stake,transfer_validator_delegate_stake,claim_unbondings,register_overwatch_node,remove_overwatch_node,remove_overwatch_node_last_pending,update_overwatch_hotkey,set_overwatch_node_peer_id,commit_overwatch_subnet_weights,reveal_overwatch_subnet_weights,update_require_subnet_registration_whitelist,update_subnet_registrant,elect_validator,elect_validator_multi_node,handle_subnet_emission_weights,handle_subnet_emission_weights_empty,emission_step_queue,add_node_stake,remove_node_stake,add_overwatch_node_stake,remove_overwatch_node_stake,swap_from_subnet_to_subnet,swap_from_subnet_to_validator,swap_from_validator_to_subnet,swap_from_validator_to_validator
target/release/hypertensor-node benchmark pallet \
  --runtime "$runtime" --genesis-builder runtime --pallets pallet_network \
  --extrinsic "$methods" --extra --steps 20 --repeat 10 --min-duration 0 \
  --output-analysis max --template .maintain/frame-weight-template.hbs \
  --output "$output_dir/weights.rs" --json-file "$output_dir/results.json" \
  > "$output_dir/run.log" 2>&1
