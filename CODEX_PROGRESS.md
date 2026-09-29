# Codex progress: five-loop optimization and controlled deployment

Authoritative plan: [CODEX_PROGRESS_PLAN.md](CODEX_PROGRESS_PLAN.md).
Root orchestrator owns this log; agents report evidence for integration here.
`[M]` denotes observed/measured evidence; `[E]` denotes interpretation or estimate.

## Audited starting state — 2026-09-29 00:30 UTC

- [M] Main checkout: `fable_5_1`, starting tip `2255bc06`, synchronized with
  `origin/fable_5_1` at the preceding audit. Symbolica gitlink `ef0db494`,
  patch-free upstream dev; native artifact format 6.
- [M] LC2 is the live campaign: `campaigns/five-loop-qcd-feynman-d9d10-lc2`,
  run `20260928T234230.071605Z`; frozen binary `rustred-lc2-fd9b9ac9`.
  At the preceding read-only snapshot (2026-09-28 23:55:33 UTC): 5,379,517
  discovered, 1,247,355 native completions, 2,125,114 pending, 2/67 initial
  obligations recursively closed, zero frontiers, 9.96 GB RSS. This is a
  historical early snapshot, not a current rate or completion forecast.
- LC1 is paused and recoverable with its own frozen format-5 binary. Do not
  alter either campaign. LC2 owns CPUs 128–227 and up to 600 GB; development
  must preserve host headroom and existing build/heavy-job locks.
- [M] Frozen input census: 116 required physical/convenience queries plus
  67 helpers; descendants are not clipped. Existing helpers: 54 finite-A,
  13 unbounded-A. Physics assumptions remain the approved Feynman-gauge QCD
  auxiliary-mass scope, not an arbitrary-theory coverage theorem.
- [M] Existing unrelated work preserved: `crates/rustred-feynkit/src/lib.rs`,
  `EPSILON.md`, `Janet_Ore_reference_code_usefulness_study.md`,
  `tools/research/symbolica_arc_mre/`, `vendor/OLD_vendored_symbolica/`.

## Baseline validation and measurements (prior receipts, not rerun here)

- [M] LC2: core 2844 passed / 32 ignored; app 822 / 12 ignored; CLI 6;
  Python 260 OK. Seven strict Ordered identity and independent full-reinspection
  controls passed. Sources: `docs/research/fable51_lc2_patchfree_2026-09-29.md`,
  `TMP/lc2/gates/gates.tsv`, `TMP/lc2/gates/oracle.tsv`.
- [M] Thread-owned Symbolica contexts: K96/K1 inspection CPU ratio 1.034–1.044
  in the recorded harness; this is not whole-walk scaling. Source:
  `TMP/progress/symeval-dev.report.json`.
- [M] G2 production-branch pilot: C-5F scheduled domains 0.756–0.761x,
  CPU 0.514–0.548x; hot-sub control scheduled 0.721x, CPU 0.271x. Two repeats;
  not a complete production campaign. Evidence:
  `.claude/worktrees/fable51-g2f/docs/research/fable51_w1_g2prod_2026-09-28.md`.
- [M] Epoch fix-round gates did run despite stale handoff text:
  `TMP/epoch-s2/runs/fixround-r2.log`; seven oracle passes and cross-width
  identities. C-5F W24 traversal 565.63 s. S2 is not the completed performance
  architecture and has not met the deployment speed gate.

## Work ownership and backlog

| Work | Status | Responsible lane | Next step / reopening condition |
|---|---|---|---|
| Plan, goal, progress bootstrap | delivered | root | Goal active; independently audited documentation milestone |
| G2′ integration | corrected focus/full native suites delivered; campaign gates pending | joint_support_pruning | Build frozen matched-profile CLI and run all controls; retain first failed receipts |
| Rescue and explicit query roles | source and combined native suites delivered | joint_support_pruning (integration; original author bounded_helpers_bmw) | Campaign/CLI and independent full-reinspection gates remain at `d12db6cf` |
| Independent math/code audit | combined corrections and private epoch writer/publisher source passed | checkpoint_final_audit | Independently review bounded restore next; verify execution receipts before release |
| Combined G2′ + rescue | corrected optimized focus/full suites and independent receipt audit passed | joint_support_pruning + root + independent auditor | `d12db6cf`; matched campaign profile, CLI and independent control reinspection next |
| Python production G2′ steering | committed locally and independently tested | joint_support_pruning (author), checkpoint_final_audit (independent review) | `cd52c90d`; real Python/native pause-resume composition smoke after CLI gates, then integrate |
| Coordinator latency / telemetry | isolated source audit and typecheck passed; native execution pending | checkpoint_final_audit (author), joint_support_pruning (independent review) | `187854b4`; five regressions prepared, no native test or performance result yet |
| Epoch S3–S6 | writer/publisher typechecked; decoder and lookup reconstruction source-audited | epoch_s3_delivery, checkpoint_final_audit (independent review) | Frozen validation `dc046711` awaits check; full assembly/session/stop and S4–S6 remain open |
| N2 allocation-free geometry | registered; pending implementation decision | checkpoint_final_audit | Redundant vector copies; profile after G2 before prioritizing |
| N1 modular witnesses | source/API feasibility audit delivered; profile gate pending | checkpoint_final_audit | Generic RHS witness is too late/insufficient; measure fully fixed predicate support after G2 before implementing narrower shortcut |
| N4 coverage-first work | source feasibility audit delivered; profiling gate pending | checkpoint_final_audit + root | Opt-in coverage-only visitor could avoid discarded exact payloads; count post-G2 eligible cost before implementation, preserving mathematical obligations |
| Scheduling / ordering | pending | research/measurement | Compare work volume and censored Ready outcomes |
| Memory / checkpoint / NUMA | pending | profiling lane | Measure process-local opportunity without host-wide changes |
| New algorithms / literature | census delivered; narrow coalescing parked | work_reduction_research + checkpoint_final_audit | Reopen only with post-G2 miss-cohort evidence; backward covers deferred |
| Closed-descendant query witnesses | parked after negative census; independently reproduced | checkpoint_final_audit; independent critique by joint_support_pruning | Reopen only on post-G2/rescue evidence of earlier exact closed-descendant coverage |
| Required-scope versus broad-helper dependencies | source audit delivered; no optimization activated | checkpoint_final_audit + root | Whole-helper dependency granularity confirmed; net work benefit unmeasured; independent finite-query closure needed before reopening earlier negative approaches |
| I1 L*-helper input variant | deferred, not rejected | root + independent research/measurement lane | Revisit only after in-run rescue passes combined gates and a fresh matched probe shows net runtime benefit; preserve all 116 required queries |
| Earlier rejected levers | deferred | root | New evidence required; retain prior negative results |

## Decisions in force

1. The approved plan supersedes stale stop/launch directives; production is
   still owner-controlled. Never start/stop/signal/resume it from an agent.
2. Leave pending-growth calculation, name, and rendering unchanged.
3. Resume compatibility is a convenience, not a veto or a migration project.
   Recommend fresh runs when necessary; preserve old campaigns for rollback.
4. A passing internal microbenchmark is not sufficient deployment evidence.
   Use matched controls, two pairs for a switch recommendation, and the
   approved 20% benefit / epoch 1.5x gates.
5. Pilots <=30 minutes including restore, preparation and safe shutdown.
   Separate compilation; report unfinished runs as censored.
6. No extra terminal minimization, numerical-master or Vakint work in this stage.

## Event log

### 2026-09-29 04:04 UTC — lookup restoration frozen; timing-lane contention checked

