# lane arrivals: DRILL delivery (sample, not a fixpoint): the drill keys arriving at a target (T162 / T178) in the drill run R
# (run4: start = CELL_TOPS main-line top cells, exact n4), and full CHAINS for a sample of them.
# (1) keys: every arrival line of R/*/out/*-<label>.arrivals.jsonl; per key the least depth (tie: parent name), its parents and
#     that line's first sender -> OUT/<target>.drill.keys.jsonl (+ stats).
# (2) chains for N sampled keys (half the highest D + r in the target frame, half seeded random), hop by hop upwards:
#     at node P with key k (a closure key of P's drill walk):
#       k in P's own top-cell keys (phys/drill3/P/start.keys.jsonl)   -> chain STARTS: top cell (D, r) of P;
#       k in P's sampled incoming keys (R/P/sampled.keys.bin)          -> its arrival line from a grandparent G (least depth): first
#                                                                         sender (G, gk) -> next hop;
#       else k was reached by P's closure                              -> closure path R/P/trace/<k>.json (trace_run_drill.sh)
#                                                                         to its start key s, then s as above.
#     Missing traces are collected per node and run as units (impl-arrivals-drilltrace-<P>, 8 threads, MemoryMax 24G), then the
#     chains are resumed; repeated until every sampled chain starts at a top cell.
# Usage: python3 -I drill_chains.py <R> <target> <label> <OUT dir> <N> [--no-traces]
import json, os, random, struct, subprocess, sys, time, collections, hashlib

R, target, label, OUT, N = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], int(sys.argv[5])
NO_TRACES = '--no-traces' in sys.argv
S = '/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals'
WS = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev'
BIN = S + '/bin/rr_arrivals.a40'
os.makedirs(OUT, exist_ok=True)
plan = {json.loads(l)['node']: json.loads(l) for l in open(R + '/plan.jsonl')}
man = json.load(open(S + '/phys/specs/manifest.json'))
TSV_SHA = json.load(open(S + '/phys/drill3/summary.json'))['tsv_sha256']

def mask_of(x):
    if x in man:
        return json.loads(open(man[x]['table']).readline())['mask']
    return None

def dr(k, mask):
    return sum((v - 1) if c == '1' else -v for v, c in zip(k, mask))

def arrival_lines(lab, keys=None):
    """{key tuple: [(parent, line dict)]} for the arrival files R/*/out/*-<lab>.arrivals.jsonl (keys: restrict to these)."""
    res = collections.defaultdict(list)
    pats = None
    if keys is not None:
        pats = S + f'/physchain/.drillpat.{os.getpid()}'
        open(pats, 'w').write(''.join('"key":' + json.dumps(list(k), separators=(',', ':')) + ',\n' for k in keys))
    for n in sorted(os.listdir(R)):
        f = f'{R}/{n}/out/{n}-{lab}.arrivals.jsonl'
        if not os.path.isfile(f):
            continue
        if pats:
            out = subprocess.run(['grep', '-F', '-f', pats, f], capture_output=True, text=True).stdout.splitlines()
        else:
            out = open(f)
        for l in out:
            if '"kind":"arrival"' not in l:
                continue
            v = json.loads(l)
            res[tuple(v['key'])].append((n, v))
    if pats:
        os.remove(pats)
    return res

own_cache, samp_cache = {}, {}
def own_keys(x):
    if x not in own_cache:
        p = f'{S}/phys/drill3/{x}/start.keys.jsonl'
        own_cache[x] = {tuple(json.loads(l)) for l in open(p)} if os.path.exists(p) else set()
    return own_cache[x]
def sampled_keys(x):
    if x not in samp_cache:
        p = f'{R}/{x}/sampled.keys.bin'
        d = {}
        if os.path.exists(p):
            with open(p, 'rb') as fh:
                assert fh.read(8) == b'RRKEYS01'
                hl = struct.unpack('<I', fh.read(4))[0]
                h = json.loads(fh.read(hl)); n = h['arity']
                b = fh.read()
            for i in range(0, len(b), n + 2):
                d[struct.unpack_from(f'<{n}b', b, i)] = b[i + n]
        samp_cache[x] = d
    return samp_cache[x]
def cell(x, k):
    m = mask_of(x)
    return {'D': sum(v - 1 for v, c in zip(k, m) if c == '1'), 'r': sum(-v for v, c in zip(k, m) if c == '0')}
def first_of(v):
    f = v['first']
    return {'parent_key': f['parent_key'], 'applied': f.get('applied'), 'record_sha256': f.get('record_sha256'),
            'term': f.get('term'), 'term_key': f.get('term_key'), 'by': f.get('by')}

