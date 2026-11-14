# Convert the checker's T178 reference records (read-only /common/ben/sym-investigation/k1) into the ladder replay
# fixture. Inspection of reference data only (golden rule 11).
import json, glob, os, re, sys
K1 = '/common/ben/sym-investigation/k1'
out_dir = sys.argv[1]
def key(s):
    return [int(x) for x in s[2:-1].split(',')]
worker = {}
for w in ('w0', 'w1', 'w2'):
    for i, l in enumerate(open(f'{K1}/{w}/STAGES.txt')):
        m = re.match(r'=== \S+ ITEM (\d+)(-add)?: ', l)
        if m and m.group(2) is None:
            worker.setdefault(int(m.group(1)), (w, i + 1))
faces = {}
for l in open(f'{K1}/faces.jsonl'):
    r = json.loads(l)
    if r.get('kind') != 'face':
        continue
    faces[r['id']] = r
recs = []
skipped = []
for f in sorted(glob.glob(f'{K1}/results/*/*.json')):
    rel = os.path.relpath(f, '/common/ben/sym-investigation')
    try:
        r = json.load(open(f))
    except Exception as e:
        skipped.append((rel, 'unreadable')); continue
    if r.get('mode') not in ('case', 'point'):
        skipped.append((rel, 'mode')); continue
    sg = r['seed_generator']; sh = sg['shells']
    if sh == 'S0+S1+Sm1+S1m1+S2': depth = 2
    elif sh == 'S0+S1+Sm1+S1m1+S2+S3': depth = 3
    else:
        skipped.append((rel, 'shells ' + sh)); continue
    if not rel.split('/')[2].isdigit():
        skipped.append((rel, 'locus item')); continue
    item = int(rel.split('/')[2])
    c = r['case']
    if r['mode'] == 'point':
        k = key(r['key'])
        sd = [i for i in range(10) if k[i] >= 2]; sn = [i for i in range(10, 20) if k[i] <= -1]
        fixed = {str(i): k[i] for i in sd + sn}
        eqs = []
        det = r.get('replay') or []
    else:
        sd, sn = c['sd'], c['sn']
        fixed = {s: v for s, v in c['fix'].items()}
        eqs = c['eqs']
        det = (r.get('census') or {}).get('detail') or []
    offs = []
    for rc in r['recenter']:
        v = rc['recenter']
        m = {str(i): x for i, x in enumerate(v) if x != 0}
        if m: offs.append(m)
    rhs = r.get('rhs') or {}
    recs.append({
        'item': item, 'source': rel, 'case': {'sd': sd, 'sn': sn, 'fixed': fixed, 'eqs': eqs},
        'offsets': offs, 'floor': sg.get('floor') == 'lines', 'depth': depth, 'meas': r['meas'],
        'exists': bool(r['exists']), 'rhs_terms': rhs.get('terms'), 'rows': r.get('closure_rows'),
        'tie_break': (not rhs.get('all_top_Dr_lower', True)) if rhs else None,
        'cpu_ms': int(1000 * float(r.get('wall_s') or 0)),
        'points': [key(d['key']) for d in det], 'verdicts': [d['verdict'] for d in det]})
items = []
for fid in sorted({r['item'] for r in recs}):
    fr = faces.get(fid)
    w = worker.get(fid)
    items.append({'item': fid, 'sd': fr['Sd'] if fr else None, 'sn': fr['Sn'] if fr else None,
                  'census': [key(s) for s in fr['census']] if fr else [],
                  'closure_keys': [key(s) for s in fr['closure_keys']] if fr else [],
                  'worker': w[0] if w else 'A', 'stages_line': w[1] if w else None})
with open(f'{out_dir}/t178_k1_attempts.jsonl', 'w') as o:
    for r in recs: o.write(json.dumps(r, separators=(',', ':')) + '\n')
with open(f'{out_dir}/t178_k1_items.jsonl', 'w') as o:
    for r in items: o.write(json.dumps(r, separators=(',', ':')) + '\n')
print(len(recs), 'records', len(items), 'items', 'skipped', skipped)
