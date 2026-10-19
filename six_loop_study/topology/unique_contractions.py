#!/usr/bin/env python3
"""Full contraction-class census without visiting every labelled subset.

Contraction commutes with matroid isomorphism. Expand every distinct class once,
contracting each possible next edge. Native Symbolica canonicalization owns the
quotient test. Keep an explicit original-parent subset for every new class.
No IBPs, symbolic coefficient algebra, or master-minimality claim is involved.
"""
import argparse
from collections import Counter, deque
import hashlib
import json
from pathlib import Path
import time

from contractions import blocks, canonical, minimal_circuits
from matroid import cycle_space


def enumerate_classes(manifest, observer=None):
    loops = manifest['loop_count']
    parents = {entry['id']: entry for entry in manifest['entries']}
    if not parents or len(parents) != len(manifest['entries']):
        raise ValueError('parent inventory is empty or contains duplicate IDs')
    spaces = {}
    for identifier, parent in parents.items():
        physical = parent['physical_propagators']
        # Masks use local physical ordinals, not complete-family coordinates.
        if len(set(physical)) != len(physical):
            raise ValueError('parent contains duplicate physical coordinates')
        space = cycle_space([parent['momenta'][i] for i in physical])
        if len(space) != 1 << loops:
            raise ValueError('parent has insufficient loop rank')
        spaces[identifier] = space
    classes, identities, queue = {}, {}, deque()
    counts = Counter()

    def admit(parent_id, selected):
        counts['candidate_subsets'] += 1
        if selected.bit_count() < loops:
            counts['rank_deficient_steps'] += 1
            return
        projected = {flow & selected for flow in spaces[parent_id]}
        if len(projected) != 1 << loops:
            counts['rank_deficient_steps'] += 1
            return
        circuits = minimal_circuits(projected)
        key, slots = canonical(selected, circuits)
        if key in classes:
            counts['known_class_hits'] += 1
            return
        identifier = 'contraction-' + hashlib.sha256(key.encode()).hexdigest()[:16]
        if identifier in identities and identities[identifier] != key:
            raise ValueError('canonical contraction identifier collision')
        identities[identifier] = key
        classification = 'factorized' if len(blocks(selected, circuits)) > 1 else 'nonfactorized'
        physical = parents[parent_id]['physical_propagators']
        classes[key] = dict(id=identifier, classification=classification,
            physical_edge_count=len(slots), representative_parent=parent_id,
            representative_slots=[physical[i] for i in slots], canonical_matroid=key)
        queue.append((parent_id, selected))

    for parent_id, parent in parents.items():
        admit(parent_id, (1 << len(parent['physical_propagators'])) - 1)
    while queue:
        parent_id, selected = queue.popleft()
        remaining = selected
        while remaining:
            bit = remaining & -remaining
            remaining ^= bit
            admit(parent_id, selected ^ bit)
        counts['expanded_classes'] += 1
        if observer is not None and counts['expanded_classes'] % 100 == 0:
            observer(dict(expanded=counts['expanded_classes'], discovered=len(classes), pending=len(queue)))
    values = sorted(classes.values(), key=lambda e: (-e['physical_edge_count'], e['classification'], e['id']))
    return dict(schema='rustred.offline-parent-contraction-matroids.v1', loops=loops,
        enumeration_method='matroid-quotient-frontier', complete=True,
        parents_done=list(parents), counts=dict(counts),
        classes_by_kind=dict(Counter(entry['classification'] for entry in values)),
        classes_by_edge_count={kind: dict(Counter(str(entry['physical_edge_count']) for entry in values
                                                if entry['classification'] == kind))
                              for kind in ('nonfactorized', 'factorized')},
        total_classes=len(values), representatives=values,
        labelled_occurrences_enumerated=False, all_duplicate_gl_routing_witnesses=False,
        authority='exact graph-matroid contraction-class census; no reduction or numerical master claims',
        closure_claim=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, required=True)
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text())
    started = time.monotonic()
    result = enumerate_classes(manifest, lambda event: print(json.dumps(
        dict(**event, seconds=time.monotonic()-started)), flush=True))
    result['wall_seconds'] = time.monotonic()-started
    (args.manifest.parent / 'contraction_census.json').write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps({key: value for key, value in result.items()
                      if key not in ('representatives', 'parents_done')}, indent=2))


if __name__ == '__main__':
    main()
