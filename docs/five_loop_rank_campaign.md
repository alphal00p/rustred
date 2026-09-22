# Shared rank-bounded vacuum campaign

## Objective and delivery order

The September 21 directive is authoritative. (a) Solve the complete supplied
five-loop family/sector census with a finite, possibly large terminal basis;
(b) achieve R=10 first and aim for R=20; (c) minimize terminals afterward;
(d) obtain/map published numerical masters; (e) integrate and validate Vakint.
Do not advance to a later gate before the earlier one succeeds. Independent
certification is optional and deferred, not the current development target.

The external census currently contains four parents, 67 graph classes and
8,246 labelled sectors. These numbers belong to input data only. The core
implementation must remain independent of topology names and loop count.

R means the sum of negative index magnitudes in the original input coordinates.
Positive denominator powers are not bounded. Neither intermediate rank nor
source translations are clipped to R. A finite target sweep is a diagnostic,
not a substitute for the requested parametric scope. Keep this distinction in
every progress report even while independent certification is deferred.

## Starting point

There are saved candidate programs for all 67 representative classes and exact
routes covering all census labels. The existing serial routed API already
deduplicates all targets supplied in one call, but the earlier external sweep
called it separately for each target. The new campaign must batch requests as
well as add concurrent scheduling, cancellation, and live progress. Two R=10 probes stopped
at 30-minute deadlines without completed frontier reports. This does not show
whether additional rules are needed. The first improvement must remove repeated
cross-request work and expose progress before launching another long attempt.

The latest native rational-coefficient numerator expansion and one-parameter
affine refinement pass 2,492 release core tests and 251 application tests.
A subsequent single-target control finishes the same dependency graph in
42.022 s trace time versus the earlier 54.968 s, but whole-process time is
220.97 s versus 160.34 s because preparation is slower in this observation.
These are single shared-host observations, not a controlled overall speedup.

The next shared-scheduler and directed-domain-search slice passes 2,515 core
release tests (zero failures; 32 existing ignored diagnostics), including
serial/parallel result equivalence, cancellation, above-entry-rank successors,
and preserved symbolic positive-power rays. The application/frontend release
gate also passes all 259 tests. The shared finite-target traversal is not yet the source-feedback
loop or a resumable parametric campaign; neither its completion percentage nor
zero sampled frontiers may be reported as full R10 closure.

The first shared rank-one control completes in 82.645 s with one worker and
34.973 s with six, with matching reported counters. These are single observations
of traversal time, not total solver speedups. The first 50-worker R10 pressure
batch is deliberately stopped after worker failures and continued native-call
memory growth; it peaks near 275 GB without a complete Rust result. Independent
reviews identify a native Symbolica polynomial-power mixed-radix overflow that
must be corrected and tested before retrying. A native map-only diagnostic
confirms that all eight stalled calls encounter the overflow; this does not yet
measure the corrected expansion or eliminate genuine dependency breadth. See the
[pressure-test evidence](research/shared_rank10_pressure_2026-09-21.md).

## Implementation slices

1. **Shared canonical work.** Install saved representative programs once.
   Route equivalent subtopologies to their existing canonical owner through
   validated exact momentum maps. Share work by full integral/owner identity,
   not only sector mask or rank. A worker must not repeat an expansion already
   scheduled or completed by another worker. Preserve local exact coefficient
   cancellation, zero handling and strict descent. Store finite uncovered
   frontiers separately; never silently retain an unresolved ray as a master.
2. **Parallel scheduling.** Use one bounded campaign worker pool, with dynamic
   scheduling across sector/dependency jobs and an explicit shared-memory
   budget. Avoid cloning Symbolica expressions per worker and avoid nested
   compute-pool oversubscription. Publish reports in deterministic order;
   independent scheduling must not alter the mathematical rule/terminal result.
3. **Frontier feedback.** Prefer existing saved compatible owner programs and
   existing routed rules before new source search. Canonicalize missing-owner
   and missing-rule requests so all parents benefit from one solve. Publish
   completed solutions through immutable versions, retry affected dependencies,
   and checkpoint progress without mixing incompatible rank/terminal policies.
   A first shared-trace implementation is not yet this entire feedback loop.
