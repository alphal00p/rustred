# Finite replay: smallest publishable and resumable integration

Status: A1 implementation is pending independent interface and mathematical
review. The coordinator will authorize combined compilation after A1/A2 source
review. No new experiment, production campaign change or default-budget change
is authorized by this document; the full goal continues beyond these milestones.

## Evidence and scope

The same279-point hot R1/D10 region closed with existing native finite tracing:
641,239operational states,639,959physical keys,280declared terminals,
10,543,465projected endpoints,28.774s live and34.137s cold replay. Both executions
have identical non-timing counters and the same immutable owner payloads.
Evidence: `TMP/route-image-pilot-hot-r1-finite-resource-20261015/`.
This is neither an unrestricted family solve nor a publishable artifact today.

Two separately reviewable slices follow. Keep default-off behavior unchanged;
no fullR6 box enumeration, per-topology dispatch, new IBPs, terminal minimization
or custom algebra. Preserve original query geometry and roles exactly.

## A. Publish one existing completed finite recipe

### A1. Optional exact census from the native trace

Current core report already owns exact `declared_terminals` and `visited_zeros`
sets. It lacks selected owner/batch/rule identities. The app discards the sets
when adapting the result to `Outcome::Closed { recipe, work }`.

Add an optional trace census, disabled for existing ordinary methods. Proposed
plain data interface: rule identity `(physical_owner, batch_ordinal,
saved_rule_ordinal)` with checked event multiplicity, plus existing terminal and
zero keys. Use `PreparedRule.ordinal`, not its current vector position (preferred
subsets/overlays make those different). Bounds are maximum distinct identities
and bytes/entries, with a typed incomplete/error outcome if exhausted.

Record actual chosen-rule identity at the existing selection/application point,
only after successful native application. Do not rematch each integral, compute
all applicable identities, or build equations just to learn provenance. Keep
the native baseline/alternative dispatch and its applicability checks untouched.
Selected-rule census retains the same meaning as current inventory: chosen rule,
not every baseline guard that was checked. All original guard-bearing programs
remain in the package, so census does not replace source/guard authority.

Suggested files:

- Core `solver/candidate_reduction/routed/campaign/census.rs` (new small module),
  `campaign/{model,scheduler,worker,inline}.rs` for opt-in result/census plumbing.
- Minimal shared hooks in `owners/evaluation.rs` and `evaluator.rs` to return the
  already-selected saved ordinal and batch ordinal without a second CAS pass.
- Existing public trace methods remain wrappers with census disabled. A new
  explicit census-enabled finite entry point avoids changing ordinary cost or
  serialized output.

Do not retain all641kvisited integrals or all equations merely for inventory;
only the much smaller distinct rule set and already-owned terminal/zero sets.

### A2. Replay into the existing publication inventory

Keep current single-ID0 recipe and CP6 wire format for sliceA. Publication can
replay the authenticated recipe once, then merge exact census into the existing
`inventory::Collector`; no census needs to be trusted solely because it appeared
in diagnostic JSON. This also supports the already-produced successful checkpoint.

Suggested files:

- `walking/finite_replay/census.rs` (new) and `finite_replay.rs`: opt-in completed
  census output distinct from numeric `work`; never return a successful census
  from declined/cancelled/error outcomes.
- `walking/verify_closure/finite_replay.rs`: pass optional inventory collector
  into successful mandatory native replay; preserve all binding checks.
- `walking/verify_closure.rs`: remove blanket inventory refusal only after the
  new finite branch exists. Do not send a finite recipe to ordinary domain census.
- `verify_closure/inventory/{census,fast,cache}.rs` and `inventory.rs`: explicit
  finite-recipe classification. Fast publication must still native-replay the
  recipe for its census because no faithful terminal census is saved in CP6.
  Deep publication reuses the same replay, not a second trace.

There is an additional policy mismatch to resolve explicitly:
`owner_domain_walk_inventory` currently requiresAsRun, whereas finite-summary
cold admission requiresAll/Off. A finite branch must always use the exact native
recipe with symbolic shortcuts disabled, independently of ordinary AsRun census.
Do not simply relax the cold verifier requirement. Keep standalone coldAll/Off,
and test that mixed ordinary/finite inventory cannot substitute a broader
symbolic cover for finite replay. Record the two assurance boundaries separately.

