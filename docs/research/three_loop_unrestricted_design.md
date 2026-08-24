# Target-driven and unrestricted three-loop reduction design

## Verdict

RustRed can be extended immediately from its present finite boxes into a
useful **certified target-driven** reducer.  Every finite search attempt can be
preflighted exactly, and every successful answer can carry a replayable proof
from native RustRed IBPs and analytic boundary identities.  A fair sequence of
growing seed shells is complete as a *semidecision procedure*: if a requested
reduction has a finite proof from the enumerated IBPs and the allowed terminal
set, some finite shell contains that proof.

That is not yet an unconditional all-index termination theorem.  An unchanged
rank or terminal list in any finite number of shells does not prove that a
later shell will not add a pivot.  Conversely, failure to find a pivot does not
prove that the integral is a new master.  A genuinely unrestricted reducer
needs a finite collection of guarded parametric recurrences whose guards cover
the whole integer-index domain and whose right-hand sides are proved strictly
lower.  The missing hard cases are:

- scalar dots and numerator powers in the four-line `B4` sector;
- arbitrary numerator powers in the five-line `F5` sector.

The six-line scalar top recurrence and both scalar `F5` dot recurrences are
already suitable parametric rules.  The tree boundary is all-index analytic,
and the finite paw numerator expansion now delegates induced sunsets to the
complete scalar `two_loop_top_dot` reducer. The cached finite two-loop table is
retained only as a compatibility and finite-audit surface. These distinctions
remain visible in the API.

This design was prepared by reading RustRed, LiteRed2, and the checked-in
Vakint/MATAD text.  No FORM or Mathematica program was run, and neither is part
of the proposed implementation or certificate.

## 1. Exact domain decomposition

For an exponent vector `a` define

\[
 D(a)=\sum_i\max(a_i-1,0),\qquad
 N(a)=\sum_i\max(-a_i,0).
\]

The proved `S4` action on the six edges of `K4` leaves six connected sector
orbits.  They have the following reduction responsibilities:

| canonical mask | active lines | treatment for arbitrary integer powers |
|---:|---:|---|
| `7`, `11` | 3 | direct tree factorization, including all inactive numerators |
| `15` | 4 | direct paw tensor/angular factorization, then a two-loop target reducer |
| `43` | 4 | genuine `B4`; two inactive numerator directions |
| `31` | 5 | genuine `F5`; one inactive numerator direction |
| `63` | 6 | genuine top; no inactive numerator direction |

Raw IBPs cannot activate an inactive line.  A derivative term for a zero power
is absent; for a negative power the raising shift moves it toward zero, never
to a positive value in one step.  A row therefore stays in its sector or
enters a proper subsector.  This proves the bottom-up dependency order

```text
factorized boundaries -> B4 -> F5 -> six-line top.
```

It also means that a numerator-decorated “top” input is really an `F5`, `B4`,
or lower-sector input.  Unrestricted numerator closure only needs parametric
rules in the two genuine proper sectors.

The existing components have the following precise status:

- [`three_loop_boundary.rs`](../../src/three_loop_boundary.rs) gives finite
  exact tree and paw numerator algorithms under explicit work limits and uses
  complete scalar-dot reduction for the induced sunset;
- [`three_loop_top_dot.rs`](../../src/three_loop_top_dot.rs) proves strict
  scalar-dot descent in mask `63`;
- [`three_loop_proper_dot.rs`](../../src/three_loop_proper_dot.rs) proves both
  scalar-dot branches in `F5` and deliberately rejects dotted `B4`;
- [`three_loop_pipeline.rs`](../../src/three_loop_pipeline.rs) proves only its
  configured finite target box, although larger boxes may be built;
- [`tensor.rs`](../../src/tensor.rs) and
  [`tensor_family.rs`](../../src/tensor_family.rs) already provide the native
  tensor projection and complete-basis denominator lowering needed before
  scalar reduction.

## 2. What an adaptive finite search can prove

### 2.1 Per-attempt termination

Fix a finite target set, a finite seed set, and finite boundary/tensor resource
limits.  IBP generation emits nine finite rows per seed, and every row contains
only the shifts

