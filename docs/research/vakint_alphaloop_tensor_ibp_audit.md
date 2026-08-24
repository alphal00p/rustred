# Vakint/alphaLoop tensor and parametric-IBP audit for RustRed

Status: source-complete audit, 2026-08-13.

This report records the Vakint and alphaLoop behavior that constrains RustRed.
It is based on the checked-in Rust, Symbolica expressions, FORM sources, and
tests under `vendor/gammaloop/crates/vakint`. No FORM process was executed.

Citation convention: abbreviated paths such as `lib.rs`, `topologies.rs`,
`graph.rs`, `symbols.rs`, `tests/...`, `templates/...`, and `form_src/...`
are relative to the exact crate root `vendor/gammaloop/crates/vakint/`.
Every citation includes checked-in line numbers.

## Governing conclusion

RustRed must reproduce the following behavior natively in Rust with
Symbolica:

1. canonicalize an input graph, propagator ordering, loop-momentum basis, and
   numerator without assuming particular input IDs;
2. reduce arbitrary Lorentz tensor numerators to scalar products and metrics;
3. rewrite scalar products into denominator powers/ISPs;
4. generate fully parametric IBP relations and guarded reduction rules by the
   generic LiteRed-like algorithm;
5. apply those generated rules with exact guard, sector, symmetry, and
   termination semantics; and
6. return exact coefficients multiplying unsubstituted master integrals.

The FORM code is a behavioral oracle, not production code. In particular:

- `tensorreduce.frm` and `pvtab10.h` describe expected projectors, but RustRed
  must construct those projectors algorithmically rather than ship rank
  tables.
- `integrateduv.frm` contains hardcoded one-, two-, and three-loop parametric
  reductions. They are excellent regression fixtures, but RustRed must derive
  equivalent rules from freshly generated IBPs rather than copy or dispatch
  to those topology-specific recurrences.
- Vakint's master expansions and decimal constants belong to an optional
  evaluation/oracle layer. They do not define IBP reduction.
- RustRed must never call FORM. A validation harness may compare a RustRed
  result with a separately obtained Vakint golden result, but FORM is not a
  RustRed dependency or subprocess.

This agrees with Vakint's stated present scope: matching plus analytic tensor
and parametric-IBP reduction are implemented with Symbolica around FORM
(`vendor/gammaloop/crates/vakint/README.md:3-10`). The public example pipeline
is canonicalization, tensor reduction, then integral evaluation
(`vendor/gammaloop/crates/vakint/README.md:64-137`). RustRed replaces both
external algebra stages while preserving their mathematical contract.

## Source inventory and classification

### Production behavior RustRed must own

| Concern | Authoritative Vakint locations | RustRed requirement |
|---|---|---|
| Expression vocabulary | `vendor/gammaloop/crates/vakint/src/symbols.rs:10-22`, `25-77`, `79-135`, `194-220` | Preserve `k`, `p`, symmetric `g`, symmetric/linear `dot`, arbitrary index atoms, and user coefficients natively. |
| Term splitting | `vendor/gammaloop/crates/vakint/src/lib.rs:2187-2197`, `2589-2618` | Split a sum into one topology atom plus a spectator numerator per term; reject malformed topology powers. |
| Topology construction | `vendor/gammaloop/crates/vakint/src/topologies.rs:40-99` | Support the same canonical 1L, 2L, and complete 3L denominator families as concrete compatibility inputs, without making them production dispatch cases. |
| Graph parsing/canonization | `vendor/gammaloop/crates/vakint/src/graph.rs:117-179`, `195-345` | Validate graph incidence, deduplicate contractions by graph isomorphism, and use stable family fingerprints. |
| Loop-basis selection | `vendor/gammaloop/crates/vakint/src/graph.rs:349-409`; `vendor/gammaloop/crates/vakint/src/topologies.rs:476-638` | Select/solve a loop-momentum basis and rewrite routings exactly. |
| Topology matching | `vendor/gammaloop/crates/vakint/src/lib.rs:1126-1204`, `1206-1501` | Match short/full forms, orientations, masses, powers, and arbitrary IDs; choose deterministically among automorphisms. |
| Simultaneous numerator map | `vendor/gammaloop/crates/vakint/src/lib.rs:1381-1417`, `2258-2364` | Apply loop-basis substitutions simultaneously, never cascading a map such as `a -> b, b -> c`. |
| Tensor reduction | `vendor/gammaloop/crates/vakint/src/lib.rs:2426-2568`; `vendor/gammaloop/crates/vakint/form_src/alphaloop/tensorreduce.frm:211-392` | Replace the entire FORM bridge with native exact projector generation/contraction. |
| Scalar reduction/application | `vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:17-1127` | Generate generic parametric IBPs/rules and apply them natively; match hardcoded alphaLoop results only as an oracle. |
| Dot/index conversion | `vendor/gammaloop/crates/vakint/src/lib.rs:4492-4651` | Accept indexed and dot forms and preserve arbitrary/namespaced index atoms. |
| Output normalization | `vendor/gammaloop/crates/vakint/src/lib.rs:4334-4435` | Keep reduction coefficients separate from measure, epsilon-series, and master-value conventions; offer an explicit adapter if Vakint-formatted output is needed. |

