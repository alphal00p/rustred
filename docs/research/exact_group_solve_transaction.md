# Exact-group `Solvej` row transaction

Status: authoritative design note and implementation checkpoint.

Scope: one topology-independent, exact-GMP affine group. This note defines the
ownership and transaction boundary joining raw-row replay, LiteRed-style
top-reduction, exact recentering, target selection, `WhenBad`, and publication.
It does not define loop-specific recurrences, infer master integrals, or permit
FORM as an implementation dependency.

The broader mathematical contract remains
`docs/research/litered_solvej_exact_group_database.md`. This note narrows that
contract to the APIs and invariants needed for an atomic implementation.

## Implemented checkpoint (updated 2026-08-25)

The topology-neutral authority and staging layers described below now exist:

- `generated_affine_residual_group_exact_database.rs` performs LiteRed-style
  hardest-only top reduction without mutation, returns a non-`Clone` staged
  row, and commits only after complete allocation/version/cursor checks.  Each
  database and each successfully staged transition have separate private,
  non-wrapping identities, so distinct databases and competing same-version
  rows cannot be cross-paired. Staged rows retain an opaque production or
  synthetic source recipe and share the exact dependent-reduction or
  new-pivot evidence that is later installed and recorded. An owning prepared
  database commit can be aborted during outer preflight; once admitted, its
  checked fail-stop boundary and infallible commit tail perform no allocation.
- `generated_affine_residual_group_exact_targets.rs` compiles the solve plan's
  persisted target order through the retained inventory authority.  It stores
  either a Ready premises certificate or a typed affine-equality-refinement
  certificate for every target.  Its immutable state is created only from an
  opaque database binding; retained target handles own the exact state `Arc`,
  and every successor binding names its exact predecessor transition rather
  than trusting adjacent numeric versions. Fallible successors preflight their
  retained/peak byte envelopes before allocating the copied disposition buffer
  and either preserve all dispositions or consume exactly one Ready handle.
- `generated_affine_residual_group_exact_session.rs` is the sole production
  owner joining the database and target-state allocation.  A staged session
  transaction seals the database row token together with the exact target
  state.  Its narrow joint view exposes only recentering data/provenance,
  target operations, and plan geometry.  `commit_unconsumed` preconstructs the
  target successor and then advances database and target versions together
  without consuming a target; this covers dependent rows and pivots rejected
  or deferred by later policy.
- Production database transition methods now require a non-constructible,
  non-`Clone` capability retained privately by that session.  Direct database
  staging/commit and raw plan access exist only as `cfg(test)` adapters, so a
  sibling production module cannot bypass the session's typed outcome policy.
- `generated_affine_residual_group_exact_recenter_kernel.rs` now contains the
  authority-free GMP/Symbolica arithmetic extracted from the legacy raw-row
  adapter.  It consumes exact-size borrowed exact-shift/coefficient/guard
  streams once into admitted stable reference buffers, computes
  `t = r - A r_F`, `delta_F = -r_F`, and
  `q = s - r`, and returns only inert centered values.  Its resource contract
  separates pre-existing live owners, additional retained output, and native
  scratch.  The legacy exact-relation compiler projects into this kernel and
  remains the differential oracle; the kernel accepts no raw relation,
  database/session token, solve plan, target bitmap, topology, or loop count.
- `generated_affine_residual_group_exact_session.rs` now owns the only
  production recentering entry point. It consumes a staged session transaction,
  authenticates the post-top-reduction pivot together with its exact target
  state, and returns a non-`Clone` NoTarget, affine-equality-refinement, or
  Ready typestate. Ordinary errors and caught Symbolica panics return the exact
  original transaction; matching and translation mutate neither database nor
  target state. Equality-bearing targets return before coefficient, shift, or
  guard translation. Ready outcomes retain translated row guards separately
  from the target's unmodified premises and expose no commit path.
