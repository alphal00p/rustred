# Exact four- and five-loop massive-vacuum reduction plan

## Scope and status

This note fixes an honest scope for RustRed's four- and five-loop milestones.
It is based only on static inspection of the checked-in Rust, Vakint, and
LiteRed2 sources.  No FORM or Mathematica process was run, and no result below
depends on either program at RustRed run time.

There are three different levels of assertion in this note:

- **Proved here / already checked in RustRed** means exact linear algebra,
  graph enumeration, or an identity which follows directly from a change of
  variables or mass homogeneity.
- **Static compatibility oracle** means the claim is a fact about the
  checked-in Vakint/FMFT source.  It is useful for tests but is not an
  independent proof that a RustRed reduction is complete.
- **Candidate / implementation target** means a proposed master or coverage
  domain which still needs a reduction certificate.

The short conclusion is:

1. The four current RustRed routings are correct and complete for **Vakint's
   current four-loop registry**.  Their scalar corners now have an exhaustive
   zero/factorization reducer with replayable unimodular witnesses. The 214
   genuine rank-four presentations additionally collapse to ten exact
   signed-`GL(4,Z)` interfamily routing types with replayable maps. Complete
   affine physical/ISP basis maps transport their direct raw-corner halos, and
   the factorized scalar `D1/N0` terms now close in a replayable mass-normalized
   160-row shell.  The selected 123-seed next shell now also has an exact
   1,968-origin manifest, degree-two affine-polynomial transport, and a
   replayed depth-two preclosure inventory. Its 1,066 witness-complete keys and
   all 4,230 occurrence references now have exact component transport: 577 N0
   and 489 N1 plans across the six factorized products. Exact caller-context
   lower-loop composition and mass normalization have landed for all 243
   T1/S2-only plans and 1,134 occurrences.  The complementary exact sibling
   has also landed for 823 `T1*B4`/`T1*F5`/`T1*M6` plans and 3,096 occurrences,
   with 204 distinct local targets.  It uses one three-loop
   `(D,N)<=(2,1)` pipeline in the parent coefficient context, not separate
   production dispatchers per three-loop label. `FourLoopNextClosedRows` now
   composes both slices and assembles and canonically scales all 1,968 parent
   rows over `Q(d)` with independent source-backed replay. Exact rank,
   source-row weights, exceptional factors, and elimination remain pending,
   so no complete next-shell reduction is claimed.
2. Vakint provides a strong four-loop compatibility oracle: its static FMFT
   workflow reduces H and X first, maps proper sectors through BMW and FG, and
   ends in a 19-symbol terminal alphabet after eliminating `PR9x`.  RustRed
   should use the names and frozen examples as an oracle, not port the ordered
   FORM control flow.
3. The current five-loop family is only the six-line banana.  It has an exact,
   unusually simple boundary: every proper physical sector is either zero or
   a product of five one-loop tadpoles.  RustRed now certifies its complete
   3,232-target `(D,N) <= (1,1)` box, including native numerator
   factorization and all 25 raw corner IBPs.  A separate exact scalar `D=2`
   certificate eliminates the triple-dot orbit but retains the double-double
   orbit as an explicit candidate terminal. A deeper replayable exact scalar
   shell now closes all three `D=3` orbits onto that candidate and the corner;
   shells beyond `D=3` are not yet closed in production.
4. A general five-loop claim needs an explicit family catalog.  Even the
   simple cubic catalog exposes a missing core feature: one graph has two
   distinct equal-mass edges with the same denominator, so an overcomplete
   denominator-set/partial-fraction layer is required.
5. Production four-/five-loop solving should be target-driven and modular in
   the single remaining symbolic variable `d`, with exact replay as the final
   proof.  Eager exact elimination over every sector is not a viable design.

## 1. Common convention and complexity

RustRed uses the Euclidean denominator convention

\[
 D_r=q_r^2+m^2,
 \qquad q_r=\sum_{i=1}^{L} C_{ri}k_i,
 \]

and stores an integral as

\[
 I(a_1,\ldots,a_K)=
 \int\prod_{i=1}^{L}d^d k_i\prod_{r=1}^{K}D_r^{-a_r}.
\]

There are

\[
 K=L(L+1)/2
\]

independent vacuum scalar products and `L^2` ordinary momentum-space IBPs per
seed.  Thus four loops require 10 basis entries and 16 identities per seed;
five loops require 15 entries and 25 identities per seed.

Auxiliary entries complete the scalar-product basis.  They are not physical
propagators and must be fixed nonpositive in physical sectors.  Calling every
completed entry a propagator would incorrectly create fictitious sectors.

For a finite target domain use

\[
 D(a)=\sum_{r\in\mathrm{physical}}\max(a_r-1,0),\qquad
 N(a)=\sum_r\max(-a_r,0).
\]

The milestone must name the requested `(D,N)` box.  Solver seeds and the IBP
dependency halo are separate data: an IBP seed in `(D,N)` can emit a column in
`(D+1,N+1)`, so target bounds are not valid seed bounds by themselves.

For any equal-mass physical family, dimensional homogeneity gives the exact
identity

\[
 \sum_{r\in\mathrm{physical}} a_r I(a+e_r)
 =\frac{\sum_r a_r-Ld/2}{m^2}I(a).
\tag{1}
\]

Equation (1) reduces the sum of dots.  It reduces an individual dot only when
the stabilizer of the input makes all relevant dotted integrals equal; it is
not a substitute for IBP in a generic sector.

## 2. Four-loop family audit

### 2.1 Exact routing comparison

Vakint registers four parents in
[`topologies.rs`](../../vendor/gammaloop/crates/vakint/src/topologies.rs):

