# TIDE five-loop banana: exact nonlinear exception

Date: 2026-09-20. This diagnoses one selected-sector search, not a closing
five-loop artifact. The broader input census and run status are in
[the TIDE census](tide_five_loop_census.md); this note records the mathematical
obstruction and the exact-solver authority boundary.

## Captured domain

The selected sector is `111000000001110` in the 15-slot common family. The
source-weight and sparse exact backends both stop at case 195 with the same
original conjunction, unresolved affine parent, and unresolved polynomial.
The source-weight trace ends at exceptional geometry, after exact target-row
validation and guard extraction; no completed sector artifact is produced. Its
native reconstructed weights pass the full exact source-product check, without
characteristic-zero elimination replay. Captured evidence
is in `TMP/source-weight-sector-comparison.Xz5haO/tide-banana-{weights,sparse}-1.*`.

The input has one coefficient parameter, `d`. The source system places its
index variables at positions `offset+i`, with `offset=1`
([`source.rs`](../../crates/rustred-core/src/solver/source.rs)); hence the
error's 16 polynomial coordinates are `(d,n0,...,n14)`. In zero-based integral
indices, case 195 fixes `n0=n1=n2=n11=n12=n13=1` and
`n5=n6=n9=n14=0`. Its five free inactive-sector indices
`n3,n4,n7,n8,n10` are all nonpositive. In particular polynomial exponent
position 11 is `n10`, not `n11`.

The admitted affine equations are

```text
-2 - 2*n10 - 3*n8 - n7 + 3*n3 = 0,
 3 + 3*n10 - n8 - 10*n7 + 8*n4 = 0.
```

Set `u=1+n10`. Their rational chart and the residual equation are

```text
n3 = ( 2*u + 3*n8 + n7)/3,
n4 = (-3*u + n8 + 10*n7)/8,

Q = 164*n7^2 - 308*n7*n8 + 189*n8^2
    +124*n7*u - 94*n8*u - 75*u^2 = 0.
```

For every integer `t>=1`, the **whole original integral-index vector**

```text
(1,1,1,-2t,-t,0,0,-t,-t,0,-t-1,1,1,1,0)
```

obeys the affine equations, Q, and all sector signs. Substitution gives
`n7=n8=u=-t`; the six grouped Q coefficients sum to zero. Its total
numerator power is `6t+1`. A second non-collinear integer ray has
`(n3,n4,n7,n8,n10)=(-36t,-10t,-11t,-21t,-17t-1)`.

The Gram matrix of Q in `(n7,n8,u)` is

```text
[[164,-154, 62],
 [-154,189,-47],
 [ 62,-47,-75]],
```

with determinant `-737280`. Thus Q is nondegenerate and irreducible over the
rationals: a product of two linear forms would have rank at most two. At the
first ray's `t=1` point, the gradient is `(-144,24,120)` and all five free
indices are strictly inside the permitted nonpositive orthant. Rational-line
parametrization from this smooth rational point yields infinitely many nearby
rational directions; scaling clears denominators of both those directions and
the affine chart. The sector therefore contains a genuinely nonlinear
infinite integer family, not merely the two displayed rays. This argument
does **not** classify every integer point of Q, but it excludes sound
replacement by finitely many affine cases or numerical masters.

## Native Symbolica boundary

RustRed's intersection service first restricts to the affine chart,
admits affine equations, and uses native Symbolica Gröbner normalization and
factorization. The narrow definite-quadratic emptiness test described below
cannot discard the **indefinite** original-order Q in this note, so that branch
still returns `UnsupportedGeometry`
([`engine.rs`](../../crates/rustred-core/src/solver/case/intersection/engine.rs),
[`native.rs`](../../crates/rustred-core/src/solver/case/intersection/native.rs)).
That error is correct: an unknown nonlinear set cannot be silently discarded.
The parent rule is not published before all exceptional branches are admitted
([`sector.rs`](../../crates/rustred-core/src/solver/sector.rs)).

