# Exact three-loop equal-mass vacuum reduction plan

## Scope and proof labels

This note fixes the target family, derives its factorized boundaries, proposes a
five-object scalar master basis, and specifies how to certify a complete RustRed
reduction.  It was prepared by reading the checked-in Rust, LiteRed2, and Vakint
sources as text.  Neither Mathematica nor FORM was invoked, and RustRed must not
invoke either of them.

Claims use these labels:

- **Proved here**: follows by an explicit unit-Jacobian momentum change, a finite
  tensor identity, or an exact check already present in RustRed.
- **Source-observed**: a checked-in independent implementation or test uses the
  stated result, but that source is not itself a proof for RustRed's conventions.
- **Expected, not yet proved**: the working hypothesis to be established by the
  certificates in this note.

The important present limitation is precise: the boundary formulae and sector
census below are proved. RustRed has exact guarded descent for arbitrary scalar
dots in the six-line top and five-line F5 sectors, but not yet in dotted B4.
The latter has a proved obstruction for the one-seed raw-row ansatz; a larger
shell recurrence remains open. Scalar numerators, guarded recurrences covering
*all* integer indices, and minimality of the five proposed masters are also not
yet proved.

## 1. RustRed convention and complete scalar basis

Use the raw positive-Euclidean formal integral

\[
 I(a_1,\ldots,a_6)=
 \int\prod_{r=1}^{3}d^d k_r\;\prod_{i=1}^{6}D_i^{-a_i},
 \qquad a_i\in\mathbb Z,
\]

with no implicit \((2\pi)^{-d}\), \(i\pi^{d/2}\), MS-bar, or epsilon
normalization, and

\[
\begin{array}{lll}
D_1=k_1^2+m^2,&D_2=k_2^2+m^2,&D_3=k_3^2+m^2,\\
D_4=(k_3-k_1)^2+m^2,&D_5=(k_1-k_2)^2+m^2,
&D_6=(k_2-k_3)^2+m^2.
\end{array}
\]

