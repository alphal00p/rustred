# Three-loop B4 scalar-dot recurrence research

## Scope and verdict

This note studies the genuine four-line sector of the equal-mass three-loop
tetrahedron family,

```text
B(a1,a2,a3,a4) = I(a1,a2,0,a3,0,a4),  ai >= 1,
```

whose canonical tetrahedron mask is `43`.  The four active routings are
positions `0,1,3,5`; positions `2,4` complete the six-dimensional scalar-
product basis and are irreducible numerators in this sector.

The result is deliberately a **no-go for a new unrestricted implementation**,
not a recurrence claim.  Static algebra exposes a useful scalar transfer
identity and a precise finite-shell implementation target, but it does not
yet prove a well-founded parametric descent for every dot degree.  In
particular, the transfer identity has an increasing number of same-degree
orbit directions which it cannot determine.  No FORM or Mathematica program
was executed while preparing this note.

## The exact scalar transfer identity

Write

```text
S = a1+a2+a3+a4,
e_i = the ith unit vector.
```

For any `ar > 1`, the static MATAD `redBNn5` rule translates to

\[
 (a_r-1)B(a)+\sum_{j\ne r}a_j B(a-e_r+e_j)
 +{3d/2-S+1\over m^2}B(a-e_r)=0.                 \tag{1}
\]

The relevant source is
`vendor/gammaloop/crates/vakint/form_src/matad/matad-ng.hh:3723`; its
`nom(x,-3)` becomes `x-3(2-d/2)` in `Conv2exact` at lines 343--364.
Equation (1) uses RustRed's Euclidean `Di=qi^2+m2` convention, with the MATAD
symbol `M^2` identified with `m2`.

There is a useful independent normalization check.  At
`a=(2,1,1,1)`, all three transfer integrals are equal by the proved `S4`
line symmetry.  Equation (1) gives

\[
 B(2,1,1,1)={8-3d\over8m^2}B(1,1,1,1),          \tag{2}
\]

which is exactly the one-dot B4 formula already frozen by the native
three-loop finite certificate and by the four-loop component-halo design.
This agreement authenticates the convention translation, but it does not
authenticate the rest of MATAD as a RustRed proof surface.

Equation (1) is also easy to recognize structurally.  Put
`x_i=a_i-1`, so `sum(x_i)=D` is the total dot degree, and set
`y=x-e_r`.  Its same-degree part is

\[
 \sum_{j=1}^4 (y_j+1)B(y+e_j).                   \tag{3}
\]

Thus choosing a different raised line `r` above the same degree-`D-1` seed
does not create a new scalar relation.  This explains why a one-lowered-seed
search cannot manufacture a strict scalar pivot merely by changing the nine
IBP weights.

## Rigorous obstruction beyond one dot

The relevant symmetry after fixing the B4 mask is its eight-element dihedral
stabilizer, not a full permutation group on the four active lines. At dot
degree two there are three orbits in cyclic compact order,

```text
A     = B(3,1,1,1),
C_adj = B(2,2,1,1),
C_opp = B(2,1,1,2).
```

Applications of (1) collapse to the same equation,

\[
 2A+2C_{\rm adj}+C_{\rm opp}
 =-{3d/2-5\over m^2}B(2,1,1,1).                 \tag{4}
\]

Consequently (1) leaves two scalar directions unresolved already at `D=2`.
This is separate from, and consistent with, the existing rank-nine/nullity-
zero result for raw-row **weights** at one lowered B4 seed: forbidding all
inactive numerators and all same-dot transfers forces the raw-row weight to
zero, while allowing transfers produces only (4).

The exact scalar-orbit generating function for this `D8` action is

\[
 {1\over8}\left(
 {1\over(1-x)^4}+{2\over1-x^4}+{3\over(1-x^2)^2}
 +{2\over(1-x)^2(1-x^2)}\right).                 \tag{5}
\]

Let `b4(D)` be its degree-`D` coefficient. Equation (3) supplies at most one
orbit row per stabilizer orbit at degree `D-1`. Therefore its orbit matrix
obeys

\[
 \operatorname{rank} R_D\le b_4(D-1),\qquad
 \operatorname{nullity}R_D\ge b_4(D)-b_4(D-1).  \tag{6}
\]

In particular `b4(1)=1` and `b4(2)=3`, reproducing the two-dimensional
obstruction above. Hence the MATAD scalar-transfer rule, even with every
allowed line symmetry, cannot be an unrestricted scalar reduction by itself.
An earlier partition-count diagnostic quotienting the active powers by a full
`S4` was too coarse and is not evidence for B4.

## What the remaining static MATAD rules do and do not prove

The old no-table MATAD path applies, in order, `redBNn5`, `redBNn34`,
`redBNn12`, and `redBNn6`; see `matad-ng.hh:5828-5918`.

- `redBNn6` (`matad-ng.hh:3990`) is a scalar recurrence for the one-line
  family `B(1,1,1,n)`.  Its visible denominator contains
  `(n-1)(d-n)(n+1-d)^2`, so at least
  `n=1`, `d=n`, and `d=n+1` belong to its exceptional locus.
- `redBNn34` (`matad-ng.hh:3770`) does **not** stay in the scalar B4 space.  In
  the `n1=n2=0` specialization it emits a `p2.p2` numerator.
- `redBNn12` then reduces those numerator branches.  Its formulas are coupled
  and can emit further numerator and pinched terms.
- The newer default path first uses a dimension-shift/d'Alembertian relation
  and then finite tables (`matad-ng.hh:5639-5738`).  Tables and dimension
  shifts are not an unrestricted native RustRed recurrence.

