#!/usr/bin/env python
"""Summarise the C-4L fp-vs-reference matrix written by c4l_matrix.sh.

For every run: metrics.json (run_control), the coordinator_duty block and
admission_preparation counters from the result.json tail, and the audit verdict
(audit.json, written by `--audit`).  For every Ordered pair (same family and
round) the strict record comparison verdict (strict-*.json), for Ready pairs
the multiset verdict.  Timing: traversal seconds per family, fp/ref ratio of the
round means, and the within-binary repeat spread.

Usage: c4l_summary.py C4L_DIR [--compare] [--audit]
  --compare / --audit run examples/python/compare_walk_records.py and
  audit_owner_domain_walk.py first (pinned by the caller).
"""
import argparse
import json
import statistics
import subprocess
import sys
from pathlib import Path

ROOT = Path("/common/dev/rustred/.claude/worktrees/fable51-fp")
CMP = ROOT / "examples/python/compare_walk_records.py"
AUDIT = ROOT / "examples/python/audit_owner_domain_walk.py"
FAMILIES = ("fg", "bmw", "h", "x")
DUTY = ("ordered_commit_seconds", "preparation_seconds", "dispatch_seconds", "poll_seconds",
        "publication_seconds", "progress_json_seconds", "closure_refresh_seconds", "checkpoint_seconds",
        "ready_service_seconds", "wait_seconds")


def tail_object(path, key, size=4_000_000):
    with open(path, "rb") as f:
        f.seek(0, 2)
        n = f.tell()
        f.seek(max(0, n - size))
        blob = f.read().decode("utf-8", "replace")
    i = blob.rfind(f'"{key}":')
    if i < 0:
        return None
    j = blob.index("{", i)
    obj, _ = json.JSONDecoder().raw_decode(blob[j:])
    return obj


def run_info(d):
    info = {"dir": str(d)}
    m = d / "metrics.json"
    if m.exists():
        info.update(json.load(open(m)))
    res = d / "result.json"
    if res.exists():
        duty = tail_object(res, "coordinator_duty")
        if duty:
            wall = duty.get("coordinator_elapsed_seconds") or 0
            info["coordinator_wall_seconds"] = wall
            info["duty_shares"] = {k.replace("_seconds", ""): duty.get(k, 0) / wall for k in DUTY} if wall else None
        prep = tail_object(res, "admission_preparation")
        if prep and prep.get("prepared_batch_records"):
            info["commit_us_per_prepared_record"] = 1e6 * prep["ordered_commit_wall_seconds"] / prep["prepared_batch_records"]
    a = d / "audit.json"
    if a.exists():
        doc = json.load(open(a))
        info["audit"] = doc.get("audit")
        info["audit_violations"] = len(doc.get("violations") or [])
    return info


def main():
    p = argparse.ArgumentParser()
    p.add_argument("c4l", type=Path)
    p.add_argument("--compare", action="store_true")
    p.add_argument("--audit", action="store_true")
    args = p.parse_args()
    c4l = args.c4l
    tags = sorted(x.name for x in c4l.iterdir() if x.is_dir() and not x.name.startswith("compare"))
    cmpdir = c4l / "compare"
    cmpdir.mkdir(exist_ok=True)
    if args.audit:
        procs = []
        for tag in tags:
            for fam in FAMILIES:
                d = c4l / tag / fam
                if (d / "result.json").exists() and not (d / "audit.json").exists():
                    procs.append(subprocess.Popen([sys.executable, str(AUDIT), str(d)],
                                                  stdout=open(d / "audit.stdout", "w"), stderr=open(d / "audit.stderr", "w")))
                    if len(procs) >= 6:
                        procs.pop(0).wait()
        for pr in procs:
            pr.wait()
    if args.compare:
        jobs = []
        for fam in FAMILIES:
            for r in ("r1", "r2"):
                a, b = c4l / f"ref-ord-{r}" / fam / "result.json", c4l / f"fp-ord-{r}" / fam / "result.json"
                if a.exists() and b.exists():
                    jobs.append(("strict", f"strict-{fam}-{r}", a, b, []))
            a, b = c4l / "ref-rdy" / fam / "result.json", c4l / "fp-rdy" / fam / "result.json"
            if a.exists() and b.exists():
                jobs.append(("multiset", f"multiset-{fam}", a, b, ["--native-tolerance", "0.02"]))
        procs = []
        for mode, name, a, b, extra in jobs:
            out = cmpdir / f"{name}.json"
            if out.exists():
                continue
            procs.append(subprocess.Popen([sys.executable, str(CMP), "--mode", mode, str(a), str(b), *extra,
                                           "--output", str(out)], stdout=open(cmpdir / f"{name}.stdout", "w"),
                                          stderr=open(cmpdir / f"{name}.stderr", "w")))
            if len(procs) >= 6:
                procs.pop(0).wait()
        for pr in procs:
            pr.wait()
    runs = {}
    for tag in tags:
        for fam in FAMILIES:
            d = c4l / tag / fam
            if d.exists():
                runs[f"{tag}/{fam}"] = run_info(d)
    comparisons = {}
    for f in sorted(cmpdir.glob("*.json")):
        doc = json.load(open(f))
        comparisons[f.stem] = {k: doc.get(k) for k in ("verdict", "status", "differing_records", "differences",
                                                          "records_a", "records_b", "native_inspections")
                               if k in doc} or {"keys": list(doc)[:12]}
    timing = {}
    for fam in FAMILIES:
        def trav(tag):
            x = runs.get(f"{tag}/{fam}", {}).get("traversal_seconds")
            return float(x) if x is not None else None
        fp = [t for t in (trav("fp-ord-r1"), trav("fp-ord-r2")) if t]
        ref = [t for t in (trav("ref-ord-r1"), trav("ref-ord-r2")) if t]
        entry = {"fp_ordered": fp, "ref_ordered": ref, "fp_ready": trav("fp-rdy"), "ref_ready": trav("ref-rdy")}
        if fp and ref:
            entry["fp_over_ref_ordered_mean"] = statistics.fmean(fp) / statistics.fmean(ref)
            entry["repeat_spread_fp"] = (max(fp) - min(fp)) / statistics.fmean(fp) if len(fp) == 2 else None
            entry["repeat_spread_ref"] = (max(ref) - min(ref)) / statistics.fmean(ref) if len(ref) == 2 else None
        if entry["fp_ready"] and entry["ref_ready"]:
            entry["fp_over_ref_ready"] = entry["fp_ready"] / entry["ref_ready"]
        timing[fam] = entry
    doc = {"runs": runs, "comparisons": comparisons, "timing": timing}
    (c4l / "summary.json").write_text(json.dumps(doc, indent=1))
    print(json.dumps({"timing": timing, "comparisons": comparisons}, indent=1))


if __name__ == "__main__":
    main()
