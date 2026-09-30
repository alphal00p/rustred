# Parallel Epoch delivery — implementation and validation ledger

September30 final follow-up: this ledger retains the earlier build-specific
measurements. See [the consolidated decision and launch guide](consolidated_campaign_launch_2026-09-30.md)
for the newer1b33 results, owner-operated Ready fallback and200-core caveats.
The original S5 parallel-merge/typed-record roadmap is **still partial**; a
working rolling engine and its negative performance tests do not establish
that the full intended architecture was implemented and tested to failure.

Status: **compiled experimental candidate; lifecycle, all four-loop and both
representative five-loop cold checks pass, including W50 four-loop correctness.
The faster-replacement performance gate is not met.**
The reindexed four-loop input library gives mixed-sign near-parity between
schedulers in two pairs. The original combined control and the finite/hot
five-loop controls favor Ready. Therefore this is not a faster-production
recommendation: alongside use is explicitly experimental. Follow
`SHORTENED_PLAN.md`; only the user operates LC2 or starts a new production run.
The latest requested launch is alongside LC2 in session `rustred`, tab
`codex_astra`, using disjoint resources, not a replacement of the current run.

September29 follow-up: the separately frozen `89d90a3a` verifier-lookup build
passes both combined matched pairs and all four query-order pairs in correctness,
but not the performance gate. The baseline two-run means are Ready23.2965s
versus Epoch25.8450s, including independent cold reinspection: Epoch is10.94%
slower. Native timings are worse in both pairs. No mechanistic query-order
candidate produced a decisive gain. A subsequent banana-owner stitched pilot
keeps all58 required queries,508 routing witnesses and15 other owner payloads
unchanged. Fresh-natural Ready/Epoch both cold-All pass in24.839/24.347s (one
pair, not a speedup gate). Mixed-line-first blows up in both engines: Epoch is
censored at129.177s/1,445,109 domains and Ready at60.672s/215,330 domains. Neither
mixed arm closes. Aggregate terminal counts are not terminal-key identity proofs;
this exploratory comparison makes no such claim. See
[the detailed ordering study](mechanistic_ordering_2026-09-29.md).

The final monitored3428 build confirms that negative result on the original
matched combined control (whole native command plus independent cold-All; Python
summary audit timed separately):

| Arm | Native seconds | Cold-All seconds | Sum seconds |
| --- | ---: | ---: | ---: |
| Ready A1 | 9.977 | 15.157 | 25.134 |
| Epoch FIFO P1 | 11.987 | 16.165 | 28.152 |
| Epoch FIFO P2 | 11.383 | 15.155 | 26.538 |
| Ready A2 | 8.782 | 15.173 | 23.955 |

All four independently verify all58 required queries (32 admitted roots) with
zero uncovered obligations. Epoch's mean27.345s versus Ready24.545s is11.41%
slower; both pairs are unfavorable. The first Epoch arm's only10s contention
sample estimated3.26 foreign busy CPUs on the16 assigned CPUs, while the first
Ready arm ended before a sample. Do not infer a contention-adjusted speedup.
Raw receipts: `TMP/codex-final-monitor-matrix.LJLGfi/runs/`. Further input/runtime
diagnostics remain exploratory, not grounds for a launch recommendation yet.

For the reindexed **input library**, two matched pairs complete with cold-All
PASS (not a performance-gate pass): the
mechanistic pinch-interface coordinate reindex, with both parents regenerated
and all58 query objects exactly inverse-equivalent, gives Ready17.730s versus
Epoch16.839s including cold-All in the first pair, then Ready16.144s versus
Epoch16.619s in the reversed pair. Both verify all32 admitted roots; generated
rules and routed fingerprints are consistent. The means16.937s/16.729s differ
by1.23%, with mixed signs, not a robust speed win. This changes sector priorities
and representation together, not just scheduler code. No1.5x architectural
speedup is established. See the
[ordering study](mechanistic_ordering_2026-09-29.md) for counts and exclusions.

A final same-library W50/CPUs32–81 correctness check passes all58 required
rows,32 admitted roots,19,597 native reinspections and36,161 graph domains.
Native7.985s+cold8.155s and393.6MB peak RSS are recorded, but there is no matched
W50 Ready arm and no W50 five-loop timing. The reverse-pair plus W50 pilot
finishes within261.147s of its900s inclusive limit; all nine owned groups drain.
Evidence: `TMP/codex-sector-reindex.D5Yawx/reverse-replication/results.json`.

