# LiteRed2 mathematical reference

## Role and source identity

LiteRed2 is read-only mathematical and behavioral evidence for RustRed. It is
not a runtime dependency, an architecture template, a public-API contract, or
permission to copy Mathematica state machinery. RustRed implements accepted
semantics in typed Rust and delegates CAS operations to Symbolica.

The principal audited source is
[`LiteRed2026.m` at `f02953115f0433d80318a92f3bc0b56a9bf51ce9`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m).
The historical commit
`9a23bfe8dd87c8969b500427cc388800e14ca25c` supplies a frozen generated
oracle corpus for the triangle and `HQET1`--`HQET5` examples. Local copies,
when present, live under `FOR_REFERENCE_ONLY_DO_NOT_PUSH/LiteRed2` and never
enter RustRed history.

The historical LiteRed2 tree has unresolved redistribution status. Its full
rule tables are not copied into this MIT repository. Formulas below record
mathematical semantics and compact independently checkable fixtures.

## Source map

| Capability | LiteRed2 entry point |
|---|---|
| Independent family construction and ISP completion | `NewDsBasis` |
| Overcomplete denominator sets and partial fractions | `NewDsSet`, `Relations`, `NewDsBases`, `GeneratePFGB`, `PFReduce` |
| Integral/sector conversion and ordering | `Toj`, `Fromj`, `jSector`, `MakeOrderMatrix`, `jComplexity` |
| Ordinary IBP and LI sources | `GenerateIBP` |
| Optional Feynman-parametric syzygy sources | `GenerateFPIBP` |
| Shift-operator representation | `ToAB`, `ABIBPLI` |
| Ordered elimination and residual search | `Solvej`, `SolvejSector` |
| Zero sectors and Symanzik data | `AnalyzeSectors`, `FeynParUF` |
| Internal and cross-family symmetries | `FindSymmetries`, `FindExtSymmetries` |
| Demand-driven rule application | `IBPSelect`, `IBPReduce` |
| Master-basis changes | `IdentifyMIs`, `ToMIsRule` |

The conventional workflow is `NewDsBasis -> GenerateIBP -> AnalyzeSectors ->
FindSymmetries -> SolvejSector -> IBPReduce`. RustRed preserves the
mathematical separation, not this monolithic execution or mutable global
state.

## Family and integral semantics

For `L` loop momenta and `E` external momenta, the complete loop-dependent
scalar-product space has

\[
N=\frac{L(L+1)}2+LE
\]

coordinates. Each denominator is affine linear in those coordinates,

\[
D_r=c_r+\sum_{t=1}^{N}A_{rt}S_t.
\]

An independent family has `N` generically independent rows. A shorter physical
list is completed by deterministic ISP coordinates. A longer or dependent
list belongs to the separate `NewDsSet`/partial-fraction problem and must not
be silently accepted as a basis.

LiteRed's scalar integral convention is

\[
J(n)=\int\prod_{i=1}^{L}d^d k_i
     \prod_{r=1}^{N}D_r^{-(n_r+\nu_r)}.
\]

The raw sector is determined by the unshifted integer indices:

```text
n_r >= 1  active denominator
n_r <= 0  inactive denominator or numerator coordinate
```

Symbolic power shifts `nu_r` modify identity coefficients, not sector bits or
integral keys. Cuts and sector patterns restrict allowed sectors; they are not
analytic zero proofs. Denominator sign, metric convention, masses, external
Gram data, and index ordering are family data rather than hidden
normalizations.

## Parametric source formulas

For every differentiated loop `k_i` and contraction momentum
`q in {k_1,...,k_L,p_1,...,p_E}`, write

\[
q\!\cdot\!\partial_{k_i}D_r
=\gamma_{riq,0}+\sum_t\gamma_{riq,t}D_t.
\]

The ordinary parametric identity is

\[
0=\delta_{q,k_i}d\,J(n)
-\sum_r(n_r+\nu_r)
 \left[\gamma_{riq,0}J(n+e_r)
 +\sum_t\gamma_{riq,t}J(n+e_r-e_t)\right].
\]

Thus there are exactly `L(L+E)` ordinary rows. LI identities are kept as a
separate collection of size `E(E-1)/2`; their weighted translations shift both
integral keys and symbolic coefficient indices before antisymmetrization.
Raw generation performs no sector-zeroing, symmetry quotient, or concrete
target reduction.

