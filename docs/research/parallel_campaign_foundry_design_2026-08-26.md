# Deterministic parallel parametric-IBP campaign foundry

Status: governing design for parallel derivation from one or more
user-provided starting topologies. This is an implementation plan, not a claim
that the campaign scheduler or a coverage-closed rule bundle exists today.

Implementation checkpoint (2026-08-26): the topology-neutral static
`CampaignPlan` core now interns exact-representation families/jobs, retains
multiple ingress roots, accepts only witnessed strict proper-subsector edges,
and projects deterministic dependency-ready antichains without enumerating all
sectors. A separate stateless resource selector computes stable first-fit
candidate waves under checked core and estimated-memory snapshots; its
100-job/100-core/1-TiB synthetic test deliberately admits only 57 one-core
jobs when RAM is the bottleneck. This selector does **not** reserve resources.
The implemented resource surface now includes versioned estimates,
capacity/baseline snapshots, deterministic wave keys, checked aggregate
core/byte counts, and a distinct move-only admission authority. That authority
replays a frozen plan and atomically charges an entire wave; task and resident
owners release or transfer their exact estimates, including old/new overlap,
under panic-safe guards. A stable indexed low-level executor now runs admitted
move-only work and commits its results after a canonical barrier; a genuine
exact session has passed through its resident-transform path. The roots-only
campaign CLI authenticates only the user's declarations. A separate sealed
publication-handoff wave now transfers committed exact-session event owners
without copying their algebraic payload, keeps one atomic byte per leaf, and
bounds live borrowed tickets independently of worker count. A subsequent
algebra-free `ExactPublicationEpochOwner` consumes only a quiescent, fully
acknowledged handoff. It retains one event handle per slot, replaces obsolete
handoff state with compact applicable/exceptional flat-leaf indices, and keeps
one atomic byte per exceptional source. Bounded exceptional leases are
retry-only: normal drop and unwind return a source to pending, while a
quiescent barrier can recover a deliberately forgotten lease. Its transferred-
payload, retained-shallow, compilation-peak, and live-lease byte limits are
component gates rather than an RSS budget. It does not admit applicable-
provider work/results, implement the stable `CampaignWorkKey` result table or
atomic result-charge transfer, construct fresh narrowed-domain mathematical
epochs, or implement rejected-candidate continuation. These layers still do
not observe RSS, estimate a physical family, construct a calibrated memory-
derived effective-width pool, hydrate a full frontier, normalize a target
numerator, discover dependencies, checkpoint, apply rules, prove closure, or
reduce a physical topology. Those production-coordinator and mathematical
stages remain unimplemented.

## 1. Objective

RustRed must accept a set of starting integral families/topologies, derive its
own generic parametric IBP and LI relations, solve every requested reachable
sector and exceptional locus, and publish one deterministic multi-start rule
bundle. FORM-authored recurrences and topology-specific dispatch are never
inputs to this process. LiteRed2 supplies the mathematical model; Symbolica is
the algebra authority.

There are two different operations which must not be conflated:

1. **source generation** constructs the finite generic IBP/LI relation span
   for a canonical family; and
2. **parametric solving and closure** derives guarded replacement rules over
   integer powers, recursively resolves exceptional loci and proper
   subsectors, and proves that the requested domain is closed.

At six vacuum loops source generation supplies only the 36 ordinary
`d/dk_i . k_j` relations for one family. The expensive work is sector solving,
exceptional closure, sparse elimination, coefficient reconstruction, and
exact verification. Parallelism must therefore surround, rather than corrupt,
the ordered reducer state machine.

## 2. LiteRed2-derived decomposition

The checked-in LiteRed2 source gives a particularly useful safe boundary:

- ordinary IBP/LI sources are constructed for a family;
- `SolvejSector` maps over unique sectors and gives each sector a fresh
  equation database;
- intrinsic sector rules may retain proper-subsector integrals on their right
  hand sides; and
- proper-subsector substitution is performed later by `IBPSelect`/`IBPReduce`
  in dependency order.

Consequently RustRed uses two phases:

```text
parallel intrinsic sector derivation
    (proper subsectors may remain on RHS)
                         |
                         v
parallel bottom-up closure over the dependency DAG
    (shared children solved once, then substituted/linked)
```

Optional solved-subsector feedback is a RustRed optimization, not a reason to
serialize all sectors. It runs at explicit closure epochs over an immutable,
allowlisted snapshot of already closed strict descendants. A scheduling level
or active-line count is never itself a descent proof. Every imported rule must
retain the raw strict-proper-subsector witness and any verified symmetry or
cross-family transport path from the current job to that child. Current,
same-sector, same-rank, and supersector material is rejected; same-rank aliases
have already been collapsed. Jobs at one ready antichain never observe one
another's partial output and are merged in canonical order.

## 3. Canonical campaign model

### 3.1 Inputs and identities

A campaign contains:

```rust,ignore
struct CampaignInput {
    roots: Vec<RootSpec>,
    convention: ConventionSpec,
    ordering: IntegralOrderingPolicy,
    specialization: CoefficientSpecialization,
    terminal_policy: TerminalPolicy,
    resources: CampaignResourcePolicy,
}
```

Input preparation is deliberately two-pass through Symbolica's public token/
parse tree and RustRed's existing authenticated parser/preflight boundary; it
does not introduce another expression lexer or parser. RustRed first scans
every root in input-ordinal order, validates the lexical envelope, collects all
user symbols, and registers their sorted union once. It then parses (serially
if parsing can allocate new symbols) and lowers roots independently, including
denominator/ISP completion and exact rank checks. Candidate routing
equivalences are proposed
from cheap graph and denominator signatures, but every accepted family map is
verified with exact Symbolica algebra. A deterministic sorted orbit merge then
chooses the least canonical structural representative. The original roots are
retained as separate ingress entries with their verified maps.

The current label-sensitive family fingerprint remains useful as a
representation/session identity. It is not the cross-root canonical family
identity.

The first scheduler version may deduplicate only byte-identical normalized
families. Verified routing aliases and compatible-domain unions are later
extensions; this keeps the first implementation small without constraining
the final data model.

### 3.2 Job key and dependency rank

The unit of derivation is a canonical family/sector/domain job:

