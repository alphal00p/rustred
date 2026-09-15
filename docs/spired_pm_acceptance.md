# Supplied SpIRed PM acceptance census and implementation recipes

Snapshot: 2026-09-15. This is a census of the supplied example inputs and
reference output, not a claim that all examples have been ported. The complete
`vac3`, `fam1_11`, `fam1_12`, `fam1_111`, `bc4PMRad1`, and selected `fam_cosmo` workloads now pass
exact Rust/C++ equation, guard, and residual checks. Reference source and binary data stay under ignored
`vendor/spired/`; no confidential PDF or vendor source is reproduced here.
See [the port status](spired_port.md) for implemented capabilities and measured
Rust/C++ comparisons.

## Acceptance matrix

Here $L$ is the number of loop momenta, $E$ the number of external momenta,
and $K=L(L+1)/2+LE$ the number of denominator/ISP coordinates. Sector strings
list coordinates from left to right, beginning at denominator zero. “Sources”
counts raw ordinary IBPs plus Lorentz-invariance identities, **before** cut
preconditioning or elimination; it is not the rank of that frame.

| Supplied generator | L / E / K | Raw sources: IBP + LI | Removed linear cuts | Actual requested sectors | Numeric seed depth | Ordering |
| --- | --- | --- | --- | --- | --- | --- |
| `fam1_11.cpp` | 2 / 3 / 9 | 10 + 3 = 13 | 0, 1 | all 40 in `nonZeroSectors_1_11.dat` | 2 | default |
| `fam1_12.cpp` | 2 / 3 / 9 | 10 + 3 = 13 | 0, 1 | all 40 in `nonZeroSectors_1_12.dat` | 2 | default |
| `fam1_12_exp.cpp` | 2 / 3 / 9 | 10 + 3 = 13 | 0, 1 | the same 40, **5,040 orderings each** | 2 | exhaustive permutations of coordinates 2–8 |
| `fam1_111.cpp` | 3 / 3 / 15 | 18 + 3 = 21 | 0, 1, 2 | **132** in `neededSectors_1_111.dat`, not all 1,792 nonzero sectors | 2 | default |
| `fam1_112.cpp` | 3 / 3 / 15 | 18 + 3 = 21 | 0, 1, 2 | **436** in `neededSectors_1_112.dat`, not all 1,792 nonzero sectors | 3 | 16 sector-specific overrides below |
| `fam1_112_exp.cpp` | 3 / 3 / 15 | 18 + 3 = 21 | 0, 1, 2 | only `111000101110111` | 3 | one explicit “long” permutation |
| `bc4PMRad1.cpp` | 2 / 2 / 7 | 8 + 1 = 9 | none | all 38 in `bc4PM_Rad1_nonZeroSectors.dat` | 3 | default |
| `fam_cosmo.cpp` | 2 / 1 / 5 | 6 + 0 = 6 | none | only `10101` | 3 | default |
| `vac3.cpp` | 3 / 0 / 6 | 9 + 0 = 9 | none | all 38 in `nonZeroSectors_vac3.dat` | 3 | default |

There is **no supplied one-loop/2PM C++ generator example** in this directory.
RustRed's existing one-loop tests are useful supplemental coverage, but cannot
be described as a comparison against a nonexistent supplied fixture. The
`fam1_...` names do not encode the number of loops in their initial “1”.
The PM-named fixtures are not vacuum families; `fam_cosmo` is also a genuinely
external-momentum, multiscale fixture.

The source-count formula used above is $L(L+E)+E(E-1)/2$.
All active examples enable LI generation; `generateIBPs()` defaults to
`generateLIs=true`. The integer passed to the C++ solver constructor is
the numeric seed-depth bound, **not** a worker count or a proof of master
minimality.

## Exact manifest census

### Reference ordering-sweep driver correction

