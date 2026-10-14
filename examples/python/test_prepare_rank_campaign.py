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

    def fixture(self, root, query_document=None):
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
        queries.write_text(json.dumps(document() if query_document is None else query_document))
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

    def scoped_fixture(self, root):
        source = self.fixture(root)
        scoped = root / "source-rank0"
        PREPARE.prepare(source, scoped, 0, max_power_difference=4)
        return source, scoped

    def replace_attachment(self, campaign, name, raw):
        """Coherently rewrite only a disposable fixture's outer inventory."""
        path = campaign / "inputs" / name
        path.chmod(0o644)
        path.write_bytes(raw)
        receipt_path = campaign / "inputs/input-receipt.json"
        receipt = json.loads(receipt_path.read_bytes())
        for item in receipt["attachments"]:
            if item["name"] == name:
                item.update(bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest())
        receipt_path.write_text(json.dumps(receipt))

    def test_original_query_opt_in_restores_templates_without_copying_checkpoint(self):
        with tempfile.TemporaryDirectory(dir=PREPARE.WORKSPACE_TEMP) as temporary:
            root = Path(temporary)
            source, scoped = self.scoped_fixture(root)
            (scoped / "checkpoints/main").mkdir(parents=True)
            (scoped / "checkpoints/main/latest.json").write_text('{"protected":"ledger"}')
            (scoped / "amendments").mkdir()
            (scoped / "amendments/amendment-0001.json").write_text('{"protected":"scope"}')
            before = snapshot(scoped)
            destination = root / "fresh-rank2"
            with redirect_stdout(io.StringIO()) as output:
                self.assertEqual(PREPARE.main([
                    "--source-campaign", str(scoped), "--source-original-queries",
                    "--destination", str(destination), "--max-numerator-rank", "2",
                    "--max-power-difference", "6"]), 0)
            receipt = json.loads(output.getvalue())
            self.assertEqual(receipt["source_query_mode"], "authenticated-original-attachment")
            self.assertFalse(receipt["source_checkpoint_reused"])
            self.assertEqual(receipt["source_campaign_queries_sha256"],
                             hashlib.sha256(before["inputs/queries.json"]).hexdigest())
            self.assertEqual(receipt["source_rank_scope_receipt_sha256"],
                             hashlib.sha256(before[f"inputs/{PREPARE.RECEIPT_NAME}"]).hexdigest())
            original_bytes = (source / "inputs/queries.json").read_bytes()
            expected, _ = PREPARE.plan_rank_queries(original_bytes, 2, max_power_difference=6)
            self.assertEqual((destination / "inputs/queries.json").read_bytes(), expected)
            self.assertEqual((destination / "inputs" / PREPARE.SOURCE_QUERIES_NAME).read_bytes(),
                             original_bytes)
            self.assertEqual([q["max_numerator_rank"] for q in json.loads(expected)["queries"]], [2, 1])
            self.assertEqual([q["power_bounds"]["max_power_difference"]
                              for q in json.loads(expected)["queries"]], [6, 6])
            staged = PREPARE.PRODUCTION.verify_inputs(destination / "inputs")[2]
            self.assertEqual([item["name"] for item in staged["attachments"]],
                             ["proof-provenance.json", PREPARE.SOURCE_QUERIES_NAME, PREPARE.RECEIPT_NAME])
            self.assertEqual((destination / "inputs/selection.json").read_bytes(),
                             before["inputs/selection.json"])
            self.assertEqual({path.name for path in destination.iterdir()}, {"inputs"})
            self.assertEqual(snapshot(scoped), before)

    def test_original_query_mode_requires_one_complete_canonical_attachment_pair(self):
        for mutation in ("no-pair", "missing-source", "duplicate-source", "duplicate-receipt"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory(
                    dir=PREPARE.WORKSPACE_TEMP) as temporary:
                root = Path(temporary)
                source, scoped = self.scoped_fixture(root)
                if mutation == "no-pair":
                    scoped = source
                else:
                    path = scoped / "inputs/input-receipt.json"
                    receipt = json.loads(path.read_bytes())
                    if mutation == "missing-source":
                        receipt["attachments"] = [item for item in receipt["attachments"]
                                                  if item["name"] != PREPARE.SOURCE_QUERIES_NAME]
                    else:
                        name = (PREPARE.SOURCE_QUERIES_NAME if mutation == "duplicate-source"
                                else PREPARE.RECEIPT_NAME)
                        receipt["attachments"].append(copy.deepcopy(next(
                            item for item in receipt["attachments"] if item["name"] == name)))
                    path.write_text(json.dumps(receipt))
                with self.assertRaisesRegex(ValueError, "exactly one canonical"):
                    PREPARE.prepare(scoped, root / "fresh", 2, source_original_queries=True)
                self.assertFalse((root / "fresh").exists())

    def test_original_query_opt_in_restores_d10_request_omitted_from_d9_base(self):
        with tempfile.TemporaryDirectory(dir=PREPARE.WORKSPACE_TEMP) as temporary:
            root = Path(temporary)
            original = document()
            original["queries"][1].update(max_numerator_rank=6, upper=[15, 6])
            original["queries"][1]["power_bounds"].update(
                max_positive_power=16, min_power_difference=10, max_power_difference=10)
            original["queries"][2]["power_bounds"].update(
                max_positive_power=23, min_power_difference=9, max_power_difference=None)
            source = self.fixture(root, original)
            capped = root / "source-d9"
            PREPARE.prepare(source, capped, 0, max_power_difference=9)
            self.assertEqual(len(json.loads((capped / "inputs/queries.json").read_bytes())["queries"]), 1)
            fresh = root / "fresh-d10"
            PREPARE.prepare(capped, fresh, 6, max_power_difference=10, source_original_queries=True)
            expected, _ = PREPARE.plan_rank_queries(
                (source / "inputs/queries.json").read_bytes(), 6, max_power_difference=10)
            self.assertEqual((fresh / "inputs/queries.json").read_bytes(), expected)
            self.assertEqual(json.loads(expected)["queries"][0], original["queries"][1])
            self.assertEqual(len(json.loads(expected)["queries"]), 2)

    def test_original_query_mode_rejects_tampered_or_stale_source_binding(self):
        for mutation in ("unbound-bytes", "wrong-source-sha", "wrong-base", "wrong-selection", "schema"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory(
                    dir=PREPARE.WORKSPACE_TEMP) as temporary:
                root = Path(temporary)
                _, scoped = self.scoped_fixture(root)
                if mutation == "unbound-bytes":
                    path = scoped / "inputs" / PREPARE.SOURCE_QUERIES_NAME
                    path.chmod(0o644)
                    path.write_bytes(path.read_bytes() + b" ")
                elif mutation == "wrong-source-sha":
                    raw = (scoped / "inputs" / PREPARE.SOURCE_QUERIES_NAME).read_bytes() + b" "
                    self.replace_attachment(scoped, PREPARE.SOURCE_QUERIES_NAME, raw)
                else:
                    path = scoped / "inputs" / PREPARE.RECEIPT_NAME
                    receipt = json.loads(path.read_bytes())
                    if mutation == "wrong-base":
                        receipt["max_numerator_rank"] = 2
                    elif mutation == "wrong-selection":
                        receipt["source_selection_sha256"] = "0" * 64
                    else:
                        receipt["schema"] = "unrecognized"
                    self.replace_attachment(scoped, PREPARE.RECEIPT_NAME, json.dumps(receipt).encode())
                before = snapshot(scoped)
                with self.assertRaises(ValueError):
                    PREPARE.prepare(scoped, root / "fresh", 2, source_original_queries=True)
                self.assertEqual(snapshot(scoped), before)
                self.assertFalse((root / "fresh").exists())

    def test_fresh_original_template_scope_supports_existing_monotone_extension(self):
        # Native checkpoint bytes below are fixture metadata only, not a replay
        # or closure certificate. Exercise the actual Python scope-chain API.
        EXTEND = PREPARE.module("extend_rank_campaign")
        with tempfile.TemporaryDirectory(dir=PREPARE.WORKSPACE_TEMP) as temporary:
            root = Path(temporary)
            _, scoped = self.scoped_fixture(root)
            fresh = root / "fresh-rank1"
            PREPARE.prepare(scoped, fresh, 1, max_power_difference=5, source_original_queries=True)
            checkpoint = fresh / "checkpoints/main"
            checkpoint.mkdir(parents=True)
            (checkpoint / "checkpoint.lock").touch()
            metadata = {"schema": 5, "request": "a" * 64, "initial_admission": "complete",
                        "processed_queries": 2, "total_queries": 2,
                        "engine_certification_void": False, "amendments": [], "p0": 1}
            raw = EXTEND.encoded(metadata)
            (checkpoint / "metadata.part").write_bytes(raw)
            (checkpoint / "latest.json").write_bytes(EXTEND.encoded({"manifest": {
                "format": "RUSTRED-WALK-CP6", "schema": 3, "generation": 1,
                "resumable": True, "files": [{"key": "meta", "file": "metadata.part", "bytes": len(raw)}]}}))
            (fresh / "bin").mkdir()
            (fresh / "bin/steering.json").write_text("{}")
            before = snapshot(fresh)
            preview = EXTEND.extend(fresh, 2, max_power_difference=6, dry_run=True)
            self.assertEqual(snapshot(fresh), before)
            self.assertEqual(preview["previous_rank"], 1)
            self.assertEqual(preview["previous_max_power_difference"], 5)
            self.assertEqual(preview["cumulative_required_queries"], 4)
            plan = EXTEND.extend(fresh, 2, max_power_difference=6)
            amendment = json.loads(Path(plan["amendment"]).read_bytes())
            self.assertEqual([q["max_numerator_rank"] for q in amendment["queries"]], [2, 1])
            self.assertEqual([q["power_bounds"]["max_power_difference"]
                              for q in amendment["queries"]], [6, 6])
            self.assertEqual({key: snapshot(fresh)[key] for key in before}, before)
            self.assertEqual(set(snapshot(fresh)) - set(before), {"amendments/amendment-0001.json"})
            with self.assertRaisesRegex(ValueError, "lower"):
                EXTEND.extend(fresh, 0, max_power_difference=6, dry_run=True)


if __name__ == "__main__":
    unittest.main()
