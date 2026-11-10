#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
output_dir="${1:-/tmp/global-pause-benchmarks}"
mkdir -p "$output_dir"

# First build the matching runtime: cargo build --release --locked -p hypertensor-runtime --features runtime-benchmarks
target/release/hypertensor-node benchmark pallet \
  --runtime target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm \
  --genesis-builder runtime --pallets pallet_network \
  --extrinsic unpause,consensus_resume_check,precheck_subnet_consensus_submission_cancelled,emission_step_cancelled \
  --extra --steps 20 --repeat 20 --min-duration 0 --output-analysis max \
  --template .maintain/frame-weight-template.hbs \
  --output "$output_dir/weights.rs" --json-file "$output_dir/results.json" \
  > "$output_dir/run.log" 2>&1

# This is a partial table. Integrate only unpause and the two cancellation methods; compose
# the isolated selector cost with the existing proposal, attestation and settlement models.
echo "Global pause benchmark results: $output_dir"
