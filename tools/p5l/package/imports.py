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


def job(t):
    n = nodes[t]
    out = f'{D}/imports'
    run = f'{D}/runs/import-{t}'
    tg = targets[t]
    led_lp = json.load(open(WS + '/P5L_STATUS.json'))
    lp = next(x for x in led_lp['nodes'] if x['id'] == t).get('lp')
    if lp is None:
        lp = tg['masters_lp_nosym']
    cmd = [BIN, 'p-import-table', '--table', n['path'], '--graph', tg['edges'], '--ext', f"{tg['S']},{tg['T']}",
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
        run, full, mem = job(t)
        rcf = run + '/rc'
        if os.path.exists(rcf) and open(rcf).read().strip() == '0' and os.path.exists(f'{D}/imports/{t}.rrbin'):
            continue
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
