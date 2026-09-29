# Epoch S2 integration and S3 checkpoint contract

Status: preparatory integration, corrective source, internal writer and private
publisher passed independent source review and consolidated release typecheck
at `6f7eb9fd`. The first typecheck failure is retained below. Follow-on bounded
restore primitives passed independent source review; their typecheck and execution
remain pending.
No epoch native execution, campaign pilot or deployment claim is made.
The active authority is `CODEX_PROGRESS_PLAN.md` and the September 29 directive
in `GOAL.md`. The earlier epoch protocol's importer, one-hour pilot windows
and socket-1 launch instructions do not authorize those actions now.

## Source and reconciled evidence

The isolated branch `codex/epoch-s3-lc2`, worktree
`.claude/worktrees/codex-epoch-s3`, starts from integration tip `56996edf`
(Rust source `7546c44c`). It merges the existing S2 history at `5d166910`.
The original epoch worktree and its vendor changes are not modified.
The integration retains Symbolica gitlink `ef0db494`, without vendor patches.

The S2 fix-round gates already ran. `TMP/epoch-s2/runs/fixround-r2.log`
records successful completion at 2026-09-28 20:12:32 UTC: FG, BMW, H, X,
four-all and four-all-p5 identities across W6/W12/W24; finite-five W12/W24;
FG B8 W6/W12; the control-oracle and legacy CP5 resume phases. Those are
historical receipts for the frozen S2 binary, not new-baseline validation.
The stronger 1.5x matched-throughput epoch deployment gate remains open.
Lockstep B stays 16; the B32/B64 breadth failure is retained in the S2 note.

The preparatory merge resolves six textual conflicts: public exports in
`application/mod.rs`, `routed_campaign/mod.rs`, `lib.rs`; queue visibility;
the verifier's additional epoch fields; and CLI help. Further source seams:

- The direct Rust epoch entry returns before legacy G2/rescue admission.
  It now explicitly refuses G2 union, G2 activation, rescue amendments and
  resume. These combinations must never silently run with their flags ignored.
- Shared `queue::Stored` now needs quarantine. Epoch supplies the empty
  view only while its entry rejects rescue. The raw export reader supplies
  empty rescue metadata under the same restriction.
- Query-root rows retain the parsed exact-ID role and whether it was
  declared. Undeclared queries stay required irrespective of their names.
  The complete 116 required / 67 auxiliary frozen schema is preserved;
  no geometry, row order or role declaration is rewritten.
- Legacy G2 publication positions and epoch merge numbers remain distinct.
  No implicit conversion of stamps or accepted-prefix pins is attempted.
- Independent review found the offline verifier could silently accept a
  G2-enabled request for an epoch export, because execution admission does
  not run there. It now shares the narrow extension refusal after identifying
  epoch state; it does not apply unrelated execution scheduling/resume rules.
  The existing export fixture tests both union and activation mutations.

LC2's thread-owned context path is in core, not a pool setup hook. Epoch's
`inspect_job` borrows the shared reducer and calls the unchanged visitor;
its result/resolver code carries geometry and serialized records, without
cloning Symbolica polynomials or context seals. The merged core files are
byte-identical to the integration baseline. `indexed/specialization.rs`
uses `zero_with_capacity_thread_owned` and `clone_thread_owned`,
`indexed/translation.rs` and `solver/case/affine/chart.rs` use
`clone_thread_owned`, and `indexed/context/binding.rs` uses thread-owned
seals. Their thread-local caches invoke the public Symbolica context APIs
on the inspector's calling thread. There is no reintroduced per-operation
shared-context Arc clone in epoch; this is a source finding, not an epoch
scaling measurement. Include the existing core thread-owned-context tests
and epoch worker-width identity in the consolidated native gate.

New preparatory regression sources extend `f20_refused_lanes` and add
`epoch_input_roots_keep_exact_roles_and_default_required`. They are not yet
executed. Targeted rustfmt and `git diff --cached --check` pass.
Independent prerequisite source/contract audit passed, conditional on native
validation. The affected Python audit/supervisor modules passed all 54 tests
in 11.243 s on CPUs 32-39; receipt:
`TMP/codex-epoch-s3.JjASCU/python-focused.log`. This is frontend validation,
not execution of the new Rust regression sources.

