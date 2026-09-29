# LC2 integration and pilot protocol (2026-09-29)

Status: registered before new performance measurements. This is not a result
or a forecast. Governing plan: `CODEX_PROGRESS_PLAN.md`.

## Hypothesis and controls

G2′ exact residual reuse should avoid repeated inspection of overlapping Apply
domains. Rescue should keep irrelevant failed helper domains from blocking
independently closed required queries. Neither permits dropping required
queries, descendants, guards or dependency edges. G2′ is the measured work
lever; rescue is recovery capability, not a presumed speed improvement.

The performance baseline is frozen LC2
`TMP/fable51-controls/bin/rustred-lc2-fd9b9ac9`, SHA-256
`fd9b9ac96a50513d8119a43938313e8a729110d6a3815b0f949cec4acb1d9d60`.
Use the same `campaign` profile (release, fat LTO, one codegen unit, no
mimalloc override) for a deployment comparison. Plain release builds are
appropriate for correctness development but not an unmatched speed claim.
Also compare candidate G2′ off versus union to isolate the mechanism.

Inputs: immutable format-6 controls already described by
`TMP/lc2/commands/{fg,bmw,h,x,four-all,four-all-p5,five-finite}.json`.
These are argv templates, not commands to run unmodified: output, checkpoint,
event and stop-file paths must all point to a fresh lane under `TMP/`.
Use the documented **r1a12** hot-sub control, not the different s2/r2a12 box.
Freeze source, binary, input hashes, argv, environment and timed boundaries
in each receipt. Never overwrite earlier measurements.

The historical baseline rejects the new optional `query_roles` field. For
matched old/new performance controls use the **same original roleless query
bytes**, with every query required and no rescue amendment in either arm.
Test explicit roles and rescue separately. Do not silently strip roles in
one timed arm, or compare rescued required-query success to the baseline's
all-root success as if they were the same workload.

## Resource and timing boundary

- Root allocates one timing lane on CPUs 64–79, W16 for C-5F, and W12 within
  that allocation for hot-sub. Recheck availability and record competing
  load; no claim of an exclusive host. Initial correctness tests use the
  separate implementation-lane allocations, not production CPUs 128–227.
- Serialize timing against our builds and heavy analysis. Respect existing
  locks and host headroom. A changed allocation is recorded before a new
  pair and used identically by both arms.
- The audited G2 pilot runner owns its solver process group and must receive
  `--heavy-lock /common/dev/rustred/TMP/locks/heavy.lock
  --minimum-start-headroom-gib 250 --minimum-headroom-gib 150` explicitly.
  Do not wrap that runner in the build guard: its separately created solver
  group would escape the outer guard. The runner retains its resource lock
  until all owned group members have stopped, not merely its launcher.
  A memory/operator/time stop is censored even if the native exit code is zero.
- Each pilot has a 1,800-second total ceiling. Request cooperative stop no
  later than 1,200 seconds and reserve the remaining time for durable save,
  output and shutdown. Set the runner grace to at most 540 seconds, leaving
  60 seconds for polling/recorder overhead before the overall ceiling.
  Use an existing owned-process harness; never signal
  production or unrelated processes. Killed/incomplete runs are censored.
- Time the matched whole command from launcher spawn through owned process-group drain,
  explicitly including `nice`/Nix setup, input load, preparation and final
  output. Exclude sampler-thread shutdown from that boundary and report it
  separately; separately record native traversal/inspection time.
  Compilation is excluded and recorded independently. Cold verification is
  separately timed, never represented as generation/traversal performance.
  Each cold-verifier job also has its own <=1,800-second ceiling and the same
  CPU/headroom protections; an unfinished verifier cannot authorize a pass.
  For a directly executed cold verifier, the local build guard may instead
  own `timeout --signal=INT --kill-after=60 1200 <binary> walk-verify-closure ...`:
  unlike the pilot runner, this command does not launch another solver session.
  Check both exit code zero and no guard stop reason before considering its
  actual verifier report. Owned normal-exit and signal-resistant timeout
  mocks were checked in `TMP/codex-verifier-guard.7rBJCw/`; the timeout case
  returned nonzero and left no owned process group. Verification is not
  included in the solver timing comparison.
- Use interleaved A/B then B/A pairs for a switch recommendation. Record
  all outcomes, including noisy runs and failures. No historical full-width
  throughput number substitutes for the contemporary reduced-width pairs.

## Correctness and acceptance

1. Feature-off strict Ordered identity on the seven controls, including
   containment checks, compared to valid LC2/reference receipts.
2. Feature-on completed controls, independent full reinspection, plus
   targeted union-hole, dropped-edge, invalid-anchor and scope mutations.
3. Explicit required/auxiliary roles: all 116 frozen starting queries stay
   required; missing or misleading names cannot reduce the required set.
4. Joint rescue/G2′ tests in both activation orders; later quarantine of a
   previously eligible anchor must preserve accepted dependencies and keep
   affected required queries unresolved until independently repaired.
5. Pause/resume and cold-load tests with accepted-prefix replay. A fresh
   campaign is acceptable if inexpensive validated activation is unavailable.

Report wall/CPU time, scheduled domains, peak pending, retained/RSS memory,
required-query closure, frontiers, work mix, and coordinator/inspector costs.
Use the runner's waited-child user/system CPU deltas for the whole command;
retain sampled thread times only as separate diagnostic measurements. The
single-child RSS maximum and sampled aggregate tree RSS are distinct metrics.
Keep the existing pending-growth metric unchanged. Require a real verifier
report with `PASS` and present integer independently-verified/total counts
equal; absent reports or absent counts must not pass a gate.

The plan's switch threshold is >=20% less end-to-end time, or >=20% less
work/pending/memory without an offsetting material throughput regression,
reproduced in two matched pairs. For the alternate resource/work gate, define
material regression as >10% higher matched end-to-end wall time; native calls
per second is not a comparable progress measure when G2′ changes the work mix.
Host noise comparable to the effect makes the decision inconclusive.
A small-control success does not establish
the same gain on the mature hot-owner mix. Only the user switches production.

## Pair attribution before the Ready comparisons

The primary decision metric is whole launcher-to-owned-group-drain wall time.
The alternative work/resource gate uses scheduled domains or sampled aggregate
RSS, subject to the <=10% wall-time regression limit above. Heartbeat maxima
are only lower bounds on peak pending, not an authoritative exact peak, and
cannot qualify this comparison through the pending-pressure alternative.
Report each of the two paired candidate/reference ratios, not only a pooled
mean or the best arm. The qualifying threshold must reproduce in both pairs;
otherwise report the result as inconclusive, retaining every outcome.

Run LC2-Off / candidate-Union, then candidate-Union / LC2-Off on identical
roleless inputs for each Ready control. Candidate-Off / candidate-Union
mechanism isolation is separate and uses fresh arms; one Union run cannot be
counted as two independent repetitions. Freeze the same tested runner revision
and hash for both executables, including the corrected bounded metric reader.
Cold reinspection and Python audits remain separately guarded and excluded
from solver timing. Record selected-core and SMT-sibling contention per pair.

The historical `g2prod_gate.py` command assumes same-binary, three-repeat,
historical labels. Do not silently use those assumptions for this two-repeat
cross-binary deployment comparison. Build the table from explicit receipt
paths and actual executable identities/worker widths, checking every
completion and verification gate. Missing or corrected historical counters
must remain explicitly attributed to their source or corrected sidecar.