LiteRed's `A_i` and `B_i` operators obey

\[
A_iJ(n)=n_iJ(n+e_i),\qquad B_iJ(n)=J(n-e_i),
\]

and are noncommutative at a fixed index. They are reference round-trip
semantics, not a required RustRed storage representation.

## Zero and symmetry semantics

The default zero-sector path constructs the restricted Lee--Pomeransky
polynomial `G = U + F`. For effective active variables `T`, each surviving
monomial `c_a x^a` contributes the coefficient-free row `[a_T, 1]`. Rank
strictly below `|T| + 1` is a sufficient scalelessness certificate. Full rank
is only failure of this criterion. Proved zero status descends to subsectors;
cut exclusion remains separately classified.

Symmetry discovery is two stage. Canonical forms of restricted parametric
polynomials cheaply group candidates. An exact invertible momentum
transformation must then reproduce every denominator and preserve external
kinematics, cuts, power shifts, and the loop-measure Jacobian. The signature is
never the proof. A generic transport permutes both integral shifts and the
symbolic index variables appearing in coefficients.

RustRed's target design improves candidate generation by using Symbolica's
graph canonization/automorphism machinery with physics-aware colors, followed
by the same topology-neutral exact momentum-map verifier. That candidate
ingress is not implemented at the current frontier.

## Residual solving and `WhenBad`

`SolvejSector` searches growing index-space diamonds, performs
complexity-ordered elimination, generalizes a useful pivot, and subtracts its
applicable domain from an exact residual case queue. It is not simply one
large elimination around the sector corner.

The audited `preparepoints` path keeps every translated source anchor in the
sector being solved. At a sector corner this turns the nominal signed L1 ball
into a one-sided cone: active coordinates may move upward and inactive
coordinates downward, while a noncorner anchor can move in either direction
only until the same-sector boundary. A rule that activates an inactive RHS
line is rejected rather than repaired with a higher-sector dependency.
`IBPReduce` later composes lower-sector dependencies. RustRed's bounded
`foundry::search::SectorSearchDiamond` mirrors this source-domain invariant;
an unrestricted signed ball is useful only as a negative diagnostic.

For an affine case `F(t) = b + A t` and local offset `delta`, correct
recentering is

\[
\sum_s c_s(F(t)+\delta)J(F(t)+\delta+s).
\]

Translation precedes substitution. One concrete anchor selects a candidate;
only the candidate's complete exact guard may cover a symbolic parent case.

`WhenBad` treats a parametric denominator as identically zero only when all of
its free-parameter coefficient polynomials vanish. Product equality is a
Boolean disjunction of factor equalities; product nonvanishing is a
conjunction. For case `C` and bad locus `B`, the rule applies on `C && !B` and
the exception `C && B` is requeued. An identically bad pivot publishes
nothing.

LiteRed's `MIs` are operationally uncovered integrals and its optional `NMIs`
count is a stopping heuristic. RustRed does not treat either as proof. A master
is terminal only through an explicit replay-bound selection policy; bounded
search failure remains unresolved.

## Demand-driven application

`IBPSelect` closes only the rules reachable from a requested expression.
`IBPReduce` composes lower-sector dependencies before higher sectors and
applies triangular within-sector layers. RustRed retains this demand-driven
dependency idea with typed integral keys, exact guards, memoization, cycle
detection, and stable master mapping. It need not reproduce LiteRed's rule
order or disk format.

## Frozen notebook fixtures

The three LiteRed2 notebooks are fixed by both Git blob and SHA-256:

| Fixture | Git blob | SHA-256 |
|---|---|---|
| `Examples/example1.nb` | `4e41f031e1d42eb3c33010a6f7cd4e39ae51b13f` | `3cae230449142a3788d489572f14bf4cdc6dbfb0202e419ba2972286e51ba9a0` |
| `Examples/example2.nb` | `3c5adc4ff9ee6cb2a4c8d5e169bfb7e40c296c3f` | `a16c3c23cc699196699467f805cbabfd8292bbb9798cfec47627c9be43a2bc27` |
| `Examples/NewDsSet.nb` | `2d3878e908998cdfe1d73c21339ea7534b3b2c38` | `e1d9b2d6e599f727f641990300ebeaef5399fb4f0d8a3504bc5b0d88e398fbc2` |

