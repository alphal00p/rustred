#!/usr/bin/env python3
"""Plan physics-scoped owner-domain root queries for L-loop vacuum renormalization.

Role: offline input planning only; native admission remains the coverage
authority. This script emits `rustred.owner-domain-queries.json.v2` rows plus
receipts. It never dispatches a solver, never clips descendants, and never
claims family closure (`family_closure_claim` stays false everywhere).

Method (generic in the loop count L and in the vertex-degree set; no topology
names enter the computation):

1. Each owner mask selects slots of an L-loop vacuum family. Its realization is
   obtained by contracting the missing slots of a parent-vertex witness whose
   slot set contains the mask. Witnesses are validated against the momenta
   manifest (zero signed momentum sum at every vertex, each slot twice with
   opposite signs, E - V + 1 = L, connected, momentum rank L).
2. The 2-isomorphism class of the realization (Whitney twists at two-vertex
   cuts, cleaving at cut vertices, vertex identification across components) is
   closed; connected members are recorded as canonical unlabelled multigraphs.
3. Connected bridgeless L-loop vacuum skeletons with vertex degrees in the
   configured set are enumerated by backtracking over symmetric multiplicity
   matrices, deduplicated by canonical form, and reduced by merging every
   series class to one edge. A skeleton whose reduced form is a member of an
   owner's class certifies that owner as entry-capable; V4min is the minimal
   degree excess sum(deg - 3) (the quartic vertex count for degrees {3, 4}).
4. Bounds (exact difference D, K gauge-parameter powers):
   connected root  R_max = D - L + 1 - V4min + K, A_max = R_max + D;
   nested factorized root  A <= 5L - 1 - V4min + K, R <= 3L - V4min + K,
   D >= 2L - 1; non-entry owners use the connected formulas at V4min = 0.
   Per-coordinate uppers are A_max - t on active axes and R_max on inactive
   axes in local coordinates (positive local x = n - 1, inactive local y = -n).
5. One full-orthant helper per owner at the largest root rank precedes the
   owner's roots (descending R_max). The Rust `entry-domain-plan` command
   counts the finite starting targets per budget group when an executable is
   supplied; the checker script re-derives every number independently.

Outputs are deterministic: sorted iteration, canonical forms as integer
tuples, `json.dumps(sort_keys=True, indent=2)`. Only `timing.json` carries
wall-clock measurements; every other output is byte-identical across runs.
"""
from __future__ import annotations

import argparse
from collections import deque
from fractions import Fraction
import hashlib
import itertools
import json
import math
from pathlib import Path
import re
import subprocess
import sys
import time

QUERY_SCHEMA = "rustred.owner-domain-queries.json.v2"
RECEIPT_SCHEMA = "rustred.renormalization-entry-plan.json.v1"
CLASSIFICATION_SCHEMA = "rustred.vacuum-skeleton-classification.json.v1"
WITNESS_SCHEMA = "rustred.vacuum-parent-vertices.json.v1"
ENTRY_SPEC_SCHEMA = "rustred.entry-domain.json.v1"
ENTRY_PLAN_SCHEMA = "rustred.entry-domain-plan.json.v1"
ROLE = "offline input planning; native admission remains the coverage authority"
HELPER_PREFIX = "owner-anchor-"
MAX_ID_BYTES = 128
CLASS_CONNECTED = "connected"
CLASS_FACTORIZED = "factorized"
CLASS_NON_ENTRY = "non_entry"
sys.setrecursionlimit(10000)


class PlanError(ValueError):
    """Any refusal; the planner never emits partial outputs."""


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise PlanError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def load_json(path):
    try:
        return json.loads(Path(path).read_bytes(), object_pairs_hook=unique_object)
    except (OSError, ValueError) as error:
        raise PlanError(f"cannot read JSON {path}: {error}") from error


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def sha256_file(path):
    return sha256_bytes(Path(path).read_bytes())


def dumps(document):
    return json.dumps(document, sort_keys=True, indent=2, allow_nan=False) + "\n"


# ---------------------------------------------------------------------------
# Momenta and witnesses
# ---------------------------------------------------------------------------

MOMENTUM_TERM = re.compile(r"([+-]?)k(\d+)")


def parse_momentum(text, loops):
    """'k1+k2-k4' -> integer vector over the L loop momenta."""
    if not isinstance(text, str) or not text or not re.fullmatch(r"([+-]?k\d+)+", text.replace(" ", "")):
        raise PlanError(f"unparseable momentum {text!r}")
    vector = [0] * loops
    for sign, index in MOMENTUM_TERM.findall(text.replace(" ", "")):
        index = int(index)
        if not 1 <= index <= loops:
            raise PlanError(f"momentum {text!r} uses k{index} outside 1..{loops}")
        vector[index - 1] += -1 if sign == "-" else 1
    if not any(vector):
        raise PlanError(f"momentum {text!r} is zero")
    return tuple(vector)


def load_momenta(document, loops):
    """Return {slot: vector} for the momenta manifest; validates shape."""
    if not isinstance(document, dict):
        raise PlanError("momenta manifest must be an object")
    if document.get("loop_count") != loops:
        raise PlanError(f"momenta manifest loop_count {document.get('loop_count')!r} differs from --loops {loops}")
    rows = document.get("momenta")
    if not isinstance(rows, list) or not rows:
        raise PlanError("momenta manifest must list momenta")
    coordinate_count = document.get("coordinate_count")
    if coordinate_count != len(rows):
        raise PlanError("momenta manifest coordinate_count must equal the momenta list length")
    momenta = {}
    for row in rows:
        if not isinstance(row, dict) or type(row.get("index_one_based")) is not int:
            raise PlanError("each momentum row needs an integer index_one_based")
        slot = row["index_one_based"]
        if slot in momenta or not 1 <= slot <= len(rows):
            raise PlanError(f"momentum slot {slot} duplicated or out of range")
        momenta[slot] = parse_momentum(row.get("momentum"), loops)
    if sorted(momenta) != list(range(1, len(rows) + 1)):
        raise PlanError("momentum slots must be exactly 1..coordinate_count")
    if len(set(momenta.values())) != len(momenta):
        raise PlanError("momenta must be pairwise distinct")
    return momenta


