# Epoch S2 integration and S3 checkpoint contract

Status: preparatory integration, corrective source, internal writer and private
publisher passed independent source review and consolidated release typecheck
at `6f7eb9fd`. The first typecheck failure is retained below. Bounded section
decoding and lookup reconstruction passed independent source review and the
lightweight release typecheck at `33b0c2ab` after a test-only correction. The
subsequent manifest/scalar-bound assembly passed independent source review and
the lightweight typecheck at `43253bfe`. Anchor/frontier and saved-dispatch
decoding passed independent source review and the batched lightweight typecheck
at `97787259`. Cross-state validation passed independent source review and the
lightweight typecheck at `613e9c6f`. The registry/roots/counter slice passed at
`c51839c1`; authenticated record bodies and private restore/session lifecycle
passed at `52d7f57f` (41.178s guard, 37.33s Cargo, 1,226,180KiB peak RSS).
The polling executor has independent source review only; its compilation and
all new runtime tests remain unexecuted.
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

The follow-on worktree `codex-epoch-s3-publish` started from the frozen writer
commit `2cc3710d`; only audited committed slices subsequently advance the separate
validation worktree, never source edits during a running check.
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

### Bounded provisional decode slice (private; no runtime validation)

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

The independently source-audited and typechecked lookup reconstruction (runtime
execution pending) consumes that provisional arena and
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

The first lightweight check of decoder/lookup source `dc046711` failed after
37.172 s with a test-only E0277: the new owned-closure assertion compared runtime
`(usize,usize)` endpoints with persisted `(u32,u32)` endpoints. An independently
reviewed explicit widening preserves exact pair order/equality. Source `770ff058`,
cherry-picked as `33b0c2ab`, passed the one-worker release metadata-only retry in
38.168 s, exit 0, reason null, CPUs 16-19/build1, peak single-child RSS 1,171,996 KiB.
Both receipts remain unchanged at `TMP/codex-epoch-s3.JjASCU/typecheck-restore-light`
and `typecheck-restore-light-retry`. No tests executed; the later assembly source
is not covered by that PASS.

### Manifest/scalar-bound provisional assembly (private; no runtime validation)

The next private assembler uses the existing bounded manifest reader, a single
borrowed/owned generic scalar schema (same writer JSON), and authenticated owner
streaming with 67 bytes of per-owner scratch. It checks request identity, canonical
owner inventory, semantics 3 and the actual diagnostic B, initial-prefix status,
ledger/run inventory, basic closure-counter ranges and checked cross-file counts
before domain-sized allocation. The arena must fit both saved and current requested
domain limits; raising aggregate allowances or changing worker width does not
change the semantic binding. Six section decoders feed the provisional arrays and
lookup reconstruction. Header counts are not universally word counts: edges retain
the corrected run-count/body-word distinction.

This returns only `Provisional` parts, not EpochState, Dispatch, a resumed record
sidecar or an accepted CP6 checkpoint. Input/root rows, frontier/anchor/dispatch
sections, record bodies and complete cross-section mathematical validation remain
pending even though their manifest inventory exists. No worker or public resume
path calls this assembler. Source tests exercise actual private publication through
provisional assembly, incomplete admission, identity/B/limit mismatches, overflowed
counts and unsupported provenance, changed owner content with rehashed files, and
forged cross-section counts. Runtime execution of this slice remains pending.

That assembly (`03bd236d` source, `43253bfe` validation) subsequently passed the
same one-worker metadata-only check in 39.162 s, exit 0, reason null, peak child
RSS 1,184,068 KiB; receipt `TMP/codex-epoch-s3.JjASCU/typecheck-assembly-light`.
It is not runtime execution and does not validate subsequent source changes.

### Initial anchors and sparse frontiers (source-only)

