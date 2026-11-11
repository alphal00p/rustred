"""(1)+(2a): per-route measured traffic and the automorphism-composition optimum.

usage: routes_a.py LIB_DIR RUN_DIR [RUN_DIR...] OUT_JSON
  RUN_DIR holds routes.tsv (census) and summary.json; slots.json is read from the first RUN_DIR that has it.
For every route (source presentation, support) -> owner, every owner-sector automorphism M is tried:
T' = T M; terms are exact expansion counts per numerator vector, weighted by the keys measured.
Calibration: the count under the stored T must equal terms/keys of the census.
"""
import sys, json, os, collections, time
sys.path.insert(0, os.path.dirname(__file__))
from mapslib import *

lib, runs, out = sys.argv[1], sys.argv[2:-1], sys.argv[-1]
sectors, classes, cpres = load_census(os.path.join(lib, 'census.json'))
slots = None
for r in runs:
    if os.path.exists(os.path.join(r, 'slots.json')):
        slots = load_slots(os.path.join(r, 'slots.json'))
        break
assert slots is not None

# traffic per route per numerator, per run
traffic = collections.defaultdict(lambda: collections.defaultdict(lambda: collections.Counter()))
meas = {}
for r in runs:
    name = os.path.basename(r.rstrip('/'))
    for line in open(os.path.join(r, 'routes.tsv')).read().splitlines()[1:]:
        p, sup, cls, num, keys, terms, ms = line.split('\t')
        key = (int(p), int(sup, 16))
        num = tuple(int(x) for x in num.split(','))
        traffic[key][num][name] += int(keys)
        meas[(key, num, name)] = (int(keys), int(terms), float(ms))

bases = {}
def basis_of(pid):
    if pid not in bases:
        p = slots[pid]
        idx = pair_index(p['loops'] + 1)
        bases[pid] = Basis([slot_form(s, idx) for s in p['slots']], p['loops'])
    return bases[pid]

autos = {}
def autos_of(pid, support):
    k = (pid, support)
    if k not in autos:
        autos[k] = automorphisms(slots[pid], support)
    return autos[k]

def factors(src, num, t, ob):
    idx = ob.idx
    fs = []
    for j, e in enumerate(num):
        if e < 0:
            v = mapped_slot_form(src['slots'][j], t, idx)
            fs.append((linear_factor(ob.coords(v)), -e))
    return fs

cache = {}
def count(src_id, num, t, owner_id):
    key = (src_id, num, json.dumps(t, sort_keys=True), owner_id)
    if key not in cache:
        cache[key] = expand_count(factors(slots[src_id], num, t, basis_of(owner_id)))
    return cache[key]

result = []
calib_bad = 0
t0 = time.time()
for (pid, sup), nums in sorted(traffic.items()):
    cls, t = sectors[(pid, sup)]
    owner = classes[cls]['owner']
    oid, osup = owner['presentation'], owner['support']
    G = autos_of(oid, osup)
    rec = {'presentation': pid, 'support': sup, 'class': cls, 'owner_presentation': oid, 'owner_support': osup,
           'src_node': cpres[pid]['node'], 'owner_node': classes[cls]['node'], 'group_order': len(G),
           'src_name': cpres[pid]['name'], 'owner_name': cpres[oid]['name'], 'per_run': {}}
    # calibration + current
    for num, per in nums.items():
        c = count(pid, num, t, oid)
        for run, k in per.items():
            mk, mt, ms = meas[((pid, sup), num, run)]
            if c * mk != mt:
                calib_bad += 1
                if calib_bad < 10:
                    print('CALIB MISMATCH', pid, sup, num, run, c, mk, mt, file=sys.stderr)
    best = None
    cand = []
    for gi, m in enumerate(G):
        t2 = compose(t, m)
        tot = collections.Counter()
        for num, per in nums.items():
            c = count(pid, num, t2, oid)
            for run, k in per.items():
                tot[run] += c * k
        cand.append((sum(tot.values()), gi, dict(tot)))
    cur = cand[0] if G and G[0]['loop_linear'] == identity(len(G[0]['loop_external']))['loop_linear'] and not any(G[0]['loop_external']) else None
    # current = identity composition
    tot_cur = collections.Counter()
    by_rank = collections.defaultdict(lambda: [0, 0])
    for num, per in nums.items():
        c = count(pid, num, t, oid)
        rank = -sum(num)
        for run, k in per.items():
            tot_cur[run] += c * k
            by_rank[rank][0] += k
            by_rank[rank][1] += c * k
    bestc = min(cand, key=lambda x: (x[0], x[1]))
    rec['keys'] = {run: sum(per.get(run, 0) for per in nums.values()) for run in set(r for per in nums.values() for r in per)}
    rec['ms'] = {}
    for num, per in nums.items():
        for run in per:
            rec['ms'][run] = rec['ms'].get(run, 0) + meas[((pid, sup), num, run)][2]
    rec['terms_current'] = dict(tot_cur)
    rec['terms_best_a'] = bestc[2]
    best_m = G[bestc[1]]
    rec['best_map'] = compose(t, best_m)
    rec['best_perm'] = {str(k): v for k, v in best_m.get('perm', {}).items()}
    rec['by_rank'] = {str(k): v for k, v in sorted(by_rank.items())}
    rec['numerators'] = len(nums)
    result.append(rec)

print('routes', len(result), 'calib mismatches', calib_bad, 'seconds', round(time.time() - t0, 1), file=sys.stderr)
json.dump({'routes': result, 'calib_mismatches': calib_bad}, open(out, 'w'))