def sector_bits(sector_id, width):
    if type(sector_id) is not int or not 0 <= sector_id < 2 ** width:
        raise PlanError(f"sector id {sector_id!r} is not an unsigned {width}-bit integer")
    return format(sector_id, f"0{width}b")


def rational_rank(vectors):
    rows = [[Fraction(x) for x in v] for v in vectors]
    rank = 0
    columns = len(rows[0]) if rows else 0
    for column in range(columns):
        pivot = next((r for r in range(rank, len(rows)) if rows[r][column] != 0), None)
        if pivot is None:
            continue
        rows[rank], rows[pivot] = rows[pivot], rows[rank]
        for r in range(len(rows)):
            if r != rank and rows[r][column] != 0:
                factor = rows[r][column] / rows[rank][column]
                rows[r] = [a - factor * b for a, b in zip(rows[r], rows[rank])]
        rank += 1
    return rank


def validate_witnesses(document, momenta, loops):
    """Validate the parent witnesses against the momenta; return {sector_id: vertices}."""
    width = len(momenta)
    if not isinstance(document, dict) or document.get("schema") != WITNESS_SCHEMA:
        raise PlanError(f"parent witnesses must carry schema {WITNESS_SCHEMA}")
    if document.get("loop_count") != loops or document.get("coordinate_count") != width:
        raise PlanError("parent witnesses loop_count/coordinate_count disagree with the momenta manifest")
    parents = document.get("parents")
    if not isinstance(parents, list) or not parents:
        raise PlanError("parent witnesses must list parents")
    result = {}
    for parent in parents:
        if not isinstance(parent, dict):
            raise PlanError("each parent witness must be an object")
        sector_id = parent.get("sector_id")
        bits = sector_bits(sector_id, width)
        if parent.get("sector_bits") != bits:
            raise PlanError(f"parent {sector_id}: sector_bits {parent.get('sector_bits')!r} != {bits}")
        if sector_id in result:
            raise PlanError(f"parent {sector_id} listed twice")
        vertices = parent.get("vertices")
        if not isinstance(vertices, list) or len(vertices) < 1:
            raise PlanError(f"parent {sector_id}: vertices must be a nonempty list")
        occurrences = {}
        for vertex in vertices:
            if not isinstance(vertex, list) or len(vertex) < 2:
                raise PlanError(f"parent {sector_id}: every vertex needs at least two slots")
            total = [0] * loops
            for signed in vertex:
                if type(signed) is not int or signed == 0 or abs(signed) > width:
                    raise PlanError(f"parent {sector_id}: slot entry {signed!r} out of range")
                occurrences.setdefault(abs(signed), []).append(1 if signed > 0 else -1)
                vector = momenta[abs(signed)]
                sign = 1 if signed > 0 else -1
                total = [a + sign * b for a, b in zip(total, vector)]
            if any(total):
                raise PlanError(f"parent {sector_id}: vertex {vertex} has nonzero momentum sum")
        expected = {slot + 1 for slot, bit in enumerate(bits) if bit == "1"}
        if set(occurrences) != expected:
            raise PlanError(f"parent {sector_id}: vertex slots {sorted(occurrences)} differ from sector bits")
        for slot, signs in sorted(occurrences.items()):
            if sorted(signs) != [-1, 1]:
                raise PlanError(f"parent {sector_id}: slot {slot} must occur exactly twice with opposite signs")
        edge_count, vertex_count = len(occurrences), len(vertices)
        if edge_count - vertex_count + 1 != loops:
            raise PlanError(f"parent {sector_id}: E - V + 1 = {edge_count - vertex_count + 1} != {loops}")
        ends = {}
        for index, vertex in enumerate(vertices):
            for signed in vertex:
                ends.setdefault(abs(signed), []).append(index)
        if not is_connected(vertex_count, list(ends.values())):
            raise PlanError(f"parent {sector_id}: witness graph is disconnected")
        if rational_rank([momenta[slot] for slot in sorted(occurrences)]) != loops:
            raise PlanError(f"parent {sector_id}: momentum rank is not {loops}")
        result[sector_id] = vertices
    return result


def is_connected(vertex_count, edge_pairs):
    if vertex_count == 0:
        return False
    adjacency = [[] for _ in range(vertex_count)]
    for a, b in edge_pairs:
        adjacency[a].append(b)
        adjacency[b].append(a)
    seen = {0}
    stack = [0]
    while stack:
        x = stack.pop()
        for y in adjacency[x]:
            if y not in seen:
                seen.add(y)
                stack.append(y)
    return len(seen) == vertex_count


# ---------------------------------------------------------------------------
# Labelled multigraphs: {slot: (a, b)}
# ---------------------------------------------------------------------------

def realization(slots, parents, preferred=None):
    """Contract the missing slots of a containing parent; returns (edges, parent_id)."""
    order = sorted(parents)
    if preferred in parents:
        order.remove(preferred)
        order.insert(0, preferred)
    wanted = set(slots)
    for parent_id in order:
        vertices = parents[parent_id]
        ends = {}
        for index, vertex in enumerate(vertices):
            for signed in vertex:
                ends.setdefault(abs(signed), []).append(index)
        if not wanted <= set(ends):
            continue
        root = list(range(len(vertices)))

        def find(x):
            while root[x] != x:
                root[x] = root[root[x]]
                x = root[x]
            return x

        for slot in sorted(set(ends) - wanted):
            a, b = (find(v) for v in ends[slot])
            if a != b:
                root[a] = b
        edges = {slot: tuple(find(v) for v in ends[slot]) for slot in sorted(wanted)}
        used = sorted({v for pair in edges.values() for v in pair})
        renumber = {v: i for i, v in enumerate(used)}
        return {slot: (renumber[a], renumber[b]) for slot, (a, b) in edges.items()}, parent_id
    return None, None


def labelled_key(edges):
    incidence = {}
    for slot, (a, b) in edges.items():
        incidence.setdefault(a, []).append(slot)
        incidence.setdefault(b, []).append(slot)
    return tuple(sorted(tuple(sorted(l)) for l in incidence.values()))


def vertex_set(edges):
    return {v for pair in edges.values() for v in pair}


