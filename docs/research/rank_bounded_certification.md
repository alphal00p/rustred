# Rank-bounded entry scopes for second-stage certification

Status: implementation recommendation, 2026-09-16. This note describes a new
optional certification mode, not a capability already provided by the public
API. Unbounded certification remains the default. See also
[complete certification literature](complete_certification_literature.md),
[the convergence plan](certification_convergence.md), and
[semi-numerical proof systems](seminumerical_proof_systems.md).

Implementation status: increment A below is now present as internal code,
reusing the existing weak-composition enumerator. Its 11 rank-geometry and six
requested-coverage regressions passed in the combined source-port gate:
249 passed, zero failed, five existing ignored tests. No public rank flag,
scoped artifact grammar, or rank-bounded closure claim is enabled by that gate.

The next internal increment now provides `PreparedSuccessorScope`: destination
box unions are admitted, copied and sorted once, then reused for exact physical
RHS-image containment checks. All four active/inactive coordinate maps and
sign crossings preserve genuine infinite endpoints. Query subtraction may only
tighten the stored resource policy, with one cumulative budget across terms.
Thirteen successor tests and a generic query-cap regression passed alongside
the existing gate: 264 source-port tests, 15 completion tests and three native
reconstruction tests, zero failures, five existing ignored source-port tests.
Independent implementation and mathematical review passed. This is still a
geometry service, not a public scoped artifact: original replay, predicate
coverage, machine-index admission, cold persistence and entry enforcement have
not been redirected to a rank-restricted publication path.

The next narrow primitive is now also present internally
(`SuccessorClosedScope`, RustRed commits `fbff9fd4`/`8e22f2fb`). It owns an
exact admitted-entry union and an immutable destination union, proves entry
admission at construction, and checks every translated RHS image against that
same union with the existing Symbolica-backed box geometry. Escapes fail closed
with a typed error. Twenty-eight focused scope tests pass. This is deliberately
not a public rank-bounded artifact claim: persistence, source replay, guards,
descent, cold loading and runtime entry enforcement still have to be composed
around it.

## Recommendation and immediate implementation

Use **finite negative-index slices with unbounded positive-index rays**, and
certify an explicit **successor-closed proof domain** containing the requested
entry domain. Reuse the existing exact box geometry, original-source replay,
native Symbolica guard checks, and well-founded descent checks. Do not build a
new polyhedral or polynomial solver for this feature.

The smallest safe implementation increment is an internal scoped-coverage
primitive and a checked slice constructor. It does not yet justify exposing a
rank flag. The first public mode must additionally persist the entry scope,
verify successor containment both warm and cold, and reject out-of-scope
starting integrals. A bound used only as a report field or only to truncate
coverage is not an implementation of rank-bounded certification.

Initially reject a proof-domain escape with an explicit diagnostic. Then add
bounded envelope expansion, reusing the same verifier. Expansion is a proposal
mechanism: reaching a resource limit or failing to find an invariant must not
publish an artifact. A deliberate full-sector fallback is safe only after all
newly included points have passed the existing unbounded checks.

### Final recommendation across the certification research

Implement one shared, domain-aware exact proof path, in this order:

1. Retain and validate the completed **target-relative whole-exclusion** and
   **singleton-face propagation** fixes. They remove demonstrated conservative
   failures without new authority, topology-specific formulas, or larger caps.
2. Add **requested-domain coverage and exact low-degree slices**, then the
   coherent **inductive envelope / cold scope / runtime entry** contract below.
   This is the most direct optional speed optimization when the caller knows
   the starting numerator bound; keep the unrestricted path unchanged.
3. Generalize remaining local polynomial implications with **compact exact
   witnesses**: use native modular/sampled linear algebra only to propose sparse
   ideal multipliers, then check the full characteristic-zero polynomial
   identity and domain sign/exclusion obligations. Share authenticated parent
   preparation and bounded caches keyed by the complete actual proof domain.
   A failed proposal is not a failed theorem; keep existing exact fallbacks.

