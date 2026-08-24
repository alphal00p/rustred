# Generic Symbolica symmetry discovery for RustRed

Status: implementation design based on the vendored LiteRed 2026 and
Symbolica 2.2.0 sources.  This is a topology-independent contract for any
authenticated `IntegralFamily` with `L` loop and `E` external momenta.  The
massive vacuum families through five loops are tests and milestones, never
branches in the production algorithm.

## 1. Decision and safety boundary

RustRed should copy LiteRed's two-stage idea, but make the boundary explicit:

1. restricted Symanzik polynomials and colored-graph canonization produce
   candidate sector matches and denominator permutations;
2. an exact momentum map is found or supplied; and
3. a standalone verifier replays the momentum, Gram, denominator, cut,
   power-shift, sector, and Jacobian equations before a rule is emitted.

A canonical signature, graph isomorphism, matching denominator count,
numerical sample, or exhausted resource budget is never a symmetry proof.
Only successful exact replay creates a `SymmetryCertificate`.  Conversely, a
failed or incomplete candidate search is not proof that no symmetry exists.

The proof-bearing core uses typed sparse polynomials and checked
`CoefficientContext` arithmetic.  Symbolica `Atom` patterns are useful at the
input/output boundary, but not for momentum algebra, guards, or certificate
replay.  FORM is not used anywhere.

## 2. What LiteRed actually does

The relevant reference implementation is
[`LiteRed2026.m`](../../vendor/LiteRed2/Source/LiteRed2026.m).

| Routine | Audited behavior | RustRed consequence |
|---|---|---|
| `jsSignature` / `jsSignaturePermutations`, lines 1567--1608 | Canonicalizes the sector-restricted `U+F`; an auxiliary parameter marks active cut denominators; also returns parameter automorphisms. | Reproduce the invariant as a candidate filter, with cuts and shifts represented as typed colors. |
| `FindShifts`, lines 3111--3231 | Tries parameter-signature permutations, constructs a general loop/external linear map, compares all scalar-product coefficients, and solves the resulting polynomial equations. | A denominator permutation is only a proposal.  Store and replay the actual momentum map. |
| `FindSymmetries`, lines 3306--3472 | Groups simple sectors by restricted Feynman-polynomial normal form, solves exact denominator-map equations, checks cuts and power shifts, then lifts maps to nonzero supersectors. | Keep candidate grouping separate from exact map validation and deterministic sector-orbit construction. |
| `FindExtSymmetries`, lines 3521--3622 | Applies the same idea between bases and records mapped/unmapped sectors. | Cross-basis certificates need both family identities and an explicit coefficient/external-basis transport. |
| `FeynParUF`, lines 4205--4280 | Builds `U` and `F` by completing the loop quadratic form and taking a determinant/adjugate. | Reuse the generic checked Symanzik builder; do not parse Mathematica expressions. |
| `polyNF` / `tableSortingPermutations`, lines 5945--6005 | Canonicalizes an exponent table whose rows carry coefficient-class labels, retaining all best column permutations. | A colored polynomial-incidence graph is the scalable equivalent; every extracted permutation still needs exact substitution replay. |

Two details must not be copied blindly:

- `FindShifts` line 3169 assigns `psy=EMs@nmx`; the surrounding logic and the
  later use show that this is almost certainly a source typo for the target
  basis.  RustRed uses independently authenticated source and target momentum
  manifests.
- LiteRed emits rules without making the loop-measure Jacobian prominent.
  RustRed calculates it explicitly and, in the first production compiler,
  accepts only unit-Jacobian loop maps.

LiteRed itself warns when two sectors have the same Feynman-parametric normal
form but no momentum shift is found.  That warning is an important semantic
clue: the polynomial signature was never intended to be the proof.

## 3. Existing RustRed input contract

The generic family implementation is
[`src/generic_family.rs`](../../src/generic_family.rs).  It already supplies:

- a complete affine denominator basis of size
  `N = L(L+1)/2 + L E`;
- coordinates ordered as upper-triangular loop-loop products followed by
  loop-external products in loop-major order;
