# lane arrivals: plan of the PHYSICAL chain run: every ancestor of the targets (all routes on file), physical start specs
# (S/phys/specs/<X>/spec.json: box D <= 1, r <= 4 on the SHIPPED table of package s12349c), child frame checks against the
# shipped tables. Usage: python3 -I plan_phys.py <S> <run dir R> <target> [..]
import json, os, sys, collections
S, R = sys.argv[1:3]; targets = sys.argv[3:]
WS = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev'
HO = WS + '/handoff/sym-investigation-for-ben/handdown-routes'
os.makedirs(R, exist_ok=True)
alias = {'T162i': 'T162', 'T178f': 'T178'}
files = {}
for d in [HO + '/chain/edges', HO + '/edges', S + '/chain162/probe/out', S + '/chain178/probe/out']:
    if os.path.isdir(d):
        for f in sorted(os.listdir(d)):
            if f.endswith('.edge.json'):
                p, c = f[:-10].split('-')
                files.setdefault((p, alias.get(c, c)), d + '/' + f)
par = collections.defaultdict(set)
for (p, c) in files:
    par[c].add(p)
def anc(x):
    s, todo = set(), [x]
    while todo:
        y = todo.pop()
        for p in par.get(y, ()):
            if p not in s:
                s.add(p); todo.append(p)
    return s
st = json.load(open(WS + '/P5L_STATUS.json'))
lines = {n['id']: n['lines'] for n in st['nodes']}
A = set()
for t in targets:
    A |= anc(t)
N = A | set(targets)
nodes = sorted(A, key=lambda x: (-lines[x], x))
man = json.load(open(S + '/phys/specs/manifest.json'))
ship = {n: man[n]['table'] for n in man}
out = []
for x in nodes:
    args = []
    for (p, c), f in sorted(files.items()):
        if p == x and c in N:
            lab = json.load(open(f))['child']['label']
            args += ['--edge', f, '--child-table', lab + '=' + ship[c]]
    inc = []
    for (p, c), f in sorted(files.items()):
        if c == x and p in N:
            lab = json.load(open(f))['child']['label']
            inc.append(f'{R}/{p}/out/{p}-{lab}.arrivals.jsonl')
    ea, ic = f'{R}/{x}.edges.txt', f'{R}/{x}.incoming.txt'
    open(ea, 'w').write(' '.join(args) + ('\n' if args else ''))
    open(ic, 'w').write(''.join(i + '\n' for i in inc))
    out.append({'node': x, 'wave': 15 - lines[x], 'lines': lines[x], 'spec': man[x]['spec'], 'edge_args': ea, 'incoming': ic,
                'out_edges': len(args) // 4, 'in_edges': len(inc)})
with open(R + '/plan.jsonl', 'w') as f:
    for o in out:
        f.write(json.dumps(o) + '\n')
for t in targets:
    inc_t = [f'{R}/{p}/out/{p}-{json.load(open(f))["child"]["label"]}.arrivals.jsonl' for (p, c), f in sorted(files.items()) if c == t and p in N]
    open(R + '/' + t + '.incoming.txt', 'w').write(''.join(i + '\n' for i in inc_t))
print(json.dumps({'targets': targets, 'nodes': len(nodes), 'waves': collections.Counter(o['lines'] for o in out),
                  'edges': sum(o['out_edges'] for o in out)}))
