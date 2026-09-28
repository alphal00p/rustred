#!/usr/bin/env python
"""epoch-s2 gate 3: natives, domains and edges of epoch vs legacy Ready (and Ordered) per control.
usage: compare.py EPOCH_LABEL READY_LABEL [ORDERED_DIR_ROOT]"""
import json, re, sys
from pathlib import Path
R = Path("/common/dev/rustred/TMP")

def head_metrics(path):
    """natives, domains, edges, aliases from result.json head/tail (files can be big)."""
    size = path.stat().st_size
    with open(path, "rb") as f:
        head = f.read(min(size, 8_000_000)).decode("utf-8", "replace")
        tail = ""
        if size > 8_000_000:
            f.seek(size - 400_000); tail = f.read().decode("utf-8", "replace")
    out = {}
    for key in ("native_processed_nodes", "completed_nodes", "scheduled_nodes", "dependency_edges",
                "delegated_publications", "status", "events", "successors"):
        for blob in (tail, head):
            m = re.search(r'"%s": ("?[A-Za-z0-9_.]+"?)' % key, blob)
            if m:
                out[key] = m.group(1).strip('"'); break
    return out

def row(d):
    p = d / "result.json"
    return head_metrics(p) if p.exists() else {}

def pct(a, b):
    try:
        return f"{100.0 * (float(a) - float(b)) / float(b):+.2f}%"
    except (TypeError, ValueError, ZeroDivisionError):
        return "n/a"

epoch, ready = sys.argv[1], sys.argv[2]
ordered = sys.argv[3] if len(sys.argv) > 3 else "fable51-controls/int-ref-g3"
table = []
for fam in ("fg", "bmw", "h", "x", "four-all", "four-all-p5", "five-finite"):
    e, r, o = row(R / "epoch-s2/runs" / epoch / fam), row(R / "epoch-s2/runs" / ready / fam), row(R / ordered / fam)
    if not e:
        continue
    entry = {"family": fam}
    for name, key in (("natives", "native_processed_nodes"), ("domains", "scheduled_nodes"), ("edges", "dependency_edges"),
                      ("aliases", "delegated_publications")):
        entry[name] = {"epoch": e.get(key), "ready": r.get(key), "ordered": o.get(key),
                       "epoch_vs_ready": pct(e.get(key), r.get(key)), "epoch_vs_ordered": pct(e.get(key), o.get(key))}
    entry["status"] = {"epoch": e.get("status"), "ready": r.get("status"), "ordered": o.get("status")}
    table.append(entry)
print(json.dumps(table, indent=1))
