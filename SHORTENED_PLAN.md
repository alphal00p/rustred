# Shortened delivery plan: compatible upgrade, then parallel Epoch

Status: **approved and executing**, T0 = **2026-09-29 13:25 UTC**. This plan
supersedes earlier absolute delivery deadlines. The user's approval explicitly
permits Stage A compilation to overlap Stage B implementation: Stage A uses an
isolated repository clone inside this workspace; Stage B uses the requested
`fable_5_1_parallel` branch at the repository root. Neither stage waits for the
user to resume or launch production. Only the user operates those campaigns.

## Objective and invariant scope

1. Deliver a stable optimized `fable_5_1` milestone that the user can use to
   pause and resume the existing LC2 campaign, retaining its progress.
2. Deliver a compiled, reasonably tested first parallel-Epoch build on
   **`fable_5_1_parallel`** for a new five-loop campaign. The target is real
   inspection/publication overlap and useful multicore scaling without
   excessive duplicate work, not a larger lockstep batch alone.

The overall mathematical goal remains scoped closure of all **116 frozen
required queries**, with their descendants and **67 original auxiliary
helpers**. Do not narrow it to improve a benchmark. Terminal minimization,
numerical evaluation, Vakint and unrestricted-family certification are out
of this delivery. No five-loop completion time is promised.

Target total elapsed time: **6 hours**; reserve: **1 additional hour**.
The user clarified this is a guideline, not a deadline that warrants a rushed
or incorrect implementation. Aim for19:25 UTC, with20:25 UTC reserve; report
slippage and continue the agreed work rather than weakening it or hiding gaps.
Stage A targets 30 minutes, followed by 5h30 for Stage B. Builds and tests count
toward this delivery clock; compilation remains excluded from solver timings.
If a milestone slips, report it immediately and spend the reserve explicitly.
Do not silently extend the schedule or call an unfinished build ready.

### Latest delivery instruction — 2026-09-29

Continue until the `fable_5_1_parallel` branch is committed/pushed, demonstrated
stable and at least performing well on the combined four-loop control, with a
bounded, measured study informing input/family order and algebraic coordinate
priority. A literature/source review alone is not the requested experiment.
Use the same compiled backend across input variants; retain negative results
and distinguish a heuristic recommendation from an optimum.

The user now wants to launch the parallel campaign in **tab `codex_astra` of
Zellij session `rustred`, alongside LC2**, not replace LC2. This supersedes
earlier stop-and-switch instructions for final Stage B deployment. Preserve
LC2's CPUs128–227 and checkpoint. Final launch instructions must assign a
disjoint CPU set and a RAM budget that allows both campaigns and system
headroom. Only the user launches production. Finish the stability/performance
gates even if necessary corrections consume more than the tentative timebox;
do not end at an unqualified source-only checkpoint.

### Additional authorized window — 2026-09-29 about20:43 UTC

The user authorizes another **4–5 hours**, targeting a decisive win on the
four-loop gate and a better-founded ordering choice before the alongside
five-loop launch. Aim to consolidate by00:43 UTC on September30, with01:43
as the five-hour planning boundary. Report actual gate results and any slippage;
do not manufacture a win by narrowing the workload or weakening verification.

The natural/reverse experiment is useful negative evidence, but no longer
satisfies the requested depth of ordering investigation by itself. Add:

1. **Mechanistic input and pivot ordering.** Use actual pinch relationships,
   common routed subtopologies, owner equivalences and rule/guard costs to
   derive a small, justified portfolio. Ask which intermediate results provide
   useful coverage to several parents, and which broad early domains create
   excessive descendants. Distinguish query admission order, routing-owner
   priority, pending-job dispatch and algebraic coordinate priority. Preserve
   every required query and the mathematical scope. Compare on combined
   four-loop controls, not just a convenient individual topology.
2. **Historical12/67 versus6/67 audit.** Find the old evidence and compare
   actual initial domains, bounds, owner programs, policies and closure-counter
   definitions. Check for stale conservative snapshots and changes to cycle/
   dependency bookkeeping. Do not assume either regression or comparability
   from the displayed count alone. Leave production read-only; avoid a large
   full-checkpoint replay until a narrow diagnosis makes it necessary.
3. **Input-driven experiments.** Reuse the same optimized executable for
   supported steering variations. A changed algebraic order requires temporary
   regenerated/validated rules, not a Rust rebuild. Add only small generic
   steering controls if a concrete experiment is otherwise blocked; audit and
   test them once, rather than recompiling for individual permutations.
