"""Data-staging tests; deliberately synthetic bytes, never native IBP evidence."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import tempfile
import unittest

import stage


REPOSITORY = Path(__file__).resolve().parents[3]


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def write_toml(path, value):
    # The synthetic documents contain only JSON-compatible TOML scalars/arrays.
    path.write_text("".join(f"{key} = {json.dumps(item)}\n"
                            for key, item in value.items()))


def bits(mask):
    return [bit == "1" for bit in mask]


class StageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="runtime-order-stage-test-",
                                                dir=REPOSITORY / "TMP")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.output = self.base / "staged"
        (self.base / "family.toml").write_text("# synthetic input, not an integral family\n")
        self.selection = {
            "family_fingerprint": "synthetic-family-only",
            "owners": [{"mask": "10", "parent": 2, "native_ordinal": 99,
                        "path": "old-10.rrbin", "bytes": 1, "sha256": "obsolete"},
                       {"mask": "11", "parent": 3, "native_ordinal": 88,
                        "path": "old-11.rrbin", "bytes": 1, "sha256": "obsolete"}],
            "owner_count": 2,
            "initial_frontier_routes": [
                {"source_mask": "01", "owner_mask": "10", "permutation": [1, 0]},
                {"source_mask": "10", "owner_mask": "10", "permutation": [0, 1]},
                {"source_mask": "11", "owner_mask": "11", "permutation": [0, 1]}],
            "route_record_count": 3, "recursive_coverage_established": True,
            "receipts": {"obsolete_claim": "must not survive"}}
        self.queries = {"schema": "rustred.owner-domain-queries.json.v2",
                        "queries": [{"id": "a", "owner": "10"},
                                    {"id": "b", "owner": "11"}],
                        "query_roles": {"required": ["a", "b"], "auxiliary": []}}
        self.plan = {"schema": stage.PLAN_SCHEMA, "input_format": "toml",
                     "integral_order": "synthetic-native-identity",
                     "discovery_strategy": None, "exact_backend": "sparse",
                     "solver_policy": "synthetic-policy", "roots": []}
        for name, value in (("selection", self.selection), ("queries", self.queries)):
            write_json(self.base / f"{name}.json", value)
        for name in ("family", "selection", "queries"):
            path = self.base / ("family.toml" if name == "family" else f"{name}.json")
            self.plan[name] = {"path": path.name, "sha256": stage.sha(path)}
        self.manifests, self.reports = {}, {}
        for parent, root_mask, sectors in ((3, "11", ["01", "10", "11"]),
                                           (2, "10", ["10"])):
            directory = self.base / f"root{parent}"
            directory.mkdir()
            manifest = {"version": stage.CHECKPOINT_VERSION, "recipe": stage.CHECKPOINT_RECIPE,
                        "family_source": (self.base / "family.toml").read_text(),
                        "input_format": "toml", "family_fingerprint": "synthetic-family-only",
                        "root_sector": bits(root_mask),
                        "integral_order": self.plan["integral_order"],
                        "solver_policy": self.plan["solver_policy"], "exact_backend": "sparse",
                        "sectors": [bits(mask) for mask in sectors]}
            report = {"schema": "rustred.family-candidates-output.toml.v1",
                      "status": "uncertified-candidates", "integral_order": self.plan["integral_order"],
                      "exact_backend": "sparse", "family_fingerprint": "synthetic-family-only",
                      "arity": 2, "root_sector": bits(root_mask), "solved_sectors": len(sectors),
                      "generation_scope": "root-downset"}
            self.manifests[parent] = manifest
            self.reports[parent] = report
            for ordinal, sector in enumerate(sectors):
                (directory / f"sector-{ordinal}.rrbin").write_bytes(
                    f"SYNTHETIC NOT NATIVE: root={parent} sector={sector}".encode())
            self.plan["roots"].append({"parent": parent, "mask": root_mask,
                                       "checkpoint": f"root{parent}/checkpoint.toml",
                                       "report": f"root{parent}/generation.toml"})
        self.save()

    def save(self):
        for parent, value in self.manifests.items():
            write_toml(self.base / f"root{parent}/checkpoint.toml", value)
            write_toml(self.base / f"root{parent}/generation.toml", self.reports[parent])
        write_json(self.base / "plan.json", self.plan)

    def replace_frozen(self, name, value):
        path = self.base / self.plan[name]["path"]
        write_json(path, value)
        self.plan[name]["sha256"] = stage.sha(path)
        self.save()

    def reject(self, pattern):
        with self.assertRaisesRegex((ValueError, KeyError), pattern):
            stage.stage(self.base / "plan.json", self.output)
        self.assertFalse(self.output.exists(), "validation must precede any staging writes")

    def test_success_maps_actual_ordinals_preserves_routes_and_query_bytes(self):
        before = {path: stage.sha(path) for path in self.base.rglob("*") if path.is_file()}
        receipt = stage.stage(self.base / "plan.json", self.output)
        selected = stage.read_json(self.output / "selection.json")
        self.assertEqual([owner["native_ordinal"] for owner in selected["owners"]], [0, 2])
        self.assertEqual(selected["initial_frontier_routes"], self.selection["initial_frontier_routes"])
        self.assertEqual((self.output / "queries.json").read_bytes(),
                         (self.base / "queries.json").read_bytes())
        self.assertEqual(receipt["status"], "STAGED_NATIVE_ADMISSION_AND_COLD_REQUIRED")
        for claim in ("native_payload_admission", "source_replay_claim", "closure_claim"):
            self.assertIs(receipt[claim], False)
        self.assertIs(selected["recursive_coverage_established"], False)
        self.assertNotIn("obsolete_claim", selected["receipts"])
        for payload in receipt["payloads"]:
            self.assertEqual(stage.sha(self.output / payload["path"]), payload["sha256"])
            self.assertFalse(Path(payload["path"]).is_absolute())
        for path, digest in before.items():
            self.assertEqual(stage.sha(path), digest)

    def test_identical_inputs_produce_identical_staging(self):
        first = stage.stage(self.base / "plan.json", self.output)
        second = stage.stage(self.base / "plan.json", self.base / "other")
        self.assertEqual(first, second)
        self.assertEqual((self.output / "selection.json").read_bytes(),
                         (self.base / "other/selection.json").read_bytes())

    def test_existing_output_is_never_overwritten(self):
        stage.stage(self.base / "plan.json", self.output)
        before = stage.sha(self.output / "selection.json")
        with self.assertRaisesRegex(ValueError, "refusing overwrite"):
            stage.stage(self.base / "plan.json", self.output)
        self.assertEqual(stage.sha(self.output / "selection.json"), before)

    def test_previous_checkpoint_version_rejected(self):
        self.manifests[3]["version"] = 3
        self.save()
        self.reject("native v4")

    def select_owner_jobs(self, parent=3):
        selected = sorted(owner["mask"] for owner in self.selection["owners"]
                          if owner["parent"] == parent)
        old_sectors = self.manifests[parent]["sectors"]
        payloads = [(self.base / f"root{parent}/sector-{old_sectors.index(bits(mask))}.rrbin").read_bytes()
                    for mask in selected]
        for path in (self.base / f"root{parent}").glob("sector-*.rrbin"):
            path.unlink()
        for ordinal, payload in enumerate(payloads):
            (self.base / f"root{parent}/sector-{ordinal}.rrbin").write_bytes(payload)
        native_selected = [bits(mask) for mask in selected]
        self.manifests[parent].update(selected_sectors=native_selected, sectors=native_selected)
        self.reports[parent].update(generation_scope="selected-sectors",
                                    selected_sectors=native_selected, solved_sectors=len(selected))
        next(root for root in self.plan["roots"] if root["parent"] == parent)["generation_scope"] = "selected-sectors"
        self.save()

    def test_selected_jobs_preserve_full_routes_queries_and_new_ordinals(self):
        self.select_owner_jobs()
        receipt = stage.stage(self.base / "plan.json", self.output)
        selected = stage.read_json(self.output / "selection.json")
        self.assertEqual([owner["native_ordinal"] for owner in selected["owners"]], [0, 0])
        self.assertEqual(selected["initial_frontier_routes"], self.selection["initial_frontier_routes"])
        self.assertEqual(receipt["routes"], 3)
        self.assertEqual(receipt["required"], 2)
        self.assertEqual([root["generation_scope"] for root in receipt["roots"]],
                         ["selected-sectors", "root-downset"])
        self.assertEqual((self.output / "queries.json").read_bytes(),
                         (self.base / "queries.json").read_bytes())
        self.assertFalse(receipt["closure_claim"])
        self.assertFalse(selected["recursive_coverage_established"])
        self.assertIn(b"sector=11", (self.output / selected["owners"][1]["path"]).read_bytes())

    def test_selected_mode_does_not_weaken_default_full_downset_check(self):
        self.select_owner_jobs()
        self.plan["roots"][0].pop("generation_scope")
        self.save()
        self.reject("full root mode")

    def test_selected_mode_preserves_auxiliary_roles_without_dropping_queries(self):
        self.select_owner_jobs()
        self.queries["query_roles"] = {"required": ["a"], "auxiliary": ["b"]}
        self.replace_frozen("queries", self.queries)
        receipt = stage.stage(self.base / "plan.json", self.output)
        self.assertEqual((receipt["queries"], receipt["required"], receipt["auxiliary"]), (2, 1, 1))
        self.assertEqual((self.output / "queries.json").read_bytes(),
                         (self.base / "queries.json").read_bytes())

    def test_selected_jobs_require_exact_native_inventory(self):
        self.select_owner_jobs()
        original = copy.deepcopy(self.manifests[3])
        for field, value in (("selected_sectors", []), ("sectors", []),
                             ("selected_sectors", [bits("10"), bits("11")])):
            with self.subTest(field=field, value=value):
                self.manifests[3] = dict(original, **{field: value})
                self.save()
                self.reject("frozen owner jobs")

    def test_selected_scope_and_report_inventory_are_explicit(self):
        self.select_owner_jobs()
        original = copy.deepcopy(self.reports[3])
        for field, value in (("generation_scope", "root-downset"),
                             ("selected_sectors", [bits("10")])):
            with self.subTest(field=field):
                self.reports[3] = dict(original, **{field: value})
                self.save()
                self.reject("report (scope|selected sectors) differs?")

    def test_selected_mode_rejects_missing_shard_without_old_owner_fallback(self):
        self.select_owner_jobs()
        (self.base / "root3/sector-0.rrbin").unlink()
        (self.base / "old-11.rrbin").write_bytes(b"OLD PAYLOAD MUST NOT BE USED")
        self.reject("missing/empty generated shard")

    def test_missing_owner_parent_is_not_inferred(self):
        self.selection["owners"][1].pop("parent")
        self.replace_frozen("selection", self.selection)
        self.reject("explicit generation parent")

    def test_unknown_generation_scope_rejected(self):
        self.plan["roots"][0]["generation_scope"] = "whatever-is-available"
        self.save()
        self.reject("unsupported generation scope")

    def test_selected_owner_outside_its_root_rejected(self):
        self.select_owner_jobs()
        self.plan["roots"][0]["mask"] = "10"
        self.manifests[3]["root_sector"] = bits("10")
        self.save()
        self.reject("outside its generation root")

    def test_duplicate_json_keys_rejected(self):
        original_plan = (self.base / "plan.json").read_bytes()
        plan_text = original_plan.decode()
        (self.base / "plan.json").write_text(plan_text.replace(
            '"integral_order":', '"integral_order": "ambiguous", "integral_order":', 1))
        self.reject("duplicate JSON key: integral_order")
        (self.base / "plan.json").write_bytes(original_plan)
        query_path = self.base / "queries.json"
        query_path.write_text(query_path.read_text().replace(
            '"required":', '"required": [], "required":', 1))
        self.plan["queries"]["sha256"] = stage.sha(query_path)
        self.save()
        self.reject("duplicate JSON key: required")

    def test_missing_and_mixed_order_rejected(self):
        for replacement in (None, "another-native-identity"):
            with self.subTest(order=replacement):
                if replacement is None:
                    self.manifests[3].pop("integral_order")
                else:
                    self.manifests[3]["integral_order"] = replacement
                self.save()
                self.reject("checkpoint integral_order")

    def test_report_order_and_count_rejected(self):
        original = copy.deepcopy(self.reports[3])
        for key, value in (("integral_order", "different"), ("solved_sectors", 2)):
            with self.subTest(key=key):
                self.reports[3] = dict(original, **{key: value})
                self.save()
                self.reject("report binding")

    def test_missing_selected_and_unselected_shard_rejected(self):
        for ordinal in (0, 2):
            with self.subTest(ordinal=ordinal):
                path = self.base / f"root3/sector-{ordinal}.rrbin"
                content = path.read_bytes()
                path.unlink()
                self.reject("missing/empty generated shard")
                path.write_bytes(content)

    def test_incomplete_root_census_rejected(self):
        self.manifests[3]["sectors"] = self.manifests[3]["sectors"][1:]
        self.save()
        self.reject("full root route census")

    def test_misassigned_owner_parent_rejected(self):
        self.selection["owners"][1]["parent"] = 2
        self.replace_frozen("selection", self.selection)
        self.reject("roots differ")

    def test_missing_owner_rejected(self):
        self.selection["owners"].pop()
        self.selection["owner_count"] = 1
        self.replace_frozen("selection", self.selection)
        self.reject("unselected owner")

    def test_missing_and_mixed_routes_rejected(self):
        original = copy.deepcopy(self.selection)
        self.selection["initial_frontier_routes"].pop(0)
        self.selection["route_record_count"] = 2
        self.replace_frozen("selection", self.selection)
        self.reject("full root route census")
        self.selection = original
        self.selection["initial_frontier_routes"][0]["owner_mask"] = "01"
        self.replace_frozen("selection", self.selection)
        self.reject("unselected owner")

    def test_frozen_route_bytes_cannot_be_changed(self):
        self.selection["initial_frontier_routes"][0]["permutation"] = [0, 1]
        write_json(self.base / "selection.json", self.selection)
        self.reject("frozen selection bytes changed")

    def test_frozen_query_bytes_cannot_be_changed(self):
        self.queries["queries"][0]["id"] = "changed"
        write_json(self.base / "queries.json", self.queries)
        self.reject("frozen queries bytes changed")

    def test_incomplete_roles_rejected(self):
        self.queries["query_roles"]["required"].pop()
        self.replace_frozen("queries", self.queries)
        self.reject("roles")

    def test_family_recipe_and_missing_strategy_binding_rejected(self):
        self.manifests[3]["family_source"] = "different"
        self.save()
        self.reject("family binding")
        self.manifests[3]["family_source"] = (self.base / "family.toml").read_text()
        self.plan.pop("discovery_strategy")
        self.save()
        self.reject("discovery_strategy")


if __name__ == "__main__":
    unittest.main()
