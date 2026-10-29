#!/usr/bin/env python3
"""Compare matched proposal-summary sweeps and verify installed reservation coverage."""
import ast
import csv
import importlib.util
import json
import re
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location('capacity_summary', Path(__file__).with_name('analyze-reputation.py'))
summary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(summary)


def write_csv(path, rows):
    with path.open('w') as handle:
        writer = csv.DictWriter(handle, fieldnames=rows[0].keys())
        writer.writeheader()
        writer.writerows(rows)


def main(root):
    before, after = (summary.summarize(root / phase) for phase in ('before', 'after'))
    assert before.keys() == after.keys()
    rows = []
    for key, old in before.items():
        new = after[key]
        assert old['samples'] == new['samples'] == 10
        rows.append(dict(benchmark=key[0], components=key[1],
                         **{'before_' + k: v for k, v in old.items()},
                         **{'after_' + k: v for k, v in new.items()}))
    write_csv(root / 'comparison.csv', rows)
    reports = {phase: summary.storage_report(root / phase) for phase in ('before', 'after', 'production')}
    forbidden = {'Network::SubnetNodeMetadata', 'Network::ValidatorMetadata'}
    for accesses in reports['after'].values():
        assert forbidden.isdisjoint(a['storage'] for a in accesses)
    (root / 'storage-accesses.json').write_text(json.dumps(reports, indent=2) + '\n')
    production = [dict(benchmark=key[0], components=key[1], **value)
                  for key, value in summary.summarize(root / 'production').items()]
    assert all(row['estimated_proof'] >= row['recorded_proof'] for row in production)
    (root / 'production-measurements.json').write_text(json.dumps(production, indent=2) + '\n')
    admission, outcomes = [], {}
    for phase, path in [('before', root / 'before-validation.log'),
                        ('after', root / 'validation/outcomes.log')]:
        log = summary.read_text(path)
        outcomes[phase] = {case: bytes(ast.literal_eval(state)).hex() for case, state in re.findall(
            r'proposal_summary_outcome,combined=(false|true),nodes=512,state=(\[[^\n]+\])', log)}
        assert set(outcomes[phase]) == {'false', 'true'}
        for kind, n, time, proof in re.findall(r'proposal_summary_weight,(\w+),(\d+),(\d+),(\d+)', log):
            admission.append(dict(phase=phase, kind=kind, nodes=int(n), ref_time_ps=int(time), proof_bytes=int(proof)))
    assert outcomes['before'] == outcomes['after'], outcomes
    (root / 'outcome-comparison.json').write_text(json.dumps(outcomes, indent=2) + '\n')
    write_csv(root / 'admission-model.csv', admission)
    reservations = {(r['kind'], r['nodes']): r for r in admission if r['phase'] == 'after'}
    coverage = []
    for row in rows:
        name, n = row['benchmark'], int(row['components'].split('=')[1])
        if name in ('capacity_settlement', 'capacity_on_initialize',
                    'capacity_settlement_reputation', 'capacity_on_initialize_reputation'):
            reservation = reservations['mandatory_slot', n]
        elif name in ('capacity_proposal', 'capacity_attest_last'):
            reservation = reservations['proposal_dispatch' if name == 'capacity_proposal' else 'attest_dispatch', 512]
        else:
            continue
        measured = round(row['after_max_ms'] * 1_000_000_000) + row['after_reads'] * 25_000_000 + row['after_writes'] * 100_000_000
        assert reservation['ref_time_ps'] >= max(measured, row['after_model_ref_time_ps']), row
        assert reservation['proof_bytes'] >= max(row['after_recorded_proof'], row['after_estimated_proof']), row
        coverage.append(dict(benchmark=name, nodes=n, reserved_ps=reservation['ref_time_ps'],
                             measured_max_plus_unique_db_ps=measured,
                             fitted_reference_ps=row['after_model_ref_time_ps'],
                             reserved_proof=reservation['proof_bytes'],
                             recorded_proof=row['after_recorded_proof'],
                             estimated_proof=row['after_estimated_proof']))
    (root / 'integrated-coverage.json').write_text(json.dumps(coverage, indent=2) + '\n')
    for row in rows:
        if row['components'] == 'h=512':
            print(json.dumps(row))


if __name__ == '__main__':
    main(Path(sys.argv[1]))