The user authorized a narrowly scoped correction to the local C++
`fam1_12_exp.cpp` benchmark driver: resolve sector ordering IDs serially before
entering the parallel sweep. Previously, each lookup could inspect the shared
ordering table while other workers replaced its entries. Each worker still
owns one sector and executes all 5,040 permutations; solver code, inputs,
requested threads, dynamic schedule, timer boundaries, and output are unchanged.
The corrected driver compiles successfully with the original optimized GCC
recipe; solver-header, solver-implementation, and shared-library hashes remain
unchanged. Its 201,600-job sweep has not yet been run. Its original source,
complete diff, and before/after hashes are retained under ignored
`target/spired-exp-driver-audit.movGDg/`; no reference source is pushed.
The separate build log and hashes are in `target/spired-fam12-exp-build.Ab7SOc/`.

A separate, as-yet-unverified concern remains in the reference's all-ones
sector: source ordering may not refresh when a permutation changes but its
ordering ID stays zero. No solver correction has been made. The sweep needs
independent per-order equation checks before all its statistics can be trusted;
the authorized lookup correction does not resolve this separate question.

### First full `fam1_112` diagnostics

Both implementations were launched on the unchanged 436-sector manifest with
all 16 supplied ordering overrides and numerical seed depth three. The Rust
release run produced 420 sector files, then returned an unsupported affine
chart error; it did **not** finish successfully. The first reported case
contains `2*n11-n14=4` in one-based C++ coordinates (source variable map
`[d,x,n0,...,n14]`). Its canonical rational substitution is
`n11=(n14+4)/2`; rejecting that representation is not evidence
that the integer case is impossible. The 16 missing sector outputs remain
failures, not implicit terminals.

Licensed isolated reruns of those 16 sectors confirm three fractional-chart
admission failures and 13 unsupported nonlinear conjunctions, with no timeouts.
The nonlinear frontier includes products that factor into a union of affine
cases after imposing sibling equalities. Supporting rational charts alone is
therefore not expected to complete this workload; exact conjunction reduction
and OR-branch expansion are also needed. Evidence:
`target/spired-fam112-isolated-licensed.5e73H1/`.

The original, unchanged C++ run hit its 600-second diagnostic limit with
288 complete sector statistics rows, 48,189 rules, and 256 residual records.
Those are partial records, not a complete or unique master census. A completed
sector (`111000101010111`) alone took 323.845 seconds and produced 489 rules
with 1,130,953 coefficient leaves.

Rust's failed run took 285.54 seconds with 876,632 KiB peak RSS. C++ was
censored at 600.116 seconds with 1,312,772 KiB peak RSS. They ran concurrently
on disjoint CPU sets, alongside some build activity: **these are diagnostic
figures, not a matched performance comparison.** No full `fam1_112` acceptance
claim follows. Evidence: `target/spired-fam112-first.8b69aX/` and
`target/spired-fam112-reference.JiWS6v/`.

### Post-rational-chart full rerun

The new release run completes 422/436 sector files, containing 66,904 rules
and 329 residual records, before returning a nonlinear-geometry error. All
previous 420 files and the preliminary cut rules remain byte-identical.
The two new completed sectors are `111000010001111` and `111000011100111`.
The first originally fractional case, `111000001010111`, advances past that
restriction but encounters a later nonlinear conjunction. Fourteen sectors
remain without a completed output.

Independent single-sector generation followed by exact native comparison gives:

| Sector | Rust outcome | Reference comparison |
| --- | --- | --- |
| `111000001010111` | Later nonlinear error | Unavailable; generation did not finish |
| `111000010001111` | 349 rules, no residuals | **Not passed:** reference has 347 nonempty rules plus one proved-empty rule; two Rust cases are sector-empty; exact residual keys agree |
| `111000011100111` | 398 rules, eight residuals | **Passed:** all exact coefficients, required domains, guards and residual keys; reference has one additional proved-empty rule |

All three preliminary cut rules compare exactly where scalar generation
finishes. The native residual reader is restricted to the three reference
files whose completion is individually established; no interrupted database
is treated as valid. Both extra Rust cases in `111000010001111` require
`n3=n12` (zero-based), but this sector requires `n3<=0` and `n12>=1`.
They are therefore exactly sector-empty. That recorded run preceded the
generic sign-bound correction. Fresh release reruns now discard those two
cases and pass all 347 exact reference coefficient/required-domain/guard
comparisons at both one and six workers; the 398-rule sector likewise passes
again. The corresponding residual counts are zero and eight. All 20 release
regression runs pass. Evidence: `target/spired-sign-bounds.vcDRFN/`.
The strict comparison was rerun, not waived. A new complete 436-sector run
has not followed this narrow correction; the nonlinear frontier remains open.

