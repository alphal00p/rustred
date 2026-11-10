"""Choose, for every routed census sector, the automorphism-composed map T M with the fewest routed terms.

usage: choose_maps.py LIB_DIR SLOTS.json MODE OUT_CHOICES.json [RUN_DIR...]
MODE = traffic : weight = measured traffic of the RUN_DIRs (sectors without traffic keep their map)
       static  : traffic-free score for every sector: sum over the source's numerator slots j of the expansion
                 terms of F_j and F_j^2 (rank 1 and 2 seeds), then the Forcer proxy; ties keep the stored map
       hybrid  : traffic where measured, static elsewhere
Writes only the sectors whose map changes (plus a summary on stderr).
"""
import sys, os, json, collections, time
sys.path.insert(0, os.path.dirname(__file__))
from mapslib import *

lib, slots_path, mode, out = sys.argv[1:5]
runs = sys.argv[5:]
slots = load_slots(slots_path)
sectors, classes, cpres = load_census(os.path.join(lib, 'census.json'))
traffic = collections.defaultdict(collections.Counter)
for r in runs:
    for line in open(os.path.join(r, 'routes.tsv')).read().splitlines()[1:]:
        p, sup, cls, num, keys, terms, ms = line.split('\t')
        traffic[(int(p), int(sup, 16))][tuple(int(x) for x in num.split(','))] += int(keys)

bases, autos = {}, {}
def basis_of(pid):
    if pid not in bases:
        p = slots[pid]; idx = pair_index(p['loops'] + 1)
        bases[pid] = Basis([slot_form(s, idx) for s in p['slots']], p['loops'])
    return bases[pid]

def lin(src, j, t, ob):
    return linear_factor(ob.coords(mapped_slot_form(slots[src]['slots'][j], t, ob.idx)))

def traffic_score(src, nums, t, ob):
    tot = 0
    for num, k in nums.items():
        tot += k * expand_count([(lin(src, j, t, ob), -e) for j, e in enumerate(num) if e < 0])
    return tot

def static_score(src, sup, t, ob):
    p = slots[src]
    js = [j for j, s in enumerate(p['slots']) if not ('line' in s and sup >> j & 1)]
    lins = [lin(src, j, t, ob) for j in js]
    s1 = sum(len(l) for l in lins)
    s2 = sum(expand_count([(l, 2)]) for l in lins)
    return (s1 + s2, sum(len(l) ** 2 for l in lins))

choices = []
stat = collections.Counter()
t0 = time.time()
for (pid, sup), (cls, t) in sorted(sectors.items()):
    o = classes[cls]['owner']
    if (pid, sup) == (o['presentation'], o['support']):
        continue
    oid, osup = o['presentation'], o['support']
    if (oid, osup) not in autos:
        autos[(oid, osup)] = automorphisms(slots[oid], osup)
    G = autos[(oid, osup)]
    if len(G) <= 1:
        stat['trivial_group'] += 1
        continue
    ob = basis_of(oid)
    use_traffic = mode == 'traffic' or (mode == 'hybrid' and (pid, sup) in traffic)
    if mode == 'traffic' and (pid, sup) not in traffic:
        continue
    best, bestscore, curscore = None, None, None
    for gi, m in enumerate(G):
        t2 = compose(t, m)
        sc = traffic_score(pid, traffic[(pid, sup)], t2, ob) if use_traffic else static_score(pid, sup, t2, ob)
        if t2['loop_linear'] == t['loop_linear'] and t2['loop_external'] == t['loop_external'] and t2['external_sign'] == t['external_sign']:
            curscore = sc
        if bestscore is None or sc < bestscore:
            best, bestscore = t2, sc
    if curscore is not None and bestscore < curscore:
        choices.append({'presentation': pid, 'support': sup, 'map': {k: best[k] for k in
                        ('loops_in', 'loops_out', 'loop_linear', 'loop_external', 'external_sign')},
                        'score_before': curscore, 'score_after': bestscore})
        stat['changed'] += 1
    else:
        stat['kept'] += 1
print(dict(stat), 'seconds', round(time.time() - t0), file=sys.stderr)
json.dump(choices, open(out, 'w'))
