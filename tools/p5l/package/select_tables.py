#!/usr/bin/env python3
"""select_tables.py OUTDIR: the table of every 5L DAG Custom node for the 1-5 package (vk5l WP6), per top the
stage, and the import plan.

Per node (P5L_STATUS.json ledger, 283 nodes):
  lib14   : owner of the shipped 1-4 package (artefacts/m3/owners-1-4): provided by the 1-4 package, not imported;
  regen   : a regenerated table ACCEPTED in P5L_SIMPLIFY.json (nodes[].regenerated.state == 'accepted'), which is
            also listed in SCRATCHPAD/seed-study/regenerated/<T>.json (same path);
  min     : the minimised table of P5L_SIMPLIFY.json (state 'minimised': nodes[].after.path);
  orig    : the original table of P5L_SIMPLIFY.json (state 'unchanged');
  ledger  : a certified node not in P5L_SIMPLIFY.json: close.table > replacement.table > imported.table;
  boxed   : T162 / T178 (boxed_closed): the boxed mixed table + certified key set (statement), see BOXED below;
  open    : T266 (stuck): declared open (no table).
An existing import record (rustred.p-imported-owner.v1 .json + .rrbin) whose 'table' is the selected path is reused.
Writes OUTDIR/selection.json, OUTDIR/selection.tsv, OUTDIR/tops.tsv."""
import csv
import glob
import json
import os
import sys

WS = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev'
SC = WS + '/SCRATCHPAD'
OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)
BOXED = {
    'T178': {'table': '/common/build-cache/vhirschi/p_integral_dev/vk5l-wp1/t178/T178-box.unionbox.jsonl',
             'statement': SC + '/t178-close/statements/statement-20261114T040105Z/statement.jsonl',
             'import': '/common/build-cache/vhirschi/p_integral_dev/vk5l-wp1/t178/out/T178.json'},
    'T162': {'table': None, 'statement': SC + '/t162-box/runs/statement-closure-20261114T0623Z', 'import': None},
}
OPEN = {'T266': 'stuck: not closed'}

led = {x['id']: x for x in json.load(open(WS + '/P5L_STATUS.json'))['nodes']}
simp = {n['id']: n for n in json.load(open(WS + '/P5L_SIMPLIFY.json'))['nodes']}
regen_files = {}
for f in glob.glob(SC + '/seed-study/regenerated/*.json'):
    r = json.load(open(f))
    regen_files[r['node']] = r

# Existing import records by source table.
records = {}
for d in [SC + '/p5l-converter/out_v2', SC + '/p5l-converter/out_v2b', SC + '/p5l-converter/out_v2c',
          SC + '/p5l-converter/out_v2d', SC + '/p5l/close/libimports', SC + '/p5l/close/libimports5',
          SC + '/p5l-library/imports', SC + '/p5l-library-2/imports', SC + '/p5l-library-2/imports_t222',
          '/common/build-cache/vhirschi/p_integral_dev/vk5l-wp6/imports']:
    for f in glob.glob(d + '/T*.json'):
        t = os.path.basename(f)[:-5]
        rb = f[:-5] + '.rrbin'
        if '.' in t or not os.path.exists(rb):
            continue
        try:
            r = json.load(open(f))
        except Exception:
            continue
        if r.get('schema') != 'rustred.p-imported-owner.v1':
            continue
        records.setdefault((t, r.get('table')), []).append((os.path.getmtime(rb), f, rb, os.path.getsize(rb)))


def size(path):
    try:
        return os.path.getsize(path)
    except OSError:
        return None


nodes = {}
for t, x in sorted(led.items()):
    row = {'id': t, 'class': x['class'], 'lines': x['lines'], 'state': x['state']}
    s = simp.get(t)
    if 'in the shipped 1-4L library' in (x.get('note') or ''):
        row.update(kind='lib14', path=None, bytes=0)
    elif t in OPEN:
        row.update(kind='open', path=None, bytes=0, status=OPEN[t])
    elif t in BOXED:
        b = BOXED[t]
        row.update(kind='boxed', path=b['table'], bytes=size(b['table']) if b['table'] else None,
                   statement=b['statement'], boxed_import=b['import'])
    elif s and (s.get('regenerated') or {}).get('state') == 'accepted':
        g = s['regenerated']
        rf = regen_files.get(t)
        row.update(kind='regen', path=g['path'], bytes=g['bytes'], sha256=g.get('sha256'),
                   min_bytes=s['after']['bytes'] if s.get('state') == 'minimised' else s['original']['bytes'],
                   regen_file_agrees=bool(rf and rf.get('table') == g['path']))
    elif s and s.get('state') == 'minimised':
        row.update(kind='min', path=s['after']['path'], bytes=s['after']['bytes'], sha256=s['after'].get('sha256'),
                   regen_state=(s.get('regenerated') or {}).get('state'))
    elif s:
        row.update(kind='orig', path=s['original']['path'], bytes=s['original']['bytes'],
                   sha256=s['original'].get('sha256'))
    else:
        p = None
        for k in ('close', 'replacement', 'imported'):
            v = x.get(k)
            if isinstance(v, dict) and v.get('table'):
                p = v['table']
                break
        row.update(kind='ledger', path=p, bytes=size(p) if p else None)
    rec = records.get((t, row.get('path'))) if row.get('path') else None
    if t == 'T178':
        rec = [(0, BOXED['T178']['import'], BOXED['T178']['import'][:-5] + '.rrbin',
                size(BOXED['T178']['import'][:-5] + '.rrbin'))]
    if rec:
        rec.sort(reverse=True)
        row.update(import_json=rec[0][1], import_rrbin=rec[0][2], payload_bytes=rec[0][3])
    nodes[t] = row

