# Finite-domain implementation audit — 22 September 2026

**Post-audit gate update:** the precise count/equivalence fixes recommended
below are implemented and independently source-reviewed. The release
application suite now passes 361 tests with one ignored diagnostic, including
the complete worker and orthant off/on comparisons. All 82 integration tests
also pass. Evidence: `TMP/semantic-containment-gate.qn9MTE/app-corrected.log`.
The initially failing gate remains recorded below as audit history.

## Scope and verdict

This independent read-only source audit covers the shared-owner domain walker,
the new cached semantic containment service, native affine routing, and the
separate parallel completed-result escrow work in the working tree. No solver,
build, production edit, or new CAS primitive was introduced by this audit.
Measurements below are independently collected receipts, not timings obtained
by inspecting source. Related reports are the [performance audit](finite_domain_performance_audit_2026-09-22.md)
and [finite-domain plan](../finite_starting_domains.md).

**No confirmed mathematical soundness defect was found in the new containment
service. The integrated application gate is not yet green.** Its two failures
are obsolete baseline-count assertions, explained below; they occur before the
serial/parallel comparisons and therefore do not demonstrate a worker-parity
failure. Those comparisons still have to run successfully after correcting the
expectations. No full five-loop finite envelope has closed.

## 1. Immediate release gate: semantic equality changes two fixture counts

Evidence: `TMP/semantic-containment-gate.qn9MTE/app-test.log` reports 359 passed,
two failed and one ignored. Separate reported gates pass 2,758 core tests with
32 ignored, 27 queue-harness tests with one ignored replay, and 82 integration
tests. This does not substitute for a passing integrated application gate.

The failures are:

- `crates/rustred-app/src/application/routed_campaign/tests/parallel_walk.rs:47`:
  actual `scheduled_nodes=3`, historical expectation 4.
- `crates/rustred-app/src/application/routed_campaign/walking/execution/initial_orthants_tests.rs:231`:
  actual `completed=4`, historical expectation 5.

Both fixtures use the one-dimensional, fully active owner `[true]`. They admit
two fixed domains, local `[2,2]` and `[3,3]`, followed by the whole ray
`[0,infinity)` with numerator cap 11, and then the same ray with no rank cap.
There are **no inactive coordinates**, so physical numerator degree is
identically `R=0`. Thus `R<=11` and an absent rank cap describe exactly the same
set. The former raw-label predicate distinguished them; the new semantic
predicate correctly reuses the first ray. The two narrow jobs remain pending:
retirement removes lookup candidates, not admitted obligations. There are
therefore three Apply jobs. The second fixture adds one distinct Route-phase
job, giving four completed jobs.

The precise correction is expectations 3 and 4, accompanied by an explicit
same-admitted-domain/semantic-equality assertion and an explanatory comment.
Keep every existing serial/parallel, native-counter, cancellation and initial
orthant on/off comparison unchanged. The subsequent loops have not run in the
failed test bodies, so their equivalence remains a validation requirement,
not an inference from the simple count explanation.

## 2. Cached semantic containment: sound for its stated domain vocabulary

Implementation:

- `crates/rustred-core/src/solver/candidate_reduction/power_domain/summary.rs:38`
  constructs an immutable summary using the existing exact `project` service.
- The same file, line 84, compares coordinate, positive-power A, numerator R,
  and signed D=A-R extrema; nonempty different supports are not identified.
- `crates/rustred-app/src/application/routed_campaign/walking/queue.rs:174`
  retains raw keys and adds summaries only in the unlimited-comparison lane.

The inclusion test is exact because this domain class is *defined* by bounds
on these same linear forms. Intersecting a domain's exact extrema reconstructs
that domain: its points satisfy every tight bound, and the tight bounds imply
every original defining inequality. Comparing all extrema is consequently a
necessary and sufficient subset test here. This is not a claim that arbitrary
sets or arbitrary affine correlations are determined by their marginals.

Important safeguards are present:

- Constructors validate invalid bands before any empty-set shortcut. A valid
  empty candidate is vacuously contained; malformed input is not.
- Aggregate extrema retain wide integer values and genuine infinities. A
  finite implied R above `u32` is not confused with infinity through the
  optional narrow effective-rank field.
