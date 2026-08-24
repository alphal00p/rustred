# Dependent symbolic residual starts: exact LiteRed contract and Symbolica design

Date: 2026-08-13

Status: source-audited implementation design. This note does not modify
production code. It extends
[`symbolic_residual_start_design_2026-08-13.md`](symbolic_residual_start_design_2026-08-13.md)
past literal-integer cylinders and assumes the already implemented
[`cylindrical_ordering.rs`](../../src/cylindrical_ordering.rs).

## Decision

RustRed should represent the first dependent `startp` layer as an
**integer-affine, idempotent substitution map** on the original denominator
indices. For example,

```text
n1 -> 3 - n2
```

is represented by the affine embedding

```text
F(t) = (3-t, t),                 t = n2.
```

A term is then a coefficient in the same authenticated Symbolica field
`K(n)`, restricted to the image of `F`, together with an ordinary ambient
`IndexShift q` and the semantic integral label

```text
J(F(t) + q).
```

This restores a sparse lattice without pretending that the restricted row is
a global `K(n)` identity. The map and the row must stay inseparable behind a
case-bound wrapper.

The first implementation must support only integer-affine maps with a
canonical set of free original indices and unit-pivot oriented equalities.
Nonlinear, rational, cyclic, parameter-dependent, or otherwise unproved index
maps remain typed `Unsupported`; they are not sampled away, declared empty,
or promoted to masters. This is narrower than the syntax accepted by
LiteRed's Mathematica helper, but it is the largest class justified by the
source while retaining a simple exact shift lattice and replayable ordering.

The operation order is mandatory:

```text
canonical generated row
  -> translate by an ambient prepare-point displacement
  -> simultaneously apply the residual index substitution
  -> eliminate only inside the retained equality locus.
```

Substitution before translation is wrong for a dependent map.

## 1. What LiteRed actually produces and consumes

The governing source is the current
[`LiteRed2026.m`](../../vendor/LiteRed2/Source/LiteRed2026.m), especially
`SolvejSector`, `SmartReduce`, `cf`, `gatherRules`, `preparepoints`, and
`WhenBad`. Mathematica is read-only specification material here; none of the
claims below depends on executing it.

### 1.1 Residual Boolean normalization

`SolvejSector` starts from the result of its `jRules` option and later rebuilds
the residual set from the old cases and all accumulated bad conditions
([`LiteRed2026.m:2372`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2372),
[`LiteRed2026.m:2522`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2522)).
`RulesToCondition` merely changes each rule to an equality and forms an OR of
ANDs
([`LiteRed2026.m:2586`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2586)).

`SmartReduce` then:

1. splits the outer disjunction into branches;
2. splits each conjunction into connected components according to shared
   index variables;
3. calls `Reduce[..., Integers]` on each component together with the sector
   half-line constraints for the indices in that component; and
4. passes the result to `cf` before rebuilding the disjunction
   ([`LiteRed2026.m:2573-2575`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2573)).

The `Reduce` call has a 300-second fallback to its unsimplified input. This is
important for parity language: `SmartReduce` is a best-effort integer-domain
normalizer, not a formal declaration that every residual branch has a solved
substitution map.

### 1.2 The exact syntactic contract of `cf`

The final parser is more permissive in expression shape, and more conservative
on failure, than an “affine solver” description would suggest
([`LiteRed2026.m:2578`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2578)).

- `True` and `False` pass through.
- `Or` is handled branch by branch.
- Integer declarations are normalized.
- Generated `C[k]` constants from `Reduce` are accepted only through a narrow
  pattern: each must be declared integral, have one usable bound, and be
  connected to an active or inactive index boundary in one of the explicit
  forms recognized by `cf`. If this reconstruction fails, `cf` returns
  `True`.
- Trivial sector facts `n_i >= 1` and `n_i <= 0`, and index integrality facts,
  are removed.
- Every remaining conjunct must syntactically be an equality with one side
  equal to one of the index symbols. Otherwise `cf` returns `True`.
- The accepted equalities are expanded and oriented as `n_i == rhs`.

The right-hand side is matched by `_`; `cf` does **not** check that it is
affine, polynomial, acyclic, or even independent of other bound indices. The
local source therefore accepts a branch language of the form

```text
True | False | Or[ And[ n_i == arbitrary_rhs, ... ], ... ]
```

after its special generated-constant processing. This is a syntactic
acceptance statement only. It is not evidence that arbitrary nonlinear maps
survive every later ordering, recentering, and `WhenBad` operation.

There are two further conservative boundaries:

- after rebuilding `noRules`, LiteRed aborts if a Mathematica `_Rational`
  occurs
  ([`LiteRed2026.m:2522-2523`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2522));