# Tops: coverage.tsv (DAG of p-families-1-5.json) joined with node_map.tsv.
nm = {r['node']: r['id'] for r in csv.DictReader(open(SC + '/p5l-converter/node_map.tsv'), delimiter='\t')}
gk = {r['id']: r['graph_key'] for r in csv.DictReader(open(SC + '/p5l-converter/node_map.tsv'), delimiter='\t')}
tops = []
for r in csv.DictReader(open(SC + '/p5l-library/coverage.tsv'), delimiter='\t'):
    tids = sorted({nm[n] for n in r['custom_nodes'].split(',') if n})
    need = [t for t in tids if nodes[t]['kind'] not in ('lib14',)]
    by = [nodes[t].get('bytes') or 0 for t in need]
    kinds = sorted({nodes[t]['kind'] for t in need})
    tops.append(dict(top=r['top'], top_action=r['top_action'], dag_nodes=int(r['nodes']), custom=tids,
                     max_table_bytes=max(by) if by else 0, table_bytes=sum(by),
                     boxed=[t for t in need if nodes[t]['kind'] == 'boxed'],
                     open=[t for t in need if nodes[t]['kind'] == 'open'], kinds=kinds))
for tp in tops:
    if tp['open']:
        tp['stage'] = 9
    elif 'T162' in tp['boxed']:
        tp['stage'] = 4
    elif tp['boxed']:
        tp['stage'] = 3
    elif tp['max_table_bytes'] <= 200e6:
        tp['stage'] = 1
    else:
        tp['stage'] = 2
tops.sort(key=lambda x: (x['stage'], x['max_table_bytes'], x['table_bytes']))
for t, row in nodes.items():
    row['graph_key'] = gk.get(t)
    row['tops'] = [tp['top'] for tp in tops if t in tp['custom']]

json.dump({'nodes': nodes, 'tops': tops}, open(OUT + '/selection.json', 'w'), indent=1)
with open(OUT + '/selection.tsv', 'w') as h:
    h.write('id\tkind\tbytes\tpayload_bytes\ttops\timport\tpath\n')
    for t, r in nodes.items():
        h.write(f"{t}\t{r['kind']}\t{r.get('bytes')}\t{r.get('payload_bytes')}\t{len(r['tops'])}\t"
                f"{r.get('import_json') or ''}\t{r.get('path') or ''}\n")
with open(OUT + '/tops.tsv', 'w') as h:
    h.write('stage\ttop\tmax_table_bytes\ttable_bytes\tcustom\tboxed\topen\n')
    for tp in tops:
        h.write(f"{tp['stage']}\t{tp['top']}\t{tp['max_table_bytes']}\t{tp['table_bytes']}\t{len(tp['custom'])}\t"
                f"{','.join(tp['boxed'])}\t{','.join(tp['open'])}\n")
# Summary.
from collections import Counter, defaultdict
used = {t for tp in tops for t in tp['custom']}
k = Counter(nodes[t]['kind'] for t in used)
b = defaultdict(int)
for t in used:
    b[nodes[t]['kind']] += nodes[t].get('bytes') or 0
reuse = sum(1 for t in used if nodes[t].get('import_json'))
need_import = [t for t in used if nodes[t]['kind'] not in ('lib14', 'open') and not nodes[t].get('import_json')]
print('nodes used by the 64 tops:', len(used), dict(k))
print('table bytes by kind:', {x: round(v / 1e9, 3) for x, v in b.items()})
print('existing imports reusable:', reuse, ' to import:', len(need_import),
      round(sum(nodes[t].get('bytes') or 0 for t in need_import) / 1e9, 3), 'GB')
print('tops per stage:', Counter(tp['stage'] for tp in tops))
print('unused ledger nodes:', sorted(set(nodes) - used))