### Oracle-only material

The following must not be embedded as RustRed's reduction algorithm:

- the rank-2/4/6/8 inverse projector coefficients in
  `vendor/gammaloop/crates/vakint/form_src/alphaloop/tensorreduce.frm:43-175`;
- the rank-10 seven-orbit table and coefficients in
  `vendor/gammaloop/crates/vakint/form_src/alphaloop/pvtab10.h:1-673`;
- the topology-specific guarded rules in
  `vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:76-137`
  and `301-1099`;
- the master epsilon expansions in
  `vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:1162-1189`;
- the fitted/special numerical constants in
  `vendor/gammaloop/crates/vakint/src/alphaloop_numerics.rs:9-68`; and
- numerical outputs in the integration tests.

They are golden data. A production result is acceptable only if its rules
carry provenance back to generic generated IBPs and replay symbolically.

### Complete alphaLoop FORM-procedure disposition

| Procedure/resource | Exact source | Disposition |
|---|---|---|
| `LoadPVTable(rank)` | `form_src/alphaloop/tensorreduce.frm:177-199` | Replace with on-demand native pairing-orbit/projector generation. |
| `TensorReduce()` | `form_src/alphaloop/tensorreduce.frm:211-392` | Reimplement fully with Symbolica atoms and exact rational-function solves. |
| rank-10 PV data | `form_src/alphaloop/pvtab10.h:1-673` | Golden fixture only; do not load it in production. |
| `TruncateExpansion` | `form_src/alphaloop/integrateduv.frm:11-15` | Optional master-evaluation concern, outside core reduction. |
| `IntegrateUV1L` | `form_src/alphaloop/integrateduv.frm:17-29` | Reproduce from generic generated IBP plus native numerator lowering/application. |
| `IntegrateUV2L` | `form_src/alphaloop/integrateduv.frm:31-153` | Oracle for generated rules; never topology-dispatch to a copied table. |
| `IntegrateUV3L` | `form_src/alphaloop/integrateduv.frm:155-1127` | Oracle for generated rules; never topology-dispatch to a copied table. |
| `IntegrateUV` | `form_src/alphaloop/integrateduv.frm:1129-1139` | Replace with typed family matching followed by the generic native reducer. |
| `Masters` | `form_src/alphaloop/integrateduv.frm:1162-1189` | Optional oracle/value-provider data, not RustRed rule generation. |
| `SubstituteMasters` | `form_src/alphaloop/integrateduv.frm:1191-1218` | Keep outside the reduction core; RustRed normally stops at master keys. |

The two driver templates are likewise eliminated from production:
`templates/run_tensor_reduction.txt:1-28` and
`templates/run_alphaloop_integral_evaluation.txt:1-30`. Vakint's generic FORM
transport helpers (`src/lib.rs:4653-5131`, `5230-5282`) are not algorithms to
port; native Symbolica objects make sanitization, index-number substitution,
text parsing, and subprocess management unnecessary.

## Vakint expression and canonicalization contract

### Input and term model

A term is represented by `VakintTerm { integral, numerator, vectors }`
(`vendor/gammaloop/crates/vakint/src/lib.rs:2187-2192`) and uses

```text
numerator * topo(
  prop(prop_id, edge(left_node,right_node), momentum, mass_squared, power)
  * ...
)
```

or a canonical short form such as `topo(I1L(muvsq,a1))`. The splitter finds
all `topo(...)` atoms and takes a Symbolica coefficient list with respect to
them (`vendor/gammaloop/crates/vakint/src/lib.rs:2589-2617`). RustRed should
retain arbitrary spectator factors in `numerator`, including namespaced
functions, complex constants, epsilon dependence, external vectors, and free
metrics.

The relevant tensor vocabulary is

```text
k(loop_id, lorentz_index)
p(external_id, lorentz_index)
g(index_1,index_2)
dot(momentum_1,momentum_2)
```

Vakint registers `dot` as symmetric and linear and `g` as symmetric
(`vendor/gammaloop/crates/vakint/src/symbols.rs:79-90`). Indices are general
Symbolica atoms, not integers; the decorated-index tests deliberately use
functions such as `mink4(4,33)` (`tests/integral_evaluation_freeform_tests.rs:23-56`).

### Canonical known families through three loops

The registry defines (`vendor/gammaloop/crates/vakint/src/topologies.rs:40-99`):

```text
I1L: D1 momentum k1

I2L: D1 momentum k1
      D2 momentum k2
      D3 momentum k1+k2

I3L: D1 momentum k1
      D2 momentum k2
      D3 momentum k3
      D4 momentum k3-k1
      D5 momentum k1-k2
      D6 momentum k2-k3
```

