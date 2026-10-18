"""Offline finite graph subset operations; native Symbolica owns canonical
graphs and all exact rational matrix operations. No polynomial/CAS kernel.
"""
import itertools
import json
from symbolica import Graph, Matrix


def validate_cycle_basis(record):
    loops = record['loop_count']
    vertices=set(record['vertices'])
    if len(vertices)!=len(record['vertices']) or len(record['edges'])-len(vertices)+1!=loops:
        raise ValueError('invalid connected vacuum Euler counts')
    adjacency={vertex:set()for vertex in vertices}
    degree={vertex:0 for vertex in vertices}
    ids=set()
    for edge in record['edges']:
        if edge['id'] in ids or edge['source']not in vertices or edge['target']not in vertices:
            raise ValueError('invalid edge identity or endpoints')
        ids.add(edge['id'])
        if len(edge['momentum'])!=loops or any(type(n)is not int for n in edge['momentum']):
            raise ValueError('invalid native integer routing row')
        adjacency[edge['source']].add(edge['target'])
        adjacency[edge['target']].add(edge['source'])
        degree[edge['source']]+=1
        degree[edge['target']]+=1
    if any(d!=3 for d in degree.values()):
        raise ValueError('raw generating graph is not cubic')
    reached=set(); queue=[next(iter(vertices))]
    while queue:
        vertex=queue.pop()
        if vertex not in reached:
            reached.add(vertex);queue.extend(adjacency[vertex]-reached)
    if reached!=vertices:
        raise ValueError('raw generating graph is disconnected')
    for vertex in record['vertices']:
        balance = [0] * loops
        for edge in record['edges']:
            for i, value in enumerate(edge['momentum']):
                balance[i] += value * ((edge['source'] == vertex) - (edge['target'] == vertex))
        if any(balance):
            raise ValueError(f'nonconserved native routing at {vertex}')
    unit_edges = []
    for i in range(loops):
        unit = tuple(int(j == i) for j in range(loops))
        matches = [e['id'] for e in record['edges']
                   if tuple(e['momentum']) in (unit, tuple(-n for n in unit))]
        if not matches:
            raise ValueError('native loop basis has no fundamental unit edge')
        unit_edges.append(matches[0])
    return unit_edges


def cycle_space(rows):
    loops = len(rows[0]) if rows else 0
    columns = [sum((row[j] & 1) << i for i, row in enumerate(rows)) for j in range(loops)]
    vectors = [0]
    for column in columns:
        vectors += [old ^ column for old in vectors]
    return set(vectors)


def circuits(rows):
    space = sorted(cycle_space(rows) - {0}, key=lambda mask: (mask.bit_count(), mask))
    minimal = []
    for mask in space:
        if not any(previous & mask == previous for previous in minimal):
            minimal.append(mask)
    return minimal


def factor_blocks(rows):
    groups = [{i} for i in range(len(rows))]
    for circuit in circuits(rows):
        edges = {i for i in range(len(rows)) if circuit >> i & 1}
        touching = [g for g in groups if g & edges]
        groups = [g for g in groups if not g & edges]
        groups.append(set().union(*touching))
    return sorted([sorted(g) for g in groups])


def canonical(rows):
    graph = Graph()
    for _ in rows:
        graph.add_node(0)
    for circuit in circuits(rows):
        node = graph.add_node(1)
        for edge in range(len(rows)):
            if circuit >> edge & 1:
                graph.add_edge(edge, node)
    graph, permutation, automorphisms, _ = graph.canonize()
    edge_order = sorted(range(len(rows)), key=permutation.__getitem__)
    exact_key = json.dumps(dict(nodes=[int(str(data)) for _, data in graph.nodes()],
                               edges=sorted((a,b,d,int(str(v))) for a,b,d,v in graph.edges())),
                           separators=(',', ':'))
    return exact_key, edge_order, int(str(automorphisms))


def compress_equal_momenta(record):
    rows, groups, signs = [], [], []
    lookup = {}
    for edge in record['edges']:
        row = tuple(edge['momentum'])
        first = next((n for n in row if n), 0)
        if first == 0:
            raise ValueError('zero-flow edge belongs in factorized classification')
        sign = 1 if first > 0 else -1
        normalized = tuple(sign*n for n in row)
        if normalized not in lookup:
            lookup[normalized] = len(rows)
            rows.append(list(normalized))
            groups.append([])
            signs.append([])
        ordinal = lookup[normalized]
        groups[ordinal].append(edge['id'])
        signs[ordinal].append(sign)
    return rows, groups, signs


def native_rank(rows):
    matrix = Matrix.from_nested(rows)
    return matrix.row_reduce(len(rows[0]))


def routing(source, target):
    """Find and exactly verify q_source R = signed q_target, det(R)=±1.

    Both lists already share canonical edge order; enumerate only signs of a
    native-independent row basis. Symbolica owns rank, inverse and products.
    """
    loops = len(source[0])
    basis = []
    for i in range(len(source)):
        if native_rank([source[j] for j in basis + [i]]) > len(basis):
            basis.append(i)
        if len(basis) == loops:
            break
    if len(basis) != loops:
        raise ValueError('candidate has insufficient loop rank')
    inverse = Matrix.from_nested([source[i] for i in basis]).inv()
    whole = Matrix.from_nested(source)
    for signs in itertools.product((1, -1), repeat=max(0, loops-1)):
        signs = (1,) + signs  # Overall sign of all loop momenta is redundant.
        right = Matrix.from_nested([[sign*v for v in target[i]] for sign, i in zip(signs, basis)])
        transform = inverse * right
        try:
            determinant = int(str(transform.det()))
            values = [[int(str(transform[i, j])) for j in range(loops)] for i in range(loops)]
        except ValueError:
            continue
        if abs(determinant) != 1:
            continue
        mapped = whole * transform
        edge_signs = []
        for i, expected in enumerate(target):
            actual = [int(str(mapped[i, j])) for j in range(loops)]
            if actual == expected:
                edge_signs.append(1)
            elif actual == [-n for n in expected]:
                edge_signs.append(-1)
            else:
                break
        else:
            return dict(matrix=values, determinant=determinant,
                        source_to_target_signs=edge_signs, basis_rows=basis)
    raise ValueError('native graph-matroid proposal failed exact unimodular routing')
