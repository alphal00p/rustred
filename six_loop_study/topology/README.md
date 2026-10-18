# Offline six-loop vacuum topology inputs

This directory contains **inputs, not generated IBP rules**. The generic
RustRed engine does not contain these topology names, graphs, or momentum
assignments. The launch manifest is `manifest.json`; each path in it is
relative to this directory.

## What was enumerated

The existing HEPKit `Model.phi3()` generator was asked for connected vacuum
graphs at exactly six loops with one cubic interaction type. Self loops,
parallel propagators, zero-flow edges, self energies, tadpoles, and factorized
graphs were all admitted during enumeration. No QCD/color/numerator filter
was applied. Each generated graph has ten cubic vertices and fifteen edges.

Measured native output:

| Stage | Count |
|---|---:|
| Connected cubic multigraphs generated | 388 |
| Factorized graphs, explicitly classified | 322 |
| Nonfactorized cubic graphs | 66 |
| Nonfactorized classes after equal-momentum compression | 66 |
| Maximal fifteen-propagator parents | 14 |
| Classes covered by an exact contraction/routing witness | 66 / 66 |

The **full contraction census** is larger than those 66 cubic-graph
quotients. Enumerating all `14 × 2^15 = 458,752` labelled physical root
subsets gives **435 full-loop graph-matroid classes: 351 nonfactorized and
84 factorized**. The nonfactorized counts by propagator number are:

| Propagators | 15 | 14 | 13 | 12 | 11 | 10 | 9 | 8 | 7 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Classes | 14 | 33 | 74 | 90 | 76 | 42 | 17 | 4 | 1 |

The remaining labelled subsets have insufficient loop-momentum rank. The
full census took 85.93 seconds on one core. `contraction_census.json` saves
one explicit parent/subset representative per class and all occurrence
counts. It **does not** save an independently checked momentum map for each
duplicate labelled subset. Thus its graph-matroid classification and the
66 explicit signed routing witnesses are deliberately distinct deliverables.

The same scripts, with no loop-count-specific algorithm changes, reproduce
the lower-loop reference censuses:

| Loops | Maximal parents | All classes | Nonfactorized | Factorized |
|---|---:|---:|---:|---:|
| 4 | 2 | 16 | 10 | 6 |
| 5 | 4 | 67 | 48 | 19 |
| 6 | 14 | 435 | 351 | 84 |

The five-loop total **and every propagator-count subdivision** agree with
TIDE's Table B.2. The control inputs and results are in `controls/`.

After equal-momentum compression the 66 classes have respectively 14, 14,
18, 12, and 8 representatives with 15, 14, 13, 12, and 11 distinct physical
propagators. The 52 lower-line representatives are all explicitly embedded
in contractions of the fourteen maximal parents. `maximal_parent_cover.json`
records the parent slots and exact signed unimodular loop-momentum map for
every class.

Enumeration took about 0.41 seconds, classification about 0.37 seconds, and
the constructive cover search about 4.60 seconds on one core of this host.
These are **offline graph/input preparation timings**, not IBP solve times.

## Equivalence and exclusions

FeynKit supplies the fundamental integer loop routing. Each routing is
checked for momentum conservation and fundamental unit rows. The binary
cycle space is enumerated as finite edge subsets; its inclusion-minimal
nonempty supports are the complete graph circuits. Symbolica's native graph
canonicalizer acts on the colored edge/circuit incidence graph. It proposes
an edge permutation; it is not itself the authority for an integral equality.

Every recorded **cubic-quotient alias and all 66 embeddings into the fourteen
maximal parents** are replayed with Symbolica's exact matrix rank, inverse,
multiplication, and determinant operations. Each such witness satisfies
`q_source R = ±q_target` for every retained propagator, with integer `R` and
`det(R)=±1`. The same matrix specifies the transformation of all numerator
scalar products; auxiliary slots are not presumed to permute trivially.
For the full 458,752-subset inventory, equivalence is instead the exact native
graph-matroid classification. The independent audit checks explicit forest
contractions and graph circuits for all **435 chosen representatives**, not
an individual signed momentum map for every duplicate labelled subset.

At a common nonzero mass, equal momenta up to sign produce the same inverse
propagator. Their powers add. `edge_groups` and `edge_group_signs` retain the
explicit compression map. A zero-momentum bridge contributes a mass factor
times lower-loop products: it is **not** declared a zero massive integral.
Cycle-matroid direct-sum blocks provide the recorded scalar factorization
witnesses. Factorized graphs were omitted only from the requested starting
parent list; factorized descendant sectors remain in the parent downsets.

Every compressed physical family has full loop rank and independent squared
propagators, checked by native Symbolica matrices. HEPKit's existing
`IntegralFamily.complete()` appends six independent auxiliary denominators
to each maximal parent, producing a complete 21-coordinate vacuum family.
Physical positions are `0..14`; auxiliary positions are `15..20` and carry
nonpositive starting powers. These positions are ordinary input data, not a
six-loop-specific solver convention.

The fourteen fifteen-line parents also agree with the cubic count quoted in
§3 of [Glue-and-Cut at Five Loops](https://arxiv.org/pdf/2104.08272). That
paper's later cut-convergence filters and restricted lower-line counts are
**not** used as a massive-vacuum completeness argument here: the native
388-graph enumeration and explicit 66-class cover are retained instead.

## Files and regeneration

- `cubic_graphs.json`, `raw_dot/`: unfiltered native cubic output.
- `classified_census.json`: classifications, compressed classes, and maps.
- `maximal_parent_cover.json`: exact embeddings into maximal parents.
- `contraction_census.json`: full parent-downset graph-matroid census.
- `families/`: ordinary RustRed TOML inputs with denominators `q²−1`.
- `dot/`: corresponding native FeynKit diagrams. Their model mass is symbolic;
  the TOML inputs explicitly normalize the common squared mass to one.
- `manifest.json`: portable launch data including all 21 momentum rows and
  physical-coordinate-to-DOT-edge/sign maps.

Use one compatible HEPKit/Symbolica Python installation, without loading a
second Symbolica extension into the process:

```bash
python -B six_loop_study/topology/enumerate.py --loops 6 --threads 1
python -B six_loop_study/topology/classify.py
python -B six_loop_study/topology/cover.py
python -B six_loop_study/topology/export_families.py
python -B six_loop_study/topology/contractions.py
python -B six_loop_study/topology/export_families.py
python -B six_loop_study/topology/audit.py
```

The final export adds **435 selected sector masks**, one representative of
each class, assigned to its recorded parent. The generation launcher uses
these by default to avoid re-solving equivalent labelled topologies. It also
offers `--full-downsets` to generate every nonzero physical sector separately.
The masks have 21 characters in original coordinate order; auxiliary slots
remain zero. No numerator-rank cutoff is encoded in these family inputs.

This selected-sector worklist is **not an assembled closing artifact for all
14 labelled downsets**. Subsequent application still needs exact routing of
duplicate sectors and appropriate lower-owner feedback; these are not
silently inferred from the census. Parent jobs may still do repeated internal
algebra, and no cross-parent live rule cache is claimed. Topology cover is not
IBP closure: no parametric rules, numerical masters, or completed six-loop
reduction are claimed by this directory.
