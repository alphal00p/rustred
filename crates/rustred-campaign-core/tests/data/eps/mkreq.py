# Build rr_proof instantiate requests (form A, with byte offsets: eps tables restart `index` per chart) for every rule
# record of a table and every key of a small box; inspect-only helper (reference data), not campaign code.
import json, sys, itertools
table, out, lo_eps, hi_eps, line_max, num_min = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5]), int(sys.argv[6])
recs = []
off = 0
with open(table, 'rb') as f:
    for raw in f:
        v = json.loads(raw)
        if v.get('type') == 'header':
            h = v
        if v.get('type') == 'rule':
            recs.append((off, v['chart'], v['index']))
        off += len(raw)
mask = h['mask']; eps = [i for i, s in enumerate(h['power_shifts']) if s != '0']
ranges = []
for i, m in enumerate(mask):
    if i in eps: ranges.append(range(lo_eps, hi_eps + 1))
    elif m == '1': ranges.append(range(1, line_max + 1))
    else: ranges.append(range(num_min, 1))
keys = [list(k) for k in itertools.product(*ranges)]
# one request file PER CHART: rr_proof instantiate groups requests by (table, rule index), and an eps table restarts
# `index` in every chart (two records of different charts with one index would be merged into one group)
charts = sorted(set(c for (_, c, _) in recs))
for c in charts:
    with open(out + "." + c, 'w') as o:
        for (off, chart, idx) in recs:
            if chart == c:
                o.write(json.dumps({"table": table, "offset": off, "rule": idx, "keys": keys}) + "\n")
json.dump({"charts": charts, "records": [[o_, c, i] for (o_, c, i) in recs], "keys": len(keys)}, open(out + ".meta.json", "w"))
print(len(recs), "records", len(keys), "keys")
