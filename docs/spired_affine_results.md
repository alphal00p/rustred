# SpIRed coupled-case integration and release results

Date: 2026-09-14. The current native-Symbolica source port completes two more
unchanged reference workloads. This is exact `solveSector` parity, not a
certified closing artifact or independence proof for its finite residuals.

## Complete reference comparison

| Workload | Selected sectors | Rust rules | Reference rules | Exact residual keys |
| --- | ---: | ---: | ---: | ---: |
| `fam1_12` | 40 | 1,104 | 1,105, including one empty case | 32 |
| `fam1_111` | 132 | 10,333 | 10,333 | 26 |

Both original requested manifests run successfully at one and six workers.
All nonempty required domains, sector signs, exact symbolic RHS coefficients,
and exceptional guards match. The two and three preliminary cut rules also
match. Independent native C++ database readers check residual sector/integral
keys, not merely counts. Reference data is read only after generation; neither
FORM rules nor C++ coefficients enter Rust search.

The omitted `fam1_12` case requires `2*n5-2*n6=1` in one-based C++ notation.
Exact integer divisibility proves that it has no solution. Requiring that
meaningless rule to meet a raw count would weaken, rather than improve,
acceptance. The comparator checks a one-to-one match of every remaining domain;
it does not excuse arbitrary missing rules.

Serial/six-worker output identity and prior coordinate-only regression output
are checked independently. All 16 correctness processes succeed: K1/K3 top
sectors and the complete supplied `vac3`, `fam1_11`, `fam1_12`, `fam1_111`,
`bc4PMRad1`, and selected `fam_cosmo` workloads, at both requested worker counts.
The one-sector fixtures use one actual worker even when six are requested.
The independent audit checks all 582 sector files and 27,462 generated rules,
307 serial/parallel payload pairs, and 131 prior-regression payloads.

## What removed the blockers

`Case` keeps coordinate cases inline and shares immutable native affine charts
through `Arc`. Sector recursion, containment, guards, and target matching now
use the same exact domain. Ordinary source translations remain unrestricted;
their coefficients are shifted before native chart substitution. Only a
winning target must be tangent to the chart before recentering. In particular,
`n0=n1` must not identify the distinct columns `I(n0+1,n1)` and `I(n0,n1+1)`.

The extra `fam1_111` sector `111110100010011` needed a joint polynomial
simplification. Symbolica's exact rational Gröbner basis reduces its actual
three-equation conjunction to `n4=n14=1` (one-based C++ indices). The fallback is cold: multiple
exceptional equations, with at least one nonlinear, must first defeat ordinary coordinate
substitution. Native primitive normalization and one coordinate retry follow;
there is no custom elimination or bounded integer enumeration.

At this measured checkpoint, fractional canonical charts were still rejected.
The subsequent rational-chart implementation described in
[the port status](spired_port.md) removes that representation restriction
without claiming general integer-lattice or coupled sector-inequality
feasibility. Unresolved nonlinear cases remain explicit failures.
Rational-polynomial reconstruction remains deferred to Symbolica.
See [the algorithm/API description](spired_port.md) for these boundaries.

## Seven-pair `fam1_12` performance comparison

Fresh processes alternate Rust/C++ order, using the same physical CPU prefixes
`2`, `2,3`, `2,3,4,5`, and `2,3,4,5,6,8` on the shared AMD EPYC 9754 host.
Rust uses the release executable built with `--release --locked`; C++ is the
unchanged `-O3 -DNDEBUG` build. No compilation or oracle comparison is timed.

| Workers | Rust process median | C++ process median | Rust campaign median | C++ campaign median |
| --- | ---: | ---: | ---: | ---: |
| 1 | 740.7 ms | 1,883.1 ms | 706.9 ms | 1,861 ms |
| 2 | 470.8 ms | 945.2 ms | 437.4 ms | 927 ms |
| 4 | 302.1 ms | 483.1 ms | 266.1 ms | 465 ms |
| 6 | 291.7 ms | 384.0 ms | 260.4 ms | 365 ms |