The full diagnostic takes 260.812 seconds wall, 1,425.45 CPU seconds and
879,688 KiB peak RSS. It runs concurrently with regression/build work, and
neither it nor the partial C++ run supports a paired speed comparison.
Evidence: `target/spired-fam112-post-chart.EDZPa7/`. The next exact
[exceptional-case slice](spired_exceptional_case_plan.md) must retain normalized
coupled equations and split factorized conjunctions into complete alternatives.

That standalone service now resolves all 13 captured nonlinear conjunctions:
two proved-empty domains and 15 explicit branches across the other eleven,
with zero incomplete inputs. This is exact local geometry validation only.
Queue integration now passes all 180 solver tests and all 20 fresh release
regressions, including 939 byte-identical mathematical-file comparisons.

### Post-disjunctive-queue full rerun (2026-09-15)

The unchanged 436-sector manifest reaches **435/436 observed complete rule
sections**, containing 72,355 rules and 390 provisional residual records, before
the 600-second cap (exit 124). No terminal algebra/geometry error was emitted.
Only `111010100001111` lacks output. Its last reported symbolic target is
`I(1,1,1,0,2,0,1,0,0,0,0,n11,n12,n13,n14)`, with 85 other cases pending;
207 case solves have been reported. The progress line does not print coupled
equalities or distinguish modular discovery from exact materialization, so it
does not identify the entire remaining case or its algebraic bottleneck.

All prior 422 sector outputs are present; 413 are byte-identical and nine
changed. Independent block-level audits find exactly 14 removed cases, no
added or altered common rule blocks, and identical residual tails in all nine
changed files. Thirteen removed affine domains contradict their sector signs.
The remaining coordinate child in `111010100011011` is covered by a retained
broader rule: its sole exception is `n4+2*n10+2*n11=4`, whereas all three powers
are positive, so the left side is at least five. No domain is lost by these
removals; this does not assert algebraic equality of different applicable
rules. The preliminary cut rules are unchanged.
Because the process was censored, a well-formed residual-section tail is not
proof that every callback flushed all its residuals. These are observed counts,
not a complete family result or a certified artifact.

Elapsed wall time is 600.163 s, aggregate CPU time 2,828.64 s, and peak RSS
1,767,052 KiB. This capped run is not a matched performance comparison with
the incomplete C++ campaign. Frozen release binary, complete manifest,
16 ordering overrides, regression checks and censoring evidence are retained
in `target/spired-disjunctive-queue.RLesUd/`.

### Isolated remaining sector and native profile (2026-09-15)

An isolated fresh run of `111010100001111` with the same frozen release
executable, original manifests and ordering overrides, numerical depth three,
unbounded symbolic search, and one worker on CPU 10 also hits its
**1,200-second diagnostic cap**. It produces no completed sector rule file.
All 415 case/rule progress lines match that sector's earlier full-run log:
208 cases started, 207 solved, the same final visible integral pattern and
85 other cases pending, with no numerical-search start or algebraic error
before censoring. The full workload consequently remains at 435/436 observed
outputs; this diagnostic does not establish nontermination or closure.

User-space perf sampling identifies the long tail more precisely. The full
56,901-sample profile records **99.92% inclusive** in
`solver::discovery::exact_materialize` and native rational-polynomial
`SparseRowReducer::add_row`. Symbolica polynomial `heap_division` accounts for
**81.18% self / 87.14% inclusive**, with `gcd` at **41.97% inclusive**.
Inclusive percentages overlap and must not be added. A separate 709-sample,
15-second attachment to the actual running process likewise observes all
samples under exact materialization. Thus the measured delay is native exact
rational-polynomial elimination, not the nonlinear exceptional-case queue
or continuing modular discovery. Frame dimensions and coefficient growth
were not exposed by this frozen executable and remain the next measurements.

