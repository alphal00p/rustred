#!/usr/bin/env python3
"""Lane allnodes: establish the integer-index node list and each node's certified table from the ledgers (read-only).

Classification: P5L_STATUS.json nodes[].class; eps classes = names ending in "_eps"; everything else is integer-index.
Table precedence (as the ledger records the CURRENT certified table): close.table > replacement.table > imported.table >
<run_dir>/F.table.jsonl. sha256: the ledger's table_sha256 where recorded, else P5L_SIMPLIFY's `original.sha256` when its
path equals the chosen path, else none (computed and labelled `computed_unpinned`). Cross-check against P5L_SIMPLIFY's
original path is reported per node.
Usage: resolve_nodes.py P5L_STATUS.json P5L_SIMPLIFY.json out.jsonl
"""
import json
import os
import re
import sys

st = json.load(open(sys.argv[1]))
si = json.load(open(sys.argv[2]))
simp = {x["id"]: x for x in si["nodes"]}
out = open(sys.argv[3], "w")
cls_count = {}
for x in st["nodes"]:
    c = x["class"]
    cls_count[c] = cls_count.get(c, 0) + 1
    if c.endswith("_eps"):
        continue
    rec = {"node": x["id"], "cls": c, "lines": x["lines"], "state": x["state"],
           "residual_masters": x["residual_masters"], "rules_ledger": x["rules"], "knockout": x["knockout"]}
    path = sha = None
    why = None
    for k in ("close", "replacement", "imported"):
        v = x.get(k)
        if isinstance(v, dict) and v.get("table"):
            path, sha, why = v["table"], v.get("table_sha256"), f"P5L {k}.table"
            break
    if path is None and x.get("run_dir"):
        p = os.path.join(x["run_dir"], "F.table.jsonl")
        if os.path.exists(p):
            path, why = p, "P5L run_dir/F.table.jsonl"
    s = simp.get(x["id"])
    if s:
        o = s.get("original") or {}
        rec["simplify_original"] = o.get("path")
        rec["simplify_state"] = s.get("state")
        if path and o.get("path") == path and o.get("sha256"):
            if sha and sha != o["sha256"]:
                rec["sha_conflict"] = [sha, o["sha256"]]
            sha = sha or o["sha256"]
            rec["sha_from"] = rec.get("sha_from") or "simplify.original"
        rec["simplify_agrees"] = (o.get("path") == path)
    if sha and "sha_from" not in rec:
        rec["sha_from"] = "P5L status"
    m = re.findall(r"sha256 ([0-9a-f]{16})", x.get("note") or "")
    rec["note_sha_prefixes"] = sorted(set(m))
    rec.update({"table": path, "sha256": sha, "why": why,
                "bytes": os.path.getsize(path) if path and os.path.exists(path) else None})
    if x["class"] == "library" and path is None:
        rec["why"] = "no table path in the ledgers (shipped 1-4L library)"
    out.write(json.dumps(rec) + "\n")
print(json.dumps(cls_count))
