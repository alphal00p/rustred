#!/usr/bin/env python
"""Read-only statistics over the combined four-loop walks (README section 5): per (family, policy, W, host)
the number of runs, the number drained (audit PASS) within their cap, the natives distribution of the
drained runs, the natives at stop of the others with their anchor + 1 signature, and slow-run flags.

usage: ready_stats.py [--root DIR] [--glob PATTERN] [--bd-dir DIR ...] [--slow-factor 3] [--output OUT.json]

Successor of TMP/c4l-s2/ready_stats.py (sha256 3cd4e26c...). Additions of the c4l fix round (2026-09-28):
- signature of each run that did not drain, from a records_breakdown.py output bd-<label>-<family>.json
  (also looked up without the "c4l-4a17f9c7-" label prefix) in any --bd-dir: N993 / N1009 = Apply natives
  of 0111110010 / 0111111001 at rank anchor + 1; "known" iff max(N993, N1009) >= 1,000 (comb_r_test.py);
- slow flag: a drained run whose traversal exceeds --slow-factor x the median traversal of the drained runs
  of its group (at least 3 of them); the recorder fields (user/system seconds, run delay, foreign load) are
  copied next to it so the cause can be read off (c4l-4a17f9c7-ready-w24s1-rep5 is the known case);
- host = socket1 for runs whose first CPU is in 128-255.
Aliases: four-all-r12anchors is four-all (identical query bytes d1ac816e).
"""
import argparse
import glob
import json
import os
import statistics
from collections import defaultdict

ALIAS = {'four-all-r12anchors': 'four-all'}
THRESHOLD = 1000


def last_heartbeat(run):
    best = None
    try:
        with open(os.path.join(run, 'events.jsonl')) as f:
            for line in f:
                if '"heartbeat"' not in line:
                    continue
                try:
                    d = json.loads(line)
                except ValueError:
                    continue
                best = (d.get('elapsed_seconds'), d.get('expanded_nodes'))
    except OSError:
        pass
    return best


def find_bd(bd_dirs, label, family):
    names = [f'bd-{label}-{family}.json', f"bd-{label.replace('c4l-4a17f9c7-', '')}-{family}.json",
             f"bd-{label.replace('c4l-4a17f9c7-', '')}_{family}.json"]
    for d in bd_dirs:
        for n in names:
            p = os.path.join(d, n)
            if os.path.exists(p):
                return p
    return None


