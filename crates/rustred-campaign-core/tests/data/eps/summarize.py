# Per-node table of the eps campaign sweep (inspect-only): Tier K (keys-only superset) and final Tier C verdicts,
# terminals vs P5L_STATUS residual masters; nodes that do NOT reproduce their certificate first.
import json, sys
W = sys.argv[1]
nodes = {json.loads(l)["node"]: json.loads(l) for l in open(W + "/nodes.jsonl")}
res = {}
for l in open(W + "/results.jsonl"):
    x = json.loads(l); res.setdefault(x["node"], []).append(x)
fin = {}
for l in open(W + "/final.jsonl"):
    if l.startswith("{"):
        x = json.loads(l); fin[x["node"]] = x
bad, ok, konly = [], [], []
for n, rs in res.items():
    if n not in fin: continue
    k = next((r for r in rs if r["mode"] == "k"), {})
    cs = [r for r in rs if r["mode"] == "c"]
    c = cs[-1] if cs else {}
    rm = nodes[n]["residual_masters"]
    t = c.get("terminals")
    box = k.get("box") or f"probe(weight<={k.get('layer')})"
    row = (box, n, nodes[n]["cls"], k.get("loaded"), k.get("verdict"), k.get("open"), k.get("terminals"), rm, c.get("verdict"),
           c.get("open"), c.get("open_by_status"), c.get("keys"), c.get("keys_eps_nonpos"), fin[n]["stop"], fin[n]["requested_keys"],
           round(sum(r["seconds"] for r in rs), 1), max(r["peak_rss_kb"] for r in rs) // 1024, c.get("error") or k.get("error"))
    kt = k.get("terminals")
    good = c.get("verdict") == "closed" and (rm is None or t == rm) and k.get("open") == 0
    tier_k_only = fin[n]["stop"] == "over_budget" and k.get("open") == 0 and (rm is None or kt == rm)
    (ok if good else konly if tier_k_only else bad).append(row)
hdr = "box node class loaded K_verdict K_open K_term P5L_masters C_verdict C_open C_open_by C_keys C_eps<=0 stop req_keys sec peakMB error"
print(f"done {len(ok)+len(bad)+len(konly)} / {len(nodes)}: reproduce (Tier C closed) {len(ok)}, Tier K only (Tier C requests over budget) {len(konly)}, NOT {len(bad)}  (verdicts on the box named per line; "
      "a probe box is the lane's choice, NOT the campaign box)")
print("eps nodes WITHOUT a table path, certified in P5L_STATUS by the shipped 1-4L library (artefacts/m3/owners-1-4, Forcer topologies, generated all-rank, validated vs Forcer; outside the table campaign): "
      "T001 T006 T007 T014 T029 T036 T042 -> 137 by tables + 7 by the library = 144 eps nodes")
print(hdr)
for r in bad: print("NOT", *r)
for r in konly: print("K-only", *r)
for r in ok: print("ok ", *r)
