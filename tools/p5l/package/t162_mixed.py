#!/usr/bin/env python3
"""t162_mixed.py STATEMENT_CLOSURE_DIR OUT_DIR (lane vk5l-wp6; copy of SCRATCHPAD/t162-box/tools/assemble2.py + domain_bounds):
the T162 mixed table on a closure statement whose catalogue carries bank qualified domains (0623Z and later). Every bank record
gets the engine field `domain_bounds` {min, max} (node/bounds.rs, engine-line ca184c0a) from the catalogue's domain_lower /
domain_upper on FREE slots where they tighten the chart (line min > 1, numerator max < 0); cross-checked against the record's own
recenter (bounds_from_recenter rule): any disagreement aborts. Everything else as assemble2.py:
the MIXED boxed table of T162 rebuilt on a CLOSURE statement (session 3).
Rules, each ONCE, new 'index' + 'origin': every rule the closure dispatches to (statement ours_used + bank_used), in DISPATCH ORDER of
the statement's catalogue (bank ordinals first, then ours), record = the original record verbatim (bank: Astra's native body from the
handoff copy; ours: the run's table record, or the interim point-table record for merged_from names), apart from 'index' and the
added 'origin'. Rules the closure never dispatches to are NOT written (this drops the shadowed duplicate point rule 920/721 of the
first mixed table and every superseded point rule). Terminals = residual records with boxed_terminal true. Header = interim header +
'boxed_node' (rustred.boxed-node.v1, box union, parametric_after_points true, complete false while open keys remain).
dispatch.json: EVERY closure key -> rule index | 'terminal' | 'astra-open' | 'open'. Streaming; prints counts + sha256."""
import hashlib, json, os, sys
D = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/t162-box'
H = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev/handoff/sym-investigation-for-ben/t162-bank'
S, O = sys.argv[1], sys.argv[2]; os.makedirs(O, exist_ok=True)
st = json.load(open(S + '/statement.json')); cat = json.load(open(S + '/catalogue.json'))['entries']
used = set(st['ours_used']) | {'bank#%d' % o for o in st['bank_used']}
idx = {json.loads(l)['study_ordinal0']: json.loads(l) for l in open(H + '/bank/RULE-INDEX.jsonl')}
body = open(H + '/bank/NATIVE-RULE-BODIES.jsonl', 'rb')
hdr = zs = ch = None
for l in open(D + '/merged/interim/T162-box.jsonl'):
    r = json.loads(l)
    if r['type'] == 'header': hdr = r
    elif r['type'] == 'zero_sectors': zs = r
    elif r['type'] == 'chart': ch = r
    elif r['type'] == 'rule': break
NB = {'with_bounds': 0, 'line_bounds': 0, 'numerator_bounds': 0}
def bounds(e, r):
    chart = [c == '1' for c in r['chart']]; rc = r.get('recenter') or [0] * len(chart); fx = r['fixed']
    lo, up = e['domain_lower'], e['domain_upper']
    mn = [None] * len(chart); mx = [None] * len(chart)
    for i, line in enumerate(chart):
        if fx[i] is not None:   # fixed slot: the recentred bound must hold at the fixed value (else the rule is dead)
            if line and fx[i] < max(1, 1 - rc[i]) or (not line) and fx[i] > min(0, -rc[i]):
                raise SystemExit(f"{e['name']}: fixed slot {i} = {fx[i]} violates its recentred bound")
            continue
        want_lo = max(1, 1 - rc[i]) if line else None; want_up = min(0, -rc[i]) if not line else None
        if (lo[i] if line else up[i]) != (want_lo if line else want_up):
            raise SystemExit(f"{e['name']}: catalogue bound slot {i} {lo[i]}/{up[i]} != recenter bound {want_lo}/{want_up}")
        if line and lo[i] > 1: mn[i] = lo[i]; NB['line_bounds'] += 1
        if (not line) and up[i] < 0: mx[i] = up[i]; NB['numerator_bounds'] += 1
    if any(v is not None for v in mn + mx):
        r['domain_bounds'] = {'min': mn, 'max': mx}; NB['with_bounds'] += 1
