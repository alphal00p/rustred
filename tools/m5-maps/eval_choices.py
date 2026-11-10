"""Routed terms of the measured traffic under a set of map choices (exact cost model).
usage: eval_choices.py LIB_DIR SLOTS.json CHOICES.json RUN_DIR...   (CHOICES '-' = stored maps)"""
import sys, os, json, collections
sys.path.insert(0, os.path.dirname(__file__))
from mapslib import *
lib, sp, ch = sys.argv[1:4]
runs = sys.argv[4:]
slots = load_slots(sp)
sectors, classes, cpres = load_census(os.path.join(lib, 'census.json'))
choice = {} if ch == '-' else {(c['presentation'], c['support']): c['map'] for c in json.load(open(ch))}
bases = {}
def basis_of(pid):
    if pid not in bases:
        p = slots[pid]; idx = pair_index(p['loops'] + 1)
        bases[pid] = Basis([slot_form(s, idx) for s in p['slots']], p['loops'])
    return bases[pid]
cache = {}
for r in runs:
    cur = new = 0
    for line in open(os.path.join(r, 'routes.tsv')).read().splitlines()[1:]:
        p, sup, cls, num, keys, terms, ms = line.split('\t')
        key = (int(p), int(sup, 16)); num = tuple(int(x) for x in num.split(','))
        cls, t = sectors[key]
        t = choice.get(key, t)
        oid = classes[cls]['owner']['presentation']
        ck = (key, num)
        if ck not in cache:
            ob = basis_of(oid)
            cache[ck] = expand_count([(linear_factor(ob.coords(mapped_slot_form(slots[key[0]]['slots'][j], t, ob.idx))), -e)
                                      for j, e in enumerate(num) if e < 0])
        cur += int(terms); new += int(keys) * cache[ck]
    print(f'{os.path.basename(r)}\t{cur}\t{new}\t{100*(new-cur)/max(cur,1):+.1f} %', flush=True)
