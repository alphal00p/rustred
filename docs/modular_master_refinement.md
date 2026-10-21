# Opt-in modular master refinement

The `modular-circuits` strategy searches for **exact relations**, not for a proof
that a master basis is minimal. The default remains `exact`. Both operate on
the same saved publications and retain their already proved terminal maps.
Neither needs new parametric-rule generation or numerical master values.
Fresh certification campaigns continue to use the `legacy` symbolic-domain
engine, including Epoch/CP6. Selecting modular master refinement does not
change that campaign engine or migrate its checkpoint.

## What it does

1. Freeze the saved exact sparse basis, source ordering, terminals and domain.
2. Insert the inherited basis into Symbolica's finite-field sparse reducer.
   Stream further ordinary IBP definitions at the same finite integer seeds as
   the exact strategy, using a separate discovery cursor.
3. When a reduced row first pivots on an unproved terminal, walk its direct
   elimination dependencies backwards. This produces an independent, compact
   frame of original sources rather than expanded symbolic elimination chains.
4. Ask RustRed's existing Symbolica source-weight materializer to reconstruct
   a relation on that frame. Unselected source weights are fixed to zero.
   The forbidden columns include every auxiliary of the selected exact sources
   and every harder terminal, including entries that vanished in the discovery
   sample. The target and easier terminals are not forbidden; every physical
   column still participates in the full exact product below.
5. Check the entire rational-polynomial identity `W*A = image` exactly with
   Symbolica. The image must be terminal-only with the prescribed unit pivot.
   Preserve original guards and all actual denominator poles. Computational
   pivots that cancelled out do not become permanent conditions.
6. Commit successful relations in deterministic order. A failed reconstruction
   or unlucky sample is not evidence that no relation exists.

The only new RustRed operations are structural source/column indexing,
dependency traversal, orchestration and persistence. Finite fields, sparse
elimination, rational reconstruction and exact products are Symbolica APIs;
there is no new interpolation, CRT or custom CAS kernel.

## Persistence and authority

Modular discovery has its own versioned native binary sidecar. The old exact
source cursor does **not** advance when numerical rows are inspected. The
sidecar records the immutable exact baseline, stable source RowIds, sampled
sparse state, proposals and exact accepted source circuits. Loading regenerates
the selected ordinary sources and rechecks every new full-column exact identity
before installing its terminal maps. A numerical rank, dependency graph or
cached pivot can never authorize an equation.

Cancellation is observed between rows and independent candidate lifts. Started
native tasks drain before checkpoint publication. Resume may change worker and
operational reconstruction grants; it must not change the source recipe or
turn an exact-strategy checkpoint into a modular-strategy checkpoint in place.
The two strategies use separate refinement phase directories. A new modular
phase may inherit the relations of a published exact refinement.
The public wrapper remembers an explicitly selected strategy for subsequent
resumes. Repeat `refine` to continue that phase; changing worker counts or
operational grants retains its cursor. Changing the source recipe or strategy
requires a new phase. Before switching away from an unfinished phase, use
`finalize-refinement` to publish its checkpointed proved maps if those maps
should be inherited by the next phase. The original search checkpoint remains
available for a later resume.

`modular_search_complete` means this finite heuristic source scan and its
attempted candidates drained. It does **not** mean exact row-span exhaustion,
independence, unrestricted family closure or a minimal master basis. Reports
keep this separate from `finite_search_complete`. Rejected proposals and their
diagnostics remain available for a later retry with different grants.

## Selecting it

Use a reconstruction-enabled optimized build (the CLI's default features include
this feature). A separate build directory keeps the existing executable in
place:

```bash
cargo build --release --locked -p rustred-app --bin rustred \
  --target-dir TMP/modular-master-build
python -B examples/python/saved_campaign.py refine \
  --campaign "$RUSTRED_CAMPAIGN" \
  --executable "$PWD/TMP/modular-master-build/release/rustred" \
  --refinement-strategy modular-circuits
```

