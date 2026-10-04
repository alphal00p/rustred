# A finite scalar-moment recurrence for an equal-mass banana

October 4, 2026. The scalar-moment construction remains a mathematical
nomination. The final section additionally records a successful native
ordinary-source/descent proof for one reusable numerator chart. No rule was
installed, master evaluated, or five-loop closure established. This supplements the
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

## Actual owner0 numerator: a simpler reusable ordinary-IBP chart

The original archived family is Minkowski-style `D_i=q_i²−1`, not the
Euclidean convention used above. Its owner `000011001001011` has the
oriented banana momenta

```
p1=k5, p2=k1−k3, p3=k2−k3, p4=k3−k5,
p5=k3−k4, p6=−(k1+k2−k4),     sum(p)=0.
```

These are respectively `D5,D6,D9,D12,D15,D14`; `D13=(p4−p5)²−1`.
The actual rank-one pressure point is
`(0,0,0,0,3,1,0,0,1,0,0,1,−1,1,2)` in original physical axes.
Thus the two lines touched by its numerator have powers one and two, not
two unit powers. The other four powers are spectators.

In Euclidean configuration space, expanding this numerator gives the exact
pinched product plus `U−T+2G`, where
`U=∫D3 D1^5`, `T=∫D3 D2 D1^4`, and
`G=∫D3 D1^3 ∂D1·∂D2`. The sign of the last term includes the minus from
the two Fourier momentum insertions. The pointwise adjacent-order identities
give `G=T+νU`; the common convergent strip justifies this scalar manipulation.
The explicit pinch is `D3(0) D1(0)^3 D2(0)`, not a dropped contact term.
Under Wick rotation, each original denominator changes sign: the target
has signed power sum eight, while `U,T,pinch` have sums eight, nine, eight.
The common five-loop measure phase cancels. Consequently the original-family
identity is `target=pinch+(d−1)U−T`.

More usefully, it has a direct ordinary-IBP version before invoking any
equal-mass exchange. Let `I(a,b;c)` retain arbitrary positive powers on
`D5,D6,D9,D14`, put powers `a,b` on `D12,D15`, power `c` on `D13`, and set
all other absent powers to zero. For `a≥1,b≥2`, the proposed relation is

\[
 I(a,b;-1)=I(a-1,b;0)
 +{d+b-1-2a\over b-1}I(a,b-1;0)+I(a,b;0)
 -{2a\over b-1}I(a+1,b-1;0).
\]

This follows from `∂_(p5)·p4` acting on the source with pair powers
`a,b−1`, all other active line momenta held fixed. In the original basis,

\[
 \partial_{p5}=-\partial_{k1}-\partial_{k2}-\partial_{k3}
                  -2\partial_{k4},\qquad p4=k3-k5.
\]

It is exactly eight original ordinary rows. In native `RowId` convention
`ordinary-ibp:contraction:differentiated`, the pairs are `(2,i)` with weights
`−s_i` and `(4,i)` with weights `s_i`, for `i=0,1,2,3` and
`s=(1,1,1,2)`. All have source offset `+e13−e15` (one-based axes).
The unnormalized target pivot is `b−1`, nonzero on the requested chart;
no dimension-dependent division is necessary. At `a=1`, the first term is
the explicit pinched product. No symmetry identity is needed for these four
tails.

At the observed point the physical RHS keys are:

| Label | Physical powers | Coefficient |
| --- | --- | --- |
| P | `(0,0,0,0,3,1,0,0,1,0,0,0,0,1,2)` | `1` |
| S | `(0,0,0,0,3,1,0,0,1,0,0,1,0,1,1)` | `d−1` |
| T | `(0,0,0,0,3,1,0,0,1,0,0,1,0,1,2)` | `1` |
| U | `(0,0,0,0,3,1,0,0,1,0,0,2,0,1,1)` | `−2` |

The exact involution exchanging `p4,p5` has determinant `−1` and sends
`k_i→k_i−k4+k5` for `i=1,2,3`, `k4→−k4+2k5`, `k5→k5`.
It leaves the other active lines and `D13` unchanged. For this particular
pair of powers it identifies `U=T`, giving the three-tail form above.
That simplification needs an authenticated symmetry bridge; it must not be
passed to an ordinary-only proof checker as though it were already a source
combination. The four-tail form avoids that complication.

The archived order is `rustred.spired-uncut-sector-order.v1`. Its source
comparison checks support and then total corner distance, with numerator
degree before dot degree at the subsequent degree tie-break. All same-sector
proposed tails lower numerator rank from one to zero without increasing total
dot count, so corner distance strictly decreases; a pinched tail lowers
support. Thus the source-level order analysis predicts
strict descent over this whole chart, independently of spectator powers;
the completed native proof below also checks it. The existing observed rule
emitted seven successors at the pressure point, versus four in this proposal.
That is an immediate support comparison, **not** a downstream speedup.

### Formal checks and completed native gate

`banana_owner0_rank1_map_v1.py` and its JSON receipt check the actual momentum
map, the Bessel/moment elimination and the relative Wick signs using the same
existing Symbolica module. An independent convergent `d=1` calculation gives
Euclidean target `89/12288`, including pinch `3/512`. The expanded scalar
result is also retained there; it agrees with the simpler identity above.
`banana_pair_ibp_formal_v1.py` checks the general rational `a,b,d` row
identity, all actual directional invariants, and the exchange map. These
formal checks take fractions of a second and are not native certificates.

