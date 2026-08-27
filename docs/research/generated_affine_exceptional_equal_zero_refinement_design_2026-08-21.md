# Generated-affine exceptional EqualZero refinement design

Date: 2026-08-21

Status: long-term semantic design, with its generalized integer-lattice kernel
superseded as an implementation prescription on 2026-08-27. This document
still specifies the topology-neutral result RustRed ultimately needs. It does
not authorize topology-specific recurrences, special loop-count formulas, or
hard-coded vacuum families.

The current production policy is stricter than the historical construction in
sections 4--8. The vendored Symbolica public Rust API has polynomial content/
primitive normalization and exact integer-matrix multiplication, but no
Smith/Hermite normal form or complete integer-kernel/affine-lattice
parameterization. RustRed must not fill that API gap with its own CAS. The
first production refinement therefore accepts one normalized integer-affine
equality only when an active free coordinate has literal coefficient `+1` or
`-1`; it delegates geometry composition to Symbolica and returns a typed
typed `RequiresIntegerNormalForm` unsupported outcome otherwise. The broader
semantics below remain an acceptance target for a future public Symbolica
normal-form API, not permission to promote the existing handwritten prototype.

Implemented checkpoint (2026-08-27):
`generated_affine_residual_case_unit_equality_refinement.rs` realizes that
strict subset in current-target coordinates. It performs a borrowed atom-row
logical-memory/GMP census before copying the predicate, prospects both virtual
integer matrices before dense staging, delegates the only map product to
Symbolica `Matrix<IntegerRing>`, and verifies the equality maps to zero through
the existing Symbolica-backed compact substitution plan. The matrix boundary
also admits a conservative prospective output retained-byte envelope before
native multiplication and authenticates exact output capacity afterwards. Its
virtual-entry census is sign-aware, records the prospective input envelope
separately from the native census, and covers `-i128::MIN` promotion. The pinned
default-GMP output bound treats only positive magnitudes through 127 bits as
inline and reserves two capacity limbs beyond the rounded dot-product envelope.
Parallel licensed default-GMP validation passed all 19 refinement tests and 37
matrix-boundary tests (56/56), including a genuine GMP-sized constant,
`i128::MIN` promotion, the signed 128-bit inline boundary, the two-limb
multi-term capacity boundary, and exact/one-below resource limits.

This certificate remains authority-neutral. It is not yet attached to a
committed exceptional Domain resident, and it neither regenerates generic
IBP/LI rows nor submits completed physical rows to the fresh child database.
`ProvedEmpty` is diagnostic until that future adapter retains and replays the
event-bound proof needed for pruning.

## 1. Decision and completeness boundary

An exceptional predicate

    P(n) = 0

cannot remain only an opaque premise when RustRed starts the next elimination
epoch. For LiteRed-like recursive closure, RustRed must use the equality to
restrict the inherited integer-affine parameter space and then compose every
new relation coefficient and guard through that restriction. Otherwise a
coefficient that is known to vanish on the exceptional branch remains
syntactically nonzero and the next elimination repeats the failed generic
pivot.

The eventual stage must be complete for simultaneous integer-affine equalities
over arbitrary-precision Symbolica integers, including congruence and
divisibility cases. That completeness is not implemented at the present
Symbolica API boundary. Today, multiple equalities, nonlinear equalities, and
an affine equality without a literal unit pivot are typed completeness
boundaries; none is an empty branch or a master integral.

NonZero predicates are not equations. They remain guards, except when exact
composition proves them contradictory or discharges them as nonzero constants.

## 2. Evidence from LiteRed

The relevant flow is in
vendor/LiteRed2/Source/LiteRed2026.m:

- Lines 2430-2447 take one residual case, find the indices not fixed by its
  rules, and build the next starting points by applying those rules:
  startps=(indices/.#)&/@cases.
- Lines 2484-2499 select the exact residual case for a candidate, construct the
  conditional rule, append its bad applicability condition, remove the covered
  case, and rebuild the starts for the remaining cases.
- Line 2522 rebuilds noRules from the old unresolved cases and newly found bad
  conditions through LogicalExpand, smartReduce, and ToRules.
- Lines 2565-2568 implement WhenBad. They collect denominator-zero conditions
  and failures caused by shifted integrals leaving the allowed sector.
- Lines 2573-2578 implement SmartReduce by calling Reduce over Integers
  together with sector inequalities. The final cf stage accepts index
  equalities suitable for conversion to substitution rules.

The semantic point is not that RustRed must reproduce Mathematica Reduce
internally. The point is that LiteRed turns exceptional integer conditions into
new parameterizations/substitutions and uses them to prepare and eliminate new
generated identities. It does not merely annotate a repeated generic
elimination with P=0.

The same requirement is recorded in
docs/research/generated_affine_residual_inventory_v2_design_2026-08-21.md:37-53.
The sector-wide work queue and the requirement to specialize generated source
identities and re-eliminate on fixed equalities are specified in
docs/research/symbolic_solvejsector_design.md:160-188.

Product equalities must already have passed through Boolean factor splitting.
In an integral domain,

    p1(n) ... pr(n) = 0

means a disjunction of factor equalities, not one affine equation.
docs/research/product_locus_affine_cover_design_2026-08-13.md:22-55 specifies
the required order: authenticated factor provenance, Boolean normalization and
disjoint branching, affine recognition of the selected zero atoms, and then
simultaneous integer-affine system compilation. Expanding the product as one
row is unsound, while sequential independent one-equality maps can make
substitution order semantic and lose simultaneous consistency.

## 3. Existing RustRed seams and their limits

### 3.1 Source-neutral exceptional input

The B0 authority projection already exposes precisely the required input:

- src/generated_affine_residual_boolean_cover.rs:577-638 exposes, through
  narrow borrowed views, the inherited affine map, existing guards, constants,
  free positions, and each ordered exceptional predicate's locus ordinal, kind,
  and polynomial.
- src/generated_affine_residual_source_authority.rs:1267-1333 constructs those
  source-neutral predicate and exceptional-case views while keeping the old
  relative case and partition sealed.

The refinement stage must consume these views through the exact B1 inventory
or Boolean-certificate owner. It must not recover or retain raw V1 locators,
old source cases, relative partitions, or old guard origins. The authority
firewall and neutral guard-origin rule are documented in
docs/research/generated_affine_residual_inventory_v2_design_2026-08-21.md:23-33
and 143-181.

### 3.2 Existing integer-system prototype (non-production)

ResidualAffineIntegerMap is currently an ambient-square map

    F(n) = b + A n

with identity rows at free_positions and solved expressions at pivot_positions.
Its contract requires A squared equal to A and A b equal to zero; see
src/residual_affine_integer_system.rs:630-711.

The historical DFS is intentionally a unit-pivot cylinder solver. It searches only
eligible original-coordinate unit columns and returns
GeneralCongruenceCaseNotSupported when none produces a complete path; see
src/residual_affine_integer_system.rs:2293-2351. It already detects a nonzero
constant row and the elementary gcd-does-not-divide-constant obstruction in
src/residual_affine_integer_system.rs:2359-2408.

The test at src/residual_affine_integer_system.rs:6068-6100 makes the boundary
explicit: a row with coefficients (2,1) is accepted because it has a unit
original column, whereas 2 n0 + 3 n1 = 0 is typed unsupported. The eventual
generalized refinement must solve the latter, but the production path must
pause there until an adequate public Symbolica normal-form/kernel API exists.
The V1 compiler and schema remain isolated prototype material and must not be
imported by the production exceptional-child ingress.

RustRed already has bounded exact arithmetic suitable for reuse:
src/residual_affine_integer_system.rs:3038-3098 implements bounded gcd and
extended gcd over Symbolica Integer and verifies the reconstructed Bezout
identity. The Budget arithmetic and prospective integer-bit checks around
src/residual_affine_integer_system.rs:1493-1818 are the model for every new GMP
operation. A field-style Gaussian matrix solver is not an integer-lattice
solver and must not replace this logic. Nor may the prototype logic itself
become production authority under the Symbolica-only algebra rule.

Sections 4--8 describe a sound mathematical route based on explicit Bezout
and unimodular transforms. They are retained as requirements and upstream-API
guidance only, not as an implementation recipe for RustRed. Production may not
author or replay those lattice transforms itself. The implemented fast path is
the strict literal-unit-pivot subset, with all matrix products performed by
Symbolica.

### 3.3 Affine recognition and polynomial composition

ResidualAffinePrimitiveRow stores the canonical component order

    [constant, coefficient(n0), ...]

and has a checked internal construction boundary at
src/residual_affine_atom_rows.rs:165-211. The recognizer's result vocabulary
distinguishes a row, a redundant zero polynomial, an inconsistent nonzero
constant, and typed unsupported affine-recognition cases; see
src/residual_affine_atom_rows.rs:280-313. Its bounded compiler begins at line
532. It recognizes a base-field factor times one primitive integer-affine row.
Non-affine index monomials and nonassociate base-parameter blocks are explicit
unsupported outcomes, not proofs of emptiness.

The polynomial composition machinery is also reusable. The authoritative
entry points and types are named here rather than pinned to source line
numbers, which move as old compositor code is deleted:

- `ResidualAffineCompositionCorePlan` and its owning plans retain the shared
  authenticated image payload.
- `compile_residual_affine_compact_composition_plan` and
  `compile_residual_affine_composition_core` build the compact-geometry full
  images.
- Polynomial composition selects Symbolica's simultaneous polynomial
  evaluator when its audited stride is safe and Symbolica's simultaneous Atom
  replace/expand/convert path otherwise. RustRed owns the typed preflight and
  postvalidation boundary, not a private polynomial compositor.
- `prepare_residual_affine_coefficient_core` and
  `execute_prepared_coefficient_on_residual_affine_core` map numerator and
  denominator, report a zero mapped denominator, and retain the exact mapped
  denominator before normalization for later guard classification.

The legacy plan adapter is bound to `ResidualAffineIntegerSystemCertificate`.
Its structural census assumes the V1 pivot/free partition and identity free rows.
The generalized refinement must therefore add an authenticated compact-
geometry adapter. The core full-image builder itself can be reused.

The complete-row behavior to preserve is visible in
src/generated_residual_affine_branch_bound_relation.rs:1154-1289: translate the
source row, compile one authenticated composition plan, compose every source
guard, then compose both halves of every rational coefficient. Exceptional
refinement changes the geometry and the authority owner, not that
all-coefficients/all-guards rule.

### 3.4 First implementation seam: source-independent lattice kernel

The smallest independently auditable implementation patch is a new
source-independent module:

    src/residual_affine_integer_lattice_kernel.rs

It operates only on ordered `ResidualAffinePrimitiveRow` values in a compact
integer parameter space. It does not import the B1 inventory or Boolean-cover
owner, retain predicate locators, construct the generalized ambient map, or
compose polynomials. The later refinement owner is responsible for composing
and recognizing predicates, sorting and deduplicating rows, merging lineage,
binding the exact source `Arc`, and sealing the kernel result into the full
refinement certificate.

The crate-private kernel boundary is:

```rust
solve_residual_affine_integer_lattice(
    parameter_arity: usize,
    ordered_rows: &[ResidualAffinePrimitiveRow],
    limits: ResidualAffineIntegerLatticeLimits,
) -> Result<ResidualAffineIntegerLatticeOutcome,
            ResidualAffineIntegerLatticeError>

verify_residual_affine_integer_lattice(
    parameter_arity: usize,
    ordered_rows: &[ResidualAffinePrimitiveRow],
    outcome: &ResidualAffineIntegerLatticeOutcome,
    limits: ResidualAffineIntegerLatticeLimits,
) -> Result<(), ResidualAffineIntegerLatticeError>
```

The module defines `ResidualAffineIntegerLatticeLimits`,
`ResidualAffineIntegerLatticeStats`,
`ResidualAffineIntegerLatticeTransform`,
`ResidualAffineIntegerLatticeRowDisposition`,
`ResidualAffineIntegerLatticeEmptyWitness`,
`ResidualAffineIntegerLatticeSolution`,
`ResidualAffineIntegerLatticeOutcome`, and
`ResidualAffineIntegerLatticeError`. A solved value owns row-major `p`, `K`,
and `L` with dimensions `r`, `r` by `s`, and `s` by `r`. The verifier checks
`L K=I_s`, `L p=0`, and, for every consumed row, `c+d p=0` and `d K=0`.
Every applied independent row decreases `s` by exactly one, a redundant row
does not change the state, and an empty outcome retains its exact row ordinal
and arithmetic witness.

This kernel result is not yet source authority and must not be published as a
refinement certificate. Keeping the first patch at this boundary avoids
mixing the new arithmetic proof with authority, generalized-map, and
composition-plan work while still providing an independently replayable
algebraic seam.

## 4. Algebraic input model

Let the ambient index vector have arity N. The inherited V1 map has image

    n = b + B t,    t in Z^r.

Let S be its ordered free_positions. B is the N by r matrix made from the
columns A[:,S]. Let E select the coordinates S from an ambient vector. The V1
identity-free-row invariants give

    E B = I_r,
    E b = 0,
    F0(n) = b + B E n.

The refinement works only in the compact inherited parameter space Z^r. It
does not inspect graph names, propagators, masses, loop count, sector names, or
topology-specific recurrence templates.

For each ordered exceptional EqualZero predicate P:

1. Compose P through the inherited affine composition plan.
2. If the result is the zero polynomial, record a redundant equality witness.
3. If it is a nonzero constant, prove the branch empty.
4. Otherwise call the bounded affine atom recognizer.
5. If recognized, extract the compact primitive equation

       c + d t = 0,

   where d is read at the inherited free-coordinate positions S.
6. If recognition reports a nonlinear index monomial or nonassociate
   parameter block, return a typed polynomial-quotient-required outcome for
   this stage. Preserve the live branch and its source predicates.

Primitive rows are canonicalized by the existing recognizer. The refinement
sorts recognized rows lexicographically, removes exact duplicates, and merges
their source-neutral predicate-position and locus-ordinal lineage. It also
retains the original ordered predicate manifest separately, because replay
must reconstruct source order even though the solver uses canonical row order.

All EqualZero rows of one Boolean child are solved simultaneously. A sequence
of independent ambient projections is not the semantic model.

## 5. Exact incremental integer-lattice solver

### 5.1 State and invariants

Maintain a parameter-space solution description

    t = p + K u,    u in Z^s,

with:

- p an r-vector of arbitrary-precision integers;
- K an r by s integer basis matrix;
- L an s by r integer left inverse;
- L K = I_s.

Initially:

    p = 0,
    K = I_r,
    L = I_r,
    s = r.

The concrete update below also maintains L p = 0. The more general projection
formula in Section 6 does not rely on that optional simplification.

### 5.2 Restrict by one row

For a canonical equation

    c + d t = 0,

set

    h     = -c,
    alpha = d K,
    delta = h - d p.

The remaining equation in the current coordinates is

    alpha u = delta.

Handle it exactly:

1. If every entry of alpha is zero:
   - delta equal to zero makes the row redundant;
   - delta nonzero proves the current branch inconsistent.
2. Otherwise compute the positive gcd

       g = gcd(alpha_0, ..., alpha_(s-1)).

3. Compute the Euclidean quotient and normalized remainder

       delta = q g + rho,    0 <= rho < g.

   If rho is nonzero, prove the branch empty. Retain the exact witness
   containing row lineage, delta, positive g, and rho. This is the integer,
   rather than merely rational, consistency condition.
4. If g divides delta, transform alpha by a deterministic unimodular column
   operation to

       (g, 0, ..., 0).

5. Fix the transformed anchor coordinate to

       z0 = q = delta / g,

   reusing the authenticated exact quotient from step 3. Absorb that value
   into p, and remove that coordinate from K and L.

### 5.3 Deterministic unimodular transform

Choose the least nonzero alpha ordinal as the anchor. If it is not the first
live ordinal, apply and record the corresponding column permutation to K and
row permutation to L. If the permuted anchor coefficient is negative, apply
and record the diagonal unimodular sign transform which negates the matching K
column and L row, and negates the anchor coefficient. This step is mandatory
even when every partner is zero. It makes the live anchor positive before the
positive-gcd convention below is used. Then visit every other live ordinal in
increasing order. Zero partners require no pair operation.

For current anchor coefficient a and partner coefficient q, obtain bounded
Bezout coefficients x and y with positive g_pair:

    x a + y q = g_pair.

The Bezout pair is not chosen from an unspecified set of valid solutions.
Compilation and replay use the exact deterministic absolute-value extended
Euclidean loop and final input-sign correction already modeled by
`bounded_extended_gcd` in
src/residual_affine_integer_system.rs:3053-3098. Euclidean quotient/remainder
semantics, least-anchor choice, partner visitation order, and the resulting x
and y are therefore part of the schema. A different valid Bezout pair is not
payload-equal replay.

Apply the two-by-two column matrix

          [ x       -q/g_pair ]
    C  =  [                    ].
          [ y        a/g_pair ]

It satisfies:

    [a q] C = [g_pair 0],
    det(C) = 1.

Its exact inverse is

                [ a/g_pair   q/g_pair ]
    C inverse = [                       ].
                [ -y          x         ]

Apply C in place to the matching two columns of K and apply C inverse in place
to the matching two rows of L. Do not needlessly materialize a dense global
unimodular matrix. Every exact division, multiplication, addition, negation,
comparison, and allocation is preflighted through the new phase budget.
Record and replay the permutation, the optional anchor-sign transform, and
every pair transform. The transcript variants are respectively a coordinate
swap, a coordinate negation retaining its coefficient before negation, a
Bezout pair retaining a, q, g_pair, x, and y, and a fixed-coordinate record
retaining delta, g, and z0.

After all partners have been eliminated, alpha is (g,0,...,0). With
z0=delta/g, using the transformed K:

    p <- p + K[:,anchor] z0.

Delete the anchor column of K and the matching anchor row of L. Decrement s.

### 5.4 Correctness

Each two-column matrix C, permutation, and diagonal sign transform is
unimodular, so it is a bijection of Z^s. Therefore it neither loses nor
invents integer points.
The scalar equation g z_anchor=delta is inhabited exactly when g divides
delta. Once inhabited, fixing z_anchor and leaving the other transformed
coordinates arbitrary enumerates every integer solution exactly once.

Before deletion, the matching row/column transforms preserve L K=I_s.
Deleting the fixed coordinate from both sides leaves

    L_new K_new = I_(s-1).

The absorbed anchor column is annihilated by every retained row of L_new, so
L_new p_new=0 follows inductively from L p=0. Consequently K always has full
column rank over Z and the state describes exactly the simultaneous solution
lattice of all consumed rows.

This is an incremental Hermite/Smith-like construction. A full batch Smith or
Hermite normal form is not required for correctness. It may later provide a
more canonical geometry key, but it is an optional optimization.

## 6. Lift the solution back to a generalized ambient map

At solver termination define

    M     = K L,
    gamma = (I_r - M) p.

Then:

    M squared = M,
    M gamma = 0.

The compact parameter-space projection

    t' = gamma + M t

is idempotent and has image exactly p + K Z^s. The equality of images follows
because p-gamma=M p=K(L p), an integral element of the K lattice. In the
concrete update above L p=0, so gamma=p, but the certificate should verify the
general formula.

Lift this projection through the inherited map:

    beta  = b + B gamma,
    Cproj = B M,

and define

    F1(n) = beta + Cproj n[S].

Equivalently, with selector E:

    A1 = B M E,
    F1(n) = beta + A1 n.

The compiler and replay verifier prove:

    M squared = M,
    M gamma = 0,
    A1 squared = A1,
    A1 beta = 0,
    F0 composed with F1 = F1,
    image(F1) = image(F0) intersected with every consumed equality,
    P composed with F1 = 0 for every consumed affine EqualZero P.

Membership for an integer point is again exact fixed-point membership:

    F1(n) = n.

### 6.1 Why this cannot masquerade as the V1 map

For a general lattice such as

    2 n0 + 3 n1 = 0,

a basis is, up to sign,

    (3, -2).

There need not be an ambient coordinate whose row is an identity free row.
Populating ResidualAffineIntegerMap.free_positions with a fictitious free
coordinate would violate its public contract and corrupt structural censuses.

Introduce a distinct generated-affine refined-map schema, for example
GeneratedAffineEqualityRefinedIntegerMap. It owns or authenticates:

- ambient arity N;
- inherited compact support positions S;
- beta;
- row-major compact coefficients Cproj;
- image rank s;
- exact fixed-point membership;
- the p, K, L, M, and gamma derivation transcript or a sealed certificate
  which provides them to replay.

The support positions name source variables used by the compact polynomial
images; they are not claimed to be identity rows or independent parameters.
Removing algebraically redundant compact support columns, or introducing
synthetic minimal parameter names, is optional.

## 7. Affine quotient and relation specialization

For affine exceptional equalities, composition through F1 is an idempotent
normal-form map:

    R(f) = f composed with F1,
    R(R(f)) = R(f).

The next generated-elimination epoch must apply R to:

- every coefficient numerator;
- every coefficient denominator;
- every inherited and newly produced guard polynomial;
- every generated or translated source row selected for the child.

It is insufficient to compose only the predicate which created the branch.
For an integral term indexed by a source shift q, the specialized term is
parameterized as J(F1(n)+q), with translation/specialization order recorded in
the row transcript. The warning in
docs/research/symbolic_solvejsector_design.md:204-208 applies: centering a pivot
changes the source index, so a translated condition must never be substituted
as if it were uncentered.

Rational coefficients are composed as two exact polynomials before
normalization:

- a mapped zero denominator makes that candidate unavailable on the child and
  feeds the appropriate exceptional/WhenBad path;
- a nonconstant mapped denominator is retained as a NonZero guard;
- the pre-normalization mapped denominator is the durable guard payload;
- only then may the rational numerator/denominator pair be normalized.

The topology-neutral acceptance fixture is:

    inherited map: identity on (n0,n1)
    P(n):          n0+n1-3
    source row:    P(n) J(n+e0) + J(n) = 0

One valid refined map is

    F1(n0,n1) = (3-n1,n1),

while another deterministic anchor convention may produce

    F1(n0,n1) = (n0,3-n0).

In either case P composed with F1 is exactly zero. Specializing the whole row
therefore removes the high term and exposes

    J(F1(n)) = 0

as the lower pivot. Keeping P=0 only as an opaque premise would leave the high
coefficient syntactically present and fail this test.

## 8. NonZero guard semantics

Never insert a NonZero predicate into the equality lattice solver.

After F1 has been built, compose every exceptional and inherited NonZero
polynomial Q through F1:

1. If Q composed with F1 is zero, the child is contradictory and therefore
   empty.
2. If it is a proven nonzero integer or admissible base-field constant,
   discharge it, or retain the corresponding base assumption where current
   coefficient-field policy requires one.
3. Otherwise retain the exact source-neutral NonZero condition on every rule
   derived in the next epoch.

Guard origins created at this layer must use the generated-affine sealed
origin, not a V1 source-case locator. Replay binds each retained guard back to
its source-neutral predicate position and exact polynomial.

General ideal reasoning must also respect inequations. Adding Q to an equality
ideal would assert Q=0 and is unsound. A future saturation proof may introduce
a witness variable w and the equation

    w Q - 1 = 0,

under explicit variable, degree, term, arithmetic, and replay limits.
docs/research/symbolic_solvejsector_design.md:210-232 records the same
boundary.

Sector inequalities are applicability or chamber predicates. This affine
equality stage does not claim to parameterize arbitrary Presburger
inequalities.

## 9. Nonlinear EqualZero boundary

An equality such as

    n0 n1 - 1 = 0

does not define an affine lattice. Full eventual LiteRed parity requires a
separate bounded polynomial-quotient layer:

- checked Symbolica polynomial-ideal or Groebner construction;
- explicit input variable, degree, term, coefficient-bit, operation, and
  memory limits;
- panic containment around native APIs;
- exact authentication of the returned basis;
- independent normal-form replay;
- saturation when NonZero assumptions participate;
- an integer-inhabitation argument, rather than only an algebraic-closure
  argument.

Until that layer exists, the outcome is typed UnsupportedPolynomialLocus or
RequiresPolynomialQuotient. The branch and its predicates remain live. It is
not sound to drop the equality, prove the branch empty, or declare the
integrals on it masters. The distinction between structural decisions,
bounded simple normalization, and general ideal reasoning is specified at
docs/research/symbolic_solvejsector_design.md:210-228.

## 10. Authority and certificate

The refinement certificate uses a new schema. It must bind:

- the exact B1 inventory or Boolean-cover certificate Arc;
- source-neutral Boolean record ordinal and actionable-case ordinal;
- context, family, sector, monomial-order, and generated-source fingerprints;
- exact inherited-map identity and schema;
- the original ordered predicate manifest:
  predicate position, locus ordinal, kind, and authenticated polynomial;
- the EqualZero and NonZero positional subsets;
- mapped-polynomial composition statistics and atom-recognition certificates;
- every canonical compact affine row and merged predicate lineage;
- row sorting and deduplication census;
- deterministic anchor permutations, anchor-sign transforms, and pairwise
  Bezout transforms;
- exact-division, redundant-row, nonzero-constant, and divisibility witnesses;
- final p, K, L, M, gamma, beta, Cproj, support positions, and rank;
- every projected NonZero classification and retained condition;
- all compile limits and exact statistics;
- logical retained-memory, fresh temporary peak, replay peak, and
  payload-comparison census.

No certificate field may embed a raw V1 relative locator, old source case,
relative partition, or reachable old GuardOrigin.

Compilation is transactional and panic-contained. A resource, allocation,
native-algebra, or invariant failure returns no certificate and no partially
authorized plan.

### 10.1 Fresh composition-plan seam

Add a non-Clone fresh authorization analogous to
[`ParametricCoefficientContext::compile_residual_affine_composition_plan_from_fresh_integer_system`](../../src/parametric_coefficient.rs).
It should:

1. consume the immediately preceding successful refinement result;
2. reauthenticate the adjacent compact geometry and authority binding;
3. build the existing full-image composition core from beta, Cproj, and S;
4. prevent arbitrary callers from supplying unauthenticated matrices;
5. allow the owner to discard the transient plan and later recompute the same
   structural census without trusting stored memory scalars.

The generalized adapter must not invoke the V1 structural census which assumes
a pivot/free partition. Its own census validates support bounds, dimensions,
integer payloads, projection identities, and exact binding to the refinement
certificate.

### 10.2 Replay

Replay starts from the retained source authority, not from stored derived
rows. It:

1. re-resolves the exact ordered predicates;
2. recomposes each predicate through the inherited map;
3. reruns affine recognition;
4. reconstructs canonical ordering, deduplication, and lineage;
5. reruns every bounded gcd, anchor-sign transform, Bezout transform, exact
   division, and state update;
6. compares p, K, L, and all witnesses;
7. reconstructs M, gamma, beta, Cproj, and the composition plan;
8. verifies all matrix identities and that every consumed P composes to zero;
9. recomposes and reclassifies every NonZero predicate;
10. independently checks retained/fresh/replay statistics and logical memory.

Equal-but-independently-allocated source owners must not pass identity checks.
Dropping external source handles while keeping the sealed owning certificate
must remain safe.

## 11. Resource model

The stage nests the existing composition-plan, polynomial-composition,
exact-algebra, and affine-atom recognition limits. Add aggregate limits for:

- ambient arity N and inherited parameter rank r;
- predicate, equality, NonZero guard, and canonical-row counts;
- source and mapped polynomial terms and exponent entries;
- row comparisons, deduplication operations, and lineage entries;
- p, K, L, M, transform, beta, and Cproj integer entries;
- Euclidean steps, pairwise Bezout steps, and exact divisions, with Euclidean
  loop divisions and proof-bearing exact divisions counted separately;
- matrix multiply-adds and verification operations;
- maximum integer coefficient bits and total GMP integer-bit work;
- retained integer entries, integer bits, and logical bytes;
- fresh temporary, retained, replay, and payload-comparison peaks;
- payload comparison units, bytes, and integer bits.

At a live dimension s, forming alpha costs at most r s multiply-adds and
forming delta costs at most r additional products/additions. One row uses at
most s-1 pair transforms. Each pair transform touches two K columns and two L
rows, each of length at most r. For m equality rows, the straightforward
bounded implementation therefore uses O(m r squared) exact arithmetic
operations. The solver state retains at most approximately 2 r squared + r
integers, before certificate transcript payloads. The lifted compact map adds
N r + N integer entries.

These are census formulas, not permission for unchecked work. Preflight every
shape before allocation. Before each GMP operation use prospective bit-size
formulas in the style of the current Budget, and verify every Bezout identity,
every recorded unimodular transform (including determinant-negative
permutations and sign changes), and every exact division. A one-below resource
limit must fail before the first operation belonging to the over-budget phase.

The V1 `Budget::quotient_remainder` increments its Euclidean-step statistic for
every quotient, including an exact division. The new kernel must not inherit
that counter meaning. It uses distinct bounded `quot_rem_euclidean` and
`quot_rem_exact` paths (or an equivalent explicit operation kind), while both
retain the same prospective integer-bit and panic-containment discipline.

Compile and replay receive separate phase budgets. Replay statistics are
recomputed rather than accumulated onto compile statistics. The parent
logical-memory envelope includes the retained transcript, generalized map, and
composition plan, while excluding shared source Arcs according to the existing
owner policy.

## 12. Generic acceptance matrix

All fixtures below are topology-neutral. Concrete integral topologies may use
the resulting rules for end-to-end validation, but no fixture authorizes
topology-specific logic in the compiler.

### 12.1 Minimal quotient fixture

Identity map on N=2 with:

    P = n0+n1-3.

Accept either deterministic equivalent parameterization described in Section
7. Verify:

- P composed with F is zero;
- F composed with F equals F;
- exact fixed-point membership;
- image points satisfy n0+n1=3 and every such integer point is represented;
- P J(n+e0)+J(n)=0 specializes to the lower pivot J(F(n))=0.

### 12.2 Non-unit congruence

    2 n0 + 3 n1 = 0.

Compilation succeeds with rank-one basis plus or minus (3,-2). This is the
essential regression beyond the current V1 unit-column boundary.

### 12.3 Divisibility obstruction

    2 n0 + 4 n1 - 3 = 0.

Compilation proves empty because gcd(2,4)=2 does not divide 3. Replay the exact
delta, gcd, and remainder witness.

### 12.3a Negative-anchor normalization

Use the canonical primitive one-variable equation

    2 - t0 = 0.

Here alpha is (-1), delta is (-2), and the positive gcd is one. Compilation
must record the anchor-sign transform and produce the rank-zero point t0=2.
Using z0=delta/g before the sign transform would incorrectly produce t0=-2.
Replay must reconstruct and authenticate the sign-transform transcript.

### 12.4 Simultaneous multi-equality system

For N=3:

    2 x + 3 y = 0,
    x + y + z = 5.

The solution is, up to basis sign:

    (x,y,z) = (0,0,5) + t (3,-2,-1).

Adding

    4 x + 6 y = 0

is redundant. Replacing it with

    4 x + 6 y = 1

is inconsistent. Both outcomes need exact lineage and replay witnesses.

### 12.5 Zero-dimensional intersection

    x+y=3,
    x-y=1.

The refined image is the single point (2,1), with rank zero and an idempotent
constant map.

### 12.6 Refinement of an inherited map

Start from:

    n1 = 3-n0

and add:

    n0 + 2 n2 = 4.

Verify that F0 composed with F1 equals F1, the new image is an exact subset of
the inherited image, and all inherited and new equalities vanish.

### 12.7 Mapped constants and redundant predicates

- An equality already zero after inherited composition is redundant.
- An equality which maps to a nonzero constant proves the child empty.
- Duplicate and integer-multiple affine rows merge lineage without changing
  the solution lattice.

### 12.8 Base factors and nonlinear boundary

- theta times (n0+n1-3), for a common nonzero base factor admitted by the
  existing atom recognizer, produces the same primitive row.
- theta n0+n1 has nonassociate parameter blocks and returns the typed
  quotient-required boundary.
- n0 n1-1 is nonlinear and returns the typed quotient-required boundary.

Neither unsupported case may be pruned or labeled a master.

### 12.9 NonZero behavior

- EqualZero P together with NonZero P is contradictory after refinement.
- An unrelated free-index-dependent NonZero predicate remains a guard.
- A guard mapping to a nonzero constant is discharged according to field
  policy.
- A relation denominator mapping to zero makes that candidate unavailable and
  is routed to the exceptional-domain machinery.

### 12.10 Arbitrary precision and adversarial replay

- Use coprime coefficients larger than 128 bits and verify exact basis,
  Bezout, and membership identities with no narrowing conversion.
- For every positive resource counter, test the exact bound and one below.
- Tamper each transform, p/K/L entry, generalized-map entry, rank, predicate
  lineage, source binding, and statistic.
- Reject an equal but independently allocated authority owner.
- Drop external source handles and replay from the sealed certificate.
- Replay the same immutable certificate concurrently from four threads and
  require identical results.

## 13. Required scope versus optional optimization

Ultimate requirements for affine recursive completeness:

- compose every EqualZero predicate through the inherited map;
- solve all simultaneous integer-affine equations exactly, including
  non-unit congruences and divisibility;
- introduce a generalized map rather than falsifying V1 free-row invariants;
- compose every next-epoch coefficient numerator, denominator, and guard;
- retain and correctly classify NonZero predicates;
- regenerate or specialize authenticated source identities and re-eliminate;
- provide complete authority binding, fresh-plan control, limits,
  panic containment, certificate, and independent replay.

Required later for full LiteRed parity, but a separate implementation seam:

- bounded polynomial-ideal normal forms for general nonlinear EqualZero loci;
- saturation-aware NonZero reasoning;
- integer inhabitation beyond affine lattices.

Permitted current production subset:

- one already-current-coordinate affine `EqualZero` predicate;
- Symbolica-backed primitive-row recognition/normalization;
- a deterministic literal `+1` or `-1` pivot on an active free coordinate;
- Symbolica-native integer-matrix composition with compact-map replay;
- exact verification that the consumed equality maps to zero;
- typed `AlreadySatisfied`, `ProvedEmpty`, nonlinear/multiple-equality, and
  `RequiresIntegerNormalForm` unsupported outcomes.

The existing RustRed integer-system and integer-lattice-kernel modules are not
production authorities. A no-unit case must not silently call them.

Optional optimizations after the missing public Symbolica capability exists:

- a batch Smith or Hermite normal-form implementation;
- canonical lattice keys for merging geometrically equal children;
- retaining the unit-pivot compiler as a proven fast path;
- eliminating redundant compact support columns;
- synthetic minimal parameter variables;
- cached but independently authenticated composition plans.

None of the optional items changes the topology-neutral semantics above.
