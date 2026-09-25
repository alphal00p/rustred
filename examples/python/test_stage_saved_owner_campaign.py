"""Input-only anchor planning tests: no native executable is invoked."""
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


STAGE = module("stage_saved_owner_campaign")
PRODUCTION = module("production_saved_owner_campaign")


def query(owner="10", query_id="original"):
    return {"id": query_id, "owner": owner, "lower": [0, 1], "upper": [4, 5],
            "max_numerator_rank": 2,
            "power_bounds": {"max_positive_power": 5, "min_power_difference": 1,
                             "max_power_difference": None}}


def document(rows=None):
    return {"schema": "rustred.owner-domain-queries.json.v2", "queries": rows or [query()]}


class AnchorPlanningTests(unittest.TestCase):
    def fixture(self, root):
        owner = root / "owner.rrbin"
        owner.write_bytes(b"fixture only, not a native rule program")
        manifest = root / "selection-source.json"
        manifest.write_text(json.dumps({"family_fingerprint": "fixture", "owners": [
            {"mask": mask, "path": str(owner), "bytes": owner.stat().st_size} for mask in ("10", "01")],
            "load_limits": {"max_total_input_bytes": 1234}, "initial_frontier_routes": []}))
        queries = root / "queries-source.json"
        queries.write_text(json.dumps(document(), separators=(",", ":")) + "\n\n")
        return manifest, queries

    def test_rank_and_optional_power_are_generic_native_obligations(self):
        for rank, power in ((0, None), (12, None), (15, 24), (2 ** 32 - 1, 2 ** 64 - 1)):
            with self.subTest(rank=rank, power=power):
                original = document()
                data, plan = STAGE.plan_anchor_queries(json.dumps(original).encode(), ["01", "10"], rank, power)
                rows = json.loads(data)["queries"]
                self.assertEqual(rows[:1], original["queries"])
                self.assertEqual([row["owner"] for row in rows[1:]], ["01", "10"])
                for row in rows[1:]:
                    self.assertEqual(row["lower"], [0, 0])
                    self.assertEqual(row["upper"], [None, None])
                    self.assertEqual(row["max_numerator_rank"], rank)
                    self.assertEqual(row["power_bounds"], {"max_positive_power": power,
                                                          "min_power_difference": None, "max_power_difference": None})
                self.assertEqual(plan["anchor_query_count"], 2)
                self.assertFalse(plan["descendant_clipping"])
                self.assertFalse(plan["family_closure_claim"])

    def test_selected_power_owners_only_and_deterministic_manifest_order(self):
        raw = json.dumps(document()).encode()
        data, plan = STAGE.plan_anchor_queries(raw, ["01", "10", "11"], 12, 19, ["11", "01"])
        anchors = json.loads(data)["queries"][1:]
        self.assertEqual([row["power_bounds"]["max_positive_power"] for row in anchors], [19, None, 19])
        self.assertEqual(plan["positive_power_owners"], ["01", "11"])
        self.assertEqual(plan["positive_power_owner_scope"], "listed")
        self.assertEqual((data, plan), STAGE.plan_anchor_queries(raw, ["01", "10", "11"], 12, 19, ["01", "11"]))

    def test_original_objects_and_metadata_not_filtered_or_interpreted(self):
        original = document([query("11", "unselected-owner-diagnostic"), query("10", "second")])
        original["metadata_for_future_native_parser"] = {"untouched": [1, 2]}
        original["queries"][1]["future_native_field"] = {"untouched": True}
        before = copy.deepcopy(original)
        raw, _ = STAGE.plan_anchor_queries(json.dumps(original).encode(), ["10", "01"], 3)
        result = json.loads(raw)
        self.assertEqual(result["queries"][:2], before["queries"])
        self.assertEqual(result["metadata_for_future_native_parser"], before["metadata_for_future_native_parser"])
        self.assertEqual(original, before)
        # Keeping unknown fields is not admitting them; native validation remains authoritative.

    def test_invalid_numbers_and_owner_scope_rejected(self):
        raw = json.dumps(document()).encode()
        for rank in (-1, 2 ** 32, True, 1.5, "12"):
            with self.subTest(rank=rank), self.assertRaisesRegex(ValueError, "unsigned 32"):
                STAGE.plan_anchor_queries(raw, ["10"], rank)
        for power in (-1, 2 ** 64, True, float("nan"), "19"):
            with self.subTest(power=power), self.assertRaisesRegex(ValueError, "unsigned 64"):
                STAGE.plan_anchor_queries(raw, ["10"], 12, power)
        for owners in ([], ["10", "10"], ["11"], "10", [True]):
            with self.subTest(owners=owners), self.assertRaisesRegex(ValueError, "unique selected"):
                STAGE.plan_anchor_queries(raw, ["10"], 12, 19, owners)
        with self.assertRaisesRegex(ValueError, "explicit anchor positive power"):
            STAGE.plan_anchor_queries(raw, ["10"], 12, None, ["10"])

    def test_invalid_masks_ids_and_duplicate_json_rejected(self):
        raw = json.dumps(document()).encode()
        for masks in ([], ["10", "10"], ["10", "001"], ["1x"], [True]):
            with self.subTest(masks=masks), self.assertRaisesRegex(ValueError, "common arity"):
                STAGE.plan_anchor_queries(raw, masks, 1)
        for query_id in ("", "é" * 65, 7):
            with self.subTest(query_id=query_id), self.assertRaisesRegex(ValueError, "query id"):
                STAGE.plan_anchor_queries(json.dumps(document([query(query_id=query_id)])).encode(), ["10"], 1)
        with self.assertRaisesRegex(ValueError, "query ids must be unique"):
            STAGE.plan_anchor_queries(json.dumps(document([query(), query()])).encode(), ["10"], 1)
        with self.assertRaisesRegex(ValueError, "arity-sized binary"):
            STAGE.plan_anchor_queries(json.dumps(document([query("100")])).encode(), ["10"], 1)
        with self.assertRaisesRegex(ValueError, "duplicate JSON field"):
            STAGE.plan_anchor_queries(b'{"schema":1,"schema":2,"queries":[]}', ["10"], 1)
        collision = query(query_id="owner-anchor-r1-anone-10")
        with self.assertRaisesRegex(ValueError, "collides"):
            STAGE.plan_anchor_queries(json.dumps(document([collision])).encode(), ["10"], 1)

    def test_default_staging_remains_byte_preserving(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, source = self.fixture(root)
            staged = root / "inputs"
            receipt = STAGE.stage(manifest, source, staged, root)
            self.assertEqual((staged / "queries.json").read_bytes(), source.read_bytes())
            self.assertTrue(receipt["query_bytes_unchanged"])
            self.assertNotIn("anchor_plan", receipt)
            self.assertFalse((staged / "queries-original.json").exists())

    def test_staging_preserves_original_bytes_and_all_payloads(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, source = self.fixture(root)
            staged = root / "inputs"
            receipt = STAGE.stage(manifest, source, staged, root, anchor_max_numerator_rank=12,
                                  anchor_max_positive_power=19, anchor_positive_power_owners=["01"])
            self.assertEqual((staged / "queries-original.json").read_bytes(), source.read_bytes())
            combined = json.loads((staged / "queries.json").read_bytes())
            self.assertEqual(combined["queries"][:1], json.loads(source.read_bytes())["queries"])
            self.assertFalse(receipt["query_bytes_unchanged"])
            self.assertEqual(receipt["original_queries"]["sha256"], STAGE.digest(source))
            self.assertEqual(receipt["queries_sha256"], STAGE.digest(staged / "queries.json"))
            self.assertEqual(PRODUCTION.verify_inputs(staged)[:2], (3, (staged / "queries.json").stat().st_size))
            self.assertFalse((staged / "STAGING_INCOMPLETE").exists())
            for owner in receipt["owners"]:
                self.assertEqual((staged / owner["path"]).read_bytes(), (root / "owner.rrbin").read_bytes())
            with self.assertRaises(FileExistsError):
                STAGE.stage(manifest, source, staged, root, anchor_max_numerator_rank=13)
            self.assertEqual(STAGE.digest(staged / "queries.json"), receipt["queries_sha256"])
            (staged / "queries-original.json").chmod(0o600)
            (staged / "queries-original.json").write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "original query identity changed"):
                PRODUCTION.verify_inputs(staged)

    def test_invalid_plan_has_no_destination_side_effects(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, source = self.fixture(root)
            for options in ({"anchor_max_positive_power": 19}, {"anchor_max_numerator_rank": -1},
                            {"anchor_positive_power_owners": ["10"]},
                            {"anchor_max_numerator_rank": 12, "anchor_max_positive_power": 19,
                             "anchor_positive_power_owners": ["11"]}):
                with self.subTest(options=options), self.assertRaises(ValueError):
                    STAGE.stage(manifest, source, root / "inputs", root, **options)
                self.assertFalse((root / "inputs").exists())

    def test_cli_strict_unsigned_limits(self):
        for bits in (32, 64):
            parse = STAGE.cli_unsigned(bits, "limit")
            self.assertEqual(parse("0"), 0)
            self.assertEqual(parse(str(2 ** bits - 1)), 2 ** bits - 1)
            for text in ("-1", "+1", "1.0", " 1", "１", str(2 ** bits)):
                with self.subTest(bits=bits, text=text), self.assertRaises(Exception):
                    parse(text)

    def test_cli_mixed_anchors_only_stage_inputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, source = self.fixture(root)
            staged = root / "inputs"
            result = subprocess.run([sys.executable, "-B", STAGE.__file__, "--manifest", str(manifest),
                                     "--queries", str(source), "--destination", str(staged),
                                     "--anchor-max-numerator-rank", "12", "--anchor-max-positive-power", "19",
                                     "--anchor-positive-power-owners", "01"], capture_output=True, text=True, check=True)
            self.assertEqual(json.loads(result.stdout)["anchor_plan"]["positive_power_owners"], ["01"])
            self.assertEqual(sorted(path.name for path in root.iterdir()),
                             ["inputs", "owner.rrbin", "queries-source.json", "selection-source.json"])

    def test_launcher_reports_immutable_anchors_and_resumes_same_native_input(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, source = self.fixture(root)
            campaign = root / "campaign"
            receipt = STAGE.stage(manifest, source, campaign / "inputs", root, anchor_max_numerator_rank=15,
                                  anchor_max_positive_power=24, anchor_positive_power_owners=["01"])
            executable = root / "not-run"
            executable.write_text("#!/bin/sh\nexit 99\n")
            executable.chmod(0o700)
            plans = []
            for extra in (["--executable", str(executable), "--workers", "1", "--cpus", str(min(os.sched_getaffinity(0)))],
                          ["--resume", "--max-memory-bytes", "700000000000"]):
                output = io.StringIO()
                with patch("sys.stdout", output), patch.object(PRODUCTION.os, "execv") as launch:
                    self.assertEqual(PRODUCTION.main(["--campaign-directory", str(campaign), "--json", *extra]), 0)
                launch.assert_not_called()
                plans.append(json.loads(output.getvalue()))
            for plan in plans:
                self.assertEqual(plan["anchor_plan"], receipt["anchor_plan"])
                self.assertEqual(plan["queries_sha256"], receipt["queries_sha256"])
                self.assertEqual(plan["command"][plan["command"].index("--max-queries") + 1], "3")
                self.assertFalse(plan["family_closure_claim"])
            self.assertEqual(plans[0]["steering_policy"], plans[1]["steering_policy"])
            self.assertIn("--resume", plans[1]["command"])
            self.assertFalse((campaign / "runs").exists())


class QueryOrderingTests(unittest.TestCase):
    fixture = AnchorPlanningTests.fixture

    def ordered_row(self, owner, query_id, rank=2):
        row = query(owner, query_id)
        row.update(lower=[0] * len(owner), upper=[None] * len(owner), max_numerator_rank=rank)
        return row

    def test_generic_first_seen_owner_order_stable_classes_and_helper_ranks(self):
        for arity in (3, 17):
            first, second = "1" + "0" * (arity - 1), "0" * (arity - 1) + "1"
            rows = [self.ordered_row(first, "original-first"), self.ordered_row(second, "original-second"),
                    self.ordered_row(first, "owner-anchor-low", 3),
                    self.ordered_row(first, "owner-anchor-high-a", 9),
                    self.ordered_row(first, "original-later"),
                    self.ordered_row(first, "owner-anchor-high-b", 9),
                    self.ordered_row(second, "owner-anchor-second", 4),
                    self.ordered_row(first, "owner-anchor-unbounded", None)]
            source = document(rows)
            source["future_document_field"] = {"preserved": True}
            source["queries"][0]["future_query_field"] = [1, 2]
            original = copy.deepcopy(source)
            raw = json.dumps(source).encode()
            result, plan = STAGE.plan_query_order(raw, "helpers-first")
            reordered = json.loads(result)
            self.assertEqual([row["id"] for row in reordered["queries"]],
                             ["owner-anchor-unbounded", "owner-anchor-high-a", "owner-anchor-high-b",
                              "owner-anchor-low", "original-first", "original-later",
                              "owner-anchor-second", "original-second"])
            self.assertEqual({row["id"]: row for row in reordered["queries"]}, {row["id"]: row for row in rows})
            self.assertEqual(reordered["future_document_field"], source["future_document_field"])
            self.assertEqual(source, original)
            self.assertEqual(STAGE.plan_query_order(raw, "helpers-first"), (result, plan))
            self.assertEqual(STAGE.plan_query_order(raw, "preserve"), (raw, None))
            self.assertFalse(plan["coverage_authority"])
            self.assertFalse(plan["strict_bottom_up_evaluation"])

    def test_reordering_staging_retains_source_bytes_and_payloads(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, source = self.fixture(root)
            source.write_text(json.dumps(document([query(), query(query_id="owner-anchor-existing")])) + "\n\n")
            original = source.read_bytes()
            staged = root / "inputs"
            receipt = STAGE.stage(manifest, source, staged, root, query_order="helpers-first")
            self.assertEqual(source.read_bytes(), original)
            self.assertEqual((staged / "queries-original.json").read_bytes(), original)
            self.assertEqual([row["id"] for row in json.loads((staged / "queries.json").read_text())["queries"]],
                             ["owner-anchor-existing", "original"])
            self.assertEqual(receipt["query_order"], "helpers-first")
            self.assertEqual(receipt["query_order_plan"]["query_count"], 2)
            self.assertEqual(PRODUCTION.verify_inputs(staged)[0], 2)
            self.assertNotIn("anchor_plan", receipt)
            for owner in receipt["owners"]:
                self.assertEqual((staged / owner["path"]).read_bytes(), (root / "owner.rrbin").read_bytes())
            with self.assertRaises(FileExistsError):
                STAGE.stage(manifest, source, staged, root, query_order="helpers-first")

    def test_ordering_and_appended_anchors_compose_without_losing_originals(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, source = self.fixture(root)
            receipt = STAGE.stage(manifest, source, root / "inputs", root,
                                  anchor_max_numerator_rank=12, query_order="helpers-first")
            rows = json.loads((root / "inputs/queries.json").read_text())["queries"]
            self.assertEqual([row["id"] for row in rows], ["owner-anchor-r12-anone-10", "original", "owner-anchor-r12-anone-01"])
            self.assertEqual(rows[1], query())
            self.assertEqual(receipt["anchor_plan"]["anchor_query_count"], 2)
            self.assertEqual(receipt["query_order_plan"]["query_count"], 3)

    def test_invalid_order_document_fails_before_destination_creation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, source = self.fixture(root)
            for contents, mode in ((json.dumps(document([query(), query()])), "helpers-first"),
                                   ('{"schema":1,"schema":2}', "helpers-first"),
                                   (json.dumps(document()), "unknown")):
                source.write_text(contents)
                with self.assertRaises(ValueError):
                    STAGE.stage(manifest, source, root / "inputs", root, query_order=mode)
                self.assertFalse((root / "inputs").exists())

    def production_source(self, root):
        manifest, queries = self.fixture(root)
        queries.write_text(json.dumps(document([query("10", "first"), query("01", "second")])))
        source = root / "source-campaign"
        STAGE.stage(manifest, queries, source / "inputs", root, anchor_max_numerator_rank=12)
        (source / "active-run.json").write_text('{"fixture": "untouched"}\n')
        executable = root / "not-run"
        executable.write_text("#!/bin/sh\nexit 99\n")
        executable.chmod(0o700)
        return source, executable

    def snapshot(self, root):
        return {str(path.relative_to(root)): (path.read_bytes(), path.stat().st_mtime_ns, path.stat().st_mode)
                for path in root.rglob("*") if path.is_file()}

    def test_fresh_production_copy_defaults_helpers_first_and_never_launches(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, executable = self.production_source(root)
            before = self.snapshot(source)
            destination = root / "future-campaign"
            output = io.StringIO()
            with patch("sys.stdout", output), patch.object(PRODUCTION.os, "execv") as launch:
                self.assertEqual(PRODUCTION.main(["--prepare-from", str(source), "--campaign-directory", str(destination),
                    "--executable", str(executable), "--workers", "1", "--cpus", str(min(os.sched_getaffinity(0))),
                    "--max-memory-bytes", "700000000000", "--json"]), 0)
            launch.assert_not_called()
            self.assertEqual(self.snapshot(source), before)
            plan = json.loads(output.getvalue())
            self.assertEqual(plan["query_order"], "helpers-first")
            self.assertFalse(plan["launch_requested"])
            self.assertEqual(plan["requested_hard_memory_bytes"], 700_000_000_000)
            self.assertEqual(plan["ram_guard_margin_percent"], 5.0)
            self.assertEqual(plan["command"][plan["command"].index("--max-queries") + 1], "4")
            self.assertFalse((destination / "active-run.json").exists())
            self.assertFalse((destination / "runs").exists())
            source_rows = json.loads((source / "inputs/queries.json").read_text())["queries"]
            new_rows = json.loads((destination / "inputs/queries.json").read_text())["queries"]
            self.assertEqual({row["id"]: row for row in new_rows}, {row["id"]: row for row in source_rows})
            self.assertEqual([row["id"] for row in new_rows],
                             ["owner-anchor-r12-anone-10", "first", "owner-anchor-r12-anone-01", "second"])
            self.assertEqual((destination / "inputs/queries-original.json").read_bytes(), (source / "inputs/queries.json").read_bytes())
            output = io.StringIO()
            with patch("sys.stdout", output), patch.object(PRODUCTION.os, "execv") as launch:
                self.assertEqual(PRODUCTION.main(["--campaign-directory", str(destination), "--resume", "--json"]), 0)
            launch.assert_not_called()
            resumed = json.loads(output.getvalue())
            self.assertEqual(resumed["steering_policy"], plan["steering_policy"])
            self.assertEqual(resumed["query_order"], "helpers-first")
            self.assertEqual(resumed["requested_hard_memory_bytes"], 700_000_000_000)
            self.assertEqual(self.snapshot(source), before)

    def test_fresh_preserve_override_and_source_tampering_rejection(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, _ = self.production_source(root)
            destination = root / "preserved"
            PRODUCTION.prepare_from(source, destination, "preserve")
            self.assertEqual((destination / "inputs/queries.json").read_bytes(), (source / "inputs/queries.json").read_bytes())
            self.assertFalse((destination / "inputs/queries-original.json").exists())
            (source / "inputs/queries.json").chmod(0o600)
            (source / "inputs/queries.json").write_text("tampered")
            with self.assertRaisesRegex(ValueError, "digest changed"):
                PRODUCTION.prepare_from(source, root / "refused", "helpers-first")
            self.assertFalse((root / "refused").exists())

    def test_prepare_refuses_existing_nested_and_symlink_destinations_before_writes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, _ = self.production_source(root)
            existing = root / "existing"
            existing.mkdir()
            link = root / "link"
            link.symlink_to(root / "missing")
            before = self.snapshot(source)
            for destination in (existing, source, source / "nested", root, link):
                with self.subTest(destination=destination), self.assertRaises(ValueError), patch.object(PRODUCTION, "verify_inputs") as verify:
                    PRODUCTION.prepare_from(source, destination, "helpers-first")
                verify.assert_not_called()
            self.assertEqual(self.snapshot(source), before)
            self.assertEqual(list(existing.iterdir()), [])
            self.assertFalse((source / "nested").exists())
            self.assertFalse((root / "missing").exists())

    def test_prepare_resume_and_existing_query_order_are_cli_errors(self):
        for extra in (["--prepare-from", "source", "--resume"], ["--query-order", "helpers-first"],
                      ["--query-order", "preserve", "--resume"]):
            with self.subTest(extra=extra), patch("sys.stderr", io.StringIO()), \
                    patch.object(PRODUCTION, "verify_inputs") as verify, patch.object(PRODUCTION, "freeze_executable") as freeze:
                with self.assertRaises(SystemExit) as error:
                    PRODUCTION.main(extra)
                self.assertEqual(error.exception.code, 2)
                verify.assert_not_called()
                freeze.assert_not_called()

    def test_prepare_refuses_nested_destination_through_symlinked_source_inputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, _ = self.production_source(root)
            alias = root / "source-alias"
            alias.mkdir()
            (alias / "inputs").symlink_to(source / "inputs", target_is_directory=True)
            destination = source / "inputs/new-campaign"
            before = self.snapshot(source)
            with self.assertRaisesRegex(ValueError, "source inputs.*not nested"), patch.object(PRODUCTION, "verify_inputs") as verify:
                PRODUCTION.prepare_from(alias, destination, "helpers-first")
            verify.assert_not_called()
            self.assertFalse(destination.exists())
            self.assertEqual(self.snapshot(source), before)


if __name__ == "__main__":
    unittest.main()
