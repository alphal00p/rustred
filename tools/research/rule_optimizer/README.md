# Bounded campaign profiling

## Exact cancellation across one routed rule boundary

`rule_optimizer_routed_cancellation` is a research diagnostic, not a new
campaign execution mode. By default it observes one complete singleton rule application,
routes every weighted term using the admitted selection, and sums coefficients
of equal canonical integral keys with Symbolica. It does not apply further
IBPs or combine independent parent identities.

```bash
cargo build --release --locked -p rustred-app --example rule_optimizer_routed_cancellation
target/release/examples/rule_optimizer_routed_cancellation REQUEST.json FRESH_OUTPUT_DIRECTORY
```

The request embeds the existing applied-observer request and explicit
route/coefficient/recording limits; see `routed_cancellation/input.rs`.
Exactly one singleton is required, together with expected complete native
successor and pinch counts. The full selection is admitted before observation.
The output retains the original native events, a routing ledger, binary
Symbolica coefficient tables, and distinct counts for endpoint occurrences,
unique keys and keys removed by exact coefficient cancellation. Incomplete
encoding, cancellation, conditional source coefficients or route failures do
not produce a successful complete report. Resource guards remain external;
the public transport API exposes per-call limits, not the aggregate charge
used by the finite campaign tracer.

On the frozen five-loop H1 stress parent tested on October4,473 routed
occurrences become372 distinct keys, of which26 cancel exactly. This is a
fixed-parent observation in the full shared-owner context, not a parametric
rule, a production bottleneck census or a measured campaign speedup. Existing
cold closure replay still requires the original successor obligations; the
diagnostic does not authorize deleting those dependencies.

### Optional weighted plateau cut

The same executable accepts an optional `plateau_cut` request field. It
composes saved-rule applications on fixed input integrals before collecting
their complete boundary with Symbolica. No production campaign mode changes.
For example, the following fields replace the legacy expected term counts:

```json
{
  "expected_successors": 0,
  "expected_strict_subsupport_successors": 0,
  "plateau_cut": {
    "max_depth": 2,
    "max_parents": 90,
    "max_apply_calls": 10000,
    "max_pending_terms": 100000
  }
}
```

All other request fields remain required. Set the recorder's `max_queries`
at least as high as `max_apply_calls`. The query file contains distinct
singleton parents with their original entry caps; child singletons are not
clipped to those caps. Depth two means the root application and at most one
further application along each path. Only same-owner children with unchanged
`E = total positive power - support size + numerator rank` are followed.
Every other term remains on the boundary, including pinches and E increases.
An incomplete, conditional or unmatched child application keeps the entire
weighted child, never a partial RHS. An exhausted global resource or recording
budget instead makes the observation incomplete.

Each parent has its own coefficient sum. The report distinguishes occurrence
deduplication from exact cancellation and also reports the union of surviving
key supports across parents; it never adds coefficients across separate input
equations. Collection occurs at the final fixed cut, not during expansion.
Thus the result measures cancellation potential, not saved application calls.
No routing or terminal declaration is performed in this mode.

Dispatch is explicitly **pointwise** and may differ from the campaign's
whole-domain rule selection. `complete` means that the requested cut was fully
observed, possibly with reported fallback boundaries—not that its integrals
were reduced or closed. Native event records, selected rules and binary
coefficient tables preserve the saved-program evidence; the adapter does not
claim a fresh flattened original-IBP proof, exceptional-dimension coverage,
or a campaign speedup. Omit `plateau_cut` to retain the original observer.

## Experimental exact initial-domain summaries

`rustred owner-domain-match` has an opt-in
`--finite-replay-initial-domain` mode for a **fresh epoch checkpoint**.
It attempts the original first query only: either a whole singleton (including
nonzero coordinate lowers) or a whole finite zero-lower A/R/D envelope whose
equality to the original capped domain is proved natively. It never truncates
a region to fit a budget. It runs the existing exact routed reducer and retains a
typed replay recipe instead of its expanded symbolic dependency graph.
Cold verification must use `--reinspect all --reference-levers off` and
recompute the reduction; recorded counters are not proof. Runtime resume and
query amendments are not supported in this experimental mode.

The nine `--finite-replay-max-*` allowances include the six trace caps plus
`positive-layers` (64), `seed-points` (1024), and `seed-bytes` (1048576).
The byte allowance bounds retained seed-buffer payload, not total trace RSS.
These additional allowances are intersected with the admitted reducer's
aggregate limits. Separate explicit controls expose those existing limits:
`--reduction-max-rule-applications` (default 1000000),
`--reduction-max-pending-frames` (1000000), and
`--reduction-max-coalescing-additions` (16000000). Zero is meaningful;
`--unbounded-work` changes neither tier. Raising only a finite-replay allowance
does not override a smaller admitted limit, and these controls do not loosen
per-formula algebra or transport expansion limits.

Add `--finite-replay-budget-preflight` to the same fresh finite-walk request,
without event/stop paths, to write numeric requested/admitted/effective limits
to `--output` and exit before owner preparation or checkpoint creation. This
reads manifest/query text but does not admit their contents. Live and cold
work report the same budget projection from the actual reducer; budget
refusals also retain native resource/requested/limit fields. The first H1
finite attempt exposed the inherited one-million rule-attempt ceiling despite
a sixteen-million additional allowance; its preserved decline is not evidence
that the intended sixteen-million budget is insufficient. The explicit CLI
controls and diagnostics pass focused native tests and the unchanged full
four-loop control; the corrected hard H1 performance comparison is separate.

