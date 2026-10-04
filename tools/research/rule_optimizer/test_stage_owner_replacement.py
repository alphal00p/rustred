"""Input-only staging tests; no native decoder, proof or campaign process."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    "stage_owner_replacement", Path(__file__).with_name("stage_owner_replacement.py"))
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)


class ReplacementStagingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "source"
        self.destination = self.root / "candidate"
        payloads = {}
        for name in ("owner-a", "owner-b", "overlay-a", "overlay-b", "new-owner", "new-overlay", "attachment"):
            path = self.root / name
            path.write_bytes(name.encode())
            payloads[name] = path
        self.manifest = {
            "owners": [{"mask": mask, "path": str(payloads[name]),
                        "bytes": payloads[name].stat().st_size,
                        "sha256": M.digest(payloads[name]), "native_ordinal": i}
                       for i, (mask, name) in enumerate((("10", "owner-a"), ("01", "owner-b")))],
            "owner_count": 2,
            "initial_frontier_routes": [{"source": "11", "owner": "10", "integer": 2**64 - 1}],
            "route_record_count": 1,
            "domain_rule_overlays": [{"owner_mask": mask, "path": str(payloads[name]),
                                      "bytes": payloads[name].stat().st_size}
                                     for mask, name in (("10", "overlay-a"), ("01", "overlay-b"))],
            "load_limits": {"max_operations": 2**64 - 1},
            "total_bundle_bytes": sum(payloads[n].stat().st_size for n in ("owner-a", "owner-b")),
        }
        manifest_path = self.root / "manifest.json"
        manifest_path.write_text(json.dumps(self.manifest))
        query_path = self.root / "queries.json"
        self.query_bytes = (json.dumps({
            "schema": "rustred.owner-domain-queries.json.v2",
            "queries": [{"id": name, "owner": mask, "lower": [0, 0], "upper": [None, None],
                         "max_numerator_rank": 1, "power_bounds": {"max_positive_power": 2**64 - 1}}
                        for name, mask in (("required-a", "10"), ("required-b", "01"), ("helper", "10"))],
            "query_roles": {"required": ["required-a", "required-b"], "auxiliary": ["helper"]},
        }, indent=3) + "\n").encode()
        query_path.write_bytes(self.query_bytes)
        M.STAGER.stage(manifest_path, query_path, self.source / "inputs", self.root,
                       attachments=[payloads["attachment"]])
        self.args = dict(
            source_campaign=self.source, destination=self.destination, owner_mask="10",
            owner_payload=payloads["new-owner"], owner_sha256=M.digest(payloads["new-owner"]),
            overlay_payload=payloads["new-overlay"], overlay_sha256=M.digest(payloads["new-overlay"]),
            selection_sha256=M.digest(self.source / "inputs" / "selection.json"),
            queries_sha256=M.digest(self.source / "inputs" / "queries.json"),
            evidence=[payloads["attachment"]],
        )

    def test_dry_run_does_not_create_campaign(self):
        plan = M.prepare(**self.args)
        self.assertFalse(plan["staged"])
        self.assertFalse(self.destination.exists())
        self.assertEqual(plan["query_roles"], {"required": 2, "auxiliary": 1})

    def test_portable_only_two_replacements_and_exact_query_bytes(self):
        before = {str(p.relative_to(self.source)): M.digest(p)
                  for p in self.source.rglob("*") if p.is_file()}
        result = M.prepare(**self.args, execute=True)
        self.assertTrue(result["staged"])
        self.assertFalse(result["native_admission_performed"])
        self.assertFalse(result["campaign_started"])
        self.assertEqual((self.destination / "inputs" / "queries.json").read_bytes(), self.query_bytes)
        self.assertEqual(before, {str(p.relative_to(self.source)): M.digest(p)
                                  for p in self.source.rglob("*") if p.is_file()})
        count, _, receipt = M.PRODUCTION.verify_inputs(self.destination / "inputs")
        self.assertEqual(count, 3)
        self.assertEqual(receipt["owners"][0]["sha256"], self.args["owner_sha256"])
        self.assertEqual(receipt["domain_rule_overlays"][0]["sha256"], self.args["overlay_sha256"])
        selection = json.loads((self.destination / "inputs" / "selection.json").read_text())
        self.assertEqual(selection["initial_frontier_routes"], self.manifest["initial_frontier_routes"])
        self.assertEqual(selection["load_limits"], self.manifest["load_limits"])
        self.assertEqual((self.destination / "inputs" / "attachment").read_bytes(), b"attachment")
        self.assertEqual((self.destination / result["evidence"][0]["staged_path"]).read_bytes(), b"attachment")
        self.assertFalse((self.destination / "STAGING_INCOMPLETE").exists())
        self.assertFalse((self.destination / "bin").exists())
        self.assertFalse((self.destination / "steering.json").exists())

    def test_hash_mismatch_precedes_writes(self):
        for field in ("owner_sha256", "overlay_sha256", "selection_sha256", "queries_sha256"):
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "SHA256"):
                M.prepare(**(self.args | {field: "0" * 64}), execute=True)
            self.assertFalse(self.destination.exists())

    def test_existing_destination_refused(self):
        self.destination.mkdir()
        with self.assertRaisesRegex(ValueError, "must not exist"):
            M.prepare(**self.args, execute=True)

    def test_dangling_destination_symlink_refused(self):
        self.destination.symlink_to(self.root / "not-created")
        with self.assertRaisesRegex(ValueError, "must not exist"):
            M.prepare(**self.args, execute=True)
        self.assertFalse((self.root / "not-created").exists())

    def test_nested_source_destination_refused(self):
        for dest in (self.source, self.source / "child", self.root):
            with self.subTest(dest=dest), self.assertRaisesRegex(ValueError, "disjoint"):
                M.prepare(**(self.args | {"destination": dest}), execute=True)

    def test_absent_owner_refused(self):
        with self.assertRaisesRegex(ValueError, "exactly one"):
            M.prepare(**(self.args | {"owner_mask": "11"}), execute=True)
        self.assertFalse(self.destination.exists())

    def test_source_payload_change_refused(self):
        source = self.source / "inputs" / "owners" / "0001-01.rrbin"
        source.chmod(0o644)
        source.write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "staged owner identity changed"):
            M.prepare(**self.args, execute=True)
        self.assertFalse(self.destination.exists())

    def test_source_roles_required(self):
        query = self.source / "inputs" / "queries.json"
        document = json.loads(query.read_text())
        document.pop("query_roles")
        query.chmod(0o644)
        query.write_text(json.dumps(document))
        receipt_path = self.source / "inputs" / "input-receipt.json"
        receipt = json.loads(receipt_path.read_text())
        receipt["queries_sha256"] = M.digest(query)
        receipt_path.write_text(json.dumps(receipt))
        with self.assertRaisesRegex(ValueError, "explicit complete"):
            M.prepare(**(self.args | {"queries_sha256": M.digest(query)}), execute=True)


if __name__ == "__main__":
    unittest.main()