def components(edges, removed=()):
    vertices = vertex_set(edges) - set(removed)
    root = {v: v for v in vertices}

    def find(x):
        while root[x] != x:
            root[x] = root[root[x]]
            x = root[x]
        return x

    for a, b in edges.values():
        if a in vertices and b in vertices:
            ra, rb = find(a), find(b)
            if ra != rb:
                root[ra] = rb
    groups = {}
    for v in sorted(vertices):
        groups.setdefault(find(v), []).append(v)
    return sorted(groups.values())


def pieces(edges, cut):
    result = []
    for component in components(edges, cut):
        member = set(component)
        result.append(sorted(slot for slot, (a, b) in edges.items() if a in member or b in member))
    for slot, (a, b) in sorted(edges.items()):
        if a in cut and b in cut:
            result.append([slot])
    return result


def whitney_neighbors(edges):
    """Twists at two-vertex cuts, cleavings at cut vertices, identifications across components."""
    out = []
    vertices = sorted(vertex_set(edges))
    fresh = max(vertices) + 1
    for u, v in itertools.combinations(vertices, 2):
        parts = pieces(edges, {u, v})
        if len(parts) >= 2:
            swap = {u: v, v: u}
            for part in parts:
                new = dict(edges)
                for slot in part:
                    a, b = edges[slot]
                    new[slot] = (swap.get(a, a), swap.get(b, b))
                out.append(new)
    for x in vertices:
        parts = pieces(edges, {x})
        if len(parts) >= 2:
            for part in parts:
                new = dict(edges)
                for slot in part:
                    a, b = edges[slot]
                    new[slot] = (fresh if a == x else a, fresh if b == x else b)
                out.append(new)
    groups = components(edges)
    if len(groups) >= 2:
        for c1, c2 in itertools.combinations(groups, 2):
            for a in c1:
                for b in c2:
                    out.append({slot: (a if p == b else p, a if q == b else q) for slot, (p, q) in edges.items()})
    return out


def two_isomorphism_closure(edges):
    """Return (connected canonical unlabelled forms, state count, connected state count)."""
    seen = {labelled_key(edges)}
    queue = deque([edges])
    forms = set()
    connected_states = 0
    while queue:
        graph = queue.popleft()
        if len(components(graph)) == 1:
            connected_states += 1
            forms.add(canonical_form(list(graph.values())))
        for neighbor in whitney_neighbors(graph):
            key = labelled_key(neighbor)
            if key not in seen:
                seen.add(key)
                queue.append(neighbor)
    return forms, len(seen), connected_states


# ---------------------------------------------------------------------------
# Unlabelled multigraphs with loops: canonical adjacency matrix
# ---------------------------------------------------------------------------

def canonical_form(edge_pairs):
    """Lexicographically minimal adjacency matrix over an individualization-refinement tree."""
    vertices = sorted({v for pair in edge_pairs for v in pair})
    index = {v: i for i, v in enumerate(vertices)}
    n = len(vertices)
    matrix = [[0] * n for _ in range(n)]
    for a, b in edge_pairs:
        a, b = index[a], index[b]
        if a == b:
            matrix[a][a] += 1
        else:
            matrix[a][b] += 1
            matrix[b][a] += 1
    neighbors = [[j for j in range(n) if j != i and matrix[i][j]] for i in range(n)]

    def relabel(values):
        keys = sorted(set(values))
        rel = {k: i for i, k in enumerate(keys)}
        return [rel[x] for x in values]

    def refine(colour):
        while True:
            signature = [(colour[i], tuple(sorted((colour[j], matrix[i][j]) for j in neighbors[i])))
                         for i in range(n)]
            new = relabel(signature)
            if len(set(new)) == len(set(colour)):
                return new
            colour = new

    best = [None]

    def search(colour):
        cells = {}
        for i, c in enumerate(colour):
            cells.setdefault(c, []).append(i)
        if len(cells) == n:
            permutation = [cells[c][0] for c in sorted(cells)]
            form = tuple(tuple(matrix[permutation[i]][permutation[j]] for j in range(n)) for i in range(n))
            if best[0] is None or form < best[0]:
                best[0] = form
            return
        target = next(c for c in sorted(cells) if len(cells[c]) > 1)
        for v in cells[target]:
            individualized = [x * 2 for x in colour]
            individualized[v] = colour[v] * 2 - 1
            search(refine(relabel(individualized)))

    initial = [(2 * matrix[i][i] + sum(matrix[i][j] for j in neighbors[i]), matrix[i][i]) for i in range(n)]
    search(refine(relabel(initial)))
    return best[0]


# ---------------------------------------------------------------------------
# Skeleton enumeration
# ---------------------------------------------------------------------------

def connected_without(vertex_count, edge_pairs, skip):
    adjacency = [[] for _ in range(vertex_count)]
    for k, (a, b) in enumerate(edge_pairs):
        if k in skip:
            continue
        adjacency[a].append(b)
        adjacency[b].append(a)
    seen = {0}
    stack = [0]
    while stack:
        x = stack.pop()
        for y in adjacency[x]:
            if y not in seen:
                seen.add(y)
                stack.append(y)
    return len(seen) == vertex_count


def bridgeless(vertex_count, edge_pairs):
    return all(connected_without(vertex_count, edge_pairs, {k}) for k in range(len(edge_pairs)))


def series_classes(vertex_count, edge_pairs):
    m = len(edge_pairs)
    root = list(range(m))

    def find(x):
        while root[x] != x:
            root[x] = root[root[x]]
            x = root[x]
        return x

    for i in range(m):
        for j in range(i + 1, m):
            if not connected_without(vertex_count, edge_pairs, {i, j}):
                a, b = find(i), find(j)
                if a != b:
                    root[a] = b
    classes = {}
    for i in range(m):
        classes.setdefault(find(i), []).append(i)
    return sorted(classes.values())


def cosimplify(vertex_count, edge_pairs):
    """Merge every series class to a single edge (contract the others)."""
    root = list(range(vertex_count))

    def find(x):
        while root[x] != x:
            root[x] = root[root[x]]
            x = root[x]
        return x

    keep = []
    for cls in series_classes(vertex_count, edge_pairs):
        keep.append(cls[0])
        for k in cls[1:]:
            a, b = edge_pairs[k]
            ra, rb = find(a), find(b)
            if ra != rb:
                root[ra] = rb
    return [(find(edge_pairs[k][0]), find(edge_pairs[k][1])) for k in sorted(keep)]


