#!/usr/bin/env python3
"""Extract affine models from this repository's FRAME template (research output only).

Usage: python3 .maintain/capacity-generated-model.py weights.rs > models.json
Fails on unknown expression syntax rather than silently dropping a term. Does not
parse the hand-composed production weight file. Ref-time includes RocksDB charges.
"""
import json
import re
import sys
from pathlib import Path


def models(path):
    source = Path(path).read_text().split("// For backwards compatibility")[0]
    found = {}
    for match in re.finditer(r"fn (\w+)\(([^)]*)\) -> Weight \{(.*?)\n\t\}", source, re.S):
        name, arguments, body = match.groups()
        # The generic and unit implementations have the same fitted coefficients.
        if name in found:
            continue
        model = {"base": [0, 0], "slopes": {}}
        for argument in re.findall(r"(\w+): u32", arguments):
            model["slopes"][argument.lstrip("_")] = [0, 0]

        def add(component, time, proof):
            target = model["base"] if component is None else model["slopes"][component]
            target[0] += time
            target[1] += proof

        clean = re.sub(r"//[^\n]*", "", body)
        weight = r"Weight::from_parts\(([\d_]+),\s*([\d_]+)\)(?:\.saturating_mul\((\w+)\.into\(\)\))?"
        for item in re.finditer(weight, clean):
            time, proof, component = item.groups()
            add(component, int(time.replace("_", "")), int(proof.replace("_", "")))
        clean = re.sub(weight, "VALUE", clean)
        db = r"(?:T::DbWeight|RocksDbWeight)::get\(\)\.(reads|writes)\((?:([\d_]+)_u64|\(([\d_]+)_u64\)\.saturating_mul\((\w+)\.into\(\)\))\)"
        for item in re.finditer(db, clean):
            operation, base, slope, component = item.groups()
            cost = 25_000_000 if operation == "reads" else 100_000_000
            add(component, int((base or slope).replace("_", "")) * cost, 0)
        clean = re.sub(db, "VALUE", clean)
        remainder = re.sub(r"VALUE|\.saturating_add|[\s();]", "", clean)
        if remainder:
            raise ValueError(f"Unrecognized weight expression in {name}: {remainder}")
        found[name] = model
    if not found:
        raise ValueError("No generated models found")
    return found


if __name__ == "__main__":
    print(json.dumps(models(sys.argv[1]), indent=2))
