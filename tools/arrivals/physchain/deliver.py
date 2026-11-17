# lane arrivals: DELIVERY of the keys arriving at a target node from the physical chain run (coordinator 2026-11-16).
# Usage: python3 -I deliver.py <run dir R> <target> <child label in the edge files> <cones.json> <out dir> [max D+r]
# Streams every finished parent's arrival file R/<P>/out/<P>-<label>.arrivals.jsonl (keys strictly increasing in each file:
# a k-way merge, constant memory), per key: CHAIN DEPTH = min over the parents' lines (each line's depth = min over its
# senders of sender depth + 1), the sending parents (with their depth), total senders. Writes
#   <out>/<target>.keys.bin     magic b"RRKEYS01", u32 LE header length, header JSON, then per key (sorted): arity x i8
#                               (the target's table frame), u8 chain depth, u8 number of sending parents (capped 255)
#   <out>/<target>.sample.jsonl every k-th key (<= 20000 lines; every key when <= 2,000,000 keys: <target>.keys.jsonl)
#   <out>/<target>.stats.json   counts per depth, per D+r, per (depth, D+r), per parent; cone check; sha256 of the files
# D = sum over line slots (k - 1), r = sum over numerator slots (-k); the cone of the target is cones.json's S (D + r <= S).
import hashlib, heapq, json, math, os, struct, sys, collections

R, target, label, cones_p, out = sys.argv[1:6]
max_dr = int(sys.argv[6]) if len(sys.argv) > 6 else None
# partial delivery: only these parents' files (env DELIVER_PARENTS, comma-separated), outputs named DELIVER_NAME
ONLY = set(filter(None, os.environ.get('DELIVER_PARENTS', '').split(',')))
NAME = os.environ.get('DELIVER_NAME') or target
os.makedirs(out, exist_ok=True)
cones = json.load(open(cones_p))
cn = cones['nodes'][target]
frame = cn['frame']
if frame is None:  # a target has no excess file: its frame is its shipped table's header mask
    _m = json.load(open('/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/phys/specs/manifest.json'))[target]
    with open(_m['table']) as _f:
        frame = json.loads(_f.readline())['mask']
S = cn['refined']['S_live'] if cn.get('refined') else cn['S']
lines_m = [c == '1' for c in frame]

def dr(k):
    return sum((x - 1) if l else -x for x, l in zip(k, lines_m))

files = []
for p in sorted(os.listdir(R)):
    f = f'{R}/{p}/out/{p}-{label}.arrivals.jsonl'
    if os.path.isfile(f) and (not ONLY or p in ONLY):
        if not os.path.exists(f'{R}/{p}/done'):
            raise SystemExit(f'{f}: parent {p} not finished')
        files.append((p, f))

def sha(f):
    h = hashlib.sha256()
    with open(f, 'rb') as fh:
        for b in iter(lambda: fh.read(1 << 22), b''):
            h.update(b)
    return h.hexdigest()

def gen(p, f):
    prev = None
    with open(f) as fh:
        for l in fh:
            if '"kind":"arrival"' not in l:
                continue
            v = json.loads(l)
            k = tuple(v['key'])
            if prev is not None and k <= prev:
                raise SystemExit(f'{f}: keys not increasing')
            prev = k
            if v.get('depth') is None:
                raise SystemExit(f'{f}: a line without depth')
            yield k, v['depth'], p, v['senders'], v['first']

arity = len(frame)
hdr = {'schema': 'rustred.campaign.arrival-keys.v1', 'target': target, 'frame': frame, 'arity': arity,
       'record': f'{arity} x i8 key (target table frame), u8 chain depth, u8 sending parents (capped 255)',
       'max_dr': max_dr, 'run': R}
