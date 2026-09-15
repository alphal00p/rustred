# From the SpIRed vacuum source port to a shipped Vakint K6 artifact

## Goal and current boundary

Turn RustRed's independently generated, complete `vac3` reference workload
into a cold-loadable, reusable K6 closing artifact, then ship it with Vakint's
opt-in RustRed scalar backend. Vakint must use its existing FeynKit tensor
prepass, topology match and routing witness, and pure-Rust master evaluation.
Ordinary evaluation must neither generate IBPs nor invoke FORM.

This is a parallel integration lane; it does not replace the ongoing generic
source-port work. The current source-port output is **not yet a certified
closing artifact**. Matching a complete C++ workload and publishing a complete
mathematical reduction program are separate milestones.

The implementation should extend the existing immutable `ClosedArtifact` and
memoized `Reducer` ownership, not introduce a second legacy artifact wrapper.
RustRed schema compatibility is not required. Preserve Vakint's existing
defaults, public conventions, and FORM-backed methods.

### First implementation checkpoint

The existing `OrderingPolicy` now exposes `SpiredUncutV1` and a persisted
coordinate-priority variant. Concrete comparisons and symbolic shift witnesses
share the same comparison semantics; the original policies and defaults remain
unchanged. Ten new tests include exhaustive small comparisons with the source
port, the actual `001011` counterexample below, extreme runtime indices, and
pinch/activation boundaries. All 76 sector tests and nine existing artifact
persistence tests pass. Independent mathematical/implementation review passes.
These witnesses describe their stated representable domains; the subsequent
whole-ray certificate must retain genuinely unbounded integer endpoints.

The parallel Vakint lane has added five-class tensor/oracle fixtures and an
explicitly pending RustRed peer in its existing comparative harness. An offline
candidate utility prepares exact MATAD-basis records for the 38 proposed finite
corners, avoiding numerical truncation if those projections succeed. Its input
keys, routing and normalization have been independently audited. Neither the
new Vakint tests nor the candidate evaluations have run: compilation is blocked
by an obsolete Spenso forwarding call following the required Symbolica update.
The user has been asked about migrating that unused public wrapper to the new
native API; Vakint's public API would remain unchanged.

The previously linked September 2 MATAD oracle executable does still run:
its nine existing exact raw-master records pass with the installed FORM5.
This is an offline oracle smoke check, not a build of current GammaLoop or a
RustRed acceptance pass. An isolated consumer for the 38 candidate keys could
not link against that preserved library set because its matching transitive
RustRed library had subsequently been overwritten. No cache/fingerprint or
Spenso workaround was applied. Evidence:
`target/vakint-existing-oracle-smoke.IbXhcU/` and
`target/vakint-cached-terminal-oracle.NWFhoN/`.

Its acceptance inventory now maps all 40 legacy test entries (46 input
executions), including aliases and previously missing normalization, epsilon
depth, decorated-index and external-vector variants. New peers reuse the same
comparative harness; two additional basketball cases include the finite part.
Enabled is not synonymous with passed: the new one-loop variant is unrun and
the new three-loop peers remain explicitly pending. The inventory lives in
GammaLoop at `crates/vakint/tests/RUSTRED_ACCEPTANCE.md`; no reference-repository
contents are copied into RustRed's Git history.

No new certified K6 artifact, installed K6 scalar backend, or three-loop
RustRed acceptance pass is claimed by this checkpoint. The cold
replay/descent/cover pass below now succeeds; durable certificate ownership
and installation are the next implementation gate.

### Complete cold rule-set audit (2026-09-15)

`SourcePortAudit` and the autonomous `spired-artifact-audit` Rust example now
perform independent ordinary-source replay, fresh RHS/derivation guard
extraction, zero-sector authentication, whole-orthant coordinate coverage, and
piecewise source-port descent. The diagnostic deliberately cannot mint a
`ClosedArtifact` or supply an installation token. Its sixteen current tests pass,
including corrupted RHS/provenance, omitted exclusions, finite versus infinite
gaps, activation-boundary checks, and inconsistent physical/coefficient seed
translations (including a jointly forged seed and RHS). Workspace checking
also passes.

The complete autonomous release census now passes for all three vacuum
families, at both one and six requested workers:

