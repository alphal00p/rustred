"""End-to-end check of the table -> payload converter on campaign nodes.

Usage: python3 -I e2e.py RUSTRED_BIN OUT_DIR TID [TID ...] [--import-only] [--dots R --num R --eps-span R]

For each node: table = the ledger's run_dir (assembled/ for repaired nodes), graph/S/T/LP from rules_study/targets.tsv;
seeds (table slot order) = the corner of every chart, one dot on each line, one unit of numerator on each ISP, and on
each eps line the powers 0 and 2; `rustred p-import-table --replay` converts, verifies and replays; laporta_check.py
compares at d = 37/7, -11/5, 9/13 (mod 2^61-1; exact over Q for arity <= 9).
Slot order: the census family rules_study/classify/families/<TID>.sym and the run's ORDER-MAP.json (if any) are passed,
so the payload records the table's slot order relative to the census node and every payload read enforces it.
"""
import csv
import json
import os
import subprocess
import sys
import time

WS = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev'
HERE = os.path.dirname(os.path.abspath(__file__))


def table_of(node):
    rd = node['run_dir']
    for p in (os.path.join(rd, 'F.table.jsonl'), os.path.join(rd, 'ASSEMBLED.table.jsonl'),
              os.path.join(rd, 'assembled', 'F.table.jsonl')):
        if os.path.exists(p):
            return p
    raise SystemExit('no table for %s in %s' % (node['id'], rd))


def seeds_of(table):
    with open(table) as f:
        header = json.loads(f.readline())
    mask = header['mask']
    shifts = header['power_shifts']
    N = len(mask)
    eps = [s != '0' for s in shifts]
    seeds = []
    for chart in header['charts']:
        corner = [1 if c == '1' else 0 for c in chart]
        seeds.append(corner)
        for a in range(N):
            if mask[a] == '1' and not eps[a]:
                k = list(corner); k[a] += 1; seeds.append(k)
            elif mask[a] == '0':
                k = list(corner); k[a] -= 1; seeds.append(k)
    for a in range(N):
        if eps[a]:
            for v in (0, 2, -1):
                k = [1 if mask[b] == '1' else 0 for b in range(N)]; k[a] = v
                if k not in seeds:
                    seeds.append(k)
    return seeds, N


def main():
    args = sys.argv[1:]
    import_only = '--import-only' in args
    opts = []
    for flag in ('--dots', '--num', '--eps-span'):
        if flag in args:
            i = args.index(flag); opts += [flag, args[i + 1]]; del args[i:i + 2]
    args = [a for a in args if a != '--import-only']
    binary, out, tids = args[0], args[1], args[2:]
    os.makedirs(out, exist_ok=True)
    ledger = {n['id']: n for n in json.load(open(WS + '/P5L_STATUS.json'))['nodes']}
    rows = {r['id']: r for r in csv.DictReader(open(WS + '/rules_study/targets.tsv'), delimiter='\t')}
    for tid in tids:
        node, row = ledger[tid], rows[tid]
        table = table_of(node)
        seeds, N = seeds_of(table)
        seeds_path = os.path.join(out, tid + '.seeds.json')
        json.dump(seeds, open(seeds_path, 'w'))
        cmd = [binary, 'p-import-table', '--table', table, '--graph', row['edges'], '--ext',
               '%s,%s' % (row['S'], row['T']), '--label', tid, '--lp', str(node.get('lp') or row['masters_lp_nosym']),
               '--out', out, '--force', '--replay', seeds_path,
               '--census-family', os.path.join(WS, 'rules_study/classify/families/%s.sym' % tid)]
        order_map = os.path.join(node['run_dir'], 'ORDER-MAP.json')
        if os.path.exists(order_map):
            cmd += ['--order-map', order_map]
        t0 = time.time()
        r = subprocess.run(cmd, capture_output=True, text=True)
        print(json.dumps(dict(tid=tid, step='import', rc=r.returncode, seconds=round(time.time() - t0, 2),
                              stdout=r.stdout.strip()[-600:], stderr=r.stderr.strip()[-1500:])), flush=True)
        if r.returncode != 0 or import_only:
            continue
        lap = [sys.executable, '-I', os.path.join(HERE, 'laporta_check.py'), table, out, tid] + opts
        if N <= 9:
            lap.append('--exact')
        t0 = time.time()
        r = subprocess.run(lap, capture_output=True, text=True)
        open(os.path.join(out, tid + '.laporta.jsonl'), 'w').write(r.stdout)
        last = r.stdout.strip().splitlines()[-1] if r.stdout.strip() else ''
        print(json.dumps(dict(tid=tid, step='laporta', rc=r.returncode, seconds=round(time.time() - t0, 2),
                              summary=last, stderr=r.stderr.strip()[-800:])), flush=True)


if __name__ == '__main__':
    main()
