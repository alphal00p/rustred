# Sector-local coordinate charts as a K6 search preconditioner

## Status and scope

This note records a bounded research proposal for the three-loop, six-coordinate
equal-mass vacuum family. It does not change RustRed's closure criterion, artifact
authority, or complete-family coverage obligations.

The proposal interprets the user idea in its strongest sound form:

1. take each useful three-loop Vakint representative as a search seed;
2. retain its surviving physical propagators;
3. complete them to a natural sector-local scalar-product basis with ISPs;
4. discover compact source portfolios or relations in that local chart;
5. transport only finite, exactly checked proposals into the parent K6
   coordinates; and
6. regenerate and replay the nominated ordinary sources in the parent K6 family
   before any rule can acquire ownership.

The local chart is therefore a cold proposal and preconditioning mechanism. It
is not a second integral family for publication, a new source of IBP identities,
or an alternative artifact authority.

## Matcher roots and complete-family obligations

Vakint registers five user-facing three-loop matcher roots. In stable K6 slot
order and after the authenticated `S4` canonicalization, they are:

| matcher root | stable active slots | canonical sector |
| --- | --- | --- |
| `I3L` | `[1,1,1,1,1,1]` | `[1,1,1,1,1,1]` |
| `I3L_pinch_6` | `[1,1,1,1,1,0]` | `[0,1,1,1,1,1]` |
| `I3L_pinch_1_6` | `[0,1,1,1,1,0]` | `[0,1,1,1,1,0]` |
| `I3L_pinch_3_6` | `[1,1,0,1,1,0]` | `[0,0,1,1,1,1]` |
| `I3L_pinch_1_3_6` | `[0,1,0,1,1,0]` | `[0,0,1,0,1,1]` |

These witnesses live in
[`crates/rustred-core/src/foundry/artifact/three_loop/manifest.rs`](../../crates/rustred-core/src/foundry/artifact/three_loop/manifest.rs).
They are sectors of one complete K6 family, not five independent families.

The maximal K4 root generates all 64 physical contraction masks. Under the
authenticated `S4` action, those masks form five zero/scaleless orbits and six
full-loop-rank closure orbits. Vakint's roots omit the four-member star orbit
represented by `[0,0,1,1,0,1]`. That orbit is not a missing interacting graph
class: the installed unimodular factorization proves it is a `K1^3` product.
Consequently:

- the five roots are useful routing, chart, and acceptance fixtures;
- they do not define publication coverage by themselves;
- together with the installed zero and factorization authority, they do cover
  every one of the 64 physical sector masks (34 matcher-seeded, 26 scaleless,
  and four in the omitted factorized star orbit);
- the omitted star orbit is discharged by its structural product action, not
  by pretending that it was one of Vakint's matcher roots; and
- the final artifact must still close every dotted and numerator-bearing
  lattice branch above that complete contraction downset.

This distinction is enforced by
[`CompletePhysicalContractionGoal`](../../crates/rustred-core/src/foundry/completion/family_campaign/model.rs),
which cannot be constructed from an arbitrary list of matcher roots.

Thus the strong form of the matcher-root proposal is now the preferred Stage-1
schedule: compile all five roots as internal local charts of one K6 campaign,
then let zero/factorized structural owners handle the complementary masks. It
removes any need to discover a sixth nonfactorized graph topology. It does not
by itself prove recurrence closure, because a finite set of graph corners is
not the infinite index lattice above those corners.

## Concrete S5 chart

Use the canonical K4 denominators from
[`three_loop/family.rs`](../../crates/rustred-core/src/foundry/artifact/three_loop/family.rs),
with scalar-product coordinates

```text
(s11, s12, s13, s22, s23, s33).
```

For the stable-slot five-line sector with `D6` pinched,

```text
D2 = s22 - 1,
D3 = s33 - 1,
D6 = s22 + s33 - 2 s23 - 1.
```

The surviving physical rows `D1,...,D5` can be completed by the local ISP
`s23`. The exact bidirectional affine relation is

```text
s23 = (1 + D2 + D3 - D6) / 2,
D6  = 1 + D2 + D3 - 2 s23.
```

Thus a nearest-neighbour walk in the local `s23` numerator degree corresponds
to a structured multinomial cloud of translations in the parent coordinates.
A bounded local solver may see a sparse one-dimensional recurrence even when a
parent-coordinate scheduler would have to select the whole correlated cloud
from thousands of admissible source translations.