```text
a
a + e_r
a + e_r - e_s.
```

The resulting column set and exact sparse matrix are finite.  Exact Gaussian
elimination therefore terminates, subject to an operational memory or
coefficient-work error.  A successful target result is a theorem if the
certificate records and replays:

1. each row's `(seed, differentiated_loop, contraction_loop)` origin;
2. symmetry, zero-sector, and lower-sector normalization of every raw term;
3. the exact row operations producing each reachable pivot;
4. recursive strict descent to an explicit terminal whitelist; and
5. every coefficient denominator introduced by pivot division.

Coverage of unrelated table columns is unnecessary.  The proof obligation is
the dependency closure reachable from the requested targets.

### 2.2 Fair-search theorem and its limitation

For target bounds `(D0,N0)`, the rectangular schedule

```text
(Dk,Nk) = (D0+k, N0+k), k = 0,1,2,...
```

is fair in each fixed sector: every finite integer seed set is contained in
some shell.  If a target-minus-terminal relation is a finite linear
combination of ordinary RustRed IBPs and proved lower-sector identities, a
row-space membership solver will therefore find it at finite `k`.  Inverse
shift parents of an unresolved column may be added first as an optimization,
but a deterministic full-shell fallback is required to preserve fairness.

This theorem is conditional on the target actually belonging to the module
generated by the allowed relations modulo the terminal span.  The search may
run forever for a genuine unlisted master or when a required identity family
is absent.  A configured implementation must instead return a typed
`SearchBudgetExceeded` carrying the last shell and unresolved columns.  It
must not return `NewMaster` or silently retain the unresolved integral.

LiteRed2's `SolvejSector` uses the same broad discovery idea: it searches
growing diamonds, patternizes successful pivots, splits bad coefficient/domain
cases, and records uncovered points.  See the static implementation in
[`LiteRed2026.m`](../../vendor/LiteRed2/Source/LiteRed2026.m) and the audit in
[`litered2_algorithm_report.md`](litered2_algorithm_report.md).  That heuristic
is useful inspiration, not a RustRed all-index proof.  Neither a stable pivot
bitmap nor a stable candidate-master list over finitely many depths is an
induction argument.

## 3. Exact seed and row bounds

The current generic enumerator's pre-filter count is not merely an asymptotic
estimate.  With six physical propagators and all subsectors enabled, the exact
number of labelled exponent vectors visited before scaleless and symmetry
filtering is

\[
 C_{\rm all}(D,N)=
 \sum_{s=0}^{6}{6\choose s}{D+s\choose s}
                         {N+6-s\choose 6-s}.                 \tag{3.1}
\]

Here `s` is the number of active lines.  Saturating arithmetic may turn the
implemented value into a conservative upper bound after overflow, but the
unsaturated combinatorial formula is exact.

A sector-local solver should do better.  Fix one labelled representative with
`s` active and `t=6-s` inactive positions.  Enumerating only that
representative, before quotienting by its stabilizer, visits exactly

\[
 R_{s,t}(D,N)={D+s\choose s}{N+t\choose t}.       \tag{3.2}
\]

The conventions are `{N\choose0}=1` and an empty inactive assignment has
numerator degree zero.  Thus the three genuine sector representatives require
at most

\[
 {D+4\choose4}{N+2\choose2}
 +{D+5\choose5}(N+1)
 +{D+6\choose6}                                  \tag{3.3}
\]

raw seed visits.  Enumerating every labelled mask and filtering afterward
would multiply the three terms by `3`, `6`, and `1`, respectively, because
those are the `B4`, `F5`, and top mask-orbit sizes.

For example, `(D,N)=(1,1)` makes the existing all-sector enumerator visit
exactly `928` candidates.  A fixed-representative genuine-sector enumerator
visits at most `15 + 12 + 7 = 34` before stabilizer canonicalization.  This is
the appropriate unit for a target-driven implementation.

Exact-degree counts are also useful for incremental shells.  For `s>0`,
`t>0`, exact degrees `(d,n)` contain

