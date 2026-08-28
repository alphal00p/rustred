# Persistent cylindrical numeric quotient checkpoint

Status: **retired on 2026-08-28**. This document records an implementation that
was independently audited and replay-tested on 2026-08-20, but whose provider,
cylindrical coverage, and numeric-quotient proof stack has since been deleted.
The affine solver lane strictly subsumes the useful cylindrical geometry, and
the old stack was disconnected from the current app and private foundry path.

All present-tense language, file/test references, schema versions, and measured
results below describe that historical checkpoint only; they are not claims
about the live source tree. The document is retained as algorithmic evidence
for the former generic `SolvejSector` experiment, not as an implementation
guide or compatibility requirement. It was never a claim of complete LiteRed
parity or a complete parametric rule database for every integer point of an
arbitrary two-loop family.

## What the retired checkpoint implemented

RustRed can take the exact translated rows retained by an authenticated
cylindrical persistent-elimination certificate and perform LiteRed's numeric
prepare-point ordering:

1. replay and bind the exact generated family, coefficient context, sector,
   residual assignment, ordering, and persistent row source;
2. specialize every satisfiable retained row at the requested integer index
   point while conjoining its separately retained base-field assumptions;
3. prove each specialized term zero, cut-excluded, or related to a canonical
   key by a replayed verified family symmetry;
4. recollect terms by the witnessed concrete key;
5. rank the resulting concrete columns and eliminate exactly over the base
   field `K = Q(parameters)`; and
6. accept a unit pivot only after proving strict descent for every nonzero
   right-hand-side integral.

The proof retains the strong `Arc` for the exact persistent source, retained
row ordinals, raw guarded specializations, every quotient witness, collected
rows, column order, exact sparse-elimination transcript, selected pivot, final
guards, and descent witnesses. Standalone replay reconstructs and compares
that payload after the provider and reduction engine have been dropped.

The retired implementation was in:

- `src/certified_rewrite.rs`, proof arm
  `GeneratedCylindricalNumericQuotientElimination`;
- `src/certified_rule_provider.rs`, V3 constructor
  `try_new_with_persistent_cylindrical_sources` (with the singular constructor
  retained as a compatibility wrapper);
- `src/generated_cylindrical_family_source_set.rs`, V1 compiler and replayable
  certificate for a complete raw unresolved family queue; and
- the generic generated cylindrical row, persistent elimination, candidate,
  `WhenBad`, and coverage layers under `src/generated_cylindrical_*.rs`.

The family source-set compiler receives only the family, coefficient context,
sector restrictions, power-shift and integral-ordering policies, IBP and
row-span configurations, cylindrical depth, and resource limits. It compiles
the generic sector inventory and preflights the unresolved-source count,
source-index surface, and complete logical family binding census before any
generated-row algebra. It then creates one shared generated row span and uses
three strict family phases: all empty-cylinder roots, all V2 row systems, and
all V3 persistent eliminations. Exact remaining prepare-point, expanded-row,
retained-row, event, and pivot allowances are projected into each operation
before that operation starts and retained positionally for replay. The
certificate retains and replays the exact subsector-first solve order, every
per-source budget, and the exact shared inventory/row-span `Arc`s. V1
deliberately does not perform symmetry-unique or mapped-sector compression,
construct dependent affine or exceptional-locus sources, infer masters, or
build an application-time dependency and back-substitution database.

No production API receives a topology name, loop count, expected recurrence,
preferred pivot, or master coefficient. FORM and a Mathematica runtime are not
used. Symbolica supplies the exact GMP-backed polynomial and rational-function
arithmetic.

## Connected two-loop validation

The frozen validation family is the equal-mass vacuum sunset

```text
D1 = k1^2 - m2
D2 = k2^2 - m2
D3 = (k1+k2)^2 - m2.
```

For `L=2` and `E=0`, the ordinary generator produces exactly
`L(L+E)=4` parametric IBPs. A depth-one cylindrical source retains 28
translated rows and exposes 26 authenticated pivots. Exhaustive coverage
selects a generated descending rule for `J(2,1,1)` whose unquotiented right
hand side is

```text
J(2,1,1) = -J(0,1,2)/(2*m2)
             +J(1,0,2)/(2*m2)
             +(d-3)*J(1,1,1)/(2*m2)
             -J(1,1,2)/2.
```

