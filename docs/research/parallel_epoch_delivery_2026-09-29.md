# Parallel Epoch delivery — implementation and validation ledger

Status: **implementation and final-binary lifecycle gates passed; combined
four-loop performance gate not met**. A bounded merge-work diagnosis is in
progress. This is not a production-launch recommendation. Follow
`SHORTENED_PLAN.md`; only the user operates LC2 or starts a new production run.
The latest requested launch is alongside LC2 in session `rustred`, tab
`codex_astra`, using disjoint resources, not a replacement of the current run.

September29 follow-up: the separately frozen `89d90a3a` verifier-lookup build
has completed, but its first matched combined pair is still negative:
Ready9.496s native +12.160875s cold versus Epoch11.183s +16.157556s cold.
Both pass full cold reinspection; 21.657s versus27.341s is **not** a deployment
win. Repeats and mechanistic input/pivot variants are in progress. Source
changes adding the requested hourly discovery-minus-closure monitor have passed
Python regression tests and native type checks, but require a subsequent
optimized build and live checks; they are not present in that89 binary.

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
  Both sides use the same future optimized binary and the same query bytes.
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

Current evidence and exact commands live in `CODEX_PROGRESS.md`. This report
will receive actual native results, optimized executable identity, matched
measurements and tested fresh-launch instructions before final delivery.

### Matched measurements and current blocker

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
not support further flight-width tuning. The next narrowly scoped diagnostic
tests publication-cut grouping without a rebuild; no source remedy or gain
has yet been established.

The final optimized executable's lifecycle evidence remains valid despite
this negative performance result. Launch instructions remain explicitly
unqualified until the requested combined four-loop gate passes. Individual
BMW/H/X and finite/hot five-loop timed arms have not yet run; mechanical W50
tests are not a substitute for a measured W50 campaign.