\[
 {d+s-1\choose s-1}{n+t-1\choose t-1}            \tag{3.4}
\]

assignments.  If `t=0`, the second factor is one for `n=0` and zero otherwise.
Nested rectangular-shell work is exactly the difference of (3.2) at the new
and old bounds.

### 3.1 Exact symmetry-unique count

An even tighter count can be computed before allocating rows.  Let `H` be the
stabilizer of the fixed sector mask: `|H|=8` for `B4`, `4` for `F5`, and `24`
for the top.  For `h in H`, let `L_A(h)` and `L_I(h)` be the cycle lengths of
its action on active and inactive positions.  Define

\[
 A_h(D)=\sum_{q=0}^{D}[x^q]\prod_{\ell\in L_A(h)}(1-x^\ell)^{-1},
\]

and define `I_h(N)` analogously on inactive cycles.  Burnside's lemma gives
the exact number of symmetry-unique seeds:

\[
 U_S(D,N)=\frac1{|H|}\sum_{h\in H}A_h(D)I_h(N).   \tag{3.5}
\]

Both truncated generating functions are small integer dynamic programs; no
symbolic algebra is needed.  A sector enumerator can assert that its output
has exactly (3.5) elements.  The exact native row count is then `9*U_S`.

### 3.2 Exact row-work preflight and halo

For a particular row `(a,i,j)`, coefficient-free family support gives the
exact number of attempted raw terms before collection:

\[
 [i=j]+\sum_{r:a_r\ne0}
 \left([c_{rij}\ne0]+|\operatorname{supp}v_{rij}|\right),   \tag{3.6}
\]

where `c` and `v` are the constant and denominator support returned by the
precomputed derivative contraction.  Summing (3.6) over nine rows and the
canonical seeds gives a true work reservation, unlike checking a vector length
after construction.

One row from a `(D,N)` seed has columns inside `(D+1,N+1)`, and both increases
can occur in the same term.  This is an exact one-row halo bound, not a seed
closure theorem.  Exponent additions and every binomial count must use checked
`u128`/`i64` arithmetic before converting to `usize`, `u64`, or `i32`.

## 4. Implementation-ready target algorithm

### 4.1 Public dispatch

For each scalar target:

1. validate six-index arity and checked aggregate degrees;
2. apply scaleless detection and sector-first `S4` canonicalization;
3. dispatch masks `7`, `11`, and `15` to the analytic boundary service;
4. for a numerator-free top target, apply the proved top-dot recurrence
   recursively until only proper sectors or `M6` remain;
5. for a numerator-free `F5` target, apply its proved central/outer recurrence
   recursively until only lower sectors or the `F5` corner remain;
6. send every remaining genuine target to the bottom-up certified sector
   solver; and
7. memoize normal forms by canonical integral and certificate version.

The recurrence fast paths are optional optimizations from the viewpoint of
row-space completeness, but they make high scalar dot degree cheap and leave
the finite solver focused on `B4` and numerator-coupled proper sectors.

### 4.2 Sector solve

For one sector, maintain separate sets of protected terminal columns and
unwanted same-sector columns.  Before inserting a row:

- canonicalize and discard proved zero terms;
- reduce every proper-subsector term through an already certified lower
  service;
- collect exact coefficients and intern the remaining columns;
- attach a stable raw-row origin and a boundary-normalization proof node.

Eliminate unwanted columns hardest first, but never pivot a protected
terminal.  Determine success by exact row-space membership of

\[
 e_{\rm target}-\sum_{m\in\text{terminals}}c_m e_m,
\]

not by the accidental absence of a key from a triangular `HashMap`.  Retain
only the pivot/rule dependency DAG reachable from the requested target, while
keeping hashes of every input row used by those nodes.

When unresolved columns remain, first enqueue valid inverse-shift parents

```text
x
x - e_r
x - e_r + e_s
```

which can emit `x`; enforce the sector inequalities before canonicalization.
If that frontier adds nothing, advance the fair total-degree shell.  The
configured candidate, row, nonzero, coefficient-degree, and wall-work budgets
are checked before extending the matrix.

