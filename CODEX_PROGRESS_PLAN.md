# RustRed five-loop completion: optimization, integration, and controlled deployment

Current delivery authority: `SHORTENED_PLAN.md`, approved September29 at13:25
UTC, with the user's overlapping Stage A clone/build and root Stage B branch
instruction. It supersedes historical delivery deadlines below. The6±1-hour
target is guidance; correctness takes precedence. Overall scope is unchanged.

## 1. Goal and execution rules

Advance RustRed toward a reusable closing rule system for the **existing frozen five-loop QCD-renormalization input scope**, prioritizing near-ready improvements, reducing redundant domain traversal, and improving multicore efficiency. Preserve topology- and loop-count-generic algorithms.

Upon starting implementation:

- Write this entire plan **verbatim** to `CODEX_PROGRESS_PLAN.md`.
- Create the currently absent tool-managed goal for this objective and add the authoritative directive to `GOAL.md`, superseding conflicting historical stop instructions.
- Initialize `CODEX_PROGRESS.md` with the audited starting state, active campaign, branch revisions, completed validations, outstanding work, and decisions.
- Leave the calculation, name, and rendering of **pending growth per completion unchanged**.

Keep the current LC2 campaign untouched. Only the user stops, starts, or resumes production. Preserving its checkpoint is desirable only when straightforward and tested; it must not constrain optimization or trigger an elaborate compatibility project.

This work concerns symbolic closure and its performance. Terminal minimization, numerical master evaluation, and further Vakint development remain subsequent stages.

## 2. Delegation, evidence, and progress logging

The primary agent acts chiefly as orchestrator, integrator, resource coordinator, and final verifier.

- Initially assign separate agents to **G2′ integration**, **rescue integration**, and **independent mathematical/code audit**. The auditor does not approve its own implementation.
- Subsequently rotate implementation, profiling, research, and independent critique across the available slots. Serialize overlapping edits and resource-intensive measurements.
- Keep `CODEX_PROGRESS.md` continuously current: update after material results, decisions, merges, failed experiments, and at every handoff. Include responsible agents, commits, exact commands, evidence paths, measured versus inferred findings, blockers, and the next executable steps.
- Keep a backlog in that log with explicit states: pending, active, delivered, rejected, or deferred with a reopening condition. Preserve negative results.
- Commit and push coherent, audited milestones to `fable_5_1`, using the requested ValentinHirschi Git identity and existing coauthorship convention. Preserve unrelated changes and never commit reference-only material, licenses, or campaign outputs inadvertently.

Use Symbolica for algebra. Audit pinned and current public APIs and existing RustRed usage before adding any algebraic primitive; do not create a competing CAS implementation.

## 3. Implementation sequence

### First: finish G2′ and frontier rescue

Reconcile existing receipts with stale handoff instructions, then integrate the existing branches onto the current LC2/Symbolica baseline rather than reimplementing them.

**G2′ residual reuse**

- Retain exact union coverage, indexed anchor lookup, explicit dependency edges, and publication-order checks.
- Borrow only the correctly inspected scope of partial anchors.
- Fall back to ordinary inspection when residual planning cannot decide; never truncate coverage.
- Preserve the existing opt-in and flag-off behavior.

**Rescue and required-query scope**

- Replace substring-based helper identification with an explicit, complete, immutable query-role declaration.
- Preserve all **116 existing physical/convenience starting queries as required** and identify the 67 original helpers as auxiliary. Do not silently narrow the frozen request.
- Undeclared queries remain required; rescue cannot proceed with an ambiguous role declaration.
- Retain append-only amendments, blocked-ancestry quarantine, bounded recovery attempts for recognized cases, and explicit stops for unknown failures.
- Required queries close only through exact containment in recursively closed results; failed auxiliary helpers do not automatically invalidate independently closed required queries.

**Combined behavior**

- Exclude quarantined anchors from new G2′ plans.
- Preserve accepted decisions and their dependencies during replay; invalidate or revalidate outstanding proposals against changed rescue state.
- Test both activation orders and an anchor becoming blocked after earlier successful local inspection.
- Support checkpoint activation only through existing, validated mechanisms. Otherwise prepare a fresh campaign.