def orig(e):
    n = e['name']
    if e['kind'] == 'bank':
        o = int(n[5:]); x = idx[o]; body.seek(x['local_body_offset']); r = json.loads(body.read(x['local_body_bytes']))
        bounds(e, r)
        r['origin'] = {'source': 'bank', 'study_ordinal0': o, 'study_ID': x['study_ID'], 'materialization': x.get('ORIGINAL_materialization'),
                       'guard_epoch': x.get('guard_epoch_unchanged'), 'native_index': r['index']}
        return r
    run, i = n.rsplit('#', 1); t = f'{D}/runs/{run}/node/F.table.jsonl'
    if os.path.exists(t):
        with open(t, 'rb') as f:
            for l in f:
                if l.startswith(b'{"type":"rule"') or l.startswith(b'{"type": "rule"'):
                    r = json.loads(l)
                    if r['index'] == int(i): r['origin'] = {'source': 't162-box', 'name': n, 'run_index': r['index']}; return r
    with open(e['table'], 'rb') as f:   # interim point-table record (compact sidecar copy of the merged record)
        f.seek(e['offset']); r = json.loads(f.readline())
    r['origin'] = {'source': 't162-box', 'name': n, 'merged_from': r.get('merged_from')}; return r
hdr = dict(hdr); hdr.pop('merged_tables', None)
openk = st['astra_keys_in_closure'] + st['open_keys']
hdr['boxed_node'] = {'schema': 'rustred.boxed-node.v1', 'node': 'T162', 'chart': ch['chart'], 'parametric_after_points': True,
    'box': {'name': 'union', 'measure': 'dots-rank-v1', 'layers': [[0, 1], [1, 2], [2, 2], [3, 2], [4, 2]], 'cap': 5}, 'box_keys': st['in_box'],
    'complete': len(openk) == 0, 'open_keys': len(openk), 'closure_keys': st['keys'],
    'mixed_sources': ['bank (Astra, 2,440 qualified, first match)', 't162-box face + point rules (batch-06)'],
    'coverage': {'tool': 'tools/closure.py (forward closure, first match, rr_proof instantiate)', 'statement': os.path.basename(S)},
    'certificate': 'see the statement directory cert summary',
    'bank_domain': 'bank records carry domain_bounds (qualified domain = chart tightened by recenter; engine node/bounds.rs)'}
h = hashlib.sha256(); nr = nt = 0; ridx = {}
out = open(O + '/T162-box.mixed.jsonl', 'w')
def w(o):
    s = json.dumps(o) + '\n'; out.write(s); h.update(s.encode())
for r in (hdr, zs, ch): w(r)
for e in cat:
    if e['name'] not in used: continue
    r = orig(e)
    if r.get('case') == 'Point (symmetry rule)':   # symrule.py label -> the engine's case text (the importer reads the case kind)
        r['case'] = 'Coordinate(CoordinateCase { fixed: [%s] })' % ', '.join('Some(%d)' % v for v in r['fixed']); r['case_note'] = 'symmetry rule'
    r['index'] = nr; ridx[e['name']] = nr; nr += 1
    nt += len(r['rhs']) if isinstance(r['rhs'], list) else len(json.loads(r['rhs'].replace("'", '"'))); w(r)
for t in st['terminals']:
    w({'type': 'residual', 'chart': ch['chart'], 'key': t, 'boxed_terminal': True, 'status': 'unknown integral', 'origin': 'declared'})
w({'type': 'chart_end', 'chart': ch['chart'], 'rules': nr, 'rhs_terms': nt, 'residuals': len(st['terminals'])})
w({'type': 'end', 'charts': 1, 'rules': nr, 'rhs_terms': nt, 'residuals': len(st['terminals']), 'state': 'complete' if not openk else 'partial'})
out.close()
disp = {}
for l in open(S + '/keys.jsonl'):
    k, lab, name, inb, L, *_ = json.loads(l)
    disp[k] = ridx[name] if name else {'terminal': 'terminal', 'astra': 'astra-open'}.get(lab, 'open')
json.dump(disp, open(O + '/dispatch.json', 'w'))
summ = {'table': O + '/T162-box.mixed.jsonl', 'sha256': h.hexdigest(), 'bytes': os.path.getsize(O + '/T162-box.mixed.jsonl'), 'rules': nr,
        'bank_rules': sum(1 for n in ridx if n.startswith('bank#')), 't162box_rules': sum(1 for n in ridx if not n.startswith('bank#')),
        'rhs_terms': nt, 'terminals': st['terminals'], 'open_keys': len(openk), 'dispatch_keys': len(disp), 'statement': os.path.basename(S),
        'missing': sorted(used - set(ridx)), 'domain_bounds': NB}
json.dump(summ, open(O + '/SUMMARY.json', 'w'), indent=1); print(json.dumps({k: v for k, v in summ.items() if k != 'terminals'}))
