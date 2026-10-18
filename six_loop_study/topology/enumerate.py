#!/usr/bin/env python3
"""Offline complete cubic vacuum graph generation through existing HEPKit.

No IBP generation; no modifications to the RustRed engine or live campaigns.
Graph enumeration/canonicalization and routing are native FeynKit/Symbolica.
"""
import argparse
import json
from pathlib import Path
import time

from symbolica.community import hepkit as hep


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--loops', type=int, default=6)
    parser.add_argument('--output', type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument('--threads', type=int, default=1)
    args = parser.parse_args()
    if args.loops < 2:
        parser.error('this cubic-vacuum census requires at least two loops')
    directory = args.output
    directory.mkdir(parents=True, exist_ok=True)
    (directory / 'raw_dot').mkdir(exist_ok=True)
    started = time.monotonic()
    model = hep.Model.phi3()
    process = model.process([], [])
    options = dict(loops=args.loops, threads=args.threads,
                   max_vertices=2*(args.loops-1), allow_self_loops=True,
                   allow_zero_flow_edges=True, maximum_bridges=None,
                   self_energy=None, tadpoles=None, zero_snails=None,
                   factorized_loop_topologies_count_range=None,
                   numerator_grouping=None, filter_zero_color=False, progress=None)
    generated = process.generate_diagrams(**options)
    if not generated.report.completed:
        raise RuntimeError('incomplete graph generation cannot establish a census')
    records = []
    for ordinal, diagram in enumerate(generated):
        identifier = f'cubic-{ordinal:04d}'
        edges = []
        for edge in diagram.internal_edges:
            loops, external = edge.momentum_signature().integer_coefficients()
            assert not external
            edges.append(dict(id=edge.id, source=edge.source, target=edge.target,
                              momentum=loops))
        assert len(edges) == 3*(args.loops-1)
        assert len(diagram.vertices) == 2*(args.loops-1)
        assert diagram.loop_count == args.loops
        dot = f'raw_dot/{identifier}.dot'
        (directory / dot).write_text(diagram.to_dot())
        records.append(dict(id=identifier, graph_id=str(diagram.id),
                            vertices=[v.id for v in diagram.vertices], edges=edges,
                            loop_count=diagram.loop_count, symmetry_factor=diagram.symmetry_factor,
                            dot=dot))
    result = dict(schema='rustred.offline-cubic-vacuum-census.v1', loops=args.loops,
                  generation=dict(engine='HEPKit Model.phi3/FeynKit/Symbolica', options=options,
                                  completed=True, report=str(generated.report),
                                  wall_seconds=time.monotonic()-started),
                  graph_count=len(records), graphs=records,
                  closure_claim=False, classification_complete=False)
    destination = directory / 'cubic_graphs.json'
    destination.write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps({key: result[key] for key in ['schema','loops','generation','graph_count']}, indent=2))


if __name__ == '__main__':
    main()
