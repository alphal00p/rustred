# lane arrivals: FIXPOINT CHECK of a target's arrival set in a chain run (coordinator request 2026-11-16 step 4).
# For every ancestor X of the target (the nodes with a route path to it in the plan):
#   (a) X is done and its walk's exact closure ended with a round adding 0 keys in its last depth layer (compute.err);
#   (b) X's walk started after every parent of X was done (chain.log: the "X start rows" line after each parent's "P done");
#   (c) X's incoming list names exactly the arrival files of its parents in the plan, each present.
# Then a second pass would read the same inputs and reproduce the same files: no new key at the target (DAG, one pass).
# Usage: python3 -I fixpoint_check.py <run dir R> <target> <label>   -> R/fixpoint.<target>.json
import json, os, re, sys

R, target, label = sys.argv[1:4]
plan = {json.loads(l)['node']: json.loads(l) for l in open(R + '/plan.jsonl')}
par = {}
for x, o in plan.items():
    inc = [l.strip() for l in open(o['incoming']) if l.strip()]
    par[x] = {os.path.basename(os.path.dirname(os.path.dirname(f))): f for f in inc}
children = {}
for x, o in plan.items():
    for c in re.findall(r'--edge \S+/(T\d+)-(T\d+[a-z]?)\.edge\.json', open(o['edge_args']).read()):
        children.setdefault(x, set()).add(c[1])
anc = {x for x in plan if label in children.get(x, ())}
todo = list(anc)
while todo:
    y = todo.pop()
    for p in par.get(y, {}):
        if p not in anc:
            anc.add(p); todo.append(p)
# chain.log times
done_t, start_t = {}, {}
for l in open(R + '/chain.log'):
    m = re.match(r'(\S+Z) (T\d+) (done|start rows)', l)
    if m:
        t, x, k = m.groups()
        if k == 'done':
            done_t[x] = t  # the last done line
        else:
            start_t[x] = t  # the last start line (the attempt that produced the result)
res = {'target': target, 'ancestors': len(anc), 'nodes': {}, 'ok': True}
# TOP-UP nodes (R/<X>/topup.json: parts with their incoming lists, merged by merge_topup.py = from scratch): every part's closure
# must end with +0 keys and start after its own inputs' parents were done; the parts' inputs together = the node's incoming list
for l in open(R + '/chain.log'):
    m = re.match(r'(\S+Z) (T\d+\.[AB]) (done|start rows)', l)
    if m:
        t, x, k = m.groups()
        (done_t if k == 'done' else start_t)[x] = t
def part_ok(x, part):
    d = f'{R}/{x}.{part}'
    rounds = [l for l in open(f'{d}/compute.err') if 'closure round' in l] if os.path.exists(f'{d}/compute.err') else []
    inc = [l.strip() for l in open(f'{d}/incoming.txt') if l.strip()]
    pars = {os.path.basename(os.path.dirname(os.path.dirname(f))) for f in inc}
    st = start_t.get(f'{x}.{part}')
    late = [q for q in pars if not (q in done_t and st and done_t[q] <= st)]
    return {'done': os.path.exists(f'{d}/done'), 'closure_fixpoint': bool(rounds) and '+0 keys' in rounds[-1],
            'closure_last': rounds[-1].strip()[:160] if rounds else None, 'started_after_parents': not late, 'late': late, 'inputs': sorted(pars)}
for x in sorted(anc):
    e = {}
    e['done'] = os.path.exists(f'{R}/{x}/done')
    if os.path.exists(f'{R}/{x}/topup.json'):
        tp = json.load(open(f'{R}/{x}/topup.json'))
        parts = {pt: part_ok(x, pt) for pt in tp['parts']}
        union = sorted(set().union(*[set(v['inputs']) for v in parts.values()]))
        e.update({'topup': parts, 'parents': sorted(par[x]), 'inputs_union_equals_incoming': union == sorted(par[x])})
        ok = e['done'] and e['inputs_union_equals_incoming'] and all(v['done'] and v['closure_fixpoint'] and v['started_after_parents'] for v in parts.values())
        e['ok'] = ok
        res['ok'] &= ok
        res['nodes'][x] = e
        continue
    rounds = [l for l in open(f'{R}/{x}/compute.err') if 'closure round' in l] if os.path.exists(f'{R}/{x}/compute.err') else []
    e['closure_rounds'] = len(rounds)
    e['closure_last'] = rounds[-1].strip()[:160] if rounds else None
    e['closure_fixpoint'] = bool(rounds) and '+0 keys' in rounds[-1]
    e['parents'] = sorted(par[x])
    e['inputs_present'] = all(os.path.isfile(f) and os.path.getsize(f) > 0 for f in par[x].values())
    st = start_t.get(x)
    late = [p for p in par[x] if not (p in done_t and st and done_t[p] <= st)]
    e['started_after_parents'] = not late
    if late:
        e['parents_not_done_before_start'] = late
    ok = e['done'] and e['closure_fixpoint'] and e['inputs_present'] and e['started_after_parents']
    e['ok'] = ok
    res['ok'] &= ok
    res['nodes'][x] = e
res['failing'] = [x for x, e in res['nodes'].items() if not e['ok']]
json.dump(res, open(f'{R}/fixpoint.{target}.json', 'w'), indent=1)
print(json.dumps({'target': target, 'ancestors': len(anc), 'ok': res['ok'], 'failing': res['failing'][:10]}))