- [M] Independently source-audited live-index/historical-orthant reconstruction
  committed as `79db22c93220fc695c4035a35262ba4a1c13aaed`. The idle validation
  tree now consolidates decoder, count correction and lookup slices at
  `dc0467110591b14548e20a119ad3da08c319ef73`; no check has run on that tip.
  Rebuild only persisted live membership, replay historical orthant updates
  separately, and never recompute antichain retirement during restore. Actual
  writer/header layouts were crosschecked during the independent review.
  This remains private reconstruction, not usable full-state/public resume.
- [M] Inert pilot recipes and hashes are in
  `TMP/codex-g2-pilot-prep.n7Kd5q/COMMANDS.md`; candidate executable identity
  remains pending. The proposed 64–79 lane contains 16 distinct NUMA2 physical
  cores, but a short snapshot showed 4.574 busy cores plus 0.110 on SMT siblings.
  That is material contention, not an exclusive reservation. A short read-only
  comparison of other permitted physical-core blocks is authorized before
  freezing the timed lane; no affinity/settings of other jobs may change.
- Candidate compilation remains the sole owned heavy job. No benchmark or
  production launch has occurred, and no speed/closure conclusion is drawn
  from these preparation observations.

### 2026-09-29 04:02 UTC — frontend milestone committed; composition smoke queued

- [M] Python G2′ steering is committed locally as
  `cd52c90d5280125f42f6c348a14f89e862206798` on `codex/g2-steering-lc2`:
  four explicit files, clean tree, independent final audit passed. Exact
  test/guard receipts remain in `TMP/codex-g2-rescue.tzdFuj/`. It is not yet
  merged into main, and does not change the frozen `d12db6cf` Rust build.
- The six existing CLI tests cover native composition, not the Python
  supervisor. After they pass, run a small real fresh-Union / cooperative
  pause / ordinary-resume / cold-verification smoke entirely under TMP.
  Reuse established supervisor ownership and stop-file handling; do not
  place a session-spawning supervisor inside an incompatible outer PG guard.
  This closes a process-chain gap left by fake frontend tests plus separate
  actual native amendment/activation tests. No production launch is authorized.
- Epoch lookup reconstruction (persisted-live index and independently replayed
  historical orthants) is source-written and awaiting independent review.
  It is not public restore. The author may consolidate the idle validation
  checkout after review, retaining all prior receipts; no second heavy build
  competes with the candidate CLI compile.

### 2026-09-29 04:00 UTC — Python steering passes final audit; decoder correction recorded

- [M] Final Python G2′ steering suite: 284 tests with one expected optional
  enumeration skip, 22.425 s; focused seven tests 1.085 s. Independent rerun:
  seven passed in 1.078 s, guard exit 0/no stop reason, 2.155 s, recorded in
  `TMP/codex-g2-rescue.tzdFuj/python-steering-independent-final/`.
  The reviewer confirmed unchanged off-mode argv/frozen bytes, sticky union
  across ordinary/rescue resumes, query preservation, and refusal of implicit
  activation and inconsistent frozen options. Both parsers already disable
  abbreviations; new tests prove that refusal without changing parser behavior.
- [M] Epoch edge decoder correction committed locally as
  `95599fcdbc132e317a954642330f877439e2a6b6`, independently re-reviewed.
  Allocation derives from actual body words; minimum bytes/run and exact
  decoded run count are checked. Empty/nonempty and mismatched-run regressions
  supplement the actual-writer roundtrip. This corrects `daad0376`; neither
  decoder commit has yet passed typechecking or native execution.
- [M] Read-only LC2 generation-3 save completed: reported 63.6895 s and
  24,909,444,805 bytes; domain/edge/index sections report 11.0366/5.0648/1.1047 s.
  No pause/resource intervention occurred. At 03:58:48, LC2 remains running
  with 41,753,211 discovered, 16,278,426 pending and 6/67 conservative initial
  obligations closed. One roughly minute-long save per four hours is not
  currently the leading amortized throughput cost. S3 durability/stop work
  remains necessary for epoch correctness, not a demonstrated checkpoint
  speed improvement.

### 2026-09-29 03:57 UTC — pre-release defects retained and corrected in isolation

- Epoch author found a missed interface mismatch after the decoder's initial
  source audit: the edge-section header counts runs, but the decoder treated
  it as a word count. Nonempty writer/reader roundtrips would fail closed.
  The auditor acknowledged the miss; `daad0376` is not a passing decoder
  implementation. A narrow correction and re-audit are underway, with the
  existing nonempty roundtrip regression still awaiting execution. Public
  restore remains disabled; the production engine is unaffected.
- Independent Python steering tests pass (seven fake-only tests, 1.062 s),
  but review found an alternate `--g2-residual-anchors=union` spelling could
  evade frozen mode/argv consistency checking. Generated policies use the
  canonical spelling. The author is fixing refusal of noncanonical frozen
  options; abbreviation handling is included in review. Do not publish a
  final source PASS until that correction and its tests are reviewed.
- Both findings illustrate why source audit/typechecking are not substitutes
  for runtime and mutation tests. The frozen G2′ Rust campaign build continues
  unchanged; neither finding affects its completed 861-test native gate.

### 2026-09-29 03:55 UTC — decoder slice delivered; frontend suite passes

- [M] `epoch_s3_delivery` committed independently source-audited bounded
  reader/fixed-section decoders as `daad03767f4cb7eadf0685fed9d73863eba7020d`
  in the isolated publisher branch. They validate count/length relationships
  before large allocations and finish section digests before acceptance.
  Owned flags and a repeatable, hashed two-pass CSR build avoid full temporary
  copies. Mutation regression sources cover forged counts and changed
  equal-count iterators. **This slice is not compiled/executed yet.**
- Full restore assembly, live index versus historical orthants, request/root/
  record validation, durable session reservation and save-before-join remain
  outstanding. Private readers do not expose a runnable or public resume.
  The last passing typecheck remains the earlier `6f7eb9fd` writer/publisher.
- Root and independent auditor recorded N4's actual late-materialization seam
  in `docs/research/codex_next_candidates_2026-09-29.md` section 9. New opt-in
  cost semantics may avoid unnecessary CAS and its resource failures; this
  does not waive poles/guards/source/descent/frontier obligations. No speed
  claim or implementation; post-G2 profiling is the prerequisite.
- [M] Separate Python steering implementation passes seven focused fake-only
  tests and full frontend discovery: 284 tests, one expected optional skeleton
  enumeration skip, 22.829 s. Guard exit 0/no stop reason, 24.170 s. Enumeration
  passed separately earlier on unchanged planner code; not rerun here.
  Independent review is underway. The compiling `d12db6cf` Rust source/cache
  is unchanged. Main progress milestone `9db7aa13` is pushed; unrelated user
  changes remain untouched.

### 2026-09-29 03:47 UTC — Python steering gap identified before deployment

- Root source inspection found that G2′ exists in Rust and native CLI but
  neither `production_saved_owner_campaign.py` nor its shared supervisor
  accepts/forwards it. Thus a normal Python-steered fresh campaign could not
  actually select the new optimization. No launch instructions will conceal
  this gap by hand-editing frozen steering.
- Delegated a narrow Python-only integration slice to `joint_support_pruning`,
  separately from the compiling frozen Rust source. Add explicit off/union
  selection, freeze it, preserve it through normal resume and automatic rescue,
  validate native prerequisites, and keep default-off argv unchanged. Separate
  author/auditor tests must cover those paths. Existing native one-shot
  activation remains advanced functionality; no new compatibility project.
- [M] Campaign-profile CLI compilation at `d12db6cf` is running under the
  registered CPU0–15/heavy/build0 guard, with `CARGO_INCREMENTAL=0`. Evidence:
  `TMP/codex-g2-rescue.tzdFuj/campaign-build/`. Normal Cargo dependency
  rebuilding is not a solver measurement; no candidate timings are available.

