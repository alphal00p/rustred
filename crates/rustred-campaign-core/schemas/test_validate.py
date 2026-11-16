#!/usr/bin/env python3
"""Mutation tests of validate.py (WP-0a: "the validator rejects at least 10 mutated ones"). Each case takes a valid
example or fixture, applies ONE named mutation and asserts that validate.py rejects it with an expected message.
Standard library only; run `python3 -I test_validate.py` (exit 0 = all pass). Also checks the canonical-JSON vectors."""

import copy
import json
import os
import shutil
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import validate as V  # noqa: E402


def ex(name):
    with open(os.path.join(HERE, "examples", name), "rb") as f:
        return V.parse_json_strict(f.read())


def ex_lines(name):
    with open(os.path.join(HERE, "examples", name), "rb") as f:
        return [V.parse_json_strict(l) for l in f.read().splitlines() if l.strip()]


def record(kind):
    for r in ex_lines("ledger-record.jsonl"):
        ev = r["event"]
        sub = ev["body"].get("ev")
        k = ev["kind"] + ("." + (sub["ev"] if isinstance(sub, dict) else sub) if sub is not None and ev["kind"] in ("item", "job", "node", "campaign") else "")
        if k == kind:
            return r
    raise KeyError(kind)


def doc_case(base, mutate, expect):
    def run():
        d = copy.deepcopy(base)
        mutate(d)
        assert not V.validate_document(base), "base not valid: %s" % V.validate_document(base)
        errs = V.validate_document(d)
        assert errs, "mutation accepted"
        assert any(expect in e for e in errs), "expected %r in %s" % (expect, errs)
    return run


def cdir_case(fixture, mutate, expect):
    def run():
        tmp = tempfile.mkdtemp(prefix="rr-validate-")
        try:
            d = os.path.join(tmp, fixture)
            shutil.copytree(os.path.join(HERE, "fixtures", fixture), d)
            base = V.validate_cdir(os.path.join(HERE, "fixtures", fixture), now="2026-11-14T11:59:00Z")
            assert base.exit_code == 0, "base fixture invalid: %s" % base.errors
            mutate(d)
            rep = V.validate_cdir(d, now="2026-11-14T11:59:00Z")
            assert rep.exit_code != 0, "mutation accepted"
            assert any(expect in e for e in rep.errors + rep.missing), "expected %r in %s" % (expect, rep.errors + rep.missing)
        finally:
            shutil.rmtree(tmp)
    return run


def edit_json(path, f):
    with open(path, "rb") as fh:
        d = V.parse_json_strict(fh.read())
    f(d)
    with open(path, "w") as fh:
        json.dump(d, fh, indent=2)
        fh.write("\n")


def edit_ledger_line(d, seg, n, f):
    p = os.path.join(d, "ledger", seg)
    lines = open(p, "rb").read().split(b"\n")
    lines[n] = f(lines[n])
    open(p, "wb").write(b"\n".join(lines))


def drop_ledger_line(d, seg, n):
    p = os.path.join(d, "ledger", seg)
    lines = open(p, "rb").read().split(b"\n")
    del lines[n]
    open(p, "wb").write(b"\n".join(lines))


def setp(path_keys, value):
    def m(d):
        x = d
        for k in path_keys[:-1]:
            x = x[k]
        x[path_keys[-1]] = value
    return m


def delp(path_keys):
    def m(d):
        x = d
        for k in path_keys[:-1]:
            x = x[k]
        del x[path_keys[-1]]
    return m