- `WhenBad` returns `True`, meaning “always bad”, if its simplified result
  contains a noninteger power
  ([`LiteRed2026.m:2565-2568`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2565)).

The source does not define `ToRules`; it relies on Wolfram's built-in
conversion. RustRed should not make correctness depend on undocumented branch
orientation by that built-in. It should normalize its own typed equality
predicates and retain their source ordinals.

### 1.3 `gatherRules` defines “contiguous” exactly

For each rule map `r`, LiteRed constructs

```text
start(r) = J(n1,...,nN) /. r.
```

Two cases are gathered when the expanded difference between their resulting
index vectors is a vector of literal Mathematica integers
([`LiteRed2026.m:2419-2425`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2419)).
Thus

```text
(3-n2, n2)    and    (4-n2, n2)
```

are contiguous, while

```text
(3-n2, n2)    and    (3-2*n2, n2)
```

are not. Grouping does not merge their logical cases. It schedules cases whose
start maps differ by an ambient integer vector, then keeps a separate target
case for rule attachment.

Groups are prioritized by the number of rule bindings, the number of literal
integer start components, and the positions of symbolic components in the
configured sector ordering. The current Rust port may use a different named,
replayed tie-break, but it must retain the exact contiguous-equivalence test.

### 1.4 `startp` and `preparepoints`

For a selected group, LiteRed computes

```text
startps = (indices /. case) for every case
startp  = first(startps)
```

([`LiteRed2026.m:2430-2447`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2430)).
An index is considered still symbolic when it is absent from the left-hand
sides of the first rule map. A bound right-hand side may itself contain a free
index.