### 2026-09-29 03:45 UTC — epoch check passes; integration receipts audited

- [M] Frozen epoch source `6f7eb9fd` passes `cargo check --release --tests
  --locked --offline -j8 -p rustred-app`, with its isolated target directory.
  Guard exit 0/no stop reason, 25.166 s including launcher, Cargo 21.26 s;
  peak single-child RSS 1,162,360 KiB. Receipt:
  `TMP/codex-epoch-s3.JjASCU/typecheck-retry/`. This checks the writer,
  private publisher and regression sources, **not native execution or resume**.
  Original failed-check evidence is retained; restore remains source WIP.
- [M] Independent G2′ audit verifies clean `d12db6cf`, matching executable
  digest, all six corrected failures and new phase/pin/quarantine assertions,
  normal guard completion and no missing-license skips. The optimized
  campaign-profile build is next; no candidate campaign timing exists yet.
- [M] Read-only LC2 snapshot at 03:44:32 UTC: 40,622,489 discovered,
  14,344,464 native completions, 15,732,569 pending, 6/67 conservative initial
  obligations closed, roughly 29.45 GB RSS. Its own scheduled generation-3
  checkpoint is writing; no intervention. These are not 116-query closure
  counts or an ETA. In its hour-window coordinator accounting, commit is
  49.38%, preparation 28.86%, and progress JSON 1.66%; this supports prioritizing
  substantive lookup/merge work over overstating the isolated telemetry change.

### 2026-09-29 03:43 UTC — full corrected G2′/rescue suite passes

- [M] The same optimized executable passed **861 tests, 0 failed, 12
  ignored**, in 141.13 s. All six earlier failing tests pass, as do the new
  query-phase regressions. Guard receipt exit 0/no stop reason, 142.197 s;
  peak single-child RSS 82,992 KiB. Evidence:
  `TMP/codex-g2-rescue.tzdFuj/native-fixed-full/`.
- The 16-CPU test allocation also produces 12 explicit W50 capacity-skip
  diagnostics inside passing test bodies, separate from the 12 ignored tests.
  Independent source review identifies ten intended omitted W50 subcases;
  two further notices precede an existing <=6 filter and would not execute
  W50 anyway. Do not describe this as validation of every worker width.
  Focused tests have no ignored or license-skip cases.
- Authorized the frozen epoch `6f7eb9fd` consolidated typecheck next, then
  the unchanged G2′ source's campaign-profile executable and CLI checks.
  Incremental compilation is explicitly disabled for that timing binary;
  fat LTO/one codegen unit match LC2. No production recommendation follows
  until complete, independently reinspected controls and matched pairs.

### 2026-09-29 03:41 UTC — corrected G2′ focus passes

- [M] `d12db6cf` passed all 20 optimized focused tests, with zero ignored,
  in 8.30 s. All three previously failing public-pipeline tests now pass;
  all five applied-G2 mutations are rejected. Activation/amendment examples
  honestly report zero loans; dedicated loan/dependency seam tests also pass.
  This is a correctness result, not a measured campaign speedup.
- Receipt: `TMP/codex-g2-rescue.tzdFuj/native-fixed-focus/`, guard exit 0,
  no stop reason, 2,942.224 s including compilation (Cargo reported 48m51s).
  Executable SHA-256:
  `227d256455028cc84cfa9a530211ec56310df7173e8f3987129de70b77e6a749`.
- The full app suite is running on that same executable without rebuilding,
  in `native-fixed-full/`. An independent auditor is checking both receipts
  and the six earlier failing assertions. After it drains, the frozen epoch
  writer/publisher gets a consolidated typecheck; matched-profile G2′ CLI
  compilation and campaign controls follow. LC2 remains read-only.

### 2026-09-29 03:36 UTC — private publisher audited and validation source frozen

- [M] Author committed the private publisher/metadata slice as `2662a846`
  in `codex/epoch-s3-publish`, after independent source audit. The frozen
  validation tree now includes it as `6f7eb9fd`, atop writer `2cc3710d`.
  No running validation was altered; its consolidated typecheck has not
  started. Exact command/resources/source are recorded in
  `TMP/codex-epoch-s3.JjASCU/typecheck-retry-plan.json`.
- The writer streams variable owner inventory separately from bounded scalar
  metadata; a 20,000-owner regression source exceeds 1 MiB inventory while
  keeping scalar metadata below 8 KiB. This avoids a hidden owner-count limit
  discovered only during an emergency save. It reuses the existing record
  sidecar sealing and atomic-file helper, writing a fixed 15-reference manifest.
  New tests are source-only, not executed measurements.
- The auditor corrected publication semantics precisely: an error after
  rename but before successful directory synchronization is uncertain, not
  rollback. The store fails closed/sticky; a previous-pointer failure after
  a durably published latest returns a warning. Internal format remains
  explicitly non-resumable/unvalidated; no runtime save path is enabled yet.
- Bounded restore is being implemented only in the source worktree. Next
  requirements include actual root geometry, record/edge/digest validation,
  live-index versus historical-orthant reconstruction, exact dispatch and
  error state, and crash/fallback behavior. Do not mark S3 complete on the
  strength of the private publisher alone.
- Read-only build-workflow review suggests a later isolated incremental
  correctness cache (opt3, explicit CGU16, ordinary-release LTO=false) may
  reduce repeated edit-to-test latency. Warming cost/savings are unmeasured;
  current build is unchanged. Campaign timing binaries remain separately
  built with incremental disabled, fat LTO and CGU1. Exact proposal is in
  `TMP/codex-g2-rescue.tzdFuj/INTEGRATION.md`; no benchmark claim follows.

### 2026-09-29 03:22 UTC — research narrowed; source-audit milestone pushed

- [M] Main documentation/source-audit milestone `6f7c1db0` is pushed to
  `origin/fable_5_1`. Independent review corrected the I1/I1b distinction
  and the historical ratios' traversal-only boundary before publication.
  Main engine remains unchanged; unrelated user work is preserved.
- N1 source/API audit is recorded in
  `TMP/codex-integration/n1-modular-witness-feasibility-2026-09-29.md`.
  A witness inside the current RHS classifier is too late to avoid exact
  normalization and cannot authorize uniform nonzero. A narrower guard-only
  shortcut is potentially sound when all occurring index variables are fixed;
  formal coefficient-field nonzero does not mean nonzero at every numerical
  dimension/mass. Preserve every original pole/admission obligation. The
  cleaner guarded-engine seam is not on the live walker path, so it is parked.
  No code or speed claim; post-G2 opportunity profiling comes first.
- Independent licensing audit found no public restricted-thread permit
  transfer in pinned Symbolica. Preserve existing inline unlicensed W1 behavior;
  do not bypass licensing or silently oversubscribe its worker budget for a
  new checkpoint controller. Responsive stop support remains an explicit
  S3 design/test obligation, not a completed capability.
- [M] Read-only LC2 heartbeat at 03:16:14 UTC: 38,231,778 discovered,
  13,366,313 local native completions, 14,883,636 pending; 6/67 conservative
  initial obligations closed, zero frontiers, about 28.0 GB tree RSS and no
  stop reason. Those are not 116-query certification counts or a completion
  estimate. At 03:22:21, corrected G2′ rustc PID 2325595 was still live
  (1,874 s elapsed, about 48 GiB RSS); this remains the only owned heavy job.

### 2026-09-29 03:15 UTC — first S3 implementation slice committed locally