- NoTarget now consumes the running session, commits its authenticated pivot,
  preserves every target disposition, and returns the sole continuation owner.
  An affine-equality outcome instead consumes the running session into a sealed
  refined-epoch suspension: its pivot is committed, its first matched target
  remains unresolved, and same-epoch staging cannot continue. Equality target
  and unconsumed successor state are minted as one capability-gated pair over
  the same exact allocation.
- The exact session now retains a private, append-only event for every consumed
  source. Each event shares the database's opaque source recipe and exact
  reduction/pivot evidence and records the authenticated disposition,
  transition versions, target information, exact offset where applicable, and
  cumulative resource statistics. Event-vector replacement, retained owners,
  target-state successor copies, comparisons, and replay work are all admitted
  against explicit cumulative limits before the commit tail.
- A retained production recipe now uses one overflow-checked source-graph
  census owned by the physical-row/re-elimination boundary. It includes the
  row, re-elimination owner, bound outcomes, elimination, and source-local
  authority/premises/ordering/schedule allocations. Only exact pointer-proven
  plan/frame ancestry and the common inventory pointee are excluded. Exact,
  one-byte-below, lifetime, and shared-anchor-deduplication tests cover that
  contract.
- Session replay is chronological rather than a final-state comparison. It
  builds a fresh shadow database/target/session owner, restages every opaque
  source recipe in order, reruns hardest-only reduction, recentering and target
  matching, compares the shared evidence and exact event disposition, and then
  compares terminal database, target, event, and resource state. A sealed
  equality suspension authenticates and replays its mandatory terminal event.
- `generated_affine_residual_group_ready_publication.rs` now implements the
  first Ready-consuming pre-publication analysis phase without committing the
  session. It reauthenticates the sealed transaction and selected target
  geometry, locates
  the unique unit zero-shift pivot, constructs every physical key from the
  exact target anchor plus centered GMP shifts, and proves each retained RHS is
  strictly easier under the persisted ordering. It also records every finite
  inactive-orthant activation interval exactly as `[1-q_i, 0]`, with Symbolica
  `Integer` bounds and counts. The successful typestate is deliberately named
  `ReadyForConditions`: it consumes no target and publishes no rule.
- Exact-local physical-key construction and its allocation-free prospective
  resource preflight use arbitrary-precision Symbolica integers throughout.
  There is no `IndexShift` or `i64` narrowing in this boundary. Unsupported
  non-descent is a mathematical terminal candidate outcome; currently
  unsupported non-independent-cylinder geometry is an operational, retryable
  `Pending` outcome and cannot be mistaken for a master or zero claim.

Adversarial tests cover value-equal foreign plans, two databases with identical
visible coordinates, competing transitions from one live version, attempted
successor laundering through an abandoned sibling branch, stale transactions,
target-state allocation siblings, production physical-row ingress, immutable
target successors, event/source/evidence `Arc` identity, cumulative and
one-below resource envelopes, and chronological fresh-shadow transition
replay. Recenter classification remains inert until a typed transition is
consumed; dependent, NoTarget, and equality transitions now advance the exact
state. None of these paths publishes a rule or infers a master.

The pre-kernel repository baseline used licensed, GMP-enabled Symbolica with
eight-way `cargo-nextest`: run
`0ba055e9-b517-4088-8d8c-ffd2bc28a4c9` passed all 1,392 tests (3 additional
tests intentionally skipped) in 1,073.130 seconds, followed by clean rustdoc
tests. Neither FORM nor Symbolica's `no_gmp` feature was used.

The extracted-kernel/session milestone was then checked with four-way,
licensed GMP `cargo-nextest` gates: run
`147bdfcb-dcb7-4d1b-bf1d-6374c5d5ff17` passed all 38 database/session/target
tests, and post-audit run `c0eeba4d-2693-4536-bec0-4a540f196572` passed all
13 exact-relation/kernel tests.  `cargo check --tests`, `cargo fmt --check`,
and `git diff --check` were clean.

