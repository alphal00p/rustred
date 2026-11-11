"""Independent check of an imported 5L-campaign owner: Laporta modulo subsectors at numeric d.

Usage: python3 -I laporta_check.py TABLE.jsonl IMPORT_DIR LABEL --d 37/7,-11/5,9/13 [--dots R] [--num R] [--eps-span R]
                                    [--prime P] [--exact]

From the table header's family (its own .sym text, q.q = +1, power shifts) this script builds the IBP identities
d/dk_i . v of the integrand (v = k_1..k_L, q) itself (no RustRed code), keeps only integrals of the top sector
(every non-eps line at power >= 1; eps lines are two-sided and always kept), and reduces the seeds of
IMPORT_DIR/LABEL.replay.json by sparse Gaussian elimination (Laporta order: most complex integral first) at each
numeric d, modulo the prime P (default 2^61-1) and, with --exact, over Q. Every seed must reduce to the residual
keys of the owner alone (else the seed box is too small: enlarge --dots/--num).

Then the replay (the imported payload's own rules run by RustRed's Custom-owner program, rustred p-import-table
--replay) is evaluated at the same d and converted back to the table convention:
    c_table = c_presentation * (-1)^(|seed| - |residual|)
and compared coefficient by coefficient. Boundary keys (strict subsectors) are the subsector part: zero modulo
subsectors on both sides. Output: one JSON line per (seed, d) and a summary; exit 0 iff all match.
"""
import argparse
import json
import re
import sys
import time
from fractions import Fraction
from itertools import product

P61 = (1 << 61) - 1


