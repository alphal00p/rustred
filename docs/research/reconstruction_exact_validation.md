# Semi-numerical generation: exact validation and measured phase pilot

Started 2026-09-19; implementation checkpoint 2026-09-20. The completed timings
below are a **single-sector** diagnostic of the existing backends, not a
full-family benchmark or closure certificate. The new opt-in source-weight
backend is implemented and has passed release compile, focused, discovery and
full core regression checks. No speedup or high-loop family solve is claimed
for that new backend.

## Current implementation checkpoint

`SymbolicExactBackend::SemiNumericalSourceWeights` now composes native finite-
field GPLU, rational reconstruction, a harder-prefix rank lower bound, and the
exact full source product described below. Its heavy operations use physical
column IDs and an arity-independent prepared frame. Neither loop count nor
topology name selects a strategy. The Rust enum is an explicit experimental
choice; the existing sparse and semi-numerical choices, defaults, candidate
contracts, and persistence schemas are unchanged. CLI/Python selection has
not been added for this new choice.

The implementation never falls back silently to characteristic-zero GPLU.
An inadmissible prefix, constant frame, exhausted reconstruction budget or
cache budget is an explicit failure. The three aggregate limits count cached
images, retained scalar/index slots, and source-weight slots; they do not
promise a process-memory ceiling. A transient native reduction and exact
product may still require substantial memory.

Independent implementation and mathematical audits are separate from runtime
acceptance. Eighteen new focused test functions cover adversarial prepared
matrices and downstream small-family/provenance/exception parity. All pass in
the optimized runtime gate: the `source_weight` filter reports 19 passes,
including one existing test; the discovery filter reports 78 passes and one
existing ignored diagnostic. The full core suite reports **2,299 passes,
zero failures and 32 existing ignored diagnostics** (215.78 seconds). Source
hashes match before and after every stage, and the gate exits zero. The frozen
gate is `TMP/source-weight-core-gate.L1c2tT/`; the initial `cargo check --tests`
finished successfully in 46.71 seconds and test code generation took 6m54s.
Compilation is not solver timing, and tiny test durations are not five-loop
performance measurements. The two separate audits are retained as
`TMP/source-weight-backend-independent-math-closure-audit-2026-09-20.md` and
`TMP/source-weight-production-genericity-audit-2026-09-20.md`.

The first input-driven comparison and bounded five-loop studies are now
recorded below. Every attempted solve retains its actual completion or failure
status. The historical proposal and diagnostic farther below record why this
design was chosen; their earlier "not implemented" labels describe those
earlier checkpoints, not the current implementation.

## Source-weight backend: first measured comparisons

The frozen release client in `TMP/source-weight-sector-comparison.Xz5haO/`
uses external family/case inputs, natural ordering and one worker. Every mode
keeps the same `Sparse` numerical tail. Core timings include sector
preconditioning and solving, but exclude input/source/zero-census preparation,
snapshot encoding and cold exact comparisons. Event writes and flushes remain
inside solving and differ in number across backends. These are instrumented
shared-host diagnostics, not a production throughput guarantee.

| Completed workload | Sparse core (s) | Source-weight core (s) | Result compared exactly |
| --- | ---: | ---: | --- |
| H sector `0000110110`, pair 1 | 0.805809 | 1.315208 | 75 rules, 1 residual; 2,017 coefficients |
| Same sector, reversed pair 2 | 0.791862 | 1.236301 | Same complete output |
| TIDE factorized sector 31744 | 20.102118 | 30.870570 | 637 rules, 1 residual; 8,365 coefficients |
| TIDE sector 28686, isolated coordinate case 195 | 20.305941 | 25.114978 | 1 guarded candidate; 781 coefficients |

The existing reconstruction backend also completes the H sector in 1.174998 s
and matches its 2,017 coefficients. Comparisons check both native coefficient
maps and the complete source/case/guard/RHS/residual structure, not only
numerical samples. Fresh processes decode the snapshots. The new symbolic
backend has zero exact-elimination replay events; the existing reconstruction
control has 36. The common numerical tail can still perform exact elimination.

The new route is not uniformly faster. In particular, H remains faster with
sparse arithmetic. The two five-loop source-weight diagnostics overlap each
other on separate fixed CPUs, whereas their sparse controls do not; preparation
times also vary considerably. Therefore their precise timing ratios are not
isolated performance conclusions. A serialization outlier in one H sparse
process lies outside core time and must not be used to advertise a speedup.