The isolated preparatory source was committed as `f81559a6`. Baseline fixes
for no-work G2 activation durability and authenticated root phases were
cherry-picked as `25f41db5` (same patch as `d12db6cf`). A guarded release
`cargo check --tests` on CPUs 16-31 failed after 83.179 s, exit 101: the epoch
resolver lacked an `ApplyG2` arm, and the A9 fixture used removed
`helper_pattern`. The first-failure receipt remains unchanged at
`TMP/codex-epoch-s3.JjASCU/typecheck`; minimum available RAM was 649.81 GiB.
The separately audited corrective commit `d5b05629` emits an explicit
Protocol/C5 receipt for unsupported legacy G2 stats (`panic=false`, no
coverage or retry), and declares exact A9 query roles. Independent review
also caught a private `Class::code()` call in the new test before compilation;
the test now uses the existing documented persisted C3 code. Corrective
format/diff checks pass. The consolidated guarded `cargo check --release --tests`
at `6f7eb9fd` passed after 25.166 s, exit 0, reason null, on CPUs 16-31 with
eight jobs. Receipt: `TMP/codex-epoch-s3.JjASCU/typecheck-retry`; minimum available
RAM 831,233,785,856 bytes, maximum single-child RSS 1,162,360 KiB. This includes
writer `2cc3710d` and publisher source `2662a846` (cherry-picked as `6f7eb9fd`).
It executes no native regressions and does not validate later restore sources.

## Internal writer slice (not CP6 publication)

`epoch/checkpoint.rs` now supplies a borrowed `MergeBoundary` and unpublished
section writers; it has no runtime call, accepted manifest or resume probe.
The constructor refuses poison, malformed array/bit shapes, unsupported G2,
bad B/sequence ranges and incomplete or overlapping Reserved accounting.
Requeue/deferred order and in-flight sequence/version descriptors are saved;
the fresh-only session prerequisite is explicit (session 1 plus current
counter). Restore must replace that prerequisite before issuing new sequences.
Disjointness uses reusable 8-KiB ID-window bits, without cloning all domains.
Boundary validation costs O(N + R*U + B^2), where U is the number of occupied
64-Ki-ID windows. Large scattered retry queues must be included in later
save-latency acceptance, not assumed cheap. Independent audit requested two
additional defenses: reject both ledger and node residual-G2 bits independently
of counters, and require every in-flight version to equal k for lockstep.
Both are implemented with isolated mutation tests; source audit passed.

The section stream has a fixed 32-KiB buffer, incremental byte/hash accounting,
checked per-image scratch (at most 165 bytes), `create_new` immutable names,
file fsync and sticky I/O failure. It writes images, exact ledger words, raw
nodes, persisted live bits, edge runs, initial D-band anchors in node order,
dispatch/in-flight state, stale closure flags and frontier counts. It never
allocates a full encoded section or refreshes closure on save. Tests cover
byte/hash fidelity, attempts/order/B preservation, anchor-layout agreement,
poison/reservation mutations, partial/interrupted writes, write/flush failure,
and immutable non-advertised files. These test sources passed the consolidated
typecheck but have not executed.

These `.part` files are deliberately orphan-only infrastructure: no directory
publication or durable restart is claimed. Request/roles/record-tail binding,
remaining scalar/closure metadata, atomic manifest publication, full restore
validators and asynchronous stop integration are the next reviewed slices.

### Follow-on private metadata/publication source

The follow-on worktree `codex-epoch-s3-publish` starts from the frozen writer
commit `2cc3710d`; it does not change that tree's pending validation target.
The next source slice passed independent source audit and the consolidated
typecheck; execution remains pending. It streams
scalar counters, root rows, input frontiers, owner fingerprints, sealed record
segment descriptors and historical dominant-orthant slots. Scalars borrow the
existing counters. The request binding and owner-inventory digest are computed
once before the save path; owner fingerprints have their own streamed section,
so a large valid owner inventory does not discover a hidden topology-count cap
only at memory stop. A synthetic 20,000-owner source test covers that case.

