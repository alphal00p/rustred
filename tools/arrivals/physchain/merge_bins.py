# lane arrivals: the FULL arrival list of a target from its per-parent partial key files (deliver_partial.py output, RRKEYS01
# records sorted by the signed key tuple): k-way merge, per key depth = min over the parents' files, parents = number of parents
# whose file holds the key (each per-parent record carries that parent's least depth). Same record format as deliver.py
# (arity x i8 key, u8 depth, u8 sending parents capped 255); stats per depth / D+r / (depth, D+r) / parent; cone check; a jsonl
# sample (every 1000th key: key, depth, dr, parents). Fast: no JSON parsing of the arrival files.
# Usage: python3 -I merge_bins.py <partial dir> <target> <cones.json> <out dir>
import hashlib, heapq, json, math, os, struct, sys, collections

PD, target, cones_p, out = sys.argv[1:5]
man = [l.rstrip('\n').split('\t') for l in open(PD + '/MANIFEST.tsv') if l.strip() and not l.startswith('#') and not l.startswith('parent\t')]
# only lines final for their parent (column 6 = yes): a LOWER-BOUND line (e.g. one top-up part before the others) is skipped
files = [(r[0], r[6]) for r in man if r[2] != '0' and r[5] == 'yes']
zero = [r[0] for r in man if r[2] == '0']
cones = json.load(open(cones_p))
S = cones['nodes'][target]['S']

def read(p, f):
    with open(f, 'rb') as fh:
        assert fh.read(8) == b'RRKEYS01'
        hl = struct.unpack('<I', fh.read(4))[0]
        h = json.loads(fh.read(hl))
        n = h['arity']
        rec = n + 2
        fmt = f'<{n}b'
        while True:
            b = fh.read(rec * 65536)
            if not b:
                break
            for i in range(0, len(b), rec):
                yield struct.unpack_from(fmt, b, i), b[i + n], p

first = files[0][1]
with open(first, 'rb') as fh:
    fh.read(8); hl = struct.unpack('<I', fh.read(4))[0]; h0 = json.loads(fh.read(hl))
arity, frame = h0['arity'], h0['frame']
lm = [c == '1' for c in frame]
hdr = {'schema': 'rustred.campaign.arrival-keys.v1', 'target': target, 'frame': frame, 'arity': arity,
       'record': f'{arity} x i8 key (target table frame), u8 chain depth, u8 sending parents (capped 255)',
       'built_from': PD + '/MANIFEST.tsv (per-parent files merged)'}
hb = json.dumps(hdr, sort_keys=True).encode()
binp = f'{out}/{target}.keys.bin'
fb = open(binp + '.partial', 'wb')
fb.write(b'RRKEYS01' + struct.pack('<I', len(hb)) + hb)
samp = open(f'{out}/{target}.sample.jsonl.partial', 'w')
n = 0; outside = 0; maxdr = -1
by_depth, by_dr, by_both, by_parent = collections.Counter(), collections.Counter(), collections.Counter(), collections.Counter()
pack = struct.Struct(f'<{arity}b').pack
cur = None
def flush(c):
    global n, outside, maxdr
    k, d, ps = c
    t = sum((x - 1) if l else -x for x, l in zip(k, lm))
    fb.write(pack(*k) + bytes([d, min(len(ps), 255)]))
    by_depth[d] += 1; by_dr[t] += 1; by_both[(d, t)] += 1
    if S is not None and t > S:
        outside += 1
    maxdr = max(maxdr, t)
    if n % 1000 == 0:
        samp.write(json.dumps({'key': list(k), 'depth': d, 'dr': t, 'parents': sorted(ps)}, separators=(',', ':')) + '\n')
    n += 1
for k, d, p in heapq.merge(*(read(p, f) for p, f in files), key=lambda r: r[0]):
    by_parent[p] += 1
    if cur is not None and cur[0] == k:
        cur = (k, min(cur[1], d), cur[2] | {p})
        continue
    if cur is not None:
        flush(cur)
    cur = (k, d, {p})
if cur is not None:
    flush(cur)
fb.close(); samp.close()
os.replace(binp + '.partial', binp)
os.replace(f'{out}/{target}.sample.jsonl.partial', f'{out}/{target}.sample.jsonl')
def sha(f):
    hh = hashlib.sha256()
    with open(f, 'rb') as fh:
        for b in iter(lambda: fh.read(1 << 22), b''):
            hh.update(b)
    return hh.hexdigest()
levels = list(range(0, (S if S is not None else maxdr) + 1))
layer = lambda t: math.comb(t + arity - 1, arity - 1)
st = {'schema': 'rustred.campaign.arrival-delivery.v1', 'target': target, 'keys': n,
      'files': {'bin': {'path': binp, 'sha256': sha(binp), 'bytes': os.path.getsize(binp)},
                'sample': {'path': f'{out}/{target}.sample.jsonl', 'sha256': sha(f'{out}/{target}.sample.jsonl'), 'every': 1000}},
      'inputs': [{'parent': p, 'file': f, 'sha256': sha(f)} for p, f in files], 'parents_sending_nothing': zero,
      'by_depth': {str(k): v for k, v in sorted(by_depth.items())}, 'by_dr': {str(k): v for k, v in sorted(by_dr.items())},
      'by_depth_dr': {f'{d}/{t}': c for (d, t), c in sorted(by_both.items())}, 'by_parent': dict(by_parent.most_common()),
      'cone': {'S': S, 'label': f'D + r <= {S}', 'outside': outside, 'max_dr_seen': maxdr,
               'levels_populated': sum(1 for t in levels if by_dr.get(t)), 'levels_in_cone': len(levels),
               'fraction_levels_populated': sum(1 for t in levels if by_dr.get(t)) / len(levels) if levels else None,
               'density_per_level': {str(t): by_dr[t] / layer(t) for t in sorted(by_dr)}}}
json.dump(st, open(f'{out}/{target}.stats.json', 'w'), indent=1, sort_keys=True)
print(json.dumps({'target': target, 'keys': n, 'by_depth': st['by_depth'], 'max_dr': maxdr, 'S': S, 'outside': outside}))