The **pinned** dependency is `vendor/symbolica`, not the older
`FOR_REFERENCE_ONLY_DO_NOT_PUSH/symbolica` tree. Its public API includes
`GroebnerBasis::new`, polynomial `reduce`, `solve`, and `solve_parametric`
(`vendor/symbolica/src/poly/groebner.rs:771,1073,1694,2269`), native
`Factorize::{factor,is_irreducible}` (`.../poly/factor.rs:2737-2745`), and
`Atom::solve(...).over(Integers).wrt(...)`
(`.../atom/core.rs:843-848`, `.../solve/solution_set.rs:402-428`).
`SolutionSet` exposes coverage, coverage guards, branch conditions, and
fallible `is_empty` and `dimension` (`.../solution_set.rs:289-372`). The
[four-loop FG diagnosis](four_loop_fg_exceptional_geometry.md) shows why
`coverage=Complete` may still have conditional radical/integer-membership
branches and an unresolved emptiness verdict. Such output is an exact
conditional representation, not automatically an integer chart that RustRed
can search or a certificate of parametric closure.

A direct-linked native probe for this Q and the **exact** sector signs was
prepared at `TMP/source-weight-sector-comparison.Xz5haO/native-geometry-solve.rs`:
it substitutes `n_i=1-p_i` for the five free indices and tags every `p_i`
positive integer. Two short offline direct-link attempts timed out before
producing an executable. One subsequent optimized direct-link attempt against
the *existing* Symbolica rlib was pinned to CPU91 and capped at 180 seconds
and 2 GiB, with no RustRed/core/dependency rebuild. It too reached the time
cap without a compiler diagnostic (`exit 124`, 181.73 seconds wall including
timeout handling, 176.94 seconds user CPU, 1,403,340 KiB peak RSS); the output
file remained zero bytes. Its measurement is retained at
`TMP/source-weight-sector-comparison.Xz5haO/native-geometry-compile.log`.
Consequently that attempt could not run the planned separate 30-second native
solve. This was a direct-link compilation bottleneck, not evidence that the
native integer solver rejects or cannot describe Q.

The subsequent lower-optimization **diagnostic adapter**, linked to the same
frozen release Symbolica library, compiled successfully in 257.50 seconds
(1,461,768 KiB peak RSS). Its bounded run then completed: 0.02 seconds process
wall, 6,144 KiB peak RSS. The instrumented 12 milliseconds starts before
parsing, substitution and exact witness checking; it is not isolated solver
latency or an IBP-generation benchmark. Evidence and independent API/domain
audit: `TMP/native-conic-solution-set.UWT1fj/`.

The actual native result is `coverage=Complete`, an empty coverage guard,
and **two radical-coordinate branches with two free variables**. It retains
explicit integer-membership and positivity conditions on the radical
coordinates, plus positivity of the free variables. Consequently
`is_empty()` returns an unresolved-conditions error and `dimension()` returns
`None`. This is a complete **conditional representation**, not a discharged
integer parametrization that RustRed's affine case engine can directly seed.
The known integer witness passes the native pre-solve checks and lies on
the returned plus branch. All conditions must survive any future adapter;
dropping them because the coverage enum says complete would be unsound.
This observed API capability should be reused for a future nonlinear-case
service instead of implementing another symbolic equation solver.

## Q-first ordering: a distinct, provably empty exception

The separate Q-first coordinate ordering reaches case 193 with 16-variable
coefficient map `(d,n0,...,n14)` and parent equality
`3-n10-n7+2*n3=0`. Its original second guard is

```text
P = 9-3*n10-n10^2-4*n7+n7*n10+14*n3
    -2*n3*n10-4*n3*n7+5*n3^2.
```

Putting `n3=(n10+n7-3)/2` gives, by exact expansion,

```text
4*P|parent = -Q,
Q = 3*n10^2-2*n10*n7+3*n7^2+2*n10-6*n7+3
  = 2*n10^2 + 2*(n7-1)^2 + (n10-n7+1)^2.
```

Here `n7` is inactive, so `n7<=0` and `Q>=2` on the entire declared
integer sector. This case is **empty**, unlike the original-order conic above.
In native exact-matrix terms, the Hessian on `(n10,n7)` is
`[[6,-2],[-2,6]]`, whose leading principal minors are `6` and `32`.
Its unique stationary point is `(0,1)`, with minimum zero; `n7=1` violates
the sector. The captured Q-first error and source ordering are in
`TMP/source-weight-lru-comparison.xSY9i1/tide-banana-q-first-sparse.stderr`.

