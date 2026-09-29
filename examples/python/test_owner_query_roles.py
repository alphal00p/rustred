"""Fail-closed scope declarations, independent of any native solver run."""
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location("owner_query_roles", Path(__file__).with_name("owner_query_roles.py"))
ROLES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ROLES)


class QueryRolesTests(unittest.TestCase):
    def test_duplicate_json_keys_fail_before_roles_are_decoded(self):
        for text in [
            '{"queries":[],"query_roles":{},"query_roles":{}}',
            '{"queries":[],"query_roles":{"required":[],"required":[],"auxiliary":[]}}',
            '{"queries":[],"query_roles":{"required":[],"auxiliary":[],"auxiliary":[]}}']:
            with self.subTest(text=text), self.assertRaisesRegex(ValueError, "duplicate JSON field"):
                ROLES.loads_document(text)

    def test_names_cannot_change_roles_and_omission_cannot_enable_rescue(self):
        document = {"queries":[{"id":"required-anchor"},{"id":"ordinary"}]}
        self.assertEqual(ROLES.query_roles(document), {"required-anchor":"required","ordinary":"required"})
        with self.assertRaisesRegex(ValueError, "explicit complete"):
            ROLES.query_roles(document, require_explicit=True)
        document["query_roles"] = {"required":["required-anchor"],"auxiliary":["ordinary"]}
        self.assertEqual(ROLES.query_roles(document, True), {"required-anchor":"required","ordinary":"auxiliary"})
        for declaration in [
                {"required":[],"auxiliary":["ordinary"]},
                {"required":["required-anchor"],"auxiliary":["ordinary","required-anchor"]},
                {"required":["required-anchor","required-anchor"],"auxiliary":["ordinary"]},
                {"required":["required-anchor"],"auxiliary":["unknown"]},
                {"required":["required-anchor"]}]:
            with self.subTest(declaration=declaration), self.assertRaises(ValueError):
                ROLES.query_roles(dict(document, query_roles=declaration), True)

    def test_current_five_loop_scope_includes_all_convenience_queries(self):
        import json
        root = Path(__file__).resolve().parents[2]
        directory = root / "examples/input/five_loop_qcd_feynman_d9d10"
        document = json.loads((directory / "queries.json").read_text())
        receipt = json.loads((directory / "entry-plan-receipt.json").read_text())
        roles = ROLES.query_roles(document, True)
        required = {root["id"] for row in receipt["owners"] for root in row["roots"]}
        convenience = {root["id"] for row in receipt["owners"] if row["class"] == "non_entry" for root in row["roots"]}
        self.assertEqual(len(required), 116)
        self.assertEqual(len(convenience), 16)
        self.assertEqual({identity for identity, role in roles.items() if role == "required"}, required)
        self.assertEqual(sum(role == "auxiliary" for role in roles.values()), 67)


if __name__ == "__main__":
    unittest.main()