The isolated case produces three guard branches but does **not** recurse over
their intersections. Completing that candidate is not a solution of its
exceptional boundary. Similarly, a completed selected sector is not a
completed parent-family artifact.

### First TIDE connected-sector attempt: same geometry failure

Both fresh whole-sector attempts for TIDE's six-line connected representative
28686 (`111000000001110`) stop at the identical 195th coordinate case:

| Backend | Core time to error (s) | Process wall (s) | Peak RSS (KiB) | Outcome |
| --- | ---: | ---: | ---: | --- |
| Source weights | 66.984708 | 74.34 | 174,516 | `UnsupportedGeometry` |
| Sparse | 81.513778 | 89.18 | 204,652 | Same `UnsupportedGeometry` |

Neither 300-second deadline fires. Both finish 68 materializations and 127
direct hits before the geometry error; the entire case sequence and unresolved
guard payload agree. The new route spends about 50.595 s in materialization,
versus 75.101 s for sparse, but **neither produces a sector snapshot or artifact**.
This is less time to the same failure, not a completed five-loop speedup.
The subsequent isolated-case comparison confirms exact equality of the final
781-coefficient candidate without claiming to resolve its exceptional locus.

Preparation enumerates all 32,768 auxiliary masks and identifies 5,566 zero
sectors in both runs, matching the thesis's zero-count reference. It does not
establish its physical/anti-sector partition. Numerical corner depth is zero;
no oracle rules, saved traces, prior artifacts or numerical catalog enter the
search. The [TIDE study](tide_five_loop_census.md) describes the exact input
census and the distinction between routing coverage and rule closure.

The separate receipt audit is
`TMP/source-weight-sector-comparison.Xz5haO/INDEPENDENT_MEASUREMENT_AUDIT.md`.
Frozen source, protocol, library, binary and input hashes pass. No default
backend change is justified by these small mixed results.

### Alternate banana input: reconstruction memoization pressure

The older external `five_loop_banana.toml` routing supplies another diagnostic,
not a different loop-specific strategy. Two one-worker selected-parent
source-weight runs use the same frozen client, natural ordering, numerical
depth zero and 300-second limits. Neither times out; both reach case 122,
an affine face with a 499-row/2,907-column frame and seven active variables,
then fail at the retained cached-value budget:

| Cached scalar/index slot limit | Core time to error (s) | Process wall (s) | Peak RSS (KiB) |
| --- | ---: | ---: | ---: |
| 16,000,000 | 19.062594 | 27.68 | 201,368 |
| 64,000,000 | 85.256889 | 94.12 | 606,812 |

The larger caller-selected budget gets farther through target-coefficient
reconstruction but still exhausts the cache; it does not complete the frame or
sector. Both attempts start 24 materializations and finish only 23. A summed
completed-interval counter omits the failed open interval and must not be
reported as all materialization time. Neither creates a rule from incomplete
reconstruction, and neither falls back to exact elimination.

This failure is distinct from TIDE's unsupported quadratic geometry, and the
two input presentations do not support a cross-basis timing ratio. Further
blind budget increases are not the next experiment. The resulting bounded-
cache implementation and its separate verification are described next.

### Bounded native-image reuse

The new backend now uses deterministic least-recently-used eviction instead
of treating cache capacity as a cumulative sample budget. Hits refresh recency;
old images may be discarded and recomputed by the unchanged native Symbolica
oracle. Both retained-image and retained scalar/index-slot limits still apply.
One individually oversized key or image remains an explicit error. Transient
native probe storage and whole-process RSS are not covered by these retained
storage limits.

The public native API audit checked the reconstruction entry point, its option
types and the native multi-output benchmark example. The available public
entry point reconstructs one scalar rational function per call; the example
shares vector images through a caller-owned cache. RustRed adds only that
bounded orchestration, not interpolation, CRT or another reconstruction
kernel. Frozen source chronology, native rank admission, exact full-column
`W*A=r` validation, existing backends, defaults and persistence schemas are
unchanged. Eviction can increase CPU cost through recomputation and does not
itself promise a completed solve or speedup.

The independent implementation/mathematical audit adds seven adversarial tests
alongside four implementation tests. The release gate
`TMP/source-weight-lru-gate.xgSynn/` passes its compile check, **30 focused tests**,
**89 discovery tests** (one existing ignored diagnostic), and **2,310 core
tests** (32 existing ignored diagnostics, zero failures; 291.84 seconds).
Source hashes match before and after the gate. Its optimized test compilation
took 11m52s; this is excluded from all solver measurements. Tiny test timings
are not five-loop generation timings. The separate audit is
`TMP/source-weight-lru-independent-audit-2026-09-20.md`.