| Family | Nonzero / proved-zero sectors | Original sources | Rules replayed and descending | Finite terminals | Additional guards / uncovered regions |
| --- | ---: | ---: | ---: | ---: | ---: |
| K1 | 1 / 1 | 1 | 1 / 1 | 1 | 0 / 0 |
| K3, all pinches | 4 / 4 | 4 | 18 / 18 | 4 | 0 / 0 |
| K6 | 38 / 26 | 9 | **617 / 617** | 38 | **0 / 0** |

Every report has zero issues, zero stored-guard and checked-rule gaps, and zero
uncovered unbounded boxes. Semantic reports agree across worker counts. This
verifies the complete rule-set replay/descent/cover; the diagnostic still
does not retain a durable certificate or construct a `ClosedArtifact`.
Canonical unit-mass installation, serialization/cold loading, the five-class
Vakint routing gate and the current acceptance suite remain separate work.

The final correction verifies full **weighted original-source identities**.
Some residual contributions have coefficient `n0*(n0+1)` and shift `n0+2`:
the integral lies in a proved-zero sector for `n0<=-2`, while the coefficient
vanishes on both activation points `n0=-1,0`. Those products become zero only
after combining rows; independent raw-term deletion cannot prove them. The
native exact reducer may propose weights in a projected frame, but both
proposal paths must pass full unprojected multiplication and exact sign-cell
verification. Finite boundary sets are exhausted under a hard budget;
unbounded coordinates remain symbolic. Corrupted weights/sources, either
missing endpoint root, a zero denominator, an infinite nonzero tail and budget
exhaustion all have rejecting tests. No generated reduction rule was changed.

K6 generation plus this independent audit took **2.151 s in-process / 2.18 s
process wall** at one worker, and **0.495 s / 0.53 s** at six workers. Aggregate
CPU times were 2.13 s and 2.27 s; peak RSS was 9,232 and 9,244 KiB. These are
single release observations, not a paired SpIRed benchmark or publication /
cold-artifact-load timing. Evidence: `target/spired-combined-replay.cmaXc8/`.
The initial failed census below is retained as diagnostic history.

The first autonomous full release census completed at both one and six workers:

| Family | Ordinary sources | Rules | Replayed and descending | Finite terminals | Bridge replay failures |
| --- | ---: | ---: | ---: | ---: | ---: |
| K1 | 1 | 1 | 1 | 1 | 0 |
| K3, including all pinches | 4 | 18 | 15 | 4 | 3 |
| K6, all 38 nonzero sectors | 9 | 617 | 387 | 38 | 230 |

Both worker counts produced identical semantic reports. All stored-guard
covers had zero uncovered boxes. The initial checked K6 cover retained 283
boxes (282 unbounded) across 28 sectors because the ordinary-source bridge
could not certify 230 rules. These were **certificate-bridge gaps**, not newly
established failures of those IBPs. All had the same original-frame membership
diagnostic; the smaller K3 pinch failures exposed the mechanism below. The
subsequent coefficient-aware and weighted-product corrections resolve all of
them, as recorded in the current complete census above.

A standalone native-Rust inspection establishes the K3 mechanism. After the
canonical target translation, an original ordinary source contains
`-(n0+1) I(n0+2,n1-1,0)` on `n0<=-1, n1>=2`. The integral is in a proved-zero
sector when `n0<=-2`; at the activation wall `n0=-1` its coefficient is zero.
The product therefore vanishes on the whole admitted domain, although the
integral label alone does not. Native Symbolica also verifies the complete
unprojected preconditioned row equals `2*ordinary0 + ordinary1 - 2*ordinary2 -
ordinary3`, with no remainder. This particular miss is therefore a projection
limitation, not missing source support or an invalid identity. The first bridge used only label-wise zero
projection. The scoped correction must prove product vanishing on every exact
sign cell using native coefficient substitution, with a coefficient-1 negative
control. It does not modify the IBPs or relax their guards. Inspection evidence:
`target/source-port-pinch-inspector.R8Y9qx/canonical-output.txt`.

On this diagnostic run, K6 generation plus the audit took 1.973 s logically
in-process / 2.01 s process wall time with one worker, and 0.494 s / 0.53 s
with six workers. Aggregate CPU times were 1.96 s and 2.10 s; peak RSS was
9,244 and 15,408 KiB. These are single diagnostic observations, not paired
SpIRed performance claims or timings for a completed artifact. Release
compilation preceded the runs. Evidence:
`target/spired-cold-artifact.ckZDsX/` (frozen binary hash and all six runs).

