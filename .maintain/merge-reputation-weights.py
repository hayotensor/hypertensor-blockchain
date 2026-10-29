#!/usr/bin/env python3
"""Install only reputation-related weights, retaining every existing manual addition.

Usage: merge-reputation-weights.py GENERATED AUDIT_JSON
"""
import hashlib
import json
import re
import sys
from pathlib import Path

METHODS = {
    'emission_step', 'emission_step_accepted_below_min_weight_reputation',
    'emission_step_accepted_non_attestor_reputation', 'emission_step_rejected',
    'emission_step_emergency', 'emission_step_missing', 'precheck_subnet_consensus_submission',
    'precheck_subnet_consensus_submission_missing',
}
METHOD = re.compile(r'(?:\t///[^\n]*\n)*\tfn (\w+)\([^\n]*\) -> Weight \{.*?\n\t\}', re.S)
MANUAL = re.compile(r'^\t\t\t\.saturating_add\((?!Weight::from_parts|(?:T::DbWeight|RocksDbWeight)::get).*$', re.M)


def main(generated_path, audit_path):
    generated = Path(generated_path).read_text()
    replacements = {}
    for match in METHOD.finditer(generated):
        replacements.setdefault(match[1], []).append(match[0])
    assert set(replacements) == METHODS
    assert all(len(v) == 2 for v in replacements.values())
    destination = Path('pallets/network/src/weights.rs')
    before = destination.read_text()
    counts = dict.fromkeys(METHODS, 0)

    def replace(match):
        name = match[1]
        if name not in METHODS:
            return match[0]
        replacement = replacements[name][counts[name]]
        counts[name] += 1
        additions = MANUAL.findall(match[0])
        if additions:
            replacement = replacement[:-3] + '\n' + '\n'.join(additions) + '\n\t}'
        return replacement

    after = METHOD.sub(replace, before)
    assert all(n == 2 for n in counts.values())
    unrelated_before = [m[0] for m in METHOD.finditer(before) if m[1] not in METHODS]
    unrelated_after = [m[0] for m in METHOD.finditer(after) if m[1] not in METHODS]
    assert unrelated_before == unrelated_after
    assert MANUAL.findall(before) == MANUAL.findall(after)
    Path(audit_path).write_text(json.dumps({
        'methods': sorted(METHODS), 'implementations_per_method': 2,
        'unchanged_method_blocks': len(unrelated_before),
        'preserved_manual_additions': MANUAL.findall(before),
        'before_sha256': hashlib.sha256(before.encode()).hexdigest(),
        'after_sha256': hashlib.sha256(after.encode()).hexdigest(),
    }, indent=2) + '\n')
    destination.write_text(after)
    print('Updated eight methods in both implementations; unrelated methods and manual additions unchanged')


if __name__ == '__main__':
    main(*sys.argv[1:])