# ---- (1) the drill keys at the target
t0 = time.time()
lines = arrival_lines(label)
keys = sorted(lines)
kp = f'{OUT}/{target}.drill.keys.jsonl'
by_depth, by_dr, by_parent = collections.Counter(), collections.Counter(), collections.Counter()
# the target's frame: the arrival header's child lines (mask in the child frame) from any arrival file
child_mask = None
for n in sorted(os.listdir(R)):
    f = f'{R}/{n}/out/{n}-{label}.arrivals.jsonl'
    if os.path.isfile(f):
        h = json.loads(open(f).readline())
        cl = set(h['child']['lines']); ar = None
        for l in open(f):
            if '"kind":"arrival"' in l:
                ar = len(json.loads(l)['key']); break
        if ar:
            child_mask = ''.join('1' if i in cl else '0' for i in range(ar)); break
with open(kp + '.partial', 'w') as fo:
    for k in keys:
        ls = sorted(lines[k], key=lambda t: (t[1].get('depth', 99), t[0]))
        P, v = ls[0]
        d = v.get('depth'); t = dr(k, child_mask) if child_mask else None
        by_depth[d] += 1; by_dr[t] += 1
        for p in {p for p, _ in ls}:
            by_parent[p] += 1
        fo.write(json.dumps({'key': list(k), 'depth': d, 'dr': t, 'parents': sorted({p for p, _ in ls}), 'from_parent': P,
                             'first': first_of(v)}, separators=(',', ':')) + '\n')
os.replace(kp + '.partial', kp)
print(f'{target}: {len(keys)} drill keys ({time.time() - t0:.0f} s)', flush=True)

