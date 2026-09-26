# Shared-owner campaign driver

This driver reuses immutable saved candidate programs and verifies supplied
routes once. Choose concrete `--targets` or symbolic `--queries`; inputs from
different owners share one native dependency worklist. Python only launches,
monitors and requests cancellation; it performs no algebra. Most historical
measurements below concern the concrete-target mode and must not be relabelled
as parametric-family timings.

This is not yet a complete parametric rank-bounded family solve. An empty finite
frontier in concrete mode is reported as `completed_finite_trace`, alongside
`family_closure_claim=false`. Missing rules/owners remain explicit frontiers,
not new masters. Rule generation, automatic frontier feedback, coefficient
back-substitution are not implemented by this driver. Symbolic successor walks
now support native queue checkpoints; concrete traces do not. Results, event
journals and Python status files are monitoring receipts, never resume
authority. Existing saved rule files remain reusable and unchanged.

Symbolic mode invokes `owner-domain-match --follow-successors`, preserving
unbounded positive powers and each query's actual numerator rank. It reports
`recursive_worklist_exhausted` and `all_scheduled_domains_resolved` rather than
a finite-target completion flag. It also keeps `family_closure_claim=false`.
See [the domain-matching interface](shared_owner_domain_matching.md) for the
conservative successor semantics and explicit unresolved obligations.

## Running

### Dependency-closure monitoring: fresh campaign format

The native shared walker now retains the dependency graph behind its progress
counts. The progress bar shows **initial domains recursively closed**, while
initial publication remains a separate `x / total` text counter. The `Domains`
line reports all discovered domains, the recursively closed count and unresolved
count. These are different from `Queue ... pending` and local completions:
a locally published domain can still depend on unfinished descendants.
Counts cover the supplied domain obligations; they are not a termination or
unrestricted-family certificate. Updates are conservative periodic snapshots,
not a completion-time estimate. See the [design and validation record](research/dependency_closure_monitoring_2026-09-25.md).

This requires complete dependency history from the beginning. New native
checkpoints use the sectioned CP5 format (`latest.json` manifest with schema 5
and `format: "RUSTRED-WALK-CP5"` over per-generation `meta`/`nodes`/`ledger`/
`index` files and append-only `domains`/`edges`/`records` segments, every
file length- and blake3-verified before decoding); old CP1-CP4 checkpoints
cannot resume under the new executable and are refused with a fresh-campaign
message. Resume is bound to the request/policy digest, the owner digests and
the executable's `WALK_SEMANTICS_VERSION`; a rebuilt executable with the same
semantics version resumes (the manifest records both digests and emits
`checkpoint_executable_changed`), a different semantics version is refused.
An old run viewed with the updated monitor reports recursive closure as
**unknown**, never substitutes its publication counter into the bar. See the
[CP5 checkpoint record](research/five_loop_checkpoint_cp5_2026-09-26.md).

At the user's request, the previous local campaigns have been removed from the
active `campaigns/` directory and retained in the recovery archive
`TMP/retired-campaigns-20260925.UtI4ay/`. The old helper-first rule inputs are
unchanged and can seed a fresh campaign without regenerating IBPs. To prepare
and start a new run with ten workers and a requested 750 GB RAM ceiling:

```sh
mkdir -p TMP
export TMPDIR="$PWD/TMP" TMP="$PWD/TMP" TEMP="$PWD/TMP"
export CARGO_HOME="$PWD/TMP/cargo-home" CARGO_TARGET_DIR="$PWD/target"
nix develop --command cargo build --release --locked -p rustred-app --bin rustred -j 12
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --prepare-from TMP/retired-campaigns-20260925.UtI4ay/five-loop-saved-helpers-first \
  --campaign-directory campaigns/five-loop-dependency-closure \
  --executable target/release/rustred \
  --workers 10 --max-memory-bytes 750000000000 --start
```

Omit `--start` to prepare only. If that destination is already prepared, omit
`--prepare-from`, `--executable` and the frozen-policy options on the start
command. A later checkpointed resume uses that destination with
`--resume --start`. The default guard requests save-and-stop at 712.5 GB;
host headroom can reduce the admitted ceiling. Worker count is frozen for
the checkpoint; ten workers are an explicit resource choice, not a demonstrated
speed optimum. The default worker split is five inspectors, four admission
helpers and one coordinator.

In this workspace the destination above is already prepared with the validated
release executable, ten workers on CPUs 0–9, unchanged 67 owners/134 helper-first
queries and the 750 GB request. It has **not** been launched. Start it with:

```sh
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/five-loop-dependency-closure --start
```