def degree_sequences(loops, min_degree, max_degree):
    """Yield (excess, degrees) with sum(deg - 3) = excess and E - V + 1 = L."""
    result = []
    for excess in range(0, 2 * (loops - 1)):
        vertex_count = 2 * (loops - 1) - excess
        if vertex_count < 1:
            continue
        degrees = list(range(min_degree, max_degree + 1))

        def fill(position, remaining_vertices, remaining_excess, chosen):
            if position == len(degrees):
                if remaining_vertices == 0 and remaining_excess == 0:
                    sequence = []
                    for degree, count in zip(degrees, chosen):
                        sequence.extend([degree] * count)
                    result.append((excess, tuple(sorted(sequence, reverse=True))))
                return
            degree = degrees[position]
            for count in range(remaining_vertices + 1):
                used = count * (degree - 3)
                if used > remaining_excess:
                    break
                fill(position + 1, remaining_vertices - count, remaining_excess - used, chosen + [count])

        fill(0, vertex_count, excess, [])
    return sorted(result, key=lambda item: (-item[0], item[1]))


def enumerate_skeletons(degrees, visit):
    """Backtrack over symmetric multiplicity matrices; visit(edge_pairs) per labelled leaf."""
    n = len(degrees)
    pairs = [(i, j) for i in range(n) for j in range(i, n)]
    remaining = list(degrees)
    multiplicity = {}
    leaves = 0

    def recurse(position):
        nonlocal leaves
        if position == len(pairs):
            if any(remaining):
                return
            edge_pairs = []
            for (i, j), m in multiplicity.items():
                edge_pairs.extend([(i, j)] * m)
            leaves += 1
            visit(edge_pairs)
            return
        i, j = pairs[position]
        if i == j:
            maximum = remaining[i] // 2
            if i == n - 1:
                if remaining[i] % 2:
                    return
                low = maximum
            else:
                low = 0
            for m in range(low, maximum + 1):
                remaining[i] -= 2 * m
                multiplicity[(i, j)] = m
                recurse(position + 1)
                remaining[i] += 2 * m
            multiplicity.pop((i, j), None)
        else:
            maximum = min(remaining[i], remaining[j])
            low = remaining[i] if j == n - 1 else 0
            for m in range(low, maximum + 1):
                remaining[i] -= m
                remaining[j] -= m
                multiplicity[(i, j)] = m
                recurse(position + 1)
                remaining[i] += m
                remaining[j] += m
            multiplicity.pop((i, j), None)

    recurse(0)
    return leaves


def classify_owners(masks, parents, preferred_parent, loops, min_degree, max_degree, progress=None):
    """Return the classification owners block and enumeration statistics."""
    class_of = {}
    per_owner = {}
    for mask in masks:
        slots = [i + 1 for i, bit in enumerate(mask) if bit == "1"]
        edges, parent_id = realization(slots, parents, preferred_parent.get(mask))
        if edges is None:
            raise PlanError(f"owner {mask}: no parent witness contains its slots")
        forms, states, connected_states = two_isomorphism_closure(edges)
        for form in forms:
            other = class_of.get(form)
            if other is not None and other != mask:
                raise PlanError(f"owners {other} and {mask} share a connected realization form")
            class_of[form] = mask
        per_owner[mask] = {"t": len(slots), "parent": parent_id, "closure_states": states,
                           "connected_states": connected_states, "connected_forms": len(forms),
                           "matches": {}}
    if progress:
        progress(f"closure: {len(masks)} owners, {len(class_of)} connected canonical forms")
    leaves_by_excess, distinct_by_excess, unmatched = {}, {}, {}
    for excess, degrees in degree_sequences(loops, min_degree, max_degree):
        n = len(degrees)
        seen = set()

        def visit(edge_pairs):
            if not is_connected(n, edge_pairs) or not bridgeless(n, edge_pairs):
                return
            form = canonical_form(edge_pairs)
            if form in seen:
                return
            seen.add(form)
            reduced = cosimplify(n, edge_pairs)
            owner = class_of.get(canonical_form(reduced))
            if owner is None:
                key = f"{excess}:{len(reduced)}"
                unmatched[key] = unmatched.get(key, 0) + 1
                return
            matches = per_owner[owner]["matches"]
            matches[excess] = matches.get(excess, 0) + 1

        leaves = enumerate_skeletons(degrees, visit)
        leaves_by_excess[excess] = leaves_by_excess.get(excess, 0) + leaves
        distinct_by_excess[excess] = distinct_by_excess.get(excess, 0) + len(seen)
        if progress:
            progress(f"degrees {list(degrees)}: {leaves} labelled leaves, {len(seen)} distinct skeletons")
    owners = {}
    for mask in masks:
        row = per_owner[mask]
        matches = row["matches"]
        owners[mask] = {
            "t": row["t"], "parent": row["parent"], "closure_states": row["closure_states"],
            "connected_states": row["connected_states"], "connected_forms": row["connected_forms"],
            "entry_capable": bool(matches),
            "V4min": min(matches) if matches else None,
            "skeleton_counts_by_V4": {str(k): matches[k] for k in sorted(matches)},
        }
    enumeration = {
        "degree_sequences": [{"V4": excess, "degrees": list(degrees)}
                             for excess, degrees in degree_sequences(loops, min_degree, max_degree)],
        "labelled_leaves_by_V4": {str(k): v for k, v in sorted(leaves_by_excess.items())},
        "distinct_skeletons_by_V4": {str(k): v for k, v in sorted(distinct_by_excess.items())},
        "owner_canonical_forms": len(class_of),
        "unmatched_reduced_skeletons_by_V4_and_t": {k: unmatched[k] for k in sorted(unmatched)},
    }
    return owners, enumeration


# ---------------------------------------------------------------------------
# Bounds
# ---------------------------------------------------------------------------

def connected_bounds(loops, difference, excess, gauge_powers):
    rank = difference - loops + 1 - excess + gauge_powers
    return {"A_max": rank + difference, "R_max": rank, "D_min": difference, "D_max": difference}


