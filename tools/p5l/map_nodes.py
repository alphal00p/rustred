"""Map the census custom nodes (T001..T283, rules_study/targets.tsv: edges, S, T) onto the Custom owner nodes of the
rustred 5L DAG (`rustred p-generate --families p-families-1-5.json --max-loops 5 --list`), by graph isomorphism of
the multigraph with eps units, external pair {s,t} as a set (q -> -q is a symmetry of the key).
Usage: python3 -I map_nodes.py TARGETS_TSV LIST_TSV OUT_TSV"""
import csv
import sys
from collections import Counter


def parse(edges):
    out = []
    for tok in edges.split():
        e = tok.count('*'); u, v = tok.replace('*', '').split('-'); out.append((int(u), int(v), e))
    return out


def graph(lines, s, t):
    verts = sorted({x for u, v, _ in lines for x in (u, v)} | {s, t})
    adj = {v: Counter() for v in verts}
    for u, v, e in lines:
        adj[u][(v, e)] += 1; adj[v][(u, e)] += 1
    return verts, adj, {s, t}


def iso(g1, g2):
    v1, a1, e1 = g1; v2, a2, e2 = g2
    if len(v1) != len(v2):
        return False
    def sig(a, v, ext):
        return (v in ext, tuple(sorted(Counter(e for (_, e), m in a[v].items() for _ in range(m)).items())), sum(a[v].values()))
    s1 = {v: sig(a1, v, e1) for v in v1}; s2 = {v: sig(a2, v, e2) for v in v2}
    if Counter(s1.values()) != Counter(s2.values()):
        return False
    order = sorted(v1, key=lambda v: -sum(a1[v].values()))
    m = {}; used = set()

    def ok(x, y):
        for (w, e), c in a1[x].items():
            if w in m and a2[y].get((m[w], e), 0) != c:
                return False
        return True

    def go(i):
        if i == len(order):
            return True
        x = order[i]
        for y in v2:
            if y in used or s2[y] != s1[x] or not ok(x, y):
                continue
            m[x] = y; used.add(y)
            if go(i + 1):
                return True
            del m[x]; used.discard(y)
        return False
    return go(0)


def main():
    rows = list(csv.DictReader(open(sys.argv[1]), delimiter='\t'))
    nodes = list(csv.DictReader(open(sys.argv[2]), delimiter='\t'))
    ng = []
    for n in nodes:
        s, t = map(int, n['s,t'].split(','))
        ng.append((n, graph(parse(n['graph']), s, t), len(n['graph'].split())))
    out = []
    used = Counter()
    for r in rows:
        g = graph(parse(r['edges']), int(r['S']), int(r['T']))
        ne = len(r['edges'].split())
        hits = [n for n, h, k in ng if k == ne and iso(g, h)]
        out.append((r['id'], ';'.join(n['node'] for n in hits), ';'.join(n['graph_key'] for n in hits)))
        for n in hits:
            used[n['node']] += 1
    with open(sys.argv[3], 'w') as f:
        f.write('id\tnode\tgraph_key\n')
        for x in out:
            f.write('\t'.join(x) + '\n')
    print('targets', len(out), 'mapped to exactly one node', sum(1 for x in out if x[1] and ';' not in x[1]),
          'unmapped', sum(1 for x in out if not x[1]), 'nodes hit', len(used), 'nodes hit twice', sum(1 for v in used.values() if v > 1))


if __name__ == '__main__':
    main()