- [M] Independently audited internal writer is committed as
  `2cc3710d1e7a83e1df6ff1ce45eae8e77db6933a` on `codex/epoch-s3-lc2`.
  Five explicit paths; worktree/vendor clean. Audit added independent refusal
  of residual-G2 flags and exact equality of in-flight and saved lockstep
  versions. Geometry, reservation and I/O fault regression sources are present.
  No native/typecheck result for this slice and **no public CP6 resumability**.
- Reservation validation costs O(N + R*U + B²): ledger length N, all queued/
  in-flight descriptors R, occupied 65,536-ID windows U, in-flight bound B.
  Deferred/requeue counts are not capped by B. Fixed scratch does not imply
  bounded save latency; eventual tests must cover scattered large retry queues.
- Preserve this validation checkout frozen. Author will continue the next
  internal publication/metadata slice separately, with independent review;
  resume stays refused until full restore and stop gates pass. The corrected
  G2′ full native gate has priority, then a brief epoch typecheck, then the
  matched-profile G2′ campaign executable and its unchanged CLI tests.
- W1 interruption design is being audited against actual Symbolica licensing
  APIs. Caller-thread fault injection is only a test seam, but unlicensed
  Symbolica retains a real thread-local permit. Do not infer safe thread
  handoff or introduce a second checkpoint authority without that audit.
- The required-scope/helper finding and its prior negative evidence are now
  recorded in `docs/research/codex_next_candidates_2026-09-29.md`; full local
  source references remain in the scope-alignment audit note. No new helper
  experiment, production change or campaign-switch recommendation.
- Independent documentation review caught an important historical distinction:
  I1 was provisional with its stated criteria met, whereas I1b was dropped.
  Corrected the draft rather than treating both as rejected. Existing owner
  decisions explicitly allow I1 reconsideration once in-run rescue exists and
  a probe shows net runtime benefit. Combined rescue gates are not yet green;
  retain I1 as a conditional follow-on, not an automatically enabled change.

### 2026-09-29 03:07 UTC — scope mismatch understood; bounded writer under review

- [M] Independent source audit confirms that admission can map a finite
  required query directly to a wider helper. Recursive closure then requires
  all of that helper's outgoing dependencies, not only a demand-restricted
  slice. Initial-overlap and G2′ save local inspection work but retain whole
  anchor dependencies. This is conservative extra work, **not false closure**.
  One frozen example is `owner-anchor-r6-anone-000011001001011` versus its
  required `conv-d10-a16-r6` query: the latter also bounds A and fixes D.
- Do not reopen bounded-helper or piece-certification changes on that source
  observation alone. Earlier all-A-bounded helpers cost 3.93–15.73x the traversal
  time on four-loop controls, I1b was dropped after negative results, finer pieces cost about
  1.57x, and the closed-descendant census found zero earlier witnesses. A future
  small falsifier must close an unchanged finite query independently of the
  open broad helper, with exact replay and the extra solve cost charged.
- [M] `epoch_s3_delivery` has written the first internal section writer and
  structural merge-boundary checks; `checkpoint_final_audit` is reviewing it.
  It uses fixed-size scratch and incremental hashing, preserves dispatch
  ordering and in-flight descriptors, and refuses poisoned state. These files
  have no runtime caller or resumable manifest yet. Native execution remains
  pending; publication/restore/save-before-join are still required.
- Root review flagged the bounded-memory reservation check's potentially
  superlinear time for large scattered retry queues. The author and auditor
  will assess it before this becomes a production save path. Bounded memory
  alone is not sufficient performance evidence. Restore must also preserve
  historical orthant slots separately from the live lookup index; no CP5
  full-edge-vector reconstruction should be silently reused.
- [M] At 03:04:31 the corrected G2′ compiler was live (PID 2325595, about
  804 s elapsed, 40 GiB RSS) with no reported errors. Compilation continues
  as the sole owned heavy job; no candidate timing or deployment claim yet.
  LC2 remains untouched, and pending-growth rendering/calculation is unchanged.

### 2026-09-29 02:56 UTC — epoch source correction reviewed; S3 writer started

- [M] Epoch's two failed typecheck seams are corrected locally in
  `d5b05629`, following baseline `25f41db5`. Unsupported `ApplyG2` statistics
  now produce an engine-fatal protocol error with `panic=false`, not a caught
  panic that would become retryable C3. The regression covers error encoding,
  decoding and classification, including an earlier retryable error. The
  role fixture names its exact required/auxiliary IDs; geometry and prior
  assertions are unchanged. Independent source audit passed; no new compile
  or execution result yet. The original failed check is retained.
- The auditor additionally caught private-method access in the new regression
  before another compilation. Author corrected it using the existing persisted
  error-tag convention. Do not equate this source review with a passing native
  test or typecheck of the corrected code.
- `epoch_s3_delivery` is implementing the first internal bounded checkpoint
  writer and borrowed merge-boundary validation, with separate independent
  audit. It will stream sections rather than clone/serialize a whole state.
  Sequence/session identity, exact queued/reserved/in-flight accounting and
  poison checks are explicit obligations. This first internal slice does not
  advertise a public resumable checkpoint; atomic publication, restore and
  save-before-join wiring remain subsequent required slices.
- [M] Root revalidated the priority corrected G2′ release compiler as a live
  process at 02:56:28 (PID 2325595, parent 2320260), with no reported errors.
  It remains the only owned heavy job. Main docs are pushed at `9f13cccb`;
  no production intervention and no fresh performance pilot.

### 2026-09-29 02:51 UTC — corrected native retry running; other lanes isolated

- [M] Corrected G2′/rescue optimized focus is running at clean `d12db6cf`:
  `nice -n 5 nix develop --command cargo test --release --locked --offline
  -j8 -p rustred-app --lib g2 -- --test-threads=1 --nocapture`.
  Evidence: `TMP/codex-g2-rescue.tzdFuj/native-fixed-focus/`; protected
  CPUs 0–15, heavy/build-0 locks, local TMP, required license environment,
  250/150 GiB headroom guard. The full same-executable app suite follows
  only after the focus passes. No result yet; this is the only owned heavy job.
- [M] Isolated telemetry typecheck passed (69.180 s guarded, exit zero/no
  stop reason, about 1.37 GiB peak single-child RSS). Evidence:
  `TMP/codex-g2-rescue.tzdFuj/telemetry-typecheck/`. Its six reviewed files
  are committed locally as `187854b43eb87e82e84d116dd817298392697383`, atop
  baseline correction `faabf3e0`. Native tests and performance remain open;
  this change is not included in the G2′ candidate.
- [M] Epoch merge absorbed the identical baseline correction as `25f41db5`.
  First guarded typecheck failed in 83.179 s: one missing `ApplyG2` statistics
  match arm and one obsolete `helper_pattern` test field. Receipt:
  `TMP/codex-epoch-s3.JjASCU/typecheck/`. These are source integration failures,
  not campaign outcomes. Author will reject unsupported G2 results explicitly
  and update the fixture's exact roles without relaxing assertions; independent
  review is assigned. Its next build waits behind the corrected G2′ gates.
- [M] Read-only LC2 snapshot at 02:50:23 UTC: 33,560,009 discovered,
  11,138,252 native completions, 13,838,478 pending, conservative 6/67 initial
  obligations closed, zero frontiers, 25.8 GB tree RSS, 14.8 observed cores.
  Last-hour coordinator time remained dominated by commit (51.3%) and
  preparation (29.3%). No stop reason, about 699 GB host available. These
  metrics do not establish scoped closure or an ETA. Production is untouched.

### 2026-09-29 02:46 UTC — corrective source committed; native gate still open

