"""Reproduce Forcer's findBestMomentumBasis (generation/pathforcer.py:636) for the momsubs input conversion of a
fixed topology, with our exact cost model, and compare with the bases in the vendored forcer/momsubs.prc.

Forcer's cost: for a candidate momentum basis B (L+1 edges incl. Q, independent, containing the P-flow path), express
every scalar product p_a.p_b with a, b in B in Forcer's variables (internal line squares i1.., the node's dots, Q^2);
score = sum over those with more than one term of (#terms)^2; first minimum in itertools.combinations order wins.
usage: forcer_basis.py NAME
"""
import sys, re, itertools, os
sys.path.insert(0, os.path.dirname(__file__))
from mapslib import pair_index, form, Basis, det
from fractions import Fraction as Fr

TOPOS = {
    'lala': ([(0, 3), (5, 8), (4, 5), (3, 4), (2, 3), (1, 2), (1, 8), (2, 4), (1, 4), (1, 5), (8, 9)],
             [(0, 5), (0, 2), (1, 4), (1, 5), (2, 4)]),
    'bubu': ([(0, 4), (4, 5), (2, 4), (3, 4), (5, 6), (2, 5), (3, 5), (1, 6), (1, 2), (1, 3), (6, 9)],
             [(0, 2), (0, 8), (2, 3), (2, 9), (3, 8)]),
    'no6': ([(0, 1), (7, 8), (6, 7), (2, 6), (1, 2), (1, 7), (3, 7), (3, 4), (4, 8), (4, 6), (2, 3), (8, 9)],
            [(1, 2), (2, 4), (0, 2), (0, 3)]),
    'cross': ([(0, 1), (3, 5), (1, 3), (4, 5), (1, 4), (2, 5), (1, 2), (2, 3), (2, 4), (5, 9)],
              [(0, 1), (0, 2), (0, 3), (0, 4), (1, 4), (2, 3)]),
}


def momenta(edges, nv):
    """Edge momenta over (k_1..k_L, Q) via a spanning tree; orientation u->v for u<v; Q enters at 0, leaves at nv-1."""
    E = len(edges)
    internal = [i for i in range(E)]
    # unknowns: edge momenta; solve conservation with cotree edges as loop momenta
    parent = list(range(nv))
    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]; x = parent[x]
        return x
    tree, cotree = [], []
    for i, (u, v) in enumerate(edges):
        a, b = find(u), find(v)
        if a != b:
            parent[max(a, b)] = min(a, b); tree.append(i)
        else:
            cotree.append(i)
    L = len(cotree)
    # linear system: for each vertex except 0 and nv-1 ... solve by least structure: use Fractions Gaussian elimination
    # variables: all E edge momenta as vectors; equations: edge0 = Q, cotree_i = k_i, conservation at internal vertices.
    width = L + 1
    rows, rhs = [], []
    def unit(i):
        r = [0] * E; r[i] = 1; return r
    rows.append(unit(0)); rhs.append([0] * L + [1])
    for m, i in enumerate(cotree):
        rows.append(unit(i)); rhs.append([int(k == m) for k in range(L)] + [0])
    for vtx in range(1, nv - 1):
        r = [0] * E
        for i, (u, v) in enumerate(edges):
            if u == vtx: r[i] -= 1   # outgoing
            if v == vtx: r[i] += 1   # incoming
        if any(r): rows.append(r); rhs.append([0] * width)
    # solve least squares exactly: pick independent rows
    A = [[Fr(x) for x in r] + [Fr(y) for y in b] for r, b in zip(rows, rhs)]
    n = E
    piv = []
    rI = 0
    for c in range(n):
        p = next((i for i in range(rI, len(A)) if A[i][c] != 0), None)
        if p is None: continue
        A[rI], A[p] = A[p], A[rI]
        inv = 1 / A[rI][c]; A[rI] = [x * inv for x in A[rI]]
        for i in range(len(A)):
            if i != rI and A[i][c] != 0:
                f = A[i][c]; A[i] = [x - f * y for x, y in zip(A[i], A[rI])]
        piv.append(c); rI += 1
    assert len(piv) == E, (piv, E)
    mom = [None] * E
    for i, c in enumerate(piv):
        mom[c] = [int(x) for x in A[i][n:]]
    return mom, L


