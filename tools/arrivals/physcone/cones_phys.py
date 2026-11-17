# lane arrivals: PHYSICAL-chain ARRIVAL CONES (shipped tables, physical start boxes) by a fixpoint over the DAG (one topological pass), and the
# cross-check against the enumerated arrival files.
# Inputs (this directory): dag.json (nodes, specs, edges), excess/<node>.json (rr_arrivals cone-excess), edges.jsonl
# (rr_arrivals edge-cone), arrival_dr.jsonl (rr_arrivals arrival-dr). Output: cones.json + cones_crosscheck.json.
# Statement per node X: every key of X reachable from the boxed starts through any chain of hand-downs and X's own closure
# satisfies D + r <= S_X, where S_X = max(S_box(X), max over DAG parents P of S_P + delta(P -> X)),
# delta(P -> X) = the max hand-down excess of P's rule terms over the drop masks P -> X routes (cone-excess), provided
# (a) P's top terms never raise D + r (top_max <= 0) and (b) every routed sector of P -> X satisfies the edge bound (edge-cone).
import json, math, os, sys, collections

C = os.path.dirname(os.path.abspath(__file__))
WS = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev'
dag = json.load(open(C + '/dag.json'))
st = json.load(open(WS + '/P5L_STATUS.json'))
lines_n = {n['id']: n['lines'] for n in st['nodes']}
alias = {'T162i': 'T162', 'T178f': 'T178'}

def box_s(path):
    b = json.load(open(path))
    if b.get('measure') != 'dots-rank-v1':
        raise SystemExit(f'{path}: box measure {b.get("measure")}')
    return max(d + r for d, r in b['layers']), b

ex = {}
for o in dag['nodes']:
    p = f"{C}/excess/{o['node']}.json"
    if os.path.exists(p):
        ex[o['node']] = json.load(open(p))
edges = {}
for l in open(C + '/edges.jsonl'):
    e = json.loads(l)
    p, c = os.path.basename(e['edge'])[:-10].split('-')
    edges[(p, alias.get(c, c))] = e

nodes = sorted((o['node'] for o in dag['nodes']), key=lambda x: (-lines_n[x], x))
spec_box = {o['node']: o['box'] for o in dag['nodes']}
parents = collections.defaultdict(list)
for p, c, f in dag['edges']:
    parents[c].append(p)

S, why, delta, problems = {}, {}, {}, []
for x in nodes:
    sb, b = box_s(spec_box[x])
    s, w = sb, ['box', sb, b['layers']]
    for p in sorted(parents[x]):
        e = edges.get((p, x))
        if e is None:
            problems.append(f'{p} -> {x}: no edge-cone line'); continue
        if not e['bound_holds']:
            problems.append(f'{p} -> {x}: edge bound fails'); s = None; continue
        if p not in ex:
            problems.append(f'{p} -> {x}: no excess for {p}'); s = None; continue
        pm = ex[p]['excess']['per_mask']
        if ex[p]['excess']['top_max'] is None or ex[p]['excess']['top_max'] > 0:
            problems.append(f'{p}: top terms can raise D + r (top_max {ex[p]["excess"]["top_max"]})')
        vals = [pm[m][0] for m in e['masks'] if m in pm]
        if not vals:
            d = None  # no term of P has a routed mask: the route sends nothing
            delta[(p, x)] = {'delta': None, 'empty_by_tables': True, 'masks': e['masks']}
            continue
        if any(v is None for v in vals):
            delta[(p, x)] = {'delta': 'unbounded', 'masks': e['masks']}
            s = None; problems.append(f'{p} -> {x}: unbounded excess'); continue
        d = max(vals)
        delta[(p, x)] = {'delta': d, 'empty_by_tables': False, 'masks': e['masks'], 'terms': sum(pm[m][1] for m in e['masks'] if m in pm)}
        if S.get(p) is None or s is None:
            s = None; continue
        if S[p] + d > s:
            s, w = S[p] + d, ['edge', p, S[p], d]
    S[x], why[x] = s, w

