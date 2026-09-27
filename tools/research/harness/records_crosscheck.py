#!/usr/bin/env python3
"""Cross-check harness re-inspections of already inspected natives against
their persisted CP5 records: native `stats` JSON equality, `accepted_events`
against the stream's callback count, and the frontier count.

  records_crosscheck.py CHECKPOINT_DIR RUN_DIR [OUT_JSON]

RUN_DIR is a harness run over an `inspected`/`inspected-sample` fixture. The
records-*.jsonl segments are pre-filtered with grep -F on '"id":<id>,' so only
candidate lines are parsed. Timing fields ("seconds") are ignored.
"""
import glob
import json
import os
import subprocess
import sys
import tempfile


def main():
    ckpt, run = sys.argv[1], sys.argv[2]
    out = sys.argv[3] if len(sys.argv) > 3 else None
    natives = {}
    for line in open(os.path.join(run, "natives.jsonl")):
        r = json.loads(line)
        natives.setdefault(r["id"], r)
    with tempfile.NamedTemporaryFile("w", delete=False) as pat:
        for i in natives:
            pat.write('"id":%d,\n' % i)
        patterns = pat.name
    records = {}
    for seg in sorted(glob.glob(os.path.join(ckpt, "records-*.jsonl"))):
        proc = subprocess.run(["grep", "-F", "-f", patterns, seg], capture_output=True, text=True)
        for line in proc.stdout.splitlines():
            rec = json.loads(line)
            if rec.get("id") in natives and rec.get("record_kind") == "native_inspection":
                records[rec["id"]] = rec
    os.unlink(patterns)
    result = {"harness_natives": len(natives), "records_found": len(records),
              "stats_equal": 0, "stats_differ": 0, "accepted_events_equal": 0,
              "accepted_events_differ": 0, "frontiers_equal": 0, "frontiers_differ": 0,
              "harness_errors": 0, "by_phase": {}, "examples": []}
    for i, r in natives.items():
        rec = records.get(i)
        if rec is None:
            continue
        phase = result["by_phase"].setdefault(r["phase"], {"compared": 0, "stats_equal": 0})
        phase["compared"] += 1
        if r["error_kind"] != "none":
            result["harness_errors"] += 1
        equal = r["stats"] == rec["stats"]
        result["stats_equal" if equal else "stats_differ"] += 1
        phase["stats_equal"] += int(equal)
        events = r["counts"]["weighted"]
        callbacks = sum(events.values())
        ok = rec.get("accepted_events") == callbacks
        result["accepted_events_equal" if ok else "accepted_events_differ"] += 1
        fr = len(rec.get("frontiers") or []) == r["counts"]["weighted"]["frontier"]
        result["frontiers_equal" if fr else "frontiers_differ"] += 1
        if (not equal or not ok or not fr) and len(result["examples"]) < 10:
            result["examples"].append({"id": i, "harness_stats": r["stats"], "record_stats": rec["stats"],
                                       "harness_callbacks": callbacks,
                                       "record_accepted_events": rec.get("accepted_events"),
                                       "record_frontiers": len(rec.get("frontiers") or [])})
    result["passed"] = (result["records_found"] == len(natives) and result["stats_differ"] == 0
                        and result["accepted_events_differ"] == 0 and result["frontiers_differ"] == 0)
    text = json.dumps(result, indent=1)
    if out:
        open(out, "w").write(text + "\n")
    print(text)


if __name__ == "__main__":
    main()
