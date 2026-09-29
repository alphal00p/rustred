# Epoch S2 integration and S3 checkpoint contract

Status: preparatory source integration; S3 design under independent review.
No compilation, native execution, campaign pilot or deployment claim is made.
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
for byte identity. Open/orphan tails are never treated as committed state.

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
