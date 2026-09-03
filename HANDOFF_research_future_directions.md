# Future directions for K6 closure and exact scalable completion

**Research snapshot:** 2026-09-03
**Scope:** literature synthesis only; no implementation was performed for this
handoff.
**Status:** proposals below are future work, not RustRed capabilities or closure
claims.

Companion plain-language explainer: [`K6_problem_explained.typ`](K6_problem_explained.typ)
and the compiled [`K6_problem_explained.pdf`](K6_problem_explained.pdf).

## 1. Executive conclusion

The sentence

> Janet queue exhaustion alone is not family closure if the monomial
> complement remains infinite.

is mathematically correct. An exhausted Janet queue says that the supplied
relation module has been completed for the chosen involutive division and
ordering. It does **not** say that the quotient by that module is finite. A
Noetherian Janet completion can terminate for a positive-dimensional module
whose standard-monomial complement contains infinite rays.

Two sharply different failure modes must therefore be kept separate:

1. **RustRed's current measured K6 failure is pre-exhaustion.** Every recorded
   exact K6 campaign stopped on coefficient growth or divisor-work limits before
   the Janet queue exhausted. There is no certified post-Janet ray count yet.
   The immediate cure is a more scalable, proof-producing exact-completion
   architecture.
2. **A future exhausted queue with an infinite complement would be a structural
   result.** It would mean that the *modeled* relation module, coefficient
   localization, sector chart, or boundary coupling is insufficient to give a
   finite quotient. More translations of the same completed generators and a
   different admissible term ordering cannot change that fact. One must add
   semantically valid relations or repair the algebra/domain being modeled.

The strongest literature-backed program is consequently:

- first reach real queue exhaustion using shift-aware modular F4-style
  discovery, signatures, trace reuse, an ordering portfolio, and one exact lazy
  replay;
- if the exact complement is infinite, compare localized momentum IBPs with
  first-order and then higher-order parametric annihilators, audit the
  positive-shift chart against the full double-shift problem, and restore all
  boundary, exceptional, symmetry, factorization, and possible supersector
  relations;
- before completion, use generating-function, seedless, or syzygy-derived
  lowering operators to expose useful leaders without blind seed growth; after
  true completion, use them structurally only if an audit proves that they
  recover a relation omitted by the current chart, boundary coupling,
  localization, or authorized generator set;
- certify the resulting finite quotient either by a completed Gröbner/Janet
  basis or, if a good monomial orientation remains pathological, by a
  rank-matched finite border/contiguity representation with exact flatness and
  guard checks.

No reviewed publication provides a turnkey, proved, practical high-loop
algorithm, nor a turnkey solution for this six-coordinate K6 family. The
proposal above is a synthesis whose stages are individually
falsifiable. It deliberately separates probabilistic discovery from exact
authority.

## 2. What the current K6 evidence does and does not show

Here `K=6` denotes the complete six-coordinate scalar-product family for the
three-loop single-scale vacuum problem. The older eager exact Janet/Ore study
started from nine input rows and reached roughly 59--91 basis rows over
861--3,177 completion iterations, depending on the orbit. It then hit resource
stops:

- transient exact numerator additions required approximately 16.8--25.2
  million polynomial terms at the standard study cap;
- after raising the cap on one orbit, the next addition's conservative
  preflight projection requested about 94.1 million terms; this was not a
  materialized canonical Symbolica output;
- other orbits hit 67,108,864 divisor visits;
- no queue exhausted, so the standard complement was never authoritatively
  constructed.

The separately reported owner-cover complements—10 boxes for one path lane, 4
for a star lane, and 3 five-dimensional boxes for one shallow S4a study—come
from finite semantic-source diagnostics. They are **not** residual rays after a
completed Janet basis.

The correct present diagnosis is therefore “exact completion was interrupted
by expression and search swell,” not “Janet completed and failed to close
K6.” The hypothetical structural branch below becomes relevant only after an
exactly certified queue exhaustion.

Later indexed, copy-on-write runs sharpened rather than overturned that
diagnosis. A natural-order orbit reached 88 rows and 4,097 iterations; an
alternate order reached 100 rows and 5,232 iterations before a raised
one-billion logical-visit cap. Indexed lookup removed the original flat
term-by-basis scan, and copy-on-write shared most unchanged rows, but hundreds
of millions of indexed queries and superlinear exact support growth remained.
An attributed rejected payload contained 1,826,367 numerator-plus-denominator
terms: source-provenance numerators were the largest component, physical-row
numerators were also substantial, and denominators were secondary.

### Current implementation boundary

