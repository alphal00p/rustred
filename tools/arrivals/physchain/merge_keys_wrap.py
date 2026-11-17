# lane arrivals: the FULL arrival list of a target from its per-parent key files via `rr_arrivals merge-keys` (Rust k-way merge;
# records identical to merge_bins.py), plus the same stats.json / sample.jsonl as merge_bins.py.
# Usage: python3 -I merge_keys_wrap.py <partial dir> <target> <cones.json> <out dir>
import hashlib, json, math, os, struct, subprocess, sys
PD, target, cones_p, out = sys.argv[1:5]
S0 = '/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals'
man = [l.rstrip('\n').split('\t') for l in open(PD + '/MANIFEST.tsv') if l.strip() and not l.startswith('#') and not l.startswith('parent\t')]
files = [(r[0], r[6]) for r in man if r[2] != '0' and r[5] == 'yes']
zero = [r[0] for r in man if r[2] == '0']
S = json.load(open(cones_p))['nodes'][target]['S']
binp = f'{out}/{target}.keys.bin'
args = [S0 + '/bin/rr_arrivals.a39', 'merge-keys', '--out', binp, '--cone', str(S)]
for p, f in files:
    args += ['--arrivals', f'{f}={p}']
r = subprocess.run(args, capture_output=True, text=True, check=True)
m = json.loads(r.stdout.strip().splitlines()[-1])
def sha(f):
    h = hashlib.sha256()
    with open(f, 'rb') as fh:
        for b in iter(lambda: fh.read(1 << 22), b''):
            h.update(b)
    return h.hexdigest()
with open(binp, 'rb') as fh:
    assert fh.read(8) == b'RRKEYS01'
    hl = struct.unpack('<I', fh.read(4))[0]; h = json.loads(fh.read(hl))
    n = h['arity']; rec = n + 2; lm = [c == '1' for c in h['frame']]; base = 12 + hl
    samp = f'{out}/{target}.sample.jsonl'
    with open(samp, 'w') as fs:
        for i in range(0, m['keys'], 1000):
            fh.seek(base + i * rec); b = fh.read(rec)
            k = list(struct.unpack(f'<{n}b', b[:n]))
            fs.write(json.dumps({'key': k, 'depth': b[n], 'dr': sum((x - 1) if l else -x for x, l in zip(k, lm)), 'parents': b[n + 1]}, separators=(',', ':')) + '\n')
by_dr = {int(k): v for k, v in m['by_dr'].items()}
levels = list(range(0, (S if S is not None else m['max_dr']) + 1))
layer = lambda t: math.comb(t + n - 1, n - 1)
st = {'schema': 'rustred.campaign.arrival-delivery.v1', 'target': target, 'keys': m['keys'],
      'files': {'bin': {'path': binp, 'sha256': m['sha256'], 'bytes': m['bytes']}, 'sample': {'path': samp, 'sha256': sha(samp), 'every': 1000}},
      'inputs': [{'parent': p, 'file': f} for p, f in files], 'parents_sending_nothing': zero,
      'by_depth': m['by_depth'], 'by_dr': m['by_dr'], 'by_depth_dr': m['by_depth_dr'], 'by_parent_records': m['by_parent'],
      'cone': {'S': S, 'label': f'D + r <= {S}', 'outside': m['outside'], 'max_dr_seen': m['max_dr'],
               'levels_populated': sum(1 for t in levels if by_dr.get(t)), 'levels_in_cone': len(levels),
               'fraction_levels_populated': sum(1 for t in levels if by_dr.get(t)) / len(levels) if levels else None,
               'density_per_level': {str(t): by_dr[t] / layer(t) for t in sorted(by_dr)}},
      'built_by': 'rr_arrivals merge-keys (commit of binary a39) over the per-parent key files of ' + PD + '/MANIFEST.tsv (lines final for their parent)'}
json.dump(st, open(f'{out}/{target}.stats.json', 'w'), indent=1, sort_keys=True)
print(json.dumps({'target': target, 'keys': m['keys'], 'by_depth': m['by_depth'], 'max_dr': m['max_dr'], 'S': S, 'outside': m['outside'], 'sha256': m['sha256']}))
