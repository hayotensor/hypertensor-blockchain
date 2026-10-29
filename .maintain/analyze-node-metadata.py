#!/usr/bin/env python3
"""Compare matched capacity samples and retain the generated storage-access reports.

Usage: analyze-node-metadata.py BEFORE AFTER PRODUCTION OUTPUT
Inputs are uncompressed benchmark-run directories. This does not install weights.
"""
import csv
import json
import re
import sys
from pathlib import Path


def storage_report(path):
    source = path.read_text().split("// For backwards compatibility")[0]
    result = {}
    for match in re.finditer(
        r"((?:\t///[^\n]*\n)*)\tfn (\w+)\([^\n]*\) -> Weight \{", source
    ):
        result[match[2]] = [
            {"storage": name, "reads": int(reads), "writes": int(writes)}
            for name, reads, writes in re.findall(
                r"Storage: `([^`]+)` \(r:(\d+) w:(\d+)\)", match[1]
            )
        ]
    if not result:
        raise ValueError(f"No storage reports in {path}")
    return result


def samples(path):
    with path.open() as handle:
        return {
            (row["benchmark"], row["components"]): row
            for row in csv.DictReader(handle)
        }


def estimate(model, components):
    value = model["base"].copy()
    for component in filter(None, components.split(";")):
        key, count = component.split("=")
        for i in range(2):
            value[i] += model["slopes"][key][i] * int(count)
    return value


def main(before, after, production, output):
    output.mkdir(parents=True, exist_ok=True)
    reports = {
        "before": storage_report(before / "capacity-weights.rs"),
        "after": storage_report(after / "capacity-weights.rs"),
        "production": storage_report(production / "affected-weights.rs"),
    }
    forbidden = {"Network::SubnetNodeMetadata", "Network::ValidatorMetadata"}
    for name, accesses in reports["after"].items():
        assert forbidden.isdisjoint(a["storage"] for a in accesses), name
    for name in ["propose_attestation", "attest", "emission_step", "emission_step_queue"]:
        assert forbidden.isdisjoint(
            a["storage"] for a in reports["production"][name]
        ), name
    # Physical cleanup must explicitly read the metadata it removes.
    for name in ["remove_admitted_subnet_node_small", "remove_queued_subnet_node", "do_remove_subnet"]:
        assert any(a["storage"] == "Network::SubnetNodeMetadata"
                   for a in reports["production"][name]), name
    (output / "storage-accesses.json").write_text(json.dumps(reports, indent=2) + "\n")

    production_models = json.loads((production / "affected-models.json").read_text())
    coverage = []
    for (name, components), row in samples(production / "affected-measurements.csv").items():
        estimated = estimate(production_models[name], components)[1]
        recorded = int(row["recorded_proof_bytes"])
        coverage.append({"benchmark": name, "components": components,
                         "estimated_proof": estimated, "recorded_proof": recorded,
                         "margin_bytes_before_manual_allowances": estimated - recorded})
    (output / "generated-proof-coverage.json").write_text(json.dumps(coverage, indent=2) + "\n")

    old, new = samples(before / "capacity-measurements.csv"), samples(after / "capacity-measurements.csv")
    assert old.keys() == new.keys()
    models = [json.loads((path / "capacity-models.json").read_text()) for path in (before, after)]
    rows = []
    for key in old:
        a, b = old[key], new[key]
        assert a["samples"] == b["samples"] == "10", key
        proof_before, proof_after = int(a["recorded_proof_bytes"]), int(b["recorded_proof_bytes"])
        if key[0] in {"capacity_proposal", "capacity_attest_last"}:
            assert proof_after < proof_before, key
        estimates = [estimate(model[key[0]], key[1]) for model in models]
        rows.append({
            "benchmark": key[0], "components": key[1],
            "before_median_ms": a["median_ms"], "after_median_ms": b["median_ms"],
            "before_p95_ms": a["p95_ms"], "after_p95_ms": b["p95_ms"],
            "before_recorded_proof": proof_before, "after_recorded_proof": proof_after,
            "proof_reduction_bytes": proof_before - proof_after,
            "before_estimated_proof": estimates[0][1], "after_estimated_proof": estimates[1][1],
            "before_model_ref_time_ps": estimates[0][0], "after_model_ref_time_ps": estimates[1][0],
            "before_reads": a["unique_reads"], "after_reads": b["unique_reads"],
            "before_writes": a["unique_writes"], "after_writes": b["unique_writes"],
        })
    with (output / "comparison.csv").open("w") as handle:
        writer = csv.DictWriter(handle, fieldnames=rows[0].keys())
        writer.writeheader()
        writer.writerows(rows)


if __name__ == "__main__":
    main(*(Path(arg) for arg in sys.argv[1:]))