### 4.3 Certificate and cache

A successful cache record should contain at least:

```rust
pub struct ThreeLoopTargetCertificate {
    pub family_fingerprint: String,
    pub order_fingerprint: String,
    pub boundary_version: String,
    pub targets: Vec<Integral>,
    pub sector_shells: Vec<SectorShell>,
    pub raw_row_hashes: Vec<RowHash>,
    pub derivation: DerivationDag,
    pub terminal_whitelist: Vec<Integral>,
    pub excluded_parameter_factors: Vec<Coefficient>,
}
```

`DerivationDag` needs nodes for an authenticated raw row, exact scaling and
addition, symmetry canonicalization, zero, and a versioned boundary result.
A loaded table is certified only after replaying this DAG or deterministically
rebuilding and comparing it.  Triangularity, coverage, and a payload checksum
alone do not prove algebraic provenance.

The single-scale grading should eventually be separated as

```rust
struct MassGradedCoefficient {
    mass_power: i64,
    rational_in_d: SymbolicaRationalPolynomial,
}
```

because homogeneity fixes the `m2` power of a coefficient relating two
integrals.  Keeping that possibly large signed power outside Symbolica avoids
turning Symbolica's polynomial-exponent representation into a false
all-index limit while retaining Symbolica for the nontrivial rational function
of `d`.

## 5. Dotted B4: required treatment

The current `UnsupportedDottedB4` error is correct for the specialized
**one-seed scalar** API, but it should not become the final target reducer's
answer.

The first dot has a direct proof independent of a Laporta search.  With

\[
 B_4=I(1,1,0,1,0,1),
\]

dimensional homogeneity gives

\[
 m^2\frac{dB_4}{dm^2}=\left(\frac{3d}{2}-4\right)B_4.
\]

Differentiating the four equal-mass denominators and using line transitivity
therefore proves

\[
 I(2,1,0,1,0,1)=\frac{8-3d}{8m^2}B_4.           \tag{5.1}
\]

This rule can be added immediately with a homogeneity certificate and checked
against the native finite pipeline.

The four active `B4` lines are the vertices of a square under the actual
sector stabilizer `D8`; they are not acted on by a full `S4`.  At total dot
degree two there are already three scalar orbits: `B(3,1,1,1)`, an adjacent
double-dot `B(2,2,1,1)`, and an opposite double-dot `B(2,1,1,2)` in cyclic
compact order.  The exact scalar-orbit generating function is

\[
 \frac18\left(
 \frac1{(1-x)^4}+\frac2{1-x^4}+\frac3{(1-x^2)^2}
 +\frac2{(1-x)^2(1-x^2)}\right),
\]

whose degree-two coefficient is three.  Thus the complete seed census through
`D=2` is `1+1+3=5` orbits.  The scalar transfer relation discussed in
[`three_loop_b4_scalar_recurrence.md`](three_loop_b4_scalar_recurrence.md)
does not by itself close these classes.  More generally, the number of scalar
orbits is the coefficient of the displayed `D8` cycle-index series, not the
partition count `p4(D)`.  The previously proved rank-nine one-seed no-go is
consistent with this: forbidding inactive numerators and same-degree transfers
removes every nonzero pivot combination.

Consequently the implementation path is:

1. add the direct degree-one rule (5.1);
2. use the implemented [`three_loop_b4_d2.rs`](../../src/three_loop_b4_d2.rs)
   replayable degree-two `B4` shell containing all three scalar target orbits
   and the complete one-step numerator halo on positions `2,4`;
3. generalize the target-driven shell to arbitrary configured `(D,N)` and
   return a budget error, never a fabricated master, when it does not close;
4. use stable finite-shell pivot skeletons only to *discover* coupled
   scalar/numerator parametric rules; and
5. replay each proposed symbolic rule exactly as a combination of generic
   native IBPs before accepting it.

Static MATAD routines show that successful `B4` reduction couples scalar dots
to numerator relations.  They may guide seed stencils, but their FORM rules,
term ordering, tables, and implicit special-case logic are not certificates
and must not be executed or copied as trusted output.

