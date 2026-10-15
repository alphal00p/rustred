# Shared finite-demand closure: implementation and delivery

## Goal and evidence

Implement a generic, opt-in finite-demand execution path for
`FIVE_LOOP_CLOSURE_DELIVERY_PLAN.md`. Preserve every required query, exact
admission, legitimate descendant, guard and terminal convention. Do not change
production campaigns or regenerate their rules.

The R0 diagnostic accounted for all116 declarations,35 empty and81 nonempty.
Sharing one native graph reduced trace time51.706→13.078 seconds and operational
states1,215,215→300,673 with identical terminal/zero/selected-rule identity sets.
This is one fixed-order mechanism measurement, not an end-to-end/R6 speedup.
The earlier default per-query R0 result was negative and remains recorded in
`docs/research/finite_initial_queries_full_r0_2026-10-15.md`.
The positive diagnostic is in
`docs/research/finite_union_diagnostic_2026-10-15.md`.

The R6 count audit estimates339,607,800 input occurrences across declarations;
this is neither completed native enumeration nor a global distinct-key count.
Unbounded eager storage of all starts is therefore inappropriate.

## Architecture decision

Use a separate, default-off **finite-demand backend**, reusing the native routed
scheduler, scope parser, finite enumeration, storage utilities, inventory and
exact application. Do not graft multi-query cohorts into CP6's one-parent
ordered commits or reinterpret old symbolic ledgers. Existing symbolic and
finite-v3 behavior/defaults remain unchanged.

The new backend needs its own explicit format/version and run/stop/resume,
append, inspect, publication and manual-refinement dispatch. Unsupported or
resource-refused windows initially remain pending with a typed reason and exact
checkpoint. No silent import of approximate symbolic completion is permitted.

Start with independently reviewed core and enumeration slices. Neither alone
is a production backend or R6 delivery. Persistent authority follows their
audits; no elaborate compatibility project for old traversal ledgers.

## Native shared execution

Extend the existing parallel scheduler with the same census as the inline
path; use the existing kernel/worker budget, not another algebra engine/pool.

Only a successful globally drained trace may mint an opaque completed-trace
capability: all submitted input was validated and exhausted, no frontier,
failure or cancellation remains, and the census is complete. Local completion,
membership presence and public counters cannot establish recursive closure.

Retain full Work identity: physical key, Route/Apply phase and literal owner.
Starting admission precedes a hit even if a new start was an old descendant.
Existing source, intrinsic-key, support and strict-descent checks precede child
reuse. A hit references an earlier closed graph, never a master, zero or cached
coefficient value. Failed/cancelled batches do not extend the baseline.

First implement safe in-memory reuse between bounded batches tied to the same
immutable prepared reducer. Add the persistence seam only after approving its
execution binding and restore contract independently.

## Exact bounded input windows

Use Symbolica-backed `FiniteEntryDomain` count/enumeration. Do not implement
another combination iterator or resume by skipping millions of previous items.

A canonical fixed prefix uses active powers1+x and inactive powers−y. With
Afix=sum(1+x) and Rfix=sum(y), the exact remaining bounds are:

```text
Amax' = Amax - Afix
Rmax' = Rmax - Rfix
Dlo'  = Dlo  - Afix + Rfix
Dhi'  = Dhi  - Afix + Rfix
```

Use checked widened arithmetic/native conversions. Negative budgets prove
emptiness, not a clamped domain. All-fixed leaves are validated full-arity
singletons, not artificial zero-axis families. Require exact equivalence with
the original domain using existing envelope checks. Genuine nonuniform/coupled
constraints need their own equivalence proof or a typed refusal.

Enumerate a window only when its complete native count fits the buffered-input
grant. Scatter free keys back and validate original membership. Splitting a
coordinate uses its complete finite range, disjoint children and native count
conservation. Keep lazy DFS sibling frames rather than all children in RAM.
Validate the complete cursor/window chain, not merely a processed count.
Overlapping queries require separate occurrence, distinct-start and Work counts.
On interruption, restart only the uncommitted bounded window.

