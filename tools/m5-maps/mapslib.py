"""M5 maps: exact route-expansion cost model for RustRed p presentations.

A presentation slot is a quadratic form in (k_1..k_L, q): a line is mom.mom, an ISP is left.right.
The owner's coordinates are its N slot forms plus q^2 (a basis of all quadratic forms). A routed key's
numerator (negative powers on source slots outside the support) is prod_j F_j(T k)^{e_j}; expressed in
owner coordinates it is a polynomial whose distinct monomials are exactly the transport's output terms
(lines in the support map monomially, by admission). Everything is integer/rational exact.
"""
from fractions import Fraction as Fr
from functools import lru_cache
import itertools, json

Q2 = -1  # P_EXTERNAL_SQUARE


def pair_index(n):
    idx = {}
    for i in range(n):
        for j in range(i, n):
            idx[(i, j)] = len(idx)
    return idx


def form(l, r, idx):
    n = len(l)
    v = [0] * len(idx)
    for i in range(n):
        if l[i] == 0:
            continue
        for j in range(n):
            c = l[i] * r[j]
            if c:
                v[idx[(min(i, j), max(i, j))]] += c
    return v


def slot_form(slot, idx):
    if 'line' in slot:
        m = slot['line']
        return form(m, m, idx)
    l, r = slot['isp']
    return form(l, r, idx)


def inverse(m):
    n = len(m)
    a = [[Fr(x) for x in row] + [Fr(int(i == j)) for j in range(n)] for i, row in enumerate(m)]
    for c in range(n):
        p = next((r for r in range(c, n) if a[r][c] != 0), None)
        if p is None:
            return None
        a[c], a[p] = a[p], a[c]
        inv = 1 / a[c][c]
        a[c] = [x * inv for x in a[c]]
        for r in range(n):
            if r != c and a[r][c] != 0:
                f = a[r][c]
                a[r] = [x - f * y for x, y in zip(a[r], a[c])]
    return [row[n:] for row in a]


def det(m):
    n = len(m)
    a = [[Fr(x) for x in row] for row in m]
    d = Fr(1)
    for c in range(n):
        p = next((r for r in range(c, n) if a[r][c] != 0), None)
        if p is None:
            return Fr(0)
        if p != c:
            a[c], a[p] = a[p], a[c]
            d = -d
        d *= a[c][c]
        for r in range(c + 1, n):
            if a[r][c] != 0:
                f = a[r][c] / a[c][c]
                a[r] = [x - f * y for x, y in zip(a[r], a[c])]
    return d


class Basis:
    """Coordinates of a presentation: forms (list of pair-vectors) + q^2 last."""

    def __init__(self, forms, loops):
        self.loops = loops
        self.idx = pair_index(loops + 1)
        q2 = [0] * len(self.idx)
        q2[self.idx[(loops, loops)]] = 1
        self.rows = [list(f) for f in forms] + [q2]
        assert len(self.rows) == len(self.idx), (len(self.rows), len(self.idx))
        self.inv = inverse(self.rows)
        self.ok = self.inv is not None

    def coords(self, v):
        """v = c . rows  ->  c (len N+1, last = q^2 coefficient)."""
        n = len(self.rows)
        return tuple(sum(v[i] * self.inv[i][j] for i in range(n) if v[i]) for j in range(n))


def map_row(row, m):
    li, lo = m['loops_in'], m['loops_out']
    A, b, s = m['loop_linear'], m['loop_external'], m['external_sign']
    out = [0] * (lo + 1)
    for i in range(li):
        if row[i]:
            for j in range(lo):
                out[j] += row[i] * A[i * lo + j]
            out[lo] += row[i] * b[i]
    out[lo] += row[li] * s
    return out


def compose(t, m):
    """r -> (r T) M."""
    L = t['loops_out']
    At, bt, st = t['loop_linear'], t['loop_external'], t['external_sign']
    Am, bm, sm = m['loop_linear'], m['loop_external'], m['external_sign']
    Li = t['loops_in']
    A = [sum(At[i * L + k] * Am[k * L + j] for k in range(L)) for i in range(Li) for j in range(L)]
    b = [sum(At[i * L + k] * bm[k] for k in range(L)) + bt[i] * sm for i in range(Li)]
    return {'loops_in': Li, 'loops_out': L, 'loop_linear': A, 'loop_external': b, 'external_sign': st * sm}


def mapped_slot_form(slot, t, idx):
    if 'line' in slot:
        r = map_row(slot['line'], t)
        return form(r, r, idx)
    l, r = slot['isp']
    return form(map_row(l, t), map_row(r, t), idx)


def linear_factor(coords):
    """Integer-scaled linear polynomial: tuple of (var, coef) with the constant as var -1."""
    den = 1
    for c in coords:
        if c != 0:
            den = den * c.denominator // __import__('math').gcd(den, c.denominator)
    n = len(coords) - 1
    terms = tuple((i, int(c * den)) for i, c in enumerate(coords[:-1]) if c != 0)
    const = coords[-1] * Q2 * den
    if const != 0:
        terms = terms + ((-1, int(const)),)
    return terms


MOD = (1 << 61) - 1


def expand_count(factors):
    """factors: list of (linear_factor, power). Distinct monomials with nonzero coefficient (mod a 61-bit prime)."""
    poly = {(): 1}
    for lin, e in factors:
        for _ in range(e):
            new = {}
            for mono, c in poly.items():
                for var, a in lin:
                    if var < 0:
                        key = mono
                    else:
                        key = tuple(sorted(mono + (var,)))
                    new[key] = (new.get(key, 0) + c * a) % MOD
            poly = {k: v for k, v in new.items() if v}
    return len(poly)


