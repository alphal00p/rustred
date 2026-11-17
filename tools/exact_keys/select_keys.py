#!/usr/bin/env python3
"""Assemble the per-key proof jobs of chosen rules from first-match dumps (generic; every input is an argument).

Inputs (all JSONL lines carry "key" and "served" = "<table name>#<label>"):
  --dump TAG=PATH    keys with their first match, e.g. an arrival dump (redispatch_keys PARAMETRIC_DUMP_OUT), a chain walk
                     (PARAMETRIC_CHAIN_KEYS_OUT; only kind "rule" lines count) or a face dump (every first-match key of the
                     rules' faces in the cone). Repeatable.
  --rules JSON       [{"name": "<table#label>", "rule": <table index>, "want": [<what>...]}, ...] where <what> is
                       "<TAG>"            every key of that dump served by the rule
                       "<TAG>:all"        same (explicit)
                       "<TAG>:sample:N"   a STRATIFIED sample of N keys of that dump served by the rule: the keys are grouped by
                                          shell (D + r, from --chart), every shell gets an equal share (capped by its size,
                                          the remainder redistributed), so the lowest and the highest shell are always in;
                                          all keys when there are at most N. Deterministic (seed = sha256 of name + TAG).
  --chart MASK       the chart ('1' = line): D + r = sum_lines (k - 1) + sum_numerators (-k)
Output --out JOBS.jsonl ({"rule", "key", "sources"}, one line per distinct (rule, key)) and --summary S.json (per rule: keys per
source, per shell, sample allocation).
"""
import argparse, collections, hashlib, json, random


def excess(k, chart):
    return sum((x - 1) if c == '1' else -x for x, c in zip(k, chart))


def stratified(keys, n, chart, seed):
    if len(keys) <= n:
        return sorted(keys), None
    by = collections.defaultdict(list)
    for k in keys:
        by[excess(k, chart)].append(k)
    shells = sorted(by)
    quota = {s: 0 for s in shells}
    left, open_ = n, list(shells)
    while left > 0 and open_:
        share = max(1, left // len(open_))
        nxt = []
        for s in open_:
            take = min(share, len(by[s]) - quota[s], left)
            quota[s] += take
            left -= take
            if quota[s] < len(by[s]):
                nxt.append(s)
            if left == 0:
                break
        open_ = nxt
    rng = random.Random(int(hashlib.sha256(seed.encode()).hexdigest(), 16))
    out = []
    for s in shells:
        pool = sorted(by[s])
        out.extend(rng.sample(pool, quota[s]))
    return sorted(out), {str(s): [quota[s], len(by[s])] for s in shells}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--dump', action='append', default=[])
    ap.add_argument('--rules', required=True)
    ap.add_argument('--chart', required=True)
    ap.add_argument('--out', required=True)
    ap.add_argument('--summary', required=True)
    a = ap.parse_args()
    rules = json.load(open(a.rules))
    names = {r['name']: r for r in rules}
    served = collections.defaultdict(lambda: collections.defaultdict(set))  # tag -> name -> keys
    for d in a.dump:
        tag, path = d.split('=', 1)
        for line in open(path):
            if not line.strip():
                continue
            v = json.loads(line)
            if v.get('kind', 'rule') != 'rule' or v.get('served') not in names:
                continue
            served[tag][v['served']].add(tuple(v['key']))
    jobs = collections.defaultdict(set)
    summ = {}
    for r in rules:
        name = r['name']
        s = {'rule': r['rule'], 'name': name, 'sources': {}}
        for w in r['want']:
            parts = w.split(':')
            tag = parts[0]
            ks = served[tag].get(name, set())
            if len(parts) >= 3 and parts[1] == 'sample':
                pick, alloc = stratified(ks, int(parts[2]), a.chart, f'{name}:{tag}')
                src = f'{tag}:sample'
                s['sources'][src] = {'available': len(ks), 'taken': len(pick), 'per_shell_taken_of': alloc}
            else:
                pick = sorted(ks)
                src = tag
                s['sources'][src] = {'available': len(ks), 'taken': len(pick)}
            for k in pick:
                jobs[(r['rule'], k)].add(src)
        s['distinct_keys'] = sum(1 for (rr, _) in jobs if rr == r['rule'])
        summ[name] = s
    with open(a.out, 'w') as f:
        for (rule, k), src in sorted(jobs.items()):
            f.write(json.dumps({'rule': rule, 'key': list(k), 'sources': sorted(src)}) + '\n')
    json.dump({'jobs': len(jobs), 'rules': summ}, open(a.summary, 'w'), indent=1)
    print(json.dumps({'jobs': len(jobs), 'rules': len(rules)}))


if __name__ == '__main__':
    main()