Input metadata records the complete parsed query inventory count independently
of the admitted prefix, with an explicit admission status. Exact row order and
roles, all protected initial roots and ordered input frontiers are checked;
incomplete admission cannot be called complete or coexist with executed work.
Input-root coverage uses reusable 8-KiB windows, not a full second root map.
That census costs O(Q*ceil(P0/65536)) and currently runs twice per save (initial
validation and scalar writing); include it in save-latency acceptance alongside
the reservation checks, without claiming a measured overhead yet.

The private publisher orchestrates all sections from one boundary; it does not
accept external receipts that could mix state borrows. It reuses `Sidecar::seal`
and the existing atomic-file helper, then fsyncs section entries before installing
`epoch-internal-latest.json`. Only after that is durable does it advance
`epoch-internal-previous.json`; failure to advance previous is a successful save
with a warning, not a reclassification of the durable latest generation.
An I/O failure before latest installation is sticky, retains the prior authority
and leaves only unreferenced files. Rename success followed by directory-fsync
failure is different: the shared atomic helper returns an error, but latest may
already name the new generation. The store remains failed, makes no rollback or
durability-success claim, and does not guess which generation survived a crash.
This slice intentionally performs no cleanup.

The 64-KiB bounded self-digested manifest names exactly 15 sections with exact
generation-local paths. Its distinct format, `resumable:false` and
`restore_validated:false` are never accepted by an existing checkpoint probe.
The 1-MiB scalar cap does not include query rows or owner/record inventories.
New fault tests cover latest/previous ordering, pre-publication orphans,
post-publication warnings, manifest mutation, row-role/admission mutations and
retired orthant-slot persistence. These are source tests, not passed native gates.

Full streamed restore, the real cross-session counter protocol, preparation
interruption, crash attribution and runtime pre-join saving remain unimplemented.
The pinned Symbolica restricted permit has no public thread handoff API:
unlicensed W1 remains inline with its responsiveness limit. A licensed async
worker must validate authorization on the worker itself and account explicitly
for control execution; W1 capability is still open, not silently promised.

### Bounded provisional decode slice (source-only)

The follow-on publisher worktree adds a fixed 32-KiB authenticated reader and
fixed-width section decoders. They verify local filenames, regular-file lengths,
section magic/version/arity/kind/count and checked count-times-width against the
actual remaining bytes before reserving count-sized runtime storage. Edge headers
count runs, not words: their minimum run bytes must fit, actual body bytes determine
the owned word allocation, and decoded run cardinality must match exactly. Each decoded
section remains provisional until full consumption, exact EOF and its digest pass.
Canonical images reuse the existing job-image decoder and summary construction;
no Symbolica state or new CAS serialization is introduced. Ledger words preserve
attempts/guard bits and rebuild counts; unsupported residual flags and invalid
alias direction/range are refused. Complete cross-state ledger checks remain open.

Only final runtime arrays are allocated: canonical arena/summaries/exact shards,
raw flags/live words/ledger, and the owned u32 edge-run log. Per-image scratch is
165 bytes. The edge log validates endpoint ranges, strictly increasing targets
and exact run boundaries, then supplies a repeatable borrowed pair iterator.
Closure construction moves owned flags and builds the final CSR in two passes;
equal edge counts alone are insufficient, so both passes must also match their
ordered digest. No full expanded pair vector, bucket partition or flag clone is
created. Source tests cover equal-count changed target distributions, malformed
run lengths/endpoints/order, forged huge counts with tiny bodies, flags/live
padding, all ledger word layouts and duplicate/noncanonical canonical images.

