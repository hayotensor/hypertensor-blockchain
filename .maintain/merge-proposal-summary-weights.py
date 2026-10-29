#!/usr/bin/env python3
"""Install only proposal-summary weights, retaining every existing manual addition.

Usage: merge-proposal-summary-weights.py GENERATED AUDIT_JSON BASELINE_WEIGHTS
"""
import hashlib
import json
import re
import sys
from pathlib import Path

METHODS = {
    'propose_attestation', 'propose_attestation_emergency', 'attest',
    'emission_step', 'emission_step_rejected', 'emission_step_emergency',
    'precheck_subnet_consensus_submission',
}
METHOD = re.compile(r'(?:\t///[^\n]*\n)*\tfn (\w+)\([^\n]*\) -> Weight \{.*?\n\t\}', re.S)
MANUAL = re.compile(r'^\t\t\t\.saturating_add\((?!Weight::from_parts|(?:T::DbWeight|RocksDbWeight)::get).*$', re.M)


def main(generated_path, audit_path, baseline_path):
    generated = Path(generated_path).read_text()
    replacements = {}
    for match in METHOD.finditer(generated):
        replacements.setdefault(match[1], []).append(match[0])
    assert set(replacements) == METHODS
    assert all(len(v) == 2 for v in replacements.values())
    destination = Path('pallets/network/src/weights.rs')
    before = destination.read_text()
    counts = dict.fromkeys(METHODS, 0)
    baseline = Path(baseline_path).read_text()
    time_baselines = {name: [m[0] for m in METHOD.finditer(baseline) if m[1] == name]
                      for name in ('emission_step', 'attest')}
    assert all(len(parts) == 2 for parts in time_baselines.values())
    floors = []

    def replace(match):
        name = match[1]
        if name not in METHODS:
            return match[0]
        replacement = replacements[name][counts[name]]
        if name in time_baselines:
            # Capacity sweeps exceeded the fresh healthy-hook and final-vote fits.
            # Retain their prior CPU envelopes, with freshly measured proofs/DB terms.
            old_parts = re.findall(r'Weight::from_parts\(([\d_]+),\s*([\d_]+)\)', time_baselines[name][counts[name]])
            new_parts = re.findall(r'Weight::from_parts\(([\d_]+),\s*([\d_]+)\)', replacement)
            assert len(old_parts) == len(new_parts)
            index = 0

            def retain_time(match):
                nonlocal index
                old_time = int(old_parts[index][0].replace('_', ''))
                new_time = int(match[1].replace('_', ''))
                index += 1
                floors.append(dict(method=name, implementation=counts[name], term=index,
                                   generated=new_time, baseline=old_time,
                                   installed=max(old_time, new_time)))
                return f'Weight::from_parts({max(old_time, new_time):_}, {match[2]})'

            replacement = re.sub(r'Weight::from_parts\(([\d_]+),\s*([\d_]+)\)', retain_time, replacement)
            replacement = replacement.replace('\n\t\tWeight::from_parts',
                '\n\t\t// Retain baseline CPU coefficients: the fresh fit did not cover capacity measurements.\n\t\tWeight::from_parts', 1)
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
        'baseline_sha256': hashlib.sha256(baseline.encode()).hexdigest(),
        'time_floors': floors,
    }, indent=2) + '\n')
    destination.write_text(after)
    print('Updated seven methods in both implementations; unrelated methods and manual additions unchanged')


if __name__ == '__main__':
    main(*sys.argv[1:])
