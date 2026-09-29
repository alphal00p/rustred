# G2 + rescue: Ready deployment comparison, 2026-09-29

## Status and scope

**All eight arms are fully accepted and the sequence has drained. This is a
qualified individual improvement, not permission to switch production.**
All Ready measurements below are from the one predeclared
sequential batch. No arm of this batch was discarded or rerun. LC2 remains
untouched.

**Deployment hold:** the user's subsequent direction permits a future restart
but asks that the validated remaining ideas be consolidated into the build
first. Retain this unchanged comparison as the G2 baseline; do not restart
with G2 alone. Remaining epoch/profiling avenues must earn their own validation
and integration gates. This does not mean enabling speculative or rejected
ideas by default. LC2 remains running and unchanged.

The comparison is **LC2-Off versus the integrated candidate with Union**, not
same-binary mechanism isolation. The eight separately prepared mechanism arms
remain unstarted. Both executables use the campaign profile (opt3, fat LTO,
one codegen unit); compilation is excluded. Candidate native source is
`d12db6cf`, executable SHA256 begins `8169221a`; LC2 begins `fd9b9ac9`.
Both use format-6 owner artifacts and plain upstream Symbolica `ef0db494`.

Each cohort uses identical immutable roleless query bytes, the same 67-owner
manifest/payloads, Ready publication, lookahead/budgets, and the same corrected
runner. The sequence is LC2/Union, then Union/LC2:

- Finite: one owner query, rank <=2, positive-power sum <=11, power difference
  >=9, coordinate upper bounds 15; W16 on CPUs 64–79.
- Hot subset: the same owner, rank <=1, positive-power sum <=12, otherwise
  unbounded upper/difference bounds; W12 on CPUs 64–75.

Each native arm has a 1,200-second cooperative deadline plus 540 seconds for
drain, 250 GiB start/150 GiB running headroom, and the shared heavy lock.
Cold reinspection and Python auditing are separately guarded and excluded
from the native whole-command time. No build or other benchmark overlapped
the native timing intervals.
Live production continued on CPUs 128–227; no affinity exclusivity is claimed.

## Paired results

Each cell is **LC2-Off → candidate-Union**. CPU is waited-child user+system
time, including the native/Nix command tree. RSS is sampled aggregate tree RSS
in GiB, not the maximum single-child RSS or an exact peak-memory certificate.
Wall is launcher start through owned-process-group drain, excluding recorder
shutdown. Every row below passed all native, cold, and Python gates.

| Cohort / pair | Whole wall (s) | Child CPU (s) | Scheduled domains | Sampled RSS (GiB) |
| --- | ---: | ---: | ---: | ---: |
| Finite 1 | 249.626 → 180.746 | 1,082.484 → 564.716 | 1,275,122 → 964,909 | 5.925 → 5.811 |
| Finite 2, reverse order | 252.306 → 180.010 | 1,093.842 → 558.191 | 1,276,229 → 963,440 | 5.943 → 5.807 |
| Hot 1 | 434.976 → 201.743 | 2,093.876 → 584.808 | 1,416,894 → 1,016,660 | 6.003 → 5.801 |
| Hot 2, reverse order | 422.950 → 197.730 | 2,070.231 → 572.837 | 1,417,721 → 1,025,239 | 6.004 → 5.802 |

Candidate/reference ratios, without pooling or selecting the best repetition:

| Cohort / pair | Wall | CPU | Domains | RSS |
| --- | ---: | ---: | ---: | ---: |
| Finite 1 | 0.7241 | 0.5217 | 0.7567 | 0.9808 |
| Finite 2 | 0.7135 | 0.5103 | 0.7549 | 0.9771 |
| Hot 1 | 0.4638 | 0.2793 | 0.7175 | 0.9663 |
| Hot 2 | 0.4675 | 0.2767 | 0.7232 | 0.9664 |