```rust,ignore
struct JobBaseKey {
    convention: ConventionId,
    canonical_family: CanonicalFamilyId,
    sector: CanonicalSectorId,
    ordering: OrderingId,
    specialization: SpecializationId,
    terminal_policy: TerminalPolicyId,
}

struct CampaignJobKey {
    base: JobBaseKey,
    domain: CoverageDomainId,
}
```

Every dependency edge owns a generic strict-descent witness:

- proper-subsector inclusion;
- certified factorization into a lower component multiset; or
- an exactly verified rank-decreasing cross-family transport.

Same-rank symmetries and routing maps are aliases, not dependency edges. A
cycle or an edge without a replayable strict-descent witness rejects the plan.
No topology name or loop-count switch selects a solver.

### 3.3 Target-driven DAG discovery

RustRed must not enumerate all `2^K` sectors at high loop count. Planning starts
at each requested physical sector and lazily expands only reachable:

- pinches appearing on selected right-hand sides;
- factorized children;
- strict dependencies proposed while resolving exceptional conditions; and
- exactly verified cross-family transports.

An exceptional/refined locus in the same family and sector is local
`CaseLane` work, not a campaign-job edge. It carries a finite-partition or
refinement-progress witness inside its parent workspace. Only when that case
requires a proper subsector, factorized component, or verified rank-decreasing
transport does it propose a new strict DAG edge under the contract above.

Zero-sector, factorization, and symmetry candidates on one frontier may be
analyzed in parallel. Their proofs are sorted and merged before the next
frontier is exposed. Two parents referencing the same canonical child receive
two edges to one job.

### 3.4 Shared family source catalog

The generic generator runs once for each canonical family:

```rust,ignore
struct FamilySourceCatalog {
    key: FamilyGenerationKey,
    family: Arc<IntegralFamily>,
    relations: Arc<ParametricIbpRelations>,
}
```

`FamilyGenerationKey` binds the canonical family representation, convention
and base coefficient context, specialization, generator configuration, and
RustRed/Symbolica revision. Two source catalogs share only when this complete
semantic key and their canonical mathematical payload agree.

Individual ordinary IBP expressions are independent to construct and may be
prepared concurrently, but the immutable ordinary catalog is assembled in
fixed `ParametricRowId` order. For non-vacuum families, LiteRed-style LI
construction consumes the assembled ordinary IBP array; it is therefore a
second barrier. LI rows within that phase may be prepared independently by
their fixed external-pair ordinals and are assembled in lexicographic order.
For a six-loop vacuum family the 36 ordinary IBPs are small enough that cross-
family and cross-sector parallelism is more important than this inner loop.
The current raw one-family executor already covers this source-generation
subset with fixed-ordinal collection, an ordinary-before-LI barrier, and tested
`N=1`/`N=2`/`N=4` relation equality. It is not the campaign executor described
below.

## 4. Execution architecture

### 4.1 Coordinator and owned workspaces

The campaign has one deterministic coordinator and a bounded compute pool.
Workers receive immutable inputs and return one immutable `TaskDelta`, which
may include dependency proposals. They never mutate the global plan directly.

```rust,ignore
struct CampaignPlan {
    roots: BTreeMap<RootId, VerifiedIngress>,
    jobs: BTreeMap<CampaignJobKey, PlannedJob>,
    ready_work: BTreeSet<CampaignWorkKey>,
    revision: u64,
}

struct CampaignWorkKey {
    job: CampaignJobKey,
    phase: WorkPhase,
    epoch: u64,
    lane_or_block: WorkUnitKey,
}

struct JobWorkspace {
    key: CampaignJobKey,
    revision: u64,
    family_sources: Arc<FamilySourceCatalog>,
    cases: BTreeMap<CaseLaneKey, CaseLane>,
    pending: BTreeSet<WorkKey>,
    dependencies: BTreeMap<CampaignJobKey, DependencyState>,
    candidate_rules: BTreeMap<RuleKey, CandidateRule>,
    coverage: CoverageWorkspace,
}
```

`CampaignWorkKey` is the future stable identity for one logical unit of
campaign work. It must contain every semantic discriminator needed to order a
retry or staged result (including a phase-local ordinal when
`lane_or_block` is not already total), while excluding worker ID, completion
time, hydration state, and resource estimates. The present low-level wave
executor and static `CampaignPlan` do not yet implement this production key or
the coordinator that owns it.

The coordinator accepts a delta only for the expected job revision. Equal
keys with unequal payloads are typed conflicts. Pure content-addressable
results such as a fixed modular sample may be cached even if the producing
workspace revision has moved; mutable state deltas may not.

The current raw `derive` command already accepts `--n-cores`, owns one private
pool for one-family source generation and relation rendering, and has no
campaign workspaces or memory-admission layer. The public execution-width
contract for the planned campaign remains `--n-cores N`, where `N` is positive
and is the total core budget for the complete RustRed invocation. It is not
applied once per root, family, sector, job, case lane, or recursive child. `N=1` runs
the same task graph entirely on the coordinator and is the serial oracle. For
`N>1`, the invocation constructs exactly one RustRed-owned local Rayon pool;
the RustRed-owned scheduler neither reads nor configures Rayon's process-global
pool. Vendored restricted/unlicensed Symbolica currently initializes a
one-thread global fallback itself; licensed multicore campaigns do not use
that fallback. The
coordinator may perform compute only while holding one of the same `N` leases;
it never adds an uncounted `(N+1)`th compute thread.
Any public Symbolica operation admitted to use internal parallelism must receive
a lease from this same total budget, reducing simultaneous outer work rather
than creating nested oversubscription. This is currently enforceable directly
only for APIs such as `AtomCore::map_terms_with_pool`, which accept a borrowed
Rayon pool. `TermStreamer::new` always constructs another pool, while sparse
parallel solve/back-substitution accept no pool argument and use ambient Rayon.
Those operations are therefore excluded from an ordinary outer worker until a
controlled exclusive-lease adapter exists; the one-pool statement is a RustRed
scheduler requirement, not a property of every current Symbolica API.