The generic cold intersection refinement in
[`definite_quadratic.rs`](../../crates/rustred-core/src/solver/case/intersection/definite_quadratic.rs)
checks only parameter-free degree-two index polynomials. It uses native
Symbolica derivatives, exact rational matrix determinants (Sylvester's
criterion), and an exact matrix solve. The initial version, after orienting
a definite Hessian, proved emptiness if the minimum is strictly positive, or if a zero
minimum's unique supported-coordinate point violates integer, fixed-face,
or sector constraints. A negative minimum, singular/indefinite Hessian,
parameter dependence remained unknown; an admissible zero minimum initially
remained unknown too. The subsequent propagation extension below now handles
that zero-minimum case exactly. The test runs only after ordinary
restriction, affine admission, normalization and factorization; it discards
one impossible AND branch without dropping any pending OR siblings. It is
not a polynomial-case representation or a claim of selected-sector closure.
At most 32 native principal-minor determinants and one native linear solve
are attempted per checked equation; larger supports return unknown. The
existing geometry subtime counters do not separately time this narrow proof,
but the enclosing sector solve time includes it.

The frozen release gate `TMP/definite-quadratic-gate.k9FOXi/` passes its compile
check, all **46** focused intersection tests, and the full core suite:
**2,325 passed, 32 existing ignored, zero failed**, with 171.87 seconds of
full-suite runtime. Nine added adversarial tests come from an independent
auditor. All frozen source/manifest hashes remain unchanged through the gate.
The independent proof/API audit is
`TMP/definite-quadratic-independent-audit-2026-09-20.md`.
These tests include the exact captured Q-first parent and guard; they do not
replace a fresh selected-sector solve or assert that its later cases close.

### Fresh public-CLI retry: a second empty factor branch, still case 193

The frozen release CLI with the first quadratic fix and independent progress
monitor was run on the complete downset of physical sector 28686 (internal
14343): seven nonzero jobs, sparse exact, Q-first ordering, numerical depth
zero, one CPU, an 8 GiB address-space cap and an initial 900-second deadline.
The preliminary K1/K3 candidate controls passed. Evidence is retained in
`TMP/tide-qfirst-public-pilot.lcnStQ/`; the binary is frozen under
`TMP/family-heartbeat-qualified-gate.Q1pFqz/`.

The parent still failed at **case 193**, with 192 prior observed rules. Removing
one empty exceptional branch did not finish all its siblings. The next
reported native Gröbner conjunction includes, with `x=n10`, `w=n14`, `y=n7`,

```text
25*w^2 - 100*x*w + 102*x^2 = 0,
102 - 25*w^2 + 68*x + 100*x*w - 204*y - 68*x*y + 102*y^2 = 0.
```

The first is `(5*w-10*x)^2+2*x^2=0`, so it forces `x=w=0` over the reals.
The second then forces `102*(y-1)^2=0`. Since `n7` is inactive, `y<=0`:
this normalized factor branch is empty. An independent audit checked this
deduction. It must not be extended to the **whole original conjunction**:
normalization and factor splitting leave other OR children. The corrected
regression analysis below exhibits exact witnesses on another child.

The initial helper checks each definite quadratic for an **excluded** minimum.
It cannot propagate an admissible zero-minimum point into the remaining
equations. The subsequently release-validated extension uses the exact equivalence
`q=0 <=> gradient(q)=0` for a definite quadratic whose minimum is exactly zero,
then invokes the existing affine admission and chart-restriction service on
the whole conjunction. This must preserve other free coordinates, all AND
equations and every OR sibling, with strict refinement and existing budgets.
It does not apply to the natural ordering's indefinite infinite conic.

#### New propagation gate: sound branch refinement, incorrect test expectation

The first gate for this extension (`TMP/quadratic-propagation-gate.mWqql1/`)
passed the release compile check, then **52 focused tests passed and one
failed**. The failure was an overly broad test expectation that the original
two cubic/quadratic equations had no admissible solutions. Exact substitution
of `n3=-1`, `n10=n14=0`, and any inactive `n7<=0` disproves that expectation.
The production routine correctly failed closed on an unresolved sibling;
it did not publish a false empty-locus certificate. The corrected regression
checks exact witnesses and atomic unsupported-geometry failure in both input
orders. Full core tests and a new CLI build were not reached by that failed
gate; the subsequent corrected gate below completes them.