Persist the resulting faithful census through the existing publication census
and artifact mechanisms, bound to request, recipe, CP6 record/payload hashes and
family identity. The existing census codec is bounded nativebincode, not canonical
coefficient strings. A finite-compatible census schema/version is required before
reusing a predecessor census; for the first minimal slice, refusing finite
census reuse and replaying it is safer than silently accepting an old cache.
Inspection afterward uses persisted inventory and does not re-run the trace.

The existing publication/export path then includes immutable owner programs,
routes, admitted source domains and exact terminal normalization/substitutions.
No new scalar application or master convention is introduced. Missing census,
unknown terminal, replay frontier or cancellation must prevent publication and
leave the predecessor artifact intact. Original rule guards/source conditions
remain attached and are checked by the existing exact application path.

### A3. Honest monitoring

Reuse existing snapshot fields to retain internal maximum numerator rank,
dot excess, actual completed/scheduled/queued states, failure work and phase.
One CP6 recipe must be displayed separately from hundreds of thousands of
internal exact expansions. Rule/terminal counts are concrete identities, not
independence claims. No top-k transport-cost framework is needed for sliceA.

### A acceptance

- Unit native census matches independently classified selected ordinals for
  batch precedence, preferred subsets, whole-piece alternatives, later terminals,
  source/exception guards and zero rules; disabled path is unchanged.
- Tiny fixtures with exact, uninflated symbolic images: finite and symbolic
  terminal keys agree, as do exact public application results in one terminal
  convention. For the real279-point case, do not demand equality with a broader
  symbolic-overcover census: extra symbolic terminals can be legitimate surplus.
  Compare actual package application against the original exact routed reducer
  for the same admitted canaries instead. Counts alone are insufficient.
- Partial trace, frontier, limit refusal, cancellation and forged/missing recipe
  cannot emit a complete inventory. Missing/duplicate seeds and geometry/owner/
  limits/order mutations fail closed using existing cold mutation seams.
- Publish the existing279-point checkpoint, cold-load exported package, inspect
  exact rule/terminal counts and reduce deterministic admitted canaries using
  `Prepared...Application`; compare exact coefficients and guards to the saved
  owner application. No new IBP generation and no master evaluation.
- Native binary/census roundtrip and cancelled publication preserve the prior
  manifest. Test normal fast and deep publication; neither double-replays finite.

## B. Completed-recipe resume and append-only scope extension

### B1. Explicit generalized recipe binding

Current blockers are deliberate: `validate_finite_replay` requires fresh CP6;
`Account` is oneAtomicBool; `original_initial` takes firstquery; inspector,
mergeP1, typed-record validation and cold loader all require domainID0; cold
loader stores one`Option<Recipe>`.

Generalize to recipes keyed by authenticated domainID/query-origin identity.
Use the exact existing input rows, query IDs/roles and amendment digest chain,
not `id < p0`: appended required roots can lie beyond the unchanged original
protected prefix. Build a bounded immutable eligible-root lookup at admission/
restore; validate equality of complete geometry/caps/source-validity status.
Aliases may reuse an already-complete root, but a containing domain, G2 residual,
partial anchor or descendant must never be relabeled as a whole finite query.

Suggested files:

- `finite_replay/{binding,account}.rs` (new focused modules),
  `epoch/inspector/finite_replay.rs`, `epoch/merge/finite_replay.rs`.
- `epoch/{job,records/typed}.rs`, `verify_closure.rs` and its finite module for
  per-domain recipes and multi-root replay. Keep accepted recipes tied to their
  actual domain and immutable owner snapshot, not only an integer ordinal.
- `walking/{mod,checkpoint}.rs` for opt-in semantic version/request marker;
  `epoch/checkpoint/{metadata,restore/runtime}.rs` and rescue append transaction
  for validating/restoring eligible roots and completed recipe records.

Use a new opt-in semantic recipe version for generalized multiple roots/resume;
default-off identity remains unchanged. Keep sliceA's v2 reading explicit if
desired, but do not reinterpret an old ID0-only checkpoint under new semantics.
Fresh v3 campaigns are acceptable and much safer than a broad migration project.