def best_basis(name, include, edges=None, dots=None, verbose=False):
    edges, dots = TOPOS[name]
    nv = max(max(e) for e in edges) + 1
    mom, L = momenta(edges, nv)
    idx = pair_index(L + 1)
    E = len(edges)
    ext_in, ext_out = 0, E - 1
    internal = [i for i in range(E) if 0 not in edges[i] and nv - 1 not in edges[i]]
    forms = [form(mom[i], mom[i], idx) for i in internal] + [form(mom[a], mom[b], idx) for a, b in dots]
    B = Basis(forms, L)
    assert B.ok
    cand_edges = [i for i in range(E) if nv - 1 not in edges[i]]
    allb = [c for c in itertools.combinations(cand_edges, L + 1) if det([mom[i] for i in c]) != 0]
    fb = [c for c in allb if set(include) <= set(c) and 0 in c]
    if not fb:  # Forcer's relaxation (pathforcer.py:702)
        includes = [list(set(include) - {0})]
        while not fb:
            includes = [[j for j in inc if j != i] for inc in includes for i in inc]
            fb = [c for c in allb if any((set(i) | {0}) <= set(c) for i in includes)]
    best, bestscore = None, 0
    for c in fb:
        score = 0
        for a, b in itertools.combinations_with_replacement(c, 2):
            co = B.coords(form(mom[a], mom[b], idx))
            n = sum(1 for x in co if x != 0)
            if n > 1:
                score += n * n
        if best is None or score < bestscore:
            best, bestscore = c, score
    return best, bestscore


def forcer_blocks(name):
    text = open('/shared/vhirschi/t3code/vakint_dev/p_integral_dev/repos/forcer-upstream/forcer/momsubs.prc').read()
    start = text.index('#case M' + name + '\n')
    end = text.index('#endswitch\n#break', start)
    block = text[start:end]
    out = []
    for part in block.split('#endif\n#if ( `DOTS')[:-1]:
        paths = re.findall(r'\* path: \(([^)]*)\)', part.split('#if ( `MOMS')[0].split('#endif')[-1] if False else part)
        dep = re.findall(r'id (Q|p\d+)\.p\?!', part)
        out.append((paths, dep))
    return out


if __name__ == '__main__':
    name = sys.argv[1]
    E = len(TOPOS[name][0])
    text = open('/shared/vhirschi/t3code/vakint_dev/p_integral_dev/repos/forcer-upstream/forcer/momsubs.prc').read()
    start = text.index('#case M' + name + '\n')
    end = text.index('#endswitch\n#break', start)
    block = text[start:end]
    segs = re.split(r'#break\n', block)
    agree = total = 0
    for seg in segs:
        paths = re.findall(r'\* path: \(([^)]*)\)', seg)
        default = '#default' in seg
        dep = re.findall(r'id (Q|p\d+)\.p\?!', seg)
        if not dep and not paths:
            continue
        depset = set(0 if d == 'Q' else int(d[1:]) for d in dep)
        forcer_basis = tuple(sorted(set(range(E - 1)) - depset))
        for pth in (paths + ([''] if default else [])):
            inc = tuple(int(x) for x in pth.replace(' ', '').split(',') if x != '')
            ours, score = best_basis(name, inc)
            total += 1
            ok = tuple(sorted(ours)) == forcer_basis
            agree += ok
            print(f'{name} path {inc or "default"}: forcer {forcer_basis} ours {tuple(ours)} score {score} {"OK" if ok else "DIFF"}')
    print(f'{name}: {agree}/{total} bases reproduced')
