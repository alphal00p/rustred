# Epoch publication rescue: measured rejection for this build

Decision: retain Ready as the deployment candidate; do not deploy any tested
Epoch setting. This rejects the current build/settings, not every possible
Epoch design. Epoch remains opt-in. No further Epoch run or engine change is
authorized by this result.

## Fixed scope and evidence

Evidence: `TMP/codex-publication-rescue-matrix.2ScWnG/`, especially
`matrix.json`, `bound-1b33ad29/matrix.json`, `results.json`, `cell-0.json` through
`cell-3.json`, and each referenced raw run. Source matrix SHA256
`6a1e3e0e462e0457b9a230cd9e8b9a90fb2189b5660c9aa69ae606871ec24f02`;
bound matrix SHA256
`77238f12fb2dc27278a78f6b7e81068ef6fd155717a759acfac90a6e22c45acc`.
Native source `1b33ad2956d87f29db6ef9987542f3364f443814`, optimized CLI SHA256
`73253922552ef341e3e97522d9481a4e369d388e9f8612c4ac92468c3c584a14`;
Python verifier tooling separately frozen at `62c763cd`.

All cells used the original58 Required/0 Auxiliary query rows,16 original owner
payloads and508 routes, G2Union, W16/CPU32–47; Epoch additionally used FIFO,
Snapshot and cut16. Actual native argv matched the bound matrix, and input
hashes were rechecked after execution. No physical reindex, helper addition,
domain clipping or mathematical policy change was introduced.

Root held the global-heavy reservation; E1 used two local locks while the
independent P1 source-strategy pilot ran on CPU64–79. Phase-dependent host
contention makes this exploratory evidence, not an isolated qualification.
Ready's configured native inspection slots were8, Epoch's15 plus coordinator,
within the same W16 total budget. No200-core throughput/utilization inference
is made.

## Four-cell result

| Cell | Guarded native s | Guarded cold s | Native+cold s | Native child CPU s | Native inspections | Domains | Cold / secondary audit |
|---|---:|---:|---:|---:|---:|---:|---|
| Ready |9.779|12.16049|21.93949|32.026|22,756|66,339|All32roots PASS / full-result PASS|
| Oldest-prefix, window31 |11.389|15.16655|26.55555|21.783|31,846|51,139|All32roots PASS / summary INCOMPLETE|
| Oldest-prefix, window76 |10.753|15.15779|25.91079|21.948|31,895|51,141|All32roots PASS / summary INCOMPLETE|
| Oldest-ready, window31 |121.363|not run|incomplete|332.852|1,033,824|1,496,524|native censored; no cold acceptance|

The completed Epoch cells passed the unchanged CP6 raw-cold gate, including
explicit `--certification-scope all-roots`, reinspection All, all58 query rows,
all32 distinct initial roots, no violations and unchanged checkpoint bytes.
Their secondary summary audits are intentionally INCOMPLETE(exit1), not
full-result PASS. Ready's full secondary audit passed. Audit guard costs,
separate from the primary native+cold boundary, were6.15931/1.15603/1.15749s.

Native time is launcher-inclusive through owned-group drain, excluding lock
admission and recorder shutdown. CPU is the runner's RUSAGE_CHILDREN user+system
delta, not an instantaneous utilization measurement. Cold is guarded wall.
All preparation, admission, drain, audit, failures and reporting were charged
to the original1800s clock; exact accounting is retained in `results.json`.
No clock or450s cell ceiling was extended. All actual children and the stopped
waiting guard independently drained; both E1-local locks were released.

## What the diagnostics establish

Window76 reduced traversal7.66245→6.64608s and blocking prefix-wait4.93787→3.98320s
relative to window31. Snapshot refresh stayed~0.243s and publication-wait was
zero in both. This is consistent with improved overlap, but the complete
native+cold boundary remained18.1% slower than Ready (window31 was21.0% slower).
Epoch still performed~40% more native inspections despite fewer stored domains.
Prefix-wait also includes waiting for enough ready results: it does not by
itself isolate pure head-of-line blocking.

Oldest-ready exercised29,934 nonprefix cuts out of64,614, so the new policy was
actually used. It produced~32.5 times the completed prefix31 native inventory
and still had10,165 pending domains at the cap. Its boundary35.483s,
P2 46.112s, P3 24.990s and snapshot-refresh31.289s grew with this work. Snapshot
refresh overlaps coordinator phases and must not be added to them. Lower
waiting or more occupied workers did not translate into less total work.
The data do not establish the exact causal ownership/representative chain
behind that expansion; no such unmeasured claim is needed for rejection.

A bounded follow-up read only the first128 accepted JSONL records of prefix31
and oldest-ready31 (about94KB each, not the complete walks). Their first cuts
contain16 native initial-root records: prefix31 publishes IDs0–15, whereas
oldest-ready skips3/11/13 and includes16/17/18, the first three broader R12
roots. The skipped owners include banana `0111100001` and BMW `0111111110`.
The13 common initial records have identical domain geometry, native statistics,
accepted-event counts and frontiers, but published edge counts already differ
for IDs1 and2. First-cut accepted events decrease86,512→60,714. The next112
records are aliases; allocated IDs beyond the initial roots are not stable
cross-run domain identities. Thus an early graph-selection difference is
observed, and changed containment representatives are a plausible mechanism,
but this excerpt does not identify the later causal chain behind the expansion.
It supplies no independently justified rescue setting or reason for another run.

The registered cut64/window79 follow-up is not triggered: the fastest accepted
Epoch cell has boundary4.92% and snapshot-refresh3.65% of traversal, both below
the10% threshold. A censored oldest-ready run cannot trigger that rescue probe.
No oldest-ready76 combination, cut grid or repeat was run.

## Censoring, setup mistakes and native-gate limits

Oldest-ready hit the unchanged admission-inclusive outer120s timeout, then
saved/stopped/drained orderly: native exit4, outer exit124, guard121.363s,
`censored=true`, stop reason `operator_signal_2`. Collection correctly refused
acceptance. No CP6 collection/freeze/accept receipt or cold report exists for
that cell, and no independently verified closure claim is made.

Two related orchestration failures are preserved: `freeze-attempt-failure.json`
transcribes the failed premature freeze while native metrics did not yet exist;
no freeze was written. The subsequently started verification guard waited on
the lane lock and was stopped before any child started; its original receipt
is `runs/consolidated-publication-oldest-ready31-r1/four-all/cold-verifier/result.json`
(exit125, `waiting_for_lock`, `child_started=false`). No verifier overlapped
native work, read the checkpoint, or ran a hidden retry. Minor audit-key/path
preparation errors are also recorded in `results.json`; all elapsed time stays
charged to the same clock.

The original combined app suite remains1139PASS/1stale-diagnostic-stringFAIL/
12ignored, not fully green. The one expected-string-only test correction did
not rebuild or execute the corrected Rust unit body. Instead, independently
reviewed frozen-public-CLI equivalent checks passed all10 phases, including
rawcold, summary INCOMPLETE, wrong-G2 binding refusal, activation/nonempty
refusal, W1resume and coldreopen; evidence is
`TMP/codex-epoch-cold-wording.MzOlZL/cli-equivalent/`. No production engine or
optimized binary changed as a result of that wording correction.
