#!/usr/bin/env bash
# Matched capacity sweep. Run before and after the storage split in separate directories.
set -euo pipefail
cd "$(dirname "$0")/.."
output_dir="${1:?Specify a new before/after artifact directory}"
mkdir -p "$output_dir"
output_dir="$(realpath "$output_dir")"
if [[ "${NODE_METADATA_SKIP_BUILD:-0}" != 1 ]]; then
  cargo build --release --locked -p hypertensor-runtime --features runtime-benchmarks \
    > "$output_dir/build.log" 2>&1
fi
runtime=target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm
sha256sum "$runtime" > "$output_dir/runtime.sha256"
git rev-parse HEAD > "$output_dir/commit.txt"
git diff -- pallets/network runtime precompiles/network > "$output_dir/source.patch"
# Include untracked research fixtures, which git diff alone cannot reproduce.
tar -czf "$output_dir/source.tar.gz" pallets/network/src precompiles/network/src runtime/src \
  .maintain/benchmark-node-metadata.sh .maintain/benchmark-node-metadata-weights.sh \
  .maintain/node-metadata-weight-methods.txt .maintain/merge-node-metadata-weights.py \
  .maintain/analyze-node-metadata.py \
  .maintain/summarize-subnet-capacity.py .maintain/capacity-generated-model.py \
  .maintain/frame-weight-template.hbs Cargo.lock
python3 - "$output_dir" <<'PY'
import hashlib, pathlib, sys
paths = sorted(p for root in ('pallets/network/src', 'runtime/src', 'precompiles/network/src')
               for p in pathlib.Path(root).rglob('*.rs'))
paths += [pathlib.Path('Cargo.lock'), pathlib.Path('.maintain/benchmark-node-metadata.sh')]
pathlib.Path(sys.argv[1], 'source.sha256').write_text(''.join(
    f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p}\n' for p in paths))
PY
rustc --version > "$output_dir/rustc.txt"
target/release/hypertensor-node --version > "$output_dir/bencher.txt"
lscpu > "$output_dir/cpu.txt"
uname -a > "$output_dir/kernel.txt"
free -h > "$output_dir/memory.txt"
target/release/hypertensor-node benchmark pallet \
  --runtime "$runtime" --genesis-builder runtime --pallets pallet_network \
  --extrinsic capacity_proposal,capacity_attest_last,capacity_settlement,capacity_on_initialize,capacity_attest_all \
  --extra --low 64 --high 512 --steps 8 --repeat 10 --min-duration 0 --output-analysis max \
  --template .maintain/frame-weight-template.hbs \
  --output "$output_dir/capacity-weights.rs" --json-file "$output_dir/capacity-results.json" \
  > "$output_dir/capacity-run.log" 2>&1
python3 .maintain/summarize-subnet-capacity.py "$output_dir/capacity-results.json" > "$output_dir/capacity-measurements.csv"
python3 .maintain/capacity-generated-model.py "$output_dir/capacity-weights.rs" > "$output_dir/capacity-models.json"