| parent | Vakint physical momenta | physical rank | deterministic RustRed auxiliary basis |
|---|---|---:|---|
| H | `k1,k2,k3,k4,k1-k3,k2-k3,k3-k1+k4,k3-k2+k4,k3+k4` | 9 | `k1.k2` |
| X | `k1,k2,k3,k4,k1-k3,k2-k3,k3-k1+k4,k3-k2+k4,k3-k1-k2+k4` | 9 | `k1.k2` |
| BMW | `k1,k2,k3,k4,k1-k2,k3-k4,k2+k3-k1,k3-k4-k1` | 8 | `k1.k3`, `k2.k4` |
| FG | `k1,k2,k3,k1-k3,k4,k2-k3,k1-k3+k4,k1-k2` | 8 | `k1.k4`, `k2.k4` |

The rows in [`src/four_loop.rs`](../../src/four_loop.rs) agree entry by entry.
Its H/X rows 7 and 8 and BMW row 8 use the opposite momentum orientation from
the display in Vakint, but `q^2=(-q)^2`, so the denominators are identical.

Exact rational row reduction gives ranks `9,9,8,8`.  Scanning the standard
upper-triangular scalar products in the order

```text
k1.k1, k1.k2, k1.k3, k1.k4,
k2.k2, k2.k3, k2.k4, k3.k3, k3.k4, k4.k4
```

proves the auxiliary choices in the table.  The checked-in family tests also
reconstruct every denominator from the inverse map and generate all 16 raw
IBPs.  These facts establish a complete scalar-product coordinate system, not
a reduction.

### 2.2 Physical sectors and proved graph symmetries

All physical denominators carry the common nonzero mass.  For a scalar corner
with no positive auxiliary power, a physical sector is therefore scaleless if
the active momentum rows have rank below four.  The exact rank census is:

| parent | labelled sectors | rank-deficient/zero | full-rank | physical graph automorphisms | full-rank orbits under parent automorphisms |
|---|---:|---:|---:|---:|---:|
| H | 512 | 198 | 314 | 12 | 42 |
| X | 512 | 184 | 328 | 72 | 15 |
| BMW | 256 | 122 | 134 | 8 | 26 |
| FG | 256 | 132 | 124 | 4 | 51 |

The automorphism counts were obtained by exhaustive vertex permutations of
Vakint's static edge lists.  Every retained edge permutation was independently
lifted to an integer `4 x 4` loop-momentum map and checked to have determinant
`+1` or `-1` and to reproduce every physical momentum row up to orientation.
This is a proof of the physical-line actions.

Small generator sets exist with sizes 3, 4, 3, and 2 respectively for H, X,
BMW, and FG.  They should be generated from the graph, not copied as unexplained
permutations.  For every generator RustRed should store:

```text
unimodular loop map
physical-line permutation and orientations
affine image of every completed denominator/ISP
inverse map and a proof hash
```

The present constructors intentionally register only the identity.  This is
safe but not production-ready.  A physical graph automorphism usually maps a
generated auxiliary to an affine linear combination of physical denominators
and auxiliaries, so the existing permutation-only symmetry API cannot express
the full action.  Scalar physical corners can use the physical permutation
immediately; numerator monomials require expanding the ISP image with a strict
term budget.

Graph equivalences between contractions can be stronger than automorphisms of
the parent.  Canonical contracted-graph fingerprints and explicit inter-family
loop maps are therefore needed in addition to the parent stabilizer.

### 2.3 What constitutes the four-loop family scope

For Vakint compatibility the required named parents are exactly H, X, BMW,
and FG, including arbitrary physical pinches representable inside each
completed family.  The current Vakint registry exposes only the unpinched H,
X, and BMW parents, while it asks Symbolica graph canonization to enumerate FG
contractions.  RustRed should not preserve that backend accident: every
physical sector is a valid typed request, subject to scaleless and
factorization checks.

Vakint's static four-loop orchestration is informative:

1. map reducible X sectors to BMW and FG;
2. reduce the remaining X top sector;
3. map reducible H sectors to BMW and FG;
4. reduce the remaining H top sector;
5. reduce BMW and map its proper sectors to FG;
6. reduce FG and its factorized lower-loop boundaries.

That hierarchy appears in the static `fmft` procedure.  It supports a native
sector DAG of `lower-loop products -> FG -> BMW -> H/X`, but RustRed must derive
each edge as an exact loop-momentum map rather than route expressions through
the legacy procedure.

## 3. Four-loop factorization and master candidates

### 3.1 Exact boundary service

Before inserting any sector into a four-loop matrix, classify it as follows:

1. apply the conservative rank/scaleless proof;
2. build the active mass-labelled graph and its connected/block decomposition;
3. find an exact unimodular loop transformation which block-diagonalizes its
   momentum forms;
4. map every block into a certified one-, two-, or three-loop family;
5. decompose scalar numerators into local and cross-block coordinates, then
   prove every cross term with two separately owned component rank-one
   projections before multiplying component reductions;
6. return a sorted commutative `MasterProduct` plus the transformation proof.

For a one-loop factor use

\[
 T_{n+1}=\frac{2n-d}{2n\,m^2}T_n.
\tag{2}
\]

This boundary service is part of the solver, not post-processing.  Treating a
factorized four-loop sector as an unrelated free column produces a false
master count and prevents its numerator halo from closing.

The **smallest honestly certifiable four-loop slice** is therefore:

- all scalar physical corners `(D,N)=(0,0)` in H/X/BMW/FG which are proved
  zero or factorize completely into the already certified lower-loop basis;
- every symmetry- and inter-family-equivalent presentation of those corners;
- arbitrary positive tadpole dots on completely factorized components via
  (2); and
- exact replay of every factorization map and every compatible generated IBP.

This slice is substantive, terminates in known lower-loop objects, and makes
no unproved assertion about a genuine four-loop corner.

### 3.2 Static FMFT terminal oracle

The checked-in FMFT source declares `PR0`, `PR1` through `PR15`, `PR4d`,
`PR9d`, `PR9x`, and `PR11d`.  It also gives an explicit linear relation which
eliminates `PR9x`.  Its resulting terminal alphabet therefore contains 19
named objects:

```text
PR0, PR1, ..., PR15, PR4d, PR9d, PR11d
```

This is a **static compatibility basis**, not a proof that RustRed's generic
quotient has dimension 19 or that this basis is minimal.  Several entries are
factorized/lower-topology objects, and the names encode the legacy convention.
RustRed should give each terminal a canonical family/exponent fingerprint and
keep the `PR...` name as an adapter alias.

Three top-sector anchors are explicit in the source:

| static terminal | legacy corner |
|---|---|
| `PR0` | X top corner in FMFT slots `d2..d10` |
| `PR12` | H top corner in slots `d1..d9` |
| `PR11` | BMW top corner in slots `d3..d10` |
| `PR11d` | the same BMW sector with the `d3` line dotted |

These are strong expected-master anchors.  They must not be installed as
masters merely because the old source stops there.  A RustRed master is a free
column only after a closed target/seed system, rank stability, and exact
certification.

The four-loop numerical fixtures in
[`integral_evaluation_analytic_tests.rs`](../../vendor/gammaloop/crates/vakint/tests/integral_evaluation_analytic_tests.rs)
cover H, equivalent PR9d embeddings through H/X/FG and pinches, PR11d/BMW,
the four-tadpole clover, a dotted clover, and tensor/scalar numerators.  Those
epsilon series are excellent end-to-end regression values, but they combine
reduction, normalization, and master evaluation.  RustRed additionally needs
frozen pre-evaluation master-coefficient vectors.

## 4. Five-loop scope

### 4.1 Audit of the current banana

Vakint has no five-loop topology type, registry entry, backend, or fixture.
The only five-loop RustRed constructor is therefore a RustRed-owned family,
not a port of a Vakint family.

[`src/five_loop.rs`](../../src/five_loop.rs) defines

```text
q1=k1, q2=k2, q3=k3, q4=k4, q5=k5,
q6=k1+k2+k3+k4+k5.
```

The six quadratic rows are independent.  The deterministic basis completion
adds the nine ISPs

```text
k1.k2, k1.k3, k1.k4, k1.k5,
k2.k3, k2.k4, k2.k5, k3.k4, k3.k5.
```

The family test reconstructs all 15 basis entries and generates all 25 raw
IBPs.  The generic `VacuumFamily` registry still contains only the identity,
because its present symmetry type can only permute denominator positions.  The
banana boundary reducer now exposes and exhaustively verifies the full `S6`
physical action instead: adjacent physical-line transpositions generate it,
and every action carries its determinant-`+/-1` loop map.  Transpositions among
`q1..q5` are loop-basis permutations; swapping one of those lines with `q6` is
also unimodular but maps the nine ISPs affinely.  Numerator-sector witnesses
therefore transform the complete quadratic form rather than pretending that
the ISP entries are permuted.

### 4.2 Exact banana boundary theorem

Let

\[
 B_5(a_1,\ldots,a_6)=
 \int\frac{d^dk_1\cdots d^dk_5}
 {(k_1^2+m^2)^{a_1}\cdots(k_5^2+m^2)^{a_5}
  ((k_1+\cdots+k_5)^2+m^2)^{a_6}}.
\]

For scalar physical sectors, with auxiliary entries nonpositive:

- zero through four active physical lines has momentum rank below five and is
  scaleless;
- every five-line sector is transformed by a determinant-`+/-1` integer loop
  map into five independent massive tadpoles; and
- the all-six-line sector is the only genuine connected five-loop sector.

Thus, for positive active powers and no numerator coupling,

\[
 B_5(a_1,\ldots,a_5,0)=\prod_{i=1}^{5}T_{a_i},
\]

and every other five-line sector is its `S6` image.  Equations (2) reduce
arbitrary positive dots on that boundary exactly.

Define the candidate top master

\[
 M_{B5}=B_5(1,1,1,1,1,1).
\]

Applying (1) at the symmetric corner proves

\[
 B_5(2,1,1,1,1,1)
 =\frac{12-5d}{12m^2}M_{B5},
\tag{3}
\]

and `S6` gives the other five one-dot positions.  Equations (2) and (3), plus
the rank-zero rules, form a complete exact certificate for all physical
pinches and the top one-dot orbit.

The implemented certificate now goes further and closes every labelled target
with total dot and numerator degrees `(D,N) <= (1,1)`.  At the top it adds the
undotted numerator class `C=(m2*M_B5-P)/5`, the incident dotted-numerator class
`X=-d*M_B5/12`, and the nonincident class
`Y=(3-d)*M_B5/12+(d-2)*P/(8*m2)`.  At a five-line boundary, an explicit
unimodular map transforms the numerator quadratic form into five independent
tadpole variables; mixed products vanish by parity and diagonal insertions are
reduced without division.  The standalone derivation and the obstruction at
`D=2` are recorded in
[`five_loop_banana_dn11_plan.md`](five_loop_banana_dn11_plan.md).

> **Candidate, not proved:** the whole positive-index banana sector has only
> the single master `M_B5`, and arbitrary dots/numerators admit guarded
> recurrences to it.  The boundary theorem and one-dot recurrence do not prove
> that all deeper shells close or that no additional dotted master appears.

This yields the currently implemented finite five-loop box:

```text
family: six-line equal-mass banana
targets: all 3,232 labelled physical-subsector targets with D <= 1 and N <= 1
terminals: product(T1,T1,T1,T1,T1), M_B5
proofs: rank/unimodular tensor factorization, tadpole recurrence, mass
        homogeneity, S6 maps, quadratic-witness replay, and all 25 generated
        top-corner IBPs
```

### 4.3 A broader five-loop catalog is a separate milestone

A connected cubic vacuum graph at five loops has eight vertices and twelve
edges, hence typically three ISPs.  Exhaustive degree completion and graph
isomorphism give exactly five connected **simple** cubic graph shapes on eight
vertices.  This combinatorial result should be frozen in a small pure-Rust
generator test before the shapes become public API.

