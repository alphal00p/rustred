# Generic family generated-rule orchestration

`src/generated_family_rule_system.rs` composes the existing generic proof
layers into one replayable family-wide transcript. It is the orchestration
slice between RustRed's `AnalyzeSectors` analogue and the still-incomplete
full `SolvejSector` fixed point.

## Production inputs

The compiler accepts only:

- an authenticated `IntegralFamily` and its exact Symbolica `K(n)` context;
- `SectorRestrictions` and `PowerShiftPolicy` for inventory/zero analysis;
- an `IntegralOrderingPolicy`;
- the family orchestration strategy; and
- explicit nested and aggregate resource limits.

There is no topology identifier, loop-count dispatch, supplied recurrence,
pivot, master count, master list, or numerical oracle. Concrete tadpole and
sunset families appear only in black-box tests.

## Pipeline and statuses

The compiler first retains the complete `FamilySectorInventoryCertificate`.
It then visits only `UnresolvedNoZeroCertificate` entries, in the inventory's
certified subsector-first order. For each such sector it runs
`GeneratedSectorDiscoveryCompiler`, followed by
`GeneratedSectorLiveLeafQueueCompiler`.

Every raw sector has exactly one transcript status:

- `Excluded`: a user restriction excluded it; this is not an analytic zero;
- `ProvedZero`: a replayable Symbolica-native zero certificate;
- `Unresolved`: both generated stages completed, but this is neither a proof
  of full reduction nor master inference;
- `ResourceLimited`: a bounded stage stopped, with the exact typed nested
  error and any completed discovery retained; or
- `Failed`: a non-resource algorithm/domain error, retained at its exact
  stage without inference.

A full-column-rank zero-test witness means only that the current sufficient
zero criterion found no kernel. It is never called a nonzero proof.

Replay reconstructs inventory, checks exact solve order and policy bindings,
replays every completed nested certificate, and reruns interrupted stages to
require the same typed error.

The V3 coverage used by each discovery stores the authenticated Boolean bad
domain directly instead of overlaying the local `WhenBad` decision-tree
prefix.  A true later bad clause therefore short-circuits an irrelevant
earlier unknown predicate.  Atomic zero-locus disjunctions are compressed by
the exact integral-domain identity

```text
p1 = 0 or ... or pk = 0    iff    p1 * ... * pk = 0.
```

Checked Symbolica multiplication and exact-algebra budgets guard construction
of that product.  On the connected equal-mass two-loop sunset at adaptive
depth one this gives a replayable partition with 2,200 splits and 2,201 leaves
(471 descending, 53 proved empty, and 1,677 still open), below a hard 4,096
regression cap.  The measurement is a concrete scaling test, not a
topology-specific production rule or a claim of complete sector closure.

Finite `i64` index-representation overflows reached during a generated
partial re-elimination are retained as an explicit V2
`PreservedIndexBoundary` outcome.  Its witness owns the ordering and exact
typed interruption, and replay must reproduce that interruption.  Only the
closed set of checked index-overflow errors is preservable; resource,
algebra, malformed-input, and replay errors still abort normally.

## Restrictions and formal power shifts

The current generated discovery and live-leaf queue APIs do not accept
`SectorRestrictions` or `PowerShiftPolicy`. Those policies therefore govern
only inventory/zero analysis and are exposed as
`inventory_restrictions()`/`inventory_power_shift_policy()`.

Generated rules remain sound because their `WhenBad` proof treats moves out
of the selected orthant conservatively as sector leaks or exceptional leaves;
it does not assume that another sector is admitted. This may retain work that
a future restriction-aware generated stage could prune, so it is
sound-but-conservative. The family certificate deliberately does not claim
that inventory restrictions govern generated-stage rows or rule domains.

The one current strategy variant,
`InventoryDiscoveryAndLiveLeafQueue`, is still stored and explicitly matched
on compilation and replay. This binds the algorithm choice into the v1
certificate and prevents a future orchestration strategy from silently
changing v1 semantics.

## Boundary of this slice

This certificate schedules and authenticates initial symbolic sector rules
and exceptional-locus work. It does not yet iterate conditional discoveries
to a global fixed point, feed solved subsector rules back into supersectors,
or select irreducible integrals as masters. Those remain required before
claiming complete LiteRed `SolvejSector` parity.

The generated whole-row symmetry span is compiled once for the family when
the inventory contains generated-stage work.  One immutable `Arc` is retained
by the family certificate and reused by every sector discovery, coverage
certificate, candidate source-authentication proof, and queue-embedded
discovery.  Family compilation and replay check allocation identity throughout
that internal graph, in addition to the ordinary payload and policy bindings.
Public lower-level replay entry points may instead receive an independently
reconstructed, payload-equal row span; they authenticate it and normalize
candidate proofs onto the supplied allocation.

A failure while constructing the shared span is retained once as the family
row-span interruption and projected conservatively onto every scheduled
sector.  The statistics distinguish the single family compilation attempt and
successful certificate from sector and candidate reuses; an interrupted
shared compilation records no reuses.  Replay regenerates or replays the span
once before traversing the nested sector transcripts, after first enforcing
the cheap family-wide transcript and scheduled-sector caps.