Use `--resume --start` instead after its first clean pause. No recompilation or
rule regeneration is needed for this prepared snapshot.

### Recommended fresh attempt: existing helpers first

The September 25 input-only control supports a conservative fresh-run preset:
retain the shared queue and all existing helper bounds, but admit each owner's
helpers before its narrower original requests. Generic staging keeps first-seen
owner order; it does not sort by topology name, loop count or support size.
The `owner-anchor-` ID convention identifies staging-generated helpers for
priority only. Native exact containment still decides whether a query can reuse
an admission; the Python ordering is not mathematical authority.

The completed four-loop FG control took 18.53 s with the original order and
16.44 s with helpers first. Broader rank helpers did not improve on reordered
existing helpers. These are local four-loop measurements, not an established
five-loop speedup. See the [experiment and limitations](research/helper_first_campaign_2026-09-25.md).
Joint support pruning and independent-topology scheduling remain off here;
their completed controls did not establish an end-to-end benefit.

From the repository root, prepare a **new** campaign without launching anything:

```sh
mkdir -p TMP
export TMPDIR="$PWD/TMP" TMP="$PWD/TMP" TEMP="$PWD/TMP"
export CARGO_HOME="$PWD/TMP/cargo-home" CARGO_TARGET_DIR="$PWD/target"
# Build only if the current release executable is not already available.
nix develop --command cargo build --release --locked -p rustred-app --bin rustred -j 16
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --prepare-from campaigns/five-loop-saved-coarse-cover \
  --campaign-directory campaigns/five-loop-saved-helpers-first \
  --executable target/release/rustred \
  --workers 50 --max-memory-bytes 500000000000 \
  --checkpoint-interval-seconds 3600
```

`--prepare-from` accepts an existing staged campaign directory, verifies its
immutable inputs and copies them to a disjoint, nonexistent destination.
It defaults to `--query-order helpers-first`; `--query-order preserve` is an
explicit control. Adding `--queries NEW.json` keeps the copied owner payloads
and selection but stages the supplied query document instead, after verifying
it (schema `rustred.owner-domain-queries.json.v2`, every owner mask present in
the selection, exactly the six native row fields `id`, `owner`, `lower`,
`upper`, `max_numerator_rank`, `power_bounds`, unique ids); the default order
is then `preserve`. `--attach FILE` (repeatable) copies planner receipts such
as `entry-plan-receipt.json` read-only into `inputs/`; the input receipt lists
each attachment's name, size and SHA-256 and the printed plan records
`entry_plan_receipt` when that file is attached. Attachments are opaque data,
never solver input. It preserves every query object/ID/bound and program payload,
saves the input query bytes as `queries-original.json` when reordering, and
records the ordering in the input receipt and displayed launch plan.
The present five-loop input remains 67 owners and 134 explicit requests.
This operation neither regenerates IBPs nor imports the source checkpoint.

Preparation freezes the executable and policy. These directory names describe
the earlier helper-first experiment, now retired in this workspace; use the
dependency-closure recipe above for the new format. For an independently
prepared campaign, omit preparation and launch with:

```sh
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/five-loop-saved-helpers-first --start
```

This begins from the reordered initial requests, not the old run's checkpoint.
Do not run both campaigns on the same reserved CPUs/RAM at once. The preset
requests 50 workers, a 500 GB RAM ceiling with a 5% cooperative checkpoint margin,
hourly checkpoints and uncapped cumulative work. The RAM ceiling has no fixed
500 GB maximum: choose a different positive byte count during fresh preparation,
or use the documented per-resume RAM override below. Ctrl-C requests a clean
checkpoint; resume this new campaign with its own `--resume --start`.
Preparation never rewrites its source campaign or checkpoints.

### Existing prepared campaigns

For the prepared five-loop saved-owner input, use the production launcher from
the repository root. It verifies the immutable input hashes and, on the first
preparation/start, copies the supplied executable into the campaign directory.
The original steering policy is frozen beside that executable. The user starts
the full run manually; building or displaying help does not launch it:

```sh
mkdir -p TMP
export TMPDIR="$PWD/TMP" TMP="$PWD/TMP" TEMP="$PWD/TMP"
export CARGO_HOME="$PWD/TMP/cargo-home" CARGO_TARGET_DIR="$PWD/target"
nix develop --command cargo build --release --locked -p rustred-app --bin rustred -j 16
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/five-loop-saved \
  --executable target/release/rustred --start
```