Four of those five routing matrices have quadratic rank 12 and complete with
three ISPs.  The fifth contains a two-edge cut whose two equal-mass edge
momenta become identical in a loop basis.  Its twelve labelled lines give only
eleven distinct quadratic denominators; after merging the duplicate powers it
needs four ISPs.  This proves that the present
`new_with_standard_auxiliaries` contract, which rejects dependent physical
rows, is insufficient for a general five-loop catalog.

RustRed consequently needs one of these typed inputs:

1. an overcomplete denominator set plus exact affine relations and
   partial-fraction/basis maps; or
2. a graph family which merges exactly identical denominators while retaining
   an explicit map from labelled edge powers to the combined power.

General affine dependence needs the former.  LiteRed2's static `NewDsSet`,
`Relations`, `NewDsBases`, `GeneratePFGB`, and `PFReduce` code is the relevant
algorithmic reference: it separates an overcomplete set from the independent
bases which cover it.  RustRed should implement this with Symbolica polynomial
and Groebner primitives in Rust; it must not serialize work to Mathematica.

The simple cubic set is still not synonymous with “all five-loop massive
vacuum bubbles.”  Multigraphs, different allowed vertex valences, or a
theory-specific graph catalog can add parents.  A complete claim must publish:

- the graph-generation restrictions;
- the canonical parent list and mass assignments;
- all 15-dimensional bases or overcomplete-set maps;
- the covered `(D,N)` region; and
- the certified master/product basis.

Until then the public wording should be “five-loop six-line banana reduction,”
not “complete five-loop reduction.”

## 5. Native high-loop reduction algorithm

### 5.1 Graph and family preprocessing

For every parent:

1. validate graph cyclomatic number against the rank of loop momenta;
2. canonize the mass-labelled graph and derive a deterministic loop basis;
3. form exact quadratic rows and distinguish physical lines from ISPs;
4. detect duplicate or general affine-dependent denominators;
5. enumerate physical sectors with a packed mask and exclude ISP bits;
6. prove zero sectors conservatively;
7. derive parent automorphisms, contraction equivalences, and their exact loop
   maps; and
8. fingerprint the graph, routing, mass labels, convention, ISP basis,
   symmetry maps, and order.

Family names and legacy FMFT edge numbers are not cache identities.

### 5.2 Bottom-up, target-driven sectors

Build a DAG whose nodes are canonical physical sectors.  Each node records one
of:

```text
zero proof
factorization proof into solved components
mapping into an already solved family/sector
genuine sector requiring elimination
```

For a genuine sector, request only targets reachable from the user's finite
box.  Generate one seed's 16 or 25 IBPs at a time, canonicalize their columns,
and stream them into a sector-local sparse reducer.  Grow seed shells
adaptively until:

- every requested target is a pivot or a declared terminal;
- every reachable RHS lies in a solved lower node or the current closed table;
- the pivot/free-column pattern is stable under another shell; and
- held-out target rows reduce to zero.

Resource exhaustion must return a typed partial/incomplete result.  It must
never silently promote an unresolved integral to a master.

### 5.3 Modular solve and exact reconstruction

At equal mass, first factor the exact mass dimension and set `m2=1` for rank
discovery.  The nontrivial coefficients are then rational functions of `d`.
Use this pipeline:

1. sample a deterministic sequence of 64-bit primes and nonsingular `d`
   values;
2. discover sparse pivot skeletons and reachable dependencies over `Zp64`;
3. reject samples which hit a denominator or change the generic rank;
4. require the skeleton to agree across independent primes;
5. reconstruct each univariate rational function in `d` adaptively;
6. validate at unused primes and dimension samples; and
7. replay every reconstructed rule over exact Symbolica rational polynomials.

The modular stage is an accelerator and rank oracle, not the proof.  The exact
replay, symmetry comparisons, mass homogeneity, and row provenance are the
proof.

### 5.4 When to derive symbolic recurrences

Finite tables should come first.  Once several shells choose the same pivot
shape, interpolate an index-dependent recurrence and verify it symbolically
against generic IBPs.  A guarded recurrence must record:

- active indices `>=1` and inactive/ISP indices `<=0`;
- every nonzero pivot-factor condition;
- a strict descent proof in the documented integral order; and
- explicit exceptional branches at zeros of an index/dimension factor.

LiteRed2's `SolvejSector` follows this broad “nearby points, patternize,
guard, split exceptional cases” strategy.  Its optional parametric syzygy IBPs
can reduce positive-sector matrix size, but its checked-in implementation
explicitly rejects numerator sectors.  Syzygies are an optimization after the
ordinary momentum-space solver is correct, not a prerequisite for the first
four-loop certificate.

## 6. Tensor reduction without FORM

Tensor processing remains outside the scalar solver and outside FORM:

1. contract existing metrics and canonicalize free/dummy indices;
2. apply the global `O(d)` vacuum projector to all internal vector factors;
3. express the resulting scalar products in the completed denominator/ISP
   basis;
4. lower denominator factors to exponent shifts;
5. reduce every scalar integral through the same certified tables; and
6. collect metric tensors and `MasterProduct` coefficients in Symbolica.

Odd internal rank is zero.  At rank `2n`, use perfect-matching orbits and the
contraction Gram matrix rather than a hard-coded table.  The naive matching
count is factorial, so rank-10 compatibility requires quotienting by repeated
loop labels, sparse solves, caching by multiplicity pattern, and explicit
resource limits.

When a scalar sector factorizes but its numerator contains products between
blocks, do not simply multiply scalar integrals.  First decompose the coupled
numerator with the native tensor projector for the block rotational groups;
then lower and reduce each block.  This is the high-loop generalization of
tensor factorization already needed at the three-loop boundary.

## 7. Independent validation oracles

Every milestone should contain all applicable layers below.

### 7.1 Structural oracles

- Freeze the four Vakint routing lists and edge-order adapters from
  `topologies.rs` and `fmft.rs` as data-only tests.
