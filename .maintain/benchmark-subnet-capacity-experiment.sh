#!/usr/bin/env bash
# Isolated 1024-node research build; uses extrapolated weights, never a deployment artifact.
set -euo pipefail
cd "$(dirname "$0")/.."
output_dir="${1:-/tmp/subnet-capacity-experiment}"
mkdir -p "$output_dir"
output_dir="$(realpath "$output_dir")"
source_dir="$output_dir/source"
python3 .maintain/prepare-subnet-capacity-experiment.py "$source_dir" 1024

# Reuse dependency artifacts, then restore the ordinary runtime build even if a probe fails.
restore_runtime() {
  local result=$?
  trap - EXIT
  if ! CARGO_TARGET_DIR="$PWD/target" cargo build --release --locked -p hypertensor-runtime --features runtime-benchmarks \
      > "$output_dir/restore-build.log" 2>&1; then
    echo "Could not restore the ordinary runtime build; inspect $output_dir/restore-build.log" >&2
    exit 1
  fi
  exit "$result"
}
trap restore_runtime EXIT
CARGO_TARGET_DIR="$PWD/target" cargo build --manifest-path "$source_dir/runtime/Cargo.toml" \
  --release --locked --features runtime-benchmarks > "$output_dir/build.log" 2>&1
cp target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm \
  "$output_dir/runtime.wasm"
sha256sum "$output_dir/runtime.wasm" > "$output_dir/runtime.sha256"

target/release/hypertensor-node benchmark pallet \
  --runtime "$output_dir/runtime.wasm" --genesis-builder runtime --pallets pallet_network \
  --extrinsic capacity_settlement,capacity_attest_all --extra --steps 20 --repeat 10 \
  --min-duration 0 --output-analysis max --template .maintain/frame-weight-template.hbs \
  --output "$output_dir/weights.rs" --json-file "$output_dir/results.json" \
  > "$output_dir/run.log" 2>&1

python3 - "$output_dir" <<'PY'
import json
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1])
base = ['target/release/hypertensor-node', 'benchmark', 'pallet', '--runtime', str(root/'runtime.wasm'),
        '--genesis-builder', 'runtime', '--pallets', 'pallet_network', '--extrinsic',
        'capacity_on_initialize', '--extra', '--steps', '2', '--repeat', '3', '--min-duration', '0',
        '--quiet']
checks = []
for nodes, expected_success in [(768, True), (847, True), (848, False), (1024, False)]:
    log_path = root/f'hook-{nodes}.log'
    with log_path.open('w') as log:
        result = subprocess.run(base + ['--low', str(nodes), '--high', str(nodes), '--json-file',
                                str(root/f'hook-{nodes}.json')], stdout=log, stderr=subprocess.STDOUT)
    passed = result.returncode == 0
    checks.append({'nodes': nodes, 'exit_code': result.returncode, 'verification_passed': passed})
    assert passed == expected_success, f'Unexpected hook admission at {nodes}: see {log_path}'
    if nodes == 848:
        assert 'SubnetElectedValidator::<T>::contains_key' in log_path.read_text()
    if nodes == 1024:
        assert 'left: Some(false)' in log_path.read_text()
(root/'hook-checks.json').write_text(json.dumps(checks, indent=2) + '\n')
PY
python3 .maintain/summarize-subnet-capacity.py "$output_dir/results.json" > "$output_dir/measurements.csv"
python3 .maintain/capacity-generated-model.py "$output_dir/weights.rs" > "$output_dir/models.json"
echo "Research artifacts: $output_dir (ordinary runtime build is being restored)"