The 2L registry contains the full sunset and the edge-3 pinch
(`topologies.rs:53-74`). The 3L registry generates graph-inequivalent
loop-preserving contractions automatically (`topologies.rs:75-99`,
`231-287`). A contraction merges nodes, removes its propagator, writes a zero
at that propagator's position in the short exponent vector, and appends a
`_pinch_<ids>` suffix (`topologies.rs:341-474`). These names are output
conventions, not valid family identities for a generic reducer.

### Matching algorithm that RustRed must be compatible with

1. Try known topologies in registry order; optionally construct `UNKNOWN` if
   no known topology matches (`topologies.rs:289-303`, `674-788`).
2. A short form directly captures all symbolic/numeric masses and powers and
   installs identity loop-momentum substitutions (`lib.rs:1126-1181`).
3. A full form first replaces directed `edge` by undirected `uedge` for a
   cheap graph-pattern rejection (`lib.rs:1215-1229`).
4. Enumerate oriented matches, require each canonical node's oriented images
   to agree and require different canonical nodes to map to different input
   nodes (`lib.rs:1240-1279`).
5. Record edge IDs and flips, and score a match first by the complexity of the
   induced loop-basis replacement, then by edge flips, ID distance, and exact
   ordering (`lib.rs:1281-1379`, `1426-1477`).
6. Build input-loop to canonical-loop substitutions, including the sign from
   an edge flip (`lib.rs:1381-1417`). Missing canonical propagators receive
   power zero (`lib.rs:1418-1423`).
7. Apply numerator replacements in one simultaneous Symbolica operation
   (`lib.rs:2258-2286`). This is an observable requirement: sequential
   substitutions can corrupt basis permutations.

`Graph::find_unique_contractions` preserves the loop count and uses
Symbolica graph canonization to retain one representative per isomorphism
class (`graph.rs:248-345`). `get_one_lmb` finds a cycle basis
(`graph.rs:349-409`), and `force_an_lmb` solves a Symbolica linear system and
rewrites every propagator routing (`topologies.rs:476-638`). RustRed can reuse
the algorithmic idea, but its cache/family key must additionally authenticate
the complete momentum coefficient matrix, masses, external kinematics, and
ISP completion.

### Canonicalization tests

The complete input-matching test inventory is:

- arbitrary 1L loop, node, and propagator IDs to canonical `k(1)`/`I1L`:
  `tests/input_matching_tests.rs:8-69`;
- arbitrary 2L sunset labels/routing: `:73-139`;
- pinched 2L independent tadpoles, including a canonical loop swap:
  `:142-233`;
- a 3L contraction with zeros in the full exponent vector: `:236-263`;
- unknown-topology rejection versus `topo(UNKNOWN(...))`: `:266-304`; and
- a second two-tadpole pinch fixture: `:307-326`.

These are compatibility fixtures. They must not become special cases in the
IBP generator.

## Native tensor reduction

### Existing FORM data flow

Vakint currently only marshals the calculation:

1. expand dot notation into indexed vector pairs (`lib.rs:2431-2438`, with
   conversion implementation at `4492-4595`);
2. map loop vectors to FORM `vec` and external vectors to `vec1`
   (`lib.rs:2440-2488`);
3. serialize arbitrary user expressions/indices (`lib.rs:2490-2505` and
   `4653-4954`);
4. run `TensorReduce` with `tensorreduce.frm` and `pvtab10.h`
   (`lib.rs:2507-2514`);
5. parse `rat`, metrics, vectors, floats, and indices back
   (`lib.rs:2535-2564`, `4956-5131`).

The template declares `D`, `ep`, vectors, `rat`, includes the reducer, calls
`TensorReduce`, and converts surviving external contractions to `dot`
(`templates/run_tensor_reduction.txt:1-28`). None of this text marshalling is
needed in RustRed.

### Mathematical behavior

`TensorReduce` first contracts metric chains and already paired internal
indices (`tensorreduce.frm:218-226`). It counts remaining loop-vector slots;
odd rank is zero (`:229-234`). For even rank (R=2r), Lorentz covariance gives

\[
  T^{\mu_1\ldots\mu_{2r}}
  =\sum_{P\in\mathcal P_2(2r)} C_P
    \prod_{(i,j)\in P}g^{\mu_i\mu_j},
\]

where each coefficient is a scalar integral containing the loop scalar
products selected by the pairing. FORM groups pairings into orbits under
identical-vector symmetry (`:240-259`), applies precomputed inverse projector
tables (`:261-292`), and reconstructs/contracts the result with outside
metrics and external vectors (`:294-365`). It dynamically loads higher even
ranks and errors if an internal tensor remains (`:369-391`).

The first projectors make the convention explicit:

\[
  \int k^\mu k^\nu F = \frac{g^{\mu\nu}}{D}\int k^2F,
  \qquad D=4-2\epsilon,
\]

from `tensorreduce.frm:43-53`. At rank four, the two orbit coefficients are

\[
  c_1=\frac{D+1}{D(D-1)(D+2)},\qquad
  c_2=-\frac1{D(D-1)(D+2)},
\]

which is `tensorreduce.frm:57-69` after replacing `ep=(4-D)/2`. For four
identical loop vectors the orbit sum collapses to the familiar

\[
 \int k^\mu k^\nu k^\rho k^\sigma F
 =\frac{g^{\mu\nu}g^{\rho\sigma}
       +g^{\mu\rho}g^{\nu\sigma}
       +g^{\mu\sigma}g^{\nu\rho}}
      {D(D+2)}\int(k^2)^2F.
\]

Ranks six and eight are hardcoded at `tensorreduce.frm:71-175`. Rank ten has
seven pairing orbits (`pvtab10.h:1-647`) and seven rational coefficients
(`pvtab10.h:648-673`). This is an implementation ceiling of the fixture, not
RustRed's intended rank ceiling.

### Symbolica-native algorithm

RustRed should generate the projector for each encountered tensor monomial:

1. contract all unambiguous metric/vector dummy indices first;
2. extract the ordered internal loop-vector slots and the outside tensor;
3. return exact zero for odd slot count;
4. enumerate perfect pairings of the internal slots;
5. quotient them by permutations of slots carrying the same loop momentum;
6. form one metric-orbit sum (B_\alpha) per orbit;
7. compute the exact Gram matrix
   (G_{\alpha\beta}(D)=B_\alpha\mathbin{:}B_\beta) by unioning the two
   pairings; each closed index cycle contributes (D);
8. compute the contracted right-hand sides, replacing a pairing of slots
   carrying (k_i,k_j) by `dot(k(i),k(j))`;
9. solve the small exact linear system over the Symbolica rational-function
   field in (D); and
10. reconstruct the outside tensor and canonicalize dummy indices.

This reproduces the PV tables but is topology-, loop-, and rank-independent.
Cache the inverse/projector by `(rank, multiplicity partition, D-domain)`;
never cache it by a concrete topology or numerator. Singular factors such as
`D`, `D-1`, or `D+2` remain exact denominator guards. Keep (D) symbolic
through projection and substitute (D=4-2\epsilon) only at the configured
output boundary.

## Fully parametric scalar-rule behavior

### Denominator convention and numerator lowering

The alphaLoop identities use

\[
  D_a=q_a^2-m^2,
  \qquad q_a^2=D_a+m^2.
\]

This is visible already at one loop:

```text
g(k,k)*uvprop(k,n) = uvprop(k,n-1) + mUV^2*uvprop(k,n)
```

(`integrateduv.frm:19-21`). At two loops the three diagonal scalar products
are lowered at `:44-47`, and the three mixed products are expressed as linear
combinations of the three denominator shifts and `mUV^2` at `:49-59`. At
three loops all six diagonal and fifteen mixed products are lowered at
`:191-229` after graph/routing maps have been built at `:160-189`.

RustRed must do this generically from each family's denominator linear forms,
including auxiliary denominators/ISPs. It must not reproduce the 2L/3L
formula list by momentum-name pattern matching.

### One-loop parametric relation and normalization map

The complete alphaLoop 1L reducer is `integrateduv.frm:17-29`:

\[
  I(n)=\frac{D+2-2n}{2(n-1)m^2}I(n-1),\qquad n>1,
\]

and (I(n)=0) for (n<1). The source writes `D=4-2*ep` directly
(`:24-26`). Iterating gives, in RustRed's natural master convention
(M_1=I(1)),

\[
 I(n)=C_n(D,m^2)M_1,
 \quad
 C_n=(m^2)^{1-n}
      \prod_{r=1}^{n-1}\frac{D/2-r}{r}
 =\frac{\Gamma(D/2)}{\Gamma(n)\Gamma(D/2-n+1)}(m^2)^{1-n}.
\]

The exact concrete oracle coefficients for powers 1 through 6 are:

| `n` | coefficient of RustRed `I(1)` at `D=4-2*ep` |
|---:|---|
| 1 | `1` |
| 2 | `(1-ep)/mUV^2` |
| 3 | `-ep*(1-ep)/(2*mUV^4)` |
| 4 | `ep*(1-ep^2)/(6*mUV^6)` |
| 5 | `-ep*(1-ep^2)*(2+ep)/(24*mUV^8)` |
| 6 | `ep*(1-ep^2)*(2+ep)*(3+ep)/(120*mUV^10)` |

Vakint's internal master symbol has an extra convention:

```text
uvprop(1) = uvid(1,1)/ep
```

(`integrateduv.frm:26`). Therefore the structural comparison map is

```text
RustRed master I(1)  <->  alphaLoop uvid(1,1)/ep.
```

