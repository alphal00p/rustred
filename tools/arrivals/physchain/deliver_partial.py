# lane arrivals: PARTIAL T162 deliveries (coordinator 2026-11-16 ~11:35Z): one file per FINISHED parent of the target, in the format
# of the full delivery (deliver.py with DELIVER_PARENTS=<P>), under S/deliver/<target>.partial/, plus one MANIFEST.tsv line per parent
# (parents already listed are skipped). A parent's file is FINAL ("complete for this parent given its ancestors: yes") because a
# parent is walked only after all of its own ancestors are done and writes its arrival file only at its end. The UNION of the
# partial files is a LOWER bound of the target's fixpoint until every parent of the target is listed.
# Usage: python3 -I deliver_partial.py <run dir R> <target> <label> <cones.json>
import json, os, subprocess, sys, time

R, target, label, cones = sys.argv[1:5]
S = '/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals'
OUT = f'{S}/deliver/{target}.partial'
os.makedirs(OUT, exist_ok=True)
MAN = OUT + '/MANIFEST.tsv'
COLS = ['parent', 'depth_range', 'count', 'sha256_keys_bin', 'clock_utc', 'complete_for_this_parent_given_its_ancestors', 'file', 'note']
if not os.path.exists(MAN):
    with open(MAN, 'w') as f:
        f.write('# PARTIAL arrivals at %s from the physical chain run %s, one line per FINISHED parent. Each file is final for its parent; '
                'the union of all files is a LOWER bound of the %s fixpoint until every parent of %s is listed (see the README).\n'
                % (target, R, target, target))
        f.write('\t'.join(COLS) + '\n')
listed = {l.split('\t')[0] for l in open(MAN) if l.strip() and not l.startswith('#') and not l.startswith('parent\t')}
plan = [json.loads(l) for l in open(R + '/plan.jsonl')]
parents = [o['node'] for o in plan if f'-{label}.edge' in open(o['edge_args']).read()]
new = []
for p in parents:
    if p in listed or not os.path.exists(f'{R}/{p}/done'):
        continue
    f = f'{R}/{p}/out/{p}-{label}.arrivals.jsonl'
    n = 0
    with open(f) as fh:
        for l in fh:
            n += '"kind":"arrival"' in l
    clock = time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())
    if n == 0:
        line = [p, '-', '0', '-', clock, 'yes', '-', 'sends no key to %s (final)' % target]
    else:
        env = dict(os.environ, DELIVER_PARENTS=p, DELIVER_NAME=p)
        subprocess.run(['python3', '-I', f'{S}/physchain/deliver.py', R, target, label, cones, OUT], env=env, check=True,
                       stdout=subprocess.DEVNULL)
        st = json.load(open(f'{OUT}/{p}.stats.json'))
        ds = sorted(int(d) for d in st['by_depth'])
        line = [p, f'{ds[0]}-{ds[-1]}', str(st['keys']), st['files']['bin']['sha256'], clock, 'yes', st['files']['bin']['path'],
                'by depth ' + json.dumps(st['by_depth'], separators=(',', ':')) + f"; max D+r {st['cone']['max_dr_seen']}; outside cone {st['cone']['outside']}"]
    with open(MAN, 'a') as fm:
        fm.write('\t'.join(line) + '\n')
    new.append((p, line[2]))
left = [p for p in parents if not os.path.exists(f'{R}/{p}/done')]
with open(OUT + '/README.txt', 'w') as f:
    f.write(f'PARTIAL deliveries of the keys arriving at {target} (physical chain run {R}).\n'
            f'Format: as S/deliver/MANIFEST.json "format" (RRKEYS01 keys.bin; sample/keys jsonl; stats.json).\n'
            f'Each <parent>.keys.bin is FINAL for that parent (walked after all its ancestors; files written at the end of its walk).\n'
            f'The union is a LOWER BOUND of the {target} fixpoint while parents are missing. Parents of {target} in the plan: {len(parents)}; '
            f'not finished at {time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}: {" ".join(left) or "none"}.\n')
print(json.dumps({'new': new, 'parents': len(parents), 'not_finished': left}))