4. **Public steering and monitoring.** Provide input-driven Rust/CLI plumbing
   and a Python campaign driver. Report completed/queued/active/failed sectors,
   distinct shared dependency nodes, deduplication hits, rule applications,
   routing expansions, uncovered entries, terminals, throughput, CPU and RSS.
   TTY output overwrites a colored dashboard; non-TTY output records equivalent
   periodic structured events. Progress must continue during expensive phases,
   and cancellation/checkpointing must be cooperative where possible.
5. **Grounded rollout.** Run focused correctness tests and independent audits;
   compare shared serial/parallel results on small fixtures and the saved
   rank-one controls; then run the combined R=10 campaign. Reuse saved rules.
   Record all stopped/incomplete attempts distinctly from completed timings.

## Operational policy

### September 22: move shared work to symbolic domains

The next slice adds conservative domain routing through admitted maps and
bounded inactive refinement of selected native guard preflight refusals.
Route requests share whole support/rank orthants; full mapped roots enter Apply,
while strict subsupports reenter Route. Exact source validity is still required,
so unchecked source conditions remain obligations. No polynomial numerator
expansion, source regeneration, or automatic missing-rule inference follows
from an overcover. The opt-in CLI/Python controls and separate RHS work budgets
are documented in [the matching interface](shared_owner_domain_matching.md).
Use measured local/full-census and shared-walk controls to decide the next
optimization; this is still preparation for the complete parametric campaign,
not an alternative definition of completion.

The selected-rule successor visitor specializes fixed faces using the native
indexed algebra, validates original terms before coalescing equal shifts, and
retains exact translated rank constraints. Bounded sign-crossing coordinates
are fixed when necessary; positive tails are not clipped. A new opt-in
application worklist deduplicates containing owner/box/rank domains in one
immutable snapshot. Pending inclusion saves scheduling, not completed work:
all admitted domains must still be processed and unresolved obligations remain
visible. Conditional coefficient images are explicitly conservative requests.

The initial visitor/worklist leaves routing frontiers explicit. A reviewed next
step can avoid expanding native numerator polynomials for coverage requests:
an admitted unit-active-row bijection maps rank-R numerators to degree at most R,
with resulting support contained in the mapped root. Its full-root and strict
subsupport rank-R domains therefore over-cover actual endpoints. Preserve the
existing Apply-versus-Route phase and support-count descent when integrating
that bound. Uncovered points of this over-cover are not automatically reached
missing rules. See the [interface](shared_owner_domain_matching.md).

The first full-census local-match control (all 67 input owner domains at R10)
stops on a first-owner native guard degree allowance, requested 17 versus 16.
This is an incomplete prefix, not a failed generation or a completed census.
The public `--max-guard-univariate-degree` knob changes only that native work
allowance, not the numerator-rank bound. The completed release gates pass
2,607 core /307 application /18 Python tests. Automatic refinement resolves
the six earlier R10/R11 queries; the full-census retries remain incomplete in
their first owner at predicate and then native factor-work allowances. Two
successor trials reach only routing frontiers before work limits. See the
[measured update](research/shared_symbolic_domains_2026-09-22.md). Do not
regenerate usable owner programs for these scheduling/guard diagnoses.

### Current implementation and next measured retry

The generic core now publishes validated children in bounded batches and uses
one full-integral-key membership index instead of two ordered indexes. Exact
native arithmetic and per-edge descent checks remain unchanged. The release
gate, including the subsequent native support visitor, passes **2,554 core
tests**, with zero failures and 32 existing ignored
diagnostics. A structural degree/variable support bound reduces unnecessary
output-size overestimates; multiplication work retains a separate allowance.

The Rust application also has an opt-in retained
[`RoutedFeedbackSession`](owner_source_feedback.md): completed finite traces
nominate actual missing-rule rays, whose positive powers stay symbolic, and
new source-derived rules are installed in immutable shared overlays. It does
not yet enumerate the complete R10 domain, persist the dependency queue, or
provide an automatic CLI source-feedback loop. Resource failures do not
nominate rules. The application release gate passes **276 tests**, and all
**10 Python supervisor tests** pass. Independent source/runtime reviews pass.

The next operational retry uses the same saved owners and rank-ten pressure
inputs after matched one-/six-worker controls. Additional already-saved
compatible owners may be installed as alternative routing representatives if
they avoid expensive numerator expansion; that is a distinct input-policy
experiment, not a same-workload speedup. None of these finite diagnostics is
silently substituted for the complete parametric campaign.

