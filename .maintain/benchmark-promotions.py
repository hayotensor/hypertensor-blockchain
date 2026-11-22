#!/usr/bin/env python3
"""Measure promotion capacity without installing weights.

Usage: python3 .maintain/benchmark-promotions.py /tmp/new-promotion-run
       python3 .maintain/benchmark-promotions.py /tmp/existing-run --resume

Each scenario uses two equivalent parameter values, avoiding FRAME's single-value
regression error. Full baseline sweeps share a conservative envelope from FRAME's
estimated proof model over that population. Timing always uses individual samples,
never a linear fit across cases. These research estimates are not production bounds.
The full run measures every partition at 256/512 nodes plus boundary profiles.
Use --cases 253,763 for a diagnostic subset (reported as incomplete).
"""
import argparse
import csv
import hashlib
import importlib.util
import json
import math
import platform
import shlex
import statistics
import subprocess
import tarfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("generated", ROOT / ".maintain/capacity-generated-model.py")
GENERATED = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GENERATED)
METHODS = ("capacity_promotion_settlement", "capacity_promotion_on_initialize")
SINGLE = "capacity_promotion_single"
PROFILES = ["ready", "recovery", "progress", "omission", "shared", "queue", "shared_queue", "departure", "mixed_progress", "mixed_omission"]


def run(command, directory, log):
    with (directory / "commands.txt").open("a") as commands:
        commands.write(shlex.join(map(str, command)) + "\n")
    with (directory / log).open("w") as output:
        subprocess.run(command, cwd=ROOT, stdout=output, stderr=subprocess.STDOUT, check=True)


def source_hashes():
    paths = [Path("Cargo.lock"), Path("rust-toolchain.toml")]
    for root in ["pallets/network/src", "runtime/src", ".maintain"]:
        paths.extend(p.relative_to(ROOT) for p in (ROOT / root).rglob("*")
                     if p.is_file() and "__pycache__" not in p.parts)
    return {str(p): hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in sorted(paths)}


def manifest(log):
    cases = {}
    limit = None
    for line in log.splitlines():
        if line.startswith("promotion_limit,"):
            ref_time, proof, last = map(int, line.split(",")[1:])
            limit = dict(ref_time_ps=ref_time, proof_bytes=proof, last_case=last)
        elif line.startswith("promotion_case,"):
            c, n, p, profile, promotions, ref_time, proof, queued = map(int, line.split(",")[1:])
            cases[c] = dict(nodes=n, probation=p, profile=profile, promotions=promotions,
                            reservation_ref_time_ps=ref_time, reservation_proof_bytes=proof,
                            queued=queued)
    if limit is None or set(cases) != set(range(limit["last_case"] + 1)):
        raise ValueError("Incomplete Rust case manifest")
    for nodes in (256, 512):
        if {v["probation"] for v in cases.values() if v["nodes"] == nodes and v["profile"] == 0} != set(range(nodes - 2)):
            raise ValueError(f"Missing baseline partition at {nodes} nodes")
    return limit, cases


