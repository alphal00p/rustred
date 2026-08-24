# RustRed family sector inventory: `AnalyzeSectors` orchestration slice

## Source map

The reference implementation is `AnalyzeSectors` in
`vendor/LiteRed2/Source/LiteRed2026.m`, especially lines 2956–3082.

- Lines 2956–3009 define the public products: zero/nonzero sector lists,
  simple sectors, basis sectors, a zero rule, patterns, and cuts.
- Lines 3020–3024 select the basis pattern and cut vector.
- Lines 3025–3040 construct the Feynman-parametric `U+F` rank predicate (or,
  on the alternate path, solve corner IBPs).
- Lines 3041–3056 enumerate sector bit masks and prune by monotonicity: a zero
  sector closes downward, while a sector that fails the zero test closes
  upward into `NonZeroSectors`.
- Lines 3057–3074 derive `SimpleSectors`, `BasisSectors`, maximal zero masks,
  and `ZerojRule`.
- Lines 3068–3069 serialize zero and “nonzero” masks by active-line count and
  bit order.

RustRed's underlying Symbolica-native rank proof is in
`src/zero_sectors.rs`.  `FamilySectorInventoryCompiler` in
`src/family_sector_inventory.rs` is the generic orchestration layer around
that proof boundary.

## RustRed contract

The compiler accepts only:

- an authenticated `IntegralFamily`;
- owned `SectorRestrictions` (cuts plus pattern);
- a named power-shift support policy;
- a named integral-ordering policy; and
- explicit resource limits.

It constructs one `ZeroSectorAnalyzer` and invokes its cached `analyze_all`
pass exactly once.  Every raw unshifted sector mask is retained with one exact
status:

1. `Excluded`: a cut or pattern removed the mask; this is metadata, not an
   analytic zero proof;
2. `ProvedZero`: a replayable primitive-kernel certificate proves the
   `U+F` logarithmic-derivative rank deficiency;
3. `UnresolvedNoZeroCertificate`: the sufficient zero test had full column
   rank, which proves only that this criterion did not prove zero;
4. `ResourceLimited`; or
5. `Failed`.

There is deliberately no conversion from the third state into “analytically
nonzero,” and no master-integral inference anywhere in this layer.

The certificate owns and replays the exact restrictions, power-shift policy,
`ZeroSectorLimits`, ordering policy, power-support mask, generic domain,
family and `G=U+F` fingerprints, complete status inventory, and aggregate
budgets.  Replay reconstructs the same analyzer configuration, repeats one
all-sector pass, and compares the complete deterministic payload.

## Solve order

Only `UnresolvedNoZeroCertificate` entries enter the unresolved sector-solving
queue.  Their sector corners are ordered by the persisted
`RustRedUnshiftedV1` complexity key.  At a corner, a proper subsector has fewer
active propagators and therefore precedes its supersector.  The compiler also
checks every pair explicitly under `max_dependency_checks`; it rejects a
queue if a later entry is a proper subsector of an earlier entry.

Excluded, proved-zero, resource-limited, and failed masks never enter this
queue.  The queue is work scheduling, not a list of proven-nonzero sectors or
masters.

## Intentional differences from LiteRed

- LiteRed calls failure of its sufficient zero predicate “nonzero” and closes
  that result upward.  RustRed retains the exact full-rank witness as
  unresolved and makes no analytic nonzero claim.
- LiteRed prunes predicate calls using monotone closure.  The existing
  `ZeroSectorAnalyzer` directly tests every admissible raw mask and caches
  identical effective masks (including formal power-shift support).  This is
  more audit-friendly; monotone zero closure is retained only as verified
  metadata.
- Missing cuts and pattern mismatches are `Excluded`, not zero certificates.
  A later zero-rule provider may separately interpret the supported cut
  semantics, but the inventory does not conflate those concepts.
- This slice uses the default Feynman-parametric proof.  LiteRed's alternate
  corner-IBP path, `SimpleSectors`/`BasisSectors` products, maximal-zero-mask
  compression, and generated `ZerojRule` remain separate future orchestration
  work.
- Formal nonintegral power shifts enlarge the effective Feynman-parameter
  support on an explicitly recorded generic locus.  Unsupported integer
  reindexing, integer-separated shifts, and shifted cuts fail with typed
  errors rather than silently changing raw sector membership.

The implementation has no topology names, loop-count branches, recurrence
tables, FORM calls, Mathematica execution, or master-count assumptions.