Both cohorts meet the **numerical scheduled-domain work criterion**: the same
metric falls by at least 20% in each matched pair, with observed wall ratios
below 1.10. Scheduled work fell 24.3%/24.5% for finite and 28.2%/27.7% for hot.
Observed wall time fell 27.6%/28.7% and 53.6%/53.2%, respectively. These are
unadjusted observations, subject to the attribution limits below. The
approximately 2–3% sampled RSS decreases are not a qualifying memory gain.
Exact peak pending is unavailable; sampled heartbeat maxima cannot substitute
for it. No pending-growth metric was changed.

## Correctness and verification cost

Every accepted arm is uncensored, exit 0, with null stop/error reasons, all
scheduled descendants resolved, a real cold verifier PASS with All native
reinspection, and a paired Python audit PASS. Each cohort has one required
query and one distinct root; each accepted arm verifies 1/1, with zero
frontiers, uncovered obligations, or audit violations.

Full cold native counts (LC2 → Union) are 979,714 → 758,183 and
981,826 → 758,280 for finite, 1,064,142 → 787,379 for hot pair 1, and
1,063,432 → 794,916 for hot pair 2. Candidate exact G2 union-cover checks are
80,598, 80,152, 100,544, and 102,191 respectively: all covered, with zero
undecided results or lattice cross-check disagreements. Raw result generation
and digest match each paired audit; verification is not inferred from exit
status alone.

Verification cost is material and is not hidden in a solver speed claim.
Candidate Python audits took 494.6/487.4 seconds for finite and 705.3/723.3
seconds for hot. Candidate cold verifier guard times were 165.3/156.2 and
182.2/187.3 seconds. These retain all exact checks and ran under the original
limits; no audit was shortened to make a run pass.
The final LC2 cold/Python guard times were 620.451/108.190 seconds (Python
audit body 107.101 seconds), all exit 0 with null guard reasons.

After all eight native process groups had drained, a separately guarded warm
epoch metadata-only check overlapped the final LC2 verification, from
08:06:16.194 to 08:06:57.364 UTC (41.169 seconds; CPUs 16–19, one Cargo job,
build-1 lock, about 1.19 GiB peak single-child RSS). It passed and its owned
group drained, with no broad rebuild. This is a verification-cost overlap,
not a native timing overlap; no solver timing or verification limit is
adjusted because of it.
Its separate receipt is
`/common/dev/rustred/TMP/codex-epoch-s3.JjASCU/typecheck-controller-light/`.

The prerequisite **14 Ordered controls** (Off and Union for FG, BMW, H, X,
four-all, four-all-p5, and finite five-loop) independently passed: strict
Off identity against LC2, including containment counters, full cold native
reinspection, and paired Python coverage. This is not universal work reduction:
four-all grew from 65,444 to 68,184 domains (+4.2%), and four-all-p5 from
68,483 to 71,284 (+4.1%). Ordinary Python→native Union pause/resume was also
validated; historical accepted-pin replay has separate native regression
coverage, not a claim inferred from that early-pause smoke.

## Host noise and limits of inference

Mean busy cores below are diagnostics, not timing corrections. Selected-core
foreign activity is the runner's sampled process-tree subtraction. SMT
sibling activity is the before-arm/after-native counter difference divided by
that interval; it is unattributed and cannot identify a foreign process.

| Cohort / pair | Selected foreign: LC2 → Union | SMT sibling busy: LC2 → Union |
| --- | ---: | ---: |
| Finite 1 | 1.146 → 1.076 | 3.317 → 1.283 |
| Finite 2 | 0.973 → 1.676 | 3.096 → 1.402 |
| Hot 1 | 0.871 → 1.879 | 2.021 → 0.758 |
| Hot 2 | 0.464 → 1.051 | 1.354 → 1.555 |

