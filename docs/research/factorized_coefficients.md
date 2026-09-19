# Native factorized-denominator application experiment

Status: independently audited release experiment, September 19, 2026.
**Production still uses Symbolica's ordinary rational polynomials.** No new
coefficient option, alternative applier, artifact schema or production default
is introduced by this study.

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

## Implementation decision

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
