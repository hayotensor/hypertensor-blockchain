#!/usr/bin/env bash
# Research artifacts only. Never install this partial output as production weights.rs.
set -euo pipefail
cd "$(dirname "$0")/.."
output_dir="${1:-/tmp/subnet-capacity}"
mkdir -p "$output_dir"
output_dir="$(realpath "$output_dir")"

CARGO_TARGET_DIR="$PWD/target" cargo build --release --locked -p hypertensor-runtime --features runtime-benchmarks \
  > "$output_dir/build.log" 2>&1
runtime=target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm
sha256sum "$runtime" > "$output_dir/runtime.sha256"
git rev-parse HEAD > "$output_dir/commit.txt"
git diff -- pallets/network runtime > "$output_dir/source.patch"
rustc --version > "$output_dir/rustc.txt"
target/release/hypertensor-node --version > "$output_dir/bencher.txt"
lscpu > "$output_dir/cpu.txt"

target/release/hypertensor-node benchmark pallet \
  --runtime "$runtime" --genesis-builder runtime --pallets pallet_network \
  --extrinsic emission_step,emission_step_accepted_queue_mutations,emission_step_accepted_queue_mutations_front,emission_step_accepted_below_min_weight_reputation,emission_step_accepted_non_attestor_reputation,emission_step_rejected,emission_step_emergency,emission_step_missing,elect_validator,elect_validator_emergency,elect_validator_expired,propose_attestation,propose_attestation_emergency,attest \
  --extra --steps "${CAPACITY_STEPS:-20}" --repeat "${CAPACITY_REPEAT:-10}" \
  --min-duration 0 --output-analysis max \
  --template .maintain/frame-weight-template.hbs \
  --output "$output_dir/weights.rs" --json-file "$output_dir/results.json" \
  > "$output_dir/run.log" 2>&1

target/release/hypertensor-node benchmark pallet \
  --runtime "$runtime" --genesis-builder runtime --pallets pallet_network \
  --extrinsic capacity_settlement,capacity_on_initialize,capacity_attest_all \
  --extra --low 64 --high 512 --steps 8 --repeat "${CAPACITY_REPEAT:-10}" \
  --min-duration 0 --output-analysis max \
  --template .maintain/frame-weight-template.hbs \
  --output "$output_dir/capacity-weights.rs" --json-file "$output_dir/capacity-results.json" \
  > "$output_dir/capacity-run.log" 2>&1

# The CLI's floating-point step grid can stop at 508 for the [1,509] component.
# Verify the exact legal endpoint independently instead of silently claiming it was sampled.
target/release/hypertensor-node benchmark pallet \
  --runtime "$runtime" --genesis-builder runtime --pallets pallet_network \
  --extrinsic emission_step_accepted_non_attestor_reputation \
  --low 508 --high 509 --steps 2 --repeat "${CAPACITY_REPEAT:-10}" --min-duration 0 \
  --json-file "$output_dir/non-attestor-endpoint.json" \
  > "$output_dir/non-attestor-endpoint.log" 2>&1

cargo test --release --locked -p pallet-network capacity_weight_report \
  -- --ignored --nocapture > "$output_dir/admission-model.log" 2>&1
python3 .maintain/summarize-subnet-capacity.py "$output_dir/results.json" \
  > "$output_dir/measurements.csv"
python3 .maintain/summarize-subnet-capacity.py "$output_dir/capacity-results.json" \
  > "$output_dir/capacity-measurements.csv"
python3 .maintain/summarize-subnet-capacity.py "$output_dir/non-attestor-endpoint.json" \
  > "$output_dir/non-attestor-endpoint.csv"
echo "Subnet capacity research: $output_dir"