The arena decoder deliberately leaves lookup indices empty and orthant slots
unset. Reindexing only persisted live IDs and independently validating historical
orthants (including retired full-orthant IDs) is a required next gate, not inferred
from a newly constructed antichain. Other remaining obligations: manifest/scalar
and actual diagnostic-B binding, counts versus request/resource limits, sparse
frontiers/anchors, run uniqueness/seals/records and record digests, closure versus
ledger state, complete/partial initial roots and role binding, exact dispatch
replay and journal-backed cross-session reservation. Merely using saved_session+1
would reuse sequence IDs after a crash before a new generation; no runtime resume
is enabled by these section readers. New regression execution is still pending.

Negative source evidence: after committing `daad0376`, next-interface inspection
found that its edge reader incorrectly interpreted the writer's run count as a
word count. Its real-writer roundtrip regression would fail for nonempty edges;
neither source audit nor an unrun test constitutes execution evidence. A narrow
follow-up corrects the count/length contract and adds explicit empty-run, variable
target-count and mismatched-inventory mutations before any runtime exposure.

The independently source-audited lookup reconstruction (typecheck/execution
pending) consumes that provisional arena and
persisted live words. It inserts exactly the live IDs in original ID order using
the existing index path with no retire set; it does not recompute an antichain.
It independently replays the full-orthant slot rule over every canonical historical
ID, including retired IDs, and compares every streamed slot with that history.
The orthant header/count/actual byte length is checked before index allocation.
Tests include a retired dominant orthant, sparse absent slots, two contained IDs
that must both remain live, wrong rank/ID/shape slots, live padding, bad digest and
actual domain/live/orthant writer output. Full state assembly and all remaining
validators listed above still gate any usable EpochState; no performance speedup
or public restart is claimed.

## Proposed S3 state and writer contract

Implementation is confined to epoch modules and the narrow request/CLI stop
plumbing needed to carry a reason. Existing legacy checkpoint semantics and
pending-growth computation, naming and presentation stay unchanged.

`MergeBoundary<'a, N>` borrows `&'a EpochState<N>`. It is issued only at a
coherent initial, completed-P3 or bookkeeping boundary, refuses poison, and
the same borrow covers the entire save. There is no separately supplied
mutable state or independently constructed token. Reserved jobs can remain
in flight while a boundary is borrowed; no result is merged, refill changes
the ledger, or dispatch bookkeeping changes until the writer returns.

A first CP6 writer uses bounded buffered streams, with digests updated while
writing. It does not accumulate domains, ledger, nodes, live bits, edge runs
or the complete anchor section in a byte vector. Each image/anchor record is
encoded separately. Immutable generation filenames and `create_new` prevent
overwriting an earlier section. Record and edge/domain tails are sealed;
an open segment is never referenced by a published manifest. A manifest
names all section counts, byte lengths, digests and append watermarks.
Its own digest covers the unsigned manifest bytes. Sections and directory
entries are fsynced before atomic publication; latest publishes before
previous. A post-publication cleanup failure must not invalidate success.

The persisted state includes canonical images, full ledger words (attempts,
guard and last-error counters), node flags, live bits, anchors, edges,
records, walk counters, initial prefix, frontier counts, closure state,
query-root map and exact roles, request/owner identity, B16, dispatch cursor,
requeue/deferred membership and order, sequence counter, session number and
every Reserved/in-flight descriptor. Queue cardinality must agree exactly
with the ledger; no native result is itself part of checkpoint state.
The actual diagnostic B is persisted and compared, not merely its default
16: a resume under a different `RUSTRED_EPOCH_LOCKSTEP_B` is refused.

Fresh-only means that an epoch run starts from inputs and may subsequently
restore its own CP6. It does not import LC2/CP5 or the nonresumable S2 export.
The writer may land internally first, but the public probe, CLI and result
must not claim resume support until the matching restore and stop tests pass.

Roles are request-bound. Reserve explicit amendment/quarantine/abandonment
provenance in the schema before freezing it. This first execution slice
refuses nonempty rescue state and G2 activation at load as well as admission.
Its exact-image uniqueness validation is conditional on that restriction:
rescue-authorized duplicate images are not globally forbidden by CP6's
design. A later supported rescued state must authenticate its amendments
and eligible representative, preserve old IDs/dependencies and independently
validate current eligibility. It must not silently drop a quarantined copy.