The native request is
`TMP/rule-optimizer-20261003/candidates/banana-owner0-rank1-chart-v1/request.json`.
It uses the existing compiled `prescribed-source` tool: generate all ordinary
rows, select the named eight at their exact common offset, normalize the
actual pivot, retain its conditions, and invoke
`check_original_source_combination`. All six active powers remain unbounded
above; only `D15≥2`, `D13=−1`, and the other inactive powers zero are fixed.
This is a reusable chart, not a fitted catalog point or a five-loop-special
engine path. Proof only was requested: no export, installation, master
evaluation, or traversal.

The sole independently preflighted attempt started at 07:28:49 UTC and
completed successfully: `EXACT_ORIGINAL_SOURCE_CHART_PROVED`. The tool regenerated 25
ordinary sources, selected the prescribed eight, retained the 37-term
unspecialized source product, and derived the pivot `n14−1` (zero-based
index variable, hence `b−1`). Its four normalized RHS coefficients agree
with the displayed formula. The native checker covers the entire requested
chart with two sign cells, separating `a=1` from `a≥2`; the pole condition
`b−1≠0` remains attached with source/normalization/RHS origins. No zero-sector
tails were discarded.

Measured costs were 1.993 seconds inclusive, with 1.131 seconds preparation
and 0.002641 seconds isolated source/descent proof. Sampled peak process-tree
RSS was 328,175,616 bytes; the 16-GiB/150-GB-host-reserve guard drained all
owned groups without interruption. Evidence is `execution-result.json` and
`guard/stdout` alongside the request, the latter SHA256
`c0adaaa3c21714f531a182bc17dc51476336b662edc470bb580712b42805a029`.
These timings measure this tiny proof, not rule-generation or campaign gain.

### Isolated export and all-positive numerator-rank extension

The rank-one checked export subsequently passed in 3.327 seconds inclusive,
with sampled peak tree RSS 896,421,888 bytes. It retains all 483 original
rules and 83 terminal keys, adding one checked
`AfterBaselinePartitionWholePiece` alternative. Its isolated payload is
`candidates/banana-owner0-rank1-export-v1/artifact/candidate.rrbin` under the
same evidence directory, SHA256
`9c00d51eeed0258c374e791e3991057a54df5d8646ee15c20c134cbfecc41ec9`.
The exporter reruns the original-source proof and checks its native codec
roundtrip; the bytes by themselves do not carry a cold source-proof authority.
Nothing has been installed or compared in a campaign.

The same eight rows also give a wider recurrence. Write
\(T_r(a,b)=I(a,b;-r)\), keeping the other four positive powers arbitrary.
For \(a\ge1,b\ge2,r\ge1\),

\[
\begin{split}
T_r(a,b)={}&T_{r-1}(a-1,b)+T_{r-1}(a,b)\\
 &+\frac{d+b-1-2a+2(r-1)}{b-1}T_{r-1}(a,b-1)
 -\frac{2a}{b-1}T_{r-1}(a+1,b-1)\\
 &+\frac{2(r-1)}{b-1}
   \left[T_{r-2}(a-1,b-1)-T_{r-2}(a,b-2)+T_{r-2}(a,b-1)\right].
\end{split}
\]

This follows by differentiating the additional source numerator
\(D13^{r-1}\), with
\(\partial_{p5}D13=-4(p4-p5)\), since \(p4=P-p5\). Thus
\(\partial_{p5}\cdot[p4\,D13^{r-1}]\) contributing
\(-2(r-1)(D12-D15+1+D13)D13^{r-2}\).
At \(r=1\), the three last terms vanish before any outside-root activation
test; they are not interpreted as new positive-ISP obligations. At \(a=1\)
or \(b=2\), the pinches remain explicit. Every surviving same-sector tail
lowers rank by one or two, without increasing corner distance; pinches lower
support. There is still only the safe denominator \(b-1\).

The tiny formal Symbolica check and rank-one specialization passed in
0.014 seconds. A preserved prepared-only v1 narrative originally mislabeled
the final three denominator powers as `c` rather than `c+2`; its algebra
was correct and no native run used that narrative. The separately frozen
v2 request corrects this explicitly.

The sole v2 native proof started at 07:50:37 UTC and passed in 2.205 seconds
inclusive, 1.118 seconds preparation and 0.003181 seconds isolated proof,
with sampled peak tree RSS 328,024,064 bytes and a clean owned-group drain.
It checks the seven-free-axis chart, including an independently unbounded
negative D13 ray, with the original eight sources and all seven derived
tails. Eight exact Cartesian sign cells retain the rank-one/higher-rank and
pinch boundary distinctions. Evidence is
`candidates/banana-owner0-all-positive-rank-proof-v2/{request.json,guard/stdout,execution-result.json}`.
This is a native reusable original-family recurrence, not merely a moment
nomination, but it is not closure of other numerators, the `b=1` face, or
the whole banana family. Its separately audited isolated export subsequently
passed in 3.621 seconds inclusive, again preserving the 483-rule suffix and
83 terminals. The broader payload at
`candidates/banana-owner0-all-positive-rank-export-v1/artifact/candidate.rrbin`
has SHA256 `3886d04f3ebb8f957deedd98ed74107bfa3ee5432388d4ae38222f82b1e85440`.
The broader chart subsequently entered the isolated comparison below; neither
chart has been installed in production.