# ---- (2) chains for a sample
rng = random.Random(f'drill-chains-{target}')
top = sorted(keys, key=lambda k: (-(dr(k, child_mask) if child_mask else 0), k))[:N - N // 2]
rest = [k for k in keys if k not in set(top)]
samp = sorted(set(top) | set(rng.sample(rest, min(N // 2, len(rest)))))
chains = []
for k in samp:
    ls = sorted(lines[k], key=lambda t: (t[1].get('depth', 99), t[0]))
    P, v = ls[0]
    chains.append({'target': target, 'key': list(k), 'hops': [{'node': target, 'key': list(k), 'depth': v.get('depth'), 'from_parent': P,
                   'parents': sorted({p for p, _ in ls}), 'sender': first_of(v)}], 'at': (P, tuple(v['first']['parent_key'])), 'done': False})

def label_of(x):
    return x

def advance(c):
    """advance chain c as far as possible; returns None when done, or ('trace', P, key) when a closure trace is missing."""
    for _ in range(200):
        P, k = c['at']
        if k in own_keys(P):
            c['hops'].append({'node': P, 'key': list(k), 'start': 'own top-cell drill key (depth 0)', 'cell': cell(P, k)})
            c['done'] = True
            return None
        if k in sampled_keys(P):
            ls = arrival_lines(label_of(P), [k]).get(k, [])
            if not ls:
                c['hops'].append({'node': P, 'key': list(k), 'error': 'sampled key without an arrival line'}); c['done'] = True
                return None
            ls.sort(key=lambda t: (t[1].get('depth', 99), t[0]))
            G, v = ls[0]
            c['hops'].append({'node': P, 'key': list(k), 'depth': v.get('depth'), 'sampled_incoming': True, 'from_parent': G,
                              'parents': sorted({p for p, _ in ls}), 'sender': first_of(v)})
            c['at'] = (G, tuple(v['first']['parent_key']))
            continue
        tf = f'{R}/{P}/trace/' + '_'.join(str(x) for x in k) + '.json'
        if not os.path.exists(tf):
            return ('trace', P, k)
        t = json.load(open(tf))
        if not t.get('path'):
            c['hops'].append({'node': P, 'key': list(k), 'error': 'trace without a path', 'trace': t}); c['done'] = True
            return None
        c['hops'].append({'node': P, 'key': list(k), 'closure_path': t['path'], 'closure_depth': t.get('depth')})
        s = tuple(t['path'][-1]['key'])
        if s == k:
            c['hops'].append({'node': P, 'key': list(k), 'error': 'closure path ends at the key itself (not a start key)'}); c['done'] = True
            return None
        c['at'] = (P, s)
    c['hops'].append({'error': 'hop limit'}); c['done'] = True
    return None

def allowed_cpus():
    held = set(json.load(open(WS + '/ops/FOREIGN_5L_LIFT.held')).get('held', []))
    out = []
    for part in '0-167,176-191,216-257,261,263-270,272-287,320-334,336-383'.split(','):
        a, _, b = part.partition('-'); out += range(int(a), int(b or a) + 1)
    return ','.join(str(c) for c in out if c not in held)

def ledger(msg):
    with open(WS + '/ops/CPU_LEDGER.md', 'a') as f:
        f.write(time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()) + ' (implementer:arrivals): ' + msg + '\n')

it = 0
while True:
    need = collections.defaultdict(set)
    for c in chains:
        if not c['done']:
            r = advance(c)
            if r:
                need[r[1]].add(r[2])
    if not need or NO_TRACES:
        break
    it += 1
    print(f'iteration {it}: traces at {len(need)} nodes ({sum(len(v) for v in need.values())} keys)', flush=True)
    procs = {}
    for P, ks in sorted(need.items()):
        while len(procs) >= 4:
            for q in list(procs):
                if procs[q].poll() is not None:
                    procs.pop(q)
            time.sleep(5)
        kf = f'{R}/{P}/trace/batch-{it}.txt'
        os.makedirs(os.path.dirname(kf), exist_ok=True)
        open(kf, 'w').write(''.join(json.dumps(list(k), separators=(',', ':')) + '\n' for k in sorted(ks)))
        while True:
            av = int(open('/proc/meminfo').read().split('MemAvailable:')[1].split()[0]) // 1048576
            if av - 24 >= 75:
                break
            time.sleep(60)
        ledger(f'unpinned, CPUQuota 800 %, nice 10, MemoryMax 24G (unit impl-arrivals-drilltrace-{P}-{it}: DRILL chain traces, {len(ks)} keys; sample, not a fixpoint)')
        procs[P] = subprocess.Popen(['systemd-run', '--user', '--wait', '--collect', f'--unit=impl-arrivals-drilltrace-{P}-{it}',
                                     '-p', 'CPUQuota=800%', '-p', f'AllowedCPUs={allowed_cpus()}', '-p', 'MemoryMax=24G', '-p', 'MemorySwapMax=0', '-p', 'Nice=10', '--',
                                     'bash', S + '/physchain/trace_run_drill.sh', BIN, R, P, kf],
                                    stdout=open(f'{R}/{P}/trace/batch-{it}.unit.txt', 'w'), stderr=subprocess.STDOUT)
    for q in procs.values():
        q.wait()
    if it > 40:
        break

res = []
for c in chains:
    nodes = [h['node'] for h in c['hops'] if 'from_parent' in h] + [c['hops'][-1].get('node')]
    st = c['hops'][-1]
    res.append({'target': target, 'key': c['key'], 'depth_at_target': c['hops'][0]['depth'], 'complete': c['done'] and 'start' in st,
                'starts_at': {'node': st.get('node'), 'key': st.get('key'), 'cell': st.get('cell')} if 'start' in st else None,
                'chain_nodes': [n for i, n in enumerate(nodes) if i == 0 or n != nodes[i - 1]], 'hops': c['hops']})
cp = f'{OUT}/{target}.drill.chains.json'
json.dump({'schema': 'rustred.campaign.drill-chains.v1', 'label': 'sample, not a fixpoint', 'target': target, 'run': R,
           'start': f'CELL_TOPS.tsv main-line top cells (exact n4, tsv sha256 {TSV_SHA}), 50 seeded keys per cell; 300 sampled incoming keys per node',
           'sample': {'n': len(res), 'rule': f'{N - N // 2} highest D + r + {N // 2} seeded random'}, 'chains': res}, open(cp, 'w'), indent=1)
def sha(f):
    h = hashlib.sha256()
    with open(f, 'rb') as fh:
        for b in iter(lambda: fh.read(1 << 22), b''):
            h.update(b)
    return h.hexdigest()
st = {'schema': 'rustred.campaign.drill-delivery.v1', 'label': 'sample, not a fixpoint', 'target': target, 'run': R, 'keys': len(keys),
      'by_depth': {str(k): v for k, v in sorted(by_depth.items(), key=lambda t: (t[0] is None, t[0]))},
      'by_dr': {str(k): v for k, v in sorted(by_dr.items(), key=lambda t: (t[0] is None, t[0]))}, 'by_parent': dict(by_parent.most_common()),
      'max_dr': max((k for k in by_dr if k is not None), default=None), 'frame': child_mask,
      'chains': {'sampled': len(res), 'complete': sum(1 for r in res if r['complete']), 'trace_iterations': it},
      'files': {'keys': {'path': kp, 'sha256': sha(kp)}, 'chains': {'path': cp, 'sha256': sha(cp)}}}
json.dump(st, open(f'{OUT}/{target}.drill.stats.json', 'w'), indent=1)
print(json.dumps({k: st[k] for k in ('target', 'keys', 'by_depth', 'max_dr', 'chains')}))
