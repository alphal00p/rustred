#!/usr/bin/env python3
"""Lane allnodes: ONE import spec + ONE box document per integer-index node, from the ledgers (inspect-only helper; the
sweep's computations are the Rust library's). Every parameter comes from this template; nothing per node.

Inputs: nodes_all.jsonl (resolve_nodes.py: node, table, ledger sha256), scan.jsonl (scan_table.sh: computed sha256, record
kinds), BOXDEF.v3.json. Output: <out>/<node>/{spec.json, box.json} and <out>/nodes.jsonl (smallest table first; the
boxed nodes of other lanes last).

Box (BOXDEF.v3 dataset A_integer: box.formula_extended at the node's own loop count L and top support E, never restricted
from a parent): clean top support -> per_loop_extended[L].clean_supports (L = 5: 0 <= D <= 4, r <= E - 7 at EVERY D incl.
D = 0, line power <= 5: the ENLARGED R* box); carrier / factorized top support -> per_loop_extended[L] (any support).
The base (v1 core) box is counted for information only (D = 0: r <= E - 2L + 2; 1 <= D <= 4: r <= E - 2L + 3; power 5).
Support class: BOXDEF.v3 sector_classes (port of the helper's check_band_v3.sector_class, exact rational ranks).
Terminals: a table with a composite_terminal_ledger -> `ledger`; residual records only -> `ledger_and_residuals`;
neither -> `ledger` (declares none).
Usage: gen_specs.py nodes_all.jsonl scan.jsonl BOXDEF.v3.json OUTDIR LAST_NODES(comma list)
"""
import hashlib
import itertools
import json
import os
import re
import sys
from fractions import Fraction

nodes_f, scan_f, boxdef_f, out, last = sys.argv[1:6]
last = [x for x in last.split(",") if x]
scan = {json.loads(l)["table"]: json.loads(l) for l in open(scan_f)}
bd = json.load(open(boxdef_f))
ext = bd["box"]["per_loop_extended"]
SPEC_SCHEMA = "rustred.campaign.import-spec.v1"


def rank(vs):
    m = [[Fraction(x) for x in v] for v in vs if any(v)]
    r, col = 0, 0
    ncol = len(m[0]) if m else 0
    while r < len(m) and col < ncol:
        p = next((i for i in range(r, len(m)) if m[i][col] != 0), None)
        if p is None:
            col += 1
            continue
        m[r], m[p] = m[p], m[r]
        for i in range(len(m)):
            if i != r and m[i][col] != 0:
                f = m[i][col] / m[r][col]
                m[i] = [a - f * b for a, b in zip(m[i], m[r])]
        r += 1
        col += 1
    return r


def sector_class(L, S):
    n = len(S)
    Q = tuple([0] * L + [1])
    loop = [v[:L] for v in S]
    full = rank(loop)
    seps = []
    for size in range(1, n):
        for A in itertools.combinations(range(n), size):
            B = [i for i in range(n) if i not in A]
            if rank([loop[i] for i in A]) + rank([loop[i] for i in B]) == full:
                seps.append(set(A))
    comps = [A for A in seps if not any(B < A for B in seps)]
    if len(comps) >= 2:
        return "factorized"
    for size in range(2, n):
        for Bi in itertools.combinations(range(n), size):
            rest = [i for i in range(n) if i not in Bi]
            VB = [S[i] for i in Bi]
            VR = [S[i] for i in rest] + [Q]
            rB, rR, rU = rank(VB), rank(VR), rank(VB + VR)
            if rB < 2 or rB + rR - rU != 1:
                continue
            for e in rest:
                if rank(VB + [S[e]]) == rB and rank(VR + [S[e]]) == rR:
                    return "carrier"
    return "clean"


def momenta(src, loops):
    names = loops + ["q"]
    props = re.findall(r"prop\(\s*(\w+)\s*,\s*\((.*?)\)\^2\s*,\s*(\d+)\s*\)", src)
    vecs = []
    for _, expr, _flag in props:
        v = [0] * len(names)
        for sgn, coef, var in re.findall(r"([+-]?)\s*(\d*)\s*\*?\s*([A-Za-z]\w*)", expr.replace(" ", "")):
            c = int(coef) if coef else 1
            v[names.index(var)] += -c if sgn == "-" else c
        vecs.append(tuple(v))
    return vecs