### Preserving the existing repair overlay

Owner0 already has an immutable digest-bound repair overlay. Direct base
replacement invalidates that binding; the preferred-program path currently
rejects a same-owner overlay. Neither restriction should be bypassed.
The existing public APIs instead support an explicitly validated re-export:
cold-load the original overlay against the original owner; precharge its
native clone-owned payload; replay the unchanged rules, source conditions,
guards and strict descent against the checked new owner; encode with its
actual digest; then independently cold-load the result and reject it under
the old digest. The original rules, requested domains, terminal boundary,
ordering and both original files remain intact. The standalone research
adapter implements that sequence with three focused clone/scope/ingress
tests; the actual overlay runs supply the native replay authority, not those
three tests or the JSON receipt.

Both re-exports passed. Rank-one took 38.556 seconds inclusive; the broader
chart took 36.778 seconds. Each preserves the two original repair rules and
one requested domain, verifies all 83 terminal keys, and replays both rules
in the old context, new context, and independent new cold import. Each also
rejects the new overlay under the old owner digest. The bounded clone charge
was 313,432 bytes; sampled peak process-tree RSS stayed below 890 MB. Evidence
is in `candidates/banana-owner0-{rank1,broad}-overlay-reexport-v1/`.
The broader overlay SHA256 is
`4c4cab7a6ef78e4572b5f891935fed1fcbb7b68288043e8728a34febdff66827`.
The original owner and overlay were not modified; this does not install the
new pair or confer recursive closure.

Compilation is separate: the first one-core all-arity optimized link was
cleanly deadline-censored after 271.611 seconds, with no binary. The unchanged
source then linked successfully on eight reserved compiler CPUs in 145.354
seconds inclusive, with a 3.49-GB sampled peak. No engine rebuild or new
algebra implementation was needed; the adapter links existing native services.

### First complete shared-work comparison

The preregistered original four-query panel completed in 381.601 seconds
inclusive, retaining all 67 old-saved owners, 8,246 routes and both repair
overlays. Only the checked owner0 payload and its officially replayed overlay
changed. Both arms used the same optimized `a3c9e542…2570ec` executable,
16 reserved workers, original query caps and independent full cold reinspection
with native levers **Off**. All four required queries passed, with no pending
work, frontiers, abandoned obligations or native errors.

| Metric | Baseline | Broader recurrence | Change |
| --- | ---: | ---: | ---: |
| Scheduled domains | 19,103 | 14,149 | −25.93% |
| Native inspections | 17,485 | 12,872 | −26.38% |
| Emitted events | 159,218 | 125,011 | −21.48% |
| Traversal time | 1.080 s | 0.766 s | −29.05% |
| Entire fresh arm, including staging/preparation/cold check | 188.341 s | 189.911 s | +0.83% |

This is a substantial reduction of actual shared descendant work on this
small completed cohort, not just a shorter immediate RHS. It is **not** yet
a whole-campaign speedup: loading and cold verification dominate these fresh
arms, and the total time is essentially flat. The reusable recurrence's
unbounded chart and exact source proof do not establish how often the full
production campaign will encounter it.

Read-only checkpoint extraction finds four and seven outgoing physical
singleton images at the two owner0 roots, exactly matching the proved cells;
the two owner3 root child sets are unchanged. Native counters corroborate
that support change. This is **support-consistent evidence**, not a selected
ordinal or coefficient-equality receipt: CP6 does not record selected rule
IDs, and the reader does not interpret coefficient displays. The independent
native cold check remains the graph-verification authority.

Evidence: `candidates/banana-owner0-broad-fourpoint-pair-v1/`, including
`comparison.json`, `execution-result.json`, both `cold-all.json` files and
`root-activation-diagnostic.json`. A separately frozen candidate-first repeat
started at 08:31:29 UTC on the identical inputs. The pre-existing H1 validation
point (independent of banana fitting, but previously exposed in a censored
union) remains a subsequent gate. No new favorable point
is selected from this result, and no production restart is recommended yet.

A prospective current-new37 transfer check exposed a naming pitfall: the
actual current production input inventory already equals the completed
pilot's inventory, byte for byte for all 67 owner payloads and both overlays,
with identical routes, family and load limits. Neither manifest contains
preferred-program entries: the name `new37` is **not** a count of 37 such
entries. The evaluator hashes actual files; this conclusion does not rely
on historical manifest metadata. The redundant transfer plan remains
prepared-only and is not run. Evidence:
`candidates/banana-owner0-broad-currentpool-fourpoint-v1/input-identity.json`.
This strengthens the relevance of the tested context but says nothing about
the untested production query workload or its eventual closure.

The candidate-first repeat also completed, in 385.755 seconds inclusive,
with both full cold checks passing. Scheduled domains decreased
19,102→14,169 (−25.82%), native inspections 17,490→12,860 (−26.47%),
and events 159,333→124,444 (−21.90%). Traversal decreased
1.023→0.820 seconds (−19.81%). Entire fresh-arm time was
189.217→193.719 seconds (+2.38%), again preparation/cold dominated.
The baseline domain variation between repeats is only one, compared with
the approximately 4,950-domain treatment reduction. This reproduces the
shared-work benefit under reversed arm order without implying byte-identical
graphs or a measured production ETA.