The bounded vacuum symmetry compiler independently discovers all six
equal-mass denominator permutations. Applying their certified concrete
quotient before a new exact pivot selection cancels the equivalent pinched
terms and identifies `J(1,1,2)` with the active dot orbit. With only
`J(1,1,1)` selected explicitly as a master, the demand reducer derives

```text
J(2,1,1) = (d-3)/(3*m2) J(1,1,1).
```

The expected coefficient occurs only in the test assertion. The accepted
rewrite must contain the persistent-source proof arm, so an adaptive rewrite
cannot satisfy the assertion through an older path.

The regression is
`tests/generated_cylindrical_sunset_provider.rs`:

```bash
cargo nextest run -j4 \
  --test generated_cylindrical_sunset_provider \
  -E 'test(all_generated_global_pivots_cover_sunset_j211_and_numeric_quotient_closes_it)' \
  --nocapture
```

The initial licensed GMP run passed `1/1` in 118.6 seconds with 28 prefix
elimination builds and 406 cumulative prefix-row submissions. After replacing
that construction with the V3 one-pass transcript and removing redundant
operation-local source replay, the same test passed in 23.134 seconds. The
persistent stage now performs one exact build over 28 rows while retaining the
same 26 pivot events. A test-only V2 oracle compares the complete semantic
event, pivot, trace, guard, and closure payload. The proof also replayed
successfully after provider destruction. Focused proof-boundary, resource, and
V2/V3 equivalence suites passed, and an independent source/replay audit found
no remaining must-fix soundness defect.

## Current-path one-loop and family-wide two-loop acceptance

`tests/generated_cylindrical_one_loop_scalar_oracle.rs` exercises only the
current root -> row-system -> V3 persistent elimination -> numeric quotient
path. Every adaptive work/output capacity is zero. The test reduces powers 2
through 6 to the selected master `I(1)`, proves powers 0, -1, and -2 zero,
requires the exact persistent proof arm for every nontrivial step, checks the
required `m2 != 0` guard, drops the engine and caller-owned source, and replays
the retained proofs. The licensed parallel run passed `1/1`.

`tests/generated_cylindrical_one_loop_symbolica_tensor_oracle.rs` sends
Vakint-syntax Symbolica `Atom` numerators of ranks one, two, and four through
the native parser, vacuum tensor projector, denominator lowering, and the same
current persistent numeric-quotient provider. Vacuum isotropy removes the odd
rank before scalar IBP. The rank-two and rank-four terms produce the exact
metric structures and coefficients frozen from the Vakint oracle, while every
non-master scalar witness `I(2)`, `I(3)`, and `I(4)` uses the persistent source.
The adaptive fallback is made deliberately hostile: depth and every offset,
scout, candidate, cache, elimination, and rule-output capacity are zero, and
the provider must retain that exact configuration. Its statistics remain
zero. After all family-rule construction/provider handles are dropped, the
retained scalar proofs replay provider-free and the authenticated tensor
results verify. An independent licensed parallel run passed `1/1` in `0.437`
seconds.

`tests/vakint_two_loop_tensor_ibp_oracle.rs` now sends Vakint's two-loop
covariant tensor fixture through native vacuum projection and denominator
lowering, but obtains every scalar rule from the same automatic raw-family
source-set compiler. The concrete equal-mass sunset family is a validation
input only: production receives no topology label, recurrence, pivot, master
count, or loop-count dispatch. The test derives the raw `011`, `101`, `110`,
and `111` sources from one four-row generated span, makes every adaptive work
and output capacity zero, requires its work counters to remain zero, and
accepts only certified persistent, symmetry, or zero traces. It reproduces the
frozen Vakint/alphaLoop oracle

```text
g12 [d/2 J011 + d*m2/3 J111] - (p2.p3)/6 J111
```

and replays after the engine and every caller-owned family-source handle have
been dropped. The licensed parallel run passed `1/1` in `135.228` seconds.

`tests/generated_cylindrical_sunset_family_oracle.rs` now invokes the generic
family source-set compiler solely from the family, context, restrictions,
policies, configurations, depth, and limits. The acceptance asserts that the
derived raw solve queue is exactly `011`, `101`, `110`, `111`, that every root
retains one exact shared inventory and generated row-span allocation, and then
passes the compiler-produced sources directly to provider V3. It freezes only
the Vakint validation expectations and requires the generated current path to
reduce the following representatives and every distinct `S3` image:

```text
J(0,2,1)  = (d-2)/(2*m2) J(0,1,1)
J(0,2,2)  = (d-2)^2/(4*m2^2) J(0,1,1)
J(-1,1,1) = m2 J(0,1,1)
J(-2,1,1) = m2^2 (1+4/d) J(0,1,1)
J(2,1,1)  = (d-3)/(3*m2) J(1,1,1)
J(0,0,1)  = 0.
```

The test permits only certified symmetry steps and persistent cylindrical
numeric-quotient steps. It verifies that canonical factorized demands select
the exact `011` source, connected demands select `111`, and the symmetry-mapped
raw `101`/`110` sources are not selected. It then drops the engine and every
caller-owned source-set/source/inventory/row-span handle before standalone
proof replay. The final automatic-source-set licensed run passed `1/1` in
`101.627` seconds. After aggregate resource gates were moved ahead of the work
they govern, the focused generic family source-set suite independently passed
`12/12`. It covers exact and one-below family totals for prepare points,
expanded rows, retained rows, future persistent events, and pivots; exact and
one-below logical binding/replay censuses; stage precedence; an empty queue
that performs no row-span algebra; four-source sequential budget replay; and
typed failure precedence. A private provenance-tamper test also passed `1/1`.

## LiteRed correspondence

This ordering follows the relevant part of
`vendor/LiteRed2/Source/LiteRed2026.m:2471-2481`: LiteRed constructs numeric
prepare points, evaluates generated IBP identities and symmetry relations,
applies zero rules, and only then solves the concrete system. Eliminating a
generic `K(n)` system first would be insufficient because specialization can
change rank or make a generic pivot vanish.

The implementation deliberately does not globally identify symbolic
`K(n)` integrals under a denominator permutation. Whole-row symbolic symmetry
transport remains a separate sound operation; the `SR`-style quotient is
performed on concrete specialized terms immediately before concrete
elimination.

Provider V3 retains a bounded immutable collection of authenticated sources,
ordered deterministically by sector, decreasing partial-assignment
specificity, and canonical assignment entries. At a canonical concrete
demand it tries every matching source in that order before entering the older
adaptive scout and generic-pivot fallbacks. Duplicate exact
sector/assignment scopes and foreign family/context/order sources are rejected
at construction. The accepted rewrite continues to retain the exact selected
source `Arc`, so standalone proof replay does not trust collection position or
provider lifetime.

## Exact status boundary

This checkpoint proves that the generic components can derive and apply the
strict one-loop scalar and Symbolica-tensor oracles and the finite family-wide
two-loop scalar matrix above without an authored recurrence. It does not yet
prove:

- finite parametric coverage of every integer point in every sunset sector;
- recursive closure of every exceptional locus;
- symmetry-unique/mapped-sector orbit compression or family compilation of
  dependent affine and exceptional-locus sources (V1 emits one empty-root
  source for every raw unresolved sector only);
- a reusable certified proper-subsector selection/dependency/back-substitution
  database;
- automatic master discovery or master minimality;
- complete coverage of arbitrary two-loop families; or
- connected three-, four-, and five-loop sector closure.

Concrete loop- or topology-named reducers and Vakint/alphaLoop fixtures remain
validation oracles only. Their authored recurrences are not imported into this
path. In particular, the sunset fixture is not a complete enumeration of
inequivalent two-loop vacuum skeletons; broader topology coverage is a
validation task, not a production dispatch mechanism.

## Immediate continuation

The next generic solver work is:

1. **Completed and independently audited:** the transactional matcher-bound residual-affine `WhenBad` owner:
   signed descent, canonical conditions, affine boundary pullbacks, numerator
   gates, and a replayable `Certified`/`IdenticallyBad`/`Unsupported` result.
   Its licensed GMP parallel suite passed `40/40` on 2026-08-20;
2. **In progress:** add the deterministic affine group pass in which only a certified result
   consumes a target;
3. extend the replayable raw family source set with proved
   unique/mapped-sector orbit metadata and iterate dependent/exceptional
   sources to a typed fixed point; and
4. retain a certified application-time proper-subsector dependency and
   back-substitution DAG, never treating search exhaustion as a master proof.

The scalar one-loop, one-loop Tensor/Atom, and finite two-loop scalar/tensor
validation matrices now use the current automatic family path. The next work
is the missing generic fixed-point/dependency machinery above, followed by
broader two-loop validation families and then connected three-, four-, and
five-loop massive-vacuum validation. Those concrete families must continue to
serve only as tests of the generic compiler.