def proxy_count(factors):
    """Forcer-style proxy: sum over factors of len(image)^2 * power."""
    return sum(len(lin) ** 2 * e for lin, e in factors if len(lin) > 1)


def load_slots(path):
    return {p['id']: p for p in json.load(open(path))}


def load_census(path):
    c = json.load(open(path))
    sectors = {}
    for s in c['sectors']:
        if 'map' in s:
            sectors[(s['presentation'], s['support'])] = (s['class'], s['map'])
    classes = {x['id']: x for x in c['classes']}
    pres = {p['id']: p for p in c['presentations']}
    return sectors, classes, pres


def line_slots(p):
    return [i for i, s in enumerate(p['slots']) if 'line' in s]


def automorphisms(p, support):
    """All signed affine loop maps M of presentation p (row action) permuting the support lines up to
    sign with equal eps units, as dicts; the global sign pair (M, -M) is kept once (s = +1)."""
    L = p['loops']
    lines = [i for i in line_slots(p) if support >> i & 1]
    rows = {i: p['slots'][i]['line'] for i in lines}
    eps = {i: p['slots'][i]['eps'] for i in lines}
    # basis lines: greedy independent k-parts
    basis = []
    for i in lines:
        cand = basis + [i]
        sub = [[rows[j][k] for k in range(L)] for j in cand]
        # rank check via Gram det
        g = [[sum(x * y for x, y in zip(a, b)) for b in sub] for a in sub]
        if det(g) != 0:
            basis = cand
        if len(basis) == L:
            break
    if len(basis) < L:
        return [identity(L)]
    Rk = [[Fr(rows[j][k]) for k in range(L)] for j in basis]
    Rinv = inverse(Rk)
    # express each line: r = sum_m c_m r_basis_m + c_q e_q
    coef = {}
    for i in lines:
        rk = rows[i][:L]
        c = [sum(Fr(rk[k]) * Rinv[k][m] for k in range(L)) for m in range(L)]
        cq = Fr(rows[i][L]) - sum(c[m] * rows[basis[m]][L] for m in range(L))
        coef[i] = (c, cq)
    lastdep = {i: max([m for m in range(L) if coef[i][0][m] != 0], default=-1) for i in lines}
    by_vec = {}
    for i in lines:
        by_vec[tuple(rows[i])] = i
        by_vec[tuple(-x for x in rows[i])] = i
    out = []
    s = 1

    def image(i, imgs):
        c, cq = coef[i]
        v = [sum(c[m] * imgs[m][k] for m in range(len(imgs)) if c[m] != 0) for k in range(L + 1)]
        v[L] += cq * s
        if any(x.denominator != 1 for x in v):
            return None
        return tuple(int(x) for x in v)

    def dfs(m, imgs, used):
        if m == L:
            perm = {}
            for i in lines:
                v = image(i, imgs)
                j = by_vec.get(v) if v is not None else None
                if j is None or eps[j] != eps[i] or j in perm.values():
                    return
                perm[i] = j
            # solve M: A = Rk^-1 Img_k ; b = Rk^-1 (img_q - r_q s)
            Ik = [[Fr(imgs[mm][k]) for k in range(L)] for mm in range(L)]
            A = [[sum(Rinv[i][kk] * Ik[kk][j] for kk in range(L)) for j in range(L)] for i in range(L)]
            iq = [Fr(imgs[mm][L]) - rows[basis[mm]][L] * s for mm in range(L)]
            b = [sum(Rinv[i][kk] * iq[kk] for kk in range(L)) for i in range(L)]
            if any(x.denominator != 1 for row in A for x in row) or any(x.denominator != 1 for x in b):
                return
            Ai = [int(x) for row in A for x in row]
            if abs(det([[int(x) for x in row] for row in A])) != 1:
                return
            out.append({'loops_in': L, 'loops_out': L, 'loop_linear': Ai, 'loop_external': [int(x) for x in b],
                        'external_sign': s, 'perm': perm})
            return
        src = basis[m]
        for j in lines:
            if j in used or eps[j] != eps[src]:
                continue
            for sg in (1, -1):
                if m == 0 and sg == -1:
                    pass
                img = [sg * x for x in rows[j]]
                imgs2 = imgs + [img]
                ok = True
                for i in lines:
                    if lastdep[i] == m:
                        v = image(i, imgs2)
                        if v is None or by_vec.get(v) is None or eps[by_vec[v]] != eps[i]:
                            ok = False
                            break
                if ok:
                    dfs(m + 1, imgs2, used | {j})

    for s_ in (1, -1):
        s = s_
        dfs(0, [], frozenset())
    # dedupe +-M (M and -M give the same forms)
    seen, uniq = set(), []
    for mm in out:
        key = tuple(mm['loop_linear']) + tuple(mm['loop_external']) + (mm['external_sign'],)
        neg = tuple(-x for x in mm['loop_linear']) + tuple(-x for x in mm['loop_external']) + (-mm['external_sign'],)
        if neg in seen or key in seen:
            continue
        seen.add(key)
        uniq.append(mm)
    return uniq


def identity(L):
    return {'loops_in': L, 'loops_out': L, 'loop_linear': [int(i == j) for i in range(L) for j in range(L)],
            'loop_external': [0] * L, 'external_sign': 1}
