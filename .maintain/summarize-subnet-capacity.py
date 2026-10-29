#!/usr/bin/env python3
"""Summarize FRAME raw results without confusing nanoseconds, weight or proof estimates.

Usage: python3 .maintain/summarize-subnet-capacity.py results.json[.gz] > measurements.csv
The proof column is the recorded proof, not the generated worst-case trie estimate.
Timing is Wasm execution with benchmark storage, not end-to-end block import.
"""
import csv
import gzip
import json
import math
import statistics
import sys
from collections import defaultdict
from pathlib import Path


def summarize(path):
    opener = gzip.open if path.suffix == ".gz" else open
    with opener(path, "rt") as handle:
        batches = json.load(handle)
    for batch in batches:
        times, dbs = defaultdict(list), defaultdict(list)
        for row in batch["time_results"]:
            times[tuple(map(tuple, row["components"]))].append(row)
        for row in batch["db_results"]:
            dbs[tuple(map(tuple, row["components"]))].append(row)
        for components, rows in sorted(times.items()):
            values = sorted(row["extrinsic_time"] / 1_000_000 for row in rows)
            tracked = dbs[components]
            if not tracked:
                raise ValueError(f"Missing DB result for {batch['benchmark']} {components}")
            yield [
                batch["benchmark"], ";".join(f"{key}={value}" for key, value in components),
                len(values), f"{statistics.median(values):.6f}",
                f"{values[math.ceil(0.95 * len(values)) - 1]:.6f}", f"{max(values):.6f}",
                max(row["reads"] for row in tracked), max(row["writes"] for row in tracked),
                max(row["proof_size"] for row in rows + tracked),
            ]


if __name__ == "__main__":
    writer = csv.writer(sys.stdout)
    writer.writerow(["benchmark", "components", "samples", "median_ms", "p95_ms", "max_ms",
                     "unique_reads", "unique_writes", "recorded_proof_bytes"])
    writer.writerows(summarize(Path(sys.argv[1])))
