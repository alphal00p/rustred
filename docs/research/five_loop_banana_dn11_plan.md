# Five-loop banana analytic `(D,N) <= (1,1)` certificate

Date: 2026-08-12

This note records the exact next shell for RustRed's equal-mass five-loop
banana.  It is deliberately independent of FORM and Mathematica execution.
All coefficient algebra is over `Q(d,m2)` and all loop changes of variables
are integer unimodular maps implemented in pure Rust.

## 1. Family and declared terminals

Use six oriented physical lines

```text
l0 = k0, ..., l4 = k4,
l5 = -(k0+k1+k2+k3+k4),
sum_a l_a = 0,
Da = la^2 + m2.
```

The bounded certificate is permitted to end only in

```text
M = B(1,1,1,1,1,1)       six-line banana corner
P = B(1,1,1,1,1,0)       product of five one-loop tadpoles
```

`M` is a declared terminal of this finite analytic certificate.  This is not a
proof that it is a minimal master of an unrestricted five-loop family.

The existing scalar identities are

```text
A = B(2,1,1,1,1,1) = (12-5*d)/(12*m2) M,
Pdot/P = (2-d)/(2*m2).
```

The full physical `S6` action is induced by determinant-`+/-1` loop maps.  It
may be used to relate physical line powers, but it must not be represented as
a permutation of the nine ISP basis entries.  ISP images are affine quadratic
forms and must be transformed explicitly.

## 2. Advertised finite box

The next complete domain is

```text
total physical dot degree D <= 1,
total numerator degree N <= 1,
all physical subsectors,
all 15 denominator-basis numerator positions allowed when inactive.
```

Positive auxiliary powers remain outside the domain.  A physical numerator is
allowed only on an inactive physical line.  Deeper dots or more than one total
negative power receive typed errors; they are never copied through as implicit
masters.

For `s` active physical lines, the labelled count is

```text
C(6,s) * (1+s) * (16-s).
```

Here `(1+s)` selects no dot or one dotted active line, and `(16-s)` selects no
numerator or one inactive denominator-basis position.  The exact census is

| active lines `s` | targets |
|---:|---:|
| 0 | 16 |
| 1 | 180 |
| 2 | 630 |
| 3 | 1040 |
| 4 | 900 |
| 5 | 396 |
| 6 | 70 |
| total | 3232 |

All 2,766 targets with `s <= 4` are rank-deficient and scaleless even after a
degree-one polynomial insertion.  The remaining 466 targets have five or six
active physical lines.  Of those, 216 five-line mixed moments vanish by odd
parity.  Thus only 250 labelled targets have generically nonzero output.

## 3. Six-line analytic classes

The nine auxiliary basis entries are the line pairs

```text
(01),(02),(03),(04),(12),(13),(14),(23),(24).
```

At the undotted top corner every distinct pair has

```text
C = integral(li.lj / product_a Da) = (m2*M-P)/5.
```

This follows from `sum_a la=0`, permutation symmetry, and
`la^2 = Da-m2`.

With physical line `r` dotted, define

```text
U = integral(lr^2 / (Dr^2 product_{a!=r} Da))
  = M - m2*A
  = 5*d/12 M.
```

If the numerator pair contains the dotted line, symmetry and momentum
conservation give

```text
X = -U/5 = -d/12 M.
```

If the pair excludes the dotted line, use

```text
V = Pdot - m2*A,
Y = -(V+X)/4
  = (3-d)/12 M + (d-2)/(8*m2) P.
```

Thus the entire six-line `(1,1)` box has only five formula classes:

```text
M, A, C, X, Y.
```

For auxiliary numerator positions 6 through 14, incidence with the dotted
physical line selects `X`; non-incidence selects `Y`.  The sixth oriented line
`l5` is incident to none of those nine stored pairs, so its mixed targets use
`Y`.

## 4. Five-line numerator factorization

When line `m` is missing, the five active oriented lines form an integer
unimodular loop basis.  Let them be `p0,...,p4` in the exact order stored by the
factorization witness.  Transform the one numerator denominator to

```text
c0*m2 + sum_{r<=s} c_rs pr.ps.
```

The witness must store and replay:

- the missing physical line;
- the ordered active physical lines;
- the determinant-`+/-1` loop map;
- the transformed upper-triangular quadratic row;
- the transformed mass coefficient.

