# Epoch preparation helpers: full-A1 five-loop causal screen

## Outcome

**Two preparation helpers improve P2 locally but do not meet the preregistered
useful-work threshold in this single fresh-launch screen. Keep h0 as the
production default on this evidence.** h2 completes 3.13% more native inspections
and 2.52% more committed domains, below the 10% threshold, while using 5.27% more
sampled supervised-tree CPU and leaving 4.64% more pending domains.

P2 wall time falls 16.27%, or 17.15% per accepted obligation; source resolution
is the principal saving. More inspect/wait, P1, P3 and boundary time offset most
of it. This is an informative negative deployment result, not evidence that the
helper implementation is ineffective in every workload.

Both arms pass their declared **bounded causal-arm validity** checks. Neither
completes the frozen five-loop query scope. Cold verification deliberately uses
`--reinspect none`: its raw verdict is **INCOMPLETE**, not a closure certificate.
No actual resume, all-native reinspection, fixed-work-prefix speedup, repeated
benefit, or unrestricted family closure is claimed.

## Frozen workload, resources and timing boundary

- Same optimized CLI in both arms: source `d55cfb1cac48712ee82fff075f08990488ecec8c`,
  campaign profile opt-level3, fat LTO, one codegen unit; SHA-256
  `23d836d7b05fac6b007cc1adea84f04548d5bf0e0a94a28ca877f98942da8045`.
  Compilation took 3706.452 s and is outside both arm clocks.
- Exact original A1 saved input: 67 owners, 8,246 routes, all 183 query rows
  (116 required, 67 auxiliary), original order. Each arm checks all 78 immutable
  input files. No generation, scope narrowing, query amendment or prior
  checkpoint is used.
- Selection SHA-256:
  `5e900a163ee7f741b25136f2f41af023464d8bbfdb369d3c7d5fbf212de87f45`;
  query SHA-256:
  `42a0c62771b6e7c53cc937d46ad9505e33c282db31a8d6f64846ecbca749ef64`.
- Both arms reserve CPUs0–31 and W32. h0 has 31 inspectors, zero preparation
  helpers and one coordinator; h2 has 29 inspectors, two helpers and one
  coordinator. Inner numerical/thread pools remain capped at one.
- Identical Epoch oldest-prefix/FIFO, base window76, cut16, extra whole-result
  allowance1024, soft returned-result byte threshold256MiB, snapshot inspector
  lookup, G2 union reuse, unbounded work and stop-on-frontier policy.
- Requested RAM600GB, soft guard margin5%, host availability reserve150GB.
  Existing admission reduces effective caps when host availability requires it;
  these are safety guards, not resource-equalizing workload truncation.
- Each separate fresh process has an inclusive1800s clock starting before
  admission, hashes and preparation. Cooperative native pause is requested at
  1200s; native save/drain may continue afterwards. Forced drain at1500s would
  invalidate the arm. Structural cold read starts only before1500s, with
  1730/1760s cooperative/forced deadlines. No hard stop occurred.
- Order is h0 then h2. Same locks serialize both arms with other heavy research
  work; there is no compiler between them. The user-started production campaign
  remains on CPUs64–95 and LC2 on its existing reservation. Those campaigns and
  other host activity are **contention**, even with disjoint CPU sets. Foreign
  CPU contention is not independently measured; do not assume a quiet host.

This is an equal-resource, equal-stop-window **causal screen**, not an equal-work
benchmark: helper count changes inspector width and can change dispatch/snapshot
history and admitted graph work. Comparing different captured graph prefixes
cannot establish a matched-work speedup.

## Measurements

Times are seconds. GB means decimal10^9 bytes. Whole elapsed values include
their real save/drain and cold-read costs, not just selected phase sums.

| Metric | h0:31+0+1 | h2:29+2+1 |
|---|---:|---:|
| Inclusive arm | 1418.984 | 1427.234 |
| Native guard, including save/drain | 1207.969 | 1209.858 |
| Prepared input/map loading | 63.819 | 64.246 |
| Native traversal | 1138.826 | 1138.936 |
| Cold structural guard | 209.480 | 215.698 |
| Native inspections completed | 3,503,792 | 3,613,632 |
| Committed/sealed domains, including aliases | 6,340,699 | 6,500,658 |
| Scheduled domains | 9,110,510 | 9,398,925 |
| Pending domains at pause | 2,769,811 | 2,898,267 |
| Dependency edges | 106,047,965 | 107,564,567 |
| Accepted P2 obligations | 178,474,588 | 180,361,291 |
| Native errors / frontiers / abandoned obligations | 0 / 0 / 0 | 0 / 0 / 0 |
| Required / auxiliary query rows admitted | 116 / 67 | 116 / 67 |
| Required-query closure evaluated | No | No |
| Cold graph roots recursively closed, not fully re-inspected | 8 / 67 | 8 / 67 |
| Sampled supervised-tree CPU-seconds | 6298.09 | 6629.83 |
| Interval-weighted sampled native busy cores, including preparation | 5.223 | 5.489 |
| Supervisor sampled aggregate peak RSS, GB | 14.917 | 15.125 |
| Cold guard sampled peak RSS, GB | 8.985 | 9.090 |

