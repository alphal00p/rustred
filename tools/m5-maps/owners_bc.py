"""(2b)/(2c) and ISP closure: per owner presentation, alternative numerator (ISP) completions.

usage: owners_bc.py LIB_DIR ROUTES_A.json SLOTS.json OUT_JSON [TOP_K] [RUN_FILTER]
For each owner (inbound routes aggregated over the runs, or only the runs whose name contains RUN_FILTER):
 - the closure of its current numerator basis under its sector automorphism group (fraction of (g, ISP) pairs
   with g(ISP) = +-one coordinate, g != +-1);
 - (b) hill climbing over completions drawn from the unit products k_i.k_j, k_i.q of its own routing;
 - (c) the same over the edge-pair products p_a.p_b (a, b in its support lines or q: every routing) plus (b)'s pool;
   score = exact routed terms of the measured inbound traffic with the stored route maps, then also with the best
   automorphism composition for the final basis ((a)+(b), (a)+(c)).
Inbound only: the owner's own outbound numerators would change too (not measurable without a rerun) [E].
"""
import sys, os, json, collections, time, itertools
sys.path.insert(0, os.path.dirname(__file__))
from mapslib import *

lib, ra, slots_path, out = sys.argv[1:5]
top_k = int(sys.argv[5]) if len(sys.argv) > 5 else 20
run_filter = sys.argv[6] if len(sys.argv) > 6 else ''
slots = load_slots(slots_path)
sectors, classes, cpres = load_census(os.path.join(lib, 'census.json'))
routes = json.load(open(ra))['routes']

# inbound traffic per owner: list of (src pid, numerator, keys, map)
traffic = {}
for line_run in []:
    pass
inbound = collections.defaultdict(list)
weights = collections.Counter()
# we need per-numerator keys: re-read from routes.tsv files listed in ROUTES_A? routes_a stores aggregate only,
# so owners_bc takes the per-numerator traffic from the RUN dirs in env M5_RUNS (colon separated).
runs = [r for r in os.environ['M5_RUNS'].split(':') if r and run_filter in os.path.basename(r.rstrip('/'))]
for r in runs:
    for line in open(os.path.join(r, 'routes.tsv')).read().splitlines()[1:]:
        p, sup, cls, num, keys, terms, ms = line.split('\t')
        key = (int(p), int(sup, 16))
        traffic.setdefault(key, collections.Counter())[tuple(int(x) for x in num.split(','))] += int(keys)
for key, nums in traffic.items():
    cls, t = sectors[key]
    o = classes[cls]['owner']
    oid = (o['presentation'], o['support'])
    for num, k in nums.items():
        inbound[oid].append((key[0], num, k, t))
        weights[oid] += k


def free_slots(p, support):
    return [i for i, s in enumerate(p['slots']) if not ('line' in s and support >> i & 1)]


def count_terms(owner_forms, L, items, mapper=None):
    B = Basis(owner_forms, L)
    if not B.ok:
        return None
    tot = 0
    cache = {}
    for src, num, k, t in items:
        tt = mapper(src, t) if mapper else t
        ck = (src, num, id(tt) if mapper else None)
        fs = []
        for j, e in enumerate(num):
            if e < 0:
                v = mapped_slot_form(slots[src]['slots'][j], tt, B.idx)
                fs.append((linear_factor(B.coords(v)), -e))
        tot += k * expand_count(fs)
    return tot


def closure(p, support, forms_free, free_idx, G, L):
    """Fraction of (g, free coordinate) with g(form) = +-single coordinate (no constant), g != +-identity."""
    idx = pair_index(L + 1)
    allforms = [slot_form(s, idx) for s in p['slots']]
    for pos, f in zip(free_idx, forms_free):
        allforms[pos] = f
    B = Basis(allforms, L)
    if not B.ok:
        return None
    hit = tot = 0
    for g in G:
        if g['loop_linear'] == identity(L)['loop_linear'] and not any(g['loop_external']):
            continue
        for pos in free_idx:
            f = allforms[pos]
            # image of the form under g: substitute rows. Build form from its pair coefficients: f(v) = v^T S v.
            img = transform_form(f, g, L, idx)
            co = B.coords(img)
            nz = [(i, c) for i, c in enumerate(co) if c != 0]
            tot += 1
            hit += len(nz) == 1 and nz[0][0] != len(co) - 1 and abs(nz[0][1]) == 1
    return hit / tot if tot else 1.0


