# lane arrivals: TOP-UP MERGE of two walks of one node X on start sets A and B (each from X's box + a subset of its incoming
# arrival files; A u B = all of them): per edge file, the arrivals of the from-scratch walk on A u B are
#   keys   = keys(A) u keys(B)                 (exact closure = reachability: CL(A u B) = CL(A) u CL(B); per-key decisions
#                                               depend only on (record, key); arrivals are a union over closure keys)
#   depth  = min(depth_A, depth_B)             (depth = min over the start keys reaching a closure key, + 1 per hand-down)
#   first  = the least (parent_key, term) of the two firsts (rows files are sorted by key: row order = key order)
#   senders= senders_A + senders_B             (an UPPER BOUND: closure keys in CL(A) n CL(B) are counted twice)
# Writes R_out/<X>-<child>.arrivals.jsonl (header of A with a "topup" note, lines, a summary line).
# Usage: python3 -I merge_topup.py <dir A/out> <dir B/out> <dir out> <X> [more input dirs ..]   (SHARDS of one walk: senders exact)
import heapq, json, os, sys, hashlib

A, B, O, X = sys.argv[1:5]
INS = [A, B] + sys.argv[5:]
os.makedirs(O, exist_ok=True)

def lines(f):
    if not os.path.exists(f):
        return
    with open(f) as fh:
        for l in fh:
            if '"kind":"arrival"' in l:
                v = json.loads(l)
                yield tuple(v['key']), v

def first_key(v):
    f = v['first']
    return (f['parent_key'], f['term'])

res = {}
for name in sorted(set().union(*[set(os.listdir(d)) for d in INS])):
    if not name.endswith('.arrivals.jsonl') or not name.startswith(X + '-'):
        continue
    fs = [f'{d}/{name}' for d in INS]
    head = None
    for f in fs:
        if os.path.exists(f):
            with open(f) as fh:
                head = json.loads(fh.readline())
            break
    head = dict(head, topup={'inputs': fs, 'rule': 'keys union, depth min, first = least (parent_key, term), senders summed (upper bound)'})
    out = f'{O}/{name}'
    h = hashlib.sha256()
    n = 0
    with open(out + '.partial', 'w') as fo:
        fo.write(json.dumps(head, sort_keys=True, separators=(',', ':')) + '\n')
        cur = None
        def flush(c):
            global n
            s = json.dumps(c, sort_keys=True, separators=(',', ':'))
            fo.write(s + '\n'); h.update((s + '\n').encode()); n += 1
        for k, v in heapq.merge(*(lines(f) for f in fs), key=lambda t: t[0]):
            if cur is not None and tuple(cur['key']) == k:
                cur['depth'] = min(cur['depth'], v['depth'])
                cur['senders'] += v['senders']
                if first_key(v) < first_key(cur):
                    cur['first'] = v['first']
                cur['exact_senders'] = None
                cur['first_exact'] = None
                continue
            if cur is not None:
                flush(cur)
            cur = dict(v)
        if cur is not None:
            flush(cur)
        fo.write(json.dumps({'kind': 'summary', 'keys': n, 'lines_digest': h.hexdigest(), 'topup': True}, sort_keys=True) + '\n')
    os.replace(out + '.partial', out)
    res[name] = n
print(json.dumps(res))
