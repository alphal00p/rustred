#!/usr/bin/env python3
"""imports.py PLAN_DIR STAGES [--parallel N] [--dry]: p-import-table of every node the tops of STAGES (e.g. 1,2,3)
need and that has no reusable import record (select_tables.py), one job per node through job6.py (one CPU, flock,
ledger, systemd scope with MemoryMax = 8 GB + 6 x table bytes, at most 48 GB; guarded_run floor 50 GB).
Arguments per node: rules_study/targets.tsv (edges, S, T), ledger lp, census family rules_study/classify/families/<T>.sym,
and the order map of the node's previous import of the ORIGINAL table when it had one. Output:
/common/build-cache/vhirschi/p_integral_dev/vk5l-wp6/imports/<T>.{json,rrbin} (a node whose run dir has rc 0 is skipped:
resumable). Small tables first. Writes PLAN_DIR/imports.<stages>.tsv (the assembly's imports file, open nodes as OPEN
lines) when every import of the stages exists."""
import csv
import glob
import json
import os
import re
import subprocess
import sys
import threading
import time

WS = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev'
SC = WS + '/SCRATCHPAD'
D = '/common/build-cache/vhirschi/p_integral_dev/vk5l-wp6'
BIN = os.environ.get('WP6_RUSTRED', WS + '/build/vk5l-wp6/release/rustred')
JOB = os.path.dirname(os.path.abspath(__file__)) + '/job6.py'

plan = sys.argv[1]
stages = {int(s) for s in sys.argv[2].split(',')}
par = int(sys.argv[sys.argv.index('--parallel') + 1]) if '--parallel' in sys.argv else 6
dry = '--dry' in sys.argv
sel = json.load(open(plan + '/selection.json'))
nodes, tops = sel['nodes'], [t for t in sel['tops'] if t['stage'] in stages]
need = sorted({t for tp in tops for t in tp['custom']})
targets = {r['id']: r for r in csv.DictReader(open(WS + '/rules_study/targets.tsv'), delimiter='\t')}
simp = {n['id']: n for n in json.load(open(WS + '/P5L_SIMPLIFY.json'))['nodes']}

# Order maps of earlier imports, by (node, table).
order_maps = {}
for f in glob.glob(SC + '/p5l-converter/out_v2*/T*.json') + glob.glob(SC + '/p5l/close/libimports*/T*.json') + \
        glob.glob(SC + '/p5l-library*/imports*/T*.json'):
    try:
        r = json.load(open(f))
    except Exception:
        continue
    if isinstance(r, dict) and r.get('order_map'):
        order_maps[(os.path.basename(f)[:-5], r.get('table'))] = r['order_map']


TRANSLATED = SC + '/p5l/close/libimports5/translated'
ASTRA_ENDS = ('assembly_end', 'composite_assembly_complete')


def record_type(line):
    """(type, record or None): the fast path reads a leading "type" key; any other record is parsed."""
    m = re.match(r'\{\s*"type"\s*:\s*"(\w+)"', line)
    if m:
        return m.group(1), None
    r = json.loads(line)
    return r.get('type'), r