out = {'schema': 'rustred.campaign.arrival-cones.v1', 'definition': {
    'D': 'sum over line slots of (k - 1)', 'r': 'sum over numerator slots of (-k)',
    'cone': 'top-sector keys of the node with D + r <= S (lane parametric Cone: constraints [(a, b)] with a_i = +1 on lines, -1 on numerators, b = S + #lines)',
    'recursion': 'S_X = max(S_box(X), max_P S_P + delta(P -> X)); delta = max hand-down excess of P over the routed masks of P -> X'},
    'nodes': {}, 'edges': {}, 'problems': problems}
for x in nodes:
    e = ex.get(x, {}).get('excess', {})
    lines_mask = ex.get(x, {}).get('frame')
    lm = [ch == '1' for ch in lines_mask] if lines_mask else None
    cone = None
    if S[x] is not None and lm:
        a = [1 if l else -1 for l in lm]
        cone = {'label': f'D + r <= {S[x]}', 'constraints': [[a, S[x] + sum(lm)]]}
    out['nodes'][x] = {'lines': lines_n[x], 'S': S[x], 'why': why[x], 'cone': cone, 'frame': lines_mask,
                       'top_max': e.get('top_max'), 'handdown_max': e.get('handdown_max'),
                       'max_lines_dropped': e.get('max_lines_dropped'),
                       'tables': ex.get(x, {}).get('tables')}
for (p, x), d in delta.items():
    out['edges'][f'{p}->{x}'] = dict(d, edge_file=edges[(p, x)]['edge'], edge_sha256=edges[(p, x)]['sha256'],
                                     lines_dropped=lines_n[p] - lines_n[x])

# REFINEMENT (optional): routes PROVED to carry nothing (no term of the parent has a routed mask feasible on its domains =
# empty_by_tables; or every such term is zero / excluded on its hand-down locus by VERIFIED hand-down proofs =
# prove_empty.json) are removed; S_live = the same recursion over the live routes; per node the live / dead routes.
pe = json.load(open(C + '/prove_empty.json')) if os.path.exists(C + '/prove_empty.json') else {'edges': {}}
dead = {}
for (p, x), d in delta.items():
    if d.get('empty_by_tables'):
        dead[(p, x)] = 'empty_by_tables'
    elif pe['edges'].get(f'{p}->{x}', {}).get('outcome') == 'proved_empty':
        dead[(p, x)] = 'proved_empty (verified hand-down proofs)'
S_live = {}
for x in nodes:
    sb, b = box_s(spec_box[x])
    s = sb
    for p in sorted(parents[x]):
        if (p, x) in dead or (p, x) not in delta:
            continue
        d = delta[(p, x)].get('delta')
        if d == 'unbounded' or S_live.get(p) is None:
            s = None
            continue
        if isinstance(d, int) and s is not None:
            s = max(s, S_live[p] + d)
    S_live[x] = s
for x in nodes:
    live = [{'parent': p, 'lines_dropped': lines_n[p] - lines_n[x], 'masks': delta[(p, x)]['masks'],
             'bound': (S_live[p] + delta[(p, x)]['delta']) if (S_live.get(p) is not None and isinstance(delta[(p, x)].get('delta'), int)) else None,
             'status': pe['edges'].get(f'{p}->{x}', {}).get('outcome', 'carries (1-2 lines) or not examined')}
            for p in sorted(parents[x]) if (p, x) in delta and (p, x) not in dead]
    dd = [{'parent': p, 'why': dead[(p, x)]} for p in sorted(parents[x]) if (p, x) in dead]
    lm = out['nodes'][x]['frame']
    cone_live = None
    if S_live[x] is not None and lm:
        a = [1 if ch == '1' else -1 for ch in lm]
        cone_live = {'label': f'D + r <= {S_live[x]}', 'constraints': [[a, S_live[x] + lm.count('1')]]}
    out['nodes'][x]['refined'] = {'S_live': S_live[x], 'cone_live': cone_live, 'live_routes': live, 'dead_routes': dd,
                                  'changed': S_live[x] != S[x]}
