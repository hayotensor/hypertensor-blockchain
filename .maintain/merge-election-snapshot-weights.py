#!/usr/bin/env python3
"""Merge measured election snapshot weights while retaining prior envelopes.

Usage: merge-election-snapshot-weights.py GENERATED AUDIT_JSON BASELINE_WEIGHTS
Both election profiles are combined using Weight::max. Previous measured bounds
remain floors, and the complete manual suffix (including its DB charges) survives.
"""
import hashlib
import json
import re
import sys
from pathlib import Path

METHODS = {
    "elect_validator", "propose_attestation", "precheck_subnet_consensus_submission",
    "emission_step", "emission_step_rejected", "emission_step_missing",
    "precheck_subnet_consensus_submission_missing",
}
METHOD = re.compile(r"(?:\t///[^\n]*\n)*\tfn (\w+)\([^\n]*\) -> Weight \{.*?\n\t\}", re.S)
MANUAL = re.compile(r"^\t\t\t\.saturating_add\((?!Weight::from_parts|(?:T::DbWeight|RocksDbWeight)::get).*$", re.M)
ALLOWANCES_BEGIN = "// BEGIN ELECTION_SNAPSHOT_MANUAL_ALLOWANCES"
ALLOWANCES_END = "// END ELECTION_SNAPSHOT_MANUAL_ALLOWANCES"


def expression(block):
    start = block.index("\t\tWeight::from_parts")
    return block[start:-3].strip()


def main(generated_path, audit_path, baseline_path):
    generated = Path(generated_path).read_text()
    baseline = Path(baseline_path).read_text()
    destination = Path("pallets/network/src/weights.rs")
    before = destination.read_text()
    generated_methods = {}
    baseline_methods = {}
    for match in METHOD.finditer(generated):
        generated_methods.setdefault(match[1], []).append(match[0])
    for match in METHOD.finditer(baseline):
        baseline_methods.setdefault(match[1], []).append(match[0])
    assert set(generated_methods) == METHODS | {"elect_validator_multi_node"}
    assert all(len(blocks) == 2 for blocks in generated_methods.values())
    counts = dict.fromkeys(METHODS, 0)
    preserved = []

    def replace(match):
        name = match[1]
        if name not in METHODS:
            return match[0]
        index = counts[name]
        counts[name] += 1
        old = baseline_methods[name][index]
        if ALLOWANCES_BEGIN in old:
            suffix = old.split(ALLOWANCES_BEGIN, 1)[1].split(ALLOWANCES_END, 1)[0].strip()
        else:
            manual = MANUAL.search(old)
            suffix = old[manual.start():-3].strip() if manual else ""
        fresh = generated_methods[name][index]
        head = fresh[:fresh.index("\t\tWeight::from_parts")]
        body = head + "\t\tlet measured = " + expression(fresh) + ";\n"
        if name == "elect_validator":
            shared = expression(generated_methods["elect_validator_multi_node"][index])
            body += "\t\tlet shared_identities = " + shared + ";\n"
        body += "\t\tlet measured = measured"
        if name == "elect_validator":
            body += ".max(shared_identities)"
        if suffix:
            body += "\n\t\t\t" + ALLOWANCES_BEGIN
            body += "\n\t\t\t" + suffix
            body += "\n\t\t\t" + ALLOWANCES_END
        body += "\n\t\t;\n"
        body += "\t\t// Preserve the complete prior envelope, including its safety allowances.\n"
        old_body = old[old.index("-> Weight {") + len("-> Weight {"):-3].strip()
        body += "\t\tlet baseline = {\n\t\t\t" + old_body + "\n\t\t};\n"
        body += "\t\tmeasured.max(baseline)"
        body += "\n\t}"
        preserved.append({"method": name, "implementation": index, "manual_suffix": suffix})
        return body

    after = METHOD.sub(replace, before)
    assert all(count == 2 for count in counts.values())
    assert [m[0] for m in METHOD.finditer(before) if m[1] not in METHODS] == [
        m[0] for m in METHOD.finditer(after) if m[1] not in METHODS
    ]
    Path(audit_path).write_text(json.dumps({
        "methods": sorted(METHODS),
        "preserved": preserved,
        "before_sha256": hashlib.sha256(before.encode()).hexdigest(),
        "after_sha256": hashlib.sha256(after.encode()).hexdigest(),
        "baseline_sha256": hashlib.sha256(baseline.encode()).hexdigest(),
    }, indent=2) + "\n")
    destination.write_text(after)
    print("Updated seven methods in both implementations; prior bounds and manual suffixes retained")


if __name__ == "__main__":
    main(*sys.argv[1:])
