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
| G2′ integration | delivered | joint_support_pruning | Merged `9ef5464d`; all14 controls, full/native/W50/CLI and real pause/resume checks pass |
| Rescue and explicit query roles | delivered | joint_support_pruning (integration; original author bounded_helpers_bmw) | Merged `9ef5464d`; all116 required queries retained, tested combined quarantine/replay invariants |
| Independent math/code audit | active | joint_support_pruning and root (current implementation review), checkpoint_final_audit (earlier) | Keep each implementation separate from its reviewer; independently verify runtime/geometry invariants and measurement interpretation |
| Combined G2′ + rescue | delivered | joint_support_pruning + root + independent auditor | All14 controls independently accepted; native/dependency tree exactly matches frozen tested source |
| G2′ component baseline / consolidated deployment | active | root, joint_support_pruning, active_goal_delivery_audit | All eight Ready arms accepted/drained; repeated 24–28% work reduction establishes component baseline; latest user defers restart until validated remaining ideas are consolidated |
| Python production G2′ steering | delivered | joint_support_pruning (author), checkpoint_final_audit (independent review) | `cd52c90d` merged via `9ef5464d`; real pause/resume/cold and post-merge regressions pass |
| Coordinator latency / telemetry | active | checkpoint_final_audit (author), joint_support_pruning (integration), root (review) | Source-only consolidation of audited `187854b4`; five regressions and performance gates still unexecuted |
| Epoch S3–S6 | active | epoch_native_validation (execution), active_goal_delivery_audit (public lifecycle), joint_support_pruning (lookup/outer integration), root (independent review/integration) | Frozen `f404cf66`: 130 focused tests and full app suite PASS (999 passed, 12 ignored; W50 skips excluded). `29e30a79` passes core/app test typechecking; native code build ongoing. Prefix, S4 miss handling and first S5 edge-feed slice combined privately at `fa783fc5`, source-reviewed only. Public runtime/frontend/cold-verifier composition, G2/rescue and rolling scheduling remain open |
| N2 allocation-free geometry | active | active_goal_delivery_audit (isolated author), root/joint_support_pruning (independent review) | Narrow shifted-image endpoint-buffer reuse only, motivated by qualified post-G2 profile; no measured gain or native execution yet |
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
