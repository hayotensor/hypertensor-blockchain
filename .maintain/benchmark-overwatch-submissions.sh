#!/usr/bin/env bash
# Measure complete submissions and rejection paths with the node built against the pinned SDK.
set -euo pipefail
cd "$(dirname "$0")/.."
output_dir="${1:-/tmp/overwatch-submission-benchmarks}"
mkdir -p "$output_dir"
output_dir="$(realpath "$output_dir")"
if [[ "${OVERWATCH_SUBMISSIONS_SKIP_BUILD:-0}" != 1 ]]; then
  cargo build --release --locked -p hypertensor-runtime --features runtime-benchmarks \
    > "$output_dir/build.log" 2>&1
fi
runtime=target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm
sha256sum "$runtime" > "$output_dir/runtime.sha256"
git rev-parse HEAD > "$output_dir/commit.txt"
git diff -- pallets/network runtime > "$output_dir/source.patch"
target/release/hypertensor-node benchmark pallet \
  --runtime "$runtime" --genesis-builder runtime --pallets pallet_network \
  --extrinsic commit_overwatch_subnet_weights,reveal_overwatch_subnet_weights,commit_overwatch_subnet_weights_invalid,commit_overwatch_subnet_weights_repeated,reveal_overwatch_subnet_weights_incomplete,reveal_overwatch_subnet_weights_invalid,reveal_overwatch_subnet_weights_repeated \
  --extra --steps 50 --repeat 20 --min-duration 0 --wasm-execution compiled \
  --output-analysis max --output-pov-analysis max \
  --template .maintain/frame-weight-template.hbs \
  --output "$output_dir/weights.rs" --json-file "$output_dir/results.json" \
  > "$output_dir/run.log" 2>&1
# This partial output must be merged with merge-overwatch-submission-weights.py.
echo "Overwatch submission benchmarks: $output_dir"