- Queue buckets separately enforce immutable program scope, owner and phase.
  The core summary does not itself establish processing or IBP coverage.
- Raw exact keys, admitted descriptors, FIFO obligations and pending work are
  retained. Reverse index retirement at `queue.rs:314` does not cancel jobs.
- Explicitly finite comparison-cap requests retain their previous predicate,
  counting and failure behavior; they do not construct semantic summaries.
- Allocation/counter preflights precede publication and reverse mutation.

The new independent finite-point tests cover 442,368 raw domain descriptions,
identical-set summaries and all resulting distinct-set inclusion pairs, plus
wide/infinite/signed and degenerate-support corners. Queue tests separately
compare admission against enumerated point sets, phase separation, invalid
input and overflow behavior. This is stronger than only testing a few desired
deduplication examples, although it is not a substitute for the pending
integrated gate above.

## 3. Main scaling risk: quadratic serial admission survives the improvement

The forward scan at `walking/queue.rs:211` and reverse maintenance at line 314
still scan an owner/phase candidate list. Cached summaries reduce comparison
cost and remove redundant candidates, but an antichain of M incomparable
regions can still require quadratic cumulative comparisons. They do not change
the single deterministic publication boundary into a parallel index.

The previous correlated pilot completed 49,686 of 195,118 scheduled domains,
with 145,431 queued, after 106.956 seconds of traversal. It recorded
4,971,600,936 comparisons, including 987,619,360 reverse comparisons. During
the later sampled window only about 2.084 cores were busy despite 50 workers;
the coordinator ran while workers waited. This supports a serial admission
bottleneck, not a stack-profile percentage for any particular function.

The new six-run, 49,686-descriptor index replay shows a 4.27-fold median paired
time improvement and almost fivefold fewer comparisons. It replays only saved
completed descriptors, not all proposed/rejected/pending requests, and the raw
baseline is a compact reconstruction of the historical index. It is an index
microbenchmark, **not** a campaign speedup or a completion ETA.

After restoring the gate, rerun the identical correlated input before adding
another index. Record per-owner/phase live candidates, comparisons per admitted
domain, semantic-equivalence hits, proper-subset hits and retirements partitioned
by pending/dispatched/completed state. The existing total of retired candidates
is not the number of jobs that could safely have been skipped. Any future
pending-job supersession needs an explicit obligation-transfer policy, not
deletion based solely on lookup retirement.

## 4. More precise routing is a relevant, small next engine slice

`crates/rustred-core/src/solver/candidate_reduction/routed/domain_overcover/visit.rs:295`
currently initializes mapped target lower coordinates to zero. This is safe
but can introduce numerator-cancellation and pinching possibilities absent
from the actual affine row supports. Existing A/R/D projection removes some,
not all, of that overcover.

For an admitted active bijection `D_i=T_pi(i)` and inactive rows
`Q_i=c_i+sum_j M_ij T_j`, let source numerator exponents satisfy
`L_i<=t_i<=U_i` and `sum t_i<=Rmax`. For each target column j, the safe bound is

```
B_j = min(sum(U_i for M_ij != 0),
          Rmax - sum(L_i for M_ij == 0)).
```

The resulting numerator degree in column j cannot exceed B_j. A surviving
positive source axis with local lower l retains target local lower
`max(0,l-B_j)`, and its pinch is impossible if `B_j<l+1`. Constants consume
source degree even when they lower the expanded target degree; cancellation
can remove terms but cannot violate this upper bound.

For example, `(T1+1)^3/(T0^2*T1)` never cancels `T0`: its column budget is zero.
A total-rank-only cover can nevertheless allow that pinch. This is precisely
the kind of avoidable region expansion relevant to the current traversal.

Independent native diagnostics find support-aware root covers strictly smaller
in 17,304 of 27,807 observed mapped calls, versus 3,696 for a uniform total-R
bound. In contrast, **none** of the 32 observed nonliteral maps is a full unit
denominator permutation. A full-permutation fast path is therefore not a useful
first optimization for this measured traffic.

These counts do not measure eliminated complete successors: the root-only
helper does not enumerate pinched subsets, and some singleton pinches are
already removed by existing A/D projection. Source:
`TMP/route-permutation-census.mWBz2v/AFFINE_SUPPORT_BOUND_PROOF.md` and
`RESULTS_AUDIT.md`.

