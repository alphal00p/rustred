# Executable SpIRed reference port

## Objective

Port `vendor/spired/src/solver.tpp::solveSector` faithfully to Rust, using
Symbolica/numerica for all CAS and finite-field arithmetic. The acceptance
scope is every supplied one- through three-loop (2–4PM) example, with equal or
better release timing and identical or algebraically smaller rules. This is
not satisfied by a fast single case, an incomplete sector, or a vacuum-only
subset. Reference source and generated reference artifacts remain local.
The [supplied-example census](spired_pm_acceptance.md) records all nine
in-scope generators, their actual selected sectors, and their ordering/data
requirements. No one-loop C++ generator is currently supplied; that missing
reference must not be replaced by an invented benchmark claim.

## Why the existing notes-based search is not the reference algorithm

The executable reference exposes several material differences:

- Integral order is harder-first SpIRed/LiteRed order, including symbolic vs
  fixed powers, sector signs, cuts, total degree, numerator degree, and
  coordinate tie-breaks. RustRed V1 order cannot be used as a substitute.
- Sources are preconditioned once per sector by GCD-scaled polynomial forward
  and backward cancellation. The reference does not first make a rational RREF.
- Any leading integral compatible with the current equalities solves the case.
  Its symbolic shifts are removed afterward. Thus an unconstrained symbolic
  case accepts the first nonzero prepared row directly at seed depth zero.
- Otherwise one incremental finite-field reducer processes all columns. A
  matching pivot triggers L-pattern ancestor extraction, ordered by pivot
  column, followed by a compact exact forward solve. No full RREF is needed.
- Fixed-index cases have a separate bounded seed search; unresolved ones are
  designated masters. Multiple fixed cases share seeding and elimination.
- Coupled linear equalities are executable cases, not sampled rectangles.

The reference's nonlinear exceptional simplification uses a bounded integer
search (`|n_i| <= 30`). This is a heuristic, not a general integer zero-locus
proof. The policy for this behavior and for residual-master classification has
been submitted to the user; neither should be silently resolved by claiming
stronger closure than the search establishes.

## Architecture and implementation sequence

1. Add compact solver primitives: one-byte symbolic/fixed powers, inline
   const-generic integral arrays, reference ordering, and allocation-free L1
   seeding in reference chronology. Structural integer arithmetic is checked;
   arithmetic on coefficients and field values belongs to Symbolica/numerica.
2. Lower existing generic RustRed IBPs once to immutable native polynomial
   rows. Share their variable maps and avoid per-term context wrappers. Port
   reference polynomial source preconditioning, including coefficient tie-breaks
   in indices-first DEGLEX order.
3. Implement direct compatible-leading extraction and canonical translation.
   Add the single native modular reducer, accepted-source tracking, L-pattern
   pruning, and native exact replay. Keep source conditions and case boundaries
   explicit; a returned equation is not yet an unconditional rewrite rule.
4. Implement the ordered equality-case queue, exact denominator/activation
   exceptions, subsumption, coupled linear cases, and shared numerical search.
   Port cut-derivative preparation and LI sources needed by the PM fixtures.
5. Exercise complete reference examples, compare rules algebraically and case
   coverage explicitly, and profile release runs. Fix the dominant measured
   costs rather than impose previous search machinery on the reference flow.
6. Connect accepted solver results to artifact certification/publication and
   user interfaces without adding that ownership bookkeeping to the hot rows.

The new cohesive `rustred::solver` module is the source-port implementation.
The existing `foundry::completion::spired` remains the prior notes-based
implementation during this transition; it must not be confused with a verified
port or used to claim reference-performance acceptance. Remove/replace redundant
search paths only once their consumers have moved, preserving independent
artifact mathematics and the optional Janet/Ore strategy.

## Native API audit

The checked vendored public APIs provide the required primitives:

- `SparseRowReducer::add_row`, `add_cols`, `LuLMode::Pattern`, `l`, `u`, and
  `pivots` implement the same dense-scratch/first-free-pivot streaming algorithm
  as C++ `gpluRow`. Do not write another elimination kernel.
- Native L includes rows for dependent inputs; accepted U-row ordinals need a
  separate mapping to L input slices. A trailing unused column avoids native
  full-rank early-return corner cases during dynamic insertion.
- `RationalPolynomial<IntegerRing,u16>` and `MultivariatePolynomial` are the
  coefficient types. Native GCD, exact division, `shift_var`, `replace`, and
  `evaluate_with_coeff_map` provide polynomial operations. Coefficient ordering
  is application scheduling, not a replacement polynomial representation.
- Exact replay uses `RationalPolynomialField<IntegerRing,u16>` with the same
  native sparse reducer. No custom rational reconstruction is introduced.

One measured-storage caveat in the current native API: Pattern L retains a
slice for every dependent input, whereas the C++ reducer discards that slice.
The Rust mapping handles accepted/dependent row identities correctly, but this
is not yet equivalent memory scaling. Both retained L rows and entries are
reported explicitly. The current public API has no safe way to drop only a
dependent slice while retaining U, pivots, and ongoing pattern recording; a
small upstream opt-in operation would remove this difference without changing
any arithmetic kernel. Do not substitute a custom Rust elimination routine.

Native polynomial parsing may construct a new, equal variable-map `Arc`.
Ordering and variable identities, not pointer equality to a caller template,
are the mathematical invariant. Let Symbolica own its context sharing.

## Reference corrections and explicit differences

The user approved preserving a complete homogeneous identity in the shared
numerical solver (2026-09-14). In the uploaded C++ `solveNumCases`, the direct
hit path calls the mutating `ibp::minusRest()` and can then submit that same
modified object to GPLU. The object now contains only the RHS, not an identity
equal to zero. Rust clones only for direct-rule extraction and keeps the
original row intact for the shared reducer. A regression uses
`I(n+1)-I(n)=0`: discovering `I(2)=I(1)` must not falsely imply `I(1)=0`.

The shared numeric row cutoff also uses the actual easiest requested integral
under the integral ordering. The last case in equality-case lexicographic
order is not necessarily that integral. Case visitation order itself remains
the reference's order. Fully numerical initial cases, when every coordinate
is a removed cut, use the bounded numerical phase directly; this edge differs
from C++ starting every initial case through `solveCase`.

These differences are not performance optimizations justified by the vacuum
fixture. They preserve exact equations and explicit bounded-search semantics
for the generic API and are recorded rather than hidden as reference parity.

## Bounded parallel execution

`SectorExecutor` is a reusable, local Rayon executor. The one-worker path runs
on the caller thread, preserving serial behavior and restricted-license thread
ownership. Multicore construction uses Symbolica's public license capability
service. It does not change global thread pools, environment variables, or
native CAS settings. The configured pool bounds this executor and native Rayon
work invoked within it, not unrelated pools that a caller might create.

Workers borrow the same `SourceSystem` and share the immutable zero-sector
census through `Arc`. Each sector still needs its own mutable preconditioned
basis, case queue, numerical probe, and exact replay buffers. Worker callbacks
consume completed solutions immediately, allowing direct per-sector output and
compact result collection. Retaining all solutions is an explicit caller choice,
used by the example only for oracle validation. No coefficient serialization
or worker-to-worker expression copying is required.

The default scheduling order is active-coordinate count descending, then sector
lexicographic order. This generic structural heuristic starts likely-expensive
sectors earlier; it does not alter ordering or seeding within a sector. Results
and the first reported error follow the original manifest order regardless of
completion order. A separate input-order policy permits controlled comparisons.

Native API audit for this slice checked the public sparse-reducer operations,
their implementations, and their actual call paths: incremental forward GPLU,
polynomial/rational arithmetic, GCD, and factorization are serial here. The native
`back_substitute_parallel` exists but is not called by this forward-only port.
No BLAS/OpenMP dependency is used by this Rust path. Nested compute pools are
therefore not introduced by the currently used CAS operations. Rational-function
reconstruction is not implemented; it remains an upstream Symbolica dependency.