The next assembler additions reuse the canonical outer section header and the
existing anchor codec with 58-byte scratch (10-byte codec header plus one 48-byte
InitialDBand record). The full section count/length and each record's kind,
single-anchor count and 8-byte scope are checked before the generic codec; G2
records remain refused. Global order, range, dispatch-version bound and initial
anchor provenance are checked without materializing a complete encoded section.
Sparse frontier rows retain sorted IDs and nonzero counts in the existing runtime
map. Actual writer→reader tests and rehashed count/layout/provenance mutations
cover these codecs; complete anchor cover/ledger/edge mathematics remains a
separate required validator, not established by these isolated codec fixtures.
Independent source review caught that the reused `AnchorMap::decode` was still
test-only. Its `cfg(test)` gate was removed without changing codec behavior or
visibility, before any build of this slice; no second decoder was introduced.

One retained semantic obligation: `frontier_counts` contains C4 records only,
whereas `walk.frontiers` accumulates retained frontier rows from every merged
result. A C2/error prefix can therefore contribute to the aggregate without a
sparse-map entry. The complete-state regression matrix must include that case;
restore must validate the map against C4/NativeFrontier records and aggregate
totals against all records, not force their sums equal or rewrite current rules.

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

The provisional dispatch decoder shares the writer's borrowed reservation
validator. Actual section bytes bound queue allocations and in-flight B before
storage grows; it preserves queue order, retry counters and every saved sequence
and lockstep version. The exact Reserved inventory is complete and disjoint,
including pending-cursor and attempts-class checks, without a domain-sized clone.
It returns saved data only, never a runnable `Dispatch`. Tests use actual writer
bytes and mutate counts, sequence/session ranges, queue membership, versions and
digests. The refill interface now rejects a zero budget before consuming any
deferred work. This slice passed independent source review and the batched
lightweight typecheck at validation `97787259` (source `7f31433f` maps to
`d329cb18`, source `a21d76bc` to `97787259`). The unchanged receipt is
`TMP/codex-epoch-s3.JjASCU/typecheck-dispatch-light`: exit 0, reason null,
40.167 s guard / 37.34 s Cargo, maximum single-child RSS 1,179,528 KiB,
minimum available RAM 805,331,378,176 bytes. Only CPUs 16-19, one Cargo
worker and the build-1 lock were used; no native tests executed. The later
cross-state source below was excluded from that frozen validation tree.
Before mutable session issuance lands, counter exhaustion must be checked before
increment/bit composition and session exhaustion before durable reservation;
neither may wrap, and a crash before the next checkpoint must not reuse a session.

The subsequent provisional cross-state validator binds node seals/inspection and
anchor flags to ledger tags, validates merge epochs and retry/error classes,
requires aliases to be forward, unprotected, retired and contained using the
existing containment authority, and checks one exact run per merged ID. A spare
bit in the owned unpublished node buffer provides O(N + E) run-source census
without another domain-sized allocation; the bit is rejected in input and
cleared on ordinary success/error. Initial D-band covers reuse the existing
anchor validator; anchor edges are checked in borrowed run slices, without a
full edge map or target-vector clones. The appendable records hasher is rebuilt
from ledger tags and actual runs and committed only after digest comparison.

Only ledger-derived aggregate counters are checked here. In particular, initial
input obligations and sparse C4 frontiers form a lower bound on the aggregate;
C2 error prefixes may also contribute. Author follow-up inspection caught the
initial draft omitting the input-obligation contribution (initialized at
`epoch/mod.rs:497`) before audit completion; the bound now includes it explicitly.
Regressions cover input-only frontiers and a C2 aggregate larger than the sparse sum.
Independent review also required rejecting merge epoch zero: the raw ledger
codec permits it, but actual P3 stamps start at one. The cross-state validator
now requires every merged native epoch in 1..=k and has a zero-epoch mutation.
Local closure flags must agree with nodes; a claimed closed source with an
unclosed descendant is rejected, including a Pending-descendant mutation.
Stale closed bits remain provisional and do not become certification: owned
closure reconstruction and its existing full counter/graph validator still
precede runnable state. Record bodies, roots/roles, whole-query admission and
durable session/replay still remain required. This cross-state slice passed
independent source review after the two corrections above and the lightweight
typecheck at validation `613e9c6f` (source `8cea173a`). Receipt:
`TMP/codex-epoch-s3.JjASCU/typecheck-cross-state-light`: exit 0, reason null,
40.165 s guard / 37.54 s Cargo, maximum single-child RSS 1,211,848 KiB,
minimum available RAM 798,267,813,888 bytes; same CPU16-19/build-1/one-worker
metadata-only allocation. Execution remains pending.

