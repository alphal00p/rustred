"""R5a: choose ISP bases of generic-lane presentations (Insertion/SP, Triangle, Carpet) to minimise routed terms,
inbound AND outbound, weighted by the measured route traffic (P_CENSUS_SLOTS runs), coordinate descent over owners.

usage: basis_opt.py LIB OUT_DIR RUN_DIR...   (env: RB_TOP owners to optimise (default 40), RB_POOL=pairs|units|both,
       RB_PASSES (2), RB_FREE (comma list of presentation ids to restrict))
Cost model [exact, calibrated]: terms(route item) = distinct monomials of prod_j F_j(T k)^e_j in owner coordinates.
Search proxy: prod_j C(n_j + e_j - 1, e_j), n_j = terms of the image of source slot j (exact when supports are disjoint).
Outbound proxy: when an owner's ISP slot changes, its outbound keys keep their exponent vectors (same shape) [E].
"""
import sys, os, json, collections, itertools, time, math
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'm5-maps'))
from mapslib import *

lib, outdir, runs = sys.argv[1], sys.argv[2], sys.argv[3:]
os.makedirs(outdir, exist_ok=True)
TOP = int(os.environ.get('RB_TOP', 40)); POOL = os.environ.get('RB_POOL', 'both'); PASSES = int(os.environ.get('RB_PASSES', 2))
RESTRICT = set(int(x) for x in os.environ.get('RB_FREE', '').split(',') if x)
sectors, classes, cpres = load_census(os.path.join(lib, 'census.json'))
slots = load_slots(os.path.join(runs[0], 'slots.json'))
lane = {}
for r in runs:
    for l in open(os.path.join(r, 'owners.tsv')).read().splitlines()[1:]:
        f = l.split('\t'); lane[int(f[2])] = f[1]
log = open(os.path.join(outdir, 'log.txt'), 'a')
def say(*a):
    s = ' '.join(str(x) for x in a); print(s, file=sys.stderr, flush=True); log.write(s + '\n'); log.flush()

# ---- traffic: route (p, sup) -> list of (exponent tuple ((j, e), ...), keys); measured terms
traffic = collections.defaultdict(collections.Counter); measured = collections.Counter()
for r in runs:
    for l in open(os.path.join(r, 'routes.tsv')).read().splitlines()[1:]:
        p, sup, cl, num, k, t, ms = l.split('\t'); key = (int(p), int(sup, 16))
        ex = tuple((j, -int(x)) for j, x in enumerate(num.split(',')) if int(x) < 0)
        traffic[key][ex] += int(k); measured[key] += int(t)
routes = {}
for key in traffic:
    cls, t = sectors[key]; o = classes[cls]['owner']['presentation']
    routes[key] = (o, t)
# ---- presentations: current forms
def isp_lr(s): return s['isp']
forms0 = {}
for pid, p in slots.items():
    idx = pair_index(p['loops'] + 1); forms0[pid] = [slot_form(s, idx) for s in p['slots']]
cur = {pid: list(f) for pid, f in forms0.items()}       # current forms (mutable per free owner)
cur_lr = {pid: [s.get('isp') for s in p['slots']] for pid, p in slots.items()}
nlines = {pid: sum(1 for s in p['slots'] if 'line' in s) for pid, p in slots.items()}
basis_cache = {}
def basis(pid, forms=None):
    f = forms if forms is not None else cur[pid]
    k = (pid, tuple(map(tuple, f)))
    if k not in basis_cache:
        basis_cache[k] = Basis(f, slots[pid]['loops'])
    return basis_cache[k]

# ---- image term counts
def image_n(src_form_vec, t, src_loops, B):
    return src_form_vec
def mapped_form(pid, j, t, idx, forms=None):
    p = slots[pid]; s = p['slots'][j]
    if 'line' in s:
        r = map_row(s['line'], t); return form(r, r, idx)
    l, r = (forms or cur_lr[pid])[j]
    return form(map_row(l, t), map_row(r, t), idx)
def nterms(co): return sum(1 for c in co if c != 0)
def comb(n, e): return math.comb(n + e - 1, e)

