# Native factorized-denominator application experiment

Status: independently audited release experiments, September 19, 2026.
**Ordinary rational polynomials remain the default, including in Vakint.**
An explicit Rust-library factorized-cache option now exists in the same
candidate applier. It does not change artifacts, public output coefficients,
rule authority or default behavior. The original frame replay below and the
subsequent full-application measurements have different timing boundaries.

Symbolica 3.0's `FactorizedRationalPolynomial<IntegerRing, u16>` preserves
denominator factors and their multiplicities; its numerator remains expanded.
Its public constructors, arithmetic implementation, constructor tests and Atom
conversion API were inspected before use. All algebra below uses that native
implementation, not a RustRed factorization or cancellation kernel.

## Real workload and controlled boundary

An isolated copy of RustRed `2b50267c` adds a diagnostic method that calls the
existing guarded rule application to expose immediate child coefficients.
It changes neither traversal nor rule selection. The unchanged reducer loads
the saved H/X programs, installs the same verified parameter-alias plan and
computes expected outputs for the target `[3,1,1,1,1,1,1,1,1,0]`.

For each family, capture three actual accumulation frames: the root and the
first two lexicographic applicable nonterminal immediate children. Selection
does not depend on timings or polynomial shape. There are 1,605 multiplication
contributions for H and 7,977 for X. Native Atom/State coefficient snapshots
occupy 101,868 and 620,600 bytes. These frames overlap in the reduction DAG;
their sum is a replay workload, not the cost of an independent full reduction.

Both variants replay identical ordered multiplication, addition and zero
removal. Factorization converts each distinct operand once, preserves its
native representation throughout accumulation and materializes only the final
frame output. Original rule guards, poles and descent are checked by the
unchanged capture path. Full coefficient-map checks and a conservative native
exponent envelope precede replay. Exact equality with the original complete
outputs is checked after every frame.

Five fresh-process pairs per family alternate representation order. Each
process runs twenty repeats of the same three frames; no warm-up sample is
excluded. All **20 processes exit successfully and all 1,200 exact frame
comparisons pass**. Affinity is 88–93 with one compute worker and nested pools
capped at one. Compilation, capture, native import/setup and final equality
checks are excluded from the arithmetic timer and reported separately in the
retained evidence. This is a shared-host experiment, not a confidence bound.

## Measured result

Repeated arithmetic is the median of five process-level medians of twenty
three-frame repeats, including final ordinary-polynomial materialization.
Preparation is measured once per process. The cold column is the median of
each process's preparation plus its first three-frame pass, not a sum of
unrelated medians.

| Family / representation | Operand preparation | Repeated arithmetic + output | Preparation + first pass |
| --- | ---: | ---: | ---: |
| H, ordinary RP | 0.175 ms | 8.893 ms | 9.273 ms |
| H, factorized denominator | 6.983 ms | 4.609 ms | 11.698 ms |
| X, ordinary RP | 1.563 ms | 77.954 ms | 79.700 ms |
| X, factorized denominator | 77.705 ms | 40.658 ms | 118.581 ms |

Median **paired** process-median speed ratios are 1.927× for H and 1.914× for
X. The ratios of the separately reported medians are slightly different. The
arithmetic advantage survives final materialization, but initial conversion
makes the first pass about 26% slower on H and 49% slower on X.

There is no demonstrated memory saving. For X, factorized inputs retain 12,924
polynomial objects versus 7,120, and 40,535 sparse terms versus 35,849, although
integer significant bits decrease from 607,871 to 451,960. Fresh replay-process
peak RSS medians are 6,168 versus 12,324 KiB. H medians are 3,092 versus
6,160 KiB, with broad overlapping tiny-process ranges. These peaks include
initialization, transient conversion, expected ordinary outputs and allocator
retention; they do not measure a persistent whole-DAG cache. Structural counts
are not allocated-byte estimates.

## Frame-replay decision, before implementation

A bounded **persistent native factorized-cache prototype** is justified. Do
not switch the production default based on this replay, and do not convert
expanded child decompositions back and forth at every frame.

A persistent representation would create child results directly as factorized
values rather than paying the conversion of their already-expanded outputs
used in this experiment. Its actual end-to-end benefit must therefore be
measured; neither the replay's conversion penalty nor its approximately 1.9×
kernel advantage predicts the complete reduction's timing. Keep the same
applier, native guards, terminal aliases, ordering, descent, mass restoration
and public ordinary-coefficient output. Preserve resource accounting for
numerators, denominator factors, scalar coefficients and multiplicities.