The session-owned recenter wrapper was validated with four-way, licensed GMP
`cargo-nextest` run `bb510535-b3be-48c2-814a-fbd81794ead3`: all six focused
behavioral tests passed. They cover natural Ready translation with separate
target premises, NoTarget beyond `i64`, exact cancellation of a 4,096-bit free
coordinate, post-top-reduction leader selection, source-event provenance,
transaction recovery from foreign/stale/resource failures, and an
affine-equality return under zero translation budgets.

The combined database/session/target/kernel regression gate then passed all
47 tests in four-way run `9b961e8e-2a6a-4bac-af78-69242fdbe0f5`. After the
source-surface test was extended to name the synthetic `cfg(test)` ingress,
run `052dfde9-3226-4df4-a815-6c88f65d6bdb` passed all 15 selected
exact-relation/kernel/source-seal tests. Final `cargo check --tests`,
`cargo fmt --check`, and `git diff --check` were clean.

The typed NoTarget/equality transition milestone passed licensed, GMP-enabled
four-way runs `f8fefe69-c966-48eb-ada8-9bac85f24158` (sealed equality success,
foreign-owner rejection, exact-limit success, and one-below/zero-budget
recovery; 1/1), `e06c10c3-2f18-48ca-8eec-51a229972d82` (production authority
surface and compositional physical-row recipe lifetime/replay; 2/2), and
`4e1ef6e4-749e-4f2e-8f00-869059b61f20` (the other affected exact
database/session/target/recenter-kernel tests; 44/44). Direct
`cargo check --tests -j 4`, `cargo fmt --all -- --check`, and
`git diff --check` passed.

The subsequent frozen chronological-ledger gate passed licensed, GMP-enabled
four-way `cargo-nextest` runs `34021f1d-7458-4700-9ec9-4155cd338c39` (all 16
exact-session tests, 16/16), `75cec2fb-6846-4a1b-8df5-029b1331e717` (exact
database, physical-row, and recenter-kernel tests, 42/42), and
`ff9310fa-fb3b-42e2-b79f-531fc93708ad` (the complete retained source-parent
graph gate, 62/62). `cargo check --all-targets -j 4`, `cargo fmt --all --
--check`, and `git diff --check` also passed. Neither FORM nor Symbolica's
`no_gmp` feature was used.

The frozen exact Ready geometry/descent/hazard phase passed independent,
licensed GMP validation with explicit `--lib -j4`: run
`a06d5558-e404-4048-a2e9-5407277a95d6` passed all 11 tests in the independent
Ready/publication validation module (11/11, 985 skipped), and run
`f74b89eb-1e59-4628-91d7-82af1f11b893`
passed the two internal Ready units plus the physical-key comparison witness
(3/3, 993 skipped).
The independent gate reaches Ready through generated one-loop IBPs rather than
injecting a recurrence, checks recoverable authentication and exact one-below
resource failures, and exercises 4,096-bit descent and hazard coordinates. A
separate fast `L=6`, `K=21` coordinate-family gate proves the generic generator
emits 36 ordered ordinary IBPs with deterministic manifests; it stops before
the currently eager Boolean-cover path and is not a six-loop reduction. A
separate existing generic-provider oracle baseline fully reduces tadpole powers
two through four against frozen Vakint scalar coefficients; it does not pass
through the unpublished `ReadyForConditions` path and is not evidence that
current-lineage publication is complete. The concrete tadpole remains
validation data only; production contains no loop-count, topology-name, or
hard-coded recurrence dispatch.

The production recipe tests are intentionally compositional: they prove
genuine physical-row Arc identity, survival after all staging owners drop,
event retention, chronological fresh-shadow replay, and final release. The
complete equality transition uses the sealed synthetic test adapter because
current physical-row construction skips equality-premise source cases. An
end-to-end production equality row remains a future refined-epoch gate.