## 6. From finite discovery to all-index rules

### 6.1 Rule representation

Add a parametric layer separate from concrete `Integral` tables:

```rust
pub struct ParametricIbpRule {
    pub sector: ThreeLoopSector,
    pub orientation: IndexOrientation,
    pub guard: IntegerGuard,
    pub pivot_nonzero: Vec<PolynomialCondition>,
    pub lhs: IntegralPattern,
    pub rhs: Vec<(IntegralPattern, ParametricCoefficient)>,
    pub source_syzygy: Vec<WeightedGenericRow>,
    pub ranking: RankingProof,
}
```

Runtime matching should use integer exponent fields and proved sector
stabilizers; arbitrary expression matching is unnecessary on this hot path.
Symbolica should represent and simplify coefficients in
`Q(d,m2,a1,...,a6)` and verify the generic row identity.  Discovery may use
modular samples and interpolation, but the accepted rule is the exact symbolic
identity, not the reconstruction evidence.

### 6.2 Discovery and proof loop

For `B4` first, then numerator `F5`:

1. solve several exact finite shells and retain source-row weights, not only
   final coefficients;
2. identify a finite shifted-seed stencil and stable pivot pattern;
3. reconstruct its weights as rational functions of symbolic indices and `d`;
4. generate the corresponding generic native IBPs in Rust and prove their
   weighted sum equals the proposed recurrence identically;
5. factor the pivot and split index cases where it is identically zero;
6. prove every guarded RHS pattern is lower under one explicit well-founded
   sector-local ranking;
7. use exact integer-domain checks to prove that symmetry orientations, rule
   guards, and named corner points cover every active-positive/inactive-
   nonpositive exponent vector; and
8. add exceptional cases as separate rules rather than silently cancelling a
   vanishing pivot.

The ranking need not equal the legacy finite-table order, but it must be
versioned and proved.  A useful search space is a lexicographic tuple built
from active-line count, numerator degree, dot degree, and stabilizer-orbit
coordinates.  Which of numerator or dot degree comes first must be determined
by the actual coupled recurrence; it cannot be assumed before inspecting its
outputs.

Finite shells validate instances of a parametric rule.  Only steps 4, 6, and 7
prove algebra, termination, and whole-domain coverage.  Once those checks pass
for the `B4`, `F5`, and top domains, induction on the ranking proves an upper
bound to the five displayed terminal integrals.  Proving that the five are a
minimal independent master basis remains a separate quotient-rank or
critical-point calculation; minimality is not required for terminating
reduction to a fixed (possibly redundant) basis.

### 6.3 Lower-loop prerequisite

The paw boundary can turn arbitrary three-loop powers into arbitrarily dotted
two-loop sunset integrals.  This prerequisite is now implemented by
[`TwoLoopTopDotReducer`](../../src/two_loop_top_dot.rs): the paw's finite
polynomial/angular expansion delegates every induced positive sunset to a
proved descending all-dot recurrence and every pinch to the exact two-line
formula.  The retained [`TwoLoopReductionPipeline`](../../src/two_loop_pipeline.rs)
and `max_two_loop_dots` field remain compatibility/finite-audit surfaces and
do not cap actual paw dispatch. Operational state, coefficient, exponent, and
boundary-work limits still return typed failures, so this removes the fixed
dot-domain gap without claiming unbounded machine resources.

## 7. Tensor inputs

The tensor path remains entirely native:

```text
contract explicit metrics
-> global O(d) vacuum projection
-> scalar-product monomials
-> complete six-denominator affine expansion
-> signed-power scalar targets
-> target-driven three-loop reduction.
```

Odd residual tensor rank is zero.  Every fixed even rank has finitely many
perfect matchings and every scalar-product monomial has a finite affine
denominator expansion, so each configured tensor request terminates or returns
a typed pairing/expansion resource error.  For rank `r`, projection introduces
`r/2` scalar products in each source pairing in addition to scalar products
already present after metric contraction.  Denominator lowering only subtracts
from base powers; for total scalar-product degree `q`, a safe degree bound is

