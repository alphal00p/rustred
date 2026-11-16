# Compact per-node table of an eps campaign pass (inspect-only): one line per finished node.
import json, sys
C = sys.argv[1]
nodes = {json.loads(l)["node"]: json.loads(l) for l in open(C + "/nodes.jsonl")}
res = {}
for l in open(C + "/results.jsonl"):
    x = json.loads(l); res.setdefault(x["node"], []).append(x)
fin = {json.loads(l)["node"]: json.loads(l) for l in open(C + "/final.jsonl") if l.startswith("{")}
print("node class box_keys D<=4 base_in_box K_open terminals/P5L C_verdict requested_keys sec peakMB reproduces")
n_ok = 0
for n in sorted(fin, key=lambda t: nodes[t]["bytes"]):
    rs = res.get(n, []); k = next((r for r in rs if r["mode"] == "k"), {}); cs = [r for r in rs if r["mode"] == "c"]
    c = cs[-1] if cs else {}
    rm = nodes[n]["residual_masters"]
    ok = c.get("verdict") == "closed" and k.get("open") == 0 and (rm is None or c.get("terminals") == rm)
    n_ok += ok
    print(n, nodes[n]["cls"], k.get("box_keys"), k.get("box_subset_keys"), k.get("base_key_in_box"), k.get("open"),
          f'{c.get("terminals")}/{rm}', c.get("verdict"), fin[n]["requested_keys"],
          round(sum(r["seconds"] for r in rs), 1), max((r["peak_rss_kb"] for r in rs), default=0) // 1024,
          "yes" if ok else "NO:" + fin[n]["stop"])
print(f"# {len(fin)} nodes done, reproduce {n_ok}")