def nested_bounds(loops, excess, gauge_powers):
    return {"A_max": 5 * loops - 1 - excess + gauge_powers, "R_max": 3 * loops - excess + gauge_powers,
            "D_min": 2 * loops - 1, "D_max": None}


def closed_form_count(active, inactive, bounds):
    """sum_A C(A-1,t-1) * sum_R C(R+m-1,m-1) over the admitted (A, R) band."""
    total = 0
    a_low = max(active, bounds["D_min"] if bounds["D_min"] is not None else 0)
    for a in range(a_low, bounds["A_max"] + 1):
        r_low = 0 if bounds["D_max"] is None else max(0, a - bounds["D_max"])
        r_high = bounds["R_max"] if bounds["D_min"] is None else min(bounds["R_max"], a - bounds["D_min"])
        if inactive == 0:
            r_high = min(r_high, 0)
        if r_high < r_low:
            continue
        positive = 1 if active == 0 else math.comb(a - 1, active - 1)
        ranks = sum(1 if inactive == 0 else math.comb(r + inactive - 1, inactive - 1)
                    for r in range(r_low, r_high + 1))
        total += positive * ranks
    return total


def root_rows(mask, klass, excess, options):
    """Root bound records for one owner in planner order (descending R_max)."""
    loops, powers = options["loops"], options["gauge_parameter_powers"]
    t = mask.count("1")
    kinds = []
    if klass == CLASS_CONNECTED:
        kinds = [("phys", connected_bounds(loops, d, excess, powers), d) for d in options["difference_set"]]
    elif klass == CLASS_FACTORIZED:
        if options["factorized_roots"] == "nested":
            kinds = [("nested", nested_bounds(loops, excess, powers), None)]
        elif options["factorized_roots"] == "box":
            kinds = [("fact", connected_bounds(loops, d, excess, powers), d) for d in options["difference_set"]]
    elif klass == CLASS_NON_ENTRY:
        if options["non_entry_roots"] == "widest":
            kinds = [("conv", connected_bounds(loops, d, 0, powers), d) for d in options["difference_set"]]
    rows = []
    for prefix, bounds, difference in kinds:
        a_upper = bounds["A_max"] - t
        if a_upper < 0 or bounds["R_max"] < 0:
            raise PlanError(f"owner {mask}: {prefix} root has a negative coordinate upper "
                            f"(A_max {bounds['A_max']}, t {t}, R_max {bounds['R_max']})")
        count = closed_form_count(t, len(mask) - t, bounds)
        if count == 0:
            raise PlanError(f"owner {mask}: {prefix} root band is empty ({bounds})")
        label = f"d{difference}" if difference is not None else f"d{bounds['D_min']}p"
        rows.append({"id": f"{prefix}-{label}-a{bounds['A_max']}-r{bounds['R_max']}-{mask}", "kind": prefix,
                     "A_max": bounds["A_max"], "R_max": bounds["R_max"], "D_min": bounds["D_min"],
                     "D_max": bounds["D_max"], "active_upper": a_upper, "inactive_upper": bounds["R_max"],
                     "closed_form_count": count})
    rows.sort(key=lambda row: (-row["R_max"], -row["A_max"], row["id"]))
    return rows


def query_row(mask, lower, upper, rank, max_positive, d_min, d_max, query_id):
    if len(query_id.encode("utf-8")) > MAX_ID_BYTES:
        raise PlanError(f"query id exceeds {MAX_ID_BYTES} bytes: {query_id}")
    return {"id": query_id, "owner": mask, "lower": lower, "upper": upper, "max_numerator_rank": rank,
            "power_bounds": {"max_positive_power": max_positive, "min_power_difference": d_min,
                             "max_power_difference": d_max}}


def build_queries(owner_rows, positive_power_owners):
    """Per owner in selection order: helper first, then roots (descending R_max)."""
    queries = []
    for row in owner_rows:
        mask = row["owner"]
        roots = row["roots"]
        if not roots:
            row["helper"] = None
            continue
        rank = max(root["R_max"] for root in roots)
        power = max(root["A_max"] for root in roots) if mask in positive_power_owners else None
        helper_id = f"{HELPER_PREFIX}r{rank}-a{'none' if power is None else power}-{mask}"
        row["helper"] = {"id": helper_id, "max_numerator_rank": rank, "max_positive_power": power}
        queries.append(query_row(mask, [0] * len(mask), [None] * len(mask), rank, power, None, None, helper_id))
        for root in roots:
            upper = [root["active_upper"] if bit == "1" else root["inactive_upper"] for bit in mask]
            queries.append(query_row(mask, [0] * len(mask), upper, root["R_max"], root["A_max"],
                                     root["D_min"], root["D_max"], root["id"]))
    ids = [q["id"] for q in queries]
    if len(set(ids)) != len(ids):
        raise PlanError("query ids collide")
    return queries


# ---------------------------------------------------------------------------
# Rust entry-domain-plan
# ---------------------------------------------------------------------------

def group_name(bounds):
    d_max = "none" if bounds["D_max"] is None else bounds["D_max"]
    return f"a{bounds['A_max']}-r{bounds['R_max']}-dmin{bounds['D_min']}-dmax{d_max}"


def budget_groups(owner_rows):
    groups = {}
    for row in owner_rows:
        for root in row["roots"]:
            key = (root["A_max"], root["R_max"], root["D_min"], root["D_max"])
            group = groups.setdefault(key, {"name": group_name(root), "budget": {
                "max_positive_power": root["A_max"], "max_numerator_rank": root["R_max"],
                "min_power_difference": root["D_min"], "max_power_difference": root["D_max"]},
                "sectors": [], "roots": []})
            if row["owner"] in group["sectors"]:
                raise PlanError(f"owner {row['owner']} has two roots in budget group {group['name']}")
            group["sectors"].append(row["owner"])
            group["roots"].append(root)
    return [groups[key] for key in sorted(groups, key=lambda k: (-k[0], -k[1], -k[2], k[3] is None, k[3] or 0))]


