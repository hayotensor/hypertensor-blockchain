#!/usr/bin/env bash
# Measure immediate removal, lifecycle reads and every removed-subnet cleanup stage.
set -euo pipefail
cd "$(dirname "$0")/.."
output_dir="${1:?Specify an artifact directory}"
mkdir -p "$output_dir"
output_dir="$(realpath "$output_dir")"
if [[ "${SUBNET_CLEANUP_SKIP_BUILD:-0}" != 1 ]]; then
  SKIP_FRAME_STORAGE_ACCESS_TEST_RUNTIME_WASM_BUILD=1 SKIP_PALLET_REVIVE_FIXTURES=1 \
    cargo build --release --locked --offline -p hypertensor-runtime --features runtime-benchmarks \
    > "$output_dir/build.log" 2>&1
fi
runtime=target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm
sha256sum "$runtime" > "$output_dir/runtime.sha256"
git rev-parse HEAD > "$output_dir/commit.txt"
git diff -- pallets/network runtime precompiles/network > "$output_dir/source.patch"
tar -czf "$output_dir/source.tar.gz" pallets/network/src runtime/src precompiles/network/src \
  .maintain/benchmark-subnet-cleanup.sh .maintain/merge-subnet-cleanup-weights.py \
  .maintain/frame-weight-template.hbs Cargo.lock
common=(target/release/hypertensor-node benchmark pallet
  --runtime "$runtime" --genesis-builder runtime --pallets pallet_network
  --extra --repeat 20 --min-duration 0 --wasm-execution compiled
  --output-analysis max --output-pov-analysis max
  --template .maintain/frame-weight-template.hbs)
# Production permits 16 physical subnets; the pallet's shared benchmark domain reaches 17.
# Sample the actual 0..16 subnet dimension separately from the 0..62 cleanup-stage selector.
"${common[@]}" \
  --extrinsic do_remove_subnet,owner_deactivate_subnet,collective_remove_subnet,activate_subnet,activate_subnet_success,do_epoch_preliminaries,subnet_existence_check,removed_subnet_cleanup_selector,removed_subnet_cleanup_progress \
  --low 0 --high 16 --steps 17 --output "$output_dir/fixed-weights.rs" --json-file "$output_dir/fixed-results.json" \
  > "$output_dir/run.log" 2>&1
"${common[@]}" \
  --extrinsic removed_subnet_cleanup_step,removed_subnet_cleanup_empty_step \
  --steps 63 --json-file "$output_dir/stage-results.json" \
  >> "$output_dir/run.log" 2>&1
python3 - "$output_dir" <<'PY'
import json
import sys
from pathlib import Path
out = Path(sys.argv[1])
(out / 'weights.rs').write_text((out / 'fixed-weights.rs').read_text())
(out / 'results.json').write_text(json.dumps(json.loads((out / 'fixed-results.json').read_text()) + json.loads((out / 'stage-results.json').read_text())))
PY
