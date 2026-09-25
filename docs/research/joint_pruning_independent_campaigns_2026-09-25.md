# Joint route pruning and independent starting-owner campaigns

## Scope

This checkpoint implements the two requested opt-in changes, without changing
the running five-loop campaign or developing another algebra/search strategy:

1. A joint source-support bound rejects impossible simultaneous denominator
   pinches before constructing their routed domains.
2. A native supervisor dynamically schedules starting-owner jobs into fixed
   topology-worker slots, retaining the existing scheduler inside every job.

The new mode applies saved parametric rules; it does not regenerate them.
Its combined output retains the original rules, routes and terminals. A
successful traversal establishes exhaustion of the supplied starting-query
obligations, not unrestricted family closure or certification of the saved IBPs.

## Implementation and independent checks

The pruning bound counts a union of source numerator factors, rather than
counting a shared factor separately for each prospective pinch. It retains
equality and every case for which a finite upper bound is unavailable.
Symbolica supplies the previously compiled affine transport; RustRed adds only
checked degree/support bookkeeping. The option is disabled by default.

The outer queue contains one job per starting owner unless explicit grouping
is requested. Original and auxiliary queries for the same owner stay together.
A free topology slot claims the next queued job, with a fixed CPU allocation.
Every job can route to every saved owner and must discharge all its own
descendants. It cannot count another job's pending work as a completed result.
Consequently it loses cross-job coverage reuse as well as repeating some work.
This can remove a finite symbolic-cover convergence mechanism, not merely
repeat a fixed finite set of tasks. For example, a recurrence
`B(n,m) -> B(n-1,m+1)` for `n>=2` terminates for every concrete integer `n`,
with a boundary rule `B(1,m) -> B(1,m-1)` for `m>=2` leading to `B(1,1)`.
Yet an unbounded region `B(n>=1,m=1)` can generate incomparable `m=2,3,...` slices.
A preadmitted `B(n>=1,m>=1)` orthant can absorb them; without it, concrete
descent does not guarantee finite abstract-domain traversal. This illustrates
a real architectural risk, not a proof of nontermination of a measured job.

One combined consumer artifact stores each distinct native payload once; large
per-job diagnostics and recovery checkpoints stay outside it. Completed jobs
are retained across resume. A frozen configuration binds inputs, executable,
partition and policies. An inherited lock prevents duplicate launches while an
orphaned native child still owns work. See the
[operator guide](../independent_owner_campaigns.md) for normal interruption,
RAM-triggered checkpointing and the orphan-supervisor limitation.

Rust owns the colored inline display, non-TTY logs, bounded JSON status and
read-only viewer. The display distinguishes reserved workers from measured
busy cores. Its progress bar counts completed jobs, not remaining symbolic
work or an estimated finish time. The Python example only prepares the input
configuration and optionally executes RustRed.

Independent source audits found no new correctness blocker in the pruning
bound, query partition, aggregate completion gate, resource ownership or
combined output. Release core tests passed 2,839 tests (32 existing ignored).
The final release application suite passed 669 tests (four ignored), including
25 supervisor/monitor tests. The final CLI passed real-PTY color, cursor
restoration, read-only Ctrl-C, `NO_COLOR`, `TERM=dumb`, one-shot and JSON checks.
Python steering checks passed 57 tests. The real ten-owner recovery test
paused with unfinished work, resumed the frozen campaign and completed all
28,517 native inspections. Its cancelled pre-pause diagnostic records were
bound to the retained checkpoint and each was subsequently discharged; they
were not treated as either lost work or a fresh failure. Completed receipts
were preserved. Recovery is a functional test, not a speedup measurement.

## Measurement protocol

All comparison commands use one frozen release CLI:

```text
SHA256 030e09ef661cc69b5acc0501376b8174a518eb95cb0bf0f656835e84613f6c60
```

The implementation is committed as `31170b4c2c6c719760fe0ad88bd1e97b902c3d88`.
Subsequent checkpoint edits only finalize documentation and these measurements.

Compilation and the post-run streaming audit are outside the timed boundary.
Whole-command time includes input freezing, each child's rule import,
traversal, checkpoint/result output and compact artifact publication. Child
preparation and traversal are also recorded separately. Summed per-job time
is not concurrent wall time. CPU time is waited-child user plus system time;
sampled aggregate peak RSS is not the maximum RSS of a single process.

The four-loop controls reuse the previous successfully completed FG/BMW/H/X
inputs: 124/134/314/328 owners and twice as many queries, including auxiliary
covers. They retain the original A<=19, R<=12, A-R>=7 entry requirement and
all descendants. FG/H/X auxiliaries are rank-12 orthants; BMW additionally
retains A<=19 for auxiliary support at least six. Inputs are byte-bound to
the earlier successful comparison, not redesigned for this benchmark.