All sampled peak process-tree RSS values are 5.20–5.26 GB. Waited child CPU,
including preparation and verifier subprocesses, is 1,584→1,698 seconds in
the first pair and 1,673→1,700 seconds in the repeat; it is not pure traversal
CPU and does not show an end-to-end CPU win here. The independent cold checks
retain 925–946 abstract cycle nodes: their coverage result is not a proof of
coefficient back-substitution through those cycles. Full measurements and
raw input bindings are consolidated in
`candidates/banana-fourpoint-replication-results-v1.json`. Predeclared H1
validation is censored as detailed below. Separately, the consolidated engine's
unchanged-algebra
four-loop control completed on all 58 queries with cold PASS and exact graph
identity; that is an engine control, not another measurement of this
five-loop recurrence. Neither successful banana training pair authorizes
a production switch by itself.

### Predeclared H1: both observations censored

H1 is the already registered `heldout-high-positive-low-rank` owner31 point,
with its original A23/R1 caps. It was not selected from the banana results
and was not replaced by an easier tuple. Its baseline hit the registered
cooperative deadline and drained cleanly after 731.162 seconds inclusive.
It had scheduled 6,932,981 domains, performed 5,362,831 native inspections,
and retained 780,624 pending obligations, with no frontiers or abandoned
obligations. Native preparation took 87.344 seconds and traversal through
the interrupted result took 630.555 seconds; sampled peak tree RSS was
11.10 GB. The checkpoint is saved and resumable, but the baseline is **not
completed or cold-verified**.

A prospectively approved contingency ran the unchanged candidate alone,
using the remaining **original** 1,800-second inclusive budget, original
candidate phase deadlines and identical inputs. Its conservative clock
starts at 08:50:20 UTC, before the actual first launch; it is not reset.
The baseline failure stays intact. A candidate completion would establish
only bounded validation feasibility after mandatory cold reinspection, not
a completed paired speedup or an inferred baseline-time lower-bound gain.
Both incomplete outcomes are retained.
Evidence is under
`candidates/banana-owner0-broad-currentpool-H1-validation-v2/`, with separately
bound `candidate-only-contingency-v1/` metadata. Candidate execution began
at 09:04:36 UTC and also reached its cooperative deadline. It drained cleanly
at 1,511.053 seconds on the original cumulative clock, having scheduled
6,295,995 domains with 4,805,983 native inspections and 756,869 pending
obligations. Its preparation took 88.245 seconds and interrupted traversal
553.371 seconds, with a 10.70-GB sampled peak and no frontiers or abandoned
obligations. Its checkpoint is also saved and resumable.

Neither arm was cold-reinspected or completed. Their unequal interrupted
durations and remaining scopes preclude relative timing/work ratios; the
smaller partial candidate count is **not** a gain. The earlier two completed
training comparisons remain positive, but this harder predeclared validation
does not establish their generality. The consolidated raw summary is
`candidates/banana-owner0-broad-currentpool-H1-validation-v2/censored-observations.json`.
No alternative easier validation point or automatic retry was substituted.

A separate discriminator was prepared to run the **same H1 point**
through the exact finite-key mechanism in both owner-treatment arms, retaining
all source guards, routes, terminal boundaries and the checked repair overlay.
That tests representation feasibility as well as the recurrence; it is not a
timing comparison with the two censored symbolic observations. Concrete H1
size was unmeasured before that attempt, and explicit aggregate limits could decline.
The initial unrun one-million-node proposal remains preserved. Before any
concrete H1 observation, root registered a larger aggregate-budget request:
16 million nodes/rule applications/transport calls, 1,024 million transport
operations, 128 million endpoints and 256 million coalescing additions,
identical in both arms. Seed, positive-layer, retained-seed-byte, per-formula,
RSS and inclusive-time limits are unchanged. The earlier six-million-domain
symbolic expansion motivates avoiding a prematurely small aggregate ceiling;
it is not a concrete cardinality estimate. The prospective request is
`candidates/banana-H1-finite-feasibility-v2/plan.json`; the actual V4 build
and test receipts were later bound in `plan-v3-bound.json` without changing
the inputs, commands or budgets. The successful four-loop, H55 and original
260-seed engine controls preceded this attempt.

The H1 baseline launched at10:02:52UTC. Its finite trace declined after
67.113 seconds with `aggregate_budget`, at **exactly1,000,000 rule
applications**, despite the requested16-million feature allowance. The
complete persisted root diagnostic records2,577,437 scheduled operational
states,1,888,757 completed states,688,679 queued states and2,572,537 physical
keys. This is an incomplete prefix, not a closed count or a cardinality bound.
The implementation configured concrete node/transport caps but retained the
ordinary `ReductionLimits` defaults; the core's deliberate minimum
intersection therefore imposed its one-million rule-application ceiling.
The same retained policy also has one-million pending-frame and16-million
coalescing caps. **The intended16-million-application experiment did not
occur**: this is a configuration-composition finding, not evidence that16
million is insufficient, nor a failure of the mathematical recurrence.

