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

RustRed's current intersection service first restricts to the affine chart,
admits affine equations, uses native Symbolica Gröbner normalization and
factorization, and returns `UnsupportedGeometry` when an irreducible coupled
nonlinear equation remains
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

### Prepared ordering experiments and bounded scope

The next input-driven portfolio keeps the supplied family and sector fixed.
The following are zero-based coordinate-priority permutations, not changes to
the denominator definitions or physical integral keys:

| Heuristic | Permutation |
| --- | --- |
| Reverse the current tie-breaks | `14,13,12,11,10,9,8,7,6,5,4,3,2,1,0` |
| Residual-Q support, then affine pivots | `7,8,10,3,4,0,1,2,5,6,9,11,12,13,14` |
| Affine pivots, then residual-Q support | `3,4,7,8,10,0,1,2,5,6,9,11,12,13,14` |

These are **prepared, not measured successes**. Each experiment must traverse
the whole selected sector independently, initially with sparse arithmetic,
one worker and a 300-second limit. Changing the permutation can change the
case queue: getting past ordinal 195 is not itself completion. Nor can rules
from different orderings be merged without proving strict descent under one
common persisted ordering. A cheaper isolated-case prescreen may reveal a
different guard, but cannot substitute for traversing its exceptional cases.
The priorities above come from the observed polynomial supports, not a
topology-name strategy or borrowed reduction relations.

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
