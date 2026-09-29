# Parallel Epoch delivery — implementation and validation ledger

Status: **implementation integrated; native execution and performance gates
pending**. This is not yet a production-switch recommendation. Follow
`SHORTENED_PLAN.md`; only the user operates LC2 or starts a new production run.

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
helpers-first query order. No owner artifacts were regenerated for this change.

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

## Validation and measurement status

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
  `f3f707af`; the complete corrected application suite is being rebuilt and
  rerun. The failed receipt is retained, not counted as a green milestone.
- Optimized Stage B binary: not yet frozen. Never use the app-opt1 correctness
  or app-opt0 correctness executable for performance comparisons.
- Matched controls: prepared, independently checked, not executed. Required
  scopes are FG, BMW, H, X, combined four-loop, finite five-loop and hot-sector.
  Both sides use the same future optimized binary and the same query bytes.
- Adaptive comparisons: pending; no benefit claimed.
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

Current evidence and exact commands live in `CODEX_PROGRESS.md`. This report
will receive actual native results, optimized executable identity, matched
measurements and tested fresh-launch instructions before final delivery.