The master-value routine later defines `uvid(1,1)` as `ep*mUV^2` times a
Laurent series (`integrateduv.frm:1162-1165`). That epsilon series, the loop
measure, and the MS-bar factors are not part of the reduction relation.

Important limitation: the alphaLoop template always calls
`SubstituteMasters` (`templates/run_alphaloop_integral_evaluation.txt:18-25`).
Vakint's `AlphaLoopOptions { susbstitute_masters: false }` only suppresses the
later Rust substitution of named numerical constants (`lib.rs:4350-4383`); it
does not leave raw `uvid` atoms intact. An exact master-coefficient oracle
should consequently capture or fixture the expression immediately after
`IntegrateUV` and before `SubstituteMasters`, or compare after applying the
documented master-normalization map. Do not infer a reduction mismatch from
the fully normalized epsilon series.

### Two-loop hardcoded oracle inventory

`IntegrateUV2L` recognizes the disconnected two-tadpole graph and the
three-edge sunset (`integrateduv.frm:31-42`), lowers all scalar numerators
(`:44-61`), then applies:

- four zero-sector rules at `:65-69`;
- two sector permutations at `:71-73`; and
- nine ordered guarded recurrences beginning at lines
  `76, 83, 90, 96, 102, 111, 118, 125, 132` and ending at `:137`.

The guards distinguish `n<0`, `n<1`, `n>0`, and `n>1`. The final all-positive
`n3>1` rule at `:132-137` is an ordered alternative to the earlier rule at
`:111-116`; preserving first-match behavior matters when interpreting this
file as an oracle. The procedure maps boundary integrals to two placeholders
at `:141-142`, then to the two master conventions at `:150-152`.

These nine recurrences must be reproducible by RustRed-generated parametric
IBPs for the concrete sunset family. They must not be copied into production.

### Three-loop hardcoded oracle inventory

`IntegrateUV3L` recognizes five graph sectors—Mercedes, five-edge, banana,
sunrise-bubble, and triple-bubble—at
`integrateduv.frm:155-182`, maps propagator powers at `:184-189`, and lowers
all 21 loop scalar products at `:191-242`.

Its sector/rule inventory is exhaustive as follows:

- 27 zero-sector identities at `:253-279` (lines `276` and `277` are the same
  rule in the checked-in source);
- 19 sector permutations at `:281-299`;
- 59 ordered guarded parametric recurrences beginning at:

```text
301, 313, 325, 337, 349, 361, 373, 382, 391, 400,
416, 432, 441, 459, 474, 486, 498, 513, 525, 537,
546, 564, 582, 591, 601, 611, 622, 631, 640, 651,
660, 669, 677, 688, 697, 702, 712, 721, 730, 743,
751, 770, 789, 808, 827, 839, 851, 863, 875, 887,
914, 933, 952, 971, 988, 1005, 1024, 1045, 1066
```

  and ending at `:1084`; and
- 14 late boundary/raising-operator transforms at `:1086-1099`.

The broad progression is lower/negative sectors (`:301-743`), sectors with a
negative first index and otherwise positive lines (`:751-913`), full-positive
denominator lowering with explicit `mUV^-2` (`:914-1023`), and three ordered
raising alternatives (`:1024-1084`). Some late transforms encode shifts as
products of `uvid` factors; those products are merged by adding their six
index vectors at `:1109-1110`. A typed RustRed rule must represent these as
shift operators directly, never as products that could be confused with
factorized physical integrals.

### Guard and application semantics

Both 2L and 3L use FORM's `id ifmatch->end...`: on a successful match, control
jumps past the remaining ordered rules. The loop variable is reset so the
sorted expression is processed again (`integrateduv.frm:139-149` for 2L and
`:1101-1116` for 3L). The observable semantics are therefore:

```text
repeat until stable or a checked budget is exhausted:
    apply zero rules
    canonicalize by ordered sector symmetry
    choose the first guarded reduction rule that matches
    apply it once to the selected integral
    combine equal shifted integrals exactly
```

RustRed may use a more efficient work queue, but it must make these concepts
explicit:

- a rule domain over index inequalities/equalities and coefficient
  nonvanishing assumptions;
- deterministic rule priority or a proved order-independent normal form;
- simultaneous shift substitution;
- exact combination/cancellation of identical integral keys;
- a strict descent/termination certificate for every accepted rule;
- preservation of exceptional loci rather than division by a coefficient
  that can vanish; and
- a resource-budget error distinct from “master integral.”

The alphaLoop source substitutes `D=4-2*ep` and merges shift products before
sorting (`integrateduv.frm:1105-1114`). RustRed should keep the configured
dimension symbolic during generation and specialize only through an explicit
coefficient-context map.

### Master and output conventions

`IntegrateUV` attempts 3L, then 2L, then 1L topology matching and errors if a
`vxs`/`uvprop` graph survives (`integrateduv.frm:1129-1139`). The master table
contains:

- one 1L master and two 2L masters (`:1164-1168`);
- the 3L Mercedes, five-edge, banana, sunrise-bubble, and triple-bubble
  representatives (`:1170-1187`).

The master substitution expands rational functions far enough to tolerate a
configured spurious pole and truncates afterward (`:1191-1213`). Those are
evaluation concerns. RustRed's reduction endpoint is a canonical exact sum

\[
  \sum_j c_j(D,\text{kinematics},a)\,M_j,
\]

with stable family/sector/exponent master keys and no implicit epsilon
expansion.

Vakint adds, per loop, the alphaLoop-to-user normalization correction

\[
 i\,\pi^{D/2}e^{-\gamma_E\epsilon}
 e^{-\epsilon(\log(m^2/\mu^2)+\log\mu^2)}
\]

and then the configured loop normalization (`lib.rs:4385-4414`). It expands
the requested epsilon series and restores logarithms at `:4416-4433`.
Structural RustRed/Vakint comparisons must strip or independently apply this
layer.

## Every Vakint Rust test: audit inventory

All test functions under the crate were inspected. The inventory below also
states how each group should be used.

### `tests/tensor_reduction_tests.rs`

- `test_reduction_1l_a`, lines `7-39`: exact rank-2 plus odd-rank result.
- `test_reduction_1l_b`, lines `42-74`: internal contractions, odd term, and
  two different external momenta.
- `test_reduction_2l_a`, lines `77-109`: mixed-loop rank-2 behavior.

These are direct exact tensor golden tests.

### `tests/input_matching_tests.rs`

- `test_1l_matching`, `8-69`;
- `test_2l_matching_3prop`, `73-139`;
- `test_2l_matching_pinched`, `142-233`;
- `test_3l_matching_with_zero_powers_in_short_form`, `236-263`;
- `test_unknown_integrals`, `266-304`; and
- `test_2l_pinched_matching`, `307-326`.

These are canonicalization fixtures, independent of reduction correctness.

### `tests/integral_alphaloop_vs_matad_tests.rs`

- 1L scalar powers 1 through 6, `test_integrate_1l_no_numerator`, `24-48`;
- squared mass syntax at powers 1 and 2,
  `test_integrate_1l_no_numerator_squared_mass`, `51-75`;
- 2L scalar sunset, `test_integrate_2l_no_numerator`, `78-101`;
- 3L basketball numerators `k1^2` and `k3^2`, `104-155`;
- 3L scalar Mercedes, `158-185`;
- 3L rank four, `188-219`; and
- the same rank-four comparison at non-unit scales, `222-253`.

The remaining master-series setup through `:600` supplies backend comparison
constants, not additional reducer algorithms. These tests are especially
valuable as a one-to-three-loop differential oracle after conventions are
mapped.

### `tests/integral_evaluation_analytic_tests.rs`

- six 1L tests at starts `24, 183, 207, 231, 262, 293`: exact unsubstituted-
  constant alphaLoop coefficients, scalar/squared-mass forms,
  `(k.p)^2` with denominator power two, user scalar coefficients, and two
  distinct external vectors (`:24-320`);
- one 2L scalar test at `323-352`;
- six 3L tests at starts `355, 388, 426, 467, 506, 548`, covering scalar and
  rank-four alphaLoop/MATAD paths with user coefficients (`:355-579`); and
- fourteen 4L FMFT tests at starts `582, 617, 652, 694, 737, 777, 817, 855,
  894, 932, 966, 1002, 1036, 1070` through `:1110` (14 starts total), covering
  H/X/FG/clover families, pinches, dotted lines, scales, and numerators.

The 4L cases are later compatibility targets; they must not widen the initial
one-loop validation gate.

### `tests/integral_evaluation_freeform_tests.rs`

- decorated/namespaced index atoms with alphaLoop, `23-56`;
- the analogous MATAD case, `60-95`;
- a 4L FMFT decorated-index case, `98-140`; and
- a 1L PySecDec rank-two external contraction, `143-178`.

The alphaLoop fixture mixes arbitrary scalar functions, a purely external
quartic term, `k^mu k^nu`, and `k.p`; it is the strongest preservation test
for nontrivial Symbolica atoms.

### `tests/integral_comparison_vs_pysecdec_tests.rs`

- five 1L comparisons at starts `14, 41, 68, 95, 122`: unit/non-unit mass,
  non-unit renormalization scale, tensor numerator, and two-external-vector
  contraction (`:14-146`);
- four 2L comparisons at starts `149, 178, 206, 234`: scalar, two pinch/LMB
  forms, and rank four (`:149-265`); and
- three 3L comparisons at starts `268, 301, 341`: scalar and rank-four
  alphaLoop/MATAD (`:268-380`).

PySecDec is a numerical cross-check, never the proof of a parametric rule.

### `tests/integral_evaluation_pysecdec_tests.rs`