def signature(path):
    bd = json.load(open(path))
    got = {}
    for row in bd.get('per_owner_apply') or []:
        if row.get('owner') in ('0111110010', '0111111001') and row.get('helper_rank') is not None:
            got[row['owner']] = (row.get('apply_native_rank_hist') or {}).get(str(row['helper_rank'] + 1), 0)
    n993, n1009 = got.get('0111110010', 0), got.get('0111111001', 0)
    return {'breakdown': path, 'N993': n993, 'N1009': n1009, 'known': max(n993, n1009) >= THRESHOLD}


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--root', default='/common/dev/rustred/TMP/fable51-controls')
    p.add_argument('--glob', default='c4l-4a17f9c7-*/four-all*')
    p.add_argument('--bd-dir', action='append', default=[])
    p.add_argument('--slow-factor', type=float, default=3.0)
    p.add_argument('--output', default='/common/dev/rustred/TMP/c4l-s2/fix/ready_stats.json')
    args = p.parse_args()
    bd_dirs = args.bd_dir or ['/common/dev/rustred/TMP/c4l-s2', '/common/dev/rustred/TMP/c4l-s2/fix']
    groups = defaultdict(list)
    for run in sorted(glob.glob(os.path.join(args.root, args.glob))):
        mpath = os.path.join(run, 'metrics.json')
        if not os.path.exists(mpath):
            continue
        m = json.load(open(mpath))
        label, fam0 = run.rstrip('/').split('/')[-2:]
        fam = ALIAS.get(fam0, fam0)
        first_cpu = int(str(m.get('cpus') or '0').split(',')[0].split('-')[0])
        host = 'socket1' if 128 <= first_cpu <= 255 else 'other'
        apath = os.path.join(run, 'audit.json')
        a = json.load(open(apath)) if os.path.exists(apath) else {}
        drained = a.get('audit') == 'PASS'
        natives = a.get('native_inspections') if drained else (int(m['completed_nodes']) if m.get('completed_nodes') else None)
        stop_s = None
        if not drained:
            hb = last_heartbeat(run)
            if natives is None and hb:
                natives = hb[1]
            stop_s = round(float(m.get('traversal_seconds') or (hb[0] if hb else 0) or 0))
        rec = m.get('recorder') or {}
        row = {'run': f'{label}/{fam0}', 'drained': drained, 'natives': natives, 'cpus': m.get('cpus'),
               'cap_s': m.get('timeout_seconds'), 'stopped_at_s': stop_s, 'stop_reason': m.get('stop_reason'),
               'traversal_s': float(m['traversal_seconds']) if m.get('traversal_seconds') else None,
               'max_rank': m.get('max_scheduled_finite_rank'),
               'instructions_per_native': rec.get('instructions_per_native'), 'ipc': rec.get('ipc'),
               'on_cpu_s': rec.get('on_cpu_seconds'), 'run_delay_s': rec.get('run_delay_seconds'),
               'user_s': rec.get('user_seconds'), 'system_s': rec.get('system_seconds'),
               'foreign': (m.get('foreign_load') or {}).get('foreign_fraction_of_run_cpus')}
        if not drained:
            bd = find_bd(bd_dirs, label, fam0)
            row['signature'] = signature(bd) if bd else None
        groups[(fam, m.get('policy'), m.get('workers'), host)].append(row)
    out = []
    for (fam, pol, w, host), runs in sorted(groups.items(), key=lambda kv: (kv[0][0], str(kv[0][1]), kv[0][2] or 0, kv[0][3])):
        d = sorted(r['natives'] for r in runs if r['drained'])
        trav = [r['traversal_s'] for r in runs if r['drained'] and r['traversal_s']]
        med = statistics.median(trav) if len(trav) >= 3 else None
        slow = [dict(r, median_traversal_s=med) for r in runs
                if med and r['drained'] and r['traversal_s'] and r['traversal_s'] > args.slow_factor * med]
        nd = [r for r in runs if not r['drained']]
        out.append({'family': fam, 'policy': pol, 'workers': w, 'host': host, 'n': len(runs), 'drained': len(d),
                    'drained_natives_min': d[0] if d else None,
                    'drained_natives_median': statistics.median(d) if d else None,
                    'drained_natives_max': d[-1] if d else None,
                    'drained_natives_spread_pct': round(100 * (d[-1] - d[0]) / d[0], 1) if d else None,
                    'median_traversal_s_drained': med, 'slow_runs': slow,
                    'not_drained': [{k: r[k] for k in ('run', 'natives', 'stopped_at_s', 'cap_s', 'stop_reason', 'signature')}
                                    for r in nd],
                    'runs': runs})
    json.dump(out, open(args.output, 'w'), indent=1)
    print('| Family | Policy | W | Host | n | Drained | Natives of drained runs (min / median / max) | Spread | Not drained: natives at stop (N993 / N1009 at anchor+1) | Slow drained runs |')
    print('|---|---|---:|---|---:|---:|---|---:|---|---|')
    for r in out:
        def nd_txt(x):
            sig = x['signature']
            s = f" ({sig['N993']:,} / {sig['N1009']:,})" if sig else ''
            return f"{x['natives']:,}{s}" if x['natives'] is not None else '?'
        dist = (f"{r['drained_natives_min']:,} / {r['drained_natives_median']:,.0f} / {r['drained_natives_max']:,}"
                if r['drained'] else '-')
        sp = f"{r['drained_natives_spread_pct']} %" if r['drained'] else '-'
        slow = '; '.join(f"{s['run']} {s['traversal_s']:.1f} s vs median {s['median_traversal_s']:.1f} s" for s in r['slow_runs']) or '-'
        print(f"| `{r['family']}` | {r['policy']} | {r['workers']} | {r['host']} | {r['n']} | {r['drained']} | {dist} | {sp} | "
              f"{'; '.join(nd_txt(x) for x in r['not_drained']) or '-'} | {slow} |")


if __name__ == '__main__':
    main()
