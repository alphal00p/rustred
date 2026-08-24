# Generic IBP and adaptive derivation parity audit

Date: 2026-08-13

This is a source audit of RustRed's loop-count-independent scalar family,
parametric identity, shift-operator, sector, zero-sector, elimination, rule, and
adaptive-search layers against the vendored Mathematica implementation in
`vendor/LiteRed2/Source/LiteRed2026.m`.  It distinguishes algebraic correctness
from still-missing LiteRed scope.  It does not treat the loop-named legacy
reducers as evidence of generic rule discovery.

## Result

No unresolved P0 correctness defect remains.  The audit found one P0 at the
public certification boundary: a same-family caller-supplied relation was not
bound to freshly generated `IBPLI`.  It was fixed during the audit by exact
ordered source-row authentication and by making the low-level generic-quotient
constructor crate-private; an adversarial forged-row test now enforces the
boundary.  No P0 was found in the identity-generation, translation,
elimination, or zero-sector formulas themselves.

`GenerateIBP` parity is implemented for an authenticated **complete affine
denominator basis**: the production code is parameterized by `L`, `E`, the
kinematics, the affine basis, and symbolic power shifts.  It does not dispatch
on loop count or topology.  This statement is narrower than full LiteRed
parity: independent short families can now be completed with ISPs, but
overcomplete families, completed `ToAB`, sector-wide symbolic `SolvejSector`
rule coverage, and the complete `AnalyzeSectors`/`FindSymmetries` workflow
remain missing.

The most important distinction is:

- the generated IBP and LI **relations** are fully parametric elements of
  `K(n)` and replay exactly;
- RustRed now derives an initial finite symbolic partition of a complete
  integer sector into guarded descending, proved-empty, uncovered, and
  unsupported leaves, and schedules those searches family-wide; but
- it does not yet iterate the exceptional-locus search to LiteRed's complete
  `SolvejSector` fixed point, feed solved subsectors into supersectors, or
  select the final master set.

## Implemented parity

### Complete affine family algebra

