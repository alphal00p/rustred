# Shared-owner finite-target campaign driver

This milestone reuses immutable saved candidate programs and verifies supplied
routes once. All requested concrete entries then share **one** Rust dependency
queue and identity set, including work reached from different owners. Python
only launches, monitors and requests cancellation; it performs no algebra.

This is not yet a complete parametric rank-bounded family solve. An empty finite
frontier is reported as `completed_finite_trace`, always alongside
`family_closure_claim=false`. Missing rules/owners remain explicit frontiers,
not new masters. Rule generation, automatic frontier feedback, coefficient
back-substitution and durable dependency-cache resume are not implemented by
this driver. The result and event journal are interruption diagnostics, not
queue checkpoints. Existing saved rule files remain reusable and unchanged.

## Running

Build the normal release `rustred` CLI, then run the standard-library Python
driver from the repository root, inheriting `SYMBOLICA_LICENSE` as necessary:

```sh
python examples/python/shared_owner_campaign.py \
  --executable target/release/rustred \
  --manifest /absolute/path/selection.json \
  --targets /absolute/path/targets.csv \
  --owner-base /absolute/path/to/workspace \
  --workers 12 --cpus 0,1,2,3,4,5,6,7,8,9,10,11
```

CPU IDs must be permitted by the process affinity. Pass all concurrent campaign
process roots with repeated `--registered-pid`, and their configured compute
workers (including builds) with `--other-workers`. The sum must not exceed 50.
Concurrent external jobs also require `--reserved-other-memory-bytes`, covering
their full intended memory allowance, not only a low initial sample. Currently
observed external RSS must fit this reservation before launch.
The supervisor measures the deduplicated registered process trees by PID/start
identity; it cannot account for unrelated unregistered jobs. It never signals
the additional registered roots. Its own PID/start and RSS/CPU are included,
and its affinity is restricted to the chosen Rust CPU set. Linux `/proc` and CPU-affinity support are
required by this example, not by the transport-neutral Rust API.
Collection reads only registered/previously observed processes and their
per-thread child lists, not every host PID. Previously observed descendants
remain tracked after reparenting. Children born and reparented between samples
before first observation can be missed; this is not universal descendant
capture. Transient unreadable live identities are retained for later retry.

Defaults are at most 50 outer workers, all native/BLAS/Rayon inner pools fixed
to one before exec, a **decimal 450 GB** measured aggregate RSS soft stop and
**500 GB** measured hard stop. RSS sampling is a supervisory threshold, not a
hard OS allocation guarantee; retain external host/cgroup headroom. The
owned multithreaded Rust process additionally receives an OS `RLIMIT_AS`
address-space ceiling before exec: hard memory minus external reservations
minus monitor headroom (`min(20 GB, hard/10)`). Thus an isolated default run
has at most **480 GB address space**. `--child-address-space-bytes` may select
a lower limit; both inherited finite soft and hard limits are preserved.
Virtual-address-space exhaustion can stop the run earlier than the RSS policy
and remains an incomplete resource outcome. This cap applies to one process,
not an arbitrary subprocess tree. Independent external jobs are not constrained
by this driver: their declared reservations plus sampled monitoring are not an
absolute aggregate-RSS guarantee if they exceed their own execution envelopes.
The 15-hour expected horizon is an objective and telemetry only, not a timeout.
There is no inherited 30-minute deadline and no fabricated dependency ETA.
Inspect backlog, completed local expansions, dedup hits, expansion bounds,
CPU utilization, memory growth and progress age before deciding to continue.
The compact bar is labelled `expanded / currently discovered`: its denominator
can grow and it is not a closure percentage. Heartbeats expose recent expanded
nodes/second and queue growth/second; Python resource records expose RSS slope.
Each resource record also includes per-PID/start CPU deltas and RSS, with the
supervisor and owned native process labelled separately. Newly observed or
temporarily unreadable processes have no CPU delta until a fresh baseline is
available; their historic CPU usage is never charged as current utilization.
Collector read/race counts make monitoring cost and incomplete reads visible.

