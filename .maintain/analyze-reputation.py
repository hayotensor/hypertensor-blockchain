#!/usr/bin/env python3
"""Summarize matched reputation runs, including repeat accesses omitted by older reports."""
import csv
import ast
import gzip
import json
import math
import re
import statistics
import sys
from collections import defaultdict
from pathlib import Path


def read_text(path):
    if path.exists():
        return path.read_text()
    with gzip.open(str(path) + '.gz', 'rt') as handle:
        return handle.read()


def summarize(directory):
    models = json.loads(read_text(directory / 'models.json'))
    rows = {}
    for batch in json.loads(read_text(directory / 'results.json')):
        times, db = defaultdict(list), defaultdict(list)
        for row in batch['time_results']:
            times[tuple(map(tuple, row['components']))].append(row)
        for row in batch['db_results']:
            db[tuple(map(tuple, row['components']))].append(row)
        for components, samples in sorted(times.items()):
            values = sorted(s['extrinsic_time'] / 1_000_000 for s in samples)
            model = models[batch['benchmark']]
            estimate = model['base'].copy()
            for key, count in components:
                for i in range(2):
                    estimate[i] += model['slopes'][key][i] * count
            result = {
                'samples': len(values), 'median_ms': statistics.median(values),
                'p95_ms': values[math.ceil(.95 * len(values)) - 1], 'max_ms': max(values),
                **{key: max(s[key] for s in db[components]) for key in
                   ('reads', 'repeat_reads', 'writes', 'repeat_writes')},
                'recorded_proof': max(s['proof_size'] for s in samples + db[components]),
                'estimated_proof': estimate[1], 'model_ref_time_ps': estimate[0],
            }
            rows[(batch['benchmark'], ';'.join(f'{k}={v}' for k,v in components))] = result
    return rows


def storage_report(directory):
    source = read_text(directory / 'weights.rs').split('// For backwards compatibility')[0]
    return {m[2]: [dict(storage=name, reads=int(r), writes=int(w))
                   for name, r, w in re.findall(r'Storage: `([^`]+)` \(r:(\d+) w:(\d+)\)', m[1])]
            for m in re.finditer(r'((?:\t///[^\n]*\n)*)\tfn (\w+)\(', source)}


def main(root):
    before, after = (summarize(root / phase) for phase in ('before', 'after'))
    assert before.keys() == after.keys()
    rows = []
    for key, old in before.items():
        new = after[key]
        assert old['samples'] == new['samples'] == 10
        rows.append(dict(benchmark=key[0], components=key[1],
                         **{'before_' + k: v for k,v in old.items()},
                         **{'after_' + k: v for k,v in new.items()}))
    with (root / 'comparison.csv').open('w') as handle:
        writer = csv.DictWriter(handle, fieldnames=rows[0].keys())
        writer.writeheader()
        writer.writerows(rows)
    reports = {phase: storage_report(root / phase) for phase in ('before', 'after', 'production')}
    forbidden = {'Network::SubnetNodeMetadata', 'Network::ValidatorMetadata'}
    for accesses in reports['after'].values():
        assert forbidden.isdisjoint(a['storage'] for a in accesses)
    for name in ('emission_step_accepted_below_min_weight_reputation',
                 'emission_step_accepted_non_attestor_reputation'):
        assert not reports['production'][name], reports['production'][name]
    (root / 'storage-accesses.json').write_text(json.dumps(reports, indent=2) + '\n')
    coverage = [dict(benchmark=key[0], components=key[1], **value)
                for key,value in summarize(root / 'production').items()]
    (root / 'production-measurements.json').write_text(json.dumps(coverage, indent=2) + '\n')
    assert all(row['estimated_proof'] >= row['recorded_proof'] for row in coverage)
    # The installed accepted reservation also adds queue work and manual safety allowances.
    # Even its generated base alone must cover the recorded integrated settlement/hook proofs.
    production = json.loads(read_text(root / 'production/models.json'))
    base = production['emission_step']
    integrated = []
    for row in rows:
        h = int(row['components'].split('=')[1])
        lower_bound = base['base'][1] + h * base['slopes']['h'][1]
        assert lower_bound >= row['after_recorded_proof']
        integrated.append(dict(benchmark=row['benchmark'], nodes=h,
                               accepted_base_proof=lower_bound,
                               recorded_proof=row['after_recorded_proof'],
                               margin_before_other_reservations=lower_bound-row['after_recorded_proof']))
    (root / 'integrated-proof-coverage.json').write_text(json.dumps(integrated, indent=2) + '\n')
    # For the integrated three-identity penalty fixture, even the generated healthy base
    # plus local arithmetic covers both the fitted full-hook cost and the maximum measured
    # execution plus unique DB charges. Queue, election and manual allowances only add margin.
    penalties = []
    for row in rows:
        if not row['benchmark'].endswith('_reputation'):
            continue
        h = int(row['components'].split('=')[1])
        reference = base['base'][0] + h * base['slopes']['h'][0]
        for name, component, count in [
            ('emission_step_accepted_below_min_weight_reputation', 'h', h),
            ('emission_step_accepted_non_attestor_reputation', 'a', h - 3),
        ]:
            model = production[name]
            reference += model['base'][0] + count * model['slopes'][component][0]
        measured = round(row['after_max_ms'] * 1_000_000_000) + (
            row['after_reads'] * 25_000_000 + row['after_writes'] * 100_000_000)
        assert reference >= max(measured, row['after_model_ref_time_ps'])
        penalties.append(dict(benchmark=row['benchmark'], nodes=h,
                              generated_accepted_plus_arithmetic_ps=reference,
                              measured_max_plus_unique_db_ps=measured,
                              integrated_model_ref_time_ps=row['after_model_ref_time_ps']))
    (root / 'integrated-penalty-coverage.json').write_text(json.dumps(penalties, indent=2) + '\n')
    outcomes = {}
    for phase, path in [('before', root/'before-outcomes.log.gz'),
                        ('after', root/'validation/outcomes.log.gz')]:
        with gzip.open(path, 'rt') as handle:
            text = handle.read()
        outcomes[phase] = {combined: dict(events=int(events), state=bytes(ast.literal_eval(state)).hex())
                          for combined, events, state in re.findall(
                              r'reputation_outcome,combined=(false|true),nodes=512,events=(\d+),state=(\[[^\n]+\])', text)}
        assert set(outcomes[phase]) == {'false', 'true'}
    for case in ('false', 'true'):
        assert outcomes['before'][case]['state'] == outcomes['after'][case]['state']
        assert outcomes['after'][case]['events'] == 512
    assert outcomes['before']['true']['events'] == 1533
    (root / 'outcome-comparison.json').write_text(json.dumps(outcomes, indent=2) + '\n')
    admission = []
    for phase, path in [('before', root/'before-admission.log.gz'),
                        ('after', root/'validation/admission.log.gz')]:
        with gzip.open(path, 'rt') as handle:
            text = handle.read()
        for kind, n, time, proof in re.findall(
            r'capacity_model,(\w+),(\d+),(\d+),(\d+)', text
        ):
            if not kind.startswith('extrapolated') and int(n) <= 512:
                admission.append(dict(phase=phase, kind=kind, n=int(n),
                                      ref_time_ps=int(time), proof_bytes=int(proof)))
    with (root / 'admission-model.csv').open('w') as handle:
        writer = csv.DictWriter(handle, fieldnames=admission[0].keys())
        writer.writeheader()
        writer.writerows(admission)
    for row in rows:
        if row['components'] == 'h=512':
            print(json.dumps(row))


if __name__ == '__main__':
    main(Path(sys.argv[1]))
