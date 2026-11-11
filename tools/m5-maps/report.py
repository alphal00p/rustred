"""Tables from routes_a.json files + run summaries.  usage: report.py RA1.json[,RA2.json...] RUN_DIR... > table.md"""
import sys, json, os, collections
merged = {}
for f in sys.argv[1].split(','):
    for x in json.load(open(f))['routes']:
        k = (x['presentation'], x['support'])
        if k not in merged:
            merged[k] = x
            continue
        y = merged[k]
        for fld in ('keys', 'ms', 'terms_current', 'terms_best_a'):
            for run, v in x[fld].items():
                y[fld][run] = y[fld].get(run, 0) + v
        for r_, (kk, tt) in x['by_rank'].items():
            a = y['by_rank'].setdefault(r_, [0, 0]); a[0] += kk; a[1] += tt
ra = list(merged.values())
runs = sys.argv[2:]
print('## Per cell: routed traffic and the (a) optimum\n')
print('| cell | keys popped | edges | step s | routed keys (share) | routed terms = edges (share) | routed step s (share) | terms/key | (a) terms | (a) gain |')
print('|---|---|---|---|---|---|---|---|---|---|')
tot_cur = tot_a = 0
for r in runs:
    name = os.path.basename(r.rstrip('/'))
    s = json.load(open(os.path.join(r, 'summary.json')))
    keys = sum(x['keys'].get(name, 0) for x in ra)
    cur = sum(x['terms_current'].get(name, 0) for x in ra)
    a = sum(x['terms_best_a'].get(name, 0) for x in ra)
    ms = sum(x['ms'].get(name, 0) for x in ra)
    tot_cur += cur; tot_a += a
    print(f"| {name} | {s['popped']:,} | {s['edges']:,} | {s['step_seconds']:.1f} | {keys:,} ({100*keys/s['popped']:.1f} %) | "
          f"{cur:,} ({100*cur/s['edges']:.1f} %) | {ms/1e3:.1f} ({100*ms/1e3/s['step_seconds']:.1f} %) | {cur/max(keys,1):.2f} | {a:,} | {100*(a-cur)/max(cur,1):+.1f} % |")
print(f'\nAll cells: routed terms {tot_cur:,} -> {tot_a:,} with (a) ({100*(tot_a-tot_cur)/max(tot_cur,1):+.1f} %).\n')

# per edge (source node -> owner node) aggregated over cells
agg = collections.defaultdict(lambda: collections.Counter())
for x in ra:
    k = (x['src_name'][:22], x['src_node'], x['owner_name'][:22], x['owner_node'])
    agg[k]['keys'] += sum(x['keys'].values())
    agg[k]['terms'] += sum(x['terms_current'].values())
    agg[k]['a'] += sum(x['terms_best_a'].values())
    agg[k]['ms'] += sum(x['ms'].values())
    agg[k]['routes'] += 1
    agg[k]['G'] = max(agg[k]['G'], x['group_order'])
print('## Worst 20 edge maps (source node -> owner node, all sectors and cells summed), by routed terms\n')
print('| # | source presentation (node) | owner presentation (node) | sectors | |G| | keys | terms | terms/key | step s | (a) terms | (a) gain |')
print('|---|---|---|---|---|---|---|---|---|---|---|')
for i, (k, v) in enumerate(sorted(agg.items(), key=lambda kv: -kv[1]['terms'])[:20]):
    print(f"| {i+1} | {k[0]} ({k[1]}) | {k[2]} ({k[3]}) | {v['routes']} | {v['G']} | {v['keys']:,} | {v['terms']:,} | {v['terms']/max(v['keys'],1):.2f} | {v['ms']/1e3:.1f} | {v['a']:,} | {100*(v['a']-v['terms'])/max(v['terms'],1):+.1f} % |")

# terms per key by numerator rank, all routes
rk = collections.defaultdict(lambda: [0, 0])
for x in ra:
    for r_, (k, t) in x['by_rank'].items():
        rk[int(r_)][0] += k; rk[int(r_)][1] += t
print('\n## Terms per routed key by numerator rank (all cells)\n')
print('| rank | keys | terms | terms/key |')
print('|---|---|---|---|')
for r_ in sorted(rk):
    k, t = rk[r_]
    print(f'| {r_} | {k:,} | {t:,} | {t/max(k,1):.2f} |')
# distribution of per-sector terms/key
print('\n## Sectors by terms/key (all cells)\n')
buckets = collections.Counter()
for x in ra:
    k = sum(x['keys'].values()); t = sum(x['terms_current'].values())
    b = '1' if t <= k else '1-2' if t <= 2 * k else '2-5' if t <= 5 * k else '5-20' if t <= 20 * k else '>20'
    buckets[b] += t
for b in ['1', '1-2', '2-5', '5-20', '>20']:
    print(f'- terms/key {b}: {buckets[b]:,} terms')
n_impr = sum(1 for x in ra if sum(x['terms_best_a'].values()) < sum(x['terms_current'].values()))
print(f'\nSectors with a strictly better automorphism composition: {n_impr} of {len(ra)}; group orders: '
      f"{dict(collections.Counter(x['group_order'] for x in ra).most_common(8))}")
