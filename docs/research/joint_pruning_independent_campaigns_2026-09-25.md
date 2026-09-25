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
`B(n,m) -> B(n-1,m+1)` terminates for every concrete integer `n`, but an
unbounded region `B(n>=1,m=1)` can generate incomparable `m=2,3,...` slices.
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

Measurements are in progress. No full five-loop speedup or completion claim
follows from these four-loop results or from the implementation alone.