Omit `--start` to prepare and print the exact command without executing it.
This still freezes the executable and policy. The default policy is 50 workers
(at most 256 and never more than the permitted CPU affinity; `--cpus` accepts
comma lists and ranges such as `128-177` or `0-3,8`), a requested 500 GB
decimal resident-memory ceiling, a 5% RAM guard margin, hourly checkpoints,
Ready publication, H256 unreserved transfer, the native inspector split,
exact initial-D reuse, finite-axis refinement and degree-64 guard admission.
The frozen `steering.json` is `rustred.production-steering.v2` and records
`publication_policy`, `transfer_unreserved_lookahead`, `inspection_workers`
and `checkpoint_interval_seconds` beside the earlier options; the supervisor
command is built from those options, and `--inspection-workers N` is added
only when frozen. v1 steering files remain readable with their recorded
values (Ordered, lookahead 256, native split).
Cumulative enumeration work is uncapped (`--unbounded-work`); input admission,
bounded worker buffers, native scratch and per-operation algebra safeguards
remain explicit. Physical subdivision is optional, with the paired
`--apply-subdivision-axis N --apply-subdivision-cut C`; it is not a default
whole-walker speed claim. The saved input remains data, not topology dispatch.

Ready publication (`--publication-policy ready`, the default for campaigns
prepared with the v2 steering) lets completed or partially ready sources
publish without waiting for an earlier slow source, including within one
owner. It still uses shared admission and bounded outstanding-work credits
(H256 in the production preset), requires unreserved-transfer scheduling and
rejects physical subdivision. New runs use CP5 checkpoints whose manifest
binds the publication policy, so an Ordered checkpoint refuses a Ready resume
as a policy change. `--publication-policy ordered` remains selectable and is
required for `--apply-subdivision-axis/--cut`. Choose the policy only when
preparing a **new** campaign; resume refuses a different policy, lookahead,
inspector count, worker count, CPU set or checkpoint interval than the frozen
one (only the RAM overrides may differ), and never swaps the frozen executable
for one with a different walk semantics version.
See [the implementation and validation record](research/five_loop_ready_publication_2026-09-24.md)
for the measurement boundaries.

After a graceful pause, resume with the same immutable binary and frozen solver
flags automatically. Each invocation creates a new receipt directory:

```sh
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/five-loop-saved --resume --start
nix develop --command python examples/python/campaign_monitor.py \
  campaigns/five-loop-saved/runs/RECEIPT_DIRECTORY
```

RAM policy is not mathematical checkpoint state. On a later resume, you may
override `--max-memory-bytes` and/or `--ram-guard-margin-percent` without changing
the frozen executable, native solver arguments or original `steering.json`:

```sh
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/five-loop-saved --resume \
  --max-memory-bytes 700000000000 --start
```

This requests 700 GB, subject to current host/cgroup headroom. It does not alter
an already-running process: first request Ctrl-C and wait for its saved pause.
These RAM overrides apply only to this invocation; repeat them on a subsequent
production resume, or use the supervisor's emitted exact resume command.
Omitting them reuses the original frozen RAM defaults. Prepared/active-run plans
record the original policy and requested RAM overrides separately; supervisor
receipts record both requested and host-admitted effective limits.

`campaigns/five-loop-saved/active-run.json` points to the latest requested run.
The launcher still rejects conflicting native policy overrides and a different executable
(except a semantics-compatible `--upgrade-executable`, below); changing native
policy requires a separate campaign. A later Cargo rebuild does not replace the
frozen executable. `campaign_monitor.py RUN --json` gives one
read-only machine-readable status; `--once` prints one human-readable snapshot.
The flake also exposes `campaign-production`, `campaign-monitor`,
`campaign-stage`, and `campaign` apps. The tested development environment uses
Python 3.11.15, Rust 1.97.1 and Cargo 1.97.0.

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

To supervise the symbolic campaign with the same resource policy:

```sh
python examples/python/shared_owner_campaign.py \
  --executable target/release/rustred \
  --manifest /absolute/path/selection.json \
  --queries /absolute/path/queries.json \
  --owner-base /absolute/path/to/workspace \
  --workers 6 --cpus 0,1,2,3,4,5 \
  --route-domain-overcover --unbounded-work \
  --checkpoint /absolute/path/to/new-checkpoint-directory
```

Finite diagnostic work allowances remain opt-in and cannot be combined with
`--unbounded-work`. Without that flag, native work defaults apply.
Aggregate symbolic containment comparisons are unlimited by default; pass
`--max-containment-checks unlimited` explicitly or a positive integer for an
opt-in finite diagnostic budget. Both Python steering paths forward this policy
without changing rank, native-work, storage or supervisory resource limits.
`--max-frontiers` independently bounds retained diagnostic records in bounded
diagnostic mode; production unbounded work removes that stop too. Concrete-only work
flags and `--expansion-limits` are rejected with `--queries`; symbolic-only
flags are rejected with `--targets`. The two input flags are mutually exclusive.