hb = json.dumps(hdr, sort_keys=True).encode()
binp = f'{out}/{NAME}.keys.bin'
tmp = binp + '.partial'
fb = open(tmp, 'wb')
fb.write(b'RRKEYS01' + struct.pack('<I', len(hb)) + hb)
n = 0
by_depth, by_dr, by_both = collections.Counter(), collections.Counter(), collections.Counter()
by_parent, by_parent_min = collections.Counter(), collections.Counter()
outside, max_seen = 0, -1
FULL_MAX = 2_000_000
fj = f'{out}/{NAME}.keys.jsonl'
ffull = open(fj + '.partial', 'w')
fstride = open(f'{out}/{NAME}.stride1000.partial', 'w')
cur = None
def flush(c):
    global n, outside, max_seen
    k, d, ps, sn, first = c
    t = dr(k)
    if max_dr is not None and t > max_dr:
        return
    n += 1
    by_depth[d] += 1; by_dr[t] += 1; by_both[(d, t)] += 1
    for p in ps:
        by_parent[p] += 1
    by_parent_min[first[0]] += 1
    if S is not None and t > S:
        outside += 1
    max_seen = max(max_seen, t)
    fb.write(struct.pack(f'<{arity}b', *k) + bytes([min(d, 255), min(len(ps), 255)]))
    rec = json.dumps({'key': list(k), 'depth': d, 'dr': t, 'senders': sn, 'parents': ps,
                      'first': {'parent': first[0], **first[1]}}, sort_keys=True, separators=(',', ':')) + '\n'
    if n <= FULL_MAX:
        ffull.write(rec)
    if (n - 1) % 1000 == 0:
        fstride.write(rec)
for k, d, p, sn, first in heapq.merge(*(gen(p, f) for p, f in files)):
    if cur is not None and cur[0] == k:
        ps = cur[2]
        if p not in ps:
            ps[p] = d
        else:
            ps[p] = min(ps[p], d)
        best = cur[4] if (cur[1], cur[4][0]) <= (d, p) else (p, first)
        cur = (k, min(cur[1], d), ps, cur[3] + sn, best)
        continue
    if cur is not None:
        flush((cur[0], cur[1], cur[2], cur[3], cur[4]))
    cur = (k, d, {p: d}, sn, (p, first))
if cur is not None:
    flush(cur)
fb.close()
ffull.close(); fstride.close()
os.replace(tmp, binp)
samp = f'{out}/{NAME}.sample.jsonl'
outs = {'bin': {'path': binp, 'sha256': sha(binp), 'bytes': os.path.getsize(binp)}}
if n <= FULL_MAX:
    os.replace(fj + '.partial', fj)
    os.remove(f'{out}/{NAME}.stride1000.partial')
    step = max(1, math.ceil(n / 20000))
    with open(fj) as fi, open(samp, 'w') as fo:
        for i, l in enumerate(fi):
            if i % step == 0:
                fo.write(l)
    outs['jsonl'] = {'path': fj, 'sha256': sha(fj), 'bytes': os.path.getsize(fj), 'keys': n}
else:
    os.remove(fj + '.partial')
    os.replace(f'{out}/{NAME}.stride1000.partial', samp)
    step = 1000
outs['sample'] = {'path': samp, 'sha256': sha(samp), 'every': step}

def layer(t):
    return math.comb(t + arity - 1, arity - 1)
pop = {t: by_dr[t] for t in sorted(by_dr)}
levels = list(range(0, (S if S is not None else max_seen) + 1))
st = {'schema': 'rustred.campaign.arrival-delivery.v1', 'target': target, 'label': label, 'name': NAME, 'parents_only': sorted(ONLY) or None, 'keys': n, 'max_dr_filter': max_dr,
      'files': outs, 'inputs': [{'parent': p, 'file': f, 'sha256': sha(f)} for p, f in files],
      'by_depth': dict(sorted(by_depth.items())), 'by_dr': pop,
      'by_depth_dr': {f'{d}/{t}': c for (d, t), c in sorted(by_both.items())},
      'by_parent': dict(by_parent.most_common()), 'by_first_parent': dict(by_parent_min.most_common()),
      'cone': {'S': S, 'label': f'D + r <= {S}', 'outside': outside, 'max_dr_seen': max_seen,
               'levels_populated': sum(1 for t in levels if by_dr.get(t)), 'levels_in_cone': len(levels),
               'fraction_levels_populated': (sum(1 for t in levels if by_dr.get(t)) / len(levels)) if levels else None,
               'density_per_level': {t: by_dr[t] / layer(t) for t in sorted(by_dr)},
               'note': 'density = keys / top-sector points of the target with that D + r (C(t + n - 1, n - 1), n slots)'}}
json.dump(st, open(f'{out}/{NAME}.stats.json', 'w'), indent=1, sort_keys=True)
print(json.dumps({'target': target, 'keys': n, 'by_depth': st['by_depth'], 'max_dr': max_seen, 'S': S, 'outside': outside,
                  'levels_populated': st['cone']['levels_populated'], 'levels_in_cone': len(levels)}))