The saved campaign's worker budget is reused. Optional `--workers` and `--cpus`
select refinement resources independently of the campaign's closure solver.
An interrupted run can be continued with `saved_campaign.py refine --campaign
"$RUSTRED_CAMPAIGN"`; its saved strategy and native checkpoint are retained.
Explicit `--refinement-strategy exact` selects the default backend for a new
phase rather than interpreting a modular cursor as an exact cursor.

Do not run two refinements writing the same campaign concurrently. The program
does not interrupt an existing exact refinement. Saved-rule and circuit-symmetry
assistance currently require the exact strategy and are explicitly rejected
with `modular-circuits`.

Omitting `--modular-policy` imposes no tuned row, coefficient, degree, probe,
prime, retry, or cache quota on modular discovery and reconstruction. The
process RAM guard governs retained work. Native integer/exponent representation
bounds still apply: Symbolica's public reconstruction API requires doubled
degrees to fit `u16`, giving its largest representable degree allowance.

Optional `--modular-policy POLICY.json` selects scheduling windows and any
resource caps the caller explicitly wants. For example, this changes only the
resumable step sizes and leaves reconstruction uncapped:

```json
{
  "batch_rows": 64,
  "max_candidates": 8
}
```

These windows do not cap the total source scan or discard pending proposals.
An explicit `discovery_samples` or `reconstruction.max_*` value opts into the
corresponding finite allowance; omitted resource fields retain the uncapped
default. Reconstruction allowances apply per frame/scalar, not to the total
campaign source inventory.
Unlimited operation does not mean retrying one unusable prime forever. Scout
points and primes are visited fairly. Symbolica reconstruction runs in growing
finite scheduling windows when grants are unbounded; an intermediate window
exhaustion is retried, not treated as rejection. Symbolica still performs all
interpolation and rational lifting. These windows impose no total search cap.

The complete resolved policy is recorded in the phase. Unknown fields are
errors; the same native policy can be changed for a resumable retry. Symbolica
may use an additional verification prime beyond its reconstruction-prime
setting; do not interpret that setting as an aggregate probe counter.

Generated master-session, modular-sidecar and collection transport in the
application also has no implicit byte/count/algebra quota. The core Rust SDK's
native encode/decode methods accept explicit `BinaryIoLimits` when a caller
wants those caps; those grants are honored. The application currently uses its
process RAM policy for this transport rather than a separate CLI native-I/O
grant. `--finite-policy` retains its existing explicit control-document and raw
provenance grants; its Work storage fields are not reinterpreted as matrix-file
caps. Imported normalization and collection recipes retain their recorded
constraints so an operational resource change cannot invalidate saved work.
The private collection-record 1 GiB ceiling and census-copy 256 MiB ceiling
have also been removed. Collection schema-one/schema-two wire records are
unchanged; caller transport grants, native replay, file-shape and digest checks
remain enforced. Compact synthetic decoded-claim tests cover this distinction
without allocating gigabyte fixtures. Generated-native transport is not a
hostile-input allocation sandbox.

Discovery insertion is presently serial; independent candidate lifts use the
configured licensed worker pool. A 128-worker allowance is not a promise that
one numerical row or one short candidate saturates 128 cores. Exact remains
available with its block-parallel elimination implementation.

A valid finite-field full-row-rank witness on the forbidden columns can reject
that **generic selected frame or prefix**, not other sources, the family, or
exceptional parameter faces. Rank drops and poles are inconclusive. A permanent
oracle failure is propagated explicitly rather than fed back as endlessly
unavailable samples. The current native adapter uses a narrowly caught private
unwind marker around the Symbolica call; unrelated panics are not swallowed.
RustRed's existing authenticated native algebra boundary already requires
`panic = "unwind"`; the marker follows that contract. The current crate does
not support an abort-only embedding, even with finite reconstruction grants.