Test first unseen points, repeated requests and multiple distinct targets,
including exact parity and process/cache memory. This is unrelated to the
withdrawn Symbolica-replacement applier. It does not benchmark generation-time
multivariate coefficients, introduce another CAS, or certify saved candidates.
The existing four-loop numerical acceptance remains unchanged.

## Evidence

The isolated sources, native snapshots, frozen hashes, release build/capture/
replay commands, all per-frame samples and GNU resource logs are retained in
`TMP/factorized-combine-replay.l37GIr/`. Its `results.json` and read-only
`summarize.py` define every statistic; `independent-audit.md` verifies the full
inventory, values, variable maps, paired ratios and interpretation.

The earlier [public application profile](vacuum_parametric_terminal_equivalence.md#first-use-application-profile)
motivated this experiment. Neither study claims an end-to-end factorized
Vakint speedup or a reduced whole-reduction memory footprint.

## Persistent cache: implemented and measured

The Rust-library opt-in is:

```rust
use rustred::solver::CandidateCacheRepresentation;

reducer.set_cache_representation(CandidateCacheRepresentation::Factorized)?;
```

Select the representation before any reduction, or after `clear_cache()`.
Selection on a nonempty cache is rejected, even if its only entry is zero.
`Sparse` is the unchanged default. This is not the withdrawn
Symbolica-replacement applier: the scheduler, rule selection, original
denominator checks, guard conjunctions, scope and descent checks are unchanged.
Only accumulation and retained descendant coefficients use native factorized
denominators. A specialized edge is converted once for its combination step;
descendant maps stay factorized. Requested public decompositions are
transiently materialized as ordinary rational polynomials, including on cache
hits. There is no second retained ordinary-result cache.

All factorization, cancellation, addition, multiplication and final normalization
use Symbolica. The narrow adapter binds exact variable maps, checks native
exponent/multiplicity arithmetic and conservative work envelopes, and preserves
atomic cache accounting. Charged coefficient terms are the larger of stored
sparse parts and a prospective expanded-support bound. Payload bytes include
native factor vectors, polynomials and scalar integer allocations; neither
charge claims to measure process RSS or bound all native CAS scratch.

### Full saved-program application protocol

Three fresh-process pairs per H/X workload alternate sparse-first,
factorized-first and sparse-first. Each uses the same saved native program,
verified U alias plan, natural rule order and reduction limits. All measured
processes are pinned to CPU 80, with nested pools capped at one. Builds and the
concurrent five-loop study use different physical cores; the host remains
shared, so this is not a confidence-interval study.

One workload starts with the D1-cubed target in an empty owner. A separate fresh
owner processes D1 squared, D1 cubed, then the D1 pinch. The latter two requests
intentionally reuse prior work. Every requested result is repeated five times.
First-request timing includes traversal, edge conversion, checked arithmetic,
cache admission and final public-output materialization. Loading, U preparation,
validation of measured results and native output encoding are separate.

All **24 application processes exit successfully**, with **24 cross-process
exact output-map comparisons (1,005 coefficient comparisons)** and **240 warm
equality checks** passing. Numerator and denominator maps are checked explicitly,
including constants; mass exponents and warm cache/work invariants also pass.
Both representations perform identical rule applications, cache hits and
coalescing additions and retain the same cached-integral counts. Returned
terminal key sets are compared exactly. H's cold target uses
27,496 rules and 27,882 cache entries; X uses 88,134 and 88,579. No rules or
terminal catalogs were regenerated, and no FORM call or certification occurs.

### Measured first-use and warm behavior

Numbers below are medians across three processes per representation. Warm
numbers are medians of the five-repeat median in each process. Paired speed
ratios are medians of the three within-pair ratios and need not equal ratios
of separately reported medians.

| Fresh D1-cubed owner | Sparse first request | Factorized first request | Paired speed ratio | Sparse warm | Factorized warm |
| --- | ---: | ---: | ---: | ---: | ---: |
| H | 4.404 s | 3.858 s | 1.150× | 10 µs | 225 µs |
| X | 24.438 s | 21.008 s | 1.161× | 15 µs | 383 µs |

Every cold pair is faster with factorized storage, but gains are much smaller
than the approximately 1.9× arithmetic replay. Cold paired ratios range from
1.141–1.215× for H and 1.102–1.183× for X. Warm materialization is consistently
more expensive, although its observed absolute cost here remains below 1 ms.

| Related-target sequence | Sparse first request | Factorized first request |
| --- | ---: | ---: |
| H, D1 squared from empty owner | 4.046 s | 3.677 s |
| H, D1 cubed after squared | 0.351 s | 0.240 s |
| X, D1 squared from empty owner | 21.021 s | 18.044 s |
| X, D1 cubed after squared | 4.747 s | 3.586 s |

Not every sequence observation improves: in the third pair H's squared target
takes 4.490→5.101 s, despite the lower factorized median across pairs. The
shared-host measurements support a workload-dependent opt-in, not an unconditional
performance guarantee.

The final pinches hit already-cached one-term terminals and apply no new rules.
Their few-microsecond timings are near timer resolution; no speed ratios are
meaningful. The sequence's later requests must not be advertised as independent
cold reductions.

### Memory and decision

| Fresh D1-cubed owner | Sparse charged coefficient bytes | Factorized charged coefficient bytes | Sparse process peak RSS | Factorized process peak RSS |
| --- | ---: | ---: | ---: | ---: |
| H | 31,850,888 | 46,615,490 | 491,572 KiB | 503,192 KiB |
| X | 149,934,954 | 232,895,902 | 1,117,968 KiB | 1,223,576 KiB |

These are medians, not a claim of byte-weight determinism: native allocated
capacities vary slightly across fresh processes. Charged bytes grow about
46%/55%; full-process RSS grows about 2%/9%. RSS includes loading, temporary
values, result validation and later output encoding, not just the cache.
Whole-process CPU medians are H 5.94→5.73 s and X 27.40→23.82 s; these include
the separate phases and checks and are not solver-core CPU measurements.

Keep this explicit opt-in available for further application experiments and
retain the sparse default. The benefit is real on these saved workloads but
modest, with a memory/warm-latency tradeoff. No Vakint activation, universal
speedup or factorized generation-backend claim follows from this gate. Any
Vakint adoption still needs its complete numerical acceptance matrix and a
matched public-backend benchmark.

Focused release groups pass: 67 factorized-filter, 28 candidate-application,
20 coefficient and 84 reduction tests; filters overlap and counts are not
additive. They include seven private native-adapter tests and nine independently
written adversarial integration tests. The implementation and mathematical
audit preserve original poles, exceptional conjunctions, descent, finite
reachability, exact contexts, partial-cache failures and resource boundaries.
`cargo fmt --all -- --check` also passes.

Full evidence, frozen library/client/input hashes, build commands, every phase,
native exact output snapshots and the read-only `summarize.py` are retained in
`TMP/factorized-cache-application.JDLsek/`. The earlier frame-replay evidence
remains separate and is not relabeled as an end-to-end result.

### Follow-up CPU profile

A separate factorized X cold run of the same frozen client completes and
matches all 63 sparse-reference output coefficients exactly. Its instrumented
first request takes 21.217 s; this is not another timing pair. Recording uses
99 Hz user-space CPU-clock samples, with no call graph. Approximate phase
windows come from externally timestamped stdout receipts; their measured target
interval differs from the client's duration by 0.111 ms. A conservative interior
omits one second at each end. Receipt timestamps are not intrinsic phase markers.

The interior contains 1,713 samples, with no lost or unresolved records. Disjoint
flat self-sample bins include allocation/vector allocation 20.37%, RustRed
validation/resource admission 12.55%, native polynomial/rational services 20.78%,
native scalar arithmetic 10.74%, factorized arithmetic/services 2.98%, and named
factorization/construction routines 1.52%. Full symbol memberships and both
observed/interior windows are retained in `TMP/factorized-cache-profile.lYwuO6/`.

These are exclusive symbol samples, **not inclusive operation costs**. Native
polynomial/scalar helpers and allocation can belong to factorization, ordinary
specialization or validation callers; inlining also obscures those boundaries.
In particular, 1.52% does not bound factorization's total cost. The evidence
motivates investigating temporary allocations and redundant checks of privately
sealed coefficient values while retaining all mathematical admission/resource
gates. It does not justify removing original poles, guards or descent checks,
nor predict the gain from a specific optimization.
