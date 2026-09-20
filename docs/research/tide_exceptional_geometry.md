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
Consequently the planned separate 30-second native solve could not run:
**no `SolutionSet` result for this particular Q has yet been observed**.
This is a direct-link compilation bottleneck, not evidence that the native
integer solver rejects or cannot describe Q.

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
criterion), and an exact matrix solve. After orienting a definite Hessian,
it proves emptiness if the minimum is strictly positive, or if a zero
minimum's unique supported-coordinate point violates integer, fixed-face,
or sector constraints. A negative minimum, singular/indefinite Hessian,
parameter dependence, or an admissible minimum remains unknown and retains
the existing unsupported-geometry behavior. The test runs only after ordinary
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

## Narrow next steps

1. Run the prepared native integer-domain probe when a reusable compiled
   Symbolica test harness or sufficient compile budget is available. Preserve
   every coverage guard and branch condition; use explicit integer witnesses
   only to prove nonemptiness, not whole-locus coverage.
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

### Measured ordering experiments and bounded scope

The next input-driven portfolio keeps the supplied family and sector fixed.
The following are zero-based coordinate-priority permutations, not changes to
the denominator definitions or physical integral keys:

| Heuristic | Permutation |
| --- | --- |
| Reverse the current tie-breaks | `14,13,12,11,10,9,8,7,6,5,4,3,2,1,0` |
| Residual-Q support, then affine pivots | `7,8,10,3,4,0,1,2,5,6,9,11,12,13,14` |
| Affine pivots, then residual-Q support | `3,4,7,8,10,0,1,2,5,6,9,11,12,13,14` |

The first two probes have now run, with sparse arithmetic, one pinned worker,
numerical depth zero and a 300-second external deadline. The third remains
unrun. Receipts and an independent audit are retained in
`TMP/source-weight-lru-comparison.xSY9i1/`.

| Ordering | Solver core to return | Process wall | Peak RSS | Result |
| --- | ---: | ---: | ---: | --- |
| Natural (earlier frozen binary) | 81.514 s | 89.18 s | 199.9 MiB | Case 195: genuine nonlinear domain |
| Reverse | No return | 300.15 s | 1,610.4 MiB | External deadline; case 177 exact frame unfinished |
| Q support first | 172.844 s | 179.29 s | 369.0 MiB | Case 193: unsupported but provably empty domain |

These are **not completed-sector timings** and cannot establish a winning
ordering or speed ratio. Reverse and Q-first overlapped on separate pinned
cores of the shared host; the earlier natural binary differs in source-weight
caching, not in the sparse path used here. Reverse reached a 611-row,
2,383-column exact frame; Q-first completed all 102 materializations before
its geometry error, with 148.810 seconds in completed exact intervals.

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
