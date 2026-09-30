"""Synthetic comparison tests, not native checkpoint-validity certificates."""
import copy
import json
from pathlib import Path
import struct
import tempfile
import unittest

from compare_state import DEFAULTS, HEADER, capture, compare, differences
from contract import FORMAT
from receipts import ROOT


def fixture(directory, schema):
    directory.mkdir()
    semantics = 4 if schema == 3 else 3
    meta = {
        "schema": 4 if schema == 3 else 3, "request": f"request-v{semantics}",
        "owner_count": 1, "owners_digest": [1] * 32, "walk_semantics_version": semantics,
        "lockstep_b": 16, "k": 1, "watermark": 2, "p0": 1,
        "max_domains": 1000, "max_events": 1000, "max_frontiers": 1000,
        "ledger_counts": [0, 0, 1, 0, 0, 1, 0, 0],
        "walk": {"events": 1, "completed": 1, "natives": 1, "aliases": 1, "merges": 1},
        "lookup": {"forward_candidates": 5}, "verify": {"calls": 3},
        "closure": {"initial": 1, "unavailable": None, "revision": 3,
                    "snapshot_revision": 3, "initial_closed": 1, "total_closed": 2,
                    "inspected": 1, "refresh_count": 1, "refresh_seconds": 0.01},
        "edge_runs": 2, "edges": 1, "self_edges": 0,
        "records_digest": f"records-v{semantics}", "edge_digest": "edges",
        "initial_admission": "complete", "total_queries": 1, "processed_queries": 1,
        "input_frontiers": 0, "stop_reason": None, "operational_stop": None,
        "admission_failure": None, "amendments": [], "quarantined": 0,
        "abandoned_obligations": 0, "g2": "union", "imported_prefix": 0,
        "engine_certification_void": False,
    }
    if schema == 3:
        meta.update(record_schema=1, preparation={"helpers": 2, "obligations": 1000, "retirements": 1000})
        meta.update(DEFAULTS)
    parts = {
        # Bodies only exercise the unchanged container reader and comparison;
        # the native gate is responsible for mathematical/header-body validity.
        "state-1": (2, b"domain-A" + b"domain-B"),
        "state-2": (2, bytes([3, 1])), "state-3": (1, struct.pack("<Q", 3)),
        "state-4": (2, struct.pack("<QQ", 2 << 61 | 1, 5 << 61)),
        "state-5": (2, struct.pack("<6I", 0, 1, 1, 1, 0, 0)),
        "state-6": (1, b"anchor-scope-pin"),
        "state-7": (0, struct.pack("<9Q", 1, 1, 16, 1, 1, 2, 0, 0, 0)),
        "state-8": (2, bytes([7, 5])), "state-9": (0, b""),
        "meta": (1, json.dumps(meta).encode()),
        "owners": (1, (json.dumps("1" * 64) + "\n").encode()),
        "inputs": (1, b'{"domain":0,"id":"q","role":"required","role_declared":true}\n'),
        "input-frontiers": (0, b""),
        "record-segments": (1, json.dumps([{"generation": 1,
            "file": "records.bin" if schema == 3 else "records.jsonl", "first": 0,
            "count": 2, "bytes": 9, "blake3": "0" * 64}]).encode()),
        "orthants": (1, b"EPORTH01" + struct.pack("<QI", 1, 0)),
    }
    refs = []
    for key, (count, payload) in parts.items():
        if key.startswith("state-"):
            payload = HEADER.pack(b"EPC6PART", 1, 2, int(key[6:]), count) + payload
        name = key + ".part"
        (directory / name).write_bytes(payload)
        refs.append({"key": key, "file": name, "count": count, "bytes": len(payload), "blake3": [0] * 32})
    manifest = {"format": FORMAT, "schema": schema, "generation": 1, "arity": 2,
                "walk_semantics_version": semantics, "resumable": True, "files": refs}
    (directory / "latest.json").write_text(json.dumps({"manifest": manifest, "blake3": [0] * 32}))
    return meta


class ComparisonTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="cp6-state-test-", dir=ROOT / "TMP")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.left, self.right = self.base / "old", self.base / "new"
        fixture(self.left, 2)
        fixture(self.right, 3)
        self.queries = self.base / "queries.json"
        self.queries.write_text('{"queries":[{"id":"q","powers":[1,1]}]}')

    def report(self, right_queries=None):
        return compare(self.left, self.right, self.queries, right_queries or self.queries)

    def change_meta(self, mutate):
        path = self.right / "meta.part"
        value = json.loads(path.read_text())
        mutate(value)
        self.change_part("meta", json.dumps(value).encode())

    def change_part(self, key, payload):
        (self.right / (key + ".part")).write_bytes(payload)
        path = self.right / "latest.json"
        envelope = json.loads(path.read_text())
        next(ref for ref in envelope["manifest"]["files"] if ref["key"] == key)["bytes"] = len(payload)
        path.write_text(json.dumps(envelope))

    def test_historical_and_typed_receipts_compare_only_durable_state(self):
        report = self.report()
        self.assertTrue(report["durable_mathematical_state_equal"])
        self.assertEqual(report["full_record_equality"], "NOT_COMPARED")
        self.assertTrue(report["native_authentication_and_cold_all_required_separately"])
        self.assertIn("schema", report["diagnostic_differences"])
        self.assertIn("scalar_diagnostics.records_digest", report["diagnostic_differences"])

    def test_geometry_ledger_targets_anchors_nodes_and_orthants_mutations_fail(self):
        for key, semantic in [("state-1", "domain_geometry"), ("state-2", "node_flags"),
                              ("state-3", "lookup_live_bits"), ("state-4", "ledger"),
                              ("state-5", "ordered_dependency_targets"),
                              ("state-6", "anchor_scopes_and_pins"), ("orthants", "orthants")]:
            with self.subTest(key=key):
                path = self.right / (key + ".part")
                original = path.read_bytes()
                changed = bytearray(original)
                changed[-1] ^= 1
                path.write_bytes(changed)
                report = self.report()
                self.assertFalse(report["durable_mathematical_state_equal"])
                self.assertIn(semantic + ".sha256", report["differences"])
                path.write_bytes(original)

    def test_physical_counts_timings_and_cached_closed_bits_are_reported(self):
        self.change_meta(lambda meta: meta.update(lookup={"forward_candidates": 500}, verify={"calls": 300}))
        self.change_meta(lambda meta: meta["closure"].update(total_closed=0, initial_closed=0,
                         refresh_count=5, snapshot_revision=0, refresh_seconds=0.9))
        path = self.right / "state-8.part"
        data = bytearray(path.read_bytes())
        data[HEADER.size:] = bytes([3, 1])
        path.write_bytes(data)
        report = self.report()
        self.assertTrue(report["durable_mathematical_state_equal"])
        self.assertIn("section_receipts.closure_flags.sha256", report["diagnostic_differences"])
        self.assertIn("scalar_diagnostics.lookup.forward_candidates", report["diagnostic_differences"])
        data[-1] ^= 1  # Unlike cached closed bit2, sealed bit0 is semantic.
        path.write_bytes(data)
        self.assertIn("closure_flags.sha256", self.report()["differences"])

    def test_roles_queries_and_logical_work_mutations_fail(self):
        self.change_meta(lambda meta: meta["walk"].update(events=2))
        self.assertIn("logical_metadata.walk.events", self.report()["differences"])
        self.change_meta(lambda meta: meta["walk"].update(events=1))
        path = self.right / "inputs.part"
        self.change_part("inputs", path.read_bytes().replace(b'"required"', b'"auxiliary"'))
        self.assertIn("inputs[0].role", self.report()["differences"])
        self.change_part("inputs", path.read_bytes().replace(b'"auxiliary"', b'"required"'))
        different = self.base / "different-queries.json"
        different.write_text(self.queries.read_text().replace("[1,1]", "[1,2]"))
        self.assertIn("query_file.sha256", self.report(different)["differences"])

    def test_drained_dispatch_transport_identity_is_not_graph_authority(self):
        path = self.right / "state-7.part"
        data = bytearray(path.read_bytes())
        struct.pack_into("<Q", data, HEADER.size + 3 * 8, 9)
        path.write_bytes(data)
        report = self.report()
        self.assertTrue(report["durable_mathematical_state_equal"])
        self.assertIn("section_receipts.dispatch.session_and_sequence_counter[0]", report["diagnostic_differences"])
        struct.pack_into("<Q", data, HEADER.size + 5 * 8, 1)
        path.write_bytes(data)
        self.assertIn("dispatch.cursor", self.report()["differences"])

    def test_missing_unknown_duplicate_and_unsupported_sections_fail_closed(self):
        path = self.right / "latest.json"
        original = json.loads(path.read_text())
        for mutation in (lambda m: m.update(schema=99),
                         lambda m: m["files"].pop(),
                         lambda m: m["files"].append(copy.deepcopy(m["files"][0])),
                         lambda m: m["files"][0].update(key="state-99"),
                         lambda m: m["files"][0].update(file="../other")):
            with self.subTest(mutation=mutation):
                changed = copy.deepcopy(original)
                mutation(changed["manifest"])
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):
                    self.report()
        path.write_text(json.dumps(original))

    def test_header_versions_size_symlink_and_invalid_flags_are_refused(self):
        path = self.right / "state-1.part"
        original = path.read_bytes()
        data = bytearray(original)
        struct.pack_into("<I", data, 8, 2)
        path.write_bytes(data)
        with self.assertRaisesRegex(ValueError, "header"):
            self.report()
        path.write_bytes(original[:-1])
        with self.assertRaisesRegex(ValueError, "byte count"):
            self.report()
        path.unlink()
        path.symlink_to(self.left / "state-1.part")
        with self.assertRaisesRegex(ValueError, "nonregular"):
            self.report()
        path.unlink()
        path.write_bytes(original)
        path = self.right / "state-8.part"
        data = bytearray(path.read_bytes())
        data[-1] = 8
        path.write_bytes(data)
        with self.assertRaisesRegex(ValueError, "closure flag"):
            self.report()

    def test_pending_incomplete_or_unknown_scalars_refuse(self):
        self.change_meta(lambda meta: meta["ledger_counts"].__setitem__(0, 1))
        with self.assertRaisesRegex(ValueError, "drained"):
            self.report()
        self.change_meta(lambda meta: meta["ledger_counts"].__setitem__(0, 0))
        self.change_meta(lambda meta: meta.update(new_semantic_field=1))
        with self.assertRaisesRegex(ValueError, "scalar fields"):
            self.report()

    def test_json_scalar_types_are_not_silently_equated(self):
        self.assertEqual(differences({"n": 1}, {"n": True}), ["n"])

    def enable_scalar5(self, extra=0, retained=None):
        self.change_meta(lambda meta: meta.update(
            schema=5, epoch_base_window=16 - extra,
            epoch_result_escrow_jobs=extra, epoch_result_escrow_bytes=retained))

    def test_scalar5_disabled_policy_matches_historical_no_escrow(self):
        self.enable_scalar5()
        report = self.report()
        self.assertTrue(report["durable_mathematical_state_equal"])
        self.assertEqual(report["right"]["semantic"]["escrow_policy"], {
            "epoch_base_window": 16, "epoch_result_escrow_jobs": 0,
            "epoch_result_escrow_bytes": None,
        })

    def test_scalar5_enabled_policy_is_not_normalized_away(self):
        self.enable_scalar5(extra=4, retained=1024)
        self.change_meta(lambda meta: meta.update(epoch_rolling=True))
        report = self.report()
        self.assertFalse(report["durable_mathematical_state_equal"])
        self.assertIn("escrow_policy.epoch_result_escrow_jobs", report["differences"])
        self.assertIn("escrow_policy.epoch_result_escrow_bytes", report["differences"])
        self.assertIn("escrow_policy.epoch_base_window", report["differences"])

    def test_scalar5_native_omitted_publication_default_is_accepted(self):
        self.enable_scalar5(extra=4, retained=1024)
        self.change_meta(lambda meta: meta.update(epoch_rolling=True))
        self.change_meta(lambda meta: meta.pop("epoch_publication_order", None))
        omitted = self.report()["right"]["semantic"]
        self.change_meta(lambda meta: meta.update(epoch_publication_order="oldest-prefix"))
        explicit = self.report()["right"]["semantic"]
        self.assertEqual(omitted, explicit)
        self.change_meta(lambda meta: meta.update(epoch_publication_order="oldest_sequence_prefix"))
        with self.assertRaisesRegex(ValueError, "enabled escrow"):
            self.report()  # Summary spelling is not the native scalar spelling.

    def test_scalar5_missing_invalid_or_inconsistent_policy_refused(self):
        self.enable_scalar5()
        original = json.loads((self.right / "meta.part").read_text())
        mutations = [
            lambda m: m.pop("epoch_base_window"),
            lambda m: m.pop("epoch_result_escrow_jobs"),
            lambda m: m.pop("epoch_result_escrow_bytes"),
            lambda m: m.update(epoch_base_window=True),
            lambda m: m.update(epoch_result_escrow_jobs=-1),
            lambda m: m.update(epoch_base_window=4097, lockstep_b=4097),
            lambda m: m.update(epoch_base_window=15),
            lambda m: m.update(epoch_result_escrow_bytes=1),
            lambda m: m.update(epoch_base_window=12, epoch_result_escrow_jobs=4),
            lambda m: m.update(epoch_base_window=12, epoch_result_escrow_jobs=4,
                               epoch_result_escrow_bytes=1, epoch_rolling=False),
            lambda m: m.update(epoch_base_window=12, epoch_result_escrow_jobs=4,
                               epoch_result_escrow_bytes=True, epoch_rolling=True),
            lambda m: m.update(epoch_base_window=12, epoch_result_escrow_jobs=4,
                               epoch_result_escrow_bytes=1, epoch_rolling=True,
                               epoch_publication_order="oldest_ready"),
        ]
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                changed = copy.deepcopy(original)
                mutation(changed)
                self.change_part("meta", json.dumps(changed).encode())
                with self.assertRaisesRegex(ValueError, "escrow"):
                    self.report()

    def test_older_scalar_cannot_smuggle_new_operational_fields(self):
        self.change_meta(lambda meta: meta.update(epoch_result_escrow_jobs=0))
        with self.assertRaisesRegex(ValueError, "older scalar"):
            self.report()


if __name__ == "__main__":
    unittest.main()