The primary cap wraps the profiler as well as the solver. Including the final
profiler flush, launch-to-exit wall time is **1,204.485 s**, with **1,196.32 CPU
seconds**, **951,160 KiB peak RSS**, and exit 124. These instrumented,
shared-host figures are not paired Rust/C++ performance measurements. The old
C++ campaign has no completed statistics for this sector and cannot serve as
a completed comparison. No source or reference algorithm changed during these
runs; input hashes are unchanged. Evidence, complete profiling commands,
setup failures and permission-free mmap workaround, reports and caveats are
in `target/spired-fam112-last-sector.DYx7uY/RESULTS.md`.

### Refreshed observation and reference-domain gate (2026-09-15)

The release containing coarse search/phase observations passes all 20
established one/six-worker regressions again, with 939 byte-identical
mathematical-file comparisons and unchanged native residual keys. These
established comparisons require zero covered-reference waivers.

The separately checked `111010100011011` sector generates 217 unchanged
rules and zero residuals. Its 220 completed native reference rules partition
into **217 exact matches, one empty required domain, and two covered nonempty
subfaces**. Both subfaces belong to the retained broad rule whose only
exception is `n4+2*n10+2*n11=4`, impossible for positive powers. The original
forecast of two empty/one covered was rejected by the post-audit; independent
inspection of the original reference confirmed the correct partition. The
initial failed assertion is preserved; no generator or reference was altered.

A separate 60-second diagnostic now exposes the remaining sector's full
case: the previously displayed pattern has `required=true`, so it is genuinely
coordinate-only. Exact lifting starts 1.504412 s after launch, approximately
0.135356 s after that case starts, with **298 selected source rows**. The modular
discovery state has 2,364 pivots, 4,405 columns, 21,677 U nonzeros and 6,929 L
entries; these are not the compact exact frame's column/nonzero counts.
The run remains in exact lifting until its cap, preserving all 415 prior
case/rule events in order. It produces no completed sector output. Evidence,
independent checks and the reference-domain explanation are retained in
`target/spired-observation-regressions.hITFfz/RESULTS.md`.

The current release runs of `fam1_12` and `fam1_111` complete their full
unchanged requested manifests, 40 and 132 sector jobs respectively. Exact
domain/guard/coefficient comparisons match 1,104 nonempty and 10,333 reference
rules, with 32 and 26 exact residual keys. The single extra C++ `fam1_12`
rule has a proved integer-empty required domain. Admitted affine cases now
flow through search and the queue; native joint-ideal normalization resolves
the extra finite polynomial conjunction. See the [affine results](spired_affine_results.md).
`fam1_112` and the ordering sweeps remain outstanding; source-port acceptance
does not establish a certified family-closing artifact.

The active whitespace-separated manifests were parsed as Boolean coordinate
rows. All rows have the expected width; all individual files are duplicate-free;
each zero/nonzero pair is disjoint; every requested “needed” row belongs to its
corresponding nonzero list.

| Family | Explicit zero | Nonzero | Chosen by ordinary generator | Universe accounted for |
| --- | ---: | ---: | ---: | --- |
| `1_11` | 88 | 40 | 40 | all $2^7=128$ sectors with both cuts present |
| `1_12` | 88 | 40 | 40 | all $2^7=128$ sectors with both cuts present |
| `1_111` | 2,304 | 1,792 | 132 | all $2^{12}=4,096$ sectors with three cuts present |
| `1_112` | 2,304 | 1,792 | 436 | all $2^{12}=4,096$ sectors with three cuts present |
| `bc4PMRad1` | 90 | 38 | 38 | all $2^7=128$ sectors |
| `vac3` | 26 | 38 | 38 | all $2^6=64$ sectors |
| `fam_cosmo` | intentionally empty | no manifest | 1 | no full-family census supplied |

For removed-cut PM families, sectors missing any cut are additionally zero by
the cut convention; they are not omitted nonzero targets. A Rust loader must
preserve both that convention and the supplied explicit zero list. “Run all
supplied requested sectors” and “close all nonzero sectors of this family” are
different claims for `1_111`, `1_112`, and `fam_cosmo`.

## Kinematics and ordered denominator recipes

