import importlib.util
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).parent
spec = importlib.util.spec_from_file_location("benchmark_validate", ROOT / "validate.py")
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(module)


class BenchmarkValidationTests(unittest.TestCase):
    def test_checked_in_example_is_valid(self):
        errors = module.validate_file(
            ROOT / "example-result.json", ROOT / "tokens/schema.json"
        )
        self.assertEqual(errors, [])

    def test_invalid_result_reports_all_ordered_errors(self):
        result = {"median": 4, "p95": 2, "sample_count": 0}
        errors = module.validate_result(result)
        self.assertIn("missing required field: commit", errors)
        self.assertIn("sample_count must be greater than zero", errors)
        self.assertIn("p95 must be greater than or equal to median", errors)


if __name__ == "__main__":
    unittest.main()
