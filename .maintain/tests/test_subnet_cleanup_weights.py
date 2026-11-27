"""Reject incomplete stage measurements and preserve both implementations when installing weights."""
import copy
import importlib.util
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('subnet_cleanup_weights', ROOT / '.maintain/merge-subnet-cleanup-weights.py')
MERGER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MERGER)


def method(name, value):
    argument = 's: u32' if name == 'removed_subnet_cleanup_step' else ''
    return f'\tfn {name}({argument}) -> Weight {{\n\t\tWeight::from_parts({value}, {value})\n\t}}'


def fixture():
    source = '\n'.join(method(n, 1) for _ in range(2) for n in sorted(MERGER.FIXED | {'removed_subnet_cleanup_step'}))
    generated = '\n'.join(method(n, 2) for _ in range(2) for n in sorted(MERGER.FIXED | {'activate_subnet_success'}))
    results = []
    for name in ['removed_subnet_cleanup_step', 'removed_subnet_cleanup_empty_step']:
        samples = [dict(components=[['s', s]], extrinsic_time=s + (100 if name.endswith('empty_step') else 10),
                        proof_size=62-s, reads=1, writes=0) for s in range(63)]
        results.append(dict(benchmark=name, time_results=samples, db_results=copy.deepcopy(samples)))
    return source, generated, results


class SubnetCleanupWeights(unittest.TestCase):
    def test_each_stage_covers_populated_and_empty_samples_in_both_implementations(self):
        source, generated, results = fixture()
        merged = MERGER.merge(source, generated, results)
        for stage in range(63):
            row = f'{stage} => ({(stage+100)*1100+1_000_000}, {62-stage+4096}, 2, 1)'
            self.assertEqual(merged.count(row), 2)
        self.assertEqual(merged.count('removal.max(activation)'), 2)
        self.assertIn('T::DbWeight::get().reads_writes(reads, writes)', merged)
        self.assertIn('RocksDbWeight::get().reads_writes(reads, writes)', merged)

    def test_missing_empty_probe_or_storage_sample_is_rejected(self):
        source, generated, results = fixture()
        for incomplete in [results[:1], copy.deepcopy(results)]:
            incomplete[-1]['db_results'] = incomplete[-1]['db_results'][:-1]
            with self.assertRaises(ValueError):
                MERGER.merge(source, generated, incomplete)

    def test_other_models_and_manual_allowances_are_unchanged(self):
        source, generated, results = fixture()
        untouched = '    fn other() -> Weight {\n        old_model().saturating_add(manual_safety_allowance())\n    }'
        merged = MERGER.merge(source + '\n' + untouched, generated, results)
        self.assertTrue(merged.endswith(untouched))
        # Reinstalling identical measurements is deterministic.
        self.assertEqual(MERGER.merge(merged, generated, results), merged)

    def test_runtime_stage_order_matches_the_benchmarked_weight_indices(self):
        source = (ROOT / 'pallets/network/src/lib.rs').read_text()
        stages = re.search(r'pub enum RemovedSubnetCleanupStage \{(.*?)\n    \}', source, re.S)[1]
        names = re.findall(r'^\s*(\w+),', stages, re.M)
        table = re.search(r'pub const ALL: \[Self; 63\] = \[(.*?)\];', source, re.S)[1]
        self.assertEqual(names, re.findall(r'Self::(\w+)', table))
        self.assertEqual(len(names), 63)


if __name__ == '__main__':
    unittest.main()