The same effect is potentially stronger in a four-line sector, where two local
ISPs can change source width, pivot support, guard factorization, and sparse
fill. It is absent algebraically in the six-line sector, where no ISP completion
is needed and a chart can differ only through routing, permutation, and order.

## Why charts can help without adding identities

Every complete three-loop vacuum chart has six independent affine scalar
coordinates and the same complete ordinary source module of nine rows. An
invertible chart change therefore cannot add an IBP identity.

It can nevertheless change the cost of a bounded discovery algorithm because
all of the following are coordinate dependent:

- the number and width of translated-source terms;
- the shape of a small source neighbourhood;
- the leading integral under a chosen ordering;
- modular pivot support and sparse fill;
- the visible factorization of coefficient guards; and
- which correlated source batch is reached by a finite upward walk.

An exhaustive parent-coordinate enumeration would eventually contain the
transported local source span. RustRed does not perform such an unbounded
enumeration: it selects a resource-bounded proposal frontier. Local charts may
therefore act as exact structural preconditioners for that selection problem.

The expected priority is:

1. **S4a, nonfactorized four-line sector:** highest-value chart experiment;
2. **S5, one missing coordinate:** high-value and simplest exact prototype;
3. **S4b, factorized `K3 x K1`:** diagnostic or fallback only, after the
   structural factorized-product owner;
4. **S6, no missing coordinate:** control arm for routing and ordering effects.

## P0 transport constraints

### Integral indices do not transform linearly

Let complete local and parent affine bases satisfy

```text
C = a + M D,
D = b + N C.
```

This is an exact linear relation between affine denominator functions. It is
not a linear map between integral-index vectors.

If a local auxiliary coordinate occurs with a nonpositive integral power, it
is a polynomial numerator and fixed powers can be expanded finitely. If it
occurs with a positive power, the integrand contains a spurious pole
`1 / C_aux^r`; a general affine `C_aux` cannot be represented by a finite sum
of parent K6 integral keys. Such a proposal must be rejected unless a separate
parent-source replay independently derives the final relation.

### Fixed degree is not an arbitrary-rank rule

For one fixed numerator degree, affine substitution has finite support. For a
symbolic exponent `N`, the support of `(a + sum_i M_i D_i)^N` depends on `N`.
Transporting several fixed integer samples therefore does not create a single
finite-shift parametric rule valid for all ranks.

Fixed samples may reveal a parent source portfolio or recurrence pattern. The
parent foundry must still lift and replay the corresponding parametric circuit,
including its guards and all-rank application domain.

### Local authority is foreign to K6

Translated sources retain their family and indexed-context fingerprints in
[`identity/generator/translated_source/model.rs`](../../crates/rustred-core/src/identity/generator/translated_source/model.rs).
Canonical replay checks the parent generator, maximal stratum, and immutable
owner snapshot identities in
[`source_discovery/canonical_replay/build.rs`](../../crates/rustred-core/src/foundry/completion/source_discovery/canonical_replay/build.rs).

A local `RuleCell`, terminal, closed cover, or artifact must never be inserted
directly into the parent snapshot. Only a fresh parent-bound exact circuit may
enter the existing promotion path in
[`source_discovery/promotion/admit.rs`](../../crates/rustred-core/src/foundry/completion/source_discovery/promotion/admit.rs).

### Source provenance must survive the boundary

Transporting only the final equality is insufficient for RustRed publication.
The proposal must retain enough local source provenance to nominate parent
ordinary-source translations. Parent K6 rows are then regenerated, joined to a
fresh physical plan, exact-lifted, and replayed. Local lower-sector or terminal
claims do not cross this boundary; the parent immutable snapshot reclassifies
all endpoints.

## Topology-neutral chart capability

A minimal capability, provisionally named `SectorLocalCoordinateChart`, should
be compiled from structural data only:

- one parent `FamilyPresentation`;
- one canonical sector and its stable physical-slot embedding;
- one exact unimodular loop-routing candidate;
- a deterministic ISP completion policy; and
- explicit algebra, expansion, storage, and work limits.

It must not dispatch on `I3L` or pinch names. Vakint routes may seed tests and
the bounded portfolio, but the compiled capability is identified by exact
family, sector, routing, and affine content.

The retained immutable value should contain:

- parent and local family/context fingerprints;
- the sector and physical/auxiliary slot roles;
- the signed loop map and determinant;
- the deterministic ISP-completion witness;
- both augmented affine matrices `C -> D` and `D -> C`;
- a stable chart fingerprint and process-local capability token;
- coefficient-domain and common-mass metadata; and
- exact resource telemetry for construction and transport.

