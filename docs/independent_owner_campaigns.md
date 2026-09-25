# Independent starting-topology campaigns

This opt-in mode adds a **dynamic outer job queue** to saved-rule recursive
application. It does not regenerate IBPs, change the mathematical input, or
replace the existing shared campaign. It is independent of loop count.
The process supervisor currently requires Linux (`/proc` and CPU affinity);
the mathematical engine and the saved binary rule format are unchanged.

Each distinct starting owner gets one queued job by default. All original
queries for that owner, including auxiliary queries, stay together. A free
execution slot immediately takes the next waiting job. Jobs are not assigned
to worker groups in advance. An optional `shards` setting groups owners when
amortizing startup is more important than fine-grained outer scheduling.

Every job has the full immutable rule selection and routing information. It
must follow **all of its own descendants**. It cannot discharge work merely
because another job has queued, visited, or completed it. This intentionally
trades repeated subsector work for independent progress. It also loses sibling
owners' initial coverage anchors. This can be much worse than a constant-factor
duplication: a shared campaign that finitely covers an unbounded symbolic
region need not finish when partitioned this way. Concrete IBP descent alone
does not prove termination of that symbolic-domain traversal. Keep the shared
mode for workloads whose convergence depends on global auxiliary covers.

## Two levels of parallelism

The existing native scheduler remains in use inside each job. For example:

| Concurrent jobs | Workers per job | Total worker budget |
|---:|---:|---:|
| 5 | 10 | 50 |
| 10 | 5 | 50 |
| 25 | 2 | 50 |
| 50 | 1 | 50 |

These are configurations, not speedup promises. All compute jobs receive
disjoint CPU affinities. Nested library pools are capped. The supervisor
measures actual CPU use separately from reserved workers. More occupied
workers can still mean more duplicated work and a slower completed campaign.

This is a shared pending queue at topology granularity, not a replacement
work-stealing executor for every internal symbolic task. A job keeps its CPU
allocation until it finishes or pauses. Straggling jobs do not acquire all
other free cores automatically.

## Python steering and native execution

The Python example writes configuration and optionally replaces itself with
the Rust CLI. Scheduling, monitoring, resource guards, checkpointing and
combined output are all native Rust responsibilities.

```sh
nix develop
cargo build --release --locked -p rustred-app --bin rustred
python examples/python/independent_owner_campaign.py \
  --executable target/release/rustred \
  --manifest PATH/selection.json --queries PATH/queries.json \
  --owner-base PATH \
  --directory campaigns/my-independent-run \
  --jobs 10 --workers-per-job 5 --total-workers 50 \
  --max-memory-bytes 500000000000 --start
```

Omit `--start` to prepare and print the command without launching it. CPU IDs
default to the permitted affinity; use `--cpus` with a comma-separated list
of OS CPU IDs to choose nonoverlapping physical cores on your machine
(the supervisor does not infer physical-core or SMT topology). The number of concurrent jobs must not
exceed the number of queued jobs. `--shards N` is optional grouping; omitting
it queues each starting owner separately.

The equivalent native command is:

```sh
target/release/rustred campaign shards \
  --config campaigns/my-independent-run.config.json \
  --directory campaigns/my-independent-run
```

The strict JSON configuration has schema
`rustred.independent-root-config.v1`. Its input fields are `manifest`,
`queries` and `owner_base`; relative paths are resolved against the
configuration's directory. Scheduling fields are `jobs`, `workers_per_job`,
`total_workers`, `cpus` and optional `shards`. Safety fields are
`max_memory_bytes`, `host_reserve_bytes` and
`checkpoint_interval_seconds` (default 3600). `publication_policy` defaults
to `ordered`; `native_options` forwards supported native search policies.
There is no solve-time deadline or artificial descendant-work ceiling.

## Monitoring, stopping and resuming

The native colored terminal display includes the completed-job bar, queued
and active jobs, measured busy cores, RSS, native progress and checkpoint
state. **The completed-job fraction is not a wall-time ETA.** A small number
of slow jobs can dominate the remaining time. Non-TTY output is a compact
text stream; JSON events and a bounded status snapshot support external
monitoring. `NO_COLOR` selects the plain periodic text output.

Another terminal can attach a read-only view:

```sh
target/release/rustred campaign monitor --directory campaigns/my-independent-run
target/release/rustred campaign monitor --directory campaigns/my-independent-run --once
target/release/rustred campaign monitor --directory campaigns/my-independent-run --json
```

Ctrl-C in the supervisor terminal requests cooperative checkpointing of active
jobs. Ctrl-C in a read-only monitor only closes that viewer. Approaching 95%
of the configured aggregate RAM limit also requests a save and stop;
emergency exhaustion can require killing owned processes, in which case
only already durable checkpoints survive. Periodic checkpoints occur at
native safe points, so a long indivisible operation can delay a requested
save. RAM limits are caller-selected, not capped at 500 GB.

Resume uses the frozen executable, inputs, job partition and scheduling
policy. Already completed jobs are retained:

```sh
target/release/rustred campaign shards \
  --directory campaigns/my-independent-run --resume
```

The Python example accepts the same `--directory ... --resume --start`
workflow. It rejects new input paths and policy overrides on resume.

If the supervisor itself is forcibly killed or crashes, native jobs can
remain alive. They retain the campaign lock, so a second supervisor cannot
silently duplicate their work. The read-only monitor reports stale status;
do not interpret it as fresh progress. Request an orphan's cooperative pause
by creating `stop-request.json` in its active `jobs/NNNN/attempt-NNNN/`
directory (use the actual attempt path recorded in its launch receipt).
The outer aggregate RAM guard is not active while the supervisor is absent.

## One combined reusable output

`artifact/selection.json` is a normal saved-owner selection containing all
owners and routes. `artifact/owners/` stores each distinct native binary
payload **once**, regardless of the number of jobs. `artifact/queries.json`
contains the original combined starting request. These are usable through
the ordinary saved-owner reader, with `artifact/` as the owner base.

`artifact/completion.json` is written only after every job exits successfully
and its native final event establishes recursive exhaustion without failures
or frontiers. This result is conditional on the supplied starting queries and
the authority of the saved rules. It combines the per-job completion results; it does not turn
candidate rules into certified rules or claim unrestricted family closure.
Until then, the saved selection is input data, not a completed campaign.

Per-job checkpoints, detailed diagnostics and raw result files remain outside
this compact consumer artifact. Copying the artifact does not copy those
potentially large run logs. Detailed receipt paths refer to the original run;
the rules and the combined completion summary remain with the artifact.

## Optional joint source-support pruning

Add `--route-joint-source-support-pruning` to the steering example, or include
it in native configuration `native_options`. The same opt-in flag is
available on `owner-domain-match` and the existing shared Python steering.
It is recorded in checkpoint policy, so it cannot silently change on resume.

For a proposed simultaneous pinch, the bound counts the **union** of source
numerator factors that can supply degree to the removed denominators. A
factor shared by several denominators contributes its degree only once. A
mask is discarded only when its necessary cancellation degree exceeds that
union's available degree. Equality is retained. Constants and cancellations
can reduce attainable degree, never increase this upper bound.

For example, `(D1 + D2) * D3^9` has total degree ten, but it cannot cancel
both `D1` and `D2`: only the first factor supplies their combined degree,
which is one rather than two. Separate single-denominator bounds and the
global degree-ten bound can miss that impossibility; the joint bound catches it.

This uses already compiled Symbolica support information; it implements no
polynomial expansion, factorization or other CAS operation. It remains
off by default because its extra support-union work is not guaranteed to
pay off for every family. Diagnostics report separately how many masks this
additional bound pruned. The underlying rule payloads do not change.
The completed five-loop rank-two comparison pruned additional masks but did
not demonstrate an end-to-end speedup; leaving it disabled is the default.

## Optional fresh launch using this workspace's five-loop inputs

Do not use this as an automatic replacement for the existing live campaign.
The [measured comparison](research/joint_pruning_independent_campaigns_2026-09-25.md)
explains the lost-coverage risk and distinguishes limited controls from the
production workload. The current shared checkpoint cannot be repartitioned
into independent jobs. This command starts **fresh**, preserving the old run.
If replacing it, first request its normal checkpointed pause and wait for it
to stop; do not run two production campaigns accidentally.

The license must already be in the environment. On the current host, CPUs
64–113 are 50 distinct physical cores; choose a suitable set on another host.

```sh
cd /common/dev/rustred
nix develop
export CARGO_HOME="$PWD/TMP/cargo-home"
export TMPDIR="$PWD/TMP"
cargo build --release --locked -p rustred-app --bin rustred

python examples/python/independent_owner_campaign.py \
  --executable target/release/rustred \
  --manifest campaigns/five-loop-saved-coarse-cover/inputs/selection.json \
  --queries campaigns/five-loop-saved-coarse-cover/inputs/queries.json \
  --owner-base campaigns/five-loop-saved-coarse-cover/inputs \
  --directory campaigns/five-loop-independent-20260925 \
  --jobs 10 --workers-per-job 5 --total-workers 50 \
  --cpus "$(seq -s, 64 113)" \
  --max-memory-bytes 500000000000 \
  --checkpoint-interval-seconds 3600 \
  --route-joint-source-support-pruning --start
```

Omit the pruning flag to test scheduling alone. For **one shared queue that
retains all initial anchors**, replace `--jobs 10 --workers-per-job 5` with
`--shards 1 --jobs 1 --workers-per-job 50` and choose a different new directory.
This uses the same native monitoring and can enable pruning without isolating
topologies. Neither configuration upgrades the authority of the saved rules.

Observe the new run from another terminal:

```sh
target/release/rustred campaign monitor \
  --directory campaigns/five-loop-independent-20260925
```

After a normal pause, resume that exact new campaign with its frozen CLI:

```sh
campaigns/five-loop-independent-20260925/bin/rustred campaign shards \
  --directory campaigns/five-loop-independent-20260925 --resume
```