def run_entry_domain_plan(executable, group):
    spec = {"schema": ENTRY_SPEC_SCHEMA, "sectors": group["sectors"], "budget": group["budget"],
            "max_positive_layers_per_sector": group["budget"]["max_positive_power"] + 1, "max_preview_targets": 0}
    command = [str(executable), "entry-domain-plan", "--input", "-", "--output", "-"]
    try:
        completed = subprocess.run(command, input=json.dumps(spec, sort_keys=True), text=True,
                                   capture_output=True, check=False)
    except OSError as error:
        raise PlanError(f"cannot run {executable}: {error}") from error
    if completed.returncode != 0:
        raise PlanError(f"entry-domain-plan failed for group {group['name']}: {completed.stderr.strip()}")
    try:
        plan = json.loads(completed.stdout, object_pairs_hook=unique_object)
    except ValueError as error:
        raise PlanError(f"entry-domain-plan returned invalid JSON for group {group['name']}: {error}") from error
    if not isinstance(plan, dict) or plan.get("schema") != ENTRY_PLAN_SCHEMA or plan.get("counts_exact") is not True:
        raise PlanError(f"entry-domain-plan returned an unexpected document for group {group['name']}")
    sectors = plan.get("sectors")
    if not isinstance(sectors, list) or [s.get("sector") for s in sectors] != group["sectors"]:
        raise PlanError(f"entry-domain-plan sector order differs for group {group['name']}")
    counts = {}
    for sector in sectors:
        text = sector.get("target_count")
        if not isinstance(text, str) or not text.isdecimal():
            raise PlanError(f"entry-domain-plan target_count must be a decimal string for {sector.get('sector')}")
        counts[sector["sector"]] = int(text)
    if str(sum(counts.values())) != plan.get("total_target_count"):
        raise PlanError(f"entry-domain-plan total disagrees with its sectors for group {group['name']}")
    return plan, spec, counts


# ---------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------

def parse_mask_list(text):
    masks = [m for m in text.split(",") if m]
    for mask in masks:
        if set(mask) - {"0", "1"}:
            raise PlanError(f"owner mask {mask!r} is not binary")
    return masks


def parse_difference_set(text):
    values = []
    for item in text.split(","):
        item = item.strip()
        if not item or not (item.lstrip("-").isdecimal()):
            raise PlanError(f"difference set entry {item!r} is not an integer")
        values.append(int(item))
    if not values or len(set(values)) != len(values):
        raise PlanError("difference set must be a nonempty list of distinct integers")
    return sorted(values, reverse=True)


def load_selection(path):
    document = load_json(path)
    owners = document.get("owners") if isinstance(document, dict) else None
    if not isinstance(owners, list) or not owners:
        raise PlanError("selection must contain a nonempty owner list")
    rows = []
    for owner in owners:
        mask = owner.get("mask") if isinstance(owner, dict) else None
        if not isinstance(mask, str) or not mask or set(mask) - {"0", "1"}:
            raise PlanError("selection owner masks must be nonempty binary strings")
        rows.append({"mask": mask, "representative": owner.get("representative"),
                     "parent": owner.get("parent"), "ordinal": owner.get("ordinal"),
                     "published_sector": owner.get("published_sector")})
    masks = [row["mask"] for row in rows]
    if len(set(masks)) != len(masks) or len({len(m) for m in masks}) != 1:
        raise PlanError("selection owner masks must be unique and of equal arity")
    return document, rows


def factorized_flags(momenta_document, owner_rows):
    representatives = momenta_document.get("representatives")
    if not isinstance(representatives, list):
        raise PlanError("momenta manifest must list representatives")
    flags = {}
    for representative in representatives:
        if not isinstance(representative, dict) or type(representative.get("sector_id")) is not int:
            raise PlanError("each representative needs an integer sector_id")
        if representative["sector_id"] in flags or not isinstance(representative.get("factorized"), bool):
            raise PlanError(f"representative {representative.get('sector_id')} duplicated or missing factorized flag")
        flags[representative["sector_id"]] = representative["factorized"]
    result = {}
    for row in owner_rows:
        if row["representative"] not in flags:
            raise PlanError(f"owner {row['mask']}: representative {row['representative']!r} is not in the momenta manifest")
        result[row["mask"]] = flags[row["representative"]]
    return result


def git_head(root):
    try:
        completed = subprocess.run(["git", "-C", str(root), "rev-parse", "HEAD"], capture_output=True, text=True,
                                   check=False)
    except OSError:
        return None
    head = completed.stdout.strip()
    return head if completed.returncode == 0 and len(head) == 40 else None


def masked_argv(argv):
    result = list(argv)
    for index, item in enumerate(result):
        if item == "--output-directory" and index + 1 < len(result):
            result[index + 1] = "<output-directory>"
        elif item.startswith("--output-directory="):
            result[index] = "--output-directory=<output-directory>"
    return result


def build_parser():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--loops", type=int, required=True)
    parser.add_argument("--manifest", type=Path, required=True, help="owner selection.json (read-only)")
    parser.add_argument("--momenta", type=Path, required=True, help="offline family census with momenta")
    parser.add_argument("--parent-witnesses", type=Path, required=True)
    parser.add_argument("--gauge", choices=("feynman", "linear-xi"), default="feynman")
    parser.add_argument("--gauge-parameter-powers", type=int, default=None,
                        help="K; must be 0 (default) for feynman, explicit for linear-xi")
    parser.add_argument("--difference-set", default="9,10", help="exact D values for connected roots")
    parser.add_argument("--factorized-roots", choices=("nested", "box", "omit"), default="nested")
    parser.add_argument("--non-entry-roots", choices=("widest", "omit"), default="widest")
    parser.add_argument("--min-vertex-degree", type=int, default=3)
    parser.add_argument("--max-vertex-degree", type=int, default=4)
    parser.add_argument("--helper-positive-power-owners", default="",
                        help="comma-separated owner masks whose helper gets max_positive_power")
    parser.add_argument("--helper-positive-power-owners-from", type=Path,
                        help="matching-summary.json with helper_positive_power_owners")
    classification = parser.add_mutually_exclusive_group()
    classification.add_argument("--classification", type=Path, help="reuse a classification document")
    classification.add_argument("--classification-fixture", type=Path,
                                help="recompute and require equality with this document")
    parser.add_argument("--executable", type=Path, help="rustred CLI for entry-domain-plan counts")
    parser.add_argument("--output-directory", type=Path, required=True, help="must not exist")
    parser.add_argument("--quiet", action="store_true")
    return parser