`N` is scheduling policy only. It may be retained in run metadata and telemetry,
but it is excluded from canonical family/source/job identities, rule and shard
hashes, checkpoint mathematical state, and final bundle semantic hashes.
Changing `N` on resume therefore does not create a new mathematical campaign or
invalidate a compatible checkpoint. Per-stage algebra limits and the campaign
memory ceiling remain separate explicit policies.

`ready_work` distinguishes intrinsic-source/solve readiness from dependency-
ready closure, exceptional case lanes, modular samples, and verification
blocks. A deterministic `ready_job_antichain()` view projects this finer set to
the Phase-1 job scheduler; it is not the only production readiness state.
Equal-payload merge compares canonical mathematical payload only. Telemetry,
timing, worker IDs, cache state, and optional derivation traces are excluded.

This requires no cryptographic authentication ceremony. Canonical value keys,
one workspace revision, move-only ownership of a live case lane, and validation
at file/global mutation boundaries are sufficient. Backward compatibility is
not a design constraint during the current development phase.

### 4.2 Safe parallel work units

| Work | Parallel boundary | Merge rule |
|---|---|---|
| Parse/lower roots | roots after one deterministic symbol-census/registration pass | sorted canonical roots |
| Propose and verify family maps | candidate map | sorted exact proofs |
| Generate generic IBP/LI sources | canonical family or source row | fixed row ID order |
| Derive intrinsic rules | canonical sector; frozen-epoch affine-group proposals | canonical reclassification and coverage merge |
| Prepare candidate rows | bounded look-ahead blocks | submit in fixed source order |
| Exceptional `WhenBad` closure | normalized case-lane proposals from one frozen epoch | fixed case priority, coverage recomputed |
| Close subsectors/factorizations | ready DAG antichain | bottom-up canonical job order |
| Modular evaluation | fixed prime/point ordinal | consume prescribed ordinal prefix |
| Exact residual verification | immutable rule blocks | sorted rule key |

Unsafe parallelism is explicitly excluded:

- no concurrent mutation of one `SparseRowReducer`;
- no pivot chosen from the first candidate or modular sample to finish;
- no shared mutable column catalog;
- no back substitution on a live forward reducer;
- no same-rank solved-subsector feedback between peers; and
- no split numeric start shells whose results are independently pivoted and
  later concatenated.

Each independently schedulable `(CampaignJobKey, CaseLaneKey)` owns exactly
one retained Symbolica reducer and submits rows to it serially. A worker is not
a reducer owner: the same lane may move between admitted waves, but two workers
may never mutate its reducer concurrently. Candidate construction may use a
bounded reorder buffer, but the reducer consumes stable source keys in order.
LiteRed carries `rulesFound` and `badconditions` across successive groups, so
groups are not assumed mutually independent merely because their equation
databases are distinct. Groups may run concurrently only as staged proposals
from one frozen residual/target epoch. The coordinator reclassifies them
against current coverage in canonical group order, commits the admissible
prefix transactionally, and schedules a new epoch for any residual work.
Independently owned sectors and families run concurrently; independently
proved case lanes within one frozen epoch may do so under this ordered
proposal contract.

### 4.3 Symbolica ownership

License initialization plus the initial root/user/fixed-symbol census and
registration occur before the worker pool is started. If multicore execution
was requested and Symbolica is not licensed, planning fails before any worker
is spawned: an unlicensed Symbolica instance is thread-pinned. Lazy target-
driven discovery can later introduce canonical child families and their scope-
specific index symbols. At each sorted dependency-frontier barrier, all workers
are quiescent while the coordinator constructs newly required family/
coefficient contexts and registers their symbols in canonical job-key order;
only then is the next ready antichain dispatched. No worker registers a symbol.
This preserves deterministic Symbolica IDs without eager `2^K` family/sector
enumeration, even though Symbolica protects global symbol state with locks.
Immutable families, source catalogs, and admitted coefficient contexts may be
shared through `Arc` where their public `Send + Sync` contract permits it.

Every concurrently active reducer/case lane must construct its own
`CheckedParametricField` controller and retained `SparseRowReducer`. Sibling
clones of one controller deliberately serialize stages, so sharing one
controller between workers would provide no useful parallelism and would couple
their scheduling/accounting through one stage gate. A new lane branched from a
frozen parent therefore calls `try_new` with the shared immutable context and
replays the parent's canonical committed rows into a fresh controller/reducer;
it does not use a native sibling clone as an independently parallel root. Once
inside one lane, only an independent clone-on-stage trial becomes its next
owner; dependent, rejected, and failed trials are discarded.

Fresh-controller lane materialization is itself deterministically cost-
admitted from the canonical parent-row census, RustRed's post-stage Symbolica
`U`/`L` stored-entry census, and
explicit campaign policy. Replay work and temporary memory are charged. If a
lane is not admitted for parallel materialization, its proposals execute in
canonical serial order on a shared-controller fork; RustRed does not silently
incur `O(cases * pivots)` replay. This choice is frozen in the plan revision and
may change timing only.

RustRed does not implement a parallel matrix algorithm. Sparse elimination,
polynomial/rational arithmetic, exact matrix operations, and reconstruction
algebra continue to use Symbolica public APIs. Outer campaign scheduling is
RustRed orchestration, not a CAS implementation.

The public Symbolica audit fixes the available inner seams:

- `AtomCore::map_terms`/`map_terms_with_pool` may preprocess independent terms,
  but any completion-order stream is canonically re-sorted afterward;
- `SparseMatrix::solve_parallel` parallelizes back substitution, not forward
  elimination;
- `SparseRowReducer::back_substitute_parallel` clears `L`, changes mode, and
  may reorder rows, so it never touches the live retained reducer; and
- no public parallel and resource-censused execution seam exists for
  `add_row`, rational-polynomial arithmetic, or polynomial GCD. Public
  polynomial GCD operations do exist and remain the algebra authority.

Rayon is now a direct RustRed dependency, and the implemented raw source
executor owns its private bounded pool through `ParallelExecution`. The planned
campaign executor must preserve this ownership rule with one invocation-wide
bounded pool; it must not rely on Symbolica's transitive Rayon dependency or
Rayon's global pool. This is a RustRed scheduler guarantee; the restricted
Symbolica fallback described in section 4.1 remains an upstream embedding
limitation. Compile-time probes now cover `CheckedParametricField`,
`SparseRowReducer<CheckedParametricField>`, the retained persistent reducer
(`Send + Sync`), and the complete owning exact session (`Send`, deliberately
not a claim that concurrent mutation is allowed). The production coordinator
must retain named `Send + Sync` probes for every immutable authority bundle it
shares across workers as those bundles become concrete.