CASES = {
    "missing_required_field": doc_case(ex("campaign-spec.json"), delp(["nodes"]), "missing required field 'nodes'"),
    "wrong_type": doc_case(record("note"), setp(["seq"], "81"), "wrong type"),
    "unknown_major_version": doc_case(ex("campaign-spec.json"), setp(["schema"], "rustred.campaign.spec.v2"), "major version 2 not understood"),
    "unknown_schema": doc_case(ex("control.json"), setp(["schema"], "rustred.campaign.nonsense.v1"), "unknown schema"),
    "bad_digest_form": doc_case(record("item.produce"), setp(["prev"], "XYZ" + "0" * 61), "does not match"),
    "boxed_without_box_id": doc_case(ex("progress-node.json"), lambda d: d.update(
        {"closure_kind": "boxed", "phase": "closed", "target": {"target": "boxed", "box_id": "T162.box"}, "spec_box": None}),
        "BOXED without a box id"),
    "full_on_open_phase": doc_case(ex("progress-node.json"), setp(["closure_kind"], "full"), "FULL needs target"),
    "negative_count": doc_case(ex("progress-campaign.json"), setp(["items", "total"], -1), "below the minimum"),
    "sum_of_phases": doc_case(ex("progress-campaign.json"), setp(["items", "total"], 999), "sum of item phases"),
    "unknown_ledger_kind": doc_case(record("note"), setp(["event", "kind"], "rule_teleported"), "is not one of"),
    "closed_object_unknown_field": doc_case(ex("child-job-spec.json"), setp(["budget"], {}), "unknown field 'budget'"),
    "elided_accept": doc_case(record("item.accept"), setp(["event", "body", "ev", "elided"], True), "elided witness loci"),
    "fail_without_disproof": doc_case(ex("accept.json"), lambda d: (d["verdict"].update({"normalised": "FAIL", "disproof": None}), d.update({"level_after": "A0", "evidence_level": None})), "FAIL only with a stored disproof"),
    "level_word_mismatch": doc_case(ex("accept.json"), setp(["evidence_level"], "geometry_accepted"), "maps to ledger word"),
    "verify_prime_in_lift_set": doc_case(ex("accept.json"), lambda d: d["rule"]["producer"].update({"lift_primes": [8070450532247928833]}), "lift-prime set"),
    "knockout_as_input": doc_case(ex("campaign-spec.json"), lambda d: d["nodes"][0].update({"knockout": True}), "forbidden form"),
    "publish_forbidden": doc_case(ex("campaign-spec.json"), setp(["outputs", "publish"], True), "forbidden form"),
    "elision_allowed_in_spec": doc_case(ex("campaign-spec.json"), setp(["output", "elide_witness_loci"], True), "must be false"),
    "bad_rung": doc_case(record("item_created"), setp(["event", "body", "rung"], "X1"), "does not match"),
    "outcome_time_decided": doc_case(ex("outcome.json"), setp(["job", "time_decided"], True), "time_decided iff"),
    "outcome_status_vs_exit": doc_case(ex("outcome.json"), setp(["job", "status"], "complete"), "must be \"incomplete\""),
    "events_utc_on_phase": doc_case(ex_lines("events-line.jsonl")[1], setp(["utc"], "2026-11-13T23:00:00.000Z"), "forbidden form"),
    "control_bad_verb": doc_case(ex("control.json"), setp(["verb"], "reboot"), "is not one of"),
    "retire_replaced_without_successor": doc_case(record("item.retire"), lambda d: d["event"]["body"]["ev"].update(
        {"proof": {"kind": "replaced_by", "proof": d["event"]["body"]["ev"]["proof"].get("proof") or d["event"]["body"]["ev"]["proof"].get("closure")}, "superseded_by": None}),
        "replaced_by needs superseded_by"),
    "unknown_producer_above_a0": doc_case(ex_lines("accept-bank-import.jsonl")[0], lambda d: d.update({"level_after": "A1", "evidence_level": "geometry_accepted"}), "producer digest unknown"),
    "unknown_producer_without_class": doc_case(ex_lines("accept-bank-import.jsonl")[0], delp(["rule", "producer", "provenance"]), "missing required field 'provenance'"),
    "evidence_unknown_producer_without_class": doc_case(record("evidence_imported"), lambda d: d["event"]["body"].update({"producer_blake3": None, "provenance_class": None}), "is not one of"),
    "duplicate_key": lambda: _dup(),
    # campaign-directory mutations
    "broken_hash_chain": cdir_case("cdir-stale", lambda d: edit_ledger_line(d, "segment-000001.jsonl", 3, lambda l: l.replace(b'"bytes":', b'"bytes": ')), "broken hash chain"),
    "dense_seq_gap": cdir_case("cdir-stale", lambda d: drop_ledger_line(d, "segment-000001.jsonl", 4), "seq must be dense"),
    "sealed_segment_tampered": cdir_case("cdir-mid", lambda d: edit_ledger_line(d, "segment-000001.jsonl", 2, lambda l: l.replace(b"9000000", b"9000001")), "BLAKE3 differs from the manifest"),
    "referenced_file_bad_digest": cdir_case("cdir-mid", lambda d: edit_json(os.path.join(d, "accepted", "T162", "0001.json"), setp(["utc"], "2026-11-20T08:14:56Z")), "bad digest"),
    "referenced_file_missing": cdir_case("cdir-stale", lambda d: os.remove(os.path.join(d, "accepted", "T162", "0000.json")), "is missing"),
    "spec_digest_differs": cdir_case("cdir-empty", lambda d: edit_json(os.path.join(d, "spec.json"), setp(["description"], "edited")), "spec.json differs"),
    "progress_ahead_of_ledger": cdir_case("cdir-stale", lambda d: edit_json(os.path.join(d, "progress", "campaign.json"), setp(["ledger", "seq"], 10 ** 6)), "progress ahead of the ledger"),
    "progress_count_disagrees": cdir_case("cdir-finished", lambda d: edit_json(os.path.join(d, "progress", "campaign.json"), lambda c: (c["rules"].update({"accepted": c["rules"]["accepted"] + 1}))), "rules.accepted"),
    "node_file_missing": cdir_case("cdir-mid", lambda d: os.remove(os.path.join(d, "progress", "nodes", "T178.json")), "node file nodes/T178.json missing"),
}


def _dup():
    try:
        V.parse_json_strict('{"schema":"rustred.campaign.control.v1","schema":"x"}')
    except V.DuplicateKey as e:
        assert "duplicate JSON key" in str(e)
        return
    raise AssertionError("duplicate key accepted")


def canon_vectors():
    with open(os.path.join(HERE, "canon-vectors.json"), "rb") as f:
        vec = V.parse_json_strict(f.read())
    n = 0
    for v in vec["vectors"]:
        got = V.canonical_json(V.parse_json_strict(v["input"]), allow_floats=v.get("floats", False))
        assert got == v["document_rule"], "vector %s: %r != %r" % (v["name"], got, v["document_rule"])
        n += 1
    return n


def main():
    failed = 0
    for name, run in CASES.items():
        try:
            run()
            print("ok      %s" % name)
        except Exception as e:  # noqa: BLE001
            failed += 1
            print("FAILED  %s: %s" % (name, e))
    try:
        n = canon_vectors()
        print("ok      canonical vectors (%d)" % n)
    except Exception as e:  # noqa: BLE001
        failed += 1
        print("FAILED  canonical vectors: %s" % e)
    print("%d mutation cases, %d failed" % (len(CASES), failed))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
