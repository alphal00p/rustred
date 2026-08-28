# Exact numerator boundaries for the three-loop tetrahedron

## Purpose and result

This note gives a finite, exact, pure-Rust/Symbolica algorithm for every
integer-index integral in the factorized boundary orbits of RustRed's
equal-mass three-loop tetrahedron:

- the star tree, canonical sector mask `7`;
- the path tree, canonical sector mask `11`;
- the triangle-with-a-leaf (paw), canonical sector mask `15`.

In all three cases every active index is positive and every inactive index may
be zero or negative.  A negative index is an arbitrary polynomial numerator,
not a new denominator.  Subject only to explicit operational resource limits,
the algorithms terminate and return only

\[
 P_3=I(1,1,1,0,0,0)=T_1^3,
 \qquad
 S_T=I(1,1,1,1,0,0)=T_1S_{111}.
\]

No IBP search is needed inside a tree boundary.  A paw boundary uses a finite
angular/polynomial calculation for its bridge loop and then the already
certified two-loop pipeline.  No FORM or Mathematica process is involved.

The derivation was checked against the local RustRed, LiteRed2, and Vakint
sources as text.  In particular:

- the tetrahedron routing and its proved 24-element edge action are in
  [`crates/rustred-legacy-oracles/src/three_loop.rs`](../../crates/rustred-legacy-oracles/src/three_loop.rs);
- the factorized scalar numerator boundary reducer is in
  [`crates/rustred-legacy-oracles/src/three_loop_boundary.rs`](../../crates/rustred-legacy-oracles/src/three_loop_boundary.rs);
- the arbitrary-numerator two-loop boundary formula and its overflow-safe
  coefficient recurrences are in [`crates/rustred-legacy-oracles/src/two_loop.rs`](../../crates/rustred-legacy-oracles/src/two_loop.rs);
- the exact global tensor projector and scalar-product lowering bridge are in
  [`src/tensor.rs`](../../src/tensor.rs) and
  [`src/tensor_family.rs`](../../src/tensor_family.rs);
