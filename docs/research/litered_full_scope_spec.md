# RustRed mathematical capability and acceptance specification

Date: 2026-08-13

## Status and authority

> **Durable scope reference.** This frozen mathematical capability and
> acceptance specification is subordinate to [`GOAL.md`](../../GOAL.md).
> It records the intended generic surface and source-derived acceptance
> semantics; it is not a statement of current implementation status or
> sequencing authority.

Production RustRed must not contain authored topology-specific recurrence
formulae. Such formulae may exist only as oracles or regression fixtures for
rules rediscovered by the generic engine.

The principal mathematical/reference sources, separated by role, are:

1. [`LiteRed2026.m`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m), for the
   scalar-family, parametric-identity, sector-solving, and reduction scope;
2. the vendored [`symbolica`](../../vendor/symbolica) Rust source, for exact
   algebra and implementation APIs; and
3. the Vakint crate and its FORM resources in the
   [pinned GammaLoop source](https://github.com/alphal00p/gammaloop/tree/395610143576507503fd2c785db3ba62340f4277/crates/vakint),
   as a readable behavioral oracle for tensor projection, topology matching,
   rule application, and normalization.

RustRed is pure Rust plus Symbolica.  It must never invoke Mathematica or FORM.

LiteRed2 defines the conceptual capability target and provides mathematical
conventions and acceptance cases. It does **not** define RustRed's internal
architecture, public Rust API, intermediate pivot/rule order, mutable-state
model, performance characteristics, or accidental behavior. RustRed should
depart from LiteRed2 whenever a typed, generic, Symbolica-native, parallel, or
more efficient design preserves the explicitly tested mathematical semantics.

## 1. What “LiteRed scope” means

The public model is a generic integral family, not a named loop topology.  The
completed RustRed scope covers the following connected capabilities:

- independent denominator bases, automatic ISP completion, and conversion
  between scalar products and denominator powers;
- overcomplete denominator sets, their linear relations, independent bases,
  and partial-fraction reduction;
- loop and external momenta, external kinematics, dimension, cuts, sector
  patterns, and symbolic power shifts;
- ordinary parametric IBPs, separate Lorentz-invariance identities, and their
  combined system;
- exact shift-operator (`A`/`B`) conversion and round trips;
- integral and sector ordering;
- zero-sector analysis;
- internal, cross-sector, and cross-basis symmetry discovery from proved
  momentum transformations;
- adaptive discovery of guarded parametric recurrence rules;
- demand-driven rule selection and bottom-up application;
- master candidates, user master choices, and honest uncovered-domain state;
- stable persistence with family, ordering, domain, and provenance
  authentication;
- Feynman-parametric `U`, `F`, and Lee--Pomeransky facilities;
- Gram, dimensional recurrence, dimension-shift, and differential-system
  utilities; and
- graph/topology helpers and the denominator-only alternative interfaces that
  LiteRed exposes.

The relevant LiteRed implementation regions are:

| Capability | Source |
|---|---|
| Overcomplete sets and PF | [`LiteRed2026.m:465`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L465) |
| Basis construction | [`LiteRed2026.m:688`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L688) |
| Persistence | [`LiteRed2026.m:1153`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L1153) |
| Integral/sector conversion and ordering | [`LiteRed2026.m:1266`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L1266) |
| Parametric IBP and LI generation | [`LiteRed2026.m:1799`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L1799) |
| Feynman-parametric IBP/syzygies | [`LiteRed2026.m:1834`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L1834) |
| `A`/`B` operators | [`LiteRed2026.m:1924`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L1924) |
| Parametric sector solver | [`LiteRed2026.m:2254`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L2254) |
| Zero-sector analysis | [`LiteRed2026.m:2956`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L2956) |
| Momentum shifts and symmetries | [`LiteRed2026.m:3111`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L3111) |
| Cross-basis symmetries | [`LiteRed2026.m:3476`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L3476) |
| Master handling | [`LiteRed2026.m:3625`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L3625) |
| Rule selection and reduction | [`LiteRed2026.m:3801`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L3801) |
| `U`, `F`, LP, Gram, and factorization | [`LiteRed2026.m:4205`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L4205) |
| Dimensional/differential utilities | [`LiteRed2026.m:4612`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L4612) |
| Denominator-only algorithms | [`LiteRed2026.m:4840`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L4840) |
| Graph utilities | [`LiteRed2026.m:5527`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L5527) |

Fermat, Sparx, Mint, Singular, and similar programs used by optional LiteRed
paths are accelerators rather than semantic dependencies.  RustRed must use
Symbolica/Rust algorithms for the corresponding exact operations.  An
optional accelerator interface may be added later, but the reference engine
must remain self-contained.

## 2. Generic family algebra

For `L` loop momenta and `E` external momenta, the complete scalar-product
space involving a loop momentum has

\[
  N = \frac{L(L+1)}2 + LE
\]

coordinates.  A deterministic RustRed order is required, for example

```text
k_0.k_0, k_0.k_1, ..., k_{L-1}.k_{L-1},
k_0.p_0, ..., k_0.p_{E-1}, ..., k_{L-1}.p_{E-1}.
```

Every denominator is an affine-linear form in those coordinates,

\[
  D_r = c_r(p) + \sum_{t=1}^{N} A_{rt}(p) S_t,
\]

where the base coefficient field

\[
  K=\mathbb Q(\theta_0,\ldots,\theta_{P-1})
\]

is an authenticated Symbolica rational-function field over dimension,
masses, external invariants, routing parameters, and any caller-declared
algebraically independent parameters.  Constant rational routing coefficients
are a fast special case; the generic type must not require them.  Every family
datum--dimension, denominator row and constant, external Gram entry, and
power shift--must use exactly `K` and cannot contain an index variable.

An independent basis has exactly `N` rows and a generically invertible `A`
over that field.  Construction retains every input coefficient-denominator
guard and adds `numerator(det(A)) != 0`; it verifies both inverse products
exactly.  A shorter independent physical-propagator list may be completed with
deterministic ISP coordinates.  A longer or dependent list belongs to the
separate denominator-set/partial-fraction layer and must not be silently
accepted as a basis.

The shorter-list case may scan deterministic candidate coordinates for generic
rank increase and append zero-shift ISPs. The completed span is the parity
contract; literal ISP ordinals need not match Mathematica's symbolic ordering.

External-external products never become integral coordinates.  Their declared
kinematic values are symmetric coefficient-field elements; a singular
external Gram matrix is allowed.  External momenta are nevertheless declared
as an independent vector basis, so vector relations must be resolved before
family construction.  Thus contraction of a
derivative with an external momentum can produce both loop-external
coordinates and coefficient-field constants.

The canonical family record contains at least:

```text
name and schema version
ordered loop and external momentum identities
ordered coefficient variables and assumptions
dimension expression
ordered scalar-product coordinates
ordered denominators and physical/auxiliary kind
inverse denominator map
power shifts
cut mask and sector pattern
integral/sector ordering policy
kinematic relations
discovered zero/symmetry/rule data with separate provenance
```

All derived data is fingerprinted from the semantic input, including variable
roles and the chosen kinematic normal-form policy.  The first implementation
requires input already expressed in algebraically independent `theta`
variables; a future quotient-ring mode must authenticate and apply one fixed
Groebner normal form everywhere.  Process-local Symbolica symbol IDs are never
persistent identities.

## 3. Exact ordinary parametric IBPs

Use LiteRed's convention

\[
  J(n)=\int\prod_{i=1}^{L}d^d k_i
       \prod_{r=1}^{N}D_r^{-(n_r+\nu_r)},
\]

where `n_r` are symbolic integer lattice coordinates and `nu_r` are the
family's constant symbolic power shifts.  For every differentiated loop
`k_i` and every contraction momentum

\[
  q\in(k_1,\ldots,k_L,p_1,\ldots,p_E),
\]

precompute the affine denominator expansion

\[
 q\mathbin\cdot\partial_{k_i}D_r
   = \gamma_{riq,0}(p)+\sum_{t=1}^{N}\gamma_{riq,t}(p)D_t.
\]

The raw identity is then

\[
  0=\delta_{q,k_i}d\,J(n)
   -\sum_{r=1}^{N}(n_r+\nu_r)
      \left[
        \gamma_{riq,0}J(n+e_r)
        +\sum_{t=1}^{N}\gamma_{riq,t}J(n+e_r-e_t)
      \right].
\]

This yields exactly `L*(L+E)` rows.  RustRed uses a documented deterministic
contraction-major order, with differentiated loop the inner/minor coordinate,
to make artifacts and oracle comparisons reproducible.  Mathematical
equivalence of the normalized row set is the acceptance invariant; LiteRed2's
incidental enumeration order is not.  The implementation in
[`GenerateIBP`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L1813) establishes
the following mathematical invariants:

- `PowerShifts` modify only the coefficient multiplier `n_r + nu_r`;
- integral keys remain `J(n + delta)` and do not contain `nu_r`;
- equal shifts are combined exactly and zero coefficients are removed;
- raw generation performs no sector-zeroing, symmetry canonicalization, or
  concrete specialization;
- the output is a reusable function of all symbolic indices; and
- generation is topology and loop-count independent.

The canonical Rust representation is a sparse map

```text
IndexShift -> IndexedCoefficient
```

over a second authenticated Symbolica field

```text
K(n)=Q(theta_0, ..., theta_{P-1}, n_0, ..., n_{N-1}).
```

`K(n)` extends `K` in exactly that order and assigns distinct parameter/index
roles in its fingerprint.  Lifting `K -> K(n)`, affine index translation, and
guarded specialization/projection `K(n) -> K` are explicit checked
operations.  Ordinary Symbolica arithmetic is never allowed to extend or
reorder a proof-bearing variable map implicitly.  `K=Q` (no symbolic base
parameters) is valid.

Typed shifts, coefficients, and contexts are the database format.  Symbolica
`Atom` expressions and patterns are adapters for parsing, display, and public
substitution surfaces, not the sole source of rule identity.

## 4. Lorentz-invariance identities

LI identities are distinct from ordinary IBPs and number `E*(E-1)/2`.
LiteRed does not author a second unrelated derivative formula.  Write

\[
  k_i\!\cdot p_a=\beta_{ia,0}+\sum_t\beta_{ia,t}D_t,
  \qquad
  X_{ia}=\beta_{ia,0}T_0+\sum_t\beta_{ia,t}T_{-e_t},
\]

and, for a whole relation, define the atomic translation

\[
  T_s\!\left(\sum_\delta c_\delta(n)J(n+\delta)\right)
   =\sum_\delta c_\delta(n+s)J(n+s+\delta).
\]

If `B_bi` is the ordinary external-contraction row for
`p_b . d/dk_i`, then

\[
  M_{ab}=\sum_iX_{ia}B_{bi},\qquad
  LI_{ab}=M_{ba}-M_{ab}\quad(a<b).
\]

This fixes the exact sign and lexicographic pair order used by LiteRed.  In
algorithmic terms it:

1. rewrites every `k_i.p_a` into the weighted shifts `0` and `-e_t`;
2. takes the external-contraction ordinary IBP for `p_b.d/dk_i`;
3. translates both the integral shifts and every occurrence of the symbolic
   coefficient indices by the shift carried by `k_i.p_a`;
4. sums over loops; and
5. antisymmetrizes in external indices `a,b`.

The relevant code is the `lis=...` construction in
[`GenerateIBP`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L1813).
Translation of coefficient variables is essential: translating only integral
keys produces an incorrect LI relation whenever coefficients depend on an
index.  RustRed therefore needs one checked operation that translates an
entire parametric relation, not separate ad-hoc key manipulation.

Translation must obey `T_0 R=R`, `T_s(T_tR)=T_{s+t}R`, and guarded
specialization commutation.  `IBP`, `LI`, and `IBPLI` remain distinguishable
collections.  Solvers can select any combination, as LiteRed's `RRs` option
does.

### Guarded concrete specialization

For an integer assignment `a` of arity `N`, specialization is

\[
  \operatorname{Spec}_a\!\left(\sum_\delta
  c_\delta(n)J(n+\delta)\right)
  =\sum_\delta c_\delta(a)I(a+\delta).
\]

It simultaneously maps every parameter variable to itself and every index
variable to its assigned integer, evaluates the original numerator and
denominator separately, retains the mapped pre-cancellation denominator as a
nonzero guard, and checked-divides.  The result is bulk-remapped to exactly
`K`; unused index slots may not survive.  A zero mapped denominator is an
inapplicable/pole result, not a panic or a zero coefficient.  Key addition is
checked, and duplicate concrete keys combine only after coefficient
specialization succeeds.  Power shifts never enter concrete keys.

## 5. Shift operators

RustRed must preserve LiteRed's operator semantics:

\[
  A_i J(n)=n_iJ(n+e_i), \qquad B_iJ(n)=J(n-e_i).
\]

These operators are noncommutative at a fixed index.  Conversion to and from
operator form must be exact and tested by structural round trips, including
repeated operators, mixed `A_i/B_i`, coefficients depending on `n_i`, and
power-shifted families.  The source implementation begins at
[`LiteRed2026.m:1924`](https://github.com/rnlg/LiteRed2/blob/f02953115f0433d80318a92f3bc0b56a9bf51ce9/Source/LiteRed2026.m#L1924).

## 6. Sectors, cuts, and ordering

A sector is the sign domain

```text
active r:   n_r >= 1
inactive r: n_r <= 0
```

before any power shift is applied.  Cuts and sector patterns remove
inadmissible domains but do not alter raw identities.  Integral ordering must
be a named, serialized policy; changing it invalidates discovered rules.
LiteRed's default sector/integral measures include propagator count, corner
complexity, dots, and numerators.  RustRed may use a more efficient exact key,
but it must provide deterministic strict ordering and the corresponding
descent proof for every accepted rule.

## 7. Zero-sector discovery

The default LiteRed `AnalyzeSectors` path constructs `U` and `F` and applies a
monomial logarithmic-derivative rank criterion at candidate sector corners.
Zero information is then closed monotonically to subsectors, subject to cuts
and the sector pattern.  A separate corner-IBP solve is available as an
alternative.

RustRed must distinguish:

- a sector proved zero by an authenticated criterion;
- a sector excluded by a user cut/pattern;
- a sector not yet analyzed; and
- a failed or resource-limited analysis.

“No rule found” is never a zero proof.

An implementation must retain these states explicitly and use a
subsector-first dependency order. Recursive exceptional-locus closure,
solved-subsector dependencies, and master selection remain separate proof
obligations rather than consequences of queue exhaustion.

## 8. Symmetry discovery

Supplied denominator permutations may be accepted only after a proved
invertible loop/external momentum transformation.  Full discovery follows
LiteRed's two-stage strategy:

1. canonical signatures of restricted Feynman-parametric polynomials cheaply
   propose equivalent sectors; and
2. an exact linear momentum-map ansatz is solved and verified against every
   denominator, external invariant, cut, and power-shift constraint.

Successful maps create unique-sector, mapped-sector, and self-symmetry rules.
Cross-basis mappings are the analogous `FindExtSymmetries` capability.
Polynomial signatures are filters, not proofs.

## 9. Guarded parametric rule discovery

`SolvejSector` is an adaptive symbolic search, not a catalogue of clever
recurrences.  Its essential behavior is:

1. form the sector's integer sign constraints;
2. maintain uncovered cases in that domain;
3. specialize freshly generated parametric identities at a growing lattice
   of nearby points;
4. perform exact ordered sparse elimination;
5. choose a solved pivot and translate it back to symbolic indices;
6. derive an applicability guard from pivot/denominator zeros and from RHS
   terms that would leak into a forbidden or harder domain;
7. split the remaining integer domain into covered and exceptional cases;
8. increase search depth for still-symbolic cases; and
9. record genuinely uncovered points as current master candidates.

LiteRed2 demonstrates one sound sequence.  For a fully numeric point,
`SolvejSector` constructs
`Join[ids@@point, SR[basis]@@point] /. ZerojRule[basis]`: exact self-symmetry
relations are appended to the selected IBP/LI identities and proved zero
sectors are erased *before* rows enter elimination.  `WhenBad` is applied only
after a candidate solved row has been shifted back and patternized.  It rejects
coefficient singularities and any surviving right-hand-side integral whose
inactive index can enter the sector.  RustRed need not reproduce that internal
sequencing, elimination pivots, or intermediate rule form.  Every published
rule must nevertheless be proved against zero/symmetry rewrites, singularity
guards, sector containment, and strict descent.  Its compiled zero/symmetry
rewrites must be proof-bearing and replayable, and the publication proof must
show the accepted rule has the same mathematical guarantees.

The rule type must therefore contain:

```text
family and ordering fingerprint
source identity-set fingerprint
sector and symbolic lhs pattern
integer-domain guard
nonzero polynomial guard
strictly descending rhs
source rows and exact elimination trace
resource-budget/cancellation metadata
schema version and checksum
```

Application uses three-valued guard evaluation: applicable, inapplicable, or
undecidable.  Undecidable must not be treated as true.  Accepted rules replay
symbolically from generated source rows; finite integer samples are an
independent regression layer, never the proof.

The implementation must keep generated-row authentication, residual-domain
partitioning, conditional re-elimination, zero/symmetry quotients, and
publication as separate owned proofs. Interrupted or resource-limited work
cannot publish; an exhausted search remains `Uncovered`, and only an explicit
terminal policy may select a master.

An integral is a proved master only when the relevant analysis establishes
that status.  LiteRed's `MIs` is operationally the set revealed so far, so
RustRed should use explicit names such as `MasterCandidate`, `SelectedMaster`,
and `UncoveredDomain` instead of silently overclaiming.

## 10. Rule selection, application, and persistence

Reduction is demand-driven.  Starting from requested integrals, select only
reachable rules, close their sector dependencies, and compose them in a
bottom-up/layered order.  Cycles, non-descent, guard gaps, coefficient-domain
violations, and step/resource exhaustion are typed failures.

Persistence is data, not executable source.  A cache must authenticate:

- schema and RustRed algorithm versions;
- complete family and kinematic fingerprint;
- coefficient/index variable order;
- sector ordering and solver options;
- source identities and rule provenance;
- assumptions and exceptional guards;
- checksums and strict canonical ordering; and
- resource bounds needed for safe decoding.

Interrupted writes use a temporary file plus atomic rename.  A reservation or
lock record is distinguishable from completed data and cannot deserialize as
a valid reduction.

## 11. Native tensor numerator reduction

Vakint's readable tensor path supplies the behavioral oracle, not an allowed
runtime.  The audited implementation has these core semantics:

- odd loop-tensor ranks vanish when no external tensor direction is available;
- even ranks are expanded in all metric pairings;
- coefficients are obtained from the exact contraction Gram/projector system;
- the checked FORM resources spell out ranks 2, 4, 6, and 8 and use a rank-10
  table, but RustRed should generate pairings and solves algorithmically;
- loop/external dot products are rewritten through the selected family before
  scalar recurrence application; and
- topology normalization, loop-momentum maps, propagator signs, and numerator
  normalization must be preserved in both directions.

For families with external tensor directions, the covariant basis also
contains symmetrized products of external momenta and metrics.  RustRed should
generate this basis, build its exact Gram matrix with Symbolica coefficients,
solve the projector system, and cache it by tensor rank plus external Gram
fingerprint.  It must not embed Vakint's fixed-rank formulas as production
logic.

The final output remains a linear combination of unreplaced master topology
objects.  Substitution of analytic master values is deliberately outside the
first validation comparison.

## 12. Validation ladder

Each rung uses production-generic generation and concrete data only as an
oracle:

1. algebraic property tests for arbitrary small `L,E,N`, variable maps,
   translations, and exact specializations;
2. one-loop scalar families with masses and external momenta, comparing every
   generated parametric IBP/LI row against an independently constructed
   concrete derivative identity at many integer assignments;
3. one-loop tensor numerators of varied even/odd rank and mixed dot products,
   comparing exact unreplaced-master output to Vakint;
4. two-loop massive-vacuum scalar and tensor inputs, including reproduction
   of alphaLoop's guarded behavior without importing its rules;
5. three-loop vacuum families and rank-four or higher numerator cases;
6. four-loop massive-vacuum families; and
7. five-loop massive-vacuum families.

For parametric rules, a passing finite sample is necessary but insufficient.
Every rule must also replay symbolically from the generic identities and pass
its declared guard/descent checks. Ordinary/default RustRed and
Vakint-RustRed tests run in parallel with the licensed GMP Symbolica build and
never execute FORM or Mathematica. A separately declared, pinned
existing-backend oracle job may execute FORM outside the new RustRed mode and
production dependency graph.

## 13. Implementation-boundary principle

Development proceeds through vertical, topology-neutral slices whose
capability claims are limited to passing current evidence. Historical APIs,
loop-authored reducers, and bounded prototypes are Git evidence only; they do
not define the production architecture or satisfy any acceptance item above.