- [M] Corrective integration commit
  `d12db6cfa23c09f7d9c2946416ea49763ece48f0` changes four explicit paths:
  activation-only checkpoint stamp invalidation, exact original/amended root
  phase validation, strengthened durable zero-work activation/ordinary-resume
  tests, and the sunset fixture's explicit roles. No geometry assertion was
  weakened. Independent source audit passed; root also inspected the actual
  checkpoint metadata/anchor schema and patch.
- [M] `cargo check --release --tests --locked --offline -j8 -p rustred-app`
  passed: 18.91 s Cargo / 22.161 s guarded command, exit zero/no stop reason,
  roughly 1 GiB peak single-child RSS. Receipt:
  `TMP/codex-g2-rescue.tzdFuj/typecheck-fixes/`. This is **not** a native
  regression pass. First failed optimized focus/full-suite evidence remains
  intact. No main engine merge, candidate CLI or benchmark yet.
- Before the next long optimized test build, root allocated short isolated
  telemetry and epoch typechecks, in that order, each with a private cache,
  current baseline correction and shared heavy-job protection. No timing
  comparison runs concurrently. They must not be mistaken for full native
  validation or silently included in the first G2′/rescue candidate.
- Main documentation milestone `2b4ade62` is pushed; subsequent event updates
  preserve the failure diagnosis and correction status for the next handoff.

### 2026-09-29 02:37 UTC — combined native gate found genuine integration defects

- [M] Frozen `7546c44c` optimized focus completed: **17 passed, three failed**,
  zero ignored, 7.44 s test time. Build-plus-test was 2,852.157 s (compilation
  47m21s), exit 101/no resource-stop reason; compilation is not solver timing.
  Evidence: `TMP/codex-g2-rescue.tzdFuj/focused-release/{stdout,stderr,result.json}`.
  All five G2 mutation refusals and the pin/quarantine seam regressions passed.
  The three new public amendment/activation pipeline tests did not pass.
- Independent diagnosis confirmed two real defects: an already-drained
  off-to-union activation changes checkpoint metadata/G2 state without a
  ChangeStamp change, allowing the forced save to be skipped; and native
  verification assumes Apply for original roots that initial admission
  legitimately places in Route when an owner is missing. The latter is a
  false rejection, not permission to accept arbitrary phase changes.
- Integrator will invalidate the saved-stamp shortcut only for a newly
  accepted G2 activation and independently derive expected original/amended
  phases from immutable input/installed owners, retaining exact geometry and
  containment checks. Original and amendment admission policies differ in
  the non-overcover case; the auditor is checking that distinction. Keep all
  failed receipts and strengthen durable no-work activation regressions.
- [M] Full same-executable app run completed before source edits:
  **853 passed, six failed, 12 ignored**, 136.67 s test time (137.198 s guarded
  command), exit 101/no stop reason, about 89 MiB peak single-child RSS.
  Evidence: `TMP/codex-g2-rescue.tzdFuj/full-app-first/`. Two additional rescue
  pipeline failures share the phase defect. The sixth failure is an old
  sunset fixture whose three named helpers lack an explicit role declaration;
  the new contract correctly treats all five queries as required. Add exact
  fixture roles while preserving its two-required/three-auxiliary assertions.
  Candidate CLI, telemetry typecheck and epoch typecheck are held until
  source-backed corrections are reconciled.
  No benchmark, deployment gate or production action has occurred.
- [M] Root independently ran full integration-tree `cargo fmt --all --
  --check`: PASS. This does not override the native failures.
- Epoch preparatory source merge is isolated at
  `f81559a649baf125515f5cdac8d1de587033c0cb`, branch `codex/epoch-s3-lc2`.
  Independent source audit passed after fixing offline verification's
  acceptance of unsupported G2 flags. Its 54 focused Python tests passed
  (11.243 s); no native test/typecheck yet. Evidence:
  `TMP/codex-epoch-s3.JjASCU/python-focused.log`. It must absorb the above
  baseline corrections before its consolidated typecheck. S3 itself is not
  implemented or advertised as resumable.

### 2026-09-29 02:26 UTC — skeleton validation passed; speculative shortcut parked

- [M] Exact optional full five-loop skeleton enumeration passed: one test,
  84.763 s (85.184 s guarded command), exit zero/no stop reason, about 21 MiB
  maximum single-child RSS. Receipt:
  `TMP/codex-g2-rescue.tzdFuj/skeleton-enumeration/{request,result}.json`.
  This removes the prior full-Python-suite skip. It is an input-generation
  consistency check, not a native closure run or speed comparison.
- [M] Closed-descendant query-witness census found no earlier witness among
  628 queries (including 314 finite full-jet queries) on the immutable H
  control: 9,033 nodes, 32,875 edges, final required publication 9,033 under
  either policy. Independent rerun reproduced every non-timing/RSS field and
  source hash. Analysis 0.372 s, repeat 0.384 s, about 61 MiB peak RSS. Evidence:
  `TMP/codex-closed-witness-census.5P0Cyu/result-r2.json` and `census.py`.
- The first census correctly refused a stale CP3 cached closure snapshot;
  one source-backed correction independently derived closure from seals and
  edges instead. The historical tracker rejects post-seal outgoing edges,
  so the publication-prefix calculation is exact for this no-rescue Ordered
  trace. This does not establish absence of opportunities after G2/rescue or
  on other workloads. Park the candidate; do not implement another traversal
  or new early-stop status without new evidence.
- Epoch S3 is split into source reconciliation, an internal streaming writer
  prerequisite, then restore/stop-path completion. Writer-only support will
  not be exposed as resumability. Independent review is assigned before
  building; no additional heavy/native job has started.

### 2026-09-29 02:22 UTC — measurement table audited; follow-on lanes bounded

- [M] Main documentation tip `c19716b8` is pushed. Integration tooling commit
  `56996edf` now reports whole-command wall time, waited-child user+system CPU,
  sampled aggregate tree RSS and the separately labelled single-child RSS
  maximum. Summed inspection time and sampled thread time remain diagnostics,
  not substitutes. Missing historical measurements remain unknown. Root's
  19 focused Python tests passed in 3.688 s; independent rerun passed in
  3.736 s. Receipt: `TMP/codex-g2-rescue.tzdFuj/reporting-table-audit.json`.
  Existing pending-growth calculation and rendering were not changed.
- [M] Independent source review passed the isolated lean-telemetry change,
  contingent on execution. Five regressions cover scalar equivalence, reused
  map storage, stale-field removal, resume counters and failure-ID
  decoding. No native execution or speedup claim; keep it out of the first
  combined G2′/rescue candidate. Its vendor is clean pinned `ef0db494`.
- [M] Combined optimized native compilation remains live without reported
  errors at approximately 34 minutes. This is compilation, not a solver
  pilot. It is the only owned heavy job; no candidate campaign-profile binary
  or new campaign performance measurement exists yet.
- [M] Read-only LC2 snapshot at 02:19:54 UTC: 29,429,189 discovered,
  9,615,916 native completions, 12,175,307 pending, conservative 6/67 initial
  obligations closed, zero frontiers, about 23.6 GB tree RSS. The last-hour
  coordinator shares were 53.2% commit and 27.0% preparation. These are not
  counts of closed required queries, nor an ETA. No production action.
- Registered a narrow read-only opportunity study: a recursively closed
  descendant might exactly contain a required query while its wider helper
  stays open. Existing input-root-only summary/verifier/audit would not use
  that witness. This is not yet observed opportunity and reporting alone
  would not shorten traversal; any future scoped stop requires separate
  authority/status, never a false all-domains-resolved claim. Candidate
  details and falsifiers are in the follow-on candidate note.