## Validation and measurement status

Final application validation passed 72 tests with one explicitly ignored
fixture writer; the CLI suite passed 15 tests. Native focused controls passed
80 terminal-relation tests (one ignored workload), 41 source-weight tests,
74 indexed-algebra tests, and 110 tests matching the persistence filter.
These are overlapping focused filters, not a summed unique-test count.
The focused Python
delivery suite passed 175 tests without skips; its command and outputs are recorded as
`TMP/modular-master-refinement-20261021/python-delivery-tests.*`.
The released CLI scratch fixture in
`TMP/modular-master-refinement-20261021/cli-final-modular` scanned two ordinary
sources, accepted one exactly verified terminal relation, and reduced two keys
to one. Its completed resume retained the same result. It reports
`modular_search_complete = true`, `refinement_complete = true`, and
`finite_search_complete = false`, with the exact source cursor still zero.
This is an integration/control fixture, not a saved-campaign coverage proof.

The following completed pre-cache release-mode probe runs used binary SHA256
`ecb2c1c7d32088a516c023720410242bc74de41640d5f76998b66134c60ae6bb`
on a shared, non-isolated host. Each receipt records the actual CPU affinity
232–239. An initial receipt-formatting bug incorrectly printed 240–247;
the table uses the corrected final repeats, not those earlier receipt labels.
The raw results and commands are in
`TMP/modular-master-refinement-20261021/final-{fixture-*,real-64-*}.{json,receipt.json,stderr}`.

| Cohort | Workers | Sources scanned | New exact relations | Modular solve (s) | Exact control solve (s) |
| --- | ---: | ---: | ---: | ---: | ---: |
| Massive one-loop tadpole | 1 | 3 | 2 | 0.000770 | 0.000044 |
| Massive one-loop tadpole | 6 | 3 | 2 | 0.000501 | 0.000151 |
| Connected two-loop vacuum sunset | 1 | 8 | 1 | 0.000514 | 0.000123 |
| Connected two-loop vacuum sunset | 6 | 8 | 1 | 0.000754 | 0.000215 |
| Three independent tadpoles with ISP basis | 1 | 27 | 2 | 0.000678 | 0.000161 |
| Three independent tadpoles with ISP basis | 6 | 27 | 2 | 0.001032 | 0.000184 |
| Saved five-loop input, next 64 sources | 1 | 64 | 0 | 0.105883 | 140.424846 |
| Saved five-loop input, next 64 sources | 6 | 64 | 0 | 0.120954 | 59.486906 |

“Modular solve” includes ordinary-source preparation, finite-field discovery,
reconstruction and exact product verification. Both solve columns exclude
loading, initialization, checkpoint encoding and same-process native
decode/replay. On the five-loop
input, modular initialization took 2.460–2.465 seconds, native input loading
1.930–2.022 seconds, encoding 0.078–0.079 seconds, and same-process native
decode/replay
3.125–3.254 seconds. The probe does not expose separate scalar-probe counts or
per-worker exact-product timings.

All three positive fixtures matched the exact control's terminal maps and
passed cold-reload map equality. Exact insertion was faster on these small
fixtures; the sub-millisecond values are single-run diagnostics, not robust
parallel scaling measurements.

The real next-64 cohort found no new terminal relation with either strategy.
Modular scanning retained the existing maps and 909 remaining terminals,
produced identical modular checkpoint bytes at one and six workers, and left
the exact source cursor unchanged. The exact control also maintained the new
symbolic auxiliary row space, which modular discovery intentionally does not
construct. This is evidence for cheap heuristic scanning on a negative
prefix, not an equivalent full-refinement speedup or a measured rate of useful
real-relation discovery.

A separate real-source recovery control over ordinals `[45000,45064)` found no
candidates and did not restore its deleted reference map
(`reference_maps_equal = false`). That is a failed prefix recovery test, not a
statement about the full source span. Its earlier harness exit status of zero
was misleading; the harness source now treats reference-map inequality as failure.
That run is not counted as successful recovery or included in the table.