As a separate diagnostic, the exact already-built test executable was then
run across the rest of the library with **only that incorrect regression
explicitly filtered out**: 2,331 passed, 32 existing ignored, zero failures,
one filtered, 167.57 seconds test runtime. This supports the branch-refinement
implementation but is not a passing full gate or execution of the corrected
test. Receipts: `TMP/propagation-existing-binary-controls.PytAS8/`.

The corrected combined gate then completes successfully in
`TMP/propagation-source-visit-gate.sr8Egb/`: **2,338 core tests passed**,
32 existing ignored, zero failed or filtered; 199.81 seconds test runtime.
The 53 intersection and 14 search tests also pass as focused, overlapping
subsets. The release CLI builds and both one-worker sparse/depth-zero
candidate-generation smoke controls pass: K1 writes one rule/one residual
and K3 writes 18 rules/four residuals. They are explicitly uncertified
candidates, not new certified closing artifacts. All 1,345 frozen source/
manifest hashes and both executable hashes pass independent verification.
The mathematical/source-provenance audit is retained in
`TMP/propagation-source-visit-independent-audit-2026-09-20.md`.

After the empty child is removed, the remaining child has
`n3=-1-n14+2*n10`. Set `a=-n10`, `b=-n14`, `c=-n7`; the exact residual becomes

```text
(7*a-4*b)*(c+1) = 15*a^2-18*a*b+5*b^2,
a,b,c >= 0, b <= 2*a+1.
```

It is not empty or a finite collection of affine faces. For every integer
`t>=1`, `a=b=3*t`, `c=2*t-1` gives an admissible ray, or in original indices
`(n3,n7,n10,n14)=(-3*t-1,1-2*t,-3*t,-3*t)`. More generally, for
`0<m<=n` and integer `t>=1`, put

```text
D = 7*n-4*m,
a = t*n*D, b = t*m*D,
c+1 = t*(15*n^2-18*m*n+5*m^2).
```

These obey the full sector inequalities and equation; distinct rational
ratios `m/n` give infinitely many conic directions. With `w=c+1`, the doubled
homogeneous matrix has determinant 38 and rank three, ruling out a product
of linear factors. The earlier empty-branch proof remains valid, but cannot
make this sibling disappear. This strengthens the reason to test alternative
exact rules/source visitation rather than repeatedly extending an emptiness
test to nonempty domains.

The executor continued its pinches after the parent error. Internal sector
14342 completed with **206 rules and four finite residuals**, saved as a
20,336,506-byte checkpoint. In sector 14341, case 130 reached a 547-row,
1,903-column exact frame with five active variables; its last row consumed
several minutes and memory continued growing. Root stopped the pilot
adaptively because the already-failed parent prevented a complete output.
The final receipt is **534.51 s wall**, **530.04 s CPU**, **2,391,020 KiB peak
RSS**, shell status **143/SIGTERM**. This is an interruption, not expiry of the
900-second deadline, memory exhaustion or completed-family timing.

There was late progress before termination: exact elimination reached
canonicalization at elapsed 531.7 s. Thus it is wrong to describe that row as
never returning. The second sector did not finish; no assembled candidate
bundle or closing artifact was written. The single completed checkpoint is
preserved. These measurements cover preparation and several downset jobs and
must not be compared as a speed ratio with the earlier selected-sector core
timings. The monitor provides bounded snapshots, not a full event journal;
coalesced events do not permit an exact aggregate phase-time decomposition.

### Narrowed original-SpIRed oracle

An input-driven, workspace-only adapter invokes the unchanged original C++
`family::init`, `generateIBPs(true)`, ordering lookup and public `solveSector`.
Before execution, a native RustRed exporter checked every supplied momentum
row against the lowered family and wrote exactly **5,566** unique proved-zero
masks. The family fingerprint, physical sector bits and Q-first permutation
match the Rust diagnostic. Preparation is not included in C++ solver timing.