After the trace declined, native execution fell back to the symbolic walk.
Root stopped that experiment early because fallback could not establish the
planned finite comparison. The original group clock ended at332.972 seconds;
the verified experiment launcher received SIGINT, its controller propagated
the signal to its owned native process, and all owned processes drained
without a hard kill. Native exit was−2: **no final result or committed
checkpoint was written**, so this interrupted attempt is not resumable.
Only its already-flushed root frame, partial sidecar, events and guard receipts
are retained. Cold verification and the candidate arm were never launched.
No time/work ratio, closure claim, cap retuning or retry follows.

Evidence:
`candidates/banana-H1-finite-feasibility-v2/effective-limit-diagnostic-v1/receipt.json`
(`6d3525c4…7af54aa`) pins the raw complete root frame, exact requested argv,
frozen application/core/default-limit sources and stop/drain receipt. It
explicitly does not authenticate the remainder of the interrupted sidecar.

The next held-out experiment was separately versioned before execution.
It adds explicit matching allowances of16 million applications,16 million
pending frames and256 million coalescing additions, leaving all originally
requested finite caps and the1,800-second group budget unchanged. Both arms
must first pass native numeric requested/admitted/effective-policy checks
before either owner pool is prepared; the live and cold budgets must then
agree with those checks. This corrects policy composition rather than
assuming the requested allowance reaches the core.

Before any corrected16-million H1 outcome, root selected the combined485
bank from the completed original-four-root training comparison. Therefore
the new registered comparison is **original483 baseline versus combined485**,
not the superseded484 candidate. The exact H1 row, caps, other66 owners,
routes, second overlay, terminal boundary and resource budget remain fixed.
The old corrected484 proposal is preserved as deferred, and failure will
not silently switch back to it or select an easier target. This is an
independent workload check with disclosed earlier H1 exposure, not globally
unseen data. Prepared evidence is
`candidates/banana-H1-combined-finite-feasibility-v1/plan.json`; the retained
old proposal is under `banana-H1-finite-feasibility-v4/`.

The corrected comparison launched at11:07:05UTC. Both native numeric
preflights passed, and the actual baseline root record confirms the requested
16-million node/application/pending allowances and256-million coalescing
allowance reached the native kernel. The baseline nevertheless declined on a
**different, correctly applied limit**: a routed expansion requested128,008,047
aggregate endpoints against the frozen128,000,000 allowance. This is a bounded
feasibility failure, not another effective-budget mismatch.

At the refusal,253.003 seconds of finite work had scheduled7,278,128 operational
nodes representing7,272,139 physical keys, completed5,522,744, and left1,755,383
queued. It performed3,186,441 rule applications and2,002,310 transport calls,
with127,999,479 reserved pre-coalescing endpoint units and530,580,163
conservative transport-operation units;
279 declared terminals and327,828 zeros were visited. It did not finish or
close H1. These counters do not expose stack depth and cannot identify a
particular recurrence corridor as the cause of endpoint growth. In particular,
the endpoint counter is a prospective structural upper bound charged before
Symbolica expansion, not the number of surviving emitted monomials. The record
does not retain the latter or attribute global deduplication hits by phase.

The existing symbolic fallback ran only until the registered cutoff. The group
ended at601.851 seconds with a cooperative deadline, exit−2, no hard kill, and
all owned processes drained. No candidate walk or cold replay ran; no final
walk result or committed checkpoint exists. The retained first2,978-byte
diagnostic frame is authenticated against its raw prefix, not authority for
the rest of the interrupted sidecar. Evidence is
`banana-H1-combined-finite-feasibility-v1/endpoint-refusal-diagnostic-v1/receipt.json`.
There is **no completed paired ratio or held-out closure claim**.

A separate candidate-only diagnostic was then prospectively frozen under
`banana-H1-combined-candidate-feasibility-v1/plan-v2.json`. It keeps the same
combined485 input, H1 point, numerical limits and600/630-second walk plus
810/840-second cold deadlines on its own900-second clock. Its purpose is to
measure the otherwise unrun treatment, not compare it to a censored baseline.
Finite refusal retains the existing fallback only to the same cutoff; fallback
closure would be reported separately, not as finite success. No budget growth,
retry, switch to484, easier point, or ratio to the failed baseline is permitted.
The candidate-only diagnostic subsequently executed with the same correctly
admitted limits. Its finite inspection also refused the endpoint allowance:
128,008,444 requested against128,000,000, after252.217 seconds. The retained
snapshot contains7,250,782 scheduled operational nodes,7,244,793 physical keys,
5,501,746 completed,1,749,035 queued,3,165,202 applications and2,002,556
transports;127,999,876 structural endpoint units and530,582,303 operation units
had been charged. It reported279 terminals,327,823 zeros and no missing owner
or rule. This is not a completed closure and is not compared by ratio with the
censored original483 observation.

Its fallback stopped at the registered cooperative cutoff,602.158 seconds
inclusive, with no hard kill and all owned processes drained. No cold replay,
final result or committed checkpoint exists. Peak sampled process-tree RSS was
9.927GB. The independent receipt retains the complete2,979-byte root diagnostic
and raw stop evidence under
`banana-H1-combined-candidate-feasibility-v1/endpoint-refusal-diagnostic-v1/receipt.json`.
Neither observation justifies raising the limit, restarting production, or
claiming that H1 can never close; they establish that this fixed finite-work
allowance was exhausted with both tested banks.

