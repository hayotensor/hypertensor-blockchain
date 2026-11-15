#!/usr/bin/env bash
# Matched, local compiled-Wasm measurements for election-time voting snapshots.
set -euo pipefail
cd "$(dirname "$0")/.."
output_dir="${1:?Specify a new artifact directory}"
mode="${2:-weights}"
mkdir "$output_dir"
output_dir="$(realpath "$output_dir")"
runtime=target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm
sha256sum "$runtime" > "$output_dir/runtime.sha256"
gzip -c "$runtime" > "$output_dir/runtime.wasm.gz"
cp pallets/network/src/weights.rs "$output_dir/baseline-weights.rs"
git rev-parse HEAD > "$output_dir/commit.txt"
git diff -- pallets/network runtime .maintain > "$output_dir/source.patch"
tar -czf "$output_dir/source.tar.gz" pallets/network/src runtime/src \
  .maintain/benchmark-election-snapshots.sh .maintain/merge-election-snapshot-weights.py \
  .maintain/frame-weight-template.hbs Cargo.lock
rustc --version > "$output_dir/rustc.txt"
target/release/hypertensor-node --version > "$output_dir/bencher.txt"
lscpu > "$output_dir/cpu.txt"
uname -a > "$output_dir/kernel.txt"
if [[ "$mode" == weights ]]; then
  methods=elect_validator,elect_validator_multi_node,propose_attestation,precheck_subnet_consensus_submission,precheck_subnet_consensus_submission_missing,emission_step,emission_step_rejected,emission_step_missing
  settings=(--steps 20 --repeat 10)
elif [[ "$mode" == capacity ]]; then
  methods=capacity_precheck,capacity_settlement,capacity_on_initialize,capacity_settlement_reputation,capacity_on_initialize_reputation
  settings=(--low 64 --high 512 --steps 8 --repeat 10)
elif [[ "$mode" == smoke ]]; then
  methods=elect_validator,elect_validator_multi_node,propose_attestation,precheck_subnet_consensus_submission,precheck_subnet_consensus_submission_missing,emission_step,emission_step_rejected,emission_step_missing
  settings=(--low 511 --high 512 --steps 2 --repeat 1)
else
  echo "Expected weights, capacity or smoke" >&2
  exit 1
fi
command=(target/release/hypertensor-node benchmark pallet
  --runtime "$runtime" --genesis-builder runtime --pallets pallet_network
  --extrinsic "$methods" --extra "${settings[@]}" --min-duration 0
  --output-analysis max --output-pov-analysis max --wasm-execution compiled
  --template .maintain/frame-weight-template.hbs
  --output "$output_dir/weights.rs" --json-file "$output_dir/results.json")
printf '%q ' "${command[@]}" > "$output_dir/command.txt"
"${command[@]}" > "$output_dir/run.log" 2>&1