Write $k_i$ for loop momenta and $D_j$ for zero-based denominator
coordinates. Every fixture keeps the dimension $d$ symbolic.

### Shared PM kinematics

The six `fam1_...` generators use external momenta in order $(q,u_1,u_2)$
and scalar parameter $x$, with

```text
q² = -1,    u1² = u2² = 1,
q·u1 = q·u2 = 0,
u1·u2 = γ = (1+x²)/(2x).
```

The helper name `kin4D()` does **not** instruct RustRed to replace $d$ by
four. No noninteger power offsets are supplied for these six examples. All
kinematic rational functions and any pole/domain conditions remain exact;
do not replace symbolic $x$ by a numeric sample in the exact family.

For `fam1_11`, the ordered list is

```text
D0 = k1·u1              D1 = k2·u1
D2 = k1·u2              D3 = k2·u2
D4 = -k1²               D5 = -k2²
D6 = -(k1+k2-q)²
D7 = -(k1-q)²           D8 = -(k2-q)²
```

For `fam1_12` and `fam1_12_exp`, change only
$D_1=k_2\cdot u_2$ and $D_3=k_2\cdot u_1$. The first two coordinates
are still the removed cuts.

For `fam1_111`, the list is

```text
D0,D1,D2   = k1·u1, k2·u1, k3·u1
D3,D4,D5   = k1·u2, k2·u2, k3·u2
D6,D7,D8   = -k1², -k2², -k3²
D9,D10,D11 = -(k1-q)², -(k2-q)², -(k3-q)²
D12,D13,D14 = -(k1-k2)², -(k2-k3)², -(k3-k1)²
```

For `fam1_112` and `fam1_112_exp`, change only
$D_2=k_3\cdot u_2$ and $D_5=k_3\cdot u_1$.
The comment that resembles “$-k_3^3$” in the C++ file is a comment typo:
the actual denominator is $-k_3^2$.

### Other supplied low-loop families

`bc4PMRad1` has externals $(q,n)$, $q^2=n^2=1$, $q\cdot n=0$,
and scalar `ep2`:

```text
D0 = k1·n, D1 = k2·n,
D2 = k1², D3 = k2²,
D4 = (k1-q)², D5 = (k2-q)², D6 = (k1-k2)².
```

There are **no cuts**. Its final family-initialization argument `{{0,1}}`
means the exponent of $D_0$ is **integer coordinate + ep2**; all other
offsets vanish. It is not a special-ordering declaration. Preserve this
offset in the exact source coefficients, numerical probes, and reduction keys.

`fam_cosmo` has external $p$, no supplied Gram replacement, and scalars
`M1,M2,M3`:

```text
D0 = k1² + M1
D1 = (k1+p)²
D2 = (k2+p)² + M2
D3 = k2²
D4 = (k1-k2)² + M3.
```

Treat $s=p^2$ as an additional independent exact parameter in a Rust Gram
matrix `[[s]]`; do not set it to zero or one. C++ local variable names
`M1,M3,M5` label the three scalar slots which are actually printed as
`M1,M2,M3`. They are additive mass-squared parameters, not instructions to
square those parameters again. There are no cuts or noninteger offsets.

`vac3` has no externals and common additive mass parameter $m$:

```text
D0 = k1²-m, D1 = k2²-m, D2 = k3²-m,
D3 = (k1+k2)²-m, D4 = (k1+k3)²-m, D5 = (k2-k3)²-m.
```

The full 617-rule Rust/C++ comparison retains exact symbolic mass `m` on
both sides. Only the earlier six-case pilot specialized the reference to
`m=1`; that pilot is not the current complete sector comparison.

## Immediate Rust recipe: fam1_11

Use the existing `CoefficientContext`, `AffineDenominator`,
`IntegralFamily::new`, and `SourceSystem::from_family` APIs. This is family
data, not a topology-name dispatch in the engine.

1. Create exact coefficient parameters `d,x`, loop names `k1,k2`, and
   external names `q,u1,u2`.
2. Build `γ=(1+x²)/(2x)` using native Symbolica coefficient arithmetic.
3. Set the external Gram matrix to
   `[[-1,0,0],[0,1,γ],[0,γ,1]]`.
