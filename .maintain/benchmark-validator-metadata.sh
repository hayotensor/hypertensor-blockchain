#!/usr/bin/env bash
# Partial outputs: merge reviewed methods and retain existing manual safety allowances.
set -euo pipefail
cd "$(dirname "$0")/.."
output_dir="${1:-/tmp/validator-metadata-benchmarks}"
mkdir -p "$output_dir"
output_dir="$(realpath "$output_dir")"
if [[ "${METADATA_SKIP_BUILD:-0}" != 1 ]]; then
  CARGO_TARGET_DIR="$PWD/target" cargo build --release --locked -p hypertensor-runtime --features runtime-benchmarks \
    > "$output_dir/build.log" 2>&1
fi
runtime=target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm
sha256sum "$runtime" > "$output_dir/runtime.sha256"
sha256sum pallets/network/src/lib.rs pallets/network/src/weights.rs \
  pallets/network/src/benchmarking.rs pallets/network/src/benchmarking/capacity.rs \
  pallets/network/src/utilities/validator.rs pallets/network/src/bank/rewards.rs \
  pallets/network/src/rpc_info/info.rs pallets/network/src/rpc_info/wire.rs \
  runtime/src/configs/common.rs Cargo.lock > "$output_dir/source.sha256"
git rev-parse HEAD > "$output_dir/commit.txt"
git diff -- pallets/network runtime precompiles/network > "$output_dir/source.patch"
rustc --version > "$output_dir/rustc.txt"
target/release/hypertensor-node --version > "$output_dir/bencher.txt"
lscpu > "$output_dir/cpu.txt"
uname -a > "$output_dir/kernel.txt"
free -h > "$output_dir/memory.txt"

# Every existing measured path listing ValidatorsData, plus metadata create/clear variants.
target/release/hypertensor-node benchmark pallet \
  --runtime "$runtime" --genesis-builder runtime --pallets pallet_network \
  --extrinsic register_validator,update_validator_hotkey,update_validator_delegate_account,update_validator_identity,update_validator_identity_create,update_validator_identity_clear,update_validator_delegate_reward_rate,add_validator_delegate_stake,swap_from_subnet_to_validator,set_overwatch_validator_whitelist,execute_ready_swap_calls,execute_ready_swap_mixed_validator,execute_ready_swap_mixed_subnet,execute_ready_swap_mixed_refund,emission_step \
  --extra --steps 20 --repeat 10 --min-duration 0 --output-analysis max \
  --template .maintain/frame-weight-template.hbs \
  --output "$output_dir/affected-weights.rs" --json-file "$output_dir/affected-results.json" \
  > "$output_dir/affected-run.log" 2>&1

target/release/hypertensor-node benchmark pallet \
  --runtime "$runtime" --genesis-builder runtime --pallets pallet_network \
  --extrinsic capacity_settlement,capacity_on_initialize,capacity_attest_all \
  --extra --low 64 --high 512 --steps 8 --repeat 10 --min-duration 0 --output-analysis max \
  --template .maintain/frame-weight-template.hbs \
  --output "$output_dir/capacity-weights.rs" --json-file "$output_dir/capacity-results.json" \
  > "$output_dir/capacity-run.log" 2>&1

python3 .maintain/summarize-subnet-capacity.py "$output_dir/affected-results.json" > "$output_dir/affected-measurements.csv"
python3 .maintain/summarize-subnet-capacity.py "$output_dir/capacity-results.json" > "$output_dir/capacity-measurements.csv"
python3 .maintain/capacity-generated-model.py "$output_dir/affected-weights.rs" > "$output_dir/affected-models.json"
python3 .maintain/capacity-generated-model.py "$output_dir/capacity-weights.rs" > "$output_dir/capacity-models.json"
echo "Validator metadata benchmark artifacts: $output_dir"