- three 1L reference tests at starts `20, 48, 84`: scalar, `(k.p)^2`, and
  spectator coefficients (`:20-117`);
- one 2L different-mass test at `120-154`;
- one 3L epsilon-order test at `157-193`; and
- five 4L tests at starts `197, 240, 286, 327, 368` through `:415`.

Only the one-loop group belongs in the first RustRed validation gate.

### Support and inline tests

`tests/test_utils.rs` contains no `#[test]`; it is nevertheless part of test
semantics. Exact expression comparison is `:165-204`. The two-backend
pipeline canonicalizes, optionally tensor-reduces, evaluates, and compares
numerically at `:260-506`; the reference pipeline does the same at
`:509-611`.

There are two inline smoke tests: dot expansion at
`src/lib.rs:5330-5352` and float precision at `src/utils.rs:360-371`. Both
print without a semantic assertion and should not count as acceptance of the
corresponding RustRed feature.

## Exact one-loop tensor fixtures and first validation gate

Let (D=4-2\epsilon), (T_a=\operatorname{topo}(I1L(m^2,a))), and leave
`I(1)` unsubstituted.

### Checked-in exact outputs

The first test inputs

```text
(k(1,1)*k(1,2) + k(1,3)*p(1,3))*T_1
```

and expects

```text
-(2*epsilon-4)^-1 * dot(k(1),k(1)) * g(1,2) * T_1
```

(`tests/tensor_reduction_tests.rs:14-37`). Since
`-(2*epsilon-4)^-1=1/D`, this is the rank-two projector; the odd `k.p` term
vanishes.

The second input is at `tests/tensor_reduction_tests.rs:49-60` and its exact
expected output is

```text
( dot(k(1),k(1))^2*g(1,2)
  -(2*epsilon-4)^-1*dot(p(2),p(3))*dot(k(1),k(1)) ) * T_1
```

(`:62-72`). This simultaneously checks pre-existing contractions, odd rank,
external contraction, and output dot notation.

The exact integrated 1L fixture uses the first numerator, sets
`susbstitute_masters=false`, and checks every epsilon coefficient at
`tests/integral_evaluation_analytic_tests.rs:24-85`; its leading result is

```text
epsilon^-1 * i*muvsq^2*g(1,2)/(64*pi^2)
```

(`:59-64`). This is a fully normalized evaluation target, not the raw
reduction coefficient.

### Required one-loop oracle matrix

Use only the concrete `I1L` topology and concrete integer powers in tests;
the implementation exercised must remain parametric in `a`, `D`, and `m^2`.

| Case | Numerator | Exact tensor result before denominator lowering |
|---|---|---|
| scalar | `1` | `1` |
| odd rank 1 | `k^mu` or `k.p` | `0` |
| free rank 2 | `k^mu k^nu` | `g(mu,nu)*k^2/D` |
| external rank 2 | `(k.p1)(k.p2)` | `dot(p1,p2)*k^2/D` |
| mixed rank 2 | `k^mu(k.p)` | `p^mu*k^2/D` |
| odd rank 3 | `k^mu(k.p1)(k.p2)` | `0` |
| free rank 4 | `k^mu k^nu k^rho k^sigma` | metric-pairing sum times `(k^2)^2/[D(D+2)]` |
| external rank 4 | `prod_i(k.pi)` | the three external-dot pairings times `(k^2)^2/[D(D+2)]` |
| mixed rank 4 | `k^mu k^nu(k.p)(k.q)` | `[g(mu,nu)dot(p,q)+p^mu q^nu+p^nu q^mu]*(k^2)^2/[D(D+2)]` |
| decorated indices | free indices such as `mink4(4,11)` | same identities with atoms preserved byte-for-byte modulo canonical ordering |

For every tensor row, test powers `a=1,2,3,4` and lower powers of `k^2` using

\[
 (k^2)^r I(a)=\sum_{s=0}^{r}\binom{r}{s}(m^2)^{r-s}I(a-s),
\]

with nonpositive/scaleless boundary integrals treated according to the
family's proven zero-sector rule. Then reduce every surviving `I(n)` with the
single parametric recurrence and compare coefficients of the mapped master.

The concrete scalar-power sweep `a=1..6` already exists at
`tests/integral_alphaloop_vs_matad_tests.rs:24-48`. The `(k.p)^2` denominator-
power-two fixture is at `tests/integral_evaluation_analytic_tests.rs:231-259`,
its spectator-coefficient variant at `:262-290`, and the two-external-vector
variant at `:293-320`. Decorated Symbolica indices and user functions are
covered at `tests/integral_evaluation_freeform_tests.rs:23-56`.

### Comparison normalization

Perform comparisons in three independently reported layers:

1. **Tensor layer:** exact `Atom` equality after canonical dummy-index and dot
   normalization; no scalar IBP or master value.
