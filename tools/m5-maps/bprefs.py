"""(b) for chosen owners, jointly with (a): hill-climb the owner's unit-row ISP completion (k_i.k_j, k_i.q of its own
routing, the presentation contract kept: no k_0^2) on the measured inbound traffic, each route scored with its best
owner-automorphism composition. Writes the RUSTRED_P_ISP_PREFERENCE table (family name -> coordinate ordinals).

usage: M5_RUNS=run1:run2.. bprefs.py LIB_DIR SLOTS.json OUT_PREF.json OUT_REPORT.json PID:SUPPORT_HEX [...]"""
import sys, os, json, collections, time
sys.path.insert(0, os.path.dirname(__file__))
from mapslib import *

lib, sp, out_pref, out_rep = sys.argv[1:5]
owners = [(int(a), int(b, 16)) for a, b in (x.split(':') for x in sys.argv[5:])]
slots = load_slots(sp)
sectors, classes, cpres = load_census(os.path.join(lib, 'census.json'))
traffic = collections.defaultdict(collections.Counter)
for r in [r for r in os.environ['M5_RUNS'].split(':') if r]:
    for line in open(os.path.join(r, 'routes.tsv')).read().splitlines()[1:]:
        p, sup, cls, num, keys, terms, ms = line.split('\t')
        traffic[(int(p), int(sup, 16))][tuple(int(x) for x in num.split(','))] += int(keys)
pref, rep = {}, []
for oid in owners:
    p = slots[oid[0]]; L = p['loops']; idx = pair_index(L + 1)
    G = automorphisms(p, oid[1])
    items = []
    for key, nums in traffic.items():
        cls, t = sectors[key]
        o = classes[cls]['owner']
        if (o['presentation'], o['support']) == oid:
            items.append((key[0], t, dict(nums)))
    base = [slot_form(s, idx) for s in p['slots']]
    free = [i for i, s in enumerate(p['slots']) if 'isp' in s]
    unit = lambda i: [int(k == i) for k in range(L + 1)]
    pool = []  # (form, ordinal)
    o = 0
    for i in range(L):
        for j in range(i, L):
            pool.append((form(unit(i), unit(j), idx), o)); o += 1
    for i in range(L):
        pool.append((form(unit(i), unit(L), idx), o)); o += 1
    def ordinal(f):
        for g, k in pool:
            if g == f:
                return k
        return None
    def total(fr, joint=True):
        forms = list(base)
        for pos, f in zip(free, fr):
            forms[pos] = f
        B = Basis(forms, L)
        if not B.ok:
            return None
        tot = 0
        for src, t, nums in items:
            best = None
            for m in (G if joint else [identity(L)]):
                t2 = compose(t, m)
                s = 0
                for num, k in nums.items():
                    s += k * expand_count([(linear_factor(B.coords(mapped_slot_form(slots[src]['slots'][j], t2, B.idx))), -e)
                                           for j, e in enumerate(num) if e < 0])
                    if best is not None and s >= best:
                        break
                best = s if best is None else min(best, s)
            tot += best
        return tot
    cur = [base[i] for i in free]
    start_plain, start_a = total(cur, False), total(cur, True)
    best = start_a
    t0 = time.time(); improved = True
    while improved and time.time() - t0 < 1500:
        improved = False
        for k in range(len(cur)):
            for f, _ in pool:
                if any(f == x for x in cur):
                    continue
                trial = cur[:k] + [f] + cur[k + 1:]
                s = total(trial)
                if s is not None and s < best:
                    best, cur, improved = s, trial, True
    ords = [ordinal(f) for f in cur]
    name = cpres[oid[0]]['name']
    pref[name] = ords
    r = {'owner': oid, 'name': name, 'G': len(G), 'routes': len(items), 'terms_stored': start_plain,
         'terms_a': start_a, 'terms_a_b': best, 'isp_ordinals': ords,
         'old_ordinals': [ordinal(base[i]) for i in free]}
    rep.append(r)
    print(json.dumps(r), file=sys.stderr, flush=True)
    json.dump(pref, open(out_pref, 'w')); json.dump(rep, open(out_rep, 'w'))