CPU IDs must be permitted by the process affinity; `--cpus` accepts comma
lists and ranges (`128-177`, `0-3,8`) and must name exactly `--workers` IDs.
Pass all concurrent campaign process roots with repeated `--registered-pid`,
and their configured compute workers (including builds) with `--other-workers`.
The sum must not exceed 256 or the number of permitted CPUs, whichever is
smaller.
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

Defaults are 50 outer workers (at most 256, bounded by the permitted CPUs),
all native/BLAS/Rayon inner pools fixed to one before exec, and a **500 GB decimal default** requested aggregate RSS ceiling.
`--max-memory-bytes` accepts any positive byte count, including a higher requested
ceiling such as 700 GB; there is no fixed numerical RAM maximum. Admission reduces it if
host/cgroup available RAM minus the host reserve is smaller. The reserve
defaults to `min(20 GB, 5% of host/cgroup capacity)`; readable cgroup-v2 ancestor
limits are included. By default measured RSS at **95% of the effective ceiling**
requests a checkpoint and stop. Set `--ram-guard-margin-percent` to change that
margin or `--soft-memory-bytes` for an earlier stop. Thus an otherwise
unconstrained 500 GB run requests a save at 475 GB, or a 700 GB run at 665 GB.
Margins must leave a representable positive soft limit strictly below the
effective hard limit. Host pressure can trigger an
earlier stop, independently of campaign RSS.

No new `RLIMIT_AS` address-space cap is imposed by default: virtual reservation
is not consumed RAM and must not preempt the graceful resident-memory guard.
Inherited limits are recorded honestly. `--child-address-space-bytes` is an
explicit diagnostic opt-in, preserving any tighter inherited limit; its
exhaustion remains incomplete. RSS sampling is not an OS allocation guarantee:
growth between samples or during checkpointing may overshoot the threshold.
The hard RSS/host-emergency safeguard can kill only the owned child group;
the last completed checkpoint remains the recovery point. External registered
jobs are measured but never signalled, and must respect their declared budgets.
The 15-hour expected horizon is an objective and telemetry only, not a timeout.