### 4.4 Ready antichain and load balancing

The coordinator stores phase-aware ready work in a deterministic `BTreeSet`
and derives the job-level ready antichain view. Dispatch may use observational
priorities without affecting semantic order:

```text
dependency criticality
-> memory class
-> predicted work
-> CampaignJobKey / WorkKey tie-break
```

Work stealing is allowed only among already-ready immutable tasks. A result is
merged by its stable key, never arrival order. The worker count, task delays,
or host scheduling must not change rules, guards, terminals, or typed
per-job failures.

The first production executor freezes each sorted ready antichain as a logical
key frontier, then processes it through bounded deterministic waves admitted by
both the core and memory policy in section 4.5. An admitted wave may use an
indexed parallel map/collect, but the executor never constructs task owners or
reducer forks for the complete frontier eagerly. Results are durably staged and
the coordinator performs one stable job-key merge only after the logical
frontier is settled. If several workers fail, the campaign reports the lowest
stable failing key rather than whichever failure arrived first. Schedule-
dependent private database nonces remain in-memory freshness checks and are
excluded from rule, checkpoint, and publication identity.

Wave packing is specified, not left to completion timing. The coordinator
scans the completely ordered logical frontier once and applies stable first-fit
admission to each task's complete resource vector. A task is added when all of
its core, retained-resident, and transient-peak requirements fit the remaining
wave envelope; otherwise it remains a compact plan record for the next wave.
Admitted keys keep frontier order. Because a finite logical frontier must
settle before newly proposed work is exposed, removing every completed wave
prevents a skipped large task from starving behind an unbounded stream of new
small tasks. If no remaining task fits an empty wave, the lowest key receives
the explicit oversized-task pause outcome instead of spinning.

Every parallel phase follows the same barrier protocol:

1. the coordinator freezes a sorted ready-antichain snapshot of compact plan
   records, not heavyweight task owners;
2. it selects the next deterministic bounded wave and acquires both core and
   memory permits before constructing immutable, index-keyed worker inputs;
3. completions are buffered under their memory permits or durably staged
   without exposing newly proposed work; permits are released only after every
   charged temporary/result owner is dropped or replaced by a small staged
   descriptor;
4. waves repeat until the logical snapshot quiesces, then the coordinator
   merges results in stable key order and reports the lowest stable failing key
   if necessary; and
5. only the completed merge exposes the next sorted frontier.

Source generation uses the same rule at its smaller barriers: ordinary rows
are assembled by `ParametricRowId` before LI construction begins, and LI rows
are assembled by fixed external-pair ordinal. No cache hit, worker completion,
or dependency proposal can cross a barrier early.

Avoid nested oversubscription. A task receives an explicit core lease. If a
public Symbolica parallel operation is used, its thread count comes from that
lease; the ordered sparse forward reducer remains single-threaded.

### 4.5 Memory admission and backpressure

Per-stage and per-job resource limits have deterministic derivation-contract
failure semantics; they never have master-discovery or other mathematical
semantics. Global campaign memory admission has scheduling semantics. They
must remain separate:

- exact one-below local limits return a typed resumable failure;
- a global estimated-memory admission semaphore delays a task but does not turn it into a
  mathematical `ResourceLimit`;
- no first-worker-to-spend-it global algebra budget is allowed; and
- concurrent speculative clones of one large reducer are avoided because
  each trial temporarily coexists with the complete base state.

Core and memory admission are conjunctive. A logically ready antichain is not
an instruction to fork every member. Before constructing a task owner, cloning
a retained reducer, or allocating its candidate/result buffers, the scheduler
must atomically acquire both its core lease and its conservative memory
permits. It dispatches only a bounded deterministic wave selected from the
sorted ready antichain; unadmitted work remains as small plan records. Memory
pressure may therefore leave worker threads intentionally idle even when
`--n-cores` is large. Permits are released only after the worker's charged
temporary and result owners have been dropped or a durable result has been
staged and only its small descriptor remains live.

One coordinator owns admission. A worker never holds cores while waiting for
memory, holds memory while waiting for cores, or recursively obtains an
unaccounted Symbolica-inner lease. The coordinator reserves the complete
resource vector atomically before dispatch; move-only RAII reservations return
transient permits on success, typed failure, cancellation, or recovered panic.
The shared-controller serial fallback obeys the same admission path.

The governing estimated-residency invariant is

```text
fixed runtime and safety reserve
+ shared immutable catalogs (charged once)
+ hydrated retained lane reducers
+ sum(in-flight transient peaks)
+ bounded in-memory staged results
<= --max-memory
```

For one admitted clone-on-stage task, the same invariant must remain visible
in the more explicit lifetime decomposition

```text
fixed runtime and shared immutable catalogs
+ old hydrated resident owner(s)
+ newly constructed retained successor output(s)
+ task transient scratch excluding those outputs
+ bounded staged-result/checkpoint buffers
<= --max-memory
```

The old resident and new retained successor are separate terms until the old
owner has actually been dropped. A commit must not subtract the predecessor
reservation merely because it has reclassified the successor as resident.
This decomposition is also the estimator interface: hiding the successor
inside a generic "scratch" allowance would make clone overlap invisible and
would understate the dominant live-set risk.

Accounting follows unique live allocations, not the number of Rust handles or
ledger categories. An immutable catalog or event payload shared by many
owners is charged once. Moving one allocation from an in-flight result to a
resident lane or durable-staging owner transfers the same charge atomically;
it is never charged once at the source and again at the destination. In
contrast, a clone-on-stage predecessor and independently allocated successor
are two unique live states and both remain charged for their entire overlap.
The resource ledger must make that distinction explicit rather than relying
on `Arc` counts or nominal ownership labels.

