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

No new certified K6 artifact, installed K6 scalar backend, or three-loop
RustRed acceptance pass is claimed by this checkpoint. The next implementation
gate remains the cold replay/descent/cover pass described below.

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
this family. These input classifications still need independent mathematical
authentication at artifact construction/loading; a supplied mask list is not
a proof that an integral vanishes.

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

Before promising that the current 617 rules already form the final artifact,
perform one complete cold certificate pass. Report independently:

- original-source replay failures or additional exceptional obligations;
- ordering/descent failures, including the explicit old-order counterexample;
- uncovered cells after all rule guards and all 38 proposed terminals;
- independently authenticated zero sectors and the five-class routing census.

The source workload may already contain everything required, but the answer
must come from this pass. New missing branches are actionable solver inputs,
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

Profile generation, cold certification/loading and warm scalar application
separately. Reference-workload timing is not artifact-generation timing until
the proof and serialization boundary is included. Each implementation slice
receives a separate mathematical/implementation audit before its milestone
commit and push.