def route_proxy(key, src_lr=None, owner_B=None):
    p, sup = key; o, t = routes[key]
    B = owner_B or basis(o)
    if not B.ok: return None
    idx = B.idx; nimg = {}
    tot = 0
    for ex, k in traffic[key].items():
        c = 1
        for j, e in ex:
            if j not in nimg:
                nimg[j] = nterms(B.coords(mapped_form(p, j, t, idx, src_lr)))
            c *= comb(nimg[j], e)
        tot += c * k
    return tot

exact_cache = {}
def route_exact(key, src_lr=None, owner_B=None, t=None):
    p, sup = key; o, t0 = routes[key]; t = t or t0
    B = owner_B or basis(o)
    idx = B.idx; lin = {}
    tot = 0
    for ex, k in traffic[key].items():
        fs = []
        for j, e in ex:
            if j not in lin: lin[j] = linear_factor(B.coords(mapped_form(p, j, t, idx, src_lr)))
            fs.append((lin[j], e))
        ck = tuple(sorted(fs))
        if ck not in exact_cache: exact_cache[ck] = expand_count(fs)
        tot += exact_cache[ck] * k
    return tot

# ---- calibration of the exact model on the current package
t0 = time.time(); bad = 0; tot_meas = sum(measured.values())
for key in routes:
    e = route_exact(key)
    if e != measured[key]:
        bad += 1
        if bad < 5: say('CALIB MISMATCH', key, e, measured[key])
say(f'calibration: {len(routes)} routes, mismatches {bad}, measured routed terms {tot_meas:,}, {time.time()-t0:.0f} s')

# ---- free owners and contracts
def kind(pid): return slots[pid]['kind'].split(' ')[0]
def unit(w, i): return [int(k == i) for k in range(w)]
def upto(a, b): return a == b or a == [-x for x in b]
def contract_fn(pid):
    p = slots[pid]; L = p['loops']; w = L + 1; K = kind(pid)
    lines = [s['line'] for s in p['slots'] if 'line' in s]
    isps = [s['isp'] for s in p['slots'] if 'isp' in s]
    def lin_ok(l, a, b):
        if a[l] == 0 and b[l] == 0: return True
        if a[l] != 0 and b[l] == 0: return upto(a, unit(w, l))
        if b[l] != 0 and a[l] == 0: return upto(b, unit(w, l))
        return False
    if K in ('InsertionAdapted', 'TriangleAdapted'):
        need = 2 if K == 'InsertionAdapted' else 3
        ls = [l for l in range(L) if sum(1 for m in lines if m[l] != 0) == need and any(upto(m, unit(w, l)) for m in lines)
              and all(lin_ok(l, a, b) for a, b in isps)]
        if not ls: return None
        return lambda a, b: all(lin_ok(l, a, b) for l in ls)
    if K == 'CarpetAdapted':
        qs = [m for m in lines if m[L] != 0]
        if len(qs) != 1: return None
        outs = [l for l in range(L) if qs[0][l] != 0]
        if len(outs) != 1: return None
        out = outs[0]; q = unit(w, L)
        def ok(a, b):
            if a[L] == 0 and b[L] == 0: return True
            if upto(a, q) and b[L] == 0: return b[out] == 0
            if upto(b, q) and a[L] == 0: return a[out] == 0
            return False
        if not all(ok(a, b) for a, b in isps): return None
        return ok
    return None

# owners ranked by routed terms touching them (inbound + outbound)
touch = collections.Counter()
for key, (o, t) in routes.items(): touch[o] += measured[key]; touch[key[0]] += measured[key]
free = []
for pid, _ in touch.most_common():
    if RESTRICT and pid not in RESTRICT: continue
    if lane.get(pid) in ('Custom', 'Scheme', None): continue
    if kind(pid) not in ('InsertionAdapted', 'TriangleAdapted', 'CarpetAdapted'): continue
    cf = contract_fn(pid)
    if cf is None: say('no contract inference for', pid, kind(pid)); continue
    free.append((pid, cf))
    if len(free) >= TOP: break
say('free owners:', [(pid, kind(pid)[:4], lane.get(pid), touch[pid]) for pid, _ in free])

inb = collections.defaultdict(list); outb = collections.defaultdict(list)
for key, (o, t) in routes.items(): inb[o].append(key); outb[key[0]].append(key)