## Durable completed generations

Bind reuse to family/context/source conditions, ordered owners/overlays/routes,
saved ordinals, ordering/dispatch and raw terminal/zero convention. Retain all
original/appended query identities, roles and domains. Removing scope or changing
execution binding requires rejection or a fresh namespace.

Keep downstream master refinement separate from raw closure. Reusing terminal
substitutions at publication/application need not invalidate completed Work;
changing native dispatch or terminal authority does.

Use bounded immutable native integer-key segments and exact backing storage
with bounded resident indexes/pages. RAM eviction must consult backing storage,
not pretend a missing Work key while claiming once-per-Work accounting. Hash
filters cannot establish equality. Corrupt/unreadable authoritative segments
are errors. Reuse existing storage facilities; no new general database/CAS.

Each generation atomically binds membership, cumulative exact census, completed
windows, next cursor, execution binding and append-only scope. Sync immutable
parts before switching one manifest. A precommit crash retains the old generation;
never adopt orphan files or advance the cursor ahead of closure evidence.
Postcommit optional cleanup failure does not undo a durable generation.

Earlier completed input remains in the retained declared scope, so retain its global terminal/zero/
selected-rule census once and union new disjoint Work. No per-node expanded
coefficient/census is needed for this global result. Do not infer per-query
terminal sets. Distinguish buffered-input, live-work, index and disk grants.
Memory refusal leaves the current window pending; never deadlock workers that
hold unfinished expansions or clip descendants to fit a budget.

## Publication and lifecycle

Runtime application uses original owner rules/routes and exact back-substitution;
completed Work is not a rule. Supply the native census once, not per query.
Preserve optional manual refinement through append/resume. Distinguish fast
trusted-checkpoint loading from optional deep replay: hashes/structure are
integrity checks, not independent IBP derivation proofs.

Expose consistent Rust and saved-campaign steering, with full required-query
closure, window/occurrence counts, new Work/reuse hits, census, memory and
checkpoint age. A prefix or empty local queue is not all-query closure.

## Ownership and gates

- `intermediate_capture_app`: core complete-only/shared execution and tests.
- `weighted_vk_gate_plan`: exact native prefix windows/cursor and tests.
- `intermediate_capture_independent_audit`: independent mathematical/code/runtime
  review, with no self-approval of implementation.
- Root: integration, resource coordination, lifecycle/publication and delivery.

Core gates: local-done with unresolved child, cancel/failure at drain, phase/
owner/hash collisions, inadmissible starting hits, overlapping appended requests,
W=1/W=4 census equality. Enumeration gates: exact small-window union, active
baseline one, all-fixed/active/inactive, empty, integer overflow, mutated/skipped
cursor. Persistence gates: eviction/corruption, crash around commit, changed
binding, refine-then-extend, cold reload and exact nonterminal application.

Next performance/lifecycle gate is the complete232-row R0+R1 scope, not the
previous279-point single owner. Keep all116 original and amendment1 geometries
and roles. Use an input-controlled optimized runner, not rebuilds for each rank.
Compare complete native sets/work against separate queries, then isolated
checkpoint→append→resume→publication/export and exact coefficient canaries.
Historical855.250-second R1 at32 workers resumed R0 and is not a matched baseline.

R2 controls must exercise twelve-line parents absent at R1. Full R6 deployment
also requires bounded residency/storage and bounded restart, not only a closed
small prefix. No ETA follows from starting cardinality. Runtime pilots remain
≤30 minutes including preparation/shutdown, with compilation separate; use
disjoint leased cores/host headroom and preserve negative evidence. Commit/push
coherent audited slices under the requested identity and keep `CODEX_PROGRESS.md`
current. Production remains untouched until explicitly authorized.