- Reconstruct every physical and auxiliary quadratic form from the inverse
  scalar-product map.
- Prove every automorphism and inter-family map by transforming all momentum
  rows and checking determinant `+1/-1`.
- Exhaustively census physical corner masks at four loops and all 3,232
  labelled `(D,N) <= (1,1)` banana targets at five loops.
- For overcomplete five-loop sets, verify every affine relation and every
  basis round trip exactly.

### 7.2 Algebraic certificates

- Verify each raw IBP before elimination.
- Store row provenance for every pivot and replay the whole generating row,
  not only the target coefficient.
- Substitute final rules into generating and held-out IBPs and obtain exact
  zero.
- Check symmetry-related targets, mass homogeneity, and denominator-sign
  parity independently.
- For reconstructed rules, use unused primes plus final exact symbolic replay.
- Check that every terminal belongs to the declared stable master/product
  list; an arbitrary unresolved free column is an error.

### 7.3 Frozen external references

- Record the pre-master-substitution coefficient vectors for the four-loop H,
  PR9d equivalence, BMW/PR11d, and clover fixtures.  They may be transcribed
  once from a trusted independent calculation, but tests must only read frozen
  data and must never launch FORM or Mathematica.
- Retain Vakint's checked-in epsilon-series values as end-to-end evaluation
  goldens.  Clearly separate normalization/master-value failures from IBP
  reduction failures.
- Vakint has no five-loop oracle.  For any five-loop shell beyond the exact
  banana boundary, require either a frozen independent reduction table or two
  independently implemented rank/certificate paths.  Agreement between two
  presentations using the same solver is not independent evidence.

### 7.4 Cache determinism

One resumable unit should be a canonical `(family,sector,target,seed-shell)`
job.  Include family/order fingerprints, primes and samples, pivot skeleton,
row-origin hashes, reconstruction state, exact certificate state, resource
limits, and checksums.  Write a complete temporary record and atomically
rename it.  Merges occur only in canonical sector order.

## 8. Concrete milestone sequence

### 8.1 Immediate exact boundary milestone

Implement before a large modular solver:

1. a proved graph/loop-map representation with affine ISP images;
2. four-loop automatic zero/factorization reduction for all scalar physical
   corners of H/X/BMW/FG;
3. canonical lower-loop `MasterProduct` output and arbitrary positive tadpole
   dots;
4. full `S6` physical symmetry for the five-loop banana;
5. complete banana pinch reduction plus the top corner and one-dot recurrence
   (3); and
6. exact identity, symmetry, factorization, and resource-limit tests.

This exact boundary milestone is implemented. It is a structural base for the
still-missing complete genuine-sector four-loop table beyond the fixed exact
160-row corner certificate, and for deeper five-loop shells.

### 8.2 Certified finite four-loop scalar box

After the modular backend exists, declare a first production box such as

```text
parents: H, X, BMW, FG
targets: every nonzero physical corner plus requested one-dot/one-ISP targets
initial public bound: (D,N)=(1,1)
seeds: adaptive sector-local shells, not equal to the target bound
terminals: stable canonical fingerprints with explicit PR aliases
```

Increase shells until the master set and pivot skeleton stabilize, then freeze
the exact table and certificates.  Only after this succeeds should the API
claim complete reduction for that explicit finite box.

### 8.3 Five-loop banana completion

The analytic `(D,N) <= (1,1)` box is established and exhaustively tested. The
exact `D=2` certificate retains `B2` honestly after the 25 one-dot-seed rows
have rank two in `{A2,B2,R}`. The next complete scalar shell uses the four
`S6` seed orbits through `D=2`: 100 authenticated native rows, 26 explicit
diagonal/momentum rows, and six certified five-line boundary equations form a
43-column exact system of rank 40. It reduces all three `D=3` orbits to the
corner and `B2`, while still naming `B2` only a candidate terminal. Derive
guarded all-positive-index recurrences only after further finite shells agree.
Publish the largest certified box and do not extrapolate beyond it.

### 8.4 General five-loop families

Finally:

1. freeze the selected graph catalog;
2. add overcomplete denominator-set support;
3. derive every 15-dimensional basis and symmetry/sector DAG;
4. run modular rank discovery before exact reconstruction;
5. reconstruct only rules reachable from requested targets; and
6. publish per-family bounds, masters, products, and certificate hashes.

## 9. Audit verdict on current sources

### 9.1 Implemented corner boundary and banana box (2026-08-12)

RustRed now implements the exact four-loop scalar-corner boundary described in
section 3.1.  Every labelled physical corner of the four checked-in parents is
classified by exact routing rank and vector-matroid components.  Every emitted
product carries a global determinant-`±1` loop map plus, for each proper
component, a determinant-`±1` map and signed line bijection onto a frozen
lower-loop routing. Independent propagator orientation changes `q -> -q` are
explicitly quotiented and tested. The separate `four_loop_genuine` classifier
maps every connected rank-four presentation to one of ten frozen H/X types,
again with a determinant-`+/-1` loop map, authenticated ordered bases, and a
signed bijection of every active line. Both witness layers replay without
rerunning canonical search.

| parent | scaleless | factorized | genuine rank four |
|---|---:|---:|---:|
| H | 198 | 240 | 74 |
| X | 184 | 231 | 97 |
| BMW | 122 | 107 | 27 |
| FG | 132 | 108 | 16 |
| total | 636 | 686 | 214 |

The 686 products split over `T1^4`, `T1^2*S2`, `S2^2`, `T1*B4`, `T1*F5`,
and `T1*M6`.  This closes `(D,N)=(0,0)` factorized boundaries; it does not
replace the still-required complete genuine-sector four-loop IBP table beyond
the fixed exact 160-row corner shell.

The ten genuine types contain one five-line, two six-line, three seven-line,
two eight-line, and two nine-line routings. Their labelled multiplicities over
all four parents are respectively

```text
21, 78, 13, 4, 44, 32, 7, 13, 1, 1.
```