The original optimized C++ selected-sector process was pinned to one CPU,
with numerical depth zero, an 8 GiB address-space ceiling and a 900-second
pilot deadline. It **timed out without returning a sector result or writing
rules**: 900.02 s process wall, 893.82 s user plus 5.10 s system CPU,
144,000 KiB peak RSS, exit 124. Its output directory is empty. External
resource observations show continuing CPU activity and modest memory use;
the release solver supplied no per-case trace, so they establish neither
which case it reached nor a completion forecast. No larger run was admitted.
Receipts are in `TMP/tide-spired-zero-oracle-prep.20260920/`.

This is not a speed comparison: Rust's public CLI attempted the whole
downset, whereas this C++ adapter attempted only its parent, and neither
produced a completed parent result. Further, original SpIRed's nonlinear
fallback enumerates each involved coordinate only through its default power
range 30; even a future completed reference solve would not establish
unrestricted parametric closure. The reference remains useful for narrower
case/rule investigations, but this run has not supplied a missing relation.

#### Completed isolated natural-order case

The next narrower adapter calls the original `rowReduce` and `solveCase` on
the exact natural-order parent `[1,1,1,*,*,0,0,*,*,0,*,1,1,1,0]`, with the same
25 ordinary sources and audited 5,566-mask zero census. It does not invoke
`solveSector`, nonlinear `linearize`, or a finite-power scan. The original
C++ solver and library are unchanged. The adapter only exports the returned
conditional rule, physical integral keys, raw exact numerator/denominator
coefficients, and all exceptional OR branches.

The optimized one-CPU run **completed with exit zero**:

| Boundary / result | Measurement |
| --- | ---: |
| Preparation | 0.022805 s |
| Preconditioning | 0.002337 s |
| `solveCase`, including guard extraction | 52.063474 s |
| Preconditioning plus case/guards | 52.065811 s |
| Process wall / CPU | 52.17 s / 52.05 s |
| Peak RSS | 103,860 KiB |
| RHS terms / exceptional OR branches | 781 / 7 |
| Formally non-descending RHS terms | 0 |

Its fifth exceptional branch, under the explicit map `Cpp n_i -> Rust n_(i-1)`,
is **exactly the same two affine equations and indefinite Q** displayed at
the start of this note. This is direct evidence that the obstruction is not
unique to RustRed. C++'s seven branches and Rust's three raw guard branches
use different intermediate nonlinear decompositions; counts alone neither
prove nor disprove equivalent bad loci.

The subsequent independent native Symbolica comparison **passes all 781
coefficients and physical RHS keys**, with no missing keys, no nonzero exact
differences and no nonproportional denominators. It decodes the Rust binary
coefficient catalog, checks family identity, joins physical rather than
ordinal integral keys, and applies a bijective simultaneous symbol map.
Each equality is checked by native rational normal form and by a full exact
polynomial cross-product. Every raw C++ denominator divided by its Rust
canonical counterpart is a nonzero constant, preserving the coefficient
pole loci rather than merely comparing numerical samples. It also checks
the physical target key and inverse symbol-map round trips. The checker
uses only native Symbolica CAS operations; its internal runtime is 1.306 s,
not part of either solver timing. Results are retained under
`coefficient-comparison/` in the oracle directory. This proves the conditional
formula match, not equality of the complete seven-versus-three guard
decompositions or whole-sector closure.

The saved Rust isolated case returned 781 terms with 20.3059412 seconds core
and 0.0112739 seconds guard extraction outside that core boundary. This older
frozen-binary measurement is not a paired, reproduced speed comparison with
the new C++ run. Neither isolated result is a completed sector or a closing
five-loop artifact. Receipts, exact exports and independent audit are in
`TMP/tide-spired-isolated-case-prep.20260920/`.

## Narrow next steps

1. Use the now-observed native integer-domain representation when designing
   any nonlinear-case adapter. Preserve every coverage guard and branch
   condition; the returned radicals are not already admissible integer charts.
   Use explicit integer witnesses only to prove nonemptiness, not coverage.
2. Experiment with multiple exact, strictly descending rule candidates for
   the *same parent case* by varying source support/schedule or ordering. The
   current search returns the first winning pivot
   ([`search.rs`](../../crates/rustred-core/src/solver/search.rs)); whether a
   guard-avoiding candidate exists is unknown. Publish a candidate portfolio
   only after native exact conjunction reasoning proves its **shared** bad
   locus empty or representable by supported cases. If every candidate has
   this locus, the experiment has not solved the obstruction.