- Assigned `epoch_s3_delivery` to reconcile the existing epoch implementation
  and propose a small fresh-only checkpoint/stop slice before editing. Work
  remains isolated; no extra native builds or pilots are authorized while
  the first integration gates run. No epoch speed/deployment claim.

### 2026-09-29 02:07 UTC — pilot guard frozen; next narrow slice isolated

- [M] Tooling-only commit `d45f325c01126641994dd83793d49eea4adc1906`
  on the integration branch passed 16 focused tests (3.725 s), independently
  repeated by the auditor (3.753 s). Root inspected the receipt at
  `TMP/codex-g2-rescue.tzdFuj/runner-guard-tests-frozen.log`. Exact command:
  `env TMPDIR=/common/dev/rustred/TMP PYTHONDONTWRITEBYTECODE=1 taskset -c
  32-39 nice -n 5 /nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
  -B -m unittest discover -s tools/research/w1_g2prod -p 'test_*.py' -v`.
- The runner now owns locks/headroom/stop handling, excludes censored outcomes
  from comparison gates and reports waited-child CPU separately from sampled
  thread times. Audit also caught and fixed partial sampler-start cleanup
  and unreadable-process uncertainty; no detached mock children remain.
- [M] Direct cold-verifier guard/timeout mocks in
  `TMP/codex-verifier-guard.7rBJCw/`: normal exit zero/no reason in 1.168 s;
  signal-resistant timeout forced nonzero exit in 3.163 s, process group
  drained. Private mock lock and CPUs 32–39 only, no native or heavy job.
  Protocol now records separate safe launch forms for pilots and verifiers.
- [M] The one skipped Python test is full skeleton enumeration, enabled by
  `RUSTRED_SLOW_TESTS=1`, not an optional native executable test. Integrator
  corrected the earlier rescue note in docs-only `83d260ab`; the exact
  enumeration test is queued after the native suite. Existing PyO3 bindings
  do not expose this owner-domain walker; affected Python integration here
  is the CLI-steering/planner surface, not a claim of a rebuilt extension.
- Rotated the independent auditor into authoring the distinct narrow telemetry
  slice on `.claude/worktrees/codex-lean-telemetry`, branch
  `codex/lean-telemetry-lc2` from `7546c44c`. G2 integrator will independently
  review it. Five focused regressions are being formatted; no native test,
  integration or speed claim yet. The G2/rescue build remains source-frozen
  and live; no production action and no campaign performance pilots yet.

### 2026-09-29 01:52 UTC — measurement preparation and ownership fix

- [M] Audited integration log committed/pushed as `16868940` on `fable_5_1`.
  Frozen implementation `7546c44c` remains on the integration branch while
  optimized native tests compile. No campaign-profile candidate yet.
- Independent runner review found that `run_arm.py` starts its solver in a
  new process group. Wrapping it in the build guard would not stop that
  solver on a memory/signal stop and could release the lock prematurely.
  Integrator is adding runner-owned shared locks, headroom admission/stop,
  signal forwarding and whole-owned-group draining, with mock-process tests.
  This is tooling only; frozen Rust compilation is unaffected. Stop-censored
  runs must fail comparison gates even if their child returns zero.
- [M] All seven converted LC2 command templates and owner payloads exist;
  queries are roleless in both matched arms. The auditor validated a new
  explicit hot r1a12 template at
  `TMP/codex-integration/hotsub-r1a12-v6-command.json` (SHA-256
  `5c5c5241e06d9d60ef7bcad611923a366e6a9622e5ceb595a5bc8a6f0859279d`).
  Compared with its historical command, only executable, converted input
  base and output placeholders changed. R<=1/A<=12, unrestricted D,
  Ready/W12/H256, and query bytes remain identical. The historical runner's
  hot fallback still points to old inputs and must not be used.
- Use the current native verifier directly with the generated command,
  checkpoint and result: `--reinspect all --reference-levers off
  --require-closure`, then current Python audit with `--verify-report`.
  Do not use historical wrapper defaults or substitute request bindings.
  LC2-versus-candidate and candidate-off-versus-union are separate comparisons;
  summed record time is not the wall-time deployment gate.
- [M] Read-only LC2 snapshot at 01:48:42 UTC: 24,593,537 discovered,
  8,110,272 native completions, 9,632,029 pending, conservative 6/67 initial
  obligations closed, zero frontiers, approximately 20.8 GB tree RSS and
  13.8 observed cores. These are not counts of the 116 required queries or
  a closure forecast. Production and pending-growth semantics are unchanged.
- Next coordinator slice remains registered, not implemented: typed scalar
  capture before JSON serialization, preserving fresh checkpoint counters.
  Additional audit requires resume totals added once, detailed-to-lean key
  cleanup, and physical failure tickets decoded only once. Do not throttle
  snapshots or present the historical 2.5–5% coordinator sample share as a
  whole-walk gain.

### 2026-09-29 01:48 UTC — combined source frozen; optimized tests launched

- [M] G2′/rescue source frozen clean as `7546c44c04ce17c8ed74c1ddc9bea88582c1b6da`
  on `codex/g2-rescue-lc2-integration`, including rescue merge `007042f1`
  and main documentation merge `38047b02`. Not yet merged/pushed as main
  engine code; source audit is not a deployment gate.
- [M] Independent combined mathematical/code source audit passed. The auditor
  independently ran 42 focused Python tests, all passed in 5.876 s. Full
  combined Python suite: 277 tests, one skipped, passed in 21.654 s.
  Rust `cargo check --release --tests --locked --offline -j8 -p rustred-app`
  passed in 67.176 s. New Rust tests are type-checked, not yet executed.
- Preserve negative receipts: the first combined Python run had 13 failures
  from the old anonymous query fixture. Correcting only its setup to exact
  IDs and explicit roles fixed them without changing assertions; legacy-v1
  no-rescue upgrade remains covered. One new audit fixture incorrectly
  described a frontier-free native inspection as locally undischarged;
  correcting that fixture preserved the validation, not a relaxation.
- Durable Planned-pin edges are now fully validated and installed before
  rescue reverse taint, including zero-callback loans. Historical plans and
  cursors survive; new quarantined anchors are excluded. Shared activation
  validation authenticates actual off/on request hashes and preserves only
  the existing valid amendment-chain base. No arbitrary request migration.
- Evidence: `TMP/codex-g2-rescue.tzdFuj/INTEGRATION.md`, `typecheck/`,
  `python-suite.log`, `python-suite-fixed.log`. Integrator launched guarded
  `nice -n 5 nix develop --command cargo test --release --locked --offline
  -j8 -p rustred-app --lib g2 -- --test-threads=1 --nocapture`, CPUs 0–15,
  heavy/build-0 locks, in `focused-release/`. Full app suite follows using
  that executable. Compilation remains separate from performance claims.
- Auditor is checking exact control paths, format-6 hot-sector input and
  runner timing/resource ownership while this build proceeds. No campaign
  pilot or production intervention; LC2 remains owner-controlled.

### 2026-09-29 01:30 UTC — standalone G2′ native focus passed

- [M] Optimized standalone G2′ focus passed all 11 tests, with zero failures
  or ignored tests. All five injected mutations were actually applied and
  correctly rejected: shrunken residual cover, late anchor, inadmissible
  anchor, dropped dependency edge, and anchor cycle. Also passed worker-count
  identity and existing checkpoint activation tests. Root independently read
  `TMP/codex-g2-lc2.mi0ekz/focused-release/{stdout,result.json}`.
- Native test time was 3.36 s. The guarded build-plus-test command took
  2,827.03 s; compilation was 46m59s and is not solver timing. Exit code zero,
  no stop reason; minimum host headroom 636.5 GiB.