The first parallel slice passes all 86 focused solver tests and the workspace
check. Release runs at 1/2/4/6 workers reproduce all 617 `vac3` equations,
coordinate guards, and sector signs; independent audits confirm identical
residuals and output payloads. See the
[parallel measurements](spired_parallel_results.md) for both retained seven-pair
benchmark batches, memory figures, host contention, and remaining scaling limits.

## Opt-in search phase diagnostics (2026-09-15)

`SectorSolver::solve_case_with_observer` exposes `SearchEvent` milestones:
depth/power-of-two seed progress, the winning modular pivot and exact dependency
trace, the prepared exact frame, each selected exact row's native reduction,
and canonicalization. The sector observer forwards these with the active case
and adds guard-extraction and exceptional-geometry boundaries. Observations do
not change source enumeration, pivot selection or exact arithmetic. The no-op
path adds no formatting, synchronization, clock reads or exact-row copies.

The executable's `RUSTRED_SPIRED_PROGRESS=1` output includes every coupled
equality at case entry, not only the integral pattern. Native GPLU column,
pivot, U-nonzero and L-entry counts describe discovery state at the hit;
the separate dependency-trace count identifies the selected source rows,
not the compact exact frame's column count or fill.
The subsequent `exact-frame` event reports that compact frame's actual column
union, stored input terms, and full/active coefficient-variable counts.
`exact-row-start` / `exact-row-finish` bracket each selected source row, including
dependent rows, and expose native exact U size. Row ordinals are one-based;
column positions are zero-based. The structural zero sentinel is excluded from
integral-column counts. There is still no per-row modular progress event.
`gplu_exact_and_canonicalization_us` describes the modular-hit path only;
direct-hit normalization is included in total search time. Optional diagnostic
I/O must not be compared as if it were a silent benchmark run. Four focused
solver tests cover unchanged direct/modular identities, a bounded miss and
sector event order; they pass within the 211-test solver/source-replay batch.

### Native exact-frame variable compaction

Selected rows can retain a full coefficient-variable map even when fixed-index
specialization has removed most variables from every numerator and denominator.
Before exact GPLU, RustRed now computes their support union with native
`MultivariatePolynomial::contains` and uses Symbolica's checked
`rearrange_with_growth` to remove only globally absent variables. The active
variables keep their original relative order by default. Every output coefficient is
restored to the full original map before canonicalization, source replay or
guard extraction. Physical integral columns, source chronology and the target
stop are unchanged; this is not a kinematic substitution or reconstruction.

This can unlock Symbolica's existing packed-exponent polynomial division:
the inspected native implementation selects that path at at most eight
variables with degrees at most 127 (or at most four variables with degrees at
most 32767). A 17-slot map bypasses it even if only six variables actually
occur. This is an implementation-motivated optimization, not a speedup claim
until the same release workloads are measured.

Five focused helper tests cover denominator-only variables, empty/constant
frames, unchanged maps, incompatible contexts and checked remapping failures.
An exact GPLU fixture verifies a 17-to-2-variable reduction restores the same
full-context rational identity. Native row-event tests also check dependent
rows, explicit zero input coefficients and the early target stop. The combined
solver/source-port batch passes 225 tests; all 53 example tests pass.