Their acceptance roles are:

| Fixture | `L,E,N` | Structural rows | Distinguishing behavior |
|---|---:|---:|---|
| one-loop massive triangle | `1,2,3` | 3 ordinary, 1 LI | family construction, sectors, masters, differential systems, reduction |
| reverse-unitarity `gr1/gr2` | `2,2,7` | 8 ordinary, 1 LI per family | cuts, graph attachment, cross-family symmetry, master-basis changes |
| related HQET families | independent bases `3,1,9`; overcomplete `HQET5` has 11 indices | 12 ordinary per independent family | dependent denominators, partial fractions, new bases, cross-family mapping |

The historical generated corpus contains 354 files for the triangle and
`HQET1`--`HQET5`: relations, sector data, masters, rules, symmetries, and
partial-fraction evidence. These are frozen external oracles, not a test
harness or production tables.

Its useful structural census is:

| Family | zero / nonzero sectors | mapped / unique sectors | masters |
|---|---:|---:|---:|
| triangle | 1 / 7 | 2 / 5 | 5 |
| `HQET1` | 209 / 47 | 22 / 25 | 7 |
| `HQET2` | 193 / 63 | 6 / 10 | 1 new |
| `HQET3` | 198 / 58 | 1 / 3 | 0 new |
| `HQET4` | 198 / 58 | 1 / 2 | 0 new |
| `HQET5` | 232 / 24 | partial-fraction family | pending independent-basis interpretation |

Counts constrain classification and mapping; they do not prove RustRed rule
closure.

The eight published LiteRed 1.x notebooks additionally cover: a one-loop
off-shell vertex; two-loop on-shell propagator and vertex families; three-loop
propagator and vertex families; a two-loop box with two invariants; a
four-loop massive tadpole; and three related two-loop bases with cross-family
symmetries. They supply generic input/identity/sector and later target oracles,
not topology-specific production recurrences.

## Independent two-loop vacuum fixture

For

\[
D_1=k_1^2-s,\quad D_2=k_2^2-s,\quad
D_3=(k_1+k_2)^2-s,\qquad s\ne0,
\]

define `J(a,b,c)` with inverse scalar-product map

\[
k_1^2=D_1+s,\quad k_2^2=D_2+s,\quad
2k_1\!\cdot\!k_2=D_3-D_1-D_2-s.
\]

The family has full `S_3` denominator symmetry. Its zero sectors are `000`,
`001`, `010`, and `100`; the nonzero sectors are `011`, `101`, `110`, and
`111`. For generic `d` and `s`, choose

\[
S=J(1,1,1),\qquad P=J(0,1,1).
\]

Compact exact reductions include

\[
\begin{aligned}
J(2,1,1)&=\frac{d-3}{3s}S,\\
J(2,2,1)&=\frac{(d-2)(d-3)}{9s^2}S
          -\frac{(d-2)^2}{12s^3}P,\\
J(3,1,1)&=\frac{(d-8)(d-3)}{18s^2}S
          +\frac{(d-2)^2}{12s^3}P,\\
J(0,2,1)&=\frac{d-2}{2s}P,\\
J(-1,1,1)&=sP,\\
J(-2,1,1)&=s^2\left(1+\frac4d\right)P.
\end{aligned}
\]

These equations follow directly from the defining IBPs, factorization, and
global tensor averaging. They are FORM- and Mathematica-free goldens. The
production engine must rediscover them from the generic family and generated
rows; none may be a topology-dispatch recurrence.

Changing to denominators `s-k^2` applies the parity map
`J_tilde(a,b,c) = (-1)^(a+b+c) J(a,b,c)`. A comparison must apply that map
rather than editing individual coefficients.

## What RustRed deliberately does not copy

RustRed does not copy Mathematica global definitions, delayed rules, disk
format, notebook APIs, pivot order, heuristic master promotion, Fermat/FORM
accelerators, or topology-authored recurrence tables. Polynomial signatures
remain candidate filters, finite searches remain discovery tools, and notebook
outputs remain external validation. The [validation ladder](../validation.md)
defines what agreement is required at each stage.