Exact Symbolica-backed counting precedes bounded complete seed collection;
one joint trace shares exact-key work among all admitted seeds. Cancellation,
iterator errors and incomplete enumeration cannot produce a summary.
The v2 recipe and CLI replace the experimental v1 singleton interface; old
enabled v1 checkpoints are rejected, while absent/default-off identity is
unchanged. Bounded v2 acceptance passes: the four-loop default-off graph is
unchanged, H55 reproduces its singleton gain, and a complete original
260-integral five-loop physical region passes online and cold replay.
That region's traversal is faster, but its whole-job time is effectively tied
because setup dominates. These controls do not establish full-family coverage
or a production speedup.
Details are in the
[finite replay study](../../../docs/research/demand_directed_symbolic_sccs_2026-10-04.md#whole-initial-domain-replay-v2-implementation-and-bounded-acceptance).
This is not a production campaign setting.
Keep its actual exact-reduction work in performance comparisons: one retained
recipe does not mean one reduction operation.

## Native applied-obligation observation

The `rule_optimizer_applied_observer` Cargo example is a read-only research
adapter for `CandidateOwnerPrograms::visit_power_bounded_owner_applied_successors`.
It records the native selected rule and its complete nonzero successor events
**before** campaign snapshot containment can replace them with dependency links.
It does not follow successors, generate rules, select a policy or certify closure.

```bash
cargo build --release --locked -p rustred-app --example rule_optimizer_applied_observer
target/release/examples/rule_optimizer_applied_observer REQUEST.json FRESH_OUTPUT_DIRECTORY
```

The request names an unchanged saved-owner selection, its owner base, a
`rustred.owner-domain-queries.json.v2` file with original rank/A-D bounds,
preparation-only CSV targets, workers, and explicit native/recording limits.
The input types and required fields are in `applied_observer/input.rs`.
Current arities1–16 and workers1–50 mirror the existing native application
loader, not a topology-specific algorithm. The actual experiment's CPU/RSS/time
reservation must be enforced externally. Observation calls are serial; a worker
reservation is not a claim that every core is used by the visitor.

The fresh output directory contains input bindings, an event ledger, native
statistics/completeness status, and a Symbolica-backed binary coefficient table.
Coefficient references belong to that table and retain the original source
coordinate context; they are not algebraic display strings to parse. Guard
provenance refers to immutable input payloads and selected batch/rule IDs, not
a self-contained new ordinary-source certificate. Target-owner membership is
not target applicability or recursive closure. Conditional tails remain
obligations, not discarded zero terms.

Errors, cancellation, truncated recording, unmatched rule completion counts,
problems or optional coefficient refusals prevent a complete-outcome claim.
Even a complete observation is not a completed reduction. Keep preparation,
visitor-plus-recording, encoding and full process times separate. Compare exact
owner/phase/box/rank/power-bound keys rather than merging equal-looking index
tuples from different contexts.

Run focused tests with a workspace-local temporary directory, for example:

```bash
mkdir -p TMP/applied-observer-tests
TMPDIR="$PWD/TMP/applied-observer-tests" cargo test --release --locked -p rustred-app \
  --example rule_optimizer_applied_observer -- --test-threads=1
```

The October4 frozen67-domain comparison and optimized cached-library build
receipts are under ignored `TMP/rule-optimizer-20261003/profiles/preferred-overlap-applied-v2/`
and `candidates/applied-observer-build-v2/` under the same evidence root.
Those measurements use the archived campaign's effective native limits,
including its unbounded cumulative-work settings; finite scratch/algebra,
recording and external resource bounds still apply.

`profile_records.py` reads diagnostic CP6 schema-1 record windows without
restoring a checkpoint. It is a research reader, not native checkpoint
authentication or algebraic authority. Its wire interpretation follows
`walking/epoch/records/{wire,typed}.rs` and the native `ResolverCounters`.

The older `tools/research/py/records_{scan,cost,census}.py` target CP5 JSONL.
`records_census.py` also depends on a historical scratchpad owner-class file.
Do not point them at the binary CP6 segments or run a full scan merely to
obtain a small diagnostic sample.

Create and retain a plan before reading record bodies:

```bash
nix develop /common/dev/rustred --command python -B \
  tools/research/rule_optimizer/profile_records.py plan \
  --source campaigns/five-loop-a1-epoch-frontier-repaired-20261001:1,13,26,38 \
  --source campaigns/five-loop-a1-new-rules-20261002:1,9,17,25 \
  --out TMP/my-profile/plan.json
```

The plan binds committed segment metadata, exact offsets, file size/mtime,
input receipts and observation time. It chooses four 1 MiB windows per
specified sealed segment, beginning at 1/8, 3/8, 5/8 and 7/8 of its bytes.
Actual sample execution also enforces the 32 MiB limit and rejects overlapping
windows, changed files, wrong schema and inconsistent committed bounds.

The default planner requires a stopped campaign. An explicit
`--live-sealed-only` opts into a smaller read-only diagnostic: exactly one
campaign and two distinct generations older than its captured committed
manifest, with the same four fixed windows per segment and at most8MiB total.
Only the captured committed registry can select segments; active tails,
unlisted/orphan files and the latest sealed generation are excluded. Immutable
registry/meta snapshots are rechecked; `latest.json` may advance without
reselection. Each segment is read through one no-follow read-only descriptor,
checking device/inode/size/mtime/ctime and path identity before and after.
Pin both `profile_records.py` and its sibling `profile_sealed.py` in the
execution receipt. This does not hash whole record bodies, restore a checkpoint,
take a production lock or alter its lifecycle. Retained regions remain
nomination evidence, not selected-rule, required-root ancestry or closure proof.

```bash
nix develop /common/dev/rustred --command taskset -c 0 python -B \
  tools/research/rule_optimizer/profile_records.py sample \
  --plan TMP/my-profile/plan.json --out TMP/my-profile/sample.json
```

CPU 0 is an example; use the coordinator's allocation. Three consecutive
fully decoded frames establish the first window boundary. Every subsequent
whole frame is decoded; an incomplete last frame is counted as omitted.
Unknown schema/corruption fails the read. Whole segment digests are recorded
as claims from metadata, never advertised as independently authenticated by
this bounded read. Window/frame SHA256 receipts allow exact re-extraction.

The sample includes per-window/phase/owner/domain-stratum aggregates, 200
slowest records and one deterministic representative per stratum. It is not
a census, probability sample or globally ranked hotspot list. Fixed byte
windows have record-size, boundary, publication-cluster and stage biases.
Do not compare campaigns with different binaries or cohorts causally.

The `panel` subcommand consumes a separate frozen policy listing retained
`source_id`/`id` pairs. It exports ordinary native query documents, original
owner-index and payload metadata, original same-owner required/helper queries,
full record provenance and expected Apply/Route phase. G2 pieces are exported
separately from their original parent; an empty G2 residual produces zero
queries and is diagnostic only. Native loading/reinspection is a separate
validation gate. Owner-family holdouts should remain excluded from candidate
fitting; adjacent record/window separation alone is insufficient.

```bash
nix develop /common/dev/rustred --command python -B \
  tools/research/rule_optimizer/profile_records.py panel \
  --plan TMP/my-profile/plan.json --sample TMP/my-profile/sample.json \
  --policy TMP/my-profile/panel-policy.json \
  --directory TMP/my-profile/panel --out TMP/my-profile/panel-manifest.json
nix develop /common/dev/rustred --command python -B -m unittest discover \
  -s tools/research/rule_optimizer -p profile_tests.py -v
```

Interpret fields literally: emitted and accepted are native event counts,
not new child nodes. `distinct_edges` includes graph obligations to existing
nodes and can include anchor edges in G2 scope. `known_reuse` means an earlier
emission in the same job already produced an edge; stored snapshot lookup
hits are a different, aggregate statistic. Neither reuse nor local inspection
establishes recursive closure. Rule identity, newly admitted children and
required/helper root ancestry are absent unless separately established.
Inspector seconds omit coordinator work and are neither process CPU time nor
end-to-end wall time. Never attach them to an inferred selected rule.

Campaign evidence belongs under ignored `TMP/`, not in a commit. The initial
October 3 receipts are in `TMP/rule-optimizer-20261003/profiles/`.

# Shared-cohort rule evaluation

`evaluate.py` plans fresh matched walks through the existing saved-owner staging,
campaign supervisor and native cold verifier. It does not launch processes,
generate rules, change production, or authenticate algebraic proof receipts.
Each arm receives the complete query document and shared owner/routing context.

Supply schema `rustred.rule-optimizer-evaluation.v1` with a pinned baseline
selection/query document and absolute owner base; a frozen executable; pinned
supervisor and native command templates; a fresh destination, cohort name and
arm order; optional owner replacements with independently reviewed native-source
provenance; explicit cumulative stage/walk/cold deadlines; and a preregistered
primary metric, minimum gain and regression bounds. An empty replacement list
is an A/A control. Complete work uses `--unbounded-work`; bounded discovery uses
an explicit `max_domains`. Every pilot's inclusive ceiling is at most 1800 seconds.

```bash
python -B tools/research/rule_optimizer/evaluate.py plan REQUEST.json
python -B tools/research/rule_optimizer/evaluate.py plan REQUEST.json --write-plan
```

The first command only prints a compact summary. The second creates the full
`plan.json` and each arm's `selection-source.json` in a fresh directory, refusing
an existing destination, and prints their location and summary. Use `--full-plan`
only when the complete planning JSON is needed on stdout: full five-loop routing
metadata can otherwise flood the terminal with tens of megabytes.
Exact stage/walk/cold argv arrays are included. The ordinary staging tool copies
owner/overlay payloads and preserves query bytes. Replacing a base-bound repair
overlay's owner requires a suitable native re-export; Python does not rebind it.

For such an owner replacement, provide `overlay_replacements` with one entry
for **every affected overlay**, using its zero-based `overlay_index`,
`owner_mask`, baseline `original_sha256`, and the new native-exported `path`
and `sha256`. Each entry also supplies a nonempty `source_provenance` list of
pinned `{path, sha256}` receipts. The planner preserves overlay ordering,
count and other metadata, leaves unrelated overlays untouched, and derives
the replacement byte count from the file. Missing or duplicate replacements,
incorrect baseline bindings, and replacements without a matching owner change
are rejected. These pins establish experiment inputs, not mathematical
authority: native loading and cold original-source replay must still pass.

Run phases through the existing owned-process guard with its heavy/build locks,
process-tree memory checks, CPU allocation and cumulative deadlines. The native
supervisor's `--objective-hours` is telemetry, not a timeout. Setup and draining
count toward the inclusive budget. Both arms use fresh checkpoints and graphs;
do not flush host caches. Counterbalanced pairs address ordinary warm-cache and
execution-order effects. The root-owned `TMP/rule-optimizer-20261003/run_pair.py`
research runner reuses `run_arm.phase` for this purpose.

The initial A/A request is
`TMP/rule-optimizer-20261003/candidates/aa-whole58-request.json`, with its generated
`aa-whole58-v2/plan.json` alongside. It preserves 58 required queries, 16 owners
and 508 routes under frozen executable `8ef80b52...`, within 600 seconds.
`known-negative-whole58-request.json` exercises the previously rejected checked
whole-piece priority replacement under the same cohort and settings.

The `compare` input contains `plan` (a plan filename) and `arms`. Each arm names
JSON files under keys `selection`, `input_receipt`, `native_request`, `supervisor`,
`walk`, `cold`, and `measurement`. The outer measurement contains `arm_seconds`,
existing guarded `phases` receipts for stage/walk/cold, and
`executable_sha256_before` / `executable_sha256_after`. Preserve raw resource
samples and source-generation costs; native traversal is not the full arm clock.

```bash
python -B tools/research/rule_optimizer/evaluate.py compare RESULTS.json
python -B -m unittest discover -s tools/research/rule_optimizer -p 'test_evaluate.py' -v
```

Completion requires zero pending or abandoned work, no frontiers/errors,
drained workers, unchanged frozen context, and native cold-All reinspection
with reference levers off. Every required/helper query and initial root must
be independently verified. Each actual command must equal its planned argv,
and the cold report must name that arm's checkpoint path and generation.
Memory/worker/CPU settings must match. Native exit 4 can mean checkpoint-only
success; its stale closure snapshot can say `incomplete`, and its optional
`error` field can be absent. Explicit clean failure/admission fields plus cold
verification decide completion. Missing evidence fails closed. Censored arms
retain their debt and cannot win by stopping early.

Metrics concern one shared cohort, never sums of independent root traces.
A/A stays neutral even with timing noise. A candidate target is evaluated only
against the preregistered primary metric and regression limits. Original-source
authority and unchanged ordered terminal inventories remain separate native
gates. Promotion additionally needs counterbalanced pairs, complete controls,
held-outs, generation-cost accounting and independent review. This helper
always retains the incumbent and never authorizes a production switch.

## Selecting a subset of preferred saved rules

Each `preferred_programs` entry in an evaluation request, or
`preferred_owner_programs` entry in a saved-owner selection, may supply
`"rule_ordinals": [3, 11]`. These are original saved ordinals in that preferred
payload, not positions in a filtered vector. The list must be strictly increasing:
it selects eligibility, not a new rule order. Omission or `null` enables all
preferred rules; `[]` enables none. Native admission rejects unknown ordinals
and still validates **every** rule in the original payload before selecting.
This is the preference-entry field, not `candidate-inspect`'s diagnostic rule
filter: that separate API rejects an empty filter; omit it for the normal view.

Baseline terminal precedence and the preferred program's entire residual-hole
set are unchanged. Unselected rules are skipped; ordinarily inapplicable rules
leave the remaining eligible preferred rules available in their saved order.
Only an ordinary gap in the preferred batch, or an explicit residual hole,
falls back to the original program. Algebra, source, resource and descent errors
are not swallowed. Staging and fresh campaign copying preserve the subset, and
evaluation receipts compare it alongside the actual payload identity.

Use a fresh traversal for a different subset. Native checkpoint identity includes
an opt-in subset-semantics marker, as well as the literal selection and payloads:
an older binary could ignore this optional JSON field, so identical JSON alone
would not establish identical semantics. Absent-field behavior is unchanged.
An empty subset preserves baseline mathematical outcomes, not necessarily
baseline-only batch IDs or matching counters: the preferred terminal/hole layer
is still present. Selecting all saved IDs explicitly preserves native dispatch
but intentionally binds the opt-in policy identity. Full input admission costs
remain even for a small subset.

Disabling earlier preferred rules can expose a later rule on previously
shadowed domains. A list of observed rule hits is therefore only a candidate
selector, not an exhaustive forecast of the new policy. Measure complete shared
cohorts and cold-reinspect before claiming a gain; do not splice old dependency
graphs together and call the result a native reduction.

# Finite projected-source-bank diagnostic

`projected_bank.rs` is a generic, input-directed native diagnostic. It reloads
one saved owner, binds its family and persisted order, regenerates requested
ordinary IBP source translations, and compares the same bank under two discovery
projections: all non-descending non-target columns (F), then F plus explicitly
nominated lower keys (H). Symbolica performs elimination and the exact weighted
product over the full original matrix. Every surviving tail must strictly
descend under the unchanged saved order; zero-sector terms are not discarded.

It is also a Cargo example, so cached machine-specific RLIBs are not required:

```bash
cargo test --release --locked --offline -p rustred-app --example rule_optimizer_projected_bank
cargo build --release --locked --offline -p rustred-app --example rule_optimizer_projected_bank
target/release/examples/rule_optimizer_projected_bank validate REQUEST.json
target/release/examples/rule_optimizer_projected_bank probe OWNER.rrbin REQUEST.json
```

Run native commands through the existing owned-process supervisor with explicit
CPU/RSS/wall budgets; the commands above do not provide process supervision.
The first optimized execution uses pinned existing release libraries, with
build/test/probe receipts in ignored
`TMP/rule-optimizer-20261003/candidates/projected-bank-build-v1/`.

Requests use schema `rustred.projected-source-bank.v1`: numeric `target`, exact
`owner_mask`, `family_fingerprint`, `expected_order`, `expensive_keys` (H, which
may be a structural hypothesis rather than measured expensive), and `sources`.
Each source supplies an `offset` and exactly one native `source_row` RowId string
or `source_ordinal`. Named IDs are resolved from the completed native inventory;
they do not assume a loop-index-to-ordinal formula. Optional `source_visitation`
permutes the native canonical offset-major selected-source order. Resource
limits bound input bytes, source counts/conditions/coordinates, matrix shape,
retained reducer nonzeros, source coefficient terms and final report size.
Native intermediate expression growth additionally needs outer RSS/deadline
limits. The shape validator does not load an owner or establish source authority.

The preregistered four unit fixtures cover input refusals, native cancellation
of both H and genuinely non-descending F columns, full-tail retention, and a
known massive tadpole bank whose extra H constraint has no solution. A miss is
only a miss in this finite bank, never a new master or irreducibility result.

Output keeps source provenance, original/specialization guards, conservative
native pivot guards, all exact finite tails and source weights. Display strings
are diagnostics, not a reusable proof artifact. Base parameters and dimension
remain symbolic, but integral powers are specialized: this is not a parametric
chart certificate, priority-owner export or shared downstream cost comparison.
If F already avoids H, identical F/F+H results do not establish that the extra
projection helped. Publication requires separate native original-source replay
on the full declared chart, complete exceptional geometry and the shared-cohort
evaluation gates above.

# Prescribed symbolic-source chart producer

`prescribed_source.rs` is separate from finite GPLU discovery. Its input names
ordinary RowIds, translations and exact rational constant weights. Native source
generation and checked indexed arithmetic form the full symbolic identity;
only explicitly fixed coordinates are specialized. The target coefficient is
derived from that full sum and used to normalize every source weight and RHS.
All source poles and specialization/normalization witnesses survive cancellation.
The existing original-source producer must then prove every sign cell under
the saved global order. No point coefficient is generalized into a chart proof.

```bash
cargo test --release --locked --offline -p rustred-app --example rule_optimizer_prescribed_source
cargo build --release --locked --offline -p rustred-app --example rule_optimizer_prescribed_source
target/release/examples/rule_optimizer_prescribed_source validate REQUEST.json
target/release/examples/rule_optimizer_prescribed_source prove OWNER.rrbin REQUEST.json
target/release/examples/rule_optimizer_prescribed_source export OWNER.rrbin REQUEST.json FRESH_DIRECTORY
```

As above, use the existing resource supervisor for builds and native work.
Schema `rustred.prescribed-source-chart.v1` requires owner/family/order bindings,
`chart.lower`, `chart.upper` (`null` for infinity), sorted `chart.fixed` physical
index/value pairs, `sources` with `source_row`, `offset`, and `weight: [p,q]`,
and explicit resource limits. Coordinates are sector-local: active `n-1`,
inactive `-n`. This producer's current input validation restricts nonfixed axes
to lower 0 or 1 and no upper bound. This format cannot encode correlated A/R/D predicates;
a broader coordinate chart needs its own fresh exact proof.

An optional `owner_load_limits` object supplies the seven native ingress limits
(`max_bundle_bytes`, `max_total_input_bytes`, `max_total_coefficient_bytes`,
`max_collection_entries`, `max_total_symbolica_state_bytes`,
`max_zero_sector_visits`, `max_coefficient_bytes`). Omission preserves native
defaults. Explicit values must be positive; the native 1 GiB bundle hard cap
still applies. The same bundle policy reaches loading, pre/post-export
inspection and priority encoding. This changes transport allowances, not proof
or exact-algebra limits. The subset/ingress revision passed 22 native producer
tests; both absent and explicit-default ingress reproduced the previous radial
candidate bytes exactly. Evidence: ignored `candidates/source-ingress-subset-build-v1/`
under `TMP/rule-optimizer-20261003/`.

Export rechecks the source proof through the existing checked-priority encoder,
including runtime guard representability and native codec roundtrip. It retains
the old rule suffix, terminals and coefficient identities and marks the new
rule `AfterBaselinePartitionWholePiece`. It writes only a fresh candidate file,
request and proof receipt; it does not install an owner or launch traversal.
Candidate bytes alone do not replay the original-source proof, and the JSON
display/receipt is not an independent mathematical authentication mechanism.
The actual matched baseline piece keeps its original A/R/D geometry.

Three native tests exercise malformed source/chart refusal, wrong native
family/order/RowId refusal, and a known unbounded tadpole proof/export with a
boundary-pole refusal. Source-weight normalization, full-chart proof, priority
export, shared-cohort closure and performance remain distinct gates.

## Geometry-derived source nomination

The same producer accepts a separate `rustred.geometry-tangent-chart.v1` schema.
Keep the exact owner/family/order, chart and limits fields, omit `sources`, and
provide a runtime choice of coordinate loop and numerator axis, both zero-based:

```json
"nomination": {"differentiated_loop": 0, "numerator_axis": 13}
```

The numeric example is a request choice, not a built-in family dispatch. The
selected numerator must be inactive and strictly negative throughout the declared
chart. `validate` checks only input shape; `prove` and `export` still require an
exact saved owner. The original prescribed schema and rational `[p,q]` source
semantics are unchanged; mixing geometry nomination with `sources` is refused.

`geometry_tangent.rs` checks native derivative incidence in the selected loop.
It requires exactly one dependent active denominator with radial loop dependence.
Half the selected numerator's off-diagonal scalar-product coefficients define
the momentum `p`. Native affine expansions of `k²` and `k·p` then decompose
`V = k² p - (k·p) k` into ordinary source RowIds and monomial translations.
Rows are resolved through the completed native source inventory, not an assumed
ordinal formula. Base-parameter coefficients remain authenticated native objects
in-process; display strings are never reparsed or used as authority.

This bounded nomination handles one explicit loop/numerator choice per request.
It neither searches arbitrary loop-coordinate transformations nor adds source
shells, solves tensor reductions, chooses a new descent order, or edits a chart.
Limits bound source pairs (including intermediate coalescing), retained poles,
and nomination work; native expression growth still requires the external
RSS/deadline supervisor. Coefficient poles are retained before cancellations.

Derivative-independent inactive axes are reported as spectators. Other dependent
inactive axes are separately reported with their declared fixed/free intervals;
they are not made into spectators by a nominal zero exponent. The shared ordinary
source product retains every mixed-numerator derivative, zero-sector term and
guard. A valid nomination is not a chart proof, and a single-numerator success
does not imply mixed-cross descent or downstream improvement. Only the existing
full original-source proof and checked exporter can admit the requested chart.

Focused fixtures cover the existing eight-view identity, loop/denominator
permutation, native parameter-valued weights and canceled poles, unsupported
incidence/resource refusals, and full mixed tails versus genuine spectators.
These join the original producer tests and a separate-schema input refusal test.
Run them with the existing `rule_optimizer_prescribed_source` Cargo example test
command above. New nomination code requires its own fresh build/test/proof
receipts; an earlier frozen prescribed-producer binary does not contain it.

The first guarded geometry build passed all nine tests. On the separately frozen
spectator-free chart, native nomination derived eight sources and proved all
18 sign cells; its checked export was byte-identical to the measured prescribed
candidate. A fresh prescribed-mode export from the new binary was also identical.
The geometry proof retains additional pre-cancellation witnesses, so its proof
receipt's condition-origin ordinals differ even though the emitted owner bytes,
exact source weights, RHS, chart cells and guard polynomials coincide. Raw receipts
are preserved in ignored `TMP/rule-optimizer-20261003/candidates/geometry-source-build-v1/`,
with `equivalence.json` recording the comparison. This validates nomination for
that chart, not a mixed-cross chart or an additional performance measurement.

## Optional protected-gradient nomination

The geometry schema also accepts `nomination.mode: "protected-gradient"`.
Omitting `mode`, or selecting `"radial"`, retains the existing radial algorithm.
An unknown mode is refused. The optional mode still requires exactly one active
denominator depending on the selected coordinate loop; it does not handle two
protected propagators or silently change the explicit fixed/free chart.

For that denominator `P`, write its loop gradient as `g = ∂P/∂k`.
Let `aP`, `aJ` be the squared-loop coefficients of `P` and the selected numerator
`J`, and `bP[r]`, `bJ[r]` their raw off-diagonal scalar-product coefficients.
The canonical direction has `p[k]=0` and
`p[r]=aP*bJ[r]-aJ*bP[r]`. Native checked multiplication/subtraction and cached
derivative contractions assemble each component of
`V = (g·g)p - (g·p)g` into ordinary source translations. This protects a
routing-shifted quadratic denominator without a Gram inverse, square root,
custom polynomial engine, or fitted source weights.

`geometry_gradient.rs` is separate from the radial algorithm and reuses its
bounded affine-source builder. All operand/product poles survive subtraction
and coalescing, including inverse-basis witnesses from native derivative
expansions. Zero squared-loop coefficient, zero effective direction, empty
vector field, or a resource miss refuses nomination; none authorizes a terminal,
new chart, or alternate search. Full ordinary-source generation, actual target
normalization, unchanged saved-order proof and checked export remain mandatory.
Mixed-numerator tails are retained, not assumed to descend.

Focused fixtures cover a generic shifted two-loop mass family, loop and
denominator permutation, a loop/external Gram example, parameter-pole propagation,
degenerate cases, and radial-limit normalization against the old algorithm.
They pass in the fresh audited build; the earlier measured binary does not
contain this optional mode. The current operational held-out cohort
motivated this extension and must not be reused as pristine held-out validation
for it. No broader validity or performance is inferred from the algebraic identity.

The first optional-mode build passed all 15 tests, including the original nine.
A separate training probe through the unchanged radial mode proved and exported
the explicit mixed-D6 chart (D6 and D14 strictly negative, D7/D8 fixed zero):
eight sources, 28 RHS terms and 72 sign cells. No shared-cost result is implied.
A prospective full-cross chart with no fixed indices initially stopped at the
native cumulative sign-partition work budget. An explicitly approved retry
changed only that work allowance to four million and the report cap to 128 MiB;
the unchanged chart then passed native proof and checked export: eight sources,
59 RHS terms and all 2,304 sign cells, with all 25 original terminals retained.
The 50.5 MB export report exceeded the research wrapper's old 32 MiB reader cap
after native success; the raw receipt was preserved and a bounded reader
correction validated the existing report without rerunning native. No shared-cost
claim follows from that export. Raw receipts are in ignored
`TMP/rule-optimizer-20261003/candidates/gradient-source-build-v1/`.
A fresh radial-control export from the optional-mode binary is byte-identical to
the measured prescribed candidate. The generic shifted-gradient toy's first
proof attempt exhausted a four-row source allowance with seven translated rows.
An approved eight-row-budget retry, with unchanged equations and chart, passed
native proof and checked export: seven sources, eight RHS terms, 18 sign cells,
and its one original terminal retained. This is an independent shifted-family
chart proof, not an operational-family or performance claim. Its input-grammar
refusal (reserved label `J`, later renamed `N`) and first budget refusal remain
in the raw evidence beside the successful retry.

## Two-protected native source bridge

The separate request schema `rustred.two-protected-tangent-chart.v1` feeds the
existing native `TangentSourcePlan` through the same producer executable. It
uses the existing family/order/chart/limit fields, but has this nomination shape
(indices are zero-based; this is a shape example, not a proved chart):

```json
{
  "differentiated_loop": 0,
  "protected_denominators": [0, 1],
  "contractions": [
    {"kind": "loop", "index": 0},
    {"kind": "loop", "index": 1},
    {"kind": "external", "index": 0}
  ],
  "recenter": [0, 0, 1]
}
```

The axes, three distinct contraction directions, and full-arity integer recenter
are explicit inputs. Protected axes need not be active. Unlike the radial mode,
this schema does not infer a numerator, impose a recenter convention, or silently
change the chart. Mixed schemas, prescribed weights, multipliers and unknown
nomination fields are refused. The native family validates actual directions.

`geometry_two_protected.rs` constructs native signed two-by-two minors and
materializes their complete ordinary-source product. It does not implement a
polynomial engine or solve its own source equations. Output vector degree is at
most two; native tangency verification separately permits degree-three
intermediate derivative-times-minor products. All native limits are explicitly
mapped from the request, and external time/RSS supervision remains mandatory.
`max_complete_source_rows` also bounds the native original-proof source batch,
so it must accommodate both the completed ordinary inventory and selected
translated rows, not just the number of ordinary RowIds.

Named RowIds, translations and indexed weights stay native in-process. Every
materialized pre-cancellation condition enters the common producer, whose full
regenerated product must equal the native materialized product exactly before
fixed specialization or target normalization. Native ordinary rows carry family
domain/input-basis guards; the adapter invents no intermediate-pole getter.
The unchanged original-source chart checker and runtime-guard exporter remain
the only admission path. Degenerate minors, absent pivots, non-descending tails,
unsupported poles, or resource misses do not authorize a partial chart or a
terminal. No downstream improvement follows merely from tangency.

The bridge fixtures cover full mixed derivatives and an explicit
parameter pole, external directions, arbitrary recenter, protected-axis order,
degree-three verification versus degree-two output, malformed inputs and bounded
refusals. They join the existing producer tests through the same Cargo example.
The first supervised optimized build passed all 20 tests (the original 15 plus
five bridge fixtures). Prospective toy and observed-owner chart inputs were
frozen before native outcomes; their original-source proof, checked export and
target applicability remain separate gates. Build receipts are in ignored
`TMP/rule-optimizer-20261003/candidates/two-protected-source-build-v1/`.

The first frozen two-protected toy chart was refused by the native saved-order
checker: a term transfers rank from its selected numerator into the other
inactive axis and is not uniformly lower. Its chart was not narrowed afterward.
The independently frozen owner-15 full sign chart (including the observed
rank-zero face) stopped at the 4,096-sign-cell allowance. That is an incomplete
proof, not a mathematical or non-descent verdict; no owner-15 candidate or target
hit was claimed. Both original refusals remain beside the successful old-radial
control export, whose bytes exactly match the previously measured candidate.

A separately approved diagnostic fixed all 15 indices to the already frozen
owner-15 rank-zero tuple, without changing the source nomination. It reached an
actual native non-descent refusal: shift `-2 e0 - e1 + 2 e13` activates physical
D14 from zero to two and enlarges support. This falsifies the unchanged recipe
on that required point; increasing the full-chart partition budget alone cannot
repair it. The fixed-point request was prove-only, produced no export, and did
not replace or narrow the original full-sign request.

## Finite joint logarithmic source kernel

`logarithmic_kernel.rs` and its private `logarithmic_kernel/` modules implement
the `rule_optimizer_logarithmic_kernel` Cargo example. Its CLI is
`validate REQUEST` or `probe OWNER REQUEST`; neither publishes a rule, installs
an owner, or performs a recursive walk. Requests bind the native family and
saved order, physical target, protected axes, recenter, degree and
finite resource limits. Degree 0/1 is an explicit experiment policy, not a
family or loop-count branch. No automatic degree or target escalation occurs.
Protected axes must be a sorted, unique, nonempty subset of the owner's actual
active axes. The report distinguishes that subset from complete active-axis
protection. `kernel_vector_subset_selection: false` means all resulting kernel
vectors are retained; it does not mean every active denominator was protected.
The seven public owner-ingress limits are explicit request fields, separate
from the finite source/matrix limits; exact-algebra defaults and the native
1 GiB per-bundle hard ceiling remain in force. Loading an owner reconstructs
its saved records, so a small algebra request does not imply cheap loading.

For every native ordinary direction and denominator monomial up to that degree,
native derivative contractions and Symbolica polynomial products construct the
coefficients of `V(Dj) mod Dj`. Symbolica reduces `[constraints | identity]`;
every resulting kernel vector is independently multiplied by the entire
constraint matrix. Native completed-source inventory supplies actual RowIds
and ordinals. Each polynomial weight uses the original source translated by
`recenter - monomial`, retaining full product-rule terms and original guards.
No coefficient display is parsed back into algebra.

Complete parametric source images distinguish identically zero combinations
from nonzero images that vanish only at the requested point. Generic image rank
over `K(n)` may be explicitly disabled and then reports `null/not_computed`;
no kernel image is dropped by that choice. Native point rank and the target
solve retain all non-target keys that fail strict descent under the saved order.
Any target combination is composed back to original translated rows and replayed
over every physical column. Zero-sector and cross-owner tails are not discarded.
Pivot, source, family and pre-cancellation denominator conditions are retained.

The initial six tests exercise a massive-tadpole degree-1 target and degree-0/boundary
misses, a genuine joint-rotation zero source image, point-only vanishing,
denominator permutation, forbidden-column cancellation and mutated replay
rejection. The supervised optimized test binary passed all six; build and test
receipts are in ignored
`TMP/rule-optimizer-20261003/candidates/logarithmic-kernel-build-v2/`.
One failed preliminary test compile is retained in `build-v1`; its only fix was
serialization of a native stable ordering identifier via `to_string()`.
The first owner66 attempt then hit the default 256 MiB native ingress limit
before algebra, despite the request's 1 GiB file allowance. This is not a
degree-0 mathematical negative. The approved explicit ingress-policy revision
copies the immutable campaign's limits and adds a focused propagation test;
the original refusal and requests remain preserved. Its rerun uses isolated
CPU64 with its own lock, unchanged 32 GiB RSS and three-minute allowance, and
may overlap other CPU32–47 work; timings are not compared with that refusal.
All seven tests passed after that revision in `logarithmic-kernel-build-v4/`.
The short `build-v3` test-compile refusal is preserved too: its only fix was
explicit unsigned types for two 2 GiB test-JSON literals.

The first owner66 requests freeze one previously sampled A12/R0 point and zero
recentering for both degrees. They are source discovery, not completion of its
468-tuple residual or the 7,315-tuple training cohort. Missing target, zero image,
guard refusal or exhausted limits are finite negative/incomplete outcomes, not
master-integral or cost claims. Structural preflight and retained-result bounds
do not bound every Symbolica scratch allocation: an owned time/RSS supervisor
and separate native execution grant remain mandatory.

Both corrected owner66 probes completed under that guard. Degree 0 had 25
unknowns and constraint rank 25, hence no kernel. Degree 1 had 400 unknowns,
constraint rank 390, and ten kernel vectors; every full **parametric** original
source image was exactly zero. Neither supplied a target pivot (point image
rank zero in both cases). This is a finite-ansatz negative, not merely a
point-specialization cancellation, and not a general irreducibility, closure
or higher-degree result. Generic image-rank computation remains explicitly
skipped; no nonzero image was discarded. The ten vectors are not identified as
a particular geometric basis by this report. No degree escalation followed.
Compact evidence and full raw reports are in ignored
`candidates/logarithmic-owner66-finite-summary-v1.json` and
`candidates/logarithmic-kernel-build-v4/probe-degree{0,1}/` beneath the same TMP
campaign. Both processes exited successfully and drained their owned groups.

A separately preregistered degree-1 experiment protected only zero-based axes
`[3,5,6,7]` (D4/D6/D7/D8), with recenter `-e9` (D10), at the same owner66 point
and saved order. These destinations came from four observed incumbent dot
transfers, not measured per-child costs. The subset validator passed all eight
kernel tests before this probe. Its 400-by-480 matrix had 2,025 nonzeros, rank
260 and kernel dimension 140. All 140 complete parametric images were nonzero;
none vanished entirely at the point. Despite point image rank 130, there was
no target pivot with all 271 non-lower columns retained. Full constraint,
parametric source and point-specialization replays passed. Generic `K(n)` image
rank was not computed. No candidate existed to replay or export.

All four nominated transfers were absent from those images, so suppressing
them alone did not produce a useful fixed-point relation. The exact nonzero
images use 312 distinct original RowId/offset pairs. Their raw union is only a
pending superspan nomination: arbitrary weights on those sources would not
inherit logarithmic protection or kernel-weight guards. No larger-bank run was
authorized by extracting it. Compact evidence is in
`candidates/selective-H4-finite-summary-v1.json` and
`candidates/selective-H4-raw-support-union-v1.json`; the full native receipt is
`candidates/source-ingress-subset-build-v1/selective-H4/`. The older all-active
zero-image negative remains unchanged. These are finite source diagnostics,
not chart, closure, irreducibility or performance claims.

## Finite symbolic source projector

`symbolic_projector.rs` and its private `symbolic_projector/` modules implement
the `rule_optimizer_symbolic_projector` Cargo example. Build it with
`cargo build --release -p rustred-app --example rule_optimizer_symbolic_projector`;
run its focused fixtures with the corresponding `cargo test --release` command.
The CLI is `validate REQUEST`, `inspect OWNER REQUEST`, `support-inspect OWNER REQUEST`,
`nominate OWNER REQUEST`, `prove OWNER REQUEST`, or `export OWNER REQUEST FRESH_DIRECTORY`. It never runs a recursive walk or
changes an installed owner. Exported candidates must still be tested with the
same frozen campaign executable and shared owner pool as their baseline.

Projection requests use `rustred.symbolic-source-projector.v1`, binding the
family, owner, persisted order, coordinate chart, ordered finite bank of native
RowIds/translations, explicit forbidden shifts, refinement count, ingress limits
and algebra/report limits. `max_complete_source_rows` also sets the native proof
source-row cap; it is not solely an ordinary-generator inventory allowance.
The JSON interface currently accepts ordinary source rows; native weighted
source spans are also supported in-process and tested.
There is no coefficient-display parser or numerical-weight lift. An optional
read-only `rustred.symbolic-source-inspection.v1` request selects one saved rule
through the existing public inspector, returning its actual coordinate/affine
case and a bounded RHS-shape summary, not source or dispatch authority.

The separate `support-inspect OWNER REQUEST` mode accepts
`rustred.symbolic-source-support-inspection.v2`. It nominates all completed
native ordinary RowIds at every distinct stored-seed displacement of one saved
coordinate rule. The common source recentering is **unknown** by default:
generation canonicalizes the winning equation but retains its original seeds,
so the saved canonical target does not reveal the raw winning pivot's shift.
Optional `source_recenter_nomination` supplies an explicit, unverified common
translation; it must have exact arity and zero entries on fixed axes. The report
tags unrecentered versus caller-recentered offsets and grants neither nominee
source authority. Fixed offsets are the seed's absolute index minus the fixed
target value, not the saved coefficient shift (which must be zero there).
The output is factored into offset and RowId lists, with exact case/target and
saved exclusion-ID metadata. Basis ordinals and weights are not interpreted or
exported; an empty source list, inconsistent seed transport or affine case
refuses. Neither unrecentered seeds nor an unverified recentering establish an
incumbent superspan. Zero-sector projection and guards also require full
original-source regeneration and proof.
Explicit limits bound the retained-seed scan, distinct offsets, ordinary rows,
Cartesian pair count and output. Generated term/condition totals are retained
inventory bounds checked after native completion, not scratch allocation caps;
native arithmetic limits and external time/RSS supervision remain necessary.
The source-support slice passed seven focused app tests (an unoptimized semantic
test binary with optimized dependencies) and all 16 optimized research-example
tests. Its app library and research producer were optimized; the core and campaign
CLI were not rebuilt. The first read-only owner66/rule698 inspection completed
in 6.223 seconds: 1,969 saved seeds yielded 212 unique physical offsets and 25
native ordinary RowIds, or 5,300 factored pairs. Its v1 report incorrectly
interpreted the already-canonical target as establishing zero source recentering;
those offsets are only unrecentered seeds. The original receipts remain intact.
The v2 regression uses a genuinely generated tadpole: target and seed both store
zero, but only source offset -1 reproduces its complete saved normalized RHS.
The v2 correction passed nine focused app inspection tests (six source-support
tests and three existing inspection tests), including that complete native RHS
comparison. Its app library is opt2/noLTO; the semantic test binary is opt0 with
optimized dependencies. Receipts are in `candidates/source-recenter-build-v1/`;
the separately rebuilt research driver gate is still pending. Frozen v1
build/test and inspection receipts are
under ignored `candidates/source-support-build-v1/` and
`candidates/symbolic-owner66-source-support-v1/` beneath the TMP root below.

Symbolica reduces the augmented `[forbidden | target | identity]` matrix over
the native indexed field. Its identity tail is multiplied by the complete source
image, then composed back to named original source rows and replayed again.
Input, source, weight, fixed-specialization, pivot and pre-cancellation conditions
are retained. The public native-result ingress checks map/layout/resources; it
does not prove an IBP identity, nonzero pivot, or source lineage. Only the existing
original-source checker can certify the entire unchanged chart and saved order;
the existing checked-priority exporter independently repeats that proof and
retains the original rules and terminal inventory.

Only a typed `UnprovedDescentObligation` can add its complete offending shift to
the forbidden set. The witness records term ordinal, shift, local cell and child
support; it is not automatically a mathematical counterexample because the
native descent prover may be conservative. Every attempt is reported. The bank,
chart and order remain fixed, and each added shift is new and belongs to the
finite image universe. This bounds refinement but guarantees neither discovery
nor export. Missing targets, exhausted limits, zero guards, unsupported poles,
nonlinear/affine proof refusals and other errors do not trigger a new bank or a
narrower chart. Each new attempt rebuilds its own conservative pivot conditions;
rejected-attempt conditions remain in the trace rather than contaminating a
different final circuit.

Optional `trace_detail` is `full` (the unchanged default) or `summary`. Summary
removes only earlier candidates' rendered products, conditions and source-weight
lists. It retains every attempt, forbidden set, prefix, typed failure/cell,
new shift and replay flag, plus payload counts and the exact displayed full
product coefficient of each failed shift. The last actual candidate remains
complete even when followed by a terminal no-target or resource record; that
terminal record is also retained. A missing failed-shift product entry refuses
compaction rather than dropping its evidence. This is display-only: all native
products, guard retention, proof checks and exported bytes are unchanged. It is
intended to bound repeated diagnostic payloads, not expand mathematical budgets.
The compact-trace standalone build passed all 15 optimized example tests,
including full/default trace identity, actual native refinement equivalence,
byte-identical checked export and preservation of the last full candidate before
terminal no-target/resource records. Receipts are in ignored
`candidates/projector-summary-build-v1/` beneath the TMP root below.

The first supervised optimized build passed all 11 example tests. They include
genuine index-dependent forbidden cancellation, complete original-source replay,
a raising tadpole relation refined to a backward chart proof, successful direct
backward export with outside-chart fallback, and cancellation of an actual lower
tadpole child. The latter's chart `n >= 3` proved exactly but the original bridge
refused its lower bound. The bounded lower-cut extension now encodes that bound;
the same fixture still refuses export because retained pivot/source guards are
not represented by the surviving runtime RHS denominator. The unchanged guard
matcher does not infer divisibility or remove assumptions. No guard pruning or
chart widening is implied.
Build/test receipts, including the two corrected test-only failures, are in
ignored `TMP/rule-optimizer-20261003/candidates/symbolic-projector-build-v{1,2,3}/`.
The follow-up optimized core test target also passed 30 focused tests: all 18
indexed-context tests, all 11 original-source producer tests, and typed-error
context propagation. That includes the new native-ingress and failed-descent
fixtures. Its guarded compile took 1,009 seconds; the tests took 0.03 seconds
natively. Receipts are in `candidates/symbolic-projector-build-core-v1/` beneath
the same ignored TMP root. The prepared unoptimized fallback was not needed.
The canonical-native-result documentation clarification is a doc-only source
delta from the already frozen research producer binary, not an engine change.
Retained matrix/output bounds do not account for every native reducer scratch
allocation or intermediate operation, so owned time/RSS supervision remains
mandatory for all nontrivial probes. Native-result admission requires canonical
Symbolica output; it does not normalize arbitrary raw rational functions or
recover semantic scope provenance from a shared native variable map.

The first owner66 trial used the actually inspected rule-698 coordinate face:
14 fixed powers and only D2 free. The same 75 original rows were tried with
initial forbidden sets empty and H4, followed by bounded whole-chart descent
refinement. Both full-face `D2 >= 1` searches stopped on a genuine source-weight
pole. An unchanged rerun using the existing failure-only native diagnostic
located it at the `D2 = 1` sign subcell, not across the entire face. Those
refusals and the original face were retained.

A separately authorized `D2 >= 2` trial retained the other 14 fixed indices,
bank, order and algebra limits, leaving D2=1 to the old rule. Both arms reached
the initial 32-refinement allowance. One explicit allowance-only retry used 75
refinements and a 256 MiB report limit: a newly forbidden column has nonzero
contraction with the current source vector, which annihilates all prior
forbidden columns, so the forbidden-column rank strictly rises within a
75-row matrix. Both searches then visited all 75 rows and found no target under
their final forbidden sets, after 62 and 68 additions respectively. Independent
inspection sharpened the empty-arm result: all 62 banned shifts preserve the
same support across this entire univariate chart and are strictly higher under
the exact saved order. Their coefficients must therefore vanish in any uniformly
descending rational circuit on this face; merely permuting these same 75 rows
cannot rescue it. This is still a fixed-bank/chart limitation, not irreducibility
or a claim that no rule exists in a larger source span. The H4 arm also imposes
its separate requested child exclusions. No owner66 artifact or graph improvement
was produced.
The exact traces and compact paired summaries are in ignored
`TMP/rule-optimizer-20261003/candidates/symbolic-owner66-raw75-*`.

`limits.max_domain_bound_endpoint_cells` optionally sets the existing native
rule-proof endpoint-storage budget. Omission preserves 8,192; explicit values
must be positive `usize` values. This is distinct from `max_coordinate_cells`
and changes no source, guard or descent semantics. A private six-file research
snapshot, linked against the same core/app libraries, passed all 12 example
tests before the exact three-line policy change and fixture were transferred
back to the main tree after the core test build drained.

A separately authorized owner66 union retained the ordered raw75 bank and
appended 287 previously nominated RowId/offset pairs, seeding F with the ordinary
raw75 trial's final 62 columns. This heuristic union does not inherit kernel
protection or prove search completeness. With 362 rows and 909 image columns,
it added 32 forbidden shifts before native endpoint preflight refused
10,560 cells against 8,192. The last candidate used 21 original contributions;
its full original product replay passed, but chart proof did not complete.
One unchanged-math retry with the explicit endpoint limit set to one million
then exceeded its 512 MiB report allowance and emitted no result body. Its final
F, target and proof outcome are therefore unknown, not a finite-span miss or
proof success. Both processes drained; neither exported an artifact. Original
requests/refusals remain under ignored `candidates/symbolic-owner66-union362-*`
and `candidates/projector-endpoints-build-v1/`. Copied nested raw75 provenance
describes ancestors only; the union preregistration and actual request fields
define the larger experiment.

The optional boolean `forbid_fixed_outside_root_activations` (default false)
adds actual source-image columns to F when an explicitly fixed physical index,
plus that column's shift, becomes positive outside the loaded owner's root.
The checked widened sum does not classify unfixed axes. Derived columns and the
bound native root are reported separately; this is projection input selection,
never removal of a term from a proposed RHS. Native source replay and full chart
guard/descent proof remain mandatory.

Guard retention validates context and resource limits, rejects zero, then uses
the existing native `IndexedPolynomial::is_nonzero_constant()` to omit only
tautologies. Base-parameter and index-dependent guards remain live. A private
snapshot passed 20 tests and produced byte-identical checked tadpole exports
against the earlier immutable producer before these changes were transferred.
The combined tree also retains the independently tested source-support inspector.
Its merged 21-test optimized integration gate passed against the new optimized
app library, with no core or campaign CLI rebuild. The merged executable and
the immutable pre-change reference independently exported the same checked
tadpole candidate bytes. These actual build/test/control receipts are separate
from the private snapshot evidence, under ignored
`candidates/projector-combined-build-v1/` and `projector-combined-control-v1/`.

The signed-shell-enriched owner66 bank of 1,062 rows first stopped at the
100,000-condition entry cap, then at a genuine outside-root RHS after an explicit
one-million-entry retry. With fixed-root column selection, 287 root exclusions
were derived before solving. On `D2 >= 2`, proof stopped at a genuine final-source
`D2 - 2` pole. A separately registered `D2 >= 3` trial visited all 1,062 rows and
found no target after 474 descent refinements (final F count 823 including the
287 root exclusions). No owner artifact was exported. This remains a finite
bank/chart result, not general irreducibility. Exact sources, full final circuits,
native diagnostics, byte controls and lifecycle receipts are retained beneath
ignored `candidates/projector-root-policy-*` and `candidates/symbolic-owner66-*`.

The checked app exporter now supports a resource-bounded finite lower cut on a
free axis with no upper bound. It appends separate native singleton exclusions
for every omitted integer slice (active powers `1..lower`, inactive powers
`0..-(lower-1)`). Multiple slices/axes are OR branches, not one conjunction.
Total boundary count, physical endpoints, and existing-plus-new exclusion
entries are checked before boundary-sized allocation/arithmetic. Fixed axes,
finite-upper refusal, native source/chart proof, and runtime guard admission
are unchanged. After-baseline whole-piece dispatch keeps a crossing baseline
piece intact; it only substitutes the alternative when the whole piece fits.

The extension passed 14 focused app semantic tests (opt0, optimized dependencies)
and 21 optimized research tests against an opt2/no-LTO app library. Successful
lower2/3 exports, below-bound fallback, above-bound whole-piece selection,
crossing-piece fallback, active/inactive predicates, multi-axis OR, limits and
unsupported guards are covered. The original one-fixture expectation failure
is retained: the two-step tadpole now reaches the existing guard check rather
than failing at bound encoding. Its corrected fixture asserts that exact native
refusal. No core or campaign CLI rebuild was performed. Receipts are in ignored
`candidates/lower-cut-build-v1/` and `lower-cut-testfix-build-v1/`.

Actual before/after compatibility exports also passed: the unchanged symbolic
tadpole and the unchanged radial candidate with free lower0/1 axes each produced
byte-identical candidate payloads against their frozen older producers. The
radial producer was relinked with the new app library without changing its
source. These controls are recorded in ignored
`candidates/lower-cut-controls-v1/`; they are export compatibility checks, not
new downstream performance measurements.

`forbid_cofinally_higher_columns` is an optional boolean (default false) for
an independent product of genuinely unbounded coordinates, every other
coordinate fixed, unshifted family powers, and plain saved Spired order. It
derives each actual source-image column's componentwise exact sign-stabilization
thresholds, then uses the
native support/shift ordering keys to identify columns that are higher on the
entire product integer tail. A rational coefficient in a uniformly descending
rule must annihilate such a column identically: a nonzero polynomial cannot
vanish on that whole product tail, even after excluding finitely many nonzero
guard/pole loci. This is a necessary exclusion, not full-chart descent; finite
boundary faces still require the existing exact proof and refinement. The finite native interior is only
a checked carrier, not an empirical or finite-box proof of cofinality. Unsupported
orders, affine/correlated/rank-capped/power-capped charts, finite nonfixed axes,
charts without an unbounded axis, overflow, and witness
storage over budget refuse. This is the unprojected strict-descent contract;
it does not waive known-zero sectors or delete any source/RHS terms. Derived
mandatory columns are reported separately from caller-selected optional cost
exclusions, and native full-source/chart/guard proof remains required.
Single-axis witness and annotation formats are unchanged. Multifree witnesses
report the free-axis list and its threshold vector, including an explicit
product-tail scope when the derived exclusion list is empty.
The orthant extension passes all 75 optimized research tests, including native
mixed-sign and support-tie comparisons, finite-face non-exclusion, restrictive
geometry refusal, carrier overflow, witness admission, and the existing
474-column regression. Both the toy export and the actual negative-ray exact
export remain byte-identical to the preceding build; the actual full report is
identical except for elapsed seconds. Evidence is retained under ignored
`candidates/projector-orthant-build-v1/` and
`candidates/projector-orthant-one-ray-control-v1/`. These establish compatibility
and nomination correctness, not faster traversal or family closure.

`RUSTRED_SYMBOLIC_PROJECTOR_PROGRESS=1` enables observational stderr JSON for
source preparation, projection, proof/refinement and export. Sparse row
heartbeats are throttled to at most one per second; disabled payloads are lazy.
Progress never supplies algebra or authority inputs. The combined cofinal,
progress and lower-cut build passed 32 optimized research tests, including the
474 previously observed higher shifts and a finite-corner counterexample.
Default, explicit-off and enabled cofinal checked tadpole exports all match the
frozen reference bytes. The tested private source was transferred unchanged,
then the observational modules received a formatting-only rustfmt pass; the
compiled private snapshot remains retained. Build/control receipts are in
ignored `candidates/projector-cofinal-progress-build-v1/` and
`candidates/cofinal-controls-v1/`; these are correctness gates, not a graph-gain
claim.

`projection_backend` optionally selects `"direct-l"`; absent or `"augmented"`
keeps the existing identity-augmented algorithm. Direct-L reduces only the
physical `[F, target]` columns with native Symbolica, records accepted input/U/L
row correspondence, and recovers a target through one native solve of the
accepted triangular `L` transpose. It verifies that solve and the complete
original-source product. All input and physical-pivot guards remain live;
dependent and empty rows are not mistaken for accepted source rows. Resource
checks cover retained matrices and recovery inputs, with the outer RSS/time
guard bounding unobservable native scratch. This remains research adapter
orchestration, not a new algebra or proof kernel.

The merged optional backend passed 39 optimized tests. Actual cofinal-enabled
positive proof reports agree between backends, and three checked toy exports
are byte-identical. A separately frozen actual owner66 1062-source/final823-F
comparison completed with identical full reports (apart from elapsed seconds
and the backend selector), including the same 47 conditions and finite target
miss. That miss does not exercise target recovery; its positive coverage comes
from the source-replay fixtures and checked exports. Receipts are in ignored
`candidates/projector-direct-l-cofinal-build-v1/`,
`candidates/direct-l-cofinal-controls-v1/`, and
`candidates/direct-l-fixed1062-comparison-v1.json`. None is a downstream cost,
closure, or campaign-speed claim. The larger 5796-source trial is separate.

`nominate OWNER REQUEST` is an observation-only mode using the same complete
ordered source bank, chart, saved order, and root/cofinal F preparation as exact
projection. Add a `modular_nomination` object with positive `max_samples`,
`max_reducer_dense_scan_work`, and an explicit `samples` array. Each sample has
a native-validated `prime`, named `base_parameter_residues`, and exact signed
`physical_indices` inside the unchanged chart. The explicit scan allowance
bounds cumulative native dual-reducer width work; the other source, matrix,
trace and coordinate caps derive from the existing request limits and are
reported. There is no automatic prime or point retry.

The public `rustred::foundry::modular_nomination::PreparedOrdinaryNomination`
borrows one validated exact ordinary-source corpus across samples. It reuses
the existing direct shifted evaluator and Spired modular kernel, including
complete structural-F registration even when a residue is zero. Reports retain
sample ranks, source visitation positions, dependency-ordered original source
requests, trace size and elapsed time, but no sampled weights or proof token.
Singular conditions/denominators are unlucky samples; a sampled hit or miss can
both be misleading through rank specialization. Prove/export modes reject the
sample configuration, and nomination returns before any exact proof/export.

Any later shortlist must retain the complete original F (absent selected-row
columns are structurally zero, not removed), unchanged chart/order, and all
explicit original/input assumptions. Both exact projection backends validate
F arity and forbid the target in F while accepting these exact zero columns.
Only independently regenerated full source replay and native chart/guard/order
proof can authorize export. The new core wrapper's five focused tests, 21
existing modular-kernel tests and ten shifted-evaluator tests passed with
semantic opt0 tests against optimized dependencies; the public runtime library
was separately built at opt2 with LTO disabled. Old libraries and campaign
binaries were not overwritten.

The merged nomination driver and structural-zero F handling passed all 43
optimized research tests. The original standalone cfg-test environment omission,
two test-call qualification errors, and the obsolete absent-F rejection
expectation are preserved with their corrected attempts; none required a
runtime algebra change. The observation-only sample batch was explicitly
authorized while that last test-only link was pending; the 43-test gate is now
complete. Actual receipts are under ignored `candidates/modular-nomination-*`,
`modular-existing-tests-v3/`, and `modular-driver-*`.

The first unchanged owner66 5,796-row bank nomination completed three explicit
samples on the same `D2 >= 3` chart and complete F of 6,448 columns. At free
physical powers 31, 47 and 3 (two validated primes, distinct dimension residues),
each visited all 5,796 rows and 120,959 structural terms. All three returned
`SAMPLED_MISS`, with forbidden and augmented ranks both 4,932, in approximately
0.7 seconds per sample. There was no nominated support to lift, no exact
nonexistence conclusion and no exported owner. The entire guarded observation
took 13.97 seconds; source preparation and bounded report rendering are separate
from the sample times. Evidence is under ignored
`candidates/symbolic-owner66-modular5796-v1/`.

That bank subsequently proved to have an unestablished source recenter: saved
canonical targets do not encode the raw winning pivot's displacement. The v2
source-support inspector above now reports unknown recentering or an explicit,
unverified caller nomination. On this actual case, shifting the bank by−eD2
and regenerating complete root/cofinal F yields target supports in all three
samples. This remains discovery evidence, not replay of the saved identity.

`exact_source_ordinals` optionally supplies a nonempty, duplicate-free list of
positions in the complete, explicitly ordered `sources` request. It controls
the exact frame only: complete original-source preparation, full image universe,
root/cofinal F and all input guards are established **before** selection. The
adapter moves the selected images and their original-source weight maps without
copying them, retaining all original rows/bindings/guards for final replay.
Absent selected-row F columns stay present as exact structural zeros. Explicit
visitation order is preserved; selection is refused in modular nomination mode.
A selected-frame miss is labelled `NO_TARGET_IN_SELECTED_EXACT_FRAME_WITH_CURRENT_F`,
not a conclusion about the entire original bank. No sampled weights are accepted.

`compact_coefficient_variables: true` optionally removes globally absent
variables from the native reducer's coefficient map, following the production
`FrameVariables` implementation and using Symbolica's own variable rearrangement.
Both numerators and denominators determine the active map; integral coordinates
are not changed. Coefficients are restored to the authenticated original context
before guards, returned weights and full-source replay. Both exact backends
support it; false/absent preserves the previous path. It is coefficient storage
and arithmetic specialization, not a new CAS or reconstruction algorithm.

The combined research suite passes53 optimized tests, and the corrected source
inspector passes9 app tests. The first merged test package omitted a JSON fixture;
the next run exposed an incorrect byte-control assumption about an unused
forward-pivot guard. Both failed attempts are retained. The corrected test still
asserts the full-prefix export refusal and compares successful selected bytes
against the established single-backward-source export; no guard is waived.
Evidence: ignored `candidates/source-recenter-build-v1/`,
`projector-selected-compact-build-v1/` and
`projector-selected-compact-tests-v3-build-v1/`.

### Native source-weight reconstruction

The optional `projection_backend: "source-weights"` reuses RustRed's existing
Symbolica-backed source-weight reconstruction service. It differs from accepting
finite-field weights: Symbolica reconstructs rational functions, then an exact
full-column source product must equal the reconstructed target identity. The
explicit complete forbidden-column prefix, target, remaining image columns and
original source positions are retained. Original coefficient maps are restored;
globally absent variables are compacted internally without specialization.

This backend requires an explicit `source_weight_reconstruction` object with
positive `max_degree`, `max_probes`, `max_attempts`, `max_primes` (at least two),
`max_cached_images`, `max_cached_values` and `max_weight_slots`. The existing
matrix, coefficient and exact-arithmetic allowances still apply. Reconstruction
budgets are not an aggregate wall-time/RSS limit: retain the outer process guard.
There is no automatic exact-GPLU fallback after reconstruction failure and no
claim that a sampled admission miss proves absence of an exact rule.

All original conditions and input denominators remain mandatory. Actual
reconstructed weight denominators enter the common exact replay **before**
cancellation. Numerical pivot samples supply discovery only; their nonzero
values are never promoted to everywhere-nonzero guard claims. The reconstructed
identity still passes full original-source composition, regenerated-source
checking, chart/guard/descent proof and checked export. By default this uses the
existing conservative condition inventory; selecting this backend alone does
not opt into the distinct fresh-condition policy below. The reconstruction
integration passed 56 research tests; operational proof remains a separate gate.

### Fresh original-source certificate (research opt-in)

`fresh_original_source_certificate` defaults to false. When true, the driver
first performs the unchanged complete source composition and exact full-image
replay, then constructs a **new** native `OriginalSourceCombinationRequest` from
the final ordinary contributions. It never edits an older sealed proof. Both
constructor provenance and actual coefficients are checked: the current span
must be an ordinary identity frame or an injective selection/permutation, with
one unit weight per distinct original binding and each image exactly equal to
that original row. Weighted/kernel constructors remain ineligible even when
their supplied weight matrix happens to resemble an identity.

The new request retains **all** original span assumptions, including unused
source rows; there is no origin-string filtering. The native checker regenerates
the actual final source, weight and RHS conditions with their proper origins.
Genuine final poles remain mandatory. The earlier elimination/composition guard
inventory remains visible as diagnostic evidence, separately from the fresh
request's `retained_input_conditions`. The checked exporter, chart/root/order
proof and publication rules are unchanged; this is not a guard waiver.

All 62 tests pass in both semantic and optimized builds, including six new tests: default report
identity; full two-row versus backward-only checked export bytes across all
three backends; genuine index-pole refusal; preservation of an unused original
guard; weighted/mutated/context rejection; and a base-field proof-origin
differential with an explicitly nonempty RHS. Its revised fixed-index fixture
retains both lower terms through full native replay: a genuine final source
weight pole must be distinguished from an added original-domain assumption.
The earlier empty-RHS fixture's native refusal and failed test-package receipts
remain preserved. Optimized test binary `60264321…` runs the 62 tests in 1.32s;
the frozen normal executable is `dd4b5c5a…`. Evidence:
`TMP/rule-optimizer-20261003/candidates/projector-reconstructed-build-v6/`.
These tests establish the research interface, not an operational five-loop gain.

### Positive-power envelope (research opt-in)

`max_positive_power_excess` is an optional nonnegative integer. Omission leaves
the existing path unchanged. When supplied, it adds an optional cost restriction
to F using only columns present in the complete native, fixed-specialized source
universe, before any exact row selection or modular nomination. No source, term,
guard, or existing forbidden column is removed. The current target is the zero
shift relative to the declared physical chart; the limit is relative to that
target's positive-power sum, not an assumed free-index value alone.

The initial scope is one positive unbounded index with every other index fixed,
unshifted family powers, and no extra affine/correlated restrictions. For each
column, native complexity-key dots plus active propagators give its positive
power sum. The excess is `C + max(n+s, 0) - n`, whose maximum on this ray is at
the exact lower corner, including sign crossings and newly active fixed axes.
All arithmetic and witness storage are bounded; unsupported charts refuse.

Whole-column cancellation is a deliberate cost policy, not a necessary descent
condition: it can exclude circuits whose coefficients vanish on a finite
violating boundary. Reports retain the actual universe count, all selected
shifts with lower-corner witnesses, and the number newly added to F. A modular
hit remains discovery only; unchanged full-source replay, chart/guard/descent
proof, checked export and shared downstream measurement are still required.
All 68 optimized tests pass, including six envelope tests for exact crossings,
fixed-axis activation, coordinate permutation, unsupported geometry and
overflow/resource refusal. The native tadpole test adds a genuinely new F
column, preserves the full source/guard proof, and exports identical bytes.
Prior/current default optimized exports also match byte-for-byte. Evidence is
under `TMP/rule-optimizer-20261003/candidates/projector-envelope-build-v2/`.
The frozen 5,796-source pilot completed both A-only and H55-plus-A arms. From
12,230 actual columns, the envelope selected 4,544 and added 867 new forbidden
columns. All three explicit samples hit in each arm: A-only used F size 7,141
and nominated 2,803 sources; H55-plus-A used F size 7,196 and nominated 2,914.
Each arm returned identical source ordinals across its three samples. Evidence:
`TMP/rule-optimizer-20261003/incidence-study/positive-power-envelope-v2/`.
These are modular feasibility observations, not exact proofs or work savings.
Only the joint arm is scheduled for exact reconstruction; the previous H55-only
rule was source-proved but increased shared domain work by about 0.50%.

### Endpoint-axis locality (research opt-in)

Portable example: [two tracked all25-row requests and a direct Cargo/native
prove/export runbook](examples/endpoint_locality/README.md). You must supply
the compatible native owner bundle; no campaign bank or output is tracked.

`forbid_endpoint_changes_on_axes` is an optional sorted, unique list of
zero-based index axes. Absent or `[]` preserves the existing search and report
(apart from the literal request echo). A nonempty list adds every actual
post-fixed source-image shift that changes any listed axis to F, before exact
row selection or modular nomination. It unions, never replaces, the explicit,
root, cofinal and envelope constraints. Nonlocal source rows remain available:
Symbolica cancels their **combined endpoint coefficients**, not each row
separately. This is a sufficient locality restriction, not a new identity space
or a claim that all useful rules preserve these axes.

The helper admits column count, worst-case additional F-coordinate storage and
axis-inspection work against the existing `max_augmented_columns`,
`max_coordinate_cells` and `max_term_operations` phase allowances before
retaining shifts. Outer time/RSS limits still govern total work and native
scratch. Reports add full-universe, locality-forbidden and newly-added counts
only when enabled. Exact original-source replay has an additional residual
locality check; all original conditions, pivot poles, chart/descent/root checks
and checked-export gates remain unchanged. A modular hit is not a proof, and
rejection of the first exact candidate does not exhaust other combinations.

The cached-library optimized suite passes110 tests, including eight locality
tests. New/old toy owner and request bytes agree, and the new native unit export
matches the prior unit/native export exactly. Evidence is under
`TMP/rule-optimizer-20261003/candidates/endpoint-locality-build-v1/`; this is
implementation/default-off evidence. A subsequent fixed prove-only pair on
the two previously nominated charts discovered both local seven-tail circuits
from25 unweighted ordinary rows, with full native source/guard/descent proof.
Charts/windows/spectator axes remained hand-informed; no export or new workload
comparison followed. See the source-nomination section of
`docs/research/banana_moment_nomination_2026-10-04.md` for the bounded result
and its distinction from the earlier unconstrained first-target negative.

### General ordinary-span obstruction diagnostic (experimental)

Set top-level `source_obstruction_diagnostic: true` on a general
`symbolic_projector prove` request to diagnose a completed ordinary-bank
`NoTarget`. The default is off. This mode requires zero refinements and refuses
export, selected-row frames, modular nomination, reconstructed weights and
boundary-polynomial/correction modes. It does not alter the original miss,
source bank, forbidden columns, chart or saved order.

Two additional positive fields in `limits` are required only when enabled:

- `max_obstruction_preimage_sources`: maximum complete deduplicated diagnostic
  source census, not the projection's `max_source_rows` allowance.
- `max_obstruction_translated_terms`: native term-entry allowance for that
  diagnostic translation. Other arithmetic, matrix, coefficient, guard,
  coordinate, report and external time/RSS limits remain in force.

The shared native Symbolica service solves for one separator on the complete
`F ∪ {target}` columns and independently checks every source product is zero
and the target coordinate is one. Its one normalization equation is charged
separately from the actual source-bank cap. The diagnostic then enumerates
raw ordinary-support preimages `t=f−tau`, translates before fixed restriction,
and computes exact unnormalized pairings without pinch, rank, activation or
target-presence filters. A census/translation refusal never selects a prefix.

Results appear under `attempts[].source_obstruction_diagnostic`, with separate
separator and preimage completion states. This is a generic rational-function
finite-span witness, not a rule, pointwise impossibility or family certificate.
Incoming and coefficient-pole conditions remain explicit. A nonzero pairing
may vanish on every point of a correlated finite demand, and new columns may
introduce additional forbidden obligations; it is not evidence of feasibility
or benefit. No source is automatically added. The shared-service revision
passes all174 tests; the original168 remain included.

### Pinched-source boundary correction (experimental)

The optional `boundary_correction` object requests a two-stage search. First,
the ordinary top-level source bank must produce a full-original relation and
pass its native whole-chart proof. That **actual newly proved relation** is
the baseline, not an imported saved-rule identity or a zero-sector quotient.
Second, an explicitly admitted pinched ordinary-source bank supplies weighted
corrections. No coefficient displays or user-supplied weights are parsed.

The object has these fields:

| Field | Meaning |
|---|---|
| `schema` | `rustred.boundary-correction.v1` |
| `fixed_pinch_axis` | Zero-based axis fixed to a positive value on the entire parent chart. |
| `correction_sources` | Native RowId/offset pairs. By default these must be the complete ordinary inventory at the single offset that makes the fixed pinch power zero; see the translated opt-in below. |
| `translated_pinch_sources` | Optional boolean, default `false`; admit an explicit finite, duplicate-free translated RowId/offset bank with nonpositive source pinch power. Source rank and other-axis activation are not prefiltered. Every complete native image must still have zero target and zero unpinched terms. |
| `cancel_rank_positive_shifts` | Explicit full-arity shifts of nonzero baseline terms to cancel; each must be a uniformly pinched, rank-positive column. Empty means a zero-objective control, not an improvement. |
| `forbid_new_rank_positive` | `true` forbids rank-positive columns absent from the baseline. `false` requires the explicit rank cap below. |
| `max_numerator_rank` | Nonnegative integer, present only with `forbid_new_rank_positive:false`; bounds the sum of all negative endpoint powers over the whole chart, including the formerly active pinch axis. |
| `exact_dual_separator` | Optional boolean, default `false`; after a complete miss, request an exact diagnostic separator for the frozen forbidden-column system. It does not change the search or produce a rule. |
| `witness_preimage_nomination` | Optional boolean, default `false`, requiring `exact_dual_separator:true`; use the native separator support and complete raw ordinary inventory to enumerate translated source preimages and inspect their exact pairings. Nomination only: no new correction solve or bank mutation. |
| `export_guard_diagnostic` | Optional boolean, default `false`; after a successful correction proof, report typed cell/guard origins and the existing priority export guard-transport checks. Diagnostic only: no condition removal or admission-policy change. |
| `fresh_original_frame_reproof` | Optional boolean, default `false`; after the weighted correction proof, regenerate the complete declared original-source frame and independently prove the identical collected identity with fresh native provenance. Checked export requires this nested proof to succeed when requested; no fallback or exporter-policy relaxation. |

The initial interface requires all inactive parent indices to be explicitly
fixed to zero, while active indices may remain free. This is a rank-zero
**parent chart**, not a promise that its baseline descendants have rank zero.
It uses `prove` or checked `export` with zero refinements; modular nomination
and unsupported projection-mode combinations refuse. Absence of the option leaves the legacy
path unchanged. The top-level `fresh_original_source_certificate` applies to
stage one; stage two composes native weighted provenance and always undergoes
the full original-source check, not the unit-selection shortcut.

Checked export passes the successful typed stage-two request to the existing
native priority exporter; it never rebuilds coefficients from report strings
or falls back to exporting stage one after a correction refusal. The exporter
rechecks the source and the complete typed roundtrip, including RHS, case and
exceptions. Proof success does not guarantee that every retained guard can be
represented by the current runtime payload. A refusal preserves the complete
proof/report and publishes no corrected artifact. `original_source_replay_verified`
is the separate stage-two proof-success field; the weighted provenance policy
`fresh_original_source_certificate:false` is unchanged.

Every correction image must remain pinched, preserving the target and the
entire unpinched/same-support recurrence. A mandatory zero-correction control
replays the baseline through this same weighted path before searching. Check
all nominated columns against the complete actual **correction-only** image
universe: structural reachability or presence in the baseline is insufficient.
A missing member must not be silently removed from the objective. New scalar
pinches may survive only if the unchanged full source, guard, root and descent
checks prove them; all genuine incoming conditions and normalization poles
remain retained. The two boundary policies are distinct. Forbidding every new
rank-positive **column** also excludes trades into different rank-one descendants,
even when they could be cheaper downstream. A total-rank cap permits different
columns within the cap, but retains the same cancellation objective and all
native proof gates. Neither policy establishes downstream cost by itself.

Existing source/matrix/coefficient limits govern the combined declared bank
and weighted provenance. Set both source-row allowances deliberately:
`max_source_rows` bounds the combined inputs, and `max_complete_source_rows`
also bounds the composed original-source proof, not merely generator inventory.
Outer time/RSS supervision remains necessary. Reports keep stage-one proof,
zero-control outcome and stage-two miss/refusal/proof distinct. A complete miss
is confined to this frozen correction space; neither a hit nor fewer local
tails establishes lower campaign work. No owner or overlay is installed.

The implementation passed all168 focused tests. Its first fixed native probe
reproduced the313-tail stage-one proof and typed zero control; all nine nominated
columns were present, but the complete26-row weighted search found no correction
under the joint constraints. This is a finite-span negative, not a general
optimization or performance result. The fixed input, raw evidence and scope
limitations are documented in
[the conceptual search note](../../../docs/research/conceptual_rule_search_2026-10-04.md).
The sole matched cap1 probe removed all30 new-rank-one-column prohibitions
without changing the sources, chart or P9, and also exhausted all26 weighted
rows without a correction. Both typed zero controls passed. The fixed block
is parked; these misses do not establish a general optimization impossibility.

The optional separator diagnostic binds the exact forbidden columns and
independently checks that every correction row pairs to zero while the baseline
pairs to one. On the unchanged cap1 input it produced an exact two-column
separator supported entirely inside P9, with all25 correction-row products
zero and baseline product one. The completed search miss and zero control
were unchanged. All447 inherited forbidden columns were jointly zero in the
baseline and correction matrix. Coefficient displays are diagnostic only;
the rational-function result does not certify exceptional parameter slices.
It explains this frozen-span obstruction, not a master-integral functional,
physical rule or authority to grow the source bank or ignore guards. The
conceptual note records the exact column bindings and raw evidence.

The preimage diagnostic is a separate experimental step. It derives offsets
`t=f-tau` from a nonzero separator column `f` and
an actual raw ordinary-row shift `tau`, before fixed-index specialization.
It retains sources whose fixed pinch power becomes nonpositive, including
numerator-bearing seeds; it does not filter source rank or other-axis
activation. The complete deduplicated census must fit `max_source_rows`
before translation; a cap refusal does not translate a prefix. Existing
translation, coefficient, condition and output limits still apply. Reports
retain the raw support, all zero/nonzero pairings, source geometry and complete
translated image shifts. A nonzero pairing can motivate a source nomination,
but neither guarantees a feasible correction nor authorizes adding it to a
bank. No coefficient display is parsed or normalized into a new rule.

The first native census completed963 pairs but translation refused its10,000
term-entry allowance before any pairing. The preserved, explicitly authorized
32,768-term continuation admitted the measured20,826 entries without changing
the census or any other input. All963 exact pairings completed:605 nonzero and
358 zero, with no target/D7-restoration leaks. However,902 individual images
have rank2 or3 and268 activate another parent-inactive axis. These are
nomination data, not accepted corrections under the unchanged final rank cap1;
no new source bank, correction solve or workload claim follows automatically.

One subsequently preregistered solve kept the original strict P9/no-new-rank-
positive-column objective. It retained the old25-row block and added all25
ordinary rows at each of the three scalar, non-activating witness-breaking
offsets, for100 correction rows. The new binary first reproduced the old25
complete report exactly, apart from timing/request provenance. The actual100-
row solve then passed native full-original-source, guard and descent proof on
all four original cells:313 baseline tails became294, with93 rank-positive
shapes becoming81. All nine objectives disappeared, no new rank-positive
column appeared, and the unpinched recurrence was preserved. There were21
dropped shifts and two new scalar shifts. The guard lists grew from32 to40
entries per cell, retaining all old entries; applicability is not unchanged.

This is an accepted parametric correction, not a campaign improvement: no
export, installation, routing comparison or recursive workload followed in
this proof group. Stage two's `fresh_original_source_certificate:false` means
weighted/non-fresh provenance policy, not absence of the mandatory native
original-source proof. Full evidence and the obstruction-guided source-selection
mechanism are recorded in the conceptual note; the5.013s guarded two-proof
group is feasibility accounting only.

The subsequent independent checked-export pair produced the313-tail baseline
payload but refused the294-tail correction: `priority proof guard is not
retained by a surviving runtime RHS denominator`. The complete corrected
source/proof/control report still matched the successful proof. No guard was
removed and no corrected payload was published. The proposed three-arm
weighted-routing comparison therefore did not start; no runtime or workload
benefit has been measured for this correction.

A flag-only prove diagnostic subsequently preserved that complete proof/report
and checked all160 cell/guard pairs natively. Twenty checks failed transport
(3/6/4/7 across the four cells), all with `OriginalDomainCondition` provenance.
The report retains exact guard ordinals, native normalized denominator matches
and first/all mismatches; polynomial displays remain non-authoritative text.
No corrected artifact or routing result follows from this diagnostic.

A subsequent one-flag fresh-original-frame continuation regenerated391
declared bindings, replayed271 contributions and proved the same294 RHS terms
and four cells. The old weighted report remained unchanged. The fresh proof's
32 guards per cell passed the unchanged checked exporter and full typed
roundtrip: an independent corrected owner payload was produced,265→266 rules
with43 terminals retained. The earlier export refusal remains recorded; no
guard list was manually filtered. This is proof/export feasibility, not an
installation or cost result. The planned H1 three-arm routing comparison is
parked: the corrected stress chart is disjoint from all43 retained owner31
production sample records. An actual full-region observation instead selected
rules4/14; its two D11-face-exit terms share one target domain and do not yet
establish the next rule dispatched. See the conceptual note for these distinct
production and stress-test scopes.