def transform_form(f, g, L, idx):
    """f as a pair vector over (k, q); the form f(v M) with M the row action of g."""
    n = L + 1
    M = [[0] * n for _ in range(n)]
    for i in range(L):
        for j in range(L):
            M[i][j] = g['loop_linear'][i * L + j]
        M[i][L] = g['loop_external'][i]
    M[L][L] = g['external_sign']
    # f(v) = sum_{a<=b} c_ab v_a v_b ; v' = v M => v'_a = sum_i v_i M[i][a]
    out = [0] * len(idx)
    for (a, b), pos in idx.items():
        c = f[pos]
        if not c:
            continue
        for i in range(n):
            if M[i][a] == 0:
                continue
            for j in range(n):
                if M[j][b] == 0:
                    continue
                out[idx[(min(i, j), max(i, j))]] += c * M[i][a] * M[j][b]
    return out


def pools(p, support, L):
    idx = pair_index(L + 1)
    unit = lambda i: [int(k == i) for k in range(L + 1)]
    pb = []
    for i in range(L):
        for j in range(i, L + 1):
            pb.append(form(unit(i), unit(j), idx))
    rows = [p['slots'][i]['line'] for i in line_slots(p) if support >> i & 1] + [unit(L)]
    pc = list(pb)
    for a, b in itertools.combinations(range(len(rows)), 2):
        pc.append(form(rows[a], rows[b], idx))
    for i in free_slots(p, support):
        pc.append(slot_form(p['slots'][i], idx))
    # dedupe up to sign
    def dd(lst):
        seen, outl = set(), []
        for f in lst:
            k1, k2 = tuple(f), tuple(-x for x in f)
            if not any(f) or k1 in seen or k2 in seen:
                continue
            seen.add(k1); outl.append(f)
        return outl
    return dd(pb), dd(pc)


def climb(p, support, L, items, pool, budget_s):
    idx = pair_index(L + 1)
    base = [slot_form(s, idx) for s in p['slots']]
    free_idx = free_slots(p, support)
    cur = [base[i] for i in free_idx]
    def score(free):
        forms = list(base)
        for pos, f in zip(free_idx, free):
            forms[pos] = f
        return count_terms(forms, L, items)
    best = score(cur)
    start = best
    t0 = time.time()
    improved = True
    while improved and time.time() - t0 < budget_s:
        improved = False
        for k in range(len(cur)):
            for f in pool:
                if any(tuple(f) == tuple(x) or tuple(f) == tuple(-y for y in x) for x in cur):
                    continue
                trial = cur[:k] + [f] + cur[k + 1:]
                s = score(trial)
                if s is not None and s < best:
                    best, cur, improved = s, trial, True
                if time.time() - t0 > budget_s:
                    break
    return start, best, cur, free_idx


order = sorted(weights, key=lambda o: -sum(k * 1 for _, _, k, _ in inbound[o]))
# rank owners by current inbound terms
cur_terms = {}
for oid in weights:
    p = slots[oid[0]]
    idx = pair_index(p['loops'] + 1)
    cur_terms[oid] = count_terms([slot_form(s, idx) for s in p['slots']], p['loops'], inbound[oid])
order = sorted(cur_terms, key=lambda o: -cur_terms[o])
res = []
t_all = time.time()
for rank, oid in enumerate(order):
    p = slots[oid[0]]
    L = p['loops']
    G = automorphisms(p, oid[1])
    idx = pair_index(L + 1)
    free_idx = free_slots(p, oid[1])
    rec = {'owner_presentation': oid[0], 'owner_support': oid[1], 'name': cpres[oid[0]]['name'],
           'kind': p['kind'], 'group_order': len(G), 'free': len(free_idx), 'inbound_keys': weights[oid],
           'inbound_terms': cur_terms[oid],
           'closure_current': closure(p, oid[1], [slot_form(p['slots'][i], idx) for i in free_idx], free_idx, G, L)}
    if rank < top_k:
        items = sorted(inbound[oid], key=lambda x: -x[2])
        pb, pc = pools(p, oid[1], L)
        sb, bb, fb, _ = climb(p, oid[1], L, items, pb, 240)
        sc, bc, fc, _ = climb(p, oid[1], L, items, pc, 480)
        rec.update({'b_start': sb, 'b_best': bb, 'c_start': sc, 'c_best': bc,
                    'closure_b': closure(p, oid[1], fb, free_idx, G, L),
                    'closure_c': closure(p, oid[1], fc, free_idx, G, L),
                    'c_forms': fc, 'b_forms': fb})
        print(rank, rec['name'][:30], 'cur', cur_terms[oid], 'b', bb, 'c', bc, 'closure', rec['closure_current'],
              '->', rec['closure_c'], round(time.time() - t_all), file=sys.stderr, flush=True)
    res.append(rec)
json.dump(res, open(out, 'w'))