### B2. Small-domain admission, not fullR6 enumeration

Before allocating seeds, use the existing exact native count/mutual-enclosure
gate. Attempt only whole initial query regions within explicit seed/byte/layer
limits; larger or unsupported boxes continue ordinary symbolic work. No clipping,
prefix enumeration or reduction of admitted scope. Keep finite opt-in. Count
attempted finite work and symbolic fallback separately. Do not automatically set
all production limits toMAX based on the successful diagnostic.

Multiple initial regions can run at existing inspector concurrency, but inline
finite attempts each own a physical-key map. Start with one concurrent finite
attempt to avoid multiplying memory; this is a scheduling knob, not a topology
special case. Completed recipes can be reused on resume without replaying them
in the live solve; independent cold verification/publication still replays or
authenticates a matching completed census.

### B3. Atomic case boundaries, explicit restart semantics

Use existing atomic merge-boundary CP6 checkpointing. A completed finite recipe
is published as one authoritative record only after the exact trace succeeds;
interruption before that commit leaves the original query pending. Resuming may
restart only that last in-flight finite case, while keeping previously completed
recipes. Do not serialize transient Symbolica objects or invent a second journal.

The first slice does NOT persist in-flight FIFO/membership; the UI/docs must say
so. A future true mid-trace checkpoint would additionally persist full-key phase
membership, FIFO, active-to-pending reset, terminal/census sets and aggregate
counters. That is a separate feature, not required for bounded atomic cases.

An interrupted/declined attempt must not be installed as a failed permanent
native record that prevents retry. Track attempts by domain/invocation and allow
unfinished cases to restart on resume; prevent endless same-invocation retries.
Completed and failed provenance must not be inferred from diagnostic counters.

### B4. Append-only R0→R1

Reuse existing immutable scope-extension amendments: original R0 requests remain
required, appended R1 requests enlarge the union; they do not overwrite R0 or
change source templates. Reconstruct eligible domains from both original and
appended query rows. Already-closed aliased domains need no new work. New finite
regions may initially re-enumerate overlap (e.g. the9R0points within279R1points);
do not introduce a cross-request point memo in this minimal slice.

### B acceptance

- Tiny R0 finite completion→CP6reload→appendR1 finite completion agrees with
  one-shot R1 on exact terminal set and public reduction canaries; both original
  and appended required queries remain represented and independently checked.
- Interrupt before/after recipe publication and during the next finite case;
  restore keeps prior recipe bytes, requeues only uncommitted case, and cold
  verifies. No source-condition frontier becomes a terminal.
- Test multiple initial roots, aliases, nonzero lower/singleton eligibility,
  duplicated IDs, changed geometry, roles and malformed amendment chains; mixed
  finite/symbolic regions must all discharge, not merely the finite subset.
- Flags/budget/version changes refuse resume unless explicitly supported; off
  outputs/request identities remain byte-identical.
- Real reproducer: fresh hotR0D10(9points)→publish→resume unchanged→append the
  exact279-point hotR1D10query→publish/cold-load/app canaries. Resource guard as
  above; no largeR6 enumeration or production campaign mutation.
  Keep concrete coefficient canaries small and predetermined; the finite support
  trace does not measure the cost of complete coefficient back-substitution for
  all279inputs. Report any application resource limit explicitly, not as closure.

## Modular build and review sequence

Implement and independently audit A1 core census first, then A2/A3 app bridge,
compile once after both source lanes agree, and run focused native/app gates.
Freeze that publishable single-recipe milestone before B. Keep plain census and
binding types in small modules; avoid new closure-type monomorphizations and
duplicated arithmetic. A module split alone does not eliminate Rust crate/LTO
rebuilds, so do not promise that. All follow-up pilots remain input-driven with
the same frozen executable whenever possible.

No algebraic primitive is needed: reuse native selection, Symbolica-backed
specialization, transport and coefficient application. Before implementation,
confirm their pinned/current signatures and existing usages; do not add a
parallel CAS implementation or recompute identities for bookkeeping.

