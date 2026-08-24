# LiteRed `SolvejSector`: residual-case recentering contract

This note records the part of LiteRed's Mathematica implementation that must
govern RustRed after initial parametric IBP/LI generation.  It is a source
reading of `vendor/LiteRed2/Source/LiteRed2026.m`, principally the
`SolvejSector`, `preparepoints`, `WhenBad`, `SmartReduce`, `gatherRules`, and
`deleteSpecific` definitions.  Mathematica is a specification source only;
RustRed must implement the behavior with Rust and Symbolica.

## What LiteRed actually iterates

`SolvejSector` does not perform one elimination around the sector corner and
then merely enlarge that corner stencil.  Its work item is a symbolic residual
case:

1. `noRules` is grouped by `gatherRules`.  Cases are ordered by the number of
   fixed parameters, the number of numeric indices, and the locations of the
   remaining symbolic indices in the sector ordering.
2. The first group is reversed into `cases`.  Each case is a list of exact
   substitutions/conditions for the current residual locus.
3. `startps=(indices/.#)&/@cases` constructs a start point for every case and
   `startp=First[startps]` chooses the current symbolic/numeric anchor.
4. `preparepoints[jsect, startp, depth]` is used when symbolic indices remain.
   It generates a diamond around that residual anchor and restricts only the
   coordinates that are already numeric to remain in the sector.
5. For a fully numeric group, `preparepoints[jsect, startps, depth]` unions the
   diamonds around all remaining numeric anchors and keeps only points in the
   sector.
6. The generated `IBPLI` equations are submitted cumulatively.  Candidate
   leading integrals are selected using the sector ordering and the active
   residual-case pattern.
7. A solved concrete/symbolic rule is reflected into the index variables,
   restricted to the selected case, and passed through `WhenBad`.  A rule is
   accepted only on the complement of its exact bad locus.
8. The accepted locus is added to `badconditions`, the covered case is
   removed, and `startps` is recomputed from the cases that remain.
9. If all current depths fail and symbolic indices remain, the search depth is
   increased automatically up to `MaxDepth`.  If the first remaining anchor
   changes, depth restarts at zero.
10. `gatherRules[deleteSpecific[...]]` subtracts all newly covered loci from
    the global residual condition, removes cases subsumed by more generic
    cases, and schedules the next group.

Consequently, a larger diamond centered only on the sector corner is not an
equivalent implementation.  A rule needed at a boundary such as
`J(-1,1,1)` can be visible in a local depth-one search recentered there while
remaining absent from a depth-one corner search.

## RustRed translation

RustRed already represents the ingredients needed for a proof-bearing
translation:

- `GeneratedSymbolicRowSpanCertificate` authenticates the topology-independent
  generated IBP/LI span and any verified whole-row symmetry transports.
- `ParametricSectorCoverageCertificate` owns the global exact `WhenBad`
  partition and distinguishes descending, uncovered, unsupported, and proved
  empty leaves.
- `GeneratedSectorLiveLeafQueueCertificate` visits residual leaves in stable
  case order and authenticates narrow coordinate equalities with
  `CoordinateEqualityLocusCertificate`.
- `AdaptiveParametricRuleProvider::candidate_layers_for_quotient` can perform
  the same cumulative diamond search with an arbitrary same-sector ordering
  anchor.

The next certificate schema therefore has to retain, at minimum:

- a deterministic ordered list of authenticated search anchors;
- the source residual case and coordinate-assignment witnesses for every
  non-corner anchor;
- per-anchor/per-depth candidate counts and checked aggregate resource usage;
- the combined candidate order used to compile one exact global coverage;
- the shared row-span allocation and exact family/context/configuration
  binding;
- replay that re-extracts every anchor from its source residual case instead
  of trusting stored coordinates.

The source-level scheduler contract is especially precise here.  In
`LiteRed2026.m`, `noRules` and its case grouping are initialized at
lines 2372 and 2419--2425; the residual loop and case selection are at
2430--2437; the current start points are rebuilt at 2446--2451; cumulative
diamonds and equation submission are at 2471--2481; a successful pivot is
recentered at 2484, attached to one selected residual case at 2486, and passed
through `WhenBad` at 2488--2490.  Its accepted locus is removed and its bad
locus is requeued at 2492--2500.  Depth growth and anchor reset are separate
operations at 2508--2516, and the exact residual fixed point is rebuilt from
the old cases plus the accumulated bad conditions at 2522--2523.  RustRed
must not port LiteRed's heuristic master inference at 2519--2520 or
2544--2547 into the correctness path.

### Candidate selection is not case closure

A concrete residual anchor is a sound way to select a useful *parametric*
candidate, but coverage of that one integer point is not proof that its parent
symbolic case is solved.  The proof-bearing sequence must be:

1. authenticate the source residual case and the equality assignment from
   which the anchor was constructed;
2. regenerate the local adaptive search and bind the chosen candidate to its
   exact anchor, depth, and within-layer ordinal;
3. authenticate the candidate against the shared generated IBP/LI row span;
4. require its `WhenBad` certificate to cover the anchor as a usefulness
   check; and
5. compose the candidate's *complete* `WhenBad` domain into the retained
   global coverage before deciding which part of the parent case remains.

Replay must regenerate both the local candidate position and the global
domain composition.  A stored candidate count plus generic IBP provenance is
insufficient: it would prove that the recurrence is physical, but not that the
declared scheduler search actually discovered that recurrence.  Conversely,
deleting the source leaf merely because the anchor is covered would silently
turn a point sample into a symbolic-domain proof.

A partial coordinate assignment may use the sector corner in unassigned
positions as a concrete elimination-order anchor.  This is a valid conservative
search heuristic, but it is not yet full `SolvejSector` parity: LiteRed preserves
symbolic components of `startp`, groups contiguous symbolic cases, and feeds
accepted rules back into the equation database.  RustRed must continue to keep
leaves with unrecognized polynomial predicates explicit and must not infer a
master from an unsuccessful bounded search.

## Required fixed point after anchored discovery

The complete topology-independent loop is:

```text
generated IBP/LI row span
  -> solve certified subsectors first
  -> exact residual case queue
  -> choose the next deterministic case group
  -> search a local diamond around its authenticated anchor
  -> derive parametric candidates
  -> compute exact WhenBad applicability
  -> subtract accepted loci and delete subsumed cases
  -> substitute certified solved-subsector rules into later eliminations
  -> repeat until residual condition is empty or a configured resource bound
     produces an explicit unresolved result
```

Symmetries are a separate certified quotient around this loop.  They may map a
query or scheduled sector to a canonical representative only after the affine
map has been proved compatible with the exact family and restrictions.  They
cannot turn an unresolved residual locus into closure.

## Master-integral policy

LiteRed exposes `NMIs` as an optional heuristic that may terminate once an
expected master count is reached.  RustRed's correctness path must not enable
that heuristic implicitly.  A terminal is a master only when supplied by an
explicit, replay-bound policy/certificate.  Exhausting depth, time, candidate,
or memory limits remains a typed unresolved/resource-limited outcome.

## Validation consequence

The connected equal-mass two-loop sunset is the first required regression.
The family-derived path must reduce

```text
J(2,1,1) = (d-3)/(3*m2) * J(1,1,1)
```

without a hardcoded recurrence.  The proof must show that the rule came from
the generated shared row span, that residual boundary anchors were scheduled
generically, and that symmetry transport (if used to avoid duplicate sector
work) was authenticated.  Only after this path is green should the same engine
be exercised on connected three-loop families.
