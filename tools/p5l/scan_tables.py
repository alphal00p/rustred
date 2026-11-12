"""Scan every table_dump.v1 table of the 5L campaign ledger: record types, schemas, dispatch values, case kinds,
chart_end deferred counters. Output JSON on stdout. Usage: python3 -I scan_tables.py P5L_STATUS.json"""
import json, sys, collections, os, re
led = json.load(open(sys.argv[1]))
out = {}
for n in led['nodes']:
    tid = n['id']
    rd = n.get('run_dir')
    if not rd or n['state'] not in ('generated',):
        continue
    cands = [os.path.join(rd, 'F.table.jsonl'), os.path.join(rd, 'assembled', 'F.table.jsonl')]
    path = next((p for p in cands if os.path.exists(p)), None)
    if path is None:
        out[tid] = {'missing': rd}; continue
    types = collections.Counter(); disp = collections.Counter(); kinds = collections.Counter()
    deferred = 0; schema = None; end = False; maxabs = 0; eqs = 0; exc = 0
    with open(path) as f:
        for line in f:
            r = json.loads(line); t = r['type']; types[t] += 1
            if t == 'header': schema = r['schema']
            elif t == 'rule':
                disp[r.get('dispatch')] += 1
                kinds[r['case'].split('(')[0]] += 1
                eqs += bool(r['equations']); exc += bool(r['exceptions'])
                for v in r['fixed']:
                    if v is not None: maxabs = max(maxabs, abs(v))
            elif t == 'chart_end':
                deferred += sum(r.get(k, 0) or 0 for k in ('deferred_cases', 'deferred_branches', 'deferred_regions'))
            elif t == 'end': end = True
    out[tid] = dict(path=path, cls=n['class'], schema=schema, types=types, dispatch=disp, kinds=kinds, deferred=deferred,
                    end=end, max_fixed=maxabs, affine_rules=eqs, rules_with_exceptions=exc, size=os.path.getsize(path))
json.dump(out, sys.stdout, indent=0, default=dict)