Alias records are2,836,907 versus2,887,026; G2 records303,268 versus317,817;
partial records80 in each. Detailed Route-versus-Apply record counts were not
extracted from the large record stream. The modest whole-count changes do not
prove an identical or uniformly weighted work mix.

### Coordinator attribution

| Phase | h0 | h2 |
|---|---:|---:|
| Inspect / wait | 541.877 | 561.268 |
| P1 validation | 97.940 | 106.004 |
| P2 preparation | 312.469 | 261.632 |
| P3 publication | 67.801 | 84.163 |
| Boundary | 115.626 | 122.243 |
| Checkpoint | 3.063 | 3.575 |

| P2 subphase | h0 | h2 |
|---|---:|---:|
| Source resolution | 221.497 | 178.353 |
| Canonical deduplication | 20.115 | 17.934 |
| Antichain | 7.784 | 4.316 |
| Representative selection | 4.435 | 2.704 |
| Reverse containment/retirement | 47.518 | 43.034 |
| Canonical transfer | 4.495 | 4.399 |

Source tasks are7,113,646 versus7,327,728; waves3,584,765 versus1,870,447.
The existing source code processes at most256 misses per block, with wave widths
2 for h0 and4 for h2. Many tasks are headers. The measured local reduction is
consistent with amortizing and parallelizing preparation, but this pair does
not separately attribute batching savings, helper speedup and changed work.

Nested diagnostics: prefix wait499.138→523.680s; publication wait41.923→36.637s;
snapshot refresh88.118→96.107s. These are **not additional wall time** to add to
the phase table. Inspector lookup1139.962→1300.974s is summed across inspectors,
not coordinator wall. Accepted lookup queries178,474,588→180,361,291 and stored
hits162,715,550→164,252,651 likewise belong to different captured prefixes.

The 256MiB returned-result setting is a soft admission threshold: in-flight
returns reached approximately568.9MB in each arm. It must not be described as a
strict memory cap.

### CPU measurement limitation: the outer pilot adapter

The ARM resource summary samples the supervised tree and **excludes the outer
Python adapter**. Root noticed substantial adapter CPU during h2. A bounded
live `/proc` snapshot recorded at least615.74 adapter CPU-seconds later in h2;
this is a lower bound, not its final total. At that point its already-waited
supervisor/native descendants totaled6630.11 CPU-seconds. Its cold child was
still running; that snapshot is not a complete cold or all-process total.

h0 used the same independently audited polling/RAM/process-ownership code, but
its outer adapter had already exited before this extra observation and its CPU
total was not persisted. Therefore **a complete all-process CPU comparison is
unavailable**. No live harness was modified to improve these measurements, and
no claim that the reported native-tree CPU is total experiment CPU is made.
The extra h2 snapshots are retained, including the final absence observation.

## Acceptance and interpretation

Both raw native runs stop with exit4, joined workers, `engine_certification_void`
false, complete state in CP6 generation1 and zero abandoned obligations. Cold
readers authenticate the request/owner/generation bindings and all183 queries.
They report exit9 and **INCOMPLETE**, with zero violations/suppressed violations,
zero natives selected for reinspection and zero roots independently re-inspected.
The structural cold checker covers all303,268 and317,817 respective G2 union
records without a coverage disagreement. This does not substitute for native
reinspection, rule-source replay or required-query closure.

The preregistered promising threshold was at least10% more useful completed
inspections and/or committed obligations, lower P2 wall per accepted obligation,
and no disproportionate queue/frontier/memory regression. The useful-work part
**fails**: native+3.13%, committed+2.52%, accepted obligations+1.06%. Pending
grows4.64%, sampled tree CPU5.27%, peak RSS1.39%. Thus no production restart or
default helper change is recommended from this single pair. The narrow P2
improvement should be preserved as evidence rather than extrapolated into a
whole-campaign win.