[`IspCompletion`](../../crates/rustred-core/src/family/isp/completion.rs)
already supplies deterministic exact rank completion, but it does not retain a
map back to a parent family. [`MomentumRouting`](../../crates/rustred-core/src/family/presentation/model.rs)
records structurally checked, caller-attested routing and is not by itself an
exact cross-family proof.

The Symbolica-backed affine construction and replay in
[`factorized_numerator_lift/compile.rs`](../../crates/rustred-core/src/foundry/artifact/factorized_numerator_lift/compile.rs)
is the relevant implementation pattern: exact matrix inversion, congruence,
affine relation construction, and independent relation replay. Fixed
polynomial expansion should similarly use Symbolica's native sparse polynomial
arithmetic rather than a RustRed multinomial CAS.

## Proof obligations

Chart construction must prove, once at its cold admission boundary:

1. the parent family, presentation, coefficient field, dimension, metric, and
   common-mass convention agree with the claimed scope;
2. the loop map has determinant `+1` or `-1`, so it preserves the integration
   measure;
3. routed physical rows replay exactly to their stable parent slots;
4. the completed local rows have full scalar-product rank;
5. the two augmented affine maps compose to the identity in both directions;
6. generated auxiliaries have explicit inactive roles and homogeneous mass
   dimension;
7. all coefficient denominators contribute exact nonzero conditions;
8. symmetry-equivalent charts and endpoints canonicalize deterministically; and
9. no topology label contributes mathematical authority.

Every finite transport must additionally prove:

1. admitted integer exponents and checked exponent arithmetic;
2. a preflighted bound on sparse terms, exponent cells, coefficients, endpoint
   keys, retained bytes, and canonicalization work;
3. exact Symbolica expansion followed by deterministic coefficient coalescing;
4. absence of positive local auxiliary denominators in every materialized
   endpoint admitted for finite transport; and
5. exact mass homogeneity, with final common-mass restoration left to the
   parent reducer.

Every promoted result must finally prove, in parent coordinates:

1. exact membership in regenerated K6 ordinary sources;
2. exact target normalization and coefficient/guard replay;
3. strict descent under the parent ordering;
4. parent symmetry and lower-owner routing;
5. complete exceptional-domain refinement; and
6. a strict shrink of the authenticated parent complement.

## Staged falsifier

### Stage A: diagnostic chart portfolio

Compile identity-routing and structurally admitted natural charts for the S4a,
S4b, S5, and S6 representatives. For the same target witnesses and resource
budgets, record:

- local and parent source-row term counts;
- affine row width and coefficient complexity;
- local physical columns and sparse fill;
- transported parent request count and source degree;
- modular rank and exact-lift outcomes;
- wall time and measured peak memory; and
- deterministic content across supported worker counts.

This stage has no owner, terminal, or artifact output.

The first bounded Stage-A fixture is now executable. It derives the five
frozen Vakint roots from the single complete K6 contraction plan and applies
each exact unimodular witness as `q = T k`. Symbolica's checked inversion and
congruence boundaries route the surviving parent propagators with
`T^-T Q T^-1`; deterministic ISP completion then occurs in those routed local
coordinates. Every completed row is routed back with `T^T Q_local T`, converted
to the parent denominator basis by Symbolica, and replayed independently. The
physical rows must be exact unit images of their original stable parent slots.
All five resulting six-coordinate families retain foreign fingerprints.

The roots cover 34 raw masks in five of the six full-rank `S4` orbits. Installed
zero authority covers 26 more, and the four-member omitted star orbit
`[0,0,1,1,0,1]` is an installed `K1^3` factorization, accounting for all 64
physical masks without inventing a sixth interacting chart. This finite corner
census is not recurrence closure. The S5 completion selects `s23` and retains
the exact parent relation `2 s23 = 1 + D2 + D3 - D6`. A bounded admission seam
accepts only fixed nonpositive auxiliary powers under a caller-supplied work
limit and refuses positive auxiliary poles before expansion; that limit is not
a semantic rank cap. The fixture still does not transport a source, compare
sparse fill, or demonstrate a parent nomination improvement; those measurements
remain the substantive Stage-A/B gate.

### Stage B: finite fixed-sample transport