### Second: cheap bottleneck relief and the epoch engine

Use current profiles to select narrow coordinator improvements, including helper-batch latency and unnecessary per-commit telemetry construction.

Advance the epoch engine in reviewable stages:

1. Reconcile completed S2 gates and remaining integration failures.
2. Implement durable merge-boundary checkpoints and correct interruption, memory-stop, error, and restore behavior.
3. Move successor lookups to inspectors using immutable snapshots and thread-owned Symbolica contexts; optimize the measured miss path.
4. Reduce merge overhead through typed records, bulk edges, and parallel preparation with controlled state mutation.
5. Introduce bounded rolling scheduling with recorded snapshot/merge decisions and replay validation.

Do not mistake the existing lockstep skeleton for the completed performance architecture. Retain the **≥1.5× matched-throughput gate** against a contemporaneous legacy baseline before recommending epoch deployment.

### Third: remaining optimizations and new research

Evaluate every remaining handoff avenue, prioritizing demonstrated bottlenecks:

- Allocation-free applied geometry.
- Symbolica finite-field nonzero witnesses, with exact fallback for inconclusive evaluations; never confuse “not identically zero” with “nonzero everywhere.”
- Coverage-first coefficient work without suppressing denominator, guard, or frontier obligations.
- Scheduling and ordering choices that reduce total work, not merely increase busy cores.
- Memory layout, checkpoint cost, and process-local NUMA placement.
- New algorithms suggested by profiles, SpiReD, or relevant literature.

For each candidate, register the mechanism, expected benefit, falsifier, and smallest representative test before implementation. Previously rejected approaches require new evidence before reopening. Park unsuccessful performance experiments after one evidence-backed correction and retest; do not let speculative work delay useful releases.

## 4. Validation and performance decisions

Use optimized frozen binaries, identical inputs, matched worker budgets and CPU placement, and consistent timing boundaries. Record host contention.

Every engine-affecting milestone covers:

- FG, BMW, H, X, combined four-loop controls, and the finite five-loop control; add the hot-sector control for G2′.
- Exact flag-off identity where semantics should be unchanged.
- Independent full reinspection for claimed completed controls.
- Targeted mutation, quarantine, query-role, interruption, checkpoint, and replay tests.
- Rust, CLI, and Python integration checks affected by the change.

Each exploratory or comparison pilot lasts **at most 30 minutes, including restore, preparation, and orderly shutdown**. Schedule cooperative stopping early enough to save safely. Incomplete runs are reported as censored, not successful closure. Compilation is recorded separately.

Compare wall time, CPU time, memory, scheduled domains, peak pending, closed required queries, frontiers, work mix, and coordinator/inspector costs. Do not substitute summed inspection times or CPU utilization for whole-campaign improvement.

A campaign-switch recommendation normally requires a reproduced benefit in two matched pairs:

- At least **20% less end-to-end time**, or
- At least **20% less domain work, pending pressure, or memory**, without an offsetting material throughput regression.

Epoch deployment retains its stronger 1.5× throughput gate. A validated rescue that removes an actual blocker can justify a switch independently of speed. Noisy or unrepresentative measurements remain inconclusive.

Respect LC2’s CPU reservation and memory headroom, existing build/heavy-job locks, and other users’ workloads. Do not change host-wide settings or run competing benchmarks on production’s cores.

## 5. Deployment and completion

At a qualifying milestone:

1. Commit and push the tested code, documentation, and progress report.
2. Freeze the optimized executable and record its identity and input compatibility.
3. Prepare exact Nix/build/launch instructions and a rollback path.
4. Recommend that the user gracefully stop LC2 and switch.
5. Prefer checkpoint continuation only when cheap, compatible, and tested. Otherwise recommend a fresh campaign without rejecting the optimization.
6. State which gains were measured, what remains uncertain, and whether progress must be recomputed.

Continue read-only observation during development to guide priorities. Never infer eventual completion from zero frontiers, faster inspections, or a temporarily shrinking queue.

The mathematical completion milestone is that **every required query in the frozen scope has its reachable obligations discharged**, with reusable output that cold-loads successfully. Report this as scoped closure, not unrestricted five-loop family closure. Until then, keep open dependencies, remaining research, and deployment decisions explicit in `CODEX_PROGRESS.md`.