4. Supply nine zero power offsets.
5. Supply the affine constants and rows below, in the stated coordinate order.
6. Generate the 10 ordinary and three LI sources. Prepare the two linear cuts
   once, retain the resulting pre-rules, and use the cut-improved frame with
   cut coordinates fixed at one.
7. Solve all 40 requested sectors with the cut mask `110000000`, the
   matching zero list, default ordering, and numeric seed depth two.
8. Compare pre-rules, every exact sector RHS and guard, and fixed residual
   integral keys against the saved reference output. Record source preparation,
   solving, writing, and comparison separately.

Rust's actual scalar-product coordinate order, verified in
`family/kinematics.rs` and its tests, is

```text
[k1², k1·k2, k2², k1·q, k1·u1, k1·u2, k2·q, k2·u1, k2·u2].
```

Each row below supplies `AffineDenominator::new(constant, coefficients)`;
integer entries are converted with the coefficient context.

| D | Constant | Coefficients in that order |
| --- | ---: | --- |
| 0 | 0 | `[0,0,0,0,1,0,0,0,0]` |
| 1 | 0 | `[0,0,0,0,0,0,0,1,0]` |
| 2 | 0 | `[0,0,0,0,0,1,0,0,0]` |
| 3 | 0 | `[0,0,0,0,0,0,0,0,1]` |
| 4 | 0 | `[-1,0,0,0,0,0,0,0,0]` |
| 5 | 0 | `[0,0,-1,0,0,0,0,0,0]` |
| 6 | 1 | `[-1,-2,-1,2,0,0,2,0,0]` |
| 7 | 1 | `[-1,0,0,2,0,0,0,0,0]` |
| 8 | 1 | `[0,0,-1,0,0,0,2,0,0]` |

The three positive constants follow from $q^2=-1$. Using $-1$ here would
build the wrong family.

### The two expected preliminary rules

These are mathematical restatements of the saved
`vendor/spired/build-release/benchmarks/serial-artifacts/preRules.dat`,
not rules to insert as hidden input into the solver. RustRed must derive them
from the corresponding ordinary cut-derivative identities.

Let $a=(a_0,\ldots,a_8)$, let $e_i$ be a unit displacement of coordinate
$i$, and abbreviate the integral by $I(a)$. The C++ text labels $a_0$
as `n1`, whereas the Rust compact integral display labels it `n0`.

For the first cut, on $a_0\ne1$,

```text
I(a) =
    - γ a2/(a0-1) I(a-e0+e2)
    + 2 a6/(a0-1) I(a-e0-e1+e6)
    + 2 a7/(a0-1) I(a-2e0+e7)
    + 2 a6/(a0-1) I(a-2e0+e6)
    + 2 a4/(a0-1) I(a-2e0+e4).
```

For the second cut, on $a_1\ne1$,

```text
I(a) =
    - γ a3/(a1-1) I(a-e1+e3)
    + 2 a8/(a1-1) I(a-2e1+e8)
    + 2 a6/(a1-1) I(a-2e1+e6)
    + 2 a5/(a1-1) I(a-2e1+e5)
    + 2 a6/(a1-1) I(a-e0-e1+e6).
```

Each has five terms. Their exceptional cut value is **one**, not zero.
The zero-cut convention and cut-priority ordering are also needed when
applying these rules. The residual sector search then starts with
$a_0=a_1=1$; simply fixing those powers before constructing improved
sources would lose the derivative information.

## Ordering experiments and permutation integration

The compact Rust `IntegralOrder` now exposes a validated optional permutation
through `with_permutation`, normalizing identity to the ordinary static lane.
Focused tests include all six three-coordinate permutations against 120,000
comparisons from the compiled C++ comparator. `SectorConfig::permutation`
passes the same order through sector preparation, symbolic and numerical
searches. End-to-end comparison of the examples with overrides remains
outstanding; the configuration API alone is not their acceptance result.

The C++ dynamic order applies its permutation **only to the final
denominator-coordinate and numerator-coordinate lexicographic tie breaks**.
Sector lexicographic comparison and cut-priority comparison stay in original
coordinate order. Permuting all family coordinates is therefore not a faithful
substitute. Default identity permutation must reproduce the static lane.

