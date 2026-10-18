#!/usr/bin/env python3
"""Offline full physical-sector census under the verified maximal parents.

Uses exact native Symbolica circuit-incidence canonicalization for matroids.
Representative parent embeddings are explicit; this does NOT construct a
separate momentum-routing matrix for every duplicate labelled sector.
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import time
from symbolica import Graph
from matroid import cycle_space


def minimal_circuits(flows):
    result=[]
    for mask in sorted(flows-{0},key=lambda m:(m.bit_count(),m)):
        if not any(previous&mask==previous for previous in result):
            result.append(mask)
    return result


def blocks(selected,circuits):
    groups=[]
    for circuit in circuits:
        merged=circuit
        keep=[]
        for group in groups:
            if group&merged:
                merged|=group
            else:
                keep.append(group)
        groups=keep+[merged]
    covered=0
    for group in groups:
        covered|=group
    # Full-rank contracted vacuum graphs have no zero-flow bridges; retain
    # singleton elements defensively rather than dropping them silently.
    for i in range(selected.bit_length()):
        if selected>>i&1 and not covered>>i&1:
            groups.append(1<<i)
    return groups


def canonical(selected,circuits):
    edges=[i for i in range(selected.bit_length())if selected>>i&1]
    graph=Graph()
    for _ in edges:
        graph.add_node(0)
    for circuit in circuits:
        node=graph.add_node(1)
        for local,edge in enumerate(edges):
            if circuit>>edge&1:
                graph.add_edge(local,node)
    graph,permutation,_,_=graph.canonize()
    key=json.dumps(dict(nodes=[int(str(data))for _,data in graph.nodes()],
                        edges=sorted((a,b,d,int(str(v)))for a,b,d,v in graph.edges())),
                   separators=(',',':'))
    return key,[edges[i]for i in sorted(range(len(edges)),key=permutation.__getitem__)]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest',type=Path,default=Path(__file__).resolve().parent/'manifest.json')
    parser.add_argument('--max-seconds',type=float,default=None,
                        help='optional explicit diagnostic deadline, not a solver limit')
    args=parser.parse_args()
    manifest=json.loads(args.manifest.read_text())
    loops=manifest['loop_count']
    started=time.monotonic(); classes={}; identities={}; counts=collections.Counter()
    complete=True; parents_done=[]
    for parent in manifest['entries']:
        physical=parent['physical_propagators']
        rows=[parent['momenta'][i]for i in physical]
        space=cycle_space(rows)
        assert len(space)==1<<loops
        for selected in range(1<<len(rows)):
            if args.max_seconds is not None and counts['labelled_sectors']%2048==0 and time.monotonic()-started>args.max_seconds:
                complete=False;break
            counts['labelled_sectors']+=1
            if selected.bit_count()<loops:
                counts['rank_deficient_sectors']+=1;continue
            projected={flow&selected for flow in space}
            if len(projected)!=1<<loops:
                counts['rank_deficient_sectors']+=1;continue
            circuits=minimal_circuits(projected)
            factored=len(blocks(selected,circuits))>1
            classification='factorized'if factored else'nonfactorized'
            counts[classification+'_sectors']+=1
            key,slots=canonical(selected,circuits)
            if key not in classes:
                identifier='contraction-'+hashlib.sha256(key.encode()).hexdigest()[:16]
                if identifier in identities and identities[identifier]!=key:
                    raise RuntimeError('canonical contraction identifier collision')
                identities[identifier]=key
                classes[key]=dict(id=identifier,classification=classification,
                                  physical_edge_count=len(slots),representative_parent=parent['id'],
                                  representative_slots=[physical[i]for i in slots],
                                  canonical_matroid=key,labelled_occurrences=0,parent_occurrences={})
            entry=classes[key]
            assert entry['classification']==classification
            entry['labelled_occurrences']+=1
            entry['parent_occurrences'][parent['id']]=entry['parent_occurrences'].get(parent['id'],0)+1
        if not complete:break
        parents_done.append(parent['id'])
        print(json.dumps(dict(parent=parent['id'],parents_done=len(parents_done),classes=len(classes),
                              labelled_sectors=counts['labelled_sectors'],seconds=time.monotonic()-started)),flush=True)
    values=sorted(classes.values(),key=lambda e:(-e['physical_edge_count'],e['classification'],e['id']))
    classified=collections.Counter(entry['classification']for entry in values)
    edges={kind:dict(collections.Counter(e['physical_edge_count']for e in values if e['classification']==kind))
           for kind in ['nonfactorized','factorized']}
    result=dict(schema='rustred.offline-parent-contraction-matroids.v1',loops=loops,complete=complete,
                parents_done=parents_done,counts=dict(counts),classes_by_kind=dict(classified),
                classes_by_edge_count=edges,total_classes=len(values),representatives=values,
                wall_seconds=time.monotonic()-started,all_duplicate_gl_routing_witnesses=False,
                authority='exact graph-matroid census only; no reduction or numerical master claims',
                closure_claim=False)
    destination=args.manifest.parent/'contraction_census.json'
    destination.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:v for k,v in result.items()if k not in ['representatives','parents_done']},indent=2))


if __name__=='__main__':
    main()