All 214 witnesses, independent `q -> -q` presentations, corrupt ordered-basis
metadata, and the complete multiplicity matrix are exhaustively tested. These
ten objects are bounded candidate terminals, not an asserted minimal master
basis.

| item | verdict |
|---|---|
| four-loop physical routings | correct relative to Vakint, including harmless orientation flips |
| four-loop scalar-product completeness | correct: 1 ISP for H/X, 2 for BMW/FG |
| four-loop raw IBP foundation | present: 16 nonzero raw identities at each tested parent corner; all ten frozen representatives now have replayable complete-basis affine maps and every integral in their 160 raw corner rows transports into the reference D1/N1 halo |
| four-loop symmetries | `VacuumFamily` still stores identity only; the scalar-corner layer has a complete ten-type signed-`GL(4,Z)` interfamily quotient |
| four-loop reduction/master claim | the fixed 160-row corner IBP certificate exists; complete next-shell reduction and master minimality remain absent |
| four-loop selected next shell | exact 123-seed/1,968-origin manifest, degree-two transport, bounded depth-two preclosure inventory, exact 1,066-plan/4,230-occurrence component transport, exact closures of the 243 T1/S2 and 823 T1-times-three-loop plan slices, and all 1,968 source-backed canonical parent rows over `Q(d)`; exact elimination, source weights, generic rank, and any reduction or master claim remain pending |
| five-loop banana routing | correct six-line banana, not derived from Vakint |
| five-loop scalar-product completeness | correct: 6 physical rows plus 9 ISPs |
| five-loop raw IBP foundation | all 25 nonzero raw corner identities reduce individually to exact zero |
| five-loop banana symmetries | complete physical `S6` proof surface with determinant-`+/-1` loop maps; ISP numerators use exact quadratic images |
| five-loop finite reduction | all 3,232 labelled `(D,N) <= (1,1)` targets reduce to `M_B5`, `T1^5`, or zero; a separate exact scalar `D<=3` shell replays 100 native origins and reduces its three `D=3` orbits to `M_B5` and the explicit `B2` candidate |
| general five-loop topology scope | absent; constructor name and documentation are honestly banana-specific |
| general five-loop family representation | insufficient for dependent/overcomplete physical denominator sets |

The current modules and the implemented corner boundary are sound structural
foundations. The selected next-shell origin and transport layers now feed an
exact bounded preclosure inventory: 26,078 compact paths end in 2,794 leaves,
with 4,230 raw occurrences across 1,066 witness-complete boundary keys.
Authenticated component transport has landed for every key and occurrence,
including local affine lowering and cross parity. Exact T1/S2 product closure
has additionally landed for 243 plans with checksum
`fnv1a64:a2b92a62c988d2cb`; its parent status deliberately remains partial.
The complementary `FourLoopThreeLoopClosure` has now landed for the other 823
transported plans.  Their exact product split is
`T1*B4:223`, `T1*F5:494`, and `T1*M6:106`, with 443 N0 and 380 N1 plans.
They contain 1,646 components, 5,761 local slots, 1,884 scalar branches, and
3,768 component calls.  The exact target cache is
`T1:4`, `B4:41`, `F5:89`, and `M6:70`; one authenticated caller-context
three-loop `D2/N1` finite pipeline covers every three-loop target.  Its five
integral terminals are mapped semantically to `T1^3`, `T1*S2`, B4, F5,
and M6, and mixed outputs are retained.  The dedicated B4 D2 and F5 D2/N1
services are replay oracles rather than production coefficient domains.  The
service validates 1,800 native target identities and retains 502 output terms.
The frozen target-manifest, service, and closure checksums are respectively
`fnv1a64:9bb3c1a6d4ea7bdd`, `fnv1a64:6a1b52ddb449d5bb`, and
`fnv1a64:da3c250b95b10976`.  The closure performs 7,356 convolution pairs and
retains 2,159 collected terms.  It reports 3,096
completed and 1,134 outside
occurrences, with 969 completed-row incidences, 511 outside-row incidences,
and 191 mixed rows.

Those row-incidence counts describe the three-loop-component slice, not the
aggregate equations. The landed `FourLoopNextClosedRows` layer now combines
the two exact partitions, binds all 4,230 boundary occurrences, and
mass-normalizes and canonically scales all 1,968 parent rows over `Q(d)`. Its
source structure is 26,078 paths (4,230 boundary and 21,848 genuine), 1,066
plans, 4,202 raw boundary groups (4,194 nonzero and eight canceled), and
20,111 genuine row groups. The final matrix uses 1,734 columns (1,728 genuine
plus six products), has 22,424 entries, no zero rows, and maximum width 45.

The grouped production route performs 28,096 contributions and agrees before
row scaling with an independent 30,353-contribution raw-path route. Assembly
uses 26,850 mass-power steps and 32,647/13,502/33,574 coefficient
multiplications/additions/divisions, retaining 71,270 terms in 107,123 bytes.
Every final coefficient is literally `m2`-free. The frozen deterministic
checksum is `fnv1a64:a55ce4ffda6f8f5c`. This exact source-backed route is
pure Rust with Symbolica coefficient arithmetic and does not use FORM.

The aggregate status is narrowly
`ExactFixedSeedParentRowsGenericQdEliminationPending`. It infers no
next-shell rank. The historical modular rank 1,762 was measured in the
2,644-column opaque-boundary probe and cannot be the rank of this 1,734-column
closed matrix. Exact elimination, source weights, and exceptional factors are
still absent. The
next five-loop implementation begins beyond the exact scalar `D=3` shell and
needs a larger adaptive seed/numerator halo; neither step should extrapolate
into an unrestricted completeness claim.

## 10. Landed exact four-loop corner certificate

This section specifies the now-landed implementation after the affine halo
mapper.  It is a finite sparse-row certificate, not a claim that the ten
genuine corners are a minimal or unrestricted master basis.

### 10.1 Frozen global reference-column convention