The next provisional record-segment slice streams the existing serde descriptor
array into the final Sidecar registry, reserving each slot only after its actual
descriptor is decoded and its generation/path/count/tiling checked. A 512-byte
per-descriptor read budget bounds untrusted JSON string scratch; this exceeds
the largest canonical descriptor (four u64 fields, fixed filename and hex
digest), rather than imposing a record-body or topology-count limit. Even a
forged large section count cannot lend its entire byte allowance to one string.
Every referenced sealed body is authenticated with fixed 32-KiB scratch; orphan
tails are ignored. This first traversal does not parse or validate record
semantics, and introduces no new record-line cap. Compact source-field visitors
must replace the raw traversal before complete state construction. Tests use
actual Sidecar segments, large bodies, fixed-shape maxima, count/tiling/path and
body/hash mutations. Independent source review passed, including the pinned
serde_json reader's one-byte lookahead contract; compilation and execution
remain pending.

The next root decoder is explicitly prepared-reducer-bound: expected Apply,
Route or source-refusal is derived from installed owners, the bound overcover
flag and source-condition capability, never a saved phase. Ordered rows must
introduce exactly the protected initial prefix; first admission equals its
canonical query, while an earlier reused root must contain the query through
the existing verifier. This monotone admission census uses no additional root
bitmap/map and supports partial admission without relabeling it complete.
Exact roles/default-required semantics and complete initial source-obligation
geometry are retained in the final input rows. A per-row serde budget is derived
from the already loaded query ID length and arity, not an arbitrary global ID
limit; long escaped/Unicode IDs are supported and forged long fields bounded.
Both JSONL readers must finish their authenticated lengths/digests. The narrow
reader budget is shared with the descriptor decoder without changing its limit.
Independent source review passed; compilation/execution remain pending. Current
root tests exercise the geometry reader and phase table; the actual authenticated
reducer-to-Provisional root path and writer/reader integration remain required
runtime gates, not implied by the source review.

Record-field gap found during body-reader design: native stats can charge a
breaking successor before the resolver refuses it, so their successor counters
cannot reconstruct the accepted P3 counters on C2. The approved epoch-only
`epoch.resolver_counters` subobject uses one typed versioned writer/reader schema
for accepted successors/conditional successors, optional-refusal triplet and
route mask counters. A checked small aggregate compares these exactly to saved
walk counters; unknown fields, unsupported versions and arithmetic overflow
cannot silently alter a partial sum. Existing algebra/report fields, legacy
records and pending-growth calculations/rendering are unchanged. A synthetic
P1-P3 C2 regression distinguishes two charged native successors from one accepted
resolver successor and rejects substitution of that charged count. These source
changes passed independent source review; compilation/execution remain pending.
The semantic body visitor must invoke checked add/equality (serde alone admits
an unsupported version), and validate the phase-specific zero fields. It will
require this fresh-only field rather than import old S2 records.

The follow-on body visitor now replaces that byte-only segment traversal in
provisional assembly. It parses restore-authoritative geometry, class/scope,
run inventory, parity and exact resolver counters through the same authenticated
reader, followed by complete-consumption and digest checks. Selected fixed
fields are individually bounded (8 KiB; canonical geometry has at most 32 axes,
stats and epoch objects are fixed scalar schemas); diagnostic/error/refusal
payloads are skipped with serde IgnoredAny and frontier entries counted, with no
whole-record JSON allocation or new record-line cap. Native kinds enforce their
zero-counter fields and version; C2 prefix frontiers contribute alongside C4
and initial-input obligations. The existing P1 classifier and error-class map
are reused rather than restated. Alias records contribute no native totals.