A successful clone-on-stage commit transfers the successor reducer's retained
reservation into the hydrated-lane term; it does not release that memory as if
the successor disappeared. Replaced base state releases its reservation only
after it is dropped. The number of hydrated inactive lanes is itself bounded.
When the next stable wave cannot fit otherwise, inactive lanes are selected by
a deterministic key/epoch policy, written to a bounded streaming checkpoint,
authenticated, and dehydrated. Rehydration is admitted and reconstructs one
native reducer from canonical rows. Shared `Arc` payloads are charged once to
the campaign baseline rather than once per lane.

The primary high-end target is a roughly 100-core EPYC node with approximately
1 TiB of physical RAM running six-loop single-scale vacuum campaigns.
`--n-cores 100` is only the compute ceiling on such a node. The operator sets
the operational scheduler envelope
`M_operational = --max-memory` so that
`M_operational < M_physical`, reserving explicit headroom for the OS,
checkpoint I/O, allocator fragmentation, Symbolica's opaque native scratch,
and thread-local caches. The inequality is mandatory even on a nominal 1-TiB
node; `--max-memory` is never inferred to equal installed RAM. A task estimate
includes the live reducer, its clone-on-stage successor, prospective
column/catalog growth, candidate and result payloads, and checkpoint/output
buffers. RustRed must prefer fewer live heavy reducers over speculative
fork-all throughput; unused cores are correct when the RAM envelope admits no
additional task.

RustRed uses one process-local Symbolica runtime and one invocation-wide
RustRed worker pool on this target. It must not launch one process per job:
that would duplicate Symbolica process state, immutable catalogs, allocator
arenas, thread-local caches, and license/runtime overhead, defeating the
shared-baseline accounting on which a 1-TiB campaign depends. A future
multi-node mode may shard durable, content-addressed jobs explicitly across
node processes, but each node remains an independently capped campaign worker
with its own declared baseline and never masquerades duplicated memory as a
shared `Arc` charge.

The opaque-native reserve must also cover Symbolica's private thread-local Atom
workspace cache. In the audited 2.2.0 source, automatic `RecycledAtom::drop`
keeps at most 30 Atom buffers whose capacity is at or below 20,000,000 bytes in
each thread-local workspace. The public direct `Workspace::return_atom` route
does not enforce either cap, so this is not an absolute workspace bound.
Neither current cache occupancy nor a trim operation is public, and any
Symbolica-created/ambient Rayon thread is another possible workspace owner.
This is not a prediction that every thread consumes the maximum, but it is a
material 100-core calibration risk. `ParallelExecution(E > 1)` creates `E`
Rayon worker threads while the coordinator remains a separate thread that also
touches Symbolica; `E = 1` instead runs inline with no worker thread. Taken
literally, the automatic-cache source caps alone permit 60,600,000,000 bytes
(about 56.4 GiB) of retained Atom buffer capacity across the coordinator plus
100 fully warmed workers, before allocator overhead or any algebraic live set.
Dense polynomial multiplication can additionally retain a private
thread-local `Vec<u32>` whose logical length is limited to `1 << 24` entries
(64 MiB of initialized elements) on every thread that touches that path;
amortized `Vec` growth means retained capacity can exceed that figure. The
multiprecision-float backend has another private TLS constants cache. Neither
cache has a public capacity census or trim API.
`SparseRowReducer::back_substitute_parallel` accepts no pool/core or memory
budget and allocates one dense `ncols` fold scratch plus local sparse output per
participating Rayon worker. These observations strengthen the calibrated
opaque-native reserve; they do not justify a RustRed replacement for Symbolica
algebra.

The invocation therefore chooses an **effective execution width `E` before
constructing the pool**, with `1 <= E <= --n-cores`. The fixed/shared baseline
separately charges the calibrated stack, TLS, and Symbolica Workspace reserve
for the coordinator, all `E` possible workers when `E > 1`, and every
explicitly admitted inner thread. Per-task admission then additionally charges
the Symbolica reducer/workspace and algebra scratch not already covered by
that warmed-thread reserve, plus a bounded result allocation. RustRed selects
the largest feasible `E`; it does not create 100 workers and assume that
currently idle threads are free.
If the coordinator-only `E = 1` baseline plus the minimum runnable task still
does not fit below `--max-memory` and operational headroom, execution returns a
typed memory-capacity pause before constructing any pool. Requested width,
effective width, worker-thread count, and estimator revision are reported as
physical run metadata and never enter mathematical hashes. APIs that create
uncontrolled inner pools remain excluded from ordinary workers. Campaign
benchmarks measure warm-thread RSS, keep explicit OS/checkpoint/allocator
headroom, and may deliberately leave cores idle. A retained case/reducer lane
is a task owner, not a worker thread. The scheduler never infers unused RAM
from a low U/L stored-entry count alone.

The pre-pool decision is a distinct, versioned
`CampaignExecutionWidthPlan`, not an interpretation of how many one-core jobs
happen to fit in the first wave. It records the requested ceiling, effective
`E`, worker-thread count, operational and enclosing memory limits, the complete
fixed-memory breakdown, and estimator revision. Only consuming that checked
plan may construct `ParallelExecution(E)`. A counting pool-factory acceptance
test must observe zero worker construction for a typed no-fit result, no pool
for an accepted inline `E=1` plan, and exactly `E` workers otherwise. In
particular, the current 100-slot wave-selector arithmetic test is not evidence
that effective-width planning or pre-pool memory admission exists.

The logical ready frontier may contain thousands of compact keys while only a
small admitted subset is hydrated. Its key/estimate metadata is bounded
separately from heavyweight ownership; reducer construction, retained-owner
replay, candidate buffers, and staged results begin only after conjunctive
core-and-memory admission. Wave barriers provide the natural checkpoint and
dehydration boundary: settle or durably stage the complete frozen wave,
release every transient owner, merge in stable key order, checkpoint the
canonical barrier state, and only then expose or hydrate the successor
frontier. The scheduler never tries to improve utilization by carrying an
unbounded tail of inactive hydrated lanes across barriers.

EPYC hosts are normally NUMA systems, so the executor must report socket/node
layout and remote-memory effects rather than treating 100 logical cores as a
uniform resource. Initial policy remains deliberately simple and
deterministic: first-touch a newly hydrated lane on its executing worker,
avoid concurrent mutation or migration of one retained reducer across NUMA
nodes, and prefer keeping a lane and its scratch node-local when observational
affinity data permits. Pinning, socket-aware packing, and lane affinity are
physical execution policy only; they cannot enter job identities, pivot
selection, wave membership, or mathematical hashes. Cross-node immutable
sharing and memory-bandwidth saturation must be measured before NUMA-aware
packing is enabled, and a one-socket/one-node run remains the correctness
oracle for such tuning.

