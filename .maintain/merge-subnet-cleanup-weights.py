#!/usr/bin/env python3
"""Install fixed removal models and per-stage maxima; never fit an affine cleanup-stage cost."""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
METHOD = re.compile(r"(?:(?:\t| {4})///[^\n]*\n)*(?:\t| {4})fn (\w+)\([^\n]*\) -> Weight \{.*?\n(?:\t| {4})\}", re.S)
FIXED = {
    "do_remove_subnet", "owner_deactivate_subnet", "collective_remove_subnet", "activate_subnet",
    "do_epoch_preliminaries", "subnet_existence_check", "removed_subnet_cleanup_selector",
    "removed_subnet_cleanup_progress",
}


def body(method):
    return method.split("{", 1)[1].rsplit("\n\t}", 1)[0].strip("\n")


def merge(source, generated, results):
    methods = {}
    for m in METHOD.finditer(generated):
        methods.setdefault(m[1], []).append(m[0])
    required = FIXED | {"activate_subnet_success"}
    if set(methods) != required or any(len(x) != 2 for x in methods.values()):
        raise ValueError("Expected both implementations of every removal/cleanup measurement")
    stages = {}
    coverage = {name: set() for name in ["removed_subnet_cleanup_step", "removed_subnet_cleanup_empty_step"]}
    for result in results:
        if result["benchmark"] not in coverage:
            continue
        for category in ["time_results", "db_results"]:
            for sample in result[category]:
                stage = dict(sample["components"])["s"]
                coverage[result["benchmark"]].add((category, stage))
                model = stages.setdefault(stage, [0, 0, 2, 1])
                # Execution samples are nanoseconds. Charge their maximum, plus 10% and a 1us
                # loop/dispatch margin. A 4KiB trie allowance supplements recorded proofs.
                model[0] = max(model[0], sample["extrinsic_time"] * 1100 + 1_000_000)
                model[1] = max(model[1], sample["proof_size"] + 4096)
                model[2] = max(model[2], sample["reads"])
                model[3] = max(model[3], sample["writes"])
    expected = {(category, stage) for category in ["time_results", "db_results"] for stage in range(63)}
    if any(seen != expected for seen in coverage.values()):
        raise ValueError("Every stage needs populated and empty timing/storage samples; run with --steps 63")
    counts = {name: 0 for name in FIXED | {"removed_subnet_cleanup_step"}}

    def replace(m):
        name = m[1]
        if name not in counts:
            return m[0]
        index = counts[name]
        counts[name] += 1
        if name == "removed_subnet_cleanup_step":
            db = "T::DbWeight" if index == 0 else "RocksDbWeight"
            lines = ["\t/// Per-stage maximum of populated and empty steps, including execution/trie margins.",
                     "\tfn removed_subnet_cleanup_step(s: u32) -> Weight {", "\t\tlet (time, proof, reads, writes) = match s {"]
            for stage, (time, proof, reads, writes) in sorted(stages.items()):
                lines.append(f"\t\t\t{stage} => ({time}, {proof}, {reads}, {writes}),")
            worst = tuple(max(x[i] for x in stages.values()) for i in range(4))
            lines += [f"\t\t\t_ => {worst},", "\t\t};", f"\t\tWeight::from_parts(time, proof).saturating_add({db}::get().reads_writes(reads, writes))", "\t}"]
            return "\n".join(lines)
        measured = methods[name][index]
        if name == "activate_subnet":
            alternate = methods["activate_subnet_success"][index]
            return measured[:measured.index("{") + 1] + "\n\t\tlet removal = {\n" + body(measured) + "\n\t\t};\n\t\tlet activation = {\n" + body(alternate) + "\n\t\t};\n\t\tremoval.max(activation)\n\t}"
        if name == "removed_subnet_cleanup_progress":
            return measured[:-3] + "\n\t\t\t// Cover cursor deletion as well as updates, with a trie/dispatch allowance.\n\t\t\t.saturating_add(Weight::from_parts(1_000_000, 4096))\n\t}"
        return measured

    merged = METHOD.sub(replace, source)
    if any(x != 2 for x in counts.values()):
        raise ValueError(f"Unexpected implementation counts: {counts}")
    return merged


if __name__ == "__main__":
    artifacts = Path(sys.argv[1])
    destination = ROOT / "pallets/network/src/weights.rs"
    destination.write_text(merge(destination.read_text(), (artifacts / "weights.rs").read_text(), json.loads((artifacts / "results.json").read_text())))
    print("Installed both removal models and all 63 cleanup stage envelopes")
