# A finite scalar-moment recurrence for an equal-mass banana

October 4, 2026. Mathematical nomination and small Symbolica checks only: no
RustRed source certificate, installed rule, native descent claim, numerical
master evaluation, or five-loop closure result. This supplements the
[structural-recurrence study](structural_recurrence_alternatives_2026-10-04.md).

The useful new result is a triangular elimination that avoids the singular
seven-by-seven transfer-matrix inversion identified there. For six equal-mass
lines it yields an order-three scalar moment recurrence. Whether that can
compress the actual numerator-bearing hot sector remains an open, testable
question rather than an implementation recommendation.

## Literature versus this derivation

Groote, Körner and Pivovarov's section 4 reduces products of integer-order
Bessel functions, after epsilon expansion, to a one-index moment family.
It explicitly checks the boundary condition for its integration by parts.
The generic-dimension triangular construction below is our derivation, not a
claim that the paper proves our complete integer-index or numerator map.
[Configuration-space recurrences, section 4](https://arxiv.org/pdf/hep-ph/9903412).

Their later numerator treatment keeps distributional terms. In its Euclidean
convention, equation 48 is
`∫D³ ΔD = m²∫D⁴ − D(0)³`: the last term is a pinched product, not zero.
Section 5.2 also explains why pointwise higher derivatives of Bessel functions
cannot simply be inserted under the integral while omitting contact terms.
[Numerators and contact terms, section 5](https://arxiv.org/pdf/hep-ph/0403122).

## Triangular elimination, with the source terms retained

Set the common mass to one, let `N` be the number of lines, and define

\[
 \nu=d/2-1,\quad u(r)=K_\nu(r),\quad v(r)=K_{\nu+1}(r),\qquad
 p_0=d-1-N\nu,
\]
\[
 M_t(q)=\int_0^\infty r^{p_0+t+2q}u(r)^{N-t}v(r)^t\,dr,
 \qquad 0\le t\le N.
\]

The derivative identities `u'=νu/r−v` and `v'=−u−(ν+1)v/r` give

\[
 c_t(q)M_t(q)-(N-t)M_{t+1}(q)-tM_{t-1}(q+1)=B_t(q),
 \qquad c_t(q)=d+2q-2t\nu,
\]

where the endpoint source is explicitly

\[
 B_t(q)=\left[r^{p_0+t+2q+1}u^{N-t}v^t\right]_0^\infty.
\]

At endpoints `t=0,N`, omit the term with zero prefactor. If a regulator or
subtraction is needed, its boundary/source prescription is part of `B`, not
permission to silently set it to zero.

Let `E f(q)=f(q+1)` and `F(q)=M_0(q)`. Write `M_t=P_t F+S_t`, with
`P_0=1`, `S_0=0`, and recursively, for `t<N`,

\[
 P_{t+1}={c_tP_t-tEP_{t-1}\over N-t},\qquad
 S_{t+1}={c_tS_t-tES_{t-1}-B_t\over N-t}.
\]

These steps divide only by the positive integer `N−t`, not by `c_t`.
The operator convention is important: `E a(q)=a(q+1)E`. The code shifts
coefficient arguments before multiplying by the shift placeholder; it does
not treat `E` as commuting with `q`.

The remaining `t=N` equation is

\[
 L_NF=B_N-c_NS_N+NES_{N-1},\qquad
 L_N=c_NP_N-NEP_{N-1}.
\]

Induction gives `deg_E P_t≤floor(t/2)` and
`deg_E L_N≤ceil(N/2)`. The coefficient at `E^0` is

\[
 A_0(d,q)={1\over N!}\prod_{t=0}^{N}\big[(1-t)d+2q+2t\big].
\]

No finite-basis claim follows merely from this degree bound: a usable
orientation also needs its leading coefficient and source terms controlled.

## Where the homogeneous scalar relation is justified

For integer `q≥0`, there is a common convergence strip **`0<d<2`** for every
`t=0,…,N`. Here `−1<ν<0`, so at small `r`, `u~r^ν` and
`v~r^(−ν−1)`. The integrand exponent is

\[
 d-1+t(2-d)+2q>-1.
\]

The endpoint in `B_t` has one extra power and tends to zero; infinity is
exponentially suppressed. Consequently `B_t(q)=0` simultaneously on this
positive-q scalar chart. The resulting identity can be continued
meromorphically in `d`; this is stronger than leaving its scalar boundary as
an unspecified obstruction.

It does **not** justify dropping contact terms when mapping momentum
numerators to derivatives, renormalized subtraction terms, or arbitrary
negative-q moments. Direct unregulated integration by parts at `d=4` is not
the argument: for example, at `N=6,q=0` its small-r boundary is divergent.

## Checked recurrence orders and exceptional loci

With the normalization above, exact Symbolica algebra gives:

| Lines `N` | Order `r` | Coefficient of `F(q+r)` | At `d=4` |
| --- | ---: | --- | --- |
| 2 | 1 | `−2(3+2q)` | unchanged |
| 3 | 2 | `3/2` | `3/2` |
| 4 | 2 | `(8/3)(6−d+2q)` | `(16/3)(1+q)` |
| 5 | 3 | `−15/8` | `−15/8` |
| 6 | 3 | `(16/5)(2d−2q−9)` | `−(16/5)(1+2q)` |

All five leading coefficients are nonzero at `d=4` for integer `q≥0`.
Their generic-dimension zero loci still belong to any forward-rule guard.
The trailing product above has different exceptions: at `d=4` it vanishes
for `q=0,…,N−2`, and at `q=−1` its `t=1` factor vanishes identically in `d`.
Thus backward recurrence cannot be used indiscriminately. At `d=4−2ε`,
division by a factor vanishing at `ε=0` may introduce epsilon poles; that is
not the same as a valid specialized relation at `d=4`.

For `N=6` and `q≥0`, the homogeneous scalar recurrence therefore expresses
successive `F(q)` through `F(0),F(1),F(2)` over generic dimension, away from
the leading exceptional loci. This is an upper generating set for **this
moment sequence**, not a proof that a five-loop family has three independent
masters or that the required numerator inputs belong to this sequence.

## A concrete bridge to powers one and two

Using Fourier measure `d^d k/(2π)^d`, the unit-mass propagator of integer
power `a≥1` is

\[
 D_a(r)={C\over 2^{a-1}\Gamma(a)}r^{a-1-\nu}K_{\nu-a+1}(r),
 \qquad C=(2\pi)^{-d/2}.
\]

The adjacent-order identity gives
`D_1=C r^(−ν)u` and `D_2=(C/2)r^(−ν)(rv−2νu)`.
Let `V_s` denote the banana integral with `s` specified lines at power two
and the other `N−s` at power one, divided by the common angular/Fourier
normalization `Ω_(d−1) C^N`. Equal masses make the choice of labels
irrelevant for this scalar integral. Direct binomial expansion gives

\[
 V_s=2^{-s}\sum_{t=0}^s{ s\choose t}(-2\nu)^{s-t}M_t(0),\qquad
 M_t(0)=\sum_{s=0}^t{t\choose s}(2\nu)^{t-s}2^sV_s.
\]

This map has no dimension-dependent division. The top-shift coefficient in
`P_(2j)` is
`(−1)^j (2j−1)!! / [(N−1)(N−3)…(N−2j+1)]`.
For `N=6,j=0,1,2`, these are `1,−1/5,1/5`; hence `M_0(0),M_2(0),M_4(0)`
give a triangular bridge to `F(0),F(1),F(2)` and then to combinations of the
ordinary powers-one/two integrals. This does not make every term a descending
rule in RustRed's existing integral order.

There is also a direct bridge to positive dots on one line:

\[
 r^2D_a=4a(a+1)D_{a+2}+4a(\nu-a)D_{a+1},\qquad a\ge1.
\]

It follows both from adjacent Bessel orders and from Fourier-transforming
`−Δ_k (k²+1)^(−a)`. If `W_a` is the normalized original banana with that
one line at power `a` and all other lines at power one, then

\[
 F(0)=W_1,\qquad F(1)=8W_3+4(\nu-1)W_2,
\]
\[
 F(2)=384W_5+192(\nu-2)W_4+32(\nu-1)(\nu-2)W_3.
\]

Repeated multiplication by `r²` similarly maps each fixed nonnegative
integer `q` to a finite combination of positive-dot integrals. Thus this
scalar proposal need not introduce a new special-function terminal type.
It is not yet a parametric reduction of all independent propagator powers
or a lowering rule for the engine's selected order.

Higher **fixed integer** powers can be reduced to adjacent Bessel orders by
their standard recurrence, but that finite transformation and its bound
have not been implemented or replayed here. Arbitrary independent symbolic
powers and tensor numerators require more work. In particular, contact
terms cannot be absorbed into the scalar homogeneous recurrence by fiat.

### A numerator countercheck that locates the missing term

Already two contracted first derivatives of unit propagators map to
`M_2(−1)`, not to the positive-q chart:

\[
 \int D_1^{N-2}(\partial D_1)^2\,d^dx
   =\Omega_{d-1}C^N M_2(-1).
\]

Here `c_1(−1)=0` identically in `d`, and the sourced recurrence becomes
`−(N−1)M_2(−1)−F(0)=B_1(−1)`. In `0<d<2`, write
`u~a r^ν`, `v~b r^(−ν−1)`, with
`a=2^(−ν−1)Γ(−ν)` and `b=2^νΓ(ν+1)`. This boundary is finite but nonzero:
`B_1(−1)=−a^(N−1)b`. Since `Ω_(d−1) C b=1` and `D_1(0)=Ca`, it gives

\[
 \int D_1^{N-2}(\partial D_1)^2\,d^dx
 ={D_1(0)^{N-1}-\int D_1^N\,d^dx\over N-1}.
\]

Thus the retained radial source reproduces the contact/pinched product in
this simple case. It is an explicit sanity check on the proposed bridge,
not a generic tensor map. It also shows precisely why continuing only the
homogeneous positive-q formula to negative q would give a wrong numerator
identity. Higher contractions can require further endpoint distributions.

## Reproducible formal check and next falsifier

The ignored evidence is
`TMP/rule-optimizer-20261003/profiles/banana_moment_nomination_v1.py` and
`banana-moment-nomination-v1.json` in the same directory. Invocation:

```bash
/nix/store/0r6k8xa2kgqyp3r4v2w7yrb80ma2iawm-python3-3.13.12/bin/python3 -B \
  TMP/rule-optimizer-20261003/profiles/banana_moment_nomination_v1.py
```

The script refuses to overwrite its receipt. It used the existing local
`symbolica-v2.2.0-86-g1f38ba9e` Python module, not a new installation or a
claim about current Symbolica-3 runtime compatibility. Existing RustRed
usage, pinned Symbolica source and public polynomial APIs were inspected;
normalization, differentiation, coefficient extraction and factorization
all use Symbolica. No polynomial kernel was added.

The receipt records 25 derivative checks, 20 triangular/degree checks and
30 independent convergent half-integer controls for `N=2,…,6`. The latter
use exact `d=3`, `q=N+2` Gamma integrals for `K_(1/2)` and `K_(3/2)`, not
numerical quadrature. It also tests leading/trailing nonvanishing as
functions of symbolic `d` for each integer `q=0,…,10`. Runtime was about
0.283 seconds under a one-core/2-GiB/60-second guard; it is not a reduction
performance measurement. Full coefficients are retained in the receipt.

A separate `banana_moment_maps_v1.py` / `banana-moment-maps-v1.json` pair in
the same directory checks eight `−Δ_k` identities (`a=1,…,8`) and 50 forward
and inverse binomial integrand identities (`N=2,…,6`) using the same module
and guard. It took about 0.221 seconds. These check the scalar maps, not
their admission as RustRed identities or generic numerator contact terms.

**Smallest useful next test:** complete the scalar normalization and one
actual numerator-bearing hot-point map, including every contact/pinch
term; then seek an ordinary-source certificate and native guarded descent
using existing machinery. If that map cannot be made exact, or the relation
only changes the presentation without shrinking the unchanged downstream
workload, park it before implementing a moment compiler. A successful
scalar toy alone does not justify altering either live campaign.