3. A separately labeled finite-power fallback (for example power at most 30)
   may evaluate requested integer points with exact replay and memoization.
   It cannot be advertised as an unbounded closing parametric IBP artifact.
   Full polynomial/ideal cases are a larger architectural alternative; reuse
   native Symbolica algebra, and keep integer-sector feasibility and rule
   coverage as explicit proof obligations.

The new public isolated-case method
`SectorSolver::solve_case_with_source_order_and_observer` supplies a controlled
diagnostic for step 2. It validates the complete permutation **after basis
preconditioning and before search**, keeps basis storage and `basis_row` IDs
unchanged, and retains the existing native trace order with matching source
witnesses. The default method takes the allocation-free natural path; sector
defaults and schemas are unchanged. Six release tests cover identity
equivalence, invalid orders before observed work, direct/provenance and GPLU
replay, reverse/rotation determinism, seed fairness and an empty basis. This
does not automatically retry a failed case or admit unsupported geometry.
The prepared external client adds bounded identity/reverse/half-rotation
experiments with the same integral ordering, and reports complete exceptional
geometry separately from a returned conditional formula. Those new campaign
results are not yet available at this checkpoint.

### Measured ordering experiments and bounded scope

The next input-driven portfolio keeps the supplied family and sector fixed.
The following are zero-based coordinate-priority permutations, not changes to
the denominator definitions or physical integral keys:

| Heuristic | Permutation |
| --- | --- |
| Reverse the current tie-breaks | `14,13,12,11,10,9,8,7,6,5,4,3,2,1,0` |
| Residual-Q support, then affine pivots | `7,8,10,3,4,0,1,2,5,6,9,11,12,13,14` |
| Affine pivots, then residual-Q support | `3,4,7,8,10,0,1,2,5,6,9,11,12,13,14` |

The first two probes reached search with sparse arithmetic, one pinned worker,
numerical depth zero and a 300-second external deadline. A later third probe
using the same frozen binary exhausted that process deadline during family
preparation, before search. It therefore supplies no ordering verdict.
Receipts and an independent audit for the first pair are retained in
`TMP/source-weight-lru-comparison.xSY9i1/`; the third receipt is in
`TMP/tide-affine-first-pilot.SJ3wBI/`.

| Ordering | Solver core to return | Process wall | Peak RSS | Result |
| --- | ---: | ---: | ---: | --- |
| Natural (earlier frozen binary) | 81.514 s | 89.18 s | 199.9 MiB | Case 195: genuine nonlinear domain |
| Reverse | No return | 300.15 s | 1,610.4 MiB | External deadline; case 177 exact frame unfinished |
| Q support first | 172.844 s | 179.29 s | 369.0 MiB | Case 193: unsupported but provably empty domain |
| Affine support first (later preparation-censored probe) | Not entered | 309.48 s | 68.4 MiB | External deadline before preparation completed; no solver event |

These are **not completed-sector timings** and cannot establish a winning
ordering or speed ratio. Reverse and Q-first overlapped on separate pinned
cores of the shared host; the earlier natural binary differs in source-weight
caching, not in the sparse path used here. Reverse reached a 611-row,
2,383-column exact frame; Q-first completed all 102 materializations before
its geometry error, with 148.810 seconds in completed exact intervals.
The later affine-first probe produced only the event-file header, no source/
zero-census completion summary or rule output. Its 303.41 s user plus 5.98 s
system CPU does not establish which preparation substage dominated. Do not
attribute this setup-only timeout to its ordering; that remains untested in
the actual search. Reuse input-bound prepared data and separate setup/core
budgets before repeating the ordering comparison.

Changing the permutation can change the case queue: getting past ordinal 195
is not itself completion. Nor can rules
from different orderings be merged without proving strict descent under one
common persisted ordering. A cheaper isolated-case prescreen may reveal a
different guard, but cannot substitute for traversing its exceptional cases.
The priorities above come from the observed polynomial supports, not a
topology-name strategy or borrowed reduction relations.