- [M] The same built binary then passed the full app suite: **833 passed,
  zero failed, 12 ignored**, 124.55 s test time (125.20 s guarded command),
  peak single-child RSS 85,580 KiB. Root independently read the full stdout
  and result receipt in `TMP/codex-g2-lc2.mi0ekz/full-app-release/` (exit zero,
  no stop reason). This avoids a second standalone compilation and clears
  the integrator to merge rescue and install the combined regressions.
- The regression lane prepared both seam/checkpoint tests and real public
  amendment pipeline tests. Drafts:
  `TMP/codex-integration/g2_rescue_tests.rs` and
  `TMP/codex-integration/g2_rescue_pipeline_tests.rs`. They remain unexecuted
  pending combined-source integration. Loan-specific fixtures are distinguished
  from pipeline fixtures that do not assert a loan actually occurred.
- A second integration issue was identified before merging: activating G2
  changes the request digest, but an existing amendment chain retains its
  original parent. Use one narrow shared base resolver, recomputing actual
  off/on request hashes and validating the existing activation receipt.
  Already-recorded chains may retain the validated old base; new chains after
  activation bind the current base. Reject tampering/changed scope; do not
  introduce a general migration layer or waive immutable role declarations.
- No engine deployment or campaign switch is approved by the standalone
  focused gate. Combined native checks, named controls, independent reinspection
  and matched performance evidence remain pending.

### 2026-09-29 — pinned-dependency interaction found; census negative

- Combined-test implementer found a durable zero-callback pin Q→A can exist
  before the graph edge Q→A is published. If a later frontier taints A, rescue
  must also quarantine Q and its already recorded ancestors before replay.
  Filtering the G2 index alone would miss those dependencies. Integrator
  proposes installing the durable pin edges before the existing rescue taint
  pass; per-open-source edge deduplication prevents double accounting at
  eventual publication. Independent critique and post-amend/pre-replay
  checkpoint tests are required before accepting this fix.
- Python audit also needs explicit local eligibility for each listed G2
  anchor now that rescued outputs may retain failed helper records. A valid
  historical anchor with a failed descendant is different from an anchor
  whose own inspection had a frontier/error or was abandoned; the latter
  must never authorize a loan. Geometry/stamps/edges remain mandatory.
- [M] Offline D-band census completed in 26.304 s, peak RSS 201.4 MB.
  It validated 1,215,537 records/domains, 12,314,557 edges and 48,412 birth
  merges. Within cohort windows 16/64/256 there were zero matching base
  signature pairs among 1,215,536 noninitial survivors and zero coalescences.
  Evidence: `TMP/codex-integration/coalescing-census.Gs3IcM/result-r2.json`.
  Historical G2-off survivor geometry only, not a post-G2 gain or benchmark.
- Preserve the initial failed-closed census receipt (`result.json`): one
  source-backed correction added the existing partial-initial-inspection
  record kind. The retry retained all birth/edge invariants. Park this narrow
  proposal; reopen only on actual post-G2 miss-cohort opportunity evidence.

### 2026-09-29 01:14 UTC — follow-on registration and combined test preparation

- [M] Audited log/protocol update committed and pushed as `a2bced18`.
- Registered follow-on mechanisms, falsifiers and negative evidence in
  `docs/research/codex_next_candidates_2026-09-29.md`; independent document
  audit passed. No proposed optimization has been presented as a speedup.
- Coordinator review rejected naive telemetry throttling: checkpoint
  callbacks require current counters. Proposed typed scalar capture and
  reused lean maps must preserve that contract. Retained the prior failed
  minimum-eight helper-grain experiment (median ratio 0.99885; preparation
  15.48% slower); no second pool/pipeline rewrite is authorized by that result.
- Research proposed exact coalescing of adjacent pending D intervals with
  every other predicate identical. Separate mathematical/code critique
  requires immutable admitted IDs, new union identity, ordinary exact aliases,
  exclusion of reserved/dead work and retained dependencies. This is not hull
  widening and does not itself establish useful opportunity.
- Legacy G2 publication receipts do not retain true miss batches. Do not
  manufacture an opportunity bound from their arbitrary record windows.
  An independently reviewed alternative reconstructs historical epoch C-5F
  **G2-off post-antichain survivor cohorts** from paired records/edge runs.
  Root authorized only this read-only census, <=540 s work / <=600 s overall,
  <2 GiB, CPUs 32–39, fresh local evidence. All birth/record/edge invariants
  must pass; otherwise the result is inconclusive. It is not post-G2 evidence
  or a solver timing comparison.
- Combined G2′/rescue dry merge found nine text conflicts; the integrator
  is resolving their design before modifying the actively compiling tree.
  Rescue implementer is preparing a separate combined-regression module.
  Startup activation and helper-rescue audit semantics need review even in
  files with clean textual merges.
- A private non-hardlinked campaign-profile cache seed was copied from the
  idle LC2 worktree to the G2′ worktree. Its copied executable remains the
  **old LC2 binary**, not a candidate; normal Cargo validation/rebuild is
  mandatory. No fingerprint overrides or compiler-profile changes.

### 2026-09-29 01:03 UTC — rescue committed; next opportunities registered

- [M] Rescue branch source committed as `3dac8aef450a379be4d7c6fda8428886f451d536`
  on `codex/rescue-lc2-integration`; native tests explicitly pending.
  Frontend receipt: `TMP/codex-integration/rescue-roles-frontend.json`.
  Integration note: `docs/research/rescue_roles_lc2_2026-09-29.md` on that branch.
  Root assigned G2′ implementer the combined merge in its existing worktree
  after the native app suite, retaining prior branch tips and build cache.
- [M] Read-only LC2 observation at 01:02:22 UTC: 17,311,629 discovered,
  5,334,966 native completions, 6,934,121 pending, 6/67 initial obligations
  recursively closed, zero frontiers, approximately 17.1 GB tree RSS.
  Hour-window coordinator commit share 51.9%, preparation 26.0%; recent
  observed CPU about 16 cores. Snapshot closure count is conservatively
  stale. These are observations, not a termination estimate or a switch gate.
- N2 candidate registration (independent read-only audit): remove redundant
  owned-vector copying in `applied/geometry.rs` using the existing checked
  `LatticeBox::try_from_preallocated` ownership path, then consider local
  scratch. Expected benefit is lower allocation cost, magnitude unknown.
  Do not rewrite `power_domain::project`: it already uses fixed arrays.
  Smallest falsifier: applied-geometry differential tests for correlated
  A/R/D, crossings, unbounded bounds, overflow/cancellation/resource prefixes;
  then exact event/counter identity and whole-command controls with G2 on
  in both arms. Park if allocations fall without useful campaign speedup.
- Negative evidence retained: previous duplicate-projection removal changed
  median traversal by only -1.46% and -0.059%, with overlapping ranges;
  historical 10.39% projection CPU is not an allocation-saving estimate.
  Source: `docs/research/finite_closure_native_profile_2026-09-23.md`.
  G2 can reduce the work N2 targets, so coordinator relief remains the next
  priority. Rescue implementer is preparing that slice read-only.
- [M] Independent build-lock cancellation regression also passed (0.141 s).

### 2026-09-29 01:00 UTC — rescue source audit passed

- [M] Independent audit passed the explicit-role source slice, conditional
  on native execution and the later combined G2′/rescue gates. It verified
  all 183 original row objects and their order are unchanged: 116 required
  (including 16 convenience queries), 67 auxiliary. Only role metadata was
  added. Duplicate-key ambiguity in Python JSON readers was found and fixed.
- [M] Rescue frontend matrix: 161 tests, 160 passed and one optional native
  checker test skipped. Independent reruns: 26 role/audit tests passed;
  earlier staging/planning/audit run: 80 passed and one expected skip.
  The offline checker independently found zero disagreements over 7,424
  membership probes. These are not native checkpoint or closure receipts.