Added source tests drive actual P1/P2/P3 and Sidecar output for C0/C4/C2,
charged-native versus accepted-resolver counters, recurring panic and Route;
review added actual two-merge InitialDBand C0/C2 residual roundtrips and bound
each anchor's dispatch version to its native body's v0 (not merely < merge k).
Tests also cover aliases, rehashed authority mutations, aggregate overflow, record
order/inventory, large skipped diagnostics and a semantically identical body
with the wrong digest. These tests are not executed yet. This remains private
provisional assembly: prepared-root integration, final Tracker reconstruction,
durable session allocation/replay and save-before-join are still required for a
runnable restored state. The last passing metadata check is validation commit
c51839c1 (source through c772f9c6), not this body-visitor work. Its receipt is
`TMP/codex-epoch-s3.JjASCU/typecheck-roots-records-light`: exit0/reason null,
39.177s guard / 35.77s Cargo, maximum single-child RSS1,209,024KiB, minimum
available RAM784,696,246,272B, CPU16–19/build1/one metadata worker. The body
visitor passed independent source review after the partial-version correction;
its compilation and all test execution remain pending.

The next cohesive private lifecycle slice consumes the provisional arrays and
prepared-reducer roots into an EpochState, moves closure flags into the existing
two-pass CSR builder and calls the existing Tracker restore validator. An
unavailable monitor receives no optional CSR edges, while the mathematical edge
log remains intact. A restored input prefix retains InProgress and its Dispatch
refuses scheduling; continuing initial admission is still runtime wiring work.

The publisher reopens under its exclusive lock, refuses sticky poison, scans
generation filenames without collecting them, and skips orphan state/record
generations. Latest is fully validated before use; rejected latest can explicitly
fall back to fully validated previous. A changed request/owner/B or insufficient
requested domain limit is an InvalidInput refusal, never permission to silently
select a smaller old generation. Other latest failures are reported when previous
is used; both failures retain their reasons if neither generation validates.
This private policy includes allocation/permission/I/O failures, not only
corruption. A selected generation whose saved session exceeds the journal
high-water is conservatively refused at adoption, without trying previous.

A separate fixed48-byte authenticated session high-water file is atomically
installed and directory-synced before issuing a restore Session capability.
It advances independently of merge saves: repeated crashes before the next save
cannot reuse saved_session+1. Missing/corrupt/out-of-range journals fail closed;
no old private writer snapshot is imported. A rename/fsync uncertainty returns
no capability and is sticky for that publisher. Dispatch reissues the saved
unfinished batch in saved sequence order with fresh sequence numbers, preserving
attempt/guard words, queues and k; new Pending work cannot overtake replay.
Counter exhaustion is checked before queue/ledger mutation and maps to a safe
capacity stop, never wrap. Session exhaustion refuses before reservation.

Source tests now connect actual owner preparation, private save, restored
EpochState/Tracker, replay, real P1/P2/P3 error merge and resave/restore. They also
cover crash-before-save session monotonicity, replay order, unchanged attempts,
partial-admission refusal, unavailable monitoring with retained mathematical
edges, orphan generation skipping, explicit previous fallback, exclusive lock,
poison, changed-limit non-fallback and closure-counter corruption before session
reservation. This private slice passed independent source review and the
lightweight metadata-only typecheck at `52d7f57f`; all test execution is pending.
Fresh entry integration, runtime stop/memory polling, durable save before worker join,
session crash-attribution policy and public resume remain unimplemented gates.

For any eventual performance/deployment comparison, the contemporaneous legacy
baseline must include its best validated configuration (G2 Union if it qualifies
in matched pilots). Private epoch still refuses G2/rescue; that reduced capability
is not production equivalence. Complete-workload throughput and native-calls/s
are distinct measurements; neither the private lifecycle nor source checks claim
the required >=1.5x matched throughput gate.

