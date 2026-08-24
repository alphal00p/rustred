# Generic residual fixed point and provider

Status: implemented and replay-tested on 2026-08-13. This is an intermediate
`SolvejSector` milestone, not a claim of complete LiteRed parity.

## Scope and non-goals

The implementation is topology- and loop-count-independent. Its production
inputs are an authenticated `IntegralFamily`, the generated Symbolica
`ParametricCoefficientContext`, a replayed family rule-system certificate,
ordering/search policies, and explicit resource limits. It receives no
topology label, loop count, recurrence, expected coefficient, master count, or
Vakint/FORM table.

The implementation is split between:

- `src/generated_family_fixed_point.rs`, which discovers and composes
  parametric candidates until a bounded residual fixed-point stop; and
- `src/generated_family_fixed_point_provider.rs`, which installs the exact
  latest material with the shared provider order
  `zero(symmetry(master(conditional(global))))`.

The current slice does not yet feed solved proper-subsector rules into
supersector elimination, infer masters, implement unrestricted LiteRed sector
symmetry discovery, or serialize certificates. Search exhaustion is always an
explicit residual status.

## LiteRed correspondence

LiteRed's `SolvejSector` keeps residual cases, chooses `startp`, constructs
`preparepoints`, eliminates generated relations at that ordering anchor,
compiles a guarded parametric rule with `WhenBad`, and feeds the remaining bad
domain back into the case queue. The relevant source is
`vendor/LiteRed2/Source/LiteRed2026.m:2428-2523`.

RustRed implements the same proof boundaries with typed Rust and Symbolica
rational polynomials:

1. retain the original family-wide generated IBP/LI row span once;
2. select residual sectors in the certified subsector-first inventory order;
3. retain every authenticated input candidate in its original priority order,
   then append only genuinely new search-backed phase-zero candidates selected
   by descending coverage;
4. rebuild a composition-only global `WhenBad` partition and live-leaf queue;
5. derive bounded concrete search witnesses from exact residual cases;
6. rerun adaptive elimination with the witness as the ordering anchor;
7. authenticate every candidate against the same generated row span;
8. evaluate candidate usefulness at the requested witness, not merely at the
   candidate's own discovery anchor;
9. append accepted candidates in deterministic priority order; and
10. rebuild the full global partition and queue before declaring any residual
    change.

An anchor is only a search and usefulness witness. It never proves that its
symbolic parent leaf is closed. Closure is determined only by the recomposed
Symbolica Boolean partition.

## Exact residual provenance

Every retained material is addressed by a stable locator:

```text
BaseRuleSystem { solve_ordinal }
BasePreparation { preparation_ordinal }
ResidualRound { round_ordinal, sector_attempt_ordinal }
```

Every request records one or more exact origins:

```text
CoordinateAssignment {
  material,
  work_item_ordinal,
}

CoordinateCompletionFrontier {
  material,
  work_item_ordinal,
  frontier_depth,
  within_frontier_ordinal,
}

ResidualFrontier {
  material,
  work_item_ordinal,
  frontier_depth,
  within_frontier_ordinal,
}
```

A coordinate-assignment request is admitted only when the completed integer
point reclassifies into the exact referenced input case. If an assignment
leaves coordinates free, deterministic exact L1 shells are enumerated only in
those free coordinates while assigned coordinates remain fixed. This matters
because an equality may hold while another predicate in the same residual
conjunction excludes the sector-corner completion.

An empty coordinate assignment is still a real symbolic residual. The bounded
scheduler enumerates deterministic exact L1 shells around the sector corner,
filters every point through the exact parent case, and retains the shell
locator. It does not invent a coordinate equality for a convenient numerator
point.

If the configured completion/frontier finds no new point, the final state is
`AnchorWitnessSearchExhaustedWithinConfiguredBounds`, with one of:

- `NoNewWitnessWithinConfiguredFrontier`;
- `MaximumLocalSearchDepthExhausted`; or
- `HeuristicStopOnNoStrictImprovement`.

This is bounded witness-search exhaustion, not proof that the residual integer
locus is empty and not a master declaration.

## Candidate provenance and semantic progress

Each adaptive request retains the full ordered candidate prefix through the
first applicable candidate, or the complete searched layers if none applies.
The locator is `(local_depth, within_layer_ordinal)`. Outcomes distinguish:

- unsupported authentication/proof capability;
- a certified candidate that does not cover the request point; and
- a certified candidate that covers it.

Accepted candidates refer back to either an exact phase-zero source ordinal or
an exact residual `(round, attempt, search, visit)` tuple. The owned
compilation remains in its search transcript; the composed V5 coverage stores
the exact payload in priority order and replay compares both.

Residual leaf or predicate counts are not a semantic partial order: splitting
a smaller bad locus may increase either count. Strict improvement is therefore
witnessed directly. Every newly selected request point must reclassify in the
new global material as `DescendingRule`. A concrete point that unexpectedly
classifies as `ProvedEmptyLocus` is a replay mismatch, not progress. Since all
prior candidates stay before appended candidates, composition is monotone at
every previously covered integer point.