Therefore copying `redBNn6` would only solve one of the unresolved orbit
directions at a fixed degree.  At degrees where
`b4(D)-b4(D-1)>1`, numerator-coupled relations are essential.  The static
procedures are valuable derivation hints, but their FORM term ordering,
implicit zero tests, dimension shifts, and table normalization cannot serve
as a RustRed certificate.

## Minimal native finite shell

The smallest new shell worth extracting is exact dot degree two. It must
contain all three scalar columns `A`, `C_adj`, and `C_opp`; equation (4) proves
that a single scalar-transfer row is insufficient. A native shell should
use the following closed bookkeeping universe:

1. scalar B4 seeds through the requested dot degree `D`;
2. all nine authenticated `d/dk_i . k_j` rows per symmetry-unique seed;
3. the one-step genuine B4 numerator halo on inactive tetrahedron positions
   `2,4`, retained as typed columns rather than discarded;
4. exact scaleless and factorized boundary dispatch for every proper-sector
   branch; and
5. all scalar B4 columns through `D+1` reached by the raw rows.

For exact scalar degree `D`, the labelled target count is
`binomial(D+3,3)` and the symmetry-unique count is `b4(D)`.  A scalar seed can
emit at most one inactive numerator in one raw step.  These facts give finite
preflightable target and halo sets for each configured `D`; they do not imply
that a fixed numerator cap closes all degrees parametrically.

The existing `ThreeLoopReductionPipeline` is already the right finite-box
oracle: with `max_numerator_degree=0` it generates only genuine scalar seeds,
but its raw equations retain the one-step numerator halo and construction
rejects any advertised target left outside the fixed terminal set.  The
checked-in `certified_three_loop_three_dot_scalar_box` test requests this
finite certificate through `D=3`.  That bounded behavior is evidence for the
shell design, not an induction theorem for arbitrary `D`.

## Implemented finite step and next proof obligation

[`three_loop_b4_d2.rs`](../../crates/rustred-legacy-oracles/src/three_loop_b4_d2.rs) now implements the
dedicated replayable `D=2` shell described below. Its deliberately narrow
surface freezes five symmetry-unique scalar seeds (corner, one dot, `A`,
`C_adj`, and `C_opp`), regenerates all 45 native rows, and gives every row a
stable `(seed,differentiated,contracted)` identifier. Scalar, one-step
numerator, and closed boundary columns use disjoint versioned types. All six
powers are moved together when a B4 stabilizer image is chosen, so numerator
positions are not canonicalized independently from physical powers.

The constructor closes proper sectors with the native
`ThreeLoopBoundaryReducer`, mass-normalizes the matrix to rational functions
of `d`, and performs deterministic exact sparse elimination. It stores each
pivot as an exact weighted combination of normalized raw rows, records the
nonconstant polynomial required to be nonzero at every `d`-dependent row or
pivot division, requires pivots for all three degree-two orbits, and rebuilds
and replays the certificate before returning it. Conservative limits for raw
incidences, boundary calls, symmetry images, columns, nonzeros, coefficient
degrees, elimination updates, and provenance weights are preflighted or
charged.

The focused exact test freezes rank `18` with free columns exactly `T1^3` and
the undotted B4 corner. Its work statistics are 45 raw rows, 311 raw term
incidences, 62 boundary calls, 5,976 stabilizer images, 236 collected
nonzeros, 1,014 elimination coefficient updates, and 55 retained source-row
weights. Two division events record a condition proportional to `d-4`; this
is one distinct exceptional factor, not two distinct loci, in addition to the
single-scale assumption `m2 != 0`.

An independent generic three-dot pipeline certificate freezes the physical
normalizations

\[
\begin{aligned}
A={}&-{3(d-2)^3\over64(d-4)m^6}T_1^3
 +{9d^3-117d^2+458d-560\over128(d-4)m^4}B_4,\\
C_{\rm adj}=C_{\rm opp}={}&{(d-2)^3\over32(d-4)m^6}T_1^3
 +{9d^3-81d^2+242d-240\over64(d-4)m^4}B_4.
\end{aligned}                                                   \tag{7}
\]

Here `m^2` is RustRed's `m2`. The equality of the two reductions is an exact
result of this finite generic certificate; the inputs remain distinct `D8`
orbits and are never identified during canonicalization. The independent
pipeline check also verifies that (7) satisfies the transfer identity (4).

This completes only the following bounded checklist:

1. enumerate and freeze the three target orbits `A,C_adj,C_opp` and every
   required scalar seed orbit through degree two;
2. regenerate each native row with a stable `(seed,differentiated,contracted)`
   identifier;
3. persist every B4 scalar and inactive-numerator halo column in a disjoint,
   versioned namespace;
4. eliminate exactly over `Q(d,m2)`, storing source-row weights;
5. require pivots for all three targets, close every factorized branch through
   the native boundary reducer, and replay every pivot from the raw rows; and
6. record all denominator polynomials in `d` and expose the excluded generic
   locus rather than silently treating exceptional specializations as proven.

The next proof obligation is to repeat for `D=3,4,...` and compare the
pivot/source-weight skeleton after
shifting the symbolic powers.  A genuine unrestricted implementation requires
turning the stable skeleton into finitely many parametric syzygies involving
the necessary numerator orbits and proving that every right-hand side is
strictly lower under a lexicographic measure such as

```text
(dot degree, numerator degree, scalar orbit order, numerator orbit order).
```

Until that symbolic skeleton and its nonzero pivots are proved for arbitrary
positive powers, RustRed should keep dotted B4 unsupported in the analytic
`ThreeLoopProperDotReducer` and rely only on explicitly certified finite
pipeline boxes.  Implementing the MATAD text directly would overstate both
termination and provenance.