Independent tadpole parity kills every `pr.ps` term with `r != s`.  If the
active powers in the same ordered basis are `a0,...,a4`, define

```text
R_a = T_a/T_1,
R_0 = 0.
```

Then the coefficient of `P=T_1^5` is the division-free expression

```text
c0*m2 * product_r R_ar
+ sum_r c_rr * (R_(ar-1) - m2*R_ar) * product_{s!=r} R_as.
```

The order alignment is part of the certificate: a diagonal coefficient may
only be paired with the exponent of the exact active physical line represented
by that loop-basis slot.  Sorting powers for an `S6` orbit is harmless for the
scalar product, but must not be used to reassign a transformed numerator row.

This handles both an inactive physical numerator `D_m` and any of the nine ISP
numerators.  It also handles one simultaneous physical dot.  No tensor engine
outside RustRed and no FORM call is needed.

## 5. Raw-IBP certificate

The previous implementation checked only the sum of five diagonal identities.
The stronger `(1,1)` certificate should reduce all 25 raw top-corner rows
individually.  In oriented-line notation they have the form

```text
0 = delta_ij*d*M - 2 J_(i;j) + 2 J_(5;j),
```

where

```text
J_(a;j) = integral(la.lj / (Da^2 product_{b!=a} Db)).
```

For `i=j`, `J_(i;i)=U` and `J_(5;i)=X`.  For `i!=j`, both relevant terms are
of class `X`.  The formulas above therefore annihilate each row exactly.
Tests should also enumerate all 3,232 labelled targets, require outputs to use
only `M` and `P`, and replay every product-sector loop/quadratic witness.

## 6. Resource accounting

Construction and reduction must carry finite caps for:

- per-integral and aggregate tadpole recurrence steps;
- per-integral and aggregate physical-symmetry word length;
- per-integral and aggregate exact algebra operations;
- input combination term count;
- transformed scalar-product terms (at most 15 here);
- diagonal factor terms (at most five here).

Work must be charged only after domain classification and before the relevant
coefficient or transformation work begins.  Scaleless targets consume no
recurrence work.

The implementation uses separate deterministic units for tadpole recurrence
iterations and structural exact algebra.  A numerator transformation reserves
4,096 exact-algebra operations before constructing its witness; this covers a
worst-case 5-by-5 Gauss-Jordan inverse, the full quadratic-form transform, and
Symbolica coefficient assembly with more than a factor-two margin.  For a
product numerator, `R_a` and `R_(a-1)` are computed together in the same
charged `a-1` recurrence traversal.  Combination caps are preflighted over all
terms before any exact witness or output coefficient is constructed.  Because
RustRed's Symbolica rational-polynomial domain uses `u16` exponents, the total
tadpole-step sum also has a non-configurable ceiling of 65,535; a larger user
budget cannot bypass that typed representability check.

## 7. Exact scalar `(D,N)=(2,0)` certificate

The 25 rows generated from one-dot seeds do not close the two-dot shell onto
`M` alone. They do give a finite certificate after retaining one honest
candidate terminal. After exact `S6`, momentum-conservation, and
factorized-boundary reduction, introduce

```text
A2 = B(3,1,1,1,1,1),
B2 = B(2,2,1,1,1,1),
R  = integral(l0.l1 / (D0^2 D1^2 product others)).
```

The nonzero row classes are

```text
E00: 4*m2*A2 + 2*R
     + (-4 + 8*d/3 - 5*d^2/12)/m2 * M = 0,

E0j: -4*m2*A2/5 + m2*B2/2 - R/2
     + (3/10-d/8)/m2 * M = 0,

Ejj: 5*m2*B2/2 - R/2
     + (-5/2 + 49*d/24 - 5*d^2/12)/m2 * M = 0.
```

But

```text
Ejj = E00 + 5*E0j.
```

The system has rank two for the three unknown classes `{A2,B2,R}`. The exact
minor in columns `{A2,R}` of rows `{E00,E0j}` is `-2*m2/5`, so it is nonzero on
the generic massive domain. Precisely one class remains free at this seed
layer. RustRed chooses the stable double-double representative

```text
B2 = B(2,2,1,1,1,1)
```

as a **candidate terminal**, not a proved unrestricted master. Solving the two
independent rows gives

```text
A2 = -5/2*B2 + (25*d^2-130*d+168)/(48*m2^2)*M,

R  = 5*m2*B2 + (-10*d^2+49*d-60)/(12*m2)*M.
```