An independent ordinary-source certificate can introduce avoidable poles.
The report therefore keeps stored-guard and certificate-checked coverage
separate, records per-rule failures, and continues the full census. A pole in
one chosen certificate is not proof that its target rule is invalid: obtaining
a regular certificate may remove that obligation. The native polynomial
preconditioner uses exact polynomial quotients and does not divide out pivot
content, so it does not by itself introduce rational poles.

## Evidence already available

The release outputs under
`target/spired-rational-regression.KO3jlH/3-w1` and `3-w6` contain:

- all 38 requested nonzero sectors, with 617 conditional rules;
- 38 fully fixed residual keys, each exactly the 0/1 corner of a nonzero
  sector: 16 three-line, 15 four-line, six five-line and one six-line corner;
- exact native coefficient, target-pattern, guard and sector-sign agreement
  with all 617 reference rules; serial/six-worker mathematical payloads are
  byte-identical;
- nine prepared ordinary sources, symbolic common squared mass `m`, and
  numerical search depth three;
- coordinate cases only for this workload: no exported affine `required`
  condition.

The 26 zero-sector masks and 38 nonzero-sector masks exhaust the 64 masks of
this family. The current cold audit independently authenticates the zero
classifications; the durable construction/loading boundary must retain that
check. A supplied mask list is not a proof that an integral vanishes.

An independent read-only structural audit additionally parsed every target
and excluded condition using only literal coordinate equality syntax. Every
guard is an OR of single-coordinate equations, with roots among `-1, 0, 1, 2`,
or the literal empty exclusion `false`. Partitioning each whole integer
orthant at all target, guard and residual values produced 7,632 exact cells:
7,594 are covered by a rule and the remaining 38 are precisely singleton
residual corners. There are zero uncovered cells in this **stored-guard
structural cover**. Unbounded interval cells were retained as intervals; this
was not a bounded sample-grid test. It does not yet discharge exact replay,
additional derivation divisors, uniform descent or zero-sector proofs.

A finite, nonminimal set of 38 terminals is acceptable. Finding no rule within
the numerical search budget does not prove a terminal independent, but
independence is unnecessary for a reduction onto a specified finite basis.
What must be proved is that every nonterminal target has an applicable exact,
strictly descending rule and that every resulting terminal has a supplied
evaluation. No positive-dimensional residual may be promoted to a terminal.

## Concrete gaps in the existing interfaces

### 1. Source provenance is not exported as an artifact certificate

`solver::RuleCandidate` retains selected `SeedSource` entries. Each entry
identifies a **sector-preconditioned** basis row and its original seed, not an
original ordinary IBP row. The text exporter omits this source support.
`canonicalize` then translates the winning pivot back to the canonical case
target without recording that final translation in `SeedSource`.

The compact support is enough to reconstruct a derivation, but it must not be
mistaken for a directly replayable original-source linear combination. A cold
bridge must regenerate the ordinary sources, reproduce the declared
preconditioning and selected translations, recover or retain the winning
target translation, and check the resulting exact identity against the
stored rule. Text equations alone are not the input to a trusted installer.

Preconditioning preserves the generic fraction-field row span, not every
exceptional specialized rank. Exact replay must respect the specialization
order and every necessary nonvanishing condition. It may use Symbolica's
native exact sparse reduction with source tracking at this cold boundary;
expanded source combinations must not be retained during hot modular search.

The normalized RHS exceptions do not, by themselves, record every divisor of
an exact derivation. If a replay certificate requires a divisor that vanishes
on an admitted integer case, either obtain a regular certificate there or
refine the exceptional domain and supply another rule. Merely clearing such
a divisor does not prove the divided identity on its zero locus. Whether
this adds any actual vac3 branches must be established by the first full
certificate pass, not assumed from the reference comparison.

### 2. The source-port ordering is genuinely different

Existing `sector::OrderingPolicy` uses ascending corner distance, then dots,
then numerators, followed by ascending coordinate excess. The source port
uses total absolute degree, then numerator degree, and the opposite
denominator-coordinate tie orientation; denominator and numerator tie passes
are also separated.

For example, the generated `001011` rule with target
`I(0,n1,1,n3,1,1)` and guards `n1 != 1`, `n3 != 0` contains

```text
(1/2) I(-1,n1,1,n3+1,1,1).
```

At `n1=-12`, `n3=-14`, this child is lower under the source-port ordering but
higher under the existing artifact ordering. Changing the coordinate
permutation alone is not a valid conversion.