Ctrl-C or SIGTERM to the **Python supervisor** creates a cooperative stop file.
The CLI polls it even during native preparation; core workers observe
cancellation between native operations. An individual native call may take
time to return. At the hard memory threshold the supervisor may kill only its
owned child process group, leaving a forced-stop receipt without claiming a
clean Rust result. A terminal nonzero status is incomplete, never closure.
If the supervisor itself fails (for example, resource-journal I/O fails), it
requests cancellation and reaps its own child, force-stopping after a five-second
cleanup grace if necessary. This failure cleanup is not a solve timeout.

Each invocation creates a fresh `TMP/shared-owner-campaign.*` directory with
the command/resource policy (no environment or credentials), structured events,
sampled CPU/RSS, result when available, and actual process status. There is no
overwrite or silent continuation of an old receipt. CPU measurements are
sampled deltas of live registered processes, not a complete GNU-time accounting
of short-lived children between samples.

## Inputs and API

The external selection JSON reuses the existing format:

```json
{
  "family_fingerprint": "native family fingerprint",
  "owners": [{"path": "owner.rrbin", "bytes": 1234, "mask": "111"}],
  "initial_frontier_routes": [],
  "load_limits": {"max_bundle_bytes": 1073741824}
}
```

Each nonidentity route supplies `source_mask`, `owner_mask`,
`requires_transport: true`, and square signed-integer string matrices
`source_to_representative` and `owner_to_representative`. Composition and map
verification use native Symbolica/RustRed APIs. This selection format currently
supports vacuum loop maps; no external-momentum shift is inferred. Inputs are
independent of loop count/topology names, using the existing supported arities
1–16. Identity routes require equal source/owner masks and may be omitted.
Relative owner paths use `--owner-base`, defaulting to the working directory.
Owner sizes, cumulative encoded bytes, masks, routes and target CSV are checked
before loading. Native family/policy/rank/ordering checks remain authoritative.

Optional positive integer `load_limits` keys are `max_bundle_bytes`,
`max_total_input_bytes`, `max_total_coefficient_bytes`, `max_collection_entries`,
`max_total_symbolica_state_bytes`, `max_zero_sector_visits`, and
`max_coefficient_bytes`. Unspecified fields use existing public loader defaults.
The per-file hard ceiling remains 1 GiB. All decoded programs remain resident;
encoded byte limits are not RSS predictions. JSON and target CSV each have a
16 MiB steering limit. Targets are integer-only CSV rows, without a header or
`trace,` prefix. Entry rank comes from the saved programs; intermediate rank is
not clipped.

`RoutedCampaignRequest` and `routed_campaign_with_progress` are the public Rust
application interface. `rustred routed-campaign --help` lists the CLI flags.
The CLI exposes campaign-aggregate node/rule/transport/coalescing budgets and
uses existing native per-operation defaults. An optional `--expansion-limits
LIMITS.json` (also accepted by the Python driver) overrides explicit per-call
native fields; custom Rust callers already supply the full
`RoutedCandidateLimits` and `ReductionLimits`. CLI direct invocation
requires the same six inner-pool environment variables and
`SYMBOLICA_HIDE_BANNER=1` (to keep stdout JSON-only). Python
sets them automatically and needs no rebuilt Python extension.

The expansion-policy file is a flat JSON object of positive integers, at most
16 KiB. Allowed names are `max_factors`, `max_relation_coefficient_entries`,
`max_total_power`, `max_native_polynomial_terms`,
`max_native_polynomial_operations`, `max_native_exponent_entries`,
`max_endpoints`, `max_endpoint_power_entries`, `max_retained_endpoint_key_bytes`,
`max_retained_coefficient_terms`, and
`max_retained_coefficient_clone_owned_bytes`. Missing fields keep native defaults;
unknown/duplicate fields, nulls and noninteger values are errors before loading.
For example, `{"max_native_polynomial_operations":500000000}` changes only that
per-call allowance, not aggregate work or physical memory caps. Fully resolved
values appear as `native_expansion_limits` in CLI admission and result records.
Exact-algebra policy is excluded from this transport-only option: it must agree
with the common owner context. This is resource steering, not a mathematical
rank cutoff or permission to turn an interrupted solve into a closure claim.