4. **Measured delivery.** Keep native closure-walk and independent cold costs
   separate: faster verification alone must not be described as faster IBP
   generation or proof of improved five-loop scaling. Repeat qualifying
   combined-four-loop comparisons; retain unfavorable and censored results.
   The1.5x criterion still governs a faster-replacement claim, not the user's
   explicitly experimental alongside first shot after correctness, four-loop
   performance and resource gates pass.

Parallel lanes: `bounded_ordering_pilots` owns the existing optimized build and
matched controls; `parallel_gate_critique` owns structural ordering research/
input design and independent verifier audit; `stage_a_release` owns the
historical closure-count audit. Root integrates, coordinates resources and
keeps `CODEX_PROGRESS.md` current. Reassign implementation and review to
different agents if a concrete bug or useful steering change is identified.
No production actions, new master/Vakint work, or open-ended permutation search.

### Additional monitoring request — 2026-09-29

Add a separate trailing-hour metric for the observed rate of domains discovered
minus the observed rate recursively closed. Leave the existing pending-growth
per-completion calculation and text unchanged. Show signed units, the actual
sample span during warm-up, and closure-snapshot freshness: closure counts are
updated in batches, so a negative observed gap trend is encouraging but not a
proof of convergence or an ETA. Handle missing telemetry, counter resets and
resumes without manufacturing a rate. Reuse existing monitoring and telemetry;
this presentation change must not require a solver rebuild or modify LC2.

## Stage A — compatible stable milestone on `fable_5_1`

### Scope

- Consolidate the already implemented, independently reviewed optimizations:
  legacy G2/rescue code, projected-geometry buffer reuse and lean coordinator
  telemetry. Keep unfinished Epoch work off this stable delivery path.
- Clean up only task-owned changes and update documentation to match the
  executable actually delivered. Preserve unrelated user changes, including
  the current Cargo.toml and feynkit edits. A clean task-owned commit does
  not mean erasing other collaborators' worktree changes.
- Preserve the current campaign's owner inputs, exact query bytes/order,
  scheduling options, checkpoint semantics and bindings. This is a compatible
  executable upgrade, not an input/role migration.

### Verification and handoff

1. Reuse existing exact-source audit/test evidence where it applies. Build
   the optimized CLI from the selected source; never present the app-opt1
   correctness-test executable as a production-performance binary.
2. Check the existing upgrade probe and a genuine save/resume path, including
   a read-only identity check against LC2 and a **separate copied checkpoint**
   where practical. Any copy must refer to one coherent committed generation;
   do not resume or modify the live checkpoint. Use a representative compatible
   control for execution if the large live-state copy cannot fit the timebox;
   clearly distinguish that from a full-size replay test.
3. Run focused regressions affected by the selected changes and a small
   end-to-end/cold control. Previously executed full suites are evidence only
   for matching code; new code requires its applicable checks.
4. Commit and push to `fable_5_1`, freeze the exact executable and identity,
   and give the user the tested pause/upgrade/resume and rollback commands.
   Announce **Stage A ready** immediately, without waiting for Stage B.

**G2 caveat:** the current production Python launcher preserves frozen G2
settings and intentionally rejects Off-to-Union activation. Native activation
support is not the same as a tested production steering path. A plain binary
upgrade must not be advertised as obtaining the measured 24–28% G2 work saving
if it leaves G2 off. Do not change roles or bindings to force activation. If a
small, safe activation adapter cannot be independently tested within Stage A,
defer it and state exactly which optimizations the compatible resume enables.
Full explicit-role G2/rescue capability belongs to the fresh Stage B campaign.

**30-minute feasibility caveat:** the existing frozen d12/8169221a campaign
CLI is already built and validated, but does not contain every later small
optimization. An earlier optimized campaign CLI build took about 57 minutes.
Therefore delivery of *all* later optimizations in a new executable within
30 minutes is not guaranteed. At T0 select the shortest legitimate build path;
do not run a redundant build. If it overruns, disclose the actual state and
consume the overall reserve rather than silently substituting an older binary
or claiming an unbuilt source change is delivered. A known-good binary remains
an explicitly identified fallback, not proof that new work is complete.

## Stage B — first parallel build on `fable_5_1_parallel`

