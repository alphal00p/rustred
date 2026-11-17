# lane arrivals: the plan of one arrival chain (data preparation only; the computation is rr_arrivals).
# Usage: python3 -I plan.py <S> <allnodes dir> <target> <run dir R> [excluded node ..]
# Writes R/plan.jsonl: one line per node of the target's ancestor sub-DAG in topological waves (line count descending):
# {node, wave, spec, edge_args (file), incoming (file)}, plus R/<node>.edges.txt and R/<node>.incoming.txt.
import json, os, sys

S, A, target, R = sys.argv[1:5]
excluded = set(sys.argv[5:])
WS = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev'
HO = WS + '/handoff/sym-investigation-for-ben/handdown-routes'
os.makedirs(R, exist_ok=True)
alias = {'T162i': 'T162', 'T178f': 'T178'}
files = {}
for d in [HO + '/chain/edges', HO + '/edges', S + '/chain162/probe/out']:
    if not os.path.isdir(d):
        continue
    for f in sorted(os.listdir(d)):
        if f.endswith('.edge.json'):
            p, c = f[:-10].split('-')
            files.setdefault((p, alias.get(c, c)), d + '/' + f)
par = {}
for (p, c) in files:
    par.setdefault(c, set()).add(p)


def anc(x):
    s, todo = set(), [x]
    while todo:
        y = todo.pop()
        for p in par.get(y, ()):
            if p not in s:
                s.add(p)
                todo.append(p)
    return s


st = json.load(open(WS + '/P5L_STATUS.json'))
lines = {n['id']: n['lines'] for n in st['nodes']}
nodes = sorted(anc(target) - excluded, key=lambda x: (-lines[x], x))
N = set(nodes) | {target}
specs = {json.loads(l)['node']: json.loads(l)['spec'] for l in open(A + '/specs/nodes.jsonl')}
for n in os.listdir(S + '/specs'):
    if os.path.exists(S + '/specs/' + n + '/spec.json') and n.startswith('T') and len(n) == 4:
        specs[n] = S + '/specs/' + n + '/spec.json'   # re-pinned copies (a table rewritten since allnodes pinned it)
tables = {json.loads(l)['node']: json.loads(l)['table'] for l in open(A + '/specs/nodes.jsonl')}
special = {'T162i': '/common/build-cache/vhirschi/p_integral_dev/bank-conv/20261114c/qualified/T162.origin000.table.jsonl',
           'T178f': '/common/build-cache/vhirschi/p_integral_dev/vk5l-wp1/t178/T178-box.unionbox.jsonl'}
out = []
for x in nodes:
    args = []
    for (p, c), f in sorted(files.items()):
        if p == x and c in N:
            lab = json.load(open(f))['child']['label']
            args += ['--edge', f, '--child-table', lab + '=' + special.get(lab, tables.get(c, ''))]
    inc = []
    for (p, c), f in sorted(files.items()):
        if c == x and p in N and p not in excluded:
            lab = json.load(open(f))['child']['label']
            inc.append(f'{R}/{p}/out/{p}-{lab}.arrivals.jsonl')
    ea, ic = f'{R}/{x}.edges.txt', f'{R}/{x}.incoming.txt'
    open(ea, 'w').write(' '.join(args) + ('\n' if args else ''))
    open(ic, 'w').write(''.join(i + '\n' for i in inc))
    out.append({'node': x, 'wave': 15 - lines[x], 'lines': lines[x], 'spec': specs.get(x), 'edge_args': ea, 'incoming': ic,
                'out_edges': len(args) // 4, 'in_edges': len(inc)})
with open(R + '/plan.jsonl', 'w') as f:
    for o in out:
        f.write(json.dumps(o) + '\n')
inc_t = [f'{R}/{p}/out/{p}-{json.load(open(f))["child"]["label"]}.arrivals.jsonl' for (p, c), f in sorted(files.items()) if c == target and p in N]
open(R + '/' + target + '.incoming.txt', 'w').write(''.join(i + '\n' for i in inc_t))
print(json.dumps({'target': target, 'nodes': len(nodes), 'excluded': sorted(excluded), 'edges_into_target': len(inc_t),
                  'no_spec': [o['node'] for o in out if not o['spec']], 'waves': sorted({o['lines'] for o in out}, reverse=True)}))