The Q-first failure is the empty domain proved above, not a missing IBP or a
new master. The exact trace still had 68 queued cases at case 193; exceptions
can create more, so that queue length is not a fixed completion denominator.
After the release-tested correction, the next pilot will traverse the same
physical root and its pinches through the public CLI, with completed-sector
checkpoints and a bounded initial time/memory allocation. That full-downset
workload is different from the selected-sector timings in this table.

#### What a reference SpIRed run can and cannot settle

The original `dioSys::simplify` calls `simplifyNlin`, then `linearize`, with default
`MAX_NLIN_SEARCH=30`. It enumerates active powers 1 through 30 and inactive
powers 0 through -30 only for variables present in the remaining nonlinear
polynomials, then checks the remaining linear relations. This is neither a
total-numerator-power-30 contract nor necessarily a box on every original
index. Its header explicitly warns that an infinite solution set can be
replaced by an inequivalent finite set. A returned rule collection has no
unrestricted-closure quality flag.

Consequently reference success on the natural conic would require inspection
of this fallback before any closure claim. A useful narrower comparison is the
same parent rule and exceptional conditions. The prepared oracle protocol also
accounts for sector-bit endianness (external MSB label 28686 corresponds to
C++ integer 14343), coefficient variable names, the identical zero-mask set,
and the same `q^2-1` convention. It is a protocol, not a completed C++ run:
`TMP/tide-spired-oracle-proposal.T9KRZR/PROPOSAL.md`.

A bound on negative index degree is `sum_i max(-n_i,0)`, not momentum tensor
rank (quadratic numerators have twice that momentum degree). For this captured
case all positive indices are fixed at one: degree at most 30 reduces the
search to `n7,n8 in [-30,0]`, `u in [-29,1]`, hence at most 29,791 triples
before exact chart-integrality, polynomial, sign and degree checks. This is a
finite test of **this case**, not a rank-bounded family certificate. In the
whole family a numerator-only bound leaves positive propagator powers
unbounded. The current certification API therefore correctly rejects that
unsupported promise; a total-excess bound also restricts dots and is a
different contract.

If every ordering encounters the same locus, compare original-source and
preconditioned ranks on exact witnesses before calling it intrinsic. The
current polynomial preconditioner preserves the generic fraction-field span,
not every specialized rank. Simply permuting the 25 input IBPs is not a
controlled change because preconditioning sorts them. Any source-subset
experiment must preserve source conditions, original-row provenance and exact
replay; success on a subset alone is not a new coverage proof.

### Pointwise original/preconditioned source-span screen

A frozen-library native finite-field diagnostic now checks seven external
integer points: three natural-order points and four Q-first points, including
on-conic witnesses and off-conic controls. At each point it compares original,
preconditioned and vertically stacked source spans over two primes, three
dimension samples and two seed prefixes: **84 comparisons**. Every sampled
rank agrees across all three matrices: 25 at the zero-seed prefix and 515
for the 21-seed depth-at-most-one prefix (525 rows). No sampled specialized
rank loss or extra stacked span was observed.

This intentionally retains every physical integral column, with **no zero-
sector projection**, and uses a pointwise numerical column ordering. The
target was not a pivot in either span, even for the off-conic controls.
Thus these shallow samples do not establish irreducibility, absence of
deeper relations, or adequacy of the preconditioner on every exceptional
point. They do rule out an observed source-span mismatch in this screen.
All arithmetic and sparse reduction are native Symbolica operations; no
custom elimination kernel or production algorithm changed. Natural and
Q-first runs completed in 0.40 s and 13.22 s process wall respectively,
both with 12,288 KiB peak RSS. These are rank-diagnostic timings, not sector
generation or a speed comparison. Receipts, external points and qualified
analysis: `TMP/native-precondition-rank.20260920/`.

## Input-only scalar-product coordinates: bounded diagnostic

The connected six-line representative also admits the unit-Jacobian routing
`p=(k1,k2,-k3,k3-k5,-k4+k5)`, with determinant +1, into the five-loop banana
presentation. Its physical denominators are `D_i=p_i^2-1` for `i=1,...,5`
and `D_6=(sum p_i)^2-1`. Keeping these six denominators unchanged, a separate
external input replaces nine inactive pair-square auxiliaries with pure
scalar products `S_ij=sp(p_i,p_j)`, omitting `S_45`. The exact identities are

