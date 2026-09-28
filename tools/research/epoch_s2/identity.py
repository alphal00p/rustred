#!/usr/bin/env python
"""epoch-s2 lane: byte identity of two epoch runs (e.g. W6 vs W24).

Compares the S2 export sections byte for byte (domains, nodes, ledger6, edges,
anchors), the records (JSON lines, timing field `seconds` removed), the
manifest's digests and positions, the lockstep size B (`extra.schedule.b`: identity is
claimed only for equal B, the key is (request, B)), and the result.json records and walk
counters (worker-dependent and timing fields excluded).
usage: identity.py RUN_A RUN_B [--out FILE]   (run directories holding result.json + checkpoint/)
"""
import hashlib, json, sys
from pathlib import Path

TIMING = {"seconds"}
SKIP_TOP = {"workers", "worker_allocation", "parallel", "prepared_seconds", "traversal_seconds",
            "elapsed_seconds", "traversal_timing_boundary", "checkpoint", "requested_inspection_workers",
            "timing_scope", "domains"}


def strip(value):
    if isinstance(value, dict):
        return {k: strip(v) for k, v in value.items() if k not in TIMING and not k.endswith("_seconds")}
    if isinstance(value, list):
        return [strip(v) for v in value]
    return value


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    args = [arg for arg in sys.argv[1:] if not arg.startswith("--out=")]
    out = next((arg.split("=", 1)[1] for arg in sys.argv[1:] if arg.startswith("--out=")), None)
    if "--out" in args:
        at = args.index("--out")
        out = args[at + 1]
        del args[at:at + 2]
    a, b = Path(args[0]), Path(args[1])
    report = {"a": str(a), "b": str(b), "differences": []}
    ma = json.loads((a / "checkpoint/epoch-export.json").read_text())
    mb = json.loads((b / "checkpoint/epoch-export.json").read_text())
    ba = ((ma.get("extra") or {}).get("schedule") or {}).get("b")
    bb = ((mb.get("extra") or {}).get("schedule") or {}).get("b")
    report["lockstep_b"] = [ba, bb]
    if ba is None or ba != bb:
        report["differences"].append(f"lockstep B differs or is missing: {ba!r} != {bb!r}")
    for key in ("domains", "nodes", "ledger6", "edges", "anchors"):
        fa, fb = a / "checkpoint" / ma["files"][key]["file"], b / "checkpoint" / mb["files"][key]["file"]
        ha, hb = sha(fa), sha(fb)
        report[f"{key}_sha256"] = ha
        if ha != hb:
            report["differences"].append(f"section {key}: {ha} != {hb}")
    for key in ("records_digest", "edge_digest", "k", "watermark", "p0", "ledger6_counts", "edges",
                "edge_runs", "self_edges", "counters", "inputs", "input_frontiers", "request"):
        if ma.get(key) != mb.get(key):
            report["differences"].append(f"manifest {key}: {ma.get(key)!r} != {mb.get(key)!r}")
    ra = [strip(json.loads(l)) for f in ma["records"] for l in (a / "checkpoint" / f["file"]).read_text().splitlines()]
    rb = [strip(json.loads(l)) for f in mb["records"] for l in (b / "checkpoint" / f["file"]).read_text().splitlines()]
    report["records"] = len(ra)
    if ra != rb:
        diff = next((i for i, (x, y) in enumerate(zip(ra, rb)) if x != y), min(len(ra), len(rb)))
        report["differences"].append(f"records differ (counts {len(ra)}/{len(rb)}; first index {diff})")
    da = json.loads((a / "result.json").read_text())
    db = json.loads((b / "result.json").read_text())
    report["workers"] = [da.get("workers"), db.get("workers")]
    if strip(da.get("domains")) != strip(db.get("domains")):
        report["differences"].append("result.json domains differ (seconds removed)")
    for key in sorted(set(da) | set(db)):
        if key in SKIP_TOP:
            continue
        va, vb = strip(da.get(key)), strip(db.get(key))
        if key == "epoch":
            for sub in ("store",):
                (va or {}).pop(sub, None); (vb or {}).pop(sub, None)
        if va != vb:
            report["differences"].append(f"result {key} differs")
    report["identical"] = not report["differences"]
    text = json.dumps(report, indent=1)
    if out:
        Path(out).write_text(text + "\n")
    print(text)
    return 0 if report["identical"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