The audited minimal seam needs only local-to-parent transport for concrete
integer samples; a bidirectional affine inverse may improve later sample
selection but is not required for the first falsifier. For every local sample
`s`, request all nine completed local rows translated by `s`, then specialize
their indexed coefficients and conditions at the zero assignment. Translation
already changes `c_delta(n)` into `c_delta(n+s)`, so specializing again at `s`
would be an incorrect double translation.

Specialize and remove exact-zero terms first. Transactionally refuse a row if
any surviving endpoint has a positive auxiliary power. Expand every remaining
nonpositive auxiliary power with Symbolica sparse-polynomial `pow` and native
multiplication over its retained affine parent relation; a monomial `D^e`
shifts the stable parent physical key to `b-e`. Coalesce the whole transported
row exactly. Extract the existing single-affine Symbolica pattern into a
topology-neutral multi-affine primitive rather than implementing a RustRed
multinomial CAS. Reject exponent overflow, unsupported coefficient domains,
and every resource excess with typed outcomes.

For a canonical blind target, first choose one deterministic orbit image in
the chart's raw sector. Canonicalizing that raw target yields one authenticated
raw-to-canonical group route; apply that same route to every expanded endpoint.
Independently canonicalizing endpoints would destroy the correlated equality.
Pin the S5 identities above at auxiliary degrees `0,1,2,4` as the first exact
fixture.

### Stage C: sealed proposal adapter

Add a cold `source_discovery::chart_proposal` seam that returns only:

- the chart capability and fixed-sample provenance;
- canonical parent support;
- nominated parent `TranslatedSourceRequest`s; and
- complete resource telemetry.

It must be unable to construct a `RuleCell`, owner, terminal, closure layer, or
artifact.

The proposal adapter specializes complete local rows but discards their
coefficients after exact row-wide coalescing. Only nonzero canonical parent
support reaches the parent ordinary-source incidence index. Local family
fingerprints, indexed contexts, row IDs, coefficients, and guards remain
diagnostic and never become parent evidence.

### Stage D: parent replay

Merge the nominated requests with the ordinary target-unit bootstrap at the
start of each independent probe. Materialize every request with the parent K6
generator, rebuild one fresh parent physical frame, and use the existing
exact-lift, canonical replay, guard-refinement, and promotion path. No modular
value, accumulated row weight, or foreign chart state crosses probes. A chart
is useful only if this parent path produces an exact replayed owner.

If support-only proposals win decisively but repeat coefficient discovery is
expensive, a later sealed proposal may carry exact suggested source weights.
Those weights remain untrusted until an independent parent-row sum reproduces
the complete circuit.

### Stage E: promotion gate

Promote the experiment beyond research only if, under identical limits, at
least one chart-derived request portfolio:

1. produces a parent-replayed exact circuit not reached by the baseline;
2. strictly shrinks an authenticated S4a, S4b, S5, or S6 complement;
3. retains exact deterministic replay under multiple independent probes;
4. adds no chart-specific runtime action to the hot reducer; and
5. leaves complete K6 coverage and transactional wave publication unchanged.

Failure to beat the baseline is a clean falsification of this proposal lane,
not evidence that the sector has no closing relation.

## Acceptance matrix

The smallest useful regression matrix is:

| case | required result |
| --- | --- |
| S5 `D6 <-> s23` | both affine maps replay exactly |
| fixed numerator degrees `0,1,2,4` | exact finite round-trip and deterministic coalescing |
| positive local auxiliary power | typed refusal, no partial output |
| non-unimodular loop route | construction refusal |
| symmetry-equivalent routes | one canonical proposal content |
| foreign family/context | parent replay refusal |
| local closed rule without parent replay | no publication capability |
| chart-nominated exact parent hit | ordinary-source replay and normal promotion checks |
| chart-nominated miss | typed inconclusive result, no negative closure claim |
| S6 control | no claim of new identities; report ordering/routing metrics only |

## Explicit non-claim

This proposal does not establish a closed K6 artifact, closure of any four-,
five-, or six-line wave, arbitrary-rank cross-chart transport, three-loop
Vakint parity, or scaling to six loops. It does not permit treating matcher
roots as a coverage manifest or shipping ISP-completed pinch artifacts beside
the parent family.

Its sole claim is testable and narrower: a sector-local completed coordinate
chart may expose a sparse, correlated ordinary-source proposal that a bounded
parent-coordinate walk fails to select efficiently. Exact parent K6 replay
remains the only path from that proposal to an executable closing rule.