A fresh post-change comparison is prepared in
`TMP/source-weight-lru-comparison.xSY9i1/`: repeat the matched small H sector
and then retry the original banana parent with the original 16-million-slot
capacity, not another capacity increase. Pending measurements must not be
inferred from the earlier cache failures. A generic, caller-supplied coordinate
permutation also permits separate ordering experiments on TIDE's nonlinear
case without changing any topology-dependent production logic.

## Existing semi-numerical backend (unchanged)

[`search.rs`](../../crates/rustred-core/src/solver/search.rs) first tests exact
source leading terms. A direct hit bypasses modular discovery/materialization.
Otherwise a modular dependency trace selects exact source rows. The
[`semi-numerical materializer`](../../crates/rustred-core/src/solver/discovery/semi_numerical.rs)
probes their pivot chronology/support, reconstructs each target-row coefficient
with Symbolica, and compares the **complete row** against characteristic-zero
sparse elimination of that selected frame. Missing sampled support is not
silently accepted.

This internal exact validation belongs to **generation** and is included in
solver-only timings. It is different from optional artifact certification,
which also concerns original-source replay, applicability/guards, strict
descent, and declared-domain coverage. Neither process requires minimal masters.

Normal successful semi-numerical materialization performs one exact replay.
The first reconstruction error instead triggers lazy exact support recovery:
an exactly absent coefficient is skipped, while a required nonzero coefficient
retains the failure. That exact row is reused within the same attempt. A full
row mismatch can retry once with exact support; the new attempt does not retain
the old exact row, so it can replay again. Direct hits and the separate shared
fixed-integer-case solver must not be counted as these semi-numerical attempts.

The new `SemiNumericalExactReplayStarted/Finished` observer pair makes this
phase measurable without forwarding every sparse-row event. Failed replay is
explicit; finished replay is not yet accepted reconstruction or closure.
Existing full-parent logs do not contain this split.

## Completed H-sector pilot

Input: unchanged external H family from the matched four-loop matrix; sector
`0000110110`, natural ordering, 16 ordinary sources, authenticated zero census,
unbounded symbolic depth and finite-corner depth two. Native reconstruction
limits: degree 128, probes 200,000, attempts 4, primes 8. One optimized binary,
one worker on CPU 82, nested pools 1; two fresh pairs in sparse→semi then
semi→sparse order. Every run had a 180-second deadline; all completed with exit 0.

| Measurement | Pair0 sparse | Pair0 semi | Pair1 sparse | Pair1 semi |
|---|---:|---:|---:|---:|
| Solver core, seconds | 0.834502 | 1.215987 | 0.867125 | 1.322641 |
| Materializer intervals, seconds | 0.229134 | 0.609375 | 0.245689 | 0.671847 |
| Internal exact replay, seconds | — | 0.211860 | — | 0.230606 |
| Replay / semi materializer | — | 34.77% | — | 34.32% |
| Replay / semi solver core | — | 17.42% | — | 17.44% |
| Whole process wall, seconds | 2.21 | 2.61 | 2.20 | 2.76 |
| Whole process user / system CPU, seconds | 0.94 / 1.24 | 1.33 / 1.25 | 0.98 / 1.19 | 1.44 / 1.29 |
| Whole process peak RSS, KiB | 12,288 | 15,384 | 15,432 | 15,392 |

Core time includes sector preconditioning and the entire case solve, but
excludes family/source/zero preparation, snapshot encoding and comparison.
Materializer intervals end before canonicalization. Non-replay materializer
time includes support probes, reconstruction and wrappers—not pure
interpolation. Monotonic observer events are buffered structurally in memory;
no exact expressions are copied or logged inside the timed solve.

Each run produced 75 rules and 1 finite residual from 74 symbolic cases:
38 direct hits and 36 materializations, plus the shared numerical tail. Each
semi run had 36 attempts and 36 exact replays, with zero support recoveries,
failed replays or retries. The selected frames totaled 1,017 source-row
occurrences; largest frame 101 rows/246 columns, at most 6 active variables.

Both cross-backend pairs and both same-backend repeats matched all 2,017 RHS
coefficients exactly, with explicit numerator/denominator context checks,
plus case/guard/provenance/RHS-integral structure and finite residuals. No
oracle rules, terminal catalog, prior trace or saved solution entered solving.
The snapshots are diagnostic values, not published closing artifacts.

