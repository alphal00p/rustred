# Codex progress: five-loop optimization and controlled deployment

Authoritative current plan:
[ASTER_FINAL_PUSH_FOR_ALL_OPTIMIZATION.md](ASTER_FINAL_PUSH_FOR_ALL_OPTIMIZATION.md).
Earlier plans and measurements below remain historical evidence, not a launch
instruction. The user launched the frozen generation-first trial on September30;
root observes it read-only and does not control its lifecycle.
Root orchestrator owns this log; agents report evidence for integration here.
`[M]` denotes observed/measured evidence; `[E]` denotes interpretation or estimate.

**Latest user direction (2026-09-30 17:29UTC):** the mandatory1.5x deployment
gate is relaxed. A smaller reproducible useful advantage is acceptable, with
work/memory tradeoffs reported. Matched same-build Ready controls, repeated
measurements, cold coverage and four-loop non-regression remain necessary for
a recommendation. The newly demonstrated gain compares Epoch E0 against E1024,
not Ready against Epoch. Earlier1.5x statements below are historical, not the
current threshold. The active tool-managed objective already asks for measured
pilot gains without specifying that ratio, so its full scope remains unchanged.
The earlier four-loop wall-time comparison was 5.16% slower for Epoch; the
post-repair ABBA below is about10% slower in both pairs. Neither passes a strict
non-regression gate. The latest user-requested release is therefore
an explicitly qualified full-scale trial, not a declaration that every earlier
performance preference gate passed.

**Latest release priority (2026-09-30 17:48UTC):** deliver a clean frozen release
and exact full-campaign launch instructions next, before further optimization
experiments. The user will launch it in `codex_astra` and wants LC2 left running
if resources permit. New production remains user-launched only. Root requested
an asynchronous choice between fresh all67 A1 rule generation first (not yet
completed/timed) and immediate reuse of existing rules with new Epoch. The user
selected **regenerate improved rules first**. Primary delivery is now executable
preparation→all67 selected-owner generation→native admission→Epoch walk, with
no silent old-payload fallback. Build/tests/freeze are complete; see the final
release receipt below. On 2026-09-30 the user selected **32 physical cores and
600 GB RAM** for the actual new campaign. W16 measurements remain W16 evidence.

## Live-trial feedback and next structural work — 2026-09-30 19:40 UTC

### Production failure and validation update — 20:41 UTC

- [M] **Current-source native validation PASS,00:10UTC October1.** Build
  exit0/no guard stop in1129.507s; test guard exit0/no stop in120.179s. Focused
  union9/observer8/layout1 PASS; full app1,285 PASS/0 failed/12 pre-existing
  ignored, candidate CLI19 and routed CLI6 PASS. The focused cases are subsets,
  not additional unique passes. CPU0–49 comprised50 distinct physical cores;
  strict license requirement/profile0 and no worker-availability SKIPPED markers.
  Build PG4102483 and test PG/SID23246 are now absent. This validates the one-use
  P1 witness and optional observer, **not** the isolated compact-buffer draft or
  an optimized performance gain. Independent final receipt audit GO confirmed
  all10 source hashes, newly built Cargo artifact bindings, test counts and
  complete process drainage. Owner-side heavy/build/test locks are free.
  Consolidated receipt: `TMP/postlaunch-20260930/P1_P2_NATIVE_ACCEPTANCE.md`.
- [M/E] Layout diagnostic measured `(large row, payload, prospective tag)` bytes
  at N2 `(592,592,16)`, N10 `(1056,1056,16)`, N15 `(1360,1360,16)`, N32
  `(2352,2352,16)`. At N15,256 rows reserve348,160B currently, versus4,096B for
  prospective all-Stored tags or352,256B for all-Candidate tags+payloads.
  The target-less fraction8.83% in h0 supports landing/testing the smaller
  transport hypothesis after this milestone. It does not measure RSS, touched
  bytes, copying cost or wall gain. The draft's actual `Option<Verified>` layout
  still receives its own diagnostic/native tests; no universal saving claimed.
- [M/D] At00:08UTC October1 the consolidated correctness build finished
  successfully (`release` app-opt1,18m46s compiler receipt); owned PG4102483 is
  absent. Focused/full native tests remain pending with the runtime agent.
  The compact transport draft has independent **source GO only** after fixing
  a draft-only `Map::nth/last` payload-association bug with an owning iterator.
  Patch `d72fadc0…3348c` remains isolated, unapplied and uncompiled. Six draft
  tests include row/storage invariants, skip/last and actual Option-tag layout.
  No current-source or production behavior changed due to that draft.
- [D/E] At23:56UTC root granted `s5_fixed_work_pilots` an **isolated draft only**
  of compact P2 row transport in `TMP/postlaunch-20260930/compact-source-rows/`.
  `final_requirements_audit` independently reviews it. No tracked source edits,
  native run or landing until the current frozen-source tests/layout diagnostic
  complete. Keep the resolver and every row's authority check unchanged; compact
  tag/token rows plus separate Candidate payloads, reserving both buffers before
  resolution. `target=None` bounds payload capacity only, never proves anything.
  Empty/stored/candidate/mixed/error panels and candidate-heavy regression risk
  are required. This is implementation preparation, not a measured win.
- [E/D] Independent review keeps inspector-to-coordinator containment-token
  reuse **deferred**. A sound process-local witness is possible, but current
  bytes-only transport discards tokens and drops the snapshot lease. Existing
  tokens do not bind full image, arena identity, job/ordinal and snapshot epoch.
  A future sidecar would cross resolver, pool/escrow, cancellation and merge
  boundaries and retain current quarantine plus full cold/replay validation.
  Holding snapshots can change publication backpressure. The91% Stored share
  does not isolate repeated-summary/check cost, so it does not justify this
  larger change yet. First test the smaller buffer-layout opportunity.
- [M] The completed h0/h2 report and extracted arithmetic received independent
  final GO. Report:
  [epoch_preparation_screen_2026-09-30.md](docs/research/epoch_preparation_screen_2026-09-30.md).
  Root updated the S5 technical status and authoritative plan with this negative
  deployment result. No native work was needed for the report audit.
- [M/D] **Full-input h0/h2 screen complete,23:47UTC.** Both bounded arms
  saved clean CP6 checkpoints and fresh-process structural cold reads report
  INCOMPLETE with zero violations; all183 queries/bindings retained. h0/h2
  native windows1207.969/1209.858s, cold209.480/215.698s, inclusive1418.984/
 1427.234s; all owned groups drained. Two helpers delivered3.13% more native
  inspections (3,503,792→3,613,632),2.52% more committed domains,4.64% more
  pending domains,5.27% more sampled native-tree CPU and1.39% more peak RSS.
  Both structural checks found8/67 closed starting roots; neither independently
  rechecked native inspections or established required-query closure. P2 fell
 312.469→261.632s, mainly source221.497→178.353s, but extra P1/P3/boundary/wait
  offset most savings. **The10% useful-work gate is not met**, and this is one
  evolving-prefix pair, not fixed-work/repeated speed evidence. Keep production
  h0 unchanged. `final_requirements_audit` independently confirmed receipts and
  arithmetic; S5 owns the combined report. No additional helper sweep is granted.
- [D/M] Resource release allowed `runtime_order_pilots` to start the consolidated
  native correctness build (exec64721, owned PG4102483) onCPU0–15, using the
  cached target, heavy/build locks and child-only `RUSTRED_EPOCH_PROFILE=0`.
  All10 source hashes are checked; focused9 union+8 observer+1 layout tests
  precede the full app/CLI suite. This app-opt1 build is not a performance binary.
  Evidence: `TMP/postlaunch-20260930/p1-p2-native-build/`. Compilation is live,
  so neither the new source nor its tests are yet claimed passed.
- [M] At23:46UTC repaired production saved its first periodic CP6 generation1
  and kept running. Live receipt: resumable=true, paused=false, zero abandoned
  obligations,10,485,584 completed native inspections,16,210,614 committed
  domains,6,111,918 pending and488,344,468 committed events. Path:
  `campaigns/five-loop-a1-epoch-repaired-20260930/checkpoints/main`.
  This is an observed save receipt, **not** a performed restore/cold reinspection.
  Root and agents did not force the save or change production.
- [M/D] The h2 pilot's outer ownership-monitor adapter used549.39 CPU seconds
  by approximately1117s; this roughly half-core cost is **outside** the ARM
  receipt's supervised native-tree CPU summary. Both arms used identical polling
  code, but h0's exited adapter has no persisted CPU total. Do not manufacture an
  all-process CPU comparison or call the pilot host uncontended. The live harness
  remains unchanged to preserve the pair; future harness-cost work is separate.
- [M] Previous goal turn made concrete progress: added an independently reviewed
  layout-only diagnostic to the forthcoming native suite, reconciled the active
  decision register, and observed repaired production progressing to13/67 roots.
  Current h2 nativePID3976753 was revalidated live at23:40UTC; its remaining
  execution/cold checking is a verified wait, not a reason to relaunch it.
- [M/E] At23:37UTC repaired production remains healthy:8,833,376 native
  inspections,19,545,831 scheduled domains,5,505,879 pending and zero frontiers.
  Latest conservative closure snapshot reports13/67 starting roots,98.08s old;
  this neither establishes all116 required queries nor provides an ETA.
  Process-tree RSS23.89GB, recent utilization2.85/32 cores. A later283s interval
  has P2=42.81%, P1=19.79%, P3=16.32%, boundary9.87%, inspect/wait11.21%;
  coordinator costs are now important even when inspector waiting is small.
  No production setting, binary, checkpoint or lifecycle action changed.
- [D/E] Registered narrow structural hypothesis: compact P2 source-row
  transport. `Vec<MissResolution<N>>` reserves the inline Candidate-sized stride
  for every row, although h0's stored proposals comprise91.17% of178.475M rows.
  Candidate contains two native summary copies; Stored needs only a proof token.
  Independent source assessment (`final_requirements_audit`) confirms a possible
  split tag/payload buffer but **no measured allocation/copy/wall-time benefit**.
  Candidate-heavy blocks may regress. Root grants only a generic layout/capacity
  diagnostic in the already planned native test build, not the optimization.
  Preserve every resolver call, error order and ordered result fold if pursued.
  Waves already bound live buffers; cumulative reserved capacity is not RSS or
  bytes touched. Reopening condition: measured layout and representative block
  mix support useful savings, followed by a separate audited implementation and
  matched pilot. Existing deferred lazy-summary bypass remains deferred.
- [M/D] At23:23UTC the h0 receipt is
  `BOUNDED_CAUSAL_ARM_VALID_NOT_CLOSURE`:1418.984s inclusive, native1207.969s
  plus structural cold209.480s. Cold exit9/INCOMPLETE is expected for its
 2,769,811 pending domains; zero violations, all183 query/input bindings retained,
  no native reinspection or actual resume claimed. All owned processes drained.
  The paired h2 is now active (adapter3976441/native3976753), same binary and
  scope,29 inspectors+2 helpers+coordinator onCPU0–31. Its1200s cooperative stop
  and1800s inclusive ceiling remain in force. S5 will explicitly reactivate
  the build agent only after h2 and its cold reader fully drain; no compiler is
  queued between arms. Consolidated native plan:
  `TMP/postlaunch-20260930/P1_P2_NATIVE_VALIDATION_PLAN.json`.
- [M] Repaired production at2329.94s remains running:6,742,816 native inspections,
 4,532,423 pending, zero frontiers,20.69GB RSS. Latest computed starting-root
  closure snapshot is9/67,352.67s old, versus5 earlier; do not treat the stale
  conservative count as an instantaneous fraction of the116 required queries.
  This is encouraging progression, not a forecast or matched speed comparison.
  LC2 remains untouched: at23:12UTC it had101,160,366 local completions,
 66,039,198 pending, zero frontiers and a6/67 snapshot2827.69s old.
- [M/D] At23:19UTC both development slices are source-frozen and independently
  cross-audited: P1 one-use union evaluation (nine focused tests added) and
  optional P2 exact cross-entry observation (eight tests). Rustfmt/diff checks
  pass; **neither has been compiled or run**. No CAS, artifact/checkpoint schema,
  live binary or production setting changed. Runtime agent owns consolidated
  app-opt1 native build, focused then full application/CLI tests only **after
  both h0/h2 pilot arms drain**; it must not steal the build/heavy slot between
  arms. Existing cached target/guard are reused; CPU0–15 build and0–49 tests
  remain disjoint from production. Optimized timing needs a later explicit
  build, not these correctness timings. P1 audit evidence:
  `TMP/postlaunch-20260930/P1_UNION_WITNESS_INDEPENDENT_AUDIT.md`.
- [M] h0 fresh full-input native screen saved cleanly at1207.969s (planned
 1200s cooperative stop), zero abandoned obligations/frontiers;3,503,792 native
  inspections,9,110,510 scheduled,2,769,811 pending. Its independent structural
  cold reader is running; **h0 acceptance and h2 remain pending**. Optimized P2
  total312.469s: source resolution221.497, reverse47.518, canonical dedup20.115,
  antichain7.784, representative4.435, transfer4.495. Work includes178.475M
  obligations,7.114M source tasks/3.585M waves. P1=97.940s, P3=67.801s,
  boundary115.626s, inspect/wait541.877s. Snapshot88.118s and prefix wait499.138s
  are nested, not additional wall time. This is an incomplete-window baseline,
  not closure or an h2 benefit. Later production intervals shifted P2 up to39.43%
  and P1 between12.52–20.64%, so this first20min screen does not settle all
  later-cut behavior. Evidence: `TMP/postlaunch-20260930/full-a1-preparation-h0-h2/h0/`.
- [D] At23:04UTC root assigned `runtime_order_pilots` a narrowly scoped optional
  P2 duplicate-observation implementation, with `final_requirements_audit` as
  independent design/code reviewer. This reopens **measurement**, not caching:
  bounded sampled-cut counts, explicit censored coverage and stored/current/stale
  context separation; no changed plans, errors, proof tokens or publication.
  Prefer the existing `RUSTRED_EPOCH_PROFILE` seam and invocation-only output,
  with no map/timing construction when disabled. Defer intrusive fine timers if
  necessary; repeated-image counts alone must not be called a performance win.
  No new native run/build is granted until the active h0/h2 pair drains. Root
  continues read-only production monitoring; frozen binary remains unchanged.
- [M/E] A later live interval (native elapsed1203s) attributes P2=26.95%,
  inspection/wait47.08%, boundary8.44%, P1=11.10%, P3=6.43%, with4.08 sampled
  cores. P1 grew from4.08% in the first interval; do not assume stationary phase
  costs. Root found two apparent exact-cover calls in `p1_anchors` (record
  validation then token verification) and assigned the independent auditor a
  narrow read-only check of whether they share identical immutable inputs. This
  is a potential same-call common-computation removal, not permission to weaken
  replay or pass trusted booleans as proof. No optimization is implemented or
  measured yet; a complicated proof-API redesign would be deferred.
- [D] The source audit confirmed identical immutable G2 union inputs in the two
  P1 calls; InitialDBand's high-slice test is different and must remain. Root
  authorized the small private one-use witness handoff, with no cached boolean
  API, schema change or relaxed cold validation. `final_requirements_audit` is
  now **implementer for this P1 slice**, while `runtime_order_pilots` independently
  audits it; conversely the former audits the latter's optional observer. Neither
  approves its own code. Both slices await native validation after h0/h2 drains,
  and neither is claimed faster from the phase fraction alone.
- [M] **Post-launch optimized ABBA complete, 22:54UTC (00:54 October1 Zurich).**
  Runtime agent used the actual repaired frozen CLI `23d836d7…`, W16/CPU32–47,
  unchanged selected-A1 payloads. All four arms independently cold-reinspect
  every native record and pass all58 required queries/32 roots with zero
  violations/frontiers/uncovered obligations. Native+cold totals in ABBA order:
  Ready14.494s, Epoch15.890s, Epoch15.297s, Ready13.920s. Epoch is9.63%/9.89%
  slower in the two pairs; its19–24% lower native CPU/about33% fewer domains
  do not offset17–18% more native inspections or establish wall non-regression.
  All12 owned process groups drained and locks were released. Secondary Python
  INCOMPLETE for Epoch remains separate from native cold-All PASS. Short runs
  supplied no foreign-CPU samples; production occupied disjoint reservations.
  Evidence/commands: `TMP/postlaunch-20260930/optimized-repair-four-qualification/RESULTS.{md,json}`.
  Independent `final_requirements_audit` review passed receipt fidelity and
  correctness; confirmed wall non-regression NOT PASSED. Median primary totals
  are Ready14.207122s/Epoch15.593741s (+9.7600%). No further native tests or
  production action were requested by that review.
- [M/D] S5 live triage now has two distinct steady approximately5min intervals:
  P2=28.62%/25.87% of coordinator wall; sampled4.29/4.07cores, substantial
  pending work and zero frontiers. Latest sample has2.333M local inspections,
  6.531M scheduled domains and1.917M pending; this is neither closure nor an ETA.
  `s5_fixed_work_pilots` may execute the already preregistered single h0→h2
  screen after the completed ABBA: W32/CPU0–31, same optimized CLI/all183
  original inputs,31+0+1 versus29+2+1 worker split. Separate `BOUND_PLAN.json`
  preserves the original unbound protocol. Each arm has1200s cooperative native
  stop/1800s inclusive ceiling and independent structural cold checking; evolving
  graph prefixes mean causal-screen evidence, not fixed-work speedup. Production
  remains unchanged. The h0 arm started at22:58UTC, adapterPID3828063,
  nativePID3828196; its paired h2 is not yet run and no gain is claimed.
  Evidence: `TMP/postlaunch-20260930/full-a1-preparation-h0-h2/`.
- [D] Runtime agent next performs read-only feasibility assessment of measuring
  exact cross-entry repeated query images within P2 cuts. Existing aggregate
  dedup counts cannot establish this opportunity. No cache implementation, new
  Rust build, native probe or large checkpoint decode is authorized by this
  assessment; independent audit remains a separate lane.
- [M] Runtime agent completed that feasibility assessment without native work.
  Source resolves each miss before candidate dedup, and stored positives bypass
  that map. No current receipt exposes the exact cross-entry duplicate rate.
  The existing small fixture has8 misses/6 exact images/2 cross-entry repeats;
  its large variant608/606/2 demonstrates why fixture rates do not predict
  production. Candidate remains **deferred pending measurement**, not approved
  as a cache optimization. A future bounded cut-local observer must separate
  geometry construction from stored-target proof/current-view exact check/stale
  lookup costs; unchanged target/prefix/quarantine authority is mandatory.
  Falsifier: rare/cheap repeats or observation/cache overhead exceeds savings.
  Evidence: `TMP/postlaunch-20260930/optimized-repair-four-qualification/P2_CROSS_ENTRY_DUPLICATES_ASSESSMENT.md`.
- [M] **Repaired optimized build delivered and user campaign running at00:44
  October1 Europe/Zurich.** Build guard exited0 without stop in3,706.452s;
  maximum single-child RSS18,282,192KiB, minimum host headroom605.788GB.
  Existing build/publisher sessions76831/83664 completed cleanly. Atomic binary:
  `TMP/postlaunch-20260930/epoch-union-repair-optimized-bin/rustred`, SHA256
  `23d836d7b05fac6b007cc1adea84f04548d5bf0e0a94a28ca877f98942da8045`, source
  `d55cfb1c`, campaign opt3/fat-LTO/CGU1. Sibling `FROZEN_BUILD.json` explicitly
  records that further optimized post-build checks were deferred by the user.
  Root did not launch production: the user's waiting PID3564028 performed
  preparation and exec'd the production wrapper after publication.
- [M] New production is
  `campaigns/five-loop-a1-epoch-repaired-20260930/runs/20260930T224435.637055Z`,
  nativePID3741792, all183 original queries, W32/CPUs64–95,31 inspectors/h0,
  original A1 saved rules and no hard runtime deadline. Admission preview was
  approximately473.393GB hard/449.723GB soft under requested600GB/host reserve
  150GB; supervisor live admission remains authoritative. The original poisoned
  campaign and LC2 remain untouched. Startup map verification briefly observed
  31.09cores/~5.03GB; subsequent traversal snapshot had205,616 local inspections,
  877,433 scheduled domains,418,555 pending, zero frontiers and3.10cores/~7.17GB
  peak. Startup CPU is not sustained traversal scaling, and1/67 conservative
  closed roots is not full required-query closure.
- [D] With user production already running, regranted the prepared short
  optimized four-loop ABBA comparison to runtime agent on CPUs32–47, using
  existing heavy/pilot locks, unchanged inputs and cold-All. This is deferred
  validation, not a launch gate. S5 agent owns bounded raw-heartbeat phase
  samples over two approximately5min traversal intervals before deciding
  whether the separate h0/h2 experiment is justified. Heavy jobs serialize;
  production state, CPU reservation and parameters are not modified.
- [M] Original optimized A1 read-only phase diagnosis (00:31 October1,
  Europe/Zurich): before the fatal reuse check, P2 accounted for31.33% of
  cumulative coordinator wall, inspect/wait46.52%, P1=4.15%, P3=6.05% and
  boundary11.94%. Last distinct progress sample was395.374s and1,183,536
  native completions; the error came at403.143s. Thus substantial P2 work
  predates the repair. Its small fatal result did not retain six-subphase,
  task-wave, lookup or detailed waiting totals, so no source-specific cost
  attribution is justified for that run. The app-opt1 repaired pilot is not
  a matched timing control. Evidence:
  `TMP/postlaunch-20260930/ORIGINAL_A1_PHASE_FRACTIONS.md`; only bounded existing
  heartbeat data were read. No production mutation or native test occurred.
- [M] Read-only observation at00:13 October1 Europe/Zurich: LC2 remains running,
  225,659,399 scheduled domains,100,154,036 local completions,65,781,605 pending,
  zero frontiers. Its conservative6/67-root closure snapshot is stale by3,366s;
  do not interpret it as a fresh closure computation. The new user's launcher
  and automatic binary publisher remain alive, awaiting the same healthy
  optimized compilation; no new native campaign has begun yet.
- [M] S5 diagnostic mapping is recorded at
  `TMP/postlaunch-20260930/S5_LIVE_OBSERVABILITY.md`: raw bounded heartbeat events
  expose cumulative P1/P2/P3/inspect/boundary wall times, but normalized status
  does not. The six detailed P2 timers and exact fallback/lookup counters are
  final-summary-only. Reuse existing bounded readers; do not force production
  stops or add a new monitor to obtain those details. After actual new traversal,
  two approximately5min intervals will decide whether the pre-registered h0/h2
  causal screen remains justified; it is not a launch gate.
- [M] Independent source critique of cut-local repeated-query work found a
  narrower hypothesis, not a measured gain. The repair pilot had116,211,553
  stored-target rows of128,652,079 obligations (90.33%);12,440,526 target-less
  rows split824,583 current-view and11,615,943 stale/full lookups. Same-job exact
  duplicates are already removed. Existing totals cannot establish cross-entry
  repetition, so do not implement a broad lookup cache speculatively. A bounded
  geometry cache would still need every row's digest, target/range/quarantine
  and exact verification, unchanged error order and cut-local lifetime.
  Current-view exact-only and stale full lookups must never share an authority
  result blindly. First measure exact cross-entry reuse and overhead if live
  profiling continues to identify this path. No implementation or CAS was added.
- [M] At00:06 October1 Europe/Zurich (22:06UTC), user recovery launcher
  PID3564028 is alive and waiting. The new campaign directory is absent:
  traversal has not started. Existing Cargo3441062/rustc3441196 are active;
  compiler CPU time advances, no error is reported. The runtime agent lost
  its tool session to an authentication error, so root installed a small
  process-independent binary publisher, session83664, that waits for this
  same build's successful final receipt and atomically exposes the complete
  binary. It invokes no native test and no production command. This removes
  any dependency on another agent turn to unblock the user's waiting launcher.
- [D] User confirmed the repaired full run should inform later optimization.
  S5 agent is mapping existing read-only profile outputs to the previously
  measured P2/inspection bottlenecks, with no new monitoring framework or
  linked Rust edits. The h0/h2 screen and wider performance gates remain
  follow-up work, not release holds. No production settings are changed.
- [D] User override at22:02UTC: stop adding pre-launch verification and supply
  recovery commands now; further controls may follow the user's launch. Revoked
  the runtime agent's ABBA native grant before any arm started. Only the already
  running optimized compilation must finish successfully, followed by atomic
  publication of the frozen binary; no help/probe/pilot gate is added. Prepared
  a user-invoked launcher at
  `TMP/postlaunch-20260930/epoch-union-repair-delivery/launch.py`, using the
  already checked exact command template, saved generated inputs and a fresh
  campaign directory. It waits for that build only. Root has not invoked it
  and does not start or mutate production. Existing performance caveats remain.
- [M] At21:58UTC the independent requirement-delta audit found no new
  source-contract gap from the repair. Remaining release work is the optimized
  freeze, actual-binary controls and bound fresh-walk recovery commands. Wider
  performance qualification remains open: four-loop wall non-regression,
  repeated current five-loop comparison and optimized hot control;20+ useful
  cores remains a target, not a condition to invent as a repair-release blocker.
  Evidence: `TMP/postlaunch-20260930/requirement_delta_1dae6099.json`.
- [M] Corrected stale present-tense generation/test status in the S5 reference,
  measured-controls introduction, active plan and runbook. Independent audit
  confirmed the four completed parent guard times total3,391.427s, excluding
  inter-group orchestration/admission, and their outputs total67 owners,
  9,966 rules and939 finite residual cases. These residuals are not a claim of
  independent masters. All prior censored evidence remains labeled historical.
- [D] Runtime agent owns the next optimized release gate: after the existing
  build exits cleanly, drains and releases locks, freeze a new immutable CLI
  and run the prepared four-loop Ready/Epoch ABBA comparison on CPUs32–47.
  Exact same inputs, W16, full native cold-All and owned-process cleanup are
  mandatory. No regeneration or production changes; each arm has its own
  inclusive1,800s ceiling. Metadata plans are ready at
  `TMP/postlaunch-20260930/optimized-repair-four-qualification/`; execution
  remains conditional on the optimized build, currently compiling.
- [M] S5 agent completed the runtime-only h0/h2 adapter; runtime agent's
  independent audit and eight pure/mocked tests pass. It reuses the audited
  lifecycle, retains all183 queries and enforces the corrected cold-start
  cutoff at1,500s. The adapter remains unbound and native execution ungranted.
  Evidence: `TMP/postlaunch-20260930/full-a1-preparation-h0-h2/`.
  Equal-window results would be causal-screen evidence, not matched-prefix
  speedup or closure. This follow-up will not delay delivery of the repair.
- [M] **Full-input repair regression PASS, independently audited at21:44UTC.**
  Receipt: `TMP/postlaunch-20260930/production-union-repro-retry/REGRESSION_RESULT.json`
  SHA `f72841bbd1fc0ea47ad3e9a2af6d15a1356e5356ecf178f4ab40135e7e9df548`.
  Four undecided-cover fallbacks, clean paused CP6 generation1, zero abandoned
  obligations; all183 inputs retained. Structural cold read finished in193.330s,
  raw exit9/verdict INCOMPLETE, with zero violations and matching request/owner
  bindings. It geometrically covered all165,305 recorded G2 unions, with zero
  undecided checks and132,039 finite-point crosschecks without disagreement.
  It re-inspected **zero** native records (`--reinspect none`), so this is not
  full cold-All, actual resume execution, all-query completion or family closure.
  All owned processes drained; total1403.219s is within the1,800s pilot budget.
  Original production poison/session controls are unchanged. Remaining pending
  work is1,974,430 domains. The optimized build started automatically after this
  gate, guard child3441062/session76831, CPUs0–15; no optimized result yet.
- [M] Read-only LC2 observation at21:43UTC:224,674,461 discovered,
  99,729,005 local completions,65,519,746 pending,6/67 closed roots,
  94.31GB sampled tree RSS. Last-hour pending growth+0.8352/completion,
  computing-inspector mean0.667. Production still runs unchanged; no ETA follows.
- [M] Repair milestone committed/pushed as `d55cfb1c`. The first full-input
  TMP pilot stopped after70.259s with an **input ENOENT during preparation**,
  before any walking heartbeat or checkpoint. This does not test the original
  union-budget failure and is not a passing regression. All owned groups
  drained; raw `REGRESSION_RESULT.json` remains INCOMPLETE_OR_FAILED.
  Root and S5 are checking the absent checkpoint parent directory against
  native fresh-checkpoint setup; an independently reviewed harness-only retry
  is required. Evidence is retained under
  `TMP/postlaunch-20260930/production-union-repro/`. Production is unchanged.
- [M] Root confirmed the setup mismatch: Epoch fresh checkpoint creation uses
  `fs::create_dir(directory)` and requires its parent; the production launcher
  creates that parent at `production_saved_owner_campaign.py:1294`, whereas
  the isolated pilot directly invoked the lower-level supervisor without it.
  No new native engine defect is inferred from this failed pilot. The retry
  must create only its own checkpoint parent and preserve the first receipts.
- [D] At21:19UTC root granted the independently audited harness-only retry
  in `TMP/postlaunch-20260930/production-union-repro-retry/`. Its sole code
  difference is creation of that fresh checkpoint parent; all78 copied input
  hashes and all original solver/resource settings match. The original failed
  test remains intact. The same1,800s inclusive pilot ceiling and independent
  structural cold-check requirement apply; no production action is authorized.
- [D] Optimized CLI build queued at21:20UTC behind the heavy-job lock; it runs
  only if the retry reports exercised-undecided-fallback PASS with a clean
  checkpoint structural cold read and the four repair-source hashes still
  match. It cannot overlap the pilot. Exact command uses existing cached target,
  `cargo build --profile campaign --locked --offline --message-format=json
  -j8 -p rustred-app --bin rustred`, CPUs0–15/eight Cargo jobs, inherited license,
  heavy+build-0 locks and host RAM protection. Evidence will be
  `TMP/postlaunch-20260930/epoch-union-optimized-build/`. A queued build is not
  a completed or recommended production executable.
- [M] Independent profiling/configuration audit during the repaired pilot:
  `--epoch-preparation-workers 0` means **serial P2**, not automatic helper
  allocation. Selected W32 is31 inspectors+one coordinator. Parallel immutable
  source/bucket/reverse preparation exists but is not selected; startup route
  verification separately uses32 workers. At311.24s the coordinator reported
  P2=104.568s, inspect/wait=85.717s, boundary=25.071s, P3=21.927s and P1=8.234s;
  P2 is42.6% of summed traversal phase wall. Final-result subphase diagnostics
  are needed to identify the parallelizable share. Historical W16 four-loop
  h0/h2 medians27.693/28.875s did not establish a helper speedup; full-input
  W32 h0 is the frozen control, not proven optimal. No settings changed during
  the repair test. A matched helper test is a follow-up candidate, not a gain.
- [M] Frozen recovery Python checkout is prepared at
  `TMP/releases/20260930-epoch-union-repair`, detached clean commit `d55cfb1c`.
  Actual Nix/import smoke and parser-only command comparison passed. Template
  and receipt are `TMP/postlaunch-20260930/epoch-union-repair-delivery/`;
  all original query order/steering retained. The optimized executable is
  deliberately UNBOUND, and the proposed new production directory does not
  exist. This is not a launch recommendation or native recovery run.
- [M] Retry observation at21:28UTC: nativePID3244094 is live (elapsed531s),
  1,171,616 local completions,3,856,595 discovered domains,1,142,993 pending,
  four conservative closed roots, zero frontiers. This is close to the former
  failure's work counters, but neither elapsed time nor nearby counts proves
  that the same obligation or fallback was exercised. The final summary must
  provide that fallback evidence, followed by clean-save/cold-read checks.
  Do not compare app-opt1 wall time with the former optimized production build.
- [M] Follow-up audits completed during the verified live wait: the frozen
  recovery template/source identity independently passed (optimized CLI still
  unbound). The unreferenced `candidate_bundle/tests/case_probe.rs` also passed
  source-only audit at SHA `6d9fe859f781beed849d3e44460ecab5c985743b2a87713f8cd2f94ee95e1d9a`.
  It uses existing SectorSolver/backends, preserves saved case/order/policy,
  reconstructs the zero census and checks source replay/descent/guards;
  shared numerical-tail cases fail closed. It has **not** compiled or run.
  Same-process fixed-order timings cannot qualify a production speedup; use
  `backend:saved` to isolate source visitation from backend changes. Neither
  audit starts work or justifies a production launch.
- [D] Runtime agent registered one data-only h0/h2 follow-up at
  `TMP/postlaunch-20260930/full-a1-preparation-h0-h2/PLAN.json`. Same saved
  full-A1 inputs and32-core budget:31 inspectors+0 helpers+coordinator versus
  29+2+coordinator. Existing API has no exact stop-at-commit-prefix control;
  domain/event capacity caps are not that control. Therefore the proposed
  1,200s equal-window screen explicitly reports graph/work-mix differences,
  final P2 subphase costs, pending/memory pressure and clean cold structure;
  it cannot label more inspections an exact-work speedup or a closure result.
  Falsifier is no useful throughput gain or savings offset by serial work /
  fewer inspectors. No helper sweep, implementation or native run is authorized;
  repair delivery retains priority and the optimized executable is unbound.
- [M] At21:39UTC the full-input retry stopped cooperatively and saved CP6
  generation1, resumable with no warnings and zero abandoned obligations.
  Native result has no error, `workers_joined=true`, no certification-void
  flag, and **four actual union-cover-undecided fallbacks**, lender fallback0.
  This directly exercises the repair on the full saved production input.
  2,393,440 native inspections completed;4,730,878 domains committed and
  1,974,430 remain pending. Native guard1207.999s/inclusive1209.483s,
  all native/supervisor groups drained. The independent structural cold reader
  is still active; final regression PASS is not claimed before its receipt.
- [M] Independently checked final correctness-run P2 attribution:409.293s,
  35.98% of1137.666s traversal, not the earlier prefix's42.6%. Subphases:
  source274.495s, reverse74.266s, antichain20.624s, dedup18.966s,
  representatives10.012s, transfer5.267s. The5.664s difference is other /
  uninstrumented P2 work, not safely all overhead.149,590 cuts and128,652,079
  obligations were processed. Other phases: inspect/wait441.115s, P3=100.399s,
  boundary93.618s and P1=90.435s. This prioritizes the registered helper screen
  but does not demonstrate helper benefit or optimized performance.
- [M] Fixed-build four-loop gate passed at21:13UTC: all58 required queries,
  32 roots,26,025 domains and495,898 edges; independent cold-All reinspection
  checked all17,957 natives with zero errors, uncovered obligations, frontiers
  or violations.49 accepted G2 unions; both new fallback counters are zero
  in this control (focused native tests exercise them). Native8.999s plus
  cold9.1751s are correctness-only app-opt1 boundaries, not optimized timing.
  Secondary Python diagnostic remains explicitly INCOMPLETE, not a substitute
  PASS. All three owned process groups drained and resource locks released.
  Evidence: `TMP/aster-integration-20260930-resumed/union-undecided-regression/runs/fixed-r1/four-all/`.
  Root granted S5 the audited full-input W32/CPUs0–31 pilot next: cooperative
  stop at1,200s, total ceiling1,800s including cold structural verification.
  It retains all67 owners/116 required queries/67 helpers and the failed run's
  steering, using separate copied inputs/checkpoint. No production is changed.
- [M] Update at21:11UTC: the union-preflight repair native build completed
  successfully in1049.499s. Full application library suite: **1,267 passed,
  zero failed,12 ignored** in117.81s; candidate CLI:19 passed; routed CLI:6
  passed. The combined guarded test command exited0 and drained in119.180s.
  Evidence: `TMP/postlaunch-20260930/epoch-union-fallback-native-{build-retry,tests}/`.
  This is release-dependencies/app-opt1 correctness, not optimized performance.
  Root granted `runtime_order_pilots` the fixed four-loop native+cold-All gate
  on CPUs32–47. The full-input five-loop replay remains ungranted until that
  gate passes. `final_requirements_audit` independently checks source/binary/test
  bindings. The recovery metadata-only rehearsal may use ordinary inherited
  affinity because its launcher validates the future64–95 production mask;
  it starts no solver and does not modify the original campaign.
- [M] The metadata-only fresh-recovery rehearsal passed independently:
  all67 saved owners/8,246 routes/183 queries retained, query bytes and
  original steering identical, no native start, no runs/checkpoints created,
  and original poison/session/pipeline controls unchanged. Evidence:
  `TMP/postlaunch-20260930/production-union-repro/RECOVERY_{COMMAND,METADATA,SOURCE_CONTROL_HASHES}.json`.
  This is recovery preparation, not a production restart or closure claim.
- [M] Read-only LC2 observation at21:13UTC:223,011,011 discovered domains,
  99,082,185 local completions,64,814,177 pending,6/67 recursively closed roots,
  about94.93GB sampled tree RSS. Last-hour pending growth is+0.3159/completion;
  computing-inspector mean0.417. No production state changed; there is still
  no justified completion ETA.
- [M] Parallel preparation/dashboard milestone committed and pushed as
  `adc686e0`. The launch runbook now explicitly holds the affected frozen Epoch
  release. No production executable, pipeline file, input, or checkpoint changed.
- [M] The four-file union-preflight repair received independent source GO from
  `final_requirements_audit` after the bounded-recursion and malformed-scope
  regressions were added. Native build is running under CPUs0–15/eight Cargo
  jobs at `TMP/postlaunch-20260930/epoch-union-fallback-native-build-retry/`.
  Root's first command named a nonexistent integration test target and exited
  before compilation in3.137s; corrected to `cli_routed_campaign`. The pending
  build includes full app library plus candidate/routed CLI integration tests.
- [D] `s5_fixed_work_pilots` owns a full-input regression harness on copied
  immutable A1 inputs (all67 owners/116 required+67 auxiliary queries), W32 on
  CPUs0–31, with a cooperative pilot stop inside the30-minute total ceiling.
  No launch is granted before native tests pass. `final_requirements_audit`
  independently reviews scope, lifecycle and interpretation; runtime agent
  prepares four-loop regression commands and cost-counter interpretation.
  Original production stays untouched. Surviving the previous failure is not
  closure; a stopped run must cold-load its checkpoint before restart claims.
- [E] Runtime-agent cost audit identifies a specific follow-up, not included
  in the urgent repair: worker preflight adds one union evaluation; P1 already
  calls the same exact predicate twice (record validation and token creation).
  A private, state-bound proof object could let these two P1 consumers share one
  check while retaining independent cold/restore validation. Worker-to-publisher
  proof transport is a separate larger change and is deferred. Existing
  `planner_telemetry.plan_seconds` includes preflight but is summed call time,
  not campaign wall; `epoch.telemetry.phase_wall_seconds.p1` includes both P1
  checks, and `epoch.verify.union_covers` counts only token-producing calls.
  Measure these before claiming a bottleneck or speedup. No extra optimization
  entered the frozen repair source during compilation.
- [M] Follow-through at20:49UTC: independent review confirmed the planner's
  262,144-point proof budget and P1's65,536-region budget can disagree without
  indicating an actual uncovered point. Epoch-only preflight repair is source
  ready, under final audit; ordinary fallback occurs before any residual events.
  P1/restore/export remain strict. The auditor also required a conservative
  optimization-only lender bound before moving the recursive verifier onto
  inspector threads. No native validation of this repair has run yet.
- [M] All three preparation controls (parallel, serial, parallel) passed fresh
  native generation/admission/walk and full cold reinspection with identical
  16-owner counts and final graph totals. App-opt1 native+cold phase sums were
  29.291/30.283/29.290s; these are correctness receipts, not optimized performance
  qualification. Every owned group drained. Evidence:
  `TMP/aster-integration-20260930-resumed/order-pilot-plans/parallel-preparation-acceptance/ACCEPTANCE_RESULT.json`
  and `RESULTS.md`. The complete wall boundaries including orchestration were
  below the30-minute pilot ceiling.
- [M] Root visually inspected native144-column exact-phase and90-column compact
  dashboard replays from unchanged actual four-loop snapshots, including
  frame69 (91 rows/343 integral columns), counts, CPU colours and stale marking.
  PNGs/PTY captures are `TMP/postlaunch-20260930/dashboard-actual*`; source
  observations are `parallel-preparation-acceptance/active-captures-v2/`.
  These are explicitly replays, not images of a still-running solve. One initial
  TMP-only capture helper stopped on null telemetry; fixed and repeated using
  a fresh6-second generation-only run, all groups drained. No production changed.
- [M] The independent recovery audit verified all67 payload paths/sizes and
  their admission receipt binding, and all four generation manifest/report
  hashes. It did not repeat full payload hashing/native replay. The existing
  source campaign remains immutable; the future repaired walk must use a new
  directory and saved inputs, not swap its frozen executable in place.
- [M] The user reported the new A1 Epoch dashboard ending FAILED. Read-only
  receipts confirm all four generation groups and native owner admission
  completed successfully, admitting all67 generated owner bundles. Parent31740
  completed in2563.821s (guarded wall); its slow sector3822 produced800 rules and
  116 finite residual cases. Parent32745 completed in102.287s. Generated outputs
  and completed-sector receipts remain intact; generation need not be repeated.
- [M] The subsequent walk failed after403.143s with exit70:
  `P1: 2387377 anchors: UnionUndecided`, classified `internal-invariant`.
  It was not a memory, timeout, or user stop (sampled peak tree RSS9.925GB).
  The final observed heartbeat had3,885,226 discovered domains,1,183,536 local
  completions and4/67 closed initial roots; this is incomplete evidence, not
  successful scoped closure. CP6 is explicitly poisoned and has no resumable
  checkpoint generation. Do not remove that marker or suggest resuming it.
- [D] Root reassigned `runtime_order_pilots` to the exact-union/P1 fallback
  defect and `final_requirements_audit` to independent mathematical/control and
  recovery review. Source shows a bounded union-cover check returns undecided
  and P1 currently promotes it to fatal. The desired repair must retain exact
  authority: decline optional reuse and inspect the whole obligation when a
  safe proof cannot be obtained. Precise cause and implementation remain under
  review; no coverage claim from undecided results is acceptable. Production
  files/processes are unchanged; any new walk remains user-launched.
- [M] The new preparation telemetry/dashboard correctness build completed in
  1024.497s. Full native application suite:1,263 passed,0 failed,12 ignored,
  118.54s. CLI candidate integration:19 passed,0 failed,0.72s. Evidence:
  `TMP/postlaunch-20260930/preparation-ui-{native-build,native-tests,cli-tests}/`.
  These remain release-dependencies/app-opt1 correctness measurements.
- [M] `s5_fixed_work_pilots` completed the fresh parallel-parent four-loop
  control:16 owners,523 rules/28 residuals,58 required queries,32 roots,
  26,025 domains,17,957 inspections. Independent cold-All replay passed with
  no mismatches/frontiers/uncovered obligations. Serial matched correctness
  arm is active; a further short parallel capture is authorized solely to
  obtain real active dashboard snapshots. No optimized speed claim yet.
- [M] Root visually inspected native pseudo-terminal rendering at90 and120
  columns on explicitly synthetic input; alignment, colours, stale and invalid
  data handling passed. Actual parallel-run visual inspection remains pending.
  Unreferenced routing-envelope and saved-case probes are source-ready only;
  no unrun probe is being described as a successful experiment.

### Validation update — 20:10 UTC

- [M] A/B milestone committed and pushed as `2de21662` on
  `fable_5_1_parallel`. Frozen production still uses its earlier executable.
- [M] A/B native correctness build passed on retry in827.434s. The complete
  application library suite then passed **1,243 tests, zero failures, 12 ignored**
  in119.11s (guard wall120.181s). This includes the new exact parallel-route
  preparation and compact-image differential/mutation tests. The build used
  release dependencies with the application package at opt-level1 for validation;
  these are **not optimized campaign performance measurements**. Receipts:
  `TMP/postlaunch-20260930/route-compact-native-build-retry/` and
  `TMP/postlaunch-20260930/route-compact-native-tests/`.
- [M] Parallel parent supervisor:48 pure/mocked tests plus three real subprocess
  controls passed. Independent `s5_fixed_work_pilots` audit passed after correcting
  failed-versus-interrupted labels and moving large-output hashing until all
  owned native groups drain. This keeps the aggregate RAM guard responsive while
  siblings solve. All seven test process groups were independently found absent.
  Evidence: `TMP/aster-integration-20260930-resumed/parent-scheduler-process-smoke-v2/`
  and `structural-routing-envelope/PARENT_SCHEDULER_AUDIT.md` under that same base.
- [M] Separate native telemetry/dashboard is source-ready, implemented by
  `final_requirements_audit`; independent review now belongs to
  `s5_fixed_work_pilots`. It is **not in the A/B test binary** and has not yet
  passed native validation. Python rendering integration and fresh parallel
  four-loop generation remain pending. No production launch recommendation yet.
- [M] The new live campaign continues advancing through sector3822: at about
  1,899s in parent31740 it reached case672/frame588, after the earlier ten-minute
  exact frame. Ten of eleven parent sectors are generated, about1.8GiB RSS,
  zero reported failures. This remains a generation tail, not a closure or
  progress-fraction estimate. Production is unchanged.
- [D] Next structural priorities: a small exact-incidence shadow census before
  changing routed covers, and bounded alternative source-trace selection before
  expensive exact materialization. Neither is a claimed speedup. The existing
  Symbolica SourceWeights backend is available for an isolated large-frame
  comparison; prior completed comparisons lost, so it is not a default switch.
- [M] Native telemetry source independently approved after fixing a progress-path
  ancestor collision, a renderer name shadow, medium-width table clipping and
  misleading Ctrl-C wording for an attached viewer. Root started the consolidated
  library/CLI-candidate correctness build at20:15UTC, evidence
  `TMP/postlaunch-20260930/preparation-ui-native-build/`; result is pending.
  `s5_fixed_work_pilots` is implementing only an unreferenced bounded routing
  shadow test while that build runs. No production cover or solver change yet.

- [M] The user has started `campaigns/five-loop-a1-epoch-20260930` from the
  frozen checkout, with 32 physical CPUs64–95. LC2 still owns CPUs128–227.
  Development builds/pilots must exclude **both** reservations; the historical
  W50 pilot mask32–81 is no longer safe. No production file or process changed.
- [M] First native generation groups completed: parent30527, 17 selected
  sectors, 164.378s, sampled peak process-tree RSS1.187GB; parent30699, 18 sectors,
  560.941s, peak1.653GB. Both exit0 with owned groups drained. Parent31740 is
  now active (11 selected sectors); at62s it had10 generated and was discovering
  rules for3822. Parent32745's21 sectors remain queued. These are generation
  milestones, not scoped closure. Evidence is each attempt's `result.json`,
  `resources.jsonl` and `stderr` beneath the user's campaign directory.
- [M] Current RAM admission is about471GB despite a600GB request: the existing
  guard retains the requested150GB host reserve. This is not a newly imposed
  arbitrary limit. No changes to the user's resource policy were made.
- [D] User requests independent parent preparations in parallel, a clean
  dedicated dashboard, and support for more complex future preparation flows.
  Implement this for a **future build**, not by editing the frozen live pipeline.
  Use one aggregate CPU/RAM budget, disjoint worker slots, one owned-process
  supervisor and explicit dependency barriers before staging/admission/walking.
  Separate structured progress production from rendering; never parse human
  progress text as mathematical authority or mislabel generated sectors closed.
- [D] The user prioritizes ambitious structural gains. Investigate avoidable
  routed-domain expansion and downstream-work-aware recurrence selection, not
  merely increased busy-core counts. Existing joint-mask pruning's negative
  result is retained: many rejected local masks did not deliver a campaign win.
  `s5_fixed_work_pilots` owns this read-only proposal/evidence pass; a separate
  reviewer must challenge the mathematics before any implementation.
- [M] Narrow post-release slices A/B are source-ready and independently audited
  by `final_requirements_audit`: A parallel exact route preparation, preserving
  ordered errors/cancellation and replica budgets (`runtime_order_pilots`);
  B allocation-free compact decode/summary construction (`s5_fixed_work_pilots`).
  Native compilation/tests and performance validation are **pending**. No speed
  gain has been assigned to either. The next action is a consolidated guarded
  native test build; this does not modify either frozen production executable.
- [M] First build (446.295s) compiled the production application library but
  refused a new B unit test: one missing `[bool; N]` annotation produced19
  cascading type-inference diagnostics. Root added that annotation only;
  the library-test rebuild is active. Evidence:
  `TMP/postlaunch-20260930/route-compact-native-build{,-retry}/`.
- [M] Structural source/math review delivered
  [a conditional routing-envelope proposal](docs/research/routing_envelopes_and_rule_portfolios_2026-09-30.md).
  Verified inactive-numerator incidence can constrain every target numerator
  axis, but the current cover only consumes it for active-axis survival/pinches.
  Exact homogeneous maps can also preserve upper A-R, when proved—not merely
  assumed from vacuum kinematics. Independent review found the derivation sound;
  benefit is unmeasured. Next gate is a bounded shadow census on actual visited
  boxes after existing projection. Repeated fragmentation/subsumption could erase
  the gain, so a tighter local bound is not itself success.
- [D] The better-supported large-gain direction remains downstream-work-aware
  recurrence selection: A1 already demonstrated sensitivity, whereas the new
  envelope is only a cheap, falsifiable opportunity. The research lane is now
  locating minimal executable tests for both, without creating a new CAS or
  claiming this fixes the live sector3822 exact-elimination tail.
- Active future-preparation implementation: `runtime_order_pilots` owns the
  Python queue/resource supervisor; `final_requirements_audit` implements new
  unreferenced Rust telemetry/dashboard modules while the current build drains.
  That agent will not audit its own implementation. Default serial behavior,
  single aggregate RAM guard, owned process-group draining, immutable command
  binding and the all-parents staging barrier remain explicit test obligations.
- [M] Source-ready Python scheduler:45 mocked/pure tests passed, not yet a
  native parallel-generation acceptance result. Root review requested real
  short-lived subprocess tests for sibling drain, interruption and lingering
  children, plus invalid/nonfinite-telemetry refusal. These small lifecycle tests
  may use CPUs32–35; they do not launch algebra or touch either campaign.
- [M] Read-only live observations at19:48–19:54UTC: old LC2 remains at6/67
  roots,219.75M discovered,97.44M completions and64.15M pending. Its rolling-hour
  pending growth is now+0.032/completion, materially below the earlier+0.70,
  but still no convergence/closure inference. New preparation remains on3822:
  frame451 has4,014 source rows/8,482 integral columns and two active coefficient
  variables; its exact-elimination phase passed400s while rows still advanced.
  About1 observed core and1.8GB RSS. This identifies an algebraic tail, not a
  stalled process; active-sector work still cannot be checkpoint-resumed.

## Post-release follow-through — 2026-09-30 19:25 UTC

- [M] Previous goal turn made concrete progress: final optimized controls,
  frozen W32 launch preflight, independent release audit and push `d93e64ad`.
  The subsequent user question was answered: metadata-only preparation exits
  intentionally; `--resume --start` begins generation. Root did not launch it.
- [M] Read-only old-LC2 snapshot at 19:22 UTC: PID360092 alive with fresh
  heartbeat; 218.8M discovered domains, at least9.84M recursively closed,
  6/67 initial roots closed, 64.4M pending, 96.7M local completions. Last-hour
  rate approximately1.07M completions/hour and pending growth+0.70/completion.
  About9 busy cores and93GB RSS, zero frontiers; latest checkpoint generation14
  saved17:16:36UTC. Snapshot closure is conservative and scan-batched, not ETA.
- [M] User authorized further promising improvements after delivery, guided
  later by the full run. Independent `final_requirements_audit` confirms the
  implementation and launch package are delivered but the original performance
  acceptance is incomplete. The plan now records that distinction explicitly.
- Initially diagnostic lanes (subsequently granted the narrow slices above):
  `s5_fixed_work_pilots` studies measured P1/P2 costs and serial task allocation;
  `runtime_order_pilots` traces repeated owner/route preparation; root coordinates
  source review and future acceptance. Frozen source/executables remain unchanged.
- [M] Initial preparation attribution: existing heartbeat records place about
  70 of82 preparation seconds in serial verification/compilation of8,179 routing
  maps, not owner loading. Detailed source/resource recommendations are pending.
  This matters to startup and cold checks; it is not automatically a substantial
  speedup for a many-hour traversal. No new CAS primitive is proposed.
- [E] Root source hypothesis under review: the binary job decoder creates two
  coordinate vectors for each compact image, and P2 reconstructs expanded
  geometry for summaries. Eliminating these allocations through shared checked
  slice/array constructors may reduce coordinator overhead without changing
  work selection. The measured P1/P2 totals are not an allocation-only profile;
  benefit remains unproven and needs differential tests and matched controls.

## Frozen generation-first release — 2026-09-30 19:20 UTC

- [M] Implementation `b21e4522` is committed/pushed on `fable_5_1_parallel`.
  Frozen source: `TMP/releases/20260930-a1-epoch`; optimized CLI SHA256
  `38e0e7637a228aeccb13902915378d70f25736323916976819d2145ea19b1d89`;
  matching inspector SHA256
  `925a778b3393016bedee38f85e9db31b7a82a6aadb8e3c568c93ed740e1b4187`.
  Exact preparation/start/resume/rollback commands are in
  [the release runbook](docs/five_loop_optimized_generation_runbook.md).
- [M] Final optimized Ready/Epoch W16 controls all pass native cold-All.
  Combined4L two-pair medians: 15.494/16.293s native+cold, Epoch5.16% slower
  wall but34.14% less native CPU and33.04% fewer domains. Finite5L one pair:
  319.826/311.466s, Epoch2.61% lower total, despite native174.275s versus
  Ready168.634s. Epoch's cold phase137.191s versus151.192s supplies the total
  difference. This is not a proven traversal win or a repeated5L speed gate.
- [M] Root independently read both finite raw cold reports: Ready760,609 and
  Epoch754,320 inspections, all reinspected, zero errors/frontiers/uncovered,
  one required query/root independently verified. Same1,324-point initial query
  and historical67-owner input; not full116-query production or regenerated A1.
  Native CPU545.133/482.216s. The separate Ready Python diagnostic timed out;
  that censored secondary is neither PASS nor charged as completed comparison
  work. All six owned finite-pilot groups drained; no further pilots granted.
- [M] Fresh public Python4L pipeline passed with16 new owners,523 rules,
  58 required queries and17,957 inspections cold-verified. Existing installed
  wheel/newCLI23 checks pass; no newly built wheel claimed. Focused source
  suites35 runtime/staging +35 production/escrow +27 supervisor tests and
  seven inspector tests pass. Independent operational audit by
  `runtime_order_pilots` and pipeline/checker review by
  `s5_fixed_work_pilots` found no remaining release blocker.
- [M] Final W32/600GB metadata-only preflight succeeded through the actual
  frozen checkout's Nix environment. Exact command: the runbook preparation
  command with `--directory
  /common/dev/rustred/TMP/aster-integration-20260930-resumed/frozen-five-launch-check-w32`.
  All67 owners/8,246 routes/116 required/67 auxiliary remain unchanged;
  no native process, generation attempt or production write was made.
  CPUs64–95 are32 distinct physical cores, disjoint from LC2's128–227.
  A150GB host reserve remains explicit;600GB admission requires750GB available.
- [E] Recommendation: a **fresh A1-generation + Epoch trial**, not an assertion
  that Epoch universally beats Ready. The stronger repeated benefit remains
  A1 rule selection. All67 generation time and full-scope closure time remain
  unknown. No32-core scaling measurement or20+-core activity claim is made.
  New production will be launched by the user only; LC2 is still untouched.
- [E] Deferred until full-run feedback: hot-control and wider-worker pilots,
  P1/P2/owner-preparation optimization, and any further helper/lookahead tuning.
  No new feature or speculative experiment delays this handoff. Exact evidence:
  `TMP/aster-integration-20260930-resumed/order-pilot-plans/optimized-qualification/`
  (`FOUR_ALL_RESULTS`, `FIVE_FINITE_RESULTS`, raw runs) and
  `order-pilot-plans/pipeline-acceptance/`.

## Final optimization push — 2026-09-30 05:49 UTC

The previous audited milestone is committed and pushed as `297be07f` on
`fable_5_1_parallel`. The user has **not** chosen to launch the Ready fallback;
LC2 remains live and untouched. A new active tool-managed goal replaces the
delivery-only objective and requires both tracks below. No production actions.
The latest follow-up removes the delivery deadline and prioritizes clean durable
implementations, runtime-driven experiments and dependency-aware build reuse.
Framework builds remain necessary; selecting existing strategies must not
rebuild the engine. Bounded pilots and resource isolation still apply.

| Track | Owner | State | Next executable step |
|---|---|---|---|
| O: generated-rule selection and full persisted integral order | implementation `order_integration_resume`; follow-through `runtime_order_pilots` | delivered; all67 improved owners generated and admitted; repaired user walk active | Keep saved rules unchanged; native validation of narrow P1/P2 development slices after h0/h2 drain |
| S: original S5 merge architecture | implementation `s5_typed_resume`/root; follow-through `s5_fixed_work_pilots` | delivered; repaired optimized four-loop cold-All PASS, about10% slower than Ready; paired full-input helper screen active | Finish h2, structural cold checking and independent interpretation; no saturation promise |
| P4 shared immutable lookup publication | root | native shared-layer differential, quarantine, compaction and cancellation tests PASS; independent source review complete | Measure whole-campaign impact, not just isolated lookup costs |
| Independent mathematical/code/performance audit | `final_requirements_audit`; prior native/P4/lane reviewers | repaired ABBA receipt audit PASS; P1/P2 authors cross-audited source, native validation pending | Audit completed h2/h0 comparison; compact-row diagnostic is measurement only |
| Integration, resources, profiling and release | root | pushed513befd6; frozen repair source d55cfb1c, optimized CLI23d836d7; both production campaigns observed read-only | Consolidate tests/evidence without changing user production |

### Active decision register — refreshed 2026-09-30 23:40 UTC

This compact register takes precedence over stale provisional next steps in the
chronological history. It records negative results explicitly to avoid reopening
the same experiment after a handoff.

| Candidate | State | Evidence / reopening condition |
|---|---|---|
| A1 structural source visitation | delivered, full-scope trial active | Two four-loop pairs:40–44% fewer domains; limited5L transfers positive. Full67 improved owners generated:3391.427s native generation sum plus12.161s admission;116-query closure remains unachieved. |
| Persisted programmable integral order | delivered | Native/interface authority tests pass; B1 full comparator is not faster overall than A1. Degree-row ablation cold-PASS but no useful gain; parked. |
| B2 support-density order | deferred | Diagnosed coupled-affine classification obstruction; missing IBPs not demonstrated. Reopen only with a separately justified exact geometry change. |
| Selected-sector public generation | delivered | Native/Python/stager tests and selected16-owner/508-route/58-query cold-All integration PASS; full67 improved owners generated and reused for repaired walk. |
| S5 typed merge/shared lookup/rolling prefix | delivered, trial active | Repaired optimized4L ABBA cold-All PASS, Epoch+9.63%/+9.89%wall versus Ready. Earlier finite5L-2.61%wall is single-pair evidence; useful20-core scaling not demonstrated. |
| Prefix wait / dispatch-credit diagnosis | delivered | Off/on cold-All and strict mathematical-state comparison PASS. Measured95.841/97.712s credit-blocked; bounded slow-job correlation has explicit limits. |
| Bounded whole-result lookahead | delivered; optimized controls PASS | Native1229PASS/0failed/12intentionalignored; interfaces9/9PASS. Earlier app-opt1 E0/E1024 mechanism gain retained separately. Current optimized Ready comparison supports CPU/work tradeoff, not a universal wall win. |
| A1 selected-owner programs with Epoch | delivered | Same16owner/508route/58query input passed CP6 cold-All;16,932inspections/25,945domains. Correctness integration, not qualified speed superiority. |
| cut1 cross-job-coalescing remedy | rejected | September29 cold-PASS control barely changed Route count and slowed down; September30 proposed repeat canceled before launch. |
| oldest-ready / blind wider-window / adaptive-dispatch sweep | rejected | Preserved work explosion or negative timing; require a new causal mechanism before reopening. |
| CP6 partial-source publication | deferred | Premature proposal withdrawn. Need measured early usable emission and a reviewed authority/restore model before any implementation. |
| Ready bulk-edge-only optimization | deferred | Entire ordered commit is <8% of finite-control primary time; no profile of edge-only share and not a cure for80s Epoch waiting. |
| Hot traversal-heavy saved-rule control | deferred until full-run feedback | Prepared/audited, no native grant; latest user prioritizes frozen release over more pilots. |
| Production deployment | user-launched, read-only observation | Repaired fresh walk on saved all67 improved owners is active, W32/CPU64–95; latest snapshot13/67 roots, not required-query completion or ETA. Original failed checkpoint is not reusable. |
| Parallel exact route preparation | delivered, performance pending | Pushed2de21662; W1/W2/W4 exact-transport, cancellation/ordered-error tests and full1243-test application suite PASS. Startup/cold cost opportunity, not a demonstrated traversal gain. |
| Allocation-free compact Epoch images | delivered, performance pending | Same2de21662 and native suite; wire/mutation/summary equivalence tests PASS. No timing gain attributed yet. |
| Parallel parent preparation + separate dashboard | delivered, optimized timing pending | Pure/lifecycle/native source audits PASS; three4L generation→admission→walk→cold-All controls PASS with identical counts. App-opt1 phase sums29.291/30.283/29.290s are correctness evidence, not optimized speed qualification; actual-snapshot dashboard visually reviewed. |
| Verified inactive-incidence envelopes / homogeneous upper-D | deferred at unreferenced probe | Source-only shadow is not compiled or run; urgent failure repair and measured coordinator costs took priority. Reopen with bounded post-projection tightening census, then end-to-end gain. |
| Bounded alternative modular source-trace portfolio | deferred at unreferenced probe | No solver edits or native measurements; reopening needs exactly replayable case and full-cost comparison. Smaller traces alone do not prove better rules or less branching. |
| Source-weight reconstruction for the observed exact tail | deferred pending exact-case capture | Existing implementation, not a new CAS project. Earlier completed comparisons lost; new two-variable large frame is a distinct hypothesis. Need a precisely replayable completed-case/frame and matched full-cost comparison. |
| P1 repeated exact-union evaluation | delivered source/native; performance pending | Private one-use local witness removes duplicate same-input evaluation;9 focused tests and full1,285-app/25-CLI suite PASS. No relaxed cold/replay check or measured timing benefit. |
| P2 preparation helpers | delivered negative deployment screen | h0/h2 both cold-structural valid; h2 gives3.13% more native work,4.64% more pending and5.27% more native CPU. P2 saves16.27%, but10% useful-work gate fails. No switch or sweep; reopening requires materially different measured workload/cost evidence. |
| Cross-entry exact-image repetition | delivered observation; production measurement pending | Bounded optional census source-audited,8 focused and full native suite PASS. No cache/reuse authority; prefix truncation can hide repeats. Not enabled in user production. |
| Compact P2 source-row transport | active isolated draft, landing pending evidence | S5 authors TMP-only patch; separate auditor reviews. Root source frozen for current build. Layout diagnostic precedes landing; no solver change or measured memory/speed gain yet. Candidate-heavy blocks are the falsifier. |
| Inspector containment-token transport | deferred after feasibility audit | Possible with stronger private process-local bindings, but crosses bytes-only pool/escrow and snapshot lifecycle. No lazy-summary bypass; reopen only after the Stored validation cost is isolated and a full boundary/cancellation design is justified. |

**Earlier engine implementation milestone:** `56176df5`, committed and pushed to
`origin/fable_5_1_parallel` on2026-09-30 after native/interface tests, independent
source audit, and completed four-/finite-five cold-verified lookahead screens.
It adds opt-in bounded whole-result lookahead to the earlier9cc1acca selected-
sector/profiling and711b18c5 ordering/S5 milestones. All39 task-owned changed
files were staged explicitly; unrelated user work remains outside the commit.
At that earlier milestone, optimized matched qualification and campaign setup
were still open. Their final outcomes are recorded in the release receipt above.

### Delivery-gap review — 2026-09-30 17:49 UTC

- [M] Independent reviewer `release_correctness_audit` checked both tracks
  against the whole active goal. Implementation/native authority gates are
  substantially complete; qualification and operational handoff are not.
  Remaining gates are the optimized freeze, matched4L/hot Ready controls,
  newly installed Python tests plus actual CLI-steered smoke, justified wider-
  worker evidence, and full-scope future generation/admission/walk instructions.
- [M] Root corrected stable-documentation drift: scalar checkpoint metadata5,
  candidate-generation checkpoint4, current1229 application tests, opt-in
  whole-result lookahead lifecycle and the user's threshold waiver. Independent
  review passed. Historical measurements are distinguished from current
  qualification rather than deleted. No Rust/Cargo source changed.
- [M] At17:44UTC optimized Cargo group329576/rustc330248 remained active,
  error-free, about16minutes elapsed,98.5% compiler CPU and13.2GiB RSS. Build
  evidence remains `escrow-optimized-build/`; source implementation56176df5.
  It is left running; no competing heavy job or production action was started.
- [E] Delivery does not require executing the116-query production campaign
  or completing all67 new A1 payloads here. It does require honest executable
  preparation→generation→admission→walk instructions retaining every request.
  Existing prepared JSON is not generated output. Standalone3822 affordability
  and metadata-only generation resume remain explicit risks. Agent
  `runtime_order_pilots` owns the future runbook; root owns stable docs and log.
- [E] A small read-only S5 profile synthesis is delegated while compiling to
  prioritize any subsequent input-only experiment. It grants no new native
  run and does not replace the ready-to-execute matched qualification plans.

### Frozen-launch milestone preparation — 2026-09-30 17:49 UTC

- [M] Audited documentation/progress milestone `869db79c` is committed and
  pushed to `origin/fable_5_1_parallel`. Optimized compilation remains active;
  `s5_fixed_work_pilots` now owns completion watch and no-clobber executable
  freezing only, with no permission for a new native experiment.
- [M] Read-only resource check: LC2 PID360092 still reserves CPUs128–227,
  uses approximately84.5GiB RSS and remains untouched. Host MemAvailable was
  approximately564GiB. Proposed new-run CPUs64–79 are16 distinct physical
  cores on a separate NUMA node from LC2, with a provisional400GB requested
  own-memory ceiling and explicit host headroom; final launch rechecks memory.
  This is a conservative starting reservation, not a scaling/throughput claim.
- [M] Existing `production_saved_owner_campaign.py` can clone/freeze immutable
  inputs and executable into a new campaign, preserve role-explicit queries,
  forward Epoch controls, supervise RAM/checkpoints/dashboard, and resume.
  Agent `runtime_order_pilots` owns exact commands/tests; independent reviewer
  checks resume/scope/resource pitfalls. Existing-rule reuse must be labeled
  honestly as unchanged source rules, not the A1 generation improvement.
- [E] Fresh A1 preparation still needs generation of every selected owner,
  and its native generation checkpoints preserve completed sectors rather
  than in-sector elimination state. The small inspector's fixed aggregate
  allowance may also reject the complete new output. No rushed new generator
  supervisor or untested loader-limit bypass is part of the immediate release.
- [E] Deferred until after launch qualification: one optional helper-count
  falsifier if optimized P2 cost justifies it. Remaining prefix waiting is not
  automatically evidence for more lookahead; the previous profile-off receipt
  establishes a peak window occupancy, not time spent blocked on that limit.

User selection follow-through (17:53UTC): `runtime_order_pilots` owns narrow
generic Python preparation/pipeline modules and mock tests under
`tools/research/runtime_order/`, the input-only five-loop recipe and launch
runbook. It will reuse existing native generation, stage and supervisor
primitives, not implement another solver. The independent auditor is reviewing
generation RAM/cancel/restart and complete payload admission. A concrete release
defect is already identified: the study inspector caps aggregate payload at1GiB,
below the historical complete67-owner size (~1.28GiB). A narrow existing-budget
interface fix is necessary; no cap bypass or incomplete-owner admission is
acceptable. Root owns that example-only change after the current frozen build,
so the running compiler's source snapshot stays unchanged. The live production
campaign and reference payloads remain read-only.

Release packaging checks (18:06UTC):

- [M] Root exercised the new tracked preparer against the real frozen5L
  selection in metadata-only mode (`launch-data-smoke/`). All67 owners,
  8,246 exact route objects,116 required+67auxiliary queries, owner/query order
  and original selection hash are preserved. Parent job counts are17/18/11/21.
  No native invocation occurred; every new payload remains explicitly
  UNGENERATED. The smoke binds older frozen executables solely to exercise
  preparation, so its emitted command is not the final release launch command.
- [M] Independent source audit accepted the narrow inspector budget proposal
  in `inspector_load_limits.patch`, applied only to a TMP copy for now. Its
  seven strict positive manifest fields match the native loader, including
  rejection of null/unknown/duplicate/overflow values and the per-owner1GiB
  ceiling. The identical resolved limits reach reads, inspection and shared
  native import. Native compilation/tests remain pending after current freeze.
- [M] Main optimized library compilation finished around18:03UTC. CLI and
  inspector final build stages remain active under group329576; no runtime
  source was changed during compilation. Prior native correctness is not
  relabeled a completed optimized build.
- [M] Root and independent reviewer found operational packaging gaps before
  deployment: recipe flags overriding resource bindings, partial staging on
  interruption, early walk-policy validation, RAM-only resume overrides,
  exceptional guard failure receipts, and visible generation progress. The
  implementation agent is correcting them and extending regression tests.
  Final acceptance includes a real fresh combined4L pipeline, not just mocks.

Release supervision checks (18:19UTC):

- [M] The new pipeline's real signal/drain smoke passed in0.684s onCPU64,
  with no CAS workload: SIGTERM to its supervisor reached the independently
  grouped native stand-in and grandchild; both groups drained and handlers were
  restored. Evidence: `order-pilot-plans/pipeline-acceptance/signal-smoke-run/`.
- [M] Root independently reran35 runtime-order/staging tests,5 escrow-steering
  tests and27 shared-supervisor tests successfully. The implementation agent
  also reports the complete35 production/escrow-steering group passing. These
  mock/operational checks are not the pending native fresh4L acceptance.
- [M] Final operational changes permit per-invocation RAM overrides even before
  the first walking checkpoint; source, worker allocation and mathematical
  policies remain frozen. Interrupted staging uses a new atomic attempt and
  retains completed generation. Monitor initialization is exception-protected.
- [E] After the active build succeeds and drains, `s5_fixed_work_pilots` is
  authorized to freeze its binaries and run the four optimized matched4L arms
  in Ready/Epoch/Epoch/Ready order onCPUs32–47, with full cold-All checks. Root
  will then apply/test the already audited inspector-only load-limit fix. No
  hot-sector or production run is authorized by this qualification grant.
- [M] At18:24UTC `s5_fixed_work_pilots` independently reviewed the final Python
  operational fixes and exact frozen query geometry/role partition; no source
  blocker found. `runtime_order_pilots` separately reviewed the inspector patch
  against native loader semantics. The4GiB generation checkpoint allowance is
  per parent, not a whole-campaign disk or RAM cap; fresh A1 sizes remain unknown.
  Neither review substitutes for native execution.

Optimized freeze (18:32UTC):

- [M] The full campaign-profile build completed successfully in3727.479s
  (Cargo62m04s), exit0/no stop reason, with its entire process group drained.
  Root independently checked Cargo's opt3/non-test artifact entries and SHA256
  equality of separate read-only copies. Frozen CLI:
  `TMP/aster-integration-20260930-resumed/optimized-escrow-bin/rustred`, SHA256
  `38e0e7637a228aeccb13902915378d70f25736323916976819d2145ea19b1d89`.
  `FROZEN_BUILD.json` records source56176df5, full fatLTO/CGU1, exact command
  and preservation of older binaries. Compilation is excluded from solver time.
- [M] Only after freezing both executables, root applied the reviewed inspector
  load-limit patch and formatted it. The frozen CLI does not change; the old
  frozen inspector remains intact and will not be presented as the patched one.
  Seven example unit tests and a separate inspector rebuild remain pending.
- [M] Read-only API-diff review found no Python/core/candidate-generation API
  change between the last23-test installed wheel and this escrow slice. Escrow
  is CLI-backed, not a newly added PyO3 entry point. The current fresh native
  Python pipeline is the relevant acceptance gate; a rebuilt wheel alone would
  not test escrow. Existing installed-wheel compatibility may be retested and
  must retain its actual earlier provenance.

Optimized four-loop qualification (18:41UTC):

- [M] `s5_fixed_work_pilots` completed the Ready/Epoch/Epoch/Ready block on
  the same frozen38e0e763 CLI,16 selected A1 owners,508 routes,58 required
  queries and32 roots, W16 onCPUs32–47. Root independently inspected all four
  raw cold-All PASS reports and complete58-query certification tallies. All
  twelve native/cold/audit process groups drained; compilation did not overlap.
- [M] Native+cold seconds: Ready15.6965/15.2916; Epoch16.2951/16.2910. Epoch's
  median is5.16% slower, not a wall-time win. Native CPU is34.14% lower and graph
  domains33.04% fewer, with17.03% more native inspections. Epoch repeats have
  identical record/edge digests. The short runs produced no foreign-load
  samples, so host contention is unmeasured, not assumed absent.
- [M] Full evidence and interpretation are in
  `order-pilot-plans/optimized-qualification/FOUR_ALL_RESULTS.{md,json}`.
  Epoch's summary/Python diagnostic remains explicitly INCOMPLETE by transport
  design; actual CP6 cold-All is PASS. This is scoped query closure, not source
  certification or unrestricted family closure.
- [M] The inspector-only seven-test plus optimized-example build is running
  under `inspector-limits-release-build/`, after all comparison arms drained.
  The delivery CLI remains frozen and unchanged. The fresh4L generation pipeline
  gate awaits that matching inspector. A finite1,324-point5L Ready/E1024 pair
  is being prepared read-only; no further heavy grant yet.

Fresh public pipeline accepted (18:57UTC):

- [M] Inspector build/test passed0/null in746.408s, seven tests passed, and
  group1166539 drained. Root froze the new standalone checker as
  `optimized-inspector-limits-bin/inspect_candidate_orders`, SHA256
  `925a778b3393016bedee38f85e9db31b7a82a6aadb8e3c568c93ed740e1b4187`.
  The independent S5 reviewer confirmed normal opt3 artifact/source identity,
  separate read-only inode, unchanged CLI and native budget semantics.
- [M] `runtime_order_pilots` completed the real public Python pipeline on the
  combined4L control:4+12 newly generated sectors,523 rules/28 finite residuals,
  no reused sectors,16 owners admitted,508 route records unchanged and all58
  required queries preserved. Generation/admission/walk18.142s plus cold-All
  10.145s; these are measured phase times, not a matched speed ratio.
- [M] Raw cold-All passed all58 queries/32 roots and17,957 of17,957 inspections;
  zero violations/errors/uncovered/frontiers. Root and the independent S5
  reviewer separately checked these reports. Graph:26,025 domains/495,898 edges.
  The dashboard retained a stale conservative closure lower bound at this short
  exit; neither that display nor native4 was used as closure evidence.
- [M] Existing installed selected-profile wheel against the new frozen CLI:
  23 tests passed,0 failed/skipped,1.136s guarded time. No new wheel or PyO3 walk
  API is claimed. Root also independently reran all35 epoch-steering tests.
- [M] Seven owned process groups drained and heavy/pilot locks were free.
  Exact scope equality, generation counts and receipts are in
  `order-pilot-plans/pipeline-acceptance/{ACCEPTANCE_RESULT.json,RESULTS.md}`.
  No source fix was required by the native pipeline test.
- [E] The S5 lane now owns the separately granted optimized finite-five
  Ready/E1024 pair, using the unchanged1,324-point query and historical67
  owners, not fresh all67 A1 rules. Root owns documentation/final release;
  no further feature development or production launch is authorized here.

Tested generation pipeline pushed; frozen source checked (19:04UTC):

- [M] Commit `b21e4522` is pushed to `origin/fable_5_1_parallel`. Its12 explicitly
  staged files contain the tested Python pipeline, input recipe, operational
  tests, narrow inspector fix and documentation. The license scan was clean;
  unrelated FeynKit edits, untracked research notes and all campaign/reference
  material were excluded.
- [M] A detached frozen source checkout exists at
  `/common/dev/rustred/TMP/releases/20260930-a1-epoch`, commit `b21e4522`.
  Its actual `nix develop` plus tracked pipeline successfully performed a
  metadata-only full5L preparation using the final frozen CLI and checker:
  `TMP/aster-integration-20260930-resumed/frozen-five-launch-check/`.
  Exact census remains67 owners/8,246 routes/116 required/67 auxiliary;
  all payloads are explicitly UNGENERATED. No native process was launched,
  and the detached checkout remained clean. This validates the script/import/
  Nix path independently of the mutable development checkout.
- [M] Finite5L Ready completed168.634s native (82.797s preparation,70.943s
  traversal) and151.192s raw cold-All, primary319.826s. All760,609 natives
  were re-inspected; its sole required query/root is verified with zero
  frontiers/uncovered/errors. The separately timed Python diagnostic and
  matched Epoch arm remain pending, so no completed comparison is claimed.

### Historical resource handoff — 2026-09-30 16:49 UTC

- [M] Library-test-only rebuild remains live under group3937511, no compiler
  error. Root has not interrupted it. `s5_fixed_work_pilots` owns the already
  granted guarded full-library retry after successful build and complete drain.
- Independent reviewer rehashed the frozen CLI and all four bound screen
  plans, checked unchanged inputs, resource isolation and explicit cold-All
  commands, and accepted the9/9 interface receipt. No screen has started yet.
- `runtime_order_pilots` has a **conditional** grant for the four-loop E0/E32
  pair only: require corrected library success, drained test group and explicit
  independent acceptance first. Each arm retains the inclusive1800s boundary,
  existing locks and CPUs32–47. Finite-five requires a further root grant.
- [E] The scheduling hypothesis is overlap of separated slow prefixes, not
  an assertion that Epoch is now faster. Extra lookahead may increase obsolete
  work and cannot reduce initialization/cold verification by itself. Both total
  native-plus-cold cost and work inflation decide the next step; no busy-core
  observation or smaller diagnostic wait bucket constitutes qualification.
- [E] Scale of that isolated opportunity: the instrumented finite control used
  272.214s native plus159.209s cold-All. Hypothetically deleting all95.841s
  classified credit waiting, with everything else fixed, gives1.286x whole-
  boundary speedup (22.2% less time), versus2.104x for its182.654s traversal
  alone. These are arithmetic ceilings for this isolated measured component,
  not speed predictions or universal bounds on the architecture. Additional
  work/preparation savings would be needed for the1.5x overall deployment gate.
  This saved-rule control includes no new IBP generation.

Native retry handoff (16:56UTC): the test-only rebuild passed0/null in803.4221s,
Cargo13m20s, peak single-child RSS20,279,880KiB; its complete process group has
drained. Agent `s5_fixed_work_pilots` discovered the library-test executable
from CargoJSON and launched the authorized1241-test retry under
`escrow-native-tests-retry`, unchanged50-core affinity and heavy/pilot locks.
The frozen solver CLI remains unchanged. Compilation is excluded from every
campaign timing. Independent acceptance and the first comparison are pending.

Native retry result (16:59UTC): **1229 passed,0failed,12 intentional ignored**,
0filtered,116.60s library time; guarded exit0/no stop reason in117.1803s.
Evidence:`escrow-native-tests-retry/{request,result,child}.json` and`stdout`.
The12 ignored tests are unchanged explicit external-fixture/scale/exploration
tests, not license/worker-count skips. All five controller escrow tests pass,
including the corrected stale/quarantine fixture; pool, memory, periodic-resume
and W50 transfer tests pass. Interfaces remain9/9PASS. The original failing
receipt remains retained, not overwritten. No runtime source changed during
the correction. Independent process-drain acceptance precedes the screen.

### First lookahead campaign pair — 2026-09-30 17:03 UTC

- [M] Both selected-A1 four-loop arms completed without censoring and passed
  the registered independent cold-All gate for all58 required queries and32
  roots. Same frozen `d68ada…319b` app-opt1 CLI,16 workers, CPUs32–47,
  base76/cut16/FIFO/snapshot/G2 Union/h0. Evidence:
  `order-pilot-plans/escrow-next/runs/{e0,e32}-r1/four-all/`.
- [M] E0:9.372s native +11.150524s cold +1.141054s secondary diagnostic;
  25,945 domains,16,932 inspections,481,213 edges. Strict historical/new E0
  durable mathematical state and physical lookup/verification counters match.
  This comparison does not claim byte identity for typed record payloads.
- [M] E32:8.562s native +11.144431s cold +1.137377s secondary diagnostic;
  25,948 domains,16,964 inspections,481,694 edges. The mechanism issued448
  extra jobs, reached108/108 logical slots and20,894,936 retained buffer bytes
  against the256MiB admission threshold, with no measured byte overshoot.
  All queued/computing/reserved/result inventories drained at completion.
- [E] Traversal5.187941→5.210677s is essentially neutral. The roughly0.81s
  launcher-inclusive difference is not demonstrated engine acceleration in a
  single small control. Work inflation is small (32 additional inspections,
  approximately0.19%). This is a correctness/integration screen, not the
  optimized1.5x performance gate or a production campaign recommendation.
- Root conditionally authorized the preregistered finite-five E0/E1024 pair
  after independent review and complete drain of the four-loop pair. Scope
  remains one1,324-point query using67 saved owners/8,246 routes, not the116
  production queries. Each arm retains its1800s inclusive pilot allowance;
  production and source generation remain untouched. Agent
  `runtime_order_pilots` owns execution, `release_correctness_audit` reviews
  evidence independently, and root coordinates resources and interpretation.

Finite control launch (17:04:28.929UTC): independent four-loop safety review
accepted all receipts and verified every owned group drained. Agent
`runtime_order_pilots` started the authorized finite-five E0 baseline under
`escrow-next/runs/e0-r1/five-finite`, collector group57163/native group57165,
same frozen CLI and reserved CPUs32–47. Startup is verifying the existing8,246
routing maps. No new rules are generated; source and production remain
untouched. The E1024 arm follows only after this baseline's full cold gate and
historical flag-off identity check. The finite comparison has no result yet.

Finite baseline accepted (17:13UTC): native274.294s, preparation84.942s,
traversal182.661s, cold-All156.206282s and secondary1.141144s; charged phase
sum431.641426s. It reproduces749,157 inspections,919,534 domains and6,978,690
edges with no pending work, errors or frontiers. Cold-All replays every native
inspection and closes the one required query/root. Independent review accepts
the strict historical/new E0 mathematical-state and physical lookup/verify
comparison. Cached closure refresh/timing and declared format differences are
not authority differences; typed-record byte identity is not claimed. Native
CPU631.035s and sampled own activity2.307/16 cores include preparation; foreign
activity averaged0.203 cores. All baseline groups drained before the next arm.

E1024 launched17:12:55.137UTC under the existing conditional grant, collector
group168057/native group168060, evidence`escrow-next/runs/e1024-r1/five-finite`.
Same frozen program/input/resource settings; total logical allowance1100,
base76 and256MiB returned-buffer admission threshold. This arm is still
running, not an accepted closure or speed result. Source remains frozen.

### Lookahead finite-five result — 2026-09-30 17:22 UTC

- [M] E1024 completed naturally and passed independent cold-All:754,706
  inspections replayed,925,732 domains,7,090,301 edges, the one required
  query/root closed, zero uncovered obligations/errors/frontiers. The separate
  Python transport diagnostic remains honestly INCOMPLETE; it is not the
  closure authority. Auditor verified receipts and all owned groups drained.
- [M] E0→E1024 native274.294→214.555s; preparation84.942→84.873s;
  traversal182.660534→123.744910s; cold156.206282→157.196070s. Including the
  secondary diagnostic, charged total431.641426→372.888661s (13.61% lower).
  Native+cold alone430.500282→371.751070s. Compilation and manual gaps are
  not included in these phase sums; both inclusive arm clocks were<1800s.
- [M] Waiting97.835→37.936s accounts for almost all59.739s native reduction.
  Native CPU631.035→632.496s (+0.23%), inspections+0.74%, domains+0.67%,
  edges+1.60%. Sampled whole-invocation own cores2.307→2.967; foreign mean
  0.203→0.281. This is real overlap evidence, not20-core saturation.
- [M] Extra dispatch credits were used525,512 times, not525,512 additional
  total inspections. Peak logical inventory1100/1100; retained capacity
  32,614,280B below256MiB; zero overshoot or inventory remaining at drain.
  Groups168060/240006/257788 absent and locks free17:20:06UTC.
- [E] This clears the preregistered causal screen: useful total improvement
  with little work inflation. It does **not** establish optimized performance,
  the1.5x Epoch-over-Ready gate, full116-query closure or a production switch.
  Root will commit the audited opt-in implementation, then qualify with one
  fully optimized frozen executable and contemporaneous matched baselines.
  Source remains frozen; production remains untouched.
- Agent `s5_fixed_work_pilots` performed a bounded read-only review of whether
  existing helper preparation is a justified next control: P1≈30s, P2≈33s,
  P3≈9s and boundary≈13s now dominate residual traversal. No new source work,
  CAS implementation, blind sweep or competing benchmark is authorized. The
  existing h2 mechanism is testable, but tiny source tasks remain about17µs
  and even halving source+reverse cost would save only about4% of the current
  whole boundary before overhead. Defer this optional single falsifier until
  the optimized profile; prioritize the existing traversal-heavy comparison.
- `runtime_order_pilots` now owns **data-only** preparation of matched optimized
  Ready/E1024 h0 controls for selected-A1 combined four-loop and the unchanged
  hot-five query, initially W16/CPUs32–47. Binary/hash must remain unbound until
  root freezes the build. The agent also plans an actual freshly built public
  Python escrow smoke; existing steering tests are not claimed as that run.
  No further solver run or build has been authorized to an agent.

### Audited milestone and optimized build — 2026-09-30 17:28 UTC

- [M] Implementation/evidence commit
  `56176df50385b5583b6c06a689e571953b977cf3` pushed successfully to
  `origin/fable_5_1_parallel`. Author and committer are the requested
  ValentinHirschi identity, with the existing Codex coauthor trailer. Remote
  was still9cc1acca before the fast-forward. No reference material, campaign
  output, license or unrelated FeynKit/user work was staged.
- [M] Root started `escrow-optimized-build` with the existing shared build
  guard, CPUs0–15,8 Cargo workers, heavy/build locks and memory headroom guard.
  Cargo target is the reusable `optimized/target`; profile `campaign` is
  opt-level3/fat-LTO/one codegen unit, **without** an app override. Targets are
  CLI `rustred` and `inspect_candidate_orders`. Exact command, nonsecret
  environment policy and child identity are in its `request.json`/`child.json`.
  Exec session32267 owns child group329576. At17:28UTC it is actively compiling
  rustred-app, no compiler error. Compilation is separate from pilot timings.
- Source is frozen. Earlier frozen binaries and checkpoints are preserved.
  No agent has a native launch grant during this build; the ordering/pilot
  agent prepares unbound data-only plans on the side. Qualification will use
  one newly frozen optimized executable for both schedulers, not compare the
  app-opt1 screen to an older optimized Ready timing.
- Python surface clarification from code inspection: owner-domain walking is
  currently available through the existing Python CLI-steering scripts, not
  a direct `import rustred` PyO3 walk method. Do not claim native escrow kwargs
  exist there. Final delivery will check a fresh installed native candidate/
  ordering API and an actual Python-steered native escrow control separately.
- Current head `3c41a5f7` is the pushed documentation-only follow-up to compiled
  source milestone56176df5. Root verified the optimized compiler process330248
  live under group329576 at17:29UTC, no errors; the user's gate relaxation
  changes only plan/measurement interpretation, not that running build.
- [E] Remaining candidates: the finite screen still spends about38s waiting,
  30s in P1 and33s in P2. Existing h2 preparation is a small runtime-testable
  opportunity, not an expected saturation breakthrough because source tasks
  average roughly17µs. Better generated rules remain a separate stronger work-
  reduction route (repeated four-loop evidence, not full-five validation).
  The already-prepared hot control is the next scheduler discriminator; larger
  core counts follow useful-work evidence rather than occupancy alone.

### Qualification preparation audited — 2026-09-30 17:36 UTC

- [M] The user-approved threshold clarification is committed/pushed as
  `7c5cf29a`; it changes no Rust source or running build. Root revalidated
  compiler330248/group329576 at17:36UTC (8m30s active, no compiler error).
  Exec session32267 remains the sole heavy build; it has not been restarted.
- Agents `runtime_order_pilots` and `release_correctness_audit` completed
  independent data-only preparation/review of all eight unbound optimized
  plans under `order-pilot-plans/optimized-qualification`. Two pairs per
  workload use Ready→Epoch then Epoch→Ready. All shared input/route/query
  bindings, original launcher rewrites, resources and distinct Ready/CP6
  acceptance paths were checked. The auditor caught an incorrect duplicate
  Ready label in pairing metadata; all four Ready matrices were corrected
  before binding or execution. No benchmark ran with that metadata defect.
- Four-loop qualification tests the actual E1024 candidate, not the E32
  safety setting. Hot-five keeps the exact existing R<=1/A<=12 query and no
  extra D floor; it is not silently narrowed to ensure quick closure. Same
  W16/CPUs32–47, base76/cut16/h0, FIFO/snapshot/G2 Union,256MiB admission
  policy and per-arm1800s inclusive allowance. Binary hashes stay zero until
  an actual optimized freeze. No native launch grant is implicit in the plans.
- `PYTHON_DELIVERY.md` and `python-steering-smoke-command.json` describe a
  fresh isolated correctness-wheel build,23 real installed candidate/order
  tests and a real CLI-backed Python supervisor escrow/cold check. The
  supervisor's actual argument parser accepted the exact prepared request
  without starting runtime/file work. A first thin-driver proposal lacked G2
  support and was rejected during preparation; the existing full supervisor
  supports both options, so no new Python API was invented. These native
  delivery tests have **not** run yet.
- Next executable actions after successful build and complete group drain:
  freeze/hash the actual CLI and inspector without overwriting prior copies;
  independently audit profile/source identities; bind and revalidate plans;
  grant first four-loop pair; then extend only through the registered gates.
  Fresh Python delivery remains a separate guarded slot. Final launch guidance,
  full67-owner A1 preparation affordability and useful wider-core evidence
  remain open; no smaller control proves the full116-query production scope.

### Integration and live observation — 2026-09-30 16:07 UTC

- [M] The sole owned heavy experiment is the fresh A1 standalone3822 source
  generation under `order-pilot-plans/selected-runtime-next/standalone3822`.
  It started15:51:29UTC; at16:03:48 it had used12m11CPU in12m18wall,
  RSS1,240,212KiB, no error or completed shard. A single selected sector uses
  roughly one core despite the16-core reservation; this is not a scaling test.
  Its generation stop is16:13:59; total preparation/drain/load budget ends
  by16:21:29. Author`runtime_order_pilots` monitors the existing guard. No
  compilation or second heavy experiment is authorized while it runs.
- [M] Escrow API/scalar5/CLI/Python source integration is ready for native
  validation;33 focused Python steering tests passed. Root measurement gates
  now distinguish base window from total logical capacity and retain the
  explicit byte allowance. All33 CP6 diagnostic/receipt unit tests pass:
  `/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
  -B -m unittest discover -s tools/research/epoch_cp6 -p 'test_*.py'`.
  These are synthetic/steering tests, not native execution or performance.
- Independent reviewer`release_correctness_audit` caught a diagnostic-reader
  mismatch between native scalar spelling`oldest-prefix` (omitted by default)
  and summary spelling`oldest_sequence_prefix`. Root fixed normalization and
  added an omitted/explicit-default regression. No native authority was weakened.
  The pool author is also testing rejection of duplicate logical keys after
  recycling a physical slot. Root owns runtime/public base-to-total glue;
  `s5_fixed_work_pilots` owns execution/RAII/lifecycle, and`runtime_order_pilots`
  owns request/steering/checkpoint/cold adapters. Native compile remains pending.
- [E] User status clarification: not every original Fable/Opus avenue has been
  exhausted. N1 modular witnesses, broader N2 geometry allocation removal,
  N4 coverage-first work, NUMA/memory tiering and helper-scope alternatives
  remain deferred/unqualified under their recorded reopening conditions.
  Current two-track focus is intentional, not a claim these avenues failed.
- [M] Epoch is not universally slower: historical W6 individual4L native+cold
  FG27.178→24.161s, BMW44.385→40.403s, H27.966→26.415s, X56.000→70.813s.
  These are one pair each on an older frozen build, with some gains in cold
  verification, not qualification of the latest architecture. Current optimized
  W16 combined4L medians22.429834→24.233158s and finite5L318.0429666→360.7542972s
  still favor Ready. Current comparisons use equal total physical-core budgets;
  role allocation differs. No current scaling curve or200-core gain is claimed.
  Today's fallback choice remains Ready16/G2Union; deployment remains postponed.

Standalone generation disposition (16:14UTC): **censored**, no candidate bundle,
report or completed sector shard; retained checkpoint metadata alone is not a
completed sector. Generation guard wall1350.620102s, outer deadline exit124;
preserved inner guard exit-9/reason`operator_signal_2` comes from the scheduled
deadline, not a user stop. Last direct native sample at22m02elapsed had21m49CPU
and1,427,840KiB RSS (~1.46GB); this is an observation, not an exact whole-run CPU
or peak-RSS receipt. Guard child accounting0CPU/12MB concerns the launcher and
must not be attributed to the solver. Process group3222656/native3222657 absent;
heavy/pilot locks independently checked free16:14:25. No cold load was attempted
without output. Full67 A1 regeneration affordability remains unknown; historical
source-policy artifacts were not substituted. The existing profile is app-opt1,
so this censoring does not establish an optimized matched speed comparison.

Escrow final source review (16:15UTC): independent reviewer found no current
runtime authority blocker. Tests now cover positive-extra held-prefix dispatch,
all logical statuses at stop, cancellation after P1 consumes result bytes,
late/duplicate slot messages, stale and quarantined results, and W1 effective
cut1 despite a larger declared total. A direct periodic-generation inventory
check was added and independently reviewed before source freeze. Actual returned-but-unmerged
monitor counts include recycled returned statuses, not an inference that would
mislabel restored or not-yet-submitted reservations. Native metadata/execution
remain pending; no extra-capacity speed claim.

Native build gate (16:19:54UTC): both guarded release metadata checks pass,
`escrow-check`29.1685s and`escrow-check-final`27.1562s, app/Python including test
targets; no compiler errors. Final check includes the direct periodic-generation
decode regression. Exact commands and nonsecret environment/resource settings
are in each evidence directory's`request.json`. Root has now started
`escrow-native-build` with the same optimized core and app-opt1 correctness
profile, CPUs0–15, heavy/build locks,8build workers, minimum250GiB start/150GiB
run host headroom. Targets are app library, CLI, routed-CLI and application API
tests. Compilation is separate from pilot timings. Source is frozen; production
CPUs128–227 and all LC2 files remain untouched.

Next registered causal screens (not launched): selectedA1 four-loop E0/E32,
then finite5 E0/E1024, base76/cut16/W16/snapshot/FIFO/G2Union, explicit256MiB
returned-buffer admission allowance when enabled. The first is a small-work
integration screen. The finite allowance is chosen from measured heavy-tail
sequence gaps981/1064 in`PROFILE_EXTRACTION.json`correlations0/2/1, not a blind
capacity grid. These selected top-window correlations are not representative;
new reservations may change the sequence and inflate work. Full cold-All,
logical/byte bounds, total blocking, traversal and whole-command-plus-cold time
must be examined. A smaller credit-blocked diagnostic bucket alone is not a
gain (byte-blocked waits are classified separately from full logical capacity).
Root authorizes no pilot before native lifecycle tests and independent review.
These app-opt1 screens cannot qualify the optimized1.5x deployment gate.
Data-only screen preflight was independently accepted: the four active plans
under`order-pilot-plans/escrow-next` retain identical owner/query/routing inputs
and resource policy; the finiteE1024 plan hash is
`05db01a54df723c8a3ba68ca70ad304ca984cc5e8690308aac8fb7efad3b7d25`.
Binary paths/hashes remain deliberately unbound. The finiteE32 proposal is
superseded and unrun, not another arm awaiting a sweep. Observational profiling
is disabled equally, so comparisons must use always-on total blocking and
whole native+cold costs, not treat an absent profile-specific bucket as zero.
Inspection of diagnostic accounting confirms logical peak is updated immediately
after dispatch (before cancellation can skip a loop). Shared returned-buffer
peaks are exported through observations: a normal drain observes their final
values, but stopped/error arms may miss late-return activity during join and
must label those exported values as lower bounds. This is a reporting caveat,
not lost checkpoint work; it is now explicit in the campaign-driver docs.

Execution handoff (16:25UTC): root's native build guard session90635 owns process
group3652085 and heavy/build locks, source remains frozen. At5m19s two compiler
processes were active, no compiler errors; compilation is not a solver timing.
`s5_fixed_work_pilots` has conditional authorization, only after successful
build/group drain, to execute the discovered app library test binary with one
test thread and physical CPUs32–81, strict license availability, existing heavy/
pilot locks and900s test timeout plus30s drain; routed-CLI/application API tests
follow sequentially. Report failures before any code repair or rebuild. Root
remains resource coordinator and final verifier. Neither this conditional test
grant nor the prepared screen plans authorize a production or performance run.

Native execution checkpoint (16:39UTC): compilation passed in1010.5037s
guarded (Cargo16m47s), peak single-child RSS19,676,128KiB, minimum host available
638,510,120,960B. The CLI is frozen read-only at`escrow-bin/rustred`, SHA256
`d68ada4577ccd71489375d536134a30a0a6b3c0ed10d1f3ec6fb98ffc56c319b`.
The first native library run completed normally with1228passed/1failed/12ignored
in116.45s; guard117.18s. No license/W50 skip markers; audited affinity is50
physical cores. The fail-fast driver did not execute interface suites.

The failure is in the new synthetic stale/quarantine test: its positive stored
target had all-zero lookup/verification counters, so P2 correctly rejected the
malformed fixture before the intended quarantine check. Root, author and
independent auditor checked`merge::lookup_accounting` and the existing
preparation fixture. A minimal **test-only** correction sets one exact hit and
one verified raw inclusion; the expected quarantine error,16 preceding accepted
records and poisoned-state assertions remain unchanged. No engine guard or
algorithm is weakened. The original failure receipt is retained. Other new
held-prefix, byte-budget, W1, periodic/P1 restore, memory and slot-generation
tests passed. Next: run the existing interface executables, rebuild only the
corrected library-test target, rerun that gate, then independently review before
granting any pilot. The frozen CLI need not be regenerated for a cfg(test)-only
fixture change. Production and prepared pilot outputs remain untouched.

Follow-through (16:42UTC): independent review confirmed the fixture correction
matches actual stored-hit accounting. Interface-only execution now passes:
`cli_routed_campaign`6/6 and`application_api`3/3,0ignored/skipped,6.142s guard,
group3931890 drained. The failed full-library run still remains in evidence;
its focused outcomes were controller escrow4PASS/1fixturefailure, pool escrow
2PASS, memory1PASS, inspector/controller profiling10PASS, plus successful public
binding/summary/monitor tests. Root started`escrow-lib-rebuild` at16:42:41UTC,
same release profile, build affinity0–15 and guarded resources, **library-test
target only**. Source stays frozen until the corrected test executes. These
are correctness gates, not performance pilots, and no live campaign was aborted
or otherwise touched in response to the user's status questions.

First-slice design audit (05:55 UTC): both tracks received a preliminary GO,
not approval of unimplemented code. Ordering uses a shared immutable validated
program; nonuniform routing/symmetry transport is an explicit proof gate.
S5 starts with immutable bucket/reverse preparation and canonical folding;
initial miss resolution, typed records, bulk dependencies and P4 remain required
follow-through. Root authorized `parallel_gate_critique` edits in epoch merge/
preparation and a narrow bounded Store lookup wrapper. Full controller/pool
wiring and shared Cargo edits remain coordinated separately. No build or pilot
has started; existing failed experiments are not repeated automatically.

Follow-through (06:00 UTC): approved a small `rustred-order` crate containing
only lawful order metadata, checked integer evaluation and transport. This lets
its focused tests run without compiling/linking Symbolica; core adapters still
require core/app rebuilds. Root owns manifests/lock; ordering author owns source
and adapter changes. The auditor refined the transport boundary after reading
the actual reducer: distinct per-owner orders are already legal because routing
after entry strictly lowers support count. Equivariance is required when reusing
a transported rule/proof, not as a blanket restriction on independent owners.
Tests will preserve mixed-order routing and reject same-support owner hopping.

Kernel checkpoint (06:09 UTC): all15 `rustred-order` release tests passed in
0.01s; Cargo's standalone compilation took1.83s, guarded whole command5.160s,
peak single-child RSS198756KiB. Command: `cargo test --release --locked --offline
-j4 -p rustred-order -- --nocapture` inside Nix with the existing resource guard,
CPUs0–7/build-0+heavy locks and local target. Evidence:
`TMP/aster-order-kernel.fiY5ua/release-tests/{request,result}.json` and stdout/stderr.
Only the new algebra-independent crate compiled. The auditor read all six source
files and confirmed finite-fibre/sign coverage, signed accumulator bounds,
allocation-checked canonical parsing and coordinate pullback. These checks do
not yet establish integrated B-order generation or whole-engine speedup.

The pure kernel is committed/pushed as `1feb5c71`. The S5 author has drafted
bounded immutable bucket/reverse preparation; root has drafted real source-wise
Tracker/Edges batches with scalar differential tests, exact fold boundaries and
failure injection. Both passed independent source review and the warm full
`cargo check --release --tests --locked --offline` for core/app/Python:61.182s
guarded,58.08s Cargo, exit0/null,2440784KiB peak single-child RSS. Evidence:
`TMP/aster-order-kernel.fiY5ua/integration-check`. This is metadata compilation,
not execution of the new S5/bulk tests. Both helpers remain unwired; full native
execution and performance claims are pending. The brief source freeze is lifted
for the end-to-end adapters and controlled source-resolution/typed-record work.
No native pilot or new production campaign has started.

Integration checkpoint (06:27 UTC): the second P2 source slice passed independent
review. Source-ordered headers and bounded 256-miss tasks resolve against an
immutable Store; canonical deduplication and error selection stay in original
order. Cancellation now reaches existing aggregate-index group/block scans.
Eight synthetic tests include actual multi-block inputs, stale/current misses,
quarantine and error precedence; they are authored, not yet executed. The prior
metadata PASS does not cover this newer slice. Typed Epoch records and their
checkpoint/cold-reader adapters are the author's next implementation slice.
Root retains ownership of bulk dependencies and is reviewing P4's shared-layer
boundary so the remaining S5 work is not serialized behind the record migration.

The ordering author ran a deliberately diagnostic core-only metadata build,
which reported104 integration errors (chiefly ownership after making order
policies shared rather than Copy): exit101,50.174s guarded, no timeout. These
are retained failures to repair, not a passed gate. The updated independent
`rustred-order` suite then passed17/0 (Cargo1.79s, guard4.157s,199496KiB peak).
Evidence: `TMP/aster-order-adapters.DOQX7C/{core-error-harvest,order-tests-v2}`.
Both guarded process groups drained and the heavy/build locks were released.
No CAS code generation or campaign run was involved. Root authorized the broad
ownership propagation because actual order identity must reach every replay and
descent consumer; preserving Copy through process-local handles would weaken
that contract. Current source remains an in-progress integration tree.

### Resumed integration — 2026-09-30 08:00 UTC

Delegation became available again: `order_integration_resume` owns O,
`s5_typed_resume` owns P2/typed records/controller integration, root owns P4,
and `p4_shared_snapshot_audit` independently reviews P4. The docs-only interrupted
checkpoint was committed/pushed as `931bcd1d`; unfinished source is still local.
No production action or new performance claim.

- [M] The next core test metadata harvest (`TMP/aster-integration-20260930-resumed/core-check`)
  failed with remaining test ownership errors:49.183s,2425632KiB peak single-child
  RSS. Compiler-directed repairs preserved assertions. The following full
  core/app/Python metadata check (`full-check`) compiled all three libraries,
  but failed test targets with4 core and11 app diagnostics:58.194s,
  2440404KiB peak. Both process groups drained. These are diagnostic builds,
  not native execution or acceptance.
- Ordering now carries full program metadata through candidate generation,
  inspection, save/load, checkpoints and CLI/Python. Tests for source replay,
  mixed-order strict-pinch composition, cut refusal, cold load and descriptor
  mismatch have been added; they remain unexecuted. The Python descriptor
  builder's3 pure tests passed (agent receipt;0.001s).
- S5 now uses concrete typed records in P3 and CP6 restore. Cold audit retains
  its independent proof logic through a one-row projection from the same
  authenticated binary stream. Reserved preparation helpers execute through
  one private pool while the coordinator polls stop/progress; no extra
  nested/global compute pool is introduced. This integration is under test,
  not yet a demonstrated throughput improvement.
- Root replaced the two full lookup replicas in source with shared immutable
  geometry/live pages and geometric aggregate-index cohorts. Lookup preserves
  retired exact positives, dominant-orthant ordering and the minimum current
  live containing ID across cohorts, with ID-bound independent verification.
  New roots are prepared off to the side; failed refresh leaves the old root
  and committed canonical state intact. Retained-root count/lag and logical
  changed-data charges bound overlap; these charges are not an RSS promise.
  Differential, quarantine, compaction and cancellation tests were added.
- [M] The isolated shared-page primitive compiled with optimized rustc and
  all4 tests passed, including every injected fallible path checkpoint and
  held-root immutability. Evidence: `pages-compile2` (3.238s guarded,
  199848KiB peak) and `pages-run` (1.209s guarded; tests0.00s) under the same
  evidence directory. The first wrapper compile failed because its absolute
  module path misplaced the test submodule; corrected only the ignored harness.
  This test does not exercise native geometry or the whole snapshot engine.

All builds/tests use the existing environment license, guarded local TMP,
heavy/build-0 locks and CPUs0–15 (isolated page test0–7), never LC2's128–227.
Warm checks use `cargo check --release --tests --locked --offline
--message-format=json --config profile.release.package.rustred-app.opt-level=1
-j8 -p rustred -p rustred-app -p rustred-python`; compilation is not a pilot.
Next: full metadata gate after P4 integration, fix audit findings, then a
coherent native test build. Only after these gates come matched optimized pilots.

Integration follow-through (08:15 UTC): `full-check2` had4 test errors,
`full-check3` one typed-counter visibility error, and **`full-check4` passed**
all core/app/Python `--tests` metadata:32.170s guarded,29.59s Cargo,
1427592KiB peak. The standalone runtime-order suite passed again17/0
(`order-kernel-tests`:2.157s guarded,199380KiB peak). Native integrated tests
have not run. Source remains uncommitted pending them, not a launch build.

Independent audit results and current corrections:

- P4 source review found no remaining false-containment/publication-atomicity
  defect after corrections for sparse-live cancellation polling, unchanged-root
  retention age, and responsive exact-key compaction. Root also fixed overlapping
  retirement sets across different survivors of one cut: clearing the same old
  ID twice is legal, while a repeated ID within one sorted row is not. Added
  forced hash-collision tests local to the new index, phase/owner/correlated and
  wide-summary comparisons, and exact-merge4095/4096/4097/8193/mixed15000-row
  boundary/interruption tests. These compile, but have not executed yet.
  Old full-replica lookup cloning is being removed; no default Ready change.
- The new exact-key layer compacts already sorted prior runs, hashes/sorts only
  new tails in4096-row std-library chunks, then uses checkpointed BinaryHeap
  merging. Logical retention charges are explicitly **not** physical RSS
  bounds. Full native-summary rebuilding on carries and binary-search lookup
  costs remain performance risks to measure, not claimed gains.
- Independent ordering review found descriptor/replay/descent/mixed-owner
  design coherent. Direct Rust reducer/owner constructors still need an
  explicit check against each solution's retained order; the ordering author
  is fixing that API consistency gap, not adding a compatibility exception.
- Independent S5 review found two operational gaps before native acceptance:
  helper0 preparation cannot poll an arriving stop/RAM request through its
  local AtomicBool, and metadata currently prevents changing helper allocation
  on checkpoint resume. The S5 author is adding a coordinator-only callback
  through the serial path (parallel tasks retain atomic polling) and permitting
  validated worker repartition independently of logical scratch limits.
  Mid-preparation interruption and helper-enabled→W1 resume tests are required.
- Typed-record authority, deterministic preparation and poisoned publication
  failure paths passed source inspection otherwise. The critic is rotating
  into a separate sidecar-test slice for buffered tails, write/seal failures
  and projection parity; root will review those tests independently.

No pilot, speedup or production deployment claim follows from metadata PASS.
All running campaign state is untouched. The long native codegen is held until
these audit fixes stabilize, avoiding a knowingly obsolete expensive build.

Native-validation freeze (08:20:21 UTC): the audit fixes stabilized and
`full-check5` passed all core/app/Python test metadata, including helper0 polling,
helper-enabled→W1 restoration, direct Rust order binding, nontrivial K3 replay
fixtures and eight binary-sidecar failure/projection tests. Guard55.178s,
Cargo52.35s, peak2450756KiB. Both independent reviewers conditionally approved
the corrected source for native testing; this is **not** runtime acceptance.
Root independently reviewed the sidecar tests authored by the rotated critic.
Python builder3/3 and the steering/audit group70/70 also passed again
(0.001s and9.496s respectively), using mocks where previously documented.

The native build has **actually started** at
`TMP/aster-integration-20260930-resumed/native-tests-build`, session61380,
with the established guard, CPUs0–15, heavy/build-0 locks and warm
`TMP/codex-runtime-discovery.280crc/target-native`:
`cargo test --release --no-run --locked --offline --message-format=json
--config profile.release.package.rustred-app.opt-level=1 -j8
-p rustred -p rustred-app --lib`.
Core is optimized normally, app opt1 is the existing correctness-only build;
neither this compilation nor these executables establish production timing.
All implementation authors have frozen edits while Cargo reads the source.
The SHA256 of the sorted per-file SHA256 listing for `.rs` under core/app/order/
Python `src` is
`019d8f094263ecfec0f6a209a4ec3213539d35d7df227aa9c0db0116702decbf`.
HEAD is still docs checkpoint931bcd1d plus the preserved uncommitted task tree;
unrelated FeynKit and reference material are not part of this build scope.
The guarded result and executable paths/hashes must be recorded on completion
before any focused tests. Then run full applicable gates and matched optimized
pilots; do not infer a speed gain or campaign readiness from compilation.

Continuation (08:26 UTC): the final independent S5 re-review found no blocker
in the serial cancellation and helper-repartition corrections. Native build
continues on the frozen source; no code was edited or production state touched.
While it compiles, the ordering and S5 authors are preparing exact executable
acceptance inventories and runtime-only pilot inputs. The separate auditor is
checking the matched timing/scope boundaries. Compilation/test-profile timings
will not be compared to the optimized production baseline. Source approval is
conditional on runtime evidence, not a recommendation to switch campaigns.

[M] Core test code generation completed before the application test executable.
At08:34 the12 focused native programmed-order/admission/replay tests all passed
(test body0.02s; guard1.150s; peak12316KiB). This includes exact generated K3
replay, transported comparisons, lawful weighted descent, mixed-owner strict
pinches and rejection of mismatched order/sector admission. A broader regression
group then passed727 tests with5 ignored (test body1.18s; guard2.157s).
Evidence: `core-order-native` and `core-order-regressions` under the resumed
integration directory. Executable `target-native/release/deps/rustred-135112256e6c039b`
SHA256 `775eaa598252a5c68b603eb932b86710b8b10239afd7c9c5d49e9907366eac45`.
These lightweight correctness runs used one test thread, separate CPUs16–17
and `aster-light-test.lock`, while the application compiler retained CPUs0–15
and the heavy/build locks. This explicit resource amendment does not authorize
concurrent heavy pilots or any performance comparison. Full core tests are next;
the S5 author owns focused app execution after its build drains. Production is
untouched.

[M] Full core native execution completed at08:37: **2864 passed,32 ignored,
zero failed**,137.10s test body,138.221s guarded,191328KiB peak single-child RSS,
exit0/no stop reason. Evidence: `core-full-native/{request,result}.json` and
stdout/stderr. The12/727 focused groups above are subsets, not additional unique
coverage. This is a correctness result, not a campaign performance measurement.
The app test compiler is still running; production readiness remains unclaimed.

The independent measurement auditor implemented a narrow diagnostic comparator
in `tools/research/epoch_cp6/{compare_state.py,test_compare_state.py,STATE_COMPARISON.md}`.
Root independently reviewed it and reran the combined26 Python tests, all PASS
in0.212s. A real historical drained four-loop checkpoint self-comparison passed
(agent receipt). It compares unchanged durable geometry, ledger, chosen edge
targets, anchors, query roles and logical work across versions. It explicitly
does not decode typed records or replace native authentication/cold-All gates;
full record equality is `NOT_COMPARED`, and physical counters/cache/timing are
reported separately. The ordering author is preparing a separate v3 staging
helper so descriptor experiments do not reuse or overwrite the historical v2
stager. S5 comparisons will retain the actual old owner payload bytes.

At08:45 the separate staging helper in `tools/research/runtime_order/` passed16
synthetic tests and independent review. Its README records executable native
acceptance, the actual historical-payload smoke and a small structurally motivated
order portfolio. These proposed orders have no new performance result yet.
Staging explicitly cannot establish a payload's actual order: a small Rust study
inspector will reuse the existing native inspection API before any comparison.
Root also reviewed `docs/epoch_s5.md`, correcting the description of exact,
dominant-orthant and minimum-live lookup precedence. Documentation-only work
does not alter the frozen engine. The full core receipt/binary/source binding
was independently rechecked and passed; the application test build continues.

The native study example `crates/rustred-app/examples/inspect_candidate_orders.rs`
now checks actual saved order identities and then passes the same immutable
payload buffers through the existing shared owner loader for sector/family
admission. Its source received independent conditional approval. The warm
example metadata check passed (`inspector-check`:3.154s guarded,0.43s Cargo,
199528KiB peak; CPUs16–17). Four decision tests and actual payload admission
are still unexecuted. It adds no CAS parser and does not claim replay/closure.
Existing engine source/manifests remain frozen; the auto-discovered example
does not change the in-flight library build.

Next source-window cleanup, explicitly pending rather than silently fixed:
the S5 author's read-only review found `epoch_lookup_tests.rs` still comparing
the default binding with literal semantics3, while the new binding correctly
uses4. A separate foreign-manifest test labels4 as a future version but rejects
because of an unknown field; make future-version rejection independent there.
Also refresh the stale S2 comment in `worker_budget.rs`. Gather actual native
failures before one consolidated correction/build, preserving every semantic
assertion. These findings do not authorize weakening identity or restore checks.

Continuation (09:18 UTC): the native app test compiler is still processing the
same frozen source; the complete core suite already passed. No restart, new
engine edit, production action or performance claim. The separate ordering
author delivered three runtime JSON recipes under
`examples/input/four_loop_combined/order_programs/`: explicit reference order,
shared-interface priority, and momentum-support-density ties. Independent audit
verified exact public Python-builder equality, E-primary admissibility, the
actual pinch-routing census and density-weight arithmetic. These are unmeasured
input hypotheses, not selected production settings. The public builder tests
passed3/3 again. Existing Source-A discovery policy will remain fixed when
comparing these B-orders.

To keep the next phase coordinated, `order_integration_resume` now owns exact
runtime-only order-study command preparation; `p4_shared_snapshot_audit` owns
the independent fixed-work S5 measurement-plan audit; `s5_typed_resume` will run
focused and full app native tests immediately after the guarded build completes.
All performance runs remain behind native correctness and the optimized binary
gate. Resource-intensive measurements stay serialized; lightweight metadata and
input validation do not justify racing heavy campaigns with the compiler.

[M] Runtime-only pilot preparation is ready (09:27 UTC), with no solver launches.
Root independently reran the order planner's validation: five arms, twenty native
argv files, unchanged16 owners/508 routes/58 required queries; both root downsets
remain314/328 sectors. Evidence/runbook:
`TMP/aster-integration-20260930-resumed/order-pilot-plans/` (`prepared-v2` is current).
The matrix separates the default/source-winner comparison from all B-order
comparisons and uses one future optimized binary. The independent S5 protocol
audit prepared six templates/twenty plans under sibling `s5-pilot-plan/` and
verified exact historical-runner rewrites, fixed B16, explicit worker partitions,
query bytes, resource masks and cold gates. Its `VALIDATION.md` is preparation
evidence, not a completed measurement. New binary identities remain deliberately
unbound; no fresh arm or repeated pair has run.

Build-cost caution: historical `native-build-f3f707af-opt0` exited101 after1178.6s
with oversized-text/GOTPCREL linker failures, while opt1 completed in4565.9s.
Do not repeat opt0 as an assumed shortcut. The current unchanged opt1 build has
passed its last long optimization thread and is still finalizing; no native app
PASS is inferred from that. A bounded read-only compilation-structure audit is
assigned to the ordering lane, without delaying or expanding the two workstreams.

The order-plan audit then requested two narrow safeguards: freeze all emitted
command bytes and distinguish a successful wrapper from an actually completed
Ready run. These are implemented in `prepared-v3`, which supersedes v2 without
overwriting it. The independent critic reran all4 synthetic receipt/mutation
tests (PASS), validated40 pinned command/helper/template files, and checked the
completion predicate against an existing real A1 result (read-only PASS).
No new solver call occurred. New Ready acceptance requires native exit0, no
censor, a complete result and fresh cold-All PASS, rather than launcher exit alone.

The bounded compilation audit found a concrete caller-type multiplier at the
public walk and cold-verifier entries: each `impl Fn(Value)` type currently
propagates into16-arity dispatches. Two API-preserving thin wrappers delegating
to non-generic borrowed-observer bodies received independent design approval.
They require no Box, Send/Sync bound, CAS change or internal callback rewrite.
The walk body will borrow its request to retain the public ownership/drop scope.
A cheap existing fixture can exercise non-Send borrowed state, callback thread
identity, Ready/Epoch and cold verification. Implementation remains pending the
native test harvest; this source evidence is **not** measured attribution of the
LLVM tail or proof of faster builds. Runtime descriptor changes themselves already
need only regeneration, not Rust compilation; new primitive capabilities still
legitimately require a rebuild.

[M] Frozen native build completed successfully at10:08 UTC: guard exit0/no stop,
6490.656s wall,31787.621s child user CPU,1691.890s system CPU,
99527096KiB peak single-child RSS and584314572800bytes minimum host availability.
Cargo reports108m07s; this is compilation, **not solver timing**. The source
fingerprint still equals the08:20 freeze. App test executable
`target-native/release/deps/rustred_app-38597ba0e64b4e2a` SHA256
`b8c853fa323c265d4102e9d914832839f812788102b2e350bf8bddc2eaf2b4a5` is about1.8GiB.
Receipt: `native-tests-build/result.json` under the resumed integration evidence.
`s5_typed_resume` now owns focused and full application execution; no app PASS
is inferred from linking. Root keeps the source freeze through failure harvest.
The very large native-test image strengthens the reason to measure the two
reviewed monomorphization boundaries at the next necessary rebuild; it does not
prove a cause or a compile-speed gain by itself.

Native application harvest (10:15 UTC): seven focused groups passed:
preparation10, wire3, record-store8, snapshot14, bulk7, edges5 and restored record
body9. Controller tests passed21/22; the failure counts selected nonprefix
attempts as if they were committed cuts. The exact single save, committed k=1,
16 records and32 outstanding identities all passed. Responsive P2 may now stop
a second selected proposal before publication. Independent audit confirms this
counter is invocation-local telemetry; retain the exact state/replay assertions
and allow the justified1–2 selection attempts. It passed in the later full run,
so its timing-sensitive expectation must be fixed rather than hidden.

[M] Full app error harvest: **1189 passed,6 failed,12 ignored**,248.02s test body,
250.237s guarded,196564KiB peak, exit101/no operational stop. Evidence:
`s5-native-full-harvest/{stdout,stderr,result.json}`. This is **not** a passed
application milestone. The six failures concern two candidate fixtures that
change coordinate priorities without binding their actual saved order, stale
semantics expectations in epoch walk/request-binding tests, an allegedly future
version now equal to current4, and CLI capability metadata. Twelve W50 portions
(ten across owner-batch cases and two native delegation cases) explicitly skipped
under the16-core affinity; W1/W2/W6 parts ran. A separate50-physical-core rerun is
required, not a claim that every width passed.

The source window is now open for one consolidated correction: the ordering
author repairs fixtures by generating/binding the intended order, keeps strict
admission and all application assertions, and implements the two reviewed
observer entry boundaries with focused regressions. The S5 author owns version
and selected-attempt expectations, unknown-field versus future-version handling,
and the stale worker-allocation comment. Follow-up found an **actual API metadata
bug**: `walk_semantics_probe` still emits nested CP6 schema2/semantics3 although
the writer is3/4. Root authorized correcting the probe from native constants and
testing both fields; changing only the assertion would conceal this issue.
The production Python plan already advertises3/4. Fresh independent auditor
`native_failure_audit` replaces the earlier critic after a per-agent thread limit,
and is reviewing the correction diffs. No matched pilot or production action yet.

Correction checkpoint (10:27 UTC): both implementation authors delivered their
slices and independent `native_failure_audit` found no source blocker. The CLI
probe now derives CP6 format/schema from the writer through crate-private
reexports. Version fixtures distinguish unsupported future versions from
same-version unknown fields with validated previous-generation fallback. The
rolling assertion retains exact committed/replayed state and bounds only the
invocation-local selected-attempt telemetry. The two non-generic observer
boundaries preserve borrowed, non-Send public callbacks, with W2 Ready/Epoch,
cold verification, resume and cancellation tests. No build-speed claim yet.

[M] The first correction metadata gate nevertheless failed one fixture ownership
error: `SectorSolution` is not Clone, so the newly written `.clone()` cloned a
reference. Ordering author owns the narrow fix and independent re-review.
Evidence: `TMP/aster-integration-20260930-resumed/corrections-metadata/`,
guard28.165s, exit101/no stop,1450640KiB peak single-child RSS. No costly native
rebuild started with the invalid fixture. Pure-Python surface checks continue
independently; the source hash before this correction was
`a9ed5da739d5dc6384143a42694df29f01b121ed210403fc79c99f004424a6e0`.
Root keeps production untouched. Corrected native, actual W50 subcases,
CLI/Python and optimized matched pilots remain mandatory gates.

[M] Correction metadata retry passed: guard29.168s, exit0/no stop,
1436352KiB peak; fixture now consumes owned solutions instead of cloning a
reference. Independent reviewer approved that repair. Source was formatted for
the four task packages only; unrelated FeynKit work was not changed. Native
rebuild started with frozen Rust-source fingerprint
`f7488be2fd8464b18095f93b825b36adc2967294065dac35eaea9befc5ff73ca`.
Exact command and resource receipt are under `corrected-native-build/`: app
lib/bin tests, CLI candidate/routed/application integration tests and native
order inspector example, release with app opt1 for correctness only, CPUs0–15.

[M] Root separately ran83 pure-Python epoch preparation/batch/checkpoint/lookup/
rolling, saved-walk audit and CP6 receipt/comparison tests: PASS in7.420s
(guard8.158s,110680KiB peak), evidence `s5-python-corrections/`. These exercise
steering and synthetic contracts, not the native solver or performance gate.

W50 count clarification from the independent auditor: the12 stderr skip markers
include two preflight calls in tests that intentionally exercise only W<=6.
There are **ten intended W50 subcases**: eight owner-batch cases and two native
delegation cases. Run the whole owner-batches group plus the two exact delegation
tests with no skip markers. Read-only topology audit confirms CPU32–81 contains
50 distinct physical cores and does not share cores with protected128–227;
actual launch affinity and locks still need enforcement. No new pilot result.

[M] Ordering surface lane independently passed26 lightweight tests (3 integral
order,3 source discovery,16 staging,4 command/receipt mutation). It revalidated
all prepared-v3 input/argv pins and three recipe/public-builder equivalences.
Evidence and future native commands:
`order-python-corrections/README.md`. Static distribution inspection is not an
archive test; separate source-archive preparation is assigned next without a
Rust build. The independent native audit receipt is
`native-audit/acceptance-audit.json`; its expected next full app inventory is
1198 active tests plus12 ignored, to be checked against the actual rebuilt list.

[M] Read-only LC2 observation around10:30 UTC: process360092 remains live with
its unchanged frozen executable,100-worker requested budget and protected
CPUs128–227. Latest sampled heartbeat had about189.247M discovered domains,
80.584M native inspections,60.069M pending and zero reported frontiers. The
recursive-closure snapshot still records6/67 original roots and is about590s
stale; it is not a fresh closure calculation or an ETA. Resident memory was
about81.6GB. No campaign files or execution controls were modified. These
figures do not justify either an eventual-completion claim or a switch before
the new implementation's correctness and matched-performance gates.

[M] Actual source archive now passes the existing distribution checker:
5,444,879 compressed bytes,1967 files,28,602,396 uncompressed bytes; SHA256
`34a6582799d88525cbed4e3be8e8a86e7aa5b12119ab8255305d0553ca9e0399`.
The first offline attempt lacked `is-macro` in the default Cargo cache; the
retry used its already-existing workspace cache (`TMP/cargo-home`) and completed
in4.00s without downloads or Rust compilation. Manifest/lock hashes unchanged.
Receipts: `order-python-corrections/`; native wheel/API tests still pending.
An exhaustive source-module inclusion check follows the sampled archive audit.

Preparation maintenance only: root's rustfmt changed the candidate parser's
byte identity, correctly invalidating `prepared-v3`. Root regenerated identical
study settings as **`order-pilot-plans/prepared-v4`**, now authoritative for the
future order portfolio. It preserves all five arms,16 owners/508 routes/58
required queries and the same roots/policies. Data validation and all4 mutation
tests pass. No native command was executed; no scope or strategy was changed.
The earlier failing-test executable was also preserved by reflink under
`native-tests-build/rustred-app-before-corrections`, with its original SHA256
confirmed, so the correction rebuild cannot erase that diagnostic evidence.

[M] Exhaustive source-archive check passed: all1795 actual Rust files in app393,
core1393,order6 and Python3 are present and byte-equal, including untracked S5
modules; no missing or stale source members. Independent v4 review confirms all
40 command/template objects equal v3 after directory renaming only, and all7
frozen input files are unchanged. V4 validation and4 mutation tests pass; v3
refuses stale parser hashes as intended. Receipts remain in
`order-python-corrections/`. Ordering lane next owns the matching native wheel
and API checks, but awaits root's build-slot grant after native failure harvest.

Native CLI harvest (10:43 UTC): Cargo reported the corrected normal library,
CLI and integration binaries before the application unit binary finished.
Root ran their low-memory correctness tests on32–47 under the pilot lock while
compilation stayed on0–15; these timings are not performance measurements.
Candidate CLI18/18 PASS (including programmed-order cold certification/application),
application API3/3 PASS, native inspector4/4 PASS. Evidence:
`corrected-cli-candidates/`, `corrected-application-api/`, `corrected-order-inspector/`.
Actual CLI probe consistently reports CP6 schema3/semantics4. CLI SHA256:
`041bf2bfd0ad685f53ebe44341d8cf4d1e260d59f00a56fb072c3d2cae2ad9ad`.

`corrected-cli-routed/` reports5 passed/1 failed: its old probe assertion expects
five top-level fields rather than six including `epoch_checkpoint`. Root proposes
a test-only correction asserting both exact CP6 objects as well as six fields,
retaining all legacy/per-policy/single-line/argument-refusal checks. Independent
review requested; the integration target will be rebuilt separately without
changing the engine or invalidating the long application compilation. No failed
test is counted as passing, and the application native gate remains pending.

[M] Corrected consolidated native build finished in1034.540s guarded (Cargo
17m11s), exit0/no stop,6752.447 userCPU seconds,161.211 systemCPU seconds,
20126080KiB peak single-child RSS. It built app unit/bin tests, three integration
targets and inspector tests. The new app test executable is about311MiB, SHA256
`40880233174f88e7caf54c4c20585d76612518a60a2f6cade2f88a3ffc919f9a`.
The earlier unit image was about1.8GiB and its build108m07s. These are observed
compile receipts with different target sets, not a controlled compiler benchmark
or a solver-speed comparison; the callback-boundary reduction removed concrete
duplication but its isolated timing contribution has not been measured.

The independently approved routed-CLI assertion repair rebuild took19.160s and
its rerun passed6/6 (0.13s test body,1.153s guarded). Existing native app/engine
artifacts were reused. Separate native loading of the unchanged historical
four-loop selection passed for all16 saved owners and their16 corner targets:
zero missing owners/rules,16 declared terminals, no regeneration. Evidence:
`historical-owner-load/` and `historical-corners-four-result.json`. It applies
zero rules, so this is a native old-payload/load smoke, **not** a closure or
whole-campaign reduction test.

Corrected app focused tests: order5, declared-affine1, namespace1, probe1 and
controllers22 all PASS. Two new observer tests fail because they expect the
sequential `domain_started` event in the parallel Ready observer stream. The
ordering author is diagnosing exact event semantics before changing those
fixtures; non-Send/thread/envelope/cold/resume/cancellation guarantees remain
required. Independent full50-core harvest continues in `corrected-app-full-w50/`.
The public help also contains stale descriptions of implemented order/CP6
controls; an audit is determining the minimal truthful help correction for the
same next batch. No mathematical engine change has been requested by these
test failures. No campaign performance or deployment claim yet.

[M] Full corrected app harvest on50 distinct physical CPUs completed:
**1196 passed,2 failed,12 ignored**,118.38s test body,120.217s guarded,
199680KiB peak, exit101/no stop. There are **no SKIPPED markers**. All intended
W50 cases executed successfully; the only failures are the two new observer
fixtures. The independent auditor confirmed this inventory and released both
resource locks. The Ready implementation intentionally emits `domain_started`
only when `!ready`; short runs also need not emit timed heartbeats. The author
is replacing the fixture's invented event assumption with a genuine initial
delegation event after native dispatch, with independent review of deterministic
ordering before accepting it. The mathematical/runtime callback implementation
is unchanged. Cold/resume portions of that new fixture still need execution.

Root applied independently specified public-help corrections: document separate
source-discovery and persisted integral-order JSON flags, current semantics4,
supported CP6 G2/rescue versus refused resume-time activation, pinned rolling
snapshots, and execution-only helper repartition versus frozen logical limits.
Only help/comments/error wording and help assertions change; existing admission
behavior remains intact. These join the observer fixture in one next native
batch. Historical explicit semantics3 export fixtures remain unchanged.

Final correction freeze (11:03 UTC): the proposed delegation-event fixture was
rejected before implementation because initial reservations can prevent the
assumed delegation. The accepted repair checks Ready's actual post-work
`checkpoint_saved` event, while an Ordered-policy test checks cancellation after
an actual dispatched `domain_started` event. Both exercise the same public
borrowed, non-Send callback boundary; coordinator-thread, drain, cold-reinspection
and resume assertions remain. No walker event or mathematical algorithm changed.
Independent source approval is recorded in
`native-audit/final-correction-source-review.json`.

[M] Final core/app/Python test metadata passed in28.167s guarded,1438388KiB peak;
app/Python formatting checks passed. The corrected-source fingerprint is
`28a0aad51be811bd8f193fe1876401e136573355135cb31e971bbd086aac7fe2`.
Root launched `final-native-build/` with the same reusable native Cargo target,
app correctness optimization1, CPUs0–15 and heavy/build-0 locks. The independent
auditor owns the next observer/full50-core gate; root owns CLI/API integration
harvest. These are correctness builds, not performance comparison binaries.

[M] The packaging lane's PATH-dispatch proof passed: Maturin issued two metadata
and four package-list calls through the existing workspace Cargo cache, no
compile calls,4.162s guarded. Its refreshed private source archive passes the
exclusion checker and contains all1795 Rust source files byte-identically at
creation. Archive SHA256:
`e3033751a893720adde6fbb56d9a5776f82356471526c2b2c3ce67847a257d0b`.
This resolves the missing optional metadata-dependency cache without downloads,
outside-workspace cache writes or changing compilation's cached source paths.
A native wheel and public Python API execution remain required, not passed.

Current delegation: `native_failure_audit` independently checks final native
results; `order_integration_resume` prepares the native wheel/API lane and
refreshes runtime pilot inputs; `s5_pilot_preflight` checks the existing matched
S5 execution plan without running a solver. Root integrates, coordinates build
resources and verifies results. No production campaign is modified or launched;
the mathematical goal and both measured-performance gates remain open.

Final frontend checks (11:15 UTC): the final build's completed CLI/integration
artifacts passed candidate CLI18/18, routed CLI6/6 and application API3/3. These
low-memory correctness checks ran on32–47 while the app unit target compiled
on0–15; the independent auditor confirmed the fresh artifact paths, zero skips
and normal exits. Final CLI SHA256:
`288a89034a0e63574ce969e2541591c315b96b2061db706b5b97aef5b4bb7ac2`.
Evidence: `final-cli-candidates/`, `final-cli-routed/`, `final-application-api/`.
They are not performance measurements.

[M] Final consolidated native build succeeded:1004.537s guarded (Cargo16m42s),
6655.995 userCPU seconds,153.539 systemCPU seconds,19721980KiB peak single-child
RSS, no stop reason. Final app unit SHA256:
`34e7d1c7f76abdffb8d43331780e0d0c68e8beeb9a99f6843f2a294e4357d0cd`.
The independent auditor verified the new executable and owns the observer/full
50-core rerun. Inspector4/4 also passed and was independently checked. Packaging
has a conditional build/test grant only after the complete app gate passes;
its native wheel/API checks will not overlap another heavy job. Evidence:
`final-native-build/`, `final-order-inspector/`.

[M] Final complete application suite is green (11:22 UTC): **1198 passed,
zero failed,12 ignored**,118.84s test body on50 distinct physical cores, with
no worker-availability skip markers. The focused observer2 gate also passed,
exercising Ready/Epoch cold verification and resume plus Ordered cancellation.
Evidence: `final-app-observers/`, `final-app-full-w50/`. The conditional native
wheel/API lane can now proceed. No solver throughput claim follows from this
correctness gate; matched optimized pilots still decide the deployment choice.

Installed Python gate (11:26 UTC): the actual Maturin wheel built in34.170s
guarded (Cargo27.93s), reusing core/app libraries. The archive checker and fresh
offline installation passed. Focus4/4 passed. The first full19-method run failed
only its two cold-subprocess invocations because the ignored launcher modified
`sys.path` without inheritable `PYTHONPATH`; no engine/test assertion changed.
The explicit installed-wheel `PYTHONPATH` retry passed **19/19, zero skips**
(0.553s test body,1.157s guarded). Both failed and successful receipts remain in
`order-python-corrections/`. The extension SHA256 is
`2dc59909afdc80a77159b89a146ea60655f5ee7c4d508a26a58fb59c42567618`;
the matching CLI retains `288a8903…`. The local Linux-tag wheel is not a claim
of PyPI-portable distribution.

Root started the matched-profile build at11:26 UTC, after all correctness groups
drained: `cargo build --profile campaign --locked --offline --message-format=json
-j8 -p rustred-app --bin rustred --example inspect_candidate_orders`, guarded on
0–15 with heavy/build-0 locks. Its target was copied from the old optimized cache
without modifying the old checkout or frozen executable. This build has full
optimization, fat LTO and one codegen unit, **no app opt-level1 override**.
Evidence: `optimized/campaign-build/`. New benchmark executables are not yet
frozen; no performance pilot or production action has run.

Build interruption (11:28 UTC): the first optimized build guard received SIGTERM
and recorded exit-15/`operator_signal_15` after108.198s, with no compiler error.
Root verified its process group had exited. A detached launch attempt returned
PID3053461 but produced no guard receipt and was absent on inspection; it was
not counted as a running build. Root then resumed the same cached build through
the ordinary supervised execution session82017, evidence
`optimized/campaign-build-resume/`. This is an interrupted build, not a failed
mathematical/performance gate. The automatic goal continuation also replaced the
agent roster: `release_correctness_audit` finishes the Python receipt review;
`s5_fixed_work_pilots` and `runtime_order_pilots` take over the already prepared
measurement lanes. No scope, source code or production state changed.

Independent installed-Python and staged-scope audit passed: wheel/native payload
identity, current source-builder identity, actual public import and cold child
loading,4/19 zero-skip test inventories, original launcher failure retained,
and all243 staged paths checked. No excluded reference/campaign/license/FeynKit
content is staged. Root is committing this tested implementation milestone;
the active goal still requires optimized matched gains and launch instructions.

Long-build supervision (11:39 UTC): the historical comparable campaign-profile
build receipt is3786.5s, so a build tied to a short execution session risks
repeated interruption. Root deliberately stopped and drained only its own
resumed build (430.314s, no compiler error), then launched the same cached
command with `setsid --fork` and the unchanged resource/memory guard. This time
guard PID3179647 and `optimized/campaign-build-detached/{request,child}.json`
were confirmed live; its logs/exit receipt remain authoritative. The earlier
plain-background attempt did not survive and is not represented as successful.

To use that wait productively, root authorized a separate **correctness-only**
combined-four-loop Epoch helper0/helper2 smoke on the already tested opt1 CLI,
using fresh evidence, unchanged saved rules/58 queries, CPUs32–47 and the pilot
lock. The optimized build stays on0–15/heavy/build-0. These low-memory checks may
overlap compilation, so their timings are explicitly ineligible for performance
claims. The existing optimized timing plans remain unmodified. Required results
are actual cold-All success and equal canonical graphs, not busy-core counts.

Representative S5 correctness (11:44–11:49 UTC): both four-loop helper0/helper2
arms passed the existing strict CP6 acceptance gate and independent cold-All.
Each verifies58/58 required queries,32/32 roots and31,826/31,826 actual native
inspections over51,166 domains and1,149,999 dependency edges. Pending, reserved,
frontier and uncovered counts are zero. Canonical durable graph, physical lookup
and verification diagnostics agree exactly between15+0+1 and13+2+1 worker
partitions. Independent `release_correctness_audit` confirmed inputs, commands,
uncensored exits and actual receipts, not just the agent's summary.

Evidence: `s5-correctness-smoke/README.md` and `h0-h2-state.json`. The native
checkpoint-only exit4/summary-INCOMPLETE is not the closure evidence; cold-All
recomputes all32 roots rather than trusting the stale18/32 cached telemetry.
This establishes the frozen control's scoped saved-rule coverage, not unrestricted
family/source certification, and **not speed** (opt1 plus concurrent compilation).
The original optimized comparison plans remain unexecuted.

Root authorized the same correctness-only boundary for the finite five-loop
control, helper0 then helper2 only if the first succeeds. It loads67 owners but
has one required R2/A11/D≥9 query (1324 integer inputs), not the full116-query
production scope. Evidence: `s5-correctness-smoke-five/`; each arm retains its
inclusive1800s budget and predeclared stop/drain limits. No production change.

[M] Finite-five-loop helper0 passed the strict CP6 gate at11:58 UTC. Independent
cold-All reinspection covered all743,502 native inspections,910,957 domains and
6,861,296 dependency edges; the sole required query/root closes, with zero
uncovered obligations, errors or frontiers. Native traversal drained naturally,
and the cold verifier finished inside its predeclared240s phase allowance.
Root checked the actual `cold-verify.json` and `cp6-accepted.json`, not just the
cached recursive-closure counter. The helper2 arm is authorized next, retaining
the same exact scope, worker total and inclusive budget. These opt1 checks
overlap the optimized build and are correctness evidence only. No speed or
all-five-loop closure claim follows. The performance comparator portfolio v5
was independently revalidated (five arms,20 commands, no native invocation);
its binaries remain unbound until the campaign-profile build finishes.

[M] Finite-five-loop helper2 also passed at12:11 UTC, with the same910,957
domains,743,502 native inspections and6,861,296 edges. All required scope (one
query/root) cold-reinspects successfully, zero uncovered/frontier/pending/error
counts. `s5-correctness-smoke-five/h0-h2-state.json` reports exact durable
mathematical-state equality; same-layout lookup/verification objects and logical
scratch allowances also match. Record/edge digests agree. The comparator does
not decode full typed records: each native cold reader independently validates
its own authority. The cached closed-domain count differs744,315 versus49,198
because time-driven maintenance refreshed at different points; cold verification
finds all910,957 closed in both. Neither cached count is a completion estimate.

Responsible pilot agent: `s5_fixed_work_pilots`; independent receipt/scope audit:
`release_correctness_audit`; root inspected the actual paired report and cold
results. No performance claim from this opt1/concurrent-build test. The guarded
optimized build remains live, with `runtime_order_pilots` assigned to monitor,
validate and freeze both normal executables after successful completion.

The same agent prepared and the auditor checked a runtime-only follow-through
at `s5-pilot-plan/ready-followthrough/`: seven existing-input Ready controls,
explicit frontier-stop/checkpoint settings aligned to Epoch, and fresh matched
Ready/Epoch ABBA instructions. Both full-JSON/CP5 and summary/CP6 output costs
remain charged to native+cold time, rather than labelled pure scheduler time.
No additional harness, source edits or solver runs were introduced by that
preparation. Timing pilots remain unexecuted pending the optimized freeze.

[M] Documentation/correctness checkpoint committed and pushed as `27b95768`;
compiled implementation remains `711b18c5`. At12:16 UTC the optimized build
finished the application library and moved to the two normal executable targets.
The build guard remains live; no final executable has yet been frozen.

[M] Read-only LC2 observation at12:24 UTC:195,033,248 discovered domains,
84,345,405 local completions,59,853,363 pending,zero frontiers,approximately
84.2GB RSS and9.6 observed cores in that instantaneous sample. Its unchanged
hour-window pending-growth/completion metric is+0.0693; coordinator duty is90.3%
(55.0% ordered commit,35.4% preparation). The cached6/67 root count is stale
by about2207s, not the all116-required-query closure status. The current Stage-A
binary remains `0995f0fd…`, PID360092, CPUs128–227; production was only read.
This continues to motivate measuring publication/preparation relief, but is
neither a prediction of S5 gains nor a closure ETA. Host headroom remains well
above the build guard's admission/stop thresholds, and no swap growth is reported.

[M] Full optimized build passed at12:44 UTC:3898.575s guarded (Cargo64m55s),
4221.704 userCPU seconds,174.266 systemCPU seconds,17858460KiB peak single-child
RSS, exit0/no stop. Both compiler-artifact entries are fresh normal executables,
opt-level3, non-test, with the requested campaign fat-LTO/one-codegen-unit profile.
The guarded process group drained. Compilation is excluded from solver timing.
Responsible freeze agent: `runtime_order_pilots`; root independently checked the
actual successful build receipt and SHA256 of the read-only copies:

- `optimized/bin/rustred`: `560f0dddea01e7be22aa7820a2c943a67c1f84b2766c1f6f1a47897867d33420`
- `optimized/bin/inspect_candidate_orders`: `6baeca06d9f1aa55bb7635cda3429a8e346eb9e885dfae48f01e2ac68d954988`

Paths are below `TMP/aster-integration-20260930-resumed/`. Compiled source is
`711b18c5`; subsequent `27b95768` changes documentation only. No old binary or
production file was replaced. Root granted `s5_fixed_work_pilots` the first
combined-four-loop optimized old/new h0 ABBA, then h0/h2 if the exact graph and
cold gates pass, on32–47 with heavy/pilot locks. `release_correctness_audit`
independently reviews freeze and measurement receipts. Ordering, finite/hot
five-loop timing and wider pilots wait for the first outcomes; no speed claim yet.

[M] First optimized combined-four-loop old/new helper0 pair passes cold-All and
exact durable-state comparison:58 required queries,32 roots,31,826 inspections,
51,166 domains,1,149,999 edges. Independent receipt audit passed. Old/new native
times are15.308/14.801s; cold guards15.181823/15.153171s; combined30.489823/29.954171s.
The apparent1.76% reduction is **inconclusive**, not a demonstrated speedup:
traversal is10.794511/10.853804s and sampled foreign busy cores differ2.445/1.999.
Sampled tree RSS is371,507,200/336,637,952 bytes (one pair, not a memory-win claim).
The reversed second pair and preparation-helper comparisons continue under the
same grant. Evidence: `s5-pilot-plan/runs/{old,new}-h0-r1/four-all/` and the paired
state report. New ordering matrices are now bound to the frozen binaries but
have not run. The prior correctness-only timings are not used in this comparison.

[M] Optimized old/new combined-four-loop ABBA is complete and independently
audited. Both pairs retain exact graph identity and full cold reinspection.
The second pair reverses the small timing difference: new27.5484s versus
old27.1787s native+cold (+1.36%). Two-sample medians are28.83425s old versus
28.75128s new (approximately0.29% lower). This is **performance-neutral/inconclusive**,
not the required decisive gain. Sampled foreign load varies between1.28 and2.45
cores. Coordinator phase diagnostics show P3 falling from0.51–0.54 to0.20–0.22s
and boundary from0.28–0.30 to0.15–0.17s, but P2 increasing from1.53–1.59 to
1.72–1.80s. Phase savings are real recorded observations, not a replacement for
whole-command comparison. Native sampled RSS is4–9% lower, below the memory gate.

Decision: finish the registered h0/h2 ABBA, then give the optimized runtime-order
portfolio its slot. Subsequently measure the larger finite/hot five-loop controls
before inferring how shared-index/publication costs scale. No source tuning or
rebuild from this small-control result alone. LC2 remains untouched; the S5
implementation is tested, but its performance deployment gate remains unmet.

### Optimized measurement follow-through — 2026-09-30 13:13 UTC

[M] All eight combined-four-loop S5 arms completed and independently passed
full cold reinspection. The h0/h2 helper block is also neutral/inconclusive:
native+cold medians27.69301/28.87475s; its slower second h2 cold check remains
in the comparison. Actual checkpoint storage falls52.269→20.200MB (61%),
record segments42.773→10.704MB (75%). These are storage, not RSS or throughput,
gains. Exact durable mathematical graphs agree across both old/new pairs;
helper pairs also match lookup/verification diagnostics. Detailed receipts:
`s5-pilot-plan/FOUR_ALL_RESULTS.{md,json}`. The independent reviewer approved
the tracked summary in `docs/research/final_order_s5_pilots_2026-09-30.md`.

[M] Runtime-order scouts use the same frozen optimized executables, regenerate
both complete four-loop parent downsets, stage the same16 owners/508 routes,
and retain all58 required queries. Four candidates passed native admission and
full cold reinspection. Measured generation+staging+admission+walk+cold totals:
A0 default99.282s; A1 sparse/coefficient-aware source visitation86.726s;
B0 A1 with explicit default order91.853s; B1 shared-interface order97.786s.
A1 reduces domains64,129→38,693 and native inspections24,415→15,311 relative
to A0. B1 reduces inspections further to14,322 but its generation cost erases
that benefit. These are single unpaired scouts, not a new deployment gate.

[M] B2 routing-density order generated/admitted successfully but encountered
four frontiers. Root authorized cooperative stopping of this owned scout;
supervisor248762/native group248764 drained, both heavy/pilot locks released.
The saved prefix has139,530 domains,100,739 native inspections,3,492 pending,
18/32 roots closed. Walk-through-drain156.212s is censored, not a successful
completion timing; no cold acceptance was attempted. The author and separate
auditor are inspecting the actual frontier reasons before drawing conclusions
about the heuristic or rule-search limits. No production process was signalled.

[D] Following the registered sequence, root granted `s5_fixed_work_pilots`
the larger optimized finite-five oldh0/newh0 pair, serially onCPU32–47 with
heavy/pilot locks. Both use the same67 saved owners/8246 routes and the same
single R2/A11/D>=9 query (1324 integer starting points), W16/B16/FIFO/snapshot/
G2Union. Old launched13:13UTC, inclusive deadline13:43UTC; new starts only
after old cold acceptance. Bound plans are under `s5-pilot-plan/{plans,bound-new}`;
exact runner/native argv and hashes are saved in each run. No broader scope,
helper sweep or source tuning is authorized by this first pair alone.

[E] Narrow next S5 hypothesis: geometric index cohorts may retain retired
containment entries until compaction, adding prefilter callbacks. Four-loop
actual callbacks rise440,008→599,701, not the3× broad candidate count (which
also includes bulk-rejected ranges). The oldest-cohort/minimum-ID shortcut
already exists and must not be reinvented or reordered. A live-aware filter
or bounded refresh is only worth implementing if the larger profile justifies
it; retired exact hits and dominant-orthant semantics must remain unchanged.
All root/agent source work remains frozen while these measurements execute.

[M] Independent B2 diagnosis: the four frontiers are two guard-classification
cells repeated under the owner's R5 and R12 starting queries. Saved records15/31
for owner `0111111111` identify batch0 rule11 excluded-conjunction branch1 and
rule13 equality0; `error=null` and `reached_missing_rule_claim=false`. This is
an unresolved classification under the chosen order, **not demonstrated missing
IBPs or an invalid mathematical comparator**. Its offset boxes fix the sole
inactive coordinate to zero, so simply lowering numerator rank does not remove
them. Exact guard-polynomial diagnosis is pending; no engine repair or scope
narrowing has been authorized. The auditor checked the complete scout table,
correcting three rounded walk+cold entries directly from raw stage receipts.

[M] Measurement/documentation milestone `84d597de` is committed and pushed;
the optimized engine remains unchanged at711b18c5. The complete source-selection
and S5 comparison report records unsuccessful candidates as well as wins.

[M] B2's two native guard-display diagnostics (CPU16,4.290s combined) recover
the exact coupled affine factor `1-n2+n1`. Rule11 excludes its zero set;
rule13 explicitly supplies an affine rule on that same zero set, excluding
`n2-2`. All original denominators remain in the diagnostic payloads. Both
displays stop at the64-event reporting cap, exit4, so no complete one-hop or
closure claim is made. This confirms the limitation is rectangular
classification of a mixed positive-dot diagonal, not absence of its generated
IBP. B2 is parked; adding a general affine geometry subsystem is outside this
optimization slice. Evidence: `order-pilot-plans/diagnostics/b2-guards/`.

[M] Optimized old finite-five baseline passed independent acceptance at13:22:
323.692s native,132.198566s cold,455.890566s total. Native preparation81.310s,
traversal236.274s; cold preparation82.228s. All743,502 native inspections were
repeated over910,957 domains and6,861,296 edges, one required query, zero
uncovered/frontiers/errors. This is scoped dependency-graph closure, not an
unrestricted source/descent certificate. The matched newh0 arm started13:23:17,
collector370976/native371715, inclusive deadline13:53:17; its result is pending.

[M] The new finite-five arm also passed and the independent reviewer accepted
the exact durable graph match:910,957 domains,743,502 natives,6,861,296 edges,
46,470 cuts, zero uncovered/error/frontier. New native326.514s plus
cold138.199082s =464.713082s, versus old455.890566s (+1.94%, single pair).
Sampled peak tree RSS6.759→6.564GB; foreign cores0.203→0.214. Native preparation
is80.207s and traversal240.403s. P3 falls11.911→5.003s, but P2 rises
13.273→15.369s and inspect181.966→190.349s. Summed lookup32.548→45.827s is not
coordinator wall time. Time-driven cached closure counts differ, while both
cold checks independently close all910,957 nodes. No performance win claimed.
Evidence: `s5-pilot-plan/five-finite-old-new-r1-state.json` and both run directories.

[D] Do not add helpers or rewrite the shared index merely to raise utilization.
The lane is aggregating existing old record timings once, after all timed native
groups drained, to distinguish cut stragglers from other inspection-phase cost.
Next S5 measurement is current Ready versus rolling oldest-prefix window76/
cut16/FIFO/h0; plans only until an explicit heavy-slot grant. O prepares two
fresh A0/A1 matched pairs and one limited784-point five-loop transfer, plus
one mechanistic B1 ablation (remove only private-excess degree priority).
Their runtime-only plans passed independent data audits; no extra source build.

[D] Ready/Epoch's common primary timing remains native-through-drain plus
native cold-All under identical scope gates. The historical Ready Python event
audit exceeded300s; treat this as separately reported secondary INCOMPLETE,
never PASS or an extra cost charged only to Ready in the primary comparison.
Actual secondary contradictions still block interpretation. This clarification
is registered in the tracked research report before any new Ready measurement;
no native authority check is removed or new certification framework introduced.

[M] Post-run streaming analysis of the old finite-five record file took13.134s
after all timed native groups drained. It groups all743,502 timed native/G2/
partial records by persisted merge epoch;167,455 aliases are the untimed rows.
Across46,470 cuts, sum of callback wall times314.570577s and sum of per-cut
maxima180.013936s; the ideal15-inspector per-cut lower bound totals180.013944s.
Measured inspection wall is181.966252s. Thus the observed fixed-cut phase
closely tracks its slowest members; it is not mostly unrelated waiting.
This is evidence for testing rolling overlap, not proof of a future speedup
or equivalent graph. Top10 long records all concern G2 Apply residuals of
owner `000011001001011`, maximum2.134845s; those timers do not separately
attribute G2 planning versus algebra. Independent methodology audit passed.
Evidence: `s5-pilot-plan/FIVE_FINITE_RECORD_SKEW.json` and
`FIVE_FINITE_RESULTS.{md,json}`. Final checkpoint storage1.143→0.500GB (56.2%
smaller), but cold loading5.578→12.972s is slower; do not equate compactness
with faster cold startup.

[D] Root granted O the next exclusive heavy slot: two fresh full4L matched
pairs A0→A1, then A1→A0, CPU64–79/W16, same560f executable and independent
1800s whole-arm allowances. Generation, staged ownership, admission and full
cold checks remain charged; no Rust rebuild. Plan directories
`order-pilot-plans/qualification-pair{1,2}` passed independent data checks;
the inherited screening-order fields were corrected before execution and
bindings refreshed without altering argv/input bytes. A0 has started under
durable supervision; no B/5L or S5 heavy run is simultaneously authorized.
An unrelated compilerPID549784 is active with affinity0–383, so host silence
is not claimed and contention observations remain part of interpretation.
ProductionPID360092 and its reserved cores are untouched.

[M] Source-selection A1 now passes the reproduced work-reduction gate on two
fresh full4L pairs, independently audited. Pair1 A0→A1 totals97.285982→84.538030s
with69,315→38,680 domains; pair2 (reverse execution) totals102.023588→82.553182s
with64,231→38,681 domains. Domain work decreases44.20%/39.78%, native inspections
37.27%/37.32%, whole time13.10%/19.08%; no offsetting throughput regression.
The20% **time-only** criterion is not met in both pairs, but the agreed >=20%
work criterion is. Waited-child CPU falls27.37%/27.84%; maximum single-child
RSS falls15.51%/12.11%, not an aggregate-memory claim. Every arm freshly
generates314/328 sectors with zero reuse, admits all16 owners and cold-verifies
all58 required queries/32 roots. This qualifies source visitation on this
four-loop control, not a changed mathematical comparator, five-loop transfer,
or the Epoch1.5x gate. All24 owned groups drained and locks released.

[D] Root granted the S5 lane fresh same-binary four-all Ready→prefix76→prefix76→
Ready comparisons, preserving the original saved rules rather than substituting
A1 in one arm. CPU32–47/W16, explicit FIFO/window76/cut16/h0, no lockstep
override; each arm keeps its1800s inclusive allowance and independent coldAll.
Ready-r1 runner695087 started13:47UTC. Limited5L ordering transfer, one B1
ablation, finite5L rolling and wider-worker measurements remain prepared but
not yet granted. No engine source changed or production action occurred.

### Current rolling-S5 comparison and next grant — 2026-09-30

[M] Committed/pushed `70cb287f` records the reproduced source-selection work
gain, finite-five fixed-cut measurements and independent audits. The current
optimized executable remains560f; no Rust changes or rebuild were involved.

[M] The fresh current-binary Ready→prefix76→prefix76→Ready four-loop block
completed and independently passed cold-All: every arm covers58required
queries/32roots, with zero remaining obligations, errors or frontiers. Primary
native+cold times are Ready21.928952/22.930716s and
prefix25.735908/22.730407s. Medians22.429834→24.233158s make prefix8.04% slower;
paired directions differ. This does **not** qualify the1.5x deployment gate.
Both Ready secondary Python audits passed (6.153/5.150s); Epoch's documented
CP6 secondary remains INCOMPLETE transport, not a contrary native result.

[M] Prefix deterministically retains51,143domains/31,895native inspections,
versus Ready69,156/24,406 and66,595/24,356. Fewer total domains do not establish
lower end-to-end cost: prefix performs about31% more native inspections, while
native CPU actually falls from roughly31s to21s. Rolling
does reduce the inspection-wait phase substantially (first arm3.328s versus
roughly8.2s fixed-cut), but that local improvement is not an end-to-end win.
All four owned groups and secondary checks drained before releasing the slot.

[D] Root granted the current finite-five Ready→prefix76 **first pair only**:
same prepared1324-point query, owners/routes, W16, CPU32–47, optimized560f,
shared locks and predeclared primary/secondary boundaries. Each arm retains
the1800s inclusive budget. No helper/W50 sweep or second pair is authorized
until the first pair identifies useful behavior. Auditor examines actual
receipts and the immutable-snapshot/work-selection mechanism. O lane prepares
the exact source-A1 regeneration/production-input inventory without native
calls or campaign mutation; limited5L transfer is next in the heavy queue.

[M] Read-only LC2 observation at heartbeat1790776664: running,201,591,293
discovered domains,88,130,037local completions,60,863,692pending,0frontiers,
about85.9GB RSS. The cached6/67 root closure is2558s old and is not the116-query
required-scope result. Pending growth/completion remains its unchanged metric
(+0.172 over the last hour). Production is untouched; no completion ETA inferred.

[E] Independent bounded source review identified a falsifiable explanation for
the four-loop work-mix difference, not a proven defect: Epoch's whole-cut maximal
antichain admission can choose broader Route obligations than Ready's original
event-order admission. Fixed-cut and rolling Epoch have31,826/31,895 natives,
only0.22% apart; the76-window alone does not explain the roughly31% gap to Ready.
Cold routed admissions rise885,369→1,053,267 while Apply successor admissions
fall962,149→884,304. Both engines revalidate stale misses, so stale immutable
snapshots are not by themselves an explanation. Native CPU is lower for Epoch.
Falsifier: one otherwise unchanged prefix76/**cut1** four-loop scout. If it does
not materially reduce the Route gap, reject the cut-coalescing hypothesis before
considering dispatch/G2 freshness. Root requested data preparation only, with
limited5L ordering transfer ahead in the heavy queue; no new solver code.

[M] O's production inventory exposed a real integration constraint. LC2's query
bodies match the tracked frozen183-query document (116required/67auxiliary), but
its saved generation policy is depth0/R10/sparse-factorized, unlike the fresh
qualified pilots' depth2/unbounded-rank/sparse policy. The owner loader requires
compatible common policies; splicing these programs is not a valid deployment
recipe. The67 owners use8,246 routes, four partial parent checkpoints and a
standalone817MB override. Its source/provenance was recovered rather than
silently replaced with an incomplete shard. The lane is mapping exact reuse/
regeneration options and the existing targeted-sector Rust/export seam; no
production input or saved artifact has been changed.

[D] Following independent preliminary design approval, root authorized
`runtime_order_pilots` to expose explicit selected-sector generation through the
existing candidate Rust/CLI/Python path. This is the runtime-expressivity part
of TrackO, not a new solver. Original family axes/root/global zero census stay
intact; only the nonzero solve inventory is filtered, before strategy validation
and checkpoint identity. Generation and checkpoint reload share validation;
missing sectors remain uncovered and certification still checks the full root.
The existing native program already carries its actual sector inventory, so no
new binary authority format or compatibility project is needed. New checkpoint
recipe/report scope will explicitly record selection. Tests must cover default
identity, exact selected/full rules, refused selection mutation and missing
sector/certification claims, native/Python surfaces and workers. Root retains
shared docs/manifests and resource coordination. No native build is authorized
during matched heavy measurements; frozen560f pilot inputs remain unchanged.

[M] Four-loop rolling comparison and the selected-sector addendum are committed
and pushed as `80ff0b91`. The current finite-five Ready primary independently
passes:167.841s native +150.201967s cold =318.042967s;967,843domains,
760,719native inspections,7,146,573edges, the unchanged single1324-point query,
zero errors/frontiers/uncovered. Its1.435GB full-result/checkpoint binding has
zero mismatches. The secondary Python diagnostic stopped at241.233s (exit124)
inside `union_covered`: **INCOMPLETE_TIMEOUT_NOT_PASS**, not a contradiction or
successful audit. Whole arm694s, within1800s; prefix finite collector921130
started14:10UTC only after Ready drained. Primary acceptance follows the
already registered boundary, not a retrospective timeout exclusion.

[M/E] Ready's owner preparation costs81.528s native and81.285s cold. Traversal
is71.034s; cold reinspection28.623s. Its interior traversal resource samples
average6.330 busy cores versus3.276 whole-run (including helpers/coordinator,
not inspector-only CPU). Even eliminating traversal alone would cap primary
speedup at1.287x if other costs stayed unchanged; also eliminating cold
reinspection gives1.456x. Thus the1.5x gate on this small control needs other
savings too. This does not relax the gate or prove larger LC2 behavior. A
predeclared existing traversal-dominant hot control is a possible later scaling
test; no hot launch or timing exclusion has been granted.

[D] **Correction before any repeated experiment:** the auditor recovered the
September29 cut1 falsifier at
`TMP/codex-ordering-study.7nspU4/cut1-falsifier/RESULT.md`. It already passed
all58 queries/32roots but changed Route work only31,558→31,547, with worse
wall time. Its retained report describes rolling cut1/window16/depth16, not
today's prefix76; this difference alone is not new evidence for reopening it.
Root cancelled the proposed cut1 preparation; no new cut1 run or data was made.
The earlier provisional cross-job-antichain explanation above is therefore
rejected as a promising untested avenue. Neither oldest-ready/cut1 nor speculative
per-origin admission changes are authorized. Preserve this negative result in
the active backlog rather than rediscovering it again.

[M] Current finite-five prefix76 completed and independently cold-PASS:
219.549s native +141.205297s cold =360.754297s, versus Ready318.042967s
(13.43% slower, one pair). Prefix covers the same single1324-point query with
919,534domains/749,157native inspections/6,978,690edges and zero uncovered
obligations, errors or frontiers. Native CPU falls536.576→476.732s, but traversal
rises71.034→133.595s. Inspection/prefix waiting remains about80s, despite falling
from fixed-cut190s. P1/P2/P3/boundary are19.88/17.55/5.02/10.35s. Sampled interior
process-tree activity is roughly2.6–3.4cores, not a useful20-core demonstration.
Both owned groups drained; Epoch's secondary stays its documented INCOMPLETE
transport, and Ready's timed-out secondary is not promoted to PASS.

[D] Heavy slot passed to O for the already bound narrow five-loop A0→A1
source-selection transfer on frozen560f/6ba executables and unchanged v3
steering, CPU64–79/W16. No B1 ablation or build is concurrently authorized.
Selected-sector implementation source is now under independent audit; this
does not retroactively change the binaries or authority of existing pilots.

### 2026-09-30 14:40 UTC — source transfer and bounded S5 follow-through

- [M] Limited five-loop source-selection transfer completed and independently
  audited: the same14-owner/14-route,784-point R<=1/A<=10/D>=9 query, full fresh
  generation and native cold-All. A0→A1 charged phase sums28.239343→20.149485s
  (-28.65%);3,149→2,185 domains (-30.61%);2,840→1,947 native inspections;
 149.939→75.029 CPU seconds. Both have14 finite residuals and zero uncovered
  obligations, errors or frontiers. One pair, depth2/unranked/sparse; this is
  not the full67-owner/116-required-query production workload. Evidence:
  `TMP/aster-integration-20260930-resumed/order-pilot-plans/FIVE_TRANSFER_RESULTS.{md,json}`.
- [M] The second narrow pair uses the actual LC2 generation policy instead:
  depth0/R10/SearchFinite/sparse-factorized, the same input/query and frozen
  executables, CPU64–79/W16. Author reports cold-All PASS and all10 process
  groups drained:27.232→20.235s charged time,3,151→2,191 domains,
  2,843→1,947 native inspections. Residual counts are18→16, not the previous
  pair's14→14. Independent receipt audit PASS confirms identical bound inputs,
  recipe, native owner admission and complete cold reinspection. Do not
  conflate different generation-policy pairs or claim a repeated full-production
  gate.
- [M] Selected-sector source passed the guarded app/Python release metadata
  check including test targets:28.159s wrapper,25.03s Cargo, exit0. Evidence:
  `TMP/aster-integration-20260930-resumed/selected-sector-check/`. This is
  **typechecking, not native test execution**. Unchanged core does not need
  another wholesale rebuild; consolidate app/CLI/Python native gates with the
  observational S5 patch below to reuse existing build caches.
- [D] The sole prepared B1 degree-row ablation is now granted after the
  production-policy pair, with the unchanged frozen v3 stager and guarded
  <=1800s-per-arm boundary. Afterward the timing slot returns to integrated
  native checks; selected-sector staging can then move to checkpoint v4.
- [E/D] Epoch heartbeat review found26 distinct phase-biased snapshots with
  zero queued jobs, positive undispatched pending work and57–76 returned jobs
  occupying its76-job window. This is evidence for measuring dispatch-credit
  blockage, not duration-weighted idle-core evidence. The independent reviewer
  approved one **observational-only** slice: mutually exclusive blocked-wait
  intervals, earliest missing prefix sequence, bounded slow-job timing and
  first-event/first-Admit markers. First Admit may be redundant; it is not
  evidence of a newly discharged obligation. No new ledger, streaming source
  authority, scheduling decision, checkpoint schema or CAS primitive is
  authorized. `s5_fixed_work_pilots` implements; `release_correctness_audit`
  audits. A private invocation-local `RUSTRED_EPOCH_PROFILE=1` switch keeps
  timers out of the normal path. Detailed mechanism/falsifiers:
  `s5-pilot-plan/ready-followthrough/S5_MECHANISM_REVIEW.md` under the evidence
  directory above. The premature partial-CP6 and bulk-edge remedies are parked.

Production LC2 remains untouched. Root's next integration boundary is native
selected-sector/interface and diagnostic-equivalence acceptance, not a new
full production run or a declaration of the1.5x Epoch performance gate.

[M/D] B1's single degree-row ablation completed at14:47UTC: both whole roots
freshly generated (314+328sectors,0reused), all58 required queries cold-PASS,
38,819domains/15,332native inspections/471,368edges. Charged phases total91.955s,
versus strong A1's prior matched qualification82.553–84.538s with about38.68k
domains/15.3k natives. This unpaired ablation provides no compelling gain; it
is parked, not promoted to another permutation sweep. All6 owned groups drained
and the heavy slot returned. The author is finishing receipts and the auditor
will check them; native candidate staging now moves to explicit selection-v4.

[D] To avoid a second instrumentation-only fat-LTO rebuild, first run the new
S5 diagnostic on the warm application-opt1 correctness build after native
acceptance. Independent reviewer agrees it can localize waiting and test
diagnostic equivalence, **not** establish optimized dominance or speedup.
Even with unchanged optimized core/Symbolica, application optimization changes
arrival, lookup and publication timing. Any actual architectural remedy still
needs a fresh fully optimized build and matched performance qualification.

[M] Measured-results/docs milestone committed and pushed as `7e9a7341`.
Independent review confirmed both limited5L transfers and the negative B1
ablation; it corrected stale next steps and avoided claiming missing IBPs were
disproved by a bounded affine-guard diagnostic. Only task documentation was
staged; unrelated FeynKit and untracked reference work remains untouched.

[M/D] Both source slices are frozen for native acceptance. The combined
app/Python release test-target metadata gate passed at14:52UTC in29.163s
(no compiler errors,1,445,480KiB maximum single-child RSS), evidence
`TMP/aster-integration-20260930-resumed/selected-profile-check/`.
The warm native app/CLI/API/inspector test build is running separately at
`selected-profile-native-build/`, with app opt1 correctness configuration,
unchanged optimized core cache, CPUs0–15 and heavy/build locks. Generation
selection is exposed through all three APIs; the private Epoch profile stays
invocation-local and changes no authority/schema. New stager-v4 synthetic
tests report25/25PASS; independent audit and actual native gates remain pending.
No extra fully optimized build or native diagnostic pilot has started yet.

[M] Independent review now accepts both new source slices with no outstanding
correction. The data-only future production recipe also passes: original parent
groups30527:17,30699:18,31740:10 plus the recovered standalone owner,32745:21.
All67 masks,8,246 routes and183 query geometries match the frozen inputs; roles
remain116required/67auxiliary. The standalone assignment to31740 follows the
original manifest hash, saved root, ordinal1116 and exporter—not containment
guessing. `order-pilot-plans/production-a1-selected/` intentionally leaves the
new sanitized selection, tested executable and caller resources unbound. It is
not a generated input set or a production launch grant. No hard500GB RAM ceiling
is introduced. An additional selected16-owner four-loop correctness recipe is
being prepared because the14-owner full/selected check alone would select its
entire downset and fail to exercise omitted jobs with a retained full route
census. Existing K3 tests compare decoded exact programs, not just counts.

[M] Native build completed successfully:988.655s guarded,16m26s Cargo,
19,587,952KiB maximum single-child RSS, no stop reason. Actual application
execution then passed **1,215 tests,0failed,12ignored** in116.32s (119.182s
guarded) on CPUs32–81. All seven new selection tests and nine profile/controller
tests passed; no license/worker-availability skips were emitted. Separate CLI
and interface execution passed19 candidate,6 routed,3 application API and4
inspector tests (7.143s guarded); the binary unit target has zero tests. Receipts:
`selected-profile-native-build/`, `selected-profile-app-w50/`,
`selected-profile-cli-api/` under the current evidence directory. These are
correctness-profile results, not optimized performance measurements. The
matching normal inspector and Python wheel are building in
`selected-profile-wheel-build/`; no source edits occurred during compilation.

[M] The matching inspector/wheel build passed in30.151s and isolated wheel
installation in1.139s. Installed-Python acceptance initially failed two cold
subprocess imports: the ignored harness added the wheel to parent `sys.path`
but omitted inherited `PYTHONPATH`. Root corrected only that harness, preserving
all test assertions and repository source. The retry passed **23 tests,0failures,
0errors,0skips**, including fresh-process CLI/Python artifact loading, in1.151s
guarded. Both receipts are retained as `selected-profile-python-tests/` and
`selected-profile-python-tests-retry/`; the independent auditor is reviewing
them. The executable and wheel are opt1-app correctness/diagnostic builds, not
optimized performance comparators. Frozen executable identities:

- `selected-profile-bin/rustred`:
  `859698b6c40da017009cb9e2454a5b008dd49dd595d5d9411e455daed9c42778`.
- `selected-profile-bin/inspect_candidate_orders`:
  `b29ef23df8a1c950fa1f6b4dc400b5ae96cbf4516a4a2c53b9bfa5df763064db`.

[D] Root grants `runtime_order_pilots` the prepared selected four-loop
integration:4+12 owner jobs instead314+328, unchanged508routes/58required queries,
qualified A1 depth2/unranked/SearchFinite/sparse policy, CPUs64–79 and existing
heavy/pilot locks. The grant includes native admission, Ready walk and independent
cold-All verification within the1800s arm boundary, not full67-owner generation.
Independent input audit passed; the bound cold command must reference the actual
walk JSON. `s5_fixed_work_pilots` prepares the finite five-loop profile off/on
diagnostic against the same frozen binary, but cannot launch until the selected
control drains. LC2 and its reserved cores remain untouched.

[M] Installed-Python receipt/source audit passed. The actual selected four-loop
control then passed every gate: fresh4+12 generation jobs,0reused shards,
16 natively admitted owners, unchanged508routes and58required query bytes.
The Ready walk produced39,178domains,14,878native inspections and472,625edges;
no pending work, frontiers or errors remained. Independent cold-All reinspected
14,878/14,878 native nodes and accepted58/58queries and32/32starting roots, with
zero uncovered obligations or violations. The six guarded phase times sum to
25.704s; this is an opt1 correctness receipt, **not** a speed comparison with
previous optimized runs. All six owned process groups drained and locks were
released, independently confirmed. Evidence:
`order-pilot-plans/selected-runtime-next/four-selected/`.

[D] Root now grants the finite-five profile off/on pair to
`s5_fixed_work_pilots`: frozen859698b6 diagnostic binary, W16/CPUs32–47,
Prefix76/cut16/h0, unchanged67owner/8,246route inventory and sole1,324-point
query. Both arms must cold-reinspect and compare durable mathematical state;
each remains bounded by1800s including preparation and drainage. Results are
causal observations in the correctness profile, not production speed evidence.
No new scheduler or partial-source publication change is authorized by the
earlier phase-biased samples alone.

[M] Independent file-level comparison additionally confirms all16 fresh
selected owner payloads are byte-identical to their prior full-root A1 outputs,
5,128,567bytes in total. No old payload was used to fill new output. Thus this
control avoided626 unused solver jobs while preserving the delivered programs;
the equality is observed here, not a universal binary-dump determinism promise.
Implementation/docs are committed and pushed as9cc1acca. The finite-five
diagnostic off arm started15:25:30UTC; no deployment action followed the push.

[M/D] The independently audited TMP-only production materializer reproduces
identical prepared inputs twice: all67owners,8,246routes and original183query
geometries survive, with explicit116required/67auxiliary roles and unchanged
loader budgets. Only the new manifest assigns owner66 to its recovered31740
parent; original files remain unchanged. Caller RAM750GB is accepted and its
external guard requirement is explicit. No full67generation was launched.
The small native-order inspector has fixed study budgets, so the standalone
owner's separately prepared public-loader fallback proves load/classification
only—not expected-order binding or closure. This contingency must not be
silently promoted to a stronger gate. Workspace formatting check also passes.

[M] The profiling-off finite control naturally drained (expected CP6-summary
exit4, uncensored), then cold-All passed in159.202s:749,157native cases,
919,534domains,6,978,690edges, one required query/root and zero uncovered/errors.
Those work counts match the prior optimized prefix control, but these opt1
timings are not an optimized comparison. Profiling-on, durable-state comparison
and causal analysis are pending. After that pair, preserve a small joint gate:
run the identical newly generated A1 four-loop owner files through Epoch, not
just Ready. This is additional integration/work evidence, not a new build or a
blind scheduling sweep.

[M/E] The profiling-on native run drained. Its independently checked small
summary attributes95.84112845s of97.712257653s blocking polls (98.09%) to the
exact credit-blocked start-state predicate. Wait-sampled means are2.631 computing
workers and72.816 returned results; these are not CPU utilization measurements.
The retained32 blocker windows cover only17.538s. Nineteen windows match retained
slow jobs and cover13.560s; all are Apply jobs,18 wholly after the first event and
16 wholly after the first Admit. Do not extrapolate early emission to the full
97.7s or treat first Admit as useful new work. This is enough evidence to review
one bounded completed-whole-result credit mechanism, not to implement partial
source publication or promise a speedup. Profiling-on cold verification and
strict paired durable-state comparison remain pending. Author and independent
auditor are reviewing count/byte/snapshot/restore bounds and the risk of extra
native work from reserving tasks that later containment could have avoided.

[M/D] The entire off/on pair now passes independent audit. Both cold-All gates
check749,157native cases and close the sole required query/root, with equal
919,534domains and6,978,690edges. Exact geometry, inspected/sealed state, ledger,
ordered dependencies, anchors, logical-record digests and physical lookup/verify
counters match. Cached periodic closure bit/timings are observational; raw typed
record files are not claimed byte-identical. All six owned groups drained.
Evidence: `profile-diagnostics/{RESULTS.json,OFF_ON_STATE_COMPARISON.json,
PROFILE_EXTRACTION.json}` under the S5 follow-through directory. Root grants the
separately audited selected-A1/Epoch four-loop joint check next, W16/CPUs64–79.
The proposed credit mechanism remains design-only pending independent review:
total logical capacity must increase (or reservations be safely eliminated),
not merely physical slots be recycled. Extra lookahead is a measured, bounded
experiment with possible work inflation, not guaranteed free parallelism.

[M/D] The joint selected-A1/Epoch control passes its CP6 raw cold-All gate:
58/58required queries,32/32roots,16,932native inspections,25,945domains and
481,213edges, with no uncovered/errors/frontiers/pending work. Native summary
exit4 and secondary Python INCOMPLETE remain explicit; the accepted authority
is the raw checkpoint cold result. Native9.388s and cold11.152s are opt1 control
receipts, not a new deployment speed claim. All three owned groups drained.
Evidence: `order-pilot-plans/selected-runtime-next/four-selected-epoch/`.
Root then grants the already prepared single-owner3822 A1 generation/load
probe: frozen859698, exact production generation policy, one actual selected
job on reservedW16/CPUs64–79,1350s generation plus load/drain within1800s.
It started15:51:29UTC. Its checkpoint is completed-sector granularity; retaining
an unfinished manifest does not promise restart inside an interrupted solve.
No full67-owner production generation or campaign action is authorized.

### Service-interrupted checkpoint — 2026-09-30 07:35 UTC

All three implementation agents returned an explicit usage-limit error; a
follow-up to the ordering lane was refused with `agent thread limit reached`.
This is an external delegation interruption, not an implementation milestone or
a completed goal. The last pushed code milestone remains `1feb5c71`. Substantial
new source is preserved in the working tree, including untracked new modules;
do not discard it or use this tree as a production executable. The user did not
ask to pause the goal, so it has not been marked paused or complete.

Root's completed work since the preceding checkpoint:

- Added explicit per-cut preparation helper/obligation/retirement controls to
  the Rust request, worker partition, CLI and all three Python steering layers.
  Helpers are **inside** the total worker budget; zero is the serial control.
  Counts bound logical scratch/output, not RAM, rank or cumulative work. Limits
  stop rather than truncate. This source still needs integrated Rust compilation
  and controller/metadata wiring by the S5 lane.
- Python steering and audit tests passed:70 tests in9.780s, covering the new
  options, existing batch/lookup/rolling/checkpoint behavior, frozen upgrade
  checks and JSON audit contracts. Command (from `examples/python`):
  `python -B -m unittest test_epoch_preparation_steering test_epoch_batch_steering
  test_epoch_checkpoint_steering test_epoch_lookup_steering
  test_epoch_rolling_steering test_audit_owner_domain_walk test_production_upgrade`.
  These use mocks/fake executables, not native solver authority. `TMPDIR` was
  `/common/dev/rustred/TMP`; no production campaign was launched.
- Research CP6 receipt tests passed17/0 in0.098s with
  `python -B -m unittest test_cp6` from `tools/research/epoch_cp6`. Manifest1/2
  pairs only with semantics3; new manifest3 only with semantics4. This read-only
  comparison adapter can inspect the old frozen baseline's receipts; it does
  not add native checkpoint compatibility. New diagnostic export schema2 pairs
  with semantics4, old export1 with3, and mismatched/unknown pairs are rejected.
- `git diff --check` passed. Production LC2 PID360092 was still live at the
  read-only check; no stop/start/resume or campaign mutation occurred.

Unfinished lane boundaries:

1. **Ordering:** seventeen isolated kernel tests passed. Core/shared-order
   propagation, candidate bundle/order codecs, CLI/Python generation options and
   source replay edits exist, but the non-Copy integration is not compile-ready.
   New `candidate_bundle/order.rs` is part of this work. Finish compiler errors
   before native tests, then test exact source replay, unsupported cut/affine
   refusal, E-primary certification capability and mixed-order routing.
2. **S5 P2/bulk/records:** second P2 source slice passed independent review;
   synthetic tests are authored but not executed. Typed authority/wire modules
   and Epoch-only sidecar are drafted and partly wired into merge publication.
   Restore/cold/export/metadata/controller integration is not complete. Agreed
   versions: Epoch semantics4, CP6 manifest3, scalar4, binary record1; diagnostic
   walk JSON remainsv6. Root's batch Tracker API is ready for integration but
   runtime differential tests remain unexecuted.
3. **P4:** canonical Store and Ready remain unchanged. The approved approach is
   shared immutable pages and bounded delta indexes, preserving exact→dominant
   orthant→minimum-current-live choices. Only `snapshot/shared.rs` and its tests
   are drafted; the existing two-replica path is not yet replaced. Root must
   independently review this code because its author has rotated out of audit.
   Layered physical probe counts may change honestly under semantics4, but not
   logical selected work. Post-P3 allocation failure must keep the old snapshot
   and save the valid merge boundary, never lose retirements or fabricate closure.

The final guarded diagnostic completed at
`TMP/aster-integration-20260930T0740/check`: warm metadata-only
`cargo check --release --tests --locked --offline --config
profile.release.package.rustred-app.opt-level=1 -j8 -p rustred -p rustred-app
-p rustred-python`, using existing heavy/build-0 locks, CPUs0–15, and
`TMP/codex-runtime-discovery.280crc/target-check`. This is an error harvest of
an interrupted integration tree, not a release acceptance or performance test.
It exited101 without an operational stop in59.204s, peak single-child
RSS2434144KiB. Core library compilation reports9 remaining errors; the core test
target reports101 (including stale test constructors and non-Copy assumptions).
App/Python integration was therefore not validated. The guard drained owned
process group4134409 and released its locks; no diagnostic job is left running.
Read `result.json` and `stderr` before retrying; preserve the negative result.
Next restore separate lane ownership, finish compilation, independently review,
execute focused/native tests, and only then schedule matched complete controls.
Do not jump to production instructions from any of these partial results.

The `297be07f` and frozen1b33/62c delivery remains a rollback/reference, not proof
that these newly authorized tracks are implemented. Source A's repeated gains
are retained evidence; Source B and full S5 need their own implementation,
tests and measured integrated results. No master, Vakint, terminal-minimization,
NUMA or unrelated research lane is reopened.

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

## Current shortened-delivery ownership — 2026-09-30 05:45 UTC

This table supersedes the older lane assignments below; those remain historical
evidence, not a claim that the new parallel implementation has passed its gates.

| Work | Status | Responsible lane | Next executable step |
|---|---|---|---|
| Compatible Stage A executable | delivered | `stage_a_release`, root independent review | Stable `931d006c` pushed; owner commands communicated. LC2 remains owner-operated |
| Rolling Epoch controller and lookup replicas | delivered, rejected for this deployment | `parallel_gate_critique` author; root/`stage_a_release` independent review | New prefix76 scout remains18.1% slower than Ready including cold verification; oldest-ready causes severe extra work and was censored. Retain opt-in, not a recommended production engine |
| Original S5 parallel merge / typed-record roadmap | partial; major throughput work unimplemented | root coordination; independent source audit `parallel_gate_critique` | Ledger6 and edge hash-feed batching exist, but P2 bucket planning/P3 remain serial, records remain JSON and tracker updates per-edge. Parallel bucket preparation, typed binary records, bulk tracker feed and P4 layers are not delivered. Reopen as a separate measured engineering slice, not another publication-order knob |
| Fresh CP6 G2 Union | final CLI cold/resume, all4L and both5L controls pass | original `epoch_g2_rescue_impl`; validation `stage_a_release`, root | W50 reindexed4L also passes; all measured scopes retained, no full-five-loop closure claim |
| CP6 rescue and required scope | targeted final optimized regression passes | root narrow fix; independent `stage_a_release` | Real resume/cold, protected-prefix mutation and abandoned-helper distinction pass; do not claim final full-suite rerun |
| Adaptive dispatch | integrated, reviewed, combined pilot negative | author `stage_a_release`, root independent review | Retain opt-in; FIFO26.877s vs adaptive30.068s including cold, one pair |
| CLI/Python policy surfaces and monitoring | final native refresh/restore and corrected dashboard pass | `parallel_gate_critique` / `stage_a_release`, independent root review | Final M2/two inspectors and M4 pass; corrected Python344 PASS/one optional skip; real visual-pilot computing sample remains separately incomplete |
| Dashboard and rate-series stream | requested thresholds/normalized balance/dual-axis plot delivered | `parallel_gate_critique` implementation; independent root review | `adcee12c` pushed and Python tree frozen;350 tests pass/one optional skip; actual FG and labelled synthetic replays inspected |
| True iterative deepening | delivered negative four-loop result; five-loop transfer deferred | `bounded_ordering_pilots` end-to-end; independent `parallel_gate_critique`/root | Actual16→32→90-row saved/amended/cold barriers pass; guarded native+cold30.038s versus16.330s; do not deploy this curriculum |
| Mechanistic input/pivot ordering | P1 isolated confirmations and P3 small5L transfer delivered | `stage_a_release` end-to-end; independent root/`parallel_gate_critique` | Two isolated4L generation+walk+cold pairs improve17.45%/18.73%; walk+cold36.51%/39.23%. Small5L scout24.911→17.670s. Existing67 owners unchanged; no production-wide transfer claim. Earlier coordinate reindex P2 remains negative |
| Epoch matched profiling/decision | E1 delivered negative; E2 not triggered | `parallel_gate_critique` end-to-end; independent root | All completed E1 cells cold-All pass; best accepted Epoch fails the speed gate. Bounded trace shows changed early cut composition/graph edges but not the full expansion's cause |
| Runtime discovery-strategy API and builds | frozen optimizedCLI and native gates delivered with explicit stale-test caveat | root builds/integration; author `stage_a_release`, independent `parallel_gate_critique`/root | Frozen1b33 native; tools62c. Consolidated app1139PASS/1stale-diagnosticFAIL/12ignored; exact public-CLI equivalent10phasesPASS. Corrected Rust test not rebuilt. Full integral-comparator B remains design only |
| Historical closure-count audit | delivered read-only diagnosis; focused experiment pending | `stage_a_release`, root independent checks | Compare four exact historical anchors with G2 Off, retaining all 67 owners; distinguish changed helper bounds from same-input scheduling effects |
| Matched performance and deployment | delivered Ready16/Union recommendation; Epoch and200-core speed unqualified | root; independent author/auditor separation | Final runbook preserves all116 required queries/67 helpers and original owners. Only the owner switches production. No claim that the incomplete original S5 architecture cannot improve |
| NUMA, new CAS/research, unrelated optimizations | deferred | root | Outside approved shortened delivery; reopen only after this delivery |

## Delivery experiment board — authoritative current execution queue

This board separates **prepared** from **executed** and is updated after each
result. Completion of one lane cannot silently close another. Root owns the
shared resource calendar, builds, integration and final independent acceptance;
the three agents own experiments end-to-end. No new engine build per recipe.

| ID | Owner | Experiment / scope | Current state | Dependency / completion receipt |
|---|---|---|---|---|
| B0 | root | Frozenb95 full native core/app correctness | delivered: core2853/0/32; app1129/0/12 | `TMP/codex-runtime-discovery.280crc/root-{core,app}-suite-b95e1465`; strict licensed execution; W50 affinity skips excluded, not performance timing |
| D1 | `bounded_ordering_pilots` | Genuine4L shallow→larger→full checkpoint amendments versus identical90-row one-shot; preserve original58 geometric scope | delivered; all barriers pass, performance negative | Guarded native+cold30.038/16.330s (+83.95%);36,894/19,529 natives. Exact final90 rows/roles and original58 coverage verified independently |
| D2 | `bounded_ordering_pilots` | Bounded5L true staging ifD1 justifies it | deferred, not executed | D1 increases both work and wall time; reopen only for a materially different justified curriculum, not a blind depth sweep |
| B1 | root, validation `bounded_ordering_pilots` | Merge validatedA with ready-batch Epoch; native controller tests and final optimizedCLI | delivered with explicit raw-suite caveat |1139PASS/1stale-diagnosticFAIL/12ignored; corrected assertion committed, not rebuilt. Ten-phase actual public-CLI equivalent PASS; no production-code failure observed |
| E1 | `parallel_gate_critique` | Original58-query4L: Ready, prefix31, prefix76, oldest-ready31; same optimized executable | delivered; reject Epoch deployment | Ready21.939s versus best accepted prefix76 25.911s native+cold; oldest-ready censored121.363s/1.034M native inspections. Completed cells All/all-roots PASS. Concurrent scout, not isolated qualification |
| E2 | `parallel_gate_critique` | Repeat promisingE1 against contemporaneousReady, then finite/hot5L if justified | not triggered; deferred | No winning E1 candidate. Best accepted boundary/refresh shares below preregistered next-cut trigger. Do not open a new blind grid |
| P1 | `stage_a_release` |4L exact-default generation plus two generic runtime source/pivot-discovery recipes | delivered, including two isolated confirmation pairs | Default/sparse103.290/85.268s and103.875/84.421s generation+walk+cold; all58 queries cold-All PASS. Stronger source certification remains unpassed, not included as success |
| P3 | `stage_a_release` | Small5L natural-coordinate default/sparse/shift source-order transfer | delivered; concurrent scout only |24.911/17.670/17.767s generation+walk+cold; all cold-All PASS. Same784-point single required query,14literal programs. No full67-input closure claim |
| P2 | `stage_a_release` |5L natural versus transferred pinch-incidence coordinate priority | delivered; scoped cold checks pass, heuristic not selected | Both artifact certifications hit8220>8192 lowering cap; guarded generation+walk+cold25.763/42.861s. Fewer inspections do not compensate for slower generation/larger coefficients |
| S1 | `bounded_ordering_pilots`, root | Ready physical scaling and final200-core decision | delivered; W50 negative, finiteW50 deferred | W16 native+cold24.742s4L/328.069s finite5L, cold-All PASS. W50 saved/drained at250.373s with1,284,105 native inspections and13,974 pending; no cold-completion claim.200-core capacity is not measured speed |
| L1 | root | Clean push, freeze, tested prepare/resume/monitor instructions and exact launch command | final reports pushed atc676f1fe; owner handoff ready | All67 original payloads/8246routes/183unchanged rows checked;116Required67Aux,Ready16Union,rescue32. Final read-only census and frozen Nix launcher check repeated. No production launch. Current Epoch rejection does not mean the unimplemented S5 roadmap was tested |

Resource calendar: all builds and D1/P2/E1/P1/P3/S1 native pilot groups have
finished and drained. P3 scouts overlapped only S1's disjoint-core cold phase;
the two subsequent counterbalanced P1 pairs had no authorized compiler/pilot
overlap. Root then performed preparation-only input copying in TMP, under the
existing heavy lock. LC2's128–227 reservation remains untouched. The milestone
map and final reports have separate implementation and audit passes; no additional
native experiment is running.
Recorded host contention is never removed through guessed corrections. Every
pilot retained its≤1800s all-in deadline.

Recipe exploration must not be mistaken for the richer mathematical integral
comparatorB: A selects finite source/sector visit order; coordinate reindexing
changes different priorities. Native API tests alone do not complete P1.

## Earlier work ownership and backlog (historical)

| Work | Status | Responsible lane | Next step / reopening condition |
|---|---|---|---|
| Plan, goal, progress bootstrap | delivered | root | Goal active; independently audited documentation milestone |
| G2′ integration | delivered | joint_support_pruning | Merged `9ef5464d`; all14 controls, full/native/W50/CLI and real pause/resume checks pass |
| Rescue and explicit query roles | delivered | joint_support_pruning (integration; original author bounded_helpers_bmw) | Merged `9ef5464d`; all116 required queries retained, tested combined quarantine/replay invariants |
| Independent math/code audit | active | joint_support_pruning and root (current implementation review), checkpoint_final_audit (earlier) | Keep each implementation separate from its reviewer; independently verify runtime/geometry invariants and measurement interpretation |
| Combined G2′ + rescue | delivered | joint_support_pruning + root + independent auditor | All14 controls independently accepted; native/dependency tree exactly matches frozen tested source |
| G2′ component baseline / consolidated deployment | active | root, joint_support_pruning, active_goal_delivery_audit | All eight Ready arms accepted/drained; repeated 24–28% work reduction establishes component baseline; latest user defers restart until validated remaining ideas are consolidated |
| Python production G2′ steering | delivered | joint_support_pruning (author), checkpoint_final_audit (independent review) | `cd52c90d` merged via `9ef5464d`; real pause/resume/cold and post-merge regressions pass |
| Coordinator latency / telemetry | active | checkpoint_final_audit (author), joint_support_pruning (integration), root (review) | Audited `187854b4` included in `29e30a79`; all five exact named regressions passed in the walking focus. Whole-campaign performance gates remain pending |
| Epoch S3–S6 | active | epoch_native_validation (execution/audit), active_goal_delivery_audit (public lifecycle/tooling), joint_support_pruning (G2 composition), root (review/integration) | `29e30a79` full core 2,845/0/32 and full app 1,016/0/12 PASS; W50 omissions excluded. Combined public lifecycle + native/CLI/Python Snapshot controls `9ecad89f` pass release metadata; actual native correctness build started 11:58 and remains live. CP6 research tooling source audited; nine synthetic tests pass, real gates pending. G2-only composition begins isolated source work; rescue duplicate-image/publication gaps recorded. Public native/cold gates, scoped completion reporting, broader merge and rolling work remain open |
| N2 allocation-free geometry | active | active_goal_delivery_audit (isolated author), root/joint_support_pruning (independent review) | Narrow shifted-image endpoint-buffer reuse only; applied focus 59/0/0 and full core 2,845/0/32 PASS on `29e30a79`, including exact new reuse regression. Matched performance gates pending; no measured speedup |
| N1 modular witnesses | deferred | active_goal_delivery_audit (research), root/joint_support_pruning (independent critique) | Sound native seam confirmed; reopen only for capped, exact-path-preserving census of expensive predicates with fully fixed occurring index support, charging support/evaluation/fallback costs |
| N4 coverage-first work | deferred | active_goal_delivery_audit (research), root/joint_support_pruning (independent critique) | Zero initial-orthant hits in finite profile; mixed job-local hits lack Apply/singleton cost attribution. Require a bounded positive opportunity census before changing coefficient work |
| Scheduling / ordering | pending | research/measurement | Compare work volume and censored Ready outcomes |
| Memory / checkpoint / NUMA | active | joint_support_pruning (read-only census), root (independent check) | NUMA snapshot shows negligible cross-socket residency; placement experiment deferred until remote-access evidence. Memory-layout and checkpoint-cost measurement remain pending |
| New algorithms / literature | delivered | closure_acceleration_research; independent critique active_goal_delivery_audit | `docs/research/codex_exact_closure_acceleration_2026-09-29.md`; one-piece coordinate/A/R/D shadow test pending opportunity; no engine implementation recommended |
| Post-G2 critical-path sampling | delivered | joint_support_pruning (analysis), root/active_goal_delivery_audit (independent critique) | Bounded phase-interior analysis completed; throttling prevents unbiased-coverage or precise-benefit claims; one narrow N2 hypothesis registered, no production attachment |
| One-piece residual opportunity probe | active | closure_acceleration_research (source author), active_goal_delivery_audit (independent review) | Isolated `codex/g2-onepiece-shadow` observer only; no planner/coverage changes; uncompiled/unexecuted, actual-snapshot sample required before any pruning implementation |
| Closed-descendant query witnesses | deferred | checkpoint_final_audit; independent critique by joint_support_pruning | Negative census independently reproduced; reopen only on post-G2/rescue evidence of earlier exact closed-descendant coverage |
| Required-scope versus broad-helper dependencies | pending | checkpoint_final_audit + root | Source audit delivered, whole-helper dependency granularity confirmed; measure useful finite-query opportunity before reopening earlier negative approaches |
| I1 L*-helper input variant | deferred | root + independent research/measurement lane | Not rejected; revisit after combined rescue gates and fresh matched net runtime benefit, preserving all 116 required queries |
| Earlier rejected levers | rejected | root | Retain negatives; reopen only with new evidence and a registered falsifier/test |

## Decisions in force

1. The approved plan supersedes stale stop/launch directives. Latest follow-up
   permits a later restart but requires a consolidated validated build first;
   leave LC2 untouched during that work. No G2′-only restart or launch now.
2. Leave pending-growth calculation and name unchanged. The September30 user
   follow-up explicitly changes its colour thresholds in the new dashboard.
3. Resume compatibility is a convenience, not a veto or a migration project.
   Recommend fresh runs when necessary; preserve old campaigns for rollback.
4. A passing internal microbenchmark is not sufficient deployment evidence.
   Use matched controls, two pairs for a switch recommendation, and the
   approved 20% benefit / epoch 1.5x gates.
5. Pilots <=30 minutes including restore, preparation and safe shutdown.
   Separate compilation; report unfinished runs as censored.
6. No extra terminal minimization, numerical-master or Vakint work in this stage.

## Event log

### 2026-09-30 05:45 UTC — final delivery audit and owner launch handoff

- [M] Root rechecked current state rather than relying on the prior summary:
  remote `fable_5_1_parallel` and local HEAD both `c676f1fe`; frozen CLI SHA256
  `73253922552ef341e3e97522d9481a4e369d388e9f8612c4ac92468c3c584a14`,
  executable mode0555, clean Python delivery tree at62c763cd. The final
  destination `campaigns/five-loop-qcd-feynman-d9d10-ready-union-1b33-w16`
  remains absent. LC2 PID360092 is live and untouched; no pilot remains live.
- [M] Repeated the existing read-only `check_prepared.py` against both real
  preparation stdout receipts: PASS,183 unchanged ordered queries,116Required,
  67Auxiliary,67 original owner payload receipts and8246 routes. No native
  launch, checkpoint, run directory or mathematical closure claim. The actual
  earlier guard results remain exit0/null,10.167s and2.157s. The first attempt
  to invoke this checker used bare `python` outside Nix and failed before any
  work; rerunning with the already pinned Python executable passed.
- [M] Verified the owner environment directly, without compiling or starting:
  `nix develop --command python -B examples/python/production_saved_owner_campaign.py --help`
  in `TMP/codex-parallel-campaign.oiPK29/python-delivery-62c763cd` exits0 and
  exposes every option in the final command. The full67 preparation itself
  remains the earlier actual executed receipt, not inferred from this help check.
- [M] Independent reviewer `active_goal_delivery_audit` rechecked the frozen
  identities, full input inventory, final finite5L raw cold reinspection and
  the distinction between delivery and mathematical closure. This audit does
  not replace the earlier independent implementation/measurement reviews.
- [E] Delivery decision is **Ready16 + G2Union**, existing natural-coordinate
  saved programs and preserved query order, explicit roles and bounded rescue.
  Current Epoch settings fail performance gates; original S5 parallel merge,
  typed records and bulk tracker work remain incomplete. No200-core saturation
  claim, no inferred impossibility theorem for Epoch, no full183-query closure,
  and no recommendation to import LC2's checkpoint into the new binding.
  The accepted source-order API remains available for later regeneration, not
  falsely advertised as changing the existing67 programs at launch.
- Final owner commands and rollback are in
  `docs/research/consolidated_campaign_launch_2026-09-30.md`. Completing this
  tool-managed **delivery** objective leaves full scoped closure, unimplemented
  S5 and the explicitly conditional/deferred studies open; it does not mark the
  broader five-loop mathematical objective complete. Only the user launches.

### 2026-09-30 05:30 UTC — confirmations complete; original Epoch S5 remains partial

- [M] P1 isolated counterbalanced pairs complete and independently audited:
  generation+walk+cold default/sparse103.290/85.268s and103.875/84.421s
  (17.45%/18.73% lower); walk+cold21.941/13.930s and21.525/13.080s
  (36.51%/39.23% lower). Every cold-All check passes all58 queries/32 roots.
  Stronger source certificates remain unpassed. See
  `docs/research/runtime_source_strategy_portfolio_2026-09-30.md`.
- [M] P3 natural-coordinate5L source-order scout completes: default/sparse/shift
  24.911/17.670/17.767s generation+walk+cold; all784-point single-query scopes
  cold-verify. Rules4283/698/697 and finite records14 each; this is14 owners,
  not full67 transfer or exact terminal-key equivalence. S1 overlap is disclosed.
- [M] S1 finiteW16 completes in172.858s native+155.211s cold; all760,033 native
  records/one required root pass, no checkpoint mutation. FiniteW50 not run.
  All pilot groups and locks drained before launch-preparation I/O.
- [M] Root actual no-start launch preparation passed in10.167s; re-reading its
  frozen plan passed in2.157s. `TMP/codex-ready-launch-preflight.H9eIUf/` contains
  the actual67 owners/8246routes and183 unchanged query rows, explicit116/67 roles,
  frozen1b33binary/62ctools and Ready16Union/rescue32 policy. No solver, checkpoint,
  run directory or production write. A one-off census initially expected range
  syntax rather than the launcher's canonical CPU list; correcting that exact
  representation made its full read-only census PASS. No launcher change.
- [M] User requests a milestone-by-milestone status. Independent current-source
  audit exposes an important boundary: S3 durableCP6 and S4/S6 lookup/rolling are
  delivered, but originalS5 is only partial. `epoch/merge.rs` still performs
  P2 bucket/antichain/reverse planning and P3 serially; `epoch/records.rs` still
  emits JSON, with per-edge tracker updates. Ledger6 and hash-feed batching do
  not equal parallel bucket preparation, typed binary records or bulk tracker
  publication. The original new layered4x lookup target also is not demonstrated.
- [E] Current Epoch settings fail deployment gates; the entire intended original
  parallel architecture has not been implemented and tested to failure. Finite5L
  Epoch P2 alone was15.03s of417.26s native+cold, so completing that one substep
  cannot be advertised as an assured rescue. Broader S5 remains a principled but
  unmeasured future slice. No200-core saturation or full116-query closure claim.

### 2026-09-30 05:10 UTC — parallel lane control and measured deployment decisions

- [M] `43e8b84d` is pushed on `fable_5_1_parallel`. Native executable remains
  optimized1b33, SHA256 `73253922552ef341e3e97522d9481a4e369d388e9f8612c4ac92468c3c584a14`;
  Python delivery tree remains62c. No engine rebuild for these report updates.
- [M] E1 completed scouts reject Epoch for deployment: Ready native+cold21.939s,
  prefix31 26.556s, prefix76 25.911s. All three cold-All/all-root checks pass.
  Oldest-ready31 was censored after121.363s with1,033,824 native inspections,
  versus31,895 for completed prefix76. It has no cold-closure claim. All owned
  groups drained; recorded setup/collection failures were not hidden or retried
  with reset clocks. Evidence: `TMP/codex-publication-rescue-matrix.2ScWnG/`.
- [M] P1 default/sparse/shift source-order recipes yield98.296/69.471/77.335s
  generation+scoped-walk+cold, with identical58-query bytes/508routes. All three
  scoped cold checks pass. The six optional stronger artifact certifications
  failed (proof/input caps); those costs are explicitly excluded from this
  diagnostic timing. Finite-record sum831 is not a unique-master census.
  Evidence: `TMP/codex-runtime-pivot-portfolio.78ueFJ/`.
- [M] S1 W16 four-loop control closed and cold-verified in24.742s. W50 instead
  expanded to1,892,833 domains/1,284,105 native inspections and13,974 pending
  after250.373s. Root requested an evidence-based cooperative performance stop;
  checkpoint saved, no kill, all owned groups drained. Child-CPU/wall average
  6.724 cores (sampled own activity6.801) is not useful scaling when work rises
  this much. Finite-five-loop W16 remains
  active; its W50 arm is deferred. Evidence: `TMP/codex-s1-scaling.69wxuY/`.
- [D] User correctly requests parallel exploration. Three explicit owners now
  maintain independent experiment outcomes: bounded-ordering owns deepening
  disposition/scaling; stage-a owns runtime pivot confirmation/5L transfer;
  critique independently audits and owns Epoch disposition. P3 scouts run
  alongside S1 on disjoint cores. Final winning P1 timing pairs alone require
  isolation. No lane is dropped because another produces a result.
- [E] Source-order gains require regenerating saved owner programs; they do not
  accelerate walking LC2's existing rules by merely adding a flag. Neither
  these small controls nor200-worker lifecycle tests justify a200-core speed or
  full-five-loop completion forecast. Final deployment instructions remain open.

### 2026-09-30 04:38 UTC — public regression passes; both experiment lanes released

- [M] Frozen1b33 public-CLI equivalent passed all10 phases in4.159s guarded:
  fresh/reopened All/all-root cold PASS (3/3 roots), generation1→2, paired
  summary INCOMPLETE, wrong Union request FAIL with exactly one binding
  violation, unsupported activation and nonempty fresh writes refused.
  Checkpoints remain unchanged across cold/rejection phases. Root and
  `bounded_ordering_pilots` independently reviewed script and actual receipts.
  Evidence: `TMP/codex-epoch-cold-wording.MzOlZL/`.
- [M] Full native suite also confirms all11 added tests, W50-capable checks
  without internal affinity skips, and W200 pool lifecycle capacity. The latter
  is not a200-physical-core performance measurement. The raw full-suite result
  remains1139/1/12; the corrected Rust unit body was not rebuilt/re-executed.
- [D] E1 released onCPU32–47 alongside P1 on64–79. No further Rust rebuild.
  P1's two default parent generations passed (44.169s/34.187s guarded); its
  first stronger certification failed after54.170s on native affine-literal
  consistency proof budget. Preserve the failure and continue the authorized
  scoped diagnostic; do not call it a certified artifact or successful replay.
- [M] Read-only LC2 snapshot around04:34 remains running with6/67 conservative
  closed initial roots,167.95M discovered domains and70.72M local inspections;
  closure snapshot was621s stale. Last-hour measured coordinator preparation
  and ordered commit shares were37.5% and53.3%; these do not establish that a
 200-worker replacement will saturate CPUs. No production state changed.

### 2026-09-30 04:31 UTC — full test outcome and independent lane release

- [M] Full app suite on64 permitted physical CPUs finished233.39s:
  1,139 passed, one failed,12 ignored. The failure is the old expected
  `binding: checkpoint request digest differs` substring; actual rejection
  correctly reports the expanded digest-or-persisted-schedule diagnostic,
  verdict FAIL, request binding false, and exactly one binding violation.
  Root isolated the test (same binary,0.62s body); independent critic confirmed
  the mismatch. Raw failed receipts remain intact.
- [D] Correct only that test's expected text. Do not change the verifier,
  assertions about rejection, or native executable. A full public-CLI equivalent
  will exercise the later resume/cold checks too. The fresh campaign rlibs are
  bitcode-only, so a supposed cheap no-LTO Rust driver would require another
  native compilation; do not link a stale app library or mislabel CLI coverage
  as execution of the corrected monolithic Rust test binary.
- [D] P1 uses Ready, whose tests and SourceA tests passed: release that lane on
  CPU64–79 now instead of blocking it on an unrelated Epoch diagnostic check.
  E1 remains gated on that focused check and independent review. Root retains
  the shared heavy reservation; no compiler or production CPU overlap.

### 2026-09-30 04:25 UTC — both builds pass; native suite released

- [M] App correctness executable built successfully in5016.043s; no stop
  reason, peak single-child RSS96,133,032KiB. The paired guard reports both
  children exit0 and all owned process groups drained. No compilation time
  is counted in solver measurements.
- [D] Released the prepared full app suite to `bounded_ordering_pilots` on
  CPU32–95 under its local validation lock. Root holds the outer heavy
  reservation through that suite and the following concurrent E1/P1 pilots;
  production CPU128–227 remains excluded. No experiment deadline started early.

### 2026-09-30 04:08 UTC — optimized executable frozen; tests still pending

- [M] Consolidated CLI build succeeded: guarded3920.618s, no stop reason,
  build group drained. Frozen executable:
  `TMP/codex-parallel-campaign.oiPK29/candidate-bin/rustred-1b33ad29`,
  SHA256 `73253922552ef341e3e97522d9481a4e369d388e9f8612c4ac92468c3c584a14`,
  126,521,824 bytes, mode0555. Native source remains `1b33ad29`; compilation
  is not included in solver timing. The semantics probe succeeds (ReadyCP5,
  EpochCP6/schema2/semantics3); this alone is not runtime correctness acceptance.
- [M] Clean detached Python/tooling tree frozen at `62c763cd` under
  `TMP/codex-parallel-campaign.oiPK29/python-delivery-62c763cd`.
  There is no Rust/Cargo diff from the native source commit. P1 independently
  checked the executable and its24 static command bindings. Both E1 and P1
  remain unstarted pending the full app correctness gate and resource release.
- [D] If P1's stronger artifact certification encounters the known lowering
  limit, preserve that failure and continue the identical58-query walk as an
  explicitly uncertified trusted-generation diagnostic. Do not equate cold
  graph reinspection with successful original-source artifact certification.

### 2026-09-30 04:07 UTC — parallel lanes and the remaining build dependency

- [M] D1 and P2 ran concurrently on disjoint physical CPU sets and are complete;
  they were not serialized by a mathematical dependency. Their negative results
  and independent audits remain recorded above. LC2 was not touched.
- [D] E1 and P1 will likewise overlap on CPU32–47 and64–79 under a root-owned
  resource reservation. Only decisive confirmation timings are isolated. Their
  present dependency is the consolidated optimized executable and its native
  correctness gate, not a single experiment queue.
- [M] Both frozen1b33 builds remain active after approximately64 minutes, with
  no failure receipt. The CLI has progressed to final optimized binary codegen.
  `bounded_ordering_pilots` now owns execution of the new full app suite on
  CPU32–95 once its binary is ready; this correctness run may overlap the CLI
  compiler on16–31. The wider affinity is deliberate so W50 tests are exercised
  rather than skipped for lack of permitted CPUs. No new engine edits planned.

### 2026-09-30 03:25 UTC — final experiment command preflight

- [M] P1's24 command arrays match the current CLI, including explicit all-root
  All-native verification and correct fresh checkpoint parents. No argument
  change or new run was needed. Certification's existing ingress limits are
  recorded separately; exceeding one cannot be called successful source replay.
- [M] E1's CP6 Python command generator still relied on automatic cold scope.
  Added explicit `--certification-scope all-roots` to that generator and all
  four prepared E1 cold commands, preserving the registered intended scope.
  Independent root review and15 focused tests pass (0.094s); native source
  `1b33ad29` and both ongoing Rust builds are unchanged. Final tooling identity
  will be frozen separately from the native executable identity.
- [D] Registered exactly one conditional larger-cut diagnostic: cut64/window79
  at W16, only if no accepted E1 candidate meets the1.5x gate and measured
  boundary OR snapshot-refresh time is at least10% of traversal. These times
  overlap and are never added. The joint cut/window change can amortize some
  setup, not per-result replay/containment or every delta update. It has its
  own <=1800s clock, no parameter grid, and still needs isolated repeats if
  promising. Plan: `TMP/codex-publication-rescue-matrix.2ScWnG/E2_CONDITIONAL.md`.
- [M] Independent launch review preserves all183 ordered rows,116 Required/
  67 Auxiliary and8246 routes. CPU0–199 is200 physical cores but overlaps72
  LC2 cores, so owner pause/save and full drain remain prerequisites. Draft
  checkpoint interval corrected to3600s; safe-boundary/save-cost policy can
  defer it, so this is a target rather than a wall-clock guarantee.

### 2026-09-30 03:07 UTC — consolidated frontend checks pass; native compilation continues

- [M] Frozen1b33 core/app/PyO3 `cargo check --release --locked --offline --tests`
  passed in71.180s under CPU84–87 with a separate metadata cache. This is not
  execution of the new native controller tests. Both native compilation jobs
  remain active, with no further engine source changes planned.
- [M] Python steering discovery ran356 tests:355 passed, one optional skip,
  guarded35.172s onCPU96–111. The first run was assigned only four CPUs and
  hit an earlier resource-validation diagnostic than a six-worker negative
  test expected; retained that failure and reran on sufficient affinity without
  changing code/assertions. Three pure discovery-descriptor tests also pass.
  Receipts are under `TMP/codex-combined-build.iirdmE/`.
- [M] Independent final D1/P2 conservation audit passed, including identical
  final row/role objects, authenticated amendments, inverse-mapped physical
  coordinates and both full cold scopes. D1 report:
  `docs/research/true_iterative_deepening_2026-09-30.md`. The next exploratory
  E1/P1 resource amendments use distinct local locks beneath root's outer
  reservation; they do not change the registered mathematical workloads.

### 2026-09-30 03:04 UTC — both concurrent pilots finish; consolidated builds start

- [M] D1 genuinely resumed and expanded a saved proof through16,32,90 rows.
  Every explicit all-root/All-native cold barrier passed; final90 row/role
  objects exactly equal one-shot and include all original58 geometries.
  Guarded native+cold30.038061s versus16.329673s (+83.95%); distinct natives
  36,894 versus19,529, final domains52,872 versus35,510. Three preventable
  harness errors (missing parent, invalid resume argv, cold auto-scope) were
  preserved and corrected within the original clock; the negative timing
  above excludes them. Independent audit passed. D2 is explicitly deferred:
  this is not support for applying the same curriculum to five loops.
- [M] P2 both regenerated seven-line libraries pass the same784-point finite
  scope with independent All checks. Reindexing reduces natives2644→1792 but
  increases generation and coefficient bytes. Guarded generation+native+cold
  25.762945→42.861415s; even guarded walk+cold3.600631→3.692896s is not faster.
  Both broader artifact certifications fail at the unchanged8220>8192 lowering
  cap; no certified artifacts are claimed. Independent audit passed.
  Report: `docs/research/five_loop_coordinate_pilot_2026-09-30.md`.
- [M] D1/P2 drained in738.5/604.6s of their original1800s clocks. Root checked
  all20 recorded process groups absent, released its outer flock, and started
  the consolidated native-test/optimized-CLI build pair at03:01:25UTC on
  `1b33ad2956d87f29db6ef9987542f3364f443814`. Receipt:
  `TMP/codex-combined-build.iirdmE/source-1b33ad29-full`. An initial abbreviated
  SHA was refused by the strict wrapper before spawning anything; full SHA
  supplied to a fresh receipt. No scope or code change resulted.
- [D] E1 and P1 are explicitly prepared to overlap on disjoint CPU sets after
  native gates, using this one executable for all variants. Decisive finalist
  pairs remain isolated. No production launch, pause or input edit occurred.

### 2026-09-30 02:46 UTC — parallel exploratory lanes and tested source integration

- [D] User correctly challenged serializing independent experiments. There
  is no algorithmic dependency between D1 and P2. Root acquired one outer
  heavy-job reservation after B0 drained, and assigned disjoint16-core sets
  with separate local locks. Each retains its own1800s all-in deadline.
  Explicit resource-only amendments preserve all mathematical inputs, phase
  caps and cold checks; decisive speed comparisons will be repeated isolated.
- [M] Frozenb95 full app suite:1129 passed,0 failed,12 ignored; body240.45s,
  guarded241.236s, maximum childRSS95,232KiB. Its W50 tests explicitly skipped
  under16-core affinity and are NOT wide-worker acceptance. Core suite was
  already2853/0/32 PASS. Both owned groups drained independently. Receipts:
  `TMP/codex-runtime-discovery.280crc/root-app-suite-b95e1465` and sibling core.
- [M] SourceA merged cleanly as `a4635a93`; its core tree is byte-identical to
  the testedb95 core. The combined app/ready-batch controller still needs its
  own native build and execution. Preserved test binaries in
  `TMP/codex-b95-native.SaxzMz`; no compiler will run during D1/P2.
- [M] Source-order plan preflight completed in2.153s, independently audited:
  all508 sectors conserve original row IDs. Sparse-versus-shift recipes differ
  in341/508 sectors; both differ from input order in every sector. This shows
  the recipes actually select different schedules, NOT that either is faster.
  Evidence: `TMP/codex-runtime-pivot-portfolio.78ueFJ/plan-dump-functional`.

### 2026-09-30 02:18 UTC — full strategy-core suite passes

- [M] Frozenb95 core suite:2853 passed,0 failed,32 intentionally ignored;
  test-body130.14s, guarded wall131.196s, maximum childRSS199,136KiB.
  CPU16–31, strict licensed mode, single test thread. Exact command and output
  are in `TMP/codex-runtime-discovery.280crc/root-core-suite-b95e1465`;
  exit0/reasonnull and independentPG1463813 drain confirmed.
- [M] New executed tests cover invalid recipes, native Symbolica feature counts,
  deterministic callback ties, row conservation, unchanged explicit defaults,
  original source-ID provenance, and both symbolic/shared-numerical paths.
  This is correctness evidence, not a pivot-performance result. App compilation
  remains active; no application-suite PASS or source merge is claimed yet.

### 2026-09-30 02:16 UTC — native core execution and frozen three-recipe portfolio

- [M] Root started the already-builtb95 core executable's full suite at02:15,
  CPU16–31/build1, strict licensed mode and one test thread, while app codegen
  continues onCPU0–15. This overlaps correctness work, not solver timing.
  Receipt: `TMP/codex-runtime-discovery.280crc/root-core-suite-b95e1465`.
  No test-suite PASS is claimed until its process and result complete.
- [M] Pivot portfolio prepared at `TMP/codex-runtime-pivot-portfolio.78ueFJ`,
  matrix`32a27c64…`: default, sparse/coefficient, and uniform shift-cost source
  priorities; fixed active-first sector scheduling. All regenerate both full
  common-family root downsets, stage16 fresh owners with508 unchanged routes,
  and test all58 unchanged Required rows. Fixed Ready+Union is the primary
  walker, so this lane does not depend on E1 selecting an Epoch policy.
- [D] Candidate loading/cold graph inspection is not original-IBP source
  replay. The public certification command attempts replay plus stronger
  downset coverage; preserve typed failures separately, and never present an
  uncertified trusted-generation diagnostic as qualified source replay.
  Existing production67 programs remain unchanged by these experiments.
- [M] `git merge-tree` found a clean source integration of b95 with current
  c672; working files and branches were not merged yet. Root reviewed the
  corrected paired-build receipt reader and its finish/cancel/partial-write
  mock receipts; the previous early-lock-release concern is resolved. Actual
  paired compilation remains behind B0 and D1.

### 2026-09-30 02:07 UTC — pivot lane restored as an explicit experiment gate

- [D] User correctly required the implemented runtime strategies to be tested,
  not displaced by Epoch and deepening. Three dedicated experiment lanes now
  own E/D/P above. Root accepted build ownership from`stage_a_release`, including
  the ongoing frozenb95 process and its untouched receipts. That agent now owns
  the4L source-strategy portfolio and separate5L coordinate pilot.
- [D] Added the authoritative execution board above with every experiment's
  owner, execution state, dependency and receipt requirement. New runtime recipe
  variants reuse one final optimized binary; sourceA's native tests are not
  substituted for actual portfolio measurements. Negative or censored results
  remain explicit and cannot be quietly dropped at final delivery.

### 2026-09-30 02:03 UTC — separate end-to-end experiment ownership

- [D] Following the user's delegation request, `bounded_ordering_pilots` owns
  true deepening from preparation through execution and results;
  `parallel_gate_critique` owns the separate matched Epoch profiling/decision
  lane. `stage_a_release` owns compilation/native validation and independent
  review of Epoch. Root coordinates resources, integration and final acceptance.
  The Epoch author cannot approve its own mathematical or measurement result.
- [M] Prepared200-core launch preflight confirms unchanged183 ordered query
  objects, explicit116 Required/67 Auxiliary roles,67 payloads and8246 routes
  (8179 transported plus67 identity). CPU0–199 is200 physical cores, not SMT.
  Evidence: `TMP/codex-launch200-preflight.CXIBRy`. No launch command is bound
  to an untested binary, and no production input has been changed.
- [D] Two independent consolidated compilation caches can run concurrently
  under one outer heavy reservation, with disjointCPU0–15/16–31 and guarded
  400GiB admission/250GiB live headroom. Preparation and mock drain tests exist
  in `TMP/codex-combined-build.iirdmE`; root found a partial-JSON-read cleanup
  issue for correction before approval. Neither build has started. The current
  b95 build and then the true4L experiment retain priority.

### 2026-09-30 02:02 UTC — explicit deepening status and next measurement

- [D] Root explicitly told the user that true staged deepening has not run at
  either four or five loops. The earlier35.244s upfront-helper test is not a
  staged test; its28.89% regression must not be used as such a result.
- [D] After the currentb95 native build and suites drain, the genuine four-loop
  staged-versus-one-shot comparison is the next heavy job, before any further
  build. It uses the already frozen3428 executable. A favorable result warrants
  a bounded five-loop staging study; a negative result does not automatically
  justify expanding that search. The separately prepared5L ordering diagnostic
  is not a deepening experiment.
- [E] Root explained the measured Epoch regression: the finite5L traversal is
  188.378s versus Ready76.891s despite less work; prefix/window blocking, replica
  availability and roughly47s in serialP1/P2/P3 are distinct constraints. The
  new policy addresses publication blocking, not all those constraints. Near
  saturation of200 physical cores is not currently supported by measurements.

### 2026-09-30 01:57 UTC — independent publication audit and launch namespace

- [M] `stage_a_release` independently audited the native`c672af8f` slice with
  no blocking finding. Nonprefix cuts retain authenticated held sequence
  inventory; P1/P2 still validate provenance and stale observations; P3 and
  cold replay use actual publication epochs. Saved holes replay before fresh
  admission. Protected roots and quarantine rules remain unchanged. This is
  source review plus the earlier typecheck, not executed native acceptance.
- [M] Launch preflight resolved a misleading terminal listing: without
  `XDG_RUNTIME_DIR`, Zellij reported an exited session; using LC2's observed
  `/run/user/1125` runtime, the `rustred` session is active with
  `five_loop_vacuum` and `fable_5_1` tabs. `codex_astra` does not exist yet.
  Only the owner will create/use it. No session or campaign was changed.

### 2026-09-30 01:53 UTC — active delivery goal and native typecheck

- [D] User replaced the tool-managed goal: settle the remaining deepening and
  pivot studies, either demonstrate an Epoch win or decide against deploying
  it, then deliver a tested long-standing-campaign setup and its exact launch
  command. Root verified goal status is now active. This replaces the earlier
  paused broad-goal state; no duplicate goal was created.
- [M] Independent `stage_a_release` metadata gate on source`c672af8f` passed
  core/app/Python release test typechecking in111.190s, usingCPU16–19/j1.
  Maximum childRSS2,433,784KiB; minimum available RAM694,123,753,472B.
  Receipt: `TMP/codex-runtime-discovery.280crc/typecheck-c672af8f-ready`.
  Process group drained and build lock released. This does not execute tests.
  The separate frozenb95 API native build continues unchanged onCPU0–15.
- [D] True-deepening/pivot preparation incorporates independent caveats before
  execution: total deadline includes lock admission/drain; development means
  disjoint CPUs and observed foreign load, not pausingLC2; the proposed5L
  coordinate order transfers an11-line pinch heuristic to a7-line diagnostic,
  which cannot directly contain that interface. No positive transfer is assumed.
- [D] `parallel_gate_critique` now prepares a read-only200-physical-core launch
  input/resource/rollback preflight independently of root's steering changes.
  User has confirmed they will pauseLC2 for the eventual launch. Production is
  untouched; current snapshots and prepared commands are not launched jobs.

### 2026-09-30 01:46 UTC — ready-publication source frozen for native checks

- [M] Native author `parallel_gate_critique` delivered opt-in oldest-ready
  publication, runtime cut/window, bounded message draining, narrow wakeups,
  and invocation-local wait/refresh diagnostics. Exact P1/P2/P3 authority is
  unchanged. Source-format/diff checks pass; no native execution is claimed.
- [M] Root independently reviewed sequence/v0 validation, reserved-parent
  retirement, durable inventory, and the delayed-earliest-result regression.
  The test's budget3 means two inspectors, so holdingseq0 leaves exactly one
  FIFO inspector and makes its first nonprefix cut deterministic. An initial
  concern about that assumption was resolved by inspecting the actual budget.
  Tests also cover saved holes/inline replay and a synthetic199-inspector plus
  coordinator lifecycle; that is not a200-physical-core performance test.
- [M] Review tightened cold request/scalar consistency without tying historical
  diagnostic-only batch sizes to the verifier's process environment. Explicit
  new cut/window and policy mismatches are rejected; legacy omitted diagnostic
  values remain independently verifiable. Native tests still need execution.
- [M] `bounded_ordering_pilots` independently audited root's CLI/Python layer
  and reran five new steering and two new receipt tests PASS. Root independently
  reran all nine prepared-deepening input tests PASS. No native experiment has
  run during the existing correctness build; LC2 remains untouched.
- [D] Commit this source for the bounded metadata gate in the inactive worktree.
  Do not describe the commit as a release/launch milestone before native tests,
  optimized build, cold controls and measured performance decisions complete.

### 2026-09-30 01:36 UTC — runtime steering and real deepening fixtures prepared

- [M] Root implemented CLI/Python forwarding, durable steering replay and
  mismatch rejection for `--epoch-publication-order`, `--epoch-cut-size`, and
  `--epoch-window`. Omitted controls leave historical steering/native arguments
  unchanged; custom controls require durable rolling Epoch. Full Python suite:
  356 run, PASS with one existing optional skip,30.982s. Focused new steering:
  five PASS. Command: `TMPDIR=/common/dev/rustred/TMP python -m unittest discover
  -s examples/python -p 'test_*.py' -q`, using the pinned Nix Python. Mocked
  launch tests are not native policy or closure evidence.
- [M] Updated small CP6 measurement contracts to bind the selected publication
  policy, explicit cut/window and inline-W1 clamping. Fourteen tests PASS in
  0.077s. Default schedule retains `oldest_sequence_prefix`; new opt-in report
  name is `oldest_ready_sequences`. Author is still implementing native held-job
  publication/replay tests; source is not frozen or compiled yet.
- [M] True-deepening fixture prepared at `TMP/codex-true-deepening.l9fXPO`:
  16 narrow shells (97 integer points), then16 larger shells (1456 points), then
  all58 original four-loop rows. Both arms have the same final90 rows and roles
  (16 required/74 auxiliary). Each staged transition requires cold-All success;
  final explicit coverage of all58 unchanged original geometries is mandatory.
  Nine synthetic input/receipt tests pass; no solver run yet. This deliberately
  tests curriculum/reuse, not production-role migration or pure rank-only cost:
  one-shot/staged protected admission histories differ.
- [E] Five-loop candidate uses native pinch-class incidence to prioritize a
  repeatedly shared interface (`111011111101010`), with inverse coordinate tie
  mapping. Proposed seven-line diagnostic root `101101100101000` has128 possible
  supports,14 routed nonzero labels and four owner classes. Natural/reindexed
  inputs and784-point entry probe are prepared, not executed. No full67-owner
  regenerated library or five-loop winning order is claimed.
- [D] Existingb95 native API build remains untouched. After source freeze,
  `stage_a_release` may run a bounded120s metadata-only check on disjoint
  CPUs16–19 and the inactive author cache; native build source/cache stay
  unchanged. Heavy pilots remain serialized after build drain.

### 2026-09-30 01:28 UTC — deployment reopened; bounded Epoch rescue

- [D] User requires continued work until ordering/deepening/Epoch questions and
  a definitive build/setup are settled. Earlier dashboard-only final was
  premature. User confirmed they will pause LC2 before the200-physical-core
  launch; no production action authorized to agents. Host inventory256 physical,
  384 logical; LC2 reserves100 physical, leaving156 disjoint during development.
- [M] Read-only scheduling audit found a real lost-overlap mechanism: completed
  results occupy the bounded window until all oldest16 results are available
  and serial P1/P2/P3 retires them. The inspector pool already shares a work
  queue; simply adding work stealing does not fix publication blocking. Two
  leased lookup replicas can additionally gate refill. Existing inspect-phase
  timer conflates these waits and must not be called pureCAS cost.
- [D] `parallel_gate_critique` now authors a narrow opt-in oldest-ready policy,
  cut/window controls, bounded message draining and causally useful diagnostics.
  Root owns CLI/Python plumbing and independent source review;
  `stage_a_release` independently audits after its existing native build.
  No changes to exact publication authority. Required counterexample: holdseq0
  while16 later jobs finish; ready policy should publish/refill, default should
  retain its prefix semantics. Register tests and falsifier before pilot runs.
- [M] Real staged-deepening seam exists in public `--resume --amend-queries`;
  amendments are Auxiliary. `bounded_ordering_pilots` prepares a diagnostic
  with identical final roles/union on both arms and cold-All validation of all
  original58 domains. This is not a production required-role migration. The
  previous74-row upfront prefix (+28.89% wall) remains a different negative.
- [M] Isolatedb95 API correctness build continues unchanged from01:07, estimate
  80–110 minutes total plus suites. No heavy pilot overlaps it. Frozen3428
  remains only an experimental baseline, not a200-core recommendation.

### 2026-09-30 01:07 UTC — W50 correctness passes; isolated API native build starts

- [M] W50/CPUs32–81 reindexed4L passes:58 required rows,32 admitted roots,
 19,597 reinspections and36,161 graph domains cold-closed; no uncovered/errors/
  violations. Native7.985s+cold8.155s, peak RSS393,580,544 bytes. CP6 accepted,
  summary/native exit4 remains distinct from cold authority. Both measurement
  author and independent auditor verified receipts and resource width. This is
  not a matched W50 speed result or a five-loop width extrapolation.
- [M] Reverse-pair plus W50 pilot ends at261.147s within900s; all nine groups
  drained. Evidence: `TMP/codex-sector-reindex.D5Yawx/reverse-replication/
  results.json`. Together with the preceding12 drained five-loop groups, this
  ends the scheduled performance runs. No additional tuning run is admitted.
- [M] Explicit heavy/build handoff completed. `stage_a_release` now compiles
  cleanb95e1465 in the dedicated validation tree, private `target-native`,
  CPUs0–15/j8, locked offline release core/app tests with app opt-level1.
  Receipt: `TMP/codex-runtime-discovery.280crc/native-build-b95e1465`, process
  group1050516. Starting headroom742.46GB. Estimated additional80–110 minutes
  plus roughly6 minutes for suites is disclosed as a correctness-build estimate,
  not a delivery/performance-binary claim. This extends beyond the earlier
  planning window for the subsequently requested API; no validation is skipped.
- [D] Frozen3428 remains the tested campaign candidate and Pythonadcee12c its
  current dashboard. Native API work remains isolated until runtime gates pass.
  Independent measurement review supports only an explicitly experimental,
  disjoint/resource-capped alongside launch; the1.5x/faster-replacement gate
  has failed on current evidence. No user campaign is started or stopped.

### 2026-09-30 01:04 UTC — reindexed four-loop replication: parity, not speedup

- [M] Same frozen inputs/binary, reversed order: FIFO9.464s native+7.155s
  cold=16.619s; Ready7.984s+8.160s=16.144s. Both cold-All58/32-root checks
  pass; Ready's separately timed full audit also passes (4.150s).
  The advantage flips from the first pair. Two-pair means are approximately
 16.729s Epoch/16.937s Ready (1.23% apart), so near parity rather than a robust
  architecture speedup. The mechanistic **input-library** improvement remains
  useful four-loop evidence, not a demonstrated five-loop ordering choice.
- [M] W50 correctness-only arm admitted with744.194s remaining in the same
 900s pilot, exceeding its preregistered340s stop/cold/drain allowance. Same
 58-query scope, only worker/CPU/verification width changes; no matched W50
  speedup claim. Heavy resources transfer to the isolated API build after
  its complete verification and drain.

### 2026-09-30 01:01 UTC — both representative five-loop comparisons complete

- [M] Hot Epoch cold-All PASS: all731,117 native inspections,905,522 domains,
 1/1 required query/root, zero uncovered/errors/violations; CP6 accepted.
  Native300.662s+cold149.212s=449.874s versus Ready373.341s,20.50% slower.
  Native CPU496.881s versus580.315s and smaller work counts do not imply faster
  wall time. The independent auditor checked raw receipts and exact query scope.
- [M] `TMP/codex-final-monitor-matrix.LJLGfi/final-five-loop-results.json`
  consolidates all four arms, including the Ready finite retry and audit censor.
  All12 owned groups drained; subsequent hot full event audits are NOT RUN and
  Epoch summary audits remain honestly INCOMPLETE. No full-pipeline/source-IBP
  replay claim is substituted for the actual scoped cold-All result.
- [D] Separate reversed reindex pair is now running within its900s pilot.
  Conditional W50 four-loop correctness follows only if the complete allowance
  fits. Then the isolated discovery-API native build starts without another
  tuning sweep. LC2 and its current production checkpoint remain unchanged.

### 2026-09-30 00:52 UTC — hot control baseline verified; dashboard frozen

- [M] Hot Ready, original W12/CPUs32–43:195.124s whole native command plus
 178.217s full cold-All=373.341s. All798,399 native inspections and1,026,779
  graph domains pass for1/1 required query/root, zero uncovered/errors.
  The independent auditor confirmed the registered query digest and scope.
  Full Python event audit is explicitly NOT RUN, not silently passed.
  Hot Epoch now runs with the same input and verification allowance.
- [M] Monitoring milestone `adcee12c` and results/doc follow-up `ee7cfbfe` are
  pushed to `fable_5_1_parallel`. New frozen Python worktree:
  `TMP/codex-parallel-campaign.oiPK29/python-delivery-adcee12c`; clean status
  and production launcher `--help` checked. Native3428/source tree unchanged.
  Only pre-existing unrelated FeynKit/reference work remains outside task edits.
- [M] Root additionally reran the isolated discovery descriptor's three pure
  Python tests (PASS). This is not native API execution; that build remains
  queued behind the final matched pilots. No extra CAS work or feature sweep.

### 2026-09-30 00:44 UTC — verified finite-five-loop pair: negative speed result

- [M] Same3428 binary, saved67-owner library/8,179 routes, single finite
  required query, W16/CPUs32–47: Ready native176.082s+cold-All156.211s=
 332.293s; Epoch FIFO278.050s+139.214s=417.264s,25.57% slower. Both primary
  cold-All checks pass1/1 query/root with zero uncovered/errors/violations.
  Epoch CP6 acceptance passes. Ready's separate failed cold/audit overheads
  remain documented; they are not charged only to Ready in the comparison.
- [M] Epoch native exit4 is its explicit checkpoint-only/incomplete summary,
  not a native closure claim. Its recorded queue drains; independent cold-All
  closes916,438 domains and rechecks all745,954 native inspections. Ready has
 966,681 domains/759,802 inspections. Epoch therefore does slightly less work
  and uses less native CPU (491.115s versus571.051s), but longer wall time:
  preparation83.368s/traversal188.378s versus82.984s/76.891s. Do not substitute
  CPU savings or overlapping diagnostic phase sums for an end-to-end speedup.
- [M] Independent `parallel_gate_critique` raw-receipt review confirms the
  common scope and timings. Epoch's tiny summary audit is explicitly
  INCOMPLETE, not Ready's missing full event proof. Remaining hot W12 pair
  starts next; no all-five-loop completion prediction follows from this control.
- [D] One final launch-width correctness smoke is conditionally added after
  reversed reindex replication: same58-query library, Epoch W50/CPUs32–81,
  full cold-All, no cross-width speed claim. Its registered stop/cold/drain
  allowance must fit inside that pilot's original900s inclusive deadline;
  otherwise omit it. This reopens W50 only for final launch-width validation,
  not a scaling sweep. Heavy build handoff follows immediately afterward.
- [M] Independent frozen-source audit corrects a potential width inference:
  publication cut remains16 at both W16 and W50; only the rolling window grows
 31→65 and inspector reservation15→49. Therefore more workers do not directly
  reduce the46,624 merge cuts. W16's poor wall time does not measure W50, but
  ideal linear inspector scaling is unsupported. No extra finite-W50 timing
  pair is added; its performance remains unknown rather than presumed rescued.

### 2026-09-30 00:38 UTC — requested dashboard thresholds/plot independently checked

- [M] `parallel_gate_critique` implemented the orthogonal monitoring slice;
  root independently reviewed the paired-window calculation, exact boundary
  colours and plot data/units, reran27 dashboard tests (PASS), and viewed the
  actual80-column FG replay plus synthetic boundary/signed dual-axis images.
  Author's full suite:351 tests,350 PASS/one existing optional skip in31.641s;
  focused58/58 PASS. No native code, pending arithmetic or production mutation.
- [M] `Closure balance` uses `(D-C)/(D+C)` from common window endpoints; zero
  denominator and invalid/missing/reset observations stay unknown. Raw net
  domains/s remains in the stream and uses the plot's RIGHT axis; unresolved
  total uses LEFT. No interpolation or invented zeroes. Active observed cores
  stay beside reserved cores; unknown observations are neutral. At previously
  unspecified exact boundaries,75% CPU is green, pending1 and balance0.5 are
  yellow, and zero pending/balance is green.
- [M] Evidence: `TMP/codex-dashboard-balance.VBpyjw/self-review.json` and ten
  inspected PNGs. These are saved actual-FG terminal replays and explicitly
  labelled synthetic stress cases, not a new solver run or OS screenshot.
  The original visual pilot's unobserved positive-computing sample remains
  incomplete; this rendering update does not change that receipt.
- [D] Root updated the public driver documentation. The agent now returns to
  independent read-only measurement review; source implementation and final
  mathematical/performance interpretation remain separately reviewed.

### 2026-09-30 00:36 UTC — finite control cold pass; secondary audit censored

- [M] Finite five-loop Ready cold-All retry passes in156.211s: all759,802
  native/partial/G2 reinspections,966,681 domains,1/1 required root/query and
  zero uncovered/errors. Successful native+cold boundary is332.293s. This is
  one finite control, not the complete five-loop production request.
- [M] The separate Python full-result/event audit times out at301.255s with
  exit124 and no report. Native+cold remain valid; full pipeline acceptance
  is **incomplete**. Native+failed cold+successful cold+audit incurred784.779s;
  whole arm through reporting1001.127s remains below1800s. Evidence:
  `TMP/codex-final-monitor-matrix.LJLGfi/finite-ready-after-retry.json` and
  `FINAL_FIVE_LOOP_STATUS.md`. All owned groups drained.
- [D] No further full Python-audit retry. Continue finite FIFO and the hot
  Ready/FIFO pair with unchanged native scope and cold-All, the registered300s
  verification allowance and inclusive1800s per-arm bound. Mark omitted full
  Python audits NOT RUN. Retain Epoch's inexpensive summary audit solely for
  its existing CP6 acceptance check; it is not the missing full-result proof.
  Python additionally checks event-accounting/coverage/publication consistency;
  cold-All re-derives native successors and closes the recorded graph. Neither
  is original algebraic-IBP replay.
- [D] After those controls, authorize one separately bounded900s reverse-order
  replication of the promising reindexed four-loop pair, using the same saved
  inputs and binary, no regeneration. Then hand heavy resources to the isolated
  discovery-API native build. No W50/new ordering sweep is added. Production
  and frozen launch source/executable remain untouched.
- [M] Dashboard author reports58 focused tests passing; actual-FG replay at
 80/100/140 columns and explicit synthetic colour boundaries are under visual
  review. An affinity-dependent full-suite failure under only2 allowed CPUs is
  not a reason to change the unrelated supervisor test; rerun with sufficient
  affinity. Root independently audits the completed slice before publication.

### 2026-09-30 00:27 UTC — dashboard follow-up delegated; finite cold retry bounded

- [D] User adds utilization/pending colour thresholds, normalized closure
  balance and a dual-axis unresolved-count/net-rate plot. Added this directive
  to `SHORTENED_PLAN.md`; pending arithmetic/name stay unchanged. Original
  dashboard thread and a new spawn hit the agent-thread limit, so the existing
  `parallel_gate_critique` slot is reassigned to implementation/self-review.
  Root independently reviews afterward and stays on native measurements.
  Existing actual FG captures suffice for visual replay; no native pilot is added.
- [M] Finite five-loop Ready native command completes in176.082s,966,681 domains,
 759,802 native inspections and6.24GB peak RSS. Preparation82.984s and traversal
 76.891s are reported separately. Its original150s cold-All allowance expires:
  exit124/151.230s, no cold report or pass. Cold preparation84.646s and graph
  checking28.546s left insufficient time for the full reinspection. This is a
  failed verification-time budget, not a completed comparison or closure failure.
- [D] Authorized one fresh cold-only300s retry plus60s drain, preserving the
  failed receipt and the original native result. At admission497s of the
  original1800s whole-arm budget had elapsed, enough for retry/audit/drain.
  Future five-loop plans explicitly register300s verification allowances with
  the same absolute1800s whole-arm bound. No native/input/coverage weakening;
  successful-cold timing and cumulative failed-attempt overhead remain separate.

### 2026-09-30 00:19 UTC — mechanistic reindex completes, promising single pair

- [M] The input-only reindex pilot finished within438.03s of its900s inclusive
  deadline. Both regenerated parents, all16 newly selected owner payloads,
 508 admitted routes (492 transported/native-verified and16 identity routes)
  and the inverse-identical58 physical
  query rows were used. All eight owned groups drained; no compiler/engine or
  production changes. Evidence: `TMP/codex-sector-reindex.D5Yawx/results.json`.
- [M] Ready:8.564s native+9.166s cold=17.730s,50,039 domains/16,082 inspections.
  Epoch FIFO:9.665s+7.174s=16.839s,36,013 domains/19,568 inspections. Both
  cold-All32/32 admitted roots pass for all58 required rows; Epoch CP6 accepted.
  Both native loaders report28 aggregate saved terminals and unchanged finite
  search policy, but exact terminal-key equality is not asserted.
- [E] This is promising **input-library** improvement versus the original
  repeated controls, and one near-parity/slightly favorable Epoch pair on the
  same new library. It does not isolate sector comparison from representation,
  coefficient variable registration or fresh regeneration, and does not prove
  a1.5x parallel throughput gain. No production recommendation follows yet.
- [D] The preregistered repeat's worst-case stop/cold/drain allowance no longer
  fit the remaining original pilot budget, so it was not squeezed in or extended.
  A reversed replication is pending separately. Representative original finite
  and hot five-loop pairs run first; finite Ready is now active. Optional
  repetitions require actual evidence, not a blind parameter sweep.

### 2026-09-30 00:12 UTC — discovery API frozen after source and metadata gates

- [M] Corrected metadata check PASS for core/app/Python test targets in28.170s
  (Cargo25.55s), exit0/no guard stop. Receipt:
  `TMP/codex-runtime-discovery.280crc/typecheck-a-corrected`. Root independently
  read it. Earlier77.188s failed receipt is preserved. Independent source audit
  also passes the added partial K3 resume and same-shape strategy-change tests.
- [M] Isolated commits `e6d1b103` (core) and `b95e1465` (app/CLI/Python,
  persistence, tests and docs) are frozen on `codex/runtime-discovery-strategy`.
  They are **not merged into the delivery build** and native execution remains
  pending. The owned inactive `TMP/codex-parallel-validation.RPJKV5/repo` now
  holds exactb95/clean Symbolicaef0 for the approved faithful native test build;
  original cache/receipts and the separate clean3428 campaign tree are preserved.
- [M] Input-only coordinate-reindex pilot has generated its first parent in
 27.169s guard/26.026s core:314 sectors,19,984 rules,386 aggregate finite
  residuals,47.43MB bundle. This is generation only, not scoped closure or a
  matched speedup. Second parent and unchanged-scope walk verification follow
  within the single900s inclusive deadline. No engine rebuild is involved.
- [M] Documentation milestone `b1005c9b` pushed: complete individual4L results,
  negative mixed-pivot/prefix probes and current draft launch identities. The
  dashboard's matching Python tree remains frozen at69f86bd6, native at3428.

### 2026-09-30 00:08 UTC — all individual four-loop cold controls pass

- [M] Frozen3428, W6 CPUs32–37, same candidate/query inputs within each pair:
  all eight cold-All controls PASS (FG248, BMW268, H628, X656); all four Epoch
  CP6 receipts accepted and all24 owned groups drained. Native plus cold seconds:

  | Control | Ready | Epoch FIFO | Interpretation |
  | --- | ---: | ---: | --- |
  | FG | 16.020 +11.158 =27.178 | 16.004 +8.157 =24.161 | One pair, about11.1% lower |
  | BMW | 26.221 +18.164 =44.385 | 27.224 +13.179 =40.403 | One pair, about9.0% lower |
  | H | 16.804 +11.162 =27.966 | 16.261 +10.154 =26.415 | One pair, about5.5% lower |
  | X | 32.824 +23.176 =56.000 | 48.649 +22.164 =70.813 | One pair, about26.45% slower |

- [M] Evidence: `TMP/codex-final-monitor-matrix.LJLGfi/
  final-individual-results.json`. Separate Python summary audits are not folded
  into these timings; Epoch's checkpoint-only summary is honestly incomplete
  despite successful raw full reinspection. These mixed results do not replace
  the unfavorable repeated combined control or establish a1.5x speed gate.
- [D] Corrected strategy metadata now has the resource slot, followed by the
  bounded full-coordinate reindex and one finite/one hot five-loop pair. Optional
  repeats/W50 wait for evidence. Faithful strategy native-test build follows
  drained measurements; no source mutation or compilation contaminates timings.

### 2026-09-30 00:00 UTC — pushed visual milestone; isolated API testing boundary

- [M] Visual fixes, prior final native lifecycle/performance receipts, and
  progress updates committed/pushed as `69f86bd6`. Corrected Python source
  is frozen separately at `TMP/codex-parallel-campaign.oiPK29/
  python-delivery-69f86bd6`; native3428 remains unchanged. The launch draft
  now binds these actual identities but explicitly remains NOT QUALIFIED.
- [M] Independent performance review found no justified remaining exposed
  scheduling knob: about2.306s of the2.800s repeated combined gap is native;
  removing all measured small-cut administrative phases alone cannot eliminate
  it. Earlier inspector8/cut1 and AllMiss controls do not support deployment.
  Do not reopen a blind scheduling sweep or treat unused Epoch transfer
  lookahead as a tunable rolling window.
- [M] Isolated discovery-strategy source review found no provenance or integral
  order bug. Original prepared-source ordinals survive both symbolic/numerical
  search; finite schedules do not change mathematical descent. Initial metadata
  check failed on one test-only import in77.188s; corrected and retained. Added
  a real partial K3 checkpoint test after review identified that an all-complete
  resume alone bypassed the pending-plan mapping. Native execution remains open.
- [D] One corrected metadata retry is authorized after individual controls;
  then freeze audited source and prepare faithful core/app opt1 native tests.
  Historical codegen takes roughly80–110min; a Cargo test filter does not avoid
  monolithic codegen. Do not substitute an old-library wrapper for integration
  tests or repeat the previously failed opt0 build. Full native build/suites
  wait for bounded4L/reindex/finite5L/hot5L controls to drain. This isolated API
  does not enter the stable parallel build until executed tests pass.

### 2026-09-29 23:55 UTC — visual audit delivered; pivot/prefix negatives retained

- [M] Real FG248 dashboard audit covers five PTYs, resize and NO_COLOR. Narrow
  fixes improve root-count contrast,80-column rate names/windows, directory-only
  checkpoint notices and short-run plot axes. Author inspected all frames; root
  independently viewed corrected80/140 and actual-rate PNGs. Unknown worker and
  publication fields are absent native telemetry, not parser losses. Full Python
 345 tests:344 PASS/one existing optional skip in30.160s; root independently
  reran21 dashboard tests PASS using workspace-localTMP.
- [M] Evidence/report: `TMP/codex-dashboard-live.09MDmS/VISUAL_AUDIT.md`.
  Images are pyte/Chromium exports of captured PTY state; faint-intensity limits
  are disclosed. Post-fix images replay data, not a second solver run. Actual
  visual pilot cold-All248 passes but computing-positive observation stays
  INCOMPLETE. No lifecycle or performance claim is inferred from a screenshot.
- [M] Fresh-natural banana: Ready24.839s versus Epoch24.347s including cold-All,
  both pass, one pair only. Mixed-line-first explodes on both: Epoch stopped at
 129.177s/1,445,109 domains; Ready censored at60.672s/215,330 domains. Neither
  completes or cold-verifies. Fewer generated rules do not imply less traversal.
- [M] Finite-prefix proxy completes in91.4s inclusive pilot time. Native13.065s
  plus cold22.179s=35.244s,48 admitted roots/all74 query rows cold-All accepted,
  including original58 required rows. Domains49,174 versus51,139 baseline, but
  primary wall28.89% worse than contemporary FIFOmean. This rejects only the
  cheap early-helper prefix, not true staged rank deepening. All jobs drained.
- [D] Continue required individual controls and one already prepared true
  sector-priority coordinate reindex, regenerating all payloads and remapping
  every query/route. Independent reviewer audits the isolated discovery-strategy
  draft; metadata alone is not executed test evidence or a launch gate.

### 2026-09-29 23:43 UTC — actual four-loop dashboard visual findings

- [M] Disposable FG248 run completed with raw cold-All248 PASS and no checkpoint
  mutation. The encompassing M4 observation receipt is honestly INCOMPLETE
  because a positive computing-worker sample was not observed; this is not
  relabelled as all lifecycle predicates passing. The earlier final native M4
  pass remains separate. Captured about2.1MB of genuine PTY output from five
  cleanly exiting monitors, including100→80→140→100 resizing and NO_COLOR.
- [M] Dashboard author and root both inspected rasterized captured-live PTY
  frames at80 and140 columns (`TMP/codex-dashboard-live.09MDmS/`). Layout,
  checkpoint notices and resize cleanup held, but ordinary ANSI blue gave poor
  contrast and the80-column layout clipped the two rate names. Approved narrow
  bright-blue/column-allocation/window-wording fixes, with regression tests and
  captured-frame replay. This does not alter native telemetry or arithmetic.
- [D] Check whether unknown counters during checkpoint events are absent native
  telemetry or parser loss; do not manufacture values or silently reuse stale
  activity. Frozen4a source remains unchanged while fixes are reviewed at root.
  The render uses a fixed xterm palette and pyte (no SGR2 faint emulation), not
  an OS terminal screenshot. Native benchmark resources have been handed back.

### 2026-09-29 23:40 UTC — lookup falsifier negative; real visual audit running

- [M] AllMiss finishes and is independently cold-All accepted for the same
  58 queries/32 roots:10.984+16.159=27.143s;51,139 domains,31,846 inspections.
  It is0.74% below Snapshot's paired mean but10.58% above Ready's mean: no
  useful win, no repeat. Snapshot P1 inspection/merge/boundary/checkpoint
  seconds4.904/1.665/0.332/0.029; AllMiss4.565/2.130/0.063/0.027. Do not add
  overlapping phase/worker totals as a replacement for measured wall time.
  Raw summaries: `TMP/codex-final-monitor-matrix.LJLGfi/{final-abba-results,
  all-miss-results}.json`; all15 owned groups drained before handoff.
- [M] `dashboard_stream` now runs the disposable actualFG248 M4 control,
  monitors on real PTYs at multiple widths/resize/NO_COLOR, and captures actual
  stream data. Evidence-only pyte0.8.2/wcwidth0.2.13 are installed only in
  ignoredTMP. Chromium renders captured terminal states; pyte's lack of SGR2
  faint support will be disclosed. No production state or terminal is changed.

### 2026-09-29 23:38 UTC — final original-input four-loop gate remains negative

- [M] Final3428 ABBA completes with four raw cold-All passes, each covering
  all58 required queries/32 admitted roots with no uncovered obligations.
  Native+cold seconds: ReadyA1 25.134,EpochP1 28.152,EpochP2 26.538,ReadyA2 23.955.
  Means:Ready24.545 versus Epoch27.345,11.41% slower. Root checked the raw
  request binding, role/physical coverage and CP6 acceptance separation;
  checkpoint-only Python audit remains honestly INCOMPLETE, while raw cold-All
  establishes the claimed scoped control closure. No parity or speed gate pass.
- [D] Continue the already prepared AllMiss and pivot diagnostics, not an
  unbounded blind permutation search. The dashboard author has the real4L
  visual-audit handoff next; a local isolated pyte dependency is authorized for
  faithful replay of captured terminal state, rather than a home-grown emulator.
  Its resource use is not included in matched solver timings.

### 2026-09-29 23:36 UTC — pivot diagnostic unblocked without an equality claim

- [M] Independent review concludes raw terminal-key equality is not a
  prerequisite for an exploratory pivot comparison. Selected-owner loading
  permits owner-specific orders while checking family/solver policy and
  native programs. Different finite bases can be legitimate; equal aggregate
  residual counts are not an identity proof. Cold-All remains required, with
  its actual scope: closure relative to supplied candidate programs, not an
  independent replay of their original algebraic derivation.
- [D] Authorized a fresh-natural versus mixed-line-first banana2x2 crossed with
  Ready/Epoch, unchanged full58-query scope and matching budgets. This separates
  pivot effects from scheduler effects. Record native loaded-terminal counts;
  do not claim production readiness or cross-program reduction equivalence.
- [M] Root prepared inputs only under `TMP/codex-banana-stitch.4Lf4R3/`, using
  its `stitch.py`. The exact generated root mask selects ordinal5; all15 other
  payload bytes,508 route objects and58 query bytes/roles are preserved. Payload
  metadata uses actual bytes, not stale original informational hashes. The
  execution lane is independently checking these inputs before native use.
- [M] First final matched pair cold-All passed: Ready9.977+15.157=25.134s;
  Epoch FIFO11.987+16.165=28.152s. No performance win. FIFO's one10s contention
  sample estimates3.26 foreign busy CPUs on the16 assigned CPUs; Ready ended
  before a corresponding sample. Retain this caveat, not an invented correction.

### 2026-09-29 23:32 UTC — isolated runtime-policy implementation begins

- [D] Approved `stage_a_release`'s A-only source/tests implementation in
  `TMP/codex-runtime-discovery.280crc/repo`, branch
  `codex/runtime-discovery-strategy`, based on4a1371a5. It must not alter the
  qualified-for-lifecycle3428 build or force a new long build into the current
  performance-gate path. No broader B-order implementation is implied.
- [M] Existing generation checkpoints already bind family input, fingerprint,
  root, permutation, solver policy, backend, sector inventory and source/
  preconditioning recipe version. There is no existing portable prepared-basis
  coefficient digest. Use the explicit recipe-version binding plus persisted
  validated finite ordinal plans, with deterministic basis regeneration and
  row-count/bijection checks; do not invent a coefficient hashing or CAS layer.
  Plans guide scheduling, while original source IDs and exact replay remain
  mathematical authority. A freshly implemented framework needs separate tests
  before merge; it is not yet exposed in the stable CLI/Python build.
- [M] Final combined4L ABBA measurement started after the N10 job drained;
  first Ready native9.977s completed with32 initial roots closed. Independent
  cold verification is still running, so no accepted pair or new win yet.

### 2026-09-29 23:30 UTC — real-terminal visual audit requested

- [D] User requests deep visual inspection by the dashboard author on a real4L
  run. Reassign the prepared actual supervisor-stream smoke from StageA to
  `dashboard_stream` after the critique slot frees. Require real PTY capture,
  actual rendered-image inspection, multiple terminal dimensions and resize/
  checkpoint/readability checks; distinguish replay/synthetic stress states
  from live observations. No production tab/campaign changes. Root found an
  already installed Chromium renderer; no new website, network service or
  solver build is necessary for visual evidence.
- [M] The single final-bound N10 diagnostic compile was censored at120.202s
  without an executable. No retry or ad-hoc decoder. Terminal identity remains
  unmeasured. Independent reviewer is assessing whether an exploratory stitched
  pivot control can validly proceed without that equality claim, comparing the
  same new programs between engines and retaining full original query scope.
  No deployment or cross-program equivalence is inferred from residual counts.

### 2026-09-29 23:29 UTC — final native lifecycle passes; dashboard pushed

- [M] Final3428 M2 with two inspectors passes every observation and exact
  lifecycle gate: saved refresh4, genuine resumed heartbeat refresh5 with
  46,876/90,095 domains closed. Both legs use the sameFG248/W6 request and
  inspector allocation. Native17.221+7.483s; full case38.594s. Independent
  cold-All248 passes, no checkpoint mutation and all owned groups drained.
  First M2's short resumed leg remains honestly INCOMPLETE, not overwritten.
- [M] M4 passes: native14.425s,6,177 committed cuts, case26.756s,
  independent cold-All248. Receipts under
  `TMP/codex-stage-a.2RU3AX/final3428-M2-live-inspect2` and `final3428-M4`.
  Explicit heavy-resource handoff to the single bounded N10 diagnostic occurred;
  final combined4L measurements follow. No lifecycle timings imply speedup.
- [M] Dashboard/source milestone4a1371a5 committed and pushed. Separate frozen
  Python checkout at `TMP/codex-parallel-campaign.oiPK29/python-delivery-4a1371a5`
  preserves the native3428 source and binary. Actual supervisor-stream smoke
  is being prepared for a gap after the combined4L comparisons. Production
  remains untouched and the launch draft remains unqualified.

### 2026-09-29 23:27 UTC — dashboard milestone ready for push

- [M] Final independent review/test pass:344 Python tests,343 passed,one
  existing optional skip,32.380s; clean diff check. The hard-link input/output
  overwrite regression also passes. Evidence summary:
  `TMP/codex-dashboard-preview.kLzfrO/receipt-independent-critique.json`.
  Author's synthetic previews and measured presentation-only cost remain
  separate from real campaign/solver evidence. No native source changed.
- [M] Final3428 M4 lockstep/public lifecycle control passed with no unobserved
  paths (`TMP/codex-stage-a.2RU3AX/final3428-M4/result.json`). The one extra M2
  two-inspector control is running. Next actual dashboard stream smoke will
  use a separately frozen matching Python checkout and the same native binary.

### 2026-09-29 23:24 UTC — independent dashboard audit; lifecycle observation gap

- [M] `parallel_gate_critique` independently audited the telemetry/UI/plot slice
  and corrected stale unresolved counts to an explicit upper bound, suppressed
  invalid/reset local rates in the plain summary, and rejected unrepresentable
  malformed numeric input in presentation. Raw legacy fields and pending-growth
  arithmetic remain unchanged. Full Python343 tests:342 passed,one optional
  skip,30.197s. A reproduced SVG/input hard-link overwrite hazard is receiving
  a final narrow guard/regression before source freeze and commit.
- [M] Final3428 M2 actual pause/resume, joined worker accounting and independent
  cold-All248/no-write checks passed. The extra resumed live-heartbeat predicate
  is INCOMPLETE: resumed work finished in4.634s, before the5s emission interval.
  First-leg live telemetry and increased final restored refresh counters are
  present; neither is substituted for the missing heartbeat observation.
- [D] After M4, authorized one additional M2 using the sameFG248/W6 request and
  public two-inspector allocation on both legs, retaining unchanged observation
  predicates and all original receipts. No artificial delay, engine edit or
  repeated open-ended test adjustment. Then hand off the bounded N10 diagnostic.
- [M] Independent strategy review accepts the finite-plan and weighted-order
  proof design, with an explicit denominator-before-numerator tie-group detail
  added for faithful default equivalence. Broader pivot control remains design,
  not a currently implemented or benchmarked engine capability.

### 2026-09-29 23:21 UTC — monitored optimized binary frozen

- [M] Final native3428 build completed successfully:3786.523s wall,
  3600.041s user+144.338s system; maximum single-child RSS17,318,900KiB;
  minimum host available704.589GB. Source/build tree clean and owned processes
  drained. Immutable executable:
  `TMP/codex-parallel-campaign.oiPK29/candidate-bin/rustred-3428b519`, SHA256
  `321b02b166c61dae927a220b7b8007b4659fef009d2b5b003084830b0f43eca3`,
  125,735,216bytes. Compilation is not solver time; frozen89 remains unchanged.
- [D] Resource sequence: final actual CLI M2/M4 lifecycle gates (`stage_a_release`),
  one bounded N10 terminal-key loader attempt (`parallel_gate_critique`), then
  final combined4L ABBA and AllMiss falsifier (`bounded_ordering_pilots`).
  Four-loop parity/win is still unproven. Do not start production from the draft.
- [M] Read-only LC2 snapshot:148,712,265 discovered,62,225,465 local completions,
  48,013,427pending,zero frontiers;6/67 initial roots and7,185,109 domains closed
  in a conservative snapshot about1496s old;67.79GB process-tree RSS. No ETA or
  fresh closure claim is inferred. The active supervisor remains untouched.
- [M] Strategy/plan documentation committed and pushed as94226d15.

### 2026-09-29 23:18 UTC — dashboard delivered; runtime strategy design reviewed

- [M] `dashboard_stream` delivered a presentation-independent bounded JSON
  producer, separate aligned coloured terminal consumer, persistent
  `telemetry.jsonl`, and bounded-memory standard-library SVG plotter. Final
  full Python suite:340 run,339 passed,one existing optional skip in30.514s.
  Root independently reran the16 dashboard tests successfully. Evidence:
  `TMP/codex-dashboard-preview.kLzfrO/receipt-post-review.json` and
  `full-python-tests-post-review-v2.log`; previews there are explicitly
  synthetic, not production observations. Independent critique is active;
  conservative unresolved-count presentation is being checked before commit.
- [D] Final delivery must freeze the new Python modules alongside the monitor.
  Native3428 remains unchanged and its optimized build is still linking.
  The existing LC2 process receives no writes, signals or source replacement.
- [M] `stage_a_release` delivered and root reviewed
  `docs/research/runtime_pivot_strategy_design_2026-09-29.md`: finite materialized
  discovery policies/callbacks (A) are distinct from a persisted weighted
  integral-order program with concrete/shifted descent semantics (B). This is
  a design, not implemented broader pivot control. Lifecycle gates remain
  first; an isolated A implementation plan is requested afterwards. No native
  source change or extra engine build is authorized merely for each policy.
- [D] Authorized one input-only4L finite-helper-prefix pilot after the required
  final baseline/AllMiss controls, within240s including cold verification and
  shutdown. Its16 added auxiliary cells contain97 initial integer points and
  are independently enumerated subsets of original required rows. All58
  original required objects/order and16 owner programs remain unchanged.
  This is an early-narrow-work proxy, not scope-changing checkpoint resume or
  true iterative deepening. Five-loop reduced-scope execution remains deferred.

### 2026-09-29 23:10 UTC — richer strategy lane; four-loop-first deepening

- [D] User asks for greater/full pivot expressivity via a Rust/Python strategy
  abstraction, delegated independently, and clarifies that rank deepening is
  speculative and should be tried at4L first. Assigned`stage_a_release` the
  strategy proposal, retaining immediate final M2/M4 priority. Native3428
  source remains frozen; no ordering-framework edit or rebuild is authorized
  before proposal review. Preserve cheap per-input future experiments rather
  than compile a full engine for every strategy.
- [M] Initial source finding: an isolated-case source-row ordering hook already
  exists, but ordinary sector and shared numerical-tail solving do not yet
  expose/use a uniform runtime recipe. A minimal discovery descriptor can reuse
  that seam without changing the mathematical order. A broader persisted order
  program needs coherent concrete/shifted comparisons, exact lifting and
  codec/replay semantics; a comparator callback alone is not safe authority.
  A/B design document requested, with a possible materialized callback plan.
- [D] Five-loop probe execution deferred. Preparation found67 nonempty finite
  subsets of original required rows,704 initial integer points:56 owners atR0,
  seven atR1, four atR2. This is topology-wide diagnostic coverage only. The4L
  candidate instead has16 finite helpers/97 initial points, prepended to the
  unchanged58 required rows; no broad R12 prefix. It is only an input-order
  proxy, since all initial admission precedes traversal. Preparation/review
  continues; no native pilot execution or benefit claimed.
- [M] Dashboard pre-review full suite:338 run,337 passed/one existing optional
  skip. Root requested two narrow corrections before final acceptance: warm-up
  must account for delayed first completion sample; the normal consumer path
  should consume a frame alone rather than raw supervisor status for checkpoint
  messages. Independent review and corrected tests remain pending.

### 2026-09-29 23:01 UTC — exact pivot controls and all-owner low-rank pilot

- [M] Answered the user's expanded ordering question: runtime coordinate ties,
  query order and FIFO/adaptive dispatch are exposed; arbitrary earlier
  sector/cut/degree or source-row pivot policies are not a public general API.
  The natural/reverse certified four-loop subroot comparison favors natural
  (core1.610/4.580s, candidate1.82/7.37MB, equal19 terminals). Later mechanistic
  query permutations give no decisive win; banana-generation comparison remains
  incomplete at the terminal-key/full-walk gate.
- [M] Existing single-owner five-loop controls are useful but not all-owner
  coverage: earlier Ready+Union finite whole-command180.010/180.746s, hot
  197.730/201.743s, followed by separate independent verification. Sources:
  this log's06:53–08:19 entries and accepted receipts under
  `TMP/codex-g2-pilot-prep.n7Kd5q/deployment-continuation/`. These are not fresh
  final-Epoch timings. Current fixed matrix retains both controls.
- [D] Delegated light all67-owner rank-pilot preparation to
  `bounded_ordering_pilots`, without altering the fixed22-arm matrix or starting
  another job. Explicitly check A/R/D nonemptiness: low globalR can remove
  high-line physical sectors. Keep probes finite at entry, descendants
  unclipped, and production unchanged. A3–4minute fully verified target is
  exploratory, not a guaranteed runtime. No cross-scope checkpoint reuse claim.
- [M] Dashboard producer/consumer first slice passes existing31 monitor tests;
  implementation and independent review continue. Review caught a plot bucket
  memory issue; author replaced growing buckets with bounded extrema state.
  Normal graph-dirty closure snapshots remain visible as conservative observed
  history; actual missing/reset/stale-heartbeat observations create gaps.
  Complete dashboard tests and frozen-source delivery remain pending.

### 2026-09-29 22:51 UTC — orthogonal dashboard and timeseries directive

- [D] User requests a substantial coloured/aligned terminal dashboard, a clean
  producer/consumer abstraction supporting a later website consumer, and saved
  completion/recursive-closure rate series with a plotting utility. Delegated
  implementation to`dashboard_stream`; root integrates, StageA/critic reviews
  independently after existing lifecycle/diagnostic slots. No native edit or
  production operation. Existing pending-growth arithmetic stays unchanged.
  Plot dependencies must not burden headless campaign execution. The stream
  retains raw observations and freshness rather than presenting scan-batched
  closure jumps as instantaneous throughput. Scope added toSHORTENED_PLAN.md.
- [M] Clarified pivot control honestly: existingCLI`--permutation` and Rust
  `IntegralOrder::with_permutation` change coordinate ties without rebuilding.
  Natural/slot9-first banana generators completed, but exact terminal-key and
  full58-query comparison is still pending. The option does not reorder earlier
  sector/degree comparisons. Regenerating changed rules is still necessary.
- [M] Progress/doc milestone`abc24fdf` pushed to`origin/fable_5_1_parallel`.
  Same-cache3428 optimized build continues unchanged; prior engine/comparison
  binaries remain frozen. Root checkedCPU32–81 are50 distinct physical cores
  on socket0, disjoint fromLC2's128–227 on socket1, including SMT identities.

### 2026-09-29 22:23 UTC — bounded remaining diagnostics

- [D] Register one runtime-only falsifier for replica overhead on the short
  combined4L control: final-binary rolling FIFO+Union with existing
  `--epoch-inspector-lookup all-miss`, versus the contemporaneous Ready and
  Snapshot arms. All58 rows, W16, placement and cold-All gates remain identical.
  No recent equivalent rolling receipt exists; older S2 tests are not this
  comparison. `all-miss` omits lookup Publication replicas and uses the ordinary
  inspector Resolver plus merge-time resolution. Falsifier: unchanged/worse
  native+cold time or increased work with no net gain. One arm after original
  final ABBA; repeat only a material benefit. No new native implementation.
- [D] The optional raw-terminal diagnostic is narrowed to the actual N10 fixture
  to avoid15 unnecessary generic instantiations. This is a research adapter,
  not topology-specific engine logic. Existing public loader and key enumeration
  remain byte-identical. StageA independently reviewed it; bind only coherent
  final3428 artifacts after build, then allow one120s compile attempt. No
  unbound older app library, custom decoder or further fallback. Exact keys
  remain pending until that native API actually executes.

### 2026-09-29 22:20 UTC — final build and completed ordering evidence

- [M] All eight same89 query-order arms pass full cold-All verification of
  the unchanged58 required rows. None establishes a decisive gain. The broad
  banana anchor reduces roots32→31, domains51,139→49,038 and inspections
  31,846→31,667, but total25.904s is11.97% slower than its matched Ready arm.
  Evidence: `TMP/codex-verifier16-matrix.0xtHE0/QUERY_ORDER89_RESULTS.md`
  and `query-order89-results.json`; exact hashes/CPU/RSS and interpretation
  are preserved in the mechanistic-ordering report. All36 groups drained.
- [M] Banana natural/slot9-first sparse generation completes in6.331/6.428s
  native-reported time (9.161/9.154s guarded); six sectors and16 aggregate
  residuals each. Rules549→469 but coefficient bytes2,604,135→2,965,222.
  The native ordinal5 root shards differ in size; counts alone do not prove
  terminal-key equality. The bounded loader diagnostic timed out after120s
  without an executable. No stitched58-query test or pivot gain is claimed.
  Root and StageA independently checked the receipt; all groups drained.
- [M] Actual a35 Tracker harness passes12 tests, zero failures, one ignored
  scale benchmark, using coherent existing non-LTO dependencies. Guard elapsed
  3.152s; tests0.06s. Evidence:
  `TMP/codex-closure-monitor-tests.qzQuvW/tracker-guard-v3/`.
  This executes Tracker internals, not the new controller/full-app tests.
- [D] After explicit critic resource/cache handoff, root authorized the
  unchanged same-cache optimized build at3428b519. Started22:16:31.858 UTC,
  group3181595, CPUs0–15, heavy/build0 locks; receipt:
  `TMP/codex-parallel-campaign.oiPK29/campaign-build-3428b519/`.
  Clean89 source/vendor and immutable89 binary retained. Initial available
  host memory722.785GB. Prior65-minute compilation informs an estimate only;
  no solver timing includes compilation. Final M2/M4, individual/combined4L,
  finite/hot5L and measuredW50 controls remain pending, not launch-qualified.
- [E] Source audit explains limited query-order leverage: both engines admit
  initial geometry before native dispatch, and full-containment lookups can
  use admitted pending domains. Earlier solving is not required for that reuse.
  Partial G2, representative/cut choices and pivot-induced geometry can still
  change work. This is not an order-invariance theorem.

### 2026-09-29 22:04 UTC — structural query-order evidence

- [M] Same89 binary, same58 query rows/roles and owner programs, W16:
  shared-interface-first gives Ready24.955696s versus Epoch24.964190s;
  dependency-ready-cost gives Ready22.170699s versus Epoch24.337209s.
  Both pairs pass full cold verification. Epoch's scheduled count changes
  only slightly (51,573 /51,097 versus original51,139); its native work stays
  essentially31,846 inspections. No meaningful native improvement established.
- [M] Critical-interface-first native/cold receipts likewise remain negative:
  Ready9.584+12.163389s versus Epoch10.992+15.150294s; all32 roots verify.
  Final secondary acceptance is being collected by the measurement lane.
- [D] Keep the one-owner broad-anchor pair and the narrow actual pivot test,
  not a factorial permutation sweep. The larger verification/pilot record
  continues to distinguish cold-check savings, native work and scope changes.
- [M] Root reviewed the opt-in M2 live-monitor acceptance extension to the
  existing driver: actual periodic native closure observations are required
  before pause, and a newer refresh than the saved count after resume. Missing
  observations remain incomplete; original stop/resume/FG248 cold-All checks
  stay intact. Execution waits for the monitored binary, not the current89.

### 2026-09-29 21:54 UTC — repeated baseline and mutation gates

- [M] Second counterbalanced pair: Epoch11.195s native +13.154533s cold =
  **24.349533s**; Ready9.780s +15.156218s = **24.936218s**. All four runs pass
  full cold-All32/32 roots /58-query scope, zero violations. Mean combined
  time is Epoch25.8450s versus Ready23.2965s (10.94% slower). One nearly even
  pair does not establish parity; native timing remains worse in both pairs.
- [M] Independent critic ran the public routed-false-hit mutation on both
  preserved checkpoints with the frozen89 verifier. Both deliberately fail
  exactly one uncovered successor; all22,601 Ready /31,846 Epoch natives were
  reinspected, with zero exact/brute or G2 disagreements. The Epoch source has
  exactly16 distinct targets, exercising the newly indexed medium-fanout path.
  Checkpoint authority/files and executable remain unchanged. Root checked
  receipts at `TMP/codex-cold-negative.HgFhPT/{ready,epoch}/receipt.json`.
- [D] The critic returned both heavy/pilot locks. Proceed with the three
  original mechanistic query-order pairs, then the separately bound one-owner
  broad-anchor variant. That variant moves only existing row45 before rows6–8,
  retains all58 rows/roles and avoids one narrow initial banana root; its
  downstream work benefit is a hypothesis, not established by containment.

### 2026-09-29 21:48 UTC — first new paired control remains negative

- [M] Optimized `89d90a3a` frozen binary SHA256 is
  `581dc252aaeffd647032a37c944df673aaa3c8e91120394d08ce0eb67679ae5c`;
  root and independent critic reproduced it. First combined-four-loop pair:
  Ready native9.496s + cold12.160875s = **21.656875s**;
  rolling FIFO native11.183s + cold16.157556s = **27.340556s**.
  Both pass full cold-All verification of all32 initial roots /58 queries;
  FIFO is 26.24% slower at this boundary. Secondary audits are separately
  charged (6.1633s Ready,1.1503s FIFO), not hidden solver improvements.
- [M] New native monitoring source `a35bfed8` passes release test metadata
  in44.168s. The standalone Tracker harness has not executed: its first
  compile omitted the existing Blake3 dependency; the corrected link then
  found campaign rlibs contain LLVM-only objects requiring LTO. Both failed
  receipts are retained. This is harness linkage, not an engine/test failure;
  a coherent existing non-LTO dependency set is prepared for a later slot.
- [D] Keep the original negative-control and query-order sequence. Do not
  label verification lookup savings as native solver speedup. Investigate a
  narrowly targeted broad-anchor-first variant for the dominant owner only,
  rather than reopening the already negative all-helpers-first experiment.

### 2026-09-29 21:42 UTC — optimized verifier build completed

- [M] `bounded_ordering_pilots` completed the clean `89d90a3a` campaign build:
  exit zero, no guard stop, 3,910.600 seconds compilation (not solver time),
  17,327,744 KiB maximum single-child RSS; minimum available host memory
  703.746 GB. Root independently checked the receipt, clean worktree and exact
  source revision. Evidence:
  `TMP/codex-parallel-campaign.oiPK29/campaign-build-89d90a3a/result.json`.
- [D] After freezing that binary, run the two counterbalanced combined-four-
  loop pairs, indexed negative controls, and three mechanistic query-order
  pairs. These still precede any launch recommendation. The new native closure
  telemetry patch is deliberately absent from this immutable comparison;
  its separately identified build and final controls remain necessary.
- [M] The build process group drained. A short serialized validation slot on
  CPUs16–19 now checks exact `a35bfed8` metadata and the independently audited
  Tracker/example diagnostics; production and its CPU allocation are untouched.

### 2026-09-29 21:35 UTC — exact terminal inventory adapter prepared

- [M] `parallel_gate_critique` added only `candidate_bundle terminals ARITY
  BUNDLE` to the existing Rust example. Root independently reviewed the
  bounded read and existing native loader call; it outputs sorted raw keys
  and explicitly makes no source-replay, normalization or closure claim.
  This is needed to detect changed terminal inventories in pivot comparisons;
  counts alone are insufficient. No CAS or binary decoder was implemented.
- [M] Formatting/source checks pass; its separate diagnostic build and native
  validation remain pending. It does not change the timed CLI engine.

### 2026-09-29 21:34 UTC — integrated Python regression pass

- [M] Root ran all example steering/monitor tests on the current branch:
  324 run, **323 passed / one existing optional skip**, 30.417 seconds.
  Command: `TMPDIR=/common/dev/rustred/TMP PYTHONPATH=examples/python
  flock TMP/locks/python-0.lock taskset -c 32-39
  /nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
  -B -m unittest discover -s examples/python -p 'test_*.py'`.
- [M] The preceding four-CPU invocation had one diagnostic-test failure:
  that test deliberately requests six workers, so affinity admission rejected
  it before the intended incompatible-scope diagnostic. Rerunning with eight
  permitted CPUs resolved it without editing code or weakening assertions.
- [M] Mechanistic pivot preparation caught and removed incompatible explicit
  finite-retention flags under the unchanged `search` policy. No generation
  arm has run yet; prepared input variants remain distinct from measurements.

### 2026-09-29 21:32 UTC — native closure telemetry source reviewed

- [M] `stage_a_release` added committed-boundary monitoring maintenance to
  both rolling and lockstep Epoch controllers. `parallel_gate_critique` and
  root independently reviewed the change: it reuses the existing exact,
  cancellable dependency scan and its five-second / approximately 1% duty
  throttle. Checkpoint saving does not force a scan, and workers/resolvers
  do not consume the observational closed flags for rule decisions.
- [M] Live scalar progress now carries closure counters, scan age and scan
  cost without traversing graph storage in the heartbeat. Restored age is
  unknown until a new scan. Scratch-allocation failure disables this optional
  tracker conservatively; it cannot manufacture closure authority.
- [M] Added source tests for throttling, cancellation, no maintenance before
  an unpublished stop, unchanged decisions/edges with monitoring enabled,
  and persisted flags/counts on cold checkpoint restore. These new native
  tests are not yet compiled or executed; metadata validation and a separate
  optimized binary follow the already-running `89d90a3a` pilot prefix.
- [D] No topology/order or production setting changed. The build tree being
  compiled remains frozen; unrelated FeynKit and untracked work is excluded.

### 2026-09-29 21:25 UTC — requested rolling-hour metric implemented

- [M] `parallel_gate_critique` implemented the display/heartbeat slice; root
  reviewed the arithmetic and independently ran both focused modules:
  47 tests passed in 0.151 seconds. Existing pending-growth arithmetic and
  rendering are unchanged.
- [M] The new signed quantity is
  `delta(total_domains - total_closed) / sampled_seconds`, using paired counts
  from the same closure telemetry over the trailing hour. Both TTY and plain
  output show actual sample span, warm-up, snapshot freshness/age and whether
  a scan advanced. Missing optional telemetry preserves history but is visibly
  marked; invalid counts, clock/counter resets and resume discontinuities do
  not fabricate a rate. The rolling boundary is not interpolated.
- [M] Root corrected the initial warm-up definition during review: a slightly
  shorter sampled span after a full hour is not perpetual startup. Age uses
  the already-aged live progress report when available, avoiding both frozen
  endpoint age and double-aging.
- [D] Authorized `stage_a_release` to add the missing native Epoch observation
  hook, reusing the existing duty-throttled exact Tracker scan with actual
  cancellation at committed coordinator boundaries. It must not force scans
  per checkpoint, change resolver decisions or grant closure authority. This
  will be a separately identified optimized binary after the current pilot
  prefix; the ongoing `89d90a3a` build is not being modified.

### 2026-09-29 21:21 UTC — guarded rescue planner independently checked

- [M] `stage_a_release` completed the Python-only planner guard. Root reviewed
  ownership, admission, resource-stop and late-resume paths and independently
  reran `test_frontier_rescue.py`: 14 tests passed in 8.483 seconds, under
  `python-0.lock`, CPUs 16–19, workspace `TMPDIR`. The author's full suite
  passed 313 tests with one existing optional skip (314 run, 31.1605 seconds).
  Receipt: `TMP/codex-stage-a.2RU3AX/planner-guard-full-v2`.
- [M] The first full run's one new test failure is retained in the v1 receipt:
  a minimal fake native result did not have a paused-state event. The final
  implementation recomputes the terminal state with a planner-time stop reason,
  rather than showing the pre-planner state. Assertions now check that actual
  handoff behavior, no second native launch, and no `execv` after the stop.
- [M] Successful planners, large stdout/stderr, affinity/thread caps, insufficient
  admission, low host memory, hard RSS, monitoring failure, operator cancellation,
  SIGTERM-resistant child cleanup and late stop/failed-plan refusal are covered.
  Resource protection applies during graph restoration, not only the solver.
  The planner never writes the native checkpoint. An interrupted post-receipt
  handoff conservatively consumes one recorded rescue attempt.
- [M] Further Epoch monitoring audit found both omissions: live scalar progress
  lacks the recursive-closure object, and runtime cuts do not periodically
  refresh the cached tracker. A formatting-only rate would be unavailable on
  live Epoch. A narrow telemetry/throttled-refresh correction is being designed;
  the current `89d90a3a` build remains unchanged and keeps its own identity.

### 2026-09-29 21:13 UTC — launch safety and requested gap-rate display

- [D] User requests a separate trailing-hour discovered-minus-recursively-
  closed domain rate. `parallel_gate_critique` owns the existing heartbeat /
  display modules and their tests; the pending-growth metric remains untouched.
  Report the observed conservative-gap trend with actual sample span and
  closure-snapshot freshness; negative is not a completion certificate.
- [M] `stage_a_release` found that automatic frontier planning runs after the
  native RAM-monitoring loop. It restores graph sections and reverse edges,
  so it is not a negligible metadata-only operation. Its process inherited
  CPU limits but lacked active RSS/host-floor/swap protection.
- [D] Approved a narrow Python-only fix reusing `owned_process`, `RamGuard`
  and the existing injectable planner runner. Planner stop/failure must leave
  the already durable native checkpoint untouched, refuse amendment publication
  and prevent automatic resume. No default address-space cap or new supervisor
  framework. Root reviews independently; no new Rust rebuild.
- [M] The new mechanistic cost census finds the banana owner takes 58.06% of
  Epoch and 66.48% of Ready native-record inspection seconds. A third exact-
  row permutation moves just that original three-query block first, testing
  critical-path overlap against possible ordered-publication head-of-line
  blocking. All 58 rows/roles and the broad-anchor suffix remain unchanged.
- [M] `certify-candidates` requires unrestricted saved-downset closure as well
  as replay. That is not the scoped campaign task. Sparse candidate generation
  constructs exact Symbolica row combinations, but its report explicitly does
  not claim independent source replay. Do not mislabel either ordinary candidate
  generation or cold graph reinspection as unrestricted algebra certification.

### 2026-09-29 21:06 UTC — bounded historical and structural audits delivered

- [M] Historical audit is written to
  `docs/research/closure_count_history_2026-09-29.md`. Root independently
  reproduced the unchanged LC1-to-LC2 walking-source diff and archived cone
  counts: root 14 has 8,383 nodes / 196,438 edges; root 15 has 15,036 /
  215,390. Roots 23 and 24 are shared dependencies. The new four-row diagnostic
  explicitly declares all four Required; every original row is unchanged.
- [M] A bounded LC2 prefix read shows an actual dependency chain
  `14 -> 168168 -> 1391654`; the last saved node is sealed but not closed.
  This is not a complete path to a pending leaf and does not prove a tracker
  bug. The archived fresh eight-versus-six difference predates Stage A's
  compatible binary upgrade.
- [M] Structural report is written to
  `docs/research/mechanistic_ordering_2026-09-29.md`. BMW is reached by three
  prism one-pinches and all nine K3,3 one-pinches. The two query-only variants
  retain the actual control's explicit 58-Required/0-Auxiliary declaration,
  exact query objects, within-owner order and broad-anchor suffix.
- [D] Test the nontrivial shared BMW interface for algebraic priority, rather
  than the degenerate four-line owner with maximum fan-in. Prepare two
  pinch-class-derived coordinate priorities and the same-source natural
  baseline. Changed payloads require exact validation and terminal-policy
  comparisons; saved-payload pivots cannot be changed by a walker flag.
- [M] Optimized application rebuild continues without diagnostics. The
  current native engine and matrix remain frozen. Heavy work sequence is
  combined four-loop repeats, two independent negative cold controls, then
  resource-serialized remaining controls and mechanistic experiments. No
  experiment below is represented as executed before its receipt exists.

### 2026-09-29 20:49 UTC — ordering mechanisms and historical narrowing

- [M] `stage_a_release` located the original12/67 receipt in
  `campaigns/five-loop-qcd-feynman-d9d10/runs/20260926T122852.894005Z/status.json`.
  That campaign differs from LC2 in47 helper A bounds (60 unbounded helpers
  became13). All116 physical rows and owner/row order agree. Do not call12→6
  a same-scope regression.
- [M] A stronger comparison exists: v2/LC1/LC2 have byte-identical queries
  and the same3,704-byte initial-domain segment. Old closed roots were
  `[0,2,12,14,15,23,24,26]`; LC2 generation8 has `[0,2,12,23,24,26]`.
  Its closure snapshot revision is current for that saved generation. Roots14
  (`111100000011100`) and15 (`111010100100101`) are the focused candidates.
- [M] The initial inspection records for14,15,23,24 match after excluding
  timing. LC1→LC2 walking/tracker/alias/CSR source is unchanged; all67 owner
  payloads correspond through the archived format conversion, without algebra
  regeneration. This favors scheduling/representative history as an explanation,
  but is not yet a demonstrated cause or a proof that no bug exists.
- [D] Prepare a small four-anchor diagnostic retaining their exact rows and
  all67 owner programs; no full production checkpoint replay. Compare exact
  cold closure and dependency behavior before attributing regression.
- [M] Structural lane verified that public `--permutation` changes only final
  denominator/numerator ties, not the earlier sector lexicography. Therefore
  putting shared-pinch edges first is not automatically a sector-priority
  experiment. A true sector-order probe can instead reindex denominator input
  rows and masks, regenerate the affected family, and use the inverse tie
  permutation to isolate the earlier sector order. That is input computation,
  not a Rust rebuild, and must not mix different family fingerprints.
- [D] First structural candidate will move selected shared-subtopology owner
  blocks, preserving every query and leaving the16 broad rank12 obligations
  in their original suffix. Do not repeat the rejected all-helpers-first H1.
  All heavy pilots remain queued behind the current build and combined gate.

### 2026-09-29 about20:43 UTC — authorized structural-ordering extension

- [D] User grants another4–5hours to obtain a decisive combined-four-loop
  improvement and a mechanistic ordering study. Updated `SHORTENED_PLAN.md`:
  target00:43–01:43 UTC September30, retaining honest gates and untouched LC2.
- [D] New structural lane `parallel_gate_critique`: derive a small portfolio
  from actual common pinches/subtopologies, owner coverage and rule/guard costs,
  not arbitrary reversal/permutation. Separate input admission, routing-owner
  priority, pending dispatch and algebraic pivot choices. Prefer existing
  runtime controls and preserve every query; temporary rule regeneration is
  charged explicitly when algebraic priority changes.
- [D] Independent `stage_a_release` audit investigates the reported historical
  12/67 versus current6/67 recursively closed roots. First establish matching
  domains, bounds, owner programs, counters and snapshot freshness. No large
  production replay or attribution of a regression from counts alone.
- [M] Existing89d90a3a optimized build continues with Symbolica, Graphica,
  Numerica and core all reported fresh; application crate recompiles. Candidate
  order trials remain input-driven on this frozen engine. Source/doc milestones
  through `ecb973d0` are pushed, but launch remains unqualified.
- [D] Measure native walking and cold reinspection separately. Even if the
  verifier improvement wins the combined gate, it is not itself a claim of
  faster generation or better five-loop scaling. No new unrelated features.

### 2026-09-29 20:35 UTC — minimal indexed-verifier slice audited and typechecked

- [M] Integrated `89d90a3a` (author commit `6610454a`): the only production
  change is the existing target-index threshold256→16 and its explanation.
  Three focused differential tests cover boundary sizes, empty/different-owner
  cells, phase isolation, corrupt/missing index hints, genuine containment,
  alias-only coverage and missing/broken target edges. No walker, CAS, schema,
  global counter or brute-force predicate change.
- [M] Independent source/math audit by `parallel_gate_critique` and root passes.
  The final alias fixture uses a valid subset-to-superset alias. Index hints
  remain non-authoritative and every accepted cover passes the same exact
  containment predicate. Candidate counters may fall; mathematical obligations
  and admitted-event counts must not.
- [M] Cached `cargo check --release --tests --locked --offline --config
  profile.release.package.rustred-app.opt-level=1 -j1 -p rustred-app` passes
  in46.181s (Cargo42.90s). Receipt:
  `TMP/codex-parallel-validation.RPJKV5/typecheck-6610454a-index16/`.
  The new tests are **typechecked, not executed**; no expensive full test-binary
  code generation was run. Actual optimized CLI cold and mutation execution
  is the next gate, not a claim that typechecking is testing.
- [D] `bounded_ordering_pilots` owns the next optimized build using the same
  absolute StageB source/cache path and campaign profile. Preserve the e1
  source and frozen binary, update to89d90a3a and record actual Cargo freshness.
  No dependency fingerprint override. The revision also includes the previously
  disclosed package/citation merge, not merely the two verifier files.
- [D] Reuse the established matched matrix and controls, new output paths
  under `TMP/codex-verifier16-matrix.0xtHE0/`. First repeat combined Ready/FIFO
  pairs. Hot remains its original W12, finite W16 and optional finite W50;
  do not silently relabel resource widths. LC2 remains untouched.

### 2026-09-29 20:32 UTC — batch hypothesis rejected; narrow verifier fix authorized

- [M] Cut1 completed and passed cold-All on every58 required input,32roots
  and54,676domains. Native14.244+cold17.225401 =31.469401s; Route count31,547
  versus31,558 at cut16. It does not cure the extra Route work; stop this
  tuning avenue and do not implement speculative per-origin antichains.
  Receipt: `TMP/codex-ordering-study.7nspU4/cut1-falsifier/RESULT.md`.
- [M] Newly available independent critic `parallel_gate_critique` identified
  a separate measured cost: combined Epoch cold verification performs127.8M
  exact containment calls and99.5M brute-force cross-checks, versus94.4M/71.2M
  for ReadyA2. Its native-seconds counter includes coverage callbacks, not
  pure physics. Preparation costs match; reinspection causes the wall gap.
- [M] Existing TargetIndex is used only at256 targets. The saved Epoch
  census finds28,738 Route records with16–255 distinct targets, emitting
  875,348 routed admits. This is approximately83% of routed admits, all
  currently below the index threshold. Each record's event count equals its
  distinct target count, so last-target locality is poor in these records.
- [D] Authorize one minimal generic correction: use the existing candidate
  index at16 targets, with focused differential tests. `stage_a_release`
  implements; `parallel_gate_critique` and root audit independently. Preserve
  exact containment, eligible brute checks, complete reference reinspection,
  full-scan/alias fallbacks and all mathematical authority. This changes
  candidate ordering only, not the walker, input scope or artifact schema.
- [D] No thread-local-counter redesign or additional CAS work. Retain all
  negative receipts. Rebuild once after review and compare Ready/Epoch using
  the **same** updated verifier; no speedup is claimed before measurements.
  `bounded_ordering_pilots` prepares cache reuse and remaining existing
  controls, without starting a competing build or production job.
- [M] Documentation/ordering/explicitly-unqualified launch milestone pushed
  as `1374eb85`. Production remains untouched.

### 2026-09-29 20:24 UTC — negative tuning result; ordering study completed

- [M] The fixed-inspector8 combined-control probe passes full cold closure
  but takes12.265+19.248647 =31.513647s, with31,555 Route inspections versus
  default31,558. This falsifies the proposed flight-width remedy. Stop that
  tuning avenue. Receipt: `TMP/codex-stage-a.2RU3AX/four-all-inspection8-result.json`.
- [M] FIFO/adaptive combined pair passes both cold checks but favors FIFO:
  26.876557s versus30.067674s; CPU126.284974 versus150.855744s. One pair,
  not an optimal-policy claim. Adaptive remains opt-in.
- [M] Public natural/reversed coordinate experiments both generate, certify
  and cold-load the same64-sector four-loop sub-root and the same19 terminals.
  Full guarded stage sums29.614053 versus42.617902s; reverse has fewer rules
  but roughly4x larger candidate payload. Retain natural production owners;
  no claim about an optimal five-loop permutation. Full measured report:
  `TMP/codex-ordering-study.7nspU4/COORDINATE_RESULTS.md`.
- [D] Authorized one fresh existing-binary cut1 diagnostic, planSHA256
  `0f8500efd082a0dfaf4c7f9f8d9d3056e6a882f53a1cc72a1a4b32808bb57dad`,
  in `TMP/codex-ordering-study.7nspU4/cut1-falsifier/`. It removes cross-job
  antichain folding but also changes window31 to16; do not over-isolate the
  interpretation. `RUSTRED_EPOCH_LOCKSTEP_B=1` is research-only, stripped by
  production steering. No deployment advice follows directly from this probe.
- [E] Independent source audit warns that simply grouping antichains per
  origin would retain contained candidates that Ready can suppress/alias
  incrementally. No source implementation is authorized before this diagnostic.
- [M] Integrated and pushed colleague commit `87135a91` (Symbolica compatible
  version/citation use) through merge `87fbf6ee`, preserving unrelated files.
  This changes no measured engine source; frozen executable still identifies
  its actual source `e1bdb9e7`, not the documentation/metadata merge.
- [D] The tentative delivery reserve expires20:25. Performance qualification
  is not complete. Continue the narrow correction/test path rather than stop
  at a failed gate or quietly weaken the user's acceptance criterion. LC2 is
  untouched; draft launch instructions are explicitly **not ready**.

### 2026-09-29 20:02 UTC — repeated four-loop parity not met; bounded diagnosis

- [M] Preserve P2 and ReadyA2 both pass complete cold checks on the unchanged
  scope. P2 native12.067+cold18.221326 =30.288326s; ReadyA2 native9.227
  +cold14.227097 =23.454097s. Together with pair1, this does **not** meet the
  combined four-loop parity gate. CPU149.493892s versus113.112375s also favors
  Ready. Retain the negative, not a claim that higher worker activity suffices.
- [M] Work differs: Epoch consistently saves51,139 domains and31,724 ordinary
  native records; Ready saves66,765/66,519 domains but only24,319/22,499 such
  native records. Cold reinspection dominates the total gap. Fewer domains is
  not synonymous with less expensive inspection work.
- [M] H1 saved/stopped without a kill: native168.296s, CPU415.728836s,
  tree RSS1,818,664,960B;1,670,241 admitted domains,1,659,458 committed,
  10,783 pending. Its accepted-status adapter correctly refuses this censored
  result. Checkpointgen1 remains resumable; H2 stays unexecuted. This rejects
  the ordering candidate on unfavorable measured lower bounds, not on a false
  mathematical-closure failure.
- [D] Granted the preregistered fresh FIFO/adaptive combined pair22/23. A
  separate ten-minute read-only implementation audit by `stage_a_release`
  investigates the extra native work and any narrowly justified existing
  runtime control. No new CAS, architectural expansion or source patch is
  authorized by that audit. Final launch remains unqualified until the
  requested performance gate is actually satisfied.

### 2026-09-29 19:56 UTC — ordering falsifier reached; preserve the negative

- [M] First combined58-row/W16 engine pair passes cold-All on both sides:
  32 distinct roots covering all58 unchanged required rows. Ready10.536s
  native+16.238391s cold =26.774391s; rolling FIFO11.514+18.206928
  =29.720928s. The first short pair is about11% slower with Epoch; do not
  label it parity or a performance-gate PASS. Repeats remain necessary.
- [M] The fixed helper-first H1 candidate reaches approximately497,493
  committed domains at47s, against preserve P1's final51,139 domains and
  completed native11.514s. These are already unfavorable lower bounds, not
  completed H1 timings or a closure claim.
- [D] Root requested an ordinary cooperative save/stop of this owned H1 pilot,
  preserving its incomplete output and censoring reason. Reject that input
  ordering for this control; leave H2 unexecuted rather than spending another
  run on the same falsified candidate. Continue original indices10/11 (P2,
  ReadyA2) after drain to finish the independent engine comparison. No query
  is removed and no acceptance threshold or runtime cap is relaxed. The
  negative four-loop result does not by itself prove a better five-loop order.

### 2026-09-29 19:49 UTC — first matched FG pair completes; combined control allocated

- [M] FG W6, same frozen executable and query bytes: Ready native16.666s plus
  cold12.208339s =28.874339s; rolling FIFO native17.714s plus cold9.212155s
  =26.926155s. Both independently verify248/248 required roots with no
  violations; Epoch's existing CP6 adapter accepts the unchanged checkpoint.
  Saved domains: Ready98,842; Epoch98,800. The1.07235x ratio is descriptive
  for this single noisy pair, not deployment-speed qualification. No censoring
  or resource stops; read-only independent interpretation audit is assigned
  to `stage_a_release` in addition to root's raw-receipt review.
- [D] Granted combined4L block8,9,28,29,10,11 next, preserving the registered
  engine-pair and PHHP query-order sequence, all58 original required rows and
  W16/CPUs32–47. Each arm retains complete native/cold/audit verification and
  its whole-pilot ceiling. Remaining individual and five-loop controls stay
  pending; the first pair does not establish their behavior.

### 2026-09-29 19:46 UTC — safety milestone pushed without overwriting colleague work

- [M] First push of safety documentation `558f6f5e` was rejected because the
  remote had colleague commit `87135a91` (Symbolica3.0.1 compatibility and
  citation/FeynKit usage tracking). Integrated it with a clean two-parent
  merge `87fbf6ee`, then pushed successfully to `origin/fable_5_1_parallel`.
  No force push. Root verified the three affected working files retain their
  exact pre-integration byte hashes; further uncommitted FeynKit documentation
  remains uncommitted and preserved. Reference-only/untracked work untouched.
- [M] The merged tree has zero change from frozen `e1bdb9e7` in Cargo.lock,
  rustred-app, rustred-core and the Symbolica gitlink. The immutable optimized
  binary and all measurement inputs/tools are unchanged; no recompile or
  relabelling of its actual build revision is needed. FG measurement continues
  on the previously bound binary. Push success is a safety milestone, not a
  declaration that the performance/deployment gates have passed.

### 2026-09-29 19:43 UTC — final lifecycle gates passed; matched FG begins

- [M] All four final optimized-CLI cases pass with no missing required
  observations: M1 W1 inline unknown-to-joined-zero; M2 W6 real interruption,
  saved cancelled work, same-binary resume and cold-All248/248; M3 explicit
  2-inspector/3-helper/1-coordinator reservations; M4 W6 lockstep6177 cuts and
  cold-All248/248. M2/M4 cold reads preserve checkpoint bytes. Complete case
  times84.354/36.118/27.710/30.223s respectively; these are lifecycle tests,
  not performance comparisons. Their existing harness does not capture reaped
  CPU seconds, so those are unreported rather than inferred. Evidence:
  `TMP/codex-stage-a.2RU3AX/final-cli-M{1,2,3,4}-e1bdb9e7/`.
- [M] Root checked raw rescue and lifecycle receipts; all owned groups drained,
  both development locks free at19:42:31. LC2 remains untouched. Native adaptive
  tests also explicitly pass: all12 adaptive-named cases in the full `f3f707af`
  app receipt, including real interruption/restoration; adaptive speed is not
  yet established.
- [D] Granted `bounded_ordering_pilots` only the first FG Ready/FIFO pair from
  `TMP/codex-rolling-measurements.eBFGjR/bound-e1bdb9e7-pilot-lock/` on W6,
  CPUs32–37. It must return complete native/cold/audit receipts for independent
  review before further allocation. The obsolete socket1-bound plans remain
  unexecuted. No build or source change is needed for these input-driven tests.

### 2026-09-29 19:37 UTC — actual final-binary rescue regression passed

- [M] `stage_a_release` completed the focused final-CLI regression, independently
  reviewed by root against raw outputs: required-scope cold PASS1/1 with8/8
  native reinspection; AllRoots rejects exactly the three abandoned helper
  roots; the existing remapped-query mutation triggers protected-prefix
  rejection; altered amendment bytes are refused explicitly. Cold reads leave
  fixture bytes unchanged. Actual resume preserves the original prefix,
  abandoned set and record/dependency digests; post-resume cold verification
  passes. Evidence: `TMP/codex-stage-a.2RU3AX/rescue-prefix-final-cli-run2/`.
- [M] Six guarded legs total7.270s wall; complete pilot through checks175s.
  This resolves the final cold-reader defect on the actual optimized binary,
  not by weakening a test. The prior full suite remains1116/1/12 on pre-fix
  source; this targeted final-code execution is not relabelled as a full-suite
  rerun. M1–M4 lifecycle checks now begin on the same frozen executable.

### 2026-09-29 19:34 UTC — frozen identity; pilot admission corrected

- [M] Root independently checked the frozen executable:
  `TMP/codex-parallel-campaign.oiPK29/candidate-bin/rustred-e1bdb9e7`,
  SHA256 `b5bd346cd9bfa925a4324031660cb3b2993e23c11f1e53765cb6758b87d9d95c`,
  125,735,056bytes,mode0555; its build clone remains clean at
  `e1bdb9e7d102652b69a6a671635b131ea6b5fc6d`.
- [M] The first focused regression attempt waited at admission, without
  launching a solver: `socket1.lock` is held by the existing orchestrator's
  reservation explicitly for CPUs128–227. The planning text incorrectly
  treated this as a spare historical label. Root verified holder text, live
  LC2 affinity and CPU32–81's distinct physical socket0 cores. No foreign
  process, lock or campaign was changed.
- [D] Retain that admission-only failure; stop only our waiting guard and
  restart a fresh bounded pilot. All our pilots now use the existing global
  heavy lock plus `TMP/locks/parallel-pilot-cpus32-81.lock`, leaving the reserved
  socket1 lock untouched. Update only unexecuted plans/test-driver lock paths
  and rebind their hashes. CPUs, inputs, headroom, timing and acceptance rules
  are unchanged; this does not require a Rust rebuild.

### 2026-09-29 19:30 UTC — optimized Stage B build passed

- [M] The clean `e1bdb9e7` campaign build completed: exit0, no stop reason,
  3,990.666s guarded wall,4,394.775s user CPU,163.678s system CPU,
  maximum single-child RSS17,309,628KiB; minimum host available696,641,871,872B.
  Cargo confirms the actual CLI target at opt3, no debug assertions or debug
  information, unchanged fat-LTO/one-codegen-unit campaign profile. Evidence:
  `TMP/codex-parallel-campaign.oiPK29/campaign-build-e1bdb9e7/`.
- [D] `stage_a_release` freezes that executable, then owns the already approved
  rescue and M1–M4 lifecycle checks. The measurement agent remains on standby;
  no competing heavy job, production launch or speed claim follows from build
  success. Compilation is excluded from all upcoming solver measurements.

### 2026-09-29 19:25 UTC — delivery reserve; alongside launch reviewed

- [M] The tentative six-hour target is reached while the unchanged optimized
  campaign build remains in its final binary fat-LTO stage. No compiler error
  is reported. Root disclosed the delay and is using the agreed reserve; the
  latest user directive permits necessary correctness/performance overrun.
  This is not a final-binary acceptance or timing result. No more features are
  being added before the frozen-build gates.
- [M] `bounded_ordering_pilots` updated the workspace-local launch draft, and
  root independently reviewed it: new `rustred` / `codex_astra` tab, W50 on
  CPUs32–81, requested400GB,5% save margin,150GB live host floor. LC2 stays
  running on its existing resources; rollback stops only the new campaign.
  Draft: `TMP/codex-stage-a.2RU3AX/stage-b-fresh-campaign-draft.md`, SHA256
  `35f3541e7d06ce362eb40f8029bb0749931c29133da9524d57beb4d8c5d28d2a`.
  No production preparation or launch was executed. Final executable identity
  and measured width qualification remain mandatory before handoff.
- [D] Final-binary rescue/lifecycle checks precede the first matched FG pair;
  root reviews its receipts before authorizing the wider matrix. All ordering
  variants reuse this same optimized executable, with no rebuild per input.

### 2026-09-29 18:38 UTC — ordering inputs ready; independent delivery audit

- [M] `bounded_ordering_pilots` prepared the helpers-first control with the
  existing stager:16 owners/6,562,373bytes and all58 exact query objects/roles
  retained; only order differs. Setup2.04s including Nix/lock, not solver time.
  Root reviewed the data/plan diff. Original28 arms and seven controls are
  unchanged; two helper-first arms append at28/29, using the same binder.
  Evidence: `TMP/codex-ordering-study.7nspU4/FIXTURE_PREPARATION.md` and
  `matrix-ordering.diff`; updated unbound matrix SHA256 `af28bc44...cbbe9`.
- [D] Execute combined controls in order8,9,28,29,10,11: ReadyA1,FIFOB1,
  H1,H2,FIFOB2,ReadyA2. This keeps both engine pairs adjacent and supplies a
  contemporaneous preserve/helpers-first two-pair comparison. No solver has
  run, and no speed benefit follows from setup or reordered input alone.
- [M] The independent ordering agent also audited code it did not implement:
  protected-prefix fix PASS. It requested an existing remapped-query negative
  and a specifically abandoned-root AllRoots failure; the execution lane
  accepted both checks. Audit: `TMP/codex-ordering-study.7nspU4/DELIVERY_AUDIT.md`.
- [D] Provisional alongside allocation is W50/CPUs32–81, requested400GB, live
  host reserve150GB, leaving LC2's CPU128–227/600GB/50GB-floor policy unchanged.
  These are distinct physical cores. Requested ceilings are not reservations;
  final launch remains contingent on tests and current available resources.

### 2026-09-29 18:23 UTC — final optimized build active

- [M] `stage_a_release` started the one optimized CLI build from clean
  `e1bdb9e7` (engine correction `d40c1b76`), with unchanged campaign profile,
  pinned clean Symbolica, CPUs0–15, heavy/build0 locks and eight build workers.
  Evidence: `TMP/codex-parallel-campaign.oiPK29/campaign-build-e1bdb9e7/`.
  No engine input variant triggers another build.
- [M] The exact pre-fix rescue regression rerun preserved its real checkpoint
  and failed as expected in1.149s, with the same sole protected-prefix finding.
  Fixture: `TMP/rustred-verify-closure-epoch-g2-rescue-required-825707-0`;
  receipt: `TMP/codex-parallel-validation.RPJKV5/native-rescue-prefix-before-f3f707af/`.
  Final optimized CLI checks remain pending and must not edit checkpoint state
  to obtain a PASS.
- [D] A requested extra audit-agent slot was unavailable due the thread limit.
  Root and `stage_a_release` retain separate implementation/review duties;
  `bounded_ordering_pilots` prepares experiments but cannot self-approve them.
  No additional heavy job runs beside the optimized build.

### 2026-09-29 18:22 UTC — one final cold-reader defect; bounded ordering lane

- [M] Corrected app-opt1 build completed in4,565.855s. The full actual suite
  then ran233.246s:1,116 passed,one failed,12 ignored,zero filtered and no
  unexpected SKIPPED markers. Existing W50 tests and all earlier corrected
  paths passed. The remaining failure is the real rescue cold-verification
  regression, not a build, license or timing issue. Receipts:
  `TMP/codex-parallel-validation.RPJKV5/native-{build,app}-f3f707af-opt1/`.
- [M] Diagnosis: the verifier inserts both initial and amended admitting roots
  into one map, then incorrectly demands that the whole map equal only the
  original prefix. Root narrowed that one inventory check to IDs below p0;
  independent `stage_a_release` review passed. Original per-query equality,
  roles, amendment authentication, containment and missing-prefix rejection
  remain. Do not remove the positive rescue regression or weaken its assertion.
- [D] For this narrow reader-only correction, retain the genuine failing native
  fixture and exercise the final optimized CLI's cold PhysicsQueries PASS,
  AllRoots non-PASS, unchanged files, resume/digest parity and altered-amendment
  refusal. This is actual final-code regression evidence plus the1,116 unchanged
  full-suite passes, not a claim that the final full native suite was rerun.
  It avoids another monolithic app-test rebuild solely for this predicate.
- [D] User requests delivery for a new `codex_astra` tab alongside LC2, so
  final resources must be disjoint rather than reusing LC2 CPUs128–227. Updated
  the shortened plan. No production operation was performed.
- [M] New delegated `bounded_ordering_pilots` lane prepares input-driven pilot
  commands with independent root review; no heavy job is authorized yet.
  It identified a useful exact-scope four-loop comparison: the58-row control
  appends16 broad R12 helpers late, whereas the existing helper-first staging
  can move them before narrower queries without changing any row or role.
  Single-query finite/hot controls cannot measure query-order effects.

### 2026-09-29 17:54 UTC — owner has resumed LC2 with Stage A

- [M] Read-only inspection shows run `20260929T165630.299496Z` using the
  delivered Stage A executable SHA256 `0995f0fd...739c21`. The owner retained
  183 query rows, Ready publication, G2 Off and100 reserved workers. Root did
  not start, stop, resume or edit the production campaign.
- [M] At the snapshot it was actively routing:120,836,403 scheduled domains,
  48,098,563 local completions,42,348,337 pending and zero frontiers. Its saved
  checkpoint generation8 reports7.279s save time. These are ongoing-run
  counters, not a closure proof, matched speed measurement or completion ETA.
- [D] Stage B remains a separate fresh CP6 campaign pending native and
  performance gates. Existing LC2 retains its independent rollback/continuation
  path; all development continues outside CPUs128–227.

### 2026-09-29 17:54 UTC — keep ordering and family experiments input-driven

- [D] User reiterates that different families and supported orderings should
  use the same backend executable. The matched matrix already follows this;
  added the instruction explicitly to `SHORTENED_PLAN.md` and the ordering
  review. No engine change or additional generation experiment was introduced.
- [M] Root checked the public CLI parsers and generator request: both
  `family-candidates` and `family-close` accept runtime `--permutation`, with
  topology-independent validation. Family input, workers and campaign dispatch
  are runtime choices too. An arbitrary source-row/pivot-policy API was not
  established and is not claimed. Changed algebraic priority entails new owner
  programs and their validation, not Rust recompilation.
- [M] Corrected full app native build remains active in the isolated validation
  tree. Its rebuild is for the integrated engine/test corrections, not an input
  family or ordering change. The optimized campaign build and measurements
  remain subsequent gates; no new throughput result exists yet.

### 2026-09-29 16:53 UTC — build-profile failure and explicit four-loop gate

- [M] Corrected `f3f707af` app-opt0 compilation failed at linking, not type
  checking or test execution. App object text totals2,294,648,001bytes, exceeding
  the signed2GiB relative-address range; binutils reports PC32/GOTPCREL overflow.
  No executable was emitted. Guard exit101/no resource stop,1178.571s,
  minimum host headroom663,890,014,208bytes. The failure receipt is retained in
  `TMP/codex-parallel-validation.RPJKV5/native-build-f3f707af-opt0/`.
- [D] Return to the known-working app-opt1 correctness profile; no speculative
  linker/code-model workaround and no weakening of the complete-suite gate.
  `epoch_g2_rescue_impl` reached its thread limit after delivering the diagnosis;
  active `stage_a_release` takes over the build/run, with root resource oversight.
  The separate optimized campaign binary and its timing remain pending.
- [M] Stage A frozen binary SHA256 rechecked successfully; its clone is clean
  at pushed931d006c. The user received exact pause/preview/upgrade/resume commands
  for their existing Zellij tab. Root did not operate production.
- [D] User explicitly requires the combined four-family physics-capped control
  to be faster or at least on par. Add this no-regression gate to the shortened
  plan and delivery report, using repeated matched current-Ready+Union versus
  rolling+Union runs, identical scope/resources and common cold-All cost.
  Preserve all existing control queries, including helpers. No Stage B timing
  has run yet; no performance pass is claimed.

### 2026-09-29 16:40 UTC — settle the matched deliverable before measuring

- [D] Root and independent `stage_a_release` review agree that the common
  deliverable is durable, resumable records plus cold-All proof of the same
  frozen scope. Native-plus-cold time is a legitimate **end-to-end verified-
  closure pipeline** comparison; merely producing different JSON layouts is
  not a missing mathematical operation. Actual CP6 resume and intact records
  remain gates, as do two valid finite/hot pairs for a1.5x switch recommendation.
- [D] Report native-only timing separately and disclose any gain from avoiding
  full-result materialization. Do not attribute it all to faster inspections or
  claim drop-in compatibility with a consumer requiring Ready's complete JSON
  and paired Python proof. Charge a specific downstream adapter if one becomes
  part of the workload. No such additional consumer is in this delivery's scope.
  This resolves the earlier finalization-accounting reservation **before any
  Stage B arm has run**; it does not relax cold verification or query scope.

### 2026-09-29 16:36 UTC — measurement budget review and production observation

- [M] Independent measurement-plan review (`stage_a_release`) found the initial
  per-child limits did not leave enough room for both cold checks within one
  30-minute pilot. The TMP-only matrix and binder now derive native900s/grace300s
  and each verification150s/kill60s from the registered inputs. Two verification
  stages plus180s preparation/admission/reporting reserve total1800s. Root checked
  all26 arms and the binder; cumulative elapsed-time enforcement remains required
  at execution. No benchmark has run, and no timer change is a success result.
- [M] Cold timing uses the existing guard's `elapsed_seconds`, not a nonexistent
  `wall_seconds`. CPU work and wall speedup are separate; differently scoped RSS
  peaks are not added. CPUs32–47 are physical cores on socket0/NUMA1, despite the
  historical `socket1.lock` filename. Production128–227 remains protected.
- [M] Read-only LC2 observation around16:30: heartbeat elapsed60430.895s,
  114,740,628 scheduled domains,45,452,322 native/completed,40,291,709 queued,
  zero frontiers. This does not imply a completion ETA or establish closure.
  No production inputs, process, checkpoint or steering were changed.

### 2026-09-29 16:30 UTC — native failures corrected; full rerun authorized

- [M] Integrated `c25bcf82` as `f3f707af`: the obsolete lockstep `config.g2`
  refusal is removed without changing the G2 planner, exact merge gates or
  publication semantics. Both root and `stage_a_release` independently reviewed
  the public lockstep path. The real W1 Union end-to-end test explicitly checks
  lockstep scheduling, actual G2 records, cold verification and resume parity.
- [M] Nine fixture corrections retain their negative guarantees: mismatched
  checkpoint binding fails, future versions and reserved bits fail, runtime-only
  watermarks are rebuilt for fresh replay, rolling fixtures enable checkpoints,
  and rescue stops before publication with a legal positive event allowance.
  A new test covers old-version replay with refreshed sequence and watermark.
- [D] `epoch_g2_rescue_impl` owns the full final application-suite build and run
  on the clean validation tree at `f3f707af`. App opt-level0 is a correctness-only
  build override; core and Symbolica remain optimized. Build CPUs16–31, tests
  CPUs64–127, heavy/build1 locks, strict license, one test thread. No optimized
  candidate build or benchmark is running concurrently. All ten earlier failures
  remain recorded until the complete corrected suite actually passes.

### 2026-09-29 16:20 UTC — native suite results and focused corrections

- [M] Full core suite on `f083f254`: 2,845 passed, zero failed,32 ignored,
  zero filtered; strict-license guard exit0/no stop,132.215s. Full app suite:
  1,103 passed,10 failed,12 ignored,zero filtered;231.252s, exit101/no resource
  stop. No SKIPPED markers; both named actual W50 mechanical tests passed.
  Evidence: `TMP/codex-parallel-validation.RPJKV5/native-{core,app}-f083f254/`.
  The failed app receipt is retained and is **not** a green native milestone.
- [M] One real integration gap identified: public CP6 lockstep+Union reaches an
  obsolete `config.g2` rejection in the responsive controller. This must be fixed
  and exercised; changing the end-to-end test to rolling would hide the bug.
  The final CLI M4 control intentionally remains lockstep+Union.
- [M] Remaining failures concern obsolete unsupported-G2 verifier expectations,
  seven-versus-eight ledger count fixtures, newly valid node flag/mutation values,
  runtime-only replay lookup watermark expectations, three rolling fixtures
  lacking their required checkpoint configuration, and a zero event allowance
  rejected by public request validation. The G2/rescue lane owns the complete
  diagnosis/fix mapping; root and `stage_a_release` independently review it.
  Assertions must test the current guarantees, not be dropped to obtain a pass.
- [D] A public-API harness for the first stale test alone is no longer sufficient
  after the complete ten-failure inventory. Rebuild and execute the **full final
  app correctness suite** after the audited corrections, using app opt-level0
  if needed to avoid another81-minute LLVM optimization pass. Core and Symbolica
  remain optimized. Such a binary is never used for solver timings; the separate
  optimized campaign build and final public-CLI gates remain mandatory.
- No optimized Stage B build or performance arm has started. No production
  operation or alteration of the116 required queries has occurred.

### 2026-09-29 16:10 UTC — consolidated native test executables built

- [M] Frozen engine source `f083f254` finishes core/app native-test compilation
  successfully: exit 0, no stop reason, 4,910.030 s (81m50s), minimum host
  available memory 503,444,058,112 bytes, maximum single-child RSS 86,848,380 KiB.
  Evidence: `TMP/codex-parallel-validation.RPJKV5/native-build-f083f254/`.
  Both Cargo-emitted library-test executables exist; the G2/rescue lane now
  executes full core then full app on CPUs64–127 under heavy/build-1 locks.
  These are correctness binaries (app opt-level1), never performance binaries.
- [M] Final-CLI monitoring/lifecycle driver is independently source-reviewed:
  `TMP/codex-stage-a.2RU3AX/final_cli_monitor_smoke.py`, four bounded public
  Python/CLI cases using existing process guards. No native case has run yet.
  Actual unknown/positive activity observations, stop/resume, lockstep multi-cut
  retirement and cold verification remain required as documented at15:34.
- [M] Fresh Stage B owner commands are prepared, not executed:
  `TMP/codex-stage-a.2RU3AX/stage-b-fresh-campaign-draft.md`.
  Exact existing183 query objects/order and all67 saved-owner files are reused;
  only the explicit116/67 role declaration is added for the fresh campaign.
  No owner regeneration, production checkpoint import or production action.
- [M] Existing matched-matrix binding helper received independent source review
  by `epoch_g2_rescue_impl`; template identity checks address its one finding.
  `TMP/codex-rolling-measurements.eBFGjR/bind_plans.py` has not bound or launched
  any arm. Query digests use RustRed's existing compiled BLAKE3 dependency, not
  an independent implementation. The future optimized binary remains unbound.
- Next: inspect actual native-suite results before the optimized CLI build;
  correctness fixes only. Final-source monitoring units remain typechecked,
  while their actual final-CLI execution gates are still pending.

### 2026-09-29 15:34 UTC — full Python regression and native validation scope

- [M] Full example/Python discovery passes 311 tests in 24.487 s with one
  documented slow skeleton-enumeration skip (310 executed passes), guarded
  command 25.171 s: `TMP/codex-epoch-final-python-1538/`.
  The preceding run found one stale policy-loop setup that supplied the required
  transfer lookahead only for Ready, not newly included Epoch. Test-only
  `ec1dfd28` supplies it for both and adds Epoch refusal cases without weakening
  any assertion. Independent source review by `stage_a_release` passes.
  Failed receipt `TMP/codex-epoch-final-python-1533/` is retained.
- [D] Avoid a second monolithic native test compilation solely for the final
  telemetry changes **only if** final optimized-CLI gates cover those changes.
  The source auditor identified the exact conditions: actual W1 unknown state,
  threaded live heartbeats, saved/cancelled unknown and joined-zero states,
  configured reservation counts including an explicit inspector override,
  unknown display semantics, stop/resume/cold parity, and a lockstep run with
  more than one cut for the added returned-receipt retirement path.
  Final-source test assertions remain compiler-checked, not claimed executed;
  full native suite evidence remains attributed to `f083f254`. Existing exact
  pool harness, Python checks and source audit supplement but do not replace
  those final executable gates. If a required path is not exercised, report
  the gap and run its missing check rather than declaring final-source success.
- Next: finish current native build/tests, freeze and build the optimized final
  candidate, then execute the final-runtime gates and matched controls. No new
  architecture or optimization work added.

### 2026-09-29 15:31 UTC — final monitoring integration audited

- [M] Final adapter integration `e8ba4cc6` retains actual configured worker
  reservations and distinguishes explicit unknown Epoch activity from a legacy
  lean heartbeat with missing fields. Unknown does not silently become zero or
  carry forward stale activity. No invented preparation/duty timings; pending
  growth is unchanged. Independent source audit passes:
  `TMP/codex-stage-a.2RU3AX/stage-b-monitor-adapter-source-audit.md`.
- [M] The 37 monitor/metrics Python tests pass in 0.164 s (guarded whole command
  1.274 s): `TMP/codex-epoch-monitor-adapter.22BCOp/full-corrected/`.
  One stale expected label was corrected in the preceding failed run; that
  evidence remains preserved. This is not native or performance validation.
- [M] Final-source release compiler/test-type check passes in 49.266 s,
  source `e8ba4cc6`, evidence
  `TMP/codex-parallel-campaign.oiPK29/typecheck-final-monitor/`.
  Heavy core/app native-test build still uses frozen `f083f254`; the core
  executable has emitted, app code generation continues. The G2/rescue agent
  will execute full core then full app under the shared locks after a clean
  build receipt, including actual W50 mechanical checks. The final monitoring
  slice requires its own subsequent native check; do not attribute this older
  executable to final source.
- [M] Stage A source audit lane completed the measurement interpretation
  checklist and now prepares fresh Stage B launch instructions without starting
  production. No optimized Stage B build or solver pilot has run yet.

### 2026-09-29 15:04 UTC — Stage B implementation freeze and native build

- [M] Combined rescue/rolling source `f083f254` passes release compiler/test-type
  checks in 46.183 s. The actual core/app library test build is running in the
  isolated validation worktree, CPU16–31/heavy/build-1; its source stays frozen.
  Exact command and receipts:
  `TMP/codex-parallel-validation.RPJKV5/{typecheck-f083f254,native-build-f083f254}/`.
  No actual combined native test result is available yet.
- [M] The final planned activity-monitor slice is integrated as `d7ecf3ee` and
  `704fb8d9`. Independent audit by `stage_a_release` caught a CLI unknown-to-zero
  rendering defect; it is fixed, with legacy rendering unchanged. W1/nonpollable
  states remain unknown, live sampled computing callbacks are not described as
  CPU utilization, and joined workers report zero. Pending-growth math and
  rendering are unchanged. Pool harness passes in 2.254 s; this is only the
  std-only pool component, not the complete engine.
- [M] Source audit receipt:
  `TMP/codex-stage-a.2RU3AX/stage-b-activity-source-audit.md`.
  Activity census is lazy behind the existing five-second gate; bounded per-cut
  receipt retirement remains separate lifecycle work. The implementation is now
  frozen except for independently identified correctness/integration fixes.
- [M] Optimized-build worktree prepared at `TMP/codex-parallel-campaign.oiPK29/repo`,
  with a normal independent copy of Stage A's idle Cargo cache (distinct inodes,
  no fingerprint/mtime manipulation). A lightweight metadata check runs there on
  CPU0–3/build-0 while the heavy native test compilation continues. No optimized
  Stage B executable or performance result has been produced yet.
- [M] Stable `fable_5_1` and its remote tracking ref now both point to delivered
  `931d006c`; root remains `fable_5_1_parallel`. Existing unrelated Cargo/feynkit
  edits and untracked collaborator files remain untouched.
- Next: execute the combined native suite with sufficient affinity for actual
  W50 mechanics, rebuild/test the final monitor slice, freeze the optimized
  candidate, then run the independently reviewed matched control matrix. No new
  optimization investigation is being added to the shortened delivery.

### 2026-09-29 14:47 UTC — Stage A native delivery gates pass; rescue integrated

- [M] Stage A optimized campaign build completed in3566.486s, source
  `986d046e`, executable SHA256
  `0995f0fda2637eb4bf0bdc5b46249aba9c6f4ba513196143a2b07d3aaa739c21`.
  Frozen executable is `TMP/codex-stage-a.2RU3AX/candidate-bin/rustred-986d046e`.
  Root independently checked its hash, CP5 probe and delivery evidence.
- [M] Genuine old-binary pause/new-binary resume drills pass for Ordered/W6
  and Ready/W6, both G2 Off, with unchanged input bytes/options, durable old
  checkpoint copies, history-only rollback probes, native full cold reinspection
  and Python audit. Ordered:98909 domains/248 roots; Ready:98881/248; all closed.
  Whole drills42.018s/39.443s are correctness checks, not solver-speed comparisons.
  Evidence: `TMP/codex-stage-a.2RU3AX/native-upgrade-smoke-{ordered-v3,ready}/`.
  Earlier controller-field/thread-cap mistakes remain recorded as incomplete.
  These are representative controls, not a60.7GB full LC2 replay.
- [M] Stable delivery source/docs independently approved and fast-forward pushed
  as `931d006c`; remote identity verified. Owner upgrade command communicated.
  No production operation performed. Plain upgrade retains frozen G2 Off;
  do not attribute the earlier Union savings to this compatible upgrade.
- [M] CP6 rescue source `69490a40` integrated as `1bacf9d9`, retaining adaptive
  metadata beside append-only amendment identities. All new CP6 checkpoints use
  manifest2/scalar3/semantics3; CP5 is unchanged. Root CLI/Python bridge
  `5742a3bb` requires a reconciled durable final handoff before automatic rescue.
  StoreOwner quarantine/replica and replay-watermark composition is the last
  narrow integration slice before consolidated native compilation.
- [M] Combined Python lifecycle suite passes103 tests in12.223s; evidence
  `TMP/codex-epoch-rescue-frontend-1439/`. Earlier1437 invocation had two wrong
  module names and is not counted. CP6 measurement-adapter tests pass12 in
  `TMP/codex-epoch-cp6-controls-1430/`. These do not replace actual native gates.
- [M] Rolling source passed independent audit by `stage_a_release`; root
  separately audited adaptive dispatch and the rescue steering correction.
  Native performance remains unmeasured. A frozen7-scope/24-arm control plan
  plus conditional W50 pair is prepared, not executed, in
  `TMP/codex-rolling-measurements.eBFGjR/`. It uses the same future optimized
  executable for Ready+Union and rolling+Union, including common cold-All costs.
  This is an allocation queue, not a promise to fit every worst-case arm.

### 2026-09-29 14:25 UTC — integrated metadata checks and live bottleneck snapshot

- [M] Integrated rolling/adaptive source at `ed5d8000` passes actual
  `cargo check --release --tests --locked --offline` for core and app. The first
  check at `23161406` found two missing test imports, fixed at `1504f519`; the
  retry passes. Evidence: `TMP/codex-parallel-validation.RPJKV5/`, separate clean
  worktree and normally copied cache, CPU16–19/build-1, one Cargo job. These are
  compiler/type checks, not executed tests or performance results.
- [M] Rolling implementation is integrated via `e65ea9fb`, `503d1b62` and
  `23161406`; `ed5d8000` adds native interruption/replay tests for both width
  directions, lookup modes and adaptive observations. Rescue integration is
  still pending and must precede the consolidated native execution gate.
- [M] Python rescue-bridge checks pass39 in4.215s
  (`TMP/codex-epoch-rescue-steering-1421/`). Independent review by
  `epoch_rolling_impl` caught an event/result handoff gap: automatic CP6 rescue
  must require the final reconciled checkpoint, not merely a saved receipt in
  result.json. Root fixed it and added missing/mismatched terminal regressions;
  follow-up source review passes. The native rescue bridge is not yet delivered.
- [M] Read-only LC2 heartbeat at14:18:56UTC:105,297,082 discovered domains,
  41,789,491 local completions,36,520,988 pending,0 active inspectors and201
  awaiting publication at that instant. Its preceding-hour inspector mean is
  2.20; ordered-commit duty51.0%, preparation37.3%; reported process RSS50.8GB.
  Six of67 initial obligations are conservatively closed; zero frontiers is
  not a completion forecast. These observations support the publication/lookup
  focus but do not measure the new engine. No campaign state was modified.
- Native code generation will be consolidated after the imminent rescue slice,
  rather than repeatedly rebuilding the app for each addition. Stage A final
  link continues independently; its compatibility smoke retains heavy-lock
  priority. Source freeze target remains15:55–16:25UTC.

### 2026-09-29 14:15 UTC — adaptive review and integration

- [M] Root independently reviewed the complete bounded adaptive implementation,
  candidate conservation, retry precedence, oldest-job fairness, score arithmetic
  and checkpoint/restore validation. Source review passed; its ten new focused
  native tests have not run yet. `e499e531` is integrated as `6eceea28`, preserving
  both rolling cut/window metadata and adaptive state. The public interface is
  `53db3b35`; rolling-controller observations are still being connected.
- [M] Stage A's application library compiled without errors; final executable
  link/LTO remains active. The production campaign remains untouched. No ready
  announcement or performance claim precedes actual compatibility checks.
- [M] Root reviewed the rolling controller draft's bounded sequence cuts,
  concurrent immutable lookup leases, partial replay handling and stop path.
  Oversized publication cuts use a quiescent path rather than a mathematical
  truncation; this and late quarantine require native regressions before release.
- [M] New unrelated `CITATION.cff` is present at root and is preserved unstaged,
  alongside previously recorded user changes.

### 2026-09-29 14:06 UTC — G2 integration and concurrent adaptive dispatch

- [M] Fresh Epoch Union integrated as `714970bc`. Root reviewed exact residual
  coverage, prior-dispatch stamps, lender scope restrictions, publication
  eligibility and the independent cold-record adapter. The binding merge
  preserves both Union and rolling nondefault keys. Native tests are pending.
- [M] Repeating the broader Python controls with eight permitted CPUs passed
  all80 (`TMP/codex-epoch-rolling-frontend-regressions-1350-w8/`), confirming
  the earlier resource-error precedence was harness affinity, not a regression.
  The combined adaptive/rolling/lookup/checkpoint and legacy frontend run now
  passes100 (`TMP/codex-epoch-adaptive-frontend-1404/`,12.663s test time).
  These use fake executables and do not establish native engine correctness.
- [M] The rolling lane's std-only pool check completed119 later jobs while
  an older worker was held; duplicate submission, retirement and cancellation
  checks passed (`TMP/codex-rolling-pool.E4ZDBn/`). This is a component
  concurrency result, not a whole-campaign speedup.
- Root's review of rolling primitives identified oversized single-cut delta
  and quarantined equal-image replay edge cases. The implementation lane is
  addressing both: the journal threshold is backpressure, not a mathematical
  work cap; a large valid cut must make progress through a quiescent path.
- `stage_a_release` now implements the separable bounded adaptive-dispatch
  module while its isolated Stage A build runs. Root wires the public
  `epoch_dispatch` policy; the rolling lane owns observation/controller hooks,
  and the G2/rescue lane owns checkpoint integration. The policy remains
  opt-in, must preserve retries and oldest-job fairness, and cannot change
  algebraic pivots. Core implementation audit will be independent of its author.
- [M] Stage A missed the13:55UTC target: compilation remained healthy at that
  time, with actual pause/resume/cold checks still to follow. The user was
  notified; no older binary was silently substituted. Stage B never waited.

### 2026-09-29 13:50 UTC — rolling interface and bounded ordering review

- Root delivered opt-in `epoch_rolling` request/CLI/Python wiring and frozen
  CP6 binding; false keeps historical argv/digest behavior. Independent source
  review by `stage_a_release` found no blocker. Eighteen focused Python
  steering tests passed (`TMP/codex-epoch-rolling-steering-tests-1344/`);
  native integration/window/runtime tests remain pending. The four earlier
  native lifecycle fixes are committed at `6228d34d`, independently reviewed,
  not yet re-executed in the new binary.
- A broader frontend run passed77, failed1 and skipped2. The failure is a test
  asking for six workers under a two-CPU test affinity, so it receives the
  resource-admission error before the expected containment diagnostic. Retain
  the failed receipt (`TMP/codex-epoch-rolling-frontend-regressions-1347/`);
  rerun with sufficient allocated test affinity, without weakening assertions.
- `epoch_g2_rescue_impl` delivered source `77e8ae72`: fresh Union planner,
  residual publication, variable CP6 anchors, restore and independent cold
  reader. Root review/integration is next; no native PASS yet. Rescue and
  activation guards remain in force until the next slice is complete.
- The delegated algebraic-order review recommends retaining saved natural
  programs and existing helper-first input order. Past source-row permutations
  changed RHS size but did not remove the exceptional obstruction. No valid
  alternative owner library is ready. Findings, negative evidence and the
  narrowly justified later prescreen are preserved in
  `docs/research/parallel_delivery_ordering_review_2026-09-29.md`. This is not
  evidence of an optimal ordering or a new performance result.

### 2026-09-29 13:38 UTC — Stage A compiling; real Epoch tests expose integration fixes

- [M] Root branch is now `fable_5_1_parallel`, pushed at `295187a4` after
  integrating the reviewed private Epoch/tooling lineage. Stable documentation
  approval is `e56b8cdb` on `fable_5_1`. Unrelated work remains untouched.
- [M] `stage_a_release` owns the independent clone
  `TMP/codex-stage-a.2RU3AX/repo`, source `986d046e`, adding only the reviewed
  geometry-buffer and lean-telemetry slices. Its optimized campaign build
  acquired the heavy/build-0 locks at13:32UTC on CPUs0–15. Root independently
  reviewed the nine-file source delta and representative FG248 upgrade harness.
  Thirteen existing Python upgrade/rollback checks passed; actual native
  pause/upgrade/resume and cold verification await the build. No live LC2 writes.
- [M] Root ran the actual9ec native walking focus, not a metadata-only check:
  `TMP/codex-epoch-s3.JjASCU/public-9ecad89f-app-focus/{request,result}.json`,
  648 passed,4 failed,12 ignored,145.83s test time. W50 capacity omissions are
  not claimed covered. Failures expose one real public-resume readiness bug,
  its two lookup-mode tests, a stale resume-refusal assertion, and a fixture
  whose whole-ray input cannot produce the asserted snapshot miss. Narrow
  fixes are implemented at root and awaiting independent review/re-execution;
  do not count them as passing yet.
- [M] `epoch_g2_rescue_impl` is implementing the existing exact G2 planner's
  CP6 adapter and persistence, then quarantine/rescue composition. The public
  guard stays closed until all authority paths are connected.
- [E] `epoch_rolling_impl` selected bounded rotation of two globally shared
  lookup-only replicas alongside the mutable canonical store. This reuses
  existing exact/SoA index semantics without cloning the whole store per job
  or publication. It excludes CAS payloads, edges, tracker, records and ledger.
  Source-size estimate at96M domains: about47GB nominal additional lookup
  storage, potentially60–80GB with capacity slack; not a measured RSS result.
  Bounded delta backlog/backpressure and true publication/inspection overlap
  require tests. This first implementation deliberately trades bounded extra
  memory for a smaller, auditable change; persistent shared pages are deferred.
- Next: finish/retest narrow public lifecycle fixes, integrate G2/rescue and
  rolling source, then freeze the consolidated build. Stage A does not block
  Stage B implementation. No speed or five-loop completion claim follows from
  the architecture choice. Algebraic-pivot review is queued for the next free
  agent slot; adaptive dispatch remains a separate deliverable.
### Stable-branch event retained on merge — 2026-09-29 14:41 UTC

- [M] Independent stable clone native source `986d046e` adds only the two
  reviewed geometry-buffer/lean-telemetry cherry-picks to `e56b8cdb`; no
  private Epoch code. Symbolica stays clean `ef0db494`.
- [M] Real optimized campaign CLI build passed after 3,566.486 s, guarded
  on CPUs 0–15 with heavy/build-0 locks and 250/150 GiB headroom floors.
  The build exceeded the 30-minute target under the approved build/correctness
  exception; Stage B continues in the root `fable_5_1_parallel` checkout.
- [M] New frozen binary SHA-256
  `0995f0fda2637eb4bf0bdc5b46249aba9c6f4ba513196143a2b07d3aaa739c21`.
  Read-only LC2 input/options checks and CP5/schema5/semantics1 probe match.
  No production write, launch or signal; the compatible upgrade keeps G2 Off.
- [M] Ordered/W6 and Ready/W6 FG248 drills passed genuine old-binary durable
  pause, candidate upgrade/resume, unchanged input/options, public history
  rollback probe and full cold verification: 248/248 roots in each, all
  98,869 / 98,841 native records reinspected respectively. Two earlier harness
  setup failures remain recorded as incomplete, not passing gates.
- [M] Evidence: `TMP/codex-stage-a.2RU3AX/`; delivery and owner commands:
  `docs/research/fable51_stable_upgrade_2026-09-29.md`. Independent final
  source/binary/receipt/delivery review passed; stable push follows the
  documentation commit. No full-size
  LC2 replay, actual post-upgrade old-binary rollback, or new speedup claimed.

### 2026-09-29 13:25 UTC — approval; concurrent Stage A build and Stage B implementation

- User approved `SHORTENED_PLAN.md` and started the clock: T0=13:25UTC,
  target19:25UTC plus one-hour reserve. Explicitly allow necessary overruns
  rather than compromising implementation. Stage A builds in an isolated
  workspace-local repository clone; Stage B proceeds at the root on
  `fable_5_1_parallel` without waiting for Stage A or production actions.
- The latest approval also requests a later delegated algebraic-pivot/input
  review using existing four-loop knowledge, distinct from adaptive dispatch.
  No open-ended research or regeneration is authorized implicitly.
- Root re-read main state: `6a153d3b`, branch `fable_5_1`; unrelated Cargo.toml,
  feynkit and reference/untracked work retained. No AGENTS.md was found by the
  scoped repository search. The completed9ec build receipt remains available
  for actual test execution; no new native PASS claimed yet.
- Tool goal status still reports paused, but this explicit user go-ahead
  authorizes implementation. No goal reset/recreation or status falsification;
  the tool has no resume setter. Current agents can execute (planning probe
  succeeded), and separate implementation/build/audit lanes are being assigned.

### 2026-09-29 — shortened two-branch proposal; awaiting approval, clock not started

- User requests a roughly30-minute compatible `fable_5_1` milestone, followed
  by5h30 of parallel implementation on `fable_5_1_parallel`, with one hour of
  reserve. Root wrote `SHORTENED_PLAN.md`; it is a proposal, not authorization
  to start. No implementation, new builds/tests, branch creation or production
  changes occurred. Previous absolute deadlines are superseded only on approval;
  T0 is that go-ahead. Tool-managed goal remains paused.
- A fresh planning subagent `short_plan_feasibility` successfully ran, so the
  earlier agent-capacity refusal is not currently reproduced. Its read-only
  review confirmed two delivery risks: an optimized CLI build previously took
  about57minutes, and public performance-only upgrade does not enable native
  G2 activation or migrate roles. Those limitations are explicit in the plan;
  an Off resume must not inherit the measured Union speedup claim.
- The one-time independent plan audit recommended source freeze at T+2h30–3h
  and explicit tests for genuinely adaptive, bounded, fair and replayable
  dispatch ordering. Both were incorporated. No research or new acceptance
  framework is planned; deferred NUMA and other nonessential work stays out.
- [M] Root read the now-completed recovery receipt:
  `TMP/codex-epoch-s3.JjASCU/public-9ecad89f-build-recovered/result.json`,
  exit0/no stop,1486.728796s recovery-interval wall. The final incremental Cargo
  command finished in0.15s with `build-finished: success=true`; original build
  and recovery PIDs are absent. This establishes witnessed build completion,
  not execution of tests, an optimized CLI or original-build total CPU/RSS.
  Exact source/artifact identity must be verified before reuse after approval.

### 2026-09-29 12:57 UTC — revised Epoch delivery target; execution capacity unavailable

- User replaces the 3–4-hour legacy-first checkpoint with a 6–8-hour target
  for a useful fresh-start Epoch build. Latest plan section records the message
  verbatim and narrows the deliverable to bounded real overlap, G2/rescue
  composition, durable restart and representative measured controls. Target
  18:57 UTC, hard handoff20:57 UTC. Long-term extras and migration are deferred.
  A performance-gate failure cannot become a production-switch recommendation.
- [M] `get_goal` now returns **usageLimited**, not active. Root attempted
  `followup_task` to restart the G2 implementation lane; the tool rejected it
  with **agent thread limit reached**. No agent is claimed to be implementing
  the new plan. The existing goal is not recreated or reset to evade limits.
  Independent new implementation/audit work awaits restored execution capacity.
- [M] Existing guarded native compilation still runs: original PG235042,
  recovery monitor912342 under root session89700. Source/cache unchanged;
  no result receipt yet. Read-only source review for the architectural answer
  confirmed current Snapshot uses one shared Store and cannot mutate until
  readers release it. Real rolling execution therefore requires a storage/view
  lifetime change, not merely increasing batch size. Original protocol replica
  proposals are superseded by Symbolica thread-owned contexts; no replicas
  or historical canary shortcuts are being quietly adopted.
- Latest authoritative docs before this update are pushed at `bea52d29`.
  LC2 and all unrelated main edits remain untouched. No new benchmark or
  five-loop closure claim follows from this revised schedule.

### 2026-09-29 12:46 UTC — user sets 3–4 hour stable-delivery window

- Latest directive recorded verbatim in the plan; target handoff 15:46 UTC,
  hard cutoff 16:46 UTC. It supersedes waiting for every experimental idea.
  Prioritize a usable tested legacy G2/rescue + reviewed small-optimization
  build; retain unfinished Epoch integration separately. Resume is not ruled
  out for the legacy release, but requires a passing upgrade/replay check;
  future Epoch deployment needs a fresh campaign. LC2 stays untouched.
- All three current subagent turns terminated with account-quota errors.
  Their already frozen reviewed work and receipts are retained. Root can
  consolidate that work; new independently audited features cannot be
  promised while this persists. G2-Epoch source worktree remains clean, with
  its pre-edit implementation contract preserved. No half-enabled feature.
- [M] The executor loss also removed the original build guard/session while
  its exact Cargo PG235042 and rustc235283 remained alive (Cargo reparented to
  PID1). Root verified their fixed source/command and recovered resource/lock
  monitoring through the existing guard at
  `TMP/codex-epoch-s3.JjASCU/public-9ecad89f-build-recovered/`, root session89700.
  Recovery helper `TMP/codex-integration/adopt_235042_build.py` waits for that
  exact group to drain, forwards stop/headroom signals to it, then reruns the
  identical Cargo command incrementally for a witnessed exit. The original
  missing result is **not** a PASS; no build/cache/source or production
  process is replaced. Compilation remains separate from solver timings.
- Progress/audit documentation through 12:12 was committed and pushed as
  `0283fb8d`; unrelated main changes remain preserved.

### 2026-09-29 12:12 UTC — composition review and comparison-tool source gate

- [M] Root revalidated live native-build PID/PG235042 at 12:10 and 12:12; the exact `9ecad89f` validation worktree remains clean. Compilation continues, not a test/performance PASS. The previous turn made implementation/typecheck progress; this interval supplies verified waiting plus concrete composition findings, not another claimed implementation milestone.
- Independent reviewer `epoch_native_validation` found two composition obligations, confirmed against source: quarantine-aware lookup alone cannot support rescue because Epoch rejects duplicate historical images; and G2 eligibility must agree in planner, publication, validation and restore, not just filter candidates. The corrected read-only map is `TMP/codex-epoch-g2-rescue-composition-2026-09-29.md`; critique `TMP/codex-epoch-s3.JjASCU/epoch-g2-rescue-independent-map-critique.md`. These are integration gaps in a currently refused combination, not observed defects in LC2. Root assigned `joint_support_pruning` isolated G2-only source work while the frozen build runs; no integration/enabling/build before current native gates. Rescue follows as a separate explicitly versioned slice.
- [M] CP6 comparison tooling: root and `joint_support_pruning` read the six-file source and actual nine-test synthetic receipt, **9/9 PASS in 0.066 s**, guard exit 0/no stop, 1.171318 s. Author receipt `TMP/codex-epoch-cp6-tool-source-2026-09-29.md`; independent audit `TMP/codex-epoch-cp6-tool-independent-audit-2026-09-29.md`; execution `TMP/codex-epoch-cp6-tool-tests.4DIJ6z/final/`. No native child or real comparison ran. Source consolidation is approved; untouched historical Ready gates remain separate. Raw cold reinspection is still mandatory, and saved-worklist timing cannot claim an Epoch win by omitting finalization performed by Ready.
- [M] The exact reviewed tooling was committed privately as `8b64e114` and normally cherry-picked onto the private integration branch as **`3e3c4a1bb6c6c99f6684a87784b8fc3a79ff0993`**, tree **`6da77f206798159c5472b0a4f475587d291d7a41`**. Only six Python/documentation files were added; the live validation build remains on unchanged `9ecad89f`. Private source consolidation is not main-branch engine delivery.
- `active_goal_delivery_audit` is assessing a narrow bulk-edge merge candidate read-only: avoid redundant per-edge hash membership when a complete outgoing run is already sorted/unique. Mechanism, failure semantics, falsifier and representative test must be registered before implementation; no speedup is assumed.
- [M] Read-only LC2 snapshot around 12:11 UTC: **96,166,248** discovered, **33,666,450** pending, **38,013,111** local completions, native-process RSS **46,265,081,856 bytes**; checkpoint generation **5**, **60,699,295,300 bytes**, saved in **60.923 s**. The unchanged hour-window pending-growth metric is 0.3704; the legacy 6/67 counter is not closure of all116 required queries. Production remains running untouched; no ETA or restart recommendation.
- Main has an additional unrelated `Cargo.toml` version-bound edit (`=3.0.0` to `~3.0.0`), preserved and not propagated into the fixed validation source. No unrelated edits are staged.

### 2026-09-29 11:58 UTC — complete lookup-control candidate enters native build

- [M] Frontend author committed audited seven-file control `a3d1bcf3`. Root and `epoch_native_validation` independently reviewed it and read the pinned Python 3.11 receipt: **21/21 fake-only tests pass**, 2.046 s harness / 3.174350 s guard, exit 0/no stop, PG161283 drained. Evidence `TMP/codex-epoch-lookup-python.lUuQnq/focused/`; source receipt `TMP/codex-epoch-lookup-outer-source-2026-09-29.md`. This is steering validation, not a native solve; fake CP6 exits honestly with checkpoint-only status.
- [M] Conflict-free integration onto `775454eb` produced clean private **`9ecad89f610244bc82c2868252bcbfad8a4bd6f5`**, tree **`88f48c56af519e7dad948ff5c846ee88e410fdb0`**, covering public lifecycle plus native/CLI/Python lookup controls. The release metadata check then **passed**, exit 0/no stop, **43.176013 s** guarded wall (Cargo 40.21 s), PG215390 drained; exact source/dependency clean. Root read `TMP/codex-epoch-s3.JjASCU/public-9ecad89f-typecheck/verified-result.json`.
- [M] Native correctness build actually started at **11:58:00 UTC**, session 23517 / PG235042, receipt `TMP/codex-epoch-s3.JjASCU/public-9ecad89f-build/`. It uses the prior exact `cargo test --release --no-run --locked --offline -j8 --config 'profile.release.package.rustred-app.opt-level=1' -p rustred -p rustred-app --lib --message-format=json` command, exclusive cache, CPU16–31/heavy+build1, 250/150 GiB headroom guard. Compilation is separate from solver timing; core opt3/app opt1 is correctness-only. Earlier `29e30a79` frozen executables remain preserved.
- After actual successful build/drain and artifact/source verification, the executor is allocated sequential core applied-geometry and app walking/epoch focused execution with strict license checks. Report named tests, ignored tests and W50 capacity omissions explicitly. Stop on failure. CLI binary-unit tests and real process/cold controls remain later gates; metadata is not their execution. No performance campaign is launched.
- Separate agents continue the CP6 comparison acceptance tool and a read-only G2/rescue integration map. Reuse existing exact anchor/merge machinery; do not enable unsupported rescue by just removing its guard. Initial source inspection finds checkpoint/reader and quarantine/ledger differences that must be handled before combined Epoch deployment. Main still carries only validated earlier engine changes and pushed progress `9cf65fcb`; production remains untouched.

### 2026-09-29 11:52 UTC — combined metadata passes; Snapshot source review

- [M] The two reader-test imports were corrected privately as `308a692b`; root and independent reviewer verified exactly two import lines in one test file changed, with assertions untouched. Normal conflict-free integration produced clean **`a9bb0f637c2dda1aeb8656c483c33c5bed5cf23a`**, tree **`bc4ee5cce5df65a31eb2d66e786da64914a81e69`**.
- [M] Actual release metadata check **PASS**, exit 0/no guard stop, **29.182775 s** guarded wall (Cargo 26.12 s), owned PG136012 drained, source and Symbolica unchanged/clean. Exact command remains `cargo check --release --tests --locked --offline --config 'profile.release.package.rustred-app.opt-level=1' -j1 -p rustred -p rustred-app` under CPU16–19/build1/guard. Evidence: `TMP/codex-epoch-s3.JjASCU/public-a9bb0f63-typecheck/verified-result.json`; root read it. This is typechecking, **not native code generation, test execution, cold acceptance or performance**. All earlier failed receipts remain intact.
- Native Snapshot control is a separately frozen 11-file delta with six new unexecuted tests: `TMP/codex-epoch-lookup-native-source-2026-09-29.md`. Root and independent executor source reviews passed, with all frozen hashes checked. Author committed `949ef0e5`; root integrated it normally into clean `775454eb` (tree `4452f05358819b3b1ce99d668071a48f6371336a`) and checked that all five earlier I/O conversion fixes remain present. This later candidate is not yet typechecked. Default AllMiss remains unchanged; control selects existing lookup implementations, not a new algebra kernel or scheduling scheme. CLI/Python integration remains separately owned by `joint_support_pruning`.
- `active_goal_delivery_audit` moves to the separate CP6 comparison-tooling slice while its native source stays frozen. Preserve the old Ready acceptance gate. A CP6 exit-4/drained summary is only transport evidence: accepted comparisons additionally require independent raw cold full reinspection with `--no-result`, actual query-role scope, matching saved generation and no resource stop. Summary auditing must remain INCOMPLETE rather than being relabelled PASS. No campaign or benchmark is allocated before native gates.
- Native compilation is deliberately held until this small lookup control is consolidated, avoiding a second expensive build. Main still contains only previously validated engine changes; LC2 remains untouched and no restart is recommended yet.

### 2026-09-29 11:46 UTC — compile corrections reviewed and consolidated

- [M] The macro split `6ccfa76d` passed root and independent executor source review. Its actual metadata retry then failed after **16.169397 s**, exit 101/no guard stop, PG34916 drained. Macro expansion is fixed; 16 diagnostics expose two remaining integration seams: five `io::Error` conversions into the existing string-valued application error, and an inherent fixed-array method name colliding with `Read::bytes`. Evidence: `TMP/codex-epoch-s3.JjASCU/public-6ccfa76d-typecheck/`. Root read the diagnostics and receipt; no native tests ran.
- Separate authors supplied minimal corrections `6a135e52` (runtime error conversion) and `0c56304b` (reader helper rename). Root and `epoch_native_validation` independently reviewed both: error classification/text and reader framing/digest/EOF semantics are preserved. These do not add algebra or alter campaign scheduling.
- [M] Root consolidated them and the reviewed `--no-result` guidance into clean private **`7774604d747e19088270a34fd76aa9dab6448689`**, tree **`3d8239a4d6363133a47658ec1b0ac5e230d0da64`**, using normal fast-forward/cherry-picks with no conflict or manual merge edit. Repeat metadata check is allocated with unchanged resources/profile. Main and production stay unchanged; the later Snapshot-control drafts remain separate.
- Independent progress-log audit passed the measured/source distinction. Independent Snapshot-design review additionally requires both-direction mode-refusal tests with a compatible previous generation, unchanged session/pointers on refusal, and an explicit full-state normalization allowlist. Tokens, edge targets, role rows and frontier geometry must never be normalized away. Positive-hit and negative-miss counters are opportunities, not performance or closure proofs. Real alias/None-prefix and all-required-query cold gates remain explicit pending acceptance work.
- [M] The actual `7774604d` retry passed the production-library typecheck but failed test checking on two overly deep imports in the new raw-reader tests. Guard elapsed **41.193765 s**, exit 101/no stop, PG83245 drained; evidence `TMP/codex-epoch-s3.JjASCU/public-7774604d-typecheck/`. Root read the actual errors/receipt. The reader author is correcting just those import paths; no assertions are being weakened. This is still not an overall passing compiler gate or native execution.

### 2026-09-29 11:39 UTC — actual compiler finding; explicit lookup comparison underway

- [M] The allocated `c8022b06` metadata check actually ran under CPU16–19/j1/build1 and failed after **4.167698 s**, exit 101/no guard stop; owned PG4044108 drained. Its sole compiler error is macro recursion depth in the large public checkpoint-summary `json!` expression. Receipt: `TMP/codex-epoch-s3.JjASCU/public-c8022b06-typecheck/`. This is a compile failure, not a runtime or performance result. Root read the actual compiler output and guard receipt. Existing unrelated users' builds use other caches; their presence is recorded contention, not a reason to touch them or block this small controlled check.
- Runtime author `active_goal_delivery_audit` is making a separate narrow JSON-construction split, preserving fields and bounded scalar behavior rather than increasing the crate recursion limit. Independent reviewer `epoch_native_validation` will review it before a normal repeat metadata check. The failed candidate stays frozen; no native build is allocated yet.
- Root approved the next small public comparison seam from `TMP/codex-public-epoch-lookup-control-plan-2026-09-29.md`: native author handles the typed AllMiss/Snapshot control, binding and runtime; `joint_support_pruning` handles CLI/Python plumbing and frozen steering. Both use separate branches from the combined candidate. Default behavior stays AllMiss; Snapshot requires checkpoint-enabled Epoch and same-mode resume. No solver, CAS, scheduling or production change is authorized by this slice. Actual native equivalence, cold verification and matched timing remain required.
- [M] Guidance-only follow-up `b6e6dc03` explicitly says `--no-result` in help and diagnostics; root reviewed its five-path diff. Ten focused Python tests pass under pinned Nix 3.11 (0.019 s harness / 1.163366 s guard, exit 0/no stop), evidence `TMP/codex-cp6-outer-python.4eLfMZ/no-result-guidance/`. Rust assertions remain unexecuted. This changes no verifier defaults and is not merged into the frozen candidate.
- [M] Read-only LC2 observation at 11:39 UTC: **92,787,050** discovered, **32,811,778** pending, **36,409,241** local completions, no frontiers, **47,202,566,144 bytes** native-process RSS. This is the heartbeat's `resident_set_bytes()` field, not a sum over a process tree. The existing hour-window metric is unchanged: pending growth per completion approximately 0.224. The conservative 6/67 initial-root count is not all-116-required-query closure. No ETA or restart recommendation follows; production remains untouched while the consolidated build is prepared.

### 2026-09-29 11:32 UTC — corrected public slices consolidated; metadata gate allocated

- [M] Runtime is privately committed as `3e8e9e3c9f64675c0f25d01c5a4ac22977577653` (tree `52df12ff6d6772bdfbea44d3c539b3ee5c06d03d`), including both runtime audit corrections. Root and two separate agents reviewed the corrected source; independent receipt `TMP/codex-public-s3-runtime-independent-audit-2026-09-29.md`. The 11 new Rust tests remain unexecuted. Outer source is `2a78d7c054233d7e09e2110ae52143090f9e4a68` (tree `4d1ddb505f03d7389986efa523a3a83a7c568df0`), with all three audit findings corrected plus a separately caught return-signature typo. Nine reader tests remain unexecuted. Neither branch is pushed as a validated implementation.
- [M] Corrected fake steering plus actual tracked-role tests passed **10/10**, no skips, first on CPython 3.13.15 (0.013 s harness / 1.256701 s guard) and then on the allocated pinned Nix Python 3.11.15 (0.029 s / 1.214520 s), both exit 0/no stop. Root discovered and documented the first interpreter difference instead of silently attributing it to Nix. Receipts: `TMP/codex-cp6-outer-python.4eLfMZ/{corrected,corrected-nix311}/`. The three role tests preserve all 116 required queries, including 16 convenience queries, and 67 auxiliary helpers; they are not native prefix/closure acceptance.
- [M] Root performed normal conflict-free cherry-picks into new `codex/public-epoch-integrated`: **`c8022b06a1a071feb6fdc369e593db35fd1f0e12`**, tree **`105274f0334e762f3dc3020b67f8ddcff0e36adc`**, clean. All 27 runtime paths match the reviewed runtime commit; all 11 outer paths match the reviewed outer commit; no manual merge edits. Cargo/lock/Nix/Symbolica inputs are unchanged versus `29e30a79`. Main and production are untouched.
- The executor is allocated a normal, non-forced advance of its exclusively owned validation worktree/cache from preserved `29e30a79` to `c8022b06`, then one release metadata check with the existing app-opt1 correctness override, CPU16–19/build1/j1 and host-headroom guard. A terminal result is required before allocating native code generation. This is not a completed check or performance run yet.
- Follow-on public comparison design found an important CLI distinction: `walk-verify-closure` automatically binds a sibling result file unless **`--no-result`** is supplied. CP6 raw cold gates must use that explicit switch; merely omitting `--result` can intentionally produce summary-only INCOMPLETE. A separate narrow help/diagnostic correction is assigned without mutating the frozen candidate. Existing Ready exit-0/full-result gates stay unchanged; CP6 acceptance must independently prove raw closure rather than treating exit 4 or queue drain as success.

### 2026-09-29 11:23 UTC — public runtime freeze and cross-interface audit

- Previous goal turn was **progress**: completed actual core/app gates on `29e30a79`, preserved the successful binaries, independently found two outer integration defects, delegated corrections and pushed `df381aec` / `a2c336b3`. This turn re-read current worktree state and the actual fake-test receipt; no build/process wait is being claimed.
- Runtime author `active_goal_delivery_audit` froze 27 source paths on `fa783fc5`, with receipt `TMP/codex-public-s3-runtime-source-2026-09-29.md` (initial SHA256 `e03fd07a96c5aa559379a65ecd581f2cc1dcb345bd5403a0803b08c2f98f3a92`). Root read the receipt, public lifecycle/controller/streamed-write paths and real-native test draft. Independent reviewer `epoch_native_validation` confirmed the source hashes and is auditing it. Nine new runtime tests and updated public cold tests remain **uncompiled/unexecuted**; their source presence is not acceptance.
- The reviewer found another outer consistency gap: unresolved query rows and input-frontier records need exact one-to-one correspondence, not just id/count checks. The outer author is adding that and mutation tests along with the earlier two corrections. The runtime reviewer separately found omitted scalar descendant-closure fields required by the existing monitor; the runtime author will supply them without restoring a whole-state census. These narrow fixes preserve CP5/default behavior and the pending-growth metric.
- Next executable gate remains one **combined, fixed-identity source** metadata check after both corrected slices are reviewed, followed by native execution and real public cold controls. Private source commits may precede review to fix their identity but do not authorize a push or imply a passing gate. The next performance seam, explicit recorded Snapshot versus AllMiss selection through public steering, is being designed read-only; it is not silently enabled by this integration.

### 2026-09-29 11:18 UTC — independent outer review catches composed-path defects

- [M] Validated-native progress checkpoint `df381aec` is pushed. Main retains only the user's unrelated work; later public engine slices are still isolated and unvalidated.
- The outer draft was source-frozen at receipt `TMP/codex-public-epoch-outer-source-2026-09-29.md` (initial SHA256 `b81b794ae59e83ea2327ee309ec515520163da83ca218c9e7a60de55247b3aed`). Its seven fake-only Python tests passed in 0.014 s / 1.1805 s guarded wall, exit 0/no stop, under `TMP/codex-cp6-outer-python.4eLfMZ/focused/`. This is not native/public-campaign coverage.
- Independent auditor `epoch_native_validation` then found two composed-path blockers that those tests did not cover. First, final status publication could replay a cached saved event and overwrite the CP6 checkpoint's deliberately unconfirmed state after a missing terminal handoff. Second, the cold reader authenticated record files and then passed paths to a second, unauthenticated open used for parsing. Root inspected the actual caller paths and confirmed both findings. These are defects in the unmerged draft, not observations about LC2.
- Outer author `joint_support_pruning` is correcting only those seams and adding corresponding regressions. Preserve terminal invalidation through final status publication; authenticate the bytes actually parsed once, without copying whole record payloads or adding a new persistence framework. Preserve CP5 behavior. Initial seven-test PASS remains a baseline, not evidence for the corrections. Re-freeze and independently re-review before consolidated native typechecking.

### 2026-09-29 11:12 UTC — focused and full-core execution; public integration review

- [M] Frozen `29e30a79` app walking focus completed: **597 passed, zero failures, 12 ignored**, 135.68 s harness / 136.204129 s guard wall, exit 0/no stop and drained PG3538466. Across the core and app focuses, root independently checked all 13 named new tests: one endpoint-buffer reuse test, five admission-atomicity tests, one native interrupted-cut test, one native periodic-save test and five telemetry tests. Two W50 subcases explicitly omitted for the 16-CPU allocation are **not** covered. Exact identities, ignored reasons and omissions are in `TMP/codex-epoch-s3.JjASCU/consolidated-29e30a79-app-focus/verified-result.json` and the corresponding core-focus receipt.
- [M] Full core then passed **2,845 tests, zero failures, 32 ignored**, 128.71 s harness / 129.210637 s guard, exit 0/no stop, PG3663119 drained, no capacity omissions. Root read the actual receipt and exact N2 passing line; both executable hashes, source tree and clean Symbolica remain unchanged. Evidence: `consolidated-29e30a79-core-full/`. Full app then executed from the same already-built app-opt1 binary; no new source/Cargo/cache mutation. These remain correctness gates, not production timing or validation of later public source.
- [M] Full app subsequently passed **1,016 tests, zero failures, 12 ignored**, 195.03 s harness / 196.240464 s guard, exit 0/no stop, PG3696488 drained. Root independently read the actual stdout/guard receipt; executor verified unchanged hashes and clean source. Its own stderr reports 12 capacity diagnostics: ten genuinely omitted W50 subcases and two helper diagnostics preceding explicitly <=6-worker execution. Do not count any as W50 coverage. Evidence: `consolidated-29e30a79-app-full/`. Both binaries are now preserved as independent read-only copies outside the Cargo cache: `TMP/codex-epoch-s3.JjASCU/frozen-29e30a79.6zQX9e/manifest.json`. This does not advance the frozen source or validate newer slices.
- Runtime and outer authors are finishing separate public-CP6 branches. Root read the new cold-reader/frontend draft and runtime report path. The draft now rate-limits telemetry independently of phase changes, emits counters at the existing envelope depth, and uses scalar Tracker counters rather than the hidden storage census. Separate agents will review each other's frozen source, with root integration review. Read-only generation selection, strict input roles, real W1/W2 interruption and explicit summary-only INCOMPLETE checks remain required; no new native public test has executed yet.
- [M] Read-only LC2 snapshot at approximately 11:09 UTC: **89,572,405** discovered, **32,398,108** pending and **34,486,224** local completions, zero frontiers, approximately **45.6 GB** tree RSS. The stale conservative 6/67 root counter is not the requested 116-query closure measure. LC2 is untouched. No restart recommendation or completion forecast follows from this snapshot; the user wants consolidated validated improvements before switching.

### 2026-09-29 11:02 UTC — consolidated build and N2 focused execution pass

- [M] Original `29e30a79` core/app native build completed successfully at 10:56:53 UTC: 3,864.736006 s guarded wall, exit 0/no guard stop, Cargo `build-finished.success=true`, owned PG2445250 drained. Root independently read the receipt, actual Cargo artifact profiles, final stderr and clean source/tree. Exact source `29e30a798fbf1fbf4ded6fdce99977914212f07b`, tree `6557cd6a975a3c995f6ed57d6a0bf3dc8faf08df`; Symbolica remains clean `ef0db494`. The build peaked at 78,535,212 KiB single-child RSS. This is compilation, not solver timing.
- Actual test binaries: core opt3 SHA256 `4fe0b74f7ca3a73c8985940e48b40f66b87ea0521dc021f8bd7d855e959dc0a0` (121,197,256 bytes); app opt1 SHA256 `8225ad98e1f27ff51c2470f2cee71001e7d66d52faa54ebae076f5a964ed6b16` (1,247,354,080 bytes). Paths and full profiles: `TMP/codex-epoch-s3.JjASCU/consolidated-29e30a79-build/frozen-executables.json`. Correctness-only app opt1 is not the production timing profile. No result validates later `fa783fc5` or public-integration drafts.
- [M] Core applied focus passed **59 tests, zero failures/ignored**, 0.09 s harness / 1.160520 s guarded wall, exit 0/no stop, PG3525808 drained. Root found the exact N2 endpoint-storage reuse test's passing line. No capacity skip; the executor confirmed both executable hashes unchanged. Evidence: `consolidated-29e30a79-core-focus/` beside the build receipt.
- App walking focus then started from the same frozen pair, session48016/PG3538466, no Cargo invocation. Root confirmed its actual process live at 11:01; no final receipt yet. Full suites remain contingent on focused-result review. The current source work continues on isolated branches while this gate executes.
- Early root review of the public draft identified two integration/performance hazards before freeze: emitting JSON on every internal phase change would recreate per-commit telemetry overhead; nesting counters under another `progress` object would hide them from existing CLI/supervisor readers. Authors are correcting these while preserving low-rate status, existing envelope conventions and the unchanged pending-growth metric. These are source findings, not measured production defects or speedups.

### 2026-09-29 10:55 UTC — public implementation underway; independent test-plan critique

- Previous goal turn was **progress**: the reviewed miss/S5 source was combined at `fa783fc5`, progress milestone `e2e5fee4` was pushed, and public lifecycle/outer integration was assigned. [M] This turn revalidated the same original build process at 10:47 and 10:52; Cargo PG2445250 and app rustc2491161 were still live. No source/cache change or replacement build, and no `29e30a79` native result yet; earlier `f404cf66` receipts remain separate.
- The shared contract is frozen: `RUSTRED-WALK-CP6`, schema 1, epoch semantics 3, `latest.json`/`previous.json`/`checkpoint.lock`, `epoch-session.bin`, `epoch-poison`, and public epoch section filenames. Ambiguous `restore_validated` is removed; the probe retains legacy CP5 fields. Initial same-executable continuation is a **launcher frozen-binary policy**, not a new native binary-hash binding. Native identity remains request/owners/B/format/semantics. No compatibility or extra authentication framework is added.
- [M] Source work is underway in `codex/s3-public-lifecycle` and `codex/public-epoch-outer`, both based on `fa783fc5`. Runtime and frontend files have separate authors. Checkpoint-only output is accepted as a staged capability even on clean drain: queue exhaustion may be true, but complete-result/finalization/required-query closure claims remain unestablished until actually checked. Before deployment a truthful exact scoped-completion report is still required; this staging decision does not shrink the goal.
- The execution agent independently reviewed the acceptance plan. Added concrete expectations include caller-thread/no-pool W1 execution; bidirectional W1/W2 replay with unchanged full-state comparison; complete-cut preservation on interruption; explicit missing-prefix `UNADMITTED` handling; read-only cold generation selection; and a summary document not impersonating a full record inventory. Raw CP6 cold verification may PASS while ordinary summary-only `--result`/Python pairing remains INCOMPLETE. Actual immutable 116-required/67-auxiliary fixture preservation is distinct from synthetic prefix-mechanics tests. These are test requirements, not executed results.
- Root identified a concrete reporting-failure seam: the private controller's broad panic handler can classify an `on_saved` callback panic as an engine failure and poison an already durable generation. The public wrapper is to catch non-authoritative observer failures separately, request cooperative stop, and retain the saved authority with a bounded diagnostic. Genuine P1–P3 invariant errors must still poison. A dedicated after-latest recovery regression is required. Root also called out the indirect `certification -> Tracker::json` census on clean drain; the lightweight report contract covers that path too.
- No next source build is automatically queued at `fa783fc5`: complete the current `29e30a79` gates, then select a reviewed consolidated candidate to avoid rebuilding every private micro-slice. Compilation remains separate from performance evidence. LC2 and its resources remain untouched.

### 2026-09-29 10:45 UTC — public lifecycle implementation assigned

- Root read the complete native design at `TMP/codex-public-s3-lifecycle-design-2026-09-29.md` (initial reviewed revision SHA256 `c60a6971a19d19941ff55084d749dc1b913e85bc1e6988bc08576637618b5d3c`). The file is a living design; later coordinated outer-contract revisions have separate hashes. Implementation is assigned on new branches from `fa783fc5`, with runtime/controller/publication/report owned by `active_goal_delivery_audit` and CLI/Python/read-only cold-adapter integration by `joint_support_pruning`. Existing validation trees remain frozen.
- Agreed direction: public `RUSTRED-WALK-CP6` schema 1, fresh-only format promotion, explicit epoch semantics, no old-private or CP5 import, and no ambiguous flag claiming a newly saved generation was cold-verified. Preserve CP5/default behavior. Checkpoint-free Memory runs remain explicit; do not silently create temporary durable state. W1 must execute CAS inline with cooperative stopping, while W>=2 retains save-before-worker-join responsiveness.
- Saved-stop reporting uses bounded scalar state and the actual durable receipt, not forced closure refresh, full resolution/annotation vectors or hidden tracker/storage scans. Missing full-output/required-query closure information stays unknown, not zero or success. The independent cold verifier must remain read-only and validate the referenced generation even when its output document is intentionally summary-only. G2/rescue composition and rolling scheduling are still required follow-on gates; this source-design approval is not public activation or a restart recommendation.
- [M] Audited progress/candidate documentation committed and pushed as `e2e5fee4`. No engine code or unexecuted private tests were merged into main by that documentation checkpoint.

### 2026-09-29 10:41 UTC — reviewed miss and edge-feed slices consolidated

- [M] Root and separate agents independently reviewed the five-file S4 miss-bypass and two-file S5 edge-feed changes. Miss source committed privately as `f128f550e23aaa7483213d72d82a87e5fc17ac89`; S5 as `9dc770a4254b739d5eacb6040bd75990b7390e0d`. Normal conflict-free combination in new worktree `codex/s3-s4-s5-combined` is `fa783fc5b7df1ea78a1e7cc522dd8698849b8842`, tree `622e16de240f31f8c565b26dc0ed18851d00d8d4`. Root independently confirmed the seven final files match their reviewed parents and whitespace checks pass. Receipt: `TMP/codex-s4-source.yAIv3U/COMBINED-S5.md`. **Four new miss tests and five new edge-feed tests are source only; no new combined execution or speed claim.**
- The miss optimization avoids repeating the published-store search only for validated same-view lockstep negative reports, while retaining exact-image collision/uniqueness checks and every new dependency obligation. A negative report never proves coverage. S5 batches exactly the same little-endian words into existing BLAKE3 using bounded stack scratch; hash bytes, schema and failure boundaries are unchanged. This is a narrow first slice, not the complete merge architecture.
- Public deployment integration is now split into native lifecycle design (`active_goal_delivery_audit`) and outer CLI/Python/cold-verifier contract review (`joint_support_pruning`). Existing public epoch exports are nonresumable, and the durable private format is not yet the format the launcher or verifier accepts. A manifest rename alone is insufficient. Preserve current CP5/default behavior, avoid importing old campaigns, and connect real prefix continuation, periodic/interrupt saves, W1 handling and lightweight RAM-stop reporting before enabling the public path.
- [M] At 10:38:55 the original `29e30a79` build remained active under the same session/PG, with approximately 72.6 GiB app-compiler RSS and 694 GiB host headroom. No competing build, source mutation or cache advancement. Conditional focused native tests still await actual build completion; newer source cannot inherit an older test PASS.
- [M] Read-only LC2 snapshot around 10:38 UTC: 86,706,513 discovered, 32,042,946 pending, 32,990,733 local completions, zero frontiers and approximately 44.3 GB tree RSS. Its conservative stale 6/67 initial-root count is not a 116-required-query closure measurement. The campaign remains untouched; no ETA or consolidated restart recommendation follows from these counters.

### 2026-09-29 10:30 UTC — continued consolidation and public-lifecycle review

- Previous goal turn was **progress**: independently reviewed prefix and S4 source were committed and combined privately, the NUMA opportunity check supplied negative evidence, and documentation milestones `f76c31b7` / `9e9f2046` were pushed. No unexecuted native change is counted as accepted deployment code.
- [M] Root revalidated the same live build at 10:28 and 10:30: Cargo/PG2445250 and app rustc2491161 remain active on frozen `29e30a79`; no replacement build or cache/source mutation. Core test code is emitted, app compilation and all new runtime gates are still pending.
- Two isolated implementation assignments now proceed from reviewed combined `e9645819`: `joint_support_pruning` removes the redundant same-view miss search under the registered guards; `active_goal_delivery_audit` implements only portable bounded edge-hash input batching, following the independently root-reviewed `TMP/codex-s5-edge-feed-proposal-2026-09-29.md` (SHA256 `e5eb7c94d9ba1a25ae44ddc2a0f021c0a3ac4548ad6d5f6d09ed1ac2b63621b2`). Existing BLAKE3, bytes/order/failure boundaries and schemas stay unchanged. Neither slice is built or measured. This does not complete S5's broader typed-record/merge architecture.
- Root reviewed the entire public `epoch::run`/`finish` path against the private lifecycle. Public resume is still explicitly refused; the public path still uses the old nonresumable exporter and synchronous controller. Wiring must also avoid unconditional forced closure refresh/full resolution allocation after a RAM-triggered save, add truthful controller heartbeat/phase/checkpoint reporting, and address the private W>=2 versus public W1 boundary. These are required integration work, not deployment-ready behavior. G2/rescue composition and rolling scheduling remain open; source unit tests alone cannot satisfy those requirements.

### 2026-09-29 10:26 UTC — prefix and S4 combined; narrow miss follow-up assigned

- [M] S4 was privately committed as `0c2c97de1a0aabd9cec05d2e262127463b5e4f7a`, then normally cherry-picked onto prefix `bce2e56e` in the new `codex/s3-s4-combined` worktree. Combined tip `e964581974221732e1708d076c463a180f234aa1`, tree `2174b880c8e407a63ae7c6a806a7512f03051b28`, is clean. No conflicts or manual integration edits. Receipt: `TMP/codex-s4-source.yAIv3U/COMBINED.md`. Root and the independent reviewer checked the seven overlapping files against both parents; all readiness, diagnostics, ownership and test changes remain. **No combined typecheck or runtime test has executed.**
- Assigned `joint_support_pruning` the registered negative-miss follow-up on a separate branch from that frozen combined tip. Keep the AllMiss control/default, exact-image uniqueness, positive verification, cut-local resolution and dependency graph unchanged. The separate reviewer accepted the design; implementation and measurement are pending. No new public API, stale-snapshot shortcut or rolling-mode claim.
- The current validation build remains the original `29e30a79` job, not the new combined source. Its source/cache are unchanged while isolated follow-up work proceeds. Main and production do not inherit private-source execution claims.

### 2026-09-29 10:23 UTC — S4 source audit accepted; miss and NUMA dispositions

- [M] Independent S4 review found and corrected one additional test fixture that mutated through the new read-only store wrapper. Assertions are unchanged. Final 24-file receipt: `TMP/codex-s4-source.yAIv3U/SOURCE-final.md`, SHA256 `0533bf26855861c71a23c94a153ca7abbd7dc6b8a4c186ea5c9d65d082515caf`; all hashes independently matched. Source review passes, but all eight new and two expanded tests remain uncompiled/unexecuted. Author is authorized to privately commit and integrate with prefix `bce2e56e` in a new isolated branch, not main or the active validation checkout.
- The author, root and separate reviewer agree on the narrow lockstep negative-miss proposal, preserving exact uniqueness and every positive/antichain/dependency check. It is registered with explicit falsifiers in `docs/research/codex_next_candidates_2026-09-29.md`; no bypass implementation or timing claim yet. The reviewer is now preparing the smallest S5 typed-record/bulk-edge proposal, not implementing another framework.
- [M] A one-shot NUMA census and root's independent small repeat find only 1,408 KiB resident across the socket boundary from LC2's allowed CPUs. This does not measure remote accesses or prove intra-socket locality. No placement change or benchmark is authorized from that observation; detailed limits and remaining memory/checkpoint questions are preserved in the candidate document. Production is untouched.
- [M] Progress documentation milestone `f76c31b7` is pushed on `fable_5_1`. The main FeynKit file still has its original user-owned SHA256 `33819cb3e5053a59f3587741e29b297a3a5b53bfa35dabcbe2e8fe95fe700332`; unrelated untracked work remains untouched.

### 2026-09-29 10:16 UTC — S4 frozen for independent review

- [M] `joint_support_pruning` froze the first lockstep inspector-lookup slice: 23 files, eight new test functions and two existing real-native interruption/periodic tests expanded to both modes. Exact source hashes and limits: `TMP/codex-s4-source.yAIv3U/SOURCE.md` (SHA256 `5603503f5db35e7fb379de7d22fc20f5c1d9f016f0ea668c1091d7eba29a1186`). Targeted formatting and whitespace checks passed; **no typecheck, native test or performance result yet**. The separate prefix author is its independent reviewer.
- Root read the snapshot, resolver, inspector, P1/P2/P3, controller and new regression source. Stored proposals still require independently reconstructed valid images and exact containment; canonical normal selection comes from unchanged lookup on the frozen view, not an independent minimum-ID replay at merge. The same-binary AllMiss control remains available and the public default is unchanged.
- Remaining measured-miss-path question: snapshot misses currently undergo a second published-store lookup in P2. Root requested a read-only proposal for avoiding that duplicate search without weakening exact uniqueness, cut-local antichains or positive containment authority. Do not change the frozen slice or count the possible saving before measurement. S5 typed records/bulk edges, S6 rolling scheduling and public G2/rescue composition remain pending.
- [M] At 10:16 the original combined native build is still compiling app code (Cargo PG2445250, same source/session; approximately 24 minutes elapsed). Core test code has been emitted, but no new runtime test has yet executed. This compilation time is not solver performance.

### 2026-09-29 10:12 UTC — prefix source checkpoint and S4 review

- [M] `active_goal_delivery_audit` privately committed the 16-file prefix-continuation slice as `bce2e56eed2dc63243259008da2dcdc9a690ae28`, tree `df311a346eacb4cccc1496510eadb8ea7e2e620b`. Root independently reviewed the implementation and the final checked-overlap and reducer-backed Route tests. It resumes from processed query-row count, preserves role/frontier diagnostics, refuses work before complete admission, and retains the atomic precommit boundary. Source receipt: `TMP/codex-s3-prefix-source-receipt-2026-09-29.md`. **All 12 new tests remain uncompiled/unexecuted**; this is not public resume activation or main integration.
- The private S4 draft now shares a single immutable store per lockstep cut, takes one lookup lease per job, and releases every lease before mutable merge. Root's early review checked inspector/P2 image validation, stop/save-before-join handling, and typed engine errors for unexpectedly shared mutation. `active_goal_delivery_audit` will independently audit the author's frozen slice. The default remains all-miss; no rolling scheduling, G2/rescue composition or performance gain is claimed.
- [M] The original combined `29e30a79` build remains live, same process group 2445250. Cargo has emitted the core test target; app test code is still compiling. No tests from this build have run yet. After a successful build/drain and executable provenance check, the executor may run the approved core and app focused filters; full suites require separate result review.
- [M] Read-only LC2 snapshot around 10:11 UTC: 84,108,744 discovered domains, 31,663,931 pending, 31,800,433 local completions, zero frontiers, approximately 43.1 GB process-tree RSS. The conservative stale initial-root count remains 6/67, **not** the 116-required-query closure count. Production is untouched; these observations establish no termination estimate. No restart recommendation before the consolidated build and matched gates.

### 2026-09-29 09:54 UTC — next continuation, validation and implementation remain live

- Previous goal turn was **progress**: the older-baseline full native suite and the combined-source metadata gate completed, the audited admission fix was privately committed, and progress milestone `69000a1c` was pushed. New prefix/S4 work is not counted as delivered merely because it is assigned.
- [M] Root revalidated the original combined build as live: PG 2445250 with Cargo and rustc PIDs 2445517/2445519 at 09:54; same frozen source and session 44669. No replacement build was started. Compilation remains separate from solver timings.
- Current implementation branches are `codex/s3-prefix-continuation` and `codex/s4-inspector-lookup`, both isolated from the validation checkout and production. S4 is deliberately a first lockstep inspector-lookup stage: snapshot sharing does not yet permit rolling concurrent merge, and no whole-store clone or unchecked positive is allowed. S5 typed-record/merge and S6 rolling work, plus epoch/G2/rescue composition, remain explicit follow-on requirements.

### 2026-09-29 09:52 UTC — consolidated source passes its metadata gate

- [M] Root independently checked the combined-source metadata command, stderr completion, exit 0 / no stop reason and drained process group. `29e30a79` passes `rustred` and `rustred-app` release test typechecking with the recorded app-only opt1 override: 138.278364 s guarded wall, approximately 2.3 GiB maximum child RSS. Evidence: `TMP/codex-epoch-s3.JjASCU/consolidated-29e30a79-typecheck/`. Existing warnings remain visible; no source correction or broad dependency rebuild was needed. This compiles metadata, not native test code, and executes no tests.
- Root allocated one core+app test-code build from the same frozen source/cache: `cargo test --release --no-run --locked --offline -j8 --config 'profile.release.package.rustred-app.opt-level=1' -p rustred -p rustred-app --lib --message-format=json`, CPUs16–31/heavy+build1 with the existing headroom guard. Record actual Cargo executable paths/hashes before running any tests. Keep later prefix/S4 edits in separate worktrees. No performance conclusion or production-switch recommendation follows from typechecking.
- [M] That exact build started at 09:52:29 UTC, session 44669 / owned process group 2445250; receipt `TMP/codex-epoch-s3.JjASCU/consolidated-29e30a79-build/`. Root read the recorded command/resource allocation. It is compiling, not executing tests; preserve its frozen source/cache and wait for an actual terminal receipt before any next gate.

### 2026-09-29 09:50 UTC — combined metadata gate and two isolated implementation slices

- [M] Root accepted the complete frozen-baseline receipts and the independent frozen executable copy at `TMP/codex-epoch-s3.JjASCU/frozen-f404.1xd7mz/`. The original branch remains at `f404cf66`. The isolated validation checkout advanced normally, without reset/cache copying, to clean detached `29e30a79`, exact tree `6557cd6a975a3c995f6ed57d6a0bf3dc8faf08df`, with clean Symbolica `ef0db494`. Its guarded `cargo check --release --tests --locked --offline -j1 --config 'profile.release.package.rustred-app.opt-level=1' -p rustred -p rustred-app` is active on CPU16–19/build1. Evidence: `TMP/codex-epoch-s3.JjASCU/consolidated-29e30a79-typecheck/`. This is not a passing gate or production timing build yet.
- Root independently reviewed the implementation-ready prefix plan, `TMP/codex-s3-prefix-continuation-plan-2026-09-29.md` (SHA256 `0a91b256335e998055de9cd74ff623f5d533f4010535ce7bf00d3f63921a9d49`), and assigned `active_goal_delivery_audit` a separate branch from `29e30a79`. Scope: resume by processed row count, checked dispatch readiness, preowned row/frontier commits, preserved admission diagnostics, and a private scalar-version bump without migration. Validate saved issuance counters **before** restore discards them. No public activation or edits to the frozen validation source.
- Root reviewed `joint_support_pruning`'s concrete S4 proposal against current resolver/merge/pool source and assigned a separate implementation branch. First move existing exact/orthant/minimum-live lookup to inspectors through a shared, immutable, lockstep Store snapshot with checked unique mutation after complete release. No whole-store copy, `Arc::make_mut`, CAS rewrite or rolling scheduler. Returned positives still undergo independent canonical-image/native-summary validation and exact containment at merge; the suggested lazy-summary bypass is deferred to preserve F1. Keep additional verification costs visible. This slice is meant to establish a measured lookup reduction, not to claim the completed S4–S6 or G2/rescue composition.
- Authors own disjoint branches; root reviews and integrates, with later cross-audits by the separate author. Source/testing work cannot mutate LC2. Main progress milestone `69000a1c` is committed and pushed; unrelated files remain untouched.

### 2026-09-29 09:45 UTC — full older-baseline app suite completes

- [M] Root read the full app stdout and guard receipt: **999 passed, zero failed, 12 ignored**, 161.14 s native test time / 162.217913 s guard wall, exit 0 / no stop reason. The owned process group 2119842 is drained. W50 tests explicitly report capacity skips under the 16-CPU allocation and remain uncovered by this execution. Evidence: `TMP/codex-epoch-s3.JjASCU/native-controller-full/`. These are tests of `f404cf66`, not the newer combined source, CLI, cold campaign verification or performance gates.
- Preserve the successful baseline executable before the next normal Cargo source/cache advancement. Proposed correctness-only iteration uses an explicitly recorded app `opt-level=1` override with release dependencies; it cannot furnish final production-profile timing evidence. No such build or source advancement has started at this entry. Root checked that the user's main FeynKit file retains SHA256 `33819cb3e5053a59f3587741e29b297a3a5b53bfa35dabcbe2e8fe95fe700332`.

### 2026-09-29 09:43 UTC — first controller execution passes; consolidated prerequisites advance

- [M] The original frozen `f404cf66624cc6bad6c516c9169f40dd4a3166a3` build finished successfully at 09:36:03 UTC, guard 4,451.987 s / exit 0 / no stop reason. Compilation is separate from native execution. The actual generated app-test executable is 921,003,736 bytes, SHA256 `5652b32c6237a4e6533b2985510d947d720d38b02ee40732626e671d6d8e7c99`; source, Symbolica and hash remained unchanged. Focused epoch execution passed **130 tests, zero failed/ignored**, 0.81 s test time / 2.213 s guard time. Both groups drained. Root independently read raw and verified receipts in `TMP/codex-epoch-s3.JjASCU/native-controller-{build,focus}/`, then authorized the same-binary full app suite under the original CPU16–31/resource/deadline guard. That suite is active; observed W50 capacity skips are not coverage.
- [M] `active_goal_delivery_audit` privately committed the independently reviewed admission-atomicity prerequisite as `29e30a798fbf1fbf4ded6fdce99977914212f07b` (tree `6557cd6a975a3c995f6ed57d6a0bf3dc8faf08df`). Root and `joint_support_pruning` reviewed the seven-file slice and five new tests. Existing native index preparation and retirement verification now precede authoritative ID/live changes; impossible commit inconsistencies stay poisoned. The five regressions are **uncompiled/unexecuted**. This does not implement prefix continuation, public resume, arbitrary allocator-OOM recovery, or a performance improvement. No untested engine slice is merged into main.
- Next: finish/drain the full frozen-baseline gate, preserve its executable outside the mutable cache, then normally advance the same isolated validation worktree to the audited combined source. The newer real-native interruption, periodic-save, atomicity, N2 and telemetry tests need their own build/execution. Source provenance is not test inheritance. The admission author is preparing a narrow continuation slice; the independent reviewer separately studies the concrete S4 inspector-lookup seam.
- [M] Read-only LC2 observation around 09:42 UTC: 80,916,156 discovered, 30,492,392 pending, 30,501,911 local completions; zero frontiers, approximately 41.5 GB RSS, conservative stale 6/67 initial roots. One-hour coordinator commit/preparation fractions are 52.89%/30.94%, mean computing inspectors 3.46. These are neither the 116-required-query closure count nor a completion estimate. Production and pending-growth rendering remain unchanged. No restart recommendation before consolidation.

### 2026-09-29 09:30 UTC — continuation and next source prerequisite

- Previous goal turn was **progress**: periodic save behavior and six tests were implemented, independently source-reviewed and privately checkpointed; the corresponding accurate progress documentation was pushed as `89cbbd25`. None of the unexecuted native tests is counted as passing.
- [M] Root revalidated the original build as live at 09:27 using Cargo PID 1133746 / rustc PID 1190542, owned PG 1133746 and the existing session 11967. It remains the same frozen `f404cf66` job, not a restart after an observation timeout. No build errors or terminal guard receipt were observed.
- Admission-atomicity implementation is isolated in `codex/s3-admission-atomicity`, based on `5bb498a3`. Reuse native insertion preparation and planned-container verification; explicitly refuse already-poisoned state so a later success cannot clear earlier authority failure. Independent review is assigned separately. No prefix continuation, public activation or new persistence schema is introduced by this prerequisite.
- Root revisited S4's immutable-snapshot/inspector-resolution design and current `Resolver`: the current S2 sink still ships nearly all successors as merge-side misses. The real throughput architecture remains ahead; checkpoint correctness work and passing steering tests are not substitutes for the inspector-side lookup, merge and rolling-scheduler gates. The latest plan's comparator is the current G2 baseline, not obsolete historical runC.

### 2026-09-29 09:24 UTC — periodic source independently audited and privately checkpointed

- [M] Root and `joint_support_pruning` independently reviewed all seven frozen periodic-save files, including all six new regressions; no blocking source finding. Root committed the private consolidated tree as `5bb498a3`. The production change uses the existing save path only after a successful nonfinal merge, with stop precedence and a monotonic interval reset after successful publication. Tests cover stop/save/refill ordering, forced error/frontier/final-drain precedence, existing publisher failure/warning seams, zero interval, and a real nonfinal checkpoint continued normally or reopened with another worker width. The pre-existing interrupted-native assertions are unchanged; fixture/comparator helpers were shared.
- Formatting and diff checks pass; **all six new tests and the earlier new S3 equivalence test remain uncompiled/unexecuted**. This is not a main engine merge or public resume, prefix continuation, G2/rescue composition, crash-during-CAS or performance claim. The older frozen `f404cf66` release app test target is still compiling; at 09:19 it had run 57m19s with no logged errors/guard stop, about 43.3 GiB compiler RSS and 785.7 GiB host headroom. Compilation is not solver timing or test success.
- The next source-only slice is the admission atomicity prerequisite in a **separate** worktree from `5bb498a3`: preprepare the existing index insertion and verify retirements before publishing an ID or changing live bits, retaining a prepared new bucket when needed. No store cloning, rollback framework, new CAS, schema or public wiring. Author: `active_goal_delivery_audit`; independent reviewers: root and `joint_support_pruning`. Consolidated source and live validation source/cache stay frozen.
- For subsequent correctness validation, first finish and drain the baseline gates and freeze their executable. Then advance that same private validation worktree/cache normally to audited consolidated source; avoid copying live caches or falsifying Cargo fingerprints. An explicitly recorded app-only optimized correctness profile may reduce iteration cost, but cannot supply production timing evidence. No such profile/source advance has run yet.
- [M] Documentation milestone `a1be7756` is pushed and main is synchronized. Existing user dirt is unchanged. LC2 remains untouched; no restart recommendation before consolidated gates.

### 2026-09-29 09:15 UTC — periodic-save implementation delegated separately

- Root and `joint_support_pruning` independently reviewed the narrow periodic-save design from `TMP/codex-s3-admission-periodic-design-2026-09-29.md`. `active_goal_delivery_audit` now implements only private post-P3/pre-refill periodic saves and regression tests in the consolidated worktree. Reuse the existing save/publisher path; preserve forced-stop precedence, session, counters, records and no-in-flight boundary. No public activation, new checkpoint schema or input-admission changes belong to this slice. Require a real nonfinal native checkpoint and exact continue/reopen comparison; source review is not execution evidence.
- Prefix-admission continuation remains the next separate slice: use processed **query rows**, not the number of unique admitted IDs, and resolve the discovered post-mutation index-allocation/diagnostic gaps before claiming a resumable prefix. Do not build a rollback or compatibility framework.
- [M] Root initialized only the new worktree's pinned Symbolica submodule at clean `ef0db494533c87adb40356c996241680dc5a7bff`, using the existing local checkout as object reference. Its graphica/numerica manifests are present. Main's vendor tree, user changes and the frozen live `f404cf66` build/cache remain untouched. No new native build was started.

### 2026-09-29 09:10 UTC — consolidated steering passes; remaining shortcuts dispositioned

- [M] Private consolidated source is clean at `711870414e8488613b5ff5f3926de03b5c3f144f` (tree `91106fb75a1f43688dfdd105c4ed5ac2500009ac`) in `codex/consolidated-ready`. It combines epoch/S3 test `170e9312`, N2 `5b08cf4f`, lean telemetry `187854b4`, and official main `80fed4a0`. The initial publisher base lacked later Python steering/tooling; the conflict-free main merge corrected that gap without changing native/Cargo/vendor bytes. Slice identities and proposed gates are in `TMP/codex-consolidated-source.NMXYnJ/SOURCE_AND_GATES.md`. No new native test, performance, public-resume or combined epoch/G2/rescue claim follows.
- [M] Root ran the combined Python discovery: 285 tests in 24.052 s, 284 passed and one explicitly gated slow test skipped. Then `RUSTRED_SLOW_TESTS=1 .../python -B examples/python/test_plan_renormalization_entry_queries.py FiveLoopTests.test_full_enumeration_equals_fixture -v` passed the full frozen five-loop enumeration in 84.776 s. Thus **285 unique tests** are covered, not 286. Exact commands/guards are in `TMP/codex-consolidated-checks.o3BJcv/{python,python-enumeration}/`; guards exited 0 with no stop reason, and both owned process groups drained. `joint_support_pruning` independently checked source, receipts, counts and drainage. These are steering/input tests, not native closure or timing comparisons.
- [M] N1/N4 reassessment independently passed at `TMP/codex-n1-n4-post-g2-reassessment-2026-09-29.md`, SHA256 `10e2b79c11ab11307f591a440c155712bab86a3a3ef3627d3498f0979af13f95`. The fixed-index native modular witness can prove a nonzero formal polynomial, not nonvanishing at every numerical parameter value. The finite post-G2 profile has zero initial-orthant reuse hits; 3,493,943 mixed job-local hits do not identify avoidable coefficient work or prove closed coverage. Defer implementation with explicit capped opportunity probes as reopening conditions; no custom CAS or broad tracing framework. See the persistent summary in `docs/research/codex_next_candidates_2026-09-29.md`.
- [M] Read-only LC2 observation near 09:09 UTC: 78,024,351 discovered, 29,738,167 pending, 29,046,808 local completions; zero frontiers, about 39.9 GB RSS and stale conservative 6/67 roots. One-hour coordinator commit/preparation shares are 55.05%/30.81%, mean computing inspectors 1.77. These support continuing coordinator work, not a completion ETA or the 116-required-query closure count. Production, its checkpoint and pending-growth metric remain unchanged.
- Frozen baseline `f404cf66` native compilation continues separately. Next S3 design is being audited for prefix-admission continuation and periodic committed-boundary saves; current prefix restore still refuses safely. All native lifecycle/feature-composition and performance gates remain open. No restart recommendation yet. Main documentation/profile milestone `80fed4a0` is pushed; unrelated user work is preserved.

### 2026-09-29 08:52 UTC — isolated consolidation and remaining algebraic questions

- [M] N2 source review passed independently at the frozen three-file hashes; root checkpointed it privately as `5b08cf4f`. The successful image constructor fills both endpoint vectors with exactly N entries, and exact projection returns same-arity arrays, so the private copies preserve their values and storage. Native execution and runtime benefit remain unproved; no production/main engine merge occurred.
- `joint_support_pruning` is preparing a separate `codex/consolidated-ready` source worktree from private S3 test tip `170e9312`, adding N2 and the previously audited lean-telemetry slice `187854b4`. Preserve slice provenance and report semantic conflicts rather than resolving them by assumption. No build or execution is authorized for this new source yet; frozen baseline `f404cf66` remains unchanged. The next validation matrix must include core geometry/power-bound tests, app telemetry/epoch/full tests, and subsequent matched controls.
- `active_goal_delivery_audit` rotates to read-only N1/N4 opportunity research using the qualified post-G2 profile, previous negative results, and pinned/current public Symbolica APIs. Seek a small exact-obligation-preserving test or a justified deferral, not a new CAS or proof framework. All required guards, poles, frontiers and nonzero-versus-not-identically-zero distinctions remain binding.

### 2026-09-29 08:50 UTC — profile report and private test checkpoint reviewed

- [M] The bounded profile report is ready at `docs/research/codex_post_g2_profile_2026-09-29.md` (SHA256 `a77cf4aaddc02653d50cbeec4e1180efd53d5c78626a7ececfff72fdd8598177`), independently reviewed by root and `active_goal_delivery_audit`. It retains exact receipt paths and reproduction commands, nominal-period/throttling caveats, no cold-verification claim, and the narrow buffer-reuse falsifier. No further profiling expansion is needed for this slice.
- [M] Root checkpointed the independently source-audited real-native interruption test as private publisher commit `170e9312`. It compares a fully admitted pre-first-merge stop and W2-to-W3 restart, not an interruption inside CAS, periodic checkpoints or public resume. Formatting passes; compilation/native execution remain pending. The author and separate reviewers are recorded above; this private commit is not merged into the tested main engine.
- N2's three-file source slice is now frozen in `codex/n2-image-buffer`: two in-place endpoint copies and one four-case storage/full-field regression. Root source review found no change to projection, rank, predicates, callback order or logical budgets; independent review is in progress. No build, test or speedup is claimed. The existing frozen epoch build remains active, separately timed, and unchanged; none of these newer source slices can inherit its forthcoming results.

### 2026-09-29 08:44 UTC — bounded profile complete; one narrow N2 slice starts

- [M] Profiling analysis is complete in `TMP/profile-analysis.lNU1ZU/{ANALYSIS.md,COMPACT_SUMMARY.json}`. All 49,119 sample rows and 631 symbol rows reconcile, with no unresolved sampled IP/DSO; call-stack quality remains unassessed. The 273 throttle streams are not 273 threads. The independently reviewed fixed-header scan finds 49,115 paired disabled elapsed intervals, four censored trailing starts and no pairing anomalies. These intervals and nominal sample periods cannot be converted into lost CPU, wall time or a sampling-bias correction.
- [M] Coarsely aligned interior windows, independently checked against compact sample timestamps, distinguish startup from traversal. The worker-active 60-second interior contains 32,326 samples: roughly 72.2% on inspectors, 15.8% on admission helpers and 12% on the main/remaining thread activity. Projection accounts for 8.41% of sampled instruction locations and named allocator routines 16.53%; these are qualified self-cost hints, not caller attribution or removable percentages. The finite control's preparation profile does not override mature production's coordinator telemetry.
- Registered and delegated a deliberately small N2 hypothesis: reuse the two already-sized shifted-image endpoint vectors instead of allocating replacement vectors after the existing exact power projection. Keep the projection and all bound, rank, failure, callback and budget behavior unchanged. Expected benefit is two fewer allocation/free pairs per applicable image, not a promised runtime improvement. Require field/storage-stability tests, existing geometry regressions and matched unprofiled controls; reject or park if benefit does not reproduce. No new CAS or generalized cache is authorized.
- `active_goal_delivery_audit` owns that isolated source implementation in a new worktree from `89d32a7d`; root and `joint_support_pruning` are independent reviewers. The same author's preceding S3 test slice is frozen in the separate publisher for review: formatted new-file SHA256 `e7404d6704de80a6b2c9e84392e98758a678eb247583880ecd5943cf3663766e`. It remains uncompiled/unexecuted and establishes no public resume claim. The original frozen `f404cf66` validation build is unchanged and still active.

### 2026-09-29 08:34 UTC — next native equivalence test delegated separately

- `active_goal_delivery_audit` completed the independent S3 gap review and now rotates to implementation of one source-only real-native stop/resplit equivalence test in the separate `codex-epoch-s3-publish` worktree. It must compare nontrivial actual dependency edges and complete non-timing record bodies, not merely accept `Ok` or compare the edge out-degree digest. Reuse the existing one-axis fixture and inspector; no new CAS or public lifecycle feature is authorized in this slice. Root and `joint_support_pruning` will review it independently; its author cannot approve it.
- Frozen validation `f404cf66` continues compiling unchanged in `codex-epoch-s3`. Once its build passes and drains, the executor may run the already reviewed focused test command against Cargo's newly produced, hashed executable, without rebuilding. Full-suite execution remains contingent on the focused result and independent review. Source work in the separate publisher cannot inherit this older executable's test result.
- Profiling analysis may make one bounded TMP-only read of throttle timestamp records to qualify the directional evidence. It must not become a new profiler framework or delay useful implementation. No host settings, production attachments or new benchmark runs are authorized by that analysis.

### 2026-09-29 08:30 UTC — profiling quality caveat; production observation

- [M] The first profile report exposed a tooling issue: this perf version rejects the `tid` sort key despite returning exit zero. Its output is retained as invalid; the corrected advertised `tgid,pid` report is usable. It reports about 49K total user-CPU samples and zero reported lost samples; displayed rows account for roughly 77.8% of the period above its threshold. This does not establish complete unknown-symbol or unwind quality. Full statistics subsequently report 49,119 samples, 49,119 throttle and 49,115 unthrottle records; their effect on sampling representativeness is under independent investigation. Do not interpret zero losses as unthrottled sampling or use these preliminary percentages as wall-time savings.
- Root approved bounded thread/DSO and full-symbol aggregate reads of the existing 403 MiB recording, not another native run. Evidence: `TMP/profile-analysis.lNU1ZU/`. Preparation and traversal remain separate questions; production startup is amortized, unlike the short control's whole-launch profile.
- [M] One read-only LC2 status observation near 08:29 UTC reports 73,780,957 discovered domains, 28,072,877 pending, 27,387,361 local completions, and the conservative 6/67 root counter. One-hour coordinator shares are approximately 49.63% commit and 35.58% preparation, with 2.30 mean computing inspectors. RSS is about 38 GB; checkpoint generation 4 is unchanged. These are not 116-query closure counts, a termination bound or a completion forecast. The pending-growth metric is unchanged and production was not modified.
- The private epoch test-target build remains active without observed compilation errors. The independent reviewer confirmed the next correctness gap: the existing real-native smoke only accepts any successful outcome, and partial-admission restore currently refuses continuation safely. Real uninterrupted/interrupted native equivalence, including full record content as well as graph edges, is still required before public activation.

### 2026-09-29 08:22 UTC — profiling run drains; native epoch build allocated

- [M] Owned diagnostic session `71170` exited zero; process group 1086503 has no remaining members. Profile runner returned zero/no exception, uncensored/no stop reason; command 177.153 s, prepared 82.776713 s, traversal 77.880618 s, 965,664 scheduled/759,916 native records. These instrumented timings are **not** another comparison arm or independently cold-verified closure claim. `perf.data` is 422,853,736 B; recorder stderr reports 402.599 MiB and 1,600 wakeups, with no error printed. Usable sample coverage, losses and unwinding remain to be checked.
- Root assigned `joint_support_pruning` the bounded post-run quality/self-cost analysis on CPU32–39, using the exact local perf executable, local build-ID location and disabled debuginfod; no production attachment, giant stack export or network/cache changes. Independent audit remains separate. Existing preparation/traversal boundaries must not be conflated with on-CPU percentages or critical-path wall time.
- Root allocated `epoch_native_validation` Stage2 only: frozen clean `f404cf66`, CPU16–31/j8, heavy+build1 locks, existing guarded private target, `cargo test --release --no-run --locked --offline -j8 -p rustred-app --lib`. No test execution yet; freeze Cargo's newly generated executable before focused/full gates. Compilation is separate from pilot/test timing; no source mutation during the build. The light disjoint profile analysis is not a benchmark. All prior comparison and metadata groups drained before this allocation.
- [M] Coherent goal amendment, complete benchmark report and progress milestone committed/pushed as `89d32a7d`. User changes remain preserved. Restart remains deferred until the planned improvements have evidence-backed dispositions and the useful changes are consolidated and tested.

### 2026-09-29 08:19 UTC — complete G2 component baseline; profiling started

- [M] All eight Ready arms are accepted and root-owned session `89286` exited zero. Independent audit verifies unchanged previous seven receipts, final LC2 1,063,432/1,063,432 native reinspections, all 1,417,721 paired records and query/root 1/1 with zero violations/uncovered/blocked obligations. All 32 recorded native/verification groups and the comparison controller have drained. Final Python guard 108.190 s, body 107.101 s, exit zero/no stop reason.
- Both finite pairs and both hot pairs meet the repeated numerical scheduled-work gate with observed wall ratios below 1.10. Finite domain ratios 0.7567/0.7549 and hot 0.7175/0.7232; observed whole-wall ratios 0.7241/0.7135 and 0.4638/0.4675. Unequal host activity remains an attribution caveat, not a timing correction. `joint_support_pruning` delivered `docs/research/codex_g2_ready_deployment_2026-09-29.md` for final independent review, retaining negative combined-control work results and all measured-versus-inferred limits. **No G2-only restart**, per the user's consolidation directive.
- [M] After independently confirming both measurement and metadata groups drained, root launched the pre-reviewed owned-process profiling diagnostic at 08:17:49 UTC, session `71170`, process group 1086503. Exact command: `env TMPDIR=/common/dev/rustred/TMP PYTHONDONTWRITEBYTECODE=1 RUSTRED_POST_READY_PROFILE_APPROVED=1 /nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python -B TMP/codex-post-ready-profile-2026-09-29/driver.py --output /common/dev/rustred/TMP/codex-post-ready-profile-2026-09-29/prepared --execute`. It uses the frozen Union executable and reviewed CPU64–79/W16/heavy-lock/1200+540 s policy. No production attachment or new benchmark comparison; sample quality and findings remain pending. Epoch native build waits for this group to drain.

### 2026-09-29 08:15 UTC — user defers restart until the improvements are consolidated

- Added the user's follow-up verbatim as an amendment to `CODEX_PROGRESS_PLAN.md` and made it authoritative in `GOAL.md`: a restart is acceptable, but not for G2′ alone. Complete evaluation/integration of the planned epoch and profiling-led avenues first. Rejected/unproven ideas are not automatically enabled; preserve evidence and reopening conditions. Scope, safety and performance gates are unchanged.
- Root informed implementation, measurement and review lanes. The running eight-arm comparison is unchanged and will become the component baseline, not trigger a restart. `joint_support_pruning` marks the draft report's G2-only commands as reference-only rather than current deployment instructions. No production process, input, checkpoint or resource setting was changed.

### 2026-09-29 08:09 UTC — epoch controller metadata gate passes

- [M] All eight native timing intervals finished before the metadata allocation. Final LC2 hot solve: 422.950 s, 1,417,721 scheduled/1,063,432 native; uncensored, exit zero/no stop or runner error. Its finalized after-native snapshot is 08:04:59.116 UTC and root independently found no members in owned group 905068. Full cold/Python verification remains in progress, so the accepted comparison count is still seven and the final report remains provisional.
- [M] `epoch_native_validation` executed the reviewed Stage1 command on frozen clean `f404cf66624cc6bad6c516c9169f40dd4a3166a3`. Metadata PASS: guard 41.168916 s, exit zero/no stop reason; Cargo 37.49 s, only `Checking rustred-app` and cached warnings, no broad rebuild. Evidence: `TMP/codex-epoch-s3.JjASCU/typecheck-controller-light/{request.json,stderr,result.json,child.json}`. Minimum headroom 586,095,497,216 B; peak **single child** 1,248,364 KiB, not aggregate RSS. No native tests ran.
- `active_goal_delivery_audit` independently confirms the actual CPU16–19/build1/j1/private-target command, frozen clean source and owned-group 990247 drain. The allocation ran 08:06:16.194–08:06:57.364 UTC, entirely after solver timing and during final verification. The benchmark report records this overlap without adjusting times or acceptance. Profiling and Stage2 have not launched; the existing sequence remains live and LC2 remains untouched.

### 2026-09-29 08:04 UTC — narrow metadata allocation prepared after timing drain

- Independent resource review supports allowing only the warm epoch metadata check after **all eight native solver intervals and owned groups have finished**, with finalized metrics/contention snapshots, while final cold/Python verification continues. CPU16–19/build1/j1 is disjoint from cold64–75/heavy and Python32–39/python-0. This revises the earlier all-work wait order explicitly; no launch yet. Preserve every verification deadline/result and record overlap: shared I/O/RAM can affect verification cost, so it is not a clean verification-speed comparison.
- The TMP epoch validation plan now records the exception. Frozen source and the normal private Cargo cache remain unchanged; stop unexpected broad dependency rebuilding. Profiling must wait for **both** final verification and metadata-owned-group drain. Native compilation/tests remain after profiling, not part of this exception. No solver timing, production setting or acceptance gate is changed.

### 2026-09-29 07:59 UTC — seven Ready arms fully accepted

- [M] `accepted-7.json` adds the reverse-hot Union arm after its Python audit passed in 723.337 s (guard 724.388 s, exit zero/no stop reason). Independent reviewer confirms unchanged earlier acceptances, all 794,916 native reinspections, 1,025,239 paired records bound to generation 3, query/root 1/1 closed and zero violations/blocked obligations. The root-owned session `89286` remains live and has started the final LC2-Off arm. No second hot-pair ratio or overall deployment conclusion yet.
- [M] One bounded read of LC2's live status records checkpoint generation 4 saved: start 07:45:25 UTC, saved 07:45:53 UTC, reported save duration 67.058 s. These fields describe different timing boundaries and are retained as reported. This is after reverse Union's native interval (07:39:24–07:42:42), during verification; no solver-time penalty is inferred. Production was not signalled or modified.
- `joint_support_pruning` is drafting the paired benchmark/deployment report from accepted receipts, leaving the final arm explicitly pending. The decision will distinguish the numerical work/observed-wall gates from causal timing attribution and full-production representativeness. The next epoch executor is ready but has launched nothing; the existing comparison and profiling allocations still take priority. Previous turn was progress (private failure-path tests, independently checked cold pass and pushed `5a827f67`), not a stopped or restarted measurement.

### 2026-09-29 07:53 UTC — reverse candidate cold pass; validation preparation advanced

- [M] The same live comparison sequencer completed reverse-hot Union natively in 197.730 s: 1,025,239 scheduled domains, 794,916 native inspections, 572.837 s waited-child CPU and 6.230 GB sampled aggregate RSS; uncensored, exit zero/no stop reason. Native cold reinspection and the oracle gate also pass (guard 187.309 s and 1.220 s). Its paired Python audit is still running, so the accepted total remains six of eight arms and the reverse LC2 baseline remains outstanding. This is not a completed second pair or a campaign-switch decision.
- [M] `closure_acceleration_research` prepared three missing observer tests: invalid explicit configuration/no parent creation, fresh-file header plus existing-file refusal, and partial-write failure preserving the prior complete line while disabling later writes. Existing configuration logic was only factored through explicit borrowed arguments to avoid global environment races. `active_goal_delivery_audit` independently accepted the formatted source; root checkpointed it privately as `1b894132563215c830008d6bee9868db85463dcd`. All 12 tests remain **unexecuted**. No main engine merge, planner change, CAS primitive, native rebuild or production action occurred.
- Independent epoch readiness review found no missing first-stage command prerequisite for frozen `f404cf66`: source/dependency state, tool paths, fresh receipt locations and private target ownership are ready. Actual validation still waits for the existing Ready sequence and short profiling run to drain. Require a real nonzero focused/full test count and report capacity skips; never run the old cached executable in place of Cargo's newly generated binary. No native execution claim follows from this preflight.
- Delegated the upcoming gated execution to `epoch_native_validation`, with stage-by-stage root allocation and independent result review. Its current authorization is narrowly reading the already-audited plan, not running or queueing a build during the comparisons; no new epoch architecture ahead of the frozen baseline.
- Deployment wording clarification from root source inspection: explicit roles preserve all 116 required queries and 67 auxiliary queries and permit safe rescue; they do not themselves add a live 116-query counter or unamended early termination. `add_rescue_report` emits per-query certification for amended results. Existing broad-helper dependency granularity remains a measured-opportunity backlog item, not a newly implemented optimization.
- Previous goal turn was **progress**: accepted hot pair one, private observer checkpoint and pushed `063eee48`. This continuation re-polled the confirmed-live session `89286`, advanced source-level failure-path tests and independently checked the next execution gate. The same measurements continue without restart; LC2 and the pending-growth metric remain untouched.

### 2026-09-29 07:42 UTC — first hot pair fully accepted; keep LC2 unchanged

- [M] `deployment-continuation/accepted-6.json` now accepts both arms of the first hot-sector Ready pair, after native completion, independent full cold reinspection, oracle checks and paired Python audit. Off/Union: 434.976/201.743 s whole command, 1,416,894/1,016,660 scheduled domains, 1,064,142/787,379 native inspections, 2,093.876/584.808 s waited-child CPU, 6.446/6.229 GB sampled aggregate RSS. Candidate Python audit took 705.308 s (guard 706.464 s), exit zero with no stop reason; it is verification cost, not solver time. All scheduled records are paired; all native records reinspected; query/root 1/1 closed; zero uncovered obligations or violations.
- [E] This first hot pair has 28.25% less scheduled work and 53.62% less measured whole-command time. Both finite pairs already showed 24.33%/24.51% less work and 27.59%/28.65% less whole-command time. Uncontrolled, unequal SMT-sibling and selected-core load remain confounders; no isolated causal speedup or full-production forecast is claimed. The same driver is running the reverse hot pair. **No campaign-switch recommendation yet:** require that pair's acceptance and final independent interpretation. No production action or duplicate measurement was launched.
- [M] Root read-only LC2 observation from the small live `status.json` at 07:41:26 UTC (not an independently frozen benchmark receipt): 68,603,165 discovered domains, 25,578,859 pending, 25,623,662 local completions, zero frontiers, approximately 39 GB RSS, still running. Its conservative initial-root counter is 6/67 with a 37-minute-old closure snapshot; this is not a count of the 116 required queries. One-hour coordinator shares are 53.61% ordered commit and 33.17% preparation; mean computing inspectors 3.54. The existing pending-growth metric is untouched. These figures motivate coordinator/epoch work but establish no closure ETA.
- Deployment remains conditional on the reviewed fresh-campaign runbook: G2 plus the complete required/auxiliary role declaration cannot cheaply and validly be added to LC2's existing request-bound checkpoint. If the remaining gates qualify a switch, only the user stops LC2 and starts a separate campaign; preserve the old directory and checkpoint for rollback. Progress would be recomputed. The prepared commands have not run.
- Next resource sequence is unchanged: finish and drain all eight Ready arms, run the independently reviewed short owned-process sampling diagnostic, then compile and execute the private epoch checkpoint/controller gates. The one-piece residual observer is source preparation only, independently reviewed but uncompiled/unexecuted; it is not a deployed optimization or a demonstrated gain.
- [M] Post-format independent source/math rereview passed for the isolated observer, now locally checkpointed as `c6f5813983115ffa67f063a5a161974c4026fd14` on `codex/g2-onepiece-shadow`. This private commit is not merged into `fable_5_1`; nine authored tests and feature-off/feature-on integration checks remain unexecuted. No new native binary or campaign behavior follows from that checkpoint.

### 2026-09-29 07:29 UTC — first hot baseline accepted; bounded observer prepared

- [M] `accepted-5.json` accepts the first hot LC2-Off arm:434.976s,1,416,894 scheduled/1,064,142 native,2,093.876s CPU,6.446GB sampled RSS. Independent reviewer confirms all1,064,142 native reinspections,1/1 root and1,416,894 paired records, zero violations/frontiers/uncovered; cold guard502.481s and Python guard109.212s, both exit0/null. Evidence remains under `TMP/codex-g2-pilot-prep.n7Kd5q/`.
- [M] Its Union counterpart completed natively in201.743s with1,016,660 scheduled/787,379 native,584.808s CPU and6.229GB sampled RSS. Cold verification/union lattice checks pass, but **Python acceptance and the reverse hot pair remain pending**. These preliminary ratios are not a deployment decision. SMT-sibling occupancy is unequal and unattributed:2.021/0.758 cores Off/Union; selected-core foreign occupancy goes the other way,0.871/1.879. This is mixed uncontrolled load, not uniformly lighter candidate surroundings. The same primary8 driver continues; no duplicate jobs or production actions.
- Source-only probe delivered in `.claude/worktrees/codex-g2-onepiece-shadow` from0cee158c, changes uncommitted pending independent review. Default builds exclude feature `g2-onepiece-shadow` entirely. The two observer hooks capture actual immutable dispatch inputs for no-covered-level/partial cases without a second lookup, membership test, new residual, authority decision or artifact change. Tooling caps64 matching attempts,8 samples/cohort,4,096 points,64 candidates,N<=32,1MiB/record and17MiB total; oversized samples are skipped whole. The limits censor diagnostics, never the solver's scope. Nine in-memory tests are authored but unexecuted after the review correction. Ready-order perturbation and prefix/ID/size bias remain explicit; no benefit is inferred from this synthetic preparation.
- `active_goal_delivery_audit` is reviewing observer bounds, provenance, no-feature behavior and mathematical interpretation. Runtime integration of one-piece tightening remains deferred pending positive actual-snapshot evidence and later native reinspection. Design receipt: the private worktree's `docs/research/g2_onepiece_shadow_probe_2026-09-29.md`.
- [M] 07:36 source-review follow-up: observer review passes for preparation after fixing diagnostic logging that could panic on unwritable stderr and clarifying encoding-work bounds. The ninth unexecuted mock covers the broken sink. Root formatted only the three touched Rust files with Nix rustfmt1.9.0 on CPUs32–39 (exit0); compilation/tests remain outstanding. Main engine and timed binaries remain unchanged.
- Epoch preflight independently reviewed: reuse the existing exclusively private `TMP/target-check`, **do not copy caches**. Matching G2/epoch archive names and sizes are not identity: their hashes and dependency fingerprints differ. Cargo must revalidate normally; no forcing timestamps/fingerprints. Frozen f404 remains uncompiled/unexecuted. Updated plan and exact hashes: `TMP/codex-epoch-s3.JjASCU/{cache-preflight.json,consolidated-native-validation-plan.md}`.
- Root corrected the upcoming epoch gate into four separate receipts: metadata check; native `cargo test --no-run` build; focused execution; full suite using that same newly generated/hash-checked executable. Actual test processes get1200s SIGINT deadlines plus60s kill grace under the existing resource/group-drain guard; compilation is recorded separately. No launch until primary8 and the short profiling allocation finish. Prior goal turn was **progress** (accepted finite repeat, pushed0cee158c, independently tested diagnostic driver); current live session89286 continues rather than being restarted.

### 2026-09-29 07:10 UTC — finite Ready gate repeated; hot comparisons active

- [M] All four finite Ready arms now pass full cold/Python verification, independently checked by `active_goal_delivery_audit`. Pair2 LC2-Off/Union:252.306/180.010s;1,276,229/963,440 scheduled;981,826/758,280 native;1,093.842/558.191s CPU;6.382/6.236GB sampled peak RSS. All roots/queries close1/1, every native record is reinspected and every scheduled record paired, without guard stops or violations. Receipt: `TMP/codex-g2-pilot-prep.n7Kd5q/deployment-continuation/accepted-4.json`.
- The repeated domain reductions24.3%/24.5% meet the finite control's numerical work threshold. Observed whole-wall reductions27.6%/28.7% retain a contention caveat: Off/Union sibling occupancy3.317/1.283 and3.096/1.402 cores, largely user-space but not process-attributed. These are not isolated causal wall-speed measurements. The same primary8 driver has started the first hot baseline; **no switch recommendation until remaining controls and interpretation complete**.
- `joint_support_pruning` prepared `TMP/codex-g2-pilot-prep.n7Kd5q/CONDITIONAL_DEPLOYMENT_RUNBOOK.md`, independently source-reviewed. It preserves all183 input objects/order, adds116/67 roles, reuses the tested frozen binary and proposes a disjoint fresh campaign. Existing Python freezes Off mode; native activation cannot simultaneously change the role-bound request. No cheap validated combined continuation exists. Commands remain unexecuted; only the user may stop/start/resume production. Printed plan fields and separately staged role/owner checks are distinguished.
- `closure_acceleration_research` prepared the post-Ready sampling driver in `TMP/codex-post-ready-profile-2026-09-29/`, source SHA3cf6bf82458243d10305857055e60edc01beeec88e13c5ad1191b773d3c7968b. Independent source audit passes. Root separately repeated all8 mocked guard/launch tests:0.066s, guard1.185s exit0/null; receipt `independent-mocks/`. No perf/native/build ran. Owned-process-only sampling retains1200+540s bounds, CPU64–79/W16,250/150GiB headroom and heavy lock; actual sample/unwind usability remains untested.
- Epoch author is preparing only the resource/cache preflight for the frozen f404 validation. No further architecture is being added ahead of the first native baseline. A private copy of the idle G2 release cache may reduce compilation; no cache copy/build is allowed until the timing sequence and profiling allocation clear.
- [M] Read-only LC2 snapshot07:07:65,556,465 discovered,24,448,891 pending,24,486,432 local completions, zero frontiers,37.69GB RSS. Its6/67 conservative root counter is204s stale and is not the116-required-query completion counter. The live process and checkpoint are untouched; no termination ETA follows.

### 2026-09-29 06:58 UTC — exact traversal research independently critiqued

- Read-only primary-literature and source review is delivered in `docs/research/codex_exact_closure_acceleration_2026-09-29.md`; `active_goal_delivery_audit` independently accepts its deliberately conditional recommendation. No engine change or new closure/speed claim follows from the research.
- A bounded future shadow test may ask whether one residual can shrink in coordinates and A/R as well as D: include every point not proved covered by eligible older anchors, take its representable bounding constraints, and intersect with the original domain. This preserves exact union coverage but needs actual dispatch-time snapshots, whole dependencies and independent checks. The34,345 no-covered-D-level jobs are merely a sample cohort, not observed opportunities. Schema/replay changes are not authorized absent worthwhile measured shrinkage.
- Symbolic capacity fallback has zero opportunity in the observed Ready finite control; multiple D pieces remain deferred on sparse historical incidence. Generic WQO/recurrence acceleration has no proved guard/pole invariance and universal RHS-exit coverage here. The literature does not remove these obligations or imply eventual production termination.
- Next diagnostic preparation is a separate guarded, owned-process CPU sampling run using the frozen Union binary and the same finite input after primary8 finishes. The author may prepare/mocked-test the driver now, but no perf/native/build execution competes with the active comparisons. This follows the measured coordinator/helper/commit bottleneck priority rather than speculative engine work.
- Previous goal turn classification: **progress**—first-pair acceptance and source-only epoch mapping were independently verified and recorded in pushed73f48d68. Current pilot session89286 was polled live; it continued the same reverse-baseline run, not a restarted campaign.

### 2026-09-29 06:53 UTC — first Ready pair fully accepted; repeat and hot controls pending

- [M] Both arms of the first finite-five-loop Ready pair pass native completion, full cold reinspection and paired Python audit. LC2-Off/Union:249.626/180.746s whole command;1,275,122/964,909 scheduled domains;979,714/758,183 native inspections;1,082.484/564.716s waited CPU;6.361/6.239GB sampled peak aggregate RSS. This is27.6% less wall time and24.3% less domain work on this control, **not full production closure**. Receipt: `TMP/codex-g2-pilot-prep.n7Kd5q/deployment-continuation/accepted-2.json`.
- Contention caveat: selected-core foreign load was1.146/1.076 cores, but SMT-sibling load was3.317/1.283 cores over the arms, favoring Union. Preserve that confounder. Reverse-order repetition and both hot-sector pairs remain necessary; **keep LC2 running unchanged, no deployment recommendation yet**.
- [M] Reverse-pair Union native solve:180.010s,963,440 scheduled/758,280 native; cold full verification passes, Python pairing remains in progress. Root owns the existing sequential primary8 driver/session89286; `joint_support_pruning` independently aggregates receipts, and `active_goal_delivery_audit` reviews the interpretation. No duplicate measurement jobs are launched.
- [M] 06:55 update: reverse Union Python audit also passes; `accepted-3.json` now records the fully accepted arm. The reverse LC2-Off native run has started. The matched second-pair comparison is still incomplete.
- [M] Epoch private controller source is frozen in validation at `f404cf66624cc6bad6c516c9169f40dd4a3166a3`, identical tracked tree to publisher `1e1d0943` (`a1c287437c493ee76a1c73bdd5f11f1a3ea8a5c6`). Mappings:1bacf62f→ef60f9a5;1e1d0943→f404cf66. Source audit passes; **this advance has not been compiled or executed**. Last actual metadata PASS remains52d7f57f. No competing builds until primary8 drains. Receipt: `TMP/codex-epoch-s3.JjASCU/controller-source-advance.json`.
- The epoch native-wrapper smoke accepts any successful typed outcome, so it is not yet an interruption-equivalence or closure oracle. The next gate is frozen metadata plus one native focused/full-suite executable; real-native uninterrupted/interrupted equivalence, failure lifecycle, public wiring and1.5x performance qualification remain open. Do not treat the lockstep/private skeleton as finished deployment architecture.
- Research continues read-only alongside the runs. Finite Union r1 has **zero** unbounded/point-cap/test-budget G2 fallbacks; its G2 planner takes5.630466s of79.936175s traversal. Thus a symbolic-cap fallback has no demonstrated opportunity here. Interior multi-band reuse likewise needs new evidence: earlier partial-domain census found2,284/2,298 residuals already single-piece. These are conditional research avenues, not approved implementation work.

### 2026-09-29 06:31 UTC — first Ready baseline accepted; candidate verification ongoing

- [M] Finite Ready LC2-Off repetition1 independently passes all gates:249.626s whole command,1,275,122 scheduled domains,979,714 native inspections,1/1 required query closed, all native records cold-reinspected and1,275,122 records paired by the Python audit. Evidence: `ready-pairs/deployment-ready-lc2-off-r1/five-finite/` and `deployment-continuation/accepted-1.json` under `TMP/codex-g2-pilot-prep.n7Kd5q/`.
- [M] Its matched Union solve completed uncensored in180.746s with964,909 domains/758,183 natives. Cold/Python verification is ongoing, and repetition2 plus the hot-control pairs remain outstanding; **no deployment conclusion yet**. Baseline and candidate sampled foreign load averaged1.146/1.076 cores on the16-core allocation; keep contention and later pair outcomes visible.
- Independent bounded post-G2 review, cross-checked against source by the epoch author, prioritizes attribution of coordinator/helper wait, commit and observer/output costs. The roughly19% Ordered coordinator progress/observer bucket is not a19% lean-allocation opportunity. Condensed evidence and candidate reopening conditions are in `docs/research/codex_next_candidates_2026-09-29.md`. Successful inspection records discard source-partition/term maps, so scope-filtering broad-anchor dependencies cannot safely be inferred from current transcripts; that idea remains deferred.
- [M] LC2 read-only snapshot06:31:62,518,566 discovered,23,307,475 pending,23,308,957 local completions, zero frontiers,6/67 conservative initial roots closed (snapshot511s old),35.97GB RSS, checkpoint generation3. One-hour commit/preparation shares46.33%/31.89%; mean computing inspectors3.31. Pending-growth-per-completion is observed unchanged at0.486, not redefined. No finite termination estimate follows; the production campaign remains untouched.
- Epoch source audit has caught pre-dispatch capability/resource failures being classified as durable engine poison. Worker license refusal is now separated in the private source and covered by a reopen regression; startup allocation/thread-spawn handling is being corrected similarly. These are unexecuted private slices, not deployed failures. Public activation, interruption/native gates and measured performance remain pending.

### 2026-09-29 06:17 UTC — audited opt-in milestone pushed; Ready comparisons started

- [M] Main `e757fbbf` is pushed and synchronized with `origin/fable_5_1`. Only the pre-existing user FeynKit edits/reference files remain dirty or untracked. Combined G2/rescue and Python steering are integrated; LC2 was not changed.
- The finished preparation agent could not be reactivated because the collaboration tool reported its thread limit. Root therefore owns measurement orchestration; `checkpoint_final_audit` remains independent reviewer and `epoch_s3_delivery` remains implementation author. This does not change the workload, acceptance thresholds or resource policy.
- Primary8 run started06:17:46 under exec session89286: `env TMPDIR=/common/dev/rustred/TMP PYTHONDONTWRITEBYTECODE=1 RUSTRED_TESTS_REQUIRE_LICENSE=1 taskset -c 32-79 /nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python -B TMP/codex-g2-pilot-prep.n7Kd5q/execute_deployment.py`. Controller affinity allows the recorded native64–79/W16 finite and64–75/W12 hot allocations and32–39 audit allocation, excluding production128–227. Exact commands/inputs are frozen in `DEPLOYMENT_PLAN.json`; each accepted arm gets an immutable snapshot in `deployment-continuation/`. No optional mechanism arms are authorized.
- The small driver reuses the audited guards and runs existing full native reinspection/Python gates after each arm. Review caught a signal between the old helper's stop check and child assignment; the corrected helper forwards that cancellation to its newly owned guard and waits for drain. Original control-matrix helper is retained verbatim (`3ed07f5d`); corrected helper `57f9e224` and new driver `6b9432b4` received independent source review and a separately repeated mocked-race regression. No timed solver/native/production code changed. Selected-core and SMT-sibling counters are captured read-only outside solver timing. Incomplete outcomes remain censored and stop the sequence.
- [M] Epoch async executor source slice `1bacf62f` is independently audited but uncompiled/unexecuted. Review fixed a poisoned-queue polling liveness bug with a retained-sender regression. Its held-worker test now exercises private durable save before joining and rejects late buffers. Private stop/controller wiring remains active; no public resume or performance claim. No new builds run concurrently with the Ready lane.

### 2026-09-29 06:06 UTC — G2/rescue integrated after all14 representative gates

- [M] Final finite-Union Python audit passed:969,467 records paired to cold generation3,752,033 natives independently reinspected,80,181 exact G2 covers/215,989 anchor links checked, all required queries closed and no frontiers/errors/uncovered domains. Native226.745s; separate cold guard152.224s and Python guard446.320s. Every one of14 native runs and63 per-stage guards passed and drained; `CONTROL_RESULTS.json`/`CONTROL_DRAIN.json` retain exact receipts.
- Independent auditor approved the complete matrix and conflict-free merge. Main integration is `9ef5464d`, with bounded metric-reader fix cherry-picked as `77059cdf`. The **entire committed** crates/Cargo/.cargo/vendor tree is identical to tested native `d12db6cf`; Python steering equals tested `cd52c90d`. User-owned dirty FeynKit content retained identical SHA256 `33819cb3e5053a59f3587741e29b297a3a5b53bfa35dabcbe2e8fe95fe700332`; reference-only/untracked material was not staged.
- [M] Root post-merge Python suite passed284 tests in22.601s (one expected optional enumeration skip, already separately tested); tooling suite passed24/24 in3.860s. Guards exited0/null in24.175s/4.158s, all processes drained. Exact commands and receipts: `TMP/codex-g2-rescue.tzdFuj/post-merge-{python,tooling}/`. Neither required native recompilation because the merged native source is identical to the frozen tested source.
- [M] Epoch body/private runtime restore/session lifecycle metadata-only check passed at validation `52d7f57f`:41.178s guarded,37.33s Cargo, peak single child1,226,180KiB. Receipt `TMP/codex-epoch-s3.JjASCU/typecheck-lifecycle-light/`. All check processes drained. This is not native execution or a working public checkpoint claim; source-only async work was excluded.
- Next executable steps: push this audited opt-in milestone; launch only the preregistered eight Ready deployment arms. No LC2 action or production-switch recommendation yet. The original foreground/production input and pending-growth metric remain untouched.

### 2026-09-29 05:58 UTC — final control verifying; production remains read-only

- [M] The finite-five-loop Union native run completed without censoring:969,467 scheduled domains and752,033 native inspections, versus1,273,376/967,621 for Off. Whole-command times226.745/310.284s are one Ordered correctness pair, **not** the matched Ready deployment gate. Full cold reinspection passed; final Python audit and corrected counter sidecar are still running, so the aggregate remains13/14 accepted.
- [M] LC2 read-only observation near05:58:58,038,854 discovered,22,178,208 pending,21,190,197 local completions, zero frontiers and6/67 conservative initial roots closed (snapshot455.9s old, not116 required-query closure). RSS approximately32.6GB; checkpoint generation3 unchanged. One-hour coordinator time is45.55% commit and34.61% preparation versus1.49% progress JSON, with2.99 mean computing inspectors. No completion ETA follows from these observations; production was not altered.
- Epoch private body validation and runtime restore/session reservation/replay are source-audited at `0762955c`/`7005de75`. Their clean validation projection is `52d7f57f`; a warm metadata-only check is queued after the final control's verification processes drain and before Ready timings. The asynchronous save-before-join slice remains separate unfinished work. Neither source review nor type checking establishes usable public checkpoint resume.
- The initial Ready comparison is limited to eight deployment arms (finite and hot controls, LC2-Off/Union then Union/LC2-Off). Candidate-Off mechanism comparisons are separate, not automatically launched. Identical bounded metric-reader revision3832 is frozen for both arms. Sampled heartbeat maxima cannot qualify as exact peak-pending savings; use whole time, scheduled work or sampled aggregate RSS with the preregistered regression/noise conditions.

### 2026-09-29 05:39 UTC — all four-loop control gates pass; finite five-loop next

- [M] All12 Off/Union four-loop combinations pass: FG, BMW, H, X, combined four-loop, and its partial-initial variant. Each Off has exact historical LC2 identity; every native record is cold-reinspected, and paired Python audits pass with no uncovered obligations/frontiers. The finite-five-loop pair is the only remaining representative correctness gate.
- Preserve the negative work results: combined Off/Union scheduled domains65,444/68,184 and native inspections30,159/32,085; partial-initial variant68,483/71,284 domains and31,717/33,663 natives. Valid G2 reuse can increase work elsewhere; it is not a universal improvement and remains opt-in. These are single Ordered controls, not deployment timing evidence.
- The combined controls have58 required input queries mapped to32 distinct initial roots; all58 queries close. The table/index must distinguish queries from roots. The finite control has one input region containing1,324 initial integer tuples, not1,324 independent query rows, and is not the full116-query production scope.
- Tracked bounded-extractor correction is committed locally as `3832bf58` (two tooling files), with24 focused tests independently repeated successfully. The running controls still use the frozen original runner and corrected sidecars. Use the fixed runner identically in both future Ready arms; no Rust rebuild is needed for this tooling-only change.

### 2026-09-29 05:32 UTC — BMW pair passes; remaining controls run sequentially

- [M] BMW Off/Union both pass full native cold verification and paired Python audit for268/268 required queries. Off exactly matches the historical LC2 records/counters:158,951 scheduled domains/147,233 native inspections. Union schedules129,616 domains/119,727 natives. This is one Ordered correctness pair, not the repeated Ready performance gate. Four of14 mode/control combinations now pass in `TMP/codex-g2-pilot-prep.n7Kd5q/CONTROL_RESULTS.json`.
- A small local continuation loop now executes the already-validated commands sequentially, stops on failure and preserves individual native/cold/audit receipts. It does not introduce another production scheduler or change the frozen runner; each job retains its own resource/time guard. A separate tooling branch corrects the bounded benchmark extractor; native binaries and the executing runner remain frozen.
- Epoch body-reader integration `0762955c` is independently source-audited and locally committed, not yet compiled/executed. It streams selected record fields and validates the same bytes through digest completion. Review added actual initial-D-band C0/C2 roundtrips and an anchor/record dispatch-version equality check. Next is runnable owned-state construction, durable session reservation, saved-job replay and interruption-before-join tests.
- Eventual epoch deployment must compare against the best contemporaneous validated legacy setup, including G2 if it qualifies, not only a stale flag-off baseline. Increased native calls/second alone cannot substitute for completing the same requested work faster. Private epoch still refuses G2/rescue combinations; that missing capability is explicit.

### 2026-09-29 05:25 UTC — read-only LC2 check; coordinator work still dominates

- [M] LC2 is running without a stop reason:53,075,126 discovered,20,897,576 pending,19,005,615 local completions, zero reported frontiers, RSS30,650,232,832B and checkpoint generation3. The conservative initial-domain count is6/67; its closure snapshot is about21.5minutes old and is not the count of116 required queries.
- [M] One-hour coordinator fractions: commit50.85%, preparation31.32%, progress JSON1.22%. Mean computing inspectors4.05; instantaneous aggregate busy cores11.84 of100 reserved. These observations continue to prioritize repeated-work reduction and inspector-side lookup/merge design over claims of a large telemetry-only gain. The unchanged pending-growth-per-completion is0.918; it is not a closure fraction or an ETA.
- No production process, input, checkpoint or setting was changed. Representative pilot resources remain separate and comfortably above their headroom floor.

### 2026-09-29 05:20 UTC — first representative pair passes; reporting defect retained

- [M] Ordered FG, W6 on CPUs64–69, passes both modes. Off is exactly identical to the historical LC2 control (98,909 records; containment checks169,509,549). Union has98,867 records/98,843 native inspections,305 G2 records and314 anchor links. Both independently cold-reinspect all native records and close248/248 required queries with no frontiers or uncovered obligations; paired Python audits pass. Evidence: `TMP/codex-g2-pilot-prep.n7Kd5q/ordered-controls/{off,union}/fg/`. This single correctness pair is not a performance decision.
- Independent receipt review caught a pre-existing benchmark extraction defect: unqualified head/tail field matching took `successors`/`events` from a nested record rather than the root totals. Example: Off FG reported1,242/1,498 instead of2,182,549/2,488,138. Solver output, strict identity, full verification and wall timing are unaffected.
- A separately guarded root-aware streaming postprocessor now writes `corrected-metrics.json` sidecars, preserving original receipts and recording mismatches/parser identities. Two independent shadowing/truncation regressions pass. No comparison will use the incorrect counters. The author will also correct the tracked bounded extractor before release without adding an unbounded scan to pilot teardown; this tooling change does not require rebuilding or rerunning the solver.
- Native/Python merge preview is conflict-free and preserves main authoritative documents; all native sources/dependencies match tested `d12db6cf`, frontend/tooling matches `cd52c90d`. Actual merge remains gated on the other six representative controls and preserves unrelated FeynKit/untracked work.

### 2026-09-29 05:13 UTC — full real-process composition gate independently accepted

- [M] Corrected Python audit passed in 6.139 s (guard7.163 s, exit0/no stop reason). It accounts for 19 carried unfinished attempts, all 305 G2 records/314 anchor links, and all 98,867 closed nodes without disagreements. Original invocation failure remains intact. Author and independent auditor checked all 124 owner payload hashes and all 248 staged query objects against the unchanged originals; all four owned process identities have drained.
- This closes the real Python→native fresh-Union/pause/ordinary-resume/cold-verification composition gate. It does not close the seven representative controls, establish durable accepted-pin process replay, or measure a speedup.
- [M] Epoch typed-counter source `c772f9c6` is independently audited; consolidated validation `c51839c1` passed the warm metadata-only check with earlier record/root slices. Receipt `TMP/codex-epoch-s3.JjASCU/typecheck-roots-records-light/`: exit0/no stop reason,39.177 s, Cargo35.77 s, peak1,209,024KiB. Root independently checked it. No native runtime test is implied; build1 has drained before G2 control measurements.

### 2026-09-29 05:12 UTC — real Union pause/resume and cold reinspection pass

- [M] The real Python/native process smoke completed in 21.699 s including preparation. First leg paused cleanly (exit4) with a durable checkpoint after 156 published/351 scheduled domains; ordinary second-leg resume retained Union and completed (exit0). Owned groups drained without resource/kill/observation errors. Final result has 98,867 domains, all 248 required roots closed, and 305 G2 records (299 residual, six full) with 314 anchor links. Evidence: `TMP/codex-g2-pilot-prep.n7Kd5q/python-union-smoke/`.
- [M] Separate cold native verification passed: 98,843/98,843 native records independently reinspected; 248/248 roots closed, zero errors/frontiers/uncovered. Guard exit0/no stop reason, 11.156 s. This is correctness evidence, not a matched performance comparison.
- Limitation: the pause preceded the first accepted G2 loan. This process test proves sticky Union and real later reuse, not accepted-plan replay across a process boundary; the latter is covered by native seam tests. Preserve that distinction.
- First Python audit invocation failed before scanning because `--command` expects a JSON array, not the supervisor request object. Source inspection confirms omitting the argument invokes its existing request-object handling. Root authorized this invocation-only correction into fresh evidence, retaining the failed receipt; no source/assertion change or rerun of the native solve.
- Next authorized lane: seven sequential Ordered controls, flag-off identity and Union independent reinspection, on the original roleless query bytes. Stop on substantive failure; matched performance pairs remain gated on these results. The epoch lane may first run one bounded warm metadata check for audited record/root/counter slices, then releases the measurement slot.

### 2026-09-29 05:06 UTC — real smoke starts; restore-record provenance decision

- [M] The authorized real Python/native smoke has launched in fresh `TMP/codex-g2-pilot-prep.n7Kd5q/python-union-smoke/`, with the audited controller owning the heavy lock and both process lifetimes. No outer process-group-only wrapper is used. Final outcome and independent cold verification remain pending; no production path is touched.
- Epoch source-only slices `b14c4231` (streamed record registry/body-byte authentication) and `e0c7510a` (prepared-reducer-derived root phases/geometry) are independently audited and committed locally. They are not yet typechecked/executed. Root validation must use the actual reducer/request, not saved phase labels; record-body semantics and runtime state/session/stop assembly remain unfinished.
- Decision: add a small typed **epoch-only** resolver-counter subobject to newly written native records. Existing records persist accepted/emitted events but omit some successor/conditional totals, and native statistics can include a breaking event refused by the resolver on C2. Thus those totals cannot generally be reconstructed from native statistics alone. Preserve the existing algebra/report fields and legacy records; share writer/reader schema, add C2/counter mutations, and independently review. Do not alter pending-growth computation/rendering or introduce an old-checkpoint importer.
- No metadata build overlaps the real smoke/cold-verification phase. Source work may continue privately; next check waits for root resource release.

### 2026-09-29 05:04 UTC — CLI gate passes; real composition smoke authorized

- [M] `campaign-cli-tests/` passes all six external CLI tests (0.16 s; zero failures/ignored). Guard exits 0/no stop reason, 1,044.515 s including 17m22s test-target compilation, peak single child 8,899,048 KiB. Root independently checked stdout/result and the unchanged candidate hash.
- Frozen executable: `TMP/codex-g2-rescue.tzdFuj/candidate-bin/rustred-d12-8169221a`, read-only copy, SHA-256 `8169221a8977ae261e777ddca5ac9e82fcb339377362d988472930595b1ea341`. Source remains `d12db6cf`; heavy/build0 locks have drained. No comparison timing yet.
- Root authorized the independently audited real Python Union smoke on unchanged FG248 inputs, Ordered/W6 on CPUs64–69, with no auto-rescue. Use the final controller `2d4c4a81…`, Python `cd52c90d`, shared heavy lock, 250/150 GiB host headroom and the registered 1,200+540 s bounds including preparation. Only its own fresh TMP stop-file/process identities may be touched.
- If both legs pass, separately run full cold reinspection and the Python result audit; real PASS and positive equal verified/total counts are required. Any failed gate stops this lane for diagnosis. This authorization is not a claimed smoke result, performance measurement or production switch.

### 2026-09-29 04:56 UTC — LC2 observation and final smoke-mode assertions

- [M] LC2 remains running and untouched: 49,547,121 discovered, 19,529,574 pending, 17,602,450 local completions, zero reported frontiers, 6/67 conservative initial obligations closed, RSS 28,616,056,832 B, latest save generation3. One-hour coordinator shares remain dominated by commit52.10% and preparation30.59%, versus progress JSON1.21%. No scoped-closure or ETA claim follows from these readings.
- [M] The real-smoke controller now also checks the actual native argv and paused/final native report for Union mode, rather than relying on Python metadata. Its independently reviewed pure mutation test passes (guard exit0/no stop reason,1.150 s), complementing the unchanged seven ownership mocks. Final controller SHA-256 `2d4c4a817fee76d3fa6e043aca634b3659cc9446ab8d3d8f5ab6c69cb7bc72ab` supersedes the earlier draft identity. Evidence: `TMP/codex-g2-pilot-prep.n7Kd5q/controller-native-mode-test-independent/`. No positive loan count is required for this functional test; real native execution and cold verification are still pending CLI-test compilation.

### 2026-09-29 04:52 UTC — structural cross-state restore validation typechecks

- [M] Independently audited source `8cea173a` adds structural ledger/flag/edge/alias/anchor checks to the private provisional loader. It reuses existing exact containment and cover validators; an owned scratch flag avoids a second full-size bitmap and is cleared on success/error. It does not make saved closure bits authoritative or enable runnable resume.
- Two pre-build findings were corrected and retained: aggregate frontier counts also include initial-input obligations, not just native C4/C2 contributions; merged Native nodes must have epoch at least one, since publication stamps `k+1`. Regressions include these cases and refusal of an inconsistent closed node with an unresolved descendant.
- [M] Validation `613e9c6f86dc5aa5cd99bad39060c24d01b6d188` passes the same metadata-only check. Receipt `TMP/codex-epoch-s3.JjASCU/typecheck-cross-state-light/`: exit 0/no stop reason, 40.165 s (Cargo 37.54 s), peak 1,211,848 KiB. Root independently read the receipt. These new native tests have **not** executed.
- Next: stream sealed-record summaries through existing serde machinery, independently derive root phases/conditions from reducer+request, assemble validated runtime state, then durable session reservation and save-before-join behavior. Avoid a new arbitrary record-size restriction; the existing writer's legitimate records must remain readable. No general checkpoint compatibility project is introduced.

### 2026-09-29 04:47 UTC — previously omitted 50-worker subcases exercised

- [M] Root ran the existing frozen release test executable `227d2564…` on 50 available physical CPUs32–81. All 13 selected tests passed, with no failures, ignored cases or skip diagnostics: 0.13 s native test time; guard exit 0/no stop reason, 1.162 s, peak 15,420 KiB. Receipt: `TMP/codex-g2-rescue.tzdFuj/native-width50/`.
- Exact executable filters: `application::routed_campaign::tests::owner_batches::`, `initial_overlap_actual_native_residual_and_alias_match_across_workers`, and `native_walk_transfer_runs_serial_two_six_fifty_and_default_keeps_every_job`, with `--test-threads=1 --nocapture`. These exercise the ten previously omitted W50 subcases plus three nearby tests. The original 16-core omissions remain recorded; this separate receipt supplies the missing width coverage, not a worker-utilization or speed benchmark.
- Resource decision: these prebuilt tiny one-/two-loop correctness tests used build1/headroom guards while the CLI test target compiled on0–15 under the sole heavy lock. Their earlier full-suite peak was about83 MiB; this subset measured15 MiB. No performance pilot overlapped. The light slot is now released. The guard used a 300 s owned-test timeout, not a production/campaign limit.
- Independent reviewer confirmed the ten source loops, the three additional tests, absence of skips, source `d12db6cf`, and unchanged test/candidate executable hashes. Physical-core identity for CPUs32–81 was separately checked by root with `lscpu`; this is a correctness allocation, not an exclusive-host reservation.

### 2026-09-29 04:46 UTC — saved-dispatch check passes; no public resume yet

- [M] Audited source `a21d76bc` adds bounded saved-dispatch decoding and shared reservation validation; it preserves exact retry/deferred order, attempt classes, in-flight descriptors, cursor and sequence checks. A zero requested refill budget now refuses before popping or mutating queues. The decoder returns provisional data, not a runnable dispatcher.
- [M] Validation `977872595108c1788737305e4eb9941366ef9ef1` combines that slice with `7f31433f` and passes the same one-worker metadata-only check. Receipt `TMP/codex-epoch-s3.JjASCU/typecheck-dispatch-light/`: exit 0/no stop reason, 40.167 s (Cargo 37.34 s), peak 1,179,528 KiB. Root independently read the receipt. No native execution or broad dependency rebuild; cross-state WIP is excluded.
- Upcoming session allocation must reject counter/session overflow before issuing a reused sequence. Restored closure flags remain non-authoritative until full graph reconstruction; mutation tests must not let a forged closed flag bypass an unresolved descendant. S3 remains incomplete until these cross-state, durable-session and stop-before-join gates work end to end.

### 2026-09-29 04:44 UTC — optimized G2′/rescue executable built successfully

- [M] Frozen native source `d12db6cfa23c09f7d9c2946416ea49763ece48f0` campaign build passes. Receipt `TMP/codex-g2-rescue.tzdFuj/campaign-build/`: exit 0/no stop reason, 3,446.419 s (Cargo 57m24s), peak single child 15,056,956 KiB, minimum host headroom 794,792,742,912 B. This is **compilation only**, not a solver performance measurement.
- [M] Candidate `target/campaign/rustred` SHA-256: `8169221a8977ae261e777ddca5ac9e82fcb339377362d988472930595b1ea341`. It uses the matched campaign profile (fat LTO, one codegen unit, no incremental build or mimalloc override). The binary will be frozen separately before controls.
- Same-source `cargo test --profile campaign --locked --offline -j8 -p rustred-app --test cli_routed_campaign -- --test-threads=1` is now running under CPU0–15/heavy/build0 guards with `CARGO_INCREMENTAL=0`. Receipt directory: `campaign-cli-tests/`. No CLI test outcome, real pause/resume or performance comparison is claimed yet.

### 2026-09-29 04:37 UTC — real composition harness independently checked

- [M] The TMP-only Python/native smoke controller passes independent source review and seven owned-process mock tests (10.384 s; guard exit 0/no stop reason, 11.157 s). Evidence: `TMP/codex-g2-pilot-prep.n7Kd5q/controller-mock-tests-independent/`. No actual campaign ran.
- Retained negative finding: if the supervisor exited before first observation, its native child could already be reparented, fail the original parent check and escape tracking. The author corrected authenticated receipt recovery after verified supervisor exit and added a delayed-observation orphan test. Actual native CPU affinity is now checked too. Bad receipts/affinity and launch failures remain censored; PID reuse must not cause a signal to an unrelated process.
- A successful process smoke will still require separate cold reinspection before acceptance. Campaign CLI build and six CLI tests remain prerequisites. A short, separately guarded 50-worker rerun of the existing test binary is queued to exercise the ten width subcases omitted under the earlier 16-core allocation; no new compilation or timing claim is involved.

### 2026-09-29 04:27 UTC — anchor/frontier decode audit delivered

- [M] Epoch private decoder slice `7f31433feea24e88abcd29f4d0499d792a029317` is committed in the isolated publisher branch after independent source review. It reuses the existing anchor codec with bounded per-record scratch, validates counts/provenance/order and digest, and decodes sparse frontier entries. Review caught a test-only visibility gate on the reused codec; the author removed that gate without duplicating or changing its algorithm. No native test/typecheck receipt is claimed for this new slice; the next metadata check will include a coherent dispatch/cross-state addition.
- Preserve the semantic distinction during restore: sparse frontier counts describe C4/native-frontier records, while aggregate frontier counters can also include C2/error prefixes. Requiring equality between those differently scoped counters would reject legitimate checkpoints. Full-state validation must check each against its proper record source.
- Main audited documentation milestone `fb3684e7` is pushed. G2′/rescue runtime code remains isolated pending representative CLI/campaign gates; LC2 and unrelated user changes remain untouched.

### 2026-09-29 04:23 UTC — provisional assembly typechecks

- [M] Validation `43253bfe06bb0bfb61252bd502a6823455db7b46` (audited source `03bd236d`) passes `cargo check --release --tests --locked --offline -j1 -p rustred-app` in the private warm target. Receipt `TMP/codex-epoch-s3.JjASCU/typecheck-assembly-light/`: exit0/no stop reason,39.162s (Cargo35.78s), peak1,184,068KiB. Root independently read the receipt and checked the clean validation tree. The light build1 slot is released; no native tests or usable resume are claimed.
- New variable anchor/frontier decoder work remains isolated and is not covered by that check. Next author/auditor gate is actual-codec roundtrip and malformed-section regression review, followed by full cross-state validation; no production or public epoch activation.

### 2026-09-29 04:21 UTC — provisional assembly reviewed; live priorities unchanged

- [M] Private epoch manifest/scalar assembly committed as `03bd236d654757528b70a1708e377418aca1df3e`, independently source-audited. It binds request/actual batch size, owner inventory, section counts and allocation limits before returning provisional arrays and lookup. It does **not** construct a runnable epoch state or enable resume. Roots, records, dispatch, full cross-state checks, session reservation and durable stop handling remain open. A one-worker metadata-only check is authorized in the same bounded light slot; execution remains pending.
- [M] G2′/rescue campaign build has reached the final binary's fat-LTO link at frozen `d12db6cf`; the six external CLI tests and real Python/native smoke remain queued. No candidate binary/timing is claimed yet.
- [M] Read-only LC2 snapshot: running, 44,793,072 discovered, 17,647,027 pending, 6/67 conservative initial obligations closed, RSS26,380,632,064B. Generation3 remains the latest completed save. Over the reported one-hour window, commit and preparation account for53.20% and29.42% of coordinator elapsed time, versus0.94% progress JSON. This continues to favor successor/merge work over a large claimed telemetry gain. No eventual-closure inference or production intervention.

### 2026-09-29 04:15 UTC — consolidated decoder check passes after retained test correction

- [M] A narrowly allocated metadata-only slot ran alongside the heavy candidate
  code-generation build: CPU16–19, one Cargo worker, private warm check target,
  build1 lock and existing250/150GiB headroom guard. Earlier checks measured
  about1.1GiB, and this job performs no native code generation or solver timing.
  The campaign build remained on0–15 and retained the sole heavy-job lock.
  No timed pilot overlaps either validation job.
- [M] First frozen `dc046711` check failed with E0277: a new regression compared
  runtime usize index pairs with persisted u32 pairs. Receipt
  `TMP/codex-epoch-s3.JjASCU/typecheck-restore-light/`: exit101/no stop reason,
  37.172s. The independently reviewed correction widens only the expected test
  indices, retaining order/cardinality and every assertion; runtime is unchanged.
- [M] Source fix `770ff058`, validation `33b0c2ab27c6fef7e1972d30b62a2b9aaed836b4`:
  retry `cargo check --release --tests --locked --offline -j1 -p rustred-app`
  passes. Fresh receipt `typecheck-restore-light-retry/`: exit0/no stop reason,
  38.168s (Cargo35.14s), peak1,171,996KiB. Root independently read the receipt.
  This includes corrected decoders/lookup regression sources, **not executed
  native tests, full-state assembly, or working resume**. Assembly WIP remains
  isolated in the publisher tree.
- Retain timing CPUs64–79 (hot64–75): alternative short samples showed similar
  contention rather than a materially cleaner lane. Finite Ordered runs are
  identity controls; finite deployment pairs use Ready, matching production's
  policy, at matched W16. The hot control uses matched Ready/W12. Per-arm contention
  recording and the original deployment gates remain unchanged.

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
