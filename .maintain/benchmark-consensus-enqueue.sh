#!/usr/bin/env bash
# Measure single and batched consensus-failure appends against the pinned runtime/SDK.
set -euo pipefail
cd "$(dirname "$0")/.."
output_dir="${1:?Specify an artifact directory}"
mkdir -p "$output_dir"
output_dir="$(realpath "$output_dir")"
if [[ "${CONSENSUS_ENQUEUE_SKIP_BUILD:-0}" != 1 ]]; then
  SKIP_FRAME_STORAGE_ACCESS_TEST_RUNTIME_WASM_BUILD=1 SKIP_PALLET_REVIVE_FIXTURES=1 \
    cargo build --release --locked -p hypertensor-runtime --features runtime-benchmarks \
    > "$output_dir/build.log" 2>&1
fi
runtime=target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm
sha256sum "$runtime" > "$output_dir/runtime.sha256"
git rev-parse HEAD > "$output_dir/commit.txt"
git diff -- pallets/network runtime > "$output_dir/source.patch"
target/release/hypertensor-node benchmark pallet \
  --runtime "$runtime" --genesis-builder runtime --pallets pallet_network \
  --extrinsic pending_consensus_operation_enqueue,pending_consensus_operations_enqueue,pending_consensus_operations_enqueue_empty_queue \
  --extra --steps 50 --repeat 20 --min-duration 0 --wasm-execution compiled \
  --output-analysis max --output-pov-analysis max \
  --template .maintain/frame-weight-template.hbs \
  --output "$output_dir/weights.rs" --json-file "$output_dir/results.json" \
  > "$output_dir/run.log" 2>&1
