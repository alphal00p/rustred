# Exact disjunctive exceptional cases: implementation and acceptance plan

Design date: 2026-09-14. The intersection service and sector-queue integration
are implemented, tested and independently audited (2026-09-15). All 20
established release regressions pass. The first integrated full `fam1_112`
attempt reached 435 of 436 observed rule sections before its 600-second cap;
complete-family acceptance remains pending. This follows the rational computational-chart
slice and retains the original integer-coordinate meaning of every case. All
coefficient algebra remains native Symbolica.

## Implementation checkpoint

`Case::intersect_many` returns one outcome containing the exact union and phase
statistics. The cohesive `solver/case/intersection` module reuses existing case
admission and validation, preserves normalized coupled equations, and uses
native factorization to distribute exceptional conjunctions. Unsupported,
overflowing or budget-exhausted siblings reject the entire result; none are
silently dropped. Limits are checked around native operations, not advertised
as interruption or hard-memory limits inside Symbolica.

The standalone tests cover the actual quadratic conjunction below, coupled
native ideal normalization, rational-chart parity, factor overlap, retained
siblings, constants and atomic failures. Ten further regressions test queue
integration and the affine-only fast path. The combined **180-test solver set
passes**, including the captured-input census. A separate agent reviewed the
implementation and mathematics. The queue now admits every exceptional
conjunction through the union-returning service before publishing its parent
rule; existing-rule coverage uses the same exact boundary. The subsequent
release results below provide campaign evidence separately from those tests;
neither unit-test success nor rule-file generation certifies a closing artifact.

The common all-affine lane retains full validation, incoming-domain emptiness
checks and work/term budgets, but directly uses existing case admission without
new primitive-content, restriction-copy, factorization or ideal-normalization
work. Its tests check those phase counters, rational-chart integer parity,
empty incoming affine domains and invalid index maps.

The subsequent captured-input diagnostic now succeeds on **all 13** original
nonlinear conjunctions. Two have empty integer intersections; the remaining
eleven yield 15 coordinate/affine branches in total. None retains unsupported
geometry. Independent provenance review checked all 195 fixed-axis entries,
33 input equations and 264 polynomial terms against the original failure logs,
including the incoming affine parent. Each returned branch is checked to imply
every original equality exactly and to retain the parent and sector signs.
Only two inputs need joint native ideal normalization; the others resolve by
affine restriction, factorization, and exact sign admission. This diagnostic
does not prove that later queue descendants will all succeed. The integrated
full-family run below still stops before every queued case has been processed.
Evidence: `target/spired-cold-artifact.ckZDsX/captured-geometry.txt`.

## Integrated release checkpoint

The reproducible protocol in `target/spired-disjunctive-queue.RLesUd/` executed
20 established fixture/frontier runs at requested worker budgets one and six.
All pass, including strict native equation, required/excluded-domain and
preliminary-rule comparison, available independently extracted native residual
keys, and **939 byte-identical mathematical-file comparisons** against the
previous checkpoint and across worker counts. This includes the complete
38-sector vac3 workload (617 rules), fam1_11 (802), fam1_12 (1,104), fam1_111
(10,333), bc4PMRad1 (856), the supplied fam_cosmo sector (15), K1/K3 regression
fixtures, and the two previously validated fam1_112 frontier sectors (347 and
398). One-sector fixtures clamp the actual worker count to one.

The following full attempt used all **436** original requested fam1_112 sectors,
all **16** ordering overrides, numeric search depth **3**, six workers, and
unbounded symbolic source enumeration under a **600-second process wall cap**.
It exited **124**, not successfully. Observed output contains **435** complete
rule sections, **72,355** rules and **390 provisional residual records**. No
visibly truncated rule section was found, but the residual-section header is
not an end-of-file completion marker: a killed callback's entire residual list
is not certified by that header. No partial C++ whole-family oracle was used.
Binary and input hashes are unchanged.

Only `111010100001111` has no rule file. Its log records 208 case starts,
207 rules found and no numerical-search start. The last visible target is

```text
I(1,1,1,0,2,0,1,0,0,0,0,n11,n12,n13,n14)
```

with 85 additional cases pending. No terminal algebra or geometry error was
reported before timeout. The observer prints the integral's fixed-coordinate
pattern, not every possible coupled equality; it also does not separate
modular search, exact lifting and subsequent guard work within that final
case. Consequently this is not evidence that any particular native primitive
is stuck or that this remaining sector cannot complete.

Compared with the prior 422-sector checkpoint, all 422 have output: 413 are
byte-identical and nine change only by removing 14 rule blocks. Every retained
block and all nine residual tails remain byte-identical; no rule is added or
modified in those nine files. Independent review proves 13 removed affine
domains empty by sector sign bounds. The other removed coordinate rule, in
`111010100011011`, is redundant: a retained broader rule frees `n11` and has
the sole exception `n4+2*n10+2*n11=4`, impossible when all three indices are
positive (the left side is at least five). It therefore covers the removed
child. This establishes no lost application domain from these deletions; it
is not a claim of equality between two independently specialized RHSs. The
original C++ output retains that redundant child, so a strict one-to-one rule
census may conservatively reject this improved cover and needs a separate
semantic comparison, not a weakened closure criterion.

Twelve of the original thirteen nonlinear-frontier sectors now have observed
rule output. The thirteenth is the remaining sector above; an additional
post-chart continuation also acquired output, giving 13 more files than the
prior checkpoint. The full run took **600.163 seconds** launch-to-exit,
**2,779.02 user + 49.62 system CPU seconds**, and peaked at **1,767,052 KiB RSS**.
These are censored diagnostic figures, not completed timing or performance
acceptance. Aggregate per-sector phase files did not flush before the timeout,
so a full-family phase breakdown is unavailable. The completed regressions do
retain those phases. Large timing variation even between two actual
single-worker repeats means an unmatched old/new comparison cannot attribute
slowdown to this queue slice.

The next bounded diagnostic should isolate `111010100001111` with the same
frozen binary, sources, orderings and numeric depth, a longer finite wall cap,
and subphase profiling where available. Do not pretend that a cached native
oracle exists for it: the censored C++ run has no completed statistics record
for this sector. Separately compare changed sectors against complete native
records where available, retaining the distinction between redundant rule
lists and complete exact application coverage. No further run or algorithm
change is authorized by this recorded proposal alone.

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
The standalone implementation test reproduces this exact factorization and
both retained branches using native substitution and factorization. The
integrated full attempt subsequently writes 544 rules for this sector; exact
native comparison of that new output and complete-family acceptance remain
separate obligations.

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

The sector queue admits **every** exceptional conjunction completely before
inserting any new child or storing the parent rule. An unsupported sibling
rejects the operation atomically, even if another sibling has already been
resolved internally. Children keep canonical primitive integer equations;
only their computational charts use rational parameters. Existing
containment-based pruning is sufficient and conservative, not a complete
integer-polyhedron implication algorithm.

`SectorRule::exceptional_cases` flattens `intersect_many` for every original
OR branch, then applies common canonical union pruning. Queue insertion has a
separate private admission step that preserves the original OR-branch order:
globally pruning before insertion could remove a numerical seed that the old
queue would have retained before a later, broader symbolic sibling arrived.
All 24 permutations of a representative coordinate/numerical sibling set are
checked against the original insertion algorithm for pending order, numerical
seed bank and discarded counters. Newly split nonlinear branches naturally
add work, but established coordinate counters and chronology are unchanged.
Each individual factor union has a deterministic canonical order.

Existing-rule coverage
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
