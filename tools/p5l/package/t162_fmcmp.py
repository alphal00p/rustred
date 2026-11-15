#!/usr/bin/env python3
"""t162_fmcmp.py keys STATEMENT_DIR OUT_KEYS | cmp STATEMENT_DIR MIXED_DIR FM_OUT : (lane vk5l-wp6) the engine's first match
(rr_node first-match) on the mixed table versus the closure statement's winner at every closure key. 'keys' writes the integer
key list; 'cmp' maps the engine label (record index) back to the rule name via dispatch.json and counts disagreements."""
import json, sys
def ints(k): return [int(x) for x in k[2:-1].split(',')]
if sys.argv[1] == 'keys':
    with open(sys.argv[3], 'w') as o:
        for l in open(sys.argv[2] + '/keys.jsonl'): o.write(json.dumps(ints(json.loads(l)[0])) + '\n')
    sys.exit(0)
S, M, F = sys.argv[2:5]
disp = json.load(open(M + '/dispatch.json'))
win = {tuple(ints(k)): v for k, v in disp.items()}
n = bad = 0; ex = []
for l in open(F):
    r = json.loads(l); k = tuple(r['key']); n += 1
    want = win[k]; got = r['label']
    want_n = None if want in ('terminal', 'open', 'astra-open') else want
    if got != want_n:
        bad += 1
        if len(ex) < 20: ex.append({'key': k, 'statement': want, 'engine': got, 'position': r['position']})
res = {'keys': n, 'statement_keys': len(win), 'disagreements': bad, 'examples': ex}
print(json.dumps(res)); sys.exit(0 if bad == 0 and n == len(win) else 1)