def read_group(path, weights, parameter, expected, methods, minimum):
    models = GENERATED.models(weights)
    batches = json.loads(path.read_text())
    if {b["benchmark"] for b in batches} != set(methods):
        raise ValueError(f"Missing or unexpected benchmark in {path}")
    rows = []
    for batch in batches:
        model = models[batch["benchmark"]]
        if set(model["slopes"]) - {parameter}:
            raise ValueError("Unexpected model component")
        groups = {kind: {} for kind in ("time_results", "db_results")}
        for kind, group in groups.items():
            for sample in batch[kind]:
                if len(sample["components"]) != 1 or sample["components"][0][0] != parameter:
                    raise ValueError(f"Wrong endpoint in {path}")
                encoded = sample["components"][0][1]
                group.setdefault(encoded // 2, []).append(sample)
            if set(group) != set(expected):
                raise ValueError(f"Wrong endpoint or incomplete coverage in {path}")
        slope = model["slopes"].get(parameter, [0, 0])
        envelope = max(model["base"][1] + slope[1] * endpoint
                       for endpoint in (2 * min(expected), 2 * max(expected) + 1))
        for value in sorted(expected):
            timed, db = (groups[kind][value] for kind in ("time_results", "db_results"))
            if len(timed) < minimum or not db:
                raise ValueError(f"Insufficient samples in {path}")
            if {r["components"][0][1] for r in timed} != {2 * value, 2 * value + 1}:
                raise ValueError(f"Wrong endpoint in {path}")
            times = sorted(r["extrinsic_time"] / 1_000_000 for r in timed)
            recorded = max(r["proof_size"] for r in timed + db)
            if envelope < recorded:
                raise ValueError(f"Estimated proof below recorded proof in {path}")
            row = dict(value=value, benchmark=batch["benchmark"], samples=len(times),
                       median_ms=statistics.median(times), p95_ms=times[math.ceil(.95 * len(times)) - 1],
                       max_ms=max(times), recorded_proof_bytes=recorded,
                       estimated_proof_bytes=envelope,
                       proof_estimate_scope="population envelope" if len(expected) > 1 else "individual case")
            for key in ("reads", "repeat_reads", "writes", "repeat_writes"):
                row[key] = max(r[key] for r in db)
            row["max_with_db_ms"] = row["max_ms"] + row["reads"] * .025 + row["writes"] * .1
            rows.append(row)
    return rows


def read_results(path, weights, parameter, value, methods, minimum):
    return read_group(path, weights, parameter, [value], methods, minimum)


def summarize(directory, limit, cases, minimum):
    rows = []
    complete = set()
    jobs = json.loads((directory / "jobs.json").read_text())
    single = []
    for job in jobs:
        stem, parameter, first, last, methods = job
        if not (directory / f"{stem}.done").exists():
            continue
        measured = read_group(directory / f"{stem}.json", directory / f"{stem}.rs", parameter, range(first, last + 1), methods, minimum)
        for row in measured:
            c = row.pop("value")
            if parameter == "e":
                single.append(dict(election_list_before=c, **row))
                continue
            scenario = cases[c]
            row.update(case=c, **scenario)
            row["profile_name"] = PROFILES[row["profile"]]
            row["reference_headroom_ms"] = limit["ref_time_ps"] / 1e9 - row["max_with_db_ms"]
            row["proof_headroom_bytes"] = limit["proof_bytes"] - row["estimated_proof_bytes"]
            row["reservation_time_headroom_ms"] = (limit["ref_time_ps"] - row["reservation_ref_time_ps"]) / 1e9
            row["reservation_proof_headroom_bytes"] = limit["proof_bytes"] - row["reservation_proof_bytes"]
            # This is meaningful for full-hook samples; settlement alone is not a slot budget.
            row["observed_above_reservation"] = row["benchmark"] == METHODS[1] and (
                row["max_with_db_ms"] > row["reservation_ref_time_ps"] / 1e9 or
                row["estimated_proof_bytes"] > row["reservation_proof_bytes"])
            rows.append(row)
            complete.add(c)
    if rows:
        with (directory / "measurements.csv").open("w") as output:
            writer = csv.DictWriter(output, fieldnames=list(rows[0]))
            writer.writeheader()
            writer.writerows(rows)
    if single:
        with (directory / "individual-promotions.csv").open("w") as output:
            writer = csv.DictWriter(output, fieldnames=list(single[0]))
            writer.writeheader()
            writer.writerows(single)
    hooks = [r for r in rows if r["benchmark"] == METHODS[1]]
    lines = ["Promotion capacity research — production weights unchanged",
             f"Completed {len(complete)}/{len(cases)} settlement + hook cases; {len(single)} individual-promotion list sizes.",
             f"Budget: {limit['ref_time_ps'] / 1e9:.3f} ms reference time; {limit['proof_bytes']} proof bytes.",
             "Measurements are local Wasm execution, not block import. Reference-time fits include RocksDB charges.",
             "Time uses individual raw samples, never a linear fit across case IDs or quadratic promotion costs.",
             "Baseline proof estimates use the maximum of FRAME's proof model across each population sweep; boundary cases are estimated individually.",
             "Recorded proof and estimated trie proof are separate. Statistical proof envelopes are research estimates, not certified production bounds.",
             "Current reservations come from the unchanged admission helper with production-sized bounds and RocksDB costs.",
             "The mandatory reservation excludes optional maintenance; full hooks include whatever maintenance executes.",
             "Promotion helpers are diagnostic only; do not add their cost to the measured full hook.", ""]
    for n in (256, 512):
        group = [r for r in hooks if r["nodes"] == n]
        if not group:
            continue
        slow = max(group, key=lambda r: r["max_with_db_ms"])
        proof = max(group, key=lambda r: r["estimated_proof_bytes"])
        lines += [f"{n} nodes: largest verified batch {max(r['promotions'] for r in group)} promotions.",
                  f"  Slowest tested mix: case {slow['case']}, {slow['profile_name']}, {slow['promotions']} promotions; max {slow['max_ms']:.3f} ms, with DB charges {slow['max_with_db_ms']:.3f} ms; time headroom {slow['reference_headroom_ms']:.3f} ms.",
                  f"  Largest estimated proof: case {proof['case']}, {proof['estimated_proof_bytes']} bytes; proof headroom {proof['proof_headroom_bytes']} bytes.",
                  f"  Mandatory reservation: up to {max(r['reservation_ref_time_ps'] for r in group)/1e9:.3f} ms and {max(r['reservation_proof_bytes'] for r in group)} bytes."]
    breaches = [r for r in hooks if r["reference_headroom_ms"] < 0 or r["proof_headroom_bytes"] < 0]
    under_reserved = [r for r in hooks if r["observed_above_reservation"]]
    lines += ["", f"Measured hook budget breaches: {len(breaches)}; samples above the current mandatory reservation: {len(under_reserved)}."]
    if complete != set(cases):
        lines.append("INCOMPLETE: conclusions apply only to completed cases; remaining cases are listed in missing-cases.json.")
    elif breaches:
        lines.append("A measured budget breach requires follow-up: investigate cost reduction or a measured promotion limit before changing production weights.")
    elif under_reserved:
        lines.append("The tested workloads fit the hook budget, but reservation coverage needs a separate weight audit, distinguishing optional maintenance from mandatory work.")
    else:
        lines.append("The tested workloads fit the hook budget and current mandatory reservation. These measurements do not establish a need for a promotion cap; production weight coverage still requires a separate conservative-model review.")
    lines += ["", "Scope of the 253/509 endpoints: ordinary qualifying rounds contain at least three Active identities at proposal/attestation; promotion uses the frozen Probation membership. Departures after attestation do not add new historical Probation entries. No universal throughput ceiling is extrapolated from these fixtures.",
              "Shared pools can grow from Active-node rewards while owning Probation nodes; promoted nodes' direct stake stays unchanged and per-node reward events must exclude them."]
    (directory / "report.txt").write_text("\n".join(lines) + "\n")
    (directory / "missing-cases.json").write_text(json.dumps(sorted(set(cases) - complete)) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--cases", help="Comma-separated case IDs; default is the full manifest")
    parser.add_argument("--repeat", type=int, default=10, help="Minimum timed samples per actual configuration (split over its two equivalent parameter values)")
    args = parser.parse_args()
    if args.repeat < 10:
        parser.error("At least ten repetitions are required")
    directory = args.directory.resolve()
    wasm = ROOT / "target/release/wbuild/hypertensor-runtime/hypertensor_runtime.compact.compressed.wasm"
    node = ROOT / "target/release/hypertensor-node"
    if not args.resume:
        directory.mkdir()  # Deliberately fail rather than overwrite evidence.
        run(["cargo", "build", "--release", "--locked", "--offline", "-p", "hypertensor-runtime", "--features", "runtime-benchmarks"], directory, "build.log")
        hashes = source_hashes()
        (directory / "source.sha256.json").write_text(json.dumps(hashes, indent=2))
        with tarfile.open(directory / "sources.tar.gz", "w:gz") as archive:
            for path in hashes:
                archive.add(ROOT / path, arcname=path)
            archive.add(wasm, arcname="runtime.wasm")
        run(["git", "rev-parse", "HEAD"], directory, "commit.txt")
        run(["git", "diff", "--", "pallets/network", "runtime", ".maintain"], directory, "source.patch")
        for command, log in [(["rustc", "--version"], "rustc.txt"), ([str(node), "--version"], "bencher.txt"), (["lscpu"], "cpu.txt"), (["free", "-h"], "memory.txt")]:
            run(command, directory, log)
        (directory / "kernel.txt").write_text(platform.platform() + "\n")
        provenance = dict(wasm_sha256=hashlib.sha256(wasm.read_bytes()).hexdigest(), node_sha256=hashlib.sha256(node.read_bytes()).hexdigest(), repeat=args.repeat)
        (directory / "provenance.json").write_text(json.dumps(provenance, indent=2))
        run(["cargo", "test", "--release", "--locked", "--offline", "-p", "hypertensor-runtime", "--features", "runtime-benchmarks", "promotion_budget_matches_research_configuration", "--", "--nocapture"], directory, "runtime-budget.log")
        run(["cargo", "test", "--release", "--locked", "--offline", "-p", "pallet-network", "--features", "runtime-benchmarks", "promotion_capacity", "--", "--nocapture"], directory, "manifest.log")
    else:
        provenance = json.loads((directory / "provenance.json").read_text())
        if source_hashes() != json.loads((directory / "source.sha256.json").read_text()) or provenance != dict(wasm_sha256=hashlib.sha256(wasm.read_bytes()).hexdigest(), node_sha256=hashlib.sha256(node.read_bytes()).hexdigest(), repeat=args.repeat):
            raise ValueError("Source/runtime/bencher/repetition mismatch: start a fresh run")
    limit, cases = manifest((directory / "manifest.log").read_text())
    runtime_rows = [line.split(",")[1:] for line in (directory / "runtime-budget.log").read_text().splitlines()
                    if line.startswith("promotion_runtime_reservation,")]
    if len(runtime_rows) != 1:
        raise ValueError("Missing real runtime budget test result")
    runtime_time, runtime_proof, _ = map(int, runtime_rows[0])
    for scenario in cases.values():
        if scenario["nodes"] == 512 and scenario["queued"] == 64:
            if (scenario["reservation_ref_time_ps"], scenario["reservation_proof_bytes"]) != (runtime_time, runtime_proof):
                raise ValueError("Pallet reservation and real runtime reservation differ")
    selected = set(map(int, args.cases.split(","))) if args.cases else set(cases)
    if not selected <= set(cases):
        raise ValueError("Unknown case ID")
    # Measure important boundaries first, then fill every intermediate partition.
    if selected == set(cases):
        experiments = [("population-256", "c", 0, 253, METHODS),
                       ("population-512", "c", 254, 763, METHODS)]
        ordered = list(range(773, limit["last_case"] + 1)) + list(range(764, 773))
    else:
        experiments = []
        ordered = sorted(selected)
    experiments += [(f"case-{c:04d}", "c", c, c, METHODS) for c in ordered]
    sizes = sorted({3, 255, 511, *(3 + (508 * i // 19) for i in range(20))})
    experiments += [(f"single-{e:04d}", "e", e, e, [SINGLE]) for e in sizes]
    (directory / "jobs.json").write_text(json.dumps(experiments, indent=2))
    try:
        for i, (stem, parameter, first, last, methods) in enumerate(experiments):
            if (directory / f"{stem}.done").exists():
                continue
            started = time.monotonic()
            run([str(node), "benchmark", "pallet", "--runtime", str(wasm), "--genesis-builder", "runtime", "--pallets", "pallet_network", "--extrinsic", ",".join(methods), "--extra", "--low", str(2 * first), "--high", str(2 * last + 1), "--steps", str(2 * (last - first + 1)), "--repeat", str((args.repeat + 1) // 2), "--min-duration", "0", "--log", "info", "--output-analysis", "max", "--template", ".maintain/frame-weight-template.hbs", "--output", str(directory / f"{stem}.rs"), "--json-file", str(directory / f"{stem}.json")], directory, f"{stem}.log")
            read_group(directory / f"{stem}.json", directory / f"{stem}.rs", parameter, range(first, last + 1), methods, args.repeat)
            (directory / f"{stem}.done").touch()
            print(f"{i+1}/{len(experiments)} {stem}: {time.monotonic()-started:.1f}s", flush=True)
            if i % 20 == 0:
                summarize(directory, limit, cases, args.repeat)
    finally:
        summarize(directory, limit, cases, args.repeat)
    print(directory / "report.txt", flush=True)


if __name__ == "__main__":
    main()