def count_box(E, N, layers, cap):
    # keys: E lines with power 1..cap and dots D, N - E other slots <= 0 with rank r
    def ways_dots(D):
        # compositions of D into E parts each in 0..cap-1
        dp = [1] + [0] * D
        for _ in range(E):
            nd = [0] * (D + 1)
            for s in range(D + 1):
                if dp[s]:
                    for a in range(0, min(cap - 1, D - s) + 1):
                        nd[s + a] += dp[s]
            dp = nd
        return dp[D]

    def ways_rank_le(R):
        from math import comb
        return sum(comb(r + N - E - 1, N - E - 1) if N - E > 0 else (1 if r == 0 else 0) for r in range(R + 1))

    return sum(ways_dots(D) * ways_rank_le(R) for D, R in layers if R >= 0)


os.makedirs(out, exist_ok=True)
rows = []
for l in open(nodes_f):
    r = json.loads(l)
    node = r["node"]
    if not r["table"]:
        rows.append({"node": node, "cls": r["cls"], "run": False, "why": r["why"]})
        continue
    t = r["table"]
    sc = scan[t]
    with open(t, "rb") as f:
        head = json.loads(f.readline())
    mask = head["mask"]
    loops = re.search(r"loops\(([^)]*)\)", head["family_source"]).group(1).replace(" ", "").split(",")
    L = len(loops)
    assert L == head["loops"], node
    vecs = momenta(head["family_source"], loops)
    assert len(vecs) == len(mask), (node, len(vecs), len(mask))
    S = [vecs[i] for i, c in enumerate(mask) if c == "1"]
    E, N = len(S), len(mask)
    klass = sector_class(L, S)
    row = ext[str(L)]["clean_supports"] if klass == "clean" else ext[str(L)]
    dmax, cap = row["D_max"], row["line_power_cap"]
    (rng, rexpr), = row["r_max_by_D"].items()
    rmax = E + int(rexpr[1:] or 0) if rexpr != "E" else E
    layers = [[d, rmax] for d in range(dmax + 1)] if rmax >= 0 else []
    base_layers = [[0, E - 2 * L + 2]] + [[d, E - 2 * L + 3] for d in range(1, 5)]
    kinds = sc["kinds"]
    tt = ("ledger" if kinds.get("composite_terminal_ledger") else
          "ledger_and_residuals" if kinds.get("residual") else "ledger")
    sha = r["sha256"] or sc["sha256"]
    nd = os.path.join(out, node)
    os.makedirs(nd, exist_ok=True)
    box = {"schema": "rustred.campaign.box.v1", "id": f"{node}-boxdef-v3-A", "name": f"{node} BOXDEF.v3 A ({klass})",
           "measure": "dots-rank-v1", "chart": mask, "layers": layers, "cap": cap,
           "origin": f"BOXDEF.v3 sha256 {hashlib.sha256(open(boxdef_f, 'rb').read()).hexdigest()} dataset A_integer "
                     f"formula_extended, L={L}, E={E}, class {klass}"}
    bp = os.path.join(nd, "box.json")
    with open(bp, "w") as f:
        json.dump(box, f)
    spec = {"schema": SPEC_SCHEMA, "node": node, "frame_from": "table",
            "start": {"path": bp, "format": "box", "sha256": hashlib.sha256(open(bp, "rb").read()).hexdigest()},
            "sources": [{"id": "table", "kind": "engine_table", "path": t, "sha256": sha, "table_terminals": tt}]}
    with open(os.path.join(nd, "spec.json"), "w") as f:
        json.dump(spec, f, indent=1)
    rows.append({"node": node, "cls": r["cls"], "run": True, "table": t, "bytes": sc["bytes"], "sha256": sha,
                 "sha_from": r.get("sha_from") or "computed_unpinned", "why": r["why"], "family": head["family"],
                 "L": L, "E": E, "arity": N, "support_class": klass, "layers": layers, "cap": cap,
                 "box_keys_expected": count_box(E, N, layers, cap),
                 "base_box_keys": count_box(E, N, base_layers, 5),
                 "table_terminals": tt, "kinds": kinds, "residual_masters": r["residual_masters"],
                 "state": r["state"], "spec": os.path.join(nd, "spec.json")})
run = sorted([x for x in rows if x["run"] and x["node"] not in last], key=lambda x: x["bytes"])
tail = [x for n in last for x in rows if x["run"] and x["node"] == n]
with open(os.path.join(out, "nodes.jsonl"), "w") as f:
    for x in run + tail:
        f.write(json.dumps(x) + "\n")
with open(os.path.join(out, "not_run.jsonl"), "w") as f:
    for x in rows:
        if not x["run"]:
            f.write(json.dumps(x) + "\n")
print(len(run), len(tail), len([x for x in rows if not x["run"]]))
