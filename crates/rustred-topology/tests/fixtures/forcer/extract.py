"""Pure extraction of the Forcer golden fixtures of rustred-topology (not run in CI).

usage: python3 -I extract.py FORCER_ROOT OUT_DIR

FORCER_ROOT is a checkout of https://github.com/benruijl/forcer at 68a30bf. Writes into OUT_DIR:
  forcer_dag_68a30bf.tsv    the 438 topologies of forcer/pathforcer.h (Forcer's 4L reduction tree):
                            name, L, action, topology, children (triangle/diamond/custom/trivial only)
  forcer_tops_68a30bf.tsv   the top-level diagrams of generation/pathforcer.py createTopLevelDiagrams, L = 2..5 (79)
  forcer_fixed_68a30bf.tsv  the fixed topologies of generation/pathforcer.py createFixedTopologies(4) (18)

Only regular expressions over the two Forcer files: no simulator, no graph algorithm. The action of a topology is read
from the call in its Reduce procedure (insertionS / carpet / triangleS / diamondS / '#call Reduce' = custom; no line =
trivial), as SCRATCHPAD/forcer-tricks/forcer-src/tally.py did. Children are the 'change topology to X' targets of the
block (X != the block itself), recorded only where they are the complete child set (insertion and carpet blocks route
their rule output through Forcer's generic handover, so their child sets are not readable from the block).
Topology notation (Forcer): 'a-b' is one line, each '*' one eps unit on it; the lines at vertex 0 and at the highest
vertex are the two external legs.
"""
import ast
import os
import re
import sys


def blocks(pathforcer_h):
    src = open(pathforcer_h).read()
    return re.findall(r'\*--#\[ reduce-(\w+) :\n(.*?)\*--#\] reduce-\1 :', src, re.S)


def parse_topology(s):
    out = []
    for tok in s.split():
        stars = tok.count('*')
        a, b = tok.replace('*', '').split('-')
        out.append((int(a), int(b), stars))
    return out


def loops_of(edges):
    vmax = max(max(a, b) for a, b, _ in edges)
    internal = [(a, b) for a, b, _ in edges if 0 not in (a, b) and vmax not in (a, b)]
    if not internal:
        return 0
    verts = {v for e in internal for v in e}
    return len(internal) - len(verts) + 1


def action_of(name, body, n_internal):
    red = re.search(r'#procedure Reduce' + name + r'\(\)\n(.*?)#endprocedure', body, re.S).group(1)
    if 'insertionS(' in red:
        return 'insertion'
    if 'carpet(' in red:
        return 'carpet'
    if 'triangleS(' in red:
        return 'triangle'
    if 'diamondS(' in red:
        return 'diamond'
    if '#call Reduce(' in red:
        return 'custom'
    if n_internal <= 1:
        return 'trivial'
    raise ValueError('no action found for ' + name)


def tops(pathforcer_py, loops):
    src = open(pathforcer_py).read()
    body = src.split("def createTopLevelDiagrams(loops):")[1].split("\ndef ")[0]
    blk = body.split("if loops == %d:" % loops)[1].split("if loops ==")[0]
    out = []
    for m in re.finditer(r"diagrams\[(\d+)\]\.add_edges\((\[.*?\])\)", blk):
        edges = ast.literal_eval(m.group(2))
        names = re.findall(r"diagrams\[%s\]\['name'\] = '(\w+)'" % m.group(1), blk)
        out.append((names[0] if names else 'j%s' % m.group(1), [(u, v, 0) for u, v in edges]))
    return out


def fixed(pathforcer_py, loops):
    src = open(pathforcer_py).read()
    body = src.split("def createFixedTopologies(loops):")[1].split("\ndef ")[0]
    blk = body.split("if loops == %d:" % loops)[1].split("for d in fixed:")[0]
    out = []
    for part in blk.split("g = Graph(n=")[1:]:
        name = re.search(r"g\['name'\] = '(\w+)'", part).group(1)
        edges = ast.literal_eval(re.search(r"g\.add_edges\((\[.*?\])\)", part).group(1))
        eps = [0] * len(edges)
        for i, k in re.findall(r"g\.es\[(\d+)\]\['nonint'\] = (\d+)", part):
            eps[int(i)] = int(k)
        out.append((name, [(u, v, k) for (u, v), k in zip(edges, eps)]))
    return out


def fmt(edges):
    return ' '.join('%d-%d%s' % (u, v, '*' * k) for u, v, k in edges)


def main():
    root, out = sys.argv[1], sys.argv[2]
    rows = []
    bl = blocks(os.path.join(root, 'forcer', 'pathforcer.h'))
    names = {n for n, _ in bl}
    for name, body in bl:
        topo = re.search(r'\* Topology: (.*)', body).group(1).split()
        edges = parse_topology(' '.join(topo))
        vmax = max(max(a, b) for a, b, _ in edges)
        n_internal = sum(1 for a, b, _ in edges if 0 not in (a, b) and vmax not in (a, b))
        action = action_of(name, body, n_internal)
        children = ''
        if action in ('triangle', 'diamond', 'custom', 'trivial'):
            targets = set(re.findall(r'change topology to (\w+)', body))
            children = ','.join(sorted((targets & names) - {name}))
        rows.append((name, loops_of(edges), action, ' '.join(topo), children))
    assert len(rows) == 438, len(rows)
    with open(os.path.join(out, 'forcer_dag_68a30bf.tsv'), 'w') as f:
        f.write('# Forcer 4L reduction tree: the 438 topologies of forcer/pathforcer.h, benruijl/forcer 68a30bf.\n')
        f.write('# columns: name L action topology children; topology in Forcer notation (a-b = one line, each * = one '
                'eps unit, legs at vertex 0 and at the highest vertex); children = change-topology targets, only for '
                'triangle/diamond/custom/trivial.\n')
        for r in sorted(rows, key=lambda r: (-r[1], r[0])):
            f.write('\t'.join([r[0], str(r[1]), r[2], r[3], r[4]]) + '\n')
    pf = os.path.join(root, 'generation', 'pathforcer.py')
    with open(os.path.join(out, 'forcer_tops_68a30bf.tsv'), 'w') as f:
        f.write('# Forcer top-level diagrams, generation/pathforcer.py createTopLevelDiagrams, benruijl/forcer 68a30bf.\n')
        f.write('# columns: L name lines (Forcer notation, all lines integer; legs at vertex 0 and at the highest '
                'vertex).\n')
        n = 0
        for loops in (2, 3, 4, 5):
            for name, edges in tops(pf, loops):
                f.write('%d\t%s\t%s\n' % (loops, name, fmt(edges)))
                n += 1
        assert n == 79, n
    with open(os.path.join(out, 'forcer_fixed_68a30bf.tsv'), 'w') as f:
        f.write('# Forcer fixed topologies, generation/pathforcer.py createFixedTopologies(4), benruijl/forcer 68a30bf.\n')
        f.write('# columns: L name lines (Forcer notation, each * = one eps unit; legs at vertex 0 and at the highest '
                'vertex).\n')
        fx = fixed(pf, 4)
        assert len(fx) == 18, len(fx)
        for name, edges in fx:
            f.write('%d\t%s\t%s\n' % (loops_of(edges), name, fmt(edges)))


if __name__ == '__main__':
    main()