For a partly symbolic start, `preparepoints` forms every exact L1-shell
displacement `delta`, but applies the sector sign test only at positions whose
`startp` component is a literal `_Integer`
([`LiteRed2026.m:2698-2710`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2698)).
In particular, `3-n2` is not sign-filtered. The shell itself is the signed
composition construction in
[`LiteRed2026.m:6094-6097`](../../vendor/LiteRed2/Source/LiteRed2026.m#L6094).

For a numeric group, the other overload unions shells around every numeric
start and checks the complete sector
([`LiteRed2026.m:2682-2695`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2682)).
Only the numeric branch adds `SR` and `ZerojRule`; a dependent symbolic start
must not silently use that numeric quotient
([`LiteRed2026.m:2471-2475`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2471)).

### 1.5 Recentring is a free-variable substitution on the complete solved rule

After a pivot is found, LiteRed replaces every still-free index `t_alpha` in
the **complete solved rule** by

```text
t_alpha -> 2*t_alpha - lhs_alpha.
```

It does this at
[`LiteRed2026.m:2483-2484`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2483),
then selects a remaining case whose start vector equals the transformed LHS
([`LiteRed2026.m:2485-2489`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2485)).
Consequently, recentering transforms coefficients, guards, and every integral
label together. It is not merely subtraction of a pivot from stored term
keys.

The exact affine formula is derived in section 6 below. It explains both
integer-cylinder pivot displacement and dependent starts such as
`n1 -> 3-n2`.

## 2. Existing RustRed representation and the missing operation

RustRed already has the correct global primitives:

- [`IndexShift`](../../src/parametric_relation.rs#L175) is an arity-checked
  `i64` displacement in the ambient integral lattice.
- [`ParametricRelation`](../../src/parametric_relation.rs#L317) stores a sparse
  `BTreeMap<IndexShift, ParametricCoefficient>` plus every guarded nonzero
  condition.
- [`ParametricRelation::translated`](../../src/parametric_relation.rs#L808)
  translates keys and every index occurrence in coefficients and guards as
  one operation.
- [`ParametricCoefficientContext::translate`](../../src/parametric_coefficient.rs#L1188)
  implements `n -> n+delta` in Symbolica's exact rational-polynomial field.
- [`PartialIndexAssignment`](../../src/parametric_coefficient.rs#L105) and
  [`ParametricRelation::partially_specialized_on`](../../src/parametric_relation.rs#L950)
  already provide a replay-bound special case for literal equalities
  `n_i=a_i`.
- [`SymbolicSectorCasePartitionCertificate`](../../src/symbolic_sector_cases.rs#L307)
  owns exact `p=0`/`p!=0` branch predicates and replays the complete finite
  partition.
- [`CoordinateEqualityLocusExtractor`](../../src/coordinate_equality_loci.rs#L798)
  currently recognizes only associates of `n_i-c`; all other predicates stay
  explicit and unresolved.
- [`ParametricEliminationOrdering`](../../src/parametric_elimination.rs#L30)
  orders a finite shift set at a concrete anchor, while
  [`CylindricalParametricEliminationOrdering`](../../src/cylindrical_ordering.rs#L71)
  supplies the first formal symbolic ordering for literal-integer cylinders.

Symbolica's relevant native operations are:

- `MultivariatePolynomial::replace` for substitution by a ring element
  ([`polynomial.rs:1778-1803`](../../vendor/symbolica/src/poly/polynomial.rs#L1778));
- `MultivariatePolynomial::replace_with_poly` for substitution by a polynomial
  on the exact same variable map
  ([`polynomial.rs:1937-1962`](../../vendor/symbolica/src/poly/polynomial.rs#L1937));
- `RationalPolynomial::from_num_den` for exact normalized reconstruction
  ([`rational_polynomial.rs:61-67`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L61));
  and
- an explicit numerator/denominator representation
  ([`rational_polynomial.rs:90-95`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L90)).

`replace_with_poly` asserts equal variable maps and can expand powers. A new
public proof boundary must therefore authenticate the map, preflight the
prospective term and integer-bit work, contain Symbolica panics, and retain
the mapped original denominator before `from_num_den` can cancel it. It must
not call `unify_variables`: RustRed's coefficient contexts deliberately reject
automatic map unification.

The missing operation is not “more pattern matching.” It is a typed,
simultaneous index-map composition which returns a locus-bound relation rather
than a global `ParametricRelation`.

## 3. Canonical affine residual map

Let the ambient family have `N` denominator indices. Choose increasing free
positions

```text
f = (f0,...,f_(r-1))
```

and write `t_alpha = n_(f_alpha)`. A canonical affine start is

```text
F_i(t) = b_i + sum_alpha A_(i,alpha) t_alpha,
```

with Symbolica/GMP integers `b_i` and `A_(i,alpha)`, subject to

```text
F_(f_alpha)(t) = t_alpha.
```

The last condition means that the free rows of `A` form the identity and their
constants vanish. It makes the same-context substitution

```text
sigma_F : n_i -> F_i(n_f)
```

idempotent. Its equality locus is

```text
L_F = { n in Z^N | n_i = F_i(n_f) for every bound i }.
```

For `n1 -> 3-n2`, using zero-based positions only in code,

```text
b = (3,0),
A = (-1,1)^T,
f = (2nd position).
```

An integer cylinder embeds exactly:

```text
F_i(t) = a_i     for a fixed coordinate,
F_i(t) = t_i     for a free coordinate.
```

### 3.1 Proposed certificate

The full general data model should be equivalent to:

```rust
pub struct ResidualAffineIndexMapCertificate {
    schema: &'static str,
    context_fingerprint: Arc<str>,
    source_material: GeneratedFixedPointMaterialLocator,
    source_work_item_ordinal: usize,
    source_case: SymbolicSectorCaseId,
    source_equality_predicate_ordinals: Box<[usize]>,
    ambient_arity: usize,
    free_positions: Box<[usize]>,
    bound_positions: Box<[usize]>,
    constants: Box<[symbolica::domains::integer::Integer]>,
    linear_coefficients: Box<[Integer]>, // row-major N by r
    literal_positions: Box<[usize]>,
    normalized_locus_polynomials: Box<[ParametricPolynomial]>,
    stable_manifest: Arc<str>,
    limits: ResidualAffineIndexMapLimits,
    stats: ResidualAffineIndexMapStats,
}
```

The exact Rust path for `Integer` may differ; the invariant is arbitrary
precision on the persisted algebraic map. `IndexShift` can remain `i64`
because generated IBP translations are finite lattice displacements. A
concrete query that cannot represent `F(t)+q` in `i64` is a typed concrete-key
boundary, not a reason to truncate the symbolic map.

Replay must resolve the source material and case, replay the partition, read
the cited equality predicates, repeat affine normalization and orientation,
reconstruct `b`, `A`, free/bound complements and locus polynomials, verify
idempotence, then compare the complete manifest and census. A stored matrix
without source predicate ordinals is not a certificate.

### 3.2 Equality normalization

The source partition stores expanded `ParametricPolynomial` predicates over
the authenticated ring `Z[theta,n]`. A usable equality may be multiplied by a
nonzero base-field factor:

```text
a(theta) * (z0 + z1*n1 + ... + zN*nN) = 0.
```

Because `a(theta)` is a unit in `K=Q(theta)`, its zero locus relative to the
formal coefficient field is the affine factor. The existing coordinate
recognizer already compares slope and intercept coefficient polynomials to
prove the special form `a(theta)*(n_i-c)`; the dependent extractor should
generalize that exact comparison, not factor by sampling.

A normalized affine row is accepted only when:

- total degree in index variables is at most one;
- no coefficient of an index depends on another index;
- after removing a proved common base-field factor, every affine coefficient
  is an exact integer;
- the primitive integer row has a chosen bound-variable coefficient `+1` or
  `-1`; and
- its oriented RHS uses only the declared free variables.

The unit-pivot restriction mirrors LiteRed's later rejection of rational
residual maps and avoids introducing hidden divisibility congruences. An
equation such as `2*n1+n2=0` is not equivalent over integers to the rational
map `n1=-n2/2`; it also carries a parity condition. It must remain
`UnsupportedNonUnitIntegerAffineEquality` until RustRed has a replayable
Smith-normal-form/congruence case language.

For multiple equations, the eventual compiler should perform deterministic
fraction-free elimination, retain every row operation, and accept only a
unit-pivot triangular result. Cycles or RHS references to a later bound
variable are not silently applied sequentially. The smallest slice in section
10 deliberately supports only one dependent unit-affine row, optionally in
addition to existing literal assignments.

### 3.3 What is deliberately outside V1

The source's wildcard RHS does not justify claiming support for:

- nonlinear maps such as `n1 -> n2^2`;
- rational maps or congruence classes;
- algebraic roots or noninteger powers;
- maps involving kinematic parameters or `d` as an index value;
- cyclic substitutions;
- arbitrary Boolean Presburger sets; or
- arbitrary nonlinear Diophantine solution parametrizations.

Each receives a typed unsupported reason retaining the original predicate and
case. A broader equality locus may still be searched by an earlier sound
fallback, but that fallback cannot claim dependent-`startp` parity.

## 4. Translate, then substitute

Let one canonical generated identity be

```text
R(n) = sum_s c_s(n) J(n+s) = 0.
```

For an ambient prepare-point displacement `delta`, the existing complete-row
translation produces

```text
R_delta(n) = sum_s c_s(n+delta) J(n+delta+s).
```

Only after that translation may the affine map be applied:

```text
R_(delta,F)(t)
  = sum_s c_s(F(t)+delta) J(F(t)+delta+s).
```

Define the retained ambient term key

```text
q = delta+s.
```

Then a case-bound sparse row has the exact semantics

```text
sum_q C_q(t) J(F(t)+q) = 0,
C_q(t) = c_(q-delta)(F(t)+delta).
```

### 4.1 Why the order matters

Substituting first and then calling the current ambient translator would
compute

```text
sigma_F(c_s)(n+delta) = c_s(F(n+delta)),
```

not

```text
c_s(F(n)+delta).
```

These differ whenever `F` is dependent. For `F(t)=(3-t,t)` and
`delta=(delta1,delta2)`, their first arguments are respectively

```text
3-(t+delta2)              and              3-t+delta1.
```

They agree only for special displacements. The proof-bearing implementation
must therefore call the existing whole-row `translated(delta)` first and its
new `substituted_on(F)` second.

### 4.2 Coefficients and guards

For every translated coefficient and guard:

1. validate the exact source context and source polynomial limits;
2. simultaneously compose every dependent index with its affine image;
3. retain the composed original denominator as a nonzero condition with a
   typed `ResidualAffineIndexSubstitution` origin;
4. reconstruct the rational polynomial with `from_num_den` only after the
   guard is durable; and
5. validate that no dependent index remains in the output.

If the mapped denominator is identically zero, this generated row is
unavailable on `L_F`. That is `UnsatisfiableRowDomainOnAffineLocus`; it is not
proof that the residual integral locus is empty.

Although `replace_with_poly` can implement the composition, the Rust wrapper
must give it simultaneous semantics. In V1 every bound image depends only on
unchanged free variables, so sequential replacement of bound variables is
mathematically order-independent. Replay should still compare the canonical
map and complete output, rather than trusting replacement order as an
implicit invariant.

### 4.3 Integral labels

Substitution acts on the base of every integral label:

```text
J(n+q) -> J(F(t)+q).
```

The production row may keep `q` as an ordinary `IndexShift` because the map is
owned once by the enclosing case-bound relation. Exposing the raw relation
without the map would be unsound: its key `q` would then be misread as the
global label `J(n+q)`.

Recommended wrapper:

```rust
pub struct AffineLocusBoundParametricRelation {
    schema: &'static str,
    source: Arc<ParametricRelation>,
    translation: IndexShift,
    affine_map: Arc<ResidualAffineIndexMapCertificate>,
    relation: ParametricRelation, // private; coefficients are sigma_F images
    base_assumptions: Box<[...]>,
    limits: AffineRelationSpecializationLimits,
    stats: AffineRelationSpecializationStats,
}
```

Only an affine-bound elimination compiler may borrow `relation`. There must be
no conversion to a global `ParametricReductionRuleCandidate`.

### 4.4 A concrete algebra example

Take

```text
F(t) = (3-t,t),
delta = (2,-1),
c(n) = (n1-1)/(d-2*n2),
s = (1,0).
```

Translate then substitute gives

```text
c(F(t)+delta) = (4-t)/(d-2*t+2),
q = delta+s = (3,-1),
J(F(t)+q) = J(6-t,t-1).
```

The denominator `d-2*t+2` remains an explicit nonzero condition even if a
later normalization cancels it against another factor.

## 5. Restoring the lattice and a formal ordering

### 5.1 Ambient shifts remain valid columns

For a fixed affine map `F`, two raw family labels are collected exactly when
their ambient shifts are equal:

```text
J(F(t)+q1) = J(F(t)+q2)  for all free t    iff    q1=q2.
```

The reverse implication is immediate. The forward implication follows by
componentwise equality after subtracting the common `F(t)`. This statement is
before any separately certified graph-symmetry quotient. Thus one restricted
elimination uses the existing `BTreeMap<IndexShift,...>` column set. Rows with
different affine-map manifests must never be mixed without a separate proved
reparameterization.

The image lattice `A Z^r` is **not** a quotient of the ambient shift lattice.
`J(F(t)+q)` and `J(F(t)+q+A*u)` become related only after reparameterizing the
complete equation, including its coefficients. They are not the same column
over `K(t)`.

### 5.2 Generalizing cylindrical ordering

LiteRed treats a `startp` component as numeric only when it is a literal
Mathematica integer. The matching RustRed distinction is:

```text
literal position:      row A_i is zero, so F_i(t)=b_i;
symbolic position:     row A_i is nonzero.
```

For a column `q`, use the existing cylindrical signed key with this change:

- at a literal position, compute the exact sector bit and excess of `b_i+q_i`;
- at a symbolic position, retain the source sector bit and use signed excess
  offset `q_i` for an active line or `-q_i` for an inactive line.

On the part of the residual case where the compared shifted labels retain the
formal source-sector bits, every occurrence of `F_i(t)` is common to all
columns in the same key component. It therefore cancels from pairwise
comparisons even when several components depend on the same free variable. No
independence assumption among the symbolic start components is needed for this
cancellation.

The new ordering can therefore reuse the arithmetic and field sequence of
[`CylindricalParametricEliminationOrdering`](../../src/cylindrical_ordering.rs),
but its manifest must additionally bind the complete affine map. Suggested
type:

```rust
pub struct AffineStartParametricEliminationOrdering {
    schema: &'static str,
    policy: IntegralOrderingPolicy,
    sector: SectorMask,
    affine_map: Arc<ResidualAffineIndexMapCertificate>,
    literal_assignment: PartialIndexAssignment,
    symbolic_positions: Box<[usize]>,
    stable_manifest: Arc<str>,
    limits: CylindricalOrderingLimits,
}
```

This is a formal symbolic-start order, not a claim that every shifted label
stays in the source sector at every integer point. `WhenBad` removes the finite
list of dangerous boundary values before a rule is applicable; each pulled-back
value may describe an infinite affine hyperplane of free-index points. The same
separation is already present in RustRed: parametric pivot order is a derivation
heuristic, while descent and leaks are a later proof boundary
([`parametric_elimination.rs:30-34`](../../src/parametric_elimination.rs#L30),
[`when_bad.rs:14-20`](../../src/when_bad.rs#L14)).

### 5.3 Prepare-point order

At exact shell depth `h`, enumerate ambient `delta in Z^N` with
`sum |delta_i|=h`. Reject a displacement only when a literal start component
`b_i+delta_i` leaves its source half-line. Do not sign-filter a nonconstant
affine component.

Sort retained displacements by the affine-start key of the zero-shift integral
at that prepare point, then by `IndexShift`. Expand rows point-major:

```text
for delta in ordered new prepare points:
    for canonical generated IBP/LI row in source order:
        translate(delta), then substitute(F)
```

This is the direct extension of the two LiteRed `preparepoints` branches and
of the previously designed integer-cylinder row order.

## 6. Grouped target matching and the exact pivot transform

Let

```text
F(t) = b + A*t
```

be the source start and let `p in Z^N` be an elimination pivot column. Because
the free rows are the identity, the pivot's free components are

```text
lhs_(f_alpha) = t_alpha + p_(f_alpha).
```

LiteRed's reflection at line 2484 is therefore

```text
t_alpha -> t_alpha - p_(f_alpha).
```

Write `p_F` for the vector of pivot components at free positions. The
transformed pivot label is

```text
F(t-p_F)+p
  = (b - A*p_F + p) + A*t.
```

Define

```text
b' = b - A*p_F + p.
```

The pivot is eligible exactly when a remaining target case in the same
contiguous group has map

```text
G(t) = b' + A*t.
```

This gives a completely replayable grouped matching algorithm:

1. require identical free-position order and identical `A`;
2. compute `b'` with checked arbitrary-precision integer arithmetic;
3. scan the remaining group in persisted priority order for exact `b'`;
4. retain every checked target-case locator up to the first match; and
5. emit `RejectedNoTargetCase` if none matches.

For any other column `q`, the transformed label is

```text
F(t-p_F)+q = G(t) + (q-p).
```

Thus recentering:

- substitutes `t -> t-p_F` in every coefficient and guard;
- changes every ambient key from `q` to `q-p`; and
- binds the resulting rule to the exact target map and target case.

The ordinary
[`ParametricPivotEquation::centered_relation`](../../src/parametric_elimination.rs#L201)
must not be reused blindly here. That method applies the ambient translation
`n -> n-p` to a global relation. A dependent restricted relation needs the
free-variable transform above.

### 6.1 Worked `n1 -> 3-n2` example

Let

```text
F(t) = (3-t,t),
p = (p1,p2).
```

Then `p_F=p2` and

```text
F(t-p2)+p = (3-t+p1+p2,t).
```

It matches the source case again when `p1+p2=0`. It matches the contiguous
target case

```text
n1 -> 4-n2
```

when `p1+p2=1`. This is precisely why a fixed-coordinate-zero test, sufficient
for a one-leaf integer-cylinder slice, is not the dependent grouped rule.

### 6.2 Group key

For normalized affine maps with the same free variable order, LiteRed's
integer-difference criterion is equivalent to

```text
A_left = A_right
and
b_left-b_right is an ambient integer vector.
```

Since V1 stores integer `b`, the second condition is automatic once `A`
matches; the offset still belongs in the transcript. A stable group key should
contain context, sector, free positions, `A`, and the named ordering policy,
but not `b`. Individual cases retain `b` and every source predicate.

## 7. `WhenBad` and effective coverage on an affine locus

An affine-bound pivot is not a global candidate. Its good domain is

```text
target case
AND L_G
AND all coefficient/domain guards
AND no coefficient-aware sector leak
AND uniform strict descent.
```

Outside the target case or `L_G`, the attempt is simply not applicable. It
must not classify another root leaf as unsupported and must not shadow an
earlier global rule.

### 7.1 Domain guards

All pivot, normalization, source-denominator, and relation guards have already
been translated, substituted through `G`, and free-recentered. Base-only
polynomials are assumptions in `K`; they are not index-case branches. An
index-dependent guard is split into zero/nonzero branches using the existing
typed Symbolica case builder.

### 7.2 Boundary pullback

Current `WhenBad` enumerates each finite dangerous value `n_i=v` for an
inactive line shifted upward and constructs the polynomial `n_i-v`
([`when_bad.rs:1263-1378`](../../src/when_bad.rs#L1263)). On the affine target
map this boundary becomes

```text
G_i(t)-v = 0.
```

The boundary polynomial must be composed through the same authenticated map.
For the first affine slice, it is sufficient and sound to split the complete
target coefficient numerator separately on that boundary:

```text
G_i(t)=v AND numerator(t)!=0    -> bad,
G_i(t)=v AND numerator(t)=0     -> continue.
```

This may retain structurally empty bad children when the numerator vanishes
modulo the boundary ideal; it cannot create false applicability. A later
unit-affine quotient can recover the sharper current behavior. It must not
specialize the original ambient coordinate `n_i` directly, because after
dependent substitution that variable may no longer occur in the coefficient.

### 7.3 Uniform descent

For labels on one target map, same-formal-sector complexity differences depend
only on the ambient shifts. The current signed proof in
[`when_bad.rs:1413-1486`](../../src/when_bad.rs#L1413) can therefore be reused
after binding its witness to the affine ordering manifest. Active-line pinches
are lower-sector targets; inactive activations are removed by the boundary
events above. A zero or harder first nonzero signed component remains
`Unsupported`, never applicable.

### 7.4 Effective coverage

The root partition must be refined only beneath the authenticated target case.
The good children receive an affine-locus conditional-rule disposition; bad
children remain live. The complete source and target predicate conjunctions
remain in the replay transcript. This is the dependent counterpart of
LiteRed's

```text
case && !WhenBad     (installed rule)
case &&  WhenBad     (new residual work)
```

at
[`LiteRed2026.m:2488-2500`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2488).
A concrete witness may select or validate a candidate, but it cannot remove
the symbolic parent cell.

## 8. Replay graph

A durable dependent-start derivation should retain this chain:

```text
generated IBP/LI row span
  -> exact source residual case
  -> affine equality extraction and map certificate
  -> contiguous case group and target inventory
  -> exact L1 prepare-point layers
  -> translated global rows
  -> affine-substituted locus-bound rows
  -> affine symbolic ordering and elimination
  -> pivot free-recentering
  -> exact target-case match
  -> affine WhenBad partition
  -> effective coverage overlay
```

Every arrow is regenerated during replay. The transcript must bind:

- family and exact `K(n)` context fingerprints;
- source material/work-item/case locators;
- equality predicate ordinals and normalized affine rows;
- free/bound/literal positions, `b`, `A`, and map manifest;
- each shell depth, enumeration transition count, accepted/rejected offset,
  and final prepare-point order;
- canonical row ordinal, translation, translated-row manifest, and substituted
  row manifest;
- every unavailable-row domain result;
- affine ordering keys and comparison policy;
- elimination source order, pivot trace, divisors, and guards;
- `p_F`, target `b'`, every target checked, and selected target;
- free-variable coefficient/guard recentering;
- every affine boundary pullback and numerator gate; and
- the final effective leaf classifications.

The large generated row span and partitions should remain shared by `Arc` in
memory. Pointer identity is a scaling invariant, not a persistence proof;
replay compares complete authenticated payloads.

## 9. Limits and typed boundaries

### 9.1 Required aggregate limits

At minimum, add checked limits for:

- ambient arity, free positions, bound positions, and affine matrix entries;
- source predicates, inspected polynomial terms, exponent entries, and base
  coefficient monomials;
- affine coefficient and constant bit lengths;
- equality normalization and fraction-free row operations;
- substitution images, source terms, prospective expanded terms, power
  operations, collected terms, and integer-bit growth;
- retained mapped denominators, guards, origins, base assumptions, and bytes;
- shell depths, offsets, iterator transitions, prepare points, components,
  fixed-sector checks, and sort comparisons;
- expanded/retained rows and row-manifest bytes across all depths;
- affine order keys and key components;
- elimination columns, pivots, reductions, sparse updates, guards, and replay
  work;
- case groups, cases per group, target checks, target references, and pivot
  recenterings;
- boundary values, affine boundary-polynomial terms, numerator gates, case
  splits, live leaves, and effective classifications; and
- total transcript terms and bytes.

The budgets are cumulative across the certificate, not reset for each row or
shell. Preflight expansion bounds before calling `replace_with_poly`; do not
measure only the collected output after Symbolica has already allocated the
intermediate powers.

### 9.2 Typed unsupported reasons

These are completeness boundaries, not algebra failures:

```text
NoUsableEqualityPredicate
NonAffineIndexEquality
IndexMapDependsOnBaseParameter
NonIntegralAffineCoefficient
NonUnitIntegerAffineEquality
DependentRhsReferencesBoundIndex
CyclicIndexSubstitution
UnconsumedIndexEqualityForFullStartParity
IncompatibleFreePositionOrder
NonContiguousAffineTarget
NoRemainingTargetCaseForPivot
BoundaryPullbackNotRepresentable
GeneralCongruenceCaseNotSupported
NonlinearDependentStartNotSupported
RationalDependentStartNotSupported
```

Each outcome retains the source case and offending predicate or pivot locator.

### 9.3 Errors and interruptions

These abort construction or preserve explicit unresolved work:

```text
WrongFamily / WrongContext / WrongArity
MalformedAuthenticatedPolynomial
UnsatisfiableRowDomainOnAffineLocus
IndexShiftOverflow
ConcreteIndexRepresentationBoundary
ResourceLimit { stage, locator, requested, limit }
ResourceCountOverflow { stage, locator }
SymbolicaPanic { stage, locator }
ExactAlgebraFailure { stage, locator }
ReplayMismatch { stage, locator }
```

`NoRemainingTargetCaseForPivot`, depth exhaustion, and an empty conditional
row system are not master proofs. `UnsatisfiableRowDomainOnAffineLocus` says a
particular guarded row cannot be used on that locus; it does not prove the
locus itself empty.

## 10. Smallest implementable slice after integer cylindrical ordering

The next merge should be a **unit-affine substitution foundation**, not a full
claim of dependent `SolvejSector` closure.

It should support:

1. any existing literal `PartialIndexAssignment`;
2. exactly one additional equality associate to

   ```text
   n_bound - (c + sum_j a_j*n_free_j) = 0
   ```

   with integer `c,a_j`, unit bound coefficient, and RHS references only to
   otherwise free original indices;
3. a replayable `ResidualUnitAffineIndexMapCertificate` which embeds the
   literal cylinder, cites the exact source equality predicate, and exposes no
   unbound raw map fields;
4. a checked Symbolica composition API for polynomials, rational coefficients,
   nonzero conditions, and complete translated relations;
5. a private `AffineLocusBoundParametricRelation` with ambient `IndexShift`
   keys and exact translate-then-substitute semantics;
6. an affine-start ordering wrapper which reuses cylindrical signed keys for
   constant versus nonconstant start rows and binds the full map manifest; and
7. exact prepare-point layer replay, filtering only literal rows.

This slice should **not** expose elimination pivots as reduction rules yet. Its
public result is a replayable, generated, locus-bound row system. That is the
smallest coherent boundary at which the central algebraic identity can be
tested without inventing target-case or `WhenBad` semantics.

The immediately following slice adds:

1. persistent elimination over one affine-map row system;
2. the `b' = b-A*p_F+p` target matcher for a real contiguous case group;
3. free-variable recentering of coefficients, guards, and labels;
4. affine boundary pullback; and
5. effective conditional coverage.

This staging is materially stronger than another concrete witness sampler:
the generated rows already hold for the complete dependent equality locus.

## 11. Suggested validation using concrete topologies only

Production code receives no topology names or expected recurrences. Concrete
families and powers below are test fixtures and independent oracles only.

### 11.1 Connected two-loop sunset: algebraic substitution oracle

Use the generated equal-mass sunset IBP/LI row span and the test-only affine
case

```text
F(t,n3) = (3-t,t,n3).
```

For every retained canonical row and several exact L1 displacements, compare

```text
translate(delta)
  -> affine-substitute(F)
  -> concretely specialize (t,n3)
```

against direct concrete specialization of the original generated row at

```text
F(t,n3)+delta.
```

Use in-sector integer values such as `t=1,2` and multiple `n3` values. Compare
complete collected integral keys, coefficients, mapped denominator guards,
and origin sets. This is an independent black-box oracle for operation order;
no reduction table is involved.

If the generated sunset residual partition naturally contains an associate
of `n1+n2-3`, bind the test to that exact source case and predicate ordinal. If
not, a test-only symbolic case fixture may add that equality while all algebra
still comes from the real sunset row span. The latter validates the mechanism,
not a claim that the current scheduler already discovers that case.

### 11.2 Connected two-loop sunset: grouping and pivot recentering

Use two authenticated test cases

```text
F0(t,n3) = (3-t,t,n3),
F1(t,n3) = (4-t,t,n3).
```

Require the group compiler to prove their difference is `(1,0,0)`. For every
generated elimination pivot considered by the fixture, independently compute
`p_F`, `b'`, and the recentered RHS keys. A pivot with `p1+p2=1` may target
`F1`; the same pivot must be `RejectedNoTargetCase` when `F1` is removed from
the group. Replay must fail after tampering with `A`, `b`, free-position order,
pivot, or target locator.

### 11.3 Concrete specialization and Vakint comparison

After affine `WhenBad` and effective coverage land, reduce concrete sunset
integrals whose powers lie on the tested affine cases. Compare the reduced
coefficients with Vakint's alphaLoop oracle while leaving master topology
substitution disabled. Vakint/FORM output appears only in test assertions; no
FORM code or hardcoded recurrence enters RustRed.

The comparison should include at least one point on each contiguous target
case and one boundary point where the affine rule is inapplicable. A bounded
failed search must remain explicitly uncovered.

### 11.4 Progression

Only after the connected sunset dependent-locus path replays should the same
generic compiler be exercised on a connected three-loop massive vacuum
family. Four- and five-loop tests should reuse the identical production API;
loop-specific source modules are not evidence for this milestone.

All test binaries should use the licensed GMP-enabled Symbolica build and run
through parallel nextest, for example:

```bash
cargo nextest run -j4 --test dependent_symbolic_start_sunset
```

No `no_gmp` feature and no FORM execution are permitted.

## 12. Claim boundary

When the first unit-affine slice lands, RustRed may accurately claim:

- exact generated IBP/LI rows can be translated and restricted to a complete
  dependent integer-affine equality locus;
- the restriction is performed entirely with authenticated Symbolica
  rational polynomials;
- integral labels retain a replayable ambient shift lattice;
- literal and nonliteral start components receive the same prepare-point
  treatment as the audited LiteRed source; and
- every result replays from the original generated row and source equality.

It may not yet claim:

- arbitrary Mathematica `ToRules` expression support;
- arbitrary nonlinear or rational dependent starts;
- general Presburger/Diophantine case solving;
- grouped pivot attachment before the target matcher exists;
- affine-locus closure before `WhenBad` and effective coverage are composed;
- a master from an unsuccessful bounded search; or
- complete `SolvejSector` parity.

The full affine grouped slice reaches the next honest claim: LiteRed-style
dependent affine `startp` derivation, free-variable recentering, exact target
case matching, and conditional coverage, all generated from the family rather
than hardcoded by loop count or topology.
