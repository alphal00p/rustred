# lane arrivals: the CHAIN of one key arriving at a target (step 0 of the coordinator request 2026-11-16): from the key's
# arrival lines at the target back to a start-box key of some ancestor.
# Each hop: the arrival line with the least depth (its file = the parent P) -> its first sender (P's key pk, record, term);
# at P: pk is a box key (depth 0: the chain starts), or an arriving key (its own arrival lines in R/*/out/*-<P>.arrivals.jsonl:
# next hop), or reached by P's exact closure (a closure path from `compute --closure-only --trace-key`, file
# R/<P>/trace/<key>.json; missing -> printed as NEED_TRACE and exit 2: run trace_run.sh P key, then rerun).
# Usage: python3 -I trace_key.py <run dir R> <target> <label> '<key json>' <out json>
import json, os, subprocess, sys

R, target, label, key_s, out = sys.argv[1:6]
key = json.loads(key_s)
plan = {json.loads(l)['node']: json.loads(l) for l in open(R + '/plan.jsonl')}
labels = {}  # node -> the child label its incoming files use
for n in os.listdir(R):
    d = f'{R}/{n}/out'
    if os.path.isdir(d):
        for f in os.listdir(d):
            if f.endswith('.arrivals.jsonl'):
                c = f[len(n) + 1:-len('.arrivals.jsonl')]
                labels.setdefault(c.rstrip('if') if c in ('T162i', 'T178f') else c, c)

def lines_for(lab, k):
    pat = '"key":' + json.dumps(k, separators=(',', ':')) + ','
    res = []
    for n in sorted(os.listdir(R)):
        f = f'{R}/{n}/out/{n}-{lab}.arrivals.jsonl'
        if not os.path.isfile(f):
            continue
        r = subprocess.run(['grep', '-F', pat, f], capture_output=True, text=True)
        for l in r.stdout.splitlines():
            v = json.loads(l)
            if v.get('kind') == 'arrival' and v['key'] == k:
                res.append((n, v))
    return res

def box_key(node, k):
    spec = json.load(open(plan[node]['spec'])) if node in plan else None
    fr = json.load(open(spec['start']['path'])) if spec else None
    if not fr:
        return False
    chart = fr['chart']
    if any((x < 1) for x, c in zip(k, chart) if c == '1') or any(x > 0 for x, c in zip(k, chart) if c == '0'):
        return False
    D = sum(x - 1 for x, c in zip(k, chart) if c == '1')
    r = sum(-x for x, c in zip(k, chart) if c == '0')
    return any(D == d and r <= rr for d, rr in fr['layers'])

hops = []
node, lab, k = target, label, key
for _ in range(64):
    ls = lines_for(lab, k)
    if not ls:
        hops.append({'node': node, 'key': k, 'arrives': False})
        break
    ls.sort(key=lambda t: (t[1].get('depth', 99), t[0]))
    P, v = ls[0]
    f = v['first']
    hops.append({'node': node, 'key': k, 'depth': v.get('depth'), 'from_parent': P, 'parents': sorted({p for p, _ in ls}),
                 'senders_total': sum(x['senders'] for _, x in ls),
                 'sender': {'parent_key': f['parent_key'], 'applied': f['applied'], 'record_sha256': f['record_sha256'],
                            'term': f['term'], 'term_key': f['term_key'], 'by': f['by']}})
    pk = f['parent_key']
    if box_key(P, pk):
        hops.append({'node': P, 'key': pk, 'start': 'box (depth 0)'})
        break
    # an arriving key of P, or reached by P's closure
    plab = labels.get(P, P)
    if lines_for(plab, pk):
        node, lab, k = P, plab, pk
        continue
    tf = f'{R}/{P}/trace/' + '_'.join(str(x) for x in pk) + '.json'
    if not os.path.exists(tf):
        print('NEED_TRACE', P, json.dumps(pk, separators=(',', ':')))
        json.dump({'partial': True, 'hops': hops}, open(out, 'w'), indent=1)
        sys.exit(2)
    t = json.load(open(tf))
    path = t['path']
    hops.append({'node': P, 'key': pk, 'closure_path': path})
    s = path[-1]['key']
    if box_key(P, s):
        hops.append({'node': P, 'key': s, 'start': 'box (depth 0)'})
        break
    node, lab, k = P, plab, s
json.dump({'target': target, 'key': key, 'hops': hops, 'chain_nodes': [h['node'] for h in hops if 'from_parent' in h] + [hops[-1]['node']]},
          open(out, 'w'), indent=1)
print(json.dumps({'chain': [(h['node'], h.get('depth'), h.get('from_parent')) for h in hops], 'out': out}))