For symbolic successor walks (`--queries`), optional `--inspection-workers I`
partitions the same total worker budget rather than adding another pool. At
`--workers 50`, I=40 requests 40 native inspectors, nine admission helpers and
one coordinator. Omission preserves the existing automatic split. A finite
containment-comparison cap requires zero helpers; invalid partitions fail before
launch. Requested and effective reservations are recorded separately from actual
activity. See [the native partition contract](shared_owner_domain_matching.md#bounded-parallel-inspection).

There is no inherited 30-minute deadline and no fabricated dependency ETA.
Inspect backlog, completed local expansions, dedup hits, expansion bounds,
CPU utilization, memory growth and progress age before deciding to continue.
The colored TTY dashboard reserves its progress bar for initial domains whose
recorded dependencies have recursively closed. Local initial publication and
native inspection are separate plain counters; publication may include
delegated entries. All-domain discovered/closed/unresolved counts distinguish
recursive coverage from local processing. Descendant work has no fixed final
denominator or fabricated ETA. Missing dependency history uses an indeterminate
bar, not a publication fallback. Sampled actual native CPU occupancy and blocked/active slots are separate
from reserved inspector/admission/coordinator workers. `NO_COLOR` suppresses
color; redirected output is low-rate plain text, including checkpoint status.
Resource records expose local completion rates and RSS slope.
`status.json` additionally carries a `derived` block (schema string unchanged;
the block is additive) computed from a bounded deque of the last two hours of
native heartbeats: `completions_per_hour_1h`, `stall_share_5s` and
`stall_share_20s` (fraction of wall time in heartbeat intervals of at least
5 s or 20 s with zero completion delta), `pending_growth_per_completion_1h`,
`rss_bytes_per_discovered_domain`, `coordinator_duty_1h` (delta of
preparation plus ordered-commit wall over delta wall), `checkpoint_duty`
(sum of save durations over elapsed), `computing_inspectors_mean_1h` (null
until the native heartbeat reports `computing_workers`),
`max_scheduled_finite_rank`, `roots_closed`, `roots_total` and
`last_checkpoint` (generation, bytes, duration). The dashboard shows them on
the `Inspectors`, `Rate` and `Checkpoint gen` lines, `unknown` when absent.
These are measured deltas, not estimates: nothing in the status or dashboard
is an ETA. `examples/python/heartbeat_metrics.py EVENTS.jsonl [--start S
--end E --window W]` recomputes the same numbers offline for any elapsed
window of any `events.jsonl`, tolerating a partially written last line.
Each resource record also includes per-PID/start CPU deltas and RSS, with the
supervisor and owned native process labelled separately. Newly observed or
temporarily unreadable processes have no CPU delta until a fresh baseline is
available; their historic CPU usage is never charged as current utilization.
Collector read/race counts make monitoring cost and incomplete reads visible.

Ctrl-C or SIGTERM to the **Python supervisor** creates a cooperative stop file.
The CLI polls it even during native preparation; core workers observe
cancellation between native operations. An individual native call may take
time to return. There is no graceful-stop timeout: wait for the saved checkpoint
and terminal `paused` receipt (native exit 4). Both the terminal and durable
JSON print the checkpoint path and an exact resume command with fresh receipt
paths. Periodic saves default to once per hour and continue the run; monitor
events identify writing/start time, completion time, duration and generation.
Bootstrap checkpoints restart preparation and are labelled as such, not as
completed computational work. A save in progress never replaces the last good
durable generation. At the hard memory threshold the supervisor may kill only its
owned child process group, leaving a forced-stop receipt without claiming a
clean Rust result. A terminal nonzero status is incomplete, never closure.
If the supervisor itself fails (for example, resource-journal I/O fails), it
requests cancellation and reaps its own child, force-stopping after a five-second
cleanup grace if necessary. This failure cleanup is not a solve timeout.

Each invocation creates a fresh `TMP/shared-owner-campaign.*` directory (or
the explicit new `--run-directory`; production uses persistent `campaigns/.../runs`)
with
the command/resource policy (no environment or credentials), structured events,
sampled CPU/RSS, result when available, and actual process status. Atomic
`status.json` and `processes.json` retain heartbeat, boot ID and PID/start
identities. The read-only monitor verifies those identities and warns about
stale/dead-supervisor snapshots instead of claiming current activity. There is no
overwrite or silent continuation of an old receipt. CPU measurements are
sampled deltas of live registered processes, not a complete GNU-time accounting
of short-lived children between samples.

### Resuming onto a semantics-compatible binary

A paused CP5 campaign may continue on a newer, performance-only executable
whose walk semantics equal the checkpoint's. The native store accepts a
different executable digest with the same `WALK_SEMANTICS_VERSION` (emitting
`checkpoint_executable_changed`) and refuses a different version before
touching the checkpoint directory. The launcher checks the same condition
first with the read-only probe `rustred walk-semantics-version`, which prints
`{"walk_semantics_version":1,"checkpoint_format":"RUSTRED-WALK-CP5","checkpoint_schema":5}`.

Pause first: Ctrl-C (or SIGTERM) to the **Python supervisor** requests the
cooperative save; wait for the saved checkpoint and the terminal `paused`
receipt (native exit 4). Never kill the native process. Then build the new
binary and run the read-only dry run:

```sh
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/CAMPAIGN --resume \
  --upgrade-executable target/release/rustred
```

It changes nothing and prints the frozen and new SHA-256 digests, the
checkpoint's walk semantics version (with generation and kind), the new
executable's probe result, any evidence that the active run is still alive
and the exact supervisor command it would launch (`--json` prints the full
plan with an `executable_upgrade` block). Apply and resume with:

```sh
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/CAMPAIGN --resume \
  --upgrade-executable target/release/rustred --start
```

With `--start` the launcher refuses a live run (the `processes.json` and
`status.json` PID/start-time identities of the run named by `active-run.json`,
or its `run.pid` before those exist) and holds the native `checkpoint.lock`
while it freezes the new bytes as `bin/rustred-<sha256>` (mode 0555, synced,
digest re-checked; the old binary is kept), re-probes that frozen copy,
rewrites `bin/steering.json` so that only the `--executable` value changes
(plus an `executable_upgrades` note; mode 0444 again) and finally commits
`bin/executable.json` as the new receipt with a `history` list of the replaced
receipts (`replaced_unix_time`, `walk_semantics_version`,
`reason: "upgrade_executable"`). It then resumes exactly like
`--resume --start`; RAM overrides combine as usual and every other frozen
option is unchanged. Later plain `--resume --start` invocations use the
upgraded binary, and the native run reports `checkpoint_executable_changed`.

Refusals exit 2 and change nothing:

- `--upgrade-executable` without `--resume`, or with `--prepare-from` or `--executable`;
- the new binary has the frozen digest (plain `--resume` suffices);
- `checkpoints/main/latest.json` is missing, or is not `RUSTRED-WALK-CP5`
  schema 5 of kind `state` or `bootstrap`;
- the new binary has no `walk-semantics-version` probe (every binary built
  before the probe, including the frozen `rustred-102adcc3…`), or the probe
  fails, exceeds 60 s or 64 KiB, or does not print one JSON object;
- a different checkpoint format/schema or walk semantics version: start a new campaign;
- with `--start`: a live supervisor or native process, or a held `checkpoint.lock`.

If an upgrade is interrupted after the steering rewrite but before the receipt
commit, a plain `--resume` refuses because steering and receipt name different
executables; rerunning the same `--upgrade-executable NEW --start` completes it.

## Profiling controls and walk audits

`examples/python/walk_control_matrix.py MATRIX.json --output DIR [--audit]
[--dry-run] [--case NAME] [--skip-existing]` runs matched controls
sequentially through this supervisor under `nice -n 5 taskset -c CPUS`. The
matrix (`rustred.walk-control-matrix.json.v1`) lists cases with `name`,
`executable`, `manifest`, `queries`, `owner_base`, `workers`, `cpus` (for
example `"192-197"`), `publication_policy`, `inspection_workers` (or null),
`native_options` (extra supervisor arguments), `max_memory_bytes` and
`checkpoint_interval_seconds`; optional `transfer_unreserved_lookahead`
(256) and `ram_guard_margin_percent` (5). Every case is validated, including
that its CPUs lie inside the harness's own affinity mask, before anything
runs. Per case it writes `command.json`, the supervisor `run/` directory and
`summary.json`: whole-command wall, user/system CPU and max RSS from `wait4`
of the supervisor (which includes its waited-for native child), the native
report's `prepared_seconds`/`traversal_seconds`/`elapsed_seconds`, native
inspections (Apply/Route from `completed_nodes` and `routed_domains`),
aliases (`scheduled_nodes - completed_nodes`), events, max scheduled finite
rank, checkpoint save seconds and the last generation from `events.jsonl`,
peak sampled RSS from `resources.jsonl`, the supervisor receipt and exit
status. Large reports are scanned head and tail for their top-level scalars
rather than parsed whole. `RESULTS.md` tabulates the cases and
`matrix-receipt.json` records executable, manifest and query SHA-256 digests.
These are single-run measurements on the stated CPUs, not portable timings.

`examples/python/audit_owner_domain_walk.py RUN [--queries Q] [--command
ARGV.json] [--supervisor-receipt R] [--expect-schema S]` streams `result.json`
once with bounded memory (record ids, owner/phase, kinds and dependency links
in arrays) and writes `audit.json`: every alias resolves to a same-phase,
same-owner completed native representative; Apply statistics have zero
problems and unsupported-support successors and consistent successor sums;
Route statistics have zero missing routes and consistent event accounting;
queue, ledger and worker pool are drained; frontiers are zero; initial-entry
and partial-anchor obligations are discharged; the input queries are
preserved verbatim; ordered records are in order or ready records sum their
accepted events to the committed watermark; the durable checkpoint manifest
matches the report; the resource receipt shows a clean exit. Violations are
listed and the exit status is nonzero. The audit checks recorded completion
and explicit dependencies only, never IBP identities or family termination.

`examples/python/compare_walk_records.py --mode strict|multiset A.json B.json`
compares two reports while streaming both. Strict mode (Ordered, old versus
new binary) requires identical completed-record geometry, native/guard/
dependency counters and outcomes, ignoring only timing fields, checkpoint
bookkeeping and scheduling diagnostics (`--ignore-top`/`--ignore-record`
extend the list explicitly). Multiset mode (Ready or cross-policy) requires
equal multisets of `(phase, owner, lower, upper, rank, power_bounds, outcome)`
and native inspection counts within `--native-tolerance`; `--shape` chooses
the outcome component (`discharged`, the default, is independent of whether a
domain was inspected natively or delegated; `kind` adds the record kind for
same-policy runs; `geometry` drops it). A difference is a nonzero exit;
equality is not a closure claim. On the September 26 FG baselines the
Ordered walk and its repeat compare strictly identical (98,909 records, 0
differences), whereas Ready records 98,881 logical domains, so the
cross-policy multisets differ; native counts (98,869 versus 98,841) lie
within 0.03%.

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
`trace,` prefix. By default entry rank comes from the saved programs;
intermediate rank is never clipped.

### Explicit finite starting domains

For concrete targets, the CLI and Python supervisor accept
`--entry-domains DOMAINS.json`; Rust callers set
`RoutedCampaignRequest::entry_domains_json`. This reuses the
`rustred.owner-domain-queries.json.v2` format described in
[domain matching](shared_owner_domain_matching.md), but every region must be
nonempty and provably finite. At most 10,000 regions and 1 MiB of JSON are
accepted. Coordinates are `n_i-1` on active axes and `-n_i` on inactive axes.
Coordinate bounds are intersected with rank and A/R/D predicates; a finite
aggregate bound can make a region finite even when individual upper bounds
are `null`. Multiple regions remain a union, including its holes.

The supplied domain **replaces only the initial-root rank admission policy**.
It does not rewrite saved generation scope, assert additional rule coverage,
or restrict descendants. For example, an explicit R15 finite request may use
R10-origin saved rules when their actual applicability permits it; uncovered
targets still produce native missing-rule frontiers. Selected source supports
can use verified routes without a literal owner. Missing routes/owners and
source-condition failures retain their usual outcomes.

This option does not enumerate all roots in the domain: `--targets` is still
required. Reports retain `entry_admission`,
`saved_generation_max_numerator_rank` and the actual submitted-target count,
with no exhaustive-domain claim. Symbolic `--queries` mode rejects the option.
Retained Rust feedback sessions preserve the same explicit policy across batch
replacement, new overlays and retracing. Root replacement rejects a whole
invalid batch atomically; descendants and searched terminals can lie outside
the entry domain.

The native interfaces are `FiniteRootAdmission<N>` with
`CandidateEntryAdmission::ExplicitFinite`,
`trace_targets_with_entry_admission`, and
`trace_targets_parallel_with_entry_admission_and_observer`. Existing trace
methods retain their saved-generation-scope default.

### Resource controls

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

### Batched-scheduler matched controls

The release executable from `6d01349` completes the same controls with the same
67 owners, 8,179 verified transports, targets, affinities and resource limits.
All reported actual graph/rule/terminal/frontier counters match both previous
corrected runs. The tighter prospective resource envelopes intentionally change
from 35,598,393 to 35,437,782 operations and from 8,557,491 to 7,700,656 endpoints;
these are not measured monomial or completed-integral counts.

| Outer workers | Preparation (s) | Shared traversal (s) | Whole command wall (s) | Whole command CPU (s) | Peak RSS (KiB) |
|---:|---:|---:|---:|---:|---:|
| 1 | 101.619 | 24.690 | 130.11 | 128.45 | 5,804,908 |
| 6 | 102.837 | 11.538 | 118.09 | 141.71 | 5,804,748 |

These are completed single shared-host observations, without profiling or an
elapsed deadline. The observed traversal ratio is 2.14×; compared with the prior
corrected controls, serial/six-worker traversal decreases by about 12.6%/5.3%.
Do not infer a statistically isolated optimization effect or extrapolate these
small controls to a full R10 solve. The finite graph contains 541,889 keys and
321 declared terminals, with no missing owner/rule. Exact full terminal sets are
compared in small regressions, not persisted by these large summary receipts.

Evidence: `TMP/shared-campaign-batched-controls.BKNOrt/`, with complete process
receipts `TMP/shared-owner-campaign.gziecd_7/` and
`TMP/shared-owner-campaign.90p94uc7/`. The following 50-worker pressure run uses
the same default per-call limits and is a distinct workload.

### Native-support visitor matched controls

The trace-only visitor avoids constructing contextual coefficient wrappers and
the full endpoint vector that dependency traversal immediately discarded. It
still performs the same exact native polynomial arithmetic and cancellation,
checks every virtual output/storage budget before exposing keys, and preserves
sorted endpoint order. The coefficient-returning reduction API is unchanged.
The coherent release gates passed 2,554 core tests (32 existing ignored) and
276 application/integration tests, including all nine new visitor regressions.

The same 67-owner rank-one controls completed with all 16 reported graph fields
**and both prospective counters** equal to the batched-scheduler baseline:
35,437,782 projected operations and 7,700,656 projected endpoints. Both runs
again reached 541,889 physical keys and 321 declared terminals with no missing
owner/rule, pending work, failure or operator stop.

| Outer workers | Preparation (s) | Shared traversal (s) | Whole command wall (s) | Whole command CPU (s) | Peak RSS (KiB) |
|---:|---:|---:|---:|---:|---:|
| 1 | 102.466 | 20.437 | 126.11 | 125.12 | 5,805,236 |
| 6 | 100.470 | 8.067 | 112.08 | 142.40 | 5,804,864 |

Previous traversal times were 24.690/11.538 seconds. The observed one-to-six
worker traversal ratio is 2.53×, not sixfold. These are single shared-host
measurements, not a statistically isolated speedup or a full-R10 completion
estimate. Preparation still dominates these small controls. The large harness
compares counters; exact ordered-key/terminal parity remains covered by focused
native tests. No new IBP generation or coefficient back-substitution occurred.

Evidence: `TMP/trace-support-matched-controls.vaYQDj/`, actual receipts
`TMP/shared-owner-campaign.6cz81qtg/` and
`TMP/shared-owner-campaign.p1mmc8h2/`, with coherent gates
`TMP/trace-support-core.sWtUZO/` and `TMP/trace-support-app.Z19PgH/`.
The separate 75-owner, 134-input pressure run measures a much larger dependency
graph; these completed rank-one controls do not establish its completion.

### Sharded-membership preprobe matched controls

The next frozen release adds bounded membership preprobes outside the global
queue lock, while retaining exact full-key/phase equality, race rechecks, FIFO
admission and the same budgets. Full gates passed 2,576 core tests (32 existing
ignored) and 281 application/integration tests. The matched controls use the
same 67 owners, input, frozen Python driver, CPU affinities and limits as the
visitor controls above. All 16 graph counters and both prospective counters
match those controls and the earlier batched baseline exactly; both queues
drain without failure or missing frontier.

| Outer workers | Preparation (s) | Shared traversal (s) | Whole command wall (s) | Whole command CPU (s) | Peak RSS (KiB) |
|---:|---:|---:|---:|---:|---:|
| 1 | 99.862 | 20.978 | 124.10 | 123.06 | 5,802,000 |
| 6 | 102.210 | 7.153 | 114.08 | 144.84 | 5,804,828 |

Against the visitor's 20.437/8.067-second traversal, serial time is about 2.6%
longer: **no serial improvement observed**. Six-worker traversal is about 11.3%
shorter, but its whole-command time increases from 112.08 to 114.08 seconds.
This is not an overall-run improvement. The new one-to-six traversal ratio is
2.93×, not sixfold. Preparation remains dominant and variable.

These are single shared-host observations, with no compilation or elapsed
deadline. A separately bounded one-core domain census on CPU40 overlapped the
first run's preparation; it finished before either measured traversal. The
control supervisor retained its original resource policy and did not include
that separate census in its own process-tree measurements. No 50-worker or
parametric R10 completion claim follows, and full terminal sets were not
persisted for this counter comparison.

Evidence: `TMP/membership-matched-controls.O3oJC2/`, including the six-way
comparison against both prior control pairs, and actual process receipts
`TMP/shared-owner-campaign.rh58lcl9/` and
`TMP/shared-owner-campaign.dpdieb3e/`. Frozen gates are
`TMP/membership-domain-core.sOjXbE/` and
`TMP/membership-domain-app.LtMTsi/`.

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
and runtime reviews accompany the slice. The matched controls above measure
performance separately; implementation tests alone imply no speedup or
parametric coverage.

The first new frontend gate caught an input-boundary defect: Serde's default
struct visitor accepted an empty positional array as the named budget object.
The production reader now explicitly requires an object and retains direct
duplicate-field rejection; the original failing assertion is unchanged and a
full positional-array regression was added. The corrected full release gate is
`TMP/shared-campaign-feedback-app-fixed.DEwkOr/`; the core runtime gate is
`TMP/shared-campaign-batched-core.EWNFkj/`. Its initial build-log parser failed
after compilation; the existing frozen binary subsequently passed every test.

### Native support-only traversal

The shared trace now consumes the exact coalesced support of Symbolica's native
rational polynomial directly. It no longer builds contextual coefficient
wrappers or a complete endpoint vector merely to discard the coefficients.
The public coefficient-returning transport and numerical reducer are unchanged.
Admission checks the entire native support before exposing keys, preserves
ascending integral-key order and charges the same conservative virtual output
budgets. Later allocation or publication failure still returns an incomplete
trace, never successful partial output.

Nine added regressions cover cancellation inside the polynomial, zero and
constant products, large rationals, ordered keys, malformed native layout,
overflow, caps and parity with full coefficient transport. The complete release
gates pass **2,554 core tests** (32 existing ignored) and **276 application and
integration tests**, with zero failures and independent reviews. Receipts are
`TMP/trace-support-core.sWtUZO/` and `TMP/trace-support-app.Z19PgH/`.
Matched one-/six-worker timing and a subsequent 50-worker pressure retry remain
separate measurements; test success does not establish R10 campaign closure.