Most sibling activity is user+nice time, but the direction and composition of
the pairwise imbalance vary. In reverse hot order, Union's user+nice sibling
activity was lower (1.072 versus 1.205 cores), while its kernel/IRQ activity
was higher (0.483 versus 0.149). The selected-core imbalance also sometimes
points the other way. Unequal occupancy alone neither
quantifies a wall penalty nor proves the noise negligible. Therefore retain
the exact numerical work/wall results **without an adjusted speedup or an
automatic clean causal-timing PASS**. The protocol makes noise comparable to
the effect an inconclusive decision; these counters cannot establish that
comparison quantitatively.
In reverse hot order, Union's measured gain repeated despite higher selected
and sibling activity by these diagnostics. This supports the repeatability
of the observed result; it still supplies no quantitative correction for
contention or proof of performance on the full production workload.

A bounded live-status read recorded LC2 checkpoint generation 4 at
07:45:25–07:45:53 UTC, with separately reported duration 67.058 seconds.
Do not force those fields into one interval. Candidate hot pair 2's native
before/after interval ended at 07:42:42; the recorded checkpoint occurred
during its subsequent verification, not a known native timing overlap. No
penalty is assigned and no arm is discarded.

These are W16/W12, single-query controls, not the mature W100 production mix.
They establish no full-family certificate, no closure of the production 116
required queries, no remaining-time forecast, and no guarantee that restarting
will repay LC2's accumulated progress. Production rescue was not exercised by
these frontier-free timed arms; its correctness has separate tests.

## Deployment hold; reference G2-only fresh-run workflow

**The eight-arm individual baseline is complete; do not restart with this
G2-only candidate.** A qualified individual improvement is evidence for the
later consolidated candidate, not satisfaction of the user's new request to
put the validated remaining ideas into the build before restarting.

The eventual consolidated source/binary must be frozen and independently
tested, with applicable correctness and matched performance gates, before a
new restart decision. Do not presume every avenue will pass or silently turn
rejected experiments on. This report neither forecasts that future build nor
authorizes production stop/start. Never rebuild dirty main or overwrite its
FeynKit/user changes.

The already-tested `rustred-d12-8169221a` needs no rebuild to serve as this
G2 baseline. The commands below preserve its **reference-only** fresh-run
workflow, not the current deployment instruction. A later consolidated
deployment must bind its own reviewed source revision and executable; do not
silently substitute a new binary into these old identities.

Fresh preparation retains all 183 original query objects in their existing
order and all 67 payloads. The only query-document change is the explicit
116-required/67-auxiliary role partition. That enables rescue without reducing
required scope; it does **not** add a fresh live 116-query counter or make an
otherwise unamended walk terminate as soon as 116 witnesses are present.
Role changes alter the request digest, so do not copy LC2's checkpoint or use
an executable upgrade/Off→Union activation as a role-migration shortcut.
An exhausted rescue budget or unresolved required frontier remains incomplete.

The following is **reference-only, has not run, and is not permission to
restart now**. The worktree and new campaign paths must not exist. Stop if
setup or validation fails; do not reuse an old destination, reset main, or
substitute its dirty working files. The licensed environment must already be
available; this reference uses the existing tested binary.

```bash
export G2_RUNBOOK_WT=/common/dev/rustred/.claude/worktrees/codex-g2-production-frozen
export G2_RUNBOOK_NEW=/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-g2-rescue
export G2_RUNBOOK_OLD=/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-lc2
export G2_RUNBOOK_BIN=/common/dev/rustred/TMP/codex-g2-rescue.tzdFuj/candidate-bin/rustred-d12-8169221a
git -c user.name=ValentinHirschi -c user.email=valentin.hirschi@gmail.com \
  -C /common/dev/rustred worktree add -b codex/g2-production-frozen \
  "$G2_RUNBOOK_WT" a0a91dbaab0718ab36dedfd455650353c033f7dd
git -c user.name=ValentinHirschi -c user.email=valentin.hirschi@gmail.com \
  -C "$G2_RUNBOOK_WT" submodule update --init vendor/symbolica
env TMPDIR=/common/dev/rustred/TMP RUSTRED_TESTS_REQUIRE_LICENSE=1 \
  taskset -c 32-39 nix develop "$G2_RUNBOOK_WT" --command python \
  "$G2_RUNBOOK_WT/examples/python/check_renormalization_entry_queries.py" \
  --queries "$G2_RUNBOOK_WT/examples/input/five_loop_qcd_feynman_d9d10/queries.json" \
  --receipt "$G2_RUNBOOK_WT/examples/input/five_loop_qcd_feynman_d9d10/entry-plan-receipt.json" \
  --entry-plans "$G2_RUNBOOK_WT/examples/input/five_loop_qcd_feynman_d9d10/entry-plans" \
  --probes 64
```

