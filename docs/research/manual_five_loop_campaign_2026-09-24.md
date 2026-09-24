# Manual five-loop campaign: launch-readiness record

## Scope

This delivery prepares a user-launched traversal of the saved five-loop rules;
it does not claim the complete campaign has already finished. The required
67-owner input retains the A≤24, R≤15, A−R≥9 starting envelope and every
escaping descendant. Missing rules, unsupported geometry and undischarged
responsibilities remain explicit failures/frontiers, never new master terminals.

The saved rule ordering works on the completed controls. It has not been shown
optimal for the full envelope. Ordered publication and the measured worker
partition remain the baseline; physical subdivision changes native task size,
not the integral ordering or the mathematical entry scope.

## Checkpoint and memory contract

- The Python launcher defaults to at most 50 permitted CPUs and a 500 GB
  decimal aggregate-RSS ceiling. A 5% margin requests a cooperative save and
  stop at 475 GB, or earlier if available host/cgroup memory requires it.
- There is no elapsed-time deadline or cumulative enumeration-work cutoff in
  the production preset. Bounded in-flight buffers, scratch and algebra
  safeguards remain explicit. Any refusal is reported as incomplete work.
- Rust checkpoints the canonical queue, dependency ledger, coverage index,
  diagnostics and committed native-event prefix. Immutable owner programs stay
  in their existing binary artifacts; checkpoint metadata does not replace
  those programs or introduce a second algebra representation.
- State is streamed to disk and atomically published. Latest and previous
  generations are retained. Resume verifies exact input, owner and executable
  identity. A corrupt latest generation is an error, not silent rollback.
- Periodic saves default to 3,600 seconds at consistent publication boundaries.
  They continue the campaign. Ctrl-C and the RAM guard request a save and exit.
  A bootstrap save can restart preparation but contains no finished native work.
- Completed native parts survive. An unfinished native part can replay its
  verified prefix; that prefix must not be published or counted twice.
- Checkpoint/report serialization streams without building a second whole
  serialized image. Final report arrays move rather than clone. Hard memory
  emergencies may still kill the owned child: sampled RSS is not an allocation
  guarantee, and the recovery point is then the last completed generation.

Save-start and save-complete events have a bounded lossless journal channel;
they are not merely sampled heartbeats. The monitor reports generation, path,
UTC times and duration in both TTY and plain modes. Actual CPU usage is distinct
from reserved workers. The initial-entry bar is not a family-closure percentage;
the descendant worklist has no known final denominator or defensible ETA.

## Persistent local input

The ignored `campaigns/five-loop-saved/inputs` directory contains 67 immutable
owner bundles, a relative-path selection, the unchanged base queries and an
input receipt. Owner payloads total 1,280,854,595 bytes; the complete staged input
is 1,291,422,690 bytes. Its query SHA256 is
`c80d332a20bf9ed5b957106d96c3e8db1c255ac873d51dbc6f18827a3a94e570`.

These are local generated inputs, not topology-specific code. They are not
included in the Git push. The production executable is deliberately not frozen
before the user builds and first prepares/launches it. Subsequent restarts use
that exact frozen executable and policy, unaffected by later Cargo rebuilds.

## Validation record

The source audits covered checkpoint restoration, physical-part publication,
prefix replay, cancellation races, bounded monitoring and crash durability.
They found and prompted fixes for late worker faults masked by cancellation,
transient full-report duplication, unsupported direct-API policy combinations,
and missing parent-directory sync at checkpoint bootstrap. Lossless checkpoint
milestones and durable launcher metadata were also added.

The first release application test pass was 582 passed, one failed, one ignored.
The failure was an obsolete assertion requiring the former one-million-frontier
hard ceiling. Invalid workers/zero-frontier rejection remains; the revised test
explicitly admits large finite and unlimited frontier allowances.

The final release gate passes **593 application/CLI tests**, with one existing
ignored test and zero failures. It includes real W2/W6 native on-disk repeated
interrupt/resume, strict partial-event-prefix replay, corruption rejection,
late worker-fault precedence and lossless checkpoint milestones. The release
CLI build also passes. The Nix Python 3.11.15 launcher/monitor gate passes
**51 focused tests**; all four packaged Nix apps pass their help smoke checks.
The measured CLI is frozen locally at `TMP/manual-launch-gate.8Nrfgc/rustred`,
SHA256 `0f1644cbc6099e3150e3b90115727761389c80794e9c39f1605accfa2cdd9f0a`.