```text
D(lowered) <= D(base)
N(lowered) <= N(base) + q.
```

The reducer should nevertheless scan the actual finite lowered combination
and choose exact per-sector target bounds from it.  This is tighter and avoids
building a rectangular box for cancelled polynomial terms.

The base integral and tensor numerator must be transformed by the same routing
witness before projection/lowering.  Once all scalar products have been
lowered to the complete denominator basis, ordinary signed-exponent `S4`
canonicalization is sufficient.  No FORM tensor stage is needed.

## 8. Proposed public API

The finite and parametric guarantees should be distinguishable rather than
hidden behind one boolean:

```rust
pub enum ThreeLoopCoverageMode {
    /// Fair finite-shell search up to explicit operational budgets.
    CertifiedTarget,
    /// Reject any point not covered by proved guarded recurrences.
    RequireParametric,
}

pub struct ThreeLoopTargetConfig {
    pub mode: ThreeLoopCoverageMode,
    pub max_shell_radius: u32,
    pub max_seed_candidates: u64,
    pub max_rows: u64,
    pub max_nonzeros: u64,
    pub max_tensor_pairings: usize,
    pub max_tensor_expansion_terms: usize,
    pub boundary: ThreeLoopBoundaryConfig,
}

pub struct ThreeLoopTargetReducer {
    // authenticated family, analytic boundaries, proved recurrences,
    // sector-local incremental matrices, and normal-form/certificate caches
}

impl ThreeLoopTargetReducer {
    pub fn reduce_integral(
        &mut self,
        target: &Integral,
    ) -> Result<CertifiedReduction, ThreeLoopTargetError>;

    pub fn reduce_combination(
        &mut self,
        input: &LinearCombination,
    ) -> Result<CertifiedReduction, ThreeLoopTargetError>;

    pub fn reduce_tensor(
        &mut self,
        base: &Integral,
        numerator: &TensorMonomial,
    ) -> Result<CertifiedTensorReduction, ThreeLoopTargetError>;
}
```

Important errors include `WrongArity`, `ExponentOverflow`,
`BoundaryResourceLimit`, `TensorResourceLimit`,
`SearchBudgetExceeded { sector, shell, unresolved }`,
`MissingParametricCoverage { sector, integral }`, `NonDescendingRule`,
`CertificateReplayMismatch`, and `ExceptionalSpecialization`.  There should be
no error variant whose wording promotes a finite-search remainder to a master.

`CertifiedReduction` should expose its guarantee explicitly:

```rust
pub enum ReductionGuarantee {
    FiniteTarget { certificate_hash: [u8; 32] },
    ParametricDomain { rule_set_hash: [u8; 32] },
}
```

## 9. Acceptance sequence

1. Separate target bounds from seed-shell bounds in the current pipeline.
2. Add fixed-representative/stabilizer seed enumeration with the exact checks
   (3.2)-(3.6).
3. Implement bottom-up target row-space solving and a replayable derivation
   DAG; compare it with the existing finite pipeline on `(1,1)` and scalar
   `(3,0)`.
4. Add the independently proved one-dot `B4` fast rule; the complete
   replayable degree-two `B4` shell with all three `D=2` scalar orbits and its
   numerator columns is now present in `three_loop_b4_d2`.
5. Compose the proved top/`F5` scalar recurrences with the adaptive `B4` and
   proper-numerator solver.
6. The two-loop all-dot prerequisite for arbitrary paw powers is now complete
   and integrated through `ThreeLoopBoundaryReducer`.
7. Discover, reconstruct, and exactly prove coupled `B4` and `F5` numerator
   rule sets; mechanically prove guard coverage and ranking descent.
8. Run exhaustive labelled finite boxes as instances, replay every rule from
   native rows, and validate at fresh modular specializations away from the
   recorded pole set.
9. Only after step 7 advertise unrestricted three-loop reduction.  Until then,
   advertise certified target reductions with explicit search budgets.

This path extracts maximum value from the trustworthy finite pipeline now,
while keeping the line between successful finite certificates and a genuine
all-index proof exact.
