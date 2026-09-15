# Exact disjunctive exceptional cases: next implementation slice

Design date: 2026-09-14. The standalone intersection service is now implemented
and independently audited (2026-09-15); queue integration and complete
`fam1_112` acceptance remain pending. It follows the rational computational-chart
slice and retains the original integer-coordinate meaning of every case. All
coefficient algebra remains native Symbolica.

## Standalone implementation checkpoint

`Case::intersect_many` returns one outcome containing the exact union and phase
statistics. The cohesive `solver/case/intersection` module reuses existing case
admission and validation, preserves normalized coupled equations, and uses
native factorization to distribute exceptional conjunctions. Unsupported,
overflowing or budget-exhausted siblings reject the entire result; none are
silently dropped. Limits are checked around native operations, not advertised
as interruption or hard-memory limits inside Symbolica.

Thirteen focused tests pass, including the actual quadratic conjunction below,
coupled native ideal normalization, rational-chart parity, factor overlap,
retained siblings, constants and atomic failures. The full 169-test solver set
and workspace all-target check pass. A separate agent reviewed implementation
and mathematics. The solver queue still uses its existing singleton service;
this checkpoint does not change completed campaign output or close any of the
14 remaining sectors. Direct validation of all 13 captured nonlinear failure
conjunctions and subsequent queue integration are the next steps.

## Evidence and immediate objective

The frozen pre-rational-chart Rust executable completed 420 of 436 requested
sectors, then returned an error. Separate licensed reruns of all 16 missing
sectors found three fractional-chart admission errors and 13 nonlinear errors.
Among the latter, nine final conjunctions have two equations, one has three,
and three have four. Every reported nonlinear conjunction contains at least
one affine equation. A reported residual conjunction is not necessarily the
entire domain: an incoming affine case may already contain additional required
equalities, which must also be preserved.

The coefficient variable map is `[d,x,n0,...,n14]`, not an index-only map.
For example, the first failed sector `111000001010111` has
`2*n10-n13=4` in zero-based integral coordinates, or `2*n11-n14=4` in C++
one-based notation. Raw polynomial exponent positions must not be printed as
integral-coordinate numbers. Rational computational charts address this
admission issue; they do not themselves split the nonlinear conjunctions.

The original full C++ `fam1_112` run was censored after 600 seconds, with
288 sector files produced. Neither this partial C++ run nor the failed Rust
run establishes full-manifest timing or reference parity. Keep every partial
result and failed/censored timing separate from completed acceptance results.

Ignored evidence:

- `target/spired-fam112-first.8b69aX`: initial Rust campaign.
- `target/spired-fam112-isolated-licensed.5e73H1/errors.txt`: decoded fixed
  faces and exact final error equations for the 16 isolated reruns.
- The neighboring per-sector stderr logs retain original incoming affine
  cases, variable maps, and full diagnostics.

## Why returning one case is insufficient

The existing cold ideal normalizer computes a complete native reduced basis,
but its coordinate-only retry discards that transformed basis when the result
is still coupled. The affine caller then sees the original nonlinear equations.
For example,

```text
(a-b)*c = 0  AND  (a-b)*(c-1) = 0
```

has the single affine solution domain `a=b`. The normalized coupled equation
must reach affine admission; retaining it only in a discarded coordinate retry
cannot solve this case.

Other equations genuinely require a union:

```text
(a-b)*(c-1) = 0  AND  d=2
```

is equivalent to

```text
(a=b AND d=2)  OR  (c=1 AND d=2).
```

Choosing only one factor loses solutions. Requiring both factors changes OR
into AND. Both are incorrect. Repeated factors need no extra branches:
`(a-b)^2=0` has the same zero set as `a-b=0`.

### Concrete `fam1_112` regression target

Sector `111100001010111` reports a quadratic equation together with `n12=1`.
With `t=n14`, `u=n10`, and `v=n8`, restriction of that quadratic should expose

```text
(t-1)*(1-t+2*u-2*v) = 0.
```

The corresponding branches retain the entire incoming face and require
`n12=1` together with either `n14=1` or `n14=1+2*n10-2*n8`. They may overlap;
they must not be replaced by their intersection or an invented larger face.
The standalone implementation test now reproduces this exact factorization
and both retained branches using native substitution and factorization. This
validates the captured conjunction, not yet an end-to-end run of its sector.

## Minimal interface and ownership

The implemented union-returning operation is:

```rust
Case::intersect_many(conjunction, index_variables, sector, limits)
    -> Result<CaseIntersectionResult<N>, CaseIntersectionError<N>>
```

The input conjunction is AND; the outcome's `cases` vector is OR, alongside
phase counters/timings in `stats`. An empty vector
means that every branch was proved empty. An empty input conjunction returns
the parent. Successful output is deterministically sorted, deduplicated, and
pruned only by proved containment. It need not be a disjoint partition.

A typed error retains the original parent and conjunction, the unresolved
branch, and whether admission, native algebra, compact indices, or a budget
prevented completion. It must not turn successfully resolved siblings into a
partial success that callers could mistake for the complete intersection.
Diagnostic partial results may be reported separately, never as coverage.

Keep the ordinary coordinate/affine single-case path fast. A singleton helper
may return a single case only when the complete result has cardinality at most
one; a genuinely disjunctive result must produce a typed error there. Do not
silently select the first element. Avoid parallel geometry engines: factor
expansion, queue traversal, and oracle comparison should share this boundary.

## Native reduction workflow

Each work item owns a current exact case and the remaining AND equations;
the immutable original conjunction remains available for diagnostics.

1. Validate index maps and index-only equation admission. Restrict equations
   by the current chart using the **zero-locus** API. Remove zero equations;
   reject the branch if a nonzero constant or exact integer contradiction is
   exposed. Keep all sector signs and required parent equalities.
2. Collect all available affine equations before rejecting any nonlinear
   equation. Intersect them with the case, retain the canonical resulting
   chart, and restrict the remaining equations again. Repeat when additional
   fixed coordinates or independent affine equations appear.
3. If equations remain, normalize their **joint conjunction** with native
   rational `GroebnerBasis::new`. Return the full normalized basis to the
   admission layer, including coupled linear equations. Never combine
   unrelated OR branches in one ideal. Never discard a useful transformed
   basis solely because the coordinate fast path cannot admit it.
4. Feed newly exposed affine equations back through step 2. Factor remaining
   nonconstant equations using native `Factorize::factor()`. Ignore nonzero
   constant factors and multiplicity for zero-locus purposes. Factor one
   equation at a time: every branch carries one distinct factor **and all
   sibling equations**, plus the entire current parent case.
5. Revisit each branch after restriction: a previously irreducible polynomial
   can become reducible or affine on its new face. Emit a case only when every
   remaining equation has been absorbed or proved zero. Otherwise return an
   explicit unsupported result when the admitted exact operations make no
   further progress.

The initial implementation should follow this bounded normalization/factoring
workflow before adding strategy complexity. Native factorization immediately
after a new chart can later be measured as an alternative ordering of the
same exact operations. No fixture name may choose an algorithmic branch.

Zero equations and coefficient values remain separate services. A nonzero
rational scalar can be removed from an equation interpreted as zero. IBP
coefficients must preserve that scalar, and a coefficient numerator and
denominator must be converted jointly. No rule coefficient should be passed
through equation primitive normalization.

## Native API audit

The pinned public declaration, implementation, and existing call sites supply
the required operations:

- `vendor/symbolica/src/poly/factor.rs:2737` declares `Factorize`; `factor()`
  returns factors over the coefficient ring and their multiplicities.
  `square_free_factorization()` alone does not split distinct irreducible
  factors and is insufficient for branch enumeration.
- The integer-polynomial implementation at `factor.rs:4640`, especially
  `factor():4689`, delegates to native square-free, quadratic, univariate,
  bivariate, and multivariate factorization paths. Zero polynomials return no
  factors and must be handled before interpreting that vector as alternatives.
  Nonzero constant content can appear explicitly in the output.
- The rational-polynomial implementation at `factor.rs:4926` and `:4957`
  removes native rational content, delegates to integer factorization, and
  restores the scalar. No local factorization kernel is needed.
- RustRed already uses `factor()` in `solver/exception.rs` for denominator
  exceptions. Native `poly/groebner.rs:1704` and `:1784` also factor elimination
  and specialized polynomials. Native factor tests cover content, common
  monomials, and repeated factors, including `factor.rs:13926`.
- Native `MultivariatePolynomial::replace_with_poly`, `make_primitive`,
  `map_coeff`, and `GroebnerBasis::new` provide chart restriction and exact
  joint normalization. Rational computational charts use the audited
  `FromNumeratorAndDenominator<Q,IntegerRing,...>` conversion when an exact
  coefficient value, rather than a zero equation, is required.