Add an explicit persisted source-port ordering policy, including its
coordinate priority. For the initial uncut vacuum lane, its simpler-first
key is: active count, sector mask, total absolute degree, numerator degree,
then reversed positive-index values in priority order, then nonpositive-index
values in priority order. Avoid arithmetic negation of `i64::MIN` when
implementing reverse comparisons. The aggregate absolute degree has finite
level sets, so the coordinate tie reversals do not destroy well-foundedness.

Extend the existing concrete and symbolic shift-descent witnesses together.
Do not merely change the runtime comparator: the installer must prove the
same persisted order uniformly on each admitted guard cell.

### 3. Installation currently accepts other proof shapes

The existing installer is registered to the established K1/K3 proofs and the
older K6 sector-wave publication path. The latter expects its own wave
structure, factorization programs and terminal basis. A successful
`SectorSolution` cannot be inserted through those constructors unchanged.

Add a source-directed coordinate-program installation path under the same
`ClosedArtifact` ownership. Reuse family/context binding, exact rule and cell
types, source replay, zero analysis, geometric cover machinery, immutable
ownership, serialization and the reducer. Do not synthesize a fictitious old
K6 wave transcript or require the old minimal terminal list.

### 4. Family routing and common-mass normalization must be explicit

The matched C++/Rust reference fixture uses momenta

```text
[k1, k2, k3, k1+k2, k1+k3, k2-k3],    D_i = q_i^2 - m.
```

The existing unit-mass K6 artifact family uses

```text
[q1, q2, q3, q3-q1, q1-q2, q2-q3],    D_i = q_i^2 - 1.
```

They are related by the unimodular routing
`(q1,q2,q3)=(k1,-k2,-k3)` and interchange of denominator slots three and four
(zero-based). Authenticate this exact map if importing the existing family;
alternatively generate directly from the canonical unit-mass family and
validate that fresh campaign. Never identify families by a topology name.

Use native exact substitution for `m=1` and coefficient-context remapping.
For a general common squared mass `M2`, retain the existing homogeneity rule:
each master coefficient receives
`M2^(sum(master powers)-sum(target powers))` in addition to the common loop
normalization handled by Vakint. Negative numerator powers participate in
these sums. Do not confuse the reference's symbol `m` with an unsquared mass.

### 5. Vakint needs K6 assets and a complete terminal catalog

Vakint currently embeds K1/K3 assets, validates them once in `LazyLock`, and
passes a `ClosedArtifact` to `Reducer`. Its matching adapter does not yet
admit the K6 family. Preserve this load-once architecture.

The scalar contract remains exact coefficients keyed by typed `IntegralKey`
masters. All 38 finite corners may initially be retained. They can be mapped
offline to the existing MATAD master basis, or supplied through the existing
numerical Laurent terminal catalog with sufficient epsilon depth. An oracle
may evaluate this finite catalog during development; production generation,
loading and scalar application must not invoke the FORM reduction.

## Concrete installer slice: retained source certificate to existing owner

This is a proposed implementation boundary, not a claim that the current cold
diagnostic can already install or reload an artifact. Complete the remaining
ordinary-replay obligations first. Public audit counters are diagnostic data;
they must never be accepted as a publication token.

### One cold proof owner, not another reducer

Add a private `ReplayCertifiedCoordinateProgram` in
`foundry/artifact/source_port/certificate.rs`. It owns the authenticated
family/indexed context, original-source recipe, deterministic sector/rule order,
exact application cover, finite terminal keys, zero-sector certificates and
per-rule original-source replay records. Its fields and constructor remain
private to this cold bridge. A successful replay/descent/cover verifier returns
this consuming owner; unresolved guards, any uncovered complement (including
finite points not explicitly admitted as terminals), or budget failures cannot
construct it. It is neither a public `SectorSolution`
wrapper nor a second `ClosedArtifact`.

Per-rule records retain the exact original `RowId`, canonical target-relative
`i64` translation, normalization scale and native rational weight together
before filtering zero weights. They also retain the coordinate face and full
RHS. Fixed coordinates are specialized only after source translation; numeric
seed values need not equal the fixed target values. No preconditioned basis
ordinal, compact `Power`, GPLU state or search transcript is required in the
durable certificate.

