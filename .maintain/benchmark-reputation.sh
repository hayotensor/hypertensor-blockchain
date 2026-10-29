#!/usr/bin/env bash
# Matched reputation research or partial production weights; never overwrite an existing run.
set -euo pipefail
cd "$(dirname "$0")/.."
output_dir="${1:?Specify a new artifact directory}"
mode="${2:-capacity}"
mkdir "$output_dir"
output_dir="$(realpath "$output_dir")"
runtime=target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm
sha256sum "$runtime" > "$output_dir/runtime.sha256"
gzip -c "$runtime" > "$output_dir/runtime.wasm.gz"
git rev-parse HEAD > "$output_dir/commit.txt"
git diff -- pallets/network runtime precompiles/network > "$output_dir/source.patch"
tar -czf "$output_dir/source.tar.gz" pallets/network/src runtime/src precompiles/network/src \
  .maintain/benchmark-reputation.sh .maintain/merge-reputation-weights.py \
  .maintain/analyze-reputation.py .maintain/frame-weight-template.hbs \
  .maintain/capacity-generated-model.py Cargo.lock
python3 - "$output_dir" <<'PY'
import hashlib, pathlib, sys
paths = sorted(p for root in ('pallets/network/src', 'runtime/src', 'precompiles/network/src')
               for p in pathlib.Path(root).rglob('*.rs'))
paths += [pathlib.Path('Cargo.lock'), pathlib.Path('.maintain/benchmark-reputation.sh')]
pathlib.Path(sys.argv[1], 'source.sha256').write_text(''.join(
    f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p}\n' for p in paths))
PY
rustc --version > "$output_dir/rustc.txt"
target/release/hypertensor-node --version > "$output_dir/bencher.txt"
lscpu > "$output_dir/cpu.txt"
uname -a > "$output_dir/kernel.txt"
free -h > "$output_dir/memory.txt"
if [[ "$mode" == capacity ]]; then
  methods=capacity_settlement,capacity_on_initialize,capacity_settlement_reputation,capacity_on_initialize_reputation
  settings=(--low 64 --high 512 --steps 8)
elif [[ "$mode" == weights ]]; then
  methods=emission_step,emission_step_accepted_below_min_weight_reputation,emission_step_accepted_non_attestor_reputation,emission_step_rejected,emission_step_emergency,emission_step_missing,precheck_subnet_consensus_submission,precheck_subnet_consensus_submission_missing
  settings=(--steps 20)
else
  echo "Expected capacity or weights" >&2
  exit 1
fi
command=(target/release/hypertensor-node benchmark pallet
  --runtime "$runtime" --genesis-builder runtime --pallets pallet_network
  --extrinsic "$methods" --extra "${settings[@]}" --repeat 10 --min-duration 0
  --output-analysis max --template .maintain/frame-weight-template.hbs
  --output "$output_dir/weights.rs" --json-file "$output_dir/results.json")
printf '%q ' "${command[@]}" > "$output_dir/command.txt"
"${command[@]}" > "$output_dir/run.log" 2>&1
python3 .maintain/capacity-generated-model.py "$output_dir/weights.rs" > "$output_dir/models.json"