Create `fable_5_1_parallel` at the repository root from the current stable
`fable_5_1` lineage, while Stage A builds independently in its workspace-local
clone. Integrate the reviewed private Epoch slices normally, preserving their
provenance; incorporate the completed Stage A milestone later. Do not mix
unfinished code back into `fable_5_1` or disturb the Stage A source/cache.

### Minimum architecture

- Implement **bounded rolling execution**: workers can continue useful
  inspections while completed work is published. Remove dependence on a single
  global 16-job barrier without merely increasing speculative batch size.
- Share versioned immutable lookup data with bounded retained views and
  in-flight work. Do not clone the whole campaign store per worker or publication.
  Resolve the existing Store/snapshot mutation-lifetime constraint explicitly.
- Keep inspector-side successor lookups, exact positive validation and sound
  miss reuse. New proposals still need merge-time deduplication; a stale miss
  is not evidence that nothing has since been registered.
- Reuse G2 Union with exact residual coverage, inspected-scope restrictions,
  publication stamps and complete dependency edges. Preserve Off as a control.
- Integrate required-query roles and rescue/quarantine semantics, including
  legitimate new representatives of quarantined equal domains. Do not silently
  disable rescue or erase historical dependencies. An engine that still refuses
  required rescue is partial, not the completed replacement promised here.
- Preserve recorded scheduling/publication decisions, typed job lifecycle,
  graceful interrupt/RAM stops and durable same-engine checkpoint/resume.
  A fresh campaign is expected; no CP5-to-CP6 migration project.
- Reuse native monitoring. Report useful activity, queue/in-flight work,
  publication/inspection costs and checkpoint status truthfully. Leave the
  calculation and rendering of pending growth per completion unchanged.

### Adaptive ordering: deliberately bounded, but included

For this build, **ordering means pending-domain/job dispatch order in the closure
walk**, not algebraic pivot/monomial ordering that would require regenerating
the source IBP artifacts. This interpretation is explicit for approval.

- Keep the current deterministic order as baseline and add a small opt-in
  adaptive policy using existing structural/reuse and observed cost/work-growth
  information. Favor less total work, not merely more busy cores.
- Bound the candidate portfolio and update frequency; guarantee fairness so
  expensive or unattractive obligations are not permanently starved. No
  topology-name dispatch, domain clipping or silent terminal creation.
- Record choices/state needed for restart and replay. Define tie-breaking and
  admission behavior; never let the heuristic confer mathematical authority.
- Add focused tests demonstrating actual state-driven priority changes,
  bounded candidate scans/bookkeeping, starvation protection, and restoration
  of adaptive state/decisions across checkpoint and replay. A static priority
  score alone must not be labelled adaptive.
- Test on a four-loop combined control and finite/hot five-loop controls under
  the same resources. Keep it opt-in if benefit is mixed or inconclusive.
  Prior rejected support/volume policies are not re-enabled without new evidence.
- No MCTS framework, open-ended ordering search or literature campaign. This
  is the user's one explicitly permitted optimization experiment in the window.

Later, delegate a separate bounded **algebraic-pivot/input-order review** using
the existing five-loop inputs and four-loop ordering results. Identify whether
an alternative input or pivot order could improve the new campaign. This is
distinct from adaptive dispatch and does not silently regenerate artifacts or
change frozen scope. First deliver a concrete recommendation; authorize only
small relevant tests if warranted and they do not derail the core architecture.

**Input-driven experiments, not rebuild-driven experiments:** freeze one
optimized executable for the matched matrix. Family inputs, query order, worker
count, FIFO/adaptive dispatch and the existing algebraic coordinate-priority
`--permutation` are runtime inputs. Changing those does not require recompiling
Rust. A changed algebraic order requires regenerating the affected owner programs
with that executable and validating them; it cannot modify pivots inside an
existing saved program. Keep input generation cost separate and explicit, retain
the fixed baseline, and use fresh campaign bindings where inputs change. Do not
claim that every arbitrary source-row or pivot-selection heuristic is exposed:
the verified public generator option is coordinate priority. Rebuild only for
actual engine/interface corrections, never just to select another family or
already supported ordering. This clarification does not add an open-ended
generation sweep to the shortened delivery.

### Delegation and time allocation

Use up to three agents besides root, with short, concrete assignments:

| Lane | Responsibility |
| --- | --- |
| Implementation 1 | Rolling controller, immutable-view lifetime, merge/scheduling; bounded adaptive ordering after the basic path works. |
| Implementation 2 | G2/rescue/role integration across execution, checkpoint and cold loading. |
| Build/validation, then independent audit | First Stage A isolated build and compatibility checks; then rotate into a substantial cross-interface audit, tests and measurement interpretation. Never approve one's own implementation. |
| Root | Integration, edit/resource coordination, final verification, builds/pilots, progress log and delivery. |

Keep overlapping edits serialized. Reuse reviewed code and existing harnesses;
do not invent a new acceptance or provenance framework. Review narrow fixes
without repeatedly restarting a full audit from scratch. No new research.

Relative schedule after approval:

- **T0–0h30:** Stage A delivery target and user notification.
- Stage B source implementation starts concurrently; no waiting for Stage A
  compilation or the user's production actions. Heavy builds/tests remain
  resource-coordinated and never share a mutable Cargo cache.
- **To T0+2h30:** Stage B implementation and concurrent independent review;
  settle the rolling storage/view mechanism early. Flag an infeasible mechanism
  rather than spending the whole window hiding behind scaffolding.
- **Around T0+2h30–3h:** freeze integrated source; thereafter narrow correctness
  fixes only. Start the consolidated optimized build promptly, preserving at
  least the final three hours for compilation, testing and delivery. A later
  freeze explicitly spends reserve; it does not excuse missing validation.
- **Through T0+5h30:** native/CLI/Python checks, real checkpoint/replay controls,
  cold verification and matched bounded pilots, including adaptive ordering.
- **T0+5h30–6h:** final cleanup, documentation, push, frozen executable and
  fresh-campaign instructions. **T0+6h–7h is reserve**, not additional features;
  the latest user instruction permits necessary overruns for correctness.

## Acceptance, resources and fallback

- Use release/optimized binaries with matched profiles for timing. Preserve
  compiler/test distinctions; do not time debug or app-opt1 correctness builds.
- Reuse FG/BMW/H/X, combined four-loop, finite five-loop and hot-sector controls.
  Cover exact/guard/dependency behavior, G2 eligibility, stale snapshots,
  duplicate proposals, late quarantine, stop/resume and cold reload. Report
  omissions and censored runs explicitly; no all-scope claim from one fixture.
- Use existing measurements for genuinely unchanged components. For the new
  parallel engine measure whole-command time, CPU, RSS, work volume and useful
  concurrency against current G2-enabled legacy, not just LC2 with G2 off.
  The user's follow-up makes the combined four-family physics-capped control
  an explicit no-regression gate: repeated matched native-plus-cold runs must
  be faster or demonstrably on par, on identical inputs and resources. Noise
  or incomplete runs are inconclusive, not a pass. Keep the control's existing
  helper obligations; do not narrow it to improve the comparison.
  Retain the agreed matched 1.5x deployment gate and two-pair principle. A correct
  but slower or inconclusive build may be shipped as experimental, not advertised
  as a qualified faster replacement. Correctness failures block launch advice.
- Each pilot remains at most 30 minutes including preparation/restore/drain.
  Allocate sufficient shutdown time. Use representative compute widths and
  account honestly for widths not exercised.
- Respect LC2 CPUs 128–227, its memory headroom, existing build/heavy locks and
  other users. No competing benchmarks on its cores, no escalation or host-wide
  tuning. Workspace-local TMP only; current license from environment, never
  committed. Use Symbolica's existing APIs; no new CAS implementation.
- Defer NUMA tuning, elaborate compaction, extra coefficient witnesses,
  unbounded ordering research and nonessential micro-optimizations.
- If the parallel engine cannot pass in time, push the stable work and clearly
  preserved experimental branch, list the precise missing gates, and recommend
  continued Stage A operation. Do not redefine that as successful parallel
  delivery or silently drop G2/rescue to make a benchmark pass.

## User control and final deliverables

Only the user stops, resumes or launches production. Keep LC2 and its old frozen
binary/checkpoint for rollback. Stage A notification provides the compatible
upgrade; Stage B notification provides the new branch revision, executable,
exact Nix/build/Python launch command, chosen worker/ordering settings, measured
results and limitations. Campaign outputs, reference-only files and licenses
are never pushed.

Use the requested ValentinHirschi Git identity and existing coauthor convention
for every Git operation. Commit/push coherent milestones to the respective
branches. Maintain `CODEX_PROGRESS.md` after material results, failures and
decisions; no status-only busywork. Do not create another tool-managed goal or
mark the broad scoped-closure objective complete at this delivery milestone.
