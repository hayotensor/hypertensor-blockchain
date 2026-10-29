#!/usr/bin/env python3
"""Merge the reviewed metadata benchmark methods, retaining existing safety additions.

Usage: python3 .maintain/merge-node-metadata-weights.py generated-weights.rs
Only accepts the explicit benchmark set from benchmark-node-metadata-weights.sh.
"""
import re
import sys
from pathlib import Path

METHODS = set(Path(__file__).with_name("node-metadata-weight-methods.txt").read_text().splitlines())

METHOD = re.compile(r"(?:\t///[^\n]*\n)*\tfn (\w+)\([^\n]*\) -> Weight \{.*?\n\t\}", re.S)
SAFETY = re.compile(
    r"^\t\t\t\.saturating_add\((?:(?:pending_liability_key_proof|settlement_pending_marker_envelope|consensus_round_liability_proof|subnet_node_registration_collision_proof)\(\)|Self::pending_(?:admitted|queued)_removal_scan\(MAX_PENDING_(?:ADMITTED|QUEUED)_REMOVALS\)).*?$",
    re.M,
)


def merge_source(source, generated):
    """Merge current methods without dropping the manually composed safety allowances."""
    methods = {}
    for match in METHOD.finditer(generated):
        methods.setdefault(match[1], []).append(match[0])
    if set(methods) != METHODS or any(len(v) != 2 for v in methods.values()):
        raise ValueError(
            f"Expected exactly the {len(METHODS)} node metadata methods in both implementations"
        )
    counts = dict.fromkeys(METHODS, 0)

    def replace(match):
        name = match[1]
        if name not in METHODS:
            return match[0]
        replacement = methods[name][counts[name]]
        counts[name] += 1
        additions = SAFETY.findall(match[0])
        if additions:
            replacement = replacement[:-3] + "\n" + "\n".join(additions) + "\n\t}"
        return replacement

    merged = METHOD.sub(replace, source)
    if any(n != 2 for n in counts.values()):
        raise ValueError(f"Expected two existing implementations: {counts}")
    if SAFETY.findall(merged) != SAFETY.findall(source):
        raise ValueError("Manual safety additions changed")
    return merged


def merge(generated_path):
    destination = Path(__file__).resolve().parents[1] / "pallets/network/src/weights.rs"
    merged = merge_source(destination.read_text(), Path(generated_path).read_text())
    destination.write_text(merged)
    print(f"Updated {len(METHODS)} methods in both implementations; retained manual safety additions")


if __name__ == "__main__":
    merge(sys.argv[1])