Here the Rust coefficient symbol `m2` denotes \(m^2\).  This is exactly the
routing and denominator sign registered in
[`crates/rustred-legacy-oracles/src/three_loop.rs:10-87`](../../crates/rustred-legacy-oracles/src/three_loop.rs#L10), and it is the same
six-momentum routing registered by Vakint in
[`topologies.rs:76-98`](../../vendor/gammaloop/crates/vakint/src/topologies.rs#L76).

The inverse scalar-product map is

\[
\begin{aligned}
k_1^2&=D_1-m^2,& k_2^2&=D_2-m^2,& k_3^2&=D_3-m^2,\\
k_1\!\cdot k_2&=(D_1+D_2-D_5-m^2)/2,\\
k_1\!\cdot k_3&=(D_1+D_3-D_4-m^2)/2,\\
k_2\!\cdot k_3&=(D_2+D_3-D_6-m^2)/2.
\end{aligned}
\]

There are six independent scalar products at three loops and six independent
denominators, so there is no ISP.  The checked-in test substitutes this inverse
back into every denominator exactly
([`crates/rustred-legacy-oracles/tests/three_loop_family.rs:16-94`](../../crates/rustred-legacy-oracles/tests/three_loop_family.rs#L16)).
RustRed also already generates the expected nine identities
\(\partial_{k_i}\!\cdot k_j\), including a hand-derived first identity and an
independent invariant for all nine
([`crates/rustred-legacy-oracles/tests/three_loop_family.rs:218-274`](../../crates/rustred-legacy-oracles/tests/three_loop_family.rs#L218)).

The six denominators are the edges

\[
(01),(02),(03),(13),(12),(23)
\]

of \(K_4\), in that order.  Vertex permutations give a proved 24-element
\(S_4\) action.  RustRed records explicit unit-Jacobian generators
([`crates/rustred-legacy-oracles/src/three_loop.rs:23-45`](../../crates/rustred-legacy-oracles/src/three_loop.rs#L23)), and the test derives
the same group independently from all vertex permutations
([`crates/rustred-legacy-oracles/tests/three_loop_family.rs:97-145`](../../crates/rustred-legacy-oracles/tests/three_loop_family.rs#L97)).

## 2. Exact sector census

A sector bit is one exactly when the corresponding index is positive.  A
disconnected active-edge graph has incidence rank below three, so some linear
combination of loop momenta occurs only polynomially.  Its dimensionally
regularized integral is scaleless, including when inactive lines occur with
negative powers.  Conversely, every connected active graph contains a spanning
tree; its three edge momenta form a unimodular loop basis and all three loop
directions have a massive denominator.  Thus, for this family:

> **Proved here:** a sector is nonzero exactly when its active \(K_4\) edge
> graph is connected.

The current independent enumeration finds 26 disconnected and 38 connected
labelled sectors, and reduces them to five zero and six nonzero \(S_4\) orbits
([`crates/rustred-legacy-oracles/tests/three_loop_family.rs:160-215`](../../crates/rustred-legacy-oracles/tests/three_loop_family.rs#L160)).
With bit zero corresponding to \(D_1\), the six canonical nonzero masks are:

| mask | positive-index representative | active graph | treatment |
|---:|---|---|---|
| 7 | `(1,1,1,0,0,0)` | star \(K_{1,3}\) | three tadpoles |
| 11 | `(1,1,0,1,0,0)` | path \(P_4\) | three tadpoles |
| 15 | `(1,1,1,1,0,0)` | triangle with a leaf (“paw”) | tadpole × sunset |
| 43 | `(1,1,0,1,0,1)` | four-cycle \(C_4\) | genuine 4-line banana sector |
| 31 | `(1,1,1,1,1,0)` | \(K_4\) minus one edge | genuine 5-line sector |
| 63 | `(1,1,1,1,1,1)` | \(K_4\) | genuine Mercedes sector |

The word “genuine” in the last column means “not graph-factorized by the
formulae below”; it does not by itself prove that the corner is an independent
IBP master.

## 3. Exact factorized boundary reducer

### 3.1 One-loop normalization

Define, in the same raw measure,

\[
 T_n=\int d^d p\,(p^2+m^2)^{-n}.
\]

Dimensional regularization gives \(T_n=0\) for integer \(n\leq0\).  For
positive integer \(n\), the one-loop IBP is

\[
 T_{n+1}=\frac{2n-d}{2n\,m^2}T_n,
 \qquad
 T_n=\frac{(1-d/2)_{n-1}}{(n-1)!(m^2)^{n-1}}T_1.
\tag{1}
\]

If an evaluator is desired, and only for the raw \(d^d p\) convention,

\[
T_n=\pi^{d/2}(m^2)^{d/2-n}
\frac{\Gamma(n-d/2)}{\Gamma(n)}
\]

by analytic continuation.  Reduction tables should retain \(T_1\) through a
chosen three-loop integral rather than introduce gamma functions into the
rational coefficient field.

### 3.2 Scalar corners and dotted active lines

The following identities hold for every positive displayed index.

For mask 7, use \((p_1,p_2,p_3)=(k_1,k_2,k_3)\):

\[
 I(a,b,c,0,0,0)=T_aT_bT_c.
\tag{2}
\]

For mask 11, use
\((p_1,p_2,p_3)=(k_1,k_2,k_3-k_1)\), whose transformation determinant is one:

\[
 I(a,b,0,c,0,0)=T_aT_bT_c.
\tag{3}
\]

Equations (1)-(3) prove that both inequivalent tree-sector corners reduce to
the same \(T_1^3\), even though \(S_4\) cannot map a star graph to a path graph.
This extra equality must be a boundary rule, not be misrepresented as a family
symmetry.

For mask 15, \(k_2\) is an independent bridge tadpole while
\((k_1,k_3)\) form a two-loop sunset:

\[
 I(a,b,c,e,0,0)=T_b\,S_{a c e},
\tag{4}
\]

where

\[
S_{a c e}=\int\frac{d^d u\,d^d v}
{(u^2+m^2)^a(v^2+m^2)^c((v-u)^2+m^2)^e}.
\]

The change \(v\mapsto-v\) converts this to the `k1+k2` sunset convention
accepted by the existing two-loop boundary reducer
([`crates/rustred-legacy-oracles/src/two_loop.rs:19-34`](../../crates/rustred-legacy-oracles/src/two_loop.rs#L19)).  The integrated
two-loop pipeline fixes its two masters to the sunset and factorized product
([`crates/rustred-legacy-oracles/src/two_loop_pipeline.rs:151-180`](../../crates/rustred-legacy-oracles/src/two_loop_pipeline.rs#L151)).
Consequently (4), followed by that pipeline and (1), reduces the scalar paw
sector to \(T_1S_{111}\) and \(T_1^3\) **inside the two-loop pipeline's
advertised finite dot box**.  An all-index paw claim additionally requires a
proved all-index two-loop sunset recurrence.

For comparison, the mask-43 transformation

\[
p_1=k_1,\qquad p_2=k_3-k_1,\qquad p_3=k_2-k_3
\]

has determinant \(-1\) and turns its four active denominators into

\[
p_1^2+m^2,\quad p_2^2+m^2,\quad p_3^2+m^2,
\quad(p_1+p_2+p_3)^2+m^2.
\]

This proves the identification of mask 43 as the four-line banana, but it does
not factorize it.

### 3.3 Arbitrary numerator powers in a tree sector

Equations (2) and (3) are not enough: a complete boundary reducer must allow
every inactive index to be any nonpositive integer.  There is an exact finite
algorithm.

Choose the active tree edges as the unimodular basis \(p_1,p_2,p_3\).  Every
inactive denominator is

\[
D_e=(c_{e1}p_1+c_{e2}p_2+c_{e3}p_3)^2+m^2,
\qquad c_{ei}\in\mathbb Z.
\]

For the star representative the inactive momenta are
\(p_3-p_1,p_1-p_2,p_2-p_3\).  For the path representative they are
\(p_1+p_3,p_1-p_2,p_2-p_1-p_3\).  If the inactive index is \(-r_e\), expand
the finite polynomial \(D_e^{r_e}\).  Integrate one independent loop momentum
at a time using

\[
\int d^d p\;p^{\mu_1}\cdots p^{\mu_{2r}} f(p^2)
=\frac{\sum_{\text{pairings}}g^{\mu_i\mu_j}\cdots}
{d(d+2)\cdots(d+2r-2)}
\int d^d p\;(p^2)^r f(p^2),
\tag{5}
\]

while every odd moment is zero.  Finally use

\[
\int d^d p\;\frac{(p^2)^s}{(p^2+m^2)^b}
=\sum_{j=0}^{s}{s\choose j}(-m^2)^{s-j}T_{b-j}
\tag{6}
\]

and (1).  Equations (5)-(6) terminate for every integer numerator degree and
return a rational function of \(d,m^2\) times \(T_1^3\).

The checked-in `VacuumTensorProjector` already has exact pairing Gram matrices,
odd-rank zero, and rank caching
([`src/tensor.rs:658-685`](../../src/tensor.rs#L658),
[`src/tensor.rs:808-872`](../../src/tensor.rs#L808)).  The boundary
implementation should reuse those pairing and contraction primitives but apply
them **component by component**.  A single global \(O(d)\) projection is weaker
than the independent angular averages available after factorization.

### 3.4 Arbitrary numerator powers in the paw sector

For the mask-15 representative retain \(u=k_1\), \(p=k_2\), and \(v=k_3\).
The two inactive denominators are exactly

\[
\begin{aligned}
D_5&=D_1+D_2-m^2-2u\!\cdot p,\\
D_6&=D_2+D_3-m^2-2p\!\cdot v.
\end{aligned}
\tag{7}
\]

For powers \(D_5^{r_5}D_6^{r_6}\), expand (7), perform the one-loop \(p\)
moment with (5)-(6), contract the resulting metrics, and replace

\[
u\!\cdot v=(D_1+D_3-D_4-m^2)/2.
\tag{8}
\]

What remains is a finite linear combination of \(T_n S_{ace}\).  The one-loop
part closes by (1); each induced sunset closes only when it lies in a certified
two-loop finite box, or after an all-index two-loop recurrence has separately
been proved.  Thus:

> **Proved here:** every integer-index tree integral has a terminating FORM-free
> reduction to \(T_1^3\), and every integer-index paw integral has a terminating
> exact factorization into one-loop tadpoles times two-loop sunset integrals.
> Closure of arbitrary paw indices to the two displayed products is conditional
> on the corresponding two-loop coverage certificate.

Implementation should canonicalize under the proved \(S_4\) action first, then
dispatch masks 7, 11, and 15 to these formulae.  Resource limits may reject an
enormous polynomial expansion, but such a rejection is an operational limit,
not an incompleteness of the formula.

## 4. Proposed master basis and normalization

Keep masters as raw RustRed integrals:

| RustRed name | integral | meaning | status |
|---|---|---|---|
| \(P_3\) | `I(1,1,1,0,0,0)` | \(T_1^3\) | proved boundary product |
| \(S_T\) | `I(1,1,1,1,0,0)` | \(T_1S_{111}\) | proved boundary product |
| \(B_4\) | `I(1,1,0,1,0,1)` | four-line banana corner | expected master |
| \(F_5\) | `I(1,1,1,1,1,0)` | five-line corner | expected master |
| \(M_6\) | `I(1,1,1,1,1,1)` | Mercedes corner | expected master |

The choice of the star rather than the path for \(P_3\) is only a stable output
convention.  Equations (2)-(3) prove their equality at the corner and reduce all
dots on either tree to that choice.

This five-object list has a strong independent source oracle.  Vakint's static
three-loop procedure recognizes exactly the topology classes “mercedes”,
“5-edge”, “banana”, “sunrise-bubble”, and “triple-bubble”
([`integrateduv.frm:155-180`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm#L155)).
Its static master substitutions then retain one corner in each class:

| alphaLoop label | `uvid` power tuple | RustRed topology |
|---|---|---|
| mercedes | `(1,1,1,1,1,1)` | \(M_6\) |
| 5-edge | `(0,1,1,1,1,1)` | \(F_5\) |
| banana | `(0,1,1,1,0,1)` | \(B_4\) |
| sunrise-bubble | `(0,0,1,1,1,1)` | \(S_T\) |
| triple-bubble | `(0,0,1,0,1,1)` | \(P_3\) |

These tuples are in `uvid`'s internal order, **not** RustRed's denominator
order; only the named topology correspondence is asserted.  The source is
[`integrateduv.frm:1162-1187`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm#L1162).
That file also contains static zero-sector and permutation rules
([`integrateduv.frm:245-299`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm#L245))
and guarded-looking index recurrences through the reduction endpoint
([`integrateduv.frm:301-1116`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm#L301)).
It is valuable as an oracle, but its power order, \(d=4-2\epsilon\)
specialization, propagator signs, and normalization must not be imported
silently.

In particular, Vakint's MATAD adapter remaps the I3L edges
([`matad.rs:369-384`](../../vendor/gammaloop/crates/vakint/src/matad.rs#L369))
and explicitly multiplies by loop normalization and
\((-1)^{\sum_i a_i}\) when reconciling its conventions
([`matad.rs:628-656`](../../vendor/gammaloop/crates/vakint/src/matad.rs#L628)).
RustRed's positive-Euclidean formal reduction must not contain that adapter
sign.  Vakint's larger MATAD expansion catalogue
([`matad.rs:33-99`](../../vendor/gammaloop/crates/vakint/src/matad.rs#L33))
also covers other mass patterns and is not evidence for additional masters in
this fixed all-equal-mass family.

For dimensional checks, put \(A=\sum_i a_i\).  Then

\[
[I(a)]=(\text{mass})^{3d-2A}.
\]

If a term reducing \(I(a)\) contains master \(I(b)\), its coefficient must be
homogeneous with mass dimension \(2(\sum b_i-A)\), equivalently proportional
to \((m^2)^{\sum b_i-A}\) times a rational function of \(d\).  Do not make the
masters dimensionless inside the reduction table: doing so would introduce
non-rational powers \((m^2)^{3d/2}\).  An evaluation adapter may apply such an
overall normalization after reduction.

> **Expected, not yet proved:** the five displayed objects are a minimal basis
> over \(\mathbb Q(d,m^2)\) for all integer indices.  The static alphaLoop list
> is compelling independent evidence, not the required RustRed rank and
> recurrence certificate.

## 5. Solver order and sufficient coverage

### 5.1 Sector order

Use this dependency order:

1. discard all disconnected sectors;
2. close masks 7 and 11 analytically onto \(P_3\);
3. close mask 15 analytically through the two-loop pipeline onto \(P_3,S_T\);
4. solve the mask-43 banana sector;
5. solve mask 31 using already closed proper subsectors;
6. solve mask 63 using already closed proper subsectors.

Every labelled sector is first mapped into one of these representatives by a
proved momentum transformation.  Boundary equality between masks 7 and 11 is
then applied separately from \(S_4\).

Within a sector define

\[
r(a)=\sum_i\max(a_i-1,0),\qquad
s(a)=\sum_i\max(-a_i,0).
\]

The current RustRed hardness tuple is

```text
(active propagators, r+s, r, sector mask, exponent vector)
```

and exact elimination pivots the maximum.  This is implemented at
[`src/family.rs:523-552`](../../src/family.rs#L523).  It gives the desired
“more lines, then more total displacement, then more dots” triangular
orientation.  Keep it for the first complete reduction; changing an order and
changing the recurrence search simultaneously would make failures hard to
diagnose.

### 5.2 Finite target boxes: an exact a-posteriori sufficiency test

`SeedConfig` currently bounds total dots and total numerator degree and can
include all subsectors
([`src/reduction.rs:8-25`](../../src/reduction.rs#L8)); enumeration removes
symmetry duplicates and sorts by the family order
([`src/reduction.rs:77-135`](../../src/reduction.rs#L77)).  Use total-degree
shells, not a componentwise cube.

For a declared finite target set

\[
\mathcal T(D,N)=\{\text{canonical nonzero }a:r(a)\leq D, s(a)\leq N\},
\]

a generated seed set is **certifiably sufficient for that target set** if:

1. every emitted rule carries exact row provenance showing it is a linear
   combination of generated nine-IBP equations and proved symmetry/boundary
   identities;
2. every target in \(\mathcal T(D,N)\) reduces without a cycle to only the five
   selected outputs;
3. substitution of the completed table annihilates every generated identity;
4. all coefficient denominators and integer-domain guards are recorded, rather
   than specialized away.

This is an a-posteriori theorem: any shell satisfying the four checks is enough
for the advertised finite box, even if it is smaller than a heuristic bound.
Conversely, “one IBP halo” or an unchanged terminal set at two successive
depths is not a proof of coverage.

A practical discovery schedule is

```text
(D,N) = (0,0), (1,0), (1,1), (2,1), (2,2), (3,2), (3,3), ...
```

solved one genuine sector at a time.  After each successful shell, reduce the
next shell as a held-out target set before generating rows from it.  If any
held-out integral remains outside the proposed basis, enlarge only the affected
sector and its immediate dependency halo.  This schedule is **heuristic**, not
a claimed universal seed bound.

The existing corner-only three-loop test explicitly warns that its six seeds
and 54 identities are exploratory and not a master proof
([`crates/rustred-legacy-oracles/tests/three_loop_family.rs:289-316`](../../crates/rustred-legacy-oracles/tests/three_loop_family.rs#L289)).
It must not be promoted to milestone completion evidence.

### 5.3 All integer indices require guarded parametric recurrences

No finite seed box proves a reduction for arbitrarily large dots or numerator
powers.  Complete all-index coverage requires a finite set of symbolic rules
whose guards partition

\[
a_i\geq1\quad\text{for active lines},\qquad
a_i\leq0\quad\text{for inactive lines},
\]

and whose right-hand sides are strictly lower in the fixed order.  For each
rule, store:

- the left-side Symbolica pattern;
- integer inequalities on matched indices;
- nonzero polynomial conditions on \(d,m^2\) and indices;
- the exact sparse right-hand side;
- provenance back to the nine generic IBPs;
- a machine-checkable proof that every guarded RHS integral is lower.

LiteRed2 follows precisely this broad strategy.  It declares positive and
nonpositive sector domains
([`LiteRed2026.m:2384-2387`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2384)),
searches increasing nearby-point depth, patternizes pivots, rejects invalid
coefficient/domain cases, and records uncovered points as masters
([`LiteRed2026.m:2470-2520`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2470)).
Its IBPs are generated as index-shift functions
([`LiteRed2026.m:1813-1823`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1813)),
and its final reducer applies lower-sector dependencies before within-sector
layers
([`LiteRed2026.m:3951-4003`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3951)).
RustRed should reproduce the mathematical invariants, not translate or execute
the Mathematica code.

Suggested recurrence-discovery procedure:

1. generate equations with symbolic indices in one canonical sector;
2. specialize enough nearby integer points to discover a stable pivot shape;
3. interpolate/reconstruct the candidate rational coefficient functions;
4. verify the candidate by exact symbolic substitution into the generic IBPs;
5. derive the maximal integer guard on which the pivot coefficient is nonzero
   and every RHS is lower;
6. subtract that guard from the uncovered domain and repeat;
7. accept the sector only when a Presburger-style domain check says that the
   rules plus named master points cover the whole sector.

The factorized boundary formulae should remain direct algorithms rather than be
rediscovered as large recurrence tables.

### 5.4 Proved six-line scalar-dot descent

One guarded rule is now known exactly. For an all-positive input

\[
 A=I(a_1,\ldots,a_6),\qquad a_1>1,
\]

set `s=A-e1` and let \(E_{ij}(s)\) be RustRed's native raw identity
\(\partial_{k_i}\cdot k_j\) at that seed. The fixed rational matrix

\[
C=\begin{pmatrix}
3/4&-1&1/2\\
3/2&-1/4&-1\\
0&1/2&-1/4
\end{pmatrix}
\]

gives a relation whose coefficient of \(A\) is
\((a_1-1)m^2\). The other raw coefficients carry the compensating factor
`1/2`, so the explicit 17-term RHS is conventionally written with the common
denominator \(2(a_1-1)m^2\). Every term which
remains in the six-line sector has total dot degree one lower; a term which
pinches a line is lower by active-propagator count. Thus every RHS integral is
strictly lower in RustRed's Laporta order. The denominator has no
dimension-dependent exceptional factor, under the generic assumption
`m2 != 0`.

The complete formula and its raw-IBP provenance are implemented in the
`three_loop_top_dot` module. `S4` edge transitivity ensures that the
lexicographically canonical representative of any dotted scalar top integral
has `a1 > 1`; `a1 = 1` is the undotted `M6` corner. For example,

\[
 I(2,1,1,1,1,1)=\frac{4-d}{4m^2}I(1,1,1,1,1,1).
\]

At higher dot degree this step can emit dotted five-line genuine sectors. For
example, the canonical target `(2,2,1,1,1,1)` contains three such branches in
addition to a lower top term. The F5 step below descends five-line branches,
while any dotted B4 branch remains an explicit obstruction; numerator powers
are a separate proof obligation.

### 5.5 Proved F5 scalar-dot descent; B4 obstruction

The initially proposed diagonal rows are not descending: `E11(A-e1)` and
`E22(A-e2)` each contain a same-total-dot transfer between active edges. Since
such transfers form cycles on a fixed-degree symmetry orbit, lexicographic
orientation cannot turn either raw row into a global recurrence.

F5 nevertheless has exact fixed combinations which cancel every transfer. For
the singleton central-edge orbit, orient the dot to `D1`, seed at `A-e1`, and
weight the nine native rows by

\[
C_{\rm central}=\begin{pmatrix}3&0&0\\2&-1&0\\2&0&-1\end{pmatrix}.
\]

The pivot is `6*(a1-1)*m2*A`. For the four outer edges, orient to `D2`, seed at
`A-e2`, and use `E21+E22`; its pivot is `3*(a2-1)*m2*A`. Before coefficient
construction, the implementation reserves the exact worst-case 41 raw terms:
38 derivative plus three diagonal dimension terms (15 total for the outer
branch). It validates every selected shift coefficient-free. In both cases
every surviving F5 term has
strictly smaller total dot degree and every pinch has fewer active lines. The
integer guard is `a_p>1`, with generic `m2 != 0`.

B4 is different. At one lowered B4 seed, require a constant contraction of
the pivot line, zero constant contractions for the other three active lines,
and zero coefficients of both inactive denominators in every active-line
contraction. These are precisely the conditions which forbid same-dot
transfers and numerator creation. Exact Gaussian elimination gives rank nine
for the nine raw-row weights, hence nullity zero: no nonzero one-seed scalar
combination can isolate a B4 pivot under those requirements. This is a no-go
for the local ansatz, not a proof that a larger shell recurrence is impossible.

The `three_loop_proper_dot` module stores the lowered position and complete 3x3
raw-row weight matrix as provenance. Its consolidated test replays both F5
combinations against native raw IBPs, exhausts powers one through three on all
five active F5 lines, and checks asymmetric and high-index points. Undotted B4
is retained as a terminal, but a dotted B4 input returns the precise
`UnsupportedDottedB4` error. Consequently the top and F5 rules do **not** yet
prove unrestricted all-dot scalar closure: a bounded multi-seed B4 shell (or a
different analytic identity) remains required.

## 6. Independent validation oracles

Milestone acceptance should use all of the following, with at least one oracle
that did not generate the reduction.

### 6.1 Exact internal certificates

- Re-run the inverse-basis, 24-symmetry, 64-sector, and nine-IBP invariants
  already encoded in the three-loop foundation test.
- Validate every reduction rule against its exact row provenance.
- Reduce every generated and held-out IBP equation to literal zero.
- Verify strict order descent, acyclicity, and termination for every guarded
  recurrence.
- Check mass homogeneity term by term.
- At several rational specializations of \(d,m^2\), avoiding recorded poles,
  repeat matrix/rule checks modulo several large primes.  This is a fast
  independent implementation check; the symbolic check remains authoritative.
- For every factorization and sector map, multiply the integer momentum matrices
  and verify determinant \(\pm1\) and exact denominator images.

### 6.2 Frozen Vakint/alphaLoop evidence, without running FORM

- Compare the final five topology classes with the static list at
  [`integrateduv.frm:1170-1187`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm#L1170).
- Translate a selected set of the static recurrence identities into the
  positive-Euclidean convention once, by hand or by a small pure-Rust text
  importer, and verify them against RustRed's output.  Do not execute FORM and
  do not make the importer part of the runtime reducer.
- Vakint has frozen analytic epsilon values for the six-line scalar integral
  ([`integral_evaluation_analytic_tests.rs:354-385`](../../vendor/gammaloop/crates/vakint/tests/integral_evaluation_analytic_tests.rs#L354))
  and rank-four tensor examples
  ([`integral_evaluation_analytic_tests.rs:387-423`](../../vendor/gammaloop/crates/vakint/tests/integral_evaluation_analytic_tests.rs#L387)).
  After applying the explicitly documented sign, edge-order, loop-measure, and
  MS-bar conversion, these are external end-to-end numerical oracles.
- Vakint also records alphaLoop-versus-MATAD comparisons for the four-line
  basketball, the scalar tetrahedron, and rank-four numerators
  ([`integral_alphaloop_vs_matad_tests.rs:103-218`](../../vendor/gammaloop/crates/vakint/tests/integral_alphaloop_vs_matad_tests.rs#L103)).
  They establish useful historical agreement, but RustRed should compare with
  frozen values rather than run either backend.
- Include canonical contraction inputs such as Vakint's four-line pinched I3L
  mapping
  ([`input_matching_tests.rs:235-262`](../../vendor/gammaloop/crates/vakint/tests/input_matching_tests.rs#L235)).

### 6.3 A backend-independent Euclidean numerical oracle

Add a pure-Rust numerical test at convergent parameter values.  Two useful
routes are:

1. set integer \(d=1\), choose powers/numerators with sufficient falloff, map
   each real loop momentum to a finite interval, and perform direct three-fold
   adaptive quadrature;
2. for positive powers use Schwinger parameters.  With
   \(A=\sum_i a_i\), \(Q_i\) the routing rows, and
   \(U(x)=\det\sum_i x_i Q_iQ_i^T\),

   \[
   I(a)=\frac{\pi^{3d/2}\Gamma(A-3d/2)}{\prod_i\Gamma(a_i)}
   (m^2)^{3d/2-A}
   \int_{x_i\geq0,\ \sum x_i=1}
   \frac{\prod_i x_i^{a_i-1}}{U(x)^{d/2}}\,d\sigma.
   \tag{9}
   \]

Evaluate both sides of randomly selected reduction rules away from UV/IR and
coefficient poles.  Equation (9) is derived directly from Gaussian integration
and shares neither the IBP solver nor the Vakint normalization path.

For tensors, first test RustRed's FORM-free tensor projection and denominator
lowering independently, then compare the complete projected-and-reduced value.
Vakint's MATAD path itself requires tensor reduction before evaluation
([`matad.rs:386-404`](../../vendor/gammaloop/crates/vakint/src/matad.rs#L386)),
which confirms that this seam must be owned by RustRed.

## 7. Master-count proof obligation

The five-master claim needs both directions.

**Upper bound.**  Produce guarded recurrences whose union covers every integer
point of masks 43, 31, and 63 and reduces to \(B_4,F_5,M_6\) plus already
closed boundaries.  Symbolically verify every recurrence and its domain.  This
proves that at most five masters are needed.

**Lower bound.**  Independently count the generic IBP quotient.  The preferred
pure-Rust route is a Lee-Pomeransky critical-ideal calculation per sector:
construct the sector polynomial, saturate away coordinate and graph-polynomial
zeros, compute an exact Groebner basis with Symbolica's polynomial layer, and
count the zero-dimensional quotient.  Record the genericity assumptions and
cross-check the count at several finite-field specializations.  Alternatively,
an exact symbolic IBP-module rank certificate with explicit nonzero minors is
acceptable.  Finite-shell stabilization alone is not a lower-bound proof.

LiteRed2 treats uncovered integrals as masters and can optionally ask Mint for a
critical-point count; its source shows the expected-count hook at
[`LiteRed2026.m:2368-2377`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2368).
RustRed must implement an equivalent proof path in pure Rust/Symbolica rather
than depend on Mint or Mathematica.

## 8. Implementation and acceptance sequence

1. Implement a `ThreeLoopBoundaryReducer` for masks 7, 11, and 15 using
   (1)-(8), per-component tensor moments, memoized tadpole windows, and the
   existing two-loop pipeline.
2. Add explicit stable output masters and boundary equality rules; do not encode
   the star/path equality as an \(S_4\) permutation.
3. Add sector-by-sector seed generation so boundary rows never enter the large
   sparse solve.
4. Establish certified finite boxes for masks 43, 31, and 63, with row
   provenance and held-out-shell tests.
5. Discover and symbolically verify guarded parametric recurrences.
6. Prove whole-domain guard coverage and the generic five-dimensional quotient.
7. Add the pure-Rust numerical oracle and frozen Vakint normalization adapter
   comparisons, including scalar and tensor inputs.
8. Only then advertise “complete three-loop equal-mass reduction.”  Before
   steps 5-6, advertise the exact finite box explicitly.

## 9. Explicit unproved claims and non-claims

- **Unproved:** exactly one scalar IBP master is needed in each of masks 43, 31,
  and 63 for generic \(d\).
- **Unproved:** the five proposed masters are linearly independent over
  \(\mathbb Q(d,m^2)\).
- **Unproved:** any particular numeric `(max_dots, max_numerator_degree)` shell
  suffices beyond the finite targets certified from it.
- **Unproved:** the current finite `SparseReducer` can discover all required
  guarded recurrences without a dedicated parametric layer.
- **Not claimed:** alphaLoop's `uvid` tuples have RustRed's edge order,
  propagator sign, loop measure, or epsilon normalization.
- **Not allowed:** calling FORM, MATAD, or Mathematica as part of generation,
  validation, or runtime.  Their checked-in text and frozen numerical results
  are reference oracles only.
- **Proved:** the zero-sector census, all \(S_4\) maps currently registered, both
  tree factorizations, and the paw's terminating arbitrary-numerator
  factorization (5)-(8) into lower-loop integrals.  All-index reduction of the
  resulting sunset integrals remains a separate proof obligation.