- `MultivariatePolynomial::reduce`, implemented publicly in
  `poly/groebner.rs:1035`, supplies exact normal-form/ideal-membership checks
  against a reduced basis; no local polynomial reduction is needed.

The reference C++ flow performs linear reduction, joint normalization,
factorization, and recursive branch simplification. Its final nonlinear
`linearize` operation searches a bounded integer box, controlled by
`MAX_NLIN_SEARCH`. That last heuristic must not be copied as an exhaustive
integer zero-locus proof. Native factorization may use internal modular
algorithms; using those audited native APIs is not implementing a rational
reconstruction framework in RustRed.

If irreducible finite guards remain an actual blocker, the public
`GroebnerBasis<Q,...,LexOrder>::solve()` at `poly/groebner.rs:1650` is a
native-first future option for zero-dimensional systems. It returns exact
algebraic solutions, not automatically integer leaves. Integer admission,
sector signs, unused variable maps, and compact-index limits would require a
separate audit. Do not implement that additional lane merely because the API
exists, and do not approximate its result by floating-point roots.

## Queue, guards, and publication

The sector queue must enqueue **every** admitted exceptional child before
storing the parent rule. Children keep canonical primitive integer equations;
only their computational charts use rational parameters. Existing
containment-based pruning is sufficient and conservative, not a complete
integer-polyhedron implication algorithm.

`SectorRule::exceptional_cases` must flatten `intersect_many` for every original
OR branch, then apply common canonical union pruning. Existing-rule coverage
is established only if every exceptional intersection is proved empty. An
unsupported intersection means coverage has not been proved; it cannot
suppress pending work. A child equal to its parent still triggers explicit
nonprogress handling.

The oracle helper must compare complete exceptional unions through this same
service. Required affine reference cases and RHS coefficients remain matched
exactly; coefficients are restricted with the exact-value API. A missing
factor branch, incorrect AND/OR grouping, or dropped sibling condition must
fail comparison. Partial original C++ output is useful for individual-sector
checks but cannot establish complete-manifest acceptance.

This geometry service creates no reduction equation, source witness, terminal
authority, or closing artifact by itself. Existing exact source replay,
coefficient guards, strict descent, and eventual artifact publication remain
separate requirements. Only genuinely fully fixed integer leaves enter the
bounded numerical search; a rational chart does not make its free coordinates
numerical or establish master independence.

## Progress, cost control, and required tests

Use deterministic work ordering and canonical equation keys. Do not rerun an
unchanged reduced ideal indefinitely. Affine rank growth, removal of equations,
and proper factor replacement give local progress witnesses; repeated states
or exhausted explicit node/term/normalization budgets return typed incomplete
results. In the original fixed polynomial ring, adding a factor not already
in the current ideal strictly enlarges that ideal. The Noetherian ascending
chain property supports this refinement strategy, but gives no practical
runtime bound and does not justify claiming that every input reaches an
admitted affine result. Native ideal membership can distinguish genuine from
redundant refinement. Never turn a repeated state or budget exhaustion into
an empty set. Branch products should be explored incrementally rather than
materializing their entire Cartesian product first.

Keep charts, original sources, and parent provenance shared. Cache native
normalization/factorization only within the relevant immutable case context.
Any counters or timings must distinguish chart restriction, joint ideal
normalization, factorization, branch admission, and queue work; profiling must
precede more elaborate scheduling or caching.

Required regression cases include:

- product-to-union, AND distribution, overlapping branches, repeated factors,
  nonzero constants, the zero polynomial, and the empty conjunction;
- all affine information collected irrespective of equation order;
- a normalized joint ideal exposing a coupled affine equation;
- re-factorization after chart restriction and after child intersection;
- fractional computational charts with retained parity restrictions, exact
  empty fixed corners, and untouched physical integral axes;
- scalar preservation for coefficient values versus permitted scalar removal
  for zero equations;
- unknown irreducible geometry, compact overflow, and exhausted budgets
  rejecting the whole success result rather than dropping a sibling;
- deterministic canonical unions and unchanged coordinate/integral fast-path
  outputs across worker counts;
- all 13 actual nonlinear failure conjunctions with their complete incoming
  cases and correct index maps, followed by fresh complete-family attempts.

After focused and independent mathematical tests, rerun the established
reference regressions, then all 16 formerly failed sectors and the full
`fam1_112` workload in release mode. Preserve failed and censored observations.
Only a completed requested manifest, exact reference checks where available,
and separately verified residual keys can support a new acceptance milestone.