out['refinement'] = {'dead_routes': len(dead), 'by_reason': dict(collections.Counter(dead.values())),
                     'nodes_with_smaller_S': [x for x in nodes if S_live[x] != S[x]],
                     'proof': 'a dead route sends nothing: empty_by_tables = no term of the parent has any routed drop mask feasible on its record domains (cone-excess); proved_empty = every such term is zero_on_locus or excluded (independently re-checked) on every plane of some dropped parameter, by verify-handdown-proofs-verified hand-down proofs (prove_empty.py); S_live is the recursion of section 2 over the remaining routes'}

# cross-check against the enumerated arrival files
cc = {'files': [], 'per_child': {}}
per_child = collections.defaultdict(lambda: {'keys_sum': 0, 'hist': collections.Counter(), 'files': 0, 'max_dr': 0, 'outside_node_cone': 0, 'outside_edge_bound': 0})
if os.path.exists(C + '/arrival_dr.jsonl'):
    for l in open(C + '/arrival_dr.jsonl'):
        a = json.loads(l)
        p, c = a['parent'], alias.get(a['child'], a['child'])
        s = a['stats']
        d = delta.get((p, c), {}).get('delta')
        eb = (S[p] + d) if (S.get(p) is not None and isinstance(d, int)) else None
        above = lambda t: sum(v for k, v in s['hist_dr'].items() if int(k) > t) if t is not None else None
        row = {'file': a['file'], 'parent': p, 'child': c, 'keys': s['keys'], 'max_dr': s['max_dr'], 'max_d': s['max_d'],
               'max_r': s['max_r'], 'edge_bound': eb, 'outside_edge_bound': above(eb), 'S_child': S.get(c),
               'outside_node_cone': above(S.get(c)), 'outside_top_sector': s['outside_top_sector']}
        cc['files'].append(row)
        q = per_child[c]
        q['keys_sum'] += s['keys']; q['files'] += 1; q['max_dr'] = max(q['max_dr'], s['max_dr'])
        q['hist'].update({int(k): v for k, v in s['hist_dr'].items()})
        q['outside_node_cone'] += row['outside_node_cone'] or 0
        q['outside_edge_bound'] += row['outside_edge_bound'] or 0

def simplex_layer(k, t):
    return math.comb(t + k - 1, k - 1) if k > 0 else int(t == 0)

for c, q in per_child.items():
    fr = out['nodes'].get(c, {}).get('frame')
    k = len(fr) if fr else None
    band = q['max_dr']
    cone_band = sum(simplex_layer(k, t) for t in range(0, band + 1)) if k else None
    cc['per_child'][c] = {'files': q['files'], 'arrival_keys_sum': q['keys_sum'], 'max_dr': q['max_dr'], 'S': S.get(c),
                          'outside_node_cone': q['outside_node_cone'], 'outside_edge_bound': q['outside_edge_bound'],
                          'hist_dr': dict(sorted(q['hist'].items())),
                          'cone_points_in_band': cone_band,
                          'tightness_in_band': (q['keys_sum'] / cone_band) if cone_band else None}
json.dump(out, open(C + '/cones.json', 'w'), indent=1)
json.dump(cc, open(C + '/cones_crosscheck.json', 'w'), indent=1)
print(json.dumps({'nodes': len(nodes), 'S': {x: S[x] for x in nodes if x in ('T162', 'T178')}, 'S_live': {x: S_live[x] for x in nodes if x in ('T162', 'T178')}, 'refinement': out['refinement']['by_reason'], 'smaller': out['refinement']['nodes_with_smaller_S'],
                  'problems': problems[:20], 'n_problems': len(problems),
                  'files_checked': len(cc['files']),
                  'outside_node_cone': sum(r['outside_node_cone'] or 0 for r in cc['files']),
                  'outside_edge_bound': sum(r['outside_edge_bound'] or 0 for r in cc['files'])}))
