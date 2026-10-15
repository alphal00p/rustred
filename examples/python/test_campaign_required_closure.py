"""Cumulative query closure is bound to native scan evidence, never the base bar."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


TELEMETRY = module("campaign_telemetry")
DASHBOARD = module("campaign_dashboard")
MONITOR = module("campaign_monitor")


def status():
    return {"elapsed_seconds": 100, "state": "running", "progress": {
        "descendant_closure": {
            "available": True, "initial_total": 67, "initial_closed": 67,
            "total_domains": 1000, "total_closed": 100, "unresolved_domains": 900,
            "snapshot_revision": 41, "graph_revision": 51, "refresh_count": 7,
            "snapshot_stale": True, "snapshot_age_seconds": 60,
        },
        "query_admission": {
            "required": 402, "original_required": 67, "appended_required": 335,
            "admitted_required": 402, "unadmitted": 0,
            "required_closed": 335, "required_closed_evaluated": True,
            "required_closure": {"available": True, "method": "closed_containing_input_root",
                "snapshot_revision": 41, "refresh_count": 7,
                "snapshot_stale": True, "snapshot_age_seconds": 60,
                "observed_query_rows": 402, "requested_query_rows": 402,
                "required_queries_total": 402},
        },
    }}


class RequiredClosureTests(unittest.TestCase):
    def test_cumulative_bar_replaces_completed_base_prefix_in_all_consumers(self):
        frame = TELEMETRY.normalize_status(status())
        self.assertTrue(frame["query_scope"]["available"])
        for width in (80, 100, 160):
            lines = DASHBOARD.render_table(frame, width=width, height=40, color=False)
            self.assertTrue(all(DASHBOARD.cell_width(line) == width for line in lines))
            primary = next(line for line in lines if "REQUIRED CLOSURE" in line)
            self.assertIn("≥335 / 402", primary)
            self.assertNotIn("67 / 67", primary)
            self.assertTrue(any("Base roots" in line and "67 / 67" in line for line in lines))
        self.assertIn("REQUIRED CLOSURE: ≥335 / 402", "\n".join(DASHBOARD.plain_summary(frame)))
        legacy = "\n".join(DASHBOARD.dashboard(status()))
        self.assertIn("≥335 / 402 cumulative requests", legacy)
        colored = "\n".join(DASHBOARD.render_table(frame, width=100, height=30, color=True))
        self.assertIn("\x1b[94m", colored)

    def test_legacy_binary_has_unknown_cumulative_not_zero_or_completed(self):
        value = status()
        admission = value["progress"]["query_admission"]
        del admission["required_closure"]
        admission["required_closed"] = 402
        frame = TELEMETRY.normalize_status(value)
        self.assertIsNone(frame["query_scope"]["required_closed"])
        self.assertIn("unknown / 402", DASHBOARD.required_progress_line(frame))
        self.assertIn("solver does not emit live query counts", DASHBOARD.required_progress_line(frame))
        self.assertIn("cumulative closure unavailable", DASHBOARD.extended_scope_line(frame))

    def test_unknown_cumulative_bar_explains_native_scan_reason(self):
        value = status()
        observation = value["progress"]["query_admission"]["required_closure"]
        observation.update(available=False, reason="awaiting first completed closure scan")
        frame = TELEMETRY.normalize_status(value)
        line = DASHBOARD.required_progress_line(frame)
        self.assertIn("unknown / 402", line)
        self.assertIn("awaiting first completed closure scan", line)
        self.assertNotIn("solver does not emit", line)
        for width in (80, 100, 160):
            lines = DASHBOARD.render_table(frame, width=width, height=40, color=False)
            self.assertTrue(all(DASHBOARD.cell_width(line) == width for line in lines))

    def test_mixed_completed_scans_cannot_reuse_old_query_numerator(self):
        for field in ("snapshot_revision", "refresh_count"):
            value = status()
            value["progress"]["descendant_closure"][field] += 1
            frame = TELEMETRY.normalize_status(value)
            self.assertFalse(frame["query_scope"]["available"])
            self.assertIsNone(frame["query_scope"]["required_closed"])

    def test_partial_admission_retains_full_denominator(self):
        value = status()
        value["progress"]["query_admission"].update(admitted_required=335, unadmitted=67)
        scope = TELEMETRY.normalize_status(value)["query_scope"]
        self.assertEqual((scope["required_closed"], scope["required"], scope["unadmitted"]), (335, 402, 67))
        value["progress"]["query_admission"]["required_closed"] = 336
        self.assertFalse(TELEMETRY.normalize_status(value)["query_scope"]["available"])

    def test_scope_extension_can_reduce_fraction_without_regressing_old_closure(self):
        before = status()
        before["progress"]["query_admission"].update(required=335, admitted_required=335, appended_required=268)
        before["progress"]["query_admission"]["required_closure"]["required_queries_total"] = 335
        old = TELEMETRY.normalize_status(before)["query_scope"]
        new = TELEMETRY.normalize_status(status())["query_scope"]
        self.assertEqual(old["required_closed"], new["required_closed"])
        self.assertGreater(old["required_closed"] / old["required"], new["required_closed"] / new["required"])

    def test_new_scope_cannot_reuse_a_previous_denominator_binding(self):
        value = status()
        value["progress"]["query_admission"]["required"] = 469
        self.assertFalse(TELEMETRY.normalize_status(value)["query_scope"]["available"])

    def test_scan_unavailable_or_not_completed_is_unknown(self):
        for mutation in ("tracker", "observer", "unevaluated", "no_scan"):
            value = status()
            admission = value["progress"]["query_admission"]
            if mutation == "tracker":
                value["progress"]["descendant_closure"]["available"] = False
            elif mutation == "observer":
                admission["required_closure"]["available"] = False
            elif mutation == "unevaluated":
                admission["required_closed_evaluated"] = False
            else:
                admission["required_closure"]["refresh_count"] = 0
                value["progress"]["descendant_closure"]["refresh_count"] = 0
            self.assertIsNone(TELEMETRY.normalize_status(value)["query_scope"]["required_closed"])

    def test_non_integer_or_impossible_counts_fail_closed(self):
        for field, bad in (("required_closed", True), ("required_closed", 1.5),
                           ("required_closed", -1), ("required_closed", 403), ("required", -1)):
            value = status()
            value["progress"]["query_admission"][field] = bad
            self.assertFalse(TELEMETRY.normalize_status(value)["query_scope"]["available"])

    def test_event_tail_retains_only_small_scan_bound_observation(self):
        value = status()["progress"]
        tail = MONITOR.EventTail(Path("unused-required-closure.jsonl"))
        tail.pending = json.dumps(value).encode()
        tail._finish(0)
        self.assertEqual(tail.query_admission["required_closed"], 335)
        tail.pending = json.dumps({"event": "epoch_heartbeat", "snapshot": {"phase": "inspect"}}).encode()
        tail._finish(1)
        self.assertEqual(tail.query_admission["required_closed"], 335)
        progress = copy.deepcopy(value)
        progress["query_admission"] = tail.query_admission
        self.assertTrue(TELEMETRY.normalize_status({"progress": progress})["query_scope"]["available"])
        progress["descendant_closure"]["refresh_count"] += 1
        self.assertFalse(TELEMETRY.normalize_status({"progress": progress})["query_scope"]["available"])

    def test_no_query_census_labels_the_old_bar_as_base_only(self):
        value = status()
        del value["progress"]["query_admission"]
        frame = TELEMETRY.normalize_status(value)
        self.assertIn("BASE ROOT CLOSURE", DASHBOARD.required_progress_line(frame))
        self.assertIn("required closure unknown", DASHBOARD.required_progress_line(frame))


if __name__ == "__main__":
    unittest.main()