If the fixed baseline plus one task's admitted estimate exceeds the whole
campaign memory ceiling, it
does not wait forever. It returns `PausedForMemoryAdmission` with the estimate,
ceiling, and last checkpoint so the user can change scheduling policy. Memory
permits bound admitted estimates, not hard RSS: Symbolica's private allocator,
thread-local caches, and infallible clone/add-column allocations are not fully
censused. Allocator abort is therefore recovered by restarting from the last
durable checkpoint, not claimed as a typed in-process error. A cgroup or outer
supervisor is required when the operator needs a hard RSS ceiling;
`--max-memory` is RustRed's conservative scheduler envelope.

For one campaign resource-policy revision, every task admission estimate is
frozen deterministically from canonical task data and already committed task-
local telemetry: RustRed-observed retained Symbolica U/L stored entries,
physical column count,
coefficient-work history, residual case count, and catalog size. Canonical
phase-barrier merges may produce deterministic task-local inputs for newly
discovered tasks; arrival order never does. Observed global RSS is reporting
data only and cannot change whether a task runs or pauses in the current
revision. Using it to tune later estimates requires an explicit new resource-
policy revision. Result channels are bounded; very large immutable results may
be written to task-local staged files and returned as small descriptors. A
permit is released after staging only when the descriptor names a durable file
and binds its content hash, size, and task key. Checkpoint and staged-result
serialization is streaming/bounded and its buffers are charged.

Two production pieces remain deliberately unimplemented: the stable
`CampaignWorkKey`-indexed result table and the atomic memory-charge transfer
from an in-flight worker result into either a resident successor owner or a
durable staged-result descriptor. The current low-level executor, exact
publication handoff, and algebra-free publication-epoch owner do not establish
either contract. Until the coordinator
owns this transition, it must not release the worker result permit merely
because a result was sent on a channel, and it must not claim that staged
outputs are covered by the campaign envelope.

The estimator is phase-specific and versioned. Checked `u64`/`u128`
arithmetic combines native U/L stored-entry, row, and column counts with serialized
coefficient/big-integer limb sizes, row/catalog capacities, base-plus-successor
clone overlap, projected catalog growth, reorder/result/checkpoint buffers,
worker stack/TLS allowances, and calibrated Symbolica/allocator safety
multipliers. Overflow is a typed planning failure, never saturation to a small
estimate. Before a six-loop production claim, isolated representative stages
record predicted and observed peak deltas, concurrency overhead, fragmentation,
and the worst ratio; a frozen policy revision selects the safety multiplier.
Observed RSS remains telemetry for that revision and can influence admission
only through an explicit later revision.

Future telemetry records predicted and observed retained/peak bytes per phase,
old/new reducer overlap, U/L stored-entry fill and coefficient-limb growth, staged bytes,
allocator/RSS delta, warm thread-local workspace occupancy effects, NUMA
locality, worker utilization, and time spent core-
versus-memory-limited. Calibration may adapt coefficients and safety margins
only by producing an explicit successor estimator revision at a canonical
barrier or between runs. It must not let completion order, instantaneous RSS,
or one unusually early task silently change admission decisions within a
frozen revision. Until representative physical families calibrate that model,
`--max-memory` must retain conservative allocator/native headroom and remains
an estimated envelope rather than a hard RSS promise.

## 5. Intrinsic solving, closure, and publication

Each job first progresses through intrinsic derivation:

```text
Planned
-> FamilySourcesReady
-> IntrinsicRulesDerived
-> ClosureOpen(epoch 0)
```

Dependency and exceptional closure then form one fixed point, not a linear
pair of states:

```text
ClosureOpen(epoch e)
  -> AwaitingClosedDependencies | ExceptionalWorkReady
  -> merge closed-child and frozen-epoch case proposals in stable order
  -> ClosureOpen(epoch e+1) when new dependencies/cases/residuals appear
-> ClosureFixedPoint
-> AllDependenciesClosed
-> ExactVerified
-> ClosedShard
```

Intrinsic sector workers retain lower-sector integrals on their right-hand
sides, matching the safe LiteRed2 decomposition. Bottom-up closure then
processes Kahn-ready antichains. The scheduler keeps separate intrinsic-ready
and closure-ready queues: intrinsic discovery need not await proper subsectors,
whereas publication work does. Exceptional analysis may propose a new child
dependency, which returns the parent to `AwaitingClosedDependencies`. If
solved-subsector feedback is enabled, each retry is an explicit new epoch using
the immutable allowlisted strict-descendant snapshot described above.

A committed mixed publication and an `IdenticallyBad` candidate take different
continuation paths. Each accepted exceptional publication leaf is future input
to a **fresh generic IBP derivation epoch** over exactly its narrowed domain
(the parent target premises conjoined with that leaf's relative predicates).
That epoch owns a fresh case lane and exact database/reducer and regenerates or
replays the family's generic IBP/LI sources in canonical order; it may share
the immutable family source catalog, but it does not keep mutating the
publication-producing reducer. This is RustRed's structured equivalent of the
LiteRed2 clean/regenerate semantics. Each successor additionally retains a
monotone continuation witness: its exact exceptional domain re-enters the
unresolved set, while a candidate found bad everywhere on that domain is
excluded before later generic rows continue. Restarting the identical candidate
order with no such domain/exclusion state could reproduce the same partition
forever. The current algebra-free epoch owner exposes the immutable narrowed
domain and compact source geometry only through a bounded retry lease. This is
scheduling access to an existing committed event, not mathematical re-entry:
source ingress, continuation state, a fresh database/reducer lane, admitted
results, and the re-entry coordinator are not implemented.

`IdenticallyBad` is deliberately separate. It publishes no good or exceptional
leaf, leaves the selected target unresolved, commits the candidate pivot once,
and continues later source rows in the **same** exact database so that the
retained pivot can reduce them. Its consumed source/candidate cannot be tried
again. It must never be converted into a fresh exceptional epoch or duplicated
in the residual queue.

A job becomes a durable `ClosedShard` only if:

1. every requested-domain leaf routes to a strictly descending rule, a closed
   dependency, or a finite explicitly selected/independently certified
   terminal or product;
2. every exceptional residual is itself closed;
3. the rejected-candidate and solved-subsector queues reached a fixed point;
4. every dependency is closed and carries a replayable descent witness;
5. every intrinsic rule has zero exact residual against freshly regenerated
   parent-family IBP/LI sources, and every rule composed with lower feedback
   also retains and replays the closed child's source/descent witnesses (or an
   equivalent flattened exact source-combination witness) recursively; and