Use the ten entries of `FourLoopGenuineCornerType::ALL` as a coordinate atlas,
with the following basis order fixed by the family constructors:

| type | reference family | active physical mask | lines |
|---|---|---:|---:|
| `V5` | H | `0x06b` | 5 |
| `V6a` | H | `0x06f` | 6 |
| `V6b` | H | `0x0cf` | 6 |
| `V7a` | H | `0x13f` | 7 |
| `V7b` | H | `0x07f` | 7 |
| `V7c` | H | `0x0df` | 7 |
| `V8a` | H | `0x17f` | 8 |
| `V8b` | H | `0x0ff` | 8 |
| `H9` | H | `0x1ff` | 9 |
| `X9` | X | `0x1ff` | 9 |

H and X positions `0..8` are the physical routings in `src/four_loop.rs`;
position 9 is the deterministic generated auxiliary `k1.k2`.  A genuine
matrix column is

```text
GenuineColumn {
    schema: "rustred-equal-mass-euclidean-four-loop-halo-v1",
    corner_type: FourLoopGenuineCornerType,
    powers: [i32; 10], // in that type's frozen H/X completed basis
}
```

and its persistent key is exactly the corresponding
`FourLoopHaloColumnKey::GenuineRepresentative::stable_key()`.  Do not apply
`VacuumFamily::canonicalize` afterward: those families intentionally contain
only the identity permutation.  The canonicalization operation is instead
the deterministic genuine-corner witness followed by
`FourLoopHaloMapper::map_raw_halo_integral`.  A term already expressed on the
same frozen reference mask is terminal for *column normalization*, even though
it need not be a reduction master.

Keep canonical `MasterProduct<MassiveVacuumMaster>` values in a disjoint
product-column namespace.  Scaleless terms are dropped and are not matrix
columns.  The only four-loop products reachable after complete lower-loop
closure are

```text
T1^4, T1^2*S2, S2^2, T1*B4, T1*F5, T1*M6.
```

The atlas must retain X for `X9`.  Nine types use H coordinates, but the X top
graph is not a signed-`GL(4,Z)` image of H.  Rewriting a positive X denominator
as an affine polynomial in the H basis would put a sum in a denominator and
would not define an H exponent vector.  Consequently the 160-row set below is
precisely **144 H-reference rows plus 16 X-reference rows**; calling all 160
rows H-family rows would be incorrect.

Normalize an arbitrary transported halo term by the following terminating
dispatch:

1. form its scalar physical corner from the positive entries and classify it;
2. drop a proved scaleless sector;
3. reduce a factorized sector to the product namespace only when its powered
   payload is in the certified scalar `D1/N0` service; otherwise retain a typed
   blocker;
4. for a genuine sector, obtain its replayed inter-family witness, construct
   the affine mapper, and expand into that type's frozen basis;
5. emit branches which retain the witnessed active mask as genuine columns,
   and recursively dispatch only branches which lost an active line.

There is no upward edge in this recursion.  A degree-one affine numerator has
at most one constant plus ten basis terms.  A same-mask branch is emitted; a
branch which cancels an active denominator has consumed that numerator and is
strictly lower by active-line count.  Thus the corner halo has expansion depth
at most one followed by lower-sector dispatch.  Cache the result by
`(source family fingerprint, physical mask, powers)` so all 160 rows reuse the
same witness and affine map.

### 10.2 Generate and normalize the 160 rows without FORM

For each frozen type `t`, construct the same routing/basis as
`equal_mass_four_loop_vacuum(t.reference_topology())`, but use a helper like
the checked-in `reference_family_in_context` so every family clones one shared
`CoefficientContext(["d","m2"])`.  Then construct the scalar seed

```text
a_t[r] = 1 if r is a physical bit of t.reference_mask(), else 0.
```

Call `IbpGenerator::try_generate_raw(&a_t)` and retain the generator's fixed
lexicographic row order

```text
(differentiated_loop, contraction_loop) = (0,0), (0,1), ..., (3,3).
```

This gives 16 authenticated rows per type and 160 total.  Generate raw rows,
not pre-canonicalized rows: every subsequent equality must remain replayable
from the derivative label, seed, exact inverse-basis contractions, genuine
witness, affine images, and lower-sector proof.

For each raw term `c_b I(b)`, run the global dispatch above, multiply and
collect exact coefficients, then perform a mass-dimension normalization.  Let

```text
w(Genuine(_, b)) = sum_i b_i
w(Product(P))    = sum_(M,n in P) n * M.physical_lines()
p_t              = sum_i a_t[i].
```

Replace its coefficient by

\[
 \bar c_b=c_b\,(m^2)^{p_t-w(b)}.                 \tag{4}
\]

Equivalently, the matrix acts on formal columns
`J_b=(m2)^w(b) I_b` and the whole raw row has had its common
`(m2)^(-p_t)` removed.  Dimensional homogeneity proves that every collected
`bar c_b` lies in `Q(d)`; reject a row if exact Symbolica inspection finds any
residual `m2`.  Rank discovery can therefore set `m2=1` without losing generic
information, while final replay still uses the original row over
`Q(d,m2)`.

Order columns deterministically, easiest first, by

```text
zero (omitted)
< product stable key
< genuine(active lines, D+N, D, corner-type stable key, powers lexicographic).
```

Use the reverse order for pivot search.  After exact collection, divide a
nonzero row by the coefficient of its hardest column and record that factor;
this makes row serialization invariant under an overall rational rescaling.
Never normalize a coefficient by string formatting.  Store rational
polynomials canonically and hash the typed column keys plus canonical
numerator/denominator polynomials.

Recommended certificate records are:

```text
RawRowId { corner_type, differentiated_loop, contraction_loop }
NormalizedRow { raw_id, raw_hash, mass_weights, row_scale, sparse_entries }
PivotRule { pivot_column, rhs, source_row_weights }
CornerShellCertificate {
    schema, family_fingerprints, column_order, rows, rank, pivots, free_columns,
    exceptional_d_factors, affine_witness_hashes, lower_sector_proof_hashes
}
```