The batched and additional-direct-owner diagnostics both finish with an explicit
operator cancellation for optimization, not a timeout or missing-rule failure.
See the [measured outcomes](research/shared_rank10_pressure_2026-09-21.md).
The native-support slice streams the exact native-coalesced support needed by
dependency tracing instead of building coefficient wrappers that it discards;
the public coefficient-returning reducer and all virtual per-call budgets stay
unchanged. Independent static and runtime reviews pass, including nine added
regressions and the full release suites. Matched one/six-worker traversal times
are 20.437/8.067 seconds, with all reported graph counters unchanged. The
subsequent 50-worker finite R10 diagnostic is stopped for optimization after
854.064 seconds of traversal, at 42.7 million queued nodes and 25.47 GB peak RSS.
Neither measurement is a complete parametric-family solve.

Do not let concrete pressure diagnostics replace the main implementation task:
use saved owner domains and shared successor obligations to retain unbounded
positive powers parametrically. The retained source-search service already
provides new rules for genuine gaps; the missing full-domain worklist must reuse
those primitives rather than regenerate all owners or launch an independent
certification project.

The new fixed-shard membership preprobe retains one ordered, exact global
admission step. Positive duplicate hints are monotonic; misses are rechecked
before insertion. Full-key equality, phase identity, per-node budgets,
valid-prefix errors and cancellation are preserved. A complementary streamed
[owner-domain scan](owner_domain_scan.md) retains unbounded positive powers,
source-rank predicates and native guard references. It inventories potential
successors, not exact first-applicable regions or proven missing jobs. The
combined release gates pass **2,576 core tests** (32 existing ignored) and
**281 application/integration tests**, zero failures. Performance measurements
and the real 67-owner domain inventory must precede the next large retry.

That inventory now completes: 17,975 rules, 1,667,335 RHS terms and 4,758,436
potential sign-cell successors across all 67 installed owners, in 11.009 seconds
after preparation. The reporting-only follow-up passes all 283 app tests and
12 Python steering tests. See the [full measurement and boundaries](research/shared_owner_domain_inventory_2026-09-21.md).
The next implementation priority is the shared parametric-domain feedback
worklist, using native guards and the existing source-search service. A complete
stored-rule scan or another finite target expansion cannot substitute for it.

## Resource and monitoring policy

The next ordered local-domain matcher now passes 2,588 core, 296 application
and 16 Python steering tests. Its real three-owner experiment finds existing
rules for all three examined rank-11 child boxes. An unresolved source guard
is resolved by ten exact finite-numerator slices while retaining its infinite
positive-power ray; no new IBPs are generated. This demonstrates a useful
bounded-rank worklist refinement, not full recursive coverage. Incorporate that
generic refinement and continue actual successor/source feedback; do not resume
large concrete-dot expansion as a replacement for the symbolic campaign.
See [the complete measurement and scope](research/shared_owner_domain_matching_2026-09-21.md).

- At most 50 compute cores for the aggregate campaign; cap native inner pools.
- At most 500 GB aggregate resident memory, with a 450 GB soft stop and
  attributable process-tree monitoring. Include helper jobs and compilation
  when concurrent. Memory estimates must not be presented as measured RSS.
- No inherited 30-minute deadline. The objective is completion within 15 hours.
  A 15-hour objective is not evidence of progress: inspect throughput, backlog,
  expression/expansion growth, memory trend and hot-phase profiles.
- Do not continue a run that is demonstrably unlikely to finish within the
  objective. Save reusable work, report why it stopped, optimize the observed
  bottleneck and retry. Avoid an unsupported ETA or a rigid extrapolation from
  one unusually easy/hard sector. A supervisor can record stop requests, but
  the decision must be explainable from measurements.
- Use workspace-local `TMP/` for transient evidence and inherit the Symbolica
  license from the environment. Never commit credentials or reference material.
- All algebra uses Symbolica's public API; audit it before adding any primitive.

## Acceptance for the next milestone

Shared serial and parallel campaigns must agree on terminal/frontier results;
duplicates must demonstrably avoid repeated work; a missing rule must remain
visible; cancellation/resource exhaustion must never produce a success report.
The long run must state its full input/rank scope and whether it completed.
Keep numerical master evaluation and five-loop Vakint outside this milestone.
Tests, profiles, independent reviews and measured documentation accompany each
coherent commit/push, using the requested ValentinHirschi identity.