All 56 processes succeed. All 3,500 checked mathematical files match their
previously validated engine baseline; all 28 Rust runs preserve native residual
keys and non-timing search statistics. Every paired Rust run is faster here.
Median paired Rust/C++ wall ratios are 0.395, 0.496, 0.626, and 0.736. All
outliers are retained. Six-worker Rust process speedup is 2.54×, versus C++
4.90×; lower absolute time is not a claim of equally good parallel scaling.

Median Rust CPU time is 0.71/0.74/0.74/0.85 s and peak RSS is approximately
9.0 MiB across 1/2/4/6 workers. C++ CPU medians are 1.86/1.85/1.86/2.03 s.
The monotonic process timer includes the same time/affinity/timeout wrapper;
campaign timers cover each program's sector loop plus output. C++ writes
compressed binary files as well as text; Rust writes text and statistics.
These are complete-example comparisons, not isolated arithmetic-kernel timings.
The shared host is not an exclusive hardware-scaling experiment.

### Measured parallel scheduling opportunity

Across the seven serial runs, median summed `fam1_12` solving is 684.704 ms:
496.136 ms symbolic search (72.5%), 143.736 ms numerical search (21.0%),
24.412 ms exceptions, and 11.713 ms geometry. Geometry is not the dominant cost.
The two largest sector jobs are `110011100` (131.464 ms) and `110000111`
(109.519 ms); both contain required affine cases. The active-count-first
worklist places them 40th and 37th out of 40, creating a plausible tail.

An offline list-scheduling calculation using measured per-job serial costs
predicts 202.301 ms for that ordering versus 132.816 ms for the already existing
input-order policy on six workers. These are **estimates**, not timings of an
alternate implementation: Rayon does not promise job start order, dispatch
timestamps were not recorded, and the actual campaign median is 260.363 ms.
The next controlled scheduling experiment should test `InputOrder` before
adding more infrastructure or fixture-specific cost rules.

That experiment has now run, using the release example's explicit
`active-first`/`input-order` switch. Seven fresh runs of each policy at each of
one and six workers, alternating policy order and with builds and our other
benchmark jobs idle, give:

| Workers | Policy | Process median | Campaign median | Process CPU median |
| --- | --- | ---: | ---: | ---: |
| 1 | active-first | 733.3 ms | 699.2 ms | 0.70 s |
| 1 | input-order | 738.7 ms | 703.5 ms | 0.70 s |
| 6 | active-first | 272.3 ms | 239.6 ms | 0.79 s |
| 6 | input-order | 380.2 ms | 343.2 ms | 0.74 s |

The measured six-worker result contradicts the simple offline list-scheduling
estimate. Rayon work splitting/stealing is not a list scheduler; these
measurements alone do not identify the full cause. **The default remains
active-first.** All 28 runs succeed, with 1,176 byte-identical mathematical
files and identical non-timing per-sector search statistics. Peak RSS medians
are 9,240–9,264 KiB. No C++ comparison is included in this scheduling-only
batch, and its times must not be paired with a different historical batch.
The binary contains the scheduling switch but predates rational-chart support.
Evidence: `target/spired-scheduling.xdfqrm/`.

In the preceding Rust/C++ paired batch, summed sector solve medians rise from
684.704 ms serial to 976.647 ms with six workers, while process CPU medians
rise from 0.71 to 0.85 s. These records
show cost inflation but do not identify its cause; no claim about host load,
allocator contention, or native locks is made without further profiling.

## Three-loop fresh-process pilot

One additional fresh Rust/C++ pair at each of one and six workers gives:

| Workers | Rust process wall | C++ process wall | Rust campaign | C++ campaign |
| --- | ---: | ---: | ---: | ---: |
| 1 | 27.537 s | 59.018 s | 27.513 s | 58.989 s |
| 6 | 5.700 s | 10.709 s | 5.676 s | 10.682 s |

These are **single pilot samples, not medians or a completed repeated
performance gate**. Launch order is Rust1, C++1, C++6, Rust6, with the same
affinity prefixes and timing boundaries as above. Oracle checking is disabled.
All four runs succeed; all 806 mathematical files and 26 native residual keys
match their validated engine baseline, with unchanged executables and inputs.