Interactive stderr reuses the inline, resize-aware colored terminal display;
`NO_COLOR` disables color and `--no-progress` disables only that presentation.
Structured JSONL heartbeat events continue during preparation and native work.
Native loading/preparation does not yet publish its internal counters: the
heartbeat preserves the current phase and age since its last update instead.
The CLI and Python expose `--max-input-targets` separately from `--max-nodes`;
both remain subject to the 16 MiB CSV steering bound.
`completed_nodes` means local expansion, not solved targets: descendants may
still be queued. No per-target completion is claimed before the global queue
drains. The result summarizes every frontier by support/reason with one example
per group, explicitly `frontier_details_complete=false`.

Fast supervisor tests: `python -m unittest examples/python/test_shared_owner_campaign.py`.
Native small-fixture tests live in the application shared-campaign module;
serial/parallel parity and cancellation are validated before any large run.

## First measured saved-owner control (September 21)

The same concrete rank-one input was run with all 67 saved representative
programs and 8,179 verified nonidentity routes, using the frozen release CLI
from `6c53b3a`. Both fresh processes completed successfully. Native inner pools
were one; CPU affinities were 84 and 84–89 respectively. There was no solve
deadline, with a 32 GB RSS threshold and 28.8 GB child address-space ceiling.

| Outer workers | Preparation (s) | Shared traversal (s) | Whole command wall (s) | Whole command CPU (s) | Peak RSS (GiB) |
|---:|---:|---:|---:|---:|---:|
| 1 | 198.853 | 82.645 | 288.36 | 284.71 | 5.53 |
| 6 | 151.752 | 34.973 | 190.11 | 305.85 | 5.53 |

Whole-command measurements include the Python supervisor, cold loading,
verification, reporting and shutdown. Preparation is the application total
minus its measured traversal interval; it is not parallelized by this slice.
This is one pair of shared-host observations, not a statistically controlled
scaling claim. The observed traversal ratio is **2.36×**, not sixfold. The old
native-only serial DFS control took 42.022 s for traversal, so the new one-worker
scheduler/monitoring stack regresses on this input and needs profiling. Setup
timing also varies; do not attribute the whole-command ratio to worker scaling.

Both runs have identical reported counters: 541,889 distinct integral keys,
544,228 operational nodes, 219,003 successful rule applications, 210,751
transports, 321 declared terminals, 111,814 zeros, 7,458,443 deduplication hits,
and no missing owner/rule. Maximum reached rank is five and dot excess ten.
Small-fixture tests compare exact terminal/frontier sets; the large CLI summary
records counters, not a persisted full graph or full terminal-set comparison.
The one-second sampled queue peaks are 46,307 and 41,028; they are not exact
high-water marks. No new IBPs were generated and no parametric coverage claim
follows from this finite control.

Evidence: `TMP/shared-campaign-controls.olWsx7/`, with native/frontend gates in
`TMP/shared-routed-core.p87lO3/` and `TMP/shared-campaign-frontend.TLRUXi/`.

### Corrected matched controls

The `59d0ab2` release executable, native polynomial-power fix and rooted process
monitor were rerun with the identical inputs, limits and affinities. Both
processes completed with all the counters above unchanged, including zero
missing owners/rules. Before/after input hashes and independent review pass.

| Outer workers | Preparation (s) | Shared traversal (s) | Whole command wall (s) | Whole command CPU (s) | Peak RSS (KiB) |
|---:|---:|---:|---:|---:|---:|
| 1 | 103.550 | 28.253 | 136.11 | 133.99 | 5,805,364 |
| 6 | 103.989 | 12.181 | 120.08 | 168.97 | 5,807,888 |

The observed traversal ratio is **2.32×** for six versus one worker. Relative to
the earlier shared implementation, traversal is 2.93×/2.87× faster respectively;
several changes were combined, so this does not isolate any single optimization.
Preparation remains serial and dominates these small controls. These are single
shared-host observations, not confidence-qualified scaling results or an R10
closure timing. Equal large-run counters do not replace exact terminal-set
comparisons in the focused tests. The corrected joint pressure run is separate.

Evidence: `TMP/shared-campaign-corrected-controls.9WwlVA/`, particularly
`INDEPENDENT_RESULTS_AUDIT.md` and its two complete process receipts.

