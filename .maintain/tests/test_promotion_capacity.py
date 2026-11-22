"""Checks that the research report cannot silently accept missing or misleading data."""
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("promotions", Path(__file__).resolve().parents[1] / "benchmark-promotions.py")
REPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPORT)


class PromotionReportTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / "results.json"
        self.sample = dict(components=[["c", 506]], extrinsic_time=2_000_000,
                           reads=10, writes=4, repeat_reads=500, repeat_writes=500,
                           proof_size=100)
        self.batches = [dict(benchmark=method, time_results=[self.sample.copy() for _ in range(10)],
                             db_results=[self.sample.copy()]) for method in REPORT.METHODS]
        for batch in self.batches:
            for sample in batch["time_results"][5:]:
                sample["components"] = [["c", 507]]
        self.models = {method: dict(base=[3_000_000_000, 200], slopes={"c": [0, 0]}) for method in REPORT.METHODS}

    def read(self):
        self.path.write_text(json.dumps(self.batches))
        with patch.object(REPORT.GENERATED, "models", return_value=self.models):
            return REPORT.read_results(self.path, self.path, "c", 253, REPORT.METHODS, 10)

    def test_units_and_shared_storage_costs(self):
        row = self.read()[0]
        self.assertEqual(row["max_ms"], 2)
        self.assertAlmostEqual(row["max_with_db_ms"], 2.65)
        self.assertEqual(row["recorded_proof_bytes"], 100)
        self.assertEqual(row["estimated_proof_bytes"], 200)
        self.assertEqual(row["repeat_writes"], 500)

    def test_missing_hook_is_rejected(self):
        self.batches.pop()
        with self.assertRaisesRegex(ValueError, "Missing"):
            self.read()

    def test_wrong_endpoint_is_rejected(self):
        self.batches[0]["time_results"][0]["components"] = [["c", 252]]
        with self.assertRaisesRegex(ValueError, "endpoint"):
            self.read()

    def test_insufficient_repetitions_are_rejected(self):
        self.batches[0]["time_results"].pop()
        with self.assertRaisesRegex(ValueError, "samples"):
            self.read()

    def test_underestimated_proof_is_rejected(self):
        self.models[REPORT.METHODS[0]]["base"][1] = 99
        with self.assertRaisesRegex(ValueError, "proof"):
            self.read()

    def test_incomplete_manifest_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "manifest"):
            REPORT.manifest("promotion_limit,1000000000000,16777216,804\n")


if __name__ == "__main__":
    unittest.main()