The next missing mathematical seam begins after `ReadyForConditions`: compile
the Ready-native condition transcript, build the relative applicable and
exceptional partition, and atomically publish guarded rules/residual leaves
with chronological replay. The event ledger currently records `Dependent`,
`NoTarget`, and mandatory affine-equality-refinement transitions; it does not
yet record unpublished future `WhenBad`/rule/residual leaves. The old raw
`generated_affine_residual_group_exact_relation.rs` compiler remains a
differential oracle only; it is not a production authority.

Condition compilation must consume the already-centered row directly. The
recenter kernel has already applied `n_F -> n_F-r_F` to row coefficients and
row guards, while target premises already describe the selected target affine
domain. Applying `Ready::coefficient_translation()` a second time would be
mathematically wrong. Stable transcript order is target premises, translated
row guards, pivot denominator, then RHS denominators in retained descent order.
Each denominator contributes LiteRed's parameter-polynomial identity clause:
all Symbolica-projected parameter coefficients vanish. It must not be replaced
by the pointwise predicate that the full denominator vanishes in
`Q(lambda)[n]`.

All algebra in that seam must continue to use Symbolica's public APIs. The
concrete API inventory is
[`symbolica_exact_linear_algebra_api_inventory.md`](symbolica_exact_linear_algebra_api_inventory.md),
and the prioritized audit is
[`symbolica_first_algebra_migration_audit_2026-08-24.md`](symbolica_first_algebra_migration_audit_2026-08-24.md).
RustRed must not grow a parallel CAS or matrix layer. Full LiteRed parity,
arbitrary one-loop pentagon reduction, and the high-throughput two- through
six-loop single-scale vacuum milestones remain pending behind the generic
condition/publication/residual pipeline.

## 1. Normative source seams

LiteRed's default `Solvej[eq, db]` repeatedly substitutes only the current
hardest known integral; the first unknown hardest integral is normalized and
inserted immediately
(`vendor/LiteRed2/Source/LiteRed2026.m:2164-2195`). `SolvejSector` clears the
database once per affine group, submits generated rows in order, recenters a
returned pivot, selects a case, and only then compiles `WhenBad`
(`vendor/LiteRed2/Source/LiteRed2026.m:2439-2505`). A failed `WhenBad` candidate
is excluded from being returned again but remains in the algebraic database
(`vendor/LiteRed2/Source/LiteRed2026.m:2501-2505`). The pending source prefix is
consumed in source order (`vendor/LiteRed2/Source/LiteRed2026.m:2648-2659`), and
the exact bad-locus calculation is at
`vendor/LiteRed2/Source/LiteRed2026.m:2565-2569`.

The source audit that motivated the implementation identified four relevant
but then-unjoined seams.  The first two have since been replaced/joined by the
checkpoint above; the descriptions remain here to explain why the new
authority boundary was necessary:

1. An exact physical row retains its authenticated re-elimination source,
   frame, row/witness locators, physical terms, and guards
   (`src/generated_affine_residual_group_exact_physical_row.rs:275-320`). Its
   `replay_for_database` method supplies a sealed borrowed ingress
   (`src/generated_affine_residual_group_exact_physical_row.rs:471-485`). The
   retained `Arc` is therefore the row's replay recipe.
2. The pre-transaction exact database authenticated that row and immediately
   ingested it mutably
   (`src/generated_affine_residual_group_exact_database.rs` at the historical
   audit revision).
   Its hardest-only loop is the required LiteRed algebra
   (`src/generated_affine_residual_group_exact_database.rs:637-742`). It
   constructs a normalized pivot and complete replacement buffers
   (`src/generated_affine_residual_group_exact_database.rs:744-847`), but then
   commits the pivot, lookup, statistics, and cursor before target processing
   (`src/generated_affine_residual_group_exact_database.rs:848-870`). The
   dependent path likewise advances the cursor immediately
   (`src/generated_affine_residual_group_exact_database.rs:654-664`).