The source adapter clears denominators using a native polynomial LCM. A weight
of that cleared row is not automatically a weight of the generator's original
`ParametricRelation`: preserve the clearing multiplier, or convert the weight
by that multiplier and verify the resulting exact relation. Source-condition
and all weight/RHS denominator obligations stay attached to the certificate.
Regenerating the same source rows and their scales must reproduce every
retained request; an ordinal without its `RowId` is insufficient.

### Combined-remainder semantics

For original translated rows `R_j`, normalized target identity `T`, and weights
`w_j`, the authority check computes the complete physical residual
`sum(w_j R_j) - T`, combining coefficients of equal integral keys with native
Symbolica arithmetic. No term is deleted first. For each remaining combined
term and every exact physical sign cell of the application cover, prove either
that its integral belongs to an independently certified zero sector or that
its coefficient vanishes identically after the cell's exact restrictions.
The target is never discarded merely because a discovery projection hid it.

An individually proved-product quotient is the cheap discovery lane. A more
permissive assumed-sector quotient may propose weights only after that lane
misses; the same full-residual check authenticates both. Finite activation
intervals may be exhaustively split into singleton integer coordinates under
an explicit budget. Every unbounded coordinate remains symbolic. Checking all
points of the exact two-point set `{-1,0}` is exhaustive; testing those points
on an infinite ray would not be. Zero numerators with identically zero
restricted denominators are rejected. Fresh applicability guards are checked
independently of serialized exclusions.

This proof sometimes exists only after weighted cancellation: individual raw
source terms need not vanish. Therefore do not encode such a certificate as
independent `ResidualTermDisposition` deletions. Extend the existing
`SourceViewConstruction` in `foundry/cell/model.rs` with one combined-identity
domain-quotient evidence variant. It retains unmodified translated source
relations, fixed-coordinate restrictions and the recomputable combined
remainder obligations. Existing cell/installer validation must recognize this
semantic variant explicitly. Do not disguise it as `Direct`, fabricate a
symmetry canonicalizer, or re-use the existing per-term residual projection
proof with a weaker meaning. The evidence need not store a large expanded
sign-cell tree: bounded cold verification can deterministically reconstruct
the partition from the domain and physical shifts.

### Lowering and runtime

Add `source_port/lower.rs` and a narrow consuming installer in the existing
`artifact/install.rs` boundary. The lowerer uses the certified program to
construct existing `ParametricRule`, `SourceViewBatch` and `Arc<RuleCell>`
payloads. Extend the private exact-replay construction seal to admit this
verified producer, preferably moving/renaming the current circuit-specific
`ExactCircuitLoweringSeal` to a neutral exact-replay boundary shared by both
producers. Never expose a public unchecked witness constructor or manufacture
an old physical-frame/wave transcript just to obtain the seal.

Compile guard-complement boxes with the existing unbounded `BoxCover` service.
For runtime rule cells, split at RHS sign walls and exact coefficient-dead
faces as needed to build the existing sector-monotone descent witnesses.
The full unbounded cover remains the publication proof; checked machine-index
bounds only delimit representable runtime inputs. Preserve deterministic
first-applicable rule priority and share immutable source/certificate data
across derived cells rather than cloning coefficient graphs for every cell.

The new installer reuses generic family/context, ordering, terminal,
zero-sector and rule-cell checks, then seals the same `ClosedArtifact`.
Initial ownership needs no factorization dependency or minimal-master proof:
the 38 finite parent-family corners can be explicit masters. The existing
`reduction::Reducer` continues to specialize coefficients, descend, memoize
and return exact typed-master decompositions. No certificate replay, source
generation or search enters its hot path. Sector-indexed dispatch may be
compiled once later if profiles justify it; the initial change need not add
another application engine.

### Canonical family and coefficient field

For the first shipped program, prefer generation directly from Vakint's
canonical unit-mass family. If transporting the already generated reference
program instead, authenticate the unimodular loop routing and denominator-slot
permutation described above, transform indices/guards/source requests/order
together, substitute the squared mass `m=1` natively, and rerun the full cold
proof against freshly generated canonical-family sources. Never permute only
the RHS or leave its ordering in the old slot convention.

Use the existing indexed coefficient context and native variable-map remapping
for the canonical field, not textual symbol replacement. Reject a denominator
that becomes zero during unit-mass specialization. Preserve generic dimension
dependence and explicitly retain parameter-locus conditions. Attach the
existing homogeneity proof so application restores a general common squared
mass with `M2^(sum(master powers)-sum(target powers))`, including negative
numerator powers. Runtime integral indices remain checked `i64` values.

