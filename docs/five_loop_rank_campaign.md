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

### Current implementation and next measured retry

The generic core now publishes validated children in bounded batches and uses
one full-integral-key membership index instead of two ordered indexes. Exact
native arithmetic and per-edge descent checks remain unchanged. The release
gate passes **2,545 core tests**, with zero failures and 32 existing ignored
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

## Resource and monitoring policy

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