Do **not** implement a new general Presburger engine, CAD/SOS solver,
comprehensive Gröbner system, relational-polyhedral engine, or unbounded
automatic envelope search now. Those are useful completeness landmarks or
possible later integrations, but carry substantially more implementation and
verification complexity than the demonstrated bottlenecks justify. Do not
replace symbolic verification by finite-field success rates or finite-grid
tests. A lazy pointwise certified lane could complement the uniform artifact
later, but must not be mislabeled as bounded-rank uniform closure.

This is a speed/coverage strategy with a small exact checker and explicit
failure states, not a claim of universal algorithmic completeness. Measure the
actual saved-bundle campaigns after each coherent step; do not infer progress
from a growing list of local regression tests alone.

## 1. Precisely define the bound

For the scalar integral key `n`, define the negative-index degree

\[
  s(n)=\sum_i \max(-n_i,0).
\]

The low-level API should say `max_negative_index_degree`, not leave the meaning
of “rank” implicit. This is the standard scalar-index complexity distinct from
the sum of positive powers; see Reduze 2, section 2.1, and Kira 2.0, section 2.1.
Neither definition supplies a proof that a reduction stays inside a chosen
bound. [Reduze 2](https://arxiv.org/pdf/1201.4330),
[Kira 2.0](https://arxiv.org/pdf/2008.06494).

For the currently certified vacuum families, whose inverse propagators are
quadratic in loop momenta, `2*s(n)` is the scalar numerator's maximal momentum
degree in this representation. A convenience `max_numerator_rank=R` can mean
`s(n)<=floor(R/2)` **only with that explicit family/representation contract**.
Mass constants add lower-degree terms. This is not a definition of an arbitrary
tensor numerator's Lorentz rank, nor of degree for general linear or mixed
propagators. Tensor-to-scalar lowering must separately establish which scalar
keys its input produces; it cannot infer applicability from a tensor label.

Let `root` be the existing caller-declared sector mask. The precise entry set is

\[
 E_D=\{n: \operatorname{sector}(n)\subseteq root,\ s(n)\le D\}.
\]

Positive denominator powers remain arbitrary, subject to the existing machine
index and checked-arithmetic contract at application time. `D=0` is useful and
valid: inactive powers are zero, but positive powers are still unbounded.
`None` means the existing unbounded contract. A finite bound applies to the sum
over **all** negative indices, including pinched root-active propagators, not
only a list of designated irreducible scalar products.

Compute entry membership using checked unsigned/wide arithmetic, including
`i64::MIN`; do not negate in `i64`, narrow to search `i16`, or wrap the sum.
Permutation changes coordinate order, not this sum. A rectangular hull with
each negative power at least `-D` is not the same set: it admits degree `k*D`
with `k` inactive axes.

## 2. Entry scope is not the proof domain

The actual artifact contract needs a second set `P`, containing every possible
nonterminal descendant of every admitted entry:

1. `E_D` is contained in `P` (or in explicitly proved zero terminals).
2. Every point of `P` is covered by an admitted executable rule, a proved zero,
   or an explicit finite terminal.
3. Every nonzero RHS contribution of every rule that the executor can select
   on `P` lands in `P` or an independently valid terminal.
4. Original-source replay, all original source conditions and denominator
   guards, and strict descent hold throughout each rule's retained domain.

These obligations imply termination to the declared terminal space for each
admitted entry, by the existing well-founded order. They are a sufficient
certificate, not a claim that the producer always discovers such a `P`.

There is no general implication `s(child)<=s(parent)` from current descent.
`sector/ordering.rs::ComplexityKey::cmp` compares sector and total corner
distance before the dot/numerator tie-break. A step replacing two dots by one
numerator can descend while increasing `s`. Repeating that step from arbitrarily
large initial denominator powers can require arbitrarily large descendant
numerator degree. Finite entry rank therefore does not imply a finite uniform
rank envelope. Sector-changing steps require the same care.

Source rows used to replay a rule may themselves refer outside `P`; that is not
an execution escape. Regenerate those rows and preserve their applicability
conditions and globally valid zero evidence as today. The inductive-domain
obligation concerns the rule's executed nonzero RHS after exact pruning, not
every intermediate column in its algebraic derivation.

Checking all retained applicable rules is a simple sufficient first contract.
A future implementation may check only exact first-owner regions, but only if
it proves that those regions match runtime precedence, including all guards.
Never assume that an unverified “unused” overlapping rule cannot be selected.

## 3. Geometry with no new CAS

In a sector with `k` inactive coordinates, enumerate all tuples
`a in N^k` with `sum(a)<=D`. Each tuple gives one `LatticeBox`: inactive axes
are singleton `a_i`; active local axes are `[0,infinity)`. Physical powers are
`-a_i` and `local+1`, respectively. There are exactly

\[
  {D+k\choose k}
\]

such slices. They are an exact finite union of infinite domains, not a finite
sample. Preflight this count and the total coordinate storage before allocation
or native algebra. Enumerate deterministically; do not silently keep only the
first budget-sized prefix. Across a root downset, sum the counts for every
nonzero sector, with one shared scope budget.

Existing box intersection/subtraction handles slice clipping, and existing
singleton substitution often collapses expensive multivariate guards. H229's
lost singleton face is a direct motivation: retaining exact fixed coordinates
can reduce polynomial work before native chart restriction. The new FG115
target-relative whole-exclusion check remains necessary on every slice; a rank
bound does not excuse a bad descent proof on a live point.

For successors, first use existing `geometry::sign_partition` so every shifted
coordinate has a fixed sign. Checked endpoint translation then produces the
child's box in its own local orthant. Keep `None` endpoints genuinely infinite.
Check containment in the destination proof-domain union with
`BoxCover::uncovered_within`, or conservatively require rectangular containment.
If a rectangular image overapproximates an affine target minus exclusions,
failure is inconclusive, not a counterexample. An exact predicate-relative
containment extension can improve coverage later without changing the contract.

No native polyhedral/Presburger optimizer was found in the pinned Symbolica 3
source inspected for this task. Symbolica supplies polynomial operations,
factorization, exact matrices, native affine canonicalization through RustRed,
and modular discovery primitives; RustRed already supplies exact integer box
geometry. Do not reinterpret a `simplex` polynomial-multiplication helper as a
polyhedral feasibility service, or a rational RREF result as integer feasibility.

## 4. Envelope choices and tradeoffs

| Choice | Valid guarantee | Cost or limitation |
| --- | --- | --- |
| Prove all sectors unbounded, then restrict entry rank | Existing global closure, smaller advertised entry set | Safe but no expected certification speedup |
| Use `P=E_D`, with a checked successor invariant | Uniform reduction for all bounded-rank starts and all positive powers | Rejects valid reductions whose descendants leave `E_D` |
| Finite larger slice envelope `P`, checked independently | Allows a limited rank excursion | Slice count grows combinatorially; some programs need unbounded excursions |
| Widen selected sectors to full orthants | Covers arbitrary rank there after full proof | May recover much of the original certification cost |
| Relational cones, e.g. rank bounded by dots plus a constant | Can express correlated unbounded descendants | Needs additional exact inequality/coverage infrastructure; not the first increment |
| Lazy exact per-target proof | Certifies the requested target's reached DAG | Does not certify every bounded-rank start with arbitrary positive powers |

A useful bounded expansion policy adds missing successor boxes to `P`, repeats
proof checks for affected regions, and widens a growing sector to its full
orthant after a declared threshold. This follows the inductive-invariant idea
of abstract interpretation: an overapproximation is useful only when its
transfer/containment obligations hold. The policy is a RustRed proposal, not a
new theorem about IBP reductions. [Cousot and Cousot, POPL 1977](https://www.di.ens.fr/~cousot/COUSOTpapers/POPL77.shtml).

Do not silently change the requested entry set when widening `P`. Report both
sets, expansion count, widened sectors, and the exact resource policy. Do not
automatically restart an expensive unbounded certification after a bounded
attempt unless that fallback is explicit. A whole-sector fallback preserves
soundness, not a promise of speed or successful completion.

The existing experimental `CandidateReducer` is useful to investigate concrete
escapes and likely dependencies. It is not an alternative publication gate:
its pointwise execution and search provenance do not replace original-source
replay or a uniform coverage certificate.

## 5. Current source seams and required changes

The following paths are relative to `crates/rustred-core/src` unless marked;
table entries beginning `source_port/` abbreviate `foundry/artifact/source_port/`,
and entries beginning `artifact/` abbreviate `foundry/artifact/`.

| Existing seam | Current behavior | Scoped implementation obligation |
| --- | --- | --- |
| `foundry/artifact/source_port/scope.rs` | Root mask downset, all negative powers unbounded; rejects arbitrary finite root bounds | Add explicit entry/proof scope types; do not reinterpret existing mask semantics |
| `source_port/mod.rs::check_sector_with_observer` | Replay and descent use full candidate application; two whole-sector cover checks | Clip retained applications to checked `P` before expensive proof; keep every original input's structural admission and whole exclusions |
| `source_port/predicate_cover.rs::certify_predicate_cover` | Boolean branches subtract owners from the entire orthant | Add explicit required-box union; preserve one aggregate traversal/consistency budget across all slices |
| `source_port/program.rs::CheckedProgram` | Lowers every retained full-domain rule, installs whole-sector coverage | Carry the checked scope; lower clipped domains and recheck successor closure after exact RHS pruning |
| `artifact/install/source_port.rs` | Independently checks complete unbounded cover | Verify entry containment, coverage of `P`, and successor containment; never trust saved scope evidence |
| `artifact/persistence/source_port/{plans.rs,../source_port.rs}` | Stores root mask and original parent/cell recipes | Version scope grammar and strictly decode limits; cold replay the same inductive obligations |
| `artifact/model.rs::ClosedArtifact` | Rectangular root bounds only | Persist an exact optional sum-bound alongside the rectangular prefilter and declared proof scope |
| `reduction/reducer.rs::validate_target` | Checks rectangular root bounds | Also check exact entry degree before reduction/cache lookup; descendants are not entry-checked |
| `scalar_numerator/lowering.rs` | Checks lowered scalar keys against root bounds | Check every lowered input against the same exact entry predicate |
| App `application/candidate_bundle/{model.rs,certify.rs}` | Separately decodes candidates and certifies them; no rank option | Add optional semantic certification scope, distinct from resource limits; generation unchanged |

`ClosedArtifact::supported_root_power_bounds()` already documents that
descendants can leave the root rectangle. Preserve that distinction. Its
rectangle can remain a cheap prefilter, but cannot encode a sum simplex alone.
Keep the existing `ClosedArtifact` authority boundary rather than introduce a
parallel “almost closed” public artifact.

`SourcePortLimits` and `ArtifactLoadLimits` are caller-owned resource policy,
not the natural home of a semantic rank restriction. A scope changes what is
proved and must survive serialization; an operation allowance does not. Old
artifacts and `None` requests must retain current behavior. Adding semantic
fields requires explicit format/report versioning and CLI/Python parity, not a
silent change to the current durable grammar.

## 6. Bounded implementation sequence

### A. Internal geometry and requested-domain coverage — implemented foundation

Own a new `source_port/scope/rank.rs` and tests, with a small `scope.rs` module
declaration. Provide checked exact entry membership, deterministic finite
negative slices, and scope intersection/containment helpers using existing
`LatticeBox`/`BoxCover`. Add `certify_predicate_cover_within` in
`source_port/predicate_cover.rs`; the existing function passes the full
orthant. Reuse the same traversal, admission, diagnostics, and work accounting;
do not obtain a fresh consistency allowance per slice.

Required tests: degree zero; all-active and all-inactive sectors; sum rather
than per-axis bound; permutation invariance; genuine infinite active rays;
inside/outside holes; disjoint/overlapping required boxes; `i64::MIN`; checked
binomial/coordinate overflow; exact budget boundaries; malformed late owners
cannot hide outside the required scope. Existing unbounded tests must remain
unchanged. This increment is infrastructure, not a public rank feature.

### B. A coherent uniform scoped certificate

Add an explicit entry bound and caller-proposed finite proof envelope. A useful
initial convenience chooses `P=E_D`; it must fail on any live successor escape.
Allow a larger independently checked envelope without requiring the user to
assert its truth. Integrate clipping, original replay, coverage, successor
containment, cold persistence, public entry checks, inspect reports, and
second-stage CLI/Python options together. Preserve all source conditions and
pole guards even for algebraically canceled contributions.

Required tests: generated K1/K3 roundtrip; omitted default stays byte/semantic
compatible where the chosen grammar permits; boundary input accepted and
degree `D+1` rejected; very large positive powers not rejected by rank; a
descending rank-increasing rule cannot publish under `P=E_D`; a larger valid
`P` accepts the same program; a rank-increasing cycle requires an explicit
failure or full-sector proof; tampered entry/proof scope, missing successor
slice, missing AND sibling, and removed pole all fail cold validation. Include
nonempty affine slices and exact empty/excluded slices from FG115/H229.

### C. Bounded envelope proposals, only after B

Add deterministic missing-image expansion and optional full-sector widening.
Reuse all verified cells that have identical original domain/context and
resource admission. Never reuse a proof merely because rank or box dimensions
match. Cache exact immutable inputs under combined memory/work limits. Stop
with an explicit resource or unresolved-closure result, never a truncated
artifact. Modular probes may propose useful envelopes or small witnesses, but
all final domain and polynomial claims remain exact.

## 7. Performance expectations and acceptance measurements

For `k=3` inactive indices, degrees `D=0,1,2,4` need `1,4,10,35` slices in one
sector. At `k=6,D=4`, the count is `210`; at `k=10,D=4`, it is `1001`. These
counts explain both the likely benefit for low-rank physical parents and why a
large bound or many low sectors can erase it. They are combinatorial estimates,
not measured speedups.

The expected gains are fewer relevant exceptional branches, exact singleton
specialization before native algebra, fewer Boolean atoms after admitted
simplification, and omitting rules whose applications provably miss `P`.
The risks are repeated parent replay across slices, combinatorial slice count,
shared-budget exhaustion, and enlargement to nearly the full original domain.
Initially share authenticated immutable parent preparation; do not multiply
source regeneration by the slice count. Retaining a global atom set may give
less speedup than per-slice atom preparation, but avoids changing admission and
budget semantics in the first implementation.

Benchmark identical saved candidate bundles at `None`, then low degrees
`0,1,2,4`, separately measuring decode/preparation, scope planning, replay,
guard/descent, coverage/inductiveness, encoding, and fresh cold load. Record
slice counts, retained rules/cells, native work, peak memory, failed escape
witnesses, widened sectors, and exact terminal manifests. Compare all common
in-scope canaries and homogeneity checks. A faster failed certification is not
a closure speedup; a smaller entry promise is not equal workload to unbounded
certification.

## 8. Completeness boundary

Finite negative slices remove some variables, not the infinite positive rays.
The remaining polynomial guards can still be nonlinear, and small operational
budgets can still fail. Rank bounding alone does not make the whole problem
Presburger, provide a finite enumeration of all starts, prove a master basis,
or guarantee existence of an invariant expressible as finitely many boxes.

If each slice's obligations lie in a defined complete affine-integer fragment,
and a finite inductive envelope in the accepted representation is supplied,
then an uncapped complete checker for that fragment can decide those finite
obligations. The present native-backed bounded checker is sound but does not
yet have that completeness guarantee. The recommended optimization improves
the proof problem's size while preserving these honest boundaries.
