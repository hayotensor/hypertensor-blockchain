"""Regression checks for current classification names and preserved weight allowances.

Run with: python3 -m unittest discover -s .maintain/tests -p 'test_*.py'
"""
import importlib.util
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "merge_node_metadata_weights", ROOT / ".maintain/merge-node-metadata-weights.py"
)
MERGER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MERGER)


class NodeMetadataWeightTests(unittest.TestCase):
    def test_regeneration_preserves_every_manual_allowance(self):
        source = (ROOT / "pallets/network/src/weights.rs").read_text()
        selected = [
            match[0]
            for match in MERGER.METHOD.finditer(source)
            if match[1] in MERGER.METHODS
        ]
        self.assertEqual(len(selected), 2 * len(MERGER.METHODS))
        generated = "\n".join(selected)
        # Fresh benchmark output lacks the manually composed additions. Remove them by
        # their call syntax, independently of the merger's preservation expression.
        manual_calls = (
            "pending_liability_key_proof()",
            "settlement_pending_marker_envelope()",
            "consensus_round_liability_proof()",
            "subnet_node_registration_collision_proof()",
            "Self::pending_admitted_removal_scan(MAX_PENDING_ADMITTED_REMOVALS)",
            "Self::pending_queued_removal_scan(MAX_PENDING_QUEUED_REMOVALS)",
        )
        for call in manual_calls[-2:]:
            self.assertEqual(generated.count(call), 4)
        generated = "\n".join(
            line for line in generated.splitlines()
            if not any(f".saturating_add({call})" in line for call in manual_calls)
        )
        self.assertEqual(MERGER.merge_source(source, generated), source)

    def test_selected_methods_exist_in_benchmarks_and_weight_contract(self):
        source = (ROOT / "pallets/network/src/weights.rs").read_text()
        trait = source.split("pub trait WeightInfo {")[1].split("\n}", 1)[0]
        declared = set(re.findall(r"\bfn (\w+)\(", trait))
        benchmarks = (ROOT / "pallets/network/src/benchmarking.rs").read_text()
        benchmark_names = set(re.findall(r"#\[benchmark(?:\(extra\))?\]\s*fn (\w+)\(", benchmarks))
        self.assertLessEqual(MERGER.METHODS, declared)
        self.assertLessEqual(MERGER.METHODS, benchmark_names)
        self.assertNotIn("graduate_class", MERGER.METHODS)
        self.assertNotIn("promote_probation_node_to_active", MERGER.METHODS)

    def test_incomplete_generated_output_is_rejected(self):
        with self.assertRaises(ValueError):
            MERGER.merge_source("", "")


if __name__ == "__main__":
    unittest.main()