**Conclusion:** replay is measurable but not dominant here. Even subtracting
its entire cost with a hypothetical zero-cost replacement leaves semi core
times 1.004127/1.092034 s, above the paired sparse times. This does not predict
the split for full H, other parents, or the censored five-loop cube. Two shared-
host observations are not a confidence interval or general speedup claim.

Evidence: `TMP/reconstruction-phase-pilot.Cuwn0a/` contains the protocol,
generic input-driven client, raw events, statuses/resources, exact snapshots,
four comparisons, summary verifier and hashes. Binary SHA256:
`c4988b1357e70e57fbadd257d6e9456c3376caad7153238c944d30556e29cd91`.

## Exact source-span witness proposal — not implemented

Symbolica's native rational reconstruction uses a fresh-prime probabilistic
identity check. Deleting exact replay would therefore weaken the current
contract. A possible alternative is to retain native finite-field GPLU's
`L*U=A`, reconstruct source multipliers `lambda` with that **existing native**
reconstruction service, then verify the full exact product `lambda^T*A=r`.
Use native sparse solving/multiplication and explicit maps; require unit target
and zero forbidden columns. No custom reconstruction or linear algebra kernel.

Independent/dependent/empty source chronology must be handled explicitly;
native L rows and pivot ordinals are not interchangeable with integral columns.
The existing [`target-only backend`](../../crates/rustred-core/src/solver/discovery/target_only.rs)
already illustrates a native transposed triangular solve and full-row product,
but still performs partial characteristic-zero elimination. It is a useful
control, not evidence that a reconstructed-weight path will be faster.

An exact identity in prepared-source span is not automatically original-IBP
provenance or exceptional-face coverage. Source guards and canceled multiplier
denominator poles cannot be discarded. Preserve the present publication and
optional certification gates; sample agreement never supplies that authority.
Measure weight support/reconstruction/product costs before implementing a new
default, and expand phase sampling before attributing five-loop delays to replay.

### Concrete experimental boundary — September 20

A further pinned-API audit supports an internal diagnostic experiment, not a
public backend or replacement of the existing `SemiNumerical` backend. Keep
target-row reconstruction and add
Symbolica reconstruction of source weights, followed by the complete native
sparse product `W*A == r`. The new path must perform no characteristic-zero
GPLU, including on failure; the old path remains the explicit control. This
is proposed work, not an implemented capability or measured speedup.

The important implementation details are:

- Track accepted U ordinal, original source ordinal and actual native L row
  separately. Empty inputs add no L row; nonempty dependent inputs can add L
  rows without adding U rows. Extract the accepted square lower factor and
  solve its transpose with existing native matrix services. With empty inputs,
  the native rectangular identity refers to the nonempty processed source rows;
  test the recorded row embedding, not an assumed original-row identity.
- Cache full target/weight images by prime and evaluation point, retaining the
  full pivot chronology. Reject unusable samples rather than combine branches.
  Bound cached slots and scalar entries independently of native per-coefficient
  reconstruction budgets.
- Restore both coefficient maps and check every physical column, including
  omitted sampled support, zero forbidden columns and unit target. Native
  sparse multiplication performs the algebra; no private reconstruction,
  elimination or rational-field kernel belongs in RustRed.
- Keep heavy operations arity-independent, using the existing `ProbeFrame`,
  CSR column IDs and a thin const-generic integral adapter. Dimensions and
  resource ceilings are input data, never loop-count or topology dispatch.

Prepared-span equality still does not prove the current publication gate's
canonical replay row or exceptional-domain applicability. For example,
`(1/x)*[x,x]=[1,1]` loses information on `x=0`. Retain original source traces,
guard/pole obligations and independent publication checks. Do not promote the
temporary weight vector into a trusted artifact witness or silently restore
ordinary exact replay when this experimental route fails.

This also matters before certification: the existing candidate applier checks
stored rule exceptions and RHS poles, not a temporary weight vector. A later
publication check cannot protect an uncertified candidate already used by that
applier. Keep the first kernel internal and diagnostic. Before public selection,
either prove complete output/provenance equivalence under the existing candidate
contract, or independently audit an expanded candidate-applicability contract
including multiplier poles. An `experimental` label alone is not a correctness
mechanism. The sufficient canonical-equivalence criterion below provides the
narrower nonregression route; it does not strengthen candidate applicability.

