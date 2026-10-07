"""Pure input staging checks: fake saved bytes are not native coverage evidence."""

import copy
from contextlib import redirect_stdout
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "prepare_rank_campaign", Path(__file__).with_name("prepare_rank_campaign.py"))
PREPARE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PREPARE)


def row(identity, owner="10", rank=3):
    return {"id": identity, "owner": owner, "lower": [0, 1], "upper": [5, None],
            "max_numerator_rank": rank,
            "power_bounds": {"max_positive_power": 7, "min_power_difference": 4,
                             "max_power_difference": 6}}


def document():
    return {"schema": "rustred.owner-domain-queries.json.v2",
            "queries": [row("helper", rank=None), row("required-second"),
                        row("required-first", owner="01", rank=1)],
            "query_roles": {"required": ["required-first", "required-second"],
                            "auxiliary": ["helper"]}}


def snapshot(root):
    return {str(path.relative_to(root)): path.read_bytes()
            for path in root.rglob("*") if path.is_file()}


class RankQueryPlanningTests(unittest.TestCase):
    def test_required_only_scalar_scope_changes_only_rank_and_auxiliary_membership(self):
        source = document()
        data = json.dumps(source).encode()
        scoped, receipt = PREPARE.plan_rank_queries(data, 0)
        expected = copy.deepcopy(source)
        expected["queries"] = expected["queries"][1:]
        expected["query_roles"]["auxiliary"] = []
        for item in expected["queries"]:
            item["max_numerator_rank"] = 0
        self.assertEqual(json.loads(scoped), expected)
        self.assertEqual(source, document())
        self.assertEqual(receipt["removed_auxiliary_query_ids"], ["helper"])
        self.assertEqual(receipt["retained_query_ids"], ["required-second", "required-first"])
        self.assertEqual(receipt["source_queries_sha256"], hashlib.sha256(data).hexdigest())
        self.assertEqual(receipt["queries_sha256"], hashlib.sha256(scoped).hexdigest())
        self.assertFalse(receipt["descendant_clipping"])
        self.assertFalse(receipt["native_coverage_checked"])
        self.assertFalse(receipt["family_closure_claim"])
        # Inactive lower=1 makes the R0 intersection empty: preserve the row and
        # its valid coordinate box; Python must not discard or repair geometry.
        self.assertEqual(json.loads(scoped)["queries"][0]["lower"], [0, 1])

    def test_include_auxiliary_and_nonzero_rank_never_widen_original_caps(self):
        original = document()
        original["queries"][0]["power_bounds"]["max_positive_power"] = None
        data = json.dumps(original).encode()
        scoped, receipt = PREPARE.plan_rank_queries(data, 2, include_auxiliary=True)
        expected = copy.deepcopy(original)
        for item, cap in zip(expected["queries"], (2, 2, 1)):
            item["max_numerator_rank"] = cap
        self.assertEqual(json.loads(scoped), expected)
        self.assertEqual(receipt["auxiliary_query_count"], 1)
        self.assertEqual(receipt["removed_auxiliary_query_ids"], [])
        self.assertEqual(receipt["query_count"], 3)
        again, _ = PREPARE.plan_rank_queries(scoped, 10, include_auxiliary=True)
        self.assertEqual(again, scoped)

    def test_explicit_complete_roles_and_unique_ids_required(self):
        for mutate in (
                lambda value: value.pop("query_roles"),
                lambda value: value["query_roles"].update(auxiliary=[]),
                lambda value: value["query_roles"]["required"].append("helper"),
                lambda value: value["queries"][1].update(id="helper")):
            value = document()
            mutate(value)
            with self.subTest(value=value), self.assertRaises(ValueError):
                PREPARE.plan_rank_queries(json.dumps(value).encode(), 0)
        with self.assertRaisesRegex(ValueError, "duplicate JSON field"):
            PREPARE.plan_rank_queries(b'{"schema":1,"schema":2}', 0)

    def test_invalid_rank_and_shape_fail_closed(self):
        raw = json.dumps(document()).encode()
        for rank in (-1, 2**32, True, "0", 1.2):
            with self.subTest(rank=rank), self.assertRaises(ValueError):
                PREPARE.plan_rank_queries(raw, rank)
        for field, value in (("max_numerator_rank", True), ("max_numerator_rank", -1),
                             ("owner", "1x"), ("upper", []), ("power_bounds", []),
                             ("id", "")):
            source = document()
            source["queries"][0][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                PREPARE.plan_rank_queries(json.dumps(source).encode(), 0)
        with self.assertRaises(ValueError):
            PREPARE.plan_rank_queries(raw, 0, include_auxiliary="yes")

    def test_no_required_start_is_not_silently_an_empty_campaign(self):
        source = document()
        source["query_roles"] = {"required": [], "auxiliary": [item["id"] for item in source["queries"]]}
        with self.assertRaisesRegex(ValueError, "at least one"):
            PREPARE.plan_rank_queries(json.dumps(source).encode(), 0)

    def test_difference_cap_omits_only_proven_disjoint_rows_and_preserves_full_source(self):
        source = document()
        source["queries"][1]["power_bounds"].update(min_power_difference=10, max_power_difference=10)
        source["queries"][2]["power_bounds"].update(min_power_difference=9, max_power_difference=None)
        raw = json.dumps(source).encode()
        scoped, receipt = PREPARE.plan_rank_queries(raw, 0, max_power_difference=9)
        selected = json.loads(scoped)
        self.assertEqual(selected["query_roles"], {"required": ["required-first"], "auxiliary": []})
        self.assertEqual(receipt["omitted_disjoint_power_difference_query_ids"], ["required-second"])
        self.assertEqual(receipt["removed_auxiliary_query_ids"], ["helper"])
        expected = copy.deepcopy(source["queries"][2])
        expected["max_numerator_rank"] = 0
        expected["power_bounds"]["max_power_difference"] = 9
        self.assertEqual(selected["queries"], [expected])
        self.assertEqual(json.loads(raw), source)
        widened, _ = PREPARE.plan_rank_queries(raw, 0, max_power_difference=10)
        self.assertEqual(len(json.loads(widened)["queries"]), 2)
        for cap in (True, 2**63, -(2**63)-1, "9", 1.2):
            with self.subTest(cap=cap), self.assertRaises(ValueError):
                PREPARE.plan_rank_queries(raw, 0, max_power_difference=cap)


class RankCampaignStagingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        PREPARE.WORKSPACE_TEMP.mkdir(exist_ok=True)

    def fixture(self, root):
        owner = root / "owner.rrbin"
        owner.write_bytes(b"fixture only; not native rules")
        overlay = root / "overlay.rrbin"
        overlay.write_bytes(b"fixture only; not an exact source proof")
        manifest = root / "selection.json"
        manifest.write_text(json.dumps({
            "owners": [{"mask": mask, "path": str(owner), "bytes": owner.stat().st_size}
                       for mask in ("10", "01")],
            "owner_count": 2, "initial_frontier_routes": [{"opaque": "preserve"}],
            "route_record_count": 1,
            "domain_rule_overlays": [{"owner_mask": "10", "path": str(overlay),
                                      "bytes": overlay.stat().st_size}],
        }))
        queries = root / "queries.json"
        queries.write_text(json.dumps(document()))
        attachment = root / "proof-provenance.json"
        attachment.write_text('{"opaque":"retain"}\n')
        source = root / "original"
        PREPARE.STAGE.stage(manifest, queries, source / "inputs", root,
                            attachments=[attachment])
        checkpoint = source / "checkpoints" / "main"
        checkpoint.mkdir(parents=True)
        (checkpoint / "latest.json").write_text('{"protected":"checkpoint"}\n')
        return source

    def test_staging_reuses_every_owner_overlay_route_and_does_no_native_work(self):
        with tempfile.TemporaryDirectory(dir=PREPARE.WORKSPACE_TEMP) as temporary:
            root = Path(temporary)
            source = self.fixture(root)
            before = snapshot(source)
            destination = root / "rank0"
            with redirect_stdout(io.StringIO()) as output, patch.object(
                    PREPARE.tempfile, "TemporaryDirectory", wraps=tempfile.TemporaryDirectory
            ) as temporary_directory:
                self.assertEqual(PREPARE.main([
                    "--source-campaign", str(source), "--destination", str(destination),
                    "--max-numerator-rank", "0"]), 0)
            self.assertEqual(temporary_directory.call_args.kwargs["dir"], PREPARE.WORKSPACE_TEMP)
            self.assertEqual(snapshot(source), before)
            report = json.loads(output.getvalue())
            self.assertEqual(report["status"], "inputs-prepared-no-native-work")
            original = PREPARE.PRODUCTION.verify_inputs(source / "inputs")[2]
            staged = PREPARE.PRODUCTION.verify_inputs(destination / "inputs")[2]
            for field in ("owners", "domain_rule_overlays"):
                self.assertEqual([item["sha256"] for item in staged[field]],
                                 [item["sha256"] for item in original[field]])
            self.assertEqual(json.loads((source / "inputs/selection.json").read_bytes()),
                             json.loads((destination / "inputs/selection.json").read_bytes()))
            self.assertEqual((destination / "inputs" / PREPARE.SOURCE_QUERIES_NAME).read_bytes(),
                             before["inputs/queries.json"])
            receipt = json.loads((destination / "inputs" / PREPARE.RECEIPT_NAME).read_bytes())
            self.assertEqual(receipt["removed_auxiliary_query_ids"], ["helper"])
            self.assertEqual({item["name"] for item in staged["attachments"]},
                             {PREPARE.RECEIPT_NAME, PREPARE.SOURCE_QUERIES_NAME, "proof-provenance.json"})
            self.assertEqual({path.name for path in destination.iterdir()}, {"inputs"})

    def test_auxiliary_option_is_explicit_and_old_campaigns_cannot_be_overwritten(self):
        with tempfile.TemporaryDirectory(dir=PREPARE.WORKSPACE_TEMP) as temporary:
            root = Path(temporary)
            source = self.fixture(root)
            before = snapshot(source)
            for destination in (source, source / "nested", root):
                with self.subTest(destination=destination), self.assertRaises(ValueError):
                    PREPARE.prepare(source, destination, 0)
            self.assertEqual(snapshot(source), before)
            scoped = root / "with-helpers"
            receipt = PREPARE.prepare(source, scoped, 0, include_auxiliary=True)
            self.assertEqual(receipt["query_count"], 3)
            self.assertEqual(receipt["auxiliary_query_count"], 1)
            scoped_before = snapshot(scoped)
            with self.assertRaisesRegex(ValueError, "original campaign"):
                PREPARE.prepare(scoped, root / "rank1", 1)
            self.assertEqual(snapshot(scoped), scoped_before)
            self.assertFalse((root / "rank1").exists())


if __name__ == "__main__":
    unittest.main()
