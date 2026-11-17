# lane arrivals: PARTIAL deliveries via `rr_arrivals arrivals-bin --fast` (one key file per finished parent; T165's top-up parts
# A, B0, B1, B2 each as a file of parent T165, written only when ALL its parts are done). MANIFEST.tsv lines as deliver_partial.py.
# Usage: python3 -I deliver_partial_fast.py <run dir R> <target> <label>
import json, os, subprocess, sys, time
R, target, label = sys.argv[1:4]
S = '/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals'
BIN = S + '/bin/rr_arrivals.a36'
OUT = f'{S}/deliver/{target}.partial'
MAN = OUT + '/MANIFEST.tsv'
listed = {l.split('\t')[0] for l in open(MAN) if l.strip() and not l.startswith('#') and not l.startswith('parent\t')}
plan = [json.loads(l) for l in open(R + '/plan.jsonl.with-T165')]
parents = [o['node'] for o in plan if f'-{label}.edge' in open(o['edge_args']).read()]
def convert(name, src):
    out = f'{OUT}/{name}.keys.bin'
    r = subprocess.run([BIN, 'arrivals-bin', '--fast', '--arrivals', src, '--out', out], capture_output=True, text=True, check=True)
    return json.loads(r.stdout.strip().splitlines()[-1])
new = []
for p in parents:
    if p in listed:
        continue
    topup = os.path.exists(f'{R}/{p}.A') or os.path.exists(f'{R}/{p}.B')
    parts = [d for d in ('A', 'B0', 'B1', 'B2') if os.path.isdir(f'{R}/{p}.{d}')] if topup else ['']
    dirs = [f'{R}/{p}.{d}' if d else f'{R}/{p}' for d in parts]
    if not all(os.path.exists(d + '/done') for d in dirs) or (topup and not os.path.exists(f'{R}/{p}/done')):
        continue
    from concurrent.futures import ThreadPoolExecutor
    jobs = [(f'{p}.{part}' if part else p, f'{d}/out/{p}-{label}.arrivals.jsonl') for d, part in zip(dirs, parts)]
    with ThreadPoolExecutor(max_workers=4) as ex:
        results = list(ex.map(lambda j: convert(*j), jobs))
    for (d, part), st in zip(zip(dirs, parts), results):
        clock = time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())
        name = f'{p}.{part}' if part else p
        ds = sorted(int(x) for x in st['by_depth']) if st['keys'] else []
        note = (f'top-up part {part} of {p} (all parts final; merged by merge_bins: keys union, depth min); ' if part else '') + \
               ('by depth ' + json.dumps(st['by_depth'], separators=(',', ':')) if st['keys'] else 'sends no key (final)')
        line = [p, f'{ds[0]}-{ds[-1]}' if ds else '-', str(st['keys']), st['sha256'] if st['keys'] else '-', clock, 'yes',
                st['out'] if st['keys'] else '-', note]
        with open(MAN, 'a') as fm:
            fm.write('\t'.join(line) + '\n')
        new.append((name, st['keys']))
print(json.dumps({'new': new}))