The separate pre-cache positive recovery cohort has now completed successfully
on the saved five-loop input, using the same probe SHA on CPUs 224–231. Starting
from a native baseline with one known terminal row deliberately removed, it
scanned 54,909 sources and recovered that relation with one candidate, one
accepted circuit and zero reconstruction misses. Its full terminal maps matched
the reference publication and survived cold reload, with 909 remaining
terminals. All inherited reference guards were retained; this does not assert
equality of the complete guard sets. The exact source cursor remained unchanged
and `search_exhausted` was false, so this is a successful known-relation recovery
cohort rather than an exhausted production refinement.

The solver phase took 796.950 seconds: 46.225 seconds for source preparation and
discovery, and 750.725 seconds for reconstruction plus exact verification.
Initialization took 2.431 seconds and cold reload 91.783 seconds. The complete
process used 899.421 seconds wall time, 897.686 seconds CPU, and a peak RSS of
5.43 GB (decimal). Results and the external resource receipt are
`TMP/modular-master-refinement-20261021/positive-release-full.{json,receipt.json}`.
No exact control was run for this cohort (`exact_control = null`), so it supplies
no speedup comparison.

### Rejected native evaluation-cache experiment

A subsequent experiment cached each source polynomial's finite-field image
per prime using native Symbolica `map_coeff`, then native `replace_all` at each
point. Five independent equivalence/pole/accounting tests passed, as did the
small release controls. However, the same real recovery workload had returned
no result after 925.639 seconds, with 15.822 GB peak RSS, versus the completed
899.421-second, 5.430-GB baseline. Root interrupted **only that scratch process**
and reverted the optimization. This interrupted result is not a completed
timing, a failed algebraic recovery, or a measured final runtime ratio.

The cause is consistent with the native APIs: conversion is saved, but both
evaluators still use the same monomial/power work, while every retained prime
duplicates exponent arrays and per-polynomial contexts. The shipped backend
therefore retains the successful baseline implementation, not this cache.
Raw evidence, the explicit interruption disposition and the reversible source
patch remain under `TMP/modular-master-refinement-20261021/`.

A dense univariate native representation/Horner path is deferred until a
separate memory-admitted, measured trial justifies it. No custom evaluator or
reconstruction code is proposed. No full-refinement speedup is established.
The earlier millisecond coefficient-only reconstruction diagnostic also did
not solve the master-refinement system.

## Frozen delivery

The final optimized CLI is
`TMP/releases/20261021-modular-master-refinement/rustred`, SHA256
`83f19eb0600400b9642a5bf5dff8cb29d0d1d557ac4b62aa6ef70888914d98d7`.
Its release diagnostic probe is beside it. The frozen manifest records the
source commit and commands; neither binaries nor campaign outputs are pushed.

The final binary passed fresh-process stop/resume (six workers to one), all
small one/six-worker positive controls, and the real next-64-source six-worker
control. The latter preserved exact control bytes
`2f5db653f8a209a733c1a76f34cfa7ffbe1c3d6f6688962bfbdfb87adfd79b30`
and modular checkpoint bytes
`0a274a6e09279af51b931490f6fc4a9cbd24e1c589f88b8d77547d7dc26a578d`.
It took 0.118 s for heuristic scanning plus 2.535 s initialization, compared
with 60.010 s exact insertion on that negative prefix; the earlier workload
and authority qualifications still apply. No extra terminal relation was
found in that prefix. Final receipts have the `delivery-` prefix in the same
TMP evidence directory.

Exact refinement remains available and default. An already completed exact
search is not enlarged merely by selecting a different backend with the same
source recipe; additional seed/containing-sector depth is a separate request.
Never run competing refiners writing the same campaign concurrently.
