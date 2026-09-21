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
uses existing native per-operation defaults; custom Rust callers may supply
the full `RoutedCandidateLimits` and `ReductionLimits`. CLI direct invocation
requires the same six inner-pool environment variables and
`SYMBOLICA_HIDE_BANNER=1` (to keep stdout JSON-only). Python
sets them automatically and needs no rebuilt Python extension.

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