### Complementary face: a directly proved mirrored ordinary-source chart

The established chart excludes `b=D15=1`. A direct hand derivation nominates
the disjoint face **`a=D12≥2, b=1, r=−D13≥1`**, retaining arbitrary positive
powers on `D5,D6,D9,D14` and zero on every other inactive axis. This is not
an exchange-symmetry assumption. In the actual original momentum basis,

\[
 V=\partial_{k1}+\partial_{k2}+\partial_{k3}+2\partial_{k4},
 \quad Vp4=1,\quad Vp5=-1,\quad \operatorname{div}(V,p5)=-d.
\]

All four spectator momenta are invariant. For `X=D12,Y=D15,Z=D13`, direct
contraction gives `p5·VX=X+Y+1−Z`, `p5·VY=−2(Y+1)`, and
`p5·VZ=2(X−Y−1−Z)`. Applying this ordinary IBP to
`H X^{−(a−1)}Y^{−b}Z^{r−1}` derives the target pivot `a−1` and

\[
\begin{split}
T_r(a,1)={}&T_{r-1}(a,0)+T_{r-1}(a,1)
 +\frac{d+a+2r-5}{a-1}T_{r-1}(a-1,1)
 -\frac{2}{a-1}T_{r-1}(a-1,2)\\
 &+\frac{2(r-1)}{a-1}
 [T_{r-2}(a-1,0)-T_{r-2}(a-2,1)+T_{r-2}(a-1,1)].
\end{split}
\]

The exact nominated source rows are `(2,i)` with weight `s_i` and `(3,i)`
with weight `−s_i`, for `i=0,…,3`, `s=(1,1,1,2)`, in the existing
`ordinary-ibp:contraction:differentiated` convention. Every row has physical
offset `+e13−e12` (one-based). The sole denominator `a−1` is positive;
same-support tails lower corner distance and all pinches remain explicit.
At `r=1`, the last three coefficients vanish before positive-ISP activation.
The prospective corner checks are `(2,1,1)`, yielding exactly
`T0(2,0)+(d−1)T0(1,1)+T0(2,1)−2T0(1,2)`, and `(2,1,2)`, which must retain
both the `D15=0` and `D12=0` pinches.

The original-family map, source convention and existing Symbolica-backed
translation/specialization/normalization/proof services were reread; an
independent hand review agrees. Preparation performed no CAS or native run.
The exact existing-schema request, short derivation and preserved prepared plan
are under `TMP/rule-optimizer-20261003/candidates/banana-owner0-mirrored-b1-prepared-v1/`.
Request SHA256 is `34dd331196b15f0225878dedf43d39cc8adf78a8c4549fbee19497ccc237c454`.
The separately authorized sole prove-only attempt subsequently passed:
`EXACT_ORIGINAL_SOURCE_CHART_PROVED`, in 2.339 seconds inclusive, with
1.128 seconds native preparation and 0.002774 seconds isolated proof.
It generated all 25 ordinary rows, selected the prescribed eight, retained
the 36-term unspecialized source product, and derived pivot `n11−1=a−1`.
Seven generic normalized shifts agree with the nomination. Four native sign
cells split `a=2` from `a≥3` and `r=1` from `r≥2`; the respective RHS counts
are four and seven, with the `a−1` condition retained in every cell. This
includes the nominated `a=2,r=1` and `a=2,r=2` faces. The checker validates
original sources, conditions, root admissibility and descent over the whole
six-free-axis chart, not only those corners. The JSON retains generic
coefficients and per-cell term counts, not separate per-corner coefficient
arrays; no display was parsed into algebraic authority.

The owned guard drained cleanly without a stop or failure; sampled peak tree
RSS was 472,772,608 bytes. Evidence is
`candidates/banana-owner0-mirrored-b1-proof-v1/{execution-result.json,guard/stdout}`
under the same TMP tree; raw stdout SHA256 is
`889e76546af33f8e07f2c01cd669b5edd118b5fe7e8dc1016855eae3e25f952f`.
An isolated checked export subsequently extended the existing `b≥2` candidate
from484 to485 rules in3.349 seconds inclusive. The exporter rechecked the
complete original-source proof and native codec roundtrip, preserved all484
prior rules and83 terminal keys, and retained both disjoint charts as
`AfterBaselinePartitionWholePiece` alternatives. Normalized sources, RHS,
pre-cancellation conditions and proof exactly match the prove-only receipt.
The new owner SHA256 is
`ebd0def4ae1c77cab74c2f377dc706ad5a9c03af4d0155781b022f3ed856b397`.

The existing two-rule owner0 repair overlay was then natively re-exported,
not manually rebound: old-context cold replay, unchanged scope/terminals,
precharged clone, new-context original-source/guard/descent replay, actual
new-digest encoding, independent output cold replay and old-digest refusal
all passed. All three replay counts are two; the original one requested
domain, zero overlay residuals and83 base terminal keys remain unchanged.
This took36.421 seconds inclusive and produced220,735 bytes with SHA256
`f330f7082f102a1dfd056be4904ca19ac90300ec0199e52620da73d1e4be38eb`.
Both guards drained without stops/failures; the other owner's overlay was
not changed. Evidence is under
`candidates/banana-owner0-combined-mirrored-{export,overlay}-v1/`.