- exact denominator constants and coordinate coefficients in one
  authenticated `CoefficientContext`;
- the external Gram matrix, formal power shifts, inverse denominator basis,
  family domain, construction limits, and a semantic family fingerprint; and
- exact constructor replay proving that the denominator-coordinate matrix is
  generically invertible.

[`src/sectors.rs`](../../src/sectors.rs) already supplies fixed-arity
`SectorMask`, `CutConstraint`, `SectorRestrictions`, explicit
excluded/unanalysed/proved states, and a persisted `IntegralOrderingPolicy`.
Cuts and patterns are admissibility metadata, not zero-integral evidence.

Symmetry code must consume these types rather than create a second momentum,
coefficient, sector, or ordering model.  In particular, all coefficient
operations must use the family context's checked `try_add`, `try_mul`,
`try_div`, and `try_neg` methods.  Raw Symbolica arithmetic can silently unify
foreign variable maps and is not an authentication boundary.

## 4. Exact map convention

Use source-to-target substitution throughout.  For equal loop count `L`, let

\[
 \ell^{s}_r=\sum_{a=0}^{L-1}A_{ra}\ell^{t}_a+
             \sum_{\alpha=0}^{E_t-1}B_{r\alpha}p^{t}_\alpha,
 \qquad
 p^{s}_\mu=\sum_{\alpha=0}^{E_t-1}C_{\mu\alpha}p^{t}_\alpha.
\]

`A`, `B`, and `C` contain exact authenticated coefficients.  Internal
symmetries normally use `C=I`.  A bijective cross-basis symmetry initially
requires equal `L`, equal `E`, and invertible `A` and `C`.  This does not bake
in a particular loop count or topology; it defines when two integration
problems can be declared equivalent.  A future one-way embedding with
rectangular `C` must have a distinct certificate kind and must not enter a
symmetry orbit.

The external map is valid only if

\[
        C G_t C^T=G_s.
\]

No inverse external Gram matrix is needed, so null or singular external
kinematics remain valid.  If an inverse map is claimed, replay both supplied
maps and their compositions; do not infer invertibility from a determinant
whose nonzero condition was discarded.

### 4.1 Scalar-product transport

Build an exact affine coordinate map

\[
       S_s=h+T S_t
\]

directly from `A`, `B`, `C`, and `G_t`.  For a source loop-loop coordinate,

\[
\begin{aligned}
 \ell^s_r\!\cdot\ell^s_u={}&
 \sum_{a,b} A_{ra}A_{ub}\,\ell^t_a\!\cdot\ell^t_b\\
 &+\sum_{a,\beta}(A_{ra}B_{u\beta}+A_{ua}B_{r\beta})
       \ell^t_a\!\cdot p^t_\beta
 +\sum_{\alpha,\beta}B_{r\alpha}B_{u\beta}G^t_{\alpha\beta}.
\end{aligned}
\]

For a source loop-external coordinate,

\[
 \ell^s_r\!\cdot p^s_\mu=
 \sum_{a,\beta}A_{ra}C_{\mu\beta}
       \ell^t_a\!\cdot p^t_\beta+
 \sum_{\alpha,\beta}B_{r\alpha}C_{\mu\beta}G^t_{\alpha\beta}.
\]

When storing an upper-triangular loop-loop coordinate, the off-diagonal
coefficient for `a<b` is
`A[r,a] A[u,b] + A[r,b] A[u,a]`; the diagonal coefficient is
`A[r,a] A[u,a]`.  This convention needs a dedicated replay test because a
factor-of-two error can survive symmetric examples.

### 4.2 Induced affine denominator map

Write the source and target denominator vectors as

\[
 D_s=c_s+R_sS_s,\qquad D_t=c_t+R_tS_t.
\]

The existing family constructor proves `R_t` invertible.  Therefore a known
momentum map induces, without another solve,

\[
 P=R_s T R_t^{-1},\qquad
 b=c_s+R_s h-Pc_t,qquad
 D_s(\ell^s,p^s)=b+P D_t.
\]