Both arms used fresh first-twenty-minute windows. Production's later phase mix
is demonstrably nonstationary: observed roughly five-minute intervals had P2
shares28.6%,25.9%,26.9%,28.0%,39.4%,24.8%,27.6%,42.8%,41.7%; P1 sometimes reached
about20%. These are read-only, distinct native-timestamp deltas, not counts of
repeated heartbeat labels. They justify keeping later-cut behavior open, **not**
launching another control or asserting h2 must help later. No extra experiment
was launched after this pair.

## Production observation and resource release

The repaired user campaign continued throughout. Its first natural hourly CP6
generation1 was observed saved, resumable, not paused, with zero abandoned
obligations:10,485,584 native inspections,16,210,614 committed domains,
6,111,918 pending domains and488,344,468 committed events. Its manifest digest
reported by the monitor is
`a37a5d027fb5178359322008d9734c20d11db6fde487b3a909270cafeb5edb8f`.
No save was forced and no production checkpoint was opened, restored or decoded
by this lane. This is an observed save receipt, not an independently verified
production resume or closure claim.

All h0 adapter/supervisor/native/cold PIDs3828063/3828194/3828196/3951470 and
h2 PIDs3976441/3976751/3976753/4074948 were checked absent. Owned child groups
drained and heavy/pilot locks were independently checked free. The next build
agent was explicitly awakened only after the entire pair had drained; prose
analysis did not hold the resource slot.

## Reproduction and retained evidence

Workspace evidence root:
`TMP/postlaunch-20260930/full-a1-preparation-h0-h2/`.
`PLAN.json` remains the immutable unbound preregistration, SHA-256
`7f5bcd1cac2d85b9b175fc12a745961733c31711d589dd595e3d0e5c05c9f495`.
The separate `BOUND_PLAN.json` SHA-256 is
`49a10d553ba13cdeee39f9ce3673b354dbb78836582b1f57cf58a1a4b15af970`.
`run_arm.py` SHA-256 is
`7e95b696a87f3e3f0303eb29a0be3b23adceea7b02a39ee989c4871678ab9672`.
It imports the unchanged audited lifecycle controller with SHA-256
`486464a5dfe1551a2b1f69a30a774510f3b26e46caea1d5a0bde253583424dd4`.

Exact historical launch commands, each issued separately only after root's
resource grant and predecessor drain, from `/common/dev/rustred`:

```sh
setsid --fork taskset -c 0-31 \
  /nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python -B \
  TMP/postlaunch-20260930/full-a1-preparation-h0-h2/run_arm.py \
  --plan TMP/postlaunch-20260930/full-a1-preparation-h0-h2/BOUND_PLAN.json \
  --arm h0 --grant root-approved-h0-h2-causal-screen

setsid --fork taskset -c 0-31 \
  /nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python -B \
  TMP/postlaunch-20260930/full-a1-preparation-h0-h2/run_arm.py \
  --plan TMP/postlaunch-20260930/full-a1-preparation-h0-h2/BOUND_PLAN.json \
  --arm h2 --grant root-approved-h0-h2-causal-screen
```

These are historical commands, not instructions to overwrite evidence or restart
production. The adapter intentionally refuses existing arm directories. A new
replication needs separate fresh output paths, frozen bindings and a new grant.
The plan contains both complete supervisor/native and cold-reader arguments.
The license is inherited from the environment and is not recorded here.

Raw evidence per arm: `BOUND.json`, `ARM_RESULT.json`, `native-guard/result.json`,
`run/result.json`, `run/supervisor-result.json`, `run/resources.jsonl`,
`cold-guard/result.json`, `cold-structural.json`. h0 ARM SHA-256:
`7970a1049f03c0e4dd0574a2428fc7e388415332047025d73acb15e28cb71b56`;
h2 ARM SHA-256:
`6f5d2030719ae629e6ffb2634dc2488d8c736431c26ebb0224e438f2d52ea8de`.
`PAIR_RESULTS.json` retains extracted metrics and percent changes;
`LIVE_TRIAGE.json` retains bounded incremental production observations;
`H2_OUTER_CPU_SNAPSHOTS.json` preserves the CPU limitation evidence.
Percent changes use100×(h2/h0−1); normalized P2 divides each arm's P2 seconds by
its own accepted-obligation count before taking that ratio.

Independent reviewer `/root/final_requirements_audit` accepted both arms' small
receipts, binding/cold/drain interpretation, and the final report arithmetic.
The review confirms the threshold is not met and preserves the different-prefix,
single-pair, cold-INCOMPLETE and outer-adapter CPU limitations above.