The deliberately smaller five-loop control includes all 67 saved starting
owners but only 663 finite starting tuples: R=0, A<=max(support,9), A-R>=9.
The separate one-owner R<=2, A<=11, A-R>=9 control has 1,324 starting tuples.
Both retain all descendants and the full saved rule/route selection. Neither
is the live rank-15 campaign, nor an estimate of its total workload.

Four-loop comparisons use six CPUs and a 100 GB aggregate RAM guard;
five-loop comparisons use 50 distinct physical cores and a 500 GB guard.
Both reserve 20 GB of host/cgroup memory and have no elapsed-time or work cap.
The user's existing campaign remains active on a disjoint CPU set. These
are shared-host measurements, not isolated-machine benchmarks.

Raw local evidence is under
`TMP/joint-shards-checkpoint.h51BgS/benchmark-v2/`; the preceding preparation
directory contains an unused input selection corrected before any measurement.
The benchmark driver audits native records, initial-query coverage, aliases,
delegation ledgers, pool drain and original artifact payloads. Fresh runs must
have no uncommitted inspection diagnostics. A recovery test may retain exactly
its checkpoint-bound cancellation history only when each corresponding
inspection is canonically discharged after resume.

## Results and conclusions

### Four-loop controls

All eight full-family runs completed, with zero remaining frontiers or work.
Pruning off/on produced identical mathematical records and counters, excluding
timing/policy diagnostics. These selections exercise no routed masks, so
**none of their timing differences is evidence of a pruning benefit**.

| Family | Pruning | Command wall, s | Native traversal, s | CPU, s | Peak aggregate GB | Inspections |
|---|---|---:|---:|---:|---:|---:|
| FG | off | 17.584 | 12.743 | 46.302 | 1.146 | 98,869 |
| FG | on | 17.517 | 12.939 | 46.664 | 1.149 | 98,869 |
| BMW | off | 42.875 | 31.467 | 111.631 | 1.813 | 147,233 |
| BMW | on | 37.806 | 31.319 | 106.133 | 1.810 | 147,233 |
| H | off | 20.091 | 12.044 | 45.421 | 0.647 | 24,680 |
| H | on | 20.012 | 12.070 | 45.464 | 0.652 | 24,680 |
| X | off | 42.186 | 32.109 | 104.444 | 1.252 | 46,826 |
| X | on | 41.035 | 31.536 | 102.639 | 1.242 | 46,826 |

The topology-scheduling pilot uses the lexicographically first ten FG owners,
including their auxiliaries and the full 124-owner rule selection. Both modes
follow all descendants. This is a limited starting-owner selection, **not** a
finite tuple box: its auxiliary rank orthants retain unbounded positive powers.

| Scheduling | Command wall, s | CPU, s | Mean busy cores | Peak aggregate GB | Inspections |
|---|---:|---:|---:|---:|---:|
| One shared job, six workers | 2.972 | 2.752 | 0.926 | 0.184 | 20 |
| Ten queued jobs, two slots of three workers | 22.816 | 47.691 | 2.090 | 0.348 | 28,517 |

The latter consumes more cores but takes **7.68 times longer**. Job preparation
sums rise from 0.978 to 9.720 seconds. More importantly, lost shared auxiliary
coverage increases the inspected domains by over three orders of magnitude.
The combined artifact retains the same 27,017,805 native payload bytes once,
not ten copies. Its fresh-process shared traversal also completes.

The broader 124-owner independent FG recovery experiment was cooperatively
paused on observed pathological work inflation, not a preset deadline. At the
pause, 34 jobs were complete, two paused and 88 queued; 1,771,896 inspections
had finished and 16,569 domains remained. The resumed session lasted 840.871
seconds and reached 15.924 GB sampled aggregate RSS. This is **incomplete,
pathology-censored evidence**, not a completion timing. It had already used
17.92 times the shared full-family inspection count. One job spent roughly
659 of 697 seconds in ordered commit and performed over 2.2 billion containment
checks. This is consistent with the missing-sibling-cover explanation above;
it does not prove that the run would continue forever.

### Limited five-loop controls

The single-owner rank-two control completed both ways, retaining all descendants
and all 67 saved rule programs. Unlike the four-loop controls, it exercises
millions of routed masks.

| Joint pruning | Command wall, s | Preparation, s | Traversal, s | CPU, s | Mean busy cores | Peak aggregate GB | Native inspections |
|---|---:|---:|---:|---:|---:|---:|---:|
| off | 458.907 | 84.554 | 298.738 | 2,158.674 | 4.704 | 15.015 | 967,621 |
| on | 463.048 | 84.722 | 302.149 | 2,170.628 | 4.688 | 14.923 | 962,717 |