Required integration cautions:

1. Derive budgets from the **source** extrema for each target support. Never
   reuse bounds inferred only after projecting the all-positive target root
   in a pinched child.
2. Preserve total weighted pinch-cost pruning. Individual column maxima need
   not be jointly attainable; `(T0+T1)^2` cannot provide degree two to both.
3. Keep source conditions, affine constants and conservative upper-D handling.
   Do not reimpose starting A/R restrictions on legitimate descendants.
4. Use checked wide arithmetic and mathematical infinity, not saturated caps.
5. Cache row incidence from the already verified native/Symbolica matrix once
   per map. No polynomial expansion, new CAS kernel or reconstruction is needed.

Minimal validation is concrete native transport endpoint containment over
small domains with constants, zero-degree rows and multiple pinches, followed
by actual emitted-child counts and the same recursive pilot. Root-cover
tightening alone cannot establish the campaign speedup.

## 5. Parallel lifecycle, monitoring and remaining architecture gaps

The separate escrow work in `walking/parallel/escrow.rs` detaches only successful
finished jobs after their final flush. It does not publish later-domain effects
out of order or equate buffered results with completed coverage. The audited
source retains ordered publication, joins workers on shutdown and reports
uncommitted attempts. No lost-obligation or false-completion defect was found
in this inspection. It remains separate working-tree work, not attributable
to the earlier correlated-domain commit.

Its 65,536-entry and 8-GiB **accounted storage** limits are not RSS limits. The
previous pilot hit the entry limit at roughly 2.50 GB. Enlarging this buffer
could keep workers occupied longer, but cannot cure indefinitely growing
single-publisher admission work.

A concrete monitoring risk deserves a small measurement: `walking/execution.rs:362`
accepts a complete result chunk before the heartbeat at line 377 or the next
outer cancellation check. Chunks can contain 16,384 records, and each admission
can scan large candidate lists. Measure maximum chunk-publication duration and
cooperative-stop latency; if needed, poll/report within bounded committed
prefixes without changing deterministic publication order. No cancellation
latency defect was measured here. Worker `notify_all` traffic and repeated
escrow accounting are lesser, unprofiled suspects, not justified priorities.

Two useful but secondary seams are immutable rule-ordinal lookup and pregrouped
RHS shifts: `owners/domains/applied/engine.rs:245` linearly finds a selected rule,
then lines 270–278 allocate and sort its RHS term indices for each application.
Profile their actual share before introducing caches; share any resulting
immutable metadata rather than cloning symbolic expressions per worker.

Finally, the symbolic walker is still diagnostic:
`walking/mod.rs:274` explicitly reports no IBP generation, and line 278 reports
no durable resume support. Concrete `RoutedFeedbackSession::run_round` exists
in `routed_campaign/feedback.rs:274`, but conservative symbolic frontiers do
not yet automatically provide actually reachable missing targets for it.
An initial source-validity frontier can even precede an empty-domain projection
(`walking/mod.rs:235`); this fails conservatively, not by falsely proving a rule.
The current incomplete pilots observed **zero** unresolved frontiers. Their
large queues therefore are not evidence of missing IBPs or a Janet/Ore failure.

## Recommended sequence

1. Correct the two mathematically obsolete count assertions, add the explicit
   equivalent-domain explanation/assertion, and pass their complete parity
   loops plus the full application gate.
2. Measure cached semantic admission on the unchanged correlated pilot. Do not
   infer campaign speedup from the completed-descriptor replay.
3. Implement the small support-aware affine degree-bound slice, independently
   audit it and run endpoint and same-input recursive comparisons.
4. If admission still dominates, use per-bucket evidence to choose a selective
   index or a properly tracked pending-obligation supersession policy. Measure
   monitor/stop latency before a longer attempt.
5. Widen the physical starting domain only after these controls become useful.
   The current R10 diagonal is not the full marginal R14/full-jet R15 envelope.
   Invoke source generation only after obtaining a concrete reachable missing
   target. Terminal minimization and numerical evaluation remain out of scope.

There is no defensible fifteen-hour whole-envelope ETA yet. The changes above
target measured redundant work without mistaking a conservative symbolic
overcover, a fixed-prefix success or a saved diagnostic receipt for closure.
