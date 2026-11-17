#!/usr/bin/env python3
"""Per-rule exactness class of a table from (1) the evidence binding of every rule, (2) the per-key proof results and
(3) the first-match / liveness data of the rules in scope. Generic: every file is an argument.

Classes (exactly these words): "whole domain", "every first-match key in cone", "sampled N keys", "dead in cone", "none".

Inputs
  --binding B.jsonl   one line per rule of the table (the exactness binding step): table_index, dispatch_position, bytes,
                      target_rhs_sha256, domain_sha256, class (evidence class or null), category (whole / per_key /
                      not_proven), changed_vs_evidence.
  --scope S.json      the rules in scope: [{"rule", "fm_keys_cone" (int or null), "fm_complete" (bool: the first-match set in
                      the cone is known exhaustively), "live": true/false/null, "liveness_source": str, ...extra}]
  --results DIR       results/r<rule>.jsonl of the per-key prover (last line per id counts)
  --fm-dump TAG=PATH  first-match keys of the cone per rule (JSONL with key + served "<table>#<label>"), to decide
                      "every first-match key in cone" (repeatable)
  --name-format F     how a rule index appears in "served", e.g. "T162.final [0,1905)#{i}" (repeatable: first match wins
                      by trying each format)
  --per-key-n RULE=N  key counts of evidence-only per-key rows (e.g. a helper's E-KEYS key counts) (repeatable)
  --whole-categories  categories counted as whole domain (default: whole)
Output --out class_table.jsonl (all rules of the binding) and --summary class_table.summary.json.
"""
import argparse, collections, json, os


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--binding', required=True)
    ap.add_argument('--scope', required=True)
    ap.add_argument('--results', required=True)
    ap.add_argument('--fm-dump', action='append', default=[])
    ap.add_argument('--name-format', action='append', default=[])
    ap.add_argument('--per-key-n', action='append', default=[])
    ap.add_argument('--per-key-all', default='', help='comma list of evidence classes meaning "all first-match keys"')
    ap.add_argument('--per-key-all-rules', default='', help='comma list of rules whose evidence covers all first-match keys')
    ap.add_argument('--out', required=True)
    ap.add_argument('--summary', required=True)
    a = ap.parse_args()
    binding = [json.loads(l) for l in open(a.binding) if l.strip()]
    scope = {int(s['rule']): s for s in json.load(open(a.scope))}
    pkn = {int(x.split('=')[0]): int(x.split('=')[1]) for x in a.per_key_n}
    allcls = set(x for x in a.per_key_all.split(',') if x)
    allrules = set(int(x) for x in a.per_key_all_rules.split(',') if x.strip())

    def label_of(served):
        for f in a.name_format:
            pre = f.split('{i}')[0]
            if served.startswith(pre):
                try:
                    return int(served[len(pre):])
                except ValueError:
                    pass
        return None

    fm = collections.defaultdict(set)
    for d in a.fm_dump:
        _, path = d.split('=', 1)
        for line in open(path):
            v = json.loads(line)
            r = label_of(v.get('served') or '')
            if r is not None and r in scope:
                fm[r].add(tuple(v['key']))
    res = {}
    for r in scope:
        p = os.path.join(a.results, f'r{r}.jsonl')
        last = {}
        if os.path.exists(p):
            for line in open(p):
                if line.endswith('\n'):
                    v = json.loads(line)
                    last[v['id']] = v
        res[r] = list(last.values())
    out = []
    summ = collections.defaultdict(lambda: {'rules': 0, 'bytes': 0, 'keys_exact': 0, 'keys_not_decided': 0})
    for b in binding:
        r = b['table_index']
        row = {'rule': r, 'dispatch_position_f0bd7e76': b.get('dispatch_position'), 'bytes': b.get('bytes'),
               'target_rhs_sha256': b.get('target_rhs_sha256'), 'domain_sha256': b.get('domain_sha256'),
               'evidence_class': b.get('class'), 'evidence_category': b.get('category')}
        if r not in scope:
            if b.get('category') == 'whole':
                row['class'] = 'whole domain'
            elif b.get('category') == 'per_key':
                if b.get('class') in allcls or r in allrules:
                    row['class'] = 'every first-match key in cone'
                else:
                    row['class'] = f'sampled {pkn[r]} keys' if r in pkn else 'sampled N keys'
                    row['keys_exact'] = pkn.get(r)
            else:
                row['class'] = 'none'
            row['class_source'] = 'evidence binding (not re-run here)'
        else:
            s = scope[r]
            rows = res.get(r, [])
            ex = {tuple(v['key']) for v in rows if v['verdict'] == 'EXACT'}
            nd = [v['key'] for v in rows if v['verdict'] == 'NOT_DECIDED']
            bad = [v for v in rows if v['verdict'] not in ('EXACT', 'NOT_DECIDED')]
            row.update({k: v for k, v in s.items() if k != 'rule'})
            row['keys_exact'] = len(ex)
            row['keys_not_decided'] = len(nd)
            row['keys_not_decided_list'] = nd[:50]
            row['keys_by_source'] = dict(collections.Counter(src for v in rows if v['verdict'] == 'EXACT' for src in v['sources']))
            if bad:
                row['class'] = 'none'
                row['STOP'] = bad[:5]
            elif s.get('live') is False:
                row['class'] = 'dead in cone'
            elif s.get('fm_complete') and fm.get(r) is not None and len(fm[r]) > 0 and fm[r] <= ex:
                row['class'] = 'every first-match key in cone'
            elif ex:
                row['class'] = f'sampled {len(ex)} keys'
            else:
                row['class'] = 'none'
            row['class_source'] = 'per-key proofs (rr_proof instance-span) of this delivery'
        cls = row['class'] if not row['class'].startswith('sampled') else 'sampled N keys'
        summ[cls]['rules'] += 1
        summ[cls]['bytes'] += row.get('bytes') or 0
        summ[cls]['keys_exact'] += row.get('keys_exact') or 0 if r in scope else 0
        summ[cls]['keys_not_decided'] += row.get('keys_not_decided') or 0
        out.append(row)
    with open(a.out, 'w') as f:
        for row in out:
            f.write(json.dumps(row) + '\n')
    scope_summ = collections.defaultdict(lambda: {'rules': 0, 'bytes': 0, 'keys_exact': 0, 'keys_not_decided': 0})
    for row in out:
        if row['rule'] in scope:
            cls = row['class'] if not row['class'].startswith('sampled') else 'sampled N keys'
            for k in ('bytes', 'keys_exact', 'keys_not_decided'):
                scope_summ[cls][k] += row.get(k) or 0
            scope_summ[cls]['rules'] += 1
    json.dump({'all_rules': summ, 'in_scope': scope_summ, 'rules': len(out), 'in_scope_rules': len(scope)},
              open(a.summary, 'w'), indent=1)
    print(json.dumps({'in_scope': scope_summ}, indent=1))


if __name__ == '__main__':
    main()