## Replay and resource behavior

`GeneratedFamilyFixedPointCertificate::replay` first replays the base family
certificate, then recompiles the whole deterministic schedule. It compares:

- configuration, limits, fingerprints, and solve order;
- phase-zero search and composition material;
- every residual origin and candidate locator;
- candidate authentication and exact `WhenBad` payload;
- accepted-candidate priority;
- recomposed coverage and live queues;
- all final material locators and residual statuses; and
- the aggregate retained-proof census.

The compiler and provider bound rounds, attempts, anchors, frontier offsets and
components, enumeration transitions, candidate visits, retained
generated-source rows/terms/bytes,
conditions, locators, residual cells, and provider material. Arithmetic and
partition limits remain inherited from their authenticated lower-level
certificates. Resource interruption and non-resource failure are typed and
never converted to uncovered masters.

The provider resolves only each sector's exact latest material and checks the
shared row-span `Arc` throughout the in-memory graph. Pointer identity is a
scaling invariant, while payload replay is the mathematical persistence
boundary. Explicit terminals are the only master policy.

## Black-box validation

`tests/generated_family_fixed_point.rs` contains two adversarial scheduler
tests:

1. Massive tadpole: the generated candidate works at `I(2)` but not the
   terminal point `I(1)`. The fixed point retains the symbolic bad locus and
   never erases it merely because the candidate has a useful discovery point.
2. Equal-mass sunset sector `011`: the original residual has an empty
   coordinate assignment. The generic frontier schedules `J(-1,1,1)`, selects
   a freshly generated depth-one candidate, and obtains

   ```text
   J(-1,1,1) = J(0,0,1)/(d-1)
               + 2*m2*J(0,0,2)/(d-1)
               - J(0,1,0)/(d-1)
               + m2*J(0,1,1).
   ```

   The three one-line terms are later proved zero; they are not deleted from
   the parametric derivation.

`tests/generated_family_fixed_point_provider.rs` exercises the public provider
and demand engine. Frozen Vakint coefficients appear only in assertions. It
checks:

```text
I(2) = (d-2)/(2*m2) I(1)
I(3) = (d-4)(d-2)/(8*m2^2) I(1)
I(4) = (d-6)(d-4)(d-2)/(48*m2^3) I(1)

J(2,1,1)  = (d-3)/(3*m2) J(1,1,1)
J(-1,1,1) = m2 J(0,1,1).
```

The sunset test enables the generic exhaustive bounded vacuum-internal
symmetry compiler, which discovers and verifies all six S3 denominator
permutations. It does not call a two-loop reducer or Vakint adapter.

Licensed validation commands use GMP-enabled Symbolica and parallel nextest:

```bash
cargo nextest run -j4 --test generated_family_fixed_point
cargo nextest run -j4 --test generated_family_fixed_point_provider
cargo check --workspace --all-targets
```

On the current phase-zero-monotone snapshot, the fixed-point scheduler target
passes its tadpole and sunset replay tests. The provider target passes its
one-loop scalar oracle and fail-closed no-master/resource tests. Its connected
sunset numerator rule is generated and replayed, but the end-to-end `J(2,1,1)`
case currently stops at the still-unsupported `J(0,1,2)` factorized-sector
leaf. That is an honest two-loop completeness blocker, not a master inference
or an oracle mismatch.

`tests/generated_family_fixed_point_tensor_vakint_oracle.rs` additionally
passes the complete Symbolica `Atom` numerator boundary, generic tensor
projection/lowering, fixed-point provider, and demand engine for odd rank one,
free rank two, and free rank four. Frozen Vakint/alphaLoop coefficients occur
only in assertions. `cargo check` passes; remaining warnings are existing
vendored SIMD deprecations and deprecated tensor constructors elsewhere.

## Next required derivation layers

Three audited gaps prevent calling this a full `SolvejSector` port:

1. residual discovery needs replayable symbolic `startp` cases and Symbolica
   `K(n)` elimination over them, rather than only bounded concrete witnesses.
   Literal-integer/free-index cylinders are the first slice; full LiteRed
   parity also needs dependent substitutions such as `n1 -> 3-n2`;
2. each contiguous LiteRed case group needs persistent per-sector
   submit/solve/clean state across depth batches and remaining start points;
   the current implementation rebuilds a fresh local elimination for every
   concrete request; and
3. fully numeric submission needs proof-carrying zero-sector and verified
   symmetry quotienting before pivot selection.

LiteRed itself does **not** substitute solved proper-subsector rule tables into
`SolvejSector` elimination. It applies those tables later in
`IBPSelect`/`IBPReduce`. RustRed's runtime recursive provider already performs
that later application role. Feeding lower-sector rules into derivation may be
a useful Symbolica adaptation, but it is an optional authenticated
enhancement, not the definition of LiteRed parity.