def plan(args, argv, progress=None):
    started = time.monotonic()
    timing = {"started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}
    loops = args.loops
    if loops < 2:
        raise PlanError("--loops must be at least 2")
    if not 3 <= args.min_vertex_degree <= args.max_vertex_degree:
        raise PlanError("vertex degrees must satisfy 3 <= min <= max")
    if args.gauge == "feynman":
        if args.gauge_parameter_powers not in (None, 0):
            raise PlanError("feynman gauge requires --gauge-parameter-powers 0")
        gauge_powers = 0
    else:
        if args.gauge_parameter_powers is None or args.gauge_parameter_powers < 0:
            raise PlanError("linear-xi gauge requires an explicit nonnegative --gauge-parameter-powers")
        gauge_powers = args.gauge_parameter_powers
    difference_set = parse_difference_set(args.difference_set)
    output = args.output_directory
    if output.exists():
        raise PlanError(f"output directory {output} already exists")

    selection_document, owner_rows = load_selection(args.manifest)
    masks = [row["mask"] for row in owner_rows]
    momenta_document = load_json(args.momenta)
    momenta = load_momenta(momenta_document, loops)
    if len(masks[0]) != len(momenta):
        raise PlanError("owner mask arity differs from the momenta coordinate count")
    witness_document = load_json(args.parent_witnesses)
    parents = validate_witnesses(witness_document, momenta, loops)
    factorized = factorized_flags(momenta_document, owner_rows)
    input_digests = {
        "manifest": {"path": str(args.manifest), "sha256": sha256_file(args.manifest)},
        "momenta": {"path": str(args.momenta), "sha256": sha256_file(args.momenta)},
        "parent_witnesses": {"path": str(args.parent_witnesses), "sha256": sha256_file(args.parent_witnesses)},
    }
    positive_power_owners = set(parse_mask_list(args.helper_positive_power_owners))
    if args.helper_positive_power_owners_from is not None:
        summary = load_json(args.helper_positive_power_owners_from)
        listed = summary.get("helper_positive_power_owners") if isinstance(summary, dict) else None
        if not isinstance(listed, list) or any(not isinstance(m, str) for m in listed):
            raise PlanError("helper positive-power summary must list owner masks")
        positive_power_owners |= set(listed)
        input_digests["helper_positive_power_owners_from"] = {
            "path": str(args.helper_positive_power_owners_from),
            "sha256": sha256_file(args.helper_positive_power_owners_from)}
    unknown = sorted(positive_power_owners - set(masks))
    if unknown:
        raise PlanError(f"helper positive-power owners are not selected owners: {unknown}")

    classification_inputs = {"momenta_sha256": input_digests["momenta"]["sha256"],
                             "parent_witnesses_sha256": input_digests["parent_witnesses"]["sha256"]}
    degrees = {"min": args.min_vertex_degree, "max": args.max_vertex_degree}
    preferred = {row["mask"]: row["parent"] for row in owner_rows}
    if args.classification is not None:
        classification = load_json(args.classification)
        if not isinstance(classification, dict) or classification.get("schema") != CLASSIFICATION_SCHEMA:
            raise PlanError(f"--classification must carry schema {CLASSIFICATION_SCHEMA}")
        if (classification.get("loops") != loops or classification.get("vertex_degrees") != degrees
                or classification.get("inputs") != classification_inputs):
            raise PlanError("--classification was produced for different loops, degrees or inputs")
        owners_block = classification.get("owners")
        if not isinstance(owners_block, dict) or any(mask not in owners_block for mask in masks):
            raise PlanError("--classification does not cover every selected owner")
        for mask in masks:
            row = owners_block[mask]
            if row.get("t") != mask.count("1"):
                raise PlanError(f"--classification owner {mask} has an inconsistent line count")
        timing["classification_seconds"] = 0.0
        classification_source = {"kind": "reused", "path": str(args.classification),
                                 "sha256": sha256_file(args.classification)}
    else:
        clock = time.monotonic()
        owners_block, enumeration = classify_owners(masks, parents, preferred, loops, args.min_vertex_degree,
                                                    args.max_vertex_degree, progress)
        timing["classification_seconds"] = time.monotonic() - clock
        classification = {"schema": CLASSIFICATION_SCHEMA, "role": ROLE, "loops": loops,
                          "coordinate_count": len(momenta), "vertex_degrees": degrees,
                          "inputs": classification_inputs, "owner_count": len(masks),
                          "V4_semantics": "sum over vertices of (degree - 3); the quartic vertex count for degrees {3, 4}",
                          "enumeration": enumeration, "owners": owners_block}
        classification_source = {"kind": "computed"}
        if args.classification_fixture is not None:
            fixture = load_json(args.classification_fixture)
            if fixture != classification:
                differing = sorted(k for k in set(fixture) | set(classification) if fixture.get(k) != classification.get(k))
                raise PlanError(f"classification differs from fixture {args.classification_fixture} in {differing}")
            classification_source = {"kind": "computed_equal_to_fixture", "path": str(args.classification_fixture),
                                     "sha256": sha256_file(args.classification_fixture)}

    options = {"loops": loops, "gauge_parameter_powers": gauge_powers, "difference_set": difference_set,
               "factorized_roots": args.factorized_roots, "non_entry_roots": args.non_entry_roots}
    receipt_rows = []
    for row in owner_rows:
        mask = row["mask"]
        entry = owners_block[mask]
        if not entry["entry_capable"]:
            klass, excess = CLASS_NON_ENTRY, None
        elif factorized[mask]:
            klass, excess = CLASS_FACTORIZED, entry["V4min"]
        else:
            klass, excess = CLASS_CONNECTED, entry["V4min"]
        roots = root_rows(mask, klass, excess if excess is not None else 0, options)
        receipt_rows.append({"owner": mask, "ordinal": row["ordinal"], "published_sector": row["published_sector"],
                             "representative": row["representative"], "parent": entry.get("parent"),
                             "t": entry["t"], "class": klass, "entry_capable": entry["entry_capable"],
                             "factorized": factorized[mask], "V4min": excess,
                             "skeleton_counts_by_V4": entry["skeleton_counts_by_V4"], "roots": roots})
    queries = build_queries(receipt_rows, positive_power_owners)
    if not queries:
        raise PlanError("no queries were planned (every owner class omitted)")
    groups = budget_groups(receipt_rows)

    plans = {}
    clock = time.monotonic()
    for group in groups:
        if args.executable is not None:
            plan_document, spec, counts = run_entry_domain_plan(args.executable, group)
            plans[group["name"]] = plan_document
            group["spec"] = spec
            for root in group["roots"]:
                rust = counts[root_owner(receipt_rows, root)]
                if rust != root["closed_form_count"]:
                    raise PlanError(f"root {root['id']}: Rust count {rust} != closed form {root['closed_form_count']}")
                root["target_count"] = str(rust)
            group["total_target_count"] = plan_document["total_target_count"]
            group["count_source"] = "rust"
        else:
            for root in group["roots"]:
                root["target_count"] = None
            group["total_target_count"] = str(sum(root["closed_form_count"] for root in group["roots"]))
            group["count_source"] = "closed_form_only"
    timing["entry_domain_plan_seconds"] = time.monotonic() - clock

    queries_document = {"schema": QUERY_SCHEMA, "queries": queries}
    queries_bytes = dumps(queries_document).encode("utf-8")
    class_counts = {k: sum(1 for r in receipt_rows if r["class"] == k)
                    for k in (CLASS_CONNECTED, CLASS_FACTORIZED, CLASS_NON_ENTRY)}
    histogram = {}
    for row in receipt_rows:
        if row["V4min"] is not None:
            bucket = histogram.setdefault(row["class"], {})
            bucket[str(row["V4min"])] = bucket.get(str(row["V4min"]), 0) + 1
    histogram = {k: {kk: v[kk] for kk in sorted(v, key=int)} for k, v in sorted(histogram.items())}
    plan_paths = {}
    for group in groups:
        plan_paths[group["name"]] = f"entry-plans/{group['name']}.json"
    receipt = {
        "schema": RECEIPT_SCHEMA,
        "role": ROLE,
        "planner": {"script": Path(__file__).name, "script_sha256": sha256_file(__file__),
                    "argv": masked_argv(argv), "git_head": git_head(Path(__file__).resolve().parent),
                    "python_version": sys.version.split()[0],
                    "executable": None if args.executable is None else {
                        "path": str(args.executable), "sha256": sha256_file(args.executable)}},
        "inputs": input_digests,
        "classification_source": classification_source,
        "physics": {
            "loops": loops, "gauge": args.gauge, "gauge_parameter_powers": gauge_powers,
            "difference_set": difference_set, "factorized_roots": args.factorized_roots,
            "non_entry_roots": args.non_entry_roots, "vertex_degrees": degrees,
            "coordinate_convention": "local: active x = n - 1 >= 0, inactive y = -n >= 0; A = t + sum x, R = sum y, D = A - R",
            "formulas": {
                "connected_root": {"R_max": "D - L + 1 - V4min + K", "A_max": "R_max + D", "D": "exact, one root per difference-set value"},
                "nested_factorized_root": {"A_max": "5L - 1 - V4min + K", "R_max": "3L - V4min + K", "D_min": "2L - 1", "D_max": None},
                "box_factorized_root": "connected formulas at the owner's V4min",
                "non_entry_root": "connected formulas at V4min = 0",
                "coordinate_uppers": {"active": "A_max - t", "inactive": "R_max"},
                "helper": {"lower": 0, "upper": None, "max_numerator_rank": "largest root R_max",
                           "max_positive_power": "largest root A_max for listed owners, else null",
                           "difference_bounds": None},
            },
            "helper_positive_power_owners": sorted(positive_power_owners),
            "id_prefixes": {"helper": HELPER_PREFIX, CLASS_CONNECTED: "phys", "factorized_nested": "nested",
                            "factorized_box": "fact", CLASS_NON_ENTRY: "conv"},
        },
        "skeleton_enumeration": classification.get("enumeration"),
        "owners": receipt_rows,
        "summary": {
            "owner_count": len(receipt_rows), "query_count": len(queries),
            "helper_count": sum(1 for r in receipt_rows if r["helper"] is not None),
            "root_count": sum(len(r["roots"]) for r in receipt_rows),
            "class_counts": class_counts, "V4min_histogram_by_class": histogram,
            "queries_sha256": sha256_bytes(queries_bytes), "queries_bytes": len(queries_bytes),
            "total_target_count": str(sum(int(g["total_target_count"]) for g in groups)),
            "count_source": "rust" if args.executable is not None else "closed_form_only",
            "budget_groups": [{"name": g["name"], "budget": g["budget"], "sectors": g["sectors"],
                               "root_ids": [r["id"] for r in g["roots"]],
                               "total_target_count": g["total_target_count"], "count_source": g["count_source"],
                               "plan_path": plan_paths[g["name"]] if g["name"] in plans else None}
                              for g in groups],
            "query_order": "selection owner order; helper first, then roots by descending R_max",
        },
        "descendant_clipping": False,
        "family_closure_claim": False,
    }
    output.mkdir(parents=True, exist_ok=False)
    (output / "queries.json").write_bytes(queries_bytes)
    (output / "skeleton-classification.json").write_text(dumps(classification), encoding="utf-8")
    if plans:
        (output / "entry-plans").mkdir()
        for name, document in plans.items():
            (output / plan_paths[name]).write_text(dumps(document), encoding="utf-8")
    (output / "entry-plan-receipt.json").write_text(dumps(receipt), encoding="utf-8")
    timing["total_seconds"] = time.monotonic() - started
    (output / "timing.json").write_text(dumps(timing), encoding="utf-8")
    return receipt


def root_owner(receipt_rows, root):
    for row in receipt_rows:
        if any(r is root for r in row["roots"]):
            return row["owner"]
    raise PlanError("root without owner")


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    args = build_parser().parse_args(argv)
    progress = None if args.quiet else (lambda text: print(text, file=sys.stderr, flush=True))
    try:
        receipt = plan(args, argv, progress)
    except PlanError as error:
        print(f"refused: {error}", file=sys.stderr)
        return 2
    summary = receipt["summary"]
    print(json.dumps({"output_directory": str(args.output_directory), "query_count": summary["query_count"],
                      "class_counts": summary["class_counts"], "queries_sha256": summary["queries_sha256"],
                      "total_target_count": summary["total_target_count"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