## September 29 follow-up: consolidate before restarting

The user's subsequent directive, verbatim:

> Continue as planned, and not that it's ok to restart the campaign.
> But I would like you to have put in the build all ideas first.

This supersedes the earlier milestone-by-milestone production-switch timing:
do not recommend or perform a G2-only restart. A restart is acceptable after
the planned improvement avenues have been evaluated and the beneficial,
validated changes integrated into a consolidated build. Finish the current
G2 comparison unchanged and use it as the baseline for that build.

Continue the epoch S3–S6 work and the remaining profiling-led candidates;
retain explicit evidence-backed dispositions for every avenue. "All ideas"
does not mean enabling rejected, harmful, or untested experiments. Keep such
results and reopening conditions in the backlog instead of silently dropping
them or weakening their acceptance gates. Newly discovered speculative ideas
must likewise be evaluated, not automatically added to the default engine.

Leave LC2 running untouched while this consolidation proceeds. A later
restart must use the tested consolidated executable, a qualified resource
configuration and a graceful save/drain with the old campaign retained for
rollback. Fresh recomputation remains acceptable; no checkpoint-compatibility
project is required. Correctness, performance and scoped-closure requirements
elsewhere in this plan remain unchanged.

## September 29 delivery timebox: stable consolidation within 3–4 hours

The user's subsequent directive, verbatim:

> Alright continue with the progress of the plan and goal, but due to quota limitation I would like you to be able to deliver a consolidated version in the next 3 to 4 hours maximum. What do you think you can still afford to implement and complete given that constraint. At the end of that timelapse I would like a cleaned-up stable push from which I can either resume the existing campaign or start a new one (which I think starting a new one will be needed right). Give me your assesment of what you can cover within that timespan (hopefully all, or if not what you think you'll need to leave out).

This overrides waiting for every remaining experimental avenue before a usable
release. Start the delivery window at 2026-09-29 12:46 UTC: target handoff by
15:46 UTC and hard cutoff 16:46 UTC. Do not spend the remaining window on new
speculative features. Freeze the release source early enough to complete the
optimized build, native/CLI/Python and cold controls, measured comparison,
documentation and push. Keep LC2 untouched and leave launch/resume to the user.

Prioritize the already validated legacy G2/rescue implementation, then the
independently reviewed and natively tested geometry-buffer and lean-telemetry
changes. These form the fallback production release. An optimization not ready
in time is omitted rather than weakening its tests. Existing component gains
are not a new consolidated-build measurement.

Preserve current Epoch work on clearly experimental branches with exact test
status. Finish the running frozen-candidate validation if feasible, but do not
require completion of Epoch G2/rescue composition, bulk merge, rolling scheduling
or its 1.5x deployment gate for this timeboxed handoff. These remain backlog,
not silently completed or default-enabled. No fresh CAS/reconstruction work.

Try the existing cheap, tested legacy resume/upgrade mechanism on a copied
checkpoint; recommend resume only on a passing compatibility/replay check.
Otherwise prepare a fresh campaign and rollback instructions. A future Epoch
deployment needs a fresh campaign; that is not automatically required for the
legacy fallback. Report the distinction explicitly.

At handoff, deliver a stable pushed revision, frozen executable and exact
Nix/build/launch instructions, measured versus unmeasured benefits, disposition
of each remaining avenue, retained old campaign/checkpoint, and concise next
steps. Do not claim five-loop closure or an ETA from pilot success.

## September 29 revised timebox: first useful Epoch deployment candidate

The user's latest directive, verbatim:

> I am really excited about this architectural re-design (and it's ok if it requires the launch of a whole new campaign) especially if it mains being able to scale up core utilization without doing excessive redundant work.
>
> So can you fit a compiled reasonably tested build to have a first shot at the 5-loop campaign with this architecture within a time budget of 6 to 8 hours max then (with some subagents to speed up the implementation)?
> I am a bit short on quota so I would need a delivery of this promising idea within that time/compute frame.
> You sort of know what do do already, so can you adjust the plan to get there within the allotted time?

This supersedes the preceding 3–4-hour legacy-first delivery scope. Target a
useful fresh-start **Epoch candidate**, not the entire long-term architecture.
Window measured from 12:57 UTC: aim for 18:57 UTC, hard handoff by 20:57 UTC.
If execution is unavailable because of quota/capacity, report that explicitly;
do not imply agents are working or silently extend the deadline. Current goal
status is `usageLimited`; reactivation of a subagent was rejected with `agent
thread limit reached`. The root cannot clear either condition through goal tools.

### Required first-campaign functionality

- Real bounded overlap of inspection and publication, beyond the current
  16-job global lockstep barrier. Versioned immutable lookup data must coexist
  with new publication without copying the whole store per worker or epoch.
  Maintain a bounded in-flight window and bounded old-view retention. A larger
  lockstep batch alone does not meet this objective.
- Reuse the existing inspector-side lookup and exact merge checks. Negative
  lookups are only reusable for the state actually searched; new proposals
  still undergo deduplication. Preserve complete dependency edges, publication
  order, replay decisions and strict lifecycle transitions.
- Integrate fresh-start G2 Union to avoid paying for higher utilization with
  excessive redundant domain work. Keep the Off control. Correct eligibility
  and inspected scope must agree in planner, merge, checkpoint and cold reader.
- Preserve all116 required queries and67 original auxiliary helpers. Rescue
  must retain its append-only scope and quarantine semantics; do not silently
  remove it. A candidate still refusing required rescue is explicitly partial,
  not a fully qualified replacement for the current campaign.
- Retain graceful interruption, RAM-stop and durable checkpoint/resume within
  the new engine, plus truthful native/Python monitoring. Fresh input is fine;
  no CP5-to-CP6 migration or historical-checkpoint compatibility project.
- Reuse Symbolica thread-owned contexts and current exact algebra. No new CAS,
  rational reconstruction, terminal minimization or Vakint work in this window.

### Parallel lanes and scope control

When agent capacity is available, use three orthogonal lanes: (1) bounded
rolling/snapshot lifetime and merge implementation; (2) G2/rescue integration;
(3) independent code/mathematical audit and validation. Root integrates,
coordinates resources, profiles and prepares delivery. Reviewers do not approve
their own changes. Serialize overlapping edits and heavy jobs.

First30minutes: finish/assess the frozen9ec build, settle the smallest viable
overlap mechanism and separate edit ownership. By hour3, require an integrated
source candidate with real overlap and reuse, or explicitly report the missing
mechanism. Reserve at least the final3hours for one optimized build, focused
native/CLI/Python checks, controls, cold verification, pilots and handoff.
After source freeze allow narrow correctness fixes, not more optimization ideas.
Use existing harnesses and receipts; do not build another acceptance framework.

### Acceptance and recommendation

- Test same-mode checkpoint/resume, cancellation and resource stops, stale
  snapshots, duplicate proposals, G2 coverage and quarantine interactions.
  Validate mathematical output independently; utilization is not correctness.
- Run FG/BMW/H/X, combined four-loop, finite five-loop and hot-sector controls
  with frozen release binaries and the existing bounded pilot rules. Report
  unfinished controls as censored. Scope/role counts must remain unchanged.
- Compare whole-campaign time, CPU, memory and domain work against current
  G2-enabled legacy, not only LC2 with G2 off. Include matched worker placement
  and observed busy cores. Keep the established1.5x Epoch deployment gate;
  high CPU use alone never justifies switching.
- A correctly executing candidate that misses the performance gate may be
  delivered as experimental, but is not recommended to replace LC2. An
  incomplete or failing engine is preserved as unfinished work, not called
  launch-ready. No five-loop completion-time promise or closure claim.

Defer adaptive ordering, sophisticated compaction/NUMA tuning, general migration,
extra witness algorithms and unrelated micro-optimizations. Their omission is
intentional; the deadline is for a useful first architecture, not every idea.

Deliver the tested revision and frozen executable, exact Nix/build/fresh-launch
instructions, resource configuration, known limitations, pilot evidence and
rollback to untouched LC2. Commit/push coherent code and update progress before
handoff. Only the user stops or launches production.
