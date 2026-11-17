#!/usr/bin/env python3
"""Every key of a rule's coordinate face inside the excess cone D + r <= R, as an RRKEYS01 key file.

Generic: the chart (which slots are lines), the bound R, the table and the rules are inputs. The face of a rule is its target
pattern: slots with a fixed integer are fixed, every other slot is free. A key of the chart has lines >= 1 and numerator slots
<= 0; its excess is D + r = sum_lines (k - 1) + sum_numerators (-k). The rule's one-slot domain bounds (domain_bounds.min /
max) prune the enumeration; everything else (equations, exceptions, earlier records) is left to the dispatcher, so the output
is a SUPERSET of the keys the rule can serve in the cone (the first-match set is cut out by dispatching these keys).

usage: face_keys.py --table T --index IDX.jsonl --rules a,b,... --bound R --out KEYS.bin [--summary S.json]
Each key is written once even when faces overlap (keys are deduplicated across the given rules).
RRKEYS01 layout (as the arrival key files): magic, u32 LE header length, header JSON, then per key arity x i8, u8 depth, u8 senders
(depth and senders are 0 here).
"""
import argparse, json, struct, itertools


def chart_of(table):
    with open(table) as f:
        for i, line in enumerate(f):
            v = json.loads(line)
            if v.get('type') == 'chart':
                return v['chart']
            if i > 4:
                break
    raise SystemExit('no chart record near the top of the table')


def rule_line(table, idx, r):
    e = idx[r]
    with open(table, 'rb') as f:
        f.seek(e['offset'])
        return json.loads(f.read(e['length']))


def compositions(budget, ranges):
    """All tuples (e_1..e_m) with e_i in [lo_i, hi_i] (excess units per free slot) and sum <= budget."""
    if not ranges:
        yield ()
        return
    lo, hi = ranges[0]
    rest_min = sum(r[0] for r in ranges[1:])
    for e in range(lo, min(hi, budget - rest_min) + 1):
        for t in compositions(budget - e, ranges[1:]):
            yield (e,) + t


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--table', required=True)
    ap.add_argument('--index', required=True)
    ap.add_argument('--rules', required=True)
    ap.add_argument('--bound', type=int, required=True)
    ap.add_argument('--out', required=True)
    ap.add_argument('--summary', default='')
    a = ap.parse_args()
    chart = chart_of(a.table)
    lines = [c == '1' for c in chart]
    n = len(chart)
    idx = {}
    for l in open(a.index):
        e = json.loads(l)
        if e.get('type') == 'rule':
            idx[e['index']] = e
    rules = [int(x) for x in a.rules.split(',') if x.strip()]
    seen = set()
    per_rule = {}
    body = bytearray()
    for r in rules:
        v = rule_line(a.table, idx, r)
        fixed = v['fixed']
        db = v.get('domain_bounds') or {}
        mn = db.get('min') or [None] * n
        mx = db.get('max') or [None] * n
        base_excess = 0
        free, ranges = [], []
        ok = True
        for s in range(n):
            x = fixed[s]
            if x is not None:
                if lines[s] and x < 1 or not lines[s] and x > 0:
                    ok = False
                base_excess += (x - 1) if lines[s] else -x
                continue
            # excess units e of a free slot: line k = 1 + e, numerator k = -e; one-slot bounds become bounds on e
            lo, hi = 0, a.bound
            if lines[s]:
                if mn[s] is not None: lo = max(lo, mn[s] - 1)
                if mx[s] is not None: hi = min(hi, mx[s] - 1)
            else:
                if mx[s] is not None: lo = max(lo, -mx[s])
                if mn[s] is not None: hi = min(hi, -mn[s])
            free.append(s)
            ranges.append((lo, hi))
        budget = a.bound - base_excess
        cnt = new = 0
        if ok and budget >= 0 and all(lo <= hi for lo, hi in ranges):
            for es in compositions(budget, ranges):
                k = list(fixed)
                for s, e in zip(free, es):
                    k[s] = 1 + e if lines[s] else -e
                cnt += 1
                t = tuple(k)
                if t in seen:
                    continue
                seen.add(t)
                new += 1
                body += struct.pack(f'{n}b', *k) + b'\0\0'
        per_rule[r] = {'target': v['target'], 'free_slots': free, 'face_keys_in_cone': cnt, 'new_keys': new,
                       'base_excess': base_excess}
        del v
    hdr = json.dumps({'schema': 'rustred.campaign.arrival-keys.v1', 'arity': n, 'frame': chart,
                      'record': f'{n} x i8 key (table frame), u8 depth (0), u8 senders (0)',
                      'built_from': f'faces of rules {a.rules} of {a.table} in D + r <= {a.bound}'}).encode()
    with open(a.out, 'wb') as f:
        f.write(b'RRKEYS01' + struct.pack('<I', len(hdr)) + hdr + bytes(body))
    s = {'keys': len(seen), 'bound': a.bound, 'chart': chart, 'rules': per_rule}
    if a.summary:
        json.dump(s, open(a.summary, 'w'), indent=1)
    print(json.dumps({'keys': len(seen), 'rules': len(rules)}))


if __name__ == '__main__':
    main()
