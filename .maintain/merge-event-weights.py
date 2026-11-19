#!/usr/bin/env python3
"""Merge event benchmarks while retaining prior weight bounds and manual safety allowances."""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = Path(__file__).with_name("benchmark-event-weights.sh")
METHODS = set(re.search(r"^methods=(.+)$", SCRIPT.read_text(), re.M)[1].split(","))
ALTERNATIVES = {
    "elect_validator": "elect_validator_multi_node",
    "remove_overwatch_node": "remove_overwatch_node_last_pending",
}
PRIMARY = METHODS - set(ALTERNATIVES.values())
METHOD = re.compile(r"(?:\t///[^\n]*\n)*\tfn (\w+)\([^\n]*\) -> Weight \{.*?\n\t\}", re.S)
SAFETY = re.compile(
    r"^\s*(\.saturating_add\((?:(?:subnet_node_registration_collision_proof|"
    r"conservative_storage_key_proof|consensus_round_settlement_proof)\(\)|"
    r"Self::pending_(?:admitted|queued)_removal_scan\(MAX_PENDING_(?:ADMITTED|QUEUED)_REMOVALS\)).*?)\s*;?\s*$",
    re.M,
)


def body(method):
    return method.split("{", 1)[1].rsplit("\n\t}", 1)[0].strip("\n")


def merge(source, generated):
    methods = {}
    for match in METHOD.finditer(generated):
        methods.setdefault(match[1], []).append(match[0])
    if set(methods) != METHODS or any(len(v) != 2 for v in methods.values()):
        raise ValueError("Expected every requested event benchmark in both weight implementations")
    counts = dict.fromkeys(PRIMARY, 0)

    def replace(match):
        name = match[1]
        if name not in PRIMARY:
            return match[0]
        index = counts[name]
        measured = methods[name][index]
        counts[name] += 1
        signature = (
            measured[: measured.index("\tfn ")]
            + match[0][match[0].index("\tfn "): match[0].index("{") + 1]
        )
        # Preserve the original argument names used by the retained bound.
        old_names = re.findall(r"(\w+): u32", signature)
        new_names = re.findall(r"(\w+): u32", measured.split("{", 1)[0])
        measured_body = body(measured)
        for old, new in zip(old_names, new_names):
            measured_body = re.sub(r"\b" + new + r"\b", old, measured_body)
        result = signature + "\n\t\t// Event-inclusive benchmark; retain the prior bound componentwise.\n"
        result += "\t\tlet measured = {\n" + measured_body + "\n\t\t};\n"
        if name in ALTERNATIVES:
            alternate = body(methods[ALTERNATIVES[name]][index])
            result += "\t\tlet alternate = {\n" + alternate + "\n\t\t};\n"
            result += "\t\tlet measured = measured.max(alternate);\n"
        # Apply each manually composed safety allowance to the new measurement too.
        allowances = list(dict.fromkeys(line.rstrip(";") for line in SAFETY.findall(match[0])))
        if allowances:
            result += "\t\tlet measured = measured\n"
            result += "\n".join("\t\t\t" + line for line in allowances) + ";\n"
        result += "\t\tlet baseline = {\n" + body(match[0]) + "\n\t\t};\n"
        result += "\t\tmeasured.max(baseline)\n\t}"
        return result

    merged = METHOD.sub(replace, source)
    if any(n != 2 for n in counts.values()):
        raise ValueError(f"Expected both existing implementations: {counts}")
    return merged


if __name__ == "__main__":
    destination = ROOT / "pallets/network/src/weights.rs"
    destination.write_text(merge(destination.read_text(), Path(sys.argv[1]).read_text()))
    print(f"Updated {len(PRIMARY)} weight methods; retained prior bounds and manual safety allowances")
