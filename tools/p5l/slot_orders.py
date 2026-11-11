"""Slot-order scoreboard of a directory of p-import-table records (P_TABLE_IMPORT_VERSION >= 2).

Usage: python3 -I slot_orders.py OUT_DIR [--json SUMMARY.json]

Per node: the table's order name and family file, the census relation (slot permutation and loop map, from momenta or
an ORDER-MAP) and the import numbers; totals by kind: census-slots (identity map, loop map 1), permuted-slots (an order
variant / presentation of the repair, Astra's D1/D9), '+<order>' for an order other than the campaign's legacy /
two_sided (Astra's isp_reverse), rerouted (loop map != 1).
"""
import glob
import json
import os
import sys


def main():
    out = sys.argv[1]
    rows = []
    for path in sorted(glob.glob(os.path.join(out, 'T*.json'))):
        if path.endswith(('.replay.json', '.seeds.json')):
            continue
        rec = json.load(open(path))
        so = rec.get('slot_order')
        if so is None:
            continue
        census = so.get('census') or {}
        perm = census.get('census_to_table')
        lm = census.get('loop_map')
        identity_perm = perm is not None and perm == list(range(len(perm)))
        identity_loop = lm is not None and all(v == (r == c) for r, row in enumerate(lm) for c, v in enumerate(row))
        native = so['order_name'] in ('legacy', 'two_sided')  # the campaign's default orders (integer / eps nodes)
        kind = ('no-census' if perm is None else
                'rerouted' if not identity_loop else
                ('census-slots' if identity_perm else 'permuted-slots') + ('' if native else '+' + so['order_name']))
        rows.append(dict(tid=rec['label'], kind=kind, order=so['order_name'],
                         family_file=os.path.basename(so['table_family_file']), census_source=census.get('source'),
                         census_to_table=perm, rules=rec['rules'], residuals=rec['residuals'],
                         payload_bytes=rec['payload_bytes'], import_seconds=round(rec['import_seconds'], 2),
                         verify_keys=rec['verify']['witness_keys'], descent_order=so['presentation_order'][:60]))
    kinds = {}
    for r in rows:
        kinds[r['kind']] = kinds.get(r['kind'], 0) + 1
    summary = dict(nodes=len(rows), kinds=kinds, payload_bytes=sum(r['payload_bytes'] for r in rows),
                   import_seconds=round(sum(r['import_seconds'] for r in rows), 1),
                   variants=[r for r in rows if r['kind'] != 'census-slots'])
    if '--json' in sys.argv:
        json.dump(dict(summary=summary, rows=rows), open(sys.argv[sys.argv.index('--json') + 1], 'w'), indent=1)
    print(json.dumps({k: v for k, v in summary.items() if k != 'variants'}))
    for r in summary['variants']:
        print('%s %-12s %-12s %-24s %s' % (r['tid'], r['kind'], r['order'], r['family_file'], r['census_to_table']))


if __name__ == '__main__':
    main()