Store this as an `N_s x (N_t+1)` matrix with the constant in a fixed, documented
column.  Replay the transformed source denominators directly as well as the
matrix formula.  This catches orientation errors in `A`, `P`, or the inverse
basis independently.

An affine family map is more general than a single-integral map.  For a
source active denominator to map one integral to one integral, its row must
be monomial:

\[
       D^s_i=\lambda_iD^t_{\pi(i)},\qquad \lambda_i\ne0.
\]

Active rows must give a bijection onto the target active set.  Inactive
auxiliary denominators may have affine images; fixed nonpositive integer
powers can then be expanded into a bounded linear combination.  An arbitrary
positive or symbolic power of an affine sum cannot be compiled as one
integral.  Expose this distinction as:

- `AffineFamilyMap`: exact momentum and full denominator identity;
- `SectorIntegralMap`: active monomial rows plus a sector mapping; and
- `FamilyPermutationMap`: every row monomial, suitable for cheap arbitrary
  index canonicalization.

Never silently coerce the first kind into the third.

The rows requiring monomial action are therefore the union of active sector
positions and positions with a nonzero formal power shift.  An inactive row
with zero shift may use bounded polynomial expansion; an inactive row carrying
a formal shift may not.

### 4.3 Jacobian and scale factors

With the convention above,

\[
  \prod_{r=0}^{L-1}d^d\ell^s_r=
  |\det A|^d\prod_{r=0}^{L-1}d^d\ell^t_r.
\]

The certificate records `det(A)` exactly and a `JacobianWitness`.  The first
rule compiler accepts only `det(A)=+1` or `-1`, for which the unoriented
measure factor is one.  A non-unit determinant can still be replayed and
stored as a formal `DeterminantPower` map, but current `Coefficient` cannot
represent `|det A|^d`; rule emission must return
`UnsupportedJacobian`, never drop the factor.

If `D^s_i=lambda_i D^t_pi(i)`, an exponent `a_i+nu_i` contributes
`lambda_i^(-a_i-nu_i)`.  For variable integer `a_i` this is not an ordinary
rational-polynomial coefficient unless `lambda_i=1` (or a separate formal
power language is used).  Version one therefore requires unit denominator
scale on parametric rules.  Fixed concrete integer indices may use a checked
integer power.  A nonzero formal shift additionally requires exact shift
transport and unit scale.

### 4.4 Cuts, shifts, and sectors

For the convention `source i -> target pi(i)`:

- `source_sector[i] == target_sector[pi(i)]` for a bijective sector map;
- required cut positions map exactly to required cut positions for a symmetry;
- `source_power_shift[i] == target_power_shift[pi(i)]` after authenticated
  coefficient transport; and
- source and target sector patterns are checked independently for
  admissibility.

One-way containment of cut sets is a different relation and must not be stored
as a symmetry.  Cuts and patterns may reject a map, but cannot prove an
integral zero.

## 5. Restricted Symanzik signatures

Use the generic checked construction specified in
[`zero_sector_symbolica_design.md`](zero_sector_symbolica_design.md).  For
Feynman parameters `x_i`, it constructs

\[
 U=\det Q,\qquad
 F=U C-R^T\operatorname{adj}(Q)R
\]

in the exact coefficient field, with the external Gram contraction included
in the second term.  Restricting a sector sets inactive `x_i` to zero while
retaining the authenticated full variable order.

For symmetry candidates, the effective parameter support is the union of:

- active sector positions; and
- positions with nonzero formal power shifts.

That matches the important behavior of LiteRed's `FindSymmetries`.  Cut,
activity, and shift data are not encoded by algebraic tricks in the proof
layer; they remain typed metadata.

### 5.1 Colored polynomial-incidence graph

Replace factorial column sorting with a bipartite graph:

- one variable vertex for each effective Feynman parameter;
- one monomial vertex for each nonzero term of `U` and `F`;
- an undirected incidence edge labelled by the positive exponent; and
- vertex colors that cannot be interchanged across semantic roles.

Recommended stable colors are:

```text
VariableColor {
    active: bool,
    required_cut: bool,
    power_shift_key: ExactCoefficientKey,
}

MonomialColor {
    channel: U | F,
    coefficient_key: ExactCoefficientKey,
    total_degree: u32,
}
```

Separate `U` and `F` channels are at least as discriminating as `U+F` and are
invariant for the unit-Jacobian map class.  If a future search class permits
rescalings that mix these normalizations, it must use an invariant signature
for that class or bypass the filter.  A filter that can reject a valid member
of the declared search domain invalidates a `Complete` result.

`ExactCoefficientKey` is a versioned structural encoding of numerator and
denominator integer term arrays in the ordered base-parameter manifest.  It
must not depend on Symbolica symbol ids, hash-map iteration, or display
formatting.  Cross-basis comparison first transports coefficients into one
authenticated target context.

The vendored [`Graph::canonize`](../../vendor/symbolica/lib/graphica/src/lib.rs#L1383)
returns a canonical graph, an input-to-canonical `vertex_map`, orbit data, and
automorphism generators expressed as cycles.  Extract a candidate denominator
permutation only from variable vertices.  Conjugate generators back from
canonical numbering, check that every cycle stays within the variable color
class, then replay the corresponding polynomial substitution exactly.  A
source-to-target candidate is obtained by composing the source canonical map
with the inverse target canonical map.

The graph's automorphism group can be enormous.  Keep generators as generators
and materialize group elements only under `max_generated_permutations`.
`Graph::canonize` also contains an unbounded `leaf_nodes` search table (source
line 1407).  Node/edge preflight alone is not a hard search-step bound.  Before
production calls this a bounded operation, RustRed must either add a
cancellation/step counter to the vendored graphica adapter or run canonization
in a killable pure-Rust worker process.  Until then, report its limit as an
input-size limit, not a misleading time or memory guarantee.

Every candidate permutation is checked by direct typed polynomial
substitution.  Graph equality remains only provenance for why the candidate
was tried.

## 6. Finding momentum maps

The hard problem is producing `A`, `B`, and possibly `C`.  Verification is
easy and independent.  Use a producer interface so faster algorithms can be
added without changing certificate semantics:

```rust
trait MomentumMapCandidateBackend {
    fn candidates(
        &self,
        problem: &MomentumMapProblem<'_>,
        limits: &SymmetrySearchLimits,
    ) -> Result<CandidateBatch, SymmetrySearchError>;
}

struct CandidateBatch {
    candidates: Vec<MomentumMapCandidate>,
    completion: SearchCompletion,
}
```

All producers feed the same verifier.  The following producers are useful in
order.

### 6.1 Explicit maps

Accept caller-provided maps, including external maps, only as candidates.
This is the reliable route for unusual kinematic bases and is also the simplest
way to import independent oracle data.  Exact replay is mandatory.

### 6.2 Authenticated routed-quadratic witnesses

When a denominator is supplied with an optional routing witness
`q_i = u_i ell + v_i p` and mass/constant data, authenticate by expanding
`q_i^2+m_i^2` into its stored affine denominator.  This is generic metadata,
not a topology case.

For a proposed denominator permutation, choose linearly independent routed
denominators and signs and solve

\[
  (u_i^s,v_i^s)
  \begin{pmatrix}A&B\\0&C\end{pmatrix}
  =\epsilon_i(u_{\pi(i)}^t,v_{\pi(i)}^t),
  \qquad \epsilon_i\in\{-1,+1\}.
\]

This is exact linear algebra once signs and `C` are fixed.  Prune a partial
sign assignment as soon as a solved row contradicts another denominator.
It generalizes the current vacuum-family geometric validator and is the fast
path expected to cover massive vacuum milestones.

An optional routing witness should be added alongside, not inferred
irreversibly from, `AffineDenominator`: a rank-one quadratic form can require
square roots or have ambiguous normalization.  Families without witnesses
remain valid and use another backend.

### 6.3 Exact polynomial-system backend

Without routing witnesses, insert unknown entries of `A`, `B`, and `C`, expand
the selected denominator equalities and external Gram condition, and equate
every scalar-product and constant coefficient.  These equations have degree
at most two before determinant constraints.  Add a branch
`det(A)=+1 | det(A)=-1` for the production unit-Jacobian search class.

Represent the equations as typed
`MultivariatePolynomial<RationalPolynomialField<IntegerRing,u16>,u16>` with a
fixed unknown-variable map.  Use Symbolica polynomial differentiation and
substitution for construction, and dense exact matrices for purely linear
subsystems.  Relevant APIs are:

- `replace_with_poly`, `replace_all`, `derivative`, and monomial-order
  `reorder` in
  [`polynomial.rs`](../../vendor/symbolica/src/poly/polynomial.rs);
- `Matrix::{solve,solve_any,row_reduce,rank,det,inv}` in
  [`matrix.rs`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs); and
- [`GroebnerBasis::new`](../../vendor/symbolica/src/poly/groebner.rs#L127).

The current Gröbner API eagerly runs F4, silently unifies variable maps, has no
cancellation or term budget, contains internal panic paths, and returns an
ideal basis rather than the solutions of that ideal.  Therefore it must not
be treated as a bounded general `Solve` replacement.

The initial exact backend should do checked linear elimination and bounded
split-rational triangular branching.  It may return complete results for a
declared finite coefficient alphabet or for a zero-dimensional system whose
branches reduce to linear factors over the authenticated base field.  It must
return `UnsupportedSolutionField` for irreducible algebraic roots and
`Incomplete` for a positive-dimensional or resource-limited problem.  An
optional Gröbner accelerator can propose a triangular system in an isolated,
bounded worker; every recovered solution is still replayed.

This honesty matters mathematically: unrestricted maps with arbitrary
rational-function entries form an infinite search space, and computing a
Gröbner basis alone does not prove that every base-field point was enumerated.
The request records its `MomentumMapSearchDomain`, for example:

```text
ExplicitCandidates
RoutedQuadratic { allowed_signs }
FiniteAlphabet { exact_entries }
SplitRationalZeroDimensional
```

Only exhaustive completion of that named domain permits
`SearchCompletion::Complete`.  The public API never converts an incomplete
domain into “no symmetries.”

## 7. Public data model

The exact names can change, but the semantic separation should not:

```rust
pub struct ExactMatrix<T> {
    pub rows: usize,
    pub columns: usize,
    pub row_major: Box<[T]>,
}

pub struct MomentumMap {
    pub loop_linear: ExactMatrix<Coefficient>,       // A: L x L
    pub loop_external: ExactMatrix<Coefficient>,     // B: L x E_t
    pub external_linear: ExactMatrix<Coefficient>,   // C: E_s x E_t
}

pub struct AffineDenominatorMap {
    pub linear: ExactMatrix<Coefficient>,             // P
    pub constant: Box<[Coefficient]>,                 // b
}

pub enum DenominatorRowAction {
    Monomial { target: usize, scale: Coefficient },
    Affine,
}

pub enum JacobianWitness {
    Unit { determinant_sign: i8 },
    FormalDeterminantPower { determinant: Coefficient },
}

pub struct SymmetryCertificate {
    pub schema: SymmetryCertificateSchema,
    pub source_family_fingerprint: String,
    pub target_family_fingerprint: String,
    pub coefficient_transport: CoefficientTransport,
    pub source_sector: SectorMask,
    pub target_sector: SectorMask,
    pub momentum: MomentumMap,
    pub inverse_momentum: Option<MomentumMap>,
    pub denominators: AffineDenominatorMap,
    pub row_actions: Box<[DenominatorRowAction]>,
    pub jacobian: JacobianWitness,
    pub cut_witness: CutTransportWitness,
    pub shift_witness: PowerShiftTransportWitness,
    pub candidate_provenance: CandidateProvenance,
    pub replay_guards: GuardSet,
    pub checksum: ArtifactChecksum,
}
```

`CoefficientTransport::SameVariableMap` is enough for the first
implementation and is checked with `CoefficientContext::has_same_variable_map`.
A later cross-context isomorphism stores forward and inverse parameter
substitutions and replays both on every family coefficient.  Equal parameter
names alone are insufficient.

Persist coefficients as structural integer term arrays plus the ordered
parameter manifest.  Do not persist Symbolica process-local symbol ids.  The
schema fixes matrix orientation, denominator-permutation orientation,
coordinate order, measure convention, and hash algorithm.

Candidate provenance may contain the signature digest, graph vertex map,
generator word, backend name, and search-domain fingerprint.  It is useful for
debugging and reproducibility but is not in the trusted proof kernel.

### 7.1 Search result, not ambiguous `Vec`

```rust
pub enum SearchCompletion {
    Complete { domain_fingerprint: String },
    Incomplete { reason: IncompleteReason },
}

pub struct SymmetrySearchReport {
    pub certificates: Vec<SymmetryCertificate>,
    pub rejected_candidates: Vec<CandidateRejectionSummary>,
    pub completion: SearchCompletion,
    pub statistics: SymmetrySearchStatistics,
}
```

An empty `certificates` vector with `Complete` means no map in the declared
domain.  The same vector with `Incomplete` says nothing about existence.

## 8. Authoritative certificate replay

`SymmetryCertificate::verify` performs the following steps in this fixed
order, with checked arithmetic and transactional error handling:

1. Verify schema, checksum, family fingerprints, coefficient manifests, and
   the exact source/target context transport.
2. Check every dimension with checked `usize` arithmetic before allocation:
   `A=LxL`, `B=LxE_t`, `C=E_sxE_t`, the affine map, sectors, cuts, and shifts.
3. Authenticate every coefficient and collect all input-denominator and
   family-domain guards.  A foreign variable map is a hard error.
4. Compute `det(A)` exactly.  Reject zero; verify the stored Jacobian witness.
   For an orbit equivalence, replay the supplied inverse and both identity
   compositions.
5. Replay `C G_t C^T=G_s` entry by entry without inverting either Gram matrix.
6. Build `h,T` from the explicit scalar-product formulas in section 4.1.
7. Transform every source denominator directly, derive `b,P`, and compare
   every coefficient with the stored affine denominator map.
8. Reclassify every row as monomial or affine; do not trust the stored tag.
   Check nonzero scales with explicit guard semantics.
9. Replay source-to-target activity, bijection, cuts, power shifts, and both
   sector restrictions.  These checks cannot be replaced by the signature.
10. Recompute the integral prefactor/Jacobian policy and prove that the
    requested rule kind can represent it.
11. Recompute the certificate's canonical structural encoding and checksum.

The verifier returns a newly derived `VerifiedSymmetryCertificate`; rule and
orbit APIs accept only that wrapper, not the deserialized unchecked struct.
No mutation of a cache or guard set occurs until every replay step succeeds.

Composition and inversion are operations on verified certificates.  They
multiply `A/B/C`, compose affine denominator maps, union guards with bounded
origin provenance, replay the result, and only then retain it.  This provides
an independent way to test orientation conventions.

## 9. Sector orbits and deterministic ordering

Discovery and integral ordering are different policies.  Persist a dedicated
symmetry representative policy.  To preserve RustRed's current lexicographic
direction and corner representatives while fixing numerator dispatch, version
one should be:

```text
rustred.symmetry-sector-first-lexmax.v1
1. maximize the sector bit string;
2. within that sector stabilizer, maximize the complete signed index vector;
3. if several maps give the same vector, minimize
   (denominator permutation, flattened A, flattened B, flattened C, certificate hash).
```

This keeps the existing `VacuumFamily::canonicalize` lexicographic direction
and its corner representatives, while adopting the corrected sector-first
behavior already documented for tensor boundaries.  Some dotted legacy
representatives can consequently change, which is why the policy has a new
fingerprint.  Reduction pivots and strict descent continue to use the separate
persisted `IntegralOrderingPolicy`; changing either policy requires a new
fingerprint.

For simple-sector lifting, follow LiteRed's useful structure but retain proof
objects:

1. sort admitted proved-nonzero simple sectors by the representative policy;
2. group only signature-compatible sectors;
3. verify direct generators;
4. use verified inverse/composition to build the orbit;
5. map each noncanonical member to the representative with the minimum stable
   certificate key; and
6. retain representative stabilizer generators as self-symmetries.

Do not enumerate a full group merely to store it.  A bounded orbit traversal
can enumerate images of a requested sector or integral.  If the traversal
hits its cap, canonicalization returns `Incomplete`, not a noncanonical input
unchanged as though it were proved unique.

## 10. Cross-basis maps

A cross-basis request contains two complete families, two restriction sets,
an explicit coefficient transport policy, an external-map policy, and a
persisted family precedence.  Version one supports bijective equivalence when:

- source and target have the same `L` and `E`;
- their coefficient contexts are identical, or an authenticated invertible
  transport is supplied;
- `C` and its inverse pass Gram replay;
- the loop map is invertible and unit-Jacobian for rule emission; and
- active denominator rows, cuts, and formal shifts map bijectively.

Different denominator orderings, loop routings, names, constants, and ISP
bases are allowed.  The full affine denominator map is precisely what makes
different auxiliary bases usable.

Family names never decide orientation.  The request supplies a stable
`FamilyPrecedenceKey`; then sector representative policy and certificate key
break ties.  A cross-basis mapping always rewrites from the lower-precedence
source to the chosen target, and the artifact records both fingerprints.

Supporting `E_s != E_t` later requires a separate one-way kinematic embedding
certificate.  It may prove a valid substitution, but without an inverse it is
not an orbit symmetry and cannot be used to claim two master integrals are the
same.

## 11. Resource limits and failure semantics

Add a dedicated `SymmetrySearchLimits`, independent of family construction
limits.  At minimum it contains:

```text
max_sectors
max_signature_terms
max_signature_vertices
max_signature_edges
max_graph_automorphism_generators
max_generated_permutations
max_candidate_sector_pairs
max_candidate_denominator_permutations
max_routing_basis_choices
max_sign_assignments
max_external_map_candidates
max_unknowns
max_equations
max_equation_terms
max_polynomial_degree
max_polynomial_branches
max_exact_operations
max_matrix_entries
max_replay_terms
max_inactive_expansion_terms
max_guard_polynomials
max_guard_origins
max_certificate_bytes
```

Use checked multiplication/addition for every prospective allocation and
charge aggregate work across the complete request, not separately per
candidate.  Limits are checked before retaining an output and before calling
panic-prone Symbolica constructors.  Checked wrappers must bound coefficient
term products and integer bit growth as well as container counts.

Failures are typed:

- `InvalidInput` or `ForeignContext`: no search was started;
- `CandidateRejected`: exact replay disproved this candidate;
- `UnsupportedMapClass` / `UnsupportedSolutionField`: sound verifier, missing
  producer/compiler capability;
- `ResourceLimit` / `Cancelled`: partial candidates may be returned with
  `SearchCompletion::Incomplete`;
- `InternalAlgebraFailure`: no proof object is emitted; and
- `Complete`: exhaustive only for the fingerprinted search domain.

Do not catch a panic and label it “no symmetry.”  If an unavoidable vendored
operation is isolated behind `catch_unwind`, the result is an internal failure
or incomplete search, and no partially assembled certificate or guard is
committed.

## 12. Symbolica pattern matching boundary

For user-facing syntax, Symbolica's `Pattern`, `MatchSettings`,
`ReplaceBuilder`, and replacement iterators can recognize forms such as a
momentum-map declaration or integral head.  Use a whole-expression match with
`partial(false)`, extract owned values, then validate arity, symbols, integer
indices, and coefficient contexts in RustRed types.

Do not encode cut membership, positivity, nonzero determinants, or
power-shift equality as matcher conditions.  Symbolica conditions can be
`Inconclusive`, and relational matcher ordering is canonical expression order,
not mathematical numeric order.  Do not use general patterns to rewrite
denominators during replay.  Typed coordinate substitution is both faster and
less ambiguous.

## 13. Tests and independent oracles

### 13.1 Unit and property tests

- Construct `h,T` for random small exact `A,B,C,G`, compare every coordinate
  against direct bilinear expansion, and explicitly test off-diagonal factors.
- Generate unimodular maps, derive the target family, discover/replay the map,
  then replay inverse and composition to the identity.
- Relabel loop, external, denominator, and base-parameter manifests and prove
  that canonical signatures change only through the authenticated transport.
- Compare graph-derived polynomial permutations with brute-force
  permutations for small parameter counts.
- Deliberately create signature collisions whose denominator equations fail;
  assert that no certificate is emitted.
- Test singular `A`, non-unit Jacobians, malformed inverse maps, asymmetric or
  singular Gram matrices, foreign coefficient maps, wrong matrix dimensions,
  and tampered checksums.
- Test cut-preserving and cut-violating maps separately.  Assert that cut
  exclusion is never returned as a zero proof.
- Test equal, permuted, unequal, and symbolic power shifts; reject non-unit
  denominator scale with symbolic shifts.
- Test active monomial/inactive affine maps, bounded numerator expansion, and
  expansion-limit rollback.
- Force every limit one below and exactly at the required count.  Results and
  statistics must be deterministic, and truncation must be `Incomplete`.
- Exhaustively compare generator-orbit canonicalization with full group
  enumeration for small groups.

### 13.2 Integration milestones

Use no production topology branches.  Express each case as ordinary family
data and compare:

1. the two-loop massive vacuum family: all expected denominator permutations,
   sector orbits, inverse maps, and unit Jacobians;
2. the three-loop massive vacuum families: tetrahedron/banana orbit sizes,
   numerator-affine ISP images, cuts, and power shifts;
3. the four- and five-loop vacuum families: generator-based orbit
   canonicalization without factorial materialization, plus bounded inactive
   numerator expansion; and
4. non-vacuum synthetic families with `E>0`, nontrivial `B` and Gram-preserving
   `C`, including singular external Gram matrices.

Golden fixtures may be generated offline with LiteRed and checked into the
test suite as plain structural data.  Rust tests do not invoke Mathematica or
FORM.  Independent verification consists of exact denominator replay, IBP
residual replay after canonicalization, inverse/composition checks, and
held-out relabelings rather than comparison to one implementation alone.

## 14. Implementation sequence

1. Add checked `ExactMatrix`, structural coefficient encoding, and momentum /
   affine-denominator replay helpers over `IntegralFamily`.
2. Add `SymmetryCertificate`, unchecked deserialization, authoritative
   verifier, inverse/composition, and unit-Jacobian rule compilation.
3. Reuse the checked Symanzik builder and add the colored incidence graph,
   exact permutation replay, and deterministic signature cache.
4. Add optional authenticated routed-quadratic witnesses and the exact
   sign/basis linear producer.  This is the fast generic path for the vacuum
   milestones.
5. Add sector orbit construction, the versioned sector-first representative
   policy, and an adapter replacing permutation-only `VacuumFamily`
   canonicalization.  Treat dotted legacy-representative changes as an
   explicit fingerprinted migration.
6. Add cross-basis requests with same-context and explicit external maps.
7. Add checked inactive-affine numerator expansion and tensor-vector transport.
8. Add the bounded polynomial-system backend, with precise completion-domain
   reporting; only later add a Gröbner accelerator behind the same producer
   interface.
9. Add coefficient-context isomorphisms and one-way kinematic embeddings as
   new certificate kinds, not relaxations of existing replay.

The dependency direction is deliberate:

```text
IntegralFamily + SectorRestrictions
        -> checked Symanzik candidate signatures
        -> momentum-map candidate producers
        -> authoritative certificate replay
        -> verified sector/family orbits
        -> bounded integral/tensor rule compilation
```

This keeps Symbolica where it is strongest—exact polynomial, matrix, and graph
operations—while RustRed owns the physics semantics, resource policy,
provenance, and proof boundary needed for a complete LiteRed port.