```text
(p_i+p_j)^2-1 = D_i+D_j+1+2*S_ij
S_45 = (D_6-sum(D_1,...,D_5)-4)/2-sum(other nine S_ij).
```

This is an input-basis experiment, not a topology-dependent engine change.
The full auxiliary-sector zero census must be recomputed; positive powers of
these new auxiliaries describe different formal sectors. It returns 20,065
proved-zero masks out of 32,768, rather than the TIDE presentation's 5,566.
Concrete polynomial numerators can be transported exactly between bases, but
arbitrary symbolic negative powers cannot be relabeled by a fixed-size key
map. Rule counts or raw coefficients across presentations are therefore not
direct equality tests.

Three one-worker, release, old-binary probes used an 8 GiB address-space
ceiling and nominal 180-second process deadline. They predate the new
quadratic propagation and do not validate it. Actual terminal receipts are:

| Backend | Workload | Process wall | Preparation | Peak RSS | Outcome |
| --- | --- | ---: | ---: | ---: | --- |
| Sparse exact | Entire selected parent | 188.78 s | 4.170 s | 156,732 KiB | Timeout; 134 preceding rules, case 135 unfinished |
| Factorized exact | Isolated case 135 | 187.20 s | 125.837 s | 21,540 KiB | Timeout during discovery; exact algebra not started |
| Source-weight reconstruction | Same isolated case | 189.74 s | 65.364 s | 33,904 KiB | Timeout during reconstruction; no returned rule |

The isolated face is `[1,1,1,1,1,1,*,*,*,0,0,*,*,0,*]`, without additional
affine equations. Sparse and source-weight runs expose a 550-row, 2,041-column
frame with seven active variables. No completed candidate or sector output
exists. In particular the factorized result says nothing about the cost of
factorized exact algebra, because that stage was not reached.

The unexplained preparation variance consumed unequal fractions of the
process deadlines. These are censored diagnostics, **not a backend speed
comparison or evidence of successful closure**. The next controlled
comparison must reuse identical prepared data or give each solver the same
core-time budget, with a separate process safety ceiling. Preserve both
preparation and core timings; do not remove initialization costs from the
logical-cold workload. Receipts and independent audits remain in
`TMP/banana-dot-isp-baseline.bVIpL9/`,
`TMP/banana-dot-isp-factorized.RRPLRG/`,
`TMP/banana-dot-isp-weights.5o0P68/` and
`TMP/five-loop-sector-local-isp-audit-2026-09-20.md`.

### Preparation-only native profile and reuse

A separate sampled rerun of the **unchanged native preparation exporter**
completed on one pinned CPU in 15.11 s wall, 14.58 s user plus 0.20 s system,
with 63,368 KiB peak RSS. Its zero list and family manifest are byte-identical
to the earlier 4.62-second export. It performed preparation only, not IBP
search. The profile has 240 user-cycle samples, zero reported lost samples:
about 40% is attributed to `Analyzer::analyze`, 35% to Symbolica/Numerica's
native finite-field `Matrix::partial_row_reduce`, and 5% to RustRed's modular
full-column-rank adapter. These are coarse sampled symbol attributions, not
exact phase durations; inlining and sampling overhead matter. The original
predicate and native matrix implementation were not changed. Receipts:
`TMP/tide-preparation-profile.IdhCGM/`.

This completed control does not explain the much longer censored preparation
in the other binary and does not establish a backend speed ratio. A trusted
workspace-only comparison loader now allows reuse of the already generated
global zero census for the *identical* family. It checks the freshly lowered
fingerprint, arity and loop count, strict binary row framing, unique ascending
bit-i order and count; run scripts pin the audited native producer output.
Four standalone optimized parser tests and independent source review pass.
This is not a new production format or a proof checker for untrusted zeros.
The selected-parent executable using it is still awaiting its build and run.

Report native preparation, prepared-data loading and solver-core time
separately. Reusing the census is appropriate for repeated solver/ordering
experiments, but does not make the original preparation disappear from a
logical-cold end-to-end benchmark. No mathematics, rule or search result is
cached by this diagnostic loader. It must reject the different pure-ISP
presentation rather than reuse the TIDE zero set there.