RustRed orders scalar products as upper-triangular loop--loop products followed
by loop--external products
([`generic_family.rs:40-54`](../../src/generic_family.rs#L40)).  The family
requires exactly

```text
N = L(L+1)/2 + L E
```

affinely independent denominators
([`generic_family.rs:423-470`](../../src/generic_family.rs#L423)), constructs the
exact inverse basis, retains input-denominator and determinant guards, and
replays the inverse
([`generic_family.rs:564-613`](../../src/generic_family.rs#L564)).  Derivatives
correctly contribute twice on a diagonal loop--loop coordinate because both
branches in `d(k_i.k_i)/d k_i` fire, once on an off-diagonal coordinate, and
use the external Gram matrix for external contractions
([`generic_family.rs:907-1002`](../../src/generic_family.rs#L907)).

This matches the scalar-product differentiation inside LiteRed
`GenerateIBP` ([`LiteRed2026.m:1813-1818`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1813)).

### Ordinary IBPs: signs, shifts, and row order

If

```text
q . partial_i D_r = a_r + sum_t b_rt D_t,
```

the generated Rust relation is

```text
delta(q,k_i) d J(n)
- sum_r (n_r + nu_r) a_r J(n + e_r)
- sum_rt (n_r + nu_r) b_rt J(n + e_r - e_t) = 0.
```

The implementation is at
[`parametric_ibp.rs:223-289`](../../src/parametric_ibp.rs#L223).  In particular,
the minus sign is introduced at
[`parametric_ibp.rs:396-420`](../../src/parametric_ibp.rs#L396), and power shifts
are added to the free indices before multiplication
([`parametric_ibp.rs:229-243`](../../src/parametric_ibp.rs#L229)).  LiteRed's
corresponding formula and final power-shift substitution are at
[`LiteRed2026.m:1813-1817`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1813).

Both implementations flatten `Outer[..., qms, lms]` in contraction-major,
differentiated-loop-minor order.  RustRed documents and implements this at
[`parametric_ibp.rs:98-100`](../../src/parametric_ibp.rs#L98) and
[`parametric_ibp.rs:245-250`](../../src/parametric_ibp.rs#L245).  The row count is
`L*(L+E)`; checked count arithmetic is at
[`parametric_ibp.rs:514-525`](../../src/parametric_ibp.rs#L514).

### Lorentz-invariance rows

LiteRed forms the antisymmetric external pair in the orientation
`M_ba - M_ab` and appends LI rows after ordinary IBPs
([`LiteRed2026.m:1818-1831`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1818)).
RustRed uses the same orientation and lexicographic external-pair order at
[`parametric_ibp.rs:313-370`](../../src/parametric_ibp.rs#L313), producing
`E*(E-1)/2` rows.

Multiplication by a denominator `D_t` is a translation of the **whole** source
relation by `-e_t`, including every occurrence of `n` in coefficients and
guards, not merely a shift of integral keys.  The LI construction performs that
translation at
[`parametric_ibp.rs:425-484`](../../src/parametric_ibp.rs#L425).  The underlying
operation translates keys, coefficients, and guard provenance together at
[`parametric_relation.rs:600-638`](../../src/parametric_relation.rs#L600), with
the Symbolica polynomial substitution `n_i -> n_i + delta_i` at
[`parametric_coefficient.rs:1195-1259`](../../src/parametric_coefficient.rs#L1195).

### Exact elimination and replay

RustRed performs sparse ordered Gaussian elimination over authenticated `K(n)`.
Every nonconstant pivot division adds a nonzero guard, every pivot retains its
source-row reduction trace, and construction immediately replays the result
([`parametric_elimination.rs:335-513`](../../src/parametric_elimination.rs#L335)).
Replay reconstructs every pivot, verifies the divisor and complete guard
provenance, and reduces every original source row to zero
([`parametric_elimination.rs:553-690`](../../src/parametric_elimination.rs#L553)).

This is a sound Symbolica-native alternative to LiteRed's incremental `Solvej`
database.  Exact pivot sequence parity is neither required nor claimed.

### Concrete guarded rule use

A compiled pivot retains the complete source system and elimination proof
([`parametric_rules.rs:54-105`](../../src/parametric_rules.rs#L54)) and rebuilds
it during replay
([`parametric_rules.rs:267-306`](../../src/parametric_rules.rs#L267)).  At an
integer point it rejects a vanished pivot/domain guard, a surviving sector
leak, or a non-descending RHS term
([`parametric_rules.rs:327-431`](../../src/parametric_rules.rs#L327)).  The leak
test is coefficient-aware because zero concrete coefficients have already been
collected away.  This is the correct concrete analogue of the numerator part of
LiteRed `WhenBad` ([`LiteRed2026.m:2565-2568`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2565)).

### Adaptive stencil geometry

LiteRed's `diamond[l,d]` is the exact `L1` shell of radius `d`
([`LiteRed2026.m:6094-6097`](../../vendor/LiteRed2/Source/LiteRed2026.m#L6094));
`preparepoints` keeps the requested sector and orders accepted points
([`LiteRed2026.m:2682-2710`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2682)).
RustRed enumerates the same exact shell with a heap stack
([`adaptive_rules.rs:740-813`](../../src/adaptive_rules.rs#L740)), filters to the
requested sector, sorts by its persisted order, and accumulates translated
source rows across depths
([`adaptive_rules.rs:378-483`](../../src/adaptive_rules.rs#L378)).

### Feynman-polynomial zero criterion

LiteRed builds, for every monomial of `G=U+F`, the row
`[x_i partial_i monomial, monomial]`, evaluates it on the effective sector face,
and declares the sector zero when its rank is at most the number of active
parameters
([`LiteRed2026.m:3020-3051`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3020)).

On an active-face monomial `c x^alpha`, every nonzero LiteRed row is a nonzero
scalar multiple of `[alpha_active, 1]`.  RustRed's exponent-row rank test is
therefore exactly rank-equivalent, not a heuristic.  A deficient result carries
a primitive integer kernel and is replayable
([`zero_sectors.rs:273-350`](../../src/zero_sectors.rs#L273)); full column rank is
explicitly only `NoZeroCertificate`, not a proof that the integral is nonzero
([`zero_sectors.rs:353-399`](../../src/zero_sectors.rs#L353)).  All admissible
masks are tested directly and monotone closure is then checked
([`zero_sectors.rs:636-698`](../../src/zero_sectors.rs#L636)).

## Findings

### P0 found and fixed: certified rewrites were not bound to generated identities

The adaptive relation and elimination types deliberately accept caller-owned
algebraic rows.  That is appropriate for a generic algebra layer, but the
`CertifiedFamilyRuleProvider` initially checked only family/context/order
metadata.  A fabricated relation carrying the same public family fingerprint
could consequently reach the generic-quotient certification path.  Likewise,
the low-level `CertifiedConcreteRewrite::from_parametric_quotient` constructor
was public and checked only the candidate's family fingerprint.  An algebraic
replay of a caller row is not proof that the row is a physical IBP.

The provider now regenerates canonical `IBPLI` in the adaptive context, requires
the exact ordered row count, and compares every row with
`has_identical_guard_provenance`
([`certified_rule_provider.rs:92-116`](../../src/certified_rule_provider.rs#L92)).
A mismatch is a typed `UnauthenticatedSourceRows` error.  The only remaining
generic-quotient constructor is crate-private
([`certified_rewrite.rs:288-307`](../../src/certified_rewrite.rs#L288)), so the
public certified boundary cannot be bypassed.

The adversarial regression replaces generated row zero by an empty relation
with the same family fingerprint, context, and row identifier, and requires
rejection at row zero
([`certified_two_loop_vakint_oracle.rs:41-76`](../../tests/certified_two_loop_vakint_oracle.rs#L41)).
The fixed scalar two-loop test binary passes both this attack and the normal
Vakint reduction/replay case.

### P1: symbolic `SolvejSector` rule coverage remains incomplete

LiteRed keeps a list of uncovered symbolic integer cases, solves rows, derives a
`WhenBad` condition, records the covered case, reduces the complement, and
recurses until the sector is partitioned or remaining points are recorded as
current master candidates
([`LiteRed2026.m:2430-2523`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2430)).
`WhenBad` combines pivot-denominator zero loci with coefficient-aware sector
leaks and simplifies them under integer sector constraints
([`LiteRed2026.m:2565-2568`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2565)).

RustRed's demand-time adaptive search still returns `Uncovered` on exhaustion
([`adaptive_rules.rs:1-11`](../../src/adaptive_rules.rs#L1),
[`adaptive_rules.rs:606-613`](../../src/adaptive_rules.rs#L606)).  The symbolic
foundation has advanced beyond that path: `symbolic_sector_cases.rs` records
the exact orthant and finite complementary splits, `when_bad.rs` compiles
coefficient-domain, inactive-boundary-leak, and uniform-descent conditions,
and `generated_when_bad.rs` rejects candidates not derived from freshly
regenerated IBP/LI rows.  `parametric_sector_coverage.rs` composes a supplied
ordered candidate set into a replayed finite partition with explicit
`DescendingRule`, `Uncovered`, and `Unsupported` leaves;
`parametric_sector_provider.rs` applies those certified descending leaves.
`generated_sector_discovery.rs` now performs the first candidate search
automatically from only the family, sector, ordering, and bounded policies:
it regenerates `IBPLI`, grows the cumulative corner stencil, eliminates it,
authenticates every retained pivot, and freezes the coverage.  Thus generated
rule attachment and automatic initial-sector discovery are implemented.

For a concrete demand, the certified numeric path now specializes and
zero/symmetry-quotients source rows before re-eliminating over `K`; it therefore
does handle a rank-changing integer point where a generic `K(n)` pivot guard
vanishes.  The symbolic exceptional-locus primitives have also landed:
`coordinate_equality_loci.rs` derives exact sparse assignments from expanded
`K*(n_i-c)` predicates, and `conditional_reelimination.rs` regenerates,
translates, partially specializes, and re-eliminates generated rows on such an
assignment. `conditional_rules.rs` binds an individual re-eliminated pivot to
its centered assignment and sector, applies it with strict descent, preserves
base assumptions, and retains a fully replayable scalar/tensor trace without
ever exposing a global candidate.  The first automatic **symbolic** driver is
now implemented: `generated_sector_live_leaf_queue.rs` visits every root
`Uncovered`/`Unsupported` leaf in stable order, extracts exact coordinate
loci, and runs generated-row partial re-elimination;
`generated_sector_conditional_provider.rs` installs the resulting
condition-bound pivots only as a fallback on those root terminal leaves.
Root descending rules remain authoritative, exhausted searches still delegate
as `Uncovered`, and exact coordinate contradictions are retained as replayable
`ProvedEmptyLocus` branches rather than queued as inhabited cases.

Remaining work:

1. repeat the automatic candidate/stencil search recursively for live leaves
   that remain after the first coordinate-locus re-elimination pass;
2. extend exact integer-domain simplification beyond coordinate equalities;
3. reapply analytic zero and concrete symmetry quotients at the appropriate
   specialized stage; and
4. attach replayed rules or caller-owned explicit terminals to every inhabited leaf and
   retain the resulting sector-wide coverage certificate.  Search exhaustion
   must remain `Uncovered`, not be promoted to a proved master.

### P1: completed LiteRed `ToAB` is absent

The current operator layer correctly implements left-to-right actions

```text
A_i J(n) = n_i J(n+e_i),   B_i J(n) = J(n-e_i),
A_i B_i = n_i,             B_i A_i = n_i-1,
A_i B_i - B_i A_i = 1.
```

The action is tested at
[`shift_operators.rs:916-1014`](../../src/shift_operators.rs#L916), and arbitrary
words can be evaluated back into `K(n)` relations.  However, the module
explicitly says that it is not completed `ToAB`: free index variables remain in
coefficients
([`shift_operators.rs:20-27`](../../src/shift_operators.rs#L20),
[`shift_operators.rs:660-664`](../../src/shift_operators.rs#L660)).  Its exact
relation/primitive-shift round trip
([`shift_operators.rs:1120-1190`](../../src/shift_operators.rs#L1120)) is useful,
but it is a different normal form.

LiteRed completed `ToAB` does all of the following
([`LiteRed2026.m:1968-1972`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1968),
[`LiteRed2026.m:2007-2023`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2007)):

1. collect a linear recurrence and find the componentwise maximal integral
   shift `s`;
2. rebase the recurrence from `n` to `n-s`;
3. replace every free `n_i` in scalar coefficients by `A_i B_i-s_i`;
4. represent a term of shift `delta` with `B_i^(s_i-delta_i)`;
5. distribute in the noncommutative algebra and repeatedly divide any common
   `B_i` factor from the **left**, respecting `B_i A_i=A_i B_i-1`; and
6. serialize monomials as ordered `A_i`, `(A_i B_i)`, and `B_i` words.

The initial common-shift rebasing and later common-left-`B` divisions mean that
`FromAB[ToAB[R]]` is generally a globally translated normalization of the
identity `R=0`, not necessarily the same sparse relation keys.  A Rust port
should retain the removed global translation as a replay witness so it can
offer both exact reconstruction and LiteRed-normalized equivalence.

`FromAB` evaluates an operator word left-to-right: each `A_i` records the
current cumulative offset as a factor `n_i+offset`, then increments that offset;
each `B_i` decrements it; the final integral receives the net offset
([`LiteRed2026.m:1979-1985`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1979),
[`LiteRed2026.m:2037-2041`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2037)).
Current word execution already supplies this algebraic primitive, but the
completed index-free operator polynomial, normalization witness, public
`ToAB`/`FromAB` round trip, `AtoLeft`, and `ABIBP`/`ABLI`/`ABIBPLI` wrappers are
missing.  LiteRed's wrappers are at
[`LiteRed2026.m:2091-2104`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2091).

Minimum acceptance tests for the completed layer should cover mixed positive
and negative shifts, coefficients involving several indices and symbolic power
shifts, the `AB-BA=1` commutator, common-left-`B` division, no residual `n`
variables, replay of the removed global translation, and absence of spurious
`n_i != 0` chart guards.  Index-dependent rational denominators must either be
cleared with retained guards before conversion or rejected by a typed error;
they must not be silently treated as base-field coefficients.

LiteRed's `TildeConjugate`, `InverseTildeConjugate`, and `FromTildeAB`
([`LiteRed2026.m:1988-1995`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1988),
[`LiteRed2026.m:2044-2088`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2044))
are an additional, separate missing AB facility.

### Numeric zero/symmetry quotient timing

For a fully numeric point LiteRed submits

```text
Join[ids@@point, SR[basis]@@point] /. ZerojRule[basis]
```

to elimination
([`LiteRed2026.m:2471-2480`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2471)).
`SR` is the difference between an integral and each verified self-symmetry image
([`LiteRed2026.m:815-820`](../../vendor/LiteRed2/Source/LiteRed2026.m#L815));
`FindSymmetries` constructs mapped-sector and self-sector rules at
[`LiteRed2026.m:3445-3468`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3445).

RustRed now exposes the same point layers directly
([`adaptive_rules.rs:169-232`](../../src/adaptive_rules.rs#L169)).  The certified
provider specializes every generated row at every accepted point, erases terms
with replayable analytic/cut-zero witnesses, maps all other terms along
replayed symmetry paths, and only then performs exact elimination over the base
field
([`certified_rule_provider.rs:228-264`](../../src/certified_rule_provider.rs#L228),
 [`certified_rewrite.rs:495-735`](../../src/certified_rewrite.rs#L495)).  This
avoids the incompleteness of eliminating over generic `K(n)` first on an index
locus where rank changes.  The generic `K(n)` candidate path remains only a
fallback.

For a single concrete demand, point preparation is semantically aligned with
LiteRed's numeric `preparepoints`: both enumerate the exact `L1` shell, retain
only points in the source sector, sort each layer from easier to harder under
the selected order, and retain all prior-layer equations.  RustRed uses its
documented deterministic order rather than an identical `jsOrder` matrix.

The concrete proof retains the generated row ordinal, assignment, raw
specialization, every quotient witness, collected equation, exact column list,
elimination trace/checksums, and selected pivot
([`certified_rewrite.rs:173-224`](../../src/certified_rewrite.rs#L173)).  Replay
regenerates the ordinary/LI rows, respecializes them, replays every zero and
symmetry witness, rebuilds exact elimination, and compares raw guard
provenance, collected rows, columns, elimination identities, selected pivot,
final RHS, guards, domain, and descent witnesses
([`certified_rewrite.rs:809-943`](../../src/certified_rewrite.rs#L809),
[`certified_rewrite.rs:1075-1160`](../../src/certified_rewrite.rs#L1075)).

This is numeric-point parity.  Complete symbolic `WhenBad` compilation,
automatic initial corner-stencil discovery, exact coordinate-locus extraction,
generated-row conditional re-elimination, the live exceptional-leaf queue,
and condition-bound concrete pivot installation now exist as proof-bearing
layers.  They do not yet constitute a recursively complete symbolic sector
solve.
Canonical quotienting is algebraically equivalent
to appending the verified `J-image(J)` relations and eliminating the image
variables; it need not reproduce LiteRed's intermediate row ordering.

### P1: `AnalyzeSectors` and symmetry workflow remains partial

RustRed's rank certificate matches LiteRed's default zero predicate.  The new
`family_sector_inventory.rs` layer owns the exact restrictions and
power-shift policy, inventories every raw mask, and produces a replayable
subsector-first queue of masks for which the sufficient zero test returned no
certificate.  It deliberately calls those masks unresolved, not analytically
nonzero, and never infers masters.  It does not yet expose LiteRed's
`SimpleSectors`, `BasisSectors`, maximal-zero `ZerojRule`, or an operational
`NonZeroSectors` claim.  LiteRed constructs those sets at
[`LiteRed2026.m:3052-3082`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3052);
RustRed's underlying result surface is at
[`zero_sectors.rs:391-439`](../../src/zero_sectors.rs#L391), with family-wide
orchestration in [`family_sector_inventory.rs`](../../src/family_sector_inventory.rs).

This matters because full `FindSymmetries` starts from `SimpleSectors` and
`NonZeroSectors`, groups restricted Feynman polynomials, solves exact momentum
maps, and extends them to supersectors
([`LiteRed2026.m:3320-3444`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3320)).
RustRed's bounded internal-vacuum symmetry search is useful and proof-bearing,
but it is not this complete workflow.  General external-momentum symmetries,
cross-basis maps, and complete sector-orbit construction remain missing, as the
README already acknowledges.

Cuts are intentionally typed as exclusions in the sector/zero analyzer rather
than analytic zero proofs
([`sectors.rs:12-14`](../../src/sectors.rs#L12),
[`sectors.rs:523-548`](../../src/sectors.rs#L523)).  A combined reduction layer
may implement the cut boundary as zero, but must retain that distinct
provenance.  Pattern exclusions must not be converted into zero statements.

### P1: family intake remains narrower than full `NewDsBasis`

The exact affine core accepts a square complete basis
([`generic_family.rs:465-499`](../../src/generic_family.rs#L465)).  A new
front-end implements exact rank-increasing identity-row ISP completion for an
independent short basis, with zero shifts and replay
([`automatic_isps.rs`](../../src/automatic_isps.rs)).  This follows
LiteRed's `append[m,IdentityMatrix[Length[sps]]]` algorithm
([`LiteRed2026.m:316`](../../vendor/LiteRed2/Source/LiteRed2026.m#L316),
[`LiteRed2026.m:783-797`](../../vendor/LiteRed2/Source/LiteRed2026.m#L783)) but
uses RustRed's persisted scalar-product coordinate order.  Mathematica's
`Union` can order symbolic scalar products differently, so exact ordinal parity
is not claimed; the resulting full-rank bases are equivalent.

Dependent/overcomplete denominator sets and partial fractioning remain
missing.  Therefore “arbitrary loop count” is correct within the authenticated
complete-or-independently-completable family class, not arbitrary LiteRed
input-family parity.

### First generic two-loop validation rung reached

The equal-mass two-loop vacuum family is now reduced through the generic chain:
four generated ordinary IBPs, analytic zero certificates, automatically
discovered and verified internal symmetries, numeric pre-quotient elimination,
and demand reduction with only explicitly selected masters.  Five independent
Vakint/alphaLoop scalar fixtures include pinches, dots, and negative powers
([`certified_two_loop_vakint_oracle.rs:90-189`](../../tests/certified_two_loop_vakint_oracle.rs#L90)).
Every retained rule/zero application is replayed.

A second test projects and lowers Vakint's complete three-summand two-loop
tensor source, derives every scalar recurrence from the same four generic rows,
leaves the two masters unsubstituted, and matches the frozen alphaLoop tensor
coefficients
([`vakint_two_loop_tensor_ibp_oracle.rs:96-237`](../../tests/vakint_two_loop_tensor_ibp_oracle.rs#L96)).
No production topology-specific recurrence is imported.

This establishes the requested first two-loop validation rung, not a finite
symbolic coverage proof for every integral in every two-loop sector.  The P1
`SolvejSector` case-partition gap therefore remains relevant.

### P2: persisted order deliberately differs from LiteRed's default

LiteRed permits a per-sector order matrix and defaults to total signed power,
then numerator power, then denominator/numerator tie rows
([`LiteRed2026.m:1378-1441`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1378),
[`LiteRed2026.m:1533-1549`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1533)).
RustRed deliberately persists one fixed key
`corner-distance,dots,numerators,index-excess`
([`sectors.rs:23-35`](../../src/sectors.rs#L23),
[`sectors.rs:646-753`](../../src/sectors.rs#L646)).  It is a strict,
well-founded alternative, but it can reverse LiteRed's choice between a dot and
a numerator at equal total distance.  This is an allowed algorithmic
difference, not exact ordering parity.  Supporting configurable deterministic
orders would improve compatibility.

### Independent generator coverage now spans loop count and multi-loop LI

The independent row oracle now covers one-loop vacuum, one loop with one or two
external momenta (including LI), two-loop vacuum, complete two-loop families
with one and two external momenta, the three-loop massive tetrahedron, and a
complete five-loop massive-vacuum basis
([`parametric_ibp_oracle.rs:605-1119`](../../tests/parametric_ibp_oracle.rs#L605)).
The physical `L=2,E=2` case checks all eight ordinary rows and its LI row at
three assignments against independent differentiation/inverse-basis code
([`parametric_ibp_oracle.rs:824-962`](../../tests/parametric_ibp_oracle.rs#L824)).
The three-loop test checks all nine ordinary rows at three assignments; the
five-loop test checks all 25 ordinary rows at two assignments.  This is strong
evidence that ordinary row generation is loop-count-parametric and closes the
previous multi-loop LI oracle gap.  These are row-generation tests, not four-
or five-loop sector-reduction coverage.

## Tests run

The following licensed GMP builds were run with four-way nextest parallelism;
no `no_gmp` feature was enabled (`Cargo.toml:14` selects `gmp`):

```sh
SYMBOLICA_HIDE_BANNER=1 cargo nextest run \
  --test parametric_ibp_oracle \
  --test parametric_elimination_black_box_audit \
  --test parametric_rules \
  --test adaptive_rules \
  --test zero_sectors \
  --test zero_sector_parametric_oracle_audit \
  --jobs 4
```

Result: **41 passed, 0 failed, 0 skipped**, across six test binaries in 0.933 s
after compilation.  The only output was pre-existing deprecation warnings in
vendored `numerica` and loop-specific tensor code.

After the certified numeric provider and P0 source-authentication fix landed,
the new scalar and tensor two-loop oracles were rerun together:

```sh
SYMBOLICA_HIDE_BANNER=1 cargo nextest run \
  --test certified_two_loop_vakint_oracle \
  --test vakint_two_loop_tensor_ibp_oracle \
  --jobs 4
```

Result: **3 passed, 0 failed, 0 skipped** in 1.349 s after compilation.  This
includes the adversarial forged-row rejection, five scalar Vakint fixtures with
owned proof replay, and the full tensor-polynomial fixture.

After the higher-loop oracle additions, the generator oracle was rerun with the
same licensed GMP build and four-way nextest parallelism:

```sh
SYMBOLICA_HIDE_BANNER=1 cargo nextest run \
  --test parametric_ibp_oracle \
  --jobs 4
```

Result after the physical two-loop/two-external LI oracle was added:
**8 passed, 0 failed, 0 skipped**.  This includes the three-loop nine-row and
five-loop 25-row vacuum cases; the other six cases cover one- and two-loop
affine families, external momenta, LI, rational basis coefficients, power
shifts, and negative indices.

The final combined licensed run used four-way nextest parallelism for automatic
ISP completion, symbolic sector cases, and this generator oracle: **21 passed,
0 failed, 0 skipped**.

The strongest independent checks are:

- all generated ordinary and LI rows specialized at several integer points and
  compared against a separately implemented derivative/inverse-basis oracle
  ([`parametric_ibp_oracle.rs:28-577`](../../tests/parametric_ibp_oracle.rs#L28));
- a hand-derived two-row echelon form plus full source-manifest and guard-
  provenance replay failures
  ([`parametric_elimination_black_box_audit.rs:60-184`](../../tests/parametric_elimination_black_box_audit.rs#L60));
- generated tadpole rules for active and numerator sectors
  ([`parametric_rules.rs:26-166`](../../tests/parametric_rules.rs#L26));
- adaptive generated-row tadpole reductions and a synthetic cumulative-depth
  stencil with no hardcoded recurrence
  ([`adaptive_rules.rs:29-228`](../../tests/adaptive_rules.rs#L29));
- hand rank tables and all eight two-loop sunset masks
  ([`zero_sector_parametric_oracle_audit.rs:350-451`](../../tests/zero_sector_parametric_oracle_audit.rs#L350));
- generic generated-rule scalar reduction and forged-row rejection
  ([`certified_two_loop_vakint_oracle.rs:41-189`](../../tests/certified_two_loop_vakint_oracle.rs#L41));
- FORM-free generic tensor projection, family lowering, generated scalar
  reduction, frozen alphaLoop comparison, and composed replay
  ([`vakint_two_loop_tensor_ibp_oracle.rs:96-237`](../../tests/vakint_two_loop_tensor_ibp_oracle.rs#L96)); and
- a three-loop massive tetrahedron with all nine ordinary generated IBPs checked
  at three assignments against the independently implemented derivative and
  inverse-basis oracle
  ([`parametric_ibp_oracle.rs:804-861`](../../tests/parametric_ibp_oracle.rs#L804)); and
- a physical complete five-loop massive-vacuum basis with all 25 ordinary IBPs
  checked at two assignments against the same independent oracle
  ([`parametric_ibp_oracle.rs:864-923`](../../tests/parametric_ibp_oracle.rs#L864)).

Concrete topologies in these tests are oracles only.  No audited production
formula branches on their names or loop counts.

## README claim audit

The current README does **not** claim complete symbolic `SolvejSector` parity.
It distinguishes finite generated-source candidate coverage and the first
replayable live-leaf/conditional-provider pass from the still-missing iterated
symbolic zero/symmetry quotient solver, preserves `Uncovered` rather than
inventing a master, and labels concrete two-loop-or-higher tests as validation
rather than unrestricted sector proofs.  Its claim that all ordinary IBP and LI relations
are generated for arbitrary `L` and `E` remains supported under the stated
complete-affine-basis qualifier.  The limitations list now also names
completed `ToAB`/`FromAB`, generic persistence, complete `AnalyzeSectors`,
partial fractions, and dimensional/differential utilities as missing scope.