6. no reachable unsupported, uncovered, timed-out, resource-limited, or
   unresolved leaf remains.

Search exhaustion and resource limits never infer a master.

The output is a multi-start bundle, not one flattened recurrence table:

```text
campaign manifest
├── convention / ordering / specialization
├── root ingress table (one verified map per user root)
├── canonical object table
├── strict dependency DAG
└── immutable Closed family/sector rule shards
```

The manifest is installed last and only after every referenced shard is
closed. Incomplete workspaces cannot be opened by the reduction runtime.

## 6. Deterministic modular acceleration

Finite-field discovery and rational reconstruction may be parallelized over a
fixed schedule:

```rust,ignore
struct ModularSampleKey {
    job: CampaignJobKey,
    prime_ordinal: u32,
    point_ordinal: u32,
    purpose: SamplePurpose,
}
```

Primes and points come from a manifest-defined sequence. Reconstruction
consumes the first required admissible ordinal prefix, not the first results to
finish. Bad samples retain deterministic rejection reasons. Held-out checks
use a separate fixed schedule. Every accepted reconstructed rule still passes
an exact Symbolica residual replay.

## 7. Checkpoint and failure contract

Resumable workspaces and closed shards are different types and file formats. A
job checkpoint stores canonical RustRed state:

- job key, input/policy IDs, and revision;
- source cursor and committed pivot rows/catalog;
- committed target dispositions plus session/event state or replayable source
  recipes;
- accepted candidate rules and their exact source/descent witnesses;
- pending and committed exceptional-case state;
- pending case/dependency queues and dependency payload identities;
- coverage state and explicitly selected/certified terminals;
- deterministic modular samples/reconstruction state; and
- algebraic telemetry.

Private Symbolica reducer internals need not become a persistence format. On
resume, RustRed rebuilds one native reducer from canonical committed rows in
source order and verifies reconstructed `U`, pivots, and RustRed evidence. The
historical native `L` allocation need not be byte-identical to the pre-
checkpoint instance; the canonical source-combination witnesses must replay
exactly. This one-time load operation does not reintroduce per-stage
reconstruction into the live path.

Checkpoint generations are written atomically and the current-generation
pointer is installed last. To make failure atomic, the lane actor/coordinator
retains committed state and gives a worker only an immutable snapshot plus a
forked/prepared trial. The worker executes behind the recoverable unwind
boundary; only a returned, validated successor is committed. A failed or
panicked worker therefore commits no delta and leaves the committed lane
unchanged. Process or allocator aborts recover from the last checkpoint; they
are not claimed as in-memory rollback.
Retrying with relaxed local limits creates an explicit resource-policy
revision; it is never silently reclassified.

Canonical barrier-state identity is separate from the physical execution
manifest. Worker count, memory ceiling, estimator revision, wave boundaries,
and hydration history may change physical checkpoint generations without
changing the mathematical campaign. Cross-resource equivalence compares the
canonical barrier-state hash and eventual shards/bundle, not byte-identical
checkpoint files. Each physical generation persists its resolved resource
policy and estimator version, committed-versus-staged descriptors, and hashes
for every durable staged delta.

Representative outcomes are:

```rust,ignore
struct TaskDelta {
    // One atomic result may both advance local state and propose children.
    dependency_proposals: Vec<DependencyProposal>,
    // rules, case progress, coverage, samples, and other phase-specific data
}

enum CampaignTaskOutcome {
    Completed(TaskDelta),
    PausedForDependencies,
    PausedForMemoryAdmission {
        estimated: CampaignBytes,
        campaign_limit: CampaignBytes,
        checkpoint: CheckpointId,
    },
    ResumableResourceLimit(ResourceFailure),
    UnsupportedFrontier(FrontierWitness),
    OperationallyCancelled,
}
```

Invalid input/maps, non-descent/cycles, exact residual failures, native
failures, and same-key/different-payload merges are errors. An unresolved or
resource-limited frontier remains unresolved.

## 8. Implementation phases

1. **CampaignPlan V1:** multiple exact-representation roots, identity ingress,
   shared proper-subsector jobs, strict dependency checks, phase-aware stable
   ready work, deterministic job-antichain projection, and plan equality under
   root permutations/idempotent repeated IDs.
2. **Bounded in-memory executor:** one owner per job/case lane, shared family
   source catalogs, independently controlled retained Symbolica reducers, and
   identical 1/2/4-worker results. Core and memory permits are acquired before
   any heavyweight owner/reducer clone, so a wide ready frontier remains a
   compact queue rather than an eager set of live forks.
3. **Exceptional closure:** extend the implemented algebra-free epoch owner
   with RAM-admitted exceptional results, stable-key result staging and atomic
   charge transfer; then add fresh narrowed-domain source ingress, separate
   same-database rejected-candidate continuation, frozen-epoch affine
   proposals, solved-subsector feedback from allowlisted strict descendants,
   and joint dependency/exception fixed-point admission.
4. **Resumable jobs:** atomic checkpoints and interruption/resume equivalence;
   rebuild and authenticate the native reducer once at load.
5. **Multi-root canonicalization:** verified routing aliases, factorization and
   rank-decreasing cross-family edges, and compatible-domain aggregation.
6. **Modular acceleration:** fixed parallel sample schedule, deterministic
   reconstruction, held-out tests, and mandatory exact replay.
7. **Bundle compiler:** immutable closed shards, shared object/dependency
   tables, root ingress maps, and manifest-last installation.
8. **Scaling gates:** semantic equivalence at 1/2/4 workers, Vakint's
   one-through-four-loop corpus as an external output oracle, then physical
   five-/six-loop derivation-only campaigns.

