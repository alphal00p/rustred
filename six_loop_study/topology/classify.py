#!/usr/bin/env python3
"""Classify the offline cubic graph census; all identifications receive exact
native routing witnesses. No integral equations are generated or reduced.
"""
import collections
import argparse
import hashlib
import json
from pathlib import Path
import time
from matroid import validate_cycle_basis, factor_blocks, compress_equal_momenta, canonical, routing, native_rank


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory',type=Path,default=Path(__file__).resolve().parent)
    directory=parser.parse_args().directory
    raw = json.loads((directory / 'cubic_graphs.json').read_text())
    started = time.monotonic()
    classes = {}
    identifiers=set()
    classified = []
    for graph in raw['graphs']:
        units = validate_cycle_basis(graph)
        original = [edge['momentum'] for edge in graph['edges']]
        zero = [edge['id'] for edge in graph['edges'] if not any(edge['momentum'])]
        blocks = factor_blocks(original)
        result = dict(id=graph['id'], fundamental_unit_edges=units,
                      zero_flow_edges=zero, factor_blocks=blocks)
        if zero or len(blocks) != 1:
            result['classification'] = 'factorized'
            result['reason'] = 'zero-flow bridge and factorized blocks' if zero else 'cycle-matroid direct sum'
            classified.append(result)
            continue
        rows, grouped, signs = compress_equal_momenta(graph)
        key, order, automorphisms = canonical(rows)
        rows = [rows[i] for i in order]
        grouped = [grouped[i] for i in order]
        signs = [signs[i] for i in order]
        if key not in classes:
            identifier = 'topology-' + hashlib.sha256(key.encode()).hexdigest()[:16]
            if identifier in identifiers:
                raise RuntimeError('canonical identifier collision; refuse ambiguous output paths')
            identifiers.add(identifier)
            # Native matrix rank checks physical inverse propagators as well
            # as full loop space; distinct momenta alone is not sufficient.
            quadratic = [[row[i]*row[j]*(1 if i == j else 2)
                          for i in range(raw['loops']) for j in range(i, raw['loops'])]
                         for row in rows]
            loop_rank=native_rank(rows)
            quadratic_rank=native_rank(quadratic)
            if loop_rank!=raw['loops'] or quadratic_rank!=len(rows):
                raise RuntimeError('compressed physical family needs explicit dependent treatment')
            classes[key] = dict(id=identifier, representative=graph['id'],
                                physical_edge_count=len(rows), momenta=rows,
                                native_loop_rank=loop_rank,
                                native_quadratic_rank=quadratic_rank,
                                canonical_matroid=key, edge_automorphisms=automorphisms,
                                edge_groups=grouped, edge_group_signs=signs, aliases=[])
        representative = classes[key]
        witness = routing(rows, representative['momenta'])
        representative['aliases'].append(graph['id'])
        result.update(classification='nonfactorized', topology=representative['id'],
                      physical_edge_count=len(rows), edge_groups=grouped,
                      edge_group_signs=signs, routing=witness)
        classified.append(result)
    representatives = sorted(classes.values(), key=lambda c: (-c['physical_edge_count'], c['id']))
    result = dict(schema='rustred.offline-vacuum-matroid-census.v1', loops=raw['loops'],
                  generated_graphs=raw['graph_count'],
                  classification_counts=dict(collections.Counter(g['classification'] for g in classified)),
                  representative_count=len(representatives),
                  representatives_by_edges=dict(collections.Counter(c['physical_edge_count'] for c in representatives)),
                  representatives=representatives, graphs=classified,
                  complete_generated_cubic_classification=True, maximal_parent_cover_verified=False,
                  wall_seconds=time.monotonic()-started, closure_claim=False)
    (directory / 'classified_census.json').write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps({k:v for k,v in result.items() if k not in ('graphs','representatives')}, indent=2))


if __name__ == '__main__':
    main()
