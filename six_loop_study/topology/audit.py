#!/usr/bin/env python3
"""Independent read-only replay of the offline topology input witnesses.

Unlike the producer's binary-cycle enumeration, this audit enumerates simple
cycles directly from graph adjacency. Symbolica supplies canonical labeling,
exact matrix products, ranks, determinants and expression comparison. This
checks input coverage only; it neither generates IBPs nor proves closure.
"""
import argparse
from collections import Counter
import copy
import hashlib
import json
from pathlib import Path
import time
import tomllib

from symbolica import E, Graph, Matrix


def require(condition, message):
    if not condition:
        raise ValueError(message)


def unique(records, label):
    result = {record['id']: record for record in records}
    require(len(result) == len(records), f'duplicate {label} IDs')
    return result


def rank(rows):
    return Matrix.from_nested(rows).row_reduce(len(rows[0]))


def quadratic(rows):
    n = len(rows[0])
    return [[row[i] * row[j] * (1 if i == j else 2)
             for i in range(n) for j in range(i, n)] for row in rows]


def graph_cycles(graph, loops, cubic=True):
    vertices, edges = graph['vertices'], graph['edges']
    require(len(set(vertices)) == len(vertices), 'duplicate vertices')
    require(len(edges) - len(vertices) + 1 == loops, 'wrong graph loop count')
    if cubic:
        require(len(vertices) == 2 * (loops - 1), 'wrong cubic vertex count')
        require(len(edges) == 3 * (loops - 1), 'wrong cubic edge count')
    require(len({e['id'] for e in edges}) == len(edges), 'duplicate edge IDs')
    adjacent = {v: [] for v in vertices}
    balance = {v: [0] * loops for v in vertices}
    for ordinal, edge in enumerate(edges):
        a, b, q = edge['source'], edge['target'], edge['momentum']
        require(a in adjacent and b in adjacent, 'unknown endpoint')
        require(len(q) == loops and all(type(n) is int for n in q), 'malformed momentum')
        adjacent[a].append((b, ordinal))
        adjacent[b].append((a, ordinal))
        for i, value in enumerate(q):
            balance[a][i] += value
            balance[b][i] -= value
    require(not cubic or all(len(a) == 3 for a in adjacent.values()), 'noncubic graph')
    require(all(not any(q) for q in balance.values()), 'momentum conservation failed')
    seen, pending = set(), [vertices[0]]
    while pending:
        vertex = pending.pop()
        if vertex not in seen:
            seen.add(vertex)
            pending.extend(other for other, _ in adjacent[vertex])
    require(seen == set(vertices), 'disconnected generated graph')
    rows = [edge['momentum'] for edge in edges]
    require(rank(rows) == loops, 'insufficient loop rank')
    units = []
    for i in range(loops) if cubic else ():
        vector = [int(j == i) for j in range(loops)]
        matches = [e['id'] for e in edges if e['momentum'] in (vector, [-n for n in vector])]
        require(matches, 'routing is not an authenticated fundamental cycle basis')
        units.append(matches[0])
    found = set()
    for start in vertices:
        def visit(vertex, visited, used):
            for other, edge in adjacent[vertex]:
                if edge in used:
                    continue
                if other == start:
                    found.add(frozenset((*used, edge)))
                elif other > start and other not in visited:
                    visit(other, visited | {other}, used | {edge})
        visit(start, {start}, set())
    return found, units


def components(edge_count, cycles):
    groups = [{i} for i in range(edge_count)]
    for cycle in cycles:
        joined = set(cycle)
        remaining = []
        for group in groups:
            if group & cycle:
                joined.update(group)
            else:
                remaining.append(group)
        groups = remaining + [joined]
    return sorted(sorted(group) for group in groups)


def canonical_key(edge_count, circuits):
    graph = Graph()
    for _ in range(edge_count):
        graph.add_node(0)
    for circuit in sorted(circuits, key=lambda c: (len(c), sorted(c))):
        node = graph.add_node(1)
        for edge in sorted(circuit):
            graph.add_edge(edge, node)
    graph, _, _, _ = graph.canonize()
    return json.dumps(dict(nodes=[int(str(data)) for _, data in graph.nodes()],
                           edges=sorted((a, b, directed, int(str(data)))
                                        for a, b, directed, data in graph.edges())),
                      separators=(',', ':'))