Serial/six-worker process CPU totals are 27.36/30.47 s Rust versus
58.67/58.77 s C++; peak RSS is 15,416/33,832 KiB Rust versus
15,464/27,676 KiB C++. Thus this pilot does not claim lower Rust memory at
six workers. Parallel worker-local exact state remains a scaling cost.

The serial pilot spends 27.231 s solving, of which 25.036 s is symbolic search
(about 92%), 1.402 s numerical search, 0.341 s exceptions, and 0.350 s geometry.
Preconditioning and writing add 0.065 s and 0.200 s. Six-worker summed solving
is 30.342 s, including 27.864 s symbolic search; summed worker durations are
neither process wall nor process CPU time. Primitive-level GPLU/GCD/replay
attribution and a repeated three-loop timing study remain future work.

### Separate initial correctness-run phase observations

The first current serial correctness run generates all 10,333 `fam1_111`
rules in a 43.768 s campaign, with a 3.668 s post-generation oracle check kept
separate. Summed solving is 43.356 s: 39.800 s symbolic search (91.8%),
2.250 s numerical search, 0.564 s exception extraction, and 0.558 s geometry.
Preconditioning adds 0.099 s and writing 0.308 s. The run visits 10,324
symbolic cases, generates 790,041 symbolic rows, and attempts 35 fixed cases;
nine fixed cases reduce and 26 remain residuals.

The six-worker correctness campaign takes 8.728 s, with 2.295 s separate
oracle checking. Correctness runs retain all solutions for comparison; their
process memory is not an oracle-free production-memory measurement. These
first-run counters are not a paired C++ benchmark or primitive-level attribution
to native GCD, GPLU, or exact lifting.

## Validation and remaining work

### Rational-chart follow-up

The next implementation slice admits exact rational computational charts while
retaining the original integer equalities and sector signs. It passes 151
focused solver tests, 42 example tests, workspace all-target checking, and a
fresh optimized build. Independent adversarial checks cover coefficient scale,
on-case poles, parity, tangent shifts, simultaneous substitutions, physical
integral-key identity and canonical queue order.

All 16 established release regressions also pass again: 582 completed sector
jobs and 27,462 generated rules. Independent audits find all 614 mathematical
files unchanged from the previous baseline, all 307 serial/six-worker pairs
identical, and all non-timing search counters unchanged. The 12 oracle-enabled
runs check 27,454 exact equations with required domains, signs and guards; the
remaining eight rules are the supplemental K1/K3 top-sector runs. Residual keys
match the previously independently decoded native baselines. Evidence:
`target/spired-rational-regression.KO3jlH/`.

A full `fam1_112` rerun now produces 422 of 436 sector outputs. One newly
completed sector passes complete reference checks; the other has a candidate
case-count discrepancy. The full workload still exits with an explicit
nonlinear-geometry error. See the [updated census](spired_pm_acceptance.md)
for exact counts and limitations. This is progress, not full-manifest parity
or a certified family-closing artifact.

### Initial integral-chart checkpoint

- 142 focused solver tests and 39 example tests passed at that checkpoint.
- Workspace all-targets checking and the release example build pass.
- Separate agents audit source translation, domain ownership/subsumption,
  native joint normalization, and the complete-domain oracle independently.
- `fam1_112` and the supplied ordering studies remain required. No complete
  PM-suite acceptance, new certified K6 artifact, or new Vakint acceptance
  milestone is claimed by this checkpoint.

Ignored local evidence:

- `target/spired-affine-integration.R5azw2`: current full correctness runs.
- `target/spired-affine-residuals.A34g6Q`: native readers and residual keys.
- `target/spired-affine-paired.qP427C`: paired release runs and output audits.
- `target/spired-fam111-pilot.0cypyo`: four fresh-process three-loop pilot runs.
- `target/spired-pm-remaining.XyEHkz`: original saved C++ reference exports.

Reference sources, exports, local notes, and runtime license values remain
uncommitted. Reproduction commands are in [the Rust examples](../examples/rust/README.md).