The shared scoped executor now provides bounded submit/poll/cancel operations;
the existing S2 `RunBatch` is an adapter over that same pool, with inline W1
unchanged. Its still-blocking adapter does not itself enable S3 runtime saves.
Cancellation closes the queued-work path before pop, drops queued payloads and
the result receiver, and leaves the caller inside the scope to save before
joining. Status distinguishes worker-accepted/returned from merely Reserved;
it is not memory attribution. Every job has exactly one ordered start/result
receipt. Independent source review found a liveness gap when queue poison left
idle sibling senders alive; poll now checks poison before waiting and after a
timeout, with a synthetic outstanding-work/live-sender regression.

The new source test holds one worker behind a latch, durably publishes a real
private checkpoint, independently reads its authenticated manifest while the
worker has not returned, and only then releases it. Its late invalid 1MiB result
is discarded; full restore replays all three unchanged Reserved entries with a
fresh session. No forced closure refresh occurs in this RamGuard test. Other
source tests cover queued jobs never starting, repeated batches, panic receipts
and cancellation races. These tests are unexecuted. No responsive native license
path or runtime controller is claimed yet; extra start/status traffic in the S2
adapter has unmeasured overhead. Public resume remains refused.

### Private restored-state controller (source slice, tests pending)

The next private controller connects restored replay, polling and the existing
P1-P3 merge path. A complete replay cut must finish before Pending admission.
An observed cancellation discards both already-polled result bytes and the
queued/late channel before streaming the coherent Reserved state. The cancelled
B-sized status inventory is moved, not newly allocated, into the stop callback;
statuses from an earlier merged cut are never reported as current in-flight.
Completed error/frontier/preflight stops save their existing merge/bookkeeping
state. C5 installs a sticky atomic poison marker, not a new generation. The
native wrapper checks LicenseManager::is_licensed on every actual worker before
the controller can submit anything, honors explicit inspection partitioning,
and requires inspector+controller
budget >=2. Public unlicensed W1 remains inline and unchanged.
Independent review rejected the first wrapper's empty-result-on-missing-license
path: it incorrectly poisoned a valid checkpoint as C5. A narrow worker
authorization barrier now returns a typed operational Capability refusal,
leaving replay and latest untouched and never entering CAS. The regression
refuses, reopens under a fresh session, changes capability, then completes and
restores; genuinely malformed inspection results still poison. Review
found pre-dispatch handle-allocation/thread-spawn failures taking the same C5
path; these now use Resource refusals and an injected mapping/reopen regression.
Review also
restored the original initial-frontier startup stop, and drained saves retain
drained_uncertified when the existing certification predicate refuses.

The existing CLI stop-file path is passed as optional epoch-only operational
context, excluded from mathematical binding. After cancellation, at most4KiB
is parsed. Empty/missing/malformed/oversized metadata still stops with explicit
unknown context. Only the three documented RAM reasons map to ram_guard;
host-wide pressure without own-memory evidence is unattributed. Typed scalar
metadata persists the reason/context and validates consistency on restore.
Tree attribution does not itself charge any job or all Reserved descriptors.

Source regressions connect a genuinely polled invalid prefix, held second
worker and never-started third job to durable save-before-join and full replay;
cover C2 terminal error resave, C5 poison, replay-before-Pending, native wrapper
authorization and actual native records, W1 refusal, manual empty file, signal/API
flag with no context, supported RAM reasons, malformed reasons and unbound path.
Independent source review passed after those corrections; compilation and
execution of this controller slice are pending. These are source-found defects,
not reproduced runtime failures. Worker startup authorization is not timeout-
bounded. The initial-frontier helper test is not full prepared-frontier restore
integration. The controller remains
private: fresh/input-prefix continuation, periodic-save scheduling, cooperative
grace (this private controller cancels immediately for either operational stop),
durable Start/Grow/End attribution/clean-session crash policy and all deployment
gates still precede public resume. No bounded native-join guarantee is made.

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
