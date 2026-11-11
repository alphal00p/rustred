"""Write the 64 5L top families as rustred.p-families.v1 entries.

Usage: python3 -I p_families_5l.py INDEX_JSON FAMILIES_DIR OUT_JSON

INDEX_JSON / FAMILIES_DIR: study/pint-closure-study/scratch/families (index.json + <family>.sym, the 64 5L tops that the
forcer-tricks census used: Forcer createTree of these 64 gives the 283 custom nodes). For each family the .sym lines
(props with power 1) are matched to the index's line_edges (vertex pairs) with orientations such that momentum is
conserved with q entering at cut_edge[0] and leaving at cut_edge[1] (checked, not assumed); ISPs are the .sym props
with power 0, as momentum squares. Vertices are renumbered 1..V (p-families convention), gram [[-1]] (q.q = -1).
"""
import json
import re
import sys
from itertools import product


def parse_sym(text):
    loops = [x.strip() for x in re.search(r'loops\(([^)]*)\)', text).group(1).split(',')]
    ext = re.search(r'externals\(([^)]*)\)', text).group(1).strip()
    names = loops + [ext]
    props = []
    for m in re.finditer(r'prop\(\s*(\w+)\s*,\s*\((.*?)\)\^2\s*,\s*([01])\s*\)', text):
        row = [0] * len(names)
        for sign, coef, name in re.findall(r'([+-]?)(?:(\d+)\*)?([A-Za-z]\w*)', m.group(2).replace(' ', '')):
            row[names.index(name)] += (-1 if sign == '-' else 1) * (int(coef) if coef else 1)
        props.append((row, m.group(3) == '1'))
    return loops, props


def orient(edges, rows, s, t, nv):
    """Signs with conservation: sum_in - sum_out + q[s] - q[t] = 0 at every vertex; line i on edges[i] (u -> v)."""
    W = len(rows[0])
    last = {}
    for i, (u, v) in enumerate(edges):
        last[u] = i; last[v] = i
    bal = [[0] * W for _ in range(nv)]
    bal[s][W - 1] += 1; bal[t][W - 1] -= 1
    signs = [0] * len(edges)

    def go(i):
        if i == len(edges):
            return True
        u, v = edges[i]
        for sg in (1, -1):
            for c in range(W):
                bal[u][c] -= sg * rows[i][c]; bal[v][c] += sg * rows[i][c]
            ok = all((last[x] != i) or not any(bal[x]) for x in (u, v))
            if ok:
                signs[i] = sg
                if go(i + 1):
                    return True
            for c in range(W):
                bal[u][c] += sg * rows[i][c]; bal[v][c] -= sg * rows[i][c]
        return False
    return signs if go(0) else None


def main():
    index, fdir, out = sys.argv[1:4]
    fams = []
    for f in json.load(open(index)):
        loops, props = parse_sym(open('%s/%s.sym' % (fdir, f['family'])).read())
        lines = [r for r, is_line in props if is_line]
        isps = [r for r, is_line in props if not is_line]
        edges = [tuple(e) for e in f['line_edges']]
        assert len(edges) == len(lines), f['family']
        nv = 1 + max(max(max(e) for e in edges), max(f['cut_edge']))
        s, t = f['cut_edge']
        signs = orient(edges, lines, s, t, nv)
        if signs is None:
            signs = orient(edges, lines, t, s, nv)
            assert signs is not None, f['family']
            s, t = t, s
        fams.append({
            'name': f['family'],
            'loops': len(loops),
            'vertices': nv,
            'lines': [{'endpoints': [u + 1, v + 1], 'momentum': [sg * c for c in row]}
                      for (u, v), row, sg in zip(edges, lines, signs)],
            'isps': isps,
            'external': {'in': s + 1, 'out': t + 1},
            'gram': [[-1]],
        })
    json.dump({'schema': 'rustred.p-families.v1',
               'provenance': 'p5l-converter tools/p_families_5l.py from study/pint-closure-study/scratch/families '
                             '(index.json + .sym of the 64 5L tops of the forcer-tricks census)',
               'families': fams}, open(out, 'w'), indent=1)
    print(len(fams), 'families')


if __name__ == '__main__':
    main()