def verify_map(source, target, witness):
    loops = len(source[0])
    matrix = witness['matrix']
    require(len(source) == len(target), 'routing edge count differs')
    require(len(matrix) == loops and all(len(row) == loops for row in matrix), 'routing shape')
    require(all(type(value) is int for row in matrix for value in row), 'noninteger routing')
    native = Matrix.from_nested(matrix)
    determinant = int(str(native.det()))
    require(abs(determinant) == 1 and determinant == witness['determinant'], 'nonunimodular routing')
    signs = witness['source_to_target_signs']
    require(len(signs) == len(source) and all(type(s) is int and abs(s) == 1 for s in signs), 'routing signs')
    mapped = Matrix.from_nested(source) * native
    for i, (row, sign) in enumerate(zip(target, signs)):
        require(all(mapped[i, j] == sign * row[j] for j in range(loops)), 'momentum identity failed')


def audit_contractions(directory, loops, graphs, representatives):
    path = directory / 'contraction_census.json'
    if not path.exists():
        return None
    census = json.loads(path.read_text())
    require(census['complete'] and census['loops'] == loops and not census['closure_claim'], 'incomplete contraction census')
    by_id = unique(census['representatives'], 'contraction')
    require(len(by_id) == census['total_classes'], 'contraction count differs')
    method = census.get('enumeration_method', 'labelled-subsets')
    require(method in ('labelled-subsets', 'matroid-quotient-frontier'), 'unknown contraction enumeration method')
    quotient = method == 'matroid-quotient-frontier'
    successor_keys = set()
    keys, kind, histogram = set(), Counter(), {'factorized': Counter(), 'nonfactorized': Counter()}
    occurrences = Counter()
    for record in by_id.values():
        parent = representatives[record['representative_parent']]
        raw = graphs[parent['representative']]
        selected = record['representative_slots']
        require(len(selected) == len(set(selected)) == record['physical_edge_count'], 'invalid contraction slots')
        require(all(0 <= slot < len(parent['momenta']) for slot in selected), 'invalid contraction slot')
        kept_ids = [parent['edge_groups'][slot][0] for slot in selected]
        require(all(len(group) == 1 for group in parent['edge_groups']), 'parent still has duplicate momenta')
        roots = {v: v for v in raw['vertices']}
        def root(v):
            while roots[v] != v:
                v = roots[v]
            return v
        for edge in raw['edges']:
            if edge['id'] not in kept_ids:
                a, b = root(edge['source']), root(edge['target'])
                require(a != b, 'contraction removes a loop, not a forest')
                roots[a] = b
        edge_map = {e['id']: e for e in raw['edges']}
        edges = [{**edge_map[edge], 'source': root(edge_map[edge]['source']),
                  'target': root(edge_map[edge]['target'])} for edge in kept_ids]
        contracted = dict(vertices=sorted({root(v) for v in raw['vertices']}), edges=edges)
        cycles, _ = graph_cycles(contracted, loops, cubic=False)
        key = canonical_key(len(edges), cycles)
        require(key == record['canonical_matroid'] and key not in keys, 'contraction canonical key differs or duplicates')
        keys.add(key)
        classification = 'factorized' if len(components(len(edges), cycles)) != 1 else 'nonfactorized'
        require(classification == record['classification'], 'contraction factorization differs')
        kind[classification] += 1
        histogram[classification][str(len(edges))] += 1
        if quotient:
            # Independent exhaustiveness gate: use graph adjacency, not the
            # producer's projected binary cycle spaces. Every non-loop edge
            # is contractible without reducing the vacuum loop count.
            for ordinal, edge in enumerate(edges):
                a, b = edge['source'], edge['target']
                if a == b:
                    continue
                def merged(v):
                    return a if v == b else v
                child = dict(vertices=sorted({merged(v) for v in contracted['vertices']}),
                    edges=[{**other, 'source': merged(other['source']), 'target': merged(other['target'])}
                           for i, other in enumerate(edges) if i != ordinal])
                child_cycles, _ = graph_cycles(child, loops, cubic=False)
                successor_keys.add(canonical_key(len(child['edges']), child_cycles))
        else:
            require(sum(record['parent_occurrences'].values()) == record['labelled_occurrences'], 'contraction multiplicity differs')
            occurrences[classification + '_sectors'] += record['labelled_occurrences']
    require(dict(kind) == census['classes_by_kind'], 'contraction classification totals differ')
    require({k: dict(v) for k, v in histogram.items()} == census['classes_by_edge_count'], 'contraction edge histogram differs')
    if quotient:
        require(census.get('labelled_occurrences_enumerated') is False, 'unproved labelled multiplicities claimed')
        require(census['counts']['expanded_classes'] == len(keys), 'unexpanded contraction classes')
        require(successor_keys <= keys, 'contraction census omits a next-edge contraction')
        parent_keys = {r['canonical_matroid'] for r in representatives.values()
                       if len(r['momenta']) == 3*(loops-1)}
        require(parent_keys <= keys, 'contraction census omits a maximal parent')
    else:
        require(all(census['counts'][key] == value for key, value in occurrences.items()), 'contraction sector totals differ')
        require(sum(occurrences.values()) + census['counts']['rank_deficient_sectors']
                == census['counts']['labelled_sectors'], 'labelled contraction census accounting')
    return dict(representatives_replayed=len(by_id), classes_by_kind=dict(kind),
                classes_by_edge_count={k: dict(v) for k, v in histogram.items()},
                enumeration_method=method,
                next_edge_class_cover_checked=quotient,
                all_duplicate_routing_maps_replayed=False,
                sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def audit(directory):
    documents = {name: json.loads((directory / name).read_text()) for name in (
        'cubic_graphs.json', 'classified_census.json', 'maximal_parent_cover.json', 'manifest.json')}
    raw, census, cover, manifest = documents.values()
    loops = raw['loops']
    graphs, classified = unique(raw['graphs'], 'raw'), unique(census['graphs'], 'classified')
    representatives = unique(census['representatives'], 'representative')
    require(raw['graph_count'] == len(graphs) == census['generated_graphs'], 'graph count differs')
    require(set(graphs) == set(classified), 'classification omits generated graph')
    require(census['loops'] == cover['loops'] == manifest['loop_count'] == loops, 'loop count differs')
    require(raw['generation']['completed'] and not raw['closure_claim'], 'wrong generation authority')
    require(not census['closure_claim'] and not cover['ibp_closure_claim'], 'wrong census authority')
    aliases, signatures, classification_counts, simple = {}, {}, Counter(), 0
    for identifier, graph in graphs.items():
        cycles, units = graph_cycles(graph, loops)
        result = classified[identifier]
        blocks = components(len(graph['edges']), cycles)
        zero = [edge['id'] for edge in graph['edges'] if not any(edge['momentum'])]
        require(result['factor_blocks'] == blocks and result['zero_flow_edges'] == zero, 'factor classification differs')
        require(result['fundamental_unit_edges'] == units, 'fundamental basis evidence differs')
        pairs = [tuple(sorted((e['source'], e['target']))) for e in graph['edges']]
        simple += int(all(a != b for a, b in pairs) and len(set(pairs)) == len(pairs))
        factorized = bool(zero) or len(blocks) != 1
        require((result['classification'] == 'factorized') == factorized, 'invalid factorization decision')
        classification_counts[result['classification']] += 1
        if factorized:
            continue
        representative = representatives[result['topology']]
        groups, signs = result['edge_groups'], result['edge_group_signs']
        by_id = {edge['id']: (i, edge['momentum']) for i, edge in enumerate(graph['edges'])}
        require(len(groups) == len(signs) == result['physical_edge_count'], 'compressed shape')
        flattened = [edge for group in groups for edge in group]
        require(len(flattened) == len(set(flattened)) and set(flattened) == set(by_id), 'edge grouping incomplete')
        source, retained = [], {}
        for i, (group, group_signs) in enumerate(zip(groups, signs)):
            require(group and len(group) == len(group_signs), 'empty or malformed duplicate group')
            require(all(type(s) is int and abs(s) == 1 for s in group_signs), 'duplicate signs')
            row = [group_signs[0] * n for n in by_id[group[0]][1]]
            require(next(n for n in row if n) > 0, 'noncanonical duplicate representative')
            require(all([sign * n for n in by_id[edge][1]] == row
                        for edge, sign in zip(group, group_signs)), 'duplicate momentum identity failed')
            source.append(row)
            retained[by_id[group[0]][0]] = i
        projected = {frozenset(retained[i] for i in cycle if i in retained) for cycle in cycles}
        projected.discard(frozenset())
        minimal = {c for c in projected if not any(other < c for other in projected)}
        key = canonical_key(len(source), minimal)
        require(key == representative['canonical_matroid'], 'independent circuit census differs')
        require(rank(source) == loops and rank(quadratic(source)) == len(source), 'physical basis dependence')
        verify_map(source, representative['momenta'], result['routing'])
        aliases.setdefault(representative['id'], []).append(identifier)
        signatures[representative['id']] = key
    require(len(set(signatures.values())) == len(representatives), 'duplicate matroid representatives')
    require(dict(classification_counts) == census['classification_counts'], 'classification totals')
    for identifier, representative in representatives.items():
        require(sorted(aliases[identifier]) == sorted(representative['aliases']), 'alias inventory differs')
        require(representative['representative'] in aliases[identifier], 'representative outside alias class')
        require(representative['native_loop_rank'] == loops, 'reported loop rank')
        require(representative['native_quadratic_rank'] == len(representative['momenta']), 'reported physical rank')
    parents = {key for key, rep in representatives.items() if len(rep['momenta']) == 3 * (loops - 1)}
    require(set(cover['maximal_parents']) == parents, 'maximal parent list differs')
    require(cover['complete'] and not cover['missing'] and set(cover['witnesses']) == set(representatives), 'incomplete cover')
    for identifier, witness in cover['witnesses'].items():
        require(witness['parent'] in parents, 'cover target is not a maximal parent')
        parent = representatives[witness['parent']]['momenta']
        selected, removed = witness['target_slots'], witness['contracted_slots']
        require(len(selected + removed) == len(parent)
                and len(set(selected + removed)) == len(parent)
                and set(selected + removed) == set(range(len(parent))), 'invalid contraction partition')
        verify_map(representatives[identifier]['momenta'], [parent[i] for i in selected], witness['routing'])
    entries = unique(manifest['entries'], 'export')
    require(set(entries) == parents and manifest['parent_count'] == len(parents), 'export omits a parent')
    for identifier, entry in entries.items():
        parent = representatives[identifier]
        rows, physical, auxiliary = entry['momenta'], entry['physical_propagators'], entry['auxiliary_slots']
        arity = loops * (loops + 1) // 2
        require(len(rows) == arity and set(physical + auxiliary) == set(range(arity))
                and len(physical + auxiliary) == arity, 'coordinate partition')
        require([rows[i] for i in physical] == parent['momenta'], 'export physical momentum differs')
        require(rank(quadratic(rows)) == arity, 'exported basis is incomplete')
        require(entry['common_mass_squared'] == 1 and entry['denominator_convention'] == 'q^2-1', 'mass convention')
        definition = tomllib.loads((directory / entry['input']).read_text())
        require(definition['target']['powers'] == [int(i in physical) for i in range(arity)], 'corner target differs')
        require(len(definition['family']['denominators']) == arity, 'TOML arity')
        for denominator, row in zip(definition['family']['denominators'], rows):
            q = '+'.join(f'({value})*k{i+1}' for i, value in enumerate(row))
            require(E(denominator['expression']).expand() == E(f'({q})^2-1').expand(), 'TOML momentum transcription')
        graph = graphs[parent['representative']]
        require((directory / entry['dot']).read_bytes() == (directory / graph['dot']).read_bytes(), 'DOT provenance')
        by_id = {edge['id']: edge['momentum'] for edge in graph['edges']}
        require(len(entry['coordinate_edge_ids']) == len(entry['coordinate_edge_signs']) == len(physical), 'DOT coordinate witness shape')
        for slot, edge, sign in zip(physical, entry['coordinate_edge_ids'], entry['coordinate_edge_signs']):
            require(sign in (-1, 1) and [sign * n for n in by_id[edge]] == rows[slot], 'DOT coordinate witness')
    probe = next(iter(cover['witnesses']))
    witness = cover['witnesses'][probe]
    source = representatives[probe]['momenta']
    target = [representatives[witness['parent']]['momenta'][i] for i in witness['target_slots']]
    rejected = 0
    for mode in ('matrix', 'determinant', 'sign', 'target'):
        bad, other = copy.deepcopy(witness['routing']), copy.deepcopy(target)
        if mode == 'matrix':
            bad['matrix'][0][0] += 1
        elif mode == 'determinant':
            bad['determinant'] *= -1
        elif mode == 'sign':
            bad['source_to_target_signs'][0] *= -1
        else:
            other[0][0] += 1
        try:
            verify_map(source, other, bad)
        except ValueError:
            rejected += 1
        else:
            raise ValueError(f'corrupted {mode} witness accepted')
    contraction_audit = audit_contractions(directory, loops, graphs, representatives)
    return dict(status='PASS', loops=loops, raw_graphs=len(graphs), simple_cubic_graphs=simple,
                classifications=dict(classification_counts), nonfactorized_representatives=len(representatives),
                exact_alias_maps=sum(len(v) for v in aliases.values()), exact_parent_maps=len(cover['witnesses']),
                exported_parents=len(entries), mutation_controls_rejected=rejected,
                contraction_audit=contraction_audit,
                input_sha256={name: hashlib.sha256((directory / name).read_bytes()).hexdigest() for name in documents},
                ibp_generation_performed=False, closure_claim=False)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args()
    started = time.monotonic()
    result = audit(args.directory)
    result['wall_seconds'] = time.monotonic() - started
    print(json.dumps(result, indent=2))
