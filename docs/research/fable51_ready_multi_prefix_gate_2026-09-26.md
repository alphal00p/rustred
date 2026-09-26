# Ready multi-prefix resume-to-exhaustion gate (fable_5_1 wave 2, B §1(a))

Status: implemented and measured on branch `fable_5_1-b2` (from `fable_5_1`
at 754200a3). Design: `fable51_design_scheduler_admission_2026-09-26.md`
§1(a). Every number below is a single measured run on a shared host (another
user's job floats over all cores); timings are informational only. Nothing
here is a closure, termination or ETA claim.

## What the gate needs and what was built

The open question (plan B3, handoff §6) was whether a real multi-inspector
Ready walk, checkpointed while at least two sources hold positive unfinished
accepted prefixes and a later record is already published (a hole beyond the
contiguous watermark), resumes in a fresh process to exhaustion. Earlier
fixtures either drained before such a state (the reordered-X attempt of
`TMP/ready-x-resume-control.nVaRU6`, accepted-prefix maximum 1 in sampled
heartbeats) or paused without draining (`TMP/ready-five-loop-prefix-v3.k1nLpy`).

1. **Trigger** (`walking/mod.rs`, `execution/streams.rs`):
   `RUSTRED_WALK_DIAGNOSTIC_PAUSE=ready-multi-prefix`, read once per walk. The
   `maybe_save` closure of the checkpointed walk checks
   `State::ready_multi_prefix_hole()` (`ready_accepted_source_prefixes >= 2`
   and `published_count() > queue.next`) at every checkpoint opportunity, i.e.
   after every chunk commit, Finished publication and delegated publication
   (not only at ~1 s heartbeats), force-saves, journals one
   `diagnostic_pause` milestone with the Ready counts and cancels; the process
   exits 4 like a stop request. Saves of that session carry
   `metadata.diagnostic_pause` in the CP5 manifest (free-form metadata: schema
   5 and every older manifest are unchanged; a resumed session saves
   unlabelled). Unknown values, Ordered walks, walks without a checkpoint and
   `--resume` runs are refused with an input error: the trigger is a state
   predicate that a paused checkpoint still satisfies once restored, so a
   resume with the variable set would pause again at its first checkpoint
   opportunity. An environment variable keeps frozen campaign argv
   untouched; both campaign supervisors (`shared_owner_campaign.py` and
   `rustred campaign shards`) remove it from their native children's
   environment, so only a harness or operator running the executable
   directly can set it.
2. **In-process exact test** (`execution/ready_native_multi_tests.rs`),
   described below.
3. **Fresh-process harness** `examples/python/ready_resume_control.py`:
   baseline, paused run (must exit 4 with the label and the counts in the
   receipt, the manifest and the journaled trigger), `--resume` in a new
   process; PASS needs `audit_owner_domain_walk.py` clean on both exhausted
   runs and native inspections and committed events within 2% of the
   baseline (Ready admission is readiness-dependent, so exact equality is not
   the criterion there).
4. **Audit fix needed by the gate**: a resumed walk reports the in-flight
   inspections its paused session returned but never published in
   `uncommitted_inspections` (`committed: false`,
   `resume_reinspects_unfinished_part: true`), and the pool's
   `returned_inspections` accumulates across sessions. The audit refused every
   resumed walk on those two checks; it now accepts exactly such carried
   entries on a `--resume` run and requires each carried domain to be
   published natively. A run that never resumed keeps
   `returned_inspections == native`; a resumed run needs
   `returned_inspections >= native + carried`, because a result finished but
   unpolled (or in Ready escrow) at a periodic save is counted as returned
   there and returned again after a crash and resume; the surplus is
   reported as `resumed_unpublished_returned_inspections`.

## In-process test (exact equality under a fixed schedule)

`ready_multi_inspector_multi_prefix_disk_resume_matches_gated_baseline`:
W=4 Ready (3 inspectors, 0 helpers, 1 coordinator), H=3, five point seeds of
the one-loop native fixture. A scripted wrapper around the unchanged native
visitor delays only *when* each ticket emits: ticket 0 flushes its whole
stream as one accepted prefix and stays unfinished, ticket 2 then does the
same, ticket 1 finishes (the hole); every later ticket emits only when all
lower IDs are published. The production pause branch
(`walking/mod.rs::diagnostic_checkpoint`, the function the CLI walk calls at
every checkpoint opportunity) runs against a real on-disk CP5 store: it
fires once, labels and force-saves the triggering state, journals one
`diagnostic_pause` event and cancels; the test then makes the walk's own
forced save after cancellation (the generation a production resume
restores, still labelled and `paused`). Both generations, the triggering
state (copied to a second store) and the post-cancellation state, are
restored in a fresh `State` with two parked prefixes and the published hole
and resumed to exhaustion. Domains, records without timing, `(events,
successors, conditional, completed, deduplicated)` and the finalized ledger
equal the uninterrupted gated baseline exactly for both. The threshold
itself (at least two prefixes and a published record beyond the watermark)
and the refusals (Ordered, no checkpoint, `--resume`) have their own
license-free unit tests. Licensed run output (release suite, CPUs 200-211):

    ready_multi_prefix_gate domains=6 events=2097168 completed=6 paused_published=1 paused_watermark=0
    test ...ready_native_multi_tests::ready_multi_inspector_multi_prefix_disk_resume_matches_gated_baseline ... ok

(2 × 1,048,576 of the events are the flush markers; the native fixture's own
content is small by construction. Heavy native content is covered by the
fresh-process runs below.)

## Fresh-process gate, binary 63e57c8a

Binary `TMP/fable51-controls/bin/rustred-63e57c8a`, sha256
`63e57c8ad1ed4bd09c8b51795c45a2e276737e32daec67b148146713c819837f`
(`fable_5_1-b2` non-test sources; CP5 records its blake3 `24e8d11d...`).

| Control | Run directory | Trigger state (committed / watermark / prefixes / holes) | Baseline native / events | Resumed native / events | Audits | Verdict |
|---|---|---|---:|---:|---|---|
| four-loop X, Ready W6, CPUs 206-211 | `TMP/fable51-controls/b2-ready-gate/x-w6-r2` | 2 / 0 / 2 / 2 (6,567 events, gen 4, 120,929 B) | 46,826 / 4,495,156 | 46,826 / 4,495,156 | PASS / PASS | PASS |
| four-loop X, first attempt | `.../b2-ready-gate/x-w6` | 65 / 62 / 2 / 3 | 46,826 / 4,495,156 | 46,827 / 4,495,166 | PASS / FAIL, then PASS after the audit fix | superseded by `x-w6-r2` |
| five-loop finite control (1 query, 1,324 tuples), Ready W12, CPUs 200-211 | `.../b2-ready-gate/five-finite-w12` | 1 / 0 / 17 / 1 (17 stream contexts, 20,296 events, gen 4, 563,584 B) | 981,279 / 32,400,559 | 981,286 / 32,466,509 | PASS / PASS | PASS |

X whole-command seconds (`x-w6-r2`): baseline 82.3, paused 5.7, resumed 67.2
(the paused run: 5.2 s preparation, 0.12 s traversal); logical records 47,187
vs 47,186, aliases 361 vs 360 (readiness-dependent transfers). Five-loop
whole-command seconds: baseline 311.8 (traversal 207.2), paused 98.9 (95.3 s
owner preparation, 0.38 s traversal), resumed 311.5 (traversal 206.9); native
+7 (+0.0007%), events +65,950 (+0.20%), logical records 1,276,297 vs
1,278,393, aliases 295,018 vs 297,107; the restore of generation 4 took
0.013 s. For orientation only (different binary, W, CPUs and load): the
frozen-binary Ready W50 baseline of the same control recorded 339.4 s whole,
236.2 s traversal and 981,183 inspections (`TMP/fable51-controls/RESULTS.md`).

### What the fresh-process runs did and did not exercise

The trigger fires at its first opportunity, so both PASS runs paused early.
From each paused receipt (`paused/result.json`, `delegation` object) and the
resumed run's `checkpoint_restored` event:

| Run | Watermark | Committed | Prefixes / contexts / holes | Delegated publications | Transferred obligations | Dispatch fence | Restored domains / edges |
|---|---:|---:|---|---:|---:|---:|---|
| `x-w6-r2` | 0 | 2 | 2 / 2 / 2 | 0 | 0 | 258 | 656 / 4 |
| `five-finite-w12` | 0 | 1 | 17 / 17 / 1 | 0 | 622 | 257 | 5,839 / 5,864 |
| `x-w6` (superseded) | 62 | 65 | 2 / 2 / 3 | 0 | 0 | 321 | 656 / 230 |

So the five-loop run restored 622 transferred obligations and a dispatch
fence far above the watermark together with 17 parked prefixes, and `x-w6`
restored parked prefixes above a nonzero watermark. No fresh-process run
restored delegated (alias) publications together with parked prefixes above
a nonzero watermark, the regime of a long-running Ready campaign; that
combination is covered only by the ledger codec unit tests (published
aliases with holes, without parked streams) and by the cross-version FG
resume below (39 delegated publications at watermark 60,147, no parked
prefixes). It remains unverified as one fresh-process state.

All resumes here used the raw CLI `--resume` in a new process, as
`resume_control.py` does, not the production launcher's `--resume`
(`shared_owner_campaign.py`) that design §1(a) names: that launcher removes
the pause variable, so it cannot produce the paused checkpoint. The native
restore path is the same; the launcher's own resume and executable-upgrade
logic is covered by its Python tests, not by this gate.

## Compatibility controls for the same binary

Reference `rustred-102adcc3` (sha256 `102adcc345ff3010...`, the binary of the
running v2 campaign) vs `rustred-63e57c8a`, both on CPUs 206-211, W6,
`TMP/fable51-controls/run_control.py`:

| Family | Policy | Ref whole / traversal (s) | New whole / traversal (s) | Native inspections | Containment checks | Record comparison |
|---|---|---:|---:|---:|---:|---|
| FG | Ordered | 29.2 / 23.7 | 27.2 / 21.9 | 98,869 | 169,509,549 | strict PASS |
| BMW | Ordered | 50.3 / 43.6 | 47.7 / 41.3 | 147,233 | 653,022,941 | strict: 0 differing records; top level only `descendant_closure.refresh_count` 10 vs 9 |
| H | Ordered | 19.0 / 13.5 | 19.5 / 14.2 | 24,680 | 15,228,826 | strict PASS |
| X | Ordered | 43.0 / 35.5 | 44.5 / 37.2 | 46,826 | 20,507,017 | strict: 0 differing records; top level only `descendant_closure.refresh_count` 8 vs 9 |
| FG | Ready | 18.0 / 13.5 | 19.9 / 15.0 | 98,841 | 167.4M / 167.7M | multiset PASS (equal multisets); both audits PASS |

`refresh_count` counts time-throttled closure refreshes (session telemetry).
Directories: `b2-ref102/`, `b2-new/`, `b2-ref102-ready/`, `b2-new-ready/`.

Cross-version resume (`TMP/fable51-controls/resume_control.py`, FG, stop at
40,000 committed): 102adcc3 -> 63e57c8a Ordered (`b2-resume-ord`, stopped at
53,301): exits 4 / 0, `checkpoint_executable_changed` journaled, strict 0
differing records; 102adcc3 -> 63e57c8a Ready (`b2-resume-ready`, stopped at
60,147): exits 4 / 0, multiset PASS; 63e57c8a -> 63e57c8a Ordered
(`b2-resume-self`, stopped at 52,324): exits 4 / 0, strict 0 differing
records. The strict top-level differences are the ones the 102adcc3 ->
102adcc3 self-test (`harness-selftest-102`) also shows:
`uncommitted_inspections` (carried attempts) and `descendant_closure`
`refresh_count` / `retained_storage_estimate_bytes`. The resume-aware audit
passes on all three resumed runs.

## Reading

- The multi-prefix hole state is reachable and resumable with real native
  streams at four and five loops. The trigger is evaluated at every
  checkpoint opportunity rather than on ~1 s heartbeat samples; on X it fired
  0.12 s into the traversal (two initial sources with flushed, unfinished
  prefixes, two later initial sources finished), on the five-loop control
  0.38 s into the traversal (17 unfinished accepted prefixes, one published
  hole). The earlier reordered-X attempt (frozen c6ac4664, sampled
  heartbeats) never observed two prefixes.
- The durable label is read by the harness before the resume: the resumed
  process continues the same CP5 store, so its later generations (and
  `latest.json`) are unlabelled and unpaused by design.
- Resumed Ready runs match their uninterrupted baselines within readiness
  noise (X: identical native and event counts in `x-w6-r2`, +1 native and +10
  events in `x-w6`; five-loop: +7 native, +0.20% events), well inside the 2%
  criterion; both runs of every pair pass the streaming audit.
- Scope: local completion of recorded obligations only; no IBP replay, no
  family closure (`family_closure_claim` stays false).

## Reproduce

    cd /common/dev/rustred/.claude/worktrees/agent-ab06981cd80007c75   # or the merged tree
    nice -n 5 taskset -c 206-211 nix develop --command python examples/python/ready_resume_control.py \
      --command /common/dev/rustred/TMP/four-loop-saved-descendants.VaNmUN/x/command-rank12orthant.json \
      --binary /common/dev/rustred/TMP/fable51-controls/bin/rustred-63e57c8a \
      --output /common/dev/rustred/TMP/fable51-controls/b2-ready-gate/<new-dir> --cpus 206-211
    # five-loop finite control: --command .../ready-five-loop-finite-w50.a6ABXd/ready-first/command.json
    #   --workers 12 --cpus 200-211 --replace \
    #   /common/dev/rustred/campaigns/five-loop-saved-coarse-cover/inputs=/common/dev/rustred/TMP/retired-campaigns-20260925.UtI4ay/five-loop-saved-coarse-cover/inputs

## Scheduler review follow-ups in the same branch

Checked against the merged code first. Already applied at merge: #2 (duty
buckets disjoint, one duty object at `progress.parallel.coordinator_duty`),
#3 (service step defers unpublished Delegates, with a test and counter), #6
(three snapshot tiers; slot arrays only on drain/final), #9 (Ready service
counts nested), #10 (`MAX_WALK_WORKERS` = 256 shared by CLI, walk and shard
supervisor), #11 (plan B3 text). Applied here: #8 (`prepared_retirements_trivial`
for empty sets from a bucket absent at the snapshot; telemetry only), #12
(license-free synchronous `reclaim_all_finished` test, including the byte cap;
visible skip markers on the Ready tests' license gates), #13 (per-domain event
key sets frozen against the pre-wave binary's journal; heartbeat deltas pinned).

Monitoring: the live v2 campaign's `status.json` showed
`coordinator_duty_breakdown_1h` and `computing_workers` as null because a
heartbeat journals the latest event, which at campaign rates is usually a lean
per-domain event without those keys (the key path itself,
`progress.parallel.coordinator_duty`, was right). `heartbeat_metrics.py` now
uses the latest (and for the breakdown the earliest) sample in the window that
carries them; tests use four real heartbeat lines of that campaign.

Suite note (release lib suite with the license, CPUs 200-211, host load
~95-130): five full runs. Before the Ready test fix below: 734/1/4 (shard
lock test), 733/2/4 (shard lock test and the Ready late-fault test),
735/0/4; after it (bed99c97): 735/0/4, then 734/1/4 with a wall-clock
presenter test (`cli::progress::tests::presenter_ticks_without_semantic_callbacks_and_stays_below_configured_rate`).
Each of these tests passed 30 of 30 isolated runs. The Ready one
(`ready_tests::ready_late_native_fault_after_cancellation_disallows_pause`)
cancelled before its faulting inspector had taken its job (`Pool::take` then
returns nothing and the walk pauses cleanly); it now cancels only once that
inspector runs. The shard-lock test
(`cli::shards::supervisor::tests::orphan_child_retains_campaign_lock_until_exit`)
and the presenter test are timing-dependent tests outside this change, left
to the shard-CLI follow-up (suspected, not verified, for the lock test: a
concurrently forked sibling briefly holds a copy of the lock descriptor).