## Stop and restore contract

The current `RunBatch` performs an unconditional `recv` for every result;
`with_pool` joins before the caller can save. Replacing only the final export
does not fix cancellation latency. The next source slice needs separate
submit/poll/cancel operations and a checkpoint callback inside the scoped
pool's body, before thread joins. A bounded polling interval observes an
external stop while a native is still running. The worker queue stops
starting new jobs once cancelled. Returned-but-unmerged results are either
accepted as a complete coherent cut or dropped; no cancelled prefix seals.

For lockstep cancellation, persist the exact unfinished batch and dispatch
state so restore reruns the same reserved work against the saved merge
version before admitting new Pending work. Retrying a cancelled inspection
does not increment attempts or guard. A policy/error stop persists the
completed cut, including every deterministic NativeError as unsealed and
terminal. Restore never redispatches those errors. Preflight failure leaves
logical state unchanged before the bookkeeping boundary is saved.

A memory stop cancels immediately and saves with bounded writers, preserving
the last valid closure snapshot instead of forcing a memory-heavy refresh.
A reason is read with a 4-KiB bound; host-wide/unattributed pressure never
charges job counters. Attributed suspects require durable provenance;
guesses from all Reserved IDs are prohibited. Crash-attribution sessions
must survive crashes before a new generation and must not double-count on
later restores. Poison is sticky, blocks saving an interrupted P3 state and
prevents certification from being restored by reopening an old generation.

Saving before join gives a durable recovery point; it does not prove a
bounded native join or authorize detaching borrowed worker state. A test
must hold one worker behind a latch, observe the manifest becoming durable,
then release the worker. W1's current inline executor also needs an explicit
solution before promising asynchronous stop handling for every worker budget.

Restore verifies bounded manifests and section paths, all section hashes,
counts and watermarks, exact request/owner identities, ledger tags and saved
reservation bounds, seals, aliases and containment, anchor scopes/stamps and
edges, native record/run correspondence and both digests, roots and roles,
live-index membership, closure validity, and dispatch completeness. Validation
must finish before any worker starts. Layer/index reconstruction uses the
persisted live set; merely rebuilding a new maximal antichain is insufficient
for byte identity. Dominant orthant slots also retain historical choices,
potentially pointing at retired full-orthant images: preserve and validate
them, or reconstruct their all-image insertion history separately from the
live-only index. Test a retired orthant plus a live subset explicitly.
The existing tracker `from_parts` clones flags and builds/partitions full
edge-pair arrays. Bounded restore instead needs a narrow owned-flags path and
a two-pass iterator over authenticated runs to build its runtime CSR without
those full temporary copies; test byte-equivalent closure restoration.
Open/orphan tails are never treated as committed state.

## Required consolidated validation

Before the preparatory merge is integrated: source audit; release typecheck
and focused epoch/role/G2/rescue tests; full affected app/CLI/Python suites;
flag-off Ordered identity and full independent reinspection on FG, BMW, H,
X, four-all, four-all-p5, finite-five, with the hot-sector G2 control. Historical S2
receipts do not replace those new-baseline checks.

Before CP6 is advertised: pause/restore at multiple merge boundaries and
worker widths with B16; save while one native cannot yet return; late-result
discard; W1 cancellation; preparation/input-admission interruptions; memory
stop without a full-section allocation; stop-file attribution; error stop
without redispatch; forced failure before/after every publication boundary;
fallback to previous; orphan tails; changed-bound refusal and allowed
worker-count change; attempts/in-flight roundtrip and abnormal-session loop
handling; byte, count, role, root, live-bit, edge, alias, anchor and ledger
mutations; sticky poison. Native stop/crash drills are owned test processes
only, never LC2. No compile/native/heavy job runs until root allocates it.

S4 inspector lookup, S5 parallel merge and S6 rolling/replay remain separate.
This contract adds no algebra primitive and no custom CAS work.