No installation followed these artifact checks. The separately registered
candidate-first484-versus485 comparison subsequently completed on the
unchanged four required roots and full67-owner/8,246-route/two-overlay
context, using the same optimized V4 executable with finite replay disabled
in both arms. Both fresh arms passed independent cold All/Off verification
with zero debt and all four roots certified; the whole pair drained cleanly
in392.698 seconds.

Broad484 versus combined485 gave14,132→11,291 scheduled domains
(**20.10% less**),12,833→10,231 native inspections (20.28% less), and
123,344→110,676 events (10.27% less). Traversal was0.7691→0.6493 seconds;
whole-arm time195.755→193.595 seconds was nearly tied (1.10% less), dominated
by88–89-second preparation and93–96-second cold checks. Waited descendant
CPU was1,697.52→1,702.07 seconds; sampled peak tree RSS5.223→5.182GB.
Concurrent app compilation and observed foreign host jobs are recorded;
these are contemporaneous measurements, not historical timing comparisons.
The fixed baseline-first repeat also completed and cold-passed, in384.438
seconds inclusive:14,136→11,264 scheduled domains (**20.32% less**),
12,830→10,215 native inspections (20.38% less), and124,124→110,158 events.
Traversal was0.7554→0.6415 seconds; whole-arm191.127→189.739 seconds again
nearly tied (0.73% less). Waited descendant CPU was1,684.84→1,689.09
seconds and sampled peak RSS5.307→5.237GB. The two contemporaneous baselines
differ by four domains, versus reductions of2,841 and2,872; both fixed
comparisons cross the domain-work threshold without a material whole-arm
regression. This supports a reproducible **local workload** improvement,
not a20% end-to-end campaign-time claim. No further strategy or threshold
adjustment followed the outcomes.

The original owner0 roots have `a=1,b=2` and cannot themselves select the
mirror. The completed candidate instead contains two whole-inspected
singleton descendants on `a=2,b=1,r=1`, with the original exact inherited
rank/A/D caps retained. All four recorded physical children of each are
in bijection with the proved mirror shifts. Their old-bank counterparts
had seven successors and28/27 term visits; the candidate has four
successors and seven term visits with three native zero terms.
Checkpoint support consistency alone was not treated as selected-rule
evidence: a separately guarded full-pool native match, with no successor
traversal, independently selected **batch0/rule0** at both exact domains,
with matching counters identical to the archived native inspections.
It completed in132.671 seconds including123.912 seconds preparation;
the local match itself took0.002946 seconds. This confirms actual native
dispatch and joins the retained complete boundary evidence, without
claiming that independent descendant-cone costs can be added. The repeat
contains the same two exact source geometries/caps, identical native matching
counters and complete mirror-consistent boundaries; a retained read-only
join connects these to the direct dispatch observation without another
native owner load.

Evidence: `candidates/banana-owner0-mirror-marginal-fourpoint-v1/` contains
the complete pair, measurement summary and bounded read-only descendant
census; `profiles/banana-mirror-native-activation-v1/` contains the native
dispatch receipt. `candidates/banana-mirror-replicated-results-v1.json`
retains all four arms' timing, CPU, memory and workload values, while the
reverse-pair directory retains its independent boundary census and activation
join. No production update follows. The two charts cover
neither `a=b=1` nor mixed numerators and do not establish full five-loop
closure or a20% end-to-end campaign-time improvement.

### Completed 260-seed treatment check: negligible work reduction

The next frozen comparison used the original required region
`phys-d9-a10-r1-111000100111001`, not a newly selected easy tuple. It was
previously exposed in the finite-envelope engine control, but held out from
the banana treatment comparisons. Both arms used the same V5 executable,
finite replay enabled, all260 seeds, and unchanged finite/source limits.
The full67-owner/8,246-route/two-overlay pool differed only by original483
versus checked combined485 owner0 and its officially replayed repair overlay.
The H1 failures above remain separate negative evidence.

Both arms completed with exact260-seed enumeration and exhaustion, no missing
owners/rules or pending work, and independent cold All/Off replay of all roots.
Live and cold finite work matched exactly apart from timing. All four guarded
process groups drained; the pair took394.003 seconds inclusive.

| Finite-work quantity | Original483 | Combined485 |
| --- | ---: | ---: |
| Operational states |69,772|69,682|
| Physical keys |69,512|69,422|
| Rule applications |33,640|33,561|
| Transport calls |23,349|23,338|
| Reserved transport endpoint bounds |465,886|465,875|
| Reached declared terminals / zeros |66 /12,457|66 /12,457|

The90-state reduction is **0.129%**, far below the preregistered20% work
target. The larger four-root improvement therefore does not transfer in
magnitude to this region. These counters describe completed native work;
reserved transport endpoints are still structural bounds, not emitted terms.
No per-rule activation or exclusive descendant-cone cost is inferred.

Traversal was2.615→2.596 seconds and whole-arm time191.103→192.264 seconds.
Foreign jobs were already using CPUs32–33 when the frozen32–47 pair started;
root retained the running experiment as a work-count comparison. These wall
times are diagnostic and **do not support a speed claim**. Preparation, cold,
waited-child CPU and sampled tree RSS remain in the raw summary. No retry,
cap adjustment, production installation or second full campaign followed.

