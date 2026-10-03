# Bounded campaign profiling

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

The first command only prints. The second creates `plan.json` and each arm's
`selection-source.json` in a fresh directory, refusing an existing destination.
Exact stage/walk/cold argv arrays are included. The ordinary staging tool copies
owner/overlay payloads and preserves query bytes. Replacing a base-bound repair
overlay's owner requires a suitable native re-export; Python does not rebind it.

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
