# lane arrivals: the ARRIVAL CONES of cones_phys.py (same excess on the shipped tables, same routes, same recursion
# S_X = max(S_box(X), max_P S_P + delta(P -> X)), dead routes = empty by tables) for OTHER START BOXES:
#   conservative : D <= 4, r <= E + D - 9 per ancestor (E = lines)  -> S_box = E - 1   (six-loop QCD closures; coordinator)
#   qed          : D <= 3, r <= 7                                   -> S_box = 10      (six-loop QED tops; informative)
#   physical     : D <= 1, r <= 4                                   -> S_box = 5       (the delivered run; check = cones.json)
# Usage: python3 -I cones_variant.py <rule> -> cones.<rule>.json
import json, os, sys, collections
C = os.path.dirname(os.path.abspath(__file__))
rule = sys.argv[1]
WS = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev'
dag = json.load(open(C + '/dag.json'))
st = json.load(open(WS + '/P5L_STATUS.json'))
lines_n = {n['id']: n['lines'] for n in st['nodes']}
alias = {'T162i': 'T162', 'T178f': 'T178'}
TSV = '/common/build-cache/vhirschi/p_integral_dev/helper-share/tadic/inputs/CELL_TOPS.tsv'
tsv_sha = None
tops, outside = {}, {}
main_col = set()
if rule.startswith('celltops'):
    import csv, hashlib
    tsv_sha = hashlib.sha256(open(TSV, 'rb').read()).hexdigest()
    for row in csv.DictReader(open(TSV), delimiter='\t'):
        x, D, r = row['node'], int(row['D']), int(row['r'])
        # MAIN line: top_r_at_D_if_n4hint (top_r_at_D where empty); on the EXACT-n4 file (2026-11-17, hint column = 'superseded')
        # the main line is top_r_exact (= top_r_at_D, both exact there). MARGIN (celltops_upper) = top_r_at_D.
        hint = row.get('top_r_at_D_if_n4hint', '').strip()
        if rule == 'celltops_upper':
            top = int(row['top_r_at_D'])
        elif hint.lstrip('-').isdigit():
            top = int(hint); main_col.add('top_r_at_D_if_n4hint')
        elif hint and (row.get('top_r_exact') or '').strip().lstrip('-').isdigit():
            top = int(row['top_r_exact']); main_col.add('top_r_exact')
        else:
            top = int(row['top_r_at_D']); main_col.add('top_r_at_D')
        tops.setdefault(x, {})[D] = top
        units = float(row.get('qcd_units_reaching') or 0)
        boxr = int(row['box_r_max'])
        if r <= top and (r > boxr or D > 4) and units > 0:
            outside.setdefault(x, []).append({'D': D, 'r': r, 'qcd_units_reaching': units, 'box_r_max': boxr})
def s_box(x):
    E = lines_n[x]
    if rule.startswith('celltops'):
        t = tops.get(x)
        return max(D + r for D, r in t.items()) if t else None  # no unit has its core here: no start set
    return {'conservative': E - 1, 'qed': 10, 'physical': 5}[rule]
ex = {o['node']: json.load(open(f"{C}/excess/{o['node']}.json")) for o in dag['nodes'] if os.path.exists(f"{C}/excess/{o['node']}.json")}
edges = {}
for l in open(C + '/edges.jsonl'):
    e = json.loads(l)
    p, c = os.path.basename(e['edge'])[:-10].split('-')
    edges[(p, alias.get(c, c))] = e
nodes = sorted((o['node'] for o in dag['nodes']), key=lambda x: (-lines_n[x], x))
parents = collections.defaultdict(list)
for p, c, f in dag['edges']:
    parents[c].append(p)
S, why, problems = {}, {}, []
for x in nodes:
    s, w = s_box(x), ['box', s_box(x)]
    if s is None:
        s, w = -10**9, ['no start box (node not in CELL_TOPS.tsv)']
    for p in sorted(parents[x]):
        e = edges.get((p, x))
        if e is None or not e['bound_holds'] or p not in ex:
            problems.append(f'{p} -> {x}: missing edge / bound / excess'); s = None; continue
        pm = ex[p]['excess']['per_mask']
        vals = [pm[m][0] for m in e['masks'] if m in pm]
        if not vals:
            continue  # empty by tables
        if any(v is None for v in vals):
            problems.append(f'{p} -> {x}: unbounded'); s = None; continue
        d = max(vals)
        if S.get(p) is None or s is None:
            s = None; continue
        if S[p] + d > s:
            s, w = S[p] + d, ['edge', p, S[p], d]
    S[x], why[x] = s, w
out = {'schema': 'rustred.campaign.arrival-cones.v1', 'rule': rule,
       'start_box': {'conservative': 'D <= 4, r <= E + D - 9 per ancestor (S_box = E - 1)', 'qed': 'D <= 3, r <= 7 (S_box = 10)', 'physical': 'D <= 1, r <= 4 (S_box = 5)',
                     'celltops_n4': 'CELL_TOPS.tsv main line: top(D) = top_r_at_D_if_n4hint (top_r_at_D where empty); top_r_exact on the exact-n4 file (hint superseded); not clipped by BOXDEF v3',
                     'celltops_upper': 'CELL_TOPS.tsv margin line: top(D) = top_r_at_D (n4 = 0 upper bound; EXACT in n4 on the exact-n4 file)'}[rule],
       'cell_tops_tsv': {'path': TSV, 'sha256': tsv_sha} if tsv_sha else None,
       'main_line_column': sorted(main_col) or None,
       'tsv_exact_n4': (tsv_sha is not None and 'top_r_exact' in main_col) or None,
       'tops': tops or None,
       'cells_outside_boxdef_v3_reached': {x: v for x, v in outside.items()} or None,
       'tables': 'shipped (package s12349c + T266 v1.2 relation entries), excess = physcone/excess', 'problems': problems,
       'nodes': {x: {'lines': lines_n[x], 'S': (S[x] if S[x] is None or S[x] > -10**8 else None), 'why': why[x], 'S_box': s_box(x)} for x in nodes}}
json.dump(out, open(f'{C}/cones.{rule}.json', 'w'), indent=1)
print(json.dumps({'rule': rule, 'S_T162': S['T162'], 'S_T178': S['T178'], 'no_box': [x for x in nodes if s_box(x) is None][:20], 'why_T162': why['T162'], 'why_T178': why['T178'], 'problems': len(problems)}))
