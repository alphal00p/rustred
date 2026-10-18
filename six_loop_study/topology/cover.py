#!/usr/bin/env python3
"""Verify that the retained maximal parents cover every nonfactorized cubic
quotient after exact equal-momentum compression. This is input-topology cover,
not IBP closure and not an all-integer reduction claim.
"""
import collections
import argparse
import itertools
import json
from pathlib import Path
import time
from matroid import canonical, cycle_space, routing


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory',type=Path,default=Path(__file__).resolve().parent)
    directory=parser.parse_args().directory
    census=json.loads((directory/'classified_census.json').read_text())
    loops=census['loops']
    maximum=3*(loops-1)
    parents=[r for r in census['representatives'] if r['physical_edge_count']==maximum]
    wanted={r['canonical_matroid']:r for r in census['representatives']}
    witnesses={}
    started=time.monotonic()
    tested=collections.Counter()
    for parent in parents:
        for size in sorted({r['physical_edge_count'] for r in wanted.values()},reverse=True):
            if not any(r['physical_edge_count']==size for r in wanted.values()):
                continue
            for selected in itertools.combinations(range(maximum),size):
                rows=[parent['momenta'][i] for i in selected]
                tested['subsets']+=1
                if len(cycle_space(rows)) != 1<<loops:
                    continue
                key,order,_=canonical(rows)
                if key not in wanted:
                    continue
                representative=wanted.pop(key)
                canonical_rows=[rows[i] for i in order]
                witness=routing(representative['momenta'],canonical_rows)
                witnesses[representative['id']]=dict(parent=parent['id'],
                    target_slots=[selected[i] for i in order],
                    contracted_slots=[i for i in range(maximum) if i not in selected],
                    routing=witness)
                tested['covered']+=1
                if not any(r['physical_edge_count']==size for r in wanted.values()):
                    break
        print(json.dumps(dict(parent=parent['id'],covered=len(witnesses),remaining=len(wanted),
                              seconds=time.monotonic()-started)),flush=True)
        if not wanted:
            break
    result=dict(schema='rustred.offline-maximal-vacuum-cover.v1',loops=loops,
                maximal_parents=[p['id'] for p in parents],
                covered=len(witnesses),required=census['representative_count'],
                complete=not wanted,missing=[r['id'] for r in wanted.values()],
                tested=dict(tested),wall_seconds=time.monotonic()-started,witnesses=witnesses,
                integral_scope='all connected nonfactorized cubic graphs and their contractions',
                ibp_closure_claim=False)
    (directory/'maximal_parent_cover.json').write_text(json.dumps(result,indent=2)+'\n')
    if wanted:
        raise RuntimeError(f'{len(wanted)} cubic quotients remain outside verified parent cover')
    print(json.dumps({k:v for k,v in result.items()if k!='witnesses'},indent=2))


if __name__=='__main__':
    main()