`fam1_12_exp` enumerates all $7!=5,040$ permutations of noncut coordinates
for each of 40 sectors: 201,600 sector solves. It forces
`omp_set_num_threads(10)`; a controlled comparison must additionally cap
`OMP_THREAD_LIMIT`, or use a clearly identified equivalent driver.
It writes permutation/timing/leaf-count records, not reduction-rule files.
A representative ordering regression is useful but is not the entire supplied
exhaustive workload.

`fam1_112_exp` actively solves only sector `111000101110111`, with

```text
[0,1,2,10,8,6,12,7,9,3,14,4,11,13,5].
```

The nearby “0.2 second” alternative
`[0,1,2,10,3,9,13,11,7,6,12,14,4,5,8]` is commented out. That comment is not
a timing measured here. The remaining random-search and timeout examples are
also commented out. The active executable prints only the resulting rule
count; it does not export rules.

The 16 active `fam1_112` overrides are listed below. Every sector is present
in its 436-row requested manifest, and every permutation is a bijection of
0–14 with cuts first.

| Sector | Comparison permutation |
| --- | --- |
| `111011010101101` | `0,1,2,6,3,5,14,7,9,11,8,12,10,13,4` |
| `111011101010101` | `0,1,2,11,7,3,4,14,6,9,8,5,13,10,12` |
| `111100101010011` | `0,1,2,7,3,10,12,9,5,14,6,8,13,4,11` |
| `111100101010111` | `0,1,2,11,8,13,4,14,6,7,12,9,5,10,3` |
| `111101100011110` | `0,1,2,13,12,8,6,5,11,3,7,4,10,9,14` |
| `111110100011011` | `0,1,2,13,11,9,10,8,4,3,12,14,6,7,5` |
| `111010001100111` | `0,1,2,5,3,8,9,14,6,12,11,10,4,7,13` |
| `111010100001111` | `0,1,2,6,11,4,7,3,14,8,9,5,12,13,10` |
| `111100001010111` | `0,1,2,9,4,10,3,8,13,14,5,11,7,12,6` |
| `111000011100111` | `0,1,2,14,11,10,13,6,7,12,4,3,8,9,5` |
| `111000100010111` | `0,1,2,9,10,5,13,6,14,8,7,12,11,4,3` |
| `111000010100111` | `0,1,2,10,6,7,3,11,9,13,5,4,14,8,12` |
| `111010100011111` | `0,1,2,11,14,9,3,5,6,7,13,8,4,10,12` |
| `111010101011111` | `0,1,2,6,8,10,11,9,13,14,3,5,12,4,7` |
| `111000100010011` | `0,1,2,9,3,14,13,10,11,5,7,4,6,12,8` |
| `111100101011111` | `0,1,2,11,12,10,7,3,13,5,8,4,14,6,9` |

## Exceptional geometry: evidence versus outstanding coverage

Saved `fam1_11` output has **802 sector rules** in 40 text files. A structural
census of every LHS and guard found:

- 733 guarded and 69 unconditional rules;
- 1,082 coordinate equalities in the guards;
- only fixed values $-1,0,1,2$;
- no coupled affine LHS pattern or guard and no non-coordinate grammar.

Together with the two coordinate pre-rule guards, this gives no evidence
that full affine case geometry is needed to reproduce this particular run.
This structural census does not prove that alternate orderings stay
coordinate-only. The subsequent independent Rust generation and exact
comparisons are recorded in [the PM result](spired_fam1_11_results.md).

The already compared `vac3` output likewise has coordinate-only guards.
Subsequent complete generations of `fam1_12` and `fam1_111` expose genuine
coupled cases, now handled by the integrated native affine service. The former
exports two required affine rule domains in Rust; the latter exports 53.
`bc4PMRad1` and selected `fam_cosmo` also pass. The remaining `fam1_112` and
ordering workloads must still be measured: different orderings or kinematics
can expose unsupported geometry. Never substitute guessed coordinate faces or
sampled points for an exact coupled case. General congruence charts and
integer-sector inequality feasibility remain outside current admission.