## Directed owner search and shared rule installation

The Rust core now retains common ordinary/LI source definitions alongside the
saved owner programs. `CandidateOwnerPrograms::bind_owner_search` binds a new
search to the admitted family, owner and original ordering. The caller supplies
an explicit prospective `OwnerFeedbackPolicy`; absent historical settings are
not guessed. `BoundOwnerSearch::solve_domains_with_observer` solves nominated
native `Case` domains and returns a distinct `BoundOwnerOverlay`, not a complete
sector or closed-family artifact.

The actual `OwnerDomainScope` is separate from the campaign entry rank: an R11
successor discovered from an R10 input remains R11 (or explicitly unbounded),
never clipped to ten. Positive indices can remain symbolic. Unsupported cases
and resource limits return errors, not invented masters.

`append_domain_overlays` atomically publishes successful results in the caller's
deterministic order. Unchanged owners, sources and base formula batches are
Arc-shared. Dispatch tries the original terminal/rules before each later batch;
a new terminal cannot shadow an older applicable formula. Only a true uncovered
result advances to the next batch—arithmetic, descent or resource errors do not.
`RoutedCandidateReducer::with_programs` reuses verified route handles for an
append-only snapshot from the same lineage, avoiding another family load or map
verification. Existing graph work is not persisted or reused by that rebind.

Cumulative overlay limits cover retained domains, rules, RHS entries, terminals
and measured native payload. The census excludes allocator/native temporary
overhead and hidden matrix spare capacity; it is not a replacement for OS/RSS
resource enforcement. Searches currently share source definitions, not each
sector's preconditioned basis. This core API does not yet give the CLI/Python
driver automatic nomination, durable overlay loading, or a complete R10 solve.

The complete corrected release gate passes 2,533 core tests, with zero failures
and 32 pre-existing ignored diagnostics; app/integration tests pass 261 and the
Python supervisor passes ten. Fourteen focused tests cover real source search,
above-entry-rank domains, immutable reuse, priority, ordering, limits and native
chart storage. A new test initially compared opposite ordering conventions; its
expected direction was corrected after independent review, with all pairwise
assertions retained. No production ordering changed to satisfy that test.

The native dependency correction and affected-map timings are documented in
[the pressure report](research/shared_rank10_pressure_2026-09-21.md). Core receipts
are in `TMP/source-bound-overlay-corrected.3DtdEI/`; the corrected frontend build
and runtime are in `TMP/shared-campaign-corrected-frontend.oAhpcl/` and
`TMP/shared-campaign-corrected-runtime.oMmXaO/`. These gates do not turn the
previous stopped 50-worker diagnostic into a completed campaign.

### Retained application feedback and batched scheduling

The opt-in Rust application `RoutedFeedbackSession` now retains successful
source-derived overlays between rounds. It traces first, nominates only genuine
missing-rule domains, leaves positive powers symbolic, and republishes through
the existing shared-owner interface. The [feedback boundary](owner_source_feedback.md)
documents admission, cancellation, partial failure and result ownership. This
service is not yet automatically enabled by the trace-only CLI/Python driver;
resource exhaustion never silently launches source search.

The scheduler publishes up to 256 validated children per lock acquisition and
deduplicates both operational phases through one full-integral-key membership
index. Hash iteration does not select work or determine result ordering. Native
arithmetic, per-edge descent, and exact key equality remain authoritative. The
core release suite passes 2,545 tests (32 existing ignored); all 276 application
and integration tests and ten Python supervisor tests pass. Independent source
and runtime reviews accompany the slice; new measured controls are pending.
Do not infer a performance
improvement or parametric coverage from these implementation changes alone.

The first new frontend gate caught an input-boundary defect: Serde's default
struct visitor accepted an empty positional array as the named budget object.
The production reader now explicitly requires an object and retains direct
duplicate-field rejection; the original failing assertion is unchanged and a
full positional-array regression was added. The corrected full release gate is
`TMP/shared-campaign-feedback-app-fixed.DEwkOr/`; the core runtime gate is
`TMP/shared-campaign-batched-core.EWNFkj/`. Its initial build-log parser failed
after compilation; the existing frozen binary subsequently passed every test.
