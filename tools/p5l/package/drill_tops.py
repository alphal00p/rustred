#!/usr/bin/env python3
"""drill_tops.py P_INTEGRAL_RS OUT.json [--workers N]: which five-loop tops a partial (drill / staged) p package must hold
so that vakint's catalogue matching finds every sector of its tops in it.

vakint (p_integral/catalogue.rs) keys every non-scaleless contraction of every family by the canonical form of its
multigraph (the two external vertices coloured alike) and stores as REPRESENTATIVE the first family in table order
(P_FAMILIES order), then the smallest pinch set. A sector of top T entered by a user is therefore dispatched to
rep(sector), which can be another top. This script ports that rule (contractions = pinched masks of the family lines,
entry vertex 1, exit vertex 2, is_scaleless of input.rs) with an exact canonical form (colour refinement +
individualisation, smallest encoding), and writes, per five-loop top T: the set of tops its sectors are dispatched to,
plus the number of its sector graphs per representative. Contractions of a family top are already normalised (pinching
creates no bridge and no repeated momentum), so input normalisation does not change the key.
"""
import json
import re
import sys
from multiprocessing import Pool


def parse_families(path):
    src = open(path).read()
    src = src[src.index('pub(crate) const P_FAMILIES'):]
    out = []
    for name, loops, lines in re.findall(r'name: "([^"]+)",\s*loops: (\d),.*?lines: "([^"]+)"', src, re.S):
        edges = [(int(i), int(a), int(b)) for i, a, b in re.findall(r'prop\((\d+),edge\((\d+),(\d+)\)', lines)]
        out.append((name, int(loops), edges))
    return out


def find(parent, x):
    while parent.get(x, x) != x:
        x = parent[x]
    return x


def components(vertices, lines, removed):
    parent = {}
    for a, b in lines:
        if a == removed or b == removed:
            continue
        ra, rb = find(parent, a), find(parent, b)
        if ra != rb:
            parent[ra] = rb
    parts = {}
    for v in vertices:
        if v == removed:
            continue
        parts.setdefault(find(parent, v), set()).add(v)
    return list(parts.values())


def is_scaleless(lines, entry, exit_, loops):
    if entry == exit_ or any(a == b for a, b in lines):
        return True
    vertices = {v for line in lines for v in line} | {entry, exit_}
    parts = components(vertices, lines, None)
    if len(lines) + len(parts) < len(vertices) + loops:
        return True
    for removed in list(vertices) + [None]:
        for part in components(vertices, lines, removed):
            if entry not in part and exit_ not in part:
                return True
    return False


def canonical(lines, entry, exit_):
    verts = sorted({v for line in lines for v in line} | {entry, exit_})
    idx = {v: i for i, v in enumerate(verts)}
    n = len(verts)
    adj = [dict() for _ in range(n)]
    for a, b in lines:
        a, b = idx[a], idx[b]
        adj[a][b] = adj[a].get(b, 0) + 1
        adj[b][a] = adj[b].get(a, 0) + 1
    ext = {idx[entry], idx[exit_]}

    def refine(colors):
        while True:
            sig = [(colors[v], tuple(sorted((colors[w], m) for w, m in adj[v].items()))) for v in range(n)]
            ranks = {s: r for r, s in enumerate(sorted(set(sig)))}
            new = [ranks[s] for s in sig]
            if len(set(new)) == len(set(colors)):
                return new
            colors = new

    def encode(colors):
        edges = sorted((min(colors[a], colors[b]), max(colors[a], colors[b]), m)
                       for a in range(n) for b, m in adj[a].items() if a <= b)
        return (tuple(sorted(colors[v] for v in ext)), tuple(edges))

    best = [None]

    def search(colors):
        colors = refine(colors)
        cells = {}
        for v, c in enumerate(colors):
            cells.setdefault(c, []).append(v)
        target = None
        for c in sorted(cells):
            if len(cells[c]) > 1 and (target is None or len(cells[c]) < len(cells[target])):
                target = c
        if target is None:
            e = encode(colors)
            if best[0] is None or e < best[0]:
                best[0] = e
            return
        for v in cells[target]:
            new = [2 * c for c in colors]
            new[v] = 2 * colors[v] - 1  # individualise v just below its cell
            search(new)

    init = [(1 if v in ext else 0, sum(adj[v].values())) for v in range(n)]
    ranks = {s: r for r, s in enumerate(sorted(set(init)))}
    search([ranks[s] for s in init])
    return (n, best[0])


def family_keys(args):
    name, loops, edges = args
    keys = {}
    count = len(edges)
    for mask in range(1 << count):
        parent = {}
        rest = []
        for pos, (_, a, b) in enumerate(edges):
            if mask >> pos & 1:
                ra, rb = find(parent, a), find(parent, b)
                if ra != rb:
                    parent[ra] = rb
        for pos, (_, a, b) in enumerate(edges):
            if not mask >> pos & 1:
                rest.append((find(parent, a), find(parent, b)))
        entry, exit_ = find(parent, 1), find(parent, 2)
        if is_scaleless(rest, entry, exit_, loops):
            continue
        key = canonical(rest, entry, exit_)
        keys[key] = keys.get(key, 0) + 1
    return name, keys


def main():
    path, out = sys.argv[1], sys.argv[2]
    workers = int(sys.argv[sys.argv.index('--workers') + 1]) if '--workers' in sys.argv else 1
    fams = [f for f in parse_families(path) if f[1] == 5]
    with Pool(workers) as pool:
        results = pool.map(family_keys, fams, chunksize=1)
    rep = {}
    for name, keys in results:  # P_FAMILIES order
        for key in keys:
            rep.setdefault(key, name)
    tops = {}
    for name, keys in results:
        per = {}
        for key in keys:
            per[rep[key]] = per.get(rep[key], 0) + 1
        tops[name] = {'sector_graphs': len(keys), 'dispatched_to': dict(sorted(per.items()))}
    json.dump({'schema': 'vk5l-wp6.drill-tops.v1', 'rule': 'vakint catalogue: first family in P_FAMILIES order',
               'families': [f[0] for f in fams], 'distinct_graphs': len(rep), 'tops': tops},
              open(out, 'w'), indent=1)
    print(f'{len(fams)} five-loop families, {len(rep)} distinct sector graphs -> {out}')


if __name__ == '__main__':
    main()