`source_row_weights` expresses each pivot row as an exact sparse combination
of the 160 normalized inputs.  It is the replayable elimination witness; a
cached RREF without these weights is not a certificate.

### 10.3 Finite dimensions and resource guards

For a frozen type with `p` active lines and `q=10-p` inactive completed-basis
entries, the scalar-corner raw halo has at most

\[
 1+p+p(p-1)+pq=1+10p                         \tag{5}
\]

distinct local index vectors: the corner, one dot, dot-plus-pinch, and
dot-plus-one-numerator cases.  Since the ten types have

```text
p = 5, 6, 6, 7, 7, 7, 8, 8, 9, 9,
```

their sum is at most 730 genuine columns.  Adding the six product columns
gives a structural corner-shell bound of 736 columns.  The actual count will
be smaller after inter-type normalization, factorization, and exact
cancellation.  The sparse matrix is therefore at most `160 x 736`, has rank
at most 160, and has at most 117,760 collected nonzeros.

Before collection, one corner row has at most
`1 + p*(1 constant + 10 basis coefficients) <= 100` terms.  Across all ten
types the sharper aggregate generator bound is 12,712 term incidences: for one
type the seed/dimension term occurs only in the four diagonal identities, so
the bound is `4 + 16*11*p`, and
`10*4 + 16*11*(5+6+6+7+7+7+8+8+9+9) = 12,712`.  (Charging one seed term in
every row would give the looser valid bound 12,832.)  An eleven-way degree-one
affine expansion gives a conservative 139,832 normalization contributions.  A
dense worst-case bidirectional elimination charge is
`160*159*736 = 18,723,840` coefficient updates, and full source-row provenance
needs at most `160^2 = 25,600` stored weights.  Use these as typed preflight
limits, together with dynamic limits on:

- coefficient numerator/denominator degree and polynomial term count;
- total serialized coefficient bytes;
- cached sector mappers and lower-sector normalization results;
- factorized tensor-expansion terms; and
- rejected modular samples and recorded exceptional factors in `d`.

The next complete `(D,N)=(1,1)` seed box is still finite but materially larger.
For one type its number of same-sector index vectors is

\[
 \binom{p+D}{D}\binom{q+N}{N}.               \tag{6}
\]

Here the first factor is the weak composition of at most `D` dots over the
`p` active lines, and the second is the weak composition of at most `N`
numerator powers over all `q` inactive completed-basis entries, including both
inactive physical lines and generated ISPs.

At `(1,1)` equation (6) sums to 296 seeds and hence 4,736 raw IBP rows over
the ten types.  Their one-step `(2,2)` dependency universe sums to 3,231
genuine vectors, or at most 3,237 columns after the six products. These are
pre-quotient estimates, not claimed observed counts. The landed
`FourLoopPolynomialHaloMapper` already supports checked degree-two polynomial
transport formed from products of affine images, with at most
`binomial(12,2)=66` collected monomials for the selected 123-seed manifest.
Scaling that service to the larger 296-seed box still requires a separately
frozen manifest and resource caps; the older
degree-one `FourLoopHaloMapper` must not be used outside its checked domain.

### 10.4 What this finite certificate proves

RustRed now builds a replayable sparse elimination certificate for the fixed
160-row corner shell over `Q(d,m2)`, with mass-normalized rows in `Q(d)` and
the structural bounds above.  Its exact acceptance path does all of the
following:

1. deterministically regenerate every native raw row and compare its stable
   `RawRowId`, normalized entries, blockers, closures, and aggregate stats;
2. replay every signed-`GL(4,Z)` and complete-basis affine identity;
3. replay every zero/factorized reduction used by global normalization;
4. verify equation (4) term by term and literal absence of residual `m2`;
5. replay every elimination source-row combination and obtain its pivot rule;
6. verify strict triangular order, reduce all 160 normalized rows to zero, and
   reconstruct every pivot from its exact stored source-row weights;
7. retain every denominator polynomial introduced by exact pivot division in
   the Symbolica coefficients; and
8. keep the advertised domain fixed to the ten corner seeds rather than infer
   coverage for any untested halo or hold-out seed.

The resulting exact rank is 159, with 64 free columns in the fixed finite
universe.  It does **not** prove that those columns are masters: corner
identities can leave dotted or numerator halo coordinates free.  Such columns
are an explicit incomplete-shell result, never silently promoted to masters.
A reduction claim for the ten corner targets requires adaptive seed growth
until every reachable nonterminal column is pivoted or reduced by a certified
lower node and the pivot/free skeleton is stable under one further shell.

The native 160-row implementation now lives in `src/four_loop_corner_shell.rs`
and is described in [`four_loop_corner_shell.md`](four_loop_corner_shell.md).
The separate lower-component scalar `D1/N0` service is integrated: it closes
all 234 coefficient-bearing occurrences in 95 preclosure rows through 28
authenticated cached witness plans and admits all 160 rows to elimination.
The immutable preclosure blockers remain in the certificate as provenance.
Its API reports the remaining finite-shell columns as free/unresolved, never
as masters.

This design mirrors only the checkable invariants of LiteRed2.  Its static
`GenerateIBP` builds the full derivative/contraction outer product and rewrites
scalar products into its denominator basis
([`LiteRed2026.m:1813-1823`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1813)).
`SolvejSector` imposes positive/nonpositive sector guards, searches nearby
points, increases depth, records bad applicability conditions, and reports
uncovered points as candidate masters
([`LiteRed2026.m:2384-2521`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2384)).
`IBPSelect` processes the hardest sector first and the final reducer applies
lower-sector dependencies before layering within-sector rules
([`LiteRed2026.m:3837-3889`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3837),
[`LiteRed2026.m:3956-4003`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3956)).
RustRed should implement the finite typed certificate above, not execute or
translate the Mathematica control flow.