2. **Reduction layer:** exact equality of rational coefficients after mapping
   `RustRed I(1) <-> alphaLoop uvid(1,1)/ep`; no master epsilon series.
3. **Optional evaluated layer:** independently apply Vakint's measure,
   normalization, and master expansions, then compare Laurent coefficients.

A pass at layer 3 cannot compensate for a failure at layers 1 or 2.

## Advancement to two and three loops

Advance only after the full one-loop matrix passes exact symbolic comparison.

At two loops, first validate scalar sunset powers and pinches against the nine
guarded oracle recurrences, then mixed-loop tensors including the checked-in
fixture

```text
-(2*epsilon-4)^-1*dot(p2,p3)*dot(k1,k2)
+dot(k1,k1)*dot(k2,k2)*g(1,2)
```

from `tests/tensor_reduction_tests.rs:77-108`. Include alternate input LMBs
from `tests/input_matching_tests.rs:73-233` and
`tests/integral_comparison_vs_pysecdec_tests.rs:178-232`.

At three loops, validate every generated sector mapping and recurrence against
the 59-rule oracle inventory, then scalar, numerator, and rank-four fixtures
at `tests/integral_alphaloop_vs_matad_tests.rs:104-253` and
`tests/integral_evaluation_analytic_tests.rs:355-579`. Agreement is required
after canonical family/sector/master mapping; alphaLoop's display names and
master-value constants need not be reproduced.

## Compatibility checklist

### Topology and input

- [ ] Full and short topology syntax accept symbolic powers and mass-squared
      expressions.
- [ ] Arbitrary known-topology edge, node, and loop IDs canonicalize
      deterministically.
- [ ] Edge flips and loop-basis changes induce one simultaneous numerator map.
- [ ] Pinches retain a full exponent vector with explicit zeros.
- [ ] Family fingerprints include routing, masses, kinematics, denominators,
      and ISP definitions, not only graph shape or a name.
- [ ] Unknown/unsupported topology is distinct from a proved master.
- [ ] Arbitrary namespaced spectator functions and arbitrary index atoms
      survive.

### Tensor reduction

- [ ] No FORM invocation, FORM source inclusion, or text round-trip exists.
- [ ] Metric chains and already contracted internal indices are simplified.
- [ ] Odd internal rank is exactly zero.
- [ ] Even-rank metric pairings are generated algorithmically.
- [ ] Projector systems are solved exactly over Symbolica rational functions
      in symbolic `D`.
- [ ] Rank 2, 4, 6, 8, and 10 agree with the FORM golden tables.
- [ ] Rank beyond 10 is limited only by an explicit checked resource budget.
- [ ] External vectors, free metrics, user coefficients, and decorated indices
      are reconstructed correctly.
- [ ] Both indexed and dot-product input/output conventions are supported.

### Parametric IBP and application

- [ ] Production relation generation depends only on the generic family,
      loop/external momenta, denominator basis, and symbolic powers.
- [ ] Concrete topologies/powers appear only in validation/benchmarks.
- [ ] Every accepted rule replays to zero from generated parametric IBPs.
- [ ] Rules carry exact index guards and coefficient nonvanishing domains.
- [ ] Zero sectors and symmetry mappings are explicit and provenance-carrying.
- [ ] Rule selection is deterministic and every rule has a descent proof.
- [ ] Shift application is typed, simultaneous, and overflow checked.
- [ ] Equal integral keys combine exactly after every application step.
- [ ] Budget exhaustion/cycle detection never promotes the current integral to
      a master.
- [ ] The 1L recurrence specializes exactly to powers 1 through 6 above.
- [ ] Generated 2L rules reproduce all nine alphaLoop guarded recurrences.
- [ ] Generated 3L rules reproduce the 59 guarded and 14 late-transform oracle
      cases after canonical mapping.

### Output and validation

- [ ] Core output is coefficients times unsubstituted, stable master keys.
- [ ] The `I(1) <-> uvid(1,1)/ep` convention map is applied before 1L oracle
      comparison.
- [ ] Measure/MS-bar/master-series normalization is a separate adapter layer.
- [ ] Tensor, reduction, and evaluated comparisons are reported separately.
- [ ] One-loop scalar and all tensor rows pass before enabling the 2L gate;
      2L passes before 3L.
- [ ] Numerical Vakint/MATAD/PySecDec agreement supplements but never replaces
      exact symbolic rule replay.

## Immediate implementation consequence

The next RustRed work should not add another concrete recurrence. It should
connect the generic parametric IBP/rule engine to a native numerator pipeline:

```text
canonical input
  -> native tensor projector
  -> scalar products in family denominator/ISP coordinates
  -> generated guarded parametric rules
  -> canonical exact master sum
```

The first acceptance target is the complete one-loop matrix above, compared
structurally with the checked-in Vakint fixtures and the alphaLoop recurrence
under the explicit normalization map. Only after that exact gate should the
same generic machinery be exercised on the concrete two-loop and three-loop
oracles.
