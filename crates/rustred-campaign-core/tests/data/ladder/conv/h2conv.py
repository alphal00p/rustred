# Convert the helper's T162 hard-face records (read-only /common/ben/sym-investigation/h2) into the replay fixture.
import json, itertools, sys
H2 = '/common/ben/sym-investigation/h2'
out_dir = sys.argv[1]
g = json.load(open(f'{H2}/group.json'))
faces = [tuple(f) for orb in g['orbits'].values() for f in orb]
assert len(faces) == 44, len(faces)
idx = {f: i for i, f in enumerate(faces)}
def census(face):
    pts = []
    for a in itertools.product(range(2, 9), repeat=len(face)):
        if sum(x - 1 for x in a) <= 7:
            pts.append(list(a))
    return pts
def key(face, a):
    k = [1] * 9 + [0] * 11
    for s, v in zip(face, a): k[s] = v
    return k
recs = []; seen = set()
def emit(face, case, offs, floor, fails, exists, s, t, src, pts=None):
    pts = pts if pts is not None else census(face)
    sig = (face, json.dumps(case, sort_keys=True), json.dumps(sorted(json.dumps(o, sort_keys=True) for o in offs)), floor)
    if sig in seen: return
    seen.add(sig)
    fs = {tuple(f) for f in (fails or [])}
    recs.append({'item': idx[face], 'source': src, 'case': case, 'offsets': offs, 'floor': floor, 'depth': 2,
                 'meas': 'M1', 'exists': exists, 'rhs_terms': s.get('relation_len'), 'rows': s.get('closure_rows'),
                 'tie_break': False, 'cpu_ms': int(1000 * (t or 0)),
                 'points': [key(face, a) for a in pts], 'verdicts': ['fail' if tuple(a) in fs else 'regular' for a in pts]})
for fn in ('sweep2_orbits.jsonl', 'sweep2_others.jsonl', 'sweep2.jsonl'):
    for i, l in enumerate(open(f'{H2}/{fn}')):
        r = json.loads(l); face = tuple(r['face'])
        if face not in idx: continue
        case = {'sd': list(face), 'sn': [], 'fixed': {}, 'eqs': []}
        emit(face, case, [{str(r['x']): -1}], r['variant'] == 'floor', r['fails'], r['search']['feasible'],
             r['search'], r['t'], f'h2/{fn}:{i+1}')
for i, l in enumerate(open(f'{H2}/mc.jsonl')):
    r = json.loads(l); lab = r['label']; s = r['search']
    if lab.endswith('_rev'): continue   # same seed set as 134mc (the helper's eliminator is order dependent, H s.1)
    face = tuple(int(x) for x in s['sym'].split(','))
    offs = [{c.split(':')[0]: -1} for c in s['rec'].split(';')]
    if 'diag' in lab:
        case = {'sd': list(face), 'sn': [], 'fixed': {}, 'eqs': ['n1-n3', 'n3-n4']}
        pts = [[k, k, k] for k in range(2, 8)]
    else:
        case = {'sd': list(face), 'sn': [], 'fixed': {}, 'eqs': []}; pts = None
    emit(face, case, offs, True, r.get('fails'), s['feasible'], s, r.get('t'), f'h2/mc.jsonl:{i+1}', pts)
items = [{'item': i, 'sd': list(f), 'sn': [], 'census': [key(f, a) for a in census(f)], 'closure_keys': [],
          'worker': 'h2', 'stages_line': 59} for i, f in enumerate(faces)]
with open(f'{out_dir}/t162_h2_attempts.jsonl', 'w') as o:
    for r in recs: o.write(json.dumps(r, separators=(',', ':')) + '\n')
with open(f'{out_dir}/t162_h2_items.jsonl', 'w') as o:
    for r in items: o.write(json.dumps(r, separators=(',', ':')) + '\n')
print(len(recs), 'records')