def pool(pid, cf):
    p = slots[pid]; L = p['loops']; w = L + 1
    lines = [(i, s['line']) for i, s in enumerate(p['slots']) if 'line' in s]
    cands = []
    if POOL in ('pairs', 'both', 'all'):
        mom = lines + [(-1, unit(w, L))]
        for (ia, a), (ib, b) in itertools.combinations(mom, 2):
            cands.append((('pair', ia, ib), a, b))
    if POOL in ('units', 'both', 'all'):
        for i in range(L):
            for j in range(i + 1, L):
                cands.append((('unit', i, j), unit(w, i), unit(w, j)))
            cands.append((('unit', i, L), unit(w, i), unit(w, L)))
    if POOL == 'all':
        for key in inb[pid]:
            sp, t = key[0], routes[key][1]
            for j, sj in enumerate(slots[sp]['slots']):
                if 'line' in sj: a = b = map_row(sj['line'], t)
                else: a, b = map_row(cur_lr[sp][j][0], t), map_row(cur_lr[sp][j][1], t)
                if a == b and sum(1 for x in a if x) == 1: continue
                cands.append((('inh', sp, j), a, b))
    idx = pair_index(L + 1); seen = set(); outl = []
    for tag, a, b in cands:
        if not cf(a, b): continue
        if a[L] != 0 and b[L] != 0: continue   # massless ISP: constant q^2 term forbidden (shifted_unit_scale_kinematics)
        f = form(a, b, idx); k1 = tuple(f); k2 = tuple(-x for x in f)
        if not any(f) or k1 in seen or k2 in seen: continue
        seen.add(k1); outl.append((tag, a, b, f))
    return outl

def owner_cost(pid, lr, forms):
    """proxy cost of inbound + outbound routes of pid with its ISP slots given (lr, forms)."""
    B = basis(pid, forms)
    if not B.ok: return None
    c = 0
    for key in inb[pid]:
        if key[0] == pid: continue
        v = route_proxy(key, None if key[0] != pid else lr, B); c += v
    for key in outb[pid]:
        o = routes[key][0]
        v = route_proxy(key, lr, B if o == pid else None); c += v
    return c

def total_exact():
    return sum(route_exact(k) for k in routes)

base_exact = total_exact(); say(f'baseline exact routed terms {base_exact:,}')
tags = {pid: [None] * len(slots[pid]['slots']) for pid, _ in free}
result = {}
for ps in range(PASSES):
    for pid, cf in free:
        t1 = time.time(); p = slots[pid]; nl = nlines[pid]
        lr = list(cur_lr[pid]); forms = list(cur[pid])
        best = owner_cost(pid, lr, forms); start = best
        P = pool(pid, cf); improved = True
        while improved:
            improved = False
            for j in range(nl, len(forms)):
                for tag, a, b, f in P:
                    if any(tuple(f) == tuple(x) or tuple(f) == tuple(-y for y in x) for x in forms[nl:]): continue
                    lr2 = lr[:j] + [[a, b]] + lr[j + 1:]; f2 = forms[:j] + [f] + forms[j + 1:]
                    c = owner_cost(pid, lr2, f2)
                    if c is not None and c < best:
                        best, lr, forms, improved = c, lr2, f2, True; tags[pid][j] = tag
        cur[pid] = forms; cur_lr[pid] = lr
        result[pid] = {'start_proxy': start, 'best_proxy': best}
        say(f'pass {ps} owner {pid} {kind(pid)[:8]}/{lane.get(pid)} proxy {start:,} -> {best:,} ({100*(best-start)/max(1,start):+.1f} %) {time.time()-t1:.0f} s')
new_exact = total_exact()
say(f'exact routed terms: baseline {base_exact:,} -> {new_exact:,} ({100*(new_exact-base_exact)/base_exact:+.2f} %) [stored maps]')
# per-route exact deltas
rows = []
for key in routes:
    rows.append((key, routes[key][0], measured[key], route_exact(key)))
json.dump({'baseline': base_exact, 'new': new_exact,
           'owners': {str(pid): {'name': cpres[pid]['name'], 'kind': kind(pid), 'lane': lane.get(pid), 'isps_old': [s.get('isp') for s in slots[pid]['slots'] if 'isp' in s],
                                 'isps_new': [x for x in cur_lr[pid] if x is not None], 'tags': tags[pid], **result.get(pid, {})} for pid, _ in free},
           'routes': [[k[0], k[1], o, m, n] for k, o, m, n in rows]}, open(os.path.join(outdir, 'result.json'), 'w'))