Evidence: `candidates/banana-260-combined-finite-pair-v1/`, frozen plan
`7c80abac…137b3f1`, execution receipt `9c1df2c1…6b2430b`, and summary
`4e32ef96…cad452e`. This is completed regional validation with a negative
substantive-benefit result, not validation of all required regions or an
identical final coefficient basis.

## Source nomination after the raw25 locality miss

The separately bounded raw25 probe supplied all25 ordinary rows, no weights,
on the original483 owner and the two already nominated full charts/offsets.
Both whole-chart source proofs passed, but the first selected identities had
15 and10 RHS terms respectively, with nonzero shifts on spectator axes. They
did **not** rediscover the proved local seven-tail mechanism. The5.122-second
pair and independent source/guard/drain audit are retained under
`TMP/rule-optimizer-20261003/candidates/banana-owner0-raw25-rediscovery-v1/`.
This is a first-selected-result negative, not absence of the known local
combinations or a closure/performance test.

A constant vector-field calculation explains a prospective restriction without
supplying the eight source weights. Preserving the four massive spectators
D5,D6,D9,D14 gives `v5=0`, `v1-v3=0`, `v2-v3=0`,
`v1+v2-v4=0`, hence `v=(1,1,1,2,0)` up to scale. For a degree-zero
logarithmic vector field, the mass constants force each proportionality factor
to zero; with all five contractions the expected spectator-tangent space has
dimension5. This is independently checked structural reasoning, not a native
kernel receipt. Existing protected-source/logarithmic tools can express that
kernel, but their point solve is not the full-chart proof bridge here. Merely
applying an invertible rebasis before complete fixed-column RREF would not
change its row space or canonical result.

The smaller experiment instead changes the selected endpoint objective. The
generic research option `forbid_endpoint_changes_on_axes: [4,5,8,13]` derives
all offending columns from the **complete native post-fixed source universe**,
then unions them with existing F before any exact row selection. No individual
source is discarded: the unchanged Symbolica projector must cancel the whole
weighted endpoint coefficient. Current raw25 receipts and support inspection
do not expose the complete75-column universe, so constructing a supposedly
complete explicit F from their displayed winning products would be unsound.
The helper adds no algebra kernel and does not weaken source or rule proof.

This remains hand-informed chart/window/spectator selection; only the source
weights are discovered. The known eight-source proofs witness feasibility
of the locality restriction, not that the first candidate will pass all guards
or have seven tails. Any source replay, guard, descent or residual-locality
failure stops the fixed attempt without automatic row/degree/chart growth.
At `a=b=1`, the two unchanged translation windows have identically zero target
columns because their raising prefactors specialize to `a-1` or `b-1`:
this is a bank/window obstruction, not terminal authority. Neither locality
search nor the proved charts claim mixed-numerator or full-family coverage.
The software gate and fixed native test below were preregistered separately;
neither authorized changing the bank, source order or charts after an outcome.

The implementation gate subsequently passed110 optimized tests (eight new),
including complete-bank-before-selection, collective native cancellation with
retained poles, cap refusals and residual checks. Independent review verified
the frozen source, raw test receipts and clean build drain. The old and new
toy input/request and exported candidate bytes agree exactly.

The sole authorized native pair then completed in5.045 seconds, with both
process groups cleanly drained and no export or installation. Both full charts
returned `EXACT_ORIGINAL_SOURCE_CHART_PROVED`, using the original483 owner,
all25 unweighted ordinary inputs, unchanged order/offsets/caps, fresh native
source certificates and zero refinements. Independent final receipt/source-proof
and documentation review passed. Each actual post-fixed universe had
75 columns; locality selected53, adding53 to broad-chart F and44 to mirror F
(final F sizes53 and58). The first target appeared after24 and19 rows.

Both resulting circuits have eight discovered original-source contributions
and seven generic tails, versus15/10 tails in the unfiltered control. Every
tail preserves all four nominated spectators. Native finite-face proofs retain
four tails at rank one and seven at rank two, including all pinches; the broad
and mirrored charts split into eight and four native cells respectively.
Their final native cell guards are precisely `b-1` and `a-1`. The eight
normalized original-weight **display strings** and their source/offset keys
are literally equal to the corresponding previously proved hand-nominated
circuits, and the RHS shift sets agree. No display was parsed, and this is
not a separate algebraic equality certificate; validity rests on each fresh
native original-source replay and full chart/guard/descent proof.

Evidence is
`TMP/rule-optimizer-20261003/candidates/banana-owner0-endpoint-locality-v1/`
(plan SHA`21bc6b73…81a4b8`, execution SHA`b42c4ccb…3ed258`). The frozen
requests retain an inherited historical provenance sentence saying no
spectator constraints were supplied; the actual new field and execution plan
explicitly impose endpoint locality, and govern this experiment. This is a
positive **hand-informed locality-to-source-weight discovery** result, not
automatic topology/chart discovery, a new identity space, CP6 closure, or a
new campaign performance measurement. The earlier raw25 negative remains
evidence that unconstrained first-target selection misses this local mechanism.
