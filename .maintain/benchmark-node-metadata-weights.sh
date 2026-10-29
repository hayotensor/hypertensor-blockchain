#!/usr/bin/env bash
# Partial network weights only. Merge using merge-node-metadata-weights.py.
# Probation promotion has a separate extra benchmark, not a standalone WeightInfo method.
set -euo pipefail
cd "$(dirname "$0")/.."
output_dir="${1:?Specify the production-weight artifact directory}"
mkdir -p "$output_dir"
output_dir="$(realpath "$output_dir")"
runtime=target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm
if [[ "${NODE_METADATA_SKIP_BUILD:-0}" != 1 ]]; then
  cargo build --release --locked -p hypertensor-runtime --features runtime-benchmarks \
    > "$output_dir/build.log" 2>&1
fi
sha256sum "$runtime" > "$output_dir/runtime.sha256"
git rev-parse HEAD > "$output_dir/commit.txt"
git diff -- pallets/network runtime precompiles/network > "$output_dir/source.patch"
tar -czf "$output_dir/source.tar.gz" pallets/network/src precompiles/network/src runtime/src \
  .maintain/benchmark-node-metadata-weights.sh .maintain/node-metadata-weight-methods.txt \
  .maintain/frame-weight-template.hbs .maintain/summarize-subnet-capacity.py \
  .maintain/capacity-generated-model.py Cargo.lock
sha256sum pallets/network/src/lib.rs pallets/network/src/weights.rs \
  pallets/network/src/benchmarking.rs pallets/network/src/benchmarking/capacity.rs \
  pallets/network/src/utilities/subnet_node.rs Cargo.lock > "$output_dir/source.sha256"
methods="$(paste -sd, .maintain/node-metadata-weight-methods.txt)"
target/release/hypertensor-node benchmark pallet \
  --runtime "$runtime" --genesis-builder runtime --pallets pallet_network \
  --extrinsic "$methods" --extra --steps 20 --repeat 10 --min-duration 0 --output-analysis max \
  --template .maintain/frame-weight-template.hbs \
  --output "$output_dir/affected-weights.rs" --json-file "$output_dir/affected-results.json" \
  > "$output_dir/affected-run.log" 2>&1
python3 .maintain/summarize-subnet-capacity.py "$output_dir/affected-results.json" > "$output_dir/affected-measurements.csv"
python3 .maintain/capacity-generated-model.py "$output_dir/affected-weights.rs" > "$output_dir/affected-models.json"