### Codec, files and focused acceptance

Extend `artifact/persistence/mod.rs` and a cohesive
`artifact/persistence/source_port.rs` grammar under the current owner. Bump
`ArtifactSchemaVersion::CURRENT` from v4 to v5 and add one source-directed
algorithm identifier; update K1/K3 assets coherently rather than retain schema
compatibility shims. Reuse the existing bounded binary/coefficient codecs and
canonical content identity. Serialize family/context, source recipe and
normalization, exact weights/RHS, cases/guards, ordering, terminals and mass
metadata. Derived caches, native reducer arenas and validation timings are not
durable data.

Cold decode reconstructs native coefficients and original sources, verifies the
stored weighted identities and their exact domain/guard/descent/cover proofs,
and seals once. It must not rerun modular discovery, preconditioning search,
case solving, Janet completion or the reference executable. Invalid payloads
fail at this boundary; ordinary Vakint evaluation loads the immutable owner
once and never regenerates a campaign. New parser resource limits remain
explicit, but do not repeat expensive authentication per integral.

Required tests for this slice are original-row/offset/scale/weight mutation
rejection; full-remainder cancellation and finite-boundary negative controls;
guard and zero-census mutation; empty versus finite versus infinite cover;
canonical routing and native context remapping; mass-one specialization poles;
strict descent under the persisted ordering; deterministic serial/six-worker
bytes; fresh-process encode/decode plus canary reduction; memoization and
master-only output beyond the search encoding's compact range; nonunit-mass
restoration; and a decoder test proving no source finder was invoked. After
these pass, ship the asset and exact 38-key terminal catalog with Vakint and
run the separately maintained complete acceptance matrix. Each small lowering,
codec and application slice receives an independent audit before publication.

## Proposed bounded implementation ownership

Keep new work outside `solver`'s hot path:

1. `sector/ordering` and its shift/monotone witnesses: add the persisted
   uncut source-port ordering, exact concrete comparisons and uniform
   piecewise descent support. Retain the existing ordering variants for
   existing artifact producers; do not add wire-format compatibility shims.
2. `foundry/artifact/source_port/`: one cohesive cold bridge, split into
   provenance/replay, coordinate-cover compilation, installation and tests.
   It consumes typed sector solutions and authenticated family metadata.
   Tiny source-port accessor/provenance additions, if unavoidable, require
   root coordination; do not duplicate its search algorithm here.
3. Existing artifact model/codec: record the new algorithm and ordering,
   exact source provenance, case/guard ownership, finite terminals, zero
   certificates and homogeneity; bump the evolving schema as appropriate.
4. Existing `reduction` module: reuse iterative descent, memoization,
   coefficient specialization, typed masters and homogeneity. Prefer a
   sector-indexed rule dispatch table compiled once over scanning all 617
   rules for every target. Any indexing optimization must preserve declared
   deterministic rule priority.
5. Examples/campaign generation: an autonomous Rust-driven K6 artifact
   generator using family input only. Keep the old text example as a
   diagnostic interface, not an alternate trusted artifact reader.
6. GammaLoop/Vakint lane, owned separately: ship the resulting immutable
   K6 asset and terminal catalog, extend existing matching and comparative
   tests, then pin the corresponding RustRed revision at publication.

The exact public bridge function name may follow the existing artifact API,
but its semantic contract is:

```text
authenticated family + source recipe + complete sector solutions
  -> regenerate/replay + guarded descent + exact cover
  -> ClosedArtifact

ClosedArtifact + integer IntegralKey
  -> existing memoized Reducer
  -> exact typed-master decomposition
```

No artifact/application API may inherit the search port's compact `Power`
range. Runtime powers use the existing checked `i64` integral carrier, with
typed overflow errors and explicitly documented representability bounds.
The mathematical cover describes whole integer rays, not a finite test grid.

## Closure certificate for the first coordinate-only vacuum bridge

For each sector, form the complete integer parent orthant. Compile every
rule's fixed-coordinate case, exceptional OR-of-AND exclusions and exact
coefficient applicability into disjoint or overlapping application cells.
Preserve conjunction semantics: excluding `a=0 AND b=0` does not mean
excluding either hyperplane individually.

The actual vac3 coordinate geometry permits exact interval/box operations.
Reuse the existing box-cover service; split at fixed values, exceptional
coordinate values and RHS activation/pinch thresholds. Unbounded intervals
must remain unbounded. Where a coefficient vanishes on a boundary, prove
that fact by native exact substitution before omitting the associated term.