The individual controls also completed on that final executable, W6 on
CPUs32–37. Every Ready and Epoch arm passed independent full cold-All; these
are one pair per input, not replicated performance claims:

| Control | Verified queries | Ready native + cold seconds | Epoch native + cold seconds |
| --- | ---: | ---: | ---: |
| FG | 248 | 16.020 +11.158 =27.178 | 16.004 +8.157 =24.161 |
| BMW | 268 | 26.221 +18.164 =44.385 | 27.224 +13.179 =40.403 |
| H | 628 | 16.804 +11.162 =27.966 | 16.261 +10.154 =26.415 |
| X | 656 | 32.824 +23.176 =56.000 | 48.649 +22.164 =70.813 |

FG/BMW/H improve by about5–11% in these single pairs, but X is26.45% slower.
This is workload-dependent behavior, not uniform scaling or a qualified
combined-control win. Full receipts, including CPU/RSS, host contention and
separately timed summary audits, are in `final-individual-results.json` beside
the repeated combined results. All24 owned groups drained afterward.

The already exposed `all-miss` lookup control also completes the same scope:
10.984s native plus16.159s cold=27.143s, with the same51,139 domains as Snapshot.
That is only0.74% below Snapshot's mean and10.58% above Ready's mean; it is not
a useful improvement and is not being repeated. Its lower boundary cost
(0.063s versus Snapshot P1's0.332s) is offset by greater merge cost
(2.130s versus1.665s). These phase measurements are diagnostic, not additive
substitutes for whole-command timing. Evidence: `all-miss-results.json` beside
`final-abba-results.json` under the final-monitor matrix directory.

### Representative finite five-loop control

The common primary boundary now completes for the original finite control,
with the full67-owner library/8,179 routes but only **one finite required
query**, W16 on CPUs32–47:

| Scheduler | Whole native command | Successful cold-All | Sum | Domains | Native inspections |
| --- | ---: | ---: | ---: | ---: | ---: |
| Ready |176.082s |156.211s |332.293s |966,681 |759,802 |
| Epoch FIFO |278.050s |139.214s |417.264s |916,438 |745,954 |

Both cold-All reports close1/1 query/root with no uncovered/errors/violations.
Epoch CP6 acceptance passes; its native exit4/summary remains explicitly
incomplete until independent cold verification. Epoch is25.57% slower on this
matched boundary despite doing slightly less domain work and using less native
CPU (491.115s versus571.051s). This is negative performance evidence, not an
all-five-loop scope result or launch qualification.

Ready first exhausted the original150s cold allowance (151.230s, exit124),
then passed the separately recorded300s retry. Its extra Python full-result/
event audit was censored at301.255s; no successful full-pipeline audit is
claimed. These452.485s of failed-attempt overhead are disclosed separately,
not silently charged only to Ready in the successful-primary comparison.
Epoch's inexpensive summary audit is explicitly INCOMPLETE and supports only
its existing CP6 acceptance contract, not the missing event proof. Raw
receipts and amendment records are under
`TMP/codex-final-monitor-matrix.LJLGfi/`.

The hot five-loop control also completes, W12/CPUs32–43, again one required
query rather than the full production request:

| Scheduler | Whole native command | Cold-All | Sum | Domains | Native inspections |
| --- | ---: | ---: | ---: | ---: | ---: |
| Ready |195.124s |178.217s |373.341s |1,026,779 |798,399 |
| Epoch FIFO |300.662s |149.212s |449.874s |905,522 |731,117 |

All native inspections and1/1 root pass full cold reinspection; Epoch CP6
acceptance also passes. Full Python event audits are explicitly NOT RUN (the
Epoch summary check remains INCOMPLETE). Epoch uses fewer domains/inspections
and less native CPU (496.881s versus580.315s), but20.50% more end-to-end wall.
All12 process groups from the two five-loop comparisons drain independently.
The consolidated raw receipts are `final-five-loop-results.json` in that
directory. These completed measurements do not establish favorable five-loop
scaling. The publication cut is fixed at16; more workers enlarge the window,
not the cut. No W50 five-loop performance result is available.

The requested hourly discovery-minus-closure monitor passes the complete Python
suite (323 passed, one optional skip), native test metadata and the actual
focused Tracker tests (12 passed, one ignored scale benchmark). The separately
identified optimized `3428b519` build finished successfully in3786.523s, including
native live closure refresh. Its frozen executable is
`TMP/codex-parallel-campaign.oiPK29/candidate-bin/rustred-3428b519`, SHA256
`321b02b166c61dae927a220b7b8007b4659fef009d2b5b003084830b0f43eca3`.
Actual CLI pause/resume/live-refresh and lockstep controls passed on this
binary; the individual four-loop and finite/hot five-loop controls above also
pass. The first M2
resumed leg finished before its5s heartbeat and remains an incomplete live
observation. One additional same-FG248/W6 run with two inspectors observed
genuine live refresh both before pause and after resume and passed independent
cold-All248. M4 also passed cold-All248 with6,177 committed cuts. These are
lifecycle/correctness receipts, not a speedup. None of that telemetry work is
present in the frozen89 comparisons.

The Python dashboard/stream milestone `4a1371a5` is pushed separately. A real
FG248 pilot then exercised five PTY monitors at80/100/140 columns, live resize
and NO_COLOR; root and the author visually inspected captured terminal states.
Narrow fixes improve contrast, prevent clipped rate labels, preserve checkpoint
directory notices, and scale short-run plot axes. The corrected full Python suite
is344 PASS/one existing optional skip. Real cold-All248 passes; the encompassing
visual-pilot M4 receipt remains INCOMPLETE only because a positive computing-worker
sample was not captured. It is not a new performance measurement or an all-gates
pass. Evidence: `TMP/codex-dashboard-live.09MDmS/VISUAL_AUDIT.md`. Rendered images
are captured-state exports/replays, not OS screenshots, and pyte does not
reproduce faint intensity. Normalized telemetry, coloured table and SVG consumers
do not change native decisions or the existing pending-growth calculation.

The latest frozen Python revision is `adcee12c`: requested CPU/pending colour
thresholds, normalized paired-window closure balance and dual-axis
unresolved-total/raw-net-rate plot. Its full suite is350 PASS/one existing
optional skip, with58 focused tests and root's independent27-test rerun passing.
Author and root inspect saved actual-FG replays and labelled synthetic boundary
plots; evidence is `TMP/codex-dashboard-balance.VBpyjw/self-review.json`. Earlier
323/344 counts above describe preceding revisions, not this latest freeze.

## Delivered separately: compatible Stage A

The stable `fable_5_1` milestone is `931d006c`, built from native source
`986d046e`. Its independently reviewed pause/upgrade/resume instructions and
limitations are in [the stable delivery report](fable51_stable_upgrade_2026-09-29.md).
LC2 keeps its progress, inputs and frozen options, including G2 Off. It does
not acquire the new Epoch architecture merely by upgrading the executable.

## Parallel architecture

The implementation on `fable_5_1_parallel` uses a bounded rolling window instead
of waiting for every job in one global batch before publishing any more work.
Workers inspect immutable inputs; the coordinator publishes completed prefixes
in recorded sequence order. The next inspections can overlap that publication.
A slow job at the front of a publication cut can still limit progress: this
architecture does not promise unrestricted work stealing or linear scaling.

Lookup data is shared through at most two immutable lookup replicas, not one
copy of the full campaign or Symbolica expressions per worker. Incremental
updates refresh an idle replica. When both are leased, bounded backpressure
applies. An oversized valid update drains leases before refreshing the idle
replicas; it is not discarded or mistaken for mathematical failure.

At publication, the coordinator validates lookup proposals against current
authority. A miss in an old snapshot is not proof that a newer result is absent.
G2 Union keeps exact residual coverage, explicit dependencies and inspected-scope
restrictions. Rescue quarantines unusable future lookup authority without
erasing historical records or edges. A geometrically equal new representative
may be admitted after quarantine; original required queries cannot be relabelled
or removed by an amendment.

The frozen production scope remains 116 required physical/convenience queries
and 67 auxiliary helpers. Query-role declarations are explicit and immutable.
Undeclared queries remain required. A closed amended required scope is not the
same claim as every historical auxiliary domain being closed.

### Optional adaptive dispatch

`--epoch-dispatch adaptive` adapts the order of pending jobs, not the algebraic
pivots in the saved IBP programs. It scans a bounded candidate window, updates
small cost/work-growth/reuse statistics only from successful publications and
periodically selects the oldest candidate to prevent starvation. Choices and
heuristic state are preserved for replay. The heuristic never authorizes an
IBP, removes an obligation or creates a terminal.

FIFO remains the comparison baseline. The separate
[algebraic-ordering review](parallel_delivery_ordering_review_2026-09-29.md)
found no qualified replacement for the current natural saved programs and
helpers-first query order. The selected campaign library remains unchanged;
separate experimental banana owner generations are not deployed replacements.

## Interfaces and checkpoint boundary

The Rust request, CLI and Python steering expose the same choices:

- `--publication-policy epoch` with a durable checkpoint;
- `--epoch-rolling` for rolling execution;
- `--epoch-inspector-lookup snapshot` for inspector-side lookup;
- `--g2-residual-anchors union` for residual reuse;
- `--epoch-dispatch fifo|adaptive`, with adaptive opt-in;
- explicit automatic rescue only with a complete query-role declaration.

New Epoch checkpoints use `RUSTRED-WALK-CP6`, manifest schema 2, scalar schema 3,
walk semantics 3. CP5 remains unchanged. There is no CP5-to-CP6 migration project;
Stage B production is a fresh campaign, and LC2 is retained independently.

Checkpoint-only summaries deliberately do not claim full closure when the queue
drains. Raw cold reinspection reads the complete saved record/dependency state.
Unamended all-domain controls must independently discharge all roots and domains;
rescue controls additionally distinguish required-query closure from abandoned
auxiliary obligations. A saved receipt must reconcile with the final event before
Python can automatically resume with a rescue amendment.

## Historical integration receipts — f083f254 through e1bdb9e7

This section preserves the **earlier** integration/testing chronology and
failed receipts. Pending controls and conditional launch statements below were
their status at that time, not current blockers. The final3428 measurements,
W50 check and current experimental-only interpretation are recorded above.

- Source: combined rolling/rescue integration `f083f254` passes release compiler
  and test-type checks in 46.183 s. This is not executed native-test evidence.
- Python: full discovery passes 310 tests with one optional slow skeleton test
  skipped. This includes 103 lifecycle, 12 CP6 measurement-contract and 37 final
  monitoring/metrics checks. The first full run exposed a stale Epoch lookahead
  fixture; the corrected complete run passed.
- Source audits: rolling/snapshot and rescue boundary/cancellation were reviewed
  independently; root separately reviewed adaptive dispatch and interface changes.
- Native tests on `f083f254`: core 2,845 passed/zero failed/32 ignored;
  app 1,103 passed/10 failed/12 ignored. Both actual W50 mechanics tests passed.
  One app failure exposed an obsolete public lockstep-G2 rejection; nine were
  stale checkpoint/test fixtures. Audited corrections are integrated at
  `f3f707af`; the complete corrected application suite was then rebuilt and
  rerun, as recorded below. The failed receipt is retained.
- Corrected full app execution on `f3f707af`:1,116 passed,one failed,12 ignored,
  with no unexpected skips. The remaining rescue test exposed a cold-reader
  inventory check that counted amended roots as if they belonged to the original
  protected prefix. Audited narrow correction `d40c1b76` preserves missing-prefix
  detection and all per-query checks. Final optimized CLI validation on the
  preserved failing checkpoint now passes: required-scope cold PASS1/1,
  all8 applicable natives reinspected, abandoned-helper AllRoots rejection,
  remapped-query rejection, unchanged cold-read bytes, actual resume with
  record/edge digest parity and altered-amendment refusal. This is targeted
  final-code execution, not a final full-suite rerun.
- Optimized Stage B binary: frozen from clean `e1bdb9e7`, campaign opt3/fat-LTO,
  one codegen unit; build3,990.666s, excluded from solver timings. Executable:
  `TMP/codex-parallel-campaign.oiPK29/candidate-bin/rustred-e1bdb9e7`, SHA256
  `b5bd346cd9bfa925a4324031660cb3b2993e23c11f1e53765cb6758b87d9d95c`.
  Never use the app-opt1/app-opt0 correctness executables for comparisons.
- Final optimized-CLI lifecycle: M1–M4 all PASS, with no missing required
  observations. These cover W1 unknown activity, W6 genuine pause/resume and
  cold-All248/248, an explicit2/3/1 worker partition, and W6 multi-cut lockstep
  with cold-All248/248. Cold reads do not mutate checkpoints; workers drain.
  Shared formatter behavior is exercised through real non-TTY events; this
  is not a claim that an interactive TTY was tested end to end.
- Matched controls: prepared and independently checked; FG and repeated combined
  four-loop comparisons have completed, as detailed below. Required
  scopes are FG, BMW, H, X, combined four-loop, finite five-loop and hot-sector.
  Both sides of each completed pair use the same frozen optimized binary and
  the same query bytes. The newly monitored binary requires its own controls.
- Adaptive comparisons: one complete combined four-loop pair favors FIFO;
  no adaptive benefit claimed. Finite/hot five-loop comparisons remain pending.
- Combined four-family physics-capped performance is an explicit acceptance
  gate: repeated matched verified-closure runs must be faster or on par with
  current optimized Ready+Union. Inconclusive measurements are not a pass.
- W50: planned separately from mechanical thread/lifecycle tests, conditional
  on successful lower-width native and cold controls.

Each pilot includes preparation and orderly shutdown within 30 minutes. Native
whole-command time, independent cold time, CPU time, memory and domain work are
reported separately. The comparison also charges common cold-All verification
to both variants. Ready's full result and Epoch's checkpoint-only summary are
different representations. The agreed common deliverable is durable, resumable
records plus independent cold-All verification of the same scope; native plus
cold time can qualify as an end-to-end verified-closure speedup. Avoiding large
full-result materialization is disclosed as part of that gain, not misattributed
to inspection. Native drain speed alone cannot establish the 1.5x gate, and no
drop-in compatibility with consumers requiring Ready's full JSON is assumed.
Failed, censored and unexecuted cells remain
explicit. No eventual five-loop completion time follows from zero frontiers or
increased worker activity.

Current evidence and exact commands live in `CODEX_PROGRESS.md`; the current
results and frozen executable identities are in the opening sections above.

### Historical matched measurements and then-current blocker

The same executable, query bytes, owner programs, CPU placement and worker
budget are used on both sides. Times below include native command plus the
same-scope independent cold reinspection, but not compilation.

| Control | Ready + Union | Rolling FIFO + Union | Interpretation |
| --- | ---: | ---: | --- |
| FG, W6, first pair | 28.874 s | 26.926 s | One descriptive pair; not a deployment gate |
| Combined four-loop, W16, pair 1 | 26.774 s | 29.721 s | Epoch slower |
| Combined four-loop, W16, pair 2 | 23.454 s | 30.288 s | Epoch slower |

All completed arms independently passed their full cold closure checks. Yet
Epoch's fewer saved domains do not imply less work: the combined FIFO arms
save 51,139 domains and perform 31,724 ordinary native inspections, versus
66,765/66,519 domains and 24,319/22,499 ordinary inspections for Ready. The
extra Route work also raises cold-verification cost. This is a failed parity
gate, not a speedup justified by memory or worker activity.

A preregistered runtime falsifier reduced Epoch's inspectors from 15 to 8
within the same W16 budget. It completed correctly but took 31.514 seconds,
with 31,555 Route inspections versus 31,558 at the default width. This does
not support further flight-width tuning. The subsequent cut-size-one
diagnostic was also negative (31.469s). Neither is a useful measured remedy.
The then-remaining runtime-only falsifier compared `all-miss` lookup against
Snapshot. It subsequently completed on3428 with a negative result, recorded above.

At this historical checkpoint, lifecycle evidence passed but a launch remained
unqualified; individual BMW/H/X and finite/hot timed arms and an actual W50
campaign were still pending. They have since completed as recorded above.
The faster-replacement gate remains unmet; the current handoff is explicitly
experimental, not a retrospective performance pass for these earlier results.