Likewise, the reference's residual master labels and bounded numerical search
do not themselves establish a zero-uncovered RustRed closing artifact or
master minimality. Reference parity and artifact closure are separate checks.

## Output inventory and outstanding reference builds

Completed C++ builds and saved full reference runs now include `fam1_11`,
`fam1_12`, `fam1_111`, `bc4PMRad1`, selected `fam_cosmo`, `vac3`, and `vac4`.
The `fam1_112` and ordering-study builds/runs remain outstanding. A `.p` build
directory alone is not a built executable.

| Generator | Output produced by its active source |
| --- | --- |
| `fam1_11`, `fam1_12` | `family.bin`, two pre-rule formats, two formats per requested sector; aggregate residual count on stdout |
| `fam1_111` | same categories, plus two zero-sector exports; see caveat below |
| `fam1_112` | family/pre-rules/sectors, plus `MIs.dat` and `stats.dat` |
| `fam1_12_exp` | one permutation/timing/leaf-count table per sector |
| `fam1_112_exp` | rule count on stdout; creates output directory but no rule files |
| `bc4PMRad1` | two formats per sector; residual list on stdout; no internal aggregate timing |
| `fam_cosmo` | `family.bin`, `10101.dat`, `10101.rules`; timing and residual list on stdout |
| `vac3` | `family.bin`, two formats per sector, `MIs.dat`; timing on stdout |

The saved `fam1_11` reference run has 83 files, 802 sector rules plus two
pre-rules, and an aggregate count of 16 residual fixed-index integrals. Files
and prior timing logs are retained under
`vendor/spired/build-release/benchmarks/`. Its text `.dat` files can be
inspected without running Mathematica; compressed binary `.rules` and
`family.bin` require the reference/native reader, not text parsing.

Input/build caveats:

- All active in-scope source and manifest inputs are present. There is no
  supplied one-loop fixture. Full `fam1_112` and ordering-study reference
  outputs have not yet been generated.
- `examples/meson.build` declares `fam1_112_test.cpp`, but that source file
  is absent. A clean reference build setup must explicitly account for this
  missing target; do not invent its intended workload.
- Alternate `*_mat.dat` sector files and `vac.wl` are auxiliary reference
  data, not manifests selected by these active generators.
- The `fam1_111` zero-sector export reads the local `zS` object after it has
  been moved into the family. The completed run confirms its incomplete output;
  use the original 2,304-entry input manifest rather than treating that
  export as authoritative.
- `fam1_112`'s stats header has four column labels but its data rows have
  five fields: sector, elapsed milliseconds, residual count, rule count,
  leaf count. Parse the actual payload, not the header alone.
- Programs read manifests relative to the working directory and write to
  fixed `/tmp/<example>` directories. Protect existing output before runs;
  preserve fresh outputs separately for comparisons.
- Timers have different boundaries. The ordinary PM sector-loop timers
  include per-sector serialization/writes and progress but exclude family,
  source, and pre-rule setup. Report an additional full-process time.
  Compare identical sectors, ordering, numeric depth, thread limits, and
  output work, with oracle verification outside the timed solver boundary.

## Out of the required low-loop generation census

- `vac4.cpp`: four loops, no externals, $K=10$, 16 ordinary sources,
  unit mass. Its 281 zero plus 743 nonzero sectors span the 1,024-sector
  universe, but the executable actually selects 27 supplied unique sectors.
  It is already built/run as an exploratory reference, not part of the
  requested through-three-loop acceptance requirement.
- `ex5PM.cpp` and the 5PM manifests: four loops, three externals, $K=22$.
  The source uses older constructor/`generateIBPs`/`solveSectorv2` interfaces
  and is not enabled in the active Meson targets. It is excluded, not silently
  treated as a working current-API example.
- `reduce.cpp`: an application benchmark, not an independent family
  generator. It loads `/tmp/fam1_112/family.bin` and external expression files
  from `/tmp/exprs_test`, and explicitly requests 16 OpenMP threads.
  Those expression inputs are not part of the supplied example-directory
  census; generation comparisons cannot stand in for application acceptance.
