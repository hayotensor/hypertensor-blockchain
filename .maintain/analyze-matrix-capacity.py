#!/usr/bin/env python3
"""Summarize per-size Matrix Wasm samples and estimated proof bounds.

No global linear regression is used for the quadratic consensus algorithm.
Point cost = slowest sample + production RocksDB unique-read/write charges.
The mandatory-slot comparison reserves another 1 ms / 32 KiB for hook selectors.
"""
import argparse
import csv
import importlib.util
import json
import math
import statistics
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('models', ROOT / '.maintain/capacity-generated-model.py')
models = importlib.util.module_from_spec(spec)
spec.loader.exec_module(models)


def weight(ms, proof, block_ms=2000, block_proof=32 * 1024**2):
    """FRAME reference time is in picoseconds; milliseconds are only a readable equivalent."""
    return dict(ref_time=round(ms * 10**9), ms=ms, proof_bytes=proof,
                block_ref_time_percent=100 * ms / block_ms,
                block_proof_percent=100 * proof / block_proof)


def analyze(directory, include=()):
    settlement_cases = ['slot', 'failed_credits', 'promotions', 'sparse_stake', 'no_scores']
    rows = []
    sources = sorted({source for root in (directory, *include)
                      for source in root.rglob('n-*/results.json')},
                     key=lambda p: (int(p.parent.name[2:]), str(p)))
    for source in sources:
        n = int(source.parent.name[2:])
        fitted = models.models(source.parent / 'weights.rs')
        for batch in json.loads(source.read_text()):
            endpoint = lambda sample: sample['components'] in ([['n', 2*n]], [['n', 2*n+1]])
            times = [s for s in batch['time_results'] if endpoint(s)]
            dbs = [s for s in batch['db_results'] if endpoint(s)]
            if not times or not dbs:
                raise ValueError(f'Missing exact endpoint: {source} / {batch["benchmark"]}')
            values = sorted(s['extrinsic_time'] / 1e6 for s in times)
            model = fitted[batch['benchmark']]
            slope = model['slopes'].get('n', [0, 0])
            proof = max(model['base'][1] + slope[1] * end for end in (2*n, 2*n+1))
            reads, writes = max(s['reads'] for s in dbs), max(s['writes'] for s in dbs)
            point_ms = max(values) + reads * 0.025 + writes * 0.1
            rows.append(dict(nodes=n, benchmark=batch['benchmark'], samples=len(times), storage_samples=len(dbs),
                median_ms=statistics.median(values), p95_ms=values[math.ceil(len(values) * .95) - 1],
                max_ms=max(values), reads=reads, writes=writes, rocksdb_inclusive_ms=point_ms,
                recorded_proof_bytes=max(s['proof_size'] for s in times + dbs),
                estimated_proof_bytes=proof, source=str(source.relative_to(directory)
                    if source.is_relative_to(directory) else source)))
    if not rows:
        raise ValueError('No benchmark results')
    # Reject incomplete or older campaigns before overwriting any analysis artifacts.
    required = {'capacity_matrix_' + name for name in settlement_cases}
    for n in sorted({r['nodes'] for r in rows}):
        present = {r['benchmark'] for r in rows if r['nodes'] == n}
        missing = required - present
        if missing:
            raise ValueError(f'Missing current Matrix scenarios at {n} nodes: {sorted(missing)}. '
                             'Use a complete campaign of the current algorithm.')
    with (directory / 'measurements.csv').open('w') as f:
        writer = csv.DictWriter(f, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    overhead = {}
    declared = {}
    signed_blocks = {}
    declared_hook = None
    limits = dict(block=dict(ms=2000, proof_bytes=32 * 1024**2),
                  network_hook=dict(ms=1000, proof_bytes=16 * 1024**2),
                  normal=dict(ms=1500, proof_bytes=24 * 1024**2),
                  base_block=dict(ms=0, proof_bytes=0))
    normal_length = int(.75 * 5 * 1024**2)
    overhead_path = directory / 'signed-overhead.log'
    if overhead_path.exists():
        for line in overhead_path.read_text().splitlines():
            if line.startswith('matrix_signed_overhead,'):
                _, kind, time, proof, length = line.split(',')
                overhead[kind] = dict(ms=int(time) / 1e9, proof=int(proof), length=int(length))
            elif line.startswith('matrix_weight_limit,'):
                _, kind, time, proof = line.split(',')
                limits[kind] = dict(ms=int(time) / 1e9, proof_bytes=int(proof))
            elif line.startswith('matrix_normal_length,'):
                normal_length = int(line.split(',')[1])
            elif line.startswith('matrix_current_declared,'):
                _, kind, time, proof, length = line.split(',')
                declared[kind] = dict(**weight(int(time) / 1e9, int(proof)), encoded_bytes=int(length))
            elif line.startswith('matrix_signed_block,'):
                _, kind, count, before_time, before_proof, after_time, after_proof = line.split(',')
                signed_blocks[kind] = dict(accepted=int(count), next_rejected='ExhaustsResources',
                    before=weight(int(before_time) / 1e9, int(before_proof)),
                    after=weight(int(after_time) / 1e9, int(after_proof)))
            elif line.startswith('matrix_current_hook_charge,'):
                _, nodes, time, proof = line.split(',')
                declared_hook = dict(nodes=int(nodes), **weight(int(time) / 1e9, int(proof)),
                                    scenario='accepted partial-reveal round, next snapshot and empty queue maintenance')
    block, hook, normal, base = (limits[k] for k in ('block', 'network_hook', 'normal', 'base_block'))

    def fits(value, budget):
        return all(value[k] <= budget[k] for k in budget)

    def weighted(time, proof):
        return weight(time, proof, block['ms'], block['proof_bytes'])

    def block_total(submissions, reservation):
        return weighted(submissions['ms'] + reservation['ms'] + base['ms'],
                        submissions['proof_bytes'] + reservation['proof_bytes'] + base['proof_bytes'])

    summary = []
    for n in sorted({r['nodes'] for r in rows}):
        group = {}
        for row in (r for r in rows if r['nodes'] == n):
            if row['benchmark'] in group:
                # Confirmation runs of the identical protocol retain the conservative
                # componentwise maximum. Raw per-run statistics stay in measurements.csv.
                previous = group[row['benchmark']]
                group[row['benchmark']] = dict(previous,
                    rocksdb_inclusive_ms=max(previous['rocksdb_inclusive_ms'], row['rocksdb_inclusive_ms']),
                    estimated_proof_bytes=max(previous['estimated_proof_bytes'], row['estimated_proof_bytes']))
            else:
                group[row['benchmark']] = row
        slots = [group['capacity_matrix_' + name] for name in settlement_cases]
        time = max(r['rocksdb_inclusive_ms'] for r in slots) + 1
        proof = max(r['estimated_proof_bytes'] for r in slots) + 32768
        item = dict(nodes=n, mandatory_ms=time, mandatory_proof_bytes=proof,
            fits_hook_limits=time <= hook['ms'] and proof <= hook['proof_bytes'],
            fits_with_20_percent_headroom=time <= .8 * hook['ms'] and proof <= .8 * hook['proof_bytes'],
            limiting_time_case=max(slots, key=lambda r: r['rocksdb_inclusive_ms'])['benchmark'])
        item['mandatory_weight'] = dict(**weighted(time, proof),
            hook_ref_time_percent=100 * time / hook['ms'], hook_proof_percent=100 * proof / hook['proof_bytes'])
        if len(overhead) == 2:
            calls = {}
            for kind in ('commit', 'reveal'):
                profiles = [group['capacity_matrix_' + kind]]
                loaded = group.get('capacity_matrix_' + kind + '_loaded')
                if loaded is not None:
                    profiles.append(loaded)
                length = overhead['commit']['length']
                if kind == 'reveal':
                    length += 20 * (n - 1) + (1 if n - 1 < 64 else 2)
                calls[kind] = dict(ms=max(p['rocksdb_inclusive_ms'] for p in profiles) + overhead[kind]['ms'],
                    proof_bytes=max(p['estimated_proof_bytes'] for p in profiles) + overhead[kind]['proof'] + length,
                    encoded_bytes=length, loaded_profile_measured=loaded is not None)
                calls[kind].update(weighted(calls[kind]['ms'], calls[kind]['proof_bytes']))
            # Constructive schedule: at most ceil(n/10) rows per subnet per phase block.
            # Sixteen subnet slots in a 20-block epoch give 6..10 simultaneous revealers.
            # Reserve the full Network hook budget in every block. Scheduler/other activity
            # must fit elsewhere; this is Matrix-only capacity, not guaranteed free bandwidth.
            quota = math.ceil(n / 10)
            peak = {}
            for field in ('ms', 'proof_bytes', 'encoded_bytes'):
                peak[field] = quota * max(c * calls['commit'][field] + (16-c) * calls['reveal'][field]
                                          for c in range(6, 11))
            submission_limits = dict(ms=min(normal['ms'], block['ms'] - hook['ms'] - base['ms']),
                proof_bytes=min(normal['proof_bytes'], block['proof_bytes'] - hook['proof_bytes'] - base['proof_bytes']),
                encoded_bytes=normal_length)
            average = {field: 16 * n / 20 * (calls['commit'][field] + calls['reveal'][field])
                       for field in submission_limits}
            item['signed_calls'] = calls
            item['all_16_subnets'] = dict(rows_per_phase_block=quota, peak=peak, average=average,
                block_limits_after_full_network_reservation=submission_limits,
                uniform_schedule_fits=fits(peak, submission_limits),
                uniform_schedule_fits_with_20_percent_headroom=fits(peak, {k: .8*v for k,v in submission_limits.items()}),
                average_within_budget=fits(average, submission_limits))
            peak.update(weighted(peak['ms'], peak['proof_bytes']))
            matrix_total = block_total(peak, item['mandatory_weight'])
            reserved_total = block_total(peak, hook)
            normal_budget = dict(**normal, encoded_bytes=normal_length)
            item['all_16_subnets'].update(
                matrix_only_peak_block=matrix_total,
                full_network_reservation_peak_block=reserved_total,
                fits_normal_class=fits(peak, normal_budget),
                matrix_only_fits=item['fits_hook_limits'] and fits(peak, normal_budget) and fits(matrix_total, block))
            # In this subnet's settlement block it has just entered the next commit phase.
            single_peak = {k: quota * max(calls['commit'][k], calls['reveal'][k])
                           for k in submission_limits}
            single_commit = {k: quota * calls['commit'][k] for k in submission_limits}
            single_total = block_total(single_commit, item['mandatory_weight'])
            reveal_total = block_total(single_peak, dict(ms=0, proof_bytes=0))
            single_total = weighted(max(single_total['ms'], reveal_total['ms']),
                                    max(single_total['proof_bytes'], reveal_total['proof_bytes']))
            item['single_subnet'] = dict(rows_per_phase_block=quota,
                peak_submissions=dict(**single_peak,
                    **{k:v for k,v in weighted(single_peak['ms'], single_peak['proof_bytes']).items()
                       if k not in single_peak}),
                matrix_only_peak_block=single_total,
                fits=item['fits_hook_limits'] and fits(single_peak, normal_budget) and fits(single_total, block))
        summary.append(item)
    report = dict(hook_limits=hook, block_limits=block, normal_limits=dict(**normal, encoded_bytes=normal_length),
                  base_block=base, current_declared_signed_calls=declared, current_signed_block_capacity=signed_blocks,
                  current_declared_hook=declared_hook,
                  slot_fixed_allowance=dict(ms=1, proof_bytes=32768),
                  database_costs_ms=dict(read=.025, write=.1),
                  timing_samples=sum(r['samples'] for r in rows), storage_samples=sum(r['storage_samples'] for r in rows),
                  results=summary,
                  notes=['Optimized compiled Wasm on the recorded host, not end-to-end block import.',
                         'Point timing uses the slowest measured sample, not a global affine model.',
                         'Estimated proof bounds are from FRAME max PoV analysis for equivalent parameter pairs at each fixed node count.',
                         'Production Matrix bound and admission weights have not been changed.',
                         'Submission scheduling assumes all 16 subnets participate fully; other traffic competes for the same block capacity.',
                         'Matrix-only peak blocks exclude other pallet hooks, global Network work, optional maintenance, other transactions and idle cleanup.',
                         'The full-Network-reservation model reserves 1,000 ms / 16 MiB every block, not just the measured Matrix settlement; base block weight is additional.',
                         'A maximum below is the largest tested population, not an exhaustive integer search or an installed runtime bound.'])
    criteria = dict(single_subnet=lambda r: r.get('single_subnet', {}).get('fits'),
                    hook_with_20_percent_headroom=lambda r: r['fits_with_20_percent_headroom'],
                    all_16_matrix_only=lambda r: r.get('all_16_subnets', {}).get('matrix_only_fits'),
                    all_16_full_network_reservation=lambda r: r.get('all_16_subnets', {}).get('uniform_schedule_fits'),
                    all_16_submission_20_percent_headroom=lambda r: r.get('all_16_subnets', {}).get('uniform_schedule_fits_with_20_percent_headroom'))
    report['largest_tested_passing'] = {name: max((r['nodes'] for r in summary if predicate(r)), default=None)
                                        for name, predicate in criteria.items()}
    (directory / 'capacity.json').write_text(json.dumps(report, indent=2) + '\n')
    block_rows = []
    for item in summary:
        row = dict(nodes=item['nodes'])
        for prefix, value in (
            ('settlement', item['mandatory_weight']),
            ('single_subnet_peak', item.get('single_subnet', {}).get('matrix_only_peak_block', {})),
            ('all_16_matrix_only_peak', item.get('all_16_subnets', {}).get('matrix_only_peak_block', {})),
            ('all_16_full_reservation_peak', item.get('all_16_subnets', {}).get('full_network_reservation_peak_block', {})),
        ):
            for field in ('ref_time', 'ms', 'proof_bytes', 'block_ref_time_percent', 'block_proof_percent'):
                row[f'{prefix}_{field}'] = value.get(field)
        row['settlement_hook_ref_time_percent'] = item['mandatory_weight']['hook_ref_time_percent']
        row['settlement_hook_proof_percent'] = item['mandatory_weight']['hook_proof_percent']
        row['single_subnet_fits'] = item.get('single_subnet', {}).get('fits')
        row['all_16_matrix_only_fits'] = item.get('all_16_subnets', {}).get('matrix_only_fits')
        row['all_16_full_reservation_fits'] = item.get('all_16_subnets', {}).get('uniform_schedule_fits')
        block_rows.append(row)
    if block_rows:
        with (directory / 'block-weights.csv').open('w') as f:
            writer = csv.DictWriter(f, fieldnames=list(block_rows[0]))
            writer.writeheader()
            writer.writerows(block_rows)
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--include', type=Path, action='append', default=[],
                        help='Include saved measurements without rerunning or copying them')
    args = parser.parse_args()
    analyze(args.directory.resolve(), [p.resolve() for p in args.include])