3. The exact-relation prototype rebuilds a raw retained relation, chooses that
   raw relation's maximum, and takes a caller-provided unresolved-target bitmap
   (`src/generated_affine_residual_group_exact_relation.rs:519-595`,
   `src/generated_affine_residual_group_exact_relation.rs:794-840`). Its GMP
   geometry and translation kernels are useful
   (`src/generated_affine_residual_group_exact_relation.rs:802-889`,
   `src/generated_affine_residual_group_exact_relation.rs:1113-1147`), but its
   input authority is not the post-database pivot authority.
4. The mature `WhenBad` and publication state machine belongs to the older
   `GeneratedResidualAffine...` lineage. Its authenticated input owns the old
   matcher/relation graph
   (`src/generated_residual_affine_when_bad_compilation.rs:1151-1227`), its
   geometry stores `IndexShift` rather than arbitrary-precision shifts
   (`src/generated_residual_affine_when_bad_compilation.rs:264-308`), and its
   terminal classifier is `Certified | IdenticallyBad | Unsupported`
   (`src/generated_residual_affine_when_bad_compilation.rs:6801-6807`). The old
   coverage owner has the correct target transition shape—only `Certified`
   consumes a target
   (`src/generated_residual_affine_group_effective_coverage.rs:631-706`)—but
   its handles retain the old compilation authority
   (`src/generated_residual_affine_group_effective_coverage.rs:352-520`).

Consequently, existing polynomial/partition algorithms may be factored and
reused, but old matcher, `WhenBad`, coverage, and rule certificates cannot
authenticate a new exact-group pivot merely by copying ordinals.

## 2. The single mutable owner

Introduce one crate-private `ExactGroupSolveOwner`. It is the only object
allowed to mutate the state of a running group and owns:

```text
immutable authority:
    family and coefficient-context identity/fingerprint
    exact inventory, solve plan, physical frame, ordering policy
    group ordinal and database epoch
    source/re-elimination authority and all resource limits

mutable state:
    exact algebra database: pivots and exact-key lookup
    pending authenticated batch and next source cursor
    targets[solve_ordinal]: Unresolved | Consumed
    append-only row/candidate/transition events
    sealed published rule leaves
    exact exceptional residual-work leaves
    aggregate statistics and monotonically increasing state_version
```

Owner identity is a private `Arc<ExactGroupSolveAuthority>` retained by both
the owner and every staged token; it is not the movable Rust address of the
owner struct. Authentication uses `Arc::ptr_eq`, while fingerprints/manifests
remain replay evidence rather than substitutes for allocation authority.

The solve plan's locator is currently only a scalar triple
(`src/generated_affine_residual_group_solve_plan.rs:308-324`) and targets are
exposed as an ordered slice
(`src/generated_affine_residual_group_solve_plan.rs:740-742`). The owner must
add a sealed target-authentication seam that resolves a locator against the
same plan/inventory allocation and exposes the target affine source and target
guards. A forgeable external `&[bool]` is not an acceptable target-state input.

The public-rule collection is never the algebraic database. Later raw rows are
top-reduced only by unconditional guarded algebraic unit pivots retained in the
database, not by conditionally published coverage leaves.

## 3. Staging API and token invariants

The database mutation should be split conceptually as follows; exact Rust names
may follow module naming conventions:

```rust,ignore
fn stage_replayed_row(
    &self,
    source: Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
    /* authenticated family/context/plan/frame/epoch binding */
) -> Result<StagedExactRow, ExactDatabaseError>;

fn prepare_next_row(
    &self,
    /* authenticated pending row */
) -> Result<PreparedRowTransaction, ExactGroupSolveError>;

fn commit_prepared(
    &mut self,
    prepared: PreparedRowTransaction,
) -> Result<CommittedRowEvent, StaleOrForeignTransaction>;
```

`stage_replayed_row` performs exact replay and the existing hardest-only
reduction without mutating the database. It returns one of:

```text
StagedDependent {
    source_recipe, source_ordinal, reduction_trace, prospective_stats, ...
}

StagedNewPivot {
    source_recipe, source_ordinal, pivot_ordinal,
    exact_unit_terms, guards, reduction_trace, normalization_divisor,
    lookup_insertion, prospective_stats, ...
}
```

Each variant owns every replacement relevant to its database outcome; the new
pivot variant owns complete pivot and lookup replacements. A staged new pivot
lends the same sealed read-only term/guard/divisor interface as a committed
pivot, so downstream code never reconstructs it from a raw relation. Database
commit is crate-private and reachable only through the outer owner.

Every prepared transaction is a non-`Clone`, consume-once capability with
private constructors and fields. It binds all of the following:

- exact outer-owner allocation identity;
- exact plan and frame allocation identities, group, and database epoch;
- the owner's `state_version`, source cursor, and current pivot count;
- the retained raw-row recipe and exact reduction trace;
- the staged database replacement;
- for a matched pivot, the first eligible unresolved target locator and its
  authenticated target source/guards;
- the exact recentering translation and centered relation;
- the terminal `WhenBad` classification and leaf manifest;
- fully preallocated event, target-state, published-rule, residual-work, and
  statistics replacements.

`commit_prepared` first checks all identities and version/cursor/count facts
without mutation. A stale, double-used, reordered, or foreign token is rejected
with the owner byte-for-byte unchanged. After validation, commit consists only
of moving/swapping already constructed values into the owner; it performs no
allocation, coefficient arithmetic, Symbolica call, user callback, clone, or
fallible conversion. Consuming the token makes successful double commit
unrepresentable in safe Rust.

The current database already preallocates both pivot and lookup replacements
before its local move-only section
(`src/generated_affine_residual_group_exact_database.rs:810-850`). Preserve
that technique, but delay the move until all outer-owner replacements have also
been constructed. Current guard copying still contains infallible deep clones
after reserving only the outer vector
(`src/generated_affine_residual_group_exact_database.rs:1216-1258`,
`src/generated_affine_residual_group_exact_database.rs:1261-1295`); this must be
removed from the final commit path and remains a separate fallibility-hardening
seam.

## 4. Why raw `exact_relation` cannot follow database ingress

The existing exact-relation compiler is explicitly a raw certificate-row
adapter, not authoritative database ingress
(`src/generated_affine_residual_group_exact_relation.rs:1-20`). Calling it
after the current mutating database method does not repair the ordering:

```text
raw row maximum K              K is already a database pivot
        |                      top-reduction substitutes K
        v
raw exact_relation centers K   actual staged unknown leader is B < K
```

If recentering rebuilds the raw row, it computes the target offset and
coefficient substitution from `K`; LiteRed computes them from post-reduction
leader `B`. The selected target, translation, guards, and published rule can all
therefore be wrong. The separate caller-owned unresolved bitmap can also race
or disagree with the owner's target state.

Extract the exact geometry/translation kernels into an
`ExactPivotRecenteringCompiler` whose only production input is a sealed
`StagedNewPivot` view plus the owner's authenticated target-state view. It must:

1. take the already normalized post-reduction physical pivot `r`;
2. compute `t = r - A r_F` in arbitrary-precision `Integer` arithmetic;
3. choose the first eligible unresolved solve-plan target;
4. translate coefficient parameters as `n_F -> n_F - r_F`;
5. center every RHS shift as `s -> s - r`;
6. retain row algebraic guards separately and source target-domain guards from
   the authenticated selected target;
7. return `NoTarget` or a sealed exact centered candidate without mutation.

No final path may downcast the pivot, target offset, centered shifts, or
boundary values to `i64`.

## 5. Terminal outcomes and atomic commits

A **terminal row outcome** is an authenticated mathematical/coverage decision
for the staged row. It commits exactly once:

| Outcome | Algebra database | Cursor/event | Target | Published/residual |
|---|---|---|---|---|
| `Dependent` | unchanged | advance; append dependent trace | unchanged | unchanged |
| `NoTarget` | commit pivot | advance; append unmatched event | unchanged | unchanged |
| `Certified` with applicable coverage | commit pivot | advance; append accepted event | consume exactly one | append good rule leaves and exceptional residual leaves |
| `IdenticallyBad` | commit pivot | advance; append rejection event | remains unresolved | no rule; retain residual/rejection provenance as specified |
| `Unsupported` | commit pivot | advance; append unsupported event | remains unresolved | no rule; retain unresolved/requeue provenance |

`IdenticallyBad` and `Unsupported` reject publication, not algebra. The pivot
must remain available to reduce later rows, while a later distinct pivot may
still solve the same target. A committed row event makes the rejected pivot a
one-shot candidate without LiteRed's mutable `except` workaround.

An **operational outcome** proves nothing about the integral domain and commits
nothing. This category includes authentication/binding failure, wrong source
order, stale/foreign token, malformed replay, arithmetic or allocation failure,
resource exhaustion, an unexpected representation/conversion error not
deliberately certified as the terminal coverage result `Unsupported`, and a
caught Symbolica panic during preparation. The row remains at the same cursor
for retry or the owner enters an explicit fail-stop interrupted state. It never
becomes a master claim.

Atomicity means that before the first owner mutation, every algebraic,
recentring, `WhenBad`, authentication, capacity, and resource check has
completed and every replacement value exists. There is one owner-wide commit,
not a successful database commit followed by a fallible target/publication
commit. Panics are caught around preparation; the final move-only commit is
designed not to invoke code that can panic or unwind through partially changed
state.

## 6. Event and replay contract

The private append-only event ledger and chronological fresh-shadow replay are
implemented for the current `Dependent`, `NoTarget`, and
`RequiresAffineEqualityRefinement` transitions. Each current event binds:

- the physical-row recipe `Arc`, source ordinal, group, epoch, and owner version;
- actual hardest-key lookup sequence and exact factors;
- dependent status or pivot key, divisor, guards, and normalized terms;
- target locator or `NoTarget`, exact offset and translations;
- the current disposition and target transition.

The following fields extend that same event contract when `WhenBad` and
publication land:

- `WhenBad` disposition and exact applicable/exceptional leaf manifests;
- published/residual handle ordinals.

Current replay regenerates each raw row in submitted order from its opaque
recipe, repeats the actual hardest-only lookup sequence, reconstructs the
staged leader and divisor, repeats recentering and target selection against a
fresh shadow target state, and compares every implemented event plus the final
owner state and statistics. Future replay additionally recompiles `WhenBad`
and compares all rule and residual leaf manifests. Final state alone is never
replay evidence. Value-equal but independently allocated
plan/frame/inventory/source objects are rejected at live authority boundaries.

## 7. Implementation sequence

1. **Stage the exact database — implemented.** Refactor current ingress into
   `stage_replayed_row` plus a consume-once sealed commit token. The uncommitted
   token retains its opaque physical-row recipe and shared exact
   dependent/new-pivot payload. The owning prepared-commit/abort seam and
   infallible final commit tail are implemented. Keep the existing exact
   hardest-only algorithm unchanged while migrating its coefficient algebra to
   Symbolica public APIs.
2. **Bind exact targets and the session — implemented.** Persist the exact
   solve-plan target order, type equality-refinement cases, bind target state
   to the database allocation/transition, and advance unconsumed successors
   atomically with the database.
3. **Recenter the staged pivot — implemented.** GMP geometry/translation has
   been extracted from
   `generated_affine_residual_group_exact_relation.rs` without carrying its
   raw-row authority. The sealed session wrapper joins that inert kernel to the
   authenticated staged pivot and owner target-state view, with
   transaction-preserving failure and persisted first-match
   NoTarget/equality/Ready classification. Raw-relation compilation is not a
   production authority path.