An opt-in `SectorConfig::coefficient_variable_order` now permits reversing the
active coefficient variables during single-target exact lifting. This tests
native polynomial multiplication/division/GCD sensitivity without changing
integral ordering, source chronology, modular discovery or numerical-tail
lifting. Both sparse and dense exact backends use the same checked native
remapping and restore the original context before canonicalization and guard
extraction. Native fraction construction also restores the correct leading
denominator sign; a bijective permutation requires no new GCD. `Original`
remains the default. The example drivers expose this as
`RUSTRED_SPIRED_COEFFICIENT_VARIABLE_ORDER=original|reverse` and record the
choice. This is an experimental representation policy, not a demonstrated
speedup or a substitute for the remaining full PM acceptance gate.
Both policies now pass 28 exact release regression jobs each; six further
source audits also pass. The two capped hard-sector probes still stop inside
the same exact row. See the [recorded ordering experiment](spired_pm_acceptance.md#native-coefficient-variable-ordering-probe-2026-09-15)
for process boundaries, memory and the distinction between partial progress
and a completed-workload timing.

`IndicesFirst` is a further opt-in policy, exposed as `indices-first` in the
same environment setting. It consumes the existing explicit source coefficient
priority rather than inferring an index suffix or recognizing variable names.
Its full permutation is checked before absent variables are removed, including
constant frames. The native rational field and polynomial ring expose no
monomial-order type parameter in the pinned/current APIs: this variable-layout
probe therefore remains Lex-based and does not implement C++ DEGLEX arithmetic.
All exact coefficients return to the original map through the same native gate.
The 34 release regression/ordering/source-audit checks pass, but the matching
120-second hard-sector probe remains inside row 293 of the same exact frame.
This does not solve the remaining PM sector; `Original` stays the default.
See the [indices-first results](spired_pm_acceptance.md#indices-first-follow-up-2026-09-15).

### Opt-in target-block exact lifting

`SymbolicExactBackend::SparseTargetOnly` is an experimental alternative exact
schedule, selected in the example drivers with
`RUSTRED_SPIRED_SYMBOLIC_EXACT_BACKEND=sparse-target-only`. `Sparse` remains
the default; modular discovery, source selection and the shared numerical
tail are unchanged. The new option is not rational reconstruction.

For the unchanged selected source prefix, write `A = [F | R]`, with `F`
containing every harder integral and the target column. Native sparse GPLU
eliminates only `F`, retaining its native full lower factor `L`. On the first
target pivot, native triangular normalization/back-substitution solves
`Lᵀ w = e_target_row`; one native sparse product `wᵀ A` then reconstructs the
entire monic identity. This avoids updating every easier-integral column during
each forward step. The original coefficient map is restored before ordinary
target canonicalization and exception extraction.

Every submitted `F` row must be independent. An empty or dependent prefix row
is a typed error, not silently dropped or sent to an arbitrary-solution solver.
Square/nonzero-diagonal `L`, insertion-order mapping, native triangular pivots,
and the final unit target/zero forbidden columns are checked. Under these
conditions the source weights are unique and the returned identity equals
the default full-row GPLU result. Temporary frame weights do not become
original-generator certificates: the independent artifact replay, source and
weight guards, descent and unbounded-domain coverage gates remain necessary.

All coefficient elimination, normalization, back-substitution and multiplication
use the pinned Symbolica/Numerica public APIs. RustRed only arranges sparse
coordinates and enforces the shape/identity contract. Progress separates
target-block elimination, weight solving and full reconstruction. Full `L`
storage and rational weight growth can outweigh the saved tail work; use an
external time/memory cap and measure before choosing this option in a campaign.

An initial release comparison on the same host confirms why this remains
opt-in: K1 sector `1` took 5,607 us with target-only versus 121 us with the
default sparse path, and K2 sector `111` took 1,554 us versus 814 us. A
representative K3 sector `111111` took 65,595 us versus 66,892 us (about 1.9%
faster). The generated rule files were byte-identical in all three pairs.
These are single-worker smoke measurements, not a controlled benchmark or a
claim of SpIRed parity; they do establish that the implementation is wired
through release binaries and that its extra triangular/product work can be
worthwhile only once the target block is large enough.

A fresh 120-second release probe on the difficult PM sector
`fam1_112/111010100001111` used the target-only backend with progress tracing.
It reached the start of row 298 in the 298-row exact frame and was then capped
(exit 124), with approximately 347 MiB peak RSS and no sector output. The
comparable default-path probe had reached row 293 at its cap, so this trace is
not a speedup: target-only's additional lower-triangular solve/reconstruction
never began because the final target pivot was not found. This is a bounded
diagnostic, not a completion or a paired benchmark; Symbolica activation
succeeded in the retry.

## Per-job orderings and integrated affine cases (2026-09-14)

`SectorExecutor::map_configured_with_observer` accepts a per-job configuration
function, identified by the original manifest ordinal and sector. This permits
independent ordering choices, including repeated sector masks, while borrowing
the same immutable `SourceSystem`. It does not mutate a shared ordering table
or clone a source system per worker. Results and the selected failure retain
manifest order; callbacks and progress remain live and nondeterministic.
The example driver accepts an explicit ordering file, including the supplied
16 `fam1_112` overrides as checked input data outside the engine, followed by
an optional `active-first` or `input-order` scheduling policy. Scheduling
changes job dispatch, not the mathematical ordering within any sector.

`Case<N>` now connects equality geometry to `solve_case`, the sector queue,
rule guards, and the example's exact oracle comparison. Its coordinate variant
stores the existing `CoordinateCase<N>` inline; its affine variant shares an
immutable `AffineCase<N>` through `Arc`. Coordinate-only work retains its
matrix-free fast path. The queue compares canonical constraints in the
reference's order and removes a narrower pending domain only when exact
containment proves it redundant. Fully fixed intersections return to the
existing shared numerical search. An unsupported exceptional intersection is
never interpreted as empty or covered.

`AffineCase` uses native rational reduced elimination, exact integer
divisibility checks, and integral or rational computational charts. `AffineIntersection`
distinguishes a proved-empty domain, a coordinate-only face, and a genuinely
coupled case. For example, `n0=n1` leaves two distinct physical integral axes:
`I(n0+1,n1)` and `I(n0,n1+1)` must not be merged merely because their base
indices obey an equality. The chart restricts coefficients, not integral keys.

Ordinary sources are translated **before** chart restriction. On `n0=n1`, a
source coefficient `n0-n1` translated by `(s0,s1)` becomes `s0-s1`, so a
transverse source can supply a necessary nonzero equation. Restricting first
would incorrectly erase it. All ordinary source seeds therefore remain
eligible; only a selected target displacement must satisfy the homogeneous
tangency test `A*s=0`. This permits recentering the winning rule without
changing its required affine domain. Search validates the immutable source and
chart maps once at case entry, then uses native substitution without repeated
per-term boundary checks.

The represented case is always the original integer coordinates, exact
equalities and sector signs. A rational chart is used only for coefficient
restriction: `2*a-b=4` permits substituting `(b+4)/2` for `a` in a coefficient,
but does not declare every integer `b` admissible. Tangency, exported equations
and reference-compatible queue order use native whole-row primitive integer
normalizations, not individual numerators of rational RREF entries.

The chart has separate APIs for `restrict_equation` (zero locus only),
`restrict_polynomial_value` (exact value), and `restrict_coefficient` (exact
quotient). Thus `a*I0+I1=0` restricts to `I0=-2/(b+4)*I1`, not
`-1/(b+4)*I1`. Native `map_coeff`, `replace_with_poly`, and the public joint
`FromNumeratorAndDenominator<Q,IntegerRing,u16>` conversion perform all
arithmetic. A denominator vanishing on the case returns
`UndefinedCoefficient`. Coordinate-only and integral-chart work retain their
integer-polynomial fast paths; no rational reconstruction is implemented.

General integer-lattice and coupled sector-inequality feasibility is not
decided. A necessary native-integer row-bound check now rejects a canonical
linear equality whose right-hand side lies outside its sector-sign bounds.
For example, `a=b` is impossible when `a<=0` and `b>=1`. Fixed coordinates
contribute exactly; unfixed coordinates retain genuine unbounded endpoints.
This check also runs when intersecting an existing affine case with a different
sector and no new equations. It does not iterate inequalities or implement an
LP solver. The public Symbolica `Atom::solve` inequality declaration, its
`wrt_with_exponent` implementation and its regression test were checked: that
path currently returns `InequalitiesNotSupported`; native `Integer` arithmetic
and comparison supply every numerical operation in this small domain check.

Proved contradictions are discarded; uncertain integer-empty domains
may remain and cost extra work. Containment is conservative, and only an
actually fully fixed case enters numerical search. Unsupported nonlinear
equations remain explicit errors. None of these limits establishes closure by
discarding a feasible branch.

### Cold exact normalization of joint exceptional equations

Coordinate intersection now has a narrowly triggered native joint-ideal
fallback. If multiple exceptional equations resist the ordinary coordinate
path and at least one is nonlinear, Symbolica constructs an exact rational
Gröbner basis. Native primitive normalization clears coefficient denominators,
and the coordinate intersection is retried once. This is a cold guard-geometry
operation, not another elimination kernel in the source-search hot path; it
does not enumerate a bounded box of integer points.

The motivating `fam1_111` conjunction contains two nonlinear equations in
`a=n4`, `b=n14` together with `8*a-b-7=0`. Their joint ideal reduces to
`a-1=0`, `b-1=0`; considering the equations separately had previously rejected
that coordinate corner. The new exact normalization regression covers the
actual conjunction, inconsistent ideals, rational content, nonprefix index
maps, and remaining unsupported geometry. A nonlinear conjunction whose basis
still requires a coupled affine admission is a known conservative limitation:
the coordinate retry preserves the original unsupported equations rather than
passing a transformed coupled basis into a second admission path.

### Validation boundary for this integration

The oracle helper now matches complete nonempty required-case sets, compares
exact RHS coefficients after native restriction to each required domain, and
compares exceptional domains by exact intersection and containment. It does
not identify physical integral axes or specialize symbolic parameters.
Proved integer-empty reference cases are counted explicitly rather than
requiring a meaningless Rust rule. Reference files are read only after all
requested sectors have independently run.

The actual new C++ runs require this distinction: `fam1_12` has two feasible
equal-index faces and also exports an integer-empty `2*n5-2*n6=1` branch;
`fam1_111` has 83 coupled-domain rule LHSs across 12 sectors. Copying a sampled
integer point or dropping those faces cannot meet the reference-port goal.

The integrated focused suite passes **142 solver tests and 39 example tests**;
the workspace all-targets check also passes. Tests include transverse-source
affine search with exact replay, preservation of physical columns, native-map
rejection, affine queue/guard semantics, cold joint-ideal normalization, and
whole-sector affine oracle comparisons. Separate agents have independently
audited implementation and mathematical correctness. These correctness tests
use the normal test profile; benchmark timings use only the separately built
release executable.

Full `fam1_12` and `fam1_111` release campaigns now pass at one and six workers:
1,104 nonempty rules over 40 sectors and 10,333 rules over 132 sectors. Exact
domains, coefficients, guards, and all 32/26 residual keys match C++.
The earlier 38/40 and 119/132 runs are historical diagnostics, not the current
acceptance status. See the [affine-case report](spired_affine_results.md) for
release measurements and remaining scope, and the
[additional-fixture report](spired_additional_fixtures.md) for earlier observations.

## Validation and current baseline

Each implementation slice gets independent audit and focused differential
tests. Cover compact overflow, exact ordering, seed chronology, source span,
dynamic columns, dependent input rows, matching shifted pivots, exact replay,
guard branches, and genuinely numerical residual cases. Test real family
preparation end to end, not just hand-authored rows.

Current C++ release measurements on the AMD EPYC 9754 host:

| Workload | Serial process wall | Six-worker process wall | Rules |
| --- | --- | --- | --- |
| `fam1_11` | median 0.75 s (3 runs) | median 0.14 s (3 runs) | 802 sector + 2 pre-rules |
| `vac3` | 1.44 s (1 run) | 0.31 s (1 run) | 617 |
| `vac4` | 72.67 s (1 run) | 45.40 s (1 run) | 1272 |

The `vac4` example selects 27 sectors, not all 743 nonzero sectors. Counts of
residual masters are not independence proofs. Logs, output files, build options,
and exact workload details are local under
`vendor/spired/build-release/benchmarks`. Timings exclude compilation but include
process initialization and output; compare the same boundary on both sides.
The newer paired `vac3` comparison below covers that complete reference run;
the PM suite and Rust `vac4` comparison remain outstanding.

## First implemented slice (2026-09-14)

The compact coordinate-case search, source adapter/preconditioner, native
discovery, pruned exact replay, and exact denominator/activation exception
extractor are implemented. All 46 focused tests passed in the integrated run;
the first 34 and subsequent exception mathematics received separate delegated
verification/audits. Compiled C++ fixtures validate 20,000
mixed-power order comparisons, 377 seeds through three-coordinate L1 depth six,
and four polynomial-preconditioning examples.

The release `spired-solve-case` example ran the genuine ordinary-source K1,
K3, and K6 generic cases and the five successive trailing-one coordinate faces
of the full K6 sector. These all produced a compatible source-row candidate at
seed depth zero. For one observed K6 generic run, source/family preparation was
1.501 ms, sector preconditioning 0.393 ms, and case search 0.119 ms. These are
**single-case component measurements**, not family closure timings or a claim
of benchmark parity. The all-one K6 corner exhausted depth three after 756
rows in a separately timed 0.06 s process; it remains a numerical residual, not
an automatically certified master.

An example-only comparator then matched the exact RHS of **all six** full-top
K6 coordinate cases against the C++ `111111.dat` export with explicit `m=1`
specialization. Each Rust candidate is generated independently before the
reference is read. This comparison is coefficient-by-coefficient native CAS,
not string equality. Five separate example-comparator tests exercise native
normalization, coordinate mapping, explicit mass substitution, pole/mismatch
rejection, and syntax validation. It does not compare guard coverage or claim
sector closure.

The subsequent automatic-sector slice implements equality-case traversal,
exact coordinate intersections, guard-aware subsumption, and the reference's
shared numerical search with one native modular system and one union-trace
exact solve. Its first combined run passed 70 focused tests. A full optimized
`vac3` campaign then independently generated all 38 sectors, 617 rules, and
38 finite residuals; all 617 RHS equations matched C++ with symbolic mass, and
an independent audit matched the coordinate guard domains and residual set.
Three latest interleaved serial repeats from the normal Cargo release driver
measured median process times of 0.66 s Rust and 1.47 s C++, with output-format
and shared-host caveats documented in
[the complete benchmark report](spired_vac3_results.md).

An LI source adapter and expanded guard comparator have also been implemented
and independently reviewed. The expanded integrated solver suite passes all
77 tests, including LI source order/signs and the actual vacuum numerical
boundary equation. The updated `--release --locked` driver again generated
all 617 rules and passed programmatic exact RHS, coordinate-guard, and
sector-sign comparison. Its equations and residual outputs are byte-identical
to the first run. All 12 example-comparator tests also passed in release mode,
and `cargo check --locked --workspace` passed with an explicitly selected
installed Python interpreter. The targeted Symbolica variable-map ownership
migration regression passed as well. The subsequent linear-cut/ordering slice
is described below; affine search is now integrated as documented above, while
complete PM fixture coverage still requires `fam1_112` and the ordering studies. `RuleCandidate` still
does not claim unconditional applicability or publish artifacts; `SectorRule`
adds its exact exceptional conditions, and `SectorSolution` explicitly retains
bounded finite residuals without declaring certified master independence.

Keep user worktree changes intact. Every Git operation uses the requested
ValentinHirschi name/email, and no reference-only material is committed or pushed.

## Removed linear cuts, fixed sources, and ordering overrides (2026-09-14)

`prepare_linear_cuts` now implements the once-per-family preparation needed
before the PM sector benchmarks. For an admitted cut `D_i = c k_a·u`, it
obtains the ordinary identity `u·∂/∂k_a`, solves for its raised cut, and
substitutes positive cut shifts throughout the ordinary/LI frame. Both exact
coefficients and integral coordinates are translated. It exports a
`LinearCutRule` and then specializes that cut to power one in the prepared
frame. Terms pinching a removed cut vanish. No reference rule or topology name
is consulted by this preparation.

The exported pre-rule is exceptional at `n_i=1`, not zero as one C++ comment
suggests. Its nonzero parameter pivot factor is retained before any coefficient
cancellation. The improved frame instead lives at `n_i=1`; the two objects
have different domains and must not inherit each other's guard. Native
`shift_var`, `replace`, rational arithmetic, GCD, and exact division perform
all coefficient operations; shared denominator clearing is a small row adapter
over those services, not a new polynomial-arithmetic implementation.

Admission is deliberately bounded: each cut is a single unshifted
loop–external scalar product, with diagonal nonzero cut-derivative incidence.
Mixed, affine-shifted, quadratic, zero-self-incidence, or mutually coupled cuts
produce `LinearCutError::Unsupported`. When a cut is requested, noninteger
power offsets anywhere in the family are also explicitly deferred rather than
silently copying the C++ `deltaReplRule` omission. An empty cut mask preserves
ordinary noninteger-offset support, including the supplied `bc4PMRad1` case.
These limits cover the supplied removed-cut PM family definitions; they are
not a claim of arbitrary cut preparation.

`SourceSystem::new_with_fixed` represents the common prepared-coordinate
pattern. Its boundary validates the native variable map, absolute numeric
powers, and absence of fixed-coordinate variables from coefficients. Replacing
prepared rows preserves variables, coefficient priority, and inherited
conditions even if every row vanishes. Instantiation retains fixed powers
instead of adding them to seeds, and rejects reopening or shifting a prepared
coordinate before processing even an empty source row. Sector construction
requires the matching removed-cut mask; both symbolic and shared numerical
searches use the same fixed-source contract.

`IntegralOrder::with_permutation` and `SectorConfig::permutation` implement
the C++ dynamic ordering override. Only the final denominator/numerator
coordinate tie-breaks change; sector lexicographic order, cut priorities,
total degree, and numerator degree do not. Identity normalizes to the static
path. Ordering construction validates the permutation once; the static hot
path has no per-coordinate optional-permutation branch. Six new focused tests
and all five existing index tests passed in a standalone native Rust build,
including 120,000 comparisons against all six three-coordinate permutations
of the compiled C++ comparator. The same stored order is used for sector
source sorting/preconditioning, modular and exact search, and numerical
cutoffs.

The first exact differential fixture uses `D0=k·u`, `D1=k·v`, `D2=-k²`,
`u²=v²=1`, `u·v=γ`. Its pre-rule is
`I(n) = -γ n1/(n0-1) I(n-e0+e1) + 2 n2/(n0-1) I(n-2e0+e2)`.
Added differential tests cover scaled cuts and Gram entries, the complete
improved toy source frame, cancellation-resistant parameter guards, two
independent cuts, unsupported admission, and no-cut/noninteger preservation.
The complete `fam1_11` checkpoint now passes: both pre-rules, all 802 equations
and guards over 40 sectors, and all 16 finite residual keys agree with C++ at
1/2/4/6 workers. All 108 focused solver tests pass. Seven paired release runs
give campaign medians 264.459 ms serial and 66.119 ms with six workers, versus
739 ms and 131 ms for C++. See [the complete PM result](spired_fam1_11_results.md)
for timing boundaries, independent audits, phase profiling, and remaining scope.
This historical fixture checkpoint requires only coordinate guards and does
not validate the subsequently integrated affine search. The latter is covered
by the complete `fam1_12` and `fam1_111` release comparisons described above.
