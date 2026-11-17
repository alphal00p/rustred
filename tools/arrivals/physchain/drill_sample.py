# lane arrivals: DRILL walk, the sampled incoming set of a node (sample, not a fixpoint), SAMPLER v2 (2026-11-17T06:1xZ; v1 =
# drill_sample.v1.py held every incoming key in a dict: > 24 GB for T163's ~10^8 incoming keys). From the node's incoming arrival
# files (its parents' drill walks), K keys: the highest D + r first (in the node's table frame), ties by a SEEDED HASH
# (blake2b of seed + key, ascending; v1: seeded shuffle); per key the least depth over the files. Streaming top-K over distinct
# keys, O(K) memory: a key is admitted the first time it is seen if it beats the current K-th best (the threshold only rises,
# so a key not admitted then is never admitted later, and an admitted key sees all its occurrences: depth min exact).
# Fast line parse (sorted-key arrival lines start with {"depth":N, and hold ,"key":[..]); JSON fallback otherwise.
# Written as an RRKEYS01 key file (key, depth, parents=1) for closure-rows / compute --depth.
# Usage: python3 -I drill_sample.py <node> <incoming list> <K> <frame mask> <out.keys.bin>
import hashlib, heapq, json, struct, sys
x, inc, K, mask, out = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4], sys.argv[5]
lm = [c == '1' for c in mask]
seed = f'drill-sample-v2-{x}'.encode()
def dr(k):
    return sum((v - 1) if l else -v for v, l in zip(k, lm))
def h(k):
    return int.from_bytes(hashlib.blake2b(seed + struct.pack(f'<{len(k)}b', *k), digest_size=8).digest(), 'big')
def parse(l):
    if l.startswith('{"depth":'):
        try:
            d = int(l[9:l.index(',', 9)])
            i = l.index(',"key":[') + 8
            return tuple(int(t) for t in l[i:l.index(']', i)].split(',')), d
        except ValueError:
            pass
    v = json.loads(l)
    return tuple(v['key']), v.get('depth', 0) or 0
heap = []   # (dr, -hash, key): the K best so far, heap[0] = the worst of them
best = {}   # key -> least depth (admitted keys only)
lines = 0
for f in [l.strip() for l in open(inc) if l.strip()]:
    with open(f) as fh:
        for l in fh:
            if '"kind":"arrival"' not in l:
                continue
            lines += 1
            k, d = parse(l)
            if k in best:
                if d < best[k]:
                    best[k] = d
                continue
            t = dr(k)
            if len(heap) >= K and t < heap[0][0]:
                continue
            s = (t, -h(k), k)
            if len(heap) < K:
                heapq.heappush(heap, s); best[k] = d
            elif s > heap[0]:
                w = heapq.heapreplace(heap, s); del best[w[2]]; best[k] = d
pick = sorted(best.items())
n = len(mask)
hdr = json.dumps({'schema': 'rustred.campaign.arrival-keys.v1', 'frame': mask, 'arity': n, 'drill_sample': True, 'sampler': 'v2: top-K by (D+r desc, seeded blake2b asc), streaming',
                  'from_lines': lines, 'K': K}).encode()
with open(out, 'wb') as fo:
    fo.write(b'RRKEYS01' + struct.pack('<I', len(hdr)) + hdr)
    for k, d in pick:
        fo.write(struct.pack(f'<{n}b', *k) + bytes([min(d, 254), 1]))
print(json.dumps({'node': x, 'sampler': 'v2', 'incoming_lines': lines, 'sampled': len(pick), 'max_dr': max((dr(k) for k, _ in pick), default=None)}))