Prepare without launching:

```bash
env TMPDIR=/common/dev/rustred/TMP RUSTRED_TESTS_REQUIRE_LICENSE=1 \
  nix develop "$G2_RUNBOOK_WT" --command python \
  "$G2_RUNBOOK_WT/examples/python/production_saved_owner_campaign.py" \
  --campaign-directory "$G2_RUNBOOK_NEW" --prepare-from "$G2_RUNBOOK_OLD" \
  --queries "$G2_RUNBOOK_WT/examples/input/five_loop_qcd_feynman_d9d10/queries.json" \
  --attach "$G2_RUNBOOK_WT/examples/input/five_loop_qcd_feynman_d9d10/entry-plan-receipt.json" \
  --query-order preserve --executable "$G2_RUNBOOK_BIN" \
  --workers 100 --cpus 128-227 --publication-policy ready \
  --transfer-unreserved-lookahead 256 --g2-residual-anchors union \
  --frontier-policy stop --auto-rescue --max-rescues 32 \
  --checkpoint-interval-seconds 14400 --max-memory-bytes 600000000000 \
  --ram-guard-margin-percent 5 --host-memory-reserve-bytes 50000000000 \
  --swap-growth-stop-bytes-per-second 33554432 --swap-growth-stop-seconds 120 --json
```

Reference preparation has no `--start`. Any eventual deployment must review
its newly frozen plan and staged role/owner census. Only after consolidation,
all applicable gates, and a renewed owner-controlled restart decision would
the owner cooperatively pause LC2, wait for its durable save and authenticated
process drain, and explicitly start the new campaign. The old G2-only launch
form is shown solely for reference:

```bash
env TMPDIR=/common/dev/rustred/TMP RUSTRED_TESTS_REQUIRE_LICENSE=1 \
  nix develop "$G2_RUNBOOK_WT" --command python \
  "$G2_RUNBOOK_WT/examples/python/production_saved_owner_campaign.py" \
  --campaign-directory "$G2_RUNBOOK_NEW" --start
```

There is no `--resume` on this first launch. Preserve LC2's entire old directory,
inputs, executable, policy, and checkpoint. To roll back, first cooperatively
pause/drain the new campaign, then use the same launcher with
`--campaign-directory "$G2_RUNBOOK_OLD" --resume --json` for review, followed
only by an owner-approved `--resume --start`. Do not add Union/rescue flags to
that old frozen policy or run both campaigns on the same CPUs.

## Evidence

All local evidence is under
`/common/dev/rustred/TMP/codex-g2-pilot-prep.n7Kd5q/`:
`DEPLOYMENT_PLAN.json`, `deployment-continuation/accepted-1.json` through the
final `accepted-8.json`, `ready-pairs/` native/guard/cold/Python reports, raw
contention snapshots, `CONTROL_RESULTS.json`, and `ORDERED_RESULTS.md`.
The detailed conditional setup/rollback/hash record is
`CONDITIONAL_DEPLOYMENT_RUNBOOK.md` in that directory. Actual runtime
identities and argv are preserved there; original receipts remain unchanged.
The sequencer exited 0 with `EIGHT_DEPLOYMENT_ARMS_PASS`. A final bounded
receipt review checked all eight metrics/actual argv and 24 verifier/oracle/
Python guard receipts; all 32 recorded owned process groups had no members,
and the sequencer PID was absent. Production was neither signaled nor changed.
The decision rules are in
`docs/research/codex_lc2_integration_protocol_2026-09-29.md`.
