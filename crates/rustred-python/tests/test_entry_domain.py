import json
import unittest

import rustred


class FiniteEntryDomainTests(unittest.TestCase):
    def specification(self):
        return {
            "schema": "rustred.entry-domain.json.v1",
            "profile": {"name": "renormalizable_marginal_feynman", "loops": 1},
            "sectors": ["1"],
            "max_positive_layers_per_sector": 16,
            "max_preview_targets": 4,
        }

    def test_public_api_counts_and_previews_exactly(self):
        report = rustred.entry_domain_plan(json.dumps(self.specification()))
        self.assertEqual(report["total_target_count"], "3")
        self.assertEqual([x["powers"] for x in report["preview"]["targets"]], [[2], [3], [4]])
        self.assertFalse(report["preview"]["truncated"])

    def test_prefix_is_not_exhaustive(self):
        specification = self.specification()
        specification["max_preview_targets"] = 1
        report = rustred.entry_domain_plan(json.dumps(specification))
        self.assertTrue(report["preview"]["truncated"])
        self.assertEqual(report["total_target_count"], "3")

    def test_conflicting_profile_and_budget_is_rejected(self):
        specification = self.specification()
        specification["budget"] = {"max_positive_power": 4, "max_numerator_rank": 2}
        with self.assertRaises(rustred.RustRedInputError):
            rustred.entry_domain_plan(json.dumps(specification))


if __name__ == "__main__":
    unittest.main()