4. **Prove exact Ready descent and orthant geometry — implemented.** The sealed
   Ready token is reauthenticated, every RHS is compared through the existing
   physical-key ordering, and inactive-coordinate crossing ranges are retained
   as arbitrary-precision Symbolica integers. Success returns
   `ReadyForConditions`; it neither mutates the session nor consumes a target.
5. **Replace eager case entry — next high-loop prerequisite.** Build the
   target-frontier lazy MTBDD/sector-DAG path so `K=21` families can enter this
   exact session without first materializing every orthant case. The existing
   lower-arity Ready gate remains the oracle while that entry path is replaced.
6. **Adapt exact `WhenBad` — pending after scalable case entry.** Reuse the authority-neutral
   polynomial/partition algorithms behind new exact authority certificates,
   replace `IndexShift`/`i64` boundaries with arbitrary-precision values, and
   source guards from the authenticated target. Do not reapply the recentering
   coefficient translation to the already-centered row or to target premises.
7. **Extend the outer owner and publish atomically.** NoTarget now commits
   through a consuming typed owner and continues only from its successful
   result. Equality commits into a sealed refined-epoch suspension while
   leaving its target unresolved. Dependent, NoTarget, and equality commits
   append private events under cumulative limits and support chronological
   fresh-shadow replay. Add `WhenBad` events, rules, residuals, and publication
   to this same database/target session boundary; do not claim complete
   publication yet. Issue authority-bound sealed
   rule and residual handles, then extend the implemented chronological replay
   to `WhenBad`, rule, and residual leaf manifests.
8. **Integrate the generic scheduler.** Only after the transaction and replay
   tests pass should one-, two-, and higher-loop families exercise this path.
   Concrete topologies are validation fixtures, never implementation branches.

## 8. Required adversarial tests

The first implementation slices are incomplete until they cover:

1. `A < B` with known `A` and row `B + c A`: stage `B` immediately and record
   no lookup of easier `A`; dropping the stage changes nothing.
2. A known-hardest chain ending in dependence and a chain whose raw maximum is
   known but whose post-reduction leader is a different unknown `B`; only `B`
   may drive target matching and recentering.
3. `NoTarget`, `IdenticallyBad`, and `Unsupported`: pivot/cursor commit, target
   remains unresolved, no public rule, pivot is not emitted twice, and a later
   pivot can target the same case.
4. Certified full and mixed coverage: consume exactly one target; mixed
   coverage retains every exceptional residual leaf. A later candidate skips
   the consumed target, while rejection does not.
5. Huge positive and negative GMP offsets and matrices, including cancellation
   in `r - A r_F`; verify `n_F-r_F`, `s-r`, unit leader, translated guards, and
   absence of an `i64` conversion.
6. Target guards come from the selected target definition, not from source-row
   premises. Exercise denominator-zero, inactive leaks, numerator-zero gates,
   allowed active pinches, and noninteger-power fail-closed behavior.
7. Stale, double, foreign-owner, value-equal foreign plan/frame, wrong epoch,
   wrong group, wrong source ordinal, and reordered-batch tokens all leave the
   owner unchanged.
8. Inject allocation/resource/arithmetic/Symbolica failure after each prepare
   phase and immediately before final commit; pivots, lookup, cursor, targets,
   events, rules, residuals, statistics, and capacities remain unchanged.
9. Replay rejects changed row order, lookup sequence, reduction factor,
   divisor, target locator, translation, classification, leaf, event, or parent
   allocation.
10. Exercise exact and one-below limits for row events, target state, rules,
    residual leaves, and cumulative child work. No failure or exhaustion path
    infers a master integral.

At least one test must use production ingress through the exact physical-row
compiler rather than private synthetic database terms. One-loop and later
families then validate the generic implementation and may use Vakint as an
output oracle; they do not authorize topology-specific derivation code.