def parse_family(header):
    src = header['family_source']
    loops = re.search(r'loops\(([^)]*)\)', src).group(1).split(',')
    loops = [x.strip() for x in loops]
    ext = re.search(r'externals\(([^)]*)\)', src).group(1).strip()
    gram = re.search(r'gram\(\s*%s\s*,\s*%s\s*,\s*([^)]*)\)' % (ext, ext), src).group(1).strip()
    assert gram == '1', gram
    names = loops + [ext]
    slots = []
    for m in re.finditer(r'prop\(\s*(\w+)\s*,\s*\((.*?)\)\^2\s*,\s*([01])\s*\)', src):
        expr = m.group(2).replace(' ', '')
        row = [0] * len(names)
        for sign, coef, name in re.findall(r'([+-]?)(?:(\d+)\*)?([A-Za-z]\w*)', expr):
            row[names.index(name)] += (-1 if sign == '-' else 1) * (int(coef) if coef else 1)
        slots.append((m.group(1), row, m.group(3) == '1'))
    eps = []
    for s in header['power_shifts']:
        # j (4-d)/2: value at d = 0 is 2j
        v = eval_expr(s, {'d': Fraction(0)})
        assert v.denominator == 1 and v.numerator % 2 == 0, s
        eps.append(v.numerator // 2)
    return loops, slots, eps


_tok = re.compile(r'\d+|[A-Za-z_][A-Za-z_0-9]*|\*\*|[-+*/^()]')


def eval_expr(text, env):
    """Exact evaluation of a printed rational expression (+ - * / ^, integers, variables)."""
    toks = _tok.findall(text.replace(' ', ''))
    pos = [0]

    def peek():
        return toks[pos[0]] if pos[0] < len(toks) else None

    def take():
        t = toks[pos[0]]; pos[0] += 1; return t

    def atom():
        t = take()
        if t == '(':
            v = expr(); assert take() == ')'; return v
        if t == '-':
            return -power()
        if t == '+':
            return power()
        if t.isdigit():
            return Fraction(int(t))
        return env[t]

    def power():
        b = atom()
        if peek() in ('^', '**'):
            take(); e = power()
            assert e.denominator == 1
            return b ** int(e)
        return b

    def term():
        v = power()
        while peek() in ('*', '/'):
            op = take(); w = power()
            v = v * w if op == '*' else v / w
        return v

    def expr():
        v = term()
        while peek() in ('+', '-'):
            op = take(); w = term()
            v = v + w if op == '+' else v - w
        return v
    v = expr()
    assert pos[0] == len(toks), text
    return v


def solve_rational(A, b):
    n = len(A)
    M = [[Fraction(x) for x in A[i]] + [Fraction(b[i])] for i in range(n)]
    for c in range(n):
        p = next(r for r in range(c, n) if M[r][c] != 0)
        M[c], M[p] = M[p], M[c]
        inv = 1 / M[c][c]
        M[c] = [x * inv for x in M[c]]
        for r in range(n):
            if r != c and M[r][c] != 0:
                f = M[r][c]
                M[r] = [x - f * y for x, y in zip(M[r], M[c])]
    return [M[i][n] for i in range(n)]


def ibp_templates(loops, slots, Q):
    """Rows as lists of (shift tuple, (const, d_coef, {a: nu_coef})): d/dk_i . v of prod D_a^{-nu_a}."""
    L = len(loops)
    V = L + 1
    pairs = [(i, j) for i in range(V) for j in range(i, V) if not (i == L and j == L)]
    N = len(slots)
    assert len(pairs) == N, (len(pairs), N)
    # D_a = sum_{i<=j} A[a][pair] s_pair + const_a
    A = [[0] * N for _ in range(N)]
    const = [0] * N
    for a, (_, m, _) in enumerate(slots):
        for i in range(V):
            for j in range(i, V):
                c = m[i] * m[j] * (1 if i == j else 2)
                if i == L and j == L:
                    const[a] += c * Q
                else:
                    A[a][pairs.index((i, j))] += c
    # s_pair = sum_b Binv[pair][b] (D_b - const_b): solve A^T columns
    At_inv = []
    # Binv = A^{-1}
    cols = []
    for b in range(N):
        e = [Fraction(int(b == r)) for r in range(N)]
        cols.append(solve_rational(A, e))  # A x = e_b -> column b of A^{-1}
    Binv = [[cols[b][p] for b in range(N)] for p in range(N)]

    def sp(i, j):
        """v_i . v_j = (constant, {b: coefficient of D_b})."""
        i, j = min(i, j), max(i, j)
        if i == L and j == L:
            return Fraction(Q), {}
        p = pairs.index((i, j))
        lin = {b: Binv[p][b] for b in range(N) if Binv[p][b] != 0}
        c0 = -sum(Binv[p][b] * const[b] for b in range(N))
        return c0, lin

    rows = []
    zero = tuple([0] * N)
    for i in range(L):
        for v in range(V):
            terms = {}

            def add(shift, c0=Fraction(0), cd=Fraction(0), nu=None):
                t = terms.setdefault(shift, [Fraction(0), Fraction(0), {}])
                t[0] += c0; t[1] += cd
                for a, c in (nu or {}).items():
                    t[2][a] = t[2].get(a, Fraction(0)) + c
            if v == i:
                add(zero, cd=Fraction(1))
            for a, (_, m, _) in enumerate(slots):
                if m[i] == 0:
                    continue
                # -nu_a * 2 m_a[i] * (v . p_a) / D_a,  v.p_a = sum_l m_a[l] v.v_l
                c0_tot = Fraction(0); lin_tot = {}
                for l in range(V):
                    if m[l] == 0:
                        continue
                    c0, lin = sp(v, l)
                    c0_tot += m[l] * c0
                    for b, c in lin.items():
                        lin_tot[b] = lin_tot.get(b, Fraction(0)) + m[l] * c
                f = -2 * m[i]
                # constant part: nu_a * f * c0 * I(n + e_a)
                if c0_tot != 0:
                    s = list(zero); s[a] += 1
                    add(tuple(s), nu={a: f * c0_tot})
                for b, c in lin_tot.items():
                    s = list(zero); s[a] += 1; s[b] -= 1
                    add(tuple(s), nu={a: f * c})
            rows.append([(s, (t[0], t[1], {a: c for a, c in t[2].items() if c != 0}))
                         for s, t in terms.items() if t[0] != 0 or t[1] != 0 or any(c != 0 for c in t[2].values())])
    return rows


class Field:
    def __init__(self, prime):
        self.p = prime

    def conv(self, x):
        if self.p is None:
            return Fraction(x)
        x = Fraction(x)
        return x.numerator % self.p * pow(x.denominator % self.p, -1, self.p) % self.p

    def add(self, a, b):
        return (a + b) % self.p if self.p else a + b

    def mul(self, a, b):
        return a * b % self.p if self.p else a * b

    def inv(self, a):
        return pow(a, -1, self.p) if self.p else 1 / a

    def neg(self, a):
        return (-a) % self.p if self.p else -a

    def is_zero(self, a):
        return a == 0


def laporta(templates, slots, eps, d, seeds_box, F, keep, masters=frozenset()):
    eps_val = (4 - d) / 2
    N = len(slots)

    def weight(k):
        # The owner's residuals (`masters`) are the least integrals: Laporta then reduces onto THEM
        # (with LP >= 2 its own complexity order may prefer another basis, e.g. a numerator over a dot).
        dots = sum(k[a] - 1 for a in range(N) if slots[a][2] and eps[a] == 0 and k[a] > 1)
        num = sum(-k[a] for a in range(N) if not slots[a][2] and k[a] < 0)
        epsx = sum(abs(k[a] - 1) for a in range(N) if eps[a] != 0)
        return (k not in masters, dots + num + epsx, dots + epsx, num, tuple(-x for x in k))
    pivots = {}

    def reduce_row(row):
        # row: dict key -> value; eliminate pivot keys from the top
        while row:
            top = max(row, key=weight)
            if top in pivots:
                c = row.pop(top)
                for k, v in pivots[top].items():
                    if k == top:
                        continue
                    nv = F.add(row.get(k, 0), F.neg(F.mul(c, v)))
                    if F.is_zero(nv):
                        row.pop(k, None)
                    else:
                        row[k] = nv
            else:
                c = F.inv(row[top])
                for k in row:
                    row[k] = F.mul(row[k], c)
                return top, row
        return None, None
    for seed in seeds_box:
        for tmpl in templates:
            row = {}
            for shift, (c0, cd, nu) in tmpl:
                k = tuple(n + s for n, s in zip(seed, shift))
                if not keep(k):
                    continue
                val = c0 + cd * d
                for a, c in nu.items():
                    val += c * (seed[a] + eps[a] * eps_val)
                if val == 0:
                    continue
                v = F.conv(val)
                if F.is_zero(v):
                    continue
                row[k] = F.add(row.get(k, 0), v)
                if F.is_zero(row[k]):
                    del row[k]
            if not row:
                continue
            top, red = reduce_row(row)
            if top is not None:
                pivots[top] = red

    def express(target):
        row = {target: F.conv(1)}
        out = {}
        while row:
            top = max(row, key=weight)
            c = row.pop(top)
            if top in pivots:
                for k, v in pivots[top].items():
                    if k == top:
                        continue
                    nv = F.add(row.get(k, 0), F.neg(F.mul(c, v)))
                    if F.is_zero(nv):
                        row.pop(k, None)
                    else:
                        row[k] = nv
            else:
                out[top] = F.add(out.get(top, 0), c)
        return {k: v for k, v in out.items() if not F.is_zero(v)}
    return express, len(pivots)


def seed_box(slots, eps, dots, num, eps_span):
    N = len(slots)
    lines = [a for a in range(N) if slots[a][2] and eps[a] == 0]
    isps = [a for a in range(N) if not slots[a][2]]
    eaxes = [a for a in range(N) if eps[a] != 0]

    def spread(axes, total):
        if not axes:
            yield {}
            return
        for k in range(total + 1):
            for rest in spread(axes[1:], total - k):
                r = dict(rest); r[axes[0]] = k; yield r
    out = []
    for dd in spread(lines, dots):
        for nn in spread(isps, num):
            for ev in product(range(1 - eps_span, 2 + eps_span), repeat=len(eaxes)):
                k = [0] * N
                for a in lines:
                    k[a] = 1 + dd[a]
                for a in isps:
                    k[a] = -nn[a]
                for a, v in zip(eaxes, ev):
                    k[a] = v
                out.append(tuple(k))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('table'); ap.add_argument('import_dir'); ap.add_argument('label')
    ap.add_argument('--d', default='37/7,-11/5,9/13')
    ap.add_argument('--dots', type=int, default=1); ap.add_argument('--num', type=int, default=1)
    ap.add_argument('--eps-span', type=int, default=1)
    ap.add_argument('--prime', type=int, default=P61)
    ap.add_argument('--exact', action='store_true')
    ap.add_argument('--qq', type=int, default=1, choices=(1, -1),
                    help='q.q of the IBP rows: +1 = the table convention (replay converted back by the sign rule); '
                         '-1 = the presentation convention (replay compared as it is: tests the sign rule itself)')
    a = ap.parse_args()
    with open(a.table) as f:
        header = json.loads(f.readline())
    loops, slots, eps = parse_family(header)
    N = len(slots)
    replay = json.load(open('%s/%s.replay.json' % (a.import_dir, a.label)))
    # The owner's residuals in TABLE slot order (the import record keeps them in presentation order).
    record = json.load(open('%s/%s.json' % (a.import_dir, a.label)))
    slot_map = record['slot_map']
    masters = frozenset(tuple(key[slot_map[j]] for j in range(len(slot_map)))
                        for chart in record['charts'] for key in chart['residuals'])
    t0 = time.time()
    templates = ibp_templates(loops, slots, a.qq)
    t_tmpl = time.time() - t0

    def keep(k):
        return all(k[i] >= 1 for i in range(N) if slots[i][2] and eps[i] == 0) and \
            all(k[i] <= 0 for i in range(N) if not slots[i][2])
    box = seed_box(slots, eps, a.dots, a.num, a.eps_span)
    fields = [('mod %d' % a.prime, Field(a.prime))] + ([('Q', Field(None))] if a.exact else [])
    ok = True
    results = []
    for dtext in a.d.split(','):
        d = Fraction(dtext)
        for fname, F in fields:
            t1 = time.time()
            express, npiv = laporta(templates, slots, eps, d, box, F, keep, masters)
            tl = time.time() - t1
            for res in replay['results']:
                seed = tuple(res['seed'])
                lap = express(seed)
                rep = {}
                for key, coef in res['residuals']:
                    v = eval_expr(coef, {'d': d})
                    sign = (-1) ** ((sum(seed) - sum(key)) % 2) if a.qq == 1 else 1
                    rep[tuple(key)] = F.conv(v * sign)
                match = lap == rep
                ok &= match
                line = dict(seed=list(seed), d=dtext, field=fname, match=match,
                            laporta={str(list(k)): str(v) for k, v in lap.items()},
                            replay={str(list(k)): str(v) for k, v in rep.items()},
                            boundary_terms=len(res['boundary']), steps=res['steps'])
                results.append(line)
                print(json.dumps(line), flush=True)
            print(json.dumps(dict(d=dtext, field=fname, seeds_box=len(box), equations=len(box) * len(templates),
                                  pivots=npiv, seconds=round(tl, 2))), flush=True)
    print(json.dumps(dict(summary=True, label=a.label, qq=a.qq, all_match=ok, comparisons=len(results),
                          matched=sum(r['match'] for r in results), ibp_rows=len(templates),
                          template_seconds=round(t_tmpl, 2))))
    sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main()