Required tests include empty/dependent-row chronology, nonunit pivots, bad
primes, variable-map permutations, hidden support, tampered weights/RHS,
surviving forbidden columns and canceled multiplier poles. Differential exact
row comparisons and end-to-end small-family regressions precede any backend
rollout. Weight reconstruction may cost more than it saves; the completed H
pilot above already excludes a general speedup merely from removing replay.
Detailed source/API review: `TMP/source-weight-reconstruction-next-slice-audit-2026-09-20.md`.
Independent mathematical review:
`TMP/source-weight-reconstruction-independent-math-audit-2026-09-20.md`.

### A sufficient canonical-row certificate

A second independent audit establishes a generic sufficient condition for
replacing exact GPLU **without changing its returned row**. This is a proved
matrix criterion, not an implemented production backend or a measured solve.

Freeze the exact prepared source prefix `A` with `m` rows, ending at the
proposed first target hit. Let `H` contain every integral column strictly
harder than target `t`. Require both:

1. At a valid finite-field point, Symbolica finds rank `m-1` for the first
   `m-1` source rows restricted to `H`. All input denominators must be defined.
2. A native **exact** full-column product establishes `W*A = r`, with
   `r_H = 0` and `r_t = 1`, in the same coefficient maps.

The nonzero modular minor proves characteristic-zero independence of the
harder prefix; it is a one-sided, deterministic lower bound, not a sampled
proof of zeros. The exact witness supplies a nontrivial relation on all `H`
rows, so their left nullspace has dimension one. Every earlier source must
therefore pivot in `H`, and the final row pivots at `t`. Fixing its target
coefficient to one makes the entire returned row unique. It is exactly the
ordinary first-target GPLU row, including the full easier tail. Intermediate
pivot positions need not agree across fields or arrive monotonically.

For instance, with columns `[h0,h1,t,e]`,

```text
A = [[0,1,1,2], [1,0,3,4], [1,1,5,9]]
W = [-1,-1,1]
W*A = [0,0,1,3]
```

The prefix harder rank is two although its pivots arrive in reverse column
order. In contrast, the earlier example with a tail-only prefix row fails
admission. A deficient sample is an admission miss, not proof that no rule
exists. Do not silently drop rows. For a longer frozen trace, use precisely
the consumed first-hit prefix while retaining complete caller provenance and
whole-input validity checks.

This certificate can justify a later explicit backend with the **same**
canonicalization, exception extraction, source trace and uncertified-candidate
contract as sparse GPLU. It need not repair an existing candidate limitation
to prove nonregression. In particular, `[x,x]/x=[1,1]` still needs separate
applicability treatment at `x=0`, exactly as for the ordinary sparse result.
Strong publication retains its independent original-source/pole checks.
No wider applicability or closure claim follows from the rank test.

Next implementation must test this admission criterion, complete downstream
equivalence and explicit misses before public selection; it must never use
sampled rank as a substitute for the exact full product. Detailed proof and
source-boundary audit:
`TMP/source-weight-rank-certificate-independent-audit-2026-09-20.md`.

### Completed private native-algebra diagnostic

The workspace-only diagnostic in `TMP/native-weight-diagnostic.QUFbJz/` now
passes, including an independent fresh-process repeat. It is **not** integrated
into RustRed, does not yet implement the rank certificate above, and solves
no integral family. It uses generic matrix dimensions and native Symbolica
GPLU, sparse solving, multiplication and rational reconstruction only.

The checks cover explicit L/source/U embeddings with empty and dependent rows,
nonmonotone pivots, nonunit diagonals, a zero accepted weight, tampering, and a
different valid witness. For the two-row frame

```text
A = [[x,1,y], [x,x+2,y+3]]
W = [-1/(x+1),1/(x+1)]
W*A = [0,1,3/(x+1)]
```

native reconstruction finds both weights with 18 probes across two primes
each. Their 36 oracle calls share 18 modular images. Original and reversed
variable maps, including an inactive variable, give the same exact full
product without characteristic-zero GPLU on that reconstruction path. The
canceled-pole and noncanonical-span counterexamples remain explicit. Native
reconstruction also succeeds for this tiny zero oracle (10 probes/two primes);
zero reconstruction is therefore not universally unavailable.

These tiny checks take milliseconds and establish only API composition and
bookkeeping feasibility—not K6/K15 timings, scaling, or a backend speed ratio.
Optimized binary SHA256:
`81145ec0599a2b35c6dfdcd8dde7188329deedc70d1f29eaf2f8d91c77a1a137`.
The independent `root-replay.*` receipt exits zero with unchanged source and
binary hashes. Production source admission, aggregate cache accounting and
public integration remain future work.