- The rescue build was still waiting for the heavy-job lock. Root requested
  cancellation of that owned queued job and source packaging, so the merged
  engine can receive one consolidated native build. The ongoing G2′ native
  test build continues. Neither campaign execution nor rule generation has
  been started by these integration jobs.
- Next gate: both activation orders, a previously inspected anchor later
  blocked through a descendant, and pinned accepted-prefix replay. The
  auditor is checking the actual quarantine update boundary before we choose
  an implementation; avoid adding unnecessary live concurrency machinery.
- No campaign switch is recommended yet. New role metadata changes the
  immutable request binding; a fresh campaign is an acceptable deployment
  path if existing validated activation cannot preserve it cheaply.
- [M] Combined audit found existing-anchor quarantine changes only at
  amended resume, before workers; in-session dead marking applies to new
  IDs, not existing eligible anchors. Implement a full-ID eligibility view
  at setup/publication, retaining historical accepted pins and their edges;
  do not introduce an unnecessary live invalidation subsystem.
- Matched old/new timing controls will use identical historical roleless
  input bytes (all queries required in both arms). Role-aware rescue tests
  remain separate; the old binary does not understand the new role field.
- [M] Root corrected cancellation while waiting for local build locks:
  nonblocking lock polling exits without spawning a child. Owned regression
  passed in 0.138 s; independent helper audit requested. Rescue's queued
  build was cancelled without touching G2′ or production processes.

### 2026-09-29 — source slices ready; native checks pending

- G2′ source/tooling tip `755e6157` is committed on its isolated branch.
  Python examples: 261 passed, one skipped (18.502 s); seven tooling tests
  passed independently. Native release compilation is still running under
  the protected resource allocation. No final CLI/campaign build is started
  yet: consolidate the combined source first to avoid duplicate long builds.
- Exact native invocation in the G2′ worktree (guarded CPUs 0–15, heavy and
  build-0 locks): `nice -n 5 nix develop --command cargo test --release
  --locked --offline -j8 -p rustred-app --lib g2 -- --test-threads=1 --nocapture`.
  Full launch/environment boundaries and build output are recorded in
  `TMP/codex-g2-lc2.mi0ekz/focused-release/`; compilation is not solver timing.
- Rescue role changes now cover native parser/planner/verifier, Python
  staging/supervision/audit, the offline planner/checker, and the tracked
  183-query fixture. Independent role-code audit is active. Original query
  geometries are unchanged; no production input was edited. The offline
  checker reports 7,424 membership probes with zero disagreements.
- Temporary read-only epoch review delivered a concrete S3/S4 sequence:
  fresh-only merge-boundary CP6, then asynchronous submit/poll/cancel for
  durable stop handling, then immutable inspector-side lookup. No general
  CP5 importer is needed. Preserve B16 while isolating those mechanisms.
- Important future integration constraint: epoch currently rejects all
  duplicate domain images, while rescue legitimately creates a new eligible
  ID for an image whose old ID is quarantined. CP6 must retain amendment/
  quarantine provenance and validate eligible representatives, not blindly
  enforce global image uniqueness. Carry query roles and rescue-abandonment
  semantics before freezing that format. No epoch implementation started.

### 2026-09-29 — integration and measurement audit follow-up

- [M] Pilot protocol committed/pushed as `2ca6c2db`. No new solver pilot run.
- G2′ source merge onto LC2 is clean at `a15562e5` on
  `codex/g2-lc2-integration`. Its old worktree-only Symbolica patch was retained
  in submodule stash `ce9d8ac8d1bc38f9555f48d088003bed5e8f96b6`; current vendor
  is clean `ef0db494`. Main vendor and unrelated work were not changed.
- Rescue is implementing a complete exact-ID query-role partition in the
  query document, included in the existing request binding. The absence of
  roles means all queries required; rescue requires an explicit partition.
- Independent measurement audit caught two further tooling pitfalls before
  runs: hard-coded worker labels and launcher/sampler time being described as
  native-only time. The G2′ lane is correcting both. Protocol now explicitly
  uses matched launcher-inclusive wall time, excluding sampler shutdown.
- Auditor confirmed all CAS primitives needed by the prospective N1 witness
  exist in Symbolica and are already used by RustRed. Crucial semantic rule:
  a nonzero sample cannot authorize `Zero::No` (uniform nonzero); only a
  conservative conditional result is possible. Profile opportunity remains
  pending; no witness optimization has been implemented or claimed faster.
- [M] G2′ benchmark-tool corrections passed seven focused Python tests, also
  run independently by the auditor. Explicit v6 templates no longer invoke
  the legacy input override; missing verification and censored runs fail the
  completed-comparison gate. Native engine release compilation is ongoing.
- [M] A local shared build guard now preserves locks/monitoring until owned
  descendants drain, even if their launcher exits first. Independent source
  audit and an owned mock-process test passed (11.442 s). Evidence:
  `TMP/codex-integration/{guard_build.py,test_guard_build.py}`. Accept build
  receipts only with exit code zero and no stop reason.

### 2026-09-29 00:36 UTC — first milestone pushed; pilot protocol registered

- [M] Documentation bootstrap committed/pushed as `6dea3737` on
  `fable_5_1`; root goal remains active.
- Registered matched baseline, input/timing boundaries, two-pair decision
  rule and owned-process pilot ceiling in
  `docs/research/codex_lc2_integration_protocol_2026-09-29.md` before new runs.
- Source audit found a measurement-gate defect in the unmerged G2′ tooling:
  missing verifier reports and missing counts could pass `g2prod_gate.ok`.
  Independent auditor confirmed it; implementation lane is correcting it
  with regression tests. This is not evidence that historical results with
  complete verifier receipts were wrong.
- Root confirmed the next coordinator opportunity in current source:
  `set_parallel_lean(pool.snapshot_lean(), ...)` still builds JSON per commit.
  Existing runC profiles attribute 2.5–5% of coordinator samples to it;
  no new optimization or speed claim made. Prioritize G2′/rescue first.
- Independent protocol audit passed with two clarifications adopted before
  measurements: cold verification also has a separate 30-minute ceiling;
  for the alternate work/memory switch gate, >10% worse matched end-to-end
  wall time is material regression, with noisy comparisons inconclusive.

### 2026-09-29 00:30 UTC — execution bootstrap

- User approved the complete plan. Recorded it verbatim and added the
  authoritative directive to `GOAL.md` before implementation edits.
- Checked the working tree and lane worktrees; no unrelated changes touched.
- Initial implementation lanes start from existing G2 `80e80b5a`, rescue
  `7ec2d16f`, and main LC2 `2255bc06`. Epoch `5d166910` remains deferred until
  the near-ready work is integrated and measured.
- [M] Root tool-managed goal created at 2026-09-29 00:32 UTC, status active.
  Subagent threads do not share the root's goal tool state.
- Dispatched separate G2′ and rescue implementation agents on isolated
  `codex/*-lc2-integration` branches and an independent auditor. Builds use
  CPUs 0–15 / 16–31 respectively, capped at eight workers, with heavy/build
  locks. No production CPUs or campaign writes authorized.
- Independent bootstrap audit found no substantive scope conflict.
- Read-only LC2 update at 00:33 UTC: 13,148,077 discovered, 3,730,934 native
  completions, 5,469,700 pending, 6/67 closed initial obligations, zero
  frontiers, approximately 14.4 GB RSS. No completion estimate inferred.
- Next: commit/push this documentation milestone; reconcile source integration
  and test receipts before any performance pilot.