def prepare(t, path):
    """The table as the importer takes it (as p5l-close's libimports prepared the ledger tables): when the header's
    `charts` miss a chart its records use, the union in first-appearance order; residual records taken from
    p5l-close's translated copy of the node (libimports5/translated/<T>.table.jsonl) when the table has none; a rule
    whose `index` breaks the consecutive numbering of its chart (a record the minimiser did not renumber, e.g.
    T211's trailing composite rule) renumbered in place (file order = first-match order is kept); an Astra end record
    (assembly_end, composite_assembly_complete) or a missing one replaced by a v1 `end` record, the original kept as
    an `assembly_provenance` record. Rules and their order are never touched otherwise. Returns the path to import
    (the original when nothing is needed) and a note."""
    with open(path) as f:
        header = json.loads(f.readline())
    charts, seen, residuals, rules, ends = list(header.get('charts') or []), [], 0, 0, 0
    count, renumber, last_type = {}, {}, None
    with open(path) as f:
        f.readline()
        for n, line in enumerate(f):
            ty, rec = record_type(line)
            last_type = ty
            if ty == 'chart':
                c = (rec or json.loads(line))['chart']
                if c not in seen:
                    seen.append(c)
            elif ty == 'residual':
                residuals += 1
            elif ty == 'rule':
                rules += 1
                head = line[:4000]
                mc = re.search(r'"chart"\s*:\s*"([01]+)"', head)
                mi = re.search(r'"index"\s*:\s*(\d+)', head)
                if rec is not None or not mc or not mi:
                    rec = rec or json.loads(line)
                    c, i = rec['chart'], rec['index']
                else:
                    c, i = mc.group(1), int(mi.group(1))
                if i != count.get(c, 0):
                    renumber[n] = count.get(c, 0)
                count[c] = count.get(c, 0) + 1
            elif ty == 'end':
                ends += 1
    fix_charts = [c for c in seen if c not in charts]
    fix_end = ends == 0
    extra_residuals = []
    if residuals == 0 and os.path.exists(f'{TRANSLATED}/{t}.table.jsonl'):
        with open(f'{TRANSLATED}/{t}.table.jsonl') as f:
            for line in f:
                if record_type(line)[0] == 'residual':
                    extra_residuals.append(line if line.endswith('\n') else line + '\n')
    if not fix_charts and not fix_end and not extra_residuals and not renumber:
        return path, None
    out = f'{D}/prepared/{t}.table.jsonl'
    os.makedirs(os.path.dirname(out), exist_ok=True)
    note = {'type': 'assembly_provenance', 'tool': 'vk5l-wp6 imports.py prepare', 'source': path,
            'header_charts_added': fix_charts, 'rules_renumbered': {str(k): v for k, v in renumber.items()},
            'residuals_from': f'{TRANSLATED}/{t}.table.jsonl' if extra_residuals else None, 'end_added': fix_end}
    with open(path) as f, open(out + '.tmp', 'w') as g:
        h = json.loads(f.readline())
        if fix_charts:
            h['charts'] = charts + fix_charts
        g.write(json.dumps(h) + '\n')
        for n, line in enumerate(f):
            ty, rec = record_type(line)
            if fix_end and ty in ASTRA_ENDS:
                note.setdefault('original_end_records', []).append(rec or json.loads(line))
                continue
            if n in renumber:
                rec = rec or json.loads(line)
                rec['index'] = renumber[n]
                line = json.dumps(rec)
            g.write(line if line.endswith('\n') else line + '\n')
        for line in extra_residuals:
            g.write(line)
        if fix_end:
            g.write(json.dumps({'type': 'end', 'charts': len(charts + fix_charts), 'rules': rules,
                                'residuals': residuals + len(extra_residuals), 'chart_errors': 0,
                                'harness': 'vk5l-wp6: end record added for the library import (see the provenance record)'}) + '\n')
        g.write(json.dumps(note) + '\n')
    os.replace(out + '.tmp', out)
    return out, {k: v for k, v in note.items() if k not in ('type', 'original_end_records')}


def job(t):
    n = nodes[t]
    out = f'{D}/imports'
    run = f'{D}/runs/import-{t}'
    tg = targets[t]
    led_lp = json.load(open(WS + '/P5L_STATUS.json'))
    lp = next(x for x in led_lp['nodes'] if x['id'] == t).get('lp')
    if lp is None:
        lp = tg['masters_lp_nosym']
    table, note = prepare(t, n['path']) if n['kind'] != 'boxed' else (n['path'], None)
    if note:
        print(f'{t}: prepared {table}: {note}', flush=True)
    cmd = [BIN, 'p-import-table', '--table', table, '--graph', tg['edges'], '--ext', f"{tg['S']},{tg['T']}",
           '--label', t, '--lp', str(lp), '--out', out, '--force',
           '--census-family', f'{WS}/rules_study/classify/families/{t}.sym']
    orig = (simp.get(t) or {}).get('original', {}).get('path')
    om = order_maps.get((t, orig)) or order_maps.get((t, n['path']))
    if om and n['kind'] != 'regen':
        cmd += ['--order-map', om]
    if n['kind'] == 'boxed':
        cmd += ['--mixed-box', '--certified-keys', n['statement']]
    mem = min(48, 8 + int(6 * (n.get('bytes') or 0) / 1e9 + 0.999))
    full = [sys.executable, '-I', JOB, '--tag', f'import-{t}', '--ncpu', '1', '--mem', f'{mem}G',
            '--max-seconds', '14400', '--out', run, '--wait', '7200', '--',
            'bash', '-c', f'source {WS}/ops/env.sh; ' + ' '.join(f"'{c}'" for c in cmd)]
    return run, full, mem


