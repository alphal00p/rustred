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
routes covering all census labels. The existing routed engine shares immutable
rules but currently traces each request independently. Two R=10 probes stopped
at 30-minute deadlines without completed frontier reports. This does not show
whether additional rules are needed. The first improvement must remove repeated
cross-request work and expose progress before launching another long attempt.

The latest native rational-coefficient numerator expansion and one-parameter
affine refinement pass 2,492 release core tests and 251 application tests.
Their effect on the saved-owner workload has not yet been measured.

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