The pushed exact-lazy milestone is a foundation, not a production completion
engine. It already provides hash-consed exact coefficient/provenance/guard
DAGs, immutable shared epochs, coefficient-free indexed Janet geometry, exact
normal forms against a frozen ingress epoch, and cold lowering/source replay.
It does
**not** yet provide production basis admission, lazy prolongation,
autoreduction/collision handling over the persistent epoch, or an exact-lazy
completion driver and exhaustion certificate. Those seams must be completed
before a decisive exact-lazy K6 run, or modular F4 must be developed as a
separate experimental completion backend with the same authority boundary.

## 3. Why queue exhaustion and finite closure are different

Let `J` be the left relation module that RustRed actually generated, and let
`in(J)` be its leading-monomial module for one fixed admissible order. A correct
exhausted Janet queue proves that the retained rows form an involutive basis of
`J`. Involutive bases are generally redundant Gröbner bases; Janet's
Noetherian property guarantees an algorithmic completion, not a
zero-dimensional quotient. This distinction follows from the foundational
involutive-basis work of [Gerdt and
Blinkov](https://arxiv.org/abs/math/9912027) and its extension to polynomial
algebras of solvable type by
[Seiler](https://arxiv.org/abs/math/0208247).

For one monomial component in `K` shift variables, the cheapest exact
finite-complement test is:

1. minimize the leading-monomial generators;
2. for every shift axis `E_i`, find a pure-power witness `E_i^a_i` in the
   leading ideal;
3. if an axis has no such witness, `1, E_i, E_i^2, ...` already exhibits an
   infinite standard ray;
4. if every axis has a witness, every standard monomial lies inside the finite
   box `0 <= e_i < a_i`; enumerate that box and filter it by divisibility;
5. for a free module, repeat the test in every module component.

Standard-pair or Stanley decompositions refine an infinite result. A standard
pair `(m, Z)` represents an entire cone `m K[Z]`; nonempty `Z` identifies the
free directions that a missing relation must cover. They are a useful targeted
diagnostic after the pure-power gate. See [Matusevich and
Yu](https://arxiv.org/abs/2005.10968) and [Hashemi, Orth, and
Seiler](https://doi.org/10.1007/s00200-022-00569-0).

### Ordering is a performance lever, not a dimensional cure

For the same exact module over the same coefficient field, finite versus
infinite quotient dimension is invariant under admissible term order. When the
quotient is finite, its dimension—the number of standard monomials—is also
invariant, although the basis size, complement shape, pure-power exponents,
and intermediate coefficient growth can change dramatically.

Consequently, an ordering portfolio may be decisive for making K6 computable,
but it cannot turn a genuinely positive-dimensional completed module into a
finite one. If two fully certified orders disagree on finiteness, either the
modeled module/localization changed or one certificate is wrong. FGLM and
ordinary border-basis conversion also start from known zero-dimensionality;
they do not create it ([Faugère et
al.](https://doi.org/10.1006/jsco.1993.1051)).

Likewise, after a true Gröbner/Janet completion, translating, prolonging, or
re-seeding the same generators adds only elements already in `J`. Such rows can
accelerate an unfinished computation, but they cannot enlarge the completed
leading ideal. This is the precise limit of the intuitive “walk upward and add
what is missing” strategy.

## 4. Why a physical finite basis can coexist with a modeled infinite one

[Smirnov and Petukhov](https://arxiv.org/abs/1004.4199) prove that, for a fixed
Feynman graph, the master space obtained from all momentum-space IBPs and all
integer index substitutions is finite. The proof is nonconstructive: it gives
neither a useful degree bound nor closing recurrence rules. It also does not
say that an arbitrary one-sided, sector-local presentation must already expose
that finiteness.

Thus a future certified infinite complement in RustRed should be interpreted
as a red flag about the presentation, not as infinitely many physical masters.
Possible lost information includes:

- inverse shifts or localization present in the all-integer problem but absent
  from a positive-monoid sector chart;
- coefficient saturation that was applied generically without retaining its
  exceptional zero branch, or was not applied when required;
- boundary terms and lower sectors treated as external sources without enough
  coupled relations;
- graph automorphisms, affine loop reroutings, scaleless zeros, or exact
  factorization identities;
- “magic” relations that are visible only in a parent/supersector or uncut
  family;
- polynomial shift relations that exist in the full parametric annihilator but
  are not generated by the chosen momentum-IBP presentation.

That list leads to a concrete structural decision tree rather than blind seed
growth.

## 5. Structural cure: audit and enlarge the exact relation module

### 5.1 Compare momentum IBPs with parametric annihilators

The strongest direct result is [Bitoun, Bogner, Klausen, and
Panzer](https://arxiv.org/abs/1712.09215). Under Mellin transform, annihilating
operators of the Lee--Pomeransky integrand `G^s` yield polynomial shift
relations, and all polynomial shift relations arise this way. Their modules
satisfy

```text
Mom  subseteq  Ann^1(G^s)  subseteq  Ann(G^s),
```

where `Mom` is generated by momentum-space IBPs and `Ann^1` contains
first-order parametric annihilators. The first inclusion can be strict before
localization. Whether momentum IBPs generate all annihilators after extending
coefficients to rational functions in the dimension and number operators is
left open in their Question 24.

This gives a decisive K6 experiment:

1. construct a complete-ISP Lee--Pomeransky representation, with the
   normalization of the transformed integral made explicit;
2. compute `Ann^1(G^s)` by syzygies for a K3 control and then one hard K6
   sector;
3. Mellin-transform the operators into RustRed's shift algebra, including the
   conversion from Bitoun et al.'s normalized integral to RustRed's
   normalization;
4. exactly normal-form them against the completed momentum-IBP module over the
   *same* coefficient field and guard branch;
5. prioritize operators whose leaders intersect diagnosed standard pairs or
   supply a missing pure-power axis;
6. retain only relations with exact parametric-annihilator or momentum-IBP
   provenance.

A nonzero remainder proves that this valid annihilator relation is missing
from the modeled localized `Mom` module. That can expose a construction/domain
mismatch, or a genuine `Ann` relation outside `Mom` if the open localized-
equality question fails for this family. Membership `P in Ann(G^s)` makes the
parametric relation valid independently of a multiplier `q`. If RustRed instead
derives `P` from a certificate `q P in Mom` by dividing through by `q`, that
*momentum-IBP derivation* is valid only on `q != 0`, and `q = 0` must become a
separately completed exceptional branch. RustRed already works generically
over rational functions in dimension and indices, so a generic coefficient
saturation may already be implicit; this comparison will also detect a
mismatch between intended and implemented coefficient domains.

If first-order annihilators are insufficient, degree-bounded second-order
annihilators are a justified next step. A four-loop form-factor calculation
reports sectors where first-order annihilators did not suffice and
second-order ones did; see [Lee et
al.](https://arxiv.org/abs/2309.00054). Full annihilator or Weyl-closure
computation should be the expensive last structural fallback, not the first
response to an unfinished Janet queue. Established localization algorithms
include [Oaku, Takayama, and
Walther](https://arxiv.org/abs/math/9811030).

### 5.2 Make localization and saturation explicit

For a polynomial coefficient factor `q`, the saturation `I : q^infinity` can
add an operator `P` when some power of `q` times `P` already lies in `I`. This
is valid on the principal open set `q != 0`; it does not prove the same rule on
`q = 0`. Every such generic rule must therefore own a nonzero guard, while the
exceptional locus is a separate closure problem.

Over RustRed's intended `Q(d,nu)` coefficient field, generic coefficient
saturation should already be implicit. A nonzero quotient between a computed
saturation and the modeled module would consequently diagnose missing
localized generators or a coefficient-domain construction error. Saturation
can lower dimension only by removing components supported on the inverted
locus; it cannot remove a generic positive-dimensional component.

Weyl closure, schematically

```text
D intersect K(x)<partial> I,
```

can reveal polynomial operators hidden after rational localization. It changes
the open-set presentation; it is not an ordering trick or a substitute for
finishing the current Janet queue. Algorithms are given by [Oaku, Takayama,
and Walther](https://arxiv.org/abs/math/9811030), while practical
implementations commonly require finite holonomic rank. This makes Weyl
closure an expensive, late diagnostic after the first-order saturation probe.

Inverting a shift `E_i` is different again: it changes the sector-positive
monoid into a Laurent lattice and permits crossing physical sector boundaries.
It is valid only when every crossing has an exact boundary/subsector owner.

### 5.3 Audit the full double-shift algebra against the sector chart

[Barakat et al.](https://arxiv.org/abs/2210.05347) formulate IBP reduction as
a left ideal in a rational double-shift algebra. This is closer to the
all-integer-shift setting of the finiteness theorem than a sector-local
`N^K` chart. A small K3/K6 pilot can answer whether RustRed's one-sided chart
loses a relation that becomes obvious with invertible forward/backward shifts.

This is a diagnostic and derivation mechanism, not permission to invert shifts
globally. A backward shift can cross a sector wall, where a different boundary
owner or zero rule applies. Every relation brought back to the positive chart
must therefore carry explicit validity guards and exact lower-sector routing.
The cited work also finds full noncommutative Gröbner computations expensive in
nontrivial examples, so a wholesale double-shift replacement is not the first
scaling recommendation.

### 5.4 Restore boundary, symmetry, factorization, and supersector relations

The generating-function formulation of [Feng et
al.](https://arxiv.org/abs/2605.09541) makes lower sectors explicit boundary
sources. RustRed's module-equivalence audit must similarly include:

- every lower sector as immutable, already closed feedback;
- all graph-isomorphism and affine-routing relations derived generically, not
  topology-name dispatch;
- scaleless-sector zeros and exact product/factorization identities;
- every guard-zero descendant of a generically divided leading coefficient;
- parent/supersector relations when the child sector alone loses all of its
  generating-sector terms.

The last item is not merely hypothetical. [Crisanti, Frellesvig, Pokraka, and
Smith](https://arxiv.org/abs/2605.29789) connect “magic relations” with
higher-dimensional critical varieties. Their diagnostic should be reproduced
in a RustRed-compatible exact framework before blaming term order or accepting
an infinite sector-local complement.

The complete logarithmic-vector-field generators of [Böhm et
al.](https://arxiv.org/abs/1712.09737) are another principled source of
dimension-preserving IBP vectors. Their completeness is for the
no-dimension-shift constraint, not for every closing recurrence, so they are a
source generator rather than a closure certificate.

## 6. Target the diagnosed rays instead of widening seeds blindly

Before a module is complete, several recent methods can use its provisional
uncovered geometry to expose a useful operator much earlier than blind Janet
completion. After a truly completed module leaves an exact standard pair,
these methods can change quotient dimension only if they recover a relation
omitted by RustRed's chart, localization, boundary coupling, or authorized
generator set. If they derive only another consequence of the same completed
momentum-IBP module in the same algebra, they are alternative representations
or performance aids, not structural cures.

### 6.1 Generating-function descendants

[Feng et al.](https://arxiv.org/abs/2605.09541) package each sector into a
generating function, turn IBPs into differential-operator equations, extract
symbolic rules, and use the remaining lattice geometry to select descendant
equations for the next iteration. This closely formalizes “walk upward, inspect
what remains, and repeat.” It is an excellent adaptive *pre-completion* source
strategy for K6.

Its limitation is important: descendants of the same equations cannot enlarge
an already correctly completed ideal. They help by avoiding a catastrophic
global completion, by exposing a better generator, or by revealing an encoding
omission. The paper's unknown-master validation by agreement of selected paths
is useful evidence but not a global confluence or closure proof, and it gives no
general high-loop termination or complexity bound.

### 6.2 Seedless and syzygy-constrained lowering operators

[de la Cruz and Kosower](https://arxiv.org/abs/2602.22111) build generic-index
lowering operators from IBP-generating vectors on triangular sublattices,
including bulk and boundary stages. Such operators are attractive strong
generators for a required pure-power or border leader. The examples also show
why exceptional hyperplanes and daughter sectors cannot be an afterthought;
some propagator-lowering construction remains future work in their hardest
example, so the paper is not a universal K6 closure theorem.

[Smith and Zeng](https://arxiv.org/abs/2507.11140) use syzygy-constrained
symbolic rules and small local systems to avoid artificially raised propagator
powers. This can precondition the K6 source set and reduce swell. Failure of a
bounded local solve does not prove that its target is a master.

[Liu and Mitov](https://arxiv.org/abs/2512.05923) offer diagonalized and fully
triangular recurrence systems. This is another promising representation after
a finite basis is known, but it does not yet supply a universal K6
finite-complement certificate.

Every imported candidate from these methods must be exactly replayed from an
authorized identity—or explicitly broaden RustRed's authority to verified
parametric-annihilator or correctly transported cross-domain relations—and
must be followed by the same finite-complement, guard, and confluence tests.

## 7. A rank-guided finite border as a second closure representation

The Lee--Pomeransky critical-point method ([Lee and
Pomeransky](https://arxiv.org/abs/1308.6676)) and the Euler-characteristic
formulation of Bitoun et al. give an independent generic master-count target.
The simple critical-point/Milnor-number count assumes proper isolated critical
points; higher-dimensional critical varieties require the more general
Euler-characteristic or regulated treatment.
The count must be aligned carefully with sector allocation, generic versus
special dimension/kinematics, graph symmetries, cross-graph relations, and
factorization conventions. A count alone supplies no reduction rules.

It can, however, close a rigorous dimension argument. In one fixed localized
operator algebra and guard branch, let `J` be the exactly proven candidate
relation module and `I` the intended full relation module, with `J subseteq I`.
If:

- a finite order ideal `O` of `r` monomials spans `A/J` by exact border rules;
- all border compositions are flat/confluent;
- an independent, convention-matched calculation proves `dim(A/I) = r`;

then the surjection `A/J -> A/I` and the two dimension bounds force equality.
This turns a trusted rank into part of a certificate rather than merely a
heuristic master count.

[Rodriguez and Sattelberger](https://arxiv.org/abs/2510.23411) develop border
bases and flat connection matrices in the rational Weyl algebra. An analogous
shift/Ore representation could be valuable if every natural monomial order
causes an enormous Gröbner/Janet basis. This is research work, not a direct
library transplant: the published algorithms concern rational differential
operators, not RustRed's guarded sectorwise integer shifts, and not every
chosen order ideal admits a border basis.

A related compact representation uses Pfaffian/contiguity matrices to apply
arbitrary parameter shifts once a basis is known; see [Chestnov et
al.](https://arxiv.org/abs/2204.12983). Matrix determinants create guards, and
resonant specializations can change rank, so the same comprehensive branch
discipline remains mandatory.

## 8. Cure for the observed K6 expression swell

The literature does not offer a single exact switch that makes the present
inhomogeneous `Q(d,n)` Ore completion cheap. The best-supported architecture is
a proof-producing split between cheap discovery and exact certification. The
combined modular-F4, signature, trace, and Janet/Ore design below is an
**experimental synthesis**: the cited papers establish its ingredients in
related commutative, free-algebra, or solvable-type settings, but none proves
the combined algorithm for RustRed's guarded multivariate inhomogeneous Ore
module. Its algebraic assumptions and exact replay must be proved explicitly.

### 8.1 Shift-aware modular F4-style discovery

F4 combines many reductions into symbolic preprocessing plus sparse matrix
elimination rather than repeatedly expanding individual normal forms
([Faugère](https://doi.org/10.1016/S0022-4049(99)00005-5)). Modular methods for
noncommutative G-algebras both reduce intermediate coefficient swell and allow
parallel prime runs ([Decker, Eder, Levandovskyy, and
Tiwari](https://arxiv.org/abs/1704.02852)). This directly targets RustRed's two
measured bottlenecks: huge exact polynomial additions and tens of millions of
divisor/index operations. F4 does not by itself guarantee relief from lookup
work; matrix symbolic preprocessing and the indexed Janet selector must be
measured independently.

Finite-field sampling must respect the Ore action. Naively replacing a number
operator `n` by one value is unsound, because

```text
E^u c(n) = c(n + u) E^u.
```

For a prime `p` and point `a`, a translated coefficient leaf must therefore be
evaluated at `a + u`. Every modularly derived row needs a compact replay recipe
or coefficient circuit; a sampled scalar row is not enough to translate it at
another lattice point.

Useful modular lanes should learn whole structural traces—obligation order,
column support, reducer multiples, pivots, zero rows, and new leaders—from
several independent good prime/point pairs and validate them on held-out lanes.
Trace disagreement means relearn or fall back; it must never be resolved by
termwise voting.

### 8.2 Signature and syzygy pruning

Signature criteria can discard rewritable rows and known syzygies before their
large normal forms or matrices are constructed. Relevant foundations include
involutive/F5-style bases ([Gerdt, Hashemi, and
M.-Alizadeh](https://arxiv.org/abs/1306.6811)) and signature Gröbner bases with
syzygy and cofactor reconstruction in noncommutative settings ([Hofstadler and
Verron](https://arxiv.org/abs/2107.14675)).

A modular skip is a discovery optimization, not an exact proof. The final
replay must either carry an exact rewritability/syzygy witness or make the
history irrelevant by independently checking every required Ore critical pair
or Janet nonmultiplicative prolongation.

### 8.3 Reconstruct only retained outputs

[FiniteFlow](https://arxiv.org/abs/1905.08019),
[FireFly](https://arxiv.org/abs/2004.01463), and related finite-field work show
how evaluation/dataflow graphs and sparse rational reconstruction avoid huge
intermediate expressions. RustRed should reconstruct only coefficients of
retained final lowering/border rows, sharing known factors and translated
leaves. It should never interpolate every swelling rejected intermediate.

This cure is conditional. If the final exact coefficients are intrinsically as
large as the intermediate peak, reconstruction only moves the cost. In that
case the publishable representation should retain an exact factorized or lazy
arithmetic circuit, with deterministic evaluation and provenance, rather than
force expansion.

### 8.4 Exact replay and arithmetic discipline

After modular discovery, one deterministic replay over `Q(d,n)` must establish
authority. Immutable shared basis payloads and coefficient-free indexed Janet
lookup already exist in the pushed foundation. Remaining useful secondary
tools include fraction-free/projective elimination, delayed monic
normalization, exact content removal over the whole augmented row, production
integration of the persistent epoch, streaming cold replay, and garbage
collection of unreachable DAG nodes. Fraction-free Ore elimination has an
established algebraic foundation; see [Beckermann, Cheng, and
Labahn](https://doi.org/10.1016/j.jsc.2005.10.002).

These are secondary because the K6 measurements show numerator/provenance
support and divisor amplification, not denominators alone, dominate the old
eager representation. Merely raising the term cap or normalizing every row to
monic form already failed as a cure.

### 8.5 Ordering portfolio and parallelism

K6 has only `6! = 720` coordinate-priority permutations. Before an MCTS or
learned policy, all safe permutations can be screened cheaply with bounded
modular traces across every orbit, together with a small set of admissible
weighted/block orders. The score must use the worst orbit and track:

- new leader and pure-power progress;
- surviving standard-pair dimension;
- pending-obligation growth;
- matrix nonzeros and fill-in;
- translated-coefficient work;
- trace stability across primes and points.

This search chooses a less expensive representation; it cannot certify or
create finite codimension. Arbitrary linear coordinate changes are also unsafe
without proof because they can destroy the integer cone, shift action, and
sector guards. Independent prime/point/order lanes are algorithmically
parallel and can exchange only sparse traces and candidates, which is much more
RAM-efficient than copying full exact bases to every worker. Actual scheduling
must respect the active Symbolica license: the last recorded campaign admitted
only one concurrent licensed Symbolica process, so process-level lanes may
need to run serially unless the license or in-process parallel interface
permits otherwise.

### Pommaret and high-dimensional complement diagnostics

Pommaret division is not a drop-in termination cure: unlike Janet, it is
non-Noetherian, and a finite Pommaret basis exists only for quasi-stable leading
ideals. Generic coordinate changes can often reach suitable coordinates in
abstract commutative algebra, but arbitrary mixing of physical shift axes can
destroy RustRed's integer cone and guard semantics. Safe permutations remain
performance choices and cannot change colength; see [Hashemi, Orth, and
Seiler](https://doi.org/10.1007/s00200-022-00569-0).

At eventual `K=21`, run the cheap per-component pure-power gate before any
full standard-pair or Janet-cone decomposition. Complementary-decomposition
algorithms are exact and valuable for diagnosing the few unresolved
components, but their worst-case dependence on the number of variables makes
blind full-cone expansion a poor primary high-loop test.

## 9. Exact authority boundary

Modular computation in a general inhomogeneous noncommutative algebra is not a
closure proof. Decker et al. explicitly note that effective deterministic
verification of their modular method is known in the graded case, while their
general method is probabilistic. RustRed's K6 problem is inhomogeneous.

Therefore a publishable artifact must independently verify all of the
following over the exact coefficient domain:

1. every retained row belongs to the authorized relation module, by exact
   source/provenance replay;
2. every original generator reduces exactly to zero, so no source relation was
   lost;
3. every required left-Ore critical composition, or every Janet
   nonmultiplicative obligation under a proven criterion, reduces exactly to
   zero;
4. the leading module has a finite complement—first by pure-power witnesses,
   then by explicit standard-monomial enumeration;
5. every terminal is explicit, universal, and owned by the artifact;
6. every leading-coefficient, pivot, and determinant guard is exact, and every
   supported exceptional branch is independently closed;
7. symmetries, zeros, factorizations, and lower-sector routing replay exactly;
8. cold serialization/reload reproduces the canonical rows, guards, terminal
   set, and reductions independently of prime portfolio and worker count.

Only the first seven concern mathematical closure; modular agreement,
selected-path agreement, and finite sampled boxes are diagnostics.

## 10. Recommended falsifiable experiment sequence

No code should be written from this document until the next implementation
session rechecks the current APIs and Symbolica's public API for every CAS
primitive under consideration.

### Phase A — establish which failure branch K6 is in

1. Preserve K1 and K3 as exact controls for every new algebra backend.
2. First finish the missing production exact-lazy admission, prolongation,
   collision/autoreduction, and completion-driver seams—or build modular F4 as
   a separate experimental driver over the same immutable geometry.
3. Then build a shift-aware modular F4 prefix on one frozen hard K6 orbit.
4. Require exact-equivalent retained rows and obligations while measuring wall
   time, peak RSS, divisor/index queries, matrix fill, and exact replay size
   against the recorded eager baseline.
5. Add signature pruning only if an ablation shows that it removes substantial
   constructed work without destabilizing traces.
6. Screen all safe coordinate priorities modularly; select on worst-orbit
   behavior, then replay the winner exactly.
7. Run the complete exact K6 queue. A resource stop remains a resource result;
   do not construct or label final rays.
8. On queue exhaustion, run the per-component pure-power test and exact
   complement enumeration.

### Phase B — only if the completed complement is infinite

1. Persist each missing pure-power axis and standard-pair cone as the target of
   the next relation search.
2. Compute a convention-matched Lee--Pomeransky/parametric rank diagnostic,
   with symmetries and sector allocation explicitly documented.
3. On K3 first, then one hard K6 sector, compare `Mom` against
   `Ann^1(G^s)` after the intended localization.
4. Add exactly verified first-order annihilator remainders under explicit
   parametric-annihilator authority; guard any coefficient divisions used in
   their orientation or momentum-IBP derivation. Recomplete and measure whether
   complement dimension or the targeted cone changes.
5. If needed, try bounded second-order annihilators, then full annihilator/Weyl
   closure only as the expensive fallback.
6. In parallel, audit the double-shift versus positive-sector presentation and
   every lower-sector, symmetry, factorization, scaleless, and supersector
   owner.
7. Use generating-function descendants, seedless operators, or
   syzygy-constrained solves targeted at still-uncovered cones only when they
   expose an audited missing relation/domain class; otherwise reserve them for
   pre-completion acceleration or a compact equivalent representation.
8. If the quotient rank is known but monomial completion remains pathological,
   test a finite rank-guided border/contiguity representation and certify all
   border compositions and guards.

### Decision table

| Observed exact result | Meaning | Next action |
|---|---|---|
| Queue does not exhaust | Current computational representation is inadequate | Modular batch/trace/signature/order work; do not discuss final rays |
| Queue exhausts; complement finite | Relation model is structurally sufficient | Enumerate terminals, close guard branches, cold-publish even if nonminimal |
| Queue exhausts; complement infinite; `Ann^1` remainder nonzero | The valid relation is absent from modeled `Mom`, either by mismatch or a genuine `Ann` extension | Add it with exact annihilator provenance; guard only divisions used in its derivation; then recomplete |
| Queue exhausts; complement infinite; localized `Mom = Ann^1` | First-order saturation is not the cure | Audit full annihilator, sector/double-shift/boundary/supersector model |
| Rank-matched finite border is exact and flat | A finite representation exists without a useful Gröbner orientation | Use border/connection artifact if application still descends or terminates demonstrably |
| Different certified orders disagree on finiteness | Not a physical/order effect | Find the changed module/domain or certificate defect |

## 11. What not to do

- Do not call current owner-cover boxes “post-Janet rays.”
- Do not call a resource-stopped queue exhausted.
- Do not declare infinite cones to be a finite set of masters.
- Do not expect a different ordering to repair genuine positive dimension.
- Do not keep adding translations of the same generators after exact
  completion and claim that the ideal was enlarged.
- Do not globally invert coefficient or shift factors without retaining every
  exceptional/boundary branch.
- Do not treat modular zeros, agreement across primes, or reconstructed
  coefficients as exact source membership.
- Do not reconstruct huge rejected intermediates when only final rows matter.
- Do not use a known master count without aligning sector, symmetry,
  kinematic, and relation conventions.
- Do not interpret a direct-target or finite-seed reducer as an all-index
  parametric closure proof.
- Do not postpone K6 measurements indefinitely while optimizing abstractions;
  each completed slice should be tested on a frozen K6 prefix, followed by a
  full attempt once its gates pass.

## 12. Literature map and caveats

Publication status matters here. The recent 2025--2026 generating-function,
seedless, triangular, border-basis, and magic-relation works below are
preprints as of this research snapshot. They provide valuable algorithms and
experiments, not settled high-loop complexity or termination theorems. Older
foundational results are linked both to their preprints and, where useful, to
their peer-reviewed journal records.

### Mathematical closure and relation authority

- [Gerdt and Blinkov, *Involutive Bases of Polynomial
  Ideals*](https://arxiv.org/abs/math/9912027): Janet/Thomas involutive bases,
  completion, and their relation to Gröbner bases. Does not equate completion
  with zero-dimensionality. [Journal
  DOI](https://doi.org/10.1016/S0378-4754(97)00127-4).
- [Seiler, *A Combinatorial Approach to Involution and
  δ-Regularity*](https://arxiv.org/abs/math/0208247): involution in polynomial
  algebras of solvable type. Physical shift coordinates constrain which
  coordinate changes are admissible. [Journal
  DOI](https://doi.org/10.1007/s00200-009-0098-0).
- [Gerdt and Hashemi, *Comprehensive Involutive
  Systems*](https://arxiv.org/abs/1206.0181): parameter space decomposes into
  cells, each with its own involutive basis. Supports exact guard branches;
  full comprehensive systems may be expensive.
- [Smirnov and Petukhov, *The Number of Master Integrals Is
  Finite*](https://arxiv.org/abs/1004.4199): physical/all-IBP finiteness,
  nonconstructive. [Journal DOI](https://doi.org/10.1007/s11005-010-0450-0).
- [Lee and Pomeransky, *Critical Points and Number of Master
  Integrals*](https://arxiv.org/abs/1308.6676): independent generic rank via
  critical points/Milnor numbers, not reduction rules. [Journal
  DOI](https://doi.org/10.1007/JHEP11(2013)165).
- [Bitoun et al., *Feynman Integral Relations from Parametric
  Annihilators*](https://arxiv.org/abs/1712.09215): all polynomial shift
  relations from parametric annihilators; localized equality with ordinary
  momentum IBPs remains open. [Journal
  DOI](https://doi.org/10.1007/s11005-018-1114-8).
- [Barakat et al., *Feynman Integral Reduction Using Gröbner
  Bases*](https://arxiv.org/abs/2210.05347): rational double-shift formulation;
  structurally useful, computationally difficult. [Journal
  DOI](https://doi.org/10.1007/JHEP05(2023)168).
- [Rodriguez and Sattelberger, *Border Bases in the Rational Weyl
  Algebra*](https://arxiv.org/abs/2510.23411): finite-rank border/connection
  alternative; adaptation to guarded difference/Ore sectors is unproved.

### Strong symbolic source generators

- [Feng et al., *An Algorithm for the Symbolic Reduction of Multi-loop
  Feynman Integrals via Generating
  Functions*](https://arxiv.org/abs/2605.09541): iterative descendant equations
  and lattice coverage; no universal high-loop termination proof.
- [de la Cruz and Kosower, *Seedless Reduction of Feynman
  Integrals*](https://arxiv.org/abs/2602.22111): generic-index lowering
  operators; boundary and exceptional handling remain essential, and the
  hardest lanes are not all completed in the paper.
- [Smith and Zeng, *Syzygy-Constrained Symbolic Reduction
  Rules*](https://arxiv.org/abs/2507.11140): compact symbolic neighborhoods and
  controlled propagator degree; not a global closure certificate.
- [Liu and Mitov, *Untangling the IBP
  Equations*](https://arxiv.org/abs/2512.05923): diagonal/triangular recurrences;
  promising representation without a demonstrated universal K6 bound.
- [Böhm et al., *Complete Sets of Logarithmic Vector Fields for IBP
  Identities*](https://arxiv.org/abs/1712.09737): complete generator set for the
  no-dimension-shift constraint, not completeness of all reduction relations.
  [Journal DOI](https://doi.org/10.1103/PhysRevD.98.025023).
- [Crisanti et al., *Magic Relations and Critical Varieties of Feynman
  Integrals*](https://arxiv.org/abs/2605.29789): detects relation loss associated
  with higher-dimensional critical varieties and cuts/supersectors.

### Expression and linear-algebra control

- [Faugère, *A New Efficient Algorithm for Computing Gröbner Bases
  (F4)*](https://doi.org/10.1016/S0022-4049(99)00005-5): batched sparse linear
  algebra and symbolic preprocessing; no worst-case cure.
- [Decker et al., *Modular Techniques for Noncommutative Gröbner
  Bases*](https://arxiv.org/abs/1704.02852): parallel modular G-algebra
  computation; general inhomogeneous result remains probabilistic without
  independent exact verification.
- [Hofstadler and Verron, *Signature Gröbner Bases, Bases of Syzygies and
  Cofactor Reconstruction*](https://arxiv.org/abs/2107.14675): noncommutative
  signatures, pruning, and provenance reconstruction. [Journal
  DOI](https://doi.org/10.1016/j.jsc.2022.04.001).
- [Peraro, *FiniteFlow*](https://arxiv.org/abs/1905.08019) and [Klappert,
  Klein, and Lange, *FireFly*](https://arxiv.org/abs/2004.01463): finite-field
  dataflow and sparse rational reconstruction. They control intermediate
  expressions, not family closure.
- [Böhm et al., *Complete Sets of Logarithmic Vector Fields for IBP
  Identities*](https://arxiv.org/abs/1712.09737) and [Wu et al.,
  *NeatIBP*](https://arxiv.org/abs/2305.08783): smaller syzygy/module-intersection
  source systems can precondition completion.
- [Guan et al., *Blade*](https://arxiv.org/abs/2405.14621): block-triangular
  systems reduce application cost; not an all-index closure theorem.

The private SpideR workflow described in [Dlapa et
al.](https://arxiv.org/abs/2604.25916) is relevant chiefly as scaling evidence
for sparse bottom-up finite-field application over a very large dependency
graph. It provides neither a public generic symbolic-rule generator nor a
finite-complement certificate, so it is not a cure for the structural issue.

## 13. Recommended future direction in one paragraph

Resume with a proof-producing modular/exact completion split and force one
full K6 queue to an honest terminal result. If its exact complement is finite,
ship the finite nonminimal basis and stop searching for artificial minimality.
If it is infinite, stop tuning orders and run the localized
`Mom`-versus-`Ann^1(G^s)` comparison, double-shift/sector-boundary audit, and
magic/supersector check against the exact standard pairs. Generate only the
relations needed to cut those cones. Parametric annihilators or correctly
transported boundary/supersector identities can genuinely enlarge an
underspecified module; generating-function descendants, seedless lowering, and
syzygies are structural cures only when they expose such an omitted class, and
otherwise remain completion accelerators. Exact-replay every retained
relation. Use an independently aligned master rank to guide—and, with a finite
flat border, potentially complete—the certificate. This is the most credible
route found that addresses both the algebraic infinite-complement risk and the
measured K6 expression swell without weakening RustRed's exact closure
standard.