todo = []
for t in need:
    n = nodes[t]
    if n['kind'] in ('lib14', 'open') or n.get('import_json'):
        continue
    if n['kind'] == 'boxed' and not n.get('path'):
        print(f'{t}: boxed without a table: stage needs it, not importable now', file=sys.stderr)
        continue
    todo.append(t)
todo.sort(key=lambda t: nodes[t].get('bytes') or 0)
print(f'stages {sorted(stages)}: {len(tops)} tops, {len(need)} nodes, {len(todo)} to import, '
      f'{sum(nodes[t].get("bytes") or 0 for t in todo) / 1e9:.2f} GB', flush=True)
if dry:
    for t in todo:
        print(t, job(t)[1][-1][:300])
    sys.exit(0)

lock = threading.Lock()
queue = list(todo)
failed = []


def worker():
    while True:
        with lock:
            if not queue or os.path.exists(WS + '/ops/PAUSE'):
                return
            t = queue.pop(0)
        rcf = f'{D}/runs/import-{t}/rc'
        if os.path.exists(rcf) and open(rcf).read().strip() == '0' and os.path.exists(f'{D}/imports/{t}.rrbin'):
            continue
        run, full, mem = job(t)
        if os.path.exists(rcf):
            os.rename(run, f'{run}.failed-{int(time.time())}')
        t0 = time.time()
        rc = subprocess.run(full).returncode
        with lock:
            print(f'{time.strftime("%H:%M:%SZ", time.gmtime())} {t} rc {rc} {time.time() - t0:.0f} s (cap {mem}G)',
                  flush=True)
            if rc != 0:
                failed.append(t)


threads = [threading.Thread(target=worker) for _ in range(par)]
for th in threads:
    th.start()
for th in threads:
    th.join()
print('failed:', failed, flush=True)

# imports.tsv of the stages (only when complete).
rows, missing = [], []
for t in need:
    n = nodes[t]
    if n['kind'] == 'lib14':
        rows.append(f'# {t} not imported (1-4 package owner)')
    elif n['kind'] == 'open':
        rows.append(f"{t}\tOPEN\t{n['graph_key']}\t{n['status']}")
    else:
        js = f'{D}/imports/{t}.json'
        if not os.path.exists(js):
            js = n.get('import_json')
        if js and os.path.exists(js) and os.path.exists(js[:-5] + '.rrbin'):
            rows.append(f"{t}\t{js}\t{js[:-5]}.rrbin")
        else:
            missing.append(t)
tag = ''.join(str(s) for s in sorted(stages))
if missing:
    print('MISSING imports, no imports file:', missing)
    sys.exit(1)
open(f'{plan}/imports.s{tag}.tsv', 'w').write('\n'.join(rows) + '\n')
fam = json.load(open(SC + '/p5l-converter/p-families-1-5.json'))
names = {tp['top'] for tp in tops}
fam['families'] = [f for f in fam['families'] if f['loops'] <= 4 or f['name'] in names]
fam['provenance'] = fam.get('provenance', '') + ' | vk5l-wp6 stages ' + tag + ': 1-4 + ' + ','.join(sorted(names))
json.dump(fam, open(f'{plan}/families.s{tag}.json', 'w'), indent=1)
print(f'wrote {plan}/imports.s{tag}.tsv and families.s{tag}.json ({len(names)} tops)')