- LiteRed2's `DAverage` implements the same componentwise isotropic pairing
  recursion in [`Vectors.m:617-640`](../../vendor/LiteRed2/Source/RNL/Vectors.m#L617);
- Vakint registers precisely the same six I3L momenta in
  [`topologies.rs:76-98`](../../vendor/gammaloop/crates/vakint/src/topologies.rs#L76),
  but its MATAD path is only a convention oracle and must not be invoked.

The formulas below use generic symbolic dimension `d` and nonzero equal mass
parameter `m2`.  They are identities of rational functions over
`Q(d,m2)`; isolated special dimensions at which a displayed angular
denominator vanishes are not a separate reduction problem.

## 1. Convention and the three canonical boundaries

RustRed defines

\[
 I(a_1,\ldots,a_6)=
 \int d^d k_1d^d k_2d^d k_3\prod_{i=1}^6D_i^{-a_i}
\]

with

\[
\begin{array}{lll}
D_1=k_1^2+m^2,&D_2=k_2^2+m^2,&D_3=k_3^2+m^2,\\
D_4=(k_3-k_1)^2+m^2,&D_5=(k_1-k_2)^2+m^2,
&D_6=(k_2-k_3)^2+m^2,
\end{array}
\]

where the code symbol `m2` is \(m^2\).  All propagators have positive
Euclidean sign.  There is no ISP because the six denominators form the full
six-dimensional scalar-product basis at three loops.

After full `S4` edge canonicalization, the factorized cases are:

| mask | powers | active factorization | inactive numerator lines |
|---:|---|---|---|
| 7 | `(a,b,c,-r4,-r5,-r6)` | three independent tadpoles | `D4,D5,D6` |
| 11 | `(a,b,-r3,e,-r5,-r6)` | three independent tadpoles | `D3,D5,D6` |
| 15 | `(a,b,c,e,-r5,-r6)` | bridge tadpole times sunset | `D5,D6` |

Here every displayed active power is positive and every \(r_i\geq0\).
Disconnected active sectors are scaleless even in the presence of polynomial
inactive-line numerators, because at least one loop direction remains a
polynomial integral without a denominator.

The checked-in boundary implementation accepts arbitrary nonnegative
\(r_i\) in these three masks, subject to its explicit numerator, polynomial,
angular, tadpole-recurrence, and Symbolica exponent limits. It performs the
sector-first canonicalization described below and returns `Ok(None)` for the
genuine masks 43, 31, and 63.

## 2. Symmetry canonicalization and the missing momentum witness

### 2.1 Scalar denominator numerators need only the edge permutation

`VacuumFamily::canonicalize` applies every symmetry as

```text
canonical[target] = input[permutation[target]]
```

and chooses the lexicographically greatest power vector
([`src/family.rs:526-547`](../../src/family.rs#L526)).  The edge permutations
are enough to transport a scalar boundary numerator, but this ordinary
full-vector ordering is **not** the right way to choose its factorization
representative.  An executed test with unequal active dots showed that full
lexicographic maximization can select a labelled tree or paw mask different
from the canonical Boolean-sector masks 7, 11, and 15.  Under the proved
unit-Jacobian map,

\[
 D_{\mathrm{source}}(k_{\rm old})
 =D_{\mathrm{target}}(k_{\rm new}),
\]

so a negative power is transported by exactly the same exponent permutation
as a positive power.  Once every scalar product has been lowered to the
complete denominator basis, no separate loop-momentum map is needed.  The
correct initial implementation is therefore a sector-first canonicalization:

1. validate arity;
2. return zero if `family.is_scaleless(input)`;
3. apply every symmetry to the Boolean vector `power > 0` and choose its
   lexicographically greatest image;
4. retain only symmetries producing that Boolean image, then maximize the full
   signed exponent vector within this sector stabilizer;
5. inspect that sector-first canonical mask and dispatch 7, 11, or 15,
   including all canonical negative powers;
6. return `Ok(None)` for a nonfactorized mask.

This is the behavior of the checked-in `canonicalize_sector_first` helper.  It
preserves the independently enumerated sector representatives, then uses dots
and numerator powers only as the tie-break inside a fixed representative.

### 2.2 Explicit tensor vectors do need a winning map

The situation changes if a tensor numerator still contains explicit
\(k_i^\mu\).  The current family constructor proves a geometric symmetry by
finding an exact transformation with determinant \(\pm1\), but discards it
([`src/family.rs:792-867`](../../src/family.rs#L792)).  The scalar
`canonicalize` result also does not say which symmetry won.  Reconstructing the
map after the fact is ambiguous when the power vector has a stabilizer.

For a future direct tensor-boundary fast path, store a witness for every proved
symmetry:

```rust
pub struct SymmetryWitness {
    /// target denominator -> source denominator
    pub permutation: Vec<usize>,
    /// old loop column = loop_map * canonical loop column
    pub loop_map: Vec<Vec<ExactRational>>,
}

pub struct CanonicalizationWitness {
    pub integral: Integral,
    pub symmetry: SymmetryWitness,
}
```

`canonicalize_with_witness` should first maximize the transformed power vector,
then break any tie by the lexicographically smallest denominator permutation
and flattened loop matrix.  The ordinary `canonicalize` can delegate to it and
discard the witness, preserving its existing scalar result.

The identity symmetry needs an explicit identity matrix rather than the current
validator's early Boolean success.  For general families, refactor
`symmetry_is_geometric` into a function that returns the lexicographically
smallest exact transformation it currently searches for.

### 2.3 Closed tetrahedron witness formula

There is also a simple independent oracle for all 24 tetrahedron witnesses.
Let vertex coordinates be

\[
 x_0=0,\qquad x_i=k_i\quad(i=1,2,3),
\]

and let a vertex permutation \(\pi\in S_4\) induce the edge permutation

\[
 p(t)=\text{edge index of }\{\pi(u_t),\pi(v_t)\}.
\]

This has the same `target -> source` convention used by `VacuumFamily`.  Put
\(w=\pi^{-1}(0)\), define new coordinates \(y_0=0,y_i=\kappa_i\), and set

\[
 x_{\pi(v)}=y_v-y_w.
\]

Then for old loop label \(j=1,2,3\), with \(v=\pi^{-1}(j)\),

\[
 k_j^{\rm old}=\kappa_v-\kappa_w,
 \qquad
 T_{ji}=\delta_{i,v}-\delta_{i,w}\quad(i=1,2,3),
\tag{2.1}
\]

where a delta with index zero is omitted.  This integer matrix has determinant
\(\pm1\), and every routed edge obeys

\[
 q_{p(t)}(k^{\rm old})=\pm q_t(\kappa).
\]

For example, \(\pi=(0\,1)\) gives

\[
(k_1,k_2,k_3)_{\rm old}
=(-\kappa_1,\kappa_2-\kappa_1,\kappa_3-\kappa_1),
\]

exactly the first documented generator in
[`crates/rustred-legacy-oracles/src/three_loop.rs:28-45`](../../crates/rustred-legacy-oracles/src/three_loop.rs#L28).  Enumerating all 24
vertex permutations, deriving (2.1), and checking all six routed squares is a
small test independent of the generic matrix search.

LiteRed2 likewise keeps momentum substitutions as first-class symmetry data:
its `FindShifts` constructs a loop-momentum matrix and solves denominator
matching equations in
[`LiteRed2026.m:3142-3229`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3142).

## 3. Shared one-loop radial engine

All componentwise algorithms should share one normalized tadpole engine.  Set

\[
 T_n=\int d^dp\,(p^2+m^2)^{-n},\qquad \tau_n=T_n/T_1.
\]

For positive integer \(n\),

\[
 \tau_1=1,\qquad
 \tau_{n+1}=\frac{2n-d}{2n\,m^2}\tau_n.
\tag{3.1}
\]

For \(n\leq0\), \(T_n=0\) as a scaleless polynomial integral.  Define the
normalized radial moment

\[
 R(n,s)=\frac1{T_1}\int d^dp\,
 \frac{(p^2)^s}{(p^2+m^2)^n},\qquad s\in\mathbb Z_{\geq0}.
\]

Since \(p^2=D-m^2\),

\[
 R(n,s)=
 \sum_{j=0}^{\min(s,n-1)}
 {s\choose j}(-m^2)^{s-j}\tau_{n-j},
\tag{3.2}
\]

with an empty sum when \(n\leq0\).  This definition handles a numerator that
cancels the bridge propagator without special cases.

Implement a per-reduction `TadpoleCache` that lazily extends a vector of
\(\tau_n\), plus a `BTreeMap<(i32,u32), Coefficient>` for repeated `R(n,s)`
queries.  A paw expansion sends many terms to the same radial moment, so this
cache is material.  The recurrence and binomial coefficients must be advanced
by exact neighboring ratios, as the existing two-loop code does, instead of
forming machine factorials.

For common use by two and three loops, the private `TadpoleWindow`,
`next_tadpole_ratio`, `binomial_row`, and `multiply_integer_ratio` machinery in
[`crates/rustred-legacy-oracles/src/two_loop.rs`](../../crates/rustred-legacy-oracles/src/two_loop.rs) should move to a small
`pub(crate)` module.  Keep the sign convention explicit: equations (3.1)-(3.2)
are for `D=p^2+m2`, while the existing two-loop reducer internally names
`s=-m2` for `D=p^2-s`.

Useful unit oracles are

\[
 R(1,0)=1,\quad R(1,s)=(-m^2)^s,\quad
 R(2,0)=\frac{2-d}{2m^2}.
\tag{3.3}
\]

## 4. Componentwise angular engine

For a radial function \(f(p^2)\),

\[
 \int p^{\mu_1}\cdots p^{\mu_{2r}}f(p^2)d^dp
 =\frac{\sum_{\text{pairings}}g^{\mu_i\mu_j}\cdots}
 {H_r(d)}
 \int(p^2)^r f(p^2)d^dp,
\tag{4.1}
\]

where

\[
 H_r(d)=\prod_{j=0}^{r-1}(d+2j),\qquad H_0=1,
\tag{4.2}
\]

and every odd moment is zero.  LiteRed2's textual `DAverage` reference uses
the same recursion: pair two occurrences, multiply by the associated scalar
product or metric, and advance the denominator from `d` to `d+2`, etc.

For the scalar boundary algorithms, never enumerate all \((2r-1)!!\)
pairings.  If the eliminated vector occurs through
\((A\cdot p)^a(B\cdot p)^b\), let \(a+b=2r\).  If `z` pairings connect an
`A` occurrence to a `B` occurrence, the exact multiplicity is

\[
 N(a,b,z)=
 \frac{a!b!}
 {z!\,2^{r-z}\,((a-z)/2)!\,((b-z)/2)!},
\tag{4.3}
\]

where

\[
0\leq z\leq\min(a,b),\qquad a-z\equiv b-z\equiv0\pmod2.
\]

Thus

\[
\begin{aligned}
\langle(A\cdot p)^a(B\cdot p)^b\rangle_p
={}&\frac{(p^2)^r}{H_r(d)}
\sum_z N(a,b,z)
(A^2)^{(a-z)/2}(B^2)^{(b-z)/2}(A\cdot B)^z.
\end{aligned}
\tag{4.4}
\]

The multiplicities can be generated without factorials.  Start at `z=0` for
even `a,b` or `z=1` for odd `a,b`; if the parities differ the moment vanishes.
The next allowed value is `z+2`, with

\[
 \frac{N(a,b,z+2)}{N(a,b,z)}
 =\frac{(a-z)(b-z)}{(z+1)(z+2)}.
\tag{4.5}
\]

The starting value is `(a-1)!!(b-1)!!` at `z=0`, or `a!! b!!` at `z=1`.
Build these products directly in `Coefficient`, not in `usize`.

When only \((A\cdot p)^{2t}\) remains, (4.4) reduces to

\[
 \langle(A\cdot p)^{2t}\rangle_p
 =\frac{(2t-1)!!}{H_t(d)}(A^2p^2)^t.
\tag{4.6}
\]

Cache `1/H_r(d)` recursively.  These scalar count formulas are much smaller
than the dense global tensor Gram projector and are the main boundary-speed
win.

## 5. Complete tree-sector algorithm

### 5.1 Fixed factorized bases

For mask 7 choose

\[
 p_1=k_1,\qquad p_2=k_2,\qquad p_3=k_3.
\]

The inactive routed momenta are

\[
 p_3-p_1,\qquad p_1-p_2,\qquad p_2-p_3.
\tag{5.1}
\]

For mask 11 choose

\[
 p_1=k_1,\qquad p_2=k_2,\qquad p_3=k_3-k_1.
\]

The inactive routed momenta are

\[
 p_1+p_3,\qquad p_1-p_2,\qquad p_2-p_1-p_3.
\tag{5.2}
\]

Both changes have unit determinant, and the active denominator product is
\(D(p_1)^aD(p_2)^bD(p_3)^c\).  Rather than hard-code (5.1)-(5.2), an
implementation may form the three active routing rows, invert that exact
unimodular matrix, and express every inactive routing in the active-edge
basis.  Assert determinant \(\pm1\) and integer output for these graph trees.
In this section `a,b,c` always mean the powers ordered with `p1,p2,p3`; for
mask 11, this section's `c` is the original fourth entry `e`, not the inactive
third entry.

### 5.2 Sparse Gram polynomial

Use the six-key monomial

```text
[n11, n22, n33, n12, n13, n23]
```

for

\[
x_{11}^{n_{11}}x_{22}^{n_{22}}x_{33}^{n_{33}}
x_{12}^{n_{12}}x_{13}^{n_{13}}x_{23}^{n_{23}},
\qquad x_{ij}=p_i\cdot p_j.
\]

Store a sparse `BTreeMap<[u32;6], Coefficient>`.  For an inactive route
\(q=c_1p_1+c_2p_2+c_3p_3\), multiply by

\[
 D_q=m^2+\sum_i c_i^2x_{ii}
       +2\sum_{i<j}c_ic_jx_{ij}
\tag{5.3}
\]

once for each unit of its numerator power.  Merge identical keys after every
multiplication and remove zero coefficients.  Do all combinatorics in Rust;
only coefficients and their exact simplification belong to Symbolica.

### 5.3 Integrate `p3`, then `p2`, then `p1`

For one polynomial monomial set

\[
A=n_{13},\qquad B=n_{23},\qquad r=(A+B)/2.
\]

If `A+B` is odd, the monomial vanishes.  For each allowed `z` in (4.3), after
the `p3` angular average define

\[
\begin{aligned}
n'_{11}&=n_{11}+(A-z)/2,\\
n'_{22}&=n_{22}+(B-z)/2,\\
n'_{12}&=n_{12}+z.
\end{aligned}
\tag{5.4}
\]

The normalized `p3` radial factor is

\[
 R(c,n_{33}+r),
\]

and the angular factor is \(N(A,B,z)/H_r(d)\).

If \(n'_{12}\) is odd, the remaining `p2` average vanishes.  Otherwise put
\(t=n'_{12}/2\).  Equations (4.6) and (3.2) give the complete contribution to
the coefficient of `P3`:

\[
\boxed{
 c_{\rm poly}
 \frac{N(A,B,z)}{H_r(d)}
 \frac{(2t-1)!!}{H_t(d)}
 R(c,n_{33}+r)
 R(b,n'_{22}+t)
 R(a,n'_{11}+t)
 }.
\tag{5.5}
\]

Sum (5.5) over polynomial monomials and `z`.  No scalar integrals other than
`P3` remain.  The proof is constructive: the polynomial expansion is finite,
both angular sums are finite, and every radial moment is the finite sum (3.2).

### 5.4 Tree goldens

The following exact results exercise successively a constant/odd average, a
rank-two angular average, and a genuinely three-vector contraction:

\[
\begin{aligned}
I(1,1,1,-1,0,0)&=-m^2P_3,\\
I(1,1,1,-2,0,0)&=(m^2)^2\frac{d+4}{d}P_3,\\
I(1,1,1,-1,-1,-1)&=(m^2)^3
\left(\frac8{d^2}-1\right)P_3.
\end{aligned}
\tag{5.6}
\]

For the last line, the only all-cross term is
\(-8(p_1\cdot p_2)(p_1\cdot p_3)(p_2\cdot p_3)\); its angular average is
\((p_1^2p_2^2p_3^2)/d^2\).  Since `R(1,1)=-m2`, it contributes
`+8*m2^3/d^2`, while the radial part contributes `-m2^3`.

The corresponding path goldens begin with

\[
 I(1,1,-1,1,0,0)=-m^2P_3,
 \qquad
 I(1,1,-2,1,0,0)=(m^2)^2\frac{d+4}{d}P_3.
\tag{5.7}
\]

These are factorization checks across two different `S4` sector orbits, not
claims that a star and a path are graph symmetries.

## 6. Complete paw-sector algorithm

### 6.1 Isolate the bridge loop

For canonical mask 15 use

\[
u=k_1,\qquad p=k_2,\qquad v=k_3.
\]

The active denominators are `D1,D2,D3,D4`; `D2` is the independent bridge
tadpole.  The two inactive numerators are

\[
\begin{aligned}
D_5&=D_1+D_2-m^2-2u\cdot p,\\
D_6&=D_2+D_3-m^2-2v\cdot p.
\end{aligned}
\tag{6.1}
\]

For input `(a,b,c,e,-r5,-r6)`, expand the finite polynomial
`D5^r5 D6^r6` into sparse monomials

\[
C\,D_1^{h_1}D_2^{h_2}D_3^{h_3}
(u\cdot p)^\alpha(v\cdot p)^\beta.
\tag{6.2}
\]

Use a `BTreeMap<[u32;5], Coefficient>` with key
`[h1,h2,h3,alpha,beta]`.  Each multiplication has only four affine terms.

### 6.2 Average and integrate the bridge

If \(\alpha+\beta\) is odd, discard (6.2).  Otherwise put
\(r=(\alpha+\beta)/2\).  For every allowed `z` in (4.3), define

\[
A=(\alpha-z)/2,\qquad B=(\beta-z)/2.
\]

The `p` integration produces the coefficient

\[
 C\,\frac{N(\alpha,\beta,z)}{H_r(d)}R(b-h_2,r)
\tag{6.3}
\]

and leaves the two-loop numerator

\[
D_1^{h_1}D_3^{h_3}
(D_1-m^2)^A(D_3-m^2)^B
\left(\frac{D_1+D_3-D_4-m^2}{2}\right)^z.
\tag{6.4}

Equation (6.4) used

\[
u^2=D_1-m^2,\quad v^2=D_3-m^2,\quad
u\cdot v=(D_1+D_3-D_4-m^2)/2.
\]

Expand (6.4) in a three-key sparse denominator-shift polynomial.  A term with
shift `(s1,s3,s4)` contributes the standard two-loop integral

\[
 S(a-s_1,c-s_3,e-s_4).
\tag{6.5}
\]

Accumulate all terms into one `LinearCombination` before calling the two-loop
pipeline.  This merges duplicate shifted integrals and permits a reduction
cache to work.  The map to the built-in two-loop routing is `q=-v`:

\[
u^2+m^2,\quad q^2+m^2,\quad(u+q)^2+m^2=D_4.
\]

It has unit Jacobian and introduces no sign because all remaining expressions
have already been written in `D1,D3,D4`.

The two-loop pipeline must be built with

```text
max_numerator_degree >= configured three-loop boundary numerator degree
```

rather than zero. The checked-in constructor forwards this configured bound to
its nested two-loop pipeline.
The shift degree in (6.4) is

\[
h_1+h_3+A+B+z=h_1+h_3+r\leq r_5+r_6,
\tag{6.6}
\]

so this bound is sufficient.  Polynomial multiplication only lowers two-loop
indices; it cannot increase their dot degree.  Therefore the configured
two-loop dot coverage need only cover the original `(a,c,e)` dot degree (plus
whatever halo the enclosing three-loop certificate explicitly requests).

### 6.3 Map the two-loop output

The bridge radial moment (6.3) was normalized by `T1`.  Consequently map the
two allowed two-loop masters as

```text
S(1,1,1) -> ST
P=I2(0,1,1)=T1^2 -> P3
```

without an extra tadpole factor.  In the scalar `r5=r6=0` case,
`R(b,0)=tau_b`, so this exactly reproduces the current implementation's bridge
tadpole ratio.

Any other two-loop terminal remains a typed `UnexpectedTwoLoopMaster` error.

### 6.4 Paw goldens

Two useful exact results are

\[
 I(1,1,1,1,-1,0)=P_3-m^2S_T,
\tag{6.7}
\]

and the mixed-numerator rank-two check

\[
\boxed{
 I(1,1,1,1,-1,-1)
 =(m^2)^2\frac{d+2}{d}S_T
 -2m^2\frac{d+1}{d}P_3.
 }
\tag{6.8}
\]

For (6.8), bridge integration gives

\[
(D_1-m^2)(D_3-m^2)
-\frac{4m^2}{d}(u\cdot v).
\]

The first term reduces to `(m2^2)*S - 2*m2*P`, while
\(u\cdot v\) reduces to `(P-m2*S)/2`, proving the displayed coefficients.
This checks the sign in (6.1), the `1/d` angular factor, the inverse
scalar-product map, and both output masters at once.

## 7. Tensor reduction outside FORM

### 7.1 Safe first implementation: global projection, then scalar boundary

The existing pure-Rust path is already mathematically complete for tensor
numerators within its configured rank and expansion limits:

```text
TensorMonomial
  -> VacuumTensorProjector::reduce
  -> TensorFamilyReducer::lower
  -> ThreeLoopBoundaryReducer::reduce_combination
```

The global projector is valid before factorization because it uses only the
global vacuum `O(d)` invariance and solves the exact metric-pairing Gram matrix
([`src/tensor.rs:658-900`](../../src/tensor.rs#L658)).  The lowering bridge
then expresses every scalar product in the complete six-denominator basis
([`src/tensor_family.rs:131-238`](../../src/tensor_family.rs#L131)).  At that
point all numerator information is encoded in signed powers, so scalar `S4`
canonicalization needs no momentum witness.

This composition should be the acceptance path for the numerator-boundary
milestone.  It is pure Rust/Symbolica and does not depend on Vakint's FORM tensor
path.  Vakint's source itself requires tensor reduction before its MATAD
adapter and applies additional convention signs; neither behavior should leak
into RustRed.

Small tensor integration checks are:

\[
\begin{aligned}
\int\frac{k_1^\mu k_1^\nu}{D_1D_2D_3}
&=-\frac{m^2}{d}g^{\mu\nu}P_3,\\
\int\frac{k_1^\mu k_2^\nu}{D_1D_2D_3}&=0,\\
\int\frac{k_2^\mu k_2^\nu}{D_1D_2D_3D_4}
&=-\frac{m^2}{d}g^{\mu\nu}S_T.
\end{aligned}
\tag{7.1}
\]

A factorized rank-four star input with two `k1` and two `k2` vectors should
produce

\[
\frac{(m^2)^2}{d^2}g^{\mu\nu}g^{\rho\sigma}P_3.
\tag{7.2}
\]

These exercise projection, scalar-product lowering, signed-power boundary
reduction, and free metric collection together.

### 7.2 Optimized direct factorized tensor path

A later optimization can exploit the larger independent rotation group of a
factorized sector.  It must not angular-average one loop while its denominator
still couples that loop to the others.

For a tree:

1. obtain the winning `S4` loop witness and transform the tensor numerator as
   a whole;
2. transform from canonical `k` to the active-edge basis `p` in (5.1) or
   (5.2);
3. expand each explicit vector using the exact combined matrix;
4. apply a local angular moment to `p3`, then `p2`, then `p1`;
5. collect the resulting free metrics and multiply by the radial moments.

For a paw, average only the bridge `p`.  The local pairing rules are:

- `(p.u,p.v) -> p2*(u.v)`;
- `(p^mu,p.u) -> p2*u^mu`;
- `(p^mu,p^nu) -> p2*g^{mu nu}`;

with the common `1/H_r(d)` factor after summing pairings.  What remains is a
two-loop tensor sunset in `u,v`; pass that through the existing global
two-loop tensor projector, lowering bridge, and two-loop pipeline.

The scalar formulas (4.3)-(4.6) should remain the fast specialization.  For
open indices, reuse `perfect_matchings`, `Metric`, `ScalarProductMonomial`, and
the contraction types from `tensor.rs`, with a separate pairing-count limit.
Transforming a rank-`R` explicit tensor through a three-term loop map may
create up to `3^R` terms, so this direct API also needs a transformation-term
cap.  This is precisely where `canonicalize_with_witness` is required.

Do not canonicalize different expanded tensor terms with different stabilizer
winners before applying the loop map.  Choose one witness for the original
denominator powers and transform the complete numerator with it.  After global
projection and denominator lowering, per-scalar-term canonicalization is safe.

## 8. Data structures and implementation sketch

The scalar implementation can stay private to `three_loop_boundary.rs` at
first.  Suggested internal types are:

```rust
type GramPowers = [u32; 6];       // 11,22,33,12,13,23
type PawPowers = [u32; 5];        // D1,D2,D3,u.p,v.p
type SunsetShifts = [u32; 3];     // D1,D3,D4

struct BoundaryWork {
    operations: u128,
    peak_terms: usize,
}

struct TadpoleCache<'a> {
    context: &'a CoefficientContext,
    dimension: &'a Coefficient,
    mass: Coefficient,
    ratios: Vec<Coefficient>,
    radial: BTreeMap<(i32, u32), Coefficient>,
}
```

High-level dispatch is:

```text
try_reduce_integral(input):
    validate arity
    canonical = family.canonicalize(input)
    if canonical is None: return Some(0)
    enforce configured total numerator degree
    switch sector_mask(canonical):
        7  -> reduce_tree(canonical, active=[0,1,2])
        11 -> reduce_tree(canonical, active=[0,1,3])
        15 -> reduce_paw(canonical)
        _  -> None
```

`reduce_tree`:

```text
derive active-edge routing basis and inactive route coordinates
expand inactive D powers into sparse Gram polynomial
for each Gram monomial:
    angular-average p3 using z recurrence
    angular-average p2 using the one-vector formula
    multiply three cached radial moments
sum coefficient and return coefficient * P3
```

`reduce_paw`:

```text
expand D5^r5 D6^r6 into PawPowers polynomial
for each term:
    angular-average and radially integrate p=k2
    expand remaining u2, v2, u.v polynomial into SunsetShifts
    accumulate shifted two-loop integrals
reduce the accumulated combination once through two-loop pipeline
embed S -> ST and P -> P3
```

Use checked `i32::checked_sub` when turning `u32` shifts into integral powers.
Handle `i32::MIN` numerator magnitudes through `i64`/`u64` before conversion to
`usize`.  Reject an input that exceeds the configured bound before allocating
or iterating over its expansion.

## 9. Resource bounds and performance rules

Completeness is algebraic; resource rejection is operational and must be a
typed error.  Add at least these fields to `ThreeLoopBoundaryConfig`:

```rust
pub max_numerator_degree: u32,
pub max_polynomial_terms: usize,
pub max_polynomial_operations: u128,
pub max_angular_terms: u128,
```

Retain `max_tadpole_steps`, `max_two_loop_dots`,
`max_two_loop_seed_candidates`, and `max_two_loop_boundary_terms`.  The induced
two-loop pipeline's numerator coverage should be derived from
`max_numerator_degree`, not be an independently smaller silent setting.

For total inactive numerator degree \(R\):

- a tree Gram polynomial has at most
  \({R+6\choose6}\) monomials, because it has six nonconstant Gram variables
  plus the mass constant;
- the initial paw polynomial has at most
  \({R+5\choose5}\) monomials, because it has five nonconstant variables plus
  the mass constant;
- one scalar two-vector angular step emits at most
  `floor(min(a,b)/2)+1` terms;
- for one paw angular output, expansion of (6.4) emits at most
  \((A+1)(B+1){z+3\choose3}\) raw terms before merging.

Compute binomial upper bounds with saturating `u128`, then enforce incremental
caps before and during all sparse multiplications.  Do not wait until after a
Symbolica-heavy expansion to report a resource error.  Increment the operation
counter by `current_terms * affine_term_count` before each multiplication and
by the number of candidate `z`/shift terms before their loops.

Further performance requirements:

- cache `tau_n`, `R(n,s)`, `1/H_r`, double factorials, and already reduced
  two-loop shifted integrals;
- merge sparse polynomial keys and shifted integrals immediately;
- never generate explicit scalar perfect matchings when (4.3) applies;
- never form factorials or multinomial products in machine integers;
- keep routing arithmetic in `ExactRational`, but convert fixed routing
  coefficients to Symbolica `Coefficient` once per reduction;
- accumulate a paw's complete two-loop `LinearCombination` before reduction;
- preserve deterministic `BTreeMap` order for reproducible caches and tests.

The existing two-loop boundary estimator is cubic in its numerator width and
checks its cap before invoking the analytic formula.  Paw output must flow
through that checked surface, not call a private unbounded coefficient routine.

## 10. Independent validation plan

Keep all Symbolica-backed checks for this module in one integration-test
function because the restricted Symbolica build is tied to its first OS
thread.

### 10.1 Formula goldens

Assert all results in (3.3), (5.6), (5.7), (6.7), and (6.8) by exact
`Coefficient` equality.  The final tree and paw goldens contain nontrivial
`1/d` factors and both are sensitive to angular signs.

### 10.2 Every `S4` image with signed powers

For each of several unequal decorated canonical inputs, including one from
each mask, apply all 24 denominator permutations and assert the same reduction.
Use unequal active dots and unequal negative inactive powers so the test checks
the full signed vector rather than only the sector mask.  Examples are

```text
(2,3,4,-1,-2,-3)       // star
(2,3,-2,4,-1,-3)       // path
(2,3,2,1,-1,-2)        // paw
```

Separately enumerate all 24 vertex permutations, build (2.1), and verify all
six route identities and determinants.  Check the three documented generator
matrices explicitly.

### 10.3 Raw nine-IBP cancellation

For a tree or paw seed, a raw IBP can raise an inactive negative index by one,
but cannot make it positive.  Lowering an active index can only stay in the
same factorized graph, enter a smaller factorized graph, or become scaleless.
Therefore all nine identities at numerator-decorated boundary seeds are
compatible with the checked-in complete factorized-boundary reducer.

At minimum enumerate:

- masks 7 and 11, active powers in `1..=2`, inactive numerator assignments of
  total degree at most two;
- mask 15, active powers in `1..=2`, `(r5,r6)` with `r5+r6<=2`.

Generate all nine raw identities for every seed, reduce every term, and assert
an exactly zero coefficient of both `P3` and `ST`.  This independently checks
derivative factors, symmetry canonicalization, scalar-product inversion,
angular averages, tadpole recurrence, and two-loop composition.

### 10.4 Tensor composition

Run the pure-Rust global projector/lowering/boundary chain on the rank-two and
rank-four checks (7.1)-(7.2).  Also assert odd total tensor rank is zero and
exercise a low pairing/expansion resource limit.

### 10.5 Resource and domain tests

Assert typed errors for:

- wrong arity;
- total numerator degree just beyond the configured maximum;
- preflight polynomial-term estimate beyond its cap;
- incremental operation/angular cap;
- tadpole recurrence cap;
- induced two-loop dot, numerator, and analytic-boundary caps;
- exponent conversion/shift overflow.

Also assert that a scaleless sector returns zero before doing a huge numerator
expansion, and that a numerator-decorated mask 43/31/63 returns `Ok(None)` from
the boundary API rather than being mislabeled as unsupported factorization.

### 10.6 Dimensional homogeneity

For every returned term, if the input has \(A=\sum_i a_i\) and the master has
\(B=\sum_i b_i\), its coefficient must carry the integer power

\[
(m^2)^{B-A}
\]

times a rational function of `d`.  This catches missing bridge `T1` factors
and wrong `m2` signs cheaply.  For example, (6.8) needs `(m2)^2` on `ST` and
`m2` on `P3`.

## 11. Implemented order

The checked-in implementation follows the first eight stages originally
specified here: bounded tadpole/radial helpers; numerator, polynomial, angular,
and exponent caps; sector-first tree and paw expansion; nested two-loop
closure; boundary-first pipeline composition; raw-IBP, symmetry, golden, and
tensor integration tests. The remaining optional symmetry-witness/direct
componentwise tensor path is a speed feature, not a prerequisite for exact
scalar numerator coverage.

## 12. Correctness summary

The tree proof consists of a unit-Jacobian active-edge change of basis, a
finite expansion of each inactive denominator power, the exact finite angular
identities (4.3)-(4.6), and the finite radial identity (3.2).  Every term is a
rational function of `d,m2` times `T1^3`.

The paw proof consists of the exact identities (6.1), the same finite bridge
angular and radial calculation, the complete inverse map (6.4) for the
remaining two-loop scalar products, and the certified two-loop reduction to
`S111` and `T1^2`.  Restoring the normalized bridge `T1` yields only `ST` and
`P3`.

`S4` canonicalization transports all signed denominator powers exactly. A
momentum witness is unnecessary after scalar-product lowering, but is required
and explicitly constructible for a direct tensor fast path. Thus the scalar
algorithm is implemented on the current RustRed architecture, while the
witness API cleanly separates the later tensor optimization from the completed
scalar numerator-boundary milestone.