On every retained cell, prove each nonzero RHS child is either in a proper
lower sector or lower in the persisted source-port order in the same sector.
Check shifted active coordinates that pinch, inactive coordinates that could
activate, and fixed numerical RHS coordinates. A sampled interior comparison
is not this proof. The order must agree across all sector programs so that
cross-sector calls cannot form a cycle.

Independently subtract application cells, authenticated zero sectors and
explicit terminal points from the entire family domain. Publication requires
an empty complement. A finite remainder may be adopted as additional
terminals only after explicit policy admission and evaluation provisioning;
an infinite remainder is incomplete. Queue exhaustion, C++ agreement and
bounded numerical search alone are not substitutes for this cover check.

The initial importer may reject genuinely coupled affine/nonlinear case
ownership with a typed unsupported error. This is an artifact-bridge scope
limit, not a regression in the source solver's broader geometry support.

## Five-class Vakint coverage

The six full-rank K6 denominator-symmetry orbits contain two different
three-line embeddings that both describe a product of three tadpoles.
Consequently six family orbits correspond to five registered physical graph
classes, not six distinct required Vakint topologies.

Freeze and test the live matcher-derived census: the six-line parent, its
five-line contraction, both inequivalent four-line contractions, and the
three-line contraction. Use each class's actual simultaneous routing witness
and stable parent denominator slots. Check both three-line family embeddings
in the artifact even though Vakint exposes one physical class. The existing
artifact test manifest is useful evidence, but its stored historical matcher
revision is not a replacement for comparing today's matcher.

## Acceptance and first implementation gate

The complete cold rule-set pass now succeeds. Retain the same independent
obligations when materializing and loading the final artifact:

- original-source replay failures or additional exceptional obligations;
- ordering/descent failures, including the explicit old-order counterexample;
- uncovered cells after all rule guards and all 38 proposed terminals;
- independently authenticated zero sectors and the five-class routing census.

The current source workload passes these replay/descent/cover checks. Its
canonical durable installation and five-class Vakint routing still need
validation. Any newly exposed missing branches are actionable solver inputs,
not grounds for weakening the installer.

Required regressions include mutated source/seed/translation rejection,
guard-zero selection, empty/finite/infinite complement distinction, terminal
admission without an independence claim, rule cycles and activation edges,
indices outside compact search encodings, deterministic one/six-worker
payloads, cold serialization reload, master-only memoized output, and non-unit
mass restoration. All algebra uses Symbolica's native APIs; no rational
reconstruction implementation is introduced.

Vakint then runs every applicable single-scale acceptance case through three
loops in its existing comparative harness, including all five classes,
scalar and tensor-bearing inputs, and dotted/numerator cases. The full
FeynKit/RustRed path uses an invalid FORM path; separate FORM-enabled
AlphaLoop/MATAD runs remain the authoritative development oracle. A finite
nonminimal terminal basis may use numerical parity instead of identical raw
master labels, with the required Laurent precision checked explicitly.

The harness inventory finds 40 applicable single-common-mass
one- through three-loop entrypoints, expanding to 46 parameter/input executions
across the five legacy end-to-end files. These are configured inputs, not fresh
passing tests or 46 distinct integrals. With the added variants and documented
aliases, enabled native peers map to 25 entries / 31 executions and pending
K6 peers map to 15 entries / 15 executions. The new enabled one-loop variant
has not yet run. Eleven core three-loop bodies already have ignored
RustRed counterparts (five comparative, six analytic, eight tensor-bearing).
Four PySecDec-reference bodies duplicate existing analytic workloads and are
explicit aliases. Four previously missing settings/input variants now have
native peers: three pending three-loop comparative variants and the decorated
one-loop input at five epsilon terms. Two additional pending basketball peers
include the finite part and are not counted in the 40-entry legacy inventory.
PySecDec itself
stays non-gating; existing native AlphaLoop/MATAD or analytic references provide
the comparison. The new five-class fixtures supplement, not replace, these
reused acceptance bodies. Exclude the genuinely multi-mass two-loop input and
the misleadingly named decorated `1l` FMFT input that actually has four loops.

Profile generation, cold certification/loading and warm scalar application
separately. Reference-workload timing is not artifact-generation timing until
the proof and serialization boundary is included. Each implementation slice
receives a separate mathematical/implementation audit before its milestone
commit and push.
