# Four-loop factorized scalar `D1/N0` halo closure

## 1. Scope and present obstruction

[`FourLoopCornerShell`](../../src/four_loop_corner_shell.rs) generates the 160
native corner rows of the ten frozen H/X representatives and transports every
raw term into a common exact basis.  Its remaining obstruction is narrow.  A
transported term whose scalar corner is factorized is closed only when the term
equals that corner.  A dot or numerator on the same proved factorization is
instead retained as an [`UnsupportedBoundaryHalo`](../../src/four_loop_corner_shell.rs#L304).

The observed certificate before this closure has

```text
complete rows       65
blocked rows        95
blocker occurrences 234
census buckets      28
rank of admitted rows 65
free/unresolved columns 39
normalization contributions 3192
elimination updates 1632
```

The rank and free-column counts describe only the 65 admitted rows.  They are
not properties of the complete 160-row shell.  This milestone must first turn
all 234 factorized occurrences into proved product columns and then rerun exact
elimination; it must not preserve or extrapolate those partial rank numbers.

The implemented closure performs that rerun: all 160 rows are admitted, the
exact rank is 159, and 64 columns remain free in this finite shell. The latter
are unresolved shell coordinates, not a claimed master basis.

More importantly, exhaustive runtime census shows that **all 234 blockers are
H-family `D=1,N=0` scalar one-dot terms**.  There are no X-family blockers and
no numerator blocker.  The 28 frozen buckets are:

| H mask | product | occurrences |
|---:|---|---:|
| 43 | `T1^4` | 2 |
| 47 | `T1^2*S2` | 2 |
| 63 | `T1*F5` | 10 |
| 75 | `T1^4` | 3 |
| 79 | `T1*B4` | 9 |
| 95 | `T1*F5` | 11 |
| 99 | `T1^4` | 8 |
| 103 | `T1^2*S2` | 9 |
| 105 | `T1^4` | 7 |
| 106 | `T1^4` | 8 |
| 109 | `T1*B4` | 8 |
| 110 | `T1^2*S2` | 9 |
| 119 | `T1*F5` | 11 |
| 125 | `T1*F5` | 10 |
| 126 | `S2^2` | 13 |
| 143 | `T1*B4` | 5 |
| 199 | `T1*B4` | 13 |
| 203 | `T1*B4` | 7 |
| 205 | `T1*B4` | 10 |
| 206 | `T1*B4` | 10 |
| 221 | `T1*F5` | 12 |
| 287 | `T1*F5` | 6 |
| 303 | `T1*F5` | 6 |
| 311 | `T1*F5` | 9 |
| 317 | `T1*F5` | 9 |
| 318 | `T1*F5` | 9 |
| 351 | `T1*M6` | 6 |
| 381 | `T1*M6` | 12 |

The counts sum to 234.  This is the acceptance inventory for the next
implementation, not diagnostic output that may silently drift.

The required public domain is therefore exactly the observed factorized scalar
one-dot halo:

```text
arity                 10 parent-family denominator entries
positive sector       exactly witness.sector_mask()
total physical dots   D = sum_active max(a_i-1,0) = 1
total numerator power N = sum_i max(-a_i,0) = 0
positive auxiliaries  forbidden
```

All positive entries must be physical propagators and every other entry must be
zero.  Numerator transport is not needed to complete the current 160 rows and
must not be added to the advertised surface merely because it is possible to
design.  Anything beyond this box is a typed out-of-coverage error, not a new
terminal.  Section 3.2 records the bounded `N=1` follow-on separately so the
first future numerator caller has an exact plan rather than an extrapolation.

The result is a finite exact linear combination of the same six product keys
already used by the scalar corner boundary:

```text
T1^4, T1^2*S2, S2^2, T1*B4, T1*F5, T1*M6.
```

`B4`, `F5`, and `M6` retain their existing meaning: candidate terminals of the
certified finite three-loop box, not unrestricted minimal masters.

## 2. What the existing witness proves

A [`FourLoopFactorizationWitness`](../../src/four_loop_boundary.rs#L202) stores
two exact changes of variables.

First, with the parent loop momenta collected in `k`, its four selected physical
routings form the rows of a unimodular matrix `B`:

```text
p = B k,                       det(B) = +/-1.
```

Every active physical routing has a stored coordinate row `c_r` satisfying

```text
q_r = c_r B.
```

The supports of the `c_r` split the four `p` slots into disjoint vector-matroid
components.  For component `c`, the witness stores slots `S_c` and a unimodular
map `U_c` such that

```text
c_r|S_c U_c = epsilon_r q_ref(r),   epsilon_r in {+1,-1}.
```

Consequently, if `ell_c` denotes the frozen reference loop momenta for that
component, the combined change of variables is

```text
p_c = U_c ell_c.
```

Choose a declared column order for the concatenated `ell_c` slots and scatter
each `U_c` into its component's recorded `global_basis_slots`; those slots need
not be contiguous.  Calling the resulting four-by-four matrix `U_scatter`,

```text
k = B^-1 U_scatter ell.                                (1)
```

The product of all Jacobians has absolute value one.  The required scalar
closure only needs the signed line bijections already replayed by
`FourLoopBoundaryReducer::replay_witness`.  A later numerator implementation
must use equation (1), not merely the component master labels, and additionally
verify the combined equation

```text
q_r B^-1 U_scatter
    = epsilon_r q_ref(r) embedded in component c       (2)
```

for every active line before constructing a numerator coefficient.  This
combined check fixes the matrix direction and prevents a transposed but
dimensionally plausible numerator map.

The only possible component partitions are

| scalar product | component ranks | recognized components |
|---|---|---|
| `T1^4` | `1+1+1+1` | four tadpoles |
| `T1^2*S2` | `1+1+2` | two tadpoles and one sunset |
| `S2^2` | `2+2` | two sunsets |
| `T1*B4` | `1+3` | tadpole and tetrahedron mask 43 |
| `T1*F5` | `1+3` | tadpole and tetrahedron mask 31 |
| `T1*M6` | `1+3` | tadpole and tetrahedron mask 63 |

This follows from the exhaustive scalar-corner classifier: recognized proper
components have `(rank,lines)` equal to `(1,1)`, `(2,3)`, `(3,4)`, `(3,5)`, or
`(3,6)`.

## 3. Scalar one-dot closure and the later numerator extension

### 3.1 Component scalar inputs

The signed line matches map every positive parent propagator to a compact
reference-line position in exactly one component.  Squaring removes the stored
orientation sign.  Since the parent has `D=1`, exactly one matched component
line owns the dot.

`FourLoopSignedLineMatch::reference_position` is **compact component-line
numbering**, not always the six-entry tetrahedron-family position.  The fixed
convention is:

| component | compact positions | lift into local family |
|---|---|---|
| `T1` | `0` | `0` |
| `S2` | `0,1,2` | `0,1,2` |
| `B4` | `0,1,2,3` | tetrahedron positions `0,1,3,5` |
| `F5` | `0,1,2,3,4` | tetrahedron positions `0,1,2,3,4` |
| `M6` | `0,1,2,3,4,5` | tetrahedron positions `0,1,2,3,4,5` |

The `B4` lift is the only nonidentity lift.  The production formula below is
independent of the `B4` line, so it need not materialize an arity-six power
vector.  Tests that compare it with the three-loop family must nevertheless
apply this lift exactly; directly treating compact positions 2 and 3 as
tetrahedron positions 2 and 3 would test the wrong integrals.

### 3.2 Later `N=1` extension: exact numerator image

This subsection is not required by the observed blocker census and is not part
of the first closure API.  It fixes the proof obligations for a later explicit
`N=1` extension.

If the sole negative parent entry is at denominator-basis position `n`, the
integrand is multiplied by

```text
D_n(k) = Q_n(k) + s_n.
```

Let

```text
T = B^-1 U_scatter.
```

Transform the flattened quadratic row with the existing convention that an
off-diagonal entry is already the coefficient multiplying `k_i.k_j`:

```text
Q'_n(ell) = Q_n(T ell).                                (3)
```

For every scalar product whose two loop slots belong to the same component,
use that component reference family's
`VacuumFamily::scalar_product_expansion` to write

```text
ell_(c,a).ell_(c,b)
  = gamma_(c,ab) + sum_j rho_(c,ab,j) D_(c,j).
```

After collecting, equation (3) becomes

```text
D_n = C + sum_(c,j) A_(c,j) D_(c,j)
          + sum_(c<e,a,b) X_(c,a,e,b)
                              ell_(c,a).ell_(e,b).      (4)
```

The equality in (4) must be replayed as an equality of the constant and all ten
flattened four-loop scalar-product coefficients.  Numerical sampling alone is
not a certificate.

### 3.3 Later `N=1` extension: why every cross-component term is zero

Each scalar component integrand, including a dot, is invariant under the
simultaneous reversal of all loop momenta in that component.  A cross term in
(4) factorizes as

```text
[integral_c ell_(c,a)^mu f_c] [integral_e ell_(e,b)_mu f_e]
    * product_(h != c,e) [integral_h f_h].              (5)
```

Each bracket carrying one vector is an odd rank-one vacuum tensor and is
exactly zero.  This can be certified with the native
`VacuumTensorProjector`, whose odd-rank result is the empty reduction.  It is
important to project the two components separately.  Applying a rank-two
global projector to `ell_c.ell_e` would ignore the factorized parity argument
and would not establish (5).

Thus the cross terms are discarded only after their component ownership is
authenticated.  No rank-two angular formula is required at `N<=1`.  At
`N=2`, products of two cross terms can give even rank in both components and
would require genuine tensor projection; that larger box is explicitly outside
this milestone.

### 3.4 Later `N=1` extension: lowering the surviving affine terms

The surviving part of (4) gives at most one constant branch and one branch for
each local denominator basis entry:

```text
C * product_c I_c(b_c)

+ A_(c,j) * I_c(b_c-e_j) * product_(h!=c) I_h(b_h).     (6)
```

All shifts use checked subtraction.  Every induced component integral remains
inside `D<=1,N<=1`:

- lowering the dotted denominator removes the dot;
- lowering another active denominator pinches that line while retaining at
  most one dot;
- lowering an inactive denominator creates exactly one degree-one numerator;
  and
- the constant branch retains the original component inputs.

The one-, two-, and three-loop reference families are complete scalar-product
bases made only of physical propagators, so (6) cannot invent an unrecognized
local ISP convention.

## 4. Fixed component-dot table

No lower-loop reducer needs to be constructed on the production path.  The
certified finite three-loop reduction and its native raw IBPs give closed
formulae for every component and every possible reference-line position in
this `D1/N0` box.  Production code should dispatch on

```text
(component master, signed_line_match.reference_position)
```

and construct the following exact product-valued answer in the shell's
existing coefficient context.  The finite derivation remains a test oracle;
the table, rather than a serialized or rebuilt reduction pipeline, is the
runtime certificate payload.

### 4.1 One loop

Use the positive-Euclidean tadpole recurrence

```text
T(a)/T(1) = product_(r=1)^(a-1) (2*r-d)/(2*r*m2),
```

so the only dotted input needed here is

```text
T(2) = (2-d)/(2*m2) T1.
```

A zero or negative sole denominator leaves an unconstrained loop integration
and is scaleless.  The bounded service only accepts the displayed `a=2` case,
so it should construct this checked ratio locally.  A later cleanup may share
the general recurrence with other reducers.

### 4.2 Two loops

Every sunset line is symmetry-equivalent, and the only required input has one
dot.  Use the direct exact identity

```text
I2(2,1,1) = (3-d)/(3*m2) S2.                           (9)
```

This avoids constructing a separate two-loop sparse table for a single known
orbit.  Equation (9) must be independently checked against the native two-loop
IBP certificate in tests; it is not an empirical shortcut.

### 4.3 Three loops

Write `B4_i`, `F5_i`, and `M6_i` for the corresponding scalar corner with a
dot on reference position `i`.  The complete required table is

```text
B4_i = (8-3*d)/(8*m2) B4,                              compact i in {0,1,2,3},

F5_0 = (8-3*d)/(6*m2^2) B4
     + 2*(d-2)/(3*m2^2) (T1*S2)
     + (6-d)/(6*m2) F5,

F5_i = (3*d-8)/(24*m2^2) B4
     + (2-d)/(6*m2^2) (T1*S2)
     + (3-d)/(3*m2) F5,                               i in {1,2,3,4},

M6_i = (4-d)/(4*m2) M6,                               i in {0,1,2,3,4,5}.
```

For `B4` and `M6` all compact reference positions lie in one symmetry orbit.
For `F5`, position zero is the central-line orbit and positions one through
four form the outer-line orbit.  This is why moving the parent dot through the
stored signed line match is proof-relevant even though the sign itself drops
out of a squared propagator.

These equalities are identities for the fixed reference-position conventions
in section 3.1.  A reordered tetrahedron family may use the table only after an
exact permutation replay proves which new position belongs to which listed
orbit.  It must not infer the `F5` orbit from a graph drawing or an unordered
component label.

### 4.4 Product multiplication and the six-output theorem

The dotted `T1`, `S2`, `B4`, and `M6` formulae have one term; a dotted `F5`
has exactly three.  Rebuild the unaffected product from every witness component
except the unique component owning the dotted parent line.  For each table
term, use checked `MasterProduct` multiplication with that unaffected product
and collect equal keys.  This avoids adding a potentially ambiguous
`remove_one_factor` operation when products contain repeated factors.

The only multi-term case is a dotted `F5` in a `T1*F5` corner, and its three
outputs are exactly

```text
T1*B4, T1^2*S2, T1*F5.
```

All one-term cases preserve their corner product.  Thus every result is one of
the six products in section 1 and one blocker has at most three contributions.
An unknown product is an error, not a seventh terminal.

## 5. Deterministic reduction algorithm

For one required scalar blocker `(topology, integral, witness)`:

1. Validate arity and the exact `D1/N0` index shape before witness or
   coefficient work.
2. Replay `FourLoopBoundaryReducer::replay_witness`; require its product to
   equal the blocker's stored product.
3. Sort components by their smallest global basis slot and authenticate their
   disjoint cover of slots `0..3` and all positive physical lines.
4. Locate the unique parent exponent two, then use its signed line match to
   select `(component master, compact reference position)`.  Require every
   other active exponent to be one and validate the compact position against
   the table in section 3.1.
5. Dispatch the fixed formula in section 4.  Rebuild the unaffected product
   from the other witness components and multiply it into each formula term.
6. Collect at most three exact contributions and require only the six allowed
   product keys.
7. Verify homogeneity for every output before returning it.

No quadratic transform, scalar-product affine expansion, or tensor projector is
on this required path.  For a later explicitly enabled `N=1` input, append the
steps from sections 3.2--3.4: construct `B^-1 U_scatter`, replay (2) and
(4), prove cross terms zero componentwise, and lower (6).

The service should expose ordinary coefficients first:

```text
I(a) = sum_P r_P(d,m2) P.
```

For a product with weight

```text
w(P) = sum_(master,multiplicity)
         multiplicity * master.physical_lines(),
```

dimensional homogeneity requires

```text
r_P * (m2)^(sum_i a_i - w(P)) in Q(d).                 (7)
```

The corner shell already stores blocker coefficients normalized using the
integral weight `sum_i a_i`.  It can therefore merge a stored normalized
blocker coefficient `b` into product `P` using

```text
b * [r_P * (m2)^(sum_i a_i-w(P))],                     (8)
```

and must reject residual `m2` dependence in the bracket.  Equation (8) avoids
reconstructing pre-normalized raw rows and makes the intended
`UnsupportedBoundaryHalo` replacement interface precise.

## 6. Finite structural and resource bounds

### 6.1 Required scalar closure

One scalar blocker has exactly one formula dispatch; every other component is
its already authenticated scalar corner.  There is no affine branching and no
lower-loop construction.  A `T1`, `S2`, `B4`, or `M6` dispatch has one term;
an `F5` dispatch has three.  Thus one blocker needs at most three checked
product multiplications and three pre-collection contributions, with six
possible distinct output products globally.

For the observed batch, preflight all 234 occurrences.  Group them by exact
`(topology, sector, product, witness)` equality and replay each unique witness
once.  The frozen census has 28 such groups and 150 signed-line dispatch
entries across their active sectors.  Each occurrence then performs one
constant-time lookup and one formula dispatch.

There are 93 occurrences whose corner product is `T1*F5`; only those can take
the three-term branch.  The other 141 occurrences have one-term component
formulae regardless of dot placement.  Therefore the census-specific safe
batch ceiling is

```text
93*3 + 141*1 = 420
```

pre-collection terms and checked product multiplications.  The actual count
may be smaller when a `T1*F5` blocker has its dot on `T1`, and must be measured
and frozen when implementation lands.  Construct the six dispatch rows (`T1`,
`S2`, `B4`, central `F5`, outer `F5`, `M6`) once in the shell's coefficient
context; they contain ten coefficient terms in total.  No three-loop pipeline,
seed enumeration, or component-reduction cache belongs to the production cost.

Required configuration fields are:

```text
max_blocker_occurrences       // default exactly 234 or a documented headroom
max_unique_witness_plans      // current exact request 28
max_signed_line_dispatches    // current exact request 150
max_formula_dispatches        // current exact request 234
max_product_multiplications   // census-specific ceiling 420
max_precollection_terms       // census-specific ceiling 420
max_output_products           // structural ceiling 6
max_coefficient_degree
```

The six-row, ten-term formula table is fixed-size code, not a user-sized
allocation.  Its coefficient-degree estimate and context compatibility are
still checked before constructing the first term.  The aggregate limits above
must be checked before replaying the first witness or building that table.

### 6.2 Bounds for the later `N=1` extension

The component ranks partition four.  A component of rank `r` has
`r(r+1)/2` scalar-product basis entries.  Hence the number of intra-component
entries in (4) is

| ranks | intra entries | cross entries | scalar affine branches |
|---|---:|---:|---:|
| `1+1+1+1` | 4 | 6 | at most 5 |
| `2+1+1` | 5 | 5 | at most 6 |
| `2+2` | 6 | 4 | at most 7 |
| `3+1` | 7 | 3 | at most 8 |

Thus one blocker needs at most:

- 4 components;
- 10 transformed scalar-product coefficients;
- 6 unordered cross-component scalar-product terms and, if both rank-one
  factors are projected explicitly, at most 12 component projector calls;
- 8 surviving scalar branches;
- 9 distinct component target reductions (the component bases plus all local
  one-denominator lowerings);
- 5 product terms per branch;
- 40 pre-collection product contributions; and
- 6 distinct output products.

These are structural caps, not observed averages.  Every configurable limit
must be checked before the corresponding allocation or lower-loop call.
Additional configuration fields for that extension are:

```text
max_cached_transforms
max_exact_transform_operations
max_cross_tensor_terms
max_scalar_branches
```

Matrix dimensions are fixed at four.  Charge a documented conservative unit
for the 4-by-4 inverse, block composition, quadratic transform, and exact replay
rather than an unexplained magic counter.  Saturating estimates are acceptable
only when saturation is reported as an over-limit request.

For batch integration, use two phases.  First retain and collect the normalized
blockers exactly as the current shell does.  Compute every closure request and
check aggregate caps before replaying the first witness or constructing formula
coefficients.  Then build authenticated witness plans, dispatch all
occurrences, and merge them with (8).  This preserves deterministic row
collection and prevents a late blocker from crossing a resource limit after
earlier exact arithmetic.

All coefficient multiplication, addition, division, and mass powers must use
the existing per-variable Symbolica exponent estimators before construction.
The fixed `D1/N0` indices themselves are far from `i32` overflow.  Formula
coefficients must use the shell's one `CoefficientContext`; independently
constructed same-named contexts must not be multiplied.

## 7. Proof and regression tests

Acceptance of the implementation requires all of the following.

1. **Frozen blocker census.** Freeze the complete table in section 1 and its 234
   occurrences.  Independently rebuild it from blocked rows before closure.
   This protects the claim that every observed class, rather than a convenient
   sample, is supported.
2. **Domain exhaustion.** Run the new service on every blocker occurrence and
   every unique blocker key.  Assert every input and induced component target
   has `D=1,N=0` or is a scalar corner, and no term is copied through unresolved.
3. **Line-map replay.** Replay every unique witness and check every signed
   parent-line to compact reference-line match used to move the dot.  Include
   nonidentity maps and orientation-flipped parent routings.  Explicitly test
   the `B4` compact-to-tetrahedron lift `0,1,2,3 -> 0,1,3,5`.
4. **Fixed-formula provenance.** Check the tadpole and sunset identities against
   native IBPs.  In tests only, build the certified finite three-loop pipeline,
   replay its native equations/certificate, and compare all 4 `B4`, 5 `F5`,
   and 6 `M6` dotted inputs with the fixed table after applying the compact
   position lift.  Independently authenticate the stabilizer orbits: one orbit
   for `B4`, `{0}` and `{1,2,3,4}` for `F5`, and one orbit for `M6`.
5. **Product multiplication.** Compare cached and uncached witness plans,
   permute component order, exercise repeated factors such as `T1^4` and
   `S2^2`, and require identical collected combinations with no product outside
   the six-key set.
6. **Mass homogeneity.** Verify (7) term by term.  After (8), every normalized
   coefficient must have zero numerator and denominator degree in `m2`.  As an
   independent check, central `F5` plus its four outer-dot formulae must cancel
   the `B4` and `T1*S2` terms and equal
   `(5-3*d/2)/m2 * F5`, the common-mass derivative identity.
7. **Complete corner shell.** Require 160 normalized rows, zero blocked rows,
   zero blocker census entries, exact replay of every original `Q(d,m2)` row,
   and exact replay of every elimination source-row combination.  Only the new
   complete-shell rank/free-column result may be reported.
8. **Resource failures.** Set each scalar-closure and aggregate cap one below
    its request and require a typed error before guarded work.  Cover blocker,
    unique-witness, signed-line, formula-dispatch, product-term, and coefficient
    budgets.
9. **Context and overflow failures.** Reject wrong arity, any nonzero auxiliary
    or numerator power, `D!=1`, a witness for another topology/sector, altered product
    metadata, and incompatible coefficient maps before arithmetic.

The later `N=1` extension has separate acceptance tests: exact numerator replay
of (4), componentwise tensor parity, intra-component lowering, corrupt-map
failures, and transform/cross-tensor/affine-branch resource failures.  Those
tests must not be claimed by the scalar milestone.

No test may use FORM or Mathematica.  The native raw IBP generator, exact
rational line maps, and the existing certified three-loop pipeline provide all
required test oracles; that pipeline is not constructed at runtime.

## 8. Minimal implementation changes

The existing APIs are close but not sufficient by themselves:

1. `FourLoopBoundaryReducer` classifies only scalar corners; a new service must
   consume its signed line matches to transport the one physical dot.
2. No service currently dispatches the six fixed component-dot rows and returns
   `ProductLinearCombination<MassiveVacuumMaster>` in the shell's context.
3. The shell has no aggregate boundary-closure budget and currently excludes
   every row containing a blocker from elimination.

The minimal implementation is therefore:

- add a focused scalar `four_loop_boundary_halo` module with a
  `FourLoopBoundaryHaloReducer`, explicit config/error types,
  authenticated witness-plan cache, and ordinary plus mass-normalized reduction
  methods;
- implement all six fixed dispatch rows in that service, using compact
  `reference_position` and checked product construction; no lower-loop runtime
  reducer or lower-loop public API change is needed;
- extend `FourLoopCornerShellConfig` with the closure limits and refactor shell
  construction into collect/preflight/close/eliminate phases; and
- retain `UnsupportedBoundaryHalo` as provenance input and compatibility data,
  but require the completed default certificate to consume every record before
  elimination.

No change to the factorization-witness schema, raw-row identifiers, global
column keys, or `MasterProduct` representation is required.  No general
four-loop tensor reducer is needed for this bounded milestone.

When an actual `N=1` caller exists, the minimal additional changes are to expose
or duplicate-with-replay the fixed-size quadratic transform, add the exact
product-numerator splitter from section 3, and wire the existing native odd-rank
projector.  Those changes are intentionally not prerequisites for closing the
current blockers.

## 9. Nonclaims and next boundary

Closing these 234 terms completes the lower boundary of the fixed 160 corner
rows.  It does not prove a four-loop master basis, reduce the remaining genuine
four-loop `D1/N1` columns, or establish closure of a 4,736-row `D1/N1` seed box.
Those claims require adaptive genuine-sector seed growth and a new replayable
elimination certificate after the factorized boundary is complete.

The first tensor extension begins at `N=1`; sections 3.2--3.4 give its exact
map and rank-one parity proof.  At `N=2`, cross-component rank-two moments can
produce nonzero products of local scalar moments, so that parity shortcut must
not be extrapolated.
