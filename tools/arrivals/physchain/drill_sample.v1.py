# lane arrivals: DRILL walk, the sampled incoming set of a node (sample, not a fixpoint): from the node's incoming arrival files
# (its parents' drill walks), K keys: the highest D + r first (in the node's table frame), ties by a seeded shuffle; per key the
# least depth over the files. Written as an RRKEYS01 key file (key, depth, parents=1) for closure-rows / compute --depth.
# Usage: python3 -I drill_sample.py <node> <incoming list> <K> <frame mask> <out.keys.bin>
import json, random, struct, sys, heapq
x, inc, K, mask, out = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4], sys.argv[5]
lm = [c == '1' for c in mask]
dr = lambda k: sum((v - 1) if l else -v for v, l in zip(k, lm))
best = {}
for f in [l.strip() for l in open(inc) if l.strip()]:
    with open(f) as fh:
        for l in fh:
            if '"kind":"arrival"' not in l:
                continue
            v = json.loads(l)
            k = tuple(v['key']); d = v.get('depth', 0)
            if k not in best or d < best[k]:
                best[k] = d
rng = random.Random(f'drill-sample-{x}')
items = list(best.items())
rng.shuffle(items)
items.sort(key=lambda kv: -dr(kv[0]))
pick = sorted(items[:K])
n = len(mask)
hdr = json.dumps({'schema': 'rustred.campaign.arrival-keys.v1', 'frame': mask, 'arity': n, 'drill_sample': True, 'from_keys': len(best), 'K': K}).encode()
with open(out, 'wb') as fo:
    fo.write(b'RRKEYS01' + struct.pack('<I', len(hdr)) + hdr)
    for k, d in pick:
        fo.write(struct.pack(f'<{n}b', *k) + bytes([min(d, 254), 1]))
print(json.dumps({'node': x, 'incoming_keys': len(best), 'sampled': len(pick), 'max_dr': dr(pick[0][0]) if pick else None}))