The fresh-process Python/Nix correctness gate also passes on the unchanged
762-query, four-owner control. Its uninterrupted result contains 29,718 logical
domains and 2,084,065 committed events. A real Ctrl-C after 179,482 committed
events saves a compact paused receipt; executing its printed restart command
in a new process reaches the same canonical mathematical result. Startup
interruption and its emitted restart command also match that baseline.
All three completed results have canonical SHA256
`f9eee5541003064f10853e59425b1be5f2dcb607faead9d1ed720b9f89c31d8f`.
The comparison retains geometry, guards, diagnostics, native statistics and
dependency obligations; elapsed time, replay attempts and speculative lookup
effort are separately reported rather than asserted identical.

This correctness-only test used a one-second checkpoint interval and verified
21 genuine interior periodic saves of unfinished work, not heartbeat duplicates.
Production and performance controls retain the hourly default. Evidence is
under `TMP/checkpoint-integration.bzunHI/run`; no full-67 run was launched.
Compilation is not included in solver timings.

## Completed integration controls and production choice

Nine anchored four-owner A12 controls ran in three rotated orders, using the
same release executable, W50 (25 inspectors, 24 admission helpers and one
coordinator), CPU0–49, H256, serial nested pools and uncapped enumeration work.
All complete with zero frontiers/failures and discharged responsibilities.
Each checkpoint/no-checkpoint pair has exactly matching canonical results.
The optional split preserves each source obligation as the exact disjoint union
of two physical inspections; its actual extra work is retained in the receipts.

| Variant | Median traversal | Whole-process CPU | Peak RSS | State-save time |
|---|---:|---:|---:|---:|
| Checkpoint disabled | 11.027 s | 120.98 s | 589,116 KiB | — |
| Checkpoint enabled | 11.548 s | 123.54 s | 590,188 KiB | 0.317 s |
| Checkpoint + initial axis-0 split | 11.731 s | 125.67 s | 585,036 KiB | 0.349 s |

Checkpointing adds about 4.7% to median traversal on this small workload,
including verified-prefix bookkeeping and startup/final state saves. These
short controls retain the production hourly interval, so they do not measure
a full hour between periodic saves. Whole-command medians are approximately
15.1 s and affected by supervisor polling; they are not solver-only timings.
Shared-host, three-rotation measurements are not a confidence interval.

Subdivision is **off by default**: it is 1.6% slower in the integrated median
and loses two of three rotations, despite the earlier native-only 1.51× gain.
The explicit `--apply-subdivision-axis 0 --apply-subdivision-cut 0` option remains
available for controlled use. This is not evidence of full-family acceleration
or fifty-core saturation.

The tenth exploratory direct-four-root checkpoint control also completes:
21.127 s traversal, 169.19 CPU s and 776,588 KiB peak RSS. Thus auxiliary P13
regions still help that particular four-owner case, but the all-owner benefit
is not established. The available 57,621-query augmented file has A12/R3/D9
roots, **not** the required A24/R15/D9 envelope. It must not replace production.
The manual launcher therefore retains the unchanged 67-root input, Ordered
publication, H256, initial-D reuse and the existing worker partition.

Raw timing evidence and full counter differences are in
`TMP/integrated-subdivision-controls.SonXit/run`. All ten controls completed;
there were no automatic retries or elapsed-time stops. Independent mathematical,
timing and resource audits all **pass**. A separate parser reproduced the raw
canonical digests, exact split geometry and counter differences, checkpoint
milestones, CPU/RSS measurements and medians. The resource audit verified the
worker budgets and sampled affinity without confusing reservation with use.
The fresh-process checkpoint integration also has an independent **PASS**.

After timing, a monitor-only startup correction made the first CPU sample
explicitly unknown: one tick divided by a sub-millisecond initial interval had
briefly exaggerated supervisor CPU use. Later samples remain measured and are
not clamped to the worker budget. Two extra regressions bring the Python gate
from 49 to 51 tests. The exact timed supervisor is archived under
`TMP/manual-campaign-validation/timed-python-before-cpu-fix`; native code,
mathematical results, GNU-time measurements and checkpoint behavior are unchanged.

This is a launch-readiness record, not a five-loop completion certificate.
See [the driver instructions](../shared_owner_campaign_driver.md) for exact
build, launch, resume and read-only monitoring commands.
