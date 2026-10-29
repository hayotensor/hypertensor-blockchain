#!/usr/bin/env python3
"""Copy source to a NEW directory and lift only the subnet bound for research.

Usage: python3 .maintain/prepare-subnet-capacity-experiment.py /tmp/capacity-1024 1024
Does not change the working checkout. Copied weights are extrapolated, NOT regenerated.
Use capacity_settlement to measure beyond current admission; capacity_on_initialize
deliberately fails verification when the existing admission model skips mandatory work.
"""
import json
import shutil
import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
destination = Path(sys.argv[1]).resolve()
nodes = int(sys.argv[2])
if not 513 <= nodes <= 4096:
    raise SystemExit("Research bound must be between 513 and 4096")
destination.mkdir(parents=True, exist_ok=False)
files = subprocess.check_output(["git", "ls-files", "-z"], cwd=root).decode().split("\0")
files += ["pallets/network/src/benchmarking/capacity.rs", "pallets/network/src/tests/capacity.rs"]
for name in set(files) - {""}:
    source = root / name
    if source.is_file():
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
replacements = {
    "pallets/network/src/lib.rs": "MAX_SUBNET_NODES_BENCHMARK_DOMAIN: u32 = 512",
    "runtime/src/configs/common.rs": "NetworkMaxSubnetNodesUpperBound: u32 = 512",
    "pallets/network/src/tests/mock.rs": "NetworkMaxSubnetNodesUpperBound: u32 = 512",
    "pallets/network/src/weights.rs": "MAX_PENDING_ADMITTED_REMOVALS: u32 = 512",
}
for name, old in replacements.items():
    path = destination / name
    source = path.read_text()
    if source.count(old) != 1:
        raise SystemExit(f"Expected exactly one bound in {name}; review the script")
    path.write_text(source.replace(old, old.replace("512", str(nodes))))
(destination / "capacity-experiment.json").write_text(json.dumps({
    "nodes": nodes, "source": str(root),
    "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
    "replacements": replacements, "production_weights_regenerated": False,
}, indent=2) + "\n")
print(destination)