The public finite domain contains all scalar physical subsectors with total dot
degree at most two and no numerator. Proper sectors and `D<=1` are delegated
to the existing analytic boundary; the six-line `D=2` shell has only the `A2`
and `B2` physical orbits. Internal proof replay uses the degree-one
oriented-line moment halo. Momentum conservation and `l_i^2=D_i-m2` reduce its
four double-double moment types and the one required triple-dot moment to
`{A2,B2,R}` plus the already certified boundary. All 25 native one-dot-seed
rows then reduce individually to literal zero. The production replay is
`FiveLoopBananaD2Reducer::validate_raw_ibp_provenance`, while the standalone
pure-Rust discovery oracle in `tools/five_loop_d2_rank.rs` can independently
rebuild the marked-edge orbit matrix over a finite field. The exact rank claim
above rests on the displayed rational rows and nonzero minor, not on a single
finite-field specialization.

The corresponding public Rust surface is `FiveLoopBananaD2Reducer`. Calling
`reduce_integral` on any labelled image of `[2,2,1,1,1,1;0,...,0]` returns
`d2_candidate_terminal()` unchanged. This method name is intentional: neither
the API nor this document calls the free column a master.

This is the smallest honest extension suggested by the rank obstruction: it
does not pretend that the free `B2` column was eliminated. A later deeper
seed/numerator halo may reduce it or may provide evidence that it is an
unrestricted master.

## 8. No fixed numerator-free top-dot descent from one 25-row seed

The successful three-loop top-dot construction suggests trying a fixed linear
combination of the 25 five-loop rows at `s = a-e0`, where

```text
a = (a0,...,a5;0,...,0),   ar >= 1,   a0 > 1
```

is the `S6`-canonical scalar target.  This attempt has a short no-go proof.  It
is included to prevent the one-dot dilation identity from being advertised as
an all-index recurrence.

Let `C_ij` weight `d/dk_i . k_j`, and let `q_r` be the six physical routings
`e0,...,e4,(1,1,1,1,1)`.  Differentiating physical line `r` produces the
quadratic form

```text
W_r(C) = 2 * sum_(i,j,p) C_ij q_(r,i) q_(r,p) (k_p.k_j).
```

A recurrence with no numerator output requires every `W_r` to lie in the span
of the six physical quadratics.  That span consists of the five diagonal
quadratics and `(k0+...+k4)^2`; consequently every off-diagonal coefficient in
one of its elements is the same.

For `r=0,...,4`,

```text
W_r(C) = 2 * sum_j C_rj (k_r.k_j).
```

It has no off-diagonal term not incident to `r`.  Uniformity therefore forces
the common off-diagonal coefficient to vanish and hence `C_rj=0` for `r!=j`.
Thus `C` is diagonal.  For the sixth routing, the coefficient of
`k_r.k_s` is proportional to `C_rr+C_ss`.  Equality for every pair forces all
five diagonal entries to agree.  The complete numerator-free weight space is
therefore one-dimensional:

```text
C = c * identity.
```

The resulting weighted equation at an arbitrary scalar seed `s` is only the
global dilation identity

```text
0 = c * ((5*d - 2*sum_r s_r) I(s)
         + 2*m2*sum_r s_r I(s+e_r)).
```

At the corner this gives the already certified one-dot formula after `S6`
collection.  It does not descend for general dots.  Take the canonical target

```text
a = (n,n,n,n,n,n),   n >= 2,
```

and `s=a-e0`.  Besides the pivot `I(a)`, every `r>0` term contains

```text
I(a-e0+er)  ->S6  I(n+1,n,n,n,n,n-1).
```

This integral has the same exact total dot degree as `I(a)` and is strictly
higher in the unsaturated lexicographic tie-break.  Its coefficient
`2*c*m2*n` is generically nonzero.  Hence no nonzero fixed combination of
these 25 rows can simultaneously avoid numerator outputs and give a strictly
descending all-positive scalar recurrence.

This proves a deliberately narrow impossibility statement: one scalar seed,
one fixed combination of its 25 native rows, and no numerator output.  It does
not rule out a recurrence derived from several seed layers or from a closed
numerator halo.  Those are exactly the extensions indicated by the rank-two
`D=2` system above.  RustRed should not add a `five_loop_top_dot` API until such
an extended certificate has been derived and checked.