The retained Symbolica sparse adapter and complete easiest-first catalog are
prerequisites for phase 2. Phase 1 can begin as soon as their live exact-
database integration has a stable private transaction boundary.

The implemented V1 static-plan subset covers identity ingress, shared
proper-subsector jobs, replayable strict-descent witnesses, and the pure
job-antichain projection. Phase-aware work records and execution/progress state
belong to the later workspace/executor and are not claimed by `CampaignPlan`.
The implemented wave selector remains a deterministic calculation over a
policy snapshot. The separate atomic controller replays that calculation and
turns it into move-only core/estimated-memory charges, including retained
successor ownership. It now also owns a stable indexed wave executor and a
resident-transform primitive. The latter has been exercised with a complete
exact session performing a genuine generated-row Symbolica dependent
transition while old, successor, and transient envelopes overlap. These are
still low-level cooperative primitives: the public generic callback cannot
prove arbitrary-`T` identity, forbid unrelated admission guards inside a user
payload, or prevent a callback from creating a nested pool. The production
campaign coordinator will supply crate-owned guard-free phase adapters, bind a
work revision, enforce the effective-execution-width calculation, hydrate jobs
only after admission, aggregate failures by stable key, and settle durable
barriers. A calibrated physical estimator, full frontier dispatcher, and
checkpoint coordinator are not yet implemented.

## 9. Acceptance matrix

The parallel foundry is accepted only after all of the following pass:

- permuting roots yields the identical complete plan; repeating the same
  `RootId` with the same payload is idempotent, while the same `RootId` with a
  different payload is a typed conflict;
- distinct root IDs that are verified routing aliases retain distinct ingress
  rows but share the identical canonical job/object DAG;
- two parents sharing one subsector derive that child once;
- the `N=1` serial oracle and `N=2`/`N=4` runs yield identical canonical task,
  barrier-state, shard, and bundle hashes and identical mathematical output;
  physical checkpoint-generation histories may differ and declare their
  resource manifests;
- an instrumented RustRed-owned executor never observes more than `N`
  simultaneous outer and Symbolica-inner leases, creates no nested pool, and
  neither reads nor writes Rayon's process-global pool;
- randomized worker delay and modular arrival order do not change output;
- candidate row preparation matches sequential ordered reducer submission;
- one frozen exceptional epoch produces the same semantic deltas through
  fresh-controller parallel materialization and shared-controller serial
  fallback;
- feedback workers see the same frozen allowlisted strict-descendant snapshot,
  and every imported rule replays its subsector/transport witness;
- same-key/different-payload, cycles, and non-descending edges reject without
  mutation;
- interruption after each commit boundary followed by resume matches an
  uninterrupted campaign;
- global memory backpressure changes timing only; an individually oversized
  estimate pauses explicitly rather than deadlocking;
- a synthetic 100-core/approximately-1-TiB admission test never exceeds its
  configured estimated-residency ceiling, never eagerly clones the complete
  ready frontier, transfers persistent successor reservations across commits,
  dehydrates inactive lanes, computes effective width before any pool would be
  constructed, hydrates only the admitted stable-key subset, and permits idle
  cores whenever the next deterministic wave would exceed that ceiling;
- a counting pool-factory spy observes no construction for a typed no-fit
  width plan, no worker pool for accepted `E=1`, and exactly `E` workers for
  accepted parallel plans; the planner charges all possible worker
  TLS/Workspace reserves before handing the plan to the factory;
- an optional non-CI soak on a real approximately-100-core EPYC/1-TiB host runs
  with `--n-cores 100`, records `M_physical`, `M_operational`, effective `E`,
  warm-worker Symbolica/TLS reserve, peak RSS, staged-result high-water mark,
  and idle-core time, and produces the same semantic hashes as the serial
  oracle. Hardware unavailability may skip this physical soak, but the
  synthetic width-100 acceptance test remains mandatory and a six-loop
  scalability claim requires named-host evidence;
- admission instrumentation proves that no heavyweight owner exists before
  its atomic core-plus-memory reservation, panic/cancellation returns its
  transient reservation, a huge frontier remains metadata-only, staged output
  buffers stay bounded, and fixed-baseline-plus-one-task overflow pauses rather
  than deadlocking;
- exact-limit and one-below-limit failures are worker-count invariant and
  resumable;
- a worker error or recoverable panic in a forked trial leaves committed lane
  and plan state unchanged; process/allocator abort resumes from checkpoint;
- no unsupported or resource-limited frontier is promoted to a terminal;
- every reloaded final rule passes exact regenerated-source residual and
  coverage verification; and
- concrete numerator/propagator cancellation closure and normalized Vakint
  comparisons pass only after independent RustRed derivation.

The first performance report records named hardware, wall time, peak RSS,
native U/L stored-entry fill, coefficient work, ready-antichain width, worker utilization,
and 1/2/4-worker speedup. A speedup claim is invalid when memory admission
allows fewer simultaneous reducer owners than the requested worker count. The
six-loop report additionally records the 100-core EPYC node's physical RAM,
configured memory ceiling/headroom, estimated-versus-observed peak per task,
maximum simultaneous heavyweight owners, and time spent core- versus
memory-limited.

## 10. CLI direction

The human-facing CLI should eventually expose the same model without requiring
large TOML documents:

```text
rustred campaign plan campaign.toml
rustred campaign plan campaign.toml --resource-report
rustred campaign derive campaign.toml --n-cores 4 --max-memory 120GiB --resume work/
rustred campaign verify bundle/ --exact
rustred campaign inspect bundle/
```

One or several compact Symbolica `Family(...)`/`I(...)` expressions may carry
the topology and target information; TOML supplies campaign-wide resources,
ordering, terminal policy, and output paths. `--n-cores` is the total local
execution budget described above; it and memory admission may change runtime
and telemetry only, never the semantic bundle.

Before implementation, the CLI contract must freeze accepted binary units,
zero/overflow rejection, explicit/default/`auto` behavior, TOML-versus-CLI
precedence, host/cgroup preflight and headroom, pause exit status, staging/work
directory and disk quota, and the resource report. No default may silently
promise that the scheduler envelope is a hard RSS cap.