The new bound rejects 445,442 masks. Inspections decrease by 4,904 (0.51%), but
the single observed traversal is 1.14% longer and command wall time 0.90%
longer. Thus **no end-to-end speedup is demonstrated**. The option stays off
by default. This is not a statistically established slowdown either: the
observations are a single pair on a shared host.

Off/on logical-domain counts are 1,273,376 / 1,265,696. Examined mask counts are
12,896,445 / 12,909,884, and total pruned masks 9,524,196 / 9,960,266. A tighter
local cover can change subsequent subdivision and reuse, so these counters
need not all decrease monotonically. The extra pruning does not mean an equal
number of native inspections is avoided. Both final ledgers exhaust without
frontiers; original programs and terminal declarations remain unchanged.
The independent raw-record audits pass both runs. Route coordinate-cell work
decreases by 12,567,510 and route events by 422,631; these local savings still
do not establish a campaign speedup. Joint-pruned masks are already included
in the total pruned-mask count and must not be added a second time.

The all-67-owner rank-zero control completed in all three scheduling modes.
Every row below uses the same 663 starting tuples, rule selection and descendant
obligation. Joint pruning is off in this scheduling comparison.

| Scheduling | Command wall, s | CPU, s | Mean busy cores | Peak aggregate GB | Native inspections |
|---|---:|---:|---:|---:|---:|
| One shared job, 50 workers | 153.398 | 183.453 | 1.196 | 5.835 | 29,354 |
| 67 queued jobs, 10 slots of five workers | 736.671 | 6,195.851 | 8.411 | 57.849 | 116,041 |
| 67 queued jobs, 50 slots of one worker | 296.050 | 6,554.132 | 22.139 | 281.035 | 116,041 |

The ten-slot run is 4.80 times slower than shared execution and performs 3.95
times as many native inspections. Its preparation time sums to 5,699.427
seconds across 67 jobs, versus 83.870 seconds for the shared run. Traversal
sums are 39.290 versus 4.005 seconds; these sums are not elapsed parallel
traversal time. The control is dominated by repeating the full saved-program
preparation in each process. During preparation, each topology job primarily
uses one CPU, so reserving five workers for it does not produce five busy cores.

The 50-slot run reaches approximately 50 measured busy cores during its first
wave, then about 17 while the final 17 jobs prepare. It therefore demonstrates
the requested outer scheduling capability. But its whole-command average is
22.139 busy cores, and much of this activity is repeated import/preparation,
not mathematical reduction. Preparation sums to 6,064.136 seconds and traversal
to 59.620 seconds, with an 8.867-second longest traversal. More simultaneous
imports also increase peak RSS to 281.035 GB. It is 2.49 times faster than
10 slots, but **1.93 times slower than the shared baseline**, with the same
3.95-fold inspection inflation. High instantaneous utilization is not useful
speedup on this workload.

All completed runs pass independent raw-record and completion-ledger audits.
The 10-slot and 50-slot runs have exactly equal 132,763 canonical mathematical
records after excluding only record timing, and all 67 completion ledgers agree.
Their canonical semantic SHA256 is
`577251a25ceb26e7cfa2a5f9bd69488dde4f2bccaa3334673c04728a07bde77b`.
This is mathematical-record equality, not identical operational work: worker
counts differ, and the 50-slot run performs 451,347 additional containment
checks spread across 20 jobs. Other top-level integer counters agree.
The combined output preserves 67 unique native payload files containing
1,280,854,595 logical bytes, stored once regardless of the number of jobs.
Diagnostics and checkpoints are separate from that consumer artifact. Filesystem
compression is not counted as an algorithmic artifact-size improvement.

No full five-loop speedup or completion claim follows from these limited controls.

## Practical conclusion and stopping point

Both requested opt-ins are usable, tested and independently audited. Neither
changes the running production campaign, the algebraic rule payloads or the
default shared scheduler. This was a bounded implementation and measurement
task, not a new generation strategy or an unrestricted closure result.

The joint bound does remove avoidable local work, but the measured rank-two
case does not support the expectation of a large end-to-end gain. Its opt-in
status is important: the extra bookkeeping is not free, and tightening a
local cover can change downstream subdivision and reuse.

The topology queue removes the scheduling barrier between independent starting
owners, as requested. It cannot guarantee useful scaling: every process repeats
preparation, and independent coverage can require much more work than the shared
walk. The full-FG observation additionally exposes an abstract convergence risk,
not just startup overhead. These findings do **not** justify replacing the live
five-loop campaign solely to obtain a busier CPU display, and they provide no
credible full-campaign ETA. No additional optimization is started at this
checkpoint.

The [operator guide](../independent_owner_campaigns.md#optional-fresh-launch-using-this-workspaces-five-loop-inputs)
gives exact Nix/build/fresh-start commands, a shared-anchor alternative using
the same native supervisor, and frozen-checkpoint resume commands. Restarting
is a user decision; the existing production process is left untouched.
